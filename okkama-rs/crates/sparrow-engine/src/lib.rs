//! Independent execution core. Unsupported SPB never falls back to the old VM.
mod arena;
mod async_host;
pub use async_host::AsyncHost;
mod array;
mod barrier;
mod channel;
mod collect;
mod diagnostic;
mod equality;
pub use diagnostic::{FailureCategory, FailureNode, FailureReport, FailureSite};
mod execution;
mod failure;
mod fake_storage;
#[cfg(unix)]
mod file_lock;
mod future;
mod group;
mod handle;
mod intrinsic;
mod jit;
mod liveness;
mod map;
mod native;
mod native_binary;
mod native_buffer;
mod native_crypto;
mod native_data;
mod native_db;
mod native_directory;
mod native_dns;
mod native_encoding;
mod native_environment;
mod native_file;
mod native_file_state;
mod native_fs;
mod native_glob;
mod native_idna;
mod native_input;
mod native_io;
mod native_json;
mod native_multicast;
mod native_net;
mod native_network;
mod native_numbers;
#[cfg(unix)]
mod native_process;
mod native_regex;
#[cfg(unix)]
mod native_root;
mod native_secret;
mod native_sqlite;
mod native_temporary;
mod native_text;
mod native_udp;
#[cfg(unix)]
mod native_unix;
#[cfg(unix)]
mod native_watch;
mod object_elision;
mod ordering;
mod path_regex;
mod path_syntax;
mod plan;
#[cfg(unix)]
mod process_host;
mod range;
mod rational;
mod recurrence;
mod resource;
mod scalar;
mod semaphore;
mod signal;
mod stall;
pub mod stats;
mod stream;
mod testing;
mod text;
mod timeout;
pub use testing::{run_tests, run_tests_with_worker_command};
mod worker;
mod worker_process;
mod worker_wire;
use scalar::{FloatBinary, FloatUnary, Kind};
mod validate;
mod value;
pub use execution::{Outcome, Session};
pub use value::Integer;

use num_bigint::BigInt;
use okkama_sparrow::{Cmp, Constant, Instruction, Program, Target, Type};
use std::fmt;
use std::rc::Rc;
use value::{Arithmetic, Slot, Unary};

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Cold,
    Jit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// Immutable, validated execution plan. The bootstrap supports scalar control
/// flow in integer entry functions; other features fail before execution.
#[derive(Debug)]
pub struct Executable {
    code: Rc<Code>,
}

#[derive(Debug)]
pub(crate) struct Code {
    worker_command: Vec<std::ffi::OsString>,
    arguments: Vec<Vec<u8>>,
    functions: Vec<Option<Rc<FunctionPlan>>>,
    entry: usize,
    constants: Vec<Rc<BigInt>>,
    strings: Vec<Rc<str>>,
    enum_names: Rc<Vec<Vec<String>>>,
    objects: Vec<okkama_sparrow::ObjectDecl>,
    resource_descriptors: Rc<[bool]>,
    error_methods: Vec<Option<usize>>,
    native_descriptor: Option<usize>,
    error_role: Option<u32>,
    /// Task buffer sizes, see `execution::window`.
    window: (usize, usize, usize),
}

#[derive(Debug)]
struct FunctionPlan {
    native: Option<okkama_runtime_abi::Native>,
    is_async: bool,
    // Cold body entry; intrinsic factories may run a synchronous prefix.
    cold_entry: u32,
    cached: bool,
    symbol: String,
    recurrence_loop: bool,
    elided_objects: usize,
    params: Vec<usize>,
    plan: Vec<Op>,
    registers: usize,
    compiled: Vec<std::cell::OnceCell<jit::Compiled>>,
    jit_kinds: Option<Vec<Kind>>,
    live: Vec<Vec<usize>>,
    call_saved: Vec<Vec<usize>>,
    handlers: Vec<okkama_sparrow::Handler>,
}

#[derive(Debug)]
enum Op {
    Const(usize, Slot),
    AnyBox(usize, usize),
    Equal(okkama_sparrow::Cmp, usize, usize, usize),
    Trap(okkama_sparrow::TrapCode, String),
    Panic(usize),
    Text(text::TextOp, usize, [Option<usize>; 3]),
    RatPart(usize, usize, bool),
    RangeNew(usize, usize, usize, bool),
    RangePart(usize, usize, bool),
    BuilderRange(usize, usize, usize, usize, bool),
    ArrayNew(usize, Vec<usize>),
    MapNew(usize, Vec<(usize, usize)>),
    Map(map::MapOp, usize, [Option<usize>; 3]),
    Lookup {
        map: bool,
        array: usize,
        index: usize,
        value: usize,
        found: Target,
        missing: Target,
    },
    Array(array::ArrayOp, usize, [Option<usize>; 3]),
    Capture(Vec<(usize, usize)>),
    ObjectNew(usize, u32, Vec<usize>),
    ObjectGet(usize, usize, u32),
    ObjectInit(usize, u32, usize),
    Role(usize, usize, u32, Option<String>),
    CallCap {
        dst: usize,
        receiver: usize,
        function: usize,
        ty: u32,
    },
    CallRole {
        dst: Option<usize>,
        receiver: usize,
        role: u32,
        slot: usize,
        args: Vec<usize>,
        tail: bool,
    },
    ObjectNext(usize, usize, Vec<(u32, usize)>),
    Arithmetic(Arithmetic, usize, usize, usize),
    Compare(Cmp, usize, usize, usize),
    Unary(Unary, usize, usize),
    FloatBinary(FloatBinary, usize, usize, usize),
    RatNew(usize, usize, usize),
    RatBinary(rational::Binary, usize, usize, usize),
    RatUnary(rational::Unary, usize, usize),
    FloatUnary(FloatUnary, usize, usize),
    Convert(Kind, usize, usize),
    ByteFrom(usize, usize, Option<String>),
    ByteBinary(Arithmetic, usize, usize, usize),
    ByteNot(usize, usize),
    Bits(okkama_sparrow::BitOp, bool, usize, usize, Option<usize>),
    Copy(usize, usize),
    Return(usize),
    ReturnVoid,
    FutureControl(okkama_sparrow::FutOp, Option<usize>, usize),
    FutureSpawn(usize, usize),
    WorkerSpawn {
        dst: usize,
        entry: usize,
        args: Vec<usize>,
        direct: bool,
    },
    WorkerBind {
        dst: usize,
        action: usize,
        entry: usize,
        fields: Vec<usize>,
    },
    Intrinsic(intrinsic::Operation, usize, [Option<usize>; 3]),
    Await {
        dst: Option<usize>,
        future: usize,
    },
    Unreachable,
    Throw(usize),
    Raise(usize),
    FailureLink(usize, usize, usize, bool),
    RegionNew(usize),
    RegionRegister(usize, usize, bool),
    RegionTransfer(usize, usize, usize),
    RegionClose {
        region: usize,
        keep: Option<usize>,
        failure: Option<(usize, usize)>,
    },
    Catch {
        failure: usize,
        kind: okkama_sparrow::CatchKind,
        ty: u32,
        value: usize,
        found: Target,
        missing: Target,
    },
    Not(usize, usize),
    Jump(Target),
    Branch(usize, Target, Target),
    Closure {
        dst: usize,
        function: usize,
        captures: Vec<usize>,
    },
    CallFn {
        dst: Option<usize>,
        callee: usize,
        args: Vec<usize>,
        tail: bool,
    },
    Call {
        dst: Option<usize>,
        function: usize,
        args: Vec<usize>,
        tail: bool,
    },
}

impl Op {
    // Storage transfers can carry an unbound constructor field. Evaluation must
    // reject it, including native float arithmetic (whose payload alone is 0).
    fn required_bound(&self) -> [Option<usize>; 3] {
        match self {
            Self::Intrinsic(_, _, arguments) => *arguments,
            Self::WorkerSpawn { entry, .. } => [Some(*entry), None, None],
            Self::WorkerBind { action, entry, .. } => [Some(*action), Some(*entry), None],
            Self::RegionRegister(a, b, _) => [Some(*a), Some(*b), None],
            Self::RegionTransfer(a, b, c) => [Some(*a), Some(*b), Some(*c)],
            Self::RegionClose {
                region,
                keep,
                failure,
            } => [Some(*region), *keep, failure.map(|(_, s)| s)],
            Self::FutureControl(_, _, future) | Self::FutureSpawn(_, future) => {
                [Some(*future), None, None]
            }
            Self::Await { future, .. } => [Some(*future), None, None],
            Self::Role(_, src, _, _) => [Some(*src), None, None],
            Self::CallCap { receiver, .. } => [Some(*receiver), None, None],
            Self::CallRole { receiver, .. } => [Some(*receiver), None, None],
            Self::CallFn { callee, .. } => [Some(*callee), None, None],
            Self::FailureLink(_, a, b, _) => [Some(*a), Some(*b), None],
            Self::Throw(src) | Self::Raise(src) | Self::Catch { failure: src, .. } => {
                [Some(*src), None, None]
            }
            Self::AnyBox(_, _) => [None; 3],
            Self::Equal(_, _, a, b) => [Some(*a), Some(*b), None],
            Self::Panic(src) => [Some(*src), None, None],
            Self::Lookup { array, index, .. } => [Some(*array), Some(*index), None],
            Self::Text(_, _, args) | Self::Array(_, _, args) | Self::Map(_, _, args) => *args,
            Self::RangeNew(_, a, b, _) => [Some(*a), Some(*b), None],
            Self::BuilderRange(_, a, b, c, _) => [Some(*a), Some(*b), Some(*c)],
            Self::RatPart(_, value, _) | Self::RangePart(_, value, _) => [Some(*value), None, None],
            Self::Arithmetic(_, _, a, b)
            | Self::Compare(_, _, a, b)
            | Self::FloatBinary(_, _, a, b)
            | Self::RatNew(_, a, b)
            | Self::RatBinary(_, _, a, b)
            | Self::ByteBinary(_, _, a, b) => [Some(*a), Some(*b), None],
            Self::Unary(_, _, a)
            | Self::FloatUnary(_, _, a)
            | Self::RatUnary(_, _, a)
            | Self::Convert(_, _, a)
            | Self::ByteFrom(_, a, _)
            | Self::ByteNot(_, a)
            | Self::Not(_, a)
            | Self::Branch(a, _, _)
            | Self::ObjectGet(_, a, _)
            | Self::ObjectInit(a, _, _)
            | Self::ObjectNext(_, a, _) => [Some(*a), None, None],
            Self::Bits(_, _, _, a, b) => [Some(*a), *b, None],
            _ => [None; 3],
        }
    }
}

struct LoadCache {
    functions: Vec<Option<Rc<FunctionPlan>>>,
    constants: Vec<Rc<BigInt>>,
    strings: Vec<Rc<str>>,
}
impl LoadCache {
    fn new(program: &Program) -> Result<Self, Error> {
        program.verify().map_err(|e| Error(e.to_string()))?;
        native::validate_builtins(program)?;
        Ok(Self {
            functions: vec![None; program.functions.len()],
            constants: vec![],
            strings: vec![],
        })
    }
}

impl Executable {
    pub fn decode(bytes: &[u8], mode: Mode) -> Result<Self, Error> {
        let program = okkama_sparrow::decode(bytes).map_err(|e| Error(e.to_string()))?;
        Self::new(&program, mode)
    }

    pub fn new(program: &Program, mode: Mode) -> Result<Self, Error> {
        let entry = program
            .entry
            .ok_or_else(|| Error("SPB has no entry".into()))?;
        let entry = entry.0 as usize;
        let entry_function = program
            .functions
            .get(entry)
            .ok_or_else(|| Error("invalid entry function".into()))?;
        if !entry_function.params.is_empty()
            || entry_function.result.and_then(|t| program.ty(t)) != Some(&Type::Int)
        {
            return Err(Error("entry arguments are not supported yet".into()));
        }
        Self::load(program, mode, entry)
    }
    fn load(program: &Program, mode: Mode, entry: usize) -> Result<Self, Error> {
        Self::load_cached(program, mode, entry, &mut LoadCache::new(program)?)
    }
    fn load_cached(
        program: &Program,
        mode: Mode,
        entry: usize,
        cache: &mut LoadCache,
    ) -> Result<Self, Error> {
        let mut functions: Vec<Option<Rc<FunctionPlan>>> =
            (0..program.functions.len()).map(|_| None).collect();
        let error_methods = diagnostic::error_methods(program)?;
        let mut pending = vec![entry];
        while let Some(index) = pending.pop() {
            if functions[index].is_some() {
                continue;
            }
            let function = if let Some(function) = &cache.functions[index] {
                function.clone()
            } else {
                let function = Rc::new(Self::lower(
                    program,
                    index,
                    mode,
                    &mut cache.constants,
                    &mut cache.strings,
                )?);
                cache.functions[index] = Some(function.clone());
                function
            };
            if let Some(intrinsic) = &program.functions[index].intrinsic {
                for object in &intrinsic.objects {
                    if let Some(text) = error_methods[object.0 as usize] {
                        pending.push(text);
                    }
                }
            }
            for op in &function.plan {
                if let Op::ObjectNew(_, object, _) = op {
                    if let Some(text) = error_methods[*object as usize] {
                        pending.push(text);
                    }
                    if let Some(destroy) = program.objects[*object as usize].destroy {
                        pending.push(destroy.0 as usize);
                    }
                }
                if let Op::CallRole { role, slot, .. } = op {
                    if !validate::role_witnesses(program, *role, *slot) {
                        return Err(Error("invalid role witness in execution plan".into()));
                    }
                    for object in &program.objects {
                        for witness in &object.does {
                            if witness.role.0 == *role {
                                pending.push(witness.slots[*slot].0 as usize);
                            }
                        }
                    }
                }
                if let Op::Call { function, .. }
                | Op::Closure { function, .. }
                | Op::CallCap { function, .. } = op
                {
                    pending.push(*function);
                }
            }
            functions[index] = Some(function);
        }
        let window = execution::window(&functions);
        Ok(Self {
            code: Rc::new(Code {
                // Embedding must never implicitly relaunch the host executable.
                worker_command: Vec::new(),
                arguments: Vec::new(),
                window,
                functions,
                entry,
                constants: cache.constants.clone(),
                strings: cache.strings.clone(),
                enum_names: Rc::new(program.enums.iter().map(|e| e.cases.clone()).collect()),
                objects: program.objects.clone(),
                resource_descriptors: resource::descriptor_flags(program),
                error_methods,
                native_descriptor: program.objects.iter().position(|o| o.is_native_failure()),
                error_role: program
                    .roles
                    .iter()
                    .position(|r| r.is_error())
                    .map(|r| r as u32),
            }),
        })
    }

    fn lower(
        program: &Program,
        index: usize,
        mode: Mode,
        constants: &mut Vec<Rc<BigInt>>,
        strings: &mut Vec<Rc<str>>,
    ) -> Result<FunctionPlan, Error> {
        let function = &program.functions[index];
        if function.native.is_some() {
            return native::lower(program, index);
        }
        if function.intrinsic.is_some() {
            return intrinsic::lower(program, index);
        }
        if function.native.is_some()
            || function.intrinsic.is_some()
            || function.result.is_some_and(|t| kind(program, t).is_none())
            || function
                .registers
                .iter()
                .any(|t| kind(program, *t).is_none())
        {
            return Err(Error(format!(
                "{}: unsupported function signature or frame in new engine",
                function.symbol
            )));
        }
        let rewrites_safe = function.handlers.is_empty()
            && !function.code.iter().any(|i| {
                matches!(
                    i,
                    Instruction::RegionEnter { .. }
                        | Instruction::RegionLeave { .. }
                        | Instruction::RegionUnwind { .. }
                )
            })
            && !program.objects.iter().any(|o| o.destroy.is_some());
        let mut plan = Vec::with_capacity(function.code.len());
        for (pc, instruction) in function.code.iter().enumerate() {
            let unsupported = || {
                Error(format!(
                    "{}:{pc}: unsupported {} in new engine",
                    function.symbol,
                    instruction.mnemonic()
                ))
            };
            plan.push(match instruction {
                Instruction::WorkerSpawn {dst,entry,args,direct} => Op::WorkerSpawn{dst:dst.0.0 as usize,entry:entry.0 as usize,args:args.iter().map(|r|r.0 as usize).collect(),direct:*direct},
                Instruction::WorkerBind {dst,action,entry,fields} => Op::WorkerBind{dst:dst.0.0 as usize,action:action.0 as usize,entry:entry.0 as usize,fields:fields.iter().map(|r|r.0 as usize).collect()},
                Instruction::FutSpawn {dst,future}=>Op::FutureSpawn(dst.0.0 as usize,future.0 as usize),
                Instruction::Fut {op,dst,future}=>Op::FutureControl(*op,dst.map(|d|d.0.0 as usize),future.0 as usize),
                Instruction::Await {dst,future}=>Op::Await{dst:dst.map(|d|d.0.0 as usize),future:future.0 as usize},
                Instruction::Unreachable {} => Op::Unreachable,
                Instruction::RegionEnter {dst} => Op::RegionNew(dst.0.0 as usize),
                Instruction::RegionAdopt {region,value} | Instruction::RegionTrack {region,value} => Op::RegionRegister(region.0 as usize,value.0 as usize,matches!(instruction,Instruction::RegionAdopt{..})),
                Instruction::RegionTransfer {from,to,value} => Op::RegionTransfer(from.0 as usize,to.0 as usize,value.0 as usize),
                Instruction::RegionLeave {region,keep} => Op::RegionClose {region:region.0 as usize,keep:keep.map(|r|r.0 as usize),failure:None},
                Instruction::RegionUnwind {dst,region,failure} => Op::RegionClose {region:region.0 as usize,keep:None,failure:Some((dst.0.0 as usize,failure.0 as usize))},
                Instruction::Const { dst, value:Constant::Str(text) } => {
                    let index=strings.iter().position(|v| v.as_ref()==text).unwrap_or_else(|| { strings.push(Rc::from(text.as_str())); strings.len()-1 });
                    let handle=u32::try_from(index).map_err(|_| Error("too many string constants".into()))?;
                    Op::Const(dst.0.0 as usize, Slot { bits:u64::from(handle),tag:value::STR })
                },
                Instruction::RangeNew {dst,start,end,inclusive}=>Op::RangeNew(dst.0.0 as usize,start.0 as usize,end.0 as usize,*inclusive),
                Instruction::RangePart {dst,range,end}=>Op::RangePart(dst.0.0 as usize,range.0 as usize,*end),
                Instruction::BuilderRange {dst,builder,start,end,inclusive}=>Op::BuilderRange(dst.0.0 as usize,builder.0 as usize,start.0 as usize,end.0 as usize,*inclusive),
                Instruction::FailSecondary{dst,primary,cleanup}=>Op::FailureLink(dst.0.0 as usize,primary.0 as usize,cleanup.0 as usize,true),
                Instruction::FailCaused{dst,new,handled}=>Op::FailureLink(dst.0.0 as usize,new.0 as usize,handled.0 as usize,false),
                Instruction::Throw{value}=>Op::Throw(value.0 as usize),
                Instruction::Raise{failure}=>Op::Raise(failure.0 as usize),
                Instruction::Catch{failure,kind,ty,value,handler,next}=>Op::Catch{failure:failure.0 as usize,kind:*kind,ty:*ty,value:value.0.0 as usize,found:handler.clone(),missing:next.clone()},
                Instruction::Trap {code,message}=>Op::Trap(*code,message.clone()),
                Instruction::Panic {message}=>Op::Panic(message.0 as usize),
                Instruction::AnyBox{dst,source}=>Op::AnyBox(dst.0.0 as usize,source.0 as usize),
                Instruction::ValCmp{op,dst,left,right}=>Op::Equal(*op,dst.0.0 as usize,left.0 as usize,right.0 as usize),
                Instruction::EnumCase {dst,ty,case}=>Op::Const(dst.0.0 as usize,Slot{bits:(u64::from(ty.0)<<32)|u64::from(*case),tag:value::ENUM}),
                Instruction::EnumCmp {op,dst,left,right}=>Op::Compare(*op,dst.0.0 as usize,left.0 as usize,right.0 as usize),
                Instruction::MapNew {dst,entries}=>Op::MapNew(dst.0.0 as usize,entries.iter().map(|(k,v)|(k.0 as usize,v.0 as usize)).collect()),
                Instruction::MapLen {dst,map}=>Op::Map(map::MapOp::Length,dst.0.0 as usize,[Some(map.0 as usize),None,None]),
                Instruction::MapHas {dst,map,key}=>Op::Map(map::MapOp::Has,dst.0.0 as usize,[Some(map.0 as usize),Some(key.0 as usize),None]),
                Instruction::MapGet {dst,map,key}=>Op::Map(map::MapOp::Get,dst.0.0 as usize,[Some(map.0 as usize),Some(key.0 as usize),None]),
                Instruction::MapKeys {dst,map}=>Op::Map(map::MapOp::Keys,dst.0.0 as usize,[Some(map.0 as usize),None,None]),
                Instruction::MapValues {dst,map}=>Op::Map(map::MapOp::Values,dst.0.0 as usize,[Some(map.0 as usize),None,None]),
                Instruction::MapBuilderNew {dst}=>Op::Map(map::MapOp::BuilderNew,dst.0.0 as usize,[None;3]),
                Instruction::MapBuilderPut {dst,builder,key,value}=>Op::Map(map::MapOp::Put,dst.0.0 as usize,[Some(builder.0 as usize),Some(key.0 as usize),Some(value.0 as usize)]),
                Instruction::MapBuilderFinish {dst,builder}=>Op::Map(map::MapOp::Finish,dst.0.0 as usize,[Some(builder.0 as usize),None,None]),
                Instruction::MapLookup {map,key,value,found,missing} => Op::Lookup {map:true,array:map.0 as usize,index:key.0 as usize,value:value.0.0 as usize,found:found.clone(),missing:missing.clone()},
                Instruction::ArrayLookup {array,index,value,found,missing} => Op::Lookup {map:false,array:array.0 as usize,index:index.0 as usize,value:value.0.0 as usize,found:found.clone(),missing:missing.clone()},
                Instruction::ArrayNew {dst,elements}=>Op::ArrayNew(dst.0.0 as usize,elements.iter().map(|r|r.0 as usize).collect()),
                Instruction::ArrayLen {dst,array}=>Op::Array(array::ArrayOp::Length,dst.0.0 as usize,[Some(array.0 as usize),None,None]),
                Instruction::ArrayGet {dst,array,index}=>Op::Array(array::ArrayOp::Get,dst.0.0 as usize,[Some(array.0 as usize),Some(index.0 as usize),None]),
                Instruction::ArraySlice {dst,array,start,end}=>Op::Array(array::ArrayOp::Slice,dst.0.0 as usize,[Some(array.0 as usize),Some(start.0 as usize),Some(end.0 as usize)]),
                Instruction::ArrayJoin {dst,array,separator}=>Op::Array(array::ArrayOp::Join,dst.0.0 as usize,[Some(array.0 as usize),Some(separator.0 as usize),None]),
                Instruction::BuilderNew {dst}=>Op::Array(array::ArrayOp::BuilderNew,dst.0.0 as usize,[None;3]),
                Instruction::BuilderPush {dst,builder,value}=>Op::Array(array::ArrayOp::Push,dst.0.0 as usize,[Some(builder.0 as usize),Some(value.0 as usize),None]),
                Instruction::BuilderAppend {dst,builder,array}=>Op::Array(array::ArrayOp::Append,dst.0.0 as usize,[Some(builder.0 as usize),Some(array.0 as usize),None]),
                Instruction::BuilderFinish {dst,builder}=>Op::Array(array::ArrayOp::Finish,dst.0.0 as usize,[Some(builder.0 as usize),None,None]),
                Instruction::StrSplit {dst,text,separator}=>Op::Array(array::ArrayOp::Split,dst.0.0 as usize,[Some(text.0 as usize),Some(separator.0 as usize),None]),
                Instruction::StrUtf8 {dst,text}=>Op::Array(array::ArrayOp::Utf8,dst.0.0 as usize,[Some(text.0 as usize),None,None]),
                Instruction::StrConcat { dst,left,right } => Op::Text(text::TextOp::Concat,dst.0.0 as usize,[Some(left.0 as usize),Some(right.0 as usize),None]),
                Instruction::StrLen { dst,text } => Op::Text(text::TextOp::Length,dst.0.0 as usize,[Some(text.0 as usize),None,None]),
                Instruction::StrTrim { dst,text } => Op::Text(text::TextOp::Trim,dst.0.0 as usize,[Some(text.0 as usize),None,None]),
                Instruction::StrChomp { dst,text } => Op::Text(text::TextOp::Chomp,dst.0.0 as usize,[Some(text.0 as usize),None,None]),
                Instruction::StrSlice { dst,text,start,end } => Op::Text(text::TextOp::Slice,dst.0.0 as usize,[Some(text.0 as usize),Some(start.0 as usize),Some(end.0 as usize)]),
                Instruction::StrReplace { dst,text,pattern,replacement } => Op::Text(text::TextOp::Replace,dst.0.0 as usize,[Some(text.0 as usize),Some(pattern.0 as usize),Some(replacement.0 as usize)]),
                Instruction::StrReplaceFirst { dst,text,pattern,replacement } => Op::Text(text::TextOp::ReplaceFirst,dst.0.0 as usize,[Some(text.0 as usize),Some(pattern.0 as usize),Some(replacement.0 as usize)]),
                Instruction::StrCmp { op,dst,left,right } => Op::Compare(*op,dst.0.0 as usize,left.0 as usize,right.0 as usize),
                Instruction::RatPart { dst,value,denominator } => Op::RatPart(dst.0.0 as usize,value.0 as usize,*denominator),
                Instruction::Undef { dst } => Op::Const(dst.0.0 as usize, Slot { bits:0, tag:value::UNBOUND }),
                Instruction::ObjInit { object, field, value } => Op::ObjectInit(object.0 as usize, *field, value.0 as usize),
                Instruction::ObjNew { dst, object, fields } => {
                    let declaration = &program.objects[object.0 as usize];
                    if declaration.builtin {
                        return Err(unsupported());
                    }
                    Op::ObjectNew(dst.0.0 as usize, object.0, fields.iter().map(|r| r.0 as usize).collect())
                },
                Instruction::ObjNext { dst, object, updates } => Op::ObjectNext(dst.0.0 as usize, object.0 as usize, updates.iter().map(|(field, r)| (*field, r.0 as usize)).collect()),
                Instruction::ObjGet { dst, object, field } => Op::ObjectGet(dst.0.0 as usize, object.0 as usize, *field),
                Instruction::Const {
                    dst,
                    value: Constant::Int(value),
                } => {
                    let slot = match Integer::parse(value)? {
                        Integer::Small(value) => Slot::small(value),
                        Integer::Big(value) => {
                            let index = constants.iter().position(|v| *v == value).unwrap_or_else(|| { constants.push(value); constants.len() - 1 });
                            let index = u32::try_from(index).map_err(|_| Error("too many integer constants".into()))?;
                            Slot::big(u64::from(index))
                        }
                    };
                    Op::Const(dst.0.0 as usize, slot)
                },
                Instruction::Const {
                    dst,
                    value: Constant::Bool(value),
                } => Op::Const(dst.0.0 as usize, Slot::boolean(*value)),
                Instruction::IntAdd { dst, left, right } => Op::Arithmetic(Arithmetic::Add, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntSub { dst, left, right } => Op::Arithmetic(Arithmetic::Subtract, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntMul { dst, left, right } => Op::Arithmetic(Arithmetic::Multiply, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntMod { dst, left, right } => Op::Arithmetic(Arithmetic::Remainder, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntAnd { dst, left, right } => Op::Arithmetic(Arithmetic::And, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntOr { dst, left, right } => Op::Arithmetic(Arithmetic::Or, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntXor { dst, left, right } => Op::Arithmetic(Arithmetic::Xor, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntShl { dst, left, right } => Op::Arithmetic(Arithmetic::ShiftLeft, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntShr { dst, left, right } => Op::Arithmetic(Arithmetic::ShiftRight, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntUshr { dst, left, right } => Op::Arithmetic(Arithmetic::ShiftUnsigned, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntNeg { dst, operand } => Op::Unary(Unary::Negate, dst.0.0 as usize, operand.0 as usize),
                Instruction::IntAbs { dst, operand } => Op::Unary(Unary::Absolute, dst.0.0 as usize, operand.0 as usize),
                Instruction::IntNot { dst, operand } => Op::Unary(Unary::Not, dst.0.0 as usize, operand.0 as usize),
                Instruction::Sign { dst, operand } => Op::Unary(Unary::Sign, dst.0.0 as usize, operand.0 as usize),
                Instruction::ByteAnd { dst, left, right } => Op::ByteBinary(Arithmetic::And, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::ByteOr { dst, left, right } => Op::ByteBinary(Arithmetic::Or, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::ByteXor { dst, left, right } => Op::ByteBinary(Arithmetic::Xor, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::ByteShl { dst, left, right } => Op::ByteBinary(Arithmetic::ShiftLeft, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::ByteShr { dst, left, right } => Op::ByteBinary(Arithmetic::ShiftRight, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::ByteUshr { dst, left, right } => Op::ByteBinary(Arithmetic::ShiftUnsigned, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::ByteNot { dst, operand } => Op::ByteNot(dst.0.0 as usize, operand.0 as usize),
                Instruction::Bits { op, byte, dst, value, count } => Op::Bits(*op, *byte, dst.0.0 as usize, value.0 as usize, count.map(|r| r.0 as usize)),
                Instruction::RatAdd { dst, left, right } => Op::RatBinary(rational::Binary::Add, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::RatSub { dst, left, right } => Op::RatBinary(rational::Binary::Subtract, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::RatMul { dst, left, right } => Op::RatBinary(rational::Binary::Multiply, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::RatDiv { dst, left, right } => Op::RatBinary(rational::Binary::Divide, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::RatMod { dst, left, right } => Op::RatBinary(rational::Binary::Remainder, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::RatNew { dst, left, right } => Op::RatNew(dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::RatNeg { dst, operand } => Op::RatUnary(rational::Unary::Negate, dst.0.0 as usize, operand.0 as usize),
                Instruction::RatAbs { dst, operand } => Op::RatUnary(rational::Unary::Absolute, dst.0.0 as usize, operand.0 as usize),
                Instruction::RatCmp { op, dst, left, right } => Op::Compare(*op, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::IntCmp { op, dst, left, right } => Op::Compare(*op, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::Const { dst, value: Constant::Byte(value) } => Op::Const(dst.0.0 as usize, Slot { bits: u64::from(*value), tag: value::BYTE }),
                Instruction::Const { dst, value: Constant::Char(value) } => Op::Const(dst.0.0 as usize, Slot { bits: u64::from(u32::from(*value)), tag: value::CHAR }),
                Instruction::Const { dst, value: Constant::Float(bits) } => Op::Const(dst.0.0 as usize, scalar::finite(f64::from_bits(*bits))?),
                Instruction::FloatAdd { dst, left, right } => Op::FloatBinary(FloatBinary::Add, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::FloatSub { dst, left, right } => Op::FloatBinary(FloatBinary::Subtract, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::FloatMul { dst, left, right } => Op::FloatBinary(FloatBinary::Multiply, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::FloatDiv { dst, left, right } => Op::FloatBinary(FloatBinary::Divide, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::FloatMod { dst, left, right } => Op::FloatBinary(FloatBinary::Remainder, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::FloatPow { dst, left, right } => Op::FloatBinary(FloatBinary::Power, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::FloatNeg { dst, operand } => Op::FloatUnary(FloatUnary::Negate, dst.0.0 as usize, operand.0 as usize),
                Instruction::FloatAbs { dst, operand } => Op::FloatUnary(FloatUnary::Absolute, dst.0.0 as usize, operand.0 as usize),
                Instruction::FloatSqrt { dst, operand } => Op::FloatUnary(FloatUnary::Sqrt, dst.0.0 as usize, operand.0 as usize),
                Instruction::FloatCmp { op, dst, left, right } | Instruction::BoolCmp { op, dst, left, right } | Instruction::ByteCmp { op, dst, left, right } | Instruction::CharCmp { op, dst, left, right } => Op::Compare(*op, dst.0.0 as usize, left.0 as usize, right.0 as usize),
                Instruction::Convert { dst, source } => Op::Convert(kind(program, function.registers[dst.0.0 as usize]).ok_or_else(unsupported)?, dst.0.0 as usize, source.0 as usize),
                Instruction::ByteFrom { dst, source, context } => Op::ByteFrom(dst.0.0 as usize, source.0 as usize, context.clone()),
                Instruction::Move { dst, source } => Op::Copy(dst.0.0 as usize, source.0 as usize),
                Instruction::BoolNot { dst, operand } => {
                    Op::Not(dst.0.0 as usize, operand.0 as usize)
                }
                Instruction::Jump { target } => Op::Jump(target.clone()),
                Instruction::Branch {
                    condition,
                    then_target,
                    else_target,
                } => Op::Branch(
                    condition.0 as usize,
                    then_target.clone(),
                    else_target.clone(),
                ),
                Instruction::Return { value:None }=>Op::ReturnVoid,
                Instruction::Return { value: Some(value) } => Op::Return(value.0 as usize),
                Instruction::CallCap {dst,cap,ty,field}=>{
                    let function=program.caps[ty.0 as usize].fields.get(*field as usize).ok_or_else(unsupported)?.1.0 as usize;
                    Op::CallCap{dst:dst.0.0 as usize,receiver:cap.0 as usize,function,ty:ty.0}
                },
                Instruction::RoleWrap {dst,source,role}=>Op::Role(dst.0.0 as usize,source.0 as usize,role.0,None),
                Instruction::RoleNarrow {dst,source,role,message}=>Op::Role(dst.0.0 as usize,source.0 as usize,role.0,Some(message.clone())),
                Instruction::CallRole {dst,receiver,role,slot,args}=>{
                    let tail=rewrites_safe && matches!((dst,function.code.get(pc+1)),(Some(dst),Some(Instruction::Return{value:Some(value)})) if dst.0==*value);
                    Op::CallRole{dst:dst.map(|d|d.0.0 as usize),receiver:receiver.0 as usize,role:role.0,slot:*slot as usize,args:args.iter().map(|r|r.0 as usize).collect(),tail}
                },
                Instruction::Closure {dst,function,captures}=>Op::Closure{dst:dst.0.0 as usize,function:function.0 as usize,captures:captures.iter().map(|r|r.0 as usize).collect()},
                Instruction::CallFn {dst,callee,args}=>{
                    let tail=rewrites_safe && matches!((dst,function.code.get(pc+1)),(Some(dst),Some(Instruction::Return{value:Some(value)})) if dst.0==*value);
                    Op::CallFn{dst:dst.map(|d|d.0.0 as usize),callee:callee.0 as usize,args:args.iter().map(|r|r.0 as usize).collect(),tail}
                },
                Instruction::Call { dst, function: callee, args } => {
                    let tail = rewrites_safe && matches!((dst, function.code.get(pc + 1)), (Some(dst), Some(Instruction::Return { value: Some(value) })) if dst.0 == *value);
                    Op::Call { dst: dst.map(|d| d.0.0 as usize), function: callee.0 as usize, args: args.iter().map(|r| r.0 as usize).collect(), tail }
                }
            });
        }

        let params: Vec<_> = function.params.iter().map(|r| r.0 as usize).collect();
        let mut kinds: Vec<_> = function
            .registers
            .iter()
            .map(|t| kind(program, *t).expect("validated scalar type"))
            .collect();
        validate::function(program, index, &plan, &kinds)?;
        let elided_objects = if rewrites_safe {
            object_elision::lower(function, &mut plan, &mut kinds)
        } else {
            0
        };
        let mut registers = kinds.len();
        let recurrence_loop =
            rewrites_safe && recurrence::lower(&mut plan, index, &params, &mut registers);
        kinds.resize(registers, Kind::Int);
        let live = liveness::analyze(&plan, &function.handlers);
        let call_saved = liveness::call_saved(&plan, &live, &function.handlers);
        let jit_kinds = match mode {
            Mode::Cold => None,
            Mode::Jit => Some(kinds),
        };
        let compiled = if jit_kinds.is_some() {
            (0..plan.len().div_ceil(jit::REGION_SIZE))
                .map(|_| Default::default())
                .collect()
        } else {
            Vec::new()
        };
        Ok(FunctionPlan {
            native: None,
            is_async: function.is_async,
            cold_entry: 0,
            cached: false,
            symbol: function.symbol.clone(),
            elided_objects,
            call_saved,
            handlers: function.handlers.clone(),
            params,
            recurrence_loop,
            plan,
            registers,
            compiled,
            jit_kinds,
            live,
        })
    }

    pub fn start(&self) -> Session {
        Session::new(self.code.clone())
    }

    pub fn elided_object_count(&self) -> usize {
        self.code
            .functions
            .iter()
            .filter_map(Option::as_ref)
            .map(|f| f.elided_objects)
            .sum()
    }
    pub fn recurrence_loop_count(&self) -> usize {
        self.code
            .functions
            .iter()
            .filter_map(Option::as_ref)
            .filter(|f| f.recurrence_loop)
            .count()
    }

    pub fn run(&self) -> Result<Integer, Error> {
        // A synchronous native may acquire an I/O resource before the first
        // await. Install Tokio before executing any such program instruction.
        let needs_host = self.code.functions.iter().flatten().any(|f| {
            f.is_async
                || matches!(
                    f.native,
                    Some(okkama_runtime_abi::Native::ProcessProcessStateStart)
                )
        });
        if needs_host {
            return AsyncHost::new()?.run(self);
        }
        let mut session = self.start();
        loop {
            match session.resume(4096)? {
                Outcome::Returned(value) => return Ok(value),
                Outcome::Yielded => {}
                Outcome::Waiting => return AsyncHost::new()?.block_on(session.complete_async())?,
            }
        }
    }
}

impl Code {
    fn function(&self, index: usize) -> &FunctionPlan {
        self.functions[index]
            .as_ref()
            .expect("reachable function was lowered")
    }
}

fn kind(program: &Program, ty: okkama_sparrow::TypeRef) -> Option<Kind> {
    // Nested container types are acyclic in verified SPB, but can be very
    // deep. Validate supported leaves iteratively instead of using Rust stack.
    let mut pending = vec![ty];
    let mut visited = std::collections::HashSet::new();
    while let Some(leaf) = pending.pop() {
        if !visited.insert(leaf) {
            continue;
        }
        match program.ty(leaf)? {
            Type::Array(element) | Type::Builder(element) => pending.push(*element),
            Type::Future(element) => pending.extend(*element),
            Type::Fn { params, result } => {
                pending.extend(params.iter().copied());
                pending.extend(*result);
            }
            Type::Map(key, value) | Type::MapBuilder(key, value) => {
                if !matches!(
                    program.ty(*key)?,
                    Type::Never
                        | Type::Int
                        | Type::Bool
                        | Type::Byte
                        | Type::Char
                        | Type::Float
                        | Type::Str
                        | Type::Enum(_)
                ) {
                    return None;
                }
                pending.push(*value);
            }
            Type::Never
            | Type::Region
            | Type::Failure
            | Type::Any
            | Type::Int
            | Type::Bool
            | Type::Byte
            | Type::Char
            | Type::Float
            | Type::Range
            | Type::Rat
            | Type::Str
            | Type::Enum(_)
            | Type::Cap(_)
            | Type::Successor(_)
            | Type::Role(_)
            | Type::Handle(_)
            | Type::Object(_) => {}
        }
    }
    match program.ty(ty)? {
        Type::Fn { .. } => Some(Kind::Function(ty.0)),
        Type::Handle(handle) => Some(Kind::Handle(*handle)),
        Type::Any => Some(Kind::Any),
        Type::Future(element) => Some(Kind::Future(element.map(|t| t.0))),
        Type::Failure => Some(Kind::Failure),
        Type::Region => Some(Kind::Region),
        Type::Int => Some(Kind::Int),
        Type::Bool => Some(Kind::Bool),
        Type::Byte => Some(Kind::Byte),
        Type::Char => Some(Kind::Char),
        Type::Float => Some(Kind::Float),
        Type::Rat => Some(Kind::Rat),
        Type::Range => Some(Kind::Range),
        Type::Enum(e) => Some(Kind::Enum(e.0)),
        Type::Map(k, v) => Some(Kind::Map(k.0, v.0)),
        Type::MapBuilder(k, v) => Some(Kind::MapBuilder(k.0, v.0)),
        Type::Str => Some(Kind::Str),
        Type::Array(element) => Some(Kind::Array(element.0)),
        Type::Builder(element) => Some(Kind::Builder(element.0)),
        Type::Object(object) => Some(Kind::Object(object.0)),
        Type::Role(role) => Some(Kind::Role(role.0)),
        Type::Cap(cap) => Some(Kind::Cap(cap.0)),
        Type::Successor(object) => Some(Kind::Successor(object.0)),
        _ => None,
    }
}

/// Loader support for each ABI symbol. This is a coverage inventory, not a
/// claim that a provider has passed all semantic conformance cases.
pub fn native_capabilities() -> impl Iterator<Item = (&'static str, bool)> {
    okkama_runtime_abi::Native::ALL
        .iter()
        .map(|n| (n.symbol(), native::supported(*n)))
}
