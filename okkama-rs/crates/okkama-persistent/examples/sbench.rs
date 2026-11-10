//! Chains of string concatenation (#255): today's flat copy, a shared byte
//! buffer with a frontier (the design #246 gave arrays), and a rope of
//! chunks as the reference the ticket names.
//!
//!     cargo run --release -p okkama-persistent --example sbench
//!
//! `flat` copies both parts at every `text + piece`, as Tomato's `rt_concat`
//! does. `buffer` writes the piece at the frontier of a buffer shared by the
//! versions when the left part ends there, and copies otherwise (a branch).
//! `rope` keeps chunks of up to 1 KB in a persistent vector and copies only
//! its last chunk; a reader that needs the text as one `&str` (every string
//! operation of Tomato) flattens it first. Medians of 5 runs; flat chains
//! that would copy more than 4 GB are not run.

use std::cell::RefCell;
use std::hint::black_box;
use std::rc::Rc;
use std::time::Instant;

use okkama_persistent::Trie;

const CHUNK: usize = 1024;

fn median(mut work: impl FnMut() -> usize) -> f64 {
    black_box(work());
    let mut t: Vec<f64> = (0..5)
        .map(|_| {
            let s = Instant::now();
            black_box(work());
            s.elapsed().as_nanos() as f64
        })
        .collect();
    t.sort_by(f64::total_cmp);
    t[2]
}

fn flat(text: &Rc<str>, piece: &str) -> Rc<str> {
    let mut joined = String::with_capacity(text.len() + piece.len());
    joined.push_str(text);
    joined.push_str(piece);
    Rc::from(joined)
}

/// A version: its length in a buffer that versions share.
#[derive(Clone)]
struct Buffer {
    bytes: Rc<RefCell<String>>,
    len: usize,
}

impl Buffer {
    fn new() -> Self {
        Self { bytes: Rc::new(RefCell::new(String::new())), len: 0 }
    }

    fn concat(&self, piece: &str) -> Self {
        let mut bytes = self.bytes.borrow_mut();
        if bytes.len() == self.len {
            bytes.push_str(piece);
            return Self { bytes: Rc::clone(&self.bytes), len: self.len + piece.len() };
        }
        let mut copy = String::with_capacity(2 * (self.len + piece.len()));
        copy.push_str(&bytes[..self.len]);
        copy.push_str(piece);
        Self { bytes: Rc::new(RefCell::new(copy)), len: self.len + piece.len() }
    }

    fn with<R>(&self, read: impl FnOnce(&str) -> R) -> R {
        read(&self.bytes.borrow()[..self.len])
    }
}

/// Full chunks in a persistent vector and a last chunk of its own.
#[derive(Clone)]
struct Rope {
    chunks: Trie<Rc<str>>,
    last: Rc<str>,
    len: usize,
}

impl Rope {
    fn new() -> Self {
        Self { chunks: Trie::new(), last: Rc::from(""), len: 0 }
    }

    fn concat(&self, piece: &str) -> Self {
        let len = self.len + piece.len();
        if self.last.len() + piece.len() <= CHUNK {
            return Self { chunks: self.chunks.clone(), last: flat(&self.last, piece), len };
        }
        Self { chunks: self.chunks.push(Rc::clone(&self.last)), last: Rc::from(piece), len }
    }

    /// The text as one `&str`: what a Tomato string operation reads.
    fn with<R>(&self, read: impl FnOnce(&str) -> R) -> R {
        let mut text = String::with_capacity(self.len);
        self.chunks.for_each_chunk(|chunks| chunks.iter().for_each(|chunk| text.push_str(chunk)));
        text.push_str(&self.last);
        read(&text)
    }

    /// Code points without flattening: a walk over the chunks.
    fn chars(&self) -> usize {
        let mut count = self.last.chars().count();
        self.chunks.for_each_chunk(|chunks| count += chunks.iter().map(|c| c.chars().count()).sum::<usize>());
        count
    }
}

fn main() {
    let large = "абв".repeat(170) + "x";
    let pieces: [(&str, &str); 3] = [("1 byte", "x"), ("short UTF-8", "жёлтый "), ("1 KB", &large)];
    let needle = "needle that is not there";
    for (label, piece) in pieces {
        println!("== pieces of {label} ({} bytes)", piece.len());
        for n in [1_000usize, 16_000, 100_000] {
            let mut line = format!("n={n:<7}");
            let copied = (n as f64) * (n as f64) * piece.len() as f64 / 2.0;
            let flat_ns = (copied <= 4e9).then(|| {
                median(|| {
                    let mut text: Rc<str> = Rc::from("");
                    for _ in 0..n {
                        text = flat(&text, piece);
                    }
                    text.len()
                }) / n as f64
            });
            let buffer_ns = median(|| {
                let mut text = Buffer::new();
                for _ in 0..n {
                    text = text.concat(piece);
                }
                text.len
            }) / n as f64;
            let rope_ns = median(|| {
                let mut text = Rope::new();
                for _ in 0..n {
                    text = text.concat(piece);
                }
                text.len
            }) / n as f64;
            match flat_ns {
                Some(ns) => line += &format!("  append ns/piece: flat {ns:9.1}"),
                None => line += "  append ns/piece: flat    (O(n²))",
            }
            line += &format!("  buffer {buffer_ns:6.1}  rope {rope_ns:6.1}");
            // Reads of the result, ns per byte: as one `&str` (what every
            // string operation of Tomato takes) and, for the rope, a walk.
            let (mut b, mut r) = (Buffer::new(), Rope::new());
            for _ in 0..n {
                b = b.concat(piece);
                r = r.concat(piece);
            }
            let bytes = b.len as f64;
            let b_chars = median(|| b.with(|s| s.chars().count())) / bytes;
            let r_chars = median(|| r.with(|s| s.chars().count())) / bytes;
            let r_walk = median(|| r.chars()) / bytes;
            let b_find = median(|| b.with(|s| usize::from(s.contains(needle)))) / bytes;
            let r_find = median(|| r.with(|s| usize::from(s.contains(needle)))) / bytes;
            line += &format!(
                "  | chars ns/byte: buffer {b_chars:.3} rope {r_chars:.3} (walk {r_walk:.3})  find: buffer {b_find:.3} rope {r_find:.3}"
            );
            println!("{line}");
        }
        // Branches: a new version of one long shared text per step.
        let mut base = Buffer::new();
        let mut rope = Rope::new();
        for _ in 0..16_000 {
            base = base.concat(piece);
            rope = rope.concat(piece);
        }
        let k = 2_000;
        let b_branch = median(|| (0..k).map(|_| base.concat(piece).len).sum()) / k as f64;
        let r_branch = median(|| (0..k).map(|_| rope.concat(piece).len).sum()) / k as f64;
        println!("  branch of a {}-byte text: buffer {b_branch:.0} ns (a copy), rope {r_branch:.0} ns", base.len);
    }
}
