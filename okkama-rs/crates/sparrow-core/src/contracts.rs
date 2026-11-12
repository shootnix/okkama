//! Load-time contracts of intrinsics: what the core assumes about the SPB
//! signature of each intrinsic function and about the objects it names. The
//! SPB verifier (`okkama_sparrow::Program::verify`) establishes the types of
//! instructions, calls, natives and runtime tables; intrinsics are typed by
//! the runtime that provides them, so their shapes are checked here before
//! anything runs. Each rule names the access in the core it makes safe.
use crate::program::{Kind, Loaded, WORKER};
use crate::Error;
use crate::intrinsics::Intrinsic;
use okkama_sparrow::{Function, Instruction, Program, Type, TypeRef};

/// Parameter and result codes: `I` int, `F` float, `R` raw (bool, char,
/// enum), `B` byte, `P` reference, `*` any kind, `-` void (results only).
/// Receivers (parameter 0) with a layout the core reads directly:
/// `A` an Atomic cell (`[value, closed]`, `jit::atomic`, `intrinsics`),
/// `E` an Event cell (`[initial, set, waiters]`, `jit::atomic`),
/// `N` an object whose field 0 is `int` (`Runtime::pool_of`, `barrier_of`),
/// `S` an object whose field 0 is a role with `size() int` and
///    `item(int) future` (`drivers::collect_start`, `intrinsics::stream_init`),
/// `T` a timeout (`[origin future, limit, clock]`, `drivers::timeout_start`).
/// `V`: the value of the Atomic cell of parameter 0.
struct Contract {
    asynchronous: bool,
    params: &'static str,
    result: char,
}

/// What an intrinsic does with its related object `index`.
enum Related {
    /// `object_new` with these field kinds; the result is that object.
    New(&'static str),
    /// A thrown error from these payload kinds (`intrinsics::raised`).
    Thrown(&'static str),
    /// The Atomic or Event cell it creates.
    Cell,
}

fn contract(intrinsic: Intrinsic) -> Contract {
    use Intrinsic::*;
    let (asynchronous, params, result) = match intrinsic {
        Worker => (true, "", '*'),
        Ordering => (false, "P", 'P'),
        AtomicNew => (false, "*", 'P'),
        AtomicValue => (false, "A", 'V'),
        AtomicReplace => (false, "AV", '-'),
        AtomicCompareReplace => (false, "AVV", 'R'),
        AtomicClosed => (false, "A", 'R'),
        AtomicClose => (false, "A", '-'),
        CancellationNew | TokenNew | LockNew | GroupNew | EventNew => (false, "", 'P'),
        Cancelling | ChannelEndpoint => (false, "P", 'P'),
        EndpointNew => (false, "*", 'P'),
        Cancel | EventSet | GrantClose | EndpointClose | GroupCancel | FakeClear | TestFail
        | TestSkip => (false, "P", '-'),
        TokenRequested | LockLocked | GrantClosed => (false, "P", 'R'),
        TokenCheck => (false, "P", '-'),
        TokenWait | EventWait | LockGuard | GroupWait => (true, "P", 'P'),
        EventNewReady => (false, "R", 'P'),
        EventReady => (false, "E", 'R'),
        SemaphorePermits => (false, "N", 'I'),
        SemaphorePermit => (true, "N", 'P'),
        BarrierWait => (true, "N", 'I'),
        BarrierState => (false, "N", 'P'),
        BarrierAbort | BarrierReset => (false, "N", '-'),
        BarrierRead(3) => (false, "P", 'R'),
        BarrierRead(_) => (false, "P", 'I'),
        ChannelNew => (false, "I", 'P'),
        Send => (true, "P*", 'P'),
        Receive | ScopeResult => (true, "P", '*'),
        GroupStart => (false, "PP", '-'),
        GroupActive | FakeSize => (false, "P", 'I'),
        CollectAll => (true, "S", 'P'),
        CollectRace | CollectFirst | StreamCompletion => (true, "S", '*'),
        StreamRemaining => (false, "S", 'I'),
        TimeoutBefore | TimeoutWithin => (true, "T", '*'),
        FakeSet => (false, "P**", '-'),
        FakeGet => (false, "P*", '*'),
        FakeHas => (false, "P*", 'R'),
        FakeRemove => (false, "P*", '-'),
        TestOk => (false, "R", '-'),
        TestIs | TestIsnt => (false, "**", '-'),
        TestClose => (false, "FFF", '-'),
        FloatFunction(_) => (false, "F", 'F'),
        FloatAtan2 | FloatMin | FloatMax => (false, "FF", 'F'),
        FloatClamp => (false, "FFF", 'F'),
        FloatFinite | FloatNan => (false, "F", 'R'),
        IntMin | IntMax => (false, "II", 'I'),
        IntClamp => (false, "III", 'I'),
        TestText | TestBytes => (false, "PP", '-'),
    };
    Contract { asynchronous, params, result }
}

fn related(intrinsic: Intrinsic) -> &'static [Related] {
    use Intrinsic::*;
    use Related::*;
    match intrinsic {
        AtomicNew | EventNew | EventNewReady => &[Cell],
        AtomicValue | AtomicReplace | AtomicCompareReplace => &[Thrown("")],
        CancellationNew | TokenNew | Cancelling | LockNew | LockGuard | SemaphorePermit | BarrierState
        | ChannelEndpoint | EndpointNew | GroupNew | ScopeResult => &[New("")],
        ChannelNew => &[New(""), Thrown("I")],
        CollectAll => &[New("P")],
        BarrierWait | Send | Receive | GroupStart | CollectRace | CollectFirst | StreamCompletion | TimeoutBefore
        | TimeoutWithin => &[Thrown("")],
        _ => &[],
    }
}

fn code(kind: Kind) -> char {
    match kind {
        Kind::Int => 'I',
        Kind::Float => 'F',
        Kind::Raw => 'R',
        Kind::Byte => 'B',
        Kind::Ref => 'P',
    }
}

struct Check<'a> {
    loaded: &'a Loaded,
    program: &'a Program,
}

impl Check<'_> {
    fn kind(&self, ty: TypeRef) -> char {
        code(Kind::of(self.program, ty))
    }

    fn object(&self, ty: TypeRef) -> Option<usize> {
        match self.program.ty(ty) {
            Some(Type::Object(object)) => Some(object.0 as usize),
            _ => None,
        }
    }

    fn fields(&self, object: usize) -> String {
        self.program.objects[object].fields.iter().map(|(_, ty)| self.kind(*ty)).collect()
    }

    /// The value type of an Atomic cell: the parameter of its constructors,
    /// which must agree (`Loaded::atomic_value` takes the first).
    fn atomic(&self, object: usize) -> Option<TypeRef> {
        self.loaded.atomic_value(object)?;
        let p = self.program;
        let mut value = None;
        for f in &p.functions {
            let Some(intrinsic) = &f.intrinsic else { continue };
            if intrinsic.name == "okkama.atomic.Atomic.new_value" && intrinsic.objects.first().is_some_and(|o| o.0 as usize == object) {
                let ty = *f.registers.get(f.params.first()?.0 as usize)?;
                match value {
                    None => value = Some(ty),
                    Some(other) if p.same_type(other, ty) => {}
                    Some(_) => return None,
                }
            }
        }
        value
    }

    fn future(&self, ty: TypeRef) -> Option<Option<TypeRef>> {
        match self.program.ty(ty) {
            Some(Type::Future(result)) => Some(*result),
            _ => None,
        }
    }

    /// `S`: field 0 a role whose `size` and `item` the core calls.
    fn sequence(&self, object: usize) -> Option<Option<TypeRef>> {
        let (_, origins) = self.program.objects[object].fields.first()?;
        let Some(Type::Role(role)) = self.program.ty(*origins) else { return None };
        let methods = &self.program.roles[role.0 as usize].methods;
        let size = methods.iter().find(|m| m.name == "size")?;
        let item = methods.iter().find(|m| m.name == "item")?;
        let int = |ty: &TypeRef| matches!(self.program.ty(*ty), Some(Type::Int));
        if !size.params.is_empty() || !size.result.as_ref().is_some_and(int) || item.params.len() != 1 || !int(&item.params[0]) {
            return None;
        }
        self.future(item.result?)
    }

    /// `T`: the origin future, then helpers over `limit` and `clock`.
    fn timeout(&self, function: &Function, object: usize, before: bool) -> Result<Option<TypeRef>, String> {
        let p = self.program;
        let fields = &p.objects[object].fields;
        let [(_, origin), (_, limit), (_, clock), ..] = fields[..] else {
            return Err("timeout receiver needs origin, limit and clock".into());
        };
        let result = self.future(origin).ok_or("timeout origin is not a future")?;
        let helpers = &function.intrinsic.as_ref().expect("intrinsic").helpers;
        let signature = |index: usize| -> Option<(Vec<TypeRef>, Option<TypeRef>)> {
            let f = p.functions.get(helpers.get(index)?.0 as usize)?;
            if f.is_async || f.native.is_some() || f.intrinsic.is_some() {
                return None;
            }
            Some((f.params.iter().map(|r| f.registers[r.0 as usize]).collect(), f.result))
        };
        let takes = |index: usize, params: &[TypeRef]| -> Option<Option<TypeRef>> {
            let (actual, result) = signature(index)?;
            (actual.len() == params.len() && actual.iter().zip(params).all(|(a, b)| p.same_type(*a, *b))).then_some(result)
        };
        let rat = |result: Option<Option<TypeRef>>| matches!(result.flatten().and_then(|r| p.ty(r)), Some(Type::Rat));
        let timer = |result: Option<Option<TypeRef>>| result.flatten().and_then(|r| self.future(r)).is_some();
        let ok = if before {
            // now = h0(clock); left = h1(deadline, now); h2(left) rat; h3(clock, left) timer
            let now = takes(0, &[clock]).flatten();
            let left = now.and_then(|now| takes(1, &[limit, now])).flatten();
            helpers.len() == 4
                && left.is_some_and(|left| rat(takes(2, &[left])) && timer(takes(3, &[clock, left])))
        } else {
            helpers.len() == 2 && rat(takes(0, &[limit])) && timer(takes(1, &[clock, limit]))
        };
        if ok { Ok(result) } else { Err("timeout helpers do not match their calls".into()) }
    }

    /// A thrown related object: an error enum (`$tag`, then payload arrays,
    /// the first case's named `$0_…`) or an object of exactly these fields.
    fn thrown(&self, object: usize, payload: &str) -> bool {
        let fields = &self.program.objects[object].fields;
        match fields.first() {
            Some((name, tag)) if name == "$tag" => {
                let mut first = fields[1..].iter().filter(|(name, _)| name.starts_with("$0_"));
                let arrays = fields[1..].iter().all(|(_, ty)| matches!(self.program.ty(*ty), Some(Type::Array(_))));
                let cases = payload.chars().all(|kind| {
                    first.next().is_some_and(|(_, ty)| match self.program.ty(*ty) {
                        Some(Type::Array(element)) => self.kind(*element) == kind,
                        _ => false,
                    })
                });
                self.kind(*tag) == 'I' && arrays && cases
            }
            _ => self.fields(object) == payload,
        }
    }

    fn function(&self, function: &Function) -> Result<(), String> {
        let Some(declared) = &function.intrinsic else { return Ok(()) };
        if declared.name == WORKER {
            return Ok(());
        }
        let Some(intrinsic) = Intrinsic::resolve(&declared.name) else { return Ok(()) };
        let contract = contract(intrinsic);
        if function.is_async != contract.asynchronous {
            return Err(format!("is {}async", if function.is_async { "" } else { "not " }));
        }
        let params: Vec<TypeRef> = function.params.iter().map(|r| function.registers[r.0 as usize]).collect();
        if params.len() != contract.params.len() {
            return Err(format!("takes {} parameters, the runtime passes {}", params.len(), contract.params.len()));
        }
        // The value the receiver holds: an Atomic's value or a future's result.
        let mut value: Option<Option<TypeRef>> = None;
        for (index, (expected, ty)) in contract.params.chars().zip(&params).enumerate() {
            let object = self.object(*ty);
            let ok = match expected {
                '*' => true,
                // Without a constructor no cell of it can exist (`cells`
                // forbids `obj.new` of the receiver below).
                'A' => match object {
                    Some(o) if self.loaded.atomic_value(o).is_none() && !self.constructed(o) => {
                        value = Some(None);
                        true
                    }
                    Some(o) => match self.atomic(o) {
                        Some(cell) => {
                            value = Some(Some(cell));
                            true
                        }
                        None => false,
                    },
                    None => false,
                },
                'E' => object.is_some_and(|o| self.loaded.is_event(o)),
                'N' => object.is_some_and(|o| self.fields(o).starts_with('I')),
                'S' => match object.and_then(|o| self.sequence(o)) {
                    Some(result) => {
                        value = Some(result);
                        true
                    }
                    None => false,
                },
                'T' => {
                    let object = object.ok_or("timeout receiver is not an object")?;
                    value = Some(self.timeout(function, object, intrinsic == Intrinsic::TimeoutBefore)?);
                    true
                }
                'V' => match value {
                    Some(Some(cell)) => self.program.same_type(cell, *ty),
                    Some(None) => true,
                    None => false,
                },
                kind => self.kind(*ty) == kind,
            };
            if !ok {
                return Err(format!("parameter {index} does not match the runtime's `{expected}`"));
            }
        }
        let result = function.result;
        let ok = match contract.result {
            '-' => result.is_none(),
            '*' => true,
            'V' => match (value, result) {
                (Some(Some(cell)), Some(r)) => self.program.same_type(cell, r),
                (Some(None), Some(_)) => true,
                _ => false,
            },
            kind => result.is_some_and(|r| self.kind(r) == kind),
        };
        // A raced or timed value is the result of the futures it waits for.
        let ok = ok
            && match (intrinsic, value) {
                (Intrinsic::CollectRace | Intrinsic::CollectFirst | Intrinsic::TimeoutBefore | Intrinsic::TimeoutWithin, Some(waited)) => {
                    match (waited, result) {
                        // By type, not representation: a string is not an object (#69).
                        (Some(w), Some(r)) => self.program.same_type(w, r),
                        (None, None) => true,
                        _ => false,
                    }
                }
                _ => true,
            };
        if !ok {
            return Err(format!("result does not match the runtime's `{}`", contract.result));
        }
        if intrinsic == Intrinsic::GroupStart && self.future(params[1]).is_none() {
            return Err("starts a value that is not a future".into());
        }
        if intrinsic == Intrinsic::ScopeResult {
            self.scope_action(params[0], declared.objects.first().map(|o| o.0 as usize), result)?;
        }
        let objects = &declared.objects;
        let rules = related(intrinsic);
        if objects.len() < rules.len() {
            return Err(format!("names {} objects, the runtime uses {}", objects.len(), rules.len()));
        }
        for (index, (rule, object)) in rules.iter().zip(objects).enumerate() {
            let object = object.0 as usize;
            // The result is the object, or a role reference to it.
            let made = || match result.and_then(|r| self.program.ty(r)) {
                Some(Type::Object(o)) => o.0 as usize == object,
                Some(Type::Role(role)) => self.program.objects[object].does.iter().any(|w| w.role == *role),
                _ => false,
            };
            let ok = match rule {
                Related::New(fields) => {
                    let fields = match intrinsic {
                        // `[results]`: an array of the waited futures' results.
                        Intrinsic::CollectAll => self.collected(object, value.flatten()),
                        _ => self.fields(object) == *fields,
                    };
                    // ScopeResult's group is internal; the others return the object.
                    fields && (intrinsic == Intrinsic::ScopeResult || made())
                }
                Related::Thrown(payload) => self.thrown(object, payload),
                Related::Cell => {
                    let cell = match intrinsic {
                        Intrinsic::AtomicNew => {
                            !self.loaded.is_event(object)
                                && self.atomic(object).is_some_and(|cell| self.program.same_type(cell, params[0]))
                        }
                        _ => self.loaded.atomic_value(object).is_none(),
                    };
                    cell && made()
                }
            };
            if !ok {
                return Err(format!("related object {index} does not match the runtime's use"));
            }
        }
        Ok(())
    }

    /// ScopeResult calls field 0 of its receiver with the group it makes and
    /// returns what that call's future yields: the field must be an async
    /// action taking the group and yielding the intrinsic's result type (#69).
    fn scope_action(&self, receiver: TypeRef, group: Option<usize>, result: Option<TypeRef>) -> Result<(), String> {
        let p = self.program;
        let object = self.object(receiver).ok_or("scope receiver is not an object")?;
        let group = group.ok_or("scope names no group object")?;
        let action = p.objects[object].fields.first().map(|(_, ty)| *ty).ok_or("scope receiver has no action")?;
        let Some(Type::Fn { params, result: Some(future) }) = p.ty(action) else {
            return Err("scope action is not an action returning a future".into());
        };
        let takes_group = match params.as_slice() {
            [param] => match p.ty(*param) {
                Some(Type::Object(o)) => o.0 as usize == group,
                Some(Type::Role(role)) => p.objects[group].does.iter().any(|w| w.role == *role),
                _ => false,
            },
            _ => false,
        };
        let yields = match (self.future(*future), result) {
            (Some(Some(value)), Some(result)) => p.same_type(value, result),
            (Some(None), None) => true,
            _ => false,
        };
        if takes_group && yields { Ok(()) } else { Err("scope action does not take the group or yield the result".into()) }
    }

    /// Channel payloads: the endpoints a channel hands out, and what `send`
    /// takes and `item` returns through them, carry one type (#69). The core
    /// keeps a channel's state on its object and on the endpoints
    /// `sending`/`receiving` make from it, so the object types linked by
    /// those calls form a class, and every `send` value and `item` result of
    /// a class must be the same type — a crafted program cannot pair
    /// `Sending<string>` with `Receiving<SomeObject>`.
    fn channels(&self) -> Result<(), String> {
        let p = self.program;
        let mut parent: Vec<usize> = (0..p.objects.len()).collect();
        let mut payload: Vec<Option<TypeRef>> = vec![None; p.objects.len()];
        fn root(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        let agree = |a: Option<TypeRef>, b: Option<TypeRef>| -> Result<Option<TypeRef>, String> {
            match (a, b) {
                (Some(a), Some(b)) if !p.same_type(a, b) => Err("channel endpoints carry different payload types".into()),
                (a, b) => Ok(a.or(b)),
            }
        };
        // Objects a value of `ty` can be: an object, or every object doing a role.
        let objects = |ty: TypeRef| -> Vec<usize> {
            match p.ty(ty) {
                Some(Type::Object(o)) => vec![o.0 as usize],
                Some(Type::Role(role)) => {
                    (0..p.objects.len()).filter(|&o| p.objects[o].does.iter().any(|w| w.role == *role)).collect()
                }
                _ => Vec::new(),
            }
        };
        for function in &p.functions {
            let Some(intrinsic) = function.intrinsic.as_ref().and_then(|i| Intrinsic::resolve(&i.name)) else { continue };
            let params: Vec<TypeRef> = function.params.iter().map(|r| function.registers[r.0 as usize]).collect();
            let (links, value): (Vec<usize>, Option<TypeRef>) = match intrinsic {
                Intrinsic::ChannelEndpoint => {
                    let mut linked = objects(params[0]);
                    linked.extend(function.result.map(objects).unwrap_or_default());
                    (linked, None)
                }
                Intrinsic::Send => (objects(params[0]), params.get(1).copied()),
                Intrinsic::Receive => (objects(params[0]), function.result),
                _ => continue,
            };
            let Some((&first, rest)) = links.split_first() else { continue };
            let mut at = root(&mut parent, first);
            payload[at] = agree(payload[at], value)?;
            for &other in rest {
                let other = root(&mut parent, other);
                if other != at {
                    let merged = agree(payload[at], payload[other])?;
                    parent[other] = at;
                    payload[at] = merged;
                    at = root(&mut parent, at);
                }
            }
        }
        Ok(())
    }

    /// CollectAll's result object: one array of the futures' results.
    fn collected(&self, object: usize, waited: Option<TypeRef>) -> bool {
        match (&self.program.objects[object].fields[..], waited) {
            ([(_, items)], Some(waited)) => match self.program.ty(*items) {
                Some(Type::Array(element)) => self.program.same_type(*element, waited),
                _ => false,
            },
            _ => false,
        }
    }

    /// Does any `obj.new` construct `object`?
    fn constructed(&self, object: usize) -> bool {
        self.program.functions.iter().flat_map(|f| &f.code).any(|i| {
            matches!(i, Instruction::ObjNew { object: o, .. } if o.0 as usize == object)
        })
    }

    /// Cells are laid out by the runtime, not by their declared fields: the
    /// program never constructs or reads them as plain objects.
    fn cells(&self, function: &Function) -> Result<(), String> {
        let cell = |object: usize| self.loaded.atomic_value(object).is_some() || self.loaded.is_event(object);
        let of = |register: &okkama_sparrow::Register| self.object(function.registers[register.0 as usize]);
        for (pc, instruction) in function.code.iter().enumerate() {
            let touched = match instruction {
                Instruction::ObjNew { object, .. } => Some(object.0 as usize),
                Instruction::ObjGet { object, .. } | Instruction::ObjInit { object, .. } | Instruction::ObjNext { object, .. } => of(object),
                _ => None,
            };
            if touched.is_some_and(cell) {
                return Err(format!("instruction {pc} accesses a runtime cell as a plain object"));
            }
        }
        Ok(())
    }
}

/// Check every intrinsic function and cell use of `loaded`.
pub fn check(loaded: &Loaded) -> Result<(), Error> {
    // The cases of a variant are consecutive objects: the JIT reads a case
    // number as the difference of descriptors (#290).
    for variant in &loaded.program.variants {
        let first = variant.cases[0].1.0;
        if variant.cases.iter().enumerate().any(|(case, (_, object))| object.0 != first + case as u32) {
            return Err(Error(format!("Sparrow: variant {} has cases that are not consecutive objects", variant.name)));
        }
    }
    let check = Check { loaded, program: &loaded.program };
    check.channels().map_err(|message| Error(format!("Sparrow: intrinsic contract: {message}")))?;
    for function in &loaded.program.functions {
        check
            .function(function)
            .and_then(|()| check.cells(function))
            .map_err(|message| Error(format!("Sparrow @{}: intrinsic contract: {message}", function.symbol)))?;
    }
    Ok(())
}
