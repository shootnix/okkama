//! #578: retained dependencies follow a cold Future through scope exits.
#[path = "sparrow_support/mod.rs"]
mod support;
use okkama_compiler::{SourceFile, ir, lowering, syntax};

fn module(body: &str) -> ir::Module {
    // The intrinsic Atomic declaration keeps these tests entirely in-process.
    let text = format!(r#"
        module async/atomic v1.0.0;
        object Atomic<T> {{
            Atomic(value is T) {{}}
            value() is T {{ panic "intrinsic"; }}
            replace(value is T) is void {{ panic "intrinsic"; }}
        }}
        enum AtomicClosed is Error = (closed);
        object Res {{
            has name is string;
            has log is Atomic<string>;
            Res(name is string, log is Atomic<string>) {{ self.name = name; self.log = log; }}
            label() is string {{ confirm log.value() == ""; name; }}
            async read(arg is Res) is string {{
                let text = self.label() + arg.label();
                // A later Res can reuse an input register, but is not retained input.
                Res("", log);
                text;
            }}
            destroy {{ log.replace(log.value() + name); }}
        }}
        object Box {{
            has cold is Future<string>;
            Box(cold is Future<string>) {{ self.cold = cold; }}
            get() is Future<string> {{ cold; }}
        }}
        object Factory {{
            Factory();
            async create(log is Atomic<string>) is Res {{ Res("V", log); }}
            make(log is Atomic<string>) is Future<string> {{
                Res("R", log).read(Res("A", log));
            }}
            borrowed(receiver is Res, arg is Res) is Future<string> {{ receiver.read(arg); }}
            boxed(log is Atomic<string>) is Box {{ Box(self.make(log)); }}
        }}
        async main() is int {{
            let log = Atomic("");
            {body}
            0;
        }}
    "#);
    let source = SourceFile::new("cold-futures.ok", text).unwrap();
    let program = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    let module = lowering::lower_program(&program).unwrap();
    ir::verify(&module).unwrap();
    module
}

fn check(body: &str) {
    let module = module(body);
    let sparrow = support::sparrow(&module);
    assert_eq!(sparrow, support::Ending::Value(0));
}

#[test]
fn cold_future_never_started_transfers_fresh_dependencies() {
    check(r#"
        { let cold = Factory().make(log); confirm log.value() == ""; }
        confirm log.value() == "AR";
    "#);
}

#[test]
fn cold_future_cancelled_before_start_retains_dependencies() {
    check(r#"
        {
            let cold = Factory().make(log);
            cold.cancel();
            confirm log.value() == "";
            try { await cold; } catch e is Cancelled { e.text(); }
            confirm log.value() == "";
        }
        confirm log.value() == "AR";
    "#);
}

#[test]
fn cold_future_awaited_twice_keeps_dependencies_until_owner_exits() {
    check(r#"
        {
            let cold = Factory().make(log);
            confirm log.value() == "";
            confirm await cold == "RA";
            confirm await cold == "RA";
            confirm log.value() == "";
        }
        confirm log.value() == "AR";
    "#);
}

#[test]
fn cold_future_borrowed_dependencies_keep_outer_owner() {
    check(r#"
        {
            let receiver = Res("R", log);
            let arg = Res("A", log);
            {
                let cold = Factory().borrowed(receiver, arg);
                confirm await cold == "RA";
            }
            confirm log.value() == "";
        }
        confirm log.value() == "AR";
    "#);
}

#[test]
fn cold_future_in_object_transfers_dependencies() {
    check(r#"
        { let boxed = Factory().boxed(log); confirm log.value() == ""; }
        confirm log.value() == "AR";
    "#);
}

#[test]
fn cold_future_completed_then_escaped_retains_original_arguments() {
    check(r#"
        {
            let cold = if true {
                let inner = Factory().make(log);
                confirm await inner == "RA";
                inner;
            } else { Factory().make(log); };
            confirm log.value() == "";
            confirm await cold == "RA";
        }
        confirm log.value() == "AR";
    "#);
}

#[test]
fn cold_future_repeated_dependency_is_destroyed_once() {
    check(r#"
        {
            let receiver = Res("R", log);
            {
                let cold = Factory().borrowed(receiver, receiver);
                confirm await cold == "RR";
            }
            confirm log.value() == "";
        }
        confirm log.value() == "R";
    "#);
}

#[test]
fn cold_future_cached_result_escapes_with_future() {
    check(r#"
        {
            let cold = if true {
                let inner = Factory().create(log);
                let value = await inner;
                confirm value.label() == "V";
                inner;
            } else { Factory().create(log); };
            confirm log.value() == "";
        }
        confirm log.value() == "V";
    "#);
}
