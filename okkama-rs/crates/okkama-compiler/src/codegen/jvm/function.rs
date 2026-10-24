//! Тело функции OIR → статический метод главного класса.

use super::asm::{self, Asm, Frame, VType};
use super::classfile::Method;
use super::runtime::{self, ClassGen, LIST, MAP};
use super::{BIG, Result, STRING, error};
use crate::ir::cfg::{Def, Dominators, UsePoint};
use crate::ir::{
    self, BlockId, CatchType, CompareOp, Constant, InstKind, IntBinaryOp, IntUnaryOp, StrOp,
    Target, TermKind, Type, ValueId,
};

const BIG_BINARY: &str = "(Ljava/math/BigInteger;Ljava/math/BigInteger;)Ljava/math/BigInteger;";
const BOOLEAN: &str = "java/lang/Boolean";

pub(super) fn emit(class: &mut ClassGen, index: usize, function: &ir::Function) -> Result<Method> {
    if let Some(symbol) = function.extern_symbol() {
        return class.native_method(index, function, symbol);
    }
    if let Some((name, objects)) = function.intrinsic() {
        return class.intrinsic_method(index, function, name, objects);
    }
    FunctionGen::new(class, function, index)?.emit()
}

struct FunctionGen<'c, 'f> {
    class: &'c mut ClassGen,
    function: &'f ir::Function,
    index: usize,
    defs: Vec<Def>,
    dominators: Dominators,
    types: Vec<VType>,
    asm: Asm,
    labels: Vec<asm::Label>,
}

impl<'c, 'f> FunctionGen<'c, 'f> {
    fn new(class: &'c mut ClassGen, function: &'f ir::Function, index: usize) -> Result<Self> {
        if function.values.len() > usize::from(u16::MAX) {
            return Err(error(format!(
                "function `{}` exceeds the JVM local-variable limit",
                function.symbol
            )));
        }
        if function
            .params
            .iter()
            .enumerate()
            .any(|(index, param)| param.0 != index)
        {
            return Err(error(format!(
                "function `{}`: parameters must be the first values for JVM argument slots",
                function.symbol
            )));
        }
        let defs = ir::cfg::definitions(function).map_err(error)?;
        let dominators = Dominators::compute(function).map_err(error)?;
        let mut types = Vec::with_capacity(function.values.len());
        for ty in &function.values {
            types.push(class.vtype(ty)?);
        }
        let mut asm = Asm::default();
        let labels = function.blocks.iter().map(|_| asm.label()).collect();
        Ok(Self {
            class,
            function,
            index,
            defs,
            dominators,
            types,
            asm,
            labels,
        })
    }

    fn slot(value: ValueId) -> u16 {
        // Количество значений проверено в `new`: индекс помещается в u16.
        value.0 as u16
    }

    fn load(&mut self, value: ValueId) {
        self.asm.load(self.types[value.0], Self::slot(value));
    }

    fn store(&mut self, value: ValueId) {
        self.asm.store(self.types[value.0], Self::slot(value));
    }

    /// Кадр: значения, видимые в точке (начало блока, конец его кода).
    fn frame(&self, at: UsePoint) -> Frame {
        let locals = self
            .defs
            .iter()
            .enumerate()
            .map(|(index, def)| {
                if self.dominators.visible(*def, at) {
                    self.types[index]
                } else {
                    VType::Top
                }
            })
            .collect();
        Frame {
            locals,
            stack: Vec::new(),
        }
    }

    fn emit(mut self) -> Result<Method> {
        // (метка начала, метка конца, блок) для блоков с целью unwind.
        let mut ranges = Vec::new();
        for index in 0..self.function.blocks.len() {
            let block = BlockId(index);
            let label = self.labels[index];
            if index == 0 {
                self.asm.place(label, 0);
            } else {
                let frame = self.frame(UsePoint::Unwind(block));
                self.asm.place_with_frame(label, frame);
            }
            let ir_block = &self.function.blocks[index];
            for inst in &ir_block.insts {
                self.inst(&inst.kind, inst.result)?;
                if let Some(result) = inst.result {
                    self.store(result);
                }
            }
            self.terminator(block, &ir_block.term.kind)?;
            if ir_block.unwind.is_some() {
                let end = self.asm.label();
                self.asm.place(end, 0);
                ranges.push((label, end, block));
            }
        }
        // Заглушка unwind: отказ со стека — в последний параметр цели,
        // аргументы — в остальные параметры, затем переход к цели.
        let failure = self.class.failure_type()?;
        let VType::Object(failure_class) = failure else {
            unreachable!("failure is a reference type")
        };
        for (start, end, block) in ranges {
            let target = self.function.blocks[block.0]
                .unwind
                .clone()
                .expect("recorded for blocks with unwind");
            let stub = self.asm.label();
            let mut frame = self.frame(UsePoint::Unwind(block));
            frame.stack.push(failure);
            self.asm.place_with_frame(stub, frame);
            self.asm.exception(start, end, stub, failure_class);
            let params = self.function.blocks[target.block.0].params.clone();
            let (failure_param, others) =
                params.split_last().expect("unwind targets take a failure");
            self.store(*failure_param);
            for arg in &target.args {
                self.load(*arg);
            }
            for param in others.iter().rev() {
                self.store(*param);
            }
            let label = self.labels[target.block.0];
            self.asm.jump(0xa7, label, 0); // goto
        }
        // Тело async-функции — свой метод; метод `name` даёт холодный Future.
        let (name, descriptor) = match &self.class.layout.bodies[self.index] {
            Some(body) => body.clone(),
            None => self.class.layout.signatures[self.index].clone(),
        };
        let max_locals = self.function.values.len() as u16;
        let FunctionGen { class, asm, .. } = self;
        class.function_method(asm, &name, &descriptor, max_locals)
    }

    fn inst(&mut self, kind: &InstKind, result: Option<ValueId>) -> Result<()> {
        match kind {
            InstKind::Const(constant) => self.constant(constant)?,
            InstKind::IntBinary { op, left, right } => {
                self.load(*left);
                self.load(*right);
                let method = match op {
                    IntBinaryOp::Add => Some("add"),
                    IntBinaryOp::Sub => Some("subtract"),
                    IntBinaryOp::Mul => Some("multiply"),
                    _ => None,
                };
                match method {
                    Some(method) => self.class.invoke_virtual(
                        &mut self.asm,
                        BIG,
                        method,
                        "(Ljava/math/BigInteger;)Ljava/math/BigInteger;",
                        2,
                        1,
                    )?,
                    None => {
                        let name = match op {
                            IntBinaryOp::Mod => "mod",
                            IntBinaryOp::And => "and",
                            IntBinaryOp::Or => "or",
                            IntBinaryOp::Xor => "xor",
                            IntBinaryOp::Shl => "shl",
                            IntBinaryOp::Shr => "shr",
                            _ => "ushr",
                        };
                        self.class.call_rt(&mut self.asm, name, BIG_BINARY, 2, 1)?;
                    }
                }
            }
            InstKind::IntUnary { op, operand } => {
                self.load(*operand);
                match op {
                    IntUnaryOp::Neg => self.class.invoke_virtual(
                        &mut self.asm,
                        BIG,
                        "negate",
                        "()Ljava/math/BigInteger;",
                        1,
                        1,
                    )?,
                    IntUnaryOp::Not => self.class.call_rt(
                        &mut self.asm,
                        "not",
                        "(Ljava/math/BigInteger;)Ljava/math/BigInteger;",
                        1,
                        1,
                    )?,
                }
            }
            InstKind::IntCompare { op, left, right } => {
                self.load(*left);
                self.load(*right);
                self.class.invoke_virtual(
                    &mut self.asm,
                    BIG,
                    "compareTo",
                    "(Ljava/math/BigInteger;)I",
                    2,
                    1,
                )?;
                self.compare_result(*op);
            }
            // byte и char — малые неотрицательные int: разность не переполняется.
            InstKind::ByteCompare { op, left, right }
            | InstKind::CharCompare { op, left, right } => {
                self.load(*left);
                self.load(*right);
                self.asm.op(&[0x64], 2, 1); // isub
                self.class.invoke_static(
                    &mut self.asm,
                    "java/lang/Integer",
                    "signum",
                    "(I)I",
                    1,
                    1,
                )?;
                self.compare_result(*op);
            }
            InstKind::ByteBinary { op, left, right } => {
                self.load(*left);
                self.load(*right);
                match op {
                    IntBinaryOp::And => self.asm.op(&[0x7e], 2, 1), // iand
                    IntBinaryOp::Or => self.asm.op(&[0x80], 2, 1),  // ior
                    IntBinaryOp::Xor => self.asm.op(&[0x82], 2, 1), // ixor
                    shift => {
                        let name = match shift {
                            IntBinaryOp::Shl => "byteShl",
                            IntBinaryOp::Shr => "byteShr",
                            _ => "byteUshr",
                        };
                        self.class.call_rt(
                            &mut self.asm,
                            name,
                            "(ILjava/math/BigInteger;)I",
                            2,
                            1,
                        )?;
                    }
                }
            }
            InstKind::ByteFrom { value, context } => {
                self.load(*value);
                match context {
                    Some(context) => {
                        let text = self.class.pool.string(context)?;
                        self.asm.ldc(text);
                    }
                    None => self.asm.op(&[0x01], 0, 1), // aconst_null
                }
                if self.function.values[value.0] == Type::Float {
                    self.class.invoke_static(
                        &mut self.asm,
                        runtime::FLOATS,
                        "toByte",
                        "(Ljava/lang/Double;Ljava/lang/String;)I",
                        2,
                        1,
                    )?;
                } else if self.function.values[value.0] == Type::Rat {
                    self.class.invoke_static(
                        &mut self.asm,
                        runtime::RAT,
                        "toByte",
                        "(Lokkama/rt/Rat;Ljava/lang/String;)I",
                        2,
                        1,
                    )?;
                } else {
                    self.class.call_rt(
                        &mut self.asm,
                        "toByte",
                        "(Ljava/math/BigInteger;Ljava/lang/String;)I",
                        2,
                        1,
                    )?;
                }
            }
            InstKind::Float { op, args } => {
                for arg in args {
                    self.load(*arg);
                }
                let name = &op.mnemonic()["float.".len()..];
                let descriptor = if args.len() == 1 {
                    "(Ljava/lang/Double;)Ljava/lang/Double;"
                } else {
                    "(Ljava/lang/Double;Ljava/lang/Double;)Ljava/lang/Double;"
                };
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::FLOATS,
                    name,
                    descriptor,
                    args.len() as i32,
                    1,
                )?;
            }
            InstKind::FloatCompare { op, left, right } => {
                self.load(*left);
                self.load(*right);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::FLOATS,
                    "compare",
                    "(Ljava/lang/Double;Ljava/lang/Double;)I",
                    2,
                    1,
                )?;
                self.compare_result(*op);
            }
            InstKind::Rat { op, args } => {
                for arg in args {
                    self.load(*arg);
                }
                let name = match op {
                    crate::ir::RatOp::New => "make",
                    other => &other.mnemonic()["rat.".len()..],
                };
                let descriptor = match (op, args.len()) {
                    (crate::ir::RatOp::New, _) => {
                        "(Ljava/math/BigInteger;Ljava/math/BigInteger;)Lokkama/rt/Rat;"
                    }
                    (_, 1) => "(Lokkama/rt/Rat;)Lokkama/rt/Rat;",
                    _ => "(Lokkama/rt/Rat;Lokkama/rt/Rat;)Lokkama/rt/Rat;",
                };
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::RAT,
                    name,
                    descriptor,
                    args.len() as i32,
                    1,
                )?;
            }
            InstKind::RatCompare { op, left, right } => {
                self.load(*left);
                self.load(*right);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::RAT,
                    "compare",
                    "(Lokkama/rt/Rat;Lokkama/rt/Rat;)I",
                    2,
                    1,
                )?;
                self.compare_result(*op);
            }
            InstKind::RatSign(value) => {
                self.load(*value);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::RAT,
                    "sign",
                    "(Lokkama/rt/Rat;)Ljava/math/BigInteger;",
                    1,
                    1,
                )?;
            }
            InstKind::RangeNew {
                start,
                end,
                inclusive,
            } => {
                self.load(*start);
                self.load(*end);
                self.asm.op(&[if *inclusive { 0x04 } else { 0x03 }], 0, 1); // iconst_1/0
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::RANGE,
                    "make",
                    "(Ljava/math/BigInteger;Ljava/math/BigInteger;Z)Lokkama/rt/Range;",
                    3,
                    1,
                )?;
            }
            InstKind::RangePart { range, end } => {
                self.load(*range);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::RANGE,
                    if *end { "end" } else { "start" },
                    "(Lokkama/rt/Range;)Ljava/math/BigInteger;",
                    1,
                    1,
                )?;
            }
            InstKind::RatPart { value, denominator } => {
                self.load(*value);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::RAT,
                    if *denominator {
                        "denominator"
                    } else {
                        "numerator"
                    },
                    "(Lokkama/rt/Rat;)Ljava/lang/String;",
                    1,
                    1,
                )?;
            }
            InstKind::FloatSign(value) => {
                self.load(*value);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::FLOATS,
                    "sign",
                    "(Ljava/lang/Double;)Ljava/math/BigInteger;",
                    1,
                    1,
                )?;
            }
            InstKind::Undef => {
                let result = result.expect("verified: undef defines a value");
                match self.types[result.0] {
                    VType::Integer => self.asm.op(&[0x03], 0, 1), // iconst_0
                    _ => self.asm.op(&[0x01], 0, 1),              // aconst_null
                }
            }
            InstKind::RegionEnter => {
                let class = self.class.pool.class(runtime::REGION)?;
                self.asm.op_index(0xbb, class, 0, 1); // new
                self.asm.op(&[0x59], 0, 1); // dup
                let init = self
                    .class
                    .pool
                    .method_ref(runtime::REGION, "<init>", "()V")?;
                self.asm.op_index(0xb7, init, 1, 0); // invokespecial
            }
            InstKind::RegionAdopt { region, value } | InstKind::RegionTrack { region, value } => {
                self.load(*region);
                self.load(*value);
                self.box_value(*value)?;
                let name = if matches!(kind, InstKind::RegionAdopt { .. }) {
                    "adopt"
                } else {
                    "track"
                };
                self.class.invoke_virtual(
                    &mut self.asm,
                    runtime::REGION,
                    name,
                    "(Ljava/lang/Object;)V",
                    2,
                    0,
                )?;
            }
            InstKind::RegionTransfer { from, to, value } => {
                self.load(*from);
                self.load(*to);
                self.load(*value);
                self.box_value(*value)?;
                self.class.invoke_virtual(
                    &mut self.asm,
                    runtime::REGION,
                    "transfer",
                    "(Lokkama/rt/Region;Ljava/lang/Object;)V",
                    3,
                    0,
                )?;
            }
            InstKind::RegionLeave { region, keep } => {
                self.load(*region);
                match keep {
                    Some(keep) => {
                        self.load(*keep);
                        self.box_value(*keep)?;
                    }
                    None => self.asm.op(&[0x01], 0, 1), // aconst_null
                }
                self.class.invoke_virtual(
                    &mut self.asm,
                    runtime::REGION,
                    "leave",
                    "(Ljava/lang/Object;)V",
                    2,
                    0,
                )?;
            }
            InstKind::RegionUnwind { region, failure } => {
                self.load(*region);
                self.load(*failure);
                self.class.invoke_virtual(
                    &mut self.asm,
                    runtime::REGION,
                    "unwind",
                    "(Lokkama/rt/Failure;)Lokkama/rt/Failure;",
                    2,
                    1,
                )?;
            }
            InstKind::FailSecondary { primary, cleanup } => {
                self.load(*primary);
                self.load(*cleanup);
                self.class.call_rt(
                    &mut self.asm,
                    "secondary",
                    "(Lokkama/rt/Failure;Lokkama/rt/Failure;)Lokkama/rt/Failure;",
                    2,
                    1,
                )?;
            }
            InstKind::EnumCase { ty, case } => {
                let decl = &self.class.layout.enums[ty.0];
                let (class, name) = (decl.class.clone(), decl.cases[*case].clone());
                let descriptor = format!("L{};", runtime::CASE);
                let field = self.class.pool.field_ref(&class, &name, &descriptor)?;
                self.asm.op_index(0xb2, field, 0, 1); // getstatic
            }
            // Case одного enum различаются номером: разность номеров и signum.
            InstKind::EnumCompare { op, left, right } => {
                let ordinal = self.class.pool.field_ref(runtime::CASE, "ordinal", "I")?;
                self.load(*left);
                self.asm.op_index(0xb4, ordinal, 1, 1); // getfield
                self.load(*right);
                self.asm.op_index(0xb4, ordinal, 1, 1); // getfield
                self.asm.op(&[0x64], 2, 1); // isub
                self.class.invoke_static(
                    &mut self.asm,
                    "java/lang/Integer",
                    "signum",
                    "(I)I",
                    1,
                    1,
                )?;
                self.compare_result(*op);
            }
            InstKind::ByteNot(value) => {
                self.load(*value);
                self.asm.op(&[0x11, 0x00, 0xff], 0, 1); // sipush 255
                self.asm.op(&[0x82], 2, 1); // ixor
            }
            InstKind::Bits {
                op,
                byte,
                value,
                count,
            } => {
                self.asm.op(&[0x10, *op as u8], 0, 1); // bipush: код BitOp
                self.load(*value);
                match count {
                    Some(count) => self.load(*count),
                    None => self.asm.op(&[0x01], 0, 1), // aconst_null
                }
                if *byte {
                    self.class.call_rt(
                        &mut self.asm,
                        "byteBits",
                        "(IILjava/math/BigInteger;)I",
                        3,
                        1,
                    )?;
                    if op.counts() {
                        self.int_to_big()?;
                    }
                } else {
                    self.class.call_rt(
                        &mut self.asm,
                        "intBits",
                        "(ILjava/math/BigInteger;Ljava/math/BigInteger;)Ljava/math/BigInteger;",
                        3,
                        1,
                    )?;
                }
            }
            InstKind::BoolNot(operand) => {
                self.load(*operand);
                self.flip();
            }
            InstKind::BoolCompare { op, left, right } => {
                self.load(*left);
                self.load(*right);
                self.asm.op(&[0x82], 2, 1); // ixor
                if *op == CompareOp::Eq {
                    self.flip();
                }
            }
            InstKind::StrConcat { left, right } => {
                self.load(*left);
                self.load(*right);
                self.class.invoke_virtual(
                    &mut self.asm,
                    STRING,
                    "concat",
                    "(Ljava/lang/String;)Ljava/lang/String;",
                    2,
                    1,
                )?;
            }
            InstKind::StrCompare { op, left, right }
                if !matches!(op, CompareOp::Eq | CompareOp::Ne) =>
            {
                self.load(*left);
                self.load(*right);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::KEY_ORDER,
                    "compareCodePoints",
                    "(Ljava/lang/String;Ljava/lang/String;)I",
                    2,
                    1,
                )?;
                self.compare_result(*op);
            }
            InstKind::StrCompare { op, left, right } => {
                self.load(*left);
                self.load(*right);
                self.class.invoke_virtual(
                    &mut self.asm,
                    STRING,
                    "equals",
                    "(Ljava/lang/Object;)Z",
                    2,
                    1,
                )?;
                if *op == CompareOp::Ne {
                    self.flip();
                }
            }
            InstKind::ObjNew { object, fields } => {
                self.construct(object.0, |generator, index| {
                    generator.load(fields[index]);
                    Ok(())
                })?;
            }
            InstKind::ObjNext { object, updates } => {
                let owner = object_of(&self.function.values[object.0]);
                self.construct(owner, |generator, index| {
                    match updates.iter().find(|(field, _)| *field == index) {
                        Some((_, value)) => generator.load(*value),
                        None => generator.get_field(*object, owner, index)?,
                    }
                    Ok(())
                })?;
            }
            InstKind::ObjGet { object, field } => {
                let owner = object_of(&self.function.values[object.0]);
                self.get_field(*object, owner, *field)?;
            }
            InstKind::ObjInit {
                object,
                field,
                value,
            } => {
                let owner = object_of(&self.function.values[object.0]);
                self.load(*object);
                self.load(*value);
                let layout = &self.class.layout.objects[owner];
                let (class, (name, descriptor)) =
                    (layout.class.clone(), layout.fields[*field].clone());
                let index = self.class.pool.field_ref(&class, &name, &descriptor)?;
                self.asm.op_index(0xb5, index, 2, 0); // putfield
            }
            InstKind::Call { func, args } => {
                for arg in args {
                    self.load(*arg);
                }
                let (name, descriptor) = self.class.layout.signatures[func.0].clone();
                let owner = self.class.layout.main.clone();
                let index = self.class.pool.method_ref(&owner, &name, &descriptor)?;
                let pushes = i32::from(!descriptor.ends_with(")V"));
                self.asm.op_index(0xb8, index, args.len() as i32, pushes); // invokestatic
            }
            InstKind::CallRole {
                receiver,
                slot,
                args,
            } => {
                let Type::Role(role) = self.function.values[receiver.0] else {
                    unreachable!("verified: call.role on a role reference")
                };
                self.load(*receiver);
                for arg in args {
                    self.load(*arg);
                }
                let layout = &self.class.layout.roles[role.0];
                let (class, (name, descriptor)) =
                    (layout.class.clone(), layout.methods[*slot].clone());
                let index = self
                    .class
                    .pool
                    .interface_method_ref(&class, &name, &descriptor)?;
                let count = u8::try_from(args.len() + 1)
                    .map_err(|_| error("role action has too many arguments for the JVM"))?;
                let pushes = i32::from(!descriptor.ends_with(")V"));
                self.asm
                    .invoke_interface(index, count, args.len() as i32 + 1, pushes);
            }
            // Ссылка роли на JVM — тот же объект, типизированный интерфейсом.
            InstKind::SuccTake(value) | InstKind::RoleWrap(value) => self.load(*value),
            // Значение cap — его владелец; поле — функция над владельцем.
            InstKind::CapMake(owner) => self.load(*owner),
            InstKind::CallCap { cap, field } => {
                let Type::Cap(id) = self.function.values[cap.0] else {
                    unreachable!("verified: call.cap on a cap")
                };
                let func = self.class.layout.cap_fields[id.0][*field];
                self.load(*cap);
                let (name, descriptor) = self.class.layout.signatures[func.0].clone();
                let owner = self.class.layout.main.clone();
                let index = self.class.pool.method_ref(&owner, &name, &descriptor)?;
                self.asm.op_index(0xb8, index, 1, 1); // invokestatic
            }
            InstKind::Await(future) => {
                self.load(*future);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::SCHEDULER,
                    "await",
                    "(Lokkama/rt/Future;)Ljava/lang/Object;",
                    1,
                    1,
                )?;
                match result {
                    Some(result) => {
                        let ty = self.function.values[result.0].clone();
                        self.unbox(&ty)?;
                    }
                    None => self.asm.op(&[0x57], 1, 0), // pop
                }
            }
            InstKind::FutSpawn(future) => {
                self.load(*future);
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::FUTURE,
                    "spawn",
                    "(Lokkama/rt/Future;)Lokkama/rt/Future;",
                    1,
                    1,
                )?;
            }
            InstKind::Fut { op, future } => {
                self.load(*future);
                let (name, descriptor, pushes) = match op {
                    ir::FutOp::Cancel => ("cancel", "(Lokkama/rt/Future;)V", 0),
                    ir::FutOp::Done => ("done", "(Lokkama/rt/Future;)Z", 1),
                    ir::FutOp::Cancelled => ("cancelled", "(Lokkama/rt/Future;)Z", 1),
                };
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::FUTURE,
                    name,
                    descriptor,
                    1,
                    pushes,
                )?;
            }
            InstKind::FnClosure { func, captures } => {
                let fn_layout = self.class.layout.fn_class(*func, false);
                let (class_name, descriptor) = (
                    fn_layout.class.clone(),
                    self.class.layout.fn_ctor_descriptor(fn_layout),
                );
                let class = self.class.pool.class(&class_name)?;
                self.asm.op_index(0xbb, class, 0, 1); // new
                self.asm.op(&[0x59], 0, 1); // dup
                for capture in captures {
                    self.load(*capture);
                }
                let init = self
                    .class
                    .pool
                    .method_ref(&class_name, "<init>", &descriptor)?;
                self.asm.op_index(0xb7, init, captures.len() as i32 + 1, 0); // invokespecial
            }
            InstKind::WorkerSpawn {
                entry,
                args,
                direct,
            } => {
                self.load(*entry);
                self.boxed_array(args)?;
                self.asm.op(&[if *direct { 0x04 } else { 0x03 }], 0, 1); // iconst_1/0
                self.class.invoke_static(
                    &mut self.asm,
                    "okkama/rt/Worker",
                    "spawn",
                    "(Lokkama/rt/Fn;[Ljava/lang/Object;Z)Lokkama/rt/Future;",
                    3,
                    1,
                )?;
            }
            InstKind::WorkerBind {
                action,
                entry,
                fields,
            } => {
                self.load(*action);
                self.load(*entry);
                self.boxed_array(fields)?;
                self.class.invoke_static(
                    &mut self.asm,
                    "okkama/rt/Worker",
                    "bind",
                    "(Lokkama/rt/Fn;Lokkama/rt/Fn;[Ljava/lang/Object;)Lokkama/rt/Fn;",
                    3,
                    1,
                )?;
            }
            InstKind::CallFn { callee, args } => {
                self.load(*callee);
                self.boxed_array(args)?;
                self.class.invoke_interface(
                    &mut self.asm,
                    runtime::FN,
                    "call",
                    "([Ljava/lang/Object;)Ljava/lang/Object;",
                    2,
                    1,
                )?;
                match result {
                    Some(result) => {
                        let ty = self.function.values[result.0].clone();
                        self.unbox(&ty)?;
                    }
                    None => self.asm.op(&[0x57], 1, 0), // pop: void-результат null
                }
            }
            InstKind::AnyBox(value) => {
                let class = self.class.pool.class(runtime::ANY)?;
                self.asm.op_index(0xbb, class, 0, 1); // new
                self.asm.op(&[0x59], 0, 1); // dup
                self.load(*value);
                self.box_value(*value)?;
                let init =
                    self.class
                        .pool
                        .method_ref(runtime::ANY, "<init>", "(Ljava/lang/Object;)V")?;
                self.asm.op_index(0xb7, init, 2, 0); // invokespecial
            }
            InstKind::RoleNarrow { value, message } => {
                self.load(*value);
                let result = result.expect("verified: role.narrow defines a value");
                let class = self.class.class_of(&self.function.values[result.0].clone());
                let index = self.class.pool.class(&class)?;
                self.asm.ldc(index); // CONSTANT_Class
                let text = self.class.pool.string(message)?;
                self.asm.ldc(text);
                self.class.call_rt(
                    &mut self.asm,
                    "narrow",
                    "(Ljava/lang/Object;Ljava/lang/Class;Ljava/lang/String;)Ljava/lang/Object;",
                    3,
                    1,
                )?;
                self.asm.op_index(0xc0, index, 1, 1); // checkcast
            }
            InstKind::FailCaused { new, handled } => {
                self.load(*new);
                self.load(*handled);
                self.class.call_rt(
                    &mut self.asm,
                    "caused",
                    "(Lokkama/rt/Failure;Lokkama/rt/Failure;)Lokkama/rt/Failure;",
                    2,
                    1,
                )?;
            }
            InstKind::Cvt(value) => {
                self.load(*value);
                let target =
                    &self.function.values[result.expect("verified: cvt defines a value").0];
                let floats = |name: &'static str, descriptor: &'static str| (name, descriptor);
                let float_call = match (&self.function.values[value.0], target) {
                    (Type::Int, Type::Float) => Some(floats(
                        "fromInt",
                        "(Ljava/math/BigInteger;)Ljava/lang/Double;",
                    )),
                    (Type::Byte, Type::Float) => Some(floats("fromByte", "(I)Ljava/lang/Double;")),
                    (Type::Float, Type::Int) => Some(floats(
                        "toInt",
                        "(Ljava/lang/Double;)Ljava/math/BigInteger;",
                    )),
                    (Type::Float, Type::Str) => {
                        Some(floats("text", "(Ljava/lang/Double;)Ljava/lang/String;"))
                    }
                    _ => None,
                };
                let rat_call = match (&self.function.values[value.0], target) {
                    (Type::Int, Type::Rat) => {
                        Some(("fromInt", "(Ljava/math/BigInteger;)Lokkama/rt/Rat;"))
                    }
                    (Type::Byte, Type::Rat) => Some(("fromByte", "(I)Lokkama/rt/Rat;")),
                    (Type::Float, Type::Rat) => {
                        Some(("fromFloat", "(Ljava/lang/Double;)Lokkama/rt/Rat;"))
                    }
                    (Type::Rat, Type::Int) => {
                        Some(("toInt", "(Lokkama/rt/Rat;)Ljava/math/BigInteger;"))
                    }
                    (Type::Rat, Type::Float) => {
                        Some(("toFloat", "(Lokkama/rt/Rat;)Ljava/lang/Double;"))
                    }
                    (Type::Rat, Type::Str) => Some(("text", "(Lokkama/rt/Rat;)Ljava/lang/String;")),
                    _ => None,
                };
                if let Some((name, descriptor)) = rat_call {
                    self.class.invoke_static(
                        &mut self.asm,
                        runtime::RAT,
                        name,
                        descriptor,
                        1,
                        1,
                    )?;
                    return Ok(());
                }
                if let Some((name, descriptor)) = float_call {
                    self.class.invoke_static(
                        &mut self.asm,
                        runtime::FLOATS,
                        name,
                        descriptor,
                        1,
                        1,
                    )?;
                    return Ok(());
                }
                match (&self.function.values[value.0], target) {
                    (Type::Byte, Type::Int) => self.int_to_big()?,
                    (Type::Int, _) => self.class.invoke_virtual(
                        &mut self.asm,
                        BIG,
                        "toString",
                        "()Ljava/lang/String;",
                        1,
                        1,
                    )?,
                    // bool.string(): "1" или "0", как в Perl-интерпретаторе.
                    _ => self.class.invoke_static(
                        &mut self.asm,
                        STRING,
                        "valueOf",
                        "(I)Ljava/lang/String;",
                        1,
                        1,
                    )?,
                }
            }
            InstKind::ArrayNew(items) => {
                self.new_list()?;
                for item in items {
                    self.asm.op(&[0x59], 0, 1); // dup
                    self.load(*item);
                    self.box_value(*item)?;
                    self.list_call("add", "(Ljava/lang/Object;)Z", 1)?;
                    self.asm.op(&[0x57], 1, 0); // pop
                }
            }
            InstKind::ArrayLen(array) => {
                self.load(*array);
                self.class.call_rt(
                    &mut self.asm,
                    "size",
                    "(Ljava/util/List;)Ljava/math/BigInteger;",
                    1,
                    1,
                )?;
            }
            InstKind::ArrayGet { array, index } => {
                self.load(*array);
                self.load(*index);
                self.class.call_rt(
                    &mut self.asm,
                    "index",
                    "(Ljava/util/List;Ljava/math/BigInteger;)Ljava/lang/Object;",
                    2,
                    1,
                )?;
                let result = result.expect("verified: array.get defines a value");
                self.unbox(&self.function.values[result.0].clone())?;
            }
            InstKind::ArraySlice { array, start, end } => {
                self.load(*array);
                self.load(*start);
                self.load(*end);
                self.class.call_rt(
                    &mut self.asm,
                    "slice",
                    "(Ljava/util/List;Ljava/math/BigInteger;Ljava/math/BigInteger;)Ljava/util/List;",
                    3,
                    1,
                )?;
            }
            InstKind::ArrayJoin { array, separator } => {
                self.load(*array);
                self.load(*separator);
                self.class.call_rt(
                    &mut self.asm,
                    "joinArray",
                    "(Ljava/util/List;Ljava/lang/String;)Ljava/lang/String;",
                    2,
                    1,
                )?;
            }
            // Сборщик линеен (§12.4): ArrayList изменяется на месте, а
            // `builder.finish` и `array.retag` — та же ссылка.
            InstKind::Retag(value) | InstKind::BuilderFinish(value) => self.load(*value),
            InstKind::BuilderNew => self.new_list()?,
            InstKind::BuilderPush { builder, value } => {
                self.load(*builder);
                self.asm.op(&[0x59], 0, 1); // dup
                self.load(*value);
                self.box_value(*value)?;
                self.list_call("add", "(Ljava/lang/Object;)Z", 1)?;
                self.asm.op(&[0x57], 1, 0); // pop
            }
            InstKind::BuilderAppend { builder, array } => {
                self.load(*builder);
                self.asm.op(&[0x59], 0, 1); // dup
                self.load(*array);
                self.list_call("addAll", "(Ljava/util/Collection;)Z", 1)?;
                self.asm.op(&[0x57], 1, 0); // pop
            }
            InstKind::BuilderAppendRange {
                builder,
                start,
                end,
                inclusive,
            } => {
                self.load(*builder);
                self.load(*start);
                self.load(*end);
                self.asm.op(&[if *inclusive { 0x04 } else { 0x03 }], 0, 1);
                self.class.call_rt(
                    &mut self.asm,
                    "appendRange",
                    "(Ljava/util/List;Ljava/math/BigInteger;Ljava/math/BigInteger;Z)Ljava/util/List;",
                    4,
                    1,
                )?;
            }
            InstKind::Str { op, args } => {
                for arg in args {
                    self.load(*arg);
                }
                let (name, descriptor) = match op {
                    StrOp::Len => ("length", "(Ljava/lang/String;)Ljava/math/BigInteger;"),
                    StrOp::Slice => (
                        "slice",
                        "(Ljava/lang/String;Ljava/math/BigInteger;Ljava/math/BigInteger;)Ljava/lang/String;",
                    ),
                    StrOp::Split => (
                        "split",
                        "(Ljava/lang/String;Ljava/lang/String;)Ljava/util/List;",
                    ),
                    StrOp::Replace => (
                        "replace",
                        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    ),
                    StrOp::ReplaceFirst => (
                        "replaceFirst",
                        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    ),
                    StrOp::Trim => ("trim", "(Ljava/lang/String;)Ljava/lang/String;"),
                    StrOp::Chomp => ("chomp", "(Ljava/lang/String;)Ljava/lang/String;"),
                    StrOp::Utf8 => ("utf8", "(Ljava/lang/String;)Ljava/util/List;"),
                };
                self.class
                    .call_rt(&mut self.asm, name, descriptor, args.len() as i32, 1)?;
            }
            // Объекты сравнивают поля через свой `equals`, списки — поэлементно.
            InstKind::ValCompare { op, left, right } => {
                self.load(*left);
                self.load(*right);
                self.class.invoke_static(
                    &mut self.asm,
                    "java/util/Objects",
                    "equals",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Z",
                    2,
                    1,
                )?;
                if *op == CompareOp::Ne {
                    self.flip();
                }
            }
            InstKind::IntAbs(value) => {
                self.load(*value);
                self.class.invoke_virtual(
                    &mut self.asm,
                    BIG,
                    "abs",
                    "()Ljava/math/BigInteger;",
                    1,
                    1,
                )?;
            }
            InstKind::IntSign(value) => {
                self.load(*value);
                self.class.call_rt(
                    &mut self.asm,
                    "sign",
                    "(Ljava/math/BigInteger;)Ljava/math/BigInteger;",
                    1,
                    1,
                )?;
            }
            InstKind::MapNew(entries) => {
                self.new_map()?;
                for (key, value) in entries {
                    self.asm.op(&[0x59], 0, 1); // dup
                    self.put(*key, *value)?;
                }
            }
            InstKind::MapBuilderNew => self.new_map()?,
            InstKind::MapBuilderPut {
                builder,
                key,
                value,
            } => {
                self.load(*builder);
                self.asm.op(&[0x59], 0, 1); // dup
                self.put(*key, *value)?;
            }
            // Сборщик словаря линеен, как и сборщик массива: та же ссылка.
            InstKind::MapRetag(map) | InstKind::MapBuilderFinish(map) => self.load(*map),
            InstKind::MapLen(map) => {
                self.load(*map);
                self.class.call_rt(
                    &mut self.asm,
                    "size",
                    "(Ljava/util/Map;)Ljava/math/BigInteger;",
                    1,
                    1,
                )?;
            }
            InstKind::MapHas { map, key } => {
                self.load(*map);
                self.load(*key);
                self.box_value(*key)?;
                self.map_call("containsKey", "(Ljava/lang/Object;)Z", 1)?;
            }
            InstKind::MapGet { map, key } => {
                self.load(*map);
                self.load(*key);
                self.box_value(*key)?;
                self.class.call_rt(
                    &mut self.asm,
                    "get",
                    "(Ljava/util/Map;Ljava/lang/Object;)Ljava/lang/Object;",
                    2,
                    1,
                )?;
                let result = result.expect("verified: map.get defines a value");
                self.unbox(&self.function.values[result.0].clone())?;
            }
            InstKind::MapKeys(map) | InstKind::MapValues(map) => {
                self.load(*map);
                let name = if matches!(kind, InstKind::MapKeys(_)) {
                    "keys"
                } else {
                    "values"
                };
                self.class.call_rt(
                    &mut self.asm,
                    name,
                    "(Ljava/util/Map;)Ljava/util/List;",
                    1,
                    1,
                )?;
            }
        }
        Ok(())
    }

    /// `okkama.rt.Rt.newMap()`: пустой `TreeMap` в порядке ключей.
    fn new_map(&mut self) -> Result<()> {
        self.class
            .call_rt(&mut self.asm, "newMap", "()Ljava/util/Map;", 0, 1)
    }

    /// `map.put(key, value)` над словарём на вершине стека; результат `put` снят.
    fn put(&mut self, key: ValueId, value: ValueId) -> Result<()> {
        self.load(key);
        self.box_value(key)?;
        self.load(value);
        self.box_value(value)?;
        self.map_call(
            "put",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            2,
        )?;
        self.asm.op(&[0x57], 1, 0); // pop
        Ok(())
    }

    /// `invokeinterface java/util/Map.<name>` на словаре и `args` аргументах.
    fn map_call(&mut self, name: &str, descriptor: &str, args: u8) -> Result<()> {
        let pushes = i32::from(!descriptor.ends_with(")V"));
        self.class
            .invoke_interface(&mut self.asm, MAP, name, descriptor, args + 1, pushes)
    }

    /// `new java/util/ArrayList; dup; invokespecial <init>()V`.
    fn new_list(&mut self) -> Result<()> {
        let class = self.class.pool.class("java/util/ArrayList")?;
        self.asm.op_index(0xbb, class, 0, 1); // new
        self.asm.op(&[0x59], 0, 1); // dup
        let init = self
            .class
            .pool
            .method_ref("java/util/ArrayList", "<init>", "()V")?;
        self.asm.op_index(0xb7, init, 1, 0); // invokespecial
        Ok(())
    }

    /// `invokeinterface java/util/List.<name>` на списке и `args` аргументах.
    fn list_call(&mut self, name: &str, descriptor: &str, args: u8) -> Result<()> {
        let pushes = i32::from(!descriptor.ends_with(")V"));
        self.class
            .invoke_interface(&mut self.asm, LIST, name, descriptor, args + 1, pushes)
    }

    /// Значение на вершине стека → элемент коллекции (`bool` — `Boolean`).
    /// `Object[]` значений, упакованных как аргументы `Fn.call`.
    fn boxed_array(&mut self, values: &[ValueId]) -> Result<()> {
        self.push_count(values.len())?;
        let object = self.class.pool.class("java/lang/Object")?;
        self.asm.op_index(0xbd, object, 1, 1); // anewarray
        for (index, value) in values.iter().enumerate() {
            self.asm.op(&[0x59], 0, 1); // dup
            self.push_count(index)?;
            self.load(*value);
            self.box_value(*value)?;
            self.asm.op(&[0x53], 3, 0); // aastore
        }
        Ok(())
    }

    fn box_value(&mut self, value: ValueId) -> Result<()> {
        match self.function.values[value.0] {
            Type::Bool => self.class.invoke_static(
                &mut self.asm,
                BOOLEAN,
                "valueOf",
                "(Z)Ljava/lang/Boolean;",
                1,
                1,
            )?,
            Type::Byte => {
                self.asm.op(&[0x91], 1, 1); // i2b
                self.class.invoke_static(
                    &mut self.asm,
                    "java/lang/Byte",
                    "valueOf",
                    "(B)Ljava/lang/Byte;",
                    1,
                    1,
                )?;
            }
            Type::Char => self.class.invoke_static(
                &mut self.asm,
                "java/lang/Integer",
                "valueOf",
                "(I)Ljava/lang/Integer;",
                1,
                1,
            )?,
            _ => {}
        }
        Ok(())
    }

    /// Неотрицательная целая константа JVM (`iconst`/`bipush`/`sipush`).
    fn push_count(&mut self, value: usize) -> Result<()> {
        match value {
            0..=5 => self.asm.op(&[0x03 + value as u8], 0, 1), // iconst_n
            6..=127 => self.asm.op(&[0x10, value as u8], 0, 1), // bipush
            _ => {
                let value = u16::try_from(value).map_err(|_| error("too many fn arguments"))?;
                let [high, low] = value.to_be_bytes();
                self.asm.op(&[0x11, high, low], 0, 1); // sipush
            }
        }
        Ok(())
    }

    /// JVM `int` на вершине стека → `BigInteger`.
    fn int_to_big(&mut self) -> Result<()> {
        self.asm.op(&[0x85], 1, 2); // i2l
        self.class.invoke_static(
            &mut self.asm,
            BIG,
            "valueOf",
            "(J)Ljava/math/BigInteger;",
            2,
            1,
        )
    }

    /// Результат `compareTo`/`signum` (ровно -1, 0 или 1) → bool без ветвлений.
    fn compare_result(&mut self, op: CompareOp) {
        match op {
            // c < 0: c >>> 31
            CompareOp::Lt => self.sign_bit(),
            CompareOp::Ge => {
                self.sign_bit();
                self.flip();
            }
            // c > 0: (-c) >>> 31
            CompareOp::Gt => {
                self.asm.op(&[0x74], 1, 1); // ineg
                self.sign_bit();
            }
            CompareOp::Le => {
                self.asm.op(&[0x74], 1, 1);
                self.sign_bit();
                self.flip();
            }
            // c != 0: c & 1
            CompareOp::Ne => self.low_bit(),
            CompareOp::Eq => {
                self.low_bit();
                self.flip();
            }
        }
    }

    /// Элемент коллекции (`Object` на вершине стека) → значение типа `ty`.
    fn unbox(&mut self, ty: &Type) -> Result<()> {
        let class = self.class.class_of(ty);
        if class != "java/lang/Object" {
            let index = self.class.pool.class(&class)?;
            self.asm.op_index(0xc0, index, 1, 1); // checkcast
        }
        match ty {
            Type::Bool => {
                self.class
                    .invoke_virtual(&mut self.asm, BOOLEAN, "booleanValue", "()Z", 1, 1)?
            }
            Type::Byte => {
                self.class.invoke_virtual(
                    &mut self.asm,
                    "java/lang/Byte",
                    "byteValue",
                    "()B",
                    1,
                    1,
                )?;
                self.asm.op(&[0x11, 0x00, 0xff], 0, 1); // sipush 255
                self.asm.op(&[0x7e], 2, 1); // iand: знаковый byte JVM → 0..255
            }
            Type::Char => self.class.invoke_virtual(
                &mut self.asm,
                "java/lang/Integer",
                "intValue",
                "()I",
                1,
                1,
            )?,
            _ => {}
        }
        Ok(())
    }

    /// `new C; dup; <поля>; invokespecial C.<init>` — поля кладёт `field`.
    fn construct(
        &mut self,
        object: usize,
        mut field: impl FnMut(&mut Self, usize) -> Result<()>,
    ) -> Result<()> {
        let layout = &self.class.layout.objects[object];
        let (layout_class, ctor, count) = (
            layout.class.clone(),
            layout.ctor_descriptor(),
            layout.fields.len(),
        );
        let class = self.class.pool.class(&layout_class)?;
        self.asm.op_index(0xbb, class, 0, 1); // new
        self.asm.op(&[0x59], 0, 1); // dup
        for index in 0..count {
            field(self, index)?;
        }
        let init = self.class.pool.method_ref(&layout_class, "<init>", &ctor)?;
        self.asm.op_index(0xb7, init, count as i32 + 1, 0); // invokespecial
        Ok(())
    }

    fn get_field(&mut self, object: ValueId, owner: usize, field: usize) -> Result<()> {
        self.load(object);
        let layout = &self.class.layout.objects[owner];
        let (class, (name, descriptor)) = (layout.class.clone(), layout.fields[field].clone());
        let index = self.class.pool.field_ref(&class, &name, &descriptor)?;
        self.asm.op_index(0xb4, index, 1, 1); // getfield
        Ok(())
    }

    /// Поле класса отказа у значения `failure` на вершине стека.
    fn failure_field(&mut self, name: &str, descriptor: &str) -> Result<()> {
        let index = self
            .class
            .pool
            .field_ref(runtime::FAILURE, name, descriptor)?;
        self.asm.op_index(0xb4, index, 1, 1); // getfield
        Ok(())
    }

    /// bool на вершине стека: `x ^ 1`.
    fn flip(&mut self) {
        self.asm.op(&[0x04], 0, 1); // iconst_1
        self.asm.op(&[0x82], 2, 1); // ixor
    }

    /// `x >>> 31`: 1 для отрицательного int.
    fn sign_bit(&mut self) {
        self.asm.op(&[0x10, 31], 0, 1); // bipush 31
        self.asm.op(&[0x7c], 2, 1); // iushr
    }

    /// `x & 1`.
    fn low_bit(&mut self) {
        self.asm.op(&[0x04], 0, 1); // iconst_1
        self.asm.op(&[0x7e], 2, 1); // iand
    }

    fn constant(&mut self, constant: &Constant) -> Result<()> {
        match constant {
            Constant::Int(value) => match value.to_i64() {
                Some(small) => {
                    match small {
                        0 => self.asm.op(&[0x09], 0, 2), // lconst_0
                        1 => self.asm.op(&[0x0a], 0, 2), // lconst_1
                        _ => {
                            let index = self.class.pool.long(small)?;
                            self.asm.op_index(0x14, index, 0, 2); // ldc2_w
                        }
                    }
                    self.class.invoke_static(
                        &mut self.asm,
                        BIG,
                        "valueOf",
                        "(J)Ljava/math/BigInteger;",
                        2,
                        1,
                    )?;
                }
                None => {
                    let class = self.class.pool.class(BIG)?;
                    self.asm.op_index(0xbb, class, 0, 1); // new
                    self.asm.op(&[0x59], 0, 1); // dup
                    let digits = self.class.pool.string(&value.to_string())?;
                    self.asm.ldc(digits);
                    let init =
                        self.class
                            .pool
                            .method_ref(BIG, "<init>", "(Ljava/lang/String;)V")?;
                    self.asm.op_index(0xb7, init, 2, 0); // invokespecial
                }
            },
            Constant::Bool(value) => self.asm.op(&[if *value { 0x04 } else { 0x03 }], 0, 1),
            // Биты константы точно: ldc2_w long, `longBitsToDouble`, `Double.valueOf`.
            Constant::Float(bits) => {
                let index = self.class.pool.long(*bits as i64)?;
                self.asm.op_index(0x14, index, 0, 2); // ldc2_w
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::DOUBLE,
                    "longBitsToDouble",
                    "(J)D",
                    2,
                    2,
                )?;
                self.class.invoke_static(
                    &mut self.asm,
                    runtime::DOUBLE,
                    "valueOf",
                    "(D)Ljava/lang/Double;",
                    2,
                    1,
                )?;
            }
            Constant::Byte(value) => {
                let [high, low] = u16::from(*value).to_be_bytes();
                self.asm.op(&[0x11, high, low], 0, 1); // sipush
            }
            Constant::Char(ch) => {
                let code = u32::from(*ch);
                match i16::try_from(code) {
                    Ok(small) => {
                        let [high, low] = small.to_be_bytes();
                        self.asm.op(&[0x11, high, low], 0, 1); // sipush
                    }
                    Err(_) => {
                        let index = self.class.pool.long(i64::from(code))?;
                        self.asm.op_index(0x14, index, 0, 2); // ldc2_w
                        self.asm.op(&[0x88], 2, 1); // l2i
                    }
                }
            }
            Constant::Str(text) if modified_utf8_len(text) > 65535 => {
                // Константа class-файла ограничена 65535 байтами: длинная
                // строка собирается из кусков в StringBuilder.
                let builder = "java/lang/StringBuilder";
                let class = self.class.pool.class(builder)?;
                self.asm.op_index(0xbb, class, 0, 1); // new
                self.asm.op(&[0x59], 0, 1); // dup
                let init = self.class.pool.method_ref(builder, "<init>", "()V")?;
                self.asm.op_index(0xb7, init, 1, 0); // invokespecial
                for chunk in utf8_chunks(text, 60000) {
                    let index = self.class.pool.string(chunk)?;
                    self.asm.ldc(index);
                    self.class.invoke_virtual(
                        &mut self.asm,
                        builder,
                        "append",
                        "(Ljava/lang/String;)Ljava/lang/StringBuilder;",
                        2,
                        1,
                    )?;
                }
                self.class.invoke_virtual(
                    &mut self.asm,
                    builder,
                    "toString",
                    "()Ljava/lang/String;",
                    1,
                    1,
                )?;
            }
            Constant::Str(text) => {
                let index = self.class.pool.string(text)?;
                self.asm.ldc(index);
            }
        }
        Ok(())
    }

    /// Аргументы ребра: все на стек, затем в параметры цели в обратном порядке.
    /// Так параллельное присваивание корректно даже при перестановке значений.
    fn edge(&mut self, target: &Target) {
        for arg in &target.args {
            self.load(*arg);
        }
        let params = self.function.blocks[target.block.0].params.clone();
        for param in params.iter().rev() {
            self.store(*param);
        }
    }

    fn goto(&mut self, target: &Target) {
        self.edge(target);
        let label = self.labels[target.block.0];
        self.asm.jump(0xa7, label, 0); // goto
    }

    fn terminator(&mut self, block: BlockId, kind: &TermKind) -> Result<()> {
        match kind {
            TermKind::Br(target) => self.goto(target),
            TermKind::Cond {
                condition,
                then_target,
                else_target,
            } => {
                self.load(*condition);
                if else_target.args.is_empty() {
                    let label = self.labels[else_target.block.0];
                    self.asm.jump(0x99, label, 1); // ifeq
                    self.goto(then_target);
                } else if then_target.args.is_empty() {
                    let label = self.labels[then_target.block.0];
                    self.asm.jump(0x9a, label, 1); // ifne
                    self.goto(else_target);
                } else {
                    // У обоих рёбер есть аргументы: ветка else копирует в своей заглушке.
                    let stub = self.asm.label();
                    self.asm.jump(0x99, stub, 1); // ifeq
                    self.goto(then_target);
                    let frame = self.frame(UsePoint::Term(block));
                    self.asm.place_with_frame(stub, frame);
                    self.goto(else_target);
                }
            }
            TermKind::Ret(Some(value)) => {
                self.load(*value);
                let opcode = match self.types[value.0] {
                    VType::Integer => 0xac, // ireturn
                    _ => 0xb0,              // areturn
                };
                self.asm.op(&[opcode], 1, 0);
            }
            TermKind::Ret(None) => self.asm.op(&[0xb1], 0, 0),
            TermKind::Panic(message) => {
                self.load(*message);
                self.class.call_rt(
                    &mut self.asm,
                    "panic",
                    "(Ljava/lang/String;)Lokkama/rt/Failure;",
                    1,
                    1,
                )?;
                self.asm.op(&[0xbf], 1, 0); // athrow
            }
            TermKind::Trap { message, .. } => self.class.throw_trap(&mut self.asm, message)?,
            TermKind::Unreachable => self.class.throw_trap(
                &mut self.asm,
                "internal error: unreachable code was executed",
            )?,
            TermKind::Throw(value) => {
                self.load(*value);
                self.class.call_rt(
                    &mut self.asm,
                    "thrown",
                    "(Ljava/lang/Object;)Lokkama/rt/Failure;",
                    1,
                    1,
                )?;
                self.asm.op(&[0xbf], 1, 0); // athrow
            }
            TermKind::Raise(value) => {
                self.load(*value);
                self.asm.op(&[0xbf], 1, 0); // athrow
            }
            TermKind::Catch {
                failure,
                ty,
                handler,
                next,
            } => self.catch(block, *failure, *ty, handler, next)?,
            TermKind::ArrayLookup {
                array,
                index,
                found,
                missing,
            } => self.lookup(block, *array, *index, found, missing)?,
            TermKind::MapLookup {
                map,
                key,
                found,
                missing,
            } => self.map_lookup(block, *map, *key, found, missing)?,
        }
        Ok(())
    }

    /// Совпадение: типизированный отказ и Error нужного класса; иначе `next`.
    fn catch(
        &mut self,
        block: BlockId,
        failure: ValueId,
        ty: CatchType,
        handler: &Target,
        next: &Target,
    ) -> Result<()> {
        let class_name = match ty {
            CatchType::Object(object) => self.class.layout.objects[object.0].class.clone(),
            CatchType::Role(role) => self.class.layout.roles[role.0].class.clone(),
        };
        let class = self.class.pool.class(&class_name)?;
        let miss = self.asm.label();
        self.load(failure);
        self.failure_field("kind", "I")?;
        self.asm.jump(0x9a, miss, 1); // ifne: не THROWN (0) — не типизированный отказ
        self.load(failure);
        self.failure_field("error", "Ljava/lang/Object;")?;
        self.asm.op_index(0xc1, class, 1, 1); // instanceof
        self.asm.jump(0x99, miss, 1); // ifeq
        for arg in &handler.args {
            self.load(*arg);
        }
        self.load(failure);
        self.failure_field("error", "Ljava/lang/Object;")?;
        self.asm.op_index(0xc0, class, 1, 1); // checkcast
        let params = self.function.blocks[handler.block.0].params.clone();
        for param in params.iter().rev() {
            self.store(*param);
        }
        let label = self.labels[handler.block.0];
        self.asm.jump(0xa7, label, 0); // goto
        let frame = self.frame(UsePoint::Term(block));
        self.asm.place_with_frame(miss, frame);
        self.goto(next);
        Ok(())
    }
}

impl FunctionGen<'_, '_> {
    /// `m[k] else …`: ключ есть — значение последним аргументом `found`.
    fn map_lookup(
        &mut self,
        block: BlockId,
        map: ValueId,
        key: ValueId,
        found: &Target,
        missing: &Target,
    ) -> Result<()> {
        let miss = self.asm.label();
        self.load(map);
        self.load(key);
        self.box_value(key)?;
        self.map_call("containsKey", "(Ljava/lang/Object;)Z", 1)?;
        self.asm.jump(0x99, miss, 1); // ifeq
        for arg in &found.args {
            self.load(*arg);
        }
        self.load(map);
        self.load(key);
        self.box_value(key)?;
        self.map_call("get", "(Ljava/lang/Object;)Ljava/lang/Object;", 1)?;
        let params = self.function.blocks[found.block.0].params.clone();
        let (value, _) = params
            .split_last()
            .expect("verified: found takes the value");
        let ty = self.function.values[value.0].clone();
        self.unbox(&ty)?;
        for param in params.iter().rev() {
            self.store(*param);
        }
        let label = self.labels[found.block.0];
        self.asm.jump(0xa7, label, 0); // goto
        let frame = self.frame(UsePoint::Term(block));
        self.asm.place_with_frame(miss, frame);
        self.goto(missing);
        Ok(())
    }

    /// `xs[i] else …`: в границах — элемент последним аргументом `found`.
    fn lookup(
        &mut self,
        block: BlockId,
        array: ValueId,
        index: ValueId,
        found: &Target,
        missing: &Target,
    ) -> Result<()> {
        let miss = self.asm.label();
        self.load(array);
        self.load(index);
        self.class.call_rt(
            &mut self.asm,
            "inBounds",
            "(Ljava/util/List;Ljava/math/BigInteger;)Z",
            2,
            1,
        )?;
        self.asm.jump(0x99, miss, 1); // ifeq
        for arg in &found.args {
            self.load(*arg);
        }
        self.load(array);
        self.load(index);
        self.class
            .invoke_virtual(&mut self.asm, BIG, "intValue", "()I", 1, 1)?;
        self.list_call("get", "(I)Ljava/lang/Object;", 1)?;
        let params = self.function.blocks[found.block.0].params.clone();
        let (element, _) = params
            .split_last()
            .expect("verified: found takes the element");
        let ty = self.function.values[element.0].clone();
        self.unbox(&ty)?;
        for param in params.iter().rev() {
            self.store(*param);
        }
        let label = self.labels[found.block.0];
        self.asm.jump(0xa7, label, 0); // goto
        let frame = self.frame(UsePoint::Term(block));
        self.asm.place_with_frame(miss, frame);
        self.goto(missing);
        Ok(())
    }
}

fn object_of(ty: &Type) -> usize {
    match ty {
        Type::Object(object) | Type::Successor(object) => object.0,
        _ => unreachable!("verified: the operand is an object"),
    }
}

/// Длина строки в modified UTF-8 class-файла.
fn modified_utf8_len(text: &str) -> usize {
    text.encode_utf16()
        .map(|unit| match unit {
            1..=0x7f => 1,
            0 | 0x80..=0x7ff => 2,
            _ => 3,
        })
        .sum()
}

/// Куски строки не длиннее `limit` байт modified UTF-8, по границам символов.
fn utf8_chunks(text: &str, limit: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut size = 0;
    for (offset, ch) in text.char_indices() {
        let width = modified_utf8_len(ch.encode_utf8(&mut [0; 4]));
        if size + width > limit {
            chunks.push(&text[start..offset]);
            start = offset;
            size = 0;
        }
        size += width;
    }
    chunks.push(&text[start..]);
    chunks
}
