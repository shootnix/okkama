//! Операнды инструкций: двоичная и текстовая форма и обход для проверки.
use crate::asm::Parser;
use crate::codec::{Reader, Writer};
use crate::{
    AsmError, BitOp, CapRef, CatchKind, Cmp, CodecError, Constant, Dst, EnumRef, FuncRef, FutOp,
    ObjectRef, Register, RoleRef, Target, TrapCode,
};

/// Что видит обход операнда.
#[derive(Clone, Copy, Debug)]
pub enum Access<'a> {
    Use(Register),
    Def(Register),
    Target(&'a Target),
    Function(FuncRef),
    Object(ObjectRef),
    Role(RoleRef),
    Enum(EnumRef),
    Cap(CapRef),
}

pub trait Operand: Sized {
    #[doc(hidden)]
    fn encode(&self, writer: &mut Writer);
    #[doc(hidden)]
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError>;
    #[doc(hidden)]
    fn print(&self, out: &mut String);
    #[doc(hidden)]
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError>;
    fn visit(&self, _visitor: &mut dyn FnMut(Access<'_>)) {}
}

impl Operand for Register {
    fn encode(&self, writer: &mut Writer) {
        writer.number(u64::from(self.0));
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Register(reader.u32()?))
    }
    fn print(&self, out: &mut String) {
        out.push_str(&format!("r{}", self.0));
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        Ok(Register(parser.prefixed("r")?))
    }
    fn visit(&self, visitor: &mut dyn FnMut(Access<'_>)) {
        visitor(Access::Use(*self));
    }
}

impl Operand for Dst {
    fn encode(&self, writer: &mut Writer) {
        self.0.encode(writer);
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Dst(Register::decode(reader)?))
    }
    fn print(&self, out: &mut String) {
        self.0.print(out);
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        Ok(Dst(Register::parse(parser)?))
    }
    fn visit(&self, visitor: &mut dyn FnMut(Access<'_>)) {
        visitor(Access::Def(self.0));
    }
}

macro_rules! table_ref {
    ($ty:ident, $prefix:literal, $access:ident) => {
        impl Operand for $ty {
            fn encode(&self, writer: &mut Writer) {
                writer.number(u64::from(self.0));
            }
            fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
                Ok($ty(reader.u32()?))
            }
            fn print(&self, out: &mut String) {
                out.push_str(&format!(concat!($prefix, "{}"), self.0));
            }
            fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
                Ok($ty(parser.prefixed($prefix)?))
            }
            fn visit(&self, visitor: &mut dyn FnMut(Access<'_>)) {
                visitor(Access::$access(*self));
            }
        }
    };
}

table_ref!(FuncRef, "f", Function);
table_ref!(ObjectRef, "obj", Object);
table_ref!(RoleRef, "role", Role);
table_ref!(EnumRef, "enum", Enum);
table_ref!(CapRef, "cap", Cap);

impl<T: Operand> Operand for Option<T> {
    fn encode(&self, writer: &mut Writer) {
        match self {
            None => writer.byte(0),
            Some(value) => {
                writer.byte(1);
                value.encode(writer);
            }
        }
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        match reader.byte()? {
            0 => Ok(None),
            1 => Ok(Some(T::decode(reader)?)),
            other => Err(reader.error(format!("invalid option tag {other}"))),
        }
    }
    fn print(&self, out: &mut String) {
        match self {
            None => out.push('_'),
            Some(value) => value.print(out),
        }
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        if parser.eat_ident("_") {
            return Ok(None);
        }
        Ok(Some(T::parse(parser)?))
    }
    fn visit(&self, visitor: &mut dyn FnMut(Access<'_>)) {
        if let Some(value) = self {
            value.visit(visitor);
        }
    }
}

impl<T: Operand> Operand for Vec<T> {
    fn encode(&self, writer: &mut Writer) {
        writer.number(self.len() as u64);
        for item in self {
            item.encode(writer);
        }
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        let count = reader.count()?;
        (0..count).map(|_| T::decode(reader)).collect()
    }
    fn print(&self, out: &mut String) {
        out.push('[');
        for (index, item) in self.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            item.print(out);
        }
        out.push(']');
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        parser.punct("[")?;
        let mut items = Vec::new();
        if parser.eat_punct("]") {
            return Ok(items);
        }
        loop {
            items.push(T::parse(parser)?);
            if parser.eat_punct("]") {
                return Ok(items);
            }
            parser.comma()?;
        }
    }
    fn visit(&self, visitor: &mut dyn FnMut(Access<'_>)) {
        for item in self {
            item.visit(visitor);
        }
    }
}

impl<A: Operand, B: Operand> Operand for (A, B) {
    fn encode(&self, writer: &mut Writer) {
        self.0.encode(writer);
        self.1.encode(writer);
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok((A::decode(reader)?, B::decode(reader)?))
    }
    fn print(&self, out: &mut String) {
        self.0.print(out);
        out.push_str(": ");
        self.1.print(out);
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        let first = A::parse(parser)?;
        parser.punct(":")?;
        Ok((first, B::parse(parser)?))
    }
    fn visit(&self, visitor: &mut dyn FnMut(Access<'_>)) {
        self.0.visit(visitor);
        self.1.visit(visitor);
    }
}

impl Operand for u32 {
    fn encode(&self, writer: &mut Writer) {
        writer.number(u64::from(*self));
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        reader.u32()
    }
    fn print(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        parser.u32()
    }
}

impl Operand for bool {
    fn encode(&self, writer: &mut Writer) {
        writer.byte(u8::from(*self));
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        match reader.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(reader.error(format!("invalid bool {other}"))),
        }
    }
    fn print(&self, out: &mut String) {
        out.push_str(if *self { "true" } else { "false" });
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        if parser.eat_ident("true") {
            return Ok(true);
        }
        if parser.eat_ident("false") {
            return Ok(false);
        }
        Err(parser.error("expected true or false"))
    }
}

impl Operand for String {
    fn encode(&self, writer: &mut Writer) {
        writer.text(self);
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        reader.text()
    }
    fn print(&self, out: &mut String) {
        crate::asm::quote(out, self);
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        parser.string()
    }
}

impl Operand for Target {
    fn encode(&self, writer: &mut Writer) {
        writer.number(u64::from(self.pc));
        writer.number(self.moves.len() as u64);
        for (dst, source) in &self.moves {
            dst.encode(writer);
            source.encode(writer);
        }
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        let pc = reader.u32()?;
        let count = reader.count()?;
        let moves = (0..count)
            .map(|_| Ok((Register::decode(reader)?, Register::decode(reader)?)))
            .collect::<Result<_, CodecError>>()?;
        Ok(Target { pc, moves })
    }
    fn print(&self, out: &mut String) {
        out.push_str(&format!("@{}", self.pc));
        if !self.moves.is_empty() {
            out.push('(');
            for (index, (dst, source)) in self.moves.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                dst.print(out);
                out.push_str(" <- ");
                source.print(out);
            }
            out.push(')');
        }
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        parser.punct("@")?;
        let pc = parser.u32()?;
        let mut moves = Vec::new();
        if parser.eat_punct("(") {
            loop {
                let dst = Register::parse(parser)?;
                parser.punct("<-")?;
                moves.push((dst, Register::parse(parser)?));
                if parser.eat_punct(")") {
                    break;
                }
                parser.comma()?;
            }
        }
        Ok(Target { pc, moves })
    }
    fn visit(&self, visitor: &mut dyn FnMut(Access<'_>)) {
        visitor(Access::Target(self));
    }
}

impl Operand for Constant {
    fn encode(&self, writer: &mut Writer) {
        match self {
            Constant::Int(text) => {
                writer.byte(0);
                writer.text(text);
            }
            Constant::Bool(value) => {
                writer.byte(1);
                writer.byte(u8::from(*value));
            }
            Constant::Byte(value) => {
                writer.byte(2);
                writer.byte(*value);
            }
            Constant::Float(bits) => {
                writer.byte(3);
                writer.bytes(&bits.to_be_bytes());
            }
            Constant::Char(value) => {
                writer.byte(4);
                writer.number(u64::from(u32::from(*value)));
            }
            Constant::Str(text) => {
                writer.byte(5);
                writer.text(text);
            }
        }
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(match reader.byte()? {
            0 => {
                let text = reader.text()?;
                if !canonical_int(&text) {
                    return Err(reader.error("non-canonical integer constant"));
                }
                Constant::Int(text)
            }
            1 => Constant::Bool(bool::decode(reader)?),
            2 => Constant::Byte(reader.byte()?),
            3 => Constant::Float(u64::from_be_bytes(reader.take(8)?.try_into().expect("eight"))),
            4 => {
                let point = reader.u32()?;
                Constant::Char(char::from_u32(point).ok_or_else(|| reader.error("invalid char constant"))?)
            }
            5 => Constant::Str(reader.text()?),
            other => return Err(reader.error(format!("invalid constant tag {other}"))),
        })
    }
    fn print(&self, out: &mut String) {
        match self {
            Constant::Int(text) => out.push_str(&format!("int {text}")),
            Constant::Bool(value) => out.push_str(&format!("bool {value}")),
            Constant::Byte(value) => out.push_str(&format!("byte {value}")),
            Constant::Float(bits) => out.push_str(&format!("float {bits:#018x}")),
            Constant::Char(value) => out.push_str(&format!("char {}", u32::from(*value))),
            Constant::Str(text) => {
                out.push_str("str ");
                crate::asm::quote(out, text);
            }
        }
    }
    fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
        let kind = parser.ident()?;
        Ok(match kind.as_str() {
            "int" => {
                let text = parser.integer_text()?;
                if !canonical_int(&text) {
                    return Err(parser.error("non-canonical integer constant"));
                }
                Constant::Int(text)
            }
            "bool" => Constant::Bool(bool::parse(parser)?),
            "byte" => {
                let value = parser.u32()?;
                Constant::Byte(u8::try_from(value).map_err(|_| parser.error("byte constant exceeds 255"))?)
            }
            "float" => Constant::Float(parser.u64()?),
            "char" => {
                let point = parser.u32()?;
                Constant::Char(char::from_u32(point).ok_or_else(|| parser.error("invalid char constant"))?)
            }
            "str" => Constant::Str(parser.string()?),
            other => return Err(parser.error(format!("unknown constant kind `{other}`"))),
        })
    }
}

/// Каноническая десятичная запись целого.
pub(crate) fn canonical_int(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && text != "-0"
}

macro_rules! op_operand {
    ($ty:ident) => {
        impl Operand for $ty {
            fn encode(&self, writer: &mut Writer) {
                writer.byte(self.code());
            }
            fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
                let code = reader.byte()?;
                $ty::from_code(code).ok_or_else(|| reader.error(format!(concat!("invalid ", stringify!($ty), " {}"), code)))
            }
            fn print(&self, out: &mut String) {
                out.push_str(self.name());
            }
            fn parse(parser: &mut Parser<'_>) -> Result<Self, AsmError> {
                let name = parser.ident()?;
                $ty::from_name(&name).ok_or_else(|| parser.error(format!(concat!("unknown ", stringify!($ty), " `{}`"), name)))
            }
        }
    };
}

op_operand!(Cmp);
op_operand!(BitOp);
op_operand!(FutOp);
op_operand!(CatchKind);
op_operand!(TrapCode);
