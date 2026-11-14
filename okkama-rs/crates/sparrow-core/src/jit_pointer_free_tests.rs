//! In-process differential tests for #535; no environment mutation or children.
use super::*;

fn runtime(source: &str, enabled: bool, frontend: bool) -> Box<Runtime> {
    let program = okkama_sparrow::assemble(source).unwrap();
    let mut loaded = crate::program::Loaded::new(program);
    for (source, info) in loaded.program.functions.iter().zip(&mut loaded.functions) {
        info.pointer_free = if enabled {
            crate::pointer_free::analyze(source, &info.kinds)
        } else {
            vec![false; info.kinds.len()]
        };
    }
    let mut runtime = Box::new(Runtime::new(loaded).unwrap());
    runtime.jit.frontend_maps.all = frontend;
    runtime.host.as_mut().unwrap().test_failures = Some(Vec::new());
    runtime
}

#[test]
fn stack_maps_and_suspended_layouts_omit_only_proven_slots() {
    let source = include_str!("../tests/pointer_free.spa");
    let mut counts = Vec::new();
    for enabled in [false, true] {
        let mut runtime = runtime(source, enabled, false);
        let info = &runtime.loaded.functions[0];
        assert!(!info.pointer_free[0]); // small, then allocated BigInt
        assert_eq!(info.pointer_free[1], enabled); // SMALL_MAX
        let layouts = runtime.loaded.layouts();
        assert_eq!(layouts.frames[0][crate::tasks::FIXED_WORDS], Repr::Int);
        assert_eq!(layouts.frames[0][crate::tasks::FIXED_WORDS + 1], if enabled { Repr::Raw } else { Repr::Int });
        // Captured parameters retain their declared layout, even if the body
        // later overwrites them with a small constant.
        assert_eq!(layouts.closures[1], vec![Repr::Int]);
        compile(&mut runtime, 0).unwrap();
        counts.push(runtime.jit.maps.iter().map(|(_, _, words)| words.len()).sum::<usize>());
    }
    assert!(counts[1] < counts[0], "root words off/on: {counts:?}");
}

#[test]
fn sync_definitions_calls_captures_and_failure_match_without_analysis() {
    crate::on_vm_thread(|| {
        let source = include_str!("../tests/pointer_free.spa");
        for enabled in [false, true] {
            let mut runtime = runtime(source, enabled, false);
            assert_eq!(runtime.run_entry(crate::STACK_BYTES).unwrap(), 42);
            // Trap failure must survive both the handler edge and a later call.
            let error = runtime.run_function(5, crate::STACK_BYTES).unwrap_err();
            assert!(error.0.contains("pointer-free failure"), "{}", error.0);
        }
        Ok(())
    }).unwrap();
}

#[test]
fn async_suspension_results_and_cleanup_match_in_both_root_modes() {
    crate::on_vm_thread(|| {
        // Independent oracle in the existing #580 fixture checks every result and
        // the exact BA destruction order. Add a dynamically allocated BigInt in a
        // reused slot, plus two genuinely constant slots, live across Event.wait.
        let source = include_str!("../tests/all_futures.spa").replace(
            "registers [t1, t4, t16, t3, t6, t17, t16]",
            "registers [t1, t4, t16, t3, t6, t17, t16, t0, t0, t0, t0]",
        ).replace(
            "    0: region.enter r3\n    1: obj.new r4, obj2, [r0, r1]\n    2: region.adopt r3, r4\n    3: call r5, f14, [r2]\n    4: await r6, r5\n    5: region.leave r3, r4\n    6: ret r4",
            "    0: const r7, int 0\n    1: const r8, int 1\n    2: const r9, int 4611686018427387903\n    3: int.add r7, r9, r8\n    4: region.enter r3\n    5: obj.new r4, obj2, [r0, r1]\n    6: region.adopt r3, r4\n    7: call r5, f14, [r2]\n    8: await r6, r5\n    9: int.sub r10, r7, r8\n    10: call _, f17, [r10, r9]\n    11: region.leave r3, r4\n    12: ret r4",
        ) + "\nfunction f17 \"test.is_int\" intrinsic \"okkama.test.is\" [] [] {\n registers [t0, t0]\n params [r0, r1]\n result void\n}\n";
        for ready in [true, false] {
            let source = source.replace("bool true", &format!("bool {ready}"));
            for frontend in [false, true] {
                let mut counts = Vec::new();
                for enabled in [false, true] {
                    let mut runtime = runtime(&source, enabled, frontend);
                    let info = &runtime.loaded.functions[4];
                    assert_eq!(info.kinds.len(), 11); // fixture replacement really applied
                    assert!(!info.pointer_free[7]); // later allocated BigInt
                    assert_eq!(&info.pointer_free[8..10], &[enabled; 2]);
                    assert!(!info.pointer_free[6]); // await result
                    let frame = &runtime.loaded.layouts().frames[4];
                    assert_eq!(frame[crate::tasks::FIXED_WORDS + 7], Repr::Int);
                    assert_eq!(frame[crate::tasks::FIXED_WORDS + 8], if enabled { Repr::Raw } else { Repr::Int });
                    compile(&mut runtime, 4).unwrap();
                    counts.push(runtime.jit.maps.iter().map(|(_, _, words)| words.len()).sum::<usize>());
                    runtime.run_function(16, crate::STACK_BYTES).unwrap();
                    assert_eq!(runtime.host.as_ref().unwrap().test_failures.as_deref(), Some([].as_slice()));
                }
                assert!(counts[1] < counts[0], "async roots, frontend={frontend}: {counts:?}");
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn sync_and_async_unwind_cleanup_preserve_failure_and_destroy_log() {
    crate::on_vm_thread(|| {
        for asynchronous in [false, true] {
            let suffix = include_str!("../tests/pointer_free_cleanup.spa")
                .replace("$ASYNC", if asynchronous { "async" } else { "" })
                .replace("$UNWIND", if asynchronous { "region.settle_unwind" } else { "region.unwind" });
            let source = include_str!("../tests/all_futures.spa").to_string() + &suffix;
            for frontend in [false, true] {
                for enabled in [false, true] {
                    let mut runtime = runtime(&source, enabled, frontend);
                    let error = runtime.run_function(17, crate::STACK_BYTES).unwrap_err();
                    assert!(error.0.contains("cleanup failure"), "{}", error.0);
                    assert_eq!(runtime.host.as_ref().unwrap().test_failures.as_deref(), Some([].as_slice()));
                }
            }
        }
        Ok(())
    }).unwrap();
}
