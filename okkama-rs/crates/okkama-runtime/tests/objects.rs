//! Объекты проходят весь путь от исходника до исполнения методов в Rust.
use okkama_compiler::{SourceFile, ir, lowering, syntax};
use okkama_runtime::{RuntimeError, execute};

fn result(module: &ir::Module) -> ir::IntValue {
    let result = execute(module).unwrap();
    let program = okkama_compiler::codegen::sparrow::generate(module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(
        okkama_runtime::sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()).unwrap(),
        result
    );
    result
}

fn lower(text: &str) -> ir::Module {
    let source = SourceFile::new("objects.ok", format!("module app v1.0.0; {text}")).unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    lowering::lower_program(&ast).unwrap()
}

#[test]
fn constructors_defaults_and_methods_execute_from_source() {
    let module = lower(
        r#"
        object Counter {
            has n is int = 20;
            Counter() {}
            Counter(n is int) { n = n; }
            value() is int { n; }
            plus(delta is int) is int { self.value() + delta; }
        }
        main() is int {
            let a = Counter();
            let b = Counter(2);
            a.plus(b.value());
        }
    "#,
    );
    assert_eq!(result(&module).to_string(), "22");
    assert_eq!(result(&module).to_string(), "22");
}

#[test]
fn nested_objects_and_returned_receivers_survive_their_call_frames() {
    let module = lower(
        r#"
        object Number {
            has n is int;
            Number(:n);
            value() is int { n; }
        }
        object Box {
            has inner is Number;
            Box(:inner);
            get() is Number { inner; }
        }
        object Factory {
            Factory();
            make(n is int) is Box { Box(inner: Number(n: n)); }
            same(b is Box) is Box { b; }
        }
        main() is int {
            let f = Factory();
            let a = f.make(40);
            let b = f.same(a);
            b.get().value() + f.make(2).get().value();
        }
    "#,
    );
    assert_eq!(result(&module).to_string(), "42");
}

#[test]
fn methods_can_branch_and_recurse_with_the_same_receiver() {
    let module = lower(
        r#"
        object Counter {
            has limit is int;
            Counter(:limit);
            count(n is int) is int {
                if n < limit { self.count(n + 1) + 1; } else { 0; }
            }
        }
        main() is int { Counter(limit: 1000).count(0); }
    "#,
    );
    assert_eq!(result(&module).to_string(), "1000");
}

#[test]
fn generic_objects_are_already_specialized_by_lowering() {
    let module = lower(
        r#"
        object Box<T> {
            has item is T;
            Box(:item);
            get() is T { item; }
        }
        main() is int {
            let n is Box<int> = Box(item: 42);
            let b is Box<bool> = Box(item: true);
            if b.get() { n.get(); } else { 0; }
        }
    "#,
    );
    assert_eq!(result(&module).to_string(), "42");
}

#[test]
fn destroy_runs_when_the_owning_region_closes() {
    let module = lower(
        r#"
        object Resource {
            Resource();
            destroy { panic "closed"; }
            value() is int { 42; }
        }
        main() is int { Resource().value(); }
    "#,
    );
    assert!(matches!(
        execute(&module),
        Err(RuntimeError::Panic { message, .. }) if message == "closed"
    ));
}
