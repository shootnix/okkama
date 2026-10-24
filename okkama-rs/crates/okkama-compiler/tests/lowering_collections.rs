//! Lowering массивов, словарей и строк: сборщики, циклы обхода,
//! `first()`/`last()`, `m[k] else` и диагностика в формате Perl-интерпретатора.

use okkama_compiler::SourceFile;
use okkama_compiler::ir;
use okkama_compiler::lowering::{LoweringError, lower_program};
use okkama_compiler::syntax::{self, ParseOptions, prune};

fn lower(body: &str) -> Result<ir::Module, LoweringError> {
    let text = format!("module app v1.0.0; main() is int {{ {body} }}");
    let source = SourceFile::new("test.ok", &text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    lower_program(&prune::strip_test_items(program))
}

fn main_listing(body: &str) -> String {
    let module = lower(body).unwrap_or_else(|error| panic!("{body}: {error}"));
    ir::verify(&module).expect("lowering output verifies");
    let text = module.to_string();
    let start = text.find("func @main").expect("main is printed");
    // Только `main`: lowering по достижимости печатает её первой.
    let end = text[start..]
        .find("\n}\n")
        .map_or(text.len(), |end| start + end + 3);
    text[start..end].to_string()
}

#[test]
fn map_threads_a_linear_builder_through_the_loop_header() {
    assert_eq!(
        main_listing("let xs = [1, 2].map() -> v { v * 2; }; xs.size();"),
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: int = const 1\n",
            "    %1: int = const 2\n",
            "    %2: array<int> = array.new [%0, %1]\n",
            "    %3: builder<int> = builder.new\n",
            "    %4: int = const 0\n",
            "    %5: int = array.len %2\n",
            "    br ^b1(%4, %3)\n",
            "^b1(%6: int, %7: builder<int>):\n",
            "    %8: bool = int.lt %6, %5\n",
            "    cond %8, ^b2, ^b4\n",
            "^b2:\n",
            "    %9: int = array.get %2, %6\n",
            "    %10: int = const 2\n",
            "    %11: int = int.mul %9, %10\n",
            "    %12: builder<int> = builder.push %7, %11\n",
            "    br ^b3(%12)\n",
            "^b3(%13: builder<int>):\n",
            "    %14: int = const 1\n",
            "    %15: int = int.add %6, %14\n",
            "    br ^b1(%15, %13)\n",
            "^b4:\n",
            "    %16: array<int> = builder.finish %7\n",
            "    %17: int = array.len %16\n",
            "    ret %17\n",
            "}\n",
        )
    );
}

#[test]
fn first_and_last_trap_with_the_empty_sequence_text() {
    assert_eq!(
        main_listing("[4, 5].first();"),
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: int = const 4\n",
            "    %1: int = const 5\n",
            "    %2: array<int> = array.new [%0, %1]\n",
            "    %3: int = const 0\n",
            "    array.lookup %2, %3, ^b1, ^b2\n",
            "^b1(%4: int):\n",
            "    ret %4\n",
            "^b2:\n",
            "    trap empty \"Cannot read first item of an empty sequence\"\n",
            "}\n",
        )
    );
    let last = main_listing("[4, 5].last();");
    assert!(last.contains("int.sub"), "{last}");
    assert!(
        last.contains("trap empty \"Cannot read last item of an empty sequence\""),
        "{last}"
    );
}

#[test]
fn collection_rule_violations_are_semantic_errors() {
    for (body, expected) in [
        (
            "\"ab\".more(\"a\"); 0;",
            "Type \"string\" has no builtin action \"more\"",
        ),
        (
            "\"ab\".more_or_equal(\"a\"); 0;",
            "Type \"string\" has no builtin action \"more_or_equal\"",
        ),
        (
            "let xs = [3, 1]; let ys = @int(0, xs..., \"9\"); 0;",
            "@int() argument 3: Cannot convert string to int",
        ),
        ("int(\"9\");", "Cannot convert string to int"),
        (
            "let m = [\"a\": 1]; for m -> k { 0; } 0;",
            "For source must be an array or range",
        ),
        (
            "let m = [\"a\": 1]; m.map() -> v { v; }; 0;",
            "Map map requires exactly 2 bindings, got 1",
        ),
        (
            "let m = [\"a\": 1]; m.reduce(0) -> acc, v { acc; };",
            "Map reduce requires exactly 3 bindings, got 2",
        ),
        (
            "[1].map() -> a, b { a; }; 0;",
            "Array map requires exactly 1 binding, got 2",
        ),
        (
            "let e = [:]; e.filter() -> k, v { true; }; 0;",
            "Functional map operations require a typed %K:V receiver",
        ),
        (
            "let e = []; e.map() -> v { v; }; 0;",
            "Cannot infer map result type from empty source",
        ),
        (
            "let m = [\"a\": 1]; m[1];",
            "Map key must be string, got int",
        ),
        (
            "let x is @int = 1; 0;",
            "Cannot initialize variable \"x\" of type @int with int",
        ),
    ] {
        let error = lower(body).unwrap_err();
        assert!(
            matches!(error, LoweringError::Invalid { .. }),
            "{body}: {error:?}"
        );
        assert_eq!(error.message(), expected, "{body}");
    }
}

#[test]
fn map_literals_and_lookups_have_their_own_instructions() {
    assert_eq!(
        main_listing("let m = [\"b\": 1, \"a\": 2]; m[\"c\"] else m.size();"),
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: str = const \"b\"\n",
            "    %1: int = const 1\n",
            "    %2: str = const \"a\"\n",
            "    %3: int = const 2\n",
            "    %4: map<str,int> = map.new [%0: %1, %2: %3]\n",
            "    %5: str = const \"c\"\n",
            "    map.lookup %4, %5, ^b1, ^b2\n",
            "^b1(%6: int):\n",
            "    br ^b3(%6)\n",
            "^b2:\n",
            "    %7: int = map.len %4\n",
            "    br ^b3(%7)\n",
            "^b3(%8: int):\n",
            "    ret %8\n",
            "}\n",
        )
    );
    // `filter` словаря собирает новый словарь линейным `mbuilder`.
    let filtered =
        main_listing("let m = [1: true, 2: false]; let f = m.filter() -> k, v { v; }; f.size();");
    for expected in [
        "map.keys %",
        "map.values %",
        "mbuilder<int,bool> = mbuilder.new",
        "mbuilder.put %",
        "map<int,bool> = mbuilder.finish %",
    ] {
        assert!(filtered.contains(expected), "{expected} in\n{filtered}");
    }
    // `[]`/`[:]` в ветви получают тип другой ветви.
    let branches = main_listing(
        "let f = 1 > 0; let m = if f { [1: true]; } else { [:]; }; \
         let xs = if f { []; } else { [2]; }; m.size() + xs.size();",
    );
    assert!(
        branches.contains("map<int,bool> = map.retag %"),
        "{branches}"
    );
    assert!(
        branches.contains("array<int> = array.retag %"),
        "{branches}"
    );
}
