//! Исполнение ячеек REPL (`okkama repl`) на IR-интерпретаторе.
//!
//! Каждая ячейка — отдельная программа: её служебный объект получает прошлые
//! переменные параметрами действия `run`, а `run` возвращает `@any` из
//! объектов `Keeper<T>` с переменными ячейки. Имя экземпляра (`Keeper<@int>`)
//! даёт тип переменной в синтаксисе исходника. Номера объявлений у следующей
//! программы могут быть другими, поэтому значение переносится по именам
//! объектов и enum. Замыкания и Future не переносятся: они ссылаются на
//! функции своей программы.
use crate::{RuntimeError, Site, interpreter, value::Value};
use okkama_compiler::ir;
use okkama_runtime_core::heap::{ArrayHeap, CANCELLED, Map, MapKey, ObjectHeap};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// Имена объявлений программы, в которой создано значение.
#[derive(Debug, PartialEq, Eq)]
struct Names {
    objects: Vec<String>,
    enums: Vec<String>,
}

impl Names {
    fn of(module: &ir::Module) -> Self {
        Self {
            objects: module.objects.iter().map(|object| object.name.clone()).collect(),
            enums: module.enums.iter().map(|decl| decl.name.clone()).collect(),
        }
    }
}

/// Значение переменной REPL вместе с именами объявлений его программы.
#[derive(Clone, Debug)]
pub struct Carried {
    value: Value,
    names: Rc<Names>,
}

impl Carried {
    /// Значение можно передать в следующую ячейку: в нём нет замыканий и Future.
    pub fn portable(&self) -> bool {
        portable(&self.value)
    }
}

/// Переменная, которую вернула ячейка: тип в синтаксисе исходника и значение.
#[derive(Clone, Debug)]
pub struct Kept {
    pub ty: String,
    pub value: Carried,
}

/// Исполнить ячейку: `<cell>.new#0`, затем `<cell>.run(args)`. `keeper` —
/// имя служебного generic-объекта, в который `run` упаковывает переменные.
pub fn run_cell(
    module: &ir::Module,
    cell: &str,
    keeper: &str,
    args: &[Carried],
) -> (Result<Vec<Kept>, RuntimeError>, Vec<Site>) {
    let mut trace = Vec::new();
    let result = run_inner(module, cell, keeper, args, &mut trace);
    (result, trace)
}

fn run_inner(
    module: &ir::Module,
    cell: &str,
    keeper: &str,
    args: &[Carried],
    trace: &mut Vec<Site>,
) -> Result<Vec<Kept>, RuntimeError> {
    let find = |symbol: String| {
        module
            .functions
            .iter()
            .position(|function| function.symbol == symbol)
            .map(ir::FuncId)
            .ok_or_else(|| crate::invalid_state(module.span, format!("REPL function {symbol} is missing")))
    };
    let ctor = find(format!("{cell}.new#0"))?;
    let run = find(format!("{cell}.run"))?;
    let names = Rc::new(Names::of(module));
    let lookup = Lookup::new(&names);
    let mut arguments = Vec::with_capacity(args.len());
    for arg in args {
        let value = if *arg.names == *names {
            arg.value.clone()
        } else {
            remap(&arg.value, &arg.names, &lookup)
                .ok_or_else(|| crate::invalid_state(module.span, "REPL value cannot be carried into this input"))?
        };
        arguments.push(value);
    }
    let result = interpreter::execute_cell(module, ctor, run, arguments, trace)?;
    let Value::Array(items) = plain(&result) else {
        return Err(crate::invalid_state(module.span, "REPL cell must return @any"));
    };
    let prefix = format!("{keeper}<");
    items
        .0
        .iter()
        .map(|item| {
            let Value::Object(object) = plain(item) else {
                return Err(crate::invalid_state(module.span, "REPL cell returned a non-keeper value"));
            };
            let name = names.objects.get(object.declaration).map_or("", String::as_str);
            let ty = name
                .strip_prefix(&prefix)
                .and_then(|rest| rest.strip_suffix('>'))
                .ok_or_else(|| crate::invalid_state(module.span, "REPL cell returned a non-keeper value"))?;
            Ok(Kept {
                ty: ty.to_string(),
                value: Carried {
                    value: object.field(0).unwrap_or(Value::Undef),
                    names: names.clone(),
                },
            })
        })
        .collect()
}

fn plain(value: &Value) -> &Value {
    match value {
        Value::Any(inner) => plain(inner),
        other => other,
    }
}

fn portable(value: &Value) -> bool {
    match value {
        Value::Fn(_) | Value::Future(_) | Value::Builder(_) | Value::MapBuilder(_) | Value::Region(_) => false,
        Value::Any(inner) => portable(inner),
        Value::Array(array) => array.0.iter().all(portable),
        Value::Map(map) => map.0.values().all(portable),
        Value::Object(object) => object.values().iter().all(portable),
        _ => true,
    }
}

/// Номера объявлений новой программы по именам.
struct Lookup {
    objects: HashMap<String, usize>,
    enums: HashMap<String, usize>,
}

impl Lookup {
    fn new(names: &Names) -> Self {
        let index = |list: &[String]| list.iter().enumerate().map(|(index, name)| (name.clone(), index)).collect();
        Self {
            objects: index(&names.objects),
            enums: index(&names.enums),
        }
    }
}

/// Значение с номерами объявлений новой программы; `None` — объявления там нет
/// или значение не переносится.
fn remap(value: &Value, old: &Names, new: &Lookup) -> Option<Value> {
    Some(match value {
        Value::Object(object) => {
            let declaration = if object.declaration == CANCELLED {
                CANCELLED
            } else {
                *new.objects.get(old.objects.get(object.declaration)?)?
            };
            let fields = (0..object.field_count())
                .map(|index| match object.field(index) {
                    Some(Value::Undef) | None => Some(Value::Undef),
                    Some(field) => remap(&field, old, new),
                })
                .collect::<Option<Vec<_>>>()?;
            ObjectHeap.allocate(declaration, fields)
        }
        Value::Enum { declaration, case } => Value::Enum {
            declaration: *new.enums.get(old.enums.get(*declaration)?)?,
            case: *case,
        },
        Value::Array(array) => {
            ArrayHeap.allocate(array.0.iter().map(|item| remap(item, old, new)).collect::<Option<Vec<_>>>()?)
        }
        Value::Map(map) => {
            let mut entries = BTreeMap::new();
            for (key, item) in &map.0 {
                let key = match key {
                    MapKey::Enum { declaration, case } => MapKey::Enum {
                        declaration: *new.enums.get(old.enums.get(*declaration)?)?,
                        case: *case,
                    },
                    other => other.clone(),
                };
                entries.insert(key, remap(item, old, new)?);
            }
            Value::Map(Rc::new(Map(entries)))
        }
        Value::Any(inner) => Value::Any(Rc::new(remap(inner, old, new)?)),
        Value::Fn(_) | Value::Future(_) | Value::Builder(_) | Value::MapBuilder(_) | Value::Region(_) => return None,
        other => other.clone(),
    })
}
