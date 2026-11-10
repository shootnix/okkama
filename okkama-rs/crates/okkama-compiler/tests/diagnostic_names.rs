//! Type names in diagnostics (#338): two nominal types that share a name
//! are told apart as the module of the diagnostic can name them, and a call
//! that fits no candidate shows what it could have called.

use okkama_compiler::lowering::{self, LoadedModule, ModuleLoader, RootFile};
use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::{self, ParseOptions};
use std::collections::HashMap;

/// Modules from memory by import path.
struct Memory(HashMap<String, String>);

impl ModuleLoader for Memory {
    fn load(&mut self, path: &str, _importer: &str) -> Result<Option<LoadedModule>, String> {
        let Some(text) = self.0.get(path) else {
            return Ok(None);
        };
        let source = SourceFile::new(format!("/lib/{path}.ok"), text.clone()).unwrap();
        let program = syntax::parse(&source, &ParseOptions::default()).map_err(|_| format!("syntax error in {path}"))?;
        Ok(Some(LoadedModule { id: source.name().to_string(), program, source }))
    }

    fn resolve(&mut self, path: &str, _importer: &str) -> Result<Option<String>, String> {
        Ok(self.0.contains_key(path).then(|| format!("/lib/{path}.ok")))
    }
}

/// `object Money` in modules `shop/money` and `bank/money`.
fn money(module: &str) -> (&str, String) {
    (
        module,
        format!("module {module} v1.0.0;\nobject Money {{ has cents is int; Money(:cents); cents() is int {{ cents; }} }}\n"),
    )
}

/// The errors of `root` (the module `app`) with `modules`, as `file: message`
/// for imported files and as the message alone for the root.
fn errors(modules: &[(&str, String)], root: &str) -> Vec<String> {
    let mut loader = Memory(modules.iter().map(|(path, text)| (path.to_string(), text.clone())).collect());
    let source = SourceFile::new("/app/main.ok", format!("module app v1.0.0;\n{root}")).unwrap();
    let program = syntax::parse(&source, &ParseOptions::default()).unwrap();
    match lowering::check_linked_files(&[RootFile { program: &program, source: &source }], &mut loader) {
        Ok(_) => Vec::new(),
        Err(Ok(errors)) => errors
            .iter()
            .map(|error| match &error.source {
                Some(file) if file.name() != "/app/main.ok" => format!("{}: {}", file.name(), error.error.message),
                _ => error.error.message.clone(),
            })
            .collect(),
        Err(Err(text)) => panic!("{text}"),
    }
}

const TILL: &str = "object Till { Till(); count(amount is shop.Money) is int { amount.cents(); } }\n";

#[test]
fn two_types_of_one_name_are_told_apart_by_their_namespaces() {
    let modules = [money("shop/money"), money("bank/money")];
    let root = format!(
        "import \"shop/money\" as shop;\nimport \"bank/money\" as bank;\n{TILL}\
         main() is int {{ Till().count(bank.Money(cents: 500)); }}\n"
    );
    assert_eq!(
        errors(&modules, &root),
        ["No action \"count\" on object \"Till\" accepts (bank.Money)\nAvailable: count(amount is shop.Money)"]
    );
    // The program that passes the right one has no error.
    let fixed = root.replace("bank.Money(cents: 500)", "shop.Money(cents: 500)");
    assert_eq!(errors(&modules, &fixed), Vec::<String>::new());
}

#[test]
fn a_name_no_other_type_has_stays_short() {
    let modules = [money("shop/money")];
    let root = format!("import \"shop/money\" as shop;\n{TILL}main() is int {{ Till().count(5); }}\n");
    assert_eq!(
        errors(&modules, &root),
        ["No action \"count\" on object \"Till\" accepts (int)\nAvailable: count(amount is Money)"]
    );
}

#[test]
fn a_type_is_named_as_the_module_imports_it() {
    let modules = [money("shop/money"), money("bank/money")];
    let call = "object Till { Till(); count(amount is Money) is int { amount.cents(); } }\n\
                main() is int { Till().count(bank.Money(cents: 500)); }\n";
    // Taken into scope, `Money` is shop's: the short name is that one.
    let taken = format!("import \"shop/money\" take Money;\nimport \"bank/money\" as bank;\n{call}");
    assert_eq!(
        errors(&modules, &taken),
        ["No action \"count\" on object \"Till\" accepts (bank.Money)\nAvailable: count(amount is Money)"]
    );
    // The default namespace is the last segment of the path.
    let default = TILL.replace("shop.Money", "money.Money");
    let default = format!(
        "import \"shop/money\";\nimport \"bank/money\" as bank;\n{default}\
         main() is int {{ Till().count(bank.Money(cents: 500)); }}\n"
    );
    assert_eq!(
        errors(&modules, &default),
        ["No action \"count\" on object \"Till\" accepts (bank.Money)\nAvailable: count(amount is money.Money)"]
    );
    // Two namespaces of one module: the first in alphabetical order.
    let twice = format!(
        "import \"shop/money\" as shop;\nimport \"bank/money\" as vault;\nimport \"bank/money\" as bank;\n{TILL}\
         main() is int {{ Till().count(vault.Money(cents: 500)); }}\n"
    );
    assert_eq!(
        errors(&modules, &twice),
        ["No action \"count\" on object \"Till\" accepts (bank.Money)\nAvailable: count(amount is shop.Money)"]
    );
}

#[test]
fn a_type_the_module_cannot_name_is_named_by_its_module() {
    // `a/money` and `b/money` reach the root only through `store`.
    let modules = [
        money("a/money"),
        money("b/money"),
        (
            "store",
            "module store v1.0.0;\nimport \"a/money\" as a;\nimport \"b/money\" as b;\n\
             object Store { Store(); take(m is a.Money) is int { m.cents(); } make() is b.Money { b.Money(cents: 1); } }\n"
                .to_string(),
        ),
    ];
    let root = "import \"store\" take Store;\nmain() is int { Store().take(Store().make()); }\n";
    assert_eq!(
        errors(&modules, root),
        ["No action \"take\" on object \"Store\" accepts (Money from \"b/money\")\n\
          Available: take(m is Money from \"a/money\")"]
    );
}

#[test]
fn each_module_names_the_types_its_own_way() {
    let modules = [
        money("shop/money"),
        money("bank/money"),
        (
            "audit",
            "module audit v1.0.0;\nimport \"shop/money\" as s;\nimport \"bank/money\" as b;\n\
             object Audit { Audit(); check(m is s.Money) is int { m.cents(); } run() is int { Audit().check(b.Money(cents: 1)); } }\n"
                .to_string(),
        ),
    ];
    let root = format!(
        "import \"shop/money\" as shop;\nimport \"bank/money\" as bank;\nimport \"audit\";\n{TILL}\
         main() is int {{ Till().count(bank.Money(cents: 500)); }}\n"
    );
    let mut found = errors(&modules, &root);
    found.sort();
    assert_eq!(
        found,
        [
            "/lib/audit.ok: No action \"check\" on object \"Audit\" accepts (b.Money)\nAvailable: check(m is s.Money)",
            "No action \"count\" on object \"Till\" accepts (bank.Money)\nAvailable: count(amount is shop.Money)",
        ]
    );
}

#[test]
fn objects_roles_and_enums_of_one_name_are_qualified() {
    let modules = [
        ("a/shape", "module a/shape v1.0.0;\nobject Shape { Shape(); }\nenum Color = (red, green);\nenum Mark = (dot, ring(size is int));\n".to_string()),
        ("b/shape", "module b/shape v1.0.0;\nrole Shape { area() is int; }\nenum Color = (red, green);\nenum Mark = (dot, ring(size is int));\n".to_string()),
    ];
    let root = "import \"a/shape\" as a;\nimport \"b/shape\" as b;\n\
                object Box { Box(); put(s is b.Shape) is int { 0; } paint(c is b.Color) is int { 0; } mark(m is b.Mark) is int { 0; } }\n\
                main() is int { Box().put(a.Shape()) + Box().paint(a.Color.red) + Box().mark(a.Mark.ring(2)); }\n";
    assert_eq!(
        errors(&modules, root),
        [
            "No action \"put\" on object \"Box\" accepts (a.Shape)\nAvailable: put(s is b.Shape)",
            "No action \"paint\" on object \"Box\" accepts (a.Color)\nAvailable: paint(c is b.Color)",
            "No action \"mark\" on object \"Box\" accepts (a.Mark)\nAvailable: mark(m is b.Mark)",
        ]
    );
}

#[test]
fn qualified_names_inside_composite_types() {
    let modules = [money("shop/money"), money("bank/money")];
    let root = "import \"shop/money\" as shop;\nimport \"bank/money\" as bank;\n\
                object Box<T> { has item is T; Box(:item); }\n\
                object Ledger { Ledger();\n\
                  all(items is @shop.Money, by is %string:shop.Money, boxed is Box<shop.Money>, rule is (shop.Money) is int) is int { 0; } }\n\
                main() is int {\n\
                  let money = bank.Money(cents: 1);\n\
                  let items = [money];\n\
                  let by = [\"a\": money];\n\
                  let boxed = Box(item: money);\n\
                  let rule = (m is bank.Money) is int { 0; };\n\
                  Ledger().all(items, by, boxed, rule);\n\
                }\n";
    assert_eq!(
        errors(&modules, root),
        ["No action \"all\" on object \"Ledger\" accepts (@bank.Money, %string:bank.Money, Box<bank.Money>, (bank.Money) is int)\n\
          Available: all(items is @shop.Money, by is %string:shop.Money, boxed is Box<shop.Money>, rule is (shop.Money) is int)"]
    );
}

#[test]
fn constructors_assignments_and_ambiguity_name_the_types_apart() {
    let modules = [money("shop/money"), money("bank/money")];
    let head = "import \"shop/money\" as shop;\nimport \"bank/money\" as bank;\n";
    // One constructor: its argument is checked, and named.
    let wallet = format!(
        "{head}object Wallet {{ has m is shop.Money; Wallet(:m); }}\n\
         main() is int {{ Wallet(m: bank.Money(cents: 1)); 0; }}\n"
    );
    assert_eq!(
        errors(&modules, &wallet),
        ["Constructor \"Wallet\" argument \"m\" must be shop.Money, got bank.Money"]
    );
    let wallets = format!(
        "{head}object Wallet {{ has m is shop.Money;\n\
         Wallet(money is shop.Money) {{ m = money; }}\n\
         Wallet(label is string) {{ m = shop.Money(cents: 0); }} }}\n\
         main() is int {{ Wallet(bank.Money(cents: 1)); 0; }}\n"
    );
    assert_eq!(
        errors(&modules, &wallets),
        ["No constructor of \"Wallet\" accepts (bank.Money)\nAvailable: Wallet(money is shop.Money); Wallet(label is string)"]
    );
    let assigned = format!("{head}main() is int {{ let m is shop.Money = bank.Money(cents: 1); m.cents(); }}\n");
    let found = errors(&modules, &assigned);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("shop.Money") && found[0].contains("bank.Money"), "{found:?}");
    let tied = format!(
        "{head}object Pair {{ Pair(); put(a is shop.Money, b is any) is int {{ 1; }} put(a is any, b is shop.Money) is int {{ 2; }} }}\n\
         main() is int {{ let m = shop.Money(cents: 1); Pair().put(m, m); }}\n"
    );
    assert_eq!(
        errors(&modules, &tied),
        ["Ambiguous call of action \"put\" on object \"Pair\" with (shop.Money, shop.Money): \
          put(shop.Money, any) and put(any, shop.Money) fit equally well"]
    );
}

#[test]
fn many_candidates_are_listed_up_to_four() {
    let root = "object Many { Many();\n\
                a(x is int) is int { 0; } a(x is string) is int { 0; } a(x is bool) is int { 0; }\n\
                a(x is char) is int { 0; } a(x is float) is int { 0; } a(x is int, y is string = \"!\") is int { 0; } }\n\
                main() is int { Many().a([1]); }\n";
    assert_eq!(
        errors(&[], root),
        ["No action \"a\" on object \"Many\" accepts (@int)\n\
          Available: a(x is int); a(x is string); a(x is bool); a(x is char); and 2 more"]
    );
}

#[test]
fn diagnostics_never_show_internal_identities() {
    let modules = [money("shop/money"), money("bank/money")];
    let root = format!(
        "import \"shop/money\" as shop;\nimport \"bank/money\" as bank;\n{TILL}\
         main() is int {{ Till().count(bank.Money(cents: 500)); }}\n"
    );
    for message in errors(&modules, &root) {
        assert!(!message.contains('#') && !message.contains("::"), "{message}");
    }
}
