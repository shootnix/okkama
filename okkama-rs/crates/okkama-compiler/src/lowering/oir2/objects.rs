//! Object references and lexical construction intents; ownership is left to `own`.
use super::body::BodyLowerer;
use super::{LowerError, Result};
use crate::analysis::thir;
use crate::oir2 as ir;
use crate::lowering::ctx::PENDING_FIELD;

pub(super) fn pending_index(fields: &[ir::Field]) -> Option<usize> {
    fields.len().checked_sub(1).filter(|&index| fields[index].name == PENDING_FIELD)
}

impl BodyLowerer<'_, '_> {
    pub fn is_constructor(&self) -> bool {
        self.context.deferred.is_none() && matches!(self.source.owner, thir::BodyOwner::Ctor { .. })
    }

    pub fn is_successor(&self) -> bool {
        let function = self.b.module().scopes.get(self.root).expect("root").function;
        self.context.deferred.is_none() && self.b.module().functions.get(function)
            .expect("function").attrs.iter().any(|attr| matches!(attr, ir::FunctionAttr::Successor(_)))
    }

    fn declaration(&self) -> Result<ir::DeclId> {
        let object = match self.source.owner {
            thir::BodyOwner::Ctor { object, .. } | thir::BodyOwner::Method { object, .. }
            | thir::BodyOwner::Destroy { object } => object,
            _ => return Err(LowerError::invalid(self.source.span, "object operation without an owner")),
        };
        Ok(self.context.objects[&object])
    }

    fn layout(&self) -> Result<Vec<ir::Field>> {
        let decl = self.declaration()?;
        let ir::DeclarationKind::Object { fields, .. } = &self.b.module().declarations.get(decl)?.kind else {
            return Err(LowerError::invalid(self.source.span, "object owner has no fields"));
        };
        Ok(fields.clone())
    }

    fn field(&self, index: usize) -> Result<ir::FieldRef> {
        Ok(ir::FieldRef { declaration: self.declaration()?, index: index as u32 })
    }

    pub fn initialize_fields(&mut self) -> Result<()> {
        if !self.is_constructor() && !self.is_successor() {
            return Ok(());
        }
        let fields = self.layout()?;
        let count = pending_index(&fields).unwrap_or(fields.len());
        for (index, field) in fields[..count].iter().enumerate() {
            let value = if self.is_constructor() {
                self.core(ir::CoreOp::Undef { ty: field.ty })?
            } else {
                self.read_field(index)?
            };
            self.env[self.field_base + index] = Some(value);
        }
        Ok(())
    }

    pub fn initialize_constructor(&mut self, params: &[ir::ValueId]) -> Result<()> {
        let layout = self.layout()?;
        // Defaults always execute before supplied values replace them (R09).
        for (field, info) in layout.iter().enumerate() {
            if let Some(function) = info.default {
                let value = self.call(function, vec![])?.ok_or_else(|| {
                    LowerError::invalid(self.source.span, "field default has no result")
                })?;
                self.write_field(field, value)?;
            }
        }
        if let Some(listed) = self.context.listed.clone() {
            for (field, value) in self.context.given.clone().into_iter().zip(params) {
                self.write_field(field, *value)?;
            }
            for (local, field) in self.source.params.iter().zip(listed) {
                let value = self.env[self.field_base + field].expect("field slot");
                self.bind(*local, value)?;
            }
        } else {
            for (local, value) in self.source.params.iter().zip(params) {
                self.bind(*local, *value)?;
            }
        }
        Ok(())
    }

    pub fn write_field(&mut self, index: usize, value: ir::ValueId) -> Result<()> {
        let value = self.take(value)?;
        let field = self.field(index)?;
        if self.early_self.is_some()
            && let Some(previous) = self.env[self.field_base + index]
            && !super::construction::undefined_values(self.b.body())?.contains(&previous) {
            return Err(LowerError::unsupported(self.source.span, "field overwrite after early self publication"));
        }
        self.b.emit(ir::Operation::Lexical(ir::LexicalOp::Stage { action: self.root, field, value }), &[])?;
        if let Some(object) = self.early_self {
            self.b.emit_schema(ir::Operation::Core(ir::CoreOp::ObjInit {
                object, field, value, completion: None,
            }))?;
        }
        self.env[self.field_base + index] = Some(value);
        Ok(())
    }

    pub fn read_field(&mut self, index: usize) -> Result<ir::ValueId> {
        let object = self.receiver.ok_or_else(|| LowerError::invalid(self.source.span, "field without receiver"))?;
        self.core(ir::CoreOp::ObjGet { object, field: self.field(index)? })
    }

    pub fn pending_field(&mut self, index: usize) -> Result<ir::ValueId> {
        if self.context.deferred.is_some() {
            return self.read_field(index);
        }
        let value = self.env[self.field_base + index]
            .ok_or_else(|| LowerError::invalid(self.source.span, "read of an uninitialized field"))?;
        if super::construction::undefined_values(self.b.body())?.contains(&value) {
            return Err(LowerError::unsupported(self.source.span, "read of an uninitialized construction field"));
        }
        if let Some(object) = self.early_self {
            return self.core(ir::CoreOp::ObjGet { object, field: self.field(index)? });
        }
        Ok(value)
    }

    pub fn construct_self(&mut self) -> Result<ir::ValueId> {
        if let Some(value) = self.early_self {
            return Ok(value);
        }
        let mut fields: Vec<_> = self.env[self.field_base..self.field_end].iter()
            .map(|value| value.expect("field slot")).collect();
        let layout = self.layout()?;
        let pending = pending_index(&layout).ok_or_else(|| {
            LowerError::invalid(self.source.span, "early self requires a pending field")
        })?;
        let ty = layout[pending].ty;
        fields.push(self.core(ir::CoreOp::Undef { ty })?);
        let value = self.core(ir::CoreOp::ObjNew {
            declaration: self.declaration()?, type_args: vec![], fields, packet: None,
            construction: ir::ConstructionStatus::Incomplete,
        })?;
        self.early_self = Some(value);
        Ok(value)
    }

    pub fn self_value(&mut self) -> Result<ir::ValueId> {
        if self.is_constructor() {
            self.construct_self()
        } else {
            self.receiver.ok_or_else(|| LowerError::unsupported(self.source.span, "self without receiver"))
        }
    }

    pub fn finish_action(&mut self, value: Option<ir::ValueId>) -> Result<()> {
        if self.is_constructor() {
            let pending = pending_index(&self.layout()?);
            let no = pending.map(|_| self.constant(ir::Literal::Bool(false))).transpose()?;
            let value = if let Some(object) = self.early_self {
                self.b.emit_schema(ir::Operation::Core(ir::CoreOp::ObjInit {
                    object,
                    field: self.field(pending.expect("early self pending field"))?,
                    value: no.expect("pending completion"),
                    completion: None,
                }))?;
                object
            } else {
                let mut fields: Vec<_> = self.env[self.field_base..self.field_end].iter()
                    .map(|value| value.expect("field slot")).collect();
                if let Some(no) = no {
                    fields.push(no);
                }
                self.core(ir::CoreOp::ObjNew {
                    declaration: self.declaration()?, type_args: vec![], fields, packet: None,
                    construction: ir::ConstructionStatus::Complete,
                })?
            };
            self.return_value(Some(value))
        } else if self.is_successor() {
            let mut updates = vec![];
            for (index, value) in self.env[self.field_base..self.field_end].iter().enumerate() {
                if let Some(value) = value {
                    updates.push(ir::FieldValue { field: self.field(index)?, value: *value });
                }
            }
            let value = self.core(ir::CoreOp::ObjNext {
                object: self.receiver.expect("successor receiver"), updates, packet: None,
            })?;
            self.return_value(Some(value))
        } else {
            self.return_value(value)
        }
    }

    pub fn take(&mut self, value: ir::ValueId) -> Result<ir::ValueId> {
        let ty = self.b.body().values.get(value)?.ty;
        if let ir::Type::Successor(ty) = *self.b.module().types.get(ty)? {
            self.core(ir::CoreOp::SuccTake { value, ty })
        } else {
            Ok(value)
        }
    }

    fn call(&mut self, function: ir::FuncId, values: Vec<ir::ValueId>) -> Result<Option<ir::ValueId>> {
        let args = values.into_iter().map(|value| ir::CallArgument {
            value, convention: ir::OperandConvention::Borrow,
        }).collect();
        let call = ir::CallSite {
            args, packet: None, deps: vec![], descriptors: vec![], type_args: vec![], summary: None,
        };
        self.sync_failure()?;
        let values = self.b.emit_schema(ir::Operation::Call(ir::CallOp::Direct { function, call }))?;
        Ok(values.first().copied())
    }

    pub fn direct_call(&mut self, expr: &thir::Expr) -> Result<Option<ir::ValueId>> {
        let thir::ExprKind::Call { callee, args } = &expr.kind else {
            unreachable!("direct call dispatch");
        };
        let mut values = vec![];
        let function = match callee {
            thir::Callee::Method { receiver, object, method } => {
                let Some(value) = self.value(receiver)? else {
                    return Ok(None);
                };
                let value = self.take(value)?;
                self.check_pending(value, self.context.objects[object])?;
                values.push(value);
                self.context.functions[&thir::BodyOwner::Method { object: *object, method: *method }]
            }
            thir::Callee::Ctor { object, ctor, given, type_args } if type_args.is_empty() => {
                self.context.constructors[&(*object, *ctor, given.clone())]
            }
            _ => return Err(LowerError::unsupported(expr.span, "non-direct or generic call")),
        };
        // Evaluate every operand before applying parameter-boundary coercions.
        let offset = values.len();
        for arg in args {
            let operand = match &arg.kind {
                thir::ExprKind::Coerce { coercion: thir::Coercion::Take, value } => value,
                _ => arg,
            };
            let Some(value) = self.value(operand)? else {
                return Ok(None);
            };
            values.push(value);
        }
        for (index, arg) in args.iter().enumerate() {
            if matches!(arg.kind, thir::ExprKind::Coerce { coercion: thir::Coercion::Take, .. }) {
                values[offset + index] = self.take(values[offset + index])?;
            }
        }
        self.call(function, values)
    }

    fn check_pending(&mut self, object: ir::ValueId, declaration: ir::DeclId) -> Result<()> {
        let ir::DeclarationKind::Object { fields, .. } = &self.b.module().declarations.get(declaration)?.kind else {
            unreachable!("method owner");
        };
        let Some(index) = pending_index(fields) else {
            return Ok(());
        };
        let field = ir::FieldRef { declaration, index: index as u32 };
        let condition = self.core(ir::CoreOp::ObjPending { object, field })?;
        let stop = self.new_block(self.scope(), &[])?;
        let go = self.new_block(self.scope(), &[])?;
        self.terminate(ir::TermKind::Cond {
            condition, yes: ir::Target { block: stop, args: vec![] }, no: ir::Target { block: go, args: vec![] },
        })?;
        self.switch(stop)?;
        self.terminate(ir::TermKind::Trap {
            code: ir::TrapCode::Pending, message: "action called before construction completed".into(),
        })?;
        self.switch(go)
    }
}
