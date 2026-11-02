//! Регионы, которые никогда не владеют идентичностями, — убираются.
//!
//! Lowering открывает регион в каждой области, если в программе есть
//! `destroy` (OIR-1 §7). Идентичности попадают в регион только через
//! `region.adopt`, `region.track` и `region.transfer`; регион, на который
//! ссылаются лишь его собственные `region.leave` и `region.unwind`, всегда
//! пуст, и его закрытие ничего не уничтожает и не освобождает. Такие
//! `region.leave` удаляются, а `region.enter` и `region.unwind` становятся
//! `undef` (их значения остаются определёнными, но не читаются): чтения
//! результата `region.unwind` заменяются отказом, который он передаёт.
//!
//! Обработчик unwind, который после этого только передаёт отказ дальше
//! (`undef` и `raise` своего параметра отказа), пропускается: блок уходит
//! сразу к его обработчику или из функции, а сам он удаляется. Иначе каждый
//! вызов машинного кода несёт ветку обработчика, который ничего не делает.
//!
//! Проход идёт до генерации кода всех движков, поэтому места инструкций
//! (`Function::span_at`) остаются согласованными с исполняемым кодом.
use std::collections::{HashMap, HashSet};

use super::{BlockId, Function, InstKind, Module, TermKind, ValueId, cfg};

/// Убрать пустые регионы во всех функциях; число убранных.
pub fn elide_unused_regions(module: &mut Module) -> usize {
    module
        .functions
        .iter_mut()
        .map(|function| {
            let elided = elide(function);
            if skip_rethrowing_unwinds(function) {
                cfg::remove_unreachable_blocks(function);
            }
            elided
        })
        .sum()
}

/// Блок только передаёт свой отказ дальше: `undef` и `raise` последнего параметра.
fn rethrows(function: &Function, block: BlockId) -> bool {
    let block = &function.blocks[block.0];
    let Some(failure) = block.params.last() else {
        return false;
    };
    block.insts.iter().all(|inst| matches!(inst.kind, InstKind::Undef))
        && matches!(block.term.kind, TermKind::Raise(value) if value == *failure)
}

/// Цели unwind, которые только передают отказ, заменяются их собственной
/// целью (или её отсутствием); true — что-то заменено.
fn skip_rethrowing_unwinds(function: &mut Function) -> bool {
    let mut changed = false;
    for index in 0..function.blocks.len() {
        // Не больше шагов, чем блоков: цикл из таких обработчиков не зациклит проход.
        for _ in 0..function.blocks.len() {
            let Some(target) = function.blocks[index].unwind.clone() else {
                break;
            };
            if target.block.0 == index || !rethrows(function, target.block) {
                break;
            }
            let forwarder = &function.blocks[target.block.0];
            let replacement = match &forwarder.unwind {
                None => None,
                Some(next) => {
                    // Аргументы обработчика: параметры пропускаемого блока — его
                    // аргументы отсюда; остальные значения доминируют и здесь.
                    let (failure, params) = forwarder.params.split_last().expect("failure parameter");
                    let defined: Vec<ValueId> = forwarder.insts.iter().filter_map(|inst| inst.result).collect();
                    let mut args = Vec::with_capacity(next.args.len());
                    for arg in &next.args {
                        if arg == failure || defined.contains(arg) {
                            break;
                        }
                        args.push(match params.iter().position(|param| param == arg) {
                            Some(position) => target.args[position],
                            None => *arg,
                        });
                    }
                    if args.len() != next.args.len() {
                        break;
                    }
                    Some(super::Target { block: next.block, args })
                }
            };
            function.blocks[index].unwind = replacement;
            changed = true;
        }
    }
    changed
}

fn elide(function: &mut Function) -> usize {
    let regions: HashSet<ValueId> = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| matches!(inst.kind, InstKind::RegionEnter))
        .filter_map(|inst| inst.result)
        .collect();
    if regions.is_empty() {
        return 0;
    }
    let mut used = HashSet::new();
    for block in &function.blocks {
        for inst in &block.insts {
            let operands = match &inst.kind {
                InstKind::RegionLeave { keep, .. } => keep.iter().copied().collect(),
                InstKind::RegionUnwind { failure, .. } => vec![*failure],
                kind => kind.operands(),
            };
            used.extend(operands.into_iter().filter(|value| regions.contains(value)));
        }
        let targets = block.term.kind.targets().into_iter().chain(block.unwind.as_ref());
        let passed = targets.flat_map(|target| target.args.iter().copied());
        let read = block.term.kind.operands().into_iter().chain(passed);
        used.extend(read.filter(|value| regions.contains(value)));
    }
    let unused: HashSet<ValueId> = regions.difference(&used).copied().collect();
    if unused.is_empty() {
        return 0;
    }
    // Результат `region.unwind` пустого региона — его отказ как есть.
    let mut replaced: HashMap<ValueId, ValueId> = HashMap::new();
    for block in &mut function.blocks {
        block.insts.retain(|inst| !matches!(&inst.kind, InstKind::RegionLeave { region, .. } if unused.contains(region)));
        for inst in &mut block.insts {
            match &inst.kind {
                InstKind::RegionEnter if inst.result.is_some_and(|region| unused.contains(&region)) => {
                    inst.kind = InstKind::Undef;
                }
                InstKind::RegionUnwind { region, failure } if unused.contains(region) => {
                    if let Some(result) = inst.result {
                        replaced.insert(result, *failure);
                    }
                    inst.kind = InstKind::Undef;
                }
                _ => {}
            }
        }
    }
    if !replaced.is_empty() {
        let resolve = |mut value: ValueId| {
            while let Some(next) = replaced.get(&value) {
                value = *next;
            }
            value
        };
        for block in &mut function.blocks {
            for inst in &mut block.insts {
                for operand in inst.kind.operands_mut() {
                    *operand = resolve(*operand);
                }
            }
            for operand in block.term.kind.operands_mut() {
                *operand = resolve(*operand);
            }
            for target in block.term.kind.targets_mut().into_iter().chain(block.unwind.as_mut()) {
                for arg in &mut target.args {
                    *arg = resolve(*arg);
                }
            }
        }
    }
    unused.len()
}
