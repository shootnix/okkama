//! Closed-world parameter borrowing; runtime regions remain authoritative (#555).

use std::collections::{BTreeSet, VecDeque};

use super::{Attr, FuncId, Function, InstKind, Module, Target, TermKind, Type, ValueId, cfg};

/// Remove proven redundant root adopts, returning their count and per-function counts.
pub fn elide_param_adopts(module: &mut Module, removed: &mut Vec<usize>) -> usize {
    let plain = Plain::of(module);
    let mut keep: Vec<bool> = module.functions.iter().map(|function| {
        function.blocks.is_empty() || function.is_async() || function.extern_symbol().is_some()
            || function.intrinsic().is_some() || function.attrs.iter().any(|attr| {
                matches!(attr, Attr::Entry | Attr::Destroy(_) | Attr::Lambda { .. })
            })
    }).collect();
    for entry in module.entry.iter().chain(module.globals.iter().map(|global| &global.init))
        .chain(module.tests.iter().flatten().map(|(_, function)| function))
        .chain(module.objects.iter().filter_map(|object| object.destroy.as_ref()))
    {
        keep[entry.0] = true;
    }
    for function in &module.functions {
        for helper in function.intrinsic_helpers() {
            keep[helper.0] = true;
        }
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            // Worker entries and bound actions also enter through closures (#555).
            if let InstKind::FnClosure { func, .. } = inst.kind {
                keep[func.0] = true;
            }
        }
    }
    let mut seen = vec![false; module.functions.len()];
    let mut owned: Vec<Vec<bool>> = module.functions.iter()
        .map(|function| vec![true; function.params.len()]).collect();
    for function in &module.functions {
        let inputs = inputs(function, &plain);
        for (index, block) in function.blocks.iter().enumerate() {
            let mut state = inputs.get(index).and_then(Clone::clone);
            for inst in &block.insts {
                let (callees, arguments) = calls(module, function, &inst.kind);
                for callee in callees {
                    seen[callee.0] = true;
                    for (position, safe) in owned[callee.0].iter_mut().enumerate() {
                        *safe &= arguments.get(position).is_some_and(|value| {
                            state.as_ref().is_some_and(|state| state.owned[value.0].is_some())
                        });
                    }
                }
                if let Some(state) = &mut state {
                    state.step(inst, function, &plain);
                }
            }
        }
    }
    removed.clear();
    removed.resize(module.functions.len(), 0);
    for (index, function) in module.functions.iter_mut().enumerate() {
        if keep[index] || !seen[index] {
            continue;
        }
        let Some(root) = root_region(function) else { continue };
        let mut prologue = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut block = 0;
        'prefix: while visited.insert(block) {
            for (position, inst) in function.blocks[block].insts.iter().enumerate() {
                match inst.kind {
                    InstKind::RegionEnter if inst.result == Some(root) => {}
                    InstKind::Undef => {}
                    InstKind::RegionAdopt { region, value }
                        if region == root && function.params.contains(&value) => {
                            prologue.insert((block, position));
                        }
                    _ => break 'prefix,
                }
            }
            match &function.blocks[block].term.kind {
                TermKind::Br(target) => block = target.block.0,
                _ => break,
            }
        }
        for (block, body) in function.blocks.iter_mut().enumerate() {
            let mut position = 0;
            body.insts.retain(|inst| {
                let in_prologue = prologue.contains(&(block, position));
                position += 1;
                let redundant = matches!(inst.kind, InstKind::RegionAdopt { region, value }
                    if region == root && function.params.iter().position(|param| *param == value)
                        .is_some_and(|param| owned[index][param]))
                    // Only the prologue adopt, never a later re-adoption (#555).
                    && in_prologue;
                removed[index] += usize::from(redundant);
                !redundant
            });
        }
    }
    removed.iter().sum()
}

fn calls(module: &Module, function: &Function, kind: &InstKind) -> (Vec<FuncId>, Vec<ValueId>) {
    match kind {
        InstKind::Call { func, args } => (vec![*func], args.clone()),
        InstKind::CallRole { receiver, slot, args } => {
            let Type::Role(role) = function.values[receiver.0] else { return (vec![], vec![]) };
            let callees = module.objects.iter().flat_map(|object| &object.does)
                .filter(|witness| witness.role == role)
                .filter_map(|witness| witness.slots.get(*slot).copied()).collect();
            (callees, std::iter::once(*receiver).chain(args.iter().copied()).collect())
        }
        InstKind::CallCap { cap, field } => {
            let Type::Cap(id) = function.values[cap.0] else { return (vec![], vec![]) };
            (vec![module.caps[id.0].fields[*field].1], vec![*cap])
        }
        _ => (vec![], vec![]),
    }
}

fn root_region(function: &Function) -> Option<ValueId> {
    function.blocks.first()?.insts.iter()
        .find(|inst| matches!(inst.kind, InstKind::RegionEnter)).and_then(|inst| inst.result)
}

fn closed(kind: &InstKind) -> Option<ValueId> {
    match kind {
        InstKind::RegionLeave { region, .. } | InstKind::RegionSettle { region, .. }
        | InstKind::RegionUnwind { region, .. } | InstKind::RegionSettleUnwind { region, .. } => Some(*region),
        _ => None,
    }
}

// A proof depends on every local region that might own part of the graph (#555).
#[derive(Clone, PartialEq, Eq)]
struct State {
    owned: Vec<Option<BTreeSet<usize>>>,
    open: BTreeSet<usize>,
}

impl State {
    fn meet(&mut self, other: &Self) {
        self.open.retain(|region| other.open.contains(region));
        for (left, right) in self.owned.iter_mut().zip(&other.owned) {
            match (left.as_mut(), right) {
                (Some(left), Some(right)) => left.extend(right),
                _ => *left = None,
            }
        }
    }

    fn step(&mut self, inst: &super::Inst, function: &Function, plain: &Plain) {
        if let Some(result) = inst.result {
            self.owned[result.0] = if plain.ty(&function.values[result.0]) {
                Some(BTreeSet::new())
            } else {
                let source = match inst.kind {
                    InstKind::ObjGet { object, .. } => Some(object),
                    InstKind::VariantGet { value, .. } | InstKind::RoleNarrow { value, .. } => Some(value),
                    InstKind::ArrayGet { array, .. } => Some(array),
                    InstKind::MapGet { map, .. } => Some(map),
                    InstKind::Cvt(value) | InstKind::RoleWrap(value) | InstKind::VariantOf(value)
                    | InstKind::Retag(value) | InstKind::MapRetag(value) | InstKind::SuccTake(value)
                    | InstKind::CapMake(value) => Some(value),
                    _ => None,
                };
                source.and_then(|value| self.owned[value.0].clone())
            };
        }
        match inst.kind {
            InstKind::RegionEnter => {
                if let Some(region) = inst.result { self.open.insert(region.0); }
            }
            InstKind::RegionAdopt { region, value } if self.open.contains(&region.0) => {
                // Another open region may already own a subset; adopt does not move it (#555).
                if self.owned[value.0].is_none() {
                    self.owned[value.0] = Some(self.open.clone());
                }
            }
            InstKind::RegionTransfer { from, to, value } if self.open.contains(&to.0) => {
                // Transfer does not adopt free identities (#555).
                if let Some(dependencies) = &mut self.owned[value.0]
                    && dependencies.remove(&from.0)
                {
                    dependencies.insert(to.0);
                }
            }
            // Stores and future progress can expose fresh children through aliases (#555).
            InstKind::ObjInit { .. } | InstKind::OnceStore { .. } | InstKind::Await(_)
            | InstKind::FutSpawn(_) | InstKind::WorkerSpawn { .. } => {
                self.forget(function, plain);
            }
            _ => {}
        }
        if let Some(region) = closed(&inst.kind) {
            if self.open.remove(&region.0) {
                for proof in &mut self.owned {
                    if proof.as_ref().is_some_and(|regions| regions.contains(&region.0)) {
                        *proof = None;
                    }
                }
            } else {
                // Region block parameters are deliberately not resolved in this version (#555).
                self.forget(function, plain);
                self.open.clear();
            }
        }
    }

    fn forget(&mut self, function: &Function, plain: &Plain) {
        for (proof, ty) in self.owned.iter_mut().zip(&function.values) {
            *proof = plain.ty(ty).then(BTreeSet::new);
        }
    }

    fn edge(&self, target: &Target, extra: Option<ValueId>, function: &Function, plain: &Plain) -> Self {
        let mut next = self.clone();
        for (index, param) in function.blocks[target.block.0].params.iter().enumerate() {
            next.owned[param.0] = if plain.ty(&function.values[param.0]) {
                Some(BTreeSet::new())
            } else {
                target.args.get(index).copied().or_else(|| {
                    (index == target.args.len()).then_some(extra).flatten()
                }).and_then(|arg| self.owned[arg.0].clone())
            };
        }
        next
    }
}

fn inputs(function: &Function, plain: &Plain) -> Vec<Option<State>> {
    if function.blocks.is_empty() || cfg::Dominators::compute(function).is_err() {
        return Vec::new();
    }
    let mut entry = State { owned: vec![None; function.values.len()], open: BTreeSet::new() };
    entry.forget(function, plain);
    // Prologue adopts establish parameter proofs before body calls. Removing them
    // together is sound by induction over calls; destroy/stub inputs stay fresh (#555).
    let mut inputs = vec![None; function.blocks.len()];
    inputs[0] = Some(entry);
    let mut queue = VecDeque::from([0]);
    let mut queued = vec![false; function.blocks.len()];
    queued[0] = true;
    while let Some(index) = queue.pop_front() {
        queued[index] = false;
        let block = &function.blocks[index];
        let mut state = inputs[index].clone().expect("queued reachable block");
        let mut unwind = block.unwind.as_ref().map(|_| state.clone());
        for inst in &block.insts {
            state.step(inst, function, plain);
            // An unwind can follow any instruction, including a region close (#555).
            if let Some(unwind) = &mut unwind { unwind.meet(&state); }
        }
        let mut edges = Vec::new();
        for target in block.term.kind.targets() {
            let extra = match &block.term.kind {
                TermKind::ArrayLookup { array, found, .. } if std::ptr::eq(target, found) => Some(*array),
                TermKind::MapLookup { map, found, .. } if std::ptr::eq(target, found) => Some(*map),
                _ => None,
            };
            edges.push((target.block.0, state.edge(target, extra, function, plain)));
        }
        if let (Some(target), Some(unwind)) = (&block.unwind, unwind) {
            edges.push((target.block.0, unwind.edge(target, None, function, plain)));
        }
        for (next, state) in edges {
            let changed = if let Some(previous) = &mut inputs[next] {
                let before = previous.clone();
                previous.meet(&state);
                *previous != before
            } else {
                inputs[next] = Some(state);
                true
            };
            if changed && !queued[next] {
                queue.push_back(next);
                queued[next] = true;
            }
        }
    }
    inputs
}

// Unlike region elision, deferred and opaque graphs are never plain here (#555).
struct Plain {
    objects: Vec<bool>,
    roles: Vec<bool>,
    variants: Vec<Vec<usize>>,
}

impl Plain {
    fn of(module: &Module) -> Self {
        let mut plain = Self {
            objects: vec![false; module.objects.len()], roles: vec![false; module.roles.len()],
            variants: module.variants.iter().map(|variant| variant.cases.iter().map(|(_, id)| id.0).collect()).collect(),
        };
        loop {
            let before = (plain.objects.clone(), plain.roles.clone());
            for (index, object) in module.objects.iter().enumerate() {
                plain.objects[index] = object.destroy.is_none() && object.fields.iter().all(|field| plain.ty(&field.ty));
            }
            for (index, role) in plain.roles.iter_mut().enumerate() {
                *role = module.objects.iter().enumerate().all(|(id, object)| {
                    !object.does.iter().any(|witness| witness.role.0 == index) || plain.objects[id]
                });
            }
            if before == (plain.objects.clone(), plain.roles.clone()) { return plain; }
        }
    }

    fn ty(&self, ty: &Type) -> bool {
        match ty {
            Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char
            | Type::Str | Type::Never | Type::Enum(_) | Type::Range => true,
            Type::Object(id) | Type::Successor(id) => self.objects[id.0],
            Type::Role(id) => self.roles[id.0],
            Type::Variant(id) => self.variants[id.0].iter().all(|id| self.objects[*id]),
            Type::Array(element) => self.ty(element),
            Type::Map(key, value) => self.ty(key) && self.ty(value),
            _ => false,
        }
    }
}
