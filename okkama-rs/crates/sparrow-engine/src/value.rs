//! New scalar storage and exact integers; no dependency on legacy VM values.
use crate::Error;
use crate::arena::Arena;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use std::borrow::Cow;
use std::rc::Rc;

pub(crate) const SMALL: u64 = 0;
pub(crate) const BIG: u64 = 1;
pub(crate) const BOOL: u64 = 2;
pub(crate) const FLOAT: u64 = 3;
pub(crate) const BYTE: u64 = 4;
pub(crate) const CHAR: u64 = 5;
pub(crate) const RAT: u64 = 6;
pub(crate) const SMALL_RAT: u64 = 7;
pub(crate) const OBJECT: u64 = 8;
pub(crate) const UNBOUND: u64 = 9;
pub(crate) const STR: u64 = 10;
pub(crate) const ARRAY: u64 = 11;
pub(crate) const BUILDER: u64 = 12;
pub(crate) const RANGE: u64 = 13;
pub(crate) const MAP: u64 = 14;
pub(crate) const MAP_BUILDER: u64 = 15;
pub(crate) const ENUM: u64 = 16;
pub(crate) const CLOSURE: u64 = 17;
pub(crate) const ANY: u64 = 18;
pub(crate) const FAILURE: u64 = 19;
pub(crate) const REGION: u64 = 20;
pub(crate) const FUTURE: u64 = 21;
pub(crate) const NATIVE_FAILURE: u64 = 22;
pub(crate) const CANCELLED: u64 = 23;
pub(crate) const HANDLE: u64 = 24;
pub(crate) const JSON_NODE: u64 = 25;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub(crate) struct Slot {
    pub bits: u64,
    pub tag: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn object_collection_is_iterative_and_reclaims_deep_graphs_and_cycles() {
        let mut heap = Heap::new(&[], &[]);
        let mut root = Slot::small(17);
        for _ in 0..100_000 {
            root = heap.object_new(0, [root].into_iter()).unwrap();
        }
        heap.collect([root].into_iter()).unwrap();
        let mut cursor = root;
        for _ in 0..100_000 {
            cursor = heap.object_get(cursor, 0).unwrap();
        }
        assert_eq!(cursor.bits, 17);
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.object_get(root, 0).is_err());
        for _ in 0..1000 {
            heap.object_new(0, [Slot::small(0)].into_iter()).unwrap();
        }
        assert_eq!(heap.objects.len(), 100_000);

        let mut cycle = Heap::new(&[], &[]);
        // Internal fixture for late-bound cycles; bytecode late initialization
        // is a separate feature. The first arena handle is known to be zero.
        let self_ref = Slot {
            bits: 0,
            tag: OBJECT,
        };
        let object = cycle.object_new(0, [self_ref].into_iter()).unwrap();
        assert_eq!(object.bits, self_ref.bits);
        cycle.collect([object].into_iter()).unwrap();
        assert_eq!(cycle.object_get(object, 0).unwrap().bits, object.bits);
        cycle.collect(std::iter::empty()).unwrap();
        assert!(cycle.object_get(object, 0).is_err());
        let reused = cycle.object_new(0, [].into_iter()).unwrap();
        assert_ne!(reused.bits, object.bits);
    }

    #[test]
    fn numeric_collection_is_bounded_and_rejects_stale_handles() {
        let mut heap = Heap::new(&[], &[]);
        let held = heap
            .arithmetic(Arithmetic::Multiply, Slot::small(i64::MAX), Slot::small(2))
            .unwrap();
        let mut current = held;
        let stale = heap
            .arithmetic(Arithmetic::Add, held, Slot::small(1))
            .unwrap();
        heap.collect([held].into_iter()).unwrap();
        assert!(heap.integer(stale).is_err());
        for _ in 0..5000 {
            current = heap
                .arithmetic(Arithmetic::Add, current, Slot::small(1))
                .unwrap();
            if heap.needs_collection() {
                heap.collect([held, current].into_iter()).unwrap();
            }
            assert!(heap.integers.len() <= 131);
        }
        assert_eq!(
            heap.integer(held).unwrap().to_string(),
            "18446744073709551614"
        );
        assert_eq!(
            heap.integer(current).unwrap().to_string(),
            "18446744073709556614"
        );
        assert!(heap.integer(stale).is_err());
        let exported = heap.integer(current).unwrap();
        heap.collect(std::iter::empty()).unwrap();
        assert_eq!(exported.to_string(), "18446744073709556614");
    }

    #[test]
    fn small_rationals_do_not_allocate_and_large_rationals_collect() {
        let mut heap = Heap::new(&[], &[]);
        let half = heap.rat_new(Slot::small(1), Slot::small(2)).unwrap();
        let mut sum = heap.rat_new(Slot::small(0), Slot::small(1)).unwrap();
        for _ in 0..10_000 {
            sum = heap
                .rat_binary(crate::rational::Binary::Add, sum, half)
                .unwrap();
        }
        assert_eq!(crate::rational::parts(sum), Some((5000, 1)));
        assert_eq!(heap.integers.len(), 0);
        assert_eq!(heap.rationals.len(), 0);
        let large = heap.rat_new(Slot::small(i64::MAX), Slot::small(1)).unwrap();
        let mut current = large;
        for _ in 0..1000 {
            current = heap
                .rat_binary(crate::rational::Binary::Add, current, half)
                .unwrap();
            if heap.needs_collection() {
                heap.collect([large, current].into_iter()).unwrap();
            }
            assert!(heap.rationals.len() <= 131);
        }
        assert_eq!(
            heap.ratio(current).unwrap().to_string(),
            "9223372036854776307"
        );
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.ratio(current).is_err());
    }

    #[test]
    fn rational_float_conversion_uses_decimal_contract_and_round_trips() {
        let mut heap = Heap::new(&[], &[]);
        let tenth = heap
            .convert(
                crate::scalar::Kind::Rat,
                crate::scalar::finite(0.1).unwrap(),
            )
            .unwrap();
        assert_eq!(crate::rational::parts(tenth), Some((1, 10)));
        for value in [
            0.1,
            -0.0,
            1e-300,
            f64::from_bits(1),
            f64::MAX,
            -1.2345678901234567,
        ] {
            let rational = heap
                .convert(
                    crate::scalar::Kind::Rat,
                    crate::scalar::finite(value).unwrap(),
                )
                .unwrap();
            let restored = heap.convert(crate::scalar::Kind::Float, rational).unwrap();
            assert_eq!(
                restored.bits,
                if value == 0.0 {
                    0.0f64.to_bits()
                } else {
                    value.to_bits()
                }
            );
        }
        for i in 1..1000u32 {
            let n = i.wrapping_mul(2654435761) as i32;
            let d = i.wrapping_mul(2246822519).max(1);
            let ratio = BigRational::new(BigInt::from(n), BigInt::from(d));
            assert_eq!(
                (f64::from(n) / f64::from(d)).to_bits(),
                crate::rational::to_float(&ratio).unwrap().to_bits()
            );
        }
    }
}
const _: () = assert!(std::mem::size_of::<Slot>() == 16);
impl Slot {
    pub fn small(value: i64) -> Self {
        Self {
            bits: value as u64,
            tag: SMALL,
        }
    }
    pub fn boolean(value: bool) -> Self {
        Self {
            bits: u64::from(value),
            tag: BOOL,
        }
    }
    pub fn big(handle: u64) -> Self {
        Self {
            bits: handle,
            tag: BIG,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Integer {
    Small(i64),
    Big(Rc<BigInt>),
}
impl From<i64> for Integer {
    fn from(value: i64) -> Self {
        Self::Small(value)
    }
}
impl PartialEq<i64> for Integer {
    fn eq(&self, other: &i64) -> bool {
        matches!(self, Self::Small(value) if value == other)
    }
}
impl std::fmt::Display for Integer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Small(value) => value.fmt(f),
            Self::Big(value) => value.fmt(f),
        }
    }
}
impl Integer {
    pub fn to_i64(&self) -> Option<i64> {
        match self {
            Self::Small(value) => Some(*value),
            Self::Big(value) => value.to_i64(),
        }
    }
    pub(crate) fn parse(text: &str) -> Result<Self, Error> {
        if let Ok(value) = text.parse::<i64>() {
            return Ok(Self::Small(value));
        }
        let value = text
            .parse::<BigInt>()
            .map_err(|_| Error("invalid integer constant".into()))?;
        Ok(Self::Big(Rc::new(value)))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Arithmetic {
    Add,
    Subtract,
    Multiply,
    Remainder,
    And,
    Or,
    Xor,
    ShiftLeft,
    ShiftRight,
    ShiftUnsigned,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Unary {
    Negate,
    Absolute,
    Not,
    Sign,
}

/// Separate typed arenas share the same generational-handle contract.
#[derive(Debug)]
pub(crate) struct Heap {
    pub(crate) stdin: Option<Box<crate::native_input::Stdin>>,
    pub(crate) arguments: Vec<Vec<u8>>,
    pub(crate) cwd: Option<std::path::PathBuf>,
    pub(crate) environment: Option<std::collections::BTreeMap<String, String>>,
    pub(crate) test_failures: Option<Vec<String>>,
    pub(crate) handles: Arena<crate::handle::Handle>,
    pub(super) enum_names: Option<Rc<Vec<Vec<String>>>>,
    integers: Arena<Rc<BigInt>>,
    rationals: Arena<BigRational>,
    pub(super) strings: Arena<Rc<str>>,
    pub(super) arrays: Arena<Vec<Slot>>,
    pub(super) builders: Arena<Vec<Slot>>,
    pub(super) ranges: Arena<[Slot; 3]>,
    pub(super) maps: Arena<crate::map::Map>,
    pub(super) map_builders: Arena<crate::map::Map>,
    // Header (descriptor ID) and fields share one allocation. References inside
    // are non-owning generational handles, so neither sweep nor drop recurses.
    pub(super) objects: Arena<Box<[Slot]>>,
    pub(super) boxes: Arena<Slot>,
    pub(super) failures: Arena<crate::failure::Failure>,
    pub(super) resources: crate::resource::Resources,
    pub(super) regions: Arena<crate::resource::Region>,
    pub(super) futures: Arena<crate::future::FutureCell>,
    pub(super) native_failures: Arena<[Slot; 4]>,
    pub(super) native_descriptor: Option<usize>,
    pub(super) equality_work: Vec<(Slot, Slot)>,
    pub(super) equality_seen: std::collections::HashSet<(u64, u64, u64)>,
    closures: Arena<Box<[Slot]>>,
    pub(crate) worker_bindings: std::collections::HashMap<u64, (Slot, Vec<Slot>)>,
    trace: Vec<Slot>,
    // Suspended Tokio tasks publish roots here; the active task supplies its
    // current liveness directly at each collection safepoint.
    parked_roots: Vec<Option<Vec<Slot>>>,
    free_root_sets: Vec<usize>,
    next_activation: u64,
    pub(super) spawned: Vec<Slot>,
    pub(super) collectors: std::collections::HashMap<u64, crate::collect::Collection>,
    pub(super) cached_futures: std::collections::HashMap<u64, Vec<(usize, Slot)>>,
    pub(super) completion_clock: u64,
    pub(super) completion_signal: Option<tokio::sync::watch::Sender<()>>,
    pub(super) streams: std::collections::HashMap<u64, crate::stream::Stream>,
    pub(super) barriers:
        std::collections::HashMap<u64, Rc<std::cell::RefCell<crate::barrier::Barrier>>>,
    pub(super) barrier_snapshots: std::collections::HashMap<u64, crate::barrier::Snapshot>,
    pub(super) semaphores: std::collections::HashMap<u64, Rc<crate::semaphore::Pool>>,
    pub(super) groups: std::collections::HashMap<u64, crate::group::Group>,
    pub(super) channels:
        std::collections::HashMap<u64, Rc<std::cell::RefCell<crate::channel::Channel>>>,
    pub(crate) json_nodes: Arena<crate::native_json::Node>,
    pub(crate) fake_storage: std::collections::HashMap<u64, crate::fake_storage::Storage>,
    pub(super) atomics: std::collections::HashMap<u64, Option<Slot>>,
    pub(super) locks: std::collections::HashMap<u64, std::sync::Arc<tokio::sync::Semaphore>>,
    pub(super) grants: std::collections::HashMap<u64, crate::intrinsic::Grant>,
    pub(super) events: std::collections::HashMap<u64, tokio::sync::watch::Sender<bool>>,
    pub collections: usize,
}
impl Heap {
    pub fn new(constants: &[Rc<BigInt>], strings: &[Rc<str>]) -> Self {
        Self {
            environment: None,
            stdin: None,
            arguments: Vec::new(),
            cwd: None,
            integers: Arena::pinned(constants),
            enum_names: None,
            test_failures: None,
            handles: Arena::default(),
            rationals: Arena::default(),
            strings: Arena::pinned(strings),
            arrays: Arena::default(),
            builders: Arena::default(),
            ranges: Arena::default(),
            maps: Arena::default(),
            map_builders: Arena::default(),
            objects: Arena::default(),
            boxes: Arena::default(),
            failures: Arena::default(),
            resources: crate::resource::Resources::default(),
            regions: Arena::default(),
            futures: Arena::default(),
            native_failures: Arena::default(),
            native_descriptor: None,
            equality_work: Vec::new(),
            equality_seen: std::collections::HashSet::new(),
            closures: Arena::default(),
            worker_bindings: std::collections::HashMap::new(),
            trace: Vec::new(),
            parked_roots: Vec::new(),
            free_root_sets: Vec::new(),
            next_activation: 0,
            spawned: Vec::new(),
            events: std::collections::HashMap::new(),
            locks: std::collections::HashMap::new(),
            collectors: std::collections::HashMap::new(),
            cached_futures: std::collections::HashMap::new(),
            completion_clock: 0,
            completion_signal: None,
            streams: std::collections::HashMap::new(),
            barriers: std::collections::HashMap::new(),
            barrier_snapshots: std::collections::HashMap::new(),
            semaphores: std::collections::HashMap::new(),
            groups: std::collections::HashMap::new(),
            channels: std::collections::HashMap::new(),
            atomics: std::collections::HashMap::new(),
            fake_storage: std::collections::HashMap::new(),
            json_nodes: Arena::default(),
            grants: std::collections::HashMap::new(),
            collections: 0,
        }
    }
    pub(crate) fn allocate_frame(&mut self) -> Result<u64, Error> {
        let frame = self.next_activation;
        self.next_activation = frame
            .checked_add(1)
            .ok_or_else(|| Error("activation identity space exhausted".into()))?;
        Ok(frame)
    }
    pub(crate) fn register_roots(&mut self) -> usize {
        if let Some(id) = self.free_root_sets.pop() {
            self.parked_roots[id] = Some(Vec::new());
            id
        } else {
            let id = self.parked_roots.len();
            self.parked_roots.push(Some(Vec::new()));
            id
        }
    }
    pub(crate) fn publish_roots(&mut self, id: usize, roots: impl Iterator<Item = Slot>) {
        let parked = self.parked_roots[id].as_mut().expect("registered task");
        parked.clear();
        parked.extend(roots);
    }
    pub(crate) fn unregister_roots(&mut self, id: usize) {
        assert!(self.parked_roots[id].take().is_some());
        self.free_root_sets.push(id);
    }
    pub fn text(&self, value: Slot) -> Result<&str, Error> {
        if value.tag != STR {
            return Err(Error("expected string handle".into()));
        }
        Ok(self.strings.get(value.bits)?.as_ref())
    }
    pub fn allocate_text(&mut self, value: String) -> Result<Slot, Error> {
        Ok(Slot {
            bits: self.strings.allocate(Rc::from(value))?,
            tag: STR,
        })
    }
    pub fn rat_part(&mut self, value: Slot, denominator: bool) -> Result<Slot, Error> {
        let ratio = self.ratio(value)?;
        let text = if denominator {
            ratio.denom()
        } else {
            ratio.numer()
        }
        .to_string();
        self.allocate_text(text)
    }
    pub fn closure_new(
        &mut self,
        function: usize,
        captures: impl ExactSizeIterator<Item = Slot>,
    ) -> Result<Slot, Error> {
        let mut storage = Vec::with_capacity(captures.len() + 1);
        storage.push(Slot {
            bits: function as u64,
            tag: CLOSURE,
        });
        storage.extend(captures);
        Ok(Slot {
            bits: self.closures.allocate(storage.into_boxed_slice())?,
            tag: CLOSURE,
        })
    }
    pub fn closure(&self, value: Slot) -> Result<(usize, &[Slot]), Error> {
        if value.tag != CLOSURE {
            return Err(Error("expected closure handle".into()));
        }
        let storage = self.closures.get(value.bits)?;
        Ok((storage[0].bits as usize, &storage[1..]))
    }
    pub fn object_new(
        &mut self,
        descriptor: u32,
        fields: impl ExactSizeIterator<Item = Slot>,
    ) -> Result<Slot, Error> {
        let mut storage = Vec::with_capacity(fields.len() + 1);
        storage.push(Slot {
            bits: u64::from(descriptor),
            tag: OBJECT,
        });
        storage.extend(fields);
        let object = Slot {
            bits: self.objects.allocate(storage.into_boxed_slice())?,
            tag: OBJECT,
        };
        self.resources.created(descriptor as usize, object)?;
        Ok(object)
    }
    pub fn object_init(&mut self, object: Slot, field: u32, value: Slot) -> Result<(), Error> {
        if object.tag != OBJECT {
            return Err(Error("expected object handle".into()));
        }
        let slot = self
            .objects
            .get_mut(object.bits)?
            .get_mut(field as usize + 1)
            .ok_or_else(|| Error("object field index is out of bounds".into()))?;
        if slot.tag != UNBOUND {
            return Err(Error("object field is already bound".into()));
        }
        *slot = value;
        Ok(())
    }
    pub fn object_next(
        &mut self,
        object: Slot,
        updates: impl Iterator<Item = (u32, Slot)>,
    ) -> Result<Slot, Error> {
        if object.tag != OBJECT {
            return Err(Error("expected object handle".into()));
        }
        let mut storage = self.objects.get(object.bits)?.clone();
        for (field, value) in updates {
            *storage
                .get_mut(field as usize + 1)
                .ok_or_else(|| Error("successor field index is out of bounds".into()))? = value;
        }
        let object = Slot {
            bits: self.objects.allocate(storage)?,
            tag: OBJECT,
        };
        self.resources
            .created(self.object_descriptor(object)?, object)?;
        Ok(object)
    }
    pub fn object_descriptor(&self, object: Slot) -> Result<usize, Error> {
        if object.tag == NATIVE_FAILURE {
            self.native_failure_fields(object)?;
            return self
                .native_descriptor
                .ok_or_else(|| Error("NativeFailure descriptor unavailable".into()));
        }
        if object.tag != OBJECT {
            return Err(Error("expected object handle".into()));
        }
        Ok(self.objects.get(object.bits)?[0].bits as usize)
    }
    pub fn object_get(&self, object: Slot, field: u32) -> Result<Slot, Error> {
        if object.tag == NATIVE_FAILURE {
            return self
                .native_failure_fields(object)?
                .get(field as usize)
                .copied()
                .ok_or_else(|| Error("NativeFailure field out of bounds".into()));
        }
        if object.tag != OBJECT {
            return Err(Error("expected object handle".into()));
        }
        self.objects
            .get(object.bits)?
            .get(field as usize + 1)
            .copied()
            .ok_or_else(|| Error("object field out of bounds".into()))
    }
    fn get(&self, slot: Slot) -> Result<&Rc<BigInt>, Error> {
        if slot.tag != BIG {
            return Err(Error("expected big integer handle".into()));
        }
        self.integers.get(slot.bits)
    }
    pub fn integer(&self, slot: Slot) -> Result<Integer, Error> {
        match slot.tag {
            SMALL => Ok(Integer::Small(slot.bits as i64)),
            BIG => Ok(Integer::Big(self.get(slot)?.clone())),
            _ => Err(Error("expected integer slot".into())),
        }
    }
    fn big(&self, slot: Slot) -> Result<Cow<'_, BigInt>, Error> {
        match slot.tag {
            SMALL => Ok(Cow::Owned(BigInt::from(slot.bits as i64))),
            BIG => Ok(Cow::Borrowed(self.get(slot)?.as_ref())),
            _ => Err(Error("expected integer slot".into())),
        }
    }
    pub(crate) fn allocate(&mut self, value: BigInt) -> Result<Slot, Error> {
        if let Some(value) = value.to_i64() {
            return Ok(Slot::small(value));
        }
        Ok(Slot::big(self.integers.allocate(Rc::new(value))?))
    }

    fn ratio(&self, slot: Slot) -> Result<Cow<'_, BigRational>, Error> {
        if let Some((n, d)) = crate::rational::parts(slot) {
            return Ok(Cow::Owned(BigRational::new(
                BigInt::from(n),
                BigInt::from(d),
            )));
        }
        if slot.tag != RAT {
            return Err(Error("expected rational slot".into()));
        }
        Ok(Cow::Borrowed(self.rationals.get(slot.bits)?))
    }
    fn allocate_rat(&mut self, value: BigRational) -> Result<Slot, Error> {
        if let (Some(n), Some(d)) = (value.numer().to_i32(), value.denom().to_u32()) {
            return Ok(Slot {
                bits: (u64::from(d) << 32) | u64::from(n as u32),
                tag: SMALL_RAT,
            });
        }
        Ok(Slot {
            bits: self.rationals.allocate(value)?,
            tag: RAT,
        })
    }
    fn ratio_pair(&mut self, n: i128, d: i128) -> Result<Slot, Error> {
        if d == 0 {
            return Err(Error("Division by zero".into()));
        }
        if let Some(slot) = crate::rational::pack(n, d) {
            return Ok(slot);
        }
        self.allocate_rat(BigRational::new(BigInt::from(n), BigInt::from(d)))
    }
    pub fn rat_new(&mut self, numerator: Slot, denominator: Slot) -> Result<Slot, Error> {
        if numerator.tag == SMALL && denominator.tag == SMALL {
            return self.ratio_pair(
                numerator.bits as i64 as i128,
                denominator.bits as i64 as i128,
            );
        }
        let n = self.big(numerator)?.into_owned();
        let d = self.big(denominator)?.into_owned();
        if d.is_zero() {
            return Err(Error("Division by zero".into()));
        }
        self.allocate_rat(BigRational::new(n, d))
    }
    pub fn rat_binary(
        &mut self,
        op: crate::rational::Binary,
        left: Slot,
        right: Slot,
    ) -> Result<Slot, Error> {
        use crate::rational::{Binary, parts};
        if let (Some((a, b)), Some((c, d))) = (parts(left), parts(right)) {
            if c == 0 && matches!(op, Binary::Divide | Binary::Remainder) {
                return Err(Error(
                    if matches!(op, Binary::Divide) {
                        "Division by zero"
                    } else {
                        "Modulo by zero"
                    }
                    .into(),
                ));
            }
            let (n, den) = match op {
                Binary::Add => (a * d + c * b, b * d),
                Binary::Subtract => (a * d - c * b, b * d),
                Binary::Multiply => (a * c, b * d),
                Binary::Divide => (a * d, b * c),
                Binary::Remainder => (a * d - (a * d / (b * c)) * c * b, b * d),
            };
            return self.ratio_pair(n, den);
        }
        let a = self.ratio(left)?;
        let b = self.ratio(right)?;
        if b.is_zero() && matches!(op, Binary::Divide | Binary::Remainder) {
            return Err(Error(
                if matches!(op, Binary::Divide) {
                    "Division by zero"
                } else {
                    "Modulo by zero"
                }
                .into(),
            ));
        }
        let result = match op {
            Binary::Add => &*a + &*b,
            Binary::Subtract => &*a - &*b,
            Binary::Multiply => &*a * &*b,
            Binary::Divide => &*a / &*b,
            Binary::Remainder => {
                let q = (&*a / &*b).to_integer();
                &*a - &*b * BigRational::from_integer(q)
            }
        };
        drop(a);
        drop(b);
        self.allocate_rat(result)
    }
    pub fn rat_unary(&mut self, op: crate::rational::Unary, value: Slot) -> Result<Slot, Error> {
        use crate::rational::Unary;
        if let Some((n, d)) = crate::rational::parts(value) {
            return self.ratio_pair(
                match op {
                    Unary::Negate => -n,
                    Unary::Absolute => n.abs(),
                },
                d,
            );
        }
        let n = self.ratio(value)?;
        if matches!(op, Unary::Absolute) && !n.is_negative() {
            return Ok(value);
        }
        let result = match op {
            Unary::Negate => -&*n,
            Unary::Absolute => n.abs(),
        };
        drop(n);
        self.allocate_rat(result)
    }
    pub fn arithmetic(&mut self, op: Arithmetic, left: Slot, right: Slot) -> Result<Slot, Error> {
        if matches!(
            op,
            Arithmetic::And
                | Arithmetic::Or
                | Arithmetic::Xor
                | Arithmetic::ShiftLeft
                | Arithmetic::ShiftRight
                | Arithmetic::ShiftUnsigned
        ) {
            let shift = matches!(
                op,
                Arithmetic::ShiftLeft | Arithmetic::ShiftRight | Arithmetic::ShiftUnsigned
            );
            // Shift count validation precedes word validation, as specified.
            let r = if shift {
                self.integer(right)?
                    .to_i64()
                    .filter(|n| (0..64).contains(n))
                    .ok_or_else(|| Error("Shift count must be in 0..63".into()))?
            } else {
                self.word(right)?
            };
            let l = self.word(left)?;
            return Ok(Slot::small(match op {
                Arithmetic::And => l & r,
                Arithmetic::Or => l | r,
                Arithmetic::Xor => l ^ r,
                Arithmetic::ShiftLeft => l << r,
                Arithmetic::ShiftRight => l >> r,
                Arithmetic::ShiftUnsigned => ((l as u64) >> r) as i64,
                _ => unreachable!(),
            }));
        }
        if left.tag == SMALL && right.tag == SMALL {
            let (l, r) = (left.bits as i64, right.bits as i64);
            let fast = match op {
                Arithmetic::Add => l.checked_add(r),
                Arithmetic::Subtract => l.checked_sub(r),
                Arithmetic::Multiply => l.checked_mul(r),
                Arithmetic::Remainder if r == 0 => {
                    return Err(Error("Integer division by zero".into()));
                }
                Arithmetic::Remainder if l == i64::MIN && r == -1 => Some(0),
                Arithmetic::Remainder => l.checked_rem(r),
                _ => unreachable!(),
            };
            if let Some(value) = fast {
                return Ok(Slot::small(value));
            }
        }
        let (l, r) = (self.big(left)?, self.big(right)?);
        if matches!(op, Arithmetic::Remainder) && *r == BigInt::from(0) {
            return Err(Error("Integer division by zero".into()));
        }
        let result = match op {
            Arithmetic::Add => &*l + &*r,
            Arithmetic::Subtract => &*l - &*r,
            Arithmetic::Multiply => &*l * &*r,
            Arithmetic::Remainder => &*l % &*r,
            _ => unreachable!(),
        };
        drop(l);
        drop(r);
        self.allocate(result)
    }
    fn word(&self, value: Slot) -> Result<i64, Error> {
        self.integer(value)?
            .to_i64()
            .ok_or_else(|| Error("Bitwise operand is outside signed-64 range".into()))
    }

    pub fn byte_binary(&self, op: Arithmetic, left: Slot, right: Slot) -> Result<Slot, Error> {
        let l = left.bits as u8;
        let value = match op {
            Arithmetic::And => l & right.bits as u8,
            Arithmetic::Or => l | right.bits as u8,
            Arithmetic::Xor => l ^ right.bits as u8,
            Arithmetic::ShiftLeft | Arithmetic::ShiftRight | Arithmetic::ShiftUnsigned => {
                let count = self
                    .integer(right)?
                    .to_i64()
                    .filter(|n| (0..8).contains(n))
                    .ok_or_else(|| Error("Shift count must be in 0..7".into()))?;
                if matches!(op, Arithmetic::ShiftLeft) {
                    l << count
                } else {
                    l >> count
                }
            }
            _ => return Err(Error("invalid byte operation".into())),
        };
        Ok(Slot {
            bits: u64::from(value),
            tag: BYTE,
        })
    }

    pub fn bits(
        &self,
        op: okkama_sparrow::BitOp,
        byte: bool,
        value: Slot,
        count: Option<Slot>,
    ) -> Result<Slot, Error> {
        use okkama_sparrow::BitOp;
        // Validate the word before rotation count, unlike the shift operators.
        let word = if byte {
            value.bits as i64
        } else {
            self.word(value)?
        };
        let rotation = || -> Result<u32, Error> {
            count
                .map(|v| self.integer(v))
                .transpose()?
                .and_then(|n| n.to_i64())
                .filter(|n| (0..if byte { 8 } else { 64 }).contains(n))
                .map(|n| n as u32)
                .ok_or_else(|| {
                    Error(
                        if byte {
                            "Shift count must be in 0..7"
                        } else {
                            "Shift count must be in 0..63"
                        }
                        .into(),
                    )
                })
        };
        let result = if byte {
            let w = word as u8;
            i64::from(match op {
                BitOp::BitCount => w.count_ones() as u8,
                BitOp::LeadingZeros => w.leading_zeros() as u8,
                BitOp::TrailingZeros => w.trailing_zeros() as u8,
                BitOp::RotateLeft => w.rotate_left(rotation()?),
                BitOp::RotateRight => w.rotate_right(rotation()?),
                BitOp::ReverseBits => w.reverse_bits(),
                BitOp::SwapBytes => w,
            })
        } else {
            match op {
                BitOp::BitCount => i64::from(word.count_ones()),
                BitOp::LeadingZeros => i64::from(word.leading_zeros()),
                BitOp::TrailingZeros => i64::from(word.trailing_zeros()),
                BitOp::RotateLeft => word.rotate_left(rotation()?),
                BitOp::RotateRight => word.rotate_right(rotation()?),
                BitOp::ReverseBits => word.reverse_bits(),
                BitOp::SwapBytes => word.swap_bytes(),
            }
        };
        Ok(if byte && !op.counts() {
            Slot {
                bits: result as u64,
                tag: BYTE,
            }
        } else {
            Slot::small(result)
        })
    }
    pub fn unary(&mut self, op: Unary, value: Slot) -> Result<Slot, Error> {
        if matches!(op, Unary::Sign) && matches!(value.tag, RAT | SMALL_RAT) {
            if let Some((n, _)) = crate::rational::parts(value) {
                return Ok(Slot::small(n.signum() as i64));
            }
            return Ok(Slot::small(match self.ratio(value)?.numer().sign() {
                num_bigint::Sign::Minus => -1,
                num_bigint::Sign::NoSign => 0,
                num_bigint::Sign::Plus => 1,
            }));
        }
        if matches!(op, Unary::Sign) && value.tag == FLOAT {
            let n = f64::from_bits(value.bits);
            return Ok(Slot::small(if n > 0.0 {
                1
            } else if n < 0.0 {
                -1
            } else {
                0
            }));
        }
        if matches!(op, Unary::Not) {
            return Ok(Slot::small(!self.word(value)?));
        }
        if value.tag == SMALL {
            let n = value.bits as i64;
            let result = match op {
                Unary::Negate => n.checked_neg(),
                Unary::Absolute => n.checked_abs(),
                Unary::Sign => Some(n.signum()),
                Unary::Not => unreachable!(),
            };
            if let Some(value) = result {
                return Ok(Slot::small(value));
            }
        }
        let value = self.big(value)?;
        let result = match op {
            Unary::Negate => -&*value,
            Unary::Absolute => value.abs(),
            Unary::Sign => value.signum(),
            Unary::Not => unreachable!(),
        };
        drop(value);
        self.allocate(result)
    }
    pub fn compare(&self, left: Slot, right: Slot) -> Result<std::cmp::Ordering, Error> {
        if left.tag == ENUM && right.tag == ENUM {
            return Ok(left.bits.cmp(&right.bits));
        }
        if left.tag == STR && right.tag == STR {
            return Ok(self.text(left)?.cmp(self.text(right)?));
        }
        if matches!(left.tag, RAT | SMALL_RAT) && matches!(right.tag, RAT | SMALL_RAT) {
            if let (Some((a, b)), Some((c, d))) =
                (crate::rational::parts(left), crate::rational::parts(right))
            {
                return Ok((a * d).cmp(&(c * b)));
            }
            return Ok(self.ratio(left)?.cmp(&self.ratio(right)?));
        }
        if left.tag == FLOAT && right.tag == FLOAT {
            return f64::from_bits(left.bits)
                .partial_cmp(&f64::from_bits(right.bits))
                .ok_or_else(|| Error("non-finite float comparison".into()));
        }
        if left.tag == right.tag && matches!(left.tag, BOOL | BYTE | CHAR) {
            return Ok(left.bits.cmp(&right.bits));
        }
        if left.tag == SMALL && right.tag == SMALL {
            return Ok((left.bits as i64).cmp(&(right.bits as i64)));
        }
        Ok(self.big(left)?.cmp(&self.big(right)?))
    }
    pub fn needs_collection(&self) -> bool {
        self.integers.allocations
            + self.rationals.allocations
            + self.objects.allocations
            + self.handles.allocations
            + self.json_nodes.allocations
            + self.closures.allocations
            + self.boxes.allocations
            + self.failures.allocations
            + self.regions.allocations
            + self.futures.allocations
            + self.native_failures.allocations
            + self.strings.allocations
            + self.arrays.allocations
            + self.builders.allocations
            + self.ranges.allocations
            + self.maps.allocations
            + self.map_builders.allocations
            >= 128
    }

    pub fn convert(&mut self, target: crate::scalar::Kind, value: Slot) -> Result<Slot, Error> {
        use crate::scalar::{Kind, finite, float_text};
        if target == Kind::Str {
            let text = match value.tag {
                SMALL | BIG => self.integer(value)?.to_string(),
                BOOL | BYTE => value.bits.to_string(),
                FLOAT => float_text(f64::from_bits(value.bits)),
                RAT | SMALL_RAT => self.ratio(value)?.to_string(),
                _ => return Err(Error("unsupported string conversion".into())),
            };
            return self.allocate_text(text);
        }
        match (value.tag, target) {
            (SMALL, Kind::Rat) => self.ratio_pair(value.bits as i64 as i128, 1),
            (BYTE, Kind::Rat) => self.ratio_pair(value.bits as i128, 1),
            (BIG, Kind::Rat) => {
                let n = self.big(value)?.into_owned();
                self.allocate_rat(BigRational::from_integer(n))
            }
            (FLOAT, Kind::Rat) => {
                let f = f64::from_bits(value.bits);
                let (n, e) = crate::rational::decimal_parts(f);
                if let Some(power) = 10i128.checked_pow(e.unsigned_abs()) {
                    if e < 0 {
                        return self.ratio_pair(n, power);
                    }
                    if let Some(n) = n.checked_mul(power) {
                        return self.ratio_pair(n, 1);
                    }
                }
                self.allocate_rat(crate::rational::decimal_ratio(f))
            }
            (SMALL_RAT, Kind::Int) => {
                let (n, d) = crate::rational::parts(value).expect("small rational");
                Ok(Slot::small((n / d) as i64))
            }
            (RAT, Kind::Int) => {
                let n = self.ratio(value)?.to_integer();
                self.allocate(n)
            }
            (SMALL_RAT, Kind::Float) => {
                let (n, d) = crate::rational::parts(value).expect("small rational");
                finite(n as f64 / d as f64)
            }
            (RAT, Kind::Float) => finite(crate::rational::to_float(self.ratio(value)?.as_ref())?),
            (BYTE, Kind::Int) => Ok(Slot::small(value.bits as i64)),
            (SMALL | BIG, Kind::Float) => {
                let n = match self.integer(value)? {
                    Integer::Small(n) => n as f64,
                    Integer::Big(n) => n.to_f64().unwrap_or(f64::INFINITY),
                };
                finite(n)
            }
            (BYTE, Kind::Float) => finite(value.bits as f64),
            (FLOAT, Kind::Int) => {
                let n = f64::from_bits(value.bits);
                let truncated = n.trunc();
                if !(-9223372036854775808.0..9223372036854775808.0).contains(&truncated) {
                    return Err(Error(format!(
                        "Cannot convert float value {} to int: expected signed-64 range",
                        float_text(n)
                    )));
                }
                Ok(Slot::small(truncated as i64))
            }
            _ => Err(Error("unsupported scalar conversion".into())),
        }
    }

    pub fn byte_from(&self, value: Slot, context: Option<&str>) -> Result<Slot, Error> {
        let (kind, text) = if value.tag == FLOAT {
            let n = f64::from_bits(value.bits);
            if (0.0..=255.0).contains(&n.trunc()) {
                return Ok(Slot {
                    bits: n.trunc() as u64,
                    tag: BYTE,
                });
            }
            ("float", crate::scalar::float_text(n))
        } else if let Some((n, d)) = crate::rational::parts(value) {
            if let Ok(value) = u8::try_from(n / d) {
                return Ok(Slot {
                    bits: u64::from(value),
                    tag: BYTE,
                });
            }
            (
                "rat",
                if d == 1 {
                    n.to_string()
                } else {
                    format!("{n}/{d}")
                },
            )
        } else if value.tag == RAT {
            let n = self.ratio(value)?;
            if let Some(value) = n.to_integer().to_u8() {
                return Ok(Slot {
                    bits: u64::from(value),
                    tag: BYTE,
                });
            }
            ("rat", n.to_string())
        } else {
            let n = self.integer(value)?;
            if let Some(value) = n.to_i64().and_then(|n| u8::try_from(n).ok()) {
                return Ok(Slot {
                    bits: u64::from(value),
                    tag: BYTE,
                });
            }
            ("int", n.to_string())
        };
        let message = format!("Cannot convert {kind} value {text} to byte: expected 0..255");
        Err(Error(match context {
            Some(c) => format!("{c}: {message}"),
            None => message,
        }))
    }
    pub fn collect(&mut self, roots: impl Iterator<Item = Slot>) -> Result<(), Error> {
        // Completed tasks need registration roots only for unobserved failures.
        // Live language references still retain the Future and its cached result.
        for &handle in &self.spawned {
            self.future(handle)?;
        }
        let futures = &self.futures;
        self.spawned.retain(|handle| {
            let future = futures.get(handle.bits).expect("validated spawned Future");
            !matches!(future.state, crate::future::State::Ready(Ok(_)))
                && !(future.observed && matches!(future.state, crate::future::State::Ready(Err(_))))
        });
        self.integers.begin_collection();
        self.rationals.begin_collection();
        self.objects.begin_collection();
        self.handles.begin_collection();
        self.json_nodes.begin_collection();
        self.closures.begin_collection();
        self.boxes.begin_collection();
        self.failures.begin_collection();
        self.regions.begin_collection();
        self.futures.begin_collection();
        self.native_failures.begin_collection();
        self.strings.begin_collection();
        self.arrays.begin_collection();
        self.builders.begin_collection();
        self.ranges.begin_collection();
        self.maps.begin_collection();
        self.map_builders.begin_collection();
        self.trace.clear();
        self.trace.extend(roots);
        self.trace.extend(self.spawned.iter().copied());
        self.trace.extend(
            self.parked_roots
                .iter()
                .flatten()
                .flat_map(|roots| roots.iter().copied()),
        );
        while let Some(root) = self.trace.pop() {
            match root.tag {
                NATIVE_FAILURE => {
                    if self.native_failures.mark(root.bits)? {
                        self.trace
                            .extend_from_slice(self.native_failures.get(root.bits)?);
                    }
                }
                FUTURE => {
                    if self.futures.mark(root.bits)? {
                        let future = self.futures.get(root.bits)?;
                        self.trace.extend(future.captures.iter().copied());
                        if let Some(request) = &future.request {
                            request.roots(&mut self.trace);
                        }
                        if let crate::future::State::Stream(owner)
                        | crate::future::State::Collect(owner)
                        | crate::future::State::Signal(owner)
                        | crate::future::State::Event(owner)
                        | crate::future::State::Permit(owner, _) = future.state
                        {
                            self.trace.push(owner);
                        }
                        if let crate::future::State::Ready(result) = future.state {
                            self.trace.push(match result {
                                Ok(v) | Err(v) => v,
                            });
                        }
                    }
                }
                REGION => {
                    if self.regions.mark(root.bits)? {
                        self.trace.extend(self.regions.get(root.bits)?.roots());
                    }
                }
                FAILURE => {
                    if self.failures.mark(root.bits)? {
                        let failure = self.failures.get(root.bits)?;
                        if let crate::failure::FailureKind::Thrown(value)
                        | crate::failure::FailureKind::Native(value) = failure.kind
                        {
                            self.trace.push(value);
                        }
                        self.trace.extend(failure.cause);
                        self.trace.extend(failure.secondary.iter().copied());
                    }
                }
                ANY => {
                    if self.boxes.mark(root.bits)? {
                        self.trace.push(*self.boxes.get(root.bits)?);
                    }
                }
                HANDLE => {
                    if self.handles.mark(root.bits)? {
                        self.handles.get(root.bits)?.roots(&mut self.trace);
                    }
                }
                JSON_NODE => {
                    if self.json_nodes.mark(root.bits)? {
                        self.json_nodes.get(root.bits)?.roots(&mut self.trace);
                    }
                }
                CLOSURE => {
                    if self.closures.mark(root.bits)? {
                        if let Some((entry, fields)) = self.worker_bindings.get(&root.bits) {
                            self.trace.push(*entry);
                            self.trace.extend_from_slice(fields);
                        }
                        self.trace
                            .extend_from_slice(&self.closures.get(root.bits)?[1..]);
                    }
                }
                MAP => {
                    if self.maps.mark(root.bits)? {
                        for (k, v) in self.maps.get(root.bits)?.values() {
                            self.trace.extend([*k, *v]);
                        }
                    }
                }
                MAP_BUILDER => {
                    if self.map_builders.mark(root.bits)? {
                        for (k, v) in self.map_builders.get(root.bits)?.values() {
                            self.trace.extend([*k, *v]);
                        }
                    }
                }
                RANGE => {
                    if self.ranges.mark(root.bits)? {
                        self.trace.extend_from_slice(self.ranges.get(root.bits)?);
                    }
                }
                ARRAY => {
                    if self.arrays.mark(root.bits)? {
                        self.trace.extend_from_slice(self.arrays.get(root.bits)?);
                    }
                }
                BUILDER => {
                    if self.builders.mark(root.bits)? {
                        self.trace.extend_from_slice(self.builders.get(root.bits)?);
                    }
                }
                STR => {
                    self.strings.mark(root.bits)?;
                }
                OBJECT => {
                    if self.objects.mark(root.bits)? {
                        if let Some(stream) = self.streams.get(&root.bits) {
                            stream.roots(&mut self.trace);
                        }
                        if let Some(collection) = self.collectors.get(&root.bits) {
                            collection.roots(&mut self.trace);
                        }
                        if let Some(cached) = self.cached_futures.get(&root.bits) {
                            self.trace.extend(cached.iter().map(|(_, future)| *future));
                        }
                        if let Some(group) = self.groups.get(&root.bits) {
                            group.roots(&mut self.trace);
                        }
                        if let Some(channel) = self.channels.get(&root.bits) {
                            channel.borrow().roots(&mut self.trace);
                        }
                        if let Some(storage) = self.fake_storage.get(&root.bits) {
                            for (key, value, _) in storage.values() {
                                self.trace.extend([*key, *value]);
                            }
                        }
                        if let Some(Some(value)) = self.atomics.get(&root.bits) {
                            self.trace.push(*value);
                        }
                        self.trace
                            .extend_from_slice(&self.objects.get(root.bits)?[1..]);
                    }
                }
                BIG => {
                    self.integers.mark(root.bits)?;
                }
                RAT => {
                    self.rationals.mark(root.bits)?;
                }
                _ => {}
            }
        }
        self.integers.sweep();
        self.rationals.sweep();
        self.objects.sweep();
        self.fake_storage
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.collectors
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.cached_futures
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.streams
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.barriers
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.barrier_snapshots
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.semaphores
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.groups
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.channels
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.atomics
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.locks
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.grants
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.events
            .retain(|handle, _| self.objects.get(*handle).is_ok());
        self.handles.sweep();
        self.json_nodes.sweep();
        self.closures.sweep();
        self.worker_bindings
            .retain(|handle, _| self.closures.get(*handle).is_ok());
        self.boxes.sweep();
        self.failures.sweep();
        self.regions.sweep();
        self.futures.sweep();
        self.native_failures.sweep();
        self.resources.sweep(&self.objects);
        self.strings.sweep();
        self.arrays.sweep();
        self.builders.sweep();
        self.ranges.sweep();
        self.maps.sweep();
        self.map_builders.sweep();
        self.collections += 1;
        Ok(())
    }
}
