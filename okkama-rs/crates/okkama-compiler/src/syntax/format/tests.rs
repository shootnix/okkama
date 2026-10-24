use super::*;

fn fmt(text: &str) -> String {
    let source = SourceFile::new("t.ok", text).unwrap();
    let once = format(&source).unwrap_or_else(|error| panic!("{error:?}"));
    let again = format(&SourceFile::new("t.ok", once.clone()).unwrap()).unwrap();
    assert_eq!(once, again, "formatting is not idempotent");
    once
}

#[test]
fn compressed_code_is_expanded_one_member_and_statement_per_line() {
    let text = "module app v1.0.0;\nimport \"io/stdout\" take Stdout;\nobject A{has x,y is int;has z is @int;A(x is int){self.x=x;y=x*2+1;z=[];}size() is int{z.size();}}\nmain() is int{let a=A(1);return a.size();}\n";
    assert_eq!(
        fmt(text),
        "module app v1.0.0;

import \"io/stdout\" take Stdout;

object A {
    has x, y is int;
    has z is @int;

    A(x is int) {
        self.x = x;
        y = x * 2 + 1;
        z = [];
    }

    size() is int {
        z.size();
    }
}

main() is int {
    let a = A(1);
    return a.size();
}
"
    );
}

#[test]
fn else_and_catch_start_their_own_line() {
    let text = "module app v1.0.0;\nmain() is int{if a{x();}else if b{y();}else{z();}let v=try{f();}catch e is Error{0;};return v;}\n";
    assert_eq!(
        fmt(text),
        "module app v1.0.0;

main() is int {
    if a {
        x();
    }
    else if b {
        y();
    }
    else {
        z();
    }
    let v = try {
        f();
    }
    catch e is Error {
        0;
    };
    return v;
}
"
    );
}

#[test]
fn comments_stay_with_their_code() {
    let text = "// header\n\nmodule app v1.0.0;\n// about A\nobject A {\n    // the value\n    has x is int; // trailing\n    /* block */\n}\nmain() is int {\n    return 0; // done\n    // last\n}\n";
    assert_eq!(
        fmt(text),
        "// header

module app v1.0.0;

// about A
object A {
    // the value
    has x is int; // trailing
    /* block */
}

main() is int {
    return 0; // done
    // last
}
"
    );
}

#[test]
fn long_lists_break_one_item_per_line_and_short_ones_stay_inline() {
    let text = "module app v1.0.0;\nmain() is int { let x = compute(first_argument_value, second_argument_value, third_argument_value, fourth_value_here); let y = f(a, b); return 0; }\n";
    assert_eq!(
        fmt(text),
        "module app v1.0.0;

main() is int {
    let x = compute(
        first_argument_value,
        second_argument_value,
        third_argument_value,
        fourth_value_here
    );
    let y = f(a, b);
    return 0;
}
"
    );
}

#[test]
fn author_line_breaks_and_trailing_commas_keep_lists_open() {
    let text = "module app v1.0.0;\nmain() is int {\n    let x = f(a,\n        b);\n    let y = g(c, d,);\n    return 0;\n}\n";
    assert_eq!(
        fmt(text),
        "module app v1.0.0;

main() is int {
    let x = f(
        a,
        b
    );
    let y = g(
        c,
        d,
    );
    return 0;
}
"
    );
}

#[test]
fn closures_hug_the_call_and_logical_chains_break_before_operators() {
    let text = "module app v1.0.0;\nmain() is int { let all = xs.map() -> x { x + 1; }; confirm first_condition_value && second_condition_value && third_condition_value_that_is_long && fourth else throw Failure(); return 0; }\n";
    assert_eq!(
        fmt(text),
        "module app v1.0.0;

main() is int {
    let all = xs.map() -> x {
        x + 1;
    };
    confirm first_condition_value
        && second_condition_value
        && third_condition_value_that_is_long
        && fourth
        else throw Failure();
    return 0;
}
"
    );
}

#[test]
fn glued_generic_close_and_assignment_split_safely() {
    let text =
        "module app v1.0.0;\nmain() is int { let a is Atomic<int>=Atomic(0); return a.value(); }\n";
    assert!(fmt(text).contains("let a is Atomic<int> = Atomic(0);"));
}

#[test]
fn match_cases_and_private_sections_are_laid_out() {
    let text = "module app v1.0.0;\nobject A { name() is string { match kind { case 1, 2 { \"a\"; } else { \"b\"; } } }\nprivate:\nhelper() is int { 1; } }\n";
    assert_eq!(
        fmt(text),
        "module app v1.0.0;

object A {
    name() is string {
        match kind {
            case 1, 2 {
                \"a\";
            }
            else {
                \"b\";
            }
        }
    }

    private:

    helper() is int {
        1;
    }
}
"
    );
}

#[test]
fn invalid_source_is_a_syntax_error() {
    let source =
        SourceFile::new("t.ok", "module app v1.0.0;\nmain() is int { let = 1; }\n").unwrap();
    assert!(matches!(format(&source), Err(FormatError::Syntax(_))));
}
