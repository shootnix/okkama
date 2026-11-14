use super::{EntityLocation as L, OperandPath, OperationSite, RuleId as R, State};
use crate::*;

#[path = "operands.rs"]
mod operands;

#[derive(Clone, Debug, Default)]
struct Substitution { entries: Vec<(TypeParameterRef, TypeId)> }
#[derive(Clone, Copy, Debug)]
struct TypeCheckError;
type Checked<T> = Result<T, TypeCheckError>;
impl From<ArenaError> for TypeCheckError {
    fn from(_: ArenaError) -> Self { Self }
}
fn value_type(body: &Body, value: ValueId) -> Checked<TypeId> { Ok(body.values.get(value)?.ty) }
fn field_type(module: &Module, field: FieldRef) -> Checked<TypeId> {
    match &module.declarations.get(field.declaration)?.kind {
        DeclarationKind::Object { fields, .. } => {
            fields.get(field.index as usize).map(|field| field.ty).ok_or(TypeCheckError)
        }
        _ => Err(TypeCheckError),
    }
}

pub(super) fn children(ty: &Type) -> impl DoubleEndedIterator<Item = TypeId> + Clone + '_ {
    let (slice, first, second): (&[TypeId], Option<TypeId>, Option<TypeId>) = match ty {
        Type::Object { args, .. } | Type::Role { args, .. } | Type::Cap { args, .. } => (args, None, None),
        Type::Successor(ty) | Type::Array(ty) | Type::Builder(ty) | Type::LowCell(ty) => (&[], Some(*ty), None),
        Type::Map(k, v) | Type::MapBuilder(k, v) => (&[], Some(*k), Some(*v)),
        Type::Fn(sig) => (&sig.params, sig.result, None),
        Type::Future(result) => (&[], *result, None),
        Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char | Type::Str
        | Type::Range | Type::Any | Type::Never | Type::Enum(_) | Type::Variant(_) | Type::Handle(_)
        | Type::Parameter(_) | Type::Descriptor | Type::Frame(_) | Type::Service(_) => (&[], None, None),
    };
    slice.iter().copied().chain(first).chain(second)
}
fn scalar(ty: ScalarType) -> Type {
    match ty {
        ScalarType::Int => Type::Int,
        ScalarType::Bool => Type::Bool,
        ScalarType::Byte => Type::Byte,
        ScalarType::Float => Type::Float,
        ScalarType::Rat => Type::Rat,
        ScalarType::Char => Type::Char,
        ScalarType::Str => Type::Str,
        ScalarType::Range => Type::Range,
        ScalarType::Any => Type::Any,
        ScalarType::Descriptor => Type::Descriptor,
    }
}

fn matches_type(
    module: &Module,
    expected: TypeId,
    actual: TypeId,
    substitution: &Substitution,
    state: &mut State<'_>,
) -> Checked<bool> {
    matches_instantiated_types(module, expected, actual, substitution, &Substitution::default(), state)
}

fn matches_instantiated_types(
    module: &Module,
    expected: TypeId,
    actual: TypeId,
    substitution: &Substitution,
    actual_substitution: &Substitution,
    state: &mut State<'_>,
) -> Checked<bool> {
    let mut pending = vec![(expected, actual)];
    let mut seen = std::collections::BTreeSet::new();
    while let Some((expected, actual)) = pending.pop() {
        if !state.step(1) || !state.memory(64) { return Err(TypeCheckError); }
        if !seen.insert((expected, actual)) { continue; }
        let mut expected_ty = module.types.get(expected)?;
        if let Type::Parameter(parameter) = expected_ty
            && let Some((_, replacement)) = substitution.entries.iter().find(|(key, _)| key == parameter) {
            expected_ty = module.types.get(*replacement)?;
        }
        let mut actual_ty = module.types.get(actual)?;
        if let Type::Parameter(parameter) = actual_ty
            && let Some((_, replacement)) = actual_substitution.entries.iter().find(|(key, _)| key == parameter) {
            actual_ty = module.types.get(*replacement)?;
        }
        // Nominal heads and arity must agree before comparing substituted children.
        let same_head = match (expected_ty, actual_ty) {
            (Type::Object { decl: a, args: aa }, Type::Object { decl: b, args: ba })
            | (Type::Role { decl: a, args: aa }, Type::Role { decl: b, args: ba })
            | (Type::Cap { decl: a, args: aa }, Type::Cap { decl: b, args: ba }) => a == b && aa.len() == ba.len(),
            (Type::Array(_), Type::Array(_)) | (Type::Builder(_), Type::Builder(_))
            | (Type::Map(..), Type::Map(..)) | (Type::MapBuilder(..), Type::MapBuilder(..))
            | (Type::Successor(_), Type::Successor(_)) | (Type::LowCell(_), Type::LowCell(_)) => true,
            (Type::Future(a), Type::Future(b)) => a.is_some() == b.is_some(),
            (Type::Fn(a), Type::Fn(b)) => a.is_async == b.is_async && a.params.len() == b.params.len()
                && a.result.is_some() == b.result.is_some(),
            _ => expected_ty == actual_ty,
        };
        if !same_head { return Ok(false); }
        let left = children(expected_ty);
        let right = children(actual_ty);
        if !state.array(left.clone().count(), 16) { return Err(TypeCheckError); }
        pending.extend(left.into_iter().zip(right));
    }
    Ok(true)
}

/// Match symbolic result formulas without interning or mutating the module.
fn matches_result_type(
    module: &Module,
    body: &Body,
    expected: &ResultType,
    actual: TypeId,
    substitution: &Substitution,
    state: &mut State<'_>,
) -> Checked<bool> {
    use ResultType as F;
    let mut formula = expected;
    let mut actual = actual;
    loop {
        if !state.step(1) { return Err(TypeCheckError); }
        let ty = module.types.get(actual)?;
        let exact = match formula {
            F::Exact(id) => Some(*id),
            F::SameAs(value) => Some(value_type(body, *value)?),
            F::Field(field) => Some(field_type(module, *field)?),
            F::Element(value) => match module.types.get(value_type(body, *value)?)? {
                Type::Array(id) => Some(*id), _ => return Err(TypeCheckError),
            },
            F::MapKey(value) | F::MapValue(value) => match module.types.get(value_type(body, *value)?)? {
                Type::Map(key, value) => Some(if matches!(formula, F::MapKey(_)) { *key } else { *value }),
                _ => return Err(TypeCheckError),
            },
            F::Awaited(value) => match module.types.get(value_type(body, *value)?)? {
                Type::Future(Some(id)) => Some(*id), _ => return Err(TypeCheckError),
            },
            F::CellValue(value) => match module.types.get(value_type(body, *value)?)? {
                Type::LowCell(id) => Some(*id), _ => return Err(TypeCheckError),
            },
            _ => None,
        };
        if let Some(exact) = exact { return matches_type(module, exact, actual, substitution, state); }
        return Ok(match formula {
            F::ExactEnum(decl) => ty == &Type::Enum(*decl),
            F::Scalar(expected) => ty == &scalar(*expected),
            F::Service(expected) => ty == &Type::Service(*expected),
            F::ArrayOf(element) => {
                let Type::Array(id) = ty else { return Ok(false); };
                formula = element;
                actual = *id;
                continue;
            }
            F::Object { decl, args } => match ty {
                Type::Object { decl: actual_decl, args: actual_args } => {
                    if decl != actual_decl || args.len() != actual_args.len() { return Ok(false); }
                    for (&expected, &actual) in args.iter().zip(actual_args) {
                        if !matches_type(module, expected, actual, substitution, state)? { return Ok(false); }
                    }
                    true
                }
                _ => false,
            },
            F::FutureOf(result) => match (result, ty) {
                (None, Type::Future(None)) => true,
                (Some(expected), Type::Future(Some(actual))) => {
                    matches_type(module, *expected, *actual, substitution, state)?
                }
                _ => false,
            },
            F::SuccessorOf(value) => match ty {
                Type::Successor(actual) => *actual == value_type(body, *value)?, _ => false,
            },
            F::BuilderOf(expected) => match ty {
                Type::Builder(actual) => matches_type(module, *expected, *actual, substitution, state)?,
                _ => false,
            },
            F::MapBuilderOf(k, v) => match ty {
                Type::MapBuilder(ak, av) => matches_type(module, *k, *ak, substitution, state)?
                    && matches_type(module, *v, *av, substitution, state)?,
                _ => false,
            },
            F::Finished(value) => match (module.types.get(value_type(body, *value)?)?, ty) {
                (Type::Builder(e), Type::Array(a)) => e == a,
                (Type::MapBuilder(k, v), Type::Map(ak, av)) => k == ak && v == av,
                _ => false,
            },
            F::Frame(function) => ty == &Type::Frame(*function),
            F::CellOf(expected) => match ty {
                Type::LowCell(actual) => matches_type(module, *expected, *actual, substitution, state)?,
                _ => false,
            },
            F::Exact(_) | F::SameAs(_) | F::Field(_) | F::Element(_) | F::MapKey(_) | F::MapValue(_)
            | F::Awaited(_) | F::CellValue(_) => return Err(TypeCheckError),
        });
    }
}

struct Checker<'a, 's> {
    module: &'a Module,
    body: &'a Body,
    state: &'s mut State<'a>,
    site: OperationSite,
    substitution: Substitution,
    operand: usize,
}
impl Checker<'_, '_> {
    fn ty(&self, value: ValueId) -> Checked<&Type> {
        Ok(self.module.types.get(value_type(self.body, value)?)?)
    }
    fn report(&mut self, rule: R, message: &str) {
        self.state.location = L::Operand { site: self.site, path: OperandPath(vec![self.operand]) };
        self.operand = self.operand.saturating_add(1);
        self.state.error(rule, message);
    }
    fn require(&mut self, valid: bool, message: &str) {
        if !self.state.step(1) { return; }
        if !valid { self.report(R::T1, message); }
        else { self.operand = self.operand.saturating_add(1); }
    }
    fn expect(&mut self, value: ValueId, expected: Type) -> Checked<()> {
        if matches!(self.ty(value)?, Type::Service(_)) && !matches!(expected, Type::Service(_)) {
            self.report(R::B11, "service token cannot be used as a language reference");
        }
        let valid = self.ty(value)? == &expected;
        self.require(valid, "operand has an unexpected type");
        Ok(())
    }
    fn exact(&mut self, value: ValueId, expected: TypeId) -> Checked<()> {
        let actual = value_type(self.body, value)?;
        if matches!(self.module.types.get(actual)?, Type::Service(_))
            && !matches!(self.module.types.get(expected)?, Type::Service(_)) {
            self.report(R::B11, "service token cannot be stored in a language field/argument");
        }
        let valid = matches_type(self.module, expected, actual, &self.substitution, self.state)?;
        self.require(valid, "operand type differs from the substituted parameter/field type");
        Ok(())
    }
    fn language(&mut self, value: ValueId) -> Checked<()> {
        let valid = language_type(self.module, value_type(self.body, value)?, self.state)?;
        if !valid { self.report(R::B11, "service token is not a language reference"); }
        self.require(valid, "service/internal value is not a language operand");
        Ok(())
    }
    fn service(&mut self, value: ValueId, service: ServiceType) -> Checked<()> {
        self.expect(value, Type::Service(service))
    }
    fn scope(&mut self, value: ValueId) -> Checked<()> {
        let valid = matches!(self.ty(value)?, Type::Service(ServiceType::Scope(_)));
        self.require(valid, "expected scope state");
        Ok(())
    }
    fn owner(&mut self, value: ValueId) -> Checked<()> {
        let valid = matches!(self.ty(value)?, Type::Service(ServiceType::Scope(_) | ServiceType::Owner
            | ServiceType::CacheState | ServiceType::CarrierState | ServiceType::CleanupState) | Type::Frame(_));
        self.require(valid, "expected owner state");
        Ok(())
    }
    fn frame(&mut self, value: ValueId) -> Checked<()> {
        let valid = matches!(self.ty(value)?, Type::Frame(_));
        self.require(valid, "expected frame");
        Ok(())
    }
    fn future(&mut self, value: ValueId) -> Checked<Option<TypeId>> {
        match self.ty(value)? {
            Type::Future(result) => Ok(*result),
            _ => {
                self.require(false, "expected Future");
                Err(TypeCheckError)
            }
        }
    }
    fn packet(&mut self, value: Option<ValueId>) -> Checked<()> {
        if let Some(value) = value { self.service(value, ServiceType::Own)?; }
        Ok(())
    }
    fn states(&mut self, states: TransferStates) -> Checked<()> {
        for state in states.values() { self.scope(state)?; }
        Ok(())
    }
    fn same(&mut self, left: ValueId, right: ValueId) -> Checked<()> {
        self.exact(right, value_type(self.body, left)?)
    }
    fn binding(&mut self, binding: BindingId, value: ValueId) -> Checked<()> {
        self.exact(value, self.body.bindings.get(binding)?.ty)
    }
    fn field(&mut self, object: ValueId, field: FieldRef) -> Checked<TypeId> {
        self.object_substitution(object)?;
        let valid = matches!(self.ty(object)?, Type::Object { decl, .. } if *decl == field.declaration);
        self.require(valid, "field does not belong to the receiver layout");
        field_type(self.module, field)
    }
    fn object_substitution(&mut self, value: ValueId) -> Checked<()> {
        if let Type::Object { decl, args } | Type::Role { decl, args } | Type::Cap { decl, args } = self.ty(value)? {
            self.substitution = substitution(GenericOwner::Declaration(*decl), args);
        }
        Ok(())
    }
    fn prepared(&mut self, exit: &PreparedExit) -> Checked<()> {
        if let Some(value) = exit.value { self.language(value)?; }
        self.packet(exit.packet)?;
        if let Some(deps) = exit.deps { self.service(deps, ServiceType::Deps)?; }
        Ok(())
    }
    fn fields(&mut self, decl: DeclId, args: &[TypeId], values: &[ValueId]) -> Checked<()> {
        let DeclarationKind::Object { fields, .. } = &self.module.declarations.get(decl)?.kind else {
            self.require(false, "constructor requires an object layout");
            return Err(TypeCheckError);
        };
        self.substitution = substitution(GenericOwner::Declaration(decl), args);
        self.require(fields.len() == values.len(), "constructor field count differs from the layout");
        for (field, &value) in fields.iter().zip(values) { self.exact(value, field.ty)?; }
        Ok(())
    }
    fn array(&mut self, value: ValueId) -> Checked<TypeId> {
        match self.ty(value)? {
            Type::Array(element) => Ok(*element),
            _ => { self.require(false, "expected array"); Err(TypeCheckError) }
        }
    }
    fn map(&mut self, value: ValueId) -> Checked<(TypeId, TypeId)> {
        match self.ty(value)? {
            Type::Map(k, v) => Ok((*k, *v)),
            _ => { self.require(false, "expected map"); Err(TypeCheckError) }
        }
    }
    fn buffer(&mut self, value: ValueId, map: bool) -> Checked<(TypeId, TypeId)> {
        match self.ty(value)? {
            Type::Builder(e) if !map => Ok((*e, *e)),
            Type::MapBuilder(k, v) if map => Ok((*k, *v)),
            _ => { self.require(false, "unexpected builder type"); Err(TypeCheckError) }
        }
    }
}
fn substitution(owner: GenericOwner, args: &[TypeId]) -> Substitution {
    Substitution { entries: args.iter().enumerate().map(|(index, &ty)| {
        (TypeParameterRef { owner, index: index as u32 }, ty)
    }).collect() }
}

fn language_type(module: &Module, root: TypeId, state: &mut State<'_>) -> Checked<bool> {
    let mut seen = std::collections::BTreeSet::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if !state.step(1) { return Err(TypeCheckError); }
        if !state.memory(64) { return Err(TypeCheckError); }
        if !seen.insert(id) { continue; }
        let ty = module.types.get(id)?;
        if matches!(ty, Type::Service(_) | Type::Builder(_) | Type::MapBuilder(..)
            | Type::Frame(_) | Type::LowCell(_)) { return Ok(false); }
        if matches!(ty, Type::Never) && id == root { return Ok(false); }
        let children = children(ty);
        if !state.array(children.clone().count(), 8) { return Err(TypeCheckError); }
        pending.extend(children);
    }
    Ok(true)
}

fn literal(module: &Module, value: &Literal, state: &mut State<'_>) -> bool {
    let integer = |value: &str| {
        let digits = value.strip_prefix('-').unwrap_or(value);
        !digits.is_empty() && digits.bytes().all(|ch| ch.is_ascii_digit())
            && (digits == "0" || !digits.starts_with('0')) && value != "-0"
    };
    match value {
        Literal::Int(value) => integer(value),
        Literal::Rat { numerator, denominator } => integer(numerator) && integer(denominator)
            && denominator != "0" && !denominator.starts_with('-')
            && coprime(numerator, denominator, state),
        Literal::Float(bits) => f64::from_bits(*bits).is_finite(),
        Literal::Enum { decl, case } => module.declarations.get(*decl).is_ok_and(|decl| {
            matches!(&decl.kind, DeclarationKind::Enum { cases, .. } if (*case as usize) < cases.len())
        }),
        Literal::Bool(_) | Literal::Byte(_) | Literal::Char(_) | Literal::Str(_) => true,
    }
}

fn operation<'a>(
    module: &'a Module,
    body: &'a Body,
    op: &Operation,
    results: Option<&[ValueId]>,
    site: OperationSite,
    state: &mut State<'a>,
) -> Option<Substitution> {
    if !state.step(1) || !super::index::intact(module, body, op, state) { return None; }
    let mut checker = Checker { module, body, state, site, substitution: Substitution::default(), operand: 0 };
    let before = checker.state.diagnostics.len();
    if checker.operands(op).is_err() {
        if before == checker.state.diagnostics.len() { checker.report(R::T1, "invalid operand type or index"); }
        return None;
    }
    let Ok(contract) = ContractRegistry::new(module).construction_schema(op, body) else {
        checker.report(R::T1, "operation contract cannot be resolved from its declarations");
        return None;
    };
    for outcome in [&contract.normal, contract.unwind.as_ref().unwrap_or(&contract.normal)] {
        if outcome.results.len() > checker.state.options.limits.outcome_slots {
            checker.state.incomplete = true;
            return None;
        }
    }
    if let Some(results) = results {
        if results.len() != contract.normal.results.len() {
            checker.report(R::T2, "result tuple arity differs from the opcode schema");
        }
        for (&value, slot) in results.iter().zip(&contract.normal.results) {
            let Ok(actual) = value_type(body, value) else { continue; };
            match matches_result_type(module, body, &slot.ty, actual, &checker.substitution, checker.state) {
                Ok(true) => {}
                Ok(false) | Err(_) => checker.report(R::T2, "result type differs from the opcode schema"),
            }
        }
    }
    Some(checker.substitution)
}

pub(super) fn check<'a>(module: &'a Module, selected: Option<FuncId>, state: &mut State<'a>) {
    if state.incomplete { return; }
    if selected.is_none() { declarations(module, state); }
    for (function, data) in module.functions.iter() {
        if selected.is_some_and(|selected| function != selected) { continue; }
        if state.poisoned.contains(&L::Function(function)) { continue; }
        state.at(L::Function(function), data.loc);
        let before = state.diagnostics.len();
        if function_types(module, data, state).is_err() && before == state.diagnostics.len() {
            state.error(R::T5, "invalid function attribute layout or signature");
        }
        let Some(body) = &data.body else { continue; };
        metadata(module, body, function, state);
        for (value, data) in body.values.iter() {
            if module.types.get(data.ty).is_ok_and(|ty| matches!(ty, Type::Never)) {
                state.at(L::Value { function, value }, None);
                state.error(R::T2, "never/void cannot be an SSA value type");
            }
        }
        let mut next_edge = 0_u32;
        for (block_id, block) in body.blocks.iter() {
            if !state.step(1) { return; }
            for &id in &block.insts {
                let Ok(inst) = body.insts.get(id) else { continue; };
                let location = L::Instruction { function, instruction: id };
                if state.poisoned.contains(&location) { continue; }
                state.at(location, inst.loc);
                let _ = operation(module, body, &inst.op, Some(&inst.results),
                    OperationSite::Instruction { function, instruction: id }, state);
            }
            state.at(L::Terminator { function, block: block_id }, block.term.loc);
            terminator(module, body, function, block_id, &data.signature, &mut next_edge, state);
        }
    }
}

fn type_valid(module: &Module, ty: &Type, state: &mut State<'_>) -> Checked<()> {
    let nominal = match ty {
        Type::Object { decl, args } | Type::Role { decl, args } | Type::Cap { decl, args } => Some((*decl, args)),
        _ => None,
    };
    if let Some((decl, args)) = nominal {
        let data = module.declarations.get(decl)?;
        let kind_valid = matches!((ty, &data.kind),
            (Type::Object { .. }, DeclarationKind::Object { .. })
            | (Type::Role { .. }, DeclarationKind::Role { .. })
            | (Type::Cap { .. }, DeclarationKind::Cap { .. }));
        if !kind_valid { state.error(R::T5, "nominal type has the wrong declaration kind"); }
        generic_arguments(module, GenericOwner::Declaration(decl), &data.type_params, args, state)?;
    }
    match ty {
        Type::Enum(id) | Type::Variant(id) | Type::Handle(id) => {
            let kind = &module.declarations.get(*id)?.kind;
            if !matches!((ty, kind), (Type::Enum(_), DeclarationKind::Enum { .. })
                | (Type::Variant(_), DeclarationKind::Variant { .. }) | (Type::Handle(_), DeclarationKind::Handle)) {
                state.error(R::T5, "nominal type has the wrong declaration kind");
            }
        }
        Type::Parameter(parameter) => {
            let params = match parameter.owner {
                GenericOwner::Declaration(id) => &module.declarations.get(id)?.type_params,
                GenericOwner::Function(id) => &module.functions.get(id)?.type_params,
            };
            if parameter.index as usize >= params.len() { state.error(R::T4, "type parameter index is out of range"); }
        }
        Type::Map(key, value) | Type::MapBuilder(key, value) => {
            if !map_key(module.types.get(*key)?) { state.error(R::T1, "map key type is not permitted"); }
            if module.types.get(*value)? != &Type::Never && !language_type(module, *value, state)? {
                state.error(R::T1, "map value must be a language type");
            }
        }
        Type::Array(element) | Type::Builder(element) | Type::Future(Some(element)) => {
            if module.types.get(*element)? != &Type::Never && !language_type(module, *element, state)? {
                state.error(R::T1, "collection/Future element must be a language type");
            }
        }
        Type::Fn(signature) => {
            for &ty in signature.params.iter().chain(signature.result.iter()) {
                if !language_type(module, ty, state)? { state.error(R::T1, "fn ABI contains an internal value"); }
            }
        }
        Type::Successor(inner) if !matches!(module.types.get(*inner)?, Type::Object { .. }) => {
            state.error(R::T1, "successor must wrap an object type");
        }
        _ => {}
    }
    Ok(())
}
fn map_key(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char | Type::Str
        | Type::Enum(_) | Type::Parameter(_) | Type::Never)
}
fn implements(
    module: &Module,
    actual: TypeId,
    bound: TypeId,
    sub: &Substitution,
    state: &mut State<'_>,
) -> Checked<bool> {
    if matches_type(module, bound, actual, sub, state)? { return Ok(true); }
    match module.types.get(actual)? {
        Type::Object { decl, args } => {
            let DeclarationKind::Object { witnesses, .. } = &module.declarations.get(*decl)?.kind else {
                return Ok(false);
            };
            let object_sub = substitution(GenericOwner::Declaration(*decl), args);
            for witness in witnesses {
                // Compare a substituted witness against the instantiated bound.
                if matches_instantiated_types(module, bound, witness.role, sub, &object_sub, state)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Type::Parameter(parameter) => {
            let params = match parameter.owner {
                GenericOwner::Declaration(id) => &module.declarations.get(id)?.type_params,
                GenericOwner::Function(id) => &module.functions.get(id)?.type_params,
            };
            Ok(params.get(parameter.index as usize).is_some_and(|param| param.bounds.contains(&bound)))
        }
        _ => Ok(false),
    }
}
fn generic_arguments(
    module: &Module,
    owner: GenericOwner,
    parameters: &[TypeParameter],
    args: &[TypeId],
    state: &mut State<'_>,
) -> Checked<()> {
    if parameters.len() != args.len() { state.error(R::T4, "generic argument arity mismatch"); }
    if !state.array(args.len(), 16) { return Err(TypeCheckError); }
    let sub = substitution(owner, args);
    for (param, &actual) in parameters.iter().zip(args) {
        for &bound in &param.bounds {
            if !implements(module, actual, bound, &sub, state)? {
                state.error(R::T4, "generic bound is not satisfied");
            }
        }
    }
    Ok(())
}

fn declarations(module: &Module, state: &mut State<'_>) {
    for (id, ty) in module.types.iter() {
        if !state.step(1) { return; }
        state.at(L::Type(id), None);
        let before = state.diagnostics.len();
        if type_valid(module, ty, state).is_err() && before == state.diagnostics.len() {
            state.error(R::T5, "type has an invalid declaration or component");
        }
    }
    for (id, decl) in module.declarations.iter() {
        if !state.step(1) { return; }
        state.at(L::Declaration(id), decl.loc);
        let before = state.diagnostics.len();
        if declaration(module, id, decl, state).is_err() && before == state.diagnostics.len() {
            state.error(R::T5, "declaration has an invalid layout or signature");
        }
    }
}
fn declaration(module: &Module, id: DeclId, decl: &Declaration, state: &mut State<'_>) -> Checked<()> {
    match &decl.kind {
        DeclarationKind::Object { fields, witnesses, destroy, .. } => {
            for (index, field) in fields.iter().enumerate() {
                if !state.step(1) { return Err(TypeCheckError); }
                if !language_type(module, field.ty, state)?
                    || matches!(module.types.get(field.ty)?, Type::Successor(_)) {
                    state.error(R::T5, "object field must have a storable language type");
                }
                if let Some(default) = field.default {
                    let function = module.functions.get(default)?;
                    if !language_slots(module, &function.signature.params, state).is_empty()
                        || language_slots(module, &function.signature.results, state) != [field.ty]
                        || !function.attrs.contains(&FunctionAttr::Default { object: id, field: index as u32 }) {
                        state.error(R::T5, "field default has an incompatible signature or attribute");
                    }
                }
            }
            if let Some(destroy) = destroy {
                let function = module.functions.get(*destroy)?;
                let valid = function.signature.params.len() == 1 && function.signature.results.is_empty()
                    && function.signature.params.first().and_then(|id| module.types.get(*id).ok())
                        .is_some_and(|ty| matches!(ty, Type::Object { decl, .. } if *decl == id))
                    && function.attrs.contains(&FunctionAttr::Destroy(id));
                if !valid { state.error(R::T5, "destroy needs a matching self parameter, void result and attribute"); }
            }
            let mut seen = std::collections::BTreeSet::new();
            for witness in witnesses {
                if !state.step(1) { return Err(TypeCheckError); }
                if !seen.insert(witness.role) { state.error(R::T5, "duplicate role witness"); }
                let Type::Role { decl: role, args } = module.types.get(witness.role)? else {
                    state.error(R::T5, "witness must name a role type");
                    continue;
                };
                let role_decl = module.declarations.get(*role)?;
                if role_decl.name == "Error" {
                    state.error(R::T6, "an object cannot implement Error");
                }
                let DeclarationKind::Role { methods, .. } = &role_decl.kind else {
                    state.error(R::T5, "witness declaration is not a role");
                    continue;
                };
                if witness.slots.len() != methods.len() { state.error(R::T5, "witness slot count mismatch"); }
                let sub = substitution(GenericOwner::Declaration(*role), args);
                for (&function, method) in witness.slots.iter().zip(methods) {
                    if !state.step(1) { return Err(TypeCheckError); }
                    let signature = &module.functions.get(function)?.signature;
                    let receiver = signature.params.first().and_then(|id| module.types.get(*id).ok());
                    if !matches!(receiver, Some(Type::Object { decl, .. }) if *decl == id)
                        || !module.functions.get(function)?.attrs.contains(&FunctionAttr::Method(id)) {
                        state.error(R::T5, "witness needs a method with its concrete object receiver");
                    }
                    let params = signature.params.get(1..).unwrap_or(&[]);
                    if params.len() != method.signature.params.len()
                        || signature.results.len() != method.signature.results.len()
                        || !witness_abi(method.signature.ownership_abi, signature.ownership_abi) {
                        state.error(R::T5, "witness signature arity or ABI mismatch");
                    }
                    for (&expected, &actual) in method.signature.params.iter().zip(params)
                        .chain(method.signature.results.iter().zip(&signature.results)) {
                        if !matches_type(module, expected, actual, &sub, state)? {
                            state.error(R::T5, "witness signature type mismatch");
                        }
                    }
                }
            }
        }
        DeclarationKind::Variant { cases, is_error } => {
            for case in cases {
                if !state.step(1) { return Err(TypeCheckError); }
                let DeclarationKind::Object { fields, destroy, .. } = &module.declarations.get(case.layout)?.kind else {
                    state.error(R::T5, "variant case requires an object layout");
                    continue;
                };
                if *is_error {
                    if destroy.is_some() { state.error(R::T6, "Error layout cannot have destroy"); }
                    for field in fields {
                        if !state.step(1) { return Err(TypeCheckError); }
                        if !error_payload(module, field.ty, state)? {
                            state.error(R::T6, "Error payload must be a permitted scalar or recursively plain enum");
                        }
                    }
                }
            }
        }
        DeclarationKind::Cap { owner, fields } => {
            if !language_type(module, *owner, state)? { state.error(R::T5, "cap owner is an internal type"); }
            for field in fields {
                if !state.step(1) { return Err(TypeCheckError); }
                let function = module.functions.get(field.function)?;
                let params = language_slots(module, &function.signature.params, state);
                let results = language_slots(module, &function.signature.results, state);
                if params != [*owner] || results.len() != 1 {
                    state.error(R::T5, "cap field must take its owner and return one value");
                }
            }
        }
        DeclarationKind::Global { ty, init } => {
            let function = module.functions.get(*init)?;
            if !language_type(module, *ty, state)?
                || !language_slots(module, &function.signature.params, state).is_empty()
                || language_slots(module, &function.signature.results, state) != [*ty] {
                state.error(R::T5, "global initializer must take no arguments and return the global type");
            }
        }
        DeclarationKind::Role { .. } | DeclarationKind::Enum { .. } | DeclarationKind::Handle => {}
    }
    Ok(())
}
fn error_payload(module: &Module, root: TypeId, state: &mut State<'_>) -> Checked<bool> {
    let mut seen = std::collections::BTreeSet::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if !state.step(1) || !state.memory(64) { return Err(TypeCheckError); }
        if !seen.insert(id) { continue; }
        match module.types.get(id)? {
            Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char | Type::Str
            | Type::Enum(_) => {}
            Type::Variant(id) => {
                let DeclarationKind::Variant { cases, .. } = &module.declarations.get(*id)?.kind else {
                    return Ok(false);
                };
                for case in cases {
                    if !state.step(1) { return Err(TypeCheckError); }
                    let DeclarationKind::Object { fields, destroy: None, .. } =
                        &module.declarations.get(case.layout)?.kind else { return Ok(false); };
                    if !state.array(fields.len(), 8) { return Err(TypeCheckError); }
                    pending.extend(fields.iter().map(|field| field.ty));
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn target(body: &Body, target: &Target, implicit_failure: bool, state: &mut State<'_>) {
    let Ok(block) = body.blocks.get(target.block) else { return; };
    let Some(expected_len) = target.args.len().checked_add(usize::from(implicit_failure)) else {
        state.incomplete = true;
        return;
    };
    if block.params.len() != expected_len { state.error(R::T3, "branch argument count mismatch"); }
    for (&arg, &param) in target.args.iter().zip(&block.params) {
        if let (Ok(a), Ok(b)) = (value_type(body, arg), value_type(body, param))
            && a != b { state.error(R::T3, "branch argument type mismatch"); }
    }
}
fn invoke_target(
    module: &Module,
    body: &Body,
    edge: &InvokeEdge,
    outcome: Outcome,
    slots: &[ResultSlot],
    sub: &Substitution,
    state: &mut State<'_>,
) {
    let Ok(block) = body.blocks.get(edge.block) else { return; };
    if edge.args.len() != block.params.len() { state.error(R::T3, "edge argument count mismatch"); }
    for (arg, &param) in edge.args.iter().zip(&block.params) {
        if !state.step(1) { return; }
        let Ok(actual) = value_type(body, param) else { continue; };
        let valid = match arg {
            EdgeArg::Value(value) => value_type(body, *value).is_ok_and(|ty| ty == actual),
            EdgeArg::Produced(result) => result.outcome == outcome && slots.get(usize::from(result.slot))
                .is_some_and(|slot| matches_result_type(module, body, &slot.ty, actual, sub, state).unwrap_or(false)),
        };
        if !valid { state.error(R::T3, "edge slot/result type mismatch"); }
    }
}
fn terminator<'a>(
    module: &'a Module,
    body: &'a Body,
    function: FuncId,
    block_id: BlockId,
    signature: &Signature,
    next_edge: &mut u32,
    state: &mut State<'a>,
) {
    let Ok(block) = body.blocks.get(block_id) else { return; };
    if let Some(unwind) = &block.unwind {
        at_edge(function, block_id, next_edge, state);
        target(body, &unwind.target, true, state);
        if let Ok(target) = body.blocks.get(unwind.target.block) {
            let valid = target.params.last().and_then(|value| body.values.get(*value).ok())
                .and_then(|value| module.types.get(value.ty).ok()) == Some(&Type::Service(ServiceType::Failure));
            if !valid { state.error(R::T3, "block unwind must end with an implicit failure parameter"); }
        }
    }
    let sub = Substitution::default();
    let site = OperationSite::Terminator { function, block: block_id };
    let mut checker = Checker { module, body, state, site, substitution: sub.clone(), operand: 0 };
    let before = checker.state.diagnostics.len();
    let mut check = || -> Checked<()> {
        match &block.term.kind {
            TermKind::Br(edge) => {
                at_edge(function, block_id, next_edge, checker.state);
                target(body, edge, false, checker.state);
            }
            TermKind::Cond { condition, yes, no } => {
                checker.expect(*condition, Type::Bool)?;
                at_edge(function, block_id, next_edge, checker.state);
                target(body, yes, false, checker.state);
                at_edge(function, block_id, next_edge, checker.state);
                target(body, no, false, checker.state);
            }
            TermKind::Switch { value, cases, default } => {
                let ty = checker.ty(*value)?;
                let valid = map_key(ty);
                checker.require(valid, "switch needs a scalar/enum value");
                for case in cases {
                    if !literal(module, &case.value, checker.state) { checker.report(R::T1, "invalid switch literal"); }
                    let formula = Operation::Const(case.value.clone()).spec();
                    if let crate::contracts::Rule::Data { ty, .. } = formula.rule
                        && !matches_result_type(module, body, &ty, value_type(body, *value)?, &sub, checker.state)? {
                        checker.report(R::T1, "switch case type differs from its scrutinee");
                    }
                    at_edge(function, block_id, next_edge, checker.state);
                target(body, &case.target, false, checker.state);
                }
                at_edge(function, block_id, next_edge, checker.state);
                target(body, default, false, checker.state);
            }
            TermKind::Ret { values } => {
                if values.len() != signature.results.len() { checker.report(R::T2, "return arity mismatch"); }
                for (&value, &ty) in values.iter().zip(&signature.results) { checker.exact(value, ty)?; }
            }
            TermKind::Raise(value) => checker.service(*value, ServiceType::Failure)?,
            TermKind::Panic(value) => checker.expect(*value, Type::Str)?,
            TermKind::Throw(value) => {
                let ty = checker.ty(*value)?;
                let valid = is_error(module, ty);
                checker.require(valid, "throw needs an Error enum value");
            }
            TermKind::Catch { failure, ty, handler, next } => {
                checker.service(*failure, ServiceType::Failure)?;
                let valid = is_error(module, module.types.get(*ty)?);
                checker.require(valid, "catch needs an Error type");
                at_edge(function, block_id, next_edge, checker.state);
                invoke_target(module, body, handler, Outcome::Normal,
                    &[ResultSlot::new(SlotName::Value, ResultType::Exact(*ty))], &sub, checker.state);
                at_edge(function, block_id, next_edge, checker.state);
                target(body, next, false, checker.state);
            }
            TermKind::Lookup { kind, collection, key, found, missing } => {
                let formula = match kind {
                    LookupKind::Array => {
                        checker.array(*collection)?;
                        checker.expect(*key, Type::Int)?;
                        ResultType::Element(*collection)
                    }
                    LookupKind::Map => {
                        let (k, _) = checker.map(*collection)?;
                        checker.exact(*key, k)?;
                        ResultType::MapValue(*collection)
                    }
                };
                at_edge(function, block_id, next_edge, checker.state);
                invoke_target(module, body, found, Outcome::Normal,
                    &[ResultSlot::new(SlotName::Value, formula)], &sub, checker.state);
                at_edge(function, block_id, next_edge, checker.state);
                target(body, missing, false, checker.state);
            }
            TermKind::InvokeOwn { op, normal, unwind } => {
                let sub = operation(module, body, op, None, site, checker.state).unwrap_or_default();
                if let Ok(contract) = ContractRegistry::new(module).construction_schema(op, body) {
                    at_edge(function, block_id, next_edge, checker.state);
                    invoke_target(module, body, normal, Outcome::Normal, &contract.normal.results, &sub, checker.state);
                    if let Some(failure) = &contract.unwind {
                        at_edge(function, block_id, next_edge, checker.state);
                        invoke_target(module, body, unwind, Outcome::Unwind, &failure.results, &sub, checker.state);
                    } else { checker.report(R::T3, "invoke operation has no unwind result schema"); }
                }
            }
            TermKind::Trap { .. } | TermKind::Unreachable => {}
        }
        Ok(())
    };
    if check().is_err() && before == checker.state.diagnostics.len() {
        checker.report(R::T1, "invalid terminator operand type or index");
    }
}
fn is_error(module: &Module, ty: &Type) -> bool {
    match ty {
        Type::Enum(id) | Type::Variant(id) => module.declarations.get(*id).is_ok_and(|decl| matches!(decl.kind,
            DeclarationKind::Enum { is_error: true, .. } | DeclarationKind::Variant { is_error: true, .. })),
        Type::Role { decl, .. } => module.declarations.get(*decl).is_ok_and(|decl| decl.name == "Error"
            && matches!(decl.kind, DeclarationKind::Role { builtin: true, .. })),
        _ => false,
    }
}

fn metadata<'a>(module: &'a Module, body: &'a Body, function: FuncId, state: &mut State<'a>) {
    let mut checker = Checker {
        module, body, state, site: OperationSite::Terminator { function, block: body.entry },
        substitution: Substitution::default(), operand: 0,
    };
    for (_, key) in body.keys.iter() {
        if !checker.state.step(1) { return; }
        let result = match key {
            KeyExpr::Created { instance, .. } => checker.language(*instance),
            KeyExpr::Runtime(value) => checker.service(*value, ServiceType::StableKey),
            KeyExpr::ServiceOf { value, .. } => checker.ty(*value).map(|_| ()),
        };
        if result.is_err() { checker.report(R::T1, "invalid metadata key operand"); }
    }
    for (_, dep) in body.deps.iter() {
        if !checker.state.step(1) { return; }
        let result = match dep {
            DepExpr::FromSsa(value) => {
                if checker.ty(*value).is_ok_and(|ty| {
                    matches!(ty, Type::Service(service) if service.is_linear())
                }) {
                    checker.report(R::B31, "a linear packet register cannot anchor a lifetime");
                }
                checker.service(*value, ServiceType::Deps)
            }
            DepExpr::Retention { value, .. } => checker.language(*value),
            DepExpr::Empty | DepExpr::Key(_) | DepExpr::Union(_) => Ok(()),
        };
        if result.is_err() { checker.report(R::T1, "invalid dependency expression operand"); }
    }
    for (_, selection) in body.selections.iter() {
        if !checker.state.step(1) { return; }
        let mut check = || -> Checked<()> {
            match selection {
                SelectionExpr::Value(value) | SelectionExpr::OwnerGraph(value) | SelectionExpr::ViewOwner(value) => {
                    checker.language(*value)?;
                }
                SelectionExpr::Descriptor { descriptor, value } => {
                    checker.expect(*descriptor, Type::Descriptor)?;
                    checker.language(*value)?;
                }
                SelectionExpr::Projection(projection) => match projection {
                    Projection::Field { value, field } => { checker.field(*value, *field)?; }
                    Projection::Index { value, index } => {
                        checker.array(*value)?;
                        checker.expect(*index, Type::Int)?;
                    }
                    Projection::Slice { value, start, end } => {
                        checker.array(*value)?;
                        checker.expect(*start, Type::Int)?;
                        checker.expect(*end, Type::Int)?;
                    }
                    Projection::Map { value, keys } => {
                        let (key, _) = checker.map(*value)?;
                        let element = checker.array(*keys)?;
                        checker.require(key == element, "selection map key type mismatch");
                    }
                },
                SelectionExpr::Union(_) | SelectionExpr::Admissible(_) => {}
            }
            Ok(())
        };
        if check().is_err() { checker.report(R::T1, "invalid selection operand"); }
    }
}

// Decimal Stein GCD keeps the crate std-only. Every digit operation is charged;
// very large literals exhaust the work budget instead of monopolising verification.
fn coprime(numerator: &str, denominator: &str, state: &mut State<'_>) -> bool {
    let numerator = numerator.strip_prefix('-').unwrap_or(numerator);
    let Some(len) = numerator.len().checked_add(denominator.len()) else {
        state.incomplete = true;
        return false;
    };
    if !state.array(len, 4) || !state.step(len) { return false; }
    let mut left: Vec<_> = numerator.bytes().map(|digit| digit - b'0').collect();
    let mut right: Vec<_> = denominator.bytes().map(|digit| digit - b'0').collect();
    if left == [0] { return right == [1]; }
    if left.last().is_some_and(|digit| digit % 2 == 0)
        && right.last().is_some_and(|digit| digit % 2 == 0) { return false; }
    loop {
        if !state.step(1) { return false; }
        for number in [&mut left, &mut right] {
            while number.last().is_some_and(|digit| digit % 2 == 0) {
                if !state.step(number.len()) { return false; }
                let mut carry = 0;
                for digit in number.iter_mut() {
                    let next = carry * 10 + *digit;
                    *digit = next / 2;
                    carry = next % 2;
                }
                trim_decimal(number);
            }
        }
        if !state.step(left.len().saturating_add(right.len())) { return false; }
        let order = left.len().cmp(&right.len()).then_with(|| left.cmp(&right));
        if order == std::cmp::Ordering::Equal { return left == [1]; }
        let (larger, smaller) = if order == std::cmp::Ordering::Greater {
            (&mut left, &right)
        } else {
            (&mut right, &left)
        };
        let mut borrow = 0_i16;
        for (index, digit) in larger.iter_mut().rev().enumerate() {
            let other = smaller.len().checked_sub(index + 1).and_then(|index| smaller.get(index)).copied();
            let next = i16::from(*digit) - i16::from(other.unwrap_or(0)) - borrow;
            borrow = i16::from(next < 0);
            *digit = (next + borrow * 10) as u8;
        }
        trim_decimal(larger);
    }
}
fn trim_decimal(number: &mut Vec<u8>) {
    if let Some(first) = number.iter().position(|&digit| digit != 0) {
        number.drain(..first);
    } else {
        number.clear();
        number.push(0);
    }
}

fn function_types(module: &Module, data: &Function, state: &mut State<'_>) -> Checked<()> {
    let mut owners = 0_usize;
    for attr in &data.attrs {
        if !state.step(1) { return Err(TypeCheckError); }
        let owner = match attr {
            FunctionAttr::Method(owner) | FunctionAttr::Ctor(owner) | FunctionAttr::Successor(owner)
            | FunctionAttr::Destroy(owner) | FunctionAttr::Default { object: owner, .. } => Some(*owner),
            _ => None,
        };
        if let Some(owner) = owner {
            owners += 1;
            if !matches!(module.declarations.get(owner)?.kind, DeclarationKind::Object { .. }) {
                state.error(R::T5, "object action attribute must name an object declaration");
            }
        }
        let receiver = |owner| data.signature.params.first().and_then(|id| module.types.get(*id).ok())
            .is_some_and(|ty| matches!(ty, Type::Object { decl, .. } if *decl == owner));
        match attr {
            FunctionAttr::Extern(name) => {
                if state.context.external_contracts.native(name) != Some(&data.signature) {
                    state.error(R::T5, "external declaration does not match the trusted native ABI catalog");
                }
            }
            FunctionAttr::Intrinsic { name, .. } => {
                if state.context.external_contracts.intrinsic(name) != Some(&data.signature) {
                    state.error(R::T5, "intrinsic declaration does not match the trusted ABI catalog");
                }
            }
            FunctionAttr::Method(owner) | FunctionAttr::Destroy(owner) | FunctionAttr::Successor(owner) => {
                if !receiver(*owner) { state.error(R::T5, "object action needs its concrete receiver in slot zero"); }
                if matches!(attr, FunctionAttr::Destroy(_))
                    && (data.signature.params.len() != 1 || !data.signature.results.is_empty()) {
                    state.error(R::T5, "destroy must take only self and return void");
                }
                if matches!(attr, FunctionAttr::Successor(_)) {
                    let results = language_slots(module, &data.signature.results, state);
                    let valid = results.len() == 1 && results.first()
                        .and_then(|id| module.types.get(*id).ok()).is_some_and(|ty| match ty {
                            Type::Successor(id) => module.types.get(*id).is_ok_and(|ty| {
                                matches!(ty, Type::Object { decl, .. } if decl == owner)
                            }),
                            _ => false,
                        });
                    if !valid { state.error(R::T5, "successor action must return its successor type"); }
                }
            }
            FunctionAttr::Ctor(owner) => {
                let results = language_slots(module, &data.signature.results, state);
                let valid = results.len() == 1 && results.first()
                    .and_then(|id| module.types.get(*id).ok())
                    .is_some_and(|ty| matches!(ty, Type::Object { decl, .. } if decl == owner));
                if !valid { state.error(R::T5, "constructor must return its object type"); }
            }
            FunctionAttr::Default { object, field } => {
                let expected = field_type(module, FieldRef { declaration: *object, index: *field })?;
                if !language_slots(module, &data.signature.params, state).is_empty()
                    || language_slots(module, &data.signature.results, state) != [expected] {
                    state.error(R::T5, "default must take no arguments and return its field type");
                }
            }
            _ => {}
        }
    }
    if owners > 1 { state.error(R::S1, "function has more than one object owner attribute"); }
    for &ty in &data.signature.params {
        if matches!(module.types.get(ty)?, Type::Successor(_)) {
            state.error(R::T5, "function parameter cannot have a successor type");
        }
    }
    Ok(())
}

fn at_edge(function: FuncId, block: BlockId, next: &mut u32, state: &mut State<'_>) {
    let edge = super::VerifyEdgeId(*next);
    let Some(successor) = next.checked_add(1) else {
        state.incomplete = true;
        return;
    };
    *next = successor;
    if !state.memory(64) { return; }
    state.edge_positions.insert((function, edge), block);
    state.location = L::Edge { function, edge };
}

fn language_slots(module: &Module, slots: &[TypeId], state: &mut State<'_>) -> Vec<TypeId> {
    if !state.step(slots.len()) || !state.array(slots.len(), std::mem::size_of::<TypeId>()) { return vec![]; }
    slots.iter().copied().filter(|id| {
        module.types.get(*id).is_ok_and(|ty| !matches!(ty, Type::Service(_) | Type::Descriptor))
    }).collect()
}
fn witness_abi(role: OwnershipAbi, method: OwnershipAbi) -> bool {
    match (role, method) {
        (OwnershipAbi::Explicit { input_packet: a, output_packet: ar },
            OwnershipAbi::Explicit { input_packet: b, output_packet: br }) => a.checked_add(1) == Some(b) && ar == br,
        _ => role == method,
    }
}

pub(super) fn edge_slot_matches(
    module: &Module,
    body: &Body,
    op: &Operation,
    expected: &ResultType,
    actual: TypeId,
    state: &mut State<'_>,
) -> bool {
    let sub = super::summaries::call(op).and_then(|(call, receiver, target)| {
        if let Some(function) = target {
            return Some(substitution(GenericOwner::Function(function), &call.type_args));
        }
        let receiver = receiver?;
        let ty = module.types.get(body.values.get(receiver.value).ok()?.ty).ok()?;
        match ty {
            Type::Role { decl, args } | Type::Cap { decl, args } => {
                Some(substitution(GenericOwner::Declaration(*decl), args))
            }
            _ => None,
        }
    }).unwrap_or_default();
    matches_result_type(module, body, expected, actual, &sub, state).unwrap_or(false)
}

pub(super) fn instantiated_matches(
    module: &Module,
    expected: TypeId,
    actual: TypeId,
    owner: GenericOwner,
    args: &[TypeId],
    state: &mut State<'_>,
) -> bool {
    matches_type(module, expected, actual, &substitution(owner, args), state).unwrap_or(false)
}
