//! SPB 9: детерминированная обратимая кодировка и строгий разбор.
mod common;
use common::*;
use okkama_sparrow::{
    BitOp, CapDecl, CapRef, CatchKind, Cmp, Constant, EnumDecl, EnumRef, FuncRef, FutOp, Handler, Instruction as I,
    Intrinsic, ObjectDecl, ObjectRef, Program, RoleDecl, RoleMethod, RoleRef, Target, TrapCode, Type, TypeRef, Witness,
    assemble, decode, disassemble, encode,
};

/// Программа со всеми видами таблиц и инструкций (синтаксически корректная).
fn everything() -> Program {
    let mut program = sample();
    program.types = vec![
        Type::Int,
        Type::Bool,
        Type::Str,
        Type::Array(TypeRef(0)),
        Type::Map(TypeRef(2), TypeRef(0)),
        Type::Fn { params: vec![TypeRef(0)], result: None },
        Type::Future(Some(TypeRef(0))),
        Type::Object(ObjectRef(0)),
        Type::Role(RoleRef(0)),
        Type::Enum(EnumRef(0)),
        Type::Cap(CapRef(0)),
        Type::Handle(0),
        Type::MapBuilder(TypeRef(2), TypeRef(0)),
        Type::Builder(TypeRef(0)),
        Type::Future(None),
        Type::Successor(ObjectRef(0)),
    ];
    program.enums = vec![EnumDecl { name: "Color".into(), cases: vec!["red".into(), "green \"x\"".into()] }];
    program.roles = vec![RoleDecl {
        name: "Error".into(),
        builtin: true,
        methods: vec![RoleMethod { name: "text".into(), params: vec![], result: Some(TypeRef(2)) }],
    }];
    program.objects = vec![ObjectDecl {
        name: "app::Oops".into(),
        fields: vec![("why".into(), TypeRef(2))],
        does: vec![Witness { role: RoleRef(0), slots: vec![FuncRef(2)] }],
        destroy: Some(FuncRef(2)),
        builtin: false,
    }];
    program.handles = vec!["FileState".into()];
    program.caps = vec![CapDecl { name: "Oops::View".into(), owner: ObjectRef(0), fields: vec![("why".into(), FuncRef(2))] }];
    let mut text = function("Oops.text", 40, vec![0], true, Vec::new());
    text.code = (0..39)
        .map(|index| I::Const { dst: d(index + 1), value: int(&format!("-{}", index + 1)) })
        .chain([
            I::Const { dst: d(20), value: Constant::Str("tab\t\u{1}é🙂".into()) },
            I::Const { dst: d(21), value: Constant::Float(1.5f64.to_bits()) },
            I::Const { dst: d(22), value: Constant::Char('я') },
            I::Const { dst: d(23), value: Constant::Byte(255) },
            I::Const { dst: d(24), value: Constant::Bool(true) },
            I::Bits { op: BitOp::RotateLeft, byte: true, dst: d(25), value: r(23), count: Some(r(1)) },
            I::ObjNext { dst: d(26), object: r(0), updates: vec![(0, r(20))] },
            I::MapNew { dst: d(27), entries: vec![(r(20), r(1))] },
            I::Fut { op: FutOp::Cancelled, dst: Some(d(28)), future: r(1) },
            I::CallRole { dst: None, receiver: r(0), role: RoleRef(0), slot: 0, args: vec![r(1), r(2)] },
            I::CallCap { dst: d(29), cap: r(0), ty: CapRef(0), field: 0 },
            I::ByteFrom { dst: d(30), source: r(1), context: Some("@byte() argument 3".into()) },
            I::RoleNarrow { dst: d(31), source: r(0), role: RoleRef(0), message: "no \"role\"".into() },
            I::EnumCase { dst: d(32), ty: EnumRef(0), case: 1 },
            I::WorkerSpawn { dst: d(33), entry: r(1), args: vec![], direct: true },
            I::RegionLeave { region: r(1), keep: None },
            I::ArrayLookup { array: r(1), index: r(2), value: d(34), found: Target { pc: 58, moves: vec![(r(35), r(3))] }, missing: to(59) },
            I::Catch { failure: r(1), kind: CatchKind::Role, ty: 0, value: d(36), handler: to(59), next: to(60) },
            I::Return { value: Some(r(34)) },
            I::Trap { code: TrapCode::Confirm, message: "confirm failed".into() },
            I::IntCmp { op: Cmp::Ge, dst: d(37), left: r(1), right: r(2) },
            I::Return { value: Some(r(37)) },
        ])
        .collect();
    text.handlers = vec![Handler { start: 40, end: 50, target: to(59), failure: r(38) }];
    text.registers[0] = TypeRef(7);
    let mut native = function("runtime.write", 1, vec![0], false, Vec::new());
    native.native = Some("runtime/stdio@1::stdout_write/1".into());
    let mut intrinsic = function("okkama.lock.Lock.new", 1, vec![], true, Vec::new());
    intrinsic.intrinsic = Some(Intrinsic { name: "okkama.lock.Lock.new".into(), objects: vec![ObjectRef(0)], helpers: vec![FuncRef(1)] });
    intrinsic.is_async = true;
    program.functions.extend([text, native, intrinsic]);
    program.tests = Some(vec![("works".into(), FuncRef(1))]);
    program
}

#[test]
fn encoding_is_deterministic_and_round_trips() {
    for program in [sample(), everything()] {
        program.verify().expect("valid program");
        let bytes = encode(&program).expect("encodes");
        assert_eq!(bytes, encode(&program).expect("encodes again"));
        assert_eq!(&bytes[..4], b"SPRW");
        assert_eq!(u16::from_be_bytes([bytes[4], bytes[5]]), okkama_sparrow::VERSION);
        assert_eq!(decode(&bytes).expect("decodes"), program);
    }
}

#[test]
fn assembly_round_trips_to_the_same_program() {
    for program in [sample(), everything()] {
        let text = disassemble(&program);
        let parsed = assemble(&text).unwrap_or_else(|error| panic!("{error}\n{text}"));
        assert_eq!(parsed, program);
        assert_eq!(disassemble(&parsed), text);
    }
}

#[test]
fn decoding_rejects_malformed_files() {
    let bytes = encode(&sample()).unwrap();
    assert!(decode(b"NOPE").unwrap_err().to_string().contains("not a Sparrow bytecode file"));
    let mut old = bytes.clone();
    old[5] = 8;
    assert!(decode(&old).unwrap_err().to_string().contains("unsupported SPB version 8"));
    for cut in [6, bytes.len() / 2, bytes.len() - 1] {
        assert!(decode(&bytes[..cut]).is_err(), "truncated at {cut}");
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&trailing).unwrap_err().to_string().contains("trailing bytes"));
}

#[test]
fn decoding_verifies_the_program() {
    let mut program = sample();
    program.functions[0].code[0] = I::Const { dst: d(0), value: int("10") };
    let mut bytes = encode(&program).unwrap();
    // Последний байт программы — маркер тестов (0); перед ним — entry `f0`
    // (тег 1, индекс 0). Ссылка на отсутствующую функцию ловится проверкой.
    let len = bytes.len();
    assert_eq!(&bytes[len - 3..], &[1, 0, 0]);
    bytes[len - 2] = 9;
    assert!(decode(&bytes).unwrap_err().to_string().contains("invalid program"));
}

#[test]
fn assembly_errors_name_the_line() {
    let text = disassemble(&sample()).replace("int.mul", "int.mull");
    let error = assemble(&text).unwrap_err();
    assert!(error.message.contains("unknown instruction `int.mull`"), "{error}");
    assert!(error.line > 1);
    let text = disassemble(&sample()).replacen("    1: call", "    7: call", 1);
    assert!(assemble(&text).unwrap_err().message.contains("expected instruction 1"));
}
