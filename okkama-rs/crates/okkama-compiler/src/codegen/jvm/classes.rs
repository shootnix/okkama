//! Вспомогательные классы модуля: объекты, enum и интерфейсы ролей.

use super::asm::{Asm, Frame, VType};
use super::classfile::{self, ACC_ABSTRACT, ClassNames, ConstantPool, FieldInfo, Method};
use super::{EnumLayout, FnLayout, Layout, ObjectLayout, Result, RoleLayout, error, runtime};
use crate::ir::{ResultType, Type};

fn names(
    pool: &mut ConstantPool,
    access: u16,
    class: &str,
    super_class: &str,
) -> Result<ClassNames> {
    Ok(ClassNames {
        access,
        this_class: pool.class(class)?,
        super_class: pool.class(super_class)?,
        interfaces: Vec::new(),
        code: pool.utf8("Code")?,
        stack_map_table: pool.utf8("StackMapTable")?,
    })
}

fn method(
    pool: &mut ConstantPool,
    asm: Asm,
    access: u16,
    name: &str,
    descriptor: &str,
    max_locals: usize,
) -> Result<Method> {
    let assembled = asm.finish()?;
    Ok(Method {
        access,
        name: pool.utf8(name)?,
        descriptor: pool.utf8(descriptor)?,
        max_stack: assembled.max_stack,
        max_locals: u16::try_from(max_locals)
            .map_err(|_| error("method has too many local variables"))?,
        code: assembled.code,
        exceptions: assembled.exceptions,
        stack_map: assembled.stack_map,
    })
}

/// Виды параметров дескриптора (все значения OIR занимают один слот) и
/// символ результата: `Z`/`I` — int-слот, `L`/`[` — ссылка, `V` — void.
fn descriptor_kinds(descriptor: &str) -> (Vec<VType>, char) {
    let (params, result) = descriptor[1..]
        .split_once(')')
        .expect("generated descriptors are well formed");
    let mut kinds = Vec::new();
    let mut chars = params.chars();
    while let Some(ch) = chars.next() {
        match ch {
            'Z' | 'I' => kinds.push(VType::Integer),
            'L' => {
                for inner in chars.by_ref() {
                    if inner == ';' {
                        break;
                    }
                }
                kinds.push(VType::Object(0));
            }
            other => unreachable!("OIR does not produce descriptor element `{other}`"),
        }
    }
    (kinds, result.chars().next().unwrap_or('V'))
}

/// Поле или параметр в int-слоте JVM: `bool` (`Z`), `byte` и `char` (`I`).
fn is_int_slot(descriptor: &str) -> bool {
    descriptor == "Z" || descriptor == "I"
}

fn return_opcode(result: char) -> (u8, i32) {
    match result {
        'V' => (0xb1, 0),       // return
        'Z' | 'I' => (0xac, 1), // ireturn
        _ => (0xb0, 1),         // areturn
    }
}

/// Класс объекта: `final`-поля, конструктор `<init>(поля…)` и мосты ролей.
pub(super) fn object_class(layout: &Layout, object: &ObjectLayout) -> Result<Vec<u8>> {
    let mut pool = ConstantPool::default();
    let mut names = names(&mut pool, 0x0030, &object.class, runtime::OBJ)?;
    for role in &object.interfaces {
        names
            .interfaces
            .push(pool.class(&layout.roles[*role].class)?);
    }
    let mut fields = Vec::new();
    for (name, descriptor) in &object.fields {
        // Не `final`: `obj.init` связывает поле после раннего `self` (§11.3).
        fields.push(FieldInfo {
            access: 0x0000,
            name: pool.utf8(name)?,
            descriptor: pool.utf8(descriptor)?,
        });
    }
    let mut methods = Vec::new();

    let mut asm = Asm::default();
    let object_init = pool.method_ref(runtime::OBJ, "<init>", "()V")?;
    asm.op(&[0x2a], 0, 1); // aload_0
    asm.op_index(0xb7, object_init, 1, 0); // invokespecial Object.<init>
    for (index, (name, descriptor)) in object.fields.iter().enumerate() {
        asm.op(&[0x2a], 0, 1); // aload_0
        let slot = u16::try_from(index + 1)
            .map_err(|_| error("object has too many fields for the JVM"))?;
        let ty = if is_int_slot(descriptor) {
            VType::Integer
        } else {
            VType::Object(0)
        };
        asm.load(ty, slot);
        let field = pool.field_ref(&object.class, name, descriptor)?;
        asm.op_index(0xb5, field, 2, 0); // putfield
    }
    asm.op(&[0xb1], 0, 0); // return
    methods.push(method(
        &mut pool,
        asm,
        0x0000,
        "<init>",
        &object.ctor_descriptor(),
        object.fields.len() + 1,
    )?);

    methods.push(refs_method(&mut pool, object)?);
    methods.push(okkama_name_method(&mut pool, object)?);
    methods.push(okkama_fields_method(&mut pool, object)?);
    if let Some(destroy) = object.destroy {
        let (name, descriptor) = &layout.signatures[destroy.0];
        let target = pool.method_ref(&layout.main, name, descriptor)?;
        let mut asm = Asm::default();
        asm.op(&[0x2a], 0, 1); // aload_0
        asm.op_index(0xb8, target, 1, 0); // invokestatic destroy-функции
        asm.op(&[0xb1], 0, 0); // return
        methods.push(method(&mut pool, asm, 0x0001, "destroyBody", "()V", 1)?);
    }
    methods.push(equals_method(&mut pool, object)?);
    methods.push(hash_code_method(&mut pool, object)?);

    // Мост: метод экземпляра роли вызывает статическую реализацию с `this`.
    for bridge in &object.bridges {
        let (params, result) = descriptor_kinds(&bridge.descriptor);
        let (target_name, target_descriptor) = &layout.signatures[bridge.target.0];
        let mut asm = Asm::default();
        asm.op(&[0x2a], 0, 1); // aload_0
        for (index, kind) in params.iter().enumerate() {
            asm.load(*kind, (index + 1) as u16);
        }
        let target = pool.method_ref(&layout.main, target_name, target_descriptor)?;
        let (opcode, pushes) = return_opcode(result);
        asm.op_index(0xb8, target, params.len() as i32 + 1, pushes); // invokestatic
        asm.op(&[opcode], pushes, 0);
        methods.push(method(
            &mut pool,
            asm,
            0x0001, // public
            &bridge.name,
            &bridge.descriptor,
            params.len() + 1,
        )?);
    }
    classfile::write_class(&pool, &names, &fields, &methods)
}

/// `refs(List)`: значения полей, через которые достижимы идентичности.
fn refs_method(pool: &mut ConstantPool, object: &ObjectLayout) -> Result<Method> {
    let add = pool.interface_method_ref(runtime::LIST, "add", "(Ljava/lang/Object;)Z")?;
    let mut asm = Asm::default();
    for index in &object.refs {
        let (name, descriptor) = &object.fields[*index];
        let field = pool.field_ref(&object.class, name, descriptor)?;
        asm.op(&[0x2b], 0, 1); // aload_1
        asm.op(&[0x2a], 0, 1); // aload_0
        asm.op_index(0xb4, field, 1, 1); // getfield
        asm.invoke_interface(add, 2, 2, 1);
        asm.op(&[0x57], 1, 0); // pop
    }
    asm.op(&[0xb1], 0, 0); // return
    method(pool, asm, 0x0001, "refs", "(Ljava/util/List;)V", 2)
}

/// `okkamaName()`: имя объекта Okkama для дампа (`runtime/debug`).
fn okkama_name_method(pool: &mut ConstantPool, object: &ObjectLayout) -> Result<Method> {
    let name = pool.string(&object.name)?;
    let mut asm = Asm::default();
    asm.ldc(name);
    asm.op(&[0xb0], 1, 0); // areturn
    method(pool, asm, 0x0001, "okkamaName", "()Ljava/lang/String;", 1)
}

/// `okkamaFields(List)`: пары «имя поля, значение» по порядку для дампа;
/// поля примитивов упаковываются, как значения `any`.
fn okkama_fields_method(pool: &mut ConstantPool, object: &ObjectLayout) -> Result<Method> {
    let add = pool.interface_method_ref(runtime::LIST, "add", "(Ljava/lang/Object;)Z")?;
    let mut asm = Asm::default();
    for (index, (name, descriptor)) in object.fields.iter().enumerate() {
        let label = pool.string(name)?;
        asm.op(&[0x2b], 0, 1); // aload_1
        asm.ldc(label);
        asm.invoke_interface(add, 2, 2, 1);
        asm.op(&[0x57], 1, 0); // pop
        let field = pool.field_ref(&object.class, name, descriptor)?;
        asm.op(&[0x2b], 0, 1); // aload_1
        asm.op(&[0x2a], 0, 1); // aload_0
        asm.op_index(0xb4, field, 1, 1); // getfield
        let boxing = match object.field_types.get(index) {
            Some(Type::Bool) => Some(("java/lang/Boolean", "(Z)Ljava/lang/Boolean;")),
            Some(Type::Byte) => {
                asm.op(&[0x91], 1, 1); // i2b
                Some(("java/lang/Byte", "(B)Ljava/lang/Byte;"))
            }
            Some(Type::Char) => Some(("java/lang/Integer", "(I)Ljava/lang/Integer;")),
            _ => None,
        };
        if let Some((owner, signature)) = boxing {
            let value_of = pool.method_ref(owner, "valueOf", signature)?;
            asm.op_index(0xb8, value_of, 1, 1); // invokestatic valueOf
        }
        asm.invoke_interface(add, 2, 2, 1);
        asm.op(&[0x57], 1, 0); // pop
    }
    asm.op(&[0xb1], 0, 0); // return
    method(pool, asm, 0x0001, "okkamaFields", "(Ljava/util/List;)V", 2)
}

/// `equals(Object)`: тот же класс и равные поля в порядке объявления
/// (`val.eq`, §16.3). Ссылочные поля сравнивает `Objects.equals`.
fn equals_method(pool: &mut ConstantPool, object: &ObjectLayout) -> Result<Method> {
    let class = pool.class(&object.class)?;
    let any = pool.class("java/lang/Object")?;
    let objects_equals = pool.method_ref(
        "java/util/Objects",
        "equals",
        "(Ljava/lang/Object;Ljava/lang/Object;)Z",
    )?;
    let mut asm = Asm::default();
    let same = asm.label();
    let differ = asm.label();
    asm.op(&[0x2b], 0, 1); // aload_1
    asm.op_index(0xc1, class, 1, 1); // instanceof
    asm.jump(0x9a, same, 1); // ifne
    asm.op(&[0x03, 0xac], 0, 0); // iconst_0 ireturn
    asm.place_with_frame(
        same,
        Frame {
            locals: vec![VType::Object(class), VType::Object(any)],
            stack: Vec::new(),
        },
    );
    asm.op(&[0x2b], 0, 1); // aload_1
    asm.op_index(0xc0, class, 1, 1); // checkcast
    asm.op(&[0x4d], 1, 0); // astore_2
    for (name, descriptor) in &object.fields {
        let field = pool.field_ref(&object.class, name, descriptor)?;
        asm.op(&[0x2a], 0, 1); // aload_0
        asm.op_index(0xb4, field, 1, 1); // getfield
        asm.op(&[0x2c], 0, 1); // aload_2
        asm.op_index(0xb4, field, 1, 1); // getfield
        if is_int_slot(descriptor) {
            asm.jump(0xa0, differ, 2); // if_icmpne
        } else {
            asm.op_index(0xb8, objects_equals, 2, 1); // invokestatic
            asm.jump(0x99, differ, 1); // ifeq
        }
    }
    asm.op(&[0x04, 0xac], 0, 0); // iconst_1 ireturn
    if !object.fields.is_empty() {
        asm.place_with_frame(
            differ,
            Frame {
                locals: vec![
                    VType::Object(class),
                    VType::Object(any),
                    VType::Object(class),
                ],
                stack: Vec::new(),
            },
        );
        asm.op(&[0x03, 0xac], 0, 0); // iconst_0 ireturn
    }
    method(pool, asm, 0x0001, "equals", "(Ljava/lang/Object;)Z", 3)
}

/// `hashCode()`, согласованный с `equals`: `h = 31 * h + hash(поле)`.
fn hash_code_method(pool: &mut ConstantPool, object: &ObjectLayout) -> Result<Method> {
    let objects_hash = pool.method_ref("java/util/Objects", "hashCode", "(Ljava/lang/Object;)I")?;
    let mut asm = Asm::default();
    asm.op(&[0x04], 0, 1); // iconst_1
    for (name, descriptor) in &object.fields {
        let field = pool.field_ref(&object.class, name, descriptor)?;
        asm.op(&[0x10, 31], 0, 1); // bipush 31
        asm.op(&[0x68], 2, 1); // imul
        asm.op(&[0x2a], 0, 1); // aload_0
        asm.op_index(0xb4, field, 1, 1); // getfield
        if !is_int_slot(descriptor) {
            asm.op_index(0xb8, objects_hash, 1, 1); // invokestatic
        }
        asm.op(&[0x60], 2, 1); // iadd
    }
    asm.op(&[0xac], 1, 0); // ireturn
    method(pool, asm, 0x0001, "hashCode", "()I", 1)
}

/// Интерфейс роли: абстрактный метод на каждый слот.
pub(super) fn role_interface(role: &RoleLayout) -> Result<Vec<u8>> {
    let mut pool = ConstantPool::default();
    let names = names(&mut pool, 0x0601, &role.class, "java/lang/Object")?;
    let mut methods = Vec::new();
    for (name, descriptor) in &role.methods {
        methods.push(Method {
            access: 0x0001 | ACC_ABSTRACT,
            name: pool.utf8(name)?,
            descriptor: pool.utf8(descriptor)?,
            max_stack: 0,
            max_locals: 0,
            code: Vec::new(),
            exceptions: Vec::new(),
            stack_map: None,
        });
    }
    classfile::write_class(&pool, &names, &[], &methods)
}

/// Класс enum: `public static final Case <case>` на каждый case в порядке
/// объявления; `<clinit>` создаёт синглтоны `new Case(номер, имя)`.
pub(super) fn enum_class(decl: &EnumLayout) -> Result<Vec<u8>> {
    let mut pool = ConstantPool::default();
    let names = names(&mut pool, 0x0031, &decl.class, "java/lang/Object")?;
    let descriptor = format!("L{};", runtime::CASE);
    let mut fields = Vec::new();
    for case in &decl.cases {
        fields.push(FieldInfo {
            access: 0x0019, // public static final
            name: pool.utf8(case)?,
            descriptor: pool.utf8(&descriptor)?,
        });
    }
    let case_class = pool.class(runtime::CASE)?;
    let init = pool.method_ref(
        runtime::CASE,
        "<init>",
        "(ILjava/lang/String;Ljava/lang/String;)V",
    )?;
    let type_name = pool.string(&decl.name)?;
    let mut asm = Asm::default();
    for (ordinal, case) in decl.cases.iter().enumerate() {
        let ordinal = i16::try_from(ordinal).map_err(|_| error("enum has too many cases"))?;
        asm.op_index(0xbb, case_class, 0, 1); // new
        asm.op(&[0x59], 0, 1); // dup
        let [high, low] = ordinal.to_be_bytes();
        asm.op(&[0x11, high, low], 0, 1); // sipush
        let text = pool.string(case)?;
        asm.ldc(text);
        asm.ldc(type_name);
        asm.op_index(0xb7, init, 4, 0); // invokespecial
        let field = pool.field_ref(&decl.class, case, &descriptor)?;
        asm.op_index(0xb3, field, 1, 0); // putstatic
    }
    asm.op(&[0xb1], 0, 0); // return
    let clinit = method(&mut pool, asm, 0x0008, "<clinit>", "()V", 0)?;
    classfile::write_class(&pool, &names, &fields, &[clinit])
}

/// Целая константа на стек (`iconst`/`bipush`/`sipush`).
pub(super) fn push_int(asm: &mut Asm, value: usize) -> Result<()> {
    match value {
        0..=5 => asm.op(&[0x03 + value as u8], 0, 1), // iconst_n
        6..=127 => asm.op(&[0x10, value as u8], 0, 1), // bipush
        _ => {
            let value =
                u16::try_from(value).map_err(|_| error("fn value has too many arguments"))?;
            let [high, low] = value.to_be_bytes();
            asm.op(&[0x11, high, low], 0, 1); // sipush
        }
    }
    Ok(())
}

/// Значение типа `ty` на стеке → объект коллекционного представления.
pub(super) fn box_value(asm: &mut Asm, pool: &mut ConstantPool, ty: &Type) -> Result<()> {
    let (owner, descriptor) = match ty {
        Type::Bool => ("java/lang/Boolean", "(Z)Ljava/lang/Boolean;"),
        Type::Byte => {
            asm.op(&[0x91], 1, 1); // i2b
            ("java/lang/Byte", "(B)Ljava/lang/Byte;")
        }
        Type::Char => ("java/lang/Integer", "(I)Ljava/lang/Integer;"),
        _ => return Ok(()),
    };
    let index = pool.method_ref(owner, "valueOf", descriptor)?;
    asm.op_index(0xb8, index, 1, 1); // invokestatic
    Ok(())
}

/// Объект на стеке → значение типа `ty` (`checkcast`, затем распаковка).
pub(super) fn unbox_value(
    asm: &mut Asm,
    pool: &mut ConstantPool,
    layout: &Layout,
    ty: &Type,
) -> Result<()> {
    let class = match ty {
        Type::Bool => "java/lang/Boolean".to_string(),
        Type::Byte => "java/lang/Byte".to_string(),
        Type::Char => "java/lang/Integer".to_string(),
        other => {
            let descriptor = layout.descriptor(other);
            descriptor[1..descriptor.len() - 1].to_string()
        }
    };
    if class != "java/lang/Object" {
        let index = pool.class(&class)?;
        asm.op_index(0xc0, index, 1, 1); // checkcast
    }
    let (owner, name, descriptor) = match ty {
        Type::Bool => ("java/lang/Boolean", "booleanValue", "()Z"),
        Type::Byte => ("java/lang/Byte", "byteValue", "()B"),
        Type::Char => ("java/lang/Integer", "intValue", "()I"),
        _ => return Ok(()),
    };
    let index = pool.method_ref(owner, name, descriptor)?;
    asm.op_index(0xb6, index, 1, 1); // invokevirtual
    if *ty == Type::Byte {
        asm.op(&[0x11, 0x00, 0xff], 0, 1); // sipush 255
        asm.op(&[0x7e], 2, 1); // iand: знаковый byte JVM → 0..255
    }
    Ok(())
}

fn slot_kind(ty: &Type) -> VType {
    if matches!(ty, Type::Bool | Type::Byte | Type::Char) {
        VType::Integer
    } else {
        VType::Object(0)
    }
}

/// Класс значения-функции: поля-захваты, `call(Object[])` — статическая
/// функция над захватами и распакованными аргументами, `captures()`.
pub(super) fn fn_class(layout: &Layout, fn_layout: &FnLayout) -> Result<Vec<u8>> {
    let mut pool = ConstantPool::default();
    let mut names = names(&mut pool, 0x0030, &fn_layout.class, "java/lang/Object")?;
    names.interfaces.push(pool.class(runtime::FN)?);
    let field_names: Vec<String> = (0..fn_layout.captures.len())
        .map(|index| format!("c{index}"))
        .collect();
    let mut fields = Vec::new();
    for (name, ty) in field_names.iter().zip(&fn_layout.captures) {
        fields.push(FieldInfo {
            access: 0x0010, // final
            name: pool.utf8(name)?,
            descriptor: pool.utf8(&layout.descriptor(ty))?,
        });
    }
    let mut methods = Vec::new();

    let mut asm = Asm::default();
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V")?;
    asm.op(&[0x2a], 0, 1); // aload_0
    asm.op_index(0xb7, object_init, 1, 0); // invokespecial Object.<init>
    for (index, (name, ty)) in field_names.iter().zip(&fn_layout.captures).enumerate() {
        asm.op(&[0x2a], 0, 1); // aload_0
        asm.load(slot_kind(ty), (index + 1) as u16);
        let field = pool.field_ref(&fn_layout.class, name, &layout.descriptor(ty))?;
        asm.op_index(0xb5, field, 2, 0); // putfield
    }
    asm.op(&[0xb1], 0, 0); // return
    methods.push(method(
        &mut pool,
        asm,
        0x0000,
        "<init>",
        &layout.fn_ctor_descriptor(fn_layout),
        fn_layout.captures.len() + 1,
    )?);

    let mut asm = Asm::default();
    for (name, ty) in field_names.iter().zip(&fn_layout.captures) {
        asm.op(&[0x2a], 0, 1); // aload_0
        let field = pool.field_ref(&fn_layout.class, name, &layout.descriptor(ty))?;
        asm.op_index(0xb4, field, 1, 1); // getfield
    }
    for (index, ty) in fn_layout.params.iter().enumerate() {
        asm.op(&[0x2b], 0, 1); // aload_1
        push_int(&mut asm, index)?;
        asm.op(&[0x32], 2, 1); // aaload
        unbox_value(&mut asm, &mut pool, layout, ty)?;
    }
    let (target_name, target_descriptor) = &fn_layout.target;
    let target = pool.method_ref(&layout.main, target_name, target_descriptor)?;
    let arity = (fn_layout.captures.len() + fn_layout.params.len()) as i32;
    match &fn_layout.result {
        ResultType::Void => {
            asm.op_index(0xb8, target, arity, 0); // invokestatic
            asm.op(&[0x01], 0, 1); // aconst_null
        }
        ResultType::Value(ty) => {
            asm.op_index(0xb8, target, arity, 1); // invokestatic
            box_value(&mut asm, &mut pool, ty)?;
        }
    }
    asm.op(&[0xb0], 1, 0); // areturn
    methods.push(method(
        &mut pool,
        asm,
        0x0001,
        "call",
        "([Ljava/lang/Object;)Ljava/lang/Object;",
        2,
    )?);

    let mut asm = Asm::default();
    push_int(&mut asm, fn_layout.captures.len())?;
    let object = pool.class("java/lang/Object")?;
    asm.op_index(0xbd, object, 1, 1); // anewarray
    for (index, (name, ty)) in field_names.iter().zip(&fn_layout.captures).enumerate() {
        asm.op(&[0x59], 0, 1); // dup
        push_int(&mut asm, index)?;
        asm.op(&[0x2a], 0, 1); // aload_0
        let field = pool.field_ref(&fn_layout.class, name, &layout.descriptor(ty))?;
        asm.op_index(0xb4, field, 1, 1); // getfield
        box_value(&mut asm, &mut pool, ty)?;
        asm.op(&[0x53], 3, 0); // aastore
    }
    asm.op(&[0xb0], 1, 0); // areturn
    methods.push(method(
        &mut pool,
        asm,
        0x0001,
        "captures",
        "()[Ljava/lang/Object;",
        1,
    )?);
    classfile::write_class(&pool, &names, &fields, &methods)
}
