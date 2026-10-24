//! JVM backend: проверенный модуль OIR-1 → class-файлы (§21.4).
//!
//! Главный класс содержит все функции модуля как статические методы,
//! `main` Okkama — `okkamaMain`; Java `main(String[])` вызывает его, проверяет
//! результат `0..255` и передаёт его в `System.exit`. Остальные классы лежат
//! рядом с главным:
//!
//! * объект Okkama — `final` класс `<Главный>$<Объект>` (наследник
//!   `okkama.rt.Obj`: владелец и порядок создания для регионов) с
//!   `final`-полями в порядке объявления, одним конструктором по всем полям,
//!   `refs`/`destroyBody` для регионов и мостовыми методами ролей;
//! * enum — `final` класс `<Главный>$<Enum>` со `static final` полем
//!   `okkama.rt.Case` на каждый case (синглтоны создаёт `<clinit>`);
//! * роль — интерфейс `<Главный>$<Роль>` с методами её слотов; системная
//!   роль `Error` — интерфейс рантайма `okkama.rt.Error`;
//! * рантайм `okkama.rt` (`Rt`, `Failure`, `Error`) — встроенные class-файлы
//!   Java-библиотеки `runtime/jvm`, пути `okkama/rt/*.class`.
//!
//! Значения: `int` — `java.math.BigInteger` (точная арифметика, §21.0);
//! `bool` — JVM `int` 0/1 (`Z` в дескрипторах); `byte` — `int` 0..255 и
//! `char` — `int` code point (`I`; в коллекциях `Byte` и `Integer`, чтобы
//! порядок и диагностика различали их); `str` — `String`; объект,
//! `^C` и роль — ссылки на их классы; массив и сборщик — `java.util.List`
//! (элементы `bool` — `Boolean`); словарь и его сборщик — `java.util.Map`
//! (`TreeMap` в порядке `okkama.rt.KeyOrder`); `failure` — `okkama.rt.Failure`.
//!
//! Каждое SSA-значение получает свой local slot, блоки — метки, параметры
//! блоков заполняются на рёбрах через стек. Перед каждым блоком кроме входа —
//! `full_frame` из значений, видимых в его начале. Цель unwind блока — запись
//! exception table на его байткод с заглушкой, которая раскладывает аргументы
//! по параметрам цели (§6.2 совпадает с правилом кадров JVM).
//!
//! Спецификация формата: https://docs.oracle.com/javase/specs/jvms/se26/html/jvms-4.html

mod asm;
mod classes;
mod classfile;
mod function;
mod outline;
mod runtime;

use std::collections::BTreeSet;

use crate::ir::{self, InstKind, ResultType, Type};

use super::{Backend, CodegenError};
use classfile::ClassNames;

type Result<T> = std::result::Result<T, CodegenError>;

fn error(message: impl Into<String>) -> CodegenError {
    CodegenError::InvalidInput {
        backend: Backend::Jvm,
        message: message.into(),
    }
}

const BIG: &str = "java/math/BigInteger";
const BIG_D: &str = "Ljava/math/BigInteger;";
const STRING: &str = "java/lang/String";
const STRING_D: &str = "Ljava/lang/String;";

/// Класс объекта: внутреннее имя JVM, поля (имя, дескриптор), роли и мосты.
struct ObjectLayout {
    class: String,
    fields: Vec<(String, String)>,
    interfaces: Vec<usize>,
    bridges: Vec<Bridge>,
    /// Поля, через которые достижимы идентичности (обход регионов).
    refs: Vec<usize>,
    /// Функция `destroy`, которую вызывает `destroyBody`.
    destroy: Option<ir::FuncId>,
}

impl ObjectLayout {
    fn ctor_descriptor(&self) -> String {
        let mut descriptor = String::from("(");
        for (_, field) in &self.fields {
            descriptor.push_str(field);
        }
        descriptor.push_str(")V");
        descriptor
    }
}

/// Метод экземпляра, реализующий слот роли вызовом статической функции.
struct Bridge {
    name: String,
    descriptor: String,
    target: ir::FuncId,
}

/// Интерфейс роли: методы слотов (имя, дескриптор) по порядку.
struct RoleLayout {
    class: String,
    methods: Vec<(String, String)>,
    /// Системная роль `Error`: интерфейс рантайма, файл не выдаётся.
    is_error: bool,
}

/// Класс значения-функции над `func`: поля-захваты и `call(Object[])`,
/// вызывающий статическую функцию (захваты, затем аргументы).
struct FnLayout {
    class: String,
    func: ir::FuncId,
    /// Тело холодного Future async-функции (захваты — все её параметры).
    thunk: bool,
    captures: Vec<Type>,
    params: Vec<Type>,
    result: ResultType,
    /// Статический метод, который вызывает `call`.
    target: (String, String),
}

/// Класс enum: внутреннее имя JVM и имена case (поля `Case`).
struct EnumLayout {
    class: String,
    /// Имя enum без модуля (`Light`): тип в отладочном выводе.
    name: String,
    cases: Vec<String>,
}

/// Имена классов и методов модуля для генерации кода.
struct Layout {
    main: String,
    enums: Vec<EnumLayout>,
    objects: Vec<ObjectLayout>,
    roles: Vec<RoleLayout>,
    /// Имя и дескриптор статического метода каждой функции модуля.
    signatures: Vec<(String, String)>,
    /// Классы значений-функций (`fn.closure`) и тел async-функций.
    fns: Vec<FnLayout>,
    /// Метод тела async-функции (`name$body`); её метод `name` даёт Future.
    bodies: Vec<Option<(String, String)>>,
    /// Владелец каждого cap: значение cap на JVM — сам владелец.
    caps: Vec<ir::ObjectId>,
    /// Функции полей каждого cap.
    cap_fields: Vec<Vec<ir::FuncId>>,
}

impl Layout {
    fn fn_class(&self, func: ir::FuncId, thunk: bool) -> &FnLayout {
        self.fns
            .iter()
            .find(|layout| layout.func == func && layout.thunk == thunk)
            .expect("layout collects every fn.closure target and async body")
    }

    fn fn_ctor_descriptor(&self, layout: &FnLayout) -> String {
        let mut descriptor = String::from("(");
        for ty in &layout.captures {
            descriptor.push_str(&self.descriptor(ty));
        }
        descriptor.push_str(")V");
        descriptor
    }
}

impl Layout {
    fn descriptor(&self, ty: &Type) -> String {
        match ty {
            Type::Int => BIG_D.to_string(),
            Type::Bool => "Z".to_string(),
            Type::Byte | Type::Char => "I".to_string(),
            Type::Float => format!("L{};", runtime::DOUBLE),
            Type::Rat => format!("L{};", runtime::RAT),
            Type::Str => STRING_D.to_string(),
            Type::Object(object) | Type::Successor(object) => {
                format!("L{};", self.objects[object.0].class)
            }
            Type::Role(role) => format!("L{};", self.roles[role.0].class),
            Type::Enum(_) => format!("L{};", runtime::CASE),
            Type::Region => format!("L{};", runtime::REGION),
            // Native object — значение класса provider; программа его не читает.
            Type::Handle(_) => "Ljava/lang/Object;".to_string(),
            Type::Any => format!("L{};", runtime::ANY),
            Type::Range => format!("L{};", runtime::RANGE),
            Type::Fn(_) => format!("L{};", runtime::FN),
            Type::Future(_) => format!("L{};", runtime::FUTURE),
            Type::Cap(id) => format!("L{};", self.objects[self.caps[id.0].0].class),
            Type::Failure => format!("L{};", runtime::FAILURE),
            Type::Array(_) | Type::Builder(_) => format!("L{};", runtime::LIST),
            Type::Map(..) | Type::MapBuilder(..) => format!("L{};", runtime::MAP),
            Type::Never => "Ljava/lang/Object;".to_string(),
        }
    }

    fn signature(&self, params: &[&Type], result: &ResultType) -> String {
        let mut text = String::from("(");
        for param in params {
            text.push_str(&self.descriptor(param));
        }
        text.push(')');
        match result {
            ResultType::Value(ty) => text.push_str(&self.descriptor(ty)),
            ResultType::Void => text.push('V'),
        }
        text
    }

    fn method_descriptor(&self, function: &ir::Function) -> String {
        let params: Vec<&Type> = function
            .params
            .iter()
            .map(|param| &function.values[param.0])
            .collect();
        self.signature(&params, &function.call_result())
    }
}

/// Class-файлы модуля: главный класс первым, затем вспомогательные.
/// Имена файлов относительны каталогу главного класса.
pub fn generate(module: &ir::Module, class_name: &str) -> Result<Vec<(String, Vec<u8>)>> {
    validate_name(class_name, false)?;
    ir::verify(module).map_err(|verify| error(format!("invalid IR: {verify}")))?;
    if !module.imports.is_empty() {
        return Err(error(
            "unresolved imports are not supported by the JVM backend",
        ));
    }
    // Метод JVM ограничен 64 КБ: большие функции делятся (`outline`).
    let mut split = module.clone();
    outline::split_large(&mut split);
    let module = &split;
    // Тестовый профиль исполняет тесты, `main` ему не нужен (§14).
    let entry = match (module.entry, &module.tests) {
        (Some(entry), _) => Some(entry),
        (None, Some(_)) => None,
        (None, None) => return Err(error("an executable class requires main() is int")),
    };
    if module.functions.len() >= usize::from(u16::MAX) {
        return Err(error("class has too many methods"));
    }
    let layout = layout(module, class_name, entry)?;

    let mut class = runtime::ClassGen::new(layout);
    let names = ClassNames {
        access: 0x0031, // public final super
        this_class: class.pool.class(class_name)?,
        super_class: class.pool.class("java/lang/Object")?,
        interfaces: Vec::new(),
        code: class.pool.utf8("Code")?,
        stack_map_table: class.pool.utf8("StackMapTable")?,
    };
    let mut methods = Vec::new();
    for (index, function) in module.functions.iter().enumerate() {
        methods.push(function::emit(&mut class, index, function)?);
        if function.is_async()
            && function.intrinsic().is_none()
            && function.extern_symbol().is_none()
        {
            methods.push(class.async_stub(index, function)?);
        }
    }
    match (&module.tests, entry) {
        (Some(tests), _) => methods.push(class.test_launcher(tests)?),
        (None, Some(entry)) => {
            let entry_name = class.layout.signatures[entry.0].0.clone();
            let entry_async = module.functions[entry.0].is_async();
            methods.push(class.launcher(&entry_name, entry_async)?);
        }
        (None, None) => unreachable!("checked above"),
    }
    let main_file = classfile::write_class(&class.pool, &names, &[], &methods)?;

    let layout = &class.layout;
    let mut files = vec![(format!("{class_name}.class"), main_file)];
    for decl in &layout.enums {
        files.push((format!("{}.class", decl.class), classes::enum_class(decl)?));
    }
    for object in layout.objects.iter().filter(|object| {
        object.class != runtime::NATIVE_FAILURE && object.class != runtime::CANCELLED
    }) {
        files.push((
            format!("{}.class", object.class),
            classes::object_class(layout, object)?,
        ));
    }
    for fn_layout in &layout.fns {
        files.push((
            format!("{}.class", fn_layout.class),
            classes::fn_class(layout, fn_layout)?,
        ));
    }
    for role in layout.roles.iter().filter(|role| !role.is_error) {
        files.push((
            format!("{}.class", role.class),
            classes::role_interface(role)?,
        ));
    }
    for (name, bytes) in runtime::RUNTIME_FILES {
        files.push((name.to_string(), bytes.to_vec()));
    }
    Ok(files)
}

fn layout(module: &ir::Module, class_name: &str, entry: Option<ir::FuncId>) -> Result<Layout> {
    let mut class_names = BTreeSet::new();
    let mut class_for = |name: &str| -> Result<String> {
        // Имя импортированного объявления квалифицировано (`io.stdout::Stdout`):
        // класс `Main$io$stdout$$Stdout`, диагностика берёт часть после последнего `$`.
        let class = format!(
            "{class_name}${}",
            name.replace(['.', ':', '<', '>', ',', ' ', '@', '%', '^'], "$")
        );
        validate_name(&class, false)?;
        if !class_names.insert(class.clone()) {
            return Err(error(format!("duplicate JVM class name `{class}`")));
        }
        Ok(class)
    };
    let mut enums = Vec::new();
    for decl in &module.enums {
        for case in &decl.cases {
            validate_name(case, true)?;
        }
        enums.push(EnumLayout {
            class: class_for(&decl.name)?,
            name: decl
                .name
                .rsplit("::")
                .next()
                .unwrap_or(&decl.name)
                .to_string(),
            cases: decl.cases.clone(),
        });
    }
    let mut layout = Layout {
        main: class_name.to_string(),
        enums,
        objects: Vec::new(),
        roles: Vec::new(),
        signatures: Vec::new(),
        fns: Vec::new(),
        bodies: Vec::new(),
        caps: module.caps.iter().map(|cap| cap.owner).collect(),
        cap_fields: module
            .caps
            .iter()
            .map(|cap| cap.fields.iter().map(|(_, func)| *func).collect())
            .collect(),
    };
    for object in &module.objects {
        layout.objects.push(ObjectLayout {
            // Класс системного объекта даёт рантайм (`okkama.rt.NativeFailure`).
            class: if object.is_native_failure() {
                runtime::NATIVE_FAILURE.to_string()
            } else if object.builtin && object.name == "Cancelled" {
                runtime::CANCELLED.to_string()
            } else {
                class_for(&object.name)?
            },
            fields: Vec::new(),
            interfaces: Vec::new(),
            bridges: Vec::new(),
            refs: object
                .fields
                .iter()
                .enumerate()
                .filter(|(_, field)| {
                    matches!(
                        field.ty,
                        Type::Object(_)
                            | Type::Role(_)
                            | Type::Array(_)
                            | Type::Map(..)
                            | Type::Any
                    )
                })
                .map(|(index, _)| index)
                .collect(),
            destroy: object.destroy,
        });
    }
    for role in &module.roles {
        layout.roles.push(RoleLayout {
            class: if role.is_error() {
                runtime::ERROR.to_string()
            } else {
                class_for(&role.name)?
            },
            methods: Vec::new(),
            is_error: role.is_error(),
        });
    }
    // Дескрипторы ссылаются на имена всех классов: второй проход.
    for (index, role) in module.roles.iter().enumerate() {
        let mut methods = Vec::new();
        for method in &role.methods {
            validate_name(&method.name, true)?;
            let params: Vec<&Type> = method.params.iter().collect();
            methods.push((
                method.name.clone(),
                layout.signature(&params, &method.result),
            ));
        }
        layout.roles[index].methods = methods;
    }
    for (index, object) in module.objects.iter().enumerate() {
        let fields = object
            .fields
            .iter()
            .map(|field| (field.name.clone(), layout.descriptor(&field.ty)))
            .collect();
        let mut bridges: Vec<Bridge> = Vec::new();
        for witness in &object.does {
            for ((name, descriptor), func) in layout.roles[witness.role.0]
                .methods
                .iter()
                .zip(&witness.slots)
            {
                // Одинаковый слот двух ролей реализуется одним мостом.
                if !bridges
                    .iter()
                    .any(|bridge| bridge.name == *name && bridge.descriptor == *descriptor)
                {
                    bridges.push(Bridge {
                        name: name.clone(),
                        descriptor: descriptor.clone(),
                        target: *func,
                    });
                }
            }
        }
        let object_layout = &mut layout.objects[index];
        object_layout.fields = fields;
        object_layout.interfaces = object.does.iter().map(|witness| witness.role.0).collect();
        object_layout.bridges = bridges;
    }
    let mut method_names = BTreeSet::new();
    for (index, function) in module.functions.iter().enumerate() {
        let name = method_name(
            &function.symbol,
            entry.is_some_and(|entry| entry.0 == index),
        );
        validate_name(&name, true)?;
        if !method_names.insert(name.clone()) {
            return Err(error(format!("duplicate JVM method name `{name}`")));
        }
        let descriptor = layout.method_descriptor(function);
        let body = (function.is_async()
            && function.intrinsic().is_none()
            && function.extern_symbol().is_none())
        .then(|| {
            let params: Vec<&Type> = function
                .params
                .iter()
                .map(|param| &function.values[param.0])
                .collect();
            (
                format!("{name}$body"),
                layout.signature(&params, &function.result),
            )
        });
        layout.signatures.push((name, descriptor));
        layout.bodies.push(body);
    }
    for (index, function) in module.functions.iter().enumerate() {
        let Some(target) = layout.bodies[index].clone() else {
            continue;
        };
        let class = format!("{class_name}$fn${}", layout.fns.len());
        validate_name(&class, false)?;
        layout.fns.push(FnLayout {
            class,
            func: ir::FuncId(index),
            thunk: true,
            captures: function
                .params
                .iter()
                .map(|param| function.values[param.0].clone())
                .collect(),
            params: Vec::new(),
            result: function.result.clone(),
            target,
        });
    }
    // Тест — значение-функция без захватов: его вызывает исполнитель.
    for (_, func) in module.tests.iter().flatten() {
        let target = &module.functions[func.0];
        let class = format!("{class_name}$fn${}", layout.fns.len());
        validate_name(&class, false)?;
        layout.fns.push(FnLayout {
            class,
            func: *func,
            thunk: false,
            captures: Vec::new(),
            params: Vec::new(),
            result: target.call_result(),
            target: layout.signatures[func.0].clone(),
        });
    }
    // Переходники intrinsic: значения-функции без захватов.
    for function in &module.functions {
        for func in function.intrinsic_helpers() {
            if layout
                .fns
                .iter()
                .any(|known| known.func == *func && !known.thunk)
            {
                continue;
            }
            let target = &module.functions[func.0];
            let class = format!("{class_name}$fn${}", layout.fns.len());
            validate_name(&class, false)?;
            layout.fns.push(FnLayout {
                class,
                func: *func,
                thunk: false,
                captures: Vec::new(),
                params: target
                    .params
                    .iter()
                    .map(|param| target.values[param.0].clone())
                    .collect(),
                result: target.call_result(),
                target: layout.signatures[func.0].clone(),
            });
        }
    }
    for function in &module.functions {
        for block in &function.blocks {
            for inst in &block.insts {
                let InstKind::FnClosure { func, captures } = &inst.kind else {
                    continue;
                };
                if layout
                    .fns
                    .iter()
                    .any(|known| known.func == *func && !known.thunk)
                {
                    continue;
                }
                let target = &module.functions[func.0];
                let types: Vec<Type> = target
                    .params
                    .iter()
                    .map(|param| target.values[param.0].clone())
                    .collect();
                let class = format!("{class_name}$fn${}", layout.fns.len());
                validate_name(&class, false)?;
                layout.fns.push(FnLayout {
                    class,
                    func: *func,
                    thunk: false,
                    captures: types[..captures.len()].to_vec(),
                    params: types[captures.len()..].to_vec(),
                    result: target.call_result(),
                    target: layout.signatures[func.0].clone(),
                });
            }
        }
    }
    Ok(layout)
}

/// `main` → `okkamaMain`; символы вида `Purse.total#1` → `Purse$total$1`.
fn method_name(symbol: &str, entry: bool) -> String {
    if entry {
        "okkamaMain".to_string()
    } else {
        // `@io.stdout::Stdout.writeln#1` → `io$stdout$$Stdout$writeln$1`.
        symbol.replace(['.', '#', ':', '<', '>', ',', ' ', '@', '%', '^'], "$")
    }
}

fn validate_name(name: &str, method: bool) -> Result<()> {
    // Unqualified JVM names (JVMS §4.2.2), not Java source identifiers.
    if name.is_empty()
        || name.chars().any(|ch| {
            matches!(ch, '.' | ';' | '[' | '/' | '\0') || (method && matches!(ch, '<' | '>'))
        })
    {
        return Err(error(format!(
            "invalid JVM {} name `{name}`",
            if method { "method" } else { "class" }
        )));
    }
    Ok(())
}
