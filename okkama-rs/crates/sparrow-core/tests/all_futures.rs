//! #580: the collected result graph belongs to the awaiting lexical scope.
//! Assemble in process, as in compile_cost.rs; no compiler or child processes.

fn check_all_futures(ready: bool) {
    let source = include_str!("all_futures.spa").replace("bool true", &format!("bool {ready}"));
    let program = okkama_sparrow::assemble(&source).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert!(sparrow_core::run_tests(bytes, Box::new(std::io::stdout())).unwrap());
}

#[test]
fn all_futures_retains_duplicate_results_until_scope_exit() {
    check_all_futures(true);
}

#[test]
fn all_futures_transfers_results_after_suspension() {
    check_all_futures(false);
}
