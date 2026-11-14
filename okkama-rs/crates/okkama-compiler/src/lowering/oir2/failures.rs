//! High failure continuations. Cleanup and failure-view lifetime belong to `own`.
use super::body::BodyLowerer;
use super::{LowerError, Result};
use crate::analysis::{thir, ty::Ty};
use crate::oir2 as ir;

#[derive(Clone)]
pub(super) struct FailureFrame {
    target: ir::BlockId,
    locals: Vec<usize>,
    handled: Option<ir::ValueId>,
}

pub(super) struct Caught {
    pub failure: ir::ValueId,
}

impl BodyLowerer<'_, '_> {
    /// Ordinary unwind observes block entry, never an instruction-local value.
    pub fn sync_failure(&mut self) -> Result<()> {
        let Some(frame) = self.failure_frames.last().cloned() else {
            return Ok(());
        };
        let block = self.current.expect("open failure source");
        let mut args = self.args(&frame.locals);
        let data = self.b.body().blocks.get(block)?;
        if let Some(unwind) = &data.unwind
            && unwind.target.block == frame.target
            && unwind.target.args[..frame.locals.len()] == args
            && unwind.target.args.iter().all(|arg| data.params.contains(arg)) {
            return Ok(());
        }
        if !data.insts.is_empty() || !args.iter().all(|arg| data.params.contains(arg)) {
            let types = args.iter().map(|value| {
                self.b.body().values.get(*value).map(|value| value.ty)
            }).collect::<std::result::Result<Vec<_>, _>>()?;
            let next = self.new_block(self.scope(), &types)?;
            let term = ir::TermKind::Br(ir::Target { block: next, args });
            self.forward_rethrows(&term)?;
            self.b.terminate(term)?;
            self.b.switch_to(next)?;
            self.current = Some(next);
            args = self.b.block_params(next)?.to_vec();
            for (&local, &value) in frame.locals.iter().zip(&args) {
                self.env[local] = Some(value);
            }
        }
        self.b.set_unwind(Some(ir::BlockUnwind {
            target: ir::Target { block: frame.target, args },
        }))?;
        Ok(())
    }

    pub fn forward_rethrows(&mut self, term: &ir::TermKind) -> Result<()> {
        let mut targets = vec![];
        match term {
            ir::TermKind::Br(target) => targets.push(target),
            ir::TermKind::Cond { yes, no, .. } => targets.extend([yes, no]),
            ir::TermKind::Switch { cases, default, .. } => {
                targets.push(default);
                targets.extend(cases.iter().map(|case| &case.target));
            }
            _ => {}
        }
        for target in targets {
            for (&value, &param) in target.args.iter().zip(self.b.block_params(target.block)?) {
                let incoming = self.rethrows.get(&value).copied();
                let agreed = self.rethrow_inputs.entry(param).or_insert(incoming);
                if *agreed != incoming {
                    *agreed = None;
                }
                if let Some(failure) = *agreed {
                    self.rethrows.insert(param, failure);
                } else {
                    self.rethrows.remove(&param);
                }
            }
        }
        Ok(())
    }

    pub fn error_wrap(&mut self, expr: &thir::Expr, inner: &thir::Expr) -> Result<Option<ir::ValueId>> {
        let ty = self.ty(expr.ty.as_ref().expect("typed Error adapter"), expr.span)?;
        if !matches!(self.b.module().types.get(ty)?, ir::Type::Role { decl, .. }
            if self.b.module().declarations.get(*decl)?.name == "Error") {
            return Err(LowerError::unsupported(expr.span, "role adapter outside plain Error"));
        }
        let Some(value) = self.value(inner)? else {
            return Ok(None);
        };
        let wrapped = self.core(ir::CoreOp::RoleWrap { value, ty })?;
        if let Some(&failure) = self.rethrows.get(&value) {
            self.rethrows.insert(wrapped, failure);
        }
        Ok(Some(wrapped))
    }

    pub fn throw_value(&mut self, expr: &thir::Expr) -> Result<()> {
        let Some(value) = self.value(expr)? else {
            return Ok(());
        };
        if let Some(&failure) = self.rethrows.get(&value)
            && self.caught.iter().any(|caught| caught.failure == failure) {
            self.rethrow(failure)
        } else {
            self.terminate(ir::TermKind::Throw(value))
        }
    }

    pub fn rethrow(&mut self, failure: ir::ValueId) -> Result<()> {
        // A nested try still gets first refusal. At this catch's boundary,
        // bypass cause attachment and preserve the original failure view.
        let bypass = self.failure_frames.last().is_some_and(|frame| frame.handled == Some(failure));
        let frame = bypass.then(|| self.failure_frames.pop().expect("catch frame"));
        if bypass {
            let locals = self.locals();
            let types = self.local_types(&locals);
            let next = self.new_block(self.scope(), &types)?;
            let args = self.args(&locals);
            self.terminate(ir::TermKind::Br(ir::Target { block: next, args }))?;
            self.enter(next, &locals, 0)?;
        }
        self.terminate(ir::TermKind::Raise(failure))?;
        if let Some(frame) = frame {
            self.failure_frames.push(frame);
        }
        Ok(())
    }

    pub fn confirm(&mut self, condition: &thir::Expr, fallback: Option<&thir::Fallback>) -> Result<()> {
        let Some(condition) = self.value(condition)? else {
            return Ok(());
        };
        let scope = self.scope();
        let locals = self.locals();
        let types = self.local_types(&locals);
        let passed = self.new_block(scope, &types)?;
        let failed = self.new_block(scope, &types)?;
        let args = self.args(&locals);
        self.terminate(ir::TermKind::Cond {
            condition,
            yes: ir::Target { block: passed, args: args.clone() },
            no: ir::Target { block: failed, args },
        })?;
        self.enter(failed, &locals, 0)?;
        match fallback {
            Some(thir::Fallback::Throw(value)) => self.throw_value(value)?,
            Some(thir::Fallback::Panic(value)) => {
                if let Some(message) = self.value(value)? {
                    self.terminate(ir::TermKind::Panic(message))?;
                }
            }
            None => self.terminate(ir::TermKind::Trap {
                code: ir::TrapCode::Confirm, message: "confirm failed".into(),
            })?,
        }
        self.enter(passed, &locals, 0)
    }

    fn catch_type(&self, catch: &thir::CatchType, span: crate::source::Span) -> Result<ir::TypeId> {
        match catch {
            thir::CatchType::Variant(id) => self.ty(&Ty::Variant(*id, vec![]), span),
            thir::CatchType::Role(id, args) => self.ty(&Ty::Role(*id, args.clone()), span),
            _ => Err(LowerError::unsupported(span, "catch type outside plain Error enums")),
        }
    }

    pub fn try_expr(
        &mut self,
        found: &thir::Try,
        result: Option<&Ty>,
        want: bool,
    ) -> Result<Option<ir::ValueId>> {
        if found.catches.iter().any(|catch| self.source.locals[catch.local.0].name == "$try") {
            return Err(LowerError::unsupported(found.body.span, "try?/try!"));
        }
        for catch in &found.catches {
            self.catch_type(&catch.ty, catch.span)?;
            for alternative in &catch.alternatives {
                self.catch_type(&alternative.ty, catch.span)?;
                if alternative.cases.is_some() {
                    return Err(LowerError::unsupported(catch.span, "catch case/payload pattern"));
                }
            }
        }
        let parent = self.scope();
        let ty = result.map(|ty| self.ty(ty, found.body.span)).transpose()?;
        let join = self.join(parent, ty)?;
        let locals = self.locals();
        let types = self.local_types(&locals);
        let failure_ty = self.b.intern_type(ir::Type::Service(ir::ServiceType::Failure))?;
        let mut dispatch_types = types.clone();
        dispatch_types.push(failure_ty);
        let dispatch = self.new_block(parent, &dispatch_types)?;
        let scope = self.b.declare_scope(parent, ir::ScopeKind::Try)?;
        let entry = self.new_block(scope, &types)?;
        let args = self.args(&locals);
        self.terminate(ir::TermKind::Br(ir::Target { block: entry, args }))?;
        self.failure_frames.push(FailureFrame { target: dispatch, locals: locals.clone(), handled: None });
        self.enter(entry, &locals, 0)?;
        let value = self.block(&found.body, want)?;
        let mut reaches = self.current.is_some();
        if reaches {
            self.branch_join(&join, value, ir::ExitKind::Fallthrough)?;
        }
        self.failure_frames.pop();
        self.enter(dispatch, &locals, 0)?;
        let mut failure = *self.b.block_params(dispatch)?.last().expect("failure parameter");
        for catch in &found.catches {
            let ty = self.catch_type(&catch.ty, catch.span)?;
            let scope = self.b.declare_scope(parent, ir::ScopeKind::Catch)?;
            let mut handler_types = dispatch_types.clone();
            handler_types.push(ty);
            let handler = self.new_block(scope, &handler_types)?;
            for alternative in &catch.alternatives {
                let alt_ty = self.catch_type(&alternative.ty, catch.span)?;
                let missed = self.new_block(parent, &dispatch_types)?;
                let mut matched_types = dispatch_types.clone();
                matched_types.push(alt_ty);
                let matched = if alt_ty == ty { handler } else { self.new_block(parent, &matched_types)? };
                let mut args = self.args(&locals);
                args.push(failure);
                let mut matched_args: Vec<_> = args.iter().copied().map(ir::EdgeArg::Value).collect();
                matched_args.push(ir::EdgeArg::Produced(ir::EdgeResult { outcome: ir::Outcome::Normal, slot: 0 }));
                self.terminate(ir::TermKind::Catch {
                    failure, ty: alt_ty, handler: ir::InvokeEdge { block: matched, args: matched_args },
                    next: ir::Target { block: missed, args },
                })?;
                if alt_ty != ty {
                    self.enter(matched, &locals, 0)?;
                    let params = self.b.block_params(matched)?.to_vec();
                    let caught = self.core(ir::CoreOp::RoleWrap { value: params[locals.len() + 1], ty })?;
                    let mut args = self.args(&locals);
                    args.extend([params[locals.len()], caught]);
                    self.terminate(ir::TermKind::Br(ir::Target { block: handler, args }))?;
                }
                self.enter(missed, &locals, 0)?;
                failure = self.b.block_params(missed)?[locals.len()];
            }
            let missed = self.current.expect("next catch dispatch");
            let missed_env = self.env.clone();
            let missed_failure = failure;
            let mut caused_types = dispatch_types.clone();
            caused_types.push(failure_ty);
            let caused = self.new_block(parent, &caused_types)?;
            self.enter(handler, &locals, 0)?;
            let params = self.b.block_params(handler)?.to_vec();
            let handled = params[locals.len()];
            let error = params[locals.len() + 1];
            self.rethrows.insert(error, handled);
            self.bind(catch.local, error)?;
            self.caught.push(Caught { failure: handled });
            // Carry the active failure as plain entry-visible context. A nested
            // try forwards it too, so its landing block itself remains catch/raise.
            let handled_slot = self.env.len();
            self.env.push(Some(handled));
            let mut failure_locals = locals.clone();
            failure_locals.push(handled_slot);
            self.failure_frames.push(FailureFrame {
                target: caused, locals: failure_locals, handled: Some(handled),
            });
            self.sync_failure()?;
            let value = self.block(&catch.body, want)?;
            if self.current.is_some() {
                reaches = true;
                self.branch_join(&join, value, ir::ExitKind::Fallthrough)?;
            }
            self.failure_frames.pop();
            self.caught.pop();
            self.enter(caused, &locals, 0)?;
            let params = self.b.block_params(caused)?.to_vec();
            let chained = self.core(ir::CoreOp::FailCaused {
                new: params[locals.len() + 1], handled: params[locals.len()],
            })?;
            self.terminate(ir::TermKind::Raise(chained))?;
            self.env = missed_env;
            self.switch(missed)?;
            failure = missed_failure;
        }
        self.terminate(ir::TermKind::Raise(failure))?;
        if reaches {
            self.finish_join(&join)
        } else {
            self.dead_block(join.block)?;
            Ok(None)
        }
    }
}
