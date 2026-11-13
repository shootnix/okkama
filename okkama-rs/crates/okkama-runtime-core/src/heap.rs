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

/// Размер тестовой структуры: Miri проходит те же пути примерно в тысячу раз
/// медленнее и получает тысячную часть узлов (#43). Размер стека он не
/// моделирует, так что большая глубина там ничего не доказывает.
#[cfg(test)]
pub(crate) const fn scaled(n: usize) -> usize {
    if cfg!(miri) { n / 1000 } else { n }
}
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
    /// Значения слотов `once let` (#491): (номер поля, `Maybe.some`). Поле
    /// объекта остаётся `none`, поэтому `==`, хэш и Dumper их не видят.
    pub(crate) once: Vec<(usize, Value)>,
    /// Состояние рантайма объекта std с intrinsic-действиями (канал, Event, …).
    pub(crate) intrinsic: Option<Rc<dyn Any>>,
}

/// Объявление системного `Cancelled`: его объекты создаёт только рантайм,
/// исполнитель сопоставляет их объявлению `Cancelled` модуля.
pub const CANCELLED: usize = usize::MAX;

/// Владелец объекта: свободен, регион с номером или уже уничтожен.
const FREE: u64 = 0;
const DEAD: u64 = u64::MAX;

#[cfg(feature = "no-regions-bench")]
fn regions_disabled() -> bool {
    thread_local! {
        static DISABLED: bool = std::env::var("OKKAMA_NO_REGIONS_BENCH").as_deref() == Ok("1");
    }
    DISABLED.with(|disabled| *disabled)
}

#[cfg(feature = "region-walk-stats")]
mod walk_stats {
    use std::cell::Cell;
    struct Stats(Cell<[u64; 9]>);
    impl Drop for Stats {
        fn drop(&mut self) {
            eprintln!("region_walk_stats {:?}", self.0.get());
        }
    }
    thread_local! { static STATS: Stats = const { Stats(Cell::new([0; 9])) }; }
    // register, transfer, keep roots, collect calls, values popped,
    // object encounters, owner rejected, duplicate rejected, accepted objects.
    pub fn add(delta: [u64; 9]) {
        STATS.with(|stats| {
            let mut totals = stats.0.get();
            for (total, increment) in totals.iter_mut().zip(delta) { *total += increment; }
            stats.0.set(totals);
        });
    }
}

thread_local! {
    static CREATED: Cell<u64> = const { Cell::new(0) };
    static REGIONS: Cell<u64> = const { Cell::new(0) };
}

pub(crate) fn next_serial() -> u64 {
    CREATED.with(|created| {
        created.set(created.get() + 1);
        created.get()
    })
}

impl Object {
    pub fn new(declaration: usize, fields: Vec<Value>) -> Self {
        let serial = next_serial();
        #[cfg(test)]
        LIVE_OBJECTS.with(|live| live.set(live.get() + 1));
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

    /// Запомненное значение слота `once let` (#491), если оно уже есть.
    pub fn once(&self, index: usize) -> Option<Value> {
        let extra = self.extra.borrow();
        let (_, value) = extra.as_ref()?.once.iter().find(|(field, _)| *field == index)?;
        Some(value.clone())
    }

    /// Запомнить значение слота `once let` (#491); поля объекта не меняются.
    pub fn once_store(&self, index: usize, value: Value) {
        let mut extra = self.extra.borrow_mut();
        let once = &mut extra.get_or_insert_default().once;
        once.retain(|(field, _)| *field != index);
        once.push((index, value));
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

    /// Поля, освобождение которых уходит в глубину, — в `pending` для
    /// итеративного освобождения; остальные освобождаются вместе с объектом.
    fn drain(&mut self, pending: &mut Vec<Value>) {
        take_deep(&mut self.fields, pending);
        if let Some(extra) = self.extra.get_mut().take() {
            pending.extend(extra.late.into_iter().map(|(_, value)| value));
            pending.extend(extra.once.into_iter().map(|(_, value)| value));
        }
    }
}

/// Неизменяемый массив (#335): плоский вектор, версия общего буфера или
/// срез плоского массива. Цепочки `a + [x]` и `[x] + a` пишут в буфер на
/// месте, у его границы; срез, который держит не меньше половины источника,
/// разделяет его хранилище. Версия видит только свои элементы, и они не
/// меняются — представления Tomato из 0.4.3 (#246, #247, #249) на `Rc`.
#[derive(Debug, Default)]
pub struct Array(Repr, Plain);

/// Держит ли массив только простые значения (числа, строки…): такой
/// массив не узел сборщика циклов — цикла через него не бывает. Известно
/// сразу у сделанного из массивов с известным ответом (сцепление, срез,
/// замена элемента); иначе выясняется при первом вопросе и запоминается.
#[derive(Debug, Default)]
struct Plain(Cell<Option<bool>>);

impl Plain {
    fn known(plain: Option<bool>) -> Plain {
        Plain(Cell::new(plain))
    }
}

#[derive(Debug)]
enum Repr {
    /// `chain`: копия, сделанная добавлением в конец или в начало; следующее
    /// добавление к ней начинает буфер.
    Flat { items: Vec<Value>, chain: Chain },
    View { buffer: Rc<Buffer>, start: usize, len: usize },
    /// Срез плоского массива `origin` (никогда — другого среза или версии).
    Slice { origin: Rc<Array>, start: usize, len: usize },
    /// Версия цепочки `replaced` (#250): замена копирует один путь, а не
    /// массив. Только для массивов простых значений: узлы общие у версий,
    /// а сборщик циклов считает каждое ребро одной сильной ссылкой.
    /// Читаемый, а не меняемый trie сплющивается: после прохода чтений его
    /// элементы — плоская копия в `flat`.
    Trie { trie: okkama_persistent::Trie<Value>, reads: Cell<usize>, flat: std::cell::OnceCell<Vec<Value>> },
}

impl Default for Repr {
    fn default() -> Self {
        Repr::Flat { items: Vec::new(), chain: Chain::None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chain {
    None,
    Appended,
    Prepended,
    /// Копия, сделанная `replaced`: следующая замена начинает trie.
    Replaced,
}

/// Массивы короче этого `replaced` копирует, как прежде (`TRIE_MIN` Tomato).
pub const TRIE_MIN: usize = 64;

/// Хранилище цепочки добавлений: `slots` до `capacity`, записаны
/// `front..` (впереди — `Undef`, место для добавлений в начало; позади —
/// невыделенная ёмкость вектора, без заполнения). Записанный слот не
/// меняется, пока буфер жив, поэтому версии, видящие его, не меняются тоже;
/// запись идёт только за границы, и только у версии, которая кончается
/// (начинается) ровно на границе.
#[derive(Debug)]
pub struct Buffer {
    slots: RefCell<Vec<Value>>,
    front: Cell<usize>,
    capacity: usize,
    /// Все записанные значения простые (см. [`Plain`]); `None` — не
    /// выяснено.
    plain: Cell<Option<bool>>,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        take_deep(self.slots.get_mut(), &mut pending);
        if !pending.is_empty() {
            release_all(pending);
        }
    }
}

impl Buffer {
    /// Записанные значения: рёбра узла для сборщика циклов.
    pub(crate) fn written(&self) -> Option<std::cell::Ref<'_, [Value]>> {
        let slots = self.slots.try_borrow().ok()?;
        let front = self.front.get();
        Some(std::cell::Ref::map(slots, |slots| &slots[front..]))
    }

    /// Отдать записанные значения мусорного буфера.
    pub(crate) fn clear(&self, sink: &mut Vec<Value>) {
        if let Ok(mut slots) = self.slots.try_borrow_mut() {
            let front = self.front.get();
            sink.extend(slots[front..].iter_mut().map(|slot| std::mem::replace(slot, Value::Undef)));
        }
    }

    /// Конец записанного: граница добавлений в конец.
    fn back(&self) -> usize {
        self.slots.borrow().len()
    }

    /// Буфер на `capacity` значений с `parts` подряд начиная со слота `front`.
    fn new(parts: &[&[Value]], capacity: usize, front: usize, plain: Option<bool>) -> Rc<Buffer> {
        let mut slots = Vec::with_capacity(capacity);
        slots.resize(front, Value::Undef);
        for part in parts {
            slots.extend_from_slice(part);
        }
        Rc::new(Buffer { slots: RefCell::new(slots), front: Cell::new(front), capacity, plain: Cell::new(plain) })
    }

    fn plain(&self) -> bool {
        if let Some(plain) = self.plain.get() {
            return plain;
        }
        let plain = self.slots.borrow()[self.front.get()..].iter().all(|value| !crate::gc::traced(value));
        self.plain.set(Some(plain));
        plain
    }

    /// Записать `items` за концом записанного или перед `front`; `false`,
    /// если места нет или буфер сейчас читают.
    fn write(self: &Rc<Self>, items: &[Value], at_back: bool) -> bool {
        let Ok(mut slots) = self.slots.try_borrow_mut() else {
            return false;
        };
        let front = self.front.get();
        if at_back {
            if slots.len() + items.len() > self.capacity {
                return false;
            }
            // В пределах `capacity` вектор не перевыделяется.
            slots.extend_from_slice(items);
        } else {
            if front < items.len() {
                return false;
            }
            slots[front - items.len()..front].clone_from_slice(items);
            self.front.set(front - items.len());
        }
        drop(slots);
        // Новое ребро старого буфера может замкнуть цикл через версию,
        // записанную в него; простые значения цикла не замкнут.
        if items.iter().any(crate::gc::traced) {
            self.plain.set(Some(false));
            crate::gc::track_buffer(self);
        }
        true
    }
}

/// Элементы массива: срез плоского вектора, заимствование буфера или
/// копия элементов trie (для чтения целиком; по индексу trie читается
/// без копии, см. [`Array::get`]).
#[derive(Debug)]
pub enum Items<'a> {
    Flat(&'a [Value]),
    Shared(std::cell::Ref<'a, [Value]>),
    Owned(Vec<Value>),
}

impl std::ops::Deref for Items<'_> {
    type Target = [Value];
    fn deref(&self) -> &[Value] {
        match self {
            Items::Flat(items) => items,
            Items::Shared(items) => items,
            Items::Owned(items) => items,
        }
    }
}

/// Массивы короче этого копируются при добавлении, как прежде: копия
/// нескольких значений дешевле буфера и версии (`CHAIN_MIN` Tomato).
pub const CHAIN_MIN: usize = 32;

/// Общие буферы и срезы, если не `OKKAMA_ARRAYS=flat`, при котором каждое
/// добавление и срез — копия (для замеров на одном бинарнике).
pub fn shared_arrays() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("OKKAMA_ARRAYS").map_or(true, |v| v != "flat"))
}

impl Array {
    pub fn new(items: Vec<Value>) -> Array {
        Array::flat(items, Chain::None, None)
    }

    fn flat(items: Vec<Value>, chain: Chain, plain: Option<bool>) -> Array {
        Array(Repr::Flat { items, chain }, Plain::known(plain))
    }

    fn shared(repr: Repr) -> Value {
        Value::Array(Rc::new(Array(repr, Plain::known(None))))
    }

    /// Массив только простых значений (см. [`Plain`]).
    pub fn plain(&self) -> bool {
        match &self.0 {
            Repr::Flat { items, .. } => {
                if let Some(plain) = self.1.0.get() {
                    return plain;
                }
                let plain = items.iter().all(|value| !crate::gc::traced(value));
                self.1.0.set(Some(plain));
                plain
            }
            Repr::View { buffer, .. } => buffer.plain(),
            Repr::Slice { origin, .. } => origin.plain(),
            Repr::Trie { .. } => true,
        }
    }

    /// Может ли массив быть частью цикла: держит ли он, сам или через
    /// буфер и источник, хоть одно непростое значение.
    pub fn holds_nodes(&self) -> bool {
        !self.plain()
    }

    /// `items.replaced(at, value)`: копия с заменённым элементом; ответ
    /// [`Plain`] — от источника и нового значения, без обхода.
    ///
    /// A chain of replacements in a large array of plain values becomes a
    /// trie (#250), which copies one path per replacement: the first copy
    /// is marked, the next replacement starts the trie.
    pub fn replaced(&self, at: usize, value: Value) -> Value {
        let plain = self.plain() && !crate::gc::traced(&value);
        let tries = shared_arrays() && plain && self.len() >= TRIE_MIN;
        if tries && let Repr::Trie { trie, .. } = &self.0 {
            let trie = trie.set(at, value).expect("index in bounds");
            return Value::Array(Rc::new(Array::trie(trie)));
        }
        let mut items = self.items().to_vec();
        items[at] = value;
        if tries && matches!(self.0, Repr::Flat { chain: Chain::Replaced, .. }) {
            return Value::Array(Rc::new(Array::trie(okkama_persistent::Trie::from_vec(items))));
        }
        let chain = if tries { Chain::Replaced } else { Chain::None };
        Value::Array(Rc::new(Array::flat(items, chain, Some(plain))))
    }

    fn trie(trie: okkama_persistent::Trie<Value>) -> Array {
        let repr = Repr::Trie { trie, reads: Cell::new(0), flat: std::cell::OnceCell::new() };
        Array(repr, Plain::known(Some(true)))
    }

    pub fn items(&self) -> Items<'_> {
        match &self.0 {
            Repr::Flat { items, .. } => Items::Flat(items),
            Repr::View { buffer, start, len } => {
                Items::Shared(std::cell::Ref::map(buffer.slots.borrow(), |slots| &slots[*start..*start + *len]))
            }
            Repr::Slice { origin, start, len } => match origin.items() {
                Items::Flat(items) => Items::Flat(&items[*start..*start + *len]),
                Items::Shared(items) => Items::Shared(std::cell::Ref::map(items, |items| &items[*start..*start + *len])),
                Items::Owned(items) => Items::Owned(items[*start..*start + *len].to_vec()),
            },
            Repr::Trie { trie, flat, .. } => match flat.get() {
                Some(items) => Items::Flat(items),
                None => Items::Owned(trie.to_vec()),
            },
        }
    }

    /// Element `index`: a trie is read by its path, not copied.
    #[inline]
    pub fn get(&self, index: usize) -> Option<Value> {
        let (trie, reads, flat) = match &self.0 {
            Repr::Flat { items, .. } => return items.get(index).cloned(),
            Repr::View { buffer, start, len } => {
                return (index < *len).then(|| buffer.slots.borrow()[start + index].clone());
            }
            Repr::Slice { origin, start, len } => return if index < *len { origin.get(start + index) } else { None },
            Repr::Trie { trie, reads, flat } => (trie, reads, flat),
        };
        if let Some(items) = flat.get() {
            return items.get(index).cloned();
        }
        let item = trie.get(index).cloned();
        // Read rather than changed: flat after a pass of reads, as many as
        // the copy costs (Tomato waits two passes; here a read by path costs
        // more against a flat read than there).
        reads.set(reads.get() + 1);
        if reads.get() >= trie.len() {
            let _ = flat.set(trie.to_vec());
        }
        item
    }

    pub fn len(&self) -> usize {
        match &self.0 {
            Repr::Flat { items, .. } => items.len(),
            Repr::View { len, .. } | Repr::Slice { len, .. } => *len,
            Repr::Trie { trie, .. } => trie.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Что ещё держит массив, кроме своих элементов: буфер версии или
    /// источник среза (рёбра узла для сборщика циклов).
    pub(crate) fn storage(&self) -> Storage<'_> {
        match &self.0 {
            Repr::Flat { items, .. } => Storage::Items(items),
            Repr::View { buffer, .. } => Storage::Buffer(buffer),
            Repr::Slice { origin, .. } => Storage::Origin(origin),
            Repr::Trie { .. } => Storage::Items(&[]),
        }
    }

    /// Сделан ли массив добавлением: копия с пометкой или версия буфера.
    fn continues_chain(&self) -> bool {
        matches!(self.0, Repr::Flat { chain: Chain::Appended | Chain::Prepended, .. } | Repr::View { .. })
    }
}

pub(crate) enum Storage<'a> {
    Items(&'a [Value]),
    Buffer(&'a Rc<Buffer>),
    Origin(&'a Rc<Array>),
}

/// `a + b + …`, когда `a` продолжает цепочку добавлений в конец (#246):
/// запись на месте за границей буфера `a`, если `a` кончается на ней и
/// место есть, иначе — в новый буфер вдвое больше. Иначе — то же для
/// цепочки добавлений в начало (#247), зеркально. `None` — копия, как
/// прежде; так и ветка от версии, которая кончается не на границе: её
/// копия помечена, и цепочка от неё начнёт свой буфер на следующем шаге —
/// новый буфер на каждую ветку стоил бы вдвое больше памяти.
pub fn concat_shared(parts: &[&Rc<Array>], size: usize) -> Option<Value> {
    if !shared_arrays() || parts.len() < 2 || size < CHAIN_MIN {
        return None;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if first.continues_chain() {
        // Добавляемые части копируются заранее: одна из них может быть
        // версией того же буфера.
        let rest: Vec<Value> = parts[1..].iter().flat_map(|part| part.items().to_vec()).collect();
        if let Repr::View { buffer, start, len } = &first.0 {
            if start + len != buffer.back() {
                return None;
            }
            if buffer.write(&rest, true) {
                return Some(Array::shared(Repr::View { buffer: buffer.clone(), start: *start, len: size }));
            }
        }
        let head = first.items().to_vec();
        let capacity = (2 * size).next_power_of_two();
        let both = matches!(first.0, Repr::Flat { chain: Chain::Prepended, .. })
            || matches!(&first.0, Repr::View { start, .. } if *start > 0);
        let front = if both { (capacity - size) / 4 } else { 0 };
        let buffer = Buffer::new(&[&head, &rest], capacity, front, Some(parts.iter().all(|part| part.plain())));
        return Some(Array::shared(Repr::View { buffer, start: front, len: size }));
    }
    if last.continues_chain() {
        let rest: Vec<Value> = parts[..parts.len() - 1].iter().flat_map(|part| part.items().to_vec()).collect();
        if let Repr::View { buffer, start, len } = &last.0 {
            if *start != buffer.front.get() {
                return None;
            }
            if buffer.write(&rest, false) {
                let start = start - rest.len();
                return Some(Array::shared(Repr::View { buffer: buffer.clone(), start, len: len + rest.len() }));
            }
        }
        let tail = last.items().to_vec();
        let capacity = (2 * size).next_power_of_two();
        // Три четверти места — впереди, где растёт цепочка.
        let front = (capacity - size) * 3 / 4;
        let buffer = Buffer::new(&[&rest, &tail], capacity, front, Some(parts.iter().all(|part| part.plain())));
        return Some(Array::shared(Repr::View { buffer, start: front, len: size }));
    }
    None
}

/// Пометка плоской копии `a + b + …` (#246, #247): большой первый
/// операнд — добавление в конец, большой последний — в начало.
pub fn concat_flat(parts: &[&Rc<Array>], items: Vec<Value>) -> Value {
    let chain = if !shared_arrays() || parts.len() < 2 {
        Chain::None
    } else if parts[0].len() >= CHAIN_MIN {
        Chain::Appended
    } else if parts[parts.len() - 1].len() >= CHAIN_MIN {
        Chain::Prepended
    } else {
        Chain::None
    };
    let plain = parts.iter().all(|part| part.plain());
    Value::Array(Rc::new(Array::flat(items, chain, Some(plain))))
}

/// Срез, который разделяет хранилище источника (#249): версию того же
/// буфера или срез плоского массива, если он держит не меньше половины
/// хранилища — так срез не держит живым больше чем вдвое против своего.
/// Короткие и малые срезы копируются, как прежде.
pub fn slice_shared(array: &Rc<Array>, range: std::ops::Range<usize>) -> Option<Value> {
    if !shared_arrays() || range.len() < CHAIN_MIN {
        return None;
    }
    let shared = match &array.0 {
        Repr::Flat { items, .. } => (2 * range.len() >= items.len())
            .then(|| Repr::Slice { origin: array.clone(), start: range.start, len: range.len() })?,
        Repr::Slice { origin, start, .. } => (2 * range.len() >= origin.len())
            .then(|| Repr::Slice { origin: origin.clone(), start: start + range.start, len: range.len() })?,
        Repr::View { buffer, start, .. } => (2 * range.len() >= buffer.capacity)
            .then(|| Repr::View { buffer: buffer.clone(), start: start + range.start, len: range.len() })?,
        Repr::Trie { .. } => return None,
    };
    Some(Value::Array(Rc::new(Array(shared, Plain::known(None)))))
}

/// Неизменяемый словарь в естественном порядке ключей (OIR-1 §12.3, R06).
///
/// Записи — B-дерево стандартной библиотеки, которое каждая версия
/// копирует, пока запись не делает версию словаря больше чем из
/// [`MAP_SMALL`] записей: тогда он на месте становится персистентным
/// B+ деревом ([`okkama_persistent::PMap`]), и версии от него делят узлы —
/// запись копирует один путь (#335, как Tomato в #252–#254). Только у
/// словаря простых ключей и значений (см. [`Plain`]): узлы общие у версий,
/// а сборщик циклов считает каждое ребро одной сильной ссылкой.
#[derive(Debug, Default)]
pub struct Map(RefCell<Entries>, Plain);

#[derive(Debug)]
enum Entries {
    Tree(BTreeMap<MapKey, Value>),
    Shared(okkama_persistent::PMap<MapKey, Value>),
}

impl Default for Entries {
    fn default() -> Self {
        Entries::Tree(BTreeMap::new())
    }
}

/// Словарь не больше этого копируется при записи, как прежде (`SMALL`
/// Tomato, столько же, сколько [`TRIE_MIN`]).
pub const MAP_SMALL: usize = 64;

/// Персистентные словари, если не `OKKAMA_MAPS=btree`, при котором каждая
/// версия — копия (для замеров на одном бинарнике; тот же ключ, что у Tomato).
pub fn persistent_maps() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("OKKAMA_MAPS").map_or(true, |v| v != "btree"))
}

impl okkama_persistent::pmap::NodeKey for MapKey {
    /// Число, байт, символ сравниваются на месте; строка, дробь, объект —
    /// вызовом, поэтому каждый ключ узла — один раз.
    #[inline]
    fn seek(keys: &[MapKey], key: &MapKey) -> (usize, bool) {
        match key {
            MapKey::Str(_) | MapKey::Rat(_) | MapKey::Object { .. } => {
                okkama_persistent::pmap::seek_three_way(keys, key)
            }
            _ => okkama_persistent::pmap::seek_scan(keys, key),
        }
    }
}

/// Записи словаря, взятые на чтение.
#[derive(Debug)]
pub struct MapEntries<'a>(std::cell::Ref<'a, Entries>);

impl MapEntries<'_> {
    pub fn iter(&self) -> MapIter<'_> {
        match &*self.0 {
            Entries::Tree(map) => MapIter::Tree(map.iter()),
            Entries::Shared(map) => MapIter::Shared(map.iter()),
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &MapKey> {
        self.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.iter().map(|(_, value)| value)
    }

    /// `f` от каждой записи по порядку: представление выбирается один раз,
    /// а не на каждой записи, как у [`MapIter`] (сбор ключей на треть дешевле).
    pub fn collect<T>(&self, mut f: impl FnMut(&MapKey, &Value) -> T) -> Vec<T> {
        match &*self.0 {
            Entries::Tree(map) => map.iter().map(|(key, value)| f(key, value)).collect(),
            Entries::Shared(map) => map.iter().map(|(key, value)| f(key, value)).collect(),
        }
    }
}

/// Записи по порядку ключей.
#[derive(Debug)]
pub enum MapIter<'a> {
    Tree(std::collections::btree_map::Iter<'a, MapKey, Value>),
    Shared(okkama_persistent::pmap::Iter<'a, MapKey, Value>),
}

impl<'a> Iterator for MapIter<'a> {
    type Item = (&'a MapKey, &'a Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            MapIter::Tree(iter) => iter.next(),
            MapIter::Shared(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            MapIter::Tree(iter) => iter.size_hint(),
            MapIter::Shared(iter) => iter.size_hint(),
        }
    }
}

impl ExactSizeIterator for MapIter<'_> {}

impl Map {
    pub fn new(entries: BTreeMap<MapKey, Value>) -> Map {
        Map(RefCell::new(Entries::Tree(entries)), Plain::known(None))
    }

    pub fn len(&self) -> usize {
        match &*self.0.borrow() {
            Entries::Tree(map) => map.len(),
            Entries::Shared(map) => map.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(&self) -> MapEntries<'_> {
        MapEntries(self.0.borrow())
    }

    /// Записи для сборщика циклов: `None`, если их сейчас читают на запись.
    pub(crate) fn try_entries(&self) -> Option<MapEntries<'_>> {
        self.0.try_borrow().ok().map(MapEntries)
    }

    pub fn get(&self, key: &MapKey) -> Option<Value> {
        match &*self.0.borrow() {
            Entries::Tree(map) => map.get(key).cloned(),
            Entries::Shared(map) => map.get(key).cloned(),
        }
    }

    /// Ключи и значения простые: такой словарь не узел сборщика циклов.
    /// Ключ-объект держит объект (#212), поэтому он не простой.
    pub fn plain(&self) -> bool {
        if let Some(plain) = self.1.0.get() {
            return plain;
        }
        let plain = self
            .entries()
            .iter()
            .all(|(key, value)| !matches!(key, MapKey::Object { .. }) && !crate::gc::traced(value));
        self.1.0.set(Some(plain));
        plain
    }

    /// Хоть один ключ или значение — узел сборщика циклов.
    pub fn holds_nodes(&self) -> bool {
        !self.plain()
    }

    /// Ответ [`Plain`] словаря из этих частей, если он уже известен у всех.
    fn known_plain(parts: &[&Map]) -> Option<bool> {
        let mut plain = Some(true);
        for part in parts {
            match part.1.0.get() {
                Some(false) => return Some(false),
                None => plain = None,
                Some(true) => {}
            }
        }
        plain
    }

    /// Большой словарь простых значений, от которого делают версию,
    /// становится персистентным на месте; его версии делят узлы.
    fn shared(&self) -> Option<okkama_persistent::PMap<MapKey, Value>> {
        if !persistent_maps() || self.len() <= MAP_SMALL || !self.plain() {
            return None;
        }
        let mut entries = self.0.borrow_mut();
        if let Entries::Tree(map) = &mut *entries {
            *entries = Entries::Shared(okkama_persistent::PMap::from_sorted(std::mem::take(map)));
        }
        match &*entries {
            Entries::Shared(map) => Some(map.clone()),
            Entries::Tree(_) => unreachable!("made persistent above"),
        }
    }

    fn persistent(map: okkama_persistent::PMap<MapKey, Value>) -> Map {
        Map(RefCell::new(Entries::Shared(map)), Plain::known(Some(true)))
    }

    /// `a + b + ...`: повтор ключа — значение правой части, ключ — левой.
    /// Версия делается от самой большой части; если она персистентна (или
    /// становится ею), остальные части вставляются в копию её пути.
    pub fn merge(parts: &[&Map]) -> Map {
        let base = (0..parts.len()).fold(0, |best, at| if parts[at].len() > parts[best].len() { at } else { best });
        if parts.get(base).is_some_and(|part| part.len() > MAP_SMALL)
            && parts.iter().enumerate().all(|(at, part)| at == base || part.plain())
            && let Some(mut map) = parts[base].shared()
        {
            // Левее основы, справа налево: значение уже записанного ключа
            // остаётся, а ключ становится левым (`BTreeMap::insert`).
            for part in parts[..base].iter().rev() {
                for (key, value) in part.entries().iter() {
                    let value = map.remove(key).map_or_else(|| value.clone(), |(_, kept)| kept);
                    map.insert(key.clone(), value);
                }
            }
            for part in &parts[base + 1..] {
                for (key, value) in part.entries().iter() {
                    map.insert(key.clone(), value.clone());
                }
            }
            return Map::persistent(map);
        }
        let mut entries = BTreeMap::new();
        for part in parts {
            for (key, value) in part.entries().iter() {
                entries.insert(key.clone(), value.clone());
            }
        }
        Map(RefCell::new(Entries::Tree(entries)), Plain::known(Map::known_plain(parts)))
    }

    /// `a - b - ...`: записи без ключей из `removed`.
    pub fn difference(&self, removed: &[&Map]) -> Map {
        if let Some(mut map) = self.shared() {
            for part in removed {
                for key in part.entries().keys() {
                    map.remove(key);
                }
            }
            return Map::persistent(map);
        }
        let mut entries: BTreeMap<MapKey, Value> = self.entries().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for part in removed {
            for key in part.entries().keys() {
                entries.remove(key);
            }
        }
        Map(RefCell::new(Entries::Tree(entries)), Plain::known(self.1.0.get()))
    }
}

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
        let mut pending = Vec::new();
        take_deep(&mut self.captures, &mut pending);
        if !pending.is_empty() {
            release_all(pending);
        }
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

/// Последняя ссылка на контейнер: её освобождение освобождает дочерние
/// значения и потому может уйти в глубину. Остальные значения (скаляры,
/// строки, числа, разделяемые ссылки) освобождаются на месте без рекурсии.
fn deep(value: &Value) -> bool {
    match value {
        Value::Object(object) => Rc::strong_count(object) == 1,
        Value::Array(array) => Rc::strong_count(array) == 1,
        Value::Map(map) => Rc::strong_count(map) == 1,
        Value::Builder(builder) => Rc::strong_count(builder) == 1,
        Value::MapBuilder(builder) => Rc::strong_count(builder) == 1,
        Value::Fn(closure) => Rc::strong_count(closure) == 1,
        Value::Any(value) => Rc::strong_count(value) == 1,
        _ => false,
    }
}

/// Переносит из `values` в `pending` значения, уходящие в глубину, по
/// порядку: разделяемая ссылка отпускается сразу (это лишь уменьшение
/// счётчика), поэтому её следующая копия в том же контейнере уже последняя
/// и тоже попадает в очередь. Иначе `[child.clone(), child]` оставлял обе
/// копии и вторая освобождала `child` рекурсивно (S07).
fn take_deep(values: &mut [Value], pending: &mut Vec<Value>) {
    for value in values {
        if deep(value) {
            pending.push(std::mem::replace(value, Value::Undef));
        } else if shared(value) {
            drop(std::mem::replace(value, Value::Undef));
        }
    }
}

/// Ссылка на контейнер, у которой есть и другие владельцы.
fn shared(value: &Value) -> bool {
    matches!(
        value,
        Value::Object(_)
            | Value::Array(_)
            | Value::Map(_)
            | Value::Builder(_)
            | Value::MapBuilder(_)
            | Value::Fn(_)
            | Value::Any(_)
    )
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
                match std::mem::take(&mut array.0) {
                    Repr::Flat { mut items, .. } => take_deep(&mut items, pending),
                    Repr::View { buffer, .. } => {
                        if let Some(mut buffer) = Rc::into_inner(buffer) {
                            take_deep(buffer.slots.get_mut(), pending);
                        }
                    }
                    Repr::Slice { origin, .. } => {
                        let mut origin = [Value::Array(origin)];
                        take_deep(&mut origin, pending);
                    }
                    Repr::Trie { trie, flat, .. } => release_trie(trie, flat, pending),
                }
            }
        }
        Value::Map(map) => {
            if let Some(mut map) = Rc::into_inner(map) {
                release_entries(std::mem::take(map.0.get_mut()), pending);
            }
        }
        Value::Builder(builder) => {
            if let Some(builder) = Rc::into_inner(builder) {
                take_deep(&mut builder.into_inner(), pending);
            }
        }
        Value::MapBuilder(builder) => {
            if let Some(builder) = Rc::into_inner(builder) {
                pending.extend(builder.into_inner().into_values().filter(deep));
            }
        }
        Value::Fn(closure) => {
            if let Some(mut closure) = Rc::into_inner(closure) {
                take_deep(&mut closure.captures, pending);
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

/// Простое значение тоже может быть версией с общими узлами (словарь
/// словарей, массив массивов): узлы, которые держит только эта версия,
/// отдают значения в очередь, а не освобождают их рекурсивно (#335).
fn release_trie(trie: okkama_persistent::Trie<Value>, flat: std::cell::OnceCell<Vec<Value>>, pending: &mut Vec<Value>) {
    if let Some(mut flat) = flat.into_inner() {
        take_deep(&mut flat, pending);
    }
    trie.into_unshared(|value| {
        if deep(&value) {
            pending.push(value);
        }
    });
}

fn release_entries(entries: Entries, pending: &mut Vec<Value>) {
    match entries {
        Entries::Tree(entries) => pending.extend(entries.into_values().filter(deep)),
        Entries::Shared(entries) => entries.into_unshared(|value| {
            if deep(&value) {
                pending.push(value);
            }
        }),
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

#[cfg(test)]
thread_local! {
    /// Живые объекты потока (тесты сборщика циклов).
    pub(crate) static LIVE_OBJECTS: Cell<usize> = const { Cell::new(0) };
}

impl Drop for Object {
    fn drop(&mut self) {
        #[cfg(test)]
        LIVE_OBJECTS.with(|live| live.set(live.get() - 1));
        // Пустая очередь не выделяет память: объект из скаляров и
        // разделяемых ссылок освобождается без обхода.
        let mut pending = Vec::new();
        self.drain(&mut pending);
        if !pending.is_empty() {
            release_all(pending);
        }
    }
}

impl Drop for Array {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        match &mut self.0 {
            Repr::Flat { items, .. } => take_deep(items, &mut pending),
            // Буфер и источник освобождают своё сами, тоже без рекурсии.
            Repr::View { .. } | Repr::Slice { .. } => {}
            Repr::Trie { .. } => {
                if let Repr::Trie { trie, flat, .. } = std::mem::take(&mut self.0) {
                    release_trie(trie, flat, &mut pending);
                }
            }
        }
        if !pending.is_empty() {
            release_all(pending);
        }
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        release_entries(std::mem::take(self.0.get_mut()), &mut pending);
        if !pending.is_empty() {
            release_all(pending);
        }
    }
}

/// Ключ словаря: `int`, `byte`, `char`, `bool`, `float`, `rat` (#296), enum,
/// `str` или объект-значение (#212). У одного словаря все ключи одного
/// статического типа.
#[derive(Clone, Debug)]
pub enum MapKey {
    Int(IntValue),
    Float(f64),
    Bool(bool),
    Byte(u8),
    Char(char),
    Str(Rc<str>),
    Enum { declaration: usize, case: usize },
    /// Дробь: несократимая, поэтому `rat(1, 2)` и `rat(2, 4)` — один ключ.
    Rat(Rc<crate::rat::Rat>),
    /// Объект: равенство и порядок — по объявлению и полям, как у `==`;
    /// сам объект хранится, чтобы `keys()` вернул его.
    Object { declaration: usize, fields: Rc<[MapKey]>, object: Rc<Object> },
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
            Value::Rat(value) => Self::Rat(value.clone()),
            Value::Object(object) => {
                let fields = (0..object.field_count())
                    .map(|index| match object.field(index) {
                        Some(Value::Undef) | None => Err(invalid("map key object has an unbound field")),
                        Some(field) => Self::from_value(&field),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Self::Object { declaration: object.declaration, fields: fields.into(), object: object.clone() }
            }
            _ => return Err(invalid("map key must be a scalar, enum, string or value object")),
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
            Self::Rat(value) => Value::Rat(value.clone()),
            Self::Object { object, .. } => Value::Object(object.clone()),
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
            Self::Rat(_) => 7,
            Self::Object { .. } => 8,
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
            (Self::Rat(a), Self::Rat(b)) => a.compare(b),
            (
                Self::Object { declaration: a, fields: x, .. },
                Self::Object { declaration: b, fields: y, .. },
            ) => a.cmp(b).then_with(|| x.iter().cmp(y.iter())),
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

    /// `builder_push` без копии builder для вызывающего, который сам
    /// хранит его версию.
    pub fn builder_push_into(&self, builder: &Value, value: Value) -> Result<(), ValueError> {
        self.builder(builder)?.borrow_mut().push(value);
        Ok(())
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

    pub fn characters(&self, text: &str) -> Value {
        self.allocate(text.chars().map(Value::Char).collect())
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
        Value::Array(Rc::new(Array::new(elements)))
    }

    pub fn elements<'a>(&self, array: &'a Value) -> Result<Items<'a>, ValueError> {
        match array {
            Value::Array(array) => Ok(array.items()),
            _ => Err(invalid("expected array value")),
        }
    }

    /// Массив значения `array`.
    pub fn array<'a>(&self, array: &'a Value) -> Result<&'a Rc<Array>, ValueError> {
        match array {
            Value::Array(array) => Ok(array),
            _ => Err(invalid("expected array value")),
        }
    }

    pub fn lookup(&self, array: &Value, index: &IntValue) -> Result<Option<Value>, ValueError> {
        let array = self.array(array)?;
        Ok(index.to_i64().and_then(|i| usize::try_from(i).ok()).and_then(|i| array.get(i)))
    }
}

/// Операции словаря; ошибки отсутствующего ключа — у исполнителя.
pub mod maps {
    use super::*;

    pub fn new_builder() -> Value {
        Value::MapBuilder(Rc::new(RefCell::new(BTreeMap::new())))
    }

    pub fn map(map: &Value) -> Result<&Rc<Map>, ValueError> {
        match map {
            Value::Map(map) => Ok(map),
            _ => Err(invalid("expected map value")),
        }
    }

    pub fn maps<'a>(maps: &[&'a Value]) -> Result<Vec<&'a Map>, ValueError> {
        maps.iter().map(|value| map(value).map(|map| &**map)).collect()
    }

    pub fn entries(map: &Value) -> Result<MapEntries<'_>, ValueError> {
        Ok(self::map(map)?.entries())
    }

    /// Литерал: повторный ключ заменяет значение, но не ключ (`TreeMap.put`).
    pub fn from_entries(entries: impl IntoIterator<Item = (Value, Value)>) -> Result<Value, ValueError> {
        let mut map = BTreeMap::new();
        for (key, value) in entries {
            put(&mut map, &key, value)?;
        }
        Ok(Value::Map(Rc::new(Map::new(map))))
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
            Value::MapBuilder(map) => Ok(Value::Map(Rc::new(Map::new(std::mem::take(&mut *map.borrow_mut()))))),
            _ => Err(invalid("expected live map builder")),
        }
    }

    pub fn get(map: &Value, key: &Value) -> Result<Option<Value>, ValueError> {
        Ok(self::map(map)?.get(&MapKey::from_value(key)?))
    }

    pub fn keys(map: &Value) -> Result<Value, ValueError> {
        Ok(ArrayHeap.allocate(entries(map)?.collect(|key, _| key.to_value())))
    }

    pub fn values(map: &Value) -> Result<Value, ValueError> {
        Ok(ArrayHeap.allocate(entries(map)?.collect(|_, value| value.clone())))
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

    /// Поля Error-объекта, который создаёт рантайм, из значений его payload.
    /// Error enum опускается в объект `[$tag, $<case>_<имя>: array…]`; рантайм
    /// создаёт первый case, значения payload — массивы из одного элемента.
    pub fn raised_fields<'a>(&self, names: impl IntoIterator<Item = &'a str>, values: Vec<Value>) -> Vec<Value> {
        let mut names = names.into_iter().peekable();
        if names.peek() != Some(&"$tag") {
            return values;
        }
        names.next();
        let mut payload = values.into_iter();
        let mut fields = vec![Value::Int(IntValue::from_i64(0))];
        for name in names {
            let elements = if name.starts_with("$0_") { payload.next().into_iter().collect() } else { Vec::new() };
            fields.push(ArrayHeap.allocate(elements));
        }
        fields
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

    /// `once.store` (#491): запомнить значение слота объекта `value`.
    pub fn once_store(&self, value: &Value, field: usize, bound: Value) -> Result<(), ValueError> {
        let object = match value {
            Value::Object(object) => object,
            _ => return Err(invalid("expected object value")),
        };
        let cyclic = crate::gc::traced(&bound);
        object.once_store(field, bound);
        // Запомненное значение может замкнуть цикл на сам объект.
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
/// идентичности регистрируются партиями; закрытие раскрывает исходы Future
/// и уничтожает объекты в обратном порядке создания, каждый не более одного раза.
#[derive(Debug)]
pub struct Region {
    id: u64,
    entries: RefCell<Vec<Identity>>,
    known: RefCell<std::collections::HashSet<usize>>,
    /// Самый старый `serial` среди `entries` (`u64::MAX` — записей нет).
    oldest: Cell<u64>,
}

/// Futures are identities too: their private region owns their cached outcome.
/// Awaited values never cross that ownership boundary.
#[derive(Debug, Clone)]
enum Identity {
    Object(Rc<Object>),
    Future(crate::sched::Future),
}

impl Identity {
    fn key(&self) -> usize {
        match self {
            Self::Object(object) => Rc::as_ptr(object) as usize,
            Self::Future(future) => Rc::as_ptr(future) as usize,
        }
    }

    fn serial(&self) -> u64 {
        match self {
            Self::Object(object) => object.serial,
            Self::Future(future) => future.serial,
        }
    }

    fn owner(&self) -> &Cell<u64> {
        match self {
            Self::Object(object) => &object.owner,
            Self::Future(future) => &future.owner,
        }
    }

    fn value(&self) -> Value {
        match self {
            Self::Object(object) => Value::Object(object.clone()),
            Self::Future(future) => Value::Future(future.clone()),
        }
    }
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

// Reuse only empty work buffers, never Values or object identities. Take the
// slot for the duration of a walk so nested/reentrant walks get independent state.
#[derive(Default)]
struct WalkScratch {
    seen: std::collections::HashSet<usize>,
    pending: Vec<Value>,
    open: Vec<Walked>,
}

/// Контейнер, элементы которого обходит `collect`: его поддерево пройдено,
/// когда `pending` вернулся к `base`.
struct Walked {
    container: Value,
    base: usize,
    steps: usize,
    events: usize,
}

/// Шагов обхода под контейнером, ради которых его стоит запомнить: меньший
/// дешевле обойти заново.
const SUMMARY_STEPS: usize = 64;

/// Слабая ссылка на запомненный контейнер: пока она жива, его адрес не
/// достанется другому значению.
enum Held {
    Array(std::rc::Weak<Array>),
    Map(std::rc::Weak<Map>),
    Fn(std::rc::Weak<Closure>),
    Any(std::rc::Weak<Value>),
}

impl Held {
    fn alive(&self) -> bool {
        match self {
            Held::Array(weak) => weak.strong_count() > 0,
            Held::Map(weak) => weak.strong_count() > 0,
            Held::Fn(weak) => weak.strong_count() > 0,
            Held::Any(weak) => weak.strong_count() > 0,
        }
    }
}

/// Адрес контейнера — ключ его сводки.
fn container_key(value: &Value) -> Option<usize> {
    Some(match value {
        Value::Array(array) => Rc::as_ptr(array) as usize,
        Value::Map(map) => Rc::as_ptr(map) as usize,
        Value::Fn(closure) => Rc::as_ptr(closure) as usize,
        Value::Any(value) => Rc::as_ptr(value) as usize,
        _ => return None,
    })
}

/// Контейнеры, обход которых не встретил свободных идентичностей (#9), со
/// счётчиком идентичностей в тот момент: всё, что они достигают, не новее
/// его. Освобождение идентичности не новее `summarized` сбрасывает все.
#[derive(Default)]
struct Summaries {
    entries: std::collections::HashMap<usize, (Held, u64)>,
    summarized: u64,
    /// Размер `entries` после последней чистки мёртвых.
    pruned: usize,
}

thread_local! {
    static SUMMARIES: RefCell<Summaries> = RefCell::new(Summaries::default());
}

/// Запомнить, что `container` сейчас не достигает свободных идентичностей.
fn summarize(container: &Value) {
    let held = match container {
        Value::Array(array) => Held::Array(Rc::downgrade(array)),
        Value::Map(map) => Held::Map(Rc::downgrade(map)),
        Value::Fn(closure) => Held::Fn(Rc::downgrade(closure)),
        Value::Any(value) => Held::Any(Rc::downgrade(value)),
        _ => return,
    };
    let Some(key) = container_key(container) else { return };
    let serial = CREATED.with(Cell::get);
    let _ = SUMMARIES.try_with(|summaries| {
        let mut summaries = summaries.borrow_mut();
        summaries.entries.insert(key, (held, serial));
        summaries.summarized = summaries.summarized.max(serial);
        if summaries.entries.len() > 2 * summaries.pruned.max(1024) {
            summaries.entries.retain(|_, (held, _)| held.alive());
            summaries.pruned = summaries.entries.len();
        }
    });
}

/// Может ли обход (через регион `via`: его id и самая старая запись)
/// пропустить `container`?
fn summary_holds(container: &Value, via: Option<(u64, u64)>) -> bool {
    let Some(key) = container_key(container) else { return false };
    let serial = SUMMARIES
        .try_with(|summaries| summaries.borrow().entries.get(&key).map(|(_, serial)| *serial))
        .ok()
        .flatten();
    match (serial, via) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(serial), Some((_, oldest))) => serial < oldest,
    }
}

/// Свободной снова стала идентичность не новее сводки: сводки могут её
/// достигать.
fn freed(serial: u64) {
    let dropped = SUMMARIES
        .try_with(|summaries| {
            let mut summaries = summaries.borrow_mut();
            if serial > summaries.summarized {
                return None;
            }
            summaries.summarized = 0;
            summaries.pruned = 0;
            Some(std::mem::take(&mut summaries.entries))
        })
        .ok()
        .flatten();
    drop(dropped);
}

fn has_summaries() -> bool {
    SUMMARIES.try_with(|summaries| !summaries.borrow().entries.is_empty()).unwrap_or(false)
}

thread_local! {
    static WALK_SCRATCH: RefCell<Option<WalkScratch>> = const { RefCell::new(None) };
}

struct WalkLease(Option<WalkScratch>);

impl WalkLease {
    fn new() -> Self {
        let scratch = WALK_SCRATCH.try_with(|slot| slot.borrow_mut().take()).ok().flatten();
        Self(Some(scratch.unwrap_or_default()))
    }
}

impl Drop for WalkLease {
    fn drop(&mut self) {
        let Some(mut scratch) = self.0.take() else { return; };
        // Clear before borrowing the cache: dropping a Value can reenter runtime.
        scratch.pending.clear();
        scratch.seen.clear();
        scratch.open.clear();
        // Do not retain an unusually large traversal's high-water mark forever.
        if scratch.pending.capacity() > 1024 || scratch.seen.capacity() > 4096 {
            return;
        }
        let mut available = Some(scratch);
        let _ = WALK_SCRATCH.try_with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut()
                && slot.is_none() { *slot = available.take(); }
        });
        // Any unused scratch drops outside the cache borrow, including at TLS exit.
    }
}

/// Идентичности, достижимые из `value`: свободные, а при `via` (id региона
/// и его самая старая запись) ещё и принадлежащие `via`. Чужие идентичности
/// не обходятся. Без рекурсии.
///
/// Контейнеры неизменяемы, поэтому всё, что контейнер достигает, не новее
/// его. Контейнер, обход которого не встретил свободной идентичности (только
/// чужие, уничтоженные или скаляры), запоминается (#9): следующие обходы его
/// пропускают, пока не освободится идентичность не новее сводки, а обход
/// через `via` — пока все записи `via` новее её. Большая структура, которой
/// владеет открытый регион, обходится один раз, а не на каждое значение,
/// которое на неё ссылается.
fn collect(value: &Value, out: &mut Vec<Identity>, via: Option<(u64, u64)>) {
    #[cfg(feature = "allocation-scopes")]
    let _allocation_scope = okkama_allocation_counter::enter(2);
    #[cfg(feature = "region-walk-stats")]
    let mut stats = [0, 0, 0, 1, 0, 0, 0, 0, 0];
    let mut lease = WalkLease::new();
    let WalkScratch { seen, pending, open } = lease.0.as_mut().expect("walk scratch");
    pending.push(value.clone());
    let summaries = has_summaries();
    // `events` — то, что мешает сводке: свободная идентичность или
    // идентичность `via`, Future (его исход меняется).
    let (mut steps, mut events) = (0, 0);
    loop {
        while let Some(walked) = open.last()
            && pending.len() <= walked.base
        {
            let walked = open.pop().expect("open container");
            if walked.events == events && steps - walked.steps >= SUMMARY_STEPS {
                summarize(&walked.container);
            }
        }
        let Some(value) = pending.pop() else { break };
        steps += 1;
        #[cfg(feature = "region-walk-stats")]
        { stats[4] += 1; }
        let base = pending.len();
        match value {
            Value::Object(object) => {
                #[cfg(feature = "region-walk-stats")]
                { stats[5] += 1; }
                let owner = object.owner.get();
                if !(owner == FREE || via.is_some_and(|(id, _)| id == owner)) {
                    #[cfg(feature = "region-walk-stats")]
                    { stats[6] += 1; }
                    continue;
                }
                events += 1;
                if !seen.insert(Rc::as_ptr(&object) as usize) {
                    #[cfg(feature = "region-walk-stats")]
                    { stats[7] += 1; }
                    continue;
                }
                #[cfg(feature = "region-walk-stats")]
                { stats[8] += 1; }
                for index in (0..object.field_count()).rev() {
                    if let Some(value) = object.field(index)
                        && !matches!(value, Value::Undef) {
                            pending.push(value);
                        }
                }
                out.push(Identity::Object(object));
            }
            // An array of plain values holds no identity to walk (#335).
            Value::Array(array) if array.plain() => {}
            Value::Map(map) if map.plain() => {}
            Value::Array(_) | Value::Map(_) | Value::Any(_) | Value::Fn(_) => {
                if summaries && summary_holds(&value, via) {
                    continue;
                }
                match &value {
                    Value::Array(array) => pending.extend(array.items().iter().rev().cloned()),
                    Value::Map(map) => {
                        let values: Vec<Value> = map.entries().values().cloned().collect();
                        pending.extend(values.into_iter().rev());
                    }
                    Value::Any(value) => pending.push((**value).clone()),
                    Value::Fn(closure) => pending.extend(closure.captures.iter().rev().cloned()),
                    _ => unreachable!("matched above"),
                }
                open.push(Walked { container: value, base, steps, events });
            }
            Value::Future(future) => {
                events += 1;
                if !seen.insert(Rc::as_ptr(&future) as usize) {
                    continue;
                }
                // Inputs still follow #578 even when a collector privately owns
                // this cached Future: an escaping results() keeps its receiver.
                // The outcome itself is never traversed here.
                pending.extend(future.inputs().into_iter().rev());
                let owner = future.owner.get();
                if owner == FREE || via.is_some_and(|(id, _)| id == owner) {
                    out.push(Identity::Future(future));
                }
            }
            _ => {}
        }
    }
    #[cfg(feature = "region-walk-stats")]
    walk_stats::add(stats);
}

/// Состояние закрытия региона: исполнитель вызывает `destroy` по одному
/// объекту за шаг, потому что тело `destroy` — обычный вызов программы.
#[derive(Debug)]
pub struct Closing {
    kept: std::collections::HashSet<usize>,
    index: usize,
}

impl Region {
    pub fn new() -> Self {
        #[cfg(feature = "allocation-scopes")]
        let _allocation_scope = okkama_allocation_counter::enter(1);
        let id = REGIONS.with(|regions| {
            regions.set(regions.get() + 1);
            regions.get()
        });
        Self {
            id,
            entries: RefCell::new(Vec::new()),
            known: RefCell::new(std::collections::HashSet::new()),
            oldest: Cell::new(u64::MAX),
        }
    }

    fn remember(&self, object: Identity) {
        if self.known.borrow_mut().insert(object.key()) {
            self.oldest.set(self.oldest.get().min(object.serial()));
            self.entries.borrow_mut().push(object);
        }
    }

    fn register(&self, value: &Value, own: bool) {
        #[cfg(feature = "no-regions-bench")]
        if regions_disabled() { return; }
        #[cfg(feature = "allocation-scopes")]
        let _allocation_scope = okkama_allocation_counter::enter(1);
        if plain(value) {
            return;
        }
        let mut fresh = Vec::new();
        #[cfg(feature = "region-walk-stats")]
        walk_stats::add([1, 0, 0, 0, 0, 0, 0, 0, 0]);
        collect(value, &mut fresh, None);
        fresh.sort_by_key(|object| object.serial());
        for object in fresh {
            if own {
                object.owner().set(self.id);
            }
            self.remember(object);
        }
    }

    fn drain_into(&self, to: &Region) {
        let entries = std::mem::take(&mut *self.entries.borrow_mut());
        self.known.borrow_mut().clear();
        self.oldest.set(u64::MAX);
        for identity in entries {
            if identity.owner().get() == self.id {
                identity.owner().set(to.id);
                to.remember(identity);
            }
        }
    }

    /// A collector owns its cached Future, whose receiver remains a borrow.
    pub(crate) fn own_future(&self, future: &crate::sched::Future) {
        if future.owner.get() == FREE {
            future.owner.set(self.id);
            self.remember(Identity::Future(future.clone()));
        }
    }

    pub(crate) fn edges(&self, out: &mut crate::gc::Edges) -> bool {
        let Ok(entries) = self.entries.try_borrow() else { return false };
        for identity in entries.iter() {
            out.value(&identity.value());
        }
        true
    }

    pub(crate) fn clear(&self, sink: &mut Vec<Value>) {
        if let Ok(mut entries) = self.entries.try_borrow_mut() {
            sink.extend(entries.drain(..).map(|identity| identity.value()));
            self.known.borrow_mut().clear();
            self.oldest.set(u64::MAX);
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
        #[cfg(feature = "no-regions-bench")]
        if regions_disabled() { return; }
        #[cfg(feature = "allocation-scopes")]
        let _allocation_scope = okkama_allocation_counter::enter(1);
        if self.entries.borrow().is_empty() || plain(value) {
            return;
        }
        let mut reachable = Vec::new();
        #[cfg(feature = "region-walk-stats")]
        walk_stats::add([0, 1, 0, 0, 0, 0, 0, 0, 0]);
        collect(value, &mut reachable, Some((self.id, self.oldest.get())));
        reachable.sort_by_key(|object| object.serial());
        for object in reachable {
            if object.owner().get() != self.id {
                continue;
            }
            object.owner().set(to.id);
            to.remember(object);
        }
    }

    /// Регион без идентичностей: его закрытие ничего не уничтожает и не
    /// освобождает, поэтому исполнители пропускают `begin_close`.
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    /// Начать закрытие; идентичности, достижимые из `keep`, освобождаются.
    pub fn begin_close(&self, keep: &[Value]) -> Closing {
        #[cfg(feature = "no-regions-bench")]
        if regions_disabled() {
            return Closing { kept: std::collections::HashSet::new(), index: 0 };
        }
        #[cfg(feature = "allocation-scopes")]
        let _allocation_scope = okkama_allocation_counter::enter(1);
        let mut kept = std::collections::HashSet::new();
        for value in keep {
            #[cfg(feature = "region-walk-stats")]
            walk_stats::add([0, 0, 1, 0, 0, 0, 0, 0, 0]);
            let mut reachable = Vec::new();
            collect(value, &mut reachable, Some((self.id, self.oldest.get())));
            kept.extend(reachable.iter().map(Identity::key));
        }
        // Expand only Futures being destroyed, never a borrowed outcome in keep.
        // Expansion precedes sorting so all resources obey creation-order LIFO,
        // including outcomes of different Futures and ordinary local objects.
        let mut index = 0;
        while index < self.entries.borrow().len() {
            let identity = self.entries.borrow()[index].clone();
            index += 1;
            if kept.contains(&identity.key())
                || (identity.owner().get() != FREE && identity.owner().get() != self.id)
            {
                continue;
            }
            match identity {
                Identity::Future(future) => {
                    future.owner.set(DEAD);
                    future.owned.drain_into(self);
                }
                Identity::Object(object) => {
                    if let Some(state) = object.intrinsic()
                        && let Some(owned) = crate::intrinsics::owned_state(state.as_ref())
                    {
                        owned.drain_into(self);
                    }
                }
            }
        }
        self.entries.borrow_mut().sort_by_key(Identity::serial);
        Closing { kept, index: self.entries.borrow().len() }
    }

    /// Следующий объект для `destroy` (уже помечен уничтоженным) или None,
    /// когда регион закрыт.
    pub fn next_destroyed(&self, closing: &mut Closing) -> Option<Rc<Object>> {
        #[cfg(feature = "allocation-scopes")]
        let _allocation_scope = okkama_allocation_counter::enter(1);
        while closing.index > 0 {
            closing.index -= 1;
            let object = self.entries.borrow()[closing.index].clone();
            if closing.kept.contains(&object.key()) {
                if object.owner().get() == self.id {
                    object.owner().set(FREE);
                    freed(object.serial());
                }
                continue;
            }
            let owner = object.owner().get();
            if owner != self.id && owner != FREE {
                continue; // принят другим владельцем или уже уничтожен
            }
            object.owner().set(DEAD);
            if let Identity::Object(object) = object {
                return Some(object);
            }
        }
        self.entries.borrow_mut().clear();
        self.known.borrow_mut().clear();
        self.oldest.set(u64::MAX);
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
                // Численно, как скалярное `==` (#314): `-0.0 == 0.0`; NaN в Okkama нет.
                (Value::Float(a), Value::Float(b)) => a == b,
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
                    if x.len() != y.len() {
                        return Ok(false);
                    }
                    if !Rc::ptr_eq(x, y)
                        && seen.insert((1, Rc::as_ptr(x) as usize, Rc::as_ptr(y) as usize))
                    {
                        pending.extend(x.items().iter().cloned().zip(y.items().iter().cloned()).rev());
                    }
                    true
                }
                // `AbstractMap.equals`: те же ключи (по порядку ключей) и равные значения.
                (Value::Map(x), Value::Map(y)) => {
                    if x.len() != y.len() {
                        return Ok(false);
                    }
                    if !Rc::ptr_eq(x, y)
                        && seen.insert((2, Rc::as_ptr(x) as usize, Rc::as_ptr(y) as usize))
                    {
                        let (x, y) = (x.entries(), y.entries());
                        for ((a, u), (b, v)) in x.iter().zip(y.iter()) {
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

    /// Chains of `replaced` (#335, #250): a trie from the second replacement
    /// of a large array of plain values, every version unchanged, reads by
    /// path until a pass of them flattens it; arrays holding objects are
    /// copied as before.
    #[test]
    fn replacement_chains_become_tries_and_keep_every_version() {
        let int = |i: i64| Value::Int(crate::IntValue::from_i64(i));
        let ints = |value: &Value| -> Vec<i64> {
            ArrayHeap.elements(value).unwrap().iter().map(|v| v.as_int().unwrap().to_i64().unwrap()).collect()
        };
        let trie = |value: &Value| matches!(value, Value::Array(array) if matches!(array.0, Repr::Trie { .. }));
        let replace = |value: &Value, at: usize, item: i64| match value {
            Value::Array(array) => array.replaced(at, int(item)),
            _ => unreachable!(),
        };
        let mut versions = vec![ArrayHeap.allocate((0..100).map(int).collect())];
        for step in 0..50 {
            let next = replace(versions.last().unwrap(), (step * 7) % 100, 1000 + step as i64);
            versions.push(next);
        }
        assert!(!trie(&versions[1]) && trie(&versions[2]) && trie(&versions[50]));
        let mut expected: Vec<i64> = (0..100).collect();
        for (step, version) in versions.iter().enumerate() {
            assert_eq!(ints(version), expected, "version {step}");
            if step < 50 {
                expected[(step * 7) % 100] = 1000 + step as i64;
            }
        }
        // Reads by index, then flat after two passes.
        let Value::Array(last) = versions.last().unwrap() else { unreachable!() };
        for pass in 0..3 {
            for (i, &want) in expected.iter().enumerate() {
                assert_eq!(last.get(i).and_then(|v| v.as_int().ok().and_then(|n| n.to_i64())), Some(want), "pass {pass}");
            }
        }
        assert!(matches!(&last.0, Repr::Trie { flat, .. } if flat.get().is_some()));
        assert!(last.get(100).is_none());
        // A flattened trie still replaces by path.
        let after = replace(versions.last().unwrap(), 3, -3);
        assert!(trie(&after) && ints(&after)[3] == -3 && ints(versions.last().unwrap())[3] == expected[3]);
        // Holding an object: never a trie.
        let object = crate::heap::ObjectHeap.allocate(0, vec![]);
        let mixed = ArrayHeap.allocate((0..99).map(int).chain([object]).collect());
        let Value::Array(array) = &mixed else { unreachable!() };
        let once = array.replaced(0, int(5));
        let Value::Array(once_array) = &once else { unreachable!() };
        let twice = once_array.replaced(1, int(6));
        assert!(!trie(&once) && !trie(&twice));
    }

    /// Versions of maps (#335): a large map of plain values turns persistent
    /// at its first new version and stays the same map; chains, branches,
    /// merges from either side and differences keep every version as it
    /// was, against a `BTreeMap` model; maps holding objects are copied as
    /// before.
    #[test]
    fn map_versions_share_nodes_and_keep_every_version() {
        use crate::ops::{map_difference, map_merge};
        let int = |i: i64| Value::Int(crate::IntValue::from_i64(i));
        let map = |entries: &[(i64, i64)]| maps::from_entries(entries.iter().map(|&(k, v)| (int(k), int(v)))).unwrap();
        let shared = |value: &Value| matches!(value, Value::Map(map) if matches!(&*map.0.borrow(), Entries::Shared(_)));
        let pairs = |value: &Value| -> Vec<(i64, i64)> {
            maps::entries(value)
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    let MapKey::Int(k) = k else { unreachable!() };
                    (k.to_i64().unwrap(), v.as_int().unwrap().to_i64().unwrap())
                })
                .collect()
        };
        let model = |value: &BTreeMap<i64, i64>| value.iter().map(|(&k, &v)| (k, v)).collect::<Vec<_>>();
        // A chain from a literal of 100 entries.
        let base = map(&(0..100).map(|i| (i * 2, i)).collect::<Vec<_>>());
        assert!(!shared(&base));
        let mut versions = vec![base.clone()];
        let mut models = vec![(0..100).map(|i| (i * 2, i)).collect::<BTreeMap<_, _>>()];
        for step in 0..200i64 {
            let key = (step * 37) % 260;
            let next = map_merge(&[versions.last().unwrap(), &map(&[(key, -step)])]).unwrap();
            let mut expected = models.last().unwrap().clone();
            expected.insert(key, -step);
            versions.push(next);
            models.push(expected);
        }
        assert!(shared(&base), "the base turns persistent in place");
        assert!(versions.iter().all(shared));
        // Branches of the same base, and removals.
        for step in 0..50i64 {
            let fresh = map_merge(&[&base, &map(&[(step * 2 + 1, step)])]).unwrap();
            let mut expected = models[0].clone();
            expected.insert(step * 2 + 1, step);
            versions.push(fresh);
            models.push(expected);
            let removed = map_difference(&base, &[&map(&[(step * 2, 0), (999, 0)])]).unwrap();
            let mut expected = models[0].clone();
            expected.remove(&(step * 2));
            versions.push(removed);
            models.push(expected);
        }
        // The large part on the right: the left keys under it lose to it.
        let left = map(&[(0, 100), (1, 101), (500, 102)]);
        let merged = map_merge(&[&left, &base, &map(&[(1, 7)])]).unwrap();
        let mut expected = models[0].clone();
        expected.insert(1, 7);
        expected.insert(500, 102);
        versions.push(merged);
        models.push(expected);
        for (at, (version, expected)) in versions.iter().zip(&models).enumerate() {
            assert_eq!(pairs(version), model(expected), "version {at}");
            assert_eq!(maps::map(version).unwrap().len(), expected.len(), "version {at}");
            for (&k, &v) in expected {
                assert_eq!(maps::get(version, &int(k)).unwrap().and_then(|v| v.as_int().ok().and_then(|n| n.to_i64())), Some(v));
            }
            assert!(maps::get(version, &int(-1)).unwrap().is_none());
        }
        // The left key stays, as with `BTreeMap::insert`: -0.0 before 0.0.
        let floats = maps::from_entries((0..100).map(|i| (Value::Float(i as f64), int(i)))).unwrap();
        let merged = map_merge(&[&maps::from_entries([(Value::Float(-0.0), int(-1))]).unwrap(), &floats]).unwrap();
        assert!(shared(&merged));
        let keys = maps::keys(&merged).unwrap();
        assert!(ArrayHeap.elements(&keys).unwrap()[0].as_float().unwrap().is_sign_negative());
        assert_eq!(maps::get(&merged, &Value::Float(0.0)).unwrap().and_then(|v| v.as_int().ok().and_then(|n| n.to_i64())), Some(0));
        // Holding an object: never persistent, and a node of the collector.
        let object = ObjectHeap.allocate(0, vec![]);
        let mixed = maps::from_entries((0..99).map(int).map(|k| (k, int(0))).chain([(int(99), object)])).unwrap();
        let once = map_merge(&[&mixed, &map(&[(5, 5)])]).unwrap();
        let twice = map_merge(&[&once, &map(&[(6, 6)])]).unwrap();
        assert!(!shared(&mixed) && !shared(&once) && !shared(&twice));
        assert!(crate::gc::traced(&twice) && !crate::gc::traced(&base) && !crate::gc::traced(&map(&[])));
        // A plain value written into a persistent map that is not plain: a copy.
        let into = map_merge(&[&base, &maps::from_entries([(int(1), ObjectHeap.allocate(0, vec![]))]).unwrap()]).unwrap();
        assert!(!shared(&into) && crate::gc::traced(&into) && maps::map(&into).unwrap().len() == 101);
    }

    /// The raw cost of `keys()` of a large map, of either representation,
    /// and straight from the tree: run with `cargo
    /// test --release -p okkama-runtime-core keys_costs -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn keys_costs() {
        let int = |i: i64| Value::Int(crate::IntValue::from_i64(i));
        let map = maps::from_entries((0..100_000).map(|i| (int(i * 2), int(i)))).unwrap();
        let Value::Map(rc) = &map else { unreachable!() };
        for round in 0..3 {
            let started = std::time::Instant::now();
            for _ in 0..50 {
                std::hint::black_box(maps::keys(&map).unwrap());
            }
            let through = started.elapsed().as_nanos() as f64 / 50.0 / 100_000.0;
            let started = std::time::Instant::now();
            for _ in 0..50 {
                let entries = rc.0.borrow();
                let Entries::Tree(tree) = &*entries else { unreachable!() };
                std::hint::black_box(ArrayHeap.allocate(tree.keys().map(MapKey::to_value).collect()));
            }
            let straight = started.elapsed().as_nanos() as f64 / 50.0 / 100_000.0;
            println!("round {round}: maps::keys {through:.2} ns/key, straight {straight:.2} ns/key");
        }
        let version = crate::ops::map_merge(&[&map, &maps::from_entries([(int(1), int(1))]).unwrap()]).unwrap();
        assert!(matches!(&*rc.0.borrow(), Entries::Shared(_)));
        for round in 0..3 {
            let started = std::time::Instant::now();
            for _ in 0..50 {
                std::hint::black_box(maps::keys(&version).unwrap());
            }
            let shared = started.elapsed().as_nanos() as f64 / 50.0 / 100_000.0;
            println!("round {round}: maps::keys of a persistent map {shared:.2} ns/key");
        }
        let probes: Vec<Value> = (0..200_000i64).map(|i| int((i * 7919) % 200_000)).collect();
        for round in 0..3 {
            for (name, value) in [("tree", &map), ("persistent", &version)] {
                let started = std::time::Instant::now();
                for probe in &probes {
                    std::hint::black_box(maps::get(value, probe).unwrap());
                }
                let ns = started.elapsed().as_nanos() as f64 / probes.len() as f64;
                println!("round {round}: maps::get of a {name} map {ns:.2} ns");
            }
        }
    }

    /// The raw cost of a step of a replacement chain: run with `cargo test
    /// --release -p okkama-runtime-core replace_costs -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn replace_costs() {
        for n in [1_000usize, 16_000, 100_000] {
            let mut value = ArrayHeap.allocate((0..n as i64).map(|i| Value::Int(crate::IntValue::from_i64(i))).collect());
            let steps = 20_000;
            let started = std::time::Instant::now();
            for step in 0..steps {
                let Value::Array(array) = &value else { unreachable!() };
                value = array.replaced((step * 7919) % n, Value::Int(crate::IntValue::from_i64(step as i64)));
            }
            println!("n={n:<7} {:8.1} ns per replacement", started.elapsed().as_nanos() as f64 / steps as f64);
        }
    }

    /// Versions of shared arrays (#335): appends and prepends written in
    /// place, branches that copy, slices that share, and no version that
    /// ever changes.
    #[test]
    fn shared_arrays_keep_every_version_as_it_was() {
        use crate::ops::{array_concat, array_slice};
        let int = |i: i64| Value::Int(crate::IntValue::from_i64(i));
        let ints = |value: &Value| -> Vec<i64> {
            ArrayHeap.elements(value).unwrap().iter().map(|v| v.as_int().unwrap().to_i64().unwrap()).collect()
        };
        let one = |i: i64| ArrayHeap.allocate(vec![int(i)]);
        let base = ArrayHeap.allocate((0..40).map(int).collect());
        let mut versions = vec![base.clone()];
        let mut value = base;
        for i in 40..100 {
            value = array_concat(&[&value, &one(i)]).unwrap();
            versions.push(value.clone());
        }
        for (n, version) in versions.iter().enumerate() {
            assert_eq!(ints(version), (0..40 + n as i64).collect::<Vec<_>>(), "append version {n}");
        }
        // The third and later appends share one buffer, written in place.
        let storage = |value: &Value| match value {
            Value::Array(array) => match array.storage() {
                Storage::Buffer(buffer) => Some(Rc::as_ptr(buffer) as usize),
                _ => None,
            },
            _ => None,
        };
        assert!(storage(&versions[3]).is_some());
        assert_eq!(storage(&versions[3]), storage(&versions[60]));
        // A branch off an older version copies: neither version changes.
        let branch = array_concat(&[&versions[30], &one(-1)]).unwrap();
        assert_ne!(storage(&branch), storage(&versions[60]));
        assert_eq!(ints(&branch).last(), Some(&-1));
        assert_eq!(ints(&versions[31]).last(), Some(&70));
        // Prepends mirror appends, and alternating ends keep both.
        let mut front = ArrayHeap.allocate((0..40).map(int).collect());
        for i in 1..=50 {
            front = array_concat(&[&one(-i), &front]).unwrap();
            front = array_concat(&[&front, &one(100 + i)]).unwrap();
        }
        let expected: Vec<i64> = (-50..=-1).chain(0..40).chain(101..=150).collect();
        assert_eq!(ints(&front), expected);
        // A slice keeping half shares the storage; a smaller one copies.
        let big = ArrayHeap.allocate((0..100).map(int).collect());
        let at = |i: i64| crate::IntValue::from_i64(i);
        let half = array_slice(&big, &at(10), &at(70)).unwrap();
        let small = array_slice(&big, &at(10), &at(30)).unwrap();
        let shares = |value: &Value| matches!(value, Value::Array(array) if matches!(array.storage(), Storage::Origin(_)));
        assert!(shares(&half) && !shares(&small));
        assert_eq!(ints(&half), (10..70).collect::<Vec<_>>());
        let inner = array_slice(&half, &at(5), &at(60)).unwrap();
        assert_eq!(ints(&inner), (15..70).collect::<Vec<_>>());
        // A slice of a slice points at the origin itself, while it keeps
        // half of it; less is copied.
        assert!(!shares(&array_slice(&half, &at(5), &at(45)).unwrap()));
        if let (Value::Array(inner), Value::Array(big)) = (&inner, &big)
            && let Storage::Origin(origin) = inner.storage()
        {
            assert!(Rc::ptr_eq(origin, big));
        } else {
            panic!("the slice of a slice shares the origin");
        }
        // Equal to a flat array with the same elements.
        let flat = ArrayHeap.allocate((15..70).map(int).collect());
        assert!(crate::ops::equal(&inner, &flat).unwrap());
    }

    #[test]
    fn walk_cache_clears_values_on_unwind_and_is_reentrant() {
        WALK_SCRATCH.with(|slot| *slot.borrow_mut() = None);
        let object = Rc::new(Object::new(0, vec![]));
        let weak = Rc::downgrade(&object);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut outer = WalkLease::new();
            outer.0.as_mut().unwrap().pending.push(Value::Object(object));
            let mut inner = WalkLease::new();
            assert!(inner.0.as_ref().unwrap().pending.is_empty());
            inner.0.as_mut().unwrap().seen.insert(weak.as_ptr() as usize);
            drop(inner);
            panic!("exercise traversal cleanup");
        }));
        assert!(weak.upgrade().is_none());
        let next = WalkLease::new();
        assert!(next.0.as_ref().unwrap().pending.is_empty());
        assert!(next.0.as_ref().unwrap().seen.is_empty());
    }

    #[test]
    fn walk_cache_does_not_retain_large_capacity() {
        WALK_SCRATCH.with(|slot| *slot.borrow_mut() = None);
        let mut walk = WalkLease::new();
        walk.0.as_mut().unwrap().pending.reserve(2048);
        drop(walk);
        WALK_SCRATCH.with(|slot| assert!(slot.borrow().is_none()));
    }

    #[test]
    fn cold_future_keep_transfers_fresh_dependencies_and_preserves_borrows() {
        let borrowed = Rc::new(Object::new(0, vec![]));
        let receiver = Rc::new(Object::new(0, vec![Value::Object(borrowed.clone())]));
        let argument = Rc::new(Object::new(0, vec![]));
        let outer = Region::new();
        outer.adopt(&Value::Object(borrowed.clone()));
        let make = Region::new();
        make.track(&Value::Object(receiver.clone()));
        make.adopt(&Value::Object(argument.clone()));
        let future = Value::Future(crate::sched::cold(
            Box::new(|| Ok(Value::Undef)),
            vec![Value::Object(receiver.clone()), Value::Object(argument.clone()),
                 Value::Object(receiver.clone()), Value::Object(borrowed.clone())],
        ));
        let mut closing = make.begin_close(std::slice::from_ref(&future));
        assert!(make.next_destroyed(&mut closing).is_none());
        assert_eq!(receiver.owner.get(), FREE);
        assert_eq!(argument.owner.get(), FREE);
        assert_eq!(borrowed.owner.get(), outer.id);
        let caller = Region::new();
        caller.adopt(&future);
        caller.adopt(&future);
        let mut closing = caller.begin_close(&[]);
        assert!(Rc::ptr_eq(&caller.next_destroyed(&mut closing).unwrap(), &argument));
        assert!(Rc::ptr_eq(&caller.next_destroyed(&mut closing).unwrap(), &receiver));
        assert!(caller.next_destroyed(&mut closing).is_none());
        let mut closing = outer.begin_close(&[]);
        assert!(Rc::ptr_eq(&outer.next_destroyed(&mut closing).unwrap(), &borrowed));
        assert!(outer.next_destroyed(&mut closing).is_none());
    }

    #[test]
    fn future_outcome_owner_blocks_adopt_track_transfer_and_keep() {
        let owner = Region::new();
        let borrower = Region::new();
        let future = crate::sched::promise();
        let value = Value::Future(future.clone());
        owner.adopt(&value);
        let result = Rc::new(Object::new(0, vec![]));
        let alias = Value::Object(result.clone());
        crate::sched::complete(&future, Ok(alias.clone()));
        borrower.adopt(&alias);
        owner.track(&alias); // #579: even the Future's own scope cannot own its result.
        owner.transfer(&borrower, &alias);
        assert_eq!(result.owner.get(), future.owned.id);
        let mut close = borrower.begin_close(&[]);
        assert!(borrower.next_destroyed(&mut close).is_none());
        let mut close = owner.begin_close(&[alias]);
        assert!(Rc::ptr_eq(&owner.next_destroyed(&mut close).unwrap(), &result));
        assert!(owner.next_destroyed(&mut close).is_none());
    }

    #[test]
    fn future_outcome_duplicate_graph_and_transfer_preserve_creation_order() {
        let first = Region::new();
        let second = Region::new();
        let a = crate::sched::promise();
        let b = crate::sched::promise();
        let graph = ArrayHeap.allocate(vec![Value::Future(b.clone()), Value::Future(a.clone()), Value::Future(b.clone())]);
        first.adopt(&graph);
        let x = Rc::new(Object::new(0, vec![]));
        crate::sched::complete(&a, Ok(Value::Object(x.clone())));
        let middle = Rc::new(Object::new(0, vec![]));
        second.adopt(&Value::Object(middle.clone()));
        let y = Rc::new(Object::new(0, vec![]));
        crate::sched::complete(&b, Ok(Value::Object(y.clone())));
        first.transfer(&second, &graph);
        let mut close = first.begin_close(&[]);
        assert!(first.next_destroyed(&mut close).is_none());
        let mut close = second.begin_close(&[]);
        for expected in [&y, &middle, &x] {
            assert!(Rc::ptr_eq(&second.next_destroyed(&mut close).unwrap(), expected));
        }
        assert!(second.next_destroyed(&mut close).is_none());
    }

    #[test]
    fn future_outcome_cached_failure_owns_payload_once() {
        let owner = Region::new();
        let future = crate::sched::promise();
        owner.adopt(&Value::Future(future.clone()));
        let payload = Rc::new(Object::new(0, vec![]));
        let failure = crate::failure::Failure::new(crate::failure::Kind::Thrown {
            object: Ref(payload.clone()), name: "Test".into(), message: None,
        });
        crate::sched::complete(&future, Err(failure.clone()));
        assert!(Rc::ptr_eq(&future.outcome().unwrap_err(), &failure));
        assert!(Rc::ptr_eq(&future.outcome().unwrap_err(), &failure));
        let borrower = Region::new();
        borrower.adopt(&Value::Object(payload.clone()));
        let mut close = borrower.begin_close(&[]);
        assert!(borrower.next_destroyed(&mut close).is_none());
        let mut close = owner.begin_close(&[]);
        assert!(Rc::ptr_eq(&owner.next_destroyed(&mut close).unwrap(), &payload));
        assert!(owner.next_destroyed(&mut close).is_none());
    }

    #[test]
    fn reused_walks_preserve_ownership_and_destruction_order() {
        let a = Rc::new(Object::new(0, vec![]));
        let b = Rc::new(Object::new(0, vec![Value::Object(a.clone())]));
        let root = Value::Object(b.clone());
        let first = Region::new();
        let second = Region::new();
        first.adopt(&root);
        first.transfer(&second, &root);
        let mut closing = first.begin_close(&[]);
        assert!(first.next_destroyed(&mut closing).is_none());
        // A cached seen-set must not suppress the same objects in a later walk.
        let mut preserved = second.begin_close(std::slice::from_ref(&root));
        assert!(second.next_destroyed(&mut preserved).is_none());
        let third = Region::new();
        third.adopt(&root);
        let mut closing = third.begin_close(&[]);
        assert!(Rc::ptr_eq(&third.next_destroyed(&mut closing).unwrap(), &b));
        assert!(Rc::ptr_eq(&third.next_destroyed(&mut closing).unwrap(), &a));
        assert!(third.next_destroyed(&mut closing).is_none());
    }

    #[test]
    fn builders_collect_in_place_and_finish_into_arrays() {
        let heap = ArrayHeap;
        let mut many = heap.builder_new();
        for _ in 0..scaled(100_000) {
            many = heap.builder_push(&many, Value::Byte(7)).unwrap();
        }
        let array = heap.builder_finish(&many).unwrap();
        assert_eq!(heap.elements(&array).unwrap().len(), scaled(100_000));
    }

    #[test]
    fn deep_chains_are_released_without_recursion() {
        let objects = ObjectHeap;
        let arrays = ArrayHeap;
        let mut chain = Value::Int(IntValue::from_i64(0));
        for _ in 0..scaled(1_000_000) {
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
        for _ in 0..scaled(20_000) {
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
    #[test]
    fn floats_compare_numerically_at_every_depth() {
        // #314: as scalar `==`, so `-0.0 == 0.0` inside composites too.
        let arrays = ArrayHeap;
        let heap = ObjectHeap;
        let (negative, positive) = (Value::Float(-0.0), Value::Float(0.0));
        assert!(heap.equal(&arrays, &negative, &positive).unwrap());
        let a = arrays.allocate(vec![arrays.allocate(vec![negative.clone()])]);
        let b = arrays.allocate(vec![arrays.allocate(vec![positive.clone()])]);
        assert!(heap.equal(&arrays, &a, &b).unwrap());
        let x = heap.allocate(0, vec![negative.clone(), Value::Float(1.5)]);
        let y = heap.allocate(0, vec![positive, Value::Float(1.5)]);
        let z = heap.allocate(0, vec![negative, Value::Float(2.5)]);
        assert!(heap.equal(&arrays, &x, &y).unwrap());
        assert!(!heap.equal(&arrays, &x, &z).unwrap());
    }
}

#[cfg(test)]
mod drop_tests {
    use super::*;
    use std::cell::RefCell;

    /// Build and drop on a 2 MiB stack: a recursive drop of these depths
    /// overflows it (S07, #28).
    fn drops_on_a_small_stack(build: fn(usize) -> Value) {
        std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || drop(build(scaled(200_000))))
            .unwrap()
            .join()
            .expect("dropped without overflowing the stack");
    }

    fn leaf() -> Value {
        Value::Int(crate::IntValue::from_i64(7))
    }

    /// Versions that share nodes (#335) hold plain values, and a plain value
    /// may be a version too: a chain of them nested one in the next still
    /// drops without recursion.
    #[test]
    fn nested_shared_versions_drop_iteratively() {
        fn nested(build: fn(usize) -> Value) {
            std::thread::Builder::new()
                .stack_size(1 << 20)
                .spawn(move || drop(build(scaled(30_000))))
                .unwrap()
                .join()
                .expect("dropped without overflowing the stack");
        }
        nested(|depth| {
            let int = |i: i64| Value::Int(crate::IntValue::from_i64(i));
            let base = ArrayHeap.allocate((0..100).map(int).collect());
            let Value::Array(marked) = base else { unreachable!() };
            let Value::Array(trie) = marked.replaced(1, int(1)) else { unreachable!() };
            let Value::Array(trie) = trie.replaced(1, int(1)) else { unreachable!() };
            assert!(matches!(trie.0, Repr::Trie { .. }));
            (0..depth).fold(leaf(), |next, _| trie.replaced(0, next))
        });
        nested(|depth| {
            let int = |i: i64| Value::Int(crate::IntValue::from_i64(i));
            let base = maps::from_entries((0..100).map(|i| (int(i), int(i)))).unwrap();
            (0..depth).fold(leaf(), |next, _| {
                let one = maps::from_entries([(int(0), next)]).unwrap();
                crate::ops::map_merge(&[&base, &one]).unwrap()
            })
        });
    }

    #[test]
    fn chains_drop_iteratively() {
        drops_on_a_small_stack(|depth| (0..depth).fold(leaf(), |next, _| ObjectHeap.allocate(0, vec![next, leaf()])));
        drops_on_a_small_stack(|depth| (0..depth).fold(leaf(), |next, _| ArrayHeap.allocate(vec![next])));
        // `any` is never boxed twice (`AnyBox` of an `any` keeps it): chains
        // go through containers.
        drops_on_a_small_stack(|depth| {
            (0..depth).fold(leaf(), |next, _| Value::Any(Rc::new(ArrayHeap.allocate(vec![next]))))
        });
        drops_on_a_small_stack(|depth| {
            (0..depth).fold(leaf(), |next, _| {
                Value::Fn(Rc::new(Closure { function: 0, captures: vec![next], worker: None }))
            })
        });
        drops_on_a_small_stack(|depth| {
            let chain = (0..depth).fold(leaf(), |next, _| ArrayHeap.allocate(vec![next]));
            Value::Builder(Rc::new(RefCell::new(vec![chain.clone(), chain])))
        });
    }

    #[test]
    fn graphs_with_repeated_edges_drop_iteratively() {
        drops_on_a_small_stack(|depth| (0..depth).fold(leaf(), |next, _| ArrayHeap.allocate(vec![next.clone(), next])));
        drops_on_a_small_stack(|depth| {
            (0..depth).fold(leaf(), |next, _| ObjectHeap.allocate(0, vec![next.clone(), leaf(), next]))
        });
        drops_on_a_small_stack(|depth| {
            (0..depth).fold(leaf(), |next, _| {
                let entries = (0..2).map(|key| (MapKey::Int(crate::IntValue::from_i64(key)), next.clone())).collect();
                drop(next);
                Value::Map(Rc::new(Map::new(entries)))
            })
        });
        drops_on_a_small_stack(|depth| {
            (0..depth).fold(leaf(), |next, _| {
                Value::Fn(Rc::new(Closure { function: 0, captures: vec![next.clone(), next], worker: None }))
            })
        });
    }
}
