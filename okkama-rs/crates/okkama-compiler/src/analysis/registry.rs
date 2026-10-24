//! Таблица объявлений программы: модули, объекты, роли, enum, natives.
//!
//! Регистрация идёт в три прохода, чтобы объявления могли ссылаться друг на
//! друга в любом порядке: имена всех модулей, затем импорты, затем сигнатуры
//! (поля, конструкторы, actions, `does`). Проверки объявлений — тексты
//! Perl-анализатора; ошибки копятся, а не обрывают проход.

use std::collections::HashMap;

use super::SemanticError;
use super::ty::{ActionTy, EnumId, HandleId, ObjectId, ParamId, RoleId, Ty};
use crate::source::Span;
use crate::syntax::ast;

/// Исходный модуль: корень или импортированный (зависимости первыми).
#[derive(Clone, Debug)]
pub struct SourceModule<'a> {
    /// Путь импорта (`io/stdout`); у корня — его имя модуля.
    pub path: String,
    pub program: &'a ast::Program,
    /// Импорты: индекс импортированного модуля и объявление.
    pub imports: Vec<(usize, &'a ast::ImportDecl)>,
}

/// Имя типа в области модуля.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TypeRef {
    Object(ObjectId),
    Role(RoleId),
    Enum(EnumId),
    Handle(HandleId),
}

#[derive(Clone, Debug, Default)]
pub struct Names {
    pub types: HashMap<String, TypeRef>,
    pub natives: HashMap<String, usize>,
}

/// Пространство имён `import "p" [as alias] [take …]`.
#[derive(Clone, Debug)]
pub struct Namespace {
    pub module: usize,
    /// `as alias take A, B`: видны только взятые имена.
    pub only: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct ModuleInfo<'a> {
    pub path: String,
    /// Имя из объявления `module` (`collections/iteration`).
    pub name: String,
    pub program: &'a ast::Program,
    /// Собственные объявления: их можно взять импортом.
    pub own: Names,
    /// Видимые без квалификации имена.
    pub scope: Names,
    pub namespaces: HashMap<String, Namespace>,
    pub imports: Vec<(usize, &'a ast::ImportDecl)>,
}

#[derive(Clone, Debug)]
pub struct ParamInfo {
    pub name: String,
    /// `T: Comparing<T>` — роль-ограничение.
    pub bound: Option<Ty>,
}

#[derive(Clone, Debug)]
pub struct FieldInfo<'a> {
    pub name: String,
    pub ty: Ty,
    pub default: Option<&'a ast::Expr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum CtorShape<'a> {
    Full {
        params: Vec<(String, Ty)>,
        names: Vec<&'a ast::Name>,
        variadic: bool,
        body: &'a ast::Block,
    },
    /// `C(:a, :b)`: допустимые поля в порядке списка.
    Shorthand {
        fields: Vec<usize>,
        body: Option<&'a ast::Block>,
    },
}

#[derive(Clone, Debug)]
pub struct CtorInfo<'a> {
    pub shape: CtorShape<'a>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct MethodInfo<'a> {
    pub name: String,
    pub params: Vec<(String, Ty)>,
    /// `None` — `is void`; successor — `Ty::Successor`.
    pub result: Option<Ty>,
    pub successor: bool,
    pub private: bool,
    pub variadic: bool,
    pub is_async: bool,
    pub projection: Option<String>,
    /// Системные действия (`NativeFailure.code()`) тела не имеют.
    pub decl: Option<&'a ast::ActionDecl>,
}

#[derive(Clone, Debug)]
pub struct CapInfo<'a> {
    pub name: String,
    pub fields: Vec<(String, Ty, &'a ast::Block)>,
    pub span: Span,
    /// `cap Owner::Name` на уровне модуля: поля владельца — `Owner::field`.
    pub external: bool,
    pub decl: &'a ast::CapDecl,
}

#[derive(Debug)]
pub struct ObjectInfo<'a> {
    pub name: String,
    pub module: usize,
    pub params: Vec<ParamId>,
    pub fields: Vec<FieldInfo<'a>>,
    pub ctors: Vec<CtorInfo<'a>>,
    pub methods: Vec<MethodInfo<'a>>,
    /// Роли объекта с аргументами (в терминах параметров объекта).
    pub does: Vec<(RoleId, Vec<Ty>)>,
    pub destroy: Option<&'a ast::DestroyDecl>,
    pub caps: Vec<CapInfo<'a>>,
    pub decl: Option<&'a ast::ObjectDecl>,
}

#[derive(Clone, Debug)]
pub struct RoleMethod {
    pub name: String,
    pub params: Vec<(String, Ty)>,
    pub result: Option<Ty>,
    pub variadic: bool,
    pub is_async: bool,
}

#[derive(Debug)]
pub struct RoleInfo<'a> {
    pub name: String,
    pub module: usize,
    pub params: Vec<ParamId>,
    pub methods: Vec<RoleMethod>,
    pub decl: Option<&'a ast::RoleDecl>,
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub name: String,
    pub module: usize,
    pub cases: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct HandleInfo {
    pub name: String,
    pub module: usize,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct NativeInfo<'a> {
    pub name: String,
    pub module: usize,
    pub params: Vec<(String, Ty)>,
    pub result: Option<Ty>,
    pub variadic: bool,
    pub is_async: bool,
    pub decl: &'a ast::NativeActionDecl,
}

/// `test "name"` корневого модуля (тестовый режим, §14): имя уже с
/// владельцем (`Owner :: name`), тело и модуль.
#[derive(Clone, Debug)]
pub struct TestInfo<'a> {
    pub name: String,
    pub module: usize,
    pub is_async: bool,
    pub body: &'a ast::Block,
    pub span: Span,
}

/// Действие роли после подстановки: имя, параметры, результат, variadic, async.
pub type RoleAction = (String, Vec<Ty>, Option<Ty>, bool, bool);

/// Системные объявления, доступные без импорта.
#[derive(Copy, Clone, Debug)]
pub struct System {
    pub error: RoleId,
    pub native_failure: ObjectId,
    pub cancelled: ObjectId,
}

/// Адаптеры встроенных значений к стандартным ролям (массив как
/// `Sequencing<E>`, скаляр как `Comparing<T>`): системные объекты без
/// исходника, чьи конструктор и actions порождает lowering. `S` — элемент
/// массива, `T` — элемент роли (массив ковариантен, §12).
#[derive(Copy, Clone, Debug, Default)]
pub struct Adapters {
    /// `ArraySequence<S, T>`: `items: @S`; Sequencing<T>, Iterating<T>, Sizing.
    pub array_sequence: Option<ObjectId>,
    /// `ArrayCursor<S, T>`: `items: @S`, `at: int`; Cursor<T>.
    pub array_cursor: Option<ObjectId>,
    /// `ScalarOrder<T>`: `value: T`; Comparing<T>.
    pub scalar_order: Option<ObjectId>,
    /// `ByteArraySource`: `items: @byte`; ByteSource.
    pub byte_array_source: Option<ObjectId>,
    /// `StringSource`: `value: string`; ByteSource (строгий UTF-8).
    pub string_source: Option<ObjectId>,
}

/// Стандартные роли, которые язык узнаёт номинально по модулю (§6.6, §8.4.1, §9.9).
#[derive(Copy, Clone, Debug, Default)]
pub struct Standard {
    pub comparing: Option<RoleId>,
    pub order: Option<EnumId>,
    pub iterating: Option<RoleId>,
    pub cursor: Option<RoleId>,
    pub sequencing: Option<RoleId>,
    pub sizing: Option<RoleId>,
    pub byte_source: Option<RoleId>,
    pub writing: Option<RoleId>,
}

#[derive(Debug)]
pub struct Registry<'a> {
    pub modules: Vec<ModuleInfo<'a>>,
    pub objects: Vec<ObjectInfo<'a>>,
    pub roles: Vec<RoleInfo<'a>>,
    pub enums: Vec<EnumInfo>,
    pub handles: Vec<HandleInfo>,
    pub natives: Vec<NativeInfo<'a>>,
    pub params: Vec<ParamInfo>,
    pub system: System,
    pub standard: Standard,
    pub adapters: Adapters,
    /// `main` корневого модуля.
    pub entry: Option<&'a ast::ActionDecl>,
    /// Файл корня, объявивший `main` (по умолчанию — последний модуль).
    pub entry_module: usize,
    /// Первый файл корневого модуля: файлы `root_start..` — одно
    /// пространство объявлений с раздельными импортами (§5.1).
    pub root_start: usize,
    /// Тестовый режим: fakes и тесты разобраны (`prune::desugar_test_items`).
    pub test_mode: bool,
    /// Тесты корневого модуля в порядке исходника.
    pub tests: Vec<TestInfo<'a>>,
    /// Программа запускает синхронное значение-action в воркере
    /// (`spawn action(…)`, §18.1): bound actions несут вход воркера.
    pub worker_actions: bool,
    pub errors: Vec<SemanticError>,
    pub warnings: Vec<SemanticError>,
    /// Модуль, в чьём файле сейчас сообщаются ошибки.
    current: usize,
    /// Применённые типы с ограничениями — проверяются после `does`.
    pending_bounds: Vec<(usize, Ty, Span)>,
}

/// Объекты std без конструктора: их создаёт только рантайм.
const RUNTIME_CONSTRUCTED: [(&str, &str); 3] = [
    ("async/lock", "LockGuard"),
    ("async/semaphore", "Permit"),
    ("async/barrier", "BarrierState"),
];

impl<'a> Registry<'a> {
    pub fn build(sources: Vec<SourceModule<'a>>) -> Registry<'a> {
        Self::build_files(sources, 1)
    }

    /// Последние `roots` источников — файлы корневого модуля.
    pub fn build_files(sources: Vec<SourceModule<'a>>, roots: usize) -> Registry<'a> {
        Self::build_mode(sources, roots, false)
    }

    /// `test_mode` — тесты корня регистрируются, `Role.fake()` доступен.
    pub fn build_mode(
        sources: Vec<SourceModule<'a>>,
        roots: usize,
        test_mode: bool,
    ) -> Registry<'a> {
        let root_start = sources.len().saturating_sub(roots.max(1));
        let mut registry = Registry {
            modules: sources
                .into_iter()
                .map(|source| ModuleInfo {
                    name: module_name(source.program),
                    path: source.path,
                    program: source.program,
                    own: Names::default(),
                    scope: Names::default(),
                    namespaces: HashMap::new(),
                    imports: source.imports,
                })
                .collect(),
            objects: Vec::new(),
            roles: Vec::new(),
            enums: Vec::new(),
            handles: Vec::new(),
            natives: Vec::new(),
            params: Vec::new(),
            system: System {
                error: RoleId(0),
                native_failure: ObjectId(0),
                cancelled: ObjectId(1),
            },
            standard: Standard::default(),
            adapters: Adapters::default(),
            entry: None,
            worker_actions: false,
            entry_module: 0,
            root_start,
            test_mode,
            tests: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            current: 0,
            pending_bounds: Vec::new(),
        };
        registry.entry_module = registry.root();
        registry.add_system();
        let count = registry.modules.len();
        for module in 0..count {
            registry.current = module;
            registry.register_names(module);
        }
        registry.share_root_names();
        for module in 0..count {
            registry.current = module;
            registry.expose_imports(module);
        }
        for module in 0..count {
            registry.current = module;
            registry.collect_signatures(module);
        }
        for module in 0..count {
            registry.current = module;
            registry.collect_objects(module);
        }
        for module in 0..count {
            registry.current = module;
            registry.collect_does(module);
        }
        registry.find_standard();
        registry.add_adapters();
        for (module, ty, span) in std::mem::take(&mut registry.pending_bounds) {
            registry.current = module;
            registry.check_bounds(&ty, span);
        }
        registry.current = registry.root();
        registry
    }

    pub fn root(&self) -> usize {
        self.modules.len() - 1
    }

    /// Файл корневого модуля (их несколько у многофайлового модуля).
    pub fn is_root(&self, module: usize) -> bool {
        module >= self.root_start
    }

    /// Файлы корня — одно пространство объявлений (§5.1): имя и версия
    /// совпадают, объявления одного файла видны в остальных, повтор имени —
    /// ошибка. Импорты остаются у своего файла.
    fn share_root_names(&mut self) {
        let roots: Vec<usize> = (self.root_start..self.modules.len()).collect();
        let Some((&first, rest)) = roots.split_first() else {
            return;
        };
        let first_program = self.modules[first].program;
        for &module in rest {
            self.current = module;
            let program = self.modules[module].program;
            let (name, other) = (&self.modules[first].name, &self.modules[module].name);
            if name != other {
                let message =
                    format!("source files declare different modules \"{name}\" and \"{other}\"");
                self.error(program.module.span, message);
                continue;
            }
            let (version, other) = (
                first_program.module.version.text(),
                program.module.version.text(),
            );
            if version != other {
                let message = format!(
                    "module \"{name}\" has conflicting versions \"{version}\" and \"{other}\""
                );
                self.error(program.module.span, message);
            }
        }
        // Повтор имени из предыдущего файла сообщается у позднего объявления.
        for (position, &module) in roots.iter().enumerate() {
            self.current = module;
            let program = self.modules[module].program;
            for item in &program.items {
                let (name, kind) = match item {
                    ast::Item::Object(decl) => (&decl.name, "Object"),
                    ast::Item::Role(decl) => (&decl.name, "Role"),
                    ast::Item::Enum(decl) => (&decl.name, "Enum"),
                    ast::Item::NativeObject(decl) => (&decl.name, "Native object type"),
                    _ => continue,
                };
                if roots[..position]
                    .iter()
                    .any(|earlier| self.modules[*earlier].own.types.contains_key(&name.text))
                {
                    self.error(
                        name.span,
                        format!("{kind} \"{}\" is already declared", name.text),
                    );
                }
            }
        }
        for &module in &roots {
            for &sibling in &roots {
                if sibling == module {
                    continue;
                }
                let own = self.modules[sibling].own.clone();
                let scope = &mut self.modules[module].scope;
                for (name, found) in own.types {
                    scope.types.entry(name).or_insert(found);
                }
                for (name, found) in own.natives {
                    scope.natives.entry(name).or_insert(found);
                }
            }
        }
    }

    pub fn set_module(&mut self, module: usize) {
        self.current = module;
    }

    pub fn current_module(&self) -> usize {
        self.current
    }

    pub fn error(&mut self, span: Span, message: impl Into<String>) {
        self.errors
            .push(SemanticError::invalid(self.current, span, message));
    }

    pub fn warn(&mut self, span: Span, message: impl Into<String>) {
        self.warnings
            .push(SemanticError::warning(self.current, span, message));
    }

    pub fn unsupported(&mut self, span: Span, message: impl Into<String>) {
        self.errors
            .push(SemanticError::unsupported(self.current, span, message));
    }

    // ----- системные объявления -----

    fn add_system(&mut self) {
        let module = self.root();
        self.roles.push(RoleInfo {
            name: "Error".to_string(),
            module,
            params: Vec::new(),
            methods: vec![RoleMethod {
                name: "text".to_string(),
                params: Vec::new(),
                result: Some(Ty::Str),
                variadic: false,
                is_async: false,
            }],
            decl: None,
        });
        let getter = |name: &str| MethodInfo {
            name: name.to_string(),
            params: Vec::new(),
            result: Some(Ty::Str),
            successor: false,
            private: false,
            variadic: false,
            is_async: false,
            projection: None,
            decl: None,
        };
        for (name, methods) in [
            (
                "NativeFailure",
                vec!["domain", "code", "operation", "message", "text"],
            ),
            ("Cancelled", vec!["text"]),
        ] {
            self.objects.push(ObjectInfo {
                name: name.to_string(),
                module,
                params: Vec::new(),
                fields: Vec::new(),
                ctors: Vec::new(),
                methods: methods.into_iter().map(getter).collect(),
                does: vec![(RoleId(0), Vec::new())],
                destroy: None,
                caps: Vec::new(),
                decl: None,
            });
        }
    }

    /// Системное имя типа: видно без импорта, если его не перекрыло своё.
    fn system_type(&self, name: &str) -> Option<Ty> {
        match name {
            "Error" => Some(Ty::role(self.system.error)),
            "NativeFailure" => Some(Ty::object(self.system.native_failure)),
            "Cancelled" => Some(Ty::object(self.system.cancelled)),
            _ => None,
        }
    }

    pub fn is_system_object(&self, id: ObjectId) -> bool {
        id == self.system.native_failure || id == self.system.cancelled || self.is_adapter(id)
    }

    pub fn is_adapter(&self, id: ObjectId) -> bool {
        let adapters = self.adapters;
        [
            adapters.array_sequence,
            adapters.array_cursor,
            adapters.scalar_order,
            adapters.byte_array_source,
            adapters.string_source,
        ]
        .contains(&Some(id))
    }

    // ----- проход 1: имена -----

    fn new_params(&mut self, generics: &[ast::GenericParam]) -> Vec<ParamId> {
        generics
            .iter()
            .map(|param| {
                self.params.push(ParamInfo {
                    name: param.name.text.clone(),
                    bound: None,
                });
                ParamId(self.params.len() - 1)
            })
            .collect()
    }

    fn own_type(&self, module: usize, name: &str) -> Option<TypeRef> {
        self.modules[module].own.types.get(name).copied()
    }

    fn register_names(&mut self, module: usize) {
        let program = self.modules[module].program;
        for item in &program.items {
            match item {
                ast::Item::Object(decl) => {
                    let name = &decl.name.text;
                    if name == "Error" {
                        self.error(decl.name.span, "\"Error\" is the name of the system role");
                        continue;
                    }
                    match self.own_type(module, name) {
                        Some(TypeRef::Handle(_)) => {
                            self.error(
                                decl.name.span,
                                format!("Type name \"{name}\" conflicts with another declaration"),
                            );
                            continue;
                        }
                        Some(_) => {
                            self.error(
                                decl.name.span,
                                format!("Object \"{name}\" is already declared"),
                            );
                            continue;
                        }
                        None => {}
                    }
                    let params = self.new_params(&decl.generics);
                    let id = ObjectId(self.objects.len());
                    self.objects.push(ObjectInfo {
                        name: name.clone(),
                        module,
                        params,
                        fields: Vec::new(),
                        ctors: Vec::new(),
                        methods: Vec::new(),
                        does: Vec::new(),
                        destroy: None,
                        caps: Vec::new(),
                        decl: Some(decl),
                    });
                    // Имена caps известны раньше типов: `Owner::Cap` — тип.
                    for member in &decl.members {
                        if let ast::ObjectMember::Cap(cap) = member {
                            self.add_cap(id, cap, false);
                        }
                    }
                    self.modules[module]
                        .own
                        .types
                        .insert(name.clone(), TypeRef::Object(id));
                }
                ast::Item::Role(decl) => {
                    let name = &decl.name.text;
                    if name == "Error" || self.own_type(module, name).is_some() {
                        self.error(
                            decl.name.span,
                            format!("Role \"{name}\" is already declared"),
                        );
                        continue;
                    }
                    let params = self.new_params(&decl.generics);
                    let id = RoleId(self.roles.len());
                    self.roles.push(RoleInfo {
                        name: name.clone(),
                        module,
                        params,
                        methods: Vec::new(),
                        decl: Some(decl),
                    });
                    self.modules[module]
                        .own
                        .types
                        .insert(name.clone(), TypeRef::Role(id));
                }
                ast::Item::Enum(decl) => {
                    let name = &decl.name.text;
                    if name == "Error" || self.own_type(module, name).is_some() {
                        self.error(
                            decl.name.span,
                            format!("Enum \"{name}\" is already declared"),
                        );
                        continue;
                    }
                    let mut cases: Vec<String> = Vec::new();
                    for case in &decl.cases {
                        if cases.contains(&case.text) {
                            self.error(
                                case.span,
                                format!("Enum \"{name}\" already has case \"{}\"", case.text),
                            );
                            continue;
                        }
                        cases.push(case.text.clone());
                    }
                    let id = EnumId(self.enums.len());
                    self.enums.push(EnumInfo {
                        name: name.clone(),
                        module,
                        cases,
                        span: decl.span,
                    });
                    self.modules[module]
                        .own
                        .types
                        .insert(name.clone(), TypeRef::Enum(id));
                }
                ast::Item::NativeObject(decl) => {
                    let name = &decl.name.text;
                    if name == "Error" {
                        self.error(
                            decl.name.span,
                            "Type name \"Error\" is reserved for the system Error role",
                        );
                        continue;
                    }
                    if let Some(existing) = self.own_type(module, name) {
                        let message = match existing {
                            TypeRef::Handle(_) => {
                                format!("Native object type \"{name}\" is already declared")
                            }
                            _ => format!("Type name \"{name}\" conflicts with another declaration"),
                        };
                        self.error(decl.name.span, message);
                        continue;
                    }
                    let id = HandleId(self.handles.len());
                    self.handles.push(HandleInfo {
                        name: name.clone(),
                        module,
                        span: decl.span,
                    });
                    self.modules[module]
                        .own
                        .types
                        .insert(name.clone(), TypeRef::Handle(id));
                }
                ast::Item::NativeAction(decl) => {
                    let name = &decl.name.text;
                    if matches!(name.as_str(), "int" | "string" | "byte" | "float" | "rat") {
                        self.error(
                            decl.name.span,
                            format!(
                                "Native declaration \"{name}\" conflicts with builtin callable \"{name}\""
                            ),
                        );
                        continue;
                    }
                    if self.modules[module].own.natives.contains_key(name) {
                        self.error(
                            decl.name.span,
                            format!("Native declaration \"{name}\" is already visible"),
                        );
                        continue;
                    }
                    let index = self.natives.len();
                    self.natives.push(NativeInfo {
                        name: name.clone(),
                        module,
                        params: Vec::new(),
                        result: None,
                        variadic: false,
                        is_async: decl.is_async,
                        decl,
                    });
                    self.modules[module].own.natives.insert(name.clone(), index);
                }
                ast::Item::Action(_) | ast::Item::Test(_) | ast::Item::Cap(_) => {}
            }
        }
        // Внешние caps — после всех имён модуля: владелец может идти позже.
        for item in &program.items {
            let ast::Item::Cap(cap) = item else {
                continue;
            };
            let owner = cap
                .owner
                .as_ref()
                .expect("module-scope caps name their owner");
            match self.own_type(module, &owner.text) {
                Some(TypeRef::Object(id)) => self.add_cap(id, cap, true),
                _ => self.error(
                    owner.span,
                    format!(
                        "Cap owner \"{}\" must be an object declared in this module",
                        owner.text
                    ),
                ),
            }
        }
        let own = self.modules[module].own.clone();
        self.modules[module].scope = own;
    }

    fn add_cap(&mut self, object: ObjectId, cap: &'a ast::CapDecl, external: bool) {
        if self.objects[object.0]
            .caps
            .iter()
            .any(|other| other.name == cap.name.text)
        {
            self.error(cap.name.span, format!("Duplicate cap {}", cap.name.text));
            return;
        }
        self.objects[object.0].caps.push(CapInfo {
            name: cap.name.text.clone(),
            fields: Vec::new(),
            span: cap.span,
            external,
            decl: cap,
        });
    }

    /// Типы вычисляемых полей caps объекта.
    fn collect_caps(&mut self, object: ObjectId) {
        let scope = self.object_scope(object);
        for index in 0..self.objects[object.0].caps.len() {
            let decl = self.objects[object.0].caps[index].decl;
            let mut fields: Vec<(String, Ty, &'a ast::Block)> = Vec::new();
            for field in &decl.fields {
                if fields.iter().any(|(other, _, _)| *other == field.name.text) {
                    self.error(
                        field.name.span,
                        format!("Duplicate computed field {}", field.name.text),
                    );
                    continue;
                }
                let ty = self.resolve(&field.ty, &scope);
                fields.push((field.name.text.clone(), ty, &field.body));
            }
            self.objects[object.0].caps[index].fields = fields;
        }
    }

    // ----- проход 2: импорты -----

    fn expose_imports(&mut self, module: usize) {
        let imports = self.modules[module].imports.clone();
        for (dependency, import) in imports {
            let path = import.path.literal.value.clone();
            let mut taken: Vec<String> = Vec::new();
            for name in &import.take {
                if taken.contains(&name.text) {
                    self.error(
                        name.span,
                        format!("Import \"{path}\" already takes \"{}\"", name.text),
                    );
                    continue;
                }
                let own = &self.modules[dependency].own;
                if !own.types.contains_key(&name.text) && !own.natives.contains_key(&name.text) {
                    self.error(
                        name.span,
                        format!("Import \"{path}\" has no declaration \"{}\"", name.text),
                    );
                    continue;
                }
                taken.push(name.text.clone());
            }
            match &import.alias {
                // `take` без `as`: только голые имена, без пространства имён.
                None if !import.take.is_empty() => {
                    for text in &taken {
                        self.bind_taken(module, dependency, text, import.span);
                    }
                }
                alias => {
                    let namespace = alias
                        .as_ref()
                        .map_or_else(|| last_segment(&path), |alias| alias.text.clone());
                    let only = (!import.take.is_empty()).then(|| taken.clone());
                    if let Some(existing) = self.modules[module].namespaces.get(&namespace) {
                        if existing.module != dependency {
                            self.error(
                                import.span,
                                format!("Namespace \"{namespace}\" is already declared"),
                            );
                            continue;
                        }
                    }
                    self.modules[module].namespaces.insert(
                        namespace,
                        Namespace {
                            module: dependency,
                            only,
                        },
                    );
                }
            }
        }
    }

    fn bind_taken(&mut self, module: usize, dependency: usize, text: &str, span: Span) {
        let own = self.modules[dependency].own.clone();
        if let Some(found) = own.types.get(text) {
            let scope = &self.modules[module].scope;
            if let Some(existing) = scope.types.get(text) {
                if existing != found {
                    let kind = match existing {
                        TypeRef::Object(_) => "Object",
                        TypeRef::Role(_) => "Role",
                        TypeRef::Enum(_) => "Enum",
                        TypeRef::Handle(_) => "Type",
                    };
                    self.error(span, format!("{kind} \"{text}\" is already declared"));
                    return;
                }
            }
            self.modules[module]
                .scope
                .types
                .insert(text.to_string(), *found);
        }
        if let Some(found) = own.natives.get(text) {
            let scope = &self.modules[module].scope;
            if scope
                .natives
                .get(text)
                .is_some_and(|existing| existing != found)
            {
                self.error(
                    span,
                    format!("Native declaration \"{text}\" is already visible"),
                );
                return;
            }
            self.modules[module]
                .scope
                .natives
                .insert(text.to_string(), *found);
        }
    }

    // ----- разрешение имён -----

    /// Тип по имени в области модуля `module` (`None` — не объявлен).
    pub fn find_type(&self, module: usize, name: &str) -> Option<TypeRef> {
        self.modules[module].scope.types.get(name).copied()
    }

    /// `ns.Name`: собственное объявление модуля пространства имён.
    pub fn find_qualified(&self, module: usize, namespace: &str, name: &str) -> Option<TypeRef> {
        let space = self.modules[module].namespaces.get(namespace)?;
        if space
            .only
            .as_ref()
            .is_some_and(|only| !only.iter().any(|taken| taken == name))
        {
            return None;
        }
        self.modules[space.module].own.types.get(name).copied()
    }

    pub fn find_native(&self, module: usize, name: &str) -> Option<usize> {
        self.modules[module].scope.natives.get(name).copied()
    }

    pub fn find_qualified_native(
        &self,
        module: usize,
        namespace: &str,
        name: &str,
    ) -> Option<usize> {
        let space = self.modules[module].namespaces.get(namespace)?;
        if space
            .only
            .as_ref()
            .is_some_and(|only| !only.iter().any(|taken| taken == name))
        {
            return None;
        }
        self.modules[space.module].own.natives.get(name).copied()
    }

    pub fn is_namespace(&self, module: usize, name: &str) -> bool {
        self.modules[module].namespaces.contains_key(name)
    }

    /// Тип, записанный в объявлении или теле. `params` — видимые параметры
    /// типа (имя → параметр). Ошибка сообщается, результат — `Ty::Error`.
    pub fn resolve(&mut self, ty: &ast::TypeExpr, params: &[(String, ParamId)]) -> Ty {
        match &ty.kind {
            ast::TypeKind::Array(element) => Ty::array(self.resolve(element, params)),
            ast::TypeKind::Map { key, value } => {
                let key_type = self.resolve(key, params);
                let value_type = self.resolve(value, params);
                if !key_type.is_error()
                    && !key_type.is_map_key()
                    && !matches!(key_type, Ty::Param(_))
                {
                    self.error(
                        key.span,
                        format!(
                            "Map key type {} is not a scalar key type",
                            self.type_name(&key_type)
                        ),
                    );
                    return Ty::Error;
                }
                Ty::map(key_type, value_type)
            }
            ast::TypeKind::Action {
                is_async,
                params: action_params,
                result,
            } => {
                let resolved = action_params
                    .iter()
                    .map(|param| self.resolve(param, params))
                    .collect();
                let result = match result {
                    ast::ActionResult::Void(_) => None,
                    ast::ActionResult::Type(result) => Some(self.resolve(result, params)),
                };
                Ty::Action(Box::new(ActionTy {
                    is_async: *is_async,
                    params: resolved,
                    result,
                }))
            }
            ast::TypeKind::Nested { owner, name } => {
                let owner_type = self.resolve(owner, params);
                match owner_type {
                    Ty::Object(id, args) => {
                        match self.objects[id.0]
                            .caps
                            .iter()
                            .position(|cap| cap.name == name.text)
                        {
                            Some(cap) => Ty::Cap(id, args, cap),
                            None => {
                                self.error(
                                    name.span,
                                    format!("Object has no cap \"{}\"", name.text),
                                );
                                Ty::Error
                            }
                        }
                    }
                    Ty::Error => Ty::Error,
                    other => {
                        self.error(
                            ty.span,
                            format!("Type {} has no caps", self.type_name(&other)),
                        );
                        Ty::Error
                    }
                }
            }
            ast::TypeKind::Named { path, args } => self.resolve_named(ty, path, args, params),
        }
    }

    fn resolve_named(
        &mut self,
        ty: &ast::TypeExpr,
        path: &[ast::Name],
        args: &[ast::TypeExpr],
        params: &[(String, ParamId)],
    ) -> Ty {
        let module = self.current;
        let found = match path {
            [name] => {
                if let Some((_, param)) = params.iter().rev().find(|(text, _)| *text == name.text) {
                    if !args.is_empty() {
                        self.error(
                            ty.span,
                            format!(
                                "Type parameter \"{}\" cannot accept type arguments",
                                name.text
                            ),
                        );
                        return Ty::Error;
                    }
                    return Ty::Param(*param);
                }
                let builtin = match name.text.as_str() {
                    "int" => Some(Ty::Int),
                    "bool" => Some(Ty::Bool),
                    "byte" => Some(Ty::Byte),
                    "float" => Some(Ty::Float),
                    "rat" => Some(Ty::Rat),
                    "char" => Some(Ty::Char),
                    "string" => Some(Ty::Str),
                    "any" => Some(Ty::Any),
                    _ => None,
                };
                if let Some(builtin) = builtin {
                    if !args.is_empty() {
                        self.error(
                            ty.span,
                            format!(
                                "Type \"{}\" is not generic and cannot accept type arguments",
                                name.text
                            ),
                        );
                        return Ty::Error;
                    }
                    return builtin;
                }
                if name.text == "Future" && self.find_type(module, "Future").is_none() {
                    let [arg] = args else {
                        self.error(
                            ty.span,
                            format!(
                                "Generic type \"Future\" expects 1 type argument(s), got {}",
                                args.len()
                            ),
                        );
                        return Ty::Error;
                    };
                    let inner = self.resolve(arg, params);
                    return Ty::Future(Some(Box::new(inner)));
                }
                match self.find_type(module, &name.text) {
                    Some(found) => found,
                    None => match self.system_type(&name.text) {
                        Some(system) => {
                            if !args.is_empty() {
                                self.error(
                                    ty.span,
                                    format!(
                                        "Type \"{}\" is not generic and cannot accept type arguments",
                                        name.text
                                    ),
                                );
                                return Ty::Error;
                            }
                            return system;
                        }
                        None => {
                            self.error(
                                name.span,
                                format!("Type \"{}\" is not declared", name.text),
                            );
                            return Ty::Error;
                        }
                    },
                }
            }
            [namespace, name] => match self.find_qualified(module, &namespace.text, &name.text) {
                Some(found) => found,
                None => {
                    let message = if self.is_namespace(module, &namespace.text) {
                        format!(
                            "Namespace \"{}\" has no type \"{}\"",
                            namespace.text, name.text
                        )
                    } else {
                        format!("Namespace \"{}\" is not imported", namespace.text)
                    };
                    self.error(ty.span, message);
                    return Ty::Error;
                }
            },
            _ => {
                self.error(ty.span, "Qualified type names have one namespace");
                return Ty::Error;
            }
        };
        let display = path.last().expect("named types have a name").text.clone();
        let arity = match found {
            TypeRef::Object(id) => self.objects[id.0].params.len(),
            TypeRef::Role(id) => self.roles[id.0].params.len(),
            TypeRef::Enum(_) | TypeRef::Handle(_) => 0,
        };
        if arity == 0 && !args.is_empty() {
            self.error(
                ty.span,
                format!("Type \"{display}\" is not generic and cannot accept type arguments"),
            );
            return Ty::Error;
        }
        if arity != args.len() {
            self.error(
                ty.span,
                format!(
                    "Generic type \"{display}\" expects {arity} type argument(s), got {}",
                    args.len()
                ),
            );
            return Ty::Error;
        }
        let resolved: Vec<Ty> = args.iter().map(|arg| self.resolve(arg, params)).collect();
        let result = match found {
            TypeRef::Object(id) => Ty::Object(id, resolved),
            TypeRef::Role(id) => Ty::Role(id, resolved),
            TypeRef::Enum(id) => Ty::Enum(id),
            TypeRef::Handle(id) => Ty::Handle(id),
        };
        if arity > 0 {
            self.pending_bounds
                .push((self.current, result.clone(), ty.span));
        }
        result
    }

    /// Ограничения параметров применённого generic-типа.
    fn check_bounds(&mut self, ty: &Ty, span: Span) {
        let Ty::Object(id, args) = ty else {
            return;
        };
        let params = self.objects[id.0].params.clone();
        let env: Vec<(ParamId, Ty)> = params.iter().copied().zip(args.iter().cloned()).collect();
        for (param, arg) in params.iter().zip(args) {
            let Some(bound) = self.params[param.0].bound.clone() else {
                continue;
            };
            let required = bound.subst(&env);
            if arg.is_error() || matches!(arg, Ty::Param(_)) {
                continue;
            }
            if !self.assignable(arg, &required) {
                let message = format!(
                    "Generic parameter \"{}\" for object \"{}\" has type {}, which does not satisfy {}",
                    self.params[param.0].name,
                    self.objects[id.0].name,
                    self.type_name(arg),
                    self.type_name(&required)
                );
                self.error(span, message);
            }
        }
    }

    // ----- проход 3: сигнатуры ролей, natives, точка входа -----

    fn param_scope(&self, params: &[ParamId]) -> Vec<(String, ParamId)> {
        params
            .iter()
            .map(|param| (self.params[param.0].name.clone(), *param))
            .collect()
    }

    fn resolve_bounds(
        &mut self,
        generics: &[ast::GenericParam],
        params: &[ParamId],
        owner: &str,
        kind: &str,
    ) {
        let scope = self.param_scope(params);
        for (generic, param) in generics.iter().zip(params) {
            let Some(bound) = &generic.bound else {
                continue;
            };
            let ty = self.resolve(bound, &scope);
            match ty {
                Ty::Role(..) => self.params[param.0].bound = Some(ty),
                Ty::Error => {}
                other => {
                    let message = format!(
                        "Generic bound \"{}\" on {kind} \"{owner}\" must be a role",
                        self.type_name(&other)
                    );
                    self.error(bound.span, message);
                }
            }
        }
    }

    fn collect_signatures(&mut self, module: usize) {
        let program = self.modules[module].program;
        let root = self.is_root(module);
        for item in &program.items {
            match item {
                ast::Item::Role(decl) => {
                    let Some(TypeRef::Role(id)) = self.own_type(module, &decl.name.text) else {
                        continue;
                    };
                    if self.roles[id.0].decl.map(|known| std::ptr::eq(known, decl)) != Some(true) {
                        continue;
                    }
                    let params = self.roles[id.0].params.clone();
                    self.resolve_bounds(&decl.generics, &params, &decl.name.text, "role");
                    self.collect_role(id, decl);
                }
                ast::Item::NativeAction(decl) => {
                    let Some(&index) = self.modules[module].own.natives.get(&decl.name.text) else {
                        continue;
                    };
                    if !std::ptr::eq(self.natives[index].decl, decl) {
                        continue;
                    }
                    let (params, variadic) = self.signature_params(&decl.params, &[]);
                    for (param, (_, ty)) in decl.params.iter().zip(&params) {
                        if contains_action(ty) {
                            self.error(
                                param.name.span,
                                format!(
                                    "Native declaration \"{}\" cannot accept action values",
                                    decl.name.text
                                ),
                            );
                        }
                    }
                    let result = self.resolve_result(&decl.result, &[]);
                    if result.as_ref().is_some_and(contains_action) {
                        self.error(
                            decl.name.span,
                            format!(
                                "Native declaration \"{}\" cannot return an action value",
                                decl.name.text
                            ),
                        );
                    }
                    let native = &mut self.natives[index];
                    native.params = params;
                    native.variadic = variadic;
                    native.result = result.map(|ty| ty.taken());
                }
                // Точка входа — только у корня; `main` библиотеки не исполняется.
                ast::Item::Action(action) if root => {
                    if self.entry.is_some() && self.entry_module != module {
                        self.error(action.name.span, "Module declares main() more than once");
                        continue;
                    }
                    if self.entry.is_some() {
                        self.error(
                            action.name.span,
                            format!(
                                "Duplicate action name \"{}\" in the module.",
                                action.name.text
                            ),
                        );
                        continue;
                    }
                    let entry_signature =
                        "The module entry point must be declared `main() is int`.";
                    if !action.params.is_empty()
                        || action.projection.is_some()
                        || action.name.text != "main"
                    {
                        self.error(action.span, entry_signature);
                    } else if let ast::ResultSpec::Type(result) = &action.result {
                        if self.resolve(result, &[]) != Ty::Int {
                            self.error(result.span, entry_signature);
                        }
                    } else {
                        self.error(action.result.span(), entry_signature);
                    }
                    self.entry = Some(action);
                    self.entry_module = module;
                }
                ast::Item::Test(test) if self.test_mode => {
                    // Исполняются только тесты корневого модуля (§14).
                    let ast::TestBody::Parsed(body) = &test.body else {
                        continue;
                    };
                    if !self.is_root(module) {
                        continue;
                    }
                    let name = test.name.literal.value.clone();
                    if self.tests.iter().any(|known| known.name == name) {
                        self.error(test.name.span, format!("Duplicate test \"{name}\""));
                        continue;
                    }
                    self.tests.push(TestInfo {
                        name,
                        module,
                        is_async: test.is_async,
                        body,
                        span: test.span,
                    });
                }
                ast::Item::Test(test) => {
                    self.unsupported(
                        test.span,
                        "Test declarations must be pruned before semantic analysis.",
                    );
                }
                _ => {}
            }
        }
    }

    /// Параметры сигнатуры; variadic-хвост `...T` — последний, его тип
    /// внутри действия — массив `@T` (§11.8.1).
    fn signature_params(
        &mut self,
        params: &[ast::Param],
        scope: &[(String, ParamId)],
    ) -> (Vec<(String, Ty)>, bool) {
        let mut resolved: Vec<(String, Ty)> = Vec::new();
        for (index, param) in params.iter().enumerate() {
            if param.variadic && index + 1 != params.len() {
                self.error(
                    param.span,
                    "A variadic parameter must be the final parameter",
                );
            }
            if resolved
                .iter()
                .any(|(existing, _)| *existing == param.name.text)
            {
                self.error(
                    param.name.span,
                    format!("Parameter \"{}\" is declared twice", param.name.text),
                );
            }
            let ty = self.resolve(&param.ty, scope);
            let ty = if param.variadic { Ty::array(ty) } else { ty };
            resolved.push((param.name.text.clone(), ty));
        }
        let variadic = params.last().is_some_and(|param| param.variadic);
        (resolved, variadic)
    }

    /// Результат действия; `^T` — `Ty::Successor(T)`.
    fn resolve_result(
        &mut self,
        result: &ast::ResultSpec,
        scope: &[(String, ParamId)],
    ) -> Option<Ty> {
        match result {
            ast::ResultSpec::Void(_) => None,
            ast::ResultSpec::Type(ty) => Some(self.resolve(ty, scope)),
            ast::ResultSpec::Successor { ty, span } => match self.resolve(ty, scope) {
                inner @ (Ty::Object(..) | Ty::Role(..)) => Some(Ty::Successor(Box::new(inner))),
                Ty::Error => Some(Ty::Error),
                _ => {
                    self.error(*span, "A successor result must name an object or role type");
                    Some(Ty::Error)
                }
            },
        }
    }

    fn collect_role(&mut self, id: RoleId, decl: &'a ast::RoleDecl) {
        let scope = self.param_scope(&self.roles[id.0].params.clone());
        let mut methods: Vec<RoleMethod> = Vec::new();
        for member in &decl.members {
            let signature = match member {
                ast::RoleMember::Signature(signature) => signature,
                ast::RoleMember::Fake(fake) => {
                    self.unsupported(
                        fake.span,
                        "Test and fake declarations must be pruned before semantic analysis.",
                    );
                    continue;
                }
                ast::RoleMember::Test(test) => {
                    self.unsupported(
                        test.span,
                        "Test and fake declarations must be pruned before semantic analysis.",
                    );
                    continue;
                }
            };
            let (params, variadic) = self.signature_params(&signature.params, &scope);
            let result = self.resolve_result(&signature.result, &scope);
            if methods.iter().any(|other| {
                other.name == signature.name.text
                    && other.params.len() == params.len()
                    && other
                        .params
                        .iter()
                        .zip(&params)
                        .all(|((_, left), (_, right))| left == right)
            }) {
                self.error(
                    signature.span,
                    format!(
                        "Role \"{}\" declares action \"{}\" twice with the same signature",
                        decl.name.text, signature.name.text
                    ),
                );
                continue;
            }
            if let Some(other) = methods
                .iter()
                .find(|other| other.name == signature.name.text)
            {
                if other.variadic || variadic {
                    self.error(
                        signature.span,
                        format!(
                            "Role \"{}\" cannot overload variadic action \"{}\"",
                            decl.name.text, signature.name.text
                        ),
                    );
                    continue;
                }
                if other.result != result {
                    self.error(
                        signature.span,
                        format!(
                            "Role action \"{}\" overloads on \"{}\" must declare the same return type",
                            signature.name.text, decl.name.text
                        ),
                    );
                    continue;
                }
            }
            methods.push(RoleMethod {
                name: signature.name.text.clone(),
                params,
                result,
                variadic,
                is_async: signature.is_async,
            });
        }
        self.roles[id.0].methods = methods;
    }

    // ----- проход 4: объекты -----

    fn collect_objects(&mut self, module: usize) {
        let program = self.modules[module].program;
        for item in &program.items {
            if let ast::Item::Object(decl) = item {
                let Some(TypeRef::Object(id)) = self.own_type(module, &decl.name.text) else {
                    continue;
                };
                if self.objects[id.0]
                    .decl
                    .map(|known| std::ptr::eq(known, decl))
                    != Some(true)
                {
                    continue;
                }
                let params = self.objects[id.0].params.clone();
                self.resolve_bounds(&decl.generics, &params, &decl.name.text, "object");
                self.collect_object(id, decl);
            }
        }
    }

    /// Тип объекта внутри его объявления: `Box<T>`.
    pub fn self_type(&self, id: ObjectId) -> Ty {
        Ty::Object(
            id,
            self.objects[id.0]
                .params
                .iter()
                .map(|param| Ty::Param(*param))
                .collect(),
        )
    }

    pub fn object_scope(&self, id: ObjectId) -> Vec<(String, ParamId)> {
        self.param_scope(&self.objects[id.0].params)
    }

    fn collect_object(&mut self, id: ObjectId, decl: &'a ast::ObjectDecl) {
        let name = decl.name.text.clone();
        let scope = self.object_scope(id);
        let self_type = self.self_type(id);
        let mut fields: Vec<FieldInfo<'a>> = Vec::new();
        let mut untyped: Vec<String> = Vec::new();
        let mut ctor_decls = Vec::new();
        let mut actions: Vec<&'a ast::ActionDecl> = Vec::new();
        let mut destroy = None;
        for member in &decl.members {
            match member {
                ast::ObjectMember::Has(has) => {
                    let ty = match &has.ty {
                        Some(ty) => self.resolve(ty, &scope),
                        None => {
                            self.unsupported(
                                has.span,
                                "Fields without an explicit type are not supported by the analysis yet.",
                            );
                            untyped.extend(has.names.iter().map(|name| name.text.clone()));
                            Ty::Error
                        }
                    };
                    for field_name in &has.names {
                        if fields.iter().any(|field| field.name == field_name.text) {
                            self.error(
                                field_name.span,
                                format!(
                                    "Field \"{}\" is already declared on object \"{name}\"",
                                    field_name.text
                                ),
                            );
                            continue;
                        }
                        fields.push(FieldInfo {
                            name: field_name.text.clone(),
                            ty: ty.clone(),
                            default: has.default.as_ref(),
                            span: field_name.span,
                        });
                    }
                }
                ast::ObjectMember::Constructor(ctor) => ctor_decls.push(ctor),
                ast::ObjectMember::Action(action) => actions.push(action),
                ast::ObjectMember::Private(section) => actions.extend(section.actions.iter()),
                ast::ObjectMember::Destroy(block) => {
                    if destroy.is_some() {
                        self.error(
                            block.span,
                            format!("Object \"{name}\" cannot declare more than one destroy block"),
                        );
                        continue;
                    }
                    destroy = Some(block);
                }
                ast::ObjectMember::Cap(_) => {}
                ast::ObjectMember::Test(test) => {
                    self.unsupported(
                        test.span,
                        "Test declarations must be pruned before semantic analysis.",
                    );
                }
            }
        }
        self.objects[id.0].fields = fields;
        self.objects[id.0].destroy = destroy;
        self.collect_caps(id);

        let module_name = self.modules[self.objects[id.0].module].name.clone();
        let runtime_constructed = RUNTIME_CONSTRUCTED
            .iter()
            .any(|(module, object)| *module == module_name && *object == name);
        if ctor_decls.is_empty() && !runtime_constructed {
            self.error(
                decl.name.span,
                format!("Object \"{name}\" must declare a constructor"),
            );
        }
        let mut ctors: Vec<CtorInfo<'a>> = Vec::new();
        for ctor in &ctor_decls {
            if ctor.name.text != decl.name.text {
                self.error(
                    ctor.name.span,
                    format!(
                        "Constructor \"{}\" must be named \"{}\"",
                        ctor.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let shape = match &ctor.form {
                ast::ConstructorForm::Full { params, body } => {
                    let (resolved, variadic) = self.signature_params(params, &scope);
                    CtorShape::Full {
                        params: resolved,
                        names: params.iter().map(|param| &param.name).collect(),
                        variadic,
                        body,
                    }
                }
                ast::ConstructorForm::Shorthand {
                    fields: listed,
                    body,
                } => {
                    let mut indices = Vec::new();
                    let mut valid = true;
                    for field in listed {
                        if untyped.contains(&field.text) {
                            self.error(
                                field.span,
                                format!(
                                    "Constructor shorthand field \"{}\" must have an explicit type",
                                    field.text
                                ),
                            );
                            valid = false;
                            continue;
                        }
                        let info = &self.objects[id.0];
                        let Some(index) = info
                            .fields
                            .iter()
                            .position(|known| known.name == field.text)
                        else {
                            let message =
                                format!("Object \"{name}\" has no field \"{}\"", field.text);
                            self.error(field.span, message);
                            valid = false;
                            continue;
                        };
                        if indices.contains(&index) {
                            self.error(
                                field.span,
                                format!("Field \"{}\" is listed twice", field.text),
                            );
                            valid = false;
                            continue;
                        }
                        indices.push(index);
                    }
                    if !valid {
                        continue;
                    }
                    if body.is_none() {
                        let missing: Vec<String> = self.objects[id.0]
                            .fields
                            .iter()
                            .enumerate()
                            .filter(|(index, field)| {
                                !indices.contains(index) && field.default.is_none()
                            })
                            .map(|(_, field)| field.name.clone())
                            .collect();
                        if let Some(field) = missing.first() {
                            self.error(
                                ctor.span,
                                format!(
                                    "Constructor shorthand for \"{name}\" cannot initialize required field \"{field}\""
                                ),
                            );
                            continue;
                        }
                    }
                    CtorShape::Shorthand {
                        fields: indices,
                        body: body.as_ref(),
                    }
                }
            };
            ctors.push(CtorInfo {
                shape,
                span: ctor.span,
            });
        }
        let variadic_ctor = ctors
            .iter()
            .any(|ctor| matches!(ctor.shape, CtorShape::Full { variadic: true, .. }));
        if variadic_ctor && ctors.len() > 1 {
            self.error(
                decl.name.span,
                format!(
                    "Object \"{name}\" cannot declare another constructor alongside variadic constructor \"{name}\""
                ),
            );
        }
        self.objects[id.0].ctors = ctors;

        let mut methods: Vec<MethodInfo<'a>> = Vec::new();
        for action in actions {
            let (params, variadic) = self.signature_params(&action.params, &scope);
            let mut result = self.resolve_result(&action.result, &scope);
            let successor = matches!(action.result, ast::ResultSpec::Successor { .. });
            let private = action.visibility == ast::Visibility::Private;
            if let Some(Ty::Successor(target)) = &result {
                let allowed = **target == self_type || matches!(&**target, Ty::Role(..));
                if !allowed {
                    self.error(
                        action.result.span(),
                        format!(
                            "Successor action \"{}\" on object \"{name}\" must return ^{name} or a role it does",
                            action.name.text
                        ),
                    );
                    result = Some(Ty::Error);
                }
            }
            let projection = match &action.projection {
                None => None,
                Some(directive) => {
                    let prefix = directive.alias.as_ref().unwrap_or(&action.name).clone();
                    let rule = |what: &str| {
                        format!(
                            "Projection action \"{}\" on object \"{name}\" {what}",
                            action.name.text
                        )
                    };
                    let violation = if !action.params.is_empty() {
                        Some(rule("cannot declare parameters"))
                    } else if successor {
                        Some(rule("cannot return a successor type"))
                    } else if private {
                        Some(rule("must be public"))
                    } else if action.is_async {
                        Some(rule("must be synchronous"))
                    } else if result.is_none() {
                        Some(rule("requires an explicit return type"))
                    } else if result.as_ref() == Some(&self_type) {
                        Some(rule(&format!(
                            "must return a type different from {}",
                            self.type_name(&self_type)
                        )))
                    } else {
                        None
                    };
                    if let Some(message) = violation {
                        self.error(action.name.span, message);
                        None
                    } else if methods
                        .iter()
                        .any(|other| other.projection.as_deref() == Some(prefix.text.as_str()))
                    {
                        self.error(
                            prefix.span,
                            format!(
                                "Object \"{name}\" declares projection prefix \"{}\" more than once",
                                prefix.text
                            ),
                        );
                        None
                    } else {
                        Some(prefix.text)
                    }
                }
            };
            if successor && private {
                self.error(
                    action.name.span,
                    "Private successor actions are not supported",
                );
            }
            let candidate = MethodInfo {
                name: action.name.text.clone(),
                params,
                result,
                successor,
                private,
                variadic,
                is_async: action.is_async,
                projection,
                decl: Some(action),
            };
            if let Some(message) = overload_conflict(&methods, &candidate, &name) {
                let span = if message.starts_with("Overloads") {
                    action.result.span()
                } else {
                    action.name.span
                };
                self.error(span, message);
                continue;
            }
            methods.push(candidate);
        }
        self.objects[id.0].methods = methods;
    }

    fn collect_does(&mut self, module: usize) {
        let program = self.modules[module].program;
        for item in &program.items {
            let ast::Item::Object(decl) = item else {
                continue;
            };
            let Some(TypeRef::Object(id)) = self.own_type(module, &decl.name.text) else {
                continue;
            };
            if self.objects[id.0]
                .decl
                .map(|known| std::ptr::eq(known, decl))
                != Some(true)
            {
                continue;
            }
            let scope = self.object_scope(id);
            let mut does: Vec<(RoleId, Vec<Ty>)> = Vec::new();
            for role_type in &decl.roles {
                let resolved = self.resolve(role_type, &scope);
                let (role, args) = match resolved {
                    Ty::Role(role, args) => (role, args),
                    Ty::Error => continue,
                    _ => {
                        self.error(role_type.span, "`does` must name a role");
                        continue;
                    }
                };
                if does.iter().any(|(declared, _)| *declared == role) {
                    self.error(
                        role_type.span,
                        format!("Role \"{}\" is listed twice", self.roles[role.0].name),
                    );
                    continue;
                }
                does.push((role, args));
            }
            // Конформность проверяется по полному списку: роль может
            // возвращать `^Role`, который объект делает.
            self.objects[id.0].does = does.clone();
            for (role, args) in &does {
                self.check_conformance(id, *role, args, decl);
            }
        }
    }

    /// `does R`: каждый action роли реализован публичным action с той же сигнатурой.
    fn check_conformance(
        &mut self,
        id: ObjectId,
        role: RoleId,
        args: &[Ty],
        decl: &ast::ObjectDecl,
    ) {
        let env: Vec<(ParamId, Ty)> = self.roles[role.0]
            .params
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .collect();
        let role_display = self.type_name(&Ty::Role(role, args.to_vec()));
        let object = decl.name.text.clone();
        let required: Vec<RoleMethod> = self.roles[role.0].methods.clone();
        for method in required {
            let params: Vec<Ty> = method.params.iter().map(|(_, ty)| ty.subst(&env)).collect();
            let result = method.result.as_ref().map(|ty| ty.subst(&env));
            let message = {
                let named: Vec<&MethodInfo<'_>> = self.objects[id.0]
                    .methods
                    .iter()
                    .filter(|candidate| candidate.name == method.name)
                    .collect();
                let same_params = |candidate: &MethodInfo<'_>| {
                    candidate.variadic == method.variadic
                        && candidate.params.len() == params.len()
                        && candidate
                            .params
                            .iter()
                            .zip(&params)
                            .all(|((_, left), right)| left == right)
                };
                if let Some(found) = named.iter().find(|candidate| {
                    same_params(candidate) && self.result_conforms(id, &candidate.result, &result)
                }) {
                    if found.private {
                        Some(format!(
                            "Role \"{role_display}\" has no public implementation of action \"{}\"",
                            method.name
                        ))
                    } else if found.is_async != method.is_async {
                        Some(format!(
                            "Action \"{}\" on object \"{object}\" must match async shape for role \"{role_display}\"",
                            method.name
                        ))
                    } else {
                        None
                    }
                } else if let Some(first) = named.first() {
                    match named
                        .iter()
                        .find(|candidate| candidate.params.len() == params.len())
                    {
                        None => Some(format!(
                            "Object \"{object}\" does role \"{role_display}\" but action \"{}\" does not accept {} parameter(s)",
                            first.name,
                            params.len()
                        )),
                        Some(candidate) => {
                            let mismatch = candidate
                                .params
                                .iter()
                                .zip(&params)
                                .enumerate()
                                .find(|(_, ((_, actual), expected))| actual != *expected);
                            Some(match mismatch {
                                Some((position, ((_, actual), expected))) => format!(
                                    "Action \"{}\" parameter {} on object \"{object}\" must be {} for role \"{role_display}\", got {}",
                                    method.name,
                                    position + 1,
                                    self.type_name(expected),
                                    self.type_name(actual)
                                ),
                                None => format!(
                                    "Action \"{}\" on object \"{object}\" must return {} for role \"{role_display}\", got {}",
                                    method.name,
                                    self.result_name(&result),
                                    self.result_name(&candidate.result)
                                ),
                            })
                        }
                    }
                } else {
                    Some(format!(
                        "Object \"{object}\" does role \"{role_display}\" but does not implement action \"{}\"",
                        method.name
                    ))
                }
            };
            if let Some(message) = message {
                self.error(decl.name.span, message);
            }
        }
    }

    /// Результат реализации подходит роли: тот же тип или `^Self` вместо `^Role`.
    fn result_conforms(&self, id: ObjectId, actual: &Option<Ty>, expected: &Option<Ty>) -> bool {
        if actual == expected {
            return true;
        }
        match (actual, expected) {
            (Some(Ty::Successor(actual)), Some(Ty::Successor(expected))) => {
                matches!(&**actual, Ty::Object(object, _) if *object == id)
                    && self.assignable(actual, expected)
            }
            _ => false,
        }
    }

    // ----- стандартные роли -----

    fn find_standard(&mut self) {
        let lookup = |registry: &Registry<'_>, module: &str, name: &str| {
            registry
                .modules
                .iter()
                .position(|info| info.name == module)
                .and_then(|index| registry.modules[index].own.types.get(name).copied())
        };
        let role = |found: Option<TypeRef>| match found {
            Some(TypeRef::Role(id)) => Some(id),
            _ => None,
        };
        self.standard = Standard {
            comparing: role(lookup(self, "compare", "Comparing")),
            order: match lookup(self, "compare", "Order") {
                Some(TypeRef::Enum(id)) => Some(id),
                _ => None,
            },
            iterating: role(lookup(self, "collections/iteration", "Iterating")),
            cursor: role(lookup(self, "collections/iteration", "Cursor")),
            sequencing: role(lookup(self, "collections/sequence", "Sequencing")),
            sizing: role(lookup(self, "size", "Sizing")),
            byte_source: role(lookup(self, "io/stream", "ByteSource")),
            writing: role(lookup(self, "io/stream", "Writing")),
        };
    }

    fn add_adapters(&mut self) {
        let module = self.root();
        let standard = self.standard;
        let method = |name: &str, params: Vec<(String, Ty)>, result: Option<Ty>| MethodInfo {
            name: name.to_string(),
            params,
            result,
            successor: false,
            private: false,
            variadic: false,
            is_async: false,
            projection: None,
            decl: None,
        };
        let field = |name: &str, ty: Ty| FieldInfo {
            name: name.to_string(),
            ty,
            default: None,
            span: Span::empty(0),
        };
        let param = |registry: &mut Self, name: &str| {
            registry.params.push(ParamInfo {
                name: name.to_string(),
                bound: None,
            });
            ParamId(registry.params.len() - 1)
        };
        let add = |registry: &mut Self,
                   name: &str,
                   params: Vec<ParamId>,
                   fields: Vec<FieldInfo<'a>>,
                   methods: Vec<MethodInfo<'a>>,
                   does: Vec<(RoleId, Vec<Ty>)>| {
            registry.objects.push(ObjectInfo {
                name: format!("core::{name}"),
                module,
                params,
                fields,
                ctors: Vec::new(),
                methods,
                does,
                destroy: None,
                caps: Vec::new(),
                decl: None,
            });
            Some(ObjectId(registry.objects.len() - 1))
        };
        let array_of = |ty: Ty| Ty::Array(Box::new(ty));
        if let Some(cursor) = standard.cursor {
            let (s, t) = (param(self, "S"), param(self, "T"));
            self.adapters.array_cursor = add(
                self,
                "ArrayCursor",
                vec![s, t],
                vec![field("items", array_of(Ty::Param(s))), field("at", Ty::Int)],
                vec![
                    method("available", Vec::new(), Some(Ty::Bool)),
                    method("item", Vec::new(), Some(Ty::Param(t))),
                    method(
                        "successor",
                        Vec::new(),
                        Some(Ty::Role(cursor, vec![Ty::Param(t)])),
                    ),
                ],
                vec![(cursor, vec![Ty::Param(t)])],
            );
        }
        let (s, t) = (param(self, "S"), param(self, "T"));
        let mut methods = Vec::new();
        let mut does = Vec::new();
        if let Some(cursor) = standard.cursor {
            methods.push(method(
                "cursor",
                Vec::new(),
                Some(Ty::Role(cursor, vec![Ty::Param(t)])),
            ));
        }
        methods.push(method("size", Vec::new(), Some(Ty::Int)));
        methods.push(method(
            "item",
            vec![("index".to_string(), Ty::Int)],
            Some(Ty::Param(t)),
        ));
        methods.push(method("first", Vec::new(), Some(Ty::Param(t))));
        methods.push(method("last", Vec::new(), Some(Ty::Param(t))));
        for role in [standard.sequencing, standard.iterating]
            .into_iter()
            .flatten()
        {
            does.push((role, vec![Ty::Param(t)]));
        }
        if let Some(sizing) = standard.sizing {
            does.push((sizing, Vec::new()));
        }
        if !does.is_empty() {
            self.adapters.array_sequence = add(
                self,
                "ArraySequence",
                vec![s, t],
                vec![field("items", array_of(Ty::Param(s)))],
                methods,
                does,
            );
        }
        if let (Some(comparing), Some(order)) = (standard.comparing, standard.order) {
            let t = param(self, "T");
            self.adapters.scalar_order = add(
                self,
                "ScalarOrder",
                vec![t],
                vec![field("value", Ty::Param(t))],
                vec![method(
                    "order",
                    vec![("other".to_string(), Ty::Param(t))],
                    Some(Ty::Enum(order)),
                )],
                vec![(comparing, vec![Ty::Param(t)])],
            );
        }
        if let Some(byte_source) = standard.byte_source {
            self.adapters.byte_array_source = add(
                self,
                "ByteArraySource",
                Vec::new(),
                vec![field("items", array_of(Ty::Byte))],
                vec![method("bytes", Vec::new(), Some(array_of(Ty::Byte)))],
                vec![(byte_source, Vec::new())],
            );
            self.adapters.string_source = add(
                self,
                "StringSource",
                Vec::new(),
                vec![field("value", Ty::Str)],
                vec![method("bytes", Vec::new(), Some(array_of(Ty::Byte)))],
                vec![(byte_source, Vec::new())],
            );
        }
    }

    // ----- отношения типов -----

    /// Роли, которые делает тип, с подставленными аргументами.
    pub fn roles_of(&self, ty: &Ty) -> Vec<Ty> {
        match ty {
            Ty::Object(id, args) => {
                let info = &self.objects[id.0];
                let env: Vec<(ParamId, Ty)> = info
                    .params
                    .iter()
                    .copied()
                    .zip(args.iter().cloned())
                    .collect();
                info.does
                    .iter()
                    .map(|(role, role_args)| {
                        Ty::Role(*role, role_args.iter().map(|arg| arg.subst(&env)).collect())
                    })
                    .collect()
            }
            Ty::Param(param) => self.params[param.0].bound.iter().cloned().collect(),
            Ty::Int | Ty::Float | Ty::Rat | Ty::Byte | Ty::Str | Ty::Char => {
                let mut roles: Vec<Ty> = self
                    .standard
                    .comparing
                    .map(|role| Ty::Role(role, vec![ty.clone()]))
                    .into_iter()
                    .collect();
                if *ty == Ty::Str {
                    roles.extend(self.standard.byte_source.map(Ty::role));
                }
                roles
            }
            Ty::Array(element) => {
                let mut roles = Vec::new();
                for role in [self.standard.sequencing, self.standard.iterating]
                    .into_iter()
                    .flatten()
                {
                    roles.push(Ty::Role(role, vec![(**element).clone()]));
                }
                roles.extend(self.standard.sizing.map(Ty::role));
                if **element == Ty::Byte {
                    roles.extend(self.standard.byte_source.map(Ty::role));
                }
                roles
            }
            _ => Vec::new(),
        }
    }

    /// Значение типа `from` пересекает границу типа `to` (§12): тот же тип,
    /// `^C` → `C`, конкретный тип → роль, которую он делает, `[]`/`[:]` → коллекция.
    pub fn assignable(&self, from: &Ty, to: &Ty) -> bool {
        if from == to || from.is_error() || to.is_error() {
            return true;
        }
        match (from, to) {
            (Ty::Never, _) => true,
            (Ty::Successor(inner), _) if !to.is_successor() => self.assignable(inner, to),
            (Ty::Successor(inner), Ty::Successor(target)) => self.assignable(inner, target),
            (Ty::Array(element), Ty::Array(_)) if **element == Ty::Never => true,
            (Ty::Map(key, _), Ty::Map(..)) if **key == Ty::Never => true,
            // `[]` пересекает границу роли массивов с любым элементом.
            (Ty::Array(element), Ty::Role(role, args)) if **element == Ty::Never => {
                let candidate = Ty::Array(Box::new(args.first().cloned().unwrap_or(Ty::Never)));
                self.roles_of(&candidate)
                    .iter()
                    .any(|found| matches!(found, Ty::Role(id, _) if id == role))
            }
            // Массив — только читаемый `Sequencing<E>`/`Iterating<E>`: элементы
            // пересекают границу `E` по одному (как адаптер Perl).
            (Ty::Array(element), Ty::Role(role, args))
                if args.len() == 1
                    && (Some(*role) == self.standard.sequencing
                        || Some(*role) == self.standard.iterating) =>
            {
                self.assignable(element, &args[0])
            }
            (Ty::Role(..), Ty::Role(..)) => self.narrows(from, to),
            (_, Ty::Role(..)) => self.roles_of(from).iter().any(|role| role == to),
            (_, Ty::Any) => !matches!(
                from,
                Ty::Action(_) | Ty::Handle(_) | Ty::Varargs(_) | Ty::Future(_) | Ty::Cap(..)
            ),
            _ => false,
        }
    }

    /// Действия роли с подставленными аргументами.
    pub fn role_actions(&self, ty: &Ty) -> Vec<RoleAction> {
        let Ty::Role(role, args) = ty else {
            return Vec::new();
        };
        let info = &self.roles[role.0];
        let env: Vec<(ParamId, Ty)> = info
            .params
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .collect();
        info.methods
            .iter()
            .map(|method| {
                (
                    method.name.clone(),
                    method.params.iter().map(|(_, ty)| ty.subst(&env)).collect(),
                    method.result.as_ref().map(|ty| ty.subst(&env)),
                    method.variadic,
                    method.is_async,
                )
            })
            .collect()
    }

    /// Роль → роль только сужением (§12.1): действия цели уже доступны через
    /// источник, и какой-то объект делает обе роли; что исходный объект
    /// делает цель, проверяется во время исполнения.
    pub fn narrows(&self, from: &Ty, to: &Ty) -> bool {
        let (Ty::Role(source, _), Ty::Role(target, _)) = (from, to) else {
            return false;
        };
        let available = self.role_actions(from);
        let subset = self
            .role_actions(to)
            .iter()
            .all(|action| available.contains(action));
        let builtin = [
            Ty::array(Ty::Never),
            Ty::Int,
            Ty::Float,
            Ty::Rat,
            Ty::Byte,
            Ty::Str,
            Ty::Char,
        ];
        let has_both = |ty: &Ty| {
            let roles = self.roles_of(ty);
            let has = |id: &RoleId| {
                roles
                    .iter()
                    .any(|role| matches!(role, Ty::Role(found, _) if found == id))
            };
            has(source) && has(target)
        };
        subset
            && (builtin.iter().any(has_both)
                || self.objects.iter().enumerate().any(|(index, object)| {
                    let ty = Ty::Object(
                        ObjectId(index),
                        object
                            .params
                            .iter()
                            .map(|param| Ty::Param(*param))
                            .collect(),
                    );
                    let roles = self.roles_of(&ty);
                    let has = |id: &RoleId| {
                        roles
                            .iter()
                            .any(|role| matches!(role, Ty::Role(found, _) if found == id))
                    };
                    has(source) && has(target)
                }))
    }

    // ----- имена типов для сообщений -----

    pub fn type_name(&self, ty: &Ty) -> String {
        match ty {
            Ty::Int => "int".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Byte => "byte".to_string(),
            Ty::Char => "char".to_string(),
            Ty::Float => "float".to_string(),
            Ty::Rat => "rat".to_string(),
            Ty::Str => "string".to_string(),
            Ty::Any => "any".to_string(),
            Ty::Range => "range".to_string(),
            Ty::Array(element) if **element == Ty::Never => "[]".to_string(),
            Ty::Array(element) => format!("@{}", self.type_name(element)),
            Ty::Map(key, _) if **key == Ty::Never => "[:]".to_string(),
            Ty::Map(key, value) => format!("%{}:{}", self.type_name(key), self.type_name(value)),
            Ty::Object(id, args) => self.applied(&self.objects[id.0].name, args),
            Ty::Role(id, args) => self.applied(&self.roles[id.0].name, args),
            Ty::Enum(id) => self.enums[id.0].name.clone(),
            Ty::Handle(id) => self.handles[id.0].name.clone(),
            Ty::Successor(inner) => format!("^{}", self.type_name(inner)),
            Ty::Param(param) => self.params[param.0].name.clone(),
            Ty::Action(action) => {
                let params: Vec<String> = action
                    .params
                    .iter()
                    .map(|param| self.type_name(param))
                    .collect();
                let result = action
                    .result
                    .as_ref()
                    .map_or_else(|| "void".to_string(), |result| self.type_name(result));
                format!(
                    "{}({}) is {result}",
                    if action.is_async { "async " } else { "" },
                    params.join(", ")
                )
            }
            Ty::Future(None) => "Future<void>".to_string(),
            Ty::Future(Some(inner)) => format!("Future<{}>", self.type_name(inner)),
            Ty::Cap(id, args, cap) => format!(
                "{}::{}",
                self.applied(&self.objects[id.0].name, args),
                self.objects[id.0].caps[*cap].name
            ),
            Ty::Varargs(inner) => format!("...{}", self.type_name(inner)),
            Ty::Never => "never".to_string(),
            Ty::Test => "test".to_string(),
            Ty::Error => "<error>".to_string(),
        }
    }

    fn applied(&self, name: &str, args: &[Ty]) -> String {
        if args.is_empty() {
            return name.to_string();
        }
        let args: Vec<String> = args.iter().map(|arg| self.type_name(arg)).collect();
        format!("{name}<{}>", args.join(", "))
    }

    pub fn result_name(&self, result: &Option<Ty>) -> String {
        result
            .as_ref()
            .map_or_else(|| "void".to_string(), |ty| self.type_name(ty))
    }
}

/// Перегрузки одного имени: одинаковые сигнатуры неразличимы, у variadic
/// перегрузок нет, у одной арности — один результат.
fn overload_conflict(
    methods: &[MethodInfo<'_>],
    method: &MethodInfo<'_>,
    object: &str,
) -> Option<String> {
    for other in methods.iter().filter(|other| other.name == method.name) {
        // Variadic-перегрузка запрещена только между двумя variadic (как Perl).
        if other.variadic && method.variadic {
            return Some(format!(
                "Object \"{object}\" cannot overload variadic action \"{}\"",
                method.name
            ));
        }
        let same_types = method.params.len() == other.params.len()
            && method
                .params
                .iter()
                .zip(&other.params)
                .all(|((_, left), (_, right))| left == right);
        let same_names = method
            .params
            .iter()
            .zip(&other.params)
            .all(|((left, _), (right, _))| left == right);
        if same_types && same_names {
            return Some(format!(
                "Action \"{}\" on object \"{object}\" is declared twice with the same signature",
                method.name
            ));
        }
        if method.params.len() == other.params.len()
            && !method.variadic
            && !other.variadic
            && method.result != other.result
        {
            return Some(format!(
                "Overloads of action \"{}\" on object \"{object}\" must declare the same result",
                method.name
            ));
        }
    }
    None
}

fn contains_action(ty: &Ty) -> bool {
    match ty {
        Ty::Action(_) => true,
        Ty::Array(inner) | Ty::Successor(inner) | Ty::Varargs(inner) => contains_action(inner),
        Ty::Map(key, value) => contains_action(key) || contains_action(value),
        Ty::Object(_, args) | Ty::Role(_, args) => args.iter().any(contains_action),
        _ => false,
    }
}

fn module_name(program: &ast::Program) -> String {
    program
        .module
        .path
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join("/")
}

/// `import "io/stdout"` — пространство имён `stdout`.
fn last_segment(path: &str) -> String {
    let path = path.strip_suffix(".ok").unwrap_or(path);
    path.rsplit('/').next().unwrap_or(path).to_string()
}
