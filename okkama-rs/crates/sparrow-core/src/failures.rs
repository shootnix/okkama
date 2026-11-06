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
}

#[derive(Debug, Default)]
pub struct RegionValue {
    entries: Vec<Word>,
    known: HashSet<Word>,
}
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

/// Closed-world reachability of destroy-bearing objects through declared types.
fn descriptor_flags(program: &Program) -> Vec<bool> {
    let offset = program.types.len();
    let mut parents = vec![Vec::new(); offset + program.objects.len()];
    let mut reachable = vec![false; parents.len()];
    let mut pending = Vec::new();
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
            | Type::MapBuilder(..) => None,
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
    fn resource_graph(&self, roots: &[Word], via: Option<Word>) -> Vec<Word> {
        if !self.resources.enabled {
            return Vec::new();
        }
        let mut pending: Vec<(Word, Repr)> = roots.iter().map(|&w| (w, Repr::Ref)).collect();
        let mut seen = HashSet::default();
        let mut found = Vec::new();
        while let Some((word, repr)) = pending.pop() {
            let Some(object) = repr.pointer(word) else {
                continue;
            };
            if !seen.insert(object) {
                continue;
            }
            let header = self.heap.header_of(object);
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
                    let layout = &self.heap.layouts.objects[descriptor];
                    for (index, &field) in layout.iter().enumerate().rev() {
                        pending.push((self.heap.field(object, index), field));
                    }
                    found.push(object);
                }
                heap::ARRAY => {
                    let repr = Repr::from_bits((header >> 9) & 3);
                    pending.extend(self.heap.elements(object).iter().rev().map(|&e| (e, repr)));
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
        found.sort_unstable_by_key(|o| self.resources.identities[o].serial);
        found
    }

    fn region_add(&mut self, region: Word, value: Word, own: bool) {
        for object in self.resource_graph(&[value], None) {
            if own {
                self.resources.identities.get_mut(&object).expect("identity").owner = Owner::Region(region);
            }
            let entries = self.heap.payload_mut::<RegionValue>(region).expect("region");
            if entries.known.insert(object) {
                entries.entries.push(object);
            }
        }
    }

    fn region_transfer(&mut self, from: Word, to: Word, value: Word) {
        if self.heap.payload::<RegionValue>(from).expect("region").entries.is_empty() {
            return;
        }
        for object in self.resource_graph(&[value], Some(from)) {
            let identity = self.resources.identities.get_mut(&object).expect("identity");
            if identity.owner == Owner::Region(from) {
                identity.owner = Owner::Region(to);
                let entries = self.heap.payload_mut::<RegionValue>(to).expect("region");
                if entries.known.insert(object) {
                    entries.entries.push(object);
                }
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
        let kept: HashSet<Word> = self.resource_graph(keep, Some(region)).into_iter().collect();
        // The region and the primary failure stay alive while destroys run.
        self.roots.push((region, Repr::Ref));
        let mut index = self.heap.payload::<RegionValue>(region).expect("region").entries.len();
        while index > 0 {
            index -= 1;
            let object = self.heap.payload::<RegionValue>(region).expect("region").entries[index];
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
        let entries = self.heap.payload_mut::<RegionValue>(region).expect("region");
        entries.entries.clear();
        entries.known.clear();
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
        self.report(root, &messages)
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

/// Resource identity of a new object with a destroy-reaching descriptor.
#[inline(never)]
pub extern "C" fn rt_created(ctx: *mut Ctx, descriptor: u64, object: Word) {
    let (_, runtime) = unsafe { parts(ctx) };
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
        I::RegionEnter { .. } => runtime.heap.allocate_payload(Box::new(RegionValue::default())),
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
