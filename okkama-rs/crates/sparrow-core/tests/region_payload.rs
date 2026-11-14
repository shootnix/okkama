//! GH-09: in-process cleanup oracles; run this file with GC stress too.
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

struct Code(Vec<String>);
impl Code {
    fn emit(&mut self, instruction: impl Into<String>) { self.0.push(instruction.into()); }
    fn text(&self) -> String {
        self.0.iter().enumerate().map(|(pc, i)| format!("    {pc}: {i}\n")).collect()
    }
}

#[derive(Clone, Copy)]
enum Transfer {
    None,
    Even,
    All,
}
impl Transfer {
    fn moves(self, n: usize) -> bool {
        match self {
            Self::None => false,
            Self::Even => n.is_multiple_of(2),
            Self::All => true,
        }
    }
}

fn fixture(count: usize, transfer: Transfer, keep: bool, asynchronous: bool, failing: bool) -> String {
    // Each destroy appends to the source region (reallocating its overflow),
    // transfers that fresh child into a nested region and closes it. The old
    // close cursor must see the original prefix and reacquire a fresh view.
    let mut source = include_str!("region_payload.spa").to_owned();
    source.push_str(&format!(r#"
function f3 "Res.destroy" {} {{
    registers [t3, t0, t1, t0, t2, t6, t2]
    params [r0]
    result void
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: obj.get r6, r0, 2
    3: region.enter r4
    4: const r3, str "!"
    5: obj.new r5, obj3, [r3, r2]
    6: region.adopt r6, r5
    7: region.transfer r6, r4, r5
    8: region.leave r4, _
    9: call r3, f1, [r2]
    10: str.concat r3, r3, r1
    11: call _, f2, [r2, r3]
    12: {}
}}
function f4 "test.is_string" intrinsic "okkama.test.is" [] [] {{
    registers [t0, t0]
    params [r0, r1]
    result void
}}
function f5 "Nested.destroy" {{
    registers [t6, t0, t1, t0]
    params [r0]
    result void
    0: obj.get r1, r0, 0
    1: obj.get r2, r0, 1
    2: call r3, f1, [r2]
    3: str.concat r3, r3, r1
    4: call _, f2, [r2, r3]
    5: ret _
}}
"#, if asynchronous { "async" } else { "" },
        if failing { "trap confirm, \"cleanup exploded\"" } else { "ret _" }));
    let mut code = Code(Vec::new());
    code.emit("const r0, str \"\"");
    code.emit("call r1, f0, [r0]");
    code.emit("region.enter r2");
    code.emit("region.enter r3");
    code.emit("region.enter r4");
    let mut names = Vec::new();
    for n in 0..count {
        let name = format!("{n:03},");
        names.push(name.clone());
        code.emit(format!("const r0, str \"{name}\""));
        // Keep the transfer oracle's graph independent of source entries.
        let parent = if matches!(transfer, Transfer::None) { "r2" } else { "r4" };
        code.emit(format!("obj.new r{}, obj2, [r0, r1, {parent}]", 8 + n));
        for _ in 0..2 { code.emit(format!("region.track r2, r{}", 8 + n)); }
        code.emit(format!("region.adopt r2, r{}", 8 + n));
        code.emit(format!("region.track r2, r{}", 8 + n));
        if transfer.moves(n) {
            code.emit(format!("region.transfer r2, r3, r{}", 8 + n));
            code.emit(format!("region.transfer r2, r3, r{}", 8 + n));
            code.emit(format!("region.transfer r3, r3, r{}", 8 + n));
        }
    }
    if keep {
        let items = (0..count).map(|n| format!("r{}", 8 + n)).collect::<Vec<_>>().join(", ");
        code.emit(format!("arr.new r6, [{items}]"));
    }
    let start = code.0.len();
    let close = if asynchronous { "region.settle" } else { "region.leave" };
    code.emit(format!("{close} r2, {}", if keep { "r6" } else { "_" }));
    if keep { code.emit("region.adopt r3, r6"); }
    code.emit(format!("{close} r3, _"));
    code.emit(format!("{close} r4, _"));
    let end = code.0.len();
    code.emit("call r0, f1, [r1]");
    let order: Vec<_> = if keep { (0..count).rev().collect() } else {
        (0..count).rev().filter(|n| !transfer.moves(*n))
            .chain((0..count).rev().filter(|n| transfer.moves(*n))).collect()
    };
    let expected: String = order.iter().map(|&n| format!("!{}", names[n])).collect();
    code.emit(format!("const r7, str \"{expected}\""));
    code.emit("call _, f4, [r0, r7]");
    code.emit("ret _");
    let handler = code.0.len();
    let unwind = if asynchronous { "region.settle_unwind" } else { "region.unwind" };
    code.emit(format!("{unwind} r5, r2, r5"));
    code.emit(format!("{unwind} r5, r3, r5"));
    code.emit(format!("{unwind} r5, r4, r5"));
    code.emit("call r0, f1, [r1]");
    code.emit(format!("const r7, str \"{expected}\""));
    code.emit("call _, f4, [r0, r7]");
    code.emit("raise r5");
    let registers = ["t0", "t1", "t2", "t2", "t2", "t5", "t4", "t0"]
        .into_iter().chain(std::iter::repeat_n("t3", count)).collect::<Vec<_>>().join(", ");
    source.push_str(&format!("function f6 \"cleanup oracle\" {} {{\n    registers [{registers}]\n    params []\n    result void\n    handler {start} {end} @{handler} r5\n{} }}\ntests {{ test \"cleanup\" f6 }}\n",
        if asynchronous { "async" } else { "" }, code.text()));
    source
}

fn check(count: usize, transfer: Transfer, keep: bool, asynchronous: bool, failing: bool) {
    let source = fixture(count, transfer, keep, asynchronous, failing);
    let program = okkama_sparrow::assemble(&source).unwrap();
    let output = Output(Arc::default());
    let passed = sparrow_core::run_tests(okkama_sparrow::encode(&program).unwrap(), Box::new(output.clone())).unwrap();
    let report = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    let failures = if failing { count } else { 0 };
    assert_eq!(passed, failures == 0, "{report}");
    assert_eq!(report.matches("cleanup exploded").count(), failures, "{report}");
    assert_eq!(report.matches("Secondary cleanup failure:").count(), failures.saturating_sub(1), "{report}");
    // A failed log assertion would add another indented report entry.
    assert_eq!(report.lines().filter(|line| line.starts_with("    ") && !line.trim().is_empty()).count(),
        if failures == 0 { 0 } else { failures * 2 - 1 }, "{report}");
}

#[test]
fn gh09_region_boundaries_duplicates_transfers_and_keeps() {
    for count in [0, 1, 4, 5, 256] {
        for (transfer, keep) in [(Transfer::None, false), (Transfer::Even, false), (Transfer::None, true)] {
            check(count, transfer, keep, false, false);
        }
    }
}

#[test]
fn gh09_region_failing_and_async_nested_cleanup() {
    for asynchronous in [false, true] {
        for failing in [false, true] {
            check(9, Transfer::None, false, asynchronous, failing);
            check(9, Transfer::Even, false, asynchronous, failing);
        }
    }
}

#[test]
fn gh09_region_full_transfer_stale_source() {
    for count in [0, 5, 256] {
        check(count, Transfer::All, false, false, false);
    }
}
