//! Whole-function compilation to machine code (Cranelift), lazily on first call.
//!
//! Every function takes the context and its parameters as words and returns
//! one word (floats travel as bits). Calls go through the code table: a zero
//! entry compiles the callee first. Integer and reference registers are
//! declared for stack maps, so every call is a precise safepoint; the
//! collector walks frame records (frame pointers are preserved) and reads the
//! spilled slots of each frame from the map of its return address.
#![allow(unsafe_code)]

use crate::Error;
use crate::heap::{self, Heap};
use crate::intrinsics::Intrinsic;
use crate::program::Kind;
use crate::runtime::{self, Ctx, FAILED, OK, Runtime};
use crate::word::{self, Repr, Word};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{AbiParam, Block, InstBuilder, MemFlagsData, Signature, StackSlotData, StackSlotKind, Value, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, default_libcall_names};
use okkama_sparrow::{Cmp, Constant, Instruction as I, Target, Type};
use std::collections::BTreeMap;

macro_rules! helpers {
    ($($field:ident = $function:path, $params:expr, $returns:expr;)*) => {
        #[derive(Debug)]
        struct Helpers { $($field: FuncId,)* }
        fn declare_helpers(builder: &mut JITBuilder) {
            $(builder.symbol(concat!("sparrow_core_", stringify!($field)), $function as *const u8);)*
        }
        fn import_helpers(module: &mut JITModule) -> Result<Helpers, Error> {
            Ok(Helpers { $($field: {
                let mut signature = module.make_signature();
                for _ in 0..$params {
                    signature.params.push(AbiParam::new(types::I64));
                }
                if $returns {
                    signature.returns.push(AbiParam::new(types::I64));
                }
                module
                    .declare_function(concat!("sparrow_core_", stringify!($field)), Linkage::Import, &signature)
                    .map_err(|e| Error(format!("JIT helper: {e}")))?
            },)* })
        }
    };
}

helpers! {
    allocate = runtime::rt_allocate, 2, true;
    compile = runtime::rt_compile, 2, true;
    int = runtime::rt_int, 4, true;
    int_order = runtime::rt_int_order, 3, true;
    concat = runtime::rt_concat, 3, true;
    array = runtime::rt_array, 4, true;
    trap = runtime::rt_trap, 2, true;
    throw = runtime::rt_throw, 2, true;
    trace = runtime::rt_trace, 4, false;
    native = runtime::rt_native, 4, true;
    float = runtime::rt_float, 4, true;
    exec = crate::ops::rt_exec, 4, true;
    closure = runtime::rt_closure, 4, true;
    call_closure = runtime::rt_call_closure, 4, true;
    map_find = runtime::rt_map_find, 5, true;
    intrinsic = crate::intrinsics::rt_intrinsic, 4, true;
    failure_op = crate::failures::rt_failure_op, 4, true;
    catch = crate::failures::rt_catch, 4, true;
    created = crate::failures::rt_created, 3, false;
    region_add = crate::failures::rt_region_add, 4, false;
    builder_push = crate::ops::rt_builder_push, 3, true;
    int_text = crate::ops::rt_int_text, 2, true;
    future_new = crate::tasks::rt_future_new, 4, true;
    await_ = crate::tasks::rt_await, 3, true;
    complete = crate::tasks::rt_complete, 3, false;
    spawn = crate::tasks::rt_spawn, 2, true;
    fut = crate::tasks::rt_fut, 3, true;
}

pub struct Jit {
    module: JITModule,
    helpers: Helpers,
    ids: BTreeMap<usize, FuncId>,
    /// Return address of every safepoint → SP-relative slots of traced values.
    maps: Vec<(u64, Vec<u32>)>,
    /// Address ranges of compiled functions, sorted by start.
    ranges: Vec<(u64, u64)>,
    /// (role, slot) → function index by object descriptor (u64::MAX: none).
    dispatch: BTreeMap<(u32, u32), Box<[u64]>>,
}

impl std::fmt::Debug for Jit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jit")
            .field("functions", &self.ids.len())
            .field("safepoints", &self.maps.len())
            .finish_non_exhaustive()
    }
}

impl Jit {
    pub fn new() -> Result<Self, Error> {
        let mut flags = settings::builder();
        let set = |flags: &mut settings::Builder, name: &str, value: &str| {
            flags.set(name, value).map_err(|e| Error(format!("JIT flag {name}: {e}")))
        };
        set(&mut flags, "opt_level", "speed")?;
        set(&mut flags, "is_pic", "false")?;
        set(&mut flags, "use_colocated_libcalls", "false")?;
        // The collector walks frame records to find each frame's stack map.
        set(&mut flags, "preserve_frame_pointers", "true")?;
        let isa = cranelift_native::builder()
            .map_err(|e| Error(format!("JIT target: {e}")))?
            .finish(settings::Flags::new(flags))
            .map_err(|e| Error(format!("JIT target: {e}")))?;
        let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
        builder.memory_provider(Box::new(crate::code_memory::CodeMemory::new()));
        declare_helpers(&mut builder);
        let mut module = JITModule::new(builder);
        let helpers = import_helpers(&mut module)?;
        Ok(Self {
            module,
            helpers,
            ids: BTreeMap::new(),
            maps: Vec::new(),
            ranges: Vec::new(),
            dispatch: BTreeMap::new(),
        })
    }

    fn in_code(&self, address: u64) -> bool {
        let index = self.ranges.partition_point(|&(start, _)| start <= address);
        index > 0 && address < self.ranges[index - 1].1
    }

    /// Report the traced values of every machine frame above `frame`, the
    /// frame pointer of a runtime function called from machine code.
    ///
    /// SAFETY: frame records from `frame` up to `entry` are live frames; those
    /// returning into code compiled here have stack maps.
    pub(crate) unsafe fn frame_roots(&self, mut frame: u64, entry: u64, visit: &mut dyn FnMut(Word, Repr)) {
        // Rust frames between machine frames (a helper that calls a closure)
        // are passed over; the walk ends at the frame that entered machine code.
        while frame != 0 && frame != entry {
            let (caller, ret) = unsafe { (*(frame as *const u64), *((frame + 8) as *const u64)) };
            if !self.in_code(ret) {
                frame = caller;
                continue;
            }
            // The caller's SP at the call is just above the callee's frame record.
            let sp = frame + 16;
            if let Ok(index) = self.maps.binary_search_by_key(&ret, |(address, _)| *address) {
                for &offset in &self.maps[index].1 {
                    // Integer slots and pointers share one rule: a pointer is
                    // non-zero with a clear low bit.
                    visit(unsafe { *((sp + u64::from(offset)) as *const u64) }, Repr::Int);
                }
            }
            frame = caller;
        }
    }

    fn signature(&self, params: usize) -> Signature {
        let mut signature = self.module.make_signature();
        for _ in 0..=params {
            signature.params.push(AbiParam::new(types::I64));
        }
        signature.returns.push(AbiParam::new(types::I64));
        signature
    }
}

/// Call compiled code with the context and `words` as parameters.
///
/// SAFETY: `code` is a function compiled here taking exactly `words.len()`
/// parameters after the context.
pub(crate) unsafe fn invoke(code: u64, ctx: *mut Ctx, words: &[Word]) -> Word {
    type W = Word;
    let c = ctx;
    let w = words;
    unsafe {
        match w.len() {
            0 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx) -> W>(code)(c),
            1 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W) -> W>(code)(c, w[0]),
            2 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W) -> W>(code)(c, w[0], w[1]),
            3 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W) -> W>(code)(c, w[0], w[1], w[2]),
            4 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3]),
            5 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3], w[4]),
            6 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3], w[4], w[5]),
            7 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3], w[4], w[5], w[6]),
            8 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]),
            _ => {
                let runtime = &mut *(*c).runtime;
                crate::runtime::fatal(&mut *c, runtime, "closure with more than 8 parameters".into())
            }
        }
    }
}

/// Compile `function` unless it is compiled; returns its code address.
pub fn compile(runtime: &mut Runtime, function: usize) -> Result<u64, Error> {
    if runtime.code[function] != 0 {
        return Ok(runtime.code[function]);
    }
    let Runtime { heap, loaded, jit, code, constants, resources, .. } = runtime;
    let program = &loaded.program;
    let source = &program.functions[function];
    if source.is_async && (source.native.is_some() || source.intrinsic.is_some()) {
        return Err(Error(format!("{}: async natives and intrinsics are driven, not compiled", source.symbol)));
    }
    // An async body takes its frame (see `tasks`).
    let params = if source.is_async { 1 } else { source.params.len() };
    let id = match jit.ids.get(&function) {
        Some(id) => *id,
        None => {
            let signature = jit.signature(params);
            let id = jit
                .module
                .declare_function(&format!("f{function}"), Linkage::Local, &signature)
                .map_err(|e| Error(format!("JIT declare: {e}")))?;
            jit.ids.insert(function, id);
            id
        }
    };
    let config = jit.module.target_config();
    let mut context = jit.module.make_context();
    context.func.signature = jit.signature(params);
    let mut frontend = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut context.func, &mut frontend);
        let mut emit = Emitter {
            builder: &mut builder,
            heap,
            loaded,
            jit,
            code_table: code.as_ptr() as i64,
            constants,
            resources,
            function,
            kinds: &loaded.functions[function].kinds,
            vars: Vec::new(),
            blocks: Vec::new(),
            ctx: None,
            frame: None,
        };
        emit.body()?;
        builder.seal_all_blocks();
        builder.finalize(config);
    }
    jit.module
        .define_function(id, &mut context)
        .map_err(|e| Error(format!("{}: {e:?}", source.symbol)))?;
    let compiled = context.compiled_code().expect("compiled function");
    let maps: Vec<(u32, Vec<u32>)> = compiled
        .buffer
        .user_stack_maps()
        .iter()
        .map(|(ret, _, map)| (*ret, map.entries().map(|(_, offset)| offset).collect()))
        .collect();
    let size = compiled.code_info().total_size;
    jit.module
        .finalize_definitions()
        .map_err(|e| Error(format!("JIT finalize: {e}")))?;
    let address = jit.module.get_finalized_function(id) as u64;
    for (ret, slots) in maps {
        jit.maps.push((address + u64::from(ret), slots));
    }
    jit.maps.sort_by_key(|(address, _)| *address);
    jit.ranges.push((address, address + u64::from(size)));
    jit.ranges.sort_unstable();
    code[function] = address;
    Ok(address)
}

struct Emitter<'a, 'b> {
    builder: &'a mut FunctionBuilder<'b>,
    heap: &'a mut Heap,
    loaded: &'a crate::program::Loaded,
    jit: &'a mut Jit,
    code_table: i64,
    constants: &'a mut Vec<(Word, Repr)>,
    resources: &'a crate::failures::Resources,
    function: usize,
    kinds: &'a [Kind],
    vars: Vec<Variable>,
    blocks: Vec<Block>,
    ctx: Option<Value>,
    /// The frame variable of an async body.
    frame: Option<Variable>,
}

const MEMORY: MemFlagsData = MemFlagsData::trusted();

impl Emitter<'_, '_> {
    fn ctx(&self) -> Value {
        self.ctx.expect("context")
    }

    fn helper(&mut self, id: fn(&Helpers) -> FuncId, args: &[Value]) -> Option<Value> {
        let id = id(&self.jit.helpers);
        let reference = self.jit.module.declare_func_in_func(id, self.builder.func);
        let call = self.builder.ins().call(reference, args);
        self.builder.inst_results(call).first().copied()
    }

    fn iconst(&mut self, value: i64) -> Value {
        self.builder.ins().iconst(types::I64, value)
    }

    fn get(&mut self, register: &okkama_sparrow::Register) -> Value {
        self.builder.use_var(self.vars[register.0 as usize])
    }

    fn set(&mut self, register: &okkama_sparrow::Register, value: Value) {
        self.builder.def_var(self.vars[register.0 as usize], value);
    }

    /// A register as a word (floats as bits).
    fn word(&mut self, register: &okkama_sparrow::Register) -> Value {
        let value = self.get(register);
        if self.kinds[register.0 as usize] == Kind::Float {
            self.builder.ins().bitcast(types::I64, MemFlagsData::new(), value)
        } else {
            value
        }
    }

    /// Store a word into a register of its kind.
    fn set_word(&mut self, register: &okkama_sparrow::Register, value: Value) {
        let value = if self.kinds[register.0 as usize] == Kind::Float {
            self.builder.ins().bitcast(types::F64, MemFlagsData::new(), value)
        } else {
            value
        };
        self.set(register, value);
    }

    fn status_offset() -> i32 {
        std::mem::offset_of!(Ctx, status) as i32
    }

    /// After anything that can fail: on a pending failure go to the handler
    /// covering `pc`, otherwise record this activation and return.
    fn check(&mut self, pc: usize) {
        let ctx = self.ctx();
        let status = self.builder.ins().load(types::I64, MEMORY, ctx, Self::status_offset());
        let failed = self.builder.create_block();
        let next = self.builder.create_block();
        self.builder.ins().brif(status, failed, &[], next, &[]);
        self.builder.switch_to_block(failed);
        let is_failure = self.builder.ins().icmp_imm_s(IntCC::Equal, status, FAILED as i64);
        let recorded = self.builder.create_block();
        let fatal = self.builder.create_block();
        self.builder.ins().brif(is_failure, recorded, &[], fatal, &[]);
        self.builder.switch_to_block(fatal);
        let zero = self.iconst(0);
        self.builder.ins().return_(&[zero]);
        self.builder.switch_to_block(recorded);
        let function = self.iconst(self.function as i64);
        let at = self.iconst(pc as i64);
        let frame = self.builder.ins().get_frame_pointer(types::I64);
        self.helper(|h| h.trace, &[ctx, function, at, frame]);
        let handler = self.loaded.program.functions[self.function]
            .handlers
            .iter()
            .find(|h| (h.start as usize..h.end as usize).contains(&pc))
            .cloned();
        match handler {
            Some(handler) => {
                let failure = self.builder.ins().load(
                    types::I64,
                    MEMORY,
                    ctx,
                    std::mem::offset_of!(Ctx, failure) as i32,
                );
                let ok = self.iconst(OK as i64);
                self.builder.ins().store(MEMORY, ok, ctx, Self::status_offset());
                // Edge moves read the old registers; the failure is written last.
                self.jump_then(&handler.target, handler.failure, failure);
            }
            None => {
                let zero = self.iconst(0);
                self.builder.ins().return_(&[zero]);
            }
        }
        self.builder.switch_to_block(next);
    }

    fn jump(&mut self, target: &Target) {
        let values: Vec<_> = target.moves.iter().map(|(_, source)| self.get(source)).collect();
        for ((dst, _), value) in target.moves.iter().zip(values) {
            self.set(dst, value);
        }
        self.builder.ins().jump(self.blocks[target.pc as usize], &[]);
    }

    /// Inline allocation of `words` words from the free list of their class.
    fn allocate(&mut self, words: usize) -> Value {
        let ctx = self.ctx();
        let Some(class) = Heap::class_of(words) else {
            let size = self.iconst(words as i64);
            return self.helper(|h| h.allocate, &[ctx, size]).expect("result");
        };
        let head = self.heap.free_head(class) as i64;
        let head = self.iconst(head);
        let cell = self.builder.ins().load(types::I64, MEMORY, head, 0);
        let fast = self.builder.create_block();
        let slow = self.builder.create_block();
        let done = self.builder.create_block();
        self.builder.append_block_param(done, types::I64);
        self.builder.ins().brif(cell, fast, &[], slow, &[]);
        self.builder.switch_to_block(fast);
        let next = self.builder.ins().load(types::I64, MEMORY, cell, 8);
        self.builder.ins().store(MEMORY, next, head, 0);
        self.builder.ins().jump(done, &[cell.into()]);
        self.builder.switch_to_block(slow);
        let size = self.iconst(words as i64);
        let fresh = self.helper(|h| h.allocate, &[ctx, size]).expect("result");
        self.builder.ins().jump(done, &[fresh.into()]);
        self.builder.switch_to_block(done);
        self.builder.block_params(done)[0]
    }

    fn constant_text(&mut self, text: &str) -> Value {
        let word = self.heap.allocate_text(text);
        self.constants.push((word, Repr::Ref));
        self.iconst(word as i64)
    }

    /// Call `function` through the code table, compiling it first if needed.
    fn call(&mut self, pc: usize, function: Value, args: &[Value]) -> Value {
        let ctx = self.ctx();
        let base = self.iconst(self.code_table);
        let offset = self.builder.ins().ishl_imm_u(function, 3);
        let entry = self.builder.ins().iadd(base, offset);
        let code = self.builder.ins().load(types::I64, MEMORY, entry, 0);
        let ready = self.builder.create_block();
        let compile = self.builder.create_block();
        self.builder.append_block_param(ready, types::I64);
        self.builder.ins().brif(code, ready, &[code.into()], compile, &[]);
        self.builder.switch_to_block(compile);
        let compiled = self.helper(|h| h.compile, &[ctx, function]).expect("result");
        self.check(pc);
        self.builder.ins().jump(ready, &[compiled.into()]);
        self.builder.switch_to_block(ready);
        let code = self.builder.block_params(ready)[0];
        let signature = self.jit.signature(args.len());
        let signature = self.builder.import_signature(signature);
        let mut values = vec![ctx];
        values.extend_from_slice(args);
        let call = self.builder.ins().call_indirect(signature, code, &values);
        let result = self.builder.inst_results(call)[0];
        self.check(pc);
        result
    }

    /// Words of `registers` in a fresh stack slot; returns its address.
    fn spill(&mut self, registers: &[okkama_sparrow::Register]) -> Value {
        let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            (registers.len().max(1) * 8) as u32,
            3,
        ));
        for (i, register) in registers.iter().enumerate() {
            let value = self.word(register);
            self.builder.ins().stack_store(types::I64, value, slot, (i * 8) as i32);
        }
        self.builder.ins().stack_addr(types::I64, slot, 0)
    }

    fn register_type(&self, register: &okkama_sparrow::Register) -> Option<&Type> {
        let source = &self.loaded.program.functions[self.function];
        self.loaded.program.ty(source.registers[register.0 as usize])
    }

    fn body(&mut self) -> Result<(), Error> {
        let program = &self.loaded.program;
        let source = &program.functions[self.function];
        let entry = self.builder.create_block();
        self.builder.append_block_params_for_function_params(entry);
        self.builder.switch_to_block(entry);
        self.ctx = Some(self.builder.block_params(entry)[0]);
        for kind in self.kinds {
            let var = self.builder.declare_var(if *kind == Kind::Float { types::F64 } else { types::I64 });
            if kind.traced() {
                self.builder.declare_var_needs_stack_map(var);
            }
            self.vars.push(var);
        }
        // Registers start unbound; the verifier proves defined use.
        for (register, kind) in self.kinds.iter().enumerate() {
            let value = if *kind == Kind::Float {
                self.builder.ins().f64const(f64::from_bits(word::UNBOUND_RAW))
            } else {
                self.iconst(0)
            };
            self.builder.def_var(self.vars[register], value);
        }
        if source.native.is_some() || source.intrinsic.is_some() {
            // A trampoline, for calls through the code table (roles, closures).
            let params = self.builder.block_params(entry)[1..].to_vec();
            let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                (params.len().max(1) * 8) as u32,
                3,
            ));
            for (i, value) in params.iter().enumerate() {
                self.builder.ins().stack_store(types::I64, *value, slot, (i * 8) as i32);
            }
            let address = self.builder.ins().stack_addr(types::I64, slot, 0);
            let ctx = self.ctx();
            let index = self.iconst(self.function as i64);
            let count = self.iconst(params.len() as i64);
            let result = if source.native.is_some() {
                self.helper(|h| h.native, &[ctx, index, address, count])
            } else {
                self.helper(|h| h.intrinsic, &[ctx, index, address, count])
            }
            .expect("result");
            self.builder.ins().return_(&[result]);
            return Ok(());
        }
        if source.is_async {
            let frame = self.builder.declare_var(types::I64);
            self.builder.declare_var_needs_stack_map(frame);
            let value = self.builder.block_params(entry)[1];
            self.builder.def_var(frame, value);
            self.frame = Some(frame);
        } else {
            for (i, register) in source.params.iter().enumerate() {
                let value = self.builder.block_params(entry)[i + 1];
                self.set_word(register, value);
            }
        }
        self.blocks = source.code.iter().map(|_| self.builder.create_block()).collect();
        // R2: stack exhaustion is a typed failure.
        let ctx = self.ctx();
        let sp = self.builder.ins().get_stack_pointer(types::I64);
        let limit = self.builder.ins().load(
            types::I64,
            MEMORY,
            ctx,
            std::mem::offset_of!(Ctx, stack_limit) as i32,
        );
        let low = self.builder.ins().icmp(IntCC::UnsignedLessThan, sp, limit);
        let overflow = self.builder.create_block();
        let start = if source.is_async { self.builder.create_block() } else { self.blocks[0] };
        self.builder.ins().brif(low, overflow, &[], start, &[]);
        if source.is_async {
            self.resume_dispatch(start);
        }
        self.builder.switch_to_block(overflow);
        let message = self.constant_text("Stack overflow");
        self.helper(|h| h.trap, &[ctx, message]);
        let zero = self.iconst(0);
        self.builder.ins().return_(&[zero]);
        for pc in 0..source.code.len() {
            self.builder.switch_to_block(self.blocks[pc]);
            if self.instruction(pc)? {
                continue;
            }
            match self.blocks.get(pc + 1).copied() {
                Some(next) => {
                    self.builder.ins().jump(next, &[]);
                }
                None => return Err(Error(format!("{}: falls off the end", source.symbol))),
            }
        }
        Ok(())
    }

    /// Entry of an async body: start with the parameters from the frame, or
    /// re-enter a suspended `await` with every register reloaded.
    fn resume_dispatch(&mut self, dispatch: Block) {
        let source = &self.loaded.program.functions[self.function];
        self.builder.switch_to_block(dispatch);
        let frame = self.builder.use_var(self.frame.expect("frame"));
        let resume = self.builder.ins().load(types::I64, MEMORY, frame, crate::tasks::RESUME as i32);
        let start = self.builder.create_block();
        let mut switch = cranelift_frontend::Switch::new();
        let mut reloads = Vec::new();
        for (pc, instruction) in source.code.iter().enumerate() {
            if matches!(instruction, I::Await { .. }) {
                let block = self.builder.create_block();
                switch.set_entry(pc as u128 + 1, block);
                reloads.push((pc, block));
            }
        }
        switch.emit(self.builder, resume, start);
        self.builder.switch_to_block(start);
        for register in &source.params {
            let value = self.frame_register(register.0 as usize);
            self.set_word(register, value);
        }
        self.builder.ins().jump(self.blocks[0], &[]);
        for (pc, block) in reloads {
            self.builder.switch_to_block(block);
            for register in 0..self.kinds.len() {
                let value = self.frame_register(register);
                self.set_word(&okkama_sparrow::Register(register as u32), value);
            }
            self.builder.ins().jump(self.blocks[pc], &[]);
        }
    }

    fn frame_register(&mut self, register: usize) -> Value {
        let frame = self.builder.use_var(self.frame.expect("frame"));
        let offset = crate::tasks::REGISTERS as i32 + register as i32 * 8;
        self.builder.ins().load(types::I64, MEMORY, frame, offset)
    }

    /// Emit one instruction; true when it ends its block.
    fn instruction(&mut self, pc: usize) -> Result<bool, Error> {
        let program = &self.loaded.program;
        let source = &program.functions[self.function];
        let instruction = &source.code[pc];
        let ctx = self.ctx();
        let unsupported = || {
            Error(format!(
                "{}:{pc}: {} is not supported by sparrow-core yet",
                source.symbol,
                instruction.mnemonic()
            ))
        };
        match instruction {
            I::Const { dst, value } => {
                let value = match value {
                    Constant::Int(text) => {
                        let n: num_bigint::BigInt =
                            text.parse().map_err(|_| Error("invalid integer constant".into()))?;
                        let word = self.heap.big(n);
                        if word & 1 == 0 {
                            self.constants.push((word, Repr::Int));
                        }
                        self.iconst(word as i64)
                    }
                    Constant::Bool(b) => self.iconst(i64::from(*b)),
                    Constant::Byte(b) => self.iconst(i64::from(*b)),
                    Constant::Char(c) => self.iconst(i64::from(u32::from(*c))),
                    Constant::Float(bits) => self.builder.ins().f64const(f64::from_bits(*bits)),
                    Constant::Str(text) => self.constant_text(text),
                };
                self.set(&dst.0, value);
            }
            I::Move { dst, source } => {
                let value = self.get(source);
                self.set(&dst.0, value);
            }
            I::IntAdd { dst, left, right }
            | I::IntSub { dst, left, right }
            | I::IntMul { dst, left, right }
            | I::IntMod { dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let both = self.builder.ins().band(l, r);
                let small = self.builder.ins().band_imm_u(both, 1);
                let fast = self.builder.create_block();
                let slow = self.builder.create_block();
                let done = self.builder.create_block();
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(small, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                let op = match instruction {
                    I::IntAdd { .. } => runtime::INT_ADD,
                    I::IntSub { .. } => runtime::INT_SUB,
                    I::IntMul { .. } => runtime::INT_MUL,
                    _ => runtime::INT_MOD,
                };
                if op == runtime::INT_MOD {
                    // Divisors 0 and -1 go to the exact path.
                    let divisor = self.builder.ins().sshr_imm_u(r, 1);
                    let shifted = self.builder.ins().iadd_imm_s(divisor, 1);
                    let special = self.builder.ins().icmp_imm_u(IntCC::UnsignedLessThanOrEqual, shifted, 1);
                    let commit = self.builder.create_block();
                    self.builder.ins().brif(special, slow, &[], commit, &[]);
                    self.builder.switch_to_block(commit);
                    let dividend = self.builder.ins().sshr_imm_u(l, 1);
                    let remainder = self.builder.ins().srem(dividend, divisor);
                    let doubled = self.builder.ins().ishl_imm_u(remainder, 1);
                    let tagged = self.builder.ins().bor_imm_u(doubled, 1);
                    self.builder.ins().jump(done, &[tagged.into()]);
                } else {
                    let even = self.builder.ins().iadd_imm_s(r, -1);
                    let (value, overflow) = match op {
                        runtime::INT_ADD => self.builder.ins().sadd_overflow(l, even),
                        runtime::INT_SUB => self.builder.ins().ssub_overflow(l, even),
                        _ => {
                            let half = self.builder.ins().sshr_imm_u(l, 1);
                            let (product, overflow) = self.builder.ins().smul_overflow(half, even);
                            (self.builder.ins().bor_imm_u(product, 1), overflow)
                        }
                    };
                    let commit = self.builder.create_block();
                    self.builder.ins().brif(overflow, slow, &[], commit, &[]);
                    self.builder.switch_to_block(commit);
                    self.builder.ins().jump(done, &[value.into()]);
                }
                self.builder.switch_to_block(slow);
                let op = self.iconst(op as i64);
                let result = self.helper(|h| h.int, &[ctx, op, l, r]).expect("result");
                self.check(pc);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::IntCmp { op, dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let both = self.builder.ins().band(l, r);
                let small = self.builder.ins().band_imm_u(both, 1);
                let fast = self.builder.create_block();
                let slow = self.builder.create_block();
                let done = self.builder.create_block();
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(small, fast, &[], slow, &[]);
                let condition = int_condition(*op);
                self.builder.switch_to_block(fast);
                let flag = self.builder.ins().icmp(condition, l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.builder.ins().jump(done, &[flag.into()]);
                self.builder.switch_to_block(slow);
                let order = self.helper(|h| h.int_order, &[ctx, l, r]).expect("result");
                let flag = self.builder.ins().icmp_imm_s(condition, order, 0);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.builder.ins().jump(done, &[flag.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::BoolNot { dst, operand } => {
                let value = self.get(operand);
                let value = self.builder.ins().bxor_imm_u(value, 1);
                self.set(&dst.0, value);
            }
            I::BoolCmp { op, dst, left, right }
            | I::ByteCmp { op, dst, left, right }
            | I::CharCmp { op, dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let flag = self.builder.ins().icmp(int_condition(*op), l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.set(&dst.0, flag);
            }
            I::FloatAdd { dst, left, right }
            | I::FloatSub { dst, left, right }
            | I::FloatMul { dst, left, right }
            | I::FloatDiv { dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let (value, op) = match instruction {
                    I::FloatAdd { .. } => (self.builder.ins().fadd(l, r), crate::scalar::FLOAT_ADD),
                    I::FloatSub { .. } => (self.builder.ins().fsub(l, r), crate::scalar::FLOAT_SUB),
                    I::FloatMul { .. } => (self.builder.ins().fmul(l, r), crate::scalar::FLOAT_MUL),
                    _ => (self.builder.ins().fdiv(l, r), crate::scalar::FLOAT_DIV),
                };
                let magnitude = self.builder.ins().fabs(value);
                let maximum = self.builder.ins().f64const(f64::MAX);
                let finite = self.builder.ins().fcmp(FloatCC::LessThanOrEqual, magnitude, maximum);
                let slow = self.builder.create_block();
                let done = self.builder.create_block();
                self.builder.append_block_param(done, types::F64);
                self.builder.ins().brif(finite, done, &[value.into()], slow, &[]);
                self.builder.switch_to_block(slow);
                let op = self.iconst(op as i64);
                let lw = self.builder.ins().bitcast(types::I64, MemFlagsData::new(), l);
                let rw = self.builder.ins().bitcast(types::I64, MemFlagsData::new(), r);
                let bits = self.helper(|h| h.float, &[ctx, op, lw, rw]).expect("result");
                self.check(pc);
                let value = self.builder.ins().bitcast(types::F64, MemFlagsData::new(), bits);
                self.builder.ins().jump(done, &[value.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::FloatNeg { dst, operand } | I::FloatAbs { dst, operand } => {
                let value = self.get(operand);
                let value = if matches!(instruction, I::FloatNeg { .. }) {
                    self.builder.ins().fneg(value)
                } else {
                    self.builder.ins().fabs(value)
                };
                self.set(&dst.0, value);
            }
            I::FloatCmp { op, dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let condition = match op {
                    Cmp::Eq => FloatCC::Equal,
                    Cmp::Ne => FloatCC::NotEqual,
                    Cmp::Lt => FloatCC::LessThan,
                    Cmp::Le => FloatCC::LessThanOrEqual,
                    Cmp::Gt => FloatCC::GreaterThan,
                    Cmp::Ge => FloatCC::GreaterThanOrEqual,
                };
                let flag = self.builder.ins().fcmp(condition, l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.set(&dst.0, flag);
            }
            I::StrConcat { dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let text = self.helper(|h| h.concat, &[ctx, l, r]).expect("result");
                self.set(&dst.0, text);
            }
            I::ArrayNew { dst, elements } => {
                let repr = match self.register_type(&dst.0) {
                    Some(Type::Array(element)) if Kind::of(program, *element) == Kind::Byte => runtime::ARRAY_BYTES,
                    Some(Type::Array(element)) => Kind::of(program, *element).repr() as u64,
                    _ => return Err(unsupported()),
                };
                let address = self.spill(elements);
                let repr = self.iconst(repr as i64);
                let count = self.iconst(elements.len() as i64);
                let array = self.helper(|h| h.array, &[ctx, repr, address, count]).expect("result");
                self.set(&dst.0, array);
            }
            I::ArrayLen { dst, array } => {
                let array = self.get(array);
                let header = self.builder.ins().load(types::I64, MEMORY, array, 0);
                let length = self.builder.ins().ushr_imm_u(header, 32);
                let doubled = self.builder.ins().ishl_imm_u(length, 1);
                let tagged = self.builder.ins().bor_imm_u(doubled, 1);
                self.set(&dst.0, tagged);
            }
            I::ArrayGet { dst, array, index } => {
                let array_register = *array;
                let (array, index) = (self.get(array), self.get(index));
                let header = self.builder.ins().load(types::I64, MEMORY, array, 0);
                let length = self.builder.ins().ushr_imm_u(header, 32);
                let small = self.builder.ins().band_imm_u(index, 1);
                let position = self.builder.ins().sshr_imm_u(index, 1);
                // Unsigned compare also rejects negative positions.
                let inside = self.builder.ins().icmp(IntCC::UnsignedLessThan, position, length);
                let inside = self.builder.ins().uextend(types::I64, inside);
                let valid = self.builder.ins().band(small, inside);
                let load = self.builder.create_block();
                let outside = self.builder.create_block();
                self.builder.ins().brif(valid, load, &[], outside, &[]);
                self.builder.switch_to_block(outside);
                let message = self.constant_text("Array index out of bounds");
                self.helper(|h| h.trap, &[ctx, message]);
                self.check(pc);
                self.builder.ins().jump(load, &[]);
                self.builder.switch_to_block(load);
                let value = self.element(array, position, self.bytes_array(array_register));
                self.set_word(&dst.0, value);
            }
            I::ObjNew { dst, object, fields } => {
                let words = fields.len().max(1) + 1;
                let pointer = self.allocate(words);
                let header = self.iconst(heap::header(heap::OBJECT, Repr::Raw, object.0) as i64);
                self.builder.ins().store(MEMORY, header, pointer, 0);
                for (i, field) in fields.iter().enumerate() {
                    let value = self.word(field);
                    self.builder.ins().store(MEMORY, value, pointer, ((i + 1) * 8) as i32);
                }
                self.set(&dst.0, pointer);
                self.created(object.0 as usize, pointer);
            }
            I::ObjGet { dst, object, field } => {
                let pointer = self.get(object);
                let value = self.builder.ins().load(types::I64, MEMORY, pointer, ((*field + 1) * 8) as i32);
                self.set_word(&dst.0, value);
            }
            I::ObjInit { object, field, value } => {
                let pointer = self.get(object);
                let value = self.word(value);
                self.builder.ins().store(MEMORY, value, pointer, ((*field + 1) * 8) as i32);
            }
            I::ObjNext { dst, object, updates } => {
                let declared = match self.register_type(object) {
                    Some(Type::Object(o) | Type::Successor(o)) => o.0 as usize,
                    _ => return Err(unsupported()),
                };
                let count = program.objects[declared].fields.len();
                let pointer = self.allocate(count.max(1) + 1);
                let source_object = self.get(object);
                for word_index in 0..=count {
                    let value = match updates.iter().find(|(f, _)| *f as usize + 1 == word_index) {
                        Some((_, register)) => self.word(register),
                        None => self.builder.ins().load(types::I64, MEMORY, source_object, (word_index * 8) as i32),
                    };
                    self.builder.ins().store(MEMORY, value, pointer, (word_index * 8) as i32);
                }
                self.set(&dst.0, pointer);
                self.created(declared, pointer);
            }
            I::RoleWrap { dst, source: value, .. } => {
                let value = self.get(value);
                self.set(&dst.0, value);
            }
            I::CallRole { dst, receiver, role, slot, args } => {
                let objects = program.objects.len();
                let table = self.jit.dispatch.entry((role.0, *slot)).or_insert_with(|| {
                    program
                        .objects
                        .iter()
                        .map(|o| {
                            o.does
                                .iter()
                                .find(|w| w.role == *role)
                                .map_or(u64::MAX, |w| u64::from(w.slots[*slot as usize].0))
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                });
                debug_assert_eq!(table.len(), objects);
                let table = table.as_ptr() as i64;
                let object = self.get(receiver);
                let header = self.builder.ins().load(types::I64, MEMORY, object, 0);
                let descriptor = self.builder.ins().ushr_imm_u(header, 32);
                let offset = self.builder.ins().ishl_imm_u(descriptor, 3);
                let base = self.iconst(table);
                let entry = self.builder.ins().iadd(base, offset);
                let function = self.builder.ins().load(types::I64, MEMORY, entry, 0);
                // Async witnesses: the call makes a cold Future of the method.
                let asynchronous = program
                    .objects
                    .iter()
                    .flat_map(|o| &o.does)
                    .filter(|w| w.role == *role)
                    .any(|w| program.functions[w.slots[*slot as usize].0 as usize].is_async);
                let result = if asynchronous {
                    let mut registers = vec![*receiver];
                    registers.extend(args.iter().copied());
                    let address = self.spill(&registers);
                    let count = self.iconst(registers.len() as i64);
                    let future = self.helper(|h| h.future_new, &[ctx, function, address, count]).expect("result");
                    self.check(pc);
                    future
                } else {
                    let mut values = vec![object];
                    for arg in args {
                        let value = self.word(arg);
                        values.push(value);
                    }
                    self.call(pc, function, &values)
                };
                if let Some(dst) = dst {
                    self.set_word(&dst.0, result);
                }
            }
            I::Call { dst, function, args } => {
                let callee = &program.functions[function.0 as usize];
                // FutureCompletions.completion reads its origins at the call.
                let eager = callee.intrinsic.as_ref().is_some_and(|i| i.name == "okkama.collect.FutureCompletions.completion");
                let result = if callee.is_async && !eager {
                    // Async functions (natives and intrinsics too) make a cold Future.
                    let address = self.spill(args);
                    let index = self.iconst(i64::from(function.0));
                    let count = self.iconst(args.len() as i64);
                    let future = self.helper(|h| h.future_new, &[ctx, index, address, count]).expect("result");
                    self.check(pc);
                    future
                } else if let Some(atomic) = atomic_access(callee) {
                    self.atomic(pc, function.0, atomic, args)
                } else if callee.native.is_some() || callee.intrinsic.is_some() {
                    let address = self.spill(args);
                    let index = self.iconst(i64::from(function.0));
                    let count = self.iconst(args.len() as i64);
                    let result = if callee.native.is_some() {
                        self.helper(|h| h.native, &[ctx, index, address, count])
                    } else {
                        self.helper(|h| h.intrinsic, &[ctx, index, address, count])
                    }
                    .expect("result");
                    self.check(pc);
                    result
                } else {
                    let mut values = Vec::with_capacity(args.len());
                    for arg in args {
                        let value = self.word(arg);
                        values.push(value);
                    }
                    let index = self.iconst(i64::from(function.0));
                    self.call(pc, index, &values)
                };
                if let Some(dst) = dst {
                    self.set_word(&dst.0, result);
                }
            }
            I::CallCap { dst, cap, ty, field } => {
                let function = program.caps[ty.0 as usize]
                    .fields
                    .get(*field as usize)
                    .ok_or_else(unsupported)?
                    .1
                    .0;
                let index = self.iconst(i64::from(function));
                let result = if program.functions[function as usize].is_async {
                    let address = self.spill(std::slice::from_ref(cap));
                    let count = self.iconst(1);
                    let future = self.helper(|h| h.future_new, &[ctx, index, address, count]).expect("result");
                    self.check(pc);
                    future
                } else {
                    let receiver = self.word(cap);
                    self.call(pc, index, &[receiver])
                };
                self.set_word(&dst.0, result);
            }
            I::Throw { value } => {
                let value = self.get(value);
                self.helper(|h| h.throw, &[ctx, value]);
                self.check(pc);
                let zero = self.iconst(0);
                self.builder.ins().return_(&[zero]);
                return Ok(true);
            }
            I::Trap { message, .. } => {
                let message = self.constant_text(message);
                self.helper(|h| h.trap, &[ctx, message]);
                self.check(pc);
                let zero = self.iconst(0);
                self.builder.ins().return_(&[zero]);
                return Ok(true);
            }
            I::Jump { target } => {
                self.jump(target);
                return Ok(true);
            }
            I::Branch { condition, then_target, else_target } => {
                let condition = self.get(condition);
                let (yes, no) = (self.builder.create_block(), self.builder.create_block());
                self.builder.ins().brif(condition, yes, &[], no, &[]);
                self.builder.switch_to_block(yes);
                self.jump(then_target);
                self.builder.switch_to_block(no);
                self.jump(else_target);
                return Ok(true);
            }
            I::Return { value } => {
                let value = match value {
                    Some(register) => self.word(register),
                    None => self.iconst(0),
                };
                if let Some(frame) = self.frame {
                    let frame = self.builder.use_var(frame);
                    self.helper(|h| h.complete, &[ctx, frame, value]);
                    let done = self.iconst(0);
                    self.builder.ins().return_(&[done]);
                } else {
                    self.builder.ins().return_(&[value]);
                }
                return Ok(true);
            }
            I::Await { dst, future } if self.frame.is_some() => {
                let this = self.builder.use_var(self.frame.expect("frame"));
                let future = self.get(future);
                let outcome = self.helper(|h| h.await_, &[ctx, this, future]).expect("result");
                self.check(pc);
                let suspend = self.builder.create_block();
                let ready = self.builder.create_block();
                let pending = self.builder.ins().icmp_imm_u(IntCC::Equal, outcome, crate::tasks::AWAIT_PENDING as i64);
                self.builder.ins().brif(pending, suspend, &[], ready, &[]);
                // Save every register and re-enter this await when resumed.
                self.builder.switch_to_block(suspend);
                let frame = self.builder.use_var(self.frame.expect("frame"));
                for register in 0..self.kinds.len() {
                    let value = self.word(&okkama_sparrow::Register(register as u32));
                    let offset = crate::tasks::REGISTERS as i32 + register as i32 * 8;
                    self.builder.ins().store(MEMORY, value, frame, offset);
                }
                let resume = self.iconst(pc as i64 + 1);
                self.builder.ins().store(MEMORY, resume, frame, crate::tasks::RESUME as i32);
                let one = self.iconst(1);
                self.builder.ins().return_(&[one]);
                self.builder.switch_to_block(ready);
                if let Some(dst) = dst {
                    let value = self.builder.ins().load(
                        types::I64,
                        MEMORY,
                        ctx,
                        std::mem::offset_of!(Ctx, scratch) as i32,
                    );
                    self.set_word(&dst.0, value);
                }
            }
            I::FutSpawn { dst, future } => {
                let future = self.get(future);
                let spawned = self.helper(|h| h.spawn, &[ctx, future]).expect("result");
                self.check(pc);
                self.set(&dst.0, spawned);
            }
            I::Fut { op, dst, future } => {
                let future = self.get(future);
                let op = self.iconst(*op as i64);
                let result = self.helper(|h| h.fut, &[ctx, op, future]).expect("result");
                self.check(pc);
                if let Some(dst) = dst {
                    self.set_word(&dst.0, result);
                }
            }
            I::ArrayLookup { array, index, value, found, missing } => {
                let array_register = *array;
                let (array, index) = (self.get(array), self.get(index));
                let header = self.builder.ins().load(types::I64, MEMORY, array, 0);
                let length = self.builder.ins().ushr_imm_u(header, 32);
                let small = self.builder.ins().band_imm_u(index, 1);
                let position = self.builder.ins().sshr_imm_u(index, 1);
                let inside = self.builder.ins().icmp(IntCC::UnsignedLessThan, position, length);
                let inside = self.builder.ins().uextend(types::I64, inside);
                let valid = self.builder.ins().band(small, inside);
                let hit = self.builder.create_block();
                let miss = self.builder.create_block();
                self.builder.ins().brif(valid, hit, &[], miss, &[]);
                self.builder.switch_to_block(hit);
                let item = self.element(array, position, self.bytes_array(array_register));
                // Edge moves read the old registers; the found value is written last.
                self.jump_then(found, value.0, item);
                self.builder.switch_to_block(miss);
                self.jump(missing);
                return Ok(true);
            }
            I::MapLookup { map, key, value, found, missing } => {
                let (map, key) = (self.get(map), self.get(key));
                let function = self.iconst(self.function as i64);
                let at = self.iconst(pc as i64);
                let present = self.helper(|h| h.map_find, &[ctx, function, at, map, key]).expect("result");
                let hit = self.builder.create_block();
                let miss = self.builder.create_block();
                self.builder.ins().brif(present, hit, &[], miss, &[]);
                self.builder.switch_to_block(hit);
                let item = self.builder.ins().load(
                    types::I64,
                    MEMORY,
                    ctx,
                    std::mem::offset_of!(Ctx, scratch) as i32,
                );
                self.jump_then(found, value.0, item);
                self.builder.switch_to_block(miss);
                self.jump(missing);
                return Ok(true);
            }
            I::Closure { dst, function, captures } => {
                let address = self.spill(captures);
                let index = self.iconst(i64::from(function.0));
                let count = self.iconst(captures.len() as i64);
                let closure = self.helper(|h| h.closure, &[ctx, index, address, count]).expect("result");
                self.set(&dst.0, closure);
            }
            I::CallFn { dst, callee, args } => {
                let closure = self.get(callee);
                let address = self.spill(args);
                let count = self.iconst(args.len() as i64);
                let result = self.helper(|h| h.call_closure, &[ctx, closure, address, count]).expect("result");
                self.check(pc);
                if let Some(dst) = dst {
                    self.set_word(&dst.0, result);
                }
            }
            I::Catch { failure, value, handler, next, .. } => {
                let failure = self.get(failure);
                let function = self.iconst(self.function as i64);
                let at = self.iconst(pc as i64);
                let matched = self.helper(|h| h.catch, &[ctx, function, at, failure]).expect("result");
                let hit = self.builder.create_block();
                let miss = self.builder.create_block();
                self.builder.ins().brif(matched, hit, &[], miss, &[]);
                self.builder.switch_to_block(hit);
                let payload = self.builder.ins().load(
                    types::I64,
                    MEMORY,
                    ctx,
                    std::mem::offset_of!(Ctx, scratch) as i32,
                );
                self.jump_then(handler, value.0, payload);
                self.builder.switch_to_block(miss);
                self.jump(next);
                return Ok(true);
            }
            I::RegionEnter { dst } => {
                // `[overflow, slots…]`, all unbound (see `failures::REGION_SLOTS`).
                let length = 1 + crate::failures::REGION_SLOTS;
                let cell = self.allocate(1 + length);
                let header = self.iconst(heap::header(heap::ARRAY, Repr::Ref, length as u32) as i64);
                self.builder.ins().store(MEMORY, header, cell, 0);
                let unbound = self.iconst(0);
                for index in 0..length {
                    self.builder.ins().store(MEMORY, unbound, cell, (8 + index * 8) as i32);
                }
                self.set(&dst.0, cell);
            }
            I::RegionAdopt { region, value } | I::RegionTrack { region, value } => {
                let (region, value) = (self.get(region), self.word(value));
                let own = self.iconst(i64::from(matches!(instruction, I::RegionAdopt { .. })));
                self.helper(|h| h.region_add, &[ctx, region, value, own]);
            }
            I::RegionLeave { region, .. } => {
                // A region that tracked nothing closes without a call.
                let cell = self.get(region);
                let entries = self.builder.ins().load(types::I64, MEMORY, cell, 16);
                let (close, done) = (self.builder.create_block(), self.builder.create_block());
                self.builder.ins().brif(entries, close, &[], done, &[]);
                self.builder.switch_to_block(close);
                let instruction = instruction.clone();
                self.failure_generic(pc, &instruction);
                self.builder.ins().jump(done, &[]);
                self.builder.switch_to_block(done);
            }
            other if failure_or_region(other) => {
                let mut uses = Vec::new();
                let mut def = None;
                other.visit(&mut |access| match access {
                    okkama_sparrow::Access::Use(register) => uses.push(register),
                    okkama_sparrow::Access::Def(register) => def = Some(register),
                    _ => {}
                });
                let address = self.spill(&uses);
                let function = self.iconst(self.function as i64);
                let at = self.iconst(pc as i64);
                let result = self.helper(|h| h.failure_op, &[ctx, function, at, address]).expect("result");
                self.check(pc);
                if matches!(other, I::Raise { .. } | I::Panic { .. }) {
                    let zero = self.iconst(0);
                    self.builder.ins().return_(&[zero]);
                    return Ok(true);
                }
                if let Some(def) = def {
                    self.set_word(&def, result);
                }
            }
            I::IntAnd { dst, left, right }
            | I::IntOr { dst, left, right }
            | I::IntXor { dst, left, right }
            | I::IntShl { dst, left, right }
            | I::IntShr { dst, left, right } => {
                // Two inline integers stay inline; the rest goes the exact way.
                let (l, r) = (self.get(left), self.get(right));
                let both = self.builder.ins().band(l, r);
                let small = self.builder.ins().band_imm_u(both, 1);
                let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(small, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                match instruction {
                    I::IntAnd { .. } => {
                        let value = self.builder.ins().band(l, r);
                        self.builder.ins().jump(done, &[value.into()]);
                    }
                    I::IntOr { .. } => {
                        let value = self.builder.ins().bor(l, r);
                        self.builder.ins().jump(done, &[value.into()]);
                    }
                    I::IntXor { .. } => {
                        let x = self.builder.ins().bxor(l, r);
                        let value = self.builder.ins().bor_imm_u(x, 1);
                        self.builder.ins().jump(done, &[value.into()]);
                    }
                    _ => {
                        let left_shift = matches!(instruction, I::IntShl { .. });
                        let v = self.builder.ins().sshr_imm_u(l, 1);
                        let n = self.builder.ins().sshr_imm_u(r, 1);
                        let in_range = self.builder.ins().icmp_imm_u(IntCC::UnsignedLessThan, n, 62);
                        let commit = self.builder.create_block();
                        self.builder.ins().brif(in_range, commit, &[], slow, &[]);
                        self.builder.switch_to_block(commit);
                        if left_shift {
                            // No bit may be lost: shifting back gives the value,
                            // and the result still fits an inline integer.
                            let shifted = self.builder.ins().ishl(v, n);
                            let tagged = self.builder.ins().ishl_imm_u(shifted, 1);
                            let tagged = self.builder.ins().bor_imm_u(tagged, 1);
                            let back = self.builder.ins().sshr(shifted, n);
                            let same = self.builder.ins().icmp(IntCC::Equal, back, v);
                            let untagged = self.builder.ins().sshr_imm_u(tagged, 1);
                            let fits = self.builder.ins().icmp(IntCC::Equal, untagged, shifted);
                            let ok = self.builder.ins().band(same, fits);
                            let exact = self.builder.create_block();
                            self.builder.ins().brif(ok, exact, &[], slow, &[]);
                            self.builder.switch_to_block(exact);
                            self.builder.ins().jump(done, &[tagged.into()]);
                        } else {
                            let shifted = self.builder.ins().sshr(v, n);
                            let tagged = self.builder.ins().ishl_imm_u(shifted, 1);
                            let tagged = self.builder.ins().bor_imm_u(tagged, 1);
                            self.builder.ins().jump(done, &[tagged.into()]);
                        }
                    }
                }
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::IntNot { dst, operand } | I::IntNeg { dst, operand } => {
                let v = self.get(operand);
                let small = self.builder.ins().band_imm_u(v, 1);
                let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(small, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                if matches!(instruction, I::IntNot { .. }) {
                    // !(2v+1) | 1 == 2(!v)+1
                    let x = self.builder.ins().bnot(v);
                    let value = self.builder.ins().bor_imm_u(x, 1);
                    self.builder.ins().jump(done, &[value.into()]);
                } else {
                    // 2(-v)+1 == 2 - (2v+1)
                    let two = self.iconst(2);
                    let (value, overflow) = self.builder.ins().ssub_overflow(two, v);
                    let commit = self.builder.create_block();
                    self.builder.ins().brif(overflow, slow, &[], commit, &[]);
                    self.builder.switch_to_block(commit);
                    self.builder.ins().jump(done, &[value.into()]);
                }
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::StrCmp { op: op @ (Cmp::Eq | Cmp::Ne), dst, left, right } => {
                // The same string, or strings of different byte lengths, decide
                // equality inline; the rest compares bytes in the slow path.
                let (l, r) = (self.get(left), self.get(right));
                let equal = u64::from(*op == Cmp::Eq) as i64;
                let (decided, compare, slow, done) = (
                    self.builder.create_block(),
                    self.builder.create_block(),
                    self.builder.create_block(),
                    self.builder.create_block(),
                );
                self.builder.append_block_param(decided, types::I64);
                self.builder.append_block_param(done, types::I64);
                let same = self.builder.ins().icmp(IntCC::Equal, l, r);
                let yes = self.iconst(equal);
                self.builder.ins().brif(same, decided, &[yes.into()], compare, &[]);
                self.builder.switch_to_block(compare);
                let (hl, hr) = (
                    self.builder.ins().load(types::I64, MEMORY, l, 0),
                    self.builder.ins().load(types::I64, MEMORY, r, 0),
                );
                let (nl, nr) = (self.builder.ins().ushr_imm_u(hl, 32), self.builder.ins().ushr_imm_u(hr, 32));
                let differ = self.builder.ins().icmp(IntCC::NotEqual, nl, nr);
                let no = self.iconst(1 - equal);
                self.builder.ins().brif(differ, decided, &[no.into()], slow, &[]);
                self.builder.switch_to_block(decided);
                let flag = self.builder.block_params(decided)[0];
                self.builder.ins().jump(done, &[flag.into()]);
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::ValCmp { op: op @ (Cmp::Eq | Cmp::Ne), dst, left, right }
                if matches!(self.kinds[left.0 as usize], Kind::Int | Kind::Raw | Kind::Byte | Kind::Float) =>
            {
                // Scalars compare by word (exact ints: two inline integers);
                // anything else takes the structural slow path.
                let int = self.kinds[left.0 as usize] == Kind::Int;
                let (l, r) = (self.word(left), self.word(right));
                let condition = if *op == Cmp::Eq { IntCC::Equal } else { IntCC::NotEqual };
                let flag = self.builder.ins().icmp(condition, l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                if int {
                    let both = self.builder.ins().band(l, r);
                    let small = self.builder.ins().band_imm_u(both, 1);
                    let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                    self.builder.append_block_param(done, types::I64);
                    self.builder.ins().brif(small, fast, &[], slow, &[]);
                    self.builder.switch_to_block(fast);
                    self.builder.ins().jump(done, &[flag.into()]);
                    self.builder.switch_to_block(slow);
                    let result = self.exec_generic(pc, instruction);
                    self.builder.ins().jump(done, &[result.into()]);
                    self.builder.switch_to_block(done);
                    let value = self.builder.block_params(done)[0];
                    self.set(&dst.0, value);
                } else {
                    self.set(&dst.0, flag);
                }
            }
            I::BuilderPush { dst, builder, value } => {
                let (builder, value) = (self.get(builder), self.word(value));
                let result = self.helper(|h| h.builder_push, &[ctx, builder, value]).expect("result");
                self.set(&dst.0, result);
            }
            I::Convert { dst, source }
                if matches!(self.register_type(source), Some(Type::Int)) && matches!(self.register_type(&dst.0), Some(Type::Str)) =>
            {
                let value = self.get(source);
                let text = self.helper(|h| h.int_text, &[ctx, value]).expect("result");
                self.set(&dst.0, text);
            }
            I::StrLen { dst, text } => {
                // ASCII strings know their length in characters: the byte length.
                let string = self.get(text);
                let header = self.builder.ins().load(types::I64, MEMORY, string, 0);
                let ascii = self.builder.ins().band_imm_u(header, heap::ASCII as i64);
                let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(ascii, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                let length = self.builder.ins().ushr_imm_u(header, 32);
                let doubled = self.builder.ins().ishl_imm_u(length, 1);
                let tagged = self.builder.ins().bor_imm_u(doubled, 1);
                self.builder.ins().jump(done, &[tagged.into()]);
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::EnumCase { dst, ty, case } => {
                let value = self.iconst(((u64::from(ty.0) << 32) | u64::from(*case)) as i64);
                self.set_word(&dst.0, value);
            }
            I::EnumCmp { op, dst, left, right } => {
                // Enum words compare as unsigned integers (see `ops`).
                let (l, r) = (self.word(left), self.word(right));
                let condition = match op {
                    Cmp::Eq => IntCC::Equal,
                    Cmp::Ne => IntCC::NotEqual,
                    Cmp::Lt => IntCC::UnsignedLessThan,
                    Cmp::Le => IntCC::UnsignedLessThanOrEqual,
                    Cmp::Gt => IntCC::UnsignedGreaterThan,
                    Cmp::Ge => IntCC::UnsignedGreaterThanOrEqual,
                };
                let flag = self.builder.ins().icmp(condition, l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.set(&dst.0, flag);
            }
            I::ByteFrom { dst, source, .. } if self.kinds[source.0 as usize] == Kind::Int => {
                let v = self.get(source);
                let n = self.builder.ins().sshr_imm_u(v, 1);
                let small = self.builder.ins().band_imm_u(v, 1);
                let in_range = self.builder.ins().icmp_imm_u(IntCC::UnsignedLessThanOrEqual, n, 255);
                let in_range = self.builder.ins().uextend(types::I64, in_range);
                let ok = self.builder.ins().band(small, in_range);
                let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(ok, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                self.builder.ins().jump(done, &[n.into()]);
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            other if generic(other) => {
                let mut def = None;
                other.visit(&mut |access| {
                    if let okkama_sparrow::Access::Def(register) = access {
                        def = Some(register);
                    }
                });
                let result = self.exec_generic(pc, other);
                if let Some(def) = def {
                    self.set_word(&def, result);
                }
            }
            _ => return Err(unsupported()),
        }
        Ok(false)
    }

    /// Is `register` statically a byte array (packed, one byte per element)?
    fn bytes_array(&self, register: okkama_sparrow::Register) -> bool {
        matches!(self.register_type(&register), Some(Type::Array(element)) if Kind::of(&self.loaded.program, *element) == Kind::Byte)
    }

    /// Element `position` (in bounds) of an array: a word, or a byte of a packed array.
    fn element(&mut self, array: Value, position: Value, bytes: bool) -> Value {
        if bytes {
            let address = self.builder.ins().iadd(array, position);
            return self.builder.ins().uload8(types::I64, MEMORY, address, 8);
        }
        let offset = self.builder.ins().ishl_imm_u(position, 3);
        let address = self.builder.ins().iadd(array, offset);
        self.builder.ins().load(types::I64, MEMORY, address, 8)
    }

    /// Inline access to an Atomic cell (`[header, value, closed]`; a closed
    /// cell takes the intrinsic call, which throws AtomicClosed) and to the
    /// set flag of an Event cell (`[header, initial, set, waiters]`).
    fn atomic(&mut self, pc: usize, function: u32, access: Intrinsic, args: &[okkama_sparrow::Register]) -> Value {
        let cell = self.get(&args[0]);
        let ctx = self.ctx();
        match access {
            Intrinsic::AtomicClosed | Intrinsic::EventReady => self.builder.ins().load(types::I64, MEMORY, cell, 16),
            Intrinsic::AtomicClose => {
                let one = self.iconst(1);
                self.builder.ins().store(MEMORY, one, cell, 16);
                let unbound = self.iconst(0);
                self.builder.ins().store(MEMORY, unbound, cell, 8);
                self.iconst(0)
            }
            _ => {
                let closed = self.builder.ins().load(types::I64, MEMORY, cell, 16);
                let (open, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(closed, slow, &[], open, &[]);
                self.builder.switch_to_block(open);
                let result = if access == Intrinsic::AtomicValue {
                    self.builder.ins().load(types::I64, MEMORY, cell, 8)
                } else {
                    let value = self.word(&args[1]);
                    self.builder.ins().store(MEMORY, value, cell, 8);
                    self.iconst(0)
                };
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(slow);
                let address = self.spill(args);
                let index = self.iconst(i64::from(function));
                let count = self.iconst(args.len() as i64);
                let result = self.helper(|h| h.intrinsic, &[ctx, index, address, count]).expect("result");
                self.check(pc);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                self.builder.block_params(done)[0]
            }
        }
    }

    /// Run a failure or region instruction through `rt_failure_op`; its result.
    fn failure_generic(&mut self, pc: usize, instruction: &I) -> Value {
        let mut uses = Vec::new();
        instruction.visit(&mut |access| {
            if let okkama_sparrow::Access::Use(register) = access {
                uses.push(register);
            }
        });
        let ctx = self.ctx();
        let address = self.spill(&uses);
        let function = self.iconst(self.function as i64);
        let at = self.iconst(pc as i64);
        let result = self.helper(|h| h.failure_op, &[ctx, function, at, address]).expect("result");
        self.check(pc);
        result
    }

    /// Run `instruction` through the shared slow path (`rt_exec`); its result word.
    fn exec_generic(&mut self, pc: usize, instruction: &I) -> Value {
        let mut uses = Vec::new();
        instruction.visit(&mut |access| {
            if let okkama_sparrow::Access::Use(register) = access {
                uses.push(register);
            }
        });
        let ctx = self.ctx();
        let address = self.spill(&uses);
        let function = self.iconst(self.function as i64);
        let at = self.iconst(pc as i64);
        let result = self.helper(|h| h.exec, &[ctx, function, at, address]).expect("result");
        self.check(pc);
        result
    }

    /// Give a new object a resource identity when its type can reach a destroy.
    fn created(&mut self, descriptor: usize, object: Value) {
        let resources = &self.resources;
        if resources.enabled && resources.flags[descriptor] {
            let ctx = self.ctx();
            let descriptor = self.iconst(descriptor as i64);
            self.helper(|h| h.created, &[ctx, descriptor, object]);
        }
    }

    /// Jump to `target` after its edge moves, then define `register` as `word`.
    fn jump_then(&mut self, target: &Target, register: okkama_sparrow::Register, word: Value) {
        let values: Vec<_> = target.moves.iter().map(|(_, source)| self.get(source)).collect();
        for ((dst, _), value) in target.moves.iter().zip(values) {
            self.set(dst, value);
        }
        self.set_word(&register, word);
        self.builder.ins().jump(self.blocks[target.pc as usize], &[]);
    }
}

/// Instructions executed by `failures::rt_failure_op`.
fn failure_or_region(instruction: &I) -> bool {
    matches!(
        instruction,
        I::RegionEnter { .. }
            | I::RegionAdopt { .. }
            | I::RegionTrack { .. }
            | I::RegionTransfer { .. }
            | I::RegionLeave { .. }
            | I::RegionUnwind { .. }
            | I::FailSecondary { .. }
            | I::FailCaused { .. }
            | I::Raise { .. }
            | I::Panic { .. }
            | I::RoleNarrow { .. }
    )
}

/// Instructions executed by `ops::rt_exec`.
/// Atomic operations compiled inline (see `Emitter::atomic`).
fn atomic_access(callee: &okkama_sparrow::Function) -> Option<Intrinsic> {
    let intrinsic = Intrinsic::resolve(&callee.intrinsic.as_ref()?.name)?;
    matches!(
        intrinsic,
        Intrinsic::AtomicValue
            | Intrinsic::AtomicReplace
            | Intrinsic::AtomicClosed
            | Intrinsic::AtomicClose
            | Intrinsic::EventReady
    )
    .then_some(intrinsic)
}

fn generic(instruction: &I) -> bool {
    matches!(
        instruction,
        I::ByteAnd { .. }
            | I::ByteOr { .. }
            | I::ByteXor { .. }
            | I::ByteShl { .. }
            | I::ByteShr { .. }
            | I::ByteUshr { .. }
            | I::ByteNot { .. }
            | I::Bits { .. }
            | I::RangeNew { .. }
            | I::RangePart { .. }
            | I::StrUtf8 { .. }
            | I::StrReplaceFirst { .. }
            | I::IntAnd { .. }
            | I::IntOr { .. }
            | I::IntXor { .. }
            | I::IntShl { .. }
            | I::IntShr { .. }
            | I::IntUshr { .. }
            | I::IntNot { .. }
            | I::IntNeg { .. }
            | I::IntAbs { .. }
            | I::Sign { .. }
            | I::FloatMod { .. }
            | I::FloatPow { .. }
            | I::FloatSqrt { .. }
            | I::Convert { .. }
            | I::ByteFrom { .. }
            | I::RatNew { .. }
            | I::RatAdd { .. }
            | I::RatSub { .. }
            | I::RatMul { .. }
            | I::RatDiv { .. }
            | I::RatMod { .. }
            | I::RatNeg { .. }
            | I::RatAbs { .. }
            | I::RatCmp { .. }
            | I::RatPart { .. }
            | I::StrCmp { .. }
            | I::StrLen { .. }
            | I::StrSlice { .. }
            | I::StrSplit { .. }
            | I::StrReplace { .. }
            | I::StrTrim { .. }
            | I::StrChomp { .. }
            | I::EnumCase { .. }
            | I::EnumCmp { .. }
            | I::ValCmp { .. }
            | I::Undef { .. }
            | I::AnyBox { .. }
            | I::ArraySlice { .. }
            | I::ArrayJoin { .. }
            | I::BuilderNew { .. }
            | I::BuilderPush { .. }
            | I::BuilderAppend { .. }
            | I::BuilderRange { .. }
            | I::BuilderFinish { .. }
            | I::MapNew { .. }
            | I::MapLen { .. }
            | I::MapHas { .. }
            | I::MapGet { .. }
            | I::MapKeys { .. }
            | I::MapValues { .. }
            | I::MapBuilderNew { .. }
            | I::MapBuilderPut { .. }
            | I::MapBuilderFinish { .. }
            | I::WorkerSpawn { .. }
            | I::WorkerBind { .. }
    )
}

fn int_condition(op: Cmp) -> IntCC {
    match op {
        Cmp::Eq => IntCC::Equal,
        Cmp::Ne => IntCC::NotEqual,
        Cmp::Lt => IntCC::SignedLessThan,
        Cmp::Le => IntCC::SignedLessThanOrEqual,
        Cmp::Gt => IntCC::SignedGreaterThan,
        Cmp::Ge => IntCC::SignedGreaterThanOrEqual,
    }
}
