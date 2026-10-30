//! Константы циклов — во входной блок функции.
//!
//! Значение `const` определено один раз и неизменяемо, поэтому его можно
//! вычислить заранее: константы блоков, лежащих на цикле, переносятся в конец
//! входного блока (он доминирует над всеми блоками) и не исполняются на
//! каждой итерации. Константы блоков вне циклов остаются на месте: перенос
//! заставил бы каждый вызов вычислять константы всех ветвей. Функция, во
//! входной блок которой есть переход, не меняется.
//!
//! Проход идёт до генерации кода всех движков, поэтому места инструкций
//! (`Function::span_at`) остаются согласованными с исполняемым кодом.
use super::cfg::all_successors;
use super::{BlockId, Function, InstKind, Module};

/// Наибольшее число блоков функции, для которой ищутся циклы.
const MAX_BLOCKS: usize = 4096;

/// Перенести константы циклов во входные блоки; число перенесённых.
pub fn hoist_loop_constants(module: &mut Module) -> usize {
    module.functions.iter_mut().map(hoist).sum()
}

fn hoist(function: &mut Function) -> usize {
    let count = function.blocks.len();
    if count < 2 || count > MAX_BLOCKS {
        return 0;
    }
    let successors: Vec<Vec<usize>> = (0..count)
        .map(|block| all_successors(function, BlockId(block)).into_iter().map(|target| target.0).collect())
        .collect();
    if successors.iter().any(|targets| targets.contains(&0)) {
        return 0;
    }
    let looping: Vec<bool> = (0..count).map(|block| reaches_itself(&successors, block)).collect();
    let mut hoisted = Vec::new();
    for (block, body) in function.blocks.iter_mut().enumerate().skip(1) {
        if !looping[block] || !body.insts.iter().any(|inst| matches!(inst.kind, InstKind::Const(_))) {
            continue;
        }
        let (constants, rest) = std::mem::take(&mut body.insts)
            .into_iter()
            .partition(|inst| matches!(inst.kind, InstKind::Const(_)));
        body.insts = rest;
        hoisted.extend::<Vec<_>>(constants);
    }
    let moved = hoisted.len();
    function.blocks[0].insts.extend(hoisted);
    moved
}

/// Есть ли путь из блока в него же.
fn reaches_itself(successors: &[Vec<usize>], start: usize) -> bool {
    let mut seen = vec![false; successors.len()];
    let mut stack: Vec<usize> = successors[start].clone();
    while let Some(block) = stack.pop() {
        if block == start {
            return true;
        }
        if !std::mem::replace(&mut seen[block], true) {
            stack.extend(&successors[block]);
        }
    }
    false
}
