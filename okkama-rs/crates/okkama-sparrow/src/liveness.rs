//! Registers live before each instruction of a function.
//!
//! A machine-code compiler keeps a liveness set per block, and the Sparrow
//! core gives every fallible instruction several blocks, so compiling a
//! function costs about Σ live registers × blocks of each instruction (#73):
//! k values live across k fallible instructions cost k² before the function
//! runs. States are kept at block leaders only, like the initialization
//! analysis.

use crate::{Function, Instruction, Register, verify};

/// Memory the analysis may use for its leader states; a function above it
/// has no answer.
pub const MAX_LIVENESS_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
struct Set(Vec<u64>);

impl Set {
    fn empty(words: usize) -> Self {
        Set(vec![0; words])
    }
    fn insert(&mut self, register: Register) {
        if let Some(word) = self.0.get_mut(register.0 as usize / 64) {
            *word |= 1 << (register.0 % 64);
        }
    }
    fn remove(&mut self, register: Register) {
        if let Some(word) = self.0.get_mut(register.0 as usize / 64) {
            *word &= !(1 << (register.0 % 64));
        }
    }
    fn union(&mut self, other: &Set) {
        for (word, other) in self.0.iter_mut().zip(&other.0) {
            *word |= other;
        }
    }
    fn count(&self) -> u64 {
        self.0.iter().map(|word| u64::from(word.count_ones())).sum()
    }
}

/// Σ over instructions of the registers live before each one (`u64::MAX`
/// above [`MAX_LIVENESS_BYTES`]).
pub fn live_weight(function: &Function) -> u64 {
    live_counts(function).map_or(u64::MAX, |counts| counts.iter().map(|&count| u64::from(count)).sum())
}

/// The number of registers live before each instruction, or `None` when the
/// leader states would exceed [`MAX_LIVENESS_BYTES`].
pub fn live_counts(function: &Function) -> Option<Vec<u32>> {
    let count = function.code.len();
    if count == 0 {
        return Some(Vec::new());
    }
    let words = function.registers.len().div_ceil(64);
    // Leaders: the entry, every edge target that is not the next
    // instruction of a fall-through, handler targets, and whatever follows a
    // terminator.
    let mut leader = vec![false; count];
    leader[0] = true;
    for (pc, instruction) in function.code.iter().enumerate() {
        for (target, _, _) in verify::edges(pc, instruction) {
            if let Some(slot) = leader.get_mut(target as usize)
                && (target as usize != pc + 1 || instruction.is_terminator()) {
                    *slot = true;
                }
        }
        if instruction.is_terminator() && pc + 1 < count {
            leader[pc + 1] = true;
        }
    }
    for handler in &function.handlers {
        if let Some(slot) = leader.get_mut(handler.target.pc as usize) {
            *slot = true;
        }
    }
    let starts: Vec<usize> = (0..count).filter(|&pc| leader[pc]).collect();
    if starts.len().saturating_mul(words).saturating_mul(8) > MAX_LIVENESS_BYTES {
        return None;
    }
    let mut block_of = vec![0usize; count];
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(count);
        block_of[start..end].fill(index);
    }
    let mut live_in = vec![Set::empty(words); starts.len()];

    // Registers live on an edge into `target`: its live-in, minus what the
    // edge writes (moves, the edge value), plus the move sources.
    let edge = |live_in: &[Set], target: u32, moves: &[(Register, Register)], value: Option<Register>| {
        let mut set = live_in[block_of[target as usize]].clone();
        for (dst, _) in moves {
            set.remove(*dst);
        }
        if let Some(value) = value {
            set.remove(value);
        }
        for (_, source) in moves {
            set.insert(*source);
        }
        set
    };
    // A failure inside a protected instruction enters its handler.
    let handler_live = |live_in: &[Set], pc: usize| {
        function.handler_at(pc as u32).map(|handler| {
            edge(live_in, handler.target.pc, &handler.target.moves, Some(handler.failure))
        })
    };
    // Walk one block backwards from its exit; `visit` sees the set before
    // each instruction.
    let walk = |live_in: &[Set], index: usize, visit: &mut dyn FnMut(usize, &Set)| -> Set {
        let start = starts[index];
        let end = starts.get(index + 1).copied().unwrap_or(count);
        let mut live = Set::empty(words);
        for pc in (start..end).rev() {
            let instruction = &function.code[pc];
            if pc + 1 == end {
                for (target, moves, value) in verify::edges(pc, instruction) {
                    if (target as usize) < count {
                        live.union(&edge(live_in, target, moves, value));
                    }
                }
            }
            transfer(instruction, &mut live);
            if let Some(handled) = handler_live(live_in, pc) {
                live.union(&handled);
            }
            visit(pc, &live);
        }
        live
    };
    loop {
        let mut changed = false;
        for index in (0..starts.len()).rev() {
            let live = walk(&live_in, index, &mut |_, _| {});
            if live.0 != live_in[index].0 {
                live_in[index] = live;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut counts = vec![0u32; count];
    for index in 0..starts.len() {
        walk(&live_in, index, &mut |pc, live| counts[pc] = live.count() as u32);
    }
    Some(counts)
}

/// Live before `instruction` from live after it: definitions die, uses live.
fn transfer(instruction: &Instruction, live: &mut Set) {
    let mut uses = Vec::new();
    instruction.visit(&mut |access| match access {
        crate::Access::Def(register) => live.remove(register),
        crate::Access::Use(register) => uses.push(register),
        _ => {}
    });
    for register in uses {
        live.insert(register);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weight(source: &str) -> u64 {
        let program = crate::assemble(source).unwrap();
        live_weight(&program.functions[0])
    }

    #[test]
    fn values_live_across_a_chain_cost_quadratically() {
        // k values defined one after another and all read at the end.
        let chain = |k: usize| {
            let mut text = String::from("sparrow 10\nmodule \"m\" \"v1.0.0\"\ntype t0 int\nfunction f0 \"main\" {\n registers [");
            text.push_str(&vec!["t0"; k + 1].join(", "));
            text.push_str("]\n params []\n result t0\n");
            let mut pc = 0;
            for i in 0..k {
                text.push_str(&format!(" {pc}: const r{i}, int {i}\n"));
                pc += 1;
            }
            text.push_str(&format!(" {pc}: const r{k}, int 0\n"));
            pc += 1;
            for i in 0..k {
                text.push_str(&format!(" {pc}: int.add r{k}, r{k}, r{i}\n"));
                pc += 1;
            }
            text.push_str(&format!(" {pc}: ret r{k}\n}}\nentry f0\n"));
            weight(&text)
        };
        let (small, large) = (chain(50), chain(100));
        assert!(large > 3 * small && large < 5 * small, "{small} {large}");
    }

    #[test]
    fn loops_and_handlers_keep_values_live() {
        let source = "sparrow 10\nmodule \"m\" \"v1.0.0\"\ntype t0 int\ntype t1 bool\ntype t2 failure\n\
            function f0 \"main\" {\n registers [t0, t0, t1, t2]\n params []\n result t0\n\
            \x20handler 2 4 @5 r3\n\
            \x200: const r0, int 1\n 1: const r1, int 0\n 2: int.add r1, r1, r0\n 3: jmp @2\n 4: ret r1\n 5: ret r0\n}\nentry f0\n";
        // r0 is live in the loop and in the handler; r1 in the loop.
        assert!(weight(source) >= 5, "{}", weight(source));
    }
}
