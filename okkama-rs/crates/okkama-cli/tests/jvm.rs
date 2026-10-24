//! Real class-file verification/execution. Requires java, javap and javac on PATH.
//! No Java compiler is used by the Okkama backend: javac only builds the test probe.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use okkama_compiler::SourceFile;
use okkama_compiler::codegen::{self, Backend, Options};
use okkama_compiler::ir::{Constant, InstKind, IntValue};
use okkama_compiler::lowering::lower_program;
use okkama_compiler::syntax::{self, ParseOptions};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct WorkDir(PathBuf);

impl WorkDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "okkama-jvm-test-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn build(&self, name: &str, body: &str) -> PathBuf {
        self.build_source(
            name,
            &format!("module app v1.0.0; main() is int {{ {body} }}"),
        )
    }

    /// Собрать полный исходник модуля в `<name>.class` (и классы объектов рядом).
    fn build_source(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(format!("{name}.class"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_okkama"))
            .args(["build", "-", "--backend", "jvm", "-o"])
            .arg(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        write!(child.stdin.take().unwrap(), "{source}").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        path
    }

    fn java(&self, class: &str, args: &[&str]) -> Output {
        Command::new("java")
            .arg("-Xverify:all")
            .arg("-cp")
            .arg(&self.0)
            .arg(class)
            .args(args)
            .output()
            .expect("JVM integration tests require a JDK with java on PATH")
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn disassemble(path: &Path) -> String {
    let output = Command::new("javap")
        .args(["-c", "-verbose"])
        .arg(path)
        .output()
        .expect("JVM integration tests require a JDK with javap on PATH");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Собрать программу из `main`-тела, запустить и вернуть (код, stdout, stderr).
fn run_main(dir: &WorkDir, name: &str, body: &str) -> (Option<i32>, String, String) {
    dir.build(name, body);
    let output = dir.java(name, &[]);
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn one_plus_one_builds_verifies_and_exits_with_two() {
    let dir = WorkDir::new();
    let path = dir.build("Arithmetic", "1 + 1;");
    let listing = disassemble(&path);
    assert!(listing.contains("major version: 52"));
    assert!(listing.contains("public static java.math.BigInteger okkamaMain()"));
    assert!(
        listing.contains("java/math/BigInteger.add:(Ljava/math/BigInteger;)Ljava/math/BigInteger;")
    );
    assert!(listing.contains("areturn"));
    assert!(listing.contains("public static void main(java.lang.String[])"));

    let (code, out, err) = run_main(&dir, "Arithmetic", "1 + 1;");
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty());
    assert!(err.is_empty());
}

#[test]
fn control_flow_programs_exit_with_their_computed_status() {
    let dir = WorkDir::new();
    for (index, (body, expected)) in [
        ("let x = 7; if x > 5 { 1; } else { 2; }", 1),
        ("let x = 3; if x > 5 { 1; } else if x == 3 { 3; } else { 2; }", 3),
        ("let v = if 1 < 2 && 2 < 3 { 10; } else { 20; }; v + 1;", 11),
        ("let v = if 1 > 2 || !(2 < 3) { 10; } else { 20; }; v;", 20),
        ("let n = loop { last 42; }; n;", 42),
        ("for 1..=3 -> i { if i == 2 { return i * 10; } } 0;", 20),
        ("for 0..5 -> i { next if i < 4; return i; } 99;", 4),
        ("for 1..=10 -> i { last if i == 3; confirm i < 3; } 7;", 7),
        ("let a = loop { loop { last; } last 5; }; a;", 5),
        ("for 0..0 -> i { return 1; } 6;", 6),
        ("(6 & 3) + (6 | 3) * 10 + (6 ^ 3) * 100 - 500;", 2 + 70 + 500 - 500),
        ("(1 << 6) + (-128 >> 3) + 16;", 64 - 16 + 16),
        ("(-1 >>> 60) + ~(-9);", 15 + 8),
        // `%` даёт остаток со знаком делителя: 2 + (-2) + 10.
        ("-7 % 3 + 7 % -3 + 10;", 10),
        // Знак делимого, как `%` Go (D-031).
        ("let a = -7 % 3; let b = 7 % -3; let c = -6 % 3; if a == -1 && b == 1 && c == 0 { 3; } else { 0; }", 3),
        (r#"if "a\r".chomp() == "a\r" && "a\r\n".chomp() == "a" { 42; } else { 0; }"#, 42),
        ("let s = \"ab\" + \"c\"; if s == \"abc\" && s != \"x\" { 9; } else { 0; }", 9),
        ("let t = true; let f = false; if (t == f) == false { 4; } else { 0; }", 4),
        ("if 3 <= 3 && 3 >= 3 && !(3 < 3) && !(3 > 3) { 8; } else { 0; }", 8),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("Flow{index}");
        let (code, out, err) = run_main(&dir, &name, body);
        assert_eq!(code, Some(expected), "{body}: {err}");
        assert!(out.is_empty(), "{body}");
        assert!(err.is_empty(), "{body}: {err}");
    }
}

#[test]
fn integer_arithmetic_is_exact_beyond_sixty_four_bits() {
    let dir = WorkDir::new();
    for (index, body) in [
        "let max = 9223372036854775807; confirm max + 1 == 9223372036854775808; 0;",
        "let min = -9223372036854775807 - 1; confirm min - 1 == -9223372036854775809; 0;",
        "let x = 340282366920938463463374607431768211456; confirm x * x - x * x == 0; 0;",
        "let big = 18446744073709551616; confirm big % 7 == 2; 0;",
    ]
    .into_iter()
    .enumerate()
    {
        let (code, _, err) = run_main(&dir, &format!("Exact{index}"), body);
        assert_eq!(code, Some(0), "{body}: {err}");
    }
}

#[test]
fn runtime_failures_print_a_diagnostic_and_exit_one() {
    let dir = WorkDir::new();
    for (index, (body, expected)) in [
        ("1 % 0;", "RuntimeError: Modulo by zero"),
        ("confirm 1 == 2; 0;", "RuntimeError: confirm failed"),
        ("confirm 1 == 2 else panic \"broken\"; 0;", "Panic: broken"),
        ("panic \"stop\";", "Panic: stop"),
        (
            "for 3..1 -> i { 0; } 0;",
            "RuntimeError: Range start must be less than or equal to range end",
        ),
        ("1 << 64;", "RuntimeError: Shift count must be in 0..63"),
        ("1 << -1;", "RuntimeError: Shift count must be in 0..63"),
        (
            "9223372036854775808 & 1;",
            "RuntimeError: Bitwise operand is outside signed-64 range",
        ),
        (
            "256;",
            "RuntimeError: main() must return an int in 0..255, got 256",
        ),
        (
            "-1;",
            "RuntimeError: main() must return an int in 0..255, got -1",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, out, err) = run_main(&dir, &format!("Failure{index}"), body);
        assert_eq!(code, Some(1), "{body}: {err}");
        assert!(out.is_empty(), "{body}");
        assert_eq!(err.trim_end(), expected, "{body}");
    }
}

#[test]
fn wide_local_indices_and_long_constants_execute() {
    let dir = WorkDir::new();
    // Значение %260 лежит в слоте 260: нужен префикс wide.
    let body = (0..260)
        .map(|value| format!("{value}; "))
        .collect::<String>()
        + "40 + 2;";
    let path = dir.build("Wide", &body);
    let listing = disassemble(&path);
    assert!(listing.contains("astore_w"), "{listing}");
    assert!(listing.contains("aload_w"), "{listing}");
    assert!(listing.contains("ldc2_w"));
    let output = dir.java("Wide", &[]);
    assert_eq!(
        output.status.code(),
        Some(42),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn exact_results_and_unicode_class_names_are_observable() {
    let dir = WorkDir::new();
    // Reflection observes the exact BigInteger result, independently of the
    // OS exit status. Okkama class files themselves are emitted directly by Rust.
    let probe = dir.0.join("JvmProbe.java");
    std::fs::write(
        &probe,
        r#"
public final class JvmProbe {
    public static void main(String[] args) throws Exception {
        Class<?> type = Class.forName(args[0]);
        String method = args.length > 1 ? args[1] : "okkamaMain";
        System.out.println(type.getMethod(method).invoke(null));
    }
}
"#,
    )
    .unwrap();
    let output = Command::new("javac")
        .arg(&probe)
        .output()
        .expect("JVM integration tests require a JDK with javac on PATH");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for (name, body, expected) in [
        ("Zero", "0;", "0"),
        ("Maximum", "9223372036854775807;", "9223372036854775807"),
        ("Beyond", "9223372036854775807 + 1;", "9223372036854775808"),
        (
            "Huge",
            "340282366920938463463374607431768211456 * 2;",
            "680564733841876926926749214863536422912",
        ),
        (
            "HugeLiteral",
            "340282366920938463463374607431768211456;",
            "340282366920938463463374607431768211456",
        ),
        ("Large", "4294967296 + 7;", "4294967303"),
        ("Nested", "7; 2 + 3 + 4;", "9"),
        ("Explicit", "return 42;", "42"),
        ("Пример𐐀", "1 + 1;", "2"),
    ] {
        dir.build(name, body);
        let output = dir.java("JvmProbe", &[name]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
        assert!(output.stderr.is_empty());
    }

    // IR may already contain additional functions even though source syntax
    // currently permits only module-level main. Every function must survive.
    let source = SourceFile::new("test.ok", "module app v1.0.0; main() is int { 1; }").unwrap();
    let ast = syntax::parse(&source, &ParseOptions::default()).unwrap();
    let mut module = lower_program(&ast).unwrap();
    let mut helper = module.functions[0].clone();
    helper.symbol = "helper".to_string();
    helper.attrs.clear();
    helper.blocks[0].insts[0].kind = InstKind::Const(Constant::Int(IntValue::from_i64(-42)));
    module.functions.push(helper);
    let bytes = codegen::generate(
        &module,
        Backend::Jvm,
        &Options {
            artifact_name: "Helpers".to_string(),
        },
    )
    .unwrap();
    for file in &bytes.files {
        std::fs::write(dir.0.join(&file.name), &file.bytes).unwrap();
    }
    for (method, expected) in [("okkamaMain", "1"), ("helper", "-42")] {
        let output = dir.java("JvmProbe", &["Helpers", method]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
    }
}

const COUNTER: &str = "module app v1.0.0;
object C {
    has n is int = 0;
    has m is int = 0;
    C() {}
    C(n is int) { n = n; confirm self.n >= 0; }
    inc() is ^C { ^n = n + 1; ^m = n; ^n = n + 10; }
    value() is int { n * 100 + m; }
    bump(by is int) is ^C { if by > 5 { ^n = n + by; } }
    add(value is int) is int { n + value; }
    add(value is string) is int { n; }
    twice() is int { self.helper() * 2; }
    private:
    helper() is int { n; }
}
object Pair {
    has left is C;
    has right is int = 7;
    Pair(:left, :right);
    total() is int { left.value() + right; }
}
";

#[test]
fn objects_become_final_classes_next_to_the_main_class() {
    let dir = WorkDir::new();
    let path = dir.build_source(
        "Objects",
        &format!("{COUNTER} main() is int {{ C().value(); }}"),
    );
    let listing = disassemble(&path);
    assert!(
        listing.contains("public static java.math.BigInteger C$value(Objects$C)"),
        "{listing}"
    );
    assert!(
        listing.contains("public static Objects$C C$inc(Objects$C)"),
        "{listing}"
    );
    let counter = disassemble(&dir.0.join("Objects$C.class"));
    assert!(counter.contains("final class Objects$C"), "{counter}");
    assert!(counter.contains("  java.math.BigInteger n;"), "{counter}");
    assert!(counter.contains("  java.math.BigInteger m;"), "{counter}");
    assert!(
        counter.contains("Objects$C(java.math.BigInteger, java.math.BigInteger)"),
        "{counter}"
    );
    let pair = disassemble(&dir.0.join("Objects$Pair.class"));
    assert!(pair.contains("  Objects$C left;"), "{pair}");
    let output = dir.java("Objects", &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn object_programs_compute_the_same_results_as_the_interpreter() {
    let dir = WorkDir::new();
    // Ожидаемые значения сверены с okkama-pl на тех же программах.
    for (index, (main, expected)) in [
        (
            "let c = C(); for 1..=3 -> i { c = c.inc(); } c.value() - 3000;",
            20,
        ),
        (
            "let c = C(); for 1..=3 -> i { c = c.inc(); } \
             if c.value() > 0 { c = c.bump(by: 7); } else { c = c.bump(1); } c.value() - 3700;",
            20,
        ),
        ("let d = C(2).bump(2); d.value() - 100;", 100),
        ("Pair(left: C(5)).total() - 256;", 251),
        ("Pair(left: C(), right: 1).total();", 1),
        ("C(3).twice() + C(4).add(1) + C(9).add(\"x\");", 6 + 5 + 9),
        (
            "let c = C(); let d = c.inc(); for 1..=2 -> i { d = d.inc(); } d.value() - 3000;",
            20,
        ),
        (
            "let c = C(); let d = c.inc(); loop { d = d.inc(); last; } d.value() - 2000;",
            10,
        ),
        (
            "let c = C(); let d = c.inc(); if 1 > 0 { d = d.inc(); } d.value() - 2000;",
            10,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("Program{index}");
        dir.build_source(&name, &format!("{COUNTER} main() is int {{ {main} }}"));
        let output = dir.java(&name, &[]);
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{main}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "{main}");
    }
}

#[test]
fn a_failed_constructor_invariant_is_a_runtime_error() {
    let dir = WorkDir::new();
    dir.build_source(
        "Invariant",
        &format!("{COUNTER} main() is int {{ C(-1).value(); }}"),
    );
    let output = dir.java("Invariant", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "RuntimeError: confirm failed"
    );
}

/// Программы сверены с okkama-pl: те же исходники дают те же коды завершения.
const TYPED_FAILURES: &str = r#"module app v1.0.0;
object Boom does Error { has code is int; Boom(:code); text() is string { "boom " + code.string(); } code_value() is int { code; } }
object Other does Error { Other(); text() is string { "other"; } }
object P { P(); risky(x is int) is int { confirm x > 0 else throw Boom(code: x); x; } }
main() is int {
    let p = P();
    let a = try { p.risky(-3); } catch e is Boom { e.code_value() * -1; };
    let b = try { p.risky(-4); } catch e is Error { 10; };
    let c = try { p.risky(5); } catch e is Boom { 0; };
    let d = try {
        try { p.risky(-1); } catch e is Other { 100; }
    } catch e is Boom { 20; };
    let f = try {
        try { p.risky(-2); } catch e is Boom { throw Other(); }
    } catch e is Other { 30; };
    a + b + c + d + f;
}
"#;

const FAILURES_AND_ROLES: &str = r#"module app v1.0.0;
object Boom does Error { has code is int; Boom(:code); text() is string { "boom"; } code_value() is int { code; } }
role Wallet { total() is int; add(extra is int) is int; }
object Purse does Wallet {
    has digit is int;
    Purse(:digit);
    total() is int { digit; }
    add(extra is int) is int { digit + extra; }
}
object Holder { has wallet is Wallet; Holder(:wallet); sum() is int { wallet.total() + wallet.add(extra: 1); } }
object C {
    has n is int = 0;
    C() {}
    inc() is ^C { ^n = n + 1; }
    value() is int { n; }
    risky(x is int) is int { confirm x > 0 else throw Boom(code: x); x; }
}
object Parsed {
    has v is int;
    Parsed(x is int) {
        try { v = C().risky(x); } catch e is Boom { v = 100 + e.code_value() * -1; }
    }
    value() is int { v; }
}
main() is int {
    let c = C();
    try { c = c.inc(); c.risky(-1); c = c.inc(); } catch e is Boom { 0; }
    let a = c.value();                             // 1
    let b = Parsed(-5).value() + Parsed(7).value(); // 105 + 7
    let h = Holder(wallet: Purse(digit: 10)).sum(); // 21
    let w is Wallet = Purse(digit: 3);
    let d = try {
        try { c.risky(-2) > 0; } catch e is Boom { throw; }
    } catch e is Error { e.text() == "boom"; };
    let f = if d { 1; } else { 0; };
    let g = try {
        try { c.risky(-3); } catch e is Boom { c.risky(-4); }
    } catch e is Boom { e.code_value(); };         // -4
    a + b + h + w.total() + f + g + 4 - 100;
}
"#;

#[test]
fn typed_failures_are_caught_by_type_and_by_the_error_role() {
    let dir = WorkDir::new();
    for (name, source, expected) in [
        ("Typed", TYPED_FAILURES, 68),
        ("Roles", FAILURES_AND_ROLES, 38),
    ] {
        dir.build_source(name, source);
        let output = dir.java(name, &[]);
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "{name}");
    }
    let roles = disassemble(&dir.0.join("Roles$Wallet.class"));
    assert!(roles.contains("interface Roles$Wallet"), "{roles}");
    let purse = disassemble(&dir.0.join("Roles$Purse.class"));
    assert!(purse.contains("implements Roles$Wallet"), "{purse}");
}

#[test]
fn an_uncaught_throw_prints_the_error_type_and_text() {
    let dir = WorkDir::new();
    dir.build_source(
        "Uncaught",
        "module app v1.0.0;
object Boom does Error { has code is int; Boom(:code); text() is string { \"boom \" + code.string(); } }
main() is int { throw Boom(code: 7); }",
    );
    let output = dir.java("Uncaught", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    // Как в okkama-pl: `<Тип>: <text()>`.
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "Boom: boom 7"
    );
}

#[test]
fn collection_programs_compute_the_same_results_as_the_interpreter() {
    let dir = WorkDir::new();
    // Ожидаемые коды сверены с okkama-pl на тех же программах.
    let program = "module app v1.0.0;
object P {
    has x is int;
    has y is int;
    P(:x, :y);
}
main() is int {
    let xs = [3, 1, 4, 1, 5];
    let ys = @int(0, xs..., 9);
    let doubled = xs.map() -> v { v * 2; };
    let odd = xs.filter() -> v { v % 2 == 1; };
    let sum = doubled.reduce(0) -> acc, v { acc + v; };
    let total = (1..=4).reduce(0) -> acc, n { acc + n; };
    for [\"a\", \"b\"] -> s {
        confirm s.length() == 1;
    }
    let words = \"a, b ,c\".split(\",\").map() -> w { w.trim(); };
    let joined = words.join(\"-\");
    let flags = [true, false, true].filter() -> f { f; };
    let hit = xs[2] else 100;
    let miss = xs[10] else 7;
    let same = [P(x: 1, y: 2), P(x: 3, y: 4)] == [P(x: 1, y: 2), P(x: 3, y: 4)];
    let differ = P(x: 1, y: 2) != P(x: 1, y: 3);
    let text = \"héllo wörld\".replace(\"l\", \"L\").slice(1, 6);
    let score = sum + total + odd.size() + ys.size() + hit + miss + joined.length()
        + flags.size() + text.length();
    if same && differ && joined == \"a-b-c\" && text == \"éLLo \" && \"x\\r\\n\".chomp() == \"x\" {
        score;
    } else {
        0;
    }
}";
    dir.build_source("Collections", program);
    let output = dir.java("Collections", &[]);
    assert_eq!(
        output.status.code(),
        Some(72),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Рантайм okkama.rt выдан рядом с программой.
    for class in ["Rt", "Failure", "Error"] {
        assert!(dir.0.join(format!("okkama/rt/{class}.class")).is_file());
    }

    for (index, (body, expected)) in [
        (
            "\"a,,b,\".split(\",\").size() * 10 + \"\".split(\",\").size();",
            41,
        ),
        ("\"héllo\".split(\"\").size();", 5),
        ("\"abc\".replace(\"\", \"-\").length();", 7),
        ("\"\".replace(\"\", \"-\").length();", 1),
        ("\"aXbXc\".replace_first(\"X\", \"--\").length();", 6),
        (
            "\"\u{2003} hi \\t\\n\".trim().length() + \"\u{a0}x\u{3000}\".trim().length() * 10;",
            12,
        ),
        ("\"x\\n\\n\".chomp().length();", 2),
        ("[4, 5].last() * 10 + [4, 5].first();", 54),
        ("(-7).abs() + (-7).sign() + 0.sign();", 6),
        ("let xs = [[1, 2], [3]]; xs.size() * 10 + xs[0].size();", 22),
        (
            "let xs = [\"b\", \"a\"]; if xs == [\"b\", \"a\"] && xs != [\"a\"] { 5; } else { 6; }",
            5,
        ),
        (
            "let ys = @string(1, true, \"x\"); if ys.join(\"\") == \"11x\" { 3; } else { 4; }",
            3,
        ),
        (
            "let xs = [1, 2]; let e = xs.filter() -> v { v > 9; }; \
             let f = xs.filter() -> v { v > 8; }; if e == f { 1; } else { 2; }",
            1,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, out, err) = run_main(&dir, &format!("Collection{index}"), body);
        assert_eq!(code, Some(expected), "{body}: {err}");
        assert!(out.is_empty() && err.is_empty(), "{body}: {err}");
    }
}

#[test]
fn collection_failures_use_the_interpreter_diagnostics() {
    let dir = WorkDir::new();
    for (index, (body, expected)) in [
        ("let xs = [1, 2]; xs.item(5);", "Array index out of bounds"),
        (
            "let xs = [1, 2]; xs.slice(-1, 1).size();",
            "Slice start must not be negative",
        ),
        (
            "let xs = [1, 2]; xs.slice(2, 1).size();",
            "Slice start must be less than or equal to slice end",
        ),
        (
            "let xs = [1, 2]; xs.slice(0, 3).size();",
            "Slice end must not exceed length",
        ),
        (
            "\"abc\".slice(1, 9).length();",
            "Slice end must not exceed length",
        ),
        (
            "let ys = [0, 5..2]; 0;",
            "Range start must be less than or equal to range end",
        ),
        (
            "let e = [1].filter() -> v { v > 5; }; e.first();",
            "Cannot read first item of an empty sequence",
        ),
        (
            "let e = [1].filter() -> v { v > 5; }; e.last();",
            "Cannot read last item of an empty sequence",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, out, err) = run_main(&dir, &format!("CollectionFailure{index}"), body);
        assert_eq!(code, Some(1), "{body}: {err}");
        assert!(out.is_empty(), "{body}");
        assert_eq!(
            err.trim_end(),
            format!("RuntimeError: {expected}"),
            "{body}"
        );
    }
}

#[test]
fn map_programs_compute_the_same_results_as_the_interpreter() {
    let dir = WorkDir::new();
    // Ожидаемые коды сверены с okkama-pl на тех же программах.
    let program = "module app v1.0.0;
object Stock {
    has items is %string:int;
    Stock(:items);
    count(name is string) is int { items[name] else 0; }
    names() is @string { items.keys(); }
}
object Util {
    Util() {}
    total(m is %string:int) is int { m.reduce(0) -> acc, k, v { acc + v; }; }
    pick(flag is bool) is %int:bool { if flag { [1: true]; } else { [:]; } }
}
main() is int {
    let s = Stock(items: [\"pear\": 3, \"apple\": 5]);
    let same = Stock(items: [\"apple\": 5, \"pear\": 3]) == s;
    let nested = [\"a\": [\"x\": 1], \"b\": [\"y\": 2, \"z\": 3]];
    let sizes = nested.map() -> k, v { v.size(); };
    let big = nested.filter() -> k, v { v.size() > 1; };
    let score = s.count(\"pear\") * 10 + s.count(\"fig\") + Util().total([\"q\": 7]) + sizes[1]
        + big.size() + Util().pick(true).size() + Util().pick(false).size();
    if same && s.names().join(\"\") == \"applepear\" { score; } else { 0; }
}";
    dir.build_source("Maps", program);
    let output = dir.java("Maps", &[]);
    assert_eq!(
        output.status.code(),
        Some(41),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for (index, (body, expected)) in [
        ("let m = [\"a\": 1, \"b\": 2]; m[\"z\"] else 7;", 7),
        (
            "let m = [\"a\": 1, \"a\": 2]; m[\"a\"] * 10 + m.size();",
            21,
        ),
        (
            "let m = [\"b\": 1, \"a\": 2]; if m == [\"a\": 2, \"b\": 1] { 1; } else { 2; }",
            1,
        ),
        (
            "let m = [\"b\": 1, \"a\": 2]; if m == [\"a\": 2, \"b\": 3] { 1; } else { 2; }",
            2,
        ),
        (
            "let m = [\"b\": 1, \"a\": 2]; m.reduce(0) -> acc, k, v { acc + v; };",
            3,
        ),
        (
            "let m = [\"b\": 1, \"a\": 2]; let r = m.filter() -> k, v { v > 1; }; \
             r.keys().first().length();",
            1,
        ),
        (
            "let m = [\"b\": 1, \"a\": 2]; let r = m.map() -> k, v { k; }; \
             if r.join(\"\") == \"ab\" { 9; } else { 8; }",
            9,
        ),
        ("let m = [true: 1, false: 2]; m.values().first();", 2),
        ("let m = [1: \"x\", 1: \"y\"]; m[1].length();", 1),
        (
            "let m = [\"a\": 1]; m.has(\"a\").string().length() + m.has(\"q\").string().length();",
            2,
        ),
        // Ключи `str` — по code points: U+FFEE раньше U+1F600 (в UTF-16 — наоборот).
        (
            "let m = [\"\u{1F600}\": 1, \"\u{FFEE}\": 2]; \
             if m.keys().join(\"\") == \"\u{FFEE}\u{1F600}\" { 5; } else { 6; }",
            5,
        ),
        (
            "let m = [\"x\": [1: true]]; if m[\"x\"][1] { 3; } else { 4; }",
            3,
        ),
        (
            "let e = [:]; e.keys().size() + e.has(\"a\").string().length();",
            1,
        ),
        ("let e = [:]; e[\"a\"] else 3;", 3),
        (
            "let e is %int:int = [:]; let f = e.filter() -> k, v { v > 0; }; f.size() + 2;",
            2,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, out, err) = run_main(&dir, &format!("Map{index}"), body);
        assert_eq!(code, Some(expected), "{body}: {err}");
        assert!(out.is_empty() && err.is_empty(), "{body}: {err}");
    }

    for (index, (body, expected)) in [
        ("let m = [\"a\": 1]; m[\"z\"];", "Map key not found: \"z\""),
        ("let m = [1: 1]; m[-5];", "Map key not found: -5"),
        ("let m = [true: 1]; m[false];", "Map key not found: 0"),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, _, err) = run_main(&dir, &format!("MapFailure{index}"), body);
        assert_eq!(code, Some(1), "{body}: {err}");
        assert_eq!(
            err.trim_end(),
            format!("RuntimeError: {expected}"),
            "{body}"
        );
    }
}

#[test]
fn byte_and_char_programs_compute_the_same_results_as_the_interpreter() {
    let dir = WorkDir::new();
    // Ожидаемые коды сверены с okkama-pl на тех же программах.
    for (index, (body, expected)) in [
        ("let b = byte(200); b.int() - 100;", 100),
        ("let b = 200.byte(); b.int() - 100;", 100),
        (
            "let b = byte(250); let c = byte(10); let s = b + c; s - 200;",
            60,
        ),
        (
            "let b = byte(3); let c = byte(4); if b < c { 1; } else { 2; }",
            1,
        ),
        ("let b = byte(7); b % byte(4);", 3),
        ("let b = byte(7); (b & byte(12)).int();", 4),
        ("let b = byte(255); (~b).int();", 0),
        ("let b = byte(200); let n = -b; n + 255;", 55),
        ("let b = byte(200); (b << 1).int();", 144),
        ("let b = byte(200); (b >>> 1).int();", 100),
        ("let b = byte(7); 10 - b.int();", 3),
        ("let b = byte(129); b.bit_count();", 2),
        ("let b = byte(1); b.rotate_right(1).int();", 128),
        (
            "let b = byte(16); b.leading_zeros() * 10 + b.trailing_zeros();",
            34,
        ),
        ("let b = byte(1); b.reverse_bits().int();", 128),
        (
            "let b = byte(1); (b ^ byte(3)).int() + (b | byte(4)).int();",
            7,
        ),
        ("let b = byte(7); b.less(byte(9)).string().length();", 1),
        ("let b = byte(200); b.string().length();", 3),
        ("let n = 300; n.bit_count();", 4),
        ("let n = 300; n.leading_zeros();", 55),
        ("let n = 1; n.rotate_left(63).string().length();", 20),
        ("let n = 1; n.swap_bytes().string().length();", 17),
        ("let n = 0; n.trailing_zeros();", 64),
        ("let n = -1; n.rotate_right(4).bit_count();", 64),
        ("let c = 'a'; if c != 'b' { 1; } else { 2; }", 1),
        ("let m = ['a': 1, 'c': 2]; m['c'];", 2),
        (
            "let m = ['\u{1F600}': 1, '\u{FFEE}': 2]; m.values().first();",
            2,
        ),
        (
            "let xs = ['a', 'b']; if xs == ['a', 'b'] { 1; } else { 2; }",
            1,
        ),
        ("let xs = [1, 2]; xs[byte(1).int()];", 2),
        ("let xs = @byte(1, 2, 3); xs[2].int();", 3),
        (
            "let c = '\u{1F600}'; let xs = [c]; if xs[0] == '\u{1F600}' { 6; } else { 7; }",
            6,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, out, err) = run_main(&dir, &format!("Scalar{index}"), body);
        assert_eq!(code, Some(expected), "{body}: {err}");
        assert!(out.is_empty() && err.is_empty(), "{body}: {err}");
    }

    for (index, (body, expected)) in [
        (
            "let n = 256; let b = byte(n); 0;",
            "Cannot convert int value 256 to byte: expected 0..255",
        ),
        (
            "let b = byte(-1); 0;",
            "Cannot convert int value -1 to byte: expected 0..255",
        ),
        (
            "let xs = @byte(1, 2, 300); 0;",
            "@byte() argument 3: Cannot convert int value 300 to byte: expected 0..255",
        ),
        (
            "let b = byte(200); (b << 8).int();",
            "Shift count must be in 0..7",
        ),
        (
            "let b = byte(1); b.rotate_left(9).int();",
            "Shift count must be in 0..7",
        ),
        (
            "let n = 1; n.rotate_left(64).bit_count();",
            "Shift count must be in 0..63",
        ),
        ("let m = ['a': 1]; m['z'];", "Map key not found: \"z\""),
        ("let m = [1: 1]; m[2];", "Map key not found: 2"),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, _, err) = run_main(&dir, &format!("ScalarFailure{index}"), body);
        assert_eq!(code, Some(1), "{body}: {err}");
        assert_eq!(
            err.trim_end(),
            format!("RuntimeError: {expected}"),
            "{body}"
        );
    }
}

#[test]
fn enum_and_match_programs_compute_the_same_results_as_the_interpreter() {
    let dir = WorkDir::new();
    let decls = "module app v1.0.0; enum Light = (red, amber, green); \
                 object Lamp { has light is Light; Lamp(:light); red() is bool { light == Light.red; } }";
    // Ожидаемые коды сверены с okkama-pl на тех же программах.
    for (index, (body, expected)) in [
        ("let l = Light.amber; if l == Light.amber { 1; } else { 2; }", 1),
        ("let l = Light.amber; if l != Light.amber { 1; } else { 2; }", 2),
        (
            "let l = Light.amber; match l { case Light.red { 1; } case Light.amber, Light.green { 2; } }",
            2,
        ),
        ("let l = Light.amber; let v = match l { case Light.red { 1; } else { 3; } }; v;", 3),
        ("let n = 2; let k = 2; match n { case k { 4; } else { 5; } }", 4),
        ("let s = \"b\"; match s { case \"a\" { 1; } case \"b\" { 2; } else { 3; } }", 2),
        ("let b = true; match b { case false { 1; } case true { 2; } }", 2),
        ("let c = 'x'; match c { case 'x' { 1; } else { 2; } }", 1),
        ("let y = byte(3); match y { case byte(3) { 1; } else { 2; } }", 1),
        ("let n = 1; match n { case 1 { 1; } case 1 { 2; } else { 3; } }", 1),
        ("let n = 1; match n { case 1 { return 9; } else { 2; } } 4;", 9),
        ("match 2 { case 2, 9 % 0 { 5; } else { 6; } }", 5),
        ("let xs = [1]; match xs { case [1] { 3; } else { 4; } }", 3),
        ("match 5 { case 1 { 1; } case 2 { 2; } } 7;", 7),
        (
            "let l = Light.green; \
             let v = match l { case Light.red { 1; } case Light.amber { 2; } case Light.green { 3; } }; v;",
            3,
        ),
        (
            "let ls = [Light.red, Light.green, Light.amber]; \
             let r = ls.map() -> l { match l { case Light.red { 1; } case Light.green { 10; } else { 100; } } }; \
             r.reduce(0) -> a, x { a + x; };",
            111,
        ),
        ("let xs = [Light.red, Light.green]; if xs == [Light.red, Light.green] { 1; } else { 2; }", 1),
        ("if Lamp(light: Light.red).red() && Lamp(light: Light.red) == Lamp(light: Light.red) { 3; } else { 4; }", 3),
        // Ключи enum — в порядке объявления case (R06; Perl упорядочивает по имени).
        ("let m = [Light.green: 1, Light.red: 2]; m.values().first();", 2),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("Match{index}");
        dir.build_source(&name, &format!("{decls} main() is int {{ {body} }}"));
        let output = dir.java(&name, &[]);
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{body}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "{body}");
    }
    dir.build_source(
        "MatchFailure",
        &format!("{decls} main() is int {{ let m = [Light.green: 1]; m[Light.amber]; }}"),
    );
    let output = dir.java("MatchFailure", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "RuntimeError: Map key not found: \"amber\""
    );
    // Класс enum лежит рядом с главным классом.
    assert!(dir.0.join("Match0$Light.class").is_file());
}

/// Объекты, чей `destroy` бросает `Boom(code)`: победивший отказ виден в
/// `catch` и даёт код завершения, поэтому порядок очистки наблюдаем без вывода.
const OWNERSHIP: &str = "module app v1.0.0;
object Boom does Error {
    has code is int;
    Boom(:code);
    text() is string { \"boom \" + code.string(); }
    code() is int { code; }
}
object Fragile { has code is int; Fragile(:code); destroy { throw Boom(code: code); } }
object Bad { Bad(); destroy { confirm false; } }
object Log {
    Log();
    look(f is Fragile) is int { 1; }
    keep(f is Fragile) is Fragile { f; }
    make(c is int) is Fragile { let t = Fragile(code: c + 100); Fragile(code: c); }
    fail(c is int) is void { throw Boom(code: c); }
    run(c is int) is int { defer Log().fail(c); let f = Fragile(code: c + 1); return c; }
    nested(c is int) is int {
        for 1..=3 -> i { let f = Fragile(code: c + i); if i == 2 { return 0; } }
        9;
    }
}
object Holder {
    has child is Fragile;
    has code is int;
    Holder(:child, :code);
    destroy { throw Boom(code: code); }
}
object T { has tag is int; T(:tag); next() is ^T { ^tag = tag + 1; } destroy { throw Boom(code: tag); } }
object Holder2 {
    has child is Fragile;
    Holder2(fail is bool) { child = Fragile(code: 5); confirm !fail else throw Boom(code: 9); }
}
object Counter { has value is int; Counter(:value); inc() is ^Counter { ^value = value + 1; } value() is int { value; } }
object Lost does Error {
    has code is int;
    Lost(:code);
    text() is string { \"lost\"; }
    code() is int { code; }
    destroy { throw Boom(code: code + 50); }
}
";

#[test]
fn ownership_cleanup_order_matches_the_interpreter() {
    let dir = WorkDir::new();
    let caught = "catch e is Boom { e.code(); }; r;";
    // Ожидаемые коды сверены с okkama-pl на тех же программах.
    for (index, (body, expected)) in [
        // destroy в обратном порядке регистрации; первый отказ — основной.
        (
            "let r = try { let a = Fragile(code: 1); let b = Fragile(code: 2); 0; }",
            2,
        ),
        // Отказ тела важнее отказа destroy.
        (
            "let r = try { let a = Fragile(code: 1); throw Boom(code: 9); 0; }",
            9,
        ),
        // Непривязанное значение принадлежит блоку.
        ("let r = try { Fragile(code: 5); 0; }", 5),
        ("let r = try { defer Fragile(code: 4); 0; }", 4),
        // Отказ defer важнее отказа destroy; все defer исполняются.
        (
            "let r = try { let bad = Fragile(code: 1); defer Log().fail(2); 0; }",
            2,
        ),
        (
            "let r = try { defer Log().fail(3); defer Log().fail(4); throw Boom(code: 1); 0; }",
            1,
        ),
        (
            "let r = try { defer Log().fail(3); defer Log().fail(4); 0; }",
            4,
        ),
        // Свежий аргумент принимает вызванное действие: destroy при его выходе.
        (
            "let r = try { let n = Log().look(Fragile(code: 3)); throw Boom(code: 8); }",
            3,
        ),
        (
            "let r = try { let f = Log().make(1); throw Boom(code: 7); }",
            101,
        ),
        (
            "let r = try { let k = Log().keep(Fragile(code: 6)); throw Boom(code: 2); }",
            2,
        ),
        // Родитель уничтожается раньше своих детей.
        (
            "let r = try { let h = Holder(child: Fragile(code: 1), code: 2); 0; }",
            2,
        ),
        // Rebind не уничтожает прежнее значение досрочно.
        (
            "let r = try { let t = T(tag: 1); t = t.next(); t = t.next(); 0; }",
            3,
        ),
        (
            "let r = try { for 1..=3 -> i { let f = Fragile(code: i); } 0; }",
            1,
        ),
        // Отказ конструктора: связанные поля уничтожаются, сам объект — нет.
        ("let r = try { let h = Holder2(fail: true); 0; }", 9),
        ("let r = try { let h = Holder2(fail: false); 0; }", 5),
        (
            "let r = try { let x = [Fragile(code: 3), Fragile(code: 4)]; 0; }",
            4,
        ),
        // Значение ветви `if`/`match`/`try` принадлежит внешнему блоку.
        (
            "let r = try { if true { let f = Fragile(code: 2); } throw Boom(code: 1); }",
            1,
        ),
        (
            "let n = 1; let r = try { match n { case 1 { let f = Fragile(code: 2); } else { 0; } } \
             throw Boom(code: 1); }",
            1,
        ),
        (
            "let r = try { { let f = Fragile(code: 2); } throw Boom(code: 1); }",
            2,
        ),
        // defer читает текущие значения привязок.
        (
            "let r = try { let c = Counter(value: 1); defer Log().fail(c.value()); \
             c = c.inc(); c = c.inc(); 0; }",
            3,
        ),
        ("let r = try { Log().run(5); }", 5),
        ("let r = try { Log().nested(10); }", 11),
        (
            "let r = try { try { throw Lost(code: 3); } catch e is Lost { e.code(); } }",
            53,
        ),
        (
            "let r = try { let x = loop { let f = Fragile(code: 3); last 5; }; x; }",
            3,
        ),
        (
            "let r = try { defer Log().fail(1); { defer Log().fail(2); } 0; }",
            2,
        ),
        ("let r = try { defer Log().fail(1); return 6; }", 1),
        // Отказ defer на `next` поднимается (OIR-1 §19.3); Perl теряет его и даёт 12.
        (
            "let r = try { for 1..=3 -> n { defer Log().fail(n + 10); next if n == 1; } 0; }",
            11,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("Owner{index}");
        dir.build_source(
            &name,
            &format!("{OWNERSHIP} main() is int {{ {body} {caught} }}"),
        );
        let output = dir.java(&name, &[]);
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{body}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for (index, (body, expected)) in [
        (
            "{ let a = Fragile(code: 1); let b = Bad(); } 0;",
            "RuntimeError: confirm failed\nSecondary cleanup failure:\n  Boom: boom 1",
        ),
        (
            "let r = Fragile(code: 1); throw Boom(code: 2);",
            "Boom: boom 2\nSecondary cleanup failure:\n  Boom: boom 1",
        ),
        (
            "let h = Holder2(fail: true); 0;",
            "Boom: boom 9\nSecondary cleanup failure:\n  Boom: boom 5",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("OwnerFailure{index}");
        dir.build_source(&name, &format!("{OWNERSHIP} main() is int {{ {body} }}"));
        let output = dir.java(&name, &[]);
        assert_eq!(output.status.code(), Some(1), "{body}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim_end(),
            expected,
            "{body}"
        );
    }
}

/// Std репозитория: импорты тестов не зависят от установки.
fn stdlib() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std")
}

impl WorkDir {
    /// Записать файлы `(путь, текст)` и собрать `entry` в `<class>.class`.
    fn build_files(&self, files: &[(&str, &str)], entry: &str, class: &str) -> Output {
        for (path, text) in files {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        Command::new(env!("CARGO_BIN_EXE_okkama"))
            .args(["build"])
            .arg(self.0.join(entry))
            .args(["--backend", "jvm", "-o"])
            .arg(self.0.join(format!("{class}.class")))
            .env("OKKAMA_STDLIB_DIR", stdlib())
            .output()
            .unwrap()
    }
}

#[test]
fn imported_modules_and_std_stdout_run_on_the_jvm() {
    let dir = WorkDir::new();
    let build = dir.build_files(
        &[
            (
                "lib/shapes.ok",
                "module lib/shapes v1.0.0;\n\
                 object Square { has side is int; Square(:side); area() is int { side * side; } }\n",
            ),
            (
                "main.ok",
                "module app v1.0.0;\n\
                 import \"io/stdout\" take Stdout;\n\
                 import \"lib/shapes\" take Square;\n\
                 main() is int {\n\
                     let out = Stdout();\n\
                     out.writeln(\"hello, \", \"world\");\n\
                     out.write(\"a\", \"b\");\n\
                     out.writeln();\n\
                     out.writeln(\"площадь \", Square(side: 6).area().string(), \" 😀\");\n\
                     Square(side: 3).area();\n\
                 }\n",
            ),
        ],
        "main.ok",
        "Hello",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Hello", &[]);
    assert_eq!(output.status.code(), Some(9));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "hello, world\nab\nплощадь 36 😀\n"
    );
    // Классы импортированного объекта и рантайма лежат рядом с программой.
    assert!(dir.0.join("Hello$lib$shapes$$Square.class").is_file());
    assert!(dir.0.join("okkama/rt/Natives.class").is_file());
}

#[test]
fn import_errors_point_at_the_importing_file() {
    let dir = WorkDir::new();
    let build = dir.build_files(
        &[
            (
                "lib/a.ok",
                "module lib/a v1.0.0;\nimport \"b\" take B;\nobject A { A(); }\n",
            ),
            (
                "lib/b.ok",
                "module lib/b v1.0.0;\nimport \"a\" take A;\nobject B { B(); }\n",
            ),
            (
                "cycle.ok",
                "module app v1.0.0;\nimport \"lib/a\" take A;\nmain() is int { 0; }\n",
            ),
            ("lib/c.ok", "module lib/c v1.0.0;\nobject C { C(); }\n"),
            (
                "missing.ok",
                "module app v1.0.0;\nimport \"lib/c\" take Nope;\nmain() is int { 0; }\n",
            ),
        ],
        "cycle.ok",
        "Cycle",
    );
    assert_eq!(build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("b.ok:2:1: semantic error: Import cycle detected: lib/a -> b -> a"),
        "{stderr}"
    );
    let build = dir.build_files(&[], "missing.ok", "Missing");
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains(
            "missing.ok:2:21: semantic error: Import \"lib/c\" has no declaration \"Nope\""
        ),
        "{stderr}"
    );
}

#[test]
fn generic_programs_compute_the_same_results_as_the_interpreter() {
    let dir = WorkDir::new();
    let decls = "module app v1.0.0;
object Box<T> { has value is T; Box(:value); get() is T { value; } }
object Pair<A, B> { has left is A; has right is B; Pair(:left, :right); first() is A { left; } second() is B { right; } }
role Echo<T> { echo(value is T) is T; }
object EchoBox<T> does Echo<T> { has seed is T; EchoBox(:seed); echo(value is T) is T { value; } }
role Ranked<T> { beats(other is T) is bool; }
object Score does Ranked<Score> {
    has n is int; Score(:n);
    beats(other is Score) is bool { n > other.value(); }
    value() is int { n; }
}
object Best<T: Ranked<T>> {
    has first is T; has second is T; Best(:first, :second);
    winner() is T { if first.beats(second) { first; } else { second; } }
}
object Min<T> {
    has first is T; has rest is @T;
    Min(first is T, rest is ...T) { first = first; let m is @T = rest...; rest = m; }
    count() is int { rest.size() + 1; }
}
object Cell<T> {
    has value is T; has version is int = 0; Cell(:value);
    revised(value is T) is ^Cell<T> { ^value = value; ^version = version + 1; }
    current() is T { value; }
    version() is int { version; }
}
";
    // Ожидаемые коды сверены с okkama-pl на тех же программах.
    for (index, (body, expected)) in [
        (
            "let a = Box(value: 7); let c = Box(value: Box(value: \"inner\")); a.get() + c.get().get().length();",
            12,
        ),
        ("let b is Box<@int> = Box(value: []); b.get().size() + 3;", 3),
        ("let p = Pair(left: 2, right: \"xyz\"); p.first() * 10 + p.second().length();", 23),
        ("let e is Echo<int> = EchoBox(seed: 1); e.echo(5);", 5),
        ("let best = Best(first: Score(n: 3), second: Score(n: 9)); best.winner().value();", 9),
        (
            "let boxes is @Box<int> = [Box(value: 1), Box(value: 2)]; \
             boxes.reduce(0) -> acc, b { acc + b.get(); };",
            3,
        ),
        ("let xs = [4, 5]; Min(1, 2, 3).count() * 10 + Min(9, xs...).count();", 33),
        (
            "let c = Cell(value: \"a\").revised(\"b\").revised(\"c\"); c.version() * 10 + c.current().length();",
            21,
        ),
        ("if Box(value: 1) == Box(value: 1) { 1; } else { 2; }", 1),
        ("let p = Pair(left: 1, right: 1); let q is Pair<int, int> = p; q.first();", 1),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("Generic{index}");
        dir.build_source(&name, &format!("{decls} main() is int {{ {body} }}"));
        let output = dir.java(&name, &[]);
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{body}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn float_text_and_arithmetic_follow_the_language_rules() {
    let dir = WorkDir::new();
    // Текст `float` — правило Go `FormatFloat(x, 'g', -1, 64)` (D-032, conformance
    // `L02.float.text-*`); Perl печатает 15 значащих цифр (DEF-009).
    let build = dir.build_files(
        &[(
            "floats.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             main() is int {\n\
                 let out = Stdout();\n\
                 out.writeln((0.1 + 0.2).string(), \" \", (1.0 / 3.0).string(), \" \", 3.0.string());\n\
                 out.writeln(100000.0.string(), \" \", 1000000.0.string(), \" \", 1234567.0.string());\n\
                 out.writeln(0.0001.string(), \" \", 0.00001.string(), \" \", 0.00000025.string());\n\
                 out.writeln((0.0 * -1.0).string(), \" \", (-0.5).string(), \" \", 123.456.string());\n\
                 out.writeln((1.5 + float(1)).string(), \" \", (float(3) / 2.0).string(), \" \", (-7.5 % 2.0).string(), \" \", (7.5 % -2.0).string());\n\
                 out.writeln(int(-3.9).string(), \" \", float(3).string(), \" \", byte(2.7).int().string(), \" \", 2.0.sqrt().string());\n\
                 let m = [1.5: \"a\", 0.5: \"b\", -0.0: \"z\"];\n\
                 out.writeln(m.values().join(\",\"));\n\
                 if 0.1 + 0.2 > 0.3 && float(9007199254740993) == float(9007199254740992) { 0; } else { 1; }\n\
             }\n",
        )],
        "floats.ok",
        "Floats",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Floats", &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "0.30000000000000004 0.3333333333333333 3\n\
         100000 1e+06 1.234567e+06\n\
         0.0001 1e-05 2.5e-07\n\
         -0 -0.5 123.456\n\
         2.5 1.5 -1.5 1.5\n\
         -3 3 2 1.4142135623730951\n\
         z,b,a\n"
    );
    // Тексты ошибок — как у Perl.
    for (index, (body, expected)) in [
        ("let x = 1.0 / 0.0; 0;", "Division by zero"),
        ("let x = 7.5 % 0.0; 0;", "Modulo by zero"),
        (
            "let x = 10.0.pow(308.0) * 10.0; 0;",
            "Float value must contain a finite number",
        ),
        (
            "let x = int(10.0.pow(30.0)); 0;",
            "Cannot convert float value 1e+30 to int: expected signed-64 range",
        ),
        (
            "let x = byte(300.5); 0;",
            "Cannot convert float value 300.5 to byte: expected 0..255",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, _, err) = run_main(&dir, &format!("FloatFailure{index}"), body);
        assert_eq!(code, Some(1), "{body}: {err}");
        assert_eq!(
            err.trim_end(),
            format!("RuntimeError: {expected}"),
            "{body}"
        );
    }
}

#[test]
fn iterating_objects_are_traversed_through_their_cursor() {
    let dir = WorkDir::new();
    // `for` и функциональные операции обходят `Iterating<T>` по протоколу
    // курсора: `cursor()`, затем `available()`, `item()`, `successor()` (§9.9).
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"collections/iteration\" take Cursor, Iterating;\n\
             object Count does Cursor<int> {\n\
                 has at, end is int;\n\
                 Count(:at, :end);\n\
                 available() is bool { at < end; }\n\
                 item() is int { at; }\n\
                 successor() is Cursor<int> { Count(at: at + 1, end: end); }\n\
             }\n\
             object Upto does Iterating<int> {\n\
                 has end is int;\n\
                 Upto(:end);\n\
                 cursor() is Cursor<int> { Count(at: 0, end: end); }\n\
             }\n\
             main() is int {\n\
                 let total = Upto(end: 5).reduce(0) -> sum, n { sum + n; };\n\
                 let evens = Upto(end: 7).filter() -> n { n % 2 == 0; };\n\
                 let squares = Upto(end: 4).map() -> n { n * n; };\n\
                 for Upto(end: 0) -> n { return 99; }\n\
                 total + evens.size() * 10 + squares[3];\n\
             }\n",
        )],
        "main.ok",
        "Iter",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Iter", &[]);
    // 0+1+2+3+4 = 10; evens 0,2,4,6 → 4; squares[3] = 9; пустой обход тело не исполняет.
    assert_eq!(
        output.status.code(),
        Some(10 + 4 * 10 + 9),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rat_values_are_exact_fractions_on_the_jvm() {
    let dir = WorkDir::new();
    // `int / int` — точная дробь (D-046): смешивать с int и float можно
    // только через явный `.rat()`.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             main() is int {\n\
                 let out = Stdout();\n\
                 out.writeln((rat(1, 2) + 1.rat()).string());\n\
                 out.writeln((1 / 3 + 2 / 3).string());\n\
                 out.writeln(rat(6, -4).string(), \" \", (-(3 / 4)).abs().string());\n\
                 out.writeln(0.125.rat().string(), \" \", (7 / 2).float().string());\n\
                 out.writeln((rat(-7, 2) % 2.rat()).string(), \" \", (1 / 2 * (4 / 3)).string());\n\
                 out.writeln((2 / 3).numerator(), \"/\", (2 / 3).denominator());\n\
                 out.writeln((1 / 3 < 1 / 2).string(), \" \", (2 / 4 == 1 / 2).string());\n\
                 rat(7, 2).int() + (-(7 / 2)).sign();\n\
             }\n",
        )],
        "main.ok",
        "Rats",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Rats", &[]);
    // 7/2 усекается к нулю: 3; знак −7/2 — −1.
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "3/2\n1\n-3/2 3/4\n1/8 3.5\n-3/2 2/3\n2/3\n1 1\n"
    );
}

#[test]
fn rat_failures_use_the_interpreter_texts() {
    let dir = WorkDir::new();
    for (index, (body, expected)) in [
        ("let r = 1 / 0; 0;", "RuntimeError: Division by zero"),
        (
            "let r = rat(1, 2) / 0.rat(); 0;",
            "RuntimeError: Division by zero",
        ),
        (
            "let r = rat(1, 2) % 0.rat(); 0;",
            "RuntimeError: Modulo by zero",
        ),
        (
            "let b = rat(600, 2).byte(); 0;",
            "RuntimeError: Cannot convert rat value 300 to byte: expected 0..255",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (code, out, err) = run_main(&dir, &format!("RatFailure{index}"), body);
        assert_eq!(code, Some(1), "{body}: {err}");
        assert!(out.is_empty(), "{body}");
        assert_eq!(err.trim_end(), expected, "{body}");
    }
}

#[test]
fn builtin_order_returns_compare_order_cases() {
    let dir = WorkDir::new();
    // `order()` естественно упорядоченных встроенных типов — `compare.Order`;
    // строки и char — по code points (`"z"` после `"aa"`, `"é"` после `"z"`).
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             import \"compare\" take Order;\n\
             object Names {\n\
                 Names();\n\
                 name(o is Order) is string {\n\
                     match o { case Order.less { \"<\"; } case Order.equal { \"=\"; } case Order.greater { \">\"; } }\n\
                 }\n\
             }\n\
             main() is int {\n\
                 let n = Names();\n\
                 Stdout().writeln(n.name(1.order(2)), n.name(\"z\".order(\"aa\")), n.name(\"é\".order(\"z\")),\n\
                     n.name('b'.order('b')), n.name((1 / 2).order(1 / 3)), n.name(2.5.order(-1.0)),\n\
                     n.name(3.byte().order(7.byte())), n.name(\"\".order(\"a\")));\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Orders",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Orders", &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "<>>=>><<\n");
}

#[test]
fn native_failures_are_catchable_system_objects() {
    let dir = WorkDir::new();
    // Ожидаемый сбой provider — `thrown` с системным `NativeFailure`
    // (domain/code/operation/message, `text()` — message); его ловят и как
    // `NativeFailure`, и как `Error`. Сбой вызова — программная ошибка.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"runtime/bigint\" take bigint_quotient, bigint_order;\n\
             import \"runtime/base64\" take base64_encode;\n\
             import \"io/stdout\" take Stdout;\n\
             main() is int {\n\
                 let out = Stdout();\n\
                 try { bigint_quotient(\"1\", \"0\"); } catch f is NativeFailure {\n\
                     out.writeln(f.domain(), \"|\", f.code(), \"|\", f.operation(), \"|\", f.message(), \"|\", f.text());\n\
                 }\n\
                 try { bigint_quotient(\"1\", \"0\"); } catch e is Error { out.writeln(\"error: \", e.text()); }\n\
                 out.writeln(bigint_quotient(\"-7\", \"2\"), \" \", bigint_order(\"10\", \"9\").string(), \" \", base64_encode([1.byte(), 2.byte(), 3.byte()], \"standard\"));\n\
                 bigint_quotient(\"1\", \"0\");\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Natives",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Natives", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "core.bigint|invalid_input|runtime/bigint@1::bigint_quotient/2|BigInt division by zero|BigInt division by zero\n\
         error: BigInt division by zero\n\
         -3 1 AQID\n"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "NativeError: BigInt division by zero\ncode: invalid_input\ndomain: core.bigint\n\
         operation: runtime/bigint@1::bigint_quotient/2\n"
    );
    let build = dir.build_files(
        &[(
            "invalid.ok",
            "module app v1.0.0;\n\
             import \"runtime/bigint\" take bigint_order;\n\
             main() is int { bigint_order(\"x\", \"0\"); }\n",
        )],
        "invalid.ok",
        "NativeInvalid",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("NativeInvalid", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "RuntimeError: Native invocation \"runtime/bigint@1::bigint_order/2\" failed\n"
    );
}

#[test]
fn arrays_and_scalars_cross_standard_role_boundaries_through_adapters() {
    let dir = WorkDir::new();
    // Массив как `Sequencing<E>`/`Iterating<E>`/`Sizing`, скаляр как
    // `Comparing<T>`: адаптеры lowering, тексты ошибок — Perl.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"collections/sequence\" take Sequencing;\n\
             import \"collections/iteration\" take Cursor, Iterating;\n\
             import \"size\" take Sizing;\n\
             import \"compare\" take Comparing, Order;\n\
             import \"io/stdout\" take Stdout;\n\
             object Probe {\n\
                 Probe();\n\
                 show(s is Sequencing<int>) is string { s.size().string() + s.first().string() + s.last().string() + s.item(1).string(); }\n\
                 walk(s is Iterating<string>) is string { let c = s.cursor(); c.item() + c.successor().item() + c.successor().successor().available().string(); }\n\
                 sized(s is Sizing) is int { s.size(); }\n\
                 less(c is Comparing<int>) is bool { c.order(5) == Order.less; }\n\
                 total(s is Iterating<int>) is int { s.reduce(0) -> sum, n { sum + n; }; }\n\
                 empty(s is Sequencing<int>) is int { s.first(); }\n\
             }\n\
             main() is int {\n\
                 let p = Probe();\n\
                 let c = [7, 8].cursor();\n\
                 Stdout().writeln(p.show([4, 5, 6]), \" \", p.walk([\"a\", \"b\"]), \" \", p.sized([]).string(),\n\
                     \" \", p.less(3).string(), \" \", p.total([1, 2, 3]).string(), \" \", c.successor().item().string());\n\
                 p.empty([]);\n\
             }\n",
        )],
        "main.ok",
        "Adapters",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Adapters", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "3465 ab0 0 1 6 8\n"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "RuntimeError: Cannot read first item of an empty sequence\n"
    );
}

#[test]
fn for_in_value_position_yields_the_last_iteration() {
    let dir = WorkDir::new();
    // Значение `for` — тело последней итерации; `next` его не меняет,
    // `last v` — значение выхода; без итераций — ошибка Perl.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             object P {\n\
                 P();\n\
                 doubled(xs is @int) is int { for xs -> n { n * 2; } }\n\
                 skipping() is string { for 1..4 -> i { if i == 3 { next; } i.string(); } }\n\
                 find(xs is @int, wanted is int) is int { for xs -> n { last n * 10 if n == wanted; 0; } }\n\
             }\n\
             main() is int {\n\
                 let p = P();\n\
                 Stdout().writeln(p.doubled([1, 2, 3]).string(), \" \", p.skipping(), \" \",\n\
                     p.find([1, 2, 3], 2).string(), \" \", p.find([1], 9).string());\n\
                 p.doubled([]);\n\
             }\n",
        )],
        "main.ok",
        "ForValue",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("ForValue", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "6 2 20 0\n");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "RuntimeError: A void result cannot be used as a value\n"
    );
}

#[test]
fn any_values_are_opaque_boxes() {
    let dir = WorkDir::new();
    // `any.box` — непрозрачный box: его передают, хранят и считают, но не
    // распаковывают; глубокое равенство, дошедшее до box, — ошибка Perl.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             object Holder { has value is any; Holder(:value); }\n\
             object Relay { Relay(); count(values is @any) is int { values.size(); } echo(value is any) is any { value; } }\n\
             main() is int {\n\
                 let mixed = [1, \"two\", 3.0];\n\
                 let row is %string:any = [\"id\": 7, \"name\": \"Ada\"];\n\
                 let echoed = Relay().echo(Holder(value: 1));\n\
                 Stdout().writeln(Relay().count(mixed).string(), \" \", row.size().string());\n\
                 if Holder(value: 1) == Holder(value: 1) { 1; } else { 0; }\n\
             }\n",
        )],
        "main.ok",
        "Anys",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Anys", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "3 2\n");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "RuntimeError: Cannot compare any without narrowing\n"
    );
}

#[test]
fn stream_writes_go_through_the_writing_protocol() {
    let dir = WorkDir::new();
    // `sink <- source`: `write(ByteSource)`; строка — строгий UTF-8, `@byte` —
    // как есть, `xs...` — по элементу; значение — исходный sink.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             main() is int {\n\
                 let out = Stdout();\n\
                 out <- \"héllo \" <- [65.byte(), 10.byte()];\n\
                 out <- @string(1, 2, 3)... <- \"\\n\";\n\
                 let empty is @string = [];\n\
                 out <- empty...;\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Streams",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Streams", &[]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "héllo A\n123\n");
}

#[test]
fn range_values_compare_and_iterate() {
    let dir = WorkDir::new();
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             main() is int {\n\
                 let r = 2..=4;\n\
                 let total = r.reduce(0) -> sum, i { sum + i; };\n\
                 let same = (1..3) == (1..3);\n\
                 let other = (1..3) == (1..=3);\n\
                 Stdout().writeln(total.string(), \" \", same.string(), other.string());\n\
                 let five = 5;\n\
                 let wrong = five..1;\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Ranges",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Ranges", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "9 10\n");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "RuntimeError: Range start must be less than or equal to range end\n"
    );
}

#[test]
fn anonymous_and_bound_actions_are_fn_values() {
    let dir = WorkDir::new();
    // `fn.closure`: снимок захватов (и `self` окружающего action), вызов —
    // `call.fn`; bound action вызываем после границы типа action (§8.10.1).
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             object Counter {\n\
                 has step is int;\n\
                 Counter(:step);\n\
                 adder() is (int) is int { (n is int) is int { n + step; }; }\n\
                 twice(f is (int) is int, x is int) is int { f(f(x)); }\n\
                 bump(n is int) is int { n + step * 10; }\n\
             }\n\
             main() is int {\n\
                 let out = Stdout();\n\
                 let factor = 3;\n\
                 let times = (n is int) is int { n * factor; };\n\
                 let c = Counter(step: 2);\n\
                 let add = c.adder();\n\
                 let bound is (int) is int = c.bump;\n\
                 let flag = (b is bool) is bool { !b; };\n\
                 let say = (s is string) is void { out.writeln(\"say \" + s); };\n\
                 say(\"hi\");\n\
                 out.writeln(times(5).string(), \" \", add(1).string(), \" \", c.twice(times, 2).string(),\n\
                     \" \", bound(1).string(), \" \", flag(false).string());\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Closures",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Closures", &[]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "say hi\n15 3 18 21 1\n"
    );
}

#[test]
fn async_futures_are_lazy_cached_and_spawned_eagerly() {
    let dir = WorkDir::new();
    // Вызов async — холодный Future: тело при первом `await`, исход
    // кэшируется; `spawn` исполняет тело сразу до приостановки; отказ
    // повторяется при каждом `await`; отмена холодного — `Cancelled`.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             object Broken does Error { Broken(); text() is string { \"broken\"; } }\n\
             object Job {\n\
                 has name is string;\n\
                 Job(:name);\n\
                 async run() is int { Stdout().writeln(\"running \" + name); 7; }\n\
                 async fail() is int { Stdout().writeln(\"failing \" + name); throw Broken(); }\n\
             }\n\
             async main() is int {\n\
                 let out = Stdout();\n\
                 let pending = Job(name: \"a\").run();\n\
                 out.writeln(\"created\");\n\
                 let total = (await pending) + (await pending);\n\
                 let spawned = spawn Job(name: \"b\").run();\n\
                 out.writeln(\"spawned \" + total.string());\n\
                 let broken = Job(name: \"c\").fail();\n\
                 let first = try { await broken; } catch e is Broken { -1; };\n\
                 let second = try { await broken; } catch e is Broken { -2; };\n\
                 let never = Job(name: \"d\").run();\n\
                 never.cancel();\n\
                 let cancelled = try { await never; } catch e is Cancelled { -3; };\n\
                 out.writeln(first.string(), \" \", second.string(), \" \", cancelled.string(),\n\
                     \" \", (await spawned).string(), \" \", never.cancelled().string());\n\
                 spawn Job(name: \"e\").fail();\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Futures",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Futures", &[]);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "created\nrunning a\nrunning b\nspawned 14\nfailing c\n-1 -2 -3 7 1\nfailing e\n"
    );
    // Ненаблюдённый отказ задачи `spawn` — отказ программы (D-034).
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Broken: broken\n"
    );
}

#[test]
fn async_intrinsics_run_in_the_jvm_runtime() {
    let dir = WorkDir::new();
    // Каналы (буфер и рандеву), Event, Atomic и AllFutures `async/*`:
    // состояние держит рантайм, порядок — как у Perl-интерпретатора.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             import \"async/channel\" take Channel, Sending, ChannelClosed;\n\
             import \"async/event\" take Event;\n\
             import \"async/atomic\" take Atomic;\n\
             import \"async/collect\" take AllFutures;\n\
             object Producer {\n\
                 has output is Sending<int>;\n\
                 has started is Event;\n\
                 Producer(:output, :started);\n\
                 async run() is int {\n\
                     started.set();\n\
                     await output.send(1);\n\
                     await output.send(2);\n\
                     output.close();\n\
                     0;\n\
                 }\n\
             }\n\
             object Job { has n is int; Job(:n); async run() is int { n * 10; } }\n\
             async main() is int {\n\
                 let out = Stdout();\n\
                 let channel is Channel<int> = Channel(capacity: 0);\n\
                 let started = Event();\n\
                 let task = spawn Producer(output: channel.sending(), started: started).run();\n\
                 await started.wait();\n\
                 let a = await channel.receiving().item();\n\
                 let b = await channel.receiving().item();\n\
                 let closed = try { await channel.receiving().item(); } catch e is ChannelClosed { -1; };\n\
                 let counter = Atomic(value: 5);\n\
                 let swapped = counter.compare_and_replace(5, 6);\n\
                 let missed = counter.compare_and_replace(5, 7);\n\
                 let pending is @Future<int> = [Job(n: 1).run(), Job(n: 2).run()];\n\
                 let all is AllFutures<int> = AllFutures(origins: pending);\n\
                 let values = await all.results();\n\
                 out.writeln(a.string(), \" \", b.string(), \" \", closed.string(), \" \", (await task).string(),\n\
                     \" \", swapped.string(), missed.string(), \" \", counter.value().string(),\n\
                     \" \", values.item(0).string(), \" \", values.item(1).string());\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Intrinsics",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Intrinsics", &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "1 2 -1 0 10 6 10 20\n"
    );
}

#[test]
fn caps_recompute_fields_over_their_owner() {
    let dir = WorkDir::new();
    // `cap.make` — capability владельца; `call.cap` вычисляет поле заново;
    // после successor cap остаётся при исходном снимке владельца.
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             object Customer {\n\
                 has given, family is string;\n\
                 has visits is int = 0;\n\
                 Customer(:given, :family);\n\
                 visited() is ^Customer { ^visits = visits + 1; }\n\
                 cap Card {\n\
                     full is string { given + \" \" + family; }\n\
                     count is int { visits; }\n\
                 }\n\
             }\n\
             cap Customer::Badge { initials is string { Customer::given.slice(0, 1) + Customer::family.slice(0, 1); } }\n\
             main() is int {\n\
                 let ada = Customer(given: \"Ada\", family: \"Lovelace\");\n\
                 let card = ada::Card;\n\
                 let later = ada.visited();\n\
                 Stdout().writeln(card::full, \" \", card::count.string(), \" \", later::Card::count.string(), \" \", ada::Badge::initials);\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Caps",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Caps", &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Ada Lovelace 0 1 AL\n"
    );
}

#[test]
fn test_mode_runs_tests_with_fakes_and_reports_failures() {
    let dir = WorkDir::new();
    // `build --tests`: исполнитель тестов вместо `main`; провал утверждения
    // не прерывает тест, отказ — только его; fake роли хранит состояние
    // экземпляра; код выхода 1, если провалился хоть один.
    std::fs::write(
        dir.0.join("main.ok"),
        "module app v1.0.0;\n\
         role Books {\n\
             remember(id is int, title is string) is void;\n\
             title(id is int) is string;\n\
             count() is int;\n\
             fake {\n\
                 has prefix is string = \"fake:\";\n\
                 remember(id is int, title is string) is void { fake.storage.set(id, title); }\n\
                 title(id is int) is string { prefix + fake.storage.get(id); }\n\
                 count() is int { fake.storage.size(); }\n\
             }\n\
         }\n\
         object Worker { Worker(); async value() is int { 7; } test \"scoped\" { test.ok(true); } }\n\
         object Denied does Error { Denied(); text() is string { \"denied\"; } }\n\
         test \"fakes keep state\" {\n\
             let first is Books = Books.fake();\n\
             let second is Books = Books.fake();\n\
             first.remember(1, \"Dune\");\n\
             test.is(first.title(1), \"fake:Dune\");\n\
             test.is(second.count(), 0);\n\
         }\n\
         async test \"async\" { test.is(await Worker().value(), 7); }\n\
         test \"wrong\" { test.is(1 + 1, 3); test.ok(true); }\n\
         test \"throws\" { throw Denied(); }\n\
         main() is int { panic \"main does not run in test mode\"; }\n",
    )
    .unwrap();
    let build = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["build", "--tests"])
        .arg(dir.0.join("main.ok"))
        .args(["--backend", "jvm", "-o"])
        .arg(dir.0.join("Suite.class"))
        .env("OKKAMA_STDLIB_DIR", stdlib())
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Suite", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "=== RUN   Worker :: scoped\n--- PASS: Worker :: scoped\n\
         === RUN   fakes keep state\n--- PASS: fakes keep state\n\
         === RUN   async\n--- PASS: async\n\
         === RUN   wrong\n--- FAIL: wrong\n    expected 3, got 2\n\
         === RUN   throws\n--- FAIL: throws\n    Denied: denied\nFAIL\n"
    );
}

#[test]
fn synchronous_spawn_runs_in_a_worker_process() {
    let dir = WorkDir::new();
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             object Worker {\n\
                 has seed is int;\n\
                 Worker(:seed);\n\
                 calculate(input is int) is int { Stdout().writeln(\"worker \" + seed.string()); seed * input; }\n\
                 tagged(word is string) is %string:@byte { [word: [1.byte(), 2.byte()]]; }\n\
                 boom(input is int) is int { confirm input < 0 else throw Bad(); input; }\n\
             }\n\
             object Bad does Error { Bad(); text() is string { \"bad input\"; } }\n\
             async main() is int {\n\
                 let left = spawn Worker(seed: 7).calculate(input: 20);\n\
                 let bound is (int) is int = Worker(seed: 9).calculate;\n\
                 let right = spawn bound(6);\n\
                 let map = await spawn Worker(seed: 1).tagged(word: \"ж\");\n\
                 Stdout().writeln(((await left) + (await right)).string() + \" \" + map[\"ж\"].size().string());\n\
                 await spawn Worker(seed: 0).boom(input: 3);\n\
             }\n",
        )],
        "main.ok",
        "Spawning",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Spawning", &[]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("worker 7\n") && stdout.contains("worker 9\n"),
        "{stdout}"
    );
    assert!(stdout.ends_with("194 2\n"), "{stdout}");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("runtime: Bad: bad input"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn huge_literals_are_outlined_under_the_method_limit() {
    let dir = WorkDir::new();
    let bytes: Vec<String> = (0..2400)
        .map(|index| format!("{}.byte()", index % 256))
        .collect();
    let literal = format!("[{}]", bytes.join(","));
    let (status, _, stderr) = run_main(
        &dir,
        "Literal",
        &format!(
            "let a = {literal}; let b = {literal}; let c = {literal}; \
             (a.size() + b.size() + c.size() + a[2399].int()) % 256;"
        ),
    );
    assert_eq!(status, Some((7200 + 95) % 256), "{stderr}");
}

#[test]
fn bound_role_actions_and_future_selectors_run_on_the_jvm() {
    let dir = WorkDir::new();
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"async/collect\" take FirstSuccessful, FutureCompletions;\n\
             role Valued { value() is int; }\n\
             object Fixed does Valued { has n is int; Fixed(:n); value() is int { n; } }\n\
             object Task { has n is int; Task(:n); async run() is int { confirm n > 1 else throw Low(); n; } }\n\
             object Low does Error { Low(); text() is string { \"low\"; } }\n\
             async main() is int {\n\
                 let r is Valued = Fixed(n: 4);\n\
                 let read is () is int = r.value;\n\
                 let selector is FirstSuccessful<int> = FirstSuccessful(origins: [Task(n: 1).run(), Task(n: 5).run()]);\n\
                 let first = await selector.result();\n\
                 let stream is FutureCompletions<int> = FutureCompletions(origins: [Task(n: 2).run(), Task(n: 3).run()]);\n\
                 let total = (await await stream.completion()) + (await await stream.completion());\n\
                 read() + first + total + stream.remaining_count();\n\
             }\n",
        )],
        "main.ok",
        "Selectors",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Selectors", &[]);
    assert_eq!(
        output.status.code(),
        Some(14),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_jar_output_is_one_executable_file() {
    let dir = WorkDir::new();
    std::fs::write(
        dir.0.join("main.ok"),
        "module app v1.0.0;\nimport \"io/stdout\" take Stdout;\n\
         object Box { has n is int; Box(:n); twice() is int { n * 2; } }\n\
         main() is int { Stdout().writeln(\"jar\"); Box(n: 21).twice(); }\n",
    )
    .unwrap();
    let jar = dir.0.join("app.jar");
    let build = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .arg("build")
        .arg(dir.0.join("main.ok"))
        .args(["--backend", "jvm", "-o"])
        .arg(&jar)
        .env("OKKAMA_STDLIB_DIR", stdlib())
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let names: Vec<String> = std::fs::read_dir(&dir.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let mut names = names;
    names.sort();
    assert_eq!(names, ["app.jar", "main.ok"]);
    let output = Command::new("java")
        .arg("-jar")
        .arg(&jar)
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(42),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "jar\n");
}

#[test]
fn child_exit_codes_and_signals_are_exact() {
    let dir = WorkDir::new();
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"os/command\" take InheritedCommand;\n\
             import \"os/command_capture\" take CommandCapture;\n\
             import \"os/process\" take ExitKind;\n\
             import \"fs/path\" take Path;\n\
             import \"io/stdout\" take Stdout;\n\
             object Run {\n\
                 Run();\n\
                 async code(script is string) is string {\n\
                     let exit = (await CommandCapture(InheritedCommand(program: Path(\"/bin/sh\"), arguments: [\"-c\", script]), 64).completion()).exit();\n\
                     match exit.kind() { case ExitKind.exited { exit.code().string(); } case ExitKind.interrupted { \"INT\"; } case ExitKind.terminated { \"TERM\"; } case ExitKind.killed { \"KILL\"; } case ExitKind.other { \"other\"; } }\n\
                 }\n\
             }\n\
             async main() is int {\n\
                 let run = Run();\n\
                 Stdout().writeln((await run.code(\"exit 130\")) + \" \" + (await run.code(\"kill -INT $$\")));\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Exits",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = dir.java("Exits", &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "130 INT\n");
}

#[test]
fn worker_receivers_are_destroyed_and_cancelled_workers_stop() {
    let dir = WorkDir::new();
    let build = dir.build_files(
        &[(
            "main.ok",
            "module app v1.0.0;\n\
             import \"io/stdout\" take Stdout;\n\
             import \"time/clock\" take SystemClock;\n\
             import \"time/duration\" take Duration, DurationUnit;\n\
             object W {\n\
                 has n is int;\n\
                 W(:n);\n\
                 f(x is int) is int { n + x; }\n\
                 slow(count is int) is int { (0..count).reduce(0) -> total, i { total + i; }; }\n\
                 destroy { Stdout().writeln(\"destroy \" + n.string()); }\n\
             }\n\
             async main() is int {\n\
                 Stdout().writeln((await spawn W(n: 1).f(x: 2)).string());\n\
                 let job = spawn W(n: 7).slow(count: 1000000000);\n\
                 await SystemClock().wait(Duration(1.rat(), DurationUnit.second));\n\
                 job.cancel();\n\
                 Stdout().writeln(job.cancelled().string());\n\
                 0;\n\
             }\n",
        )],
        "main.ok",
        "Workers",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let started = std::time::Instant::now();
    let output = dir.java("Workers", &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "destroy 1\n3\n1\n"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}

#[test]
fn deep_recursion_runs_on_the_program_stack_and_tail_calls_loop() {
    let dir = WorkDir::new();
    // Нехвостовая рекурсия на 1 млн кадров: поток программы с большим стеком;
    // хвостовая на 10 млн — цикл (OIR), стек не растёт.
    let source = "module app v1.0.0; object R { R() {} \
        sum(n is int) is int { if n == 0 { 0; } else { n + self.sum(n - 1); } } \
        count(n is int, acc is int) is int { if n == 0 { acc; } else { self.count(n - 1, acc + n); } } } \
        main() is int { (R().sum(1000000) + R().count(10000000, 0)) % 256; }";
    dir.build_source("Deep", source);
    let output = dir.java("Deep", &[]);
    let expected = (1_000_000u64 * 1_000_001 / 2 + 10_000_000u64 * 10_000_001 / 2) % 256;
    assert_eq!(output.status.code(), Some(expected as i32), "{}", String::from_utf8_lossy(&output.stderr));
}
