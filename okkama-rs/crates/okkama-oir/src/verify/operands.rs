use super::*;

impl Checker<'_, '_> {
    pub(super) fn operands(&mut self, op: &Operation) -> Checked<()> {
        match op {
            Operation::Const(value) => {
                let valid = literal(self.module, value, self.state);
                self.require(valid, "invalid literal encoding or enum case");
            }
            Operation::Core(op) => self.core(op)?,
            Operation::Collection(op) => self.collection(op)?,
            Operation::Lexical(op) => self.lexical(op)?,
            Operation::Ownership(op) => self.ownership(op)?,
            Operation::Call(op) => self.call(op)?,
            Operation::Async(op) => self.async_op(op)?,
            Operation::Carrier(op) => self.carrier(op)?,
            Operation::Exit(op) => self.exit(op)?,
            Operation::Cleanup(op) => self.cleanup(op)?,
        }
        Ok(())
    }
    fn core(&mut self, op: &CoreOp) -> Checked<()> {
        match op {
            CoreOp::IntAdd { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntSub { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntMul { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntDiv { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntMod { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntAnd { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntOr { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntXor { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntShl { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntShr { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntUshr { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntNeg { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntNot { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntAbs { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntSign { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntBitCount { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntLeadingZeros { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntTrailingZeros { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntReverseBits { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntSwapBytes { value } => {
                self.expect(*value, Type::Int)?;
            }
            CoreOp::IntRotateLeft { value, count } => {
                self.expect(*value, Type::Int)?;
                self.expect(*count, Type::Int)?;
            }
            CoreOp::IntRotateRight { value, count } => {
                self.expect(*value, Type::Int)?;
                self.expect(*count, Type::Int)?;
            }
            CoreOp::ByteAnd { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteOr { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteXor { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteShl { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::ByteShr { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::ByteUshr { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteNot { value } => {
                self.expect(*value, Type::Byte)?;
            }
            CoreOp::ByteBitCount { value } => {
                self.expect(*value, Type::Byte)?;
            }
            CoreOp::ByteLeadingZeros { value } => {
                self.expect(*value, Type::Byte)?;
            }
            CoreOp::ByteTrailingZeros { value } => {
                self.expect(*value, Type::Byte)?;
            }
            CoreOp::ByteReverseBits { value } => {
                self.expect(*value, Type::Byte)?;
            }
            CoreOp::ByteSwapBytes { value } => {
                self.expect(*value, Type::Byte)?;
            }
            CoreOp::ByteRotateLeft { value, count } => {
                self.expect(*value, Type::Byte)?;
                self.expect(*count, Type::Int)?;
            }
            CoreOp::ByteRotateRight { value, count } => {
                self.expect(*value, Type::Byte)?;
                self.expect(*count, Type::Int)?;
            }
            CoreOp::FloatAdd { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatSub { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatMul { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatDiv { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatMod { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatPow { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatNeg { value } => {
                self.expect(*value, Type::Float)?;
            }
            CoreOp::FloatAbs { value } => {
                self.expect(*value, Type::Float)?;
            }
            CoreOp::FloatSign { value } => {
                self.expect(*value, Type::Float)?;
            }
            CoreOp::FloatSqrt { value } => {
                self.expect(*value, Type::Float)?;
            }
            CoreOp::RatAdd { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatSub { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatMul { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatDiv { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatMod { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatNeg { value } => {
                self.expect(*value, Type::Rat)?;
            }
            CoreOp::RatAbs { value } => {
                self.expect(*value, Type::Rat)?;
            }
            CoreOp::RatSign { value } => {
                self.expect(*value, Type::Rat)?;
            }
            CoreOp::IntEq { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntNe { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntLt { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntLe { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntGt { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::IntGe { left, right } => {
                self.expect(*left, Type::Int)?;
                self.expect(*right, Type::Int)?;
            }
            CoreOp::ByteEq { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteNe { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteLt { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteLe { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteGt { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::ByteGe { left, right } => {
                self.expect(*left, Type::Byte)?;
                self.expect(*right, Type::Byte)?;
            }
            CoreOp::FloatEq { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatNe { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatLt { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatLe { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatGt { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::FloatGe { left, right } => {
                self.expect(*left, Type::Float)?;
                self.expect(*right, Type::Float)?;
            }
            CoreOp::RatEq { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatNe { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatLt { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatLe { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatGt { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::RatGe { left, right } => {
                self.expect(*left, Type::Rat)?;
                self.expect(*right, Type::Rat)?;
            }
            CoreOp::BoolEq { left, right } => {
                self.expect(*left, Type::Bool)?;
                self.expect(*right, Type::Bool)?;
            }
            CoreOp::BoolNe { left, right } => {
                self.expect(*left, Type::Bool)?;
                self.expect(*right, Type::Bool)?;
            }
            CoreOp::CharEq { left, right } => {
                self.expect(*left, Type::Char)?;
                self.expect(*right, Type::Char)?;
            }
            CoreOp::CharNe { left, right } => {
                self.expect(*left, Type::Char)?;
                self.expect(*right, Type::Char)?;
            }
            CoreOp::CharLt { left, right } => {
                self.expect(*left, Type::Char)?;
                self.expect(*right, Type::Char)?;
            }
            CoreOp::CharLe { left, right } => {
                self.expect(*left, Type::Char)?;
                self.expect(*right, Type::Char)?;
            }
            CoreOp::CharGt { left, right } => {
                self.expect(*left, Type::Char)?;
                self.expect(*right, Type::Char)?;
            }
            CoreOp::CharGe { left, right } => {
                self.expect(*left, Type::Char)?;
                self.expect(*right, Type::Char)?;
            }
            CoreOp::StrEq { left, right } => {
                self.expect(*left, Type::Str)?;
                self.expect(*right, Type::Str)?;
            }
            CoreOp::StrNe { left, right } => {
                self.expect(*left, Type::Str)?;
                self.expect(*right, Type::Str)?;
            }
            CoreOp::StrLt { left, right } => {
                self.expect(*left, Type::Str)?;
                self.expect(*right, Type::Str)?;
            }
            CoreOp::StrLe { left, right } => {
                self.expect(*left, Type::Str)?;
                self.expect(*right, Type::Str)?;
            }
            CoreOp::StrGt { left, right } => {
                self.expect(*left, Type::Str)?;
                self.expect(*right, Type::Str)?;
            }
            CoreOp::StrGe { left, right } => {
                self.expect(*left, Type::Str)?;
                self.expect(*right, Type::Str)?;
            }
            CoreOp::BoolNot { value } => {
                self.expect(*value, Type::Bool)?;
            }
            CoreOp::RatNumerator { value } => {
                self.expect(*value, Type::Rat)?;
            }
            CoreOp::RatDenominator { value } => {
                self.expect(*value, Type::Rat)?;
            }
            CoreOp::StrConcat { left, right } => {
                self.expect(*left, Type::Str)?;
                self.expect(*right, Type::Str)?;
            }
            CoreOp::StrLen { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::StrUtf8 { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::StrSlice { value, start, end } => {
                self.expect(*value, Type::Str)?;
                self.expect(*start, Type::Int)?;
                self.expect(*end, Type::Int)?;
            }
            CoreOp::StrSplit { value, separator } => {
                self.expect(*value, Type::Str)?;
                self.expect(*separator, Type::Str)?;
            }
            CoreOp::StrReplace { value, target, replacement } => {
                self.expect(*value, Type::Str)?;
                self.expect(*target, Type::Str)?;
                self.expect(*replacement, Type::Str)?;
            }
            CoreOp::StrReplaceFirst { value, target, replacement } => {
                self.expect(*value, Type::Str)?;
                self.expect(*target, Type::Str)?;
                self.expect(*replacement, Type::Str)?;
            }
            CoreOp::StrRepeat { value, count } => {
                self.expect(*value, Type::Str)?;
                self.expect(*count, Type::Int)?;
            }
            CoreOp::StrTrim { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::StrChomp { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::StrTrimStart { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::StrTrimEnd { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::StrLower { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::StrUpper { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::StrFind { value, part } => {
                self.expect(*value, Type::Str)?;
                self.expect(*part, Type::Str)?;
            }
            CoreOp::StrFindLast { value, part } => {
                self.expect(*value, Type::Str)?;
                self.expect(*part, Type::Str)?;
            }
            CoreOp::StrStartsWith { value, part } => {
                self.expect(*value, Type::Str)?;
                self.expect(*part, Type::Str)?;
            }
            CoreOp::StrEndsWith { value, part } => {
                self.expect(*value, Type::Str)?;
                self.expect(*part, Type::Str)?;
            }
            CoreOp::StrCharacters { value } => {
                self.expect(*value, Type::Str)?;
            }
            CoreOp::RangeNew { start, end, .. } => {
                self.expect(*start, Type::Int)?;
                self.expect(*end, Type::Int)?;
            }
            CoreOp::RangeStart { value } => {
                self.expect(*value, Type::Range)?;
            }
            CoreOp::RangeEnd { value } => {
                self.expect(*value, Type::Range)?;
            }
            CoreOp::RangeLen { value } => {
                self.expect(*value, Type::Range)?;
            }
            CoreOp::ValEq { left, right } | CoreOp::ValNe { left, right } => {
                self.same(*left, *right)?;
                self.language(*left)?;
                let valid = !matches!(self.ty(*left)?, Type::Any | Type::Handle(_) | Type::Cap { .. }
                    | Type::Successor(_));
                self.require(valid, "val equality is not defined for this type");
            }
            CoreOp::EnumEq { left, right } | CoreOp::EnumNe { left, right } => {
                self.same(*left, *right)?;
                let valid = matches!(self.ty(*left)?, Type::Enum(_));
                self.require(valid, "enum comparison requires an enum");
            }
            CoreOp::RatNew { numerator, denominator } => {
                self.expect(*numerator, Type::Int)?;
                self.expect(*denominator, Type::Int)?;
            }
            CoreOp::Convert { value, from, .. } => {
                self.expect(*value, match from {
                    NumericType::Int => Type::Int,
                    NumericType::Byte => Type::Byte,
                    NumericType::Float => Type::Float,
                    NumericType::Rat => Type::Rat,
                    NumericType::Bool => Type::Bool,
                    NumericType::Str => Type::Str,
                })?;
            }
            CoreOp::ByteFrom { value, .. } => {
                let valid = matches!(self.ty(*value)?, Type::Int | Type::Float | Type::Rat);
                self.require(valid, "byte.from requires int, float or rat");
            }
            CoreOp::Undef { ty } => {
                let valid = language_type(self.module, *ty, self.state)?;
                self.require(valid, "undef requires a language value type");
            }
            CoreOp::Global { declaration, ty } => {
                let valid = matches!(self.module.declarations.get(*declaration)?.kind,
                    DeclarationKind::Global { ty: actual, .. } if actual == *ty);
                self.require(valid, "global annotation differs from its declaration");
            }
            CoreOp::ObjNew { declaration, type_args, fields, packet, .. } => {
                let decl = self.module.declarations.get(*declaration)?;
                generic_arguments(self.module, GenericOwner::Declaration(*declaration),
                    &decl.type_params, type_args, self.state)?;
                self.fields(*declaration, type_args, fields)?;
                self.packet(*packet)?;
            }
            CoreOp::ObjNext { object, updates, packet } => {
                let valid = matches!(self.ty(*object)?, Type::Object { .. });
                self.require(valid, "obj.next requires an object");
                let mut fields = std::collections::BTreeSet::new();
                for update in updates {
                    let ty = self.field(*object, update.field)?;
                    self.exact(update.value, ty)?;
                    self.require(fields.insert(update.field.index), "duplicate updated field");
                }
                self.packet(*packet)?;
            }
            CoreOp::ObjInit { object, field, value, completion } => {
                let ty = self.field(*object, *field)?;
                self.exact(*value, ty)?;
                if let Some(completion) = completion { self.scope(completion.state)?; }
            }
            CoreOp::OnceStore { object, field, value } => {
                let ty = self.field(*object, *field)?;
                self.exact(*value, ty)?;
                self.once_field(*field)?;
            }
            CoreOp::ObjGet { object, field } | CoreOp::ObjPending { object, field } => {
                self.field(*object, *field)?;
            }
            CoreOp::OnceLoad { object, field } => {
                self.field(*object, *field)?;
                self.once_field(*field)?;
            }
            CoreOp::SuccTake { value, ty } => {
                let valid = matches!(self.ty(*value)?, Type::Successor(inner) if inner == ty);
                self.require(valid, "succ.take result differs from the wrapped object");
            }
            CoreOp::RoleWrap { value, ty } | CoreOp::RoleNarrow { value, ty } => {
                let valid = matches!(self.module.types.get(*ty)?, Type::Role { .. });
                self.require(valid, "role conversion requires a role destination");
                let input = value_type(self.body, *value)?;
                let valid = implements(self.module, input, *ty, &self.substitution, self.state)?
                    || matches!(op, CoreOp::RoleNarrow { .. }) && matches!(self.ty(*value)?, Type::Role { .. });
                self.require(valid, "receiver has no matching role witness");
            }
            CoreOp::VariantOf { value, ty } => {
                let valid = match (self.ty(*value)?, self.module.types.get(*ty)?) {
                    (Type::Object { decl: layout, .. }, Type::Variant(decl)) => {
                        matches!(&self.module.declarations.get(*decl)?.kind,
                            DeclarationKind::Variant { cases, .. } if cases.iter().any(|case| case.layout == *layout))
                    }
                    _ => false,
                };
                self.require(valid, "variant.of needs a layout belonging to the destination variant");
            }
            CoreOp::Retag { value, ty } => {
                self.language(*value)?;
                let valid = retaggable(self.module, value_type(self.body, *value)?, *ty, self.state)?;
                self.require(valid, "retag requires compatible nominal types");
            }
            CoreOp::VariantNew { ty, case, fields, packet } => {
                let layout = self.variant_case(*ty, *case)?;
                self.fields(layout, &[], fields)?;
                self.packet(*packet)?;
            }
            CoreOp::VariantTag { value } => {
                let valid = matches!(self.ty(*value)?, Type::Variant(_));
                self.require(valid, "variant.tag requires a variant");
            }
            CoreOp::VariantGet { value, case, field } => {
                let layout = self.variant_case(value_type(self.body, *value)?, *case)?;
                self.require(layout == field.declaration, "variant field belongs to a different case layout");
                field_type(self.module, *field)?;
            }
            CoreOp::CapMake { owner, ty, packet } => {
                let Type::Cap { decl, args } = self.module.types.get(*ty)? else {
                    self.require(false, "cap.make requires a cap destination");
                    return Err(TypeCheckError);
                };
                let DeclarationKind::Cap { owner: expected, .. } = &self.module.declarations.get(*decl)?.kind else {
                    return Err(TypeCheckError);
                };
                self.substitution = substitution(GenericOwner::Declaration(*decl), args);
                self.exact(*owner, *expected)?;
                self.packet(*packet)?;
            }
            CoreOp::FnClosure { function, captures, ty, packet } => {
                let function = self.module.functions.get(*function)?;
                let count = function.attrs.iter().find_map(|attr| match attr {
                    FunctionAttr::Lambda { captures, .. } => Some(*captures as usize), _ => None,
                }).unwrap_or(0);
                self.require(count == captures.len(), "closure capture arity mismatch");
                for (&value, &expected) in captures.iter().zip(&function.signature.params) {
                    self.language(value)?;
                    self.exact(value, expected)?;
                }
                self.bound_fn(&function.signature, captures.len(), *ty)?;
                self.packet(*packet)?;
            }
            CoreOp::FnBind { receiver, function, ty, packet } => {
                let signature = &self.module.functions.get(*function)?.signature;
                if let Some(&expected) = signature.params.first() { self.exact(*receiver, expected)?; }
                else { self.require(false, "bound function has no receiver"); }
                self.bound_fn(signature, 1, *ty)?;
                self.packet(*packet)?;
            }
            CoreOp::FnBindRole { receiver, role, slot, ty, packet } => {
                let signature = self.role_signature(*receiver, *role, *slot)?.clone();
                self.bound_fn(&signature, 0, *ty)?;
                self.packet(*packet)?;
            }
            CoreOp::AnyBox { value, ty, packet } => {
                self.language(*value)?;
                let valid = !matches!(self.ty(*value)?, Type::Any | Type::Handle(_) | Type::Fn(_)
                    | Type::Future(_) | Type::Cap { .. } | Type::Successor(_));
                self.require(valid, "any.box cannot box this type");
                self.require(self.module.types.get(*ty)? == &Type::Any, "any.box destination must be any");
                self.packet(*packet)?;
            }
            CoreOp::FailSecondary { primary, cleanup } => {
                self.service(*primary, ServiceType::Failure)?;
                self.service(*cleanup, ServiceType::Failure)?;
            }
            CoreOp::FailCaused { new, handled } => {
                self.service(*new, ServiceType::Failure)?;
                self.service(*handled, ServiceType::Failure)?;
            }
            CoreOp::FailIsThrown { failure } => self.service(*failure, ServiceType::Failure)?,
            CoreOp::TypeOf { ty, descriptor } => {
                self.module.types.get(*ty)?;
                self.module.functions.get(descriptor.cleanup)?;
                self.module.summaries.get(descriptor.summary)?;
            }
            CoreOp::TestOk { condition } => self.expect(*condition, Type::Bool)?,
            CoreOp::TestIs { left, right } | CoreOp::TestIsnt { left, right } => {
                self.language(*left)?;
                self.same(*left, *right)?;
            }
            CoreOp::TestFail { message } => self.expect(*message, Type::Str)?,
            CoreOp::EnumCase { declaration, case } => {
                let valid = literal(self.module, &Literal::Enum { decl: *declaration, case: *case }, self.state);
                self.require(valid, "enum case index is out of range");
            }
            CoreOp::FakeNew { declaration, packet } => {
                let valid = matches!(self.module.declarations.get(*declaration)?.kind, DeclarationKind::Object { .. });
                self.require(valid, "fake.new requires an object declaration");
                self.packet(*packet)?;
            }
        }
        Ok(())
    }
    fn once_field(&mut self, field: FieldRef) -> Checked<()> {
        let valid = match &self.module.declarations.get(field.declaration)?.kind {
            DeclarationKind::Object { fields, .. } => fields.get(field.index as usize).is_some_and(|field| field.once),
            _ => false,
        };
        self.require(valid, "once operation requires a once field");
        Ok(())
    }
    fn variant_case(&mut self, ty: TypeId, case: u32) -> Checked<DeclId> {
        if let Type::Variant(decl) = self.module.types.get(ty)?
            && let DeclarationKind::Variant { cases, .. } = &self.module.declarations.get(*decl)?.kind
            && let Some(case) = cases.get(case as usize) { return Ok(case.layout); }
        self.require(false, "variant/case index is invalid");
        Err(TypeCheckError)
    }
    fn bound_fn(&mut self, signature: &Signature, skip: usize, ty: TypeId) -> Checked<()> {
        let Type::Fn(function) = self.module.types.get(ty)? else {
            self.require(false, "closure destination must have fn type");
            return Err(TypeCheckError);
        };
        let params = language_slots(self.module, signature.params.get(skip..).unwrap_or(&[]), self.state);
        let results = language_slots(self.module, &signature.results, self.state);
        self.require(params.len() == function.params.len() && results.len() <= 1
            && results.len() == usize::from(function.result.is_some()), "bound fn arity mismatch");
        for (&expected, &actual) in params.iter().zip(&function.params)
            .chain(results.iter().zip(function.result.iter())) {
            let valid = matches_type(self.module, expected, actual, &self.substitution, self.state)?;
            self.require(valid, "bound fn type mismatch");
        }
        Ok(())
    }
    fn collection(&mut self, op: &CollectionOp) -> Checked<()> {
        match op {
            CollectionOp::ArrayNew { values, ty, packet } => {
                let Type::Array(element) = self.module.types.get(*ty)? else {
                    self.require(false, "array.new destination must be an array");
                    return Err(TypeCheckError);
                };
                for &value in values { self.exact(value, *element)?; }
                self.packet(*packet)?;
            }
            CollectionOp::ArrayConcat { arrays, ty, packet } => {
                self.require(matches!(self.module.types.get(*ty)?, Type::Array(_)), "expected array type");
                for &array in arrays { self.exact(array, *ty)?; }
                self.packet(*packet)?;
            }
            CollectionOp::ArraySlice { array, start, end, packet } => {
                self.array(*array)?;
                self.expect(*start, Type::Int)?;
                self.expect(*end, Type::Int)?;
                self.packet(*packet)?;
            }
            CollectionOp::ArrayReplace { array, index, value, packet } => {
                let element = self.array(*array)?;
                self.expect(*index, Type::Int)?;
                self.exact(*value, element)?;
                self.packet(*packet)?;
            }
            CollectionOp::ArrayRemove { array, index, packet } => {
                self.array(*array)?;
                self.expect(*index, Type::Int)?;
                self.packet(*packet)?;
            }
            CollectionOp::ArrayDifference { array, removed, packet } => {
                self.array(*array)?;
                for &other in removed { self.same(*array, other)?; }
                self.packet(*packet)?;
            }
            CollectionOp::MapNew { entries, ty, packet } => {
                let Type::Map(key, value) = self.module.types.get(*ty)? else {
                    self.require(false, "map.new destination must be a map");
                    return Err(TypeCheckError);
                };
                for &(k, v) in entries {
                    self.exact(k, *key)?;
                    self.exact(v, *value)?;
                }
                self.packet(*packet)?;
            }
            CollectionOp::MapMerge { maps, ty, packet } => {
                self.require(matches!(self.module.types.get(*ty)?, Type::Map(..)), "expected map type");
                for &map in maps { self.exact(map, *ty)?; }
                self.packet(*packet)?;
            }
            CollectionOp::MapDifference { map, removed, packet } => {
                self.map(*map)?;
                for &other in removed { self.same(*map, other)?; }
                self.packet(*packet)?;
            }
            CollectionOp::ArrayLen { array } => { self.array(*array)?; }
            CollectionOp::ArrayGet { array, index } => {
                self.array(*array)?;
                self.expect(*index, Type::Int)?;
            }
            CollectionOp::ArrayJoin { array, separator } => {
                let element = self.array(*array)?;
                self.require(self.module.types.get(element)? == &Type::Str, "array.join requires str elements");
                self.expect(*separator, Type::Str)?;
            }
            CollectionOp::ArrayRetag { array, ty } => {
                let element = self.array(*array)?;
                let valid = match self.module.types.get(*ty)? {
                    Type::Array(target) => retaggable(self.module, element, *target, self.state)?, _ => false,
                };
                self.require(valid, "array.retag requires compatible element types");
            }
            CollectionOp::MapLen { map } | CollectionOp::MapKeys { map } | CollectionOp::MapValues { map } => {
                self.map(*map)?;
            }
            CollectionOp::MapHas { map, key } | CollectionOp::MapGet { map, key } => {
                let (expected, _) = self.map(*map)?;
                self.exact(*key, expected)?;
            }
            CollectionOp::MapKeyAt { map, index } | CollectionOp::MapValueAt { map, index } => {
                self.map(*map)?;
                self.expect(*index, Type::Int)?;
            }
            CollectionOp::MapRetag { map, ty } => {
                let (key, value) = self.map(*map)?;
                let valid = match self.module.types.get(*ty)? {
                    Type::Map(k, v) => retaggable(self.module, key, *k, self.state)?
                        && retaggable(self.module, value, *v, self.state)?, _ => false,
                };
                self.require(valid, "map.retag requires identical keys and compatible values");
            }
            CollectionOp::BuilderNew { element } => {
                let valid = language_type(self.module, *element, self.state)?;
                self.require(valid, "builder element must be a language type");
            }
            CollectionOp::BuilderPush { buffer, value, packet } => {
                let (element, _) = self.buffer(*buffer, false)?;
                self.exact(*value, element)?;
                self.packet(*packet)?;
            }
            CollectionOp::BuilderAppend { buffer, array, packet } => {
                let (element, _) = self.buffer(*buffer, false)?;
                let actual = self.array(*array)?;
                self.require(element == actual, "builder.append element type mismatch");
                self.packet(*packet)?;
            }
            CollectionOp::BuilderAppendRange { buffer, start, end, packet, .. } => {
                let (element, _) = self.buffer(*buffer, false)?;
                self.require(self.module.types.get(element)? == &Type::Int, "range append needs builder<int>");
                self.expect(*start, Type::Int)?;
                self.expect(*end, Type::Int)?;
                self.packet(*packet)?;
            }
            CollectionOp::BuilderFinish { buffer, packet } | CollectionOp::BuilderAbandon { buffer, packet } => {
                self.buffer(*buffer, false)?;
                self.packet(*packet)?;
            }
            CollectionOp::MapBuilderNew { key, value } => {
                self.require(map_key(self.module.types.get(*key)?), "invalid map key type");
                let valid = language_type(self.module, *value, self.state)?;
                self.require(valid, "map builder value must be a language type");
            }
            CollectionOp::MapBuilderPut { buffer, key, value, packet } => {
                let (k, v) = self.buffer(*buffer, true)?;
                self.exact(*key, k)?;
                self.exact(*value, v)?;
                self.packet(*packet)?;
            }
            CollectionOp::MapBuilderAppend { buffer, map, packet } => {
                let expected = self.buffer(*buffer, true)?;
                let actual = self.map(*map)?;
                self.require(expected == actual, "mbuilder.append map type mismatch");
                self.packet(*packet)?;
            }
            CollectionOp::MapBuilderFinish { buffer, packet }
            | CollectionOp::MapBuilderAbandon { buffer, packet } => {
                self.buffer(*buffer, true)?;
                self.packet(*packet)?;
            }
        }
        Ok(())
    }
    fn lexical(&mut self, op: &LexicalOp) -> Checked<()> {
        match op {
            LexicalOp::Bind { binding, value } | LexicalOp::Rebind { binding, value, .. } => {
                self.binding(*binding, *value)?;
            }
            LexicalOp::Stage { field, value, .. } => self.exact(*value, field_type(self.module, *field)?)?,
            LexicalOp::Temp { value, .. } | LexicalOp::Want { value } | LexicalOp::Discard { value } => {
                self.language(*value)?;
            }
            LexicalOp::DeferRegister { action, .. } => {
                let signature = &self.module.functions.get(action.function)?.signature;
                let capture_count = action.captures.len();
                self.require(signature.params.len() == capture_count + action.bindings.len(),
                    "defer parameters must be captures followed by bindings");
                self.arguments(&action.captures, &signature.params[..capture_count.min(signature.params.len())])?;
                for (binding, expected) in action.bindings.iter().zip(signature.params.iter().skip(capture_count)) {
                    let ty = self.body.bindings.get(*binding)?.ty;
                    let valid = matches_type(self.module, *expected, ty, &self.substitution, self.state)?;
                    self.require(valid, "defer binding parameter type mismatch");
                }
                self.require(signature.results.is_empty(), "defer action must return void");
                let declaration = self.module.functions.get(action.function)?;
                let labels = declaration.attrs.iter().find_map(|attr| {
                    if let FunctionAttr::DeferEntry { replacements } = attr { Some(replacements) } else { None }
                });
                let declared: Vec<_> = labels.into_iter().flatten().map(|label| &label.label).collect();
                let registered: Vec<_> = action.replacements.iter().map(|item| &item.label).collect();
                self.require(declared == registered, "defer registration must map every declared replacement label");
            }
        }
        Ok(())
    }
    fn ownership(&mut self, op: &OwnershipOp) -> Checked<()> {
        match op {
            OwnershipOp::Empty {} | OwnershipOp::ScopeBegin { .. } => {}
            OwnershipOp::Move { packet } | OwnershipOp::Split { packet, .. }
            | OwnershipOp::SplitChecked { packet, .. } | OwnershipOp::End { packet, .. } => {
                self.service(*packet, ServiceType::Own)?;
            }
            OwnershipOp::Join { packets } => {
                for &packet in packets { self.service(packet, ServiceType::Own)?; }
            }
            OwnershipOp::Admit { state, packet } | OwnershipOp::Accept { state, packet } => {
                self.scope(*state)?;
                self.service(*packet, ServiceType::Own)?;
            }
            OwnershipOp::Bind { binding } | OwnershipOp::Rebind { binding } => {
                self.scope(binding.state)?;
                self.binding(binding.binding, binding.resource.value)?;
                self.service(binding.resource.packet, ServiceType::Own)?;
            }
            OwnershipOp::Stage { stage } => {
                self.states(stage.states)?;
                self.exact(stage.resource.value, field_type(self.module, stage.field)?)?;
                self.service(stage.resource.packet, ServiceType::Own)?;
            }
            OwnershipOp::ScopeSplit { state, .. } => self.scope(*state)?,
            OwnershipOp::ScopeEnd { state, reserve, effects } => {
                self.close_envelope(effects, false, *state)?;
                self.scope(*state)?;
                if let Some(reserve) = reserve { self.service(*reserve, ServiceType::Pending)?; }
            }
            OwnershipOp::ScopeUnwind { state, failure, reserve, effects } => {
                self.close_envelope(effects, true, *state)?;
                if let Some(reserve) = reserve { self.service(*reserve, ServiceType::Pending)?; }
                self.scope(*state)?;
                self.service(*failure, ServiceType::Failure)?;
            }
            OwnershipOp::ScopeTransfer { states, .. } => self.states(*states)?,
            OwnershipOp::Dynamic { state, packet, target, .. } => {
                self.service(*state, ServiceType::Scope(*target))?;
                self.service(*packet, ServiceType::Own)?;
            }
            OwnershipOp::DynamicCheck { key, snapshot } => {
                self.service(*key, ServiceType::StableKey)?;
                self.service(*snapshot, ServiceType::OwnerSnapshot)?;
            }
            OwnershipOp::DynamicRefresh { key, access } => {
                self.service(*key, ServiceType::StableKey)?;
                self.service(access.owner, ServiceType::Owner)?;
                self.service(access.holder, ServiceType::Holder)?;
                self.language(access.authority)?;
            }
        }
        Ok(())
    }
    fn arguments(&mut self, values: &[ValueId], params: &[TypeId]) -> Checked<()> {
        self.require(values.len() == params.len(), "argument arity mismatch");
        for (&value, &expected) in values.iter().zip(params) { self.exact(value, expected)?; }
        Ok(())
    }
    fn role_signature(&mut self, receiver: ValueId, role: DeclId, slot: u32) -> Checked<&Signature> {
        self.object_substitution(receiver)?;
        let valid = matches!(self.ty(receiver)?, Type::Role { decl, .. } if *decl == role);
        self.require(valid, "role call receiver type mismatch");
        let DeclarationKind::Role { methods, .. } = &self.module.declarations.get(role)?.kind else {
            self.require(false, "role slot requires a role declaration");
            return Err(TypeCheckError);
        };
        if let Some(method) = methods.get(slot as usize) { return Ok(&method.signature); }
        self.require(false, "role slot is out of range");
        Err(TypeCheckError)
    }
    fn call_site(&mut self, call: &CallSite, params: &[TypeId], receiver: Option<ValueId>) -> Checked<()> {
        if call.cleanup == CallCleanupMode::Shielded {
            let function = match self.site {
                OperationSite::Instruction { function, .. } | OperationSite::Terminator { function, .. } => function,
            };
            let attrs = &self.module.functions.get(function)?.attrs;
            let cleanup = attrs.iter().any(|attr| matches!(attr,
                FunctionAttr::Cleanup | FunctionAttr::Destroy(_) | FunctionAttr::DeferEntry { .. }));
            self.require(cleanup, "shielded calls require declared cleanup execution");
        }
        self.packet(call.packet)?;
        for &dep in &call.deps { self.service(dep, ServiceType::Deps)?; }
        for &descriptor in &call.descriptors { self.expect(descriptor, Type::Descriptor)?; }
        let count = call.args.len().checked_add(usize::from(receiver.is_some())).ok_or(TypeCheckError)?;
        self.require(count == params.len(), "call argument count mismatch");
        if let Some(receiver) = receiver && let Some(&expected) = params.first() {
            // A role dispatch permits the concrete witness receiver.
            let valid = matches_type(self.module, expected, value_type(self.body, receiver)?,
                &self.substitution, self.state)?
                || implements(self.module, value_type(self.body, receiver)?, expected, &self.substitution, self.state)?;
            self.require(valid, "call receiver type mismatch");
        }
        let skip = usize::from(receiver.is_some());
        for (argument, &expected) in call.args.iter().zip(params.iter().skip(skip)) {
            self.exact(argument.value, expected)?;
        }
        Ok(())
    }
    fn close_envelope(&mut self, effects: &ScopeEffectEnvelope, active_failure: bool, state: ValueId) -> Checked<()> {
        super::elaboration::failure_set(&effects.failures, false, self.state);
        if effects.cancel {
            let function = match self.site {
                OperationSite::Instruction { function, .. } | OperationSite::Terminator { function, .. } => function,
            };
            let root = match self.ty(state)? {
                Type::Service(ServiceType::Scope(scope)) => self.module.scopes.get(*scope)
                    .is_ok_and(|scope| scope.parent.is_none() && scope.kind == ScopeKind::Action),
                _ => false,
            };
            let attrs = &self.module.functions.get(function)?.attrs;
            self.require(root && !active_failure && attrs.contains(&FunctionAttr::Async),
                "cancel delivery requires a successful async action exit");
        }
        Ok(())
    }
    pub(super) fn direct_call(&mut self, function: FuncId, call: &CallSite) -> Checked<()> {
        let data = self.module.functions.get(function)?;
        if data.attrs.iter().any(|attr| matches!(attr, FunctionAttr::DeferEntry { .. })) {
            let tagged = match self.site {
                OperationSite::Terminator { block, .. } => self.body.blocks.get(block).is_ok_and(|block| {
                    matches!(block.term.kind, TermKind::InvokeDefer { function: callee, .. } if callee == function)
                }),
                _ => false,
            };
            if !tagged { self.report(R::T1, "defer entry must be called through invoke.defer"); }
        }
        if !self.state.array(data.signature.params.len(), 64)
            || !self.state.step(data.signature.params.len()) { return Err(TypeCheckError); }
        for attr in &data.attrs {
            let trusted = match attr {
                FunctionAttr::Extern(name) => self.state.context.external_contracts.native(name),
                FunctionAttr::Intrinsic { name, .. } => self.state.context.external_contracts.intrinsic(name),
                _ => continue,
            };
            self.require(trusted == Some(&data.signature), "call target has no matching trusted external ABI");
        }
        generic_arguments(self.module, GenericOwner::Function(function),
            &data.type_params, &call.type_args, self.state)?;
        self.substitution = substitution(GenericOwner::Function(function), &call.type_args);
        let params: Vec<_> = data.signature.params.iter().copied().filter(|id| {
            self.module.types.get(*id).is_ok_and(|ty| !matches!(ty, Type::Service(_) | Type::Descriptor))
        }).collect();
        self.call_site(call, &params, None)?;
        if let OwnershipAbi::Explicit { .. } = data.signature.ownership_abi {
            self.require(call.packet.is_some(), "explicit callee ABI needs an input packet");
        }
        if call.summary.is_some() && let Some(summary) = data.summary {
            self.require(Some(summary) == call.summary, "call summary differs from its target declaration");
        }
        let Some(summary) = call.summary else { return Ok(()); };
        let summary = self.module.summaries.get(summary)?;
        let language_results: Vec<_> = data.signature.results.iter().copied().filter(|id| {
            self.module.types.get(*id).is_ok_and(|ty| !matches!(ty, Type::Service(_)))
        }).collect();
        self.require(language_results == summary.contract.results,
            "summary result types differ from the target signature");
        Ok(())
    }
    fn call(&mut self, op: &CallOp) -> Checked<()> {
        match op {
            CallOp::Direct { function, call } => self.direct_call(*function, call)?,
            CallOp::Native { function, call } | CallOp::Intrinsic { function, call } => {
                self.direct_call(*function, call)?;
                let data = self.module.functions.get(*function)?;
                let signature = data.attrs.iter().find_map(|attr| match (op, attr) {
                    (CallOp::Native { .. }, FunctionAttr::Extern(name)) => {
                        self.state.context.external_contracts.native(name)
                    }
                    (CallOp::Intrinsic { .. }, FunctionAttr::Intrinsic { name, .. }) => {
                        self.state.context.external_contracts.intrinsic(name)
                    }
                    _ => None,
                });
                self.require(signature == Some(&data.signature),
                    "external ABI is absent from the trusted catalog or mismatches");
            }
            CallOp::Resume { frame, function, call } => {
                self.expect(*frame, Type::Frame(*function))?;
                self.direct_call(*function, call)?;
            }
            CallOp::Role { receiver, role, slot, call } => {
                let signature = self.role_signature(receiver.value, *role, *slot)?.clone();
                let params = language_slots(self.module, &signature.params, self.state);
                self.call_site(call, &params, None)?;
            }
            CallOp::Closure { callee, call } => {
                let Type::Fn(signature) = self.ty(callee.value)? else {
                    self.require(false, "fn.call requires a fn value");
                    return Err(TypeCheckError);
                };
                let params = signature.params.clone();
                let result = signature.result;
                self.call_site(call, &params, None)?;
                let Some(summary) = call.summary else { return Ok(()); };
        let summary = self.module.summaries.get(summary)?;
                self.require(summary.contract.results.as_slice() == result.as_slice(), "fn summary result mismatch");
                self.require(call.type_args.is_empty(), "fn.call cannot have nominal type arguments");
            }
            CallOp::Cap { cap, field, call } => {
                let Type::Cap { decl, args } = self.ty(cap.value)? else {
                    self.require(false, "call.cap requires a cap value");
                    return Err(TypeCheckError);
                };
                let decl = *decl;
                let args = args.clone();
                self.require(decl == field.declaration, "cap field belongs to another declaration");
                let DeclarationKind::Cap { owner, fields } = &self.module.declarations.get(decl)?.kind else {
                    return Err(TypeCheckError);
                };
                let Some(field) = fields.get(field.index as usize) else {
                    self.require(false, "cap field is out of range");
                    return Err(TypeCheckError);
                };
                self.substitution = substitution(GenericOwner::Declaration(decl), &args);
                let signature = &self.module.functions.get(field.function)?.signature;
                let valid = signature.params.first().is_some_and(|expected| {
                    matches_type(self.module, *expected, *owner, &self.substitution, self.state).unwrap_or(false)
                });
                self.require(valid, "cap implementation owner type mismatch");
                let params = language_slots(self.module, signature.params.get(1..).unwrap_or(&[]), self.state);
                self.call_site(call, &params, None)?;
            }
        }
        Ok(())
    }
    fn async_op(&mut self, op: &AsyncOp) -> Checked<()> {
        match op {
            AsyncOp::Await { future, result } | AsyncOp::AwaitShielded { future, result } => {
                let actual = self.future(*future)?;
                self.require(actual == *result, "await result annotation differs from the Future result");
            }
            AsyncOp::AwaitTake { future, result, unique, .. } => {
                let actual = self.future(*future)?;
                self.require(actual == *result, "await.take result annotation differs from the Future result");
                self.service(*unique, ServiceType::UniqueCache)?;
            }
            AsyncOp::UniqueCacheProof { proof } => {
                self.future(proof.future)?;
                match proof.evidence.creation {
                    CreationOrigin::Instruction(inst) => {
                        let results = &self.body.insts.get(inst)?.results;
                        let ty = value_type(self.body, proof.future)?;
                        let valid = results.iter().any(|v| value_type(self.body, *v).is_ok_and(|actual| actual == ty));
                        self.require(valid, "proof instruction origin has no matching Future result");
                    }
                    CreationOrigin::Invoke { block, outcome, slot } => {
                        let TermKind::InvokeOwn { op, .. } = &self.body.blocks.get(block)?.term.kind else {
                            self.require(false, "proof invoke origin is not an invoke");
                            return Ok(());
                        };
                        let contract = ContractRegistry::new(self.module).lookup(op).map_err(|_| TypeCheckError)?;
                        let result = contract.outcome(outcome).and_then(|schema| schema.results.get(usize::from(slot)));
                        let valid = result.is_some_and(|result| super::edge_slot_matches(self.module, self.body, op,
                            &result.ty, value_type(self.body, proof.future).unwrap_or(TypeId::from_u32(u32::MAX)),
                            self.state));
                        self.require(valid, "proof invoke origin has an invalid outcome/slot/Future type");
                    }
                }
                for &alias in &proof.evidence.aliases { self.same(proof.future, alias)?; }
            }
            AsyncOp::CacheRehome { state, .. } => self.service(*state, ServiceType::CacheState)?,
            AsyncOp::FutureSpawn { future } | AsyncOp::FutureCancel { future }
            | AsyncOp::FutureDone { future } | AsyncOp::FutureCancelled { future } => { self.future(*future)?; }
            AsyncOp::FutureNew { environment, packet, deps, owner, owner_state, result } => {
                match environment {
                    FutureEnvironment::Deferred { function, arguments, descriptors } => {
                        let signature = &self.module.functions.get(*function)?.signature;
                        let params: Vec<_> = signature.params.iter().copied().filter(|ty| {
                            self.module.types.get(*ty).is_ok_and(|ty| !matches!(ty,
                                Type::Service(_) | Type::Descriptor))
                        }).collect();
                        self.arguments(arguments, &params)?;
                        let results: Vec<_> = signature.results.iter().copied().filter(|ty| {
                            self.module.types.get(*ty).is_ok_and(|ty| !matches!(ty, Type::Service(_)))
                        }).collect();
                        self.require(results == result.iter().copied().collect::<Vec<_>>(),
                            "Future environment body result differs from Future result");
                        let count = signature.params.iter().filter(|ty| {
                            self.module.types.get(**ty) == Ok(&Type::Descriptor)
                        }).count();
                        self.require(descriptors.len() == count, "Future environment dictionary arity mismatch");
                        for descriptor in descriptors {
                            self.expect(*descriptor, Type::Descriptor)?;
                        }
                        self.require(self.module.profile == Profile::Own,
                            "deferred Future environment belongs to own");
                    }
                    FutureEnvironment::Frame(frame) => {
                        self.frame(*frame)?;
                        self.require(self.module.profile != Profile::Own,
                            "own Future requires a deferred environment");
                    }
                }
                self.service(*packet, ServiceType::Own)?;
                self.service(*deps, ServiceType::Deps)?;
                self.service(*owner_state, ServiceType::Scope(*owner))?;
            }
            AsyncOp::FuturePublish { task, cache, value, packet, deps, failure, .. } => {
                self.service(*task, ServiceType::TaskState)?;
                self.service(*cache, ServiceType::CacheState)?;
                if let Some(value) = value { self.language(*value)?; }
                self.service(*packet, ServiceType::Own)?;
                self.service(*deps, ServiceType::Deps)?;
                if let Some(failure) = failure { self.service(*failure, ServiceType::Failure)?; }
            }
            AsyncOp::FutureSettle { task, .. } => self.service(*task, ServiceType::TaskState)?,
            AsyncOp::FrameAlloc { function } => { self.module.functions.get(*function)?; }
            AsyncOp::FrameLoad { frame, ty, .. } => {
                self.frame(*frame)?;
                let valid = non_linear(self.module.types.get(*ty)?);
                self.require(valid, "frame.load cannot copy a linear slot");
            }
            AsyncOp::FrameStore { frame, value, .. } => {
                self.frame(*frame)?;
                let valid = non_linear(self.ty(*value)?);
                self.require(valid, "frame.store cannot copy a linear slot");
            }
            AsyncOp::FrameSave { frame, slots, .. } => {
                self.frame(*frame)?;
                let mut seen = std::collections::BTreeSet::new();
                for slot in slots {
                    self.ty(slot.value)?;
                    self.require(seen.insert(slot.slot), "duplicate frame slot");
                }
            }
            AsyncOp::FrameResume { frame, slots } => {
                self.frame(*frame)?;
                let mut seen = std::collections::BTreeSet::new();
                for slot in slots {
                    let valid = !non_linear(self.module.types.get(slot.ty)?);
                    self.require(valid == slot.linear, "saved slot linear flag differs from its type");
                    self.require(seen.insert(slot.slot), "duplicate frame slot");
                }
            }
            AsyncOp::FrameCancel { frame, cleanup, .. } => {
                self.frame(*frame)?;
                self.service(*cleanup, ServiceType::CleanupState)?;
            }
            AsyncOp::Pin { value, deps, owner, .. } => {
                self.language(*value)?;
                self.service(*deps, ServiceType::Deps)?;
                self.owner(*owner)?;
            }
            AsyncOp::PinTransfer { pin, owner, .. } | AsyncOp::Unpin { pin, owner, .. } => {
                self.service(*pin, ServiceType::Pin)?;
                self.owner(*owner)?;
            }
            AsyncOp::WorkerBind { action, entry, fields, ty } => {
                let valid = matches!(self.ty(*action)?, Type::Fn(_));
                self.require(valid, "worker action must be a fn");
                self.exact(*action, *ty)?;
                let valid = matches!(self.ty(*entry)?, Type::Fn(_));
                self.require(valid, "worker entry must be a fn");
                for &field in fields { self.language(field)?; }
                self.module.types.get(*ty)?;
            }
            AsyncOp::WorkerSpawn { entry, args, result, .. } => {
                let Type::Fn(signature) = self.ty(*entry)? else {
                    self.require(false, "worker entry must be a fn");
                    return Err(TypeCheckError);
                };
                let signature = signature.clone();
                self.arguments(args, &signature.params)?;
                self.require(signature.result == *result, "worker result type mismatch");
            }
            AsyncOp::ProcessSpawn { constructor, constructor_args, action, args, result } => {
                let constructor = self.module.functions.get(*constructor)?;
                self.arguments(constructor_args, &constructor.signature.params)?;
                let action = self.module.functions.get(*action)?;
                self.arguments(args, &action.signature.params)?;
                self.require(action.signature.results.as_slice() == result.as_slice(), "process result type mismatch");
            }
        }
        Ok(())
    }
    fn carrier(&mut self, op: &CarrierOp) -> Checked<()> {
        match op {
            CarrierOp::Attach { failure, packet, owner_state, .. } => {
                self.service(*failure, ServiceType::Failure)?;
                self.service(*packet, ServiceType::Own)?;
                self.owner(*owner_state)?;
            }
            CarrierOp::Transfer { state, target, .. } => {
                self.service(*state, ServiceType::CarrierState)?;
                match target {
                    CarrierDestination::FailureContext(_) => {}
                    CarrierDestination::Cache(value) => self.service(*value, ServiceType::CacheState)?,
                    CarrierDestination::Diagnostic(value) => self.owner(*value)?,
                }
            }
            CarrierOp::Rehome { state, owner_state, .. } | CarrierOp::Release { state, owner_state, .. } => {
                self.service(*state, ServiceType::CarrierState)?;
                self.owner(*owner_state)?;
            }
        }
        Ok(())
    }
    fn exit(&mut self, op: &ExitOp) -> Checked<()> {
        match op {
            ExitOp::Prepare { exit } => self.prepared(exit)?,
            ExitOp::Replace { pending, replacement } => {
                self.service(*pending, ServiceType::Pending)?;
                self.prepared(replacement)?;
            }
            ExitOp::Extend { pending, packet } => {
                self.service(*pending, ServiceType::Pending)?;
                self.service(*packet, ServiceType::Own)?;
            }
            ExitOp::Commit { pending, .. } => {
                self.service(*pending, ServiceType::Pending)?;
            }
            ExitOp::Abort { pending, cleanup, .. } => {
                self.service(*pending, ServiceType::Pending)?;
                self.service(*cleanup, ServiceType::CleanupState)?;
            }
        }
        Ok(())
    }
    fn cleanup(&mut self, op: &CleanupOp) -> Checked<()> {
        match op {
            CleanupOp::Begin { state, pending, failure, close, effects } => {
                self.close_envelope(effects, failure.is_some(), *state)?;
                let source_type = self.ty(*state)?.clone();
                if let Ok(descriptor) = self.body.cleanup.closes.get(*close) {
                    let expected = match descriptor.source {
                        CleanupSource::Scope(scope) => ServiceType::Scope(scope),
                        CleanupSource::Packet(_) => ServiceType::Own,
                    };
                    self.require(source_type == Type::Service(expected), "cleanup source type differs from close");
                    self.require(descriptor.reserve == pending.is_some(), "cleanup reserve differs from close");
                    self.require(descriptor.active_failure == failure.is_some(), "cleanup failure differs from close");
                    self.require(descriptor.effects == *effects, "cleanup D18 differs from close");
                } else { self.report(R::T1, "invalid cleanup close ID"); }
                if let Some(pending) = pending { self.service(*pending, ServiceType::Pending)?; }
                if let Some(failure) = failure { self.service(*failure, ServiceType::Failure)?; }

            }
            CleanupOp::Defer { cleanup, action } => {
                self.service(*cleanup, ServiceType::CleanupState)?;
                self.require(matches!(self.ty(*action)?, Type::Fn(_)), "cleanup defer action must be a fn");
            }
            CleanupOp::Settle { cleanup, holder, .. } => {
                self.service(*cleanup, ServiceType::CleanupState)?;
                let valid = matches!(self.ty(*holder)?, Type::Service(ServiceType::Holder | ServiceType::TaskState
                    | ServiceType::CacheState | ServiceType::CarrierState | ServiceType::Pin | ServiceType::Receipt)
                    | Type::Frame(_));
                self.require(valid, "cleanup settle requires a service holder");
            }
            CleanupOp::SettleEmpty { cleanup, schedule } => {
                self.service(*cleanup, ServiceType::CleanupState)?;
                let valid = self.body.cleanup.schedules.get(*schedule).is_ok_and(|item| item.holders.is_empty()
                    && self.body.cleanup.closes.get(item.close).is_ok());
                self.require(valid, "empty settle requires a checked empty D16 schedule");
            }
            CleanupOp::Plan { cleanup, plan } => {
                self.service(*cleanup, ServiceType::CleanupState)?;
                if let Ok(plan) = self.body.cleanup.plans.get(*plan)
                    && let Ok(close) = self.body.cleanup.closes.get(plan.close) {
                    let mut expected = vec![match close.source {
                        CleanupSource::Scope(_) => MergeSource::ScopeInventory,
                        CleanupSource::Packet(_) => MergeSource::PacketInventory,
                    }];
                    if close.reserve {
                        expected.push(MergeSource::WholeReserveOnFailure);
                    }
                    self.require(plan.sources == expected,
                        "merge plan must cover its complete close and reserve sources");

                } else {
                    self.report(R::T1, "invalid cleanup plan ID");
                }
            }
            CleanupOp::Destroy { cleanup, key } => {
                self.service(*cleanup, ServiceType::CleanupState)?;
                self.service(*key, ServiceType::StableKey)?;
            }
            CleanupOp::Shield { cleanup }
            | CleanupOp::DeliverCancel { cleanup } | CleanupOp::Finish { cleanup } => {
                self.service(*cleanup, ServiceType::CleanupState)?;
            }
            CleanupOp::DestroyDirect { cleanup, identity, descriptor, destroy } => {
                self.service(*cleanup, ServiceType::CleanupState)?;
                self.language(*identity)?;
                self.expect(*descriptor, Type::Descriptor)?;
                let signature = &self.module.functions.get(*destroy)?.signature;
                self.arguments(&[*identity], &signature.params)?;
                self.require(signature.results.is_empty(), "destroy must return void");
            }
            CleanupOp::RecordFailure { cleanup, failure } => {
                self.service(*cleanup, ServiceType::CleanupState)?;
                self.service(*failure, ServiceType::Failure)?;
            }
            CleanupOp::CellNew { value, ty } => {
                self.exact(*value, *ty)?;
                self.require(non_linear(self.module.types.get(*ty)?), "cell cannot store a linear value");
            }
            CleanupOp::CellLoad { cell } => {
                let valid = match self.ty(*cell)? {
                    Type::LowCell(ty) => non_linear(self.module.types.get(*ty)?),
                    _ => false,
                };
                self.require(valid, "cell.load requires a cell containing a non-linear value");
            }
            CleanupOp::CellStore { cell, value } => {
                let Type::LowCell(ty) = self.ty(*cell)? else {
                    self.require(false, "expected low cell");
                    return Err(TypeCheckError);
                };
                let ty = *ty;
                self.exact(*value, ty)?;
                self.require(non_linear(self.ty(*value)?), "cell cannot store a linear value");
            }
        }
        Ok(())
    }
}
fn non_linear(ty: &Type) -> bool {
    !matches!(ty, Type::Service(service) if service.is_linear())
        && !matches!(ty, Type::Builder(_) | Type::MapBuilder(..) | Type::Frame(_))
}
fn retaggable(module: &Module, left: TypeId, right: TypeId, state: &mut State<'_>) -> Checked<bool> {
    let mut stack = vec![(left, right)];
    let mut seen = std::collections::BTreeSet::new();
    while let Some((left, right)) = stack.pop() {
        if !state.step(1) || !state.memory(64) { return Err(TypeCheckError); }
        if !seen.insert((left, right)) || left == right { continue; }
        match (module.types.get(left)?, module.types.get(right)?) {
            (Type::Never, _) => {}
            (Type::Array(a), Type::Array(b)) => stack.push((*a, *b)),
            (Type::Map(ak, av), Type::Map(bk, bv)) => {
                stack.push((*ak, *bk));
                stack.push((*av, *bv));
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}
