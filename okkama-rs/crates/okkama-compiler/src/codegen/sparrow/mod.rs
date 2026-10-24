//! Проверенный IR → программа Sparrow (SPB 9), как IR → JVM-байткод.
//!
//! Значение SSA `%n` становится регистром `rn` кадра; блоки раскладываются
//! подряд, переход к блоку — переход к его первой инструкции с параллельной
//! записью параметров блока. Значение, которое IR передаёт последним
//! параметром цели (`catch`, `lookup`, отказ `unwind`), пишет сама
//! инструкция (или обработчик). Цель `unwind` блока — обработчик диапазона
//! его инструкций, включая терминатор. Таблицы модуля переносятся целиком.
use crate::{ir, source::Span};
use okkama_sparrow::{
    BitOp, CapDecl, CapRef, CatchKind, Cmp, Constant, Dst, EnumDecl, EnumRef, FuncRef, FutOp, Function,
    Handler, Instruction as I, Intrinsic, ObjectDecl, ObjectRef, Program, Register, RoleDecl, RoleMethod,
    RoleRef, Target, TrapCode, Type, TypeRef, Witness,
};
use std::{collections::HashMap, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerateError {
    pub span: Span,
    pub message: String,
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sparrow codegen: {}", self.message)
    }
}

impl std::error::Error for GenerateError {}

fn error(span: Span, message: impl Into<String>) -> GenerateError {
    GenerateError {
        span,
        message: message.into(),
    }
}

/// Таблица типов: каждый тип IR — одна запись, вложенные раньше внешних.
#[derive(Default)]
struct Types {
    table: Vec<Type>,
    index: HashMap<ir::Type, TypeRef>,
}

impl Types {
    fn intern(&mut self, ty: &ir::Type) -> TypeRef {
        if let Some(found) = self.index.get(ty) {
            return *found;
        }
        let entry = match ty {
            ir::Type::Int => Type::Int,
            ir::Type::Bool => Type::Bool,
            ir::Type::Byte => Type::Byte,
            ir::Type::Float => Type::Float,
            ir::Type::Rat => Type::Rat,
            ir::Type::Char => Type::Char,
            ir::Type::Str => Type::Str,
            ir::Type::Object(object) => Type::Object(ObjectRef(object.0 as u32)),
            ir::Type::Successor(object) => Type::Successor(ObjectRef(object.0 as u32)),
            ir::Type::Role(role) => Type::Role(RoleRef(role.0 as u32)),
            ir::Type::Enum(id) => Type::Enum(EnumRef(id.0 as u32)),
            ir::Type::Failure => Type::Failure,
            ir::Type::Array(item) => Type::Array(self.intern(item)),
            ir::Type::Never => Type::Never,
            ir::Type::Builder(item) => Type::Builder(self.intern(item)),
            ir::Type::Map(key, value) => {
                let key = self.intern(key);
                Type::Map(key, self.intern(value))
            }
            ir::Type::MapBuilder(key, value) => {
                let key = self.intern(key);
                Type::MapBuilder(key, self.intern(value))
            }
            ir::Type::Region => Type::Region,
            ir::Type::Handle(handle) => Type::Handle(handle.0 as u32),
            ir::Type::Any => Type::Any,
            ir::Type::Range => Type::Range,
            ir::Type::Fn(signature) => {
                let params = signature.params.iter().map(|param| self.intern(param)).collect();
                let result = self.result(&signature.result);
                Type::Fn { params, result }
            }
            ir::Type::Future(result) => Type::Future(self.result(result)),
            ir::Type::Cap(cap) => Type::Cap(CapRef(cap.0 as u32)),
        };
        let reference = TypeRef(self.table.len() as u32);
        self.table.push(entry);
        self.index.insert(ty.clone(), reference);
        reference
    }

    fn result(&mut self, result: &ir::ResultType) -> Option<TypeRef> {
        match result {
            ir::ResultType::Value(ty) => Some(self.intern(ty)),
            ir::ResultType::Void => None,
        }
    }
}

fn cmp(op: ir::CompareOp) -> Cmp {
    match op {
        ir::CompareOp::Eq => Cmp::Eq,
        ir::CompareOp::Ne => Cmp::Ne,
        ir::CompareOp::Lt => Cmp::Lt,
        ir::CompareOp::Le => Cmp::Le,
        ir::CompareOp::Gt => Cmp::Gt,
        ir::CompareOp::Ge => Cmp::Ge,
    }
}

fn reg(value: ir::ValueId) -> Register {
    Register(value.0 as u32)
}

fn regs(values: &[ir::ValueId]) -> Vec<Register> {
    values.iter().map(|value| reg(*value)).collect()
}

/// Генератор одной функции: начала блоков и типы значений.
struct FunctionGen<'a> {
    module: &'a ir::Module,
    function: &'a ir::Function,
    starts: Vec<u32>,
}

impl FunctionGen<'_> {
    /// Переход без значения ребра: все параметры цели из аргументов.
    fn target(&self, target: &ir::Target, span: Span) -> Result<Target, GenerateError> {
        let params = &self.function.blocks[target.block.0].params;
        if params.len() != target.args.len() {
            return Err(error(span, "branch argument count does not match block parameters"));
        }
        Ok(Target {
            pc: self.starts[target.block.0],
            moves: params.iter().zip(&target.args).map(|(param, arg)| (reg(*param), reg(*arg))).collect(),
        })
    }

    /// Переход со значением ребра: последний параметр цели пишет инструкция.
    fn payload_target(&self, target: &ir::Target, span: Span) -> Result<(Target, Register), GenerateError> {
        let params = &self.function.blocks[target.block.0].params;
        let Some((last, rest)) = params.split_last() else {
            return Err(error(span, "edge value target has no parameter"));
        };
        if rest.len() != target.args.len() {
            return Err(error(span, "branch argument count does not match block parameters"));
        }
        Ok((
            Target {
                pc: self.starts[target.block.0],
                moves: rest.iter().zip(&target.args).map(|(param, arg)| (reg(*param), reg(*arg))).collect(),
            },
            reg(*last),
        ))
    }

    fn value_type(&self, value: ir::ValueId) -> &ir::Type {
        &self.function.values[value.0]
    }

    fn instruction(&self, inst: &ir::Inst) -> Result<I, GenerateError> {
        use ir::InstKind as K;
        let span = inst.span;
        let dst = || inst.result.map(|value| Dst(reg(value))).ok_or_else(|| error(span, "instruction has no result"));
        let opt = inst.result.map(|value| Dst(reg(value)));
        let result_role = || match inst.result.map(|value| self.value_type(value)) {
            Some(ir::Type::Role(role)) => Ok(RoleRef(role.0 as u32)),
            _ => Err(error(span, "role result type is missing")),
        };
        Ok(match &inst.kind {
            K::Const(constant) => I::Const {
                dst: dst()?,
                value: match constant {
                    ir::Constant::Int(value) => Constant::Int(value.to_string()),
                    ir::Constant::Bool(value) => Constant::Bool(*value),
                    ir::Constant::Byte(value) => Constant::Byte(*value),
                    ir::Constant::Float(bits) => Constant::Float(*bits),
                    ir::Constant::Char(value) => Constant::Char(*value),
                    ir::Constant::Str(value) => Constant::Str(value.clone()),
                },
            },
            K::IntBinary { op, left, right } => {
                let (dst, left, right) = (dst()?, reg(*left), reg(*right));
                match op {
                    ir::IntBinaryOp::Add => I::IntAdd { dst, left, right },
                    ir::IntBinaryOp::Sub => I::IntSub { dst, left, right },
                    ir::IntBinaryOp::Mul => I::IntMul { dst, left, right },
                    ir::IntBinaryOp::Mod => I::IntMod { dst, left, right },
                    ir::IntBinaryOp::And => I::IntAnd { dst, left, right },
                    ir::IntBinaryOp::Or => I::IntOr { dst, left, right },
                    ir::IntBinaryOp::Xor => I::IntXor { dst, left, right },
                    ir::IntBinaryOp::Shl => I::IntShl { dst, left, right },
                    ir::IntBinaryOp::Shr => I::IntShr { dst, left, right },
                    ir::IntBinaryOp::Ushr => I::IntUshr { dst, left, right },
                }
            }
            K::IntUnary { op, operand } => match op {
                ir::IntUnaryOp::Neg => I::IntNeg { dst: dst()?, operand: reg(*operand) },
                ir::IntUnaryOp::Not => I::IntNot { dst: dst()?, operand: reg(*operand) },
            },
            K::IntCompare { op, left, right } => I::IntCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::BoolNot(value) => I::BoolNot { dst: dst()?, operand: reg(*value) },
            K::BoolCompare { op, left, right } => I::BoolCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::StrConcat { left, right } => I::StrConcat { dst: dst()?, left: reg(*left), right: reg(*right) },
            K::StrCompare { op, left, right } => I::StrCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::ObjNew { object, fields } => I::ObjNew { dst: dst()?, object: ObjectRef(object.0 as u32), fields: regs(fields) },
            K::ObjInit { object, field, value } => I::ObjInit { object: reg(*object), field: *field as u32, value: reg(*value) },
            K::ObjGet { object, field } => I::ObjGet { dst: dst()?, object: reg(*object), field: *field as u32 },
            K::ObjNext { object, updates } => I::ObjNext {
                dst: dst()?,
                object: reg(*object),
                updates: updates.iter().map(|(field, value)| (*field as u32, reg(*value))).collect(),
            },
            K::Call { func, args } => I::Call { dst: opt, function: FuncRef(func.0 as u32), args: regs(args) },
            K::SuccTake(value) | K::Retag(value) | K::MapRetag(value) | K::CapMake(value) => I::Move { dst: dst()?, source: reg(*value) },
            K::Cvt(value) => I::Convert { dst: dst()?, source: reg(*value) },
            K::RoleWrap(value) => I::RoleWrap { dst: dst()?, source: reg(*value), role: result_role()? },
            K::AnyBox(value) => I::AnyBox { dst: dst()?, source: reg(*value) },
            K::FnClosure { func, captures } => I::Closure { dst: dst()?, function: FuncRef(func.0 as u32), captures: regs(captures) },
            K::CallFn { callee, args } => I::CallFn { dst: opt, callee: reg(*callee), args: regs(args) },
            K::CallCap { cap, field } => {
                let ir::Type::Cap(id) = self.value_type(*cap) else {
                    return Err(error(span, "call.cap of a non-cap value"));
                };
                I::CallCap { dst: dst()?, cap: reg(*cap), ty: CapRef(id.0 as u32), field: *field as u32 }
            }
            K::Await(future) => I::Await { dst: opt, future: reg(*future) },
            K::FutSpawn(future) => I::FutSpawn { dst: dst()?, future: reg(*future) },
            K::WorkerSpawn { entry, args, direct } => I::WorkerSpawn { dst: dst()?, entry: reg(*entry), args: regs(args), direct: *direct },
            K::WorkerBind { action, entry, fields } => I::WorkerBind {
                dst: dst()?,
                action: reg(*action),
                entry: reg(*entry),
                fields: regs(fields),
            },
            K::Fut { op, future } => I::Fut {
                op: match op {
                    ir::FutOp::Cancel => FutOp::Cancel,
                    ir::FutOp::Done => FutOp::Done,
                    ir::FutOp::Cancelled => FutOp::Cancelled,
                },
                dst: opt,
                future: reg(*future),
            },
            K::Undef => I::Undef { dst: dst()? },
            K::RoleNarrow { value, message } => I::RoleNarrow {
                dst: dst()?,
                source: reg(*value),
                role: result_role()?,
                message: message.clone(),
            },
            K::CallRole { receiver, slot, args } => {
                let ir::Type::Role(role) = self.value_type(*receiver) else {
                    return Err(error(span, "role receiver type is missing"));
                };
                I::CallRole { dst: opt, receiver: reg(*receiver), role: RoleRef(role.0 as u32), slot: *slot as u32, args: regs(args) }
            }
            K::FailCaused { new, handled } => I::FailCaused { dst: dst()?, new: reg(*new), handled: reg(*handled) },
            K::ArrayNew(elements) => I::ArrayNew { dst: dst()?, elements: regs(elements) },
            K::ArrayLen(array) => I::ArrayLen { dst: dst()?, array: reg(*array) },
            K::ArrayGet { array, index } => I::ArrayGet { dst: dst()?, array: reg(*array), index: reg(*index) },
            K::ArraySlice { array, start, end } => I::ArraySlice { dst: dst()?, array: reg(*array), start: reg(*start), end: reg(*end) },
            K::ArrayJoin { array, separator } => I::ArrayJoin { dst: dst()?, array: reg(*array), separator: reg(*separator) },
            K::BuilderNew => I::BuilderNew { dst: dst()? },
            K::BuilderPush { builder, value } => I::BuilderPush { dst: dst()?, builder: reg(*builder), value: reg(*value) },
            K::BuilderAppend { builder, array } => I::BuilderAppend { dst: dst()?, builder: reg(*builder), array: reg(*array) },
            K::BuilderAppendRange { builder, start, end, inclusive } => I::BuilderRange {
                dst: dst()?,
                builder: reg(*builder),
                start: reg(*start),
                end: reg(*end),
                inclusive: *inclusive,
            },
            K::BuilderFinish(builder) => I::BuilderFinish { dst: dst()?, builder: reg(*builder) },
            K::Str { op, args } => {
                let dst = dst()?;
                let arg = |index: usize| args.get(index).map(|value| reg(*value)).ok_or_else(|| error(span, "string operation argument is missing"));
                if args.len() != op.arity() {
                    return Err(error(span, "string operation argument count mismatch"));
                }
                match op {
                    ir::StrOp::Len => I::StrLen { dst, text: arg(0)? },
                    ir::StrOp::Slice => I::StrSlice { dst, text: arg(0)?, start: arg(1)?, end: arg(2)? },
                    ir::StrOp::Split => I::StrSplit { dst, text: arg(0)?, separator: arg(1)? },
                    ir::StrOp::Replace => I::StrReplace { dst, text: arg(0)?, pattern: arg(1)?, replacement: arg(2)? },
                    ir::StrOp::ReplaceFirst => I::StrReplaceFirst { dst, text: arg(0)?, pattern: arg(1)?, replacement: arg(2)? },
                    ir::StrOp::Trim => I::StrTrim { dst, text: arg(0)? },
                    ir::StrOp::Chomp => I::StrChomp { dst, text: arg(0)? },
                    ir::StrOp::Utf8 => I::StrUtf8 { dst, text: arg(0)? },
                }
            }
            K::ValCompare { op, left, right } => I::ValCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::IntAbs(value) => I::IntAbs { dst: dst()?, operand: reg(*value) },
            K::IntSign(value) | K::FloatSign(value) | K::RatSign(value) => I::Sign { dst: dst()?, operand: reg(*value) },
            K::ByteBinary { op, left, right } => {
                let (dst, left, right) = (dst()?, reg(*left), reg(*right));
                match op {
                    ir::IntBinaryOp::And => I::ByteAnd { dst, left, right },
                    ir::IntBinaryOp::Or => I::ByteOr { dst, left, right },
                    ir::IntBinaryOp::Xor => I::ByteXor { dst, left, right },
                    ir::IntBinaryOp::Shl => I::ByteShl { dst, left, right },
                    ir::IntBinaryOp::Shr => I::ByteShr { dst, left, right },
                    ir::IntBinaryOp::Ushr => I::ByteUshr { dst, left, right },
                    other => return Err(error(span, format!("invalid byte operation {other:?}"))),
                }
            }
            K::ByteNot(value) => I::ByteNot { dst: dst()?, operand: reg(*value) },
            K::ByteFrom { value, context } => I::ByteFrom { dst: dst()?, source: reg(*value), context: context.clone() },
            K::ByteCompare { op, left, right } => I::ByteCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::CharCompare { op, left, right } => I::CharCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::RegionEnter => I::RegionEnter { dst: dst()? },
            K::RegionAdopt { region, value } => I::RegionAdopt { region: reg(*region), value: reg(*value) },
            K::RegionTransfer { from, to, value } => I::RegionTransfer { from: reg(*from), to: reg(*to), value: reg(*value) },
            K::RegionTrack { region, value } => I::RegionTrack { region: reg(*region), value: reg(*value) },
            K::RegionLeave { region, keep } => I::RegionLeave { region: reg(*region), keep: keep.map(reg) },
            K::RegionUnwind { region, failure } => I::RegionUnwind { dst: dst()?, region: reg(*region), failure: reg(*failure) },
            K::FailSecondary { primary, cleanup } => I::FailSecondary { dst: dst()?, primary: reg(*primary), cleanup: reg(*cleanup) },
            K::EnumCase { ty, case } => I::EnumCase { dst: dst()?, ty: EnumRef(ty.0 as u32), case: *case as u32 },
            K::EnumCompare { op, left, right } => I::EnumCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::Float { op, args } => {
                let dst = dst()?;
                if args.len() != op.arity() {
                    return Err(error(span, "float operation argument count mismatch"));
                }
                let left = reg(args[0]);
                let right = || reg(args[1]);
                match op {
                    ir::FloatOp::Add => I::FloatAdd { dst, left, right: right() },
                    ir::FloatOp::Sub => I::FloatSub { dst, left, right: right() },
                    ir::FloatOp::Mul => I::FloatMul { dst, left, right: right() },
                    ir::FloatOp::Div => I::FloatDiv { dst, left, right: right() },
                    ir::FloatOp::Mod => I::FloatMod { dst, left, right: right() },
                    ir::FloatOp::Pow => I::FloatPow { dst, left, right: right() },
                    ir::FloatOp::Neg => I::FloatNeg { dst, operand: left },
                    ir::FloatOp::Abs => I::FloatAbs { dst, operand: left },
                    ir::FloatOp::Sqrt => I::FloatSqrt { dst, operand: left },
                }
            }
            K::FloatCompare { op, left, right } => I::FloatCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::Rat { op, args } => {
                let dst = dst()?;
                if args.len() != op.arity() {
                    return Err(error(span, "rat operation argument count mismatch"));
                }
                let left = reg(args[0]);
                let right = || reg(args[1]);
                match op {
                    ir::RatOp::New => I::RatNew { dst, left, right: right() },
                    ir::RatOp::Add => I::RatAdd { dst, left, right: right() },
                    ir::RatOp::Sub => I::RatSub { dst, left, right: right() },
                    ir::RatOp::Mul => I::RatMul { dst, left, right: right() },
                    ir::RatOp::Div => I::RatDiv { dst, left, right: right() },
                    ir::RatOp::Mod => I::RatMod { dst, left, right: right() },
                    ir::RatOp::Neg => I::RatNeg { dst, operand: left },
                    ir::RatOp::Abs => I::RatAbs { dst, operand: left },
                }
            }
            K::RatCompare { op, left, right } => I::RatCmp { op: cmp(*op), dst: dst()?, left: reg(*left), right: reg(*right) },
            K::RatPart { value, denominator } => I::RatPart { dst: dst()?, value: reg(*value), denominator: *denominator },
            K::RangeNew { start, end, inclusive } => I::RangeNew { dst: dst()?, start: reg(*start), end: reg(*end), inclusive: *inclusive },
            K::RangePart { range, end } => I::RangePart { dst: dst()?, range: reg(*range), end: *end },
            K::Bits { op, byte, value, count } => I::Bits {
                op: match op {
                    ir::BitOp::BitCount => BitOp::BitCount,
                    ir::BitOp::LeadingZeros => BitOp::LeadingZeros,
                    ir::BitOp::TrailingZeros => BitOp::TrailingZeros,
                    ir::BitOp::RotateLeft => BitOp::RotateLeft,
                    ir::BitOp::RotateRight => BitOp::RotateRight,
                    ir::BitOp::ReverseBits => BitOp::ReverseBits,
                    ir::BitOp::SwapBytes => BitOp::SwapBytes,
                },
                byte: *byte,
                dst: dst()?,
                value: reg(*value),
                count: count.map(reg),
            },
            K::MapNew(entries) => I::MapNew { dst: dst()?, entries: entries.iter().map(|(key, value)| (reg(*key), reg(*value))).collect() },
            K::MapLen(map) => I::MapLen { dst: dst()?, map: reg(*map) },
            K::MapHas { map, key } => I::MapHas { dst: dst()?, map: reg(*map), key: reg(*key) },
            K::MapGet { map, key } => I::MapGet { dst: dst()?, map: reg(*map), key: reg(*key) },
            K::MapKeys(map) => I::MapKeys { dst: dst()?, map: reg(*map) },
            K::MapValues(map) => I::MapValues { dst: dst()?, map: reg(*map) },
            K::MapBuilderNew => I::MapBuilderNew { dst: dst()? },
            K::MapBuilderPut { builder, key, value } => I::MapBuilderPut {
                dst: dst()?,
                builder: reg(*builder),
                key: reg(*key),
                value: reg(*value),
            },
            K::MapBuilderFinish(builder) => I::MapBuilderFinish { dst: dst()?, builder: reg(*builder) },
        })
    }

    fn terminator(&self, term: &ir::Terminator) -> Result<I, GenerateError> {
        use ir::TermKind as T;
        let span = term.span;
        Ok(match &term.kind {
            T::Br(target) => I::Jump { target: self.target(target, span)? },
            T::Cond { condition, then_target, else_target } => I::Branch {
                condition: reg(*condition),
                then_target: self.target(then_target, span)?,
                else_target: self.target(else_target, span)?,
            },
            T::Ret(value) => I::Return { value: value.map(reg) },
            T::Panic(message) => I::Panic { message: reg(*message) },
            T::Trap { code, message } => I::Trap {
                code: match code {
                    ir::TrapCode::Confirm => TrapCode::Confirm,
                    ir::TrapCode::Range => TrapCode::Range,
                    ir::TrapCode::Empty => TrapCode::Empty,
                    ir::TrapCode::Unbound => TrapCode::Unbound,
                    ir::TrapCode::Unreachable => TrapCode::Unreachable,
                },
                message: message.clone(),
            },
            T::Unreachable => I::Unreachable {},
            T::Throw(value) => I::Throw { value: reg(*value) },
            T::Raise(value) => I::Raise { failure: reg(*value) },
            T::Catch { failure, ty, handler, next } => {
                let (handler, value) = self.payload_target(handler, span)?;
                let (kind, ty) = match ty {
                    ir::CatchType::Object(object) => (CatchKind::Object, object.0 as u32),
                    ir::CatchType::Role(role) => (CatchKind::Role, role.0 as u32),
                };
                I::Catch {
                    failure: reg(*failure),
                    kind,
                    ty,
                    value: Dst(value),
                    handler,
                    next: self.target(next, span)?,
                }
            }
            T::ArrayLookup { array, index, found, missing } => {
                let (found, value) = self.payload_target(found, span)?;
                I::ArrayLookup {
                    array: reg(*array),
                    index: reg(*index),
                    value: Dst(value),
                    found,
                    missing: self.target(missing, span)?,
                }
            }
            T::MapLookup { map, key, found, missing } => {
                let (found, value) = self.payload_target(found, span)?;
                I::MapLookup {
                    map: reg(*map),
                    key: reg(*key),
                    value: Dst(value),
                    found,
                    missing: self.target(missing, span)?,
                }
            }
        })
    }
}

fn function(module: &ir::Module, function: &ir::Function, types: &mut Types) -> Result<Function, GenerateError> {
    let registers = function.values.iter().map(|ty| types.intern(ty)).collect();
    let result = types.result(&function.result);
    let intrinsic = function.attrs.iter().find_map(|attr| match attr {
        ir::Attr::Intrinsic { name, objects, helpers } => Some(Intrinsic {
            name: name.clone(),
            objects: objects.iter().map(|object| ObjectRef(object.0 as u32)).collect(),
            helpers: helpers.iter().map(|helper| FuncRef(helper.0 as u32)).collect(),
        }),
        _ => None,
    });
    let native = function.extern_symbol().map(str::to_string);
    let mut out = Function {
        symbol: function.symbol.clone(),
        is_async: function.is_async(),
        native,
        intrinsic,
        registers,
        params: regs(&function.params),
        result,
        code: Vec::new(),
        handlers: Vec::new(),
    };
    if out.native.is_some() || out.intrinsic.is_some() {
        return Ok(out);
    }
    let mut starts = Vec::with_capacity(function.blocks.len());
    let mut pc = 0u32;
    for block in &function.blocks {
        starts.push(pc);
        pc += block.insts.len() as u32 + 1;
    }
    let generator = FunctionGen { module, function, starts };
    for (index, block) in function.blocks.iter().enumerate() {
        let start = generator.starts[index];
        for inst in &block.insts {
            out.code.push(generator.instruction(inst)?);
        }
        out.code.push(generator.terminator(&block.term)?);
        if let Some(unwind) = &block.unwind {
            let (target, failure) = generator.payload_target(unwind, block.term.span)?;
            out.handlers.push(Handler {
                start,
                end: start + block.insts.len() as u32 + 1,
                target,
                failure,
            });
        }
    }
    let _ = generator.module;
    Ok(out)
}

/// Модуль с разрешёнными импортами → программа Sparrow.
pub fn generate(module: &ir::Module) -> Result<Program, GenerateError> {
    ir::verify(module).map_err(|e| error(module.span, format!("invalid IR: {e}")))?;
    if !module.imports.is_empty() {
        return Err(error(module.span, "imports must be resolved before codegen"));
    }
    let mut types = Types::default();
    let enums = module
        .enums
        .iter()
        .map(|decl| EnumDecl {
            name: decl.name.clone(),
            cases: decl.cases.clone(),
        })
        .collect();
    let roles = module
        .roles
        .iter()
        .map(|role| RoleDecl {
            name: role.name.clone(),
            builtin: role.builtin,
            methods: role
                .methods
                .iter()
                .map(|method| RoleMethod {
                    name: method.name.clone(),
                    params: method.params.iter().map(|param| types.intern(param)).collect(),
                    result: types.result(&method.result),
                })
                .collect(),
        })
        .collect();
    let objects = module
        .objects
        .iter()
        .map(|object| ObjectDecl {
            name: object.name.clone(),
            fields: object.fields.iter().map(|field| (field.name.clone(), types.intern(&field.ty))).collect(),
            does: object
                .does
                .iter()
                .map(|witness| Witness {
                    role: RoleRef(witness.role.0 as u32),
                    slots: witness.slots.iter().map(|slot| FuncRef(slot.0 as u32)).collect(),
                })
                .collect(),
            destroy: object.destroy.map(|destroy| FuncRef(destroy.0 as u32)),
            builtin: object.builtin,
        })
        .collect();
    let handles = module.handles.iter().map(|handle| handle.name.clone()).collect();
    let caps = module
        .caps
        .iter()
        .map(|cap| CapDecl {
            name: cap.name.clone(),
            owner: ObjectRef(cap.owner.0 as u32),
            fields: cap.fields.iter().map(|(name, function)| (name.clone(), FuncRef(function.0 as u32))).collect(),
        })
        .collect();
    let functions = module
        .functions
        .iter()
        .map(|item| function(module, item, &mut types))
        .collect::<Result<Vec<_>, _>>()?;
    let program = Program {
        module: module.name.clone(),
        version: module.version.clone(),
        types: types.table,
        enums,
        roles,
        objects,
        handles,
        caps,
        functions,
        entry: module.entry.map(|entry| FuncRef(entry.0 as u32)),
        tests: module
            .tests
            .as_ref()
            .map(|tests| tests.iter().map(|(name, function)| (name.clone(), FuncRef(function.0 as u32))).collect()),
    };
    program
        .verify()
        .map_err(|e| error(module.span, format!("generated invalid Sparrow: {e}")))?;
    Ok(program)
}
