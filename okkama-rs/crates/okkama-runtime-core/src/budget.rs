//! Allocation accounting and memory scope shared with Sparrow.
use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::sync::atomic::{AtomicIsize, Ordering};

/// What a memory budget measures (#269).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MemoryScope {
    /// What the process allocator holds, through the embedder's probe:
    /// every thread's memory counts. The CLIs run one runtime per process.
    #[default]
    Process,
    /// This runtime's own memory, so that runtimes in one process keep
    /// budgets of their own. Sparrow measures its heap. Memory a runtime
    /// cannot see, such as native libraries' and other threads', does not count.
    Runtime,
}

/// Net bytes allocated through [`Counted`], by every thread and by this one.
static NET: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static THREAD_NET: Cell<isize> = const { Cell::new(0) };
}

/// A counting global allocator for embedders without a probe of their own
/// (and tests): [`counted_bytes`] is its process probe and
/// [`thread_counted_bytes`] measures allocations on this thread.
/// It costs an atomic and a thread-local add per call, so the CLIs
/// use their allocator's statistics instead.
#[derive(Debug)]
pub struct Counted<A>(pub A);

#[inline]
fn count(bytes: isize) {
    NET.fetch_add(bytes, Ordering::Relaxed);
    // A thread being torn down has no counter left to keep.
    let _ = THREAD_NET.try_with(|net| net.set(net.get() + bytes));
}

// SAFETY: every call forwards to the wrapped allocator unchanged.
#[allow(unsafe_code)]
unsafe impl<A: GlobalAlloc> GlobalAlloc for Counted<A> {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        unsafe { self.0.alloc(layout) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        unsafe { self.0.alloc_zeroed(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        count(-(layout.size() as isize));
        unsafe { self.0.dealloc(pointer, layout) }
    }

    #[inline]
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size as isize - layout.size() as isize);
        unsafe { self.0.realloc(pointer, layout, size) }
    }
}

/// The process probe of [`Counted`].
pub fn counted_bytes() -> usize {
    NET.load(Ordering::Relaxed).max(0) as usize
}

/// Net bytes this thread has allocated through [`Counted`]; a thread that
/// frees what another allocated can go below zero, read as 0.
pub fn thread_counted_bytes() -> usize {
    THREAD_NET.with(Cell::get).max(0) as usize
}
