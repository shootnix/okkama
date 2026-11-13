//! Контекст lowering из THIR: специализации, функции, объявления OIR.
//!
//! Анализ уже проверил программу; здесь только выбираются представления.
//! Generic-объявление анализа — одно, а в OIR каждая специализация
//! (`Box<int>`) — отдельный объект со своими функциями (§10).
//!
//! Lowering идёт по достижимости от точки входа: объект появляется в OIR при
//! первом упоминании его типа (поля, defaults, `destroy` и actions, которыми
//! он реализует роли), остальные actions, конструкторы и natives получают
//! функции при первом вызове. Неиспользуемые объявления — в том числе std —
//! в модуль не попадают и не могут помешать программе.

use std::collections::HashMap;

use super::{LoweringError, Result};
use crate::analysis::registry::{CtorShape, Registry};
use crate::analysis::thir::{self, BodyOwner};
use crate::analysis::ty::{ObjectId as DeclObject, ParamId, RoleId as DeclRole, Ty};
use crate::ir::{self, FuncId, ObjectId, ResultType, RoleId, Type};
use crate::source::Span;
use crate::syntax::ast;

/// Скрытое последнее поле объекта, чей конструктор отдаёт `self`: `true`,
/// пока создание не завершено (#271).
pub(super) const PENDING_FIELD: &str = "$pending";

/// Специализация объекта в OIR.
#[derive(Debug)]
pub(super) struct Instance {
    pub decl: DeclObject,
    /// Подстановка параметров объявления.
    pub env: Vec<(ParamId, Ty)>,
    /// Имя без префикса модуля: `Box<int>`.
    pub name: String,
    pub module: usize,
    pub fields: Vec<(String, Type, Option<FuncId>)>,
    /// Функция action объявления (по индексу action), если оно используется.
    pub methods: Vec<Option<FuncId>>,
    pub destroy: Option<FuncId>,
    pub does: Vec<(RoleId, Vec<FuncId>)>,
    /// Номер скрытого поля `$pending`, если конструктор отдаёт `self`
    /// значением: пока создание не завершено, действия отвергаются (#271).
    pub pending: Option<usize>,
    /// Слоты `once let` (#491): (action, локальная, номер поля).
    pub once_slots: Vec<(usize, thir::LocalId, usize)>,
    pub span: Span,
}

#[derive(Debug)]
pub(super) struct RoleInstance {
    pub decl: DeclRole,
    pub args: Vec<Ty>,
    pub name: String,
    pub module: usize,
    pub methods: Vec<ir::RoleMethod>,
    pub builtin: bool,
    pub span: Span,
}

/// Работа очереди lowering.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Task {
    Entry,
    Method {
        object: ObjectId,
        method: usize,
    },
    /// `given` — переданные поля shorthand по возрастанию индекса (пусто у полного).
    Ctor {
        object: ObjectId,
        ctor: usize,
        given: Vec<usize>,
    },
    Default {
        object: ObjectId,
        field: usize,
    },
    Destroy {
        object: ObjectId,
    },
    Native {
        native: usize,
    },
    /// Функция, строящая значение массива или словаря `const` (#153).
    Global {
        constant: usize,
    },
    /// Конструктор адаптера встроенного значения: параметры — поля.
    AdapterNew {
        object: ObjectId,
    },
    /// Текст значения варианта `Enum.case(payload, ...)` (#290).
    VariantText {
        variant: ir::VariantId,
    },
    /// Слот `text()` роли Error у случая error enum: текст его варианта.
    CaseText {
        object: ObjectId,
    },
    /// Текст `bool` или case enum без payload внутри текста варианта.
    Describe {
        ty: Ty,
    },
    /// Строка payload ошибки в синтаксисе литерала Okkama (#306).
    EscapeText,
    /// Тело анонимного action: `Ctx::closures[index]`.
    Closure {
        index: usize,
    },
    /// `test "name"` корневого модуля (`Registry::tests[index]`).
    Test {
        index: usize,
    },
    /// Intrinsic рантайма без владельца с данной сигнатурой (`test.is`,
    /// `fake.storage.set`, …): одна функция на набор типов.
    Runtime {
        name: String,
        params: Vec<Type>,
        result: ResultType,
    },
    /// Поле cap: функция над владельцем.
    CapField {
        object: ObjectId,
        cap: usize,
        field: usize,
    },
    /// Переходник слота роли: action объекта, затем `succ.take` и
    /// `role.wrap` в тип результата слота.
    Witness {
        object: ObjectId,
        method: usize,
        result: Type,
    },
    /// Переходник для рантайма: действие `name` арности `arity` значения
    /// типа `receiver` (объекта или роли); результат — как у вызова.
    Member {
        receiver: Type,
        name: String,
        arity: usize,
        /// Типы параметров, если имя и арность не выбирают одну перегрузку.
        params: Option<Vec<Type>>,
        /// Тело bound action: receiver — захват замыкания (`fn.closure`).
        bound: bool,
    },
    /// Вход процесса-воркера (§18.1): конструирует receiver и вызывает
    /// синхронное действие `method`. Параметры — аргументы конструктора
    /// `ctor` (`fields` — все поля объекта по порядку объявления, из которых
    /// он восстанавливается), затем аргументы действия.
    WorkerEntry {
        object: ObjectId,
        ctor: usize,
        given: Vec<usize>,
        method: usize,
        fields: bool,
    },
    /// Bound action `method` для воркеров: функция объекта над receiver,
    /// дающая `worker.bind` с его полями.
    WorkerBind {
        object: ObjectId,
        method: usize,
    },
}

/// Cap специализации: владелец, индекс cap объявления, функции полей.
pub(super) type CapInstance = (ObjectId, usize, Vec<(String, FuncId)>);

/// Анонимное action, выделенное функцией `[lambda]`: тело THIR внутри
/// выражения, подстановка и объект окружающего тела, захваты по порядку.
#[derive(Clone, Debug)]
pub(super) struct ClosureTask<'r> {
    pub body: &'r thir::Body,
    pub env: Vec<(ParamId, Ty)>,
    /// Объект окружающего action: владелец лямбды (читает его поля).
    pub object: Option<ObjectId>,
    /// Первый захват — `self` окружающего action.
    pub self_capture: bool,
    /// Локальные тела лямбды, связываемые захваченными значениями.
    pub captures: Vec<(thir::LocalId, Type)>,
    pub sig: ir::FnSig,
}

/// Limits of monomorphization (H-15): a request chain deeper than this is a
/// program that specializes without end (`Grow<Box<T>>` asking for
/// `Grow<Box<Box<T>>>`, …); so is one with more functions than the count.
pub const SPECIALIZATION_DEPTH: u32 = 256;
pub const SPECIALIZATION_COUNT: usize = 1 << 18;

pub(super) struct Ctx<'r, 'a> {
    pub reg: &'r Registry<'a>,
    pub closures: Vec<ClosureTask<'r>>,
    pub caps: Vec<CapInstance>,
    /// Функции тестов корня; `None` — production.
    pub tests: Option<Vec<(String, FuncId)>>,
    bodies: &'r [thir::Body],
    body_index: HashMap<BodyOwner, usize>,
    pub objects: Vec<Instance>,
    object_map: HashMap<(DeclObject, Vec<Ty>), ObjectId>,
    variant_map: HashMap<Ty, ir::VariantId>,
    /// Варианты по `ir::VariantId`: случаи — объекты из `objects` (#290).
    pub variants: Vec<ir::Variant>,
    /// Модуль объявления каждого варианта.
    variant_modules: Vec<usize>,
    pub roles: Vec<RoleInstance>,
    role_map: HashMap<(DeclRole, Vec<Ty>), RoleId>,
    /// Функции в порядке выделения: задача и символ.
    pub tasks: Vec<(Task, String)>,
    task_map: HashMap<Task, FuncId>,
    /// `const` модуля (#153) по индексу `Registry::consts`: номер в `globals`.
    global_map: HashMap<usize, usize>,
    pub globals: Vec<ir::Global>,
    /// Задача, тело которой сейчас понижается: она запрашивает новые.
    pub current: Option<usize>,
    /// Кто запросил каждую задачу и глубина цепочки запросов (H-15).
    requested_by: Vec<Option<usize>>,
    depths: Vec<u32>,
    /// Предел специализаций превышен: текст диагностики с цепочкой.
    pub overflow: Option<String>,
    pub natives: Vec<Option<FuncId>>,
    pub entry: Option<FuncId>,
    /// Префикс символов модуля: пусто у корня, `io.stdout::` у импортов.
    pub prefixes: Vec<String>,
    /// В программе есть `destroy`: lowering выпускает регионы владения (§7).
    pub ownership: bool,
    /// В программе есть `async destroy`: области async-функций закрываются
    /// `region.settle` (#154), если в них может попасть значение, которое
    /// достаёт объект с асинхронным destroy (#328).
    pub settles: bool,
    plain_objects: Vec<bool>,
    /// Специализации объектов и ролей, чьи значения могут достать объект с
    /// `async destroy` (#328): неподвижная точка, как у plain-ности, по
    /// всем специализациям программы, когда тела уже опущены.
    async_objects: Vec<bool>,
    async_roles: Vec<bool>,
    /// Типы значений, принятых областями каждой задачи, чьи области
    /// закрываются `region.settle` (#328); решение — после всех тел.
    pub settle_types: Vec<Option<Vec<Type>>>,
    /// Some value that may reach an object with an `async destroy` is boxed
    /// as `any`, captured by a function value or owns a capability somewhere
    /// in the program (#328): then values of those types may too.
    pub dynamic_async: bool,
    /// `once let` whose value type is plain only if function values and
    /// role values turn out plain (#520): checked when every body is lowered,
    /// with the closures and specializations of the whole program. Each is
    /// (value type, object, action, span, module of the task).
    pub once_checks: Vec<(Type, String, Span, usize)>,
    plain_roles: Vec<bool>,
    /// Plain-ность объявлений ролей по всем объявлениям объектов программы
    /// (не только уже специализированным): роль, которую делает объект с
    /// `destroy`, не plain ещё до его специализации.
    decl_plain_roles: Vec<bool>,
    decl_plain_objects: Vec<bool>,
}

impl<'r, 'a> Ctx<'r, 'a> {
    pub fn new(reg: &'r Registry<'a>, bodies: &'r [thir::Body]) -> Self {
        let body_index = bodies
            .iter()
            .enumerate()
            .map(|(index, body)| (body.owner.clone(), index))
            .collect();
        let prefixes = reg
            .modules
            .iter()
            .enumerate()
            .map(|(index, module)| {
                if reg.is_root(index) {
                    String::new()
                } else {
                    format!("{}::", module.name.replace('/', "."))
                }
            })
            .collect();
        let ownership = reg.objects.iter().any(|object| object.destroy.is_some());
        let settles = reg.objects.iter().any(|object| object.destroy.is_some_and(|destroy| destroy.is_async));
        let decl_plain_roles = decl_plain_roles(reg);
        let decl_plain_objects = decl_plain_objects(reg, &decl_plain_roles);
        Ctx {
            reg,
            closures: Vec::new(),
            caps: Vec::new(),
            tests: None,
            bodies,
            body_index,
            objects: Vec::new(),
            object_map: HashMap::new(),
            variant_map: HashMap::new(),
            variants: Vec::new(),
            variant_modules: Vec::new(),
            roles: Vec::new(),
            role_map: HashMap::new(),
            tasks: Vec::new(),
            task_map: HashMap::new(),
            global_map: HashMap::new(),
            globals: Vec::new(),
            current: None,
            requested_by: Vec::new(),
            depths: Vec::new(),
            overflow: None,
            natives: vec![None; reg.natives.len()],
            entry: None,
            prefixes,
            ownership,
            settles,
            plain_objects: Vec::new(),
            plain_roles: Vec::new(),
            decl_plain_roles,
            decl_plain_objects,
            async_objects: Vec::new(),
            async_roles: Vec::new(),
            settle_types: Vec::new(),
            dynamic_async: false,
            once_checks: Vec::new(),
        }
    }

    pub fn body(&self, owner: &BodyOwner) -> &'r thir::Body {
        &self.bodies[self.body_index[owner]]
    }

    /// Корни достижимости: все объявления корневого модуля (natives, затем
    /// роли, объекты со всеми конструкторами и actions и точка входа в
    /// порядке исходника). Импортированные модули — только по достижимости.
    pub fn collect(&mut self) -> Result<()> {
        // Все файлы корня — в порядке, в котором их дали.
        let reg = self.reg;
        let items: Vec<&ast::Item> = (reg.root_start..reg.modules.len())
            .flat_map(|module| reg.modules[module].program.items.iter())
            .collect();
        for &item in &items {
            if let ast::Item::NativeAction(decl) = item
                && let Some(native) = self
                    .reg
                    .natives
                    .iter()
                    .position(|info| std::ptr::eq(info.decl, decl))
                {
                    self.native_func(native, decl.span)?;
                }
        }
        for &item in &items {
            match item {
                ast::Item::Role(decl) if decl.generics.is_empty() => {
                    let id = self.decl_role(decl);
                    self.role_instance(id, Vec::new(), decl.span)?;
                }
                // Native object — непрозрачный handle рантайма: тип появляется
                // в модуле при первом использовании (`ir_type`).
                // Caps выпускаются при первом использовании (`cap_instance`).
                _ => {}
            }
        }
        for &item in &items {
            match item {
                ast::Item::Object(decl) if decl.generics.is_empty() => {
                    let decl_id = self.decl_object(decl);
                    let id = self.object_instance(decl_id, Vec::new(), decl.span)?;
                    let info = &self.reg.objects[decl_id.0];
                    let shapes: Vec<Vec<usize>> = info
                        .ctors
                        .iter()
                        .map(|ctor| match &ctor.shape {
                            CtorShape::Full { .. } => Vec::new(),
                            CtorShape::Shorthand { fields, .. } => {
                                let mut given = fields.clone();
                                given.sort_unstable();
                                given
                            }
                        })
                        .collect();
                    for (ctor, given) in shapes.into_iter().enumerate() {
                        self.ctor_func(id, ctor, given);
                    }
                    for method in 0..self.reg.objects[decl_id.0].methods.len() {
                        self.method_func(id, method, decl.span)?;
                    }
                }
                ast::Item::Action(action) => {
                    let func = self.allocate(Task::Entry, action.name.text.clone());
                    self.entry = Some(func);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Тесты корня — корни достижимости тестового режима.
    pub fn collect_tests(&mut self) {
        let mut tests = Vec::new();
        for index in 0..self.reg.tests.len() {
            let name = self.reg.tests[index].name.clone();
            let func = self.allocate(Task::Test { index }, format!("test#{index}"));
            tests.push((name, func));
        }
        self.tests = Some(tests);
    }

    fn decl_object(&self, decl: &ast::ObjectDecl) -> DeclObject {
        DeclObject(
            self.reg
                .objects
                .iter()
                .position(|object| object.decl.is_some_and(|known| std::ptr::eq(known, decl)))
                .expect("analysed objects are registered"),
        )
    }

    fn decl_role(&self, decl: &ast::RoleDecl) -> DeclRole {
        DeclRole(
            self.reg
                .roles
                .iter()
                .position(|role| role.decl.is_some_and(|known| std::ptr::eq(known, decl)))
                .expect("analysed roles are registered"),
        )
    }

    /// Функция `native` при первом вызове.
    pub fn native_func(&mut self, native: usize, span: Span) -> Result<FuncId> {
        if let Some(func) = self.natives[native] {
            return Ok(func);
        }
        let _ = span;
        let info = &self.reg.natives[native];
        let symbol = format!("{}{}", self.prefixes[info.module], info.name);
        let func = self.allocate(Task::Native { native }, symbol);
        self.natives[native] = Some(func);
        Ok(func)
    }

    /// Функция action специализации при первом вызове.
    pub fn method_func(&mut self, object: ObjectId, method: usize, _span: Span) -> Result<FuncId> {
        if let Some(func) = self.objects[object.0].methods[method] {
            return Ok(func);
        }
        let info = &self.reg.objects[self.objects[object.0].decl.0];
        let target = &info.methods[method];
        // Overloads получают суффикс `#k` только если имя действительно перегружено.
        let same: Vec<usize> = info
            .methods
            .iter()
            .enumerate()
            .filter(|(_, other)| other.name == target.name)
            .map(|(index, _)| index)
            .collect();
        let suffix = if same.len() > 1 {
            let position = same
                .iter()
                .position(|index| *index == method)
                .expect("listed");
            format!(".{}#{position}", target.name)
        } else {
            format!(".{}", target.name)
        };
        let symbol = self.symbol(object, &suffix);
        let func = self.allocate(Task::Method { object, method }, symbol);
        self.objects[object.0].methods[method] = Some(func);
        Ok(func)
    }

    /// Функция конструктора для набора переданных полей (`given` пуст у полного).
    pub fn ctor_func(&mut self, object: ObjectId, ctor: usize, given: Vec<usize>) -> FuncId {
        let task = Task::Ctor {
            object,
            ctor,
            given: given.clone(),
        };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        let info = &self.reg.objects[self.objects[object.0].decl.0];
        let complete = match &info.ctors[ctor].shape {
            CtorShape::Full { .. } => true,
            CtorShape::Shorthand { fields, .. } => {
                let mut listed = fields.clone();
                listed.sort_unstable();
                listed == given
            }
        };
        let symbol = if complete {
            self.symbol(object, &format!(".new#{ctor}"))
        } else {
            let suffix: Vec<&str> = given
                .iter()
                .map(|field| info.fields[*field].name.as_str())
                .collect();
            self.symbol(object, &format!(".new#{ctor}.{}", suffix.join(".")))
        };
        self.allocate(task, symbol)
    }

    /// Action, которое std объявляет заглушкой: его даёт VM Perl
    /// (`panic "Atomic.value is a VM intrinsic"`), а не тело Okkama.
    pub fn is_intrinsic(&self, object: ObjectId, method: usize) -> bool {
        // Intrinsics, которые даёт рантайм, вызываются как обычные действия.
        if self
            .intrinsic_name(object, super::intrinsics::Member::Method(method))
            .is_some()
        {
            return false;
        }
        let decl = self.objects[object.0].decl;
        let Some(action) = self.reg.objects[decl.0].methods[method].decl else {
            return false;
        };
        // `panic "… is a VM intrinsic";` — первым (за ним бывает `self;`
        // ради типа) или последним (у FutureScope.outcome — после подготовки).
        let marks = |stmt: &ast::Stmt| {
            matches!(
                &stmt.kind,
                ast::StmtKind::Expr { expr, condition: None }
                    if matches!(
                        &expr.kind,
                        ast::ExprKind::Panic { value }
                            if matches!(
                                &value.kind,
                                ast::ExprKind::Literal(ast::Literal::String(text))
                                    if text.value.ends_with("is a VM intrinsic")
                            )
                    )
            )
        };
        action.body.stmts.first().is_some_and(marks) || action.body.stmts.last().is_some_and(marks)
    }

    /// Модуль, в чьём файле лежит тело функции (для сообщений об ошибках).
    pub fn task_module(&self, func: usize) -> usize {
        match &self.tasks[func].0 {
            Task::Entry => self.reg.entry_module,
            Task::Method { object, .. }
            | Task::Ctor { object, .. }
            | Task::Default { object, .. }
            | Task::Destroy { object }
            | Task::AdapterNew { object }
            | Task::CaseText { object }
            | Task::Witness { object, .. }
            | Task::WorkerEntry { object, .. }
            | Task::WorkerBind { object, .. }
            | Task::CapField { object, .. } => self.objects[object.0].module,
            Task::VariantText { variant } => self.variant_modules[variant.0],
            Task::Closure { index } => self.closures[*index].body.module,
            Task::Test { index } => self.reg.tests[*index].module,
            Task::Runtime { .. } | Task::Member { .. } | Task::Describe { .. } | Task::EscapeText => {
                self.reg.root()
            }
            Task::Native { native } => self.reg.natives[*native].module,
            Task::Global { constant } => self.reg.consts[*constant].module,
        }
    }

    /// Cap `cap` специализации объекта: функции полей выделяются сразу.
    pub fn cap_instance(
        &mut self,
        decl: DeclObject,
        args: Vec<Ty>,
        cap: usize,
        span: Span,
    ) -> Result<ir::CapId> {
        let object = self.object_instance(decl, args, span)?;
        if let Some(index) = self
            .caps
            .iter()
            .position(|(owner, known, _)| *owner == object && *known == cap)
        {
            return Ok(ir::CapId(index));
        }
        let index = self.caps.len();
        self.caps.push((object, cap, Vec::new()));
        let reg = self.reg;
        let info = &reg.objects[decl.0].caps[cap];
        let mut fields = Vec::new();
        for (field, (name, ..)) in info.fields.iter().enumerate() {
            let symbol = self.symbol(object, &format!("::{}.{name}", info.name));
            fields.push((
                name.clone(),
                self.allocate(Task::CapField { object, cap, field }, symbol),
            ));
        }
        self.caps[index].2 = fields;
        Ok(ir::CapId(index))
    }

    /// Функция intrinsic рантайма `name` с сигнатурой `params -> result`.
    pub fn runtime_intrinsic(
        &mut self,
        name: &str,
        params: Vec<Type>,
        result: ResultType,
    ) -> FuncId {
        let task = Task::Runtime {
            name: name.to_string(),
            params,
            result,
        };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        let symbol = format!("{name}#{}", self.tasks.len());
        self.allocate(task, symbol)
    }

    /// Переходник действия `name` значения типа `receiver` для рантайма.
    pub fn member_thunk(
        &mut self,
        receiver: Type,
        name: &str,
        arity: usize,
        params: Option<Vec<Type>>,
        bound: bool,
    ) -> FuncId {
        let task = Task::Member {
            receiver,
            name: name.to_string(),
            arity,
            params,
            bound,
        };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        let symbol = format!("member.{name}#{}", self.tasks.len());
        self.allocate(task, symbol)
    }

    /// Действие `name` арности `arity` специализации `object`; с `params` —
    /// перегрузка именно с этими типами параметров.
    pub fn member_method(
        &mut self,
        object: ObjectId,
        name: &str,
        arity: usize,
        params: Option<&[Type]>,
        span: Span,
    ) -> Result<Option<usize>> {
        let env = self.objects[object.0].env.clone();
        let reg = self.reg;
        let info = &reg.objects[self.objects[object.0].decl.0];
        for (index, method) in info.methods.iter().enumerate() {
            if method.name != name || method.params.len() != arity {
                continue;
            }
            let Some(params) = params else {
                return Ok(Some(index));
            };
            let mut types = Vec::new();
            for (_, ty) in &method.params {
                types.push(self.ir_type(&ty.subst(&env), span)?);
            }
            if types == params {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    /// Вход воркера для `spawn` действия `method` специализации `object`.
    pub fn worker_entry(
        &mut self,
        object: ObjectId,
        ctor: usize,
        given: Vec<usize>,
        method: usize,
        fields: bool,
    ) -> FuncId {
        let task = Task::WorkerEntry {
            object,
            ctor,
            given,
            method,
            fields,
        };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        let name = &self.reg.objects[self.objects[object.0].decl.0].methods[method].name;
        let symbol = self.symbol(object, &format!(".{name}$worker#{}", self.tasks.len()));
        self.allocate(task, symbol)
    }

    /// Функция объекта, создающая bound action `method` для воркеров.
    pub fn worker_bind(&mut self, object: ObjectId, method: usize) -> FuncId {
        let task = Task::WorkerBind { object, method };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        let name = &self.reg.objects[self.objects[object.0].decl.0].methods[method].name;
        let symbol = self.symbol(object, &format!(".{name}$bind#{}", self.tasks.len()));
        self.allocate(task, symbol)
    }

    /// Конструктор, восстанавливающий receiver воркера из всех его полей
    /// (как Perl: поля — именованные аргументы): `(ctor, given, поле на
    /// каждый параметр по порядку)`.
    pub fn worker_ctor(&self, object: ObjectId) -> Option<(usize, Vec<usize>, Vec<usize>)> {
        let info = &self.reg.objects[self.objects[object.0].decl.0];
        let all: Vec<usize> = (0..info.fields.len()).collect();
        info.ctors
            .iter()
            .enumerate()
            .find_map(|(index, ctor)| match &ctor.shape {
                CtorShape::Shorthand { fields, .. } => {
                    let mut allowed = fields.clone();
                    allowed.sort_unstable();
                    (allowed == all).then(|| (index, all.clone(), all.clone()))
                }
                CtorShape::Full {
                    params, variadic, ..
                } => {
                    if *variadic || params.len() != all.len() {
                        return None;
                    }
                    let order: Option<Vec<usize>> = params
                        .iter()
                        .map(|(name, ty)| {
                            info.fields
                                .iter()
                                .position(|field| field.name == *name && field.ty == *ty)
                        })
                        .collect();
                    let order = order?;
                    let mut distinct = order.clone();
                    distinct.sort_unstable();
                    (distinct == all).then(|| (index, Vec::new(), order))
                }
            })
    }

    /// Типы параметров конструктора `ctor` специализации `object`.
    pub fn ctor_param_types(
        &mut self,
        object: ObjectId,
        ctor: usize,
        given: &[usize],
        span: Span,
    ) -> Result<Vec<Type>> {
        let env = self.objects[object.0].env.clone();
        let reg = self.reg;
        let info = &reg.objects[self.objects[object.0].decl.0];
        match &info.ctors[ctor].shape {
            CtorShape::Full { params, .. } => params
                .iter()
                .map(|(_, ty)| self.ir_type(&ty.subst(&env), span))
                .collect(),
            CtorShape::Shorthand { .. } => Ok(given
                .iter()
                .map(|field| self.objects[object.0].fields[*field].1.clone())
                .collect()),
        }
    }

    /// Конструктор адаптера `object` (`obj.new` из полей по порядку).
    /// The global of constant `constant` (#153): its init function is lowered
    /// once, and every use reads the value built at program start.
    pub fn global(&mut self, constant: usize, span: Span) -> Result<usize> {
        if let Some(index) = self.global_map.get(&constant) {
            return Ok(*index);
        }
        let Some(crate::analysis::registry::ConstState::Done(Some(result))) = self.reg.const_state.get(constant) else {
            return Err(LoweringError::invalid(span, "internal compiler error: a constant without a value"));
        };
        let ty = self.ir_type(&result.ty.clone(), span)?;
        let symbol = format!("{}const.{}", self.prefixes[self.reg.consts[constant].module], self.reg.consts[constant].name);
        let init = self.allocate(Task::Global { constant }, symbol);
        let index = self.globals.len();
        self.globals.push(ir::Global { ty, init });
        self.global_map.insert(constant, index);
        Ok(index)
    }

    pub fn adapter_ctor(&mut self, object: ObjectId) -> FuncId {
        let task = Task::AdapterNew { object };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        let symbol = self.symbol(object, ".new");
        self.allocate(task, symbol)
    }

    pub fn allocate(&mut self, task: Task, symbol: String) -> FuncId {
        let func = FuncId(self.tasks.len());
        let depth = self.current.map_or(0, |parent| self.depths[parent] + 1);
        if self.overflow.is_none() && (depth > SPECIALIZATION_DEPTH || self.tasks.len() >= SPECIALIZATION_COUNT) {
            self.overflow = Some(self.chain(&symbol, depth));
        }
        self.task_map.insert(task.clone(), func);
        self.tasks.push((task, symbol));
        self.requested_by.push(self.current);
        self.depths.push(depth);
        func
    }

    /// The diagnostic of a program that specializes without end: the newest
    /// requests of the chain that led to `symbol`.
    fn chain(&self, symbol: &str, depth: u32) -> String {
        // Names of a growing chain grow too: keep their start and end.
        let short = |name: &str| {
            let chars: Vec<char> = name.chars().collect();
            if chars.len() <= 80 {
                return name.to_string();
            }
            let head: String = chars[..40].iter().collect();
            let tail: String = chars[chars.len() - 30..].iter().collect();
            format!("{head}…{tail}")
        };
        let mut names = vec![short(symbol)];
        let mut at = self.current;
        while let Some(task) = at {
            if names.len() == 6 {
                names.push("…".into());
                break;
            }
            names.push(short(&self.tasks[task].1));
            at = self.requested_by[task];
        }
        let what = if depth > SPECIALIZATION_DEPTH {
            format!("a chain of more than {SPECIALIZATION_DEPTH} generic specializations")
        } else {
            format!("more than {SPECIALIZATION_COUNT} generic specializations")
        };
        format!("The program requests {what}, each asking for the next: {}", names.join(" <- "))
    }

    pub fn enum_id(&self, id: crate::analysis::ty::EnumId) -> ir::EnumId {
        ir::EnumId(
            self.reg.enums[..id.0]
                .iter()
                .filter(|decl| !decl.variant)
                .count(),
        )
    }

    /// Функция текста значения варианта: `(variant) -> str`.
    pub fn variant_text(&mut self, variant: ir::VariantId) -> FuncId {
        let task = Task::VariantText { variant };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        let symbol = format!("{}.$text", self.variants[variant.0].name);
        self.allocate(task, symbol)
    }

    /// Слот `text()` роли Error у объекта случая error enum.
    fn case_text(&mut self, object: ObjectId, symbol: String) -> FuncId {
        let task = Task::CaseText { object };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        self.allocate(task, symbol)
    }

    /// Функция текста `bool` (`true`/`false`) или case enum без payload.
    /// `(str) -> str`: экранирование строки payload для `text()` ошибки.
    pub fn escape_text(&mut self) -> FuncId {
        if let Some(func) = self.task_map.get(&Task::EscapeText) {
            return *func;
        }
        self.allocate(Task::EscapeText, "$escape.text".to_string())
    }

    pub fn describe(&mut self, ty: &Ty) -> FuncId {
        let task = Task::Describe { ty: ty.clone() };
        if let Some(func) = self.task_map.get(&task) {
            return *func;
        }
        let symbol = match ty {
            Ty::Enum(id) => format!("$describe.enum{}", id.0),
            _ => "$describe.bool".to_string(),
        };
        self.allocate(task, symbol)
    }

    /// Тип анализа варианта.
    pub fn variant_ty(&self, variant: ir::VariantId) -> Option<Ty> {
        self.variant_map
            .iter()
            .find(|(_, value)| **value == variant)
            .map(|(ty, _)| ty.clone())
    }

    /// Вариант и номер случая объекта `object`, если это раскладка случая.
    pub fn case_of(&self, object: ObjectId) -> Option<(ir::VariantId, usize)> {
        self.variants.iter().enumerate().find_map(|(id, variant)| {
            let case = variant.cases.iter().position(|(_, case)| *case == object)?;
            Some((ir::VariantId(id), case))
        })
    }

    /// Вариант (#290): по объекту раскладки на каждый случай, поля объекта —
    /// payload этого случая. Значение создаёт `variant.new`, поэтому у
    /// объектов случаев нет конструкторов.
    fn variant_instance(&mut self, ty: &Ty, span: Span) -> Result<ir::VariantId> {
        if let Some(id) = self.variant_map.get(ty) {
            return Ok(*id);
        }
        let Ty::Variant(decl_id, args) = ty else {
            unreachable!()
        };
        let reg = self.reg;
        let decl = &reg.enums[decl_id.0];
        let env: Vec<_> = decl.params.iter().copied().zip(args.clone()).collect();
        let module = decl.module;
        let error = decl.error;
        let id = ir::VariantId(self.variants.len());
        self.variant_map.insert(ty.clone(), id);
        // Error enum не generic: одно объявление — одна специализация, и имя
        // объекта случая — имя типа ошибки в сообщениях о неперехваченном отказе.
        let name = if error {
            decl.name.clone()
        } else {
            format!("$variant_{}_{}", decl.name, id.0)
        };
        self.variants.push(ir::Variant {
            name: format!("{}{name}", self.prefixes[module]),
            cases: Vec::new(),
            span,
        });
        self.variant_modules.push(module);
        let mut cases = Vec::new();
        for case in &decl.cases {
            let object = ObjectId(self.objects.len());
            self.objects.push(Instance {
                decl: DeclObject(usize::MAX),
                env: env.clone(),
                name: if error { name.clone() } else { format!("{name}.{case}") },
                module,
                fields: Vec::new(),
                methods: Vec::new(),
                pending: None,
                once_slots: Vec::new(),
                destroy: None,
                does: Vec::new(),
                span,
            });
            cases.push((case.clone(), object));
        }
        self.variants[id.0].cases = cases.clone();
        for ((case, object), payload) in cases.iter().zip(&decl.payloads) {
            let mut fields = Vec::new();
            for (field, field_ty) in payload {
                fields.push((field.clone(), self.ir_type(&field_ty.subst(&env), span)?, None));
            }
            self.objects[object.0].fields = fields;
            if error {
                let symbol = format!("{}{name}.{case}.text", self.prefixes[module]);
                let text = self.case_text(*object, symbol);
                let role = self.role_instance(self.reg.system.error, Vec::new(), span)?;
                self.objects[object.0].does = vec![(role, vec![text])];
            }
        }
        // A payload may own resources. Publish its plainness before callers emit
        // region keep/track operations for the returned variant value.
        self.compute_plain();
        Ok(id)
    }

    // ----- типы -----

    /// Тип OIR для типа анализа (после подстановки параметров).
    pub fn ir_type(&mut self, ty: &Ty, span: Span) -> Result<Type> {
        Ok(match ty {
            Ty::Int => Type::Int,
            Ty::Bool => Type::Bool,
            Ty::Byte => Type::Byte,
            Ty::Char => Type::Char,
            Ty::Float => Type::Float,
            Ty::Str => Type::Str,
            Ty::Never => Type::Never,
            Ty::Array(element) | Ty::Varargs(element) => {
                Type::Array(Box::new(self.ir_type(element, span)?))
            }
            Ty::Map(key, value) => Type::Map(
                Box::new(self.ir_type(key, span)?),
                Box::new(self.ir_type(value, span)?),
            ),
            Ty::Object(id, args) => Type::Object(self.object_instance(*id, args.clone(), span)?),
            Ty::Role(id, args) => Type::Role(self.role_instance(*id, args.clone(), span)?),
            Ty::Enum(id) => Type::Enum(self.enum_id(*id)),
            Ty::Variant(..) => Type::Variant(self.variant_instance(ty, span)?),
            Ty::Successor(inner) => match self.ir_type(inner, span)? {
                Type::Object(object) => Type::Successor(object),
                // `^R` роли: обязательство проверил анализ; в OIR — ссылка роли.
                role @ Type::Role(_) => role,
                _ => {
                    return Err(LoweringError::unsupported(
                        span,
                        "IR lowering of successor role actions is not implemented yet.",
                    ));
                }
            },
            Ty::Rat => Type::Rat,
            Ty::Any => Type::Any,
            Ty::Range => Type::Range,
            Ty::Action(action) => {
                let mut params = Vec::new();
                for param in &action.params {
                    params.push(self.ir_type(param, span)?);
                }
                let mut result = match &action.result {
                    None => ResultType::Void,
                    Some(result) => ResultType::Value(self.ir_type(result, span)?),
                };
                if action.is_async {
                    result = ResultType::Value(Type::Future(Box::new(result)));
                }
                Type::Fn(Box::new(ir::FnSig { params, result }))
            }
            Ty::Future(result) => Type::Future(Box::new(match result {
                None => ResultType::Void,
                Some(result) => ResultType::Value(self.ir_type(result, span)?),
            })),
            Ty::Handle(id) => Type::Handle(ir::HandleId(id.0)),
            Ty::Cap(decl, args, cap) => {
                Type::Cap(self.cap_instance(*decl, args.clone(), *cap, span)?)
            }
            Ty::Test => return Err(unsupported_type(span, "test mode")),
            Ty::Param(_) | Ty::Error => {
                return Err(LoweringError::invalid(
                    span,
                    format!(
                        "internal compiler error: type {} reached lowering",
                        self.reg.plain_type_name(ty)
                    ),
                ));
            }
        })
    }

    pub fn ir_result(
        &mut self,
        result: &Option<Ty>,
        env: &[(ParamId, Ty)],
        span: Span,
    ) -> Result<ResultType> {
        Ok(match result {
            None => ResultType::Void,
            Some(ty) => ResultType::Value(self.ir_type(&ty.subst(env), span)?),
        })
    }

    // ----- специализации -----

    /// Специализация объекта (создаётся один раз, со всеми функциями).
    pub fn object_instance(
        &mut self,
        decl: DeclObject,
        args: Vec<Ty>,
        span: Span,
    ) -> Result<ObjectId> {
        if let Some(id) = self.object_map.get(&(decl, args.clone())) {
            return Ok(*id);
        }
        if self.reg.is_adapter(decl) {
            // Поля и роли — из системного объявления; actions порождает lowering.
            let id = self.reserve_object(decl, args, span);
            let env = self.objects[id.0].env.clone();
            let reg = self.reg;
            let mut fields = Vec::new();
            for field in &reg.objects[decl.0].fields {
                let ty = self.ir_type(&field.ty.subst(&env), span)?;
                fields.push((field.name.clone(), ty, None));
            }
            self.objects[id.0].fields = fields;
            self.collect_does(id)?;
            self.compute_plain();
            return Ok(id);
        }
        if decl == self.reg.system.cancelled {
            let id = self.reserve_object(decl, args, span);
            self.collect_does(id)?;
            self.compute_plain();
            return Ok(id);
        }
        if decl == self.reg.system.native_failure {
            // Поля — данные отказа provider; actions читают их (`text` — `message`).
            let id = self.reserve_object(decl, args, span);
            self.objects[id.0].fields = ir::Object::NATIVE_FAILURE_FIELDS
                .iter()
                .map(|name| (name.to_string(), Type::Str, None))
                .collect();
            self.collect_does(id)?;
            self.compute_plain();
            return Ok(id);
        }
        if self.reg.is_system_object(decl) {
            return Err(LoweringError::unsupported(
                span,
                format!(
                    "IR lowering of `{}` values is not implemented yet.",
                    self.reg.objects[decl.0].name
                ),
            ));
        }
        let id = self.reserve_object(decl, args, span);
        self.collect_object(id)?;
        self.collect_does(id)?;
        self.compute_plain();
        Ok(id)
    }

    /// Место специализации без функций (их выделяет `collect_object`).
    fn reserve_object(&mut self, decl: DeclObject, args: Vec<Ty>, span: Span) -> ObjectId {
        let info = &self.reg.objects[decl.0];
        let env: Vec<(ParamId, Ty)> = info
            .params
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .collect();
        let name = if args.is_empty() {
            info.name.clone()
        } else {
            self.reg.plain_type_name(&Ty::Object(decl, args.clone()))
        };
        let module = info.module;
        let id = ObjectId(self.objects.len());
        self.objects.push(Instance {
            decl,
            env: env.clone(),
            name,
            module,
            fields: Vec::new(),
            methods: vec![None; info.methods.len()],
            destroy: None,
            does: Vec::new(),
            pending: None,
            once_slots: Vec::new(),
            span: info.decl.map_or(span, |decl| decl.span),
        });
        self.object_map.insert((decl, args), id);
        id
    }

    fn symbol(&self, object: ObjectId, rest: &str) -> String {
        let instance = &self.objects[object.0];
        format!("{}{}{rest}", self.prefixes[instance.module], instance.name)
    }

    /// Поля, defaults и `destroy` специализации.
    fn collect_object(&mut self, id: ObjectId) -> Result<()> {
        let decl = self.objects[id.0].decl;
        let env = self.objects[id.0].env.clone();
        let reg = self.reg;
        let info = &reg.objects[decl.0];
        let Some(object_decl) = info.decl else {
            return Ok(());
        };
        let mut field_index = 0;
        for member in &object_decl.members {
            match member {
                ast::ObjectMember::Has(has) => {
                    for field_name in &has.names {
                        let Some(field) = info.fields.get(field_index) else {
                            continue;
                        };
                        if field.name != field_name.text {
                            continue;
                        }
                        let ty = self.ir_type(&field.ty.subst(&env), field.span)?;
                        let default = match field.default {
                            Some(_) => {
                                let symbol = self.symbol(id, &format!(".default.{}", field.name));
                                Some(self.allocate(
                                    Task::Default {
                                        object: id,
                                        field: field_index,
                                    },
                                    symbol,
                                ))
                            }
                            None => None,
                        };
                        self.objects[id.0]
                            .fields
                            .push((field.name.clone(), ty, default));
                        field_index += 1;
                    }
                }
                ast::ObjectMember::Destroy(_) => {
                    let symbol = self.symbol(id, ".destroy");
                    let func = self.allocate(Task::Destroy { object: id }, symbol);
                    self.objects[id.0].destroy = Some(func);
                }
                _ => {}
            }
        }
        if self.escapes_self(decl) {
            let instance = &mut self.objects[id.0];
            instance.pending = Some(instance.fields.len());
            instance.fields.push((PENDING_FIELD.to_string(), Type::Bool, None));
        }
        // Слоты `once let` (#491): по скрытому полю `Maybe<T>` на каждый
        // `once let` верхнего уровня тел действий объявления.
        let mut sites = Vec::new();
        for body in self.bodies.iter() {
            let BodyOwner::Method { object, method } = body.owner else {
                continue;
            };
            if object != decl {
                continue;
            }
            for stmt in &body.block.stmts {
                if let thir::StmtKind::Let { local, once: true, .. } = &stmt.kind {
                    sites.push((method, *local, body.locals[local.0].ty.clone(), stmt.span));
                }
            }
        }
        let env = self.objects[id.0].env.clone();
        for (method, local, ty, span) in sites {
            let maybe = Ty::Variant(crate::analysis::ty::EnumId(0), vec![ty.subst(&env)]);
            let slot = self.ir_type(&maybe, span)?;
            let instance = &mut self.objects[id.0];
            let field = instance.fields.len();
            instance.fields.push((format!("$once{}", instance.once_slots.len()), slot, None));
            instance.once_slots.push((method, local, field));
        }
        Ok(())
    }

    /// Поле слота `once let` локальной `local` действия `method` (#491).
    pub fn once_field(&self, object: ObjectId, method: usize, local: thir::LocalId) -> Option<usize> {
        self.objects[object.0]
            .once_slots
            .iter()
            .find(|(m, l, _)| *m == method && *l == local)
            .map(|(_, _, field)| *field)
    }

    /// Конструктор объявления отдаёт `self` значением (§11.4): сотрудник может
    /// получить объект, создание которого ещё не завершено.
    fn escapes_self(&self, decl: DeclObject) -> bool {
        self.bodies.iter().any(|body| {
            matches!(body.owner, BodyOwner::Ctor { object, .. } if object == decl)
                && thir::first_expr(&body.block, &|expr| matches!(expr.kind, thir::ExprKind::SelfValue)).is_some()
        })
    }

    /// `does R`: функция каждого слота роли (анализ проверил соответствие).
    fn collect_does(&mut self, id: ObjectId) -> Result<()> {
        let decl = self.objects[id.0].decl;
        let env = self.objects[id.0].env.clone();
        let reg = self.reg;
        let info = &reg.objects[decl.0];
        let span = self.objects[id.0].span;
        let mut does = Vec::new();
        for (role, role_args) in &info.does {
            let role_args: Vec<Ty> = role_args.iter().map(|arg| arg.subst(&env)).collect();
            let role_id = self.role_instance(*role, role_args.clone(), span)?;
            let role_env: Vec<(ParamId, Ty)> = reg.roles[role.0]
                .params
                .iter()
                .copied()
                .zip(role_args.iter().cloned())
                .collect();
            let mut slots = Vec::new();
            for (slot, method) in reg.roles[role.0].methods.iter().enumerate() {
                let params: Vec<Ty> = method
                    .params
                    .iter()
                    .map(|(_, ty)| ty.subst(&role_env))
                    .collect();
                let found = info.methods.iter().position(|candidate| {
                    candidate.name == method.name
                        && candidate.variadic == method.variadic
                        && candidate.params.len() == params.len()
                        && candidate
                            .params
                            .iter()
                            .zip(&params)
                            .all(|((_, left), right)| left.subst(&env) == *right)
                        && !candidate.hidden
                });
                let Some(found) = found else {
                    return Err(LoweringError::unsupported(
                        span,
                        format!(
                            "IR lowering of role \"{}\" implemented by \"{}\" is not implemented yet.",
                            reg.roles[role.0].name, info.name
                        ),
                    ));
                };
                let mut func = self.method_func(id, found, span)?;
                // Результат слота другой (`^Box` против `^Package`): переходник.
                if let Some(expected) = self.roles[role_id.0]
                    .methods
                    .get(slot)
                    .map(|m| m.result.clone())
                {
                    let target = &info.methods[found];
                    let mut own = self.ir_result(&target.result, &env, span)?;
                    if target.is_async {
                        own = ResultType::Value(Type::Future(Box::new(own)));
                    }
                    if let (true, ResultType::Value(result @ Type::Role(_))) =
                        (own != expected, &expected)
                    {
                        let task = Task::Witness {
                            object: id,
                            method: found,
                            result: result.clone(),
                        };
                        func = match self.task_map.get(&task) {
                            Some(existing) => *existing,
                            None => {
                                let symbol = self.symbol(
                                    id,
                                    &format!(".{}$as${}", target.name, reg.roles[role.0].name),
                                );
                                self.allocate(task, symbol)
                            }
                        };
                    }
                }
                slots.push(func);
            }
            does.push((role_id, slots));
        }
        self.objects[id.0].does = does;
        Ok(())
    }

    /// Специализация роли; системная `Error` — встроенная роль OIR.
    pub fn role_instance(&mut self, decl: DeclRole, args: Vec<Ty>, span: Span) -> Result<RoleId> {
        if let Some(id) = self.role_map.get(&(decl, args.clone())) {
            return Ok(*id);
        }
        let id = self.reserve_role(decl, args, span);
        self.collect_role(id)?;
        Ok(id)
    }

    fn reserve_role(&mut self, decl: DeclRole, args: Vec<Ty>, span: Span) -> RoleId {
        let reg = self.reg;
        let info = &reg.roles[decl.0];
        let builtin = decl == reg.system.error;
        let name = if args.is_empty() {
            info.name.clone()
        } else {
            reg.plain_type_name(&Ty::Role(decl, args.clone()))
        };
        let id = RoleId(self.roles.len());
        self.roles.push(RoleInstance {
            decl,
            args: args.clone(),
            name,
            module: if builtin { reg.root() } else { info.module },
            methods: Vec::new(),
            builtin,
            span: info.decl.map_or(span, |decl| decl.span),
        });
        self.role_map.insert((decl, args), id);
        id
    }

    /// Сигнатуры слотов роли.
    fn collect_role(&mut self, id: RoleId) -> Result<()> {
        let reg = self.reg;
        let (decl, span) = (self.roles[id.0].decl, self.roles[id.0].span);
        let info = &reg.roles[decl.0];
        let env: Vec<(ParamId, Ty)> = info
            .params
            .iter()
            .copied()
            .zip(self.roles[id.0].args.iter().cloned())
            .collect();
        let mut methods = Vec::new();
        for method in &info.methods {
            let mut params = Vec::new();
            for (_, ty) in &method.params {
                params.push(self.ir_type(&ty.subst(&env), span)?);
            }
            let mut result = self.ir_result(&method.result, &env, span)?;
            // Слот async-действия возвращает Future его результата.
            if method.is_async {
                result = ResultType::Value(Type::Future(Box::new(result)));
            }
            methods.push(ir::RoleMethod {
                name: method.name.clone(),
                params,
                result,
            });
        }
        self.roles[id.0].methods = methods;
        self.compute_plain();
        Ok(())
    }

    // ----- plain-типы (§3.3) -----

    /// Plain-ность объектов и ролей — неподвижная точка: объект не plain, если
    /// у него есть `destroy` или поле не plain типа; роль — если её делает не
    /// plain объект.
    fn compute_plain(&mut self) {
        self.plain_objects = vec![true; self.objects.len()];
        self.plain_roles = vec![true; self.roles.len()];
        loop {
            let mut changed = false;
            for index in 0..self.objects.len() {
                let plain = self.objects[index].destroy.is_none()
                    && self.objects[index]
                        .fields
                        .iter()
                        .all(|(_, ty, _)| self.is_plain(ty));
                if self.plain_objects[index] && !plain {
                    self.plain_objects[index] = false;
                    changed = true;
                }
            }
            for index in 0..self.objects.len() {
                if !self.plain_objects[index] {
                    for (role, _) in &self.objects[index].does {
                        if self.plain_roles[role.0] {
                            self.plain_roles[role.0] = false;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Какие специализации могут достать объект с `async destroy` (#328):
    /// объект — свой асинхронный destroy или такое поле; роль — её делает
    /// такой объект. Считается, когда все тела опущены: значение роли
    /// держит только объект, чья специализация уже есть.
    pub fn compute_async(&mut self) {
        self.async_objects = vec![false; self.objects.len()];
        self.async_roles = vec![false; self.roles.len()];
        if !self.settles {
            return;
        }
        loop {
            let mut changed = false;
            for index in 0..self.objects.len() {
                // System objects (`Cancelled`, `NativeFailure`) have no destroy.
                let own = self.reg.objects.get(self.objects[index].decl.0)
                    .is_some_and(|info| info.destroy.is_some_and(|destroy| destroy.is_async));
                let reaching = own || self.objects[index].fields.iter().any(|(_, ty, _)| self.may_async_destroy(ty));
                if !self.async_objects[index] && reaching {
                    self.async_objects[index] = true;
                    changed = true;
                }
            }
            for index in 0..self.objects.len() {
                if self.async_objects[index] {
                    for (role, _) in &self.objects[index].does {
                        if !self.async_roles[role.0] {
                            self.async_roles[role.0] = true;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Значение типа может достать объект с `async destroy`, так что
    /// область, которая им владеет, закрывается `region.settle` (#328).
    /// Ошибка в сторону `true` стоит только лишней точки ожидания, в
    /// сторону `false` — пропущенного destroy, поэтому всё, что не доказано,
    /// — может: неизвестные специализации, и `any`, замыкания и cap, если в
    /// программе в них попадает хоть одно такое значение.
    pub fn may_async_destroy(&self, ty: &Type) -> bool {
        if !self.settles {
            return false;
        }
        match ty {
            Type::Object(object) | Type::Successor(object) => self.async_objects.get(object.0).copied().unwrap_or(true),
            Type::Role(role) => self.async_roles.get(role.0).copied().unwrap_or(true),
            Type::Variant(id) => self.variants.get(id.0).is_none_or(|variant| {
                variant
                    .cases
                    .iter()
                    .any(|(_, object)| self.async_objects.get(object.0).copied().unwrap_or(true))
            }),
            Type::Array(element) | Type::Builder(element) => self.may_async_destroy(element),
            Type::Map(_, value) | Type::MapBuilder(_, value) => self.may_async_destroy(value),
            Type::Any | Type::Fn(_) | Type::Cap(_) => self.dynamic_async,
            _ => false,
        }
    }

    /// Значение типа не достигает идентичностей с `destroy` (§3.3).
    pub fn is_plain(&self, ty: &Type) -> bool {
        match ty {
            Type::Object(object) | Type::Successor(object) => {
                self.plain_objects.get(object.0).copied().unwrap_or(true)
            }
            Type::Role(role) => {
                self.plain_roles.get(role.0).copied().unwrap_or(true)
                    && self.roles.get(role.0).is_none_or(|instance| {
                        self.decl_plain_roles[instance.decl.0]
                            && instance.args.iter().all(|arg| {
                                decl_plain_ty(arg, &self.decl_plain_objects, &self.decl_plain_roles)
                            })
                    })
            }
            Type::Variant(id) => self.variants.get(id.0).is_none_or(|variant| {
                variant
                    .cases
                    .iter()
                    .all(|(_, object)| self.plain_objects.get(object.0).copied().unwrap_or(true))
            }),
            Type::Array(element) | Type::Builder(element) => self.is_plain(element),
            Type::Map(_, value) | Type::MapBuilder(_, value) => self.is_plain(value),
            // Boxes, captures and Future dependencies may retain owned objects.
            Type::Any | Type::Fn(_) | Type::Cap(_) | Type::Future(_) => false,
            _ => true,
        }
    }

    /// Whether the type may be plain once function and role values are
    /// judged with the whole program (#520): `is_plain` with every function
    /// value and every role taken as plain. A type that fails even this
    /// reaches an object with `destroy` directly and is refused at once.
    pub fn plain_but_for_values(&self, ty: &Type) -> bool {
        let objects = self.settled_objects(&|_| true, &|_| true);
        settled_plain(ty, &objects, &|_| true, &|_| true, &|id| self.variant_plain(id, &objects))
    }

    /// A variant is plain when the objects of all its cases are.
    fn variant_plain(&self, id: usize, objects: &[bool]) -> bool {
        self.variants
            .get(id)
            .is_none_or(|variant| variant.cases.iter().all(|(_, object)| objects.get(object.0).copied().unwrap_or(true)))
    }

    /// Plainness when every body is lowered (#520). A function value is
    /// created only by `fn.closure` (a lambda or a bound action) with a
    /// snapshot of its captures, so values of one signature are plain when
    /// every closure of that signature captures only plain values. A role
    /// value holds only an object whose specialization exists by now, as
    /// for `async destroy` (#328), so a role is plain when every object that
    /// does it is. `any` and capabilities stay conservative. A fixed point:
    /// closures capture closures and objects hold roles.
    pub fn settled_plain_types(&self, closures: &[(ir::FnSig, Vec<Type>)]) -> SettledPlain {
        let mut fns: HashMap<ir::FnSig, bool> = closures.iter().map(|(sig, _)| (sig.clone(), true)).collect();
        let mut roles = vec![true; self.roles.len()];
        loop {
            let fn_plain = |sig: &ir::FnSig| fns.get(sig).copied().unwrap_or(true);
            let role_plain = |role: usize| roles.get(role).copied().unwrap_or(true);
            let objects = self.settled_objects(&fn_plain, &role_plain);
            let mut changed = false;
            let mut next_roles = roles.clone();
            for (index, object) in self.objects.iter().enumerate() {
                if !objects[index] {
                    for (role, _) in &object.does {
                        if next_roles[role.0] {
                            next_roles[role.0] = false;
                            changed = true;
                        }
                    }
                }
            }
            let mut next_fns = fns.clone();
            let variant_plain = |id: usize| self.variant_plain(id, &objects);
            for (sig, captures) in closures {
                if next_fns[sig] && !captures.iter().all(|ty| settled_plain(ty, &objects, &fn_plain, &role_plain, &variant_plain)) {
                    next_fns.insert(sig.clone(), false);
                    changed = true;
                }
            }
            if !changed {
                let variants = (0..self.variants.len()).map(|id| self.variant_plain(id, &objects)).collect();
                return SettledPlain { objects, roles, fns, variants };
            }
            roles = next_roles;
            fns = next_fns;
        }
    }

    /// Objects without `destroy` whose fields are plain, given how function
    /// and role values are judged.
    fn settled_objects(&self, fn_plain: &dyn Fn(&ir::FnSig) -> bool, role_plain: &dyn Fn(usize) -> bool) -> Vec<bool> {
        let mut objects = vec![true; self.objects.len()];
        // One snapshot per pass: a field is judged by the previous pass, and
        // the table only ever turns true into false, so it settles.
        loop {
            let snapshot = objects.clone();
            let variant_plain = |id: usize| self.variant_plain(id, &snapshot);
            let mut changed = false;
            for (index, object) in self.objects.iter().enumerate() {
                let plain = object.destroy.is_none()
                    && object
                        .fields
                        .iter()
                        .all(|(_, ty, _)| settled_plain(ty, &snapshot, fn_plain, role_plain, &variant_plain));
                if objects[index] && !plain {
                    objects[index] = false;
                    changed = true;
                }
            }
            if !changed {
                return objects;
            }
        }
    }

    // ----- сборка модуля -----

    pub fn module(
        &self,
        program: &ast::Program,
        imports: Vec<ir::Import>,
        functions: Vec<ir::Function>,
    ) -> ir::Module {
        let objects = self
            .objects
            .iter()
            .map(|object| ir::Object {
                name: if self.reg.is_system_object(object.decl) {
                    object.name.clone()
                } else {
                    format!("{}{}", self.prefixes[object.module], object.name)
                },
                fields: object
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, (name, ty, default))| ir::Field {
                        name: name.clone(),
                        ty: ty.clone(),
                        default: *default,
                        once: object.once_slots.iter().any(|(_, _, field)| *field == index),
                    })
                    .collect(),
                does: object
                    .does
                    .iter()
                    .map(|(role, slots)| ir::Witness {
                        role: *role,
                        slots: slots.clone(),
                    })
                    .collect(),
                destroy: object.destroy,
                builtin: object.decl == self.reg.system.native_failure
                    || object.decl == self.reg.system.cancelled,
                span: object.span,
            })
            .collect();
        let roles = self
            .roles
            .iter()
            .map(|role| ir::Role {
                name: if role.builtin {
                    role.name.clone()
                } else {
                    format!("{}{}", self.prefixes[role.module], role.name)
                },
                methods: role.methods.clone(),
                builtin: role.builtin,
                span: if role.builtin {
                    program.span
                } else {
                    role.span
                },
            })
            .collect();
        let enums = self
            .reg
            .enums
            .iter()
            .filter(|decl| !decl.variant)
            .map(|decl| ir::Enum {
                name: format!("{}{}", self.prefixes[decl.module], decl.name),
                cases: decl.cases.clone(),
                span: decl.span,
            })
            .collect();
        let caps = self
            .caps
            .iter()
            .map(|(object, cap, fields)| {
                let instance = &self.objects[object.0];
                let info = &self.reg.objects[instance.decl.0].caps[*cap];
                ir::Cap {
                    name: format!(
                        "{}{}::{}",
                        self.prefixes[instance.module], instance.name, info.name
                    ),
                    owner: *object,
                    fields: fields.clone(),
                    span: info.span,
                }
            })
            .collect();
        let handles = self
            .reg
            .handles
            .iter()
            .map(|decl| ir::Handle {
                name: format!("{}{}", self.prefixes[decl.module], decl.name),
                span: decl.span,
            })
            .collect();
        ir::Module {
            name: module_path(program),
            version: program.module.version_text(),
            imports,
            enums,
            roles,
            objects,
            variants: self.variants.clone(),
            handles,
            caps,
            globals: self.globals.clone(),
            tests: self.tests.clone(),
            functions,
            entry: self.entry,
            span: program.span,
            debug: ir::DebugInfo::default(),
        }
    }
}

fn unsupported_type(span: Span, what: &str) -> LoweringError {
    LoweringError::unsupported(
        span,
        format!("IR lowering of {what} is not implemented yet."),
    )
}

fn module_path(program: &ast::Program) -> String {
    program
        .module
        .path
        .iter()
        .map(|part| part.text.as_str())
        .collect::<Vec<_>>()
        .join("/")
}

/// Plain-ность типа анализа на уровне объявлений: параметры типа plain
/// (их подстановку учитывают аргументы), коробки и функции — нет.
/// Plainness of a whole lowered program (#520): see
/// `Ctx::settled_plain_types`.
pub struct SettledPlain {
    objects: Vec<bool>,
    roles: Vec<bool>,
    fns: HashMap<ir::FnSig, bool>,
    variants: Vec<bool>,
}

impl SettledPlain {
    pub fn is_plain(&self, ty: &Type) -> bool {
        settled_plain(
            ty,
            &self.objects,
            &|sig| self.fns.get(sig).copied().unwrap_or(true),
            &|role| self.roles.get(role).copied().unwrap_or(true),
            &|id| self.variants.get(id).copied().unwrap_or(true),
        )
    }
}

/// `Ctx::is_plain` with given tables for objects and given judgements of
/// function and role values.
fn settled_plain(
    ty: &Type,
    objects: &[bool],
    fn_plain: &dyn Fn(&ir::FnSig) -> bool,
    role_plain: &dyn Fn(usize) -> bool,
    variant_plain: &dyn Fn(usize) -> bool,
) -> bool {
    match ty {
        Type::Object(object) | Type::Successor(object) => objects.get(object.0).copied().unwrap_or(true),
        Type::Role(role) => role_plain(role.0),
        Type::Variant(id) => variant_plain(id.0),
        Type::Array(element) | Type::Builder(element) => {
            settled_plain(element, objects, fn_plain, role_plain, variant_plain)
        }
        Type::Map(_, value) | Type::MapBuilder(_, value) => {
            settled_plain(value, objects, fn_plain, role_plain, variant_plain)
        }
        Type::Fn(sig) => fn_plain(sig),
        Type::Any | Type::Cap(_) | Type::Future(_) => false,
        _ => true,
    }
}

fn decl_plain_ty(ty: &Ty, objects: &[bool], roles: &[bool]) -> bool {
    match ty {
        Ty::Object(object, args) => {
            objects[object.0] && args.iter().all(|arg| decl_plain_ty(arg, objects, roles))
        }
        Ty::Role(role, args) => {
            roles[role.0] && args.iter().all(|arg| decl_plain_ty(arg, objects, roles))
        }
        Ty::Array(element) | Ty::Successor(element) => decl_plain_ty(element, objects, roles),
        Ty::Map(_, value) => decl_plain_ty(value, objects, roles),
        Ty::Any | Ty::Action(_) | Ty::Cap(..) | Ty::Variant(..) | Ty::Future(_) => false,
        _ => true,
    }
}

/// Объявления объектов без `destroy`, чьи поля plain (при данных ролях).
fn decl_plain_objects(reg: &Registry<'_>, roles: &[bool]) -> Vec<bool> {
    let mut objects = vec![true; reg.objects.len()];
    loop {
        let mut changed = false;
        for (index, info) in reg.objects.iter().enumerate() {
            let plain = info.destroy.is_none()
                && info
                    .fields
                    .iter()
                    .all(|field| decl_plain_ty(&field.ty, &objects, roles));
            if objects[index] && !plain {
                objects[index] = false;
                changed = true;
            }
        }
        if !changed {
            return objects;
        }
    }
}

/// Неподвижная точка plain-ности объявлений ролей: роль не plain, если её
/// делает объявление объекта, которое не plain.
fn decl_plain_roles(reg: &Registry<'_>) -> Vec<bool> {
    let mut roles = vec![true; reg.roles.len()];
    loop {
        let objects = decl_plain_objects(reg, &roles);
        let mut changed = false;
        for (index, info) in reg.objects.iter().enumerate() {
            if objects[index] {
                continue;
            }
            for (role, _) in &info.does {
                if roles[role.0] {
                    roles[role.0] = false;
                    changed = true;
                }
            }
        }
        if !changed {
            return roles;
        }
    }
}
