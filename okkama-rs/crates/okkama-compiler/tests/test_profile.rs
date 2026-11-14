//! Test state uses ordinary containers (#583). Compile only: never run programs.

use std::path::{Path, PathBuf};

use okkama_compiler::SourceFile;
use okkama_compiler::ir;
use okkama_compiler::lowering::{self, LoadedModule, ModuleLoader, RootFile};
use okkama_compiler::syntax::{self, ParseOptions, prune};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

struct Std;

impl ModuleLoader for Std {
    fn load(&mut self, path: &str, _importer: &str) -> Result<Option<LoadedModule>, String> {
        let file = root().join("std").join(format!("{path}.ok"));
        let text = std::fs::read_to_string(&file).map_err(|error| error.to_string())?;
        let source = SourceFile::new(file.to_string_lossy(), text).map_err(|error| error.to_string())?;
        let program = syntax::parse(&source, &ParseOptions::default()).map_err(|error| format!("{error:?}"))?;
        Ok(Some(LoadedModule {
            id: source.name().to_string(), source, program: prune::desugar_test_items(program),
        }))
    }

    fn resolve(&mut self, path: &str, _importer: &str) -> Result<Option<String>, String> {
        Ok(Some(root().join("std").join(format!("{path}.ok")).to_string_lossy().into_owned()))
    }
}

fn lower(text: &str, name: &str) -> Result<ir::Module, String> {
    let source = SourceFile::new(name, text).unwrap();
    let program = syntax::parse(&source, &ParseOptions::default()).map_err(|error| format!("{error:?}"))?;
    let program = prune::desugar_test_items(program);
    lowering::lower_linked_tests(&[RootFile { program: &program, source: &source }], &mut Std)
        .map(|(module, _)| module).map_err(|error| format!("{error:?}"))
}

#[test]
fn migrated_fakes_and_their_inline_tests_lower_and_verify() {
    for path in [
        "std/archive/stream.ok",
        "std/compression/read.ok",
        "std/compression/state.ok",
        "conformance/cases/L04-objects-roles-caps-successors/fakes.failing-assertion/main.ok",
        "conformance/cases/L04-objects-roles-caps-successors/fakes.production-rejected/main.ok",
    ] {
        let text = std::fs::read_to_string(root().join(path)).unwrap();
        let module = lower(&text, path).unwrap_or_else(|error| panic!("{path}: {error}"));
        ir::verify(&module).unwrap_or_else(|error| panic!("{path}: {error:?}"));
        assert!(module.tests.is_some(), "{path}");
    }
}

#[test]
fn removed_storage_calls_are_ordinary_unresolved_names() {
    for call in ["set(1, 2)", "get(1)", "has(1)", "remove(1)", "clear()", "size()"] {
        let text = format!("module app v1.0.0; role Counter {{ count() is int; fake {{
            count() is int {{ fake.storage.{call}; 0; }}
        }} }} test \"counter\" {{ test.is(Counter.fake().count(), 0); }}");
        let error = lower(&text, "removed.ok").expect_err(call);
        assert!(error.contains("Undefined variable") && error.contains("fake"), "{call}: {error}");
    }
}

#[test]
fn cell_locals_and_atomic_fake_fields_use_the_ordinary_pipeline() {
    let path = "conformance/cases/L04-objects-roles-caps-successors/fakes.typed-state/main.ok";
    let text = std::fs::read_to_string(root().join(path)).unwrap();
    let module = lower(&text, path).unwrap();
    ir::verify(&module).unwrap();
}

#[test]
fn removed_intrinsics_are_rejected_at_the_oir_boundary() {
    let original = lower("module app v1.0.0; test \"ok\" { test.ok(true); }", "boundary.ok").unwrap();
    for operation in ["set", "get", "has", "remove", "clear", "size"] {
        let mut module = original.clone();
        let name = module.functions.iter_mut().flat_map(|function| &mut function.attrs)
            .find_map(|attr| match attr {
                ir::Attr::Intrinsic { name, .. } => Some(name),
                _ => None,
            }).expect("test assertion intrinsic");
        *name = format!("okkama.fake.storage.{operation}");
        let expected = format!("unknown intrinsic \"{name}\"");
        let error = ir::verify(&module).expect_err(operation);
        assert!(error.to_string().contains(&expected), "{error}");
    }
}
