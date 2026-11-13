//! Read-only ownership census after the OIR passes (#553).
//!
//! Counts instruction sites, not allocations or execution frequencies. A call
//! returning a possibly fresh identity has a separate `call.result` site in
//! the caller; the allocation in its callee is also reported. Parameter-only
//! aliases do not introduce sites. Owners describe escape from the function,
//! not the first binding's precise lexical region. No region is rewritten.
//!
//! Provenance distinguishes roots, contents, and borrowed child projections.
//! Unknown operations are sinks; parameters and opaque origins never prove a region operation static.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use super::regions::Plain;
use super::{Function, InstKind, Module, TermKind, Type, ValueId, cfg};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Escape {
    pub result: bool,
    pub result_field: bool,
    /// The result borrows a child, not the parameter identity.
    pub result_part: bool,
    pub reasons: BTreeSet<String>,
}

impl Escape {
    pub fn escapes(&self) -> bool {
        self.result || self.result_field || self.result_part || !self.reasons.is_empty()
    }

    fn join(&mut self, other: &Self) {
        self.result |= other.result;
        self.result_field |= other.result_field;
        self.result_part |= other.result_part;
        self.reasons.extend(other.reasons.iter().cloned());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
    Scope,
    Result,
    Aggregate { owner: AggregateOwner, containers: Vec<usize> },
    Unknown(BTreeSet<String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateOwner {
    Scope,
    Result,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creation {
    pub block: usize,
    pub instruction: usize,
    pub value: usize,
    pub kind: String,
    pub owner: Owner,
    /// Origin IDs retained without acquiring a second owner.
    pub borrowed_contents: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionOp {
    pub block: usize,
    pub instruction: usize,
    pub kind: String,
    pub values: Vec<usize>,
    pub statically_owned: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub scope: usize,
    pub result: usize,
    pub aggregate: usize,
    pub unknown: usize,
    /// A site can contribute to more than one reason.
    pub reasons: BTreeMap<String, usize>,
    pub region_static: usize,
    pub region_dynamic: usize,
}

impl Totals {
    fn add(&mut self, other: &Self) {
        self.scope += other.scope;
        self.result += other.result;
        self.aggregate += other.aggregate;
        self.unknown += other.unknown;
        self.region_static += other.region_static;
        self.region_dynamic += other.region_dynamic;
        for (reason, count) in &other.reasons {
            *self.reasons.entry(reason.clone()).or_default() += count;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionReport {
    pub function: usize,
    pub symbol: String,
    pub parameters: Vec<Escape>,
    pub creations: Vec<Creation>,
    pub region_ops: Vec<RegionOp>,
    pub totals: Totals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub totals: Totals,
    pub functions: Vec<FunctionReport>,
    /// Function indices, descending by unknown count, ties by module order.
    pub top_unknown: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Summary {
    parameters: Vec<Escape>,
    fresh: bool,
    opaque_result: bool,
    result_reasons: BTreeSet<String>,
}

impl Summary {
    fn join(&mut self, other: &Self) {
        for (parameter, next) in self.parameters.iter_mut().zip(&other.parameters) {
            parameter.join(next);
        }
        self.fresh |= other.fresh;
        self.opaque_result |= other.opaque_result;
        self.result_reasons.extend(other.result_reasons.iter().cloned());
    }
}

/// Analyze the finished module without changing it or consulting environment variables.
pub fn analyze(module: &Module) -> Report {
    // In a closed program without destroy, even boxes have nothing to own.
    if module.objects.iter().all(|object| object.destroy.is_none()) {
        return Report {
            totals: Totals::default(),
            functions: module.functions.iter().enumerate().map(|(function, body)| FunctionReport {
                function, symbol: body.symbol.clone(),
                parameters: vec![Escape::default(); body.params.len()],
                creations: Vec::new(), region_ops: Vec::new(), totals: Totals::default(),
            }).collect(),
            top_unknown: (0..module.functions.len().min(50)).collect(),
        };
    }
    let plain = CensusPlain::of(module);
    let mut destructors = HashMap::new();
    for ty in module.functions.iter().flat_map(|function| &function.values) {
        destructors.entry(ty.clone()).or_insert_with(|| destroyers(module, ty));
    }
    let mut summaries: Vec<Summary> = module.functions.iter().map(|function| Summary {
        parameters: vec![Escape::default(); function.params.len()],
        ..Summary::default()
    }).collect();
    let edges: Vec<Vec<usize>> = module.functions.iter().map(|function| {
        function.blocks.iter().flat_map(|block| &block.insts)
            .flat_map(|inst| callees(module, function, &inst.kind))
            .chain(function.values.iter().flat_map(|ty| destructors[ty].iter().copied()))
            .collect::<BTreeSet<_>>().into_iter().collect()
    }).collect();
    for component in components(&edges) {
        loop {
            let mut changed = false;
            for &index in &component {
                let flow = Flow::build(module, index, &plain, &summaries, &destructors);
                let next = flow.summary(&module.functions[index]);
                let before = summaries[index].clone();
                summaries[index].join(&next);
                changed |= summaries[index] != before;
            }
            if !changed {
                break;
            }
        }
    }
    let mut report = Report { totals: Totals::default(), functions: Vec::new(), top_unknown: Vec::new() };
    for (index, function) in module.functions.iter().enumerate() {
        let flow = Flow::build(module, index, &plain, &summaries, &destructors);
        let found = flow.report(index, function, &plain, summaries[index].parameters.clone());
        report.totals.add(&found.totals);
        report.functions.push(found);
    }
    report.top_unknown = (0..report.functions.len()).collect();
    report.top_unknown.sort_by_key(|&index| (std::cmp::Reverse(report.functions[index].totals.unknown), index));
    report.top_unknown.truncate(50);
    report
}

fn callees(module: &Module, function: &Function, kind: &InstKind) -> Vec<usize> {
    match kind {
        InstKind::Call { func, .. } => vec![func.0],
        InstKind::FnClosure { func, .. } => vec![func.0],
        InstKind::CallFn { callee, .. } => match definition(function, *callee) {
            Some(InstKind::FnClosure { func, .. }) => vec![func.0],
            _ => Vec::new(),
        },
        InstKind::CallCap { cap, field } => {
            let Type::Cap(id) = function.values[cap.0] else { return Vec::new() };
            module.caps.get(id.0).and_then(|cap| cap.fields.get(*field))
                .map_or_else(Vec::new, |(_, func)| vec![func.0])
        }
        InstKind::CallRole { receiver, slot, .. } => {
            let Type::Role(role) = function.values[receiver.0] else { return Vec::new() };
            module.objects.iter().flat_map(|object| &object.does)
                .filter(|witness| witness.role == role)
                .filter_map(|witness| witness.slots.get(*slot).map(|func| func.0))
                .collect::<BTreeSet<_>>().into_iter().collect()
        }
        _ => Vec::new(),
    }
}

/// Cleanup can call user code that publishes an identity or one of its fields.
fn destroyers(module: &Module, ty: &Type) -> Vec<usize> {
    let mut types = vec![ty];
    let mut objects = BTreeSet::new();
    let mut result = BTreeSet::new();
    while let Some(ty) = types.pop() {
        let candidates = match ty {
            Type::Object(id) | Type::Successor(id) => vec![id.0],
            Type::Role(role) => module.objects.iter().enumerate().filter_map(|(index, object)| {
                object.does.iter().any(|witness| witness.role == *role).then_some(index)
            }).collect(),
            Type::Variant(id) => module.variants[id.0].cases.iter().map(|(_, object)| object.0).collect(),
            Type::Array(element) | Type::Builder(element) => {
                types.push(element);
                Vec::new()
            }
            Type::Map(key, value) | Type::MapBuilder(key, value) => {
                types.extend([key.as_ref(), value.as_ref()]);
                Vec::new()
            }
            _ => Vec::new(),
        };
        for index in candidates {
            if objects.insert(index) {
                let object = &module.objects[index];
                if let Some(destroy) = object.destroy {
                    result.insert(destroy.0);
                }
                types.extend(object.fields.iter().map(|field| &field.ty));
            }
        }
    }
    result.into_iter().collect()
}

/// Reuse Tarjan's cycle partition, then visit the condensed DAG callee first.
fn components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let cycles = cfg::cycles(edges);
    let mut next = cycles.iter().flatten().max().map_or(0, |id| id + 1);
    let groups: Vec<usize> = cycles.into_iter().map(|cycle| cycle.unwrap_or_else(|| {
        let id = next;
        next += 1;
        id
    })).collect();
    let mut members = vec![Vec::new(); next];
    let mut dependencies = vec![BTreeSet::new(); next];
    let mut callers = vec![BTreeSet::new(); next];
    for (from, targets) in edges.iter().enumerate() {
        members[groups[from]].push(from);
        for &to in targets {
            if groups[from] != groups[to] {
                dependencies[groups[from]].insert(groups[to]);
                callers[groups[to]].insert(groups[from]);
            }
        }
    }
    let mut ready: VecDeque<usize> = (0..next).filter(|&id| dependencies[id].is_empty()).collect();
    let mut ordered = Vec::new();
    while let Some(id) = ready.pop_front() {
        ordered.push(std::mem::take(&mut members[id]));
        for &caller in &callers[id] {
            dependencies[caller].remove(&id);
            if dependencies[caller].is_empty() {
                ready.push_back(caller);
            }
        }
    }
    ordered
}

fn definition(function: &Function, value: ValueId) -> Option<&InstKind> {
    function.blocks.iter().flat_map(|block| &block.insts)
        .find(|inst| inst.result == Some(value)).map(|inst| &inst.kind)
}

// Deferred values retain resources independently of their result type.
struct CensusPlain(HashMap<Type, bool>);

impl CensusPlain {
    fn of(module: &Module) -> Self {
        let plain = Plain::of(module);
        fn retains_deferred(module: &Module, ty: &Type, seen: &mut BTreeSet<usize>) -> bool {
            match ty {
                Type::Future(_) | Type::Fn(_) => true,
                Type::Array(element) | Type::Builder(element) => retains_deferred(module, element, seen),
                Type::Map(_, value) | Type::MapBuilder(_, value) => retains_deferred(module, value, seen),
                Type::Object(id) | Type::Successor(id) => seen.insert(id.0)
                    && module.objects[id.0].fields.iter().any(|field| retains_deferred(module, &field.ty, seen)),
                Type::Role(role) => module.objects.iter().enumerate().any(|(id, object)| {
                    object.does.iter().any(|witness| witness.role == *role)
                        && retains_deferred(module, &Type::Object(super::ObjectId(id)), seen)
                }),
                Type::Variant(id) => module.variants[id.0].cases.iter().any(|(_, object)| {
                    retains_deferred(module, &Type::Object(*object), seen)
                }),
                _ => false,
            }
        }
        Self(module.functions.iter().flat_map(|f| &f.values).map(|ty| {
            (ty.clone(), plain.ty(ty) && !retains_deferred(module, ty, &mut BTreeSet::new()))
        }).collect())
    }

    fn ty(&self, ty: &Type) -> bool {
        self.0[ty]
    }
}

fn opaque_reason(function: &Function) -> Option<String> {
    if let Some((name, _)) = function.intrinsic() {
        return Some(if name.starts_with("okkama.atomic.") {
            "atomic".to_string()
        } else if name.starts_with("okkama.channel.") {
            "channel".to_string()
        } else if name.starts_with("okkama.scope.") || name.starts_with("okkama.collect.") {
            "future".to_string()
        } else {
            format!("intrinsic:{name}")
        });
    }
    if let Some(symbol) = function.extern_symbol() {
        Some(format!("native:{symbol}"))
    } else if function.blocks.is_empty() {
        Some(format!("native:{}", function.symbol))
    } else {
        None
    }
}

// Each origin is a parameter, an instruction result, or an opaque block input.
// Child reads cannot prove that the container itself escapes.
const DIRECT: u8 = 1;
const INSIDE: u8 = 2;
const PART: u8 = 4;
const BORROW: u8 = 8;
type Facts = BTreeMap<usize, u8>;

#[derive(Clone, Copy)]
enum EdgeKind {
    Alias,
    Store,
    Read,
    Contents,
    Borrow,
}

struct Flow {
    facts: Vec<Facts>,
    edges: Vec<Vec<(usize, EdgeKind)>>,
    effects: Vec<Escape>,
    sites: BTreeMap<usize, (usize, usize, String)>,
    opaque: BTreeSet<usize>,
    sinks: Vec<(usize, String)>,
    returns: Vec<usize>,
    // Borrowing is harmless until a deferred value escapes.
    deferred: BTreeMap<usize, String>,
    awaited: Vec<(usize, usize)>,
}

impl Flow {
    fn build(
        module: &Module,
        index: usize,
        plain: &CensusPlain,
        summaries: &[Summary],
        destructors: &HashMap<Type, Vec<usize>>,
    ) -> Self {
        let function = &module.functions[index];
        let count = function.values.len();
        let mut flow = Self {
            facts: vec![Facts::new(); count], edges: vec![Vec::new(); count],
            effects: vec![Escape::default(); count], sites: BTreeMap::new(),
            opaque: BTreeSet::new(), sinks: Vec::new(), returns: Vec::new(),
            deferred: BTreeMap::new(), awaited: Vec::new(),
        };
        for &parameter in &function.params {
            if !plain.ty(&function.values[parameter.0]) {
                flow.facts[parameter.0].insert(parameter.0, DIRECT);
                if let Some(reason) = opaque_reason(function) {
                    flow.sink(parameter, &reason);
                }
            }
        }
        let mut await_results = BTreeMap::new();
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction, inst) in block.insts.iter().enumerate() {
                let result = inst.result.filter(|value| !plain.ty(&function.values[value.0]));
                let operands = inst.kind.operands();
                let mut site = None;
                match &inst.kind {
                    InstKind::ObjNew { .. } | InstKind::VariantNew { .. }
                    | InstKind::ArrayNew(_) | InstKind::MapNew(_)
                    | InstKind::BuilderNew | InstKind::MapBuilderNew => {
                        site = Some(name(&inst.kind));
                        if let Some(to) = result {
                            for &from in &operands {
                                flow.edge(from, to, EdgeKind::Store);
                            }
                        }
                    }
                    InstKind::SuccTake(from) | InstKind::Cvt(from) | InstKind::RoleWrap(from)
                    | InstKind::VariantOf(from) | InstKind::RoleNarrow { value: from, .. }
                    | InstKind::Retag(from) | InstKind::MapRetag(from)
                    | InstKind::BuilderFinish(from) | InstKind::MapBuilderFinish(from) => {
                        if let Some(to) = result {
                            flow.edge(*from, to, EdgeKind::Alias);
                        }
                    }
                    InstKind::ObjGet { object: from, .. } | InstKind::OnceLoad { object: from, .. }
                    | InstKind::VariantGet { value: from, .. } | InstKind::ArrayGet { array: from, .. }
                    | InstKind::MapGet { map: from, .. } => {
                        if let Some(to) = result {
                            flow.edge(*from, to, EdgeKind::Read);
                        }
                    }
                    InstKind::ArraySlice { array, .. } | InstKind::ArrayRemove { array, .. }
                    | InstKind::ArrayDifference { array, .. } => {
                        site = Some(name(&inst.kind));
                        if let Some(to) = result {
                            flow.edge(*array, to, EdgeKind::Contents);
                        }
                    }
                    InstKind::ArrayReplace { array, value, .. } => {
                        site = Some(name(&inst.kind));
                        if let Some(to) = result {
                            flow.edge(*array, to, EdgeKind::Contents);
                            flow.edge(*value, to, EdgeKind::Store);
                        }
                    }
                    InstKind::ArrayConcat(parts) | InstKind::MapMerge(parts) => {
                        site = Some(name(&inst.kind));
                        if let Some(to) = result {
                            for &part in parts {
                                flow.edge(part, to, EdgeKind::Contents);
                            }
                        }
                    }
                    InstKind::MapDifference { map, .. } | InstKind::MapKeys(map)
                    | InstKind::MapValues(map) => {
                        site = Some(name(&inst.kind));
                        if let Some(to) = result {
                            flow.edge(*map, to, EdgeKind::Contents);
                        }
                    }
                    InstKind::ObjNext { object, updates } => {
                        site = Some(name(&inst.kind));
                        if let Some(to) = result {
                            flow.edge(*object, to, EdgeKind::Contents);
                            for &(_, value) in updates {
                                flow.edge(value, to, EdgeKind::Store);
                            }
                        }
                    }
                    InstKind::BuilderPush { builder, value }
                    | InstKind::MapBuilderPut { builder, value, .. } => {
                        if let Some(to) = result {
                            flow.edge(*builder, to, EdgeKind::Alias);
                            flow.edge(*value, to, EdgeKind::Store);
                        }
                    }
                    InstKind::BuilderAppend { builder, array } => {
                        if let Some(to) = result {
                            flow.edge(*builder, to, EdgeKind::Alias);
                            flow.edge(*array, to, EdgeKind::Contents);
                        }
                    }
                    InstKind::BuilderAppendRange { builder, .. } => {
                        if let Some(to) = result {
                            flow.edge(*builder, to, EdgeKind::Alias);
                        }
                    }
                    InstKind::CapMake(owner) => {
                        if let Some(to) = result {
                            // Lowered caps represent their owner, not a fresh identity.
                            flow.edge(*owner, to, EdgeKind::Alias);
                        }
                    }
                    InstKind::Call { .. } | InstKind::CallRole { .. }
                    | InstKind::CallCap { .. } | InstKind::CallFn { .. } => {
                        let targets = callees(module, function, &inst.kind);
                        let mut arguments = operands.clone();
                        if let InstKind::CallFn { callee, args } = &inst.kind
                            && let Some(InstKind::FnClosure { captures, .. }) = definition(function, *callee)
                        {
                            arguments = captures.iter().chain(args).copied().collect();
                        }
                        if targets.is_empty() {
                            let reason = match inst.kind {
                                InstKind::CallCap { .. } => "cap",
                                InstKind::CallFn { .. } => "indirect-call",
                                _ => "role-call",
                            };
                            if reason == "cap" {
                                if let Some(to) = result { flow.sink(to, reason); }
                            } else {
                                flow.unknown(&arguments, result, reason);
                            }
                            site = Some("call.result".to_string());
                        }
                        for target in targets {
                            if module.functions[target].is_async() {
                                site = Some("call.future".to_string());
                                if let Some(to) = result {
                                    for &argument in &arguments {
                                        flow.edge(argument, to, EdgeKind::Borrow);
                                    }
                                    flow.deferred.insert(to.0, "future".to_string());
                                }
                            } else if flow.call(&module.functions[target], &summaries[target], &arguments, result) {
                                site = Some("call.result".to_string());
                            }
                        }
                    }
                    InstKind::FnClosure { captures, .. } => {
                        site = Some(name(&inst.kind));
                        if let Some(to) = result {
                            for &capture in captures {
                                flow.edge(capture, to, EdgeKind::Borrow);
                            }
                            flow.deferred.insert(to.0, "closure".to_string());
                        }
                    }
                    InstKind::AnyBox(_) => {
                        flow.unknown(&operands, result, "any");
                        site = Some(name(&inst.kind));
                    }
                    InstKind::Await(future) => {
                        if let Some(InstKind::Call { func, args }) = definition(function, *future)
                            && module.functions[func.0].is_async()
                        {
                            let fresh = flow.call(&module.functions[func.0], &summaries[func.0], args, result);
                            if let Some(to) = result {
                                // Repeated awaits reuse the future's cached identity.
                                if let Some(&previous) = await_results.get(future) {
                                    flow.edge(previous, to, EdgeKind::Alias);
                                } else {
                                    await_results.insert(*future, to);
                                    if fresh { site = Some("await.result".to_string()); }
                                }
                                flow.awaited.push((future.0, to.0));
                            }
                        } else {
                            flow.unknown(&operands, result, "future");
                            site = Some(name(&inst.kind));
                        }
                    }
                    InstKind::FutSpawn(_) | InstKind::WorkerSpawn { .. }
                    | InstKind::WorkerBind { .. } => {
                        flow.unknown(&operands, result, "spawn");
                        site = Some(name(&inst.kind));
                    }
                    InstKind::Fut { .. } => {
                        flow.unknown(&operands, result, "future");
                        site = Some(name(&inst.kind));
                    }
                    InstKind::Global(_) | InstKind::Undef => {
                        if let Some(to) = result {
                            flow.facts[to.0].insert(to.0, DIRECT);
                            flow.opaque.insert(to.0);
                            flow.sink(to, &format!("other:{}", name(&inst.kind)));
                        }
                    }
                    InstKind::RegionEnter | InstKind::RegionAdopt { .. }
                    | InstKind::RegionTrack { .. } | InstKind::RegionTransfer { .. }
                    | InstKind::RegionLeave { .. } | InstKind::RegionSettle { .. }
                    | InstKind::RegionUnwind { .. } | InstKind::RegionSettleUnwind { .. } => {}
                    // These operations only observe their operands and produce plain values.
                    InstKind::ValCompare { .. } | InstKind::ArrayLen(_) | InstKind::MapLen(_)
                    | InstKind::MapHas { .. } | InstKind::VariantTag(_) | InstKind::ObjPending { .. }
                    | InstKind::Const(_) | InstKind::IntBinary { .. } | InstKind::IntUnary { .. }
                    | InstKind::IntCompare { .. } | InstKind::BoolNot(_) | InstKind::BoolCompare { .. }
                    | InstKind::StrConcat { .. } | InstKind::StrCompare { .. } | InstKind::Str { .. }
                    | InstKind::ArrayJoin { .. } | InstKind::IntAbs(_) | InstKind::IntSign(_)
                    | InstKind::ByteBinary { .. } | InstKind::ByteNot(_) | InstKind::ByteFrom { .. }
                    | InstKind::ByteCompare { .. } | InstKind::CharCompare { .. }
                    | InstKind::EnumCase { .. } | InstKind::EnumCompare { .. }
                    | InstKind::Float { .. } | InstKind::FloatCompare { .. } | InstKind::FloatSign(_)
                    | InstKind::Rat { .. } | InstKind::RatCompare { .. } | InstKind::RatSign(_)
                    | InstKind::RatPart { .. } | InstKind::RangeNew { .. } | InstKind::RangePart { .. }
                    | InstKind::Bits { .. } => {}
                    // Mutation and error chains need ownership across scopes.
                    InstKind::ObjInit { .. } | InstKind::OnceStore { .. }
                    | InstKind::FailCaused { .. } | InstKind::FailSecondary { .. } => {
                        let reason = format!("other:{}", name(&inst.kind));
                        flow.unknown(&operands, result, &reason);
                        site = Some(name(&inst.kind));
                    }
                }
                if !matches!(inst.kind, InstKind::Await(_) | InstKind::FutSpawn(_)
                    | InstKind::Fut { .. } | InstKind::WorkerSpawn { .. } | InstKind::WorkerBind { .. }
                    | InstKind::RegionEnter | InstKind::RegionAdopt { .. } | InstKind::RegionTrack { .. }
                    | InstKind::RegionTransfer { .. } | InstKind::RegionLeave { .. }
                    | InstKind::RegionSettle { .. } | InstKind::RegionUnwind { .. }
                    | InstKind::RegionSettleUnwind { .. })
                {
                    for &operand in &operands {
                        if matches!(function.values[operand.0], Type::Future(_)) {
                            flow.sink(operand, "future");
                        }
                    }
                }
                if let (Some(value), Some(kind)) = (result, site) {
                    flow.facts[value.0].insert(value.0, DIRECT);
                    flow.sites.insert(value.0, (block_index, instruction, kind));
                }
            }
            for target in block.term.kind.targets().into_iter().chain(block.unwind.as_ref()) {
                for (&argument, &parameter) in target.args.iter().zip(&function.blocks[target.block.0].params) {
                    if !plain.ty(&function.values[parameter.0]) {
                        flow.edge(argument, parameter, EdgeKind::Alias);
                    }
                }
            }
            match &block.term.kind {
                TermKind::Ret(Some(value)) => flow.returns.push(value.0),
                TermKind::Throw(value) | TermKind::Raise(value) => flow.sink(*value, "other:throw"),
                TermKind::ArrayLookup { array: from, found, .. }
                | TermKind::MapLookup { map: from, found, .. } => {
                    if let Some(&to) = function.blocks[found.block.0].params.last()
                        && !plain.ty(&function.values[to.0])
                    {
                        flow.edge(*from, to, EdgeKind::Read);
                    }
                }
                TermKind::Catch { handler, .. } => {
                    if let Some(&to) = function.blocks[handler.block.0].params.last()
                        && !plain.ty(&function.values[to.0])
                    {
                        flow.facts[to.0].insert(to.0, DIRECT);
                        flow.opaque.insert(to.0);
                        flow.sink(to, "other:catch");
                    }
                }
                _ => {}
            }
        }
        for value in function.params.iter().map(|value| value.0).chain(flow.sites.keys().copied()).collect::<Vec<_>>() {
            for &destroy in &destructors[&function.values[value]] {
                if let Some(receiver) = summaries[destroy].parameters.first() {
                    for reason in &receiver.reasons {
                        flow.sink(ValueId(value), reason);
                    }
                }
            }
        }
        flow.solve();
        flow
    }

    fn call(&mut self, callee: &Function, summary: &Summary, arguments: &[ValueId], result: Option<ValueId>) -> bool {
        if let Some(reason) = opaque_reason(callee) {
            self.unknown(arguments, result, &reason);
            return true;
        }
        for (&argument, escape) in arguments.iter().zip(&summary.parameters) {
            for reason in &escape.reasons {
                self.sink(argument, reason);
            }
            if let Some(to) = result {
                if escape.result { self.edge(argument, to, EdgeKind::Alias); }
                if escape.result_field { self.edge(argument, to, EdgeKind::Store); }
                if escape.result_part { self.edge(argument, to, EdgeKind::Read); }
            }
        }
        if let Some(to) = result {
            if summary.opaque_result {
                self.facts[to.0].insert(to.0, DIRECT);
                self.opaque.insert(to.0);
            }
            for reason in &summary.result_reasons {
                self.sink(to, reason);
            }
        }
        summary.fresh || summary.opaque_result
    }

    fn edge(&mut self, from: ValueId, to: ValueId, kind: EdgeKind) {
        self.edges[from.0].push((to.0, kind));
    }

    fn sink(&mut self, value: ValueId, reason: &str) {
        self.sinks.push((value.0, reason.to_string()));
    }

    fn unknown(&mut self, operands: &[ValueId], result: Option<ValueId>, reason: &str) {
        for &operand in operands {
            self.sink(operand, reason);
        }
        if let Some(result) = result {
            self.sink(result, reason);
        }
    }

    fn solve(&mut self) {
        let mut queue: VecDeque<usize> = (0..self.facts.len()).filter(|&id| !self.facts[id].is_empty()).collect();
        let mut queued: Vec<bool> = self.facts.iter().map(|facts| !facts.is_empty()).collect();
        while let Some(from) = queue.pop_front() {
            queued[from] = false;
            for &(to, kind) in &self.edges[from] {
                let mut changed = false;
                for (origin, bits) in self.facts[from].clone() {
                    let bits = match kind {
                        EdgeKind::Alias => bits,
                        EdgeKind::Store => (if bits & (DIRECT | INSIDE) != 0 { INSIDE } else { 0 }) | (bits & (PART | BORROW)),
                        EdgeKind::Read | EdgeKind::Contents => PART,
                        EdgeKind::Borrow => (if bits & (DIRECT | INSIDE | BORROW) != 0 { BORROW } else { 0 }) | (bits & PART),
                    };
                    let previous = self.facts[to].entry(origin).or_default();
                    changed |= *previous | bits != *previous;
                    *previous |= bits;
                }
                if changed && !queued[to] {
                    queued[to] = true;
                    queue.push_back(to);
                }
            }
        }
        for &value in &self.returns {
            for (&origin, &bits) in &self.facts[value] {
                self.effects[origin].result |= bits & DIRECT != 0;
                self.effects[origin].result_field |= bits & (INSIDE | BORROW) != 0;
                self.effects[origin].result_part |= bits & PART != 0;
            }
        }
        let mut processed = 0;
        loop {
            while processed < self.sinks.len() {
                let (value, reason) = &self.sinks[processed];
                for (&origin, &bits) in &self.facts[*value] {
                    self.effects[origin].reasons.insert(if bits == PART { "child".to_string() } else { reason.clone() });
                }
                processed += 1;
            }
            let escaped: Vec<_> = self.deferred.iter().filter(|(origin, _)| self.effects[**origin].escapes())
                .map(|(&origin, reason)| (origin, reason.clone())).collect();
            for (origin, reason) in escaped {
                self.deferred.remove(&origin);
                self.sink(ValueId(origin), &reason);
            }
            // An escaped future no longer has a locally owned result.
            for &(future, result) in &self.awaited {
                for reason in &self.effects[future].reasons {
                    let sink = (result, reason.clone());
                    if !self.sinks.contains(&sink) { self.sinks.push(sink); }
                }
            }
            if processed == self.sinks.len() { break; }
        }
    }

    fn summary(&self, function: &Function) -> Summary {
        let mut summary = Summary {
            parameters: function.params.iter().map(|value| self.effects[value.0].clone()).collect(),
            ..Summary::default()
        };
        for &value in &self.returns {
            for (&origin, &bits) in &self.facts[value] {
                if bits == PART && !function.params.contains(&ValueId(origin)) {
                    summary.opaque_result = true;
                    summary.result_reasons.insert("child".to_string());
                } else {
                    summary.fresh |= self.sites.contains_key(&origin);
                    summary.opaque_result |= self.opaque.contains(&origin);
                    summary.result_reasons.extend(self.effects[origin].reasons.iter().cloned());
                }
            }
        }
        summary
    }

    fn owner(&self, value: usize, containers: Vec<usize>) -> Owner {
        let effect = &self.effects[value];
        if !effect.reasons.is_empty() || effect.result_part {
            let mut reasons = effect.reasons.clone();
            if effect.result_part { reasons.insert("child".to_string()); }
            return Owner::Unknown(reasons);
        }
        if effect.result {
            return Owner::Result;
        }
        if !containers.is_empty() {
            // An unknown container cannot provide a known aggregate owner.
            let reasons: BTreeSet<String> = containers.iter().flat_map(|&id| self.effects[id].reasons.iter().cloned()).collect();
            if !reasons.is_empty() {
                return Owner::Unknown(reasons);
            }
            return Owner::Aggregate {
                owner: if effect.result_field { AggregateOwner::Result } else { AggregateOwner::Scope },
                containers,
            };
        }
        if effect.result_field {
            // A callee can return an aggregate containing only borrowed inputs.
            Owner::Aggregate { owner: AggregateOwner::Result, containers }
        } else {
            Owner::Scope
        }
    }

    fn report(&self, index: usize, function: &Function, plain: &CensusPlain, parameters: Vec<Escape>) -> FunctionReport {
        let mut report = FunctionReport {
            function: index, symbol: function.symbol.clone(), parameters,
            creations: Vec::new(), region_ops: Vec::new(), totals: Totals::default(),
        };
        let mut containers: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        let mut borrows: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for facts in &self.facts {
            for (&container, &bits) in facts {
                if bits & DIRECT == 0 || !self.sites.contains_key(&container) {
                    continue;
                }
                for (&value, &bits) in facts {
                    if value != container && bits & INSIDE != 0 {
                        containers.entry(value).or_default().insert(container);
                    }
                    if value != container && bits & (PART | BORROW) != 0 {
                        borrows.entry(container).or_default().insert(value);
                    }
                }
            }
        }
        let owners: BTreeMap<usize, Owner> = self.sites.keys().map(|&value| {
            (value, self.owner(value, containers.remove(&value).unwrap_or_default().into_iter().collect()))
        }).collect();
        for (&value, &(block, instruction, ref kind)) in &self.sites {
            let owner = owners[&value].clone();
            match &owner {
                Owner::Scope => report.totals.scope += 1,
                Owner::Result => report.totals.result += 1,
                Owner::Aggregate { .. } => report.totals.aggregate += 1,
                Owner::Unknown(reasons) => {
                    report.totals.unknown += 1;
                    for reason in reasons {
                        *report.totals.reasons.entry(reason.clone()).or_default() += 1;
                    }
                }
            }
            let borrowed_contents = borrows.remove(&value).unwrap_or_default().into_iter().collect();
            report.creations.push(Creation { block, instruction, value, kind: kind.clone(), owner, borrowed_contents });
        }
        let members = region_members(function);
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction, inst) in block.insts.iter().enumerate() {
                let mut values = BTreeSet::new();
                let value = match inst.kind {
                    InstKind::RegionAdopt { value, .. } | InstKind::RegionTrack { value, .. }
                    | InstKind::RegionTransfer { value, .. } => value,
                    InstKind::RegionLeave { region, keep: Some(value) }
                    | InstKind::RegionSettle { region, keep: Some(value) } => {
                        values.extend(members[region.0].iter().copied());
                        value
                    }
                    _ => continue,
                };
                values.insert(value.0);
                let statically_owned = values.iter().all(|&value| plain.ty(&function.values[value])
                    || (!self.facts[value].is_empty() && self.facts[value].iter().all(|(origin, bits)| {
                        *bits & PART == 0 && owners.get(origin).is_some_and(|owner| !matches!(owner, Owner::Unknown(_)))
                    })));
                if statically_owned {
                    report.totals.region_static += 1;
                } else {
                    report.totals.region_dynamic += 1;
                }
                report.region_ops.push(RegionOp {
                    block: block_index, instruction, kind: name(&inst.kind),
                    values: values.into_iter().collect(), statically_owned,
                });
            }
        }
        report
    }
}

/// Closing a region also concerns its contents, not just its keep operand.
/// Transfers are not subtracted: mutually exclusive paths may retain a value.
fn region_members(function: &Function) -> Vec<BTreeSet<usize>> {
    fn root(parents: &[usize], mut value: usize) -> usize {
        while parents[value] != value {
            value = parents[value];
        }
        value
    }
    let mut parents: Vec<usize> = (0..function.values.len()).collect();
    for block in &function.blocks {
        for target in block.term.kind.targets().into_iter().chain(block.unwind.as_ref()) {
            for (&arg, &param) in target.args.iter().zip(&function.blocks[target.block.0].params) {
                if function.values[param.0] == Type::Region {
                    let from = root(&parents, arg.0);
                    let to = root(&parents, param.0);
                    parents[to] = from;
                }
            }
        }
    }
    let mut members = vec![BTreeSet::new(); function.values.len()];
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        match inst.kind {
            InstKind::RegionAdopt { region, value } | InstKind::RegionTrack { region, value } => {
                members[root(&parents, region.0)].insert(value.0);
            }
            InstKind::RegionTransfer { from, to, value } => {
                members[root(&parents, from.0)].insert(value.0);
                members[root(&parents, to.0)].insert(value.0);
            }
            _ => {}
        }
    }
    (0..function.values.len()).map(|value| members[root(&parents, value)].clone()).collect()
}

fn name(kind: &InstKind) -> String {
    format!("{kind:?}").split([' ', '(', '{']).next().expect("instruction name").to_ascii_lowercase()
}

// The compiler has no JSON dependency. Escape controls as well as quotes.
fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0}'..='\u{1f}' => out.push_str(&format!("\\u{:04x}", character as u32)),
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}

fn reasons_json(reasons: &BTreeSet<String>) -> String {
    format!("[{}]", reasons.iter().map(|reason| quoted(reason)).collect::<Vec<_>>().join(","))
}

impl Totals {
    fn json(&self) -> String {
        let reasons = self.reasons.iter().map(|(reason, count)| format!("{}:{count}", quoted(reason))).collect::<Vec<_>>().join(",");
        format!("{{\"scope\":{},\"result\":{},\"aggregate\":{},\"unknown\":{},\"reasons\":{{{reasons}}},\"region_ops\":{{\"static\":{},\"dynamic\":{}}}}}",
            self.scope, self.result, self.aggregate, self.unknown, self.region_static, self.region_dynamic)
    }
}

impl Report {
    /// Stable, dependency-free JSON, including all sites and parameter summaries.
    pub fn to_json(&self) -> String {
        let functions = self.functions.iter().map(|function| {
            let parameters = function.parameters.iter().map(|parameter| format!(
                "{{\"escapes\":{},\"result\":{},\"result_field\":{},\"result_part\":{},\"reasons\":{}}}",
                parameter.escapes(), parameter.result, parameter.result_field, parameter.result_part, reasons_json(&parameter.reasons)
            )).collect::<Vec<_>>().join(",");
            let creations = function.creations.iter().map(|site| {
                let owner = match &site.owner {
                    Owner::Scope => "\"class\":\"scope\"".to_string(),
                    Owner::Result => "\"class\":\"result\"".to_string(),
                    Owner::Aggregate { owner, containers } => format!("\"class\":\"aggregate\",\"owner\":{},\"containers\":{containers:?}", quoted(match owner {
                        AggregateOwner::Scope => "scope", AggregateOwner::Result => "result",
                    })),
                    Owner::Unknown(reasons) => format!("\"class\":\"unknown\",\"reasons\":{}", reasons_json(reasons)),
                };
                format!("{{\"block\":{},\"instruction\":{},\"value\":{},\"kind\":{},\"borrowed_contents\":{:?},{owner}}}", site.block, site.instruction, site.value, quoted(&site.kind), site.borrowed_contents)
            }).collect::<Vec<_>>().join(",");
            let regions = function.region_ops.iter().map(|op| format!(
                "{{\"block\":{},\"instruction\":{},\"kind\":{},\"values\":{:?},\"static\":{}}}",
                op.block, op.instruction, quoted(&op.kind), op.values, op.statically_owned
            )).collect::<Vec<_>>().join(",");
            format!("{{\"function\":{},\"symbol\":{},\"totals\":{},\"parameters\":[{parameters}],\"creations\":[{creations}],\"region_ops\":[{regions}]}}",
                function.function, quoted(&function.symbol), function.totals.json())
        }).collect::<Vec<_>>().join(",");
        let split = |genuine| self.totals.reasons.iter().filter(|(reason, _)| {
            matches!(reason.as_str(), "atomic" | "channel" | "spawn" | "future" | "any" | "child" | "closure" | "indirect-call") == genuine
        }).map(|(reason, count)| format!("{}:{count}", quoted(reason))).collect::<Vec<_>>().join(",");
        let genuine = split(true);
        let model = split(false);
        format!("{{\"reasons_genuine\":{{{genuine}}},\"reasons_model\":{{{model}}},\"schema\":1,\"site_basis\":\"instructions-including-fresh-call-results\",\"totals\":{},\"top_unknown\":{:?},\"functions\":[{functions}]}}\n", self.totals.json(), self.top_unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_json_escapes_controls() {
        assert_eq!(quoted("\"\\\n\t\0雪"), "\"\\\"\\\\\\u000a\\u0009\\u0000雪\"");
    }

    #[test]
    fn ownership_components_are_callee_first() {
        assert_eq!(components(&[vec![1], vec![2], vec![1, 3], vec![]]), vec![vec![3], vec![1, 2], vec![0]]);
    }
}
