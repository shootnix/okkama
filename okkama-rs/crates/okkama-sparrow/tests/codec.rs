//! SPB 9: детерминированная обратимая кодировка и строгий разбор.
mod common;
use common::*;
use okkama_sparrow::{
    BitOp, CapDecl, CapRef, CatchKind, Cmp, Constant, EnumDecl, EnumRef, FuncRef, FutOp, Handler, Instruction as I,
    Intrinsic, ObjectDecl, ObjectRef, Program, RoleDecl, RoleMethod, RoleRef, Target, TrapCode, Type, TypeRef, Witness,
    assemble, decode, disassemble, encode,
};

/// Тип регистра `Oops.text` по индексу в таблице типов `everything`.
const TEXT_REGISTERS: [u32; 40] = [
    7, 0, 0, 0, 0, 6, 8, 10, 5, 20, 3, 19, 11, 12, 13, 15, 9, 14, 4, 0, 2, 17, 18, 16, 1, 16, 7, 4, 1, 0, 16, 8, 9, 14, 0,
    0, 8, 1, 19, 0,
];

/// Определение регистра `index` значением его типа (пролог `Oops.text`).
fn define(index: u32, ty: &Type) -> I {
    let dst = d(index);
    match ty {
        Type::Int => I::Const { dst, value: int(&format!("-{index}")) },
        Type::Bool => I::Const { dst, value: Constant::Bool(index.is_multiple_of(2)) },
        Type::Str => I::Const { dst, value: Constant::Str(format!("s{index}")) },
        Type::Byte => I::Const { dst, value: Constant::Byte(index as u8) },
        Type::Float => I::Const { dst, value: Constant::Float((index as f64).to_bits()) },
        Type::Char => I::Const { dst, value: Constant::Char('c') },
        Type::Builder(_) => I::BuilderNew { dst },
        Type::MapBuilder(..) => I::MapBuilderNew { dst },
        _ => I::Undef { dst },
    }
}

/// Программа со всеми видами таблиц и инструкций (корректная, с типами).
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
        Type::Byte,
        Type::Float,
        Type::Char,
        Type::Failure,
        Type::Region,
        Type::Array(TypeRef(16)),
    ];
    program.enums = vec![EnumDecl { name: "Color".into(), cases: vec!["red".into(), "green \"x\"".into()] }];
    program.roles = vec![RoleDecl {
        name: "Error".into(),
        builtin: true,
        methods: vec![RoleMethod { name: "code".into(), params: vec![], result: Some(TypeRef(0)) }],
    }];
    program.objects = vec![ObjectDecl {
        name: "app::Oops".into(),
        fields: vec![("why".into(), TypeRef(2))],
        does: vec![Witness { role: RoleRef(0), slots: vec![FuncRef(2)] }],
        destroy: Some(FuncRef(5)),
        builtin: false,
    }];
    program.handles = vec!["FileState".into()];
    program.caps = vec![CapDecl { name: "Oops::View".into(), owner: ObjectRef(0), fields: vec![("why".into(), FuncRef(2))] }];
    let mut text = function("Oops.text", 40, vec![0], true, Vec::new());
    text.registers = TEXT_REGISTERS.iter().map(|&ty| TypeRef(ty)).collect();
    text.code = (1..40)
        .map(|index| define(index, &program.types[TEXT_REGISTERS[index as usize] as usize]))
        .chain([
            I::Const { dst: d(20), value: Constant::Str("tab\t\u{1}é🙂".into()) },
            I::Const { dst: d(21), value: Constant::Float(1.5f64.to_bits()) },
            I::Const { dst: d(22), value: Constant::Char('я') },
            I::Const { dst: d(23), value: Constant::Byte(255) },
            I::Const { dst: d(24), value: Constant::Bool(true) },
            I::Bits { op: BitOp::RotateLeft, byte: true, dst: d(25), value: r(23), count: Some(r(1)) },
            I::ObjNext { dst: d(26), object: r(0), updates: vec![(0, r(20))] },
            I::MapNew { dst: d(27), entries: vec![(r(20), r(1))] },
            I::Fut { op: FutOp::Cancelled, dst: Some(d(28)), future: r(5) },
            I::CallRole { dst: None, receiver: r(6), role: RoleRef(0), slot: 0, args: vec![] },
            I::CallCap { dst: d(29), cap: r(7), ty: CapRef(0), field: 0 },
            I::ByteFrom { dst: d(30), source: r(1), context: Some("@byte() argument 3".into()) },
            I::RoleNarrow { dst: d(31), source: r(6), role: RoleRef(0), message: "no \"role\"".into() },
            I::EnumCase { dst: d(32), ty: EnumRef(0), case: 1 },
            I::WorkerSpawn { dst: d(33), entry: r(8), args: vec![r(1)], direct: true },
            I::RegionLeave { region: r(9), keep: None },
            I::ArrayLookup { array: r(10), index: r(2), value: d(34), found: Target { pc: 58, moves: vec![(r(35), r(3))] }, missing: to(59) },
            I::Catch { failure: r(11), kind: CatchKind::Role, ty: 0, value: d(36), handler: to(59), next: to(60) },
            I::Return { value: Some(r(34)) },
            I::Trap { code: TrapCode::Confirm, message: "confirm failed".into() },
            I::IntCmp { op: Cmp::Ge, dst: d(37), left: r(1), right: r(2) },
            I::Return { value: Some(r(1)) },
        ])
        .collect();
    text.handlers = vec![Handler { start: 40, end: 50, target: to(59), failure: r(38) }];
    let mut native = function("runtime.write", 1, vec![0], true, Vec::new());
    native.registers[0] = TypeRef(21);
    native.native = Some("runtime/stdio@1::stdout_write/1".into());
    let mut intrinsic = function("okkama.lock.Lock.new", 1, vec![], true, Vec::new());
    intrinsic.intrinsic = Some(Intrinsic { name: "okkama.lock.Lock.new".into(), objects: vec![ObjectRef(0)], helpers: vec![FuncRef(1)] });
    intrinsic.is_async = true;
    let mut destroy = function("Oops.destroy", 1, vec![0], false, vec![I::Return { value: None }]);
    destroy.registers[0] = TypeRef(7);
    program.functions.extend([text, native, intrinsic, destroy]);
    program.tests = Some(vec![("works".into(), FuncRef(0))]);
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

/// Hardening H-11 (#24): every truncation is a decoder error with a byte
/// position, and no single corrupted byte panics or yields an unverified
/// program.
#[test]
fn every_truncation_and_corrupted_byte_is_a_positioned_error() {
    let program = everything();
    let bytes = encode(&program).unwrap();
    for cut in 0..bytes.len() {
        let error = decode(&bytes[..cut]).expect_err("a truncated file decodes");
        assert!(error.offset.is_some(), "truncated at {cut}: {error}");
    }
    for at in 0..bytes.len() {
        for value in 0..=u8::MAX {
            if bytes[at] == value {
                continue;
            }
            let mut corrupted = bytes.clone();
            corrupted[at] = value;
            if let Ok(decoded) = decode(&corrupted) {
                decoded.verify().expect("decode returns verified programs");
            }
        }
    }
}

/// A constant carried as data (#506): `init` only reads the global.
fn constant_program(data: &str) -> String {
    format!(
        "sparrow {}\nmodule \"tiny\" \"v1.0.0\"\ntype t0 str\ntype t1 array t0\ntype t2 int\ntype t3 map t0 t2\n\
         global g0 t1 f1 data {data}\n\
         function f0 \"main\" {{\n    registers [t1, t2]\n    params []\n    result t2\n    0: global.get r0, 0\n    1: arr.len r1, r0\n    2: ret r1\n}}\n\
         function f1 \"const.WORDS\" {{\n    registers [t1]\n    params []\n    result t1\n    0: global.get r0, 0\n    1: ret r0\n}}\n\
         entry f0\n",
        okkama_sparrow::VERSION
    )
}

#[test]
fn constant_data_round_trips_and_keeps_its_type() {
    let text = constant_program("array 3 str \"a\" str \"\" str \"β \\\"q\\\"\"");
    let program = assemble(&text).expect("assembles");
    let data = program.globals[0].data.as_ref().expect("data");
    assert_eq!(
        data,
        &okkama_sparrow::Data::Array(vec![
            okkama_sparrow::Data::Scalar(Constant::Str("a".into())),
            okkama_sparrow::Data::Scalar(Constant::Str(String::new())),
            okkama_sparrow::Data::Scalar(Constant::Str("β \"q\"".into())),
        ])
    );
    let bytes = encode(&program).expect("encodes");
    assert_eq!(decode(&bytes).expect("decodes"), program);
    assert_eq!(assemble(&disassemble(&program)).expect("reassembles"), program);
    // Data of another shape than the global's type is refused.
    for wrong in ["array 1 int 1", "str \"a\"", "map 1 str \"a\" int 1", "array 1 array 0"] {
        let error = assemble(&constant_program(wrong)).and_then(|program| {
            program.verify().map_err(|error| okkama_sparrow::AsmError { line: 0, message: error.to_string() })
        });
        assert!(
            error.as_ref().is_err_and(|error| error.message.contains("does not have its type")),
            "{wrong}: {error:?}"
        );
    }
}

/// A constant of `levels` nested arrays around one string, typed to match.
fn deep_program(levels: usize) -> String {
    let mut text = format!("sparrow {}\nmodule \"deep\" \"v1.0.0\"\ntype t0 int\ntype t1 str\n", okkama_sparrow::VERSION);
    for level in 0..levels {
        text += &format!("type t{} array t{}\n", level + 2, level + 1);
    }
    let top = levels + 1;
    text += &format!("global g0 t{top} f1 data {}str \"x\"\n", "array 1 ".repeat(levels));
    text += "function f0 \"main\" {\n    registers [t0]\n    params []\n    result t0\n    0: const r0, int 0\n    1: ret r0\n}\n";
    text += &format!(
        "function f1 \"const.DEEP\" {{\n    registers [t{top}]\n    params []\n    result t{top}\n    0: global.get r0, 0\n    1: ret r0\n}}\nentry f0\n"
    );
    text
}

#[test]
fn constant_data_nests_at_most_max_depth() {
    let max = okkama_sparrow::Data::MAX_DEPTH;
    let program = assemble(&deep_program(max)).expect("assembles");
    assert_eq!(decode(&encode(&program).expect("encodes")).expect("decodes"), program);
    assert!(assemble(&deep_program(max + 1)).is_err_and(|error| error.message.contains("nested deeper")));
    // One level more, typed to match, encodes but does not decode.
    let mut deeper = program;
    let top = TypeRef(deeper.types.len() as u32);
    deeper.types.push(Type::Array(deeper.globals[0].ty));
    deeper.globals[0].ty = top;
    let data = deeper.globals[0].data.take().expect("data");
    deeper.globals[0].data = Some(okkama_sparrow::Data::Array(vec![data]));
    deeper.functions[1].registers[0] = top;
    deeper.functions[1].result = Some(top);
    let bytes = encode(&deeper).expect("encodes");
    assert!(decode(&bytes).is_err_and(|error| error.to_string().contains("nested deeper")));
}
