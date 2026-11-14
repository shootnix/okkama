//! #581 regression: unique temporary takes and shared cache borrows on Sparrow.
use okkama_compiler::{SourceFile, ir, lowering, syntax};
use std::path::PathBuf;

struct Stdlib(PathBuf);
impl lowering::ModuleLoader for Stdlib {
    fn load(&mut self, path: &str, _importer: &str) -> Result<Option<lowering::LoadedModule>, String> {
        let file = [self.0.join(format!("{path}.ok")), self.0.join("../t/support").join(format!("{path}.ok"))]
            .into_iter().find(|file| file.is_file());
        let Some(file) = file else { return Ok(None); };
        let text = std::fs::read_to_string(&file).map_err(|error| error.to_string())?;
        let id = file.to_string_lossy().into_owned();
        let source = SourceFile::new(id.clone(), text).unwrap();
        let program = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
        Ok(Some(lowering::LoadedModule { id, source, program: syntax::prune::strip_test_items(program) }))
    }
}

fn compile(body: &str) -> okkama_sparrow::Program {
    let text = format!(r#"
        module app v1.0.0;
        import "async/atomic" take Atomic;
        import "async/event" take Event;
        import "async/channel" take Channel;
        import "async/collect" take AllFutures, FutureCompletions;
        import "async/scope" take FutureGroup;
        import "db/sqlite" take SqliteMemory;
        object Res {{
            has name is string;
            has log is Atomic<string>;
            Res(:name, :log);
            label() is string {{ confirm log.value() == ""; name; }}
            destroy {{ log.replace(log.value() + name); }}
        }}
        enum Stop is Error = (now);
        object AsyncRes {{
            has name is string;
            has log is Atomic<string>;
            AsyncRes(:name, :log);
            async destroy {{ await Event(ready: true).wait(); log.replace(log.value() + name); }}
        }}
        object Bad {{
            has name is string;
            has log is Atomic<string>;
            Bad(:name, :log);
            destroy {{ log.replace(log.value() + name); throw Stop.now; }}
        }}
        object Holder {{
            has resource is Res;
            Holder(:resource);
            label() is string {{ resource.label(); }}
        }}
        object Work {{
            Work();
            observe(value is Res) is void {{ value.label(); }}
            async opening(gate is Event, ready is Event) is void {{ await ready.wait(); gate.set(); }}
            async server(log is Atomic<string>) is Holder {{
                let listener = await self.make("A", log);
                Holder(resource: listener);
            }}
            async make(name is string, log is Atomic<string>) is Res {{ Res(name: name, log: log); }}
            async borrow(value is Res) is Res {{ value; }}
            async waiting(name is string, log is Atomic<string>, gate is Event) is Res {{
                await gate.wait(); Res(name: name, log: log);
            }}
            async nested(log is Atomic<string>) is Future<Res> {{ self.make("N", log); }}
            async asynchronous(name is string, log is Atomic<string>) is AsyncRes {{ AsyncRes(name: name, log: log); }}
            async bad(name is string, log is Atomic<string>) is Bad {{ Bad(name: name, log: log); }}
            async failed() is Res {{ throw Stop.now; }}
            async receive(channel is Channel<Res>) is Res {{ await channel.receiving().item(); }}
        }}
        async main() is int {{
            let log = Atomic("");
            {body}
            0;
        }}
    "#);
    let source = SourceFile::new("future-outcomes.ok", text).unwrap();
    compile_source(&source)
}

fn compile_source(source: &SourceFile) -> okkama_sparrow::Program {
    let program = syntax::parse(source, &syntax::ParseOptions::default()).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../std");
    let module = lowering::lower_linked(&program, source.name(), &mut Stdlib(root)).unwrap();
    ir::verify(&module).unwrap();
    okkama_compiler::codegen::sparrow::generate(&module).unwrap()
}

fn run(program: &okkama_sparrow::Program) -> Result<i64, sparrow_core::Error> {
    sparrow_core::run(okkama_sparrow::encode(program).unwrap())
}

fn check(body: &str) {
    assert_eq!(run(&compile(body)).unwrap(), 0);
}

/// Reproduce the original #581 behavior without another binary or engine.
fn check_regression(body: &str, expected_failure: &str) {
    let mut program = compile(body);
    assert_eq!(run(&program).unwrap(), 0);
    for instruction in program.functions.iter_mut().flat_map(|function| &mut function.code) {
        if let okkama_sparrow::Instruction::AwaitTake { dst, future } = *instruction {
            *instruction = okkama_sparrow::Instruction::Await { dst, future };
        }
    }
    let error = run(&program).expect_err("borrow-only baseline must expose the early close");
    assert!(error.0.contains(expected_failure), "{error}");
}

#[test]
fn future_outcome_nested_awaiters_borrow() {
    check(r#"
        {
            let future = Work().make("A", log);
            { let value = await future; confirm value.label() == "A"; }
            confirm log.value() == "";
            { (await future).label(); }
            confirm log.value() == "";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_discarded_temporary_and_same_scope_track() {
    check(r#"
        { await Work().make("A", log); confirm log.value() == ""; }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_escape_requires_future_not_borrowed_value() {
    check(r#"
        let borrowed = if true {
            let future = Work().make("A", log);
            await future;
        } else { Res(name: "wrong", log: log); };
        // Do not dereference the expired borrow.
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_escaping_future_keeps_cache() {
    check(r#"
        {
            let future = if true {
                let inner = Work().make("A", log);
                { (await inner).label(); }
                inner;
            } else { Work().make("wrong", log); };
            confirm log.value() == "";
            (await future).label();
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_global_creation_order() {
    check(r#"
        {
            let b = Work().make("B", log);
            let a = Work().make("A", log);
            await a;
            let c = Res(name: "C", log: log);
            await b;
            confirm log.value() == "";
        }
        confirm log.value() == "BCA";
    "#);
}

#[test]
fn future_outcome_borrowed_input_keeps_original_owner() {
    check(r#"
        {
            let resource = Res(name: "A", log: log);
            {
                let a = Work().borrow(resource);
                let b = Work().borrow(resource);
                await a; await b;
            }
            confirm log.value() == "";
            resource.label();
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_nested_future_owned_transitively() {
    check(r#"
        {
            let outer = Work().nested(log);
            { let inner = await outer; (await inner).label(); }
            confirm log.value() == "";
        }
        confirm log.value() == "N";
    "#);
}

#[test]
fn future_outcome_suspended_await_and_cached_failure() {
    check(r#"
        {
            let gate = Event();
            let future = spawn Work().waiting("A", log, gate);
            gate.set();
            { (await future).label(); }
            { (await future).label(); }
            let failed = Work().failed();
            try { await failed; } catch e is Stop { e.text(); }
            try { await failed; } catch e is Stop { e.text(); }
            confirm log.value() == "";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_all_futures_duplicates_and_nested_consumers() {
    check(r#"
        {
            let collector is AllFutures<Res> = AllFutures(origins: [Work().make("A", log), Work().make("B", log)]);
            { let results = await collector.results(); results.item(0).label(); }
            confirm log.value() == "";
            { let results = await collector.results(); results.item(1).label(); }
            confirm log.value() == "";
        }
        confirm log.value() == "BA";
        log.replace("");
        {
            let a = Work().make("A", log);
            let b = Work().make("B", log);
            {
                let collector is AllFutures<Res> = AllFutures(origins: [a, b, a]);
                let results = await collector.results();
                results.item(0).label(); results.item(2).label();
            }
            confirm log.value() == "";
        }
        confirm log.value() == "BA";
    "#);
}

#[test]
fn future_outcome_completions_duplicates_preserve_identity() {
    check(r#"
        {
            let first_gate = Event();
            let second_gate = Event();
            let a = Work().waiting("A", log, first_gate);
            let b = Work().waiting("B", log, second_gate);
            let stream is FutureCompletions<Res> = FutureCompletions(origins: [a, b, a]);
            first_gate.set();
            { let first = await stream.completion(); (await first).label(); }
            confirm log.value() == "";
            { let duplicate = await stream.completion(); (await duplicate).label(); }
            confirm log.value() == "";
            second_gate.set();
            { let second = await stream.completion(); (await second).label(); }
            confirm log.value() == "";
        }
        confirm log.value() == "BA";
    "#);
}

#[test]
fn future_outcome_async_destroy_order() {
    check(r#"
        {
            let a = Work().asynchronous("A", log);
            let b = Work().asynchronous("B", log);
            { await a; await b; }
            confirm log.value() == "";
        }
        confirm log.value() == "BA";
    "#);
}

#[test]
fn future_outcome_cleanup_failure_does_not_repeat_or_skip() {
    check(r#"
        try {
            let a = Work().bad("A", log);
            let b = Work().bad("B", log);
            { await a; await b; }
            confirm log.value() == "";
        } catch e is Stop { e.text(); }
        confirm log.value() == "BA";
    "#);
}

#[test]
fn future_outcome_temporary_collector_owns_inputs() {
    check(r#"
        {
            let results = await AllFutures(origins: [Work().make("A", log), Work().make("B", log)]).results();
            results.item(0).label(); results.item(1).label();
            confirm log.value() == "";
        }
        confirm log.value() == "BA";
    "#);
}

#[test]
fn future_outcome_returned_channel_take_escapes_helper() {
    check(r#"
        {
            let channel is Channel<Res> = Channel(capacity: 1);
            await channel.sending().send(Res(name: "R", log: log));
            { let value = await Work().receive(channel); confirm value.label() == "R"; }
            confirm log.value() == "R";
            channel.sending().close();
        }
        confirm log.value() == "R";
    "#);
}

#[test]
fn future_outcome_escaping_collector_future_retains_fresh_receiver() {
    check(r#"
        {
            let future = if true {
                let collector is AllFutures<Res> = AllFutures(origins: [Work().make("A", log), Work().make("B", log)]);
                collector.results();
            } else { AllFutures(origins: [Work().make("wrong", log)]).results(); };
            let results = await future;
            results.item(0).label(); results.item(1).label();
        }
        confirm log.value() == "BA";
        log.replace("");
        {
            let future = if true {
                let collector is AllFutures<Res> = AllFutures(origins: [Work().make("A", log), Work().make("B", log)]);
                let result = collector.results();
                await result;
                result;
            } else { AllFutures(origins: [Work().make("wrong", log)]).results(); };
            confirm log.value() == "";
            let results = await future;
            results.item(0).label(); results.item(1).label();
        }
        confirm log.value() == "BA";
    "#);
}

#[test]
fn future_outcome_temporary_take_escapes_producer_and_binding_scope() {
    check(r#"
        {
            let value = if true { await Work().make("A", log); }
                else { await Work().make("wrong", log); };
            confirm value.label() == "A";
            confirm log.value() == "";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_bound_once_stays_at_original_boundary() {
    check(r#"
        {
            let future = Work().make("A", log);
            { let value = await future; confirm value.label() == "A"; }
            confirm log.value() == "";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_temporary_take_borrowed_input_keeps_owner() {
    check(r#"
        {
            let resource = Res(name: "A", log: log);
            { let value = await Work().borrow(resource); confirm value.label() == "A"; }
            confirm resource.label() == "A";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_taken_argument_has_fresh_call_cleanup() {
    check(r#"
        Work().observe(await Work().make("A", log));
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_suspended_temporary_take() {
    check(r#"
        {
            let gate = Event();
            let ready = Event();
            spawn Work().opening(gate, ready);
            ready.set();
            let value = if true { await Work().waiting("A", log, gate); }
                else { await Work().make("wrong", log); };
            confirm value.label() == "A";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_temporary_async_destroy_and_failure_cleanup() {
    check(r#"
        { await Work().asynchronous("A", log); confirm log.value() == ""; }
        confirm log.value() == "A";
        log.replace("");
        try { await Work().bad("B", log); confirm log.value() == ""; }
            catch e is Stop { e.text(); }
        confirm log.value() == "B";
        try { await Work().failed(); } catch e is Stop { e.text(); }
        confirm log.value() == "B";
    "#);
}

#[test]
fn future_outcome_sqlite_temporary_connection_survives_helper() {
    check_regression(r#"
        let db = await SqliteMemory().connection();
        defer await db.settled();
        defer db.close();
        await db.execution("CREATE TABLE t(n INTEGER)");
        await db.execution("INSERT INTO t VALUES (42)");
        let row = await db.query("SELECT n FROM t").one();
        confirm row.int("n") == 42;
    "#, "DbClosed");
}

#[test]
fn future_outcome_group_and_collection_aliases_borrow() {
    check(r#"
        {
            let future = Work().make("A", log);
            let group is FutureGroup<Res> = FutureGroup();
            group.start(future);
            group.start(future);
            let futures = [future, future];
            { (await futures.item(0)).label(); }
            { (await future).label(); }
            await group.wait();
            confirm log.value() == "";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_web_server_listener_model() {
    check_regression(r#"
        {
            let server = await Work().server(log);
            confirm server.label() == "A";
            confirm log.value() == "";
        }
        confirm log.value() == "A";
    "#, "confirm failed");
}

#[test]
fn future_outcome_action_value_temporary_take() {
    check(r#"
        {
            let action is async (string, Atomic<string>) is Res = Work().make;
            let value = if true { await action("A", log); }
                else { await Work().make("wrong", log); };
            confirm value.label() == "A";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_bound_argument_borrows_at_call_boundary() {
    check(r#"
        {
            let future = Work().make("A", log);
            { Work().observe(await future); }
            confirm log.value() == "";
        }
        confirm log.value() == "A";
    "#);
}

#[test]
fn future_outcome_modified_corpus_expectations() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tests/ownership-corpus");
    for name in ["async-results", "channel-transfer"] {
        let path = root.join(format!("{name}.ok"));
        let source = SourceFile::new(path.to_string_lossy(), std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(run(&compile_source(&source)).unwrap(), 0, "{name}");
    }
}
