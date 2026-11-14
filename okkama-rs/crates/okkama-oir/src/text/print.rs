use std::collections::{HashMap, HashSet};
use super::*;

pub fn print(module: &Module) -> Result<String, PrintError> {
    parser::check_type_depth(module, Limits::default().max_type_depth).map_err(PrintError::new)?;
    let mut w = Writer { module, output: String::new(), names: HashMap::new(), scope: 0, depth: 0, overflow: false };
    w.module_names()?;
    let types = w.type_order()?;
    w.push(match module.profile { Profile::High => "oir 2 high\n", Profile::Own => "oir 2 own\n", Profile::Coro => "oir 2 coro\n", Profile::Low => "oir 2 low\n" });
    w.push("module "); module.name.write(&mut w)?; w.push(" version "); module.version.write(&mut w)?;
    w.push("\nrequires "); module.requires.write(&mut w)?; w.push("\nimports "); module.imports.write(&mut w)?;
    w.push("\nentry "); module.entry.write(&mut w)?; w.push("\ntests "); module.tests.write(&mut w)?; w.push("\n");
    macro_rules! declarations { ($arena:ident, $directive:literal, $ns:literal) => {
        for (id, item) in module.$arena.iter() { w.push(concat!($directive, " ")); w.reference($ns, id.as_u32(), false)?; w.push(" = "); item.write(&mut w)?; w.push("\n"); }
    }; }
    declarations!(files, "file", "file");
    for id in types { w.push("type "); id.write(&mut w)?; w.push(" = "); module.types.get(id).map_err(|e| PrintError::new(e.to_string()))?.write(&mut w)?; w.push("\n"); }
    declarations!(declarations, "decl", "decl"); declarations!(scopes, "scope", "scope");
    declarations!(summaries, "summary", "summary"); declarations!(protocols, "protocol", "protocol");
    for (id, function) in module.functions.iter() {
        w.push("\nfunc "); id.write(&mut w)?; w.push(" "); function.signature.write(&mut w)?;
        w.push(" attrs="); function.attrs.write(&mut w)?; w.push(" type_params="); function.type_params.write(&mut w)?;
        w.push(" summary="); function.summary.write(&mut w)?; w.push(" async_creation="); function.async_creation_summary.write(&mut w)?;
        w.push(" async_body="); function.async_body_summary.write(&mut w)?;
        if let Some(loc) = function.loc { w.push(" "); loc.write(&mut w)?; }
        w.scope = id.as_u32() as usize + 1;
        if let Some(body) = &function.body { w.body_names(body)?; w.push(" {\n    entry "); body.entry.write(&mut w)?; w.push("\n"); w.body(body)?; w.push("}\n"); }
        else { w.push(";\n"); }
        w.scope = 0;
    }
    if w.overflow { return Err(PrintError::new("canonical text size limit")); }
    // Reject any structure whose encoding cannot be read with the public
    // defaults (including nesting, literal and entity limits).
    parser::parse(&w.output).map_err(|e| PrintError::new(e.to_string()))?;
    Ok(w.output)
}

/// Canonical equality ignores SSA names, type-table and SSA arena numbering.
/// Source locations and ordered vectors remain significant.
pub fn normalized_eq(left: &Module, right: &Module) -> Result<bool, PrintError> { Ok(print(left)? == print(right)?) }

pub(crate) struct Writer<'a> {
    pub(crate) module: &'a Module, pub(crate) output: String,
    names: HashMap<(usize, &'static str, u32), String>, scope: usize, depth: usize, overflow: bool,
}
impl Writer<'_> {
    pub(crate) fn push(&mut self, s: &str) {
        if self.output.len().saturating_add(s.len()) > Limits::default().max_text_bytes { self.overflow = true; }
        else if !self.overflow { self.output.push_str(s); }
    }
    pub(crate) fn field(&mut self, name: &str) { self.push(name); self.push("="); }
    pub(crate) fn close_record(&mut self) { if self.output.ends_with(", ") { self.output.truncate(self.output.len() - 2); } self.push(")"); }
    pub(crate) fn enter(&mut self) -> Result<(), PrintError> {
        self.depth += 1; if self.depth > 128 { return Err(PrintError::new("structure nesting exceeds canonical codec limit")); } Ok(())
    }
    pub(crate) fn leave(&mut self) { self.depth -= 1; }
    pub(crate) fn reference(&mut self, ns: &'static str, id: u32, local: bool) -> Result<(), PrintError> {
        let name = self.names.get(&(if local { self.scope } else { 0 }, ns, id)).cloned()
            .ok_or_else(|| PrintError::new(format!("unknown {ns} index {id} in scope {}", self.scope)))?;
        self.push(&name); Ok(())
    }
    fn bind(&mut self, scope: usize, ns: &'static str, id: u32, name: String) -> Result<(), PrintError> {
        if self.names.insert((scope, ns, id), name).is_some() { return Err(PrintError::new(format!("duplicate {ns} definition"))); } Ok(())
    }
    fn module_names(&mut self) -> Result<(), PrintError> {
        let module = self.module;
        macro_rules! arena { ($arena:ident, $ns:literal, $prefix:literal) => {
            for (id, _) in module.$arena.iter() { self.bind(0, $ns, id.as_u32(), format!("{}{}", $prefix, id.as_u32()))?; }
        }; }
        arena!(declarations, "decl", "D"); arena!(scopes, "scope", "S"); arena!(summaries, "summary", "M");
        arena!(protocols, "protocol", "P"); arena!(files, "file", "");
        let mut symbols = HashSet::new();
        for (id, f) in module.functions.iter() {
            if f.symbol.is_empty() || !f.symbol.chars().all(|c| c.is_alphanumeric() || "_.#:".contains(c)) { return Err(PrintError::new("unrepresentable function symbol")); }
            if !symbols.insert(f.symbol.as_str()) { return Err(PrintError::new("duplicate function symbol")); }
            self.bind(0, "func", id.as_u32(), format!("@{}", f.symbol))?;
        }
        Ok(())
    }
    fn type_order(&mut self) -> Result<Vec<TypeId>, PrintError> {
        // Level-by-level structural ordering uses already canonical child names.
        // It never expands a shared type DAG into an exponential-size tree.
        let module = self.module;
        let mut remaining: Vec<_> = module.types.iter().map(|(id, _)| id).collect();
        let mut order = vec![];
        while !remaining.is_empty() {
            let mut next = vec![]; let mut ready = vec![];
            for id in remaining {
                let ty = module.types.get(id).map_err(|e| PrintError::new(e.to_string()))?;
                if parser::type_children(ty).iter().any(|id| !self.names.contains_key(&(0, "type", id.as_u32()))) { next.push(id); continue; }
                let start = self.output.len(); ty.write(self)?; let key = self.output.split_off(start); if self.overflow { return Err(PrintError::new("type encoding size limit")); } ready.push((key, id));
            }
            if ready.is_empty() { return Err(PrintError::new("cyclic or unknown type reference")); }
            ready.sort_by(|a,b| a.0.cmp(&b.0));
            for (_, id) in ready { self.names.insert((0, "type", id.as_u32()), format!("t{}", order.len())); order.push(id); }
            remaining = next;
        }
        Ok(order)
    }
    fn body_names(&mut self, body: &Body) -> Result<(), PrintError> {
        macro_rules! arena { ($arena:ident, $ns:literal, $prefix:literal) => {
            for (id, _) in body.$arena.iter() { self.bind(self.scope, $ns, id.as_u32(), format!("{}{}", $prefix, id.as_u32()))?; }
        }; }
        arena!(bindings, "binding", "b"); arena!(boundaries, "boundary", "B"); arena!(keys, "key", "k");
        arena!(deps, "dependency", "d"); arena!(selections, "selection", "s"); arena!(blocks, "block", "^b");
        let mut value_index = 0; let mut inst_index = 0;
        for (block_id, block) in body.blocks.iter() {
            for (index, value) in block.params.iter().enumerate() {
                let expected = Definition::BlockParam { block: block_id, index: u32::try_from(index).map_err(|_| PrintError::new("too many params"))? };
                self.value_name(body, *value, expected, value_index)?; value_index += 1;
            }
            for id in &block.insts {
                self.bind(self.scope, "inst", id.as_u32(), format!("i{inst_index}"))?; inst_index += 1;
                let inst = body.insts.get(*id).map_err(|e| PrintError::new(e.to_string()))?;
                for (index, value) in inst.results.iter().enumerate() {
                    let expected = Definition::InstResult { inst: *id, index: u16::try_from(index).map_err(|_| PrintError::new("too many results"))? };
                    self.value_name(body, *value, expected, value_index)?; value_index += 1;
                }
            }
        }
        if value_index != body.values.len() || inst_index != body.insts.len() { return Err(PrintError::new("orphan SSA value or instruction is not representable")); }
        Ok(())
    }
    fn value_name(&mut self, body: &Body, value: ValueId, expected: Definition, index: usize) -> Result<(), PrintError> {
        let data = body.values.get(value).map_err(|e| PrintError::new(e.to_string()))?;
        if data.def != expected { return Err(PrintError::new("SSA definition disagrees with its owner")); }
        self.bind(self.scope, "value", value.as_u32(), format!("%{index}"))
    }
    fn body(&mut self, body: &Body) -> Result<(), PrintError> {
        macro_rules! declarations { ($arena:ident, $directive:literal, $ns:literal) => {
            for (id, item) in body.$arena.iter() { self.push(concat!("    ", $directive, " ")); self.reference($ns, id.as_u32(), true)?;
                self.push(concat!(": ", $directive, " = ")); item.write(self)?; self.push("\n"); }
        }; }
        declarations!(bindings, "binding", "binding"); declarations!(boundaries, "boundary", "boundary");
        declarations!(keys, "key", "key"); declarations!(deps, "dependency", "dependency"); declarations!(selections, "selection", "selection");
        for (id, block) in body.blocks.iter() {
            id.write(self)?; self.values(body, &block.params)?; self.push(" scope "); block.lexical_scope.write(self)?;
            if let Some(unwind) = &block.unwind { self.push(" unwind "); unwind.target.write(self)?; }
            self.push(":\n");
            for id in &block.insts {
                let inst = body.insts.get(*id).map_err(|e| PrintError::new(e.to_string()))?;
                self.push("    ["); id.write(self)?; self.push("] "); self.values(body, &inst.results)?; self.push(" = "); inst.op.write(self)?;
                self.push(" facts(result_deps="); inst.facts.result_deps.write(self)?; self.push(", retention="); inst.facts.retention.write(self)?;
                self.push(", provenance="); inst.facts.provenance.write(self)?; self.push(")");
                if let Some(loc) = inst.loc { self.push(" "); loc.write(self)?; } self.push("\n");
            }
            self.push("    "); self.terminator(&block.term)?; self.push("\n");
        }
        Ok(())
    }
    fn values(&mut self, body: &Body, values: &[ValueId]) -> Result<(), PrintError> {
        self.push("(");
        for (index, value) in values.iter().enumerate() {
            if index > 0 { self.push(", "); } value.write(self)?;
            let data = body.values.get(*value).map_err(|e| PrintError::new(e.to_string()))?;
            self.push(": "); data.ty.write(self)?; self.push(" facts("); data.facts.write(self)?; self.push(")");
        }
        self.push(")"); Ok(())
    }
    fn edge(&mut self, term: &TermKind, edge: &InvokeEdge, outcome: Outcome) -> Result<(), PrintError> {
        edge.block.write(self)?; self.push("("); let slots = parser::edge_slots(self.module, term, outcome)?;
        for (index, arg) in edge.args.iter().enumerate() {
            if index > 0 { self.push(", "); }
            match arg {
                EdgeArg::Value(v) => v.write(self)?,
                EdgeArg::Produced(result) => {
                    if result.outcome != outcome { return Err(PrintError::new("edge result belongs to another outcome")); }
                    let name = slots.get(usize::from(result.slot)).ok_or_else(|| PrintError::new("edge result not in operation schema"))?;
                    self.push("$"); self.push(&slot_name(*name));
                }
            }
        }
        self.push(")"); Ok(())
    }
    fn terminator(&mut self, term: &Terminator) -> Result<(), PrintError> {
        match &term.kind {
            TermKind::Br(t) => { self.push("br "); t.write(self)?; }
            TermKind::Cond { condition, yes, no } => { self.push("cond "); condition.write(self)?; self.push(", "); yes.write(self)?; self.push(", "); no.write(self)?; }
            TermKind::Switch { value, cases, default } => { self.push("switch "); value.write(self)?; self.push(", "); cases.write(self)?; self.push(", "); default.write(self)?; }
            TermKind::InvokeOwn { op, normal, unwind } => {
                self.push("invoke.own "); op.write(self)?; self.push("\n        normal "); self.edge(&term.kind, normal, Outcome::Normal)?;
                self.push("\n        unwind "); self.edge(&term.kind, unwind, Outcome::Unwind)?;
            }
            TermKind::Ret { values } => { self.push("ret "); values.write(self)?; }
            TermKind::Throw(v) => { self.push("throw "); v.write(self)?; }
            TermKind::Raise(v) => { self.push("raise "); v.write(self)?; }
            TermKind::Panic(v) => { self.push("panic "); v.write(self)?; }
            TermKind::Trap { code, message } => { self.push("trap "); code.write(self)?; self.push(", "); message.write(self)?; }
            TermKind::Catch { failure, ty, handler, next } => { self.push("catch "); failure.write(self)?; self.push(", "); ty.write(self)?; self.push(", "); self.edge(&term.kind, handler, Outcome::Normal)?; self.push(", "); next.write(self)?; }
            TermKind::Lookup { kind, collection, key, found, missing } => { self.push("lookup "); kind.write(self)?; self.push(", "); collection.write(self)?; self.push(", "); key.write(self)?; self.push(", "); self.edge(&term.kind, found, Outcome::Normal)?; self.push(", "); missing.write(self)?; }
            TermKind::Unreachable => self.push("unreachable"),
        }
        if let Some(loc) = term.loc { self.push(" "); loc.write(self)?; } Ok(())
    }
}

#[cfg(test)]
pub(super) fn payload_print<T: Codec>(value: &T) -> String {
    let module = ModuleBuilder::new("payload", Profile::High).finish().unwrap();
    let mut writer = Writer { module: &module, output: String::new(), names: HashMap::new(), scope: 0, depth: 0, overflow: false };
    for &(ns, name) in TEST_NAMES { writer.bind(0, ns, 0, name.into()).unwrap(); }
    value.write(&mut writer).unwrap(); writer.output
}
