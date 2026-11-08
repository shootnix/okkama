//! Константы циклов — перед циклом.
//!
//! Значение `const` определено один раз и неизменяемо, поэтому его можно
//! вычислить заранее: константы блоков, лежащих на цикле, переносятся в конец
//! ближайшего блока вне цикла, доминирующего над его входами, и не
//! исполняются на каждой итерации. Не во входной блок функции: там константа
//! жила бы от входа до своего цикла, и в функции из тысяч циклов тысячи
//! значений жили бы одновременно — живость и раскраска регистров стали бы
//! квадратичными (#239). Константы блоков вне циклов остаются на месте:
//! перенос заставил бы каждый вызов вычислять константы всех ветвей. Функция,
//! во входной блок которой есть переход, не меняется.
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
    // The cycle of each block: one linear pass over the strongly connected
    // components, whatever the size of the function (#51).
    let cycle = super::cfg::cycles(&successors);
    let (idom, rpo) = super::cfg::dominators(&successors);
    // Each cycle's constants go to the nearest block outside it that
    // dominates all its entries, hence every block of the cycle (#239).
    let mut entry: Vec<Option<usize>> = Vec::new();
    for (block, targets) in successors.iter().enumerate() {
        for &target in targets {
            let Some(component) = cycle[target] else { continue };
            if cycle[block] == Some(component) || idom[block].is_none() {
                continue;
            }
            if entry.len() <= component {
                entry.resize(component + 1, None);
            }
            entry[component] = Some(match entry[component] {
                None => block,
                Some(current) => super::cfg::common_dominator(&idom, &rpo, block, current),
            });
        }
    }
    let mut hoisted: Vec<Vec<super::Inst>> = vec![Vec::new(); count];
    for (block, body) in function.blocks.iter_mut().enumerate().skip(1) {
        let Some(component) = cycle[block] else { continue };
        // A cycle without a reachable entry stays as it is.
        let Some(mut target) = entry.get(component).copied().flatten() else { continue };
        while cycle[target] == Some(component) {
            target = idom[target].expect("reachable");
        }
        if !body.insts.iter().any(|inst| matches!(inst.kind, InstKind::Const(_))) {
            continue;
        }
        let (constants, rest) = std::mem::take(&mut body.insts)
            .into_iter()
            .partition(|inst| matches!(inst.kind, InstKind::Const(_)));
        body.insts = rest;
        hoisted[target].extend::<Vec<_>>(constants);
    }
    let mut moved = 0;
    for (block, constants) in hoisted.into_iter().enumerate() {
        moved += constants.len();
        function.blocks[block].insts.extend(constants);
    }
    moved
}
