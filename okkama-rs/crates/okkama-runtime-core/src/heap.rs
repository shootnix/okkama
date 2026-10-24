//! Составные значения, общие для исполнителей: объекты, массивы, словари и
//! линейные сборщики. Значения — счётчики ссылок (`Rc`): недостижимое
//! освобождается сразу, как сборщиком мусора JVM. Значения неизменяемы, и
//! цикл ссылок замыкает только изменяемая часть — поле, связанное после ухода
//! `self` (`obj.init`), состояние intrinsic, исход Future; такие циклы
//! освобождает сборщик циклов [`crate::gc`]. Освобождение глубоких цепочек
//! итеративно — стек Rust не растёт с глубиной данных.
//!
//! `ArrayHeap` и `ObjectHeap` не хранят состояния: это операции над значениями.
use crate::{IntValue, Value, ValueError};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Объект: номер объявления и поля. Поле, чей `self` ушёл значением до конца
/// конструктора, связывается позже ровно один раз (`obj.init`, §11.3).
/// `serial` — порядок создания, `owner` — регион-владелец (OIR-1 §7).
#[derive(Debug)]
pub struct Object {
    pub declaration: usize,
    /// Поля конструктора; несвязанное — `Undef` до `obj.init`.
    fields: Vec<Value>,
    pub serial: u64,
    owner: Cell<u64>,
    /// Изменяемая часть; у большинства объектов её нет.
    extra: RefCell<Option<Box<Extra>>>,
}

/// Изменяемая часть объекта: только она может замкнуть цикл ссылок, поэтому
/// сборщик циклов очищает её у недостижимого объекта.
#[derive(Debug, Default)]
pub(crate) struct Extra {
    /// Поля, связанные `obj.init`: (номер поля, значение).
    pub(crate) late: Vec<(usize, Value)>,
    /// Состояние рантайма объекта std с intrinsic-действиями (канал, Event, …).
    pub(crate) intrinsic: Option<Rc<dyn Any>>,
}

/// Объявление системного `Cancelled`: его объекты создаёт только рантайм,
/// исполнитель сопоставляет их объявлению `Cancelled` модуля.
pub const CANCELLED: usize = usize::MAX;

/// Владелец объекта: свободен, регион с номером или уже уничтожен.
const FREE: u64 = 0;
const DEAD: u64 = u64::MAX;

thread_local! {
    static CREATED: Cell<u64> = const { Cell::new(0) };
    static REGIONS: Cell<u64> = const { Cell::new(0) };
}

impl Object {
    pub fn new(declaration: usize, fields: Vec<Value>) -> Self {
        let serial = CREATED.with(|created| {
            created.set(created.get() + 1);
            created.get()
        });
        Self {
            declaration,
            fields,
            serial,
            owner: Cell::new(FREE),
            extra: RefCell::new(None),
        }
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Значение поля; несвязанное поле — `Undef`.
    pub fn field(&self, index: usize) -> Option<Value> {
        match self.fields.get(index)? {
            Value::Undef => Some(self.late(index).unwrap_or(Value::Undef)),
            value => Some(value.clone()),
        }
    }

    fn late(&self, index: usize) -> Option<Value> {
        let extra = self.extra.borrow();
        let (_, value) = extra.as_ref()?.late.iter().find(|(field, _)| *field == index)?;
        Some(value.clone())
    }

    /// Связанные поля по порядку (несвязанные пропускаются).
    pub fn values(&self) -> Vec<Value> {
        (0..self.fields.len())
            .filter_map(|index| self.field(index))
            .filter(|value| !matches!(value, Value::Undef))
            .collect()
    }

    /// `obj.init`: связать поле после ухода `self`.
    pub fn init(&self, index: usize, value: Value) -> Result<(), ValueError> {
        if !matches!(
            self.fields.get(index).ok_or_else(|| invalid("object field index is out of bounds"))?,
            Value::Undef
        ) || self.late(index).is_some()
        {
            return Err(invalid("object field is already bound"));
        }
        if !matches!(value, Value::Undef) {
            self.extra.borrow_mut().get_or_insert_default().late.push((index, value));
        }
        Ok(())
    }

    /// Состояние intrinsic объекта.
    pub fn intrinsic(&self) -> Option<Rc<dyn Any>> {
        self.extra.borrow().as_ref()?.intrinsic.clone()
    }

    /// Связать состояние intrinsic (объект становится кандидатом сборщика циклов).
    pub fn set_intrinsic(self: &Rc<Self>, state: Rc<dyn Any>) {
        self.extra.borrow_mut().get_or_insert_default().intrinsic = Some(state);
        crate::gc::track_object(self);
    }

    /// Изменяемая часть для обхода сборщиком циклов.
    pub(crate) fn extra(&self) -> &RefCell<Option<Box<Extra>>> {
        &self.extra
    }

    /// Поля конструктора (несвязанные — `Undef`).
    pub(crate) fn fixed(&self) -> &[Value] {
        &self.fields
    }

    /// Все значения полей, включая поздние, — для итеративного освобождения.
    fn drain(&mut self, pending: &mut Vec<Value>) {
        pending.extend(
            std::mem::take(&mut self.fields)
                .into_iter()
                .filter(|value| !matches!(value, Value::Undef)),
        );
        if let Some(extra) = self.extra.get_mut().take() {
            pending.extend(extra.late.into_iter().map(|(_, value)| value));
        }
    }
}

/// Неизменяемый массив.
#[derive(Debug, Default)]
pub struct Array(pub Vec<Value>);

/// Неизменяемый словарь в естественном порядке ключей (OIR-1 §12.3, R06).
#[derive(Debug, Default)]
pub struct Map(pub BTreeMap<MapKey, Value>);

/// Значение-функция: номер функции исполнителя и снимок захватов; вызов —
/// функция над захватами и затем аргументами.
#[derive(Debug)]
pub struct Closure {
    pub function: usize,
    pub captures: Vec<Value>,
    /// `worker.bind`: вход воркера и поля receiver (`Worker.Bound` JVM).
    pub worker: Option<Box<Bound>>,
}

/// Связанное действие воркера: функция-вход и значения полей receiver.
#[derive(Debug)]
pub struct Bound {
    pub entry: Value,
    pub fields: Vec<Value>,
}

impl Drop for Closure {
    fn drop(&mut self) {
        release_all(std::mem::take(&mut self.captures));
    }
}

/// Линейный сборщик массива: верификатор гарантирует одно использование
/// каждой версии, поэтому push изменяет тот же буфер, как `ArrayList` JVM.
pub type ArrayBuilder = RefCell<Vec<Value>>;
pub type MapBuilder = RefCell<BTreeMap<MapKey, Value>>;

/// Указатель на значение кучи с равенством по идентичности.
#[derive(Debug)]
pub struct Ref<T>(pub Rc<T>);

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> PartialEq for Ref<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<T> Eq for Ref<T> {}

impl<T> std::ops::Deref for Ref<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

/// Отдаёт дочерние значения последней ссылки в `pending` вместо рекурсии.
fn release(value: Value, pending: &mut Vec<Value>) {
    match value {
        Value::Object(object) => {
            if let Some(mut object) = Rc::into_inner(object) {
                object.drain(pending);
            }
        }
        Value::Array(array) => {
            if let Some(mut array) = Rc::into_inner(array) {
                pending.append(&mut array.0);
            }
        }
        Value::Map(map) => {
            if let Some(mut map) = Rc::into_inner(map) {
                pending.extend(std::mem::take(&mut map.0).into_values());
            }
        }
        Value::Builder(builder) => {
            if let Some(builder) = Rc::into_inner(builder) {
                pending.append(&mut builder.into_inner());
            }
        }
        Value::MapBuilder(builder) => {
            if let Some(builder) = Rc::into_inner(builder) {
                pending.extend(builder.into_inner().into_values());
            }
        }
        Value::Fn(closure) => {
            if let Some(mut closure) = Rc::into_inner(closure) {
                pending.append(&mut closure.captures);
            }
        }
        Value::Any(value) => {
            if let Some(value) = Rc::into_inner(value) {
                pending.push(value);
            }
        }
        _ => {}
    }
}

/// Освободить значения итеративно (сборщик циклов отдаёт разорванные рёбра).
pub(crate) fn release_values(pending: Vec<Value>) {
    release_all(pending);
}

fn release_all(mut pending: Vec<Value>) {
    while let Some(value) = pending.pop() {
        release(value, &mut pending);
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.drain(&mut pending);
        release_all(pending);
    }
}

impl Drop for Array {
    fn drop(&mut self) {
        release_all(std::mem::take(&mut self.0));
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        release_all(std::mem::take(&mut self.0).into_values().collect());
    }
}

/// Ключ словаря: `int`, `byte`, `char`, `bool`, `float`, enum или `str`.
/// У одного словаря все ключи одного статического типа.
#[derive(Clone, Debug)]
pub enum MapKey {
    Int(IntValue),
    Float(f64),
    Bool(bool),
    Byte(u8),
    Char(char),
    Str(Rc<str>),
    Enum { declaration: usize, case: usize },
}

impl MapKey {
    pub fn from_value(value: &Value) -> Result<Self, ValueError> {
        Ok(match value {
            Value::Int(value) => Self::Int(value.clone()),
            Value::Float(value) => Self::Float(*value),
            Value::Bool(value) => Self::Bool(*value),
            Value::Byte(value) => Self::Byte(*value),
            Value::Char(value) => Self::Char(*value),
            Value::Str(value) => Self::Str(value.clone()),
            Value::Enum { declaration, case } => Self::Enum {
                declaration: *declaration,
                case: *case,
            },
            _ => return Err(invalid("map key must be a scalar, enum or string")),
        })
    }

    pub fn to_value(&self) -> Value {
        match self {
            Self::Int(value) => Value::Int(value.clone()),
            Self::Float(value) => Value::Float(*value),
            Self::Bool(value) => Value::Bool(*value),
            Self::Byte(value) => Value::Byte(*value),
            Self::Char(value) => Value::Char(*value),
            Self::Str(value) => Value::Str(value.clone()),
            Self::Enum { declaration, case } => Value::Enum {
                declaration: *declaration,
                case: *case,
            },
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Int(_) => 0,
            Self::Float(_) => 1,
            Self::Bool(_) => 2,
            Self::Byte(_) => 3,
            Self::Char(_) => 4,
            Self::Str(_) => 5,
            Self::Enum { .. } => 6,
        }
    }
}

impl Ord for MapKey {
    /// `KeyOrder` JVM: числа по значению (`-0.0 == 0.0`), `false < true`,
    /// строки по code points (для UTF-8 — побайтово), enum — порядок case.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => crate::integer::compare(a, b),
            (Self::Float(a), Self::Float(b)) => crate::float::compare(*a, *b),
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            (Self::Byte(a), Self::Byte(b)) => a.cmp(b),
            (Self::Char(a), Self::Char(b)) => a.cmp(b),
            (Self::Str(a), Self::Str(b)) => a.cmp(b),
            (
                Self::Enum {
                    declaration: a,
                    case: x,
                },
                Self::Enum {
                    declaration: b,
                    case: y,
                },
            ) => (a, x).cmp(&(b, y)),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for MapKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for MapKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for MapKey {}

#[derive(Debug, Default)]
pub struct ArrayHeap;

impl ArrayHeap {
    pub fn builder_new(&self) -> Value {
        Value::Builder(Rc::new(RefCell::new(Vec::new())))
    }

    fn builder<'a>(&self, builder: &'a Value) -> Result<&'a Rc<ArrayBuilder>, ValueError> {
        match builder {
            Value::Builder(builder) => Ok(builder),
            _ => Err(invalid("expected live array builder")),
        }
    }

    pub fn builder_push(&self, builder: &Value, value: Value) -> Result<Value, ValueError> {
        self.builder(builder)?.borrow_mut().push(value);
        Ok(builder.clone())
    }

    pub fn builder_append(&self, builder: &Value, array: &Value) -> Result<Value, ValueError> {
        let values = self.elements(array)?.to_vec();
        self.builder(builder)?.borrow_mut().extend(values);
        Ok(builder.clone())
    }

    pub fn builder_finish(&self, builder: &Value) -> Result<Value, ValueError> {
        let values = std::mem::take(&mut *self.builder(builder)?.borrow_mut());
        Ok(self.allocate(values))
    }

    pub fn builder_extend(
        &self,
        builder: &Value,
        values: impl IntoIterator<Item = Value>,
    ) -> Result<Value, ValueError> {
        self.builder(builder)?.borrow_mut().extend(values);
        Ok(builder.clone())
    }

    pub fn utf8(&self, text: &str) -> Value {
        self.allocate(text.bytes().map(Value::Byte).collect())
    }

    pub fn split(&self, text: &str, separator: &str) -> Value {
        self.allocate(
            crate::strings::split(text, separator)
                .into_iter()
                .map(|s| Value::Str(Rc::from(s)))
                .collect(),
        )
    }

    pub fn join(&self, array: &Value, separator: &str) -> Result<String, ValueError> {
        let mut result = String::new();
        for (index, value) in self.elements(array)?.iter().enumerate() {
            if index != 0 {
                result.push_str(separator);
            }
            result.push_str(value.as_string()?);
        }
        Ok(result)
    }

    pub fn allocate(&self, elements: Vec<Value>) -> Value {
        Value::Array(Rc::new(Array(elements)))
    }

    pub fn elements<'a>(&self, array: &'a Value) -> Result<&'a [Value], ValueError> {
        match array {
            Value::Array(array) => Ok(&array.0),
            _ => Err(invalid("expected array value")),
        }
    }

    pub fn lookup<'a>(
        &self,
        array: &'a Value,
        index: &IntValue,
    ) -> Result<Option<&'a Value>, ValueError> {
        let elements = self.elements(array)?;
        Ok(index
            .to_i64()
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| elements.get(i)))
    }
}

/// Операции словаря; ошибки отсутствующего ключа — у исполнителя.
pub mod maps {
    use super::*;

    pub fn new_builder() -> Value {
        Value::MapBuilder(Rc::new(RefCell::new(BTreeMap::new())))
    }

    pub fn entries(map: &Value) -> Result<&BTreeMap<MapKey, Value>, ValueError> {
        match map {
            Value::Map(map) => Ok(&map.0),
            _ => Err(invalid("expected map value")),
        }
    }

    /// Литерал: повторный ключ заменяет значение, но не ключ (`TreeMap.put`).
    pub fn from_entries(entries: impl IntoIterator<Item = (Value, Value)>) -> Result<Value, ValueError> {
        let mut map = BTreeMap::new();
        for (key, value) in entries {
            put(&mut map, &key, value)?;
        }
        Ok(Value::Map(Rc::new(Map(map))))
    }

    fn put(map: &mut BTreeMap<MapKey, Value>, key: &Value, value: Value) -> Result<(), ValueError> {
        let key = MapKey::from_value(key)?;
        match map.get_mut(&key) {
            Some(slot) => *slot = value,
            None => {
                map.insert(key, value);
            }
        }
        Ok(())
    }

    pub fn builder_put(builder: &Value, key: &Value, value: Value) -> Result<Value, ValueError> {
        match builder {
            Value::MapBuilder(map) => {
                put(&mut map.borrow_mut(), key, value)?;
                Ok(builder.clone())
            }
            _ => Err(invalid("expected live map builder")),
        }
    }

    pub fn builder_finish(builder: &Value) -> Result<Value, ValueError> {
        match builder {
            Value::MapBuilder(map) => Ok(Value::Map(Rc::new(Map(std::mem::take(
                &mut *map.borrow_mut(),
            ))))),
            _ => Err(invalid("expected live map builder")),
        }
    }

    pub fn get<'a>(map: &'a Value, key: &Value) -> Result<Option<&'a Value>, ValueError> {
        Ok(entries(map)?.get(&MapKey::from_value(key)?))
    }

    pub fn keys(map: &Value) -> Result<Value, ValueError> {
        Ok(ArrayHeap.allocate(entries(map)?.keys().map(MapKey::to_value).collect()))
    }

    pub fn values(map: &Value) -> Result<Value, ValueError> {
        Ok(ArrayHeap.allocate(entries(map)?.values().cloned().collect()))
    }
}

#[derive(Debug, Default)]
pub struct ObjectHeap;

fn invalid(message: &str) -> ValueError {
    ValueError {
        message: message.into(),
    }
}

impl ObjectHeap {
    pub fn declaration(&self, value: &Value) -> Result<usize, ValueError> {
        Ok(self.object(value)?.declaration)
    }

    pub fn allocate(&self, declaration: usize, fields: Vec<Value>) -> Value {
        Value::Object(Rc::new(Object::new(declaration, fields)))
    }

    fn object<'a>(&self, value: &'a Value) -> Result<&'a Object, ValueError> {
        match value {
            Value::Object(object) => Ok(object),
            _ => Err(invalid("expected object value")),
        }
    }

    pub fn field(&self, value: &Value, field: usize) -> Result<Value, ValueError> {
        self.object(value)?
            .field(field)
            .ok_or_else(|| invalid("object field index is out of bounds"))
    }

    pub fn init(&self, value: &Value, field: usize, bound: Value) -> Result<(), ValueError> {
        let object = match value {
            Value::Object(object) => object,
            _ => return Err(invalid("expected object value")),
        };
        let cyclic = crate::gc::traced(&bound);
        object.init(field, bound)?;
        // Поздняя ссылка может замкнуть цикл через уже ушедший `self`.
        if cyclic {
            crate::gc::track_object(object);
        }
        Ok(())
    }

    pub fn successor(
        &self,
        value: &Value,
        updates: Vec<(usize, Value)>,
    ) -> Result<Value, ValueError> {
        let original = self.object(value)?;
        let declaration = original.declaration;
        let mut fields: Vec<Value> = (0..original.field_count())
            .map(|index| original.field(index).expect("field index in bounds"))
            .collect();
        for (field, value) in updates {
            *fields
                .get_mut(field)
                .ok_or_else(|| invalid("successor field index is out of bounds"))? = value;
        }
        Ok(self.allocate(declaration, fields))
    }

    /// Successor значения, которым владеет вызывающий (последнее использование):
    /// при единственной ссылке, без слабых ссылок сборщика, поздних полей и
    /// региона-владельца объект обновляется на месте — без выделения и копии
    /// неизменённых полей. Наблюдаемо это неотличимо от копии: старое значение
    /// больше никто не видит; новая идентичность получает новый `serial`.
    /// Поля объекта для обновления на месте, если ссылка `value` единственная
    /// (условия `successor_owned`); новая идентичность получает новый `serial`.
    pub fn unique_fields<'a>(&self, value: &'a mut Value) -> Option<&'a mut [Value]> {
        let Value::Object(object) = value else {
            return None;
        };
        let inner = Rc::get_mut(object)?;
        if inner.extra.get_mut().is_some() || inner.owner.get() != FREE {
            return None;
        }
        inner.serial = CREATED.with(|created| {
            created.set(created.get() + 1);
            created.get()
        });
        Some(&mut inner.fields)
    }

    pub fn successor_owned(&self, value: Value, updates: Vec<(usize, Value)>) -> Result<Value, ValueError> {
        let Value::Object(mut object) = value else {
            return Err(invalid("expected object value"));
        };
        if let Some(inner) = Rc::get_mut(&mut object)
            && inner.extra.get_mut().is_none()
            && inner.owner.get() == FREE
            && updates.iter().all(|(field, _)| *field < inner.fields.len())
        {
            for (field, value) in updates {
                inner.fields[field] = value;
            }
            inner.serial = CREATED.with(|created| {
                created.set(created.get() + 1);
                created.get()
            });
            return Ok(Value::Object(object));
        }
        self.successor(&Value::Object(object), updates)
    }
}

/// Регион владения лексической области (OIR-1 §7), как `okkama.rt.Region`:
/// идентичности регистрируются партиями в порядке создания; закрытие
/// уничтожает их в обратном порядке регистрации, каждую не более одного раза.
#[derive(Debug)]
pub struct Region {
    id: u64,
    entries: RefCell<Vec<Rc<Object>>>,
    known: RefCell<std::collections::HashSet<*const Object>>,
}

impl Default for Region {
    fn default() -> Self {
        Self::new()
    }
}

/// Значение без идентичностей: обходить нечего.
fn plain(value: &Value) -> bool {
    !matches!(
        value,
        Value::Object(_)
            | Value::Array(_)
            | Value::Map(_)
            | Value::Any(_)
            | Value::Fn(_)
            | Value::Future(_)
    )
}

/// Идентичности, достижимые из `value`: свободные, а при `via` ещё и
/// принадлежащие `via`. Чужие идентичности не обходятся. Без рекурсии.
fn collect(value: &Value, out: &mut Vec<Rc<Object>>, via: Option<u64>) {
    let mut seen = std::collections::HashSet::new();
    let mut pending = vec![value.clone()];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                let owner = object.owner.get();
                if !(owner == FREE || Some(owner) == via) || !seen.insert(Rc::as_ptr(&object)) {
                    continue;
                }
                pending.extend(object.values().into_iter().rev());
                out.push(object);
            }
            Value::Array(array) => pending.extend(array.0.iter().rev().cloned()),
            Value::Map(map) => pending.extend(map.0.values().rev().cloned()),
            Value::Any(value) => pending.push((*value).clone()),
            Value::Fn(closure) => pending.extend(closure.captures.iter().rev().cloned()),
            Value::Future(future) => pending.extend(future.retained().into_iter().rev()),
            _ => {}
        }
    }
}

/// Состояние закрытия региона: исполнитель вызывает `destroy` по одному
/// объекту за шаг, потому что тело `destroy` — обычный вызов программы.
#[derive(Debug)]
pub struct Closing {
    kept: std::collections::HashSet<*const Object>,
    index: usize,
}

impl Region {
    pub fn new() -> Self {
        let id = REGIONS.with(|regions| {
            regions.set(regions.get() + 1);
            regions.get()
        });
        Self {
            id,
            entries: RefCell::new(Vec::new()),
            known: RefCell::new(std::collections::HashSet::new()),
        }
    }

    fn remember(&self, object: Rc<Object>) {
        if self.known.borrow_mut().insert(Rc::as_ptr(&object)) {
            self.entries.borrow_mut().push(object);
        }
    }

    fn register(&self, value: &Value, own: bool) {
        if plain(value) {
            return;
        }
        let mut fresh = Vec::new();
        collect(value, &mut fresh, None);
        fresh.sort_by_key(|object| object.serial);
        for object in fresh {
            if own {
                object.owner.set(self.id);
            }
            self.remember(object);
        }
    }

    /// Владеть свободными идентичностями, достижимыми из `value`.
    pub fn adopt(&self, value: &Value) {
        self.register(value, true);
    }

    /// Учитывать свободные идентичности без владения (временные значения).
    pub fn track(&self, value: &Value) {
        self.register(value, false);
    }

    /// Идентичности `value`, принадлежащие этому региону, — во владение `to`.
    pub fn transfer(&self, to: &Region, value: &Value) {
        if self.entries.borrow().is_empty() || plain(value) {
            return;
        }
        let mut reachable = Vec::new();
        collect(value, &mut reachable, Some(self.id));
        reachable.sort_by_key(|object| object.serial);
        for object in reachable {
            if object.owner.get() != self.id {
                continue;
            }
            object.owner.set(to.id);
            to.remember(object);
        }
    }

    /// Начать закрытие; идентичности, достижимые из `keep`, освобождаются.
    pub fn begin_close(&self, keep: &[Value]) -> Closing {
        let mut kept = std::collections::HashSet::new();
        for value in keep {
            let mut reachable = Vec::new();
            collect(value, &mut reachable, Some(self.id));
            kept.extend(reachable.iter().map(Rc::as_ptr));
        }
        Closing {
            kept,
            index: self.entries.borrow().len(),
        }
    }

    /// Следующий объект для `destroy` (уже помечен уничтоженным) или None,
    /// когда регион закрыт.
    pub fn next_destroyed(&self, closing: &mut Closing) -> Option<Rc<Object>> {
        while closing.index > 0 {
            closing.index -= 1;
            let object = self.entries.borrow()[closing.index].clone();
            if closing.kept.contains(&Rc::as_ptr(&object)) {
                if object.owner.get() == self.id {
                    object.owner.set(FREE);
                }
                continue;
            }
            let owner = object.owner.get();
            if owner != self.id && owner != FREE {
                continue; // принят другим владельцем или уже уничтожен
            }
            object.owner.set(DEAD);
            return Some(object);
        }
        self.entries.borrow_mut().clear();
        self.known.borrow_mut().clear();
        None
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EqualityError {
    InvalidState(ValueError),
    MixedTypes,
    /// Box `any` сравнивается только после сужения.
    Any,
}
impl From<ValueError> for EqualityError {
    fn from(error: ValueError) -> Self {
        Self::InvalidState(error)
    }
}

impl ObjectHeap {
    /// Structural equality without invoking user methods or recursing on Rust's stack.
    pub fn equal(
        &self,
        _arrays: &ArrayHeap,
        left: &Value,
        right: &Value,
    ) -> Result<bool, EqualityError> {
        // Пары — копии значений: поздние поля объекта не дают ссылок.
        let mut pending = vec![(left.clone(), right.clone())];
        let mut seen = std::collections::HashSet::new();
        while let Some((left, right)) = pending.pop() {
            let equal = match (&left, &right) {
                (Value::Range(a), Value::Range(b)) => a == b,
                (Value::Int(a), Value::Int(b)) => a == b,
                // Как `Double.equals` JVM: побитово, `-0.0 != 0.0`.
                (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
                (Value::Rat(a), Value::Rat(b)) => a == b,
                (Value::Handle(a), Value::Handle(b)) => std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b)),
                (Value::Fn(a), Value::Fn(b)) => Rc::ptr_eq(a, b),
                (Value::Future(a), Value::Future(b)) => Rc::ptr_eq(a, b),
                // `Objects.equals`: та же ссылка равна без вызова `Any.equals`.
                (Value::Any(a), Value::Any(b)) if Rc::ptr_eq(a, b) => true,
                (Value::Any(_), _) | (_, Value::Any(_)) => return Err(EqualityError::Any),
                (Value::Bool(a), Value::Bool(b)) => a == b,
                (Value::Byte(a), Value::Byte(b)) => a == b,
                (Value::Char(a), Value::Char(b)) => a == b,
                (Value::Str(a), Value::Str(b)) => a == b,
                (
                    Value::Enum {
                        declaration: a,
                        case: x,
                    },
                    Value::Enum {
                        declaration: b,
                        case: y,
                    },
                ) => {
                    if a != b {
                        return Err(EqualityError::MixedTypes);
                    }
                    x == y
                }
                (Value::NativeFailure(a), Value::NativeFailure(b)) => a == b,
                (Value::NativeFailure(_), Value::Object(_))
                | (Value::Object(_), Value::NativeFailure(_)) => false,
                (Value::Object(x), Value::Object(y)) => {
                    if x.declaration != y.declaration || x.fields.len() != y.fields.len() {
                        return Ok(false);
                    }
                    if !Rc::ptr_eq(x, y)
                        && seen.insert((0, Rc::as_ptr(x) as usize, Rc::as_ptr(y) as usize))
                    {
                        for index in (0..x.fields.len()).rev() {
                            match (x.field(index), y.field(index)) {
                                (Some(a), Some(b))
                                    if !matches!(a, Value::Undef) && !matches!(b, Value::Undef) =>
                                {
                                    pending.push((a, b))
                                }
                                _ => {
                                    return Err(
                                        invalid("cannot compare unbound object fields").into()
                                    );
                                }
                            }
                        }
                    }
                    true
                }
                (Value::Array(x), Value::Array(y)) => {
                    if x.0.len() != y.0.len() {
                        return Ok(false);
                    }
                    if !Rc::ptr_eq(x, y)
                        && seen.insert((1, Rc::as_ptr(x) as usize, Rc::as_ptr(y) as usize))
                    {
                        pending.extend(x.0.iter().cloned().zip(y.0.iter().cloned()).rev());
                    }
                    true
                }
                // `AbstractMap.equals`: те же ключи (по порядку ключей) и равные значения.
                (Value::Map(x), Value::Map(y)) => {
                    if x.0.len() != y.0.len() {
                        return Ok(false);
                    }
                    if !Rc::ptr_eq(x, y)
                        && seen.insert((2, Rc::as_ptr(x) as usize, Rc::as_ptr(y) as usize))
                    {
                        for ((a, u), (b, v)) in x.0.iter().zip(&y.0) {
                            if a != b {
                                return Ok(false);
                            }
                            pending.push((u.clone(), v.clone()));
                        }
                    }
                    true
                }
                (
                    Value::Undef | Value::Failure(_) | Value::Builder(_) | Value::MapBuilder(_),
                    _,
                )
                | (
                    _,
                    Value::Undef | Value::Failure(_) | Value::Builder(_) | Value::MapBuilder(_),
                ) => {
                    return Err(invalid("cannot compare internal or undefined values").into());
                }
                _ => return Err(EqualityError::MixedTypes),
            };
            if !equal {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_collect_in_place_and_finish_into_arrays() {
        let heap = ArrayHeap;
        let mut many = heap.builder_new();
        for _ in 0..100_000 {
            many = heap.builder_push(&many, Value::Byte(7)).unwrap();
        }
        let array = heap.builder_finish(&many).unwrap();
        assert_eq!(heap.elements(&array).unwrap().len(), 100_000);
    }

    #[test]
    fn deep_chains_are_released_without_recursion() {
        let objects = ObjectHeap;
        let arrays = ArrayHeap;
        let mut chain = Value::Int(IntValue::from_i64(0));
        for _ in 0..1_000_000 {
            chain = objects.allocate(0, vec![chain]);
            chain = arrays.allocate(vec![chain]);
        }
        drop(chain);
    }

    #[test]
    fn map_keys_follow_key_order_and_literals_keep_the_first_key() {
        let map = maps::from_entries([
            (Value::Float(0.0), Value::Byte(1)),
            (Value::Float(-0.0), Value::Byte(2)),
            (Value::Float(-1.0), Value::Byte(3)),
        ])
        .unwrap();
        let keys = maps::keys(&map).unwrap();
        let keys: Vec<_> = ArrayHeap
            .elements(&keys)
            .unwrap()
            .iter()
            .map(|key| key.as_float().unwrap().to_bits())
            .collect();
        assert_eq!(keys, vec![(-1.0f64).to_bits(), 0.0f64.to_bits()]);
        assert_eq!(
            maps::get(&map, &Value::Float(-0.0)).unwrap().unwrap().as_byte().unwrap(),
            2
        );
    }
}

#[cfg(test)]
mod equality_tests {
    use super::*;
    #[test]
    fn equality_is_iterative_and_reuses_shared_subgraphs() {
        let arrays = ArrayHeap;
        let heap = ObjectHeap;
        let mut a = Value::Int(crate::IntValue::from_i64(42));
        let mut b = a.clone();
        for _ in 0..20_000 {
            a = arrays.allocate(vec![a.clone(), a]);
            b = arrays.allocate(vec![b.clone(), b]);
        }
        assert!(heap.equal(&arrays, &a, &b).unwrap());
        assert!(heap.equal(&arrays, &a, &a).unwrap());
    }
    #[test]
    fn equality_checks_nominal_types_private_state_and_invalid_values() {
        let arrays = ArrayHeap;
        let heap = ObjectHeap;
        let a = heap.allocate(0, vec![Value::Byte(1)]);
        let b = heap.allocate(0, vec![Value::Byte(1)]);
        let c = heap.allocate(1, vec![Value::Byte(1)]);
        let d = heap.allocate(0, vec![Value::Byte(2)]);
        assert!(heap.equal(&arrays, &a, &b).unwrap());
        assert!(!heap.equal(&arrays, &a, &c).unwrap());
        assert!(!heap.equal(&arrays, &a, &d).unwrap());
        assert_eq!(
            heap.equal(
                &arrays,
                &Value::Byte(1),
                &Value::Int(crate::IntValue::from_i64(1))
            ),
            Err(EqualityError::MixedTypes)
        );
        assert!(heap.equal(&arrays, &Value::Undef, &Value::Undef).is_err());
    }
}
