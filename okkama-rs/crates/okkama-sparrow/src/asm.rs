//! Текстовая форма Sparrow (ассемблер): [`disassemble`] печатает программу,
//! [`assemble`] разбирает текст обратно; цикл SPB → ASM → SPB сохраняет
//! программу. Индексы таблиц — позиционные (`t3`, `obj2`, `f5`), номера
//! инструкций (`12:`) проверяются при разборе.
use crate::operand::Operand;
use crate::{
    CapDecl, EnumDecl, FuncRef, Function, Handler, Instruction, Intrinsic, ObjectDecl, ObjectRef, Program,
    Register, RoleDecl, RoleMethod, RoleRef, Target, Type, TypeRef, Witness,
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsmError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sparrow assembly line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for AsmError {}

/// Строка в кавычках с экранированием `\"`, `\\`, `\n`, `\r`, `\t` и `\u{…}`.
pub(crate) fn quote(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn type_ref(out: &mut String, ty: TypeRef) {
    out.push_str(&format!("t{}", ty.0));
}

fn option_type(out: &mut String, ty: Option<TypeRef>) {
    match ty {
        Some(ty) => type_ref(out, ty),
        None => out.push_str("void"),
    }
}

fn type_list(out: &mut String, types: &[TypeRef]) {
    out.push('[');
    for (index, ty) in types.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        type_ref(out, *ty);
    }
    out.push(']');
}

fn print_type(out: &mut String, ty: &Type) {
    match ty {
        Type::Int => out.push_str("int"),
        Type::Bool => out.push_str("bool"),
        Type::Byte => out.push_str("byte"),
        Type::Float => out.push_str("float"),
        Type::Rat => out.push_str("rat"),
        Type::Char => out.push_str("char"),
        Type::Str => out.push_str("str"),
        Type::Object(object) => {
            out.push_str("object ");
            object.print(out);
        }
        Type::Successor(object) => {
            out.push_str("successor ");
            object.print(out);
        }
        Type::Role(role) => {
            out.push_str("role ");
            role.print(out);
        }
        Type::Enum(id) => {
            out.push_str("enum ");
            id.print(out);
        }
        Type::Failure => out.push_str("failure"),
        Type::Array(item) => {
            out.push_str("array ");
            type_ref(out, *item);
        }
        Type::Never => out.push_str("never"),
        Type::Builder(item) => {
            out.push_str("builder ");
            type_ref(out, *item);
        }
        Type::Map(key, value) => {
            out.push_str("map ");
            type_ref(out, *key);
            out.push(' ');
            type_ref(out, *value);
        }
        Type::MapBuilder(key, value) => {
            out.push_str("mbuilder ");
            type_ref(out, *key);
            out.push(' ');
            type_ref(out, *value);
        }
        Type::Region => out.push_str("region"),
        Type::Handle(handle) => out.push_str(&format!("handle {handle}")),
        Type::Any => out.push_str("any"),
        Type::Range => out.push_str("range"),
        Type::Fn { params, result } => {
            out.push_str("fn ");
            type_list(out, params);
            out.push_str(" -> ");
            option_type(out, *result);
        }
        Type::Future(result) => {
            out.push_str("future ");
            option_type(out, *result);
        }
        Type::Cap(cap) => {
            out.push_str("cap ");
            cap.print(out);
        }
    }
}

fn print_function(out: &mut String, index: usize, function: &Function) {
    out.push_str(&format!("function f{index} "));
    quote(out, &function.symbol);
    if function.is_async {
        out.push_str(" async");
    }
    if let Some(native) = &function.native {
        out.push_str(" native ");
        quote(out, native);
    }
    if let Some(intrinsic) = &function.intrinsic {
        out.push_str(" intrinsic ");
        quote(out, &intrinsic.name);
        out.push(' ');
        intrinsic.objects.print(out);
        out.push(' ');
        intrinsic.helpers.print(out);
    }
    out.push_str(" {\n    registers ");
    type_list(out, &function.registers);
    out.push_str("\n    params ");
    function.params.print(out);
    out.push_str("\n    result ");
    option_type(out, function.result);
    out.push('\n');
    for handler in &function.handlers {
        out.push_str(&format!("    handler {} {} ", handler.start, handler.end));
        handler.target.print(out);
        out.push(' ');
        handler.failure.print(out);
        out.push('\n');
    }
    for (pc, instruction) in function.code.iter().enumerate() {
        out.push_str(&format!("    {pc}: {}", instruction.mnemonic()));
        instruction.print_operands(out);
        out.push('\n');
    }
    out.push_str("}\n");
}

/// Текст программы; [`assemble`] даёт ту же программу.
pub fn disassemble(program: &Program) -> String {
    let mut out = format!("sparrow {}\nmodule ", crate::VERSION);
    quote(&mut out, &program.module);
    out.push(' ');
    quote(&mut out, &program.version);
    out.push('\n');
    for (index, ty) in program.types.iter().enumerate() {
        out.push_str(&format!("type t{index} "));
        print_type(&mut out, ty);
        out.push('\n');
    }
    for (index, decl) in program.enums.iter().enumerate() {
        out.push_str(&format!("enum enum{index} "));
        quote(&mut out, &decl.name);
        out.push(' ');
        decl.cases.print(&mut out);
        out.push('\n');
    }
    for (index, role) in program.roles.iter().enumerate() {
        out.push_str(&format!("role role{index} "));
        quote(&mut out, &role.name);
        if role.builtin {
            out.push_str(" builtin");
        }
        out.push_str(" {\n");
        for method in &role.methods {
            out.push_str("    method ");
            quote(&mut out, &method.name);
            out.push(' ');
            type_list(&mut out, &method.params);
            out.push_str(" -> ");
            option_type(&mut out, method.result);
            out.push('\n');
        }
        out.push_str("}\n");
    }
    for (index, object) in program.objects.iter().enumerate() {
        out.push_str(&format!("object obj{index} "));
        quote(&mut out, &object.name);
        if object.builtin {
            out.push_str(" builtin");
        }
        out.push_str(" {\n");
        for (name, ty) in &object.fields {
            out.push_str("    field ");
            quote(&mut out, name);
            out.push(' ');
            type_ref(&mut out, *ty);
            out.push('\n');
        }
        for witness in &object.does {
            out.push_str("    does ");
            witness.role.print(&mut out);
            out.push(' ');
            witness.slots.print(&mut out);
            out.push('\n');
        }
        if let Some(destroy) = object.destroy {
            out.push_str("    destroy ");
            destroy.print(&mut out);
            out.push('\n');
        }
        out.push_str("}\n");
    }
    for handle in &program.handles {
        out.push_str("handle ");
        quote(&mut out, handle);
        out.push('\n');
    }
    for (index, cap) in program.caps.iter().enumerate() {
        out.push_str(&format!("cap cap{index} "));
        quote(&mut out, &cap.name);
        out.push(' ');
        cap.owner.print(&mut out);
        out.push_str(" {\n");
        for (name, function) in &cap.fields {
            out.push_str("    field ");
            quote(&mut out, name);
            out.push(' ');
            function.print(&mut out);
            out.push('\n');
        }
        out.push_str("}\n");
    }
    for (index, function) in program.functions.iter().enumerate() {
        print_function(&mut out, index, function);
    }
    if let Some(entry) = program.entry {
        out.push_str("entry ");
        entry.print(&mut out);
        out.push('\n');
    }
    if let Some(tests) = &program.tests {
        out.push_str("tests {\n");
        for (name, function) in tests {
            out.push_str("    test ");
            quote(&mut out, name);
            out.push(' ');
            function.print(&mut out);
            out.push('\n');
        }
        out.push_str("}\n");
    }
    out
}

// ---- разбор ----

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Number(String),
    Text(String),
    Punct(&'static str),
}

const PUNCTUATION: [&str; 12] = ["<-", "->", "[", "]", "(", ")", "{", "}", ",", ":", "@", "-"];

fn tokenize(source: &str) -> Result<Vec<(Token, usize)>, AsmError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut at = 0;
    let mut line = 1;
    while at < chars.len() {
        let c = chars[at];
        if c == '\n' {
            line += 1;
            at += 1;
            continue;
        }
        if c.is_whitespace() {
            at += 1;
            continue;
        }
        if c == ';' {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
            continue;
        }
        if c == '"' {
            at += 1;
            let mut text = String::new();
            loop {
                let Some(&next) = chars.get(at) else {
                    return Err(AsmError { line, message: "unterminated string".into() });
                };
                at += 1;
                match next {
                    '"' => break,
                    '\\' => {
                        let Some(&escape) = chars.get(at) else {
                            return Err(AsmError { line, message: "unterminated escape".into() });
                        };
                        at += 1;
                        match escape {
                            '"' => text.push('"'),
                            '\\' => text.push('\\'),
                            'n' => text.push('\n'),
                            'r' => text.push('\r'),
                            't' => text.push('\t'),
                            'u' => {
                                if chars.get(at) != Some(&'{') {
                                    return Err(AsmError { line, message: "expected `{` after \\u".into() });
                                }
                                at += 1;
                                let start = at;
                                while at < chars.len() && chars[at] != '}' {
                                    at += 1;
                                }
                                let digits: String = chars[start..at].iter().collect();
                                at += 1;
                                let point = u32::from_str_radix(&digits, 16)
                                    .ok()
                                    .and_then(char::from_u32)
                                    .ok_or_else(|| AsmError { line, message: format!("invalid escape \\u{{{digits}}}") })?;
                                text.push(point);
                            }
                            other => return Err(AsmError { line, message: format!("unknown escape \\{other}") }),
                        }
                    }
                    '\n' => return Err(AsmError { line, message: "newline in string".into() }),
                    other => text.push(other),
                }
            }
            tokens.push((Token::Text(text), line));
            continue;
        }
        if c.is_ascii_digit() {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric()) {
                at += 1;
            }
            tokens.push((Token::Number(chars[start..at].iter().collect()), line));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = at;
            while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_' || chars[at] == '.') {
                at += 1;
            }
            tokens.push((Token::Ident(chars[start..at].iter().collect()), line));
            continue;
        }
        let rest: String = chars[at..chars.len().min(at + 2)].iter().collect();
        let Some(punct) = PUNCTUATION.iter().find(|punct| rest.starts_with(**punct)) else {
            return Err(AsmError { line, message: format!("unexpected character `{c}`") });
        };
        at += punct.chars().count();
        tokens.push((Token::Punct(punct), line));
    }
    Ok(tokens)
}

#[doc(hidden)]
#[derive(Debug)]
pub struct Parser<'a> {
    tokens: &'a [(Token, usize)],
    at: usize,
}

impl Parser<'_> {
    fn line(&self) -> usize {
        self.tokens
            .get(self.at)
            .or_else(|| self.tokens.last())
            .map_or(1, |(_, line)| *line)
    }

    pub(crate) fn error(&self, message: impl Into<String>) -> AsmError {
        AsmError {
            line: self.line(),
            message: message.into(),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|(token, _)| token)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.peek().cloned();
        self.at += 1;
        token
    }

    pub(crate) fn ident(&mut self) -> Result<String, AsmError> {
        match self.next() {
            Some(Token::Ident(name)) => Ok(name),
            _ => {
                self.at -= 1;
                Err(self.error("expected a name"))
            }
        }
    }

    pub(crate) fn eat_ident(&mut self, name: &str) -> bool {
        if matches!(self.peek(), Some(Token::Ident(found)) if found == name) {
            self.at += 1;
            return true;
        }
        false
    }

    fn keyword(&mut self, name: &str) -> Result<(), AsmError> {
        if self.eat_ident(name) {
            return Ok(());
        }
        Err(self.error(format!("expected `{name}`")))
    }

    pub(crate) fn punct(&mut self, punct: &str) -> Result<(), AsmError> {
        if self.eat_punct(punct) {
            return Ok(());
        }
        Err(self.error(format!("expected `{punct}`")))
    }

    pub(crate) fn eat_punct(&mut self, punct: &str) -> bool {
        if matches!(self.peek(), Some(Token::Punct(found)) if *found == punct) {
            self.at += 1;
            return true;
        }
        false
    }

    pub(crate) fn comma(&mut self) -> Result<(), AsmError> {
        self.punct(",")
    }

    fn number_text(&mut self) -> Result<String, AsmError> {
        match self.next() {
            Some(Token::Number(text)) => Ok(text),
            _ => {
                self.at -= 1;
                Err(self.error("expected a number"))
            }
        }
    }

    pub(crate) fn u64(&mut self) -> Result<u64, AsmError> {
        let text = self.number_text()?;
        let parsed = match text.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16),
            None => text.parse(),
        };
        parsed.map_err(|_| self.error(format!("invalid number `{text}`")))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, AsmError> {
        let value = self.u64()?;
        u32::try_from(value).map_err(|_| self.error("number exceeds u32"))
    }

    /// Десятичное целое со знаком (текст константы `int`).
    pub(crate) fn integer_text(&mut self) -> Result<String, AsmError> {
        let negative = self.eat_punct("-");
        let digits = self.number_text()?;
        Ok(if negative { format!("-{digits}") } else { digits })
    }

    pub(crate) fn string(&mut self) -> Result<String, AsmError> {
        match self.next() {
            Some(Token::Text(text)) => Ok(text),
            _ => {
                self.at -= 1;
                Err(self.error("expected a string"))
            }
        }
    }

    /// Имя вида `<prefix><число>` (`r3`, `f12`, `obj0`).
    pub(crate) fn prefixed(&mut self, prefix: &str) -> Result<u32, AsmError> {
        let name = self.ident()?;
        name.strip_prefix(prefix)
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|digits| digits.parse().ok())
            .ok_or_else(|| {
                self.at -= 1;
                self.error(format!("expected `{prefix}<number>`, found `{name}`"))
            })
    }

    fn type_ref(&mut self) -> Result<TypeRef, AsmError> {
        Ok(TypeRef(self.prefixed("t")?))
    }

    fn option_type(&mut self) -> Result<Option<TypeRef>, AsmError> {
        if self.eat_ident("void") {
            return Ok(None);
        }
        Ok(Some(self.type_ref()?))
    }

    fn type_list(&mut self) -> Result<Vec<TypeRef>, AsmError> {
        self.punct("[")?;
        let mut types = Vec::new();
        if self.eat_punct("]") {
            return Ok(types);
        }
        loop {
            types.push(self.type_ref()?);
            if self.eat_punct("]") {
                return Ok(types);
            }
            self.comma()?;
        }
    }

    fn index(&mut self, prefix: &str, expected: usize) -> Result<(), AsmError> {
        let found = self.prefixed(prefix)?;
        if found as usize != expected {
            return Err(self.error(format!("expected {prefix}{expected}, found {prefix}{found}")));
        }
        Ok(())
    }

    fn parse_type(&mut self) -> Result<Type, AsmError> {
        let kind = self.ident()?;
        Ok(match kind.as_str() {
            "int" => Type::Int,
            "bool" => Type::Bool,
            "byte" => Type::Byte,
            "float" => Type::Float,
            "rat" => Type::Rat,
            "char" => Type::Char,
            "str" => Type::Str,
            "object" => Type::Object(ObjectRef::parse(self)?),
            "successor" => Type::Successor(ObjectRef::parse(self)?),
            "role" => Type::Role(RoleRef::parse(self)?),
            "enum" => Type::Enum(crate::EnumRef::parse(self)?),
            "failure" => Type::Failure,
            "array" => Type::Array(self.type_ref()?),
            "never" => Type::Never,
            "builder" => Type::Builder(self.type_ref()?),
            "map" => Type::Map(self.type_ref()?, self.type_ref()?),
            "mbuilder" => Type::MapBuilder(self.type_ref()?, self.type_ref()?),
            "region" => Type::Region,
            "handle" => Type::Handle(self.u32()?),
            "any" => Type::Any,
            "range" => Type::Range,
            "fn" => {
                let params = self.type_list()?;
                self.punct("->")?;
                Type::Fn {
                    params,
                    result: self.option_type()?,
                }
            }
            "future" => Type::Future(self.option_type()?),
            "cap" => Type::Cap(crate::CapRef::parse(self)?),
            other => return Err(self.error(format!("unknown type `{other}`"))),
        })
    }

    fn function(&mut self) -> Result<Function, AsmError> {
        let symbol = self.string()?;
        let is_async = self.eat_ident("async");
        let native = if self.eat_ident("native") { Some(self.string()?) } else { None };
        let intrinsic = if self.eat_ident("intrinsic") {
            Some(Intrinsic {
                name: self.string()?,
                objects: Vec::<ObjectRef>::parse(self)?,
                helpers: Vec::<FuncRef>::parse(self)?,
            })
        } else {
            None
        };
        self.punct("{")?;
        self.keyword("registers")?;
        let registers = self.type_list()?;
        self.keyword("params")?;
        let params = Vec::<Register>::parse(self)?;
        self.keyword("result")?;
        let result = self.option_type()?;
        let mut handlers = Vec::new();
        while self.eat_ident("handler") {
            handlers.push(Handler {
                start: self.u32()?,
                end: self.u32()?,
                target: Target::parse(self)?,
                failure: Register::parse(self)?,
            });
        }
        let mut code = Vec::new();
        while !self.eat_punct("}") {
            let pc = self.u32()?;
            if pc as usize != code.len() {
                return Err(self.error(format!("expected instruction {}, found {pc}", code.len())));
            }
            self.punct(":")?;
            let mnemonic = self.ident()?;
            code.push(Instruction::parse_operands(&mnemonic, self)?);
        }
        Ok(Function {
            symbol,
            is_async,
            native,
            intrinsic,
            registers,
            params,
            result,
            code,
            handlers,
        })
    }

    fn program(&mut self) -> Result<Program, AsmError> {
        self.keyword("sparrow")?;
        let version = self.u32()?;
        if version != u32::from(crate::VERSION) {
            return Err(self.error(format!("unsupported Sparrow assembly version {version}")));
        }
        self.keyword("module")?;
        let mut program = Program {
            module: self.string()?,
            version: self.string()?,
            ..Program::default()
        };
        while let Some(token) = self.peek().cloned() {
            let Token::Ident(keyword) = token else {
                return Err(self.error("expected a declaration"));
            };
            self.at += 1;
            match keyword.as_str() {
                "type" => {
                    self.index("t", program.types.len())?;
                    let ty = self.parse_type()?;
                    program.types.push(ty);
                }
                "enum" => {
                    self.index("enum", program.enums.len())?;
                    program.enums.push(EnumDecl {
                        name: self.string()?,
                        cases: Vec::<String>::parse(self)?,
                    });
                }
                "role" => {
                    self.index("role", program.roles.len())?;
                    let name = self.string()?;
                    let builtin = self.eat_ident("builtin");
                    self.punct("{")?;
                    let mut methods = Vec::new();
                    while self.eat_ident("method") {
                        let name = self.string()?;
                        let params = self.type_list()?;
                        self.punct("->")?;
                        methods.push(RoleMethod {
                            name,
                            params,
                            result: self.option_type()?,
                        });
                    }
                    self.punct("}")?;
                    program.roles.push(RoleDecl { name, builtin, methods });
                }
                "object" => {
                    self.index("obj", program.objects.len())?;
                    let name = self.string()?;
                    let builtin = self.eat_ident("builtin");
                    self.punct("{")?;
                    let mut object = ObjectDecl {
                        name,
                        fields: Vec::new(),
                        does: Vec::new(),
                        destroy: None,
                        builtin,
                    };
                    loop {
                        if self.eat_ident("field") {
                            let name = self.string()?;
                            object.fields.push((name, self.type_ref()?));
                        } else if self.eat_ident("does") {
                            object.does.push(Witness {
                                role: RoleRef::parse(self)?,
                                slots: Vec::<FuncRef>::parse(self)?,
                            });
                        } else if self.eat_ident("destroy") {
                            object.destroy = Some(FuncRef::parse(self)?);
                        } else {
                            break;
                        }
                    }
                    self.punct("}")?;
                    program.objects.push(object);
                }
                "handle" => {
                    let name = self.string()?;
                    program.handles.push(name);
                }
                "cap" => {
                    self.index("cap", program.caps.len())?;
                    let name = self.string()?;
                    let owner = ObjectRef::parse(self)?;
                    self.punct("{")?;
                    let mut fields = Vec::new();
                    while self.eat_ident("field") {
                        let name = self.string()?;
                        fields.push((name, FuncRef::parse(self)?));
                    }
                    self.punct("}")?;
                    program.caps.push(CapDecl { name, owner, fields });
                }
                "function" => {
                    self.index("f", program.functions.len())?;
                    let function = self.function()?;
                    program.functions.push(function);
                }
                "entry" => program.entry = Some(FuncRef::parse(self)?),
                "tests" => {
                    self.punct("{")?;
                    let mut tests = Vec::new();
                    while self.eat_ident("test") {
                        let name = self.string()?;
                        tests.push((name, FuncRef::parse(self)?));
                    }
                    self.punct("}")?;
                    program.tests = Some(tests);
                }
                other => {
                    self.at -= 1;
                    return Err(self.error(format!("unknown declaration `{other}`")));
                }
            }
        }
        Ok(program)
    }
}

/// Разобрать текст ассемблера и проверить программу.
pub fn assemble(source: &str) -> Result<Program, AsmError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens: &tokens, at: 0 };
    let program = parser.program()?;
    program.verify().map_err(|error| AsmError {
        line: 0,
        message: error.to_string(),
    })?;
    Ok(program)
}
