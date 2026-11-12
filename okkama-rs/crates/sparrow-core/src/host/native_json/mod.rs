//! JSON trees use a flat generational arena. Native handles retain node IDs;
//! parsing, writing, identity paths, GC traversal and reclamation are iterative.
mod parser;
use crate::host::*;
use num_bigint::BigInt;
use okkama_runtime_abi::{Native, NativeFailure};
use std::{
    cell::{OnceCell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};
#[derive(Debug)]
pub(crate) enum Node {
    Null,
    Boolean(bool),
    Number(Rc<str>),
    Text(Rc<str>),
    Array(Vec<u64>),
    Object(Names, Vec<u64>),
}
/// Below this size an object finds a member by a scan, which beats hashing.
const INDEXED_OBJECT: usize = 16;
/// Member names in document order. A large object finds a name through an
/// index built on the first lookup; otherwise walking `names()` with `get()`
/// is quadratic (#352).
#[derive(Debug)]
pub(crate) struct Names {
    list: Vec<Rc<str>>,
    // Boxed: few objects have an index, and every node pays for `Node`'s size.
    #[allow(clippy::box_collection)]
    index: OnceCell<Box<HashMap<Rc<str>, usize>>>,
}
impl Names {
    fn position(&self, sought: &str) -> Option<usize> {
        if self.list.len() < INDEXED_OBJECT {
            return self.list.iter().position(|name| &**name == sought);
        }
        let index = self.index.get_or_init(|| {
            let mut index = HashMap::with_capacity(self.list.len());
            for (at, name) in self.list.iter().enumerate() {
                index.entry(name.clone()).or_insert(at);
            }
            Box::new(index)
        });
        index.get(sought).copied()
    }
}
impl From<Vec<Rc<str>>> for Names {
    fn from(list: Vec<Rc<str>>) -> Self {
        Self { list, index: OnceCell::new() }
    }
}
impl std::ops::Deref for Names {
    type Target = [Rc<str>];
    fn deref(&self) -> &[Rc<str>] {
        &self.list
    }
}
impl Node {
    fn kind(&self) -> i64 {
        match self {
            Self::Null => 0,
            Self::Boolean(_) => 1,
            Self::Number(_) => 2,
            Self::Text(_) => 3,
            Self::Array(_) => 4,
            Self::Object(..) => 5,
        }
    }
    fn children(&self) -> &[u64] {
        match self {
            Self::Array(values) | Self::Object(_, values) => values,
            _ => &[],
        }
    }
    pub(crate) fn roots(&self, roots: &mut Vec<Slot>) {
        roots.extend(self.children().iter().map(|id| Slot {
            tag: JSON_NODE,
            bits: *id,
        }));
    }
}
#[derive(Debug)]
struct ParsedDocument {
    root: Option<u64>,
    offset: i64,
    line: i64,
    column: i64,
    message: String,
    duplicate: Option<String>,
}
#[derive(Debug)]
pub(crate) struct ParseState {
    source: Vec<char>,
    maximum_depth: BigInt,
    outcome: RefCell<Option<Rc<ParsedDocument>>>,
}
#[derive(Debug, Clone)]
pub(crate) enum JsonHandle {
    Node(u64),
    Parse(Rc<ParseState>),
    Output(Rc<RefCell<String>>),
}
impl JsonHandle {
    pub(crate) fn roots(&self, roots: &mut Vec<Slot>) {
        let root = match self {
            Self::Node(id) => Some(*id),
            Self::Parse(state) => state.outcome.borrow().as_ref().and_then(|p| p.root),
            Self::Output(_) => None,
        };
        if let Some(bits) = root {
            roots.push(Slot {
                tag: JSON_NODE,
                bits,
            });
        }
    }
}
/// Line and column of `offset` in `source`, from 1; CR LF is one line break.
fn located(source: &[char], offset: usize) -> (i64, i64) {
    let (mut line, mut column, mut position) = (1, 1, 0);
    while position < offset {
        match source[position] {
            '\r' => {
                position += 1;
                if position < offset && source[position] == '\n' {
                    position += 1;
                }
                line += 1;
                column = 1;
                continue;
            }
            '\n' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        position += 1;
    }
    (line, column)
}

/// JSON as UTF-8 bytes (#135): the parser reads the decoded characters, with
/// no text in between. Bytes that are not UTF-8 fail where the valid prefix
/// ends, as a parse error there would.
fn bytes_state(bytes: &[u8], maximum_depth: BigInt) -> ParseState {
    match std::str::from_utf8(bytes) {
        Ok(text) => ParseState {
            source: text.chars().collect(),
            maximum_depth,
            outcome: RefCell::new(None),
        },
        Err(error) => {
            let valid = std::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap_or_default();
            let source: Vec<char> = valid.chars().collect();
            let (line, column) = located(&source, source.len());
            let outcome = ParsedDocument {
                root: None,
                offset: source.len() as i64,
                line,
                column,
                message: "invalid UTF-8".into(),
                duplicate: None,
            };
            ParseState {
                source,
                maximum_depth,
                outcome: RefCell::new(Some(Rc::new(outcome))),
            }
        }
    }
}

pub(crate) fn handles(native: Native) -> bool {
    // The stream splitter (#136) runs in okkama-runtime-core, through the
    // origin bridge.
    native.symbol().starts_with("runtime/json@1::") && !native.symbol().starts_with("runtime/json@1::json_split_")
}
fn failure(native: Native, code: &str, message: &str) -> ProviderError {
    ProviderError::Native(NativeFailure {
        domain: "core.json".into(),
        code: code.into(),
        operation: native.symbol().into(),
        message: message.into(),
    })
}
impl Heap {
    fn json_handle(&self, value: Slot) -> Result<JsonHandle, Error> {
        if value.kind() != HANDLE {
            return Err(Error("expected native JSON handle".into()));
        }
        match self.handles.get(value.bits)? {
            Handle::Json(state) => Ok(state.clone()),
            _ => Err(Error("expected native JSON handle".into())),
        }
    }
    fn json_id(&self, value: Slot) -> Result<u64, Error> {
        match self.json_handle(value)? {
            JsonHandle::Node(id) => {
                self.json_nodes.get(id)?;
                Ok(id)
            }
            _ => Err(Error("expected JSON node handle".into())),
        }
    }
    fn json_node(&self, value: Slot) -> Result<&Node, Error> {
        self.json_nodes.get(self.json_id(value)?)
    }
    fn json_handle_new(&mut self, state: JsonHandle) -> Result<Slot, Error> {
        Ok(Slot {
            tag: HANDLE,
            bits: self.handles.allocate(Handle::Json(state))?,
        })
    }
    fn json_node_value(&mut self, id: u64) -> Result<Slot, Error> {
        self.json_nodes.get(id)?;
        self.json_handle_new(JsonHandle::Node(id))
    }
    fn json_new(&mut self, node: Node) -> Result<Slot, Error> {
        let id = self.json_nodes.allocate(node)?;
        self.json_node_value(id)
    }
    fn json_shared(&self, value: Slot) -> Result<Rc<str>, Error> {
        if value.kind() != STR {
            return Err(Error("expected string slot".into()));
        }
        Ok(self.text(value)?.into())
    }
    fn json_text(&mut self, text: Rc<str>) -> Result<Slot, Error> {
        self.allocate_text(text.to_string())
    }
    fn json_member(&self, object: Slot, name: Slot) -> Result<Option<u64>, Error> {
        let name = self.text(name)?;
        Ok(match self.json_node(object)? {
            Node::Object(names, values) => {
                names.iter().position(|n| &**n == name).map(|i| values[i])
            }
            _ => None,
        })
    }
    fn json_parsed(&mut self, value: Slot) -> Result<Rc<ParsedDocument>, Error> {
        let JsonHandle::Parse(state) = self.json_handle(value)? else {
            return Err(Error("expected JSON parse state".into()));
        };
        if let Some(outcome) = state.outcome.borrow().as_ref() {
            return Ok(outcome.clone());
        }
        let outcome = match parser::parse(&state.source, &state.maximum_depth) {
            Ok((nodes, root)) => {
                let mut ids = Vec::with_capacity(nodes.len());
                for mut node in nodes {
                    if let Node::Array(children) | Node::Object(_, children) = &mut node {
                        for child in children {
                            *child = ids[*child as usize];
                        }
                    }
                    ids.push(self.json_nodes.allocate(node)?);
                }
                ParsedDocument {
                    root: Some(ids[root as usize]),
                    offset: 0,
                    line: 0,
                    column: 0,
                    message: String::new(),
                    duplicate: None,
                }
            }
            Err(error) => {
                let overflow = error.message == "\u{0}overflow";
                let (line, column) = located(&state.source, error.offset);
                ParsedDocument {
                    root: None,
                    offset: error.offset as i64,
                    line: if overflow { 0 } else { line },
                    column: if overflow { 0 } else { column },
                    message: if overflow {
                        "maximum JSON depth exceeded".into()
                    } else {
                        error.message
                    },
                    duplicate: error.duplicate,
                }
            }
        };
        let outcome = Rc::new(outcome);
        *state.outcome.borrow_mut() = Some(outcome.clone());
        Ok(outcome)
    }
    fn json_write(&self, root: u64, indentation: Option<usize>) -> Result<String, Error> {
        enum Part {
            Node(u64, usize),
            Char(char),
            Line(usize),
            Name(Rc<str>),
        }
        let mut pending = vec![Part::Node(root, 0)];
        let mut out = String::new();
        while let Some(part) = pending.pop() {
            match part {
                Part::Char(ch) => out.push(ch),
                Part::Name(name) => {
                    push_quoted(&mut out, &name);
                    out.push(':');
                    if indentation.is_some() {
                        out.push(' ');
                    }
                }
                Part::Line(level) => {
                    if let Some(width) = indentation {
                        let size = width
                            .checked_mul(level)
                            .ok_or_else(|| Error("JSON indentation exceeds host range".into()))?;
                        out.try_reserve(
                            size.checked_add(1)
                                .ok_or_else(|| Error("JSON output exceeds host capacity".into()))?,
                        )
                        .map_err(|_| Error("JSON output exceeds host capacity".into()))?;
                        out.push('\n');
                        out.extend(std::iter::repeat_n(' ', size));
                    }
                }
                Part::Node(id, level) => match self.json_nodes.get(id)? {
                    Node::Null => out.push_str("null"),
                    Node::Boolean(value) => out.push_str(if *value { "true" } else { "false" }),
                    Node::Text(text) => push_quoted(&mut out, text),
                    Node::Number(text) => {
                        if let Some(parts) = crate::host::native_numbers::components(text) {
                            out.push_str(&number_text(&parts));
                        } else {
                            out.push_str(text);
                        }
                    }
                    node @ (Node::Array(_) | Node::Object(..)) => {
                        let values = node.children();
                        let object = matches!(node, Node::Object(..));
                        out.push(if object { '{' } else { '[' });
                        pending.push(Part::Char(if object { '}' } else { ']' }));
                        if !values.is_empty() {
                            pending.push(Part::Line(level));
                        }
                        for (index, &child) in values.iter().enumerate().rev() {
                            pending.push(Part::Node(child, level + 1));
                            if let Node::Object(names, _) = node {
                                pending.push(Part::Name(names[index].clone()));
                            }
                            pending.push(Part::Line(level + 1));
                            if index > 0 {
                                pending.push(Part::Char(','));
                            }
                        }
                    }
                },
            }
        }
        Ok(out)
    }
    fn json_pointer(&self, root: u64, target: u64) -> Result<Option<String>, Error> {
        let mut stack = vec![(root, 0usize)];
        let mut path: Vec<String> = Vec::new();
        let mut seen = HashSet::new();
        seen.insert(root);
        if root == target {
            return Ok(Some(String::new()));
        }
        while let Some((id, index)) = stack.last_mut() {
            let node = self.json_nodes.get(*id)?;
            let children = node.children();
            if *index == children.len() {
                stack.pop();
                path.pop();
                continue;
            }
            let at = *index;
            *index += 1;
            let child = children[at];
            if !seen.insert(child) {
                continue;
            }
            let token = match node {
                Node::Object(names, _) => names[at].to_string(),
                _ => at.to_string(),
            };
            path.push(token);
            if child == target {
                return Ok(Some(
                    path.iter()
                        .map(|t| format!("/{}", t.replace('~', "~0").replace('/', "~1")))
                        .collect(),
                ));
            }
            stack.push((child, 0));
        }
        Ok(None)
    }
}
fn push_quoted(quoted: &mut String, origin: &str) {
    quoted.push('"');
    for character in origin.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                quoted.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
}

/// A JSON string literal: `"` and `\` escaped, `\n`, `\r`, `\t` by name, the
/// other controls U+0000..U+001F as lowercase `\u00xx`, everything else as is.
fn quote(origin: &str) -> String {
    let mut quoted = String::with_capacity(origin.len() + 2);
    push_quoted(&mut quoted, origin);
    quoted
}

/// The JSON spelling of an exact decimal (as `json.Json` writes it):
/// positional while that stays short — integers up to 21 digits, fractions
/// down to 1e-6, the thresholds of JavaScript's `Number#toString` — and the
/// canonical scientific form beyond them, so the value is never expanded.
fn number_text(parts: &crate::host::native_numbers::Decimal) -> String {
    let sign = if parts.sign < 0 { "-" } else { "" };
    let digits = parts.digits.as_str();
    let length = digits.len() as i128;
    if parts.exponent >= 0 {
        if length + parts.exponent as i128 <= 21 {
            return format!("{sign}{digits}{}", "0".repeat(parts.exponent as usize));
        }
        return crate::host::native_numbers::canonical(parts);
    }
    let point = length + parts.exponent as i128;
    if point > 0 {
        let (whole, fraction) = digits.split_at(point as usize);
        format!("{sign}{whole}.{fraction}")
    } else if -point <= 5 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else {
        crate::host::native_numbers::canonical(parts)
    }
}

impl Heap {
    pub(crate) fn native_json(&mut self, native: Native, args: &[Slot]) -> Result<Outcome, Error> {
        match self.json_invoke(native, args) {
            Ok(value) => Ok(Outcome::Value(value)),
            Err(ProviderError::Engine(error)) => Err(error),
            Err(ProviderError::Native(failure)) => {
                let payload = self.native_failure_new(failure)?;
                self.failure_new(crate::host::FailureKind::Native(payload))
                    .map(Outcome::Failure)
            }
        }
    }
    fn json_invoke(&mut self, native: Native, args: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        let invalid = |message| failure(native, "invalid_input", message);
        let index =
            |heap: &Heap, value: Slot, len: usize, message| -> Result<usize, ProviderError> {
                heap.integer(value)?
                    .to_i64()
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|n| *n < len)
                    .ok_or_else(|| invalid(message))
            };
        if matches!(
            native,
            JsonJsonParseValid
                | JsonJsonParseHasDuplicate
                | JsonJsonParseOffset
                | JsonJsonParseLine
                | JsonJsonParseColumn
                | JsonJsonParseMessage
                | JsonJsonParseDuplicate
                | JsonJsonParseRoot
        ) {
            let parsed = self.json_parsed(args[0])?;
            return Ok(match native {
                JsonJsonParseValid => Slot::boolean(parsed.root.is_some()),
                JsonJsonParseHasDuplicate => Slot::boolean(parsed.duplicate.is_some()),
                JsonJsonParseOffset => Slot::small(parsed.offset),
                JsonJsonParseLine => Slot::small(parsed.line),
                JsonJsonParseColumn => Slot::small(parsed.column),
                JsonJsonParseMessage => self.allocate_text(parsed.message.clone())?,
                JsonJsonParseDuplicate => {
                    self.allocate_text(parsed.duplicate.clone().unwrap_or_default())?
                }
                JsonJsonParseRoot => self.json_node_value(parsed.root.ok_or_else(|| {
                    failure(native, "invalid_data", "invalid JSON has no root")
                })?)?,
                _ => unreachable!(),
            });
        }
        let value = match native {
            JsonJsonParseState => {
                let maximum_depth = match self.integer(args[1])? {
                    Integer::Small(n) => BigInt::from(n),
                    Integer::Big(n) => (*n).clone(),
                };
                let source = self.text(args[0])?.chars().collect();
                self.json_handle_new(JsonHandle::Parse(Rc::new(ParseState {
                    source,
                    maximum_depth,
                    outcome: RefCell::new(None),
                })))?
            }
            JsonJsonParseBytesState => {
                let maximum_depth = match self.integer(args[1])? {
                    Integer::Small(n) => BigInt::from(n),
                    Integer::Big(n) => (*n).clone(),
                };
                let state = bytes_state(&self.byte_slice(args[0])?, maximum_depth);
                self.json_handle_new(JsonHandle::Parse(Rc::new(state)))?
            }
            JsonJsonNodeKind => Slot::small(self.json_node(args[0])?.kind()),
            JsonJsonNodeBoolean => match self.json_node(args[0])? {
                Node::Boolean(value) => Slot::boolean(*value),
                _ => return Err(invalid("JSON node is not boolean")),
            },
            JsonJsonNodeText | JsonJsonNodeNumber => {
                let text = match (native, self.json_node(args[0])?) {
                    (JsonJsonNodeText, Node::Text(text))
                    | (JsonJsonNodeNumber, Node::Number(text)) => text.clone(),
                    _ => return Err(invalid("JSON node has the wrong kind")),
                };
                self.json_text(text)?
            }
            JsonJsonNodeSize => match self.json_node(args[0])? {
                Node::Array(values) | Node::Object(_, values) => Slot::small(values.len() as i64),
                _ => return Err(invalid("JSON node has no children")),
            },
            JsonJsonNodeItem => {
                let Node::Array(values) = self.json_node(args[0])? else {
                    return Err(invalid("JSON node is not an array"));
                };
                let at = index(
                    self,
                    args[1],
                    values.len(),
                    "JSON array index is outside the node",
                )?;
                let child = values[at];
                self.json_node_value(child)?
            }
            JsonJsonMemberName | JsonJsonMemberValue => {
                let Node::Object(names, values) = self.json_node(args[0])? else {
                    return Err(invalid("JSON node is not an object"));
                };
                let at = index(
                    self,
                    args[1],
                    values.len(),
                    "JSON member index is outside the node",
                )?;
                if native == JsonJsonMemberName {
                    let name = names[at].clone();
                    self.json_text(name)?
                } else {
                    let child = values[at];
                    self.json_node_value(child)?
                }
            }
            JsonJsonMemberPosition => {
                let Node::Object(names, _) = self.json_node(args[0])? else {
                    return Err(invalid("JSON node is not an object"));
                };
                let sought = self.text(args[1])?;
                Slot::small(names.position(sought).map_or(-1, |i| i as i64))
            }
            JsonJsonQuote => self.allocate_text(quote(self.text(args[0])?))?,
            JsonJsonNumberText => {
                let parts = crate::host::native_numbers::components(self.text(args[0])?)
                    .ok_or_else(|| invalid("not a decimal number"))?;
                self.allocate_text(number_text(&parts))?
            }
            JsonJsonOutput => {
                self.json_handle_new(JsonHandle::Output(Rc::new(RefCell::new(String::new()))))?
            }
            JsonJsonOutputRaw | JsonJsonOutputText | JsonJsonOutputNumber
            | JsonJsonOutputMember | JsonJsonOutputString => {
                let JsonHandle::Output(output) = self.json_handle(args[0])? else {
                    return Err(Error("expected JSON output handle".into()).into());
                };
                let mut out = output.borrow_mut();
                match native {
                    JsonJsonOutputRaw => out.push_str(self.text(args[1])?),
                    JsonJsonOutputText => push_quoted(&mut out, self.text(args[1])?),
                    JsonJsonOutputNumber => {
                        let parts = crate::host::native_numbers::components(self.text(args[1])?)
                            .ok_or_else(|| invalid("not a decimal number"))?;
                        out.push_str(&number_text(&parts));
                    }
                    JsonJsonOutputMember => {
                        if args[2].bits == 0 {
                            out.push(',');
                        }
                        push_quoted(&mut out, self.text(args[1])?);
                        out.push(':');
                    }
                    JsonJsonOutputString => return Ok(self.allocate_text(out.clone())?),
                    _ => unreachable!(),
                }
                Slot::small(out.len() as i64)
            }
            JsonJsonMemberText
            | JsonJsonMemberSpelling
            | JsonJsonMemberBoolean
            | JsonJsonMemberInt => {
                let member = self
                    .json_member(args[0], args[1])?
                    .map(|id| self.json_nodes.get(id))
                    .transpose()?;
                match (native, member) {
                    (JsonJsonMemberText, Some(Node::Text(text)))
                    | (JsonJsonMemberSpelling, Some(Node::Number(text))) => {
                        let text = text.clone();
                        self.json_text(text)?
                    }
                    (JsonJsonMemberBoolean, Some(Node::Boolean(value))) => Slot::boolean(*value),
                    (JsonJsonMemberInt, Some(Node::Number(text)))
                        if text.trim_start_matches('-').len() <= 18
                            && !text.trim_start_matches('-').is_empty()
                            && text
                                .trim_start_matches('-')
                                .bytes()
                                .all(|b| b.is_ascii_digit()) =>
                    {
                        Slot::small(text.parse::<i64>().map_err(|_| invalid("JSON integer"))?)
                    }
                    _ => {
                        return Err(invalid(match native {
                            JsonJsonMemberText => "JSON member is missing or not a string",
                            JsonJsonMemberSpelling => "JSON member is missing or not a number",
                            JsonJsonMemberBoolean => "JSON member is missing or not boolean",
                            _ => "JSON member is missing or not a short integer",
                        }));
                    }
                }
            }
            JsonJsonTextNode => {
                let text = self.json_shared(args[0])?;
                self.json_new(Node::Text(text))?
            }
            JsonJsonNumberNode => {
                crate::host::native_numbers::components(self.text(args[0])?)
                    .ok_or_else(|| invalid("not a decimal number"))?;
                let text = self.json_shared(args[0])?;
                self.json_new(Node::Number(text))?
            }
            JsonJsonBooleanNode => self.json_new(Node::Boolean(args[0].bits != 0))?,
            JsonJsonNullNode => self.json_new(Node::Null)?,
            JsonJsonArrayNode => {
                let children = self
                    .elements(args[0])?
                    .iter()
                    .map(|s| self.json_id(*s))
                    .collect::<Result<Vec<_>, _>>()?;
                self.json_new(Node::Array(children))?
            }
            JsonJsonObjectNode => {
                let names = self
                    .elements(args[0])?
                    .iter()
                    .map(|s| self.json_shared(*s))
                    .collect::<Result<Vec<_>, _>>()?;
                let children = self
                    .elements(args[1])?
                    .iter()
                    .map(|s| self.json_id(*s))
                    .collect::<Result<Vec<_>, _>>()?;
                if names.len() != children.len() {
                    return Err(invalid("JSON member names and values differ in number"));
                }
                let mut seen = HashSet::with_capacity(names.len());
                if !names.iter().all(|n| seen.insert(n.as_ref())) {
                    return Err(invalid("duplicate JSON member name"));
                }
                self.json_new(Node::Object(names.into(), children))?
            }
            JsonJsonNodeWrite => {
                let indentation = self
                    .integer(args[1])?
                    .to_i64()
                    .ok_or_else(|| invalid("JSON indentation is out of range"))?;
                let text =
                    self.json_write(self.json_id(args[0])?, usize::try_from(indentation).ok())?;
                self.allocate_text(text)?
            }
            JsonJsonNodePointer => {
                let path = self
                    .json_pointer(self.json_id(args[0])?, self.json_id(args[1])?)?
                    .ok_or_else(|| invalid("JSON node is not inside the document"))?;
                self.allocate_text(path)?
            }
            JsonJsonMemberValues | JsonJsonNodeItems => {
                let children = match (native, self.json_node(args[0])?) {
                    (JsonJsonMemberValues, Node::Object(_, values))
                    | (JsonJsonNodeItems, Node::Array(values)) => values.clone(),
                    _ => {
                        return Err(invalid(if native == JsonJsonNodeItems {
                            "JSON node is not an array"
                        } else {
                            "JSON node is not an object"
                        }));
                    }
                };
                let slots = children
                    .into_iter()
                    .map(|id| self.json_node_value(id))
                    .collect::<Result<Vec<_>, _>>()?;
                self.allocate_array(slots)?
            }
            JsonJsonMemberNames => {
                let Node::Object(names, _) = self.json_node(args[0])? else {
                    return Err(invalid("JSON node is not an object"));
                };
                let names = names.to_vec();
                let slots = names
                    .into_iter()
                    .map(|s| self.json_text(s))
                    .collect::<Result<Vec<_>, _>>()?;
                self.allocate_array(slots)?
            }
            JsonJsonRepeatedName => {
                let names = self.elements(args[0])?;
                let mut seen = HashSet::with_capacity(names.len());
                let mut repeated = names.len();
                for (i, &name) in names.iter().enumerate() {
                    if !seen.insert(self.text(name)?) {
                        repeated = i;
                        break;
                    }
                }
                Slot::small(repeated as i64)
            }
            _ => return Err(Error("invalid JSON provider".into()).into()),
        };
        Ok(value)
    }
}

