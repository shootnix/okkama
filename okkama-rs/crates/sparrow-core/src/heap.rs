//! Non-moving precise heap (decision R1). Every object starts with a header:
//!
//! ```text
//! bits 0..8   kind          bit 8  mark          bits 9..11 element Repr
//! bits 32..64 payload: object descriptor, string byte length, array length,
//!             closure/frame function
//! ```
//!
//! Small objects live in 64 KiB pages of one size class and are allocated from
//! per-class free lists (machine code can pop them inline); large objects are
//! separate allocations. Allocation never collects: the executor collects at
//! safepoints, with precise roots, when [`Heap::needs_collection`] says so.
//! Rust data owned by an object (BigInt, maps, native state) is dropped when the
//! sweep finds the object dead.
#![allow(unsafe_code)]

use crate::word::{self, Repr, Word};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

pub const FREE: u8 = 0;
pub const OBJECT: u8 = 1;
pub const STRING: u8 = 2;
pub const ARRAY: u8 = 3;
pub const BIGINT: u8 = 4;
pub const PAYLOAD: u8 = 5;
pub const CLOSURE: u8 = 6;
pub const FRAME: u8 = 7;

const MARK: u64 = 1 << 8;
/// An array of bytes stored one per byte (`@byte`), not one per word.
pub const PACKED: u64 = 1 << 11;
/// Temporary mark of graph walks outside collection (`resource_graph`); always
/// cleared before the walk returns.
pub const VISITED: u64 = 1 << 12;
/// A string whose bytes are all ASCII: lengths and slices are byte offsets.
pub const ASCII: u64 = 1 << 13;
/// A container (array, closure, map, box) with an entry in the resource walk
/// summaries (#9): the walk looks the entry up only when this bit is set.
pub const SUMMARY: u64 = 1 << 14;
const PAGE_WORDS: usize = 64 * 1024 / 8;
/// Bytes of one page of small cells.
pub const PAGE_BYTES: usize = PAGE_WORDS * 8;
/// Cell sizes in words, header included. Async frames are seven words plus
/// one per register: classes up to 128 words keep frames of large functions
/// on free lists instead of separate allocations.
pub const CLASSES: [usize; 18] = [2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96, 128];
/// The words of a reclaimed cell under collector stress.
const POISON: u64 = 0xdead_dead_dead_dead;
/// Collections start once this much has been allocated, at least.
const COLLECTION_FLOOR: usize = 1 << 20;

#[inline]
pub fn header(kind: u8, repr: Repr, payload: u32) -> u64 {
    u64::from(kind) | (u64::from(repr as u8) << 9) | (u64::from(payload) << 32)
}
#[inline]
pub fn kind(header: u64) -> u8 {
    header as u8
}
#[inline]
pub fn payload(header: u64) -> u32 {
    (header >> 32) as u32
}
#[inline]
fn element_repr(header: u64) -> Repr {
    Repr::from_bits((header >> 9) & 3)
}

/// Rust data owned by a heap object, traced like fields and dropped with it.
pub trait Payload: std::any::Any + std::fmt::Debug {
    /// Report every word this payload keeps alive.
    fn trace(&self, _visit: &mut dyn FnMut(Word, Repr)) {}
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Field representations of objects, closures and async frames, by index.
#[derive(Debug, Default, Clone)]
pub struct Layouts {
    pub objects: Vec<Vec<Repr>>,
    pub closures: Vec<Vec<Repr>>,
    pub frames: Vec<Vec<Repr>>,
}

#[derive(Debug)]
struct Page {
    class: usize,
    memory: Box<[u64]>,
}

/// An integer read from a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integer<'a> {
    Small(i64),
    Big(&'a BigInt),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HeapStats {
    pub collections: usize,
    /// Bytes allocated since the start, and the live bytes after each
    /// collection added up (their mean is the typical live heap).
    pub allocated_bytes: usize,
    pub live_sum: usize,
    pub live_bytes: usize,
    pub pages: usize,
    pub large: usize,
}

#[derive(Debug)]
pub struct Heap {
    pub layouts: Layouts,
    pages: Vec<Page>,
    /// Free list heads per class; a free cell's second word links the next.
    free: [Word; CLASSES.len()],
    large: Vec<Box<[u64]>>,
    allocated: usize,
    threshold: usize,
    live: usize,
    collections: usize,
    allocated_before: usize,
    live_sum: usize,
    worklist: Vec<Word>,
    /// Objects whose death must be reported (resource identities).
    watched: crate::word::FastSet<Word>,
    /// Watched objects reclaimed by the last sweeps, until taken.
    released: Vec<Word>,
    /// Growth checks against a memory budget (`budget`).
    pub watch: Option<crate::budget::Watch>,
    /// Collector stress (`SPARROW_CORE_GC_STRESS`): collect at every safepoint
    /// and poison reclaimed cells, so a value kept alive by mistake fails at
    /// once instead of after a reuse.
    stress: bool,
    /// Code point positions of recently indexed non-ASCII strings.
    positions: Positions,
    /// Allocations made by the runtime (not inline by machine code), by
    /// kind, and the time spent collecting — when statistics are on (#55).
    pub allocations: Option<Box<Allocations>>,
}

/// Code point positions of a few recently indexed non-ASCII strings (#52):
/// the number of code points, and the byte offset of the last code point
/// index reached. Indexing near it walks from there, so `size()` in a loop
/// and slicing one code point after another are linear, not quadratic.
/// Strings are immutable and freed only by a collection, which clears it.
#[derive(Debug, Default)]
struct Positions {
    entries: [Position; 4],
    next: usize,
}

#[derive(Debug, Default, Clone, Copy)]
struct Position {
    string: Word,
    count: Option<usize>,
    char: usize,
    byte: usize,
}

impl Positions {
    fn entry(&mut self, string: Word) -> &mut Position {
        let at = match self.entries.iter().position(|entry| entry.string == string) {
            Some(at) => at,
            None => {
                let at = self.next;
                self.next = (self.next + 1) % self.entries.len();
                self.entries[at] = Position { string, ..Position::default() };
                at
            }
        };
        &mut self.entries[at]
    }
}

/// Runtime allocations by kind: count and bytes; collection time.
#[derive(Debug, Default)]
pub struct Allocations {
    /// (kind, object descriptor or 0) → (count, bytes).
    pub kinds: std::collections::BTreeMap<(&'static str, u32), (u64, u64)>,
    pub collection_nanos: u64,
}

impl Heap {
    #[inline]
    pub(crate) fn counted(&mut self, kind: &'static str, descriptor: u32, words: usize) {
        if let Some(allocations) = &mut self.allocations {
            let entry = allocations.kinds.entry((kind, descriptor)).or_default();
            entry.0 += 1;
            entry.1 += words as u64 * 8;
        }
    }

    pub fn new(layouts: Layouts) -> Self {
        Self {
            layouts,
            pages: Vec::new(),
            free: [0; CLASSES.len()],
            large: Vec::new(),
            allocated: 0,
            threshold: COLLECTION_FLOOR,
            live: 0,
            collections: 0,
            allocated_before: 0,
            live_sum: 0,
            worklist: Vec::new(),
            watched: crate::word::FastSet::default(),
            released: Vec::new(),
            watch: None,
            positions: Positions::default(),
            allocations: None,
            stress: false,
        }
    }

    /// Allocation-triggered collection is proportional to the live heap.
    pub fn needs_collection(&self) -> bool {
        self.allocated >= self.threshold
    }

    /// Collector stress: a zero threshold makes every safepoint collect.
    pub fn set_stress(&mut self, on: bool) {
        self.stress = on;
        if on {
            self.threshold = 0;
        }
    }

    /// Report the death of `object` through [`Heap::take_released`].
    pub fn watch(&mut self, object: Word) {
        self.watched.insert(object);
    }
    pub fn take_released(&mut self) -> Vec<Word> {
        std::mem::take(&mut self.released)
    }

    pub fn stats(&self) -> HeapStats {
        HeapStats {
            collections: self.collections,
            allocated_bytes: self.allocated_before + self.allocated,
            live_sum: self.live_sum,
            live_bytes: self.live,
            pages: self.pages.len(),
            large: self.large.len(),
        }
    }

    /// Uninitialized storage of `words` words (header included); the caller
    /// writes the header and every word before the next collection.
    pub fn allocate(&mut self, words: usize) -> Word {
        let Some(class) = Self::class_of(words) else {
            self.allocated += words * 8;
            if let Some(watch) = &mut self.watch {
                watch.grow(words * 8);
            }
            let mut memory = vec![0u64; words].into_boxed_slice();
            let address = memory.as_mut_ptr() as Word;
            self.large.push(memory);
            return address;
        };
        self.allocate_class(class)
    }

    /// Size class of an object of `words` words, if it is not large.
    pub fn class_of(words: usize) -> Option<usize> {
        CLASSES.iter().position(|&size| size >= words)
    }

    /// Address of the free-list head of a class, popped inline by machine code.
    pub(crate) fn free_head(&mut self, class: usize) -> *mut Word {
        &mut self.free[class]
    }

    /// Pop a cell of `class` from its free list, adding a page when empty.
    pub fn allocate_class(&mut self, class: usize) -> Word {
        if self.free[class] == 0 {
            self.add_page(class);
        }
        let cell = self.free[class];
        // SAFETY: free cells are at least two words inside a live page.
        self.free[class] = unsafe { *((cell + 8) as *const u64) };
        cell
    }

    /// Growth is counted in pages and large objects, not cells: machine code
    /// pops free cells inline, and only an empty list reaches the heap.
    fn add_page(&mut self, class: usize) {
        self.allocated += PAGE_WORDS * 8;
        if let Some(watch) = &mut self.watch {
            watch.grow(PAGE_BYTES);
        }
        let size = CLASSES[class];
        let mut memory = vec![0u64; PAGE_WORDS - PAGE_WORDS % size].into_boxed_slice();
        let base = memory.as_mut_ptr() as Word;
        let cells = memory.len() / size;
        for cell in (0..cells).rev() {
            let address = base + (cell * size * 8) as Word;
            // SAFETY: inside the page just allocated.
            unsafe {
                *(address as *mut u64) = header(FREE, Repr::Raw, 0);
                *((address + 8) as *mut u64) = self.free[class];
            }
            self.free[class] = address;
        }
        self.pages.push(Page { class, memory });
    }

    // ---- objects ----

    pub fn object_new(&mut self, descriptor: u32, fields: &[Word]) -> Word {
        self.counted("object", descriptor, fields.len().max(1) + 1);
        let object = self.allocate(fields.len().max(1) + 1);
        // SAFETY: freshly allocated storage of fields.len() + 1 words.
        unsafe {
            *(object as *mut u64) = header(OBJECT, Repr::Raw, descriptor);
            std::ptr::copy_nonoverlapping(fields.as_ptr(), (object + 8) as *mut u64, fields.len());
        }
        object
    }
    /// SAFETY of the accessors below: `object` is a live object of the right
    /// kind and the index is within its layout, as the verifier guarantees.
    pub fn field(&self, object: Word, index: usize) -> Word {
        unsafe { *((object + 8 + index as Word * 8) as *const u64) }
    }
    pub fn set_field(&mut self, object: Word, index: usize, value: Word) {
        unsafe { *((object + 8 + index as Word * 8) as *mut u64) = value }
    }
    /// Set the walk mark of `object`; false when it was already set.
    pub fn visit(&mut self, object: Word) -> bool {
        // SAFETY: `object` is a live heap object.
        unsafe {
            let header = &mut *(object as *mut u64);
            let first = *header & VISITED == 0;
            *header |= VISITED;
            first
        }
    }
    pub fn unvisit(&mut self, object: Word) {
        // SAFETY: as in `visit`.
        unsafe { *(object as *mut u64) &= !VISITED }
    }
    /// Set or clear the [`SUMMARY`] bit of `object`.
    pub fn summarized(&mut self, object: Word, on: bool) {
        // SAFETY: as in `visit`.
        unsafe {
            let header = &mut *(object as *mut u64);
            if on {
                *header |= SUMMARY;
            } else {
                *header &= !SUMMARY;
            }
        }
    }
    pub fn header_of(&self, object: Word) -> u64 {
        unsafe { *(object as *const u64) }
    }

    // ---- strings ----

    pub fn allocate_text(&mut self, text: &str) -> Word {
        let bytes = text.len();
        let words = 1 + bytes.div_ceil(8).max(1);
        self.counted("string", 0, words);
        let string = self.allocate(words);
        // SAFETY: room for the header and `bytes` bytes.
        unsafe {
            *(string as *mut u64) =
                header(STRING, Repr::Raw, bytes as u32) | if text.is_ascii() { ASCII } else { 0 };
            std::ptr::copy_nonoverlapping(text.as_ptr(), (string + 8) as *mut u8, bytes);
        }
        string
    }
    pub fn is_ascii_text(&self, string: Word) -> bool {
        self.header_of(string) & ASCII != 0
    }

    /// A new string of bytes `start..end` of `string` (char boundaries).
    pub fn substring(&mut self, string: Word, start: usize, end: usize) -> Word {
        let part = &self.text(string)[start..end];
        let (pointer, length) = (part.as_ptr(), part.len());
        // SAFETY: the source string stays where it is while the copy is
        // allocated (allocation neither moves nor collects).
        let part = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(pointer, length)) };
        self.allocate_text(part)
    }

    /// The number of code points of `string`.
    pub fn char_count(&mut self, string: Word) -> usize {
        if self.is_ascii_text(string) {
            return self.text(string).len();
        }
        if let Some(count) = self.positions.entry(string).count {
            return count;
        }
        let count = self.text(string).chars().count();
        self.positions.entry(string).count = Some(count);
        count
    }

    /// The byte offset of code point `index` (at most the count) of
    /// `string`, walking from the nearest of the start, the end and the last
    /// position reached.
    pub fn char_offset(&mut self, string: Word, index: usize) -> usize {
        if self.is_ascii_text(string) {
            return index;
        }
        let count = self.char_count(string);
        let text: &str = {
            let length = payload(self.header_of(string)) as usize;
            // SAFETY: as in `text`; the string outlives this call.
            unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts((string + 8) as *const u8, length)) }
        };
        let entry = self.positions.entry(string);
        let (mut char, mut byte) = [(0, 0), (count, text.len()), (entry.char, entry.byte)]
            .into_iter()
            .min_by_key(|(char, _)| char.abs_diff(index))
            .expect("three starts");
        let bytes = text.as_bytes();
        while char < index {
            byte += utf8_width(bytes[byte]);
            char += 1;
        }
        while char > index {
            byte -= 1;
            while bytes[byte] & 0xC0 == 0x80 {
                byte -= 1;
            }
            char -= 1;
        }
        (entry.char, entry.byte) = (char, byte);
        byte
    }

    pub fn text(&self, string: Word) -> &str {
        let length = payload(self.header_of(string)) as usize;
        // SAFETY: strings are written once from valid UTF-8 and never mutated.
        unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                (string + 8) as *const u8,
                length,
            ))
        }
    }

    // ---- arrays ----

    pub fn allocate_array(&mut self, repr: Repr, elements: &[Word]) -> Word {
        self.counted("array", 0, elements.len() + 1);
        let array = self.allocate(elements.len() + 1);
        // SAFETY: room for the header and the elements.
        unsafe {
            *(array as *mut u64) = header(ARRAY, repr, elements.len() as u32);
            std::ptr::copy_nonoverlapping(
                elements.as_ptr(),
                (array + 8) as *mut u64,
                elements.len(),
            );
        }
        array
    }
    /// The elements as words; a byte array is widened (prefer [`Heap::bytes`]).
    pub fn elements(&self, array: Word) -> std::borrow::Cow<'_, [Word]> {
        let header = self.header_of(array);
        let length = payload(header) as usize;
        if header & PACKED != 0 {
            return std::borrow::Cow::Owned(self.bytes(array).iter().map(|&b| Word::from(b)).collect());
        }
        // SAFETY: arrays are immutable after construction.
        std::borrow::Cow::Borrowed(unsafe { std::slice::from_raw_parts((array + 8) as *const u64, length) })
    }

    /// A byte array (`@byte`): one byte per element after the header.
    pub fn allocate_bytes(&mut self, bytes: &[u8]) -> Word {
        self.counted("bytes", 0, 1 + bytes.len().div_ceil(8));
        let array = self.allocate(1 + bytes.len().div_ceil(8));
        // SAFETY: room for the header and the bytes.
        unsafe {
            *(array as *mut u64) = header(ARRAY, Repr::Raw, bytes.len() as u32) | PACKED;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), (array + 8) as *mut u8, bytes.len());
        }
        array
    }

    pub fn is_bytes(&self, array: Word) -> bool {
        self.header_of(array) & PACKED != 0
    }

    /// The bytes of any `@byte` value: borrowed when packed, else narrowed
    /// from its words (an empty array literal, for instance).
    pub fn byte_view(&self, array: Word) -> std::borrow::Cow<'_, [u8]> {
        if self.is_bytes(array) {
            return std::borrow::Cow::Borrowed(self.bytes(array));
        }
        std::borrow::Cow::Owned(self.elements(array).iter().map(|&w| w as u8).collect())
    }

    /// A string of bytes `start..end` of any `@byte` value, if they are UTF-8: one copy.
    pub fn text_from_byte_range(&mut self, array: Word, start: usize, end: usize) -> Option<Word> {
        if !self.is_bytes(array) {
            let bytes = self.byte_view(array)[start..end].to_vec();
            return std::str::from_utf8(&bytes).ok().map(|text| self.allocate_text(text));
        }
        let bytes = &self.bytes(array)[start..end];
        std::str::from_utf8(bytes).ok()?;
        let (pointer, length) = (bytes.as_ptr(), bytes.len());
        // SAFETY: as in `text_from_bytes`.
        let text = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(pointer, length)) };
        Some(self.allocate_text(text))
    }

    /// A string with the bytes of byte array `array`, if they are UTF-8: one copy.
    pub fn text_from_bytes(&mut self, array: Word) -> Option<Word> {
        let bytes = self.bytes(array);
        std::str::from_utf8(bytes).ok()?;
        let (pointer, length) = (bytes.as_ptr(), bytes.len());
        // SAFETY: the array stays where it is while the string is allocated
        // (allocation neither moves nor collects), and it is valid UTF-8.
        let text = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(pointer, length)) };
        Some(self.allocate_text(text))
    }

    /// The bytes of a byte array.
    pub fn bytes(&self, array: Word) -> &[u8] {
        let header = self.header_of(array);
        debug_assert!(header & PACKED != 0, "not a byte array");
        // SAFETY: a packed array holds `length` bytes after its header.
        unsafe { std::slice::from_raw_parts((array + 8) as *const u8, payload(header) as usize) }
    }

    // ---- integers ----

    pub fn int(&mut self, n: i64) -> Word {
        word::small(n).unwrap_or_else(|| self.big(BigInt::from(n)))
    }
    /// Canonical integer word: inline when it fits in 63 bits.
    pub fn big(&mut self, n: BigInt) -> Word {
        if let Some(word) = n.to_i64().and_then(word::small) {
            return word;
        }
        // The digits live outside the heap pages; count them so that garbage
        // big integers trigger collections by their real size.
        self.allocated += (n.bits() / 8) as usize + 32;
        self.counted("bigint", 0, 2);
        let object = self.allocate(2);
        // SAFETY: two fresh words; the box is dropped by the sweep.
        unsafe {
            *(object as *mut u64) = header(BIGINT, Repr::Raw, 0);
            *((object + 8) as *mut u64) = Box::into_raw(Box::new(n)) as u64;
        }
        object
    }
    pub fn integer(&self, value: Word) -> Integer<'_> {
        match word::small_value(value) {
            Some(n) => Integer::Small(n),
            // SAFETY: a non-inline int word points to a live BigInt object.
            None => Integer::Big(unsafe { &*(self.field(value, 0) as *const BigInt) }),
        }
    }

    // ---- Rust payloads ----

    pub fn allocate_payload(&mut self, payload: Box<dyn Payload>) -> Word {
        self.counted("payload", 0, 2);
        let object = self.allocate(2);
        // SAFETY: two fresh words; the double box gives a thin pointer.
        unsafe {
            *(object as *mut u64) = header(PAYLOAD, Repr::Raw, 0);
            *((object + 8) as *mut u64) = Box::into_raw(Box::new(payload)) as u64;
        }
        object
    }
    pub fn payload<T: Payload>(&self, object: Word) -> Option<&T> {
        // SAFETY: a PAYLOAD object holds a live Box<Box<dyn Payload>>.
        (kind(self.header_of(object)) == PAYLOAD)
            .then(|| unsafe { &**(self.field(object, 0) as *const Box<dyn Payload>) })
            .and_then(|p| p.as_any().downcast_ref())
    }
    pub fn payload_mut<T: Payload>(&mut self, object: Word) -> Option<&mut T> {
        // SAFETY: as above, uniquely borrowed through &mut self.
        (kind(self.header_of(object)) == PAYLOAD)
            .then(|| unsafe { &mut **(self.field(object, 0) as *mut Box<dyn Payload>) })
            .and_then(|p| p.as_any_mut().downcast_mut())
    }

    // ---- closures and async frames ----

    /// A closure has one word per parameter of its function (its layout);
    /// captures fill a prefix and the remaining words are unbound.
    pub fn closure_new(&mut self, function: u32, captures: &[Word]) -> Word {
        let words = self.layouts.closures[function as usize].len();
        debug_assert!(captures.len() <= words);
        self.counted("closure", 0, words.max(1) + 1);
        let closure = self.allocate(words.max(1) + 1);
        // SAFETY: room for the header and `words` words.
        unsafe {
            *(closure as *mut u64) = header(CLOSURE, Repr::Raw, function);
            std::ptr::write_bytes((closure + 8) as *mut u64, 0, words.max(1));
            std::ptr::copy_nonoverlapping(
                captures.as_ptr(),
                (closure + 8) as *mut u64,
                captures.len(),
            );
        }
        closure
    }

    // ---- collection ----

    /// Mark everything reachable from `roots`, then sweep. Roots report each
    /// word with its representation; the heap traces the rest by layout.
    pub fn collect(&mut self, roots: impl FnOnce(&mut dyn FnMut(Word, Repr))) {
        // A freed string's address may be reused for another one.
        self.positions = Positions::default();
        let started = self.allocations.is_some().then(std::time::Instant::now);
        self.collect_inner(roots);
        if let (Some(allocations), Some(started)) = (&mut self.allocations, started) {
            allocations.collection_nanos += started.elapsed().as_nanos() as u64;
        }
    }

    fn collect_inner(&mut self, roots: impl FnOnce(&mut dyn FnMut(Word, Repr))) {
        let mut worklist = std::mem::take(&mut self.worklist);
        roots(&mut |word, repr| {
            if let Some(pointer) = repr.pointer(word) {
                worklist.push(pointer);
            }
        });
        while let Some(object) = worklist.pop() {
            // SAFETY: every pushed word is a live object pointer.
            let header = unsafe { *(object as *const u64) };
            if header & MARK != 0 {
                continue;
            }
            unsafe { *(object as *mut u64) = header | MARK };
            let mut push = |word: Word, repr: Repr| {
                if let Some(pointer) = repr.pointer(word) {
                    worklist.push(pointer);
                }
            };
            let fields = |layout: &[Repr], push: &mut dyn FnMut(Word, Repr)| {
                for (index, &repr) in layout.iter().enumerate() {
                    push(
                        unsafe { *((object + 8 + index as Word * 8) as *const u64) },
                        repr,
                    );
                }
            };
            match kind(header) {
                OBJECT => fields(&self.layouts.objects[payload(header) as usize], &mut push),
                CLOSURE => fields(&self.layouts.closures[payload(header) as usize], &mut push),
                FRAME => fields(&self.layouts.frames[payload(header) as usize], &mut push),
                ARRAY => {
                    let repr = element_repr(header);
                    if repr != Repr::Raw {
                        for index in 0..payload(header) as usize {
                            push(
                                unsafe { *((object + 8 + index as Word * 8) as *const u64) },
                                repr,
                            );
                        }
                    }
                }
                PAYLOAD => {
                    let payload =
                        unsafe { &**(*((object + 8) as *const u64) as *const Box<dyn Payload>) };
                    payload.trace(&mut push);
                }
                _ => {}
            }
        }
        self.worklist = worklist;
        self.sweep();
    }

    /// Overwrite every free cell past its link (collector stress).
    fn poison(&mut self) {
        for (class, &size) in CLASSES.iter().enumerate() {
            let mut cell = self.free[class];
            while cell != 0 {
                for word in 2..size {
                    // SAFETY: a free cell of `size` words inside a live page.
                    unsafe { *((cell + word as Word * 8) as *mut u64) = POISON };
                }
                cell = unsafe { *((cell + 8) as *const u64) };
            }
        }
    }

    fn sweep(&mut self) {
        self.free = [0; CLASSES.len()];
        let mut live = 0;
        for page in &mut self.pages {
            let size = CLASSES[page.class];
            let base = page.memory.as_mut_ptr() as Word;
            for cell in (0..page.memory.len() / size).rev() {
                let address = base + (cell * size * 8) as Word;
                // SAFETY: every cell of a page starts with a header.
                let header = unsafe { *(address as *const u64) };
                if header & MARK != 0 {
                    unsafe { *(address as *mut u64) = header & !MARK };
                    live += size * 8;
                    continue;
                }
                if kind(header) != FREE {
                    unsafe { release(address, header) };
                    unsafe { *(address as *mut u64) = 0 };
                    if !self.watched.is_empty() && self.watched.remove(&address) {
                        self.released.push(address);
                    }
                }
                unsafe { *((address + 8) as *mut u64) = self.free[page.class] };
                self.free[page.class] = address;
            }
        }
        self.large.retain_mut(|memory| {
            let address = memory.as_mut_ptr() as Word;
            let header = memory[0];
            if header & MARK != 0 {
                memory[0] = header & !MARK;
                live += memory.len() * 8;
                true
            } else {
                unsafe { release(address, header) };
                if self.watched.remove(&address) {
                    self.released.push(address);
                }
                false
            }
        });
        if self.stress {
            self.poison();
        }
        self.live = live;
        self.live_sum += live;
        self.allocated_before += self.allocated;
        self.allocated = 0;
        self.threshold = if self.stress { 0 } else { live.max(COLLECTION_FLOOR) };
        self.collections += 1;
    }
}

/// Drop Rust data owned by a dead object.
///
/// SAFETY: `address` is a dead object with `header`, released exactly once.
unsafe fn release(address: Word, header: u64) {
    match kind(header) {
        BIGINT => drop(unsafe { Box::from_raw(*((address + 8) as *const u64) as *mut BigInt) }),
        PAYLOAD => {
            drop(unsafe { Box::from_raw(*((address + 8) as *const u64) as *mut Box<dyn Payload>) })
        }
        _ => {}
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        // Release Rust data of every remaining object.
        self.collect(|_| {});
    }
}

impl std::fmt::Display for Integer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Small(n) => write!(f, "{n}"),
            Self::Big(n) => write!(f, "{n}"),
        }
    }
}

/// Bytes of the UTF-8 sequence that starts with `lead`.
fn utf8_width(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::UNBOUND;
    use std::rc::Rc;

    fn layouts() -> Layouts {
        Layouts {
            // 0: Node { next: ref, value: int }   1: Pair { left, right: ref }
            objects: vec![vec![Repr::Ref, Repr::Int], vec![Repr::Ref, Repr::Ref]],
            closures: vec![vec![Repr::Ref, Repr::Int]],
            frames: vec![],
        }
    }

    #[derive(Debug)]
    struct Counted {
        _alive: Rc<()>,
        words: Vec<Word>,
    }
    impl Payload for Counted {
        fn trace(&self, visit: &mut dyn FnMut(Word, Repr)) {
            for &word in &self.words {
                visit(word, Repr::Ref);
            }
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn reachable_graphs_survive_and_cycles_are_reclaimed() {
        let mut heap = Heap::new(layouts());
        let a = heap.object_new(1, &[UNBOUND, UNBOUND]);
        let b = heap.object_new(1, &[a, UNBOUND]);
        heap.set_field(a, 0, b); // cycle a <-> b, kept by `kept`
        let seven = heap.int(7);
        let kept = heap.object_new(0, &[a, seven]);
        let garbage = heap.object_new(1, &[UNBOUND, UNBOUND]);
        heap.set_field(garbage, 0, garbage);
        heap.collect(|visit| visit(kept, Repr::Ref));
        assert_eq!(heap.field(heap.field(kept, 0), 0), b);
        assert_eq!(heap.integer(heap.field(kept, 1)), Integer::Small(7));
        // The unreachable self-cycle went to the free list: header cleared.
        assert_eq!(kind(heap.header_of(garbage)), FREE);
        heap.collect(|_| {});
        assert_eq!(heap.stats().live_bytes, 0);
    }

    #[test]
    fn marking_a_million_node_chain_does_not_recurse() {
        let mut heap = Heap::new(layouts());
        let mut head = UNBOUND;
        for n in 0..1_000_000 {
            let value = heap.int(n);
            head = heap.object_new(0, &[head, value]);
        }
        heap.collect(|visit| visit(head, Repr::Ref));
        assert_eq!(heap.stats().live_bytes, 1_000_000 * 3 * 8);
        let mut count = 0;
        let mut node = head;
        while node != UNBOUND {
            count += 1;
            node = heap.field(node, 0);
        }
        assert_eq!(count, 1_000_000);
    }

    #[test]
    fn repeated_garbage_reuses_free_cells_and_memory_stays_bounded() {
        let mut heap = Heap::new(layouts());
        let kept = heap.allocate_text("kept across every collection 🙂");
        for round in 0..50 {
            for n in 0..20_000 {
                let text = heap.allocate_text(&format!("temporary {round} {n}"));
                let _array = heap.allocate_array(Repr::Ref, &[text, kept]);
            }
            if heap.needs_collection() {
                heap.collect(|visit| visit(kept, Repr::Ref));
            }
        }
        heap.collect(|visit| visit(kept, Repr::Ref));
        assert_eq!(heap.text(kept), "kept across every collection 🙂");
        assert!(heap.stats().collections >= 10);
        // Bounded by the collection threshold, not by the 2M allocations.
        assert!(heap.stats().pages < 200, "{:?}", heap.stats());
    }

    #[test]
    fn integers_promote_to_bigints_and_bigints_are_dropped_with_their_object() {
        let mut heap = Heap::new(layouts());
        let small = heap.int(word::SMALL_MAX);
        let big = heap.int(i64::MAX);
        assert_eq!(heap.integer(small), Integer::Small(word::SMALL_MAX));
        assert_eq!(heap.integer(big), Integer::Big(&BigInt::from(i64::MAX)));
        let canonical = heap.big(BigInt::from(5));
        assert_eq!(word::small_value(canonical), Some(5));
        let huge: BigInt = "123456789012345678901234567890".parse().unwrap();
        let huge = heap.big(huge);
        let node = heap.object_new(0, &[UNBOUND, huge]);
        heap.collect(|visit| visit(node, Repr::Ref));
        assert_eq!(
            heap.integer(heap.field(node, 1)).to_string(),
            "123456789012345678901234567890"
        );
        heap.collect(|_| {});
        assert_eq!(heap.stats().live_bytes, 0);
    }

    #[test]
    fn payloads_trace_their_words_and_drop_with_their_object() {
        let mut heap = Heap::new(layouts());
        let alive = Rc::new(());
        let text = heap.allocate_text("held by Rust data");
        let payload = heap.allocate_payload(Box::new(Counted {
            _alive: alive.clone(),
            words: vec![text],
        }));
        heap.collect(|visit| visit(payload, Repr::Ref));
        assert_eq!(heap.text(text), "held by Rust data");
        assert_eq!(heap.payload::<Counted>(payload).unwrap().words, vec![text]);
        assert_eq!(Rc::strong_count(&alive), 2);
        heap.collect(|_| {});
        assert_eq!(Rc::strong_count(&alive), 1);
    }

    #[test]
    fn large_arrays_and_closures_trace_their_elements() {
        let mut heap = Heap::new(layouts());
        let texts: Vec<Word> = (0..1000)
            .map(|n| heap.allocate_text(&n.to_string()))
            .collect();
        let array = heap.allocate_array(Repr::Ref, &texts);
        let numbers: Vec<Word> = (0..1000).map(|n| heap.int(n)).collect();
        let raw = heap.allocate_array(Repr::Int, &numbers);
        let three = heap.int(3);
        let closure = heap.closure_new(0, &[array, three]);
        heap.collect(|visit| {
            visit(closure, Repr::Ref);
            visit(raw, Repr::Ref);
        });
        let array = heap.field(closure, 0);
        assert_eq!(heap.text(heap.elements(array)[999]), "999");
        assert_eq!(heap.integer(heap.elements(raw)[500]), Integer::Small(500));
        assert_eq!(heap.stats().large, 2);
        heap.collect(|_| {});
        assert_eq!(heap.stats().large, 0);
    }

    /// Code point positions agree with a plain count from the start, in any
    /// order of access, for several strings at once and across collections
    /// (#52).
    #[test]
    fn code_point_positions_match_a_plain_count() {
        let mut heap = Heap::new(layouts());
        let texts = ["ascii only text", "жёлтый 😀 e\u{301} 中文 ok", "😀😀😀", ""];
        let words: Vec<Word> = texts.iter().map(|text| heap.allocate_text(text)).collect();
        let mut state: u64 = 7;
        for round in 0..2000 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let which = (state >> 33) as usize % texts.len();
            let (text, word) = (texts[which], words[which]);
            let count = text.chars().count();
            assert_eq!(heap.char_count(word), count);
            let index = (state >> 13) as usize % (count + 1);
            let plain = text.char_indices().map(|(at, _)| at).chain([text.len()]).nth(index).unwrap();
            assert_eq!(heap.char_offset(word, index), plain, "{text:?} {index}");
            if round % 500 == 0 {
                heap.collect(|visit| words.iter().for_each(|&word| visit(word, Repr::Ref)));
            }
        }
    }
}
