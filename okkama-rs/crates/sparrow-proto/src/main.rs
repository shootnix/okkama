//! Architecture spike for the next Sparrow core: synchronous functions compiled
//! whole, direct machine calls, untagged registers (SPB types are static),
//! pointer objects with a header and inline bump allocation. Object registers
//! are declared for Cranelift stack maps, so calls pay the spills a precise GC
//! needs. Small integers only: an
//! overflow stops with a status instead of switching to BigInt.
//!
//! A semispace copying collector finds frame roots through those stack maps by
//! walking frame records (aarch64), so every reference in machine frames moves.
//!
//! Usage: sparrow-proto dis FILE.spb
//!        sparrow-proto run FILE.spb calls|objects|tree|async  (PROTO_SPACE_KB: semispace size)
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{AbiParam, InstBuilder, MemFlagsData, Signature, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, default_libcall_names};
use okkama_sparrow::{Cmp, Constant, Instruction as I, Program, Target, Type};
use std::collections::BTreeMap;

const OK: u64 = 0;
const TRAP: u64 = 1;
const OVERFLOW: u64 = 2;
const STACK: u64 = 3;
const DIVISION: u64 = 4;

// Async frames are heap objects: [header, state, result, waiter, resume, registers…].
// The frame of a call to an async function is also its Future.
const COLD: i64 = 0;
const SCHEDULED: i64 = 1;
const RUNNING: i64 = 2;
const DONE: i64 = 3;
const STATE: i32 = 8;
const RESULT: i32 = 16;
const WAITER: i32 = 24;
const RESUME: i32 = 32;
const REGISTERS: i32 = 40;
const FRAME_WORDS: usize = 5;

/// Shared with machine code; field offsets come from `offset_of!`. Machine code
/// touches only the leading words; the collector state follows.
#[repr(C)]
struct Ctx {
    cursor: u64,
    limit: u64,
    status: u64,
    stack_limit: u64,
    /// The harness's own reference, a root like any frame slot.
    root: u64,
    gc: Gc,
    /// Frames to start or resume; roots for the collector.
    ready: std::collections::VecDeque<u64>,
    /// Descriptor → body code of async functions (frames are their objects).
    bodies: Box<[u64]>,
}
impl std::fmt::Debug for Ctx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx").field("status", &self.status).finish_non_exhaustive()
    }
}

/// Semispace copying collector (Cheney). Objects are `[header, fields…]`;
/// a header is `descriptor << 1`, and a forwarded object's header is
/// `new address | 1`. Which fields hold references is known from static types.
#[derive(Debug, Default)]
struct Gc {
    current: Vec<u64>,
    other: Vec<u64>,
    /// Per descriptor: does each field hold an object reference?
    layouts: Vec<Vec<bool>>,
    /// Absolute return address of every safepoint → SP-relative reference slots.
    maps: Vec<(u64, Vec<u32>)>,
    code: Vec<(u64, u64)>,
    collections: u64,
    nanos: u128,
    copied_words: u64,
}
impl Gc {
    fn in_code(&self, address: u64) -> bool {
        self.code.iter().any(|&(start, end)| (start..end).contains(&address))
    }
    fn slots(&self, return_address: u64) -> &[u32] {
        match self.maps.binary_search_by_key(&return_address, |(a, _)| *a) {
            Ok(i) => &self.maps[i].1,
            Err(_) => &[],
        }
    }
}

/// Copy one object into to-space, or return its forwarding address.
///
/// SAFETY: `pointer` is 0 or a live object in from-space; `free` has room.
unsafe fn evacuate(layouts: &[Vec<bool>], pointer: u64, free: &mut u64) -> u64 {
    if pointer == 0 {
        return 0;
    }
    let header = unsafe { *(pointer as *const u64) };
    if header & 1 == 1 {
        return header & !1;
    }
    let words = layouts[(header >> 1) as usize].len() + 1;
    let target = *free;
    unsafe { std::ptr::copy_nonoverlapping(pointer as *const u64, target as *mut u64, words) };
    *free += words as u64 * 8;
    unsafe { *(pointer as *mut u64) = target | 1 };
    target
}

/// Copy everything reachable into the other space and swap; returns the
/// first free address. `frame` is the frame pointer of the runtime function
/// called from generated code.
unsafe fn evacuate_all(ctx: &mut Ctx, mut frame: u64) -> u64 {
    let gc = &mut ctx.gc;
    let to = gc.other.as_mut_ptr() as u64;
    let mut free = to;
    // Walk frame records: [fp] = caller's fp, [fp + 8] = return address into the
    // caller. The caller's SP at that call is fp + 16 (frame record at the top).
    loop {
        let (caller, ret) = unsafe { (*(frame as *const u64), *((frame + 8) as *const u64)) };
        if !gc.in_code(ret) {
            break;
        }
        let sp = frame + 16;
        for &offset in gc.slots(ret) {
            let slot = (sp + u64::from(offset)) as *mut u64;
            unsafe { *slot = evacuate(&gc.layouts, *slot, &mut free) };
        }
        frame = caller;
    }
    ctx.root = unsafe { evacuate(&gc.layouts, ctx.root, &mut free) };
    for frame in ctx.ready.iter_mut() {
        *frame = unsafe { evacuate(&gc.layouts, *frame, &mut free) };
    }
    let mut scan = to;
    while scan < free {
        let descriptor = (unsafe { *(scan as *const u64) } >> 1) as usize;
        let fields = gc.layouts[descriptor].len();
        for field in 0..fields {
            if gc.layouts[descriptor][field] {
                let slot = (scan + (field as u64 + 1) * 8) as *mut u64;
                unsafe { *slot = evacuate(&gc.layouts, *slot, &mut free) };
            }
        }
        scan += (fields as u64 + 1) * 8;
    }
    gc.copied_words += (free - to) / 8;
    gc.collections += 1;
    std::mem::swap(&mut gc.current, &mut gc.other);
    free
}

/// Collect, then allocate `words`. Called from generated code when the bump
/// region is full; `frame` is the frame pointer of that runtime function.
unsafe fn collect(ctx: &mut Ctx, frame: u64, words: u64) -> u64 {
    let started = std::time::Instant::now();
    let mut free = unsafe { evacuate_all(ctx, frame) };
    loop {
        let gc = &mut ctx.gc;
        let start = gc.current.as_ptr() as u64;
        let capacity = gc.current.len() as u64 * 8;
        let live = free - start;
        // Keep the live set under half of a space: the next to-space grows.
        let wanted = if live + words * 8 > capacity / 2 {
            gc.current.len() * 2
        } else {
            gc.current.len()
        };
        if gc.other.len() < wanted {
            gc.other = vec![0; wanted];
        }
        if free + words * 8 <= start + capacity {
            ctx.cursor = free + words * 8;
            ctx.limit = start + capacity;
            gc.nanos += started.elapsed().as_nanos();
            return free;
        }
        // No room even after collection: move once more, into the larger space.
        free = unsafe { evacuate_all(ctx, frame) };
    }
}

/// A completed frame resumes the frame awaiting it.
extern "C" fn wake(ctx: *mut Ctx, frame: u64) {
    // SAFETY: generated code passes its context and a live frame.
    unsafe { &mut *ctx }.ready.push_back(frame);
}

/// Run ready frames until `root` is done. Returns its result.
///
/// SAFETY: every queued frame is a live async frame with a compiled body.
unsafe fn run_until(ctx: &mut Ctx, root: u64) -> Option<i64> {
    loop {
        let state = |frame: u64| unsafe { *((frame + STATE as u64) as *const i64) };
        if state(ctx.root) == DONE {
            return Some(unsafe { *((ctx.root + RESULT as u64) as *const i64) });
        }
        let _ = root;
        let frame = ctx.ready.pop_front()?;
        match state(frame) {
            SCHEDULED | RUNNING => {
                unsafe { *((frame + STATE as u64) as *mut i64) = RUNNING };
                let descriptor = (unsafe { *(frame as *const u64) } >> 1) as usize;
                let body: extern "C" fn(*mut Ctx, u64) -> i64 =
                    unsafe { std::mem::transmute(ctx.bodies[descriptor]) };
                body(ctx, frame);
                if ctx.status != OK {
                    return None;
                }
            }
            _ => {}
        }
    }
}

/// Slow path of inline allocation.
#[inline(never)]
extern "C" fn refill(ctx: *mut Ctx, words: u64) -> u64 {
    let frame: u64;
    // SAFETY: reads this function's frame pointer (aarch64 frame record).
    unsafe { std::arch::asm!("mov {}, x29", out(reg) frame) };
    // SAFETY: called only by generated code with the context it was given,
    // while every frame above is a JIT frame with stack maps or the harness.
    unsafe { collect(&mut *ctx, frame, words) }
}

/// Registers and fields the collector must trace: objects and role values,
/// which are object pointers dispatched through the header.
fn is_reference(program: &Program, ty: okkama_sparrow::TypeRef) -> bool {
    matches!(
        program.ty(ty),
        Some(Type::Object(_) | Type::Successor(_) | Type::Role(_) | Type::Future(_))
    )
}

fn reachable(program: &Program, roots: &[usize]) -> Vec<usize> {
    let mut seen = std::collections::BTreeSet::new();
    let mut pending = roots.to_vec();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        for instruction in &program.functions[index].code {
            match instruction {
                I::Call { function, .. } => pending.push(function.0 as usize),
                I::CallRole { role, slot, .. } => {
                    for object in &program.objects {
                        for witness in object.does.iter().filter(|w| w.role == *role) {
                            pending.push(witness.slots[*slot as usize].0 as usize);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    seen.into_iter().collect()
}

struct Compiler<'a> {
    program: &'a Program,
    module: JITModule,
    ids: BTreeMap<usize, FuncId>,
    refill: FuncId,
    /// Per function: (return-address offset, SP-relative slots) and code size.
    stack_maps: Vec<(FuncId, Vec<(u32, Vec<u32>)>, u32)>,
    /// (role, slot) → code address per object descriptor, filled after finalize.
    dispatch: BTreeMap<(u32, u32), Box<[u64]>>,
    wake: FuncId,
    /// Descriptor → async body address, filled after finalize.
    bodies: Box<[u64]>,
}

impl Compiler<'_> {
    fn signature(&self, params: usize) -> Signature {
        let mut signature = self.module.make_signature();
        signature.params.push(AbiParam::new(types::I64));
        for _ in 0..params {
            signature.params.push(AbiParam::new(types::I64));
        }
        signature.returns.push(AbiParam::new(types::I64));
        signature
    }

    fn compile(&mut self, index: usize) -> Result<(), String> {
        let function = &self.program.functions[index];
        if function.native.is_some() || !function.handlers.is_empty() {
            return Err(format!("{}: outside the spike subset", function.symbol));
        }
        let config = self.module.target_config();
        let mut context = self.module.make_context();
        // An async body takes its frame; a synchronous function its parameters.
        let asynchronous = function.is_async;
        context.func.signature =
            self.signature(if asynchronous { 1 } else { function.params.len() });
        let mut frontend = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut context.func, &mut frontend);
        let memory = MemFlagsData::trusted();
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let ctx = builder.block_params(entry)[0];
        let vars: Vec<Variable> = function
            .registers
            .iter()
            .map(|ty| {
                let var = builder.declare_var(types::I64);
                if is_reference(self.program, *ty) {
                    builder.declare_var_needs_stack_map(var);
                }
                var
            })
            .collect();
        let frame_var = builder.declare_var(types::I64);
        if asynchronous {
            builder.declare_var_needs_stack_map(frame_var);
            let frame = builder.block_params(entry)[1];
            builder.def_var(frame_var, frame);
        } else {
            for (i, register) in function.params.iter().enumerate() {
                let value = builder.block_params(entry)[i + 1];
                builder.def_var(vars[register.0 as usize], value);
            }
        }
        // Stack guard: a real core would fall back to heap frames here.
        let sp = builder.ins().get_stack_pointer(types::I64);
        let limit = builder.ins().load(
            types::I64,
            memory,
            ctx,
            std::mem::offset_of!(Ctx, stack_limit) as i32,
        );
        let low = builder.ins().icmp(IntCC::UnsignedLessThan, sp, limit);
        let blocks: Vec<_> = function.code.iter().map(|_| builder.create_block()).collect();
        let fail = |builder: &mut FunctionBuilder<'_>, status: u64| {
            let block = builder.create_block();
            let current = builder.current_block();
            builder.switch_to_block(block);
            let code = builder.ins().iconst(types::I64, status as i64);
            builder.ins().store(memory, code, ctx, std::mem::offset_of!(Ctx, status) as i32);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);
            if let Some(current) = current {
                builder.switch_to_block(current);
            }
            block
        };
        let stack = fail(&mut builder, STACK);
        if asynchronous {
            // Resume point 0 starts with the parameters; pc + 1 re-enters the
            // await at pc with every register reloaded from the frame.
            let dispatch = builder.create_block();
            builder.ins().brif(low, stack, &[], dispatch, &[]);
            builder.switch_to_block(dispatch);
            let frame = builder.use_var(frame_var);
            let resume = builder.ins().load(types::I64, memory, frame, RESUME);
            let start = builder.create_block();
            let mut switch = cranelift_frontend::Switch::new();
            let mut reloads = Vec::new();
            for (pc, instruction) in function.code.iter().enumerate() {
                if matches!(instruction, I::Await { .. }) {
                    let block = builder.create_block();
                    switch.set_entry(pc as u128 + 1, block);
                    reloads.push((pc, block));
                }
            }
            switch.emit(&mut builder, resume, start);
            builder.switch_to_block(start);
            let frame = builder.use_var(frame_var);
            for register in &function.params {
                let value = builder.ins().load(
                    types::I64,
                    memory,
                    frame,
                    REGISTERS + register.0 as i32 * 8,
                );
                builder.def_var(vars[register.0 as usize], value);
            }
            builder.ins().jump(blocks[0], &[]);
            for (pc, block) in reloads {
                builder.switch_to_block(block);
                let frame = builder.use_var(frame_var);
                for (register, var) in vars.iter().enumerate() {
                    let value = builder.ins().load(
                        types::I64,
                        memory,
                        frame,
                        REGISTERS + register as i32 * 8,
                    );
                    builder.def_var(*var, value);
                }
                builder.ins().jump(blocks[pc], &[]);
            }
        } else {
            builder.ins().brif(low, stack, &[], blocks[0], &[]);
        }
        let jump = |builder: &mut FunctionBuilder<'_>, target: &Target| {
            let values: Vec<_> = target
                .moves
                .iter()
                .map(|(_, source)| builder.use_var(vars[source.0 as usize]))
                .collect();
            for ((dst, _), value) in target.moves.iter().zip(values) {
                builder.def_var(vars[dst.0 as usize], value);
            }
            builder.ins().jump(blocks[target.pc as usize], &[]);
        };
        for (pc, instruction) in function.code.iter().enumerate() {
            builder.switch_to_block(blocks[pc]);
            let next = blocks.get(pc + 1).copied();
            let var = |r: &okkama_sparrow::Register| vars[r.0 as usize];
            match instruction {
                I::Const { dst, value } => {
                    let bits = match value {
                        Constant::Int(text) => text
                            .parse::<i64>()
                            .map_err(|_| format!("{}: big constant", function.symbol))?,
                        Constant::Bool(b) => i64::from(*b),
                        _ => return Err(format!("{}:{pc}: constant kind", function.symbol)),
                    };
                    let value = builder.ins().iconst(types::I64, bits);
                    builder.def_var(var(&dst.0), value);
                }
                I::Move { dst, source } => {
                    let value = builder.use_var(var(source));
                    builder.def_var(var(&dst.0), value);
                }
                I::IntAdd { dst, left, right }
                | I::IntSub { dst, left, right }
                | I::IntMul { dst, left, right } => {
                    let (l, r) = (builder.use_var(var(left)), builder.use_var(var(right)));
                    let (value, overflow) = match instruction {
                        I::IntAdd { .. } => builder.ins().sadd_overflow(l, r),
                        I::IntSub { .. } => builder.ins().ssub_overflow(l, r),
                        _ => builder.ins().smul_overflow(l, r),
                    };
                    let failed = fail(&mut builder, OVERFLOW);
                    let commit = builder.create_block();
                    builder.ins().brif(overflow, failed, &[], commit, &[]);
                    builder.switch_to_block(commit);
                    builder.def_var(var(&dst.0), value);
                }
                I::IntMod { dst, left, right } => {
                    let (l, r) = (builder.use_var(var(left)), builder.use_var(var(right)));
                    let failed = fail(&mut builder, DIVISION);
                    let nonzero = builder.create_block();
                    builder.ins().brif(r, nonzero, &[], failed, &[]);
                    builder.switch_to_block(nonzero);
                    let minus_one = builder.ins().icmp_imm_s(IntCC::Equal, r, -1);
                    let safe = builder.ins().iconst(types::I64, 1);
                    let divisor = builder.ins().select(minus_one, safe, r);
                    let value = builder.ins().srem(l, divisor);
                    builder.def_var(var(&dst.0), value);
                }
                I::IntCmp { op, dst, left, right } => {
                    let (l, r) = (builder.use_var(var(left)), builder.use_var(var(right)));
                    let condition = match op {
                        Cmp::Eq => IntCC::Equal,
                        Cmp::Ne => IntCC::NotEqual,
                        Cmp::Lt => IntCC::SignedLessThan,
                        Cmp::Le => IntCC::SignedLessThanOrEqual,
                        Cmp::Gt => IntCC::SignedGreaterThan,
                        Cmp::Ge => IntCC::SignedGreaterThanOrEqual,
                    };
                    let flag = builder.ins().icmp(condition, l, r);
                    let value = builder.ins().uextend(types::I64, flag);
                    builder.def_var(var(&dst.0), value);
                }
                I::Branch {
                    condition,
                    then_target,
                    else_target,
                } => {
                    let condition = builder.use_var(var(condition));
                    let (yes, no) = (builder.create_block(), builder.create_block());
                    builder.ins().brif(condition, yes, &[], no, &[]);
                    builder.switch_to_block(yes);
                    jump(&mut builder, then_target);
                    builder.switch_to_block(no);
                    jump(&mut builder, else_target);
                    continue;
                }
                I::Jump { target } => {
                    jump(&mut builder, target);
                    continue;
                }
                I::Return { value } => {
                    let value = match value {
                        Some(r) => builder.use_var(var(r)),
                        None => builder.ins().iconst(types::I64, 0),
                    };
                    if asynchronous {
                        // Complete the Future and resume whoever awaits it.
                        let frame = builder.use_var(frame_var);
                        builder.ins().store(memory, value, frame, RESULT);
                        let done = builder.ins().iconst(types::I64, DONE);
                        builder.ins().store(memory, done, frame, STATE);
                        let waiter = builder.ins().load(types::I64, memory, frame, WAITER);
                        let notify = builder.create_block();
                        let finish = builder.create_block();
                        builder.ins().brif(waiter, notify, &[], finish, &[]);
                        builder.switch_to_block(notify);
                        let reference = self.module.declare_func_in_func(self.wake, builder.func);
                        builder.ins().call(reference, &[ctx, waiter]);
                        builder.ins().jump(finish, &[]);
                        builder.switch_to_block(finish);
                        let ready = builder.ins().iconst(types::I64, 0);
                        builder.ins().return_(&[ready]);
                    } else {
                        builder.ins().return_(&[value]);
                    }
                    continue;
                }
                I::FutSpawn { dst, future } => {
                    // Eager start, as in the language: the child runs on this
                    // stack until it completes or first suspends; a suspended
                    // child is resumed later by whatever it waits for.
                    let target = builder.use_var(var(future));
                    let state = builder.ins().load(types::I64, memory, target, STATE);
                    let start = builder.create_block();
                    let started = builder.create_block();
                    let is_cold = builder.ins().icmp_imm_s(IntCC::Equal, state, COLD);
                    builder.ins().brif(is_cold, start, &[], started, &[]);
                    builder.switch_to_block(start);
                    let running = builder.ins().iconst(types::I64, RUNNING);
                    builder.ins().store(memory, running, target, STATE);
                    let header = builder.ins().load(types::I64, memory, target, 0);
                    let offset = builder.ins().ishl_imm_u(header, 2);
                    let base = builder.ins().iconst(types::I64, self.bodies.as_ptr() as i64);
                    let entry = builder.ins().iadd(base, offset);
                    let code = builder.ins().load(types::I64, memory, entry, 0);
                    let signature = self.signature(1);
                    let signature = builder.import_signature(signature);
                    builder.ins().call_indirect(signature, code, &[ctx, target]);
                    let status = builder.ins().load(
                        types::I64,
                        memory,
                        ctx,
                        std::mem::offset_of!(Ctx, status) as i32,
                    );
                    let propagate = builder.create_block();
                    builder.ins().brif(status, propagate, &[], started, &[]);
                    builder.switch_to_block(propagate);
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[zero]);
                    builder.switch_to_block(started);
                    let frame = builder.use_var(var(future));
                    builder.def_var(var(&dst.0), frame);
                }
                I::Await { dst, future } if asynchronous => {
                    let target = builder.use_var(var(future));
                    let state = builder.ins().load(types::I64, memory, target, STATE);
                    let done = builder.create_block();
                    let pending = builder.create_block();
                    let start = builder.create_block();
                    let suspend = builder.create_block();
                    let is_done = builder.ins().icmp_imm_s(IntCC::Equal, state, DONE);
                    builder.ins().brif(is_done, done, &[], pending, &[]);
                    builder.switch_to_block(pending);
                    let is_cold = builder.ins().icmp_imm_s(IntCC::Equal, state, COLD);
                    builder.ins().brif(is_cold, start, &[], suspend, &[]);
                    // A cold Future runs here, on this native stack.
                    builder.switch_to_block(start);
                    let running = builder.ins().iconst(types::I64, RUNNING);
                    builder.ins().store(memory, running, target, STATE);
                    let header = builder.ins().load(types::I64, memory, target, 0);
                    let offset = builder.ins().ishl_imm_u(header, 2);
                    let base = builder.ins().iconst(types::I64, self.bodies.as_ptr() as i64);
                    let entry = builder.ins().iadd(base, offset);
                    let code = builder.ins().load(types::I64, memory, entry, 0);
                    let signature = self.signature(1);
                    let signature = builder.import_signature(signature);
                    let call = builder.ins().call_indirect(signature, code, &[ctx, target]);
                    let suspended = builder.inst_results(call)[0];
                    let status = builder.ins().load(
                        types::I64,
                        memory,
                        ctx,
                        std::mem::offset_of!(Ctx, status) as i32,
                    );
                    let checked = builder.create_block();
                    let propagate = builder.create_block();
                    builder.ins().brif(status, propagate, &[], checked, &[]);
                    builder.switch_to_block(propagate);
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[zero]);
                    builder.switch_to_block(checked);
                    builder.ins().brif(suspended, suspend, &[], done, &[]);
                    // Not ready: save every register, wait on the Future, re-enter
                    // this await when it completes.
                    builder.switch_to_block(suspend);
                    let frame = builder.use_var(frame_var);
                    let target = builder.use_var(var(future));
                    builder.ins().store(memory, frame, target, WAITER);
                    for (register, variable) in vars.iter().enumerate() {
                        let value = builder.use_var(*variable);
                        builder.ins().store(memory, value, frame, REGISTERS + register as i32 * 8);
                    }
                    let resume = builder.ins().iconst(types::I64, pc as i64 + 1);
                    builder.ins().store(memory, resume, frame, RESUME);
                    let pending_status = builder.ins().iconst(types::I64, 1);
                    builder.ins().return_(&[pending_status]);
                    builder.switch_to_block(done);
                    let target = builder.use_var(var(future));
                    let value = builder.ins().load(types::I64, memory, target, RESULT);
                    if let Some(dst) = dst {
                        builder.def_var(var(&dst.0), value);
                    }
                }
                I::Trap { .. } => {
                    let code = builder.ins().iconst(types::I64, TRAP as i64);
                    builder.ins().store(memory, code, ctx, std::mem::offset_of!(Ctx, status) as i32);
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[zero]);
                    continue;
                }
                I::Call { dst, function: callee, args }
                    if self.program.functions[callee.0 as usize].is_async =>
                {
                    let target = &self.program.functions[callee.0 as usize];
                    let registers = target.registers.len();
                    let descriptor = (self.program.objects.len() + callee.0 as usize) as i64;
                    let frame =
                        self.allocate(&mut builder, ctx, (FRAME_WORDS + registers) as i64);
                    let header = builder.ins().iconst(types::I64, descriptor << 1);
                    builder.ins().store(memory, header, frame, 0);
                    let zero = builder.ins().iconst(types::I64, 0);
                    for word in 1..FRAME_WORDS + registers {
                        builder.ins().store(memory, zero, frame, word as i32 * 8);
                    }
                    for (param, arg) in target.params.iter().zip(args) {
                        let value = builder.use_var(var(arg));
                        builder.ins().store(memory, value, frame, REGISTERS + param.0 as i32 * 8);
                    }
                    if let Some(dst) = dst {
                        builder.def_var(var(&dst.0), frame);
                    }
                }
                I::Call { dst, function: callee, args } => {
                    let id = self.ids[&(callee.0 as usize)];
                    let reference = self.module.declare_func_in_func(id, builder.func);
                    let mut values = vec![ctx];
                    values.extend(args.iter().map(|r| builder.use_var(var(r))));
                    let call = builder.ins().call(reference, &values);
                    let result = builder.inst_results(call)[0];
                    // A failure propagates as a status in the context.
                    let status = builder.ins().load(
                        types::I64,
                        memory,
                        ctx,
                        std::mem::offset_of!(Ctx, status) as i32,
                    );
                    let propagate = builder.create_block();
                    let commit = builder.create_block();
                    builder.ins().brif(status, propagate, &[], commit, &[]);
                    builder.switch_to_block(propagate);
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[zero]);
                    builder.switch_to_block(commit);
                    if let Some(dst) = dst {
                        builder.def_var(var(&dst.0), result);
                    }
                }
                I::RoleWrap { dst, source, .. } => {
                    // A role value is the object pointer; the header selects code.
                    let value = builder.use_var(var(source));
                    builder.def_var(var(&dst.0), value);
                }
                I::CallRole {
                    dst,
                    receiver,
                    role,
                    slot,
                    args,
                } => {
                    let objects = self.program.objects.len();
                    let table = self
                        .dispatch
                        .entry((role.0, *slot))
                        .or_insert_with(|| vec![0; objects].into_boxed_slice())
                        .as_ptr() as i64;
                    let object = builder.use_var(var(receiver));
                    let header = builder.ins().load(types::I64, memory, object, 0);
                    // header = descriptor << 1, so header * 4 = descriptor * 8.
                    let offset = builder.ins().ishl_imm_u(header, 2);
                    let base = builder.ins().iconst(types::I64, table);
                    let entry = builder.ins().iadd(base, offset);
                    let code = builder.ins().load(types::I64, memory, entry, 0);
                    let signature = self.signature(args.len() + 1);
                    let signature = builder.import_signature(signature);
                    let mut values = vec![ctx, object];
                    values.extend(args.iter().map(|r| builder.use_var(var(r))));
                    let call = builder.ins().call_indirect(signature, code, &values);
                    let result = builder.inst_results(call)[0];
                    let status = builder.ins().load(
                        types::I64,
                        memory,
                        ctx,
                        std::mem::offset_of!(Ctx, status) as i32,
                    );
                    let propagate = builder.create_block();
                    let commit = builder.create_block();
                    builder.ins().brif(status, propagate, &[], commit, &[]);
                    builder.switch_to_block(propagate);
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[zero]);
                    builder.switch_to_block(commit);
                    if let Some(dst) = dst {
                        builder.def_var(var(&dst.0), result);
                    }
                }
                I::ObjNew { dst, object, fields } => {
                    let words = fields.len() as i64 + 1;
                    let pointer = self.allocate(&mut builder, ctx, words);
                    let header = builder.ins().iconst(types::I64, i64::from(object.0) << 1);
                    builder.ins().store(memory, header, pointer, 0);
                    for (i, field) in fields.iter().enumerate() {
                        let value = builder.use_var(var(field));
                        builder.ins().store(memory, value, pointer, ((i + 1) * 8) as i32);
                    }
                    builder.def_var(var(&dst.0), pointer);
                }
                I::ObjGet { dst, object, field } => {
                    let pointer = builder.use_var(var(object));
                    let value =
                        builder
                            .ins()
                            .load(types::I64, memory, pointer, ((*field + 1) * 8) as i32);
                    builder.def_var(var(&dst.0), value);
                }
                I::ObjNext { dst, object, updates } => {
                    let declared = match self
                        .program
                        .ty(function.registers[object.0 as usize])
                    {
                        Some(Type::Object(o) | Type::Successor(o)) => o.0 as usize,
                        _ => return Err(format!("{}:{pc}: obj.next type", function.symbol)),
                    };
                    let count = self.program.objects[declared].fields.len();
                    let source = builder.use_var(var(object));
                    let pointer = self.allocate(&mut builder, ctx, count as i64 + 1);
                    for word in 0..=count {
                        let value = match updates.iter().find(|(f, _)| *f as usize + 1 == word) {
                            Some((_, r)) => builder.use_var(var(r)),
                            None => builder.ins().load(types::I64, memory, source, (word * 8) as i32),
                        };
                        builder.ins().store(memory, value, pointer, (word * 8) as i32);
                    }
                    builder.def_var(var(&dst.0), pointer);
                }
                other => {
                    return Err(format!(
                        "{}:{pc}: {} outside the spike subset",
                        function.symbol,
                        other.mnemonic()
                    ));
                }
            }
            match next {
                Some(next) => {
                    builder.ins().jump(next, &[]);
                }
                None => return Err(format!("{}: falls off the end", function.symbol)),
            }
        }
        builder.seal_all_blocks();
        builder.finalize(config);
        let id = self.ids[&index];
        self.module
            .define_function(id, &mut context)
            .map_err(|e| format!("{}: {e:?}", function.symbol))?;
        let compiled = context.compiled_code().expect("compiled function");
        let maps = compiled
            .buffer
            .user_stack_maps()
            .iter()
            .map(|(ret, _, map)| (*ret, map.entries().map(|(_, offset)| offset).collect()))
            .collect();
        self.stack_maps.push((id, maps, compiled.code_info().total_size));
        Ok(())
    }

    /// Inline bump allocation; the cold path takes a fresh chunk.
    fn allocate(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        ctx: cranelift_codegen::ir::Value,
        words: i64,
    ) -> cranelift_codegen::ir::Value {
        let memory = MemFlagsData::trusted();
        let cursor = builder.ins().load(types::I64, memory, ctx, std::mem::offset_of!(Ctx, cursor) as i32);
        let limit = builder.ins().load(types::I64, memory, ctx, std::mem::offset_of!(Ctx, limit) as i32);
        let next = builder.ins().iadd_imm_s(cursor, words * 8);
        let full = builder.ins().icmp(IntCC::UnsignedGreaterThan, next, limit);
        let fast = builder.create_block();
        let slow = builder.create_block();
        let done = builder.create_block();
        builder.append_block_param(done, types::I64);
        builder.ins().brif(full, slow, &[], fast, &[]);
        builder.switch_to_block(fast);
        builder.ins().store(memory, next, ctx, std::mem::offset_of!(Ctx, cursor) as i32);
        builder.ins().jump(done, &[cursor.into()]);
        builder.switch_to_block(slow);
        let reference = self.module.declare_func_in_func(self.refill, builder.func);
        let size = builder.ins().iconst(types::I64, words);
        let call = builder.ins().call(reference, &[ctx, size]);
        let fresh = builder.inst_results(call)[0];
        builder.ins().jump(done, &[fresh.into()]);
        builder.switch_to_block(done);
        builder.block_params(done)[0]
    }
}

fn find(program: &Program, symbol: &str) -> usize {
    program
        .functions
        .iter()
        .position(|f| f.symbol == symbol)
        .unwrap_or_else(|| panic!("missing function {symbol}"))
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&args[1]).expect("read SPB");
    let program = okkama_sparrow::decode(&bytes).expect("decode SPB");
    if args[0] == "dis" {
        print!("{}", okkama_sparrow::disassemble(&program));
        return 0.into();
    }
    let (roots, scenario): (Vec<usize>, &str) = match args[2].as_str() {
        "calls" => (vec![find(&program, "Fib.new#0"), find(&program, "Fib.fib")], "calls"),
        "objects" => (
            vec![find(&program, "World.new#0"), find(&program, "World.simulate")],
            "objects",
        ),
        "tree" => (
            vec![find(&program, "Builder.new#0"), find(&program, "Builder.run")],
            "tree",
        ),
        "async" => (
            vec![find(&program, "Worker.new#0"), find(&program, "Worker.run")],
            "async",
        ),
        other => panic!("unknown scenario {other}"),
    };
    let compile_started = std::time::Instant::now();
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").unwrap();
    flags.set("is_pic", "false").unwrap();
    flags.set("use_colocated_libcalls", "false").unwrap();
    // The collector walks frame records to find each frame's stack map.
    flags.set("preserve_frame_pointers", "true").unwrap();
    let isa = cranelift_native::builder()
        .unwrap()
        .finish(settings::Flags::new(flags))
        .unwrap();
    let mut jit = JITBuilder::with_isa(isa, default_libcall_names());
    jit.symbol("sparrow_proto_refill", refill as *const u8);
    jit.symbol("sparrow_proto_wake", wake as *const u8);
    let mut module = JITModule::new(jit);
    let mut refill_signature = module.make_signature();
    refill_signature.params.push(AbiParam::new(types::I64));
    refill_signature.params.push(AbiParam::new(types::I64));
    refill_signature.returns.push(AbiParam::new(types::I64));
    let refill_id = module
        .declare_function("sparrow_proto_refill", Linkage::Import, &refill_signature)
        .unwrap();
    let mut notify_signature = module.make_signature();
    notify_signature.params.push(AbiParam::new(types::I64));
    notify_signature.params.push(AbiParam::new(types::I64));
    let wake_id = module
        .declare_function("sparrow_proto_wake", Linkage::Import, &notify_signature)
        .unwrap();
    let functions = reachable(&program, &roots);
    let mut compiler = Compiler {
        program: &program,
        module,
        ids: BTreeMap::new(),
        refill: refill_id,
        stack_maps: Vec::new(),
        dispatch: BTreeMap::new(),
        wake: wake_id,
        bodies: vec![0; program.objects.len() + program.functions.len()].into_boxed_slice(),
    };
    for &index in &functions {
        let signature = compiler.signature(program.functions[index].params.len());
        let id = compiler
            .module
            .declare_function(&format!("f{index}"), Linkage::Local, &signature)
            .unwrap();
        compiler.ids.insert(index, id);
    }
    for &index in &functions {
        if let Err(error) = compiler.compile(index) {
            eprintln!("sparrow-proto: {error}");
            return 2.into();
        }
    }
    compiler.module.finalize_definitions().unwrap();
    let address = |index: usize| compiler.module.get_finalized_function(compiler.ids[&index]);
    let compile_time = compile_started.elapsed();
    for (&(role, slot), table) in &mut compiler.dispatch {
        for (descriptor, object) in program.objects.iter().enumerate() {
            if let Some(witness) = object.does.iter().find(|w| w.role.0 == role) {
                let function = witness.slots[slot as usize].0 as usize;
                table[descriptor] = compiler.module.get_finalized_function(compiler.ids[&function]) as u64;
            }
        }
    }
    let mut gc = Gc::default();
    for (id, maps, size) in &compiler.stack_maps {
        let start = compiler.module.get_finalized_function(*id) as u64;
        gc.code.push((start, start + u64::from(*size)));
        for (ret, slots) in maps {
            gc.maps.push((start + u64::from(*ret), slots.clone()));
        }
    }
    gc.maps.sort_by_key(|(address, _)| *address);
    gc.layouts = program
        .objects
        .iter()
        .map(|object| {
            object
                .fields
                .iter()
                .map(|(_, ty)| is_reference(&program, *ty))
                .collect()
        })
        .collect();
    // Async frames follow the objects: state, result, waiter, resume, registers.
    for function in &program.functions {
        let mut layout = Vec::new();
        if function.is_async {
            let result = function.result.is_some_and(|ty| is_reference(&program, ty));
            layout.extend([false, result, true, false]);
            layout.extend(function.registers.iter().map(|ty| is_reference(&program, *ty)));
        }
        gc.layouts.push(layout);
    }
    for (&index, &id) in &compiler.ids {
        if program.functions[index].is_async {
            compiler.bodies[program.objects.len() + index] =
                compiler.module.get_finalized_function(id) as u64;
        }
    }
    let space_words = std::env::var("PROTO_SPACE_KB")
        .ok()
        .and_then(|kb| kb.parse::<usize>().ok())
        .unwrap_or(4096)
        * 128;
    gc.current = vec![0; space_words];
    gc.other = vec![0; space_words];
    let marker = 0u8;
    let mut ctx = Ctx {
        cursor: gc.current.as_ptr() as u64,
        limit: gc.current.as_ptr() as u64 + space_words as u64 * 8,
        status: OK,
        // 8 MB main-thread stack: keep 1 MB of headroom.
        stack_limit: (&marker as *const u8 as u64).saturating_sub(7 << 20),
        root: 0,
        gc,
        ready: std::collections::VecDeque::new(),
        bodies: std::mem::take(&mut compiler.bodies),
    };
    let started = std::time::Instant::now();
    // SAFETY: the generated functions take the context plus their int/pointer
    // parameters in the platform C ABI and return one word.
    let result = unsafe {
        let new: extern "C" fn(*mut Ctx) -> i64 = std::mem::transmute(address(roots[0]));
        // The harness keeps its reference in the context root: a collection
        // during a call moves the object and updates the root.
        ctx.root = new(&mut ctx) as u64;
        match scenario {
            "calls" => {
                let fib: extern "C" fn(*mut Ctx, i64, i64) -> i64 =
                    std::mem::transmute(address(roots[1]));
                fib(&mut ctx, ctx.root as i64, 32)
            }
            "async" => {
                // The root Future: a cold frame of Worker.run(worker), then the
                // scheduler until it completes.
                let worker = ctx.root;
                let registers = program.functions[roots[1]].registers.len();
                let words = (FRAME_WORDS + registers) as u64;
                let frame = ctx.cursor;
                ctx.cursor += words * 8;
                assert!(ctx.cursor <= ctx.limit);
                std::ptr::write_bytes(frame as *mut u64, 0, words as usize);
                *(frame as *mut u64) = ((program.objects.len() + roots[1]) as u64) << 1;
                let receiver = program.functions[roots[1]].params[0].0 as u64;
                *((frame + REGISTERS as u64 + receiver * 8) as *mut u64) = worker;
                *((frame + STATE as u64) as *mut i64) = SCHEDULED;
                ctx.root = frame;
                ctx.ready.push_back(frame);
                run_until(&mut ctx, frame).unwrap_or(0)
            }
            "tree" => {
                let run: extern "C" fn(*mut Ctx, i64, i64, i64) -> i64 =
                    std::mem::transmute(address(roots[1]));
                run(&mut ctx, ctx.root as i64, 20, 0)
            }
            _ => {
                let simulate: extern "C" fn(*mut Ctx, i64, i64, i64) -> i64 =
                    std::mem::transmute(address(roots[1]));
                let mut total = 0i64;
                for id in 0..400 {
                    total += simulate(&mut ctx, ctx.root as i64, id, 2500);
                }
                total
            }
        }
    };
    let elapsed = started.elapsed();
    if ctx.status != OK {
        eprintln!("sparrow-proto: stopped with status {}", ctx.status);
        return 1.into();
    }
    println!("{result}");
    eprintln!(
        "compile {:.3} ms, run {:.3} ms, gc {} collections {:.3} ms, copied {} words, space {} KB, safepoints {}",
        compile_time.as_secs_f64() * 1e3,
        elapsed.as_secs_f64() * 1e3,
        ctx.gc.collections,
        ctx.gc.nanos as f64 / 1e6,
        ctx.gc.copied_words,
        ctx.gc.current.len() * 8 / 1024,
        ctx.gc.maps.len()
    );
    0.into()
}
