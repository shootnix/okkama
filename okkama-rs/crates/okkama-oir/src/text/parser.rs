use std::collections::HashMap;
use super::{lexer::{lex, Token}, *};

type Names = HashMap<(usize, &'static str, String), u32>;

pub fn parse(input: &str) -> Result<Module, Diagnostic> { parse_with_limits(input, Limits::default()) }

pub fn parse_with_limits(input: &str, limits: Limits) -> Result<Module, Diagnostic> {
    // Header validation deliberately precedes lexing or limits on the body.
    let (profile, offset) = header(input)?;
    if input.len() > limits.max_text_bytes { return Err(Diagnostic::new(DiagnosticKind::Limit, Span { start: 0, end: input.len() }, "text size limit")); }
    let tokens = lex(input, offset, limits)?;
    let mut names = Names::new();
    let mut first = Reader::new(&tokens, &mut names, true, limits, profile);
    first.module()?;
    let mut second = Reader::new(&tokens, &mut names, false, limits, profile);
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
    if words.first() != Some(&"oir") { return Err(Diagnostic::new(DiagnosticKind::Header, span, "expected oir 2 <profile>")); }
    if words.get(1) != Some(&"2") { return Err(Diagnostic::new(DiagnosticKind::UnsupportedVersion, span, format!("unsupported OIR version {}; expected 2 (no v1 reader)", words.get(1).copied().unwrap_or("<missing>")))); }
    let Some(profile) = words.get(2) else { return Err(Diagnostic::new(DiagnosticKind::MissingProfile, span, "oir 2 requires high, own, coro or low")); };
    let profile = match *profile { "high" => Profile::High, "own" => Profile::Own, "coro" => Profile::Coro, "low" => Profile::Low,
        _ => return Err(Diagnostic::new(DiagnosticKind::UnknownProfile, span, format!("unknown OIR-2 profile {profile}"))) };
    if words.len() != 3 { return Err(Diagnostic::new(DiagnosticKind::Header, span, "unexpected header tokens")); }
    Ok((profile, if end < input.len() { end + 1 } else { end }))
}

struct PendingSlot { scope: usize, block: BlockId, outcome: Outcome, index: usize, token: Token }

pub(crate) struct Reader<'a> {
    tokens: &'a [Token], pos: usize, names: &'a mut Names,
    counts: HashMap<(usize, &'static str), u32>, first: bool,
    pub(crate) scope: usize, limits: Limits, entities: usize, profile: Profile,
    pending: Vec<PendingSlot>, current_block: BlockId,
}
impl<'a> Reader<'a> {
    fn new(tokens: &'a [Token], names: &'a mut Names, first: bool, limits: Limits, profile: Profile) -> Self {
        Self { tokens, pos: 0, names, counts: HashMap::new(), first, scope: 0, limits, entities: 0,
            profile, pending: vec![], current_block: BlockId::from_u32(0) }
    }
    pub(crate) fn peek(&self) -> &Token {
        let mut p = self.pos;
        while p + 1 < self.tokens.len() && self.tokens[p].text == "\n" && !self.tokens[p].quoted { p += 1; }
        &self.tokens[p.min(self.tokens.len() - 1)]
    }
    pub(crate) fn next(&mut self) -> Result<Token, Diagnostic> {
        while self.pos + 1 < self.tokens.len() && self.tokens[self.pos].text == "\n" && !self.tokens[self.pos].quoted { self.pos += 1; }
        let token = self.tokens[self.pos].clone();
        if token.text == "<eof>" && !token.quoted { return Err(self.at(&token, DiagnosticKind::Syntax, "unexpected end of text")); }
        self.pos += 1; Ok(token)
    }
    pub(crate) fn is(&self, text: &str) -> bool { !self.peek().quoted && self.peek().text == text }
    pub(crate) fn eat(&mut self, text: &str) -> bool { if self.is(text) { let _ = self.next(); true } else { false } }
    pub(crate) fn expect(&mut self, text: &str) -> Result<(), Diagnostic> {
        if self.eat(text) { Ok(()) } else { Err(self.error(DiagnosticKind::Syntax, format!("expected {text}"))) }
    }
    pub(crate) fn at(&self, token: &Token, kind: DiagnosticKind, message: impl Into<String>) -> Diagnostic { Diagnostic::new(kind, token.span, message) }
    pub(crate) fn error(&self, kind: DiagnosticKind, message: impl Into<String>) -> Diagnostic { self.at(self.peek(), kind, message) }
    pub(crate) fn field(&mut self, name: &str) -> Result<(), Diagnostic> { self.expect(name)?; self.expect("=") }
    pub(crate) fn comma(&mut self) -> Result<(), Diagnostic> {
        if self.eat(",") || self.is(")") || self.is("]") { Ok(()) } else { Err(self.error(DiagnosticKind::Syntax, "expected comma")) }
    }
    pub(crate) fn entity(&mut self) -> Result<(), Diagnostic> {
        self.entities = self.entities.checked_add(1).ok_or_else(|| self.error(DiagnosticKind::Limit, "entity limit"))?;
        if self.entities > self.limits.max_entities { Err(self.error(DiagnosticKind::Limit, "entity limit")) } else { Ok(()) }
    }
    fn line(&mut self) -> Result<(), Diagnostic> {
        if self.tokens[self.pos].text == "\n" && !self.tokens[self.pos].quoted { self.pos += 1; Ok(()) }
        else if self.is("<eof>") { Ok(()) } else { Err(self.error(DiagnosticKind::Syntax, "expected end of line")) }
    }
    pub(crate) fn decimal(&mut self) -> Result<String, Diagnostic> {
        let token = self.next()?;
        if token.quoted { return Err(self.at(&token, DiagnosticKind::Literal, "expected decimal integer")); }
        decimal(&token.text).ok_or_else(|| self.at(&token, DiagnosticKind::Literal, "invalid decimal integer"))
    }
    fn name(&mut self, prefix: &str) -> Result<Token, Diagnostic> {
        let token = self.next()?;
        let rest = token.text.strip_prefix(prefix);
        let valid = rest.is_some_and(|s| !s.is_empty() && if prefix.is_empty() { s.bytes().all(|b| b.is_ascii_digit()) }
            else { s.chars().all(|c| c.is_alphanumeric() || c == '_' || prefix == "@" && ".#:".contains(c)) });
        if token.quoted || !valid { return Err(self.at(&token, DiagnosticKind::Syntax, "invalid entity name")); }
        Ok(token)
    }
    pub(crate) fn reference(&mut self, ns: &'static str, prefix: &str, local: bool) -> Result<u32, Diagnostic> {
        let token = self.name(prefix)?;
        if self.first { return Ok(0); }
        self.names.get(&(if local { self.scope } else { 0 }, ns, token.text.clone())).copied()
            .ok_or_else(|| self.at(&token, DiagnosticKind::UnknownName, format!("unknown {ns} {}", token.text)))
    }
    fn definition(&mut self, ns: &'static str, prefix: &str, local: bool) -> Result<u32, Diagnostic> {
        self.entity()?;
        let token = self.name(prefix)?;
        let scope = if local { self.scope } else { 0 };
        let count = self.counts.entry((scope, ns)).or_default();
        let id = *count; *count = count.checked_add(1).ok_or_else(|| Diagnostic::new(DiagnosticKind::Limit, token.span, "name count exceeds u32"))?;
        if self.first && self.names.insert((scope, ns, token.text.clone()), id).is_some() { return Err(self.at(&token, DiagnosticKind::DuplicateName, format!("duplicate {ns} {}", token.text))); }
        Ok(id)
    }
    fn module(&mut self) -> Result<Module, Diagnostic> {
        self.expect("module")?; let name = String::read(self)?;
        self.expect("version")?; let version = Option::<String>::read(self)?; self.line()?;
        self.expect("requires")?; let requires = Vec::read(self)?; self.line()?;
        self.expect("imports")?; let imports = Vec::read(self)?; self.line()?;
        self.expect("entry")?; let entry = Option::read(self)?; self.line()?;
        self.expect("tests")?; let tests = Option::read(self)?; self.line()?;
        let mut module = Module { name, version, profile: self.profile, requires, imports, entry, tests,
            declarations: Arena::new(), types: TypeTable::new(), scopes: Arena::new(), summaries: Arena::new(), protocols: Arena::new(), functions: Arena::new(), files: Arena::new() };
        while !self.is("<eof>") {
            let directive = self.next()?;
            macro_rules! declaration { ($ns:literal, $prefix:literal, $arena:ident, $ty:ty) => {{
                self.definition($ns, $prefix, false)?; self.expect("=")?;
                let item = <$ty>::read(self)?; module.$arena.alloc(item).map_err(|e| self.error(DiagnosticKind::Limit, e.to_string()))?; self.line()?;
            }}; }
            match directive.text.as_str() {
                "file" if !directive.quoted => declaration!("file", "", files, SourceFileDecl),
                "type" if !directive.quoted => {
                    let id = self.definition("type", "t", false)?; self.expect("=")?; let ty = Type::read(self)?;
                    if !self.first { let actual = module.types.intern(ty).map_err(|e| self.error(DiagnosticKind::Limit, e.to_string()))?;
                        if actual.as_u32() != id { return Err(self.error(DiagnosticKind::DuplicateName, "duplicate interned type declaration")); } }
                    self.line()?;
                }
                "decl" if !directive.quoted => declaration!("decl", "D", declarations, Declaration),
                "scope" if !directive.quoted => declaration!("scope", "S", scopes, ScopeDecl),
                "summary" if !directive.quoted => declaration!("summary", "M", summaries, Summary),
                "protocol" if !directive.quoted => declaration!("protocol", "P", protocols, ProtocolDecl),
                "func" if !directive.quoted => {
                    let id = self.definition("func", "@", false)?;
                    let symbol = self.tokens[self.pos - 1].text[1..].to_owned();
                    let function = self.function(id, symbol)?;
                    module.functions.alloc(function).map_err(|e| self.error(DiagnosticKind::Limit, e.to_string()))?;
                }
                _ => return Err(self.at(&directive, DiagnosticKind::Syntax, "unknown module directive")),
            }
        }
        Ok(module)
    }
    fn function(&mut self, id: u32, symbol: String) -> Result<Function, Diagnostic> {
        let signature = Signature::read(self)?;
        self.field("attrs")?; let attrs = Vec::read(self)?;
        self.field("type_params")?; let type_params = Vec::read(self)?;
        self.field("summary")?; let summary = Option::read(self)?;
        self.field("async_creation")?; let async_creation_summary = Option::read(self)?;
        self.field("async_body")?; let async_body_summary = Option::read(self)?;
        let loc = if self.is("!") { Some(DebugLoc::read(self)?) } else { None };
        self.scope = id as usize + 1;
        let body = if self.eat(";") { None } else { self.expect("{")?; self.line()?; Some(self.body()?) };
        self.scope = 0; self.line()?;
        Ok(Function { symbol, signature, attrs, type_params, summary, async_creation_summary, async_body_summary, body, loc })
    }
    fn body(&mut self) -> Result<Body, Diagnostic> {
        self.expect("entry")?; let entry = BlockId::read(self)?; self.line()?;
        let mut body = Body { entry, values: Arena::new(), blocks: Arena::new(), insts: Arena::new(), bindings: Arena::new(), boundaries: Arena::new(), deps: Arena::new(), keys: Arena::new(), selections: Arena::new() };
        while !self.eat("}") {
            macro_rules! declaration { ($ns:literal, $prefix:literal, $arena:ident, $ty:ty) => {{
                self.expect($ns)?; self.definition($ns, $prefix, true)?; self.expect(":")?; self.expect($ns)?; self.expect("=")?;
                let item = <$ty>::read(self)?; body.$arena.alloc(item).map_err(|e| self.error(DiagnosticKind::Limit, e.to_string()))?; self.line()?;
            }}; }
            if self.is("binding") { declaration!("binding", "b", bindings, BindingDecl); }
            else if self.is("boundary") { declaration!("boundary", "B", boundaries, Boundary); }
            else if self.is("key") { declaration!("key", "k", keys, KeyExpr); }
            else if self.is("dependency") { declaration!("dependency", "d", deps, DepExpr); }
            else if self.is("selection") { declaration!("selection", "s", selections, SelectionExpr); }
            else {
                let id = self.definition("block", "^", true)?;
                self.current_block = BlockId::from_u32(id);
                let params = self.values(&mut body, None, BlockId::from_u32(id))?;
                self.expect("scope")?; let lexical_scope = ScopeId::read(self)?;
                let unwind = if self.eat("unwind") { Some(BlockUnwind { target: Target::read(self)? }) } else { None };
                self.expect(":")?; self.line()?;
                let mut insts = vec![];
                while self.eat("[") {
                    let inst = InstId::from_u32(self.definition("inst", "i", true)?); self.expect("]")?;
                    let results = self.values(&mut body, Some(inst), BlockId::from_u32(id))?; self.expect("=")?;
                    let op = Operation::read(self)?; let facts = if self.is("facts") { self.facts()? } else { InstructionFacts::default() };
                    let loc = if self.is("!") { Some(DebugLoc::read(self)?) } else { None }; self.line()?;
                    body.insts.alloc(Instruction { results, op, facts, loc }).map_err(|e| self.error(DiagnosticKind::Limit, e.to_string()))?; insts.push(inst);
                }
                let term = self.terminator()?; self.line()?;
                body.blocks.alloc(Block { lexical_scope, params, unwind, insts, term }).map_err(|e| self.error(DiagnosticKind::Limit, e.to_string()))?;
            }
        }
        Ok(body)
    }
    fn values(&mut self, body: &mut Body, inst: Option<InstId>, block: BlockId) -> Result<Vec<ValueId>, Diagnostic> {
        self.expect("(")?; let mut values = vec![];
        while !self.is(")") {
            let value = ValueId::from_u32(self.definition("value", "%", true)?);
            self.expect(":")?; let ty = TypeId::read(self)?;
            let facts = if self.eat("facts") { self.expect("(")?; let facts = ValueFacts::read(self)?; self.expect(")")?; facts } else { ValueFacts::Unelaborated };
            let def = if let Some(inst) = inst { Definition::InstResult { inst, index: u16::try_from(values.len()).map_err(|_| self.error(DiagnosticKind::Limit, "too many instruction results"))? } }
                else { Definition::BlockParam { block, index: u32::try_from(values.len()).map_err(|_| self.error(DiagnosticKind::Limit, "too many block parameters"))? } };
            body.values.alloc(ValueData { ty, def, facts }).map_err(|e| self.error(DiagnosticKind::Limit, e.to_string()))?;
            values.push(value); self.comma()?;
        }
        self.expect(")")?; Ok(values)
    }
    fn facts(&mut self) -> Result<InstructionFacts, Diagnostic> {
        self.expect("facts")?; self.expect("(")?;
        self.field("result_deps")?; let result_deps = Vec::read(self)?; self.expect(",")?;
        self.field("retention")?; let retention = Vec::read(self)?; self.expect(",")?;
        self.field("provenance")?; let provenance = Vec::read(self)?; self.expect(")")?;
        Ok(InstructionFacts { result_deps, retention, provenance })
    }
    fn invoke_edge(&mut self, outcome: Outcome) -> Result<InvokeEdge, Diagnostic> {
        let block = BlockId::read(self)?; self.expect("(")?; let mut args = vec![];
        while !self.is(")") {
            self.entity()?;
            if self.peek().text.starts_with('$') && !self.peek().quoted {
                let token = self.next()?;
                self.pending.push(PendingSlot { scope: self.scope, block: self.current_block, outcome, index: args.len(), token });
                args.push(EdgeArg::Produced(EdgeResult { outcome, slot: 0 }));
            } else { args.push(EdgeArg::Value(ValueId::read(self)?)); }
            self.comma()?;
        }
        self.expect(")")?; Ok(InvokeEdge { block, args })
    }
    fn terminator(&mut self) -> Result<Terminator, Diagnostic> {
        let tag = self.next()?;
        let kind = match tag.text.as_str() {
            "br" if !tag.quoted => TermKind::Br(Target::read(self)?),
            "cond" if !tag.quoted => { let condition = ValueId::read(self)?; self.expect(",")?; let yes = Target::read(self)?; self.expect(",")?; let no = Target::read(self)?; TermKind::Cond { condition, yes, no } }
            "switch" if !tag.quoted => { let value = ValueId::read(self)?; self.expect(",")?; let cases = Vec::<SwitchCase>::read(self)?; self.expect(",")?; let default = Target::read(self)?; TermKind::Switch { value, cases, default } }
            "invoke.own" if !tag.quoted => { let op = Box::new(Operation::read(self)?); self.line()?;
                self.expect("normal")?; let normal = self.invoke_edge(Outcome::Normal)?; self.line()?;
                self.expect("unwind")?; let unwind = self.invoke_edge(Outcome::Unwind)?;
                TermKind::InvokeOwn { op, normal, unwind } }
            "ret" if !tag.quoted => TermKind::Ret { values: Vec::read(self)? },
            "throw" if !tag.quoted => TermKind::Throw(ValueId::read(self)?),
            "raise" if !tag.quoted => TermKind::Raise(ValueId::read(self)?),
            "panic" if !tag.quoted => TermKind::Panic(ValueId::read(self)?),
            "trap" if !tag.quoted => { let code = TrapCode::read(self)?; self.expect(",")?; let message = String::read(self)?; TermKind::Trap { code, message } }
            "catch" if !tag.quoted => { let failure = ValueId::read(self)?; self.expect(",")?; let ty = TypeId::read(self)?; self.expect(",")?;
                let handler = self.invoke_edge(Outcome::Normal)?; self.expect(",")?; let next = Target::read(self)?; TermKind::Catch { failure, ty, handler, next } }
            "lookup" if !tag.quoted => { let kind = LookupKind::read(self)?; self.expect(",")?; let collection = ValueId::read(self)?; self.expect(",")?;
                let key = ValueId::read(self)?; self.expect(",")?; let found = self.invoke_edge(Outcome::Normal)?; self.expect(",")?; let missing = Target::read(self)?;
                TermKind::Lookup { kind, collection, key, found, missing } }
            "unreachable" if !tag.quoted => TermKind::Unreachable,
            _ => return Err(self.at(&tag, DiagnosticKind::Syntax, "expected terminator")),
        };
        let loc = if self.is("!") { Some(DebugLoc::read(self)?) } else { None }; Ok(Terminator { kind, loc })
    }
    fn resolve_slots(&self, module: &mut Module) -> Result<(), Diagnostic> {
        for pending in &self.pending {
            let function = FuncId::from_u32((pending.scope - 1) as u32);
            let body = module.functions.get(function).ok().and_then(|f| f.body.as_ref()).ok_or_else(|| self.at(&pending.token, DiagnosticKind::Structure, "missing invoke body"))?;
            let term = &body.blocks.get(pending.block).map_err(|e| self.at(&pending.token, DiagnosticKind::Structure, e.to_string()))?.term.kind;
            let names = edge_slots(module, term, pending.outcome).map_err(|e| self.at(&pending.token, DiagnosticKind::OutcomeSlot, e.message))?;
            let slot = names.iter().position(|name| format!("${}", slot_name(*name)) == pending.token.text)
                .and_then(|i| u16::try_from(i).ok()).ok_or_else(|| self.at(&pending.token, DiagnosticKind::OutcomeSlot, "slot is not in this operation's outcome schema"))?;
            let body = module.functions.get_mut(function).map_err(|e| self.at(&pending.token, DiagnosticKind::Structure, e.to_string()))?.body.as_mut()
                .ok_or_else(|| self.at(&pending.token, DiagnosticKind::Structure, "missing body"))?;
            let term = &mut body.blocks.get_mut(pending.block).map_err(|e| self.at(&pending.token, DiagnosticKind::Structure, e.to_string()))?.term.kind;
            let edge = match term { TermKind::InvokeOwn { normal, unwind, .. } => if pending.outcome == Outcome::Normal { normal } else { unwind },
                TermKind::Catch { handler, .. } => handler, TermKind::Lookup { found, .. } => found,
                _ => return Err(self.at(&pending.token, DiagnosticKind::Structure, "missing invoke edge")) };
            let arg = edge.args.get_mut(pending.index).ok_or_else(|| self.at(&pending.token, DiagnosticKind::Structure, "missing edge argument"))?;
            *arg = EdgeArg::Produced(EdgeResult { outcome: pending.outcome, slot });
        }
        Ok(())
    }
}

impl Codec for Target {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        let block = BlockId::read(r)?; r.expect("(")?; let mut args = vec![];
        while !r.is(")") { r.entity()?; args.push(ValueId::read(r)?); r.comma()?; } r.expect(")")?; Ok(Self { block, args })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        self.block.write(w)?; w.push("("); for (i, value) in self.args.iter().enumerate() { if i > 0 { w.push(", "); } value.write(w)?; } w.push(")"); Ok(())
    }
}

pub(crate) fn edge_slots(module: &Module, term: &TermKind, outcome: Outcome) -> Result<Vec<SlotName>, PrintError> {
    match term {
        TermKind::InvokeOwn { op, .. } => {
            let contract = ContractRegistry::new(module).text_schema(op).map_err(|e| PrintError::new(e.to_string()))?;
            let outcome = contract.outcome(outcome).ok_or_else(|| PrintError::new("operation has no requested outcome"))?;
            Ok(outcome.results.iter().map(|s| s.name).collect())
        }
        TermKind::Catch { .. } | TermKind::Lookup { .. } if outcome == Outcome::Normal => Ok(vec![SlotName::Value]),
        _ => Err(PrintError::new("edge result outside an outcome schema")),
    }
}

/// Iterative graph walk: aliases cannot bypass the nesting bound or overflow
/// the stack, including forward references and cyclic type graphs.
pub(crate) fn check_type_depth(module: &Module, limit: usize) -> Result<(), String> {
    let mut depths = vec![0usize; module.types.len()];
    let mut active = vec![false; module.types.len()];
    for (root, _) in module.types.iter() {
        let mut stack = vec![(root, false)];
        while let Some((id, exiting)) = stack.pop() {
            let i = id.as_u32() as usize;
            let ty = module.types.get(id).map_err(|e| e.to_string())?;
            let children = type_children(ty);
            if exiting {
                let depth = children.iter().map(|child| depths[child.as_u32() as usize]).max().unwrap_or(0).saturating_add(1);
                if depth > limit { return Err("type nesting limit".into()); }
                depths[i] = depth; active[i] = false;
            } else if depths[i] == 0 {
                if active[i] { return Err("cyclic type aliases".into()); }
                active[i] = true; stack.push((id, true));
                for child in children.into_iter().rev() { module.types.get(child).map_err(|e| e.to_string())?; stack.push((child, false)); }
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
        Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char | Type::Str | Type::Range | Type::Any | Type::Never |
        Type::Enum(_) | Type::Variant(_) | Type::Handle(_) | Type::Parameter(_) | Type::Descriptor | Type::Frame(_) | Type::Service(_) => vec![],
    }
}

#[cfg(test)]
pub(super) fn payload_read<T: Codec>(input: &str) -> T {
    let tokens = lex(input, 0, Limits::default()).unwrap();
    let mut names = Names::new();
    for &(ns, name) in TEST_NAMES { names.insert((0, ns, name.into()), 0); }
    let mut reader = Reader::new(&tokens, &mut names, false, Limits::default(), Profile::High);
    let value = T::read(&mut reader).unwrap_or_else(|e| panic!("{e} in {input}"));
    assert!(reader.is("<eof>")); value
}
