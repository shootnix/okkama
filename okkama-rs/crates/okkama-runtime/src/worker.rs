//! Воркеры `spawn` исполнителя IR над общим протоколом
//! `okkama_runtime_core::worker`: символ входа, имена типов и исполнение
//! запроса в дочернем процессе.
use crate::interpreter::{Engine, describe_failure, execute_function, into_failure};
use okkama_compiler::ir;
use okkama_runtime_core::failure::Failure;
use okkama_runtime_core::{Value, sched, worker};
use std::rc::Rc;

/// Команда запуска той же программы для воркера (`okkama run …`).
pub fn set_worker_command(command: Vec<String>) {
    worker::set_command(command);
}

pub(crate) fn active() -> bool {
    worker::active()
}

fn type_name(module: &ir::Module, value: &Value) -> String {
    match value {
        Value::Rat(_) => "rat".into(),
        Value::Enum { declaration, .. } => module.enums.get(*declaration).map_or_else(|| "enum".into(), |e| e.name.clone()),
        Value::Future(_) => "Future".into(),
        Value::Fn(_) => "action".into(),
        Value::Any(_) => "any".into(),
        Value::Object(object) => module
            .objects
            .get(object.declaration)
            .map_or_else(|| "object".into(), |declaration| declaration.name.rsplit(['.', '$']).next().unwrap_or("").to_string()),
        Value::Handle(_) => "NativeHandle".into(),
        Value::Region(_) => "Region".into(),
        Value::Range(_) => "Range".into(),
        _ => "value".into(),
    }
}

pub(crate) fn spawn(engine: &Rc<Engine>, entry: &Value, args: Vec<Value>, direct: bool) -> Result<Value, Rc<Failure>> {
    let module = &engine.module;
    let symbol = |function: usize| module.functions.get(function).map(|function| function.symbol.clone());
    let (target, all) = worker::target(entry, args, direct, &symbol)?;
    worker::spawn(target, all, &|value| type_name(module, value))
}

/// Исполнить запрос родителя и завершить процесс; `main` не идёт.
pub(crate) fn serve(engine: &Rc<Engine>) -> ! {
    let module = &engine.module;
    worker::serve(
        |target, args| {
            let function = module
                .functions
                .iter()
                .position(|function| function.symbol == target)
                .ok_or_else(|| format!("runtime: unknown spawn entry {target}"))?;
            let span = module.span;
            let described = |failure: Rc<Failure>| -> String {
                match describe_failure(engine, &failure, span) {
                    Ok(failure) => format!("runtime: {}", crate::describe(&failure)),
                    Err(error) => format!("runtime: {error}"),
                }
            };
            match sched::on_large_stack(|| execute_function(engine, ir::FuncId(function), args)) {
                Ok(value) => Ok(value.unwrap_or(Value::Undef)),
                Err(error) => Err(match into_failure(error) {
                    Ok(failure) => described(failure),
                    Err(fatal) => format!("runtime: {fatal}"),
                }),
            }
        },
        &|value| type_name(module, value),
    )
}

pub(crate) fn bind(action: &Value, entry: Value, fields: Vec<Value>) -> Result<Value, String> {
    worker::bind(action, entry, fields)
}
