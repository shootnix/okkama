//! #535: public loading path, including the off/on environment switch.
//! In-process assembly/execution only; run this filter with GC stress too.

fn encode(source: &str) -> Vec<u8> {
    okkama_sparrow::encode(&okkama_sparrow::assemble(source).unwrap()).unwrap()
}

#[test]
fn pointer_free_sync_public_loader() {
    assert_eq!(sparrow_core::run(encode(include_str!("pointer_free.spa"))).unwrap(), 42);
}

#[test]
fn pointer_free_async_public_loader() {
    let source = include_str!("all_futures.spa").replace("bool true", "bool false");
    assert!(sparrow_core::run_tests(encode(&source), Box::new(std::io::sink())).unwrap());
}

// #539: in-process public loader; repeat in all proof modes and with GC stress.
#[test]
fn int_range_public_loader_preserves_exact_operation_results() {
    let program = okkama_sparrow::assemble(include_str!("int_range.spa")).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(sparrow_core::run(bytes).unwrap(), 42);
}
