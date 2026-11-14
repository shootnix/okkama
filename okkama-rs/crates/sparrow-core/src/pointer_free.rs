//! Whole-register proofs for GC root omission (#535), after SPB verification.

use crate::{program::Kind, word};
use okkama_sparrow::{Access, Constant, Function, Instruction as I, Register};

/// A least fixed point over *all* definitions, including unreachable code.
/// A register becomes proven only after all its move sources are proven and
/// all other definitions are independently pointer-free. Unknown definitions
/// veto the proof permanently. Even seeded move cycles stay traced: this is
/// intentionally less powerful than a control-flow/range analysis (#539).
pub(crate) fn analyze(function: &Function, kinds: &[Kind]) -> Vec<bool> {
    analyze_bounded(function, kinds, okkama_sparrow::MAX_LIVENESS_BYTES)
}

fn analyze_bounded(function: &Function, kinds: &[Kind], budget: usize) -> Vec<bool> {
    let n = kinds.len();
    let mut proven = vec![false; n];
    // Driver/native registers belong to external code, not this instruction list.
    if function.native.is_some() || function.intrinsic.is_some() {
        return proven;
    }
    // Bound auxiliary graph storage before allocating it, like liveness.
    // Returning no proofs is always safe. Four words per edge cover even a
    // one-element Vec's minimum capacity; the queue reserves exactly n words.
    let mut moves = 0usize;
    for instruction in &function.code {
        moves = moves.saturating_add(usize::from(matches!(instruction, I::Move { .. })));
        instruction.visit(&mut |access| {
            if let Access::Target(target) = access {
                moves = moves.saturating_add(target.moves.len());
            }
        });
    }
    for handler in &function.handlers {
        moves = moves.saturating_add(handler.target.moves.len());
    }
    let scratch = n.saturating_mul(size_of::<Vec<usize>>() + 2 * size_of::<usize>() + 2)
        .saturating_add(moves.saturating_mul(4 * size_of::<usize>()));
    if scratch > budget {
        return proven;
    }
    let mut defined = vec![false; n];
    let mut unknown = vec![false; n];
    let mut remaining = vec![0usize; n];
    let mut users = vec![Vec::new(); n];
    for r in &function.params {
        // Closure captures occupy a prefix of these same parameters.
        unknown[r.0 as usize] = true;
    }
    let mut moved = |dst: Register, source: Register| {
        let dst = dst.0 as usize;
        defined[dst] = true;
        remaining[dst] += 1;
        users[source.0 as usize].push(dst);
    };
    // Targets expose their parallel moves separately from Access::Def. Do not
    // interpret the move list sequentially or omit a handler's implicit value.
    for instruction in &function.code {
        instruction.visit(&mut |access| {
            if let Access::Target(target) = access {
                for &(dst, source) in &target.moves {
                    moved(dst, source);
                }
            }
        });
        if let I::Move { dst, source } = instruction {
            moved(dst.0, *source);
        }
    }
    for handler in &function.handlers {
        unknown[handler.failure.0 as usize] = true;
        for &(dst, source) in &handler.target.moves {
            moved(dst, source);
        }
    }
    for instruction in &function.code {
        if matches!(instruction, I::Move { .. }) {
            continue;
        }
        instruction.visit(&mut |access| {
            if let Access::Def(r) = access {
                let r = r.0 as usize;
                defined[r] = true;
                let free = match instruction {
                    I::Const { value: Constant::Int(text), .. } => {
                        kinds[r] == Kind::Int && text.parse::<i64>().ok().and_then(word::small).is_some()
                    }
                    I::Const { value: Constant::Bool(_) | Constant::Byte(_) | Constant::Char(_) | Constant::Float(_), .. } => {
                        !kinds[r].traced()
                    }
                    // ops::execute writes UNBOUND (0) for Int/Ref and
                    // UNBOUND_RAW for raw kinds; none is a traced pointer.
                    I::Undef { .. } => true,
                    // Includes strings, calls, await results, lookup/catch
                    // edge values, arithmetic and every future instruction.
                    _ => false,
                };
                unknown[r] |= !free;
            }
        });
    }
    let mut pending = Vec::with_capacity(n);
    for r in 0..n {
        if defined[r] && !unknown[r] && remaining[r] == 0 {
            proven[r] = true;
            pending.push(r);
        }
    }
    // O(registers + move operands) propagation, including duplicates.
    while let Some(source) = pending.pop() {
        for &dst in &users[source] {
            remaining[dst] -= 1;
            if remaining[dst] == 0 && !unknown[dst] {
                proven[dst] = true;
                pending.push(dst);
            }
        }
    }
    // Initial register/frame storage is unbound. Async restoration only
    // copies this register's own previously saved word; parameters are unknown
    // above, and await/settle results are ordinary unknown definitions. Thus
    // saving/restoring preserves the invariant without seeding a move cycle.
    proven
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::{self, Repr};
    use okkama_sparrow::{Constant, Dst, Instruction as I, Register as R, Target};

    fn fixture(registers: &str, params: &str, handlers: &str, code: &[&str]) -> (Function, Vec<Kind>) {
        let code: String = code.iter().enumerate().map(|(pc, text)| format!(" {pc}: {text}\n")).collect();
        let program = okkama_sparrow::assemble(&format!(
            "sparrow 15\nmodule \"proof\" \"1.0.0\"\ntype t0 int\ntype t1 bool\ntype t2 str\ntype t3 failure\ntype t4 array t0\n\
             function f0 \"proof\" {{\n registers [{registers}]\n params [{params}]\n result t0\n{handlers}{code}}}\n"
        )).unwrap();
        let function = program.functions[0].clone();
        let kinds = function.registers.iter().map(|t| Kind::of(&program, *t)).collect();
        (function, kinds)
    }

    #[test]
    fn small_limits_big_and_text_literals() {
        for n in [word::SMALL_MIN, -1, 0, 1, word::SMALL_MAX, word::SMALL_MIN - 1, word::SMALL_MAX + 1, i64::MAX] {
            let (f, k) = fixture("t0", "", "", &[&format!("const r0, int {n}"), "ret r0"]);
            assert_eq!(analyze(&f, &k), vec![word::small(n).is_some()], "{n}");
            if let Some(w) = word::small(n) {
                assert_eq!(Repr::Int.pointer(w), None);
            }
        }
        let (f, k) = fixture("t0, t2", "", "", &[
            "const r0, int 999999999999999999999999999999999999999",
            "const r1, str \"root me\"", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k), vec![false, false]);
    }

    #[test]
    fn every_definition_including_later_pointer_and_parameters() {
        let (mut f, k) = fixture("t0, t0, t0", "r0", "", &[
            "const r0, int 1", "const r1, int 2", "const r1, int 3",
            "move r2, r1", "ret r2",
        ]);
        // A capture is a parameter prefix too; an overwritten input is unknown.
        assert_eq!(analyze(&f, &k), vec![false, true, true]);
        f.code.insert(4, I::Const { dst: Dst(R(1)), value: Constant::Int("4611686018427387904".into()) });
        assert_eq!(analyze(&f, &k), vec![false, false, false]);
    }

    #[test]
    fn joins_parallel_edges_and_fixed_point_moves() {
        let (mut f, k) = fixture("t0, t0, t0, t0, t1", "r4", "", &[
            "const r0, int 7", "const r1, int 8", "br r4, @3(r2 <- r0), @3(r2 <- r1)",
            "move r3, r2", "ret r3",
        ]);
        assert_eq!(&analyze(&f, &k)[..4], &[true; 4]);
        f.params.push(R(1));
        assert_eq!(&analyze(&f, &k)[..4], &[true, false, false, false]);
        f.params.pop();
        // Read all sources of a parallel move in the pre-edge state. A swap
        // creates a dependency cycle, not an ordered sequence of assignments.
        f.code[2] = I::Jump { target: Target { pc: 3, moves: vec![(R(0), R(1)), (R(1), R(0)), (R(2), R(0))] } };
        assert_eq!(&analyze(&f, &k)[..4], &[false; 4]);
    }

    #[test]
    fn loops_unknown_sources_and_unbound_states() {
        let (mut f, k) = fixture("t0, t0, t0, t0, t0", "", "", &[
            "const r0, int 1", "undef r1", "move r2, r0", "jmp @2", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k), vec![true, true, true, false, false]);
        // Deliberately unverified IR to exercise unseeded cycles and absent
        // definitions; neither may bootstrap a proof.
        f.code[0] = I::Move { dst: Dst(R(0)), source: R(2) };
        f.code.push(I::Move { dst: Dst(R(1)), source: R(3) });
        assert_eq!(analyze(&f, &k), vec![false; 5]);
    }

    #[test]
    fn handlers_and_edge_values_are_definitions() {
        let (mut f, k) = fixture("t0, t0, t3", "r0", " handler 1 2 @3(r1 <- r0) r2\n", &[
            "const r1, int 1", "const r0, int 2", "ret r1", "ret r1",
        ]);
        f.code.push(I::Undef { dst: Dst(R(2)) });
        assert_eq!(analyze(&f, &k), vec![false; 3]);
        let (f, k) = fixture("t0, t0, t4", "r2", "", &[
            "const r0, int 0", "const r1, int 1", "arr.lookup r2, r0, r1, @3, @3", "ret r1",
        ]);
        assert_eq!(analyze(&f, &k), vec![true, false, false]);
    }

    #[test]
    fn unsupported_definitions_never_inherit_a_constant_proof() {
        let (f, k) = fixture("t0, t0", "", "", &[
            "const r0, int 1", "const r1, int 2", "int.add r0, r0, r1", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k), vec![false, true]);
    }

    #[test]
    fn reverse_order_dependencies_and_pointer_free_unbound_references() {
        let (f, k) = fixture("t0, t0, t0, t2, t2, t1", "", "", &[
            "const r2, int 9", "move r1, r2", "move r0, r1",
            "undef r3", "move r4, r3", "const r5, bool true", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k), vec![true; 6]);
        assert_eq!(Repr::Ref.pointer(word::UNBOUND), None);
    }

    #[test]
    fn exhausted_analysis_budget_keeps_all_registers_traced() {
        let (f, k) = fixture("t0, t0", "", "", &[
            "const r0, int 7", "jmp @2(r1 <- r0)", "ret r1",
        ]);
        assert_eq!(analyze_bounded(&f, &k, 0), vec![false; 2]);
        assert_eq!(analyze(&f, &k), vec![true; 2]);
    }
}
