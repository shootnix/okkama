//! Load-time contracts of generic intrinsics (#69): a crafted SPB that pairs
//! channel endpoints of different payload types, or a scope whose action
//! yields another type than its result, is refused by `sparrow check`; the
//! compiler's own SPB is accepted.

use okkama_sparrow::{Type, TypeRef};
use std::path::Path;
use std::process::Command;

fn compiled(case: &str) -> okkama_sparrow::Program {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source = root.join(format!("conformance/cases/{case}/main.ok"));
    let out = std::env::temp_dir().join(format!("intrinsic-contracts-{}.spb", std::process::id()));
    let built = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["build", "--target", "sparrow"])
        .arg(&source)
        .arg("-o")
        .arg(&out)
        .env("OKKAMA_STDLIB_DIR", root.join("std"))
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let program = okkama_sparrow::decode(&std::fs::read(&out).unwrap()).unwrap();
    let _ = std::fs::remove_file(out);
    program
}

fn check(program: &okkama_sparrow::Program) -> Result<(), String> {
    sparrow_core::check(&okkama_sparrow::encode(program).unwrap()).map_err(|error| error.0)
}

/// A copy of the intrinsic function `name`, never called, whose result is
/// changed to an array of what it was: a different reference type.
fn with_retyped_copy(mut program: okkama_sparrow::Program, name: &str) -> okkama_sparrow::Program {
    let original = program
        .functions
        .iter()
        .find(|f| f.intrinsic.as_ref().is_some_and(|i| i.name == name))
        .unwrap_or_else(|| panic!("no {name}"))
        .clone();
    let result = original.result.expect("a result");
    program.types.push(Type::Array(result));
    let mut copy = original;
    copy.symbol.push_str("$crafted");
    copy.result = Some(TypeRef(program.types.len() as u32 - 1));
    program.functions.push(copy);
    program
}

#[test]
fn endpoints_of_one_channel_carry_one_payload_type() {
    let program = compiled("L08-async-and-concurrency/channel.close-semantics");
    check(&program).expect("the compiler's SPB is accepted");
    let crafted = with_retyped_copy(program, "okkama.channel.Receiving.item");
    let refused = check(&crafted).expect_err("a receiving endpoint of another payload type");
    assert!(refused.contains("channel endpoints carry different payload types"), "{refused}");
}

#[test]
fn scopes_and_races_yield_the_type_they_wait_for() {
    let program = compiled("L08-async-and-concurrency/channel.close-semantics");
    if program.functions.iter().any(|f| f.intrinsic.as_ref().is_some_and(|i| i.name == "okkama.scope.FutureScope.result")) {
        let refused = check(&with_retyped_copy(program, "okkama.scope.FutureScope.result")).expect_err("a scope of another result type");
        assert!(refused.contains("scope action does not take the group or yield the result"), "{refused}");
    }
    let program = compiled("L08-async-and-concurrency/collect.race-empty");
    check(&program).expect("the compiler's SPB is accepted");
    let refused = check(&with_retyped_copy(program, "okkama.collect.Raced.result")).expect_err("a race of another result type");
    assert!(refused.contains("result does not match"), "{refused}");
}
