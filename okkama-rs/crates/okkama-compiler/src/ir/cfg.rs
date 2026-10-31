//! Граф управления функции: преемники, доминаторы, места определений.
//!
//! Общий для верификатора и backend: кадры StackMapTable JVM и правило
//! видимости значений OIR (§5.2, §6.2) опираются на одно доминирование.
//!
//! Каждый блок представлен двумя узлами: **начало** (параметры) и **тело**
//! (инструкции и терминатор). Обычные переходы идут из тела к началу цели,
//! ребро unwind — из начала блока к началу цели: отказ возможен на любой
//! инструкции, поэтому в цели unwind видно только то, что определено до
//! начала блока-источника.

use super::{BlockId, Function, Inst, InstKind, ValueId};

/// Место определения значения.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Def {
    /// Параметр функции (блок 0) или параметр блока: определён в его начале.
    BlockStart(BlockId),
    /// Результат инструкции `index` блока.
    Inst(BlockId, usize),
}

impl Def {
    pub fn block(self) -> BlockId {
        match self {
            Def::BlockStart(block) | Def::Inst(block, _) => block,
        }
    }
}

/// Место использования значения.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UsePoint {
    /// Операнд инструкции `index` блока.
    Inst(BlockId, usize),
    /// Операнд терминатора или аргумент его перехода.
    Term(BlockId),
    /// Аргумент цели unwind блока: вычисляется в его начале.
    Unwind(BlockId),
}

/// Определения всех значений функции; ошибка — повтор или отсутствие.
pub fn definitions(function: &Function) -> Result<Vec<Def>, String> {
    let mut defs = vec![None; function.values.len()];
    let mut define = |value: ValueId, def: Def| -> Result<(), String> {
        let slot = defs
            .get_mut(value.0)
            .ok_or_else(|| format!("{value} is not a value of this function"))?;
        if slot.is_some() {
            return Err(format!("{value} is defined more than once"));
        }
        *slot = Some(def);
        Ok(())
    };
    for param in &function.params {
        define(*param, Def::BlockStart(BlockId(0)))?;
    }
    for (index, block) in function.blocks.iter().enumerate() {
        for param in &block.params {
            define(*param, Def::BlockStart(BlockId(index)))?;
        }
        for (position, inst) in block.insts.iter().enumerate() {
            if let Some(result) = inst.result {
                define(result, Def::Inst(BlockId(index), position))?;
            }
        }
    }
    defs.into_iter()
        .enumerate()
        .map(|(index, def)| def.ok_or_else(|| format!("%{index} is declared but never defined")))
        .collect()
}

/// Цели переходов терминатора блока (без unwind).
pub fn successors(function: &Function, block: BlockId) -> Vec<BlockId> {
    function.blocks[block.0]
        .term
        .kind
        .targets()
        .into_iter()
        .map(|target| target.block)
        .collect()
}

/// Все преемники блока, включая цель unwind.
pub fn all_successors(function: &Function, block: BlockId) -> Vec<BlockId> {
    let mut targets = successors(function, block);
    if let Some(unwind) = &function.blocks[block.0].unwind {
        targets.push(unwind.block);
    }
    targets
}

/// Удалить блоки, недостижимые от входа, и перенумеровать остальные.
pub fn remove_unreachable_blocks(function: &mut Function) {
    let mut reachable = vec![false; function.blocks.len()];
    let mut stack = vec![BlockId(0)];
    reachable[0] = true;
    while let Some(block) = stack.pop() {
        for next in all_successors(function, block) {
            if !reachable[next.0] {
                reachable[next.0] = true;
                stack.push(next);
            }
        }
    }
    if reachable.iter().all(|seen| *seen) {
        return;
    }
    let mut renumbered = vec![0; reachable.len()];
    let mut next = 0;
    for (old, seen) in reachable.iter().enumerate() {
        if *seen {
            renumbered[old] = next;
            next += 1;
        }
    }
    let blocks = std::mem::take(&mut function.blocks);
    let mut dropped = Vec::new();
    for (block, seen) in blocks.into_iter().zip(&reachable) {
        if *seen {
            function.blocks.push(block);
        } else {
            dropped.extend(block.params);
            dropped.extend(block.insts.iter().filter_map(|inst| inst.result));
        }
    }
    // Значения удалённых блоков объявлены: они определяются на входе и нигде не читаются.
    let span = function.span;
    function.blocks[0]
        .insts
        .extend(dropped.into_iter().map(|value| Inst { result: Some(value), kind: InstKind::Undef, span }));
    for block in &mut function.blocks {
        for target in block.term.kind.targets_mut().into_iter().chain(block.unwind.as_mut()) {
            target.block = BlockId(renumbered[target.block.0]);
        }
    }
}

/// Убрать `undef`, результат которых никто не читает, и сжать номера
/// значений: остаются только определённые. Такие `undef` оставляют убранные
/// регионы и удалённые блоки; машинный код исполнял бы каждый при каждом
/// вызове.
pub fn drop_unread_undef(function: &mut Function) {
    let mut read = vec![false; function.values.len()];
    for block in &function.blocks {
        for inst in &block.insts {
            for operand in inst.kind.operands() {
                read[operand.0] = true;
            }
        }
        for operand in block.term.kind.operands() {
            read[operand.0] = true;
        }
        for target in block.term.kind.targets().into_iter().chain(block.unwind.as_ref()) {
            for arg in &target.args {
                read[arg.0] = true;
            }
        }
    }
    let mut removed = false;
    for block in &mut function.blocks {
        block.insts.retain(|inst| {
            let unread = matches!(inst.kind, InstKind::Undef) && inst.result.is_none_or(|value| !read[value.0]);
            removed |= unread;
            !unread
        });
    }
    if !removed {
        return;
    }
    let mut defined = vec![false; function.values.len()];
    for value in &function.params {
        defined[value.0] = true;
    }
    for block in &function.blocks {
        for value in block.params.iter().chain(block.insts.iter().filter_map(|inst| inst.result.as_ref())) {
            defined[value.0] = true;
        }
    }
    let mut renumbered = vec![ValueId(usize::MAX); function.values.len()];
    let mut values = Vec::new();
    for (old, ty) in function.values.iter().enumerate() {
        if defined[old] {
            renumbered[old] = ValueId(values.len());
            values.push(ty.clone());
        }
    }
    function.values = values;
    let rename = |value: &mut ValueId| *value = renumbered[value.0];
    function.params.iter_mut().for_each(rename);
    for block in &mut function.blocks {
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
}

fn start(block: BlockId) -> usize {
    block.0 * 2
}

fn body(block: BlockId) -> usize {
    block.0 * 2 + 1
}

/// Дерево доминаторов узлов «начало»/«тело» (Cooper–Harvey–Kennedy).
/// Требует, чтобы все цели существовали и все блоки были достижимы.
#[derive(Clone, Debug)]
pub struct Dominators {
    idom: Vec<usize>,
}

impl Dominators {
    pub fn compute(function: &Function) -> Result<Self, String> {
        let count = function.blocks.len() * 2;
        let node_successors = |node: usize| -> Vec<usize> {
            let block = BlockId(node / 2);
            if node % 2 == 0 {
                let mut next = vec![body(block)];
                if let Some(unwind) = &function.blocks[block.0].unwind {
                    next.push(start(unwind.block));
                }
                next
            } else {
                successors(function, block).into_iter().map(start).collect()
            }
        };
        let mut order = Vec::with_capacity(count);
        let mut visited = vec![false; count];
        // Итеративный DFS с явным стеком: глубина CFG не ограничена стеком потока.
        let mut stack = vec![(0usize, 0usize)];
        visited[0] = true;
        while let Some((node, next)) = stack.pop() {
            let targets = node_successors(node);
            if let Some(target) = targets.get(next) {
                stack.push((node, next + 1));
                if !visited[*target] {
                    visited[*target] = true;
                    stack.push((*target, 0));
                }
            } else {
                order.push(node);
            }
        }
        if let Some(node) = visited.iter().position(|seen| !seen) {
            return Err(format!("block ^b{} is unreachable", node / 2));
        }
        order.reverse(); // reverse postorder
        let mut rpo_index = vec![0; count];
        for (position, node) in order.iter().enumerate() {
            rpo_index[*node] = position;
        }
        let mut predecessors = vec![Vec::new(); count];
        for node in 0..count {
            for target in node_successors(node) {
                predecessors[target].push(node);
            }
        }
        let mut idom: Vec<Option<usize>> = vec![None; count];
        idom[0] = Some(0);
        let intersect = |idom: &[Option<usize>], mut left: usize, mut right: usize| {
            while left != right {
                while rpo_index[left] > rpo_index[right] {
                    left = idom[left].expect("processed node has an idom");
                }
                while rpo_index[right] > rpo_index[left] {
                    right = idom[right].expect("processed node has an idom");
                }
            }
            left
        };
        let mut changed = true;
        while changed {
            changed = false;
            for node in order.iter().skip(1) {
                let mut new_idom: Option<usize> = None;
                for predecessor in &predecessors[*node] {
                    if idom[*predecessor].is_none() {
                        continue;
                    }
                    new_idom = Some(match new_idom {
                        None => *predecessor,
                        Some(current) => intersect(&idom, *predecessor, current),
                    });
                }
                if new_idom.is_some() && idom[*node] != new_idom {
                    idom[*node] = new_idom;
                    changed = true;
                }
            }
        }
        Ok(Self {
            idom: idom
                .into_iter()
                .map(|parent| parent.expect("every reachable node has an idom"))
                .collect(),
        })
    }

    fn dominates_node(&self, dominator: usize, mut node: usize) -> bool {
        loop {
            if node == dominator {
                return true;
            }
            let parent = self.idom[node];
            if parent == node {
                return false;
            }
            node = parent;
        }
    }

    /// Видно ли значение с определением `def` в точке использования.
    pub fn visible(&self, def: Def, at: UsePoint) -> bool {
        let (use_node, position) = match at {
            UsePoint::Inst(block, index) => (body(block), Some((block, index))),
            UsePoint::Term(block) => (body(block), None),
            UsePoint::Unwind(block) => (start(block), None),
        };
        match def {
            Def::BlockStart(block) => self.dominates_node(start(block), use_node),
            Def::Inst(block, index) => match position {
                Some((use_block, use_index)) if use_block == block => index < use_index,
                _ => self.dominates_node(body(block), use_node),
            },
        }
    }
}
