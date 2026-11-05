//! Explicit resource ownership, independent of memory reclamation.
//! Ordinary programs leave identity metadata disabled. No assignment walks a graph.
use crate::{Error, arena::Arena, value::*};
use std::collections::HashSet;

/// Closed-world type reachability: a plain wrapper is skipped only when none of
/// its declared fields can reach a destroy-bearing identity. Cycles converge by
/// fixed point; any, roles, functions and future/native values stay conservative.
pub(crate) fn descriptor_flags(program: &okkama_sparrow::Program) -> std::rc::Rc<[bool]> {
    use okkama_sparrow::Type;
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
    // Reverse graph reachability is linear in declarations and fields, including
    // mutually recursive types; no repeated scan or recursive Rust traversal.
    while let Some(node) = pending.pop() {
        if std::mem::replace(&mut reachable[node], true) {
            continue;
        }
        pending.extend(parents[node].iter().copied());
    }
    reachable[offset..].into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Free,
    Region(u64),
    Dead,
}
#[derive(Debug)]
struct Identity {
    serial: u64,
    owner: Owner,
}
#[derive(Debug, Default)]
pub(crate) struct Region {
    entries: Vec<Slot>,
    known: HashSet<u64>,
}
impl Region {
    fn remember(&mut self, object: Slot) {
        if self.known.insert(object.bits) {
            self.entries.push(object);
        }
    }
    pub fn roots(&self) -> impl Iterator<Item = Slot> + '_ {
        self.entries.iter().copied()
    }
}
#[derive(Debug, Default)]
pub(crate) struct Resources {
    pub enabled: bool,
    pub descriptors: Option<std::rc::Rc<[bool]>>,
    serial: u64,
    identities: crate::value::HandleMap<Identity>,
    pending: Vec<Slot>,
    seen: HashSet<(u64, u64), crate::value::HandleBuild>,
}
impl Resources {
    pub fn created(&mut self, descriptor: usize, object: Slot) -> Result<(), Error> {
        if self.enabled
            && self
                .descriptors
                .as_ref()
                .is_none_or(|flags| flags[descriptor])
        {
            let serial = self
                .serial
                .checked_add(1)
                .ok_or_else(|| Error("resource identity space exhausted".into()))?;
            self.serial = serial;
            self.identities.insert(
                object.bits,
                Identity {
                    serial,
                    owner: Owner::Free,
                },
            );
        }
        Ok(())
    }
    pub fn sweep(&mut self, objects: &Arena<Box<[Slot]>>) {
        self.identities
            .retain(|handle, _| objects.get(*handle).is_ok());
    }
}
#[derive(Debug)]
pub(crate) struct Closing {
    pub region: Slot,
    kept: HashSet<u64>,
    index: usize,
}
impl Heap {
    pub fn region_new(&mut self) -> Result<Slot, Error> {
        Ok(Slot {
            bits: self.regions.allocate(Region::default())?,
            tag: REGION,
        })
    }
    fn region_check(&self, region: Slot) -> Result<(), Error> {
        if region.tag != REGION {
            return Err(Error("expected region handle".into()));
        }
        self.regions.get(region.bits)?;
        Ok(())
    }
    // Snapshot the same free/owned subgraph that the language ownership operation
    // observes. Foreign owners stop traversal, including through plain wrappers.
    fn resource_graph(
        &mut self,
        roots: impl Iterator<Item = Slot>,
        via: Option<u64>,
    ) -> Result<Vec<Slot>, Error> {
        if !self.resources.enabled {
            return Ok(Vec::new());
        }
        let mut pending = std::mem::take(&mut self.resources.pending);
        let mut seen = std::mem::take(&mut self.resources.seen);
        pending.extend(roots);
        let mut found = Vec::new();
        let result = (|| {
            while let Some(value) = pending.pop() {
                if !seen.insert((value.tag, value.bits)) {
                    continue;
                }
                match value.tag {
                    OBJECT => {
                        let descriptor = self.object_descriptor(value)?;
                        if self
                            .resources
                            .descriptors
                            .as_ref()
                            .is_some_and(|flags| !flags[descriptor])
                        {
                            continue;
                        }
                        let identity = self
                            .resources
                            .identities
                            .get(&value.bits)
                            .ok_or_else(|| Error("missing resource identity".into()))?;
                        if identity.owner != Owner::Free
                            && Some(identity.owner) != via.map(Owner::Region)
                        {
                            continue;
                        }
                        pending.extend(self.objects.get(value.bits)?[1..].iter().rev().copied());
                        found.push(value);
                    }
                    ARRAY => pending.extend(self.arrays.get(value.bits)?.iter().rev().copied()),
                    MAP => {
                        pending.extend(self.maps.get(value.bits)?.values().rev().map(|(_, v)| *v))
                    }
                    ANY => pending.push(*self.boxes.get(value.bits)?),
                    FUTURE => {
                        let future = self.future(value)?;
                        pending.extend(future.captures.iter().copied());
                        if let crate::future::State::Ready(Ok(value)) = future.state {
                            pending.push(value);
                        }
                    }
                    CLOSURE => pending.extend(self.closure(value)?.1.iter().rev().copied()),
                    _ => {}
                }
            }
            Ok(())
        })();
        pending.clear();
        seen.clear();
        self.resources.pending = pending;
        self.resources.seen = seen;
        result?;
        found.sort_unstable_by_key(|s| self.resources.identities[&s.bits].serial);
        Ok(found)
    }
    pub fn region_register(&mut self, region: Slot, value: Slot, own: bool) -> Result<(), Error> {
        self.region_check(region)?;
        for object in self.resource_graph([value].into_iter(), None)? {
            if own {
                self.resources
                    .identities
                    .get_mut(&object.bits)
                    .unwrap()
                    .owner = Owner::Region(region.bits);
            }
            self.regions.get_mut(region.bits)?.remember(object);
        }
        Ok(())
    }
    pub fn region_transfer(&mut self, from: Slot, to: Slot, value: Slot) -> Result<(), Error> {
        self.region_check(from)?;
        self.region_check(to)?;
        if self.regions.get(from.bits)?.entries.is_empty() {
            return Ok(());
        }
        for object in self.resource_graph([value].into_iter(), Some(from.bits))? {
            let identity = self.resources.identities.get_mut(&object.bits).unwrap();
            if identity.owner == Owner::Region(from.bits) {
                identity.owner = Owner::Region(to.bits);
                self.regions.get_mut(to.bits)?.remember(object);
            }
        }
        Ok(())
    }
    pub fn region_close(
        &mut self,
        region: Slot,
        keep: impl Iterator<Item = Slot>,
    ) -> Result<Closing, Error> {
        self.region_check(region)?;
        let kept = self
            .resource_graph(keep, Some(region.bits))?
            .into_iter()
            .map(|v| v.bits)
            .collect();
        Ok(Closing {
            region,
            kept,
            index: self.regions.get(region.bits)?.entries.len(),
        })
    }
    pub fn region_next(&mut self, closing: &mut Closing) -> Result<Option<Slot>, Error> {
        while closing.index > 0 {
            closing.index -= 1;
            let object = self.regions.get(closing.region.bits)?.entries[closing.index];
            let identity = self
                .resources
                .identities
                .get_mut(&object.bits)
                .ok_or_else(|| Error("missing resource identity".into()))?;
            if closing.kept.contains(&object.bits) {
                if identity.owner == Owner::Region(closing.region.bits) {
                    identity.owner = Owner::Free;
                }
            } else if identity.owner == Owner::Free
                || identity.owner == Owner::Region(closing.region.bits)
            {
                identity.owner = Owner::Dead;
                return Ok(Some(object));
            }
        }
        let region = self.regions.get_mut(closing.region.bits)?;
        region.entries.clear();
        region.known.clear();
        Ok(None)
    }
    pub fn failure_payloads(&self, failure: Slot) -> Result<Vec<Slot>, Error> {
        let mut roots = Vec::new();
        let mut pending = vec![failure];
        let mut seen = HashSet::new();
        while let Some(value) = pending.pop() {
            if !seen.insert(value.bits) {
                continue;
            }
            let record = self.failure(value)?;
            if let crate::failure::FailureKind::Thrown(payload) = record.kind {
                roots.push(payload);
            }
            pending.extend(record.cause);
            pending.extend(record.secondary.iter().copied());
        }
        Ok(roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn heap() -> Heap {
        let mut heap = Heap::new(&[], &[]);
        heap.resources.enabled = true;
        heap
    }
    fn drain(heap: &mut Heap, region: Slot, keep: &[Slot]) -> Vec<u64> {
        let mut closing = heap.region_close(region, keep.iter().copied()).unwrap();
        let mut result = Vec::new();
        while let Some(object) = heap.region_next(&mut closing).unwrap() {
            result.push(object.bits);
        }
        result
    }
    #[test]
    fn ownership_uses_creation_order_and_transfer_never_takes_foreign_objects() {
        let mut heap = heap();
        let first = heap.object_new(0, [].into_iter()).unwrap();
        let second = heap.object_new(0, [].into_iter()).unwrap();
        let wrapper = heap.object_new(0, [second, first].into_iter()).unwrap();
        let a = heap.region_new().unwrap();
        let b = heap.region_new().unwrap();
        heap.region_register(a, wrapper, true).unwrap();
        heap.region_register(b, wrapper, true).unwrap(); // borrowing owned root
        assert!(drain(&mut heap, b, &[]).is_empty());
        heap.region_transfer(a, b, first).unwrap();
        assert_eq!(drain(&mut heap, a, &[]), vec![wrapper.bits, second.bits]);
        assert_eq!(drain(&mut heap, b, &[]), vec![first.bits]);
        assert!(drain(&mut heap, a, &[]).is_empty());
    }
    #[test]
    fn retained_closure_graph_survives_close_and_is_adopted_by_another_scope() {
        let mut heap = heap();
        let child = heap.object_new(0, [Slot::small(42)].into_iter()).unwrap();
        let closure = heap.closure_new(0, [child].into_iter()).unwrap();
        let parent = heap.region_new().unwrap();
        let scope = heap.region_new().unwrap();
        heap.region_register(scope, closure, true).unwrap();
        assert!(drain(&mut heap, scope, &[closure]).is_empty());
        heap.region_register(parent, closure, true).unwrap();
        heap.collect([parent].into_iter()).unwrap();
        assert_eq!(heap.object_get(child, 0).unwrap().bits, 42);
        assert_eq!(drain(&mut heap, parent, &[]), vec![child.bits]);
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.object_descriptor(child).is_err());
    }
    #[test]
    fn tracking_aliases_is_idempotent_and_destruction_never_repeats() {
        let mut heap = heap();
        let value = heap.object_new(0, [Slot::default()].into_iter()).unwrap();
        let a = heap.region_new().unwrap();
        let b = heap.region_new().unwrap();
        heap.region_register(a, value, false).unwrap();
        heap.region_register(a, value, false).unwrap();
        heap.region_register(b, value, true).unwrap();
        assert!(drain(&mut heap, a, &[]).is_empty());
        assert_eq!(drain(&mut heap, b, &[]), vec![value.bits]);
        heap.region_register(a, value, true).unwrap();
        assert!(drain(&mut heap, a, &[]).is_empty());
    }
    #[test]
    fn late_binding_does_not_retroactively_change_the_ownership_snapshot() {
        let mut heap = heap();
        let a = heap.region_new().unwrap();
        let b = heap.region_new().unwrap();
        let wrapper = heap
            .object_new(
                0,
                [Slot {
                    bits: 0,
                    tag: UNBOUND,
                }]
                .into_iter(),
            )
            .unwrap();
        heap.region_register(a, wrapper, true).unwrap();
        let child = heap.object_new(0, [Slot::small(42)].into_iter()).unwrap();
        heap.object_init(wrapper, 0, child).unwrap();
        // An already-owned wrapper blocks adoption by a different scope.
        heap.region_register(b, wrapper, true).unwrap();
        assert!(drain(&mut heap, b, &[]).is_empty());
        // The keep traversal does see late fields, without stealing the child.
        assert!(drain(&mut heap, a, &[wrapper]).is_empty());
        heap.region_register(b, wrapper, true).unwrap();
        assert_eq!(drain(&mut heap, b, &[]), vec![child.bits, wrapper.bits]);
    }
    #[test]
    fn plain_programs_do_not_allocate_identity_metadata_or_walk_deep_values() {
        let mut heap = Heap::new(&[], &[]);
        let mut value = Slot::small(42);
        for _ in 0..100_000 {
            value = heap.object_new(0, [value].into_iter()).unwrap();
        }
        let region = heap.region_new().unwrap();
        heap.region_register(region, value, true).unwrap();
        assert!(heap.resources.identities.is_empty());
        assert!(heap.resources.pending.is_empty());
        assert!(drain(&mut heap, region, &[]).is_empty());
    }
}

#[cfg(test)]
mod descriptor_tests {
    use super::*;
    use okkama_sparrow::{FuncRef, ObjectDecl, ObjectRef, Program, Type, TypeRef};
    #[test]
    fn resource_reachability_includes_wrappers_and_late_cycles_but_skips_plain_cycles() {
        let object = |name: &str, fields: Vec<(String, TypeRef)>, destroy| ObjectDecl {
            name: name.into(),
            fields,
            does: vec![],
            destroy,
            builtin: false,
        };
        let program = Program {
            types: vec![
                Type::Object(ObjectRef(0)),
                Type::Object(ObjectRef(1)),
                Type::Object(ObjectRef(2)),
                Type::Array(TypeRef(2)),
                Type::Any,
            ],
            objects: vec![
                object("Plain", vec![("next".into(), TypeRef(0))], None),
                object(
                    "Wrapper",
                    vec![
                        ("next".into(), TypeRef(1)),
                        ("resources".into(), TypeRef(3)),
                    ],
                    None,
                ),
                object("Resource", vec![], Some(FuncRef(0))),
                object("Opaque", vec![("value".into(), TypeRef(4))], None),
            ],
            ..Program::default()
        };
        let flags = descriptor_flags(&program);
        assert_eq!(&*flags, &[false, true, true, true]);
        let mut heap = Heap::new(&[], &[]);
        heap.resources.enabled = true;
        heap.resources.descriptors = Some(flags);
        let mut plain = Slot::small(42);
        for _ in 0..100_000 {
            plain = heap.object_new(0, [plain].into_iter()).unwrap();
        }
        let region = heap.region_new().unwrap();
        heap.region_register(region, plain, true).unwrap();
        assert!(heap.resources.identities.is_empty());
        assert!(heap.regions.get(region.bits).unwrap().entries.is_empty());
        let child = heap.object_new(2, [].into_iter()).unwrap();
        let array = heap.allocate_array(vec![child]).unwrap();
        let wrapper = heap.object_new(1, [plain, array].into_iter()).unwrap();
        heap.region_register(region, wrapper, true).unwrap();
        let mut closing = heap.region_close(region, std::iter::empty()).unwrap();
        assert_eq!(
            heap.region_next(&mut closing).unwrap().unwrap().bits,
            wrapper.bits
        );
        assert_eq!(
            heap.region_next(&mut closing).unwrap().unwrap().bits,
            child.bits
        );
        assert!(heap.region_next(&mut closing).unwrap().is_none());
    }
}
