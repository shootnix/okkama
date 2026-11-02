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

/// Множество значений: биты по номерам.
#[derive(Clone, PartialEq, Eq)]
struct Set(Vec<u64>);

impl Set {
    fn new(count: usize) -> Self {
        Self(vec![0; count.div_ceil(64)])
    }

    fn insert(&mut self, value: ValueId) {
        self.0[value.0 / 64] |= 1 << (value.0 % 64);
    }

    fn remove(&mut self, value: ValueId) {
        self.0[value.0 / 64] &= !(1 << (value.0 % 64));
    }

    fn union(&mut self, other: &Set) {
        for (word, more) in self.0.iter_mut().zip(&other.0) {
            *word |= more;
        }
    }

    fn values(&self) -> impl Iterator<Item = ValueId> + '_ {
        self.0.iter().enumerate().flat_map(|(index, word)| {
            (0..64).filter(move |bit| word & (1 << bit) != 0).map(move |bit| ValueId(index * 64 + bit))
        })
    }
}

/// Наибольшее число рёбер графа конфликтов. Функция, где тысячи значений
/// живут одновременно, даёт квадратичный граф (20 000 живых значений —
/// сотни миллионов рёбер и гигабайты памяти); сверх бюджета каждое значение
/// получает свой регистр, как без раскраски.
const EDGE_BUDGET: usize = 1 << 22;

/// Функция, в которой номер значения — номер регистра после раскраски.
pub(super) fn allocate(function: &Function) -> Function {
    let count = function.values.len();
    if count == 0 || function.blocks.is_empty() {
        return function.clone();
    }
    let live_in = live_in(function);
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
            let mut busy = live.clone();
            for operand in inst.kind.operands() {
                busy.insert(operand);
            }
            if let Some(result) = inst.result {
                if !conflict(result, &busy) {
                    return function.clone();
                }
                live.remove(result);
            }
            for operand in inst.kind.operands() {
                live.insert(operand);
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
                return function.clone();
            }
        }
    }
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
    let rename = |value: &mut ValueId| *value = ValueId(colors[value.0].expect("colored"));
    let mut out = function.clone();
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
    out
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
