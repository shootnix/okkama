use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classification {
    Plain,
    Managed,
    Residual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeClassification {
    pub class: Classification,
    pub certificate: Option<PlainCertificate>,
    pub reachable_resources: BTreeSet<TypeId>,
    pub destroy_entries: BTreeSet<FuncId>,
    pub service_types: BTreeSet<TypeId>,
    pub descriptors: Vec<DescriptorContract>,
    pub unresolved: BTreeSet<TypeId>,
}

impl TypeClassification {
    pub fn may_own(&self) -> bool {
        !self.reachable_resources.is_empty() || !self.service_types.is_empty()
            || !self.unresolved.is_empty()
            || self.descriptors.iter().any(|descriptor| descriptor.managed != DescriptorManaged::Plain)
    }
}

// Terms pair an input type with bindings to already interned terms. In
// Box<Array<T>>, the argument's environment survives shadowing Box's T.
// No input type is added to the module, and recursive graphs share terms.
type ParameterKey = (u8, u32, u32);
type Environment = BTreeMap<ParameterKey, usize>;
fn parameter_key(parameter: TypeParameterRef) -> ParameterKey {
    let (kind, id) = match parameter.owner {
        GenericOwner::Declaration(id) => (0, id.as_u32()),
        GenericOwner::Function(id) => (1, id.as_u32()),
    };
    (kind, id, parameter.index)
}

fn free_parameters(module: &Module, budget: &mut Budget<'_>)
    -> Result<BTreeMap<TypeId, BTreeSet<ParameterKey>>, OwnError> {
    budget.memory(module.types.len() * 128)?;
    let mut free: BTreeMap<_, BTreeSet<_>> = module.types.iter().map(|(ty, _)| (ty, BTreeSet::new())).collect();
    loop {
        let mut changed = false;
        for (id, ty) in module.types.iter() {
            budget.work(1)?;
            let mut children = vec![];
            match ty {
                Type::Parameter(parameter) => {
                    changed |= free.get_mut(&id).unwrap().insert(parameter_key(*parameter));
                }
                Type::Object { args, .. } | Type::Role { args, .. } | Type::Cap { args, .. } => {
                    children.extend(args);
                }
                Type::Successor(ty) | Type::Array(ty) | Type::Builder(ty) | Type::LowCell(ty) => children.push(ty),
                Type::Map(k, v) | Type::MapBuilder(k, v) => children.extend([k, v]),
                Type::Fn(function) => {
                    children.extend(&function.params);
                    children.extend(&function.result);
                }
                Type::Future(result) => children.extend(result),
                _ => {}
            }
            let mut additions = BTreeSet::new();
            for child in children {
                additions.extend(free[child].iter().copied());
            }
            for parameter in additions {
                if free.get_mut(&id).unwrap().insert(parameter) {
                    budget.memory(64)?;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(free)
}

struct Terms<'a> {
    module: &'a Module,
    free: &'a BTreeMap<TypeId, BTreeSet<ParameterKey>>,
    ids: BTreeMap<(TypeId, Environment), usize>,
    nodes: Vec<(TypeId, Environment)>,
}
impl Terms<'_> {
    fn intern(&mut self, ty: TypeId, env: &Environment, budget: &mut Budget<'_>) -> Result<usize, OwnError> {
        budget.work(1)?;
        if let Type::Parameter(parameter) = self.module.types.get(ty).map_err(arena)?
            && let Some(term) = env.get(&parameter_key(*parameter)) {
            return Ok(*term);
        }
        let env: Environment = env.iter().filter(|(key, _)| self.free[&ty].contains(key))
            .map(|(key, term)| (*key, *term)).collect();
        let key = (ty, env);
        if let Some(id) = self.ids.get(&key) {
            return Ok(*id);
        }
        budget.memory(192 + key.1.len() * 128)?;
        let id = self.nodes.len();
        self.nodes.push(key.clone());
        self.ids.insert(key, id);
        Ok(id)
    }
}

pub(super) fn classify(
    module: &Module,
    options: &OwnOptions,
    budget: &mut Budget<'_>,
) -> Result<BTreeMap<TypeId, TypeClassification>, OwnError> {
    budget.memory(options.descriptors.len() * 512)?;
    let mut dictionaries = options.descriptors.clone();
    let mut captures: BTreeMap<TypeId, BTreeSet<TypeId>> = BTreeMap::new();
    let mut origins = BTreeSet::new();
    for (_, function) in module.functions.iter() {
        let Some(body) = &function.body else {
            continue;
        };
        for (_, instruction) in body.insts.iter() {
            budget.work(1)?;
            match &instruction.op {
                Operation::Core(CoreOp::TypeOf { ty, descriptor }) => {
                    if dictionaries.get(ty).is_some_and(|old| old != descriptor) {
                        return Err(budget.fail(OwnDiagnosticKind::InvalidDescriptor, format!(
                            "conflicting dictionaries for {ty:?}",
                        )));
                    }
                    dictionaries.insert(*ty, descriptor.clone());
                }
                Operation::Core(CoreOp::FnClosure { captures: values, ty, .. }) => {
                    origins.insert(*ty);
                    for value in values {
                        captures.entry(*ty).or_default().insert(body.values.get(*value).map_err(arena)?.ty);
                    }
                }
                Operation::Core(CoreOp::FnBind { receiver, ty, .. }
                    | CoreOp::FnBindRole { receiver, ty, .. }) => {
                    origins.insert(*ty);
                    captures.entry(*ty).or_default().insert(body.values.get(*receiver).map_err(arena)?.ty);
                }
                Operation::Core(CoreOp::AnyBox { value, ty, .. }) => {
                    origins.insert(*ty);
                    captures.entry(*ty).or_default().insert(body.values.get(*value).map_err(arena)?.ty);
                }
                _ => {}
            }
        }
    }
    let free = free_parameters(module, budget)?;
    let mut result = BTreeMap::new();
    for (root, _) in module.types.iter() {
        let mut facts = TypeClassification {
            class: Classification::Plain,
            certificate: None,
            reachable_resources: BTreeSet::new(),
            destroy_entries: BTreeSet::new(),
            service_types: BTreeSet::new(),
            descriptors: vec![],
            unresolved: BTreeSet::new(),
        };
        let mut terms = Terms { module, free: &free, ids: BTreeMap::new(), nodes: vec![] };
        let root_term = terms.intern(root, &Environment::new(), budget)?;
        let mut pending = vec![root_term];
        let mut visited = BTreeSet::new();
        while let Some(term) = pending.pop() {
            budget.work(1)?;
            if !visited.insert(term) {
                continue;
            }
            let (ty, mut env) = terms.nodes[term].clone();
            if let Some(descriptor) = dictionaries.get(&ty) {
                module.functions.get(descriptor.cleanup).map_err(arena)?;
                module.summaries.get(descriptor.summary).map_err(arena)?;
                if !facts.descriptors.contains(descriptor) {
                    facts.descriptors.push(descriptor.clone());
                }
                if descriptor.managed != DescriptorManaged::Plain {
                    facts.reachable_resources.insert(ty);
                    facts.destroy_entries.insert(descriptor.cleanup);
                }
                // A residual dictionary covers traversal, settlement and all
                // permitted substitutions. Visible concrete fields are still
                // traversed, so a dictionary cannot hide a known resource.
                if matches!(module.types.get(ty).map_err(arena)?, Type::Parameter(_) | Type::Any | Type::Fn(_)) {
                    continue;
                }
            }
            let mut children = vec![];
            match module.types.get(ty).map_err(arena)? {
                Type::Successor(child) | Type::Array(child) | Type::Builder(child) | Type::LowCell(child) => {
                    children.push(*child);
                }
                Type::Map(key, value) | Type::MapBuilder(key, value) => {
                    children.extend([*key, *value]);
                }
                Type::Object { decl, args } | Type::Role { decl, args } | Type::Cap { decl, args } => {
                    let mut actuals = vec![];
                    for ty in args {
                        actuals.push(terms.intern(*ty, &env, budget)?);
                    }
                    for (index, actual) in actuals.into_iter().enumerate() {
                        env.insert((0, decl.as_u32(), index as u32), actual);
                    }
                    declaration_edges(module, ty, *decl, &mut facts, &mut children, budget)?;
                    if let Type::Role { .. } = module.types.get(ty).map_err(arena)? {
                        // Every enabled witness target participates, including
                        // targets visible only in a module's test profile.
                        for (candidate, candidate_type) in module.types.iter() {
                            budget.work(1)?;
                            let Type::Object { decl: object, .. } = candidate_type else {
                                continue;
                            };
                            let DeclarationKind::Object { witnesses, .. } =
                                &module.declarations.get(*object).map_err(arena)?.kind else {
                                continue;
                            };
                            if witnesses.iter().any(|witness| matches!(module.types.get(witness.role),
                                Ok(Type::Role { decl: role, .. }) if role == decl)) {
                                children.push(candidate);
                            }
                        }
                    }
                }
                Type::Variant(decl) => {
                    declaration_edges(module, ty, *decl, &mut facts, &mut children, budget)?;
                }
                Type::Fn(_) | Type::Any => {
                    children.extend(captures.get(&ty).into_iter().flatten().copied());
                    if !origins.contains(&ty) {
                        facts.unresolved.insert(ty);
                    }
                }
                Type::Parameter(_) => {
                    facts.unresolved.insert(ty);
                }
                Type::Future(result) => {
                    facts.service_types.insert(ty);
                    children.extend(result);
                }
                Type::Frame(function) => {
                    facts.service_types.insert(ty);
                    let function = module.functions.get(*function).map_err(arena)?;
                    children.extend(&function.signature.params);
                    if let Some(body) = &function.body {
                        children.extend(body.values.iter().map(|(_, value)| value.ty));
                    }
                }
                Type::Service(service) if service.is_linear() => {
                    facts.service_types.insert(ty);
                }
                Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char | Type::Str
                | Type::Range | Type::Never | Type::Enum(_) | Type::Handle(_) | Type::Descriptor
                | Type::Service(_) => {}
            }
            budget.memory(children.len() * (64 + env.len() * 64))?;
            for child in children.into_iter().rev() {
                pending.push(terms.intern(child, &env, budget)?);
            }
        }
        facts.class = if !facts.descriptors.is_empty() || !facts.unresolved.is_empty() {
            Classification::Residual
        } else if facts.may_own() {
            Classification::Managed
        } else {
            Classification::Plain
        };
        if facts.class == Classification::Plain {
            facts.certificate = Some(PlainCertificate::ClosedGraph { ty: root });
        }
        result.insert(root, facts);
    }
    Ok(result)
}

fn declaration_edges(
    module: &Module,
    ty: TypeId,
    decl: DeclId,
    facts: &mut TypeClassification,
    children: &mut Vec<TypeId>,
    budget: &mut Budget<'_>,
) -> Result<(), OwnError> {
    budget.work(1)?;
    match &module.declarations.get(decl).map_err(arena)?.kind {
        DeclarationKind::Object { fields, destroy, builtin, .. } => {
            children.extend(fields.iter().map(|field| field.ty));
            if *builtin {
                facts.service_types.insert(ty);
            }
            if let Some(destroy) = destroy {
                facts.reachable_resources.insert(ty);
                facts.destroy_entries.insert(*destroy);
            }
        }
        DeclarationKind::Variant { cases, .. } => {
            for case in cases {
                if let DeclarationKind::Object { fields, destroy, .. } =
                    &module.declarations.get(case.layout).map_err(arena)?.kind {
                    children.extend(fields.iter().map(|field| field.ty));
                    if let Some(destroy) = destroy {
                        facts.reachable_resources.insert(ty);
                        facts.destroy_entries.insert(*destroy);
                    }
                }
            }
        }
        DeclarationKind::Role { builtin: true, .. } => {
            facts.service_types.insert(ty);
        }
        DeclarationKind::Cap { owner, .. } => children.push(*owner),
        _ => {}
    }
    Ok(())
}
