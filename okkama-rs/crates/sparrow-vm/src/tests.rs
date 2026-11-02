//! Встроенный исполнитель тестов (§14) — как `okkama.rt.Tests` JVM: тесты
//! корня по порядку; провал утверждения не прерывает тест, отказ — только его.
use crate::machine::{Engine, Located, cold_call, describe_failure, execute_function, into_failure, new_engine, public_error};
use crate::{Error, OperationError};
use sparrow_runtime::failure::{Failure, Kind};
use sparrow_runtime::{Value, sched};
use okkama_sparrow::Program;
use std::io::Write;
use std::rc::Rc;

/// Текст значения в сообщении утверждения.
pub(crate) fn show(engine: &Engine, value: &Value) -> String {
    let case = |declaration: usize, case: usize| engine.case_name(declaration, case);
    let object = |declaration: usize| engine.program.objects.get(declaration).map(|object| object.name.clone());
    sparrow_runtime::testing::show(&sparrow_runtime::testing::Names { case: &case, object: &object }, value)
}

/// `Rt.describe`: текст отказа теста.
fn describe(engine: &Rc<Engine>, failure: &Rc<Failure>) -> Result<String, Located> {
    Ok(match &failure.kind {
        Kind::Thrown { .. } => sparrow_runtime::failure::describe(&*describe_failure(engine, failure)?),
        _ => sparrow_runtime::failure::describe(failure),
    })
}

fn io(error: std::io::Error) -> Error {
    Error::Operation {
        function: String::new(),
        instruction: 0,
        error: OperationError::InvalidState {
            message: format!("test report: {error}"),
        },
    }
}

/// Тесты программы по порядку; результат — прошли ли все. Отчёт — в `out`.
pub fn run_tests(program: &Program, out: &mut dyn Write) -> Result<bool, Error> {
    sched::on_large_stack(|| run_tests_here(program, out))
}

fn run_tests_here(program: &Program, out: &mut dyn Write) -> Result<bool, Error> {
    program.verify().map_err(Error::InvalidProgram)?;
    let Some(tests) = program.tests.clone() else {
        return Err(Error::Operation {
            function: String::new(),
            instruction: 0,
            error: OperationError::Unsupported {
                message: "module has no tests".into(),
            },
        });
    };
    let engine = new_engine(program);
    if sparrow_runtime::worker::active() {
        crate::machine::serve(&engine);
    }
    if tests.is_empty() {
        return Err(Error::Operation {
            function: String::new(),
            instruction: 0,
            error: OperationError::Trap {
                message: "Test run requires at least one test".into(),
            },
        });
    }
    let mut passed = true;
    sched::reset();
    for (name, function) in &tests {
        let function = function.0 as usize;
        writeln!(out, "=== RUN   {name}").map_err(io)?;
        out.flush().map_err(io)?;
        sparrow_runtime::testing::begin();
        let outcome = if engine.program.functions[function].is_async {
            let body = cold_call(&engine, function, Vec::new());
            sched::run_test(Some(&body))
        } else {
            match execute_function(&engine, function, Vec::new()) {
                Ok(_) => sched::run_test(None),
                Err(located) => match into_failure(located.stop) {
                    Ok(failure) => Err(failure),
                    Err(stop) => {
                        return Err(public_error(
                            &engine,
                            Located {
                                function: located.function,
                                pc: located.pc,
                                stop,
                            },
                        ));
                    }
                },
            }
        };
        let failure = match outcome {
            Ok(()) => None,
            Err(failure) => Some(describe(&engine, &failure).map_err(|located| public_error(&engine, located))?),
        };
        if let Err(fatal) = engine.take_fatal() {
            return Err(public_error(&engine, fatal));
        }
        let mut failures = sparrow_runtime::testing::finish();
        failures.extend(failure);
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
    sched::reset();
    writeln!(out, "{}", if passed { "PASS" } else { "FAIL" }).map_err(io)?;
    out.flush().map_err(io)?;
    Ok(passed)
}
