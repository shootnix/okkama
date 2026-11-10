//! Сборщик циклов ссылок, общий для исполнителей IR и Sparrow.
//!
//! Значения — счётчики ссылок `Rc`: недостижимое освобождается сразу. Цикл
//! замыкает только ребро, записанное после создания узла, — позднее поле
//! (`obj.init`), состояние intrinsic объекта, исход Future, причина и
//! вторичные отказы `Failure`, содержимое builder. Неизменяемые рёбра ведут
//! к более старым узлам и цикла не образуют.
//!
//! Алгоритм — пробное удаление (trial deletion, как сборщик циклов CPython):
//!
//! 1. Кандидаты — узлы, получившие позднее ребро (слабые ссылки).
//! 2. Подграф — узлы, достижимые из кандидатов по точно перечисленным сильным
//!    рёбрам; неизвестные держатели (кадры исполнителя, корутины, замыкания
//!    рантайма, регионы, handles) не перечисляются.
//! 3. Внешние ссылки узла — `strong_count` минус ссылки изнутри подграфа и
//!    наша копия. Узел с внешними ссылками — корень; достижимое из корней живо.
//! 4. У остальных (мусорных) узлов очищаются поздние рёбра: цикл разорван,
//!    и обычное освобождение `Rc` завершает работу.
//!
//! Неперечисленное ребро только делает цель «внешне достижимой», поэтому
//! сборщик консервативен: он может не найти мусор, но не освободит живое.
//! Перечисленное ребро обязано быть ровно одной сильной ссылкой.
//!
//! Темп: сборка, когда новых кандидатов не меньше `max(MIN, выживших,
//! узлов прошлой сборки / 8)` — амортизированно O(1) работы на кандидата.
//! `OKKAMA_GC=stress` собирает на каждом новом кандидате (#43): ошибка
//! перечисления рёбер проявляется сразу, а не через тысячи кандидатов.
//!
//! Проверки debug-сборки: ни один узел не получает больше перечисленных
//! рёбер, чем у него сильных ссылок, и каждый узел, признанный мусором,
//! действительно освобождается после разрыва поздних рёбер. Нарушение любой
//! из них значит, что перечисление насчитало ребро, которого нет, — то есть
//! сборщик мог очистить живой узел.
use crate::Value;
use crate::failure::{Failure, Kind};
use crate::heap::{Array, ArrayBuilder, Buffer, Closure, Map, MapBuilder, Object, Storage};
use crate::sched::FutureCell;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::{Rc, Weak};

/// Хеш адреса узла: умножение Фибоначчи, без SipHash.
#[derive(Default)]
struct Address(u64);

impl Hasher for Address {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {
        unreachable!("addresses are hashed as usize")
    }
    fn write_usize(&mut self, value: usize) {
        self.0 = (value as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_right(29);
    }
}

type Index = HashMap<usize, usize, BuildHasherDefault<Address>>;

/// Наименьший порог новых кандидатов между сборками. Под Miri порог мал:
/// тесты доходят до сборок на сотнях узлов, а не на тысячах (#43).
const MIN_THRESHOLD: usize = if cfg!(miri) { 64 } else { 4096 };

enum Candidate {
    Object(Weak<Object>),
    Future(Weak<FutureCell>),
    /// Буфер массивов, записанный на месте (#335).
    Buffer(Weak<Buffer>),
}

struct Collector {
    candidates: RefCell<Vec<Candidate>>,
    fresh: Cell<usize>,
    threshold: Cell<usize>,
    collecting: Cell<bool>,
    enabled: Cell<bool>,
    /// Собирать на каждом кандидате (`OKKAMA_GC=stress`).
    stress: Cell<bool>,
    collections: Cell<u64>,
    freed: Cell<u64>,
}

thread_local! {
    static GC: Collector = Collector {
        candidates: RefCell::new(Vec::new()),
        fresh: Cell::new(0),
        threshold: Cell::new(MIN_THRESHOLD),
        collecting: Cell::new(false),
        enabled: Cell::new(std::env::var_os("OKKAMA_GC").is_none_or(|value| value != "off")),
        stress: Cell::new(std::env::var_os("OKKAMA_GC").is_some_and(|value| value == "stress")),
        collections: Cell::new(0),
        freed: Cell::new(0),
    };
}

/// Значение, которое может участвовать в цикле (узел подграфа).
pub fn traced(value: &Value) -> bool {
    Node::of(value).is_some()
}

fn track(candidate: Candidate) {
    let due = GC.with(|gc| {
        if !gc.enabled.get() {
            return false;
        }
        gc.candidates.borrow_mut().push(candidate);
        gc.fresh.set(gc.fresh.get() + 1);
        !gc.collecting.get() && (gc.stress.get() || gc.fresh.get() >= gc.threshold.get())
    });
    if due {
        collect();
    }
}

/// Объект получил позднее поле или состояние intrinsic.
pub fn track_object(object: &Rc<Object>) {
    track(Candidate::Object(Rc::downgrade(object)));
}

/// Future получил исход.
pub fn track_future(future: &Rc<FutureCell>) {
    track(Candidate::Future(Rc::downgrade(future)));
}

/// Буфер массивов получил значение после создания (#335).
pub fn track_buffer(buffer: &Rc<Buffer>) {
    track(Candidate::Buffer(Rc::downgrade(buffer)));
}

/// Включить или выключить сборщик (`OKKAMA_GC=off` выключает его при старте).
pub fn set_enabled(enabled: bool) {
    GC.with(|gc| {
        gc.enabled.set(enabled);
        if !enabled {
            gc.candidates.borrow_mut().clear();
            gc.fresh.set(0);
        }
    });
}

/// Собирать на каждом новом кандидате (как `OKKAMA_GC=stress`).
pub fn set_stress(stress: bool) {
    GC.with(|gc| gc.stress.set(stress));
}

/// Статистика: (число сборок, освобождённых узлов-мусора).
pub fn stats() -> (u64, u64) {
    GC.with(|gc| (gc.collections.get(), gc.freed.get()))
}

/// Узел подграфа: копия сильной ссылки на разделяемое значение.
#[derive(Clone)]
pub(crate) enum Node {
    Object(Rc<Object>),
    Array(Rc<Array>),
    Buffer(Rc<Buffer>),
    Map(Rc<Map>),
    Fn(Rc<Closure>),
    Any(Rc<Value>),
    Future(Rc<FutureCell>),
    Failure(Rc<Failure>),
    Builder(Rc<ArrayBuilder>),
    MapBuilder(Rc<MapBuilder>),
    State(Rc<dyn Any>),
}

impl Node {
    fn of(value: &Value) -> Option<Self> {
        Some(match value {
            Value::Object(object) => Self::Object(object.clone()),
            // Массив простых значений цикла не замкнёт (#335).
            Value::Array(array) if array.holds_nodes() => Self::Array(array.clone()),
            Value::Map(map) if !map.0.is_empty() => Self::Map(map.clone()),
            Value::Fn(closure) => Self::Fn(closure.clone()),
            Value::Any(inner) => Self::Any(inner.clone()),
            Value::Future(future) => Self::Future(future.clone()),
            Value::Failure(failure) => Self::Failure(failure.clone()),
            Value::Builder(builder) => Self::Builder(builder.clone()),
            Value::MapBuilder(builder) => Self::MapBuilder(builder.clone()),
            _ => return None,
        })
    }

    fn key(&self) -> usize {
        match self {
            Self::Object(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::Array(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::Buffer(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::Map(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::Fn(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::Any(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::Future(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::Failure(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::Builder(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::MapBuilder(rc) => Rc::as_ptr(rc) as *const () as usize,
            Self::State(rc) => Rc::as_ptr(rc) as *const () as usize,
        }
    }

    fn strong(&self) -> usize {
        match self {
            Self::Object(rc) => Rc::strong_count(rc),
            Self::Array(rc) => Rc::strong_count(rc),
            Self::Buffer(rc) => Rc::strong_count(rc),
            Self::Map(rc) => Rc::strong_count(rc),
            Self::Fn(rc) => Rc::strong_count(rc),
            Self::Any(rc) => Rc::strong_count(rc),
            Self::Future(rc) => Rc::strong_count(rc),
            Self::Failure(rc) => Rc::strong_count(rc),
            Self::Builder(rc) => Rc::strong_count(rc),
            Self::MapBuilder(rc) => Rc::strong_count(rc),
            Self::State(rc) => Rc::strong_count(rc),
        }
    }

    /// Сильные рёбра узла; `false`, если состояние сейчас заимствовано.
    fn edges(&self, out: &mut Edges) -> bool {
        match self {
            Self::Object(object) => {
                object.fixed().iter().for_each(|value| out.value(value));
                let Ok(extra) = object.extra().try_borrow() else {
                    return false;
                };
                if let Some(extra) = extra.as_ref() {
                    extra.late.iter().for_each(|(_, value)| out.value(value));
                    if let Some(state) = &extra.intrinsic {
                        out.0.push(Self::State(state.clone()));
                    }
                }
            }
            // Версия держит свой буфер, срез — источник: ровно одно ребро.
            Self::Array(array) => match array.storage() {
                Storage::Items(items) => items.iter().for_each(|value| out.value(value)),
                Storage::Buffer(buffer) => out.0.push(Self::Buffer(buffer.clone())),
                Storage::Origin(origin) => out.0.push(Self::Array(origin.clone())),
            },
            Self::Buffer(buffer) => {
                let Some(written) = buffer.written() else {
                    return false;
                };
                written.iter().for_each(|value| out.value(value));
            }
            Self::Map(map) => {
                // Ключ-объект держит объект (#212).
                for (key, value) in map.0.iter() {
                    if let crate::heap::MapKey::Object { object, .. } = key {
                        out.object(object);
                    }
                    out.value(value);
                }
            }
            Self::Fn(closure) => {
                closure.captures.iter().for_each(|value| out.value(value));
                if let Some(bound) = &closure.worker {
                    out.value(&bound.entry);
                    bound.fields.iter().for_each(|value| out.value(value));
                }
            }
            Self::Any(inner) => out.value(inner),
            Self::Future(future) => return future.edges(out),
            Self::Failure(failure) => {
                if let Kind::Thrown { object, .. } = &failure.kind {
                    out.0.push(Self::Object(object.0.clone()));
                }
                let (Ok(secondary), Ok(cause)) = (failure.secondary.try_borrow(), failure.cause.try_borrow()) else {
                    return false;
                };
                secondary.iter().for_each(|failure| out.failure(failure));
                cause.iter().for_each(|failure| out.failure(failure));
            }
            Self::Builder(builder) => {
                let Ok(values) = builder.try_borrow() else {
                    return false;
                };
                values.iter().for_each(|value| out.value(value));
            }
            Self::MapBuilder(builder) => {
                let Ok(values) = builder.try_borrow() else {
                    return false;
                };
                values.values().for_each(|value| out.value(value));
            }
            Self::State(state) => return crate::intrinsics::state_edges(&**state, out),
        }
        true
    }

    /// Разорвать поздние рёбра мусорного узла; отнятое — в `sink`.
    fn clear(&self, sink: &mut Vec<Value>, states: &mut Vec<Rc<dyn Any>>, failures: &mut Vec<Rc<Failure>>) {
        match self {
            Self::Object(object) => {
                if let Ok(mut extra) = object.extra().try_borrow_mut()
                    && let Some(extra) = extra.take()
                {
                    sink.extend(extra.late.into_iter().map(|(_, value)| value));
                    states.extend(extra.intrinsic);
                }
            }
            Self::Future(future) => future.clear(sink, failures),
            Self::Failure(failure) => {
                if let Ok(mut secondary) = failure.secondary.try_borrow_mut() {
                    failures.append(&mut secondary);
                }
                if let Ok(mut cause) = failure.cause.try_borrow_mut() {
                    failures.extend(cause.take());
                }
            }
            Self::Builder(builder) => {
                if let Ok(mut values) = builder.try_borrow_mut() {
                    sink.append(&mut values);
                }
            }
            // Записанное после создания буфера — поздние рёбра.
            Self::Buffer(buffer) => buffer.clear(sink),
            Self::MapBuilder(builder) => {
                if let Ok(mut values) = builder.try_borrow_mut() {
                    sink.extend(std::mem::take(&mut *values).into_values());
                }
            }
            Self::Array(_) | Self::Map(_) | Self::Fn(_) | Self::Any(_) | Self::State(_) => {}
        }
    }
}

/// Слабая ссылка на узел: проверка debug-сборки, что мусор освобождён.
#[cfg(debug_assertions)]
enum Freed {
    Object(Weak<Object>),
    Array(Weak<Array>),
    Buffer(Weak<Buffer>),
    Map(Weak<Map>),
    Fn(Weak<Closure>),
    Any(Weak<Value>),
    Future(Weak<FutureCell>),
    Failure(Weak<Failure>),
    Builder(Weak<ArrayBuilder>),
    MapBuilder(Weak<MapBuilder>),
    State(Weak<dyn Any>),
}

#[cfg(debug_assertions)]
impl Freed {
    fn of(node: &Node) -> Self {
        match node {
            Node::Object(rc) => Self::Object(Rc::downgrade(rc)),
            Node::Array(rc) => Self::Array(Rc::downgrade(rc)),
            Node::Buffer(rc) => Self::Buffer(Rc::downgrade(rc)),
            Node::Map(rc) => Self::Map(Rc::downgrade(rc)),
            Node::Fn(rc) => Self::Fn(Rc::downgrade(rc)),
            Node::Any(rc) => Self::Any(Rc::downgrade(rc)),
            Node::Future(rc) => Self::Future(Rc::downgrade(rc)),
            Node::Failure(rc) => Self::Failure(Rc::downgrade(rc)),
            Node::Builder(rc) => Self::Builder(Rc::downgrade(rc)),
            Node::MapBuilder(rc) => Self::MapBuilder(Rc::downgrade(rc)),
            Node::State(rc) => Self::State(Rc::downgrade(rc)),
        }
    }

    fn alive(&self) -> bool {
        match self {
            Self::Object(weak) => weak.strong_count() > 0,
            Self::Array(weak) => weak.strong_count() > 0,
            Self::Buffer(weak) => weak.strong_count() > 0,
            Self::Map(weak) => weak.strong_count() > 0,
            Self::Fn(weak) => weak.strong_count() > 0,
            Self::Any(weak) => weak.strong_count() > 0,
            Self::Future(weak) => weak.strong_count() > 0,
            Self::Failure(weak) => weak.strong_count() > 0,
            Self::Builder(weak) => weak.strong_count() > 0,
            Self::MapBuilder(weak) => weak.strong_count() > 0,
            Self::State(weak) => weak.strong_count() > 0,
        }
    }
}

/// Приёмник рёбер обхода.
pub(crate) struct Edges(Vec<Node>);

impl Edges {
    pub(crate) fn value(&mut self, value: &Value) {
        if let Some(node) = Node::of(value) {
            self.0.push(node);
        }
    }

    pub(crate) fn object(&mut self, object: &Rc<Object>) {
        self.0.push(Node::Object(object.clone()));
    }

    pub(crate) fn future(&mut self, future: &Rc<FutureCell>) {
        self.0.push(Node::Future(future.clone()));
    }

    pub(crate) fn failure(&mut self, failure: &Rc<Failure>) {
        self.0.push(Node::Failure(failure.clone()));
    }
}

/// Собрать мусорные циклы сейчас; результат — число освобождённых узлов.
pub fn collect() -> usize {
    let started = GC.with(|gc| {
        if gc.collecting.get() || !gc.enabled.get() {
            return None;
        }
        gc.collecting.set(true);
        gc.fresh.set(0);
        Some(std::mem::take(&mut *gc.candidates.borrow_mut()))
    });
    let Some(candidates) = started else {
        return 0;
    };
    let (garbage, nodes, survivors) = sweep(candidates);
    GC.with(|gc| {
        let mut kept = gc.candidates.borrow_mut();
        // Кандидаты, добавленные во время освобождения, сохраняются.
        let added = std::mem::replace(&mut *kept, survivors);
        kept.extend(added);
        gc.threshold.set(MIN_THRESHOLD.max(kept.len()).max(nodes / 8));
        gc.collections.set(gc.collections.get() + 1);
        gc.freed.set(gc.freed.get() + garbage as u64);
        gc.collecting.set(false);
    });
    garbage
}

/// Пробное удаление над подграфом кандидатов: (мусор, узлов, выжившие кандидаты).
fn sweep(candidates: Vec<Candidate>) -> (usize, usize, Vec<Candidate>) {
    let mut index = Index::default();
    let mut nodes: Vec<Node> = Vec::new();
    let mut candidate_nodes = Vec::new();
    for candidate in &candidates {
        let node = match candidate {
            Candidate::Object(weak) => weak.upgrade().map(Node::Object),
            Candidate::Future(weak) => weak.upgrade().map(Node::Future),
            Candidate::Buffer(weak) => weak.upgrade().map(Node::Buffer),
        };
        let Some(node) = node else { continue };
        let key = node.key();
        let at = *index.entry(key).or_insert_with(|| {
            nodes.push(node);
            nodes.len() - 1
        });
        candidate_nodes.push(at);
    }
    drop(candidates);
    candidate_nodes.sort_unstable();
    candidate_nodes.dedup();

    // Обход: рёбра узла `n` — номера целей `targets[starts[n]..starts[n + 1]]`
    // (с кратностью); `opaque` — узел, чьи рёбра не удалось перечислить: корень.
    let mut starts: Vec<usize> = vec![0];
    let mut targets: Vec<usize> = Vec::new();
    let mut opaque: Vec<bool> = Vec::new();
    let mut edges = Edges(Vec::new());
    let mut next = 0;
    while next < nodes.len() {
        let complete = nodes[next].edges(&mut edges);
        for node in edges.0.drain(..) {
            let key = node.key();
            let at = match index.get(&key) {
                Some(&at) => at,
                None => {
                    nodes.push(node);
                    index.insert(key, nodes.len() - 1);
                    nodes.len() - 1
                }
            };
            targets.push(at);
        }
        starts.push(targets.len());
        opaque.push(!complete);
        next += 1;
    }
    drop(index);

    // Внешние ссылки: всё, кроме нашей копии и рёбер подграфа.
    let mut external: Vec<isize> = nodes.iter().map(|node| node.strong() as isize - 1).collect();
    for &target in &targets {
        external[target] -= 1;
    }
    debug_assert!(external.iter().all(|&count| count >= 0), "gc: edge count exceeds strong count");
    let mut live = vec![false; nodes.len()];
    let mut pending: Vec<usize> = (0..nodes.len()).filter(|&at| external[at] != 0 || opaque[at]).collect();
    for &at in &pending {
        live[at] = true;
    }
    while let Some(at) = pending.pop() {
        for &target in &targets[starts[at]..starts[at + 1]] {
            if !live[target] {
                live[target] = true;
                pending.push(target);
            }
        }
    }
    drop(targets);

    let survivors = candidate_nodes
        .iter()
        .filter(|&&at| live[at])
        .map(|&at| match &nodes[at] {
            Node::Object(object) => Candidate::Object(Rc::downgrade(object)),
            Node::Future(future) => Candidate::Future(Rc::downgrade(future)),
            Node::Buffer(buffer) => Candidate::Buffer(Rc::downgrade(buffer)),
            _ => unreachable!("candidates are objects, futures and buffers"),
        })
        .collect();

    let mut sink = Vec::new();
    let mut states = Vec::new();
    let mut failures = Vec::new();
    let mut garbage = 0;
    #[cfg(debug_assertions)]
    let mut freed = Vec::new();
    for (at, node) in nodes.iter().enumerate() {
        if !live[at] {
            node.clear(&mut sink, &mut states, &mut failures);
            garbage += 1;
            #[cfg(debug_assertions)]
            freed.push(Freed::of(node));
        }
    }
    let count = nodes.len();
    // Сначала копии узлов, затем разорванные рёбра: последняя ссылка уходит
    // в обычное итеративное освобождение.
    drop(nodes);
    drop(states);
    drop(failures);
    crate::heap::release_values(sink);
    #[cfg(debug_assertions)]
    assert!(
        freed.iter().all(|node| !node.alive()),
        "gc: a node found garbage outlived its broken edges (an enumerated edge is not a strong reference)"
    );
    (garbage, count, survivors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heap::{ArrayHeap, ObjectHeap};

    fn pair() -> (Value, Rc<Object>) {
        // Owner(child: Child(owner: self)): поле owner связано позже.
        let owner = ObjectHeap.allocate(0, vec![Value::Undef]);
        let child = ObjectHeap.allocate(1, vec![owner.clone()]);
        ObjectHeap.init(&owner, 0, child).unwrap();
        let Value::Object(object) = &owner else { unreachable!() };
        let object = object.clone();
        (owner, object)
    }

    #[test]
    fn unreachable_cycles_are_freed_and_reachable_ones_kept() {
        let (live, _) = pair();
        let (dead, object) = pair();
        let weak = Rc::downgrade(&object);
        drop(object);
        drop(dead);
        assert!(weak.upgrade().is_some(), "a cycle survives plain reference counting");
        collect();
        assert!(weak.upgrade().is_none(), "the unreachable cycle is freed");
        let Value::Object(owner) = &live else { unreachable!() };
        let Some(Value::Object(child)) = owner.field(0) else {
            panic!("live cycle lost its field")
        };
        assert!(matches!(child.field(0), Some(Value::Object(back)) if Rc::ptr_eq(&back, owner)));
        // Garbage once dropped: collected, so nothing outlives the test (Miri checks leaks).
        drop(child);
        drop(live);
        collect();
    }

    /// An append written in place gives an old buffer a new edge (#335): the
    /// element can hold a version of that buffer, a cycle no flat copy makes.
    #[test]
    fn a_cycle_through_a_buffer_written_in_place_is_freed() {
        use crate::ops::array_concat;
        let start: Vec<Value> = (0..40).map(|i| Value::Int(crate::IntValue::from_i64(i))).collect();
        let first = array_concat(&[&ArrayHeap.allocate(start), &ArrayHeap.allocate(vec![])]).unwrap();
        // Two appends: a buffer, then a version written in place.
        let second = array_concat(&[&first, &ArrayHeap.allocate(vec![Value::Undef])]).unwrap();
        let (holder, object) = object(1);
        let third = array_concat(&[&second, &ArrayHeap.allocate(vec![holder.clone()])]).unwrap();
        // The object holds the version before it: holder -> second -> buffer -> holder.
        ObjectHeap.init(&holder, 0, second.clone()).unwrap();
        let weak = Rc::downgrade(&object);
        drop((first, second, third, holder, object));
        assert!(weak.upgrade().is_some(), "the cycle survives plain reference counting");
        collect();
        assert!(weak.upgrade().is_none(), "the cycle through the buffer is freed");
    }

    #[test]
    fn cycles_held_by_unknown_owners_are_conservatively_kept() {
        let (value, object) = pair();
        let weak = Rc::downgrade(&object);
        // Внешний держатель, чьи рёбра сборщик не перечисляет.
        let opaque: Rc<dyn Any> = Rc::new(value);
        drop(object);
        collect();
        assert!(weak.upgrade().is_some());
        drop(opaque);
        collect();
        assert!(weak.upgrade().is_none());
    }

    fn object(fields: usize) -> (Value, Rc<Object>) {
        let value = ObjectHeap.allocate(0, vec![Value::Undef; fields]);
        let Value::Object(object) = &value else { unreachable!() };
        let object = object.clone();
        (value, object)
    }

    fn live_objects() -> usize {
        crate::heap::LIVE_OBJECTS.with(Cell::get)
    }

    fn thrown(object: &Rc<Object>) -> Rc<Failure> {
        Failure::new(Kind::Thrown { object: crate::heap::Ref(object.clone()), name: "Error".into(), message: None })
    }

    /// A named way to build a cycle; the result is its only outside holder.
    type Cycle = (&'static str, fn() -> Value);

    /// A cycle closed by each kind of late edge.
    fn cycles() -> Vec<Cycle> {
        vec![
            ("late field", || {
                let (value, _) = object(1);
                ObjectHeap.init(&value, 0, value.clone()).unwrap();
                value
            }),
            ("future outcome", || {
                // Future → its value object → late field → the Future.
                let future = crate::sched::promise();
                let (value, _) = object(1);
                ObjectHeap.init(&value, 0, Value::Future(future.clone())).unwrap();
                crate::sched::complete(&future, Ok(value.clone()));
                value
            }),
            ("future failure", || {
                let future = crate::sched::promise();
                let (value, holder) = object(1);
                ObjectHeap.init(&value, 0, Value::Future(future.clone())).unwrap();
                crate::sched::complete(&future, Err(thrown(&holder)));
                value
            }),
            ("thrown object", || {
                let (value, holder) = object(1);
                ObjectHeap.init(&value, 0, Value::Failure(thrown(&holder))).unwrap();
                value
            }),
            ("failure cause and secondary", || {
                // later → cause → earlier → thrown object → late field → later.
                let (value, holder) = object(1);
                let earlier = thrown(&holder);
                let later = Failure::new(Kind::Panic("later".into()));
                *later.cause.borrow_mut() = Some(earlier.clone());
                later.secondary.borrow_mut().push(earlier);
                ObjectHeap.init(&value, 0, Value::Failure(later)).unwrap();
                value
            }),
            ("closure capture", || {
                let (value, _) = object(1);
                let closure = Rc::new(Closure { function: 0, captures: vec![value.clone()], worker: None });
                ObjectHeap.init(&value, 0, Value::Fn(closure)).unwrap();
                value
            }),
            ("array builder", || {
                let (value, _) = object(1);
                let builder: Rc<ArrayBuilder> = Rc::new(RefCell::new(vec![value.clone()]));
                ObjectHeap.init(&value, 0, Value::Builder(builder)).unwrap();
                value
            }),
            ("map builder", || {
                let (value, _) = object(1);
                let key = crate::heap::MapKey::Int(crate::IntValue::from_i64(1));
                let builder: Rc<MapBuilder> = Rc::new(RefCell::new([(key, value.clone())].into()));
                ObjectHeap.init(&value, 0, Value::MapBuilder(builder)).unwrap();
                value
            }),
            ("intrinsic state", || {
                // A handle's state that holds its own object.
                let (value, holder) = object(0);
                holder.set_intrinsic(Rc::new(value.clone()));
                value
            }),
            ("any box and array", || {
                let (value, _) = object(1);
                let inner = crate::heap::ArrayHeap.allocate(vec![Value::Any(Rc::new(value.clone()))]);
                ObjectHeap.init(&value, 0, inner).unwrap();
                value
            }),
        ]
    }

    #[test]
    fn every_kind_of_late_edge_closes_a_collectable_cycle() {
        for stress in [false, true] {
            set_stress(stress);
            for (name, cycle) in cycles() {
                collect();
                let base = live_objects();
                for _ in 0..3 {
                    let value = cycle();
                    let Value::Object(object) = &value else { unreachable!() };
                    let weak = Rc::downgrade(object);
                    // Held from outside: kept with every edge intact.
                    collect();
                    assert!(weak.upgrade().is_some_and(|object| object.extra().borrow().is_some()), "{name}: a held cycle was cleared");
                    drop(value);
                    assert!(weak.upgrade().is_some(), "{name}: reference counting alone frees a cycle");
                    collect();
                    assert!(weak.upgrade().is_none(), "{name}: the unreachable cycle survives");
                }
                assert_eq!(live_objects(), base, "{name}: objects outlive repeated cycles");
            }
        }
        set_stress(false);
    }

    #[test]
    fn stress_mode_collects_at_every_candidate_without_losing_live_values() {
        set_stress(true);
        let (before, _) = stats();
        let kept: Vec<Value> = (0..64)
            .map(|_| {
                let (value, _) = object(1);
                ObjectHeap.init(&value, 0, value.clone()).unwrap();
                value
            })
            .collect();
        for _ in 0..64 {
            let (value, _) = object(1);
            ObjectHeap.init(&value, 0, value.clone()).unwrap();
        }
        set_stress(false);
        assert!(stats().0 - before >= 128, "a collection per candidate");
        for value in &kept {
            let Value::Object(object) = value else { unreachable!() };
            assert!(matches!(object.field(0), Some(Value::Object(back)) if Rc::ptr_eq(&back, object)));
        }
        drop(kept);
        collect();
    }

    #[test]
    fn many_cycles_are_collected_incrementally() {
        let (before, _) = stats();
        let mut probe = None;
        for _ in 0..(MIN_THRESHOLD * 3) {
            let (value, object) = pair();
            probe.get_or_insert_with(|| Rc::downgrade(&object));
            drop(value);
        }
        assert!(stats().0 > before);
        assert!(probe.unwrap().upgrade().is_none());
        // The cycles since the last threshold are still garbage.
        collect();
    }
}
