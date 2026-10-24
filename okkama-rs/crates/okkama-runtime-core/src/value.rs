use crate::IntValue;
use std::{fmt, rc::Rc};

#[derive(Clone, Debug)]
#[repr(C, u8)]
pub enum Value {
    Range(Rc<crate::range::Range>),
    /// Runtime-owned immutable NativeFailure, including when viewed as Error.
    NativeFailure(Rc<crate::abi::NativeFailure>),
    Failure(Rc<crate::failure::Failure>),
    /// Заглушка перед первой итерацией; может передаваться, но не вычисляться.
    Undef,
    Int(IntValue),
    /// Конечное binary64.
    Float(f64),
    /// Несократимая дробь; копирование не копирует цифры.
    Rat(Rc<crate::rat::Rat>),
    Bool(bool),
    Byte(u8),
    Char(char),
    /// Nominal enum identity and case ordinal within the current program.
    Enum {
        declaration: usize,
        case: usize,
    },
    /// Неизменяемый текст: передача через кадры и поля не копирует байты.
    Str(Rc<str>),
    /// Неизменяемый объект; копирование сохраняет идентичность.
    Object(Rc<crate::heap::Object>),
    /// Неизменяемый массив.
    Array(Rc<crate::heap::Array>),
    /// Неизменяемый словарь в порядке ключей.
    Map(Rc<crate::heap::Map>),
    /// Служебный линейный сборщик массива; программе не виден.
    Builder(Rc<crate::heap::ArrayBuilder>),
    /// Служебный линейный сборщик словаря.
    MapBuilder(Rc<crate::heap::MapBuilder>),
    /// Регион владения вызова (служебное значение `region`).
    Region(Rc<crate::heap::Region>),
    /// Значение-функция: замыкание или связанный action; равенство — по
    /// идентичности, как у `okkama.rt.Fn` JVM.
    Fn(Rc<crate::heap::Closure>),
    /// Непрозрачный box `any`: сравнение без сужения — ошибка программы.
    Any(Rc<Value>),
    /// Future (OIR-1 §13); равенство — по идентичности.
    Future(crate::sched::Future),
    /// Значение `native object` provider (например, `DigestState`):
    /// неизменяемое, копирование сохраняет идентичность.
    Handle(Rc<dyn std::any::Any>),
}

impl Value {
    pub fn as_range(&self) -> Result<&crate::range::Range, ValueError> {
        match self {
            Self::Range(range) => Ok(range),
            _ => Err(invalid_state("expected range value")),
        }
    }
    pub fn enum_eq(&self, other: &Self) -> Result<bool, ValueError> {
        match (self, other) {
            (
                Self::Enum {
                    declaration: left,
                    case: a,
                },
                Self::Enum {
                    declaration: right,
                    case: b,
                },
            ) if left == right => Ok(a == b),
            _ => Err(invalid_state("expected values of the same enum")),
        }
    }

    pub fn as_byte(&self) -> Result<u8, ValueError> {
        match self {
            Self::Byte(value) => Ok(*value),
            _ => Err(invalid_state("expected byte value")),
        }
    }

    pub fn as_char(&self) -> Result<char, ValueError> {
        match self {
            Self::Char(value) => Ok(*value),
            _ => Err(invalid_state("expected char value")),
        }
    }
    pub fn as_string(&self) -> Result<&str, ValueError> {
        match self {
            Self::Str(value) => Ok(value),
            _ => Err(invalid_state("expected string value")),
        }
    }

    pub fn as_object(&self) -> Result<&Rc<crate::heap::Object>, ValueError> {
        match self {
            Self::Object(object) => Ok(object),
            _ => Err(invalid_state("expected object value")),
        }
    }

    pub fn as_int(&self) -> Result<&IntValue, ValueError> {
        match self {
            Self::Int(value) => Ok(value),
            _ => Err(invalid_state("expected integer value")),
        }
    }

    pub fn as_float(&self) -> Result<f64, ValueError> {
        match self {
            Self::Float(value) => Ok(*value),
            _ => Err(invalid_state("expected float value")),
        }
    }

    pub fn as_rat(&self) -> Result<&crate::rat::Rat, ValueError> {
        match self {
            Self::Rat(value) => Ok(value),
            _ => Err(invalid_state("expected rat value")),
        }
    }

    pub fn as_bool(&self) -> Result<bool, ValueError> {
        match self {
            Self::Bool(value) => Ok(*value),
            _ => Err(invalid_state("expected boolean value")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueError {
    pub message: String,
}
fn invalid_state(message: impl Into<String>) -> ValueError {
    ValueError {
        message: message.into(),
    }
}
impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid interpreter state: {}", self.message)
    }
}
impl std::error::Error for ValueError {}
