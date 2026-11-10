//! The entries of a map value (#252–#254): a persistent B+ tree whose nodes
//! versions share (the default), or the standard B-tree that every version
//! copies (`OKKAMA_MAPS=btree`, kept so that one binary measures both).
//! Under the default a map is a standard B-tree until a write makes a new
//! version of it while it holds more than [`SMALL`] entries; it then turns
//! persistent in place ([`Entries::make_persistent`]), and it and its
//! versions share nodes from there on. A map that is only built and read —
//! by a builder, a literal, or a version of a small map — costs what it
//! did before; its first version costs the copy it did before, the next
//! ones a path (the owner's decision on #252–#254, 2026-11-10).
//!
//! A map of at most [`TINY`] entries is neither: it is one sorted vector of
//! its own size, cloned and searched in place, and becomes the standard
//! B-tree when it grows past that; a literal of more entries starts as the
//! B-tree (#325).
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

/// Entries a standard map holds at most and still makes its versions by a
/// copy, as many as an array keeps flat before it becomes a trie.
pub const SMALL: usize = 64;

/// Entries a map keeps in one sorted vector before it becomes a B-tree: a
/// small map is one allocation of its own size, not a B-tree leaf (#325).
pub const TINY: usize = 8;

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

impl pmap::NodeKey for Key {
    fn weight(&self) -> usize {
        self.data_bytes()
    }

    /// A small integer compares inline; any other key may need a call.
    #[inline]
    fn seek(keys: &[Key], key: &Key) -> (usize, bool) {
        if let Key::Small(_) = key { pmap::seek_scan(keys, key) } else { pmap::seek_three_way(keys, key) }
    }
}

#[derive(Debug, Clone)]
pub enum Entries {
    Tiny(Vec<(Key, Entry)>),
    BTree(BTreeMap<Key, Entry>),
    Persistent(PMap<Key, Entry>),
}

impl Default for Entries {
    fn default() -> Self {
        Entries::Tiny(Vec::new())
    }
}

impl Entries {
    /// Room for `count` entries, as a literal knows them: a tiny map of its
    /// own size, or at once the B-tree that more entries would turn it into.
    pub fn with_capacity(count: usize) -> Self {
        if count <= TINY { Entries::Tiny(Vec::with_capacity(count)) } else { Entries::BTree(BTreeMap::new()) }
    }

    pub fn len(&self) -> usize {
        match self {
            Entries::Tiny(list) => list.len(),
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
            Entries::Tiny(list) => tiny_seek(list, key).ok().map(|at| &list[at].1),
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
            Entries::Tiny(list) => match tiny_seek(list, &key) {
                Ok(at) => list[at].1 = entry,
                Err(at) => self.tiny_insert(at, key, entry),
            },
            Entries::BTree(map) => {
                map.insert(key, entry);
            }
            Entries::Persistent(map) => {
                map.insert(key, entry);
            }
        }
    }

    /// A standard map of more than [`SMALL`] entries that a new version is
    /// about to be made from turns persistent, in place: the same entries,
    /// built bottom-up into full nodes as the B-tree hands them over, so it
    /// and every later version share them. A Tomato heap has one thread, so
    /// no reader sees the change.
    pub fn make_persistent(&mut self) {
        if let Entries::BTree(map) = self
            && map.len() > SMALL
            && persistent_maps()
        {
            *self = Entries::Persistent(PMap::from_sorted(std::mem::take(map)));
        }
    }

    /// Set the value of an equal key and keep its stored key word, or insert
    /// `entry`; true when the key was new. The left operand of a merge keeps
    /// its key representatives, as `BTreeMap::entry` did.
    pub fn upsert(&mut self, key: Key, entry: Entry) -> bool {
        match self {
            Entries::Tiny(list) => match tiny_seek(list, &key) {
                Ok(at) => {
                    list[at].1.1 = entry.1;
                    false
                }
                Err(at) => {
                    self.tiny_insert(at, key, entry);
                    true
                }
            },
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
            Entries::Tiny(list) => {
                if let Ok(at) = tiny_seek(list, key) {
                    list.remove(at);
                }
            }
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
            Entries::Tiny(list) => list.iter().for_each(|(_, entry)| visit(entry)),
            Entries::BTree(map) => map.values().for_each(visit),
            Entries::Persistent(map) => map.for_each_unvisited(PASS.with(Cell::get), |_, entry| visit(entry)),
        }
    }

    /// Merge `right` in: its values win, the key words of `self` stay
    /// (#254). Into a standard map, entry by entry; persistent maps by the
    /// cheaper of inserting the smaller into the larger and one ordered pass
    /// over both.
    pub fn merge(&mut self, right: &Entries) {
        match (&mut *self, right) {
            (Entries::Persistent(left), Entries::Persistent(right)) => *left = merged(left, right),
            (Entries::BTree(left), Entries::Persistent(right)) => {
                let left = PMap::from_sorted(std::mem::take(left));
                *self = Entries::Persistent(merged(&left, right));
            }
            (Entries::Tiny(left), Entries::Persistent(right)) => {
                let left = PMap::from_sorted(std::mem::take(left));
                *self = Entries::Persistent(merged(&left, right));
            }
            (this, right) => {
                for (key, &entry) in right.iter() {
                    this.upsert(key.clone(), entry);
                }
            }
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        match self {
            Entries::Tiny(list) => Iter::Tiny(list.iter()),
            Entries::BTree(map) => Iter::BTree(map.iter()),
            Entries::Persistent(map) => Iter::Persistent(map.iter()),
        }
    }

    /// Put a new key at `at` of a tiny map, which past [`TINY`] entries
    /// becomes a B-tree.
    fn tiny_insert(&mut self, at: usize, key: Key, entry: Entry) {
        let Entries::Tiny(list) = self else { unreachable!() };
        if list.len() < TINY {
            list.insert(at, (key, entry));
        } else {
            let mut map: BTreeMap<Key, Entry> = std::mem::take(list).into_iter().collect();
            map.insert(key, entry);
            *self = Entries::BTree(map);
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
    Tiny(std::slice::Iter<'a, (Key, Entry)>),
    BTree(btree_map::Iter<'a, Key, Entry>),
    Persistent(pmap::Iter<'a, Key, Entry>),
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Key, &'a Entry);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Tiny(iter) => iter.next().map(|(key, entry)| (key, entry)),
            Iter::BTree(iter) => iter.next(),
            Iter::Persistent(iter) => iter.next(),
        }
    }
}

fn tiny_seek(list: &[(Key, Entry)], key: &Key) -> Result<usize, usize> {
    list.binary_search_by(|(k, _)| k.cmp(key))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::rc::Rc;
    use std::time::Instant;

    fn spot(x: &mut u64, n: usize) -> usize {
        *x ^= *x << 13;
        *x ^= *x >> 7;
        *x ^= *x << 17;
        (((*x >> 32) * n as u64) >> 32) as usize
    }

    fn median(mut work: impl FnMut() -> usize) -> f64 {
        black_box(work());
        let mut t: Vec<f64> = (0..7)
            .map(|_| {
                let s = Instant::now();
                black_box(work());
                s.elapsed().as_nanos() as f64
            })
            .collect();
        t.sort_by(f64::total_cmp);
        t[3]
    }

    #[test]
    fn large_maps_turn_persistent_in_place() {
        // Built entry by entry, as a builder does: still the standard map.
        let mut map = Entries::default();
        for i in (0..1_000i64).rev() {
            map.upsert(Key::Small(i * 7 % 1_000), (i as Word, 0));
        }
        assert!(!map.is_persistent() && map.len() == 1_000);
        let before: Vec<(Key, Entry)> = map.iter().map(|(k, e)| (k.clone(), *e)).collect();
        map.make_persistent();
        assert!(map.is_persistent());
        assert!(map.iter().map(|(k, e)| (k.clone(), *e)).eq(before));
        // A small map stays standard.
        let mut small = Entries::default();
        for i in 0..SMALL as i64 {
            small.upsert(Key::Small(i), (i as Word, 0));
        }
        small.make_persistent();
        assert!(!small.is_persistent());
        // A small left part merged into a persistent one keeps its key words.
        let mut left = Entries::default();
        left.upsert(Key::Small(3), (77, 1));
        left.merge(&map);
        assert!(left.is_persistent() && left.len() == 1_000);
        assert_eq!(left.get(&Key::Small(3)).map(|e| e.0), Some(77));
    }

    #[test]
    fn tiny_maps_keep_key_order_and_grow_into_b_trees() {
        let mut map = Entries::default();
        for i in (0..TINY as i64).rev() {
            assert!(map.upsert(Key::Small(i), (i as Word, 0)));
        }
        assert!(matches!(map, Entries::Tiny(_)));
        // A new value for an equal key keeps the stored key word.
        assert!(!map.upsert(Key::Small(3), (99, 7)));
        assert_eq!(map.get(&Key::Small(3)), Some(&(3, 7)));
        map.remove(&Key::Small(0));
        assert!(map.get(&Key::Small(0)).is_none() && map.len() == TINY - 1);
        map.insert(Key::Small(0), (0, 0));
        assert!(map.keys().cloned().eq((0..TINY as i64).map(Key::Small)));
        map.upsert(Key::Small(TINY as i64), (0, 0));
        assert!(matches!(map, Entries::BTree(_)) && map.len() == TINY + 1);
        assert!(map.keys().cloned().eq((0..=TINY as i64).map(Key::Small)));
        // A literal that holds more than a tiny map starts as the B-tree.
        assert!(matches!(Entries::with_capacity(TINY), Entries::Tiny(_)));
        assert!(matches!(Entries::with_capacity(TINY + 1), Entries::BTree(_)));
    }

    /// Both kinds of entries with Tomato's own keys (#252): run with
    /// `cargo test --release -p sparrow-core maps::tests::costs -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn costs() {
        let kinds = |persistent: bool| if persistent { Entries::Persistent(PMap::new()) } else { Entries::BTree(BTreeMap::new()) };
        // The standard map as `OKKAMA_MAPS=btree` keeps it: never persistent.
        let put = |m: &mut Entries, key: Key, entry: Entry| match m {
            Entries::BTree(map) => drop(map.insert(key, entry)),
            m => drop(m.upsert(key, entry)),
        };
        for n in [1_000usize, 16_000, 100_000] {
            let names: Vec<Key> = (0..n).map(|i| Key::text(Rc::from(format!("key-{i}")))).collect();
            let mut line = format!("n={n:<7} key {}", std::mem::size_of::<Key>());
            for (label, persistent) in [("btree", false), ("pmap", true)] {
                let mut ints = kinds(persistent);
                let mut strs = kinds(persistent);
                for (i, name) in names.iter().enumerate() {
                    put(&mut ints, Key::Small(i as i64 * 2), (i as Word, i as Word));
                    put(&mut strs, name.clone(), (i as Word, i as Word));
                }
                let reads = 1_000_000;
                let get = median(|| {
                    let mut x = 1;
                    (0..reads).filter(|_| ints.get(&Key::Small(spot(&mut x, n) as i64 * 2)).is_some()).count()
                }) / reads as f64;
                let miss = median(|| {
                    let mut x = 1;
                    (0..reads).filter(|_| ints.get(&Key::Small(spot(&mut x, n) as i64 * 2 + 1)).is_some()).count()
                }) / reads as f64;
                let text = median(|| {
                    let mut x = 1;
                    (0..reads).filter(|_| strs.get(&names[spot(&mut x, n)]).is_some()).count()
                }) / reads as f64;
                let packed = if persistent {
                    let packed = Entries::Persistent(PMap::from_sorted(strs.iter().map(|(k, v)| (k.clone(), *v))));
                    median(|| {
                        let mut x = 1;
                        (0..reads).filter(|_| packed.get(&names[spot(&mut x, n)]).is_some()).count()
                    }) / reads as f64
                } else {
                    0.0
                };
                let build = median(|| {
                    let mut m = kinds(persistent);
                    for i in 0..n {
                        put(&mut m, Key::Small(i as i64 * 2), (i as Word, i as Word));
                    }
                    m.len()
                }) / n as f64;
                let churn = median(|| {
                    (0..200_000usize)
                        .map(|j| {
                            let mut m = if persistent { Entries::default() } else { kinds(false) };
                            for d in 0..3 {
                                put(&mut m, Key::Small((j + d) as i64), (d as Word, d as Word));
                            }
                            m.get(&Key::Small(j as i64 + 1)).map_or(0, |e| e.1 as usize)
                        })
                        .sum()
                }) / 200_000.0;
                line += &format!("  {label}: get {get:5.1} miss {miss:5.1} str {text:5.1} packed {packed:5.1} build {build:5.1} churn {churn:5.1}");
            }
            println!("{line}");
        }
    }
}
