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

/// Перенести константы циклов во входные блоки; число перенесённых.
pub fn hoist_loop_constants(module: &mut Module) -> usize {
    module.functions.iter_mut().map(hoist).sum()
}

fn hoist(function: &mut Function) -> usize {
    let count = function.blocks.len();
    if count < 2 {
        return 0;
    }
    let successors: Vec<Vec<usize>> = (0..count)
        .map(|block| all_successors(function, BlockId(block)).into_iter().map(|target| target.0).collect())
        .collect();
    if successors.iter().any(|targets| targets.contains(&0)) {
        return 0;
    }
    // Blocks on a cycle: one linear pass over the strongly connected
    // components, whatever the size of the function (#51).
    let looping = super::cfg::on_cycles(&successors);
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
