//! Канонический текст OIR-1 (§18 спецификации).
//!
//! Значения печатаются как `%N`, блоки как `^bN`, по их индексам; объекты,
//! поля и функции — по именам из модуля. Принтер не проверяет модуль:
//! корректность обеспечивает [`super::verify`], а повреждённые ссылки
//! печатаются как `<invalid>`, без паники.
//!
//! `Display` у [`Type`] и [`InstKind`] без модуля нужен диагностике
//! верификатора: объекты, поля и функции там выводятся номерами.

use std::fmt::{self, Display, Formatter, Write};

use super::{
    Attr, Block, CatchType, Constant, Function, InstKind, Module, ResultType, Target, TermKind,
    Type, ValueId,
};

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_type(f, None, self)
    }
}

impl Display for ValueId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "^b{}", self.block.0)?;
        if !self.args.is_empty() {
            f.write_char('(')?;
            write_list(f, &self.args)?;
            f.write_char(')')?;
        }
        Ok(())
    }
}

impl Display for Constant {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Constant::Int(value) => write!(f, "{value}"),
            Constant::Bool(value) => write!(f, "{value}"),
            Constant::Byte(value) => write!(f, "{value}"),
            Constant::Float(bits) => f.write_str(&super::format_float(f64::from_bits(*bits))),
            Constant::Char(ch) => write_char_literal(f, *ch),
            Constant::Str(text) => write_string(f, text),
        }
    }
}

impl Display for InstKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_inst(f, None, self)
    }
}

impl Display for TermKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_term(f, None, self)
    }
}

fn write_term(f: &mut Formatter<'_>, module: Option<&Module>, kind: &TermKind) -> fmt::Result {
    {
        match kind {
            TermKind::Br(target) => write!(f, "br {target}"),
            TermKind::Cond {
                condition,
                then_target,
                else_target,
            } => write!(f, "cond {condition}, {then_target}, {else_target}"),
            TermKind::Ret(Some(value)) => write!(f, "ret {value}"),
            TermKind::Ret(None) => f.write_str("ret"),
            TermKind::Panic(value) => write!(f, "panic {value}"),
            TermKind::Trap { code, message } => {
                write!(f, "trap {} ", code.name())?;
                write_string(f, message)
            }
            TermKind::Unreachable => f.write_str("unreachable"),
            TermKind::Throw(value) => write!(f, "throw {value}"),
            TermKind::Raise(value) => write!(f, "raise {value}"),
            TermKind::Catch {
                failure,
                ty,
                handler,
                next,
            } => {
                write!(f, "catch {failure} is ")?;
                match ty {
                    CatchType::Object(object) => write_object(f, module, object.0)?,
                    CatchType::Role(role) => write_role(f, module, role.0)?,
                }
                write!(f, ", {handler}, {next}")
            }
            TermKind::ArrayLookup {
                array,
                index,
                found,
                missing,
            } => write!(f, "array.lookup {array}, {index}, {found}, {missing}"),
            TermKind::MapLookup {
                map,
                key,
                found,
                missing,
            } => write!(f, "map.lookup {map}, {key}, {found}, {missing}"),
        }
    }
}

impl Display for Module {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "oir 1 high")?;
        match self.version.as_str() {
            "" => writeln!(f, "module {}", self.name)?,
            version => writeln!(f, "module {} {version}", self.name)?,
        }
        writeln!(f, "requires core")?;
        if self.tests.is_some() {
            writeln!(f, "requires test")?;
        }
        for import in &self.imports {
            f.write_str("import ")?;
            write_string(f, &import.path)?;
            if let Some(alias) = &import.alias {
                write!(f, " as {alias}")?;
            }
            if !import.take.is_empty() {
                write!(f, " take {}", import.take.join(", "))?;
            }
            writeln!(f)?;
        }
        // rust-version 1.85 не поддерживает let-chains.
        if let Some(function) = self.entry.and_then(|entry| self.functions.get(entry.0)) {
            writeln!(f, "entry @{}", function.symbol)?;
        }
        for (name, func) in self.tests.iter().flatten() {
            f.write_str("test ")?;
            write_string(f, name)?;
            f.write_str(" = ")?;
            write_symbol(f, self, func.0)?;
            writeln!(f)?;
        }
        for decl in &self.enums {
            writeln!(f)?;
            writeln!(f, "enum {} = ({})", decl.name, decl.cases.join(", "))?;
        }
        for decl in &self.handles {
            writeln!(f)?;
            writeln!(f, "native object {}", decl.name)?;
        }
        for cap in &self.caps {
            writeln!(f)?;
            writeln!(f, "cap {} {{", cap.name)?;
            for (name, func) in &cap.fields {
                write!(f, "    field {name} = ")?;
                write_symbol(f, self, func.0)?;
                writeln!(f)?;
            }
            writeln!(f, "}}")?;
        }
        for role in &self.roles {
            writeln!(f)?;
            write!(f, "role {}", role.name)?;
            if role.builtin {
                f.write_str(" [builtin]")?;
            }
            writeln!(f, " {{")?;
            for method in &role.methods {
                write!(f, "    {}(", method.name)?;
                for (index, ty) in method.params.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write_type(f, Some(self), ty)?;
                }
                f.write_str(") -> ")?;
                write_result(f, self, &method.result)?;
                writeln!(f)?;
            }
            writeln!(f, "}}")?;
        }
        for object in &self.objects {
            writeln!(f)?;
            write!(f, "object {}", object.name)?;
            if object.builtin {
                f.write_str(" [builtin]")?;
            }
            writeln!(f, " {{")?;
            for field in &object.fields {
                write!(f, "    field {}: ", field.name)?;
                write_type(f, Some(self), &field.ty)?;
                if let Some(default) = field.default {
                    write!(f, " = ")?;
                    write_symbol(f, self, default.0)?;
                }
                writeln!(f)?;
            }
            for witness in &object.does {
                f.write_str("    does ")?;
                write_role(f, Some(self), witness.role.0)?;
                f.write_str(" {")?;
                for (slot, func) in witness.slots.iter().enumerate() {
                    f.write_str(if slot == 0 { " " } else { ", " })?;
                    match self.role(witness.role) {
                        Some(role) => f.write_str(&role.slot_name(slot))?,
                        None => f.write_str("<invalid>")?,
                    }
                    f.write_str(" = ")?;
                    write_symbol(f, self, func.0)?;
                }
                writeln!(f, "{}", if witness.slots.is_empty() { "}" } else { " }" })?;
            }
            if let Some(destroy) = object.destroy {
                f.write_str("    destroy = ")?;
                write_symbol(f, self, destroy.0)?;
                writeln!(f)?;
            }
            writeln!(f, "}}")?;
        }
        for function in &self.functions {
            writeln!(f)?;
            write_function(f, self, function)?;
        }
        Ok(())
    }
}

fn write_type(f: &mut Formatter<'_>, module: Option<&Module>, ty: &Type) -> fmt::Result {
    match ty {
        Type::Int => f.write_str("int"),
        Type::Bool => f.write_str("bool"),
        Type::Byte => f.write_str("byte"),
        Type::Float => f.write_str("float"),
        Type::Rat => f.write_str("rat"),
        Type::Char => f.write_str("char"),
        Type::Str => f.write_str("str"),
        Type::Object(object) => write_object(f, module, object.0),
        Type::Successor(object) => {
            f.write_char('^')?;
            write_object(f, module, object.0)
        }
        Type::Role(role) => write_role(f, module, role.0),
        Type::Enum(id) => match module.and_then(|module| module.enum_decl(*id)) {
            Some(decl) => f.write_str(&decl.name),
            None => write!(f, "enum#{}", id.0),
        },
        Type::Failure => f.write_str("failure"),
        Type::Array(element) => {
            f.write_str("array<")?;
            write_type(f, module, element)?;
            f.write_char('>')
        }
        Type::Builder(element) => {
            f.write_str("builder<")?;
            write_type(f, module, element)?;
            f.write_char('>')
        }
        Type::Never => f.write_str("never"),
        Type::Region => f.write_str("region"),
        Type::Any => f.write_str("any"),
        Type::Range => f.write_str("range"),
        Type::Cap(id) => match module.and_then(|module| module.cap(*id)) {
            Some(cap) => f.write_str(&cap.name),
            None => write!(f, "cap#{}", id.0),
        },
        Type::Future(result) => {
            f.write_str("future<")?;
            match &**result {
                ResultType::Void => f.write_str("void")?,
                ResultType::Value(ty) => write_type(f, module, ty)?,
            }
            f.write_char('>')
        }
        Type::Fn(sig) => {
            f.write_str("fn(")?;
            for (index, param) in sig.params.iter().enumerate() {
                if index > 0 {
                    f.write_str(", ")?;
                }
                write_type(f, module, param)?;
            }
            f.write_str(") -> ")?;
            match &sig.result {
                ResultType::Void => f.write_str("void"),
                ResultType::Value(ty) => write_type(f, module, ty),
            }
        }
        Type::Handle(id) => match module.and_then(|module| module.handle(*id)) {
            Some(decl) => f.write_str(&decl.name),
            None => write!(f, "handle#{}", id.0),
        },
        Type::Map(key, value) | Type::MapBuilder(key, value) => {
            f.write_str(if matches!(ty, Type::Map(..)) {
                "map<"
            } else {
                "mbuilder<"
            })?;
            write_type(f, module, key)?;
            f.write_char(',')?;
            write_type(f, module, value)?;
            f.write_char('>')
        }
    }
}

fn write_role(f: &mut Formatter<'_>, module: Option<&Module>, index: usize) -> fmt::Result {
    match module {
        Some(module) => match module.roles.get(index) {
            Some(role) => f.write_str(&role.name),
            None => f.write_str("<invalid>"),
        },
        None => write!(f, "role#{index}"),
    }
}

fn write_object(f: &mut Formatter<'_>, module: Option<&Module>, index: usize) -> fmt::Result {
    match module {
        Some(module) => match module.objects.get(index) {
            Some(object) => f.write_str(&object.name),
            None => f.write_str("<invalid>"),
        },
        None => write!(f, "object#{index}"),
    }
}

fn write_field(
    f: &mut Formatter<'_>,
    module: Option<&Module>,
    object: Option<usize>,
    field: usize,
) -> fmt::Result {
    let name = module
        .zip(object)
        .and_then(|(module, object)| module.objects.get(object))
        .and_then(|object| object.fields.get(field));
    match (module, name) {
        (_, Some(field)) => f.write_str(&field.name),
        (Some(_), None) => f.write_str("<invalid>"),
        (None, None) => write!(f, "#{field}"),
    }
}

fn write_symbol(f: &mut Formatter<'_>, module: &Module, index: usize) -> fmt::Result {
    match module.functions.get(index) {
        Some(function) => write!(f, "@{}", function.symbol),
        None => f.write_str("@<invalid>"),
    }
}

fn write_result(f: &mut Formatter<'_>, module: &Module, result: &ResultType) -> fmt::Result {
    match result {
        ResultType::Value(ty) => write_type(f, Some(module), ty),
        ResultType::Void => f.write_str("void"),
    }
}

/// Инструкция; `context` — модуль и функция для имён объектов и полей.
fn write_inst(
    f: &mut Formatter<'_>,
    context: Option<(&Module, &Function)>,
    kind: &InstKind,
) -> fmt::Result {
    let module = context.map(|(module, _)| module);
    // Объект операнда — по типу значения в функции.
    let object_of = |value: &ValueId| {
        context.and_then(|(_, function)| match function.value_type(*value) {
            Some(Type::Object(object) | Type::Successor(object)) => Some(object.0),
            _ => None,
        })
    };
    match kind {
        InstKind::Const(constant) => write!(f, "const {constant}"),
        InstKind::IntBinary { op, left, right } => {
            write!(f, "{} {left}, {right}", op.mnemonic())
        }
        InstKind::IntUnary { op, operand } => write!(f, "{} {operand}", op.mnemonic()),
        InstKind::IntCompare { op, left, right } => {
            write!(f, "int.{} {left}, {right}", op.suffix())
        }
        InstKind::BoolNot(operand) => write!(f, "bool.not {operand}"),
        InstKind::BoolCompare { op, left, right } => {
            write!(f, "bool.{} {left}, {right}", op.suffix())
        }
        InstKind::StrConcat { left, right } => write!(f, "str.concat {left}, {right}"),
        InstKind::StrCompare { op, left, right } => {
            write!(f, "str.{} {left}, {right}", op.suffix())
        }
        InstKind::ObjNew { object, fields } => {
            f.write_str("obj.new ")?;
            write_object(f, module, object.0)?;
            f.write_str(" {")?;
            for (index, value) in fields.iter().enumerate() {
                f.write_str(if index == 0 { " " } else { ", " })?;
                write_field(f, module, Some(object.0), index)?;
                write!(f, ": {value}")?;
            }
            f.write_str(if fields.is_empty() { "}" } else { " }" })
        }
        InstKind::ObjInit {
            object,
            field,
            value,
        } => {
            write!(f, "obj.init {object}, ")?;
            write_field(f, module, object_of(object), *field)?;
            write!(f, ", {value}")
        }
        InstKind::ObjGet { object, field } => {
            write!(f, "obj.get {object}, ")?;
            write_field(f, module, object_of(object), *field)
        }
        InstKind::ObjNext { object, updates } => {
            write!(f, "obj.next {object} {{")?;
            for (index, (field, value)) in updates.iter().enumerate() {
                f.write_str(if index == 0 { " " } else { ", " })?;
                write_field(f, module, object_of(object), *field)?;
                write!(f, ": {value}")?;
            }
            f.write_str(if updates.is_empty() { "}" } else { " }" })
        }
        InstKind::Call { func, args } => {
            f.write_str("call ")?;
            match module {
                Some(module) => write_symbol(f, module, func.0)?,
                None => write!(f, "@#{}", func.0)?,
            }
            f.write_char('(')?;
            write_list(f, args)?;
            f.write_char(')')
        }
        InstKind::FnClosure { func, captures } => {
            f.write_str("fn.closure ")?;
            match module {
                Some(module) => write_symbol(f, module, func.0)?,
                None => write!(f, "@#{}", func.0)?,
            }
            f.write_str(" [")?;
            write_list(f, captures)?;
            f.write_char(']')
        }
        InstKind::Await(future) => write!(f, "await {future}"),
        InstKind::CapMake(owner) => write!(f, "cap.make {owner}"),
        InstKind::CallCap { cap, field } => write!(f, "call.cap {cap}, #{field}()"),
        InstKind::FutSpawn(future) => write!(f, "fut.spawn {future}"),
        InstKind::WorkerSpawn {
            entry,
            args,
            direct,
        } => {
            let form = if *direct { ".direct" } else { "" };
            write!(f, "worker.spawn{form} {entry}(")?;
            write_list(f, args)?;
            f.write_char(')')
        }
        InstKind::WorkerBind {
            action,
            entry,
            fields,
        } => {
            write!(f, "worker.bind {action}, {entry}, [")?;
            write_list(f, fields)?;
            f.write_char(']')
        }
        InstKind::Fut { op, future } => write!(f, "{} {future}", op.mnemonic()),
        InstKind::CallFn { callee, args } => {
            write!(f, "call.fn {callee}(")?;
            write_list(f, args)?;
            f.write_char(')')
        }
        InstKind::SuccTake(operand) => write!(f, "succ.take {operand}"),
        InstKind::Cvt(operand) => write!(f, "cvt {operand}"),
        InstKind::RoleWrap(operand) => write!(f, "role.wrap {operand}"),
        InstKind::AnyBox(operand) => write!(f, "any.box {operand}"),
        InstKind::RoleNarrow { value, message } => {
            write!(f, "role.narrow {value}, ")?;
            write_string(f, message)
        }
        InstKind::CallRole {
            receiver,
            slot,
            args,
        } => {
            write!(f, "call.role {receiver}, ")?;
            let role =
                context.and_then(|(module, function)| match function.value_type(*receiver) {
                    Some(Type::Role(role)) => module.role(*role),
                    _ => None,
                });
            match role {
                Some(role) => write!(f, "{}.{}", role.name, role.slot_name(*slot))?,
                None => write!(f, "#{slot}")?,
            }
            f.write_char('(')?;
            write_list(f, args)?;
            f.write_char(')')
        }
        InstKind::FailCaused { new, handled } => write!(f, "fail.caused {new}, {handled}"),
        InstKind::ArrayNew(values) => {
            f.write_str("array.new [")?;
            write_list(f, values)?;
            f.write_char(']')
        }
        InstKind::ArrayLen(array) => write!(f, "array.len {array}"),
        InstKind::ArrayGet { array, index } => write!(f, "array.get {array}, {index}"),
        InstKind::ArraySlice { array, start, end } => {
            write!(f, "array.slice {array}, {start}, {end}")
        }
        InstKind::ArrayJoin { array, separator } => write!(f, "array.join {array}, {separator}"),
        InstKind::Retag(value) => write!(f, "array.retag {value}"),
        InstKind::BuilderNew => f.write_str("builder.new"),
        InstKind::BuilderPush { builder, value } => write!(f, "builder.push {builder}, {value}"),
        InstKind::BuilderAppend { builder, array } => {
            write!(f, "builder.append {builder}, {array}")
        }
        InstKind::BuilderAppendRange {
            builder,
            start,
            end,
            inclusive,
        } => write!(
            f,
            "builder.append.range {builder}, {start}, {end}, {}",
            if *inclusive { "inclusive" } else { "exclusive" }
        ),
        InstKind::BuilderFinish(builder) => write!(f, "builder.finish {builder}"),
        InstKind::Str { op, args } => {
            write!(f, "{} ", op.mnemonic())?;
            write_list(f, args)
        }
        InstKind::ValCompare { op, left, right } => {
            write!(f, "val.{} {left}, {right}", op.suffix())
        }
        InstKind::IntAbs(value) => write!(f, "int.abs {value}"),
        InstKind::IntSign(value) => write!(f, "int.sign {value}"),
        InstKind::ByteBinary { op, left, right } => {
            write!(f, "{} {left}, {right}", op.byte_mnemonic())
        }
        InstKind::ByteNot(value) => write!(f, "byte.not {value}"),
        InstKind::Float { op, args } => {
            write!(f, "{} ", op.mnemonic())?;
            write_list(f, args)
        }
        InstKind::FloatCompare { op, left, right } => {
            write!(f, "float.{} {left}, {right}", op.suffix())
        }
        InstKind::FloatSign(value) => write!(f, "float.sign {value}"),
        InstKind::Rat { op, args } => {
            write!(f, "{} ", op.mnemonic())?;
            write_list(f, args)
        }
        InstKind::RatCompare { op, left, right } => {
            write!(f, "rat.{} {left}, {right}", op.suffix())
        }
        InstKind::RatSign(value) => write!(f, "rat.sign {value}"),
        InstKind::RangeNew {
            start,
            end,
            inclusive,
        } => write!(
            f,
            "range.new {start}, {end}, {}",
            if *inclusive { "inclusive" } else { "exclusive" }
        ),
        InstKind::RangePart { range, end } => {
            write!(f, "range.{} {range}", if *end { "end" } else { "start" })
        }
        InstKind::RatPart { value, denominator } => write!(
            f,
            "rat.{} {value}",
            if *denominator {
                "denominator"
            } else {
                "numerator"
            }
        ),
        InstKind::EnumCase { ty, case } => match module.and_then(|module| module.enum_decl(*ty)) {
            Some(decl) => write!(
                f,
                "enum.case {}.{}",
                decl.name,
                decl.cases.get(*case).map_or("<invalid>", String::as_str)
            ),
            None => write!(f, "enum.case enum#{}.#{case}", ty.0),
        },
        InstKind::RegionEnter => f.write_str("region.enter"),
        InstKind::Undef => f.write_str("undef"),
        InstKind::RegionAdopt { region, value } => write!(f, "region.adopt {region}, {value}"),
        InstKind::RegionTransfer { from, to, value } => {
            write!(f, "region.transfer {from}, {to}, {value}")
        }
        InstKind::RegionTrack { region, value } => write!(f, "region.track {region}, {value}"),
        InstKind::RegionLeave { region, keep } => {
            write!(f, "region.leave {region}")?;
            if let Some(keep) = keep {
                write!(f, ", keep {keep}")?;
            }
            Ok(())
        }
        InstKind::RegionUnwind { region, failure } => {
            write!(f, "region.unwind {region}, {failure}")
        }
        InstKind::FailSecondary { primary, cleanup } => {
            write!(f, "fail.secondary {primary}, {cleanup}")
        }
        InstKind::EnumCompare { op, left, right } => {
            write!(f, "enum.{} {left}, {right}", op.suffix())
        }
        InstKind::ByteFrom { value, context } => {
            write!(f, "byte.from {value}")?;
            if let Some(context) = context {
                f.write_str(", ")?;
                write_string(f, context)?;
            }
            Ok(())
        }
        InstKind::ByteCompare { op, left, right } => {
            write!(f, "byte.{} {left}, {right}", op.suffix())
        }
        InstKind::CharCompare { op, left, right } => {
            write!(f, "char.{} {left}, {right}", op.suffix())
        }
        InstKind::Bits {
            op,
            byte,
            value,
            count,
        } => {
            let family = if *byte { "byte" } else { "int" };
            write!(f, "{family}.{} {value}", op.name())?;
            if let Some(count) = count {
                write!(f, ", {count}")?;
            }
            Ok(())
        }
        InstKind::MapNew(entries) => {
            f.write_str("map.new [")?;
            for (index, (key, value)) in entries.iter().enumerate() {
                if index > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{key}: {value}")?;
            }
            f.write_char(']')
        }
        InstKind::MapLen(map) => write!(f, "map.len {map}"),
        InstKind::MapHas { map, key } => write!(f, "map.has {map}, {key}"),
        InstKind::MapGet { map, key } => write!(f, "map.get {map}, {key}"),
        InstKind::MapKeys(map) => write!(f, "map.keys {map}"),
        InstKind::MapValues(map) => write!(f, "map.values {map}"),
        InstKind::MapRetag(map) => write!(f, "map.retag {map}"),
        InstKind::MapBuilderNew => f.write_str("mbuilder.new"),
        InstKind::MapBuilderPut {
            builder,
            key,
            value,
        } => write!(f, "mbuilder.put {builder}, {key}, {value}"),
        InstKind::MapBuilderFinish(builder) => write!(f, "mbuilder.finish {builder}"),
    }
}

fn write_function(f: &mut Formatter<'_>, module: &Module, function: &Function) -> fmt::Result {
    write!(f, "func @{}(", function.symbol)?;
    for (index, param) in function.params.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{param}: ")?;
        write_value_type(f, module, function, *param)?;
    }
    f.write_str(") -> ")?;
    write_result(f, module, &function.result)?;
    if !function.attrs.is_empty() {
        f.write_str(" [")?;
        for (index, attr) in function.attrs.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            match attr {
                Attr::Entry => f.write_str("entry")?,
                Attr::Private => f.write_str("private")?,
                Attr::Async => f.write_str("async")?,
                Attr::Intrinsic {
                    name,
                    objects,
                    helpers,
                } => {
                    f.write_str("intrinsic ")?;
                    write_string(f, name)?;
                    for object in objects {
                        f.write_char(' ')?;
                        write_object(f, Some(module), object.0)?;
                    }
                    for helper in helpers {
                        write!(f, " @{}", module.functions[helper.0].symbol)?;
                    }
                }
                Attr::Lambda { captures, owner } => {
                    write!(f, "lambda {captures}")?;
                    if let Some(object) = owner {
                        f.write_str(" of ")?;
                        write_object(f, Some(module), object.0)?;
                    }
                }
                Attr::Method(object) => {
                    f.write_str("method ")?;
                    write_object(f, Some(module), object.0)?;
                }
                Attr::Ctor(object) => {
                    f.write_str("ctor ")?;
                    write_object(f, Some(module), object.0)?;
                }
                Attr::Successor(object) => {
                    f.write_str("successor ")?;
                    write_object(f, Some(module), object.0)?;
                }
                Attr::Extern(symbol) => {
                    f.write_str("extern ")?;
                    write_string(f, symbol)?;
                }
                Attr::Destroy(object) => {
                    f.write_str("destroy ")?;
                    write_object(f, Some(module), object.0)?;
                }
                Attr::Default(object, field) => {
                    f.write_str("default ")?;
                    write_object(f, Some(module), object.0)?;
                    f.write_char('.')?;
                    write_field(f, Some(module), Some(object.0), *field)?;
                }
            }
        }
        f.write_char(']')?;
    }
    if function.extern_symbol().is_some() || function.intrinsic().is_some() {
        return writeln!(f);
    }
    writeln!(f, " {{")?;
    for (index, block) in function.blocks.iter().enumerate() {
        write_block(f, module, function, index, block)?;
    }
    writeln!(f, "}}")
}

fn write_block(
    f: &mut Formatter<'_>,
    module: &Module,
    function: &Function,
    index: usize,
    block: &Block,
) -> fmt::Result {
    write!(f, "^b{index}")?;
    if !block.params.is_empty() {
        f.write_char('(')?;
        for (position, param) in block.params.iter().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{param}: ")?;
            write_value_type(f, module, function, *param)?;
        }
        f.write_char(')')?;
    }
    if let Some(unwind) = &block.unwind {
        write!(f, " unwind {unwind}")?;
    }
    writeln!(f, ":")?;
    for inst in &block.insts {
        f.write_str("    ")?;
        if let Some(result) = inst.result {
            write!(f, "{result}: ")?;
            write_value_type(f, module, function, result)?;
            f.write_str(" = ")?;
        }
        write_inst(f, Some((module, function)), &inst.kind)?;
        writeln!(f)?;
    }
    f.write_str("    ")?;
    write_term(f, Some(module), &block.term.kind)?;
    writeln!(f)
}

fn write_value_type(
    f: &mut Formatter<'_>,
    module: &Module,
    function: &Function,
    value: ValueId,
) -> fmt::Result {
    match function.value_type(value) {
        Some(ty) => write_type(f, Some(module), ty),
        None => f.write_str("<invalid>"),
    }
}

fn write_list(f: &mut Formatter<'_>, values: &[ValueId]) -> fmt::Result {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{value}")?;
    }
    Ok(())
}

/// Строковый литерал OIR: `\\`, `\"`, `\n`, `\r`, `\t`, прочие управляющие — `\u{HEX}`.
fn write_char_literal(f: &mut Formatter<'_>, ch: char) -> fmt::Result {
    f.write_char('\'')?;
    match ch {
        '\\' => f.write_str("\\\\")?,
        '\'' => f.write_str("\\'")?,
        '\n' => f.write_str("\\n")?,
        '\r' => f.write_str("\\r")?,
        '\t' => f.write_str("\\t")?,
        ch if ch.is_control() => write!(f, "\\u{{{:X}}}", u32::from(ch))?,
        ch => f.write_char(ch)?,
    }
    f.write_char('\'')
}

fn write_string(f: &mut Formatter<'_>, text: &str) -> fmt::Result {
    f.write_char('"')?;
    for ch in text.chars() {
        match ch {
            '\\' => f.write_str("\\\\")?,
            '"' => f.write_str("\\\"")?,
            '\n' => f.write_str("\\n")?,
            '\r' => f.write_str("\\r")?,
            '\t' => f.write_str("\\t")?,
            ch if ch.is_control() => write!(f, "\\u{{{:X}}}", u32::from(ch))?,
            ch => f.write_char(ch)?,
        }
    }
    f.write_char('"')
}
