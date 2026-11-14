//! Whole-register proofs for GC root omission (#535/#539), after SPB verification.

use crate::{program::Kind, word};
use okkama_sparrow::{Access, Constant, Function, Instruction as I, Program, Register, Type};

/// A least fixed point over *all* definitions, including unreachable code.
/// A register becomes proven only after all its move sources are proven and
/// all other definitions are independently pointer-free. Unknown definitions
/// veto the proof permanently. Even seeded move cycles stay traced: this is
/// intentionally less powerful than control-flow interval analysis.
pub(crate) fn analyze(function: &Function, kinds: &[Kind], program: &Program) -> Vec<bool> {
    analyze_with_ranges(function, kinds, program, true)
}

pub(crate) fn analyze_with_ranges(function: &Function, kinds: &[Kind], program: &Program, ranges: bool) -> Vec<bool> {
    analyze_bounded(function, kinds, program, ranges, okkama_sparrow::MAX_LIVENESS_BYTES)
}

fn analyze_bounded(function: &Function, kinds: &[Kind], program: &Program, ranges: bool, budget: usize) -> Vec<bool> {
    let n = kinds.len();
    // Driver/native registers belong to external code, not this instruction list.
    if function.native.is_some() || function.intrinsic.is_some() {
        return vec![false; n];
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
    // Preserve #535's storage cutoff when operation proofs are disabled.
    let per_register = if ranges { 4 * size_of::<usize>() + 8 } else { 2 * size_of::<usize>() + 2 };
    let scratch = n.saturating_mul(size_of::<Vec<usize>>() + per_register)
        .saturating_add(moves.saturating_mul(4 * size_of::<usize>()));
    if scratch > budget {
        return vec![false; n];
    }
    let mut defined = vec![false; n];
    let mut unknown = vec![false; n];
    let mut constant_unknown = vec![false; if ranges { n } else { 0 }];
    let mut remaining = vec![0usize; n];
    let mut users = vec![Vec::new(); n];
    for r in &function.params {
        // Closure captures occupy a prefix of these same parameters.
        unknown[r.0 as usize] = true;
        if ranges { constant_unknown[r.0 as usize] = true; }
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
        if ranges { constant_unknown[handler.failure.0 as usize] = true; }
        for &(dst, source) in &handler.target.moves {
            moved(dst, source);
        }
    }
    // First prove constant operand ranges over exactly the same whole-register
    // move graph. Int constants must be nonzero and tagged-small; byte/char
    // constants include zero. Verified moves preserve their source types.
    // Undef is NOT a numeric value: its raw sentinel can box in ops::convert.
    if ranges {
        for instruction in &function.code {
            if matches!(instruction, I::Move { .. }) {
                continue;
            }
            instruction.visit(&mut |access| {
                if let Access::Def(r) = access {
                    let r = r.0 as usize;
                    defined[r] = true;
                    let bounded = match instruction {
                        I::Const { value: Constant::Int(text), .. } => kinds[r] == Kind::Int
                            && text.parse::<i64>().ok().is_some_and(|n| n != 0 && word::small(n).is_some()),
                        I::Const { value: Constant::Byte(_) | Constant::Char(_), .. } => true,
                        _ => false,
                    };
                    constant_unknown[r] |= !bounded;
                }
            });
        }
    }
    let bounded_constants = if ranges {
        propagate(&defined, &constant_unknown, remaining.clone(), &users)
    } else {
        Vec::new()
    };
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
                    // Array lengths come from the upper u32 of the header in
                    // every flat/packed/shared/trie representation (jit.rs).
                    // Text's UTF-8 code-point count is <= its u32 byte payload,
                    // including shared text and the non-ASCII helper/cache.
                    // 0..=u32::MAX fits word::SMALL_MAX on every supported JIT
                    // target (Word is u64, even if usize were narrower).
                    I::ArrayLen { .. } | I::StrLen { .. } => ranges && kinds[r] == Kind::Int,
                    // MapLen uses a usize count in both BTree/PMap. There is no
                    // enforced tagged-range cap: heap.int may box. Allocation
                    // feasibility is not a range proof, so keep it traced.
                    I::MapLen { .. } => false,
                    // ops::convert uses heap.int(raw as i64); JIT tags inline.
                    // A byte is 0..255; a Unicode scalar is <=0x10ffff. Prove
                    // valid constants/moves too: declared raw types alone allow
                    // undef sentinels, including through parameters/calls.
                    // bool->int is unsupported; float/rat->int can box.
                    I::Convert { source, .. } => ranges && kinds[r] == Kind::Int
                        && matches!(program.ty(function.registers[source.0 as usize]), Some(Type::Byte | Type::Char))
                        && bounded_constants[source.0 as usize],
                    // Signed remainder (truncation toward zero): |a % d| < |d|.
                    // Nonzero tagged-small d has |d| <= 2^62, even SMALL_MIN;
                    // thus the result fits [-2^62+1, 2^62-1] for ANY BigInt a.
                    // The fast path tags; rt_int's BigInt fallback calls
                    // Heap::big, which normalizes small results. -1 uses that
                    // fallback; zero is excluded, retaining its failure path.
                    // All definitions of d, including edges/handlers, must be
                    // constants/moves in that range. No arithmetic/cycle seed.
                    I::IntMod { right, .. } => ranges && kinds[r] == Kind::Int
                        && bounded_constants[right.0 as usize],
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
    // Initial register/frame storage is unbound. Async restoration only
    // copies this register's own previously saved word; parameters are unknown
    // above, and await/settle results are ordinary unknown definitions. Thus
    // saving/restoring preserves the invariant without seeding a move cycle.
    propagate(&defined, &unknown, remaining, &users)
}

/// Least fixed point. Every definition must agree, duplicate move edges count
/// separately, and a seeded or unseeded cycle never proves itself.
fn propagate(defined: &[bool], unknown: &[bool], mut remaining: Vec<usize>, users: &[Vec<usize>]) -> Vec<bool> {
    let mut proven = vec![false; defined.len()];
    let mut pending = Vec::with_capacity(defined.len());
    for r in 0..defined.len() {
        if defined[r] && !unknown[r] && remaining[r] == 0 {
            proven[r] = true;
            pending.push(r);
        }
    }
    while let Some(source) = pending.pop() {
        for &dst in &users[source] {
            remaining[dst] -= 1;
            if remaining[dst] == 0 && !unknown[dst] {
                proven[dst] = true;
                pending.push(dst);
            }
        }
    }
    proven
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::{self, Repr};
    use okkama_sparrow::{Constant, Dst, Instruction as I, Register as R, Target};

    fn fixture(registers: &str, params: &str, handlers: &str, code: &[&str]) -> (Function, Vec<Kind>, Program) {
        let code: String = code.iter().enumerate().map(|(pc, text)| format!(" {pc}: {text}\n")).collect();
        let program = okkama_sparrow::assemble(&format!(
            "sparrow 16\nmodule \"proof\" \"1.0.0\"\ntype t0 int\ntype t1 bool\ntype t2 str\ntype t3 failure\ntype t4 array t0\ntype t5 byte\ntype t6 char\ntype t7 float\ntype t8 rat\ntype t9 map t0 t0\n\
             function f0 \"proof\" {{\n registers [{registers}]\n params [{params}]\n result t0\n{handlers}{code}}}\n"
        )).unwrap();
        let function = program.functions[0].clone();
        let kinds = function.registers.iter().map(|t| Kind::of(&program, *t)).collect();
        (function, kinds, program)
    }

    #[test]
    fn small_limits_big_and_text_literals() {
        for n in [word::SMALL_MIN, -1, 0, 1, word::SMALL_MAX, word::SMALL_MIN - 1, word::SMALL_MAX + 1, i64::MAX] {
            let (f, k, p) = fixture("t0", "", "", &[&format!("const r0, int {n}"), "ret r0"]);
            assert_eq!(analyze(&f, &k, &p), vec![word::small(n).is_some()], "{n}");
            if let Some(w) = word::small(n) {
                assert_eq!(Repr::Int.pointer(w), None);
            }
        }
        let (f, k, p) = fixture("t0, t2", "", "", &[
            "const r0, int 999999999999999999999999999999999999999",
            "const r1, str \"root me\"", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![false, false]);
    }

    #[test]
    fn every_definition_including_later_pointer_and_parameters() {
        let (mut f, k, p) = fixture("t0, t0, t0", "r0", "", &[
            "const r0, int 1", "const r1, int 2", "const r1, int 3",
            "move r2, r1", "ret r2",
        ]);
        // A capture is a parameter prefix too; an overwritten input is unknown.
        assert_eq!(analyze(&f, &k, &p), vec![false, true, true]);
        f.code.insert(4, I::Const { dst: Dst(R(1)), value: Constant::Int("4611686018427387904".into()) });
        assert_eq!(analyze(&f, &k, &p), vec![false, false, false]);
    }

    #[test]
    fn joins_parallel_edges_and_fixed_point_moves() {
        let (mut f, k, p) = fixture("t0, t0, t0, t0, t1", "r4", "", &[
            "const r0, int 7", "const r1, int 8", "br r4, @3(r2 <- r0), @3(r2 <- r1)",
            "move r3, r2", "ret r3",
        ]);
        assert_eq!(&analyze(&f, &k, &p)[..4], &[true; 4]);
        f.params.push(R(1));
        assert_eq!(&analyze(&f, &k, &p)[..4], &[true, false, false, false]);
        f.params.pop();
        // Read all sources of a parallel move in the pre-edge state. A swap
        // creates a dependency cycle, not an ordered sequence of assignments.
        f.code[2] = I::Jump { target: Target { pc: 3, moves: vec![(R(0), R(1)), (R(1), R(0)), (R(2), R(0))] } };
        assert_eq!(&analyze(&f, &k, &p)[..4], &[false; 4]);
    }

    #[test]
    fn loops_unknown_sources_and_unbound_states() {
        let (mut f, k, p) = fixture("t0, t0, t0, t0, t0", "", "", &[
            "const r0, int 1", "undef r1", "move r2, r0", "jmp @2", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![true, true, true, false, false]);
        // Deliberately unverified IR to exercise unseeded cycles and absent
        // definitions; neither may bootstrap a proof.
        f.code[0] = I::Move { dst: Dst(R(0)), source: R(2) };
        f.code.push(I::Move { dst: Dst(R(1)), source: R(3) });
        assert_eq!(analyze(&f, &k, &p), vec![false; 5]);
    }

    #[test]
    fn handlers_and_edge_values_are_definitions() {
        let (mut f, k, p) = fixture("t0, t0, t3", "r0", " handler 1 2 @3(r1 <- r0) r2\n", &[
            "const r1, int 1", "const r0, int 2", "ret r1", "ret r1",
        ]);
        f.code.push(I::Undef { dst: Dst(R(2)) });
        assert_eq!(analyze(&f, &k, &p), vec![false; 3]);
        let (f, k, p) = fixture("t0, t0, t4", "r2", "", &[
            "const r0, int 0", "const r1, int 1", "arr.lookup r2, r0, r1, @3, @3", "ret r1",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![true, false, false]);
    }

    #[test]
    fn unsupported_definitions_never_inherit_a_constant_proof() {
        let (f, k, p) = fixture("t0, t0", "", "", &[
            "const r0, int 1", "const r1, int 2", "int.add r0, r0, r1", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![false, true]);
    }

    #[test]
    fn reverse_order_dependencies_and_pointer_free_unbound_references() {
        let (f, k, p) = fixture("t0, t0, t0, t2, t2, t1", "", "", &[
            "const r2, int 9", "move r1, r2", "move r0, r1",
            "undef r3", "move r4, r3", "const r5, bool true", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![true; 6]);
        assert_eq!(Repr::Ref.pointer(word::UNBOUND), None);
    }

    #[test]
    fn operation_ranges_accept_only_bounded_representations() {
        for (ty, op, expected) in [
            ("t4", "arr.len", true),
            ("t2", "str.len", true),
            ("t9", "map.len", false),
            ("t5", "cvt", false),
            ("t6", "cvt", false),
            ("t7", "cvt", false),
            ("t8", "cvt", false),
        ] {
            let (f, k, p) = fixture(&format!("t0, {ty}"), "r1", "", &[
                &format!("{op} r0, r1"), "ret r0",
            ]);
            assert_eq!(analyze(&f, &k, &p)[0], expected, "{ty} {op}");
        }
        for (ty, literal) in [("t5", "byte 255"), ("t6", "char 1114111")] {
            let (f, k, p) = fixture(&format!("t0, {ty}, {ty}"), "", "", &[
                &format!("const r1, {literal}"), "move r2, r1", "cvt r0, r2", "ret r0",
            ]);
            assert!(analyze(&f, &k, &p)[0], "{literal}");
            let (f, k, p) = fixture(&format!("t0, {ty}"), "", "", &[
                "undef r1", "cvt r0, r1", "ret r0",
            ]);
            assert!(!analyze(&f, &k, &p)[0], "unbound {ty}");
        }
        // bool -> int is not a legal SPB conversion. Never widen the numeric
        // contract just because the roadmap's candidate list mentions bool.
        assert!(okkama_sparrow::assemble("sparrow 16\nmodule \"bad\" \"1\"\ntype t0 int\ntype t1 bool\nfunction f0 \"bad\" {\n registers [t0, t1]\n params [r1]\n result t0\n 0: cvt r0, r1\n 1: ret r0\n}\n").is_err());
    }

    #[test]
    fn modulo_proves_every_divisor_definition_and_signed_limits() {
        for divisor in [word::SMALL_MIN, -7, -1, 0, 1, 7, word::SMALL_MAX,
            word::SMALL_MIN - 1, word::SMALL_MAX + 1, i64::MIN, i64::MAX] {
            let (f, k, p) = fixture("t0, t0, t0, t0", "r0", "", &[
                &format!("const r1, int {divisor}"), "move r2, r1",
                "int.mod r3, r0, r2", "ret r3",
            ]);
            assert_eq!(analyze(&f, &k, &p), vec![false, word::small(divisor).is_some(),
                word::small(divisor).is_some(), divisor != 0 && word::small(divisor).is_some()], "{divisor}");
        }
        for definition in ["const r1, int 0", "const r1, int 4611686018427387904",
            "const r1, int 999999999999999999999999999999999", "int.add r1, r1, r1", "undef r1"] {
            let (f, k, p) = fixture("t0, t0, t0", "r0", "", &[
                "const r1, int 7", definition, "int.mod r2, r0, r1", "ret r2",
            ]);
            assert!(!analyze(&f, &k, &p)[2], "{definition}");
        }
    }

    #[test]
    fn operation_joins_reuse_parameters_handlers_and_cycles_veto_proofs() {
        for (ty, op) in [("t4", "arr.len"), ("t2", "str.len"), ("t5", "cvt"), ("t6", "cvt")] {
            let initial = match ty {
                "t5" => "const r1, byte 255",
                "t6" => "const r1, char 1114111",
                _ => "undef r0",
            };
            let params = if matches!(ty, "t4" | "t2") { "r1" } else { "" };
            let (mut f, k, p) = fixture(&format!("t0, {ty}, t0"), params, "", &[
                initial, &format!("{op} r0, r1"), "jmp @3(r2 <- r0)", "ret r2",
            ]);
            assert!(analyze(&f, &k, &p)[2], "{op} {ty}");
            // A coloured destination with a later boxed definition loses both
            // its proof and the edge receiver's proof, even if unreachable.
            f.code.push(I::Const { dst: Dst(R(0)), value: Constant::Int("4611686018427387904".into()) });
            assert!(!analyze(&f, &k, &p)[0], "{op} {ty}");
            assert!(!analyze(&f, &k, &p)[2], "{op} {ty}");
        }
        let (mut f, k, p) = fixture("t0, t5, t5, t5, t1", "r4", "", &[
            "const r1, byte 0", "const r2, byte 255",
            "br r4, @3(r3 <- r1), @3(r3 <- r2)", "cvt r0, r3", "ret r0",
        ]);
        assert!(analyze(&f, &k, &p)[0]);
        f.params.push(R(2));
        assert!(!analyze(&f, &k, &p)[0]);
        let (mut f, k, p) = fixture("t0, t0, t0, t0, t0, t1", "r0, r5", "", &[
            "const r1, int -7", "const r2, int 3",
            "br r5, @3(r3 <- r1), @3(r3 <- r2)",
            "int.mod r4, r0, r3", "ret r4",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![false, true, true, true, true, false]);
        assert!(!analyze_with_ranges(&f, &k, &p, false)[4]);
        f.code[1] = I::Const { dst: Dst(R(2)), value: Constant::Int("0".into()) };
        assert!(!analyze(&f, &k, &p)[4]);
        let (mut f, k, p) = fixture("t0, t0, t0, t0, t1, t3", "r0, r4", "", &[
            "const r1, int -7", "const r2, int 3",
            "br r4, @3(r3 <- r1), @3(r3 <- r2)",
            "int.mod r1, r0, r3", "ret r1",
        ]);
        // Reusing a constant's slot for a modulo result invalidates its use as
        // a nonzero divisor everywhere, including the earlier edge.
        assert!(!analyze(&f, &k, &p)[1]);
        f.code[3] = I::IntMod { dst: Dst(R(2)), left: R(0), right: R(3) };
        assert!(!analyze(&f, &k, &p)[2]);
        let (mut f, k, p) = fixture("t0, t0, t0, t0, t1, t3", "r0, r4", "", &[
            "const r1, int -7", "const r2, int 3",
            "br r4, @3(r3 <- r1), @3(r3 <- r2)",
            "int.mod r0, r0, r3", "ret r0",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![false, true, true, true, false, false]);
        f.params.push(R(1));
        // Unknown divisor parameters poison dependent modulo definitions.
        assert!(!analyze(&f, &k, &p)[3]);
        let (f, k, p) = fixture("t0, t0, t4", "r2", "", &[
            "arr.len r0, r2", "move r1, r0", "int.mul r0, r0, r0", "ret r1",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![false; 3]);
        let (f, k, p) = fixture("t0, t0, t3", "r0", " handler 1 2 @3(r1 <- r0) r2\n", &[
            "const r1, int 7", "int.mod r0, r0, r1", "ret r0", "ret r1",
        ]);
        assert_eq!(analyze(&f, &k, &p), vec![false; 3]);
    }

    #[test]
    fn exhausted_analysis_budget_keeps_all_registers_traced() {
        let (f, k, p) = fixture("t0, t0", "", "", &[
            "const r0, int 7", "jmp @2(r1 <- r0)", "ret r1",
        ]);
        assert_eq!(analyze_bounded(&f, &k, &p, true, 0), vec![false; 2]);
        assert_eq!(analyze_bounded(&f, &k, &p, false, 0), vec![false; 2]);
        // The exact #535 budget still admits its constant/move graph when
        // #539 is disabled; the extra range state needs a larger budget.
        let constant_budget = 2 * (size_of::<Vec<usize>>() + 2 * size_of::<usize>() + 2)
            + 4 * size_of::<usize>();
        assert_eq!(analyze_bounded(&f, &k, &p, false, constant_budget), vec![true; 2]);
        assert_eq!(analyze_bounded(&f, &k, &p, true, constant_budget), vec![false; 2]);
        assert_eq!(analyze(&f, &k, &p), vec![true; 2]);
    }
}
