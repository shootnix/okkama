use super::*;

#[test]
fn numeric_and_scalar_opcode_operands_have_positive_and_mutation_cases() {
    let cases = vec![
        (CoreOp::IntAdd { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntSub { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntMul { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntDiv { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntMod { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntAnd { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntOr { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntXor { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntShl { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntShr { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntUshr { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntNeg { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntNot { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntAbs { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntSign { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntBitCount { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntLeadingZeros { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntTrailingZeros { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntReverseBits { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntSwapBytes { value: ValueId::from_u32(0) }, vec![Type::Int]),
        (CoreOp::IntRotateLeft {
            value: ValueId::from_u32(0),
            count: ValueId::from_u32(1),
        }, vec![Type::Int, Type::Int]),
        (CoreOp::IntRotateRight {
            value: ValueId::from_u32(0),
            count: ValueId::from_u32(1),
        }, vec![Type::Int, Type::Int]),
        (CoreOp::ByteAnd { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteOr { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteXor { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteShl { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Int]),
        (CoreOp::ByteShr { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Int]),
        (CoreOp::ByteUshr { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteNot { value: ValueId::from_u32(0) }, vec![Type::Byte]),
        (CoreOp::ByteBitCount { value: ValueId::from_u32(0) }, vec![Type::Byte]),
        (CoreOp::ByteLeadingZeros { value: ValueId::from_u32(0) }, vec![Type::Byte]),
        (CoreOp::ByteTrailingZeros { value: ValueId::from_u32(0) }, vec![Type::Byte]),
        (CoreOp::ByteReverseBits { value: ValueId::from_u32(0) }, vec![Type::Byte]),
        (CoreOp::ByteSwapBytes { value: ValueId::from_u32(0) }, vec![Type::Byte]),
        (CoreOp::ByteRotateLeft {
            value: ValueId::from_u32(0),
            count: ValueId::from_u32(1),
        }, vec![Type::Byte, Type::Int]),
        (CoreOp::ByteRotateRight {
            value: ValueId::from_u32(0),
            count: ValueId::from_u32(1),
        }, vec![Type::Byte, Type::Int]),
        (CoreOp::FloatAdd { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatSub { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatMul { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatDiv { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatMod { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatPow { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatNeg { value: ValueId::from_u32(0) }, vec![Type::Float]),
        (CoreOp::FloatAbs { value: ValueId::from_u32(0) }, vec![Type::Float]),
        (CoreOp::FloatSign { value: ValueId::from_u32(0) }, vec![Type::Float]),
        (CoreOp::FloatSqrt { value: ValueId::from_u32(0) }, vec![Type::Float]),
        (CoreOp::RatAdd { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatSub { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatMul { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatDiv { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatMod { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatNeg { value: ValueId::from_u32(0) }, vec![Type::Rat]),
        (CoreOp::RatAbs { value: ValueId::from_u32(0) }, vec![Type::Rat]),
        (CoreOp::RatSign { value: ValueId::from_u32(0) }, vec![Type::Rat]),
        (CoreOp::IntEq { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntNe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntLt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntLe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntGt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::IntGe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Int, Type::Int]),
        (CoreOp::ByteEq { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteNe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteLt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteLe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteGt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::ByteGe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Byte, Type::Byte]),
        (CoreOp::FloatEq { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatNe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatLt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatLe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatGt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::FloatGe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Float, Type::Float]),
        (CoreOp::RatEq { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatNe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatLt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatLe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatGt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::RatGe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Rat, Type::Rat]),
        (CoreOp::BoolEq { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Bool, Type::Bool]),
        (CoreOp::BoolNe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Bool, Type::Bool]),
        (CoreOp::CharEq { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Char, Type::Char]),
        (CoreOp::CharNe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Char, Type::Char]),
        (CoreOp::CharLt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Char, Type::Char]),
        (CoreOp::CharLe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Char, Type::Char]),
        (CoreOp::CharGt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Char, Type::Char]),
        (CoreOp::CharGe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Char, Type::Char]),
        (CoreOp::StrEq { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrNe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrLt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrLe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrGt { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrGe { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::BoolNot { value: ValueId::from_u32(0) }, vec![Type::Bool]),
        (CoreOp::RatNumerator { value: ValueId::from_u32(0) }, vec![Type::Rat]),
        (CoreOp::RatDenominator { value: ValueId::from_u32(0) }, vec![Type::Rat]),
        (CoreOp::StrConcat { left: ValueId::from_u32(0), right: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrLen { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::StrUtf8 { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::StrSlice {
            value: ValueId::from_u32(0),
            start: ValueId::from_u32(1),
            end: ValueId::from_u32(2),
        }, vec![Type::Str, Type::Int, Type::Int]),
        (CoreOp::StrSplit { value: ValueId::from_u32(0), separator: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrReplace {
            value: ValueId::from_u32(0),
            target: ValueId::from_u32(1),
            replacement: ValueId::from_u32(2),
        }, vec![Type::Str, Type::Str, Type::Str]),
        (CoreOp::StrReplaceFirst {
            value: ValueId::from_u32(0),
            target: ValueId::from_u32(1),
            replacement: ValueId::from_u32(2),
        }, vec![Type::Str, Type::Str, Type::Str]),
        (CoreOp::StrRepeat { value: ValueId::from_u32(0), count: ValueId::from_u32(1) }, vec![Type::Str, Type::Int]),
        (CoreOp::StrTrim { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::StrChomp { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::StrTrimStart { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::StrTrimEnd { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::StrLower { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::StrUpper { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::StrFind { value: ValueId::from_u32(0), part: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrFindLast { value: ValueId::from_u32(0), part: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrStartsWith { value: ValueId::from_u32(0), part: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrEndsWith { value: ValueId::from_u32(0), part: ValueId::from_u32(1) }, vec![Type::Str, Type::Str]),
        (CoreOp::StrCharacters { value: ValueId::from_u32(0) }, vec![Type::Str]),
        (CoreOp::RangeStart { value: ValueId::from_u32(0) }, vec![Type::Range]),
        (CoreOp::RangeEnd { value: ValueId::from_u32(0) }, vec![Type::Range]),
        (CoreOp::RangeLen { value: ValueId::from_u32(0) }, vec![Type::Range]),
        (CoreOp::RatNew {
            numerator: ValueId::from_u32(0),
            denominator: ValueId::from_u32(1),
        }, vec![Type::Int, Type::Int]),
    ];
    for (op, types) in cases {
        let mut module = ModuleBuilder::new("scalar_types", Profile::High);
        let params = types.iter().cloned().map(|ty| module.intern_type(ty).unwrap()).collect::<Vec<_>>();
        let function = module.declare_function(function("operation", params, vec![])).unwrap();
        let scope = module.declare_scope(ScopeDecl {
            function, parent: None, kind: ScopeKind::Action, loc: None,
        }).unwrap();
        let failure = module.intern_type(Type::Service(ServiceType::Failure)).unwrap();
        let mut builder = module.function_builder(function, scope).unwrap();
        let handler = builder.create_block(scope, &[ParamSpec::new(failure)]).unwrap();
        builder.set_unwind(Some(BlockUnwind { target: Target { block: handler, args: vec![] } })).unwrap();
        builder.emit_schema(Operation::Core(op.clone())).unwrap();
        builder.terminate(TermKind::Ret { values: vec![] }).unwrap();
        builder.switch_to(handler).unwrap();
        let failure = builder.block_params(handler).unwrap()[0];
        builder.terminate(TermKind::Raise(failure)).unwrap();
        let body = builder.finish().unwrap();
        module.define_function(function, body).unwrap();
        let positive = module.finish().unwrap();
        verify(&positive).unwrap_or_else(|report| panic!("{op:?}: {report:?}"));
        for (index, ty) in types.iter().enumerate() {
            let mut negative = positive.clone();
            let bad = if ty == &Type::Bool { Type::Int } else { Type::Bool };
            let bad = negative.types.intern(bad).unwrap();
            negative.functions.get_mut(function).unwrap().signature.params[index] = bad;
            body_mut(&mut negative).values.get_mut(ValueId::from_u32(index as u32)).unwrap().ty = bad;
            rejected(&negative, RuleId::T1);
        }
    }
}
