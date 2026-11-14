//! #537: builder-produced arrays keep aliased resources alive through GC,
//! await and region cleanup. The existing #580 oracle expects exactly BA.
use okkama_sparrow::{Dst, Instruction as I, Register, Type, TypeRef};

#[test]
fn builder_dispatch_resources_survive_finish_and_cleanup_once_in_reverse_order() {
    for ready in [false, true] {
        let source = include_str!("all_futures.spa").replace("bool true", &format!("bool {ready}"));
        let mut program = okkama_sparrow::assemble(&source).unwrap();
        let builder_type = TypeRef(program.types.len() as u32);
        program.types.push(Type::Builder(TypeRef(7)));
        let function = &mut program.functions[16];
        // This straight-line fixture has no branch targets or handlers to
        // relocate. Async resume PCs are computed from the expanded code.
        assert!(function.handlers.is_empty());
        assert!(function.code.iter().all(|i| {
            let mut target = false;
            i.visit(&mut |access| target |= matches!(access, okkama_sparrow::Access::Target(_)));
            !target
        }));
        let builder = Register(function.registers.len() as u32);
        function.registers.push(builder_type);
        let I::ArrayNew { dst, elements } = function.code[10].clone() else { panic!("array fixture") };
        let mut code = vec![I::BuilderNew { dst: Dst(builder) }];
        code.extend(elements.into_iter().map(|value| I::BuilderPush { dst: Dst(builder), builder, value }));
        code.push(I::BuilderFinish { dst, builder });
        function.code.splice(10..11, code);
        program.verify().unwrap();
        let bytes = okkama_sparrow::encode(&program).unwrap();
        assert!(sparrow_core::run_tests(bytes, Box::new(std::io::sink())).unwrap());
    }
}
