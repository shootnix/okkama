//! Memory for machine code: one reserved region per kind, filled in order.
//!
//! Cranelift's default provider maps fresh pages for every finalized batch
//! (each lazily compiled function) and, on AArch64 Linux, follows each batch
//! with `membarrier(PRIVATE_EXPEDITED_SYNC_CORE)`: an IPI to every core of the
//! process, and several milliseconds on the first call to register it. The
//! barrier is for code that other threads may run; machine code here runs on
//! the VM thread only, which wrote it. Cleaning the caches for the new range
//! on this core is enough (`clear_cache` ends with an ISB, and a migration to
//! another core is a context synchronization event).
#![allow(unsafe_code)]

use cranelift_jit::{BranchProtection, JITMemoryKind, JITMemoryProvider};
use cranelift_module::{ModuleError, ModuleResult};
use std::io;

/// Virtual reservation per kind; pages are committed as they are used.
const RESERVE: usize = 1 << 30;

fn page() -> usize {
    // SAFETY: sysconf has no preconditions.
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

fn ceil(value: usize, unit: usize) -> usize {
    value.div_ceil(unit) * unit
}

/// One reserved region: `[0, finalized)` has its final protection, new
/// allocations start on the page after it and are writable until finalized.
struct Region {
    base: *mut u8,
    top: usize,
    committed: usize,
    finalized: usize,
    protection: libc::c_int,
}

impl Region {
    fn new(protection: libc::c_int) -> Self {
        Self { base: std::ptr::null_mut(), top: 0, committed: 0, finalized: 0, protection }
    }

    fn allocate(&mut self, size: usize, align: u64) -> io::Result<*mut u8> {
        if self.base.is_null() {
            // SAFETY: an anonymous reservation without access.
            let base = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    RESERVE,
                    libc::PROT_NONE,
                    libc::MAP_PRIVATE | libc::MAP_ANON | libc::MAP_NORESERVE,
                    -1,
                    0,
                )
            };
            if base == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            self.base = base.cast();
        }
        let start = ceil(self.top, align.max(1) as usize);
        let end = start.checked_add(size).filter(|&end| end <= RESERVE).ok_or_else(|| {
            io::Error::new(io::ErrorKind::OutOfMemory, "machine code region is full")
        })?;
        let needed = ceil(end, page());
        if needed > self.committed {
            // SAFETY: pages inside the reservation, not yet finalized.
            let result = unsafe {
                libc::mprotect(
                    self.base.add(self.committed).cast(),
                    needed - self.committed,
                    libc::PROT_READ | libc::PROT_WRITE,
                )
            };
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            self.committed = needed;
        }
        self.top = end;
        // SAFETY: inside the committed part of the reservation.
        Ok(unsafe { self.base.add(start) })
    }

    fn finalize(&mut self, code: bool) -> io::Result<()> {
        let end = ceil(self.top, page());
        if end == self.finalized || self.protection == libc::PROT_READ | libc::PROT_WRITE {
            return Ok(());
        }
        // SAFETY: the pages written since the last finalization.
        unsafe {
            let start = self.base.add(self.finalized);
            if code {
                wasmtime_jit_icache_coherence::clear_cache(start.cast(), end - self.finalized)
                    .map_err(|e| io::Error::other(e.to_string()))?;
            }
            if libc::mprotect(start.cast(), end - self.finalized, self.protection) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        // Later allocations start on a fresh page: finalized pages stay read-only.
        self.finalized = end;
        self.top = end;
        Ok(())
    }

    fn free(&mut self) {
        if !self.base.is_null() {
            // SAFETY: the whole reservation made in `allocate`.
            unsafe { libc::munmap(self.base.cast(), RESERVE) };
            *self = Self::new(self.protection);
        }
    }
}

pub struct CodeMemory {
    code: Region,
    writable: Region,
    read_only: Region,
}

// SAFETY: the regions are owned and only touched through `&mut self`.
unsafe impl Send for CodeMemory {}

impl CodeMemory {
    pub fn new() -> Self {
        Self {
            code: Region::new(libc::PROT_READ | libc::PROT_EXEC),
            writable: Region::new(libc::PROT_READ | libc::PROT_WRITE),
            read_only: Region::new(libc::PROT_READ),
        }
    }
}

impl JITMemoryProvider for CodeMemory {
    fn allocate(&mut self, size: usize, align: u64, kind: JITMemoryKind) -> io::Result<*mut u8> {
        match kind {
            JITMemoryKind::Executable => self.code.allocate(size, align),
            JITMemoryKind::Writable => self.writable.allocate(size, align),
            JITMemoryKind::ReadOnly => self.read_only.allocate(size, align),
        }
    }

    unsafe fn free_memory(&mut self) {
        self.code.free();
        self.writable.free();
        self.read_only.free();
    }

    fn finalize(&mut self, _branch_protection: BranchProtection) -> ModuleResult<()> {
        // Branch target identification is not enabled for this ISA (see `jit`).
        let allocation = |err| ModuleError::Allocation { err };
        self.code.finalize(true).map_err(allocation)?;
        self.read_only.finalize(false).map_err(allocation)?;
        Ok(())
    }
}

impl Drop for CodeMemory {
    fn drop(&mut self) {
        // SAFETY: the JIT module that used this memory is gone with it.
        unsafe { self.free_memory() };
    }
}
