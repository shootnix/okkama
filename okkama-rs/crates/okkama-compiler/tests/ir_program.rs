//! Module/function lowering preserves source locations as well as values.

use okkama_compiler::SourceFile;
use okkama_compiler::ir::{
    Attr, FuncId, InstKind, IntBinaryOp, ResultType, TermKind, Type, ValueId,
};
use okkama_compiler::lowering::lower_program;
use okkama_compiler::syntax::{self, ParseOptions};

#[test]
fn return_and_instruction_spans_refer_to_original_source() {
    for (body, return_source) in [("1 + 2;", "1 + 2;"), ("return 1 + 2;", "return 1 + 2;")] {
        let text = format!("module app/math v1.2.3; main() is int {{ {body} }}");
        let source = SourceFile::new("test.ok", text).unwrap();
        let ast = syntax::parse(&source, &ParseOptions::default()).unwrap();
        let module = lower_program(&ast).unwrap();

        assert_eq!(module.name, "app/math");
        assert_eq!(module.version, "v1.2.3");
        assert_eq!(module.span, ast.span);
        assert_eq!(module.entry, Some(FuncId(0)));
        assert_eq!(module.functions.len(), 1);
        let function = &module.functions[0];
        assert_eq!(function.symbol, "main");
        assert_eq!(function.result, ResultType::Value(Type::Int));
        assert_eq!(function.attrs, [Attr::Entry]);
        assert!(function.params.is_empty());
        assert!(source.text()[function.span.range()].starts_with("main() is int"));

        assert_eq!(function.blocks.len(), 1);
        let block = &function.blocks[0];
        assert_eq!(block.insts.len(), 3);
        for (inst, spelling) in block.insts.iter().zip(["1", "2", "1 + 2"]) {
            assert_eq!(&source.text()[inst.span.range()], spelling);
        }
        assert_eq!(
            block.insts[2].kind,
            InstKind::IntBinary {
                op: IntBinaryOp::Add,
                left: ValueId(0),
                right: ValueId(1),
            }
        );
        assert_eq!(block.insts[2].result, Some(ValueId(2)));
        assert_eq!(block.term.kind, TermKind::Ret(Some(ValueId(2))));
        assert_eq!(&source.text()[block.term.span.range()], return_source);
    }
}

#[test]
fn empty_library_module_keeps_its_import_metadata() {
    let source = SourceFile::new(
        "library.ok",
        "module library v1.0.0; import \"math\" as m take Number, value;",
    )
    .unwrap();
    let ast = syntax::parse(&source, &ParseOptions::default()).unwrap();
    let module = lower_program(&ast).unwrap();

    assert_eq!(module.name, "library");
    assert!(module.functions.is_empty());
    assert_eq!(module.entry, None);
    assert_eq!(module.imports.len(), 1);
    let import = &module.imports[0];
    assert_eq!(import.path, "math");
    assert_eq!(import.alias.as_deref(), Some("m"));
    assert_eq!(import.take, ["Number", "value"]);
    assert_eq!(
        &source.text()[import.span.range()],
        "import \"math\" as m take Number, value;"
    );
    assert_eq!(
        module.to_string(),
        concat!(
            "oir 1 high\n",
            "module library v1.0.0\n",
            "requires core\n",
            "import \"math\" as m take Number, value\n",
        )
    );
}
