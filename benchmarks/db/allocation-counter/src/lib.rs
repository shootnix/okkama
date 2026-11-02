//! Diagnostic Rust allocation requests, delegated unchanged to MiMalloc.
//! Excludes C allocations and allocator internals. Bytes are requested traffic,
//! not resident memory. Reallocation records the entire new requested size.
use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static REALLOCS: AtomicU64 = AtomicU64::new(0);
static REALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

// Exclusive categories: rest, Region methods outside collect, collect.
static SCOPES: [[AtomicU64; 5]; 3] = [const { [const { AtomicU64::new(0) }; 5] }; 3];
thread_local! { static SCOPE: Cell<usize> = const { Cell::new(0) }; }

#[derive(Debug)]
pub struct Scope { previous: usize, _thread: PhantomData<Rc<()>> }

/// Synchronous scopes only: never hold across a coroutine yield/await.
pub fn enter(scope: usize) -> Scope {
    assert!(scope < 3);
    Scope { previous: SCOPE.with(|value| value.replace(scope)), _thread: PhantomData }
}

impl Drop for Scope {
    fn drop(&mut self) { SCOPE.with(|value| value.set(self.previous)); }
}

fn record(column: usize, amount: u64) {
    let scope = SCOPE.try_with(Cell::get).unwrap_or(0);
    SCOPES[scope][column].fetch_add(amount, Relaxed);
}

#[derive(Debug)]
pub struct Counting;
// SAFETY: every operation forwards its pointer and layout unchanged to the same
// underlying allocator. Counter updates do not allocate, unwind or dereference.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(layout.size() as u64, Relaxed);
        record(0, 1); record(1, layout.size() as u64);
        unsafe { mimalloc::MiMalloc.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(layout.size() as u64, Relaxed);
        record(0, 1); record(1, layout.size() as u64);
        unsafe { mimalloc::MiMalloc.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        REALLOCS.fetch_add(1, Relaxed);
        REALLOC_BYTES.fetch_add(size as u64, Relaxed);
        record(2, 1); record(3, size as u64);
        unsafe { mimalloc::MiMalloc.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREES.fetch_add(1, Relaxed);
        record(4, 1);
        unsafe { mimalloc::MiMalloc.dealloc(ptr, layout) }
    }
}

#[derive(Debug)]
pub struct Report;
impl Drop for Report {
    fn drop(&mut self) {
        let counts = [ALLOCS.load(Relaxed), BYTES.load(Relaxed), REALLOCS.load(Relaxed),
                      REALLOC_BYTES.load(Relaxed), FREES.load(Relaxed)];
        let scopes: [[u64; 5]; 3] = std::array::from_fn(|row|
            std::array::from_fn(|col| SCOPES[row][col].load(Relaxed)));
        eprintln!("allocation_stats {counts:?}");
        eprintln!("allocation_scopes {scopes:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_scope_is_exclusive_and_restored_on_unwind() {
        assert_eq!(SCOPE.with(Cell::get), 0);
        let region = enter(1);
        let _ = std::panic::catch_unwind(|| {
            let _collect = enter(2);
            assert_eq!(SCOPE.with(Cell::get), 2);
            panic!("exercise scope cleanup");
        });
        assert_eq!(SCOPE.with(Cell::get), 1);
        drop(region);
        assert_eq!(SCOPE.with(Cell::get), 0);
    }

    #[test]
    fn preserves_alignment_zeroing_and_reallocation_contents() {
        unsafe {
            let layout = Layout::from_size_align(128, 64).unwrap();
            let ptr = Counting.alloc_zeroed(layout);
            assert!(!ptr.is_null());
            assert_eq!(ptr as usize % 64, 0);
            assert!(std::slice::from_raw_parts(ptr, 128).iter().all(|b| *b == 0));
            ptr.write_bytes(0x5a, 128);
            let grown = Counting.realloc(ptr, layout, 4096);
            assert!(!grown.is_null());
            assert_eq!(grown as usize % 64, 0);
            assert!(std::slice::from_raw_parts(grown, 128).iter().all(|b| *b == 0x5a));
            Counting.dealloc(grown, Layout::from_size_align(4096, 64).unwrap());
            let plain = Counting.alloc(layout);
            assert!(!plain.is_null());
            assert_eq!(plain as usize % 64, 0);
            Counting.dealloc(plain, layout);
        }
    }
}
