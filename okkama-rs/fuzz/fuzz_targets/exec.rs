//! Programs that compile run on the Sparrow core — machine code — under
//! budgets and with no host access: every outcome is a result, a failure
//! report or an exhausted budget, never a crash.
#![no_main]
use libfuzzer_sys::fuzz_target;
use okkama_compiler::SourceFile;
use okkama_compiler::syntax::{self, ParseOptions, prune};
use sparrow_core::budget::{Counted, Limits, counted_bytes, set_memory_probe};

#[global_allocator]
static ALLOCATOR: Counted<std::alloc::System> = Counted(std::alloc::System);

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let Ok(source) = SourceFile::new("fuzz.ok", text) else { return };
    let Ok(program) = syntax::parse(&source, &ParseOptions::default()) else { return };
    let Ok(module) = okkama_compiler::lowering::lower_program(&prune::strip_test_items(program)) else { return };
    let Ok(program) = okkama_compiler::codegen::sparrow::generate(&module) else { return };
    let Ok(bytes) = okkama_sparrow::encode(&program) else { return };
    set_memory_probe(counted_bytes);
    let limits = Limits {
        steps: Some(1_000_000),
        memory: Some(64 << 20),
        output: Some(4096),
        // Pure computation only: no files, network, processes or environment.
        natives: Some(Vec::new()),
        ..Limits::default()
    };
    let _ = sparrow_core::run_limited(bytes, Vec::new(), false, Vec::new(), limits);
});
