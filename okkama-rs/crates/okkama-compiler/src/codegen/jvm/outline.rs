//! Вынос частей слишком больших функций в отдельные функции: метод JVM не
//! длиннее 65535 байт байткода (JVMS §4.7.3), а сгенерированные данные (CLDR)
//! дают функции в сотни килобайт. Выносится замкнутая область — блоки,
//! доминируемые блоком `B`, из которых нет переходов наружу (кроме общей с
//! `B` цели unwind): она становится функцией над живыми на входе значениями
//! и параметрами `B`, а `B` — вызовом этой функции и возвратом её результата.
//! Проход внутренний для backend: IR уже проверен, атрибуты не нужны.

use std::collections::{BTreeSet, HashMap};

use crate::ir::{
    self, Block, BlockId, Function, Inst, InstKind, Module, ResultType, Target, TermKind,
    Terminator, ValueId,
};

/// Оценка байткода функции, выше которой она делится.
const LIMIT: usize = 36_000;
/// Желаемый размер выносимой области.
const CHUNK: usize = 24_000;

/// Делит функции модуля, чья оценка превышает лимит метода.
pub(super) fn split_large(module: &mut Module) {
    let mut index = 0;
    while index < module.functions.len() {
        if estimate(&module.functions[index]) > LIMIT {
            for block in 0..module.functions[index].blocks.len() {
                if block_weight(&module.functions[index].blocks[block]) > CHUNK {
                    chain_literals(&mut module.functions[index], block);
                }
            }
            outline_segments(module, index);
        }
        if estimate(&module.functions[index]) > LIMIT {
            split_blocks(&mut module.functions[index]);
        }
        let mut guard = 0;
        while estimate(&module.functions[index]) > LIMIT && guard < 10_000 {
            if !outline_once(module, index) {
                break;
            }
            guard += 1;
        }
        index += 1;
    }
}

/// Оценка байт инструкции: у больших функций индексы локалей `wide`
/// (4 байта на load/store), операнды часто упаковываются.
fn weight(inst: &Inst) -> usize {
    match &inst.kind {
        InstKind::Const(_) => 9,
        _ => 6 + 5 * (inst.kind.operands().len() + 1),
    }
}

fn block_weight(block: &Block) -> usize {
    12 + block.insts.iter().map(weight).sum::<usize>() + 4 * block.params.len()
}

fn estimate(function: &Function) -> usize {
    function.blocks.iter().map(block_weight).sum()
}

/// Больше живых значений на разрезе — область вышла бы с параметрами сверх
/// лимита метода JVM (255 слотов).
const CUT_LIVE: usize = 32;

/// Режет блоки тяжелее `CHUNK` на цепочку `br`-блоков в точках, где живо
/// мало значений: так длинный линейный код (большие литералы) становится
/// хвостами, которые выносит `outline_once`. SSA не меняется: верхний блок
/// цепочки доминирует нижний.
fn split_blocks(function: &mut Function) {
    let mut block = 0;
    while block < function.blocks.len() {
        if block_weight(&function.blocks[block]) > CHUNK {
            chain_literals(function, block);
        }
        if block_weight(&function.blocks[block]) > CHUNK && function.blocks[block].insts.len() > 1 {
            let cuts = cut_points(function, block);
            if !cuts.is_empty() {
                split_at(function, block, &cuts);
            }
        }
        block += 1;
    }
}

/// Большой литерал массива держит все элементы живыми до `arr.new`, и блок
/// не режется. Если элементы определены в этом блоке по порядку, литерал
/// становится цепочкой `builder.push` сразу за каждым элементом: живы лишь
/// сборщик и текущий элемент. Значение массива то же.
fn chain_literals(function: &mut Function, block: usize) {
    let mut at = 0;
    while at < function.blocks[block].insts.len() {
        let data = &function.blocks[block];
        let InstKind::ArrayNew(items) = &data.insts[at].kind else {
            at += 1;
            continue;
        };
        let Some(result) = data.insts[at].result else {
            at += 1;
            continue;
        };
        if items.len() <= CUT_LIVE {
            at += 1;
            continue;
        }
        // Позиции определений элементов: строго по возрастанию, до литерала.
        let position = |value: ValueId| data.insts[..at].iter().position(|inst| inst.result == Some(value));
        let mut positions = Vec::with_capacity(items.len());
        for item in items {
            match position(*item) {
                Some(found) if positions.last().is_none_or(|last| found > *last) => positions.push(found),
                _ => break,
            }
        }
        if positions.len() != items.len() {
            at += 1;
            continue;
        }
        let ir::Type::Array(element) = function.values[result.0].clone() else {
            at += 1;
            continue;
        };
        let builder_type = ir::Type::Builder(element);
        let span = data.insts[at].span;
        let items = items.clone();
        let old = std::mem::take(&mut function.blocks[block].insts);
        let mut insts = Vec::with_capacity(old.len() + items.len() + 1);
        function.values.push(builder_type.clone());
        let mut builder = ValueId(function.values.len() - 1);
        insts.push(Inst {
            result: Some(builder),
            kind: InstKind::BuilderNew,
            span,
        });
        let mut next = 0;
        for (index, inst) in old.into_iter().enumerate() {
            if index == at {
                insts.push(Inst {
                    result: Some(result),
                    kind: InstKind::BuilderFinish(builder),
                    span,
                });
                continue;
            }
            insts.push(inst);
            if next < positions.len() && positions[next] == index {
                function.values.push(builder_type.clone());
                let pushed = ValueId(function.values.len() - 1);
                insts.push(Inst {
                    result: Some(pushed),
                    kind: InstKind::BuilderPush {
                        builder,
                        value: items[next],
                    },
                    span,
                });
                builder = pushed;
                next += 1;
            }
        }
        let shift = items.len() + 1;
        function.blocks[block].insts = insts;
        at += shift + 1;
    }
}

/// Инструкции, которым нужен контекст своей функции (async, закрытие региона).
fn movable(inst: &Inst) -> bool {
    !matches!(
        inst.kind,
        InstKind::Await(_)
            | InstKind::FutSpawn(_)
            | InstKind::RegionLeave { .. }
            | InstKind::RegionUnwind { .. }
    )
}

/// Выносит линейные отрезки блоков тяжелее `CHUNK` в функции: отрезок
/// весом около `CHUNK / 2`, которому на входе нужно не больше `CUT_LIVE`
/// значений, а после него живо не больше одного определённого в нём, —
/// становится вызовом, результат которого и есть это значение. Отказ
/// отрезка выходит из вызова в ту же цель unwind блока.
fn outline_segments(module: &mut Module, index: usize) {
    let mut block = 0;
    while block < module.functions[index].blocks.len() {
        let mut from = 0;
        while block_weight(&module.functions[index].blocks[block]) > CHUNK {
            let Some((start, end, live, out)) = segment(&module.functions[index], block, from) else {
                break;
            };
            extract(module, index, block, start, end, &live, out);
            from = start + 1;
        }
        block += 1;
    }
    compact_values(&mut module.functions[index]);
}

/// Отрезок `[start, end)` блока не раньше `from`: живые на входе и
/// единственное живое на выходе значение.
fn segment(function: &Function, block: usize, from: usize) -> Option<(usize, usize, Vec<usize>, Option<ValueId>)> {
    let data = &function.blocks[block];
    let count = data.insts.len();
    // Последнее использование значения в блоке; вне блока — навсегда.
    let mut last_use: HashMap<usize, usize> = HashMap::new();
    for (position, inst) in data.insts.iter().enumerate() {
        for operand in inst.kind.operands() {
            last_use.insert(operand.0, position);
        }
    }
    let mut external: Vec<ValueId> = data.term.kind.operands();
    for target in data.term.kind.targets() {
        external.extend(target.args.iter().copied());
    }
    if let Some(unwind) = &data.unwind {
        external.extend(unwind.args.iter().copied());
    }
    for (other, blockdata) in function.blocks.iter().enumerate() {
        if other == block {
            continue;
        }
        for inst in &blockdata.insts {
            external.extend(inst.kind.operands());
        }
        external.extend(blockdata.term.kind.operands());
        for target in blockdata.term.kind.targets() {
            external.extend(target.args.iter().copied());
        }
        if let Some(unwind) = &blockdata.unwind {
            external.extend(unwind.args.iter().copied());
        }
    }
    for value in external {
        last_use.insert(value.0, usize::MAX);
    }
    let mut start = from;
    while start < count {
        let mut defined: Vec<usize> = Vec::new();
        let mut defined_set: BTreeSet<usize> = BTreeSet::new();
        // Определённые в отрезке по последнему использованию: истёкшие — мертвы после отрезка.
        let mut by_last: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        let mut expired = 0usize;
        let mut live: BTreeSet<usize> = BTreeSet::new();
        let mut weight_so_far = 0;
        let mut end = start;
        let mut found = None;
        while end < count {
            let inst = &data.insts[end];
            if !movable(inst) {
                break;
            }
            for operand in inst.kind.operands() {
                if !defined_set.contains(&operand.0) {
                    live.insert(operand.0);
                }
            }
            if let Some(result) = inst.result {
                defined.push(result.0);
                defined_set.insert(result.0);
                // Не используемое нигде значение мертво сразу.
                let last = last_use.get(&result.0).copied().unwrap_or(end);
                *by_last.entry(last).or_insert(0) += 1;
            }
            weight_so_far += weight(inst);
            end += 1;
            if live.len() > CUT_LIVE {
                break;
            }
            while let Some((&last, &count_at)) = by_last.first_key_value() {
                if last >= end {
                    break;
                }
                expired += count_at;
                by_last.remove(&last);
            }
            if weight_so_far >= CHUNK / 2 {
                if defined.len() - expired <= 1 {
                    let out = defined
                        .iter()
                        .copied()
                        .find(|value| last_use.get(value).is_some_and(|last| *last >= end));
                    found = Some(out.map(ValueId));
                    break;
                }
                if weight_so_far >= CHUNK {
                    break;
                }
            }
        }
        if let Some(out) = found {
            return Some((start, end, live.into_iter().collect(), out));
        }
        start += 1;
        // Начало отрезка — после инструкции с большим числом живых результатов.
        while start < count && !movable(&data.insts[start]) {
            start += 1;
        }
    }
    None
}

/// Заменить отрезок блока вызовом новой функции над живыми значениями.
fn extract(module: &mut Module, index: usize, block: usize, start: usize, end: usize, live: &[usize], out: Option<ValueId>) {
    let new_func = ir::FuncId(module.functions.len());
    let function = &mut module.functions[index];
    let mut values: Vec<ir::Type> = Vec::new();
    let mut map: HashMap<usize, ValueId> = HashMap::new();
    let mut params = Vec::new();
    for value in live {
        values.push(function.values[*value].clone());
        let id = ValueId(values.len() - 1);
        map.insert(*value, id);
        params.push(id);
    }
    let removed: Vec<Inst> = function.blocks[block].insts.drain(start..end).collect();
    let mut insts = Vec::with_capacity(removed.len());
    for mut inst in removed {
        if let Some(result) = inst.result {
            values.push(function.values[result.0].clone());
            let id = ValueId(values.len() - 1);
            map.insert(result.0, id);
            inst.result = Some(id);
        }
        for operand in inst.kind.operands_mut() {
            *operand = map[&operand.0];
        }
        insts.push(inst);
    }
    let span = insts.first().map_or(function.span, |inst| inst.span);
    let result = match out {
        Some(value) => ResultType::Value(function.values[value.0].clone()),
        None => ResultType::Void,
    };
    let outlined = Function {
        symbol: format!("{}$part{}", function.symbol, new_func.0),
        params,
        result,
        attrs: Vec::new(),
        values,
        blocks: vec![Block {
            params: Vec::new(),
            unwind: None,
            insts,
            term: Terminator {
                kind: TermKind::Ret(out.map(|value| map[&value.0])),
                span,
            },
        }],
        span: function.span,
    };
    function.blocks[block].insts.insert(
        start,
        Inst {
            result: out,
            kind: InstKind::Call {
                func: new_func,
                args: live.iter().map(|value| ValueId(*value)).collect(),
            },
            span,
        },
    );
    module.functions.push(outlined);
}

/// Позиции разреза блока: перед инструкцией, через ~`CHUNK / 2` веса, где
/// хвосту нужно не больше `CUT_LIVE` внешних значений.
fn cut_points(function: &Function, block: usize) -> Vec<usize> {
    let idom = dominators(function);
    // Блоки, строго доминируемые `block`.
    let dominated = |mut other: usize| -> bool {
        if other == block {
            return false;
        }
        loop {
            match idom[other] {
                Some(parent) if parent == other => return false,
                Some(parent) if parent == block => return true,
                Some(parent) => other = parent,
                None => return false,
            }
        }
    };
    let data = &function.blocks[block];
    let mut live: BTreeSet<usize> = BTreeSet::new();
    let mut defined: BTreeSet<usize> = BTreeSet::new();
    let mut uses: Vec<ValueId> = data.term.kind.operands();
    for target in data.term.kind.targets() {
        uses.extend(target.args.iter().copied());
    }
    if let Some(unwind) = &data.unwind {
        uses.extend(unwind.args.iter().copied());
    }
    for (index, other) in function.blocks.iter().enumerate() {
        if !dominated(index) {
            continue;
        }
        defined.extend(other.params.iter().map(|value| value.0));
        for inst in &other.insts {
            defined.extend(inst.result.map(|value| value.0));
            uses.extend(inst.kind.operands());
        }
        uses.extend(other.term.kind.operands());
        for target in other.term.kind.targets() {
            uses.extend(target.args.iter().copied());
        }
        if let Some(unwind) = &other.unwind {
            uses.extend(unwind.args.iter().copied());
        }
    }
    live.extend(
        uses.iter()
            .map(|value| value.0)
            .filter(|value| !defined.contains(value)),
    );
    // Число живых перед каждой инструкцией (обратный проход).
    let mut counts = vec![0usize; data.insts.len()];
    for (index, inst) in data.insts.iter().enumerate().rev() {
        if let Some(result) = inst.result {
            live.remove(&result.0);
        }
        live.extend(inst.kind.operands().iter().map(|value| value.0));
        counts[index] = live.len();
    }
    let mut cuts = Vec::new();
    let mut weight_since = 0;
    for (index, inst) in data.insts.iter().enumerate() {
        if index > 0 && weight_since >= CHUNK / 2 && counts[index] <= CUT_LIVE {
            cuts.push(index);
            weight_since = 0;
        }
        weight_since += weight(inst);
    }
    cuts
}

fn split_at(function: &mut Function, block: usize, cuts: &[usize]) {
    let data = &mut function.blocks[block];
    let unwind = data.unwind.clone();
    let span = data.term.span;
    let mut insts = std::mem::take(&mut data.insts);
    let term = data.term.clone();
    let mut pieces = Vec::new();
    for cut in cuts.iter().rev() {
        pieces.push(insts.split_off(*cut));
    }
    pieces.reverse();
    let first = function.blocks.len();
    function.blocks[block].insts = insts;
    function.blocks[block].term = Terminator {
        kind: TermKind::Br(Target {
            block: BlockId(first),
            args: Vec::new(),
        }),
        span,
    };
    let count = pieces.len();
    for (position, piece) in pieces.into_iter().enumerate() {
        let term = if position + 1 == count {
            term.clone()
        } else {
            Terminator {
                kind: TermKind::Br(Target {
                    block: BlockId(first + position + 1),
                    args: Vec::new(),
                }),
                span,
            }
        };
        function.blocks.push(Block {
            params: Vec::new(),
            unwind: unwind.clone(),
            insts: piece,
            term,
        });
    }
}

/// Преемники блока: цели терминатора и цель unwind.
fn successors(block: &Block) -> Vec<usize> {
    let mut out: Vec<usize> = block
        .term
        .kind
        .targets()
        .iter()
        .map(|target| target.block.0)
        .collect();
    if let Some(unwind) = &block.unwind {
        out.push(unwind.block.0);
    }
    out
}

/// Непосредственные доминаторы (Cooper–Harvey–Kennedy); `None` — недостижим.
fn dominators(function: &Function) -> Vec<Option<usize>> {
    let count = function.blocks.len();
    let mut order = Vec::new();
    let mut seen = vec![false; count];
    let mut stack = vec![(0usize, 0usize)];
    seen[0] = true;
    while let Some((block, next)) = stack.pop() {
        let succ = successors(&function.blocks[block]);
        if next < succ.len() {
            stack.push((block, next + 1));
            let target = succ[next];
            if !seen[target] {
                seen[target] = true;
                stack.push((target, 0));
            }
        } else {
            order.push(block);
        }
    }
    order.reverse();
    let mut rpo = vec![usize::MAX; count];
    for (index, block) in order.iter().enumerate() {
        rpo[*block] = index;
    }
    let mut preds = vec![Vec::new(); count];
    for &block in &order {
        for target in successors(&function.blocks[block]) {
            preds[target].push(block);
        }
    }
    let mut idom: Vec<Option<usize>> = vec![None; count];
    idom[0] = Some(0);
    let intersect = |idom: &Vec<Option<usize>>, mut a: usize, mut b: usize| {
        while a != b {
            while rpo[a] > rpo[b] {
                a = idom[a].expect("processed");
            }
            while rpo[b] > rpo[a] {
                b = idom[b].expect("processed");
            }
        }
        a
    };
    let mut changed = true;
    while changed {
        changed = false;
        for &block in order.iter().skip(1) {
            let mut new = None;
            for &pred in &preds[block] {
                if idom[pred].is_none() {
                    continue;
                }
                new = Some(match new {
                    None => pred,
                    Some(current) => intersect(&idom, pred, current),
                });
            }
            if new.is_some() && idom[block] != new {
                idom[block] = new;
                changed = true;
            }
        }
    }
    idom
}

/// Вынести одну замкнутую область функции `index`; `false` — нечего.
///
/// Поддерево доминаторов — отрезок прямого порядка обхода дерева, поэтому
/// вес и замкнутость переходов всех поддеревьев считаются за один проход
/// снизу вверх; полный обход нужен лишь для проверки unwind-целей, общих с
/// корнем.
fn outline_once(module: &mut Module, index: usize) -> bool {
    let function = &module.functions[index];
    if function.blocks.len() < 2 {
        return false;
    }
    let idom = dominators(function);
    let count = function.blocks.len();
    // Поддеревья доминаторов: дети каждого блока.
    let mut children = vec![Vec::new(); count];
    for (block, dom) in idom.iter().enumerate() {
        if let Some(dom) = dom {
            if *dom != block {
                children[*dom].push(block);
            }
        }
    }
    // Прямой порядок: поддерево `b` — блоки с pre в [pre[b], last[b]].
    let mut pre = vec![usize::MAX; count];
    let mut last = vec![0usize; count];
    let mut order = Vec::with_capacity(count);
    let mut stack = vec![0usize];
    while let Some(block) = stack.pop() {
        pre[block] = order.len();
        order.push(block);
        // Дети в обратном порядке: обход как у рекурсивного DFS.
        stack.extend(children[block].iter().rev().copied());
    }
    // Снизу вверх: вес, границы целей переходов и unwind-целей.
    let mut size = vec![0usize; count];
    let mut low = vec![usize::MAX; count];
    let mut high = vec![0usize; count];
    let mut unwind_low = vec![usize::MAX; count];
    let mut unwind_high = vec![0usize; count];
    for &block in order.iter().rev() {
        let data = &function.blocks[block];
        let mut weight = block_weight(data);
        let (mut lo, mut hi) = (usize::MAX, 0usize);
        for target in data.term.kind.targets() {
            let at = pre[target.block.0];
            lo = lo.min(at);
            hi = hi.max(at);
        }
        let (mut ulo, mut uhi) = (usize::MAX, 0usize);
        if let Some(unwind) = &data.unwind {
            let at = pre[unwind.block.0];
            ulo = at;
            uhi = at;
        }
        let mut end = pre[block];
        for &child in &children[block] {
            weight += size[child];
            lo = lo.min(low[child]);
            hi = hi.max(high[child]);
            ulo = ulo.min(unwind_low[child]);
            uhi = uhi.max(unwind_high[child]);
            end = end.max(last[child]);
        }
        size[block] = weight;
        low[block] = lo;
        high[block] = hi;
        unwind_low[block] = ulo;
        unwind_high[block] = uhi;
        last[block] = end;
    }
    let inside = |root: usize, at: usize| at != usize::MAX && pre[root] <= at && at <= last[root];
    let mut best: Option<(usize, usize)> = None;
    let mut smallest_large: Option<(usize, usize)> = None;
    for (root, dom) in idom.iter().enumerate().skip(1) {
        if dom.is_none() {
            continue;
        }
        let weight = size[root];
        if weight < 200 {
            continue;
        }
        // Вся функция без входа: вынос лишь переложил бы её в новую.
        if last[root] - pre[root] + 1 >= count - 1 && weight > LIMIT {
            continue;
        }
        // Переходы поддерева — только внутрь него.
        if low[root] != usize::MAX && !(inside(root, low[root]) && inside(root, high[root])) {
            continue;
        }
        // Unwind — внутрь поддерева или в общую с корнем цель.
        let unwinds_inside = unwind_low[root] == usize::MAX
            || (inside(root, unwind_low[root]) && inside(root, unwind_high[root]));
        if !unwinds_inside {
            let root_unwind = function.blocks[root].unwind.as_ref();
            let closed = order[pre[root]..=last[root]].iter().all(|block| {
                match &function.blocks[*block].unwind {
                    None => true,
                    Some(unwind) => inside(root, pre[unwind.block.0]) || Some(unwind) == root_unwind,
                }
            });
            if !closed {
                continue;
            }
        }
        if weight <= CHUNK {
            if best.is_none_or(|(_, current)| weight > current) {
                best = Some((root, weight));
            }
        } else if smallest_large.is_none_or(|(_, current)| weight < current) {
            smallest_large = Some((root, weight));
        }
    }
    let Some((root, _)) = best.or(smallest_large) else {
        return false;
    };
    let mut region: Vec<usize> = order[pre[root]..=last[root]].to_vec();
    region.sort_unstable();


    outline(module, index, root, &region);
    true
}

fn outline(module: &mut Module, index: usize, root: usize, region: &[usize]) {
    let new_func = ir::FuncId(module.functions.len());
    let function = &mut module.functions[index];
    let inside: BTreeSet<usize> = region.iter().copied().collect();
    // Значения, определённые в области.
    let mut defined: BTreeSet<usize> = BTreeSet::new();
    for &block in region {
        let data = &function.blocks[block];
        defined.extend(data.params.iter().map(|value| value.0));
        defined.extend(
            data.insts
                .iter()
                .filter_map(|inst| inst.result.map(|value| value.0)),
        );
    }
    // Живые на входе: используются в области, определены вне её.
    let mut live: BTreeSet<usize> = BTreeSet::new();
    for &block in region {
        let data = &function.blocks[block];
        let mut uses: Vec<ValueId> = Vec::new();
        for inst in &data.insts {
            uses.extend(inst.kind.operands());
        }
        uses.extend(data.term.kind.operands());
        for target in data.term.kind.targets() {
            uses.extend(target.args.iter().copied());
        }
        if let Some(unwind) = &data.unwind {
            uses.extend(unwind.args.iter().copied());
        }
        for value in uses {
            if !defined.contains(&value.0) {
                live.insert(value.0);
            }
        }
    }
    let live: Vec<usize> = live.into_iter().collect();
    let root_params = function.blocks[root].params.clone();

    // Новая функция: значения переименованы, блок 0 — переход в копию `root`.
    let mut values: Vec<ir::Type> = Vec::new();
    let mut map: HashMap<usize, ValueId> = HashMap::new();
    let fresh = |old: usize, values: &mut Vec<ir::Type>, function: &Function| -> ValueId {
        values.push(function.values[old].clone());
        ValueId(values.len() - 1)
    };
    let mut params = Vec::new();
    for &value in &live {
        let id = fresh(value, &mut values, function);
        map.insert(value, id);
        params.push(id);
    }
    let mut entry_args = Vec::new();
    for value in &root_params {
        let id = fresh(value.0, &mut values, function);
        params.push(id);
        entry_args.push(id);
    }
    for &value in &defined {
        let id = fresh(value, &mut values, function);
        map.insert(value, id);
    }
    let block_map: HashMap<usize, BlockId> = region
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, BlockId(position + 1)))
        .collect();
    let rename_target = |target: &Target| Target {
        block: block_map[&target.block.0],
        args: target.args.iter().map(|value| map[&value.0]).collect(),
    };
    let mut blocks = vec![Block {
        params: Vec::new(),
        unwind: None,
        insts: Vec::new(),
        term: Terminator {
            kind: TermKind::Br(Target {
                block: block_map[&root],
                args: entry_args,
            }),
            span: function.span,
        },
    }];
    let root_unwind = function.blocks[root].unwind.clone();
    for &block in region {
        let data = &function.blocks[block];
        let mut copy = data.clone();
        copy.params = data.params.iter().map(|value| map[&value.0]).collect();
        for inst in &mut copy.insts {
            if let Some(result) = &mut inst.result {
                *result = map[&result.0];
            }
            for operand in inst.kind.operands_mut() {
                *operand = map[&operand.0];
            }
        }
        for operand in copy.term.kind.operands_mut() {
            *operand = map[&operand.0];
        }
        for target in copy.term.kind.targets_mut() {
            *target = rename_target(target);
        }
        // Общая с `root` внешняя цель unwind: отказ уходит из функции.
        copy.unwind = match &data.unwind {
            Some(unwind) if inside.contains(&unwind.block.0) => Some(rename_target(unwind)),
            _ => None,
        };
        blocks.push(copy);
    }
    let result = function.result.clone();
    let outlined = Function {
        symbol: format!("{}$part{}", function.symbol, new_func.0),
        params,
        result: result.clone(),
        attrs: Vec::new(),
        values,
        blocks,
        span: function.span,
    };

    // Исходная функция: `root` вызывает вынесенную часть, остальное удалено.
    let mut args: Vec<ValueId> = live.iter().map(|value| ValueId(*value)).collect();
    args.extend(root_params.iter().copied());
    let span = function.blocks[root].term.span;
    let (call_result, ret) = match &result {
        ResultType::Value(ty) => {
            function.values.push(ty.clone());
            let value = ValueId(function.values.len() - 1);
            (Some(value), Some(value))
        }
        ResultType::Void => (None, None),
    };
    function.blocks[root] = Block {
        params: root_params,
        unwind: root_unwind,
        insts: vec![Inst {
            result: call_result,
            kind: InstKind::Call {
                func: new_func,
                args,
            },
            span,
        }],
        term: Terminator {
            kind: TermKind::Ret(ret),
            span,
        },
    };
    // Удалить остальные блоки области и перенумеровать переходы.
    let removed: BTreeSet<usize> = region
        .iter()
        .copied()
        .filter(|block| *block != root)
        .collect();
    let mut renumber = vec![0usize; function.blocks.len()];
    let mut next = 0;
    for (block, slot) in renumber.iter_mut().enumerate() {
        if !removed.contains(&block) {
            *slot = next;
            next += 1;
        }
    }
    let old = std::mem::take(&mut function.blocks);
    for (block, mut data) in old.into_iter().enumerate() {
        if removed.contains(&block) {
            continue;
        }
        for target in data.term.kind.targets_mut() {
            target.block = BlockId(renumber[target.block.0]);
        }
        if let Some(unwind) = &mut data.unwind {
            unwind.block = BlockId(renumber[unwind.block.0]);
        }
        function.blocks.push(data);
    }
    compact_values(function);
    module.functions.push(outlined);
}

/// Оставить только определённые значения функции (параметры, параметры
/// блоков, результаты инструкций) и перенумеровать их.
fn compact_values(function: &mut Function) {
    let mut map: HashMap<usize, ValueId> = HashMap::new();
    let mut values = Vec::new();
    let keep = |old: ValueId, values: &mut Vec<ir::Type>, map: &mut HashMap<usize, ValueId>| {
        if let std::collections::hash_map::Entry::Vacant(slot) = map.entry(old.0) {
            values.push(function.values[old.0].clone());
            slot.insert(ValueId(values.len() - 1));
        }
    };
    for param in &function.params {
        keep(*param, &mut values, &mut map);
    }
    for block in &function.blocks {
        for param in &block.params {
            keep(*param, &mut values, &mut map);
        }
        for inst in &block.insts {
            if let Some(result) = inst.result {
                keep(result, &mut values, &mut map);
            }
        }
    }
    let rename = |value: &mut ValueId| *value = map[&value.0];
    function.params.iter_mut().for_each(rename);
    for block in &mut function.blocks {
        block.params.iter_mut().for_each(rename);
        for inst in &mut block.insts {
            if let Some(result) = &mut inst.result {
                rename(result);
            }
            for operand in inst.kind.operands_mut() {
                rename(operand);
            }
        }
        for operand in block.term.kind.operands_mut() {
            rename(operand);
        }
        for target in block.term.kind.targets_mut() {
            target.args.iter_mut().for_each(rename);
        }
        if let Some(unwind) = &mut block.unwind {
            unwind.args.iter_mut().for_each(rename);
        }
    }
    function.values = values;
}
