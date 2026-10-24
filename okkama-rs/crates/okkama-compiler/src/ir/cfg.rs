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

use super::{BlockId, Function, ValueId};

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
