use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericType { Int, Byte, Float, Rat, Bool, Str }
impl NumericType {
    pub(crate) fn result_type(self) -> ResultType {
        ResultType::Scalar(match self { Self::Int => ScalarType::Int, Self::Byte => ScalarType::Byte,
            Self::Float => ScalarType::Float, Self::Rat => ScalarType::Rat, Self::Bool => ScalarType::Bool, Self::Str => ScalarType::Str })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldValue { pub field: FieldRef, pub value: ValueId }
impl OperandSource for FieldValue {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) { self.value.append(mode, out); }
}


/// An early self has no destroy obligation until the constructor commits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructionStatus { Complete, Incomplete }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstructionCompletion { pub state: ValueId }
impl OperandSource for ConstructionCompletion {
    fn append(&self, _: OperandMode, out: &mut Vec<ReferencedOperand>) { self.state.append(OperandMode::Consume, out); }
}

operation_group! { CoreOp {
    IntAdd { left: ValueId, right: ValueId }
        => ("int.add", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    IntSub { left: ValueId, right: ValueId }
        => ("int.sub", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    IntMul { left: ValueId, right: ValueId }
        => ("int.mul", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    IntDiv { left: ValueId, right: ValueId }
        => ("int.div", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: true });
    IntMod { left: ValueId, right: ValueId }
        => ("int.mod", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntAnd { left: ValueId, right: ValueId }
        => ("int.and", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntOr { left: ValueId, right: ValueId }
        => ("int.or", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntXor { left: ValueId, right: ValueId }
        => ("int.xor", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntShl { left: ValueId, right: ValueId }
        => ("int.shl", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntShr { left: ValueId, right: ValueId }
        => ("int.shr", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntUshr { left: ValueId, right: ValueId }
        => ("int.ushr", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntNeg { value: ValueId }
        => ("int.neg", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    IntNot { value: ValueId }
        => ("int.not", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntAbs { value: ValueId }
        => ("int.abs", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    IntSign { value: ValueId }
        => ("int.sign", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    IntBitCount { value: ValueId }
        => ("int.bit_count", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntLeadingZeros { value: ValueId }
        => ("int.leading_zeros", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntTrailingZeros { value: ValueId }
        => ("int.trailing_zeros", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntReverseBits { value: ValueId }
        => ("int.reverse_bits", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntSwapBytes { value: ValueId }
        => ("int.swap_bytes", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntRotateLeft { value: ValueId, count: ValueId }
        => ("int.rotl", [value: Observe, count: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    IntRotateRight { value: ValueId, count: ValueId }
        => ("int.rotr", [value: Observe, count: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    ByteAnd { left: ValueId, right: ValueId }
        => ("byte.and", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    ByteOr { left: ValueId, right: ValueId }
        => ("byte.or", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    ByteXor { left: ValueId, right: ValueId }
        => ("byte.xor", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    ByteShl { left: ValueId, right: ValueId }
        => ("byte.shl", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    ByteShr { left: ValueId, right: ValueId }
        => ("byte.shr", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    ByteUshr { left: ValueId, right: ValueId }
        => ("byte.ushr", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    ByteNot { value: ValueId }
        => ("byte.not", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: false });
    ByteBitCount { value: ValueId }
        => ("byte.bit_count", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    ByteLeadingZeros { value: ValueId }
        => ("byte.leading_zeros", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    ByteTrailingZeros { value: ValueId }
        => ("byte.trailing_zeros", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    ByteReverseBits { value: ValueId }
        => ("byte.reverse_bits", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: false });
    ByteSwapBytes { value: ValueId }
        => ("byte.swap_bytes", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: false });
    ByteRotateLeft { value: ValueId, count: ValueId }
        => ("byte.rotl", [value: Observe, count: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    ByteRotateRight { value: ValueId, count: ValueId }
        => ("byte.rotr", [value: Observe, count: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    FloatAdd { left: ValueId, right: ValueId }
        => ("float.add", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    FloatSub { left: ValueId, right: ValueId }
        => ("float.sub", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    FloatMul { left: ValueId, right: ValueId }
        => ("float.mul", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    FloatDiv { left: ValueId, right: ValueId }
        => ("float.div", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    FloatMod { left: ValueId, right: ValueId }
        => ("float.mod", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    FloatPow { left: ValueId, right: ValueId }
        => ("float.pow", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    FloatNeg { value: ValueId }
        => ("float.neg", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    FloatAbs { value: ValueId }
        => ("float.abs", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    FloatSign { value: ValueId }
        => ("float.sign", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    FloatSqrt { value: ValueId }
        => ("float.sqrt", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Float), trap: true });
    RatAdd { left: ValueId, right: ValueId }
        => ("rat.add", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: false });
    RatSub { left: ValueId, right: ValueId }
        => ("rat.sub", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: false });
    RatMul { left: ValueId, right: ValueId }
        => ("rat.mul", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: false });
    RatDiv { left: ValueId, right: ValueId }
        => ("rat.div", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: true });
    RatMod { left: ValueId, right: ValueId }
        => ("rat.mod", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: true });
    RatNeg { value: ValueId }
        => ("rat.neg", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: false });
    RatAbs { value: ValueId }
        => ("rat.abs", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: false });
    RatSign { value: ValueId }
        => ("rat.sign", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    IntEq { left: ValueId, right: ValueId }
        => ("int.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    IntNe { left: ValueId, right: ValueId }
        => ("int.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    IntLt { left: ValueId, right: ValueId }
        => ("int.lt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    IntLe { left: ValueId, right: ValueId }
        => ("int.le", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    IntGt { left: ValueId, right: ValueId }
        => ("int.gt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    IntGe { left: ValueId, right: ValueId }
        => ("int.ge", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    ByteEq { left: ValueId, right: ValueId }
        => ("byte.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    ByteNe { left: ValueId, right: ValueId }
        => ("byte.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    ByteLt { left: ValueId, right: ValueId }
        => ("byte.lt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    ByteLe { left: ValueId, right: ValueId }
        => ("byte.le", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    ByteGt { left: ValueId, right: ValueId }
        => ("byte.gt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    ByteGe { left: ValueId, right: ValueId }
        => ("byte.ge", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    FloatEq { left: ValueId, right: ValueId }
        => ("float.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    FloatNe { left: ValueId, right: ValueId }
        => ("float.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    FloatLt { left: ValueId, right: ValueId }
        => ("float.lt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    FloatLe { left: ValueId, right: ValueId }
        => ("float.le", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    FloatGt { left: ValueId, right: ValueId }
        => ("float.gt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    FloatGe { left: ValueId, right: ValueId }
        => ("float.ge", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    RatEq { left: ValueId, right: ValueId }
        => ("rat.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    RatNe { left: ValueId, right: ValueId }
        => ("rat.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    RatLt { left: ValueId, right: ValueId }
        => ("rat.lt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    RatLe { left: ValueId, right: ValueId }
        => ("rat.le", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    RatGt { left: ValueId, right: ValueId }
        => ("rat.gt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    RatGe { left: ValueId, right: ValueId }
        => ("rat.ge", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    BoolEq { left: ValueId, right: ValueId }
        => ("bool.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    BoolNe { left: ValueId, right: ValueId }
        => ("bool.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    EnumEq { left: ValueId, right: ValueId }
        => ("enum.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    EnumNe { left: ValueId, right: ValueId }
        => ("enum.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    CharEq { left: ValueId, right: ValueId }
        => ("char.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    CharNe { left: ValueId, right: ValueId }
        => ("char.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    CharLt { left: ValueId, right: ValueId }
        => ("char.lt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    CharLe { left: ValueId, right: ValueId }
        => ("char.le", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    CharGt { left: ValueId, right: ValueId }
        => ("char.gt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    CharGe { left: ValueId, right: ValueId }
        => ("char.ge", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    StrEq { left: ValueId, right: ValueId }
        => ("str.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    StrNe { left: ValueId, right: ValueId }
        => ("str.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    StrLt { left: ValueId, right: ValueId }
        => ("str.lt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    StrLe { left: ValueId, right: ValueId }
        => ("str.le", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    StrGt { left: ValueId, right: ValueId }
        => ("str.gt", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    StrGe { left: ValueId, right: ValueId }
        => ("str.ge", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    ValEq { left: ValueId, right: ValueId }
        => ("val.eq", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: true });
    ValNe { left: ValueId, right: ValueId }
        => ("val.ne", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: true });
    BoolNot { value: ValueId }
        => ("bool.not", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    RatNew { numerator: ValueId, denominator: ValueId }
        => ("rat.new", [numerator: Observe, denominator: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Rat), trap: true });
    RatNumerator { value: ValueId }
        => ("rat.numerator", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    RatDenominator { value: ValueId }
        => ("rat.denominator", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    Convert { value: ValueId, from: NumericType, to: NumericType }
        => ("cvt", [value: Observe], Rule::Convert { from: *from, to: *to });
    ByteFrom { value: ValueId, context: Option<String> }
        => ("byte.from", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Byte), trap: true });
    Undef { ty: TypeId }
        => ("undef", [], Rule::Data { ty: ResultType::Exact(*ty), trap: false });
    Global { declaration: DeclId, ty: TypeId }
        => ("global", [], Rule::Data { ty: ResultType::Exact(*ty), trap: false }.retains(Retention::Global(*declaration)));
    StrConcat { left: ValueId, right: ValueId }
        => ("str.concat", [left: Observe, right: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrLen { value: ValueId }
        => ("str.len", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    StrUtf8 { value: ValueId }
        => ("str.utf8", [value: Observe], Rule::Data { ty: ResultType::ArrayOf(Box::new(ResultType::Scalar(ScalarType::Byte))), trap: false });
    StrSlice { value: ValueId, start: ValueId, end: ValueId }
        => ("str.slice", [value: Observe, start: Observe, end: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: true });
    StrSplit { value: ValueId, separator: ValueId }
        => ("str.split", [value: Observe, separator: Observe], Rule::Data { ty: ResultType::ArrayOf(Box::new(ResultType::Scalar(ScalarType::Str))), trap: false });
    StrReplace { value: ValueId, target: ValueId, replacement: ValueId }
        => ("str.replace", [value: Observe, target: Observe, replacement: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrReplaceFirst { value: ValueId, target: ValueId, replacement: ValueId }
        => ("str.replace_first", [value: Observe, target: Observe, replacement: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrRepeat { value: ValueId, count: ValueId }
        => ("str.repeat", [value: Observe, count: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: true });
    StrTrim { value: ValueId }
        => ("str.trim", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrChomp { value: ValueId }
        => ("str.chomp", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrTrimStart { value: ValueId }
        => ("str.trim_start", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrTrimEnd { value: ValueId }
        => ("str.trim_end", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrLower { value: ValueId }
        => ("str.lower", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrUpper { value: ValueId }
        => ("str.upper", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    StrFind { value: ValueId, part: ValueId }
        => ("str.find", [value: Observe, part: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    StrFindLast { value: ValueId, part: ValueId }
        => ("str.find_last", [value: Observe, part: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    StrStartsWith { value: ValueId, part: ValueId }
        => ("str.starts_with", [value: Observe, part: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    StrEndsWith { value: ValueId, part: ValueId }
        => ("str.ends_with", [value: Observe, part: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    StrCharacters { value: ValueId }
        => ("str.characters", [value: Observe], Rule::Data { ty: ResultType::ArrayOf(Box::new(ResultType::Scalar(ScalarType::Char))), trap: false });
    RangeNew { start: ValueId, end: ValueId, inclusive: bool }
        => ("range.new", [start: Observe, end: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Range), trap: true });
    RangeStart { value: ValueId }
        => ("range.start", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    RangeEnd { value: ValueId }
        => ("range.end", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    RangeLen { value: ValueId }
        => ("range.len", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: true });
    ObjNew { declaration: DeclId, type_args: Vec<TypeId>, fields: Vec<ValueId>, packet: Option<ValueId>, construction: ConstructionStatus }
        => ("obj.new", [fields: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Object { decl: *declaration, args: type_args.clone() }, packet: *packet, checked: true, identity: *construction == ConstructionStatus::Complete }.retains(Retention::Union(fields.clone())).requires(if *construction == ConstructionStatus::Incomplete { Postcondition::IncompleteIdentity } else { Postcondition::PreserveFullDependencies }));
    ObjNext { object: ValueId, updates: Vec<FieldValue>, packet: Option<ValueId> }
        => ("obj.next", [object: Borrow, updates: Borrow, packet: Consume], Rule::Construct { ty: ResultType::SuccessorOf(*object), packet: *packet, checked: true, identity: true }.retains(Retention::ObjectFields { object: *object, updates: updates.clone() }).requires(Postcondition::CreatesMustUse));
    ObjInit { object: ValueId, field: FieldRef, value: ValueId, completion: Option<ConstructionCompletion> }
        => ("obj.init", [object: Borrow, value: Borrow, completion: Consume], Rule::ObjectInit { object: *object, field: *field, completion: *completion });
    ObjGet { object: ValueId, field: FieldRef }
        => ("obj.get", [object: Borrow], Rule::Data { ty: ResultType::Field(*field), trap: false }.retains(Retention::Field { base: *object, field: *field }));
    ObjPending { object: ValueId, field: FieldRef }
        => ("obj.pending", [object: Borrow], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    OnceLoad { object: ValueId, field: FieldRef }
        => ("once.load", [object: Borrow], Rule::Data { ty: ResultType::Field(*field), trap: false }.retains(Retention::Field { base: *object, field: *field }));
    OnceStore { object: ValueId, field: FieldRef, value: ValueId }
        => ("once.store", [object: Borrow, value: Borrow], Rule::Intent.requires(Postcondition::OnceValuePlain));
    SuccTake { value: ValueId, ty: TypeId }
        => ("succ.take", [value: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: false }.retains(Retention::Value(*value)).requires(Postcondition::DischargeMustUse(*value)));
    RoleWrap { value: ValueId, ty: TypeId }
        => ("role.wrap", [value: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: false }.retains(Retention::Value(*value)));
    RoleNarrow { value: ValueId, ty: TypeId }
        => ("role.narrow", [value: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: true }.retains(Retention::Value(*value)));
    VariantOf { value: ValueId, ty: TypeId }
        => ("variant.of", [value: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: false }.retains(Retention::Value(*value)));
    Retag { value: ValueId, ty: TypeId }
        => ("retag", [value: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: false }.retains(Retention::Value(*value)));
    VariantNew { ty: TypeId, case: u32, fields: Vec<ValueId>, packet: Option<ValueId> }
        => ("variant.new", [fields: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: true, identity: true }.retains(Retention::Union(fields.clone())));
    VariantTag { value: ValueId }
        => ("variant.tag", [value: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    VariantGet { value: ValueId, case: u32, field: FieldRef }
        => ("variant.get", [value: Borrow], Rule::Data { ty: ResultType::Field(*field), trap: true }.retains(Retention::Field { base: *value, field: *field }));
    CapMake { owner: ValueId, ty: TypeId, packet: Option<ValueId> }
        => ("cap.make", [owner: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::OwnerGraph(*owner)));
    FnClosure { function: FuncId, captures: Vec<ValueId>, ty: TypeId, packet: Option<ValueId> }
        => ("fn.closure", [captures: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::Union(captures.clone())));
    FnBind { receiver: ValueId, function: FuncId, ty: TypeId, packet: Option<ValueId> }
        => ("fn.bind", [receiver: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::Value(*receiver)));
    FnBindRole { receiver: ValueId, role: DeclId, slot: u32, ty: TypeId, packet: Option<ValueId> }
        => ("fn.bind.role", [receiver: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::Value(*receiver)));
    AnyBox { value: ValueId, ty: TypeId, packet: Option<ValueId> }
        => ("any.box", [value: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::Value(*value)));
    FailSecondary { primary: ValueId, cleanup: ValueId }
        => ("fail.secondary", [primary: Observe, cleanup: Observe], Rule::Data { ty: ResultType::Service(ServiceType::Failure), trap: false }.retains(Retention::CarrierGraph(vec![*primary, *cleanup])).requires(Postcondition::FullCarrierGraphOwner));
    FailCaused { new: ValueId, handled: ValueId }
        => ("fail.caused", [new: Observe, handled: Observe], Rule::Data { ty: ResultType::Service(ServiceType::Failure), trap: false }.retains(Retention::CarrierGraph(vec![*new, *handled])).requires(Postcondition::FullCarrierGraphOwner));
    FailIsThrown { failure: ValueId }
        => ("fail.is_thrown", [failure: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    TypeOf { ty: TypeId, descriptor: DescriptorContract }
        => ("type.of", [], Rule::Data { ty: ResultType::Scalar(ScalarType::Descriptor), trap: false });
    TestOk { condition: ValueId }
        => ("test.ok", [condition: Observe], Rule::Check);
    TestIs { left: ValueId, right: ValueId }
        => ("test.is", [left: Observe, right: Observe], Rule::Check);
    TestIsnt { left: ValueId, right: ValueId }
        => ("test.isnt", [left: Observe, right: Observe], Rule::Check);
    TestFail { message: ValueId }
        => ("test.fail", [message: Observe], Rule::Check);
    EnumCase { declaration: DeclId, case: u32 }
        => ("enum.case", [], Rule::Data { ty: ResultType::ExactEnum(*declaration), trap: false });
    FakeNew { declaration: DeclId, packet: Option<ValueId> }
        => ("fake.new", [packet: Consume], Rule::Construct { ty: ResultType::Object { decl: *declaration, args: vec![] }, packet: *packet, checked: true, identity: true });
} }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallArgument { pub value: ValueId, pub convention: OperandConvention }
impl OperandSource for CallArgument {
    fn append(&self, _: OperandMode, out: &mut Vec<ReferencedOperand>) {
        self.value.append(match self.convention { OperandConvention::Borrow => OperandMode::Borrow, OperandConvention::Consume => OperandMode::Consume }, out);
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSite {
    pub args: Vec<CallArgument>, pub packet: Option<ValueId>, pub deps: Vec<ValueId>,
    pub descriptors: Vec<ValueId>, pub type_args: Vec<TypeId>, pub summary: Option<SummaryId>,
}
impl CallSite {
    // Summaries number the receiver/environment first, then explicit arguments.
    pub(crate) fn with_receiver(&self, receiver: CallArgument) -> Self {
        let mut call = self.clone(); call.args.insert(0, receiver); call
    }
}
impl OperandSource for CallSite {
    fn append(&self, _: OperandMode, out: &mut Vec<ReferencedOperand>) {
        for arg in &self.args { arg.value.append(match arg.convention {
            OperandConvention::Borrow => OperandMode::Borrow, OperandConvention::Consume => OperandMode::Consume,
        }, out); }
        self.packet.append(OperandMode::Consume, out); self.deps.append(OperandMode::Observe, out);
        self.descriptors.append(OperandMode::Observe, out);
    }
}

operation_group! { CallOp {
    Direct { function: FuncId, call: CallSite }
        => ("call", [call: Borrow], Rule::Call(call.clone()));
    Role { receiver: CallArgument, role: DeclId, slot: u32, call: CallSite }
        => ("call.role", [receiver: Borrow, call: Borrow], Rule::Call(call.with_receiver(*receiver)));
    Closure { callee: CallArgument, call: CallSite }
        => ("fn.call", [callee: Borrow, call: Borrow], Rule::Call(call.with_receiver(*callee)));
    Cap { cap: CallArgument, field: FieldRef, call: CallSite }
        => ("call.cap", [cap: Borrow, call: Borrow], Rule::Call(call.with_receiver(*cap)));
    Native { function: FuncId, call: CallSite }
        => ("call.native", [call: Borrow], Rule::Call(call.clone()).requires(Postcondition::NativeTransferableOnly));
    Intrinsic { function: FuncId, call: CallSite }
        => ("call.intrinsic", [call: Borrow], Rule::Call(call.clone()));
    Resume { frame: ValueId, function: FuncId, call: CallSite }
        => ("call.resume", [frame: Consume, call: Borrow], Rule::Resume { frame: *frame, call: call.clone() });
} }


operation_group! { CollectionOp {
    ArrayNew { values: Vec<ValueId>, ty: TypeId, packet: Option<ValueId> }
        => ("array.new", [values: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::Union(values.clone())));
    ArraySlice { array: ValueId, start: ValueId, end: ValueId, packet: Option<ValueId> }
        => ("array.slice", [array: Borrow, start: Borrow, end: Borrow, packet: Consume], Rule::Construct { ty: ResultType::SameAs(*array), packet: *packet, checked: true, identity: false }.retains(Retention::Slice { base: *array, start: *start, end: *end }));
    ArrayConcat { arrays: Vec<ValueId>, ty: TypeId, packet: Option<ValueId> }
        => ("array.concat", [arrays: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::Union(arrays.clone())));
    ArrayReplace { array: ValueId, index: ValueId, value: ValueId, packet: Option<ValueId> }
        => ("array.replace", [array: Borrow, index: Borrow, value: Borrow, packet: Consume], Rule::Construct { ty: ResultType::SameAs(*array), packet: *packet, checked: true, identity: false }.retains(Retention::ArrayReplace { array: *array, index: *index, value: *value }));
    ArrayRemove { array: ValueId, index: ValueId, packet: Option<ValueId> }
        => ("array.remove", [array: Borrow, index: Borrow, packet: Consume], Rule::Construct { ty: ResultType::SameAs(*array), packet: *packet, checked: true, identity: false }.retains(Retention::ArrayRemove { array: *array, index: *index }));
    ArrayDifference { array: ValueId, removed: Vec<ValueId>, packet: Option<ValueId> }
        => ("array.difference", [array: Borrow, removed: Borrow, packet: Consume], Rule::Construct { ty: ResultType::SameAs(*array), packet: *packet, checked: true, identity: false }.retains(Retention::Difference { base: *array, removed: removed.clone() }));
    MapNew { entries: Vec<(ValueId, ValueId)>, ty: TypeId, packet: Option<ValueId> }
        => ("map.new", [entries: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::Entries(entries.clone())));
    MapMerge { maps: Vec<ValueId>, ty: TypeId, packet: Option<ValueId> }
        => ("map.merge", [maps: Borrow, packet: Consume], Rule::Construct { ty: ResultType::Exact(*ty), packet: *packet, checked: false, identity: false }.retains(Retention::MapMerge(maps.clone())));
    MapDifference { map: ValueId, removed: Vec<ValueId>, packet: Option<ValueId> }
        => ("map.difference", [map: Borrow, removed: Borrow, packet: Consume], Rule::Construct { ty: ResultType::SameAs(*map), packet: *packet, checked: true, identity: false }.retains(Retention::Difference { base: *map, removed: removed.clone() }));
    ArrayLen { array: ValueId }
        => ("array.len", [array: Borrow], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    ArrayGet { array: ValueId, index: ValueId }
        => ("array.get", [array: Borrow, index: Borrow], Rule::Data { ty: ResultType::Element(*array), trap: true }.retains(Retention::Index { base: *array, index: *index }));
    ArrayJoin { array: ValueId, separator: ValueId }
        => ("array.join", [array: Borrow, separator: Borrow], Rule::Data { ty: ResultType::Scalar(ScalarType::Str), trap: false });
    ArrayRetag { array: ValueId, ty: TypeId }
        => ("array.retag", [array: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: false }.retains(Retention::Value(*array)));
    MapLen { map: ValueId }
        => ("map.len", [map: Borrow], Rule::Data { ty: ResultType::Scalar(ScalarType::Int), trap: false });
    MapHas { map: ValueId, key: ValueId }
        => ("map.has", [map: Borrow, key: Borrow], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    MapGet { map: ValueId, key: ValueId }
        => ("map.get", [map: Borrow, key: Borrow], Rule::Data { ty: ResultType::MapValue(*map), trap: true }.retains(Retention::MapEntry { map: *map, key: *key }));
    MapKeys { map: ValueId }
        => ("map.keys", [map: Borrow], Rule::Data { ty: ResultType::ArrayOf(Box::new(ResultType::MapKey(*map))), trap: false }.retains(Retention::Keys(*map)));
    MapValues { map: ValueId }
        => ("map.values", [map: Borrow], Rule::Data { ty: ResultType::ArrayOf(Box::new(ResultType::MapValue(*map))), trap: false }.retains(Retention::Values(*map)));
    MapKeyAt { map: ValueId, index: ValueId }
        => ("map.key_at", [map: Borrow, index: Borrow], Rule::Data { ty: ResultType::MapKey(*map), trap: true }.retains(Retention::MapPosition { map: *map, index: *index, key: true }));
    MapValueAt { map: ValueId, index: ValueId }
        => ("map.value_at", [map: Borrow, index: Borrow], Rule::Data { ty: ResultType::MapValue(*map), trap: true }.retains(Retention::MapPosition { map: *map, index: *index, key: false }));
    MapRetag { map: ValueId, ty: TypeId }
        => ("map.retag", [map: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: false }.retains(Retention::Value(*map)));
    BuilderNew { element: TypeId }
        => ("builder.new", [], Rule::BufferNew(ResultType::BuilderOf(*element)));
    BuilderPush { buffer: ValueId, value: ValueId, packet: Option<ValueId> }
        => ("builder.push", [buffer: Consume, value: Borrow, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Update });
    BuilderAppend { buffer: ValueId, array: ValueId, packet: Option<ValueId> }
        => ("builder.append", [buffer: Consume, array: Borrow, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Update });
    BuilderAppendRange { buffer: ValueId, start: ValueId, end: ValueId, inclusive: bool, packet: Option<ValueId> }
        => ("builder.append.range", [buffer: Consume, start: Borrow, end: Borrow, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Update });
    BuilderFinish { buffer: ValueId, packet: Option<ValueId> }
        => ("builder.finish", [buffer: Consume, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Finish });
    BuilderAbandon { buffer: ValueId, packet: Option<ValueId> }
        => ("builder.abandon", [buffer: Consume, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Abandon });
    MapBuilderNew { key: TypeId, value: TypeId }
        => ("mbuilder.new", [], Rule::BufferNew(ResultType::MapBuilderOf(*key, *value)));
    MapBuilderPut { buffer: ValueId, key: ValueId, value: ValueId, packet: Option<ValueId> }
        => ("mbuilder.put", [buffer: Consume, key: Borrow, value: Borrow, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Update });
    MapBuilderAppend { buffer: ValueId, map: ValueId, packet: Option<ValueId> }
        => ("mbuilder.append", [buffer: Consume, map: Borrow, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Update });
    MapBuilderFinish { buffer: ValueId, packet: Option<ValueId> }
        => ("mbuilder.finish", [buffer: Consume, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Finish });
    MapBuilderAbandon { buffer: ValueId, packet: Option<ValueId> }
        => ("mbuilder.abandon", [buffer: Consume, packet: Consume], Rule::Buffer { buffer: *buffer, packet: *packet, action: BufferAction::Abandon });
} }
