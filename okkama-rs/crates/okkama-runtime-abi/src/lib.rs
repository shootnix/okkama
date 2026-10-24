//! Native ABI metadata only: no compiler, VM, runtime values, or host I/O.

/// Тип ABI: тип Okkama параметра или результата provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Rat,
    Bool,
    Byte,
    Char,
    Str,
    Any,
    Void,
    /// Непрозрачное значение provider (`native object`) с этим именем.
    Handle(&'static str),
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
}

impl Type {
    /// `@byte`.
    pub fn bytes() -> Self {
        Self::Array(Box::new(Self::Byte))
    }

    fn parse(text: &'static str) -> (Self, &'static str) {
        let text = text.trim_start();
        if let Some(rest) = text.strip_prefix('@') {
            let (item, rest) = Self::parse(rest);
            return (Self::Array(Box::new(item)), rest);
        }
        if let Some(rest) = text.strip_prefix('%') {
            let (key, rest) = Self::parse(rest);
            let rest = rest.strip_prefix(':').expect("map type has `:`");
            let (value, rest) = Self::parse(rest);
            return (Self::Map(Box::new(key), Box::new(value)), rest);
        }
        let end = text
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .unwrap_or(text.len());
        let (name, rest) = text.split_at(end);
        let ty = match name {
            "int" => Self::Int,
            "float" => Self::Float,
            "rat" => Self::Rat,
            "bool" => Self::Bool,
            "byte" => Self::Byte,
            "char" => Self::Char,
            "string" => Self::Str,
            "any" => Self::Any,
            "void" => Self::Void,
            handle => Self::Handle(handle),
        };
        (ty, rest)
    }
}

/// Сигнатура provider: параметры, результат; `async` — результат Future.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub is_async: bool,
    pub params: Vec<Type>,
    pub result: Type,
}

impl Signature {
    fn parse(text: &'static str) -> Self {
        let (is_async, text) = match text.strip_prefix("async ") {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (params, result) = text
            .strip_prefix('(')
            .and_then(|rest| rest.split_once(") -> "))
            .expect("signature is `(params) -> result`");
        let params = if params.trim().is_empty() {
            Vec::new()
        } else {
            let mut list = Vec::new();
            let mut rest = params;
            loop {
                let (ty, tail) = Type::parse(rest);
                list.push(ty);
                match tail.trim_start().strip_prefix(',') {
                    Some(tail) => rest = tail,
                    None => break,
                }
            }
            list
        };
        Self {
            is_async,
            params,
            result: Type::parse(result).0,
        }
    }
}

/// Таблица native: имя, стабильный wire ID, символ, сигнатура Okkama.
macro_rules! natives {
    ($($name:ident = $id:literal, $symbol:literal, $signature:literal;)*) => {
        /// Stable wire IDs. Never renumber an existing provider operation.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u32)]
        pub enum Native {
            $($name = $id,)*
        }

        impl Native {
            pub const ALL: &'static [Self] = &[$(Self::$name,)*];

            pub fn symbol(self) -> &'static str {
                match self {
                    $(Self::$name => $symbol,)*
                }
            }

            fn signature_text(self) -> &'static str {
                match self {
                    $(Self::$name => $signature,)*
                }
            }
        }
    };
}

include!("table.rs");

impl Native {
    /// Сигнатура provider (разбирается один раз).
    pub fn signature(self) -> &'static Signature {
        static SIGNATURES: std::sync::OnceLock<std::collections::HashMap<Native, Signature>> =
            std::sync::OnceLock::new();
        &SIGNATURES.get_or_init(|| {
            Self::ALL
                .iter()
                .map(|native| (*native, Signature::parse(native.signature_text())))
                .collect()
        })[&self]
    }

    pub fn parameters(self) -> &'static [Type] {
        &self.signature().params
    }

    pub fn result(self) -> &'static Type {
        &self.signature().result
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|native| *native as u32 == id)
    }

    pub fn resolve(symbol: &str) -> Option<Self> {
        static SYMBOLS: std::sync::OnceLock<std::collections::HashMap<&'static str, Native>> =
            std::sync::OnceLock::new();
        SYMBOLS
            .get_or_init(|| Self::ALL.iter().map(|n| (n.symbol(), *n)).collect())
            .get(symbol)
            .copied()
    }
}

/// Provider failure: fields of the language's NativeFailure object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeFailure {
    pub domain: String,
    pub code: String,
    pub operation: String,
    pub message: String,
}
impl std::fmt::Display for NativeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NativeFailure: {}", self.message)
    }
}
impl std::error::Error for NativeFailure {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_ids_and_symbols_are_unique() {
        let mut ids: Vec<u32> = Native::ALL.iter().map(|native| *native as u32).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Native::ALL.len());
        let mut symbols: Vec<&str> = Native::ALL.iter().map(|native| native.symbol()).collect();
        symbols.sort_unstable();
        symbols.dedup();
        assert_eq!(symbols.len(), Native::ALL.len());
        for native in Native::ALL {
            let arity: usize = native.symbol().rsplit('/').next().unwrap().parse().unwrap();
            assert_eq!(arity, native.parameters().len(), "{}", native.symbol());
            assert_eq!(Native::resolve(native.symbol()), Some(*native));
        }
    }
}
