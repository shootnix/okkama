//! Registers live before each instruction of a function.
//!
//! A machine-code compiler keeps a liveness set per block, and the Sparrow
//! core gives every fallible instruction several blocks, so compiling a
//! function costs about Σ live registers × blocks of each instruction (#73):
//! k values live across k fallible instructions cost k² before the function
//! runs. States are kept at block leaders only, like the initialization
//! analysis.

use crate::{Function, Instruction, Register, verify};

/// Maximum bitset allocation for leader states or instruction sets; a
/// function above it has no answer.
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
    let mut counts = vec![0u32; function.code.len()];
    analyze(function, &mut |pc, live| counts[pc] = live.count() as u32)?;
    Some(counts)
}

/// Owned instruction sets in one allocation (#456). Indexing borrows the
/// words before an instruction: register `r` is bit `r % 64` of word `r / 64`.
#[derive(Debug)]
pub struct LiveSets {
    bits: Vec<u64>,
    words: usize,
    instructions: usize,
}

impl std::ops::Index<usize> for LiveSets {
    type Output = [u64];

    fn index(&self, pc: usize) -> &Self::Output {
        assert!(pc < self.instructions, "liveness instruction out of bounds");
        &self.bits[pc * self.words..(pc + 1) * self.words]
    }
}

fn storage_words(instructions: usize, words: usize) -> Option<usize> {
    let total = instructions.checked_mul(words)?;
    (total.checked_mul(size_of::<u64>())? <= MAX_LIVENESS_BYTES).then_some(total)
}

/// The registers live before each instruction, or `None` when the slab or
/// leader states exceed [`MAX_LIVENESS_BYTES`]. A machine-code compiler
/// roots a call inside an instruction with them (#354).
pub fn live_sets(function: &Function) -> Option<LiveSets> {
    let words = function.registers.len().div_ceil(64);
    let instructions = function.code.len();
    // The slab has no per-instruction headers or index allocations (#456).
    let mut sets = LiveSets { bits: vec![0; storage_words(instructions, words)?], words, instructions };
    analyze(function, &mut |pc, live| sets.bits[pc * words..(pc + 1) * words].copy_from_slice(&live.0))?;
    Some(sets)
}

/// Every register instruction `pc` may write: its definitions, the
/// destinations and values of its edges, its handler's failure and moves
/// (#354).
pub fn written(function: &Function, pc: usize) -> Vec<Register> {
    let instruction = &function.code[pc];
    let mut out = Vec::new();
    instruction.visit(&mut |access| {
        if let crate::Access::Def(register) = access {
            out.push(register);
        }
    });
    for (_, moves, value) in verify::edges(pc, instruction) {
        out.extend(moves.iter().map(|(dst, _)| *dst));
        out.extend(value);
    }
    if let Some(handler) = function.handler_at(pc as u32) {
        out.extend(handler.target.moves.iter().map(|(dst, _)| *dst));
        out.push(handler.failure);
    }
    out.sort_by_key(|register| register.0);
    out.dedup_by_key(|register| register.0);
    out
}

/// The analysis: `visit` sees the set before each instruction once the
/// leader states are settled.
fn analyze(function: &Function, visit: &mut dyn FnMut(usize, &Set)) -> Option<()> {
    let count = function.code.len();
    if count == 0 {
        return Some(());
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
    storage_words(starts.len(), words)?;
    let mut block_of = vec![0usize; count];
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(count);
        block_of[start..end].fill(index);
    }
    let mut live_in = vec![Set::empty(words); starts.len()];

    // Cache exits and all handler dependencies before iteration (#455).
    let mut exits = Vec::with_capacity(starts.len());
    let mut predecessors = vec![Vec::new(); starts.len()];
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(count);
        let edges = verify::edges(end - 1, &function.code[end - 1]);
        let mut depend_on = |target: u32| {
            let incoming = &mut predecessors[block_of[target as usize]];
            if incoming.last() != Some(&index) {
                incoming.push(index);
            }
        };
        for &(target, _, _) in &edges {
            if (target as usize) < count {
                depend_on(target);
            }
        }
        for pc in start..end {
            if let Some(handler) = function.handler_at(pc as u32) {
                depend_on(handler.target.pc);
            }
        }
        exits.push(edges);
    }

    // Registers live on an edge into `target`: its live-in, minus what the
    // edge writes (moves, the edge value), plus the move sources.
    let edge = |live_in: &[Set], target: u32, moves: &[(Register, Register)], value: Option<Register>, set: &mut Set| {
        set.0.copy_from_slice(&live_in[block_of[target as usize]].0);
        for (dst, _) in moves {
            set.remove(*dst);
        }
        if let Some(value) = value {
            set.remove(value);
        }
        for (_, source) in moves {
            set.insert(*source);
        }
    };
    // Walk one block backwards from its exit; `visit` sees the set before
    // each instruction.
    let walk = |live_in: &[Set], index: usize, live: &mut Set, edge_live: &mut Set, visit: &mut dyn FnMut(usize, &Set)| {
        let start = starts[index];
        let end = starts.get(index + 1).copied().unwrap_or(count);
        live.0.fill(0);
        for pc in (start..end).rev() {
            let instruction = &function.code[pc];
            // Edge definitions belong to terminators, hence to block exits.
            let edges = if pc + 1 == end { exits[index].as_slice() } else { &[] };
            for &(target, moves, value) in edges {
                if (target as usize) < count {
                    edge(live_in, target, moves, value, edge_live);
                    live.union(edge_live);
                }
            }
            transfer(instruction, edges, live);
            // Failure observes the values before the instruction's writes.
            if let Some(handler) = function.handler_at(pc as u32) {
                edge(live_in, handler.target.pc, &handler.target.moves, Some(handler.failure), edge_live);
                live.union(edge_live);
            }
            visit(pc, live);
        }
    };

    // Each block is pending at most once; scratch survives every visit (#455).
    let mut pending: Vec<usize> = (0..starts.len()).collect();
    let mut queued = vec![true; starts.len()];
    let mut live = Set::empty(words);
    let mut edge_live = Set::empty(words);
    while let Some(index) = pending.pop() {
        queued[index] = false;
        walk(&live_in, index, &mut live, &mut edge_live, &mut |_, _| {});
        if live.0 != live_in[index].0 {
            live_in[index].0.copy_from_slice(&live.0);
            for &predecessor in &predecessors[index] {
                if !queued[predecessor] {
                    queued[predecessor] = true;
                    pending.push(predecessor);
                }
            }
        }
    }
    for index in 0..starts.len() {
        walk(&live_in, index, &mut live, &mut edge_live, visit);
    }
    Some(())
}

/// Live before `instruction` from live after it: definitions die, uses live.
/// A value an edge writes (the found item of `map.lookup`) dies on that edge
/// only, in `edge`: on the other edge the register keeps what it held, so
/// it is live before the instruction there (#354).
fn transfer(instruction: &Instruction, edges: &[verify::Edge<'_>], live: &mut Set) {
    instruction.visit(&mut |access| {
        if let crate::Access::Def(register) = access
            && !edges.iter().any(|&(_, _, value)| value == Some(register))
        {
            live.remove(register);
        }
    });
    // Insert uses last so a read and write keeps the old value live (#455).
    instruction.visit(&mut |access| {
        if let crate::Access::Use(register) = access {
            live.insert(register);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Baseline from 96b2ab228f08ea3f1ad796812e446254fc17084c (#455, #456).
    fn oracle_live_counts(function: &Function) -> Option<Vec<u32>> {
        let mut counts = vec![0u32; function.code.len()];
        oracle_analyze(function, &mut |pc, live| counts[pc] = live.count() as u32)?;
        Some(counts)
    }

    fn oracle_live_sets(function: &Function) -> Option<Vec<Vec<u64>>> {
        let words = function.registers.len().div_ceil(64);
        if function.code.len().saturating_mul(words).saturating_mul(8) > MAX_LIVENESS_BYTES {
            return None;
        }
        let mut sets = vec![Vec::new(); function.code.len()];
        oracle_analyze(function, &mut |pc, live| sets[pc] = live.0.clone())?;
        Some(sets)
    }

    fn oracle_analyze(function: &Function, visit: &mut dyn FnMut(usize, &Set)) -> Option<()> {
        let count = function.code.len();
        if count == 0 {
            return Some(());
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
                oracle_transfer(pc, instruction, &mut live);
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
        for index in 0..starts.len() {
            walk(&live_in, index, visit);
        }
        Some(())
    }

    /// Live before `instruction` from live after it: definitions die, uses live.
    /// A value an edge writes (the found item of `map.lookup`) dies on that edge
    /// only, in `edge`: on the other edge the register keeps what it held, so
    /// it is live before the instruction there (#354).
    fn oracle_transfer(pc: usize, instruction: &Instruction, live: &mut Set) {
        let on_edges: Vec<Register> = verify::edges(pc, instruction).into_iter().filter_map(|(_, _, value)| value).collect();
        let mut uses = Vec::new();
        instruction.visit(&mut |access| match access {
            crate::Access::Def(register) if !on_edges.iter().any(|edge| edge.0 == register.0) => live.remove(register),
            crate::Access::Use(register) => uses.push(register),
            _ => {}
        });
        for register in uses {
            live.insert(register);
        }
    }

    fn function(registers: &[&str], handlers: &str, code: &[String]) -> Function {
        let mut source = String::from("sparrow 15\nmodule \"m\" \"v1.0.0\"\ntype t0 int\ntype t1 bool\ntype t2 failure\ntype t3 array t0\nfunction f0 \"main\" {\n registers [");
        source.push_str(&registers.join(", "));
        source.push_str("]\n params [");
        source.push_str(&(0..registers.len()).map(|r| format!("r{r}")).collect::<Vec<_>>().join(", "));
        source.push_str("]\n result t0\n");
        source.push_str(handlers);
        for (pc, instruction) in code.iter().enumerate() {
            source.push_str(&format!(" {pc}: {instruction}\n"));
        }
        source.push_str("}\n");
        crate::assemble(&source).unwrap().functions.remove(0)
    }

    fn sets(function: &Function) -> Option<Vec<Vec<u64>>> {
        live_sets(function).map(|sets| (0..function.code.len()).map(|pc| sets[pc].to_vec()).collect())
    }

    fn assert_oracle(function: &Function) {
        assert_eq!(sets(function), oracle_live_sets(function));
        let expected = oracle_live_counts(function);
        assert_eq!(live_counts(function), expected);
        assert_eq!(live_weight(function), expected.map_or(u64::MAX, |counts| counts.iter().map(|&n| u64::from(n)).sum()));
    }

    #[test]
    fn generated_functions_match_oracle() {
        for seed in 0..16u64 {
            for registers in [3, 63, 64, 65, 127, 128, 129, 193] {
                for shape in 0..6 {
                    let mut state = seed;
                    let mut next = |limit: usize| {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        ((state >> 32) as usize) % limit
                    };
                    let mut types = vec!["t0"; registers];
                    types.extend(["t1", "t2"]);
                    let condition = registers;
                    let failure = registers + 1;
                    let mut code: Vec<String> = (0..registers).map(|r| format!("const r{r}, int {}", next(100))).collect();
                    code.push(format!("const r{condition}, bool true"));
                    let start = code.len();
                    for _ in 0..12 {
                        let dst = next(registers);
                        code.push(format!("int.add r{dst}, r{dst}, r{}", next(registers)));
                    }
                    let mut target = |pc: usize| {
                        let a = next(registers);
                        let b = (a + 1 + next(registers - 1)) % registers;
                        format!("@{pc}(r{a} <- r{b}, r{b} <- r{a})")
                    };
                    let mut handlers = String::new();
                    match shape {
                        0 => {}
                        1 => {
                            code[start] = format!("br r{condition}, {}, {}", target(start + 1), target(start + 5));
                            code[start + 4] = format!("jmp {}", target(start + 9));
                            code[start + 8] = format!("jmp {}", target(start + 9));
                        }
                        2 => {
                            code[start + 8] = format!("br r{condition}, {}, {}", target(start), target(start + 9));
                        }
                        3 => {
                            code[start + 4] = format!("br r{condition}, {}, {}", target(start + 2), target(start + 5));
                            code[start + 8] = format!("br r{condition}, {}, {}", target(start), target(start + 9));
                        }
                        4 => {
                            handlers = format!(" handler {} {} {} r{failure}\n", start + 1, start + 8, target(start + 9));
                            code[start + 8] = format!("jmp {}", target(start + 12));
                            code[start + 11] = format!("br r{condition}, {}, {}", target(start), target(start + 12));
                        }
                        5 => {
                            handlers = format!(" handler {start} {} {} r{failure}\n handler {} {} {} r{failure}\n",
                                start + 5, target(start + 7), start + 7, start + 10, target(start));
                            code[start + 4] = format!("br r{condition}, {}, {}", target(start), target(start + 12));
                            // This unreachable block must still be analyzed.
                            code[start + 6] = format!("jmp {}", target(start + 6));
                            code[start + 10] = format!("br r{condition}, {}, {}", target(start + 7), target(start + 12));
                        }
                        _ => unreachable!(),
                    }
                    for r in 1..registers {
                        code.push(format!("int.add r0, r0, r{r}"));
                    }
                    code.push("ret r0".into());
                    assert_oracle(&function(&types, &handlers, &code));
                    // Also vary kills so loop inputs need not contain every register.
                    for instruction in &mut code[start..start + 12] {
                        if instruction.starts_with("int.add") {
                            let dst = next(registers);
                            *instruction = match next(3) {
                                0 => format!("const r{dst}, int 0"),
                                1 => format!("move r{dst}, r{}", next(registers)),
                                _ => format!("int.add r{dst}, r{}, r{}", next(registers), next(registers)),
                            };
                        }
                    }
                    assert_oracle(&function(&types, &handlers, &code));
                }
            }
        }
    }

    #[test]
    fn read_write_and_parallel_moves_keep_old_values() {
        let function = function(&["t0", "t0"], "", &[
            "int.add r0, r0, r1".into(),
            "jmp @2(r0 <- r1, r1 <- r0)".into(),
            "ret r0".into(),
        ]);
        assert_oracle(&function);
        assert_eq!(sets(&function).unwrap(), vec![vec![3], vec![3], vec![1]]);
        assert_eq!(written(&function, 1), vec![Register(0), Register(1)]);
    }

    #[test]
    fn edge_value_dies_only_on_the_found_edge() {
        let function = function(&["t3", "t0", "t0"], "", &[
            "arr.lookup r0, r1, r2, @1, @2".into(),
            "ret r2".into(),
            "ret r2".into(),
        ]);
        assert_oracle(&function);
        assert_eq!(sets(&function).unwrap(), vec![vec![7], vec![4], vec![4]]);
        assert_eq!(written(&function, 0), vec![Register(2)]);
    }

    #[test]
    fn handler_reads_values_before_instruction_definitions() {
        let function = function(&["t0", "t0", "t2"], " handler 0 1 @2(r1 <- r0) r2\n", &[
            "const r0, int 1".into(),
            "ret r0".into(),
            "ret r1".into(),
        ]);
        assert_oracle(&function);
        assert_eq!(sets(&function).unwrap(), vec![vec![1], vec![1], vec![2]]);
        assert_eq!(written(&function, 0), vec![Register(0), Register(1), Register(2)]);
    }

    #[test]
    fn empty_code_and_zero_registers_match_oracle() {
        let mut function = function(&["t0"], "", &["ret r0".into()]);
        function.code.clear();
        assert_oracle(&function);
        assert_eq!(sets(&function).unwrap(), Vec::<Vec<u64>>::new());
        function.registers.clear();
        function.params.clear();
        function.code.push(Instruction::Unreachable {});
        assert_oracle(&function);
        assert_eq!(sets(&function).unwrap(), vec![Vec::<u64>::new()]);
    }

    #[test]
    fn slab_size_checks_boundaries_and_overflow() {
        let limit = MAX_LIVENESS_BYTES / size_of::<u64>();
        assert_eq!(storage_words(limit, 1), Some(limit));
        assert_eq!(storage_words(limit + 1, 1), None);
        assert_eq!(storage_words(limit / 2, 2), Some(limit));
        assert_eq!(storage_words(limit / 2 + 1, 2), None);
        assert_eq!(storage_words(usize::MAX, 2), None);
        assert_eq!(storage_words(1, usize::MAX), None);
        assert_eq!(storage_words(0, usize::MAX), Some(0));
        assert_eq!(storage_words(usize::MAX, 0), Some(0));
    }

    #[test]
    fn oversized_functions_are_refused() {
        let mut function = function(&["t0"], "", &["jmp @0".into()]);
        function.registers.resize(65536, crate::TypeRef(0));
        function.code.resize(MAX_LIVENESS_BYTES / (1024 * 8) + 1, function.code[0].clone());
        assert_oracle(&function);
        assert!(sets(&function).is_none());
        assert!(live_counts(&function).is_none());
        assert_eq!(live_weight(&function), u64::MAX);
    }

    fn weight(source: &str) -> u64 {
        let program = crate::assemble(source).unwrap();
        live_weight(&program.functions[0])
    }

    #[test]
    fn values_live_across_a_chain_cost_quadratically() {
        // k values defined one after another and all read at the end.
        let chain = |k: usize| {
            let mut text = String::from("sparrow 15\nmodule \"m\" \"v1.0.0\"\ntype t0 int\nfunction f0 \"main\" {\n registers [");
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
        let source = "sparrow 15\nmodule \"m\" \"v1.0.0\"\ntype t0 int\ntype t1 bool\ntype t2 failure\n\
            function f0 \"main\" {\n registers [t0, t0, t1, t2]\n params []\n result t0\n\
            \x20handler 2 4 @5 r3\n\
            \x200: const r0, int 1\n 1: const r1, int 0\n 2: int.add r1, r1, r0\n 3: jmp @2\n 4: ret r1\n 5: ret r0\n}\nentry f0\n";
        // r0 is live in the loop and in the handler; r1 in the loop.
        assert!(weight(source) >= 5, "{}", weight(source));
    }
}
