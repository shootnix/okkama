//! Lookups with a key shaped like Tomato's map key (#252): a 40-byte enum
//! compared by variant, then by value. Medians of 7 runs.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::rc::Rc;
use std::time::Instant;

use okkama_persistent::PMap;
use okkama_persistent::pmap::Weigh;

#[allow(dead_code)]
#[derive(Clone, Debug)]
enum Key {
    Small(i64),
    Int(Vec<u64>, bool),
    Text(Rc<str>),
}
impl Key {
    fn rank(&self) -> u8 {
        match self {
            Key::Small(_) | Key::Int(..) => 0,
            Key::Text(_) => 1,
        }
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Key::Small(a), Key::Small(b)) => a.cmp(b),
            (Key::Text(a), Key::Text(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Key {}
impl Weigh for Key {}

fn spot(x: &mut u64, n: usize) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    ((*x >> 32) * n as u64) >> 32
}

fn median(mut work: impl FnMut() -> u64) -> f64 {
    black_box(work());
    let mut t: Vec<f64> = (0..7).map(|_| { let s = Instant::now(); black_box(work()); s.elapsed().as_nanos() as f64 }).collect();
    t.sort_by(f64::total_cmp);
    t[3]
}

fn main() {
    println!("size_of Key {} entry {}", std::mem::size_of::<Key>(), std::mem::size_of::<(Key, (u64, u64))>());
    for n in [1_000usize, 16_000, 100_000, 1_000_000] {
        let entries: Vec<(Key, (u64, u64))> = (0..n as i64).map(|i| (Key::Small(i * 2), (i as u64, i as u64))).collect();
        let btree: BTreeMap<Key, (u64, u64)> = entries.iter().cloned().collect();
        let mut pmap = PMap::new();
        for (k, v) in entries.iter().cloned().rev() { pmap.insert(k, v); }
        let reads = 2_000_000;
        let b = median(|| { let mut x = 1; (0..reads).filter(|_| btree.contains_key(&Key::Small(spot(&mut x, n) as i64 * 2))).count() as u64 });
        let p = median(|| { let mut x = 1; (0..reads).filter(|_| pmap.contains_key(&Key::Small(spot(&mut x, n) as i64 * 2))).count() as u64 });
        println!("n={n:<8} get btree {:6.1} ns  pmap {:6.1} ns  ({:+.0}%)  depth {}", b / reads as f64, p / reads as f64, 100.0 * (p / b - 1.0), pmap.check().unwrap());
    }
}
