//! Failures, resource regions and the text of a failure that reaches the top.
//!
//! Regions own destroy-bearing objects created while they were open. Closing
//! a region runs each owned object's `destroy` (newest first) as an ordinary
//! machine call; a failing destroy becomes the primary failure or a secondary
//! of it, and the remaining destroys still run. Ownership states and the
//! traversal of an adopted value follow the previous engine.
#![allow(unsafe_code)]

use crate::heap::{self, Payload};
use crate::runtime::{Ctx, FAILED, Failure, FailureKind, OK, Runtime};
use crate::word::{Repr, UNBOUND, Word};
use okkama_sparrow::{CatchKind, Instruction as I, Program, Type};
use crate::word::{FastMap as HashMap, FastSet as HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Free,
    Region(Word),
    Dead,
}

#[derive(Debug)]
struct Identity {
    serial: u64,
    owner: Owner,
}

/// Explicit ownership of destroy-bearing objects; disabled without `destroy`.
#[derive(Debug, Default)]
pub struct Resources {
    pub enabled: bool,
    /// Per descriptor: can it reach a destroy-bearing identity?
    pub flags: Vec<bool>,
    serial: u64,
    identities: HashMap<Word, Identity>,
    /// Work list and visited set of `resource_graph`, reused.
    scratch: (Vec<(Word, Repr)>, Vec<Word>),
}

/// Entries of a region beyond its inline slots.
#[derive(Debug, Default)]
pub struct RegionValue {
    entries: Vec<Word>,
    /// The entries as a set: membership checks stay constant-time.
    members: HashSet<Word>,
}

/// A region is an array `[overflow, slot × REGION_SLOTS]`: identities fill the
/// slots in order, then the overflow `RegionValue`; an empty first slot means
/// an empty region (machine code checks just that word).
pub const REGION_SLOTS: usize = 4;
impl Payload for RegionValue {
    fn trace(&self, visit: &mut dyn FnMut(Word, Repr)) {
        for &entry in &self.entries {
            visit(entry, Repr::Ref);
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Resources {
    pub fn new(program: &Program) -> Self {
        Self {
            enabled: program.objects.iter().any(|o| o.destroy.is_some()),
            flags: descriptor_flags(program),
            ..Self::default()
        }
    }
    /// Drop identities of objects the collector reclaimed.
    pub fn forget(&mut self, released: &[Word]) {
        for object in released {
            self.identities.remove(object);
        }
    }
}

/// Closed-world reachability of destroy-bearing objects through declared
/// types; a role reaches what the objects doing it reach.
fn descriptor_flags(program: &Program) -> Vec<bool> {
    let offset = program.types.len();
    let mut parents = vec![Vec::new(); offset + program.objects.len()];
    let mut reachable = vec![false; parents.len()];
    let mut pending = Vec::new();
    let mut roles: HashMap<u32, Vec<usize>> = HashMap::default();
    for (index, ty) in program.types.iter().enumerate() {
        let child = match ty {
            Type::Object(id) | Type::Successor(id) => Some(offset + id.0 as usize),
            Type::Array(t) => Some(t.0 as usize),
            Type::Map(_, v) => Some(v.0 as usize),
            Type::Int
            | Type::Bool
            | Type::Byte
            | Type::Char
            | Type::Float
            | Type::Rat
            | Type::Str
            | Type::Enum(_)
            | Type::Range
            | Type::Region
            | Type::Builder(_)
            | Type::MapBuilder(..)
            // The walk never enters a native state.
            | Type::Handle(_) => None,
            // A role value is one of the objects doing it (linked below);
            // runtime-made values of a builtin role (`Error`) are not.
            Type::Role(role) if !program.roles[role.0 as usize].builtin => {
                roles.entry(role.0).or_insert_with(Vec::new).push(index);
                None
            }
            _ => {
                pending.push(index);
                None
            }
        };
        if let Some(child) = child {
            parents[child].push(index);
        }
    }
    for (index, object) in program.objects.iter().enumerate() {
        let node = offset + index;
        if object.destroy.is_some() {
            pending.push(node);
        }
        for witness in &object.does {
            if let Some(types) = roles.get(&witness.role.0) {
                parents[node].extend(types.iter().copied());
            }
        }
        for (_, ty) in &object.fields {
            parents[ty.0 as usize].push(node);
        }
    }
    while let Some(node) = pending.pop() {
        if std::mem::replace(&mut reachable[node], true) {
            continue;
        }
        pending.extend(parents[node].iter().copied());
    }
    reachable[offset..].to_vec()
}

impl Runtime {
    /// A new object of `descriptor` gets a resource identity if it can matter.
    pub fn created(&mut self, descriptor: usize, object: Word) {
        let resources = &mut self.resources;
        if resources.enabled && resources.flags[descriptor] {
            resources.serial += 1;
            resources.identities.insert(object, Identity { serial: resources.serial, owner: Owner::Free });
            self.heap.watch(object);
        }
    }

    /// Free or `via`-owned identities reachable from `roots`, oldest first.
    fn resource_graph(&mut self, roots: &[Word], via: Option<Word>) -> Vec<Word> {
        self.resource_graph_into(roots, via, None)
    }

    /// `resource_graph`, not descending into identities already in `known`
    /// (a region): objects are immutable, so their graph is already there.
    fn resource_graph_into(&mut self, roots: &[Word], via: Option<Word>, known: Option<Word>) -> Vec<Word> {
        if !self.resources.enabled {
            return Vec::new();
        }
        // Scratch storage reused across calls (regions close on every scope
        // exit); visited objects carry a header mark until the walk ends.
        let (mut pending, mut seen) = std::mem::take(&mut self.resources.scratch);
        pending.clear();
        seen.clear();
        pending.extend(roots.iter().map(|&w| (w, Repr::Ref)));
        let mut found = Vec::new();
        while let Some((word, repr)) = pending.pop() {
            let Some(object) = repr.pointer(word) else {
                continue;
            };
            let header = self.heap.header_of(object);
            // Leaves cannot reach an identity: skip them before the visited set.
            match heap::kind(header) {
                heap::OBJECT if !self.resources.flags[heap::payload(header) as usize] => continue,
                heap::ARRAY if header & heap::PACKED != 0 || Repr::from_bits((header >> 9) & 3) != Repr::Ref => continue,
                heap::OBJECT | heap::ARRAY | heap::CLOSURE | heap::PAYLOAD => {}
                _ => continue,
            }
            if !self.heap.visit(object) {
                continue;
            }
            seen.push(object);
            match heap::kind(header) {
                heap::OBJECT => {
                    let descriptor = heap::payload(header) as usize;
                    if !self.resources.flags[descriptor] {
                        continue;
                    }
                    let Some(identity) = self.resources.identities.get(&object) else {
                        continue;
                    };
                    if identity.owner != Owner::Free && Some(identity.owner) != via.map(Owner::Region) {
                        continue;
                    }
                    if known.is_some_and(|region| self.region_contains(region, object)) {
                        continue;
                    }
                    let layout = &self.heap.layouts.objects[descriptor];
                    for (index, &field) in layout.iter().enumerate().rev() {
                        pending.push((self.heap.field(object, index), field));
                    }
                    found.push(object);
                }
                heap::ARRAY => {
                    let repr = Repr::from_bits((header >> 9) & 3);
                    let elements = self.heap.elements(object);
                    pending.extend(elements.iter().rev().map(|&e| (e, repr)));
                }
                heap::CLOSURE => {
                    let layout = &self.heap.layouts.closures[heap::payload(header) as usize];
                    for (index, &field) in layout.iter().enumerate().rev() {
                        pending.push((self.heap.field(object, index), field));
                    }
                }
                heap::PAYLOAD => {
                    if let Some(map) = self.heap.payload::<crate::ops::MapValue>(object) {
                        pending.extend(map.entries.values().rev().map(|(_, v)| (*v, map.value)));
                    } else if let Some(boxed) = self.heap.payload::<crate::ops::AnyBox>(object) {
                        pending.push((boxed.value, boxed.repr));
                    }
                }
                _ => {}
            }
        }
        for &object in &seen {
            self.heap.unvisit(object);
        }
        self.resources.scratch = (pending, seen);
        found.sort_unstable_by_key(|o| self.resources.identities[o].serial);
        found
    }

    /// Can `value` reach a free identity? False is certain (the common case:
    /// a leaf, an object without identity, or one owned elsewhere).
    fn may_reach_free(&self, value: Word) -> bool {
        if !self.resources.enabled || value == UNBOUND || value & 1 == 1 {
            return false;
        }
        let header = self.heap.header_of(value);
        match heap::kind(header) {
            heap::OBJECT => {
                self.resources.flags[heap::payload(header) as usize]
                    && self.resources.identities.get(&value).is_some_and(|i| i.owner == Owner::Free)
            }
            heap::ARRAY => header & heap::PACKED == 0 && Repr::from_bits((header >> 9) & 3) == Repr::Ref,
            heap::CLOSURE | heap::PAYLOAD => true,
            _ => false,
        }
    }

    fn region_len(&self, region: Word) -> usize {
        let inline = (1..=REGION_SLOTS).take_while(|&i| self.heap.field(region, i) != UNBOUND).count();
        if inline < REGION_SLOTS {
            return inline;
        }
        match self.heap.field(region, 0) {
            UNBOUND => inline,
            overflow => inline + self.heap.payload::<RegionValue>(overflow).expect("region").entries.len(),
        }
    }

    fn region_get(&self, region: Word, index: usize) -> Word {
        if index < REGION_SLOTS {
            return self.heap.field(region, index + 1);
        }
        let overflow = self.heap.field(region, 0);
        self.heap.payload::<RegionValue>(overflow).expect("region").entries[index - REGION_SLOTS]
    }

    fn region_contains(&self, region: Word, object: Word) -> bool {
        for slot in 1..=REGION_SLOTS {
            match self.heap.field(region, slot) {
                UNBOUND => return false,
                entry if entry == object => return true,
                _ => {}
            }
        }
        match self.heap.field(region, 0) {
            UNBOUND => false,
            overflow => self.heap.payload::<RegionValue>(overflow).expect("region").members.contains(&object),
        }
    }

    fn region_push(&mut self, region: Word, object: Word) {
        if self.region_contains(region, object) {
            return;
        }
        let length = self.region_len(region);
        if length < REGION_SLOTS {
            self.heap.set_field(region, length + 1, object);
            return;
        }
        if self.heap.field(region, 0) == UNBOUND {
            let overflow = self.heap.allocate_payload(Box::new(RegionValue::default()));
            self.heap.set_field(region, 0, overflow);
        }
        let overflow = self.heap.field(region, 0);
        let overflow = self.heap.payload_mut::<RegionValue>(overflow).expect("region");
        overflow.entries.push(object);
        overflow.members.insert(object);
    }

    fn region_clear(&mut self, region: Word) {
        for slot in 1..=REGION_SLOTS {
            self.heap.set_field(region, slot, UNBOUND);
        }
        let overflow = self.heap.field(region, 0);
        if overflow != UNBOUND {
            let overflow = self.heap.payload_mut::<RegionValue>(overflow).expect("region");
            overflow.entries.clear();
            overflow.members.clear();
        }
    }

    fn region_add(&mut self, region: Word, value: Word, own: bool) {
        if !self.may_reach_free(value) {
            return;
        }
        // Tracking (not owning) skips what the region already holds; adopting
        // must still take ownership of tracked objects.
        let known = if own { None } else { Some(region) };
        for object in self.resource_graph_into(&[value], None, known) {
            if own {
                self.resources.identities.get_mut(&object).expect("identity").owner = Owner::Region(region);
            }
            self.region_push(region, object);
        }
    }

    fn region_transfer(&mut self, from: Word, to: Word, value: Word) {
        if self.region_len(from) == 0 {
            return;
        }
        for object in self.resource_graph(&[value], Some(from)) {
            let identity = self.resources.identities.get_mut(&object).expect("identity");
            if identity.owner == Owner::Region(from) {
                identity.owner = Owner::Region(to);
                self.region_push(to, object);
            }
        }
    }

    /// Thrown payloads of a failure, its causes and secondaries.
    fn failure_payloads(&self, failure: Word) -> Vec<Word> {
        let mut roots = Vec::new();
        let mut pending = vec![failure];
        let mut seen = HashSet::default();
        while let Some(value) = pending.pop() {
            if !seen.insert(value) {
                continue;
            }
            let Some(record) = self.failure(value) else { continue };
            if let FailureKind::Thrown(payload) = record.kind {
                roots.push(payload);
            }
            pending.extend(record.cause);
            pending.extend(record.secondary.iter().copied());
        }
        roots
    }

    /// Link `other` to `fresh`: as a secondary, or at the end of its cause chain.
    pub fn failure_link(&mut self, fresh: Word, other: Word, secondary: bool) -> Word {
        if secondary {
            self.heap.payload_mut::<Failure>(fresh).expect("failure").secondary.push(other);
            return fresh;
        }
        if !matches!(self.failure(fresh).map(|f| &f.kind), Some(FailureKind::Thrown(_))) || fresh == other {
            return fresh;
        }
        let mut cursor = fresh;
        let mut seen = HashSet::default();
        loop {
            if !seen.insert(cursor) {
                return fresh;
            }
            match self.failure(cursor).and_then(|f| f.cause) {
                Some(next) if next == other => return fresh,
                Some(next) => cursor = next,
                None => break,
            }
        }
        self.heap.payload_mut::<Failure>(cursor).expect("failure").cause = Some(other);
        fresh
    }

    /// Close `region`, keeping `keep` (and its graph) alive, running destroys.
    /// Returns the primary failure (the given one, a destroy's, or none).
    fn region_close(&mut self, ctx: *mut Ctx, region: Word, keep: &[Word], mut primary: Option<Word>) -> Option<Word> {
        if self.region_len(region) == 0 {
            return primary;
        }
        let kept: HashSet<Word> =
            if keep.is_empty() { HashSet::default() } else { self.resource_graph(keep, Some(region)).into_iter().collect() };
        // The region and the primary failure stay alive while destroys run.
        self.roots.push((region, Repr::Ref));
        let mut index = self.region_len(region);
        while index > 0 {
            index -= 1;
            let object = self.region_get(region, index);
            let Some(identity) = self.resources.identities.get_mut(&object) else { continue };
            if kept.contains(&object) {
                if identity.owner == Owner::Region(region) {
                    identity.owner = Owner::Free;
                }
                continue;
            }
            if identity.owner != Owner::Free && identity.owner != Owner::Region(region) {
                continue;
            }
            identity.owner = Owner::Dead;
            let descriptor = heap::payload(self.heap.header_of(object)) as usize;
            let Some(destroy) = self.loaded.program.objects[descriptor].destroy else { continue };
            let code = match crate::jit::compile(self, destroy.0 as usize) {
                Ok(code) => code,
                Err(error) => {
                    crate::runtime::fatal(unsafe { &mut *ctx }, self, error.0);
                    break;
                }
            };
            if let Some(primary) = primary {
                self.roots.push((primary, Repr::Ref));
            }
            // SAFETY: a destroy body takes its receiver.
            unsafe { crate::jit::invoke(code, ctx, &[object]) };
            if primary.is_some() {
                self.roots.pop();
            }
            let ctx = unsafe { &mut *ctx };
            if ctx.status == FAILED {
                ctx.status = OK;
                let failure = ctx.failure;
                primary = Some(match primary {
                    Some(primary) => self.failure_link(primary, failure, true),
                    None => failure,
                });
            } else if ctx.status != OK {
                break;
            }
        }
        self.roots.pop();
        self.region_clear(region);
        primary
    }

    /// Text of a failure that reached the top, running `Error.text` for
    /// thrown values, formatted as the previous engine's report.
    pub fn describe(&mut self, ctx: *mut Ctx, root: Word) -> String {
        self.roots.push((root, Repr::Ref));
        let mut order = Vec::new();
        let mut pending = vec![root];
        let mut seen = HashSet::default();
        while let Some(next) = pending.pop() {
            if !seen.insert(next) {
                continue;
            }
            let Some(record) = self.failure(next) else { continue };
            pending.extend(record.cause);
            pending.extend(record.secondary.iter().rev().copied());
            order.push(next);
        }
        let mut messages: HashMap<Word, String> = HashMap::default();
        for &failure in &order {
            if let Some(FailureKind::Thrown(object)) = self.failure(failure).map(|f| &f.kind) {
                let object = *object;
                if let Some(function) = self.error_text_method(object) {
                    if let Ok(code) = crate::jit::compile(self, function) {
                        // SAFETY: Error.text takes its receiver and returns a string.
                        let text = unsafe { crate::jit::invoke(code, ctx, &[object]) };
                        let ctx = unsafe { &mut *ctx };
                        if ctx.status == OK {
                            messages.insert(failure, self.heap.text(text).to_owned());
                        } else {
                            ctx.status = OK;
                        }
                    }
                }
            }
        }
        self.roots.pop();
        self.uncaught = self.uncaught_of(root, &messages);
        self.report(root, &messages)
    }

    /// Category, error type and message of a reported failure.
    fn uncaught_of(&self, root: Word, messages: &HashMap<Word, String>) -> Option<crate::Uncaught> {
        let record = self.failure(root)?;
        let (category, error_type, message) = match &record.kind {
            FailureKind::Cancelled => ("runtime", None, "Operation was cancelled".to_owned()),
            FailureKind::Native(object) => {
                ("runtime", Some("NativeFailure".to_owned()), self.heap.text(self.heap.field(*object, 3)).to_owned())
            }
            FailureKind::Program(message) => ("runtime", None, message.clone()),
            FailureKind::Panic(message) => ("panic", None, message.clone()),
            FailureKind::Thrown(object) => {
                let header = self.heap.header_of(*object);
                let name = if heap::kind(header) == heap::OBJECT {
                    let full = &self.loaded.program.objects[heap::payload(header) as usize].name;
                    full.rsplit("::").next().unwrap_or(full).to_owned()
                } else {
                    "value".into()
                };
                ("uncaught-error", Some(name), messages.get(&root).cloned().unwrap_or_default())
            }
        };
        Some(crate::Uncaught { category, error_type, message })
    }

    fn error_text_method(&self, object: Word) -> Option<usize> {
        let program = &self.loaded.program;
        let header = self.heap.header_of(object);
        if heap::kind(header) != heap::OBJECT {
            return None;
        }
        let (role, declaration) = program.roles.iter().enumerate().find(|(_, r)| r.is_error())?;
        let witness = program.objects[heap::payload(header) as usize].does.iter().find(|w| w.role.0 as usize == role)?;
        let slot = declaration.methods.iter().position(|m| m.name == "text")?;
        Some(witness.slots[slot].0 as usize)
    }

    fn report(&self, root: Word, messages: &HashMap<Word, String>) -> String {
        use std::fmt::Write;
        let mut output = String::new();
        let mut pending = vec![(root, "", true)];
        let mut shown: HashMap<Word, usize> = HashMap::default();
        let mut next_index = 0;
        while let Some((failure, label, first)) = pending.pop() {
            if !label.is_empty() {
                let _ = write!(output, "\n{label}\n  ");
            }
            if let Some(index) = shown.get(&failure) {
                let _ = write!(output, "[failure #{index} already shown]");
                continue;
            }
            shown.insert(failure, next_index);
            next_index += 1;
            let Some(record) = self.failure(failure) else {
                output.push_str("<invalid failure reference>");
                continue;
            };
            let simple = first && record.cause.is_none() && record.secondary.is_empty();
            match &record.kind {
                FailureKind::Cancelled => output.push_str("Cancelled: Operation was cancelled"),
                FailureKind::Native(object) => {
                    let _ = write!(output, "NativeFailure: {}", self.heap.text(self.heap.field(*object, 3)));
                }
                FailureKind::Program(message) | FailureKind::Panic(message) if simple => output.push_str(message),
                FailureKind::Program(message) => {
                    let _ = write!(output, "RuntimeError: {message}");
                }
                FailureKind::Panic(message) => {
                    let _ = write!(output, "Panic: {message}");
                }
                FailureKind::Thrown(object) => {
                    let header = self.heap.header_of(*object);
                    let name = if heap::kind(header) == heap::OBJECT {
                        self.loaded.program.objects[heap::payload(header) as usize].name.clone()
                    } else {
                        "value".into()
                    };
                    match messages.get(&failure) {
                        Some(message) => {
                            let _ = write!(output, "{name}: {message}");
                        }
                        None => {
                            let _ = write!(output, "Uncaught {name}");
                        }
                    }
                }
            }
            if let Some(cause) = record.cause {
                pending.push((cause, "Caused by:", false));
            }
            pending.extend(record.secondary.iter().rev().map(|&s| (s, "Secondary cleanup failure:", false)));
        }
        output
    }
}

// ---- helpers called from machine code ----

unsafe fn parts<'a>(ctx: *mut Ctx) -> (&'a mut Ctx, &'a mut Runtime) {
    let ctx = unsafe { &mut *ctx };
    let runtime = unsafe { &mut *ctx.runtime };
    (ctx, runtime)
}

/// `region.adopt` (own) and `region.track` of `value`.
#[inline(never)]
pub extern "C" fn rt_region_add(ctx: *mut Ctx, region: Word, value: Word, own: u64) {
    let (_, runtime) = unsafe { parts(ctx) };
    runtime.count(|_| "region add".into());
    runtime.region_add(region, value, own != 0);
}

/// Resource identity of a new object with a destroy-reaching descriptor.
#[inline(never)]
pub extern "C" fn rt_created(ctx: *mut Ctx, descriptor: u64, object: Word) {
    let (_, runtime) = unsafe { parts(ctx) };
    runtime.count(|r| format!("resource {}", r.loaded.program.objects[descriptor as usize].name));
    runtime.created(descriptor as usize, object);
}

/// Region instructions and failure links of instruction `pc` (uses at `uses`).
#[inline(never)]
pub extern "C" fn rt_failure_op(ctx_ptr: *mut Ctx, function: u64, pc: u64, uses: *const Word) -> Word {
    let frame = crate::runtime::frame_pointer!();
    let (ctx, runtime) = unsafe { parts(ctx_ptr) };
    // SAFETY: the loaded program is never mutated while machine code runs, so
    // the instruction stays valid while the runtime is borrowed mutably.
    let instruction: &I =
        unsafe { &*(&runtime.loaded.program.functions[function as usize].code[pc as usize] as *const I) };
    runtime.count(|r| format!("region {} in {}", instruction.mnemonic(), r.loaded.program.functions[function as usize].symbol));
    // Uses in visit order; a region may keep many registers, so spill past 16.
    let mut inline = [okkama_sparrow::Register(u32::MAX); 16];
    let mut spilled = Vec::new();
    let mut count = 0;
    instruction.visit(&mut |access| {
        if let okkama_sparrow::Access::Use(register) = access {
            if count < inline.len() {
                inline[count] = register;
            } else {
                if spilled.is_empty() {
                    spilled.extend_from_slice(&inline);
                }
                spilled.push(register);
            }
            count += 1;
        }
    });
    let registers: &[okkama_sparrow::Register] = if count <= inline.len() { &inline[..count] } else { &spilled };
    let words = unsafe { std::slice::from_raw_parts(uses, count) };
    let value = |register: &okkama_sparrow::Register| words[registers.iter().position(|r| r == register).expect("used register")];
    let result = match instruction {
        I::RegionEnter { .. } => runtime.heap.allocate_array(Repr::Ref, &[UNBOUND; 1 + REGION_SLOTS]),
        I::RegionAdopt { region, value: v } | I::RegionTrack { region, value: v } => {
            runtime.region_add(value(region), value(v), matches!(instruction, I::RegionAdopt { .. }));
            UNBOUND
        }
        I::RegionTransfer { from, to, value: v } => {
            runtime.region_transfer(value(from), value(to), value(v));
            UNBOUND
        }
        I::RegionLeave { region, keep } => {
            let keep: Vec<Word> = keep.iter().map(&value).collect();
            if let Some(failure) = runtime.region_close(ctx_ptr, value(region), &keep, None) {
                let ctx = unsafe { &mut *ctx_ptr };
                ctx.failure = failure;
                ctx.status = FAILED;
            }
            UNBOUND
        }
        I::RegionUnwind { region, failure, .. } => {
            let primary = value(failure);
            let kept = runtime.failure_payloads(primary);
            runtime.region_close(ctx_ptr, value(region), &kept, Some(primary)).unwrap_or(primary)
        }
        I::FailSecondary { primary, cleanup, .. } => runtime.failure_link(value(primary), value(cleanup), true),
        I::FailCaused { new, handled, .. } => runtime.failure_link(value(new), value(handled), false),
        I::Raise { failure } => {
            ctx.failure = value(failure);
            ctx.status = FAILED;
            UNBOUND
        }
        I::Panic { message } => {
            let message = runtime.heap.text(value(message)).to_owned();
            ctx.failure = runtime.failure_new(FailureKind::Panic(message));
            ctx.status = FAILED;
            UNBOUND
        }
        I::RoleNarrow { source, role, message, .. } => {
            let object = value(source);
            let header = runtime.heap.header_of(object);
            let does = heap::kind(header) == heap::OBJECT
                && runtime.loaded.program.objects[heap::payload(header) as usize]
                    .does
                    .iter()
                    .any(|w| w.role == *role);
            if does {
                object
            } else {
                let message = if message.is_empty() { "role witness is missing".to_owned() } else { message.clone() };
                ctx.failure = runtime.failure_new(FailureKind::Program(message));
                ctx.status = FAILED;
                UNBOUND
            }
        }
        _ => {
            return crate::runtime::fatal(ctx, runtime, format!("{} is not a failure or region instruction", instruction.mnemonic()));
        }
    };
    let ctx = unsafe { &mut *ctx_ptr };
    if ctx.status == OK {
        runtime.safepoint(frame, ctx, &[(result, Repr::Ref)]);
    }
    result
}

/// `catch`: the payload (in `scratch`) and 1 when the failure matches.
#[inline(never)]
pub extern "C" fn rt_catch(ctx: *mut Ctx, function: u64, pc: u64, failure: Word) -> u64 {
    let (ctx, runtime) = unsafe { parts(ctx) };
    let I::Catch { kind, ty, .. } = &runtime.loaded.program.functions[function as usize].code[pc as usize] else {
        return 0;
    };
    if matches!(runtime.failure(failure).map(|f| &f.kind), Some(FailureKind::Cancelled)) {
        // Cancelled matches the builtin `Cancelled` object and the Error role.
        let program = &runtime.loaded.program;
        let builtin = program.objects.iter().position(|o| o.builtin && o.name == "Cancelled");
        let matches = match kind {
            CatchKind::Object => builtin == Some(*ty as usize),
            CatchKind::Role => program.roles.get(*ty as usize).is_some_and(|r| r.is_error()),
        };
        if !matches {
            return 0;
        }
        ctx.scratch = match builtin {
            Some(descriptor) => {
                let fields = vec![UNBOUND; program.objects[descriptor].fields.len()];
                runtime.heap.object_new(descriptor as u32, &fields)
            }
            None => UNBOUND,
        };
        return 1;
    }
    if let Some(FailureKind::Native(object)) = runtime.failure(failure).map(|f| &f.kind) {
        // NativeFailure matches its builtin object and the Error role.
        let object = *object;
        let program = &runtime.loaded.program;
        let matches = match kind {
            CatchKind::Object => program.objects.get(*ty as usize).is_some_and(|o| o.is_native_failure()),
            CatchKind::Role => program.roles.get(*ty as usize).is_some_and(|r| r.is_error()),
        };
        if matches {
            ctx.scratch = object;
        }
        return u64::from(matches);
    }
    let Some(FailureKind::Thrown(payload)) = runtime.failure(failure).map(|f| &f.kind) else {
        return 0;
    };
    let payload = *payload;
    let header = runtime.heap.header_of(payload);
    if heap::kind(header) != heap::OBJECT {
        return 0;
    }
    let owner = heap::payload(header);
    let matches = match kind {
        CatchKind::Object => owner == *ty,
        CatchKind::Role => runtime.loaded.program.objects[owner as usize].does.iter().any(|w| w.role.0 == *ty),
    };
    if matches {
        ctx.scratch = payload;
    }
    u64::from(matches)
}
