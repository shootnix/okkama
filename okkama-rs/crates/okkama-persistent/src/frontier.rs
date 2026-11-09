//! A flat array shared by the versions of an append chain.
//!
//! A version is a buffer and its own length. The buffer's elements
//! `0 .. len` are never written again; `push` appends in place when the
//! version ends where the buffer ends (its frontier), so a chain of appends
//! in which each version extends the previous one costs amortized O(1), and
//! copies only when a second version is appended to (a branch). Reads are a
//! plain indexed load through one indirection.

#![allow(unsafe_code)]

use std::cell::UnsafeCell;
use std::rc::Rc;

#[derive(Debug)]
struct Buffer<T> {
    items: UnsafeCell<Vec<T>>,
}

/// An immutable array version over a shared append-only buffer.
#[derive(Debug)]
pub struct Frontier<T> {
    buffer: Rc<Buffer<T>>,
    len: usize,
}

impl<T> Clone for Frontier<T> {
    fn clone(&self) -> Self {
        Self { buffer: Rc::clone(&self.buffer), len: self.len }
    }
}

impl<T: Clone> Default for Frontier<T> {
    fn default() -> Self {
        Self::from_vec(Vec::new())
    }
}

impl<T: Clone> Frontier<T> {
    pub fn from_vec(items: Vec<T>) -> Self {
        let len = items.len();
        Self { buffer: Rc::new(Buffer { items: UnsafeCell::new(items) }), len }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    // SAFETY (for every access below): `Rc` keeps this single-threaded, and
    // no reference into the vector escapes a method — reads return clones —
    // so the one `&mut` that `push` takes never overlaps another borrow.
    // `T::clone` must not push to a version of the same buffer (no element
    // type of the prototype does).

    pub fn get(&self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }
        let items = unsafe { &*self.buffer.items.get() };
        Some(items[index].clone())
    }

    /// A new version with `item` appended; this one is unchanged.
    pub fn push(&self, item: T) -> Self {
        let items = unsafe { &mut *self.buffer.items.get() };
        if items.len() == self.len {
            items.push(item);
            return Self { buffer: Rc::clone(&self.buffer), len: self.len + 1 };
        }
        // Another version already took the next slot: copy this one's prefix.
        let mut copy = Vec::with_capacity((self.len + 1).next_power_of_two());
        copy.extend_from_slice(&items[..self.len]);
        copy.push(item);
        Self::from_vec(copy)
    }

    /// Folds the elements in order, one slice access (the engines' `reduce`).
    pub fn fold<A>(&self, init: A, step: impl FnMut(A, &T) -> A) -> A {
        let items = unsafe { &*self.buffer.items.get() };
        items[..self.len].iter().fold(init, step)
    }

    pub fn to_vec(&self) -> Vec<T> {
        let items = unsafe { &*self.buffer.items.get() };
        items[..self.len].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_shares_and_a_branch_copies() {
        let base = (0..10u64).fold(Frontier::default(), |v, i| v.push(i));
        let left = base.push(100);
        let right = base.push(200);
        assert_eq!(base.to_vec(), (0..10).collect::<Vec<_>>());
        assert_eq!(left.get(10), Some(100));
        assert_eq!(right.get(10), Some(200));
        assert!(Rc::ptr_eq(&base.buffer, &left.buffer));
        assert!(!Rc::ptr_eq(&base.buffer, &right.buffer));
        let longer = left.push(101);
        assert_eq!(left.to_vec().len(), 11);
        assert_eq!(longer.get(11), Some(101));
        assert_eq!(base.len(), 10);
        assert_eq!(longer.fold(0, |a, x| a + x), 45 + 100 + 101);
    }
}
