//! The OIR verifier rejects mutated OIR without panicking: the input is a
//! program, a NUL byte, then mutations (pairs: which operand, which value).
#![no_main]
use libfuzzer_sys::fuzz_target;
use okkama_compiler::SourceFile;
use okkama_compiler::ir::ValueId;
use okkama_compiler::syntax::{self, ParseOptions, prune};

fuzz_target!(|data: &[u8]| {
    let (text, edits) = match data.iter().position(|&byte| byte == 0) {
        Some(at) => (&data[..at], &data[at + 1..]),
        None => (data, &[][..]),
    };
    let Ok(text) = std::str::from_utf8(text) else { return };
    let Ok(source) = SourceFile::new("fuzz.ok", text) else { return };
    let Ok(program) = syntax::parse(&source, &ParseOptions::default()) else { return };
    let Ok(mut module) = okkama_compiler::lowering::lower_program(&prune::strip_test_items(program)) else { return };
    for edit in edits.chunks_exact(2) {
        let values: usize = module.functions.iter().map(|function| function.values.len()).max().unwrap_or(0);
        let mut operands: Vec<&mut ValueId> = module
            .functions
            .iter_mut()
            .flat_map(|function| function.blocks.iter_mut())
            .flat_map(|block| {
                let insts = block.insts.iter_mut().flat_map(|inst| inst.kind.operands_mut());
                insts.chain(block.term.kind.operands_mut())
            })
            .collect();
        if operands.is_empty() || values == 0 {
            break;
        }
        let at = edit[0] as usize % operands.len();
        *operands[at] = ValueId(edit[1] as usize % (values + 2));
    }
    let _ = okkama_compiler::ir::verify(&module);
});
