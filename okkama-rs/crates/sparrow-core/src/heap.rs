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
/// An array version whose elements live in a shared buffer (#246): word 1
/// is the address of the version's first element (so machine code reads an
/// element with one more load than from a flat array), word 2 the buffer,
/// which keeps it alive; the payload is the version's own length. Machine code reads elements
/// through it (see `jit::element`), Rust code through [`Heap::elements`].
pub const SHARED: u64 = 1 << 15;
/// A flat array made by appending to an array of at least [`CHAIN_MIN`]
/// elements: appending to it again is a chain, which gets a buffer.
pub const APPENDED: u64 = 1 << 16;
/// A buffer of shared arrays: a flat array whose payload counts the slots
/// written so far (the frontier), with room for `1 << capacity_bits`
/// elements. Slots below the frontier are never written again.
pub const BUFFER: u64 = 1 << 17;
const CAPACITY_SHIFT: u32 = 18;
/// Arrays shorter than this are copied on append, as before: a copy of a
/// few words costs less than a buffer and a view.
pub const CHAIN_MIN: usize = 32;
/// Strings shorter than this, in bytes, are copied by `+`, as before (#255);
/// a longer one made by `+` starts a chain ([`APPENDED`], [`PREPENDED`]),
/// whose next `+` gets a text buffer. A string version in a text buffer is
/// [`SHARED`]: word 1 the address of its first byte, word 2 the buffer.
pub const TEXT_CHAIN_MIN: usize = 256;
/// An array version kept as a 32-way trie (#250): word 1 is the root branch,
/// word 2 the tail (the last 1–32 elements), word 3 the root's shift; the
/// payload is the length. Its nodes are ordinary arrays — branches of node
/// references, leaves of 32 elements — so the collector traces them as such.
/// Machine code reads it through a helper ([`Heap::trie_get`]); Rust code
/// through [`Heap::elements`], which copies it out.
pub const TRIE: u64 = 1 << 24;
/// A flat array made by `replaced` from one of [`TRIE_MIN`] elements or
/// more: replacing in it again is a chain, which becomes a trie.
pub const REPLACED: u64 = 1 << 25;
/// A flat array made by prepending to an array of at least [`CHAIN_MIN`]
/// elements (`[x] + items`): prepending to it again is a chain (#247).
pub const PREPENDED: u64 = 1 << 26;
/// Arrays shorter than this are copied by `replaced`, as before.
pub const TRIE_MIN: usize = 64;
const TRIE_BITS: u32 = 5;
const TRIE_WIDTH: usize = 1 << TRIE_BITS;
const TRIE_MASK: usize = TRIE_WIDTH - 1;
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
/// The ephemeron pass of a collection (#503): given which objects are marked,
/// it visits what marked keys keep alive.
pub type Ephemerons<'a> = dyn FnMut(&dyn Fn(Word) -> bool, &mut dyn FnMut(Word, Repr)) + 'a;
const COLLECTION_FLOOR: usize = 1 << 20;
/// Spare builder payloads kept at most, each with a buffer of at most
/// [`SPARE_CAPACITY`] words.
const SPARE_PAYLOADS: usize = 64;
pub const SPARE_CAPACITY: usize = 512;
/// Empty pages kept beyond what the next cycle needs (#435).
const EMPTY_PAGES_SLACK: usize = 16;

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
    /// Rust memory this payload owns beyond its own struct (#331): tree
    /// nodes, vector capacity, owned key data. An estimate for the collection
    /// trigger, not a measurement; heap words it refers to are not counted.
    fn external_bytes(&self) -> usize {
        0
    }
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    /// The payload's type, for the allocation counts of `--stats`.
    fn kind_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
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
    /// Rust memory owned by heap objects (payloads, big integer digits;
    /// estimated, #331): charged since the start, and owned by the objects
    /// alive after the last collection.
    pub external_allocated_bytes: usize,
    pub external_live_bytes: usize,
    /// Collections triggered mostly by that memory rather than by heap cells.
    pub external_collections: usize,
}

#[derive(Debug)]
pub struct Heap {
    pub layouts: Layouts,
    pages: Vec<Page>,
    /// Free list heads per class; a free cell's second word links the next.
    free: [Word; CLASSES.len()],
    /// The free cells of the last sweep, per class: one chain per page and
    /// its bytes (#522). A sweep leaves the free lists empty; when one runs
    /// dry, the next chain becomes the list and its bytes count as
    /// allocated, as a new page does. Machine code pops cells inline,
    /// uncounted, so without this a program that recycles its garbage
    /// reused every swept cell before a collection could come, then grew
    /// by new pages every cycle.
    reserve: [Vec<(Word, usize)>; CLASSES.len()],
    large: Vec<Box<[u64]>>,
    /// Bytes of the large objects, live or not yet swept.
    large_bytes: usize,
    /// Allocation pressure since the last collection, in two parts (#331):
    /// heap cells (pages and large objects) and Rust memory owned by heap
    /// objects (payloads as they are made or grow, big integer digits). A
    /// collection is due when their sum reaches `threshold`.
    allocated: usize,
    external: usize,
    /// The live heap after the last collection: its cells (`live`) plus the
    /// Rust memory its objects own (`external_live`, recomputed by every
    /// sweep). It sets the next threshold, so a heap of large live maps does
    /// not collect over and over for nothing.
    threshold: usize,
    /// Payloads of finished array builders, for the next builder (#503):
    /// a `map` or `filter` then allocates no Rust memory, releases nothing
    /// in the sweep and brings no collection closer. Kept double-boxed: the
    /// outer box is the thin pointer a PAYLOAD cell holds, reused as it is.
    #[allow(clippy::vec_box)]
    spare: Vec<Box<Box<dyn Payload>>>,
    live: usize,
    external_live: usize,
    collections: usize,
    external_collections: usize,
    allocated_before: usize,
    external_before: usize,
    /// Persistent map nodes allocated by this thread when the last
    /// collection ended (`maps::node_memory`): their growth since is
    /// pressure, their live bytes part of `external_live`.
    nodes_at_collection: usize,
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
    /// Objects alive after the last collection, by kind and descriptor:
    /// (count, bytes). A census for leak hunting (`--stats`).
    pub live: std::collections::BTreeMap<(&'static str, u32), (u64, u64)>,
    pub collection_nanos: u64,
    /// The two phases of collections: marking from the roots, and the sweep.
    pub mark_nanos: u64,
    pub sweep_nanos: u64,
    /// Time of the runtime's own work around collections (`Runtime::safepoint`).
    pub around_nanos: u64,
}

/// The census name of a header's kind.
fn kind_name(kind: u8) -> &'static str {
    match kind {
        OBJECT => "object",
        STRING => "string",
        ARRAY => "array",
        BIGINT => "bigint",
        PAYLOAD => "payload",
        CLOSURE => "closure",
        FRAME => "frame",
        _ => "other",
    }
}

fn census(live: &mut std::collections::BTreeMap<(&'static str, u32), (u64, u64)>, header: u64, bytes: usize) {
    let descriptor = if kind(header) == OBJECT { payload(header) } else { 0 };
    let entry = live.entry((kind_name(kind(header)), descriptor)).or_default();
    entry.0 += 1;
    entry.1 += bytes as u64;
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
            reserve: std::array::from_fn(|_| Vec::new()),
            large: Vec::new(),
            large_bytes: 0,
            allocated: 0,
            external: 0,
            threshold: COLLECTION_FLOOR,
            spare: Vec::new(),
            live: 0,
            external_live: 0,
            collections: 0,
            external_collections: 0,
            allocated_before: 0,
            external_before: 0,
            nodes_at_collection: 0,
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
        self.allocated + self.external + self.node_growth() >= self.threshold
    }

    /// What this heap holds now, the measure of a runtime's own memory
    /// budget (#269): its pages and large objects, the Rust memory its
    /// objects owned after the last collection, and what has been charged
    /// since (garbage among it until the next collection).
    pub fn held_bytes(&self) -> usize {
        self.pages.len() * PAGE_BYTES + self.large_bytes + self.external_live + self.external + self.node_growth()
    }

    /// Bytes of persistent map nodes allocated since the last collection.
    fn node_growth(&self) -> usize {
        crate::maps::node_memory().0.saturating_sub(self.nodes_at_collection)
    }

    /// Charge Rust memory that a heap object owns and just took on (a
    /// payload that grew). Nothing collects here: the next safepoint does.
    pub fn charge_external(&mut self, bytes: usize) {
        self.external += bytes;
    }

    /// Rust memory charged since the last collection and already freed: a
    /// builder's buffer when it finishes. It no longer brings the next
    /// collection closer (#503).
    pub fn discharge_external(&mut self, bytes: usize) {
        self.external = self.external.saturating_sub(bytes);
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
            external_allocated_bytes: self.external_before + self.external,
            external_live_bytes: self.external_live,
            external_collections: self.external_collections,
        }
    }

    /// Uninitialized storage of `words` words (header included); the caller
    /// writes the header and every word before the next collection.
    pub fn allocate(&mut self, words: usize) -> Word {
        let Some(class) = Self::class_of(words) else {
            self.allocated += words * 8;
            self.large_bytes += words * 8;
            let held = self.held_bytes();
            if let Some(watch) = &mut self.watch {
                watch.grow(words * 8, held);
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

    /// Pop a cell of `class` from its free list, refilling it from the swept
    /// cells or, when there are none, with a new page.
    pub fn allocate_class(&mut self, class: usize) -> Word {
        if self.free[class] == 0 {
            match self.reserve[class].pop() {
                Some((chain, bytes)) => {
                    self.free[class] = chain;
                    self.allocated += bytes;
                }
                None => self.add_page(class),
            }
        }
        let cell = self.free[class];
        // SAFETY: free cells are at least two words inside a live page.
        self.free[class] = unsafe { *((cell + 8) as *const u64) };
        cell
    }

    /// Growth is counted in pages, swept chains and large objects, not
    /// cells: machine code pops free cells inline, and only an empty list
    /// reaches the heap.
    fn add_page(&mut self, class: usize) {
        self.allocated += PAGE_WORDS * 8;
        let held = self.held_bytes() + PAGE_BYTES;
        if let Some(watch) = &mut self.watch {
            watch.grow(PAGE_BYTES, held);
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
        self.allocate_text_of(&[text])
    }

    /// A flat string of `parts` in order, copied once.
    pub fn allocate_text_of(&mut self, parts: &[&str]) -> Word {
        let bytes: usize = parts.iter().map(|part| part.len()).sum();
        let words = 1 + bytes.div_ceil(8).max(1);
        self.counted("string", 0, words);
        let string = self.allocate(words);
        let ascii = parts.iter().all(|part| part.is_ascii());
        // SAFETY: room for the header and `bytes` bytes.
        unsafe {
            *(string as *mut u64) = header(STRING, Repr::Raw, bytes as u32) | if ascii { ASCII } else { 0 };
            let mut at = (string + 8) as *mut u8;
            for part in parts {
                std::ptr::copy_nonoverlapping(part.as_ptr(), at, part.len());
                at = at.add(part.len());
            }
        }
        string
    }

    /// The texts of the strings in `array` with `separator` between them, as
    /// one flat string: the lengths are summed first and the bytes copied
    /// once into the new string, with no temporary (#447). None when the
    /// result would not fit the 4 GiB a string can hold.
    pub fn allocate_join(&mut self, array: Word, separator: Word) -> Option<Word> {
        // A trie's elements come as an owned copy, kept alive here; a flat or
        // shared array's are its own slots. Moving the copy does not move its
        // buffer, and an allocation neither moves nor frees heap objects.
        let (first, count, _copy) = match self.elements(array) {
            std::borrow::Cow::Borrowed(slots) => (slots.as_ptr(), slots.len(), None),
            std::borrow::Cow::Owned(copy) => (copy.as_ptr(), copy.len(), Some(copy)),
        };
        // SAFETY: see above; the slots stay valid until this returns.
        let items = unsafe { std::slice::from_raw_parts(first, count) };
        let (separator_at, separator_length) = self.text_bytes(separator);
        let mut bytes = separator_length.checked_mul(count.saturating_sub(1))?;
        let mut ascii = count < 2 || self.ascii_text(separator);
        for &item in items {
            bytes = bytes.checked_add(self.text_bytes(item).1)?;
            ascii = ascii && self.ascii_text(item);
        }
        if bytes > u32::MAX as usize {
            return None;
        }
        let words = 1 + bytes.div_ceil(8).max(1);
        self.counted("string", 0, words);
        let string = self.allocate(words);
        // SAFETY: room for the header and `bytes` bytes; the parts are other
        // strings, written once and never mutated.
        unsafe {
            *(string as *mut u64) = header(STRING, Repr::Raw, bytes as u32) | if ascii { ASCII } else { 0 };
            let mut at = (string + 8) as *mut u8;
            for (index, &item) in items.iter().enumerate() {
                if index != 0 {
                    std::ptr::copy_nonoverlapping(separator_at, at, separator_length);
                    at = at.add(separator_length);
                }
                let (from, length) = self.text_bytes(item);
                std::ptr::copy_nonoverlapping(from, at, length);
                at = at.add(length);
            }
        }
        Some(string)
    }

    /// Whether a string is all ASCII: its flag, or its bytes when the flag
    /// is not set (a view may leave it unset).
    fn ascii_text(&self, string: Word) -> bool {
        self.header_of(string) & ASCII != 0 || self.text(string).is_ascii()
    }

    /// The address of the first byte of a string and its length in bytes:
    /// inline in a flat string, in a text buffer for a [`SHARED`] one.
    fn text_bytes(&self, string: Word) -> (*const u8, usize) {
        let header = self.header_of(string);
        let first = if header & SHARED != 0 {
            // SAFETY: a shared string has three words.
            unsafe { *((string + 8) as *const u64) }
        } else {
            string + 8
        };
        (first as *const u8, payload(header) as usize)
    }

    // ---- text buffers (#255) ----

    /// A text buffer with room for `capacity` bytes (rounded up to a power of
    /// two) holding `parts` in order from byte `front` on. Word 1 is the
    /// lowest byte written, the payload one past the highest; bytes between
    /// them are never written again, so versions can share them.
    pub fn allocate_text_buffer(&mut self, parts: &[&str], capacity: usize, front: usize) -> Word {
        let capacity = capacity.next_power_of_two();
        let used: usize = parts.iter().map(|part| part.len()).sum();
        assert!(front + used <= capacity, "text buffer too small");
        let words = 2 + capacity / 8;
        self.counted("text buffer", 0, words);
        let buffer = self.allocate(words);
        // SAFETY: room for two words and `capacity` bytes.
        unsafe {
            *(buffer as *mut u64) = header(STRING, Repr::Raw, (front + used) as u32)
                | BUFFER
                | (u64::from(capacity.trailing_zeros()) << CAPACITY_SHIFT);
            *((buffer + 8) as *mut u64) = front as u64;
            let mut at = (buffer + 16 + front as Word) as *mut u8;
            for part in parts {
                std::ptr::copy_nonoverlapping(part.as_ptr(), at, part.len());
                at = at.add(part.len());
            }
        }
        buffer
    }

    /// The lowest byte written, one past the highest, and the capacity of a
    /// text buffer.
    pub fn text_buffer_room(&self, buffer: Word) -> (usize, usize, usize) {
        let header = self.header_of(buffer);
        // SAFETY: a text buffer has its low mark in word 1.
        let low = unsafe { *((buffer + 8) as *const u64) } as usize;
        (low, payload(header) as usize, 1 << ((header >> CAPACITY_SHIFT) & 63))
    }

    /// The text buffer of a [`SHARED`] string and the offset of its first byte.
    pub fn text_shared(&self, string: Word) -> (Word, usize) {
        // SAFETY: a shared string has three words.
        unsafe {
            let (first, buffer) = (*((string + 8) as *const u64), *((string + 16) as *const u64));
            (buffer, (first - buffer - 16) as usize)
        }
    }

    /// Is `string` a version in a text buffer?
    pub fn is_text_view(&self, string: Word) -> bool {
        self.header_of(string) & SHARED != 0
    }

    /// Write `text` at the high end of `buffer`, or right before its low end
    /// (the caller checked the room). `text` may be part of a version of the
    /// same buffer: it lies between the marks, the write outside them.
    pub fn text_buffer_write(&mut self, buffer: Word, text: &str, at_front: bool) {
        let (low, high, capacity) = self.text_buffer_room(buffer);
        let at = if at_front { low.checked_sub(text.len()).expect("text buffer underflow") } else { high };
        assert!(at + text.len() <= capacity, "text buffer overflow");
        // SAFETY: the bytes written are inside the buffer and outside every
        // version, which lie between the marks.
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), (buffer + 16 + at as Word) as *mut u8, text.len());
            if at_front {
                *((buffer + 8) as *mut u64) = at as u64;
            } else {
                let header = &mut *(buffer as *mut u64);
                *header = (*header & 0xffff_ffff) | (((high + text.len()) as u64) << 32);
            }
        }
    }

    /// A string of `length` bytes of `buffer` from byte `start`.
    pub fn allocate_text_view(&mut self, buffer: Word, start: usize, length: usize, ascii: bool) -> Word {
        self.counted("string view", 0, 3);
        let view = self.allocate(3);
        // SAFETY: three words.
        unsafe {
            *(view as *mut u64) = header(STRING, Repr::Raw, length as u32) | SHARED | if ascii { ASCII } else { 0 };
            *((view + 8) as *mut u64) = buffer + 16 + start as u64;
            *((view + 16) as *mut u64) = buffer;
        }
        view
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
            let (first, length) = self.text_bytes(string);
            // SAFETY: as in `text`; the string outlives this call.
            unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(first, length)) }
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
        let (first, length) = self.text_bytes(string);
        // SAFETY: strings are written once from valid UTF-8 and never mutated;
        // the bytes of a text buffer a version covers are never written again.
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(first, length)) }
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
        if header & TRIE != 0 {
            return std::borrow::Cow::Owned(self.trie_elements(array));
        }
        if header & SHARED != 0 {
            let (buffer, start) = self.shared(array);
            // SAFETY: a version's slots are below its buffer's frontier, which
            // nothing writes again.
            return std::borrow::Cow::Borrowed(unsafe {
                std::slice::from_raw_parts((buffer + 8 + start as Word * 8) as *const u64, length)
            });
        }
        // SAFETY: arrays are immutable after construction.
        std::borrow::Cow::Borrowed(unsafe { std::slice::from_raw_parts((array + 8) as *const u64, length) })
    }

    // ---- shared buffers (#246) ----

    /// The buffer of a [`SHARED`] array and the position of its first element.
    pub fn shared(&self, array: Word) -> (Word, usize) {
        // SAFETY: a shared array has three words.
        unsafe {
            let (first, buffer) = (*((array + 8) as *const u64), *((array + 16) as *const u64));
            (buffer, ((first - buffer - 8) / 8) as usize)
        }
    }

    /// A buffer with room for `capacity` elements (rounded up to a power of
    /// two) holding `parts` in order from slot `front` on; the slots before
    /// them hold [`free_slot`], so that a prepend can claim them (#247) and
    /// the collector, which traces the buffer from slot 0, finds no pointer.
    pub fn allocate_buffer(&mut self, repr: Repr, parts: &[&[Word]], capacity: usize, front: usize) -> Word {
        let capacity = capacity.next_power_of_two();
        let used: usize = front + parts.iter().map(|part| part.len()).sum::<usize>();
        assert!(used <= capacity, "buffer too small");
        self.counted("array buffer", 0, capacity + 1);
        let buffer = self.allocate(capacity + 1);
        // SAFETY: room for the header and `capacity` elements.
        unsafe {
            *(buffer as *mut u64) = header(ARRAY, repr, used as u32)
                | BUFFER
                | (u64::from(capacity.trailing_zeros()) << CAPACITY_SHIFT);
            std::slice::from_raw_parts_mut((buffer + 8) as *mut u64, front).fill(free_slot(repr));
            let mut at = buffer + 8 + front as Word * 8;
            for part in parts {
                std::ptr::copy_nonoverlapping(part.as_ptr(), at as *mut u64, part.len());
                at += part.len() as Word * 8;
            }
        }
        buffer
    }

    /// Can `count` elements go right before slot `start` of `buffer`? Only
    /// when `start` is the buffer's lowest written slot (the slot before it
    /// is still free) and there is room.
    pub fn buffer_front_free(&self, buffer: Word, start: usize, count: usize, repr: Repr) -> bool {
        // SAFETY: slot `start - 1` is inside the buffer when `start > 0`.
        start >= count && count > 0 && unsafe { *((buffer + 8 + (start as Word - 1) * 8) as *const u64) } == free_slot(repr)
    }

    /// Write `items` right before slot `start` (the caller checked the room).
    pub fn buffer_prepend(&mut self, buffer: Word, start: usize, items: &[Word]) {
        assert!(start >= items.len(), "buffer underflow");
        // SAFETY: the slots before `start` are inside the buffer and free.
        unsafe {
            std::ptr::copy_nonoverlapping(items.as_ptr(), (buffer + 8 + (start - items.len()) as Word * 8) as *mut u64, items.len());
        }
    }

    /// Is `array` a buffer of shared arrays ([`BUFFER`])?
    pub fn is_buffer(&self, array: Word) -> bool {
        self.header_of(array) & BUFFER != 0
    }

    /// The frontier (slots written) and the capacity of a buffer.
    pub fn buffer_room(&self, buffer: Word) -> (usize, usize) {
        let header = self.header_of(buffer);
        (payload(header) as usize, 1 << ((header >> CAPACITY_SHIFT) & 63))
    }

    /// Write `items` at the frontier of `buffer` (the caller checked the room).
    pub fn buffer_extend(&mut self, buffer: Word, items: &[Word]) {
        let (used, capacity) = self.buffer_room(buffer);
        assert!(used + items.len() <= capacity, "buffer overflow");
        // SAFETY: the slots past the frontier are inside the buffer and no
        // version covers them yet.
        unsafe {
            std::ptr::copy_nonoverlapping(items.as_ptr(), (buffer + 8 + used as Word * 8) as *mut u64, items.len());
            let header = &mut *(buffer as *mut u64);
            *header = (*header & 0xffff_ffff) | (((used + items.len()) as u64) << 32);
        }
    }

    /// An array version of `length` elements of `buffer` from `start`.
    pub fn allocate_view(&mut self, repr: Repr, buffer: Word, start: usize, length: usize) -> Word {
        self.counted("array view", 0, 3);
        let view = self.allocate(3);
        // SAFETY: three words.
        unsafe {
            *(view as *mut u64) = header(ARRAY, repr, length as u32) | SHARED;
            *((view + 8) as *mut u64) = buffer + 8 + start as u64 * 8;
            *((view + 16) as *mut u64) = buffer;
        }
        view
    }

    /// Mark a flat array as made by an append (see [`APPENDED`]).
    pub fn mark_appended(&mut self, array: Word) {
        self.mark(array, APPENDED);
    }

    /// Set a header flag of an array the caller just allocated.
    pub fn mark(&mut self, array: Word, flag: u64) {
        // SAFETY: `array` is a live array just allocated by the caller.
        unsafe { *(array as *mut u64) |= flag }
    }

    /// Clear representation flags of a live object: hints of how it was
    /// made, which no reader of its value looks at (#255).
    pub fn unmark(&mut self, object: Word, flags: u64) {
        // SAFETY: `object` is a live object; one thread owns the heap.
        unsafe { *(object as *mut u64) &= !flags }
    }

    // ---- tries (#250) ----

    fn trie_parts(&self, array: Word) -> (Word, Word, u32) {
        // SAFETY: a trie array has four words; the low byte of the fourth is
        // the shift, the rest counts the reads of machine code.
        unsafe {
            (*((array + 8) as *const u64), *((array + 16) as *const u64), (*((array + 24) as *const u64) & 0xff) as u32)
        }
    }

    fn trie_tail_offset(length: usize) -> usize {
        if length < TRIE_WIDTH { 0 } else { ((length - 1) >> TRIE_BITS) << TRIE_BITS }
    }

    fn word_at(node: Word, index: usize) -> Word {
        // SAFETY: `node` is a live flat array with more than `index` elements.
        unsafe { *((node + 8 + index as Word * 8) as *const u64) }
    }

    /// A trie of `items` (all of them, built bottom-up).
    pub fn trie_from(&mut self, repr: Repr, items: &[Word]) -> Word {
        let length = items.len();
        let offset = Self::trie_tail_offset(length);
        let mut level: Vec<Word> = items[..offset].chunks(TRIE_WIDTH).map(|leaf| self.allocate_array(repr, leaf)).collect();
        let mut shift = TRIE_BITS;
        while level.len() > TRIE_WIDTH {
            level = level.chunks(TRIE_WIDTH).map(|group| self.allocate_array(Repr::Ref, group)).collect();
            shift += TRIE_BITS;
        }
        let root = self.allocate_array(Repr::Ref, &level);
        let tail = self.allocate_array(repr, &items[offset..]);
        self.trie_object(repr, length, root, tail, shift)
    }

    fn trie_object(&mut self, repr: Repr, length: usize, root: Word, tail: Word, shift: u32) -> Word {
        self.counted("array trie", 0, 4);
        let object = self.allocate(4);
        // SAFETY: four words.
        unsafe {
            *(object as *mut u64) = header(ARRAY, repr, length as u32) | TRIE;
            *((object + 8) as *mut u64) = root;
            *((object + 16) as *mut u64) = tail;
            *((object + 24) as *mut u64) = u64::from(shift);
        }
        object
    }

    /// Element `index` (in bounds) of a trie: the tail, or a walk of
    /// ⌈log₃₂ n⌉ branches down to a leaf.
    pub fn trie_get(&self, array: Word, index: usize) -> Word {
        let length = payload(self.header_of(array)) as usize;
        let (mut node, tail, mut shift) = self.trie_parts(array);
        let offset = Self::trie_tail_offset(length);
        if index >= offset {
            return Self::word_at(tail, index - offset);
        }
        while shift > 0 {
            node = Self::word_at(node, (index >> shift) & TRIE_MASK);
            shift -= TRIE_BITS;
        }
        Self::word_at(node, index & TRIE_MASK)
    }

    /// Element `index` (in bounds) read by machine code. A trie that is read
    /// rather than changed flattens itself: after as many reads as two passes
    /// over it, the object becomes a shared view ([`SHARED`]) of a flat copy,
    /// which machine code reads inline. It stays [`REPLACED`], so the next
    /// replacement makes it a trie again. Every holder of the word sees the
    /// same elements before and after.
    pub fn trie_read(&mut self, array: Word, index: usize) -> Word {
        let item = self.trie_get(array, index);
        let header = self.header_of(array);
        let length = payload(header) as u64;
        // SAFETY: a trie array has four words.
        let counter = unsafe { &mut *((array + 24) as *mut u64) };
        if (*counter >> 8) + 1 < 2 * length {
            *counter += 1 << 8;
            return item;
        }
        let items = self.trie_elements(array);
        let flat = self.allocate_array(element_repr(header), &items);
        // SAFETY: the four words of the trie become the three of a view.
        unsafe {
            *(array as *mut u64) = (header & !TRIE) | SHARED | REPLACED;
            *((array + 8) as *mut u64) = flat + 8;
            *((array + 16) as *mut u64) = flat;
            *((array + 24) as *mut u64) = 0;
        }
        item
    }

    /// A new trie with element `index` (in bounds) replaced: the nodes on its
    /// path are copied, every other node is shared with `array`.
    pub fn trie_set(&mut self, array: Word, index: usize, item: Word) -> Word {
        let header = self.header_of(array);
        let (repr, length) = (element_repr(header), payload(header) as usize);
        let (root, tail, shift) = self.trie_parts(array);
        let offset = Self::trie_tail_offset(length);
        if index >= offset {
            let mut items = self.elements(tail).into_owned();
            items[index - offset] = item;
            let tail = self.allocate_array(repr, &items);
            return self.trie_object(repr, length, root, tail, shift);
        }
        let root = self.trie_path(root, shift, index, item, repr);
        self.trie_object(repr, length, root, tail, shift)
    }

    fn trie_path(&mut self, node: Word, shift: u32, index: usize, item: Word, repr: Repr) -> Word {
        let mut words = self.elements(node).into_owned();
        if shift == 0 {
            words[index & TRIE_MASK] = item;
            return self.allocate_array(repr, &words);
        }
        let at = (index >> shift) & TRIE_MASK;
        words[at] = self.trie_path(words[at], shift - TRIE_BITS, index, item, repr);
        self.allocate_array(Repr::Ref, &words)
    }

    /// Every element of a trie, in order.
    fn trie_elements(&self, array: Word) -> Vec<Word> {
        fn walk(heap: &Heap, node: Word, shift: u32, out: &mut Vec<Word>) {
            let words = heap.elements(node);
            if shift == 0 {
                out.extend_from_slice(&words);
            } else {
                for &child in words.iter() {
                    walk(heap, child, shift - TRIE_BITS, out);
                }
            }
        }
        let (root, tail, shift) = self.trie_parts(array);
        let mut out = Vec::with_capacity(payload(self.header_of(array)) as usize);
        walk(self, root, shift, &mut out);
        out.extend_from_slice(&self.elements(tail));
        out
    }

    /// Every element of `array` in order, with nothing materialised (#446):
    /// a byte array's bytes widened one at a time, a trie walked in place.
    pub fn each_element(&self, array: Word, visit: &mut impl FnMut(Word)) {
        fn walk(heap: &Heap, node: Word, shift: u32, visit: &mut impl FnMut(Word)) {
            for &word in heap.elements(node).iter() {
                if shift == 0 {
                    visit(word);
                } else {
                    walk(heap, word, shift - TRIE_BITS, visit);
                }
            }
        }
        let header = self.header_of(array);
        if header & PACKED != 0 {
            self.bytes(array).iter().for_each(|&byte| visit(Word::from(byte)));
        } else if header & TRIE != 0 {
            let (root, tail, shift) = self.trie_parts(array);
            walk(self, root, shift, visit);
            self.elements(tail).iter().for_each(|&word| visit(word));
        } else {
            self.elements(array).iter().for_each(|&word| visit(word));
        }
    }

    /// The number of elements of an array, whatever its representation.
    pub fn array_length(&self, array: Word) -> usize {
        payload(self.header_of(array)) as usize
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
        // The digits live outside the heap pages; charge them so that garbage
        // big integers trigger collections by their real size.
        self.external += big_bytes(&n);
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
        // The Rust memory the payload owns; the payload itself and its boxes
        // are charged by `allocate_payload_moved`, each once (#332).
        self.external += payload.external_bytes();
        self.allocate_payload_moved(payload)
    }
    /// [`Heap::allocate_payload`] for Rust memory already charged: a builder's
    /// entries moved into the finished value. What is new is charged: the
    /// payload struct and its two boxes, not the contents it took over.
    pub fn allocate_payload_moved(&mut self, payload: Box<dyn Payload>) -> Word {
        self.external += BOX_BYTES + std::mem::size_of_val(&*payload);
        self.counted("payload", 0, 2);
        if self.allocations.is_some() {
            self.counted(payload.kind_name(), 0, 0);
        }
        let object = self.allocate(2);
        // SAFETY: two fresh words; the double box gives a thin pointer.
        unsafe {
            *(object as *mut u64) = header(PAYLOAD, Repr::Raw, 0);
            *((object + 8) as *mut u64) = Box::into_raw(Box::new(payload)) as u64;
        }
        object
    }
    pub fn payload<T: Payload>(&self, object: Word) -> Option<&T> {
        // SAFETY: a PAYLOAD object holds a live Box<Box<dyn Payload>>, or
        // null once its payload was retired.
        (kind(self.header_of(object)) == PAYLOAD && self.field(object, 0) != 0)
            .then(|| unsafe { &**(self.field(object, 0) as *const Box<dyn Payload>) })
            .and_then(|p| p.as_any().downcast_ref())
    }
    pub fn payload_mut<T: Payload>(&mut self, object: Word) -> Option<&mut T> {
        // SAFETY: as above, uniquely borrowed through &mut self.
        (kind(self.header_of(object)) == PAYLOAD && self.field(object, 0) != 0)
            .then(|| unsafe { &mut **(self.field(object, 0) as *mut Box<dyn Payload>) })
            .and_then(|p| p.as_any_mut().downcast_mut())
    }

    /// A PAYLOAD object around a spare payload that `reuse` takes (and
    /// resets), or none when there is no spare.
    pub fn allocate_spare(&mut self, reuse: impl FnOnce(&mut dyn Payload) -> bool) -> Option<Word> {
        let mut boxed = self.spare.pop()?;
        if !reuse(&mut **boxed) {
            return None;
        }
        self.counted("payload", 0, 2);
        let object = self.allocate(2);
        // SAFETY: two fresh words.
        unsafe {
            *(object as *mut u64) = header(PAYLOAD, Repr::Raw, 0);
            *((object + 8) as *mut u64) = Box::into_raw(boxed) as u64;
        }
        Some(object)
    }

    /// Take the payload out of a finished object for a later one: the object
    /// then owns nothing, and its payload is kept while there is room.
    pub fn retire_payload(&mut self, object: Word) {
        if kind(self.header_of(object)) != PAYLOAD {
            return;
        }
        let pointer = self.field(object, 0) as *mut Box<dyn Payload>;
        if pointer.is_null() {
            return;
        }
        // SAFETY: the object's live payload, taken out exactly once.
        let boxed = unsafe {
            *((object + 8) as *mut u64) = 0;
            Box::from_raw(pointer)
        };
        if self.spare.len() < SPARE_PAYLOADS {
            self.spare.push(boxed);
        }
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
        self.collect_with(roots, |_, _| {});
    }

    /// `collect` with ephemerons (#503): after everything reachable from
    /// `roots` is marked, `ephemerons` is asked, with a test of whether an
    /// object is marked, to report the values of entries whose keys are.
    /// It is asked again until it reports nothing unmarked, so a value is kept
    /// only while its key is reachable some other way, and a cycle through
    /// the entry (a group's task referring to the group) is collected.
    pub fn collect_with(
        &mut self,
        roots: impl FnOnce(&mut dyn FnMut(Word, Repr)),
        mut ephemerons: impl FnMut(&dyn Fn(Word) -> bool, &mut dyn FnMut(Word, Repr)),
    ) {
        // A freed string's address may be reused for another one.
        self.positions = Positions::default();
        let started = self.allocations.is_some().then(std::time::Instant::now);
        self.collect_inner(roots, &mut ephemerons);
        if let (Some(allocations), Some(started)) = (&mut self.allocations, started) {
            allocations.collection_nanos += started.elapsed().as_nanos() as u64;
        }
    }

    fn collect_inner(
        &mut self,
        roots: impl FnOnce(&mut dyn FnMut(Word, Repr)),
        ephemerons: &mut Ephemerons<'_>,
    ) {
        crate::maps::begin_pass();
        let marking = self.allocations.is_some().then(std::time::Instant::now);
        let mut worklist = std::mem::take(&mut self.worklist);
        // An object is marked when it is queued, not when it is traced
        // (#438): one shared by many edges enters the worklist once.
        roots(&mut |word, repr| {
            if let Some(pointer) = repr.pointer(word) {
                // SAFETY: a root is a live object pointer.
                unsafe { mark_and_queue(pointer, &mut worklist) };
            }
        });
        loop {
            self.mark_reachable(&mut worklist);
            // SAFETY: keys are live object pointers; their headers are read.
            let marked = |object: Word| unsafe { *(object as *const u64) } & MARK != 0;
            let mut found = false;
            ephemerons(&marked, &mut |word, repr| {
                if let Some(pointer) = repr.pointer(word)
                    && unsafe { *(pointer as *const u64) } & MARK == 0
                {
                    // SAFETY: an ephemeron value is a live object pointer.
                    unsafe { mark_and_queue(pointer, &mut worklist) };
                    found = true;
                }
            });
            if !found {
                break;
            }
        }
        if let (Some(allocations), Some(marking)) = (&mut self.allocations, marking) {
            allocations.mark_nanos += marking.elapsed().as_nanos() as u64;
        }
        self.worklist = worklist;
        // Watched objects that died, found by their marks before the sweep:
        // a pass over the watched few, instead of a set lookup for every dead
        // cell in the sweep (#503).
        if !self.watched.is_empty() {
            // SAFETY: watched addresses are objects allocated and not yet
            // reported dead, so their headers are live until this sweep.
            let dead: Vec<Word> = self
                .watched
                .iter()
                .copied()
                .filter(|&object| unsafe { *(object as *const u64) } & MARK == 0)
                .collect();
            for object in dead {
                self.watched.remove(&object);
                self.released.push(object);
            }
        }
        let swept = self.allocations.is_some().then(std::time::Instant::now);
        self.sweep();
        if let (Some(allocations), Some(swept)) = (&mut self.allocations, swept) {
            allocations.sweep_nanos += swept.elapsed().as_nanos() as u64;
        }
    }

    /// Mark everything reachable from the worklist.
    fn mark_reachable(&self, worklist: &mut Vec<Word>) {
        while let Some(object) = worklist.pop() {
            // SAFETY: every queued word is a live object pointer, marked when
            // it was queued; the header's other bits are read without it.
            let header = unsafe { *(object as *const u64) } & !MARK;
            let mut push = |word: Word, repr: Repr| {
                if let Some(pointer) = repr.pointer(word) {
                    // SAFETY: a traced field holds a live object pointer.
                    unsafe { mark_and_queue(pointer, worklist) };
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
                // A shared version keeps its buffer alive; the buffer traces
                // the slots below its frontier like any array.
                ARRAY if header & SHARED != 0 => push(unsafe { *((object + 16) as *const u64) }, Repr::Ref),
                // A string version keeps its text buffer alive (#255).
                STRING if header & SHARED != 0 => push(unsafe { *((object + 16) as *const u64) }, Repr::Ref),
                // A trie keeps its root and its tail; they trace the rest.
                ARRAY if header & TRIE != 0 => {
                    push(unsafe { *((object + 8) as *const u64) }, Repr::Ref);
                    push(unsafe { *((object + 16) as *const u64) }, Repr::Ref);
                }
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
                    let pointer = unsafe { *((object + 8) as *const u64) };
                    if pointer != 0 {
                        let payload = unsafe { &**(pointer as *const Box<dyn Payload>) };
                        payload.trace(&mut push);
                    }
                }
                _ => {}
            }
        }
    }

    /// Keep as many empty pages as the next cycle can fill before its
    /// collection, plus a few, and free the rest (#435): after a burst the
    /// heap shrinks back instead of keeping, and sweeping, its high-water
    /// pages for the life of the program. Kept pages offer their cells like
    /// any other; the allocator returns freed ones to the system.
    fn release_empty_pages(&mut self, empty: Vec<(usize, Word, usize)>) {
        let keep = self.threshold / PAGE_BYTES + EMPTY_PAGES_SLACK;
        let mut freed = Vec::new();
        for (count, (index, chain, bytes)) in empty.into_iter().enumerate() {
            if count < keep || self.stress {
                self.reserve[self.pages[index].class].push((chain, bytes));
            } else {
                freed.push(index);
            }
        }
        if freed.is_empty() {
            return;
        }
        let mut index = 0;
        let mut next = freed.iter().peekable();
        self.pages.retain(|_| {
            let drop = next.peek() == Some(&&index);
            if drop {
                next.next();
            }
            index += 1;
            !drop
        });
    }

    /// Overwrite every free cell past its link (collector stress).
    fn poison(&mut self) {
        for (class, &size) in CLASSES.iter().enumerate() {
            let chains = self.reserve[class].iter().map(|&(chain, _)| chain);
            for mut cell in std::iter::once(self.free[class]).chain(chains) {
                while cell != 0 {
                    for word in 2..size {
                        // SAFETY: a free cell of `size` words inside a live page.
                        unsafe { *((cell + word as Word * 8) as *mut u64) = POISON };
                    }
                    cell = unsafe { *((cell + 8) as *const u64) };
                }
            }
        }
    }

    fn sweep(&mut self) {
        self.free = [0; CLASSES.len()];
        for chains in &mut self.reserve {
            chains.clear();
        }
        if let Some(allocations) = &mut self.allocations {
            allocations.live.clear();
        }
        let mut live = 0;
        let mut external_live = 0;
        // Pages with no live cell (#435): their chains wait until the end of
        // the sweep decides how many empty pages the next cycle needs.
        let mut empty: Vec<(usize, Word, usize)> = Vec::new();
        for (index, page) in self.pages.iter_mut().enumerate() {
            let size = CLASSES[page.class];
            let base = page.memory.as_mut_ptr() as Word;
            let cells = page.memory.len() / size;
            let mut kept = 0;
            let mut chain = 0;
            for cell in (0..cells).rev() {
                let address = base + (cell * size * 8) as Word;
                // SAFETY: every cell of a page starts with a header.
                let header = unsafe { *(address as *const u64) };
                if header & MARK != 0 {
                    unsafe { *(address as *mut u64) = header & !MARK };
                    live += size * 8;
                    kept += 1;
                    external_live += unsafe { owned_bytes(address, header) };
                    if let Some(allocations) = &mut self.allocations {
                        census(&mut allocations.live, header, size * 8);
                    }
                    continue;
                }
                if kind(header) != FREE {
                    unsafe { release(address, header) };
                    unsafe { *(address as *mut u64) = 0 };
                }
                unsafe { *((address + 8) as *mut u64) = chain };
                chain = address;
            }
            if kept == 0 {
                empty.push((index, chain, cells * size * 8));
            } else if chain != 0 {
                self.reserve[page.class].push((chain, (cells - kept) * size * 8));
            }
        }
        let mut large_bytes = 0;
        self.large.retain_mut(|memory| {
            let address = memory.as_mut_ptr() as Word;
            let header = memory[0];
            if header & MARK != 0 {
                memory[0] = header & !MARK;
                live += memory.len() * 8;
                if let Some(allocations) = &mut self.allocations {
                    census(&mut allocations.live, header, memory.len() * 8);
                }
                large_bytes += memory.len() * 8;
                external_live += unsafe { owned_bytes(address, header) };
                true
            } else {
                unsafe { release(address, header) };
                false
            }
        });
        if self.stress {
            self.poison();
        }
        self.large_bytes = large_bytes;
        self.live = live;
        self.live_sum += live;
        // Map nodes freed by this sweep are already gone from the live count.
        let (nodes_allocated, nodes_live) = crate::maps::node_memory();
        let external = self.external + nodes_allocated.saturating_sub(self.nodes_at_collection);
        if external > self.allocated {
            self.external_collections += 1;
        }
        self.allocated_before += self.allocated;
        self.allocated = 0;
        self.external_before += external;
        self.external = 0;
        self.nodes_at_collection = nodes_allocated;
        let external_live = external_live + nodes_live;
        self.external_live = external_live;
        self.threshold = if self.stress { 0 } else { (live + external_live).max(COLLECTION_FLOOR) };
        self.release_empty_pages(empty);
        self.collections += 1;
    }
}

/// The word of a buffer slot nothing has written (#247): no element is ever
/// this word — an `int` or a reference is never 0, a raw word never the
/// unbound marker — and neither is a pointer to the collector.
pub fn free_slot(repr: Repr) -> Word {
    if repr == Repr::Raw { crate::word::UNBOUND_RAW } else { crate::word::UNBOUND }
}

/// The two boxes of a payload cell (`Box<Box<dyn Payload>>`): the fat
/// pointer and the allocator's rounding of each.
const BOX_BYTES: usize = 32;

/// Rust memory of a payload: its boxes, its struct and what it owns.
fn payload_bytes(payload: &dyn Payload) -> usize {
    BOX_BYTES + std::mem::size_of_val(payload) + payload.external_bytes()
}

/// Rust memory of the digits of a big integer and its box.
fn big_bytes(n: &BigInt) -> usize {
    (n.bits() / 8) as usize + 32
}

/// Rust memory owned by a live object: a payload or a big integer.
///
/// SAFETY: `address` is a live object with `header`.
unsafe fn owned_bytes(address: Word, header: u64) -> usize {
    match kind(header) {
        BIGINT => big_bytes(unsafe { &*(*((address + 8) as *const u64) as *const BigInt) }),
        PAYLOAD if unsafe { *((address + 8) as *const u64) } == 0 => 0,
        PAYLOAD => payload_bytes(unsafe { &**(*((address + 8) as *const u64) as *const Box<dyn Payload>) }),
        _ => 0,
    }
}

/// Mark `object` and queue it for tracing, unless it is marked already
/// (#438): each reachable object enters the worklist once.
///
/// SAFETY: `object` is a live object pointer.
#[inline(always)]
unsafe fn mark_and_queue(object: Word, worklist: &mut Vec<Word>) {
    let header = unsafe { *(object as *const u64) };
    if header & MARK == 0 {
        unsafe { *(object as *mut u64) = header | MARK };
        worklist.push(object);
    }
}

/// Drop Rust data owned by a dead object.
///
/// SAFETY: `address` is a dead object with `header`, released exactly once.
unsafe fn release(address: Word, header: u64) {
    match kind(header) {
        BIGINT => drop(unsafe { Box::from_raw(*((address + 8) as *const u64) as *mut BigInt) }),
        PAYLOAD => {
            let pointer = unsafe { *((address + 8) as *const u64) };
            if pointer != 0 {
                drop(unsafe { Box::from_raw(pointer as *mut Box<dyn Payload>) })
            }
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
    fn every_representation_walks_its_elements_in_order() {
        let mut heap = Heap::new(layouts());
        let words: Vec<Word> = (0..300).map(|i| heap.int(i)).collect();
        let flat = heap.allocate_array(Repr::Ref, &words[..5]);
        let packed = heap.allocate_bytes(&[1, 2, 255]);
        let trie = heap.trie_from(Repr::Ref, &words);
        let buffer = heap.allocate_buffer(Repr::Ref, &[&words[..10]], 16, 3);
        let shared = heap.allocate_view(Repr::Ref, buffer, 3, 10);
        let empty = heap.allocate_array(Repr::Ref, &[]);
        for array in [flat, packed, trie, shared, empty] {
            let mut walked = Vec::new();
            heap.each_element(array, &mut |word| walked.push(word));
            assert_eq!(walked, heap.elements(array).into_owned());
            assert_eq!(heap.array_length(array), walked.len());
        }
    }

    #[test]
    fn a_join_copies_each_part_once_with_the_right_flag() {
        let mut heap = Heap::new(layouts());
        let parts = ["alpha", "beta", "gamma"].map(|text| heap.allocate_text(text));
        let array = heap.allocate_array(Repr::Ref, &parts);
        let comma = heap.allocate_text(", ");
        let bullet = heap.allocate_text(" · ");
        let joined = heap.allocate_join(array, comma).unwrap();
        assert_eq!(heap.text(joined), "alpha, beta, gamma");
        assert!(heap.is_ascii_text(joined));
        // A multibyte separator clears the flag only when it is used.
        let joined = heap.allocate_join(array, bullet).unwrap();
        assert_eq!(heap.text(joined), "alpha · beta · gamma");
        assert!(!heap.is_ascii_text(joined));
        let one = heap.allocate_array(Repr::Ref, &parts[..1]);
        let single = heap.allocate_join(one, bullet).unwrap();
        assert_eq!(heap.text(single), "alpha");
        assert!(heap.is_ascii_text(single));
        let empty = heap.allocate_array(Repr::Ref, &[]);
        let nothing = heap.allocate_join(empty, comma).unwrap();
        assert_eq!(heap.text(nothing), "");
        // A multibyte part, and a trie's elements in order.
        let greek = [heap.allocate_text("α"), heap.allocate_text("b")];
        let mixed = heap.allocate_array(Repr::Ref, &greek);
        let joined = heap.allocate_join(mixed, comma).unwrap();
        assert_eq!(heap.text(joined), "α, b");
        assert!(!heap.is_ascii_text(joined));
        let many: Vec<Word> = (0..100).map(|i| heap.allocate_text(&i.to_string())).collect();
        let trie = heap.trie_from(Repr::Ref, &many);
        let joined = heap.allocate_join(trie, comma).unwrap();
        let expected: Vec<String> = (0..100).map(|i| i.to_string()).collect();
        assert_eq!(heap.text(joined), expected.join(", "));
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
    fn reused_cells_bring_a_collection_and_the_heap_stops_growing() {
        // #522: garbage of heap cells alone, nothing charged as Rust memory.
        // Before, only new pages counted, so every cycle first reused all
        // the swept cells and then grew by a whole threshold of pages.
        let mut heap = Heap::new(layouts());
        let mut warmed = 0;
        for round in 0..60 {
            for _ in 0..20_000 {
                let value = heap.int(7);
                let _garbage = heap.object_new(0, &[value, value]);
                if heap.needs_collection() {
                    heap.collect(|_| {});
                }
            }
            if round == 9 {
                warmed = heap.stats().pages;
            }
        }
        assert!(heap.stats().collections >= 10, "{:?}", heap.stats());
        assert!(heap.stats().pages <= warmed + 1, "{warmed} pages after warm-up: {:?}", heap.stats());
    }

    #[test]
    fn empty_pages_after_a_burst_are_released() {
        // #435: a burst keeps many objects alive, then they all die. The heap
        // keeps only the pages the next cycle can fill, not its high water.
        let mut heap = Heap::new(layouts());
        let mut head = UNBOUND;
        for n in 0..400_000 {
            let value = heap.int(n);
            head = heap.object_new(0, &[head, value]);
        }
        heap.collect(|visit| visit(head, Repr::Ref));
        let peak = heap.stats().pages;
        assert!(peak > 100, "{peak} pages at the peak");
        heap.collect(|_| {});
        let after = heap.stats().pages;
        let keep = COLLECTION_FLOOR / PAGE_BYTES + EMPTY_PAGES_SLACK;
        assert!(after <= keep, "{after} pages kept, at most {keep}; {peak} at the peak");
        // The kept pages still serve allocations, and the heap works on.
        let kept = heap.allocate_text("after the burst");
        heap.collect(|visit| visit(kept, Repr::Ref));
        assert_eq!(heap.text(kept), "after the burst");
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

    /// A payload that owns the given number of bytes of Rust memory.
    #[derive(Debug)]
    struct Owning(usize);
    impl Payload for Owning {
        fn external_bytes(&self) -> usize {
            self.0
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    /// #331: Rust memory owned by heap objects drives collection like heap
    /// cells do. Dead owners trigger a collection by what they own; live ones
    /// raise the next threshold instead of making every safepoint collect;
    /// pressure starts over at each collection, the live estimate is
    /// recomputed by the sweep, and big integer digits count too.
    #[test]
    fn owned_rust_memory_drives_collection() {
        let mut heap = Heap::new(layouts());
        // Two small cells and a megabyte owned: due before any page fills.
        let _dead = heap.allocate_payload(Box::new(Owning(COLLECTION_FLOOR)));
        assert!(heap.needs_collection(), "{:?}", heap.stats());
        heap.collect(|_| {});
        let stats = heap.stats();
        assert_eq!((stats.external_live_bytes, stats.external_collections), (0, 1));
        assert!(!heap.needs_collection());

        let kept = heap.allocate_payload(Box::new(Owning(8 * COLLECTION_FLOOR)));
        let huge = heap.big(BigInt::from(1) << 100_000);
        let pair = heap.object_new(1, &[kept, huge]);
        heap.collect(|visit| visit(pair, Repr::Ref));
        let live = heap.stats().external_live_bytes;
        assert!(live >= 8 * COLLECTION_FLOOR + 100_000 / 8, "{live}");
        assert!(!heap.needs_collection(), "a large live owner must not make every safepoint collect");
        // The next collection waits for as much again as is alive.
        heap.charge_external(live / 2);
        assert!(!heap.needs_collection());
        heap.charge_external(live);
        assert!(heap.needs_collection());
        heap.collect(|visit| visit(pair, Repr::Ref));
        assert_eq!(heap.stats().external_live_bytes, live, "the estimate of the same live objects is stable");
        heap.collect(|_| {});
        assert_eq!(heap.stats().external_live_bytes, 0);
        let total = heap.stats().external_allocated_bytes;
        assert!(total >= 9 * COLLECTION_FLOOR + live / 2 + live, "{total}");
    }

    /// #332: a payload is charged once — its contents, its struct and its
    /// boxes — as `payload_bytes` estimates it alive; one that takes over a
    /// builder's contents is charged only for the struct and boxes.
    #[test]
    fn a_payload_is_charged_once() {
        let mut heap = Heap::new(layouts());
        let before = heap.stats().external_allocated_bytes;
        let owning = Owning(12_345);
        let expected = payload_bytes(&owning);
        heap.allocate_payload(Box::new(owning));
        assert_eq!(heap.stats().external_allocated_bytes - before, expected);
        let before = heap.stats().external_allocated_bytes;
        heap.allocate_payload_moved(Box::new(Owning(12_345)));
        assert_eq!(
            heap.stats().external_allocated_bytes - before,
            BOX_BYTES + std::mem::size_of::<Owning>(),
            "the moved contents are not charged again"
        );
    }

    /// Shared arrays (#246): a version reads its own slots of the buffer, the
    /// buffer lives as long as any version and traces the slots below its
    /// frontier, and writing past the frontier changes no version.
    #[test]
    fn shared_versions_keep_their_buffer_and_elements() {
        let mut heap = Heap::new(layouts());
        let texts: Vec<Word> = (0..40).map(|n| heap.allocate_text(&n.to_string())).collect();
        let buffer = heap.allocate_buffer(Repr::Ref, &[&texts[..30]], 200, 0);
        assert_eq!(heap.buffer_room(buffer), (30, 256));
        let short = heap.allocate_view(Repr::Ref, buffer, 0, 30);
        heap.buffer_extend(buffer, &texts[30..40]);
        let long = heap.allocate_view(Repr::Ref, buffer, 0, 40);
        let tail = heap.allocate_view(Repr::Ref, buffer, 35, 5);
        assert_eq!(heap.elements(short).len(), 30);
        assert_eq!(heap.text(heap.elements(long)[39]), "39");
        assert_eq!(heap.text(heap.elements(tail)[0]), "35");
        // Only the tail view is a root: the buffer and every written slot survive.
        heap.collect(|visit| visit(tail, Repr::Ref));
        assert_eq!(heap.text(heap.elements(tail)[4]), "39");
        assert_eq!(heap.buffer_room(buffer), (40, 256));
        heap.collect(|_| {});
        assert_eq!(heap.stats().large, 0, "the buffer dies with its last version");
    }

    /// Tries (#250): reads agree with the flat array across level boundaries,
    /// a replacement copies one path and leaves every older version as it
    /// was, and a version survives a collection through its nodes alone.
    #[test]
    fn tries_replace_one_path_and_keep_old_versions() {
        let mut heap = Heap::new(layouts());
        for n in [64usize, 65, 1056, 1057, 33_000] {
            let items: Vec<Word> = (0..n as i64).map(|i| heap.int(i * 3)).collect();
            let trie = heap.trie_from(Repr::Int, &items);
            assert_eq!(payload(heap.header_of(trie)) as usize, n);
            for index in [0, 1, 31, 32, 33, n / 2, n - 33, n - 32, n - 1] {
                assert_eq!(heap.trie_get(trie, index), items[index], "n = {n}, index {index}");
            }
            assert_eq!(&*heap.elements(trie), &items[..]);
            let mut versions = vec![(trie, items.clone())];
            for (step, index) in [0, n - 1, n / 2, 31, 32, n - 33].into_iter().enumerate() {
                let (last, model) = versions.last().unwrap().clone();
                let value = heap.int(-(step as i64) - 1);
                let next = heap.trie_set(last, index, value);
                let mut changed = model.clone();
                changed[index] = value;
                versions.push((next, changed));
            }
            for (version, model) in &versions {
                assert_eq!(&*heap.elements(*version), &model[..], "n = {n}");
            }
            // Only the newest version is a root: its nodes keep it whole.
            let (newest, model) = versions.last().unwrap().clone();
            heap.collect(|visit| visit(newest, Repr::Ref));
            assert_eq!(&*heap.elements(newest), &model[..]);
            assert_eq!(heap.trie_get(newest, n / 2), model[n / 2]);
            // Read as often as two passes, it becomes a flat shared view with
            // the same elements, still marked as made by a replacement.
            for pass in 0..2 {
                for (index, &item) in model.iter().enumerate() {
                    assert_eq!(heap.trie_read(newest, index), item, "pass {pass}");
                }
            }
            let header = heap.header_of(newest);
            assert!(header & SHARED != 0 && header & TRIE == 0 && header & REPLACED != 0);
            assert_eq!(&*heap.elements(newest), &model[..]);
            heap.collect(|visit| visit(newest, Repr::Ref));
            assert_eq!(&*heap.elements(newest), &model[..]);
        }
    }

    /// Buffers grow at both ends (#247): a prepend claims the free slots right
    /// before the lowest written one, a second prepend to the same version
    /// finds them taken, and the free slots in front are no pointers to the
    /// collector, whatever the representation.
    #[test]
    fn buffers_grow_at_both_ends() {
        let mut heap = Heap::new(layouts());
        for repr in [Repr::Ref, Repr::Int, Repr::Raw] {
            let items: Vec<Word> = (0..10i64)
                .map(|n| match repr {
                    Repr::Ref => heap.allocate_text(&n.to_string()),
                    Repr::Int => heap.int(n),
                    _ => n as u64 * 3,
                })
                .collect();
            let buffer = heap.allocate_buffer(repr, &[&items[5..]], 32, 20);
            let base = heap.allocate_view(repr, buffer, 20, 5);
            assert!(heap.buffer_front_free(buffer, 20, 2, repr));
            heap.buffer_prepend(buffer, 20, &items[3..5]);
            let longer = heap.allocate_view(repr, buffer, 18, 7);
            assert!(!heap.buffer_front_free(buffer, 20, 1, repr), "the same version prepended to twice copies");
            assert!(heap.buffer_front_free(buffer, 18, 3, repr));
            assert!(!heap.buffer_front_free(buffer, 18, 19, repr), "no room for 19 before slot 18");
            assert_eq!(&*heap.elements(longer), &items[3..10]);
            assert_eq!(&*heap.elements(base), &items[5..10]);
            heap.collect(|visit| {
                visit(longer, Repr::Ref);
                visit(base, Repr::Ref);
            });
            assert_eq!(&*heap.elements(longer), &items[3..10]);
            assert_eq!(&*heap.elements(base), &items[5..10]);
        }
    }

    /// Code point positions agree with a plain count from the start, in any
    /// order of access, for several strings at once and across collections
    /// (#52).
    #[test]
    fn text_views_share_a_buffer_and_keep_their_bytes() {
        let mut heap = Heap::new(layouts());
        // Room for 64 bytes, "ab" written from byte 16.
        let buffer = heap.allocate_text_buffer(&["a", "b"], 40, 16);
        assert_eq!(heap.text_buffer_room(buffer), (16, 18, 64));
        let first = heap.allocate_text_view(buffer, 16, 2, true);
        heap.text_buffer_write(buffer, "cжd", false);
        heap.text_buffer_write(buffer, "😀", true);
        let back = heap.allocate_text_view(buffer, 16, 6, false);
        let both = heap.allocate_text_view(buffer, 12, 10, false);
        assert_eq!((heap.text(first), heap.text(back), heap.text(both)), ("ab", "abcжd", "😀abcжd"));
        assert_eq!(heap.text_shared(both), (buffer, 12));
        assert_eq!(heap.char_count(both), 6);
        assert_eq!(heap.char_offset(both, 2), 5);
        // The views keep the buffer alive; their bytes stay.
        heap.collect(|visit| {
            visit(first, Repr::Ref);
            visit(both, Repr::Ref);
        });
        heap.allocate_text(&"x".repeat(200));
        assert_eq!((heap.text(first), heap.text(both)), ("ab", "😀abcжd"));
        heap.collect(|_| {});
        assert_eq!(heap.stats().live_bytes, 0, "the views and the buffer are released");
    }

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
