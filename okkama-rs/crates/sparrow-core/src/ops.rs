//! Exact semantics of value instructions without an inline fast path.
//!
//! Machine code stores the words used by the instruction (in `visit` order) and
//! calls [`rt_exec`]; the helper re-reads the instruction, knows the static type
//! of every register and returns the defined word. The behaviour and failure
//! texts follow the previous engine.
#![allow(unsafe_code)]

use crate::heap::{self, Integer, Payload};
use crate::program::Kind;
use crate::runtime::{Ctx, FailureKind, Runtime};
use crate::word::{self, Repr, UNBOUND, Word};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};
use okkama_runtime_core::IntValue;
use okkama_runtime_core::bignum::Big;
use okkama_runtime_core::rat::Rat;
use okkama_sparrow::{Access, Cmp, Instruction as I, StrFn, Type, TypeRef};
use std::cmp::Ordering;
use std::rc::Rc;

type Outcome = Result<Word, String>;

// ---- payload values ----

#[derive(Debug)]
/// A `rat`: the engines' shared exact fraction, whose small parts are
/// machine integers (#326).
pub struct Rational(pub Rat);
#[derive(Debug)]
pub struct Builder {
    pub repr: Repr,
    pub items: Vec<Word>,
}
#[derive(Debug)]
pub struct RangeValue {
    pub start: Word,
    pub end: Word,
    pub inclusive: bool,
}
#[derive(Debug)]
pub struct AnyBox {
    pub value: Word,
    pub ty: TypeRef,
    pub repr: Repr,
}
#[derive(Debug)]
pub struct MapValue {
    pub key: Repr,
    pub value: Repr,
    pub entries: crate::maps::Entries,
    /// The Rust memory estimate (#331), computed once: a finished map does
    /// not change, and a builder adds each new entry's share as it goes.
    pub owned: std::cell::Cell<Option<usize>>,
}

macro_rules! payload {
    ($($ty:ty => |$this:ident, $visit:ident| $body:block $(, owns |$owner:ident| $bytes:block)?)*) => {$(
        impl Payload for $ty {
            fn trace(&self, $visit: &mut dyn FnMut(Word, Repr)) {
                let $this = self;
                $body
            }
            $(fn external_bytes(&self) -> usize {
                let $owner = self;
                $bytes
            })?
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    )*};
}
payload! {
    Rational => |_this, _visit| {}, owns |this| { rational_bytes(&this.0) }
    RangeValue => |this, visit| { visit(this.start, Repr::Int); visit(this.end, Repr::Int); }
    Builder => |this, visit| { for &item in &this.items { visit(item, this.repr); } },
        owns |this| { this.items.capacity() * 8 }
    AnyBox => |this, visit| { visit(this.value, this.repr); }
    MapValue => |this, visit| {
        this.entries.trace(|&(key, value)| {
            visit(key, this.key);
            visit(value, this.value);
        });
    }, owns |this| {
        // A persistent map's nodes and their keys are counted exactly by the
        // heap (`maps::node_memory`), shared ones once.
        if this.entries.is_persistent() {
            return 0;
        }
        this.owned.get().unwrap_or_else(|| {
            let bytes = this.entries.len() * MAP_ENTRY_BYTES + this.entries.keys().map(Key::owned_bytes).sum::<usize>();
            this.owned.set(Some(bytes));
            bytes
        })
    }
}

/// Bytes of B-tree nodes per map entry (#331): an entry's size twice. A
/// counting allocator measured 1.5–1.95 times (random and ascending keys,
/// 10³–10⁵ entries, Rust 1.97's `BTreeMap`); the estimate errs high.
pub const MAP_ENTRY_BYTES: usize = 2 * std::mem::size_of::<(Key, (Word, Word))>();

/// What a fraction owns outside itself: the digits of a part beyond 64 bits.
fn rational_bytes(value: &Rat) -> usize {
    let part = |n: &IntValue| if n.to_i64().is_some() { 0 } else { 48 + n.as_big().bits().div_ceil(64) as usize * 8 };
    part(value.numerator()) + part(value.denominator())
}

/// Map keys own immutable data; order follows the previous engine. An
/// object key (#212) is its declaration and its fields as keys, so equality
/// and order are structural, like `==`; the entry keeps the object's word.
#[derive(Debug, Clone)]
pub enum Key {
    /// An `int` that fits in 64 bits: no allocation to make, copy or free
    /// (#331). Always this form when the value fits, so equal keys are
    /// equal variants.
    Small(i64),
    /// An `int` outside 64 bits.
    Int(BigInt),
    Float(f64),
    Raw(u64),
    /// A string, and its first 8 bytes as a big-endian number, zeros past
    /// the end ([`Key::text`]): strings whose numbers differ order by them
    /// alone, inline, without reading either text.
    Str(u64, Rc<str>),
    /// A `rat` field of an object key.
    Rat(Rc<Rat>),
    Object(u32, Rc<[Key]>),
}
impl Key {
    /// The key of an `int`: small when it fits in 64 bits.
    pub fn int(value: Integer<'_>) -> Self {
        match value {
            Integer::Small(n) => Self::Small(n),
            Integer::Big(n) => n.to_i64().map_or_else(|| Self::Int(n.clone()), Self::Small),
        }
    }

    /// The key of a string.
    pub fn text(text: Rc<str>) -> Self {
        let mut first = [0; 8];
        let length = text.len().min(8);
        first[..length].copy_from_slice(&text.as_bytes()[..length]);
        Self::Str(u64::from_be_bytes(first), text)
    }

    /// Rust memory this key owns beyond the enum itself (#331). Data shared
    /// through `Rc` (copies of one map share their keys) counts its share
    /// only, so a map and its copies do not each pay for it in full.
    pub fn owned_bytes(&self) -> usize {
        match self {
            // A `BigInt` keeps its digits in a vector of its own.
            Self::Int(n) => n.bits().div_ceil(64) as usize * 8,
            Self::Small(_) | Self::Float(_) | Self::Raw(_) => 0,
            Self::Str(_, text) => (text.len() + 16) / std::rc::Rc::strong_count(text),
            Self::Rat(value) => rational_bytes(value) / std::rc::Rc::strong_count(value),
            Self::Object(_, fields) => {
                let own = 16 + fields.len() * std::mem::size_of::<Key>() + fields.iter().map(Key::owned_bytes).sum::<usize>();
                own / std::rc::Rc::strong_count(fields)
            }
        }
    }

    /// Rust memory this key's data takes, shared or not: what a persistent
    /// map node counts for its keys, the same when it takes them on and when
    /// it lets them go.
    pub fn data_bytes(&self) -> usize {
        match self {
            Self::Small(_) | Self::Float(_) | Self::Raw(_) => 0,
            Self::Int(n) => n.bits().div_ceil(64) as usize * 8,
            Self::Str(_, text) => text.len() + 16,
            Self::Rat(value) => rational_bytes(value),
            Self::Object(_, fields) => 16 + fields.len() * std::mem::size_of::<Key>() + fields.iter().map(Key::data_bytes).sum::<usize>(),
        }
    }
}
impl Ord for Key {
    /// Small integers and strings that differ in their first bytes, the
    /// common keys, compare inline; the rest out of line.
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Small(a), Self::Small(b)) => a.cmp(b),
            (Self::Str(a, _), Self::Str(b, _)) if a != b => a.cmp(b),
            _ => self.cmp_other(other),
        }
    }
}
impl Key {
    #[inline(never)]
    fn cmp_other(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Small(a), Self::Small(b)) => a.cmp(b),
            (Self::Int(a), Self::Int(b)) => a.cmp(b),
            // A wide key lies beyond every small one, on its sign's side.
            (Self::Small(_), Self::Int(b)) => if b.is_positive() { Ordering::Less } else { Ordering::Greater },
            (Self::Int(a), Self::Small(_)) => if a.is_positive() { Ordering::Greater } else { Ordering::Less },
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b).expect("finite float key"),
            (Self::Raw(a), Self::Raw(b)) => a.cmp(b),
            (Self::Str(_, a), Self::Str(_, b)) => a.cmp(b),
            (Self::Rat(a), Self::Rat(b)) => a.compare(b),
            (Self::Object(a, x), Self::Object(b, y)) => a.cmp(b).then_with(|| x.iter().cmp(y.iter())),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}
impl Key {
    fn rank(&self) -> u8 {
        match self {
            Self::Small(_) | Self::Int(_) => 0,
            Self::Float(_) => 1,
            Self::Raw(_) => 2,
            Self::Str(..) => 3,
            Self::Rat(_) => 4,
            Self::Object(..) => 5,
        }
    }
}
impl PartialOrd for Key {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Key {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Key {}

// ---- conversions between words and Rust values ----

impl Runtime {
    pub fn big_value(&self, word: Word) -> BigInt {
        match self.heap.integer(word) {
            Integer::Small(n) => BigInt::from(n),
            Integer::Big(n) => n.clone(),
        }
    }
    /// An `int` word as the shared integer type: small ones need no digits.
    pub fn int_value(&self, word: Word) -> IntValue {
        match self.heap.integer(word) {
            Integer::Small(n) => IntValue::from_i64(n),
            Integer::Big(n) => IntValue::from_big(Big::from_bigint(n.clone())),
        }
    }
    pub fn small_or_big(&self, word: Word) -> Option<i64> {
        match self.heap.integer(word) {
            Integer::Small(n) => Some(n),
            Integer::Big(n) => n.to_i64(),
        }
    }
    pub fn rational(&self, word: Word) -> &Rat {
        &self.heap.payload::<Rational>(word).expect("rational value").0
    }
    pub fn rational_new(&mut self, value: Rat) -> Word {
        self.heap.allocate_payload(Box::new(Rational(value)))
    }
    pub fn text_new(&mut self, text: &str) -> Word {
        self.heap.allocate_text(text)
    }
    fn ty(&self, function: usize, register: okkama_sparrow::Register) -> Option<&Type> {
        let source = &self.loaded.program.functions[function];
        self.loaded.program.ty(source.registers[register.0 as usize])
    }
    fn kind_of(&self, ty: TypeRef) -> Kind {
        Kind::of(&self.loaded.program, ty)
    }
    /// A map key (#242): a value whose type is not a key type — an array, a
    /// map, an object holding one — is refused instead of being compared by
    /// identity; a generic map could otherwise reach one at run time.
    pub(crate) fn checked_key(&self, word: Word, kind: Kind, ty: Option<&Type>) -> Result<Key, String> {
        if self.is_key(word, kind, ty) {
            Ok(self.key(word, kind, ty))
        } else {
            Err("map key must be a scalar, enum, string or value object".into())
        }
    }

    fn is_key(&self, word: Word, kind: Kind, ty: Option<&Type>) -> bool {
        match (kind, ty) {
            (Kind::Int | Kind::Float | Kind::Raw | Kind::Byte, _) | (_, Some(Type::Str | Type::Rat)) => true,
            (_, Some(Type::Object(_))) if heap::kind(self.heap.header_of(word)) == heap::OBJECT => {
                let declaration = heap::payload(self.heap.header_of(word));
                let program = &self.loaded.program;
                program.objects[declaration as usize]
                    .fields
                    .iter()
                    .enumerate()
                    .all(|(index, &(_, field))| self.is_key(self.heap.field(word, index), self.kind_of(field), program.ty(field)))
            }
            _ => false,
        }
    }

    pub(crate) fn key(&self, word: Word, kind: Kind, ty: Option<&Type>) -> Key {
        match (kind, ty) {
            (Kind::Int, _) => Key::int(self.heap.integer(word)),
            (Kind::Float, _) => Key::Float(f64::from_bits(word)),
            (Kind::Raw | Kind::Byte, _) => Key::Raw(word),
            (_, Some(Type::Str)) => Key::text(Rc::from(self.heap.text(word))),
            (_, Some(Type::Rat)) => Key::Rat(Rc::new(self.rational(word).clone())),
            (_, Some(Type::Object(_))) if heap::kind(self.heap.header_of(word)) == heap::OBJECT => {
                let declaration = heap::payload(self.heap.header_of(word));
                let program = &self.loaded.program;
                let fields = program.objects[declaration as usize]
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, &(_, field))| self.key(self.heap.field(word, index), self.kind_of(field), program.ty(field)))
                    .collect();
                Key::Object(declaration, fields)
            }
            _ => Key::Raw(word),
        }
    }
}

fn integer_word(runtime: &mut Runtime, value: BigInt) -> Word {
    runtime.heap.big(value)
}

fn finite(value: f64) -> Outcome {
    if value.is_finite() {
        Ok(value.to_bits())
    } else {
        Err("Float value must contain a finite number".into())
    }
}

fn bool_word(value: bool) -> Word {
    Word::from(value)
}

fn compare_flag(op: Cmp, order: Ordering) -> Word {
    bool_word(match op {
        Cmp::Eq => order.is_eq(),
        Cmp::Ne => !order.is_eq(),
        Cmp::Lt => order.is_lt(),
        Cmp::Le => !order.is_gt(),
        Cmp::Gt => order.is_gt(),
        Cmp::Ge => !order.is_lt(),
    })
}

fn slice_bounds(runtime: &Runtime, length: usize, start: Word, end: Word) -> Result<std::ops::Range<usize>, String> {
    let (start, end) = (runtime.big_value(start), runtime.big_value(end));
    if start.is_negative() {
        return Err("Slice start must not be negative".into());
    }
    if start > end {
        return Err("Slice start must be less than or equal to slice end".into());
    }
    if end > BigInt::from(length) {
        return Err("Slice end must not exceed length".into());
    }
    Ok(start.to_usize().expect("validated")..end.to_usize().expect("validated"))
}

// ---- shared arrays (#246) ----

/// Which persistent array representations are on (#246, #250), so that one
/// binary measures each: `OKKAMA_ARRAYS=flat` copies as before, `shared`
/// keeps append chains in shared buffers but no tries, anything else (the
/// default) has both.
fn arrays_mode() -> &'static str {
    static MODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    MODE.get_or_init(|| std::env::var("OKKAMA_ARRAYS").unwrap_or_default())
}

pub(crate) fn shared_arrays() -> bool {
    arrays_mode() != "flat"
}

pub(crate) fn trie_arrays() -> bool {
    !matches!(arrays_mode(), "flat" | "shared")
}

/// Tries that are only read flatten themselves (`Heap::trie_read`) unless
/// `OKKAMA_ARRAYS=trie` keeps them tries, so that both can be measured.
fn flattening_tries() -> bool {
    arrays_mode() != "trie"
}

/// `a + b + …` when `a` continues a chain of appends: written in place at
/// the frontier of `a`'s buffer when `a` ends there and there is room, else
/// into a new buffer twice the size. `None` for anything else, which is
/// copied flat as before (and marked [`crate::heap::APPENDED`] when `a` is
/// large, so that the next append starts a chain).
fn append_shared(runtime: &mut Runtime, repr: Repr, words: &[Word], size: usize) -> Option<Word> {
    use crate::heap::{PREPENDED, SHARED, payload};
    if !shared_arrays() || words.len() < 2 {
        return None;
    }
    let first = words[0];
    let header = runtime.heap.header_of(first);
    // A version made by an append or a prepend: a chain either way, also
    // one that alternates ends.
    if !continues_chain(runtime, first, header) {
        return None;
    }
    let length = payload(header) as usize;
    // The appended parts are copied out first: one of them may be a version
    // of the very buffer written below.
    let rest: Vec<Word> = words[1..].iter().flat_map(|&w| runtime.heap.elements(w).into_owned()).collect();
    if header & SHARED != 0 {
        let (buffer, start) = runtime.heap.shared(first);
        let (used, capacity) = runtime.heap.buffer_room(buffer);
        if start + length == used && used + rest.len() <= capacity {
            runtime.heap.buffer_extend(buffer, &rest);
            return Some(runtime.heap.allocate_view(repr, buffer, start, size));
        }
    }
    // A new buffer twice the size; a chain that has grown at the front too
    // keeps a quarter of the room there, so that alternating ends does not
    // copy at every turn.
    let head = runtime.heap.elements(first).into_owned();
    let capacity = (2 * size).next_power_of_two();
    let both = header & PREPENDED != 0 || (header & SHARED != 0 && runtime.heap.shared(first).1 > 0);
    let front = if both { (capacity - size) / 4 } else { 0 };
    let buffer = runtime.heap.allocate_buffer(repr, &[&head, &rest], capacity, front);
    Some(runtime.heap.allocate_view(repr, buffer, front, size))
}

/// Text buffers for chains of `+` on strings (#255) unless
/// `OKKAMA_STRINGS=flat`, which keeps every concatenation a copy.
pub(crate) fn shared_strings() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("OKKAMA_STRINGS").map_or(true, |v| v != "flat"))
}

/// `left + right` on strings (#255): bytes the size of a new allocation,
/// checked against the memory budget first, or `None` when it would exceed
/// it.
///
/// A chain of appends writes at the high end of a text buffer when `left`
/// ends there and there is room, a chain of prepends right before the low
/// end when `right` starts there. A chain that outgrows its buffer moves to
/// one twice the size (three quarters of the room in front for prepends, a
/// quarter for a chain that has grown at both ends), but only when the part
/// added is at most half the other: `s + s`, or a version made from an
/// older one (a branch), is copied flat as before, and so is everything
/// shorter than [`TEXT_CHAIN_MIN`](crate::heap::TEXT_CHAIN_MIN). A flat
/// string made by `+` from a long part is marked
/// [`APPENDED`](crate::heap::APPENDED) or [`PREPENDED`](crate::heap::PREPENDED):
/// its next `+` starts a buffer, and loses the mark, so that more versions
/// made from it (branches) copy instead of each taking a buffer twice its
/// size (x86-64 made that +80…375%).
pub(crate) fn concat_text(runtime: &mut Runtime, left: Word, right: Word, fits: impl Fn(&Runtime, usize) -> bool) -> Option<Word> {
    use crate::heap::{APPENDED, ASCII, PREPENDED, SHARED, TEXT_CHAIN_MIN, payload};
    let (lh, rh) = (runtime.heap.header_of(left), runtime.heap.header_of(right));
    let (ll, rl) = (payload(lh) as usize, payload(rh) as usize);
    let (size, ascii) = (ll + rl, lh & rh & ASCII != 0);
    let chains = shared_strings() && 2 * size <= u32::MAX as usize;
    let chained = |header: u64| header & (APPENDED | PREPENDED | SHARED) != 0;
    // A new buffer for a chain that outgrew its own: the share of the room
    // it keeps in front, as a fraction.
    let mut grow = None;
    // A flat string a buffer is started from: no longer a chain's start.
    let mut spent = None;
    if chains && chained(lh) && ll >= rl {
        if lh & SHARED != 0 {
            let (buffer, start) = runtime.heap.text_shared(left);
            let (_, high, capacity) = runtime.heap.text_buffer_room(buffer);
            if start + ll == high && high + rl <= capacity {
                let text = runtime.heap.text(right) as *const str;
                // SAFETY: `right` stays where it is; the write lies outside it.
                runtime.heap.text_buffer_write(buffer, unsafe { &*text }, false);
                return Some(runtime.heap.allocate_text_view(buffer, start, size, ascii));
            }
            if start + ll == high && 2 * rl <= ll {
                grow = Some(if start > 0 { (1, 4) } else { (0, 1) });
            }
        } else if 2 * rl <= ll {
            grow = Some(if lh & PREPENDED != 0 { (1, 4) } else { (0, 1) });
            spent = Some(left);
        }
    } else if chains && chained(rh) && rl > ll {
        if rh & SHARED != 0 {
            let (buffer, start) = runtime.heap.text_shared(right);
            let (low, _, _) = runtime.heap.text_buffer_room(buffer);
            if start == low && start >= ll {
                let text = runtime.heap.text(left) as *const str;
                // SAFETY: as above.
                runtime.heap.text_buffer_write(buffer, unsafe { &*text }, true);
                return Some(runtime.heap.allocate_text_view(buffer, start - ll, size, ascii));
            }
            if start == low && 2 * ll <= rl {
                grow = Some((3, 4));
            }
        } else if 2 * ll <= rl {
            grow = Some((3, 4));
            spent = Some(right);
        }
    }
    let (l, r) = (runtime.heap.text(left) as *const str, runtime.heap.text(right) as *const str);
    // SAFETY: allocation neither moves nor collects the parts.
    let parts: [&str; 2] = unsafe { [&*l, &*r] };
    if let Some((share, of)) = grow {
        let capacity = (2 * size).next_power_of_two();
        if !fits(runtime, capacity) {
            return None;
        }
        let front = (capacity - size) * share / of;
        if let Some(string) = spent {
            runtime.heap.unmark(string, APPENDED | PREPENDED);
        }
        let buffer = runtime.heap.allocate_text_buffer(&parts, capacity, front);
        return Some(runtime.heap.allocate_text_view(buffer, front, size, ascii));
    }
    if !fits(runtime, size) {
        return None;
    }
    let string = runtime.heap.allocate_text_of(&parts);
    if chains && ll >= TEXT_CHAIN_MIN {
        runtime.heap.mark(string, APPENDED);
    } else if chains && rl >= TEXT_CHAIN_MIN {
        runtime.heap.mark(string, PREPENDED);
    }
    Some(string)
}

/// Was `array` made by an append or a prepend (a flat copy marked so, or a
/// view of a buffer)? A view of a plain array — a slice (#249) or a
/// flattened trie — is not: concatenating it is a one-off and copies once.
fn continues_chain(runtime: &Runtime, array: Word, header: u64) -> bool {
    use crate::heap::{APPENDED, PREPENDED, SHARED};
    header & (APPENDED | PREPENDED) != 0 || (header & SHARED != 0 && runtime.heap.is_buffer(runtime.heap.shared(array).0))
}

/// A slice (and `take`, `drop`, which lower to it) that shares its origin's
/// storage (#249): a view of the flat array, or of the buffer of a view,
/// when the slice keeps at least half of that storage, so a slice never
/// keeps alive more than twice what it holds. Views always point at the
/// storage itself, never at another view. Smaller slices, short ones and
/// slices of tries are copied, as before. `rest = items.drop(1)` in a loop
/// shares until half is left, copies that half once, and so on: O(n) in all.
fn slice_shared(runtime: &mut Runtime, array: Word, repr: Repr, range: std::ops::Range<usize>) -> Option<Word> {
    use crate::heap::{CHAIN_MIN, SHARED, TRIE, payload};
    let header = runtime.heap.header_of(array);
    if !shared_arrays() || range.len() < CHAIN_MIN || header & TRIE != 0 {
        return None;
    }
    let (storage, start, kept) = if header & SHARED != 0 {
        let (buffer, start) = runtime.heap.shared(array);
        let (used, capacity) = runtime.heap.buffer_room(buffer);
        (buffer, start, capacity.max(used))
    } else {
        (array, 0, payload(header) as usize)
    };
    if 2 * range.len() < kept {
        return None;
    }
    Some(runtime.heap.allocate_view(repr, storage, start + range.start, range.len()))
}

/// `… + [x] + items` when `items` continues a chain of prepends (#247): the
/// mirror of [`append_shared`]. Written in place right before `items` when
/// it starts at its buffer's lowest written slot and there is room, else
/// into a new buffer twice the size with three quarters of its room in
/// front. `None` for anything else.
fn prepend_shared(runtime: &mut Runtime, repr: Repr, words: &[Word], size: usize) -> Option<Word> {
    use crate::heap::{SHARED, payload};
    if !shared_arrays() || words.len() < 2 {
        return None;
    }
    let last = words[words.len() - 1];
    let header = runtime.heap.header_of(last);
    if !continues_chain(runtime, last, header) {
        return None;
    }
    let length = payload(header) as usize;
    let front: Vec<Word> = words[..words.len() - 1].iter().flat_map(|&w| runtime.heap.elements(w).into_owned()).collect();
    if header & SHARED != 0 {
        let (buffer, start) = runtime.heap.shared(last);
        if runtime.heap.buffer_front_free(buffer, start, front.len(), repr) {
            runtime.heap.buffer_prepend(buffer, start, &front);
            return Some(runtime.heap.allocate_view(repr, buffer, start - front.len(), size));
        }
    }
    let tail = runtime.heap.elements(last).into_owned();
    let capacity = (2 * size).next_power_of_two();
    let room = (capacity - size) * 3 / 4;
    let buffer = runtime.heap.allocate_buffer(repr, &[&front, &tail], capacity, room);
    debug_assert_eq!(length, tail.len());
    Some(runtime.heap.allocate_view(repr, buffer, room, size))
}

// ---- structural equality ----

/// A new array for register `dst` of `function`: byte arrays (`@byte`) are
/// packed, whatever produced the words.
pub(crate) fn new_array(runtime: &mut Runtime, function: usize, dst: okkama_sparrow::Register, repr: Repr, items: &[Word]) -> Word {
    let program = &runtime.loaded.program;
    let ty = program.functions[function].registers[dst.0 as usize];
    if let Some(Type::Array(element)) = program.ty(ty)
        && Kind::of(program, *element) == Kind::Byte {
            let bytes: Vec<u8> = items.iter().map(|&w| w as u8).collect();
            return runtime.heap.allocate_bytes(&bytes);
        }
    runtime.heap.allocate_array(repr, items)
}

pub(crate) fn equal(runtime: &Runtime, left: Word, right: Word, ty: TypeRef) -> Result<bool, String> {
    let program = &runtime.loaded.program;
    let mut pending = vec![(left, right, ty)];
    // Visited pairs, only once a composite value is entered.
    let mut seen: crate::word::FastSet<(Word, Word)> = Default::default();
    while let Some((a, b, ty)) = pending.pop() {
        let kind = runtime.kind_of(ty);
        match kind {
            Kind::Int => {
                if runtime.big_value(a) != runtime.big_value(b) {
                    return Ok(false);
                }
                continue;
            }
            // Numerically, as scalar `==` (#314): `-0.0 == 0.0`.
            Kind::Float => {
                if f64::from_bits(a) != f64::from_bits(b) {
                    return Ok(false);
                }
                continue;
            }
            Kind::Raw | Kind::Byte => {
                if a != b {
                    return Ok(false);
                }
                continue;
            }
            Kind::Ref => {}
        }
        if a == UNBOUND || b == UNBOUND {
            return Err("cannot compare internal or undefined values".into());
        }
        match program.ty(ty) {
            Some(Type::Any) => {
                if a == b {
                    continue;
                }
                return Err("Cannot compare any without narrowing".into());
            }
            Some(Type::Str) => {
                if runtime.heap.text(a) != runtime.heap.text(b) {
                    return Ok(false);
                }
            }
            Some(Type::Rat) => {
                if runtime.rational(a) != runtime.rational(b) {
                    return Ok(false);
                }
            }
            Some(Type::Range) => {
                let (x, y) = (
                    runtime.heap.payload::<RangeValue>(a).expect("range"),
                    runtime.heap.payload::<RangeValue>(b).expect("range"),
                );
                if x.inclusive != y.inclusive
                    || runtime.big_value(x.start) != runtime.big_value(y.start)
                    || runtime.big_value(x.end) != runtime.big_value(y.end)
                {
                    return Ok(false);
                }
            }
            Some(Type::Array(_)) if runtime.heap.is_bytes(a) && runtime.heap.is_bytes(b) => {
                if runtime.heap.bytes(a) != runtime.heap.bytes(b) {
                    return Ok(false);
                }
            }
            Some(Type::Array(element)) => {
                let (x, y) = (runtime.heap.elements(a), runtime.heap.elements(b));
                if x.len() != y.len() {
                    return Ok(false);
                }
                // Scalar elements compare in place, without the pair stack.
                match runtime.kind_of(*element) {
                    Kind::Float => {
                        if !x.iter().zip(y.iter()).all(|(&x, &y)| f64::from_bits(x) == f64::from_bits(y)) {
                            return Ok(false);
                        }
                    }
                    Kind::Raw | Kind::Byte => {
                        if x != y {
                            return Ok(false);
                        }
                    }
                    Kind::Int | Kind::Ref => {
                        if a != b && seen.insert((a, b)) {
                            pending.extend(x.iter().zip(y.iter()).rev().map(|(&x, &y)| (x, y, *element)));
                        }
                    }
                }
            }
            Some(Type::Map(_, value)) => {
                let (x, y) = (
                    runtime.heap.payload::<MapValue>(a).expect("map"),
                    runtime.heap.payload::<MapValue>(b).expect("map"),
                );
                if x.entries.len() != y.entries.len() {
                    return Ok(false);
                }
                if a != b && seen.insert((a, b)) {
                    // Scalar values compare in place, as array elements do.
                    let kind = runtime.kind_of(*value);
                    for ((k, (_, v)), (k2, (_, v2))) in x.entries.iter().zip(y.entries.iter()) {
                        if k != k2 {
                            return Ok(false);
                        }
                        match kind {
                            Kind::Float if f64::from_bits(*v) != f64::from_bits(*v2) => return Ok(false),
                            Kind::Raw | Kind::Byte if v != v2 => return Ok(false),
                            Kind::Float | Kind::Raw | Kind::Byte => {}
                            Kind::Int | Kind::Ref => pending.push((*v, *v2, *value)),
                        }
                    }
                }
            }
            Some(Type::Object(_) | Type::Successor(_) | Type::Role(_) | Type::Variant(_)) => {
                let (ha, hb) = (runtime.heap.header_of(a), runtime.heap.header_of(b));
                if heap::kind(ha) != heap::OBJECT || heap::kind(hb) != heap::OBJECT {
                    if a != b {
                        return Ok(false);
                    }
                    continue;
                }
                if heap::payload(ha) != heap::payload(hb) {
                    return Ok(false);
                }
                if a != b && seen.insert((a, b)) {
                    let fields = &program.objects[heap::payload(ha) as usize].fields;
                    for (index, (_, field)) in fields.iter().enumerate().rev() {
                        let (x, y) = (runtime.heap.field(a, index), runtime.heap.field(b, index));
                        let unbound = |w: Word| match runtime.kind_of(*field) {
                            Kind::Float | Kind::Raw | Kind::Byte => w == word::UNBOUND_RAW,
                            _ => w == UNBOUND,
                        };
                        if unbound(x) || unbound(y) {
                            return Err("cannot compare unbound object fields".into());
                        }
                        pending.push((x, y, *field));
                    }
                }
            }
            _ => {
                // Handles, futures, closures, ranges: identity.
                if a != b {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

// ---- the helper ----

/// `builder.push`: append a word to a builder (no collection here: the
/// items vector is Rust memory). Returns the builder.
#[inline(never)]
pub extern "C" fn rt_builder_push(ctx: *mut Ctx, builder: Word, item: Word) -> Word {
    let runtime = unsafe { &mut *(*ctx).runtime };
    grow_builder(&mut runtime.heap, builder, |items| items.push(item));
    builder
}

/// Element `position` (in bounds) of a trie array (#250), for machine code.
#[inline(never)]
pub extern "C" fn rt_trie_get(ctx: *mut Ctx, array: Word, position: u64) -> Word {
    let runtime = unsafe { &mut *(*ctx).runtime };
    if flattening_tries() {
        runtime.heap.trie_read(array, position as usize)
    } else {
        runtime.heap.trie_get(array, position as usize)
    }
}

/// Grow a builder's items and charge the capacity its vector took on
/// (#331): O(1), and nothing at all while the capacity holds.
fn grow_builder(heap: &mut crate::heap::Heap, builder: Word, grow: impl FnOnce(&mut Vec<Word>)) {
    let grown = {
        let items = &mut heap.payload_mut::<Builder>(builder).expect("builder").items;
        let before = items.capacity();
        grow(items);
        items.capacity().saturating_sub(before)
    };
    if grown > 0 {
        heap.charge_external(grown * 8);
    }
}

/// Execute instruction `pc` of `function` with its used words at `uses`.
///
/// SAFETY: called from machine code with its context; `uses` holds one word
/// per `Access::Use` of the instruction, in `visit` order.
#[inline(never)]
pub extern "C" fn rt_exec(
    ctx: *mut Ctx,
    function: u64,
    pc: u64,
    uses: *const Word,
    registers: *const okkama_sparrow::Register,
    count: u64,
) -> Word {
    let frame = crate::runtime::frame_pointer!();
    let ctx = unsafe { &mut *ctx };
    let runtime = unsafe { &mut *ctx.runtime };
    let function = function as usize;
    // SAFETY: the loaded program is never mutated while machine code runs, so
    // the instruction stays valid while the runtime is borrowed mutably.
    let instruction: &I = unsafe { &*(&runtime.loaded.program.functions[function].code[pc as usize] as *const I) };
    runtime.count(|_| format!("exec {}", instruction.mnemonic()));
    // The instruction's uses in visit order, as the call site recorded them
    // when it was compiled, and one word for each.
    // SAFETY: the table lives with the machine code; the caller spilled one
    // word per register.
    let registers = unsafe { std::slice::from_raw_parts(registers, count as usize) };
    let words = unsafe { std::slice::from_raw_parts(uses, count as usize) };
    let value = |register: &okkama_sparrow::Register| {
        words[registers.iter().position(|r| r == register).expect("used register")]
    };
    if runtime.limits.memory.is_some()
        && let Some(bytes) = result_bytes(runtime, instruction, &value)
            && runtime.would_exceed(bytes) {
                return runtime.exhaust(ctx, crate::budget::Exhausted::Memory);
            }
    let outcome = match instruction {
        // Spawning runs the worker's driver now: it needs the context.
        I::WorkerSpawn { entry, args, direct, .. } => {
            let arguments: Vec<Word> = args.iter().map(value).collect();
            match &runtime.limits.natives {
                Some(allowed) if !allowed.contains(&crate::budget::NativeGroup::Process) => {
                    Err("Process workers need the Process permission".to_string())
                }
                _ => crate::worker::spawn(runtime, &mut *ctx, value(entry), &arguments, *direct),
            }
        }
        // The init of a constant runs machine code: it needs the context.
        I::GlobalGet { global, .. } => crate::intrinsics::global(ctx, runtime, *global as usize),
        _ => execute(runtime, function, instruction, &value),
    };
    match outcome {
        Ok(word) => {
            if runtime.heap.needs_collection() {
                let repr = def_repr(runtime, function, instruction);
                runtime.safepoint(frame, ctx, &[(word, repr)]);
            }
            word
        }
        Err(message) => {
            ctx.failure = runtime.failure_new(FailureKind::Program(message));
            ctx.status = crate::runtime::FAILED;
            0
        }
    }
}

/// The size of the result of an instruction whose result can be much larger
/// than its operands (#72) — a join, a replacement — before it is built,
/// so a memory budget stops it instead of the allocation it would make.
fn result_bytes(runtime: &Runtime, instruction: &I, value: &dyn Fn(&okkama_sparrow::Register) -> Word) -> Option<u64> {
    let text = |register| runtime.heap.text(value(register));
    match instruction {
        I::ArrayJoin { array, separator, .. } => {
            let elements = runtime.heap.elements(value(array));
            let parts: u64 = elements.iter().map(|&element| runtime.heap.text(element).len() as u64).sum();
            Some(parts + text(separator).len() as u64 * elements.len().saturating_sub(1) as u64)
        }
        I::StrReplace { text: source, pattern, replacement, .. } => {
            let (source, pattern, replacement) = (text(source), text(pattern), text(replacement));
            let count = if pattern.is_empty() { source.chars().count() + 1 } else { source.matches(pattern).count() };
            Some(source.len() as u64 + count as u64 * replacement.len() as u64)
        }
        _ => None,
    }
}

fn def_repr(runtime: &Runtime, function: usize, instruction: &I) -> Repr {
    let mut repr = Repr::Raw;
    instruction.visit(&mut |access| {
        if let Access::Def(register) = access {
            repr = runtime.loaded.functions[function].kinds[register.0 as usize].repr();
        }
    });
    repr
}

fn execute(runtime: &mut Runtime, function: usize, instruction: &I, value: &dyn Fn(&okkama_sparrow::Register) -> Word) -> Outcome {
    // SAFETY: as in rt_exec, the loaded program is immutable during execution.
    let kinds: &[Kind] = unsafe { &*(runtime.loaded.functions[function].kinds.as_slice() as *const [Kind]) };
    match instruction {
        I::IntAnd { left, right, .. }
        | I::IntOr { left, right, .. }
        | I::IntXor { left, right, .. }
        | I::IntShl { left, right, .. }
        | I::IntShr { left, right, .. }
        | I::IntUshr { left, right, .. } => {
            let shift = matches!(instruction, I::IntShl { .. } | I::IntShr { .. } | I::IntUshr { .. });
            let r = if shift {
                runtime
                    .small_or_big(value(right))
                    .filter(|n| (0..64).contains(n))
                    .ok_or("Shift count must be in 0..63")?
            } else {
                runtime.small_or_big(value(right)).ok_or("Bitwise operand is outside signed-64 range")?
            };
            let l = runtime.small_or_big(value(left)).ok_or("Bitwise operand is outside signed-64 range")?;
            let result = match instruction {
                I::IntAnd { .. } => l & r,
                I::IntOr { .. } => l | r,
                I::IntXor { .. } => l ^ r,
                I::IntShl { .. } => l << r,
                I::IntShr { .. } => l >> r,
                _ => ((l as u64) >> r) as i64,
            };
            Ok(runtime.heap.int(result))
        }
        I::ByteAnd { left, right, .. }
        | I::ByteOr { left, right, .. }
        | I::ByteXor { left, right, .. }
        | I::ByteShl { left, right, .. }
        | I::ByteShr { left, right, .. }
        | I::ByteUshr { left, right, .. } => {
            let l = value(left) as u8;
            let result = match instruction {
                I::ByteAnd { .. } => l & value(right) as u8,
                I::ByteOr { .. } => l | value(right) as u8,
                I::ByteXor { .. } => l ^ value(right) as u8,
                _ => {
                    let count = runtime
                        .small_or_big(value(right))
                        .filter(|n| (0..8).contains(n))
                        .ok_or("Shift count must be in 0..7")?;
                    if matches!(instruction, I::ByteShl { .. }) { l << count } else { l >> count }
                }
            };
            Ok(Word::from(result))
        }
        I::ByteNot { operand, .. } => Ok(Word::from(!(value(operand) as u8))),
        I::Bits { op, byte, value: source, count, .. } => {
            use okkama_sparrow::BitOp;
            let word = if *byte {
                value(source) as i64
            } else {
                runtime.small_or_big(value(source)).ok_or("Bitwise operand is outside signed-64 range")?
            };
            let rotation = || -> Result<u32, String> {
                count
                    .and_then(|c| runtime.small_or_big(value(&c)))
                    .filter(|n| (0..if *byte { 8 } else { 64 }).contains(n))
                    .map(|n| n as u32)
                    .ok_or_else(|| if *byte { "Shift count must be in 0..7" } else { "Shift count must be in 0..63" }.into())
            };
            let result = if *byte {
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
            Ok(if *byte && !op.counts() { result as u8 as Word } else { runtime.heap.int(result) })
        }
        I::RangeNew { start, end, inclusive, .. } => {
            let (start, end) = (value(start), value(end));
            if runtime.big_value(start) > runtime.big_value(end) {
                return Err("Range start must be less than or equal to range end".into());
            }
            Ok(runtime.heap.allocate_payload(Box::new(RangeValue { start, end, inclusive: *inclusive })))
        }
        I::RangePart { range, end, .. } => {
            let range = runtime.heap.payload::<RangeValue>(value(range)).expect("range");
            let (start, bound, inclusive) = (range.start, range.end, range.inclusive);
            if !*end {
                Ok(start)
            } else if !inclusive {
                Ok(bound)
            } else {
                let next = runtime.big_value(bound) + 1;
                Ok(integer_word(runtime, next))
            }
        }
        I::StrUtf8 { text, .. } => {
            // SAFETY of the copy: the text is a live heap string.
            let bytes = runtime.heap.text(value(text)).as_bytes().to_vec();
            Ok(runtime.heap.allocate_bytes(&bytes))
        }
        I::StrFn { op, args, .. } => str_fn(runtime, *op, args.iter().map(value).collect()),
        I::StrReplaceFirst { text, pattern, replacement, .. } => {
            let replaced = runtime
                .heap
                .text(value(text))
                .replacen(runtime.heap.text(value(pattern)), runtime.heap.text(value(replacement)), 1);
            Ok(runtime.text_new(&replaced))
        }
        I::IntNot { operand, .. } => {
            let n = runtime.small_or_big(value(operand)).ok_or("Bitwise operand is outside signed-64 range")?;
            Ok(runtime.heap.int(!n))
        }
        I::IntNeg { operand, .. } => {
            let n = -runtime.big_value(value(operand));
            Ok(integer_word(runtime, n))
        }
        I::IntAbs { operand, .. } => {
            let n = runtime.big_value(value(operand)).abs();
            Ok(integer_word(runtime, n))
        }
        I::Sign { operand, .. } => {
            let word = value(operand);
            let sign = match kinds[operand.0 as usize] {
                Kind::Int => match runtime.big_value(word).sign() {
                    num_bigint::Sign::Minus => -1,
                    num_bigint::Sign::NoSign => 0,
                    num_bigint::Sign::Plus => 1,
                },
                Kind::Float => {
                    let n = f64::from_bits(word);
                    if n > 0.0 { 1 } else if n < 0.0 { -1 } else { 0 }
                }
                _ => runtime.rational(word).sign(),
            };
            Ok(runtime.heap.int(sign))
        }
        I::FloatMod { left, right, .. } | I::FloatPow { left, right, .. } => {
            let op = if matches!(instruction, I::FloatMod { .. }) {
                crate::scalar::FLOAT_MOD
            } else {
                crate::scalar::FLOAT_POW
            };
            crate::scalar::float_binary(op, f64::from_bits(value(left)), f64::from_bits(value(right)))
                .map(f64::to_bits)
                .map_err(Into::into)
        }
        I::FloatSqrt { operand, .. } => {
            let n = f64::from_bits(value(operand));
            if n < 0.0 {
                return Err(format!("Can't take sqrt of {}", crate::scalar::float_text(n)));
            }
            finite(n.sqrt())
        }
        I::Convert { dst, source } => convert(runtime, function, *source, dst.0, value(source)),
        I::ByteFrom { source, context, .. } => byte_from(runtime, kinds[source.0 as usize], value(source), context.as_deref()),
        I::RatNew { left, right, .. } => {
            let ratio = Rat::new(&runtime.int_value(value(left)), &runtime.int_value(value(right)))?;
            Ok(runtime.rational_new(ratio))
        }
        I::RatAdd { left, right, .. }
        | I::RatSub { left, right, .. }
        | I::RatMul { left, right, .. }
        | I::RatDiv { left, right, .. }
        | I::RatMod { left, right, .. } => {
            // Borrowed operands: the result is computed before the runtime
            // is borrowed again to allocate it (#326).
            let result = {
                let (a, b) = (runtime.rational(value(left)), runtime.rational(value(right)));
                match instruction {
                    I::RatAdd { .. } => a.add(b),
                    I::RatSub { .. } => a.sub(b),
                    I::RatMul { .. } => a.mul(b),
                    I::RatDiv { .. } => a.div(b)?,
                    _ => a.modulo(b)?,
                }
            };
            Ok(runtime.rational_new(result))
        }
        I::RatNeg { operand, .. } | I::RatAbs { operand, .. } => {
            let n = runtime.rational(value(operand));
            let result = if matches!(instruction, I::RatNeg { .. }) { n.neg() } else { n.abs() };
            Ok(runtime.rational_new(result))
        }
        I::RatCmp { op, left, right, .. } => {
            let order = runtime.rational(value(left)).compare(runtime.rational(value(right)));
            Ok(compare_flag(*op, order))
        }
        I::RatPart { value: source, denominator, .. } => {
            let ratio = runtime.rational(value(source));
            let text = if *denominator { ratio.denominator() } else { ratio.numerator() }.to_string();
            Ok(runtime.text_new(&text))
        }
        I::StrCmp { op, left, right, .. } => {
            let order = runtime.heap.text(value(left)).cmp(runtime.heap.text(value(right)));
            Ok(compare_flag(*op, order))
        }
        I::StrLen { text, .. } => {
            let length = runtime.heap.char_count(value(text));
            Ok(runtime.heap.int(length as i64))
        }
        I::StrSlice { text, start, end, .. } => {
            let text = value(text);
            // Code points map to bytes from the last position reached (#52).
            let count = runtime.heap.char_count(text);
            let range = slice_bounds(runtime, count, value(start), value(end))?;
            let first = runtime.heap.char_offset(text, range.start);
            let last = runtime.heap.char_offset(text, range.end);
            Ok(runtime.heap.substring(text, first, last))
        }
        I::StrSplit { text, separator, .. } => {
            let (text, separator) = (value(text), value(separator));
            // Byte ranges of the parts, then one copy per part.
            let ranges: Vec<(usize, usize)> = {
                let (source, separator) = (runtime.heap.text(text), runtime.heap.text(separator));
                if separator.is_empty() {
                    source.char_indices().map(|(i, c)| (i, i + c.len_utf8())).collect()
                } else {
                    let mut ranges = Vec::new();
                    let mut start = 0;
                    for (at, matched) in source.match_indices(separator) {
                        ranges.push((start, at));
                        start = at + matched.len();
                    }
                    ranges.push((start, source.len()));
                    ranges
                }
            };
            let words: Vec<Word> = ranges.into_iter().map(|(a, b)| runtime.heap.substring(text, a, b)).collect();
            Ok(runtime.heap.allocate_array(Repr::Ref, &words))
        }
        I::StrReplace { text, pattern, replacement, .. } => {
            let replaced = runtime
                .heap
                .text(value(text))
                .replace(runtime.heap.text(value(pattern)), runtime.heap.text(value(replacement)));
            Ok(runtime.text_new(&replaced))
        }
        I::StrTrim { text, .. } => {
            let trimmed = runtime.heap.text(value(text)).trim_matches(char::is_whitespace).to_owned();
            Ok(runtime.text_new(&trimmed))
        }
        I::StrChomp { text, .. } => {
            let source = runtime.heap.text(value(text));
            let chomped = source
                .strip_suffix("\r\n")
                .or_else(|| source.strip_suffix('\n'))
                .unwrap_or(source)
                .to_owned();
            Ok(runtime.text_new(&chomped))
        }
        I::EnumCase { ty, case, .. } => Ok((u64::from(ty.0) << 32) | u64::from(*case)),
        I::EnumCmp { op, left, right, .. } => Ok(compare_flag(*op, value(left).cmp(&value(right)))),
        I::ValCmp { op, left, right, .. } => {
            let ty = runtime.loaded.program.functions[function].registers[left.0 as usize];
            let equal = equal(runtime, value(left), value(right), ty)?;
            Ok(bool_word(if matches!(op, Cmp::Ne) { !equal } else { equal }))
        }
        I::Undef { dst } => Ok(if matches!(kinds[dst.0 .0 as usize], Kind::Float | Kind::Raw | Kind::Byte) {
            word::UNBOUND_RAW
        } else {
            UNBOUND
        }),
        I::AnyBox { source, .. } => {
            let ty = runtime.loaded.program.functions[function].registers[source.0 as usize];
            let repr = kinds[source.0 as usize].repr();
            let boxed = AnyBox { value: value(source), ty, repr };
            Ok(runtime.heap.allocate_payload(Box::new(boxed)))
        }
        I::ArraySlice { array, start, end, .. } => {
            let array = value(array);
            let length = crate::heap::payload(runtime.heap.header_of(array)) as usize;
            let range = slice_bounds(runtime, length, value(start), value(end))?;
            // Copy only the range.
            if runtime.heap.is_bytes(array) {
                let bytes = runtime.heap.bytes(array)[range].to_vec();
                return Ok(runtime.heap.allocate_bytes(&bytes));
            }
            let repr = element_repr(runtime, array);
            if let Some(view) = slice_shared(runtime, array, repr, range.clone()) {
                return Ok(view);
            }
            let items: Vec<Word> = runtime.heap.elements(array)[range].to_vec();
            Ok(runtime.heap.allocate_array(repr, &items))
        }
        I::ArrayRemove { array, index, .. } => {
            // A copy without one element (#251); `arr.get`'s bounds.
            let (array, index) = (value(array), value(index));
            let length = crate::heap::payload(runtime.heap.header_of(array)) as usize;
            let at = word::small_value(index)
                .and_then(|i| usize::try_from(i).ok())
                .filter(|&i| i < length)
                .ok_or_else(|| "Array index out of bounds".to_string())?;
            if runtime.heap.is_bytes(array) {
                let mut bytes = runtime.heap.bytes(array).to_vec();
                bytes.remove(at);
                return Ok(runtime.heap.allocate_bytes(&bytes));
            }
            let repr = element_repr(runtime, array);
            let mut items = runtime.heap.elements(array).into_owned();
            items.remove(at);
            Ok(runtime.heap.allocate_array(repr, &items))
        }
        I::ArrayReplace { array, index, value: item, .. } => {
            // A copy with one element replaced (#250); `arr.get`'s bounds.
            let (array, index, item) = (value(array), value(index), value(item));
            let length = crate::heap::payload(runtime.heap.header_of(array)) as usize;
            let at = word::small_value(index)
                .and_then(|i| usize::try_from(i).ok())
                .filter(|&i| i < length)
                .ok_or_else(|| "Array index out of bounds".to_string())?;
            if runtime.heap.is_bytes(array) {
                let mut bytes = runtime.heap.bytes(array).to_vec();
                bytes[at] = item as u8;
                return Ok(runtime.heap.allocate_bytes(&bytes));
            }
            let repr = element_repr(runtime, array);
            let header = runtime.heap.header_of(array);
            // A chain of replacements in a large array becomes a trie, which
            // copies one path per replacement instead of the array (#250).
            if trie_arrays() && header & crate::heap::TRIE != 0 {
                return Ok(runtime.heap.trie_set(array, at, item));
            }
            let mut items = runtime.heap.elements(array).into_owned();
            items[at] = item;
            if trie_arrays() && length >= crate::heap::TRIE_MIN {
                if header & crate::heap::REPLACED != 0 {
                    return Ok(runtime.heap.trie_from(repr, &items));
                }
                let copy = runtime.heap.allocate_array(repr, &items);
                runtime.heap.mark(copy, crate::heap::REPLACED);
                return Ok(copy);
            }
            Ok(runtime.heap.allocate_array(repr, &items))
        }
        I::ArrayJoin { array, separator, .. } => {
            let separator = runtime.heap.text(value(separator)).to_owned();
            let mut text = String::new();
            for (i, &element) in runtime.heap.elements(value(array)).iter().enumerate() {
                if i != 0 {
                    text.push_str(&separator);
                }
                text.push_str(runtime.heap.text(element));
            }
            Ok(runtime.text_new(&text))
        }
        I::BuilderNew { dst } => {
            let repr = match runtime.ty(function, dst.0) {
                Some(Type::Builder(element)) => runtime.kind_of(*element).repr(),
                _ => return Err("builder.new needs a builder type".into()),
            };
            Ok(runtime.heap.allocate_payload(Box::new(Builder { repr, items: Vec::new() })))
        }
        I::BuilderPush { builder, value: item, .. } => {
            let (builder, item) = (value(builder), value(item));
            grow_builder(&mut runtime.heap, builder, |items| items.push(item));
            Ok(builder)
        }
        I::BuilderAppend { builder, array, .. } => {
            let builder = value(builder);
            let items = runtime.heap.elements(value(array)).to_vec();
            grow_builder(&mut runtime.heap, builder, |all| all.extend(items));
            Ok(builder)
        }
        I::BuilderRange { builder, start, end, inclusive, .. } => {
            let builder = value(builder);
            let (start, end) = (runtime.big_value(value(start)), runtime.big_value(value(end)));
            if start > end {
                return Err("Range start must be less than or equal to range end".into());
            }
            let mut n = start;
            let mut items = Vec::new();
            while if *inclusive { n <= end } else { n < end } {
                items.push(integer_word(runtime, n.clone()));
                n += 1;
            }
            grow_builder(&mut runtime.heap, builder, |all| all.extend(items));
            Ok(builder)
        }
        I::BuilderFinish { dst, builder } => {
            let builder = runtime.heap.payload_mut::<Builder>(value(builder)).expect("builder");
            let (repr, items) = (builder.repr, std::mem::take(&mut builder.items));
            Ok(new_array(runtime, function, dst.0, repr, &items))
        }
        I::MapNew { dst, entries } => {
            let mut map = map_for(runtime, function, dst.0)?;
            let (key_kind, key_type) = map_key(runtime, function, dst.0);
            // What the entries own, counted as they go in, so the heap does
            // not walk the keys again to charge them (#325).
            let mut owned = 0;
            for (key, item) in entries {
                let (key, item) = (value(key), value(item));
                let k = runtime.checked_key(key, key_kind, key_type.as_ref())?;
                let bytes = MAP_ENTRY_BYTES + k.owned_bytes();
                if map.entries.upsert(k, (key, item)) {
                    owned += bytes;
                }
            }
            map.owned.set(Some(owned));
            Ok(runtime.heap.allocate_payload(Box::new(map)))
        }
        I::ArrayConcat { dst, parts } => {
            let words: Vec<Word> = parts.iter().map(value).collect();
            let element = match runtime.ty(function, dst.0) {
                Some(Type::Array(element)) => *element,
                _ => return Err("arr.concat needs an array type".into()),
            };
            // The result is allocated once, for the sum of the parts' lengths.
            let size: usize = words.iter().map(|&w| crate::heap::payload(runtime.heap.header_of(w)) as usize).sum();
            if runtime.kind_of(element) == Kind::Byte {
                let mut bytes = Vec::with_capacity(size);
                for &w in &words {
                    bytes.extend_from_slice(&runtime.heap.byte_view(w));
                }
                return Ok(runtime.heap.allocate_bytes(&bytes));
            }
            let repr = runtime.kind_of(element).repr();
            if let Some(array) = append_shared(runtime, repr, &words, size) {
                return Ok(array);
            }
            if let Some(array) = prepend_shared(runtime, repr, &words, size) {
                return Ok(array);
            }
            let mut items = Vec::with_capacity(size);
            for &w in &words {
                items.extend_from_slice(&runtime.heap.elements(w));
            }
            let array = new_array(runtime, function, dst.0, repr, &items);
            let first = crate::heap::payload(runtime.heap.header_of(words[0])) as usize;
            let last = crate::heap::payload(runtime.heap.header_of(words[words.len() - 1])) as usize;
            if shared_arrays() && words.len() > 1 && first < crate::heap::CHAIN_MIN && last >= crate::heap::CHAIN_MIN {
                runtime.heap.mark(array, crate::heap::PREPENDED);
            }
            if shared_arrays() && words.len() > 1 && first >= crate::heap::CHAIN_MIN {
                runtime.heap.mark_appended(array);
            }
            Ok(array)
        }
        I::ArrayDiff { dst, array, removed } => {
            let element = match runtime.ty(function, dst.0) {
                Some(Type::Array(element)) => *element,
                _ => return Err("arr.diff needs an array type".into()),
            };
            let kind = runtime.kind_of(element);
            let ty = runtime.loaded.program.ty(element).cloned();
            let mut excluded = Vec::new();
            for part in removed {
                excluded.extend_from_slice(&runtime.heap.elements(value(part)));
            }
            let items = runtime.heap.elements(value(array)).into_owned();
            // Scalars and strings compare by key (a float key is numeric, so
            // `-0.0` and `0.0` are one, #314); other elements deeply.
            let keyed = matches!(kind, Kind::Int | Kind::Float | Kind::Raw | Kind::Byte) || ty == Some(Type::Str);
            let mut kept = Vec::with_capacity(items.len());
            if keyed {
                let keys: std::collections::BTreeSet<_> = excluded.iter().map(|&w| runtime.key(w, kind, ty.as_ref())).collect();
                kept.extend(items.into_iter().filter(|&w| !keys.contains(&runtime.key(w, kind, ty.as_ref()))));
            } else {
                for item in items {
                    let mut found = false;
                    for &other in &excluded {
                        if equal(runtime, item, other, element)? {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        kept.push(item);
                    }
                }
            }
            if kind == Kind::Byte {
                let bytes: Vec<u8> = kept.iter().map(|&w| w as u8).collect();
                return Ok(runtime.heap.allocate_bytes(&bytes));
            }
            Ok(new_array(runtime, function, dst.0, kind.repr(), &kept))
        }
        I::MapMerge { dst, parts } => {
            // The first part as it is (O(1) for a persistent map), then each
            // later part's entries: the rightmost value wins, the leftmost key
            // word stays (#254).
            let mut map = map_for(runtime, function, dst.0)?;
            // A large standard map a version is made from turns persistent
            // in place, so that this and later versions share its nodes.
            for part in parts {
                runtime.heap.payload_mut::<MapValue>(value(part)).expect("map").entries.make_persistent();
            }
            let mut parts = parts.iter().map(|part| &runtime.heap.payload::<MapValue>(value(part)).expect("map").entries);
            if let Some(first) = parts.next() {
                map.entries = first.clone();
            }
            for entries in parts {
                map.entries.merge(entries);
            }
            Ok(runtime.heap.allocate_payload(Box::new(map)))
        }
        I::MapDiff { dst, map: base, removed } => {
            let mut map = map_for(runtime, function, dst.0)?;
            runtime.heap.payload_mut::<MapValue>(value(base)).expect("map").entries.make_persistent();
            map.entries = runtime.heap.payload::<MapValue>(value(base)).expect("map").entries.clone();
            for part in removed {
                let entries = &runtime.heap.payload::<MapValue>(value(part)).expect("map").entries;
                for key in entries.keys() {
                    map.entries.remove(key);
                }
            }
            Ok(runtime.heap.allocate_payload(Box::new(map)))
        }
        I::MapBuilderNew { dst } => Ok({
            let map = map_for(runtime, function, dst.0)?;
            runtime.heap.allocate_payload(Box::new(map))
        }),
        I::MapBuilderPut { builder, key, value: item, .. } => {
            let (builder, key, item) = (value(builder), value(key), value(item));
            let (key_kind, key_type) = map_key(runtime, function, *builder_register(instruction));
            let k = runtime.checked_key(key, key_kind, key_type.as_ref())?;
            let map = runtime.heap.payload_mut::<MapValue>(builder).expect("map builder");
            // A new entry of a standard map is Rust memory the builder took
            // on (#331); persistent map nodes count themselves.
            let charged = (!map.entries.is_persistent()).then(|| MAP_ENTRY_BYTES + k.owned_bytes());
            let fresh = map.entries.upsert(k, (key, item));
            if let (true, Some(bytes)) = (fresh, charged) {
                map.owned.set(map.owned.get().map(|owned| owned + bytes));
                runtime.heap.charge_external(bytes);
            }
            Ok(builder)
        }
        I::WorkerBind { action, entry, fields, .. } => {
            let fields = fields.iter().map(|r| (value(r), kinds[r.0 as usize].repr())).collect();
            Ok(crate::worker::bind(runtime, value(action), value(entry), fields))
        }
        I::MapBuilderFinish { builder, .. } => {
            let map = runtime.heap.payload_mut::<MapValue>(value(builder)).expect("map builder");
            let owned = map.owned.replace(Some(0));
            let finished = MapValue { key: map.key, value: map.value, entries: std::mem::take(&mut map.entries), owned: owned.into() };
            // The entries were charged as the builder took them.
            Ok(runtime.heap.allocate_payload_moved(Box::new(finished)))
        }
        I::MapLen { map, .. } => {
            let length = runtime.heap.payload::<MapValue>(value(map)).expect("map").entries.len();
            Ok(runtime.heap.int(length as i64))
        }
        I::MapHas { map, key, .. } | I::MapGet { map, key, .. } => {
            let (key_kind, key_type) = map_key(runtime, function, *map);
            let key_word = value(key);
            let k = runtime.checked_key(key_word, key_kind, key_type.as_ref())?;
            let found = runtime.heap.payload::<MapValue>(value(map)).expect("map").entries.get(&k).map(|e| e.1);
            if matches!(instruction, I::MapHas { .. }) {
                return Ok(bool_word(found.is_some()));
            }
            found.ok_or_else(|| format!("Map key not found: {}", key_text(runtime, key_word, key_kind, key_type.as_ref())))
        }
        I::MapKeys { dst, map } | I::MapValues { dst, map } => {
            let map = runtime.heap.payload::<MapValue>(value(map)).expect("map");
            let keys = matches!(instruction, I::MapKeys { .. });
            let repr = if keys { map.key } else { map.value };
            let items: Vec<Word> = map.entries.values().map(|(k, v)| if keys { *k } else { *v }).collect();
            Ok(new_array(runtime, function, dst.0, repr, &items))
        }
        _ => Err(format!("{} has no runtime path in sparrow-core yet", instruction.mnemonic())),
    }
}

/// `map.lookup`: the value stored under `key`, if any.
pub fn map_find(runtime: &Runtime, function: usize, pc: usize, map: Word, key: Word) -> Option<Word> {
    let I::MapLookup { map: register, .. } = &runtime.loaded.program.functions[function].code[pc] else {
        return None;
    };
    let (key_kind, key_type) = map_key(runtime, function, *register);
    let k = runtime.key(key, key_kind, key_type.as_ref());
    runtime.heap.payload::<MapValue>(map)?.entries.get(&k).map(|e| e.1)
}

fn builder_register(instruction: &I) -> &okkama_sparrow::Register {
    match instruction {
        I::MapBuilderPut { builder, .. } => builder,
        _ => unreachable!("map builder instruction"),
    }
}

fn element_repr(runtime: &Runtime, array: Word) -> Repr {
    Repr::from_bits((runtime.heap.header_of(array) >> 9) & 3)
}

fn map_key(runtime: &Runtime, function: usize, map: okkama_sparrow::Register) -> (Kind, Option<Type>) {
    match runtime.ty(function, map) {
        Some(Type::Map(key, _) | Type::MapBuilder(key, _)) => {
            (runtime.kind_of(*key), runtime.loaded.program.ty(*key).cloned())
        }
        _ => (Kind::Raw, None),
    }
}

fn map_for(runtime: &Runtime, function: usize, register: okkama_sparrow::Register) -> Result<MapValue, String> {
    match runtime.ty(function, register) {
        Some(Type::Map(key, value) | Type::MapBuilder(key, value)) => Ok(MapValue {
            key: runtime.kind_of(*key).repr(),
            value: runtime.kind_of(*value).repr(),
            entries: Default::default(),
            owned: Default::default(),
        }),
        _ => Err("map instruction needs a map type".into()),
    }
}

fn key_text(runtime: &Runtime, word: Word, kind: Kind, ty: Option<&Type>) -> String {
    match (kind, ty) {
        (_, Some(Type::Enum(e))) => {
            let cases = &runtime.loaded.program.enums[e.0 as usize].cases;
            format!("\"{}\"", cases.get(word as u32 as usize).map_or("?", String::as_str))
        }
        (_, Some(Type::Str)) => format!("\"{}\"", runtime.heap.text(word)),
        (_, Some(Type::Char)) => format!("\"{}\"", char::from_u32(word as u32).unwrap_or('?')),
        (_, Some(Type::Bool)) => if word != 0 { "true" } else { "false" }.to_string(),
        (_, Some(Type::Rat)) => runtime.rational(word).to_string(),
        (Kind::Float, _) => crate::scalar::float_text(f64::from_bits(word)),
        (Kind::Raw | Kind::Byte, _) => word.to_string(),
        _ => runtime.big_value(word).to_string(),
    }
}

/// Decimal text of an integer word (inline integers without a BigInt).
fn int_text(runtime: &mut Runtime, word: Word) -> Word {
    let text = match runtime.heap.integer(word) {
        crate::heap::Integer::Small(n) => n.to_string(),
        crate::heap::Integer::Big(n) => n.to_string(),
    };
    runtime.text_new(&text)
}

/// `cvt` of an int to a string.
#[inline(never)]
pub extern "C" fn rt_int_text(ctx: *mut Ctx, word: Word) -> Word {
    let frame = crate::runtime::frame_pointer!();
    let ctx = unsafe { &mut *ctx };
    let runtime = unsafe { &mut *ctx.runtime };
    let text = int_text(runtime, word);
    if runtime.heap.needs_collection() {
        runtime.safepoint(frame, ctx, &[(text, Repr::Ref)]);
    }
    text
}

fn convert(runtime: &mut Runtime, function: usize, source: okkama_sparrow::Register, dst: okkama_sparrow::Register, word: Word) -> Outcome {
    let from = runtime.ty(function, source).cloned();
    let to = runtime.ty(function, dst).cloned();
    match (from, to) {
        (Some(Type::Int), Some(Type::Str)) => Ok(int_text(runtime, word)),
        (Some(Type::Bool), Some(Type::Str)) => Ok(runtime.text_new(if word != 0 { "true" } else { "false" })),
        (Some(Type::Byte), Some(Type::Str)) => Ok(runtime.text_new(&word.to_string())),
        (Some(Type::Float), Some(Type::Str)) => Ok(runtime.text_new(&crate::scalar::float_text(f64::from_bits(word)))),
        (Some(Type::Rat), Some(Type::Str)) => {
            let text = runtime.rational(word).to_string();
            Ok(runtime.text_new(&text))
        }
        (Some(Type::Int), Some(Type::Rat)) => {
            let n = runtime.int_value(word);
            Ok(runtime.rational_new(Rat::from_int(&n)))
        }
        (Some(Type::Byte), Some(Type::Rat)) => Ok(runtime.rational_new(Rat::from_int(&IntValue::from_i64(word as i64)))),
        (Some(Type::Float), Some(Type::Rat)) => Ok(runtime.rational_new(Rat::from_float(f64::from_bits(word)))),
        (Some(Type::Rat), Some(Type::Int)) => {
            let n = runtime.rational(word).to_int();
            Ok(match n.to_i64() {
                Some(small) => runtime.heap.int(small),
                None => integer_word(runtime, n.as_big().as_bigint().clone()),
            })
        }
        (Some(Type::Rat), Some(Type::Float)) => runtime.rational(word).to_float().map(f64::to_bits),
        (Some(Type::Byte), Some(Type::Int)) => Ok(runtime.heap.int(word as i64)),
        // `char` ↔ text and code point (#231).
        (Some(Type::Char), Some(Type::Int)) => Ok(runtime.heap.int(word as i64)),
        (Some(Type::Char), Some(Type::Str)) => {
            let text = char::from_u32(word as u32).map(String::from).unwrap_or_default();
            Ok(runtime.text_new(&text))
        }
        (Some(Type::Int), Some(Type::Char)) => {
            let code = int_value(runtime, word);
            Ok(u64::from(u32::from(okkama_runtime_core::strings::character(&code)?)))
        }
        (Some(Type::Int), Some(Type::Float)) => {
            let n = runtime.big_value(word).to_f64().unwrap_or(f64::INFINITY);
            finite(n)
        }
        (Some(Type::Byte), Some(Type::Float)) => finite(word as f64),
        (Some(Type::Float), Some(Type::Int)) => {
            let n = f64::from_bits(word);
            let truncated = n.trunc();
            if !(-9223372036854775808.0..9223372036854775808.0).contains(&truncated) {
                return Err(format!(
                    "Cannot convert float value {} to int: expected signed-64 range",
                    crate::scalar::float_text(n)
                ));
            }
            Ok(runtime.heap.int(truncated as i64))
        }
        _ => Err("unsupported scalar conversion".into()),
    }
}

/// An `int` word as the IR engine's integer, for the shared string contracts.
fn int_value(runtime: &Runtime, word: Word) -> okkama_runtime_core::IntValue {
    match runtime.small_or_big(word) {
        Some(n) => okkama_runtime_core::IntValue::from_i64(n),
        None => okkama_runtime_core::IntValue::parse(&runtime.big_value(word).to_string()).expect("decimal digits"),
    }
}

/// `str.fn` (#210, #231): the same contracts as the IR engine's
/// `okkama_runtime_core::strings`.
fn str_fn(runtime: &mut Runtime, op: StrFn, args: Vec<Word>) -> Outcome {
    use okkama_runtime_core::strings;
    let text = runtime.heap.text(args[0]);
    match op {
        StrFn::Find | StrFn::FindLast => {
            let part = runtime.heap.text(args[1]);
            let found = if op == StrFn::Find { strings::find(text, part) } else { strings::find_last(text, part) };
            Ok(runtime.heap.int(found.map_or(-1, |index| index as i64)))
        }
        StrFn::StartsWith => Ok(bool_word(text.starts_with(runtime.heap.text(args[1])))),
        StrFn::EndsWith => Ok(bool_word(text.ends_with(runtime.heap.text(args[1])))),
        StrFn::TrimStart | StrFn::TrimEnd | StrFn::Lower | StrFn::Upper => {
            let result = match op {
                StrFn::TrimStart => strings::trim_start(text).to_owned(),
                StrFn::TrimEnd => strings::trim_end(text).to_owned(),
                StrFn::Lower => text.to_lowercase(),
                _ => text.to_uppercase(),
            };
            Ok(runtime.text_new(&result))
        }
        StrFn::Repeat => {
            let count = int_value(runtime, args[1]);
            let repeated = strings::repeat(runtime.heap.text(args[0]), &count)?;
            Ok(runtime.text_new(&repeated))
        }
        StrFn::Characters => {
            let codes: Vec<Word> = text.chars().map(|c| u64::from(u32::from(c))).collect();
            Ok(runtime.heap.allocate_array(Repr::Raw, &codes))
        }
    }
}

fn byte_from(runtime: &mut Runtime, kind: Kind, word: Word, context: Option<&str>) -> Outcome {
    let (name, text) = match kind {
        Kind::Float => {
            let n = f64::from_bits(word);
            if (0.0..=255.0).contains(&n.trunc()) {
                return Ok(n.trunc() as Word);
            }
            ("float", crate::scalar::float_text(n))
        }
        Kind::Int => {
            if let Some(byte) = runtime.small_or_big(word).and_then(|n| u8::try_from(n).ok()) {
                return Ok(Word::from(byte));
            }
            ("int", runtime.big_value(word).to_string())
        }
        _ => {
            let n = runtime.rational(word);
            if let Some(byte) = n.to_int().to_i64().and_then(|n| u8::try_from(n).ok()) {
                return Ok(Word::from(byte));
            }
            ("rat", n.to_string())
        }
    };
    let message = format!("Cannot convert {name} value {text} to byte: expected 0..255");
    Err(match context {
        Some(c) => format!("{c}: {message}"),
        None => message,
    })
}
