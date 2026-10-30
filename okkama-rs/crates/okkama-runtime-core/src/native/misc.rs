//! Небольшие natives std — перенос `MiscNatives` JVM: `runtime/crypto`,
//! `runtime/secret`, `runtime/debug`, `runtime/diff`, `runtime/math_aggregate`,
//! `runtime/url_text`, `runtime/layout`. Тексты отказов — как у Perl-provider.
use super::support::{
    byte_array, bytes, failure, float, handle, int, int_value, items, new_handle, string, text,
};
use super::{Context, Error};
use crate::{Value, abi::Native};
use std::cell::RefCell;
use std::collections::HashMap;

fn operation(native: Native) -> &'static str {
    native
        .symbol()
        .split("::")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("")
}

fn length(text: &str) -> usize {
    text.chars().count()
}

// ---- runtime/crypto ----

fn digest_with(native: Native, algorithm: &str, key: &[u8], data: &[u8]) -> Result<Vec<u8>, Error> {
    let unsupported = || {
        let what = if key.is_empty() { "digest" } else { "HMAC" };
        failure("core.crypto", "unsupported", native, format!("Unsupported {what} algorithm"))
    };
    let mut hasher = okkama_native::Hasher::new(algorithm, key, 0).map_err(|_| unsupported())?;
    hasher.update(data);
    hasher.finish(0).map_err(|_| unsupported())
}

// ---- runtime/secret ----

#[derive(Debug, Default)]
struct Secret {
    bytes: RefCell<Option<Vec<u8>>>,
}

fn open_secret(native: Native, value: &Value) -> Result<Vec<u8>, Error> {
    handle::<Secret>(value)?.bytes.borrow().clone().ok_or_else(|| {
        failure(
            "core.secret",
            "already_closed",
            native,
            "Native handle is already closed",
        )
    })
}

// ---- runtime/debug ----

fn escaped(value: &str, quote: char) -> String {
    let mut out = String::new();
    out.push(quote);
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            '\u{00}'..='\u{08}' | '\u{0b}' | '\u{0c}' | '\u{0e}'..='\u{1f}' | '\u{7f}' => {
                out.push_str(&format!("\\x{{{:02X}}}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

pub(super) fn dump(native: Native, context: &Context, value: &Value) -> Result<String, Error> {
    Ok(match value {
        Value::Any(inner) => dump(native, context, inner)?,
        Value::Int(value) => value.to_string(),
        Value::Float(value) => {
            let text = crate::float::format_g(*value, 15);
            if text.contains(['.', 'e', 'E']) {
                text
            } else {
                format!("{text}.0")
            }
        }
        Value::Rat(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Str(value) => escaped(value, '"'),
        Value::Char(value) => escaped(&value.to_string(), '\''),
        Value::Byte(value) => format!("byte({value})"),
        Value::Enum { declaration, case } => {
            let (name, cases) = context
                .enums
                .get(*declaration)
                .ok_or_else(|| Error::InvalidState("enum names are missing".into()))?;
            format!("{name}.{}", cases[*case])
        }
        Value::Array(array) => {
            let parts = array
                .0
                .iter()
                .map(|item| dump(native, context, item))
                .collect::<Result<Vec<_>, _>>()?;
            format!("[{}]", parts.join(", "))
        }
        Value::Map(map) => {
            let parts = map
                .0
                .iter()
                .map(|(key, item)| {
                    Ok(format!(
                        "{}: {}",
                        dump(native, context, &key.to_value())?,
                        dump(native, context, item)?
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            format!("[{}]", parts.join(", "))
        }
        _ => return Err(super::support::failed(native)),
    })
}

// ---- runtime/diff (Algorithm::Diff LCSidx) ----

/// `_replaceNextLargerWith`: индекс замены или None.
fn replace_next_larger(array: &mut Vec<usize>, value: usize, high: Option<usize>) -> Option<usize> {
    let top = match high {
        None | Some(0) => array.len() as i64 - 1,
        Some(high) => high as i64,
    };
    if top == -1 || value > *array.last().expect("non-empty when top >= 0") {
        array.push(value);
        return Some((top + 1) as usize);
    }
    let (mut low, mut top) = (0i64, top);
    while low <= top {
        let index = ((top + low) / 2) as usize;
        let found = array[index];
        if value == found {
            return None;
        } else if value > found {
            low = index as i64 + 1;
        } else {
            top = index as i64 - 1;
        }
    }
    array[low as usize] = value;
    Some(low as usize)
}

struct Link {
    previous: Option<std::rc::Rc<Link>>,
    i: usize,
    j: usize,
}

fn lcs(a: &[String], b: &[String]) -> Vec<Option<usize>> {
    let mut matched = vec![None; a.len()];
    let (mut a_start, mut b_start) = (0i64, 0i64);
    let (mut a_finish, mut b_finish) = (a.len() as i64 - 1, b.len() as i64 - 1);
    while a_start <= a_finish && b_start <= b_finish && a[a_start as usize] == b[b_start as usize] {
        matched[a_start as usize] = Some(b_start as usize);
        a_start += 1;
        b_start += 1;
    }
    while a_start <= a_finish
        && b_start <= b_finish
        && a[a_finish as usize] == b[b_finish as usize]
    {
        matched[a_finish as usize] = Some(b_finish as usize);
        a_finish -= 1;
        b_finish -= 1;
    }
    let mut positions: HashMap<&str, Vec<usize>> = HashMap::new();
    for index in b_start..=b_finish {
        positions
            .entry(b[index as usize].as_str())
            .or_default()
            .insert(0, index as usize);
    }
    let mut thresh: Vec<usize> = Vec::new();
    let mut links: Vec<Option<std::rc::Rc<Link>>> = Vec::new();
    for i in a_start..=a_finish {
        let Some(matches) = positions.get(a[i as usize].as_str()) else {
            continue;
        };
        let mut k: Option<usize> = Some(0);
        for &j in matches {
            match k {
                Some(at) if at != 0 && thresh[at] > j && thresh[at - 1] < j => thresh[at] = j,
                _ => k = replace_next_larger(&mut thresh, j, k),
            }
            if let Some(at) = k {
                while links.len() <= at {
                    links.push(None);
                }
                let previous = if at > 0 { links[at - 1].clone() } else { None };
                links[at] = Some(std::rc::Rc::new(Link {
                    previous,
                    i: i as usize,
                    j,
                }));
            }
        }
    }
    if !thresh.is_empty() {
        let mut link = links[thresh.len() - 1].clone();
        while let Some(current) = link {
            matched[current.i] = Some(current.j);
            link = current.previous.clone();
        }
    }
    matched
}

fn diff_lines(native: Native, value: &Value) -> Result<Vec<String>, Error> {
    let values = items(value)?;
    let invalid = |message: &str| failure("core.diff", "invalid_data", native, message);
    if values.len() > 512 {
        return Err(invalid("Diff requires at most 512 lines"));
    }
    let mut out = Vec::new();
    let mut scalars = 0;
    for value in values {
        let line = value.as_string()?;
        let size = length(line);
        if size > 4096 || line.contains('\n') {
            return Err(invalid("Invalid diff line"));
        }
        scalars += size;
        out.push(line.to_string());
    }
    if scalars > 1_048_576 {
        return Err(invalid("Diff input exceeds 1048576 scalars"));
    }
    Ok(out)
}

fn diff_script(left: &[String], right: &[String]) -> Vec<Value> {
    let matched = lcs(left, right);
    let mut script = Vec::new();
    let mut add = |kind: i64, x: usize, y: usize| {
        script.extend([int_value(kind), int_value(x as i64), int_value(y as i64)]);
    };
    let (mut x, mut y) = (0, 0);
    for (index, found) in matched.iter().enumerate() {
        let Some(next_y) = *found else {
            continue;
        };
        while x < index {
            add(1, x, y);
            x += 1;
        }
        while y < next_y {
            add(2, x, y);
            y += 1;
        }
        add(0, x, y);
        x += 1;
        y += 1;
    }
    while x < left.len() {
        add(1, x, y);
        x += 1;
    }
    while y < right.len() {
        add(2, x, y);
        y += 1;
    }
    script
}

// ---- runtime/url_text ----

fn dot_segments(native: Native, source: &str) -> Result<String, Error> {
    let invalid = |message: &str| failure("core.url_text", "invalid_input", native, message);
    if length(source) > 8192 {
        return Err(invalid("invalid bounded dot path"));
    }
    let parts: Vec<&str> = source.split('/').collect();
    if parts.len() > 1024 {
        return Err(invalid("dot path segment limit exceeded"));
    }
    let mut stack: Vec<String> = Vec::new();
    let mut history = false;
    for (at, part) in parts.iter().enumerate() {
        if *part == "." || *part == ".." {
            if *part == ".." {
                stack.pop();
            }
            if at == parts.len() - 1 && history {
                stack.push("/".into());
            }
        } else {
            stack.push(format!("{}{part}", if history { "/" } else { "" }));
            history = true;
        }
    }
    Ok(stack.concat())
}

// ---- runtime/layout ----

/// Строки с их окончаниями (`\r\n`, `\n`, `\r` или конец текста).
fn layout_lines(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = source;
    while !rest.is_empty() {
        match rest.find(['\r', '\n']) {
            Some(at) => {
                let ending = if rest[at..].starts_with("\r\n") { 2 } else { 1 };
                out.push((rest[..at].to_string(), rest[at..at + ending].to_string()));
                rest = &rest[at + ending..];
            }
            None => {
                out.push((rest.to_string(), String::new()));
                rest = "";
            }
        }
    }
    out
}

fn indent(origin: &str, prefix: &str) -> String {
    layout_lines(origin)
        .into_iter()
        .map(|(line, ending)| {
            let lead = if line.chars().any(|c| !c.is_whitespace()) {
                prefix
            } else {
                ""
            };
            format!("{lead}{line}{ending}")
        })
        .collect()
}

fn dedent(origin: &str) -> String {
    let mut lines = layout_lines(origin);
    let mut margin: Option<String> = None;
    for (line, _) in &mut lines {
        if line.chars().all(|c| c == ' ' || c == '\t') {
            line.clear();
            continue;
        }
        let current: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        margin = Some(match margin {
            None => current,
            Some(margin) => margin
                .chars()
                .zip(current.chars())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a)
                .collect(),
        });
    }
    let width = margin.map_or(0, |margin| margin.len());
    lines
        .into_iter()
        .map(|(line, ending)| {
            let body = if line.is_empty() { "" } else { &line[width..] };
            format!("{body}{ending}")
        })
        .collect()
}

fn normalized(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn execute(native: Native, args: &[Value], context: &Context) -> Result<Value, Error> {
    let name = operation(native);
    let module = native.symbol().split('@').next().unwrap_or("");
    match (module, name) {
        ("runtime/crypto", "digest") => {
            let algorithm = string(&args[0])?;
            let name = match algorithm {
                "sha256" | "sha384" | "sha512" => algorithm,
                "sha512256" => "sha512-256",
                _ => {
                    return Err(failure(
                        "core.crypto",
                        "unsupported",
                        native,
                        "Unsupported digest algorithm",
                    ));
                }
            };
            Ok(byte_array(digest_with(native, name, &[], &bytes(&args[1])?)?))
        }
        ("runtime/crypto", "hmac") => {
            let algorithm = string(&args[0])?;
            if !matches!(algorithm, "sha256" | "sha384" | "sha512") {
                return Err(failure(
                    "core.crypto",
                    "unsupported",
                    native,
                    "Unsupported HMAC algorithm",
                ));
            }
            let mut key = bytes(&args[1])?;
            // Пустой ключ HMAC дополняется нулями, как и ключ из одного нуля.
            if key.is_empty() {
                key.push(0);
            }
            Ok(byte_array(digest_with(
                native,
                &format!("hmac-{algorithm}"),
                &key,
                &bytes(&args[2])?,
            )?))
        }
        ("runtime/crypto", "secure_equal") => {
            let (left, right) = (bytes(&args[0])?, bytes(&args[1])?);
            let limit = left.len().max(right.len());
            let mut difference = left.len() ^ right.len();
            for index in 0..limit {
                let a = left.get(index).copied().unwrap_or(0);
                let b = right.get(index).copied().unwrap_or(0);
                difference |= usize::from(a ^ b);
            }
            Ok(Value::Bool(difference == 0))
        }
        ("runtime/secret", "create") => {
            if items(&args[0])?.len() > 1_048_576 {
                return Err(failure(
                    "core.secret",
                    "invalid_input",
                    native,
                    "secret payload exceeds limit",
                ));
            }
            Ok(new_handle(Secret {
                bytes: RefCell::new(Some(bytes(&args[0])?)),
            }))
        }
        ("runtime/secret", "size") => Ok(int_value(open_secret(native, &args[0])?.len() as i64)),
        ("runtime/secret", "export") => Ok(byte_array(open_secret(native, &args[0])?)),
        ("runtime/secret", "close") => {
            let state = handle::<Secret>(&args[0])?;
            if let Some(mut bytes) = state.bytes.borrow_mut().take() {
                bytes.fill(0);
            }
            Ok(Value::Bool(true))
        }
        ("runtime/secret", "closed") => Ok(Value::Bool(
            handle::<Secret>(&args[0])?.bytes.borrow().is_none(),
        )),
        ("runtime/debug", "data_dump") => Ok(text(dump(native, context, &args[0])?)),
        ("runtime/debug", name) => super::inspect::execute(name, native, &args[0], context),
        ("runtime/diff", "diff_script") => {
            let left = diff_lines(native, &args[0])?;
            let right = diff_lines(native, &args[1])?;
            Ok(super::support::array(diff_script(&left, &right)))
        }
        ("runtime/math_aggregate", "aggregate_square_root") => {
            let value = float(&args[0])?;
            if value < 0.0 {
                return Err(failure(
                    "core.math_aggregate",
                    "invalid_input",
                    native,
                    "Square-root input must be non-negative",
                ));
            }
            Ok(Value::Float(value.sqrt()))
        }
        ("runtime/url_text", "url_dot_segments") => {
            Ok(text(dot_segments(native, string(&args[0])?)?))
        }
        ("runtime/layout", "indent") => Ok(text(indent(string(&args[0])?, string(&args[1])?))),
        ("runtime/layout", "dedent") => Ok(text(dedent(string(&args[0])?))),
        ("runtime/layout", "wrap") => {
            let width = int(&args[1])?;
            if width.signum() <= 0 {
                return Err(failure(
                    "core.layout",
                    "invalid_argument",
                    native,
                    "wrap width must be positive",
                ));
            }
            let text_value = normalized(string(&args[0])?);
            if text_value.is_empty() {
                return Ok(text(""));
            }
            let width = width.to_i64().unwrap_or(i64::MAX);
            let mut out = Vec::new();
            let mut current = String::new();
            for word in text_value.split(' ') {
                let candidate = if current.is_empty() {
                    word.to_string()
                } else {
                    format!("{current} {word}")
                };
                if current.is_empty() || length(&candidate) as i64 <= width {
                    current = candidate;
                } else {
                    out.push(std::mem::replace(&mut current, word.to_string()));
                }
            }
            out.push(current);
            Ok(text(out.join("\n")))
        }
        ("runtime/layout", "shorten") => {
            let width = int(&args[1])?;
            let placeholder = string(&args[2])?;
            let invalid = |message: &str| failure("core.layout", "invalid_argument", native, message);
            if width.is_negative() {
                return Err(invalid("shorten width cannot be negative"));
            }
            let width = width.to_i64().unwrap_or(i64::MAX);
            if length(placeholder) as i64 > width {
                return Err(invalid("shorten placeholder exceeds width"));
            }
            let text_value = normalized(string(&args[0])?);
            if length(&text_value) as i64 <= width {
                return Ok(text(text_value));
            }
            let mut current = String::new();
            for word in text_value.split(' ') {
                let candidate = if current.is_empty() {
                    word.to_string()
                } else {
                    format!("{current} {word}")
                };
                if (length(&candidate) + length(placeholder)) as i64 > width {
                    break;
                }
                current = candidate;
            }
            Ok(text(if current.is_empty() {
                placeholder.to_string()
            } else {
                format!("{current}{placeholder}")
            }))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
