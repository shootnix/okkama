//! The entries of a map value (#252–#254): a persistent B+ tree whose nodes
//! versions share (the default), or the standard B-tree that every version
//! copies (`OKKAMA_MAPS=btree`, kept so that one binary measures both).
//!
//! A persistent map is cloned in O(1); a write copies one path from the
//! root, or changes nodes in place while no other version holds them (a map
//! being built). Its nodes' memory, with what their keys own, is counted
//! exactly per thread (`okkama_persistent::pmap::memory`), which the heap
//! reads instead of an estimate per map, so versions that share nodes are
//! not charged twice; and the collector traces a shared node once per
//! collection ([`Entries::trace`]).

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, btree_map};

use okkama_persistent::{PMap, pmap};

use crate::ops::Key;
use crate::word::Word;

/// A key's word and the value's word, as stored.
pub type Entry = (Word, Word);

/// Persistent maps unless `OKKAMA_MAPS=btree`.
pub fn persistent_maps() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("OKKAMA_MAPS").map_or(true, |v| v != "btree"))
}

/// Bytes of persistent map nodes allocated by this thread since it started,
/// and live now.
pub fn node_memory() -> (usize, usize) {
    pmap::memory()
}

thread_local! {
    static PASS: Cell<u32> = const { Cell::new(0) };
}

/// A collection starts marking: nodes visited from here on are visited in a
/// new pass ([`PMap::for_each_unvisited`]).
pub fn begin_pass() {
    PASS.with(|pass| pass.set(pass.get().checked_add(1).unwrap_or(1)));
}

impl pmap::Weigh for Key {
    fn weight(&self) -> usize {
        self.data_bytes()
    }
}

#[derive(Debug, Clone)]
pub enum Entries {
    BTree(BTreeMap<Key, Entry>),
    Persistent(PMap<Key, Entry>),
}

impl Default for Entries {
    fn default() -> Self {
        if persistent_maps() { Entries::Persistent(PMap::new()) } else { Entries::BTree(BTreeMap::new()) }
    }
}

impl Entries {
    pub fn len(&self) -> usize {
        match self {
            Entries::BTree(map) => map.len(),
            Entries::Persistent(map) => map.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_persistent(&self) -> bool {
        matches!(self, Entries::Persistent(_))
    }

    pub fn get(&self, key: &Key) -> Option<&Entry> {
        match self {
            Entries::BTree(map) => map.get(key),
            Entries::Persistent(map) => map.get(key),
        }
    }

    pub fn contains_key(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }

    /// Insert or replace the entry of `key`.
    pub fn insert(&mut self, key: Key, entry: Entry) {
        match self {
            Entries::BTree(map) => {
                map.insert(key, entry);
            }
            Entries::Persistent(map) => {
                map.insert(key, entry);
            }
        }
    }

    /// Set the value of an equal key and keep its stored key word, or insert
    /// `entry`; true when the key was new. The left operand of a merge keeps
    /// its key representatives, as `BTreeMap::entry` did.
    pub fn upsert(&mut self, key: Key, entry: Entry) -> bool {
        match self {
            Entries::BTree(map) => match map.entry(key) {
                btree_map::Entry::Occupied(mut found) => {
                    found.get_mut().1 = entry.1;
                    false
                }
                btree_map::Entry::Vacant(slot) => {
                    slot.insert(entry);
                    true
                }
            },
            Entries::Persistent(map) => map.upsert(key, || entry, |stored| stored.1 = entry.1),
        }
    }

    pub fn remove(&mut self, key: &Key) {
        match self {
            Entries::BTree(map) => {
                map.remove(key);
            }
            Entries::Persistent(map) => {
                map.remove(key);
            }
        }
    }

    /// Each entry for the collector to trace; of a persistent map, only the
    /// entries of nodes this collection has not traced through another
    /// version.
    pub fn trace(&self, mut visit: impl FnMut(&Entry)) {
        match self {
            Entries::BTree(map) => map.values().for_each(visit),
            Entries::Persistent(map) => map.for_each_unvisited(PASS.with(Cell::get), |_, entry| visit(entry)),
        }
    }

    /// Merge `right` in: its values win, the key words of `self` stay
    /// (#254). Into a standard map, entry by entry; persistent maps by the
    /// cheaper of inserting the smaller into the larger and one ordered pass
    /// over both.
    pub fn merge(&mut self, right: &Entries) {
        match (self, right) {
            (Entries::Persistent(left), Entries::Persistent(right)) => *left = merged(left, right),
            (this, right) => {
                for (key, &entry) in right.iter() {
                    this.upsert(key.clone(), entry);
                }
            }
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        match self {
            Entries::BTree(map) => Iter::BTree(map.iter()),
            Entries::Persistent(map) => Iter::Persistent(map.iter()),
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &Entry> {
        self.iter().map(|(_, v)| v)
    }
}

/// Entries in key order.
#[derive(Debug)]
pub enum Iter<'a> {
    BTree(btree_map::Iter<'a, Key, Entry>),
    Persistent(pmap::Iter<'a, Key, Entry>),
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Key, &'a Entry);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::BTree(iter) => iter.next(),
            Iter::Persistent(iter) => iter.next(),
        }
    }
}

/// An insert costs about as much as copying this many entries in an ordered
/// pass, so a part this many times smaller is inserted entry by entry.
const INSERT_COST: usize = 32;

fn merged(left: &PMap<Key, Entry>, right: &PMap<Key, Entry>) -> PMap<Key, Entry> {
    if right.len() * INSERT_COST < left.len() {
        let mut out = left.clone();
        for (key, &(word, value)) in right.iter() {
            out.upsert(key.clone(), || (word, value), |stored| stored.1 = value);
        }
        return out;
    }
    if left.len() * INSERT_COST < right.len() {
        let mut out = right.clone();
        for (key, &(word, value)) in left.iter() {
            out.upsert(key.clone(), || (word, value), |stored| stored.0 = word);
        }
        return out;
    }
    let mut entries = Vec::with_capacity(left.len() + right.len());
    let (mut a, mut b) = (left.iter().peekable(), right.iter().peekable());
    loop {
        let order = match (a.peek(), b.peek()) {
            (Some((x, _)), Some((y, _))) => x.cmp(y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => break,
        };
        let entry = match order {
            Ordering::Less => a.next().map(|(key, &entry)| (key.clone(), entry)),
            Ordering::Greater => b.next().map(|(key, &entry)| (key.clone(), entry)),
            Ordering::Equal => a.next().zip(b.next()).map(|((key, &(word, _)), (_, &(_, value)))| (key.clone(), (word, value))),
        };
        entries.extend(entry);
    }
    PMap::from_sorted(entries)
}
