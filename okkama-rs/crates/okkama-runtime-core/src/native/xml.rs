//! `runtime/xml` (#144): a bounded, non-validating tokenizer of a whole XML
//! document. It checks that the document is well formed and hands
//! `std/xml/parse` its events; the library builds the tree and resolves
//! namespaces. There is no DTD: a DOCTYPE with an internal subset is refused
//! and an external one is never fetched, so the only entities are the five
//! predefined ones and character references. Depth and the number of events
//! are bounded.

use super::Error;
use super::support::{handle, int, int_value, new_handle, string, text};
use crate::{Value, abi::Native};

#[derive(Debug)]
enum Event {
    Start { name: String, attributes: Vec<(String, String)> },
    End { name: String },
    Text(String),
    Comment(String),
    Instruction { target: String, data: String },
}

impl Event {
    fn kind(&self) -> i64 {
        match self {
            Event::Start { .. } => 0,
            Event::End { .. } => 1,
            Event::Text(_) => 2,
            Event::Comment(_) => 3,
            Event::Instruction { .. } => 4,
        }
    }
}

#[derive(Debug)]
pub struct Events {
    events: Vec<Event>,
    /// Character offset and reason of the first error.
    error: Option<(usize, String)>,
}

struct Parser<'a> {
    source: &'a str,
    position: usize,
    events: Vec<Event>,
    open: Vec<String>,
    root_closed: bool,
    root_seen: bool,
    maximum_depth: usize,
    maximum_events: usize,
}

type Step = Result<(), (usize, String)>;

/// A character XML 1.0 allows.
fn allowed(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

fn name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == ':' || (c as u32) > 0x7F
}

fn name_char(c: char) -> bool {
    name_start(c) || c.is_ascii_digit() || c == '-' || c == '.'
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.source[self.position..]
    }

    fn fail<T>(&self, message: impl Into<String>) -> Result<T, (usize, String)> {
        Err((self.position, message.into()))
    }

    fn push(&mut self, event: Event) -> Step {
        if let (Event::Text(more), Some(Event::Text(text))) = (&event, self.events.last_mut()) {
            text.push_str(more);
            return Ok(());
        }
        if self.events.len() >= self.maximum_events {
            return self.fail(format!("more than {} XML events", self.maximum_events));
        }
        self.events.push(event);
        Ok(())
    }

    fn skip_space(&mut self) {
        let skipped = self.rest().len() - self.rest().trim_start_matches([' ', '\t', '\n', '\r']).len();
        self.position += skipped;
    }

    fn name(&mut self) -> Result<String, (usize, String)> {
        let rest = self.rest();
        let mut chars = rest.char_indices();
        match chars.next() {
            Some((_, c)) if name_start(c) => {}
            _ => return self.fail("expected a name"),
        }
        let end = rest.char_indices().find(|&(_, c)| !name_char(c)).map_or(rest.len(), |(at, _)| at);
        self.position += end;
        Ok(rest[..end].to_string())
    }

    /// Text up to `<` (or a quote in an attribute), with references decoded.
    fn decoded(&mut self, raw: &str, attribute: bool) -> Result<String, (usize, String)> {
        let mut out = String::with_capacity(raw.len());
        let mut rest = raw;
        let start = self.position;
        while let Some(c) = rest.chars().next() {
            if !allowed(c) {
                return Err((start + raw.len() - rest.len(), format!("a character U+{:04X} XML does not allow", c as u32)));
            }
            if c == '&' {
                let Some(end) = rest.find(';') else {
                    return Err((start + raw.len() - rest.len(), "an unterminated reference".into()));
                };
                let reference = &rest[1..end];
                let value = match reference {
                    "lt" => '<',
                    "gt" => '>',
                    "amp" => '&',
                    "quot" => '"',
                    "apos" => '\'',
                    _ if reference.starts_with("#x") => u32::from_str_radix(&reference[2..], 16)
                        .ok()
                        .and_then(char::from_u32)
                        .filter(|c| allowed(*c))
                        .ok_or((start + raw.len() - rest.len(), format!("a bad character reference &{reference};")))?,
                    _ if reference.starts_with('#') => reference[1..]
                        .parse::<u32>()
                        .ok()
                        .and_then(char::from_u32)
                        .filter(|c| allowed(*c))
                        .ok_or((start + raw.len() - rest.len(), format!("a bad character reference &{reference};")))?,
                    _ => {
                        return Err((start + raw.len() - rest.len(), format!("an unknown entity &{reference};")));
                    }
                };
                out.push(value);
                rest = &rest[end + 1..];
                continue;
            }
            if attribute && c == '<' {
                return Err((start + raw.len() - rest.len(), "< inside an attribute value".into()));
            }
            // Attribute values normalize whitespace characters to spaces.
            out.push(if attribute && matches!(c, '\t' | '\n' | '\r') { ' ' } else { c });
            rest = &rest[c.len_utf8()..];
        }
        Ok(out)
    }

    fn run(&mut self) -> Step {
        if self.rest().starts_with('\u{FEFF}') {
            self.position += '\u{FEFF}'.len_utf8();
        }
        let start = self.position;
        while self.position < self.source.len() {
            let rest = self.rest();
            if let Some(body) = rest.strip_prefix("<?") {
                let Some(end) = body.find("?>") else { return self.fail("an unterminated processing instruction") };
                self.position += 2;
                let target = self.name()?;
                let data = body[target.len()..end].trim_start_matches([' ', '\t', '\n', '\r']).to_string();
                if target.eq_ignore_ascii_case("xml") {
                    if self.position - 5 != start {
                        return self.fail("an XML declaration that is not at the start");
                    }
                } else if !self.open.is_empty() {
                    self.push(Event::Instruction { target, data })?;
                }
                self.position = self.source.len() - rest.len() + 2 + end + 2;
            } else if let Some(body) = rest.strip_prefix("<!--") {
                let Some(end) = body.find("-->") else { return self.fail("an unterminated comment") };
                if body[..end].contains("--") {
                    return self.fail("-- inside a comment");
                }
                if !self.open.is_empty() {
                    self.push(Event::Comment(body[..end].to_string()))?;
                }
                self.position += 4 + end + 3;
            } else if let Some(body) = rest.strip_prefix("<![CDATA[") {
                if self.open.is_empty() {
                    return self.fail("a CDATA section outside the root element");
                }
                let Some(end) = body.find("]]>") else { return self.fail("an unterminated CDATA section") };
                if let Some(bad) = body[..end].chars().find(|c| !allowed(*c)) {
                    return self.fail(format!("a character U+{:04X} XML does not allow", bad as u32));
                }
                self.push(Event::Text(body[..end].to_string()))?;
                self.position += 9 + end + 3;
            } else if rest.starts_with("<!DOCTYPE") {
                if self.root_seen {
                    return self.fail("a DOCTYPE after the root element");
                }
                let mut quote = None;
                let mut end = None;
                for (at, c) in rest.char_indices() {
                    match (quote, c) {
                        (Some(q), _) if c == q => quote = None,
                        (Some(_), _) => {}
                        (None, '"' | '\'') => quote = Some(c),
                        (None, '[') => return Err((self.position + at, "a DTD internal subset is not supported".into())),
                        (None, '>') => {
                            end = Some(at);
                            break;
                        }
                        _ => {}
                    }
                }
                let Some(end) = end else { return self.fail("an unterminated DOCTYPE") };
                self.position += end + 1;
            } else if let Some(_body) = rest.strip_prefix("</") {
                self.position += 2;
                let named_at = self.position;
                let name = self.name()?;
                self.skip_space();
                if !self.rest().starts_with('>') {
                    return self.fail("expected > to close the end tag");
                }
                self.position += 1;
                match self.open.pop() {
                    Some(open) if open == name => {}
                    Some(open) => return Err((named_at, format!("</{name}> closes <{open}>"))),
                    None => return Err((named_at, format!("</{name}> closes nothing"))),
                }
                self.push(Event::End { name })?;
                if self.open.is_empty() {
                    self.root_closed = true;
                }
            } else if rest.starts_with('<') {
                if self.root_closed {
                    return self.fail("a second root element");
                }
                self.position += 1;
                let name = self.name()?;
                let mut attributes: Vec<(String, String)> = Vec::new();
                loop {
                    let before = self.position;
                    self.skip_space();
                    let rest = self.rest();
                    if rest.starts_with("/>") || rest.starts_with('>') {
                        break;
                    }
                    if self.position == before {
                        return self.fail("expected a space before an attribute");
                    }
                    let named_at = self.position;
                    let attribute = self.name()?;
                    self.skip_space();
                    if !self.rest().starts_with('=') {
                        return self.fail(format!("expected = after {attribute}"));
                    }
                    self.position += 1;
                    self.skip_space();
                    let Some(quote) = self.rest().chars().next().filter(|c| *c == '"' || *c == '\'') else {
                        return self.fail("expected a quoted attribute value");
                    };
                    self.position += 1;
                    let Some(end) = self.rest().find(quote) else { return self.fail("an unterminated attribute value") };
                    let raw = &self.rest()[..end];
                    let value = self.decoded(raw, true)?;
                    if attributes.iter().any(|(n, _)| *n == attribute) {
                        return Err((named_at, format!("the attribute {attribute} given twice")));
                    }
                    attributes.push((attribute, value));
                    self.position += end + 1;
                }
                let closed = self.rest().starts_with("/>");
                self.position += if closed { 2 } else { 1 };
                if self.open.len() >= self.maximum_depth {
                    return self.fail(format!("elements nested deeper than {}", self.maximum_depth));
                }
                self.root_seen = true;
                self.push(Event::Start { name: name.clone(), attributes })?;
                if closed {
                    self.push(Event::End { name })?;
                    if self.open.is_empty() {
                        self.root_closed = true;
                    }
                } else {
                    self.open.push(name);
                }
            } else {
                let end = rest.find('<').unwrap_or(rest.len());
                let raw = &rest[..end];
                if self.open.is_empty() {
                    if let Some(at) = raw.find(|c: char| !matches!(c, ' ' | '\t' | '\n' | '\r')) {
                        self.position += at;
                        return self.fail("text outside the root element");
                    }
                } else {
                    if raw.contains("]]>") {
                        return self.fail("]]> in text");
                    }
                    let value = self.decoded(raw, false)?;
                    self.push(Event::Text(value))?;
                }
                self.position += end;
            }
        }
        if let Some(open) = self.open.last() {
            return self.fail(format!("<{open}> is not closed"));
        }
        if !self.root_seen {
            return self.fail("no root element");
        }
        Ok(())
    }
}

/// The events of `source`, or the first error.
pub fn events(source: &str, maximum_depth: usize, maximum_events: usize) -> Events {
    let mut parser = Parser {
        source,
        position: 0,
        events: Vec::new(),
        open: Vec::new(),
        root_closed: false,
        root_seen: false,
        maximum_depth,
        maximum_events,
    };
    match parser.run() {
        Ok(()) => Events { events: parser.events, error: None },
        Err((at, message)) => {
            let at = at.min(source.len());
            let characters = source[..source.floor_char_boundary(at)].chars().count();
            Events { events: Vec::new(), error: Some((characters, message)) }
        }
    }
}

fn state(value: &Value) -> Result<&Events, Error> {
    handle::<Events>(value)
}

fn event<'a>(native: Native, events: &'a Events, index: &Value) -> Result<&'a Event, Error> {
    int(index)?
        .to_i64()
        .and_then(|i| usize::try_from(i).ok())
        .and_then(|i| events.events.get(i))
        .ok_or_else(|| super::support::failure("core.xml", "invalid_input", native, "XML event index out of range"))
}

fn size(native: Native, value: &Value, what: &str) -> Result<usize, Error> {
    int(value)?
        .to_i64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| super::support::failure("core.xml", "invalid_input", native, format!("{what} must be positive")))
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    use Native::*;
    let missing = || super::support::failure("core.xml", "invalid_input", native, "no such attribute");
    match native {
        XmlXmlEvents => Ok(new_handle(events(
            string(&args[0])?,
            size(native, &args[1], "maximum depth")?,
            size(native, &args[2], "maximum events")?,
        ))),
        XmlXmlEventsError => Ok(text(state(&args[0])?.error.as_ref().map(|(_, m)| m.clone()).unwrap_or_default())),
        XmlXmlEventsOffset => Ok(int_value(state(&args[0])?.error.as_ref().map_or(0, |(at, _)| *at as i64))),
        XmlXmlEventCount => Ok(int_value(state(&args[0])?.events.len() as i64)),
        XmlXmlEventKind => Ok(int_value(event(native, state(&args[0])?, &args[1])?.kind())),
        XmlXmlEventName => Ok(text(match event(native, state(&args[0])?, &args[1])? {
            Event::Start { name, .. } | Event::End { name } => name.clone(),
            Event::Instruction { target, .. } => target.clone(),
            _ => String::new(),
        })),
        XmlXmlEventText => Ok(text(match event(native, state(&args[0])?, &args[1])? {
            Event::Text(value) | Event::Comment(value) => value.clone(),
            Event::Instruction { data, .. } => data.clone(),
            _ => String::new(),
        })),
        XmlXmlAttributeCount => Ok(int_value(match event(native, state(&args[0])?, &args[1])? {
            Event::Start { attributes, .. } => attributes.len() as i64,
            _ => 0,
        })),
        XmlXmlAttributeName | XmlXmlAttributeValue => {
            let Event::Start { attributes, .. } = event(native, state(&args[0])?, &args[1])? else {
                return Err(missing());
            };
            let at = int(&args[2])?.to_i64().and_then(|i| usize::try_from(i).ok()).ok_or_else(missing)?;
            let (name, value) = attributes.get(at).ok_or_else(missing)?;
            Ok(text(if native == XmlXmlAttributeName { name.clone() } else { value.clone() }))
        }
        _ => Err(super::support::failure("core.xml", "unsupported", native, "not an XML native")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(source: &str) -> Result<String, String> {
        let parsed = events(source, 64, 10_000);
        if let Some((at, message)) = parsed.error {
            return Err(format!("{at}: {message}"));
        }
        Ok(parsed
            .events
            .iter()
            .map(|event| match event {
                Event::Start { name, attributes } => format!(
                    "<{name}{}>",
                    attributes.iter().map(|(n, v)| format!(" {n}={v}")).collect::<String>()
                ),
                Event::End { name } => format!("</{name}>"),
                Event::Text(text) => format!("[{text}]"),
                Event::Comment(text) => format!("!{text}"),
                Event::Instruction { target, data } => format!("?{target} {data}"),
            })
            .collect())
    }

    #[test]
    fn documents_become_events() {
        assert_eq!(
            shape("<?xml version=\"1.0\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple\" \"x.dtd\">\n<rss version='2.0'><item a=\"1 &amp; 2\">x&lt;y<![CDATA[<raw>]]>&#x41;&#66;</item><e/><!--c--><?p d?></rss>\n"),
            Ok("<rss version=2.0><item a=1 & 2>[x<y<raw>AB]</item><e></e>!c?p d</rss>".into())
        );
        assert_eq!(shape("<a\tb=\"x\ny\"/>"), Ok("<a b=x y></a>".into()));
        assert_eq!(shape("<a>чай 😀</a>"), Ok("<a>[чай 😀]</a>".into()));
    }

    #[test]
    fn broken_documents_are_refused() {
        assert_eq!(shape("<a><b></a>"), Err("8: </a> closes <b>".into()));
        assert_eq!(shape("<a></a><b/>"), Err("7: a second root element".into()));
        assert_eq!(shape("<a x='1' x='2'/>"), Err("9: the attribute x given twice".into()));
        assert_eq!(shape("<a>&bogus;</a>").unwrap_err(), "3: an unknown entity &bogus;");
        assert_eq!(shape("<!DOCTYPE a [<!ENTITY e 'x'>]><a/>").unwrap_err(), "12: a DTD internal subset is not supported");
        assert_eq!(shape("x<a/>").unwrap_err(), "0: text outside the root element");
        assert_eq!(shape("<a>").unwrap_err(), "3: <a> is not closed");
        assert_eq!(shape("").unwrap_err(), "0: no root element");
        assert_eq!(shape("<a>&#0;</a>").unwrap_err(), "3: a bad character reference &#0;");
        assert!(events("<a><a><a></a></a></a>", 2, 100).error.is_some());
        assert!(events("<a><b/><b/><b/></a>", 64, 4).error.is_some());
    }
}
