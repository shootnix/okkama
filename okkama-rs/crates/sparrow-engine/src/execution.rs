use crate::value::{Heap, Slot};
use crate::{Code, Error, Integer, Op};
use okkama_sparrow::Cmp;
use okkama_sparrow::Target;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) const RETURNED: u32 = 0;
pub(crate) const YIELDED: u32 = 1;
pub(crate) const INVALID_STATE: u32 = 2;
pub(crate) const CALL: u32 = 3;
pub(crate) const ARITHMETIC: u32 = 4;
pub(crate) const UNBOUND_VALUE: u32 = 5;
pub(crate) const TRANSFER: u32 = 6;
pub(crate) const RAISE: u32 = 7;
pub(crate) const CLOSE: u32 = 8;
pub(crate) const UNREACHABLE: u32 = 9;
pub(crate) const AWAIT: u32 = 10;
pub(crate) const REGION_TRANSFER: u32 = 11;
/// A JIT heap call committed its instruction; collect before resuming at `pc`.
pub(crate) const COLLECT: u32 = 12;

/// Largest edge move list, argument list and register frame of a program:
/// the sizes of every task's reusable buffers. Computed once per program.
pub(crate) fn window(functions: &[Option<Rc<crate::FunctionPlan>>]) -> (usize, usize, usize) {
    let functions = || functions.iter().filter_map(Option::as_ref);
    let maximum_moves = functions()
        .flat_map(|f| &f.plan)
        .map(|op| match op {
            Op::Jump(t) => t.moves.len(),
            Op::Branch(_, a, b)
            | Op::Catch {
                found: a,
                missing: b,
                ..
            }
            | Op::Lookup {
                found: a,
                missing: b,
                ..
            } => a.moves.len().max(b.moves.len()),
            _ => 0,
        })
        .chain(functions().flat_map(|f| f.handlers.iter().map(|h| h.target.moves.len())))
        .max()
        .unwrap_or(0);
    let maximum_arguments = functions().map(|f| f.params.len()).max().unwrap_or(0);
    let maximum_registers = functions().map(|f| f.registers).max().unwrap_or(0);
    (maximum_moves, maximum_arguments, maximum_registers)
}

/// Private JIT boundary, valid only during resume. No native stack address is
/// retained by the continuation; slot storage is exclusively owned by Session.
#[repr(C)]
pub(crate) struct RunState {
    pub slots: *mut Slot,
    pub fuel: u64,
    pub result: Slot,
    pub pc: u32,
    /// Heap calls from machine code: the resuming task's slot window and heap,
    /// both exclusively borrowed for the duration of the run.
    pub slot_count: usize,
    pub heap: *mut Heap,
    /// Immutable program, for native calls made from machine code.
    pub code: *const Code,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Returned(Integer),
    Yielded,
    Waiting,
}

/// Internal tasks return typed VM values; only the command entrypoint requires int.
#[derive(Debug)]
pub(crate) enum TaskOutcome {
    Returned(Slot),
    Failed(Slot),
    Yielded,
    Waiting,
}

#[derive(Debug)]
struct Continuation {
    frame: u64,
    function: usize,
    pc: u32,
    saved_start: usize,
    dst: Option<usize>,
    cleanup: bool,
    future: Option<Slot>,
    suspended: bool,
}

#[derive(Debug)]
struct Description {
    pending: Vec<Slot>,
    current: Option<Slot>,
}

#[derive(Debug)]
struct Cleanup {
    closing: crate::resource::Closing,
    primary: Option<Slot>,
    dst: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct Task {
    function: usize,
    slots: Vec<Slot>,
    moves: Vec<Slot>,
    arguments: Vec<Slot>,
    continuations: Vec<Continuation>,
    cleanups: Vec<Cleanup>,
    saved: Vec<(usize, Slot)>,
    pc: u32,
    result: Option<Slot>,
    failure: Option<Error>,
    terminal_failure: Option<Slot>,
    description: Option<Description>,
    frame: u64,
    waiting: Option<Slot>,
    owner_future: Option<Slot>,
    spawn_request: Option<Slot>,
    native_task: bool,
    cancellation_wake: bool,
    cancellation_drain: Option<Slot>,
    cancel_signal: Option<tokio::sync::watch::Sender<()>>,
}

impl Task {
    /// Snapshot the terminal language failure, including independent cleanup traces.
    /// Invalid bytecode/internal faults may have no language failure record.
    pub fn failure_report(
        &self,
        code: &Code,
        heap: &Heap,
    ) -> Result<Option<crate::FailureReport>, Error> {
        self.terminal_failure
            .map(|root| crate::FailureReport::snapshot(code, heap, root))
            .transpose()
    }
    pub fn live_registers<'a>(&'a self, code: &'a Code) -> impl Iterator<Item = usize> + 'a {
        code.function(self.function)
            .live
            .get(self.pc as usize)
            .into_iter()
            .flatten()
            .copied()
            .filter(|_| !self.native_task && self.result.is_none() && self.failure.is_none())
    }

    pub fn live_integer(
        &self,
        code: &Code,
        heap: &Heap,
        register: usize,
    ) -> Result<Integer, Error> {
        if !self.live_registers(code).any(|r| r == register) {
            return Err(Error("register is not live".into()));
        }
        heap.integer(self.slots[register])
    }

    /// Explicit continuation storage, excluding the active function.
    pub fn continuation_depth(&self) -> usize {
        self.continuations.len()
    }
    pub fn saved_value_count(&self) -> usize {
        self.saved.len()
    }

    pub(crate) fn new(code: &Code) -> Self {
        let (maximum_moves, maximum_arguments, maximum_registers) = code.window;
        // One reusable active window. The verifier proves every observed slot is
        // initialized along its path; dead slots need no clearing on a call.
        let slots = vec![Slot::default(); maximum_registers];
        Self {
            function: code.entry,
            slots,
            moves: vec![Slot::default(); maximum_moves],
            arguments: vec![Slot::default(); maximum_arguments],
            continuations: Vec::new(),
            cleanups: Vec::new(),
            saved: Vec::new(),
            pc: 0,
            result: None,
            failure: None,
            terminal_failure: None,
            description: None,
            frame: u64::MAX,
            waiting: None,
            owner_future: None,
            spawn_request: None,
            native_task: false,
            cancellation_wake: false,
            cancellation_drain: None,
            cancel_signal: None,
        }
    }

    /// A bounded, flat dispatcher: generated code returns before another
    /// function is entered. Neither ordinary nor tail calls recurse in Rust.
    pub fn resume(
        &mut self,
        code: &Code,
        heap: &mut Heap,
        fuel: u64,
    ) -> Result<TaskOutcome, Error> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        if self.frame == u64::MAX {
            self.frame = heap.allocate_frame()?;
        }
        let result = self.resume_inner(code, heap, fuel);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    fn resume_inner(
        &mut self,
        code: &Code,
        heap: &mut Heap,
        mut fuel: u64,
    ) -> Result<TaskOutcome, Error> {
        if let Some(result) = &self.result {
            return Ok(TaskOutcome::Returned(*result));
        }
        while fuel != 0 {
            if self.spawn_request.is_some() {
                return Ok(TaskOutcome::Waiting);
            }
            if self.native_task {
                let owner = self.owner_future.expect("native task Future");
                return Ok(match heap.future(owner)?.state {
                    crate::future::State::Ready(Ok(value)) => TaskOutcome::Returned(value),
                    crate::future::State::Ready(Err(failure)) => TaskOutcome::Failed(failure),
                    _ => {
                        self.waiting = Some(owner);
                        TaskOutcome::Waiting
                    }
                });
            }
            if self.owner_future.is_some() {
                if let Some(failure) = self.terminal_failure {
                    return Ok(TaskOutcome::Failed(failure));
                }
            }
            if let Some(future) = self.waiting {
                heap.poll_collection(future)?;
                heap.poll_stream(future)?;
                if self.cancellation_wake {
                    self.cancellation_wake = false;
                    if let Some(failure) = self.take_cancellation(heap)? {
                        if matches!(
                            heap.future(future)?.state,
                            crate::future::State::Worker
                                | crate::future::State::Io
                                | crate::future::State::Permit(_, _)
                                | crate::future::State::Event(_)
                                | crate::future::State::Stream(_)
                                | crate::future::State::Collect(_)
                                | crate::future::State::Signal(_)
                        ) {
                            heap.future_control(okkama_sparrow::FutOp::Cancel, future)?;
                        }
                        if matches!(
                            heap.future(future)?.state,
                            crate::future::State::Io | crate::future::State::Worker
                        ) {
                            // Workers acknowledge cancellation only after process
                            // termination/reaping. Language cleanup must wait too.
                            self.cancellation_drain = Some(failure);
                            return Ok(TaskOutcome::Waiting);
                        }
                        self.waiting = None;
                        self.unwind(code, heap, failure)?;
                        continue;
                    }
                }
                if matches!(heap.future(future)?.state, crate::future::State::Ready(_)) {
                    self.waiting = None;
                    if let Some(cancelled) = self.cancellation_drain.take() {
                        if let crate::future::State::Ready(Err(failure)) =
                            heap.future(future)?.state
                        {
                            if matches!(
                                heap.failure(failure)?.kind,
                                crate::failure::FailureKind::Cancelled
                            ) {
                                let secondary = heap.failure(failure)?.secondary.clone();
                                for failure in secondary {
                                    heap.failure_link(cancelled, failure, true)?;
                                }
                            } else {
                                // A failed cancellation/reap is secondary to the
                                // parent cancellation, but must not disappear.
                                heap.failure_link(cancelled, failure, true)?;
                            }
                        }
                        self.unwind(code, heap, cancelled)?;
                        continue;
                    }
                    if let Some(failure) = self.take_cancellation(heap)? {
                        self.unwind(code, heap, failure)?;
                        continue;
                    }
                } else {
                    return Ok(TaskOutcome::Waiting);
                }
            }
            let function = code.function(self.function);
            let compiled = if let Some(kinds) = &function.jit_kinds {
                let pc = self.pc as usize;
                let region = pc / crate::jit::REGION_SIZE;
                let cell = function
                    .compiled
                    .get(region)
                    .ok_or_else(|| Error("invalid JIT region".into()))?;
                if cell.get().is_none() {
                    let start = region * crate::jit::REGION_SIZE;
                    let end = (start + crate::jit::REGION_SIZE).min(function.plan.len());
                    let compiled = crate::jit::Compiled::new(
                        code,
                        &function.plan,
                        kinds,
                        &function.live,
                        start..end,
                    )?;
                    cell.set(compiled)
                        .expect("single-threaded region compilation");
                }
                cell.get()
            } else {
                None
            };
            if let Some(compiled) = compiled {
                let mut state = RunState {
                    slots: self.slots.as_mut_ptr(),
                    fuel,
                    result: Slot::default(),
                    pc: self.pc,
                    slot_count: self.slots.len(),
                    heap: &mut *heap,
                    code,
                };
                let status = compiled.run(&mut state);
                self.pc = state.pc;
                fuel = state.fuel;
                crate::stats::record(|s| {
                    s.jit_entries += 1;
                    let name = match status {
                        RETURNED => "returned",
                        REGION_TRANSFER => "region-transfer",
                        COLLECT => "collect",
                        YIELDED => "yielded",
                        UNBOUND_VALUE => "unbound",
                        RAISE => "raise",
                        CLOSE => "close",
                        AWAIT => "await",
                        TRANSFER => "transfer",
                        ARITHMETIC => "service",
                        CALL => "call",
                        _ => "other",
                    };
                    *s.exits.entry(name).or_default() += 1;
                });
                match status {
                    RETURNED => {
                        if let Some(value) = self.return_value(code, heap, state.result)? {
                            return Ok(TaskOutcome::Returned(value));
                        }
                    }
                    REGION_TRANSFER => {}
                    COLLECT => self.collect(code, heap)?,
                    YIELDED => return Ok(TaskOutcome::Yielded),
                    UNBOUND_VALUE => self.program_failure(
                        code,
                        heap,
                        Error("unbound value used in evaluation".into()),
                    )?,
                    RAISE => self.raise(code, heap)?,
                    CLOSE => self.begin_cleanup(code, heap)?,
                    AWAIT => self.await_future(code, heap)?,
                    UNREACHABLE => return Err(Error("unreachable instruction reached".into())),
                    TRANSFER => {
                        if let Err(error) = self.lookup(code, heap) {
                            self.program_failure(code, heap, error)?;
                        }
                    }
                    ARITHMETIC => self.service(code, heap)?,
                    CALL => {
                        if let Err(error) = self.dispatch_call(code, heap) {
                            self.program_failure(code, heap, error)?;
                        }
                    }
                    _ => return Err(Error("new engine: invalid JIT execution state".into())),
                }
            } else {
                fuel -= 1;
                crate::stats::record(|s| s.cold_steps += 1);
                let op = function
                    .plan
                    .get(self.pc as usize)
                    .ok_or_else(|| Error("new engine: invalid cold execution state".into()))?;
                if op
                    .required_bound()
                    .into_iter()
                    .flatten()
                    .any(|r| self.slots[r].tag == crate::value::UNBOUND)
                {
                    self.program_failure(
                        code,
                        heap,
                        Error("unbound value used in evaluation".into()),
                    )?;
                    continue;
                }
                match op {
                    Op::Await { .. } => {
                        self.await_future(code, heap)?;
                        continue;
                    }
                    Op::Unreachable => return Err(Error("unreachable instruction reached".into())),
                    Op::Arithmetic(..)
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
                    | Op::Panic(..)
                    | Op::Closure { .. }
                    | Op::Role(..)
                    | Op::AnyBox(..)
                    | Op::RegionNew(..)
                    | Op::RegionRegister(..)
                    | Op::RegionTransfer(..)
                    | Op::FailureLink(..)
                    | Op::FutureControl(..)
                    | Op::WorkerSpawn { .. }
                    | Op::WorkerBind { .. }
                    | Op::FutureSpawn(..)
                    | Op::Intrinsic(..)
                    | Op::Equal(..) => {
                        self.service(code, heap)?;
                        continue;
                    }
                    Op::RegionClose { .. } => {
                        self.begin_cleanup(code, heap)?;
                        continue;
                    }
                    Op::Throw(..) | Op::Raise(..) => {
                        self.raise(code, heap)?;
                        continue;
                    }
                    Op::Lookup { .. } | Op::Catch { .. } => {
                        if let Err(error) = self.lookup(code, heap) {
                            self.program_failure(code, heap, error)?;
                        }
                        continue;
                    }
                    Op::Capture(pairs) => {
                        // Destinations are fresh registers, never source aliases.
                        for (dst, src) in pairs {
                            self.slots[*dst] = self.slots[*src];
                        }
                    }
                    Op::Const(dst, value) => self.slots[*dst] = *value,
                    Op::Copy(dst, source) => self.slots[*dst] = self.slots[*source],
                    Op::Not(dst, source) => {
                        self.slots[*dst] = Slot::boolean(self.slots[*source].bits == 0)
                    }
                    Op::Jump(target) => {
                        self.jump(target);
                        continue;
                    }
                    Op::Branch(condition, yes, no) => {
                        self.jump(if self.slots[*condition].bits != 0 {
                            yes
                        } else {
                            no
                        });
                        continue;
                    }
                    Op::CallFn { .. } | Op::CallRole { .. } | Op::CallCap { .. } => {
                        if let Err(error) = self.dispatch_call(code, heap) {
                            self.program_failure(code, heap, error)?;
                        }
                        continue;
                    }
                    Op::Call {
                        dst,
                        function,
                        args,
                        tail,
                    } => {
                        self.call(code, heap, *function, *dst, args, *tail)?;
                        continue;
                    }
                    Op::Return(_) | Op::ReturnVoid => {
                        let value = if let Op::Return(source) = op {
                            self.slots[*source]
                        } else {
                            Slot {
                                bits: 0,
                                tag: crate::value::UNBOUND,
                            }
                        };
                        if let Some(value) = self.return_value(code, heap, value)? {
                            return Ok(TaskOutcome::Returned(value));
                        }
                        continue;
                    }
                }
                self.pc += 1;
            }
        }
        Ok(TaskOutcome::Yielded)
    }

    fn dispatch_call(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        crate::stats::record(|s| {
            s.calls += 1;
            let name = crate::stats::op_name(&code.function(self.function).plan[self.pc as usize]);
            *s.service.entry(name).or_default() += 1;
        });
        match &code.function(self.function).plan[self.pc as usize] {
            Op::CallCap {
                dst,
                receiver,
                function,
                ..
            } => self.call(
                code,
                heap,
                *function,
                Some(*dst),
                std::slice::from_ref(receiver),
                false,
            )?,
            Op::Call {
                dst,
                function,
                args,
                tail,
            } => self.call(code, heap, *function, *dst, args, *tail)?,
            Op::CallRole {
                dst,
                receiver,
                role,
                slot,
                args,
                tail,
            } => {
                let value = self.slots[*receiver];
                if matches!(
                    value.tag,
                    crate::value::NATIVE_FAILURE | crate::value::CANCELLED
                ) && crate::native::is_error_role(code, *role)
                {
                    if let Some(dst) = dst {
                        self.slots[*dst] = if value.tag == crate::value::CANCELLED {
                            heap.allocate_text("Operation was cancelled".into())?
                        } else {
                            heap.native_failure_fields(value)?[3]
                        };
                    }
                    self.pc += 1;
                    return Ok(());
                }
                let owner = heap.object_descriptor(value)?;
                let witness = code.objects[owner]
                    .does
                    .iter()
                    .find(|w| w.role.0 == *role)
                    .ok_or_else(|| Error("role witness is missing".into()))?;
                let function = witness.slots[*slot].0 as usize;
                self.arguments[0] = value;
                for (i, r) in args.iter().enumerate() {
                    self.arguments[i + 1] = self.slots[*r];
                }
                self.invoke(code, heap, function, *dst, *tail)?;
            }
            Op::CallFn {
                dst,
                callee,
                args,
                tail,
            } => {
                let (function, captures) = heap.closure(self.slots[*callee])?;
                self.arguments[..captures.len()].copy_from_slice(captures);
                for (i, r) in args.iter().enumerate() {
                    self.arguments[captures.len() + i] = self.slots[*r];
                }
                self.invoke(code, heap, function, *dst, *tail)?;
            }
            _ => return Err(Error("invalid call request".into())),
        }
        Ok(())
    }
    fn call(
        &mut self,
        code: &Code,
        heap: &mut Heap,
        callee: usize,
        dst: Option<usize>,
        args: &[usize],
        tail: bool,
    ) -> Result<(), Error> {
        // Capture all arguments before reusing the active frame, including cycles.
        for (i, source) in args.iter().enumerate() {
            self.arguments[i] = self.slots[*source];
        }
        self.invoke(code, heap, callee, dst, tail)
    }
    fn invoke(
        &mut self,
        code: &Code,
        heap: &mut Heap,
        callee: usize,
        dst: Option<usize>,
        tail: bool,
    ) -> Result<(), Error> {
        crate::stats::record(|s| {
            let function = code.function(callee);
            let kind = if function.native.is_some() {
                "native"
            } else if function.is_async {
                "async"
            } else if function.jit_kinds.is_none() {
                "plan"
            } else {
                "jit"
            };
            *s.callees
                .entry(format!("{kind} {}", function.symbol))
                .or_default() += 1;
        });
        if let Some(native) = code.function(callee).native {
            match heap.native_invoke(
                native,
                &self.arguments[..code.function(callee).params.len()],
            )? {
                crate::native::Outcome::Value(value) => {
                    if let Some(dst) = dst {
                        self.slots[dst] = value;
                    }
                }
                crate::native::Outcome::Failure(failure) => {
                    return self.unwind(code, heap, failure);
                }
            }
            self.pc += 1;
            self.collect(code, heap)
        } else if code.function(callee).is_async {
            let arguments = &self.arguments[..code.function(callee).params.len()];
            let future =
                if code.function(callee).cached {
                    let key = arguments[0].bits;
                    if let Some(&(_, future)) = heap.cached_futures.get(&key).and_then(|entries| {
                        entries.iter().find(|(function, _)| *function == callee)
                    }) {
                        future
                    } else {
                        let future = heap.future_new(callee, arguments)?;
                        heap.cached_futures
                            .entry(key)
                            .or_default()
                            .push((callee, future));
                        future
                    }
                } else {
                    heap.future_new(callee, arguments)?
                };
            if let Some(dst) = dst {
                self.slots[dst] = future;
            }
            self.pc += 1;
            self.collect(code, heap)
        } else {
            self.enter(code, heap, callee, dst, tail)
        }
    }
    fn take_cancellation(&mut self, heap: &mut Heap) -> Result<Option<Slot>, Error> {
        // Cancellation of an outer task propagates through inline cold awaits;
        // the innermost cleanup runs first, followed by its callers' cleanup.
        let mut cancel = false;
        for handle in self
            .owner_future
            .into_iter()
            .chain(self.continuations.iter().filter_map(|c| c.future))
        {
            let future = heap.future(handle)?;
            cancel |= future.cancel_requested && !future.cancel_delivered;
            if cancel {
                heap.futures.get_mut(handle.bits)?.cancel_requested = true;
            }
        }
        if let Some(active) = self
            .continuations
            .iter()
            .rev()
            .find_map(|c| c.future)
            .or(self.owner_future)
        {
            let future = heap.future(active)?;
            if future.cancel_requested && !future.cancel_delivered {
                heap.futures.get_mut(active.bits)?.cancel_delivered = true;
                return heap
                    .failure_new(crate::failure::FailureKind::Cancelled)
                    .map(Some);
            }
        }
        Ok(None)
    }
    fn await_future(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        let Op::Await { dst, future } = code.function(self.function).plan[self.pc as usize] else {
            return Err(Error("invalid await request".into()));
        };
        let handle = self.slots[future];
        heap.future(handle)?;
        heap.futures.get_mut(handle.bits)?.observed = true;
        let future = heap.future(handle)?;
        match future.state {
            crate::future::State::Ready(Ok(value)) => {
                if let Some(dst) = dst {
                    self.slots[dst] = value;
                }
                self.pc += 1;
                Ok(())
            }
            crate::future::State::Ready(Err(failure)) => self.unwind(code, heap, failure),
            crate::future::State::Cold => {
                let function = future.function;
                self.arguments[..future.captures.len()].copy_from_slice(&future.captures);
                heap.future_state(handle, crate::future::State::Running)?;
                self.enter(code, heap, function, dst, false)?;
                self.pc = code.function(function).cold_entry;
                self.continuations
                    .last_mut()
                    .expect("await continuation")
                    .future = Some(handle);
                Ok(())
            }
            // A timer remains a pending promise until the host delivers it.
            // Reading the clock here would skip a real language suspension and
            // change cancellation/cleanup semantics for very short timers.
            crate::future::State::Worker
            | crate::future::State::Io
            | crate::future::State::Timer(_)
            | crate::future::State::Stream(_)
            | crate::future::State::Collect(_)
            | crate::future::State::Signal(_)
            | crate::future::State::Event(_)
            | crate::future::State::Permit(_, _)
            | crate::future::State::Running => {
                self.waiting = Some(handle);
                for caller in &mut self.continuations {
                    if caller.future.is_some() {
                        caller.suspended = true;
                    }
                }
                Ok(())
            }
        }
    }

    fn enter(
        &mut self,
        code: &Code,
        heap: &mut Heap,
        callee: usize,
        dst: Option<usize>,
        tail: bool,
    ) -> Result<(), Error> {
        let frame = heap.allocate_frame()?;
        if !tail {
            let saved_start = self.saved.len();
            for &register in &code.function(self.function).call_saved[self.pc as usize] {
                self.saved.push((register, self.slots[register]));
            }
            self.continuations.push(Continuation {
                frame: self.frame,
                function: self.function,
                pc: self.pc + 1,
                saved_start,
                dst,
                cleanup: false,
                future: None,
                suspended: false,
            });
        }
        self.frame = frame;
        self.function = callee;
        self.pc = 0;
        let function = code.function(callee);
        for (i, &register) in function.params.iter().enumerate() {
            self.slots[register] = self.arguments[i];
        }
        Ok(())
    }

    /// True only when the root invocation has completed.
    fn return_value(
        &mut self,
        code: &Code,
        heap: &mut Heap,
        value: Slot,
    ) -> Result<Option<Slot>, Error> {
        let Some(continuation) = self.continuations.pop() else {
            if let Some(description) = &mut self.description {
                let failure = description.current.take().expect("active Error.text call");
                let message = heap.text(value)?.to_owned();
                heap.failures.get_mut(failure.bits)?.message = Some(message);
                self.advance_description(code, heap)?;
                return Ok(None);
            }
            if let Some(owner) = self.owner_future {
                heap.future_state(owner, crate::future::State::Ready(Ok(value)))?;
            }
            self.result = Some(value);
            return Ok(Some(value));
        };
        if let Some(future) = continuation.future {
            heap.future_state(future, crate::future::State::Ready(Ok(value)))?;
        }
        self.frame = continuation.frame;
        self.function = continuation.function;
        self.pc = continuation.pc;
        for &(register, saved) in &self.saved[continuation.saved_start..] {
            self.slots[register] = saved;
        }
        self.saved.truncate(continuation.saved_start);
        if continuation.suspended {
            if let Some(failure) = self.take_cancellation(heap)? {
                self.pc = continuation.pc - 1;
                self.unwind(code, heap, failure)?;
                return Ok(None);
            }
        }
        if continuation.cleanup {
            if let Some(failure) = self.advance_cleanup(code, heap)? {
                self.unwind(code, heap, failure)?;
            }
        } else if let Some(dst) = continuation.dst {
            self.slots[dst] = value;
        }
        Ok(None)
    }

    fn lookup(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        let (value, found, missing, result) =
            match &code.function(self.function).plan[self.pc as usize] {
                Op::Lookup {
                    map,
                    array,
                    index,
                    value,
                    found,
                    missing,
                } => {
                    let result = if *map {
                        heap.map_lookup(self.slots[*array], self.slots[*index])?
                    } else {
                        heap.array_lookup(self.slots[*array], self.slots[*index])?
                    };
                    (*value, found, missing, result)
                }
                Op::Catch {
                    failure,
                    kind,
                    ty,
                    value,
                    found,
                    missing,
                } => {
                    let payload = match heap.failure(self.slots[*failure])?.kind {
                        crate::failure::FailureKind::Thrown(payload) => {
                            let owner = heap.object_descriptor(payload)?;
                            let matches = match kind {
                                okkama_sparrow::CatchKind::Object => owner == *ty as usize,
                                okkama_sparrow::CatchKind::Role => {
                                    code.objects[owner].does.iter().any(|w| w.role.0 == *ty)
                                }
                            };
                            matches.then_some(payload)
                        }
                        crate::failure::FailureKind::Cancelled => {
                            let matches = match kind {
                                okkama_sparrow::CatchKind::Object => {
                                    code.objects[*ty as usize].builtin
                                        && code.objects[*ty as usize].name == "Cancelled"
                                }
                                okkama_sparrow::CatchKind::Role => {
                                    crate::native::is_error_role(code, *ty)
                                }
                            };
                            matches.then_some(Slot {
                                bits: 0,
                                tag: crate::value::CANCELLED,
                            })
                        }
                        crate::failure::FailureKind::Native(payload) => {
                            let matches = match kind {
                                okkama_sparrow::CatchKind::Object => {
                                    code.objects[*ty as usize].is_native_failure()
                                }
                                okkama_sparrow::CatchKind::Role => {
                                    crate::native::is_error_role(code, *ty)
                                }
                            };
                            matches.then_some(payload)
                        }
                        _ => None,
                    };
                    (*value, found, missing, payload)
                }
                _ => return Err(Error("invalid lookup request".into())),
            };
        if let Some(item) = result {
            self.jump(found);
            // Edge moves read the old registers; the found value is written last.
            self.slots[value] = item;
        } else {
            self.jump(missing);
        }
        Ok(())
    }
    fn service(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        crate::stats::record(|s| {
            let name = crate::stats::op_name(&code.function(self.function).plan[self.pc as usize]);
            *s.service.entry(name).or_default() += 1;
        });
        // A JIT heap or native call that failed left its outcome; do not execute
        // the instruction twice.
        if let Some(failure) = heap.deferred_raise.take() {
            return self.unwind(code, heap, failure);
        }
        let result = match heap.deferred_failure.take() {
            Some(error) => Err(error),
            None => self.arithmetic(code, heap),
        };
        if let Err(error) = result {
            let kind = if matches!(
                code.function(self.function).plan[self.pc as usize],
                Op::Panic(_)
            ) {
                crate::failure::FailureKind::Panic(error.0)
            } else {
                crate::failure::FailureKind::Program(error.0)
            };
            let failure = heap.failure_new(kind)?;
            self.unwind(code, heap, failure)
        } else {
            self.pc += 1;
            self.collect(code, heap)
        }
    }
    fn program_failure(&mut self, code: &Code, heap: &mut Heap, error: Error) -> Result<(), Error> {
        let failure = heap.failure_new(crate::failure::FailureKind::Program(error.0))?;
        self.unwind(code, heap, failure)
    }
    fn raise(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        let failure = match code.function(self.function).plan[self.pc as usize] {
            Op::Throw(src) if self.slots[src].tag == crate::value::CANCELLED => {
                heap.failure_new(crate::failure::FailureKind::Cancelled)?
            }
            Op::Throw(src) if self.slots[src].tag == crate::value::NATIVE_FAILURE => {
                heap.native_failure_fields(self.slots[src])?;
                heap.failure_new(crate::failure::FailureKind::Native(self.slots[src]))?
            }
            Op::Throw(src) => {
                heap.object_descriptor(self.slots[src])?;
                heap.failure_new(crate::failure::FailureKind::Thrown(self.slots[src]))?
            }
            Op::Raise(src) => {
                heap.failure(self.slots[src])?;
                self.slots[src]
            }
            _ => return Err(Error("invalid raise request".into())),
        };
        self.unwind(code, heap, failure)
    }
    fn unwind(&mut self, code: &Code, heap: &mut Heap, mut failure: Slot) -> Result<(), Error> {
        loop {
            heap.failure_record(failure, self.frame, self.function, self.pc)?;
            if let Some(handler) = code
                .function(self.function)
                .handlers
                .iter()
                .find(|h| h.start <= self.pc && self.pc < h.end)
            {
                let dst = handler.failure.0 as usize;
                self.jump(&handler.target);
                self.slots[dst] = failure;
                return Ok(());
            }
            let Some(caller) = self.continuations.pop() else {
                if self.description.is_some() {
                    self.description = None;
                    return Err(Error(
                        "failure while formatting Error.text is not supported yet".into(),
                    ));
                }
                self.terminal_failure = Some(failure);
                if let Some(owner) = self.owner_future {
                    heap.future_state(owner, crate::future::State::Ready(Err(failure)))?;
                    return Ok(());
                }
                return self.begin_description(code, heap, failure);
            };
            if let Some(future) = caller.future {
                heap.future_state(future, crate::future::State::Ready(Err(failure)))?;
            }
            self.frame = caller.frame;
            self.function = caller.function;
            self.pc = if caller.cleanup {
                caller.pc
            } else {
                caller.pc - 1
            };
            for &(register, value) in &self.saved[caller.saved_start..] {
                self.slots[register] = value;
            }
            self.saved.truncate(caller.saved_start);
            if caller.suspended {
                if let Some(cancelled) = self.take_cancellation(heap)? {
                    failure = cancelled;
                }
            }
            if caller.cleanup {
                let cleanup = self.cleanups.last_mut().expect("cleanup continuation");
                if let Some(primary) = cleanup.primary {
                    heap.failure_link(primary, failure, true)?;
                } else {
                    cleanup.primary = Some(failure);
                }
                if let Some(next) = self.advance_cleanup(code, heap)? {
                    failure = next;
                } else {
                    return Ok(());
                }
            }
        }
    }
    fn arithmetic(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        let op = &code.function(self.function).plan[self.pc as usize];
        match *op {
            Op::Intrinsic(operation, dst, arguments) => {
                let args = arguments.map(|r| r.map(|r| self.slots[r]));
                self.slots[dst] = if let crate::intrinsic::Operation::Test(operation) = operation {
                    heap.test_operation(operation, args, &code.objects)?
                } else if let crate::intrinsic::Operation::FakeStorage(op) = operation {
                    heap.fake_storage_operation(op, args, code)?
                } else if let crate::intrinsic::Operation::Stream(op) = operation {
                    heap.stream_operation(op, args)?
                } else if let crate::intrinsic::Operation::Barrier(op) = operation {
                    heap.barrier_operation(
                        op,
                        args[0].ok_or_else(|| Error("missing barrier receiver".into()))?,
                    )?
                } else if let crate::intrinsic::Operation::Collect(op) = operation {
                    heap.collect_operation(op, args)?
                } else if let crate::intrinsic::Operation::Group(op) = operation {
                    heap.group_operation(op, args)?
                } else if let crate::intrinsic::Operation::Channel(op) = operation {
                    heap.channel_operation(op, args)?
                } else if matches!(
                    operation,
                    crate::intrinsic::Operation::AtomicNew(_)
                        | crate::intrinsic::Operation::AtomicValue
                        | crate::intrinsic::Operation::AtomicReplace
                        | crate::intrinsic::Operation::AtomicCompareReplace
                        | crate::intrinsic::Operation::AtomicClosed
                        | crate::intrinsic::Operation::AtomicClose
                ) {
                    heap.atomic_operation(operation, args)?
                } else {
                    heap.intrinsic(operation, args[0])?
                };
            }
            Op::WorkerBind {
                dst,
                action,
                entry,
                ref fields,
            } => {
                self.slots[dst] = heap.worker_bind(
                    self.slots[action],
                    self.slots[entry],
                    fields.iter().map(|&r| self.slots[r]).collect(),
                )?;
            }
            Op::WorkerSpawn {
                dst,
                entry,
                ref args,
                direct,
            } => {
                let handle = heap.worker_spawn(
                    code,
                    self.slots[entry],
                    args.iter().map(|&r| self.slots[r]).collect(),
                    direct,
                )?;
                heap.futures.get_mut(handle.bits)?.spawned = true;
                heap.spawned.push(handle);
                self.spawn_request = Some(handle);
                self.slots[dst] = handle;
            }
            Op::FutureSpawn(dst, src) => {
                let handle = self.slots[src];
                if !heap.future(handle)?.spawned {
                    heap.futures.get_mut(handle.bits)?.spawned = true;
                    heap.spawned.push(handle);
                    self.spawn_request = Some(handle);
                }
                self.slots[dst] = handle;
            }
            Op::FutureControl(op, dst, future) => {
                let result = heap.future_control(op, self.slots[future])?;
                if let Some(dst) = dst {
                    self.slots[dst] = result;
                }
            }
            Op::RegionNew(dst) => self.slots[dst] = heap.region_new()?,
            Op::RegionRegister(region, value, own) => {
                heap.region_register(self.slots[region], self.slots[value], own)?
            }
            Op::RegionTransfer(from, to, value) => {
                heap.region_transfer(self.slots[from], self.slots[to], self.slots[value])?
            }
            Op::Trap(code, ref message) => {
                let _category = code;
                return Err(Error(message.clone()));
            }
            Op::Panic(src) => return Err(Error(heap.text(self.slots[src])?.to_owned())),
            Op::Role(dst, src, role, ref message) => {
                let value = self.slots[src];
                if matches!(
                    value.tag,
                    crate::value::NATIVE_FAILURE | crate::value::CANCELLED
                ) {
                    if crate::native::is_error_role(code, role) {
                        self.slots[dst] = value;
                        return Ok(());
                    }
                    return Err(Error(
                        message
                            .clone()
                            .unwrap_or_else(|| "role witness is missing".into()),
                    ));
                }
                let owner = heap.object_descriptor(value)?;
                if !code.objects[owner].does.iter().any(|w| w.role.0 == role) {
                    return Err(Error(
                        message
                            .clone()
                            .unwrap_or_else(|| "role witness is missing".into()),
                    ));
                }
                self.slots[dst] = value;
            }
            _ => return heap_op(op, &mut self.slots, heap),
        }
        Ok(())
    }

    fn begin_description(
        &mut self,
        code: &Code,
        heap: &mut Heap,
        failure: Slot,
    ) -> Result<(), Error> {
        let mut pending = vec![failure];
        let mut seen = std::collections::HashSet::new();
        let mut order = Vec::new();
        while let Some(next) = pending.pop() {
            if !seen.insert(next.bits) {
                continue;
            }
            let record = heap.failure(next)?;
            pending.extend(record.cause);
            pending.extend(record.secondary.iter().rev().copied());
            order.push(next);
        }
        order.reverse();
        self.description = Some(Description {
            pending: order,
            current: None,
        });
        self.advance_description(code, heap)
    }

    fn advance_description(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        while let Some(next) = self
            .description
            .as_mut()
            .expect("active diagnostic")
            .pending
            .pop()
        {
            let failure = heap.failure(next)?;
            if failure.message.is_some() {
                continue;
            }
            if let crate::failure::FailureKind::Thrown(object) = failure.kind {
                let descriptor = heap.object_descriptor(object)?;
                if let Some(function) = code.error_methods[descriptor] {
                    self.frame = heap.allocate_frame()?;
                    self.function = function;
                    self.pc = 0;
                    self.slots[code.function(function).params[0]] = object;
                    self.description.as_mut().unwrap().current = Some(next);
                    return Ok(());
                }
            }
        }
        self.description = None;
        let report = crate::FailureReport::snapshot(
            code,
            heap,
            self.terminal_failure.expect("terminal failure"),
        )?;
        Err(Error(report.message()))
    }

    fn begin_cleanup(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        let Op::RegionClose {
            region,
            keep,
            failure,
        } = code.function(self.function).plan[self.pc as usize]
        else {
            return Err(Error("invalid cleanup request".into()));
        };
        let primary = failure.map(|(_, src)| self.slots[src]);
        let kept = if let Some(primary) = primary {
            heap.failure_payloads(primary)?
        } else {
            keep.map(|r| self.slots[r]).into_iter().collect()
        };
        let closing = heap.region_close(self.slots[region], kept.into_iter())?;
        self.cleanups.push(Cleanup {
            closing,
            primary,
            dst: failure.map(|(dst, _)| dst),
        });
        if let Some(failure) = self.advance_cleanup(code, heap)? {
            self.unwind(code, heap, failure)?;
        }
        Ok(())
    }

    // Each destructor is an ordinary flat VM call. Its return/raise resumes this
    // cursor; nested cleanup owns another cursor, never a nested Rust invocation.
    fn advance_cleanup(&mut self, code: &Code, heap: &mut Heap) -> Result<Option<Slot>, Error> {
        loop {
            let cleanup = self.cleanups.last_mut().expect("active cleanup");
            if let Some(object) = heap.region_next(&mut cleanup.closing)? {
                let descriptor = heap.object_descriptor(object)?;
                if let Some(destroy) = code.objects[descriptor].destroy {
                    let frame = heap.allocate_frame()?;
                    let saved_start = self.saved.len();
                    for &register in &code.function(self.function).live[self.pc as usize] {
                        self.saved.push((register, self.slots[register]));
                    }
                    self.continuations.push(Continuation {
                        frame: self.frame,
                        function: self.function,
                        pc: self.pc,
                        saved_start,
                        dst: None,
                        cleanup: true,
                        future: None,
                        suspended: false,
                    });
                    self.frame = frame;
                    self.function = destroy.0 as usize;
                    self.pc = 0;
                    let receiver = code.function(self.function).params[0];
                    self.slots[receiver] = object;
                    return Ok(None);
                }
            } else {
                let cleanup = self.cleanups.pop().expect("active cleanup");
                if let Some(dst) = cleanup.dst {
                    self.slots[dst] = cleanup.primary.expect("unwind primary");
                } else if cleanup.primary.is_some() {
                    return Ok(cleanup.primary);
                }
                self.pc += 1;
                self.collect(code, heap)?;
                return Ok(None);
            }
        }
    }

    fn roots<'a>(&'a self, code: &'a Code) -> impl Iterator<Item = Slot> + 'a {
        self.live_registers(code)
            .map(|r| self.slots[r])
            .chain(self.saved.iter().map(|(_, value)| *value))
            .chain(self.result)
            .chain(self.owner_future)
            .chain(self.spawn_request)
            .chain(self.terminal_failure)
            .chain(self.cancellation_drain)
            .chain(self.waiting)
            .chain(self.continuations.iter().filter_map(|c| c.future))
            .chain(
                self.cleanups
                    .iter()
                    .flat_map(|c| std::iter::once(c.closing.region).chain(c.primary)),
            )
    }

    fn collect(&mut self, code: &Code, heap: &mut Heap) -> Result<(), Error> {
        if heap.needs_collection() {
            heap.collect(self.roots(code))?;
        }
        Ok(())
    }

    fn jump(&mut self, target: &Target) {
        for (i, (_, source)) in target.moves.iter().enumerate() {
            self.moves[i] = self.slots[source.0 as usize];
        }
        for (i, (dst, _)) in target.moves.iter().enumerate() {
            self.slots[dst.0 as usize] = self.moves[i];
        }
        self.pc = target.pc;
    }
}

/// Instructions that read registers and the heap and write at most their
/// destination register. Shared by the service path and JIT heap calls, so both
/// have one semantics. Validation precedes mutation; a failure is returned to
/// the caller, which raises it as a program failure.
pub(crate) fn heap_op(op: &Op, slots: &mut [Slot], heap: &mut Heap) -> Result<(), Error> {
    match *op {
        Op::Closure {
            dst,
            function,
            ref captures,
        } => {
            slots[dst] =
                heap.closure_new(function, captures.iter().map(|r| slots[*r]))?;
        }
        Op::FailureLink(dst, a, b, secondary) => {
            slots[dst] = heap.failure_link(slots[a], slots[b], secondary)?;
        }
        Op::AnyBox(dst, src) => {
            slots[dst] = heap.any_box(slots[src])?;
        }
        Op::Equal(op, dst, a, b) => {
            let equal = heap.equal(slots[a], slots[b])?;
            slots[dst] = Slot::boolean(if matches!(op, Cmp::Eq) { equal } else { !equal });
        }
        Op::ObjectNew(dst, descriptor, ref fields) => {
            slots[dst] =
                heap.object_new(descriptor, fields.iter().map(|r| slots[*r]))?;
        }
        Op::ObjectNext(dst, object, ref updates) => {
            slots[dst] = heap.object_next(
                slots[object],
                updates.iter().map(|(field, r)| (*field, slots[*r])),
            )?;
        }
        Op::ObjectInit(object, field, value) => {
            heap.object_init(slots[object], field, slots[value])?;
        }
        Op::ObjectGet(dst, object, field) => {
            slots[dst] = heap.object_get(slots[object], field)?;
        }
        Op::RangeNew(dst, start, end, inclusive) => {
            slots[dst] = heap.range_new(slots[start], slots[end], inclusive)?;
        }
        Op::RangePart(dst, range, end) => {
            slots[dst] = heap.range_part(slots[range], end)?;
        }
        Op::BuilderRange(dst, builder, start, end, inclusive) => {
            slots[dst] = heap.builder_range(
                slots[builder],
                slots[start],
                slots[end],
                inclusive,
            )?;
        }
        Op::MapNew(dst, ref entries) => {
            slots[dst] = heap.map_new(
                entries
                    .iter()
                    .map(|(k, v)| (slots[*k], slots[*v])),
            )?;
        }
        Op::Map(op, dst, args) => {
            slots[dst] = heap.map_op(
                op,
                args.map(|r| r.map(|r| slots[r]).unwrap_or_default()),
            )?;
        }
        Op::ArrayNew(dst, ref elements) => {
            slots[dst] =
                heap.allocate_array(elements.iter().map(|r| slots[*r]).collect())?;
        }
        Op::Array(op, dst, args) => {
            slots[dst] = heap.array_op(
                op,
                args.map(|r| r.map(|r| slots[r]).unwrap_or_default()),
            )?;
        }
        Op::Text(op, dst, args) => {
            slots[dst] = heap.text_op(
                op,
                args.map(|r| r.map(|r| slots[r]).unwrap_or_default()),
            )?;
        }
        Op::RatPart(dst, src, denominator) => {
            slots[dst] = heap.rat_part(slots[src], denominator)?;
        }
        Op::RatNew(dst, a, b) => {
            slots[dst] = heap.rat_new(slots[a], slots[b])?;
        }
        Op::RatBinary(op, dst, a, b) => {
            slots[dst] = heap.rat_binary(op, slots[a], slots[b])?;
        }
        Op::RatUnary(op, dst, src) => {
            slots[dst] = heap.rat_unary(op, slots[src])?;
        }
        Op::ByteBinary(op, dst, left, right) => {
            slots[dst] = heap.byte_binary(op, slots[left], slots[right])?;
        }
        Op::ByteNot(dst, src) => {
            slots[dst] = Slot {
                bits: slots[src].bits ^ 255,
                tag: crate::value::BYTE,
            };
        }
        Op::Bits(op, byte, dst, value, count) => {
            slots[dst] =
                heap.bits(op, byte, slots[value], count.map(|r| slots[r]))?;
        }
        Op::FloatBinary(op, dst, left, right) => {
            slots[dst] =
                crate::scalar::float_binary(op, slots[left], slots[right])?;
        }
        Op::FloatUnary(op, dst, source) => {
            slots[dst] = crate::scalar::float_unary(op, slots[source])?;
        }
        Op::Convert(target, dst, source) => {
            slots[dst] = heap.convert(target, slots[source])?;
        }
        Op::ByteFrom(dst, source, ref context) => {
            slots[dst] = heap.byte_from(slots[source], context.as_deref())?;
        }
        Op::Unary(op, dst, source) => {
            slots[dst] = heap.unary(op, slots[source])?;
        }
        Op::Arithmetic(op, dst, left, right) => {
            slots[dst] = heap.arithmetic(op, slots[left], slots[right])?;
        }
        Op::Compare(op, dst, left, right) => {
            let order = heap.compare(slots[left], slots[right])?;
            slots[dst] = Slot::boolean(match op {
                Cmp::Eq => order.is_eq(),
                Cmp::Ne => !order.is_eq(),
                Cmp::Lt => order.is_lt(),
                Cmp::Le => !order.is_gt(),
                Cmp::Gt => order.is_gt(),
                Cmp::Ge => !order.is_lt(),
            });
        }
        _ => return Err(Error("invalid numeric request".into())),
    }
    Ok(())
}

/// A standalone invocation. Immutable machine code can outlive the loader handle.
#[derive(Debug)]
pub struct Session {
    code: Rc<Code>,
    task: Task,
    heap: Rc<RefCell<Heap>>,
    root_set: usize,
    local: Option<Rc<tokio::task::LocalSet>>,
    startup: Option<crate::async_host::StartupWait>,
}
pub(crate) struct PendingWait {
    pub future: Slot,
    pub worker: Option<Rc<crate::worker_process::Request>>,
    pub io: Option<Rc<crate::native_io::Request>>,
    pub deadline: Option<std::time::Instant>,
    pub event: Option<tokio::sync::watch::Receiver<bool>>,
    pub permit: Option<Rc<crate::intrinsic::PermitRequest>>,
    pub changed: tokio::sync::watch::Receiver<()>,
    pub cancelled: tokio::sync::watch::Receiver<()>,
}
impl Session {
    pub(crate) fn begin_test(&mut self) {
        self.heap.borrow_mut().test_failures = Some(Vec::new());
    }
    pub(crate) fn finish_test(&mut self) -> Vec<String> {
        self.heap
            .borrow_mut()
            .test_failures
            .take()
            .unwrap_or_default()
    }
    pub(crate) fn worker_arguments(
        &mut self,
        reader: &mut crate::worker_wire::Reader<'_>,
    ) -> Result<(), Error> {
        let mut heap = self.heap.borrow_mut();
        for &register in &self.code.function(self.task.function).params {
            self.task.slots[register] = reader.value(&mut heap)?;
        }
        heap.publish_roots(self.root_set, self.task.roots(&self.code));
        Ok(())
    }
    pub(crate) fn worker_directory(&mut self, directory: &str) -> Result<(), Error> {
        let path = std::fs::canonicalize(directory).map_err(|e| Error(e.to_string()))?;
        if !path.is_dir() {
            return Err(Error("worker working directory is not a directory".into()));
        }
        self.heap.borrow_mut().cwd = Some(path);
        Ok(())
    }
    pub(crate) async fn worker_result(&mut self) -> Result<Vec<u8>, Error> {
        let value = self.complete_slot().await?;
        let mut bytes = Vec::new();
        crate::worker_wire::encode(&self.heap.borrow(), &mut bytes, value)?;
        Ok(bytes)
    }
    pub(crate) fn driver_group(&self) -> usize {
        Rc::as_ptr(&self.heap) as usize
    }
    pub(crate) fn fail_stalled_wait(&mut self) -> Result<(), Error> {
        let waiting = self.task.waiting.take();
        let mut heap = self.heap.borrow_mut();
        let failure = heap.failure_new(crate::failure::FailureKind::Program(
            "async main cannot complete: no task can make progress".into(),
        ))?;
        if let Some(waiting) = waiting {
            use crate::future::State;
            if matches!(
                heap.future(waiting)?.state,
                State::Timer(_)
                    | State::Event(_)
                    | State::Signal(_)
                    | State::Stream(_)
                    | State::Collect(_)
                    | State::Permit(_, _)
            ) {
                // Release native queue membership before language cleanup can
                // close/release the same resource. Never settle a running body
                // here: its cached failure is published after its own cleanup.
                heap.future_state(waiting, State::Ready(Err(failure)))?;
            }
        }
        self.task.unwind(&self.code, &mut heap, failure)?;
        heap.publish_roots(self.root_set, self.task.roots(&self.code));
        Ok(())
    }
    pub(crate) fn startup_wait(&mut self) -> Option<&mut crate::async_host::StartupWait> {
        self.startup.as_mut()
    }
    pub(crate) fn set_startup(&mut self, startup: crate::async_host::StartupWait) {
        self.startup = Some(startup);
    }
    pub(crate) fn clear_startup(&mut self) {
        self.startup = None;
    }
    pub(crate) fn fail_child(&mut self, error: Error) -> Result<(), Error> {
        if let Some(owner) = self.task.owner_future {
            let mut heap = self.heap.borrow_mut();
            let failure = heap.failure_new(crate::failure::FailureKind::Program(error.0))?;
            heap.future_state(owner, crate::future::State::Ready(Err(failure)))?;
        }
        Ok(())
    }
    pub(crate) fn take_spawn(&mut self) -> Result<Option<Option<Session>>, Error> {
        let Some(handle) = self.task.spawn_request.take() else {
            return Ok(None);
        };
        let state = self.heap.borrow().future(handle)?.state;
        if matches!(
            state,
            crate::future::State::Ready(_) | crate::future::State::Running
        ) {
            return Ok(Some(None));
        }
        Ok(Some(Some(self.future_session(handle, false)?)))
    }
    pub(crate) fn future_session(&self, handle: Slot, wait_only: bool) -> Result<Session, Error> {
        let mut task = Task::new(&self.code);
        let mut heap = self.heap.borrow_mut();
        let future = heap.future(handle)?;
        task.owner_future = Some(handle);
        task.native_task = wait_only || future.function == usize::MAX;
        if !task.native_task {
            task.function = future.function;
            task.pc = self.code.function(task.function).cold_entry;
            for (&register, &value) in self
                .code
                .function(task.function)
                .params
                .iter()
                .zip(future.captures.iter())
            {
                task.slots[register] = value;
            }
            heap.future_state(handle, crate::future::State::Running)?;
        }
        let root_set = heap.register_roots();
        heap.publish_roots(root_set, task.roots(&self.code));
        Ok(Session {
            code: self.code.clone(),
            task,
            heap: self.heap.clone(),
            root_set,
            local: None,
            startup: None,
        })
    }
    pub(crate) fn cancel_spawned(&mut self) -> Result<Option<Slot>, Error> {
        let mut heap = self.heap.borrow_mut();
        let mut pending = None;
        for index in 0..heap.spawned.len() {
            let handle = heap.spawned[index];
            heap.future_control(okkama_sparrow::FutOp::Cancel, handle)?;
            if pending.is_none()
                && !matches!(heap.future(handle)?.state, crate::future::State::Ready(_))
            {
                pending = Some(handle);
            }
        }
        Ok(pending)
    }
    pub(crate) fn unobserved_spawned_failure(&self) -> Result<Option<Slot>, Error> {
        for &handle in &self.heap.borrow().spawned {
            if let Some(failure) = self.unobserved_failure(handle)? {
                return Ok(Some(failure));
            }
        }
        Ok(None)
    }
    pub(crate) fn unobserved_failure(&self, handle: Slot) -> Result<Option<Slot>, Error> {
        let heap = self.heap.borrow();
        let future = heap.future(handle)?;
        if !future.observed {
            if let crate::future::State::Ready(Err(failure)) = future.state {
                if !matches!(
                    heap.failure(failure)?.kind,
                    crate::failure::FailureKind::Cancelled
                ) {
                    return Ok(Some(failure));
                }
            }
        }
        Ok(None)
    }
    pub(crate) fn clear_spawned(&mut self) {
        self.heap.borrow_mut().spawned.clear();
    }
    pub(crate) fn outcome_integer(&self, value: Slot) -> Result<Integer, Error> {
        self.heap.borrow().integer(value)
    }
    pub(crate) fn describe_failure(&mut self, failure: Slot) -> Result<(), Error> {
        self.task.result = None;
        self.task.failure = None;
        self.task.owner_future = None;
        self.task.waiting = None;
        self.task.terminal_failure = Some(failure);
        let mut heap = self.heap.borrow_mut();
        self.task.begin_description(&self.code, &mut heap, failure)
    }
    pub(crate) fn pending_wait(&mut self) -> Result<PendingWait, Error> {
        let handle = self
            .task
            .waiting
            .ok_or_else(|| Error("no pending Future".into()))?;
        let mut heap = self.heap.borrow_mut();
        let deadline = match heap.future(handle)?.state {
            crate::future::State::Worker | crate::future::State::Io => None,
            crate::future::State::Timer(deadline) => Some(deadline),
            crate::future::State::Running
            | crate::future::State::Ready(_)
            | crate::future::State::Stream(_)
            | crate::future::State::Collect(_)
            | crate::future::State::Signal(_)
            | crate::future::State::Event(_)
            | crate::future::State::Permit(_, _) => None,
            crate::future::State::Cold => {
                return Err(Error("waiting on an unstarted Future".into()));
            }
        };
        let event = if let Some(channel) = &heap.future(handle)?.request {
            Some(channel.subscribe())
        } else if let crate::future::State::Event(object) = heap.future(handle)?.state {
            Some(
                heap.events
                    .get(&object.bits)
                    .ok_or_else(|| Error("Invalid async event".into()))?
                    .subscribe(),
            )
        } else {
            None
        };
        let mut changed = heap.future_subscribe(handle)?;
        if matches!(heap.future(handle)?.state, crate::future::State::Ready(_)) {
            changed.mark_changed();
        }
        let cancel_permit = heap.future(handle)?.permit.as_ref().map(Rc::downgrade);
        let cancel_request = heap
            .future(handle)?
            .request
            .as_ref()
            .map(crate::signal::Request::downgrade);
        let signal = self
            .task
            .cancel_signal
            .get_or_insert_with(|| tokio::sync::watch::channel(()).0);
        for owner in self
            .task
            .owner_future
            .into_iter()
            .chain(self.task.continuations.iter().filter_map(|c| c.future))
        {
            if !self.task.native_task {
                let owner = heap.futures.get_mut(owner.bits)?;
                owner.cancel_wake = Some(signal.clone());
                owner.cancel_permit = cancel_permit.clone();
                owner.cancel_request = cancel_request.clone();
            }
        }
        let cancelled = signal.subscribe();
        Ok(PendingWait {
            future: handle,
            io: heap.future(handle)?.io.clone(),
            worker: if self.task.native_task {
                heap.future(handle)?.worker.clone()
            } else {
                None
            },
            deadline,
            event,
            permit: heap.future(handle)?.permit.clone(),
            changed,
            cancelled,
        })
    }
    pub(crate) fn wake_cancellation(&mut self) {
        self.task.cancellation_wake = true;
    }
    #[cfg(test)]
    pub(crate) fn pending_timer(&self) -> Result<(Slot, std::time::Instant), Error> {
        let future = self
            .task
            .waiting
            .ok_or_else(|| Error("no pending native operation".into()))?;
        let crate::future::State::Timer(deadline) = self.heap.borrow().future(future)?.state else {
            return Err(Error("unsupported pending native operation".into()));
        };
        Ok((future, deadline))
    }
    pub(crate) fn settle_pending(
        &mut self,
        future: Slot,
        permit: Option<tokio::sync::OwnedSemaphorePermit>,
    ) -> Result<(), Error> {
        let mut heap = self.heap.borrow_mut();
        match heap.future(future)?.state {
            crate::future::State::Worker => heap.worker_settle(future)?,
            crate::future::State::Io => heap.native_io_settle(future)?,
            crate::future::State::Signal(_) | crate::future::State::Stream(_) => {
                heap.settle_signal(future)?
            }
            crate::future::State::Permit(receiver, owner) => {
                let permit = permit.ok_or_else(|| Error("missing acquired permit".into()))?;
                let guard = if let Some(pool) = heap.semaphores.get(&receiver.bits).cloned() {
                    heap.semaphore_grant(owner, pool, permit)?
                } else {
                    heap.grant(owner, permit)?
                };
                heap.future_state(future, crate::future::State::Ready(Ok(guard)))?;
            }
            crate::future::State::Timer(_) => {
                heap.future_state(future, crate::future::State::Ready(Ok(Slot::boolean(true))))?
            }
            crate::future::State::Event(object)
                if *heap
                    .events
                    .get(&object.bits)
                    .ok_or_else(|| Error("Invalid async event".into()))?
                    .borrow() =>
            {
                heap.future_state(future, crate::future::State::Ready(Ok(object)))?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn new(code: Rc<Code>) -> Self {
        let mut heap = Heap::new(&code.constants, &code.strings);
        heap.enum_names = Some(code.enum_names.clone());
        heap.arguments = code.arguments.clone();
        heap.native_descriptor = code.native_descriptor;
        heap.resources.enabled = code.objects.iter().any(|o| o.destroy.is_some());
        heap.resources.descriptors = Some(code.resource_descriptors.clone());
        let task = Task::new(&code);
        let root_set = heap.register_roots();
        heap.publish_roots(root_set, task.roots(&code));
        Self {
            code,
            task,
            heap: Rc::new(RefCell::new(heap)),
            root_set,
            local: None,
            startup: None,
        }
    }
    pub(crate) fn local_set(&mut self) -> Rc<tokio::task::LocalSet> {
        self.local
            .get_or_insert_with(|| Rc::new(tokio::task::LocalSet::new()))
            .clone()
    }
    pub fn resume(&mut self, fuel: u64) -> Result<Outcome, Error> {
        match self.resume_task(fuel)? {
            TaskOutcome::Returned(value) => {
                Ok(Outcome::Returned(self.heap.borrow().integer(value)?))
            }
            TaskOutcome::Failed(failure) => Err(Error(
                crate::FailureReport::snapshot(&self.code, &self.heap.borrow(), failure)?.message(),
            )),
            TaskOutcome::Yielded => Ok(Outcome::Yielded),
            TaskOutcome::Waiting => Ok(Outcome::Waiting),
        }
    }
    pub(crate) fn resume_task(&mut self, fuel: u64) -> Result<TaskOutcome, Error> {
        if self.startup.is_some() {
            return Ok(TaskOutcome::Waiting);
        }
        // One dynamic borrow per execution slice, never per instruction or call.
        // Clear stale parked values before GC can run in this active task.
        let mut heap = self.heap.borrow_mut();
        heap.publish_roots(self.root_set, std::iter::empty());
        let outcome = self.task.resume(&self.code, &mut heap, fuel);
        heap.publish_roots(self.root_set, self.task.roots(&self.code));
        outcome
    }
    pub fn live_registers(&self) -> impl Iterator<Item = usize> + '_ {
        self.task.live_registers(&self.code)
    }
    pub fn live_integer(&self, register: usize) -> Result<Integer, Error> {
        self.task
            .live_integer(&self.code, &self.heap.borrow(), register)
    }
    pub fn continuation_depth(&self) -> usize {
        self.task.continuation_depth()
    }
    pub fn saved_value_count(&self) -> usize {
        self.task.saved_value_count()
    }
    /// Root checks: collect every COLLECTION_FLOOR allocations regardless of
    /// the live heap size, as the fixed trigger did before it became adaptive.
    pub fn stress_collection(&mut self) {
        self.heap.borrow_mut().stress_collection = true;
    }
    pub fn collection_count(&self) -> usize {
        self.heap.borrow().collections
    }
    pub fn failure_report(&self) -> Result<Option<crate::FailureReport>, Error> {
        self.task.failure_report(&self.code, &self.heap.borrow())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.heap.borrow_mut().unregister_roots(self.root_set);
    }
}

#[cfg(test)]
mod parked_task_tests {
    use super::*;
    use crate::{Executable, Mode};

    #[test]
    fn jit_compile_failure_is_terminal_without_fallback_and_releases_dead_values() {
        use okkama_sparrow::{
            Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Type,
            TypeRef,
        };
        let mut instructions = vec![
            I::Const {
                dst: Dst(R(0)),
                value: Constant::Int("9223372036854775808".into()),
            },
            I::IntAdd {
                dst: Dst(R(0)),
                left: R(0),
                right: R(0),
            },
        ];
        while instructions.len() < crate::jit::REGION_SIZE {
            instructions.push(I::Move {
                dst: Dst(R(1)),
                source: R(0),
            });
        }
        instructions.push(I::Return { value: Some(R(0)) });
        let program = Program {
            module: "jit_failure".into(),
            version: "1.0.0".into(),
            types: vec![Type::Int],
            entry: Some(FuncRef(0)),
            functions: vec![Function {
                symbol: "main".into(),
                is_async: false,
                native: None,
                intrinsic: None,
                registers: vec![TypeRef(0); 2],
                params: vec![],
                result: Some(TypeRef(0)),
                handlers: vec![],
                code: instructions,
            }],
            ..Program::default()
        };
        for start in [0, crate::jit::REGION_SIZE] {
            let executable = Executable::new(&program, Mode::Jit).unwrap();
            let mut session = executable.start();
            let error = crate::jit::with_failed_region(start, || session.resume(128)).unwrap_err();
            assert_eq!(error.0, "new engine JIT: injected compilation failure");
            assert_eq!(session.task.pc as usize, start);
            assert!(session.failure_report().unwrap().is_none());
            assert!(
                session.code.function(0).compiled[start / crate::jit::REGION_SIZE]
                    .get()
                    .is_none()
            );
            // Engine errors are terminal, so active registers are no longer live.
            // A computed bigint must not leak merely because the failed Session lives.
            assert_eq!(session.live_registers().count(), 0);
            let previous_value = session.task.slots[0];
            if start != 0 {
                assert_eq!(
                    session
                        .heap
                        .borrow()
                        .integer(previous_value)
                        .unwrap()
                        .to_string(),
                    "18446744073709551616"
                );
            }
            session
                .heap
                .borrow_mut()
                .collect(std::iter::empty())
                .unwrap();
            if start != 0 {
                assert!(session.heap.borrow().integer(previous_value).is_err());
            }
            // Injection is gone: a retry must still return the memoized error.
            for fuel in [0, 128] {
                assert_eq!(session.resume(fuel).unwrap_err().0, error.0);
                assert_eq!(session.task.pc as usize, start);
            }
            // A new session using the same Code may compile and run successfully.
            assert_eq!(
                executable.run().unwrap().to_string(),
                "18446744073709551616"
            );
        }
    }

    #[test]
    fn worker_directory_is_session_local_and_rejects_files() {
        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-worker.asm")).unwrap();
        let code = Executable::new(&program, Mode::Cold).unwrap();
        let mut session = code.start();
        let before = std::env::current_dir().unwrap();
        let directory = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        session
            .worker_directory(directory.to_str().unwrap())
            .unwrap();
        assert_eq!(session.heap.borrow().cwd.as_ref(), Some(&directory));
        assert_eq!(std::env::current_dir().unwrap(), before);
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(session.worker_directory(file.to_str().unwrap()).is_err());
        assert_eq!(session.heap.borrow().cwd.as_ref(), Some(&directory));
        assert_eq!(std::env::current_dir().unwrap(), before);
    }

    #[test]
    fn shutdown_selection_survives_gc_pruning_completed_tasks() {
        use crate::future::State;
        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-spawn-shutdown.asm"))
                .unwrap();
        let code = Executable::new(&program, Mode::Cold).unwrap();
        let mut session = code.start();
        let (first, second) = {
            let mut heap = session.heap.borrow_mut();
            let first = heap.future_with_state(State::Running).unwrap();
            let second = heap.future_with_state(State::Running).unwrap();
            for handle in [first, second] {
                heap.futures.get_mut(handle.bits).unwrap().spawned = true;
                heap.spawned.push(handle);
            }
            (first, second)
        };
        assert_eq!(session.cancel_spawned().unwrap().unwrap().bits, first.bits);
        {
            let mut heap = session.heap.borrow_mut();
            heap.future_state(first, State::Ready(Ok(Slot::small(0))))
                .unwrap();
            heap.collect(std::iter::empty()).unwrap();
            assert!(heap.future(first).is_err());
        }
        assert_eq!(session.cancel_spawned().unwrap().unwrap().bits, second.bits);
        session
            .heap
            .borrow_mut()
            .future_state(second, State::Ready(Ok(Slot::small(0))))
            .unwrap();
        assert!(session.cancel_spawned().unwrap().is_none());
    }

    #[test]
    fn idle_tokio_reports_stalled_wait_and_runs_language_defer() {
        for source in [
            include_str!("../tests/fixtures/stalled-event.asm"),
            include_str!("../tests/fixtures/stalled-channel.asm"),
            include_str!("../tests/fixtures/stalled-lock.asm"),
        ] {
            let program = okkama_sparrow::assemble(source).unwrap();
            for mode in [crate::Mode::Cold, crate::Mode::Jit] {
                let code = Executable::new(&program, mode).unwrap();
                let mut session = code.start();
                let host = crate::AsyncHost::new().unwrap();
                let error = host
                    .block_on(session.complete_async())
                    .unwrap()
                    .unwrap_err();
                assert!(error.0.contains("no task can make progress"));
                let heap = session.heap.borrow();
                assert!(heap.events.values().any(|event| *event.borrow()));
                assert!(heap.spawned.is_empty());
                assert!(
                    heap.locks
                        .values()
                        .all(|lock| lock.available_permits() == 1)
                );
            }
        }
    }

    #[test]
    fn abandoning_a_host_poll_keeps_spawned_task_frames_and_roots_for_resumption() {
        let source =
            include_str!("../tests/fixtures/async-spawn.asm").replace("1000000", "30000000");
        let program = okkama_sparrow::assemble(&source).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&program, mode).unwrap();
            let mut session = code.start();
            let host = crate::AsyncHost::new().unwrap();
            host.block_on(async {
                assert!(
                    tokio::time::timeout(
                        std::time::Duration::from_millis(1),
                        session.complete_async()
                    )
                    .await
                    .is_err()
                );
                assert!(!session.heap.borrow().spawned.is_empty());
                session
                    .heap
                    .borrow_mut()
                    .collect(std::iter::empty())
                    .unwrap();
                assert_eq!(
                    tokio::time::timeout(
                        std::time::Duration::from_secs(1),
                        session.complete_async()
                    )
                    .await
                    .expect("spawned tasks must survive a dropped host poll")
                    .unwrap(),
                    42
                );
            })
            .unwrap();
        }
    }

    #[test]
    fn eager_startup_handshake_survives_a_dropped_host_poll() {
        let source =
            include_str!("../tests/fixtures/async-spawn-eager.asm").replace("int 200", "int 20000");
        let program = okkama_sparrow::assemble(&source).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&program, mode).unwrap();
            let mut session = code.start();
            let host = crate::AsyncHost::new().unwrap();
            host.block_on(async {
                assert!(
                    tokio::time::timeout(
                        std::time::Duration::from_millis(1),
                        session.complete_async()
                    )
                    .await
                    .is_err()
                );
                assert!(session.startup.is_some());
                session
                    .heap
                    .borrow_mut()
                    .collect(std::iter::empty())
                    .unwrap();
                assert_eq!(
                    tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        session.complete_async()
                    )
                    .await
                    .unwrap()
                    .unwrap(),
                    42
                );
            })
            .unwrap();
        }
    }

    #[test]
    fn root_shutdown_cancels_the_child_and_runs_its_destroy_before_returning() {
        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-spawn-shutdown.asm"))
                .unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&program, mode).unwrap();
            let mut session = code.start();
            assert_eq!(session.resume(4096).unwrap(), Outcome::Waiting);
            let child = session.task.spawn_request.unwrap();
            let host = crate::AsyncHost::new().unwrap();
            assert_eq!(
                host.block_on(session.complete_async()).unwrap().unwrap(),
                42
            );
            let heap = session.heap.borrow();
            assert!(heap.spawned.is_empty());
            let crate::future::State::Ready(Err(failure)) = heap.future(child).unwrap().state
            else {
                panic!("child was not cancelled at shutdown");
            };
            let failure = heap.failure(failure).unwrap();
            assert!(matches!(
                failure.kind,
                crate::failure::FailureKind::Cancelled
            ));
            assert_eq!(failure.secondary.len(), 1);
            let cleanup = heap.failure(failure.secondary[0]).unwrap();
            let crate::failure::FailureKind::Thrown(payload) = cleanup.kind else {
                panic!("destroy did not run");
            };
            assert_eq!(
                session.code.objects[heap.object_descriptor(payload).unwrap()].name,
                "CleanupFailure"
            );
        }
    }

    #[test]
    fn parent_cancel_keeps_native_cleanup_rooted_until_it_finishes() {
        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-timer.asm")).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&program, mode).unwrap();
            let mut session = code.start();
            assert_eq!(session.resume(4096).unwrap(), Outcome::Waiting);
            let awaited = session.task.waiting.unwrap();
            let (release, wait) = tokio::sync::oneshot::channel();
            let owner = {
                let mut heap = session.heap.borrow_mut();
                let native = heap
                    .native_io_with_cleanup(
                        std::future::pending::<Result<bool, okkama_runtime_abi::NativeFailure>>(),
                        Some(Box::new(move || {
                            Box::pin(async move {
                                wait.await.unwrap();
                                Err(okkama_runtime_abi::NativeFailure {
                                    domain: "core.process".into(),
                                    code: "unexpected".into(),
                                    operation: "cleanup".into(),
                                    message: "cleanup test failure".into(),
                                })
                            })
                        })),
                    )
                    .unwrap();
                let request = heap.future(native).unwrap().io.clone();
                heap.future_state(awaited, crate::future::State::Io)
                    .unwrap();
                heap.futures.get_mut(awaited.bits).unwrap().io = request;
                let owner = heap
                    .future_with_state(crate::future::State::Running)
                    .unwrap();
                heap.future_control(okkama_sparrow::FutOp::Cancel, owner)
                    .unwrap();
                owner
            };
            session.task.owner_future = Some(owner);
            session.wake_cancellation();
            assert_eq!(session.resume(4096).unwrap(), Outcome::Waiting);
            assert!(session.task.terminal_failure.is_none());
            assert!(session.task.cancellation_drain.is_some());
            session
                .heap
                .borrow_mut()
                .collect(std::iter::empty())
                .unwrap();
            release.send(()).unwrap();
            let request = session
                .heap
                .borrow()
                .future(awaited)
                .unwrap()
                .io
                .clone()
                .unwrap();
            crate::AsyncHost::new()
                .unwrap()
                .block_on(std::future::poll_fn(|cx| request.poll(cx)))
                .unwrap();
            session.settle_pending(awaited, None).unwrap();
            assert!(matches!(
                session
                    .task
                    .resume(&session.code, &mut session.heap.borrow_mut(), 4096)
                    .unwrap(),
                TaskOutcome::Failed(_)
            ));
            let failure = session.task.terminal_failure.unwrap();
            let heap = session.heap.borrow();
            let failure = heap.failure(failure).unwrap();
            assert!(matches!(
                failure.kind,
                crate::failure::FailureKind::Cancelled
            ));
            assert_eq!(failure.secondary.len(), 1);
        }
    }

    #[test]
    fn parent_cancel_drains_simulated_worker_before_language_cleanup() {
        use crate::future::State;
        use crate::worker_process::{Failure, Request};
        use std::{
            cell::Cell,
            task::{Context, Poll, Waker},
        };

        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-spawn-shutdown.asm"))
                .unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            for cleanup_failed in [false, true] {
                let code = Executable::new(&program, mode).unwrap();
                let mut root = code.start();
                assert_eq!(root.resume(4096).unwrap(), Outcome::Waiting);
                // Create the child session directly: no Tokio task or OS worker.
                let mut child = root.take_spawn().unwrap().unwrap().unwrap();
                assert_eq!(child.resume(4096).unwrap(), Outcome::Waiting);
                let awaited = child.task.waiting.unwrap();
                let owner = child.task.owner_future.unwrap();
                let released = Rc::new(Cell::new(false));
                let ready = released.clone();
                let request = Request::from_test_future(std::future::poll_fn(move |_| {
                    if !ready.get() {
                        return Poll::Pending;
                    }
                    Poll::Ready(Err(Rc::new(if cleanup_failed {
                        Failure::Process("simulated reap failure".into())
                    } else {
                        Failure::Cancelled
                    })))
                }));
                {
                    let mut heap = child.heap.borrow_mut();
                    heap.future_state(awaited, State::Worker).unwrap();
                    heap.futures.get_mut(awaited.bits).unwrap().worker = Some(request.clone());
                    heap.future_control(okkama_sparrow::FutOp::Cancel, owner)
                        .unwrap();
                }
                child.wake_cancellation();
                assert_eq!(child.resume(4096).unwrap(), Outcome::Waiting);
                assert!(child.task.terminal_failure.is_none());
                assert!(child.task.cancellation_drain.is_some());
                child.heap.borrow_mut().collect(std::iter::empty()).unwrap();
                assert!(
                    request
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
                released.set(true);
                assert!(
                    request
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_ready()
                );
                child.settle_pending(awaited, None).unwrap();
                let outcome = child
                    .task
                    .resume(&child.code, &mut child.heap.borrow_mut(), 4096)
                    .unwrap();
                let TaskOutcome::Failed(failure) = outcome else {
                    panic!("cancelled child did not finish cleanup");
                };
                let heap = child.heap.borrow();
                let record = heap.failure(failure).unwrap();
                assert!(matches!(
                    record.kind,
                    crate::failure::FailureKind::Cancelled
                ));
                assert_eq!(record.secondary.len(), if cleanup_failed { 2 } else { 1 });
                if cleanup_failed {
                    assert!(record.secondary.iter().any(|&secondary| matches!(
                        &heap.failure(secondary).unwrap().kind,
                        crate::failure::FailureKind::Program(message) if message.contains("simulated reap failure")
                    )));
                }
                assert!(record.secondary.iter().any(|&secondary| matches!(
                    heap.failure(secondary).unwrap().kind,
                    crate::failure::FailureKind::Thrown(_)
                )));
            }
        }
    }

    #[test]
    fn two_parked_tasks_share_completion_and_survive_collection() {
        let p = okkama_sparrow::assemble(
            &include_str!("../tests/fixtures/async-timer.asm").replace("10000000", "10000000000"),
        )
        .unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            for cancel in [false, true] {
                let code = Executable::new(&p, mode).unwrap();
                let mut first = code.start();
                assert_eq!(first.resume(4096).unwrap(), Outcome::Waiting);
                let handle = first.task.waiting.unwrap();
                first
                    .heap
                    .borrow_mut()
                    .future_state(handle, crate::future::State::Running)
                    .unwrap();
                let mut task = Task::new(&first.code);
                task.function = first.task.function;
                task.pc = first.task.pc;
                task.slots.copy_from_slice(&first.task.slots);
                // The peer independently executes the await of a Running Future.
                let root_set = first.heap.borrow_mut().register_roots();
                first
                    .heap
                    .borrow_mut()
                    .publish_roots(root_set, task.roots(&first.code));
                let mut second = Session {
                    code: first.code.clone(),
                    task,
                    heap: first.heap.clone(),
                    root_set,
                    local: None,
                    startup: None,
                };
                assert_eq!(second.resume(4096).unwrap(), Outcome::Waiting);
                let heap = first.heap.clone();
                let host = crate::AsyncHost::new().unwrap();
                host.block_on(async move {
                    let a = tokio::task::spawn_local(async move { first.complete_async().await });
                    let b = tokio::task::spawn_local(async move { second.complete_async().await });
                    tokio::task::yield_now().await;
                    {
                        let mut heap = heap.borrow_mut();
                        heap.collect(std::iter::empty()).unwrap();
                        let result = if cancel {
                            Err(heap
                                .failure_new(crate::failure::FailureKind::Cancelled)
                                .unwrap())
                        } else {
                            Ok(Slot::boolean(true))
                        };
                        heap.future_state(handle, crate::future::State::Ready(result))
                            .unwrap();
                    }
                    for task in [a, b] {
                        let result = tokio::time::timeout(std::time::Duration::from_secs(1), task)
                            .await
                            .unwrap()
                            .unwrap();
                        if cancel {
                            assert!(result.unwrap_err().0.contains("Operation was cancelled"));
                        } else {
                            assert_eq!(result.unwrap(), 42);
                        }
                    }
                    if cancel {
                        let heap = heap.borrow();
                        let crate::future::State::Ready(Err(failure)) =
                            heap.future(handle).unwrap().state
                        else {
                            panic!("cancelled future lost its failure");
                        };
                        // Equal function/PC in different tasks are distinct frames.
                        assert_eq!(heap.failure(failure).unwrap().trace.len(), 2);
                    }
                })
                .unwrap();
            }
        }
    }

    #[test]
    fn task_results_keep_non_integer_heap_values_alive_after_completion() {
        let p = okkama_sparrow::assemble(
            r#"sparrow 9
module "task_value" "1.0.0"
type t0 int
type t1 str
type t2 future t1
function f0 "main" {
 registers [t2,t0]
 params []
 result t0
 0: call r0, f1, []
 1: const r1, int 0
 2: ret r1
}
function f1 "text" async {
 registers [t1,t1,t1]
 params []
 result t1
 0: const r0, str "a"
 1: const r1, str "b"
 2: str.concat r2, r0, r1
 3: ret r2
}
entry f0
"#,
        )
        .unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&p, mode).unwrap();
            let mut session = code.start();
            session.task.function = 1;
            let mut heap = session.heap.borrow_mut();
            let owner = heap.future_new(1, &[]).unwrap();
            heap.future_state(owner, crate::future::State::Running)
                .unwrap();
            session.task.owner_future = Some(owner);
            let TaskOutcome::Returned(value) =
                session.task.resume(&session.code, &mut heap, 100).unwrap()
            else {
                panic!("text task should complete");
            };
            heap.publish_roots(session.root_set, session.task.roots(&session.code));
            heap.collect(std::iter::empty()).unwrap();
            assert_eq!(heap.text(value).unwrap(), "ab");
            let crate::future::State::Ready(Ok(cached)) = heap.future(owner).unwrap().state else {
                panic!("uncached task result");
            };
            assert_eq!(heap.text(cached).unwrap(), "ab");
        }
        let mut failing = p;
        failing.functions[1].code[3] = okkama_sparrow::Instruction::Panic {
            message: okkama_sparrow::Register(2),
        };
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&failing, mode).unwrap();
            let mut session = code.start();
            session.task.function = 1;
            let mut heap = session.heap.borrow_mut();
            let owner = heap.future_new(1, &[]).unwrap();
            heap.future_state(owner, crate::future::State::Running)
                .unwrap();
            session.task.owner_future = Some(owner);
            let TaskOutcome::Failed(failure) =
                session.task.resume(&session.code, &mut heap, 100).unwrap()
            else {
                panic!("expected child failure");
            };
            heap.publish_roots(session.root_set, session.task.roots(&session.code));
            heap.collect(std::iter::empty()).unwrap();
            assert!(
                matches!(&heap.failure(failure).unwrap().kind, crate::failure::FailureKind::Panic(message) if message == "ab")
            );
            assert!(session.task.description.is_none());
            assert!(
                matches!(heap.future(owner).unwrap().state, crate::future::State::Ready(Err(cached)) if cached.bits == failure.bits)
            );
        }
    }

    #[test]
    fn cancelling_a_long_native_timer_wakes_a_parked_driver() {
        let mut p =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-timer.asm")).unwrap();
        p.functions[0].code[0] = okkama_sparrow::Instruction::Const {
            dst: okkama_sparrow::Dst(okkama_sparrow::Register(0)),
            value: okkama_sparrow::Constant::Str(i64::MAX.to_string()),
        };
        for (mode, owner_cancel) in [
            (Mode::Cold, false),
            (Mode::Jit, false),
            (Mode::Cold, true),
            (Mode::Jit, true),
        ] {
            let code = Executable::new(&p, mode).unwrap();
            let mut session = code.start();
            if owner_cancel {
                let mut heap = session.heap.borrow_mut();
                let owner = heap.future_new(session.task.function, &[]).unwrap();
                heap.future_state(owner, crate::future::State::Running)
                    .unwrap();
                session.task.owner_future = Some(owner);
            }
            assert_eq!(session.resume(4096).unwrap(), Outcome::Waiting);
            let future = if owner_cancel {
                session.task.owner_future.unwrap()
            } else {
                session.task.waiting.unwrap()
            };
            let heap = session.heap.clone();
            let host = crate::AsyncHost::new().unwrap();
            host.block_on(async {
                let cancel = tokio::task::spawn_local(async move {
                    tokio::task::yield_now().await;
                    heap.borrow_mut()
                        .future_control(okkama_sparrow::FutOp::Cancel, future)
                        .unwrap();
                });
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    session.complete_async(),
                )
                .await
                .expect("cancel must wake the timer before its deadline");
                assert!(result.unwrap_err().0.contains("Operation was cancelled"));
                cancel.await.unwrap();
            })
            .unwrap();
        }
    }

    #[test]
    fn suspended_task_roots_survive_peer_collection_and_drop_releases_them() {
        let p = okkama_sparrow::assemble(
            r#"sparrow 9
module "parked" "1.0.0"
type t0 int
type t1 str
function f0 "main" {
 registers [t1,t1,t1,t0]
 params []
 result t0
 0: const r0, str "a"
 1: const r1, str "b"
 2: str.concat r2, r0, r1
 3: str.len r3, r2
 4: ret r3
}
entry f0
"#,
        )
        .unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&p, mode).unwrap();
            let mut session = code.start();
            assert_eq!(session.resume(3).unwrap(), Outcome::Yielded);
            let text = session.task.slots[2];
            let heap = session.heap.clone();
            heap.borrow_mut().collect(std::iter::empty()).unwrap();
            assert_eq!(heap.borrow().text(text).unwrap(), "ab");
            // Removing a different task must not discard this task's roots.
            let peer = heap.borrow_mut().register_roots();
            heap.borrow_mut().unregister_roots(peer);
            heap.borrow_mut().collect(std::iter::empty()).unwrap();
            assert_eq!(heap.borrow().text(text).unwrap(), "ab");
            assert_eq!(session.resume(10).unwrap(), Outcome::Returned(2.into()));
            // Completion publishes no dead registers.
            heap.borrow_mut().collect(std::iter::empty()).unwrap();
            assert!(heap.borrow().text(text).is_err());
            let id = session.root_set;
            drop(session);
            assert_eq!(heap.borrow_mut().register_roots(), id);
        }
    }
}

#[cfg(test)]
mod diagnostic_shutdown_tests {
    #[test]
    fn tasks_started_by_lazy_error_text_finish_cleanup_before_host_shutdown() {
        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-error-text-spawn.asm"))
                .unwrap();
        for mode in [crate::Mode::Cold, crate::Mode::Jit] {
            let code = crate::Executable::new(&program, mode).unwrap();
            let mut session = code.start();
            let host = crate::AsyncHost::new().unwrap();
            let error = host
                .block_on(session.complete_async())
                .unwrap()
                .unwrap_err();
            assert!(error.to_string().contains("background failure"));
            let heap = session.heap.borrow();
            assert_eq!(heap.events.len(), 1);
            assert!(
                heap.spawned.is_empty(),
                "diagnostics left registered tasks alive"
            );
            assert!(
                heap.events.values().all(|event| *event.borrow()),
                "diagnostic child cleanup was skipped"
            );
        }
    }
}
