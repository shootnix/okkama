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
//! A persistent map is cloned in O(1); a write copies one path from the
//! root, or changes nodes in place while no other version holds them (a map
//! being built). Its nodes' memory, with what their keys own, is counted
//! exactly per thread (`okkama_persistent::pmap::memory`), which the heap
//! reads instead of an estimate per map, so versions that share nodes are
//! not charged twice; and the collector traces a shared node once per
//! collection ([`Entries::trace`]).

use std::borrow::Borrow;
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

/// What a standard map is searched by (#343): a stored key, or the text of a
/// string being looked up, which needs no key built for it. Its order is
/// the order of [`Key`] exactly, as `Borrow` requires.
pub trait Query {
    fn probe(&self) -> Probe<'_>;
}

#[derive(Debug)]
pub enum Probe<'a> {
    Key(&'a Key),
    /// A text and its [`Key::prefix`].
    Text(u64, &'a str),
}

impl Query for Key {
    #[inline]
    fn probe(&self) -> Probe<'_> {
        Probe::Key(self)
    }
}

struct Text<'a>(u64, &'a str);

impl Query for Text<'_> {
    #[inline]
    fn probe(&self) -> Probe<'_> {
        Probe::Text(self.0, self.1)
    }
}

impl<'a> Borrow<dyn Query + 'a> for Key {
    fn borrow(&self) -> &(dyn Query + 'a) {
        self
    }
}

impl Ord for dyn Query + '_ {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.probe(), other.probe()) {
            (Probe::Key(a), Probe::Key(b)) => a.cmp(b),
            (Probe::Key(a), Probe::Text(prefix, text)) => a.cmp_text(prefix, text),
            (Probe::Text(prefix, text), Probe::Key(b)) => b.cmp_text(prefix, text).reverse(),
            (Probe::Text(a, x), Probe::Text(b, y)) => a.cmp(&b).then_with(|| x.cmp(y)),
        }
    }
}

impl PartialOrd for dyn Query + '_ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for dyn Query + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for dyn Query + '_ {}

#[derive(Debug, Clone)]
pub enum Entries {
    BTree(BTreeMap<Key, Entry>),
    Persistent(PMap<Key, Entry>),
}

impl Default for Entries {
    fn default() -> Self {
        Entries::BTree(BTreeMap::new())
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

    /// The entry of the string key `text`, found without making the key
    /// (#343): no allocation, no copy of the text.
    pub fn get_text(&self, text: &str) -> Option<&Entry> {
        let prefix = Key::prefix(text);
        match self {
            Entries::BTree(map) => map.get(&Text(prefix, text) as &dyn Query),
            Entries::Persistent(map) => map.get_by(|stored| stored.cmp_text(prefix, text)),
        }
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
        match (&mut *self, right) {
            (Entries::Persistent(left), Entries::Persistent(right)) => *left = merged(left, right),
            (Entries::BTree(left), Entries::Persistent(right)) => {
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

    /// A string looked up by its text (#343) finds what its key finds, in
    /// both kinds of entries, beside keys of every other kind; and the
    /// order a lookup by text sees is the order of [`Key`].
    #[test]
    fn borrowed_text_lookups_find_what_owned_keys_find() {
        let long = "x".repeat(1_000);
        let texts: Vec<String> = [
            "", "a", "abcdefg", "abcdefgh", "abcdefghi", "abcdefghz", "abc\0def", "abc\0", "ключ", "日本語のテキスト",
        ]
        .iter()
        .map(|text| text.to_string())
        .chain([long.clone(), format!("{long}y")])
        .collect();
        let misses = ["abcdefgh\0", "abcdefgi", "abcdefg\0", "ключи", "zzz", "\0", &long[1..], &format!("{long}x")]
            .map(str::to_string);
        let rat = |n: i64, d: i64| {
            let big = okkama_runtime_core::IntValue::from_i64;
            Key::Rat(Rc::new(okkama_runtime_core::rat::Rat::new(&big(n), &big(d)).unwrap()))
        };
        let others = vec![
            Key::Small(-5),
            Key::Small(0),
            Key::Small(7),
            Key::Int(num_bigint::BigInt::from(1u8) << 80usize),
            Key::Int(-(num_bigint::BigInt::from(1u8) << 80usize)),
            Key::Float(-0.0),
            Key::Float(2.5),
            Key::Raw(3),
            rat(1, 3),
            Key::Object(1, Rc::from(vec![Key::text(Rc::from("abcdefgh")), Key::Small(1)])),
            Key::Object(1, Rc::from(vec![Key::Object(2, Rc::from(vec![Key::Small(2)]))])),
        ];
        for padding in [0usize, SMALL * 3] {
            let mut keys: Vec<Key> = texts.iter().map(|text| Key::text(Rc::from(text.as_str()))).collect();
            keys.extend(others.iter().cloned());
            keys.extend((0..padding).map(|i| Key::text(Rc::from(format!("key-{i:05}")))));
            for persistent in [false, true] {
                let mut map = if persistent { Entries::Persistent(PMap::new()) } else { Entries::BTree(BTreeMap::new()) };
                for (i, key) in keys.iter().enumerate() {
                    map.insert(key.clone(), (i as Word, i as Word));
                }
                let queries = texts.iter().chain(&misses).cloned().chain((0..padding).step_by(17).map(|i| format!("key-{i:05}")));
                for query in queries {
                    let owned = map.get(&Key::text(Rc::from(query.as_str()))).copied();
                    assert_eq!(map.get_text(&query).copied(), owned, "{query:?} persistent={persistent} padding={padding}");
                }
                assert!(map.get_text("ключ").is_some());
                assert_eq!(map.get_text("ключи"), None);
            }
            for a in &keys {
                for b in &keys {
                    assert_eq!(<dyn Query>::cmp(a, b), a.cmp(b), "{a:?} {b:?}");
                }
                for text in texts.iter().chain(&misses) {
                    let owned = a.cmp(&Key::text(Rc::from(text.as_str())));
                    assert_eq!(a.cmp_text(Key::prefix(text), text), owned, "{a:?} {text:?}");
                    let probe = Text(Key::prefix(text), text);
                    assert_eq!(<dyn Query>::cmp(a, &probe), owned, "{a:?} {text:?}");
                    assert_eq!(<dyn Query>::cmp(&probe, a), owned.reverse(), "{a:?} {text:?}");
                }
            }
        }
        // Empty entries find nothing.
        assert_eq!(Entries::default().get_text("a"), None);
        assert_eq!(Entries::Persistent(PMap::new()).get_text(""), None);
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

    /// A string key looked up by its text against one made for the lookup,
    /// as Tomato did before #343: run with `cargo test --release -p
    /// sparrow-core maps::tests::text_costs -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn text_costs() {
        // A label and the key of entry `i`.
        type Shape = (&'static str, fn(usize) -> String);
        let shapes: [Shape; 3] = [
            ("short", |i| format!("k{i}")),
            ("prefix", |i| format!("prefix--{i}")),
            ("long", |i| format!("{}{i}", "a long key that shares its first fifty-six bytes .......")),
        ];
        for (shape, name) in shapes {
            for n in [1usize, 8, 16, 64, 1_000, 16_000, 100_000] {
                let texts: Vec<String> = (0..n).map(name).collect();
                let absent: Vec<String> = (0..n).map(|i| name(i) + "~").collect();
                let mut line = format!("{shape:<6} n={n:<7}");
                for (label, persistent) in [("btree", false), ("pmap", true)] {
                    if persistent && n <= SMALL && n > 1 {
                        line += "  pmap: (small maps stay standard)";
                        continue;
                    }
                    let mut map = if persistent { Entries::Persistent(PMap::new()) } else { Entries::BTree(BTreeMap::new()) };
                    for (i, text) in texts.iter().enumerate() {
                        map.insert(Key::text(Rc::from(text.as_str())), (i as Word, i as Word));
                    }
                    let reads = 200_000;
                    let time = |queries: &[String], borrowed: bool| {
                        median(|| {
                            let mut x = 1;
                            (0..reads)
                                .filter(|_| {
                                    let query = queries[spot(&mut x, n)].as_str();
                                    if borrowed {
                                        map.get_text(black_box(query)).is_some()
                                    } else {
                                        map.get(&Key::text(Rc::from(black_box(query)))).is_some()
                                    }
                                })
                                .count()
                        }) / reads as f64
                    };
                    let (owned, borrowed) = (time(&texts, false), time(&texts, true));
                    let (owned_miss, borrowed_miss) = (time(&absent, false), time(&absent, true));
                    line += &format!(
                        "  {label}: hit {owned:5.1} -> {borrowed:5.1}  miss {owned_miss:5.1} -> {borrowed_miss:5.1}"
                    );
                }
                println!("{line}");
            }
        }
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
