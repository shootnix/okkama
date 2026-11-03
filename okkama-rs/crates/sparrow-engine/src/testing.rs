//! Isolated test sessions; assertions accumulate while runtime failures end only
//! their current case. The runner executes new-engine plans in both modes.
use crate::{Error, Executable, FunctionPlan, Mode, Op, Program, Type, value::*};
#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    Ok,
    Is,
    Isnt,
    Fail,
}

pub(crate) fn lower(program: &Program, index: usize) -> Result<FunctionPlan, Error> {
    let f = &program.functions[index];
    let intrinsic = f.intrinsic.as_ref().expect("test intrinsic");
    let operation = match intrinsic.name.as_str() {
        "okkama.test.ok" => Operation::Ok,
        "okkama.test.is" => Operation::Is,
        "okkama.test.isnt" => Operation::Isnt,
        "okkama.test.fail" => Operation::Fail,
        _ => return Err(Error("unknown test assertion".into())),
    };
    let count = if matches!(operation, Operation::Is | Operation::Isnt) {
        2
    } else {
        1
    };
    if f.is_async
        || f.result.is_some()
        || f.params.len() != count
        || !intrinsic.helpers.is_empty()
        || !intrinsic.objects.is_empty()
        || matches!(operation, Operation::Ok)
            && program.ty(f.registers[f.params[0].0 as usize]) != Some(&Type::Bool)
        || matches!(operation, Operation::Fail)
            && program.ty(f.registers[f.params[0].0 as usize]) != Some(&Type::Str)
    {
        return Err(Error("invalid test assertion signature".into()));
    }
    let params: Vec<_> = f.params.iter().map(|r| r.0 as usize).collect();
    let plan = vec![
        Op::Intrinsic(
            crate::intrinsic::Operation::Test(operation),
            f.registers.len(),
            [Some(params[0]), params.get(1).copied(), None],
        ),
        Op::ReturnVoid,
    ];
    let live = crate::liveness::analyze(&plan, &[]);
    let call_saved = crate::liveness::call_saved(&plan, &live, &[]);
    Ok(FunctionPlan {
        native: None,
        is_async: false,
        cold_entry: 0,
        cached: false,
        symbol: f.symbol.clone(),
        recurrence_loop: false,
        elided_objects: 0,
        params,
        plan,
        registers: f.registers.len() + 1,
        compiled: Default::default(),
        jit_kinds: None,
        live,
        call_saved,
        handlers: vec![],
    })
}
impl Heap {
    fn test_same(&mut self, left: Slot, right: Slot) -> Result<bool, Error> {
        let mut pending = vec![(left, right)];
        let mut seen = std::collections::HashSet::new();
        while let Some((a, b)) = pending.pop() {
            if !seen.insert((a.tag, a.bits, b.tag, b.bits)) {
                continue;
            }
            match (a.tag, b.tag) {
                (ANY, ANY) => pending.push((*self.boxes.get(a.bits)?, *self.boxes.get(b.bits)?)),
                (ARRAY, ARRAY) => {
                    let (a, b) = (self.elements(a)?, self.elements(b)?);
                    if a.len() != b.len() {
                        return Ok(false);
                    }
                    pending.extend(a.iter().copied().zip(b.iter().copied()));
                }
                (MAP, MAP) => {
                    let (a, b) = (self.maps.get(a.bits)?, self.maps.get(b.bits)?);
                    if a.len() != b.len() {
                        return Ok(false);
                    }
                    for ((key, (_, a)), (other, (_, b))) in a.iter().zip(b) {
                        if key != other {
                            return Ok(false);
                        }
                        pending.push((*a, *b));
                    }
                }
                _ => {
                    if !self.equal(a, b)? {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }
    pub(crate) fn test_show(
        &mut self,
        value: Slot,
        objects: &[okkama_sparrow::ObjectDecl],
    ) -> Result<String, Error> {
        enum Part {
            Value(Slot, bool),
            Text(&'static str),
        }
        let mut pending = vec![Part::Value(value, true)];
        let mut out = String::new();
        while let Some(part) = pending.pop() {
            let Part::Value(value, quoted) = part else {
                if let Part::Text(text) = part {
                    out.push_str(text);
                }
                continue;
            };
            match value.tag {
                ANY => pending.push(Part::Value(*self.boxes.get(value.bits)?, quoted)),
                STR => {
                    if quoted {
                        out.push('"');
                    }
                    out.push_str(self.text(value)?);
                    if quoted {
                        out.push('"');
                    }
                }
                BOOL => out.push_str(if value.bits == 0 { "false" } else { "true" }),
                BYTE => out.push_str(&(value.bits as u8 as i8).to_string()),
                CHAR => out.push_str(&value.bits.to_string()),
                UNBOUND => out.push_str("null"),
                OBJECT => out.push_str(&format!(
                    "{}@{:x}",
                    objects[self.object_descriptor(value)?].name,
                    value.bits
                )),
                ENUM => {
                    let name = self
                        .enum_names
                        .as_ref()
                        .and_then(|names| names.get((value.bits >> 32) as usize))
                        .and_then(|names| names.get(value.bits as u32 as usize))
                        .ok_or_else(|| Error("invalid enum name".into()))?;
                    out.push_str(name);
                }
                ARRAY => {
                    out.push('[');
                    pending.push(Part::Text("]"));
                    let values = self.elements(value)?;
                    for (i, &value) in values.iter().enumerate().rev() {
                        pending.push(Part::Value(value, false));
                        if i > 0 {
                            pending.push(Part::Text(", "));
                        }
                    }
                }
                MAP => {
                    out.push('{');
                    pending.push(Part::Text("}"));
                    for (i, &(key, value)) in self.maps.get(value.bits)?.values().enumerate().rev()
                    {
                        pending.push(Part::Value(value, false));
                        pending.push(Part::Text("="));
                        pending.push(Part::Value(key, false));
                        if i > 0 {
                            pending.push(Part::Text(", "));
                        }
                    }
                }
                _ => {
                    let text = self.convert(crate::scalar::Kind::Str, value)?;
                    out.push_str(self.text(text)?);
                }
            }
        }
        Ok(out)
    }
    pub(crate) fn test_operation(
        &mut self,
        operation: Operation,
        args: [Option<Slot>; 3],
        objects: &[okkama_sparrow::ObjectDecl],
    ) -> Result<Slot, Error> {
        if self.test_failures.is_none() {
            return Err(Error(
                "The test capability is available only inside a running test".into(),
            ));
        }
        let actual = args[0].ok_or_else(|| Error("missing assertion operand".into()))?;
        let message = match operation {
            Operation::Ok if actual.bits == 0 => Some("expected true, got false".into()),
            Operation::Ok => None,
            Operation::Fail => Some(self.text(actual)?.to_owned()),
            Operation::Is | Operation::Isnt => {
                let expected = args[1].ok_or_else(|| Error("missing assertion operand".into()))?;
                let equal = self.test_same(actual, expected)?;
                if matches!(operation, Operation::Is) && !equal {
                    Some(format!(
                        "expected {}, got {}",
                        self.test_show(expected, objects)?,
                        self.test_show(actual, objects)?
                    ))
                } else if matches!(operation, Operation::Isnt) && equal {
                    Some(format!(
                        "expected values to differ, both were {}",
                        self.test_show(actual, objects)?
                    ))
                } else {
                    None
                }
            }
        };
        self.test_failures
            .as_mut()
            .expect("active test")
            .extend(message);
        Ok(Slot::default())
    }
}

/// Run the module's compiled test profile and emit the standard textual report.
pub fn run_tests(
    program: &Program,
    mode: Mode,
    out: &mut dyn std::io::Write,
) -> Result<bool, Error> {
    run_tests_configured(program, mode, None, out)
}

/// Run a test profile with an explicit process-worker launcher.
pub fn run_tests_with_worker_command(
    program: &Program,
    mode: Mode,
    command: Vec<std::ffi::OsString>,
    out: &mut dyn std::io::Write,
) -> Result<bool, Error> {
    run_tests_configured(program, mode, Some(command), out)
}

fn run_tests_configured(
    program: &Program,
    mode: Mode,
    command: Option<Vec<std::ffi::OsString>>,
    out: &mut dyn std::io::Write,
) -> Result<bool, Error> {
    let mut cache = crate::LoadCache::new(program)?;
    let tests = program
        .tests
        .as_ref()
        .ok_or_else(|| Error("module has no tests".into()))?;
    if tests.is_empty() {
        return Err(Error("Test run requires at least one test".into()));
    }
    let host = crate::AsyncHost::new()?;
    let mut passed = true;
    let io = |e: std::io::Error| Error(format!("test report: {e}"));
    for (name, function) in tests {
        writeln!(out, "=== RUN   {name}").map_err(io)?;
        out.flush().map_err(io)?;
        let declaration = &program.functions[function.0 as usize];
        let loaded = if !declaration.params.is_empty() || declaration.result.is_some() {
            Err(Error("invalid test entry signature".into()))
        } else {
            Executable::load_cached(program, mode, function.0 as usize, &mut cache).and_then(
                |code| match &command {
                    Some(command) => code.with_worker_command(command.clone()),
                    None => Ok(code),
                },
            )
        };
        let failures = match loaded {
            Err(error) => vec![error.0],
            Ok(code) => {
                let mut session = code.start();
                session.begin_test();
                let outcome = host.block_on(session.complete_slot())?;
                let mut failures = session.finish_test();
                if let Err(error) = outcome {
                    failures.push(error.0);
                }
                failures
            }
        };
        if failures.is_empty() {
            writeln!(out, "--- PASS: {name}").map_err(io)?;
        } else {
            passed = false;
            writeln!(out, "--- FAIL: {name}").map_err(io)?;
            for failure in failures {
                writeln!(out, "    {}", failure.replace('\n', "\n    ")).map_err(io)?;
            }
        }
        out.flush().map_err(io)?;
    }
    writeln!(out, "{}", if passed { "PASS" } else { "FAIL" }).map_err(io)?;
    out.flush().map_err(io)?;
    Ok(passed)
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::rc::Rc;
    #[test]
    fn module_cache_reuses_immutable_plans_but_test_state_stays_isolated() {
        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/test-profile.asm")).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let mut cache = crate::LoadCache::new(&program).unwrap();
            let first = Executable::load_cached(&program, mode, 0, &mut cache).unwrap();
            let strings = cache.strings.len();
            let second = Executable::load_cached(&program, mode, 0, &mut cache).unwrap();
            assert_eq!(cache.strings.len(), strings);
            assert!(Rc::ptr_eq(
                first.code.functions[0].as_ref().unwrap(),
                second.code.functions[0].as_ref().unwrap()
            ));
            assert!(
                cache.functions[0]
                    .as_ref()
                    .unwrap()
                    .compiled
                    .iter()
                    .all(|c| c.get().is_none())
            );
            let host = crate::AsyncHost::new().unwrap();
            for code in [first, second] {
                let mut session = code.start();
                session.begin_test();
                host.block_on(session.complete_slot()).unwrap().unwrap();
                assert_eq!(session.finish_test().len(), 3);
                assert_eq!(
                    cache.functions[0]
                        .as_ref()
                        .unwrap()
                        .compiled
                        .iter()
                        .any(|c| c.get().is_some()),
                    matches!(mode, Mode::Jit)
                );
            }
        }
    }
    #[test]
    fn a_cached_caller_does_not_hide_an_unsupported_dependency() {
        let mut program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/test-profile.asm")).unwrap();
        program.functions[2].native = Some("runtime/unavailable@1::missing/0".into());
        for mode in [Mode::Cold, Mode::Jit] {
            let mut cache = crate::LoadCache::new(&program).unwrap();
            for _ in 0..2 {
                assert!(Executable::load_cached(&program, mode, 0, &mut cache).is_err());
            }
            assert!(cache.functions[0].is_some());
        }
    }
}
