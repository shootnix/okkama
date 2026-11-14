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
    /// Escaping a shared cache preserves a scope owner, never fresh call
    /// admission. The next track installs that owner before a callee can adopt.
    cache_rehomed: bool,
    /// Additional group packets retaining this Future. Its lexical owner may
    /// expire first; the last live owner drains the cache and retained inputs.
    groups: Vec<Word>,
}

/// Explicit ownership of destroy-bearing objects; disabled without `destroy`.
#[derive(Debug, Default)]
pub struct Resources {
    pub enabled: bool,
    /// Per descriptor: can it reach a destroy-bearing identity?
    pub flags: Vec<bool>,
    serial: u64,
    identities: HashMap<Word, Identity>,
    /// Work list, visited set and open containers of `resource_graph`, reused.
    scratch: (Vec<(Word, Repr)>, Vec<Word>, Vec<Walked>),
    /// Containers whose walk found no free identity (#9), with `serial` at
    /// that moment: every identity they reach is at most that old. Freeing
    /// an identity no newer than `summarized` drops them all.
    summaries: HashMap<Word, u64>,
    summarized: u64,
    /// `SPARROW_TRACE_DESTROY=FILE`: one line per destroyed identity, type and
    /// creation serial, the oracle for static ownership (#555).
    trace: Option<std::fs::File>,
}

/// A container whose elements `resource_graph` is walking: its subtree is
/// done when the work list is back to `base`.
#[derive(Debug, Clone, Copy)]
struct Walked {
    container: Word,
    base: usize,
    steps: usize,
    events: usize,
}

/// Walk steps under one container worth a summary: smaller ones are cheaper
/// to walk again than to remember.
const SUMMARY_STEPS: usize = 64;

/// Entries of a region beyond its inline slots.
#[derive(Debug, Default)]
pub struct RegionValue {
    entries: Vec<Word>,
    /// The entries as a set: membership checks stay constant-time.
    members: HashSet<Word>,
    /// The oldest serial among `entries` (`None`: no entries).
    oldest: Option<u64>,
    /// A `region.settle` that waits (#154): the async destroy it awaits and
    /// the failure gathered so far.
    settling: Option<Word>,
    settled: Option<Word>,
}

/// Where a region close stopped (#154).
pub(crate) enum Closed {
    /// Every identity is destroyed; the primary failure, if any.
    Done(Option<Word>),
    /// `object`, already dead, has an async destroy to await.
    Async(Word, Option<Word>),
}

/// A region is an array `[overflow, slot × REGION_SLOTS]`: identities fill the
/// slots in order, then the overflow `RegionValue`; an empty first slot means
/// an empty region (machine code checks just that word).
pub const REGION_SLOTS: usize = 4;
impl Payload for RegionValue {
    fn trace(&self, visit: &mut dyn FnMut(Word, Repr)) {
        for &entry in self.entries.iter().chain(&self.settling).chain(&self.settled) {
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
    /// Test runtimes start fresh; mark even tests with no destroyed objects.
    pub(crate) fn trace_test(name: &str) {
        use std::io::Write;
        if let Some(mut file) = std::env::var_os("SPARROW_TRACE_DESTROY")
            .and_then(|path| std::fs::OpenOptions::new().create(true).append(true).open(path).ok())
        {
            let _ = writeln!(file, "# test {name}");
        }
    }

    pub fn new(program: &Program) -> Self {
        Self {
            enabled: program.objects.iter().any(|o| o.destroy.is_some()),
            flags: descriptor_flags(program),
            trace: std::env::var_os("SPARROW_TRACE_DESTROY")
                .and_then(|path| std::fs::OpenOptions::new().create(true).append(true).open(path).ok()),
            ..Self::default()
        }
    }
    /// Drop identities and summaries of objects the collector reclaimed.
    pub fn forget(&mut self, released: &[Word]) {
        for object in released {
            self.identities.remove(object);
            self.summaries.remove(object);
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
    for function in &program.functions {
        if let Some(intrinsic) = &function.intrinsic
            && matches!(intrinsic.name.as_str(), "okkama.scope.FutureGroup.new" | "okkama.scope.FutureScope.outcome")
        {
            pending.push(offset + intrinsic.objects[0].0 as usize);
        }
    }
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
                roles.entry(role.0).or_default().push(index);
                None
            }
            // A variant value is an object of one of its cases (#290).
            Type::Variant(variant) => {
                for (_, case) in &program.variants[variant.0 as usize].cases {
                    parents[offset + case.0 as usize].push(index);
                }
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
            resources.identities.insert(object, Identity { serial: resources.serial, owner: Owner::Free, cache_rehomed: false, groups: Vec::new() });
            self.heap.watch(object);
        }
    }

    /// A Future must be tracked even before its result exists.
    pub(crate) fn future_created(&mut self, frame: Word) {
        if self.resources.enabled {
            self.resources.serial += 1;
            self.resources.identities.insert(frame, Identity { serial: self.resources.serial, owner: Owner::Free, cache_rehomed: false, groups: Vec::new() });
            self.heap.watch(frame);
        }
    }

    /// Publication owns the outcome until cache cleanup or a proven unique take.
    pub(crate) fn own_outcome(&mut self, frame: Word, success: bool, value: Word) {
        if !self.resources.enabled {
            return;
        }
        let roots = if success {
            let repr = self.heap.layouts.frames[self.function_of(frame)][1];
            repr.pointer(value).into_iter().collect()
        } else {
            self.failure_payloads(value)
        };
        if roots.is_empty() {
            return;
        }
        let owned = self.heap.allocate_array(Repr::Ref, &[UNBOUND; 1 + REGION_SLOTS]);
        crate::tasks::set(frame, crate::tasks::OWNED, owned);
        for value in roots {
            self.region_add(owned, value, true);
        }
    }

    /// Group registration admits fresh Futures without taking external owners.
    pub(crate) fn group_own(&mut self, group: Word, future: Word) {
        if !self.resources.enabled { return; }
        let owned = match self.group_mut(group).expect("group").resources {
            Some(owned) => owned,
            None => {
                let owned = self.heap.allocate_array(Repr::Ref, &[UNBOUND; 1 + REGION_SLOTS]);
                self.group_mut(group).expect("group").resources = Some(owned);
                owned
            }
        };
        let identity = self.resources.identities.get_mut(&future).expect("Future identity");
        if !identity.groups.contains(&owned) { identity.groups.push(owned); }
        self.region_push(owned, future);
        // A fresh temporary has no lexical owner yet. Bound Futures keep
        // theirs, and registration adds a lifetime rather than stealing it.
        self.region_add(owned, future, true);
    }

    fn queue_private(&self, object: Word, groups: &mut Vec<Word>, futures: &mut Vec<Word>) {
        if heap::kind(self.heap.header_of(object)) == heap::FRAME {
            futures.push(object);
        } else if matches!(self.states.get(&object), Some(crate::primitives::State::Group(_))) {
            groups.push(object);
        }
    }

    fn private_region(&self, object: Word) -> Option<Word> {
        if heap::kind(self.heap.header_of(object)) == heap::FRAME {
            let owned = crate::tasks::get(object, crate::tasks::OWNED);
            (owned != UNBOUND).then_some(owned)
        } else if let Some(crate::primitives::State::Group(group)) = self.states.get(&object) {
            group.resources
        } else { None }
    }

    /// Release only this cache's actual package. External borrows retain their
    /// owners. No allocation, safepoint or user code occurs before region.track
    /// admits the successfully delivered result in the caller.
    pub(crate) fn take_outcome(&mut self, frame: Word) {
        let owned = crate::tasks::get(frame, crate::tasks::OWNED);
        if owned == UNBOUND {
            return;
        }
        for index in 0..self.region_len(owned) {
            let object = self.region_get(owned, index);
            let identity = self.resources.identities.get_mut(&object).expect("identity");
            if identity.owner == Owner::Region(owned) {
                identity.owner = Owner::Free;
                let serial = identity.serial;
                self.freed(serial);
            }
        }
        self.region_clear(owned);
        crate::tasks::set(frame, crate::tasks::OWNED, UNBOUND);
    }

    /// Free or `via`-owned identities reachable from `roots`, oldest first.
    fn resource_graph(&mut self, roots: &[Word], via: Option<Word>) -> Vec<Word> {
        self.resource_graph_into(roots, via, None)
    }

    /// `resource_graph`, not descending into identities already in `known`
    /// (a region): objects are immutable, so their graph is already there.
    ///
    /// Containers are immutable too, so what a container reaches is at most
    /// as old as the container. One whose walk meets no free identity (only
    /// identities owned elsewhere, dead or plain) is summarized (#9): later
    /// walks skip it until an identity at least as old becomes free again,
    /// and a `via` walk skips it while every entry of `via` is newer. A big
    /// structure held by an open region then costs one walk, not one per
    /// value that points into it.
    fn resource_graph_into(&mut self, roots: &[Word], via: Option<Word>, known: Option<Word>) -> Vec<Word> {
        if !self.resources.enabled {
            return Vec::new();
        }
        // Scratch storage reused across calls (regions close on every scope
        // exit); visited objects carry a header mark until the walk ends.
        let (mut pending, mut seen, mut open) = std::mem::take(&mut self.resources.scratch);
        pending.clear();
        seen.clear();
        open.clear();
        pending.extend(roots.iter().map(|&w| (w, Repr::Ref)));
        let mut found = Vec::new();
        // `events` counts what keeps a container from a summary: a free or
        // `via` identity, and anything met twice (its first meeting may have
        // been one).
        let (mut steps, mut events) = (0, 0);
        let mut oldest = None;
        loop {
            while let Some(&walked) = open.last()
                && pending.len() <= walked.base
            {
                open.pop();
                if walked.events == events && steps - walked.steps >= SUMMARY_STEPS {
                    self.summarize(walked.container);
                }
            }
            let Some((word, repr)) = pending.pop() else {
                break;
            };
            steps += 1;
            let Some(object) = repr.pointer(word) else {
                continue;
            };
            let header = self.heap.header_of(object);
            // Leaves cannot reach an identity: skip them before the visited set.
            match heap::kind(header) {
                heap::OBJECT if !self.resources.flags[heap::payload(header) as usize] => continue,
                heap::ARRAY if header & heap::PACKED != 0 || Repr::from_bits((header >> 9) & 3) != Repr::Ref => continue,
                heap::OBJECT => {
                    let Some(identity) = self.resources.identities.get(&object) else {
                        continue;
                    };
                    if identity.owner != Owner::Free && Some(identity.owner) != via.map(Owner::Region) {
                        continue;
                    }
                    events += 1;
                    if known.is_some_and(|region| self.region_contains(region, object)) || !self.heap.visit(object) {
                        continue;
                    }
                    seen.push(object);
                    let layout = &self.heap.layouts().objects[heap::payload(header) as usize];
                    for (index, &field) in layout.iter().enumerate().rev() {
                        pending.push((self.heap.field(object, index), field));
                    }
                    found.push(object);
                    continue;
                }
                heap::ARRAY | heap::CLOSURE | heap::PAYLOAD | heap::FRAME => {}
                _ => continue,
            }
            if header & heap::SUMMARY != 0 && self.summary_holds(object, via, &mut oldest) {
                continue;
            }
            if !self.heap.visit(object) {
                events += 1;
                continue;
            }
            seen.push(object);
            let base = pending.len();
            match heap::kind(header) {
                heap::FRAME => {
                    // The outcome has a private owner. Only retained inputs
                    // follow the Future through lexical ownership transfers.
                    events += 1;
                    let Some(identity) = self.resources.identities.get(&object) else { continue };
                    if identity.owner == Owner::Free || Some(identity.owner) == via.map(Owner::Region) {
                        found.push(object);
                    }
                    // Retained inputs keep the #578 transfer rules even when
                    // the Future itself is already owned through another edge.
                    let function = heap::payload(header) as usize;
                    let source = &self.loaded.program.functions[function];
                    let layout = &self.heap.layouts().frames[function];
                    let retained = layout.len() - source.params.len();
                    for index in (retained..layout.len()).rev() {
                        pending.push((self.heap.field(object, index), layout[index]));
                    }
                    continue;
                }
                heap::ARRAY => {
                    let repr = Repr::from_bits((header >> 9) & 3);
                    let elements = self.heap.elements(object);
                    pending.extend(elements.iter().rev().map(|&e| (e, repr)));
                }
                heap::CLOSURE => {
                    let layout = &self.heap.layouts().closures[heap::payload(header) as usize];
                    for (index, &field) in layout.iter().enumerate().rev() {
                        pending.push((self.heap.field(object, index), field));
                    }
                }
                _ => {
                    if let Some(map) = self.heap.payload::<crate::ops::MapValue>(object) {
                        let values: Vec<Word> = map.entries.values().map(|(_, v)| *v).collect();
                        pending.extend(values.into_iter().rev().map(|v| (v, map.value)));
                    } else if let Some(boxed) = self.heap.payload::<crate::ops::AnyBox>(object) {
                        pending.push((boxed.value, boxed.repr));
                    } else {
                        // Other payloads (region entries, waiters) change in
                        // place: never summarized.
                        continue;
                    }
                }
            }
            open.push(Walked { container: object, base, steps, events });
        }
        for &object in &seen {
            self.heap.unvisit(object);
        }
        self.resources.scratch = (pending, seen, open);
        found.sort_unstable_by_key(|o| self.resources.identities[o].serial);
        found
    }

    /// Remember that `container` reaches no free identity now.
    fn summarize(&mut self, container: Word) {
        let serial = self.resources.serial;
        self.resources.summaries.insert(container, serial);
        self.resources.summarized = self.resources.summarized.max(serial);
        self.heap.summarized(container, true);
        self.heap.watch(container);
    }

    /// May a walk (through `via`, if any) skip `container`? `oldest` caches
    /// the oldest entry of `via` for the rest of the walk.
    fn summary_holds(&self, container: Word, via: Option<Word>, oldest: &mut Option<u64>) -> bool {
        let Some(&serial) = self.resources.summaries.get(&container) else {
            return false;
        };
        let Some(region) = via else {
            return true;
        };
        let oldest = *oldest.get_or_insert_with(|| self.region_oldest(region));
        serial < oldest
    }

    /// An identity no newer than a summary becomes free: the summaries may
    /// reach it now.
    fn freed(&mut self, serial: u64) {
        if serial > self.resources.summarized {
            return;
        }
        for (container, _) in self.resources.summaries.drain() {
            self.heap.summarized(container, false);
        }
        self.resources.summarized = 0;
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
            heap::CLOSURE | heap::PAYLOAD | heap::FRAME => true,
            _ => false,
        }
    }

    /// `value` if it is a free identity none of whose fields may reach a free
    /// identity (`may_reach_free(value)` holds): the walk would find just it.
    fn lone_free(&self, value: Word) -> Option<Word> {
        let header = self.heap.header_of(value);
        if heap::kind(header) != heap::OBJECT {
            return None;
        }
        let layout = &self.heap.layouts().objects[heap::payload(header) as usize];
        for (index, &field) in layout.iter().enumerate() {
            if field.pointer(self.heap.field(value, index)).is_some_and(|pointer| self.may_reach_free(pointer)) {
                return None;
            }
        }
        Some(value)
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
        let serial = self.serial_of(object);
        let overflow = self.heap.field(region, 0);
        let overflow = self.heap.payload_mut::<RegionValue>(overflow).expect("region");
        overflow.entries.push(object);
        overflow.members.insert(object);
        overflow.oldest = Some(overflow.oldest.map_or(serial, |oldest| oldest.min(serial)));
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
            overflow.oldest = None;
        }
    }

    /// Serial of an identity; 0 (older than anything) for none.
    fn serial_of(&self, object: Word) -> u64 {
        self.resources.identities.get(&object).map_or(0, |identity| identity.serial)
    }

    /// The oldest serial among the region's entries (`u64::MAX`: none).
    fn region_oldest(&self, region: Word) -> u64 {
        let inline = (1..=REGION_SLOTS)
            .map(|slot| self.heap.field(region, slot))
            .take_while(|&entry| entry != UNBOUND)
            .map(|entry| self.serial_of(entry))
            .min()
            .unwrap_or(u64::MAX);
        match self.heap.field(region, 0) {
            UNBOUND => inline,
            overflow => {
                let overflow = self.heap.payload::<RegionValue>(overflow).expect("region");
                overflow.oldest.map_or(inline, |oldest| oldest.min(inline))
            }
        }
    }

    pub(crate) fn region_add(&mut self, region: Word, value: Word, own: bool) {
        if !self.may_reach_free(value) {
            return;
        }
        // The common add, a cursor or a constructor's result, is one free
        // identity whose fields reach no other: no walk.
        if let Some(object) = self.lone_free(value) {
            if own || self.resources.identities[&object].cache_rehomed {
                self.resources.identities.get_mut(&object).expect("identity").owner = Owner::Region(region);
            } else if self.region_contains(region, object) {
                return;
            }
            self.region_push(region, object);
            return;
        }
        // Tracking (not owning) skips what the region already holds; adopting
        // must still take ownership of tracked objects.
        let known = if own { None } else { Some(region) };
        for object in self.resource_graph_into(&[value], None, known) {
            if own || self.resources.identities[&object].cache_rehomed {
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
    pub(crate) fn failure_payloads(&self, failure: Word) -> Vec<Word> {
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
    fn region_close(&mut self, ctx: *mut Ctx, region: Word, keep: &[Word], primary: Option<Word>) -> Option<Word> {
        match self.region_close_until(ctx, region, keep, primary, false) {
            Closed::Done(primary) => primary,
            Closed::Async(..) => unreachable!("a synchronous close awaits nothing"),
        }
    }

    /// The pending async destroy of a settling region and the failure so far.
    pub(crate) fn region_settling(&self, region: Word) -> (Option<Word>, Option<Word>) {
        match self.heap.field(region, 0) {
            UNBOUND => (None, None),
            overflow => {
                let value = self.heap.payload::<RegionValue>(overflow).expect("region");
                (value.settling, value.settled)
            }
        }
    }

    pub(crate) fn set_region_settling(&mut self, region: Word, settling: Option<Word>, settled: Option<Word>) {
        if self.heap.field(region, 0) == UNBOUND {
            if settling.is_none() && settled.is_none() {
                return;
            }
            let overflow = self.heap.allocate_payload(Box::new(RegionValue::default()));
            self.heap.set_field(region, 0, overflow);
        }
        let overflow = self.heap.field(region, 0);
        let value = self.heap.payload_mut::<RegionValue>(overflow).expect("region");
        value.settling = settling;
        value.settled = settled;
    }

    /// Destroy the region's identities from the last; with `awaits`, stop at
    /// one whose destroy is async (#154), otherwise that one is a failure.
    pub(crate) fn region_close_until(
        &mut self,
        ctx: *mut Ctx,
        region: Word,
        keep: &[Word],
        mut primary: Option<Word>,
        awaits: bool,
    ) -> Closed {
        let length = self.region_len(region);
        if length == 0 {
            return Closed::Done(primary);
        }
        // The common close keeps all it holds: a constructor, or an action
        // returning what it made. Nothing dies, so nothing is walked.
        if length <= REGION_SLOTS && (0..length).all(|index| keep.contains(&self.region_get(region, index))) {
            for index in 0..length {
                let object = self.region_get(region, index);
                if let Some(identity) = self.resources.identities.get_mut(&object)
                    && identity.owner == Owner::Region(region)
                {
                    identity.owner = Owner::Free;
                    let serial = identity.serial;
                    self.freed(serial);
                }
            }
            self.region_clear(region);
            return Closed::Done(primary);
        }
        let mut kept: HashSet<Word> =
            if keep.is_empty() { HashSet::default() } else { self.resource_graph(keep, Some(region)).into_iter().collect() };
        // Groups add ownership edges, including to bound Futures. Drain
        // dying groups first, withdrawing their edges before deciding which
        // Futures die. The surviving group packets retain a Future when its
        // lexical owner ends; the final owner releases its cache exactly once.
        let (mut groups, mut futures) = (Vec::new(), Vec::new());
        for index in 0..self.region_len(region) {
            self.queue_private(self.region_get(region, index), &mut groups, &mut futures);
        }
        while let Some(object) = groups.pop().or_else(|| futures.pop()) {
            if kept.contains(&object) || !self.resources.identities.get(&object)
                .is_some_and(|identity| identity.owner == Owner::Free || identity.owner == Owner::Region(region))
            {
                continue;
            }
            let group = matches!(self.states.get(&object), Some(crate::primitives::State::Group(_)));
            if !group
                && let Some(&home) = self.resources.identities[&object].groups.first()
            {
                for child in self.resource_graph(&[object], Some(region)) {
                    self.resources.identities.get_mut(&child).expect("identity").owner = Owner::Region(home);
                    self.region_push(home, child);
                }
                continue;
            }
            self.resources.identities.get_mut(&object).expect("identity").owner = Owner::Dead;
            if let Some(owned) = self.private_region(object) {
                for index in 0..self.region_len(owned) {
                    let child = self.region_get(owned, index);
                    let identity = self.resources.identities.get_mut(&child).expect("identity");
                    if group { identity.groups.retain(|&packet| packet != owned); }
                    if identity.owner == Owner::Region(owned) {
                        identity.owner = Owner::Region(region);
                        identity.cache_rehomed = true;
                        self.region_push(region, child);
                        self.queue_private(child, &mut groups, &mut futures);
                    }
                }
                self.region_clear(owned);
                if !keep.is_empty() {
                    kept.extend(self.resource_graph(keep, Some(region)));
                }
            }
        }
        if !keep.is_empty() {
            kept.extend(self.resource_graph(keep, Some(region)));
        }
        let mut entries: Vec<Word> = (0..self.region_len(region)).map(|i| self.region_get(region, i)).collect();
        entries.sort_unstable_by_key(|object| self.serial_of(*object));
        self.region_clear(region);
        for object in entries {
            self.region_push(region, object);
        }
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
                    let serial = identity.serial;
                    self.freed(serial);
                }
                continue;
            }
            if identity.owner != Owner::Free && identity.owner != Owner::Region(region) {
                continue;
            }
            identity.owner = Owner::Dead;
            let serial = identity.serial;
            let descriptor = heap::payload(self.heap.header_of(object)) as usize;
            let Some(destroy) = self.loaded.program.objects[descriptor].destroy else { continue };
            if let Some(trace) = &mut self.resources.trace {
                use std::io::Write;
                let _ = writeln!(trace, "{} {serial}", self.loaded.program.objects[descriptor].name);
            }
            if self.loaded.program.functions[destroy.0 as usize].is_async {
                if awaits {
                    self.roots.pop();
                    return Closed::Async(object, primary);
                }
                if let Some(primary) = primary {
                    self.roots.push((primary, Repr::Ref));
                }
                let failure = self.failure_new(FailureKind::Program(
                    "an object with an async destroy was released in a synchronous scope".into(),
                ));
                if primary.is_some() {
                    self.roots.pop();
                }
                primary = Some(match primary {
                    Some(primary) => self.failure_link(primary, failure, true),
                    None => failure,
                });
                continue;
            }
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
        Closed::Done(primary)
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
                if let Some(function) = self.error_text_method(object)
                    && let Ok(code) = crate::jit::compile(self, function) {
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
        let trace = record.trace.iter().map(|&(function, pc)| crate::Site { function, pc }).collect();
        Some(crate::Uncaught { category, error_type, message, trace })
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
        const CHAIN_ENTRIES: usize = 64;
        use std::fmt::Write;
        let mut output = String::new();
        let mut pending = vec![(root, "")];
        let mut shown: HashMap<Word, usize> = HashMap::default();
        let mut next_index = 0;
        while let Some((failure, label)) = pending.pop() {
            if !label.is_empty() {
                let _ = write!(output, "\n{label}\n  ");
            }
            if let Some(index) = shown.get(&failure) {
                let _ = write!(output, "[failure #{index} already shown]");
                continue;
            }
            // The same bound as the other engines' reports (#270).
            if next_index == CHAIN_ENTRIES {
                output.push_str("... further failures not shown");
                break;
            }
            shown.insert(failure, next_index);
            next_index += 1;
            let Some(record) = self.failure(failure) else {
                output.push_str("<invalid failure reference>");
                continue;
            };
            match &record.kind {
                FailureKind::Cancelled => output.push_str("Cancelled: Operation was cancelled"),
                FailureKind::Native(object) => {
                    // Host details as the IR engine and the JVM print them; a
                    // nested entry keeps them indented under its label.
                    let field = |index| self.heap.text(self.heap.field(*object, index));
                    let text = format!(
                        "NativeFailure: {}\ncode: {}\ndomain: {}\noperation: {}",
                        field(3),
                        field(1),
                        field(0),
                        field(2)
                    );
                    output.push_str(&if label.is_empty() { text } else { text.replace('\n', "\n  ") });
                }
                FailureKind::Program(message) => {
                    let _ = write!(output, "RuntimeError: {message}");
                }
                FailureKind::Panic(message) => {
                    let _ = write!(output, "Panic: {message}");
                }
                FailureKind::Thrown(object) => {
                    let header = self.heap.header_of(*object);
                    // The type's own name, as the other engines print it.
                    let name = if heap::kind(header) == heap::OBJECT {
                        let full = &self.loaded.program.objects[heap::payload(header) as usize].name;
                        full.rsplit("::").next().unwrap_or(full).to_owned()
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
                pending.push((cause, "Caused by:"));
            }
            pending.extend(record.secondary.iter().rev().map(|&s| (s, "Secondary cleanup failure:")));
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

/// `region.leave` of a region that holds something: close it, keeping what the
/// `count` words at `keep` reach.
#[inline(never)]
pub extern "C" fn rt_region_leave(ctx_ptr: *mut Ctx, region: Word, keep: *const Word, count: u64) {
    let (_, runtime) = unsafe { parts(ctx_ptr) };
    runtime.count(|_| "region leave".into());
    let keep = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(keep, count as usize) } };
    if let Some(failure) = runtime.region_close(ctx_ptr, region, keep, None) {
        let ctx = unsafe { &mut *ctx_ptr };
        ctx.failure = failure;
        ctx.status = FAILED;
    }
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
    let operands = crate::ops::Operands::new(registers, words);
    let value = |register: &okkama_sparrow::Register| operands.value(register);
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
            CatchKind::Variant => false,
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
            CatchKind::Variant => false,
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
        CatchKind::Variant => runtime.loaded.program.variants[*ty as usize].cases.iter().any(|(_, o)| o.0 == owner),
    };
    if matches {
        ctx.scratch = payload;
    }
    u64::from(matches)
}
