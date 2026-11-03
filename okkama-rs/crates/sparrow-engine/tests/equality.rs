use okkama_sparrow::{Cmp, Dst, Instruction as I, Register as R, assemble};
use sparrow_engine::{Executable, Mode};
#[test]
fn array_equality_and_any_identity_execute_in_both_engines() {
    let template = r#"sparrow 9
module "equality" "1.0.0"
type t0 int
type t1 array t0
type t2 bool
type t3 any
function f0 "main" {
registers [t0, t1, t1, t2, t0, t0, t3, t3]
params []
result t0
0: const r0, int 42
1: arr.new r1, [r0]
2: arr.new r2, [r0]
3: val.cmp eq, r3, r1, r2
4: const r4, int 0
5: const r5, int 1
6: br r3, @7, @8
7: ret r4
8: ret r5
}
entry f0
"#;
    let p = assemble(template).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), 0);
    }
    let mut boxed = p.clone();
    boxed.functions[0].code[1] = I::AnyBox {
        dst: Dst(R(6)),
        source: R(0),
    };
    boxed.functions[0].code[2] = I::Move {
        dst: Dst(R(7)),
        source: R(6),
    };
    boxed.functions[0].code[3] = I::ValCmp {
        op: Cmp::Eq,
        dst: Dst(R(3)),
        left: R(6),
        right: R(7),
    };
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&boxed, mode).unwrap().run().unwrap(), 0);
    }
    boxed.functions[0].code[2] = I::AnyBox {
        dst: Dst(R(7)),
        source: R(0),
    };
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(
            Executable::new(&boxed, mode).unwrap().run().unwrap_err().0,
            "Cannot compare any without narrowing"
        );
    }
}
