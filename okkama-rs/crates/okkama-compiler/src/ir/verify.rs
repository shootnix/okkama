//! Верификатор OIR-1 (§17 спецификации).
//!
//! Проверяет структуру модуля и объявлений объектов, однократное
//! определение значений, доминирование использований, типы инструкций и
//! терминаторов, аргументы переходов, достижимость блоков, авторитет
//! (`obj.*` и `private` — только функциям владельца) и обязательство
//! использования successor-результатов (§8.4). Backend получают только
//! проверенный модуль, поэтому сами повторно эти правила не проверяют.

use std::collections::HashSet;
use std::fmt;

use super::cfg::{self, Def, Dominators, UsePoint};
use super::{
    Attr, BlockId, CatchType, CompareOp, Function, FutOp, InstKind, IntBinaryOp, Module,
    Object, ObjectId, ResultType, RoleId, StrOp, Target, TermKind, Type, ValueId,
};

/// Ошибка верификации: функция (если ошибка внутри неё) и описание.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyError {
    pub function: Option<String>,
    pub message: String,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.function {
            Some(function) => write!(f, "@{function}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for VerifyError {}

type Result<T> = std::result::Result<T, VerifyError>;

pub fn verify(module: &Module) -> Result<()> {
    let module_error = |message: String| VerifyError {
        function: None,
        message,
    };
    let mut symbols = HashSet::new();
    for function in &module.functions {
        if !symbols.insert(function.symbol.as_str()) {
            return Err(module_error(format!(
                "duplicate function symbol @{}",
                function.symbol
            )));
        }
    }
    for decl in &module.enums {
        let mut cases = HashSet::new();
        if decl.cases.is_empty() || !decl.cases.iter().all(|case| cases.insert(case)) {
            return Err(module_error(format!(
                "enum {} needs distinct cases",
                decl.name
            )));
        }
    }
    if let Some(entry) = module.entry {
        let Some(function) = module.functions.get(entry.0) else {
            return Err(module_error(format!(
                "entry refers to missing function #{}",
                entry.0
            )));
        };
        if !function.params.is_empty() || function.result != ResultType::Value(Type::Int) {
            return Err(module_error(format!(
                "entry @{} must have signature () -> int",
                function.symbol
            )));
        }
    }
    for (index, object) in module.objects.iter().enumerate() {
        if let Some(destroy) = object.destroy {
            let valid = module
                .functions
                .get(destroy.0)
                .is_some_and(|function| function.attrs.contains(&Attr::Destroy(ObjectId(index))));
            if !valid {
                return Err(module_error(format!(
                    "object {}: destroy must be a `[destroy {}]` function",
                    object.name, object.name
                )));
            }
        }
        let mut names = HashSet::new();
        for (position, field) in object.fields.iter().enumerate() {
            if !names.insert(field.name.as_str()) {
                return Err(module_error(format!(
                    "object {} declares field {} twice",
                    object.name, field.name
                )));
            }
            check_type(module, &field.ty)
                .map_err(|message| module_error(format!("object {}: {message}", object.name)))?;
            if matches!(field.ty, Type::Successor(_)) {
                return Err(module_error(format!(
                    "object {}: field {} cannot have a successor type",
                    object.name, field.name
                )));
            }
            if let Some(default) = field.default {
                let valid = module.functions.get(default.0).is_some_and(|function| {
                    function.params.is_empty()
                        && function.result == ResultType::Value(field.ty.clone())
                        && function
                            .attrs
                            .contains(&Attr::Default(ObjectId(index), position))
                });
                if !valid {
                    return Err(module_error(format!(
                        "object {}: default of field {} must be a `[default {}.{}]` function \
                         without parameters returning its type",
                        object.name, field.name, object.name, field.name
                    )));
                }
            }
        }
    }
    for role in &module.roles {
        for method in &role.methods {
            for ty in &method.params {
                check_type(module, ty)
                    .map_err(|message| module_error(format!("role {}: {message}", role.name)))?;
            }
        }
        if role.is_error()
            && (role.methods.len() != 1
                || role.methods[0].name != "text"
                || !role.methods[0].params.is_empty()
                || role.methods[0].result != ResultType::Value(Type::Str))
        {
            return Err(module_error(
                "the builtin role Error must declare exactly text() -> str".to_string(),
            ));
        }
    }
    for (index, object) in module.objects.iter().enumerate() {
        let cancelled = object.builtin && object.name == "Cancelled" && object.fields.is_empty();
        if object.builtin
            && !cancelled
            && (!object.is_native_failure()
                || object.destroy.is_some()
                || object.fields.len() != Object::NATIVE_FAILURE_FIELDS.len()
                || object
                    .fields
                    .iter()
                    .zip(Object::NATIVE_FAILURE_FIELDS)
                    .any(|(field, name)| field.name != name || field.ty != Type::Str))
        {
            return Err(module_error(format!(
                "the builtin object {} must be NativeFailure with fields domain, code, operation, message: str",
                object.name
            )));
        }
        let mut roles = HashSet::new();
        for witness in &object.does {
            check_witness(module, ObjectId(index), witness.role, &witness.slots)
                .map_err(|message| module_error(format!("object {}: {message}", object.name)))?;
            if !roles.insert(witness.role) {
                return Err(module_error(format!(
                    "object {} lists a role twice",
                    object.name
                )));
            }
        }
    }
    for function in &module.functions {
        check_signature(module, function).map_err(|message| VerifyError {
            function: Some(function.symbol.clone()),
            message,
        })?;
    }
    for (index, function) in module.functions.iter().enumerate() {
        let is_entry = module.entry.map(|entry| entry.0) == Some(index);
        if function.attrs.contains(&Attr::Entry) != is_entry {
            return Err(module_error(format!(
                "@{}: the entry attribute must mark exactly the module entry",
                function.symbol
            )));
        }
        FunctionVerifier::new(module, function)
            .verify()
            .map_err(|message| VerifyError {
                function: Some(function.symbol.clone()),
                message,
            })?;
    }
    Ok(())
}

/// Ссылки типа на объекты и роли существуют.
fn check_type(module: &Module, ty: &Type) -> std::result::Result<(), String> {
    match ty {
        Type::Object(object) | Type::Successor(object) if module.object(*object).is_none() => {
            Err(format!("type refers to missing object #{}", object.0))
        }
        Type::Role(role) if module.role(*role).is_none() => {
            Err(format!("type refers to missing role #{}", role.0))
        }
        Type::Enum(id) if module.enum_decl(*id).is_none() => {
            Err(format!("type refers to missing enum #{}", id.0))
        }
        Type::Cap(id) if module.cap(*id).is_none() => {
            Err(format!("type refers to missing cap #{}", id.0))
        }
        Type::Handle(id) if module.handle(*id).is_none() => {
            Err(format!("type refers to missing native object #{}", id.0))
        }
        Type::Array(element) | Type::Builder(element) => check_type(module, element),
        Type::Map(key, value) | Type::MapBuilder(key, value) => {
            let untagged = **key == Type::Never && **value == Type::Never;
            if !untagged && !key.is_map_key() {
                return Err(format!("map key type {key} is not int, bool or str"));
            }
            check_type(module, value)
        }
        _ => Ok(()),
    }
}

/// Таблица свидетельства: каждый слот роли — action этого объекта с той же сигнатурой.
fn check_witness(
    module: &Module,
    object: ObjectId,
    role: RoleId,
    slots: &[super::FuncId],
) -> std::result::Result<(), String> {
    let declared = module
        .role(role)
        .ok_or_else(|| format!("does refers to missing role #{}", role.0))?;
    if declared.methods.len() != slots.len() {
        return Err(format!(
            "does {} lists {} actions for {} role slots",
            declared.name,
            slots.len(),
            declared.methods.len()
        ));
    }
    for (slot, (method, func)) in declared.methods.iter().zip(slots).enumerate() {
        let function = module
            .function(*func)
            .ok_or_else(|| format!("does {} refers to a missing function", declared.name))?;
        let params: Vec<&Type> = function
            .params
            .iter()
            .skip(1)
            .filter_map(|param| function.value_type(*param))
            .collect();
        let matches = function.attrs.contains(&Attr::Method(object))
            && !function.is_private()
            && params.len() == method.params.len()
            && params
                .iter()
                .zip(&method.params)
                .all(|(left, right)| *left == right)
            && function.call_result() == method.result;
        if !matches {
            return Err(format!(
                "@{} does not implement {}.{}",
                function.symbol,
                declared.name,
                declared.slot_name(slot)
            ));
        }
    }
    Ok(())
}

/// Атрибуты владельца согласованы с сигнатурой (§5.1).
fn check_signature(module: &Module, function: &Function) -> std::result::Result<(), String> {
    for ty in &function.values {
        check_type(module, ty)?;
    }
    if let ResultType::Value(ty) = &function.result {
        check_type(module, ty)?;
    }
    let owners: Vec<&Attr> = function
        .attrs
        .iter()
        .filter(|attr| {
            !matches!(
                attr,
                Attr::Entry
                    | Attr::Private
                    | Attr::Extern(_)
                    | Attr::Lambda { .. }
                    | Attr::Async
                    | Attr::Intrinsic { .. }
            )
        })
        .collect();
    if owners.len() > 1 {
        return Err("a function has at most one owner attribute".to_string());
    }
    for param in &function.params {
        if matches!(function.value_type(*param), Some(Type::Successor(_))) {
            return Err(format!("parameter {param} cannot have a successor type"));
        }
    }
    let first_param = function
        .params
        .first()
        .and_then(|param| function.value_type(*param));
    let receiver = |object: &ObjectId| -> std::result::Result<(), String> {
        if first_param == Some(&Type::Object(*object)) {
            Ok(())
        } else {
            Err("the first parameter of an object action must be its receiver".to_string())
        }
    };
    match owners.first() {
        Some(Attr::Method(object)) => {
            receiver(object)?;
            if matches!(function.result, ResultType::Value(Type::Successor(_))) {
                return Err("a method returning ^T must be marked `successor`".to_string());
            }
        }
        Some(Attr::Successor(object)) => {
            receiver(object)?;
            if function.result != ResultType::Value(Type::Successor(*object)) {
                return Err("a successor action must return ^ of its object".to_string());
            }
            if function.is_private() {
                return Err("private successor actions are not supported".to_string());
            }
        }
        Some(Attr::Destroy(object)) => {
            receiver(object)?;
            if function.params.len() != 1 || function.result != ResultType::Void {
                return Err(
                    "a destroy function takes only its receiver and returns void".to_string(),
                );
            }
        }
        Some(Attr::Ctor(object)) => {
            if function.result != ResultType::Value(Type::Object(*object)) {
                return Err("a constructor must return its object".to_string());
            }
        }
        Some(Attr::Default(object, field)) => {
            let declared = module
                .object(*object)
                .and_then(|object| object.fields.get(*field))
                .map(|field| ResultType::Value(field.ty.clone()));
            if !function.params.is_empty() || declared.as_ref() != Some(&function.result) {
                return Err(
                    "a field default takes no parameters and returns the field type".to_string(),
                );
            }
        }
        _ => {
            if matches!(function.result, ResultType::Value(Type::Successor(_))) {
                return Err("only successor actions may return ^T".to_string());
            }
        }
    }
    Ok(())
}

struct FunctionVerifier<'a> {
    module: &'a Module,
    function: &'a Function,
    defs: Vec<Def>,
    dominators: Option<Dominators>,
}

impl<'a> FunctionVerifier<'a> {
    fn new(module: &'a Module, function: &'a Function) -> Self {
        Self {
            module,
            function,
            defs: Vec::new(),
            dominators: None,
        }
    }

    fn verify(mut self) -> std::result::Result<(), String> {
        if self.function.extern_symbol().is_some() {
            return if self.function.blocks.is_empty() {
                Ok(())
            } else {
                Err("a native function has no body".to_string())
            };
        }
        if let Some((name, objects)) = self.function.intrinsic() {
            if !super::INTRINSICS.contains(&name) {
                return Err(format!("unknown intrinsic \"{name}\""));
            }
            if let Some(missing) = self
                .function
                .intrinsic_helpers()
                .iter()
                .find(|func| func.0 >= self.module.functions.len())
            {
                return Err(format!(
                    "intrinsic \"{name}\" refers to missing function #{}",
                    missing.0
                ));
            }
            if let Some(missing) = objects
                .iter()
                .find(|object| self.module.object(**object).is_none())
            {
                return Err(format!(
                    "intrinsic \"{name}\" refers to missing object #{}",
                    missing.0
                ));
            }
            return if self.function.blocks.is_empty() {
                Ok(())
            } else {
                Err("an intrinsic function has no body".to_string())
            };
        }
        if self.function.blocks.is_empty() {
            return Err("function has no blocks".to_string());
        }
        if !self.function.blocks[0].params.is_empty() {
            return Err("the entry block cannot declare block parameters".to_string());
        }
        self.defs = cfg::definitions(self.function)?;
        self.check_targets()?;
        self.dominators = Some(Dominators::compute(self.function)?);
        for (index, block) in self.function.blocks.iter().enumerate() {
            let block_id = BlockId(index);
            if let Some(unwind) = &block.unwind {
                for arg in &unwind.args {
                    self.check_use(*arg, UsePoint::Unwind(block_id))?;
                }
            }
            for (position, inst) in block.insts.iter().enumerate() {
                for operand in inst.kind.operands() {
                    self.check_use(operand, UsePoint::Inst(block_id, position))?;
                }
                self.check_inst(&inst.kind, inst.result)?;
            }
            for operand in block.term.kind.operands() {
                self.check_use(operand, UsePoint::Term(block_id))?;
            }
            for target in block.term.kind.targets() {
                for arg in &target.args {
                    self.check_use(*arg, UsePoint::Term(block_id))?;
                }
            }
            self.check_terminator(&block.term.kind)?;
        }
        self.check_builders()?;
        self.check_obligations()
    }

    /// Сборщик линеен (§17): на каждом пути значение `builder<T>` используется
    /// не больше одного раза. Ни одно его использование не достижимо из
    /// другого (или из себя же по циклу), пока путь не прошёл через его
    /// определение; рёбра одного терминатора взаимоисключающие. Реализация
    /// дописывает на месте, поэтому использование в аргументах unwind
    /// конфликтует с любым другим использованием в том же блоке.
    fn check_builders(&self) -> std::result::Result<(), String> {
        let uses = self.builder_uses()?;
        for (index, uses) in uses.iter().enumerate() {
            let value = ValueId(index);
            for at in uses {
                if self.builder_reused(value, *at, uses) {
                    return Err(format!(
                        "builder {value} is used more than once on a path; builders are linear"
                    ));
                }
            }
        }
        Ok(())
    }

    /// Точки использования каждого сборщика (по номеру значения; у прочих
    /// значений — пусто); повтор внутри одной точки — ошибка. Один проход по
    /// функции: каждая версия сборщика (`builder.push` даёт новую) иначе
    /// обходила бы функцию заново.
    fn builder_uses(&self) -> std::result::Result<Vec<Vec<UsePoint>>, String> {
        let values = &self.function.values;
        let mut uses: Vec<Vec<UsePoint>> = vec![Vec::new(); values.len()];
        if !values.iter().any(Type::is_builder) {
            return Ok(uses);
        }
        let twice = |value: ValueId| format!("builder {value} is used twice by one operation; builders are linear");
        // Сборщики среди `used` и сколько раз встречается каждый.
        let builders = |used: &[ValueId]| -> Vec<(ValueId, usize)> {
            let mut found: Vec<(ValueId, usize)> = Vec::new();
            for value in used.iter().filter(|value| values[value.0].is_builder()) {
                match found.iter_mut().find(|(seen, _)| seen == value) {
                    Some((_, count)) => *count += 1,
                    None => found.push((*value, 1)),
                }
            }
            found
        };
        for (index, block) in self.function.blocks.iter().enumerate() {
            let id = BlockId(index);
            for (position, inst) in block.insts.iter().enumerate() {
                for (value, count) in builders(&inst.kind.operands()) {
                    if count > 1 {
                        return Err(twice(value));
                    }
                    uses[value.0].push(UsePoint::Inst(id, position));
                }
            }
            let term = &block.term.kind;
            let operands = term.operands();
            let targets = term.targets();
            // Операнды терминатора плюс самый нагруженный переход: переходы
            // взаимоисключающие, поэтому берётся наибольший.
            let mut candidates: Vec<ValueId> = builders(&operands).into_iter().map(|(value, _)| value).collect();
            for target in &targets {
                for (value, _) in builders(&target.args) {
                    if !candidates.contains(&value) {
                        candidates.push(value);
                    }
                }
            }
            for value in candidates {
                let count = |used: &[ValueId]| used.iter().filter(|used| **used == value).count();
                let edges = targets.iter().map(|target| count(&target.args)).max().unwrap_or(0);
                match count(&operands) + edges {
                    0 => {}
                    1 => uses[value.0].push(UsePoint::Term(id)),
                    _ => return Err(twice(value)),
                }
            }
            if let Some(unwind) = &block.unwind {
                for (value, count) in builders(&unwind.args) {
                    if count > 1 {
                        return Err(twice(value));
                    }
                    uses[value.0].push(UsePoint::Unwind(id));
                }
            }
        }
        Ok(uses)
    }

    /// Достижимо ли другое использование (или `from` снова) из `from`.
    fn builder_reused(&self, value: ValueId, from: UsePoint, uses: &[UsePoint]) -> bool {
        let in_block = |block: BlockId| {
            uses.iter().filter(move |at| match at {
                UsePoint::Inst(other, _) | UsePoint::Term(other) | UsePoint::Unwind(other) => {
                    *other == block
                }
            })
        };
        let mut pending = match from {
            UsePoint::Inst(block, position) => {
                let later = in_block(block).any(|at| match at {
                    UsePoint::Inst(_, other) => *other > position,
                    UsePoint::Term(_) | UsePoint::Unwind(_) => true,
                });
                if later {
                    return true;
                }
                cfg::all_successors(self.function, block)
            }
            UsePoint::Term(block) => {
                let mut next = cfg::successors(self.function, block);
                if self.function.blocks[block.0].term.kind.fails() {
                    next.extend(self.function.blocks[block.0].unwind.iter().map(|t| t.block));
                }
                next
            }
            UsePoint::Unwind(block) => {
                if in_block(block).any(|at| *at != from) {
                    return true;
                }
                let target = self.function.blocks[block.0]
                    .unwind
                    .as_ref()
                    .map(|target| target.block);
                target.into_iter().collect()
            }
        };
        let def_block = self.defs[value.0].block();
        let mut seen = HashSet::new();
        while let Some(next) = pending.pop() {
            // Вход в блок определения — уже другое динамическое значение.
            if next == def_block || !seen.insert(next) {
                continue;
            }
            if in_block(next).next().is_some() {
                return true;
            }
            pending.extend(cfg::all_successors(self.function, next));
        }
        false
    }

    /// Типы ключа и значения словаря (не `map<never,never>`).
    fn entry_of(&self, map: ValueId) -> std::result::Result<(&'a Type, &'a Type), String> {
        match self.ty(map)? {
            Type::Map(key, value) if **key != Type::Never => Ok((key, value)),
            other => Err(format!(
                "{map} has type {other}, expected a non-empty map type"
            )),
        }
    }

    /// Тип элемента массива (не `never`).
    fn element_of(&self, array: ValueId) -> std::result::Result<&'a Type, String> {
        match self.ty(array)? {
            Type::Array(element) if **element != Type::Never => Ok(element),
            other => Err(format!(
                "{array} has type {other}, expected a non-empty array type"
            )),
        }
    }

    fn expect_int(&self, value: ValueId) -> std::result::Result<(), String> {
        self.expect_type(value, &Type::Int)
    }

    /// Каждое значение `^C` на каждом нормальном пути до `ret` погашено
    /// `succ.take`, возвращено или передано параметру блока (тот проверяется сам).
    fn check_obligations(&self) -> std::result::Result<(), String> {
        for (index, ty) in self.function.values.iter().enumerate() {
            if !matches!(ty, Type::Successor(_)) {
                continue;
            }
            let value = ValueId(index);
            let (start_block, start) = match self.defs[index] {
                Def::BlockStart(block) => (block, 0),
                Def::Inst(block, position) => (block, position + 1),
            };
            let mut visited = HashSet::new();
            let mut work = vec![(start_block, start)];
            while let Some((block, start)) = work.pop() {
                let data = &self.function.blocks[block.0];
                if data.insts[start.min(data.insts.len())..]
                    .iter()
                    .any(|inst| inst.kind == InstKind::SuccTake(value))
                {
                    continue;
                }
                match &data.term.kind {
                    TermKind::Ret(returned) => {
                        if *returned != Some(value) {
                            return Err(format!(
                                "successor value {value} reaches `ret` in ^b{} without succ.take",
                                block.0
                            ));
                        }
                    }
                    TermKind::Panic(_)
                    | TermKind::Trap { .. }
                    | TermKind::Unreachable
                    | TermKind::Throw(_)
                    | TermKind::Raise(_) => {}
                    TermKind::Br(_)
                    | TermKind::Cond { .. }
                    | TermKind::Catch { .. }
                    | TermKind::ArrayLookup { .. }
                    | TermKind::MapLookup { .. } => {
                        for target in data.term.kind.targets() {
                            if target.args.contains(&value) {
                                continue;
                            }
                            if visited.insert(target.block) {
                                work.push((target.block, 0));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Функции владельца объекта (`method`, `ctor`, `successor`, `default`).
    fn require_owner(&self, object: ObjectId, what: &str) -> std::result::Result<(), String> {
        if self.function.owner() == Some(object) {
            Ok(())
        } else {
            Err(format!("{what} is allowed only in functions of its object"))
        }
    }

    fn object_of(&self, value: ValueId) -> std::result::Result<ObjectId, String> {
        match self.ty(value)? {
            Type::Object(object) => Ok(*object),
            other => Err(format!("{value} has type {other}, expected an object")),
        }
    }

    fn field_type(&self, object: ObjectId, field: usize) -> std::result::Result<&'a Type, String> {
        self.module
            .object(object)
            .and_then(|declared| declared.fields.get(field))
            .map(|field| &field.ty)
            .ok_or_else(|| format!("object#{} has no field #{field}", object.0))
    }

    fn ty(&self, value: ValueId) -> std::result::Result<&'a Type, String> {
        self.function
            .value_type(value)
            .ok_or_else(|| format!("{value} is not a value of this function"))
    }

    fn check_targets(&self) -> std::result::Result<(), String> {
        for block in &self.function.blocks {
            match &block.term.kind {
                // Ветвь handler получает пойманное значение последним параметром.
                TermKind::Catch {
                    ty, handler, next, ..
                } => {
                    self.check_target_with(handler, Some(ty.ty()))?;
                    self.check_target(next)?;
                }
                // Ветвь found получает элемент последним параметром.
                TermKind::ArrayLookup {
                    array,
                    found,
                    missing,
                    ..
                } => {
                    let element = self.element_of(*array)?.clone();
                    self.check_target_with(found, Some(element))?;
                    self.check_target(missing)?;
                }
                TermKind::MapLookup {
                    map,
                    found,
                    missing,
                    ..
                } => {
                    let (_, value) = self.entry_of(*map)?;
                    self.check_target_with(found, Some(value.clone()))?;
                    self.check_target(missing)?;
                }
                kind => {
                    for target in kind.targets() {
                        self.check_target(target)?;
                    }
                }
            }
            if let Some(unwind) = &block.unwind {
                self.check_target_with(unwind, Some(Type::Failure))?;
            }
        }
        Ok(())
    }

    fn check_target(&self, target: &Target) -> std::result::Result<(), String> {
        self.check_target_with(target, None)
    }

    /// Аргументы перехода и, если задано, тип последнего параметра,
    /// который заполняет сам переход (пойманная ошибка, `failure`).
    fn check_target_with(
        &self,
        target: &Target,
        implicit: Option<Type>,
    ) -> std::result::Result<(), String> {
        let Some(block) = self.function.blocks.get(target.block.0) else {
            return Err(format!("branch to missing block ^b{}", target.block.0));
        };
        if target.block.0 == 0 {
            return Err("the entry block cannot be a branch target".to_string());
        }
        let explicit = block
            .params
            .len()
            .checked_sub(usize::from(implicit.is_some()));
        if explicit != Some(target.args.len()) {
            return Err(format!(
                "branch to ^b{} passes {} arguments for {} parameters",
                target.block.0,
                target.args.len() + usize::from(implicit.is_some()),
                block.params.len()
            ));
        }
        if let Some(implicit) = &implicit {
            let last = *block.params.last().expect("checked above");
            if self.ty(last)? != implicit {
                return Err(format!(
                    "the last parameter {last} of ^b{} must have type {implicit}",
                    target.block.0
                ));
            }
        }
        for (arg, param) in target.args.iter().zip(&block.params) {
            let (arg_type, param_type) = (self.ty(*arg)?, self.ty(*param)?);
            // Путь отказа несёт значения только для cleanup: обязательство
            // `^C` на нём не действует, `^C` и `C` совместимы.
            let unwind_compatible = implicit.is_some()
                && matches!(
                    (arg_type, param_type),
                    (Type::Successor(a), Type::Object(b)) | (Type::Object(a), Type::Successor(b)) if a == b
                );
            if arg_type != param_type && !unwind_compatible {
                return Err(format!(
                    "branch to ^b{} passes {arg}: {arg_type} for {param}: {param_type}",
                    target.block.0
                ));
            }
        }
        Ok(())
    }

    fn check_use(&self, value: ValueId, at: UsePoint) -> std::result::Result<(), String> {
        let def = self
            .defs
            .get(value.0)
            .copied()
            .ok_or_else(|| format!("{value} is not a value of this function"))?;
        let dominators = self
            .dominators
            .as_ref()
            .expect("computed before uses are checked");
        if dominators.visible(def, at) {
            return Ok(());
        }
        Err(match at {
            UsePoint::Unwind(block) => format!(
                "{value} is passed to the unwind target of ^b{} but is not defined before the block starts",
                block.0
            ),
            UsePoint::Inst(block, _) | UsePoint::Term(block) => format!(
                "{value} is used in ^b{} where its definition does not dominate",
                block.0
            ),
        })
    }

    fn expect_type(&self, value: ValueId, expected: &Type) -> std::result::Result<(), String> {
        let actual = self.ty(value)?;
        if actual == expected {
            Ok(())
        } else {
            Err(format!("{value} has type {actual}, expected {expected}"))
        }
    }

    fn check_inst(
        &self,
        kind: &InstKind,
        result: Option<ValueId>,
    ) -> std::result::Result<(), String> {
        let result_type = match kind {
            InstKind::Const(constant) => {
                constant.ty()
            }
            InstKind::IntBinary { left, right, .. } => {
                self.expect_type(*left, &Type::Int)?;
                self.expect_type(*right, &Type::Int)?;
                Type::Int
            }
            InstKind::IntUnary { operand, .. } => {
                self.expect_type(*operand, &Type::Int)?;
                Type::Int
            }
            InstKind::IntCompare { left, right, .. } => {
                self.expect_type(*left, &Type::Int)?;
                self.expect_type(*right, &Type::Int)?;
                Type::Bool
            }
            InstKind::BoolNot(operand) => {
                self.expect_type(*operand, &Type::Bool)?;
                Type::Bool
            }
            InstKind::BoolCompare { op, left, right } => {
                equality_only(*op, "bool")?;
                self.expect_type(*left, &Type::Bool)?;
                self.expect_type(*right, &Type::Bool)?;
                Type::Bool
            }
            InstKind::StrConcat { left, right } => {
                self.expect_type(*left, &Type::Str)?;
                self.expect_type(*right, &Type::Str)?;
                Type::Str
            }
            InstKind::StrCompare { op: _, left, right } => {
                self.expect_type(*left, &Type::Str)?;
                self.expect_type(*right, &Type::Str)?;
                Type::Bool
            }
            InstKind::ObjNew { object, fields } => {
                if self
                    .module
                    .objects
                    .get(object.0)
                    .is_some_and(|decl| decl.builtin)
                {
                    return Err(
                        "values of a builtin object are created only by the runtime".to_string()
                    );
                }
                if !self.function.attrs.contains(&Attr::Ctor(*object)) {
                    return Err("obj.new is allowed only in constructors of its object".to_string());
                }
                let declared = self
                    .module
                    .object(*object)
                    .ok_or_else(|| format!("obj.new of missing object#{}", object.0))?;
                if declared.fields.len() != fields.len() {
                    return Err(format!(
                        "obj.new {} lists {} fields, the object declares {}",
                        declared.name,
                        fields.len(),
                        declared.fields.len()
                    ));
                }
                for (value, field) in fields.iter().zip(&declared.fields) {
                    self.expect_type(*value, &field.ty)?;
                }
                Type::Object(*object)
            }
            InstKind::ObjInit {
                object,
                field,
                value,
            } => {
                let owner = self.object_of(*object)?;
                if !self.function.attrs.contains(&Attr::Ctor(owner)) {
                    return Err(
                        "obj.init is allowed only in constructors of its object".to_string()
                    );
                }
                let expected = self.field_type(owner, *field)?.clone();
                self.expect_type(*value, &expected)?;
                return match result {
                    None => Ok(()),
                    Some(_) => Err("obj.init defines no value".to_string()),
                };
            }
            InstKind::ObjGet { object, field } => {
                let owner = self.object_of(*object)?;
                self.require_owner(owner, "obj.get")?;
                self.field_type(owner, *field)?.clone()
            }
            InstKind::ObjNext { object, updates } => {
                let owner = self.object_of(*object)?;
                if !self.function.attrs.contains(&Attr::Successor(owner)) {
                    return Err(
                        "obj.next is allowed only in successor actions of its object".to_string(),
                    );
                }
                let mut seen = HashSet::new();
                for (field, value) in updates {
                    if !seen.insert(*field) {
                        return Err(format!("obj.next replaces field #{field} twice"));
                    }
                    let ty = self.field_type(owner, *field)?;
                    self.expect_type(*value, ty)?;
                }
                Type::Object(owner)
            }
            InstKind::Call { func, args } => {
                let callee = self
                    .module
                    .function(*func)
                    .ok_or_else(|| format!("call of missing function #{}", func.0))?;
                if callee.attrs.contains(&Attr::Entry) {
                    return Err("the entry function cannot be called".to_string());
                }
                if callee.is_private() && callee.owner() != self.function.owner() {
                    return Err(format!("@{} is private to its object", callee.symbol));
                }
                if callee.params.len() != args.len() {
                    return Err(format!(
                        "call of @{} passes {} arguments for {} parameters",
                        callee.symbol,
                        args.len(),
                        callee.params.len()
                    ));
                }
                for (arg, param) in args.iter().zip(&callee.params) {
                    let expected = callee
                        .value_type(*param)
                        .ok_or_else(|| format!("@{} has an invalid parameter", callee.symbol))?;
                    self.expect_type(*arg, expected)?;
                }
                match &callee.call_result() {
                    ResultType::Value(ty) => ty.clone(),
                    ResultType::Void => {
                        return match result {
                            None => Ok(()),
                            Some(_) => Err(format!(
                                "call of void @{} cannot define a value",
                                callee.symbol
                            )),
                        };
                    }
                }
            }
            InstKind::SuccTake(operand) => match self.ty(*operand)? {
                Type::Successor(object) => Type::Object(*object),
                other => return Err(format!("succ.take of {operand}: {other}, expected ^T")),
            },
            InstKind::RoleWrap(operand) => {
                let Some(result) = result else {
                    return Err("`role.wrap` must define a value".to_string());
                };
                let (Type::Object(object), Type::Role(role)) =
                    (self.ty(*operand)?, self.ty(result)?)
                else {
                    return Err("role.wrap converts an object to a role reference".to_string());
                };
                if !self.module.does(*object, *role) {
                    return Err(format!("object#{} does not do role#{}", object.0, role.0));
                }
                return Ok(());
            }
            InstKind::AnyBox(operand) => {
                let ty = self.ty(*operand)?;
                if matches!(
                    ty,
                    Type::Any
                        | Type::Handle(_)
                        | Type::Fn(_)
                        | Type::Future(_)
                        | Type::Cap(_)
                        | Type::Failure
                        | Type::Region
                        | Type::Never
                        | Type::Successor(_)
                ) || ty.is_builder()
                {
                    return Err(format!("any.box cannot box {ty}"));
                }
                Type::Any
            }
            InstKind::RoleNarrow { value, .. } => {
                let Some(result) = result else {
                    return Err("`role.narrow` must define a value".to_string());
                };
                let (Type::Role(_), Type::Role(_)) = (self.ty(*value)?, self.ty(result)?) else {
                    return Err("role.narrow converts a role reference to another role".to_string());
                };
                return Ok(());
            }
            InstKind::CallRole {
                receiver,
                slot,
                args,
            } => {
                let Type::Role(role) = self.ty(*receiver)? else {
                    return Err(format!(
                        "call.role receiver {receiver} is not a role reference"
                    ));
                };
                let declared = self
                    .module
                    .role(*role)
                    .ok_or_else(|| format!("missing role#{}", role.0))?;
                let method = declared
                    .methods
                    .get(*slot)
                    .ok_or_else(|| format!("role {} has no slot #{slot}", declared.name))?;
                if method.params.len() != args.len() {
                    return Err(format!(
                        "call.role {}.{} passes {} arguments for {} parameters",
                        declared.name,
                        declared.slot_name(*slot),
                        args.len(),
                        method.params.len()
                    ));
                }
                for (arg, ty) in args.iter().zip(&method.params) {
                    self.expect_type(*arg, ty)?;
                }
                match &method.result {
                    ResultType::Value(ty) => ty.clone(),
                    ResultType::Void => {
                        return match result {
                            None => Ok(()),
                            Some(_) => {
                                Err("call of a void role action cannot define a value".to_string())
                            }
                        };
                    }
                }
            }
            InstKind::FailCaused { new, handled } => {
                self.expect_type(*new, &Type::Failure)?;
                self.expect_type(*handled, &Type::Failure)?;
                Type::Failure
            }
            InstKind::ArrayNew(values) => {
                let Some(result) = result else {
                    return Err("`array.new` must define a value".to_string());
                };
                let Type::Array(element) = self.ty(result)? else {
                    return Err(format!("{result} must have an array type"));
                };
                if **element == Type::Never && !values.is_empty() {
                    return Err("array<never> cannot have elements".to_string());
                }
                for value in values {
                    self.expect_type(*value, element)?;
                }
                return Ok(());
            }
            InstKind::ArrayLen(array) => {
                let Type::Array(_) = self.ty(*array)? else {
                    return Err(format!("array.len of non-array {array}"));
                };
                Type::Int
            }
            InstKind::ArrayGet { array, index } => {
                self.expect_int(*index)?;
                self.element_of(*array)?.clone()
            }
            InstKind::ArraySlice { array, start, end } => {
                self.expect_int(*start)?;
                self.expect_int(*end)?;
                self.ty(*array)?.clone()
            }
            InstKind::ArrayJoin { array, separator } => {
                self.expect_type(*array, &Type::Array(Box::new(Type::Str)))?;
                self.expect_type(*separator, &Type::Str)?;
                Type::Str
            }
            InstKind::Retag(value) => {
                let Some(result) = result else {
                    return Err("`retag` must define a value".to_string());
                };
                self.expect_type(*value, &Type::Array(Box::new(Type::Never)))?;
                let Type::Array(_) = self.ty(result)? else {
                    return Err(format!("{result} must have an array type"));
                };
                return Ok(());
            }
            InstKind::BuilderNew => {
                let Some(result) = result else {
                    return Err("`builder.new` must define a value".to_string());
                };
                let Type::Builder(_) = self.ty(result)? else {
                    return Err(format!("{result} must have a builder type"));
                };
                return Ok(());
            }
            InstKind::BuilderPush { builder, value } => {
                let ty = self.ty(*builder)?.clone();
                let Type::Builder(element) = &ty else {
                    return Err(format!("{builder} is not a builder"));
                };
                self.expect_type(*value, element)?;
                ty
            }
            InstKind::BuilderAppend { builder, array } => {
                let ty = self.ty(*builder)?.clone();
                let Type::Builder(element) = &ty else {
                    return Err(format!("{builder} is not a builder"));
                };
                let source = self.ty(*array)?;
                if source != &Type::Array(element.clone())
                    && source != &Type::Array(Box::new(Type::Never))
                {
                    return Err(format!("builder.append of {array}: {source} into {ty}"));
                }
                ty
            }
            InstKind::BuilderAppendRange {
                builder,
                start,
                end,
                ..
            } => {
                self.expect_type(*builder, &Type::Builder(Box::new(Type::Int)))?;
                self.expect_int(*start)?;
                self.expect_int(*end)?;
                Type::Builder(Box::new(Type::Int))
            }
            InstKind::BuilderFinish(builder) => match self.ty(*builder)? {
                Type::Builder(element) => Type::Array(element.clone()),
                other => return Err(format!("builder.finish of {builder}: {other}")),
            },
            InstKind::Str { op, args } => {
                if args.len() != op.arity() {
                    return Err(format!("{} takes {} operands", op.mnemonic(), op.arity()));
                }
                self.expect_type(args[0], &Type::Str)?;
                match op {
                    StrOp::Slice => {
                        self.expect_int(args[1])?;
                        self.expect_int(args[2])?;
                    }
                    _ => {
                        for arg in &args[1..] {
                            self.expect_type(*arg, &Type::Str)?;
                        }
                    }
                }
                match op {
                    StrOp::Len => Type::Int,
                    StrOp::Split => Type::Array(Box::new(Type::Str)),
                    StrOp::Utf8 => Type::Array(Box::new(Type::Byte)),
                    _ => Type::Str,
                }
            }
            InstKind::ValCompare { op, left, right } => {
                equality_only(*op, "val")?;
                let ty = self.ty(*left)?;
                self.expect_type(*right, ty)?;
                if matches!(
                    ty,
                    Type::Failure
                        | Type::Builder(_)
                        | Type::MapBuilder(..)
                        | Type::Successor(_)
                        | Type::Never
                        | Type::Handle(_)
                        | Type::Any
                        | Type::Cap(_)
                ) {
                    return Err(format!("val.{} cannot compare {ty}", op.suffix()));
                }
                Type::Bool
            }
            InstKind::IntAbs(value) | InstKind::IntSign(value) => {
                self.expect_int(*value)?;
                Type::Int
            }
            InstKind::ByteBinary { op, left, right } => {
                self.expect_type(*left, &Type::Byte)?;
                match op {
                    IntBinaryOp::And | IntBinaryOp::Or | IntBinaryOp::Xor => {
                        self.expect_type(*right, &Type::Byte)?
                    }
                    IntBinaryOp::Shl | IntBinaryOp::Shr | IntBinaryOp::Ushr => {
                        self.expect_int(*right)?
                    }
                    other => {
                        return Err(format!(
                            "{} is not a byte instruction",
                            other.byte_mnemonic()
                        ));
                    }
                }
                Type::Byte
            }
            InstKind::ByteNot(value) => {
                self.expect_type(*value, &Type::Byte)?;
                Type::Byte
            }
            InstKind::Float { op, args } => {
                if args.len() != op.arity() {
                    return Err(format!("{} takes {} operands", op.mnemonic(), op.arity()));
                }
                for arg in args {
                    self.expect_type(*arg, &Type::Float)?;
                }
                Type::Float
            }
            InstKind::FloatCompare { left, right, .. } => {
                self.expect_type(*left, &Type::Float)?;
                self.expect_type(*right, &Type::Float)?;
                Type::Bool
            }
            InstKind::FloatSign(value) => {
                self.expect_type(*value, &Type::Float)?;
                Type::Int
            }
            InstKind::Rat { op, args } => {
                if args.len() != op.arity() {
                    return Err(format!("{} takes {} operands", op.mnemonic(), op.arity()));
                }
                let operand = if *op == crate::ir::RatOp::New {
                    Type::Int
                } else {
                    Type::Rat
                };
                for arg in args {
                    self.expect_type(*arg, &operand)?;
                }
                Type::Rat
            }
            InstKind::RatCompare { left, right, .. } => {
                self.expect_type(*left, &Type::Rat)?;
                self.expect_type(*right, &Type::Rat)?;
                Type::Bool
            }
            InstKind::RatSign(value) => {
                self.expect_type(*value, &Type::Rat)?;
                Type::Int
            }
            InstKind::FnClosure { func, captures } => {
                let Some(result) = result else {
                    return Err("`fn.closure` must define a value".to_string());
                };
                let Type::Fn(sig) = self.ty(result)?.clone() else {
                    return Err("fn.closure defines a fn value".to_string());
                };
                let callee = self
                    .module
                    .functions
                    .get(func.0)
                    .ok_or_else(|| format!("fn.closure of missing function #{}", func.0))?;
                // Лямбда захватывает свой список; action объекта — receiver.
                let declared = callee.attrs.iter().find_map(|attr| match attr {
                    Attr::Lambda { captures, .. } => Some(*captures),
                    Attr::Method(_) => Some(1),
                    _ => None,
                });
                if declared != Some(captures.len()) || callee.params.len() < captures.len() {
                    return Err(format!(
                        "fn.closure of @{} needs its declared captures",
                        callee.symbol
                    ));
                }
                if callee.is_private() && callee.owner() != self.function.owner() {
                    return Err(format!("@{} is private to its owner", callee.symbol));
                }
                let types: Vec<Type> = callee
                    .params
                    .iter()
                    .map(|param| callee.values[param.0].clone())
                    .collect();
                for (capture, ty) in captures.iter().zip(&types) {
                    self.expect_type(*capture, ty)?;
                }
                if types[captures.len()..] != sig.params[..] || callee.call_result() != sig.result {
                    return Err(format!(
                        "fn.closure of @{} does not match its fn type",
                        callee.symbol
                    ));
                }
                return Ok(());
            }
            InstKind::CapMake(owner) => {
                let Some(result) = result else {
                    return Err("`cap.make` must define a value".to_string());
                };
                let Type::Cap(id) = self.ty(result)?.clone() else {
                    return Err("cap.make defines a cap".to_string());
                };
                let cap = self
                    .module
                    .cap(id)
                    .ok_or_else(|| format!("cap.make of missing cap #{}", id.0))?;
                self.expect_type(*owner, &Type::Object(cap.owner))?;
                return Ok(());
            }
            InstKind::CallCap { cap, field } => {
                let Type::Cap(id) = self.ty(*cap)?.clone() else {
                    return Err(format!("call.cap of {cap}: not a cap"));
                };
                let func = self
                    .module
                    .cap(id)
                    .and_then(|cap| cap.fields.get(*field))
                    .map(|(_, func)| *func)
                    .ok_or_else(|| format!("call.cap of missing field #{field}"))?;
                let callee = self
                    .module
                    .function(func)
                    .ok_or_else(|| "cap field refers to a missing function".to_string())?;
                return match (&callee.result, result) {
                    (ResultType::Value(expected), Some(result)) if self.ty(result)? == expected => {
                        Ok(())
                    }
                    _ => Err("call.cap result does not match the cap field".to_string()),
                };
            }
            InstKind::Await(future) => {
                if !self.function.is_async() {
                    return Err("await is allowed only in async functions".to_string());
                }
                let Type::Future(outcome) = self.ty(*future)?.clone() else {
                    return Err(format!("await of {future}: not a future"));
                };
                return match (&*outcome, result) {
                    (ResultType::Void, None) => Ok(()),
                    (ResultType::Value(expected), Some(result)) if self.ty(result)? == expected => {
                        Ok(())
                    }
                    _ => Err("await result does not match the future type".to_string()),
                };
            }
            InstKind::FutSpawn(future) => {
                let ty = self.ty(*future)?.clone();
                if !matches!(ty, Type::Future(_)) {
                    return Err(format!("fut.spawn of {future}: not a future"));
                }
                ty
            }
            InstKind::Fut { op, future } => {
                if !matches!(self.ty(*future)?, Type::Future(_)) {
                    return Err(format!("{} of {future}: not a future", op.mnemonic()));
                }
                match op {
                    FutOp::Cancel => {
                        return match result {
                            None => Ok(()),
                            Some(_) => Err("fut.cancel defines no value".to_string()),
                        };
                    }
                    FutOp::Done | FutOp::Cancelled => Type::Bool,
                }
            }
            InstKind::WorkerSpawn { entry, args, .. } => {
                let Type::Fn(sig) = self.ty(*entry)?.clone() else {
                    return Err(format!("worker.spawn of {entry}: not a fn value"));
                };
                if args.len() != sig.params.len() {
                    return Err(format!(
                        "worker.spawn passes {} arguments for {}",
                        args.len(),
                        sig.params.len()
                    ));
                }
                for (arg, ty) in args.iter().zip(&sig.params) {
                    self.expect_type(*arg, ty)?;
                }
                Type::Future(Box::new(sig.result.clone()))
            }
            InstKind::WorkerBind { action, entry, .. } => {
                let action_ty = self.ty(*action)?.clone();
                if !matches!(action_ty, Type::Fn(_)) {
                    return Err(format!("worker.bind of {action}: not a fn value"));
                }
                if !matches!(self.ty(*entry)?, Type::Fn(_)) {
                    return Err(format!("worker.bind entry {entry}: not a fn value"));
                }
                action_ty
            }
            InstKind::CallFn { callee, args } => {
                let Type::Fn(sig) = self.ty(*callee)?.clone() else {
                    return Err(format!("call.fn of {callee}: not a fn value"));
                };
                if args.len() != sig.params.len() {
                    return Err(format!(
                        "call.fn passes {} arguments for {}",
                        args.len(),
                        sig.params.len()
                    ));
                }
                for (arg, ty) in args.iter().zip(&sig.params) {
                    self.expect_type(*arg, ty)?;
                }
                return match (&sig.result, result) {
                    (ResultType::Void, None) => Ok(()),
                    (ResultType::Value(expected), Some(result)) if self.ty(result)? == expected => {
                        Ok(())
                    }
                    _ => Err("call.fn result does not match the fn type".to_string()),
                };
            }
            InstKind::RangeNew { start, end, .. } => {
                self.expect_int(*start)?;
                self.expect_int(*end)?;
                Type::Range
            }
            InstKind::RangePart { range, .. } => {
                self.expect_type(*range, &Type::Range)?;
                Type::Int
            }
            InstKind::RatPart { value, .. } => {
                self.expect_type(*value, &Type::Rat)?;
                Type::Str
            }
            InstKind::Undef => {
                return match result {
                    Some(result) if !self.ty(result)?.is_builder() => Ok(()),
                    _ => Err("`undef` defines a non-builder value".to_string()),
                };
            }
            InstKind::RegionEnter => {
                let Some(result) = result else {
                    return Err("`region.enter` must define a value".to_string());
                };
                self.expect_type(result, &Type::Region)?;
                return Ok(());
            }
            InstKind::RegionAdopt { region, value } | InstKind::RegionTrack { region, value } => {
                self.expect_type(*region, &Type::Region)?;
                let ty = self.ty(*value)?;
                if matches!(ty, Type::Region | Type::Failure) || ty.is_builder() {
                    return Err(format!("{kind}: {ty} has no identities to own"));
                }
                return match result {
                    None => Ok(()),
                    Some(_) => Err(format!("`{kind}` defines no value")),
                };
            }
            InstKind::RegionTransfer { from, to, value } => {
                self.expect_type(*from, &Type::Region)?;
                self.expect_type(*to, &Type::Region)?;
                let ty = self.ty(*value)?;
                if matches!(ty, Type::Region | Type::Failure) || ty.is_builder() {
                    return Err(format!("{kind}: {ty} has no identities to own"));
                }
                return match result {
                    None => Ok(()),
                    Some(_) => Err(format!("`{kind}` defines no value")),
                };
            }
            InstKind::RegionLeave { region, .. } => {
                self.expect_type(*region, &Type::Region)?;
                return match result {
                    None => Ok(()),
                    Some(_) => Err("`region.leave` defines no value".to_string()),
                };
            }
            InstKind::RegionUnwind { region, failure } => {
                self.expect_type(*region, &Type::Region)?;
                self.expect_type(*failure, &Type::Failure)?;
                Type::Failure
            }
            InstKind::FailSecondary { primary, cleanup } => {
                self.expect_type(*primary, &Type::Failure)?;
                self.expect_type(*cleanup, &Type::Failure)?;
                Type::Failure
            }
            InstKind::EnumCase { ty, case } => {
                let decl = self
                    .module
                    .enum_decl(*ty)
                    .ok_or_else(|| format!("enum.case of missing enum #{}", ty.0))?;
                if *case >= decl.cases.len() {
                    return Err(format!("enum {} has no case #{case}", decl.name));
                }
                Type::Enum(*ty)
            }
            InstKind::EnumCompare { op, left, right } => {
                equality_only(*op, "enum")?;
                let ty = self.ty(*left)?;
                if !matches!(ty, Type::Enum(_)) {
                    return Err(format!("enum.{} of non-enum {left}", op.suffix()));
                }
                self.expect_type(*right, ty)?;
                Type::Bool
            }
            InstKind::ByteFrom { value, .. } => {
                if !matches!(self.ty(*value)?, Type::Int | Type::Float | Type::Rat) {
                    return Err(format!("byte.from of {value}: expected int, float or rat"));
                }
                Type::Byte
            }
            InstKind::ByteCompare { left, right, .. } => {
                self.expect_type(*left, &Type::Byte)?;
                self.expect_type(*right, &Type::Byte)?;
                Type::Bool
            }
            InstKind::CharCompare { op: _, left, right } => {
                self.expect_type(*left, &Type::Char)?;
                self.expect_type(*right, &Type::Char)?;
                Type::Bool
            }
            InstKind::Bits {
                op,
                byte,
                value,
                count,
            } => {
                let word = if *byte { Type::Byte } else { Type::Int };
                self.expect_type(*value, &word)?;
                match (op.rotates(), count) {
                    (true, Some(count)) => self.expect_int(*count)?,
                    (false, None) => {}
                    _ => return Err(format!("{kind}: only rotations take a count")),
                }
                if op.counts() { Type::Int } else { word }
            }
            InstKind::MapNew(entries) => {
                let Some(result) = result else {
                    return Err("`map.new` must define a value".to_string());
                };
                let Type::Map(key, value) = self.ty(result)? else {
                    return Err(format!("{result} must have a map type"));
                };
                if **key == Type::Never && !entries.is_empty() {
                    return Err("map<never,never> cannot have entries".to_string());
                }
                for (k, v) in entries {
                    self.expect_type(*k, key)?;
                    self.expect_type(*v, value)?;
                }
                return Ok(());
            }
            InstKind::MapLen(map) => {
                let Type::Map(..) = self.ty(*map)? else {
                    return Err(format!("map.len of non-map {map}"));
                };
                Type::Int
            }
            InstKind::MapHas { map, key } => {
                let (key_type, _) = self.entry_of(*map)?;
                self.expect_type(*key, key_type)?;
                Type::Bool
            }
            InstKind::MapGet { map, key } => {
                let (key_type, value) = self.entry_of(*map)?;
                self.expect_type(*key, key_type)?;
                value.clone()
            }
            InstKind::MapKeys(map) | InstKind::MapValues(map) => match self.ty(*map)? {
                Type::Map(key, value) => {
                    let part = if matches!(kind, InstKind::MapKeys(_)) {
                        key
                    } else {
                        value
                    };
                    Type::Array(part.clone())
                }
                other => return Err(format!("{kind} of {map}: {other}")),
            },
            InstKind::MapRetag(map) => {
                let Some(result) = result else {
                    return Err("`map.retag` must define a value".to_string());
                };
                let never = Box::new(Type::Never);
                self.expect_type(*map, &Type::Map(never.clone(), never))?;
                let Type::Map(..) = self.ty(result)? else {
                    return Err(format!("{result} must have a map type"));
                };
                return Ok(());
            }
            InstKind::MapBuilderNew => {
                let Some(result) = result else {
                    return Err("`mbuilder.new` must define a value".to_string());
                };
                let Type::MapBuilder(..) = self.ty(result)? else {
                    return Err(format!("{result} must have a map builder type"));
                };
                return Ok(());
            }
            InstKind::MapBuilderPut {
                builder,
                key,
                value,
            } => {
                let ty = self.ty(*builder)?.clone();
                let Type::MapBuilder(key_type, value_type) = &ty else {
                    return Err(format!("{builder} is not a map builder"));
                };
                self.expect_type(*key, key_type)?;
                self.expect_type(*value, value_type)?;
                ty
            }
            InstKind::MapBuilderFinish(builder) => match self.ty(*builder)? {
                Type::MapBuilder(key, value) => Type::Map(key.clone(), value.clone()),
                other => return Err(format!("mbuilder.finish of {builder}: {other}")),
            },
            InstKind::Cvt(operand) => {
                let from = self.ty(*operand)?;
                let to = result.map(|result| self.ty(result)).transpose()?;
                return match (from, to) {
                    (
                        Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat,
                        Some(Type::Str),
                    )
                    | (Type::Byte | Type::Rat, Some(Type::Int))
                    | (Type::Int | Type::Byte | Type::Rat, Some(Type::Float))
                    | (Type::Float, Some(Type::Int))
                    | (Type::Int | Type::Byte | Type::Float, Some(Type::Rat)) => Ok(()),
                    (_, None) => Err("`cvt` must define a value".to_string()),
                    (from, Some(to)) => {
                        Err(format!("cvt from {from} to {to} is not an instruction"))
                    }
                };
            }
        };
        let Some(result) = result else {
            return Err(format!("`{kind}` must define a value"));
        };
        let actual = self.ty(result)?;
        if *actual != result_type {
            return Err(format!(
                "{result}: {actual} cannot hold the {result_type} result of `{kind}`"
            ));
        }
        Ok(())
    }

    fn check_terminator(&self, kind: &TermKind) -> std::result::Result<(), String> {
        match kind {
            TermKind::Cond { condition, .. } => self.expect_type(*condition, &Type::Bool),
            TermKind::Ret(value) => match (&self.function.result, value) {
                // Successor-действие возвращает построенный объект или ^ того же объекта.
                (ResultType::Value(Type::Successor(object)), Some(value))
                    if self.ty(*value)? == &Type::Object(*object) =>
                {
                    Ok(())
                }
                (ResultType::Value(ty), Some(value)) => self.expect_type(*value, ty),
                (ResultType::Void, None) => Ok(()),
                (ResultType::Value(ty), None) => Err(format!(
                    "`ret` without a value in a function returning {ty}"
                )),
                (ResultType::Void, Some(_)) => {
                    Err("`ret` with a value in a void function".to_string())
                }
            },
            TermKind::Panic(value) => self.expect_type(*value, &Type::Str),
            TermKind::Throw(value) => {
                let ty = self.ty(*value)?;
                if self.module.throwable(ty) {
                    Ok(())
                } else {
                    Err(format!("throw {value}: {ty} does not do Error"))
                }
            }
            TermKind::Raise(value) => self.expect_type(*value, &Type::Failure),
            TermKind::Catch { failure, ty, .. } => {
                self.expect_type(*failure, &Type::Failure)?;
                let caught = match ty {
                    CatchType::Object(object) => Type::Object(*object),
                    CatchType::Role(role) => Type::Role(*role),
                };
                if self.module.throwable(&caught) {
                    Ok(())
                } else {
                    Err(format!("catch type {caught} does not guarantee Error"))
                }
            }
            TermKind::ArrayLookup { index, .. } => self.expect_int(*index),
            TermKind::MapLookup { map, key, .. } => {
                let (key_type, _) = self.entry_of(*map)?;
                self.expect_type(*key, key_type)
            }
            TermKind::Br(_) | TermKind::Trap { .. } | TermKind::Unreachable => Ok(()),
        }
    }
}

fn equality_only(op: CompareOp, family: &str) -> std::result::Result<(), String> {
    if op.is_equality() {
        Ok(())
    } else {
        Err(format!("{family}.{} is not an instruction", op.suffix()))
    }
}
