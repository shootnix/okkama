//! Compiler phases on very large functions (#50): the whole pipeline — parse,
//! analysis, lowering, IR passes, Sparrow codegen with its verification —
//! grows linearly with the size of one function. Four times the instructions
//! may cost at most eight times the time (linear gives four, quadratic
//! sixteen), whatever the build profile.

use okkama_compiler::SourceFile;
use okkama_compiler::codegen;
use okkama_compiler::lowering::lower_program;
use okkama_compiler::syntax::{self, ParseOptions, prune};
use std::time::{Duration, Instant};

fn lets(n: usize) -> String {
    let body: Vec<String> = std::iter::once("    let v0 = 1;".to_string())
        .chain((1..n).map(|i| format!("    let v{i} = v{} + {};", i - 1, i % 7)))
        .collect();
    format!("module app v1.0.0;\nmain() is int {{\n{}\n    v{} % 256;\n}}\n", body.join("\n"), n - 1)
}

fn branches(n: usize) -> String {
    let body: Vec<String> = (0..n).map(|i| format!("    if x == {i} {{ return {}; }}", i % 200)).collect();
    format!("module app v1.0.0;\nmain() is int {{\n    let x = 3;\n{}\n    0;\n}}\n", body.join("\n"))
}

fn literal(n: usize) -> String {
    let items: Vec<String> = (0..n).map(|i| (i % 97).to_string()).collect();
    format!("module app v1.0.0;\nmain() is int {{\n    let items = [{}];\n    items.size() % 256;\n}}\n", items.join(", "))
}

/// The best of three compilations of `text` to Sparrow.
fn compile(text: &str) -> Duration {
    (0..3)
        .map(|_| {
            let start = Instant::now();
            let source = SourceFile::new("large.ok", text).unwrap();
            let program = syntax::parse(&source, &ParseOptions::default()).unwrap();
            let module = lower_program(&prune::strip_test_items(program)).unwrap();
            codegen::sparrow::generate(&module).unwrap();
            start.elapsed()
        })
        .min()
        .unwrap()
}

#[test]
fn one_large_function_compiles_in_linear_time() {
    // Debug builds also run the independent register check after codegen
    // (`codegen/sparrow/check.rs`), a deliberately simple dataflow over
    // whole register files; the release gate measures the compiler itself.
    if cfg!(debug_assertions) {
        return;
    }
    for (name, shape) in [("lets", lets as fn(usize) -> String), ("branches", branches), ("literal", literal)] {
        let (small, large) = (compile(&shape(5_000)), compile(&shape(20_000)));
        let growth = large.as_secs_f64() / small.as_secs_f64().max(1e-4);
        assert!(growth < 8.0, "{name}: 5 000 → 20 000 took {small:?} → {large:?} (×{growth:.1})");
    }
}
