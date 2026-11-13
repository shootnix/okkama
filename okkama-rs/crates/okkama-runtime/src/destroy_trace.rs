//! Sparrow-compatible destruction traces (#561).
//!
//! Allocation audit (#561 follow-up):
//! - Sparrow `jit.rs`: ObjNew, ObjNext and VariantNew call the JIT's
//!   `created`, which emits `failures::rt_created` -> `Runtime::created`.
//!   `drivers::collect_finish` calls `Runtime::created` directly for the
//!   AllFutures sequence, after its array is allocated, including empty input.
//! - IR ObjNew and VariantNew use `State::object_new` -> `allocate_object`;
//!   ObjNext uses `State::object_next` and registers the successful successor.
//!   `IrHost::make` uses the same `allocate_object`, including `Host::thrown`.
//! - All core intrinsic object allocations use Host::make: AllFutures (empty
//!   and successful nonempty results), channel/endpoints, event, atomic,
//!   cancellation/token, future group/scope, lock/grant/semaphore permit,
//!   barrier snapshot, and runtime-raised errors via Host::thrown. Cached
//!   futures and forwarded results do not create another object identity.
//! - Sparrow's other object_new sites are intrinsics::call's `new`, `raised`,
//!   driver grants/groups, native failure construction and caught Cancelled.
//!   Their standard descriptors have no destroy-reachable fields: they do not
//!   call created and consume no serial. IR's descriptor filter likewise
//!   ignores these, even when hidden intrinsic state holds resource values.
//! - ObjectHeap::allocate -> Object::new is the core object allocator;
//!   ObjectHeap::successor delegates to it. The in-place unique_fields and
//!   successor_owned helpers are not used by the IR interpreter.
//! - sched::cancelled_failure directly constructs the descriptorless CANCELLED
//!   sentinel, which cannot reach a destroy. REPL remap allocates copies before
//!   an engine exists, transporting prior-cell values rather than executing
//!   creation instructions (Sparrow has no corresponding REPL path).
//! - Arrays/maps/builders, closures, futures, native handles and scheduler/GC
//!   storage have no object descriptor and consume no identity serial. Worker
//!   decoding constructs only scalar/array/map values. Sparrow native_debug's
//!   ObjectHeap copies are diagnostic representations, not runtime identities.
//!
//! Registration belongs at creation, never when adopting or tracing a returned
//! value: that would miss discarded/empty containers and reorder identities.
use okkama_compiler::ir;
use okkama_runtime_core::heap::Object;
use std::collections::HashMap;
use std::io::Write;

#[derive(Debug, Default)]
pub(crate) struct DestroyTrace {
    file: Option<std::fs::File>,
    flags: Vec<bool>,
    serial: u64,
    identities: HashMap<u64, u64>,
}

impl DestroyTrace {
    pub(crate) fn new(module: &ir::Module) -> Self {
        let file = std::env::var_os("OKKAMA_TRACE_DESTROY")
            .and_then(|path| std::fs::OpenOptions::new().create(true).append(true).open(path).ok());
        let flags = if file.is_some() { descriptor_flags(module) } else { Vec::new() };
        Self { file, flags, ..Self::default() }
    }

    pub(crate) fn created(&mut self, object: &Object) {
        if self.flags.get(object.declaration) == Some(&true) {
            self.serial += 1;
            self.identities.insert(object.serial, self.serial);
        }
    }

    pub(crate) fn destroy(&mut self, object: &Object, name: &str) {
        if let Some(file) = &mut self.file
            && let Some(serial) = self.identities.remove(&object.serial)
        {
            let _ = writeln!(file, "{name} {serial}");
        }
    }
}

// The same closed-world fixed point as Sparrow's descriptor_flags (#561).
fn descriptor_flags(module: &ir::Module) -> Vec<bool> {
    let mut flags: Vec<_> = module.objects.iter().map(|object| object.destroy.is_some()).collect();
    if !flags.iter().any(|flag| *flag) {
        return flags;
    }
    loop {
        let mut changed = false;
        for (index, object) in module.objects.iter().enumerate() {
            if !flags[index] && object.fields.iter().any(|field| reaches(&field.ty, module, &flags)) {
                flags[index] = true;
                changed = true;
            }
        }
        if !changed {
            return flags;
        }
    }
}

fn reaches(ty: &ir::Type, module: &ir::Module, flags: &[bool]) -> bool {
    use ir::Type;
    match ty {
        Type::Object(id) | Type::Successor(id) => flags[id.0],
        Type::Array(element) | Type::Map(_, element) => reaches(element, module, flags),
        Type::Role(role) if !module.roles[role.0].builtin => module.objects.iter().enumerate()
            .any(|(index, object)| flags[index] && object.does.iter().any(|witness| witness.role == *role)),
        Type::Variant(variant) => module.variants[variant.0].cases.iter()
            .any(|(_, object)| flags[object.0]),
        Type::Int | Type::Bool | Type::Byte | Type::Char | Type::Float | Type::Rat
        | Type::Str | Type::Enum(_) | Type::Range | Type::Region | Type::Builder(_)
        | Type::MapBuilder(..) | Type::Handle(_) => false,
        Type::Role(_) | Type::Failure | Type::Never | Type::Any | Type::Fn(_)
        | Type::Future(_) | Type::Cap(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use okkama_compiler::{SourceFile, ir, lowering, syntax};

    fn lower(text: &str) -> ir::Module {
        let source = SourceFile::new("destroy_trace.ok", format!("module app v1.0.0; {text}")).unwrap();
        let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
        lowering::lower_program(&ast).unwrap()
    }

    #[test]
    fn trace_runtime_allocations() {
        use crate::{host::IrHost, interpreter::new_engine};
        use okkama_runtime_core::{Value, heap::ArrayHeap, intrinsics::{self, Host, HostRef}, sched};
        use std::rc::Rc;

        const CHILD: &str = "OKKAMA_TEST_RUNTIME_DESTROY_TRACE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let directory = std::env::temp_dir().join(format!("okkama-runtime-destroy-{}", std::process::id()));
            std::fs::create_dir(&directory).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "destroy_trace::tests::trace_runtime_allocations", "--nocapture"])
                .env(CHILD, "1")
                .env("OKKAMA_TRACE_DESTROY", directory.join("trace"))
                .output()
                .unwrap();
            std::fs::remove_dir_all(directory).unwrap();
            assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            return;
        }
        let path = std::env::var_os("OKKAMA_TRACE_DESTROY").unwrap();
        let module = lower("object Res { Res(); destroy {} } object Box { has items is @Res; Box(:items); } role Pending { size() is int; item(index is int) is Future<Res>; } object Origins does Pending { has count is int; has value is Future<Res>; Origins(:count, :value); size() is int { count; } item(index is int) is Future<Res> { value; } } object Inputs { has origins is Origins; Inputs(:origins); } object Plain { Plain(); } main() is int { 0; }");
        let id = |name: &str| module.objects.iter().position(|object| object.name == name).unwrap();
        let (res, container, inputs, plain) = (id("Res"), id("Box"), id("Inputs"), id("Plain"));
        let emit = |engine: &crate::interpreter::Engine, value: &Value| {
            let Value::Object(object) = value else { panic!("expected object") };
            engine.state.trace_destroy(object, "Res");
        };

        // Mixed allocation paths share one counter; descriptors, not payloads,
        // decide membership. Aliases do not count; each engine starts afresh.
        std::fs::write(&path, "").unwrap();
        for _ in 0..2 {
            let engine = new_engine(&module);
            let host = IrHost(engine.clone());
            host.make(plain, vec![]);
            let first = host.make(res, vec![]); // 1
            let alias = first.clone();
            host.make(container, vec![ArrayHeap.allocate(vec![])]); // 2
            let second = engine.state.object_new(ir::ObjectId(res), &module.objects[res], vec![], module.span).unwrap(); // 3
            let failure = host.thrown(res, vec![]); // 4
            let next = engine.state.object_next(&second, vec![], module.span).unwrap(); // 5
            emit(&engine, &next);
            let okkama_runtime_core::failure::Kind::Thrown { object, .. } = &failure.kind else { panic!("expected thrown") };
            engine.state.trace_destroy(object, "Res");
            emit(&engine, &second);
            emit(&engine, &alias);
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Res 5\nRes 4\nRes 3\nRes 1\nRes 5\nRes 4\nRes 3\nRes 1\n");

        // Real AllFutures empty and populated paths allocate a result wrapper
        // before the next resource. Reading its cached future adds no identity.
        for populated in [false, true] {
            std::fs::write(&path, "").unwrap();
            sched::reset();
            let engine = new_engine(&module);
            let host: HostRef = Rc::new(IrHost(engine.clone()));
            let value = if populated {
                Value::Future(sched::resolved(Ok(host.make(res, vec![]))))
            } else {
                Value::Undef
            };
            let origins = host.make(id("Origins"), vec![Value::Int(ir::IntValue::from_i64(i64::from(populated))), value]);
            let receiver = host.make(inputs, vec![origins]);
            let result = intrinsics::call(&host, "collect.AllFutures.results", &[container], &[], vec![receiver.clone()]).unwrap();
            let Value::Future(future) = &result else { panic!("expected future") };
            let wrapper = sched::run_main(future).unwrap();
            let repeated = intrinsics::call(&host, "collect.AllFutures.results", &[container], &[], vec![receiver]).unwrap();
            let Value::Future(repeated) = repeated else { panic!("expected future") };
            assert!(Rc::ptr_eq(future, &repeated));
            let Value::Object(wrapper) = wrapper else { panic!("expected result container") };
            assert_eq!(wrapper.declaration, container);
            let after = host.make(res, vec![]);
            emit(&engine, &after);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), if populated { "Res 5\n" } else { "Res 4\n" });
            sched::reset();
        }
    }

    #[test]
    fn trace_destroy_programs() {
        const CHILD: &str = "OKKAMA_TEST_DESTROY_TRACE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let directory = std::env::temp_dir().join(format!("okkama-destroy-{}", std::process::id()));
            std::fs::create_dir(&directory).unwrap();
            let path = directory.join("trace");
            // A separate test process sets the environment without unsafe (#561).
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "destroy_trace::tests::trace_destroy_programs", "--nocapture"])
                .env(CHILD, "1")
                .env("OKKAMA_TRACE_DESTROY", &path)
                .output()
                .unwrap();
            std::fs::remove_dir_all(directory).unwrap();
            assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            return;
        }
        let path = std::env::var_os("OKKAMA_TRACE_DESTROY").unwrap();
        let cases = [
            (
                "object Res { Res(); destroy {} } object Holder { has res is Res; Holder(:res); destroy {} } main() is int { { let a = Res(); let b = Res(); let h = Holder(res: a); } 0; }",
                "Holder 3\nRes 2\nRes 1\n",
                false,
            ),
            (
                "object Plain { Plain(); } object Res { Res(); destroy {} } object Box { has res is Res; Box(:res); } main() is int { { let p = Plain(); let a = Res(); let box = Box(res: a); let b = Res(); } 0; }",
                "Res 3\nRes 1\n",
                false,
            ),
            (
                "object Res { Res(); async destroy {} } async main() is int { { let a = Res(); let b = Res(); } 0; }",
                "Res 2\nRes 1\n",
                false,
            ),
            (
                "object Res { Res(); destroy { panic \"closed\"; } } main() is int { let a = Res(); let b = Res(); 0; }",
                "Res 2\nRes 1\n",
                true,
            ),
            (
                "object Res { Res(); destroy {} } main() is int { let a = Res(); panic \"stop\"; }",
                "Res 1\n",
                true,
            ),
            (
                "object Res { Res(); destroy {} } object Box { has items is @Res; Box(:items); } main() is int { let box = Box(items: []); let a = Res(); 0; }",
                "Res 2\n",
                false,
            ),
            (
                "role Reading { read() is int; } object Plain does Reading { Plain(); read() is int { 0; } } object Res does Reading { Res(); read() is int { 1; } destroy {} } object Box { has value is Reading; Box(:value); } main() is int { let box = Box(value: Plain()); let a = Res(); 0; }",
                "Res 2\n",
                false,
            ),
            (
                "object Res { Res(); destroy {} } object Box { has value is Maybe<Res>; Box(:value); } main() is int { let box = Box(value: Maybe.none); let a = Res(); 0; }",
                "Res 2\n",
                false,
            ),
            (
                "object Res { has n is int; Res(:n); next() is ^Res { ^n = n + 1; } value() is int { n; } destroy {} } main() is int { let a = Res(n: 0); let b = a.next(); b.value(); }",
                "Res 2\nRes 1\n",
                false,
            ),
            (
                "object Marker { Marker(); destroy {} } object Res { Res(); destroy { let marker = Marker(); } } main() is int { let a = Res(); 0; }",
                "Res 1\nMarker 2\n",
                false,
            ),
            (
                "object Marker { Marker(); async destroy {} } object Res { Res(); async destroy { let marker = Marker(); } } async main() is int { let a = Res(); 0; }",
                "Res 1\nMarker 2\n",
                false,
            ),
            (
                "object Plain { Plain(); } main() is int { let p = Plain(); 0; }",
                "",
                false,
            ),
        ];
        for (source, expected, fails) in cases {
            std::fs::write(&path, "prefix\n").unwrap();
            let module = lower(source);
            // Repeated runs restart numbering but append to the file (#561).
            for _ in 0..2 {
                assert_eq!(crate::execute(&module).is_err(), fails, "{source}");
            }
            assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("prefix\n{expected}{expected}"), "{source}");
        }
        let mut module = lower("object Res { Res(); destroy {} } main() is int { let a = Res(); 0; }");
        module.objects[0].name = "example.resources::Res".into();
        std::fs::write(&path, "").unwrap();
        crate::execute(&module).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "example.resources::Res 1\n");
    }
}
