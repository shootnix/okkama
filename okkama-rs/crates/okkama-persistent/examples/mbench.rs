//! Native cost of the persistent map against the standard B-tree (#252).
//!
//!     cargo run --release -p okkama-persistent --example mbench -- [OUT.json]
//!
//! Keys and values are `u64`. `btree` is the standard `BTreeMap`, which is
//! what Tomato's maps are today: a new version is a clone, then the change.
//! `pmap` is the persistent B+ tree: a new version shares all but one path.
//! Each measurement is the median of 7 runs.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

use okkama_persistent::PMap;

const SIZES: [usize; 4] = [1_000, 16_000, 100_000, 1_000_000];
const RUNS: usize = 7;

fn measure(mut work: impl FnMut() -> u64) -> f64 {
    black_box(work());
    let mut times: Vec<f64> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            black_box(work());
            started.elapsed().as_nanos() as f64
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[RUNS / 2]
}

fn spot(x: &mut u64, n: usize) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    ((*x >> 32) * n as u64) >> 32
}

fn main() {
    let out = std::env::args().nth(1);
    let mut rows = Vec::new();
    let mut add = |op: &str, repr: &str, n: usize, ops: usize, ns: f64| {
        println!("{op:16} {repr:6} n={n:<8} {:10.2} ns/op", ns / ops as f64);
        rows.push(format!("{{\"op\":\"{op}\",\"repr\":\"{repr}\",\"n\":{n},\"ns_per_op\":{:.4}}}", ns / ops as f64));
    };
    for n in SIZES {
        let keys: Vec<u64> = (0..n as u64).map(|i| i * 2).collect();
        let btree: BTreeMap<u64, u64> = keys.iter().map(|&k| (k, k)).collect();
        let pmap = PMap::from_sorted(keys.iter().map(|&k| (k, k)).collect());
        let reads = 4_000_000;
        add("get-random", "btree", n, reads, measure(|| {
            let (mut x, mut acc) = (1u64, 0u64);
            for _ in 0..reads { acc = acc.wrapping_add(*btree.get(&(spot(&mut x, n) * 2)).unwrap()); }
            acc
        }));
        add("get-random", "pmap", n, reads, measure(|| {
            let (mut x, mut acc) = (1u64, 0u64);
            for _ in 0..reads { acc = acc.wrapping_add(*pmap.get(&(spot(&mut x, n) * 2)).unwrap()); }
            acc
        }));
        add("get-miss", "btree", n, reads, measure(|| {
            let mut x = 1u64;
            (0..reads).filter(|_| btree.get(&(spot(&mut x, n) * 2 + 1)).is_some()).count() as u64
        }));
        add("get-miss", "pmap", n, reads, measure(|| {
            let mut x = 1u64;
            (0..reads).filter(|_| pmap.get(&(spot(&mut x, n) * 2 + 1)).is_some()).count() as u64
        }));
        let passes = (8_000_000 / n).max(1);
        add("iterate", "btree", n, passes * n, measure(|| (0..passes).fold(0u64, |a, _| btree.values().fold(a, |a, v| a.wrapping_add(*v)))));
        add("iterate", "pmap", n, passes * n, measure(|| (0..passes).fold(0u64, |a, _| pmap.values().fold(a, |a, v| a.wrapping_add(*v)))));
        add("build-sorted", "btree", n, n, measure(|| keys.iter().map(|&k| (k, k)).collect::<BTreeMap<_, _>>().len() as u64));
        add("build-sorted", "pmap", n, n, measure(|| PMap::from_sorted(keys.iter().map(|&k| (k, k)).collect()).len() as u64));
        add("build-insert", "btree", n, n, measure(|| {
            let mut m = BTreeMap::new();
            for &k in &keys { m.insert(k.wrapping_mul(0x9E37_79B9_7F4A_7C15), k); }
            m.len() as u64
        }));
        add("build-insert", "pmap", n, n, measure(|| {
            let mut m = PMap::new();
            for &k in &keys { m.insert(k.wrapping_mul(0x9E37_79B9_7F4A_7C15), k); }
            m.len() as u64
        }));
        // A new version of one shared base per step (today: clone + insert).
        let k = (2_000_000 / n).max(20);
        add("put-branch", "btree", n, k, measure(|| {
            (0..k as u64).fold(0, |a, j| { let mut m = btree.clone(); m.insert(j * 2 + 1, j); a + m.len() as u64 })
        }));
        add("put-branch", "pmap", n, k, measure(|| {
            (0..k as u64).fold(0, |a, j| { let mut m = pmap.clone(); m.insert(j * 2 + 1, j); a + m.len() as u64 })
        }));
        add("remove-branch", "btree", n, k, measure(|| {
            (0..k as u64).fold(0, |a, j| { let mut m = btree.clone(); m.remove(&((j % n as u64) * 2)); a + m.len() as u64 })
        }));
        add("remove-branch", "pmap", n, k, measure(|| {
            (0..k as u64).fold(0, |a, j| { let mut m = pmap.clone(); m.remove(&((j % n as u64) * 2)); a + m.len() as u64 })
        }));
        // Each version made from the previous one, all kept (the history).
        add("put-keep", "btree", n, k, measure(|| {
            let mut kept = vec![btree.clone()];
            for j in 0..k as u64 { let mut m = kept.last().unwrap().clone(); m.insert(j * 2 + 1, j); kept.push(m); }
            kept.len() as u64
        }));
        add("put-keep", "pmap", n, k, measure(|| {
            let mut kept = vec![pmap.clone()];
            for j in 0..k as u64 { let mut m = kept.last().unwrap().clone(); m.insert(j * 2 + 1, j); kept.push(m); }
            kept.len() as u64
        }));
    }
    if let Some(path) = out {
        std::fs::write(path, format!("[\n{}\n]\n", rows.join(",\n"))).expect("write the results");
    }
}
