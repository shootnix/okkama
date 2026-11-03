use okkama_sparrow::{
    Cmp, Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Target, Type,
    TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};
fn str_(r: u32, s: &str) -> I {
    I::Const {
        dst: Dst(R(r)),
        value: Constant::Str(s.into()),
    }
}
fn int(r: u32, n: &str) -> I {
    I::Const {
        dst: Dst(R(r)),
        value: Constant::Int(n.into()),
    }
}
fn program(kinds: Vec<TypeRef>, code: Vec<I>) -> Program {
    Program {
        module: "text".into(),
        version: "1.0.0".into(),
        types: vec![Type::Str, Type::Int, Type::Bool, Type::Float, Type::Rat],
        entry: Some(FuncRef(0)),
        functions: vec![Function {
            symbol: "main".into(),
            params: vec![],
            is_async: false,
            native: None,
            intrinsic: None,
            handlers: vec![],
            registers: kinds,
            result: Some(TypeRef(1)),
            code,
        }],
        ..Program::default()
    }
}
fn check(mut kinds: Vec<TypeRef>, mut code: Vec<I>, result: u32, expected: &str) {
    let n = kinds.len() as u32;
    kinds.extend([TypeRef(0), TypeRef(2), TypeRef(1), TypeRef(1)]);
    code.extend([
        str_(n, expected),
        I::StrCmp {
            op: Cmp::Eq,
            dst: Dst(R(n + 1)),
            left: R(result),
            right: R(n),
        },
        int(n + 2, "0"),
        int(n + 3, "1"),
    ]);
    let branch = code.len() as u32;
    code.extend([
        I::Branch {
            condition: R(n + 1),
            then_target: Target {
                pc: branch + 1,
                moves: vec![],
            },
            else_target: Target {
                pc: branch + 2,
                moves: vec![],
            },
        },
        I::Return {
            value: Some(R(n + 2)),
        },
        I::Return {
            value: Some(R(n + 3)),
        },
    ]);
    let p = program(kinds, code);
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut s = code.start();
        loop {
            if let Outcome::Returned(n) = s.resume(1).unwrap() {
                assert_eq!(n, 0, "{mode:?}, expected {expected:?}");
                break;
            }
        }
    }
}
#[test]
fn unicode_text_operations_match_code_point_contracts() {
    for (op, input, expected) in [
        (
            I::StrTrim {
                dst: Dst(R(1)),
                text: R(0),
            },
            "\u{2003} A🙂 \u{2003}",
            "A🙂",
        ),
        (
            I::StrChomp {
                dst: Dst(R(1)),
                text: R(0),
            },
            "a\r\n\r\n",
            "a\r\n",
        ),
        (
            I::StrChomp {
                dst: Dst(R(1)),
                text: R(0),
            },
            "a\r",
            "a\r",
        ),
    ] {
        check(vec![TypeRef(0); 2], vec![str_(0, input), op], 1, expected);
    }
    check(
        vec![TypeRef(0), TypeRef(1), TypeRef(1), TypeRef(0)],
        vec![
            str_(0, "A🙂e\u{301}Z"),
            int(1, "1"),
            int(2, "4"),
            I::StrSlice {
                dst: Dst(R(3)),
                text: R(0),
                start: R(1),
                end: R(2),
            },
        ],
        3,
        "🙂e\u{301}",
    );
    check(
        vec![TypeRef(0), TypeRef(1), TypeRef(0)],
        vec![
            str_(0, "A🙂e\u{301}"),
            I::StrLen {
                dst: Dst(R(1)),
                text: R(0),
            },
            I::Convert {
                dst: Dst(R(2)),
                source: R(1),
            },
        ],
        2,
        "4",
    );
    check(
        vec![TypeRef(0); 3],
        vec![
            str_(0, "A🙂"),
            str_(1, "e\u{301}"),
            I::StrConcat {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
        ],
        2,
        "A🙂e\u{301}",
    );
    for (op, expected) in [
        (
            I::StrReplace {
                dst: Dst(R(3)),
                text: R(0),
                pattern: R(1),
                replacement: R(2),
            },
            "-🙂-A-",
        ),
        (
            I::StrReplaceFirst {
                dst: Dst(R(3)),
                text: R(0),
                pattern: R(1),
                replacement: R(2),
            },
            "-🙂A",
        ),
    ] {
        check(
            vec![TypeRef(0); 4],
            vec![str_(0, "🙂A"), str_(1, ""), str_(2, "-"), op],
            3,
            expected,
        );
    }
}
#[test]
fn exact_slice_bounds_preserve_error_priority() {
    for (start, end, message) in [
        ("-1", "999999999999999999999", "negative"),
        ("999999999999999999999", "-2", "less than or equal"),
        ("0", "999999999999999999999", "exceed length"),
    ] {
        let p = program(
            vec![TypeRef(0), TypeRef(1), TypeRef(1), TypeRef(0)],
            vec![
                str_(0, "🙂"),
                int(1, start),
                int(2, end),
                I::StrSlice {
                    dst: Dst(R(3)),
                    text: R(0),
                    start: R(1),
                    end: R(2),
                },
                I::Return { value: Some(R(1)) },
            ],
        );
        for mode in [Mode::Cold, Mode::Jit] {
            assert!(
                Executable::new(&p, mode)
                    .unwrap()
                    .run()
                    .unwrap_err()
                    .0
                    .contains(message)
            );
        }
    }
}
#[test]
fn scalar_text_and_rational_parts_preserve_language_representation() {
    check(
        vec![TypeRef(2), TypeRef(0)],
        vec![
            I::Const {
                dst: Dst(R(0)),
                value: Constant::Bool(true),
            },
            I::Convert {
                dst: Dst(R(1)),
                source: R(0),
            },
        ],
        1,
        "1",
    );
    check(
        vec![TypeRef(3), TypeRef(0)],
        vec![
            I::Const {
                dst: Dst(R(0)),
                value: Constant::Float((-0.0f64).to_bits()),
            },
            I::Convert {
                dst: Dst(R(1)),
                source: R(0),
            },
        ],
        1,
        "-0",
    );
    for (instruction, expected) in [
        (
            I::RatPart {
                dst: Dst(R(3)),
                value: R(2),
                denominator: false,
            },
            "-1",
        ),
        (
            I::RatPart {
                dst: Dst(R(3)),
                value: R(2),
                denominator: true,
            },
            "2",
        ),
        (
            I::Convert {
                dst: Dst(R(3)),
                source: R(2),
            },
            "-1/2",
        ),
    ] {
        check(
            vec![TypeRef(1), TypeRef(1), TypeRef(4), TypeRef(0)],
            vec![
                int(0, "-2"),
                int(1, "4"),
                I::RatNew {
                    dst: Dst(R(2)),
                    left: R(0),
                    right: R(1),
                },
                instruction,
            ],
            3,
            expected,
        );
    }
}
