//! Exact semantics of value instructions without an inline fast path.
//!
//! Machine code stores the words used by the instruction (in `visit` order) and
//! calls [`rt_exec`]; the helper re-reads the instruction, knows the static type
//! of every register and returns the defined word. The behaviour and failure
//! texts follow the previous engine.
#![allow(unsafe_code)]

use crate::heap::{self, Integer, Payload};
use crate::program::Kind;
use crate::runtime::{Ctx, FailureKind, Runtime};
use crate::word::{self, Repr, UNBOUND, Word};
use num_bigint::BigInt;
use num_integer::Integer as _;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use okkama_sparrow::{Access, Cmp, Instruction as I, Type, TypeRef};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::rc::Rc;

type Outcome = Result<Word, String>;

// ---- payload values ----

#[derive(Debug)]
pub struct Rational(pub BigRational);
#[derive(Debug)]
pub struct Builder {
    pub repr: Repr,
    pub items: Vec<Word>,
}
#[derive(Debug)]
pub struct RangeValue {
    pub start: Word,
    pub end: Word,
    pub inclusive: bool,
}
#[derive(Debug)]
pub struct AnyBox {
    pub value: Word,
    pub ty: TypeRef,
    pub repr: Repr,
}
#[derive(Debug)]
pub struct MapValue {
    pub key: Repr,
    pub value: Repr,
    pub entries: BTreeMap<Key, (Word, Word)>,
}

macro_rules! payload {
    ($($ty:ty => |$this:ident, $visit:ident| $body:block)*) => {$(
        impl Payload for $ty {
            fn trace(&self, $visit: &mut dyn FnMut(Word, Repr)) {
                let $this = self;
                $body
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    )*};
}
payload! {
    Rational => |_this, _visit| {}
    RangeValue => |this, visit| { visit(this.start, Repr::Int); visit(this.end, Repr::Int); }
    Builder => |this, visit| { for &item in &this.items { visit(item, this.repr); } }
    AnyBox => |this, visit| { visit(this.value, this.repr); }
    MapValue => |this, visit| {
        for &(key, value) in this.entries.values() {
            visit(key, this.key);
            visit(value, this.value);
        }
    }
}

/// Map keys own immutable scalar data; order follows the previous engine.
#[derive(Debug, Clone)]
pub enum Key {
    Int(BigInt),
    Float(f64),
    Raw(u64),
    Str(Rc<str>),
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a.cmp(b),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b).expect("finite float key"),
            (Self::Raw(a), Self::Raw(b)) => a.cmp(b),
            (Self::Str(a), Self::Str(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}
impl Key {
    fn rank(&self) -> u8 {
        match self {
            Self::Int(_) => 0,
            Self::Float(_) => 1,
            Self::Raw(_) => 2,
            Self::Str(_) => 3,
        }
    }
}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Key {}

// ---- conversions between words and Rust values ----

impl Runtime {
    pub fn big_value(&self, word: Word) -> BigInt {
        match self.heap.integer(word) {
            Integer::Small(n) => BigInt::from(n),
            Integer::Big(n) => n.clone(),
        }
    }
    pub fn small_or_big(&self, word: Word) -> Option<i64> {
        match self.heap.integer(word) {
            Integer::Small(n) => Some(n),
            Integer::Big(n) => n.to_i64(),
        }
    }
    pub fn rational(&self, word: Word) -> &BigRational {
        &self.heap.payload::<Rational>(word).expect("rational value").0
    }
    pub fn rational_new(&mut self, value: BigRational) -> Word {
        self.heap.allocate_payload(Box::new(Rational(value)))
    }
    pub fn text_new(&mut self, text: &str) -> Word {
        self.heap.allocate_text(text)
    }
    fn ty(&self, function: usize, register: okkama_sparrow::Register) -> Option<&Type> {
        let source = &self.loaded.program.functions[function];
        self.loaded.program.ty(source.registers[register.0 as usize])
    }
    fn kind_of(&self, ty: TypeRef) -> Kind {
        Kind::of(&self.loaded.program, ty)
    }
    pub(crate) fn key(&self, word: Word, kind: Kind, ty: Option<&Type>) -> Key {
        match (kind, ty) {
            (Kind::Int, _) => Key::Int(self.big_value(word)),
            (Kind::Float, _) => Key::Float(f64::from_bits(word)),
            (Kind::Raw | Kind::Byte, _) => Key::Raw(word),
            (_, Some(Type::Str)) => Key::Str(Rc::from(self.heap.text(word))),
            _ => Key::Raw(word),
        }
    }
}

fn integer_word(runtime: &mut Runtime, value: BigInt) -> Word {
    runtime.heap.big(value)
}

fn decimal_parts(value: f64) -> (i128, i32) {
    let text = crate::scalar::float_text(value);
    let (mantissa, exponent) = text.split_once('e').map_or((text.as_str(), 0), |(m, e)| {
        (m, e.parse::<i32>().expect("float decimal exponent"))
    });
    let fraction = mantissa.split_once('.').map_or(0, |(_, f)| f.len() as i32);
    let digits = mantissa
        .replace('.', "")
        .parse()
        .expect("shortest binary64 decimal fits i128");
    (digits, exponent - fraction)
}

fn decimal_ratio(value: f64) -> BigRational {
    let (digits, exponent) = decimal_parts(value);
    let power = BigInt::from(10).pow(exponent.unsigned_abs());
    if exponent >= 0 {
        BigRational::from_integer(BigInt::from(digits) * power)
    } else {
        BigRational::new(BigInt::from(digits), power)
    }
}

/// DECIMAL128 half-even (34 significant digits), then binary64: the Okkama/JVM
/// conversion contract.
fn rational_to_float(value: &BigRational) -> Result<f64, String> {
    if value.is_zero() {
        return Ok(0.0);
    }
    let n = value.numer().abs();
    let d = value.denom();
    let mut scale = 33 - (n.to_string().len() as i64 - d.to_string().len() as i64);
    let minimum = BigInt::from(10).pow(33);
    let (mut quotient, remainder, divisor) = loop {
        let power = BigInt::from(10).pow(
            u32::try_from(scale.unsigned_abs()).map_err(|_| "rational conversion exponent too large")?,
        );
        let (numerator, divisor) = if scale >= 0 { (&n * power, d.clone()) } else { (n.clone(), d * power) };
        let (q, r) = numerator.div_rem(&divisor);
        if q < minimum {
            scale += 1;
        } else if q.to_string().len() > 34 {
            scale -= 1;
        } else {
            break (q, r, divisor);
        }
    };
    let twice = remainder * 2;
    if twice > divisor || (twice == divisor && quotient.is_odd()) {
        quotient += 1;
    }
    let sign = if value.is_negative() { "-" } else { "" };
    let result: f64 = format!("{sign}{quotient}e{}", -scale)
        .parse()
        .map_err(|_| "invalid rational conversion")?;
    if !result.is_finite() {
        return Err("Cannot convert rat to float: expected a finite binary64 value".into());
    }
    Ok(result)
}

fn finite(value: f64) -> Outcome {
    if value.is_finite() {
        Ok(value.to_bits())
    } else {
        Err("Float value must contain a finite number".into())
    }
}

fn bool_word(value: bool) -> Word {
    Word::from(value)
}

fn compare_flag(op: Cmp, order: Ordering) -> Word {
    bool_word(match op {
        Cmp::Eq => order.is_eq(),
        Cmp::Ne => !order.is_eq(),
        Cmp::Lt => order.is_lt(),
        Cmp::Le => !order.is_gt(),
        Cmp::Gt => order.is_gt(),
        Cmp::Ge => !order.is_lt(),
    })
}

fn slice_bounds(runtime: &Runtime, length: usize, start: Word, end: Word) -> Result<std::ops::Range<usize>, String> {
    let (start, end) = (runtime.big_value(start), runtime.big_value(end));
    if start.is_negative() {
        return Err("Slice start must not be negative".into());
    }
    if start > end {
        return Err("Slice start must be less than or equal to slice end".into());
    }
    if end > BigInt::from(length) {
        return Err("Slice end must not exceed length".into());
    }
    Ok(start.to_usize().expect("validated")..end.to_usize().expect("validated"))
}

// ---- structural equality ----

/// A new array for register `dst` of `function`: byte arrays (`@byte`) are
/// packed, whatever produced the words.
pub(crate) fn new_array(runtime: &mut Runtime, function: usize, dst: okkama_sparrow::Register, repr: Repr, items: &[Word]) -> Word {
    let program = &runtime.loaded.program;
    let ty = program.functions[function].registers[dst.0 as usize];
    if let Some(Type::Array(element)) = program.ty(ty)
        && Kind::of(program, *element) == Kind::Byte {
            let bytes: Vec<u8> = items.iter().map(|&w| w as u8).collect();
            return runtime.heap.allocate_bytes(&bytes);
        }
    runtime.heap.allocate_array(repr, items)
}

pub(crate) fn equal(runtime: &Runtime, left: Word, right: Word, ty: TypeRef) -> Result<bool, String> {
    let program = &runtime.loaded.program;
    let mut pending = vec![(left, right, ty)];
    // Visited pairs, only once a composite value is entered.
    let mut seen: crate::word::FastSet<(Word, Word)> = Default::default();
    while let Some((a, b, ty)) = pending.pop() {
        let kind = runtime.kind_of(ty);
        match kind {
            Kind::Int => {
                if runtime.big_value(a) != runtime.big_value(b) {
                    return Ok(false);
                }
                continue;
            }
            Kind::Float | Kind::Raw | Kind::Byte => {
                if a != b {
                    return Ok(false);
                }
                continue;
            }
            Kind::Ref => {}
        }
        if a == UNBOUND || b == UNBOUND {
            return Err("cannot compare internal or undefined values".into());
        }
        match program.ty(ty) {
            Some(Type::Any) => {
                if a == b {
                    continue;
                }
                return Err("Cannot compare any without narrowing".into());
            }
            Some(Type::Str) => {
                if runtime.heap.text(a) != runtime.heap.text(b) {
                    return Ok(false);
                }
            }
            Some(Type::Rat) => {
                if runtime.rational(a) != runtime.rational(b) {
                    return Ok(false);
                }
            }
            Some(Type::Range) => {
                let (x, y) = (
                    runtime.heap.payload::<RangeValue>(a).expect("range"),
                    runtime.heap.payload::<RangeValue>(b).expect("range"),
                );
                if x.inclusive != y.inclusive
                    || runtime.big_value(x.start) != runtime.big_value(y.start)
                    || runtime.big_value(x.end) != runtime.big_value(y.end)
                {
                    return Ok(false);
                }
            }
            Some(Type::Array(_)) if runtime.heap.is_bytes(a) && runtime.heap.is_bytes(b) => {
                if runtime.heap.bytes(a) != runtime.heap.bytes(b) {
                    return Ok(false);
                }
            }
            Some(Type::Array(element)) => {
                let (x, y) = (runtime.heap.elements(a), runtime.heap.elements(b));
                if x.len() != y.len() {
                    return Ok(false);
                }
                if a != b && seen.insert((a, b)) {
                    pending.extend(x.iter().zip(y.iter()).rev().map(|(&x, &y)| (x, y, *element)));
                }
            }
            Some(Type::Map(_, value)) => {
                let (x, y) = (
                    runtime.heap.payload::<MapValue>(a).expect("map"),
                    runtime.heap.payload::<MapValue>(b).expect("map"),
                );
                if x.entries.len() != y.entries.len() {
                    return Ok(false);
                }
                if a != b && seen.insert((a, b)) {
                    for ((k, (_, v)), (k2, (_, v2))) in x.entries.iter().zip(&y.entries) {
                        if k != k2 {
                            return Ok(false);
                        }
                        pending.push((*v, *v2, *value));
                    }
                }
            }
            Some(Type::Object(_) | Type::Successor(_) | Type::Role(_)) => {
                let (ha, hb) = (runtime.heap.header_of(a), runtime.heap.header_of(b));
                if heap::kind(ha) != heap::OBJECT || heap::kind(hb) != heap::OBJECT {
                    if a != b {
                        return Ok(false);
                    }
                    continue;
                }
                if heap::payload(ha) != heap::payload(hb) {
                    return Ok(false);
                }
                if a != b && seen.insert((a, b)) {
                    let fields = &program.objects[heap::payload(ha) as usize].fields;
                    for (index, (_, field)) in fields.iter().enumerate().rev() {
                        let (x, y) = (runtime.heap.field(a, index), runtime.heap.field(b, index));
                        let unbound = |w: Word| match runtime.kind_of(*field) {
                            Kind::Float | Kind::Raw | Kind::Byte => w == word::UNBOUND_RAW,
                            _ => w == UNBOUND,
                        };
                        if unbound(x) || unbound(y) {
                            return Err("cannot compare unbound object fields".into());
                        }
                        pending.push((x, y, *field));
                    }
                }
            }
            _ => {
                // Handles, futures, closures, ranges: identity.
                if a != b {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

// ---- the helper ----

/// `builder.push`: append a word to a builder (no collection here: the
/// items vector is Rust memory). Returns the builder.
#[inline(never)]
pub extern "C" fn rt_builder_push(ctx: *mut Ctx, builder: Word, item: Word) -> Word {
    let runtime = unsafe { &mut *(*ctx).runtime };
    runtime.heap.payload_mut::<Builder>(builder).expect("builder").items.push(item);
    builder
}

/// Execute instruction `pc` of `function` with its used words at `uses`.
///
/// SAFETY: called from machine code with its context; `uses` holds one word
/// per `Access::Use` of the instruction, in `visit` order.
#[inline(never)]
pub extern "C" fn rt_exec(ctx: *mut Ctx, function: u64, pc: u64, uses: *const Word) -> Word {
    let frame = crate::runtime::frame_pointer!();
    let ctx = unsafe { &mut *ctx };
    let runtime = unsafe { &mut *ctx.runtime };
    let function = function as usize;
    // SAFETY: the loaded program is never mutated while machine code runs, so
    // the instruction stays valid while the runtime is borrowed mutably.
    let instruction: &I = unsafe { &*(&runtime.loaded.program.functions[function].code[pc as usize] as *const I) };
    runtime.count(|_| format!("exec {}", instruction.mnemonic()));
    // Uses in visit order: inline for the common case, spilled past 16 (map
    // and array literals can use any number of registers).
    let mut inline = [okkama_sparrow::Register(u32::MAX); 16];
    let mut spilled = Vec::new();
    let mut count = 0;
    instruction.visit(&mut |access| {
        if let Access::Use(register) = access {
            if count < inline.len() {
                inline[count] = register;
            } else {
                if spilled.is_empty() {
                    spilled.extend_from_slice(&inline);
                }
                spilled.push(register);
            }
            count += 1;
        }
    });
    let registers: &[okkama_sparrow::Register] = if count <= inline.len() { &inline[..count] } else { &spilled };
    let words = unsafe { std::slice::from_raw_parts(uses, count) };
    let value = |register: &okkama_sparrow::Register| {
        words[registers.iter().position(|r| r == register).expect("used register")]
    };
    let outcome = match instruction {
        // Spawning runs the worker's driver now: it needs the context.
        I::WorkerSpawn { entry, args, direct, .. } => {
            let arguments: Vec<Word> = args.iter().map(value).collect();
            match &runtime.limits.natives {
                Some(allowed) if !allowed.contains(&crate::budget::NativeGroup::Process) => {
                    Err("Process workers need the Process permission".to_string())
                }
                _ => crate::worker::spawn(runtime, &mut *ctx, value(entry), &arguments, *direct),
            }
        }
        _ => execute(runtime, function, instruction, &value),
    };
    match outcome {
        Ok(word) => {
            if runtime.heap.needs_collection() {
                let repr = def_repr(runtime, function, instruction);
                runtime.safepoint(frame, ctx, &[(word, repr)]);
            }
            word
        }
        Err(message) => {
            ctx.failure = runtime.failure_new(FailureKind::Program(message));
            ctx.status = crate::runtime::FAILED;
            0
        }
    }
}

fn def_repr(runtime: &Runtime, function: usize, instruction: &I) -> Repr {
    let mut repr = Repr::Raw;
    instruction.visit(&mut |access| {
        if let Access::Def(register) = access {
            repr = runtime.loaded.functions[function].kinds[register.0 as usize].repr();
        }
    });
    repr
}

fn execute(runtime: &mut Runtime, function: usize, instruction: &I, value: &dyn Fn(&okkama_sparrow::Register) -> Word) -> Outcome {
    // SAFETY: as in rt_exec, the loaded program is immutable during execution.
    let kinds: &[Kind] = unsafe { &*(runtime.loaded.functions[function].kinds.as_slice() as *const [Kind]) };
    match instruction {
        I::IntAnd { left, right, .. }
        | I::IntOr { left, right, .. }
        | I::IntXor { left, right, .. }
        | I::IntShl { left, right, .. }
        | I::IntShr { left, right, .. }
        | I::IntUshr { left, right, .. } => {
            let shift = matches!(instruction, I::IntShl { .. } | I::IntShr { .. } | I::IntUshr { .. });
            let r = if shift {
                runtime
                    .small_or_big(value(right))
                    .filter(|n| (0..64).contains(n))
                    .ok_or("Shift count must be in 0..63")?
            } else {
                runtime.small_or_big(value(right)).ok_or("Bitwise operand is outside signed-64 range")?
            };
            let l = runtime.small_or_big(value(left)).ok_or("Bitwise operand is outside signed-64 range")?;
            let result = match instruction {
                I::IntAnd { .. } => l & r,
                I::IntOr { .. } => l | r,
                I::IntXor { .. } => l ^ r,
                I::IntShl { .. } => l << r,
                I::IntShr { .. } => l >> r,
                _ => ((l as u64) >> r) as i64,
            };
            Ok(runtime.heap.int(result))
        }
        I::ByteAnd { left, right, .. }
        | I::ByteOr { left, right, .. }
        | I::ByteXor { left, right, .. }
        | I::ByteShl { left, right, .. }
        | I::ByteShr { left, right, .. }
        | I::ByteUshr { left, right, .. } => {
            let l = value(left) as u8;
            let result = match instruction {
                I::ByteAnd { .. } => l & value(right) as u8,
                I::ByteOr { .. } => l | value(right) as u8,
                I::ByteXor { .. } => l ^ value(right) as u8,
                _ => {
                    let count = runtime
                        .small_or_big(value(right))
                        .filter(|n| (0..8).contains(n))
                        .ok_or("Shift count must be in 0..7")?;
                    if matches!(instruction, I::ByteShl { .. }) { l << count } else { l >> count }
                }
            };
            Ok(Word::from(result))
        }
        I::ByteNot { operand, .. } => Ok(Word::from(!(value(operand) as u8))),
        I::Bits { op, byte, value: source, count, .. } => {
            use okkama_sparrow::BitOp;
            let word = if *byte {
                value(source) as i64
            } else {
                runtime.small_or_big(value(source)).ok_or("Bitwise operand is outside signed-64 range")?
            };
            let rotation = || -> Result<u32, String> {
                count
                    .and_then(|c| runtime.small_or_big(value(&c)))
                    .filter(|n| (0..if *byte { 8 } else { 64 }).contains(n))
                    .map(|n| n as u32)
                    .ok_or_else(|| if *byte { "Shift count must be in 0..7" } else { "Shift count must be in 0..63" }.into())
            };
            let result = if *byte {
                let w = word as u8;
                i64::from(match op {
                    BitOp::BitCount => w.count_ones() as u8,
                    BitOp::LeadingZeros => w.leading_zeros() as u8,
                    BitOp::TrailingZeros => w.trailing_zeros() as u8,
                    BitOp::RotateLeft => w.rotate_left(rotation()?),
                    BitOp::RotateRight => w.rotate_right(rotation()?),
                    BitOp::ReverseBits => w.reverse_bits(),
                    BitOp::SwapBytes => w,
                })
            } else {
                match op {
                    BitOp::BitCount => i64::from(word.count_ones()),
                    BitOp::LeadingZeros => i64::from(word.leading_zeros()),
                    BitOp::TrailingZeros => i64::from(word.trailing_zeros()),
                    BitOp::RotateLeft => word.rotate_left(rotation()?),
                    BitOp::RotateRight => word.rotate_right(rotation()?),
                    BitOp::ReverseBits => word.reverse_bits(),
                    BitOp::SwapBytes => word.swap_bytes(),
                }
            };
            Ok(if *byte && !op.counts() { result as u8 as Word } else { runtime.heap.int(result) })
        }
        I::RangeNew { start, end, inclusive, .. } => {
            let (start, end) = (value(start), value(end));
            if runtime.big_value(start) > runtime.big_value(end) {
                return Err("Range start must be less than or equal to range end".into());
            }
            Ok(runtime.heap.allocate_payload(Box::new(RangeValue { start, end, inclusive: *inclusive })))
        }
        I::RangePart { range, end, .. } => {
            let range = runtime.heap.payload::<RangeValue>(value(range)).expect("range");
            let (start, bound, inclusive) = (range.start, range.end, range.inclusive);
            if !*end {
                Ok(start)
            } else if !inclusive {
                Ok(bound)
            } else {
                let next = runtime.big_value(bound) + 1;
                Ok(integer_word(runtime, next))
            }
        }
        I::StrUtf8 { text, .. } => {
            // SAFETY of the copy: the text is a live heap string.
            let bytes = runtime.heap.text(value(text)).as_bytes().to_vec();
            Ok(runtime.heap.allocate_bytes(&bytes))
        }
        I::StrReplaceFirst { text, pattern, replacement, .. } => {
            let replaced = runtime
                .heap
                .text(value(text))
                .replacen(runtime.heap.text(value(pattern)), runtime.heap.text(value(replacement)), 1);
            Ok(runtime.text_new(&replaced))
        }
        I::IntNot { operand, .. } => {
            let n = runtime.small_or_big(value(operand)).ok_or("Bitwise operand is outside signed-64 range")?;
            Ok(runtime.heap.int(!n))
        }
        I::IntNeg { operand, .. } => {
            let n = -runtime.big_value(value(operand));
            Ok(integer_word(runtime, n))
        }
        I::IntAbs { operand, .. } => {
            let n = runtime.big_value(value(operand)).abs();
            Ok(integer_word(runtime, n))
        }
        I::Sign { operand, .. } => {
            let word = value(operand);
            let sign = match kinds[operand.0 as usize] {
                Kind::Int => match runtime.big_value(word).sign() {
                    num_bigint::Sign::Minus => -1,
                    num_bigint::Sign::NoSign => 0,
                    num_bigint::Sign::Plus => 1,
                },
                Kind::Float => {
                    let n = f64::from_bits(word);
                    if n > 0.0 { 1 } else if n < 0.0 { -1 } else { 0 }
                }
                _ => match runtime.rational(word).numer().sign() {
                    num_bigint::Sign::Minus => -1,
                    num_bigint::Sign::NoSign => 0,
                    num_bigint::Sign::Plus => 1,
                },
            };
            Ok(runtime.heap.int(sign))
        }
        I::FloatMod { left, right, .. } | I::FloatPow { left, right, .. } => {
            let op = if matches!(instruction, I::FloatMod { .. }) {
                crate::scalar::FLOAT_MOD
            } else {
                crate::scalar::FLOAT_POW
            };
            crate::scalar::float_binary(op, f64::from_bits(value(left)), f64::from_bits(value(right)))
                .map(f64::to_bits)
                .map_err(Into::into)
        }
        I::FloatSqrt { operand, .. } => {
            let n = f64::from_bits(value(operand));
            if n < 0.0 {
                return Err(format!("Can't take sqrt of {}", crate::scalar::float_text(n)));
            }
            finite(n.sqrt())
        }
        I::Convert { dst, source } => convert(runtime, function, *source, dst.0, value(source)),
        I::ByteFrom { source, context, .. } => byte_from(runtime, kinds[source.0 as usize], value(source), context.as_deref()),
        I::RatNew { left, right, .. } => {
            let (n, d) = (runtime.big_value(value(left)), runtime.big_value(value(right)));
            if d.is_zero() {
                return Err("Division by zero".into());
            }
            Ok(runtime.rational_new(BigRational::new(n, d)))
        }
        I::RatAdd { left, right, .. }
        | I::RatSub { left, right, .. }
        | I::RatMul { left, right, .. }
        | I::RatDiv { left, right, .. }
        | I::RatMod { left, right, .. } => {
            let (a, b) = (runtime.rational(value(left)).clone(), runtime.rational(value(right)).clone());
            if b.is_zero() && matches!(instruction, I::RatDiv { .. } | I::RatMod { .. }) {
                return Err(if matches!(instruction, I::RatDiv { .. }) { "Division by zero" } else { "Modulo by zero" }.into());
            }
            let result = match instruction {
                I::RatAdd { .. } => a + b,
                I::RatSub { .. } => a - b,
                I::RatMul { .. } => a * b,
                I::RatDiv { .. } => a / b,
                _ => a % b,
            };
            Ok(runtime.rational_new(result))
        }
        I::RatNeg { operand, .. } | I::RatAbs { operand, .. } => {
            let n = runtime.rational(value(operand)).clone();
            let result = if matches!(instruction, I::RatNeg { .. }) { -n } else { n.abs() };
            Ok(runtime.rational_new(result))
        }
        I::RatCmp { op, left, right, .. } => {
            let order = runtime.rational(value(left)).cmp(runtime.rational(value(right)));
            Ok(compare_flag(*op, order))
        }
        I::RatPart { value: source, denominator, .. } => {
            let ratio = runtime.rational(value(source));
            let text = if *denominator { ratio.denom() } else { ratio.numer() }.to_string();
            Ok(runtime.text_new(&text))
        }
        I::StrCmp { op, left, right, .. } => {
            let order = runtime.heap.text(value(left)).cmp(runtime.heap.text(value(right)));
            Ok(compare_flag(*op, order))
        }
        I::StrLen { text, .. } => {
            let length = runtime.heap.char_count(value(text));
            Ok(runtime.heap.int(length as i64))
        }
        I::StrSlice { text, start, end, .. } => {
            let text = value(text);
            // Code points map to bytes from the last position reached (#52).
            let count = runtime.heap.char_count(text);
            let range = slice_bounds(runtime, count, value(start), value(end))?;
            let first = runtime.heap.char_offset(text, range.start);
            let last = runtime.heap.char_offset(text, range.end);
            Ok(runtime.heap.substring(text, first, last))
        }
        I::StrSplit { text, separator, .. } => {
            let (text, separator) = (value(text), value(separator));
            // Byte ranges of the parts, then one copy per part.
            let ranges: Vec<(usize, usize)> = {
                let (source, separator) = (runtime.heap.text(text), runtime.heap.text(separator));
                if separator.is_empty() {
                    source.char_indices().map(|(i, c)| (i, i + c.len_utf8())).collect()
                } else {
                    let mut ranges = Vec::new();
                    let mut start = 0;
                    for (at, matched) in source.match_indices(separator) {
                        ranges.push((start, at));
                        start = at + matched.len();
                    }
                    ranges.push((start, source.len()));
                    ranges
                }
            };
            let words: Vec<Word> = ranges.into_iter().map(|(a, b)| runtime.heap.substring(text, a, b)).collect();
            Ok(runtime.heap.allocate_array(Repr::Ref, &words))
        }
        I::StrReplace { text, pattern, replacement, .. } => {
            let replaced = runtime
                .heap
                .text(value(text))
                .replace(runtime.heap.text(value(pattern)), runtime.heap.text(value(replacement)));
            Ok(runtime.text_new(&replaced))
        }
        I::StrTrim { text, .. } => {
            let trimmed = runtime.heap.text(value(text)).trim_matches(char::is_whitespace).to_owned();
            Ok(runtime.text_new(&trimmed))
        }
        I::StrChomp { text, .. } => {
            let source = runtime.heap.text(value(text));
            let chomped = source
                .strip_suffix("\r\n")
                .or_else(|| source.strip_suffix('\n'))
                .unwrap_or(source)
                .to_owned();
            Ok(runtime.text_new(&chomped))
        }
        I::EnumCase { ty, case, .. } => Ok((u64::from(ty.0) << 32) | u64::from(*case)),
        I::EnumCmp { op, left, right, .. } => Ok(compare_flag(*op, value(left).cmp(&value(right)))),
        I::ValCmp { op, left, right, .. } => {
            let ty = runtime.loaded.program.functions[function].registers[left.0 as usize];
            let equal = equal(runtime, value(left), value(right), ty)?;
            Ok(bool_word(if matches!(op, Cmp::Ne) { !equal } else { equal }))
        }
        I::Undef { dst } => Ok(if matches!(kinds[dst.0 .0 as usize], Kind::Float | Kind::Raw | Kind::Byte) {
            word::UNBOUND_RAW
        } else {
            UNBOUND
        }),
        I::AnyBox { source, .. } => {
            let ty = runtime.loaded.program.functions[function].registers[source.0 as usize];
            let repr = kinds[source.0 as usize].repr();
            let boxed = AnyBox { value: value(source), ty, repr };
            Ok(runtime.heap.allocate_payload(Box::new(boxed)))
        }
        I::ArraySlice { array, start, end, .. } => {
            let array = value(array);
            let length = crate::heap::payload(runtime.heap.header_of(array)) as usize;
            let range = slice_bounds(runtime, length, value(start), value(end))?;
            // Copy only the range.
            if runtime.heap.is_bytes(array) {
                let bytes = runtime.heap.bytes(array)[range].to_vec();
                return Ok(runtime.heap.allocate_bytes(&bytes));
            }
            let repr = element_repr(runtime, array);
            let items: Vec<Word> = runtime.heap.elements(array)[range].to_vec();
            Ok(runtime.heap.allocate_array(repr, &items))
        }
        I::ArrayJoin { array, separator, .. } => {
            let separator = runtime.heap.text(value(separator)).to_owned();
            let mut text = String::new();
            for (i, &element) in runtime.heap.elements(value(array)).iter().enumerate() {
                if i != 0 {
                    text.push_str(&separator);
                }
                text.push_str(runtime.heap.text(element));
            }
            Ok(runtime.text_new(&text))
        }
        I::BuilderNew { dst } => {
            let repr = match runtime.ty(function, dst.0) {
                Some(Type::Builder(element)) => runtime.kind_of(*element).repr(),
                _ => return Err("builder.new needs a builder type".into()),
            };
            Ok(runtime.heap.allocate_payload(Box::new(Builder { repr, items: Vec::new() })))
        }
        I::BuilderPush { builder, value: item, .. } => {
            let (builder, item) = (value(builder), value(item));
            runtime.heap.payload_mut::<Builder>(builder).expect("builder").items.push(item);
            Ok(builder)
        }
        I::BuilderAppend { builder, array, .. } => {
            let builder = value(builder);
            let items = runtime.heap.elements(value(array)).to_vec();
            runtime.heap.payload_mut::<Builder>(builder).expect("builder").items.extend(items);
            Ok(builder)
        }
        I::BuilderRange { builder, start, end, inclusive, .. } => {
            let builder = value(builder);
            let (start, end) = (runtime.big_value(value(start)), runtime.big_value(value(end)));
            if start > end {
                return Err("Range start must be less than or equal to range end".into());
            }
            let mut n = start;
            let mut items = Vec::new();
            while if *inclusive { n <= end } else { n < end } {
                items.push(integer_word(runtime, n.clone()));
                n += 1;
            }
            runtime.heap.payload_mut::<Builder>(builder).expect("builder").items.extend(items);
            Ok(builder)
        }
        I::BuilderFinish { dst, builder } => {
            let builder = runtime.heap.payload_mut::<Builder>(value(builder)).expect("builder");
            let (repr, items) = (builder.repr, std::mem::take(&mut builder.items));
            Ok(new_array(runtime, function, dst.0, repr, &items))
        }
        I::MapNew { dst, entries } => {
            let mut map = map_for(runtime, function, dst.0)?;
            let (key_kind, key_type) = map_key(runtime, function, dst.0);
            for (key, item) in entries {
                let (key, item) = (value(key), value(item));
                let k = runtime.key(key, key_kind, key_type.as_ref());
                map.entries.entry(k).and_modify(|e| e.1 = item).or_insert((key, item));
            }
            Ok(runtime.heap.allocate_payload(Box::new(map)))
        }
        I::ArrayConcat { dst, parts } => {
            let words: Vec<Word> = parts.iter().map(value).collect();
            let element = match runtime.ty(function, dst.0) {
                Some(Type::Array(element)) => *element,
                _ => return Err("arr.concat needs an array type".into()),
            };
            // The result is allocated once, for the sum of the parts' lengths.
            let size: usize = words.iter().map(|&w| crate::heap::payload(runtime.heap.header_of(w)) as usize).sum();
            if runtime.kind_of(element) == Kind::Byte {
                let mut bytes = Vec::with_capacity(size);
                for &w in &words {
                    bytes.extend_from_slice(&runtime.heap.byte_view(w));
                }
                return Ok(runtime.heap.allocate_bytes(&bytes));
            }
            let mut items = Vec::with_capacity(size);
            for &w in &words {
                items.extend_from_slice(&runtime.heap.elements(w));
            }
            let repr = runtime.kind_of(element).repr();
            Ok(new_array(runtime, function, dst.0, repr, &items))
        }
        I::ArrayDiff { dst, array, removed } => {
            let element = match runtime.ty(function, dst.0) {
                Some(Type::Array(element)) => *element,
                _ => return Err("arr.diff needs an array type".into()),
            };
            let kind = runtime.kind_of(element);
            let ty = runtime.loaded.program.ty(element).cloned();
            let mut excluded = Vec::new();
            for part in removed {
                excluded.extend_from_slice(&runtime.heap.elements(value(part)));
            }
            let items = runtime.heap.elements(value(array)).into_owned();
            // Scalars and strings compare by key; other elements deeply.
            let keyed = matches!(kind, Kind::Int | Kind::Raw | Kind::Byte) || ty == Some(Type::Str);
            let mut kept = Vec::with_capacity(items.len());
            if keyed {
                let keys: std::collections::BTreeSet<_> = excluded.iter().map(|&w| runtime.key(w, kind, ty.as_ref())).collect();
                kept.extend(items.into_iter().filter(|&w| !keys.contains(&runtime.key(w, kind, ty.as_ref()))));
            } else {
                for item in items {
                    let mut found = false;
                    for &other in &excluded {
                        if equal(runtime, item, other, element)? {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        kept.push(item);
                    }
                }
            }
            if kind == Kind::Byte {
                let bytes: Vec<u8> = kept.iter().map(|&w| w as u8).collect();
                return Ok(runtime.heap.allocate_bytes(&bytes));
            }
            Ok(new_array(runtime, function, dst.0, kind.repr(), &kept))
        }
        I::MapMerge { dst, parts } => {
            let mut map = map_for(runtime, function, dst.0)?;
            for part in parts {
                let entries = &runtime.heap.payload::<MapValue>(value(part)).expect("map").entries;
                for (key, (key_word, item)) in entries {
                    map.entries.entry(key.clone()).and_modify(|e| e.1 = *item).or_insert((*key_word, *item));
                }
            }
            Ok(runtime.heap.allocate_payload(Box::new(map)))
        }
        I::MapDiff { dst, map: base, removed } => {
            let mut map = map_for(runtime, function, dst.0)?;
            map.entries = runtime.heap.payload::<MapValue>(value(base)).expect("map").entries.clone();
            for part in removed {
                let entries = &runtime.heap.payload::<MapValue>(value(part)).expect("map").entries;
                for key in entries.keys() {
                    map.entries.remove(key);
                }
            }
            Ok(runtime.heap.allocate_payload(Box::new(map)))
        }
        I::MapBuilderNew { dst } => Ok({
            let map = map_for(runtime, function, dst.0)?;
            runtime.heap.allocate_payload(Box::new(map))
        }),
        I::MapBuilderPut { builder, key, value: item, .. } => {
            let (builder, key, item) = (value(builder), value(key), value(item));
            let (key_kind, key_type) = map_key(runtime, function, *builder_register(instruction));
            let k = runtime.key(key, key_kind, key_type.as_ref());
            let map = runtime.heap.payload_mut::<MapValue>(builder).expect("map builder");
            map.entries.entry(k).and_modify(|e| e.1 = item).or_insert((key, item));
            Ok(builder)
        }
        I::WorkerBind { action, entry, fields, .. } => {
            let fields = fields.iter().map(|r| (value(r), kinds[r.0 as usize].repr())).collect();
            Ok(crate::worker::bind(runtime, value(action), value(entry), fields))
        }
        I::MapBuilderFinish { builder, .. } => {
            let map = runtime.heap.payload_mut::<MapValue>(value(builder)).expect("map builder");
            let finished = MapValue { key: map.key, value: map.value, entries: std::mem::take(&mut map.entries) };
            Ok(runtime.heap.allocate_payload(Box::new(finished)))
        }
        I::MapLen { map, .. } => {
            let length = runtime.heap.payload::<MapValue>(value(map)).expect("map").entries.len();
            Ok(runtime.heap.int(length as i64))
        }
        I::MapHas { map, key, .. } | I::MapGet { map, key, .. } => {
            let (key_kind, key_type) = map_key(runtime, function, *map);
            let key_word = value(key);
            let k = runtime.key(key_word, key_kind, key_type.as_ref());
            let found = runtime.heap.payload::<MapValue>(value(map)).expect("map").entries.get(&k).map(|e| e.1);
            if matches!(instruction, I::MapHas { .. }) {
                return Ok(bool_word(found.is_some()));
            }
            found.ok_or_else(|| format!("Map key not found: {}", key_text(runtime, key_word, key_kind, key_type.as_ref())))
        }
        I::MapKeys { dst, map } | I::MapValues { dst, map } => {
            let map = runtime.heap.payload::<MapValue>(value(map)).expect("map");
            let keys = matches!(instruction, I::MapKeys { .. });
            let repr = if keys { map.key } else { map.value };
            let items: Vec<Word> = map.entries.values().map(|(k, v)| if keys { *k } else { *v }).collect();
            Ok(new_array(runtime, function, dst.0, repr, &items))
        }
        _ => Err(format!("{} has no runtime path in sparrow-core yet", instruction.mnemonic())),
    }
}

/// `map.lookup`: the value stored under `key`, if any.
pub fn map_find(runtime: &Runtime, function: usize, pc: usize, map: Word, key: Word) -> Option<Word> {
    let I::MapLookup { map: register, .. } = &runtime.loaded.program.functions[function].code[pc] else {
        return None;
    };
    let (key_kind, key_type) = map_key(runtime, function, *register);
    let k = runtime.key(key, key_kind, key_type.as_ref());
    runtime.heap.payload::<MapValue>(map)?.entries.get(&k).map(|e| e.1)
}

fn builder_register(instruction: &I) -> &okkama_sparrow::Register {
    match instruction {
        I::MapBuilderPut { builder, .. } => builder,
        _ => unreachable!("map builder instruction"),
    }
}

fn element_repr(runtime: &Runtime, array: Word) -> Repr {
    Repr::from_bits((runtime.heap.header_of(array) >> 9) & 3)
}

fn map_key(runtime: &Runtime, function: usize, map: okkama_sparrow::Register) -> (Kind, Option<Type>) {
    match runtime.ty(function, map) {
        Some(Type::Map(key, _) | Type::MapBuilder(key, _)) => {
            (runtime.kind_of(*key), runtime.loaded.program.ty(*key).cloned())
        }
        _ => (Kind::Raw, None),
    }
}

fn map_for(runtime: &Runtime, function: usize, register: okkama_sparrow::Register) -> Result<MapValue, String> {
    match runtime.ty(function, register) {
        Some(Type::Map(key, value) | Type::MapBuilder(key, value)) => Ok(MapValue {
            key: runtime.kind_of(*key).repr(),
            value: runtime.kind_of(*value).repr(),
            entries: BTreeMap::new(),
        }),
        _ => Err("map instruction needs a map type".into()),
    }
}

fn key_text(runtime: &Runtime, word: Word, kind: Kind, ty: Option<&Type>) -> String {
    match (kind, ty) {
        (_, Some(Type::Enum(e))) => {
            let cases = &runtime.loaded.program.enums[e.0 as usize].cases;
            format!("\"{}\"", cases.get(word as u32 as usize).map_or("?", String::as_str))
        }
        (_, Some(Type::Str)) => format!("\"{}\"", runtime.heap.text(word)),
        (_, Some(Type::Char)) => format!("\"{}\"", char::from_u32(word as u32).unwrap_or('?')),
        (Kind::Float, _) => crate::scalar::float_text(f64::from_bits(word)),
        (Kind::Raw | Kind::Byte, _) => word.to_string(),
        _ => runtime.big_value(word).to_string(),
    }
}

/// Decimal text of an integer word (inline integers without a BigInt).
fn int_text(runtime: &mut Runtime, word: Word) -> Word {
    let text = match runtime.heap.integer(word) {
        crate::heap::Integer::Small(n) => n.to_string(),
        crate::heap::Integer::Big(n) => n.to_string(),
    };
    runtime.text_new(&text)
}

/// `cvt` of an int to a string.
#[inline(never)]
pub extern "C" fn rt_int_text(ctx: *mut Ctx, word: Word) -> Word {
    let frame = crate::runtime::frame_pointer!();
    let ctx = unsafe { &mut *ctx };
    let runtime = unsafe { &mut *ctx.runtime };
    let text = int_text(runtime, word);
    if runtime.heap.needs_collection() {
        runtime.safepoint(frame, ctx, &[(text, Repr::Ref)]);
    }
    text
}

fn convert(runtime: &mut Runtime, function: usize, source: okkama_sparrow::Register, dst: okkama_sparrow::Register, word: Word) -> Outcome {
    let from = runtime.ty(function, source).cloned();
    let to = runtime.ty(function, dst).cloned();
    match (from, to) {
        (Some(Type::Int), Some(Type::Str)) => Ok(int_text(runtime, word)),
        (Some(Type::Bool | Type::Byte), Some(Type::Str)) => Ok(runtime.text_new(&word.to_string())),
        (Some(Type::Float), Some(Type::Str)) => Ok(runtime.text_new(&crate::scalar::float_text(f64::from_bits(word)))),
        (Some(Type::Rat), Some(Type::Str)) => {
            let text = runtime.rational(word).to_string();
            Ok(runtime.text_new(&text))
        }
        (Some(Type::Int), Some(Type::Rat)) => {
            let n = runtime.big_value(word);
            Ok(runtime.rational_new(BigRational::from_integer(n)))
        }
        (Some(Type::Byte), Some(Type::Rat)) => Ok(runtime.rational_new(BigRational::from_integer(BigInt::from(word)))),
        (Some(Type::Float), Some(Type::Rat)) => Ok(runtime.rational_new(decimal_ratio(f64::from_bits(word)))),
        (Some(Type::Rat), Some(Type::Int)) => {
            let n = runtime.rational(word).to_integer();
            Ok(integer_word(runtime, n))
        }
        (Some(Type::Rat), Some(Type::Float)) => rational_to_float(runtime.rational(word)).map(f64::to_bits),
        (Some(Type::Byte), Some(Type::Int)) => Ok(runtime.heap.int(word as i64)),
        (Some(Type::Int), Some(Type::Float)) => {
            let n = runtime.big_value(word).to_f64().unwrap_or(f64::INFINITY);
            finite(n)
        }
        (Some(Type::Byte), Some(Type::Float)) => finite(word as f64),
        (Some(Type::Float), Some(Type::Int)) => {
            let n = f64::from_bits(word);
            let truncated = n.trunc();
            if !(-9223372036854775808.0..9223372036854775808.0).contains(&truncated) {
                return Err(format!(
                    "Cannot convert float value {} to int: expected signed-64 range",
                    crate::scalar::float_text(n)
                ));
            }
            Ok(runtime.heap.int(truncated as i64))
        }
        _ => Err("unsupported scalar conversion".into()),
    }
}

fn byte_from(runtime: &mut Runtime, kind: Kind, word: Word, context: Option<&str>) -> Outcome {
    let (name, text) = match kind {
        Kind::Float => {
            let n = f64::from_bits(word);
            if (0.0..=255.0).contains(&n.trunc()) {
                return Ok(n.trunc() as Word);
            }
            ("float", crate::scalar::float_text(n))
        }
        Kind::Int => {
            if let Some(byte) = runtime.small_or_big(word).and_then(|n| u8::try_from(n).ok()) {
                return Ok(Word::from(byte));
            }
            ("int", runtime.big_value(word).to_string())
        }
        _ => {
            let n = runtime.rational(word);
            if let Some(byte) = n.to_integer().to_u8() {
                return Ok(Word::from(byte));
            }
            ("rat", n.to_string())
        }
    };
    let message = format!("Cannot convert {name} value {text} to byte: expected 0..255");
    Err(match context {
        Some(c) => format!("{c}: {message}"),
        None => message,
    })
}
