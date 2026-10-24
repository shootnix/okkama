//! Минимальный writer формата class (JVMS §4), без зависимостей и файлового IO.
//! Индексы constant pool начинаются с 1; Long занимает два индекса.

use std::collections::HashMap;

use super::{Result, error};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Constant {
    Utf8(Vec<u8>),
    Class(u16),
    String(u16),
    Long(i64),
    NameAndType(u16, u16),
    FieldRef(u16, u16),
    MethodRef(u16, u16),
    InterfaceMethodRef(u16, u16),
}

#[derive(Default)]
pub(super) struct ConstantPool {
    entries: Vec<Option<Constant>>,
    indices: HashMap<Constant, u16>,
}

impl ConstantPool {
    fn add(&mut self, constant: Constant) -> Result<u16> {
        if let Some(index) = self.indices.get(&constant) {
            return Ok(*index);
        }
        let slots = if matches!(constant, Constant::Long(_)) {
            2
        } else {
            1
        };
        if self.entries.len() + slots + 1 > usize::from(u16::MAX) {
            return Err(error("constant pool exceeds the JVM u16 limit"));
        }
        let index = (self.entries.len() + 1) as u16;
        self.indices.insert(constant.clone(), index);
        self.entries.push(Some(constant));
        if slots == 2 {
            self.entries.push(None);
        }
        Ok(index)
    }

    pub(super) fn utf8(&mut self, text: &str) -> Result<u16> {
        // Class files use modified UTF-8 over UTF-16 code units (JVMS §4.4.7),
        // including two-byte NUL and surrogate pairs for supplementary chars.
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            match unit {
                1..=0x7f => bytes.push(unit as u8),
                0..=0x7ff => {
                    bytes.push(0xc0 | (unit >> 6) as u8);
                    bytes.push(0x80 | (unit & 0x3f) as u8);
                }
                _ => {
                    bytes.push(0xe0 | (unit >> 12) as u8);
                    bytes.push(0x80 | ((unit >> 6) & 0x3f) as u8);
                    bytes.push(0x80 | (unit & 0x3f) as u8);
                }
            }
        }
        if bytes.len() > usize::from(u16::MAX) {
            return Err(error("modified UTF-8 constant exceeds 65535 bytes"));
        }
        self.add(Constant::Utf8(bytes))
    }

    pub(super) fn class(&mut self, name: &str) -> Result<u16> {
        let name = self.utf8(name)?;
        self.add(Constant::Class(name))
    }

    pub(super) fn string(&mut self, text: &str) -> Result<u16> {
        let text = self.utf8(text)?;
        self.add(Constant::String(text))
    }

    pub(super) fn long(&mut self, value: i64) -> Result<u16> {
        self.add(Constant::Long(value))
    }

    fn name_and_type(&mut self, name: &str, descriptor: &str) -> Result<u16> {
        let name = self.utf8(name)?;
        let descriptor = self.utf8(descriptor)?;
        self.add(Constant::NameAndType(name, descriptor))
    }

    pub(super) fn method_ref(&mut self, owner: &str, name: &str, descriptor: &str) -> Result<u16> {
        let owner = self.class(owner)?;
        let name_and_type = self.name_and_type(name, descriptor)?;
        self.add(Constant::MethodRef(owner, name_and_type))
    }

    pub(super) fn interface_method_ref(
        &mut self,
        owner: &str,
        name: &str,
        descriptor: &str,
    ) -> Result<u16> {
        let owner = self.class(owner)?;
        let name_and_type = self.name_and_type(name, descriptor)?;
        self.add(Constant::InterfaceMethodRef(owner, name_and_type))
    }

    pub(super) fn field_ref(&mut self, owner: &str, name: &str, descriptor: &str) -> Result<u16> {
        let owner = self.class(owner)?;
        let name_and_type = self.name_and_type(name, descriptor)?;
        self.add(Constant::FieldRef(owner, name_and_type))
    }

    fn write(&self, bytes: &mut Vec<u8>) {
        u16_to(bytes, (self.entries.len() + 1) as u16);
        // Never serialize the reserved second slot of a long constant.
        for entry in self.entries.iter().flatten() {
            match entry {
                Constant::Utf8(value) => {
                    bytes.push(1);
                    u16_to(bytes, value.len() as u16);
                    bytes.extend_from_slice(value);
                }
                Constant::Long(value) => {
                    bytes.push(5);
                    bytes.extend_from_slice(&value.to_be_bytes());
                }
                Constant::Class(name) => {
                    bytes.push(7);
                    u16_to(bytes, *name);
                }
                Constant::String(text) => {
                    bytes.push(8);
                    u16_to(bytes, *text);
                }
                Constant::FieldRef(owner, signature) => {
                    bytes.push(9);
                    u16_to(bytes, *owner);
                    u16_to(bytes, *signature);
                }
                Constant::MethodRef(owner, signature) => {
                    bytes.push(10);
                    u16_to(bytes, *owner);
                    u16_to(bytes, *signature);
                }
                Constant::InterfaceMethodRef(owner, signature) => {
                    bytes.push(11);
                    u16_to(bytes, *owner);
                    u16_to(bytes, *signature);
                }
                Constant::NameAndType(name, descriptor) => {
                    bytes.push(12);
                    u16_to(bytes, *name);
                    u16_to(bytes, *descriptor);
                }
            }
        }
    }
}

/// Запись exception table: `[start, end)` → `handler`, перехват `catch_type`.
pub(super) struct ExceptionEntry {
    pub start: u16,
    pub end: u16,
    pub handler: u16,
    pub catch_type: u16,
}

pub(super) struct Method {
    pub access: u16,
    pub name: u16,
    pub descriptor: u16,
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: Vec<u8>,
    pub exceptions: Vec<ExceptionEntry>,
    /// Готовое тело атрибута StackMapTable (`number_of_entries` и кадры) или пусто.
    pub stack_map: Option<Vec<u8>>,
}

pub(super) struct ClassNames {
    pub access: u16,
    pub this_class: u16,
    pub super_class: u16,
    /// Индексы `CONSTANT_Class` реализуемых интерфейсов.
    pub interfaces: Vec<u16>,
    pub code: u16,
    pub stack_map_table: u16,
}

pub(super) const ACC_ABSTRACT: u16 = 0x0400;

pub(super) struct FieldInfo {
    pub access: u16,
    pub name: u16,
    pub descriptor: u16,
}

pub(super) fn write_class(
    pool: &ConstantPool,
    names: &ClassNames,
    fields: &[FieldInfo],
    methods: &[Method],
) -> Result<Vec<u8>> {
    let method_count =
        u16::try_from(methods.len()).map_err(|_| error("class has too many methods"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xcafebabeu32.to_be_bytes());
    u16_to(&mut bytes, 0); // minor version
    u16_to(&mut bytes, 52); // Java 8; branches carry explicit StackMapTable frames
    pool.write(&mut bytes);
    u16_to(&mut bytes, names.access);
    u16_to(&mut bytes, names.this_class);
    u16_to(&mut bytes, names.super_class);
    let interface_count = u16::try_from(names.interfaces.len())
        .map_err(|_| error("class implements too many interfaces"))?;
    u16_to(&mut bytes, interface_count);
    for interface in &names.interfaces {
        u16_to(&mut bytes, *interface);
    }
    let field_count =
        u16::try_from(fields.len()).map_err(|_| error("class has too many fields"))?;
    u16_to(&mut bytes, field_count);
    for field in fields {
        u16_to(&mut bytes, field.access);
        u16_to(&mut bytes, field.name);
        u16_to(&mut bytes, field.descriptor);
        u16_to(&mut bytes, 0); // attributes
    }
    u16_to(&mut bytes, method_count);
    for method in methods {
        u16_to(&mut bytes, method.access);
        u16_to(&mut bytes, method.name);
        u16_to(&mut bytes, method.descriptor);
        // Абстрактный метод интерфейса не имеет атрибута Code.
        if method.access & ACC_ABSTRACT != 0 {
            u16_to(&mut bytes, 0);
            continue;
        }
        if method.code.is_empty() || method.code.len() > 65535 {
            return Err(error("method bytecode exceeds 65535 bytes"));
        }
        u16_to(&mut bytes, 1); // one Code attribute
        u16_to(&mut bytes, names.code);
        let mut code = Vec::new();
        u16_to(&mut code, method.max_stack);
        u16_to(&mut code, method.max_locals);
        code.extend_from_slice(&(method.code.len() as u32).to_be_bytes());
        code.extend_from_slice(&method.code);
        let exception_count = u16::try_from(method.exceptions.len())
            .map_err(|_| error("exception table exceeds the JVM u16 limit"))?;
        u16_to(&mut code, exception_count);
        for entry in &method.exceptions {
            u16_to(&mut code, entry.start);
            u16_to(&mut code, entry.end);
            u16_to(&mut code, entry.handler);
            u16_to(&mut code, entry.catch_type);
        }
        match &method.stack_map {
            Some(table) => {
                u16_to(&mut code, 1);
                u16_to(&mut code, names.stack_map_table);
                code.extend_from_slice(&(table.len() as u32).to_be_bytes());
                code.extend_from_slice(table);
            }
            None => u16_to(&mut code, 0),
        }
        bytes.extend_from_slice(&(code.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&code);
    }
    u16_to(&mut bytes, 0); // class attributes
    Ok(bytes)
}

pub(super) fn u16_to(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}
