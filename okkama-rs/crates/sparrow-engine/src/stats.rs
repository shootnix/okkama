//! Execution counters for `sparrow-next --stats`. Diagnostics only: disabled
//! counters cost one relaxed load, and nothing here influences semantics.
//! Counters are per thread; the session runs its tasks on the calling thread.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static ENABLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static STATS: RefCell<Stats> = RefCell::new(Stats::default());
}

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub jit_entries: u64,
    pub cold_steps: u64,
    /// JIT exit statuses by name.
    pub exits: BTreeMap<&'static str, u64>,
    /// Operations executed by the Rust service path, by `Op` variant.
    pub service: BTreeMap<String, u64>,
    /// Operations executed by heap calls from machine code, by `Op` variant.
    pub heap_calls: BTreeMap<String, u64>,
    pub calls: u64,
    /// Invoked functions by kind and symbol.
    pub callees: BTreeMap<String, u64>,
    pub collections: u64,
    pub collection_time: Duration,
    /// Values surviving the most recent collection.
    pub live: u64,
    pub peak_live: u64,
}

pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn take() -> Stats {
    STATS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

#[inline]
pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

#[inline]
pub(crate) fn record(update: impl FnOnce(&mut Stats)) {
    if enabled() {
        STATS.with(|s| update(&mut s.borrow_mut()));
    }
}

/// Variant name of a plan instruction from its Debug form, refined by a leading
/// operation kind: `Arithmetic(Modulo, ..)` becomes `Arithmetic.Modulo`.
pub(crate) fn op_name(op: &impl std::fmt::Debug) -> String {
    let text = format!("{op:?}");
    let ident = |s: &str| {
        s.find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(s.len())
    };
    let end = ident(&text);
    let rest = &text[end..];
    match rest.strip_prefix('(') {
        Some(inner) if inner.starts_with(|c: char| c.is_ascii_uppercase()) => {
            format!("{}.{}", &text[..end], &inner[..ident(inner)])
        }
        _ => text[..end].to_owned(),
    }
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "jit entries      {}", self.jit_entries)?;
        writeln!(f, "cold steps       {}", self.cold_steps)?;
        writeln!(f, "calls            {}", self.calls)?;
        writeln!(
            f,
            "collections      {} ({:.3} ms, live {}, peak live {})",
            self.collections,
            self.collection_time.as_secs_f64() * 1e3,
            self.live,
            self.peak_live
        )?;
        let mut exits: Vec<_> = self.exits.iter().collect();
        exits.sort_by(|a, b| b.1.cmp(a.1));
        for (name, count) in exits {
            writeln!(f, "exit {name:<16} {count}")?;
        }
        let mut service: Vec<_> = self.service.iter().collect();
        service.sort_by(|a, b| b.1.cmp(a.1));
        for (name, count) in service.into_iter().take(20) {
            writeln!(f, "service {name:<13} {count}")?;
        }
        let mut callees: Vec<_> = self.callees.iter().collect();
        callees.sort_by(|a, b| b.1.cmp(a.1));
        for (name, count) in callees.into_iter().take(25) {
            writeln!(f, "callee {name:<14} {count}")?;
        }
        let mut calls: Vec<_> = self.heap_calls.iter().collect();
        calls.sort_by(|a, b| b.1.cmp(a.1));
        for (name, count) in calls.into_iter().take(20) {
            writeln!(f, "heap call {name:<11} {count}")?;
        }
        Ok(())
    }
}
