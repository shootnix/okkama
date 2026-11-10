//! A vector of at most `N` items stored inline, so that a map node is one
//! allocation and a search reads its keys without following a pointer.
//!
//! The invariant every method keeps: `items[..len]` are initialized and
//! owned, the rest are not. Operations past the capacity panic.

#![allow(unsafe_code)]

use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::ptr;

pub(crate) struct Slots<T, const N: usize> {
    len: usize,
    items: [MaybeUninit<T>; N],
}

impl<T, const N: usize> Slots<T, N> {
    pub(crate) fn new() -> Self {
        Self { len: 0, items: [const { MaybeUninit::uninit() }; N] }
    }

    fn base(&mut self) -> *mut T {
        self.items.as_mut_ptr().cast()
    }

    pub(crate) fn push(&mut self, item: T) {
        assert!(self.len < N, "a node over its capacity");
        self.items[self.len].write(item);
        self.len += 1;
    }

    pub(crate) fn insert(&mut self, at: usize, item: T) {
        assert!(at <= self.len && self.len < N, "a node over its capacity");
        let (len, base) = (self.len, self.base());
        // SAFETY: `at ..= len` is within the capacity; the items `at .. len`
        // move up one slot, and `at` is then written.
        unsafe {
            ptr::copy(base.add(at), base.add(at + 1), len - at);
            base.add(at).write(item);
        }
        self.len += 1;
    }

    pub(crate) fn remove(&mut self, at: usize) -> T {
        assert!(at < self.len, "an index past the end");
        let (len, base) = (self.len, self.base());
        self.len -= 1;
        // SAFETY: `at` is initialized; it is read out, and the items after
        // it move down one slot over it.
        unsafe {
            let item = base.add(at).read();
            ptr::copy(base.add(at + 1), base.add(at), len - at - 1);
            item
        }
    }

    /// The items from `at` on, moved into a new vector.
    pub(crate) fn split_off(&mut self, at: usize) -> Self {
        assert!(at <= self.len, "an index past the end");
        let mut right = Self::new();
        let moved = self.len - at;
        // SAFETY: the items `at .. len` move to the start of `right`, which
        // owns them from here; `self` stops at `at`.
        unsafe { ptr::copy_nonoverlapping(self.base().add(at), right.base(), moved) };
        self.len = at;
        right.len = moved;
        right
    }

    /// Move all of `other`'s items to the end.
    pub(crate) fn append(&mut self, other: &mut Self) {
        assert!(self.len + other.len <= N, "a node over its capacity");
        let moved = other.len;
        // SAFETY: `other`'s items move behind ours; it is left empty.
        unsafe { ptr::copy_nonoverlapping(other.base(), self.base().add(self.len), moved) };
        other.len = 0;
        self.len += moved;
    }
}

impl<T: Clone, const N: usize> Slots<T, N> {
    pub(crate) fn extend_from_slice(&mut self, items: &[T]) {
        for item in items {
            self.push(item.clone());
        }
    }
}

impl<T, const N: usize> Deref for Slots<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        // SAFETY: the first `len` items are initialized.
        unsafe { std::slice::from_raw_parts(self.items.as_ptr().cast(), self.len) }
    }
}

impl<T, const N: usize> DerefMut for Slots<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        // SAFETY: the first `len` items are initialized.
        unsafe { std::slice::from_raw_parts_mut(self.base(), self.len) }
    }
}

impl<T, const N: usize> FromIterator<T> for Slots<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let mut slots = Self::new();
        for item in items {
            slots.push(item);
        }
        slots
    }
}

impl<T: Clone, const N: usize> Clone for Slots<T, N> {
    fn clone(&self) -> Self {
        let mut copy = Self::new();
        copy.extend_from_slice(self);
        copy
    }
}

impl<T, const N: usize> Drop for Slots<T, N> {
    fn drop(&mut self) {
        let items: *mut [T] = &mut **self;
        self.len = 0;
        // SAFETY: the items were initialized and are dropped once; `len` is
        // already zero should a drop panic.
        unsafe { ptr::drop_in_place(items) };
    }
}

impl<T: std::fmt::Debug, const N: usize> std::fmt::Debug for Slots<T, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn moves_and_drops_every_item_once() {
        let probe = Rc::new(());
        {
            let mut a: Slots<Rc<()>, 8> = (0..5).map(|_| probe.clone()).collect();
            a.insert(0, probe.clone());
            a.insert(6, probe.clone());
            drop(a.remove(3));
            let mut b = a.split_off(2);
            assert_eq!((a.len(), b.len()), (2, 4));
            a.append(&mut b);
            assert_eq!((a.len(), b.len()), (6, 0));
            let c = a.clone();
            assert_eq!(Rc::strong_count(&probe), 13);
            drop(c);
        }
        assert_eq!(Rc::strong_count(&probe), 1);
        let mut order: Slots<u32, 6> = [1, 3, 5].into_iter().collect();
        order.insert(1, 2);
        order.insert(3, 4);
        assert_eq!(&*order, &[1, 2, 3, 4, 5]);
        assert_eq!(order.remove(0), 1);
        assert_eq!(&*order.split_off(2), &[4, 5]);
        assert_eq!(&*order, &[2, 3]);
    }
}
