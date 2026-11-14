//! Phase contracts (H-19, #36): every public consumer of OIR rejects a
//! corrupted module, and failures point at the source operation whichever IR
//! passes ran.

use okkama_compiler::codegen::{self, Backend, Options};
use okkama_compiler::ir::{self, ValueId};
use okkama_compiler::lowering::lower_program;
use okkama_compiler::syntax::{self, ParseOptions};
use okkama_compiler::SourceFile;
use std::process::Command;

fn module(text: &str) -> ir::Module {
    let source = SourceFile::new("test.ok", text).unwrap();
    lower_program(&syntax::parse(&source, &ParseOptions::default()).unwrap()).unwrap()
}

#[test]
fn every_consumer_rejects_corrupted_ir() {
    let mut corrupted = module("module app; object Sum { Sum(); of(a is int, b is int) is int { a * b + a; } } main() is int { Sum().of(2, 3); }");
    // An operand that names no value of its function.
    let function = corrupted.functions.iter_mut().find(|function| function.symbol == "Sum.of").unwrap();
    let missing = ValueId(function.values.len() + 7);
    let operand = function.blocks.iter_mut().flat_map(|block| block.insts.iter_mut()).flat_map(|inst| inst.kind.operands_mut()).next();
    *operand.expect("an operand") = missing;
    assert!(ir::verify(&corrupted).is_err());
    let options = Options { artifact_name: "Corrupted".into() };
    assert!(codegen::generate(&corrupted, Backend::Sparrow, &options).is_err());
}

#[test]
fn failures_point_at_the_source_operation_after_every_pass() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/provenance");
    for (program, place) in [
        ("short-circuit", "short-circuit.ok:4:38"),
        ("throw", "throw.ok:6:9"),
        ("closure", "closure.ok:5:33"),
        ("region", "region.ok:6:22"),
    ] {
        for passes in ["all", "none"] {
            let output = Command::new(env!("CARGO_BIN_EXE_okkama"))
                .arg("run")
                .arg(format!("{program}.ok"))
                .current_dir(&directory)
                .env("OKKAMA_IR_PASSES", passes)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains(&format!("--> {place}")), "{program}, passes {passes}: {stderr}");
        }
    }
}
