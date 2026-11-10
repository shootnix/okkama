//! A memory budget of the runtime (#269): on the IR engine, what the run's
//! thread holds through `Counted`, so a run is not stopped by what another
//! runtime of the process allocates. A budget of the process sees it.
use okkama_runtime_core::budget::{self, Counted, Exhausted, Limits, MemoryScope, counted_bytes};
use std::hint::black_box;
use std::sync::{Arc, Barrier};

#[global_allocator]
static ALLOCATOR: Counted<std::alloc::System> = Counted(std::alloc::System);

/// The memory check of a run on its own thread with `scope`, while another
/// thread allocates 16 MiB after the run has started.
fn neighbour_grows(scope: MemoryScope) -> (Result<(), Exhausted>, Result<(), Exhausted>) {
    let started = Arc::new(Barrier::new(2));
    let grown = Arc::new(Barrier::new(2));
    let checked = Arc::new(Barrier::new(2));
    let limits = Limits { memory: Some(4 << 20), memory_scope: scope, ..Limits::default() };
    let neighbour = {
        let (started, grown, checked, limits) = (started.clone(), grown.clone(), checked.clone(), limits.clone());
        std::thread::spawn(move || {
            let _run = budget::begin(&limits).unwrap();
            started.wait();
            let held = black_box(vec![1u8; 16 << 20]);
            grown.wait();
            let check = budget::check_now();
            // Both runs look before the neighbour lets its memory go.
            checked.wait();
            drop(held);
            check
        })
    };
    let _run = budget::begin(&limits).unwrap();
    started.wait();
    grown.wait();
    let own = black_box(vec![1u8; 1 << 20]);
    let check = budget::check_now();
    checked.wait();
    drop(own);
    (check, neighbour.join().unwrap())
}

#[test]
fn a_runtime_budget_counts_only_its_own_thread() {
    budget::set_memory_probe(counted_bytes);
    // The neighbour is over its own budget either way.
    let (mine, neighbour) = neighbour_grows(MemoryScope::Runtime);
    assert_eq!((mine, neighbour), (Ok(()), Err(Exhausted::Memory)));
    // Measured over the process, the neighbour's 16 MiB stop this run too.
    let (mine, neighbour) = neighbour_grows(MemoryScope::Process);
    assert_eq!((mine, neighbour), (Err(Exhausted::Memory), Err(Exhausted::Memory)));
}
