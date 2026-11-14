//! Backend generation verifies IR and emits deterministic SPB bytes.

use okkama_compiler::codegen::{self, Backend, CodegenError, Options};
use okkama_compiler::ir::{self, InstKind, TermKind, ValueId};
use okkama_compiler::lowering::lower_program;
use okkama_compiler::syntax::{self, ParseOptions};
use okkama_compiler::SourceFile;

fn module(source: &str) -> ir::Module {
    let source = SourceFile::new("test.ok", source).unwrap();
    let ast = syntax::parse(&source, &ParseOptions::default()).unwrap();
    lower_program(&ast).unwrap()
}

fn generate(module: &ir::Module) -> Result<Vec<u8>, CodegenError> {
    codegen::generate(
        module,
        Backend::Sparrow,
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
fn generates_deterministic_sparrow_bytecode() {
    let module = addition();
    let bytes = generate(&module).unwrap();
    okkama_sparrow::decode(&bytes).expect("valid SPB");
    assert_eq!(bytes, generate(&module).unwrap());
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
fn unresolved_imports_are_rejected_before_serialization() {
    let imported = module("module app v1.0.0; import \"missing\"; main() is int { 1; }");
    assert!(generate(&imported).unwrap_err().to_string().contains("imports must be resolved"));
}
