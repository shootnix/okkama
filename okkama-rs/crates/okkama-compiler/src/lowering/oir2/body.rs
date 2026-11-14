use std::collections::HashMap;

use super::{LowerError, Result};
use crate::analysis::{thir, ty::{Ty, ObjectId}};
use crate::oir2 as ir;

pub(super) struct BodyContext<'t> {
    pub functions: &'t HashMap<thir::BodyOwner, ir::FuncId>,
    pub natives: &'t HashMap<usize, ir::FuncId>,
    pub error_text: Option<ir::FuncId>,
    pub objects: &'t HashMap<ObjectId, ir::DeclId>,
    pub constructors: &'t HashMap<(ObjectId, usize, Vec<usize>), ir::FuncId>,
    pub given: Vec<usize>,
    pub listed: Option<Vec<usize>>,
    pub defers: &'t [super::defer::Deferred],
    // Some(receiver snapshot present) denotes a lazy helper rather than its source action.
    pub deferred: Option<bool>,
}

pub(super) struct BodyLowerer<'a, 't> {
    pub b: ir::FunctionBuilder<'a>,
    pub source: &'t thir::Body,
    pub types: &'t HashMap<Ty, ir::TypeId>,
    pub current: Option<ir::BlockId>,
    pub context: BodyContext<'t>,
    pub receiver: Option<ir::ValueId>,
    pub early_self: Option<ir::ValueId>,
    pub field_base: usize,
    pub field_end: usize,
    // Locals, field slots, then active catch failure views are forwarded at joins/backedges.
    // A scope exit restores the outer set; shadowed locals have distinct IDs.
    pub env: Vec<Option<ir::ValueId>>,
    pub bindings: Vec<Option<ir::BindingId>>,
    pub root: ir::ScopeId,
    return_boundary: ir::BoundaryId,
    pub failure_frames: Vec<super::failures::FailureFrame>,
    pub caught: Vec<super::failures::Caught>,
    pub rethrows: HashMap<ir::ValueId, ir::ValueId>,
    pub rethrow_inputs: HashMap<ir::ValueId, Option<ir::ValueId>>,
    failures: HashMap<Vec<(ir::ScopeId, ir::FuncId)>, ir::BlockId>,
    pub registrations: HashMap<ir::ScopeId, Vec<ir::FuncId>>,
    loops: Vec<LoopFrame>,
}

#[derive(Clone)]
pub(super) struct Join {
    pub block: ir::BlockId,
    locals: Vec<usize>,
    value: bool,
}

#[derive(Clone)]
struct LoopFrame {
    next: ir::BlockId,
    exit: Join,
    boundary: ir::BoundaryId,
    state: Vec<ir::ValueId>,
}

impl<'a, 't> BodyLowerer<'a, 't> {
    pub fn lower(
        mut b: ir::FunctionBuilder<'a>,
        source: &'t thir::Body,
        root: ir::ScopeId,
        types: &'t HashMap<Ty, ir::TypeId>,
        context: BodyContext<'t>,
    ) -> Result<ir::Body> {
        let entry = b.entry();
        let params = b.block_params(entry)?.to_vec();
        let failure_ty = b.intern_type(ir::Type::Service(ir::ServiceType::Failure))?;
        let failure = b.create_block(root, &[ir::ParamSpec::new(failure_ty)])?;
        b.switch_to(failure)?;
        b.terminate(ir::TermKind::Raise(b.block_params(failure)?[0]))?;
        b.switch_to(entry)?;
        b.set_unwind(Some(ir::BlockUnwind { target: ir::Target { block: failure, args: vec![] } }))?;
        let return_boundary = b.declare_boundary(ir::Boundary { scope: root, kind: ir::BoundaryKind::Lexical })?;
        let field_base = source.locals.len();
        let owner = match source.owner {
            thir::BodyOwner::Ctor { object, .. } | thir::BodyOwner::Method { object, .. } => Some(object),
            _ => None,
        };
        let fields = owner.map(|object| context.objects[&object]).map(|decl| {
            match &b.module().declarations.get(decl).expect("declared owner").kind {
                ir::DeclarationKind::Object { fields, .. } => {
                    super::objects::pending_index(fields).unwrap_or(fields.len())
                }
                _ => unreachable!("object owner"),
            }
        }).unwrap_or(0);
        let count = field_base + fields;
        let mut this = Self {
            b, source, types, current: Some(entry), env: vec![None; count], bindings: vec![None; count],
            root, return_boundary, failures: HashMap::from([(vec![], failure)]),
            registrations: HashMap::new(), loops: vec![], context,
            failure_frames: vec![], caught: vec![], rethrows: HashMap::new(), rethrow_inputs: HashMap::new(),
            receiver: None, early_self: None, field_base, field_end: count,
        };
        let receiver = usize::from(this.context.deferred.unwrap_or(matches!(source.owner,
            thir::BodyOwner::Method { .. } | thir::BodyOwner::Destroy { .. })));
        if receiver == 1 {
            this.receiver = params.first().copied();
        }
        if this.context.deferred.is_none() {
            this.initialize_fields()?;
        }
        if this.is_constructor() {
            this.initialize_constructor(&params)?;
        } else {
            for (local, value) in source.params.iter().zip(params.into_iter().skip(receiver)) {
                this.bind(*local, value)?;
            }
        }
        let want = !this.b.module().functions.get(this.function())?.signature.results.is_empty();
        let special = this.is_constructor() || this.is_successor();
        let out = this.block(&source.block, want && !special)?;
        if this.current.is_some() {
            this.finish_action(out)?;
        }
        Ok(this.b.finish()?)
    }

    fn function(&self) -> ir::FuncId {
        self.b.module().scopes.get(self.root).expect("allocated root scope").function
    }

    pub fn scope(&self) -> ir::ScopeId {
        let block = self.current.expect("expression is lowered only in an open block");
        self.b.body().blocks.get(block).expect("allocated block").lexical_scope
    }

    pub fn ty(&self, ty: &Ty, span: crate::source::Span) -> Result<ir::TypeId> {
        self.types.get(ty).copied().ok_or_else(|| LowerError::unsupported(span, format!("type {ty:?}")))
    }

    pub fn new_block(&mut self, scope: ir::ScopeId, types: &[ir::TypeId]) -> Result<ir::BlockId> {
        let specs: Vec<_> = types.iter().copied().map(ir::ParamSpec::new).collect();
        Ok(self.b.create_block(scope, &specs)?)
    }

    pub fn switch(&mut self, block: ir::BlockId) -> Result<()> {
        if !self.failure_frames.is_empty() {
            self.b.switch_to(block)?;
            self.b.set_unwind(None)?;
            self.current = Some(block);
            return self.sync_failure();
        }
        let mut scopes = vec![];
        let mut scope = Some(self.b.body().blocks.get(block)?.lexical_scope);
        while let Some(id) = scope {
            scopes.push(id);
            scope = self.b.module().scopes.get(id)?.parent;
        }
        let prefix: Vec<_> = scopes.into_iter().rev().flat_map(|scope| {
            self.registrations.get(&scope).into_iter().flatten().map(move |&function| (scope, function))
        }).collect();
        let failure = if let Some(&failure) = self.failures.get(&prefix) {
            failure
        } else {
            let ty = self.b.intern_type(ir::Type::Service(ir::ServiceType::Failure))?;
            let failure = self.new_block(self.root, &[ty])?;
            self.b.switch_to(failure)?;
            self.b.terminate(ir::TermKind::Raise(self.b.block_params(failure)?[0]))?;
            self.failures.insert(prefix, failure);
            failure
        };
        self.b.switch_to(block)?;
        self.b.set_unwind(Some(ir::BlockUnwind {
            target: ir::Target { block: failure, args: vec![] },
        }))?;
        self.current = Some(block);
        Ok(())
    }

    pub fn dead_block(&mut self, block: ir::BlockId) -> Result<()> {
        self.b.switch_to(block)?;
        self.b.set_unwind(None)?;
        self.b.terminate(ir::TermKind::Unreachable)?;
        self.current = None;
        Ok(())
    }

    pub fn terminate(&mut self, term: ir::TermKind) -> Result<()> {
        // A local discard still needs an ordinary edge when evaluation later diverges.
        // Keep that edge in the originating scope so cleanup is not moved outward.
        let block = self.current.expect("open terminator source");
        let discarded: Vec<_> = self.b.body().high_results.iter().filter(|record| {
            record.block == block && record.demand == ir::ResultDemand::Discard
        }).map(|record| record.value).collect();
        let edges = match &term {
            ir::TermKind::Br(_) | ir::TermKind::Ret { .. } => 1,
            ir::TermKind::Cond { .. } => 2,
            ir::TermKind::Switch { cases, .. } => cases.len() + 1,
            _ => 0,
        };
        if !discarded.is_empty() && edges == 0 {
            let next = self.new_block(self.scope(), &[])?;
            self.terminate(ir::TermKind::Br(ir::Target { block: next, args: vec![] }))?;
            self.switch(next)?;
        }
        for edge in 1..edges {
            let edge = u16::try_from(edge).map_err(|_| {
                LowerError::invalid(self.source.span, "too many result exit edges")
            })?;
            for &value in &discarded {
                self.b.add_high_result(ir::HighResultExit {
                    block, edge: ir::HighEdgeSelector::Normal(edge), value,
                    demand: ir::ResultDemand::Discard, prepared: None,
                })?;
            }
        }
        if matches!(term, ir::TermKind::Throw(_) | ir::TermKind::Raise(_)
            | ir::TermKind::Panic(_) | ir::TermKind::Trap { .. }) {
            self.sync_failure()?;
        }
        self.forward_rethrows(&term)?;
        self.b.terminate(term)?;
        self.current = None;
        Ok(())
    }

    pub fn locals(&self) -> Vec<usize> {
        self.env.iter().enumerate().filter_map(|(index, value)| value.map(|_| index)).collect()
    }

    pub fn args(&self, locals: &[usize]) -> Vec<ir::ValueId> {
        locals.iter().map(|&index| self.env[index].expect("live local on incoming edge")).collect()
    }

    pub fn local_types(&self, locals: &[usize]) -> Vec<ir::TypeId> {
        self.args(locals).iter().map(|value| self.b.body().values.get(*value).expect("live SSA value").ty).collect()
    }

    pub fn enter(&mut self, block: ir::BlockId, locals: &[usize], offset: usize) -> Result<()> {
        let params = self.b.block_params(block)?.to_vec();
        self.env.fill(None);
        for (&local, &value) in locals.iter().zip(&params[offset..]) {
            self.env[local] = Some(value);
        }
        self.switch(block)
    }

    pub(super) fn join(&mut self, scope: ir::ScopeId, value: Option<ir::TypeId>) -> Result<Join> {
        let locals = self.locals();
        let mut types: Vec<_> = value.into_iter().collect();
        types.extend(self.local_types(&locals));
        let block = self.new_block(scope, &types)?;
        Ok(Join { block, locals, value: value.is_some() })
    }

    pub(super) fn finish_join(&mut self, join: &Join) -> Result<Option<ir::ValueId>> {
        self.enter(join.block, &join.locals, usize::from(join.value))?;
        Ok(join.value.then(|| self.b.block_params(join.block).expect("allocated join")[0]))
    }

    pub(super) fn branch_join(&mut self, join: &Join, value: Option<ir::ValueId>, kind: ir::ExitKind) -> Result<()> {
        self.branch_join_at(join, value, kind, None)
    }

    fn branch_join_at(
        &mut self,
        join: &Join,
        value: Option<ir::ValueId>,
        kind: ir::ExitKind,
        boundary: Option<ir::BoundaryId>,
    ) -> Result<()> {
        let mut args = vec![];
        if join.value {
            args.push(value.ok_or_else(|| LowerError::invalid(self.source.span, "valued arm has no value"))?);
        }
        args.extend(self.args(&join.locals));
        self.edge(join.block, args, kind, boundary)
    }

    fn boundary(&mut self, scope: ir::ScopeId, kind: ir::BoundaryKind) -> Result<ir::BoundaryId> {
        Ok(self.b.declare_boundary(ir::Boundary { scope, kind })?)
    }

    /// Record both the lexical intent and its explicit D11 edge/value/preparation link.
    fn prepare(&mut self, value: Option<ir::ValueId>, kind: ir::ExitKind, boundary: ir::BoundaryId) -> Result<()> {
        let selection = match value {
            Some(value) => ir::SelectionExpr::Value(value),
            None => ir::SelectionExpr::Union(vec![]),
        };
        let selection = self.b.declare_selection(selection)?;
        if let Some(value) = value {
            self.b.emit(ir::Operation::Lexical(ir::LexicalOp::Want { value }), &[])?;
        }
        self.b.emit(ir::Operation::Exit(ir::ExitOp::Prepare {
            exit: ir::PreparedExit { kind, value, packet: None, deps: None, selection, boundary },
        }), &[])?;
        if let Some(value) = value {
            let block = self.current.expect("open prepared exit");
            let prepared = self.b.body().blocks.get(block)?.insts.last().copied();
            self.b.add_high_result(ir::HighResultExit {
                block, edge: ir::HighEdgeSelector::Normal(0), value,
                demand: ir::ResultDemand::Want, prepared,
            })?;
        }
        Ok(())
    }

    fn separate_discarded(&mut self, values: &[ir::ValueId]) -> Result<()> {
        let block = self.b.body().blocks.get(self.current.expect("open source block"))?;
        let conflict = block.insts.iter().any(|id| {
            matches!(self.b.body().insts.get(*id).map(|i| &i.op),
                Ok(ir::Operation::Lexical(ir::LexicalOp::Discard { value })) if values.contains(value))
        });
        if conflict {
            let next = self.new_block(self.scope(), &[])?;
            self.terminate(ir::TermKind::Br(ir::Target { block: next, args: vec![] }))?;
            self.switch(next)?;
        }
        Ok(())
    }

    fn edge(
        &mut self,
        target: ir::BlockId,
        args: Vec<ir::ValueId>,
        kind: ir::ExitKind,
        boundary: Option<ir::BoundaryId>,
    ) -> Result<()> {
        let source = self.scope();
        let target_scope = self.b.body().blocks.get(target)?.lexical_scope;
        if source != target_scope {
            self.separate_discarded(&args)?;
            let boundary = match boundary {
                Some(boundary) => boundary,
                None => self.boundary(source, ir::BoundaryKind::Lexical)?,
            };
            let mut prepared = std::collections::HashSet::new();
            for &value in &args {
                let ty = self.b.body().values.get(value)?.ty;
                // Active catch failures are forwarded context, not semantic language results.
                if !matches!(self.b.module().types.get(ty)?, ir::Type::Service(_)) && prepared.insert(value) {
                    self.prepare(Some(value), kind, boundary)?;
                }
            }
            if prepared.is_empty() {
                self.prepare(None, kind, boundary)?;
            }
        }
        self.terminate(ir::TermKind::Br(ir::Target { block: target, args }))
    }

    pub fn return_value(&mut self, value: Option<ir::ValueId>) -> Result<()> {
        self.separate_discarded(&value.into_iter().collect::<Vec<_>>())?;
        self.prepare(value, ir::ExitKind::Return, self.return_boundary)?;
        self.terminate(ir::TermKind::Ret { values: value.into_iter().collect() })
    }

    pub fn bind(&mut self, local: thir::LocalId, value: ir::ValueId) -> Result<()> {
        let info = &self.source.locals[local.0];
        let binding = self.b.declare_binding(ir::BindingDecl {
            name: info.name.clone(), scope: self.scope(), ty: self.ty(&info.ty, info.span)?, loc: None,
        })?;
        self.bindings[local.0] = Some(binding);
        self.env[local.0] = Some(value);
        self.b.emit(ir::Operation::Lexical(ir::LexicalOp::Bind { binding, value }), &[])?;
        Ok(())
    }

    pub(super) fn discard(&mut self, value: Option<ir::ValueId>) -> Result<()> {
        if let Some(value) = value {
            let ty = self.b.body().values.get(value)?.ty;
            let bound = self.env.contains(&Some(value)) || self.receiver == Some(value);
            if !bound && matches!(self.b.module().types.get(ty)?, ir::Type::Object { .. } | ir::Type::Successor(_)) {
                self.b.emit(ir::Operation::Lexical(ir::LexicalOp::Temp { value, scope: self.scope() }), &[])?;
            }
            self.b.emit(ir::Operation::Lexical(ir::LexicalOp::Discard { value }), &[])?;
            let block = self.current.expect("open discarded result");
            if !self.b.body().high_results.iter().any(|record| record.block == block && record.value == value) {
                self.b.add_high_result(ir::HighResultExit {
                    block, edge: ir::HighEdgeSelector::Normal(0), value,
                    demand: ir::ResultDemand::Discard, prepared: None,
                })?;
            }
        }
        Ok(())
    }

    pub(super) fn block(&mut self, block: &thir::Block, want: bool) -> Result<Option<ir::ValueId>> {
        let mut result = None;
        for (index, stmt) in block.stmts.iter().enumerate() {
            if self.current.is_none() {
                break;
            }
            // Allocate early self before the whole statement that first exposes it,
            // including nested branches, so it dominates every alias and completion.
            if self.is_constructor() && self.early_self.is_none()
                && (thir::stmt_contains(stmt, &|expr| matches!(expr.kind, thir::ExprKind::SelfValue))
                    || super::defer::uses_receiver(stmt)) {
                self.construct_self()?;
            }
            let tail = want && index + 1 == block.stmts.len();
            if let thir::StmtKind::Expr(expr) = &stmt.kind {
                result = self.expr(expr, tail)?;
                if !tail {
                    self.discard(result)?;
                    result = None;
                }
            } else {
                self.stmt(stmt)?;
                result = None;
            }
        }
        Ok(result)
    }

    fn stmt(&mut self, stmt: &thir::Stmt) -> Result<()> {
        use thir::StmtKind as S;
        match &stmt.kind {
            S::Let { once: true, .. } => return Err(LowerError::unsupported(stmt.span, "once let")),
            S::Let { local, value, .. } => {
                if let Some(value) = self.value(value)? {
                    self.bind(*local, value)?;
                }
            }
            S::Rebind { local, value } => {
                if self.context.deferred.is_some() && self.source.params.contains(local) {
                    return Err(LowerError::unsupported(stmt.span, "rebind of defer capture (#567 S3)"));
                }
                if let Some(value) = self.value(value)? {
                    let binding = self.bindings[local.0].ok_or_else(|| {
                        LowerError::invalid(stmt.span, "rebind without initial bind")
                    })?;
                    let target = self.b.body().bindings.get(binding)?.scope;
                    self.b.emit(ir::Operation::Lexical(ir::LexicalOp::Rebind { binding, value, target }), &[])?;
                    self.env[local.0] = Some(value);
                }
            }
            S::BindField { field, value } | S::StageField { field, value } => {
                if self.context.deferred.is_some() {
                    return Err(LowerError::unsupported(stmt.span, "field write from defer (#567 S3)"));
                }
                if let Some(value) = self.value(value)? {
                    self.write_field(*field, value)?;
                }
            }
            S::Return(expr) => {
                if self.context.deferred.is_some() {
                    return Err(LowerError::unsupported(stmt.span, "return from defer (#567 S3/D8)"));
                }
                let value = match expr {
                    Some(expr) => self.value(expr)?,
                    None => None,
                };
                if self.current.is_some() {
                    self.finish_action(value)?;
                }
            }
            S::Defer(expr) => self.register_defer(expr)?,
            S::Throw(expr) => self.throw_value(expr)?,
            S::Rethrow => {
                let failure = self.caught.last().ok_or_else(|| {
                    LowerError::unsupported(stmt.span, "rethrow outside a lexical catch")
                })?.failure;
                self.rethrow(failure)?;
            }
            S::Confirm { condition, fallback } => self.confirm(condition, fallback.as_ref())?,
            S::Next => self.loop_exit(None, true)?,
            S::Last(value) => self.loop_exit(value.as_ref(), false)?,
            S::Guarded { condition, stmt: guarded } => {
                let block = thir::Block { stmts: vec![(**guarded).clone()], span: stmt.span };
                self.if_expr(condition, &block, None, None, false)?;
            }
            S::Expr(expr) => {
                let out = self.expr(expr, false)?;
                self.discard(out)?;
            }
        }
        Ok(())
    }

    pub fn value(&mut self, expr: &thir::Expr) -> Result<Option<ir::ValueId>> {
        let value = self.expr(expr, true)?;
        if value.is_none() && self.current.is_some() {
            return Err(LowerError::invalid(expr.span, "value expression produced no value"));
        }
        Ok(value)
    }

    pub fn expr(&mut self, expr: &thir::Expr, want: bool) -> Result<Option<ir::ValueId>> {
        use thir::ExprKind as E;
        let result_ty = expr.ty.as_ref().filter(|ty| **ty != Ty::Never);
        match &expr.kind {
            E::Local(local) => Ok(self.env[local.0]),
            E::SelfValue => self.self_value().map(Some),
            E::Field(field) => self.read_field(*field).map(Some),
            E::PendingField(field) => self.pending_field(*field).map(Some),
            E::VariantNew { case, values } => {
                let ty = self.ty(expr.ty.as_ref().expect("typed variant"), expr.span)?;
                if let ir::Type::Enum(declaration) = *self.b.module().types.get(ty)? {
                    return self.core(ir::CoreOp::EnumCase { declaration, case: *case as u32 }).map(Some);
                }
                let mut fields = vec![];
                for field in values {
                    let Some(value) = self.value(field)? else {
                        return Ok(None);
                    };
                    fields.push(value);
                }
                self.core(ir::CoreOp::VariantNew { ty, case: *case as u32, fields, packet: None }).map(Some)
            }
            E::Pack { items, spread: None } => {
                let ty = self.ty(expr.ty.as_ref().expect("typed pack"), expr.span)?;
                let mut values = vec![];
                for item in items {
                    let Some(value) = self.value(item)? else {
                        return Ok(None);
                    };
                    values.push(value);
                }
                self.collection(ir::CollectionOp::ArrayNew { ty, values, packet: None }).map(Some)
            }
            E::Unpack(value) => self.value(value),
            E::EnumCase { ty, case } => {
                let ty = self.ty(&Ty::Enum(*ty), expr.span)?;
                let ir::Type::Enum(declaration) = *self.b.module().types.get(ty)? else {
                    return Err(LowerError::invalid(expr.span, "enum case without enum type"));
                };
                self.core(ir::CoreOp::EnumCase { declaration, case: *case as u32 }).map(Some)
            }
            E::Coerce { coercion: thir::Coercion::Take, value } => {
                let Some(value) = self.value(value)? else {
                    return Ok(None);
                };
                self.take(value).map(Some)
            }
            E::Call { callee: thir::Callee::Method { .. } | thir::Callee::Ctor { .. }
                | thir::Callee::Native(_) | thir::Callee::Role { .. }, .. } => {
                self.direct_call(expr)
            }
            E::If { condition, then_block, else_block } => {
                self.if_expr(condition, then_block, else_block.as_ref(), result_ty, want)
            }
            E::Block(block) => {
                let parent = self.scope();
                let ty = if want { result_ty.map(|ty| self.ty(ty, expr.span)).transpose()? } else { None };
                let join = self.join(parent, ty)?;
                let scope = self.b.declare_scope(parent, ir::ScopeKind::Block)?;
                let entry = self.new_block(scope, &[])?;
                self.terminate(ir::TermKind::Br(ir::Target { block: entry, args: vec![] }))?;
                self.switch(entry)?;
                let value = self.block(block, want)?;
                if self.current.is_some() {
                    self.branch_join(&join, value, ir::ExitKind::Fallthrough)?;
                    self.finish_join(&join)
                } else {
                    self.dead_block(join.block)?;
                    Ok(None)
                }
            }
            E::Logical { and, left, right } => {
                let constant = thir::Expr {
                    kind: E::Constant(thir::Constant::Bool(!and)), ty: Some(Ty::Bool),
                    diverges: false, span: expr.span,
                };
                let arm = |expr: thir::Expr| thir::Block {
                    span: expr.span, stmts: vec![thir::Stmt { span: expr.span, kind: thir::StmtKind::Expr(expr) }],
                };
                let rhs = arm((**right).clone());
                let short = arm(constant);
                let (yes, no) = if *and { (&rhs, &short) } else { (&short, &rhs) };
                self.if_expr(left, yes, Some(no), Some(&Ty::Bool), want)
            }
            E::Try(found) => self.try_expr(found, result_ty.filter(|_| want), want),
            E::Panic(message) => {
                if let Some(message) = self.value(message)? {
                    self.terminate(ir::TermKind::Panic(message))?;
                }
                Ok(None)
            }
            E::Coerce { coercion: thir::Coercion::RoleWrap, value } => {
                self.error_wrap(expr, value)
            }
            E::Match(matching) => self.match_expr(matching, result_ty, want),
            E::Loop(block) => self.loop_expr(block, result_ty.filter(|_| want)),
            E::For { source, binding, body } => {
                self.for_expr(source, *binding, body, result_ty.filter(|_| want))
            }
            E::Unreachable => {
                self.terminate(ir::TermKind::Unreachable)?;
                Ok(None)
            }
            _ => self.scalar(expr),
        }
    }

    fn if_expr(
        &mut self,
        condition: &thir::Expr,
        then_block: &thir::Block,
        else_block: Option<&thir::Block>,
        ty: Option<&Ty>,
        want: bool,
    ) -> Result<Option<ir::ValueId>> {
        let Some(condition) = self.value(condition)? else {
            return Ok(None);
        };
        let parent = self.scope();
        let ty = ty.filter(|_| want).map(|ty| self.ty(ty, then_block.span)).transpose()?;
        let join = self.join(parent, ty)?;
        let base = self.env.clone();
        let yes_scope = self.b.declare_scope(parent, ir::ScopeKind::Branch)?;
        let no_scope = self.b.declare_scope(parent, ir::ScopeKind::Branch)?;
        let types = self.local_types(&join.locals);
        let yes = self.new_block(yes_scope, &types)?;
        let no = self.new_block(no_scope, &types)?;
        let args = self.args(&join.locals);
        self.terminate(ir::TermKind::Cond {
            condition, yes: ir::Target { block: yes, args: args.clone() }, no: ir::Target { block: no, args },
        })?;
        let mut reaches = false;
        for (target, block) in [(yes, Some(then_block)), (no, else_block)] {
            self.env = base.clone();
            self.enter(target, &join.locals, 0)?;
            let out = match block {
                Some(block) => self.block(block, want)?,
                None => None,
            };
            if self.current.is_some() {
                self.branch_join(&join, out, ir::ExitKind::Fallthrough)?;
                reaches = true;
            }
        }
        if !reaches {
            self.dead_block(join.block)?;
            return Ok(None);
        }
        self.finish_join(&join)
    }

    fn match_expr(&mut self, matching: &thir::Match, ty: Option<&Ty>, want: bool) -> Result<Option<ir::ValueId>> {
        let Some(subject) = self.value(&matching.subject)? else {
            return Ok(None);
        };
        let parent = self.scope();
        let ty = ty.filter(|_| want).map(|ty| self.ty(ty, matching.subject.span)).transpose()?;
        let join = self.join(parent, ty)?;
        let mut reaches = false;
        for (patterns, block) in &matching.cases {
            let scope = self.b.declare_scope(parent, ir::ScopeKind::Branch)?;
            let types = self.local_types(&join.locals);
            let arm = self.new_block(scope, &types)?;
            for pattern in patterns {
                let Some(pattern) = self.value(pattern)? else {
                    return Err(LowerError::unsupported(pattern.span, "diverging match pattern"));
                };
                let condition = self.equal(subject, pattern, matching.subject.span)?;
                let next = self.new_block(parent, &types)?;
                let args = self.args(&join.locals);
                self.terminate(ir::TermKind::Cond {
                    condition,
                    yes: ir::Target { block: arm, args: args.clone() },
                    no: ir::Target { block: next, args },
                })?;
                self.enter(next, &join.locals, 0)?;
            }
            let test = self.current.expect("match test remains open");
            let test_env = self.env.clone();
            self.enter(arm, &join.locals, 0)?;
            let value = self.block(block, want)?;
            if self.current.is_some() {
                self.branch_join(&join, value, ir::ExitKind::Fallthrough)?;
                reaches = true;
            }
            self.env = test_env;
            self.switch(test)?;
        }
        if let Some(block) = &matching.else_block {
            let scope = self.b.declare_scope(parent, ir::ScopeKind::Branch)?;
            let entry = self.new_block(scope, &[])?;
            self.terminate(ir::TermKind::Br(ir::Target { block: entry, args: vec![] }))?;
            self.switch(entry)?;
            let value = self.block(block, want)?;
            if self.current.is_some() {
                self.branch_join(&join, value, ir::ExitKind::Fallthrough)?;
                reaches = true;
            }
        } else if matching.exhaustive {
            self.terminate(ir::TermKind::Unreachable)?;
        } else {
            self.branch_join(&join, None, ir::ExitKind::Fallthrough)?;
            reaches = true;
        }
        if !reaches {
            self.dead_block(join.block)?;
            return Ok(None);
        }
        self.finish_join(&join)
    }

    fn loop_expr(&mut self, block: &thir::Block, ty: Option<&Ty>) -> Result<Option<ir::ValueId>> {
        let parent = self.scope();
        let ty = ty.map(|ty| self.ty(ty, block.span)).transpose()?;
        let exit = self.join(parent, ty)?;
        let types = self.local_types(&exit.locals);
        let head = self.new_block(parent, &types)?;
        let scope = self.b.declare_scope(parent, ir::ScopeKind::Iteration)?;
        let iteration = self.new_block(scope, &types)?;
        let args = self.args(&exit.locals);
        self.terminate(ir::TermKind::Br(ir::Target { block: head, args }))?;
        self.enter(head, &exit.locals, 0)?;
        let args = self.args(&exit.locals);
        self.terminate(ir::TermKind::Br(ir::Target { block: iteration, args }))?;
        self.enter(iteration, &exit.locals, 0)?;
        let boundary = self.boundary(scope, ir::BoundaryKind::Loop)?;
        self.loops.push(LoopFrame { next: head, exit: exit.clone(), boundary, state: vec![] });
        self.block(block, false)?;
        if self.current.is_some() {
            let args = self.args(&exit.locals);
            self.edge(head, args, ir::ExitKind::Next, Some(boundary))?;
        }
        self.loops.pop();
        let has_exit = self.b.body().blocks.iter().any(|(_, block)| {
            matches!(&block.term.kind, ir::TermKind::Br(target) if target.block == exit.block)
        });
        if !has_exit {
            self.dead_block(exit.block)?;
            return Ok(None);
        }
        self.finish_join(&exit)
    }

    fn loop_exit(&mut self, expr: Option<&thir::Expr>, next: bool) -> Result<()> {
        let frame = self.loops.last().cloned().ok_or_else(|| {
            LowerError::invalid(self.source.span, "loop exit outside loop")
        })?;
        if next {
            let mut args = frame.state.clone();
            args.extend(self.args(&frame.exit.locals));
            return self.edge(frame.next, args, ir::ExitKind::Next, Some(frame.boundary));
        }
        let value = match expr {
            Some(expr) => {
                let value = self.expr(expr, frame.exit.value)?;
                if !frame.exit.value {
                    self.discard(value)?;
                }
                value.filter(|_| frame.exit.value)
            }
            None if frame.exit.value && !frame.state.is_empty() => {
                self.require_iteration(frame.state[0])?;
                Some(frame.state[1])
            }
            None => None,
        };
        if self.current.is_some() {
            self.branch_join_at(&frame.exit, value, ir::ExitKind::Last, Some(frame.boundary))?;
        }
        Ok(())
    }

    fn require_iteration(&mut self, ran: ir::ValueId) -> Result<()> {
        let scope = self.scope();
        let yes = self.new_block(scope, &[])?;
        let no = self.new_block(scope, &[])?;
        self.terminate(ir::TermKind::Cond {
            condition: ran,
            yes: ir::Target { block: yes, args: vec![] }, no: ir::Target { block: no, args: vec![] },
        })?;
        self.switch(no)?;
        self.terminate(ir::TermKind::Trap {
            code: ir::TrapCode::Empty, message: "A void result cannot be used as a value".into(),
        })?;
        self.switch(yes)
    }

    fn for_expr(
        &mut self,
        source: &thir::Source,
        binding: thir::LocalId,
        block: &thir::Block,
        ty: Option<&Ty>,
    ) -> Result<Option<ir::ValueId>> {
        let mut array = None;
        let (start, end, inclusive) = match source {
            thir::Source::Range { start, end, inclusive } => {
                let Some(start) = self.value(start)? else {
                    return Ok(None);
                };
                let Some(end) = self.value(end)? else {
                    return Ok(None);
                };
                (start, end, *inclusive)
            }
            thir::Source::Array(expr) if expr.ty == Some(Ty::Range) => {
                let Some(range) = self.value(expr)? else {
                    return Ok(None);
                };
                let start = self.core(ir::CoreOp::RangeStart { value: range })?;
                let len = self.core(ir::CoreOp::RangeLen { value: range })?;
                let end = self.core(ir::CoreOp::IntAdd { left: start, right: len })?;
                (start, end, false)
            }
            thir::Source::Array(expr) if matches!(expr.ty, Some(Ty::Array(_))) => {
                let Some(value) = self.value(expr)? else {
                    return Ok(None);
                };
                array = Some(value);
                let start = self.constant(ir::Literal::Int("0".into()))?;
                let end = self.collection(ir::CollectionOp::ArrayLen { array: value })?;
                (start, end, false)
            }
            _ => return Err(LowerError::unsupported(block.span, "for source other than integer range or std array")),
        };
        // Keep range validation even when traversal needs only integer endpoints.
        self.core(ir::CoreOp::RangeNew { start, end, inclusive })?;
        let parent = self.scope();
        let value_ty = ty.map(|ty| self.ty(ty, block.span)).transpose()?;
        let exit = self.join(parent, value_ty)?;
        let mut state = vec![];
        if let Some(ty) = ty {
            state.push(self.constant(ir::Literal::Bool(false))?);
            // Plain placeholders are never observed before `ran` is true.
            // This avoids undef, which S1 reserves for S2 construction layouts.
            let initial = match ty {
                Ty::Int => self.constant(ir::Literal::Int("0".into()))?,
                Ty::Bool => self.constant(ir::Literal::Bool(false))?,
                Ty::Str => self.constant(ir::Literal::Str(String::new()))?,
                Ty::Range => {
                    let zero = self.constant(ir::Literal::Int("0".into()))?;
                    self.core(ir::CoreOp::RangeNew { start: zero, end: zero, inclusive: false })?
                }
                _ => return Err(LowerError::unsupported(block.span, "non-scalar valued for")),
            };
            state.push(initial);
        }
        let state_types: Vec<_> = state.iter().map(|v| self.b.body().values.get(*v).expect("state value").ty).collect();
        let int_ty = self.ty(&Ty::Int, block.span)?;
        let mut head_types = vec![int_ty];
        head_types.extend(&state_types);
        head_types.extend(self.local_types(&exit.locals));
        let head = self.new_block(parent, &head_types)?;
        let mut initial = vec![start];
        initial.extend(&state);
        initial.extend(self.args(&exit.locals));
        self.terminate(ir::TermKind::Br(ir::Target { block: head, args: initial }))?;
        self.enter(head, &exit.locals, 1 + state.len())?;
        let head_params = self.b.block_params(head)?.to_vec();
        let index = head_params[0];
        let state = head_params[1..1 + state.len()].to_vec();
        let more = if inclusive {
            self.core(ir::CoreOp::IntLe { left: index, right: end })?
        } else {
            self.core(ir::CoreOp::IntLt { left: index, right: end })?
        };
        let scope = self.b.declare_scope(parent, ir::ScopeKind::Iteration)?;
        let iteration = self.new_block(scope, &[])?;
        let exhausted = self.new_block(parent, &[])?;
        self.terminate(ir::TermKind::Cond {
            condition: more, yes: ir::Target { block: iteration, args: vec![] },
            no: ir::Target { block: exhausted, args: vec![] },
        })?;
        let head_env = self.env.clone();
        self.switch(exhausted)?;
        let result = if exit.value {
            self.require_iteration(state[0])?;
            Some(state[1])
        } else {
            None
        };
        self.branch_join(&exit, result, ir::ExitKind::Fallthrough)?;
        let mut step_types = state_types;
        step_types.extend(self.local_types(&exit.locals));
        let step = self.new_block(parent, &step_types)?;
        self.env = head_env;
        self.switch(iteration)?;
        let element = if let Some(array) = array {
            self.collection(ir::CollectionOp::ArrayGet { array, index })?
        } else {
            index
        };
        self.bind(binding, element)?;
        let boundary = self.boundary(scope, ir::BoundaryKind::Loop)?;
        self.loops.push(LoopFrame { next: step, exit: exit.clone(), boundary, state: state.clone() });
        let out = self.block(block, exit.value)?;
        if self.current.is_some() {
            let mut args = vec![];
            if exit.value {
                args.push(self.constant(ir::Literal::Bool(true))?);
                args.push(out.ok_or_else(|| LowerError::invalid(block.span, "valued for body is void"))?);
            }
            args.extend(self.args(&exit.locals));
            self.edge(step, args, ir::ExitKind::Fallthrough, Some(boundary))?;
        }
        self.loops.pop();
        let has_step = self.b.body().blocks.iter().any(|(_, block)| {
            matches!(&block.term.kind, ir::TermKind::Br(target) if target.block == step)
        });
        if has_step {
            self.enter(step, &exit.locals, state.len())?;
            let step_state = self.b.block_params(step)?[..state.len()].to_vec();
            let one = self.constant(ir::Literal::Int("1".into()))?;
            let advanced = self.core(ir::CoreOp::IntAdd { left: index, right: one })?;
            let mut args = vec![advanced];
            args.extend(step_state);
            args.extend(self.args(&exit.locals));
            self.edge(head, args, ir::ExitKind::Next, None)?;
        } else {
            // No body continuation: an unreachable step has no header value.
            self.dead_block(step)?;
        }
        self.finish_join(&exit)
    }
}
