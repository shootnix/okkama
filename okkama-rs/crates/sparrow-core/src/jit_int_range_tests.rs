//! #539: actual result representations, post-definition GC, helpers and resume.
use super::*;

fn range_runtime(source: &str, ranges: bool, frontend: bool) -> Box<Runtime> {
    let mut runtime = runtime(source, true, frontend);
    for (f, info) in runtime.loaded.program.functions.iter().zip(&mut runtime.loaded.functions) {
        info.pointer_free = crate::pointer_free::analyze_with_ranges(f, &info.kinds, &runtime.loaded.program, ranges);
    }
    runtime
}

fn source(registers: &str, code: &[String]) -> String {
    let code: String = code.iter().enumerate().map(|(pc, op)| format!(" {pc}: {op}\n")).collect();
    format!("sparrow 16\nmodule \"range\" \"1\"\ntype t0 int\ntype t1 str\ntype t2 array t0\ntype t3 byte\ntype t4 char\ntype t5 map t0 t0\ntype t6 float\ntype t7 rat\ntype t8 bool\nfunction f0 \"range\" {{\n registers [{registers}]\n params []\n result t0\n{code}}}\nfunction f1 \"collect\" {{\n registers [t1, t1]\n params []\n result void\n 0: const r0, str \"gc\"\n 1: str.concat r1, r0, r0\n 2: ret _\n}}\nentry f0\n")
}

#[test]
fn pointer_free_modulo_normalizes_big_inputs_and_preserves_failures() {
    crate::on_vm_thread(|| {
        // Literal expected values are an independent oracle (including the
        // dividend's sign); do not compute them with the implementation's %.
        for (input, divisor, expected, omitted) in [
            ("-4611686018427387904", "-1", "0", true),
            ("4611686018427387903", "-4611686018427387904", "4611686018427387903", true),
            ("-4611686018427387903", "-4611686018427387904", "-4611686018427387903", true),
            ("1267650600228229401496703205376", "7", "2", true),
            ("-1267650600228229401496703205376", "-7", "-2", true),
            ("1267650600228229401496703205376", "-1", "0", true),
            ("1267650600228229401496703205375", "1267650600228229401496703205376", "1267650600228229401496703205375", false),
            ("-4611686018427387904", "4611686018427387904", "0", false),
            ("123", "0", "failure", false),
        ] {
            let source = source("t0, t0, t0, t0, t0, t0, t8", &[
                format!("const r0, int {input}"), "const r1, int 0".into(),
                "int.add r2, r0, r1".into(), format!("const r3, int {divisor}"),
                "int.mod r4, r2, r3".into(), "call _, f1, []".into(),
                // A dynamically allocated dividend stays live through both the
                // modulo helper and the later allocating call.
                "int.sub r5, r2, r0".into(), "int.cmp eq, r6, r5, r1".into(),
                "br r6, @10, @9".into(), "trap confirm, \"lost big dividend\"".into(), "ret r4".into(),
            ]);
            for ranges in [false, true] {
                let mut runtime = range_runtime(&source, ranges, false);
                assert_eq!(runtime.loaded.functions[0].pointer_free[4], ranges && omitted);
                assert!(!runtime.loaded.functions[0].pointer_free[2]);
                let result = runtime.run_function(0, crate::STACK_BYTES);
                if expected == "failure" {
                    assert!(result.unwrap_err().0.contains("Modulo by zero"));
                } else {
                    let result = result.unwrap();
                    assert_eq!(runtime.big_value(result).to_string(), expected);
                    if omitted {
                        assert_eq!(Repr::Int.pointer(result), None);
                    } else if expected.len() > 20 {
                        assert!(Repr::Int.pointer(result).is_some());
                    }
                }
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn pointer_free_lengths_conversions_and_rejected_scalar_ranges() {
    crate::on_vm_thread(|| {
        for (registers, prefix, expected, omitted) in [
            ("t0, t2", vec!["arr.new r1, []", "arr.len r0, r1"], "0", true),
            ("t0, t1", vec!["const r1, str \"ascii\"", "str.len r0, r1"], "5", true),
            ("t0, t1", vec!["const r1, str \"é🙂\"", "str.len r0, r1"], "2", true),
            ("t0, t3", vec!["const r1, byte 255", "cvt r0, r1"], "255", true),
            ("t0, t4", vec!["const r1, char 1114111", "cvt r0, r1"], "1114111", true),
            ("t0, t3", vec!["const r1, byte 0", "cvt r0, r1"], "0", true),
            ("t0, t4", vec!["const r1, char 0", "cvt r0, r1"], "0", true),
            ("t0, t5", vec!["map.new r1, []", "map.len r0, r1"], "0", false),
            ("t0, t6", vec!["const r1, float 0x43d0000000000000", "cvt r0, r1"], "4611686018427387904", false),
        ] {
            let mut code: Vec<String> = prefix.into_iter().map(String::from).collect();
            code.extend(["call _, f1, []".into(), "ret r0".into()]);
            let source = source(registers, &code);
            for ranges in [false, true] {
                for plain in [false, true] {
                    let mut runtime = range_runtime(&source, ranges, false);
                    runtime.jit.plain.all = plain;
                    assert_eq!(runtime.loaded.functions[0].pointer_free[0], ranges && omitted);
                    let result = runtime.run_function(0, crate::STACK_BYTES).unwrap();
                    assert_eq!(runtime.big_value(result).to_string(), expected);
                    assert_eq!(Repr::Int.pointer(result).is_none(), expected != "4611686018427387904");
                }
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn pointer_free_physical_length_bounds_cover_all_heap_representations() {
    let mut runtime = range_runtime(&source("t0", &["const r0, int 0".into(), "ret r0".into()]), true, false);
    let heap = &mut runtime.heap;
    let words = vec![word::small(1).unwrap(); 300];
    let flat = heap.allocate_array(Repr::Int, &words[..5]);
    let packed = heap.allocate_bytes(&[0, 255]);
    let trie = heap.trie_from(Repr::Int, &words);
    let buffer = heap.allocate_buffer(Repr::Int, &[&words[..10]], 16, 3);
    let shared = heap.allocate_view(Repr::Int, buffer, 3, 10);
    for (array, length) in [(flat, 5), (packed, 2), (trie, 300), (shared, 10)] {
        assert_eq!(u64::from(heap::payload(heap.header_of(array))), length);
        let result = heap.int(heap.array_length(array) as i64);
        assert_eq!(word::small_value(result), Some(length as i64));
        assert_eq!(Repr::Int.pointer(result), None);
    }
    let buffer = heap.allocate_text_buffer(&["é🙂"], 32, 3);
    let view = heap.allocate_text_view(buffer, 3, "é🙂".len(), false);
    for text in [heap.allocate_text("ascii"), heap.allocate_text("é🙂"), view] {
        let bytes = heap::payload(heap.header_of(text));
        let length = heap.char_count(text);
        assert!(length <= bytes as usize);
        assert_eq!(heap.char_count(text), length); // cached path too
        let result = heap.int(length as i64);
        assert_eq!(Repr::Int.pointer(result), None);
    }
    // No huge allocation or feasibility assumption: the physical payload's
    // largest possible u32 value is itself inside the actual tagged limits.
    assert_eq!(word::small_value(word::small(i64::from(u32::MAX)).unwrap()), Some(i64::from(u32::MAX)));
}

#[test]
fn pointer_free_operation_results_survive_await_and_keep_big_aliases_and_cleanup() {
    crate::on_vm_thread(|| {
        let source = include_str!("../tests/all_futures.spa")
            .replace("type t17 future t16", "type t17 future t16\ntype t18 byte\ntype t19 array t0")
            .replace("registers [t1, t4, t16, t3, t6, t17, t16]",
                "registers [t1, t4, t16, t3, t6, t17, t16, t0, t0, t0, t0, t0, t0, t0, t18, t0, t19, t0, t0, t0]")
            .replace(
                "    0: region.enter r3\n    1: obj.new r4, obj2, [r0, r1]\n    2: region.adopt r3, r4\n    3: call r5, f14, [r2]\n    4: await r6, r5\n    5: region.leave r3, r4\n    6: ret r4",
                "    0: const r7, int 4611686018427387903\n    1: const r8, int 1\n    2: int.add r9, r7, r8\n    3: const r10, int -1\n    4: int.mod r11, r9, r10\n    5: str.len r12, r0\n    6: const r13, int 0\n    7: const r14, byte 255\n    8: cvt r15, r14\n    9: arr.new r16, [r9]\n    10: arr.len r17, r16\n    11: const r18, int 255\n    12: region.enter r3\n    13: obj.new r4, obj2, [r0, r1]\n    14: region.adopt r3, r4\n    15: call r5, f14, [r2]\n    16: await r6, r5\n    17: call _, f17, [r11, r13]\n    18: call _, f17, [r12, r8]\n    19: call _, f17, [r15, r18]\n    20: call _, f17, [r17, r8]\n    21: arr.get r19, r16, r13\n    22: int.sub r19, r19, r7\n    23: call _, f17, [r19, r8]\n    24: int.sub r19, r9, r7\n    25: call _, f17, [r19, r8]\n    26: region.leave r3, r4\n    27: ret r4",
            ) + "\nfunction f17 \"test.range_int\" intrinsic \"okkama.test.is\" [] [] {\n registers [t0, t0]\n params [r0, r1]\n result void\n}\n";
        for ready in [false, true] {
            let source = source.replace("bool true", &format!("bool {ready}"));
            for frontend in [false, true] {
                let mut counts = Vec::new();
                for ranges in [false, true] {
                    let mut runtime = range_runtime(&source, ranges, frontend);
                    let info = &runtime.loaded.functions[4];
                    assert_eq!(info.kinds.len(), 20); // replacement applied
                    assert!(!info.pointer_free[9]); // dynamically boxed 2^62
                    let layouts = runtime.loaded.layouts();
                    // #578 retains the original arguments after the working
                    // registers/driver slot. Never truncate or reclassify that
                    // tail when integrating the older #535 candidate.
                    for ((source, info), frame) in runtime.loaded.program.functions.iter()
                        .zip(&runtime.loaded.functions).zip(&layouts.frames)
                    {
                        let retained = crate::tasks::FIXED_WORDS + info.kinds.len() + usize::from(info.driver);
                        assert_eq!(frame.len(), retained + source.params.len(), "{}", source.symbol);
                        for (index, param) in source.params.iter().enumerate() {
                            assert_eq!(frame[retained + index], info.kinds[param.0 as usize].repr());
                        }
                    }
                    for register in [11, 12, 15, 17] {
                        assert_eq!(info.pointer_free[register], ranges);
                        assert_eq!(layouts.frames[4][crate::tasks::FIXED_WORDS + register],
                            if ranges { Repr::Raw } else { Repr::Int });
                    }
                    assert_eq!(layouts.frames[4][crate::tasks::FIXED_WORDS + 9], Repr::Int);
                    compile(&mut runtime, 4).unwrap();
                    counts.push(runtime.jit.maps.iter().map(|(_, _, words)| words.len()).sum::<usize>());
                    runtime.run_function(16, crate::STACK_BYTES).unwrap();
                    // Existing independent fixture also asserts alias identity
                    // and the exact BA cleanup order; check all its assertions.
                    assert_eq!(runtime.host.as_ref().unwrap().test_failures.as_deref(), Some([].as_slice()));
                }
                assert!(counts[1] < counts[0], "frontend={frontend}, roots={counts:?}");
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn pointer_free_unbound_conversion_generic_fallback_remains_rooted() {
    crate::on_vm_thread(|| {
        for ty in ["t3", "t4"] {
            let source = source(&format!("t0, {ty}"), &[
                "undef r1".into(), "cvt r0, r1".into(),
                "call _, f1, []".into(), "ret r0".into(),
            ]);
            for ranges in [false, true] {
                let mut runtime = range_runtime(&source, ranges, false);
                runtime.jit.plain.all = true;
                assert!(!runtime.loaded.functions[0].pointer_free[0]);
                let result = runtime.run_function(0, crate::STACK_BYTES).unwrap();
                assert_eq!(runtime.big_value(result).to_string(), word::UNBOUND_RAW.to_string());
                assert!(Repr::Int.pointer(result).is_some());
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn pointer_free_modulo_zero_unwind_preserves_lengths_failure_and_cleanup() {
    crate::on_vm_thread(|| {
        for asynchronous in [false, true] {
            let suffix = include_str!("../tests/int_range_cleanup.spa")
                .replace("$ASYNC", if asynchronous { "async" } else { "" })
                .replace("$UNWIND", if asynchronous { "region.settle_unwind" } else { "region.unwind" });
            let source = include_str!("../tests/all_futures.spa").to_string() + &suffix;
            for frontend in [false, true] {
                for ranges in [false, true] {
                    let mut runtime = range_runtime(&source, ranges, frontend);
                    assert_eq!(runtime.loaded.functions[17].pointer_free[5], ranges);
                    assert!(!runtime.loaded.functions[17].pointer_free[6]); // zero/failure, reused slot
                    let error = runtime.run_function(17, crate::STACK_BYTES).unwrap_err();
                    assert!(error.0.contains("Modulo by zero"), "{}", error.0);
                    assert_eq!(runtime.host.as_ref().unwrap().test_failures.as_deref(), Some([].as_slice()));
                }
            }
        }
        Ok(())
    }).unwrap();
}
