//! The documentation model (#512): what a module says about itself in its
//! `//` comments, attached to the module and to its public declarations, with
//! signatures taken from the syntax tree. `okkama man`, the language server and
//! a future reference site read this one model; none of them keeps a copy.
//!
//! Comments stay ordinary `//` comments whose text is a CommonMark subset
//! (see [`render`]). Where a comment belongs:
//! - **A declaration** owns the run of comment lines right above its first
//!   line, with no blank line between. A blank line or a line of code ends the
//!   run. This holds for module declarations and for the members of objects,
//!   roles and caps alike.
//! - **The module** owns the comment blocks between the `module` line and its
//!   first declaration that no declaration owns: the canonical place, right
//!   after `module` and before the imports, and the older one after the
//!   imports. Both kinds are kept; when a module has both, a
//!   [`DocNote`] says so rather than one being dropped.
//! - **Elsewhere**, a comment block that no declaration owns is reported as a
//!   [`DocNote`], never attached to a neighbour.
//!
//! Comment lines are found with the lexer's tokens, so `//` inside a string,
//! even a string over several lines, is never a comment, and nothing inside an
//! action body is ever taken for documentation.
//!
//! Hidden declarations, fields, tests, fakes and `destroy` never appear.

pub mod render;
#[cfg(test)]
mod tests;

use crate::ide::outline::{result_text, type_text};
use crate::source::{SourceFile, Span};
use crate::syntax::{self, ParseMode, ParseOptions, ast};
use crate::syntax::token::TokenKind;

/// One module's documentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleDoc {
    /// `io/stdout`, as the `module` line names it.
    pub path: String,
    /// `v1.0.0`, only when the `module` line declares one.
    pub version: Option<String>,
    /// The source file's name.
    pub file: String,
    /// The module overview, when it has one.
    pub overview: Option<DocText>,
    /// Public module-scope declarations in source order.
    pub items: Vec<ItemDoc>,
    /// Where the attachment rules met something worth saying.
    pub notes: Vec<DocNote>,
}

/// The text of one comment block, without its `//` markers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocText {
    pub text: String,
    /// From the first comment line to the end of the last.
    pub span: Span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Object,
    Role,
    Enum,
    /// `enum Name is Error = (…)`: a failure contract of the module.
    Error,
    Const,
    Native,
    NativeObject,
    /// `cap Owner::Name { … }` at module scope.
    Cap,
}

impl ItemKind {
    pub fn word(self) -> &'static str {
        match self {
            ItemKind::Object => "object",
            ItemKind::Role => "role",
            ItemKind::Enum | ItemKind::Error => "enum",
            ItemKind::Const => "const",
            ItemKind::Native => "native",
            ItemKind::NativeObject => "native object",
            ItemKind::Cap => "cap",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MemberKind {
    Constructor,
    Action,
    /// An action signature of a role.
    Signature,
    Cap,
}

/// A public module-scope declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemDoc {
    pub name: String,
    pub kind: ItemKind,
    /// As it would be declared, without a body: `object Stdout does Writing`.
    pub signature: String,
    pub doc: Option<DocText>,
    /// Constructors, public actions and caps of an object; the action
    /// signatures of a role.
    pub members: Vec<MemberDoc>,
    pub span: Span,
}

impl ItemDoc {
    /// The members called `name`: every overload of an action.
    pub fn members_named(&self, name: &str) -> Vec<&MemberDoc> {
        self.members.iter().filter(|member| member.name == name).collect()
    }
}

/// A public member of an object or role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberDoc {
    pub name: String,
    pub kind: MemberKind,
    /// `async write(bytes is @byte) is AsyncWriting`.
    pub signature: String,
    pub doc: Option<DocText>,
    pub span: Span,
}

/// Something the attachment rules want a reader of the source to know: a
/// comment no declaration owns, or an overview in two places.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocNote {
    pub span: Span,
    pub message: String,
}

impl ModuleDoc {
    /// The public declaration called `name`.
    pub fn item(&self, name: &str) -> Option<&ItemDoc> {
        self.items.iter().find(|item| item.name == name)
    }

    /// The first sentence of the overview, for an index line.
    pub fn summary(&self) -> String {
        self.overview.as_ref().map(|overview| summary(&overview.text)).unwrap_or_default()
    }

    /// The documentation of a parsed module.
    pub fn of(source: &SourceFile, program: &ast::Program) -> ModuleDoc {
        let lines = CommentLines::new(source);
        let mut notes = Vec::new();
        let first_item = program.items.first().map(|item| lines.line_of(item.span().start));
        let header_end = first_item.unwrap_or(lines.count() + 1);
        let module_line = lines.line_of(program.module.span.end.saturating_sub(1));
        let imports_end = program
            .imports
            .last()
            .map(|import| lines.line_of(import.span.end.saturating_sub(1)));
        // The overview: the comment blocks of the header that no declaration owns.
        let mut overview_blocks = Vec::new();
        for block in lines.blocks(module_line + 1, header_end) {
            if Some(block.last + 1) == first_item {
                continue;
            }
            overview_blocks.push(block);
        }
        let before_imports = overview_blocks
            .iter()
            .any(|block| imports_end.is_some_and(|end| block.last < end));
        let after_imports = overview_blocks
            .iter()
            .any(|block| imports_end.is_none_or(|end| block.first > end));
        if before_imports && after_imports && imports_end.is_some() {
            notes.push(DocNote {
                span: lines.span(overview_blocks[0].first, overview_blocks[overview_blocks.len() - 1].last),
                message: "the module overview is written both before and after the imports; both are kept"
                    .to_string(),
            });
        }
        // Without an overview, a long comment glued to the first declaration
        // after the imports may be the overview written in the old place: it
        // stays the declaration's, and the note asks for a blank line.
        if overview_blocks.is_empty()
            && let Some(first) = first_item
            && let Some(block) = lines.blocks(module_line + 1, header_end).last().copied()
            && block.last + 1 == first
            && imports_end.is_some_and(|end| block.first > end)
            && block.last - block.first >= 2
        {
            notes.push(DocNote {
                span: lines.span(block.first, block.last),
                message: "this comment may be the module overview: it is attached to the first declaration; \
                          a blank line after it would make it the overview"
                    .to_string(),
            });
        }
        let overview = (!overview_blocks.is_empty()).then(|| DocText {
            text: overview_blocks
                .iter()
                .map(|block| lines.text(block))
                .collect::<Vec<_>>()
                .join("\n\n"),
            span: lines.span(overview_blocks[0].first, overview_blocks[overview_blocks.len() - 1].last),
        });
        let mut items = Vec::new();
        let mut owned = Vec::new();
        for item in &program.items {
            let Some(doc) = item_doc(source, &lines, item) else {
                continue;
            };
            if let Some(text) = &doc.doc {
                owned.push(text.span);
            }
            for member in &doc.members {
                if let Some(text) = &member.doc {
                    owned.push(text.span);
                }
            }
            items.push(doc);
        }
        // Comment blocks after the header that nothing owns.
        if let Some(first) = first_item {
            for block in lines.blocks(first, lines.count() + 1) {
                let span = lines.span(block.first, block.last);
                let attached = lines.code_at(block.last + 1);
                if !attached && !owned.contains(&span) && !lines.inside_body(block.first, program) {
                    notes.push(DocNote {
                        span,
                        message: "this comment belongs to no declaration: a blank line separates it".to_string(),
                    });
                }
            }
        }
        ModuleDoc {
            path: program.module.path.iter().map(|part| part.text.as_str()).collect::<Vec<_>>().join("/"),
            version: program.module.version.as_ref().map(ast::Version::text),
            file: source.name().to_string(),
            overview,
            items,
            notes,
        }
    }

    /// Reads and parses `path` and documents it. Test bodies are skipped, as
    /// a production build skips them.
    pub fn load(path: &std::path::Path) -> Result<ModuleDoc, String> {
        let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        ModuleDoc::parse(path.display().to_string(), text)
    }

    /// Documents source `text` called `name`.
    pub fn parse(name: String, text: String) -> Result<ModuleDoc, String> {
        let source = SourceFile::new(name.clone(), text).map_err(|_| format!("{name}: the file is too large"))?;
        let options = ParseOptions {
            mode: ParseMode::Production,
            ..ParseOptions::default()
        };
        let program = syntax::parse(&source, &options).map_err(|diagnostics| {
            let first = diagnostics.first().map(|diagnostic| diagnostic.message.clone()).unwrap_or_default();
            format!("{name}: does not parse: {first}")
        })?;
        Ok(ModuleDoc::of(&source, &program))
    }
}

/// The comment above the declaration that starts at `offset`, by the rule of
/// this module; the language server's hover and completion read it too.
pub fn comment_above(source: &SourceFile, offset: u32) -> String {
    let lines = CommentLines::new(source);
    lines.above(lines.line_of(offset)).map(|text| text.text).unwrap_or_default()
}

/// The module overview of `source`, by the rule of this module; an empty
/// string when the module has none or the source does not parse.
pub fn module_overview(source: &SourceFile) -> String {
    let options = ParseOptions {
        mode: ParseMode::Production,
        ..ParseOptions::default()
    };
    syntax::parse(source, &options)
        .ok()
        .and_then(|program| ModuleDoc::of(source, &program).overview)
        .map(|overview| overview.text)
        .unwrap_or_default()
}

/// The first sentence of `text`: up to the first ". " or the end of its first
/// paragraph, on one line.
pub fn summary(text: &str) -> String {
    let paragraph: Vec<&str> = text
        .lines()
        .skip_while(|line| line.trim().is_empty())
        .take_while(|line| !line.trim().is_empty() && !line.starts_with("    "))
        .map(str::trim)
        .collect();
    let joined = paragraph.join(" ");
    match joined.find(". ") {
        Some(at) => joined[..=at].to_string(),
        None => joined,
    }
}

fn item_doc(source: &SourceFile, lines: &CommentLines<'_>, item: &ast::Item) -> Option<ItemDoc> {
    let doc = lines.above(lines.line_of(item.span().start));
    Some(match item {
        ast::Item::Object(decl) => {
            if decl.hidden {
                return None;
            }
            let mut signature = format!("object {}{}", decl.name.text, generics_text(&decl.generics));
            if !decl.roles.is_empty() {
                let roles: Vec<String> = decl.roles.iter().map(type_text).collect();
                signature.push_str(&format!(" does {}", roles.join(", ")));
            }
            let members = decl.members.iter().filter_map(|member| object_member(source, lines, member)).collect();
            ItemDoc { name: decl.name.text.clone(), kind: ItemKind::Object, signature, doc, members, span: decl.span }
        }
        ast::Item::Role(decl) => {
            if decl.hidden {
                return None;
            }
            let members = decl
                .members
                .iter()
                .filter_map(|member| match member {
                    ast::RoleMember::Signature(signature) => Some(MemberDoc {
                        name: signature.name.text.clone(),
                        kind: MemberKind::Signature,
                        signature: action_text(source, signature.is_async, &signature.name, &signature.params, &signature.result),
                        doc: lines.above(lines.line_of(signature.span.start)),
                        span: signature.span,
                    }),
                    _ => None,
                })
                .collect();
            ItemDoc {
                name: decl.name.text.clone(),
                kind: ItemKind::Role,
                signature: format!("role {}{}", decl.name.text, generics_text(&decl.generics)),
                doc,
                members,
                span: decl.span,
            }
        }
        ast::Item::Enum(decl) => {
            if decl.hidden {
                return None;
            }
            let cases: Vec<String> = decl
                .cases
                .iter()
                .zip(decl.payloads.iter().map(Some).chain(std::iter::repeat(None)))
                .map(|(case, payload)| match payload {
                    Some(params) if !params.is_empty() => format!("{}{}", case.text, params_text(source, params)),
                    _ => case.text.clone(),
                })
                .collect();
            let role = decl.role.as_ref().map(|role| format!(" is {}", role.text)).unwrap_or_default();
            let error = decl.role.as_ref().is_some_and(|role| role.text == "Error");
            ItemDoc {
                name: decl.name.text.clone(),
                kind: if error { ItemKind::Error } else { ItemKind::Enum },
                signature: format!("enum {}{}{role} = ({})", decl.name.text, generics_text(&decl.generics), cases.join(", ")),
                doc,
                members: Vec::new(),
                span: decl.span,
            }
        }
        ast::Item::Const(decl) => {
            if decl.hidden {
                return None;
            }
            let ty = decl.ty.as_ref().map(|ty| format!(" is {}", type_text(ty))).unwrap_or_default();
            let value = one_line(source.slice(decl.value.span));
            let value = if value.chars().count() > 60 { String::new() } else { format!(" = {value}") };
            ItemDoc {
                name: decl.name.text.clone(),
                kind: ItemKind::Const,
                signature: format!("const {}{ty}{value}", decl.name.text),
                doc,
                members: Vec::new(),
                span: decl.span,
            }
        }
        ast::Item::NativeAction(decl) => ItemDoc {
            name: decl.name.text.clone(),
            kind: ItemKind::Native,
            signature: format!("native {}", action_text(source, decl.is_async, &decl.name, &decl.params, &decl.result)),
            doc,
            members: Vec::new(),
            span: decl.span,
        },
        ast::Item::NativeObject(decl) => ItemDoc {
            name: decl.name.text.clone(),
            kind: ItemKind::NativeObject,
            signature: format!("native object {}", decl.name.text),
            doc,
            members: Vec::new(),
            span: decl.span,
        },
        ast::Item::Cap(decl) => ItemDoc {
            name: decl.owner.as_ref().map_or_else(|| decl.name.text.clone(), |owner| format!("{}::{}", owner.text, decl.name.text)),
            kind: ItemKind::Cap,
            signature: cap_text(decl),
            doc,
            members: Vec::new(),
            span: decl.span,
        },
        ast::Item::Test(_) | ast::Item::Action(_) => return None,
    })
}

fn object_member(source: &SourceFile, lines: &CommentLines<'_>, member: &ast::ObjectMember) -> Option<MemberDoc> {
    let doc = lines.above(lines.line_of(member.span().start));
    Some(match member {
        ast::ObjectMember::Constructor(ctor) => MemberDoc {
            name: ctor.name.text.clone(),
            kind: MemberKind::Constructor,
            signature: match &ctor.form {
                ast::ConstructorForm::Full { params, .. } => format!("{}{}", ctor.name.text, params_text(source, params)),
                ast::ConstructorForm::Shorthand { fields, .. } => {
                    let fields: Vec<String> = fields.iter().map(|field| format!(":{}", field.text)).collect();
                    format!("{}({})", ctor.name.text, fields.join(", "))
                }
            },
            doc,
            span: ctor.span,
        },
        ast::ObjectMember::Action(action) => {
            if action.visibility == ast::Visibility::Hidden {
                return None;
            }
            MemberDoc {
                name: action.name.text.clone(),
                kind: MemberKind::Action,
                signature: action_text(source, action.is_async, &action.name, &action.params, &action.result),
                doc,
                span: action.span,
            }
        }
        ast::ObjectMember::Cap(cap) => MemberDoc {
            name: cap.name.text.clone(),
            kind: MemberKind::Cap,
            signature: cap_text(cap),
            doc,
            span: cap.span,
        },
        ast::ObjectMember::Has(_) | ast::ObjectMember::Destroy(_) | ast::ObjectMember::Test(_) => return None,
    })
}

/// `async name(a is int, b is string = "x") is T`.
fn action_text(source: &SourceFile, is_async: bool, name: &ast::Name, params: &[ast::Param], result: &ast::ResultSpec) -> String {
    format!(
        "{}{}{} is {}",
        if is_async { "async " } else { "" },
        name.text,
        params_text(source, params),
        result_text(result)
    )
}

/// `(a is int, rest is ...string, limit is int = 10)`: defaults as written.
fn params_text(source: &SourceFile, params: &[ast::Param]) -> String {
    let params: Vec<String> = params
        .iter()
        .map(|param| {
            let default = param
                .default
                .as_ref()
                .map(|value| format!(" = {}", one_line(source.slice(value.span))))
                .unwrap_or_default();
            format!(
                "{} is {}{}{default}",
                param.name.text,
                if param.variadic { "..." } else { "" },
                type_text(&param.ty)
            )
        })
        .collect();
    format!("({})", params.join(", "))
}

fn generics_text(generics: &[ast::GenericParam]) -> String {
    if generics.is_empty() {
        return String::new();
    }
    let params: Vec<String> = generics
        .iter()
        .map(|param| match &param.bound {
            Some(bound) => format!("{}: {}", param.name.text, type_text(bound)),
            None => param.name.text.clone(),
        })
        .collect();
    format!("<{}>", params.join(", "))
}

fn cap_text(cap: &ast::CapDecl) -> String {
    let owner = cap.owner.as_ref().map(|owner| format!("{}::", owner.text)).unwrap_or_default();
    let fields: Vec<String> =
        cap.fields.iter().map(|field| format!("{} is {}", field.name.text, type_text(&field.ty))).collect();
    format!("cap {owner}{} {{ {} }}", cap.name.text, fields.join("; "))
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// FNV-1a of a text: an edited buffer at the same address is told apart.
fn fingerprint(text: &str) -> u64 {
    text.bytes().fold(1469598103934665603u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(1099511628211))
}

/// A text's address, length and fingerprint, and its line kinds.
type Classified = (usize, usize, u64, Vec<Line>);

/// A run of comment-only lines, 1-based and inclusive.
#[derive(Copy, Clone, Debug)]
struct Block {
    first: u32,
    last: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Line {
    Blank,
    Comment,
    Code,
}

/// Each line of a source classified with the lexer's tokens.
struct CommentLines<'a> {
    source: &'a SourceFile,
    kinds: Vec<Line>,
}

impl<'a> CommentLines<'a> {
    fn new(source: &'a SourceFile) -> Self {
        // The language server asks for the comment above every symbol of a
        // file in turn: the last file's classification is kept.
        thread_local! {
            static LAST: std::cell::RefCell<Option<Classified>> = const { std::cell::RefCell::new(None) };
        }
        let text = source.text();
        let key = (text.as_ptr() as usize, text.len(), fingerprint(text));
        if let Some(kinds) = LAST.with(|last| {
            last.borrow()
                .as_ref()
                .filter(|(pointer, length, print, _)| (*pointer, *length, *print) == key)
                .map(|(_, _, _, kinds)| kinds.clone())
        }) {
            return CommentLines { source, kinds };
        }
        let lines = CommentLines::classify(source);
        LAST.with(|last| *last.borrow_mut() = Some((key.0, key.1, key.2, lines.kinds.clone())));
        lines
    }

    fn classify(source: &'a SourceFile) -> Self {
        let count = source.line_count();
        let mut covered = vec![false; count as usize + 2];
        for token in syntax::lexer::lex(source.text()).tokens {
            if token.kind == TokenKind::Eof || token.span.start == token.span.end {
                continue;
            }
            let first = source.location(token.span.start).line;
            let last = source.location(token.span.end - 1).line;
            for line in first..=last {
                if let Some(slot) = covered.get_mut(line as usize) {
                    *slot = true;
                }
            }
        }
        let kinds = (1..=count)
            .map(|line| {
                let text = source.line_text(line).trim();
                if covered[line as usize] {
                    Line::Code
                } else if text.starts_with("//") {
                    Line::Comment
                } else if text.is_empty() {
                    Line::Blank
                } else {
                    Line::Code
                }
            })
            .collect();
        CommentLines { source, kinds }
    }

    fn count(&self) -> u32 {
        self.kinds.len() as u32
    }

    fn kind(&self, line: u32) -> Line {
        if line == 0 {
            return Line::Blank;
        }
        self.kinds.get(line as usize - 1).copied().unwrap_or(Line::Blank)
    }

    fn code_at(&self, line: u32) -> bool {
        self.kind(line) == Line::Code
    }

    fn line_of(&self, offset: u32) -> u32 {
        self.source.location(offset).line
    }

    /// The comment owned by the declaration on `line`.
    fn above(&self, line: u32) -> Option<DocText> {
        let mut first = line;
        while first > 1 && self.kind(first - 1) == Line::Comment {
            first -= 1;
        }
        (first < line).then(|| {
            let block = Block { first, last: line - 1 };
            DocText { text: self.text(&block), span: self.span(block.first, block.last) }
        })
    }

    /// The comment blocks starting at lines `from..until`.
    fn blocks(&self, from: u32, until: u32) -> Vec<Block> {
        let mut out = Vec::new();
        let mut line = from.max(1);
        while line < until && line <= self.count() {
            if self.kind(line) == Line::Comment {
                let first = line;
                while line + 1 < until && self.kind(line + 1) == Line::Comment {
                    line += 1;
                }
                out.push(Block { first, last: line });
            }
            line += 1;
        }
        out
    }

    fn text(&self, block: &Block) -> String {
        (block.first..=block.last)
            .map(|line| {
                let text = self.source.line_text(line).trim_start();
                let text = text.strip_prefix("//").unwrap_or(text);
                text.strip_prefix(' ').unwrap_or(text).trim_end_matches('\r').to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn span(&self, first: u32, last: u32) -> Span {
        let text = self.source.text();
        let start = self.offset_of_line(first);
        let end = self.offset_of_line(last + 1).min(text.len() as u32);
        let end = if end > start && text.as_bytes().get(end as usize - 1) == Some(&b'\n') { end - 1 } else { end };
        Span::new(start, end)
    }

    fn offset_of_line(&self, line: u32) -> u32 {
        let text = self.source.text();
        if line <= 1 {
            return 0;
        }
        let mut seen = 1;
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                seen += 1;
                if seen == line {
                    return index as u32 + 1;
                }
            }
        }
        text.len() as u32
    }

    /// Whether `line` lies inside the body of a declaration: a comment there
    /// documents code, not an API.
    fn inside_body(&self, line: u32, program: &ast::Program) -> bool {
        program.items.iter().any(|item| {
            let span = item.span();
            self.line_of(span.start) < line && line < self.line_of(span.end.saturating_sub(1))
        })
    }
}
