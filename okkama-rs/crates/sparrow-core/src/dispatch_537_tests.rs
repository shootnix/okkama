//! #537: independent expected values and forced-generic differential checks.
use super::*;
use crate::budget::{Limits, MemoryScope};
use crate::program::Loaded;
use crate::runtime::Runtime;

fn runtime(source: &str, plain: bool, stress: bool) -> Box<Runtime> {
    let program = okkama_sparrow::assemble(source).unwrap();
    program.verify().unwrap();
    let mut runtime = Box::new(Runtime::new(Loaded::new(program)).unwrap());
    // Per-runtime switches: no process-wide environment mutation in tests.
    runtime.jit.plain = Chosen { all: plain, ..Chosen::default() };
    runtime.heap.set_stress(stress);
    runtime.stats = Some(Box::default());
    runtime
}

fn source(types: &str, registers: &str, params: &str, result: &str, code: &[String]) -> String {
    let body: String = code.iter().enumerate().map(|(pc, line)| format!(" {pc}: {line}\n")).collect();
    format!("sparrow 15\nmodule \"dispatch-537\" \"1.0.0\"\n{types}\nfunction f0 \"main\" {{\n registers [{registers}]\n params [{params}]\n result {result}\n{body}}}\n")
}

fn event(runtime: &Runtime, name: &str) -> u64 {
    runtime.stats.as_ref().unwrap().get(name).copied().unwrap_or(0)
}

fn helper(runtime: &Runtime, name: &str) -> u64 {
    runtime.jit.helper_calls().into_iter().find(|(key, _)| key == &format!("helper {name}")).map_or(0, |(_, count)| count)
}

#[test]
fn dispatch_537_helper_effects() {
    for name in ["builder_new", "builder_finish", "rat_new", "map_len"] {
        let effects = HELPERS.iter().find(|(key, _)| *key == name).unwrap_or_else(|| panic!("missing {name}")).1;
        assert!(effects.contains(&Effect::Allocates), "{name}");
        assert!(effects.contains(&Effect::Collects), "{name}");
        assert_eq!(effects.contains(&Effect::Fails), name == "rat_new", "{name}");
        assert!(!effects.contains(&Effect::Reenters), "{name}");
    }
    let effects = HELPERS.iter().find(|(key, _)| *key == "scalar_safepoint").expect("scalar safepoint").1;
    assert_eq!(effects, &[Effect::Collects]);
}

#[test]
fn dispatch_537_int_float_rounding_and_big_fallback() {
    crate::on_vm_thread(|| {
        let values = [
            "0", "1", "-1", "9007199254740991", "9007199254740992", "9007199254740993",
            "9007199254740995", "-9007199254740995", "4611686018427387902", "4611686018427387903",
            "-4611686018427387903", "-4611686018427387904",
            "4611686018427387904", "-4611686018427387905", "9223372036854775807", "-9223372036854775808",
            "9223372036854775808", "1234567890123456789012345678901234567890",
        ];
        let text = source("type t0 int\ntype t1 float", "t0, t1", "r0", "t1", &["cvt r1, r0".into(), "ret r1".into()]);
        for plain in [false, true] {
            let mut rt = runtime(&text, plain, true);
            let mut misses = 0;
            for value in values {
                let n: num_bigint::BigInt = value.parse().unwrap();
                // Rust's decimal parser is independent of BigInt::to_f64,
                // which is the generic implementation being compared.
                let expected = value.parse::<f64>().unwrap().to_bits();
                let word = rt.heap.big(n);
                misses += u64::from(word & 1 == 0);
                assert_eq!(rt.run_function_with(0, &[word], 1 << 24).unwrap(), expected, "{value}, plain={plain}");
            }
            assert_eq!(event(&rt, "exec cvt"), if plain { values.len() as u64 } else { misses });
            assert_eq!(event(&rt, "generic cvt int.float"), if plain { values.len() as u64 } else { misses });
            assert_eq!(misses, 6);
            if !plain {
                assert_eq!(event(&rt, "fast cvt int.float"), values.len() as u64 - misses);
            }
            let huge = rt.heap.big(num_bigint::BigInt::from(1) << 2048usize);
            let error = rt.run_function_with(0, &[huge], 1 << 24).unwrap_err();
            assert!(error.0.contains("finite"), "{}", error.0);
        }
        Ok(())
    }).unwrap();
}

#[test]
fn dispatch_537_float_int_limits_nonfinite_and_representation() {
    crate::on_vm_thread(|| {
        let limit = 4611686018427387904.0f64;
        let i64_limit = 9223372036854775808.0f64;
        // Exact binary64 neighbours; SMALL_MAX rounds UP to 2^62 as f64.
        let cases = [
            (0.0, Some(0)), (-0.0, Some(0)), (0.75, Some(0)), (-0.75, Some(0)),
            (1.999, Some(1)), (-1.999, Some(-1)),
            (f64::from_bits(limit.to_bits() - 1), Some(word::SMALL_MAX - 511)),
            (limit, Some(word::SMALL_MAX + 1)),
            (f64::from_bits(limit.to_bits() + 1), Some(word::SMALL_MAX + 1025)),
            (-limit, Some(word::SMALL_MIN)),
            (-f64::from_bits(limit.to_bits() - 1), Some(word::SMALL_MIN + 512)),
            (-f64::from_bits(limit.to_bits() + 1), Some(word::SMALL_MIN - 1024)),
            (f64::from_bits(i64_limit.to_bits() - 1), Some(i64::MAX - 1023)),
            (-f64::from_bits(i64_limit.to_bits() - 1), Some(i64::MIN + 1024)),
            (-i64_limit, Some(i64::MIN)), (i64_limit, None),
            (-f64::from_bits(i64_limit.to_bits() + 1), None),
            (f64::NAN, None), (f64::INFINITY, None), (f64::NEG_INFINITY, None),
        ];
        let text = source("type t0 float\ntype t1 int", "t0, t1", "r0", "t1", &["cvt r1, r0".into(), "ret r1".into()]);
        for plain in [false, true] {
            let mut rt = runtime(&text, plain, true);
            let mut misses = 0;
            for (value, expected) in cases {
                // Parameters are valid low-level fixtures, including NaN/Inf;
                // no invalid SPB constants or unchecked bytecode are used.
                let result = rt.run_function_with(0, &[value.to_bits()], 1 << 24);
                match expected {
                    Some(n) => {
                        let word = result.unwrap();
                        assert_eq!(rt.heap.integer(word).to_string(), n.to_string(), "{value}, plain={plain}");
                        assert_eq!(word::small_value(word), word::small(n).map(|_| n));
                        misses += u64::from(word::small(n).is_none());
                    }
                    None => {
                        misses += 1;
                        let error = result.unwrap_err();
                        assert!(error.0.contains("expected signed-64 range"), "{}", error.0);
                        let expected = if value.is_finite() {
                            // The shared IR numeric contract is independent
                            // of Sparrow's formatter and generic conversion.
                            okkama_runtime_core::float::to_int(value).unwrap_err()
                        } else {
                            format!("Cannot convert float value {value} to int: expected signed-64 range")
                        };
                        let failure = rt.uncaught.as_ref().unwrap();
                        assert_eq!(failure.category, "runtime");
                        assert_eq!(failure.message, expected);
                        assert_eq!(failure.trace, vec![crate::Site { function: 0, pc: 0 }]);
                    }
                }
            }
            assert_eq!(event(&rt, "exec cvt"), if plain { cases.len() as u64 } else { misses });
            assert_eq!(event(&rt, "generic cvt float.int"), if plain { cases.len() as u64 } else { misses });
            assert_eq!(misses, 11);
            if !plain {
                assert_eq!(event(&rt, "fast cvt float.int"), cases.len() as u64 - misses);
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn dispatch_537_async_roots_and_scalar_safepoints() {
    crate::on_vm_thread(|| {
        let text = source("type t0 str\ntype t1 int\ntype t2 float\ntype t3 builder t0\ntype t4 array t0",
            "t0, t0, t0, t1, t2, t1, t3, t4", "", "t0", &[
                "const r0, str \"a\"".into(), "const r1, str \"b\"".into(), "str.concat r2, r0, r1".into(),
                "const r3, int 9223372036854775807".into(), "const r5, int 1".into(), "int.add r3, r3, r5".into(),
                "builder.new r6".into(), "builder.push r6, r6, r2".into(),
                "cvt r4, r5".into(), "cvt r5, r4".into(), "cvt r4, r3".into(),
                "builder.finish r7, r6".into(), "int.add r3, r3, r5".into(), "cvt r0, r3".into(),
                "const r5, int 0".into(), "arr.get r2, r7, r5".into(), "str.concat r1, r2, r0".into(), "ret r1".into(),
            ]);
        for asynchronous in [false, true] {
            let text = if asynchronous { text.replace("function f0 \"main\" {", "function f0 \"main\" async {") } else { text.clone() };
            for plain in [false, true] {
                let mut rt = runtime(&text, plain, true);
                let result = rt.run_function(0, 1 << 24).unwrap();
                assert_eq!(rt.heap.text(result), "ab9223372036854775809");
                assert!(event(&rt, "gc collection") > 0);
                assert_eq!(event(&rt, "fast cvt int.float"), u64::from(!plain));
                assert_eq!(event(&rt, "fast cvt float.int"), u64::from(!plain));
                assert_eq!(event(&rt, "generic cvt int.float"), if plain { 2 } else { 1 });
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn dispatch_537_builders_reuse_grow_and_trace_all_element_reprs() {
    crate::on_vm_thread(|| {
        for (element, repr) in [
            ("int", Repr::Int), ("str", Repr::Ref), ("float", Repr::Raw), ("byte", Repr::Raw),
        ] {
            for length in [0usize, 1, 17, heap::SPARE_CAPACITY, heap::SPARE_CAPACITY + 1] {
                let mut code = vec!["builder.new r0".into()];
                code.extend(std::iter::repeat_n("builder.push r0, r0, r1".into(), length));
                // r1 dies before this safepoint: only the builder can keep
                // its dynamically allocated (not constant-pool) items alive.
                code.extend(["const r3, int 1".into(), "cvt r4, r3".into()]);
                code.extend(["builder.finish r2, r0".into(), "ret r2".into()]);
                let mut text = source(&format!("type t0 {element}\ntype t1 builder t0\ntype t2 array t0\ntype t3 int\ntype t4 float"),
                    "t1, t0, t2, t3, t4", "r1", "t2", &code);
                text.push_str("function f1 \"fresh\" {\n registers [t1]\n params []\n result t1\n 0: builder.new r0\n 1: ret r0\n}\n");
                for plain in [false, true] {
                    let mut rt = runtime(&text, plain, true);
                    let mut published = Vec::new();
                    for _ in 0..3 {
                        let value = match repr {
                            Repr::Ref => rt.heap.allocate_text("live reference"),
                            Repr::Int => rt.heap.big("9223372036854775808".parse().unwrap()),
                            Repr::Raw if element == "float" => (-0.0f64).to_bits(),
                            Repr::Raw => 255,
                        };
                        let array = rt.run_function_with(0, &[value], 1 << 24).unwrap();
                        assert_eq!(rt.heap.array_length(array), length);
                        rt.roots.push((array, Repr::Ref));
                        published.push(array);
                        let fresh = rt.run_function(1, 1 << 24).unwrap();
                        let payload = rt.heap.payload::<crate::ops::Builder>(fresh).unwrap();
                        assert!(payload.items.is_empty());
                        assert_eq!(payload.repr, repr);
                        if length <= heap::SPARE_CAPACITY {
                            assert!(payload.items.capacity() >= length, "spare capacity was lost");
                        } else {
                            assert_eq!(payload.items.capacity(), 0, "oversized capacity was retained");
                        }
                        // The spare is reused by a new builder before these
                        // checks: previously published arrays stay immutable.
                        for item in published.iter().flat_map(|&array| rt.heap.elements(array).into_owned()) {
                            match repr {
                                Repr::Ref => assert_eq!(rt.heap.text(item), "live reference"),
                                Repr::Int => assert_eq!(rt.heap.integer(item).to_string(), "9223372036854775808"),
                                Repr::Raw if element == "float" => assert_eq!(item, (-0.0f64).to_bits()),
                                Repr::Raw => assert_eq!(item, 255),
                            }
                        }
                    }
                    assert_eq!(event(&rt, "exec builder.new"), if plain { 6 } else { 0 });
                    assert_eq!(event(&rt, "exec builder.finish"), if plain { 3 } else { 0 });
                    assert_eq!(helper(&rt, "builder_new"), if plain { 0 } else { 6 });
                    assert_eq!(helper(&rt, "builder_finish"), if plain { 0 } else { 3 });
                }
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn dispatch_537_map_lengths_btree_persistent_shared() {
    crate::on_vm_thread(|| {
        let text = source("type t0 int\ntype t1 map t0 t0", "t1, t0", "r0", "t0", &["map.len r1, r0".into(), "ret r1".into()]);
        for plain in [false, true] {
            let mut rt = runtime(&text, plain, true);
            for length in [0usize, 1, 65] {
                let mut entries = crate::maps::Entries::default();
                for n in 0..length as i64 {
                    entries.upsert(crate::ops::Key::Small(n), (word::small(n).unwrap(), word::small(n).unwrap()));
                }
                for persistent in [false, true] {
                    if persistent {
                        let crate::maps::Entries::BTree(tree) = std::mem::take(&mut entries) else { panic!("BTree fixture") };
                        entries = crate::maps::Entries::Persistent(okkama_persistent::PMap::from_sorted(tree));
                    }
                    assert_eq!(entries.is_persistent(), persistent);
                    for _ in 0..2 {
                        let map = rt.heap.allocate_payload(Box::new(crate::ops::MapValue {
                            key: Repr::Int, value: Repr::Int, entries: entries.clone(), owned: None.into(),
                        }));
                        let result = rt.run_function_with(0, &[map], 1 << 24).unwrap();
                        assert_eq!(word::small_value(result), Some(length as i64));
                    }
                }
            }
            assert_eq!(event(&rt, "exec map.len"), if plain { 12 } else { 0 });
            assert_eq!(helper(&rt, "map_len"), if plain { 0 } else { 12 });
        }
        Ok(())
    }).unwrap();
}

#[test]
fn dispatch_537_rat_new_exact_normalization_failure_and_gc() {
    crate::on_vm_thread(|| {
        let text = source("type t0 int\ntype t1 rat", "t0, t0, t1", "r0, r1", "t1", &["rat.new r2, r0, r1".into(), "ret r2".into()]);
        for plain in [false, true] {
            let mut rt = runtime(&text, plain, true);
            for (a, b, expected) in [
                ("2", "-4", "-1/2"), ("0", "-1", "0"),
                ("9223372036854775808", "-2", "-4611686018427387904"),
                ("123456789012345678901234567890", "3", "41152263004115226300411522630"),
            ] {
                let args = [rt.heap.big(a.parse().unwrap()), rt.heap.big(b.parse().unwrap())];
                let result = rt.run_function_with(0, &args, 1 << 24).unwrap();
                assert_eq!(rt.rational(result).to_string(), expected);
            }
            let error = rt.run_function_with(0, &[word::small(1).unwrap(), word::small(0).unwrap()], 1 << 24).unwrap_err();
            assert!(error.0.contains("zero"), "{}", error.0);
            let expected = okkama_runtime_core::rat::Rat::new(
                &okkama_runtime_core::IntValue::from_i64(1), &okkama_runtime_core::IntValue::from_i64(0),
            ).unwrap_err();
            let failure = rt.uncaught.as_ref().unwrap();
            assert_eq!(failure.category, "runtime");
            assert_eq!(failure.message, expected);
            assert_eq!(failure.trace, vec![crate::Site { function: 0, pc: 0 }]);
            assert_eq!(event(&rt, "exec rat.new"), if plain { 5 } else { 0 });
            assert_eq!(helper(&rt, "rat_new"), if plain { 0 } else { 5 });
        }
        Ok(())
    }).unwrap();
}

#[test]
fn dispatch_537_builder_budgets_match_generic() {
    crate::on_vm_thread(|| {
        let text = source("type t0 int\ntype t1 builder t0\ntype t2 array t0", "t1, t2, t0", "", "t0", &[
            "builder.new r0".into(), "builder.finish r1, r0".into(), "jmp @0".into(),
            "const r2, int 0".into(), "ret r2".into(),
        ]);
        for plain in [false, true] {
            for memory in [false, true] {
                let mut rt = runtime(&text, plain, false);
                rt.limits = Limits {
                    steps: Some(2000), memory: memory.then_some(16 << 10), memory_scope: MemoryScope::Runtime,
                    ..Limits::default()
                };
                let error = rt.run_function(0, 1 << 24).unwrap_err();
                assert_eq!(error.0, if memory { "Execution budget exhausted: memory" } else { "Execution budget exhausted: steps" });
            }
        }
        Ok(())
    }).unwrap();
}
