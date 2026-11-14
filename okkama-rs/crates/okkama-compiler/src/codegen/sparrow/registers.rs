//! Регистры Sparrow: значения IR, которые никогда не живут одновременно,
//! делят регистр одного типа.
//!
//! Без этого регистр — номер значения, и кадр функции с `match` на шесть
//! ветвей держит регистры всех ветвей: вызывающий очищает и освобождает их
//! при каждом вызове. Живость считается по блокам (обратный поток данных),
//! конфликты — в точках определения, раскраска жадная.
//!
//! Консервативно: результат инструкции не делит регистр с её операндами
//! (машинный код может записать результат раньше, чем прочитает операнд), а
//! значения, нужные обработчику unwind блока, живут весь блок — отказ
//! возможен в любой его инструкции.
use crate::ir::{Function, ValueId};

/// Множество значений, разреженное (#50): копия и обход стоят числа живых
/// значений, а не числа значений функции, — иначе кодоген функции на 10⁵
/// инструкций квадратичен, даже когда одновременно живут два значения.
#[derive(Clone, PartialEq, Eq)]
struct Set(std::collections::BTreeSet<usize>);

impl Set {
    fn new(_count: usize) -> Self {
        Self(std::collections::BTreeSet::new())
    }

    fn insert(&mut self, value: ValueId) {
        self.0.insert(value.0);
    }

    fn remove(&mut self, value: ValueId) {
        self.0.remove(&value.0);
    }

    fn union(&mut self, other: &Set) {
        self.0.extend(other.0.iter().copied());
    }

    fn values(&self) -> impl Iterator<Item = ValueId> + '_ {
        self.0.iter().map(|&value| ValueId(value))
    }
}

/// Наибольшее число рёбер графа конфликтов. Функция, где тысячи значений
/// живут одновременно, даёт квадратичный граф (20 000 живых значений —
/// сотни миллионов рёбер и гигабайты памяти); сверх бюджета каждое значение
/// получает свой регистр, как без раскраски.
const EDGE_BUDGET: usize = 1 << 22;

/// Функция, в которой номер значения — номер регистра после раскраски.
pub(super) fn allocate(function: &Function) -> Function {
    static PROFILE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *PROFILE.get_or_init(|| std::env::var_os("OKKAMA_PROFILE_REGISTERS").is_some_and(|v| v == "1")) {
        allocate_profiled::<true>(function)
    } else {
        allocate_profiled::<false>(function)
    }
}

// The const parameter removes per-instruction clocks/counters from ordinary
// compilation. Both modes execute the very same baseline allocator (#540).
fn allocate_profiled<const PROFILE: bool>(function: &Function) -> Function {
    let mut profile = Profile::<PROFILE>::new();
    let count = function.values.len();
    if count == 0 || function.blocks.is_empty() {
        return profile.fallback(function, "empty");
    }
    let live_in = live_in(function);
    profile.phase(0);
    let mut conflicts: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut edges = 0usize;
    let mut conflict = |value: ValueId, live: &Set| {
        for other in live.values() {
            if other != value {
                conflicts[value.0].push(other.0);
                conflicts[other.0].push(value.0);
                edges += 1;
            }
        }
        edges <= EDGE_BUDGET
    };
    for (index, block) in function.blocks.iter().enumerate() {
        let unwind = unwind_live(function, index, &live_in);
        let mut live = live_out(function, index, &live_in);
        live.union(&unwind);
        for operand in block.term.kind.operands() {
            live.insert(operand);
        }
        for inst in block.insts.iter().rev() {
            let clone_start = PROFILE.then(std::time::Instant::now);
            let mut busy = live.clone();
            if let Some(start) = clone_start {
                profile.busy_clone_ns += start.elapsed().as_nanos();
                profile.busy_clones += 1;
                profile.busy_clone_values += live.0.len();
            }
            for operand in inst.kind.operands() {
                busy.insert(operand);
            }
            if let Some(result) = inst.result {
                if !conflict(result, &busy) {
                    profile.phase(1);
                    profile.graph(&conflicts, edges);
                    return profile.fallback(function, "edge-budget");
                }
                live.remove(result);
            }
            for operand in inst.kind.operands() {
                live.insert(operand);
            }
            if PROFILE {
                profile.unwind_insert_visits += unwind.0.len();
            }
            live.union(&unwind);
        }
        // Параметры записываются переходом все вместе (у входа — и параметры функции).
        let mut params = block.params.clone();
        if index == 0 {
            params.extend(&function.params);
        }
        for param in &params {
            live.insert(*param);
        }
        for param in &params {
            if !conflict(*param, &live) {
                profile.phase(1);
                profile.graph(&conflicts, edges);
                return profile.fallback(function, "edge-budget");
            }
        }
    }
    profile.phase(1);
    profile.graph(&conflicts, edges);
    profile.restart();
    // Аргумент перехода и параметр цели в одном регистре — без записи на ребре.
    let mut affinity: Vec<Vec<usize>> = vec![Vec::new(); count];
    for block in &function.blocks {
        for target in block.term.kind.targets() {
            for (param, arg) in function.blocks[target.block.0].params.iter().zip(&target.args) {
                affinity[param.0].push(arg.0);
                affinity[arg.0].push(param.0);
            }
        }
    }
    profile.phase(2);
    // Параметры функции — первыми: их регистры различны и малы.
    let mut order: Vec<usize> = function.params.iter().map(|param| param.0).collect();
    order.extend((0..count).filter(|value| !function.params.contains(&ValueId(*value))));
    let mut colors: Vec<Option<usize>> = vec![None; count];
    let mut types: Vec<crate::ir::Type> = Vec::new();
    for value in order {
        let taken: Vec<usize> = conflicts[value].iter().filter_map(|other| colors[*other]).collect();
        let ty = &function.values[value];
        let free = |color: &usize| types[*color] == *ty && !taken.contains(color);
        let preferred = affinity[value].iter().filter_map(|other| colors[*other]).find(free);
        let color = preferred.or_else(|| (0..types.len()).find(free)).unwrap_or_else(|| {
            types.push(ty.clone());
            types.len() - 1
        });
        colors[value] = Some(color);
    }
    profile.phase(3);
    let rename = |value: &mut ValueId| *value = ValueId(colors[value.0].expect("colored"));
    let mut out = function.clone();
    profile.phase(4);
    out.values = types;
    out.params.iter_mut().for_each(rename);
    for block in &mut out.blocks {
        block.params.iter_mut().for_each(rename);
        for inst in &mut block.insts {
            inst.result.iter_mut().for_each(rename);
            inst.kind.operands_mut().into_iter().for_each(rename);
        }
        block.term.kind.operands_mut().into_iter().for_each(rename);
        for target in block.term.kind.targets_mut().into_iter().chain(block.unwind.as_mut()) {
            target.args.iter_mut().for_each(rename);
        }
    }
    profile.phase(5);
    profile.report(function, &out, "none");
    out
}

/// Diagnostic counts are deliberately not a new allocator/budget policy.
/// `conflict_pairs` includes duplicate pairs, `graph_capacity_bytes` includes
/// actual adjacency capacities and Vec headers, but not the liveness trees,
/// input/output IR, allocator metadata or the rest of the process (use RSS).
struct Profile<const ON: bool> {
    last: Option<std::time::Instant>,
    ns: [u128; 6],
    busy_clone_ns: u128,
    busy_clones: usize,
    busy_clone_values: usize,
    unwind_insert_visits: usize,
    conflict_pairs: usize,
    graph_capacity_bytes: usize,
}

impl<const ON: bool> Profile<ON> {
    fn new() -> Self {
        Self {
            last: ON.then(std::time::Instant::now),
            ns: [0; 6],
            busy_clone_ns: 0,
            busy_clones: 0,
            busy_clone_values: 0,
            unwind_insert_visits: 0,
            conflict_pairs: 0,
            graph_capacity_bytes: 0,
        }
    }

    fn phase(&mut self, phase: usize) {
        if ON {
            let now = std::time::Instant::now();
            self.ns[phase] = now.duration_since(self.last.expect("profiling clock")).as_nanos();
            self.last = Some(now);
        }
    }

    fn restart(&mut self) {
        if ON {
            self.last = Some(std::time::Instant::now());
        }
    }

    fn graph(&mut self, conflicts: &[Vec<usize>], pairs: usize) {
        if ON {
            self.conflict_pairs = pairs;
            self.graph_capacity_bytes = std::mem::size_of_val(conflicts)
                + conflicts.iter().map(|neighbors| neighbors.capacity() * std::mem::size_of::<usize>()).sum::<usize>();
        }
    }

    fn fallback(mut self, function: &Function, reason: &str) -> Function {
        self.restart();
        let out = function.clone();
        self.phase(4);
        self.report(function, &out, reason);
        out
    }

    fn report(&self, input: &Function, out: &Function, fallback: &str) {
        if ON {
            let instructions: usize = input.blocks.iter().map(|b| b.insts.len()).sum();
            let moves: usize = out.blocks.iter().flat_map(|b| b.term.kind.targets().into_iter().chain(b.unwind.as_ref()))
                .map(|target| out.blocks[target.block.0].params.iter().zip(&target.args).filter(|(param, arg)| param != arg).count())
                .sum();
            eprintln!(
                "register-colouring symbol={:?} instructions={instructions} blocks={} values={} registers={} fallback={fallback} liveness_ns={} conflicts_ns={} affinity_ns={} colour_ns={} clone_ns={} rename_ns={} busy_clone_ns={} busy_clones={} busy_clone_values={} unwind_reinsert_visits={} conflict_pairs={} adjacency_entries={} graph_capacity_bytes={} edge_moves={moves}",
                input.symbol, input.blocks.len(), input.values.len(), out.values.len(),
                self.ns[0], self.ns[1], self.ns[2], self.ns[3], self.ns[4], self.ns[5],
                self.busy_clone_ns, self.busy_clones, self.busy_clone_values,
                self.unwind_insert_visits, self.conflict_pairs, self.conflict_pairs * 2, self.graph_capacity_bytes,
            );
        }
    }
}

/// Живые на входе каждого блока (с его параметрами).
fn live_in(function: &Function) -> Vec<Set> {
    let count = function.values.len();
    let mut live_in = vec![Set::new(count); function.blocks.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for index in (0..function.blocks.len()).rev() {
            let block = &function.blocks[index];
            let unwind = unwind_live(function, index, &live_in);
            let mut live = live_out(function, index, &live_in);
            live.union(&unwind);
            for operand in block.term.kind.operands() {
                live.insert(operand);
            }
            for inst in block.insts.iter().rev() {
                if let Some(result) = inst.result {
                    live.remove(result);
                }
                for operand in inst.kind.operands() {
                    live.insert(operand);
                }
            }
            live.union(&unwind);
            if live != live_in[index] {
                live_in[index] = live;
                changed = true;
            }
        }
    }
    live_in
}

/// Нужное после перехода: живое на входе цели без её параметров и аргументы.
fn through(function: &Function, target: &crate::ir::Target, live_in: &[Set], into: &mut Set) {
    let mut needed = live_in[target.block.0].clone();
    for param in &function.blocks[target.block.0].params {
        needed.remove(*param);
    }
    into.union(&needed);
    for arg in &target.args {
        into.insert(*arg);
    }
}

fn live_out(function: &Function, index: usize, live_in: &[Set]) -> Set {
    let mut out = Set::new(function.values.len());
    for target in function.blocks[index].term.kind.targets() {
        through(function, target, live_in, &mut out);
    }
    out
}

/// Нужное обработчику unwind блока: живёт весь блок.
fn unwind_live(function: &Function, index: usize, live_in: &[Set]) -> Set {
    let mut out = Set::new(function.values.len());
    if let Some(unwind) = &function.blocks[index].unwind {
        through(function, unwind, live_in, &mut out);
    }
    out
}
