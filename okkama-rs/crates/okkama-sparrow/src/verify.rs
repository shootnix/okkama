//! Проверка программы до исполнения: ссылки таблиц, регистры, переходы и
//! обработчики, завершённость кода, и что каждый читаемый регистр записан на
//! всех путях к чтению (прямой анализ потока данных над битовыми множествами).
use crate::{Access, Function, Instruction, Program, Register, Target, Type, VerifyError};

fn program_error(message: impl Into<String>) -> VerifyError {
    VerifyError {
        function: None,
        instruction: None,
        message: message.into(),
    }
}

struct Context<'a> {
    program: &'a Program,
    function: &'a Function,
}

impl Context<'_> {
    fn error(&self, instruction: Option<usize>, message: impl Into<String>) -> VerifyError {
        VerifyError {
            function: Some(self.function.symbol.clone()),
            instruction,
            message: message.into(),
        }
    }

    fn register(&self, pc: Option<usize>, register: Register) -> Result<(), VerifyError> {
        if register.0 as usize >= self.function.registers.len() {
            return Err(self.error(pc, format!("register r{} is out of bounds", register.0)));
        }
        Ok(())
    }

    fn target(&self, pc: Option<usize>, target: &Target) -> Result<(), VerifyError> {
        if target.pc as usize >= self.function.code.len() {
            return Err(self.error(pc, format!("jump target @{} is out of bounds", target.pc)));
        }
        let mut seen = std::collections::HashSet::new();
        for (dst, source) in &target.moves {
            self.register(pc, *dst)?;
            self.register(pc, *source)?;
            if !seen.insert(*dst) {
                return Err(self.error(pc, format!("register r{} is assigned twice by one transfer", dst.0)));
            }
        }
        Ok(())
    }
}

fn check_type_ref(program: &Program, index: u32, own: Option<usize>) -> Result<(), VerifyError> {
    let limit = own.unwrap_or(program.types.len());
    if index as usize >= limit {
        return Err(program_error(format!("type t{index} is out of bounds")));
    }
    Ok(())
}

fn check_types(program: &Program) -> Result<(), VerifyError> {
    for (index, ty) in program.types.iter().enumerate() {
        let earlier = |ty: crate::TypeRef| check_type_ref(program, ty.0, Some(index));
        match ty {
            Type::Object(object) | Type::Successor(object) => {
                if object.0 as usize >= program.objects.len() {
                    return Err(program_error(format!("type t{index} names missing object obj{}", object.0)));
                }
            }
            Type::Role(role) => {
                if role.0 as usize >= program.roles.len() {
                    return Err(program_error(format!("type t{index} names missing role role{}", role.0)));
                }
            }
            Type::Enum(id) => {
                if id.0 as usize >= program.enums.len() {
                    return Err(program_error(format!("type t{index} names missing enum enum{}", id.0)));
                }
            }
            Type::Handle(handle) => {
                if *handle as usize >= program.handles.len() {
                    return Err(program_error(format!("type t{index} names missing handle {handle}")));
                }
            }
            Type::Cap(cap) => {
                if cap.0 as usize >= program.caps.len() {
                    return Err(program_error(format!("type t{index} names missing cap cap{}", cap.0)));
                }
            }
            Type::Array(item) | Type::Builder(item) => earlier(*item)?,
            Type::Map(key, value) | Type::MapBuilder(key, value) => {
                earlier(*key)?;
                earlier(*value)?;
            }
            Type::Fn { params, result } => {
                for param in params {
                    earlier(*param)?;
                }
                if let Some(result) = result {
                    earlier(*result)?;
                }
            }
            Type::Future(Some(result)) => earlier(*result)?,
            _ => {}
        }
    }
    Ok(())
}

fn check_function_ref(program: &Program, function: u32) -> Result<(), VerifyError> {
    if function as usize >= program.functions.len() {
        return Err(program_error(format!("function f{function} is out of bounds")));
    }
    Ok(())
}

fn check_tables(program: &Program) -> Result<(), VerifyError> {
    check_types(program)?;
    for role in &program.roles {
        for method in &role.methods {
            for param in &method.params {
                check_type_ref(program, param.0, None)?;
            }
            if let Some(result) = method.result {
                check_type_ref(program, result.0, None)?;
            }
        }
    }
    for object in &program.objects {
        for (_, ty) in &object.fields {
            check_type_ref(program, ty.0, None)?;
        }
        for witness in &object.does {
            let Some(role) = program.roles.get(witness.role.0 as usize) else {
                return Err(program_error(format!("object {} does missing role role{}", object.name, witness.role.0)));
            };
            if witness.slots.len() != role.methods.len() {
                return Err(program_error(format!("object {} witness of {} has wrong slot count", object.name, role.name)));
            }
            for slot in &witness.slots {
                check_function_ref(program, slot.0)?;
            }
        }
        if let Some(destroy) = object.destroy {
            check_function_ref(program, destroy.0)?;
        }
    }
    for cap in &program.caps {
        if cap.owner.0 as usize >= program.objects.len() {
            return Err(program_error(format!("cap {} owner is missing", cap.name)));
        }
        for (_, function) in &cap.fields {
            check_function_ref(program, function.0)?;
        }
    }
    if let Some(entry) = program.entry {
        check_function_ref(program, entry.0)?;
    }
    for (_, function) in program.tests.iter().flatten() {
        check_function_ref(program, function.0)?;
    }
    Ok(())
}

/// Ссылки и регистры одной инструкции.
fn check_instruction(context: &Context<'_>, pc: usize, instruction: &Instruction) -> Result<(), VerifyError> {
    let program = context.program;
    let mut failure = None;
    instruction.visit(&mut |access| {
        if failure.is_some() {
            return;
        }
        let result = match access {
            Access::Use(register) | Access::Def(register) => context.register(Some(pc), register),
            Access::Target(target) => context.target(Some(pc), target),
            Access::Function(function) => check_function_ref(program, function.0).map_err(|e| context.error(Some(pc), e.message)),
            Access::Object(object) if object.0 as usize >= program.objects.len() => {
                Err(context.error(Some(pc), format!("object obj{} is out of bounds", object.0)))
            }
            Access::Role(role) if role.0 as usize >= program.roles.len() => {
                Err(context.error(Some(pc), format!("role role{} is out of bounds", role.0)))
            }
            Access::Enum(id) if id.0 as usize >= program.enums.len() => {
                Err(context.error(Some(pc), format!("enum enum{} is out of bounds", id.0)))
            }
            Access::Cap(cap) if cap.0 as usize >= program.caps.len() => {
                Err(context.error(Some(pc), format!("cap cap{} is out of bounds", cap.0)))
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            failure = Some(error);
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    match instruction {
        Instruction::EnumCase { ty, case, .. } => {
            if *case as usize >= program.enums[ty.0 as usize].cases.len() {
                return Err(context.error(Some(pc), format!("enum case {case} is out of bounds")));
            }
        }
        Instruction::CallRole { role, slot, .. } => {
            if *slot as usize >= program.roles[role.0 as usize].methods.len() {
                return Err(context.error(Some(pc), format!("role slot {slot} is out of bounds")));
            }
        }
        Instruction::CallCap { ty, field, .. } => {
            if *field as usize >= program.caps[ty.0 as usize].fields.len() {
                return Err(context.error(Some(pc), format!("cap field {field} is out of bounds")));
            }
        }
        Instruction::ObjNew { object, fields, .. } => {
            if fields.len() != program.objects[object.0 as usize].fields.len() {
                return Err(context.error(Some(pc), "obj.new field count does not match the object"));
            }
        }
        Instruction::Catch { kind, ty, .. } => {
            let limit = match kind {
                crate::CatchKind::Object => program.objects.len(),
                crate::CatchKind::Role => program.roles.len(),
            };
            if *ty as usize >= limit {
                return Err(context.error(Some(pc), format!("catch type {ty} is out of bounds")));
            }
        }
        Instruction::Bits { op, count, .. } => {
            let rotates = matches!(op, crate::BitOp::RotateLeft | crate::BitOp::RotateRight);
            if rotates != count.is_some() {
                return Err(context.error(Some(pc), "only rotations take a count"));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Битовое множество регистров.
#[derive(Clone, PartialEq, Eq)]
struct Set(Vec<u64>);

impl Set {
    fn empty(size: usize) -> Self {
        Set(vec![0; size.div_ceil(64)])
    }
    fn insert(&mut self, register: Register) {
        self.0[register.0 as usize / 64] |= 1 << (register.0 % 64);
    }
    fn contains(&self, register: Register) -> bool {
        self.0[register.0 as usize / 64] & (1 << (register.0 % 64)) != 0
    }
    /// Пересечение; `true`, если множество изменилось.
    fn intersect(&mut self, other: &Set) -> bool {
        let mut changed = false;
        for (word, other) in self.0.iter_mut().zip(&other.0) {
            let next = *word & other;
            changed |= next != *word;
            *word = next;
        }
        changed
    }
}

/// Рёбра инструкции: цель, присваивания и регистр значения ребра.
fn edges(pc: usize, instruction: &Instruction) -> Vec<(u32, &[(Register, Register)], Option<Register>)> {
    match instruction {
        Instruction::Jump { target } => vec![(target.pc, &target.moves[..], None)],
        Instruction::Branch {
            then_target,
            else_target,
            ..
        } => vec![
            (then_target.pc, &then_target.moves[..], None),
            (else_target.pc, &else_target.moves[..], None),
        ],
        Instruction::Catch { value, handler, next, .. } => vec![
            (handler.pc, &handler.moves[..], Some(value.0)),
            (next.pc, &next.moves[..], None),
        ],
        Instruction::ArrayLookup { value, found, missing, .. } | Instruction::MapLookup { value, found, missing, .. } => vec![
            (found.pc, &found.moves[..], Some(value.0)),
            (missing.pc, &missing.moves[..], None),
        ],
        instruction if instruction.is_terminator() => Vec::new(),
        _ => vec![(pc as u32 + 1, &[][..], None)],
    }
}

fn check_initialization(context: &Context<'_>) -> Result<(), VerifyError> {
    let function = context.function;
    let size = function.registers.len();
    let count = function.code.len();
    let mut state: Vec<Option<Set>> = vec![None; count];
    let mut entry = Set::empty(size);
    for param in &function.params {
        entry.insert(*param);
    }
    state[0] = Some(entry);
    let mut work = vec![0usize];
    let mut queued = vec![false; count];
    queued[0] = true;
    let merge = |state: &mut Vec<Option<Set>>, work: &mut Vec<usize>, queued: &mut Vec<bool>, pc: usize, incoming: Set| {
        let changed = match &mut state[pc] {
            None => {
                state[pc] = Some(incoming);
                true
            }
            Some(existing) => existing.intersect(&incoming),
        };
        if changed && !queued[pc] {
            queued[pc] = true;
            work.push(pc);
        }
    };
    while let Some(pc) = work.pop() {
        queued[pc] = false;
        let current = state[pc].clone().expect("reached");
        let instruction = &function.code[pc];
        // Чтения инструкции и источники её переходов — до записи.
        let mut defs = Vec::new();
        let mut unread = None;
        instruction.visit(&mut |access| match access {
            Access::Use(register) if !current.contains(register) && unread.is_none() => unread = Some(register),
            Access::Target(target) => {
                for (_, source) in &target.moves {
                    if !current.contains(*source) && unread.is_none() {
                        unread = Some(*source);
                    }
                }
            }
            Access::Def(register) => defs.push(register),
            _ => {}
        });
        if let Some(register) = unread {
            return Err(context.error(Some(pc), format!("register r{} may be read before it is written", register.0)));
        }
        let payload = match instruction {
            Instruction::Catch { value, .. } | Instruction::ArrayLookup { value, .. } | Instruction::MapLookup { value, .. } => Some(value.0),
            _ => None,
        };
        let mut out = current.clone();
        for register in defs {
            if Some(register) != payload {
                out.insert(register);
            }
        }
        for (target, moves, value) in edges(pc, instruction) {
            if target as usize >= count {
                return Err(context.error(Some(pc), "execution falls off the end of the code"));
            }
            let mut incoming = out.clone();
            for (dst, _) in moves {
                incoming.insert(*dst);
            }
            if let Some(value) = value {
                incoming.insert(value);
            }
            merge(&mut state, &mut work, &mut queued, target as usize, incoming);
        }
        if let Some(handler) = function.handler_at(pc as u32) {
            for (_, source) in &handler.target.moves {
                if !current.contains(*source) {
                    return Err(context.error(Some(pc), format!("handler reads r{} before it is written", source.0)));
                }
            }
            let mut incoming = current.clone();
            for (dst, _) in &handler.target.moves {
                incoming.insert(*dst);
            }
            incoming.insert(handler.failure);
            merge(&mut state, &mut work, &mut queued, handler.target.pc as usize, incoming);
        }
    }
    Ok(())
}

fn check_function(program: &Program, function: &Function) -> Result<(), VerifyError> {
    let context = Context { program, function };
    for ty in &function.registers {
        check_type_ref(program, ty.0, None).map_err(|e| context.error(None, e.message))?;
    }
    if let Some(result) = function.result {
        check_type_ref(program, result.0, None).map_err(|e| context.error(None, e.message))?;
    }
    let mut params = std::collections::HashSet::new();
    for param in &function.params {
        context.register(None, *param)?;
        if !params.insert(*param) {
            return Err(context.error(None, format!("parameter register r{} is repeated", param.0)));
        }
    }
    if let Some(intrinsic) = &function.intrinsic {
        for object in &intrinsic.objects {
            if object.0 as usize >= program.objects.len() {
                return Err(context.error(None, format!("intrinsic object obj{} is out of bounds", object.0)));
            }
        }
        for helper in &intrinsic.helpers {
            check_function_ref(program, helper.0).map_err(|e| context.error(None, e.message))?;
        }
    }
    let bodiless = function.native.is_some() || function.intrinsic.is_some();
    if function.native.is_some() && function.intrinsic.is_some() {
        return Err(context.error(None, "a function cannot be both native and intrinsic"));
    }
    if bodiless {
        if !function.code.is_empty() || !function.handlers.is_empty() {
            return Err(context.error(None, "native and intrinsic functions have no code"));
        }
        return Ok(());
    }
    if function.code.is_empty() {
        return Err(context.error(None, "function has no code"));
    }
    for (pc, instruction) in function.code.iter().enumerate() {
        check_instruction(&context, pc, instruction)?;
    }
    let mut previous_end = 0;
    for handler in &function.handlers {
        if handler.start >= handler.end || handler.end as usize > function.code.len() {
            return Err(context.error(None, format!("handler {}..{} is out of bounds", handler.start, handler.end)));
        }
        if handler.start < previous_end {
            return Err(context.error(None, "handlers must be sorted and disjoint"));
        }
        previous_end = handler.end;
        context.target(None, &handler.target)?;
        context.register(None, handler.failure)?;
    }
    check_initialization(&context)
}

pub(crate) fn verify(program: &Program) -> Result<(), VerifyError> {
    check_tables(program)?;
    for function in &program.functions {
        check_function(program, function)?;
    }
    Ok(())
}
