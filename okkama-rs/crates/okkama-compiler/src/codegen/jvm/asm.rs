//! Ассемблер байткода JVM: метки, глубина стека и кадры StackMapTable.
//!
//! Генератор задаёт кадр явно для каждой метки, на которую возможен переход
//! или которая следует за безусловной передачей управления. Ассемблер
//! вычисляет смещения переходов, `max_stack` и кодирует кадры как
//! `full_frame` (JVMS §4.7.4) — так кадр не зависит от предыдущего.

use super::classfile::{ExceptionEntry, u16_to};
use super::{Result, error};

/// Тип ячейки для верификатора (JVMS §4.10.1.2).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum VType {
    Top,
    Integer,
    /// Ссылка на класс по индексу `CONSTANT_Class`.
    Object(u16),
}

impl VType {
    fn encode(self, bytes: &mut Vec<u8>) {
        match self {
            VType::Top => bytes.push(0),
            VType::Integer => bytes.push(1),
            VType::Object(class) => {
                bytes.push(7);
                u16_to(bytes, class);
            }
        }
    }
}

/// Кадр: записи locals по слотам и стек.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Frame {
    pub locals: Vec<VType>,
    pub stack: Vec<VType>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct Label(usize);

pub(super) struct Assembled {
    pub code: Vec<u8>,
    pub max_stack: u16,
    pub stack_map: Option<Vec<u8>>,
    pub exceptions: Vec<ExceptionEntry>,
}

#[derive(Default)]
pub(super) struct Asm {
    code: Vec<u8>,
    depth: i32,
    max_depth: i32,
    labels: Vec<Option<usize>>,
    /// (смещение opcode, позиция 16-битного операнда, цель).
    fixups: Vec<(usize, usize, Label)>,
    frames: Vec<(Label, Frame)>,
    exceptions: Vec<(Label, Label, Label, u16)>,
}

impl Asm {
    pub(super) fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }

    /// Привязать метку к текущему смещению; `depth` — глубина стека в этой точке.
    pub(super) fn place(&mut self, label: Label, depth: i32) {
        debug_assert!(self.labels[label.0].is_none(), "label placed twice");
        self.labels[label.0] = Some(self.code.len());
        self.depth = depth;
    }

    /// Привязать метку и объявить в ней кадр.
    pub(super) fn place_with_frame(&mut self, label: Label, frame: Frame) {
        let depth = frame.stack.len() as i32;
        self.place(label, depth);
        self.frames.push((label, frame));
    }

    pub(super) fn exception(&mut self, start: Label, end: Label, handler: Label, catch_type: u16) {
        self.exceptions.push((start, end, handler, catch_type));
    }

    /// Произвольная инструкция с известным действием на стек.
    pub(super) fn op(&mut self, bytes: &[u8], pops: i32, pushes: i32) {
        self.code.extend_from_slice(bytes);
        self.adjust(pops, pushes);
    }

    /// Инструкция с 16-битным индексом constant pool.
    pub(super) fn op_index(&mut self, opcode: u8, index: u16, pops: i32, pushes: i32) {
        self.code.push(opcode);
        u16_to(&mut self.code, index);
        self.adjust(pops, pushes);
    }

    fn adjust(&mut self, pops: i32, pushes: i32) {
        self.depth -= pops;
        debug_assert!(self.depth >= 0, "operand stack underflow");
        self.depth += pushes;
        self.max_depth = self.max_depth.max(self.depth);
    }

    /// `invokeinterface`: индекс, число слотов аргументов с receiver, ноль.
    pub(super) fn invoke_interface(&mut self, index: u16, count: u8, pops: i32, pushes: i32) {
        self.code.push(0xb9);
        u16_to(&mut self.code, index);
        self.code.extend_from_slice(&[count, 0]);
        self.adjust(pops, pushes);
    }

    /// Переход с 16-битным смещением (goto, if*, if_icmp*).
    pub(super) fn jump(&mut self, opcode: u8, target: Label, pops: i32) {
        let at = self.code.len();
        self.code.push(opcode);
        self.code.extend_from_slice(&[0, 0]);
        self.fixups.push((at, at + 1, target));
        self.adjust(pops, 0);
    }

    /// `ldc`/`ldc_w` для одно-слотовой константы.
    pub(super) fn ldc(&mut self, index: u16) {
        if index <= u16::from(u8::MAX) {
            self.op(&[0x12, index as u8], 0, 1);
        } else {
            self.op_index(0x13, index, 0, 1);
        }
    }

    pub(super) fn load(&mut self, ty: VType, slot: u16) {
        let (opcode, size) = match ty {
            VType::Integer => (0x15, 1),   // iload
            VType::Object(_) => (0x19, 1), // aload
            VType::Top => unreachable!("Top values are never loaded"),
        };
        self.local(opcode, slot);
        self.adjust(0, size);
    }

    pub(super) fn store(&mut self, ty: VType, slot: u16) {
        let (opcode, size) = match ty {
            VType::Integer => (0x36, 1),   // istore
            VType::Object(_) => (0x3a, 1), // astore
            VType::Top => unreachable!("Top values are never stored"),
        };
        self.local(opcode, slot);
        self.adjust(size, 0);
    }

    fn local(&mut self, opcode: u8, slot: u16) {
        if slot <= u16::from(u8::MAX) {
            self.code.extend_from_slice(&[opcode, slot as u8]);
        } else {
            self.code.extend_from_slice(&[0xc4, opcode]); // wide
            u16_to(&mut self.code, slot);
        }
    }

    pub(super) fn finish(mut self) -> Result<Assembled> {
        if self.code.len() > 65535 {
            return Err(error("method bytecode exceeds 65535 bytes"));
        }
        let offset = |labels: &[Option<usize>], label: Label| {
            labels[label.0].expect("every referenced label is placed")
        };
        for (at, operand, target) in std::mem::take(&mut self.fixups) {
            let delta = offset(&self.labels, target) as i64 - at as i64;
            let delta = i16::try_from(delta)
                .map_err(|_| error("method too large for 16-bit branch offsets"))?;
            self.code[operand..operand + 2].copy_from_slice(&delta.to_be_bytes());
        }

        let mut frames: Vec<(usize, Frame)> = self
            .frames
            .iter()
            .map(|(label, frame)| (offset(&self.labels, *label), frame.clone()))
            .collect();
        frames.sort_by_key(|(at, _)| *at);
        frames.dedup();
        if frames.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(error("conflicting stack map frames at one bytecode offset"));
        }
        let stack_map = if frames.is_empty() {
            None
        } else {
            let mut table = Vec::new();
            u16_to(&mut table, frames.len() as u16);
            let mut previous: Option<usize> = None;
            for (at, frame) in &frames {
                let delta = match previous {
                    None => *at,
                    Some(previous) => at - previous - 1,
                };
                previous = Some(*at);
                table.push(255); // full_frame
                u16_to(&mut table, delta as u16);
                let mut locals = frame.locals.clone();
                while locals.last() == Some(&VType::Top) {
                    locals.pop();
                }
                u16_to(&mut table, locals.len() as u16);
                for local in locals {
                    local.encode(&mut table);
                }
                u16_to(&mut table, frame.stack.len() as u16);
                for item in &frame.stack {
                    item.encode(&mut table);
                }
            }
            Some(table)
        };
        let exceptions = self
            .exceptions
            .iter()
            .map(|(start, end, handler, catch_type)| ExceptionEntry {
                start: offset(&self.labels, *start) as u16,
                end: offset(&self.labels, *end) as u16,
                handler: offset(&self.labels, *handler) as u16,
                catch_type: *catch_type,
            })
            .collect();
        Ok(Assembled {
            code: self.code,
            max_stack: u16::try_from(self.max_depth)
                .map_err(|_| error("operand stack exceeds the JVM u16 limit"))?,
            stack_map,
            exceptions,
        })
    }
}
