//! Bounded, in-process #542 regressions; run this file in normal, old-dispatch
//! and GC-stress configurations. No workers, external services or deep recursion.
use sparrow_core::{jit, program::Loaded, runtime::Runtime};

fn runtime() -> Box<Runtime> {
    let program = okkama_sparrow::assemble(include_str!("ready_callees.spa")).unwrap();
    Box::new(Runtime::new(Loaded::new(program)).unwrap())
}

#[test]
fn ready_callees_preserve_bigint_reference_and_handler_roots() {
    for ready in [false, true] {
        let mut runtime = runtime();
        if ready {
            jit::compile(&mut runtime, 0).unwrap();
            jit::compile(&mut runtime, 2).unwrap();
        }
        for _ in 0..2 {
            // Fresh heap-only inputs: no Runtime::roots/constants can mask a
            // missing machine-frame root during the allocating call.
            let big = runtime.heap.big("123456789012345678901234567890".parse().unwrap());
            let text = runtime.heap.allocate_text("heap");
            let object_text = runtime.heap.allocate_text("object");
            let object = runtime.heap.object_new(0, &[object_text]);
            let result = runtime.run_function_with(1, &[big, text, object], 2 << 20).unwrap();
            let number = runtime.heap.field(result, 0);
            assert_eq!(runtime.heap.integer(number).to_string(), "246913578024691357802469135780");
            assert_eq!(runtime.heap.text(runtime.heap.field(result, 1)), "heapheapobjectheap");
            let handler_text = runtime.heap.allocate_text("handler");
            let caught = runtime.run_function_with(3, &[handler_text], 2 << 20).unwrap();
            assert_eq!(runtime.heap.text(caught), "handlerhandler");
        }
        let error = runtime.run_function(4, 2 << 20).unwrap_err();
        assert!(error.0.contains("callee failurecallee failure"), "{}", error.0);
        let trace = sparrow_core::render_trace(&runtime.loaded.program, &runtime.uncaught.as_ref().unwrap().trace);
        assert!(trace.contains("trace"), "{trace}");
        assert!(trace.contains("fail"), "{trace}");
    }
}
