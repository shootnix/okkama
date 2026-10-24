//! Встроенный исполнитель тестов (§14) — как `okkama.rt.Tests` JVM: тесты
//! корня по порядку, каждый в своём состоянии; провал утверждения не
//! прерывает тест, отказ — только его. Записи `=== RUN` / `--- PASS` /
//! `--- FAIL`; успех — только если прошли все.
use crate::interpreter::{Engine, cold_call, describe_failure, execute_function};
use crate::{RuntimeError, invalid_state, unsupported};
use okkama_compiler::ir;
use okkama_runtime_core::failure::{Failure, Kind};
use okkama_runtime_core::{Value, sched};
use std::io::Write;
use std::rc::Rc;

pub(crate) fn fail(message: String) -> Result<(), Rc<Failure>> {
    okkama_runtime_core::testing::fail(message)
}

/// Текст значения в сообщении утверждения (`okkama_runtime_core::testing::show`).
pub(crate) fn show(module: &ir::Module, value: &Value) -> String {
    let case = |declaration: usize, case: usize| module.enums[declaration].cases[case].clone();
    let object = |declaration: usize| module.objects.get(declaration).map(|object| object.name.clone());
    okkama_runtime_core::testing::show(&okkama_runtime_core::testing::Names { case: &case, object: &object }, value)
}

/// `Rt.describe`: текст отказа теста.
fn describe(engine: &Rc<Engine>, failure: &Rc<Failure>) -> Result<String, RuntimeError> {
    Ok(match &failure.kind {
        Kind::Native(native) => format!(
            "NativeError: {}\ncode: {}\ndomain: {}\noperation: {}",
            native.message, native.code, native.domain, native.operation
        ),
        Kind::Thrown { .. } => {
            let described = describe_failure(engine, failure, engine.module.span)?;
            let Kind::Thrown { name, message, .. } = &described.kind else {
                unreachable!("described failure keeps its kind")
            };
            format!("{name}: {}", message.as_deref().unwrap_or(""))
        }
        Kind::Panic(message) => format!("Panic: {message}"),
        Kind::Program(message) => format!("RuntimeError: {message}"),
    })
}

/// Тесты модуля по порядку; результат — прошли ли все. Отчёт — в `out`.
pub fn run_tests(module: &ir::Module, out: &mut dyn Write) -> Result<bool, RuntimeError> {
    sched::on_large_stack(|| run_tests_here(module, out))
}

fn run_tests_here(module: &ir::Module, out: &mut dyn Write) -> Result<bool, RuntimeError> {
    ir::verify(module).map_err(RuntimeError::InvalidIr)?;
    let tests = module
        .tests
        .clone()
        .ok_or_else(|| unsupported(module.span, "module has no tests"))?;
    let engine = crate::interpreter::new_engine(module);
    if crate::worker::active() {
        crate::worker::serve(&engine);
    }
    let io = |error: std::io::Error| invalid_state(module.span, format!("test report: {error}"));
    if tests.is_empty() {
        return Err(RuntimeError::Trap {
            span: module.span,
            message: "Test run requires at least one test".into(),
        });
    }
    let mut passed = true;
    sched::reset();
    for (name, func) in &tests {
        writeln!(out, "=== RUN   {name}").map_err(io)?;
        out.flush().map_err(io)?;
        okkama_runtime_core::testing::begin();
        let outcome = if engine.module.functions[func.0].is_async() {
            let body = cold_call(&engine, *func, Vec::new());
            sched::run_test(Some(&body))
        } else {
            match execute_function(&engine, *func, Vec::new()) {
                Ok(_) => sched::run_test(None),
                Err(error) => Err(crate::interpreter::into_failure(error)?),
            }
        };
        let failure = match outcome {
            Ok(()) => None,
            Err(failure) => Some(describe(&engine, &failure)?),
        };
        crate::interpreter::take_fatal(&engine)?;
        let mut failures = okkama_runtime_core::testing::finish();
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

#[cfg(test)]
mod unit {
    use okkama_runtime_core::testing::java_double;

    #[test]
    fn doubles_print_like_java() {
        for (value, text) in [
            (1.0, "1.0"),
            (1.5, "1.5"),
            (1e7, "1.0E7"),
            (1234567.0, "1234567.0"),
            (0.001, "0.001"),
            (1e-4, "1.0E-4"),
            (-2.5e-10, "-2.5E-10"),
            (100.0, "100.0"),
        ] {
            assert_eq!(java_double(value), text);
        }
    }
}
