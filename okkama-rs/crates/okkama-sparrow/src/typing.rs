//! Типы операндов и результатов инструкций (OIR-1, как верификатор IR
//! `okkama-compiler/src/ir/verify.rs`): тип регистра назначения совместим с
//! результатом инструкции, операнды — с её ожиданием, арность и типы
//! аргументов вызовов — с сигнатурой, перемещения на переходах — с типами
//! параметров. Ядро Sparrow выбирает машинное представление регистра по его
//! типу, поэтому программа с ложными типами отвергается до исполнения.
use crate::{
    CatchKind, Constant, FutOp, Function, Instruction, Program, Register, StrFn, Target, Type, TypeRef,
};

type Rule = Result<(), String>;

pub(crate) struct Typing<'a> {
    program: &'a Program,
    function: &'a Function,
}

impl<'a> Typing<'a> {
    pub(crate) fn new(program: &'a Program, function: &'a Function) -> Self {
        Typing { program, function }
    }

    fn resolve(&self, ty: TypeRef) -> &'a Type {
        // Ссылки таблицы типов проверены раньше (check_types).
        &self.program.types[ty.0 as usize]
    }

    fn ty(&self, register: Register) -> &'a Type {
        self.resolve(self.function.registers[register.0 as usize])
    }

    fn name(&self, ty: &Type) -> String {
        let r = |t: &TypeRef| self.name(self.resolve(*t));
        match ty {
            Type::Int => "int".into(),
            Type::Bool => "bool".into(),
            Type::Byte => "byte".into(),
            Type::Float => "float".into(),
            Type::Rat => "rat".into(),
            Type::Char => "char".into(),
            Type::Str => "str".into(),
            Type::Object(o) => self.program.objects.get(o.0 as usize).map_or("object".into(), |d| d.name.clone()),
            Type::Successor(o) => format!("^{}", self.program.objects.get(o.0 as usize).map_or("object", |d| &d.name)),
            Type::Role(o) => self.program.roles.get(o.0 as usize).map_or("role".into(), |d| d.name.clone()),
            Type::Enum(o) => self.program.enums.get(o.0 as usize).map_or("enum".into(), |d| d.name.clone()),
            Type::Failure => "failure".into(),
            Type::Array(e) => format!("@{}", r(e)),
            Type::Never => "never".into(),
            Type::Builder(e) => format!("builder<{}>", r(e)),
            Type::Map(k, v) => format!("%{}:{}", r(k), r(v)),
            Type::MapBuilder(k, v) => format!("mbuilder<{},{}>", r(k), r(v)),
            Type::Region => "region".into(),
            Type::Handle(h) => self.program.handles.get(*h as usize).cloned().unwrap_or_else(|| "handle".into()),
            Type::Any => "any".into(),
            Type::Range => "range".into(),
            Type::Fn { params, result } => format!(
                "fn({}){}",
                params.iter().map(r).collect::<Vec<_>>().join(", "),
                result.as_ref().map(|t| format!(" is {}", r(t))).unwrap_or_default()
            ),
            Type::Future(result) => format!("future<{}>", result.as_ref().map(r).unwrap_or_else(|| "void".into())),
            Type::Cap(c) => self.program.caps.get(c.0 as usize).map_or("cap".into(), |d| d.name.clone()),
            Type::Variant(v) => self.program.variants.get(v.0 as usize).map_or("variant".into(), |d| d.name.clone()),
        }
    }

    /// Структурное равенство: одинаковые типы могут стоять в таблице дважды.
    fn equal(&self, a: &Type, b: &Type) -> bool {
        let refs = |x: &TypeRef, y: &TypeRef| x == y || self.equal(self.resolve(*x), self.resolve(*y));
        let opt = |x: &Option<TypeRef>, y: &Option<TypeRef>| match (x, y) {
            (None, None) => true,
            (Some(x), Some(y)) => refs(x, y),
            _ => false,
        };
        match (a, b) {
            (Type::Array(x), Type::Array(y)) | (Type::Builder(x), Type::Builder(y)) => refs(x, y),
            (Type::Map(k1, v1), Type::Map(k2, v2)) | (Type::MapBuilder(k1, v1), Type::MapBuilder(k2, v2)) => {
                refs(k1, k2) && refs(v1, v2)
            }
            (Type::Fn { params: p1, result: r1 }, Type::Fn { params: p2, result: r2 }) => {
                p1.len() == p2.len() && p1.iter().zip(p2).all(|(x, y)| refs(x, y)) && opt(r1, r2)
            }
            (Type::Future(x), Type::Future(y)) => opt(x, y),
            _ => a == b,
        }
    }

    fn expect(&self, register: Register, expected: &Type) -> Rule {
        let actual = self.ty(register);
        if self.equal(actual, expected) {
            Ok(())
        } else {
            Err(format!("r{} has type {}, expected {}", register.0, self.name(actual), self.name(expected)))
        }
    }

    fn expect_ref(&self, register: Register, expected: TypeRef) -> Rule {
        self.expect(register, self.resolve(expected))
    }

    /// Регистр назначения может хранить результат `result`.
    fn define(&self, dst: Register, result: &Type, what: &str) -> Rule {
        let actual = self.ty(dst);
        if self.equal(actual, result) {
            Ok(())
        } else {
            Err(format!("r{}: {} cannot hold the {} result of `{what}`", dst.0, self.name(actual), self.name(result)))
        }
    }

    fn define_ref(&self, dst: Register, result: TypeRef, what: &str) -> Rule {
        self.define(dst, self.resolve(result), what)
    }

    fn element(&self, array: Register) -> Result<TypeRef, String> {
        match self.ty(array) {
            Type::Array(e) if *self.resolve(*e) != Type::Never => Ok(*e),
            other => Err(format!("r{} has type {}, expected a non-empty array", array.0, self.name(other))),
        }
    }

    fn entry(&self, map: Register) -> Result<(TypeRef, TypeRef), String> {
        match self.ty(map) {
            Type::Map(k, v) if *self.resolve(*k) != Type::Never => Ok((*k, *v)),
            other => Err(format!("r{} has type {}, expected a non-empty map", map.0, self.name(other))),
        }
    }

    fn variant(&self, register: Register) -> Result<crate::VariantRef, String> {
        match self.ty(register) {
            Type::Variant(v) => Ok(*v),
            other => Err(format!("r{} has type {}, expected a variant", register.0, self.name(other))),
        }
    }

    /// Объект раскладки случая `case` варианта.
    fn case_object(&self, variant: crate::VariantRef, case: u32) -> Result<crate::ObjectRef, String> {
        self.program
            .variants
            .get(variant.0 as usize)
            .and_then(|declared| declared.cases.get(case as usize))
            .map(|(_, object)| *object)
            .ok_or_else(|| format!("variant variant{} has no case {case}", variant.0))
    }

    fn object(&self, register: Register) -> Result<crate::ObjectRef, String> {
        match self.ty(register) {
            Type::Object(o) => Ok(*o),
            other => Err(format!("r{} has type {}, expected an object", register.0, self.name(other))),
        }
    }

    fn field(&self, object: crate::ObjectRef, field: u32) -> Result<TypeRef, String> {
        self.program
            .objects
            .get(object.0 as usize)
            .and_then(|declared| declared.fields.get(field as usize))
            .map(|(_, ty)| *ty)
            .ok_or_else(|| format!("object obj{} has no field {field}", object.0))
    }

    fn arguments(&self, what: &str, args: &[Register], params: &[TypeRef]) -> Rule {
        if args.len() != params.len() {
            return Err(format!("{what} passes {} arguments for {} parameters", args.len(), params.len()));
        }
        for (arg, param) in args.iter().zip(params) {
            self.expect_ref(*arg, *param)?;
        }
        Ok(())
    }

    /// Результат вызова: значение (`dst`) или его отсутствие.
    fn result(&self, what: &str, dst: Option<Register>, result: Option<&Type>) -> Rule {
        match (dst, result) {
            (None, _) => Ok(()),
            (Some(dst), Some(result)) => self.define(dst, result, what),
            (Some(_), None) => Err(format!("{what} of a void callee cannot define a value")),
        }
    }

    fn throwable(&self, ty: &Type) -> bool {
        let error_role = |role: &crate::RoleRef| self.program.roles.get(role.0 as usize).is_some_and(|r| r.is_error());
        match ty {
            Type::Object(o) => self
                .program
                .objects
                .get(o.0 as usize)
                .is_some_and(|d| d.does.iter().any(|w| error_role(&w.role))),
            Type::Role(r) => error_role(r),
            Type::Variant(v) => self.program.variants.get(v.0 as usize).is_some_and(|declared| {
                declared.cases.iter().all(|(_, o)| self.throwable(&Type::Object(*o)))
            }),
            _ => false,
        }
    }

    /// Перемещения перехода: параметр получает аргумент того же типа; на пути
    /// отказа `^C` и `C` совместимы (обязательство там не действует).
    fn moves(&self, moves: &[(Register, Register)], unwind: bool) -> Rule {
        for (param, arg) in moves {
            let (to, from) = (self.ty(*param), self.ty(*arg));
            let compatible = unwind
                && matches!((from, to), (Type::Successor(a), Type::Object(b)) | (Type::Object(a), Type::Successor(b)) if a == b);
            if !self.equal(from, to) && !compatible {
                return Err(format!(
                    "branch passes r{}: {} for r{}: {}",
                    arg.0,
                    self.name(from),
                    param.0,
                    self.name(to)
                ));
            }
        }
        Ok(())
    }

    fn target(&self, target: &Target) -> Rule {
        self.moves(&target.moves, false)
    }

    /// Обработчики: регистр отказа — `failure`, перемещения — пути отказа.
    pub(crate) fn handlers(&self) -> Rule {
        for handler in &self.function.handlers {
            self.expect(handler.failure, &Type::Failure)?;
            self.moves(&handler.target.moves, true)?;
        }
        Ok(())
    }

    pub(crate) fn instruction(&self, instruction: &Instruction) -> Rule {
        use Instruction as I;
        let int = Type::Int;
        let bool_ = Type::Bool;
        let byte = Type::Byte;
        let float = Type::Float;
        let rat = Type::Rat;
        let str_ = Type::Str;
        match instruction {
            I::GlobalGet { dst, global } => match self.program.globals.get(*global as usize) {
                Some(decl) => {
                    let ty = self.resolve(decl.ty).clone();
                    self.define(dst.0, &ty, "global.get")
                }
                None => Err(format!("global g{global} does not exist")),
            },
            I::Const { dst, value } => {
                let ty = match value {
                    Constant::Int(_) => Type::Int,
                    Constant::Bool(_) => Type::Bool,
                    Constant::Byte(_) => Type::Byte,
                    Constant::Float(_) => Type::Float,
                    Constant::Char(_) => Type::Char,
                    Constant::Str(_) => Type::Str,
                };
                self.define(dst.0, &ty, "const")
            }
            I::Move { dst, source } => {
                // `move` также переименовывает: succ.take (^C → C), retag
                // (@never → @T), map.retag (%never:never → %K:V), cap.make (C → cap of C).
                let (from, to) = (self.ty(*source), self.ty(dst.0));
                let renamed = match (from, to) {
                    (Type::Successor(a), Type::Object(b)) => a == b,
                    (Type::Array(e), Type::Array(_)) => *self.resolve(*e) == Type::Never,
                    (Type::Map(k, v), Type::Map(..)) => *self.resolve(*k) == Type::Never && *self.resolve(*v) == Type::Never,
                    (Type::Object(o), Type::Cap(c)) => self.program.caps.get(c.0 as usize).is_some_and(|cap| cap.owner == *o),
                    _ => false,
                };
                if self.equal(from, to) || renamed {
                    Ok(())
                } else {
                    Err(format!("move of r{}: {} into r{}: {}", source.0, self.name(from), dst.0.0, self.name(to)))
                }
            }
            I::Undef { dst } => match self.ty(dst.0) {
                Type::Builder(_) | Type::MapBuilder(..) => Err("`undef` defines a non-builder value".into()),
                _ => Ok(()),
            },
            I::IntAdd { dst, left, right }
            | I::IntSub { dst, left, right }
            | I::IntMul { dst, left, right }
            | I::IntMod { dst, left, right }
            | I::IntAnd { dst, left, right }
            | I::IntOr { dst, left, right }
            | I::IntXor { dst, left, right }
            | I::IntShl { dst, left, right }
            | I::IntShr { dst, left, right }
            | I::IntUshr { dst, left, right } => {
                self.expect(*left, &int)?;
                self.expect(*right, &int)?;
                self.define(dst.0, &int, "int")
            }
            I::IntNeg { dst, operand } | I::IntNot { dst, operand } | I::IntAbs { dst, operand } => {
                self.expect(*operand, &int)?;
                self.define(dst.0, &int, "int")
            }
            I::Sign { dst, operand } => {
                if !matches!(self.ty(*operand), Type::Int | Type::Float | Type::Rat) {
                    return Err(format!("sign of r{}: expected int, float or rat", operand.0));
                }
                self.define(dst.0, &int, "sign")
            }
            I::IntCmp { dst, left, right, .. } => {
                self.expect(*left, &int)?;
                self.expect(*right, &int)?;
                self.define(dst.0, &bool_, "int.cmp")
            }
            I::ByteFrom { dst, source, .. } => {
                if !matches!(self.ty(*source), Type::Int | Type::Float | Type::Rat) {
                    return Err(format!("byte.from of r{}: expected int, float or rat", source.0));
                }
                self.define(dst.0, &byte, "byte.from")
            }
            I::Convert { dst, source } => {
                let ok = matches!(
                    (self.ty(*source), self.ty(dst.0)),
                    (Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char, Type::Str)
                        | (Type::Byte | Type::Rat | Type::Char, Type::Int)
                        | (Type::Int, Type::Char)
                        | (Type::Int | Type::Byte | Type::Rat, Type::Float)
                        | (Type::Float, Type::Int)
                        | (Type::Int | Type::Byte | Type::Float, Type::Rat)
                );
                if ok {
                    Ok(())
                } else {
                    Err(format!(
                        "cvt from {} to {} is not an instruction",
                        self.name(self.ty(*source)),
                        self.name(self.ty(dst.0))
                    ))
                }
            }
            I::Bits { op, byte: is_byte, dst, value, count } => {
                let word = if *is_byte { Type::Byte } else { Type::Int };
                self.expect(*value, &word)?;
                if let Some(count) = count {
                    self.expect(*count, &int)?;
                }
                self.define(dst.0, if op.counts() { &int } else { &word }, "bits")
            }
            I::ByteAnd { dst, left, right } | I::ByteOr { dst, left, right } | I::ByteXor { dst, left, right } => {
                self.expect(*left, &byte)?;
                self.expect(*right, &byte)?;
                self.define(dst.0, &byte, "byte")
            }
            I::ByteShl { dst, left, right } | I::ByteShr { dst, left, right } | I::ByteUshr { dst, left, right } => {
                self.expect(*left, &byte)?;
                self.expect(*right, &int)?;
                self.define(dst.0, &byte, "byte shift")
            }
            I::ByteNot { dst, operand } => {
                self.expect(*operand, &byte)?;
                self.define(dst.0, &byte, "byte.not")
            }
            I::ByteCmp { dst, left, right, .. } => {
                self.expect(*left, &byte)?;
                self.expect(*right, &byte)?;
                self.define(dst.0, &bool_, "byte.cmp")
            }
            I::CharCmp { dst, left, right, .. } => {
                self.expect(*left, &Type::Char)?;
                self.expect(*right, &Type::Char)?;
                self.define(dst.0, &bool_, "char.cmp")
            }
            I::BoolNot { dst, operand } => {
                self.expect(*operand, &bool_)?;
                self.define(dst.0, &bool_, "bool.not")
            }
            I::BoolCmp { dst, left, right, .. } => {
                self.expect(*left, &bool_)?;
                self.expect(*right, &bool_)?;
                self.define(dst.0, &bool_, "bool.cmp")
            }
            I::FloatAdd { dst, left, right }
            | I::FloatSub { dst, left, right }
            | I::FloatMul { dst, left, right }
            | I::FloatDiv { dst, left, right }
            | I::FloatMod { dst, left, right }
            | I::FloatPow { dst, left, right } => {
                self.expect(*left, &float)?;
                self.expect(*right, &float)?;
                self.define(dst.0, &float, "float")
            }
            I::FloatNeg { dst, operand } | I::FloatAbs { dst, operand } | I::FloatSqrt { dst, operand } => {
                self.expect(*operand, &float)?;
                self.define(dst.0, &float, "float")
            }
            I::FloatCmp { dst, left, right, .. } => {
                self.expect(*left, &float)?;
                self.expect(*right, &float)?;
                self.define(dst.0, &bool_, "float.cmp")
            }
            I::RatNew { dst, left, right } => {
                self.expect(*left, &int)?;
                self.expect(*right, &int)?;
                self.define(dst.0, &rat, "rat.new")
            }
            I::RatAdd { dst, left, right }
            | I::RatSub { dst, left, right }
            | I::RatMul { dst, left, right }
            | I::RatDiv { dst, left, right }
            | I::RatMod { dst, left, right } => {
                self.expect(*left, &rat)?;
                self.expect(*right, &rat)?;
                self.define(dst.0, &rat, "rat")
            }
            I::RatNeg { dst, operand } | I::RatAbs { dst, operand } => {
                self.expect(*operand, &rat)?;
                self.define(dst.0, &rat, "rat")
            }
            I::RatCmp { dst, left, right, .. } => {
                self.expect(*left, &rat)?;
                self.expect(*right, &rat)?;
                self.define(dst.0, &bool_, "rat.cmp")
            }
            I::RatPart { dst, value, .. } => {
                self.expect(*value, &rat)?;
                self.define(dst.0, &str_, "rat.part")
            }
            I::StrConcat { dst, left, right } => {
                self.expect(*left, &str_)?;
                self.expect(*right, &str_)?;
                self.define(dst.0, &str_, "str.concat")
            }
            I::StrCmp { dst, left, right, .. } => {
                self.expect(*left, &str_)?;
                self.expect(*right, &str_)?;
                self.define(dst.0, &bool_, "str.cmp")
            }
            I::StrLen { dst, text } => {
                self.expect(*text, &str_)?;
                self.define(dst.0, &int, "str.len")
            }
            I::StrSlice { dst, text, start, end } => {
                self.expect(*text, &str_)?;
                self.expect(*start, &int)?;
                self.expect(*end, &int)?;
                self.define(dst.0, &str_, "str.slice")
            }
            I::StrSplit { dst, text, separator } => {
                self.expect(*text, &str_)?;
                self.expect(*separator, &str_)?;
                match self.ty(dst.0) {
                    Type::Array(e) if *self.resolve(*e) == Type::Str => Ok(()),
                    other => Err(format!("str.split defines @str, not {}", self.name(other))),
                }
            }
            I::StrReplace { dst, text, pattern, replacement }
            | I::StrReplaceFirst { dst, text, pattern, replacement } => {
                self.expect(*text, &str_)?;
                self.expect(*pattern, &str_)?;
                self.expect(*replacement, &str_)?;
                self.define(dst.0, &str_, "str.replace")
            }
            I::StrTrim { dst, text } | I::StrChomp { dst, text } => {
                self.expect(*text, &str_)?;
                self.define(dst.0, &str_, "str")
            }
            I::StrUtf8 { dst, text } => {
                self.expect(*text, &str_)?;
                match self.ty(dst.0) {
                    Type::Array(e) if *self.resolve(*e) == Type::Byte => Ok(()),
                    other => Err(format!("str.utf8 defines @byte, not {}", self.name(other))),
                }
            }
            I::StrFn { op, dst, args } => {
                let binary = matches!(
                    op,
                    StrFn::Find | StrFn::FindLast | StrFn::StartsWith | StrFn::EndsWith | StrFn::Repeat
                );
                if args.len() != 1 + usize::from(binary) {
                    return Err(format!("str.fn {} takes {} operands", op.name(), 1 + usize::from(binary)));
                }
                self.expect(args[0], &str_)?;
                if binary {
                    self.expect(args[1], if *op == StrFn::Repeat { &int } else { &str_ })?;
                }
                match op {
                    StrFn::Find | StrFn::FindLast => self.define(dst.0, &int, "str.fn"),
                    StrFn::StartsWith | StrFn::EndsWith => self.define(dst.0, &bool_, "str.fn"),
                    StrFn::Characters => match self.ty(dst.0) {
                        Type::Array(e) if *self.resolve(*e) == Type::Char => Ok(()),
                        other => Err(format!("str.fn characters defines @char, not {}", self.name(other))),
                    },
                    _ => self.define(dst.0, &str_, "str.fn"),
                }
            }
            I::ValCmp { dst, left, right, .. } => {
                let ty = self.ty(*left);
                self.expect(*right, ty)?;
                if matches!(
                    ty,
                    Type::Failure
                        | Type::Builder(_)
                        | Type::MapBuilder(..)
                        | Type::Successor(_)
                        | Type::Never
                        | Type::Handle(_)
                        | Type::Any
                        | Type::Cap(_)
                ) {
                    return Err(format!("val.cmp cannot compare {}", self.name(ty)));
                }
                self.define(dst.0, &bool_, "val.cmp")
            }
            I::EnumCase { dst, ty, .. } => self.define(dst.0, &Type::Enum(*ty), "enum.case"),
            I::EnumCmp { dst, left, right, .. } => {
                let ty = self.ty(*left);
                if !matches!(ty, Type::Enum(_)) {
                    return Err(format!("enum.cmp of non-enum r{}", left.0));
                }
                self.expect(*right, ty)?;
                self.define(dst.0, &bool_, "enum.cmp")
            }
            I::AnyBox { dst, source } => {
                let ty = self.ty(*source);
                if matches!(
                    ty,
                    Type::Any
                        | Type::Handle(_)
                        | Type::Fn { .. }
                        | Type::Future(_)
                        | Type::Cap(_)
                        | Type::Failure
                        | Type::Region
                        | Type::Never
                        | Type::Successor(_)
                        | Type::Builder(_)
                        | Type::MapBuilder(..)
                ) {
                    return Err(format!("any.box cannot box {}", self.name(ty)));
                }
                self.define(dst.0, &Type::Any, "any.box")
            }
            I::RangeNew { dst, start, end, .. } => {
                self.expect(*start, &int)?;
                self.expect(*end, &int)?;
                self.define(dst.0, &Type::Range, "range.new")
            }
            I::RangePart { dst, range, .. } => {
                self.expect(*range, &Type::Range)?;
                self.define(dst.0, &int, "range.part")
            }
            I::ObjNew { dst, object, fields } => {
                let declared = &self.program.objects[object.0 as usize];
                if declared.builtin {
                    return Err("values of a builtin object are created only by the runtime".into());
                }
                for (value, (_, ty)) in fields.iter().zip(&declared.fields) {
                    self.expect_ref(*value, *ty)?;
                }
                self.define(dst.0, &Type::Object(*object), "obj.new")
            }
            I::ObjInit { object, field, value } => {
                let owner = self.object(*object)?;
                self.expect_ref(*value, self.field(owner, *field)?)
            }
            I::ObjGet { dst, object, field } => {
                let owner = self.object(*object)?;
                self.define_ref(dst.0, self.field(owner, *field)?, "obj.get")
            }
            I::ObjNext { dst, object, updates } => {
                let owner = self.object(*object)?;
                let mut seen = std::collections::HashSet::new();
                for (field, value) in updates {
                    if !seen.insert(*field) {
                        return Err(format!("obj.next replaces field {field} twice"));
                    }
                    self.expect_ref(*value, self.field(owner, *field)?)?;
                }
                self.define(dst.0, &Type::Object(owner), "obj.next")
            }
            I::RoleWrap { dst, source, role } => {
                // Вариант делает роль, если её делает объект каждого случая.
                let objects: Vec<crate::ObjectRef> = match self.ty(*source) {
                    Type::Variant(v) => self.program.variants[v.0 as usize].cases.iter().map(|(_, o)| *o).collect(),
                    _ => vec![self.object(*source)?],
                };
                for object in objects {
                    let does = self.program.objects[object.0 as usize].does.iter().any(|w| w.role == *role);
                    if !does {
                        return Err(format!("object obj{} does not do role role{}", object.0, role.0));
                    }
                }
                self.define(dst.0, &Type::Role(*role), "role.wrap")
            }
            I::VariantNew { dst, variant, case, fields } => {
                let object = self.case_object(*variant, *case)?;
                let declared = &self.program.objects[object.0 as usize];
                if declared.fields.len() != fields.len() {
                    return Err(format!("variant.new case {case} lists {} fields, the case has {}", fields.len(), declared.fields.len()));
                }
                for (value, (_, ty)) in fields.iter().zip(&declared.fields) {
                    self.expect_ref(*value, *ty)?;
                }
                self.define(dst.0, &Type::Variant(*variant), "variant.new")
            }
            I::VariantTag { dst, value } => {
                self.variant(*value)?;
                self.define(dst.0, &Type::Int, "variant.tag")
            }
            I::VariantGet { dst, value, case, field } => {
                let variant = self.variant(*value)?;
                let object = self.case_object(variant, *case)?;
                self.define_ref(dst.0, self.field(object, *field)?, "variant.get")
            }
            I::VariantOf { dst, source, variant } => {
                let object = self.object(*source)?;
                let case = self.program.variants[variant.0 as usize].cases.iter().any(|(_, o)| *o == object);
                if !case {
                    return Err(format!("object obj{} is not a case of variant variant{}", object.0, variant.0));
                }
                self.define(dst.0, &Type::Variant(*variant), "variant.of")
            }
            I::RoleNarrow { dst, source, role, .. } => {
                if !matches!(self.ty(*source), Type::Role(_)) {
                    return Err(format!("role.narrow of r{}: not a role reference", source.0));
                }
                self.define(dst.0, &Type::Role(*role), "role.narrow")
            }
            I::ArrayNew { dst, elements } => match self.ty(dst.0) {
                Type::Array(e) => {
                    if *self.resolve(*e) == Type::Never && !elements.is_empty() {
                        return Err("@never cannot have elements".into());
                    }
                    for value in elements {
                        self.expect_ref(*value, *e)?;
                    }
                    Ok(())
                }
                other => Err(format!("arr.new defines an array, not {}", self.name(other))),
            },
            I::ArrayLen { dst, array } => {
                if !matches!(self.ty(*array), Type::Array(_)) {
                    return Err(format!("arr.len of non-array r{}", array.0));
                }
                self.define(dst.0, &int, "arr.len")
            }
            I::ArrayGet { dst, array, index } => {
                self.expect(*index, &int)?;
                self.define_ref(dst.0, self.element(*array)?, "arr.get")
            }
            I::ArraySlice { dst, array, start, end } => {
                self.expect(*start, &int)?;
                self.expect(*end, &int)?;
                if !matches!(self.ty(*array), Type::Array(_)) {
                    return Err(format!("arr.slice of non-array r{}", array.0));
                }
                self.define(dst.0, self.ty(*array), "arr.slice")
            }
            I::ArrayJoin { dst, array, separator } => {
                match self.ty(*array) {
                    Type::Array(e) if *self.resolve(*e) == Type::Str => {}
                    other => return Err(format!("arr.join of {}, expected @str", self.name(other))),
                }
                self.expect(*separator, &str_)?;
                self.define(dst.0, &str_, "arr.join")
            }
            I::BuilderNew { dst } => match self.ty(dst.0) {
                Type::Builder(_) => Ok(()),
                other => Err(format!("builder.new defines a builder, not {}", self.name(other))),
            },
            I::BuilderPush { dst, builder, value } => {
                let ty = self.ty(*builder);
                let Type::Builder(e) = ty else {
                    return Err(format!("r{} is not a builder", builder.0));
                };
                self.expect_ref(*value, *e)?;
                self.define(dst.0, ty, "builder.push")
            }
            I::ArrayConcat { dst, parts } | I::ArrayDiff { dst, removed: parts, .. } => {
                let ty = self.ty(dst.0);
                let Type::Array(element) = ty else {
                    return Err(format!("{} defines {}, not an array", instruction.mnemonic(), self.name(ty)));
                };
                if parts.is_empty() && matches!(instruction, I::ArrayConcat { .. }) {
                    return Err("arr.concat needs at least one part".into());
                }
                let base = if let I::ArrayDiff { array, .. } = instruction { Some(*array) } else { None };
                for part in base.iter().chain(parts.iter()) {
                    match self.ty(*part) {
                        Type::Array(e) if self.equal(self.resolve(*e), self.resolve(*element)) || *self.resolve(*e) == Type::Never => {}
                        other => return Err(format!("{} of {} into {}", instruction.mnemonic(), self.name(other), self.name(ty))),
                    }
                }
                Ok(())
            }
            I::MapMerge { dst, parts } | I::MapDiff { dst, removed: parts, .. } => {
                let ty = self.ty(dst.0);
                let Type::Map(key, value) = ty else {
                    return Err(format!("{} defines {}, not a map", instruction.mnemonic(), self.name(ty)));
                };
                if parts.is_empty() && matches!(instruction, I::MapMerge { .. }) {
                    return Err("map.merge needs at least one part".into());
                }
                let base = if let I::MapDiff { map, .. } = instruction { Some(*map) } else { None };
                for part in base.iter().chain(parts.iter()) {
                    match self.ty(*part) {
                        Type::Map(k, v)
                            if (self.equal(self.resolve(*k), self.resolve(*key)) && self.equal(self.resolve(*v), self.resolve(*value)))
                                || *self.resolve(*k) == Type::Never => {}
                        other => return Err(format!("{} of {} into {}", instruction.mnemonic(), self.name(other), self.name(ty))),
                    }
                }
                Ok(())
            }
            I::BuilderAppend { dst, builder, array } => {
                let ty = self.ty(*builder);
                let Type::Builder(e) = ty else {
                    return Err(format!("r{} is not a builder", builder.0));
                };
                match self.ty(*array) {
                    Type::Array(a) if self.equal(self.resolve(*a), self.resolve(*e)) || *self.resolve(*a) == Type::Never => {}
                    other => return Err(format!("builder.append of {} into {}", self.name(other), self.name(ty))),
                }
                self.define(dst.0, ty, "builder.append")
            }
            I::BuilderRange { dst, builder, start, end, .. } => {
                match self.ty(*builder) {
                    Type::Builder(e) if *self.resolve(*e) == Type::Int => {}
                    other => return Err(format!("builder.range of {}, expected builder<int>", self.name(other))),
                }
                self.expect(*start, &int)?;
                self.expect(*end, &int)?;
                self.define(dst.0, self.ty(*builder), "builder.range")
            }
            I::BuilderFinish { dst, builder } => match self.ty(*builder) {
                Type::Builder(e) => self.define(dst.0, &Type::Array(*e), "builder.finish"),
                other => Err(format!("builder.finish of {}", self.name(other))),
            },
            I::MapNew { dst, entries } => match self.ty(dst.0) {
                Type::Map(k, v) => {
                    if *self.resolve(*k) == Type::Never && !entries.is_empty() {
                        return Err("%never:never cannot have entries".into());
                    }
                    for (key, value) in entries {
                        self.expect_ref(*key, *k)?;
                        self.expect_ref(*value, *v)?;
                    }
                    Ok(())
                }
                other => Err(format!("map.new defines a map, not {}", self.name(other))),
            },
            I::MapLen { dst, map } => {
                if !matches!(self.ty(*map), Type::Map(..)) {
                    return Err(format!("map.len of non-map r{}", map.0));
                }
                self.define(dst.0, &int, "map.len")
            }
            I::MapHas { dst, map, key } => {
                let (k, _) = self.entry(*map)?;
                self.expect_ref(*key, k)?;
                self.define(dst.0, &bool_, "map.has")
            }
            I::MapGet { dst, map, key } => {
                let (k, v) = self.entry(*map)?;
                self.expect_ref(*key, k)?;
                self.define_ref(dst.0, v, "map.get")
            }
            I::MapKeys { dst, map } | I::MapValues { dst, map } => match self.ty(*map) {
                Type::Map(k, v) => {
                    let part = if matches!(instruction, I::MapKeys { .. }) { *k } else { *v };
                    self.define(dst.0, &Type::Array(part), "map.keys/values")
                }
                other => Err(format!("map.keys/values of {}", self.name(other))),
            },
            I::MapBuilderNew { dst } => match self.ty(dst.0) {
                Type::MapBuilder(..) => Ok(()),
                other => Err(format!("mbuilder.new defines a map builder, not {}", self.name(other))),
            },
            I::MapBuilderPut { dst, builder, key, value } => {
                let ty = self.ty(*builder);
                let Type::MapBuilder(k, v) = ty else {
                    return Err(format!("r{} is not a map builder", builder.0));
                };
                self.expect_ref(*key, *k)?;
                self.expect_ref(*value, *v)?;
                self.define(dst.0, ty, "mbuilder.put")
            }
            I::MapBuilderFinish { dst, builder } => match self.ty(*builder) {
                Type::MapBuilder(k, v) => self.define(dst.0, &Type::Map(*k, *v), "mbuilder.finish"),
                other => Err(format!("mbuilder.finish of {}", self.name(other))),
            },
            I::Call { dst, function, args } => {
                let callee = &self.program.functions[function.0 as usize];
                let params: Vec<TypeRef> = callee.params.iter().map(|p| callee.registers[p.0 as usize]).collect();
                self.arguments(&format!("call of @{}", callee.symbol), args, &params)?;
                let result = if callee.is_async {
                    Some(Type::Future(callee.result))
                } else {
                    callee.result.map(|r| self.resolve(r).clone())
                };
                self.result(&format!("call of @{}", callee.symbol), dst.map(|d| d.0), result.as_ref())
            }
            I::Closure { dst, function, captures } => {
                let callee = &self.program.functions[function.0 as usize];
                let Type::Fn { params, result } = self.ty(dst.0) else {
                    return Err("fn.closure defines a fn value".into());
                };
                if callee.params.len() != captures.len() + params.len() {
                    return Err(format!("fn.closure of @{} does not match its fn type", callee.symbol));
                }
                let types: Vec<TypeRef> = callee.params.iter().map(|p| callee.registers[p.0 as usize]).collect();
                for (capture, ty) in captures.iter().zip(&types) {
                    self.expect_ref(*capture, *ty)?;
                }
                let rest = &types[captures.len()..];
                let same_params = rest.iter().zip(params).all(|(a, b)| self.equal(self.resolve(*a), self.resolve(*b)));
                // Тип fn async-цели несёт Future результата.
                let callee_result = if callee.is_async { Some(Type::Future(callee.result)) } else { callee.result.map(|r| self.resolve(r).clone()) };
                let same_result = match (&callee_result, result) {
                    (None, None) => true,
                    (Some(a), Some(b)) => self.equal(a, self.resolve(*b)),
                    _ => false,
                };
                if !same_params || !same_result {
                    return Err(format!("fn.closure of @{} does not match its fn type", callee.symbol));
                }
                Ok(())
            }
            I::CallFn { dst, callee, args } => {
                let Type::Fn { params, result } = self.ty(*callee) else {
                    return Err(format!("call.fn of r{}: not a fn value", callee.0));
                };
                self.arguments("call.fn", args, params)?;
                self.result("call.fn", dst.map(|d| d.0), result.map(|r| self.resolve(r)))
            }
            I::CallRole { dst, receiver, role, slot, args } => {
                match self.ty(*receiver) {
                    Type::Role(actual) if actual == role => {}
                    other => return Err(format!("call.role receiver r{}: {}, expected role{}", receiver.0, self.name(other), role.0)),
                }
                let method = &self.program.roles[role.0 as usize].methods[*slot as usize];
                self.arguments(&format!("call.role .{}", method.name), args, &method.params)?;
                self.result(&format!("call.role .{}", method.name), dst.map(|d| d.0), method.result.map(|r| self.resolve(r)))
            }
            I::CallCap { dst, cap, ty, field } => {
                match self.ty(*cap) {
                    Type::Cap(actual) if actual == ty => {}
                    other => return Err(format!("call.cap of r{}: {}, expected cap{}", cap.0, self.name(other), ty.0)),
                }
                let function = self.program.caps[ty.0 as usize].fields[*field as usize].1;
                let callee = &self.program.functions[function.0 as usize];
                match callee.result {
                    Some(result) => self.define_ref(dst.0, result, "call.cap"),
                    None => Err("call.cap of a void field".into()),
                }
            }
            I::Await { dst, future } => {
                if !self.function.is_async {
                    return Err("await is allowed only in async functions".into());
                }
                let Type::Future(result) = self.ty(*future) else {
                    return Err(format!("await of r{}: not a future", future.0));
                };
                match (dst, result) {
                    (None, _) => Ok(()),
                    (Some(dst), Some(result)) => self.define_ref(dst.0, *result, "await"),
                    (Some(_), None) => Err("await of future<void> cannot define a value".into()),
                }
            }
            I::FutSpawn { dst, future } => {
                if !matches!(self.ty(*future), Type::Future(_)) {
                    return Err(format!("fut.spawn of r{}: not a future", future.0));
                }
                self.define(dst.0, self.ty(*future), "fut.spawn")
            }
            I::Fut { op, dst, future } => {
                if !matches!(self.ty(*future), Type::Future(_)) {
                    return Err(format!("fut.{op:?} of r{}: not a future", future.0));
                }
                match (op, dst) {
                    (FutOp::Cancel, None) => Ok(()),
                    (FutOp::Cancel, Some(_)) => Err("fut.cancel defines no value".into()),
                    (_, Some(dst)) => self.define(dst.0, &bool_, "fut"),
                    (_, None) => Ok(()),
                }
            }
            I::WorkerSpawn { dst, entry, args, .. } => {
                let Type::Fn { params, result } = self.ty(*entry) else {
                    return Err(format!("worker.spawn of r{}: not a fn value", entry.0));
                };
                self.arguments("worker.spawn", args, params)?;
                self.define(dst.0, &Type::Future(*result), "worker.spawn")
            }
            I::WorkerBind { dst, action, entry, .. } => {
                let ty = self.ty(*action);
                if !matches!(ty, Type::Fn { .. }) || !matches!(self.ty(*entry), Type::Fn { .. }) {
                    return Err("worker.bind takes fn values".into());
                }
                self.define(dst.0, ty, "worker.bind")
            }
            I::RegionEnter { dst } => self.define(dst.0, &Type::Region, "region.enter"),
            I::RegionAdopt { region, value } | I::RegionTrack { region, value } => {
                self.expect(*region, &Type::Region)?;
                self.ownable(*value)
            }
            I::RegionTransfer { from, to, value } => {
                self.expect(*from, &Type::Region)?;
                self.expect(*to, &Type::Region)?;
                self.ownable(*value)
            }
            I::RegionLeave { region, .. } => self.expect(*region, &Type::Region),
            I::RegionUnwind { dst, region, failure } => {
                self.expect(*region, &Type::Region)?;
                self.expect(*failure, &Type::Failure)?;
                self.define(dst.0, &Type::Failure, "region.unwind")
            }
            I::RegionSettle { region, .. } => {
                if !self.function.is_async {
                    return Err("region.settle is allowed only in async functions".into());
                }
                self.expect(*region, &Type::Region)
            }
            I::RegionSettleUnwind { dst, region, failure } => {
                if !self.function.is_async {
                    return Err("region.settle_unwind is allowed only in async functions".into());
                }
                self.expect(*region, &Type::Region)?;
                self.expect(*failure, &Type::Failure)?;
                self.define(dst.0, &Type::Failure, "region.settle_unwind")
            }
            I::FailSecondary { dst, primary, cleanup } => {
                self.expect(*primary, &Type::Failure)?;
                self.expect(*cleanup, &Type::Failure)?;
                self.define(dst.0, &Type::Failure, "fail.secondary")
            }
            I::FailCaused { dst, new, handled } => {
                self.expect(*new, &Type::Failure)?;
                self.expect(*handled, &Type::Failure)?;
                self.define(dst.0, &Type::Failure, "fail.caused")
            }
            I::Jump { target } => self.target(target),
            I::Branch { condition, then_target, else_target } => {
                self.expect(*condition, &bool_)?;
                self.target(then_target)?;
                self.target(else_target)
            }
            I::Return { value } => match (self.function.result, value) {
                (Some(result), Some(value)) => {
                    // Successor-действие возвращает построенный объект или ^ того же объекта.
                    if let (Type::Successor(o), Type::Object(p)) = (self.resolve(result), self.ty(*value))
                        && o == p {
                            return Ok(());
                        }
                    self.expect_ref(*value, result)
                }
                (None, None) => Ok(()),
                (Some(_), None) => Err("`ret` without a value in a function with a result".into()),
                (None, Some(_)) => Err("`ret` with a value in a void function".into()),
            },
            I::Panic { message } => self.expect(*message, &str_),
            I::Trap { .. } | I::Unreachable {} => Ok(()),
            I::Throw { value } => {
                let ty = self.ty(*value);
                if self.throwable(ty) {
                    Ok(())
                } else {
                    Err(format!("throw r{}: {} does not do Error", value.0, self.name(ty)))
                }
            }
            I::Raise { failure } => self.expect(*failure, &Type::Failure),
            I::Catch { failure, kind, ty, value, handler, next } => {
                self.expect(*failure, &Type::Failure)?;
                let caught = match kind {
                    CatchKind::Object => Type::Object(crate::ObjectRef(*ty)),
                    CatchKind::Role => Type::Role(crate::RoleRef(*ty)),
                    CatchKind::Variant => Type::Variant(crate::VariantRef(*ty)),
                };
                self.define(value.0, &caught, "catch")?;
                self.target(handler)?;
                self.target(next)
            }
            I::ArrayLookup { array, index, value, found, missing } => {
                self.expect(*index, &int)?;
                self.define_ref(value.0, self.element(*array)?, "arr.lookup")?;
                self.target(found)?;
                self.target(missing)
            }
            I::MapLookup { map, key, value, found, missing } => {
                let (k, v) = self.entry(*map)?;
                self.expect_ref(*key, k)?;
                self.define_ref(value.0, v, "map.lookup")?;
                self.target(found)?;
                self.target(missing)
            }
        }
    }

    /// Тип SPB совпадает с типом ABI provider (`native`).
    fn abi(&self, ty: &Type, abi: &crate::abi::Type) -> bool {
        use crate::abi::Type as A;
        match (ty, abi) {
            (Type::Int, A::Int)
            | (Type::Float, A::Float)
            | (Type::Rat, A::Rat)
            | (Type::Bool, A::Bool)
            | (Type::Byte, A::Byte)
            | (Type::Char, A::Char)
            | (Type::Str, A::Str)
            | (Type::Any, A::Any) => true,
            // Handle SPB назван с модулем: `runtime.buffer::BufferState`.
            (Type::Handle(h), A::Handle(name)) => self
                .program
                .handles
                .get(*h as usize)
                .is_some_and(|h| h.rsplit("::").next() == Some(name)),
            (Type::Array(e), A::Array(item)) => self.abi(self.resolve(*e), item),
            (Type::Map(k, v), A::Map(key, value)) => self.abi(self.resolve(*k), key) && self.abi(self.resolve(*v), value),
            _ => false,
        }
    }

    fn ownable(&self, value: Register) -> Rule {
        let ty = self.ty(value);
        if matches!(ty, Type::Region | Type::Failure | Type::Builder(_) | Type::MapBuilder(..)) {
            return Err(format!("{} has no identities to own", self.name(ty)));
        }
        Ok(())
    }
}

/// Структурное равенство ссылок таблицы типов (вне функции).
pub(crate) fn same(program: &Program, a: TypeRef, b: TypeRef) -> bool {
    let function = Function {
        symbol: String::new(),
        is_async: false,
        native: None,
        intrinsic: None,
        registers: Vec::new(),
        params: Vec::new(),
        result: None,
        code: Vec::new(),
        handlers: Vec::new(),
    };
    let typing = Typing::new(program, &function);
    match (program.types.get(a.0 as usize), program.types.get(b.0 as usize)) {
        (Some(a), Some(b)) => typing.equal(a, b),
        _ => false,
    }
}

/// Сигнатуры функций, которые рантайм вызывает по таблицам: слоты ролей
/// (получатель, затем параметры метода), поля cap и `destroy` (владелец),
/// точка входа и тесты (без параметров).
pub(crate) fn tables(program: &Program) -> Result<(), String> {
    let signature = |function: crate::FuncRef| {
        let function = &program.functions[function.0 as usize];
        let params: Vec<TypeRef> = function.params.iter().map(|p| function.registers[p.0 as usize]).collect();
        (function, params)
    };
    for (owner, object) in program.objects.iter().enumerate() {
        let receiver = Type::Object(crate::ObjectRef(owner as u32));
        let first = |function: &Function, params: &[TypeRef]| {
            let typing = Typing::new(program, function);
            params.first().is_some_and(|p| typing.equal(typing.resolve(*p), &receiver))
        };
        for witness in &object.does {
            let role = &program.roles[witness.role.0 as usize];
            for (method, slot) in role.methods.iter().zip(&witness.slots) {
                let (function, params) = signature(*slot);
                let typing = Typing::new(program, function);
                let same_params = params.len() == method.params.len() + 1
                    && first(function, &params)
                    && params[1..].iter().zip(&method.params).all(|(a, b)| typing.equal(typing.resolve(*a), typing.resolve(*b)));
                let result = if function.is_async {
                    Some(Type::Future(function.result))
                } else {
                    function.result.map(|r| typing.resolve(r).clone())
                };
                let same_result = match (&result, method.result) {
                    (None, None) => true,
                    (Some(a), Some(b)) => typing.equal(a, typing.resolve(b)),
                    _ => false,
                };
                if !same_params || !same_result {
                    return Err(format!(
                        "@{} does not match method {}.{} of {}",
                        function.symbol, role.name, method.name, object.name
                    ));
                }
            }
        }
        if let Some(destroy) = object.destroy {
            let (function, params) = signature(destroy);
            // An async destroy (#154) is awaited by `region.settle`.
            if params.len() != 1 || !first(function, &params) {
                return Err(format!("@{} is not a destroy of {}", function.symbol, object.name));
            }
        }
    }
    for cap in &program.caps {
        let receiver = Type::Object(cap.owner);
        for (name, field) in &cap.fields {
            let (function, params) = signature(*field);
            let typing = Typing::new(program, function);
            let owner = params.len() == 1 && typing.equal(typing.resolve(params[0]), &receiver);
            if !owner || function.is_async || function.result.is_none() {
                return Err(format!("@{} is not field {name} of cap {}", function.symbol, cap.name));
            }
        }
    }
    for (index, global) in program.globals.iter().enumerate() {
        let (function, params) = signature(global.init);
        let typing = Typing::new(program, function);
        let result = function.result.map(|result| typing.resolve(result).clone());
        let ty = typing.resolve(global.ty).clone();
        if !params.is_empty() || function.is_async || !result.is_some_and(|result| typing.equal(&result, &ty)) {
            return Err(format!("@{} is not the init of global g{index}", function.symbol));
        }
    }
    for function in &program.functions {
        let Some(native) = function.native.as_deref().and_then(crate::abi::Native::resolve) else {
            continue;
        };
        let typing = Typing::new(program, function);
        let signature = native.signature();
        let params = function.params.len() == signature.params.len()
            && function.params.iter().zip(&signature.params).all(|(p, abi)| typing.abi(typing.ty(*p), abi));
        let result = match function.result {
            None => signature.result == crate::abi::Type::Void,
            Some(result) => typing.abi(typing.resolve(result), &signature.result),
        };
        if !params || !result || function.is_async != signature.is_async {
            return Err(format!("@{} does not match the signature of native {}", function.symbol, native.symbol()));
        }
    }
    for (name, function) in program.entry.iter().map(|f| ("entry", *f)).chain(program.tests.iter().flatten().map(|(_, f)| ("test", *f))) {
        let (function, params) = signature(function);
        if !params.is_empty() {
            return Err(format!("{name} @{} takes parameters", function.symbol));
        }
        // The core reads the entry's result as the exit status (`run_entry`).
        let int = function.result.is_some_and(|r| program.types[r.0 as usize] == Type::Int);
        if name == "entry" && !int {
            return Err(format!("entry @{} must return int", function.symbol));
        }
    }
    Ok(())
}
