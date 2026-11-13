//! #437: layout tracing must preserve every potentially pointer-bearing slot.
//! In-process, bounded fixtures; timings belong to the isolated A/B run.
use sparrow_core::heap::{self, Heap, Integer, Layouts};
use sparrow_core::program::Loaded;
use sparrow_core::word::{self, Repr, UNBOUND, UNBOUND_RAW};

#[test]
fn gc_layout_empty_and_all_raw_objects_and_closures() {
    for width in [0, 8] {
        let mut heap = Heap::new(Layouts {
            objects: vec![vec![Repr::Raw; width]],
            closures: vec![vec![Repr::Raw; width]],
            ..Layouts::default()
        });
        heap.set_stress(true);
        let raw = heap.allocate_text("not a reference field");
        let fields = vec![raw; width];
        let object = heap.object_new(0, &fields);
        let closure = heap.closure_new(0, &fields);
        heap.collect(|visit| {
            visit(object, Repr::Ref);
            visit(closure, Repr::Ref);
        });
        assert_eq!(heap::kind(heap.header_of(object)), heap::OBJECT);
        assert_eq!(heap::kind(heap.header_of(closure)), heap::CLOSURE);
        assert_eq!(heap::kind(heap.header_of(raw)), heap::FREE);
        heap.collect(|_| {});
        assert_eq!(heap.stats().live_bytes, 0);
    }
}

#[test]
fn gc_layout_mixed_objects_and_closures() {
    let layout = vec![Repr::Raw, Repr::Ref, Repr::Int, Repr::Raw, Repr::Int, Repr::Ref, Repr::Int];
    for stress in [false, true] {
        for kind in [heap::OBJECT, heap::CLOSURE] {
            let mut heap = Heap::new(Layouts {
                objects: vec![layout.clone()],
                closures: vec![layout.clone()],
                frames: vec![layout.clone()],
            });
            heap.set_stress(stress);
            let text = heap.allocate_text("live through a sparse layout");
            let big = heap.int(i64::MAX);
            // Even pointer-shaped raw bits must not keep this object alive.
            let raw = heap.allocate_text("raw bits only");
            let fields = [raw, text, big, UNBOUND_RAW, word::small(-7).unwrap(), UNBOUND, UNBOUND];
            let object = match kind {
                heap::OBJECT => heap.object_new(0, &fields),
                heap::CLOSURE => heap.closure_new(0, &fields[..5]),
                _ => unreachable!(),
            };
            for _ in 0..3 {
                heap.collect(|visit| visit(object, Repr::Ref));
                assert_eq!(heap.text(heap.field(object, 1)), "live through a sparse layout");
                assert_eq!(heap.integer(heap.field(object, 2)).to_string(), i64::MAX.to_string());
                assert_eq!(heap.integer(heap.field(object, 4)), Integer::Small(-7));
                assert_eq!(heap.field(object, 5), UNBOUND);
                assert_eq!(heap.field(object, 6), UNBOUND);
                assert_eq!(heap::kind(heap.header_of(raw)), heap::FREE);
            }
            heap.collect(|_| {});
            assert_eq!(heap.stats().live_bytes, 0);
            assert_eq!(heap.stats().external_live_bytes, 0);
        }
    }
}

#[test]
fn gc_layout_retained_arguments_survive_reused_async_registers() {
    let program = okkama_sparrow::assemble(
        "sparrow 15\nmodule \"gc-layout-frame\" \"1.0.0\"\n\
         type t0 int\ntype t1 str\ntype t2 float\ntype t3 bool\n\
         function f0 \"pending\" async {\n\
         registers [t0, t2, t1, t3]\nparams [r0, r2]\nresult t0\n0: ret r0\n}\n",
    ).unwrap();
    let mut runtime = sparrow_core::runtime::Runtime::new(Loaded::new(program)).unwrap();
    runtime.heap.set_stress(true);
    let integer = runtime.heap.int(i64::MIN);
    let text = runtime.heap.allocate_text("retained argument");
    let frame = runtime.future_new(0, &[integer, text]);
    // The register copies die; the retained inputs must remain independently live.
    runtime.heap.set_field(frame, 7, UNBOUND);
    runtime.heap.set_field(frame, 9, UNBOUND);
    let result = runtime.heap.int(i64::MAX);
    runtime.heap.set_field(frame, 1, result);
    let edges = ["failure", "waiter", "awaiting"].map(|s| runtime.heap.allocate_text(s));
    for (index, edge) in edges.into_iter().enumerate() {
        runtime.heap.set_field(frame, 2 + index, edge);
    }
    for _ in 0..3 {
        runtime.heap.collect(|visit| visit(frame, Repr::Ref));
        assert_eq!(runtime.heap.integer(runtime.heap.field(frame, 11)).to_string(), i64::MIN.to_string());
        assert_eq!(runtime.heap.text(runtime.heap.field(frame, 12)), "retained argument");
        assert_eq!(runtime.heap.integer(runtime.heap.field(frame, 1)).to_string(), i64::MAX.to_string());
        for (index, expected) in ["failure", "waiter", "awaiting"].into_iter().enumerate() {
            assert_eq!(runtime.heap.text(runtime.heap.field(frame, 2 + index)), expected);
        }
    }
}

#[test]
fn gc_layout_driver_state_and_runtime_object_overrides() {
    // Reuse the suspended-Event/ownership regression instead of duplicating it.
    let program = okkama_sparrow::assemble(include_str!("all_futures.spa")).unwrap();
    let loaded = Loaded::new(program);
    let layouts = loaded.layouts();
    assert_eq!(layouts.objects[0], [Repr::Ref, Repr::Raw]); // Atomic<string>
    assert_eq!(layouts.objects[6], [Repr::Raw, Repr::Raw, Repr::Ref]); // Event
    let driver = loaded.program.functions.iter().position(|f| f.symbol == "Event.wait").unwrap();
    assert!(loaded.functions[driver].driver);
    assert_eq!(&layouts.frames[driver][7..], &[Repr::Ref, Repr::Ref, Repr::Ref]);
    let mut runtime = sparrow_core::runtime::Runtime::new(loaded).unwrap();
    runtime.heap.set_stress(true);
    let waiter = runtime.heap.allocate_text("event waiter");
    let event = runtime.heap.object_new(6, &[0, 0, waiter]);
    let frame = runtime.future_new(driver, &[event]);
    let state = runtime.heap.allocate_text("driver state");
    runtime.heap.set_field(frame, 7, UNBOUND); // register reused
    runtime.heap.set_field(frame, 8, state); // driver slot, before retained inputs
    for _ in 0..3 {
        runtime.heap.collect(|visit| visit(frame, Repr::Ref));
        assert_eq!(runtime.heap.text(runtime.heap.field(frame, 8)), "driver state");
        assert_eq!(runtime.heap.field(frame, 9), event);
        assert_eq!(runtime.heap.text(runtime.heap.field(event, 2)), "event waiter");
    }

    // An Atomic<int> is declared fieldless too, but its hidden value can be BigInt.
    let program = okkama_sparrow::assemble(
        "sparrow 15\nmodule \"gc-layout-atomic\" \"1.0.0\"\n\
         type t0 int\ntype t1 object obj0\nobject obj0 \"Atomic<int>\" {}\n\
         function f0 \"new\" intrinsic \"okkama.atomic.Atomic.new_value\" [obj0] [] {\n\
         registers [t0]\nparams [r0]\nresult t1\n}\n",
    ).unwrap();
    let loaded = Loaded::new(program);
    let layouts = loaded.layouts();
    assert_eq!(layouts.objects[0], [Repr::Int, Repr::Raw]);
    let mut heap = Heap::new(layouts);
    heap.set_stress(true);
    let big = heap.int(i64::MAX);
    let atomic = heap.object_new(0, &[big, 0]);
    heap.collect(|visit| visit(atomic, Repr::Ref));
    assert_eq!(heap.integer(heap.field(atomic, 0)).to_string(), i64::MAX.to_string());
}

#[test]
fn gc_layout_fake_storage_preserves_key_and_value_representations() {
    use okkama_sparrow::TypeRef;
    use sparrow_core::{ops::Key, primitives::State};
    let mut heap = Heap::new(Layouts::default());
    heap.set_stress(true);
    let text = heap.allocate_text("fake value");
    let key = heap.int(i64::MAX);
    let value = heap.int(i64::MIN);
    let string_key = heap.allocate_text("fake key");
    let state = State::FakeStorage([
        (Key::Small(7), (word::small(7).unwrap(), Repr::Int, text, TypeRef(0), Repr::Ref)),
        (Key::Small(i64::MAX), (key, Repr::Int, value, TypeRef(1), Repr::Int)),
        (Key::Raw(0), (string_key, Repr::Ref, UNBOUND_RAW, TypeRef(2), Repr::Raw)),
    ].into());
    for _ in 0..3 {
        heap.collect(|visit| state.trace(visit));
        assert_eq!(heap.text(text), "fake value");
        assert_eq!(heap.text(string_key), "fake key");
        assert_eq!(heap.integer(key).to_string(), i64::MAX.to_string());
        assert_eq!(heap.integer(value).to_string(), i64::MIN.to_string());
    }
    heap.collect(|_| {});
    assert_eq!(heap.stats().live_bytes, 0);
}

#[test]
fn gc_layout_bounded_fixture_census() {
    for (width, count) in [(8, 16), (256, 128)] {
        for references in [false, true] {
            let mut layout = vec![if references { Repr::Ref } else { Repr::Raw }; width];
            layout[0] = Repr::Ref;
            let mut heap = Heap::new(Layouts { objects: vec![layout], ..Layouts::default() });
            heap.set_stress(std::env::var_os("SPARROW_CORE_GC_STRESS").is_some());
            heap.allocations = Some(Box::default());
            let child = heap.allocate_text("shared");
            let mut fields = vec![0; width];
            fields[0] = child;
            let roots: Vec<_> = (0..count).map(|_| heap.object_new(0, &fields)).collect();
            for _ in 0..3 {
                heap.collect(|visit| roots.iter().for_each(|&root| visit(root, Repr::Ref)));
                assert_eq!(heap.text(child), "shared");
                let allocations = heap.allocations.as_ref().unwrap();
                assert_eq!(allocations.kinds[&("object", 0)].0, count as u64);
                assert_eq!(allocations.live[&("object", 0)].0, count as u64);
                assert_eq!(allocations.live[&("string", 0)].0, 1);
                // The live census charges size classes; allocation counters
                // report requested bytes, so these byte totals can differ.
                assert_eq!(allocations.live.values().map(|&(_, bytes)| bytes).sum::<u64>(), heap.stats().live_bytes as u64);
            }
            let stats = heap.stats();
            eprintln!("gc-layout width={width} objects={count} references={references} allocations={:?} allocated={} live={} external_allocated={} external_live={}",
                heap.allocations.as_ref().unwrap().kinds, stats.allocated_bytes, stats.live_bytes,
                stats.external_allocated_bytes, stats.external_live_bytes);
            heap.collect(|_| {});
            assert_eq!(heap.stats().live_bytes, 0);
        }
    }
}
