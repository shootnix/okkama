//! Native cost of the candidate array representations (#246, phase 1).
//!
//!     cargo run --release -p okkama-persistent --example pbench -- [OUT.json]
//!
//! Elements are `u64`, one word, as in Tomato. `flat` is today's
//! representation: an `Rc<[u64]>` that every new version copies whole;
//! `trie` is the 32-way trie with a tail; `frontier` the shared append-only
//! buffer. Each measurement is the median of 7 runs; a counting allocator
//! reports the bytes allocated per operation and the bytes live when every
//! version of an append chain is kept.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use okkama_persistent::{Frontier, Trie};

struct Counting;
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Relaxed);
        LIVE.fetch_add(layout.size(), Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED.fetch_add(new_size, Relaxed);
        LIVE.fetch_add(new_size, Relaxed);
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const SIZES: [usize; 5] = [1_000, 4_000, 16_000, 100_000, 1_000_000];
const READS: usize = 10_000_000;
const RUNS: usize = 7;

fn lcg(x: u64) -> u64 {
    x.wrapping_mul(48271) % 2_147_483_647
}

/// A cheap pseudo-random index in `0 .. n` (xorshift64, then Lemire's
/// multiply-shift instead of a division), so a random read measures the
/// read and not the index arithmetic.
fn spot(x: &mut u64, n: usize) -> usize {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    (((*x >> 32) * n as u64) >> 32) as usize
}

/// Median nanoseconds of `RUNS` runs of `work`, and the bytes it allocated per run.
fn measure(mut work: impl FnMut() -> u64) -> (f64, usize) {
    let mut times = Vec::with_capacity(RUNS);
    let mut bytes = 0;
    black_box(work());
    for _ in 0..RUNS {
        let before = ALLOCATED.load(Relaxed);
        let started = Instant::now();
        black_box(work());
        times.push(started.elapsed().as_nanos() as f64);
        bytes = ALLOCATED.load(Relaxed) - before;
    }
    times.sort_by(f64::total_cmp);
    (times[RUNS / 2], bytes)
}

fn copy_push(xs: &Rc<[u64]>, item: u64) -> Rc<[u64]> {
    let mut v = Vec::with_capacity(xs.len() + 1);
    v.extend_from_slice(xs);
    v.push(item);
    Rc::from(v)
}

fn copy_set(xs: &Rc<[u64]>, at: usize, item: u64) -> Rc<[u64]> {
    let mut v = xs.to_vec();
    v[at] = item;
    Rc::from(v)
}

struct Row {
    op: &'static str,
    repr: &'static str,
    n: usize,
    ns_per_op: f64,
    bytes_per_op: f64,
    note: String,
}

fn main() {
    let out = std::env::args().nth(1);
    let mut rows: Vec<Row> = Vec::new();
    let mut add = |op, repr, n, ops: usize, (ns, bytes): (f64, usize), note: String| {
        let row = Row { op, repr, n, ns_per_op: ns / ops as f64, bytes_per_op: bytes as f64 / ops as f64, note };
        println!("{:16} {:9} n={:<8} {:10.2} ns/op {:12.1} B/op {}", row.op, row.repr, row.n, row.ns_per_op, row.bytes_per_op, row.note);
        rows.push(row);
    };
    for n in SIZES {
        let values: Vec<u64> = (0..n as u64).map(|i| (i * 7919 + 13) % 10007).collect();
        let flat: Rc<[u64]> = Rc::from(values.clone());
        let trie = Trie::from_vec(values.clone());
        let frontier = Frontier::from_vec(values.clone());
        let depth = format!("depth {}", trie.depth());

        // ---- reads ----
        add("read-random", "flat", n, READS, measure(|| {
            let (mut x, mut acc) = (1u64, 0u64);
            for _ in 0..READS { acc = acc.wrapping_add(flat[spot(&mut x, n)]); }
            acc
        }), String::new());
        add("read-random", "trie", n, READS, measure(|| {
            let (mut x, mut acc) = (1u64, 0u64);
            for _ in 0..READS { acc = acc.wrapping_add(*trie.get(spot(&mut x, n)).unwrap()); }
            acc
        }), depth.clone());
        add("read-random", "frontier", n, READS, measure(|| {
            let (mut x, mut acc) = (1u64, 0u64);
            for _ in 0..READS { acc = acc.wrapping_add(frontier.get(spot(&mut x, n)).unwrap()); }
            acc
        }), String::new());
        add("read-random", "loop", n, READS, measure(|| {
            let (mut x, mut acc) = (1u64, 0u64);
            for _ in 0..READS { acc = acc.wrapping_add(spot(&mut x, n) as u64); }
            acc
        }), "the index loop alone".into());

        let passes = (READS / n).max(1);
        add("read-seq-index", "flat", n, passes * n, measure(|| {
            let mut acc = 0u64;
            for _ in 0..passes { for i in 0..n { acc = acc.wrapping_add(black_box(&flat)[i]); } }
            acc
        }), String::new());
        add("read-seq-index", "trie", n, passes * n, measure(|| {
            let mut acc = 0u64;
            for _ in 0..passes { for i in 0..n { acc = acc.wrapping_add(*black_box(&trie).get(i).unwrap()); } }
            acc
        }), depth.clone());
        add("read-seq-index", "frontier", n, passes * n, measure(|| {
            let mut acc = 0u64;
            for _ in 0..passes { for i in 0..n { acc = acc.wrapping_add(black_box(&frontier).get(i).unwrap()); } }
            acc
        }), String::new());

        add("fold", "flat", n, passes * n, measure(|| {
            (0..passes).fold(0u64, |acc, _| black_box(&flat).iter().fold(acc, |a, &x| a.wrapping_add(x)))
        }), String::new());
        add("fold", "trie", n, passes * n, measure(|| {
            let mut acc = 0u64;
            for _ in 0..passes { black_box(&trie).for_each_chunk(|chunk| { acc = chunk.iter().fold(acc, |a, &x| a.wrapping_add(x)); }); }
            acc
        }), "leaf by leaf".into());
        add("fold", "frontier", n, passes * n, measure(|| {
            (0..passes).fold(0u64, |acc, _| black_box(&frontier).fold(acc, |a, &x| a.wrapping_add(x)))
        }), String::new());

        // ---- builds ----
        add("build", "flat", n, n, measure(|| { let v: Rc<[u64]> = Rc::from(values.clone()); v[n - 1] }), String::new());
        add("build", "trie", n, n, measure(|| *Trie::from_vec(values.clone()).get(n - 1).unwrap()), String::new());
        add("build", "frontier", n, n, measure(|| Frontier::from_vec(values.clone()).get(n - 1).unwrap()), String::new());

        // ---- writes: an append chain, each version made from the previous one ----
        if n <= 100_000 {
            add("append-chain", "flat", n, n, measure(|| {
                let mut v: Rc<[u64]> = Rc::from(Vec::new());
                for i in 0..n as u64 { v = copy_push(&v, i); }
                v[n - 1]
            }), "O(n) per append".into());
        }
        add("append-chain", "trie", n, n, measure(|| {
            let mut v = Trie::new();
            for i in 0..n as u64 { v = v.push(i); }
            *v.get(n - 1).unwrap()
        }), String::new());
        add("append-chain", "frontier", n, n, measure(|| {
            let mut v = Frontier::default();
            for i in 0..n as u64 { v = v.push(i); }
            v.get(n - 1).unwrap()
        }), String::new());

        // ---- writes: many versions of one base ----
        let k = (4_000_000 / n).max(200);
        add("append-branch", "flat", n, k, measure(|| {
            (0..k as u64).fold(0, |acc, j| { let v = copy_push(&flat, j); acc + v[n] })
        }), String::new());
        add("append-branch", "trie", n, k, measure(|| {
            (0..k as u64).fold(0, |acc, j| { let v = trie.push(j); acc + v.get(n).unwrap() })
        }), String::new());
        add("append-branch", "frontier", n, k, measure(|| {
            (0..k as u64).fold(0, |acc, j| { let v = frontier.push(j); acc + v.get(n).unwrap() })
        }), "a branch copies".into());

        // ---- writes: replace one element, each version from the previous one ----
        add("set-chain", "flat", n, k, measure(|| {
            let (mut v, mut x) = (Rc::clone(&flat), 1u64);
            for j in 0..k as u64 { x = lcg(x); v = copy_set(&v, (x % n as u64) as usize, j); }
            v[0]
        }), String::new());
        add("set-chain", "trie", n, k, measure(|| {
            let (mut v, mut x) = (trie.clone(), 1u64);
            for j in 0..k as u64 { x = lcg(x); v = v.set((x % n as u64) as usize, j).unwrap(); }
            *v.get(0).unwrap()
        }), String::new());

        // ---- memory: an append chain that keeps every version ----
        if n <= 16_000 {
            let before = LIVE.load(Relaxed);
            let mut kept: Vec<Rc<[u64]>> = vec![Rc::from(Vec::new())];
            for i in 0..n as u64 { let next = copy_push(kept.last().unwrap(), i); kept.push(next); }
            let bytes = LIVE.load(Relaxed) - before;
            add("keep-versions", "flat", n, n, (0.0, bytes), format!("{:.1} MB live", bytes as f64 / 1e6));
            drop(kept);
        }
        for repr in ["trie", "frontier"] {
            let before = LIVE.load(Relaxed);
            let bytes = if repr == "trie" {
                let mut kept = vec![Trie::new()];
                for i in 0..n as u64 { let next = kept.last().unwrap().push(i); kept.push(next); }
                let bytes = LIVE.load(Relaxed) - before;
                drop(kept);
                bytes
            } else {
                let mut kept = vec![Frontier::default()];
                for i in 0..n as u64 { let next = kept.last().unwrap().push(i); kept.push(next); }
                let bytes = LIVE.load(Relaxed) - before;
                drop(kept);
                bytes
            };
            add("keep-versions", repr, n, n, (0.0, bytes), format!("{:.1} MB live", bytes as f64 / 1e6));
        }
        // Memory of one array of n elements.
        for (repr, bytes) in [("flat", n * 8 + 16), ("trie", {
            let before = LIVE.load(Relaxed);
            let t = Trie::from_vec(values.clone());
            let b = LIVE.load(Relaxed) - before;
            drop(t);
            b
        })] {
            add("footprint", repr, n, 1, (0.0, bytes), format!("{:.3} B per element", bytes as f64 / n as f64));
        }
    }
    if let Some(path) = out {
        let json: Vec<String> = rows.iter().map(|r| format!(
            "{{\"op\":\"{}\",\"repr\":\"{}\",\"n\":{},\"ns_per_op\":{:.4},\"bytes_per_op\":{:.2},\"note\":\"{}\"}}",
            r.op, r.repr, r.n, r.ns_per_op, r.bytes_per_op, r.note)).collect();
        std::fs::write(path, format!("[\n{}\n]\n", json.join(",\n"))).expect("write the results");
    }
}
