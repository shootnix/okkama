//! SPB 9: магия `SPRW`, версия u16 big-endian, затем таблицы и функции.
//! Числа — беззнаковый LEB128, строки — длина и UTF-8, инструкции — код
//! операции и операнды в порядке полей; никакой сериализации layout Rust.
use crate::operand::Operand;
use crate::{
    CapDecl, CapRef, EnumDecl, EnumRef, FuncRef, Function, Handler, Instruction, Intrinsic, ObjectDecl,
    ObjectRef, Program, Register, RoleDecl, RoleMethod, RoleRef, Target, Type, TypeRef, Witness,
};
use std::fmt;

/// Предел размера файла; проверяется до разбора.
pub const MAX_FILE_BYTES: usize = 256 * 1024 * 1024;
/// Версия формата.
pub const VERSION: u16 = 9;
const MAGIC: &[u8; 4] = b"SPRW";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodecError {
    /// Позиция в файле, если ошибка обнаружена при декодировании поля.
    pub offset: Option<usize>,
    pub message: String,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.offset {
            Some(offset) => write!(f, "SPB byte {offset}: {}", self.message),
            None => write!(f, "SPB: {}", self.message),
        }
    }
}

impl std::error::Error for CodecError {}

#[doc(hidden)]
#[derive(Debug, Default)]
pub struct Writer {
    pub(crate) bytes: Vec<u8>,
}

impl Writer {
    pub(crate) fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    pub(crate) fn number(&mut self, mut value: u64) {
        loop {
            let low = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.bytes.push(low);
                return;
            }
            self.bytes.push(low | 0x80);
        }
    }

    pub(crate) fn text(&mut self, value: &str) {
        self.number(value.len() as u64);
        self.bytes(value.as_bytes());
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn error(&self, message: impl Into<String>) -> CodecError {
        CodecError {
            offset: Some(self.at),
            message: message.into(),
        }
    }

    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8], CodecError> {
        let end = self
            .at
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| self.error("unexpected end of file"))?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn number(&mut self) -> Result<u64, CodecError> {
        let mut value: u64 = 0;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            let part = u64::from(byte & 0x7f);
            if shift == 63 && part > 1 {
                return Err(self.error("number exceeds u64"));
            }
            value |= part << shift;
            if byte & 0x80 == 0 {
                // Каноническая запись: без лишних нулевых групп.
                if byte == 0 && shift > 0 {
                    return Err(self.error("non-canonical number"));
                }
                return Ok(value);
            }
        }
        Err(self.error("number exceeds u64"))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, CodecError> {
        let value = self.number()?;
        u32::try_from(value).map_err(|_| self.error("number exceeds u32"))
    }

    /// Число элементов: не больше оставшихся байтов (каждый — хотя бы байт).
    pub(crate) fn count(&mut self) -> Result<usize, CodecError> {
        let count = self.number()?;
        if count > (self.bytes.len() - self.at) as u64 {
            return Err(self.error("count exceeds remaining bytes"));
        }
        Ok(count as usize)
    }

    pub(crate) fn text(&mut self) -> Result<String, CodecError> {
        let size = self.count()?;
        let bytes = self.take(size)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| self.error("invalid UTF-8 text"))
    }
}

fn type_ref(writer: &mut Writer, ty: TypeRef) {
    writer.number(u64::from(ty.0));
}

fn read_type_ref(reader: &mut Reader<'_>) -> Result<TypeRef, CodecError> {
    Ok(TypeRef(reader.u32()?))
}

fn encode_type(writer: &mut Writer, ty: &Type) {
    match ty {
        Type::Int => writer.byte(0),
        Type::Bool => writer.byte(1),
        Type::Byte => writer.byte(2),
        Type::Float => writer.byte(3),
        Type::Rat => writer.byte(4),
        Type::Char => writer.byte(5),
        Type::Str => writer.byte(6),
        Type::Object(object) => {
            writer.byte(7);
            object.encode(writer);
        }
        Type::Successor(object) => {
            writer.byte(8);
            object.encode(writer);
        }
        Type::Role(role) => {
            writer.byte(9);
            role.encode(writer);
        }
        Type::Enum(id) => {
            writer.byte(10);
            id.encode(writer);
        }
        Type::Failure => writer.byte(11),
        Type::Array(item) => {
            writer.byte(12);
            type_ref(writer, *item);
        }
        Type::Never => writer.byte(13),
        Type::Builder(item) => {
            writer.byte(14);
            type_ref(writer, *item);
        }
        Type::Map(key, value) => {
            writer.byte(15);
            type_ref(writer, *key);
            type_ref(writer, *value);
        }
        Type::MapBuilder(key, value) => {
            writer.byte(16);
            type_ref(writer, *key);
            type_ref(writer, *value);
        }
        Type::Region => writer.byte(17),
        Type::Handle(handle) => {
            writer.byte(18);
            writer.number(u64::from(*handle));
        }
        Type::Any => writer.byte(19),
        Type::Range => writer.byte(20),
        Type::Fn { params, result } => {
            writer.byte(21);
            writer.number(params.len() as u64);
            for param in params {
                type_ref(writer, *param);
            }
            option_type(writer, *result);
        }
        Type::Future(result) => {
            writer.byte(22);
            option_type(writer, *result);
        }
        Type::Cap(cap) => {
            writer.byte(23);
            cap.encode(writer);
        }
    }
}

fn option_type(writer: &mut Writer, ty: Option<TypeRef>) {
    match ty {
        None => writer.byte(0),
        Some(ty) => {
            writer.byte(1);
            type_ref(writer, ty);
        }
    }
}

fn read_option_type(reader: &mut Reader<'_>) -> Result<Option<TypeRef>, CodecError> {
    match reader.byte()? {
        0 => Ok(None),
        1 => Ok(Some(read_type_ref(reader)?)),
        other => Err(reader.error(format!("invalid option tag {other}"))),
    }
}

fn decode_type(reader: &mut Reader<'_>) -> Result<Type, CodecError> {
    Ok(match reader.byte()? {
        0 => Type::Int,
        1 => Type::Bool,
        2 => Type::Byte,
        3 => Type::Float,
        4 => Type::Rat,
        5 => Type::Char,
        6 => Type::Str,
        7 => Type::Object(ObjectRef::decode(reader)?),
        8 => Type::Successor(ObjectRef::decode(reader)?),
        9 => Type::Role(RoleRef::decode(reader)?),
        10 => Type::Enum(EnumRef::decode(reader)?),
        11 => Type::Failure,
        12 => Type::Array(read_type_ref(reader)?),
        13 => Type::Never,
        14 => Type::Builder(read_type_ref(reader)?),
        15 => Type::Map(read_type_ref(reader)?, read_type_ref(reader)?),
        16 => Type::MapBuilder(read_type_ref(reader)?, read_type_ref(reader)?),
        17 => Type::Region,
        18 => Type::Handle(reader.u32()?),
        19 => Type::Any,
        20 => Type::Range,
        21 => {
            let count = reader.count()?;
            let params = (0..count).map(|_| read_type_ref(reader)).collect::<Result<_, _>>()?;
            Type::Fn {
                params,
                result: read_option_type(reader)?,
            }
        }
        22 => Type::Future(read_option_type(reader)?),
        23 => Type::Cap(CapRef::decode(reader)?),
        other => return Err(reader.error(format!("invalid type tag {other}"))),
    })
}

fn encode_function(writer: &mut Writer, function: &Function) {
    writer.text(&function.symbol);
    writer.byte(u8::from(function.is_async));
    function.native.encode(writer);
    match &function.intrinsic {
        None => writer.byte(0),
        Some(intrinsic) => {
            writer.byte(1);
            writer.text(&intrinsic.name);
            intrinsic.objects.encode(writer);
            intrinsic.helpers.encode(writer);
        }
    }
    writer.number(function.registers.len() as u64);
    for ty in &function.registers {
        type_ref(writer, *ty);
    }
    function.params.encode(writer);
    option_type(writer, function.result);
    writer.number(function.code.len() as u64);
    for instruction in &function.code {
        writer.byte(instruction.opcode());
        instruction.encode_operands(writer);
    }
    writer.number(function.handlers.len() as u64);
    for handler in &function.handlers {
        writer.number(u64::from(handler.start));
        writer.number(u64::from(handler.end));
        handler.target.encode(writer);
        handler.failure.encode(writer);
    }
}

fn decode_function(reader: &mut Reader<'_>) -> Result<Function, CodecError> {
    let symbol = reader.text()?;
    let is_async = bool::decode(reader)?;
    let native = Option::<String>::decode(reader)?;
    let intrinsic = match reader.byte()? {
        0 => None,
        1 => Some(Intrinsic {
            name: reader.text()?,
            objects: Vec::<ObjectRef>::decode(reader)?,
            helpers: Vec::<FuncRef>::decode(reader)?,
        }),
        other => return Err(reader.error(format!("invalid intrinsic tag {other}"))),
    };
    let count = reader.count()?;
    let registers = (0..count).map(|_| read_type_ref(reader)).collect::<Result<_, _>>()?;
    let params = Vec::<Register>::decode(reader)?;
    let result = read_option_type(reader)?;
    let count = reader.count()?;
    let mut code = Vec::with_capacity(count);
    for _ in 0..count {
        let opcode = reader.byte()?;
        code.push(Instruction::decode_operands(opcode, reader)?);
    }
    let count = reader.count()?;
    let mut handlers = Vec::with_capacity(count);
    for _ in 0..count {
        handlers.push(Handler {
            start: reader.u32()?,
            end: reader.u32()?,
            target: Target::decode(reader)?,
            failure: Register::decode(reader)?,
        });
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

/// Кодирует проверенную программу детерминированно; не пишет файлов.
pub fn encode(program: &Program) -> Result<Vec<u8>, CodecError> {
    program.verify().map_err(|error| CodecError {
        offset: None,
        message: format!("invalid program: {error}"),
    })?;
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.bytes(&VERSION.to_be_bytes());
    writer.text(&program.module);
    writer.text(&program.version);
    writer.number(program.types.len() as u64);
    for ty in &program.types {
        encode_type(&mut writer, ty);
    }
    writer.number(program.enums.len() as u64);
    for decl in &program.enums {
        writer.text(&decl.name);
        decl.cases.encode(&mut writer);
    }
    writer.number(program.roles.len() as u64);
    for role in &program.roles {
        writer.text(&role.name);
        writer.byte(u8::from(role.builtin));
        writer.number(role.methods.len() as u64);
        for method in &role.methods {
            writer.text(&method.name);
            writer.number(method.params.len() as u64);
            for param in &method.params {
                type_ref(&mut writer, *param);
            }
            option_type(&mut writer, method.result);
        }
    }
    writer.number(program.objects.len() as u64);
    for object in &program.objects {
        writer.text(&object.name);
        writer.number(object.fields.len() as u64);
        for (name, ty) in &object.fields {
            writer.text(name);
            type_ref(&mut writer, *ty);
        }
        writer.number(object.does.len() as u64);
        for witness in &object.does {
            witness.role.encode(&mut writer);
            witness.slots.encode(&mut writer);
        }
        object.destroy.encode(&mut writer);
        writer.byte(u8::from(object.builtin));
    }
    program.handles.encode(&mut writer);
    writer.number(program.caps.len() as u64);
    for cap in &program.caps {
        writer.text(&cap.name);
        cap.owner.encode(&mut writer);
        writer.number(cap.fields.len() as u64);
        for (name, function) in &cap.fields {
            writer.text(name);
            function.encode(&mut writer);
        }
    }
    writer.number(program.functions.len() as u64);
    for function in &program.functions {
        encode_function(&mut writer, function);
    }
    program.entry.encode(&mut writer);
    match &program.tests {
        None => writer.byte(0),
        Some(tests) => {
            writer.byte(1);
            writer.number(tests.len() as u64);
            for (name, function) in tests {
                writer.text(name);
                function.encode(&mut writer);
            }
        }
    }
    if writer.bytes.len() > MAX_FILE_BYTES {
        return Err(CodecError {
            offset: None,
            message: format!("encoded program exceeds {MAX_FILE_BYTES} bytes"),
        });
    }
    Ok(writer.bytes)
}

/// Декодирует и проверяет программу; лишние байты — ошибка.
pub fn decode(bytes: &[u8]) -> Result<Program, CodecError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(CodecError {
            offset: None,
            message: format!("file exceeds {MAX_FILE_BYTES} bytes"),
        });
    }
    let mut reader = Reader { bytes, at: 0 };
    if reader.take(4)? != MAGIC {
        return Err(CodecError {
            offset: Some(0),
            message: "not a Sparrow bytecode file".into(),
        });
    }
    let version = u16::from_be_bytes(reader.take(2)?.try_into().expect("two"));
    if version != VERSION {
        return Err(CodecError {
            offset: Some(4),
            message: format!("unsupported SPB version {version} (this VM reads SPB {VERSION}; rebuild the program)"),
        });
    }
    let module = reader.text()?;
    let version_text = reader.text()?;
    let count = reader.count()?;
    let types = (0..count).map(|_| decode_type(&mut reader)).collect::<Result<_, _>>()?;
    let count = reader.count()?;
    let mut enums = Vec::with_capacity(count);
    for _ in 0..count {
        enums.push(EnumDecl {
            name: reader.text()?,
            cases: Vec::<String>::decode(&mut reader)?,
        });
    }
    let count = reader.count()?;
    let mut roles = Vec::with_capacity(count);
    for _ in 0..count {
        let name = reader.text()?;
        let builtin = bool::decode(&mut reader)?;
        let methods_count = reader.count()?;
        let mut methods = Vec::with_capacity(methods_count);
        for _ in 0..methods_count {
            let name = reader.text()?;
            let params_count = reader.count()?;
            let params = (0..params_count).map(|_| read_type_ref(&mut reader)).collect::<Result<_, _>>()?;
            methods.push(RoleMethod {
                name,
                params,
                result: read_option_type(&mut reader)?,
            });
        }
        roles.push(RoleDecl { name, builtin, methods });
    }
    let count = reader.count()?;
    let mut objects = Vec::with_capacity(count);
    for _ in 0..count {
        let name = reader.text()?;
        let fields_count = reader.count()?;
        let mut fields = Vec::with_capacity(fields_count);
        for _ in 0..fields_count {
            fields.push((reader.text()?, read_type_ref(&mut reader)?));
        }
        let does_count = reader.count()?;
        let mut does = Vec::with_capacity(does_count);
        for _ in 0..does_count {
            does.push(Witness {
                role: RoleRef::decode(&mut reader)?,
                slots: Vec::<FuncRef>::decode(&mut reader)?,
            });
        }
        objects.push(ObjectDecl {
            name,
            fields,
            does,
            destroy: Option::<FuncRef>::decode(&mut reader)?,
            builtin: bool::decode(&mut reader)?,
        });
    }
    let handles = Vec::<String>::decode(&mut reader)?;
    let count = reader.count()?;
    let mut caps = Vec::with_capacity(count);
    for _ in 0..count {
        let name = reader.text()?;
        let owner = ObjectRef::decode(&mut reader)?;
        let fields_count = reader.count()?;
        let mut fields = Vec::with_capacity(fields_count);
        for _ in 0..fields_count {
            fields.push((reader.text()?, FuncRef::decode(&mut reader)?));
        }
        caps.push(CapDecl { name, owner, fields });
    }
    let count = reader.count()?;
    let mut functions = Vec::with_capacity(count);
    for _ in 0..count {
        functions.push(decode_function(&mut reader)?);
    }
    let entry = Option::<FuncRef>::decode(&mut reader)?;
    let tests = match reader.byte()? {
        0 => None,
        1 => {
            let count = reader.count()?;
            let mut tests = Vec::with_capacity(count);
            for _ in 0..count {
                tests.push((reader.text()?, FuncRef::decode(&mut reader)?));
            }
            Some(tests)
        }
        other => return Err(reader.error(format!("invalid tests tag {other}"))),
    };
    if reader.at != bytes.len() {
        return Err(reader.error("trailing bytes after program"));
    }
    let program = Program {
        module,
        version: version_text,
        types,
        enums,
        roles,
        objects,
        handles,
        caps,
        functions,
        entry,
        tests,
    };
    program.verify().map_err(|error| CodecError {
        offset: None,
        message: format!("invalid program: {error}"),
    })?;
    Ok(program)
}
