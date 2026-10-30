//! `runtime/debug`: описание одного значения для `debug/dumper` — вид, тип,
//! размер и части. Значение не меняется; многострочный дамп собирает Okkama.
//! Тексты совпадают с `MiscNatives` JVM: дамп одинаков на всех движках.
use super::support::{array, int_value, text};
use super::{Context, Error};
use crate::{Value, abi::Native, heap::CANCELLED};
use std::rc::Rc;

/// Поля системного `NativeFailure` по порядку (`ir::Object::NATIVE_FAILURE_FIELDS`).
const NATIVE_FAILURE_FIELDS: [&str; 4] = ["domain", "code", "operation", "message"];

pub(super) fn execute(name: &str, native: Native, value: &Value, context: &Context) -> Result<Value, Error> {
    let value = plain(value);
    Ok(match name {
        "data_kind" => text(kind(value)),
        "data_type" => text(type_name(context, value)),
        "data_size" => int_value(size(context, value) as i64),
        "data_leaf" => text(leaf(native, context, value)?),
        "data_names" => array(names(context, value).into_iter().map(text).collect()),
        "data_keys" => array(keys(value).into_iter().map(boxed).collect()),
        "data_items" => array(items(value).into_iter().map(boxed).collect()),
        _ => return Err(super::support::failed(native)),
    })
}

/// Значение под всеми `any`.
fn plain(value: &Value) -> &Value {
    match value {
        Value::Any(inner) => plain(inner),
        other => other,
    }
}

/// Элемент результата `@any`.
fn boxed(value: Value) -> Value {
    match value {
        Value::Any(_) => value,
        other => Value::Any(Rc::new(other)),
    }
}

fn kind(value: &Value) -> &'static str {
    match plain(value) {
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Rat(_) => "rat",
        Value::Bool(_) => "bool",
        Value::Byte(_) => "byte",
        Value::Char(_) => "char",
        Value::Str(_) => "string",
        Value::Enum { .. } => "enum",
        Value::Array(_) => "array",
        Value::Map(_) => "map",
        Value::Object(_) | Value::NativeFailure(_) => "object",
        Value::Fn(_) => "action",
        Value::Future(_) => "future",
        Value::Failure(_) => "failure",
        _ => "native",
    }
}

fn type_name(context: &Context, value: &Value) -> String {
    match plain(value) {
        Value::Enum { declaration, .. } => context
            .enums
            .get(*declaration)
            .map_or_else(|| "enum".to_string(), |(name, _)| name.clone()),
        Value::Object(object) => object_name(context, object.declaration),
        Value::NativeFailure(_) => "NativeFailure".to_string(),
        Value::Array(array) => format!("@{}", common(context, array.0.iter())),
        Value::Map(map) => {
            let keys: Vec<Value> = map.0.keys().map(|key| key.to_value()).collect();
            format!("%{}:{}", common(context, keys.iter()), common(context, map.0.values()))
        }
        other => kind(other).to_string(),
    }
}

/// Общий тип элементов: один — он сам, нет — `?`, разные — `(A | B)`.
fn common<'a>(context: &Context, items: impl Iterator<Item = &'a Value>) -> String {
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        let name = type_name(context, item);
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    match seen.len() {
        0 => "?".to_string(),
        1 => seen.remove(0),
        _ => format!("({})", seen.join(" | ")),
    }
}

fn object_name(context: &Context, declaration: usize) -> String {
    if declaration == CANCELLED {
        return "Cancelled".to_string();
    }
    context
        .objects
        .get(declaration)
        .map_or_else(|| "object".to_string(), |(name, _)| short_type(name))
}

/// Имя типа без модулей: `io.stdout::Stdout` → `Stdout`,
/// `Box<io.stdout::Stdout>` → `Box<Stdout>`.
pub fn short_type(name: &str) -> String {
    let mut out = String::new();
    let mut token = 0;
    let mut chars = name.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == ':' && name[index..].starts_with("::") {
            out.truncate(token);
            chars.next();
        } else if ch.is_alphanumeric() || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push(ch);
            token = out.len();
        }
    }
    out
}

fn size(context: &Context, value: &Value) -> usize {
    match plain(value) {
        Value::Array(array) => array.0.len(),
        Value::Map(map) => map.0.len(),
        Value::Str(text) => text.chars().count(),
        Value::Object(_) | Value::NativeFailure(_) => names(context, value).len(),
        _ => 0,
    }
}

fn leaf(native: Native, context: &Context, value: &Value) -> Result<String, Error> {
    Ok(match plain(value) {
        value @ (Value::Int(_)
        | Value::Float(_)
        | Value::Rat(_)
        | Value::Bool(_)
        | Value::Byte(_)
        | Value::Char(_)
        | Value::Str(_)
        | Value::Enum { .. }) => super::misc::dump(native, context, value)?,
        Value::Fn(_) => "<action>".to_string(),
        Value::Future(_) => "<future>".to_string(),
        Value::Failure(failure) => format!("<failure: {}>", crate::failure::describe(failure)),
        Value::Array(_) | Value::Map(_) | Value::Object(_) | Value::NativeFailure(_) => {
            format!("<{} of {}>", type_name(context, value), size(context, value))
        }
        _ => "<native>".to_string(),
    })
}

fn names(context: &Context, value: &Value) -> Vec<String> {
    match plain(value) {
        Value::Object(object) => match context.objects.get(object.declaration) {
            Some((_, fields)) => fields.clone(),
            None => (0..object.field_count()).map(|index| format!("#{index}")).collect(),
        },
        Value::NativeFailure(_) => NATIVE_FAILURE_FIELDS.map(String::from).to_vec(),
        _ => Vec::new(),
    }
}

fn keys(value: &Value) -> Vec<Value> {
    match plain(value) {
        Value::Map(map) => map.0.keys().map(|key| key.to_value()).collect(),
        _ => Vec::new(),
    }
}

fn items(value: &Value) -> Vec<Value> {
    match plain(value) {
        Value::Array(array) => array.0.clone(),
        Value::Map(map) => map.0.values().cloned().collect(),
        Value::Object(object) => (0..object.field_count())
            .map(|index| object.field(index).unwrap_or(Value::Undef))
            .collect(),
        Value::NativeFailure(failure) => [&failure.domain, &failure.code, &failure.operation, &failure.message]
            .into_iter()
            .map(|field| text(field.clone()))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::short_type;

    #[test]
    fn type_names_drop_module_prefixes() {
        assert_eq!(short_type("Point"), "Point");
        assert_eq!(short_type("io.stdout::Stdout"), "Stdout");
        assert_eq!(short_type("Box<io.stdout::Stdout>"), "Box<Stdout>");
        assert_eq!(short_type("Pair<int, a.b::C>"), "Pair<int, C>");
    }
}
