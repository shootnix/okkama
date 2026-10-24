//! Рекурсия в OIR: проверка бесконечной рекурсии и хвостовой самовызов → цикл.
//!
//! **Бесконечная рекурсия.** Функция ошибочна, если ни один путь от её входа
//! не завершает её (возврат или отказ), не пройдя сначала через синхронный
//! вызов функции той же группы рекурсии. Группа — наибольшее множество `S`
//! функций со свойством «каждый путь до завершения проходит через вызов из
//! `S`» (наибольшая неподвижная точка). Анализ консервативен: любая
//! инструкция, которая может отказать (вызов вне группы, `await`, деление,
//! индекс, …), считается возможным завершением, а динамические вызовы
//! (`call.fn`, `call.role`) — не рекурсией. Поэтому ошибка — только там, где
//! вызов функции заведомо не может завершиться ничем, кроме исчерпания стека
//! или бесконечного цикла.
//!
//! **Хвостовой самовызов.** `r = call F(args); ret r` в самой `F` (без
//! обработчика отказа у блока) становится переходом к началу тела с новыми
//! аргументами: рекурсия в хвостовой позиции исполняется циклом на всех
//! движках, стек не растёт.
use super::{Attr, Block, BlockId, FuncId, Function, Inst, InstKind, IntBinaryOp, IntUnaryOp, Module, Target, TermKind, Terminator, ValueId};
use std::collections::BTreeSet;

/// Функции, которые не могут завершиться без рекурсивного вызова.
pub fn infinite(module: &Module) -> Vec<FuncId> {
    let mut group: BTreeSet<usize> = recursive(module);
    loop {
        let finishing: Vec<usize> = group
            .iter()
            .copied()
            .filter(|&index| can_finish(&module.functions[index], &group))
            .collect();
        if finishing.is_empty() {
            break;
        }
        for index in finishing {
            group.remove(&index);
        }
    }
    group.into_iter().map(FuncId).collect()
}

/// Синхронные прямые вызовы функции.
fn callees(module: &Module, function: &Function) -> Vec<usize> {
    let mut out: Vec<usize> = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter_map(|inst| match &inst.kind {
            InstKind::Call { func, .. } if !module.functions[func.0].is_async() => Some(func.0),
            _ => None,
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Функции на циклах графа прямых вызовов (Тарьян, без рекурсии Rust).
fn recursive(module: &Module) -> BTreeSet<usize> {
    let count = module.functions.len();
    let edges: Vec<Vec<usize>> = module
        .functions
        .iter()
        .map(|function| if function.is_async() { Vec::new() } else { callees(module, function) })
        .collect();
    let mut index = vec![usize::MAX; count];
    let mut low = vec![0; count];
    let mut on_stack = vec![false; count];
    let mut stack = Vec::new();
    let mut next = 0;
    let mut result = BTreeSet::new();
    for root in 0..count {
        if index[root] != usize::MAX {
            continue;
        }
        // Кадр обхода: вершина и номер следующего ребра.
        let mut frames = vec![(root, 0usize)];
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&(node, edge)) = frames.last() {
            if let Some(&to) = edges[node].get(edge) {
                frames.last_mut().expect("frame").1 += 1;
                if index[to] == usize::MAX {
                    index[to] = next;
                    low[to] = next;
                    next += 1;
                    stack.push(to);
                    on_stack[to] = true;
                    frames.push((to, 0));
                } else if on_stack[to] {
                    low[node] = low[node].min(index[to]);
                }
                continue;
            }
            frames.pop();
            if let Some(&(parent, _)) = frames.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                let mut component = Vec::new();
                loop {
                    let member = stack.pop().expect("tarjan stack");
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                if component.len() > 1 || edges[node].contains(&node) {
                    result.extend(component);
                }
            }
        }
    }
    result
}

/// Инструкция не может отказать (и не завершает функцию).
fn total(kind: &InstKind) -> bool {
    match kind {
        InstKind::IntBinary { op, .. } => matches!(op, IntBinaryOp::Add | IntBinaryOp::Sub | IntBinaryOp::Mul),
        InstKind::IntUnary { op, .. } => matches!(op, IntUnaryOp::Neg),
        InstKind::Const(_)
        | InstKind::IntCompare { .. }
        | InstKind::BoolNot(_)
        | InstKind::BoolCompare { .. }
        | InstKind::StrConcat { .. }
        | InstKind::StrCompare { .. }
        | InstKind::ObjNew { .. }
        | InstKind::ObjGet { .. }
        | InstKind::AnyBox(_)
        | InstKind::FnClosure { .. }
        | InstKind::EnumCase { .. }
        | InstKind::EnumCompare { .. }
        | InstKind::Undef
        | InstKind::ArrayNew(_)
        | InstKind::ArrayLen(_)
        | InstKind::BuilderNew
        | InstKind::IntAbs(_)
        | InstKind::IntSign(_)
        | InstKind::ByteCompare { .. }
        | InstKind::CharCompare { .. }
        | InstKind::RegionEnter
        | InstKind::RegionAdopt { .. }
        | InstKind::RegionTrack { .. }
        | InstKind::RegionTransfer { .. } => true,
        _ => false,
    }
}

/// Есть путь от входа до завершения, не проходящий через вызов из `group`.
fn can_finish(function: &Function, group: &BTreeSet<usize>) -> bool {
    let mut seen = vec![false; function.blocks.len()];
    let mut work = vec![BlockId(0)];
    seen[0] = true;
    fn visit(block: BlockId, work: &mut Vec<BlockId>, seen: &mut [bool]) {
        if !seen[block.0] {
            seen[block.0] = true;
            work.push(block);
        }
    }
    // Отказ в блоке: к его обработчику, иначе — завершение функции.
    fn fail(block: &Block, work: &mut Vec<BlockId>, seen: &mut [bool]) -> bool {
        match &block.unwind {
            Some(target) => {
                visit(target.block, work, seen);
                false
            }
            None => true,
        }
    }
    while let Some(id) = work.pop() {
        let block = &function.blocks[id.0];
        let mut recursed = false;
        for inst in &block.insts {
            if let InstKind::Call { func, .. } = &inst.kind
                && group.contains(&func.0)
            {
                recursed = true;
                break;
            }
            if !total(&inst.kind) && fail(block, &mut work, &mut seen) {
                return true;
            }
        }
        if recursed {
            continue;
        }
        match &block.term.kind {
            TermKind::Ret(_) | TermKind::Panic(_) => return true,
            TermKind::Trap { .. } | TermKind::Throw(_) | TermKind::Raise(_) => {
                if fail(block, &mut work, &mut seen) {
                    return true;
                }
            }
            TermKind::Unreachable => {}
            kind => {
                for target in kind.targets() {
                    visit(target.block, &mut work, &mut seen);
                }
            }
        }
    }
    false
}

/// Хвостовой самовызов: блок, чья последняя инструкция — вызов `self_id`,
/// результат которого сразу возвращается.
fn tail_call(function: &Function, self_id: FuncId, block: &Block) -> Option<Vec<ValueId>> {
    if block.unwind.is_some() {
        return None;
    }
    let last = block.insts.last()?;
    let InstKind::Call { func, args } = &last.kind else {
        return None;
    };
    if *func != self_id {
        return None;
    }
    // Возврат результата: прямо или через блок, который только возвращает параметр.
    let mut returned: Vec<ValueId> = last.result.into_iter().collect();
    let mut term = &block.term.kind;
    for _ in 0..4 {
        match term {
            TermKind::Ret(value) => {
                return (value.iter().copied().collect::<Vec<_>>() == returned).then(|| args.clone());
            }
            TermKind::Br(Target { block: next, args: passed }) => {
                let next = &function.blocks[next.0];
                if !next.insts.is_empty() || next.unwind.is_some() || *passed != returned {
                    return None;
                }
                returned = next.params.clone();
                term = &next.term.kind;
            }
            _ => return None,
        }
    }
    None
}

/// Заменить хвостовые самовызовы переходами; результат — число замен.
pub fn eliminate_tail_calls(module: &mut Module) -> usize {
    let mut replaced = 0;
    for index in 0..module.functions.len() {
        let function = &mut module.functions[index];
        let plain = !function.is_async()
            && !function.blocks.is_empty()
            && !function.attrs.iter().any(|attr| {
                matches!(
                    attr,
                    Attr::Ctor(_) | Attr::Successor(_) | Attr::Destroy(_) | Attr::Default(..) | Attr::Extern(_) | Attr::Intrinsic { .. }
                )
            });
        if !plain {
            continue;
        }
        let sites: Vec<(usize, Vec<ValueId>)> = function
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(block, body)| tail_call(function, FuncId(index), body).map(|args| (block, args)))
            .collect();
        if sites.is_empty() {
            continue;
        }
        // Вход становится переходом к телу; тело — блоком-циклом с новыми
        // значениями-параметрами (параметры функции остаются первыми значениями).
        let fresh: Vec<ValueId> = function
            .params
            .iter()
            .map(|param| {
                let ty = function.values[param.0].clone();
                function.values.push(ty);
                ValueId(function.values.len() - 1)
            })
            .collect();
        let rename = |value: &mut ValueId, params: &[ValueId], fresh: &[ValueId]| {
            if let Some(position) = params.iter().position(|param| param == value) {
                *value = fresh[position];
            }
        };
        let params = function.params.clone();
        for block in &mut function.blocks {
            for inst in &mut block.insts {
                for operand in inst.kind.operands_mut() {
                    rename(operand, &params, &fresh);
                }
            }
            for operand in block.term.kind.operands_mut() {
                rename(operand, &params, &fresh);
            }
            for target in block.term.kind.targets_mut().into_iter().chain(block.unwind.as_mut()) {
                for arg in &mut target.args {
                    rename(arg, &params, &fresh);
                }
            }
        }
        let sites: Vec<(usize, Vec<ValueId>)> = sites
            .into_iter()
            .map(|(block, mut args)| {
                for arg in &mut args {
                    rename(arg, &params, &fresh);
                }
                (block, args)
            })
            .collect();
        let body = BlockId(function.blocks.len());
        let span = function.blocks[0].term.span;
        let mut entry = std::mem::replace(
            &mut function.blocks[0],
            Block {
                params: Vec::new(),
                unwind: None,
                insts: Vec::new(),
                term: Terminator {
                    kind: TermKind::Br(Target { block: body, args: params }),
                    span,
                },
            },
        );
        entry.params = fresh;
        function.blocks.push(entry);
        for (block, args) in sites {
            let block = if block == 0 { body.0 } else { block };
            let site = &mut function.blocks[block];
            let call = site.insts.pop().expect("tail call");
            site.term = Terminator {
                kind: TermKind::Br(Target { block: body, args }),
                span: call.span,
            };
            // Значение результата больше не вычисляется, но объявлено: оно
            // определяется один раз на входе и нигде не читается.
            if let Some(result) = call.result {
                function.blocks[0].insts.push(Inst { result: Some(result), kind: InstKind::Undef, span: call.span });
            }
            replaced += 1;
        }
    }
    replaced
}
