//! Рекурсия в OIR: бесконечная рекурсия — ошибка, хвостовой самовызов — цикл.
use okkama_compiler::{
    SourceFile, ir,
    lowering::{LoweringError, lower_program},
    syntax::{self, ParseOptions},
};

fn lower(objects: &str, main: &str) -> Result<ir::Module, LoweringError> {
    let text = format!("module app v1.0.0; {objects} main() is int {{ {main} }}");
    let source = SourceFile::new("test.ok", text).expect("small source");
    lower_program(&syntax::parse(&source, &ParseOptions::default()).expect("valid syntax"))
}

fn self_calls(module: &ir::Module, symbol: &str) -> usize {
    let (index, function) = module
        .functions
        .iter()
        .enumerate()
        .find(|(_, function)| function.symbol == symbol)
        .expect("function");
    function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| matches!(&inst.kind, ir::InstKind::Call { func, .. } if func.0 == index))
        .count()
}

#[test]
fn recursion_without_a_finishing_path_is_rejected() {
    for (objects, symbol, through) in [
        ("object L { L() {} spin(n is int) is int { n + self.spin(n + 1); } }", "L.spin", None),
        // Хвостовая форма тоже: вызов не может завершиться ничем.
        ("object L { L() {} spin(n is int) is int { self.spin(n + 1); } }", "L.spin", None),
        (
            "object P { P() {} ping(n is int) is int { self.pong(n) * 2; } pong(n is int) is int { self.ping(n) + 1; } }",
            "P.ping",
            Some("P.pong"),
        ),
    ] {
        let error = lower(objects, "0;").expect_err(objects).to_string();
        assert!(error.contains(&format!("Infinite recursion: \"{symbol}\"")), "{error}");
        if let Some(through) = through {
            assert!(error.contains(&format!("through \"{through}\"")), "{error}");
        }
    }
}

#[test]
fn recursion_with_a_base_case_or_a_possible_failure_is_accepted() {
    for objects in [
        "object M { M() {} fact(n is int) is int { if n <= 1 { 1; } else { n * self.fact(n - 1); } } }",
        // Деление может отказать: путь завершения есть.
        "object M { M() {} fact(n is int) is int { (10 % n) + self.fact(n - 1); } }",
        "object M { M() {} even(n is int) is bool { if n == 0 { true; } else { self.odd(n - 1); } } odd(n is int) is bool { if n == 0 { false; } else { self.even(n - 1); } } }",
    ] {
        lower(objects, "0;").unwrap_or_else(|error| panic!("{objects}: {error}"));
    }
}

#[test]
fn self_tail_calls_become_loops() {
    let module = lower(
        "object C { C() {} count(n is int, acc is int) is int { if n == 0 { acc; } else { self.count(n - 1, acc + n); } } fib(n is int) is int { if n < 2 { n; } else { self.fib(n - 1) + self.fib(n - 2); } } }",
        "C().count(10, 0) + C().fib(5);",
    )
    .expect("lowers");
    ir::verify(&module).expect("verifies");
    assert_eq!(self_calls(&module, "C.count"), 0, "tail call is a loop");
    assert_eq!(self_calls(&module, "C.fib"), 2, "non-tail calls stay calls");
}

#[test]
fn tail_calls_in_both_branches_leave_no_unreachable_join() {
    // Обе ветви — хвостовые вызовы: блок, где сходились их результаты, удаляется.
    let module = lower(
        "object F { has lo is int; has hi is int; F(:lo, :hi); #projected value() is int { if lo >= hi { lo; } else { let middle = lo + ((hi - lo) / 2).int(); if middle < 5 { value F(lo: middle + 1, hi: hi); } else { value F(lo: lo, hi: middle); } } } }",
        "F(lo: 0, hi: 100).value();",
    )
    .expect("lowers");
    ir::verify(&module).expect("verifies");
    assert_eq!(self_calls(&module, "F.value"), 0, "both tail calls are loops");
}
