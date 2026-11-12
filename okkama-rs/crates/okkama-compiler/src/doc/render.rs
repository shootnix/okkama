//! Reference pages from the documentation model (#512): terminal text and
//! Markdown, in one order:
//! - NAME and VERSION;
//! - SYNOPSIS;
//! - DESCRIPTION;
//! - PUBLIC API;
//! - ERRORS;
//! - ENGINE NOTES and SEE ALSO.
//!
//! A section with nothing to say is left out; nothing is invented.
//!
//! Doc comments are a CommonMark subset:
//! - paragraphs and headings;
//! - ordered and unordered lists;
//! - inline code;
//! - fenced and indented code blocks (std writes its examples indented by
//!   four spaces);
//! - links and simple tables.
//!
//! Other CommonMark parses but has no special look. A heading in a module
//! overview named Synopsis, Errors, Engine notes or See also fills that
//! section of the page. Without a Synopsis heading, the overview's first code
//! block is the synopsis.

use super::{ItemDoc, ItemKind, MemberDoc, ModuleDoc, summary};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// How terminal text looks.
#[derive(Copy, Clone, Debug)]
pub struct Style {
    /// Columns to wrap at.
    pub width: usize,
    /// Bold headings and signatures through ANSI escapes.
    pub ansi: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style { width: 80, ansi: false }
    }
}

/// One line of the index of modules: `io/stdout` and its summary.
#[derive(Clone, Debug)]
pub struct IndexEntry {
    pub path: String,
    pub summary: String,
}

/// The overview split by the headings that name page sections.
struct Overview {
    synopsis: Option<String>,
    description: String,
    errors: String,
    engines: String,
    see_also: String,
}

fn overview(doc: &ModuleDoc) -> Overview {
    let text = doc.overview.as_ref().map(|overview| overview.text.clone()).unwrap_or_default();
    let mut parts = Overview {
        synopsis: None,
        description: String::new(),
        errors: String::new(),
        engines: String::new(),
        see_also: String::new(),
    };
    let mut current = String::new();
    let mut section: Option<&'static str> = None;
    let flush = |parts: &mut Overview, section: Option<&'static str>, current: &mut String| {
        let text = std::mem::take(current).trim_matches('\n').to_string();
        match section {
            Some("synopsis") => parts.synopsis = Some(code_of(&text).unwrap_or(text)),
            Some("errors") => parts.errors = text,
            Some("engines") => parts.engines = text,
            Some("see") => parts.see_also = text,
            _ => {
                if !parts.description.is_empty() && !text.is_empty() {
                    parts.description.push_str("\n\n");
                }
                parts.description.push_str(&text);
            }
        }
    };
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let heading = (!fenced && line.starts_with('#'))
            .then(|| line.trim_start_matches('#').trim().to_lowercase());
        let named = heading.as_deref().and_then(|title| match title {
            "synopsis" => Some(Some("synopsis")),
            "errors" => Some(Some("errors")),
            "engine notes" | "engines" => Some(Some("engines")),
            "see also" => Some(Some("see")),
            _ => None,
        });
        match named {
            Some(next) => {
                flush(&mut parts, section, &mut current);
                section = next;
            }
            None => {
                current.push_str(line);
                current.push('\n');
            }
        }
    }
    flush(&mut parts, section, &mut current);
    if parts.synopsis.is_none()
        && let Some((code, range)) = first_code_block(&parts.description)
    {
        parts.synopsis = Some(code);
        parts.description.replace_range(range, "");
        parts.description = parts.description.trim_matches('\n').to_string();
    }
    parts
}

fn options() -> Options {
    Options::ENABLE_TABLES
}

/// The first code block of `text`, its content and where it is.
fn first_code_block(text: &str) -> Option<(String, std::ops::Range<usize>)> {
    let mut code = None::<(String, std::ops::Range<usize>)>;
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) if code.is_none() => code = Some((String::new(), range)),
            Event::Text(content) if code.is_some() => {
                if let Some((body, _)) = code.as_mut() {
                    body.push_str(&content);
                }
            }
            Event::End(TagEnd::CodeBlock) if code.is_some() => break,
            _ => {}
        }
    }
    code.map(|(body, range)| (body.trim_end_matches('\n').to_string(), range))
}

fn code_of(text: &str) -> Option<String> {
    first_code_block(text).map(|(code, _)| code)
}

// ---------------------------------------------------------------- text

/// The reference page of a module as terminal text.
pub fn module_text(doc: &ModuleDoc, style: &Style) -> String {
    let parts = overview(doc);
    let mut out = String::new();
    let summary = doc.summary();
    section(&mut out, "NAME", style);
    if summary.is_empty() {
        line(&mut out, 4, &doc.path);
    } else {
        wrapped(&mut out, 4, 4, &format!("{} - {summary}", doc.path), style.width);
    }
    if let Some(version) = &doc.version {
        section(&mut out, "VERSION", style);
        line(&mut out, 4, version);
    }
    if let Some(synopsis) = &parts.synopsis {
        section(&mut out, "SYNOPSIS", style);
        for code in synopsis.lines() {
            if code.trim().is_empty() {
                out.push('\n');
            } else {
                line(&mut out, 4, code);
            }
        }
    }
    if !parts.description.trim().is_empty() {
        section(&mut out, "DESCRIPTION", style);
        markdown_text(&mut out, &parts.description, 4, style);
    }
    let api: Vec<&ItemDoc> = doc.items.iter().filter(|item| item.kind != ItemKind::Error).collect();
    if !api.is_empty() {
        section(&mut out, "PUBLIC API", style);
        for (index, item) in api.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            item_block(&mut out, item, None, 4, style);
        }
    }
    let errors: Vec<&ItemDoc> = doc.items.iter().filter(|item| item.kind == ItemKind::Error).collect();
    if !errors.is_empty() || !parts.errors.trim().is_empty() {
        section(&mut out, "ERRORS", style);
        if !parts.errors.trim().is_empty() {
            markdown_text(&mut out, &parts.errors, 4, style);
        }
        for (index, item) in errors.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            item_block(&mut out, item, None, 4, style);
        }
    }
    if !parts.engines.trim().is_empty() {
        section(&mut out, "ENGINE NOTES", style);
        markdown_text(&mut out, &parts.engines, 4, style);
    }
    if !parts.see_also.trim().is_empty() {
        section(&mut out, "SEE ALSO", style);
        markdown_text(&mut out, &parts.see_also, 4, style);
    }
    out.trim_end().to_string() + "\n"
}

/// One declaration, or some of its members, as terminal text.
pub fn item_text(doc: &ModuleDoc, item: &ItemDoc, members: Option<&[&MemberDoc]>, style: &Style) -> String {
    let mut out = String::new();
    let mut title = format!("{} {}", doc.path, item.name);
    if let Some(members) = members
        && let Some(first) = members.first()
    {
        title = format!("{title}.{}", first.name);
    }
    section(&mut out, "NAME", style);
    let summary = match members {
        Some(members) => members.iter().find_map(|member| member.doc.as_ref()).map(|doc| summary(&doc.text)),
        None => item.doc.as_ref().map(|doc| summary(&doc.text)),
    }
    .unwrap_or_default();
    if summary.is_empty() {
        line(&mut out, 4, &title);
    } else {
        wrapped(&mut out, 4, 4, &format!("{title} - {summary}"), style.width);
    }
    if let Some(version) = &doc.version {
        section(&mut out, "VERSION", style);
        line(&mut out, 4, &format!("{} {version}", doc.path));
    }
    section(&mut out, "DECLARATION", style);
    item_block(&mut out, item, members, 4, style);
    out.trim_end().to_string() + "\n"
}

/// The index of modules as terminal text, grouped by their first segment.
pub fn index_text(entries: &[IndexEntry], style: &Style) -> String {
    let mut out = String::new();
    section(&mut out, "OKKAMA STANDARD LIBRARY", style);
    wrapped(
        &mut out,
        4,
        4,
        "okkama man MODULE shows a module; okkama man MODULE NAME shows one of its declarations, and NAME.ACTION one action.",
        style.width,
    );
    let width = entries.iter().map(|entry| entry.path.len()).max().unwrap_or(0).min(32);
    let mut group = None::<String>;
    for entry in entries {
        let area = area_of(&entry.path);
        if group.as_deref() != Some(area.as_str()) {
            section(&mut out, &area, style);
            group = Some(area);
        }
        let head = format!("{:width$}  ", entry.path);
        let room = style.width.saturating_sub(4 + head.chars().count()).max(20);
        let summary = clip(&entry.summary, room);
        line(&mut out, 4, format!("{head}{summary}").trim_end());
    }
    out.trim_end().to_string() + "\n"
}

fn area_of(path: &str) -> String {
    match path.split_once('/') {
        Some((area, _)) => area.to_string(),
        None => "top level".to_string(),
    }
}

fn clip(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_string();
    }
    let kept: String = text.chars().take(room.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

fn item_block(out: &mut String, item: &ItemDoc, only: Option<&[&MemberDoc]>, indent: usize, style: &Style) {
    signature(out, indent, &item.signature, style);
    if only.is_none()
        && let Some(doc) = &item.doc
    {
        markdown_text(out, &doc.text, indent + 4, style);
    }
    let members: Vec<&MemberDoc> = match only {
        Some(members) => members.to_vec(),
        None => item.members.iter().collect(),
    };
    // Members without a comment stay together; a documented one stands apart.
    let mut spaced = item.doc.is_some() && only.is_none();
    for (index, member) in members.iter().enumerate() {
        if index == 0 || spaced || member.doc.is_some() {
            out.push('\n');
        }
        signature(out, indent + 4, &member.signature, style);
        if let Some(doc) = &member.doc {
            markdown_text(out, &doc.text, indent + 8, style);
        }
        spaced = member.doc.is_some();
    }
}

fn section(out: &mut String, title: &str, style: &Style) {
    if !out.is_empty() {
        out.push('\n');
    }
    if style.ansi {
        out.push_str(&format!("\u{1b}[1m{title}\u{1b}[0m\n"));
    } else {
        out.push_str(title);
        out.push('\n');
    }
}

/// A signature on its own line; a long one breaks after commas, its later
/// lines four columns further in.
fn signature(out: &mut String, indent: usize, text: &str, style: &Style) {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for (index, piece) in text.split(", ").enumerate() {
        let piece = if index == 0 { piece.to_string() } else { format!(", {piece}") };
        let room = style.width.saturating_sub(indent + if lines.is_empty() { 0 } else { 4 });
        if !current.is_empty() && current.chars().count() + piece.chars().count() > room {
            lines.push(current.trim_end().to_string());
            current = piece.trim_start_matches(", ").to_string();
            if let Some(last) = lines.last_mut() {
                last.push(',');
            }
        } else {
            current.push_str(&piece);
        }
    }
    lines.push(current);
    for (index, part) in lines.iter().enumerate() {
        let indent = if index == 0 { indent } else { indent + 4 };
        if style.ansi {
            out.push_str(&format!("{}\u{1b}[1m{part}\u{1b}[0m\n", " ".repeat(indent)));
        } else {
            line(out, indent, part);
        }
    }
}

fn line(out: &mut String, indent: usize, text: &str) {
    out.push_str(&" ".repeat(indent));
    out.push_str(text);
    out.push('\n');
}

/// `text` wrapped at `width`: the first line at `first`, the rest at `rest`.
fn wrapped(out: &mut String, first: usize, rest: usize, text: &str, width: usize) {
    let mut current = " ".repeat(first);
    let mut used = first;
    let mut empty = true;
    for word in text.split_whitespace() {
        let size = word.chars().count();
        if !empty && used + 1 + size > width {
            out.push_str(current.trim_end());
            out.push('\n');
            current = " ".repeat(rest);
            used = rest;
            empty = true;
        }
        if !empty {
            current.push(' ');
            used += 1;
        }
        current.push_str(word);
        used += size;
        empty = false;
    }
    if !empty {
        out.push_str(current.trim_end());
        out.push('\n');
    }
}

/// CommonMark `text` as terminal text indented by `indent`.
fn markdown_text(out: &mut String, text: &str, indent: usize, style: &Style) {
    let mut writer = TextWriter {
        out,
        indent,
        style,
        inline: String::new(),
        lists: Vec::new(),
        marker: None,
        code: None,
        link: Vec::new(),
        row: Vec::new(),
        table: Vec::new(),
        blocks: 0,
    };
    for event in Parser::new_ext(text, options()) {
        writer.event(event);
    }
    writer.flush_inline();
}

struct TextWriter<'a> {
    out: &'a mut String,
    indent: usize,
    style: &'a Style,
    inline: String,
    /// Open lists: the next number, or None for bullets.
    lists: Vec<Option<u64>>,
    /// The marker of a list item whose text has not been written yet.
    marker: Option<String>,
    code: Option<String>,
    link: Vec<String>,
    row: Vec<String>,
    table: Vec<Vec<String>>,
    blocks: usize,
}

impl TextWriter<'_> {
    fn gap(&mut self) {
        if self.blocks > 0 && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
        self.blocks += 1;
    }

    /// Where the lines of the innermost list item start.
    fn item_indent(&self) -> usize {
        match self.lists.len() {
            0 => self.indent,
            depth => self.indent + 2 * (depth - 1) + 2,
        }
    }

    fn flush_inline(&mut self) {
        let text = std::mem::take(&mut self.inline);
        let body = self.item_indent();
        match self.marker.take() {
            Some(marker) => {
                let start = body.saturating_sub(2);
                let mut lines = String::new();
                wrapped(&mut lines, 0, 0, &text, self.style.width.saturating_sub(body).max(20));
                let mut first = true;
                for part in lines.lines() {
                    if first {
                        self.out.push_str(&" ".repeat(start));
                        self.out.push_str(&marker);
                        self.out.push(' ');
                        self.out.push_str(part);
                        first = false;
                    } else {
                        self.out.push_str(&" ".repeat(body + marker.len().saturating_sub(1)));
                        self.out.push_str(part);
                    }
                    self.out.push('\n');
                }
                if first {
                    self.out.push_str(&" ".repeat(start));
                    self.out.push_str(&marker);
                    self.out.push('\n');
                }
            }
            None => {
                if text.trim().is_empty() {
                    return;
                }
                for part in text.split('\n') {
                    wrapped(self.out, body, body, part, self.style.width);
                }
            }
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(Tag::Paragraph) => {
                if self.lists.is_empty() {
                    self.gap();
                }
            }
            Event::End(TagEnd::Paragraph) | Event::End(TagEnd::Item) => self.flush_inline(),
            Event::Start(Tag::Heading { .. }) => {
                self.flush_inline();
                self.gap();
            }
            Event::End(TagEnd::Heading(_)) => {
                let title = std::mem::take(&mut self.inline);
                if self.style.ansi {
                    self.out.push_str(&format!("{}\u{1b}[1m{}\u{1b}[0m\n", " ".repeat(self.indent), title.trim()));
                } else {
                    line(self.out, self.indent, title.trim());
                }
            }
            Event::Start(Tag::List(start)) => {
                self.flush_inline();
                if self.lists.is_empty() {
                    self.gap();
                }
                self.lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                self.flush_inline();
                self.lists.pop();
            }
            Event::Start(Tag::Item) => {
                self.flush_inline();
                self.marker = Some(match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = format!("{number}.");
                        *number += 1;
                        marker
                    }
                    _ => "-".to_string(),
                });
            }
            Event::Start(Tag::CodeBlock(_)) => {
                self.flush_inline();
                if self.lists.is_empty() {
                    self.gap();
                }
                self.code = Some(String::new());
            }
            Event::End(TagEnd::CodeBlock) => {
                let code = self.code.take().unwrap_or_default();
                let indent = self.item_indent() + 4;
                for code_line in code.trim_end_matches('\n').lines() {
                    if code_line.is_empty() {
                        self.out.push('\n');
                    } else {
                        line(self.out, indent, code_line);
                    }
                }
            }
            Event::Text(text) => match self.code.as_mut() {
                Some(code) => code.push_str(&text),
                None => self.inline.push_str(&text),
            },
            Event::Code(code) => self.inline.push_str(&format!("`{code}`")),
            Event::Html(html) | Event::InlineHtml(html) => self.inline.push_str(&html),
            Event::SoftBreak => self.inline.push(' '),
            Event::HardBreak => self.inline.push('\n'),
            Event::Start(Tag::Link { dest_url, .. }) => self.link.push(dest_url.to_string()),
            Event::End(TagEnd::Link) => {
                if let Some(url) = self.link.pop()
                    && !url.is_empty()
                    && !self.inline.ends_with(url.as_str())
                {
                    self.inline.push_str(&format!(" <{url}>"));
                }
            }
            Event::Start(Tag::Table(_)) => {
                self.flush_inline();
                self.gap();
                self.table.clear();
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                let row = std::mem::take(&mut self.row);
                self.table.push(row);
            }
            Event::End(TagEnd::TableCell) => {
                let cell = std::mem::take(&mut self.inline);
                self.row.push(cell.trim().to_string());
            }
            Event::End(TagEnd::Table) => {
                let rows = std::mem::take(&mut self.table);
                let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
                let widths: Vec<usize> = (0..columns)
                    .map(|column| rows.iter().filter_map(|row| row.get(column)).map(|cell| cell.chars().count()).max().unwrap_or(0))
                    .collect();
                for row in rows {
                    let cells: Vec<String> = row
                        .iter()
                        .enumerate()
                        .map(|(column, cell)| format!("{cell:width$}", width = widths[column]))
                        .collect();
                    line(self.out, self.indent, cells.join("  ").trim_end());
                }
            }
            Event::Rule => {
                self.flush_inline();
                self.gap();
                line(self.out, self.indent, "----");
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- markdown

/// The reference page of a module as Markdown. Doc comments are already
/// CommonMark, so they are kept as written.
pub fn module_markdown(doc: &ModuleDoc) -> String {
    let parts = overview(doc);
    let mut out = format!("# {}\n", doc.path);
    let summary = doc.summary();
    if !summary.is_empty() {
        out.push_str(&format!("\n{summary}\n"));
    }
    if let Some(version) = &doc.version {
        out.push_str(&format!("\n**Version:** `{version}`\n"));
    }
    if let Some(synopsis) = &parts.synopsis {
        out.push_str(&format!("\n## Synopsis\n\n```okkama\n{synopsis}\n```\n"));
    }
    if !parts.description.trim().is_empty() {
        out.push_str(&format!("\n## Description\n\n{}\n", parts.description.trim()));
    }
    let api: Vec<&ItemDoc> = doc.items.iter().filter(|item| item.kind != ItemKind::Error).collect();
    if !api.is_empty() {
        out.push_str("\n## Public API\n");
        for item in api {
            item_markdown_block(&mut out, item, None, 3);
        }
    }
    let errors: Vec<&ItemDoc> = doc.items.iter().filter(|item| item.kind == ItemKind::Error).collect();
    if !errors.is_empty() || !parts.errors.trim().is_empty() {
        out.push_str("\n## Errors\n");
        if !parts.errors.trim().is_empty() {
            out.push_str(&format!("\n{}\n", parts.errors.trim()));
        }
        for item in errors {
            item_markdown_block(&mut out, item, None, 3);
        }
    }
    if !parts.engines.trim().is_empty() {
        out.push_str(&format!("\n## Engine notes\n\n{}\n", parts.engines.trim()));
    }
    if !parts.see_also.trim().is_empty() {
        out.push_str(&format!("\n## See also\n\n{}\n", parts.see_also.trim()));
    }
    out
}

/// One declaration, or some of its members, as Markdown.
pub fn item_markdown(doc: &ModuleDoc, item: &ItemDoc, members: Option<&[&MemberDoc]>) -> String {
    let mut title = format!("{} {}", doc.path, item.name);
    if let Some(members) = members
        && let Some(first) = members.first()
    {
        title = format!("{title}.{}", first.name);
    }
    let mut out = format!("# {title}\n");
    if let Some(version) = &doc.version {
        out.push_str(&format!("\n**Module:** `{} {version}`\n", doc.path));
    }
    item_markdown_block(&mut out, item, members, 2);
    out
}

/// The index of modules as Markdown.
pub fn index_markdown(entries: &[IndexEntry]) -> String {
    let mut out = "# Okkama standard library\n".to_string();
    let mut group = None::<String>;
    for entry in entries {
        let area = area_of(&entry.path);
        if group.as_deref() != Some(area.as_str()) {
            out.push_str(&format!("\n## {area}\n\n"));
            group = Some(area);
        }
        if entry.summary.is_empty() {
            out.push_str(&format!("- `{}`\n", entry.path));
        } else {
            out.push_str(&format!("- `{}`: {}\n", entry.path, entry.summary));
        }
    }
    out
}

fn item_markdown_block(out: &mut String, item: &ItemDoc, only: Option<&[&MemberDoc]>, level: usize) {
    let hashes = "#".repeat(level);
    out.push_str(&format!("\n{hashes} {} {}\n\n```okkama\n{}\n```\n", item.kind.word(), item.name, item.signature));
    if only.is_none()
        && let Some(doc) = &item.doc
    {
        out.push_str(&format!("\n{}\n", doc.text.trim()));
    }
    let members: Vec<&MemberDoc> = match only {
        Some(members) => members.to_vec(),
        None => item.members.iter().collect(),
    };
    for member in members {
        out.push_str(&format!("\n{hashes}# {}.{}\n\n```okkama\n{}\n```\n", item.name, member.name, member.signature));
        if let Some(doc) = &member.doc {
            out.push_str(&format!("\n{}\n", doc.text.trim()));
        }
    }
}
