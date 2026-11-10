//! A persistent ordered map (#252–#254): a B+ tree whose nodes versions share.
//!
//! Entries live in leaves of at most [`MAX`] pairs, sorted by key; a branch
//! holds, for each child, the smallest key below it. A version is a root
//! and a length. Writes copy on write: a node only this version holds is
//! changed in place (a map being built never copies), a node another version
//! still holds is copied, so a write copies one path from the root and the
//! older versions keep their entries. Lookups walk ⌈log₁₆ n⌉ nodes, as in a
//! standard B-tree.
//!
//! The memory of the nodes is counted per thread ([`memory`]): what was
//! allocated in all and what is live now, exactly, whatever the nodes are
//! shared by, with what the keys of a leaf own besides ([`Weigh`]). An
//! embedding collector can use it instead of estimating what each version
//! owns, and trace the versions it holds visiting each shared node once
//! ([`PMap::for_each_unvisited`]).

use std::cell::Cell;
use std::rc::Rc;

use crate::slots::Slots;

/// Most entries of a leaf and most children of a branch.
pub const MAX: usize = 16;
/// Room in a node: one over [`MAX`] until an insert splits it.
const CAP: usize = MAX + 1;
/// A node with fewer after a removal is merged with a neighbour.
const MIN: usize = MAX / 4;
/// How full a bulk-built node is: room for inserts without a split.
const BULK: usize = MAX * 3 / 4;

thread_local! {
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
}

/// Bytes of map nodes allocated by this thread since it started, and live now.
pub fn memory() -> (usize, usize) {
    (ALLOCATED.with(Cell::get), LIVE.with(Cell::get))
}

fn grew(bytes: usize) {
    ALLOCATED.with(|a| a.set(a.get() + bytes));
    LIVE.with(|l| l.set(l.get() + bytes));
}

fn shrank(bytes: usize) {
    LIVE.with(|l| l.set(l.get().saturating_sub(bytes)));
}

/// Memory a key owns outside the node that holds it (a text, a big
/// number), counted with the node. The keys of branches are copies of keys
/// in leaves and are not counted again.
pub trait Weigh {
    fn weight(&self) -> usize {
        0
    }
}

macro_rules! weightless {
    ($($ty:ty)*) => {$(impl Weigh for $ty {})*};
}
weightless!(u8 u16 u32 u64 usize i8 i16 i32 i64 isize char bool);

#[derive(Debug)]
enum Node<K, V> {
    /// Keys and values apart and inline, as in the standard B-tree: a node
    /// is one allocation, and a search touches only its keys.
    Leaf { keys: Slots<K, CAP>, vals: Slots<V, CAP> },
    /// The smallest key below each child.
    Branch { keys: Slots<K, CAP>, kids: Slots<Rc<Counted<K, V>>, CAP> },
}

/// A node, the bytes it is counted for, and the last pass that visited it.
#[derive(Debug)]
struct Counted<K, V> {
    node: Node<K, V>,
    bytes: usize,
    pass: Cell<u32>,
}

impl<K, V> Node<K, V> {
    /// The node's allocation (the node and the two counts of its `Rc`), and
    /// what the keys of a leaf own.
    fn bytes(&self) -> usize
    where
        K: Weigh,
    {
        let keys = match self {
            Node::Leaf { keys, .. } => keys.iter().map(Weigh::weight).sum(),
            Node::Branch { .. } => 0,
        };
        std::mem::size_of::<Counted<K, V>>() + 2 * std::mem::size_of::<usize>() + keys
    }
    fn len(&self) -> usize {
        match self {
            Node::Leaf { keys, .. } | Node::Branch { keys, .. } => keys.len(),
        }
    }
    fn first_key(&self) -> &K {
        match self {
            Node::Leaf { keys, .. } | Node::Branch { keys, .. } => &keys[0],
        }
    }
}

impl<K: Weigh, V> Counted<K, V> {
    fn new(node: Node<K, V>) -> Rc<Self> {
        let bytes = node.bytes();
        grew(bytes);
        Rc::new(Counted { node, bytes, pass: Cell::new(0) })
    }

    /// Count the node again after a change in place.
    fn recount(&mut self) {
        let bytes = self.node.bytes();
        if bytes > self.bytes {
            grew(bytes - self.bytes);
        } else {
            shrank(self.bytes - bytes);
        }
        self.bytes = bytes;
    }
}

impl<K: Clone, V: Clone> Clone for Counted<K, V> {
    fn clone(&self) -> Self {
        let node = match &self.node {
            Node::Leaf { keys, vals } => Node::Leaf { keys: keys.clone(), vals: vals.clone() },
            Node::Branch { keys, kids } => Node::Branch { keys: keys.clone(), kids: kids.clone() },
        };
        grew(self.bytes);
        Counted { node, bytes: self.bytes, pass: Cell::new(0) }
    }
}

impl<K, V> Drop for Counted<K, V> {
    fn drop(&mut self) {
        shrank(self.bytes);
    }
}

/// Change the node behind `rc`, copying it first when another version holds
/// it.
fn edit<K: Clone + Weigh, V: Clone, R>(rc: &mut Rc<Counted<K, V>>, change: impl FnOnce(&mut Node<K, V>) -> R) -> R {
    let counted = Rc::make_mut(rc);
    let result = change(&mut counted.node);
    counted.recount();
    result
}

/// How many of the sorted `keys` are at most `key`: a scan from the left,
/// as in the standard B-tree. The keys of a node are adjacent, so the loads
/// of a scan go out together where a halving would wait on each one.
#[inline]
fn count_le<K: Ord>(keys: &[K], key: &K) -> usize {
    keys.iter().position(|k| k > key).unwrap_or(keys.len())
}

/// The child of a branch whose range holds `key`, if any.
#[inline]
fn child_of<K: Ord>(keys: &[K], key: &K) -> Option<usize> {
    count_le(keys, key).checked_sub(1)
}

/// The index of `key` in sorted `keys`, or where it would go.
#[inline]
fn find<K: Ord>(keys: &[K], key: &K) -> Result<usize, usize> {
    let at = count_le(keys, key);
    if at > 0 && keys[at - 1] == *key { Ok(at - 1) } else { Err(at) }
}

/// An immutable ordered map version; cloning it is O(1).
#[derive(Debug)]
pub struct PMap<K, V> {
    root: Option<Rc<Counted<K, V>>>,
    len: usize,
}

impl<K, V> Clone for PMap<K, V> {
    fn clone(&self) -> Self {
        Self { root: self.root.clone(), len: self.len }
    }
}

impl<K: Ord + Clone + Weigh, V: Clone> Default for PMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + Clone + Weigh, V: Clone> PMap<K, V> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// A map of `entries` already sorted by key with no key twice, built
    /// bottom-up with room in every node.
    pub fn from_sorted(entries: Vec<(K, V)>) -> Self {
        let len = entries.len();
        if len == 0 {
            return Self::new();
        }
        let mut level: Vec<(K, Rc<Counted<K, V>>)> = Vec::with_capacity(len.div_ceil(BULK));
        let mut rest = entries.into_iter().peekable();
        while rest.peek().is_some() {
            let (mut keys, mut vals) = (Slots::new(), Slots::new());
            for (key, value) in rest.by_ref().take(BULK) {
                keys.push(key);
                vals.push(value);
            }
            level.push((keys[0].clone(), Counted::new(Node::Leaf { keys, vals })));
        }
        while level.len() > 1 {
            let mut parents = Vec::with_capacity(level.len().div_ceil(BULK));
            let mut children = level.into_iter().peekable();
            while children.peek().is_some() {
                let (mut keys, mut kids) = (Slots::new(), Slots::new());
                for (key, kid) in children.by_ref().take(BULK) {
                    keys.push(key);
                    kids.push(kid);
                }
                parents.push((keys[0].clone(), Counted::new(Node::Branch { keys, kids })));
            }
            level = parents;
        }
        Self { root: level.pop().map(|(_, node)| node), len }
    }

    /// The stored key equal to `key` and its value.
    pub fn get_key_value(&self, key: &K) -> Option<(&K, &V)> {
        let mut node = &self.root.as_ref()?.node;
        loop {
            match node {
                Node::Leaf { keys, vals } => {
                    return find(keys, key).ok().map(|i| (&keys[i], &vals[i]));
                }
                Node::Branch { keys, kids } => node = &kids[child_of(keys, key)?].node,
            }
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.get_key_value(key).map(|(_, v)| v)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Insert `key` with `value`, or replace the value of an equal key and
    /// keep the stored key; the old value of a replaced entry.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let value = Cell::new(Some(value));
        let mut old = None;
        self.upsert(key, || value.take().expect("one value"), |v| old = Some(std::mem::replace(v, value.take().expect("one value"))));
        old
    }

    /// `modify` the value of an equal key, or insert `key` with `make()`;
    /// true when the key was new.
    pub fn upsert(&mut self, key: K, make: impl FnOnce() -> V, modify: impl FnOnce(&mut V)) -> bool {
        let Some(root) = self.root.as_mut() else {
            self.root = Some(Counted::new(Node::Leaf { keys: [key].into_iter().collect(), vals: [make()].into_iter().collect() }));
            self.len = 1;
            return true;
        };
        let (fresh, split) = upsert_in(root, key, make, modify);
        if let Some((right_key, right)) = split {
            let left = self.root.take().expect("root");
            let first = left.node.first_key().clone();
            self.root = Some(Counted::new(Node::Branch { keys: [first, right_key].into_iter().collect(), kids: [left, right].into_iter().collect() }));
        }
        if fresh {
            self.len += 1;
        }
        fresh
    }

    /// Remove an equal key; its stored key and value.
    pub fn remove(&mut self, key: &K) -> Option<(K, V)> {
        let root = self.root.as_mut()?;
        let removed = remove_in(root, key)?;
        self.len -= 1;
        // A root left empty, or a branch with one child, gives way.
        loop {
            let root = self.root.as_ref().expect("root");
            match &root.node {
                Node::Leaf { keys, .. } if keys.is_empty() => self.root = None,
                Node::Branch { kids, .. } if kids.len() == 1 => {
                    let only = kids[0].clone();
                    self.root = Some(only);
                    continue;
                }
                _ => {}
            }
            break;
        }
        Some(removed)
    }

    pub fn iter(&self) -> Iter<'_, K, V> {
        let mut iter = Iter { leaf: [].iter().zip([].iter()), stack: Vec::new(), left: self.len };
        if let Some(root) = &self.root {
            iter.descend(&root.node);
        }
        iter
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }

    /// Call `visit` on the entries of the nodes that no call with the same
    /// `pass` has visited yet, and mark them visited: a collector tracing
    /// every version it holds, with a new pass per collection, visits a node
    /// that versions share once. Pass 0 is never a pass.
    pub fn for_each_unvisited(&self, pass: u32, mut visit: impl FnMut(&K, &V)) {
        fn walk<K, V>(counted: &Counted<K, V>, pass: u32, visit: &mut impl FnMut(&K, &V)) {
            if counted.pass.replace(pass) == pass {
                return;
            }
            match &counted.node {
                Node::Leaf { keys, vals } => keys.iter().zip(vals.iter()).for_each(|(k, v)| visit(k, v)),
                Node::Branch { kids, .. } => kids.iter().for_each(|kid| walk(kid, pass, visit)),
            }
        }
        if let Some(root) = &self.root {
            walk(root, pass, &mut visit);
        }
    }

    /// Depth, and whether every node is within bounds, sorted, and labelled
    /// with its smallest key (for tests).
    pub fn check(&self) -> Result<usize, String> {
        fn walk<K: Ord + Clone + Weigh, V: Clone>(node: &Node<K, V>, root: bool) -> Result<(usize, usize), String> {
            if node.len() > MAX || (!root && node.len() == 0) {
                return Err(format!("a node of {} entries", node.len()));
            }
            match node {
                Node::Leaf { keys, vals } => {
                    if keys.len() != vals.len() || keys.windows(2).any(|w| w[0] >= w[1]) {
                        return Err("a leaf out of order or out of step".into());
                    }
                    Ok((1, keys.len()))
                }
                Node::Branch { keys, kids } => {
                    if keys.len() != kids.len() || keys.windows(2).any(|w| w[0] >= w[1]) {
                        return Err("a branch out of order or out of step".into());
                    }
                    let mut depth = None;
                    let mut count = 0;
                    for (key, child) in keys.iter().zip(kids.iter()) {
                        if child.node.first_key() != key {
                            return Err("a branch key that is not its child's smallest".into());
                        }
                        let (d, n) = walk(&child.node, false)?;
                        if depth.is_some_and(|depth| depth != d) {
                            return Err("leaves at different depths".into());
                        }
                        depth = Some(d);
                        count += n;
                    }
                    Ok((depth.unwrap_or(0) + 1, count))
                }
            }
        }
        match &self.root {
            None => Ok(0),
            Some(root) => {
                let (depth, count) = walk(&root.node, true)?;
                if count != self.len {
                    return Err(format!("{count} entries for a length of {}", self.len));
                }
                Ok(depth)
            }
        }
    }
}

type Split<K, V> = Option<(K, Rc<Counted<K, V>>)>;

fn upsert_in<K: Ord + Clone + Weigh, V: Clone>(
    rc: &mut Rc<Counted<K, V>>,
    key: K,
    make: impl FnOnce() -> V,
    modify: impl FnOnce(&mut V),
) -> (bool, Split<K, V>) {
    edit(rc, |node| match node {
        Node::Leaf { keys, vals } => match find(keys, &key) {
            Ok(i) => {
                modify(&mut vals[i]);
                (false, None)
            }
            Err(i) => {
                keys.insert(i, key);
                vals.insert(i, make());
                (true, split_leaf(keys, vals, i))
            }
        },
        Node::Branch { keys, kids } => {
            let i = child_of(keys, &key).unwrap_or(0);
            if key < keys[i] {
                keys[i] = key.clone();
            }
            let (fresh, split) = upsert_in(&mut kids[i], key, make, modify);
            if let Some((right_key, right)) = split {
                keys.insert(i + 1, right_key);
                kids.insert(i + 1, right);
            }
            (fresh, split_branch(keys, kids, i + 1))
        }
    })
}

/// Where to split a node that overflowed after an insert at `inserted`: in
/// the middle, but at the edge when the insert went to an edge, so that keys
/// arriving in order leave full nodes behind them instead of half-empty ones.
fn split_point(len: usize, inserted: usize) -> usize {
    if inserted + 1 == len {
        len - 1
    } else if inserted == 0 {
        1
    } else {
        len / 2
    }
}

fn split_leaf<K: Clone + Weigh, V: Clone>(keys: &mut Slots<K, CAP>, vals: &mut Slots<V, CAP>, inserted: usize) -> Split<K, V> {
    (keys.len() > MAX).then(|| {
        let at = split_point(keys.len(), inserted);
        let (right_keys, right_vals) = (keys.split_off(at), vals.split_off(at));
        (right_keys[0].clone(), Counted::new(Node::Leaf { keys: right_keys, vals: right_vals }))
    })
}

fn split_branch<K: Clone + Weigh, V: Clone>(
    keys: &mut Slots<K, CAP>,
    kids: &mut Slots<Rc<Counted<K, V>>, CAP>,
    inserted: usize,
) -> Split<K, V> {
    (keys.len() > MAX).then(|| {
        let at = split_point(keys.len(), inserted);
        let (right_keys, right_kids) = (keys.split_off(at), kids.split_off(at));
        (right_keys[0].clone(), Counted::new(Node::Branch { keys: right_keys, kids: right_kids }))
    })
}

fn remove_in<K: Ord + Clone + Weigh, V: Clone>(rc: &mut Rc<Counted<K, V>>, key: &K) -> Option<(K, V)> {
    // Find before copying: a missing key changes no node.
    if !contains_in(&rc.node, key) {
        return None;
    }
    edit(rc, |node| match node {
        Node::Leaf { keys, vals } => {
            let i = find(keys, key).ok()?;
            Some((keys.remove(i), vals.remove(i)))
        }
        Node::Branch { keys, kids } => {
            let i = child_of(keys, key)?;
            let removed = remove_in(&mut kids[i], key)?;
            if kids[i].node.len() == 0 {
                keys.remove(i);
                kids.remove(i);
            } else {
                keys[i] = kids[i].node.first_key().clone();
                if kids[i].node.len() < MIN && kids.len() > 1 {
                    rebalance(keys, kids, i);
                }
            }
            Some(removed)
        }
    })
}

fn contains_in<K: Ord, V>(mut node: &Node<K, V>, key: &K) -> bool {
    loop {
        match node {
            Node::Leaf { keys, .. } => return find(keys, key).is_ok(),
            Node::Branch { keys, kids } => match child_of(keys, key) {
                Some(i) => node = &kids[i].node,
                None => return false,
            },
        }
    }
}

/// Child `i` fell below [`MIN`]: merge it with a neighbour, or even out
/// their entries when together they would not fit one node.
fn rebalance<K: Ord + Clone + Weigh, V: Clone>(keys: &mut Slots<K, CAP>, kids: &mut Slots<Rc<Counted<K, V>>, CAP>, i: usize) {
    let (a, b) = if i + 1 < kids.len() { (i, i + 1) } else { (i - 1, i) };
    let total = kids[a].node.len() + kids[b].node.len();
    if total <= MAX {
        keys.remove(b);
        let right = kids.remove(b);
        edit(&mut kids[a], |node| match (node, &right.node) {
            (Node::Leaf { keys: lk, vals: lv }, Node::Leaf { keys: rk, vals: rv }) => {
                lk.extend_from_slice(rk);
                lv.extend_from_slice(rv);
            }
            (Node::Branch { keys: lk, kids: lc }, Node::Branch { keys: rk, kids: rc }) => {
                lk.extend_from_slice(rk);
                lc.extend_from_slice(rc);
            }
            _ => unreachable!("siblings at one depth"),
        });
    } else {
        let (front, back) = kids.split_at_mut(b);
        let (left, right) = (Rc::make_mut(&mut front[a]), Rc::make_mut(&mut back[0]));
        match (&mut left.node, &mut right.node) {
            (Node::Leaf { keys: lk, vals: lv }, Node::Leaf { keys: rk, vals: rv }) => {
                even(lk, rk, total / 2);
                even(lv, rv, total / 2);
            }
            (Node::Branch { keys: lk, kids: lc }, Node::Branch { keys: rk, kids: rc }) => {
                even(lk, rk, total / 2);
                even(lc, rc, total / 2);
            }
            _ => unreachable!("siblings at one depth"),
        }
        left.recount();
        right.recount();
        keys[b] = kids[b].node.first_key().clone();
    }
    keys[a] = kids[a].node.first_key().clone();
}

/// Move items between neighbours so that the left one holds `want`.
fn even<T>(left: &mut Slots<T, CAP>, right: &mut Slots<T, CAP>, want: usize) {
    if left.len() > want {
        let mut moved = left.split_off(want);
        moved.append(right);
        *right = moved;
    } else {
        let rest = right.split_off(want - left.len());
        left.append(right);
        *right = rest;
    }
}

type Kids<K, V> = [Rc<Counted<K, V>>];

/// In-order iterator over a map version: the entries of one leaf at a time,
/// and the branches above it with the child being walked in each.
#[derive(Debug)]
pub struct Iter<'a, K, V> {
    leaf: std::iter::Zip<std::slice::Iter<'a, K>, std::slice::Iter<'a, V>>,
    stack: Vec<(&'a Kids<K, V>, usize)>,
    left: usize,
}

impl<'a, K, V> Iter<'a, K, V> {
    fn descend(&mut self, mut node: &'a Node<K, V>) {
        loop {
            match node {
                Node::Branch { kids, .. } => {
                    self.stack.push((kids, 0));
                    node = &kids[0].node;
                }
                Node::Leaf { keys, vals } => {
                    self.leaf = keys.iter().zip(vals.iter());
                    return;
                }
            }
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(entry) = self.leaf.next() {
                self.left -= 1;
                return Some(entry);
            }
            // The leaf is done: the next child of the nearest branch that has one.
            loop {
                let (kids, at) = self.stack.last_mut()?;
                let kids: &'a Kids<K, V> = kids;
                *at += 1;
                if *at < kids.len() {
                    let child = &kids[*at].node;
                    self.descend(child);
                    break;
                }
                self.stack.pop();
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Rng(u64);
    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
    }

    fn same(map: &PMap<u64, u64>, model: &BTreeMap<u64, u64>) {
        map.check().unwrap();
        assert_eq!(map.len(), model.len());
        assert!(map.iter().map(|(k, v)| (*k, *v)).eq(model.iter().map(|(k, v)| (*k, *v))));
        for key in model.keys().step_by(7) {
            assert_eq!(map.get(key), model.get(key));
        }
    }

    #[test]
    fn versions_match_a_model_through_random_writes() {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        let mut versions: Vec<(PMap<u64, u64>, BTreeMap<u64, u64>)> = vec![(PMap::new(), BTreeMap::new())];
        for step in 0..30_000u64 {
            // Mostly the newest version (a chain), sometimes an older one (a branch).
            let pick = if rng.below(4) == 0 { rng.below(versions.len() as u64) as usize } else { versions.len() - 1 };
            let (mut map, mut model) = versions[pick].clone();
            let key = rng.below(4_000);
            if rng.below(3) == 0 {
                assert_eq!(map.remove(&key), model.remove(&key).map(|v| (key, v)));
            } else {
                assert_eq!(map.insert(key, step), model.insert(key, step));
            }
            if step % 997 == 0 {
                same(&map, &model);
            }
            versions.push((map, model));
        }
        for (map, model) in versions.iter().step_by(1_009) {
            same(map, model);
        }
        // Remove everything from the newest version, in a scattered order.
        let (mut map, model) = versions.last().unwrap().clone();
        let mut keys: Vec<u64> = model.keys().copied().collect();
        keys.sort_by_key(|k| k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for (i, key) in keys.iter().enumerate() {
            assert!(map.remove(key).is_some());
            if i % 101 == 0 {
                map.check().unwrap();
            }
        }
        assert!(map.is_empty() && map.root.is_none());
    }

    #[test]
    fn from_sorted_and_upsert_keep_the_stored_key() {
        for n in [0u64, 1, 24, 25, 600, 601, 20_000] {
            let map = PMap::from_sorted((0..n).map(|i| (i * 2, i)).collect());
            map.check().unwrap();
            assert_eq!(map.len() as u64, n);
            assert!(map.keys().copied().eq((0..n).map(|i| i * 2)));
            assert_eq!(map.get(&1), None);
        }
        // A key type whose equal keys differ in a payload: the stored one stays.
        #[derive(Clone, Debug)]
        struct Tagged(u64, &'static str);
        impl PartialEq for Tagged {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl Eq for Tagged {}
        impl PartialOrd for Tagged {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Tagged {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0.cmp(&other.0)
            }
        }
        impl Weigh for Tagged {}
        let mut map = PMap::new();
        map.insert(Tagged(1, "left"), 1);
        let fresh = map.upsert(Tagged(1, "right"), || 9, |v| *v = 2);
        assert!(!fresh);
        assert_eq!(map.get_key_value(&Tagged(1, "")).map(|(k, v)| (k.1, *v)), Some(("left", 2)));
    }

    #[test]
    fn node_memory_is_counted_exactly() {
        let (_, before) = memory();
        {
            let base = PMap::from_sorted((0..5_000u64).map(|i| (i, i)).collect());
            let mut versions = Vec::new();
            for k in 0..200u64 {
                let mut next = base.clone();
                next.insert(k * 25 + 1, k);
                versions.push(next);
            }
            let (_, live) = memory();
            // 200 versions share the base: far less than 200 copies of it.
            assert!(live - before < 30 * base.len() * std::mem::size_of::<(u64, u64)>(), "{}", live - before);
            drop(versions);
        }
        assert_eq!(memory().1, before, "every node released");
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct Text(String);
    impl Weigh for Text {
        fn weight(&self) -> usize {
            self.0.len()
        }
    }

    #[test]
    fn key_weights_are_counted_with_their_leaves() {
        let (_, before) = memory();
        {
            let mut map = PMap::new();
            for i in 0..2_000u64 {
                map.insert(Text(format!("{i:0>100}")), i);
            }
            let (_, built) = memory();
            assert!(built - before >= 2_000 * 100, "{}", built - before);
            let mut versions = vec![map.clone()];
            for i in 0..300u64 {
                let mut next = versions.last().unwrap().clone();
                next.remove(&Text(format!("{:0>100}", i * 5)));
                next.insert(Text(format!("x{i}")), i);
                versions.push(next);
            }
            versions.last().unwrap().check().unwrap();
        }
        assert_eq!(memory().1, before, "every node and its keys released");
    }

    #[test]
    fn a_pass_visits_a_shared_node_once() {
        let base = PMap::from_sorted((0..10_000u64).map(|i| (i * 2, i)).collect());
        let mut next = base.clone();
        next.insert(5_001, 0);
        let mut seen = std::collections::BTreeSet::new();
        let mut count = 0;
        base.for_each_unvisited(1, |k, _| {
            seen.insert(*k);
            count += 1;
        });
        assert_eq!(count, base.len());
        next.for_each_unvisited(1, |k, _| {
            seen.insert(*k);
            count += 1;
        });
        // Only the copied leaf of the new version is new to the pass.
        assert!(count - base.len() <= MAX, "{}", count - base.len());
        assert!(next.keys().all(|k| seen.contains(k)));
        // A new pass visits everything again.
        let mut again = 0;
        next.for_each_unvisited(2, |_, _| again += 1);
        assert_eq!(again, next.len());
    }
}
