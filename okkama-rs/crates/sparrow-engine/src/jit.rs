//! The only unsafe boundary: owned executable memory and its exact C ABI.
#![allow(unsafe_code)]
use crate::execution::{ARITHMETIC, CALL, COLLECT, INVALID_STATE, RETURNED, RunState, YIELDED};
use crate::scalar::{FloatBinary, FloatUnary, Kind};
use crate::value::{Arithmetic, BOOL, SMALL, Slot};
use crate::{Error, Op};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::{
    ir::{AbiParam, Block, InstBuilder, MemFlagsData, types},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Switch, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, default_libcall_names};
use okkama_sparrow::Cmp;
use okkama_sparrow::Target;

pub(crate) const REGION_SIZE: usize = 64;

#[cfg(test)]
thread_local! {
    static FAIL_REGION: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn with_failed_region<T>(start: usize, body: impl FnOnce() -> T) -> T {
    struct Reset(Option<usize>);
    impl Drop for Reset {
        fn drop(&mut self) {
            FAIL_REGION.set(self.0);
        }
    }
    let _reset = Reset(FAIL_REGION.replace(Some(start)));
    body()
}

pub(crate) struct Compiled {
    // Keep executable memory alive for every call. The pointer never escapes.
    module: CodeMemory,
    entry: unsafe extern "C" fn(*mut RunState) -> u32,
}
impl std::fmt::Debug for Compiled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compiled").finish_non_exhaustive()
    }
}
impl Compiled {
    pub(crate) fn new(
        code: &crate::Code,
        plan: &[Op],
        kinds: &[Kind],
        live: &[Vec<usize>],
        region: std::ops::Range<usize>,
    ) -> Result<Self, Error> {
        #[cfg(test)]
        if FAIL_REGION.get() == Some(region.start) {
            return Err(Error("new engine JIT: injected compilation failure".into()));
        }
        let registers = kinds.len();
        let failure = |e: String| Error(format!("new engine JIT: {e}"));
        let mut flags = settings::builder();
        flags
            .set("opt_level", "speed")
            .map_err(|e| failure(e.to_string()))?;
        flags
            .set("use_colocated_libcalls", "false")
            .map_err(|e| failure(e.to_string()))?;
        flags
            .set("is_pic", "false")
            .map_err(|e| failure(e.to_string()))?;
        let isa = cranelift_native::builder()
            .map_err(|e| failure(e.into()))?
            .finish(settings::Flags::new(flags))
            .map_err(|e| failure(e.to_string()))?;
        let mut module = CodeMemory(Some(JITModule::new(JITBuilder::with_isa(
            isa,
            default_libcall_names(),
        ))));
        let pointer = module.target_config().pointer_type();
        let mut context = module.make_context();
        context.func.signature.params.push(AbiParam::new(pointer));
        context
            .func
            .signature
            .returns
            .push(AbiParam::new(types::I32));
        let id = module
            .declare_function(
                "sparrow_next_entry",
                Linkage::Local,
                &context.func.signature,
            )
            .map_err(|e| failure(e.to_string()))?;
        // Each slot displacement is encoded as signed i32 by Cranelift.
        if registers > (i32::MAX as usize / 16) {
            return Err(failure("frame exceeds native displacement range".into()));
        }
        let mut heap_signature = module.make_signature();
        heap_signature.params.push(AbiParam::new(pointer));
        heap_signature.params.push(AbiParam::new(pointer));
        heap_signature.returns.push(AbiParam::new(types::I32));
        let mut frontend = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut context.func, &mut frontend);
            let heap_signature = builder.import_signature(heap_signature);
            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            let state = builder.block_params(entry)[0];
            let memory = MemFlagsData::trusted();
            let slots = builder.ins().load(
                pointer,
                memory,
                state,
                std::mem::offset_of!(RunState, slots) as i32,
            );
            let vars: Vec<_> = kinds
                .iter()
                .map(|kind| {
                    builder.declare_var(if *kind == Kind::Float {
                        types::F64
                    } else {
                        types::I64
                    })
                })
                .collect();
            let tags: Vec<_> = (0..registers)
                .map(|_| builder.declare_var(types::I64))
                .collect();
            let fuel_var = builder.declare_var(types::I64);
            let fuel = builder.ins().load(
                types::I64,
                memory,
                state,
                std::mem::offset_of!(RunState, fuel) as i32,
            );
            builder.def_var(fuel_var, fuel);
            let pc = builder.ins().load(
                types::I32,
                memory,
                state,
                std::mem::offset_of!(RunState, pc) as i32,
            );
            let invalid = builder.create_block();
            let mut targets: std::collections::BTreeSet<usize> = region.clone().collect();
            if region.end < plan.len() {
                targets.insert(region.end);
            }
            for op in &plan[region.clone()] {
                match op {
                    Op::Jump(target) => {
                        targets.insert(target.pc as usize);
                    }
                    Op::Branch(_, yes, no) => {
                        targets.extend([yes.pc as usize, no.pc as usize]);
                    }
                    _ => {}
                }
            }
            let mut blocks = vec![invalid; plan.len()];
            for &pc in &targets {
                blocks[pc] = builder.create_block();
            }
            // Entering at a PC loads only the values live there (on a call, the
            // parameters); every exit stored exactly those. Registers dead at
            // the entry PC are redefined before any use.
            let entries: Vec<_> = region.clone().map(|_| builder.create_block()).collect();
            let mut dispatch = Switch::new();
            for (pc, entry) in region.clone().zip(&entries) {
                dispatch.set_entry(pc as u128, *entry);
            }
            dispatch.emit(&mut builder, pc, invalid);
            builder.switch_to_block(invalid);
            let status = builder.ins().iconst(types::I32, i64::from(INVALID_STATE));
            builder.ins().return_(&[status]);
            for (pc, entry) in region.clone().zip(entries) {
                builder.switch_to_block(entry);
                for &i in &live[pc] {
                    let value = builder.ins().load(
                        if kinds[i] == Kind::Float {
                            types::F64
                        } else {
                            types::I64
                        },
                        memory,
                        slots,
                        (i * 16) as i32,
                    );
                    builder.def_var(vars[i], value);
                    let tag = builder
                        .ins()
                        .load(types::I64, memory, slots, (i * 16 + 8) as i32);
                    builder.def_var(tags[i], tag);
                }
                builder.ins().jump(blocks[pc], &[]);
            }
            for pc in region.clone() {
                let op = &plan[pc];
                builder.switch_to_block(blocks[pc]);
                let fuel = builder.use_var(fuel_var);
                let body = builder.create_block();
                let yielded = builder.create_block();
                builder.ins().brif(fuel, body, &[], yielded, &[]);
                builder.switch_to_block(yielded);
                for &i in &live[pc] {
                    let value = builder.use_var(vars[i]);
                    builder.ins().store(memory, value, slots, (i * 16) as i32);
                    let tag = builder.use_var(tags[i]);
                    builder.ins().store(memory, tag, slots, (i * 16 + 8) as i32);
                }
                let next = builder.ins().iconst(types::I32, pc as i64);
                builder.ins().store(
                    memory,
                    next,
                    state,
                    std::mem::offset_of!(RunState, pc) as i32,
                );
                let status = builder.ins().iconst(types::I32, i64::from(YIELDED));
                builder.ins().store(
                    memory,
                    fuel,
                    state,
                    std::mem::offset_of!(RunState, fuel) as i32,
                );
                builder.ins().return_(&[status]);
                builder.switch_to_block(body);
                let remaining = builder.ins().iadd_imm_s(fuel, -1);
                builder.def_var(fuel_var, remaining);
                for register in op.required_bound().into_iter().flatten() {
                    let tag = builder.use_var(tags[register]);
                    let absent =
                        builder
                            .ins()
                            .icmp_imm_s(IntCC::Equal, tag, crate::value::UNBOUND as i64);
                    let bound = builder.create_block();
                    let unbound = builder.create_block();
                    builder.ins().brif(absent, unbound, &[], bound, &[]);
                    builder.switch_to_block(unbound);
                    for &i in &live[pc] {
                        let value = builder.use_var(vars[i]);
                        let tag = builder.use_var(tags[i]);
                        builder.ins().store(memory, value, slots, (i * 16) as i32);
                        builder.ins().store(memory, tag, slots, (i * 16 + 8) as i32);
                    }
                    let at = builder.ins().iconst(types::I32, pc as i64);
                    builder.ins().store(
                        memory,
                        at,
                        state,
                        std::mem::offset_of!(RunState, pc) as i32,
                    );
                    builder.ins().store(
                        memory,
                        remaining,
                        state,
                        std::mem::offset_of!(RunState, fuel) as i32,
                    );
                    let status = builder
                        .ins()
                        .iconst(types::I32, i64::from(crate::execution::UNBOUND_VALUE));
                    builder.ins().return_(&[status]);
                    builder.switch_to_block(bound);
                }
                match op {
                    Op::Capture(pairs) => {
                        for (dst, src) in pairs {
                            let value = builder.use_var(vars[*src]);
                            let tag = builder.use_var(tags[*src]);
                            builder.def_var(vars[*dst], value);
                            builder.def_var(tags[*dst], tag);
                        }
                    }
                    Op::Const(dst, slot) => {
                        let value = if kinds[*dst] == Kind::Float {
                            builder.ins().f64const(f64::from_bits(slot.bits))
                        } else {
                            builder.ins().iconst(types::I64, slot.bits as i64)
                        };
                        let tag = builder.ins().iconst(types::I64, slot.tag as i64);
                        builder.def_var(vars[*dst], value);
                        builder.def_var(tags[*dst], tag);
                    }
                    Op::Copy(dst, source) => {
                        let value = builder.use_var(vars[*source]);
                        builder.def_var(vars[*dst], value);
                        let tag = builder.use_var(tags[*source]);
                        builder.def_var(tags[*dst], tag);
                    }
                    Op::Not(dst, source) => {
                        let value = builder.use_var(vars[*source]);
                        let value = builder.ins().bxor_imm_u(value, 1);
                        builder.def_var(vars[*dst], value);
                        let tag = builder.ins().iconst(types::I64, BOOL as i64);
                        builder.def_var(tags[*dst], tag);
                    }
                    Op::Jump(target) => {
                        jump(&mut builder, &vars, &tags, &blocks, target);
                        continue;
                    }
                    Op::Branch(condition, yes, no) => {
                        let condition = builder.use_var(vars[*condition]);
                        let yes_block = builder.create_block();
                        let no_block = builder.create_block();
                        builder.ins().brif(condition, yes_block, &[], no_block, &[]);
                        builder.switch_to_block(yes_block);
                        jump(&mut builder, &vars, &tags, &blocks, yes);
                        builder.switch_to_block(no_block);
                        jump(&mut builder, &vars, &tags, &blocks, no);
                        continue;
                    }
                    Op::Call { .. }
                    | Op::CallCap { .. }
                    | Op::CallRole { .. }
                    | Op::Role(..)
                    | Op::AnyBox(..)
                    | Op::FailureLink(..)
                    | Op::RegionNew(..)
                    | Op::RegionRegister(..)
                    | Op::RegionTransfer(..)
                    | Op::RegionClose { .. }
                    | Op::Await { .. }
                    | Op::FutureControl(..)
                    | Op::WorkerSpawn { .. }
                    | Op::WorkerBind { .. }
                    | Op::FutureSpawn(..)
                    | Op::Intrinsic(..)
                    | Op::Equal(..)
                    | Op::CallFn { .. }
                    | Op::Closure { .. }
                    | Op::Throw(..)
                    | Op::Raise(..)
                    | Op::Catch { .. }
                    | Op::Lookup { .. }
                    | Op::Arithmetic(..)
                    | Op::Compare(..)
                    | Op::Unary(..)
                    | Op::FloatBinary(..)
                    | Op::FloatUnary(..)
                    | Op::Convert(..)
                    | Op::ByteFrom(..)
                    | Op::ByteBinary(..)
                    | Op::ByteNot(..)
                    | Op::Bits(..)
                    | Op::RatNew(..)
                    | Op::RatBinary(..)
                    | Op::RatUnary(..)
                    | Op::ObjectNew(..)
                    | Op::ObjectGet(..)
                    | Op::ObjectNext(..)
                    | Op::ObjectInit(..)
                    | Op::Text(..)
                    | Op::RatPart(..)
                    | Op::ArrayNew(..)
                    | Op::Array(..)
                    | Op::RangeNew(..)
                    | Op::RangePart(..)
                    | Op::BuilderRange(..)
                    | Op::MapNew(..)
                    | Op::Map(..)
                    | Op::Trap(..)
                    | Op::Panic(..) => {
                        let slow = builder.create_block();
                        // Heap instructions, and the general cases of natively
                        // specialised ones, run in Rust without leaving machine code.
                        let call = heap_call(code, op).then(|| builder.create_block());
                        let fallback = call.unwrap_or(slow);
                        match op {
                            Op::FloatBinary(
                                kind @ (FloatBinary::Add
                                | FloatBinary::Subtract
                                | FloatBinary::Multiply
                                | FloatBinary::Divide),
                                dst,
                                left,
                                right,
                            ) => {
                                let left = builder.use_var(vars[*left]);
                                let right = builder.use_var(vars[*right]);
                                let value = match kind {
                                    FloatBinary::Add => builder.ins().fadd(left, right),
                                    FloatBinary::Subtract => builder.ins().fsub(left, right),
                                    FloatBinary::Multiply => builder.ins().fmul(left, right),
                                    FloatBinary::Divide => builder.ins().fdiv(left, right),
                                    _ => unreachable!(),
                                };
                                let magnitude = builder.ins().fabs(value);
                                let maximum = builder.ins().f64const(f64::MAX);
                                let finite = builder.ins().fcmp(
                                    FloatCC::LessThanOrEqual,
                                    magnitude,
                                    maximum,
                                );
                                let commit = builder.create_block();
                                builder.ins().brif(finite, commit, &[], fallback, &[]);
                                builder.switch_to_block(commit);
                                builder.def_var(vars[*dst], value);
                                let tag =
                                    builder.ins().iconst(types::I64, crate::value::FLOAT as i64);
                                builder.def_var(tags[*dst], tag);
                                builder.ins().jump(blocks[pc + 1], &[]);
                            }
                            Op::FloatUnary(
                                kind @ (FloatUnary::Negate | FloatUnary::Absolute),
                                dst,
                                source,
                            ) => {
                                let source = builder.use_var(vars[*source]);
                                let value = match kind {
                                    FloatUnary::Negate => builder.ins().fneg(source),
                                    FloatUnary::Absolute => builder.ins().fabs(source),
                                    _ => unreachable!(),
                                };
                                builder.def_var(vars[*dst], value);
                                let tag =
                                    builder.ins().iconst(types::I64, crate::value::FLOAT as i64);
                                builder.def_var(tags[*dst], tag);
                                builder.ins().jump(blocks[pc + 1], &[]);
                            }
                            Op::Compare(cmp, dst, left, right) if kinds[*left] == Kind::Float => {
                                let left = builder.use_var(vars[*left]);
                                let right = builder.use_var(vars[*right]);
                                let condition = match cmp {
                                    Cmp::Eq => FloatCC::Equal,
                                    Cmp::Ne => FloatCC::NotEqual,
                                    Cmp::Lt => FloatCC::LessThan,
                                    Cmp::Le => FloatCC::LessThanOrEqual,
                                    Cmp::Gt => FloatCC::GreaterThan,
                                    Cmp::Ge => FloatCC::GreaterThanOrEqual,
                                };
                                let value = builder.ins().fcmp(condition, left, right);
                                let value = builder.ins().uextend(types::I64, value);
                                builder.def_var(vars[*dst], value);
                                let tag =
                                    builder.ins().iconst(types::I64, crate::value::BOOL as i64);
                                builder.def_var(tags[*dst], tag);
                                builder.ins().jump(blocks[pc + 1], &[]);
                            }
                            Op::Arithmetic(
                                kind @ (Arithmetic::Add
                                | Arithmetic::Subtract
                                | Arithmetic::Multiply),
                                dst,
                                left,
                                right,
                            ) => {
                                let left_tag = builder.use_var(tags[*left]);
                                let right_tag = builder.use_var(tags[*right]);
                                let non_small = builder.ins().bor(left_tag, right_tag);
                                let fast = builder.create_block();
                                builder.ins().brif(non_small, fallback, &[], fast, &[]);
                                builder.switch_to_block(fast);
                                let left = builder.use_var(vars[*left]);
                                let right = builder.use_var(vars[*right]);
                                let (value, overflow) = match kind {
                                    Arithmetic::Add => builder.ins().sadd_overflow(left, right),
                                    Arithmetic::Subtract => {
                                        builder.ins().ssub_overflow(left, right)
                                    }
                                    Arithmetic::Multiply => {
                                        builder.ins().smul_overflow(left, right)
                                    }
                                    _ => unreachable!(),
                                };
                                let commit = builder.create_block();
                                builder.ins().brif(overflow, fallback, &[], commit, &[]);
                                builder.switch_to_block(commit);
                                builder.def_var(vars[*dst], value);
                                let tag = builder.ins().iconst(types::I64, SMALL as i64);
                                builder.def_var(tags[*dst], tag);
                                builder.ins().jump(blocks[pc + 1], &[]);
                            }
                            Op::Arithmetic(Arithmetic::Remainder, dst, left, right) => {
                                let left_tag = builder.use_var(tags[*left]);
                                let right_tag = builder.use_var(tags[*right]);
                                let non_small = builder.ins().bor(left_tag, right_tag);
                                let fast = builder.create_block();
                                builder.ins().brif(non_small, fallback, &[], fast, &[]);
                                builder.switch_to_block(fast);
                                let left = builder.use_var(vars[*left]);
                                let right = builder.use_var(vars[*right]);
                                // Divisors 0 (typed failure) and -1 (MIN % -1) stay
                                // on the service path: right + 1 <= 1 unsigned.
                                let shifted = builder.ins().iadd_imm_s(right, 1);
                                let special = builder.ins().icmp_imm_u(
                                    IntCC::UnsignedLessThanOrEqual,
                                    shifted,
                                    1,
                                );
                                let commit = builder.create_block();
                                builder.ins().brif(special, fallback, &[], commit, &[]);
                                builder.switch_to_block(commit);
                                let value = builder.ins().srem(left, right);
                                builder.def_var(vars[*dst], value);
                                let tag = builder.ins().iconst(types::I64, SMALL as i64);
                                builder.def_var(tags[*dst], tag);
                                builder.ins().jump(blocks[pc + 1], &[]);
                            }
                            Op::Compare(cmp, dst, left, right) if kinds[*left] != Kind::Float => {
                                let left_tag = builder.use_var(tags[*left]);
                                let right_tag = builder.use_var(tags[*right]);
                                let non_small = builder.ins().bor(left_tag, right_tag);
                                let fast = builder.create_block();
                                builder.ins().brif(non_small, fallback, &[], fast, &[]);
                                builder.switch_to_block(fast);
                                let left = builder.use_var(vars[*left]);
                                let right = builder.use_var(vars[*right]);
                                let condition = match cmp {
                                    Cmp::Eq => IntCC::Equal,
                                    Cmp::Ne => IntCC::NotEqual,
                                    Cmp::Lt => IntCC::SignedLessThan,
                                    Cmp::Le => IntCC::SignedLessThanOrEqual,
                                    Cmp::Gt => IntCC::SignedGreaterThan,
                                    Cmp::Ge => IntCC::SignedGreaterThanOrEqual,
                                };
                                let value = builder.ins().icmp(condition, left, right);
                                let value = builder.ins().uextend(types::I64, value);
                                builder.def_var(vars[*dst], value);
                                let tag = builder.ins().iconst(types::I64, BOOL as i64);
                                builder.def_var(tags[*dst], tag);
                                builder.ins().jump(blocks[pc + 1], &[]);
                            }
                            _ => {
                                builder.ins().jump(fallback, &[]);
                            }
                        }
                        if let Some(block) = call {
                            builder.switch_to_block(block);
                            for &i in &live[pc] {
                                let value = builder.use_var(vars[i]);
                                builder.ins().store(memory, value, slots, (i * 16) as i32);
                                let tag = builder.use_var(tags[i]);
                                builder.ins().store(memory, tag, slots, (i * 16 + 8) as i32);
                            }
                            // The plan owns this compiled region (see heap_service).
                            let instruction = builder.ins().iconst(pointer, op as *const Op as i64);
                            let callee = builder.ins().iconst(
                                pointer,
                                heap_service as extern "C" fn(*mut RunState, *const Op) -> u32
                                    as usize as i64,
                            );
                            let call = builder.ins().call_indirect(
                                heap_signature,
                                callee,
                                &[state, instruction],
                            );
                            let status = builder.inst_results(call)[0];
                            let failed =
                                builder
                                    .ins()
                                    .icmp_imm_u(IntCC::Equal, status, i64::from(HEAP_FAILED));
                            let done = builder.create_block();
                            let raise = builder.create_block();
                            builder.ins().brif(failed, raise, &[], done, &[]);
                            // Live values are already in their slots: exit to the
                            // service path, which raises the deferred failure.
                            builder.switch_to_block(raise);
                            let at = builder.ins().iconst(types::I32, pc as i64);
                            builder.ins().store(
                                memory,
                                at,
                                state,
                                std::mem::offset_of!(RunState, pc) as i32,
                            );
                            builder.ins().store(
                                memory,
                                remaining,
                                state,
                                std::mem::offset_of!(RunState, fuel) as i32,
                            );
                            let exit = builder.ins().iconst(types::I32, i64::from(ARITHMETIC));
                            builder.ins().return_(&[exit]);
                            // Slots are the home of every value across the call:
                            // reload what the successor needs instead of keeping
                            // machine registers live (and spilled) around it.
                            builder.switch_to_block(done);
                            for &i in live.get(pc + 1).into_iter().flatten() {
                                let value = builder.ins().load(
                                    if kinds[i] == Kind::Float {
                                        types::F64
                                    } else {
                                        types::I64
                                    },
                                    memory,
                                    slots,
                                    (i * 16) as i32,
                                );
                                builder.def_var(vars[i], value);
                                let tag =
                                    builder
                                        .ins()
                                        .load(types::I64, memory, slots, (i * 16 + 8) as i32);
                                builder.def_var(tags[i], tag);
                            }
                            let collect = builder.create_block();
                            let next = blocks.get(pc + 1).copied().unwrap_or(invalid);
                            builder.ins().brif(status, collect, &[], next, &[]);
                            builder.switch_to_block(collect);
                            let resume = builder.ins().iconst(types::I32, (pc + 1) as i64);
                            builder.ins().store(
                                memory,
                                resume,
                                state,
                                std::mem::offset_of!(RunState, pc) as i32,
                            );
                            builder.ins().store(
                                memory,
                                remaining,
                                state,
                                std::mem::offset_of!(RunState, fuel) as i32,
                            );
                            let status = builder.ins().iconst(types::I32, i64::from(COLLECT));
                            builder.ins().return_(&[status]);
                        }
                        if call.is_some() {
                            continue;
                        }
                        builder.switch_to_block(slow);
                        for &i in &live[pc] {
                            let value = builder.use_var(vars[i]);
                            builder.ins().store(memory, value, slots, (i * 16) as i32);
                            let tag = builder.use_var(tags[i]);
                            builder.ins().store(memory, tag, slots, (i * 16 + 8) as i32);
                        }
                        let next = builder.ins().iconst(types::I32, pc as i64);
                        builder.ins().store(
                            memory,
                            next,
                            state,
                            std::mem::offset_of!(RunState, pc) as i32,
                        );
                        builder.ins().store(
                            memory,
                            remaining,
                            state,
                            std::mem::offset_of!(RunState, fuel) as i32,
                        );
                        let status = builder.ins().iconst(
                            types::I32,
                            i64::from(
                                if matches!(
                                    op,
                                    Op::Call { .. }
                                        | Op::CallFn { .. }
                                        | Op::CallRole { .. }
                                        | Op::CallCap { .. }
                                ) {
                                    CALL
                                } else if matches!(op, Op::Await { .. }) {
                                    crate::execution::AWAIT
                                } else if matches!(op, Op::RegionClose { .. }) {
                                    crate::execution::CLOSE
                                } else if matches!(op, Op::Throw(..) | Op::Raise(..)) {
                                    crate::execution::RAISE
                                } else if matches!(op, Op::Lookup { .. } | Op::Catch { .. }) {
                                    crate::execution::TRANSFER
                                } else {
                                    ARITHMETIC
                                },
                            ),
                        );
                        builder.ins().return_(&[status]);
                        continue;
                    }
                    Op::Unreachable => {
                        let status = builder
                            .ins()
                            .iconst(types::I32, i64::from(crate::execution::UNREACHABLE));
                        builder.ins().return_(&[status]);
                        continue;
                    }
                    Op::Return(_) | Op::ReturnVoid => {
                        let (value, tag) = if let Op::Return(source) = op {
                            (
                                builder.use_var(vars[*source]),
                                builder.use_var(tags[*source]),
                            )
                        } else {
                            (
                                builder.ins().iconst(types::I64, 0),
                                builder
                                    .ins()
                                    .iconst(types::I64, crate::value::UNBOUND as i64),
                            )
                        };
                        builder.ins().store(
                            memory,
                            value,
                            state,
                            std::mem::offset_of!(RunState, result) as i32,
                        );
                        builder.ins().store(
                            memory,
                            tag,
                            state,
                            (std::mem::offset_of!(RunState, result) + 8) as i32,
                        );
                        let status = builder.ins().iconst(types::I32, i64::from(RETURNED));
                        builder.ins().store(
                            memory,
                            remaining,
                            state,
                            std::mem::offset_of!(RunState, fuel) as i32,
                        );
                        builder.ins().return_(&[status]);
                        continue;
                    }
                }
                builder
                    .ins()
                    .jump(blocks.get(pc + 1).copied().unwrap_or(invalid), &[]);
            }
            // Cross-region edges preserve only destination-live slots. They do
            // not consume an instruction or a second fuel unit; dispatch resumes
            // at the exact target PC, including parallel edge assignments.
            for &pc in targets.iter().filter(|pc| !region.contains(pc)) {
                builder.switch_to_block(blocks[pc]);
                for &i in &live[pc] {
                    let value = builder.use_var(vars[i]);
                    let tag = builder.use_var(tags[i]);
                    builder.ins().store(memory, value, slots, (i * 16) as i32);
                    builder.ins().store(memory, tag, slots, (i * 16 + 8) as i32);
                }
                let next = builder.ins().iconst(types::I32, pc as i64);
                let fuel = builder.use_var(fuel_var);
                builder.ins().store(
                    memory,
                    next,
                    state,
                    std::mem::offset_of!(RunState, pc) as i32,
                );
                builder.ins().store(
                    memory,
                    fuel,
                    state,
                    std::mem::offset_of!(RunState, fuel) as i32,
                );
                let status = builder
                    .ins()
                    .iconst(types::I32, crate::execution::REGION_TRANSFER as i64);
                builder.ins().return_(&[status]);
            }
            builder.seal_all_blocks();
            builder.finalize(module.target_config());
        }
        module
            .define_function(id, &mut context)
            .map_err(|e| failure(e.to_string()))?;
        module
            .finalize_definitions()
            .map_err(|e| failure(e.to_string()))?;
        let address = module.get_finalized_function(id);
        // SAFETY: the generated C ABI accepts exactly RunState* and returns u32.
        // CodeMemory keeps the code alive and the private pointer cannot escape.
        let entry = unsafe {
            std::mem::transmute::<*const u8, unsafe extern "C" fn(*mut RunState) -> u32>(address)
        };
        Ok(Self { module, entry })
    }
    pub(crate) fn run(&self, state: &mut RunState) -> u32 {
        let _keep_alive = &self.module;
        // SAFETY: only Session constructs RunState, with storage sized for this
        // executable. It remains alive and exclusively borrowed for this call.
        unsafe { (self.entry)(state) }
    }
}

const HEAP_DONE: u32 = 0;
const HEAP_COLLECT: u32 = 1;
const HEAP_FAILED: u32 = 2;
const NATIVE_ARGUMENTS: usize = 16;

/// Instructions machine code hands to `heap_service`: those executed by
/// `execution::heap_op`, exactly as on the service path. Only the exit, dispatch
/// and re-entry around them are saved.
fn heap_call(code: &crate::Code, op: &Op) -> bool {
    if let Op::Call { function, args, .. } = op {
        // Natives run in Rust either way; only the dispatcher round trip is saved.
        return code.function(*function).native.is_some() && args.len() <= NATIVE_ARGUMENTS;
    }
    matches!(
        op,
        Op::Equal(..)
            | Op::ObjectNew(..)
            | Op::ObjectNext(..)
            | Op::ObjectGet(..)
            | Op::ObjectInit(..)
            | Op::RangeNew(..)
            | Op::RangePart(..)
            | Op::BuilderRange(..)
            | Op::MapNew(..)
            | Op::Map(..)
            | Op::ArrayNew(..)
            | Op::Array(..)
            | Op::Text(..)
            | Op::RatPart(..)
            | Op::RatNew(..)
            | Op::RatBinary(..)
            | Op::RatUnary(..)
            | Op::ByteBinary(..)
            | Op::ByteNot(..)
            | Op::Bits(..)
            | Op::FloatBinary(..)
            | Op::FloatUnary(..)
            | Op::Convert(..)
            | Op::ByteFrom(..)
            | Op::Unary(..)
            | Op::Arithmetic(..)
            | Op::Compare(..)
            | Op::Closure { .. }
            | Op::AnyBox(..)
    )
}

/// Executes one heap instruction for machine code. A failure is left in
/// `Heap::deferred_failure` and the code exits to the service path at the same
/// PC, which raises it. An internal panic cannot unwind through machine code
/// and aborts the process (`catch_unwind` here cost 40% of object-heavy runs).
extern "C" fn heap_service(state: *mut RunState, op: *const Op) -> u32 {
    // SAFETY: only code compiled above calls this, during Compiled::run. `state`
    // is the RunState exclusively borrowed by that run; its slot window and heap
    // belong to the resuming task, which does not use them until the run returns.
    // `op` points into the immutable plan that owns the compiled region.
    let (state, op) = unsafe { (&mut *state, &*op) };
    let heap = unsafe { &mut *state.heap };
    let slots = unsafe { std::slice::from_raw_parts_mut(state.slots, state.slot_count) };
    let result = if let Op::Call {
        dst,
        function,
        ref args,
        ..
    } = *op
    {
        // SAFETY: the program outlives every run of its compiled regions.
        let code = unsafe { &*state.code };
        let native = code.function(function).native.expect("native call");
        let mut arguments = [Slot::default(); NATIVE_ARGUMENTS];
        for (argument, register) in arguments.iter_mut().zip(args) {
            *argument = slots[*register];
        }
        match heap.native_invoke(native, &arguments[..args.len()]) {
            Ok(crate::native::Outcome::Value(value)) => {
                if let Some(dst) = dst {
                    slots[dst] = value;
                }
                Ok(())
            }
            Ok(crate::native::Outcome::Failure(failure)) => {
                heap.deferred_raise = Some(failure);
                return HEAP_FAILED;
            }
            Err(error) => Err(error),
        }
    } else {
        crate::execution::heap_op(op, slots, heap)
    };
    let heap = unsafe { &mut *state.heap };
    crate::stats::record(|s| *s.heap_calls.entry(crate::stats::op_name(op)).or_default() += 1);
    match result {
        Ok(()) if heap.needs_collection() => HEAP_COLLECT,
        Ok(()) => HEAP_DONE,
        Err(error) => {
            heap.deferred_failure = Some(error);
            HEAP_FAILED
        }
    }
}

fn jump(
    builder: &mut FunctionBuilder<'_>,
    vars: &[Variable],
    tags: &[Variable],
    blocks: &[Block],
    target: &Target,
) {
    // Read both words of every source before defining any destination.
    let values: Vec<_> = target
        .moves
        .iter()
        .map(|(_, source)| {
            (
                builder.use_var(vars[source.0 as usize]),
                builder.use_var(tags[source.0 as usize]),
            )
        })
        .collect();
    for ((dst, _), (value, tag)) in target.moves.iter().zip(values) {
        builder.def_var(vars[dst.0 as usize], value);
        builder.def_var(tags[dst.0 as usize], tag);
    }
    builder.ins().jump(blocks[target.pc as usize], &[]);
}

// Cranelift deliberately does not free executable memory on ordinary module
// drop. Own it from construction onward, including compilation error paths.
struct CodeMemory(Option<JITModule>);
impl std::ops::Deref for CodeMemory {
    type Target = JITModule;
    fn deref(&self) -> &JITModule {
        self.0.as_ref().expect("live code owner")
    }
}
impl std::ops::DerefMut for CodeMemory {
    fn deref_mut(&mut self) -> &mut JITModule {
        self.0.as_mut().expect("live code owner")
    }
}
impl Drop for CodeMemory {
    fn drop(&mut self) {
        if let Some(module) = self.0.take() {
            // SAFETY: entry pointer is private, calls borrow self, and no generated
            // function/closure can outlive the sole owner of executable memory.
            unsafe {
                module.free_memory();
            }
        }
    }
}
