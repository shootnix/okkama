//! Построение тела функции OIR.
//!
//! Lowering добавляет блоки и инструкции в любом порядке и может оставить
//! «мёртвые» блоки: код после `return`, выход цикла без `last`, заготовку
//! слияния, куда не пришёл ни один переход. [`FnBuilder::finish`] удаляет
//! недостижимые блоки и перенумеровывает значения и блоки по порядку
//! определения, поэтому листинг детерминирован и не содержит дыр.

use crate::ir::{self, BlockId, Inst, InstKind, Target, TermKind, Terminator, Type, ValueId};
use crate::source::Span;

struct PendingBlock {
    params: Vec<ValueId>,
    unwind: Option<Target>,
    insts: Vec<Inst>,
    term: Option<Terminator>,
}

/// Готовое тело: типы значений, блоки и новые идентификаторы параметров.
pub(super) type FinishedBody = (Vec<Type>, Vec<ir::Block>, Vec<ValueId>);

pub(super) struct FnBuilder {
    values: Vec<Type>,
    blocks: Vec<PendingBlock>,
    current: Option<BlockId>,
    /// Цель unwind для блоков, начинающихся сейчас (внутри `try`).
    /// Блок получает её вместе с первой инструкцией: аргументы — значения на его входе.
    pub(super) unwind: Option<Target>,
}

impl FnBuilder {
    /// Новый построитель с открытым входным блоком 0.
    pub(super) fn new() -> Self {
        let mut builder = Self {
            values: Vec::new(),
            blocks: Vec::new(),
            current: None,
            unwind: None,
        };
        let entry = builder.new_block();
        builder.current = Some(entry);
        builder
    }

    pub(super) fn new_value(&mut self, ty: Type) -> ValueId {
        self.values.push(ty);
        ValueId(self.values.len() - 1)
    }

    pub(super) fn value_type(&self, value: ValueId) -> &Type {
        &self.values[value.0]
    }

    pub(super) fn new_block(&mut self) -> BlockId {
        self.blocks.push(PendingBlock {
            params: Vec::new(),
            unwind: None,
            insts: Vec::new(),
            term: None,
        });
        BlockId(self.blocks.len() - 1)
    }

    /// Параметры блока по порядку.
    pub(super) fn block_params(&self, block: BlockId) -> &[ValueId] {
        &self.blocks[block.0].params
    }

    pub(super) fn add_param(&mut self, block: BlockId, ty: Type) -> ValueId {
        let value = self.new_value(ty);
        self.blocks[block.0].params.push(value);
        value
    }

    /// Есть ли блок, в который сейчас пишется код (поток управления не ушёл).
    pub(super) fn is_open(&self) -> bool {
        self.current.is_some()
    }

    pub(super) fn current(&self) -> Option<BlockId> {
        self.current
    }

    pub(super) fn switch_to(&mut self, block: BlockId) {
        self.current = Some(block);
    }

    /// Закрыть текущую позицию: следующий код недостижим.
    pub(super) fn close(&mut self) {
        self.current = None;
    }

    /// Текущий блок; после ухода управления создаётся отдельный мёртвый блок,
    /// чтобы недостижимый код всё равно прошёл проверку типов.
    fn open_block(&mut self) -> BlockId {
        match self.current {
            Some(block) => block,
            None => {
                let block = self.new_block();
                self.current = Some(block);
                block
            }
        }
    }

    /// Есть ли в текущем блоке инструкции (тогда смена unwind требует нового блока).
    pub(super) fn current_has_code(&self) -> bool {
        self.current
            .is_some_and(|block| !self.blocks[block.0].insts.is_empty())
    }

    fn assign_unwind(&mut self, block: BlockId) {
        let pending = &mut self.blocks[block.0];
        if pending.insts.is_empty() && pending.unwind.is_none() {
            pending.unwind = self.unwind.clone();
        }
    }

    pub(super) fn emit(&mut self, kind: InstKind, ty: Type, span: Span) -> ValueId {
        let block = self.open_block();
        self.assign_unwind(block);
        let result = self.new_value(ty);
        self.blocks[block.0].insts.push(Inst {
            result: Some(result),
            kind,
            span,
        });
        result
    }

    /// Инструкция без результата (вызов void-функции).
    pub(super) fn emit_void(&mut self, kind: InstKind, span: Span) {
        let block = self.open_block();
        self.assign_unwind(block);
        self.blocks[block.0].insts.push(Inst {
            result: None,
            kind,
            span,
        });
    }

    /// Завершить текущий блок; после этого позиция закрыта.
    pub(super) fn terminate(&mut self, kind: TermKind, span: Span) {
        let block = self.open_block();
        // Отказывающий терминатор пустого блока тоже уходит в unwind.
        if kind.fails() {
            self.assign_unwind(block);
        }
        self.terminate_block(block, kind, span);
        self.current = None;
    }

    /// Завершить блок, который был оставлен открытым ранее (ветви слияния).
    pub(super) fn terminate_block(&mut self, block: BlockId, kind: TermKind, span: Span) {
        let pending = &mut self.blocks[block.0];
        debug_assert!(
            pending.term.is_none(),
            "block ^b{} terminated twice",
            block.0
        );
        pending.term = Some(Terminator { kind, span });
    }

    /// Удаляет недостижимые блоки и перенумеровывает значения и блоки.
    ///
    /// Возвращает типы значений, блоки и новые идентификаторы параметров.
    /// Удалить определения значений `values`, которые нигде не читаются
    /// (служебные константы, выпущенные заранее на случай надобности).
    pub(super) fn drop_unused(&mut self, values: &[ValueId]) {
        let mut used = std::collections::HashSet::new();
        for block in &self.blocks {
            for inst in &block.insts {
                used.extend(inst.kind.operands());
            }
            if let Some(term) = &block.term {
                used.extend(term.kind.operands());
                for target in term.kind.targets() {
                    used.extend(target.args.iter().copied());
                }
            }
            if let Some(unwind) = &block.unwind {
                used.extend(unwind.args.iter().copied());
            }
        }
        for block in &mut self.blocks {
            block.insts.retain(|inst| {
                inst.result
                    .is_none_or(|result| used.contains(&result) || !values.contains(&result))
            });
        }
    }

    pub(super) fn finish(self, params: &[ValueId]) -> Result<FinishedBody, String> {
        // Достижимые блоки в reverse postorder: вход первым, каждый блок после
        // своих предшественников по прямым рёбрам, ветви в порядке целей.
        let count = self.blocks.len();
        let mut visited = vec![false; count];
        let mut postorder = Vec::new();
        let mut stack = vec![(BlockId(0), 0usize)];
        visited[0] = true;
        while let Some((block, next)) = stack.pop() {
            let Some(term) = &self.blocks[block.0].term else {
                return Err(format!("reachable block ^b{} has no terminator", block.0));
            };
            // Цели обходятся с конца: в обращённом postorder первая цель
            // (ветка `then`) окажется раньше второй.
            let unwind = self.blocks[block.0].unwind.as_ref();
            let targets: Vec<_> = unwind
                .into_iter()
                .chain(term.kind.targets().into_iter().rev())
                .collect();
            if let Some(target) = targets.get(next) {
                stack.push((block, next + 1));
                if !visited[target.block.0] {
                    visited[target.block.0] = true;
                    stack.push((target.block, 0));
                }
            } else {
                postorder.push(block);
            }
        }
        let mut order = postorder;
        order.reverse();
        let mut block_map = vec![None; count];
        for (new, old) in order.iter().enumerate() {
            block_map[old.0] = Some(BlockId(new));
        }

        const MISSING: ValueId = ValueId(usize::MAX);
        let mut value_map = vec![MISSING; self.values.len()];
        let mut types = Vec::new();
        let mut define = |value: ValueId, types: &mut Vec<Type>| {
            value_map[value.0] = ValueId(types.len());
            types.push(self.values[value.0].clone());
        };
        for param in params {
            define(*param, &mut types);
        }
        for old in &order {
            let block = &self.blocks[old.0];
            for param in &block.params {
                define(*param, &mut types);
            }
            for inst in &block.insts {
                if let Some(result) = inst.result {
                    define(result, &mut types);
                }
            }
        }
        let map_value = |value: &mut ValueId| *value = value_map[value.0];

        let mut blocks = Vec::with_capacity(order.len());
        let mut pending: Vec<Option<PendingBlock>> = self.blocks.into_iter().map(Some).collect();
        for old in &order {
            let mut block = pending[old.0].take().expect("each block is kept once");
            let mut term = block.term.take().expect("reachable blocks are terminated");
            for param in &mut block.params {
                map_value(param);
            }
            for inst in &mut block.insts {
                if let Some(result) = &mut inst.result {
                    map_value(result);
                }
                for operand in inst.kind.operands_mut() {
                    map_value(operand);
                }
            }
            for operand in term.kind.operands_mut() {
                map_value(operand);
            }
            let mut unwind = block.unwind.take();
            for target in term.kind.targets_mut().into_iter().chain(unwind.as_mut()) {
                target.block = block_map[target.block.0].expect("targets of kept blocks are kept");
                for arg in &mut target.args {
                    map_value(arg);
                }
            }
            blocks.push(ir::Block {
                params: block.params,
                unwind,
                insts: block.insts,
                term,
            });
        }
        let params = params.iter().map(|param| value_map[param.0]).collect();
        Ok((types, blocks, params))
    }
}
