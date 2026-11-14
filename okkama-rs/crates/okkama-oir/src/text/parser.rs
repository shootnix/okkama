use std::collections::HashMap;
use super::{lexer::{lex, Token}, *};

pub(super) type Names = HashMap<(usize, &'static str, String), u32>;

pub fn parse(input: &str) -> Result<Module, Diagnostic> {
    parse_with_limits(input, Limits::default())
}

pub fn parse_with_limits(input: &str, limits: Limits) -> Result<Module, Diagnostic> {
    // Header validation deliberately precedes lexing or limits on the body.
    let (profile, offset) = header(input)?;
    if input.len() > limits.max_text_bytes {
        return Err(Diagnostic::new(DiagnosticKind::Limit, Span { start: 0,
            end: input.len() }, "text size limit"));
    }
    let tokens = lex(input, offset, limits)?;
    let mut names = Names::new();
    let mut annotations = HashMap::new();
    let mut first = Reader::new(&tokens, &mut names, &mut annotations, true, limits, profile);
    first.module()?;
    let summary_names: Vec<_> = names.iter().filter_map(|((function, ns, name), _)| {
        (*ns == "function-summary-name").then_some((*function, name.clone()))
    }).collect();
    for (function, name) in summary_names {
        if let Some(id) = names.get(&(0, "summary", name)).copied() {
            names.insert((0, "function-summary", function.to_string()), id);
        }
    }
    let mut second = Reader::new(&tokens, &mut names, &mut annotations, false, limits, profile);
    let mut module = second.module()?;
    second.resolve_slots(&mut module)?;
    check_type_depth(&module, limits.max_type_depth).map_err(|message| second.error(DiagnosticKind::Limit, message))?;
    Ok(module)
}

fn header(input: &str) -> Result<(Profile, usize), Diagnostic> {
    let end = input.find('\n').unwrap_or(input.len());
    let line = input[..end].strip_suffix('\r').unwrap_or(&input[..end]);
    let line = line.split_once("//").map_or(line, |(head, _)| head);
    let words: Vec<_> = line.split_whitespace().take(4).collect();
    let span = Span { start: 0, end };
    if words.first() != Some(&"oir") {
        return Err(Diagnostic::new(DiagnosticKind::Header, span,
        "expected oir 2 <profile>"));
    }
    if words.get(1) != Some(&"2") {
        return Err(Diagnostic::new(DiagnosticKind::UnsupportedVersion, span,
            format!("unsupported OIR version {}; expected 2 (no v1 reader)",
                words.get(1).copied().unwrap_or("<missing>"))));
    }
    let Some(profile) = words.get(2) else {
        return Err(Diagnostic::new(DiagnosticKind::MissingProfile, span,
        "oir 2 requires high, own, coro or low"));
    };
    let profile = match *profile {
        "high" => Profile::High, "own" => Profile::Own, "coro" => Profile::Coro,
        "low" => Profile::Low,
        _ => return Err(Diagnostic::new(DiagnosticKind::UnknownProfile, span,
            format!("unknown OIR-2 profile {profile}")))
    };
    if words.len() != 3 {
        return Err(Diagnostic::new(DiagnosticKind::Header, span, "unexpected header tokens"));
    }
    Ok((profile, if end < input.len() {
        end + 1
    } else {
        end
    }))
}

struct PendingSlot {
    scope: usize,
    block: BlockId,
    outcome: Outcome,
    index: usize,
    token: Token,
}

pub(crate) struct Reader<'a> {
    pub(super) tokens: &'a [Token],
    pub(super) pos: usize,
    pub(super) names: &'a mut Names,
    pub(super) annotations: &'a mut HashMap<(usize, ValueId), String>,
    pub(super) counts: HashMap<(usize, &'static str), u32>,
    pub(super) first: bool,
    pub(crate) scope: usize,
    pub(super) limits: Limits,
    entities: usize,
    pub(super) profile: Profile,
    pending: Vec<PendingSlot>,
    current_block: BlockId,
    pub(super) types: TypeTable,
    pub(super) generic_owner: Option<GenericOwner>,
    pub(super) values: Arena<ValueId, ValueData>,
    pub(super) insts: Arena<InstId, Instruction>,
    pub(super) selections: Arena<SelectionId, SelectionExpr>,
    pub(super) deps: Arena<DepExprId, DepExpr>,
    pub(super) implicit: Vec<InstId>,
    pub(super) default_scope: Option<ScopeId>,
    pub(super) literal_type: Option<TypeId>,
    pub(super) type_depth: usize,
}
impl<'a> Reader<'a> {
    fn new(
        tokens: &'a [Token], names: &'a mut Names, annotations: &'a mut HashMap<(usize, ValueId), String>,
        first: bool, limits: Limits, profile: Profile,
    ) -> Self {
        Self { tokens, pos: 0, names, annotations, counts: HashMap::new(), first, scope: 0, limits, entities: 0,
            profile, pending: vec![], current_block: BlockId::from_u32(0), types: TypeTable::new(),
            generic_owner: None, values: Arena::new(), insts: Arena::new(), selections: Arena::new(),
            deps: Arena::new(), implicit: vec![], default_scope: None, literal_type: None,
            type_depth: 0 }
    }
    pub(crate) fn peek(&self) -> &Token {
        let mut p = self.pos;
        while p + 1 < self.tokens.len() && self.tokens[p].text == "\n" && !self.tokens[p].quoted {
            p += 1;
        }
        &self.tokens[p.min(self.tokens.len() - 1)]
    }
    pub(crate) fn next(&mut self) -> Result<Token, Diagnostic> {
        while self.pos + 1 < self.tokens.len() && self.tokens[self.pos].text == "\n"
            && !self.tokens[self.pos].quoted
        {
            self.pos += 1;
        }
        let token = self.tokens[self.pos].clone();
        if token.text == "<eof>" && !token.quoted {
            return Err(self.at(&token, DiagnosticKind::Syntax,
            "unexpected end of text"));
        }
        self.pos += 1;
        Ok(token)
    }
    pub(crate) fn is(&self, text: &str) -> bool {
        !self.peek().quoted && self.peek().text == text
    }
    pub(crate) fn eat(&mut self, text: &str) -> bool {
        if self.is(text) {
            let _ = self.next();
            true
        } else {
            false
        }
    }
    pub(crate) fn expect(&mut self, text: &str) -> Result<(), Diagnostic> {
        if self.eat(text) {
            Ok(())
        } else {
            Err(self.error(DiagnosticKind::Syntax, format!("expected {text}")))
        }
    }
    pub(crate) fn at(&self, token: &Token, kind: DiagnosticKind,
        message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(kind, token.span, message)
    }
    pub(crate) fn error(&self, kind: DiagnosticKind, message: impl Into<String>) -> Diagnostic {
        self.at(self.peek(),
            kind, message)
    }
    pub(crate) fn field(&mut self, name: &str) -> Result<(), Diagnostic> {
        self.expect(name)?;
        self.expect("=")
    }
    pub(crate) fn comma(&mut self) -> Result<(), Diagnostic> {
        if self.eat(",") || self.is(")") || self.is("]") {
            Ok(())
        } else {
            Err(self.error(DiagnosticKind::Syntax,
            "expected comma"))
        }
    }
    pub(crate) fn entity(&mut self) -> Result<(), Diagnostic> {
        self.entities = self.entities.checked_add(1).ok_or_else(|| self.error(DiagnosticKind::Limit, "entity limit"))?;
        if self.entities > self.limits.max_entities {
            Err(self.error(DiagnosticKind::Limit,
            "entity limit"))
        } else {
            Ok(())
        }
    }
    pub(super) fn line(&mut self) -> Result<(), Diagnostic> {
        if self.tokens[self.pos].text == "\n" && !self.tokens[self.pos].quoted {
            self.pos += 1;
            Ok(())
        }
        else if self.is("<eof>") {
            Ok(())
        } else {
            Err(self.error(DiagnosticKind::Syntax, "expected end of line"))
        }
    }
    pub(crate) fn decimal(&mut self) -> Result<String, Diagnostic> {
        let token = self.next()?;
        if token.quoted {
            return Err(self.at(&token, DiagnosticKind::Literal, "expected decimal integer"));
        }
        decimal(&token.text).ok_or_else(|| self.at(&token, DiagnosticKind::Literal, "invalid decimal integer"))
    }
    fn name(&mut self, prefix: &str) -> Result<Token, Diagnostic> {
        let mut token = self.next()?;
        if prefix == "^" && token.text == "^" {
            let name = self.next()?;
            token.text.push_str(&name.text);
            token.span.end = name.span.end;
            token.quoted |= name.quoted;
        }
        let rest = token.text.strip_prefix(prefix);
        let valid = rest.is_some_and(|name| {
            if name.is_empty() && prefix != "S" {
                return false;
            }
            name.chars().all(|c| {
                c.is_alphanumeric() || c == '_'
                    || prefix.is_empty() && "./:-".contains(c)
                    || prefix == "@" && ".#:".contains(c)
            })
        });
        if token.quoted || !valid {
            return Err(self.at(&token, DiagnosticKind::Syntax, "invalid entity name"));
        }
        Ok(token)
    }

    pub(crate) fn reference(&mut self, ns: &'static str, prefix: &str, local: bool) -> Result<u32, Diagnostic> {
        let token = self.name(prefix)?;
        if ns == "file" && !token.text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(self.at(&token, DiagnosticKind::Syntax, "expected numeric file id"));
        }
        let scope = if local { self.scope } else { 0 };
        let found = self.names.get(&(scope, ns, token.text.clone())).copied();
        if self.first {
            return Ok(found.unwrap_or(0));
        }
        found.ok_or_else(|| self.at(&token, DiagnosticKind::UnknownName, format!("unknown {ns} {}", token.text)))
    }

    pub(super) fn definition(&mut self, ns: &'static str, prefix: &str, local: bool) -> Result<u32, Diagnostic> {
        self.entity()?;
        let token = self.name(prefix)?;
        if ns == "file" && !token.text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(self.at(&token, DiagnosticKind::Syntax, "expected numeric file id"));
        }
        let scope = if local {
            self.scope
        } else {
            0
        };
        let count = self.counts.entry((scope, ns)).or_default();
        let id = *count;
        *count = count.checked_add(1).ok_or_else(|| Diagnostic::new(DiagnosticKind::Limit, token.span,
        "name count exceeds u32"))?;
        if self.first && self.names.insert((scope, ns, token.text.clone()),
            id).is_some() { return Err(self.at(&token, DiagnosticKind::DuplicateName, format!("duplicate {ns} {}",
                token.text)));
        }
        Ok(id)
    }
    fn module(&mut self) -> Result<Module, Diagnostic> {
        self.expect("module")?;
        let name = self.word()?;
        let version = if !self.end_of_line() && self.peek().text.starts_with('v') {
            let token = self.next()?;
            let version = token.text[1..].to_owned();
            if version.split('.').count() != 3
                || !version.split('.').all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            {
                return Err(self.at(&token, DiagnosticKind::Syntax, "expected vX.Y.Z"));
            }
            Some(version)
        } else {
            None
        };
        self.line()?;
        let mut module = Module {
            name, version, profile: self.profile, requires: vec![], imports: vec![], entry: None, tests: None,
            declarations: Arena::new(), types: TypeTable::new(), scopes: Arena::new(), summaries: Arena::new(),
            protocols: Arena::new(), functions: Arena::new(), files: Arena::new(),
        };
        while !self.is("<eof>") {
            let directive = self.next()?;
            if directive.quoted {
                return Err(self.at(&directive, DiagnosticKind::Syntax, "expected module directive"));
            }
            match directive.text.as_str() {
                "requires" => {
                    loop {
                        self.entity()?;
                        module.requires.push(Feature::read(self)?);
                        if !self.eat(",") {
                            break;
                        }
                    }
                    self.line()?;
                }
                "import" => {
                    module.imports.push(self.import()?);
                    self.line()?;
                }
                "entry" => {
                    module.entry = Some(FuncId::read(self)?);
                    self.line()?;
                }
                "test" => {
                    let tests = module.tests.get_or_insert_with(Vec::new);
                    if self.eat("module") {
                        self.line()?;
                    } else {
                        let name = String::read(self)?;
                        self.expect("=")?;
                        let function = FuncId::read(self)?;
                        tests.push(TestDecl { name, function });
                        self.line()?;
                    }
                }
                "file" => {
                    self.definition("file", "", false)?;
                    let path = String::read(self)?;
                    module.files.alloc(SourceFileDecl { path }).map_err(|e| self.arena_error(e))?;
                    self.line()?;
                }
                "scope" => {
                    let id = self.definition("scope", "S", false)?;
                    let kind = ScopeKind::read(self)?;
                    self.expect("in")?;
                    let function = FuncId::read(self)?;
                    if self.first {
                        let symbol = self.tokens[self.pos - 1].text.clone();
                        let key = (0, "function-scope", symbol);
                        if kind == ScopeKind::Action {
                            self.names.insert(key, id);
                        }
                    }
                    let parent = if self.eat("parent") {
                        Some(ScopeId::read(self)?)
                    } else {
                        None
                    };
                    let loc = self.location()?;
                    module.scopes.alloc(ScopeDecl { function, parent, kind, loc })
                        .map_err(|e| self.arena_error(e))?;
                    self.line()?;
                }
                "summary" => {
                    self.definition("summary", "M", false)?;
                    let item = self.summary()?;
                    module.summaries.alloc(item).map_err(|e| self.arena_error(e))?;
                    self.line()?;
                }
                "protocol" => {
                    self.definition("protocol", "", false)?;
                    let item = self.protocol()?;
                    module.protocols.alloc(item).map_err(|e| self.arena_error(e))?;
                    self.line()?;
                }
                "func" | "extern" => {
                    let id = self.definition("func", "@", false)?;
                    let symbol = self.tokens[self.pos - 1].text[1..].to_owned();
                    let function = self.function(id, symbol, directive.text == "extern")?;
                    module.functions.alloc(function).map_err(|e| self.arena_error(e))?;
                }
                "object" | "role" | "enum" | "variant" | "native" | "cap" | "global" => {
                    let item = self.declaration(&directive.text)?;
                    module.declarations.alloc(item).map_err(|e| self.arena_error(e))?;
                }
                _ => return Err(self.at(&directive, DiagnosticKind::Syntax, "unknown module directive")),
            }
        }
        module.types = std::mem::take(&mut self.types);
        Ok(module)
    }

    fn function(&mut self, id: u32, symbol: String, external: bool) -> Result<Function, Diagnostic> {
        self.scope = id as usize + 1;
        self.generic_owner = Some(GenericOwner::Function(FuncId::from_u32(id)));
        self.values = Arena::new();
        self.insts = Arena::new();
        self.selections = Arena::new();
        self.deps = Arena::new();
        self.implicit.clear();
        let type_params = self.type_parameters()?;
        let params = self.values(None, BlockId::from_u32(0), true)?;
        let param_types = params.iter().map(|v| self.values.get(*v).map(|v| v.ty))
            .collect::<Result<Vec<_>, _>>().map_err(|e| self.arena_error(e))?;
        self.expect("->")?;
        let results = self.results()?;
        let attrs = if self.is("[") {
            self.attributes()?
        } else {
            vec![]
        };
        let ownership_abi = if self.eat("abi") {
            OwnershipAbi::read(self)?
        } else {
            OwnershipAbi::Unelaborated
        };
        let summary = self.summary_reference("summary")?;
        if self.first && summary.is_some() {
            let name = self.tokens[self.pos - 2].text.clone();
            self.names.insert((id as usize, "function-summary-name", name), 0);
        }
        let async_creation_summary = self.summary_reference("async-creation")?;
        let async_body_summary = self.summary_reference("async-body")?;
        let loc = self.location()?;
        let mut attrs = attrs;
        let body = if external {
            self.expect("=")?;
            attrs.push(FunctionAttr::Extern(String::read(self)?));
            None
        } else if self.eat(";") {
            None
        } else {
            self.expect("{")?;
            self.line()?;
            self.default_scope = self.names.get(&(0, "function-scope", format!("@{symbol}")))
                .copied().map(ScopeId::from_u32);
            Some(self.body(params)?)
        };
        self.scope = 0;
        self.generic_owner = None;
        self.line()?;
        Ok(Function {
            symbol, signature: Signature { params: param_types, results, ownership_abi }, attrs, type_params,
            summary, async_creation_summary, async_body_summary, body, loc,
        })
    }

    fn body(&mut self, entry_params: Vec<ValueId>) -> Result<Body, Diagnostic> {
        let explicit_entry = if self.eat("entry") {
            let entry = BlockId::read(self)?;
            self.line()?;
            Some(entry)
        } else {
            None
        };
        let entry = explicit_entry.unwrap_or(BlockId::from_u32(0));
        let mut body = Body {
            entry, values: Arena::new(), blocks: Arena::new(), insts: Arena::new(), bindings: Arena::new(),
            boundaries: Arena::new(), deps: Arena::new(), keys: Arena::new(), selections: Arena::new(),
        };
        while !self.eat("}") {
            macro_rules! local {
                ($ns:literal, $prefix:literal, $arena:ident, $ty:ty) => {
                    {
                        self.expect($ns)?;
                        self.definition($ns, $prefix, true)?;
                        self.expect(":")?;
                        self.expect($ns)?;
                        self.expect("=")?;
                        let item = <$ty>::read(self)?;
                        body.$arena.alloc(item).map_err(|e| self.arena_error(e))?;
                        self.line()?;
                    }
                };
            }
            if self.is("binding") {
                let item = self.binding()?;
                body.bindings.alloc(item).map_err(|e| self.arena_error(e))?;
            } else if self.is("boundary") {
                let item = self.boundary()?;
                body.boundaries.alloc(item).map_err(|e| self.arena_error(e))?;
            } else if self.is("key") {
                local!("key", "k", keys, KeyExpr);
            } else if self.is("dependency") {
                self.expect("dependency")?;
                self.definition("dependency", "d", true)?;
                self.expect(":")?;
                self.expect("dependency")?;
                self.expect("=")?;
                let item = DepExpr::read(self)?;
                self.deps.alloc(item).map_err(|e| self.arena_error(e))?;
                self.line()?;
            } else if self.is("selection") {
                self.expect("selection")?;
                self.definition("selection", "s", true)?;
                self.expect(":")?;
                self.expect("selection")?;
                self.expect("=")?;
                let id = SelectionId::from_u32(self.selections.len() as u32);
                self.selections.alloc(SelectionExpr::Union(vec![])).map_err(|e| self.arena_error(e))?;
                let item = SelectionExpr::read(self)?;
                *self.selections.get_mut(id).map_err(|e| Diagnostic::new(
                    DiagnosticKind::Structure, Span { start: 0, end: 0 }, e.to_string()
                ))? = item;
                self.line()?;
            } else {
                let id = BlockId::from_u32(self.definition("block", "^", true)?);
                self.current_block = id;
                let params = if id == entry && !entry_params.is_empty() {
                    for (index, value) in entry_params.iter().enumerate() {
                        self.values.get_mut(*value).map_err(|e| Diagnostic::new(
                            DiagnosticKind::Structure, Span { start: 0, end: 0 }, e.to_string()
                        ))?.def = Definition::BlockParam { block: id, index: index as u32 };
                    }
                    entry_params.clone()
                } else if self.is("(") {
                    self.values(None, id, true)?
                } else {
                    vec![]
                };
                let lexical_scope = if self.eat("in") {
                    ScopeId::read(self)?
                } else {
                    self.default_scope.or_else(|| self.first.then_some(ScopeId::from_u32(u32::MAX)))
                        .ok_or_else(|| self.error(DiagnosticKind::Syntax, "block requires in Sx when the function has no action scope"))?
                };
                let unwind = if self.eat("unwind") {
                    Some(BlockUnwind { target: Target::read(self)? })
                } else {
                    None
                };
                self.expect(":")?;
                self.line()?;
                let mut insts = vec![];
                while !self.is_terminator() {
                    let index = self.insts.len() as u32;
                    let inst = InstId::from_u32(index);
                    if self.eat("[") {
                        self.definition("inst", "i", true)?;
                        self.expect("]")?;
                    } else {
                        self.anonymous("inst")?;
                    }
                    self.insts.alloc(Instruction {
                        results: vec![], op: Operation::Ownership(OwnershipOp::Empty {}),
                        facts: InstructionFacts::default(), loc: None,
                    }).map_err(|e| self.arena_error(e))?;
                    let results = if self.peek().text.starts_with('%') || self.is("(") {
                        let values = self.values(Some(inst), id, self.is("("))?;
                        self.expect("=")?;
                        values
                    } else {
                        vec![]
                    };
                    self.literal_type = results.first().and_then(|v| self.values.get(*v).ok()).map(|v| v.ty);
                    let op = Operation::read(self)?;
                    let facts = if self.is("facts") {
                        self.facts()?
                    } else {
                        InstructionFacts::default()
                    };
                    let loc = self.location()?;
                    self.line()?;
                    self.insts.get_mut(inst).map_err(|e| Diagnostic::new(
                        DiagnosticKind::Structure, Span { start: 0, end: 0 }, e.to_string()
                    ))?.clone_from(&Instruction { results, op, facts, loc });
                    Vec::append(&mut insts, &mut self.implicit);
                    insts.push(inst);
                }
                let term = self.terminator()?;
                Vec::append(&mut insts, &mut self.implicit);
                self.line()?;
                body.blocks.alloc(Block { lexical_scope, params, unwind, insts, term })
                    .map_err(|e| self.arena_error(e))?;
            }
        }
        body.values = std::mem::take(&mut self.values);
        body.insts = std::mem::take(&mut self.insts);
        body.selections = std::mem::take(&mut self.selections);
        body.deps = std::mem::take(&mut self.deps);
        Ok(body)
    }

    fn values(&mut self, inst: Option<InstId>, block: BlockId, parens: bool)
    -> Result<Vec<ValueId>, Diagnostic>
    {
        if parens {
            self.expect("(")?;
        }
        let mut values = vec![];
        while !parens || !self.is(")") {
            let value = ValueId::from_u32(self.definition("value", "%", true)?);
            self.expect(":")?;
            if self.first {
                // Retain nominal spelling for fields on forward SSA uses. Type
                // annotations do not imply dominance or ownership validity.
                self.annotations.insert((self.scope, value), self.peek().text.clone());
            }
            let ty = TypeId::read(self)?;
            let facts = if self.eat("facts") {
                self.expect("(")?;
                let facts = ValueFacts::read(self)?;
                self.expect(")")?;
                facts
            } else {
                value_default(self.profile, self.types.get(ty).ok())
            };
            let def = if let Some(inst) = inst {
                Definition::InstResult { inst, index: u16::try_from(values.len())
                        .map_err(|_| self.error(DiagnosticKind::Limit, "too many results"))? }
            } else {
                Definition::BlockParam { block, index: u32::try_from(values.len())
                        .map_err(|_| self.error(DiagnosticKind::Limit, "too many parameters"))? }
            };
            self.values.alloc(ValueData { ty, def, facts }).map_err(|e| self.arena_error(e))?;
            values.push(value);
            if !parens {
                break;
            }
            if !self.eat(";") {
                self.comma()?;
            }
        }
        if parens {
            self.expect(")")?;
        }
        Ok(values)
    }

    fn facts(&mut self) -> Result<InstructionFacts, Diagnostic> {
        self.expect("facts")?;
        self.expect("(")?;
        let mut facts = InstructionFacts::default();
        while !self.is(")") {
            let name = self.next()?;
            self.expect("=")?;
            match name.text.as_str() {
                "result_deps" => facts.result_deps = Vec::read(self)?,
                "retention" => facts.retention = Vec::read(self)?,
                "provenance" => facts.provenance = Vec::read(self)?,
                _ => return Err(self.at(&name, DiagnosticKind::Syntax, "unknown facts component")),
            }
            self.comma()?;
        }
        self.expect(")")?;
        Ok(facts)
    }

    fn is_terminator(&self) -> bool {
        matches!(self.peek().text.as_str(), "br" | "cond" | "switch" | "invoke.own" | "ret"
            | "throw" | "raise" | "panic" | "trap" | "catch" | "array.lookup" | "map.lookup"
            | "unreachable" | "}" | "<eof>")
    }

    fn invoke_edge(&mut self, outcome: Outcome) -> Result<InvokeEdge, Diagnostic> {
        let block = BlockId::read(self)?;
        self.expect("(")?;
        let mut args = vec![];
        while !self.is(")") {
            self.entity()?;
            if self.peek().text.starts_with('$') && !self.peek().quoted {
                let token = self.next()?;
                self.pending.push(PendingSlot { scope: self.scope, block: self.current_block, outcome,
                    index: args.len(), token });
                args.push(EdgeArg::Produced(EdgeResult { outcome, slot: 0 }));
            } else {
                args.push(EdgeArg::Value(ValueId::read(self)?));
            }
            self.comma()?;
        }
        self.expect(")")?;
        Ok(InvokeEdge { block, args })
    }
    fn terminator(&mut self) -> Result<Terminator, Diagnostic> {
        let tag = self.next()?;
        let kind = match tag.text.as_str() {
            "br" if !tag.quoted => TermKind::Br(Target::read(self)?),
            "cond" if !tag.quoted => {
                let condition = ValueId::read(self)?;
                self.expect(",")?;
                let yes = Target::read(self)?;
                self.expect(",")?;
                let no = Target::read(self)?;
                TermKind::Cond { condition, yes, no }
            }
            "switch" if !tag.quoted => {
                let value = ValueId::read(self)?;
                self.expect(",")?;
                let cases = Vec::<SwitchCase>::read(self)?;
                self.expect(",")?;
                self.expect("else")?;
                let default = Target::read(self)?;
                TermKind::Switch { value, cases, default }
            }
            "invoke.own" if !tag.quoted => {
                let op = Box::new(Operation::read(self)?);
                self.line()?;
                self.expect("normal")?;
                let normal = self.invoke_edge(Outcome::Normal)?;
                self.line()?;
                self.expect("unwind")?;
                let unwind = self.invoke_edge(Outcome::Unwind)?;
                TermKind::InvokeOwn { op, normal, unwind }
            }
            "ret" if !tag.quoted => {
                let mut values = vec![];
                if !self.end_of_line() && !self.is("!") {
                    loop {
                        values.push(ValueId::read(self)?);
                        if !self.eat(",") {
                            break;
                        }
                    }
                }
                TermKind::Ret { values }
            },
            "throw" if !tag.quoted => TermKind::Throw(ValueId::read(self)?),
            "raise" if !tag.quoted => TermKind::Raise(ValueId::read(self)?),
            "panic" if !tag.quoted => TermKind::Panic(ValueId::read(self)?),
            "trap" if !tag.quoted => {
                let code = TrapCode::read(self)?;
                let message = String::read(self)?;
                TermKind::Trap { code, message }
            }
            "catch" if !tag.quoted => {
                let failure = ValueId::read(self)?;
                self.expect("is")?;
                let ty = TypeId::read(self)?;
                self.expect(",")?;
                let handler = self.invoke_edge(Outcome::Normal)?;
                self.expect(",")?;
                let next = Target::read(self)?;
                TermKind::Catch { failure, ty, handler, next }
            }
            "array.lookup" | "map.lookup" if !tag.quoted => {
                let kind = if tag.text == "array.lookup" { LookupKind::Array } else { LookupKind::Map };
                let collection = ValueId::read(self)?;
                self.expect(",")?;
                let key = ValueId::read(self)?;
                self.expect(",")?;
                let found = self.invoke_edge(Outcome::Normal)?;
                self.expect(",")?;
                let missing = Target::read(self)?;
                TermKind::Lookup { kind, collection, key, found, missing }
            }
            "unreachable" if !tag.quoted => TermKind::Unreachable,
            _ => return Err(self.at(&tag, DiagnosticKind::Syntax, "expected terminator")),
        };
        let loc = if self.is("!") {
            Some(DebugLoc::read(self)?)
        } else {
            None
        };
        Ok(Terminator { kind, loc })
    }
    fn resolve_slots(&self, module: &mut Module) -> Result<(), Diagnostic> {
        for pending in &self.pending {
            let function = FuncId::from_u32((pending.scope - 1) as u32);
            let body = module.functions.get(function).ok().and_then(|f| f.body.as_ref())
                .ok_or_else(|| self.at(&pending.token, DiagnosticKind::Structure, "missing invoke body"))?;
            let term = &body.blocks.get(pending.block).map_err(|e| self.at(&pending.token, DiagnosticKind::Structure,
                e.to_string()))?.term.kind;
            let names = edge_slots(module, body, term, pending.outcome).map_err(|e| self.at(&pending.token,
                DiagnosticKind::OutcomeSlot, e.message))?;
            let slot = names.iter().position(|name| format!("${}", slot_name(*name)) == pending.token.text)
                .and_then(|i| u16::try_from(i).ok()).ok_or_else(|| self.at(&pending.token,
                    DiagnosticKind::OutcomeSlot, "slot is not in this operation's outcome schema"))?;
            let body = module.functions.get_mut(function).map_err(|e| self.at(&pending.token,
                DiagnosticKind::Structure, e.to_string()))?.body.as_mut()
                .ok_or_else(|| self.at(&pending.token, DiagnosticKind::Structure, "missing body"))?;
            let term = &mut body.blocks.get_mut(pending.block).map_err(|e| self.at(&pending.token,
                DiagnosticKind::Structure, e.to_string()))?.term.kind;
            let edge = match term {
                TermKind::InvokeOwn { normal, unwind,
                        .. } => if pending.outcome == Outcome::Normal {
                    normal
                } else {
                    unwind
                },
                TermKind::Catch { handler, .. } => handler, TermKind::Lookup {
                    found, ..
                } => found,
                _ => return Err(self.at(&pending.token, DiagnosticKind::Structure, "missing invoke edge"))
            };
            let arg = edge.args.get_mut(pending.index).ok_or_else(|| self.at(&pending.token,
                DiagnosticKind::Structure, "missing edge argument"))?;
            *arg = EdgeArg::Produced(EdgeResult { outcome: pending.outcome, slot });
        }
        Ok(())
    }
}

impl Codec for Target {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        let block = BlockId::read(r)?;
        if !r.eat("(") {
            return Ok(Self { block, args: vec![] });
        }
        let mut args = vec![];
        while !r.is(")") {
            r.entity()?;
            args.push(ValueId::read(r)?);
            r.comma()?;
        }
        r.expect(")")?;
        Ok(Self { block, args })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        self.block.write(w)?;
        if self.args.is_empty() {
            return Ok(());
        }
        w.push("(");
        for (i, value) in self.args.iter().enumerate() {
            if i > 0 {
                w.push(", ");
            }
            value.write(w)?;
        }
        w.push(")");
        Ok(())
    }
}

pub(crate) fn edge_slots(module: &Module, body: &Body, term: &TermKind, outcome: Outcome) -> Result<Vec<SlotName>, PrintError> {
    match term {
        TermKind::InvokeOwn { op, .. } => {
            let contract = ContractRegistry::new(module).text_schema(op, body).map_err(|e| PrintError::new(e.to_string()))?;
            let outcome = contract.outcome(outcome)
                .ok_or_else(|| PrintError::new("operation has no requested outcome"))?;
            Ok(outcome.results.iter().map(|s| s.name).collect())
        }
        TermKind::Catch { .. } | TermKind::Lookup { .. } if outcome == Outcome::Normal => Ok(vec![SlotName::Value]),
        _ => Err(PrintError::new("edge result outside an outcome schema")),
    }
}

/// Iterative graph walk: aliases cannot bypass the nesting bound or overflow
/// the stack, including forward references and cyclic type graphs.
pub(crate) fn check_type_depth(module: &Module, limit: usize) -> Result<(), String> {
    let mut depths = vec![0usize;
        module.types.len()];
    let mut active = vec![false;
        module.types.len()];
    for (root, _) in module.types.iter() {
        let mut stack = vec![(root, false)];
        while let Some((id, exiting)) = stack.pop() {
            let i = id.as_u32() as usize;
            let ty = module.types.get(id).map_err(|e| e.to_string())?;
            let children = type_children(ty);
            if exiting {
                let depth = children.iter().map(|child
                    | depths[child.as_u32() as usize]).max().unwrap_or(0).saturating_add(1);
                if depth > limit {
                    return Err("type nesting limit".into());
                }
                depths[i] = depth;
                active[i] = false;
            } else if depths[i] == 0 {
                if active[i] {
                    return Err("cyclic type aliases".into());
                }
                active[i] = true;
                stack.push((id, true));
                for child in children.into_iter().rev() {
                    module.types.get(child).map_err(|e| e.to_string())?;
                    stack.push((child, false));
                }
            }
        }
    }
    Ok(())
}
pub(crate) fn type_children(ty: &Type) -> Vec<TypeId> {
    match ty {
        Type::Object { args, .. } | Type::Role { args, .. } | Type::Cap { args, .. } => args.clone(),
        Type::Successor(t) | Type::Array(t) | Type::Builder(t) | Type::LowCell(t) => vec![*t],
        Type::Map(a,b) | Type::MapBuilder(a,b) => vec![*a,*b],
        Type::Fn(f) => f.params.iter().copied().chain(f.result).collect(),
        Type::Future(t) => t.iter().copied().collect(),
        Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char | Type::Str | Type::Range
            | Type::Any | Type::Never |
        Type::Enum(_) | Type::Variant(_) | Type::Handle(_) | Type::Parameter(_) | Type::Descriptor | Type::Frame(_)
            | Type::Service(_) => vec![],
    }
}

#[cfg(test)]
pub(super) fn payload_read<T: Codec>(input: &str, nominal_kind: u32) -> T {
    let tokens = lex(input, 0, Limits::default()).unwrap();
    let mut names = Names::new();
    for &(ns, name) in TEST_NAMES {
        names.insert((0, ns, name.into()), 0);
    }
    names.insert((0, "decl-kind", "D0".into()), nominal_kind);
    let mut annotations = HashMap::new();
    let mut reader = Reader::new(&tokens, &mut names, &mut annotations, false, Limits::default(), Profile::High);
    reader.types.intern(Type::Int).unwrap();
    reader.selections.alloc(SelectionExpr::Value(ValueId::from_u32(0))).unwrap();
    let value = T::read(&mut reader).unwrap_or_else(|e| panic!("{e} in {input}"));
    assert!(reader.is("<eof>"));
    value
}
