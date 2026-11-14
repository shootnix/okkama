//! Structural construction and editing, without ownership inference or semantic
//! verification. `finish` returns ordinary IR; #564 must verify phase outputs.
use std::collections::{HashMap, HashSet};
use crate::*;
use crate::visit::{Rewrite, VisitorMut};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    Arena(ArenaError), Contract(ContractError), Traversal(cfg::TraversalError),
    NoCurrentBlock, Terminated(BlockId), Unterminated(Vec<BlockId>),
    WrongScope(ScopeId), AlreadyDefined(FuncId), ExpectedDeclaration,
    ResultArity { expected: usize, actual: usize },
    ResultType { index: usize, expected: TypeId, actual: TypeId },
    UnresolvableType(ResultType), InvalidField(FieldRef), TooManyResults,
    EdgeArity { block: BlockId, expected: usize, actual: usize },
    EdgeType { block: BlockId, index: usize, expected: TypeId, actual: TypeId },
    InvalidEdgeResult(EdgeResult), NoUnwindOutcome, MissingUnwind,
    NeedsInvoke, NeedsNewBlock(BlockId), InvalidPosition, DuplicateInstruction(InstId),
}
impl From<ArenaError> for BuildError { fn from(e: ArenaError) -> Self { Self::Arena(e) } }
impl From<ContractError> for BuildError { fn from(e: ContractError) -> Self { Self::Contract(e) } }
impl From<cfg::TraversalError> for BuildError { fn from(e: cfg::TraversalError) -> Self { Self::Traversal(e) } }
impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for BuildError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultSpec { pub ty: TypeId, pub facts: ValueFacts }
impl ResultSpec {
    pub fn new(ty: TypeId) -> Self { Self { ty, facts: ValueFacts::Unelaborated } }
}
pub type ParamSpec = ResultSpec;

#[derive(Debug)]
pub struct ModuleBuilder { module: Module }
impl ModuleBuilder {
    pub fn new(name: impl Into<String>, profile: Profile) -> Self {
        Self { module: Module { name: name.into(), version: None, profile,
            requires: vec![], imports: vec![], declarations: Arena::new(), types: TypeTable::new(),
            scopes: Arena::new(), summaries: Arena::new(), protocols: Arena::new(),
            functions: Arena::new(), entry: None, tests: None, files: Arena::new() } }
    }
    pub fn module(&self) -> &Module { &self.module }
    /// Explicit unchecked structure editing, also for verifier negative tests.
    pub fn raw_mut(&mut self) -> &mut Module { &mut self.module }
    pub fn declare(&mut self, decl: Declaration) -> Result<DeclId, BuildError> { Ok(self.module.declarations.alloc(decl)?) }
    pub fn intern_type(&mut self, ty: Type) -> Result<TypeId, BuildError> { Ok(self.module.types.intern(ty)?) }
    pub fn add_summary(&mut self, summary: Summary) -> Result<SummaryId, BuildError> { Ok(self.module.summaries.alloc(summary)?) }
    pub fn add_protocol(&mut self, protocol: ProtocolDecl) -> Result<ProtocolId, BuildError> { Ok(self.module.protocols.alloc(protocol)?) }
    pub fn add_file(&mut self, file: SourceFileDecl) -> Result<FileId, BuildError> { Ok(self.module.files.alloc(file)?) }
    pub fn declare_scope(&mut self, scope: ScopeDecl) -> Result<ScopeId, BuildError> {
        self.module.functions.get(scope.function)?;
        if let Some(parent) = scope.parent
            && self.module.scopes.get(parent)?.function != scope.function { return Err(BuildError::WrongScope(parent)); }
        Ok(self.module.scopes.alloc(scope)?)
    }
    pub fn declare_function(&mut self, function: Function) -> Result<FuncId, BuildError> {
        if function.body.is_some() { return Err(BuildError::ExpectedDeclaration); }
        for ty in function.signature.params.iter().chain(&function.signature.results) { self.module.types.get(*ty)?; }
        Ok(self.module.functions.alloc(function)?)
    }
    pub fn define_function(&mut self, function: FuncId, body: Body) -> Result<(), BuildError> {
        let decl = self.module.functions.get(function)?;
        if decl.body.is_some() { return Err(BuildError::AlreadyDefined(function)); }
        let types = param_types(&body, body.entry)?;
        check_result_types(&decl.signature.params, &types)?;
        self.module.functions.get_mut(function)?.body = Some(body);
        Ok(())
    }
    pub fn function_builder(&mut self, function: FuncId, scope: ScopeId) -> Result<FunctionBuilder<'_>, BuildError> {
        FunctionBuilder::new(&mut self.module, function, scope)
    }
    /// No semantic verification, including for declarations without bodies.
    pub fn finish(self) -> Result<Module, BuildError> { Ok(self.module) }
}

#[derive(Debug)]
pub struct FunctionBuilder<'a> {
    module: &'a mut Module, function: FuncId, body: Body,
    current: Option<BlockId>, location: Option<DebugLoc>,
    terminated: HashSet<BlockId>, transitioned: HashSet<BlockId>,
}
impl<'a> FunctionBuilder<'a> {
    pub fn new(module: &'a mut Module, function: FuncId, scope: ScopeId) -> Result<Self, BuildError> {
        let declaration = module.functions.get(function)?;
        if declaration.body.is_some() { return Err(BuildError::AlreadyDefined(function)); }
        let params: Vec<_> = declaration.signature.params.iter().copied().map(ParamSpec::new).collect();
        let body = Body { entry: BlockId::from_u32(0), values: Arena::new(), blocks: Arena::new(),
            insts: Arena::new(), bindings: Arena::new(), boundaries: Arena::new(),
            deps: Arena::new(), keys: Arena::new(), selections: Arena::new() };
        let mut builder = Self { module, function, body, current: None, location: None,
            terminated: HashSet::new(), transitioned: HashSet::new() };
        let entry = builder.create_block(scope, &params)?;
        builder.body.entry = entry;
        builder.switch_to(entry)?;
        Ok(builder)
    }
    pub fn body(&self) -> &Body { &self.body }
    pub fn module(&self) -> &Module { self.module }
    pub fn entry(&self) -> BlockId { self.body.entry }
    pub fn intern_type(&mut self, ty: Type) -> Result<TypeId, BuildError> { Ok(self.module.types.intern(ty)?) }
    pub fn declare_binding(&mut self, binding: BindingDecl) -> Result<BindingId, BuildError> {
        self.check_scope(binding.scope)?; self.module.types.get(binding.ty)?;
        Ok(self.body.bindings.alloc(binding)?)
    }
    pub fn declare_boundary(&mut self, boundary: Boundary) -> Result<BoundaryId, BuildError> {
        self.check_scope(boundary.scope)?; Ok(self.body.boundaries.alloc(boundary)?)
    }
    pub fn declare_key(&mut self, key: KeyExpr) -> Result<KeyExprId, BuildError> {
        check_references(&key, self.module, &self.body)?; Ok(self.body.keys.alloc(key)?)
    }
    pub fn intern_dependency(&mut self, dep: DepExpr) -> Result<DepExprId, BuildError> {
        check_references(&dep, self.module, &self.body)?;
        if let Some((id, _)) = self.body.deps.iter().find(|(_, existing)| **existing == dep) { return Ok(id); }
        Ok(self.body.deps.alloc(dep)?)
    }
    pub fn declare_selection(&mut self, selection: SelectionExpr) -> Result<SelectionId, BuildError> {
        check_references(&selection, self.module, &self.body)?; Ok(self.body.selections.alloc(selection)?)
    }
    pub fn set_value_facts(&mut self, value: ValueId, facts: ValueFacts) -> Result<(), BuildError> {
        check_references(&facts, self.module, &self.body)?;
        self.body.values.get_mut(value)?.facts = facts; Ok(())
    }
    fn check_scope(&self, scope: ScopeId) -> Result<(), BuildError> {
        if self.module.scopes.get(scope)?.function != self.function { return Err(BuildError::WrongScope(scope)); }
        Ok(())
    }
    pub fn create_block(&mut self, scope: ScopeId, params: &[ParamSpec]) -> Result<BlockId, BuildError> {
        self.check_scope(scope)?;
        for param in params { self.module.types.get(param.ty)?; check_references(&param.facts, self.module, &self.body)?; }
        append_block(&mut self.body, scope, params)
    }
    pub fn block_params(&self, block: BlockId) -> Result<&[ValueId], BuildError> { Ok(&self.body.blocks.get(block)?.params) }
    pub fn switch_to(&mut self, block: BlockId) -> Result<(), BuildError> {
        self.body.blocks.get(block)?; self.current = Some(block); Ok(())
    }
    pub fn set_location(&mut self, loc: Option<DebugLoc>) { self.location = loc; }
    fn open_block(&self) -> Result<BlockId, BuildError> {
        let block = self.current.ok_or(BuildError::NoCurrentBlock)?;
        if self.terminated.contains(&block) { return Err(BuildError::Terminated(block)); }
        Ok(block)
    }
    pub fn set_unwind(&mut self, unwind: Option<BlockUnwind>) -> Result<(), BuildError> {
        let block = self.open_block()?;
        // A block's unwind state describes its entry, not a point after a move.
        if !self.body.blocks.get(block)?.insts.is_empty() { return Err(BuildError::NeedsNewBlock(block)); }
        if let Some(unwind) = &unwind { validate_block_unwind(self.module, &self.body, unwind)?; }
        self.body.blocks.get_mut(block)?.unwind = unwind; Ok(())
    }
    pub fn result_schema(&mut self, op: &Operation, outcome: Outcome) -> Result<Vec<ResultSpec>, BuildError> {
        check_operation(op, self.module, &self.body)?;
        let contract = ContractRegistry::new(self.module).construction_schema(op, &self.body)?;
        let schema = contract.outcome(outcome).ok_or(BuildError::NoUnwindOutcome)?;
        result_specs(self.module, &self.body, &schema.results)
    }
    pub fn emit(&mut self, op: Operation, results: &[ResultSpec]) -> Result<Vec<ValueId>, BuildError> {
        self.emit_with_facts(op, results, InstructionFacts::default())
    }
    pub fn emit_with_facts(&mut self, op: Operation, results: &[ResultSpec], facts: InstructionFacts) -> Result<Vec<ValueId>, BuildError> {
        let block = self.open_block()?;
        let contract = validate_instruction(self.module, &self.body, &op, results, &facts)?;
        if let Some(unwind) = &contract.unwind {
            if self.transitioned.contains(&block) { return Err(BuildError::NeedsNewBlock(block)); }
            if unwind.results.iter().any(|slot| !matches!(slot.ty, ResultType::SameAs(_) | ResultType::Service(ServiceType::Failure))) {
                return Err(BuildError::NeedsInvoke);
            }
            if self.body.blocks.get(block)?.unwind.is_none() { return Err(BuildError::MissingUnwind); }
        }
        let (inst, values) = append_instruction(&mut self.body, op, results, facts, self.location)?;
        self.body.blocks.get_mut(block)?.insts.push(inst);
        if contract.operands.iter().any(|operand| operand.mode == OperandMode::Consume) { self.transitioned.insert(block); }
        Ok(values)
    }
    /// Evaluate the registry's type formulas, without ownership/type inference.
    pub fn emit_schema(&mut self, op: Operation) -> Result<Vec<ValueId>, BuildError> {
        let results = self.result_schema(&op, Outcome::Normal)?; self.emit(op, &results)
    }
    pub fn terminate(&mut self, term: TermKind) -> Result<(), BuildError> {
        let block = self.open_block()?;
        let signature = self.module.functions.get(self.function)?.signature.clone();
        validate_term(self.module, &self.body, &term, Some(&signature))?;
        self.body.blocks.get_mut(block)?.term = Terminator { kind: term, loc: self.location };
        self.terminated.insert(block); Ok(())
    }
    pub fn invoke_own(&mut self, op: Operation, normal: InvokeEdge, unwind: InvokeEdge) -> Result<(), BuildError> {
        self.terminate(TermKind::InvokeOwn { op: Box::new(op), normal, unwind })
    }
    /// Create one parameter per outcome slot, in registry order. The source
    /// block gets no SSA result; parameters belong exclusively to successors.
    pub fn invoke_own_successors(&mut self, op: Operation, normal_scope: ScopeId, unwind_scope: ScopeId) -> Result<InvokeSuccessors, BuildError> {
        self.open_block()?; self.check_scope(normal_scope)?; self.check_scope(unwind_scope)?;
        let normal_specs = self.result_schema(&op, Outcome::Normal)?;
        let unwind_specs = self.result_schema(&op, Outcome::Unwind)?;
        let normal = self.create_block(normal_scope, &normal_specs)?;
        let unwind = self.create_block(unwind_scope, &unwind_specs)?;
        let normal_params = self.block_params(normal)?.to_vec();
        let unwind_params = self.block_params(unwind)?.to_vec();
        self.invoke_own(op, schema_edge(normal, Outcome::Normal, normal_specs.len())?, schema_edge(unwind, Outcome::Unwind, unwind_specs.len())?)?;
        Ok(InvokeSuccessors { normal, normal_params, unwind, unwind_params })
    }
    /// Requires a terminator in every draft block. Does not verify dominance,
    /// ownership, summaries, protocols, or any phase-specific semantic rules.
    pub fn finish(self) -> Result<Body, BuildError> {
        let missing: Vec<_> = self.body.blocks.iter().filter_map(|(id, _)| (!self.terminated.contains(&id)).then_some(id)).collect();
        if !missing.is_empty() { return Err(BuildError::Unterminated(missing)); }
        Ok(self.body)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvokeSuccessors {
    pub normal: BlockId, pub normal_params: Vec<ValueId>,
    pub unwind: BlockId, pub unwind_params: Vec<ValueId>,
}
fn schema_edge(block: BlockId, outcome: Outcome, count: usize) -> Result<InvokeEdge, BuildError> {
    let args = (0..count).map(|index| Ok(EdgeArg::Produced(EdgeResult { outcome,
        slot: u16::try_from(index).map_err(|_| BuildError::TooManyResults)? }))).collect::<Result<_, BuildError>>()?;
    Ok(InvokeEdge { block, args })
}
fn append_block(body: &mut Body, scope: ScopeId, params: &[ParamSpec]) -> Result<BlockId, BuildError> {
    let block = body.blocks.alloc(Block { lexical_scope: scope, params: vec![], unwind: None,
        insts: vec![], term: Terminator { kind: TermKind::Unreachable, loc: None } })?;
    for (index, param) in params.iter().enumerate() {
        let value = body.values.alloc(ValueData { ty: param.ty, facts: param.facts.clone(),
            def: Definition::BlockParam { block, index: u32::try_from(index).map_err(|_| BuildError::TooManyResults)? } })?;
        body.blocks.get_mut(block)?.params.push(value);
    }
    Ok(block)
}
fn append_instruction(body: &mut Body, op: Operation, results: &[ResultSpec], facts: InstructionFacts, loc: Option<DebugLoc>) -> Result<(InstId, Vec<ValueId>), BuildError> {
    if results.len() > usize::from(u16::MAX) + 1 { return Err(BuildError::TooManyResults); }
    let inst = body.insts.alloc(Instruction { results: vec![], op, facts, loc })?;
    let mut values = vec![];
    for (index, result) in results.iter().enumerate() {
        values.push(body.values.alloc(ValueData { ty: result.ty, facts: result.facts.clone(),
            def: Definition::InstResult { inst, index: index as u16 } })?);
    }
    body.insts.get_mut(inst)?.results.clone_from(&values);
    Ok((inst, values))
}

/// Thin conveniences: all result types come from the operation contract. No
/// summary inference, unique-cache proof synthesis or cleanup insertion occurs.
#[derive(Debug)]
pub struct LoweringBuilder<'b, 'm> { builder: &'b mut FunctionBuilder<'m> }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitResults { pub rest: ValueId, pub selected: ValueId, pub external_deps: ValueId }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScopeSplitResults { pub state: ValueId, pub selected: ValueId, pub external_deps: ValueId }
impl<'b, 'm> LoweringBuilder<'b, 'm> {
    pub fn new(builder: &'b mut FunctionBuilder<'m>) -> Self { Self { builder } }
    pub fn scope_begin(&mut self, scope: ScopeId) -> Result<ValueId, BuildError> {
        Ok(self.builder.emit_schema(Operation::Ownership(OwnershipOp::ScopeBegin { scope }))?[0])
    }
    pub fn own_empty(&mut self) -> Result<ValueId, BuildError> {
        Ok(self.builder.emit_schema(Operation::Ownership(OwnershipOp::Empty {}))?[0])
    }
    pub fn own_admit(&mut self, state: ValueId, packet: ValueId) -> Result<ValueId, BuildError> {
        Ok(self.builder.emit_schema(Operation::Ownership(OwnershipOp::Admit { state, packet }))?[0])
    }
    pub fn own_accept(&mut self, state: ValueId, packet: ValueId) -> Result<ValueId, BuildError> {
        Ok(self.builder.emit_schema(Operation::Ownership(OwnershipOp::Accept { state, packet }))?[0])
    }
    pub fn own_split(&mut self, packet: ValueId, selection: SelectionId) -> Result<SplitResults, BuildError> {
        let values = self.builder.emit_schema(Operation::Ownership(OwnershipOp::Split { packet, selection }))?;
        Ok(SplitResults { rest: values[0], selected: values[1], external_deps: values[2] })
    }
    /// A checked transition: configure an entry-state block unwind first, or
    /// use FunctionBuilder::invoke_own_successors for explicit edge results.
    pub fn scope_split(&mut self, state: ValueId, selection: SelectionId) -> Result<ScopeSplitResults, BuildError> {
        let values = self.builder.emit_schema(Operation::Ownership(OwnershipOp::ScopeSplit { state, selection }))?;
        Ok(ScopeSplitResults { state: values[0], selected: values[1], external_deps: values[2] })
    }
    pub fn bind_high(&mut self, binding: BindingId, value: ValueId) -> Result<(), BuildError> {
        self.builder.emit(Operation::Lexical(LexicalOp::Bind { binding, value }), &[])?; Ok(())
    }
    pub fn rebind_high(&mut self, binding: BindingId, value: ValueId, target: ScopeId) -> Result<(), BuildError> {
        self.builder.emit(Operation::Lexical(LexicalOp::Rebind { binding, value, target }), &[])?; Ok(())
    }
    pub fn stage_high(&mut self, action: ScopeId, field: FieldRef, value: ValueId) -> Result<(), BuildError> {
        self.builder.emit(Operation::Lexical(LexicalOp::Stage { action, field, value }), &[])?; Ok(())
    }
    pub fn demand(&mut self, value: ValueId, demand: ResultDemand) -> Result<(), BuildError> {
        let op = match demand { ResultDemand::Want => LexicalOp::Want { value }, ResultDemand::Discard => LexicalOp::Discard { value } };
        self.builder.emit(Operation::Lexical(op), &[])?; Ok(())
    }
}

fn value_type(body: &Body, value: ValueId) -> Result<TypeId, BuildError> { Ok(body.values.get(value)?.ty) }
fn param_types(body: &Body, block: BlockId) -> Result<Vec<TypeId>, BuildError> {
    body.blocks.get(block)?.params.iter().map(|&value| value_type(body, value)).collect()
}
fn check_result_types(expected: &[TypeId], actual: &[TypeId]) -> Result<(), BuildError> {
    if expected.len() != actual.len() { return Err(BuildError::ResultArity { expected: expected.len(), actual: actual.len() }); }
    for (index, (&expected, &actual)) in expected.iter().zip(actual).enumerate() {
        if expected != actual { return Err(BuildError::ResultType { index, expected, actual }); }
    }
    Ok(())
}
fn result_specs(module: &mut Module, body: &Body, schema: &[ResultSlot]) -> Result<Vec<ResultSpec>, BuildError> {
    if schema.len() > usize::from(u16::MAX) + 1 { return Err(BuildError::TooManyResults); }
    schema.iter().map(|slot| Ok(ResultSpec::new(resolve_result_type(module, body, &slot.ty)?))).collect()
}
/// Evaluate a declarative type formula. This is schema resolution, not inference
/// of ownership, dependencies, summaries or result relations.
pub fn resolve_result_type(module: &mut Module, body: &Body, formula: &ResultType) -> Result<TypeId, BuildError> {
    use ResultType as R;
    let input = |module: &Module, value| -> Result<Type, BuildError> { Ok(module.types.get(value_type(body, value)?)?.clone()) };
    let ty = match formula {
        R::Exact(ty) => { module.types.get(*ty)?; return Ok(*ty); }
        R::SameAs(value) => { let ty = value_type(body, *value)?; module.types.get(ty)?; return Ok(ty); }
        R::ExactEnum(decl) => Type::Enum(*decl),
        R::Scalar(scalar) => match scalar { ScalarType::Int => Type::Int, ScalarType::Bool => Type::Bool,
            ScalarType::Byte => Type::Byte, ScalarType::Float => Type::Float, ScalarType::Rat => Type::Rat,
            ScalarType::Char => Type::Char, ScalarType::Str => Type::Str, ScalarType::Range => Type::Range,
            ScalarType::Any => Type::Any, ScalarType::Descriptor => Type::Descriptor },
        R::Service(service) => Type::Service(*service),
        R::Element(value) => match input(module, *value)? { Type::Array(ty) => { return Ok(ty); }, _ => return Err(BuildError::UnresolvableType(formula.clone())) },
        R::MapKey(value) | R::MapValue(value) => match input(module, *value)? {
            Type::Map(key, value) => { return Ok(if matches!(formula, R::MapKey(_)) { key } else { value }); }
            _ => return Err(BuildError::UnresolvableType(formula.clone())),
        },
        R::Field(field) => match &module.declarations.get(field.declaration)?.kind {
            DeclarationKind::Object { fields, .. } => { return fields.get(field.index as usize).map(|field| field.ty).ok_or(BuildError::InvalidField(*field)); }
            _ => return Err(BuildError::InvalidField(*field)),
        },
        R::ArrayOf(element) => Type::Array(resolve_result_type(module, body, element)?),
        R::Object { decl, args } => Type::Object { decl: *decl, args: args.clone() },
        R::FutureOf(result) => Type::Future(*result),
        R::Awaited(value) => match input(module, *value)? { Type::Future(Some(ty)) => { return Ok(ty); }, _ => return Err(BuildError::UnresolvableType(formula.clone())) },
        R::SuccessorOf(value) => Type::Successor(value_type(body, *value)?),
        R::BuilderOf(ty) => Type::Builder(*ty), R::MapBuilderOf(key, value) => Type::MapBuilder(*key, *value),
        R::Finished(value) => match input(module, *value)? { Type::Builder(ty) => Type::Array(ty), Type::MapBuilder(k, v) => Type::Map(k, v), _ => return Err(BuildError::UnresolvableType(formula.clone())) },
        R::Frame(function) => Type::Frame(*function), R::CellOf(ty) => Type::LowCell(*ty),
        R::CellValue(value) => match input(module, *value)? { Type::LowCell(ty) => { return Ok(ty); }, _ => return Err(BuildError::UnresolvableType(formula.clone())) },
    };
    Ok(module.types.intern(ty)?)
}

fn check_references<T: Rewrite + Clone>(item: &T, module: &Module, body: &Body) -> Result<(), BuildError> {
    struct Check<'a> { module: &'a Module, body: &'a Body, error: Option<ArenaError> }
    macro_rules! check {
        ($($method:ident : $ty:ty => $owner:ident.$arena:ident),* $(,)?) => {$(
            fn $method(&mut self, id: &mut $ty) {
                if self.error.is_none() { self.error = self.$owner.$arena.get(*id).err(); }
            }
        )*};
    }
    impl VisitorMut for Check<'_> {
        check!(value: ValueId => body.values, instruction: InstId => body.insts,
            block: BlockId => body.blocks, binding: BindingId => body.bindings,
            boundary: BoundaryId => body.boundaries, dependency: DepExprId => body.deps,
            key: KeyExprId => body.keys, selection: SelectionId => body.selections,
            ty: TypeId => module.types, declaration: DeclId => module.declarations,
            function: FuncId => module.functions, scope: ScopeId => module.scopes,
            summary: SummaryId => module.summaries, protocol: ProtocolId => module.protocols,
            file: FileId => module.files);
    }
    let mut check = Check { module, body, error: None };
    item.clone().rewrite(&mut check);
    if let Some(error) = check.error { return Err(error.into()); } Ok(())
}
fn check_operation(op: &Operation, module: &Module, body: &Body) -> Result<(), BuildError> {
    check_references(op, module, body)?;
    for operand in cfg::operation_uses(op, body)? { module.types.get(value_type(body, operand.value)?)?; }
    Ok(())
}
fn validate_instruction(module: &mut Module, body: &Body, op: &Operation, results: &[ResultSpec], facts: &InstructionFacts) -> Result<OpContract, BuildError> {
    check_operation(op, module, body)?;
    check_references(facts, module, body)?;
    let contract = ContractRegistry::new(module).construction_schema(op, body)?;
    let expected = result_specs(module, body, &contract.normal.results)?;
    check_result_types(&expected.iter().map(|result| result.ty).collect::<Vec<_>>(), &results.iter().map(|result| result.ty).collect::<Vec<_>>())?;
    for result in results { check_references(&result.facts, module, body)?; }
    if facts.result_deps.iter().any(|(slot, _)| usize::from(*slot) >= results.len()) { return Err(BuildError::TooManyResults); }
    Ok(contract)
}
fn validate_edge(body: &Body, block: BlockId, args: &[TypeId]) -> Result<(), BuildError> {
    let expected = param_types(body, block)?;
    if args.len() != expected.len() { return Err(BuildError::EdgeArity { block, expected: expected.len(), actual: args.len() }); }
    for (index, (&expected, &actual)) in expected.iter().zip(args).enumerate() {
        if expected != actual { return Err(BuildError::EdgeType { block, index, expected, actual }); }
    }
    Ok(())
}
fn validate_target(body: &Body, target: &Target) -> Result<(), BuildError> {
    let args = target.args.iter().map(|&value| value_type(body, value)).collect::<Result<Vec<_>, _>>()?;
    validate_edge(body, target.block, &args)
}
fn validate_block_unwind(module: &mut Module, body: &Body, unwind: &BlockUnwind) -> Result<(), BuildError> {
    let mut args = unwind.target.args.iter().map(|&value| value_type(body, value)).collect::<Result<Vec<_>, _>>()?;
    args.push(module.types.intern(Type::Service(ServiceType::Failure))?);
    validate_edge(body, unwind.target.block, &args)
}
fn validate_invoke_edge(module: &mut Module, body: &Body, edge: &InvokeEdge, outcome: Outcome, schema: &[ResultSlot]) -> Result<(), BuildError> {
    let mut args = vec![];
    for arg in &edge.args {
        args.push(match arg {
            EdgeArg::Value(value) => value_type(body, *value)?,
            EdgeArg::Produced(result) => {
                if result.outcome != outcome { return Err(BuildError::InvalidEdgeResult(*result)); }
                let slot = schema.get(usize::from(result.slot)).ok_or(BuildError::InvalidEdgeResult(*result))?;
                resolve_result_type(module, body, &slot.ty)?
            }
        });
    }
    validate_edge(body, edge.block, &args)
}
fn validate_term(module: &mut Module, body: &Body, term: &TermKind, signature: Option<&Signature>) -> Result<(), BuildError> {
    check_references(term, module, body)?;
    match term {
        TermKind::Br(target) => validate_target(body, target)?,
        TermKind::Cond { condition, yes, no } => {
            check_result_types(&[module.types.intern(Type::Bool)?], &[value_type(body, *condition)?])?;
            validate_target(body, yes)?; validate_target(body, no)?;
        }
        TermKind::Switch { cases, default, .. } => { for case in cases { validate_target(body, &case.target)?; } validate_target(body, default)?; }
        TermKind::InvokeOwn { op, normal, unwind } => {
            check_operation(op, module, body)?;
            let contract = ContractRegistry::new(module).construction_schema(op, body)?;
            let failure = contract.unwind.as_ref().ok_or(BuildError::NoUnwindOutcome)?;
            validate_invoke_edge(module, body, normal, Outcome::Normal, &contract.normal.results)?;
            validate_invoke_edge(module, body, unwind, Outcome::Unwind, &failure.results)?;
        }
        TermKind::Ret { values } => {
            if let Some(signature) = signature { check_result_types(&signature.results, &values.iter().map(|&value| value_type(body, value)).collect::<Result<Vec<_>, _>>()?)?; }
        }
        TermKind::Catch { ty, handler, next, .. } => {
            validate_invoke_edge(module, body, handler, Outcome::Normal, &[ResultSlot::new(SlotName::Value, ResultType::Exact(*ty))])?;
            validate_target(body, next)?;
        }
        TermKind::Lookup { kind, collection, found, missing, .. } => {
            let ty = match kind { LookupKind::Array => ResultType::Element(*collection), LookupKind::Map => ResultType::MapValue(*collection) };
            validate_invoke_edge(module, body, found, Outcome::Normal, &[ResultSlot::new(SlotName::Value, ty)])?;
            validate_target(body, missing)?;
        }
        TermKind::Throw(_) | TermKind::Raise(_) | TermKind::Panic(_) | TermKind::Trap { .. } | TermKind::Unreachable => {},
    }
    Ok(())
}

/// Ordinary edits preserve IDs. Explicit renumbering returns a complete map,
/// including identity mappings, for updating clients' analysis side tables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdRemap {
    pub blocks: HashMap<BlockId, BlockId>,
    pub instructions: HashMap<InstId, InstId>,
    pub values: HashMap<ValueId, ValueId>,
}
impl VisitorMut for IdRemap {
    fn block(&mut self, id: &mut BlockId) { if let Some(new) = self.blocks.get(id) { *id = *new; } }
    fn instruction(&mut self, id: &mut InstId) { if let Some(new) = self.instructions.get(id) { *id = *new; } }
    fn value(&mut self, id: &mut ValueId) { if let Some(new) = self.values.get(id) { *id = *new; } }
}

/// Edit a detached body, then install it with ModuleBuilder::define_function or
/// assign it back to Function::body. Semantic validation belongs to #564.
#[derive(Debug)]
pub struct FunctionEditor<'a> { module: &'a mut Module, body: &'a mut Body }
impl<'a> FunctionEditor<'a> {
    pub fn new(module: &'a mut Module, body: &'a mut Body) -> Self { Self { module, body } }
    pub fn body(&self) -> &Body { self.body }
    /// Explicit unchecked editing for transformations and verifier negatives.
    pub fn raw_mut(&mut self) -> &mut Body { self.body }
    pub fn insert_instruction(&mut self, block: BlockId, position: usize, op: Operation, results: &[ResultSpec], facts: InstructionFacts, loc: Option<DebugLoc>) -> Result<(InstId, Vec<ValueId>), BuildError> {
        let data = self.body.blocks.get(block)?;
        if position > data.insts.len() { return Err(BuildError::InvalidPosition); }
        validate_instruction(self.module, self.body, &op, results, &facts)?;
        let mut ops = data.insts.iter().map(|&id| Ok(&self.body.insts.get(id)?.op)).collect::<Result<Vec<_>, BuildError>>()?;
        ops.insert(position, &op);
        validate_flow(self.module, self.body, &ops, data.unwind.is_some(), block)?;
        let (inst, values) = append_instruction(self.body, op, results, facts, loc)?;
        self.body.blocks.get_mut(block)?.insts.insert(position, inst);
        Ok((inst, values))
    }
    /// Keep existing result definitions and facts. Changing the result shape
    /// requires inserting a new instruction and explicitly replacing its uses.
    pub fn replace_instruction(&mut self, inst: InstId, op: Operation) -> Result<(), BuildError> {
        let old = self.body.insts.get(inst)?;
        let results = old.results.iter().map(|&value| {
            let data = self.body.values.get(value)?; Ok(ResultSpec { ty: data.ty, facts: data.facts.clone() })
        }).collect::<Result<Vec<_>, BuildError>>()?;
        let facts = old.facts.clone();
        validate_instruction(self.module, self.body, &op, &results, &facts)?;
        for (block, data) in self.body.blocks.iter() {
            if data.insts.contains(&inst) {
                let ops = data.insts.iter().map(|&id| if id == inst { Ok(&op) } else { Ok(&self.body.insts.get(id)?.op) }).collect::<Result<Vec<_>, BuildError>>()?;
                validate_flow(self.module, self.body, &ops, data.unwind.is_some(), block)?;
            }
        }
        self.body.insts.get_mut(inst)?.op = op; Ok(())
    }
    /// Move suffix and terminator, preserve instruction/value IDs, inherit scope
    /// and block unwind, and branch from the prefix to the suffix.
    pub fn split_block(&mut self, block: BlockId, position: usize) -> Result<BlockId, BuildError> {
        let data = self.body.blocks.get(block)?;
        if position > data.insts.len() { return Err(BuildError::InvalidPosition); }
        let suffix = Block { lexical_scope: data.lexical_scope, params: vec![], unwind: data.unwind.clone(),
            insts: data.insts[position..].to_vec(), term: data.term.clone() };
        let next = self.body.blocks.alloc(suffix)?;
        let prefix = self.body.blocks.get_mut(block)?;
        prefix.insts.truncate(position);
        prefix.term = Terminator { kind: TermKind::Br(Target { block: next, args: vec![] }), loc: None };
        Ok(next)
    }
    pub fn replace_uses(&mut self, old: ValueId, new: ValueId) -> Result<(), BuildError> {
        check_result_types(&[value_type(self.body, old)?], &[value_type(self.body, new)?])?;
        struct Replace { old: ValueId, new: ValueId }
        impl VisitorMut for Replace { fn value(&mut self, value: &mut ValueId) { if *value == self.old { *value = self.new; } } }
        visit::rewrite_uses(self.body, &mut Replace { old, new }); Ok(())
    }
    /// Replace explicit arguments of one edge. Block-unwind's implicit failure
    /// is excluded; produced slots are legal only on invoke-like edges.
    pub fn replace_edge_arguments(&mut self, block: BlockId, kind: cfg::EdgeKind, args: Vec<EdgeArg>) -> Result<(), BuildError> {
        use cfg::EdgeKind as E;
        let data = self.body.blocks.get(block)?;
        if kind == E::BlockUnwind {
            let mut unwind = data.unwind.clone().ok_or(BuildError::InvalidPosition)?;
            unwind.target.args = ordinary_args(args)?;
            validate_block_unwind(self.module, self.body, &unwind)?;
            self.body.blocks.get_mut(block)?.unwind = Some(unwind); return Ok(());
        }
        let mut term = data.term.clone();
        match (&mut term.kind, kind) {
            (TermKind::Br(target), E::Branch) | (TermKind::Cond { yes: target, .. }, E::Yes) |
            (TermKind::Cond { no: target, .. }, E::No) | (TermKind::Switch { default: target, .. }, E::Default) |
            (TermKind::Catch { next: target, .. }, E::CatchNext) | (TermKind::Lookup { missing: target, .. }, E::LookupMissing) => target.args = ordinary_args(args)?,
            (TermKind::Switch { cases, .. }, E::SwitchCase(index)) => cases.get_mut(index as usize).ok_or(BuildError::InvalidPosition)?.target.args = ordinary_args(args)?,
            (TermKind::InvokeOwn { normal: edge, .. }, E::InvokeNormal) | (TermKind::InvokeOwn { unwind: edge, .. }, E::InvokeUnwind) |
            (TermKind::Catch { handler: edge, .. }, E::CatchHandler) | (TermKind::Lookup { found: edge, .. }, E::LookupFound) => edge.args = args,
            _ => return Err(BuildError::InvalidPosition),
        }
        validate_term(self.module, self.body, &term.kind, None)?;
        self.body.blocks.get_mut(block)?.term = term; Ok(())
    }
    /// Full traversal for deliberate transformations of references and locations.
    /// Like direct editing, arbitrary visitors require semantic re-verification.
    pub fn rewrite(&mut self, visitor: &mut impl VisitorMut) { self.body.rewrite(visitor); }
    /// Renumber instructions by block layout and values by definition order.
    /// Block IDs/expression keys stay stable; unattached entries are preserved.
    pub fn renumber(&mut self) -> Result<IdRemap, BuildError> {
        let mut map = IdRemap::default();
        let mut instructions = Arena::new();
        let mut values = Arena::new();
        for (block, data) in self.body.blocks.iter() {
            map.blocks.insert(block, block);
            for &value in &data.params {
                if map.values.contains_key(&value) { return Err(BuildError::InvalidPosition); }
                map.values.insert(value, values.alloc(self.body.values.get(value)?.clone())?);
            }
            for &inst in &data.insts {
                if map.instructions.contains_key(&inst) { return Err(BuildError::DuplicateInstruction(inst)); }
                map.instructions.insert(inst, instructions.alloc(self.body.insts.get(inst)?.clone())?);
                for &value in &self.body.insts.get(inst)?.results {
                    if map.values.contains_key(&value) { return Err(BuildError::InvalidPosition); }
                    map.values.insert(value, values.alloc(self.body.values.get(value)?.clone())?);
                }
            }
        }
        for (inst, data) in self.body.insts.iter() {
            if let std::collections::hash_map::Entry::Vacant(entry) = map.instructions.entry(inst) { entry.insert(instructions.alloc(data.clone())?); }
        }
        for (value, data) in self.body.values.iter() {
            if let std::collections::hash_map::Entry::Vacant(entry) = map.values.entry(value) { entry.insert(values.alloc(data.clone())?); }
        }
        self.body.insts = instructions; self.body.values = values;
        self.body.rewrite(&mut map);
        Ok(map)
    }
}
fn ordinary_args(args: Vec<EdgeArg>) -> Result<Vec<ValueId>, BuildError> {
    args.into_iter().map(|arg| match arg { EdgeArg::Value(value) => Ok(value), EdgeArg::Produced(result) => Err(BuildError::InvalidEdgeResult(result)) }).collect()
}
fn validate_flow(module: &Module, body: &Body, ops: &[&Operation], has_unwind: bool, block: BlockId) -> Result<(), BuildError> {
    let mut transitioned = false;
    for op in ops {
        let contract = ContractRegistry::new(module).construction_schema(op, body)?;
        if let Some(unwind) = &contract.unwind {
            if transitioned { return Err(BuildError::NeedsNewBlock(block)); }
            if unwind.results.iter().any(|slot| !matches!(slot.ty, ResultType::SameAs(_) | ResultType::Service(ServiceType::Failure))) { return Err(BuildError::NeedsInvoke); }
            if !has_unwind { return Err(BuildError::MissingUnwind); }
        }
        transitioned |= contract.operands.iter().any(|operand| operand.mode == OperandMode::Consume);
    }
    Ok(())
}
