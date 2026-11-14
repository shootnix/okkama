use std::collections::{HashMap, HashSet};
use super::*;

pub fn print(module: &Module) -> Result<String, PrintError> {
    parser::check_type_depth(module, Limits::default().max_type_depth).map_err(PrintError::new)?;
    let mut w = Writer { module, output: String::new(), names: HashMap::new(), scope: 0, depth: 0, overflow: false,
        generic_owner: None, body: None, literal_type: None };
    w.module_names()?;
    w.push(match module.profile {
        Profile::High => "oir 2 high\n", Profile::Own => "oir 2 own\n",
        Profile::Coro => "oir 2 coro\n", Profile::Low => "oir 2 low\n",
    });
    w.push("module ");
    w.word(&module.name)?;
    if let Some(version) = &module.version {
        w.push(" v");
        w.push(version);
    }
    w.push("\n");
    if !module.requires.is_empty() {
        w.push("requires ");
        for (i, feature) in module.requires.iter().enumerate() {
            if i > 0 {
                w.push(", ");
            }
            feature.write(&mut w)?;
        }
        w.push("\n");
    }
    for import in &module.imports {
        w.push("import ");
        if import.major.is_some() {
            w.word(&import.path)?;
        } else {
            import.path.write(&mut w)?;
        }
        if let Some(major) = import.major {
            w.push(" @");
            major.write(&mut w)?;
        }
        if let Some(alias) = &import.alias {
            w.push(" as ");
            w.word(alias)?;
        }
        if !import.take.is_empty() {
            w.push(" take ");
            for (i, name) in import.take.iter().enumerate() {
                if i > 0 {
                    w.push(", ");
                }
                w.word(name)?;
            }
        }
        w.push("\n");
    }
    for (id, file) in module.files.iter() {
        w.push("file ");
        id.write(&mut w)?;
        w.push(" ");
        file.path.write(&mut w)?;
        w.push("\n");
    }
    if let Some(entry) = module.entry {
        w.push("entry ");
        entry.write(&mut w)?;
        w.push("\n");
    }
    if let Some(tests) = &module.tests {
        if tests.is_empty() {
            w.push("test module\n");
        }
        for test in tests {
            w.push("test ");
            test.name.write(&mut w)?;
            w.push(" = ");
            test.function.write(&mut w)?;
            w.push("\n");
        }
    }
    for (id, decl) in module.declarations.iter() {
        w.declaration(id, decl)?;
    }
    for (id, scope) in module.scopes.iter() {
        w.push("scope ");
        id.write(&mut w)?;
        w.push(" ");
        scope.kind.write(&mut w)?;
        w.push(" in ");
        scope.function.write(&mut w)?;
        if let Some(parent) = scope.parent {
            w.push(" parent ");
            parent.write(&mut w)?;
        }
        w.location(scope.loc)?;
        w.push("\n");
    }
    for (id, summary) in module.summaries.iter() {
        w.push("summary ");
        id.write(&mut w)?;
        w.push(" ");
        summary.entry_kind.write(&mut w)?;
        if !summary.parameters.is_empty() {
            w.push(" parameters ");
            summary.parameters.write(&mut w)?;
        }
        if !summary.results.is_empty() {
            w.push(" results ");
            summary.results.write(&mut w)?;
        }
        w.push(" contract ");
        summary.contract.write(&mut w)?;
        w.push("\n");
    }
    for (id, protocol) in module.protocols.iter() {
        w.push("protocol ");
        id.write(&mut w)?;
        if !protocol.holders.is_empty() {
            w.push(" holders ");
            protocol.holders.write(&mut w)?;
        }
        if !protocol.rules.is_empty() {
            w.push(" rules ");
            protocol.rules.write(&mut w)?;
        }
        if !protocol.roots.is_empty() {
            w.push(" roots ");
            protocol.roots.write(&mut w)?;
        }
        w.push("\n");
    }
    for (id, function) in module.functions.iter() {
        w.scope = id.as_u32() as usize + 1;
        w.generic_owner = Some(GenericOwner::Function(id));
        w.body = function.body.as_ref();
        if let Some(body) = &function.body {
            w.body_names(body)?;
        }
        let external = if function.body.is_none() {
            match function.attrs.last() {
                Some(FunctionAttr::Extern(name)) => Some(name), _ => None
            }
        } else {
            None
        };
        w.push(if external.is_some() { "\nextern " } else { "\nfunc " });
        id.write(&mut w)?;
        w.type_parameters(&function.type_params)?;
        if let Some(body) = &function.body {
            let entry = body.blocks.get(body.entry).map_err(|e| PrintError::new(e.to_string()))?;
            let params = entry.params.iter().map(|v| body.values.get(*v).map(|v| v.ty))
                .collect::<Result<Vec<_>, _>>().map_err(|e| PrintError::new(e.to_string()))?;
            if params != function.signature.params {
                return Err(PrintError::new(
                "signature and entry parameters differ"));
            }
            w.values(body, &entry.params, true, true)?;
        } else {
            w.push("(");
            for (i, ty) in function.signature.params.iter().enumerate() {
                if i > 0 {
                    w.push(if is_service(module, *ty) { "; " } else { ", " });
                }
                w.push(&format!("%arg{i}: "));
                ty.write(&mut w)?;
            }
            w.push(")");
        }
        w.push(" -> ");
        w.results(&function.signature.results)?;
        let attrs = if external.is_some() {
            &function.attrs[..function.attrs.len() - 1]
        }
        else {
            &function.attrs[..]
        };
        w.attributes(attrs)?;
        if function.signature.ownership_abi != OwnershipAbi::Unelaborated {
            w.push(" abi ");
            function.signature.ownership_abi.write(&mut w)?;
        }
        w.summary_reference("summary", function.summary)?;
        w.summary_reference("async-creation", function.async_creation_summary)?;
        w.summary_reference("async-body", function.async_body_summary)?;
        w.location(function.loc)?;
        if let Some(body) = &function.body {
            w.push(" {\n");
            if body.entry.as_u32() != 0 {
                w.push("    entry ");
                body.entry.write(&mut w)?;
                w.push("\n");
            }
            w.body(body)?;
            w.push("}\n");
        } else if let Some(name) = external {
            w.push(" = ");
            name.write(&mut w)?;
            w.push("\n");
        } else {
            w.push(";\n");
        }
        w.scope = 0;
        w.generic_owner = None;
        w.body = None;
    }
    if w.overflow {
        return Err(PrintError::new("canonical text size limit"));
    }
    // Reject any structure whose encoding cannot be read with the public
    // defaults (including nesting, literal and entity limits).
    parser::parse(&w.output).map_err(|e| PrintError::new(e.to_string()))?;
    Ok(w.output)
}

/// Canonical equality ignores SSA names, type-table and SSA arena numbering.
/// Source locations and ordered vectors remain significant.
pub fn normalized_eq(left: &Module, right: &Module) -> Result<bool, PrintError> {
    Ok(print(left)? == print(right)?)
}

pub(crate) struct Writer<'a> {
    pub(crate) module: &'a Module, pub(crate) output: String,
    pub(super) names: HashMap<(usize, &'static str, u32), String>, scope: usize, depth: usize, overflow: bool,
    pub(super) generic_owner: Option<GenericOwner>,
    pub(super) body: Option<&'a Body>,
    pub(super) literal_type: Option<TypeId>,
}
impl Writer<'_> {
    pub(crate) fn push(&mut self, s: &str) {
        if self.output.len().saturating_add(s.len()) > Limits::default().max_text_bytes {
            self.overflow = true;
        }
        else if !self.overflow {
            self.output.push_str(s);
        }
    }
    pub(crate) fn field(&mut self, name: &str) {
        self.push(name);
        self.push("=");
    }
    pub(crate) fn close_record(&mut self) {
        if self.output.ends_with(", ") {
            self.output.truncate(
                self.output.len() - 2);
        }
        self.push(")");
    }
    pub(crate) fn enter(&mut self) -> Result<(), PrintError> {
        if self.overflow {
            return Err(PrintError::new("canonical text size limit"));
        }
        self.depth += 1;
        if self.depth > 128 {
            return Err(PrintError::new("structure nesting exceeds canonical codec limit"));
        }
        Ok(())
    }
    pub(crate) fn leave(&mut self) {
        self.depth -= 1;
    }
    pub(crate) fn reference(&mut self, ns: &'static str, id: u32, local: bool) -> Result<(), PrintError> {
        let name = self.names.get(&(if local {
            self.scope
        } else {
            0
        }, ns, id)).cloned()
            .ok_or_else(|| PrintError::new(format!("unknown {ns} index {id} in scope {}", self.scope)))?;
        self.push(&name);
        Ok(())
    }
    fn bind(&mut self, scope: usize, ns: &'static str, id: u32, name: String) -> Result<(), PrintError> {
        if self.names.insert((scope, ns, id),
            name).is_some() { return Err(PrintError::new(format!("duplicate {ns} definition")));
        } Ok(())
    }
    fn module_names(&mut self) -> Result<(), PrintError> {
        let module = self.module;
        macro_rules! arena { ($arena:ident, $ns:literal, $prefix:literal) => {
            for (id, _) in module.$arena.iter() {
                self.bind(0, $ns, id.as_u32(), format!("{}{}", $prefix,
                    id.as_u32()))?;
            }
        };
        }
        arena!(scopes, "scope", "S");
        arena!(summaries, "summary", "M");
        arena!(protocols, "protocol", "P");
        arena!(files, "file", "");
        let mut decl_names = HashSet::new();
        for (id, decl) in module.declarations.iter() {
            if !decl_names.insert(&decl.name) {
                return Err(PrintError::new("duplicate declaration name"));
            }
            self.bind(0, "decl", id.as_u32(), decl.name.clone())?;
        }
        let mut symbols = HashSet::new();
        for (id, f) in module.functions.iter() {
            if f.symbol.is_empty() || !f.symbol.chars().all(|c| c.is_alphanumeric()
                || "_.#:".contains(c)) { return Err(PrintError::new("unrepresentable function symbol"));
            }
            if !symbols.insert(f.symbol.as_str()) {
                return Err(PrintError::new("duplicate function symbol"));
            }
            self.bind(0, "func", id.as_u32(), format!("@{}", f.symbol))?;
        }
        Ok(())
    }
    fn body_names(&mut self, body: &Body) -> Result<(), PrintError> {
        macro_rules! arena { ($arena:ident, $ns:literal, $prefix:literal) => {
            for (id, _) in body.$arena.iter() {
                self.bind(self.scope, $ns, id.as_u32(), format!("{}{}", $prefix,
                    id.as_u32()))?;
            }
        };
        }
        arena!(bindings, "binding", "b");
        arena!(boundaries, "boundary", "B");
        arena!(keys, "key", "k");
        arena!(deps, "dependency", "d");
        arena!(blocks, "block", "^b");
        let mut selection_index = 0;
        for (id, selection) in body.selections.iter() {
            if !matches!(selection, SelectionExpr::Value(_)) {
                self.bind(self.scope, "selection", id.as_u32(), format!("s{selection_index}"))?;
                selection_index += 1;
            }
        }
        let mut value_index = 0;
        let mut inst_index = 0;
        for (block_id, block) in body.blocks.iter() {
            for (index, value) in block.params.iter().enumerate() {
                let expected = Definition::BlockParam { block: block_id, index: u32::try_from(index).map_err(|_
                    | PrintError::new("too many params"))? };
                self.value_name(body, *value, expected, value_index)?;
                value_index += 1;
            }
            for id in &block.insts {
                self.bind(self.scope, "inst", id.as_u32(), format!("i{inst_index}"))?;
                inst_index += 1;
                let inst = body.insts.get(*id).map_err(|e| PrintError::new(e.to_string()))?;
                for (index, value) in inst.results.iter().enumerate() {
                    let expected = Definition::InstResult { inst: *id, index: u16::try_from(index).map_err(|_
                        | PrintError::new("too many results"))? };
                    self.value_name(body, *value, expected, value_index)?;
                    value_index += 1;
                }
            }
        }
        if value_index != body.values.len()
            || inst_index != body.insts.len() { return Err(PrintError::new(
            "orphan SSA value or instruction is not representable"));
        }
        Ok(())
    }
    fn value_name(&mut self, body: &Body, value: ValueId, expected: Definition, index: usize) -> Result<(),
    PrintError> {
        let data = body.values.get(value).map_err(|e| PrintError::new(e.to_string()))?;
        if data.def != expected {
            return Err(PrintError::new("SSA definition disagrees with its owner"));
        }
        self.bind(self.scope, "value", value.as_u32(), format!("%{index}"))
    }
    fn body(&mut self, body: &Body) -> Result<(), PrintError> {
        for (id, binding) in body.bindings.iter() {
            self.push("    binding ");
            id.write(self)?;
            self.push(" ");
            binding.name.write(self)?;
            self.push(": ");
            binding.ty.write(self)?;
            self.push(" in ");
            binding.scope.write(self)?;
            self.location(binding.loc)?;
            self.push("\n");
        }
        for (id, boundary) in body.boundaries.iter() {
            self.push("    boundary ");
            id.write(self)?;
            self.push(" ");
            boundary.kind.write(self)?;
            self.push(" in ");
            boundary.scope.write(self)?;
            self.push("\n");
        }
        macro_rules! declarations {
            ($arena:ident, $directive:literal) => {
                for (id, item) in body.$arena.iter() {
                    self.push(concat!("    ", $directive, " "));
                    id.write(self)?;
                    self.push(concat!(": ", $directive, " = "));
                    item.write(self)?;
                    self.push("\n");
                }
            };
        }
        declarations!(keys, "key");
        declarations!(deps, "dependency");
        for (id, selection) in body.selections.iter() {
            if matches!(selection, SelectionExpr::Value(_)) {
                continue;
            }
            self.push("    selection ");
            self.reference("selection", id.as_u32(), true)?;
            self.push(": selection = ");
            selection.write(self)?;
            self.push("\n");
        }
        let mut referenced = HashSet::new();
        for (_, inst) in body.insts.iter() {
            referenced_creation(&inst.op, &mut referenced);
        }
        for (_, block) in body.blocks.iter() {
            if let TermKind::InvokeOwn {
                op, ..
            }
            = &block.term.kind { referenced_creation(op, &mut referenced);
            }
        }
        for (id, block) in body.blocks.iter() {
            self.push("    ");
            id.write(self)?;
            if id != body.entry && !block.params.is_empty() {
                self.values(body, &block.params, true, false)?;
            }
            self.push(" in ");
            block.lexical_scope.write(self)?;
            if let Some(unwind) = &block.unwind {
                self.push(" unwind ");
                unwind.target.write(self)?;
            }
            self.push(":\n");
            for id in &block.insts {
                let inst = body.insts.get(*id).map_err(|e| PrintError::new(e.to_string()))?;
                self.push("    ");
                if referenced.contains(id) {
                    self.push("[");
                    id.write(self)?;
                    self.push("] ");
                }
                if !inst.results.is_empty() {
                    self.values(body, &inst.results, inst.results.len() > 1, false)?;
                    self.push(" = ");
                }
                self.literal_type = inst.results.first().and_then(|id| body.values.get(*id).ok()).map(|v| v.ty);
                inst.op.write(self)?;
                self.facts(&inst.facts)?;
                self.location(inst.loc)?;
                self.push("\n");
            }
            self.push("    ");
            self.terminator(&block.term)?;
            self.push("\n");
        }
        Ok(())
    }

    pub(super) fn results(&mut self, results: &[TypeId]) -> Result<(), PrintError> {
        if results.is_empty() {
            self.push("void");
        }
        else if results.len() == 1 {
            results[0].write(self)?;
        }
        else {
            self.push("(");
            types::write_types(results, self)?;
            self.push(")");
        }
        Ok(())
    }

    fn values(&mut self, body: &Body, values: &[ValueId], parens: bool, service_separator: bool)
    -> Result<(), PrintError>
    {
        if parens {
            self.push("(");
        }
        let mut service_started = false;
        for (index, value) in values.iter().enumerate() {
            let data = body.values.get(*value).map_err(|e| PrintError::new(e.to_string()))?;
            let service = service_separator && is_service(self.module, data.ty);
            if index > 0 {
                self.push(if service && !service_started { "; " } else { ", " });
            }
            service_started |= service;
            value.write(self)?;
            self.push(": ");
            data.ty.write(self)?;
            if data.facts != value_default(self.module.profile, self.module.types.get(data.ty).ok()) {
                self.push(" facts(");
                data.facts.write(self)?;
                self.push(")");
            }
        }
        if parens {
            self.push(")");
        }
        Ok(())
    }

    fn facts(&mut self, facts: &InstructionFacts) -> Result<(), PrintError> {
        if *facts == InstructionFacts::default() {
            return Ok(());
        }
        self.push(" facts(");
        let mut first = true;
        macro_rules! component {
            ($field:ident) => {
                if !facts.$field.is_empty() {
                    if !first {
                        self.push(", ");
                    }
                    first = false;
                    self.field(stringify!($field));
                    facts.$field.write(self)?;
                }
            };
        }
        component!(result_deps);
        component!(retention);
        component!(provenance);
        let _ = first;
        self.push(")");
        Ok(())
    }

    fn edge(&mut self, term: &TermKind, edge: &InvokeEdge, outcome: Outcome) -> Result<(), PrintError> {
        edge.block.write(self)?;
        self.push("(");
        let slots = parser::edge_slots(self.module, self.body.ok_or_else(|| PrintError::new("missing invoke body"))?, term, outcome)?;
        for (index, arg) in edge.args.iter().enumerate() {
            if index > 0 {
                self.push(", ");
            }
            match arg {
                EdgeArg::Value(v) => v.write(self)?,
                EdgeArg::Produced(result) => {
                    if result.outcome != outcome {
                        return Err(PrintError::new(
                        "edge result belongs to another outcome"));
                    }
                    let name = slots.get(usize::from(result.slot))
                        .ok_or_else(|| PrintError::new("edge result not in operation schema"))?;
                    self.push("$");
                    self.push(&slot_name(*name));
                }
            }
        }
        self.push(")");
        Ok(())
    }
    fn terminator(&mut self, term: &Terminator) -> Result<(), PrintError> {
        match &term.kind {
            TermKind::Br(t) => {
                self.push("br ");
                t.write(self)?;
            }
            TermKind::Cond { condition, yes, no } => {
                self.push("cond ");
                condition.write(self)?;
                self.push(", ");
                yes.write(self)?;
                self.push(", ");
                no.write(self)?;
            }
            TermKind::Switch { value, cases, default } => {
                self.push("switch ");
                value.write(self)?;
                self.push(", ");
                cases.write(self)?;
                self.push(", else ");
                default.write(self)?;
            }
            TermKind::InvokeOwn { op, normal, unwind } => {
                self.push("invoke.own ");
                op.write(self)?;
                self.push("\n        normal ");
                self.edge(&term.kind, normal, Outcome::Normal)?;
                self.push("\n        unwind ");
                self.edge(&term.kind, unwind, Outcome::Unwind)?;
            }
            TermKind::Ret { values } => {
                self.push("ret");
                for (i, value) in values.iter().enumerate() {
                    self.push(if i == 0 { " " } else { ", " });
                    value.write(self)?;
                }
            }
            TermKind::Throw(v) => {
                self.push("throw ");
                v.write(self)?;
            }
            TermKind::Raise(v) => {
                self.push("raise ");
                v.write(self)?;
            }
            TermKind::Panic(v) => {
                self.push("panic ");
                v.write(self)?;
            }
            TermKind::Trap { code, message } => {
                self.push("trap ");
                code.write(self)?;
                self.push(" ");
                message.write(self)?;
            }
            TermKind::Catch { failure, ty, handler, next } => {
                self.push("catch ");
                failure.write(self)?;
                self.push(" is ");
                ty.write(self)?;
                self.push(", ");
                self.edge(&term.kind, handler, Outcome::Normal)?;
                self.push(", ");
                next.write(self)?;
            }
            TermKind::Lookup { kind, collection, key, found, missing } => {
                self.push(match kind { LookupKind::Array => "array.lookup ", LookupKind::Map => "map.lookup " });
                collection.write(self)?;
                self.push(", ");
                key.write(self)?;
                self.push(", ");
                self.edge(&term.kind, found, Outcome::Normal)?;
                self.push(", ");
                missing.write(self)?;
            }
            TermKind::Unreachable => self.push("unreachable"),
        }
        if let Some(loc) = term.loc {
            self.push(" ");
            loc.write(self)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn payload_print<T: Codec>(value: &T) -> String {
    let mut builder = ModuleBuilder::new("payload", Profile::High);
    builder.intern_type(Type::Int).unwrap();
    let module = builder.finish().unwrap();
    let mut writer = Writer { module: &module, output: String::new(), names: HashMap::new(), scope: 0, depth: 0,
        overflow: false, generic_owner: None, body: None, literal_type: None };
    for &(ns, name) in TEST_NAMES {
        writer.bind(0, ns, 0, name.into()).unwrap();
    }
    value.write(&mut writer).unwrap();
    writer.output
}

fn is_service(module: &Module, ty: TypeId) -> bool {
    matches!(module.types.get(ty), Ok(Type::Service(_)))
}

fn referenced_creation(op: &Operation, referenced: &mut HashSet<InstId>) {
    if let Operation::Async(AsyncOp::UniqueCacheProof { proof }) = op
        && let CreationOrigin::Instruction(inst) = proof.evidence.creation
    { referenced.insert(inst); }
}
