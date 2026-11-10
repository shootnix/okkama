//! Таблица объявлений программы: модули, объекты, роли, enum, natives.
//!
//! Регистрация идёт в три прохода, чтобы объявления могли ссылаться друг на
//! друга в любом порядке: имена всех модулей, затем импорты, затем сигнатуры
//! (поля, конструкторы, actions, `does`). Проверки объявлений — тексты
//! Perl-анализатора; ошибки копятся, а не обрывают проход.

use std::collections::{HashMap, HashSet};

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
    /// `const` модулей: индекс в `Registry::consts` (#153).
    pub consts: HashMap<String, usize>,
}

/// Вычисленная константа (#153): тип, значение для lowering и то же
/// значение в представлении рантайма (из него строятся зависящие константы).
#[derive(Clone, Debug)]
pub struct ConstResult {
    pub ty: Ty,
    pub value: super::thir::ConstValue,
    pub runtime: okkama_runtime_core::Value,
}

/// Состояние вычисления константы.
#[derive(Clone, Debug)]
pub enum ConstState {
    Pending,
    /// Вычисляется: ссылка на неё сейчас — цикл.
    Busy,
    /// `None` — ошибка уже сообщена.
    Done(Option<ConstResult>),
}

/// `const NAME = value;` модуля (#153); значение вычисляет анализ.
#[derive(Clone, Debug)]
pub struct ConstInfo<'a> {
    pub name: String,
    pub module: usize,
    pub decl: &'a ast::ConstDecl,
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
    /// `hidden` объявления: видны только внутри модуля.
    pub hidden: HashSet<String>,
}

#[derive(Clone, Debug)]
pub struct ParamInfo {
    pub name: String,
    /// `T: Comparing<T>` — роль-ограничение.
    pub bound: Option<Ty>,
    /// `%T:V` в объявлении: аргумент должен быть типом ключа (#242).
    pub key: bool,
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
        /// `name is T = value` per parameter (#149).
        defaults: Vec<Option<&'a ast::Expr>>,
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
    pub params: Vec<ParamId>,
    pub payloads: Vec<Vec<(String, Ty)>>,
    pub variant: bool,
    /// `enum Name is Error`: значения бросаются и ловятся, `text()` порождается.
    pub error: bool,
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
    /// `cell::Cell`: stays in synchronous code (#152).
    pub cell: Option<ObjectId>,
    /// Operator roles of module `operators` (#215), by operator.
    pub operators: Operators,
}

/// The roles that give `+ - * / %` and unary `-` a meaning for objects.
#[derive(Clone, Copy, Debug, Default)]
pub struct Operators {
    pub adding: Option<RoleId>,
    pub subtracting: Option<RoleId>,
    pub multiplying: Option<RoleId>,
    pub dividing: Option<RoleId>,
    pub remaindering: Option<RoleId>,
    pub negating: Option<RoleId>,
}

#[derive(Debug)]
pub struct Registry<'a> {
    pub modules: Vec<ModuleInfo<'a>>,
    pub objects: Vec<ObjectInfo<'a>>,
    pub roles: Vec<RoleInfo<'a>>,
    pub enums: Vec<EnumInfo>,
    pub handles: Vec<HandleInfo>,
    pub natives: Vec<NativeInfo<'a>>,
    pub consts: Vec<ConstInfo<'a>>,
    /// По одному на `consts`, заполняется при проверке тел.
    pub const_state: Vec<ConstState>,
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
    /// Объекты-ключи словарей в объявлениях — проверяются, когда известны
    /// поля всех объектов (#212).
    pending_keys: Vec<(usize, Ty, Span)>,
    /// Specializations whose key parameters are checked after the bodies (#242).
    pub key_uses: Vec<(usize, ObjectId, Vec<Ty>, Span)>,
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
                    hidden: hidden_names(source.program),
                })
                .collect(),
            objects: Vec::new(),
            roles: Vec::new(),
            enums: Vec::new(),
            handles: Vec::new(),
            natives: Vec::new(),
            consts: Vec::new(),
            const_state: Vec::new(),
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
            pending_keys: Vec::new(),
            key_uses: Vec::new(),
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
            registry.collect_enum_payloads(module);
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
        registry.check_variant_recursion();
        registry.check_error_payloads();
        registry.check_hidden_leaks();
        registry.find_standard();
        registry.check_cell_fields();
        registry.add_adapters();
        for (module, ty, span) in std::mem::take(&mut registry.pending_bounds) {
            registry.current = module;
            registry.check_bounds(&ty, span);
        }
        for (module, ty, span) in std::mem::take(&mut registry.pending_keys) {
            registry.current = module;
            if let Some(problem) = registry.key_problem(&ty) {
                registry.error(span, problem);
            }
        }
        registry.current = registry.root();
        registry
    }

    /// Почему `ty` не может быть ключом словаря; `None` — может (#212).
    /// Ключ — скаляр, enum, параметр типа (проверяется при исполнении) или
    /// объект-значение: без `destroy` и cap, все поля которого — скаляры
    /// (также `byte`), enum или такие же объекты. Объекты
    /// неизменяемы, поэтому равенство и порядок ключа — по полям, как `==`.
    pub fn key_problem(&self, ty: &Ty) -> Option<String> {
        if ty.is_map_key() || matches!(ty, Ty::Param(_)) || ty.is_error() {
            return None;
        }
        let Ty::Object(..) = ty else {
            return Some(format!("Map key type {} is not a scalar key type", self.type_name(ty)));
        };
        self.object_key_problem(ty, &mut Vec::new())
            .map(|reason| format!("Map key type {} cannot be a key: {reason}", self.type_name(ty)))
    }

    /// The argument of a parameter used as a map key must be a key type (#242).
    fn key_argument_problem(&self, object: ObjectId, param: ParamId, arg: &Ty) -> Option<String> {
        if !self.params[param.0].key {
            return None;
        }
        self.key_problem(arg).map(|problem| {
            format!(
                "Generic parameter \"{}\" for object \"{}\" is a map key: {problem}",
                self.params[param.0].name, self.objects[object.0].name
            )
        })
    }

    /// Key arguments (#242), checked once every body is known: a parameter
    /// passed on as a key argument — `Wrap<U> { has inner is Index<U>; }` —
    /// is a key parameter too, so `Wrap<@int>` fails like `Index<@int>`.
    pub fn check_key_arguments(&mut self) {
        let uses = std::mem::take(&mut self.key_uses);
        let mut changed = true;
        while changed {
            changed = false;
            for (_, object, args, _) in &uses {
                for (param, arg) in self.objects[object.0].params.iter().zip(args) {
                    if let Ty::Param(inner) = arg
                        && self.params[param.0].key
                        && !self.params[inner.0].key
                    {
                        self.params[inner.0].key = true;
                        changed = true;
                    }
                }
            }
        }
        for (module, object, args, span) in uses {
            let params = self.objects[object.0].params.clone();
            for (param, arg) in params.iter().zip(&args) {
                if let Some(message) = self.key_argument_problem(object, *param, arg) {
                    self.set_module(module);
                    self.error(span, message);
                }
            }
        }
    }

    /// The object, or one it holds in a field, has an async destroy (#154):
    /// it may be owned only where the end of its scope can be awaited.
    pub fn async_destroy(&self, ty: &Ty) -> bool {
        self.async_destroy_in(ty, &mut Vec::new())
    }

    fn async_destroy_in(&self, ty: &Ty, visiting: &mut Vec<ObjectId>) -> bool {
        let (id, args) = match ty {
            Ty::Object(id, args) => (id, args),
            Ty::Array(element) => return self.async_destroy_in(element, visiting),
            Ty::Map(_, value) => return self.async_destroy_in(value, visiting),
            _ => return false,
        };
        if visiting.contains(id) {
            return false;
        }
        let info = &self.objects[id.0];
        if info.destroy.is_some_and(|destroy| destroy.is_async) {
            return true;
        }
        let env: Vec<(ParamId, Ty)> = info.params.iter().copied().zip(args.iter().cloned()).collect();
        visiting.push(*id);
        let found = info.fields.iter().any(|field| self.async_destroy_in(&field.ty.subst(&env), visiting));
        visiting.pop();
        found
    }

    fn object_key_problem(&self, ty: &Ty, visiting: &mut Vec<ObjectId>) -> Option<String> {
        let Ty::Object(id, args) = ty else {
            return Some(format!("{} is not a key type", self.type_name(ty)));
        };
        if visiting.contains(id) {
            return Some(format!("{} contains itself", self.type_name(ty)));
        }
        let info = &self.objects[id.0];
        if info.destroy.is_some() {
            return Some(format!("{} has a destroy block", self.type_name(ty)));
        }
        if !info.caps.is_empty() {
            return Some(format!("{} holds capabilities", self.type_name(ty)));
        }
        let env: Vec<(ParamId, Ty)> = info.params.iter().copied().zip(args.iter().cloned()).collect();
        visiting.push(*id);
        let problem = info.fields.iter().find_map(|field| {
            let field_ty = field.ty.subst(&env);
            let fits = field_ty.is_map_key() || matches!(field_ty, Ty::Byte | Ty::Error);
            if fits {
                return None;
            }
            match &field_ty {
                Ty::Object(..) => self
                    .object_key_problem(&field_ty, visiting)
                    .map(|inner| format!("field \"{}\" ({}): {inner}", field.name, self.type_name(&field_ty))),
                other => Some(format!("field \"{}\" is {}", field.name, self.type_name(other))),
            }
        });
        visiting.pop();
        problem
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
                first_program.module.version_text(),
                program.module.version_text(),
            );
            if version != other {
                let shown = |version: &str| match version {
                    "" => "no version".to_string(),
                    version => format!("\"{version}\""),
                };
                let message = format!(
                    "module \"{name}\" has conflicting versions {} and {}",
                    shown(&version),
                    shown(&other)
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
                if let Some(&earlier) = roots[..position]
                    .iter()
                    .find(|earlier| self.modules[**earlier].own.types.contains_key(&name.text))
                {
                    self.error(
                        name.span,
                        format!("{kind} \"{}\" is already declared in another file of this module", name.text),
                    );
                    // The first declaration, reported in its own file.
                    let first = self.modules[earlier].program.items.iter().find_map(|item| match item {
                        ast::Item::Object(decl) if decl.name.text == name.text => Some(decl.name.span),
                        ast::Item::Role(decl) if decl.name.text == name.text => Some(decl.name.span),
                        ast::Item::Enum(decl) if decl.name.text == name.text => Some(decl.name.span),
                        ast::Item::NativeObject(decl) if decl.name.text == name.text => Some(decl.name.span),
                        _ => None,
                    });
                    if let Some(span) = first {
                        self.errors.push(SemanticError::invalid(
                            earlier,
                            span,
                            format!("{kind} \"{}\" is first declared here", name.text),
                        ));
                    }
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
                for (name, found) in own.consts {
                    scope.consts.entry(name).or_insert(found);
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
        for (name, cases) in [
            ("Maybe", vec!["some", "none"]),
            ("Presence", vec!["some", "none", "missing"]),
        ] {
            let param = ParamId(self.params.len());
            self.params.push(ParamInfo {
                name: "T".into(),
                bound: None,
                key: false,
            });
            let mut payloads = vec![Vec::new(); cases.len()];
            payloads[0] = vec![("value".into(), Ty::Param(param))];
            self.enums.push(EnumInfo {
                name: name.into(),
                module,
                params: vec![param],
                payloads,
                variant: true,
                error: false,
                cases: cases.into_iter().map(str::to_string).collect(),
                span: Span::empty(0),
            });
        }
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
                    key: false,
                });
                ParamId(self.params.len() - 1)
            })
            .collect()
    }

    fn own_type(&self, module: usize, name: &str) -> Option<TypeRef> {
        self.modules[module].own.types.get(name).copied()
    }

    /// The module declaring `ty`, named for a diagnostic: both sources of a
    /// conflicting name are shown (H-16).
    fn origin(&self, ty: TypeRef) -> String {
        let module = match ty {
            TypeRef::Object(id) => self.objects[id.0].module,
            TypeRef::Role(id) => self.roles[id.0].module,
            TypeRef::Enum(id) => self.enums[id.0].module,
            TypeRef::Handle(id) => self.handles[id.0].module,
        };
        format!("module \"{}\"", self.modules[module].name)
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
                    if matches!(name.as_str(), "Error" | "Maybe" | "Presence")
                        || self.own_type(module, name).is_some()
                    {
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
                    if matches!(name.as_str(), "Error" | "Maybe" | "Presence")
                        || self.own_type(module, name).is_some()
                    {
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
                    let error = match &decl.role {
                        Some(role) if role.text != "Error" => {
                            self.error(
                                role.span,
                                format!(
                                    "An enum can only be declared \"is Error\", not \"is {}\"",
                                    role.text
                                ),
                            );
                            false
                        }
                        Some(role) if !decl.generics.is_empty() => {
                            self.error(
                                role.span,
                                format!("Error enum \"{name}\" cannot have type parameters"),
                            );
                            false
                        }
                        Some(_) => true,
                        None => false,
                    };
                    let id = EnumId(self.enums.len());
                    let params = self.new_params(&decl.generics);
                    self.enums.push(EnumInfo {
                        params,
                        payloads: vec![Vec::new(); cases.len()],
                        // Error enum — всегда вариантный: полный match и объект в OIR.
                        variant: error
                            || !decl.generics.is_empty()
                            || decl.payloads.iter().any(|p| !p.is_empty()),
                        error,
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
                ast::Item::Const(decl) => {
                    let name = &decl.name.text;
                    let own = &self.modules[module].own;
                    if own.consts.contains_key(name) || own.natives.contains_key(name) {
                        self.error(decl.name.span, format!("Constant \"{name}\" is already declared"));
                        continue;
                    }
                    let index = self.consts.len();
                    self.consts.push(ConstInfo {
                        name: name.clone(),
                        module,
                        decl,
                    });
                    self.modules[module].own.consts.insert(name.clone(), index);
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
                if self.modules[dependency].hidden.contains(&name.text) {
                    self.error(
                        name.span,
                        format!(
                            "Import \"{path}\" cannot take hidden declaration \"{}\"",
                            name.text
                        ),
                    );
                    continue;
                }
                let own = &self.modules[dependency].own;
                if !own.types.contains_key(&name.text)
                    && !own.natives.contains_key(&name.text)
                    && !own.consts.contains_key(&name.text)
                {
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
                    if let Some(existing) = self.modules[module].namespaces.get(&namespace)
                        && existing.module != dependency {
                            let (first, second) = (&self.modules[existing.module].name, &self.modules[dependency].name);
                            self.error(
                                import.span,
                                format!("Namespace \"{namespace}\" is already declared for module \"{first}\"; this import names module \"{second}\""),
                            );
                            continue;
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
            if let Some(existing) = scope.types.get(text)
                && existing != found {
                    let kind = match existing {
                        TypeRef::Object(_) => "Object",
                        TypeRef::Role(_) => "Role",
                        TypeRef::Enum(_) => "Enum",
                        TypeRef::Handle(_) => "Type",
                    };
                    let (first, second) = (self.origin(*existing), self.origin(*found));
                    self.error(span, format!("{kind} \"{text}\" is already declared by {first}; this import takes another from {second}"));
                    return;
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
        if let Some(found) = own.consts.get(text) {
            if self.modules[module].scope.consts.get(text).is_some_and(|existing| existing != found) {
                self.error(span, format!("Constant \"{text}\" is already visible"));
                return;
            }
            self.modules[module].scope.consts.insert(text.to_string(), *found);
        }
    }

    // ----- разрешение имён -----

    /// Тип по имени в области модуля `module` (`None` — не объявлен).
    pub fn find_type(&self, module: usize, name: &str) -> Option<TypeRef> {
        self.modules[module]
            .scope
            .types
            .get(name)
            .copied()
            .or(match name {
                "Maybe" => Some(TypeRef::Enum(EnumId(0))),
                "Presence" => Some(TypeRef::Enum(EnumId(1))),
                _ => None,
            })
    }

    /// `ns.Name`: собственное объявление модуля пространства имён.
    pub fn find_qualified(&self, module: usize, namespace: &str, name: &str) -> Option<TypeRef> {
        let space = self.modules[module].namespaces.get(namespace)?;
        if self.modules[space.module].hidden.contains(name) {
            return None;
        }
        if space
            .only
            .as_ref()
            .is_some_and(|only| !only.iter().any(|taken| taken == name))
        {
            return None;
        }
        self.modules[space.module].own.types.get(name).copied()
    }

    /// `ns.Name` называет `hidden` объявление модуля `ns`: текст ошибки.
    pub fn hidden_qualified(&self, module: usize, namespace: &str, name: &str) -> Option<String> {
        let space = self.modules[module].namespaces.get(namespace)?;
        let owner = &self.modules[space.module];
        owner.hidden.contains(name).then(|| {
            format!(
                "Type \"{namespace}.{name}\" is hidden in module \"{}\"",
                owner.name
            )
        })
    }

    /// `NAME`: константа модуля или взятая импортом (#153).
    pub fn find_const(&self, module: usize, name: &str) -> Option<usize> {
        self.modules[module].scope.consts.get(name).copied()
    }

    /// `ns.NAME`: открытая константа модуля пространства имён.
    pub fn find_qualified_const(&self, module: usize, namespace: &str, name: &str) -> Option<usize> {
        let space = self.modules[module].namespaces.get(namespace)?;
        if self.modules[space.module].hidden.contains(name)
            || space.only.as_ref().is_some_and(|only| !only.iter().any(|taken| taken == name))
        {
            return None;
        }
        self.modules[space.module].own.consts.get(name).copied()
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
                // A parameter used as a key bounds its argument to key types (#242).
                if let Ty::Param(param) = key_type {
                    self.params[param.0].key = true;
                }
                if matches!(key_type, Ty::Object(..)) {
                    // Поля объекта ещё могут быть не собраны: проверка в конце.
                    self.pending_keys.push((self.current, key_type.clone(), key.span));
                } else if !key_type.is_error()
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
                    let message = if let Some(hidden) =
                        self.hidden_qualified(module, &namespace.text, &name.text)
                    {
                        hidden
                    } else if self.is_namespace(module, &namespace.text) {
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
            TypeRef::Enum(id) => self.enums[id.0].params.len(),
            TypeRef::Handle(_) => 0,
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
            TypeRef::Enum(id) if self.enums[id.0].variant => Ty::Variant(id, resolved),
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
        self.key_uses.push((self.current, *id, args.clone(), span));
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

    fn check_variant_recursion(&mut self) {
        fn walk(
            reg: &Registry<'_>,
            ty: &Ty,
            path: &mut Vec<EnumId>,
            objects: &mut Vec<ObjectId>,
        ) -> bool {
            match ty {
                Ty::Variant(id, args) => {
                    if args.iter().any(|t| walk(reg, t, path, objects)) {
                        return true;
                    }
                    if path.contains(id) {
                        return true;
                    }
                    path.push(*id);
                    let decl = &reg.enums[id.0];
                    let found = decl
                        .payloads
                        .iter()
                        .flatten()
                        .any(|(_, t)| walk(reg, t, path, objects));
                    path.pop();
                    found
                }
                Ty::Object(id, args) => {
                    if args.iter().any(|t| walk(reg, t, path, objects)) {
                        return true;
                    }
                    if objects.contains(id) {
                        return false;
                    }
                    objects.push(*id);
                    let decl = &reg.objects[id.0];
                    let found = decl.fields.iter().any(|f| walk(reg, &f.ty, path, objects));
                    objects.pop();
                    found
                }
                Ty::Array(t) | Ty::Successor(t) | Ty::Varargs(t) => walk(reg, t, path, objects),
                Ty::Map(k, v) => walk(reg, k, path, objects) || walk(reg, v, path, objects),
                _ => false,
            }
        }
        for index in 0..self.enums.len() {
            let decl = &self.enums[index];
            let ty = Ty::Variant(
                EnumId(index),
                decl.params.iter().copied().map(Ty::Param).collect(),
            );
            if decl.variant && walk(self, &ty, &mut Vec::new(), &mut Vec::new()) {
                let span = decl.span;
                self.current = decl.module;
                self.error(span, "Recursive variant payload types are not supported");
            }
        }
    }

    /// Payload error enum — значения, которые `text()` умеет показать: строки,
    /// числа, `bool`, enum (в том числе другие ошибки) с такими же payload.
    fn check_error_payloads(&mut self) {
        fn shown(reg: &Registry<'_>, ty: &Ty, depth: usize) -> bool {
            match ty {
                Ty::Str | Ty::Int | Ty::Byte | Ty::Float | Ty::Rat | Ty::Bool | Ty::Enum(_) => true,
                Ty::Variant(id, args) if depth < 64 => {
                    let decl = &reg.enums[id.0];
                    let env: Vec<(ParamId, Ty)> =
                        decl.params.iter().copied().zip(args.iter().cloned()).collect();
                    decl.payloads
                        .iter()
                        .flatten()
                        .all(|(_, field)| shown(reg, &field.subst(&env), depth + 1))
                }
                Ty::Error => true,
                _ => false,
            }
        }
        for index in 0..self.enums.len() {
            if !self.enums[index].error {
                continue;
            }
            let decl = self.enums[index].clone();
            for (field, ty) in decl.payloads.iter().flatten() {
                if !shown(self, ty, 0) {
                    self.current = decl.module;
                    let message = format!(
                        "Payload \"{field}\" of error enum \"{}\" has type {}; error payloads are limited to string, int, byte, float, rat, bool and enums with such payloads",
                        decl.name,
                        self.type_name(ty)
                    );
                    self.error(decl.span, message);
                }
            }
        }
    }

    /// `hidden` тип не утекает через публичную сигнатуру: его нельзя назвать
    /// снаружи, значит, и получить в публичном контракте. Публичный объект не
    /// делает скрытую роль: так наружу ушёл бы внутренний контракт.
    fn check_hidden_leaks(&mut self) {
        let mut found: Vec<(usize, Span, String)> = Vec::new();
        let bound_leak = |reg: &Registry<'_>, params: &[ParamId]| {
            params
                .iter()
                .filter_map(|param| reg.params[param.0].bound.as_ref())
                .find_map(|bound| reg.hidden_in(bound))
        };
        for object in &self.objects {
            let Some(decl) = object.decl else {
                continue;
            };
            if self.is_hidden(object.module, &object.name) {
                continue;
            }
            let module = object.module;
            for (role, args) in &object.does {
                let info = &self.roles[role.0];
                if self.is_hidden(info.module, &info.name) {
                    found.push((
                        module,
                        decl.name.span,
                        format!(
                            "Public object \"{}\" does hidden role \"{}\"",
                            object.name, info.name
                        ),
                    ));
                } else if let Some(hidden) = args.iter().find_map(|arg| self.hidden_in(arg)) {
                    found.push((
                        module,
                        decl.name.span,
                        format!(
                            "Public object \"{}\" exposes hidden type \"{hidden}\" in role \"{}\"",
                            object.name, info.name
                        ),
                    ));
                }
            }
            if let Some(hidden) = bound_leak(self, &object.params) {
                found.push((
                    module,
                    decl.name.span,
                    format!(
                        "Type parameter bound of public object \"{}\" exposes hidden type \"{hidden}\"",
                        object.name
                    ),
                ));
            }
            for method in &object.methods {
                let Some(action) = method.decl else {
                    continue;
                };
                if method.private {
                    continue;
                }
                let leak = method
                    .params
                    .iter()
                    .map(|(_, ty)| ty)
                    .chain(method.result.iter())
                    .find_map(|ty| self.hidden_in(ty));
                if let Some(hidden) = leak {
                    found.push((
                        module,
                        action.name.span,
                        format!(
                            "Public action \"{}\" of \"{}\" exposes hidden type \"{hidden}\"",
                            method.name, object.name
                        ),
                    ));
                }
            }
            for ctor in &object.ctors {
                let leak = match &ctor.shape {
                    CtorShape::Full { params, .. } => {
                        params.iter().find_map(|(_, ty)| self.hidden_in(ty))
                    }
                    CtorShape::Shorthand { fields, .. } => fields
                        .iter()
                        .find_map(|field| self.hidden_in(&object.fields[*field].ty)),
                };
                if let Some(hidden) = leak {
                    found.push((
                        module,
                        ctor.span,
                        format!(
                            "Constructor of public object \"{}\" exposes hidden type \"{hidden}\"",
                            object.name
                        ),
                    ));
                }
            }
            for cap in &object.caps {
                if let Some((field, hidden)) = cap
                    .fields
                    .iter()
                    .find_map(|(field, ty, _)| self.hidden_in(ty).map(|hidden| (field, hidden)))
                {
                    found.push((
                        module,
                        cap.span,
                        format!(
                            "Cap \"{}\" field \"{field}\" of public object \"{}\" exposes hidden type \"{hidden}\"",
                            cap.name, object.name
                        ),
                    ));
                }
            }
        }
        for role in &self.roles {
            let Some(decl) = role.decl else {
                continue;
            };
            if self.is_hidden(role.module, &role.name) {
                continue;
            }
            let leak = bound_leak(self, &role.params).or_else(|| {
                role.methods.iter().find_map(|method| {
                    method
                        .params
                        .iter()
                        .map(|(_, ty)| ty)
                        .chain(method.result.iter())
                        .find_map(|ty| self.hidden_in(ty))
                })
            });
            if let Some(hidden) = leak {
                found.push((
                    role.module,
                    decl.name.span,
                    format!(
                        "Public role \"{}\" exposes hidden type \"{hidden}\"",
                        role.name
                    ),
                ));
            }
        }
        for info in &self.enums {
            if info.module >= self.modules.len() || self.is_hidden(info.module, &info.name) {
                continue;
            }
            if let Some((field, hidden)) = info
                .payloads
                .iter()
                .flatten()
                .find_map(|(field, ty)| self.hidden_in(ty).map(|hidden| (field, hidden)))
            {
                found.push((
                    info.module,
                    info.span,
                    format!(
                        "Payload \"{field}\" of public enum \"{}\" exposes hidden type \"{hidden}\"",
                        info.name
                    ),
                ));
            }
        }
        for (module, span, message) in found {
            self.current = module;
            self.error(span, message);
        }
    }

    fn is_hidden(&self, module: usize, name: &str) -> bool {
        self.modules
            .get(module)
            .is_some_and(|info| info.hidden.contains(name))
    }

    /// Первый `hidden` тип внутри типа (с аргументами и элементами).
    fn hidden_in(&self, ty: &Ty) -> Option<String> {
        let own = |module: usize, name: &String| self.is_hidden(module, name).then(|| name.clone());
        match ty {
            Ty::Object(id, args) | Ty::Cap(id, args, _) => {
                let info = &self.objects[id.0];
                own(info.module, &info.name).or_else(|| args.iter().find_map(|arg| self.hidden_in(arg)))
            }
            Ty::Role(id, args) => {
                let info = &self.roles[id.0];
                own(info.module, &info.name).or_else(|| args.iter().find_map(|arg| self.hidden_in(arg)))
            }
            Ty::Variant(id, args) => {
                let info = &self.enums[id.0];
                own(info.module, &info.name).or_else(|| args.iter().find_map(|arg| self.hidden_in(arg)))
            }
            Ty::Enum(id) => {
                let info = &self.enums[id.0];
                own(info.module, &info.name)
            }
            Ty::Array(inner) | Ty::Successor(inner) | Ty::Varargs(inner) => self.hidden_in(inner),
            Ty::Future(Some(inner)) => self.hidden_in(inner),
            Ty::Map(key, value) => self.hidden_in(key).or_else(|| self.hidden_in(value)),
            Ty::Action(action) => action
                .params
                .iter()
                .chain(action.result.iter())
                .find_map(|ty| self.hidden_in(ty)),
            _ => None,
        }
    }

    fn collect_enum_payloads(&mut self, module: usize) {
        for item in &self.modules[module].program.items {
            let ast::Item::Enum(decl) = item else {
                continue;
            };
            let Some(TypeRef::Enum(id)) = self.own_type(module, &decl.name.text) else {
                continue;
            };
            let params = self.enums[id.0].params.clone();
            let names: Vec<_> = decl
                .generics
                .iter()
                .zip(&params)
                .map(|(g, p)| (g.name.text.clone(), *p))
                .collect();
            let mut payloads = Vec::new();
            for fields in &decl.payloads {
                let (fields, variadic) = self.signature_params(fields, &names, Some("Enum payloads"));
                if variadic {
                    self.error(decl.span, "Variant payloads cannot be variadic");
                }
                payloads.push(fields);
            }
            self.enums[id.0].payloads = payloads;
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
                    let (params, variadic) = self.signature_params(&decl.params, &[], Some("Natives"));
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
                    if !action.params.is_empty() || action.name.text != "main" {
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
    /// `without_defaults` names a declaration whose parameters cannot have
    /// default values (roles, natives, enum payloads).
    fn signature_params(
        &mut self,
        params: &[ast::Param],
        scope: &[(String, ParamId)],
        without_defaults: Option<&str>,
    ) -> (Vec<(String, Ty)>, bool) {
        let mut resolved: Vec<(String, Ty)> = Vec::new();
        // Default values (#149): only on trailing parameters, not with a
        // variadic tail, and only on object actions and constructors.
        let mut defaulted = false;
        for param in params {
            match (&param.default, without_defaults) {
                (Some(default), Some(what)) => {
                    self.error(default.span, format!("{what} cannot declare default values"));
                }
                (Some(default), None) if params.last().is_some_and(|last| last.variadic) => {
                    self.error(default.span, "An action with a variadic parameter cannot declare default values");
                }
                (Some(_), None) => defaulted = true,
                (None, None) if defaulted && !param.variadic => {
                    self.error(
                        param.name.span,
                        format!("Parameter \"{}\" follows a parameter with a default value, so it needs one too", param.name.text),
                    );
                }
                _ => {}
            }
        }
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
            let (params, variadic) = self.signature_params(&signature.params, &scope, Some("Role actions"));
            let result = self.resolve_result(&signature.result, &scope);
            if methods.iter().any(|other| {
                other.name == signature.name.text
                    && other.variadic == variadic
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
                    let (resolved, variadic) = self.signature_params(params, &scope, None);
                    CtorShape::Full {
                        params: resolved,
                        names: params.iter().map(|param| &param.name).collect(),
                        defaults: params.iter().map(|param| param.default.as_ref()).collect(),
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
                        let missing: Vec<(String, Span)> = self.objects[id.0]
                            .fields
                            .iter()
                            .enumerate()
                            .filter(|(index, field)| {
                                !indices.contains(index) && field.default.is_none()
                            })
                            .map(|(_, field)| (field.name.clone(), field.span))
                            .collect();
                        if let Some((field, at)) = missing.first() {
                            // A fake's constructor is generated (#276): the
                            // field is what needs fixing.
                            match name.strip_suffix("$fake") {
                                Some(role) => self.error(
                                    *at,
                                    format!(
                                        "Fake field \"{field}\" of role \"{role}\" needs a default: a fake takes no constructor arguments"
                                    ),
                                ),
                                None => self.error(
                                    ctor.span,
                                    format!(
                                        "Constructor shorthand for \"{name}\" cannot initialize required field \"{field}\""
                                    ),
                                ),
                            }
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
        // Неразличимые полные конструкторы: те же типы и вариадичность; имена
        // различают только фиксированные (именованным вызовом).
        for (index, ctor) in ctors.iter().enumerate() {
            let CtorShape::Full {
                params, variadic, ..
            } = &ctor.shape
            else {
                continue;
            };
            let twin = ctors[..index].iter().any(|other| {
                let CtorShape::Full {
                    params: others,
                    variadic: other_variadic,
                    ..
                } = &other.shape
                else {
                    return false;
                };
                variadic == other_variadic
                    && params.len() == others.len()
                    && params
                        .iter()
                        .zip(others)
                        .all(|((left_name, left), (right_name, right))| {
                            left == right && (*variadic || left_name == right_name)
                        })
            });
            if twin {
                self.error(
                    ctor.span,
                    format!("Constructor \"{name}\" is declared twice with the same signature"),
                );
            }
        }
        self.objects[id.0].ctors = ctors;

        let mut methods: Vec<MethodInfo<'a>> = Vec::new();
        for action in actions {
            let (params, variadic) = self.signature_params(&action.params, &scope, None);
            let mut result = self.resolve_result(&action.result, &scope);
            let successor = matches!(action.result, ast::ResultSpec::Successor { .. });
            let private = action.visibility == ast::Visibility::Private;
            // Successor объекта — его собственная копия (§11.9); `^Role` бывает
            // только в сигнатуре роли, реализация которой возвращает `^Self`.
            if let Some(Ty::Successor(target)) = &result
                && **target != self_type {
                    let message = if matches!(&**target, Ty::Role(..)) {
                        format!(
                            "Successor action \"{}\" on object \"{name}\" must return ^{name}; declare ^{} in a role signature and implement it as ^{name}",
                            action.name.text,
                            self.type_name(target)
                        )
                    } else {
                        format!(
                            "Successor action \"{}\" on object \"{name}\" must return ^{name}",
                            action.name.text
                        )
                    };
                    self.error(action.result.span(), message);
                    result = Some(Ty::Error);
                }
            let candidate = MethodInfo {
                name: action.name.text.clone(),
                params,
                result,
                successor,
                private,
                variadic,
                is_async: action.is_async,
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
                // A generic role may be declared again with other arguments
                // (`Subtracting<Duration, Instant>` and `Subtracting<Instant,
                // Duration>`, #215); the same instance twice is an error.
                if does.iter().any(|(declared, known)| *declared == role && *known == args) {
                    self.error(
                        role_type.span,
                        format!("Role \"{}\" is listed twice", self.roles[role.0].name),
                    );
                    continue;
                }
                // Ошибки — только error enum (§9.5.1): объект не делает Error.
                if role == self.system.error {
                    self.error(
                        role_type.span,
                        format!(
                            "Object \"{}\" cannot do Error: errors are enums, declare `enum {} is Error = (...)`",
                            decl.name.text, decl.name.text
                        ),
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
        // A fake is named by its role, not by the generated object, and its
        // errors point at the fake's own code (#276).
        let fake = object.ends_with("$fake");
        let (on, for_role) = if fake {
            (format!("of the fake of role \"{role_display}\""), String::new())
        } else {
            (format!("on object \"{object}\""), format!(" for role \"{role_display}\""))
        };
        let required: Vec<RoleMethod> = self.roles[role.0].methods.clone();
        for method in required {
            let mut at = if fake { decl.span } else { decl.name.span };
            let mut point = |candidate: &MethodInfo<'_>| {
                if let (true, Some(action)) = (fake, candidate.decl) {
                    at = action.span;
                }
            };
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
                    point(found);
                    if found.private {
                        Some(format!(
                            "Role \"{role_display}\" has no public implementation of action \"{}\"",
                            method.name
                        ))
                    } else if found.is_async != method.is_async {
                        Some(format!(
                            "Action \"{}\" {on} must match async shape{for_role}",
                            method.name
                        ))
                    } else {
                        None
                    }
                } else if let Some(first) = named.first() {
                    point(first);
                    // Та же арность; среди перегрузок — с той же вариадичностью.
                    let same_arity =
                        |candidate: &&&MethodInfo<'_>| candidate.params.len() == params.len();
                    match named
                        .iter()
                        .filter(same_arity)
                        .find(|candidate| candidate.variadic == method.variadic)
                        .or_else(|| named.iter().find(same_arity))
                    {
                        None if fake => Some(format!(
                            "Action \"{}\" {on} does not accept {} parameter(s)",
                            first.name,
                            params.len()
                        )),
                        None => Some(format!(
                            "Object \"{object}\" does role \"{role_display}\" but action \"{}\" does not accept {} parameter(s)",
                            first.name,
                            params.len()
                        )),
                        Some(candidate) => {
                            point(candidate);
                            let mismatch = candidate
                                .params
                                .iter()
                                .zip(&params)
                                .enumerate()
                                .find(|(_, ((_, actual), expected))| actual != *expected);
                            // Хвост показывается как `...T`, а не `@T`.
                            let shown = |ty: &Ty, variadic: bool, position: usize| match ty {
                                Ty::Array(element) if variadic && position + 1 == params.len() => {
                                    format!("...{}", self.type_name(element))
                                }
                                _ => self.type_name(ty),
                            };
                            Some(match mismatch {
                                Some((position, ((_, actual), expected))) => format!(
                                    "Action \"{}\" parameter {} {on} must be {}{for_role}, got {}",
                                    method.name,
                                    position + 1,
                                    shown(expected, method.variadic, position),
                                    shown(actual, candidate.variadic, position)
                                ),
                                None => format!(
                                    "Action \"{}\" {on} must return {}{for_role}, got {}",
                                    method.name,
                                    self.result_name(&result),
                                    self.result_name(&candidate.result)
                                ),
                            })
                        }
                    }
                } else if fake {
                    Some(format!(
                        "The fake of role \"{role_display}\" does not implement action \"{}\"",
                        method.name
                    ))
                } else {
                    Some(format!(
                        "Object \"{object}\" does role \"{role_display}\" but does not implement action \"{}\"",
                        method.name
                    ))
                }
            };
            if let Some(message) = message {
                self.error(at, message);
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

    /// A field cannot hold a `Cell` (#152): an object can reach async code,
    /// and a cell stays in synchronous code.
    fn check_cell_fields(&mut self) {
        let Some(cell) = self.standard.cell else {
            return;
        };
        for object in 0..self.objects.len() {
            if object == cell.0 {
                continue;
            }
            let found: Vec<(Span, String)> = self.objects[object]
                .fields
                .iter()
                .filter(|field| field.ty.holds_object(cell))
                .map(|field| (field.span, field.name.clone()))
                .collect();
            if found.is_empty() {
                continue;
            }
            self.current = self.objects[object].module;
            for (span, name) in found {
                self.error(
                    span,
                    format!("Field \"{name}\" cannot hold a Cell: a cell stays in synchronous code; state an object shares with tasks is an Atomic"),
                );
            }
        }
    }

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
            cell: match lookup(self, "cell", "Cell") {
                Some(TypeRef::Object(id)) => Some(id),
                _ => None,
            },
            operators: Operators {
                adding: role(lookup(self, "operators", "Adding")),
                subtracting: role(lookup(self, "operators", "Subtracting")),
                multiplying: role(lookup(self, "operators", "Multiplying")),
                dividing: role(lookup(self, "operators", "Dividing")),
                remaindering: role(lookup(self, "operators", "Remaindering")),
                negating: role(lookup(self, "operators", "Negating")),
            },
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
                key: false,
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
            Ty::Variant(id, _) if self.enums[id.0].error => vec![Ty::role(self.system.error)],
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
            Ty::Variant(id, args) => {
                let name = &self.enums[id.0].name;
                if args.is_empty() {
                    name.clone()
                } else {
                    format!(
                        "{}<{}>",
                        name,
                        args.iter()
                            .map(|t| self.type_name(t))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
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

/// Перегрузки одного имени: одинаковые сигнатуры неразличимы (у variadic
/// имена не различают — их вызов позиционный), у одной арности — один
/// результат. Арность — число параметров, variadic-хвост — один параметр.
fn overload_conflict(
    methods: &[MethodInfo<'_>],
    method: &MethodInfo<'_>,
    object: &str,
) -> Option<String> {
    for other in methods.iter().filter(|other| other.name == method.name) {
        let same_types = method.variadic == other.variadic
            && method.params.len() == other.params.len()
            && method
                .params
                .iter()
                .zip(&other.params)
                .all(|((_, left), (_, right))| left == right);
        let same_names = method.variadic
            || method
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

/// Имена `hidden` объявлений программы модуля.
fn hidden_names(program: &ast::Program) -> HashSet<String> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            ast::Item::Object(decl) if decl.hidden => Some(decl.name.text.clone()),
            ast::Item::Role(decl) if decl.hidden => Some(decl.name.text.clone()),
            ast::Item::Enum(decl) if decl.hidden => Some(decl.name.text.clone()),
            ast::Item::Const(decl) if decl.hidden => Some(decl.name.text.clone()),
            _ => None,
        })
        .collect()
}
