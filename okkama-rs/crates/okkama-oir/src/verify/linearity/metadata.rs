use super::*;

pub(super) fn linear(module: &Module, body: &Body, value: ValueId) -> bool {
    body.values.get(value).ok().and_then(|v| module.types.get(v.ty).ok()).is_some_and(|ty| {
        matches!(ty, Type::Builder(_) | Type::MapBuilder(..) | Type::Frame(_))
            || matches!(ty, Type::Service(service) if service.is_linear())
    })
}
pub(super) fn builder(module: &Module, body: &Body, value: ValueId) -> bool {
    body.values.get(value).ok().and_then(|v| module.types.get(v.ty).ok())
        .is_some_and(|ty| matches!(ty, Type::Builder(_) | Type::MapBuilder(..)))
}
pub(super) fn scope(module: &Module, body: &Body, value: ValueId) -> Option<ScopeId> {
    match module.types.get(body.values.get(value).ok()?.ty).ok()? {
        Type::Service(ServiceType::Scope(scope)) => Some(*scope),
        _ => None,
    }
}
pub(super) fn successor(module: &Module, body: &Body, value: ValueId) -> bool {
    body.values.get(value).ok().and_then(|v| module.types.get(v.ty).ok())
        .is_some_and(|ty| matches!(ty, Type::Successor(_)))
}

pub(super) fn managed_types(module: &Module, state: &mut State<'_>) -> Vec<bool> {
    if !state.array(module.types.len(), 1) {
        return Vec::new();
    }
    let mut managed = vec![false; module.types.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for (id, ty) in module.types.iter() {
            if !state.step(1) {
                return managed;
            }
            let child = |id: TypeId| managed.get(id.as_u32() as usize).copied().unwrap_or(true);
            let next = match ty {
                Type::Object { decl, .. } => module.declarations.get(*decl).is_ok_and(|d| match &d.kind {
                    DeclarationKind::Object { fields, destroy, builtin, .. } => {
                        *builtin || destroy.is_some() || fields.iter().any(|f| child(f.ty))
                    }
                    _ => true,
                }),
                Type::Variant(decl) => module.declarations.get(*decl).is_ok_and(|d| match &d.kind {
                    DeclarationKind::Variant { cases, .. } => cases.iter().any(|case| {
                        module.declarations.get(case.layout).is_ok_and(|layout| match &layout.kind {
                            DeclarationKind::Object { fields, destroy, .. } => {
                                destroy.is_some() || fields.iter().any(|f| child(f.ty))
                            }
                            _ => true,
                        })
                    }),
                    _ => true,
                }),
                Type::Successor(ty) | Type::Array(ty) | Type::Builder(ty) => child(*ty),
                Type::Map(k, v) | Type::MapBuilder(k, v) => child(*k) || child(*v),
                Type::Any | Type::Role { .. } | Type::Cap { .. } | Type::Fn(_) | Type::Future(_)
                | Type::Parameter(_) => true,
                _ => false,
            };
            if next && !managed[id.as_u32() as usize] {
                managed[id.as_u32() as usize] = true;
                changed = true;
            }
        }
    }
    managed
}

pub(super) fn value_facts(
    module: &Module, function: FuncId, body: &Body, managed: &[bool], state: &mut State<'_>,
) {
    for (value, data) in body.values.iter() {
        if !state.step(1) {
            return;
        }
        state.at(L::Value { function, value }, None);
        let Ok(ty) = module.types.get(data.ty) else {
            continue;
        };
        if matches!(ty, Type::Service(_) | Type::Descriptor | Type::Frame(_)
            | Type::Builder(_) | Type::MapBuilder(..)) { continue; }
        let needs = managed.get(data.ty.as_u32() as usize).copied().unwrap_or(true);
        let valid = match &data.facts {
            ValueFacts::Managed { .. } => true,
            ValueFacts::Plain { certificate: PlainCertificate::Scalar } => matches!(ty,
                Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char
                | Type::Str | Type::Range | Type::Enum(_) | Type::Never),
            ValueFacts::Plain { certificate: PlainCertificate::ClosedGraph { ty } } => *ty == data.ty && !needs,
            // Dictionary contracts are a commit-4 obligation; no scalar/plain
            // certificate is inferred from an empty ownership selection.
            ValueFacts::Plain { certificate: PlainCertificate::Descriptor(_) } => false,
            ValueFacts::Unelaborated => false,
            ValueFacts::Service => false,
        };
        if !valid {
            state.error(R::B12, "managed value needs full dependencies or a checked plain certificate");
        }
    }
}

impl Engine<'_, '_> {
    pub(super) fn identity(&self, flow: &Flow, value: ValueId) -> Origin {
        flow.identities[value.as_u32() as usize].unwrap_or(Origin::External(value))
    }
    pub(super) fn field(&self, flow: &Flow, value: ValueId, field: FieldRef) -> BTreeSet<Origin> {
        let identity = self.identity(flow, value);
        if let Some((refs, _)) = flow.fields.get(&(identity, field.declaration, field.index)) {
            return refs.clone();
        }
        if self.module.declarations.get(field.declaration).is_ok_and(|declaration| match &declaration.kind {
            DeclarationKind::Object { fields, .. } => fields.get(field.index as usize)
                .is_some_and(|field| !self.managed.get(field.ty.as_u32() as usize).copied().unwrap_or(true)),
            _ => false,
        }) { return BTreeSet::new(); }
        let root = match identity { Origin::External(value) => value, _ => value };
        BTreeSet::from([Origin::Projection(root, ProjectionKind::Field(field.declaration, field.index))])
    }
    pub(super) fn possible(&mut self, flow: &Flow, keys: &BTreeSet<Origin>) -> BTreeSet<Origin> {
        let mut possible = BTreeSet::new();
        let mut pending: Vec<_> = keys.iter().copied().collect();
        while let Some(key) = pending.pop() {
            if !self.state.step(1) {
                break;
            }
            let refs = if let Some(refs) = flow.views.get(&key) { Some(refs) }
                else if let Origin::Projection(value, _) | Origin::RetiredProjection(value, _) = key {
                    Some(&flow.refs[value.as_u32() as usize])
                } else { None };
            if let Some(refs) = refs {
                for &dependency in refs {
                    if possible.insert(dependency) {
                        pending.push(dependency);
                    }
                }
            }
        }
        possible
    }
    fn may_alias(&mut self, flow: &Flow, a: Origin, b: Origin) -> bool {
        if !self.state.step(1) {
            return true;
        }
        if a == b {
            return true;
        }
        let plain_external = |key| matches!(key, Origin::External(_));
        if plain_external(a) && plain_external(b) {
            return true;
        }
        let left = self.possible(flow, &BTreeSet::from([a]));
        let right = self.possible(flow, &BTreeSet::from([b]));
        if left.contains(&b) || right.contains(&a) || !left.is_disjoint(&right) {
            return true;
        }
        let external = |keys: &BTreeSet<Origin>| keys.iter().any(|key| plain_external(*key));
        (plain_external(a) && external(&right)) || (plain_external(b) && external(&left))
            || (external(&left) && external(&right))
    }
    pub(super) fn dependencies(&mut self, flow: &Flow, root: DepExprId) -> BTreeSet<Origin> {
        let mut result = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            if !self.state.step(1) {
                break;
            }
            if !seen.insert(id) {
                continue;
            }
            match self.body.deps.get(id) {
                Ok(DepExpr::Empty) | Err(_) => {}
                Ok(DepExpr::Key(key)) => {
                    match self.body.keys.get(*key) {
                        Ok(KeyExpr::Created { instance, .. }) => {
                            self.live_use(flow, *instance);
                            result.insert(self.identity(flow, *instance));
                        }
                        Ok(KeyExpr::Runtime(value) | KeyExpr::ServiceOf { value, .. }) => {
                            self.live_use(flow, *value);
                            result.extend(flow.refs[value.as_u32() as usize].iter().copied());
                            if flow.refs[value.as_u32() as usize].is_empty() {
                                result.insert(Origin::External(*value));
                            }
                        }
                        Err(_) => {}
                    }
                }
                Ok(DepExpr::Union(items)) => pending.extend(items),
                Ok(DepExpr::FromSsa(value)) => {
                    self.live_use(flow, *value);
                    if linear(self.module, self.body, *value) {
                        self.error(R::B31, "a packet register cannot anchor a lifetime");
                    }
                    result.extend(flow.refs[value.as_u32() as usize].iter().copied());
                }
                Ok(DepExpr::Retention { value, selection }) => {
                    self.live_use(flow, *value);
                    result.extend(flow.full_deps[value.as_u32() as usize].iter().copied());
                    let (selected, _) = self.selection(flow, *selection);
                    result.extend(selected);
                }
            }
        }
        result
    }
    pub(super) fn selection_terms(
        &mut self, flow: &Flow, root: SelectionId,
    ) -> Vec<(BTreeSet<Origin>, bool, BTreeSet<ValueId>)> {
        let mut terms = Vec::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![(root, false)];
        while let Some((id, fresh)) = pending.pop() {
            if !self.state.step(1) {
                break;
            }
            if !seen.insert((id, fresh)) {
                continue;
            }
            match self.body.selections.get(id) {
                Ok(SelectionExpr::Admissible(inner)) => pending.push((*inner, true)),
                Ok(SelectionExpr::Union(items)) => pending.extend(items.iter().map(|id| (*id, fresh))),
                Ok(selection) => {
                    let (keys, _) = self.selection(flow, id);
                    let labels = match selection {
                        SelectionExpr::Value(value) | SelectionExpr::OwnerGraph(value)
                        | SelectionExpr::ViewOwner(value) | SelectionExpr::Descriptor { value, .. } => {
                            flow.contained[value.as_u32() as usize].clone()
                        }
                        _ => BTreeSet::new(),
                    };
                    terms.push((keys, fresh, labels));
                }
                Err(_) => {}
            }
        }
        terms
    }
    pub(super) fn selection(&mut self, flow: &Flow, root: SelectionId) -> (BTreeSet<Origin>, bool) {
        let mut result = BTreeSet::new();
        let mut fresh = false;
        let mut seen = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            if !self.state.step(1) {
                break;
            }
            if !seen.insert(id) {
                continue;
            }
            let value = match self.body.selections.get(id) {
                Ok(SelectionExpr::Value(v) | SelectionExpr::OwnerGraph(v) | SelectionExpr::ViewOwner(v)) => *v,
                Ok(SelectionExpr::Descriptor { value, .. }) => *value,
                Ok(SelectionExpr::Projection(projection)) => {
                    let key = match projection {
                        Projection::Field { value, field } => {
                            result.extend(self.field(flow, *value, *field));
                            continue;
                        }
                        Projection::Index { value, index } => Origin::Projection(*value, ProjectionKind::Index(*index)),
                        Projection::Slice { value, start, end } => {
                            Origin::Projection(*value, ProjectionKind::Slice(*start, *end))
                        }
                        Projection::Map { value, keys } => Origin::Projection(*value, ProjectionKind::Map(*keys)),
                    };
                    result.insert(key);
                    continue;
                }
                Ok(SelectionExpr::Admissible(inner)) => {
                    fresh = true;
                    pending.push(*inner);
                    continue;
                }
                Ok(SelectionExpr::Union(items)) => {
                    pending.extend(items);
                    continue;
                }
                Err(_) => continue,
            };
            result.extend(flow.refs[value.as_u32() as usize].iter().copied());
        }
        (result, fresh)
    }
    pub(super) fn live_use(&mut self, flow: &Flow, value: ValueId) {
        if flow.lifetimes[value.as_u32() as usize] != 1 {
            self.error(R::B31, "use of a value whose stable dependency has been closed");
        }
    }
    pub(super) fn invalidate(&mut self, before: &Flow, flow: &mut Flow, pieces: &[Piece]) {
        for (value, _) in self.body.values.iter() {
            if before.lifetimes[value.as_u32() as usize] == 1 && self.invalidated(before, value, pieces) {
                flow.lifetimes[value.as_u32() as usize] = 2;
            }
        }
    }
    fn invalidated(&mut self, flow: &Flow, value: ValueId, pieces: &[Piece]) -> bool {
        self.keys_invalidated(flow, &flow.full_deps[value.as_u32() as usize],
            &flow.aliases[value.as_u32() as usize], pieces)
    }
    pub(super) fn keys_invalidated(
        &mut self, flow: &Flow, refs: &BTreeSet<Origin>, aliases: &BTreeSet<ValueId>, pieces: &[Piece],
    ) -> bool {
        refs.iter().any(|key| {
            pieces.iter().any(|piece| {
                if !self.state.step(piece.predicates.len().saturating_add(1)) {
                    return true;
                }
                let retired = matches!(key, Origin::RetiredInstruction(..) | Origin::RetiredInvoke(..)
                    | Origin::RetiredProjection(..) | Origin::RetiredViewInstruction(..)
                    | Origin::RetiredViewInvoke(..));
                let excluded = piece.predicates.iter().any(|(filter, selected)| {
                    !*selected && !filter.fresh && filter.keys.contains(key)
                        && (!retired || (!filter.stale
                            && !filter.labels.is_disjoint(aliases)))
                });
                if excluded {
                    return false;
                }
                if !matches!(piece.origin, Origin::Input(_)) {
                    return self.may_alias(flow, piece.origin, *key);
                }
                piece.predicates.iter().all(|(filter, included)| {
                    !*included || filter.keys.iter().any(|other| self.may_alias(flow, *other, *key))
                })
            })
        })
    }
    pub(super) fn check_deps(&mut self, flow: &mut Flow, value: ValueId, expected: &BTreeSet<Origin>) {
        if let Ok(ValueData { facts: ValueFacts::Managed { deps }, .. }) = self.body.values.get(value) {
            let stated = self.dependencies(flow, *deps);
            if !expected.is_subset(&stated) {
                self.error(R::B32, "value.deps omits part of its retention graph");
            }
            flow.full_deps[value.as_u32() as usize].extend(stated);
        }
    }
}
