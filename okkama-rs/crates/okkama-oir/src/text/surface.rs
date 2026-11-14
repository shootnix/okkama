use super::*;

impl Codec for ValueId {
    fn positional(_name: &str) -> bool {
        true
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        if r.peek().text.starts_with('%') && !r.peek().quoted {
            return Ok(Self::from_u32(r.reference("value", "%", true)?));
        }
        let (op, ty) = if r.eat("own.empty") {
            (Operation::Ownership(OwnershipOp::Empty {}), Type::Service(ServiceType::Own))
        } else {
            let saved = r.literal_type.take();
            let literal = Literal::read(r)?;
            r.literal_type = saved;
            let ty = match &literal {
                Literal::Int(_) => Type::Int, Literal::Bool(_) => Type::Bool, Literal::Byte(_) => Type::Byte,
                Literal::Float(_) => Type::Float, Literal::Rat {
                        ..
                } => Type::Rat,
                Literal::Char(_) => Type::Char, Literal::Str(_) => Type::Str, Literal::Enum {
                    decl,
                        ..
                } => Type::Enum(*decl),
            };
            (Operation::Const(literal), ty)
        };
        let ty = r.types.intern(ty).map_err(|e| r.arena_error(e))?;
        let inst = InstId::from_u32(r.anonymous("inst")?);
        let value = Self::from_u32(r.anonymous("value")?);
        r.values.alloc(ValueData {
            ty, def: Definition::InstResult { inst, index: 0 }, facts: value_default(r.profile(), r.types.get(ty).ok()),
        }).map_err(|e| r.arena_error(e))?;
        r.insts.alloc(Instruction {
            results: vec![value], op, facts: InstructionFacts::default(), loc: None,
        }).map_err(|e| r.arena_error(e))?;
        r.implicit.push(inst);
        Ok(value)
    }

    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.reference("value", self.as_u32(), true)
    }
}

impl Codec for SelectionId {
    fn positional(_name: &str) -> bool {
        true
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        if !r.eat("selection") {
            return Ok(Self::from_u32(r.reference("selection", "s", true)?));
        }
        r.expect("(")?;
        let value = ValueId::read(r)?;
        r.expect(")")?;
        let id = Self::from_u32(r.anonymous("selection")?);
        r.selections.alloc(SelectionExpr::Value(value)).map_err(|e| r.arena_error(e))?;
        Ok(id)
    }

    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        if let Some(body) = w.body
            && let SelectionExpr::Value(value) = body.selections.get(*self)
                .map_err(|e| PrintError::new(e.to_string()))?
        {
            w.push("selection(");
            value.write(w)?;
            w.push(")");
            return Ok(());
        }
        w.reference("selection", self.as_u32(), true)
    }
}

pub(super) fn read_operation(r: &mut Reader<'_>) -> Result<Option<Operation>, Diagnostic> {
    let tag = r.peek().text.clone();
    let op = match tag.as_str() {
        "call" | "call.native" | "call.intrinsic" => {
            r.next()?;
            let function = FuncId::read(r)?;
            let default = r.names.get(&(0, "function-summary", function.as_u32().to_string())).copied().map(SummaryId::from_u32);
            let call = read_call(r, default)?;
            Operation::Call(match tag.as_str() {
                "call.native" => CallOp::Native {
                    function, call
                },
                "call.intrinsic" => CallOp::Intrinsic {
                    function, call
                },
                _ => CallOp::Direct {
                    function, call
                },
            })
        }
        "call.role" => {
            r.next()?;
            let receiver = read_argument(r)?;
            r.expect(",")?;
            let role = DeclId::read(r)?;
            r.expect("slot")?;
            let slot = u32::read(r)?;
            let call = read_call(r, None)?;
            Operation::Call(CallOp::Role { receiver, role, slot, call })
        }
        "fn.call" => {
            r.next()?;
            let callee = read_argument(r)?;
            let call = read_call(r, None)?;
            Operation::Call(CallOp::Closure { callee, call })
        }
        "call.cap" => {
            r.next()?;
            let cap = read_argument(r)?;
            r.expect(",")?;
            let field = read_field(r, cap.value)?;
            let call = read_call(r, None)?;
            Operation::Call(CallOp::Cap { cap, field, call })
        }
        "call.resume" => {
            r.next()?;
            let frame = ValueId::read(r)?;
            r.expect(",")?;
            let function = FuncId::read(r)?;
            let call = read_call(r, None)?;
            Operation::Call(CallOp::Resume { frame, function, call })
        }
        "obj.next" => {
            r.next()?;
            let object = ValueId::read(r)?;
            r.expect("{")?;
            let mut updates = vec![];
            while !r.is("}") && !r.is(";") {
                r.entity()?;
                let field = read_field(r, object)?;
                r.expect(":")?;
                let value = ValueId::read(r)?;
                updates.push(FieldValue { field, value });
                if !r.eat(",") {
                    break;
                }
            }
            let packet = if r.eat(";") {
                Some(ValueId::read(r)?)
            } else {
                None
            };
            r.expect("}")?;
            Operation::Core(CoreOp::ObjNext { object, updates, packet })
        }
        "obj.new" => {
            r.next()?;
            let declaration = DeclId::read(r)?;
            let type_args = r.type_arguments()?;
            r.expect("{")?;
            let mut fields = vec![];
            while !r.is("}") && !r.is(";") {
                r.entity()?;
                let name = r.next()?;
                if !r.first {
                    let index = name.text.parse::<usize>().ok().or_else(|| {
                        r.names.get(&(declaration.as_u32() as usize, "field", name.text.clone()))
                            .map(|index| *index as usize)
                    });
                    if name.quoted || index != Some(fields.len()) {
                        return Err(r.at(&name, DiagnosticKind::Syntax, "expected field in declaration order"));
                    }
                }
                r.expect(":")?;
                fields.push(ValueId::read(r)?);
                if !r.eat(",") {
                    break;
                }
            }
            let packet = if r.eat(";") {
                Some(ValueId::read(r)?)
            } else {
                None
            };
            r.expect("}")?;
            let construction = if r.eat("construction") {
                ConstructionStatus::read(r)?
            } else {
                ConstructionStatus::Complete
            };
            Operation::Core(CoreOp::ObjNew { declaration, type_args, fields, packet, construction })
        }
        "obj.get" | "obj.pending" | "once.load" | "once.store" => {
            r.next()?;
            let object = ValueId::read(r)?;
            r.expect(",")?;
            let field = read_field(r, object)?;
            Operation::Core(match tag.as_str() {
                "obj.pending" => CoreOp::ObjPending {
                    object, field
                },
                "once.load" => CoreOp::OnceLoad {
                    object, field
                },
                "once.store" => {
                    r.expect(",")?;
                    CoreOp::OnceStore { object, field, value: ValueId::read(r)? }
                }
                _ => CoreOp::ObjGet {
                    object, field
                },
            })
        }
        "exit.prepare" => {
            r.next()?;
            Operation::Exit(ExitOp::Prepare { exit: read_exit(r)? })
        }
        _ => return Ok(None),
    };
    Ok(Some(op))
}

pub(super) fn read_call(r: &mut Reader<'_>, default_summary: Option<SummaryId>) -> Result<CallSite, Diagnostic> {
    let type_args = r.type_arguments()?;
    r.expect("(")?;
    let mut args = vec![];
    while !r.is(")") && !r.is(";") {
        r.entity()?;
        let convention = if r.eat("consume") {
            OperandConvention::Consume
        } else {
            OperandConvention::Borrow
        };
        let value = ValueId::read(r)?;
        args.push(CallArgument { value, convention });
        if !r.eat(",") {
            break;
        }
    }
    let packet = if r.eat(";") {
        Some(ValueId::read(r)?)
    } else {
        None
    };
    r.expect(")")?;
    let deps = if r.eat("deps") {
        Vec::read(r)?
    } else {
        vec![]
    };
    let descriptors = if r.eat("descriptors") {
        Vec::read(r)?
    } else {
        vec![]
    };
    let summary = if r.eat("summary") {
        r.expect("(")?;
        let summary = if r.eat("none") { None } else { Some(SummaryId::read(r)?) };
        r.expect(")")?;
        summary
    } else { default_summary };
    let cleanup = if r.operand_keyword("cleanup") {
        r.eat(",");
        r.expect("cleanup")?;
        CallCleanupMode::read(r)?
    } else {
        CallCleanupMode::Ordinary
    };
    Ok(CallSite { args, packet, deps, descriptors, type_args, summary, cleanup })
}

fn read_field(r: &mut Reader<'_>, object: ValueId) -> Result<FieldRef, Diagnostic> {
    if r.is("field_ref") {
        return FieldRef::read(r);
    }
    let name = r.next()?;
    let declaration = if let Ok(value) = r.values.get(object) {
        match r.types.get(value.ty) {
            Ok(Type::Object { decl, .. } | Type::Role { decl, .. } | Type::Cap { decl, .. }) => Some(*decl),
            _ => None,
        }
    } else {
        r.annotations.get(&(r.scope, object))
            .and_then(|name| r.names.get(&(0, "decl", name.clone())))
            .copied().map(DeclId::from_u32)
    };
    // The declaration pass records names before forward SSA types resolve.
    // Its sentinel never escapes into the returned module (the second pass).
    let declaration = declaration.or_else(|| r.first.then_some(DeclId::from_u32(u32::MAX)))
        .ok_or_else(|| r.at(&name, DiagnosticKind::Syntax, "field declaration cannot be inferred; expected field_ref(declaration=..., index=...)"))?;
    let index = if r.first {
        0
    } else {
        r.names.get(&(declaration.as_u32() as usize, "field", name.text.clone())).copied()
            .ok_or_else(|| r.at(&name, DiagnosticKind::UnknownName, "unknown field"))?
    };
    Ok(FieldRef { declaration, index })
}

fn read_exit(r: &mut Reader<'_>) -> Result<PreparedExit, Diagnostic> {
    let kind = ExitKind::read(r)?;
    let value = if r.peek().text.starts_with('%') {
        Some(ValueId::read(r)?)
    } else {
        None
    };
    let comma_packet = r.is(",") && r.tokens.get(r.pos + 1)
        .is_some_and(|token| token.quoted || token.text != "deps");
    let packet = if comma_packet {
        r.expect(",")?;
        Some(ValueId::read(r)?)
    } else if r.eat("packet") {
        Some(ValueId::read(r)?)
    } else {
        None
    };
    let deps = if r.eat(",") {
        r.expect("deps")?;
        r.expect("(")?;
        let value = ValueId::read(r)?;
        r.expect(")")?;
        Some(value)
    } else {
        None
    };
    let selection = if r.eat("select") {
        SelectionId::read(r)?
    } else if let Some(value) = value {
        let id = SelectionId::from_u32(r.anonymous("selection")?);
        r.selections.alloc(SelectionExpr::Value(value)).map_err(|e| r.arena_error(e))?;
        id
    } else {
        return Err(r.error(DiagnosticKind::Syntax, "exit without a value needs select"));
    };
    r.expect("at").map_err(|_| r.error(DiagnosticKind::Syntax, "exit.prepare requires an explicit boundary: at Bx"))?;
    let boundary = BoundaryId::read(r)?;
    Ok(PreparedExit { kind, value, packet, deps, selection, boundary })
}

pub(super) fn write_operation(op: &Operation, w: &mut Writer<'_>) -> Result<bool, PrintError> {
    match op {
        Operation::Call(CallOp::Direct { function, call } | CallOp::Native { function, call }
            | CallOp::Intrinsic { function, call }) => {
            w.push(op.opcode());
            w.push(" ");
            function.write(w)?;
            let default = w.module.functions.get(*function).map_err(|e| PrintError::new(e.to_string()))?.summary;
            write_call(call, default, w)?;
        }
        Operation::Call(CallOp::Role { receiver, role, slot, call }) => {
            w.push("call.role ");
            write_argument(receiver, w)?;
            w.push(", ");
            role.write(w)?;
            w.push(" slot ");
            slot.write(w)?;
            write_call(call, None, w)?;
        }
        Operation::Call(CallOp::Closure { callee, call }) => {
            w.push("fn.call ");
            write_argument(callee, w)?;
            write_call(call, None, w)?;
        }
        Operation::Call(CallOp::Cap { cap, field, call }) => {
            w.push("call.cap ");
            write_argument(cap, w)?;
            w.push(", ");
            write_field(cap.value, field, w)?;
            write_call(call, None, w)?;
        }
        Operation::Call(CallOp::Resume { frame, function, call }) => {
            w.push("call.resume ");
            frame.write(w)?;
            w.push(", ");
            function.write(w)?;
            write_call(call, None, w)?;
        }
        Operation::Core(CoreOp::ObjNext { object, updates, packet }) => {
            w.push("obj.next ");
            object.write(w)?;
            w.push(" {");
            for (i, update) in updates.iter().enumerate() {
                w.push(if i == 0 { " " } else { ", " });
                write_field(*object, &update.field, w)?;
                w.push(": ");
                update.value.write(w)?;
            }
            if let Some(packet) = packet {
                w.push("; ");
                packet.write(w)?;
            }
            w.push(" }");
        }
        Operation::Core(CoreOp::ObjNew { declaration, type_args, fields, packet, construction }) => {
            w.push("obj.new ");
            declaration.write(w)?;
            types::write_type_arguments(type_args, w)?;
            w.push(" {");
            for (i, value) in fields.iter().enumerate() {
                w.push(if i == 0 { " " } else { ", " });
                let name = w.module.declarations.get(*declaration).ok().and_then(|decl| match &decl.kind {
                    DeclarationKind::Object { fields, .. } => fields.get(i).map(|f| f.name.as_str()),
                    _ => None,
                });
                if let Some(name) = name {
                    w.word(name)?;
                } else {
                    w.push(&i.to_string());
                }
                w.push(": ");
                value.write(w)?;
            }
            if let Some(packet) = packet {
                w.push("; ");
                packet.write(w)?;
            }
            w.push(" }");
            if *construction != ConstructionStatus::Complete {
                w.push(" construction ");
                construction.write(w)?;
            }
        }
        Operation::Core(CoreOp::ObjGet { object, field } | CoreOp::ObjPending { object, field }
            | CoreOp::OnceLoad { object, field } | CoreOp::OnceStore { object, field, .. }) => {
            w.push(op.opcode());
            w.push(" ");
            object.write(w)?;
            w.push(", ");
            write_field(*object, field, w)?;
            if let Operation::Core(CoreOp::OnceStore {
                value, ..
            }) = op { w.push(", ");
                value.write(w)?;
            }
        }
        Operation::Exit(ExitOp::Prepare { exit }) => {
            w.push("exit.prepare ");
            exit.kind.write(w)?;
            if let Some(value) = exit.value {
                w.push(" ");
                value.write(w)?;
            }
            if let Some(packet) = exit.packet {
                w.push(if exit.value.is_some() { ", " } else { " packet " });
                packet.write(w)?;
            }
            if let Some(deps) = exit.deps {
                w.push(", deps(");
                deps.write(w)?;
                w.push(")");
            }
            let inferred = w.body.and_then(|b| b.selections.get(exit.selection).ok());
            if !matches!(inferred, Some(SelectionExpr::Value(v)) if Some(*v) == exit.value) {
                w.push(" select ");
                exit.selection.write(w)?;
            }
            w.push(" at ");
            exit.boundary.write(w)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub(super) fn write_call(call: &CallSite, default_summary: Option<SummaryId>, w: &mut Writer<'_>) -> Result<(),
    PrintError> {
    if let Some(summary) = call.summary { w.module.summaries.get(summary).map_err(|e| PrintError::new(e.to_string()))?; }
    types::write_type_arguments(&call.type_args, w)?;
    w.push("(");
    for (i, arg) in call.args.iter().enumerate() {
        if i > 0 {
            w.push(", ");
        }
        if arg.convention == OperandConvention::Consume {
            w.push("consume ");
        }
        arg.value.write(w)?;
    }
    if let Some(packet) = call.packet {
        w.push("; ");
        packet.write(w)?;
    }
    w.push(")");
    if !call.deps.is_empty() {
        w.push(" deps ");
        call.deps.write(w)?;
    }
    if !call.descriptors.is_empty() {
        w.push(" descriptors ");
        call.descriptors.write(w)?;
    }
    if call.cleanup == CallCleanupMode::Shielded || call.summary != default_summary {
        w.push(" summary(");
        if let Some(summary) = call.summary { summary.write(w)?; } else { w.push("none"); }
        w.push(")");
    }
    if call.cleanup == CallCleanupMode::Shielded {
        w.push(", cleanup shielded");
    }
    Ok(())
}

// Two schema passes keep SSA operands before keyword metadata, including when
// the Rust payload stores a type or descriptor before its SSA fields.
pub(super) fn read_phase<T: Codec>(
    slot: &mut Option<T>, r: &mut Reader<'_>, name: &str, positional: bool, first: &mut bool,
) -> Result<(), Diagnostic> {
    if T::positional(name) == positional {
        *slot = Some(T::read_operand(r, name, first)?);
    }
    Ok(())
}

pub(super) fn write_phase<T: Codec>(
    value: &T, w: &mut Writer<'_>, name: &str, positional: bool, first: &mut bool,
) -> Result<(), PrintError> {
    if T::positional(name) == positional {
        value.write_operand(w, name, first)?;
    }
    Ok(())
}

fn read_argument(r: &mut Reader<'_>) -> Result<CallArgument, Diagnostic> {
    let convention = if r.eat("consume") {
        OperandConvention::Consume
    } else {
        OperandConvention::Borrow
    };
    let value = ValueId::read(r)?;
    Ok(CallArgument { value, convention })
}

fn write_argument(arg: &CallArgument, w: &mut Writer<'_>) -> Result<(), PrintError> {
    if arg.convention == OperandConvention::Consume {
        w.push("consume ");
    }
    arg.value.write(w)
}

fn write_field(object: ValueId, field: &FieldRef, w: &mut Writer<'_>) -> Result<(), PrintError> {
    let inferred = w.body.and_then(|b| b.values.get(object).ok())
        .and_then(|v| w.module.types.get(v.ty).ok()).and_then(|ty| match ty {
            Type::Object { decl, .. } | Type::Role { decl, .. } | Type::Cap { decl, .. } => Some(*decl),
            _ => None,
        });
    let name = w.module.declarations.get(field.declaration).ok().and_then(|decl| match &decl.kind {
        DeclarationKind::Object { fields, .. } => fields.get(field.index as usize).map(|f| f.name.as_str()),
        _ => None,
    });
    if inferred == Some(field.declaration) {
        if let Some(name) = name {
            w.word(name)?;
        } else {
            field.write(w)?;
        }
    } else {
        field.write(w)?;
    }
    Ok(())
}
