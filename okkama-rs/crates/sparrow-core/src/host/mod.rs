//! Native provider host. The providers of the previous engine (`sparrow-engine`
//! `native_*`) run here with their shape intact: they see a [`Heap`] with the
//! same methods and [`Slot`] values. A slot is a typed view of a core word, not
//! a copy: strings, arrays and maps are core heap objects (`bits` is the word),
//! integers that fit i64 travel unboxed (`SMALL`), handles are ids in the
//! host's arena. Words become slots at the call by the provider's ABI
//! signature, and the result slot becomes a word again.
//!
//! A handle reaches the program as a small core object naming its arena id;
//! the heap watches it, and when it dies the handle state is dropped. JSON
//! nodes are traced from the live handles after each collection. Providers
//! allocate only between safepoints, so slots stay valid during a call.
#![allow(unsafe_code)]

pub(crate) mod arena;
#[cfg(unix)]
pub(crate) mod file_lock;
mod handle;
pub(crate) mod native_binary;
pub(crate) mod native_buffer;
pub(crate) mod native_crypto;
pub(crate) mod native_data;
pub(crate) mod native_db;
pub(crate) mod native_debug;
pub(crate) mod native_directory;
pub(crate) mod native_dns;
pub(crate) mod native_encoding;
pub(crate) mod native_environment;
pub(crate) mod native_file;
pub(crate) mod native_file_state;
pub(crate) mod native_fs;
pub(crate) mod native_glob;
pub(crate) mod native_idna;
pub(crate) mod native_input;
pub(crate) mod native_io;
pub(crate) mod native_json;
pub(crate) mod native_multicast;
pub(crate) mod native_net;
pub(crate) mod native_network;
pub(crate) mod native_numbers;
pub(crate) mod native_origin;
pub(crate) mod native_postgres;
pub(crate) mod native_tls;
#[cfg(unix)]
pub(crate) mod native_process;
pub(crate) mod native_regex;
#[cfg(unix)]
pub(crate) mod native_root;
pub(crate) mod native_secret;
pub(crate) mod native_sqlite;
pub(crate) mod native_temporary;
pub(crate) mod native_text;
pub(crate) mod native_udp;
#[cfg(unix)]
pub(crate) mod native_unix;
#[cfg(unix)]
pub(crate) mod native_watch;
pub(crate) mod path_regex;
pub(crate) mod path_syntax;
#[cfg(unix)]
pub(crate) mod process_host;

pub(crate) use crate::Error;
pub(crate) use handle::Handle;
pub(crate) use native_io::{CancelHook, Output};

use crate::runtime::Runtime;
use crate::word::{Repr, Word};
use arena::Arena;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use okkama_runtime_abi::{Native, NativeFailure, Type as Abi};
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) const SMALL: u64 = 0;
pub(crate) const BIG: u64 = 1;
pub(crate) const BOOL: u64 = 2;
pub(crate) const FLOAT: u64 = 3;
pub(crate) const BYTE: u64 = 4;
pub(crate) const CHAR: u64 = 5;
pub(crate) const OBJECT: u64 = 8;
pub(crate) const STR: u64 = 10;
pub(crate) const ARRAY: u64 = 11;
pub(crate) const MAP: u64 = 14;
pub(crate) const FAILURE: u64 = 19;
pub(crate) const FUTURE: u64 = 21;
pub(crate) const NATIVE_FAILURE: u64 = 22;
pub(crate) const HANDLE: u64 = 24;
pub(crate) const JSON_NODE: u64 = 25;
/// Low byte of a tag is the kind; arrays and maps keep a shape index above it.
const KIND: u64 = 0xff;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Slot {
    pub bits: u64,
    pub tag: u64,
}
impl Slot {
    pub fn small(value: i64) -> Self {
        Self { bits: value as u64, tag: SMALL }
    }
    pub fn boolean(value: bool) -> Self {
        Self { bits: u64::from(value), tag: BOOL }
    }
    /// The kind of the slot (arrays and maps carry a shape above it).
    pub fn kind(self) -> u64 {
        self.tag & KIND
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Integer {
    Small(i64),
    Big(Rc<BigInt>),
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
}

#[derive(Debug)]
pub(crate) enum FailureKind {
    Native(Slot),
    Program(String),
    Cancelled,
}

/// A native result that may complete later (`native_io`).
#[derive(Debug)]
pub(crate) enum State {
    Ready(Result<Slot, Slot>),
    Io,
}

#[derive(Debug)]
pub(crate) struct FutureCell {
    pub state: State,
    pub io: Option<Rc<native_io::Request>>,
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Value(Slot),
    Failure(Slot),
}

#[derive(Debug)]
pub(crate) enum ProviderError {
    Engine(Error),
    Native(NativeFailure),
}
impl From<Error> for ProviderError {
    fn from(error: Error) -> Self {
        Self::Engine(error)
    }
}

/// Static shape of a value at the boundary (from ABI signatures).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Shape {
    Unknown,
    Int,
    Bool,
    Byte,
    Char,
    Float,
    Str,
    Handle,
    Array(usize),
    Map(usize, usize),
}


#[derive(Debug)]
pub(crate) struct Heap {
    rt: *mut Runtime,
    pub(crate) handles: Arena<Handle>,
    pub(crate) json_nodes: Arena<native_json::Node>,
    /// Results of async natives until their driver takes them.
    pub(crate) futures: Arena<FutureCell>,
    pub(crate) test_failures: Option<Vec<String>>,
    /// The reason of the running test's `test.skip`.
    pub(crate) test_skipped: Option<String>,
    /// Core object of each live handle, and back.
    objects: crate::word::FastMap<u64, Word>,
    ids: crate::word::FastMap<Word, u64>,
    shapes: Vec<Shape>,
    shape_index: HashMap<Shape, usize>,
    /// Shapes of each native's parameters, by native.
    signatures: Vec<Option<Box<[Shape]>>>,
    pub(crate) stdin: Option<Box<native_input::Stdin>>,
    pub(crate) arguments: Vec<Vec<u8>>,
    /// Descriptor of the builtin NativeFailure object.
    native_failure: Option<u32>,
    /// Provider state of the bridged origin natives (`native_origin`).
    pub(crate) origin: okkama_runtime_core::native::Context,
}

impl Heap {
    pub(crate) fn new(program: &okkama_sparrow::Program) -> Self {
        let mut heap = Self {
            rt: std::ptr::null_mut(),
            handles: Arena::default(),
            origin: Default::default(),
            json_nodes: Arena::default(),
            futures: Arena::default(),
            test_failures: None,
            test_skipped: None,
            stdin: None,
            arguments: Vec::new(),
            objects: Default::default(),
            ids: Default::default(),
            signatures: Vec::new(),
            shapes: Vec::new(),
            shape_index: HashMap::new(),
            native_failure: program.objects.iter().position(|o| o.is_native_failure()).map(|d| d as u32),
        };
        heap.intern(Shape::Unknown);
        heap
    }

    fn core(&self) -> &crate::heap::Heap {
        // SAFETY: `rt` is set for the duration of a call (see `call`).
        unsafe { &(*self.rt).heap }
    }
    /// The loaded program (types of `any` values).
    pub(crate) fn program(&self) -> &okkama_sparrow::Program {
        // SAFETY: as in `core`.
        unsafe { &(*self.rt).loaded.program }
    }
    fn core_mut(&mut self) -> &mut crate::heap::Heap {
        // SAFETY: as in `core`; the runtime does not hold the host during a call.
        unsafe { &mut (*self.rt).heap }
    }

    fn intern(&mut self, shape: Shape) -> usize {
        if let Some(&index) = self.shape_index.get(&shape) {
            return index;
        }
        self.shapes.push(shape.clone());
        self.shape_index.insert(shape, self.shapes.len() - 1);
        self.shapes.len() - 1
    }

    fn shape(&mut self, ty: &Abi) -> Shape {
        match ty {
            Abi::Int => Shape::Int,
            Abi::Bool => Shape::Bool,
            Abi::Byte => Shape::Byte,
            Abi::Char => Shape::Char,
            Abi::Float => Shape::Float,
            Abi::Str => Shape::Str,
            Abi::Handle(_) => Shape::Handle,
            Abi::Array(element) => {
                let element = self.shape(element);
                Shape::Array(self.intern(element))
            }
            Abi::Map(key, value) => {
                let (key, value) = (self.shape(key), self.shape(value));
                let map = Shape::Map(self.intern(key), self.intern(value));
                self.intern(map.clone());
                map
            }
            _ => Shape::Unknown,
        }
    }

    /// Argument slots of a call to `native` (parameter shapes cached per native).
    fn slots(&mut self, native: Native, arguments: &[Word], out: &mut [Slot]) {
        let index = native as usize;
        if self.signatures.len() <= index {
            self.signatures.resize(index + 1, None);
        }
        if self.signatures[index].is_none() {
            let shapes: Box<[Shape]> = native.signature().params.iter().map(|t| self.shape(t)).collect();
            self.signatures[index] = Some(shapes);
        }
        let shapes = self.signatures[index].as_ref().expect("parameter shapes");
        for ((slot, &word), shape) in out.iter_mut().zip(arguments).zip(shapes.iter()) {
            *slot = self.slot(word, shape);
        }
    }

    pub(crate) fn slot(&self, word: Word, shape: &Shape) -> Slot {
        match *shape {
            Shape::Int => match crate::word::small_value(word) {
                Some(value) => Slot::small(value),
                None => Slot { bits: word, tag: BIG },
            },
            Shape::Bool => Slot { bits: word, tag: BOOL },
            Shape::Byte => Slot { bits: word, tag: BYTE },
            Shape::Char => Slot { bits: word, tag: CHAR },
            Shape::Float => Slot { bits: word, tag: FLOAT },
            Shape::Str => Slot { bits: word, tag: STR },
            // A handle cell holds its id (see `handle_object`).
            Shape::Handle => Slot { bits: if word == crate::word::UNBOUND { u64::MAX } else { self.core().field(word, 0) }, tag: HANDLE },
            Shape::Array(element) => Slot { bits: word, tag: ARRAY | ((element as u64) << 8) },
            Shape::Map(..) => {
                let index = self.shape_index.get(shape).copied().unwrap_or(0);
                Slot { bits: word, tag: MAP | ((index as u64) << 8) }
            }
            Shape::Unknown => Slot { bits: word, tag: OBJECT },
        }
    }

    /// The core word of a slot (allocating integers and handle objects).
    fn word(&mut self, slot: Slot) -> Word {
        match slot.kind() {
            SMALL => self.core_mut().int(slot.bits as i64),
            HANDLE => self.handle_object(slot.bits),
            _ => slot.bits,
        }
    }

    fn handle_object(&mut self, id: u64) -> Word {
        if let Some(&object) = self.objects.get(&id) {
            return object;
        }
        // A plain cell `[header, id]`: no Rust allocation; watched so the
        // handle is dropped with it (the id is also kept in `ids`, because a
        // released cell's words are reused).
        let object = self.core_mut().allocate_array(Repr::Raw, &[id]);
        self.core_mut().watch(object);
        self.objects.insert(id, object);
        self.ids.insert(object, id);
        object
    }

    /// Objects the collector released: drop their handles, then the JSON
    /// nodes no live handle reaches.
    pub(crate) fn released(&mut self, released: &[Word]) {
        let mut json = false;
        for object in released {
            if let Some(id) = self.ids.remove(object) {
                self.objects.remove(&id);
                if let Ok(handle) = self.handles.get(id) {
                    json |= matches!(handle, Handle::Json(_));
                }
                self.handles.release(id);
            }
        }
        if json {
            let mut roots = Vec::new();
            for &id in self.objects.keys() {
                if let Ok(handle) = self.handles.get(id) {
                    handle.roots(&mut roots);
                }
            }
            self.json_nodes.begin_collection();
            while let Some(root) = roots.pop() {
                if root.kind() == JSON_NODE && self.json_nodes.mark(root.bits).unwrap_or(false)
                    && let Ok(node) = self.json_nodes.get(root.bits) {
                        node.roots(&mut roots);
                    }
            }
            self.json_nodes.sweep();
        }
    }

    // ---- the value API providers use ----

    pub fn text(&self, value: Slot) -> Result<&str, Error> {
        if value.kind() != STR {
            return Err(Error("expected string slot".into()));
        }
        Ok(self.core().text(value.bits))
    }
    pub fn allocate_text(&mut self, value: String) -> Result<Slot, Error> {
        Ok(Slot { bits: self.core_mut().allocate_text(&value), tag: STR })
    }
    /// The integer as an owned value: a big one's digits are copied into a
    /// new `Rc`. For a value kept past the call; readers use the views below.
    pub fn integer(&self, slot: Slot) -> Result<Integer, Error> {
        Ok(match self.integer_view(slot)? {
            crate::heap::Integer::Small(n) => Integer::Small(n),
            crate::heap::Integer::Big(n) => Integer::Big(Rc::new(n.clone())),
        })
    }
    /// The integer borrowed from the heap, with no copy of the digits
    /// (#451). The borrow of the heap keeps it from outliving an
    /// allocation, a collection or a suspension.
    pub fn integer_view(&self, slot: Slot) -> Result<crate::heap::Integer<'_>, Error> {
        match slot.kind() {
            SMALL => Ok(crate::heap::Integer::Small(slot.bits as i64)),
            BIG => Ok(self.core().integer(slot.bits)),
            _ => Err(Error("expected integer slot".into())),
        }
    }
    /// The integer as an `i64` when it fits, None when it does not.
    pub fn int64(&self, slot: Slot) -> Result<Option<i64>, Error> {
        Ok(match self.integer_view(slot)? {
            crate::heap::Integer::Small(n) => Some(n),
            crate::heap::Integer::Big(n) => n.to_i64(),
        })
    }
    /// The integer as a `BigInt`: a big one borrowed, a small one converted.
    pub fn bigint(&self, slot: Slot) -> Result<std::borrow::Cow<'_, BigInt>, Error> {
        Ok(match self.integer_view(slot)? {
            crate::heap::Integer::Small(n) => std::borrow::Cow::Owned(BigInt::from(n)),
            crate::heap::Integer::Big(n) => std::borrow::Cow::Borrowed(n),
        })
    }
    pub(crate) fn allocate(&mut self, value: BigInt) -> Result<Slot, Error> {
        if let Some(value) = value.to_i64() {
            return Ok(Slot::small(value));
        }
        Ok(Slot { bits: self.core_mut().big(value), tag: BIG })
    }
    pub fn elements(&self, value: Slot) -> Result<Vec<Slot>, Error> {
        if value.kind() != ARRAY {
            return Err(Error("expected array slot".into()));
        }
        let shape = self.shapes.get((value.tag >> 8) as usize).cloned().unwrap_or(Shape::Unknown);
        // One copy, into the slots (#446): a byte array or a trie is not
        // first widened or flattened into a vector of words.
        let core = self.core();
        let mut slots = Vec::with_capacity(core.array_length(value.bits));
        core.each_element(value.bits, &mut |word| slots.push(self.slot(word, &shape)));
        Ok(slots)
    }
    /// The number of elements of an array slot, with no copy (#446).
    pub fn array_length(&self, value: Slot) -> Result<usize, Error> {
        if value.kind() != ARRAY {
            return Err(Error("expected array slot".into()));
        }
        Ok(self.core().array_length(value.bits))
    }
    /// The bytes of a `@byte` slot, borrowed from the core heap.
    pub fn byte_slice(&self, value: Slot) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
        if value.kind() != ARRAY {
            return Err(Error("expected byte array".into()));
        }
        Ok(self.core().byte_view(value.bits))
    }
    pub fn allocate_bytes(&mut self, bytes: &[u8]) -> Result<Slot, Error> {
        let index = self.intern(Shape::Byte) as u64;
        Ok(Slot { bits: self.core_mut().allocate_bytes(bytes), tag: ARRAY | (index << 8) })
    }
    pub fn allocate_array(&mut self, elements: Vec<Slot>) -> Result<Slot, Error> {
        if elements.first().is_some_and(|s| s.kind() == BYTE) {
            let bytes: Vec<u8> = elements.iter().map(|s| s.bits as u8).collect();
            return self.allocate_bytes(&bytes);
        }
        let (repr, shape) = match elements.first().map(|s| s.kind()) {
            Some(SMALL | BIG) => (Repr::Int, Shape::Int),
            Some(BOOL) => (Repr::Raw, Shape::Bool),
            Some(BYTE) => (Repr::Raw, Shape::Byte),
            Some(CHAR) => (Repr::Raw, Shape::Char),
            Some(FLOAT) => (Repr::Raw, Shape::Float),
            Some(STR) => (Repr::Ref, Shape::Str),
            Some(HANDLE) => (Repr::Ref, Shape::Handle),
            Some(ARRAY) => {
                let element = (elements[0].tag >> 8) as usize;
                (Repr::Ref, Shape::Array(element))
            }
            Some(_) => (Repr::Ref, Shape::Unknown),
            None => (Repr::Raw, Shape::Unknown),
        };
        let words: Vec<Word> = elements.into_iter().map(|s| self.word(s)).collect();
        let index = self.intern(shape) as u64;
        Ok(Slot { bits: self.core_mut().allocate_array(repr, &words), tag: ARRAY | (index << 8) })
    }
    pub fn map_new(&mut self, entries: impl Iterator<Item = (Slot, Slot)>) -> Result<Slot, Error> {
        use crate::ops::{Key, MapValue};
        let entries: Vec<(Slot, Slot)> = entries.collect();
        let repr = |slot: Option<&Slot>| match slot.map(|s| s.kind()) {
            Some(SMALL | BIG) => Repr::Int,
            Some(BOOL | BYTE | CHAR | FLOAT) | None => Repr::Raw,
            Some(_) => Repr::Ref,
        };
        let mut map = MapValue {
            key: repr(entries.first().map(|e| &e.0)),
            value: repr(entries.first().map(|e| &e.1)),
            entries: Default::default(),
            owned: Default::default(),
        };
        for (key, value) in entries {
            let k = match key.kind() {
                SMALL => Key::Small(key.bits as i64),
                BIG => Key::int(self.core().integer(key.bits)),
                STR => Key::text(self.text(key)?.into()),
                FLOAT => Key::Float(f64::from_bits(key.bits)),
                _ => Key::Raw(key.bits),
            };
            let (key, value) = (self.word(key), self.word(value));
            map.entries.insert(k, (key, value));
        }
        Ok(Slot { bits: self.core_mut().allocate_payload(Box::new(map)), tag: MAP })
    }

    /// Entries of a map slot in key order.
    pub fn map_entries(&self, map: Slot) -> Result<Vec<(Slot, Slot)>, Error> {
        if map.kind() != MAP {
            return Err(Error("expected map slot".into()));
        }
        let (key, value) = match self.shapes.get((map.tag >> 8) as usize) {
            Some(&Shape::Map(key, value)) => (self.shapes[key].clone(), self.shapes[value].clone()),
            _ => (Shape::Unknown, Shape::Unknown),
        };
        let entries = self.core().payload::<crate::ops::MapValue>(map.bits).ok_or_else(|| Error("expected map".into()))?;
        Ok(entries.entries.values().map(|&(k, v)| (self.slot(k, &key), self.slot(v, &value))).collect())
    }
    pub fn native_failure_new(&mut self, failure: NativeFailure) -> Result<Slot, Error> {
        let descriptor = self.native_failure.ok_or_else(|| Error("program has no NativeFailure".into()))?;
        let fields = [
            self.core_mut().allocate_text(&failure.domain),
            self.core_mut().allocate_text(&failure.code),
            self.core_mut().allocate_text(&failure.operation),
            self.core_mut().allocate_text(&failure.message),
        ];
        Ok(Slot { bits: self.core_mut().object_new(descriptor, &fields), tag: NATIVE_FAILURE })
    }
    pub fn failure_new(&mut self, kind: FailureKind) -> Result<Slot, Error> {
        let kind = match kind {
            FailureKind::Native(payload) => crate::runtime::FailureKind::Native(payload.bits),
            FailureKind::Program(message) => crate::runtime::FailureKind::Program(message),
            FailureKind::Cancelled => crate::runtime::FailureKind::Cancelled,
        };
        // SAFETY: as in `core`.
        let runtime = unsafe { &mut *self.rt };
        Ok(Slot { bits: runtime.failure_new(kind), tag: FAILURE })
    }
    /// Attach `secondary` to `primary` as a secondary failure.
    pub fn failure_link(&mut self, primary: Slot, secondary: Slot) {
        // SAFETY: as in `core`.
        let runtime = unsafe { &mut *self.rt };
        runtime.failure_link(primary.bits, secondary.bits, true);
    }
    pub fn future_with_state(&mut self, state: State) -> Result<Slot, Error> {
        Ok(Slot { bits: self.futures.allocate(FutureCell { state, io: None })?, tag: FUTURE })
    }
    pub fn future(&self, value: Slot) -> Result<&FutureCell, Error> {
        if value.kind() != FUTURE {
            return Err(Error("expected native future".into()));
        }
        self.futures.get(value.bits)
    }
}

pub(crate) fn io_code(kind: std::io::ErrorKind) -> &'static str {
    use std::io::ErrorKind::*;
    match kind {
        NotFound => "not_found",
        PermissionDenied => "permission_denied",
        ConnectionRefused => "connection_refused",
        ConnectionReset | ConnectionAborted => "connection_reset",
        TimedOut => "timed_out",
        AddrInUse => "address_in_use",
        AddrNotAvailable => "address_not_available",
        NotConnected => "not_connected",
        BrokenPipe => "broken_pipe",
        WouldBlock => "would_block",
        Interrupted => "interrupted",
        InvalidInput => "invalid_input",
        Unsupported => "unsupported",
        OutOfMemory => "resource_exhausted",
        AlreadyExists => "already_exists",
        _ => "unexpected",
    }
}

pub(crate) type HandleBuild = crate::word::FastBuild;

/// Natives this host provides.
pub fn supported(native: Native) -> bool {
    provider(native).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    #[cfg(unix)]
    Unix,
    #[cfg(unix)]
    Root,
    #[cfg(unix)]
    Process,
    Input,
    Fs,
    Environment,
    Multicast,
    Udp,
    Network,
    Dns,
    Sqlite,
    Db,
    Json,
    Regex,
    Idna,
    Net,
    Binary,
    Secret,
    Encoding,
    Numbers,
    Buffer,
    Crypto,
    Text,
    Core,
    Origin,
    Postgres,
    Tls,
}

fn provider(native: Native) -> Option<Provider> {
    static TABLE: std::sync::OnceLock<Vec<Option<Provider>>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let size = Native::ALL.iter().map(|&n| n as usize + 1).max().unwrap_or(0);
        let mut table = vec![None; size];
        for &n in Native::ALL {
            table[n as usize] = classify(n);
        }
        table
    });
    table[native as usize]
}

fn classify(native: Native) -> Option<Provider> {
    #[cfg(unix)]
    if native_unix::handles(native) {
        return Some(Provider::Unix);
    }
    #[cfg(unix)]
    if native_root::handles(native) {
        return Some(Provider::Root);
    }
    #[cfg(unix)]
    if native_process::handles(native) {
        return Some(Provider::Process);
    }
    Some(if native_input::handles(native) {
        Provider::Input
    } else if native_fs::handles(native) {
        Provider::Fs
    } else if native_environment::handles(native) {
        Provider::Environment
    } else if native_multicast::handles(native) {
        Provider::Multicast
    } else if native_udp::handles(native) {
        Provider::Udp
    } else if native_network::handles(native) {
        Provider::Network
    } else if native_dns::handles(native) {
        Provider::Dns
    } else if native_sqlite::handles(native) {
        Provider::Sqlite
    } else if native_db::handles(native) {
        Provider::Db
    } else if native_json::handles(native) {
        Provider::Json
    } else if native_regex::handles(native) {
        Provider::Regex
    } else if native_idna::handles(native) {
        Provider::Idna
    } else if native_net::handles(native) {
        Provider::Net
    } else if native_binary::handles(native) {
        Provider::Binary
    } else if native_secret::handles(native) {
        Provider::Secret
    } else if native_encoding::handles(native) {
        Provider::Encoding
    } else if native_numbers::handles(native) {
        Provider::Numbers
    } else if native_buffer::handles(native) {
        Provider::Buffer
    } else if native_crypto::handles(native) {
        Provider::Crypto
    } else if native_text::handles(native) {
        Provider::Text
    } else if matches!(
        native,
        Native::SystemSystemPlatform
            | Native::SystemSystemArchitecture
            | Native::SystemSystemByteOrder
            | Native::TimeWallClock
            | Native::TimeMonotonicClock
            | Native::TimeClockDomain
            | Native::TestingTestStateOpen
            | Native::TestingTestStateNext
            | Native::TestingTestStateSize
            | Native::TestingTestStateClose
            | Native::TestingTestContextActive
    ) {
        Provider::Core
    } else if native_postgres::handles(native) {
        Provider::Postgres
    } else if native_tls::handles(native) {
        Provider::Tls
    } else if native_origin::handles(native) {
        Provider::Origin
    } else {
        return None;
    })
}

impl Heap {
    fn native_invoke(&mut self, native: Native, args: &[Slot]) -> Result<Outcome, Error> {
        match provider(native) {
            #[cfg(unix)]
            Some(Provider::Unix) => return self.native_unix(native, args),
            #[cfg(unix)]
            Some(Provider::Root) => return self.native_root(native, args),
            #[cfg(unix)]
            Some(Provider::Process) => return self.native_process(native, args),
            Some(Provider::Input) => return self.native_input(native, args),
            Some(Provider::Fs) => return self.native_fs(native, args),
            Some(Provider::Environment) => return self.native_environment(native, args),
            Some(Provider::Multicast) => return self.native_multicast(native, args),
            Some(Provider::Udp) => return self.native_udp(native, args),
            Some(Provider::Network) => return self.native_network(native, args),
            Some(Provider::Dns) => return self.native_dns(native, args),
            Some(Provider::Sqlite) => return self.native_sqlite(native, args),
            Some(Provider::Db) => return self.native_db(native, args),
            Some(Provider::Json) => return self.native_json(native, args),
            Some(Provider::Regex) => return self.native_regex(native, args),
            Some(Provider::Idna) => return self.native_idna(native, args),
            Some(Provider::Net) => return self.native_net(native, args),
            Some(Provider::Binary) => return self.native_binary(native, args),
            Some(Provider::Secret) => return self.native_secret(native, args),
            Some(Provider::Encoding) => return self.native_encoding(native, args),
            Some(Provider::Numbers) => return self.native_numbers(native, args),
            Some(Provider::Buffer) => return self.native_buffer(native, args),
            Some(Provider::Crypto) => return self.native_crypto(native, args),
            Some(Provider::Text) => return self.native_text(native, args),
            Some(Provider::Origin) => return self.native_origin(native, args),
            Some(Provider::Postgres) => return self.native_postgres(native, args),
            Some(Provider::Tls) => return self.native_tls(native, args),
            Some(Provider::Core) => {}
            None => return Err(Error("unsupported native invocation".into())),
        }
        let text = |heap: &mut Heap, text: &str| heap.allocate_text(text.into()).map(Outcome::Value);
        match native {
            Native::TestingTestStateOpen
            | Native::TestingTestStateNext
            | Native::TestingTestStateSize
            | Native::TestingTestStateClose => self.handle_test(native, args),
            Native::TestingTestContextActive => Ok(Outcome::Value(Slot::boolean(self.test_failures.is_some()))),
            Native::TimeWallClock => {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                self.allocate(BigInt::from(nanos)).map(Outcome::Value)
            }
            Native::TimeMonotonicClock => {
                static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
                let nanos = ORIGIN.get_or_init(std::time::Instant::now).elapsed().as_nanos();
                self.allocate(BigInt::from(nanos)).map(Outcome::Value)
            }
            Native::TimeClockDomain => {
                if args[0].bits == 0 {
                    text(self, &format!("system:{}:1", std::process::id()))
                } else if self.test_failures.is_some() {
                    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                    let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    text(self, &format!("manual:{}:{serial}", std::process::id()))
                } else {
                    let payload = self.native_failure_new(NativeFailure {
                        domain: "core.time".into(),
                        code: "permission_denied".into(),
                        operation: native.symbol().into(),
                        message: "Manual clock domains require a running test case".into(),
                    })?;
                    self.failure_new(FailureKind::Native(payload)).map(Outcome::Failure)
                }
            }
            Native::SystemSystemPlatform => text(
                self,
                match std::env::consts::OS {
                    "macos" | "ios" => "macos",
                    "linux" | "android" => "linux",
                    "windows" => "windows",
                    "freebsd" => "freebsd",
                    "openbsd" => "openbsd",
                    "netbsd" => "netbsd",
                    "dragonfly" => "dragonfly",
                    "solaris" | "illumos" => "solaris",
                    "aix" => "aix",
                    _ => "unknown",
                },
            ),
            Native::SystemSystemArchitecture => text(
                self,
                match std::env::consts::ARCH {
                    "x86_64" => "x86_64",
                    "x86" => "x86",
                    "aarch64" => "arm64",
                    "arm" => "arm",
                    "powerpc64" => "ppc64",
                    "powerpc" => "ppc",
                    "riscv64" => "riscv64",
                    "s390x" => "s390x",
                    _ => "unknown",
                },
            ),
            Native::SystemSystemByteOrder => text(self, if cfg!(target_endian = "little") { "little" } else { "big" }),
            _ => Err(Error("unsupported native invocation".into())),
        }
    }
}

/// What a native call produced, in core terms.
#[derive(Debug)]
pub enum Result2 {
    Value(Word, Repr),
    /// A failure word to raise.
    Raise(Word),
    /// A provider refusal (an engine error): a program failure.
    Refused(String),
}

/// Call `native` with argument words through the host of `runtime`.
pub fn call(runtime: &mut Runtime, native: Native, arguments: &[Word]) -> Result2 {
    with_host(runtime, needs_tokio(native), |host| call_in(host, native, arguments))
}

/// Providers whose synchronous calls create Tokio resources (sockets, processes).
fn needs_tokio(native: Native) -> bool {
    !matches!(
        provider(native),
        Some(
            Provider::Json
                | Provider::Regex
                | Provider::Idna
                | Provider::Net
                | Provider::Binary
                | Provider::Secret
                | Provider::Encoding
                | Provider::Numbers
                | Provider::Buffer
                | Provider::Crypto
                | Provider::Text
                | Provider::Db
                | Provider::Sqlite
                | Provider::Core
        )
    )
}

fn call_in(host: &mut Heap, native: Native, arguments: &[Word]) -> Result2 {
    let signature = native.signature();
    let mut buffer = [Slot::default(); 8];
    let mut heap_slots = Vec::new();
    let slots: &mut [Slot] = if arguments.len() <= buffer.len() {
        &mut buffer[..arguments.len()]
    } else {
        heap_slots.resize(arguments.len(), Slot::default());
        &mut heap_slots
    };
    host.slots(native, arguments, slots);
    match host.native_invoke(native, slots) {
        Ok(Outcome::Value(slot)) => {
            let repr = match &signature.result {
                Abi::Int => Repr::Int,
                Abi::Bool | Abi::Byte | Abi::Char | Abi::Float | Abi::Void => Repr::Raw,
                _ => Repr::Ref,
            };
            let word = if matches!(signature.result, Abi::Void) { crate::word::UNBOUND } else { host.word(slot) };
            Result2::Value(word, repr)
        }
        Ok(Outcome::Failure(failure)) => Result2::Raise(failure.bits),
        Err(Error(message)) => Result2::Refused(message),
    }
}

/// Outcome of starting or polling an async native.
#[derive(Debug)]
pub enum Start {
    Done(Word),
    Raise(Word),
    Refused(String),
    /// A pending request (its id in the host).
    Pending(u64),
}

fn with_host<T>(runtime: &mut Runtime, tokio: bool, body: impl FnOnce(&mut Heap) -> T) -> T {
    let mut host = runtime.host.take().expect("native host");
    host.rt = runtime;
    // Tokio resources created or polled by providers need the runtime context.
    let handle = tokio.then(|| runtime.io.tokio().handle().clone());
    let _context = handle.as_ref().map(|h| h.enter());
    let _local = tokio.then(|| runtime.io.local().enter());
    let result = body(&mut host);
    host.rt = std::ptr::null_mut();
    runtime.host = Some(host);
    result
}

impl Heap {
    fn settled(&mut self, native: Native, id: u64) -> Start {
        // Taking the state leaves `Io`, which is what a pending request keeps.
        let state = match self.futures.get_mut(id) {
            Ok(cell) => std::mem::replace(&mut cell.state, State::Io),
            Err(Error(message)) => return Start::Refused(message),
        };
        match state {
            State::Io => Start::Pending(id),
            State::Ready(result) => {
                self.futures.release(id);
                match result {
                    Ok(value) if matches!(native.signature().result, Abi::Void) => {
                        let _ = value;
                        Start::Done(crate::word::UNBOUND)
                    }
                    Ok(value) => Start::Done(self.word(value)),
                    Err(failure) => Start::Raise(failure.bits),
                }
            }
        }
    }
}

/// Start async `native`: its result now, or a pending request.
pub fn start(runtime: &mut Runtime, native: Native, arguments: &[Word]) -> Start {
    with_host(runtime, true, |host| {
        let mut slots = vec![Slot::default(); arguments.len()];
        host.slots(native, arguments, &mut slots);
        match host.native_invoke(native, &slots) {
            Ok(Outcome::Value(future)) if future.kind() == FUTURE => host.settled(native, future.bits),
            Ok(Outcome::Value(value)) => Start::Done(host.word(value)),
            Ok(Outcome::Failure(failure)) => Start::Raise(failure.bits),
            Err(Error(message)) => Start::Refused(message),
        }
    })
}

/// Poll a pending request with `waker`; None while it is pending.
pub fn poll(runtime: &mut Runtime, native: Native, id: u64, waker: &std::task::Waker) -> Option<Start> {
    with_host(runtime, true, |host| {
        let request = match host.futures.get(id) {
            Ok(cell) => cell.io.clone(),
            Err(Error(message)) => return Some(Start::Refused(message)),
        };
        if let Some(request) = request {
            if request.poll(&mut std::task::Context::from_waker(waker)).is_pending() {
                return None;
            }
            if let Err(Error(message)) = host.native_io_settle(Slot { bits: id, tag: FUTURE }) {
                return Some(Start::Refused(message));
            }
        }
        Some(host.settled(native, id))
    })
}

/// Cancel a pending request; true while its cleanup still runs (keep polling).
pub fn cancel(runtime: &mut Runtime, id: u64) -> bool {
    with_host(runtime, true, |host| {
        let Some(request) = host.futures.get(id).ok().and_then(|cell| cell.io.clone()) else {
            return false;
        };
        let cleanup = request.request_cancel();
        if !cleanup {
            host.futures.release(id);
        }
        cleanup
    })
}
