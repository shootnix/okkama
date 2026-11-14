//! #436: in-process payload owners, precise roots and immutable versions.
use sparrow_core::heap::{Heap, Layouts};
use sparrow_core::maps::Entries;
use sparrow_core::ops::{AnyBox, Builder, Key, MapValue, RangeValue};
use sparrow_core::program::Loaded;
use sparrow_core::runtime::{Failure, FailureKind, Runtime};
use sparrow_core::word::Repr;

#[test]
fn payload_436_jit_maps_builders_and_versions() {
    // Dynamically made strings must survive through their containers, rather
    // than the constant table. Calls put all live containers across safepoints.
    let code = [
        "const r0, str \"key\"",
        "const r1, str \"!\"",
        "str.concat r0, r0, r1",
        "const r2, str \"first\"",
        "str.concat r2, r2, r1",
        "builder.new r3",
        "builder.push r3, r3, r2",
        "map.new r5, [r0: r2]",
        "builder.finish r4, r3",
        "builder.new r3",
        "const r2, str \"second\"",
        "str.concat r2, r2, r1",
        "builder.push r3, r3, r2",
        "mbuilder.new r7",
        "mbuilder.put r7, r7, r0, r2",
        "mbuilder.finish r6, r7",
        "map.merge r6, [r5, r6]",
        "const r2, str \"overwritten\"",
        "call r9, f1, []",
        "map.get r2, r5, r0",
        "str.len r8, r2",
        "map.get r2, r6, r0",
        "str.len r9, r2",
        "int.add r8, r8, r9",
        "builder.finish r10, r3",
        "call r9, f1, []",
        "const r9, int 0",
        "arr.get r2, r4, r9",
        "str.len r9, r2",
        "int.add r8, r8, r9",
        "const r9, int 0",
        "arr.get r2, r10, r9",
        "str.len r9, r2",
        "int.add r8, r8, r9",
        "ret r8",
    ];
    let body: String = code.iter().enumerate().map(|(pc, line)| format!(" {pc}: {line}\n")).collect();
    for asynchronous in ["", " async"] {
        let source = format!(r#"sparrow 16
module "payload-436" "1.0.0"
type t0 int
type t1 str
type t2 builder t1
type t3 array t1
type t4 map t1 t1
type t5 mbuilder t1 t1
function f0 "main"{asynchronous} {{
 registers [t1, t1, t1, t2, t3, t4, t4, t5, t0, t0, t3]
 params []
 result t0
{body}}}
function f1 "churn" {{
 registers [t2, t3, t0]
 params []
 result t0
 0: builder.new r0
 1: builder.finish r1, r0
 2: const r2, int 0
 3: ret r2
}}
entry f0
"#);
        let program = okkama_sparrow::assemble(&source).unwrap();
        let bytes = okkama_sparrow::encode(&program).unwrap();
        assert_eq!(sparrow_core::run(bytes).unwrap(), 26);
    }
}

#[test]
fn payload_436_boxed_integer_and_reference_children() {
    let mut heap = Heap::new(Layouts::default());
    heap.set_stress(true);
    let huge = heap.big("123456789012345678901234567890".parse().unwrap());
    let inline = heap.int(7);
    let range = heap.allocate_payload(Box::new(RangeValue { start: huge, end: inline, inclusive: true }));
    let text = heap.allocate_text("builder reference");
    let builder = heap.allocate_payload(Box::new(Builder { repr: Repr::Ref, items: vec![text] }));
    let boxed = heap.allocate_payload(Box::new(AnyBox { value: range, ty: okkama_sparrow::TypeRef(0), repr: Repr::Ref }));
    for _ in 0..3 {
        heap.collect(|visit| {
            visit(boxed, Repr::Ref);
            visit(builder, Repr::Ref);
        });
        let range = heap.payload::<AnyBox>(boxed).unwrap().value;
        let range = heap.payload::<RangeValue>(range).unwrap();
        assert_eq!(heap.integer(range.start).to_string(), "123456789012345678901234567890");
        assert_eq!(heap.integer(range.end).to_string(), "7");
        assert_eq!(heap.text(heap.payload::<Builder>(builder).unwrap().items[0]), "builder reference");
    }
    heap.collect(|_| {});
    assert_eq!(heap.stats().external_live_bytes, 0);
}

#[test]
fn payload_436_persistent_map_versions_trace_shared_nodes() {
    let mut heap = Heap::new(Layouts::default());
    heap.set_stress(true);
    let mut entries = Entries::default();
    for n in 0..=sparrow_core::maps::SMALL {
        let key = format!("key-{n}");
        let value = heap.allocate_text(&format!("value-{n}"));
        let word = heap.allocate_text(&key);
        entries.insert(Key::text(std::rc::Rc::from(key)), (word, value));
    }
    entries.make_persistent();
    assert_eq!(entries.is_persistent(), sparrow_core::maps::persistent_maps());
    let mut updated = entries.clone();
    let key = updated.get_text("key-0").unwrap().0;
    let replacement = heap.allocate_text("replacement");
    updated.insert(Key::text(std::rc::Rc::from("key-0")), (key, replacement));
    let original = heap.allocate_payload(Box::new(MapValue {
        key: Repr::Ref, value: Repr::Ref, entries, owned: Default::default(),
    }));
    let version = heap.allocate_payload(Box::new(MapValue {
        key: Repr::Ref, value: Repr::Ref, entries: updated, owned: Default::default(),
    }));
    for keep_original in [true, true, false, false] {
        heap.collect(|visit| {
            if keep_original {
                visit(original, Repr::Ref);
            }
            visit(version, Repr::Ref);
        });
        let updated = &heap.payload::<MapValue>(version).unwrap().entries;
        for n in 0..=sparrow_core::maps::SMALL {
            let key = format!("key-{n}");
            let &(word, value) = updated.get_text(&key).unwrap();
            assert_eq!(heap.text(word), key);
            assert_eq!(heap.text(value), if n == 0 { "replacement".into() } else { format!("value-{n}") });
        }
        if keep_original {
            let old = &heap.payload::<MapValue>(original).unwrap().entries;
            assert_eq!(heap.text(old.get_text("key-0").unwrap().1), "value-0");
        }
    }
    heap.collect(|_| {});
    assert_eq!(heap.stats().external_live_bytes, 0);
}

#[test]
fn payload_436_failure_causes_secondaries_and_cancellation() {
    let program = okkama_sparrow::assemble("sparrow 16\nmodule \"failures\" \"1.0.0\"\ntype t0 int\nfunction f0 \"main\" {\n registers [t0]\n params []\n result t0\n 0: const r0, int 0\n 1: ret r0\n}\nentry f0\n").unwrap();
    let mut runtime = Runtime::new(Loaded::new(program)).unwrap();
    runtime.heap.set_stress(true);
    let thrown = runtime.heap.allocate_text("thrown value");
    let native = runtime.heap.allocate_text("native value");
    let cause = runtime.failure_new(FailureKind::Thrown(thrown));
    let secondary = runtime.failure_new(FailureKind::Native(native));
    let cancelled = runtime.failure_new(FailureKind::Cancelled);
    let primary = runtime.failure_new(FailureKind::Program("primary".into()));
    let failure = runtime.heap.payload_mut::<Failure>(primary).unwrap();
    failure.cause = Some(cause);
    failure.secondary = vec![secondary, cancelled];
    for _ in 0..3 {
        runtime.heap.collect(|visit| visit(primary, Repr::Ref));
        let failure = runtime.failure(primary).unwrap();
        assert!(matches!(&failure.kind, FailureKind::Program(message) if message == "primary"));
        assert_eq!(failure.cause, Some(cause));
        assert_eq!(failure.secondary, [secondary, cancelled]);
        assert!(matches!(runtime.failure(cause).unwrap().kind, FailureKind::Thrown(value) if value == thrown));
        assert!(matches!(runtime.failure(secondary).unwrap().kind, FailureKind::Native(value) if value == native));
        assert!(matches!(runtime.failure(cancelled).unwrap().kind, FailureKind::Cancelled));
        assert_eq!(runtime.heap.text(thrown), "thrown value");
        assert_eq!(runtime.heap.text(native), "native value");
    }
    runtime.heap.collect(|_| {});
    assert_eq!(runtime.heap.stats().external_live_bytes, 0);
}
