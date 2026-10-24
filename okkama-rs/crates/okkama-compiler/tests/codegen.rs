//! Backend generation verifies IR and emits deterministic class-file bytes.

use okkama_compiler::codegen::{self, Backend, CodegenError, Options};
use okkama_compiler::ir::{self, Constant, Inst, InstKind, IntValue, TermKind, Type, ValueId};
use okkama_compiler::lowering::lower_program;
use okkama_compiler::syntax::{self, ParseOptions};
use okkama_compiler::{SourceFile, Span};

fn module(source: &str) -> ir::Module {
    let source = SourceFile::new("test.ok", source).unwrap();
    let ast = syntax::parse(&source, &ParseOptions::default()).unwrap();
    lower_program(&ast).unwrap()
}

fn generate(module: &ir::Module) -> Result<Vec<u8>, CodegenError> {
    codegen::generate(
        module,
        Backend::Jvm,
        &Options {
            artifact_name: "Arithmetic".to_string(),
        },
    )
    .map(|artifact| artifact.main().to_vec())
}

fn addition() -> ir::Module {
    module("module app v1.0.0; main() is int { 1 + 1; }")
}

#[test]
fn generates_a_deterministic_java_8_class_from_module_ir() {
    for module in [
        addition(),
        module(
            "module app v1.0.0; main() is int { let x = 3; \
             let y = if x > 2 && x != 7 { x * 2; } else { 0; }; \
             for 0..x -> i { next if i == 1; } y; }",
        ),
    ] {
        let bytes = generate(&module).unwrap();
        assert_eq!(&bytes[..8], &[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52]);
        assert_eq!(bytes, generate(&module).unwrap());
        let renamed = codegen::generate(
            &module,
            Backend::Jvm,
            &Options {
                artifact_name: "Other".to_string(),
            },
        )
        .unwrap();
        assert_ne!(bytes, renamed.main());
    }
}

#[test]
fn invalid_ir_is_rejected_before_serialization() {
    let mut module = addition();
    let block = &mut module.functions[0].blocks[0];
    block.insts[2].kind = InstKind::IntBinary {
        op: ir::IntBinaryOp::Add,
        left: ValueId(0),
        right: ValueId(2),
    };
    let error = generate(&module).unwrap_err().to_string();
    assert!(error.contains("invalid IR"), "{error}");
    assert!(error.contains("%2 is used in ^b0"), "{error}");

    let mut module = addition();
    module.functions[0].blocks[0].term.kind = TermKind::Ret(Some(ValueId(99)));
    let error = generate(&module).unwrap_err().to_string();
    assert!(
        error.contains("%99 is not a value of this function"),
        "{error}"
    );
}

#[test]
fn unresolved_imports_missing_entry_and_method_collisions_are_errors() {
    let imported = module("module app v1.0.0; import \"missing\"; main() is int { 1; }");
    assert!(
        generate(&imported)
            .unwrap_err()
            .to_string()
            .contains("unresolved imports")
    );
    let empty = module("module library v1.0.0;");
    assert!(
        generate(&empty)
            .unwrap_err()
            .to_string()
            .contains("requires main() is int")
    );

    let mut module = addition();
    let mut other = module.functions[0].clone();
    other.symbol = "okkamaMain".to_string();
    other.attrs.clear();
    module.functions.push(other);
    assert!(
        generate(&module)
            .unwrap_err()
            .to_string()
            .contains("duplicate JVM method")
    );
}

#[test]
fn invalid_class_names_and_jvm_size_limits_are_errors() {
    let module = addition();
    for name in ["", "a/b", "a.b", "a;b", "a[b", "a\0b"] {
        let error = codegen::generate(
            &module,
            Backend::Jvm,
            &Options {
                artifact_name: name.to_string(),
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("invalid JVM class name"));
    }
    let error = codegen::generate(
        &module,
        Backend::Jvm,
        &Options {
            artifact_name: "a".repeat(65536),
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("UTF-8 constant exceeds"));

    // Длинный линейный код делится на части (`outline`): лимиты метода не
    // нарушаются, пока на разрезах живо мало значений; пул констант класса
    // остаётся общим.
    for (count, expected) in [(65536, Some("constant pool exceeds")), (12000, None)] {
        let mut module = addition();
        let function = &mut module.functions[0];
        function.values = vec![Type::Int; count];
        function.blocks[0].insts = (0..count)
            .map(|index| Inst {
                result: Some(ValueId(index)),
                kind: InstKind::Const(Constant::Int(IntValue::from_i64(index as i64))),
                span: Span::empty(0),
            })
            .collect();
        function.blocks[0].term.kind = TermKind::Ret(Some(ValueId(count - 1)));
        match (generate(&module), expected) {
            (Ok(_), None) => {}
            (Err(error), Some(text)) if error.to_string().contains(text) => {}
            (result, _) => panic!("{count}: {:?}", result.err().map(|error| error.to_string())),
        }
    }
    // Литерал из 70000 значений делится на отрезки builder, но 70000 int-констант
    // не помещаются в пул констант класса (u16): ошибка ограничения JVM.
    let count = 70000;
    let mut module = addition();
    let function = &mut module.functions[0];
    function.values = vec![Type::Int; count];
    function.values.push(Type::Array(Box::new(Type::Int)));
    let mut insts: Vec<Inst> = (0..count)
        .map(|index| Inst {
            result: Some(ValueId(index)),
            kind: InstKind::Const(Constant::Int(IntValue::from_i64(index as i64))),
            span: Span::empty(0),
        })
        .collect();
    insts.push(Inst {
        result: Some(ValueId(count)),
        kind: InstKind::ArrayNew((0..count).map(ValueId).collect()),
        span: Span::empty(0),
    });
    function.blocks[0].insts = insts;
    function.blocks[0].term.kind = TermKind::Ret(Some(ValueId(0)));
    let error = generate(&module).unwrap_err().to_string();
    assert!(
        error.contains("local-variable limit")
            || error.contains("bytecode exceeds")
            || error.contains("constant pool exceeds"),
        "{error}"
    );
}
