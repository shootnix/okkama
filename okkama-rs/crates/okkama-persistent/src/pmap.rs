//! A persistent ordered map (#252–#254): a B+ tree whose nodes versions share.
//!
//! Entries live in leaves of at most [`MAX`] pairs, sorted by key; a branch
//! holds, for each child, the smallest key below it. A version is a root
//! and a length. Writes copy on write: a node only this version holds is
//! changed in place (a map being built never copies), a node another version
//! still holds is copied, so a write copies one path from the root and the
//! older versions keep their entries. Lookups walk ⌈log₃₂ n⌉ nodes, as in a
//! standard B-tree.
//!
//! The memory of the nodes is counted per thread ([`memory`]): what was
//! allocated in all and what is live now, exactly, whatever the nodes are
//! shared by. An embedding collector can use it instead of estimating what
//! each version owns.

use std::cell::Cell;
use std::rc::Rc;

/// Most entries of a leaf and most children of a branch.
pub const MAX: usize = 32;
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

#[derive(Debug)]
enum Node<K, V> {
    Leaf(Vec<(K, V)>),
    Branch(Vec<(K, Rc<Counted<K, V>>)>),
}

/// A node and the bytes it is counted for.
#[derive(Debug)]
struct Counted<K, V> {
    node: Node<K, V>,
}

impl<K, V> Node<K, V> {
    fn bytes(&self) -> usize {
        std::mem::size_of::<Counted<K, V>>()
            + match self {
                Node::Leaf(entries) => entries.capacity() * std::mem::size_of::<(K, V)>(),
                Node::Branch(children) => children.capacity() * std::mem::size_of::<(K, Rc<Counted<K, V>>)>(),
            }
    }
    fn len(&self) -> usize {
        match self {
            Node::Leaf(entries) => entries.len(),
            Node::Branch(children) => children.len(),
        }
    }
}

impl<K: Clone, V: Clone> Node<K, V> {
    fn first_key(&self) -> &K {
        match self {
            Node::Leaf(entries) => &entries[0].0,
            Node::Branch(children) => &children[0].0,
        }
    }
}

impl<K, V> Counted<K, V> {
    fn new(node: Node<K, V>) -> Rc<Self> {
        grew(node.bytes());
        Rc::new(Counted { node })
    }
}

impl<K: Clone, V: Clone> Clone for Counted<K, V> {
    fn clone(&self) -> Self {
        let node = match &self.node {
            Node::Leaf(entries) => Node::Leaf(entries.clone()),
            Node::Branch(children) => Node::Branch(children.clone()),
        };
        grew(node.bytes());
        Counted { node }
    }
}

impl<K, V> Drop for Counted<K, V> {
    fn drop(&mut self) {
        shrank(self.node.bytes());
    }
}

/// Change the node behind `rc`, copying it first when another version holds
/// it, and count a change of its capacity.
fn edit<K: Clone, V: Clone, R>(rc: &mut Rc<Counted<K, V>>, change: impl FnOnce(&mut Node<K, V>) -> R) -> R {
    let counted = Rc::make_mut(rc);
    let before = counted.node.bytes();
    let result = change(&mut counted.node);
    let after = counted.node.bytes();
    if after > before {
        grew(after - before);
    } else {
        shrank(before - after);
    }
    result
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

impl<K: Ord + Clone, V: Clone> Default for PMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + Clone, V: Clone> PMap<K, V> {
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
            let leaf: Vec<(K, V)> = rest.by_ref().take(BULK).collect();
            level.push((leaf[0].0.clone(), Counted::new(Node::Leaf(leaf))));
        }
        while level.len() > 1 {
            let mut parents = Vec::with_capacity(level.len().div_ceil(BULK));
            let mut children = level.into_iter().peekable();
            while children.peek().is_some() {
                let group: Vec<_> = children.by_ref().take(BULK).collect();
                parents.push((group[0].0.clone(), Counted::new(Node::Branch(group))));
            }
            level = parents;
        }
        Self { root: level.pop().map(|(_, node)| node), len }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let mut node = &self.root.as_ref()?.node;
        loop {
            match node {
                Node::Leaf(entries) => {
                    return entries.binary_search_by(|e| e.0.cmp(key)).ok().map(|i| &entries[i].1);
                }
                Node::Branch(children) => {
                    let i = children.partition_point(|c| c.0 <= *key);
                    if i == 0 {
                        return None;
                    }
                    node = &children[i - 1].1.node;
                }
            }
        }
    }

    /// The stored key equal to `key` and its value.
    pub fn get_key_value(&self, key: &K) -> Option<(&K, &V)> {
        let mut node = &self.root.as_ref()?.node;
        loop {
            match node {
                Node::Leaf(entries) => {
                    return entries.binary_search_by(|e| e.0.cmp(key)).ok().map(|i| (&entries[i].0, &entries[i].1));
                }
                Node::Branch(children) => {
                    let i = children.partition_point(|c| c.0 <= *key);
                    if i == 0 {
                        return None;
                    }
                    node = &children[i - 1].1.node;
                }
            }
        }
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
            self.root = Some(Counted::new(Node::Leaf(vec![(key, make())])));
            self.len = 1;
            return true;
        };
        let (fresh, split) = upsert_in(root, key, make, modify);
        if let Some(split) = split {
            let left = self.root.take().expect("root");
            let first = left.node.first_key().clone();
            self.root = Some(Counted::new(Node::Branch(vec![(first, left), split])));
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
                Node::Leaf(entries) if entries.is_empty() => self.root = None,
                Node::Branch(children) if children.len() == 1 => {
                    let only = children[0].1.clone();
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
        let mut iter = Iter { stack: Vec::new(), left: self.len };
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

    /// Depth, and whether every node is within bounds, sorted, and labelled
    /// with its smallest key (for tests).
    pub fn check(&self) -> Result<usize, String> {
        fn walk<K: Ord + Clone, V: Clone>(node: &Node<K, V>, root: bool) -> Result<(usize, usize), String> {
            if node.len() > MAX || (!root && node.len() == 0) {
                return Err(format!("a node of {} entries", node.len()));
            }
            match node {
                Node::Leaf(entries) => {
                    if entries.windows(2).any(|w| w[0].0 >= w[1].0) {
                        return Err("a leaf out of order".into());
                    }
                    Ok((1, entries.len()))
                }
                Node::Branch(children) => {
                    let mut depth = None;
                    let mut count = 0;
                    for (i, (key, child)) in children.iter().enumerate() {
                        if child.node.first_key() != key {
                            return Err("a branch key that is not its child's smallest".into());
                        }
                        if i > 0 && children[i - 1].0 >= *key {
                            return Err("a branch out of order".into());
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

fn upsert_in<K: Ord + Clone, V: Clone>(
    rc: &mut Rc<Counted<K, V>>,
    key: K,
    make: impl FnOnce() -> V,
    modify: impl FnOnce(&mut V),
) -> (bool, Option<(K, Rc<Counted<K, V>>)>) {
    edit(rc, |node| match node {
        Node::Leaf(entries) => match entries.binary_search_by(|e| e.0.cmp(&key)) {
            Ok(i) => {
                modify(&mut entries[i].1);
                (false, None)
            }
            Err(i) => {
                entries.insert(i, (key, make()));
                (true, split_leaf(entries))
            }
        },
        Node::Branch(children) => {
            let i = children.partition_point(|c| c.0 <= key).saturating_sub(1);
            if key < children[i].0 {
                children[i].0 = key.clone();
            }
            let (fresh, split) = upsert_in(&mut children[i].1, key, make, modify);
            if let Some(split) = split {
                children.insert(i + 1, split);
            }
            (fresh, split_branch(children))
        }
    })
}

fn split_leaf<K: Clone, V: Clone>(entries: &mut Vec<(K, V)>) -> Option<(K, Rc<Counted<K, V>>)> {
    (entries.len() > MAX).then(|| {
        let right = entries.split_off(entries.len() / 2);
        entries.shrink_to(MAX);
        (right[0].0.clone(), Counted::new(Node::Leaf(right)))
    })
}

#[allow(clippy::type_complexity)]
fn split_branch<K: Clone, V: Clone>(children: &mut Vec<(K, Rc<Counted<K, V>>)>) -> Option<(K, Rc<Counted<K, V>>)> {
    (children.len() > MAX).then(|| {
        let right = children.split_off(children.len() / 2);
        children.shrink_to(MAX);
        (right[0].0.clone(), Counted::new(Node::Branch(right)))
    })
}

fn remove_in<K: Ord + Clone, V: Clone>(rc: &mut Rc<Counted<K, V>>, key: &K) -> Option<(K, V)> {
    // Find before copying: a missing key changes no node.
    if !contains_in(&rc.node, key) {
        return None;
    }
    edit(rc, |node| match node {
        Node::Leaf(entries) => {
            let i = entries.binary_search_by(|e| e.0.cmp(key)).ok()?;
            Some(entries.remove(i))
        }
        Node::Branch(children) => {
            let i = children.partition_point(|c| c.0 <= *key).checked_sub(1)?;
            let removed = remove_in(&mut children[i].1, key)?;
            if children[i].1.node.len() == 0 {
                children.remove(i);
            } else {
                children[i].0 = children[i].1.node.first_key().clone();
                if children[i].1.node.len() < MIN && children.len() > 1 {
                    rebalance(children, i);
                }
            }
            Some(removed)
        }
    })
}

fn contains_in<K: Ord + Clone, V: Clone>(mut node: &Node<K, V>, key: &K) -> bool {
    loop {
        match node {
            Node::Leaf(entries) => return entries.binary_search_by(|e| e.0.cmp(key)).is_ok(),
            Node::Branch(children) => {
                let i = children.partition_point(|c| c.0 <= *key);
                if i == 0 {
                    return false;
                }
                node = &children[i - 1].1.node;
            }
        }
    }
}

/// Child `i` fell below [`MIN`]: merge it with a neighbour, or share their
/// entries evenly when together they would not fit one node.
fn rebalance<K: Ord + Clone, V: Clone>(children: &mut Vec<(K, Rc<Counted<K, V>>)>, i: usize) {
    let (a, b) = if i + 1 < children.len() { (i, i + 1) } else { (i - 1, i) };
    let right = children.remove(b).1;
    let mut left = children[a].1.clone();
    edit(&mut left, |node| match (node, &right.node) {
        (Node::Leaf(l), Node::Leaf(r)) => l.extend(r.iter().cloned()),
        (Node::Branch(l), Node::Branch(r)) => l.extend(r.iter().cloned()),
        _ => unreachable!("siblings at one depth"),
    });
    drop(right);
    let extra = edit(&mut left, |node| match node {
        Node::Leaf(entries) if entries.len() > MAX => split_leaf(entries),
        Node::Branch(children) if children.len() > MAX => split_branch(children),
        _ => None,
    });
    children[a] = (left.node.first_key().clone(), left);
    if let Some(extra) = extra {
        children.insert(a + 1, extra);
    }
}

/// In-order iterator over a map version.
#[derive(Debug)]
pub struct Iter<'a, K, V> {
    stack: Vec<(&'a Node<K, V>, usize)>,
    left: usize,
}

impl<'a, K, V> Iter<'a, K, V> {
    fn descend(&mut self, mut node: &'a Node<K, V>) {
        loop {
            self.stack.push((node, 0));
            match node {
                Node::Branch(children) if !children.is_empty() => node = &children[0].1.node,
                _ => return,
            }
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (node, at) = self.stack.last_mut()?;
            match node {
                Node::Leaf(entries) => {
                    if *at < entries.len() {
                        let entry = &entries[*at];
                        *at += 1;
                        self.left -= 1;
                        return Some((&entry.0, &entry.1));
                    }
                    self.stack.pop();
                }
                Node::Branch(children) => {
                    // The child at `at` was walked; go to the next one.
                    *at += 1;
                    if *at < children.len() {
                        let child = &children[*at].1.node;
                        self.descend(child);
                    } else {
                        self.stack.pop();
                    }
                }
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
                assert_eq!(map.remove(&key).map(|(k, v)| (k, v)), model.remove(&key).map(|v| (key, v)));
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
}
