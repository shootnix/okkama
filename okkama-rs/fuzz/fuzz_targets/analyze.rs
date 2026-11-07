//! Semantic analysis and lowering of any parsed program end in OIR or
//! diagnostics; produced OIR verifies.
#![no_main]
use libfuzzer_sys::fuzz_target;
use okkama_compiler::SourceFile;
use okkama_compiler::syntax::{self, ParseOptions, prune};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let Ok(source) = SourceFile::new("fuzz.ok", text) else { return };
    let Ok(program) = syntax::parse(&source, &ParseOptions::default()) else { return };
    if let Ok(module) = okkama_compiler::lowering::lower_program(&prune::strip_test_items(program)) {
        okkama_compiler::ir::verify(&module).expect("lowering produced invalid OIR");
    }
});
