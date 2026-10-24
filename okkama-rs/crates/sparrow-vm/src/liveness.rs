//! Живость регистров функции по инструкциям (обратный поток данных).
//!
//! Нужна машинному коду, чтобы последнее использование значения было
//! перемещением, а не копией: тогда у объекта остаётся единственная ссылка и
//! successor правит его на месте. Анализ консервативен: живым считается всё,
//! что может быть прочитано на каком-либо пути, включая пути отказа к
//! обработчику.
use okkama_sparrow::{Access, Function, Instruction as I, Register, Target};

/// Множества живых регистров на входе и выходе каждой инструкции.
#[derive(Debug)]
pub(crate) struct Liveness {
    words: usize,
    live_in: Vec<u64>,
    live_out: Vec<u64>,
}

/// Ребро инструкции: цель, записи и регистр значения ребра.
pub(crate) struct Edge<'a> {
    pub(crate) target: u32,
    pub(crate) moves: &'a [(Register, Register)],
    pub(crate) payload: Option<Register>,
}

/// Рёбра инструкции `pc`, включая ребро отказа к обработчику диапазона.
pub(crate) fn edges<'a>(function: &'a Function, pc: usize) -> Vec<Edge<'a>> {
    let instruction = &function.code[pc];
    let edge = |target: &'a Target, payload: Option<Register>| Edge { target: target.pc, moves: &target.moves, payload };
    let mut out = match instruction {
        I::Jump { target } => vec![edge(target, None)],
        I::Branch { then_target, else_target, .. } => vec![edge(then_target, None), edge(else_target, None)],
        I::Catch { value, handler, next, .. } => vec![edge(handler, Some(value.0)), edge(next, None)],
        I::ArrayLookup { value, found, missing, .. } | I::MapLookup { value, found, missing, .. } => {
            vec![edge(found, Some(value.0)), edge(missing, None)]
        }
        instruction if instruction.is_terminator() => Vec::new(),
        _ => vec![Edge { target: pc as u32 + 1, moves: &[], payload: None }],
    };
    if let Some(handler) = function.handler_at(pc as u32) {
        out.push(edge(&handler.target, Some(handler.failure)));
    }
    out
}

/// Регистр значения ребра у ветвящейся инструкции (пишется только на ребре).
fn payload(instruction: &I) -> Option<Register> {
    match instruction {
        I::Catch { value, .. } | I::ArrayLookup { value, .. } | I::MapLookup { value, .. } => Some(value.0),
        _ => None,
    }
}

impl Liveness {
    pub(crate) fn new(function: &Function) -> Self {
        let count = function.code.len();
        let words = function.registers.len().div_ceil(64).max(1);
        let mut uses = vec![0u64; count * words];
        let mut defs = vec![0u64; count * words];
        let set = |bits: &mut [u64], pc: usize, register: Register| {
            let index = register.0 as usize;
            bits[pc * words + index / 64] |= 1 << (index % 64);
        };
        for (pc, instruction) in function.code.iter().enumerate() {
            let skip = payload(instruction);
            instruction.visit(&mut |access| match access {
                Access::Use(register) => set(&mut uses, pc, register),
                Access::Def(register) if Some(register) != skip => set(&mut defs, pc, register),
                _ => {}
            });
        }
        let mut live = Self { words, live_in: vec![0; count * words], live_out: vec![0; count * words] };
        let all_edges: Vec<Vec<Edge<'_>>> = (0..count).map(|pc| edges(function, pc)).collect();
        let mut changed = true;
        while changed {
            changed = false;
            for pc in (0..count).rev() {
                let mut out = vec![0u64; words];
                for edge in &all_edges[pc] {
                    live.edge_into(edge, &mut out);
                }
                let mut input = out.clone();
                for word in 0..words {
                    input[word] = (input[word] & !defs[pc * words + word]) | uses[pc * words + word];
                }
                let range = pc * words..(pc + 1) * words;
                if live.live_out[range.clone()] != out[..] || live.live_in[range.clone()] != input[..] {
                    live.live_out[range.clone()].copy_from_slice(&out);
                    live.live_in[range].copy_from_slice(&input);
                    changed = true;
                }
            }
        }
        live
    }

    /// Живое после ребра: живое на входе цели без записанных ребром, плюс источники.
    fn edge_into(&self, edge: &Edge<'_>, out: &mut [u64]) {
        let Some(target) = (edge.target as usize).checked_mul(self.words) else { return };
        if target + self.words > self.live_in.len() {
            return;
        }
        let mut needed = self.live_in[target..target + self.words].to_vec();
        let clear = |bits: &mut [u64], register: Register| {
            let index = register.0 as usize;
            if let Some(word) = bits.get_mut(index / 64) {
                *word &= !(1 << (index % 64));
            }
        };
        for (dst, _) in edge.moves {
            clear(&mut needed, *dst);
        }
        if let Some(payload) = edge.payload {
            clear(&mut needed, payload);
        }
        for (word, value) in needed.iter().enumerate() {
            out[word] |= value;
        }
        for (_, source) in edge.moves {
            let index = source.0 as usize;
            if let Some(word) = out.get_mut(index / 64) {
                *word |= 1 << (index % 64);
            }
        }
    }

    fn bit(bits: &[u64], words: usize, pc: usize, register: Register) -> bool {
        let index = register.0 as usize;
        bits.get(pc * words + index / 64).is_some_and(|word| word & (1 << (index % 64)) != 0)
    }

    /// Регистр может быть прочитан после инструкции `pc`.
    pub(crate) fn live_after(&self, pc: usize, register: Register) -> bool {
        Self::bit(&self.live_out, self.words, pc, register)
    }

    /// Источник записи ребра можно переместить: после ребра он не нужен
    /// (кроме как в своём назначении) и встречается среди источников один раз.
    pub(crate) fn movable(&self, edge: &Edge<'_>, source: Register) -> bool {
        if edge.moves.iter().filter(|(_, from)| *from == source).count() != 1 {
            return false;
        }
        let overwritten = edge.moves.iter().any(|(dst, _)| *dst == source) || edge.payload == Some(source);
        overwritten || !Self::bit(&self.live_in, self.words, edge.target as usize, source)
    }
}
