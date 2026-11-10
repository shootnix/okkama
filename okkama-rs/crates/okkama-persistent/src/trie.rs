//! A persistent vector: a 32-way trie of leaves plus a tail of the last
//! 1–32 elements (Bagwell 2000; Hickey's `PersistentVector`).
//!
//! Elements `0 .. tail_offset()` live in full leaves of 32 under `root`,
//! the rest in `tail`. A version is immutable: `push` and `set` copy the
//! nodes on one path and share everything else, so an older version stays
//! valid and unchanged.

use std::rc::Rc;

const BITS: u32 = 5;
const WIDTH: usize = 1 << BITS;
const MASK: usize = WIDTH - 1;

#[derive(Debug)]
enum Node<T> {
    Branch(Vec<Rc<Node<T>>>),
    /// Always exactly `WIDTH` elements.
    Leaf(Vec<T>),
}

/// An immutable array version sharing structure with the versions it was
/// made from.
#[derive(Debug)]
pub struct Trie<T> {
    len: usize,
    /// Bits to shift an index by at the root (`BITS` × levels).
    shift: u32,
    root: Rc<Node<T>>,
    tail: Rc<Vec<T>>,
}

impl<T> Clone for Trie<T> {
    fn clone(&self) -> Self {
        Self { len: self.len, shift: self.shift, root: Rc::clone(&self.root), tail: Rc::clone(&self.tail) }
    }
}

impl<T: Clone> Default for Trie<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Trie<T> {
    pub fn new() -> Self {
        Self { len: 0, shift: BITS, root: Rc::new(Node::Branch(Vec::new())), tail: Rc::new(Vec::new()) }
    }

    /// Builds a version from elements at once (the builder path): full
    /// leaves bottom-up, no intermediate versions.
    pub fn from_vec(mut items: Vec<T>) -> Self {
        let len = items.len();
        if len == 0 {
            return Self::new();
        }
        let tail_offset = ((len - 1) >> BITS) << BITS;
        let tail: Vec<T> = items.split_off(tail_offset);
        let mut level: Vec<Rc<Node<T>>> = Vec::with_capacity(tail_offset / WIDTH);
        let mut rest = items.into_iter();
        for _ in 0..tail_offset / WIDTH {
            level.push(Rc::new(Node::Leaf(rest.by_ref().take(WIDTH).collect())));
        }
        let mut shift = BITS;
        while level.len() > WIDTH {
            let mut parents = Vec::with_capacity(level.len().div_ceil(WIDTH));
            let mut children = level.into_iter();
            loop {
                let group: Vec<_> = children.by_ref().take(WIDTH).collect();
                if group.is_empty() {
                    break;
                }
                parents.push(Rc::new(Node::Branch(group)));
            }
            level = parents;
            shift += BITS;
        }
        Self { len, shift, root: Rc::new(Node::Branch(level)), tail: Rc::new(tail) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn tail_offset(&self) -> usize {
        if self.len < WIDTH { 0 } else { ((self.len - 1) >> BITS) << BITS }
    }

    /// The trie depth (levels of branches above the leaves).
    pub fn depth(&self) -> u32 {
        self.shift / BITS
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let offset = self.tail_offset();
        if index >= offset {
            return self.tail.get(index - offset);
        }
        let mut node = &*self.root;
        let mut shift = self.shift;
        loop {
            match node {
                Node::Branch(children) => {
                    node = &children[(index >> shift) & MASK];
                    shift -= BITS;
                }
                Node::Leaf(items) => return Some(&items[index & MASK]),
            }
        }
    }

    /// Calls `visit` with every run of consecutive elements in order (whole
    /// leaves, then the tail): iteration without a per-element trie walk.
    pub fn for_each_chunk(&self, mut visit: impl FnMut(&[T])) {
        fn walk<T>(node: &Node<T>, visit: &mut impl FnMut(&[T])) {
            match node {
                Node::Branch(children) => children.iter().for_each(|child| walk(child, visit)),
                Node::Leaf(items) => visit(items),
            }
        }
        walk(&self.root, &mut visit);
        visit(&self.tail);
    }

    /// Let this version go without recursion through its elements: the
    /// elements of the nodes no other version holds go to `visit` one at a
    /// time, and the nodes other versions hold are only let go. For
    /// elements that hold versions in turn, which the caller frees from a
    /// queue of its own.
    pub fn into_unshared(self, mut visit: impl FnMut(T)) {
        if let Ok(tail) = Rc::try_unwrap(self.tail) {
            tail.into_iter().for_each(&mut visit);
        }
        let mut stack = vec![self.root];
        while let Some(node) = stack.pop() {
            match Rc::try_unwrap(node) {
                Ok(Node::Branch(kids)) => stack.extend(kids),
                Ok(Node::Leaf(items)) => items.into_iter().for_each(&mut visit),
                Err(_) => {}
            }
        }
    }

    pub fn to_vec(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.len);
        self.for_each_chunk(|chunk| out.extend_from_slice(chunk));
        out
    }

    /// A new version with `item` appended; this one is unchanged.
    pub fn push(&self, item: T) -> Self {
        if self.len - self.tail_offset() < WIDTH {
            // Room in the tail: copy it (at most 31 elements) with the new one.
            let mut tail = Vec::with_capacity(self.tail.len() + 1);
            tail.extend_from_slice(&self.tail);
            tail.push(item);
            return Self { len: self.len + 1, shift: self.shift, root: Rc::clone(&self.root), tail: Rc::new(tail) };
        }
        // The tail is full: it becomes a leaf of the trie, and a new tail starts.
        let leaf = Rc::new(Node::Leaf(self.tail.as_ref().clone()));
        let (root, shift) = if (self.len >> BITS) > (1 << self.shift) {
            // The root is full: one more level.
            let path = Self::path(self.shift, leaf);
            (Rc::new(Node::Branch(vec![Rc::clone(&self.root), Rc::new(path)])), self.shift + BITS)
        } else {
            (Rc::new(self.push_leaf(self.shift, &self.root, leaf)), self.shift)
        };
        Self { len: self.len + 1, shift, root, tail: Rc::new(vec![item]) }
    }

    fn path(shift: u32, leaf: Rc<Node<T>>) -> Node<T> {
        if shift == 0 { Rc::try_unwrap(leaf).unwrap_or_else(|_| unreachable!("a fresh leaf")) } else { Node::Branch(vec![Rc::new(Self::path(shift - BITS, leaf))]) }
    }

    fn push_leaf(&self, shift: u32, parent: &Node<T>, leaf: Rc<Node<T>>) -> Node<T> {
        let Node::Branch(children) = parent else { unreachable!("a branch above the leaves") };
        let at = ((self.len - 1) >> shift) & MASK;
        let child = if shift == BITS {
            leaf
        } else if let Some(existing) = children.get(at) {
            Rc::new(self.push_leaf(shift - BITS, existing, leaf))
        } else {
            Rc::new(Self::path(shift - BITS, leaf))
        };
        let mut copy = children.clone();
        if at < copy.len() {
            copy[at] = child;
        } else {
            copy.push(child);
        }
        Node::Branch(copy)
    }

    /// A new version with element `index` replaced (path copy); `None` out
    /// of bounds.
    pub fn set(&self, index: usize, item: T) -> Option<Self> {
        if index >= self.len {
            return None;
        }
        let offset = self.tail_offset();
        if index >= offset {
            let mut tail = self.tail.as_ref().clone();
            tail[index - offset] = item;
            return Some(Self { len: self.len, shift: self.shift, root: Rc::clone(&self.root), tail: Rc::new(tail) });
        }
        fn replace<T: Clone>(node: &Node<T>, shift: u32, index: usize, item: T) -> Node<T> {
            match node {
                Node::Branch(children) => {
                    let at = (index >> shift) & MASK;
                    let mut copy = children.clone();
                    copy[at] = Rc::new(replace(&children[at], shift - BITS, index, item));
                    Node::Branch(copy)
                }
                Node::Leaf(items) => {
                    let mut copy = items.clone();
                    copy[index & MASK] = item;
                    Node::Leaf(copy)
                }
            }
        }
        let root = Rc::new(replace(&self.root, self.shift, index, item));
        Some(Self { len: self.len, shift: self.shift, root, tail: Rc::clone(&self.tail) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator (xorshift64*), so failures replay.
    struct Rng(u64);
    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % n as u64) as usize
        }
    }

    fn check(trie: &Trie<u64>, model: &[u64]) {
        assert_eq!(trie.len(), model.len());
        assert_eq!(trie.to_vec(), model);
        for (i, item) in model.iter().enumerate() {
            assert_eq!(trie.get(i), Some(item), "index {i} of {}", model.len());
        }
        assert_eq!(trie.get(model.len()), None);
    }

    #[test]
    fn push_keeps_every_version_across_level_boundaries() {
        // Up to past 32 + 32² + 32³: the tail, one level, two levels, three.
        let mut versions = vec![Trie::new()];
        let mut model = Vec::new();
        for i in 0..34_000u64 {
            let last = versions.last().unwrap().push(i);
            model.push(i);
            if [31, 32, 33, 1056, 1057, 33_824, 33_825].contains(&model.len()) || model.len() % 4093 == 0 {
                check(&last, &model);
            }
            versions.push(last);
        }
        // Old versions are untouched by the pushes made after them.
        for (len, version) in versions.iter().enumerate().step_by(997) {
            check(version, &model[..len]);
        }
    }

    #[test]
    fn from_vec_equals_pushes() {
        for n in [0, 1, 31, 32, 33, 64, 65, 1024, 1056, 1057, 1088, 32_768 + 32, 40_000] {
            let items: Vec<u64> = (0..n as u64).collect();
            let built = Trie::from_vec(items.clone());
            check(&built, &items);
            let pushed = built.push(7);
            let mut more = items.clone();
            more.push(7);
            check(&pushed, &more);
            check(&built, &items);
        }
    }

    #[test]
    fn random_sets_and_pushes_against_a_model() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut history: Vec<(Trie<u64>, Vec<u64>)> = vec![(Trie::new(), Vec::new())];
        for step in 0..20_000u64 {
            let (trie, model) = history[rng.below(history.len())].clone();
            let (next, mut next_model) = (trie.clone(), model.clone());
            let made = if model.is_empty() || rng.below(3) > 0 {
                next_model.push(step);
                next.push(step)
            } else {
                let at = rng.below(model.len());
                next_model[at] = step;
                next.set(at, step).unwrap()
            };
            assert_eq!(made.get(next_model.len() - 1), next_model.last());
            history.push((made, next_model));
        }
        for (trie, model) in history.iter().step_by(613) {
            check(trie, model);
        }
    }

    #[test]
    fn into_unshared_visits_only_what_no_other_version_holds() {
        let old = Trie::from_vec((0..100).collect::<Vec<u32>>());
        let new = old.set(0, 1000).unwrap().set(99, 2000).unwrap();
        let mut seen = Vec::new();
        new.into_unshared(|item| seen.push(item));
        // The leaf of index 0 and the tail were copied; the rest is shared.
        seen.sort_unstable();
        let mut copied: Vec<u32> = (1..32).chain(96..99).chain([1000, 2000]).collect();
        copied.sort_unstable();
        assert_eq!(seen, copied);
        assert_eq!(old.to_vec(), (0..100).collect::<Vec<_>>());
        let mut all = Vec::new();
        old.into_unshared(|item| all.push(item));
        all.sort_unstable();
        assert_eq!(all, (0..100).collect::<Vec<_>>());
    }
}
