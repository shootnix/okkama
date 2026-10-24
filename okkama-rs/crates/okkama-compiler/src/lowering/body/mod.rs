//! Lowering тела функции из THIR: SSA-переменные, слияние ветвей, области.
//!
//! Анализ уже разрешил имена, проверил типы, выбрал перегрузки и проверил
//! обязательства successor; здесь — только представление. Каждая
//! конструкция даёт [`Out`]: значение, отсутствие значения или уход
//! управления (`return`, `panic`, бесконечный `loop`).
//!
//! Переменные, которые могут меняться (rebind, поля строящегося объекта,
//! staged-поля successor), хранятся как текущее SSA-значение. В точке
//! слияния различающиеся значения передаются параметрами блока; переменные,
//! присваиваемые в теле цикла, становятся параметрами его заголовка. Значение
//! `^C` на слиянии гасится (`succ.take`) в каждой ветви: обязательства уже
//! проверены анализом, и тип параметра слияния — `C`.

mod adapters;
mod calls;
mod collections;
mod matching;
mod scopes;

use std::collections::HashMap;

use super::builder::FnBuilder;
use super::ctx::{Ctx, Task};
use super::intrinsics::Member;
use super::{LoweringError, Result};
use crate::analysis::thir::{self, BodyOwner, Coercion, ExprKind, LocalId, StmtKind};
use crate::analysis::ty::{ParamId, Ty};
use crate::ir::{
    self, BlockId, Constant, FuncId, InstKind, IntValue, ObjectId, ResultType, Target, TermKind,
    TrapCode, Type, ValueId,
};
use crate::source::Span;

/// Значение и его тип OIR.
#[derive(Clone, Debug, PartialEq)]
struct Typed {
    value: ValueId,
    ty: Type,
}

/// Итог конструкции.
#[derive(Clone, Debug)]
enum Out {
    Value(Typed),
    /// Конструкция завершилась без значения.
    Unit,
    /// Управление не возвращается в текущую позицию.
    Diverged,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum VarKind {
    Local,
    /// Поле строящегося объекта (конструктор).
    Field,
    /// Staged-поле будущего successor с индексом поля.
    Staged(usize),
}

#[derive(Clone, Debug)]
struct Var {
    /// `None` — поле ещё не связано или не staged.
    value: Option<Typed>,
    rebindable: bool,
    kind: VarKind,
    /// Область объявления: rebind принимает значение в её регион.
    scope: usize,
}

/// Незавершённая ветвь: блок, итог и значения переменных в её конце.
struct Arm {
    end: BlockId,
    out: Out,
    env: Vec<Option<Typed>>,
}

struct LoopFrame {
    /// Цель `next` и переменные, передаваемые ей параметрами.
    next: BlockId,
    /// Состояние обхода на входе в итерацию: `next` передаёт его неизменным
    /// (значение `for` — тело последней завершённой итерации).
    state: Vec<ValueId>,
    carried: Vec<usize>,
    exits: Vec<Arm>,
    /// Число переменных при входе в цикл: они сводятся на выходе.
    base: usize,
    /// Глубина областей при входе в тело: глубже — локальные для итерации.
    depth: usize,
    want: bool,
}

/// Активная область отказа: тело `try`, тело выбранного `catch` или цель
/// отказа области с регионом/defer (`scopes`).
#[derive(Clone)]
struct TryCtx {
    target: BlockId,
    tracked: Vec<usize>,
    /// В теле `catch`: обрабатываемый отказ (для `throw;` и причин).
    handled: Option<ValueId>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Kind {
    Entry,
    Method,
    Successor,
    Ctor,
    Default,
    Destroy,
}

struct BodyLowerer<'c, 'r, 'a> {
    ctx: &'c mut Ctx<'r, 'a>,
    body: &'r thir::Body,
    kind: Kind,
    object: Option<ObjectId>,
    /// Подстановка параметров типа специализации.
    env: Vec<(ParamId, Ty)>,
    self_value: Option<ValueId>,
    b: FnBuilder,
    vars: Vec<Var>,
    /// Переменная каждой локальной THIR.
    local_vars: Vec<Option<usize>>,
    /// Переменные каждой открытой области.
    scopes: Vec<Vec<usize>>,
    /// Переменные полей (конструктор) или staged-полей (successor) по индексу поля.
    field_vars: Vec<usize>,
    loops: Vec<LoopFrame>,
    result: ResultType,
    params: Vec<ValueId>,
    tries: Vec<TryCtx>,
    /// Очистка каждой области `scopes` (регион, defer, цели отказа).
    cleanups: Vec<scopes::Cleanup<'r>>,
    /// Значения, которые следующая блочная область примет при открытии
    /// (пойманный Error для тела `catch`).
    pending_adopt: Vec<Typed>,
    /// `succ.take` → исходное значение `^C`: цель unwind, чей параметр
    /// получил тип `^C` до погашения, получает исходное значение.
    taken_from: HashMap<ValueId, ValueId>,
    /// Конструктор: скрытая переменная-признак «поле связано» для каждого поля
    /// и константы `true`/`false` входного блока. Поле, связанное не на всех
    /// путях, проверяется после тела, как в Perl (§11.3).
    field_flags: Vec<usize>,
    flag_values: Option<(ValueId, ValueId)>,
    /// `self` конструктора как значение (когда все поля связаны): объект,
    /// построенный в этой точке, и значения его полей; конец конструктора
    /// возвращает его же.
    early_self: Option<(ValueId, Vec<ValueId>)>,
}

/// Lowering одной выделенной функции модуля.
pub(super) fn lower_task(ctx: &mut Ctx<'_, '_>, func: FuncId) -> Result<ir::Function> {
    let (task, symbol) = ctx.tasks[func.0].clone();
    match task {
        Task::Entry => lower_entry(ctx, symbol),
        Task::Method { object, method }
            if ctx.intrinsic_name(object, Member::Method(method)).is_some() =>
        {
            let name = ctx
                .intrinsic_name(object, Member::Method(method))
                .expect("checked");
            ctx.lower_intrinsic(symbol, object, Member::Method(method), name)
        }
        Task::Ctor { object, ctor, .. }
            if ctx.intrinsic_name(object, Member::Ctor(ctor)).is_some() =>
        {
            let name = ctx
                .intrinsic_name(object, Member::Ctor(ctor))
                .expect("checked");
            ctx.lower_intrinsic(symbol, object, Member::Ctor(ctor), name)
        }
        Task::Method { object, method } if ctx.reg.is_adapter(ctx.objects[object.0].decl) => {
            adapters::lower_method(ctx, symbol, object, method)
        }
        Task::AdapterNew { object } => adapters::lower_new(ctx, symbol, object),
        Task::Closure { index } => lower_closure(ctx, symbol, index),
        Task::Test { index } => lower_test(ctx, symbol, index),
        Task::Runtime {
            name,
            params,
            result,
        } => Ok(ir::Function {
            symbol,
            params: (0..params.len()).map(ValueId).collect(),
            result,
            attrs: vec![ir::Attr::Intrinsic {
                name,
                objects: Vec::new(),
                helpers: Vec::new(),
            }],
            values: params,
            blocks: Vec::new(),
            span: Span::empty(0),
        }),
        Task::CapField { object, cap, field } => lower_cap_field(ctx, symbol, object, cap, field),
        Task::Witness {
            object,
            method,
            result,
        } => lower_witness(ctx, symbol, object, method, result),
        Task::Member {
            receiver,
            name,
            arity,
            bound,
        } => lower_member(ctx, symbol, receiver, &name, arity, bound),
        Task::WorkerEntry {
            object,
            ctor,
            given,
            method,
            fields,
        } => lower_worker_entry(ctx, symbol, object, ctor, given, method, fields),
        Task::WorkerBind { object, method } => lower_worker_bind(ctx, symbol, object, method),
        Task::Method { object, method } if ctx.reg.is_system_object(ctx.objects[object.0].decl) => {
            lower_system_method(ctx, symbol, object, method)
        }
        Task::Method { object, method } => lower_method(ctx, symbol, object, method),
        Task::Ctor {
            object,
            ctor,
            given,
        } => lower_ctor(ctx, symbol, object, ctor, given),
        Task::Default { object, field } => lower_default(ctx, symbol, object, field),
        Task::Destroy { object } => lower_destroy(ctx, symbol, object),
        Task::Native { native } => lower_native(ctx, symbol, native),
    }
}

fn new_lowerer<'c, 'r, 'a>(
    ctx: &'c mut Ctx<'r, 'a>,
    owner: BodyOwner,
    kind: Kind,
    object: Option<ObjectId>,
    result: ResultType,
) -> BodyLowerer<'c, 'r, 'a> {
    let body = ctx.body(&owner);
    let env = object
        .map(|object| ctx.objects[object.0].env.clone())
        .unwrap_or_default();
    lowerer_for(ctx, body, env, kind, object, result)
}

fn lowerer_for<'c, 'r, 'a>(
    ctx: &'c mut Ctx<'r, 'a>,
    body: &'r thir::Body,
    env: Vec<(ParamId, Ty)>,
    kind: Kind,
    object: Option<ObjectId>,
    result: ResultType,
) -> BodyLowerer<'c, 'r, 'a> {
    BodyLowerer {
        body,
        kind,
        object,
        env,
        self_value: None,
        b: FnBuilder::new(),
        vars: Vec::new(),
        local_vars: vec![None; body.locals.len()],
        scopes: vec![Vec::new()],
        field_vars: Vec::new(),
        loops: Vec::new(),
        result,
        params: Vec::new(),
        tries: Vec::new(),
        cleanups: vec![scopes::Cleanup::default()],
        pending_adopt: Vec::new(),
        taken_from: HashMap::new(),
        field_flags: Vec::new(),
        flag_values: None,
        early_self: None,
        ctx,
    }
}

fn finish(
    mut lowerer: BodyLowerer<'_, '_, '_>,
    symbol: String,
    attrs: Vec<ir::Attr>,
    span: Span,
) -> Result<ir::Function> {
    lowerer.close_cleanup(span)?;
    if let Some((yes, no)) = lowerer.flag_values {
        lowerer.b.drop_unused(&[yes, no]);
    }
    let result = lowerer.result.clone();
    let (values, blocks, params) = lowerer.b.finish(&lowerer.params).map_err(|message| {
        LoweringError::invalid(span, format!("internal compiler error: {message}"))
    })?;
    Ok(ir::Function {
        symbol,
        params,
        result,
        attrs,
        values,
        blocks,
        span,
    })
}

/// Поле cap: функция над владельцем (`[method Owner]`), читающая его поля.
fn lower_cap_field(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    cap: usize,
    field: usize,
) -> Result<ir::Function> {
    let decl = ctx.objects[object.0].decl;
    let env = ctx.objects[object.0].env.clone();
    let reg = ctx.reg;
    let (_, ty, block) = &reg.objects[decl.0].caps[cap].fields[field];
    let span = block.span;
    let result = ResultType::Value(ctx.ir_type(&ty.subst(&env), span)?);
    let mut lowerer = new_lowerer(
        ctx,
        BodyOwner::CapField {
            object: decl,
            cap,
            field,
        },
        Kind::Method,
        Some(object),
        result,
    );
    let receiver = lowerer.b.new_value(Type::Object(object));
    lowerer.self_value = Some(receiver);
    lowerer.params.push(receiver);
    lowerer.open_root(span);
    lowerer.body_block()?;
    finish(lowerer, symbol, vec![ir::Attr::Method(object)], span)
}

/// Переходник слота роли: вызов action, `succ.take`, `role.wrap` (§9).
fn lower_witness(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    method: usize,
    result: Type,
) -> Result<ir::Function> {
    let span = ctx.objects[object.0].span;
    let env = ctx.objects[object.0].env.clone();
    let reg = ctx.reg;
    let info = &reg.objects[ctx.objects[object.0].decl.0].methods[method];
    let mut types = vec![Type::Object(object)];
    for (_, ty) in &info.params {
        types.push(ctx.ir_type(&ty.subst(&env), span)?);
    }
    let own = ctx.ir_result(&info.result, &env, span)?;
    let func = ctx.method_func(object, method, span)?;
    let mut b = FnBuilder::new();
    let params: Vec<ValueId> = types.into_iter().map(|ty| b.new_value(ty)).collect();
    let ResultType::Value(own) = own else {
        return Err(LoweringError::invalid(
            span,
            "internal compiler error: a void witness",
        ));
    };
    let mut value = b.emit(
        InstKind::Call {
            func,
            args: params.clone(),
        },
        own.clone(),
        span,
    );
    if let Type::Successor(taken) = own {
        value = b.emit(InstKind::SuccTake(value), Type::Object(taken), span);
    }
    let wrapped = b.emit(InstKind::RoleWrap(value), result.clone(), span);
    b.terminate(TermKind::Ret(Some(wrapped)), span);
    let (values, blocks, params) = b.finish(&params).map_err(|message| {
        LoweringError::invalid(span, format!("internal compiler error: {message}"))
    })?;
    Ok(ir::Function {
        symbol,
        params,
        result: ResultType::Value(result),
        attrs: vec![ir::Attr::Method(object)],
        values,
        blocks,
        span,
    })
}

/// Переходник для рантайма: вызов действия `name` значения `receiver`
/// (объекта — функцией действия, роли — слотом); результат вызова как есть
/// (у async — Future).
fn lower_member(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    receiver: Type,
    name: &str,
    arity: usize,
    bound: bool,
) -> Result<ir::Function> {
    let span = Span::empty(0);
    let missing = || {
        LoweringError::invalid(
            span,
            format!("internal compiler error: no action {name}/{arity} for a runtime helper"),
        )
    };
    let mut b = FnBuilder::new();
    let (params, result, kind) = match &receiver {
        Type::Object(object) => {
            let env = ctx.objects[object.0].env.clone();
            let reg = ctx.reg;
            let info = &reg.objects[ctx.objects[object.0].decl.0];
            let method = info
                .methods
                .iter()
                .position(|method| method.name == name && method.params.len() == arity)
                .ok_or_else(missing)?;
            let target = &info.methods[method];
            let mut types = vec![receiver.clone()];
            for (_, ty) in &target.params {
                types.push(ctx.ir_type(&ty.subst(&env), span)?);
            }
            let mut result = ctx.ir_result(&target.result, &env, span)?;
            if target.is_async {
                result = ResultType::Value(Type::Future(Box::new(result)));
            }
            let func = ctx.method_func(*object, method, span)?;
            let params: Vec<ValueId> = types.into_iter().map(|ty| b.new_value(ty)).collect();
            let kind = InstKind::Call {
                func,
                args: params.clone(),
            };
            (params, result, kind)
        }
        Type::Role(role) => {
            let slot = ctx.roles[role.0]
                .methods
                .iter()
                .position(|method| method.name == name && method.params.len() == arity)
                .ok_or_else(missing)?;
            let method = ctx.roles[role.0].methods[slot].clone();
            let mut types = vec![receiver.clone()];
            types.extend(method.params.iter().cloned());
            let params: Vec<ValueId> = types.into_iter().map(|ty| b.new_value(ty)).collect();
            let kind = InstKind::CallRole {
                receiver: params[0],
                slot,
                args: params[1..].to_vec(),
            };
            (params, method.result, kind)
        }
        _ => return Err(missing()),
    };
    match &result {
        ResultType::Value(ty) => {
            let value = b.emit(kind, ty.clone(), span);
            b.terminate(TermKind::Ret(Some(value)), span);
        }
        ResultType::Void => {
            b.emit_void(kind, span);
            b.terminate(TermKind::Ret(None), span);
        }
    }
    let (values, blocks, params) = b.finish(&params).map_err(|message| {
        LoweringError::invalid(span, format!("internal compiler error: {message}"))
    })?;
    let attrs = if bound {
        vec![ir::Attr::Lambda {
            captures: 1,
            owner: None,
        }]
    } else {
        Vec::new()
    };
    Ok(ir::Function {
        symbol,
        params,
        result,
        attrs,
        values,
        blocks,
        span,
    })
}

/// Вход процесса-воркера (§18.1): параметры — аргументы конструктора (или
/// все поля receiver), затем аргументы действия; конструирует receiver и
/// вызывает синхронное действие.
fn lower_worker_entry(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    ctor: usize,
    given: Vec<usize>,
    method: usize,
    fields: bool,
) -> Result<ir::Function> {
    let span = ctx.objects[object.0].span;
    let mut b = FnBuilder::new();
    let (ctor_types, order) = if fields {
        let order = ctx
            .worker_ctor(object)
            .map(|(_, _, order)| order)
            .ok_or_else(|| {
                LoweringError::invalid(
                    span,
                    "internal compiler error: a worker receiver has no field constructor",
                )
            })?;
        let types: Vec<Type> = ctx.objects[object.0]
            .fields
            .iter()
            .map(|(_, ty, _)| ty.clone())
            .collect();
        (types, Some(order))
    } else {
        (ctx.ctor_param_types(object, ctor, &given, span)?, None)
    };
    let env = ctx.objects[object.0].env.clone();
    let reg = ctx.reg;
    let target = &reg.objects[ctx.objects[object.0].decl.0].methods[method];
    let mut method_types = Vec::new();
    for (_, ty) in &target.params {
        method_types.push(ctx.ir_type(&ty.subst(&env), span)?);
    }
    let result = ctx.ir_result(&target.result, &env, span)?;
    let ctor_params: Vec<ValueId> = ctor_types.into_iter().map(|ty| b.new_value(ty)).collect();
    let method_params: Vec<ValueId> = method_types.into_iter().map(|ty| b.new_value(ty)).collect();
    let ctor_args = match &order {
        Some(order) => order.iter().map(|field| ctor_params[*field]).collect(),
        None => ctor_params.clone(),
    };
    let ctor_given = if fields {
        ctx.worker_ctor(object)
            .map(|(_, given, _)| given)
            .unwrap_or_default()
    } else {
        given
    };
    let ctor_func = ctx.ctor_func(object, ctor, ctor_given);
    let receiver = b.emit(
        InstKind::Call {
            func: ctor_func,
            args: ctor_args,
        },
        Type::Object(object),
        span,
    );
    let func = ctx.method_func(object, method, span)?;
    let mut args = vec![receiver];
    args.extend(method_params.iter().copied());
    let kind = InstKind::Call { func, args };
    let value = match &result {
        ResultType::Value(ty) => Some(b.emit(kind, ty.clone(), span)),
        ResultType::Void => {
            b.emit_void(kind, span);
            None
        }
    };
    b.terminate(TermKind::Ret(value), span);
    let mut all = ctor_params;
    all.extend(method_params);
    let (values, blocks, params) = b.finish(&all).map_err(|message| {
        LoweringError::invalid(span, format!("internal compiler error: {message}"))
    })?;
    Ok(ir::Function {
        symbol,
        params,
        result,
        attrs: vec![ir::Attr::Lambda {
            captures: 0,
            owner: None,
        }],
        values,
        blocks,
        span,
    })
}

/// Bound action `method` для воркеров (функция объекта): `fn.closure` над
/// receiver, вход воркера и поля receiver в `worker.bind`.
fn lower_worker_bind(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    method: usize,
) -> Result<ir::Function> {
    let span = ctx.objects[object.0].span;
    let (ctor, given, _) = ctx.worker_ctor(object).ok_or_else(|| {
        LoweringError::invalid(
            span,
            "internal compiler error: a worker receiver has no field constructor",
        )
    })?;
    let env = ctx.objects[object.0].env.clone();
    let reg = ctx.reg;
    let target = &reg.objects[ctx.objects[object.0].decl.0].methods[method];
    let mut method_types = Vec::new();
    for (_, ty) in &target.params {
        method_types.push(ctx.ir_type(&ty.subst(&env), span)?);
    }
    let result = ctx.ir_result(&target.result, &env, span)?;
    let field_types: Vec<Type> = ctx.objects[object.0]
        .fields
        .iter()
        .map(|(_, ty, _)| ty.clone())
        .collect();
    let action_ty = Type::Fn(Box::new(ir::FnSig {
        params: method_types.clone(),
        result: result.clone(),
    }));
    let mut entry_params = field_types.clone();
    entry_params.extend(method_types);
    let entry_ty = Type::Fn(Box::new(ir::FnSig {
        params: entry_params,
        result,
    }));
    let method_func = ctx.method_func(object, method, span)?;
    let entry_func = ctx.worker_entry(object, ctor, given, method, true);
    let mut b = FnBuilder::new();
    let receiver = b.new_value(Type::Object(object));
    let action = b.emit(
        InstKind::FnClosure {
            func: method_func,
            captures: vec![receiver],
        },
        action_ty.clone(),
        span,
    );
    let entry = b.emit(
        InstKind::FnClosure {
            func: entry_func,
            captures: Vec::new(),
        },
        entry_ty,
        span,
    );
    let fields = field_types
        .into_iter()
        .enumerate()
        .map(|(field, ty)| {
            b.emit(
                InstKind::ObjGet {
                    object: receiver,
                    field,
                },
                ty,
                span,
            )
        })
        .collect();
    let bound = b.emit(
        InstKind::WorkerBind {
            action,
            entry,
            fields,
        },
        action_ty.clone(),
        span,
    );
    b.terminate(TermKind::Ret(Some(bound)), span);
    let (values, blocks, params) = b.finish(&[receiver]).map_err(|message| {
        LoweringError::invalid(span, format!("internal compiler error: {message}"))
    })?;
    Ok(ir::Function {
        symbol,
        params,
        result: ResultType::Value(action_ty),
        attrs: vec![ir::Attr::Method(object)],
        values,
        blocks,
        span,
    })
}

/// Функция `[lambda]` анонимного action: захваты (сначала `self`), затем
/// его параметры; `return` возвращает из лямбды.
fn lower_closure(ctx: &mut Ctx<'_, '_>, symbol: String, index: usize) -> Result<ir::Function> {
    let task = ctx.closures[index].clone();
    let span = task.body.span;
    let mut lowerer = lowerer_for(
        ctx,
        task.body,
        task.env.clone(),
        Kind::Method,
        task.object,
        task.sig.result.clone(),
    );
    if task.self_capture {
        let object = task.object.expect("a self capture has an object");
        let receiver = lowerer.b.new_value(Type::Object(object));
        lowerer.self_value = Some(receiver);
        lowerer.params.push(receiver);
    }
    for (local, ty) in &task.captures {
        let value = lowerer.b.new_value(ty.clone());
        lowerer.bind_local(
            *local,
            Typed {
                value,
                ty: ty.clone(),
            },
        );
        lowerer.params.push(value);
    }
    for local in lowerer.body.params.clone() {
        let value = lowerer.new_param(local, span)?;
        lowerer.params.push(value);
    }
    lowerer.open_root(span);
    lowerer.body_block()?;
    let captures = usize::from(task.self_capture) + task.captures.len();
    let attrs = vec![ir::Attr::Lambda {
        captures,
        owner: task.object,
    }];
    finish(lowerer, symbol, attrs, span)
}

/// `test "name"`: тело без результата; `async test` — функция `[async]`.
fn lower_test(ctx: &mut Ctx<'_, '_>, symbol: String, index: usize) -> Result<ir::Function> {
    let (is_async, span) = (ctx.reg.tests[index].is_async, ctx.reg.tests[index].span);
    let mut lowerer = new_lowerer(
        ctx,
        BodyOwner::Test(index),
        Kind::Method,
        None,
        ResultType::Void,
    );
    lowerer.open_root(span);
    lowerer.body_block()?;
    let attrs = if is_async {
        vec![ir::Attr::Async]
    } else {
        Vec::new()
    };
    finish(lowerer, symbol, attrs, span)
}

fn lower_entry(ctx: &mut Ctx<'_, '_>, symbol: String) -> Result<ir::Function> {
    let entry = ctx.reg.entry.expect("entry task has a declaration");
    let mut lowerer = new_lowerer(
        ctx,
        BodyOwner::Entry,
        Kind::Entry,
        None,
        ResultType::Value(Type::Int),
    );
    lowerer.open_root(entry.span);
    lowerer.body_block()?;
    let mut attrs = vec![ir::Attr::Entry];
    if entry.is_async {
        attrs.push(ir::Attr::Async);
    }
    finish(lowerer, symbol, attrs, entry.span)
}

fn lower_method(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    method: usize,
) -> Result<ir::Function> {
    let decl = ctx.objects[object.0].decl;
    let env = ctx.objects[object.0].env.clone();
    let reg = ctx.reg;
    let info = &reg.objects[decl.0].methods[method];
    let span = info.decl.map_or(Span::empty(0), |decl| decl.span);
    let result = ctx.ir_result(&info.result, &env, span)?;
    let (successor, private, is_async) = (info.successor, info.private, info.is_async);
    let kind = if successor {
        Kind::Successor
    } else {
        Kind::Method
    };
    let mut lowerer = new_lowerer(
        ctx,
        BodyOwner::Method {
            object: decl,
            method,
        },
        kind,
        Some(object),
        result,
    );
    let receiver = lowerer.b.new_value(Type::Object(object));
    lowerer.self_value = Some(receiver);
    lowerer.params.push(receiver);
    for local in lowerer.body.params.clone() {
        let value = lowerer.new_param(local, span)?;
        lowerer.params.push(value);
    }
    lowerer.open_root(span);
    let mut attrs = vec![if successor {
        ir::Attr::Successor(object)
    } else {
        ir::Attr::Method(object)
    }];
    if private {
        attrs.push(ir::Attr::Private);
    }
    if is_async {
        attrs.push(ir::Attr::Async);
    }
    if successor {
        for field in 0..lowerer.ctx.objects[object.0].fields.len() {
            lowerer.vars.push(Var {
                value: None,
                rebindable: false,
                kind: VarKind::Staged(field),
                scope: 0,
            });
            lowerer.field_vars.push(lowerer.vars.len() - 1);
        }
        let block = &lowerer.body.block;
        let out = lowerer.block(block, false)?;
        if !matches!(out, Out::Diverged) && lowerer.b.is_open() {
            lowerer.finish_successor(block.span)?;
        }
    } else {
        lowerer.body_block()?;
    }
    finish(lowerer, symbol, attrs, span)
}

/// Action системного объекта (`NativeFailure.code()`): чтение поля;
/// `text()` — `message`, как в Perl-интерпретаторе.
fn lower_system_method(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    method: usize,
) -> Result<ir::Function> {
    let instance = &ctx.objects[object.0];
    let name = ctx.reg.objects[instance.decl.0].methods[method]
        .name
        .as_str();
    let field_name = if name == "text" { "message" } else { name };
    let span = instance.span;
    if instance.decl == ctx.reg.system.cancelled {
        // `Cancelled.text()` — текст Perl-интерпретатора.
        let mut b = FnBuilder::new();
        let receiver = b.new_value(Type::Object(object));
        let text = b.emit(
            InstKind::Const(ir::Constant::Str("Operation was cancelled".to_string())),
            Type::Str,
            span,
        );
        b.terminate(TermKind::Ret(Some(text)), span);
        let (values, blocks, params) = b.finish(&[receiver]).map_err(|message| {
            LoweringError::invalid(span, format!("internal compiler error: {message}"))
        })?;
        return Ok(ir::Function {
            symbol,
            params,
            result: ResultType::Value(Type::Str),
            attrs: vec![ir::Attr::Method(object)],
            values,
            blocks,
            span,
        });
    }
    let field = instance
        .fields
        .iter()
        .position(|(candidate, ..)| candidate == field_name)
        .ok_or_else(|| {
            LoweringError::invalid(
                span,
                format!("internal compiler error: system action {name} has no field"),
            )
        })?;
    let mut b = FnBuilder::new();
    let receiver = b.new_value(Type::Object(object));
    let value = b.emit(
        InstKind::ObjGet {
            object: receiver,
            field,
        },
        Type::Str,
        span,
    );
    b.terminate(TermKind::Ret(Some(value)), span);
    let (values, blocks, params) = b.finish(&[receiver]).map_err(|message| {
        LoweringError::invalid(span, format!("internal compiler error: {message}"))
    })?;
    Ok(ir::Function {
        symbol,
        params,
        result: ResultType::Value(Type::Str),
        attrs: vec![ir::Attr::Method(object)],
        values,
        blocks,
        span,
    })
}

fn lower_ctor(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    ctor: usize,
    given: Vec<usize>,
) -> Result<ir::Function> {
    let decl = ctx.objects[object.0].decl;
    let reg = ctx.reg;
    let info = &reg.objects[decl.0];
    let span = info.ctors[ctor].span;
    let full = matches!(
        info.ctors[ctor].shape,
        crate::analysis::registry::CtorShape::Full { .. }
    );
    let listed = match &info.ctors[ctor].shape {
        crate::analysis::registry::CtorShape::Shorthand { fields, .. } => fields.clone(),
        crate::analysis::registry::CtorShape::Full { .. } => Vec::new(),
    };
    let fields: Vec<(String, Type, Option<FuncId>)> = ctx.objects[object.0].fields.clone();
    let mut lowerer = new_lowerer(
        ctx,
        BodyOwner::Ctor { object: decl, ctor },
        Kind::Ctor,
        Some(object),
        ResultType::Value(Type::Object(object)),
    );
    for _ in &fields {
        lowerer.vars.push(Var {
            value: None,
            rebindable: false,
            kind: VarKind::Field,
            scope: 0,
        });
        lowerer.field_vars.push(lowerer.vars.len() - 1);
    }
    // Признаки связывания полей: `false` на входе, `true` после связывания.
    let yes = lowerer.constant(Constant::Bool(true), span).value;
    let no = lowerer.constant(Constant::Bool(false), span).value;
    lowerer.flag_values = Some((yes, no));
    for _ in &fields {
        lowerer.vars.push(Var {
            value: Some(Typed {
                value: no,
                ty: Type::Bool,
            }),
            rebindable: false,
            kind: VarKind::Local,
            scope: 0,
        });
        lowerer.field_flags.push(lowerer.vars.len() - 1);
    }
    // Параметры — значения вызывающего, определены на входе.
    let param_values: Vec<ValueId> = if full {
        let mut values = Vec::new();
        for local in lowerer.body.params.clone() {
            let ty = lowerer.local_type(local, span)?;
            values.push(lowerer.b.new_value(ty));
        }
        values
    } else {
        given
            .iter()
            .map(|field| lowerer.b.new_value(fields[*field].1.clone()))
            .collect()
    };
    lowerer.params = param_values.clone();
    lowerer.open_root(span);
    // Defaults вычисляются всегда, в порядке полей, до тела (R09).
    for (field, (_, ty, default)) in fields.iter().enumerate() {
        let Some(func) = default else {
            continue;
        };
        let value = lowerer.b.emit(
            InstKind::Call {
                func: *func,
                args: Vec::new(),
            },
            ty.clone(),
            span,
        );
        let var = lowerer.field_vars[field];
        let typed = Typed {
            value,
            ty: ty.clone(),
        };
        lowerer.adopt_into(0, &typed, span);
        lowerer.vars[var].value = Some(typed);
        lowerer.mark_bound(field);
    }
    if full {
        for (local, value) in lowerer.body.params.clone().into_iter().zip(&param_values) {
            let ty = lowerer.b.value_type(*value).clone();
            lowerer.bind_local(local, Typed { value: *value, ty });
        }
    } else {
        // Shorthand: переданные поля связываются до тела.
        for (field, value) in given.iter().zip(&param_values) {
            let var = lowerer.field_vars[*field];
            lowerer.vars[var].value = Some(Typed {
                value: *value,
                ty: fields[*field].1.clone(),
            });
            lowerer.mark_bound(*field);
        }
        // В теле shorthand поля списка читаются по имени: их значения на
        // входе в тело (переданные или default), §11.3.
        for (local, field) in lowerer.body.params.clone().into_iter().zip(listed) {
            let var = lowerer.field_vars[field];
            if let Some(value) = lowerer.vars[var].value.clone() {
                lowerer.bind_local(local, value);
            }
        }
    }
    let block = &lowerer.body.block;
    lowerer.block(block, false)?;
    if lowerer.b.is_open() {
        lowerer.finish_ctor(span)?;
    }
    finish(lowerer, symbol, vec![ir::Attr::Ctor(object)], span)
}

/// `native f(...) is R;`: функция без тела с символом рантайма `модуль::имя`.
fn lower_native(ctx: &mut Ctx<'_, '_>, symbol: String, native: usize) -> Result<ir::Function> {
    let reg = ctx.reg;
    let info = &reg.natives[native];
    if info.variadic {
        return Err(LoweringError::unsupported(
            info.decl.span,
            "IR lowering of variadic native parameters is not implemented yet.",
        ));
    }
    let mut values = Vec::new();
    for (_, ty) in &info.params {
        values.push(ctx.ir_type(ty, info.decl.span)?);
    }
    let result = ctx.ir_result(&info.result, &[], info.decl.span)?;
    Ok(ir::Function {
        symbol,
        params: (0..values.len()).map(ValueId).collect(),
        result,
        // Канонический символ ABI (§14.1): `модуль@major::имя/арность`.
        attrs: {
            let mut attrs = vec![ir::Attr::Extern(format!(
                "{}@{}::{}/{}",
                reg.modules[info.module].name,
                reg.modules[info.module].program.module.version.major,
                info.name,
                info.params.len()
            ))];
            // Async native: вызов даёт `future<R>` от provider (§13).
            if info.is_async {
                attrs.push(ir::Attr::Async);
            }
            attrs
        },
        values,
        blocks: Vec::new(),
        span: info.decl.span,
    })
}

/// `destroy { … }`: `(self) -> void`, вызывает только рантайм (§7).
fn lower_destroy(ctx: &mut Ctx<'_, '_>, symbol: String, object: ObjectId) -> Result<ir::Function> {
    let decl = ctx.objects[object.0].decl;
    let span = ctx.reg.objects[decl.0]
        .destroy
        .map_or(Span::empty(0), |destroy| destroy.span);
    let mut lowerer = new_lowerer(
        ctx,
        BodyOwner::Destroy { object: decl },
        Kind::Destroy,
        Some(object),
        ResultType::Void,
    );
    let receiver = lowerer.b.new_value(Type::Object(object));
    lowerer.self_value = Some(receiver);
    lowerer.params.push(receiver);
    lowerer.open_root(span);
    lowerer.body_block()?;
    finish(lowerer, symbol, vec![ir::Attr::Destroy(object)], span)
}

fn lower_default(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    field: usize,
) -> Result<ir::Function> {
    let decl = ctx.objects[object.0].decl;
    let ty = ctx.objects[object.0].fields[field].1.clone();
    let mut lowerer = new_lowerer(
        ctx,
        BodyOwner::Default {
            object: decl,
            field,
        },
        Kind::Default,
        Some(object),
        ResultType::Value(ty),
    );
    let span = lowerer.body.span;
    lowerer.open_root(span);
    // Default — одно выражение без собственной области.
    let stmts = &lowerer.body.block.stmts;
    let [
        thir::Stmt {
            kind: StmtKind::Expr(expr),
            ..
        },
    ] = stmts.as_slice()
    else {
        return Err(LoweringError::invalid(
            span,
            "internal compiler error: a default is one expression",
        ));
    };
    if let Some(value) = lowerer.value(expr)? {
        let value = lowerer.take(value, expr.span);
        lowerer.ret(Some(value), expr.span)?;
    }
    finish(
        lowerer,
        symbol,
        vec![ir::Attr::Default(object, field)],
        span,
    )
}

impl<'r> BodyLowerer<'_, 'r, '_> {
    // ----- типы и локальные -----

    /// Тип OIR типа анализа в этой специализации.
    fn ir_type(&mut self, ty: &Ty, span: Span) -> Result<Type> {
        let ty = ty.subst(&self.env);
        self.ctx.ir_type(&ty, span)
    }

    fn local_type(&mut self, local: LocalId, span: Span) -> Result<Type> {
        let ty = self.body.locals[local.0].ty.clone();
        self.ir_type(&ty, span)
    }

    fn expr_type(&mut self, expr: &thir::Expr) -> Result<Type> {
        match &expr.ty {
            Some(ty) => self.ir_type(ty, expr.span),
            None => Err(LoweringError::invalid(
                expr.span,
                "internal compiler error: a value expression has no type",
            )),
        }
    }

    /// Параметр функции для локальной THIR (variadic — массив).
    fn new_param(&mut self, local: LocalId, span: Span) -> Result<ValueId> {
        let ty = self.local_type(local, span)?;
        let value = self.b.new_value(ty.clone());
        self.bind_local(local, Typed { value, ty });
        Ok(value)
    }

    /// Новая переменная локальной THIR в текущей области.
    fn bind_local(&mut self, local: LocalId, value: Typed) {
        let rebindable = self.body.locals[local.0].rebindable;
        self.vars.push(Var {
            value: Some(value),
            rebindable,
            kind: VarKind::Local,
            scope: self.scopes.len() - 1,
        });
        let var = self.vars.len() - 1;
        self.local_vars[local.0] = Some(var);
        self.scopes
            .last_mut()
            .expect("statements run inside a scope")
            .push(var);
    }

    fn local_var(&self, local: LocalId, span: Span) -> Result<usize> {
        self.local_vars[local.0].ok_or_else(|| {
            LoweringError::invalid(
                span,
                format!(
                    "internal compiler error: local \"{}\" has no value",
                    self.body.locals[local.0].name
                ),
            )
        })
    }

    /// Переменные текущей позиции: все области и поля функции.
    fn visible_vars(&self) -> impl Iterator<Item = usize> + '_ {
        self.scopes
            .iter()
            .flatten()
            .copied()
            .chain(self.field_vars.iter().copied())
    }

    fn emit(&mut self, kind: InstKind, ty: Type, span: Span) -> Typed {
        let call = matches!(kind, InstKind::Call { .. } | InstKind::CallRole { .. });
        let value = Typed {
            value: self.b.emit(kind, ty.clone(), span),
            ty,
        };
        // Результат вызова — свежие идентичности вызывающего (§7).
        if call {
            self.track(&value, span);
        }
        value
    }

    fn constant(&mut self, constant: Constant, span: Span) -> Typed {
        let ty = constant.ty();
        self.emit(InstKind::Const(constant), ty, span)
    }

    /// Погасить обязательство `^C`; у остальных типов значение не меняется.
    /// Алиасы разделяют одно обязательство: их значение тоже становится `C`.
    fn take(&mut self, value: Typed, span: Span) -> Typed {
        let Type::Successor(object) = value.ty else {
            return value;
        };
        let taken = self
            .b
            .emit(InstKind::SuccTake(value.value), Type::Object(object), span);
        let original = self
            .taken_from
            .get(&value.value)
            .copied()
            .unwrap_or(value.value);
        self.taken_from.insert(taken, original);
        let replacement = Typed {
            value: taken,
            ty: Type::Object(object),
        };
        for var in &mut self.vars {
            if var.value.as_ref().map(|typed| typed.value) == Some(value.value) {
                var.value = Some(replacement.clone());
            }
        }
        replacement
    }

    // ----- тело -----

    /// Тело действия: значение последнего оператора — результат.
    fn body_block(&mut self) -> Result<()> {
        let block = &self.body.block;
        let want = self.body.returns_value;
        let out = self.block(block, want)?;
        let tail_span = block.stmts.last().map_or(block.span, |stmt| stmt.span);
        match out {
            Out::Value(value) if want => {
                let value = self.take(value, tail_span);
                self.ret(Some(value), tail_span)?;
            }
            Out::Value(_) | Out::Unit => {
                if want {
                    return Err(LoweringError::invalid(
                        block.span,
                        "internal compiler error: an action body produced no value",
                    ));
                }
                self.ret(None, tail_span)?;
            }
            Out::Diverged => {}
        }
        Ok(())
    }

    fn finish_successor(&mut self, span: Span) -> Result<()> {
        let receiver = self.self_value.expect("successor has a receiver");
        let mut updates = Vec::new();
        for (field, var) in self.field_vars.iter().enumerate() {
            if let Some(value) = &self.vars[*var].value {
                updates.push((field, value.value));
            }
        }
        let object = self.object.expect("successor belongs to an object");
        let next = self.b.emit(
            InstKind::ObjNext {
                object: receiver,
                updates,
            },
            Type::Object(object),
            span,
        );
        // `^C` на JVM — тот же объект; payload — новая идентичность.
        self.ret(
            Some(Typed {
                value: next,
                ty: Type::Object(object),
            }),
            span,
        )
    }

    fn finish_ctor(&mut self, span: Span) -> Result<()> {
        let object = self.object.expect("constructor belongs to an object");
        let name = self.ctx.objects[object.0].name.clone();
        let mut fields = Vec::new();
        for (index, var) in self.field_vars.iter().enumerate() {
            match &self.vars[*var].value {
                Some(value) => fields.push(value.value),
                None => {
                    return Err(LoweringError::invalid(
                        span,
                        format!(
                            "Object \"{name}\" field \"{}\" was not initialized",
                            self.ctx.objects[object.0].fields[index].0
                        ),
                    ));
                }
            }
        }
        // Поля, связанные не на всех путях: проверка признаков после тела.
        for (field, flag) in self.field_flags.clone().into_iter().enumerate() {
            let Some((yes, _)) = self.flag_values else {
                break;
            };
            let bound = self.vars[flag]
                .value
                .as_ref()
                .expect("flags hold a value")
                .value;
            if bound == yes {
                continue;
            }
            let ok = self.b.new_block();
            let missing = self.b.new_block();
            self.b.terminate(
                TermKind::Cond {
                    condition: bound,
                    then_target: Target::new(ok),
                    else_target: Target::new(missing),
                },
                span,
            );
            self.b.switch_to(missing);
            self.b.terminate(
                TermKind::Trap {
                    code: TrapCode::Unbound,
                    message: format!(
                        "Object \"{name}\" field \"{}\" was not initialized",
                        self.ctx.objects[object.0].fields[field].0
                    ),
                },
                span,
            );
            self.b.switch_to(ok);
            self.sync_unwind();
        }
        let built = match self.early_self.take() {
            // `self` уже ушёл значением: результат — тот же объект.
            Some((built, known)) if known == fields => built,
            Some(_) => {
                return Err(unsupported(
                    span,
                    "field assignment after `self` escaped its constructor",
                ));
            }
            None => self.b.emit(
                InstKind::ObjNew { object, fields },
                Type::Object(object),
                span,
            ),
        };
        self.ret(
            Some(Typed {
                value: built,
                ty: Type::Object(object),
            }),
            span,
        )
    }

    /// `self` в конструкторе после связывания всех полей: объект строится
    /// здесь (§11.3); повторное `self` с теми же полями — тот же объект.
    fn constructed_self(&mut self, span: Span) -> Result<Out> {
        let (Kind::Ctor, Some(object)) = (self.kind, self.object) else {
            return Err(LoweringError::invalid(
                span,
                "`self` cannot be used as a value before construction completes",
            ));
        };
        if let Some((built, _)) = &self.early_self {
            return Ok(Out::Value(Typed {
                value: *built,
                ty: Type::Object(object),
            }));
        }
        // Ещё не связанные поля: `undef`, позже — `obj.init` того же объекта.
        let mut fields = Vec::new();
        for (index, var) in self.field_vars.clone().into_iter().enumerate() {
            match &self.vars[var].value {
                Some(value) => fields.push(value.value),
                None => {
                    let ty = self.ctx.objects[object.0].fields[index].1.clone();
                    fields.push(self.b.emit(InstKind::Undef, ty, span));
                }
            }
        }
        let built = self.b.emit(
            InstKind::ObjNew {
                object,
                fields: fields.clone(),
            },
            Type::Object(object),
            span,
        );
        self.early_self = Some((built, fields));
        Ok(Out::Value(Typed {
            value: built,
            ty: Type::Object(object),
        }))
    }

    /// Поле строящегося объекта связано на текущем пути.
    fn mark_bound(&mut self, field: usize) {
        if let (Some(flag), Some((yes, _))) = (self.field_flags.get(field), self.flag_values) {
            self.vars[*flag].value = Some(Typed {
                value: yes,
                ty: Type::Bool,
            });
        }
    }

    /// Исходное значение поля receiver (successor читает старые поля, D04).
    fn read_field(&mut self, field: usize, span: Span) -> Typed {
        let object = self.object.expect("field reads have an object");
        let ty = self.ctx.objects[object.0].fields[field].1.clone();
        let value = self.b.emit(
            InstKind::ObjGet {
                object: self.self_value.expect("field reads have a receiver"),
                field,
            },
            ty.clone(),
            span,
        );
        Typed { value, ty }
    }

    // ----- слияние ветвей -----

    fn snapshot(&self) -> Vec<Option<Typed>> {
        self.vars.iter().map(|var| var.value.clone()).collect()
    }

    fn restore(&mut self, env: &[Option<Typed>]) {
        for (var, value) in self.vars.iter_mut().zip(env) {
            var.value = value.clone();
        }
        self.sync_unwind();
    }

    /// Цель unwind для блоков, начинающихся с текущими значениями переменных.
    fn sync_unwind(&mut self) {
        let unwind = self.tries.last().map(|ctx| {
            let params = self.b.block_params(ctx.target);
            let mut args: Vec<ValueId> = ctx
                .tracked
                .iter()
                .enumerate()
                .map(|(index, var)| {
                    let value = self.vars[*var]
                        .value
                        .as_ref()
                        .expect("tracked variables hold values")
                        .value;
                    // Параметр цели создан до `succ.take`: передаём `^C`.
                    match (params.get(index), self.taken_from.get(&value)) {
                        (Some(param), Some(original))
                            if self.b.value_type(*param) != self.b.value_type(value) =>
                        {
                            *original
                        }
                        _ => value,
                    }
                })
                .collect();
            args.extend(ctx.handled);
            Target {
                block: ctx.target,
                args,
            }
        });
        self.b.unwind = unwind;
    }

    /// После присваивания переменной, которую получает цель unwind, следующий
    /// код начинается новым блоком: аргументы unwind — значения на входе блока.
    fn assigned(&mut self, var: usize) {
        if self.tries.iter().any(|ctx| ctx.tracked.contains(&var)) {
            self.split();
        }
    }

    fn split(&mut self) {
        if self.b.current_has_code() {
            let next = self.b.new_block();
            self.b
                .terminate(TermKind::Br(Target::new(next)), Span::empty(0));
            self.b.switch_to(next);
        }
        self.sync_unwind();
    }

    /// Текущая позиция как незавершённая ветвь (или `None`, если управление ушло).
    fn arm(&self, out: Out) -> Option<Arm> {
        match (self.b.current(), out) {
            (Some(end), out @ (Out::Value(_) | Out::Unit)) => Some(Arm {
                end,
                out,
                env: self.snapshot(),
            }),
            _ => None,
        }
    }

    /// Привести значение ветви к типу слияния: `[]`/`[:]` получают тег,
    /// `^C` гасится (обязательства проверил анализ).
    fn normalize(&mut self, value: &Typed, target: &Type, span: Span) -> Typed {
        if value.ty == *target {
            return value.clone();
        }
        match (&value.ty, target) {
            (Type::Array(element), Type::Array(_)) if **element == Type::Never => {
                self.emit(InstKind::Retag(value.value), target.clone(), span)
            }
            (Type::Map(key, _), Type::Map(..)) if **key == Type::Never => {
                self.emit(InstKind::MapRetag(value.value), target.clone(), span)
            }
            (Type::Successor(_), _) => self.take(value.clone(), span),
            _ => value.clone(),
        }
    }

    /// Тип слияния значений: тегированная коллекция, иначе `C` вместо `^C`.
    fn join_type(types: &[Type]) -> Type {
        if let Some(tagged) = types.iter().find(|ty| match ty {
            Type::Array(element) => **element != Type::Never,
            Type::Map(key, _) => **key != Type::Never,
            _ => false,
        }) {
            return tagged.clone();
        }
        let first = types[0].clone();
        if types.iter().all(|ty| *ty == first) {
            return first;
        }
        first.taken()
    }

    /// Сводит ветви: значение (если его ждут и все ветви его дали) и
    /// различающиеся значения переменных `0..base` становятся параметрами.
    fn join(&mut self, mut arms: Vec<Arm>, base: usize, want: bool, span: Span) -> Result<Out> {
        if arms.is_empty() {
            self.b.close();
            return Ok(Out::Diverged);
        }
        let carried = want && arms.iter().all(|arm| matches!(arm.out, Out::Value(_)));
        if arms.len() == 1 {
            let arm = arms.pop().expect("one arm");
            self.b.switch_to(arm.end);
            self.restore(&arm.env);
            return Ok(if carried { arm.out } else { Out::Unit });
        }
        let join = self.b.new_block();
        let mut result = None;
        if carried {
            let types: Vec<Type> = arms
                .iter()
                .map(|arm| match &arm.out {
                    Out::Value(value) => value.ty.clone(),
                    _ => unreachable!("carried arms have values"),
                })
                .collect();
            let ty = Self::join_type(&types);
            for arm in arms.iter_mut() {
                let Out::Value(value) = arm.out.clone() else {
                    continue;
                };
                if value.ty != ty {
                    self.b.switch_to(arm.end);
                    let normalized = self.normalize(&value, &ty, span);
                    arm.out = Out::Value(normalized);
                    arm.end = self.b.current().expect("normalizing keeps the arm open");
                }
            }
            self.b.close();
            result = Some(Typed {
                value: self.b.add_param(join, ty.clone()),
                ty,
            });
        }
        let mut merged: Vec<(usize, Option<Typed>)> = Vec::new();
        let mut params: Vec<usize> = Vec::new();
        for var in 0..base {
            let values: Vec<Option<Typed>> = arms.iter().map(|arm| arm.env[var].clone()).collect();
            if values.iter().all(|value| *value == values[0]) {
                merged.push((var, values[0].clone()));
                continue;
            }
            if values.iter().any(Option::is_none) {
                match self.vars[var].kind {
                    // Ветвь без связывания даёт заглушку; признак поля на ней —
                    // `false`, и полноту проверит конец конструктора.
                    VarKind::Field => {
                        let ty = arms
                            .iter()
                            .find_map(|arm| arm.env[var].as_ref().map(|value| value.ty.clone()))
                            .expect("some arm binds the field");
                        for arm in arms.iter_mut() {
                            if arm.env[var].is_none() {
                                self.b.switch_to(arm.end);
                                let value = self.b.emit(InstKind::Undef, ty.clone(), span);
                                arm.env[var] = Some(Typed {
                                    value,
                                    ty: ty.clone(),
                                });
                            }
                        }
                        self.b.close();
                    }
                    VarKind::Staged(field) => {
                        // Ветвь без `^field` передаёт исходное значение поля.
                        for arm in arms.iter_mut() {
                            if arm.env[var].is_none() {
                                self.b.switch_to(arm.end);
                                let original = self.read_field(field, span);
                                arm.env[var] = Some(original);
                            }
                        }
                        self.b.close();
                    }
                    VarKind::Local => unreachable!("locals always hold a value"),
                }
            }
            let types: Vec<Type> = arms
                .iter()
                .map(|arm| arm.env[var].as_ref().expect("filled above").ty.clone())
                .collect();
            let ty = Self::join_type(&types);
            for arm in arms.iter_mut() {
                let value = arm.env[var].clone().expect("filled above");
                if value.ty != ty {
                    self.b.switch_to(arm.end);
                    let normalized = self.normalize(&value, &ty, span);
                    arm.env[var] = Some(normalized);
                    arm.end = self.b.current().expect("normalizing keeps the arm open");
                }
            }
            self.b.close();
            let param = self.b.add_param(join, ty.clone());
            params.push(var);
            merged.push((var, Some(Typed { value: param, ty })));
        }
        for arm in &arms {
            let mut args = Vec::new();
            if let (Some(_), Out::Value(value)) = (&result, &arm.out) {
                args.push(value.value);
            }
            for var in &params {
                args.push(arm.env[*var].as_ref().expect("checked").value);
            }
            self.b
                .terminate_block(arm.end, TermKind::Br(Target { block: join, args }), span);
        }
        self.b.switch_to(join);
        for (var, value) in merged {
            self.vars[var].value = value;
        }
        self.sync_unwind();
        Ok(match result {
            Some(result) => Out::Value(result),
            None => Out::Unit,
        })
    }

    // ----- блоки и операторы -----

    fn block(&mut self, block: &'r thir::Block, want: bool) -> Result<Out> {
        self.block_keeping(block, want, want)
    }

    /// Ветвь `if`/`match`/`try`: значение последнего оператора уходит во
    /// внешнюю область, даже если результат оператора не используется
    /// (как в Perl-интерпретаторе: результат оператора принадлежит блоку).
    fn branch(&mut self, block: &'r thir::Block, want: bool) -> Result<Out> {
        self.block_keeping(block, want, true)
    }

    fn block_keeping(&mut self, block: &'r thir::Block, want: bool, keep: bool) -> Result<Out> {
        self.push_scope();
        self.open_cleanup(block.span);
        let result = self.stmts(&block.stmts, want).and_then(|out| {
            if self.b.is_open() {
                let payload = match &out {
                    Out::Value(value) if keep => Some(value.clone()),
                    _ => None,
                };
                let depth = self.scopes.len() - 1;
                if !self.exit_scopes(depth, payload.as_ref(), block.span)? {
                    return Ok(Out::Diverged);
                }
            }
            Ok(out)
        });
        let closed = match &result {
            Ok(_) => self.close_cleanup(block.span),
            Err(_) => {
                self.cleanups.pop();
                Ok(())
            }
        };
        self.scopes.pop();
        let out = result?;
        closed?;
        // Значение блока освобождено его регионом: его учитывает внешняя область.
        if let Out::Value(value) = &out {
            if keep {
                self.track(value, block.span);
            }
        }
        Ok(out)
    }

    fn stmts(&mut self, stmts: &'r [thir::Stmt], want: bool) -> Result<Out> {
        let mut out = Out::Unit;
        for (index, stmt) in stmts.iter().enumerate() {
            let last = index + 1 == stmts.len();
            let result = self.stmt(stmt, want && last)?;
            // Код после ухода управления проверен анализом и не исполняется.
            if matches!(result, Out::Diverged) || !self.b.is_open() {
                self.b.close();
                return Ok(Out::Diverged);
            }
            if last {
                out = result;
            }
        }
        Ok(out)
    }

    fn stmt(&mut self, stmt: &'r thir::Stmt, want: bool) -> Result<Out> {
        let span = stmt.span;
        match &stmt.kind {
            StmtKind::Let { local, value } => {
                let Some(typed) = self.value(value)? else {
                    return Ok(Out::Diverged);
                };
                self.bind_local(*local, typed.clone());
                self.adopt_into(self.scopes.len() - 1, &typed, value.span);
                Ok(Out::Value(typed))
            }
            StmtKind::Expr(expr) => self.expr(expr, want),
            StmtKind::Guarded { condition, stmt } => self.guarded(condition, stmt),
            StmtKind::Return(value) => self.return_stmt(value.as_ref(), span),
            StmtKind::Confirm {
                condition,
                fallback,
            } => self.confirm(condition, fallback.as_ref(), span),
            StmtKind::Throw(value) => self.throw(value, span),
            StmtKind::Rethrow => {
                let handled = self
                    .tries
                    .iter()
                    .rev()
                    .find_map(|ctx| ctx.handled)
                    .ok_or_else(|| {
                        LoweringError::invalid(
                            span,
                            "internal compiler error: rethrow outside catch",
                        )
                    })?;
                self.b.terminate(TermKind::Raise(handled), span);
                Ok(Out::Diverged)
            }
            StmtKind::BindField { field, value } | StmtKind::StageField { field, value } => {
                self.bind_field(*field, value)
            }
            StmtKind::Rebind { local, value } => self.rebind(*local, value),
            StmtKind::Defer(value) => self.defer_stmt(value),
            StmtKind::Next => self.next(span),
            StmtKind::Last(value) => self.last(value.as_ref(), span),
        }
    }

    /// Связать поле строящегося объекта или staged-поле successor.
    fn bind_field(&mut self, field: usize, value: &'r thir::Expr) -> Result<Out> {
        let Some(typed) = self.value(value)? else {
            return Ok(Out::Diverged);
        };
        // Поле хранит сам объект: присваивание погашает обязательство `^C`.
        let typed = self.take(typed, value.span);
        let var = self.field_vars[field];
        // Значение поля принадлежит корню, пока объект не построен: и
        // свободное, и уже принятое вложенной областью (`let x = …; f = x;`).
        self.transfer_to_root(&typed, value.span);
        self.adopt_into(0, &typed, value.span);
        if let Some((built, fields)) = &mut self.early_self {
            let built = *built;
            fields[field] = typed.value;
            self.b.emit_void(
                InstKind::ObjInit {
                    object: built,
                    field,
                    value: typed.value,
                },
                value.span,
            );
        }
        self.vars[var].value = Some(typed);
        self.assigned(var);
        self.mark_bound(field);
        Ok(Out::Unit)
    }

    fn rebind(&mut self, local: LocalId, value: &'r thir::Expr) -> Result<Out> {
        let var = self.local_var(local, value.span)?;
        let Some(typed) = self.value(value)? else {
            return Ok(Out::Diverged);
        };
        // Rebind принимает новое значение в регион исходной привязки (§7).
        self.adopt_into(self.vars[var].scope, &typed, value.span);
        self.vars[var].value = Some(typed);
        self.assigned(var);
        Ok(Out::Unit)
    }

    fn return_stmt(&mut self, value: Option<&'r thir::Expr>, span: Span) -> Result<Out> {
        if self.kind == Kind::Successor {
            self.finish_successor(span)?;
            return Ok(Out::Diverged);
        }
        match value {
            Some(expr) => {
                let Some(typed) = self.value(expr)? else {
                    return Ok(Out::Diverged);
                };
                let typed = self.take(typed, expr.span);
                self.ret(Some(typed), span)?;
            }
            None => self.ret(None, span)?,
        }
        Ok(Out::Diverged)
    }

    fn confirm(
        &mut self,
        condition: &'r thir::Expr,
        fallback: Option<&'r thir::Fallback>,
        span: Span,
    ) -> Result<Out> {
        let Some(check) = self.value(condition)? else {
            return Ok(Out::Diverged);
        };
        let passed = self.b.new_block();
        let failed = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: check.value,
                then_target: Target::new(passed),
                else_target: Target::new(failed),
            },
            condition.span,
        );
        self.b.switch_to(failed);
        let env = self.snapshot();
        match fallback {
            None => self.b.terminate(
                TermKind::Trap {
                    code: TrapCode::Confirm,
                    message: "confirm failed".to_string(),
                },
                span,
            ),
            Some(thir::Fallback::Panic(value)) => {
                self.panic(value, span)?;
            }
            Some(thir::Fallback::Throw(value)) => {
                self.throw(value, span)?;
            }
        }
        self.restore(&env);
        self.b.switch_to(passed);
        Ok(Out::Unit)
    }

    fn panic(&mut self, value: &'r thir::Expr, span: Span) -> Result<Out> {
        let Some(message) = self.value(value)? else {
            return Ok(Out::Diverged);
        };
        self.b.terminate(TermKind::Panic(message.value), span);
        Ok(Out::Diverged)
    }

    fn throw(&mut self, value: &'r thir::Expr, span: Span) -> Result<Out> {
        let Some(error) = self.value(value)? else {
            return Ok(Out::Diverged);
        };
        self.b.terminate(TermKind::Throw(error.value), span);
        Ok(Out::Diverged)
    }

    /// Выполнить `stmt` только при истинном условии; ветви сходятся без значения.
    fn guarded(&mut self, condition: &'r thir::Expr, stmt: &'r thir::Stmt) -> Result<Out> {
        // Литеральное условие известно заранее, как в анализаторе Perl.
        if let Some(known) = literal(condition) {
            if !known {
                return Ok(Out::Unit);
            }
            let out = self.stmt(stmt, false)?;
            return Ok(if matches!(out, Out::Diverged) || !self.b.is_open() {
                Out::Diverged
            } else {
                Out::Unit
            });
        }
        let Some(check) = self.value(condition)? else {
            return Ok(Out::Diverged);
        };
        let base = self.vars.len();
        let taken = self.b.new_block();
        let skip = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: check.value,
                then_target: Target::new(taken),
                else_target: Target::new(skip),
            },
            condition.span,
        );
        let env = self.snapshot();
        self.b.switch_to(taken);
        let out = self.stmt(stmt, false)?;
        let mut arms: Vec<Arm> = match out {
            Out::Diverged => Vec::new(),
            _ => self.arm(Out::Unit).into_iter().collect(),
        };
        self.restore(&env);
        self.b.switch_to(skip);
        arms.extend(self.arm(Out::Unit));
        self.join(arms, base, false, condition.span)
    }

    // ----- выражения -----

    /// Выражение в позиции значения; `None` — управление ушло.
    fn value(&mut self, expr: &'r thir::Expr) -> Result<Option<Typed>> {
        match self.expr(expr, true)? {
            Out::Value(value) => Ok(Some(value)),
            Out::Diverged => Ok(None),
            Out::Unit => Err(LoweringError::invalid(
                expr.span,
                "internal compiler error: a value expression produced no value",
            )),
        }
    }

    fn expr(&mut self, expr: &'r thir::Expr, want: bool) -> Result<Out> {
        let span = expr.span;
        match &expr.kind {
            ExprKind::Constant(constant) => {
                let constant = match constant {
                    thir::Constant::Int(text) => {
                        Constant::Int(IntValue::parse(text).ok_or_else(|| {
                            LoweringError::invalid(
                                span,
                                "internal compiler error: malformed integer constant",
                            )
                        })?)
                    }
                    thir::Constant::Float(bits) => Constant::Float(*bits),
                    thir::Constant::Str(text) => Constant::Str(text.clone()),
                    thir::Constant::Char(ch) => Constant::Char(*ch),
                    thir::Constant::Bool(value) => Constant::Bool(*value),
                    thir::Constant::Byte(value) => Constant::Byte(*value),
                };
                Ok(Out::Value(self.constant(constant, span)))
            }
            ExprKind::Local(local) => {
                let var = self.local_var(*local, span)?;
                Ok(Out::Value(
                    self.vars[var]
                        .value
                        .clone()
                        .expect("locals always hold a value"),
                ))
            }
            ExprKind::SelfValue => match self.self_value {
                Some(value) => Ok(Out::Value(Typed {
                    value,
                    ty: Type::Object(self.object.expect("self belongs to an object")),
                })),
                None => self.constructed_self(span),
            },
            ExprKind::Field(field) => Ok(Out::Value(self.read_field(*field, span))),
            ExprKind::PendingField(field) => {
                let var = self.field_vars[*field];
                match self.vars[var].value.clone() {
                    Some(value) => Ok(Out::Value(value)),
                    None => {
                        let object = self.object.expect("constructors belong to an object");
                        Err(LoweringError::invalid(
                            span,
                            format!(
                                "Object \"{}\" field \"{}\" is read before it is bound",
                                self.ctx.objects[object.0].name,
                                self.ctx.objects[object.0].fields[*field].0
                            ),
                        ))
                    }
                }
            }
            ExprKind::EnumCase { ty, case } => Ok(Out::Value(self.emit(
                InstKind::EnumCase {
                    ty: ir::EnumId(ty.0),
                    case: *case,
                },
                Type::Enum(ir::EnumId(ty.0)),
                span,
            ))),
            ExprKind::Unary { op, operand } => self.unary(*op, operand, span),
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right, span),
            ExprKind::Logical { and, left, right } => self.logical(*and, left, right, span),
            ExprKind::Coerce { coercion, value } => self.coercion(coercion, value, expr, span),
            ExprKind::Cast {
                target,
                value,
                position,
            } => {
                // `byte(200)`: целый литерал в 0..255 — сразу константа `byte`.
                if let (
                    thir::CastTarget::Byte,
                    None,
                    ExprKind::Constant(thir::Constant::Int(text)),
                ) = (target, position, &value.kind)
                {
                    if let Ok(byte) = text.parse::<u8>() {
                        return Ok(Out::Value(self.constant(Constant::Byte(byte), span)));
                    }
                }
                let Some(typed) = self.value(value)? else {
                    return Ok(Out::Diverged);
                };
                Ok(Out::Value(self.convert(typed, *target, span, *position)?))
            }
            ExprKind::If {
                condition,
                then_block,
                else_block,
            } => self.if_expr(condition, then_block, else_block.as_ref(), want, span),
            ExprKind::Match(body) => self.match_body(body, want, span),
            ExprKind::Loop(block) => self.loop_expr(block, want, span),
            ExprKind::For {
                source,
                binding,
                body,
            } => self.for_expr(source, *binding, body, want.then_some(expr), span),
            ExprKind::Try(body) => self.try_body(body, want, span),
            ExprKind::Block(block) => self.block(block, want),
            ExprKind::Panic(message) => self.panic(message, span),
            ExprKind::Call { callee, args } => self.call(callee, args, expr, span),
            ExprKind::Array(parts) => self.array_literal(parts, expr, span),
            ExprKind::Map(entries) => self.map_literal(entries, expr, span),
            ExprKind::Index {
                object,
                index,
                fallback,
            } => self.index(object, index, fallback.as_deref(), want, span),
            ExprKind::Functional(functional) => self.functional(functional, expr, span),
            ExprKind::ArrayCast { element, parts } => self.array_cast(*element, parts, span),
            ExprKind::Projection {
                construction,
                method,
                ..
            } => self.projection(construction, *method, expr, span),
            ExprKind::Unpack(value) => self.expr(value, true),
            ExprKind::Pack { items, spread } => self.pack(items, spread.as_deref(), expr, span),
            ExprKind::Unreachable => Err(LoweringError::invalid(
                span,
                "internal compiler error: unreachable expression reached lowering",
            )),
            ExprKind::RatNew {
                numerator,
                denominator,
            } => {
                // Операнды `int` или `byte` (`byte` — через `int`).
                let mut parts = Vec::new();
                for part in [numerator, denominator] {
                    let Some(value) = self.value(part)? else {
                        return Ok(Out::Diverged);
                    };
                    let value = match value.ty {
                        Type::Byte => self.emit(InstKind::Cvt(value.value), Type::Int, span),
                        _ => value,
                    };
                    parts.push(value.value);
                }
                Ok(Out::Value(self.emit(
                    InstKind::Rat {
                        op: ir::RatOp::New,
                        args: parts,
                    },
                    Type::Rat,
                    span,
                )))
            }
            ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                let Some((start, end)) = self.range_bounds(start, end, span)? else {
                    return Ok(Out::Diverged);
                };
                Ok(Out::Value(self.emit(
                    InstKind::RangeNew {
                        start,
                        end,
                        inclusive: *inclusive,
                    },
                    Type::Range,
                    span,
                )))
            }
            ExprKind::Await(inner) => {
                let Some(future) = self.value(inner)? else {
                    return Ok(Out::Diverged);
                };
                let future = self.take(future, inner.span);
                match &expr.ty {
                    None => {
                        self.b.emit_void(InstKind::Await(future.value), span);
                        Ok(Out::Unit)
                    }
                    Some(_) => {
                        let ty = self.expr_type(expr)?;
                        Ok(Out::Value(self.emit(
                            InstKind::Await(future.value),
                            ty,
                            span,
                        )))
                    }
                }
            }
            ExprKind::Spawn(inner) if !matches!(inner.ty, Some(Ty::Future(_))) => {
                self.worker_spawn(inner, expr, span)
            }
            ExprKind::Spawn(inner) => {
                let Some(future) = self.value(inner)? else {
                    return Ok(Out::Diverged);
                };
                Ok(Out::Value(self.emit(
                    InstKind::FutSpawn(future.value),
                    future.ty.clone(),
                    span,
                )))
            }
            ExprKind::Closure { body, captures } => self.closure(body, captures, expr, span),
            ExprKind::BoundAction {
                receiver, method, ..
            } => self.bound_action(receiver, *method, expr, span),
            ExprKind::BoundRoleAction {
                receiver,
                role,
                slot,
            } => self.bound_role_action(receiver, *role, *slot, expr, span),
            ExprKind::CallValue { callee, args } => {
                let Some(target) = self.value(callee)? else {
                    return Ok(Out::Diverged);
                };
                let target = self.take(target, callee.span);
                let Some(values) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                self.finish_call(
                    InstKind::CallFn {
                        callee: target.value,
                        args: values,
                    },
                    expr,
                    span,
                )
            }
            ExprKind::StreamWrite {
                sink,
                source,
                unpack,
            } => self.stream_write(sink, source, *unpack, span),
            ExprKind::CapSelect { owner, .. } => {
                let Some(target) = self.value(owner)? else {
                    return Ok(Out::Diverged);
                };
                let target = self.take(target, owner.span);
                let ty = self.expr_type(expr)?;
                Ok(Out::Value(self.emit(
                    InstKind::CapMake(target.value),
                    ty,
                    span,
                )))
            }
            ExprKind::CapRead { cap, field, .. } => {
                let Some(target) = self.value(cap)? else {
                    return Ok(Out::Diverged);
                };
                let target = self.take(target, cap.span);
                let ty = self.expr_type(expr)?;
                Ok(Out::Value(self.emit(
                    InstKind::CallCap {
                        cap: target.value,
                        field: *field,
                    },
                    ty,
                    span,
                )))
            }
            ExprKind::OwnerField(field) => Ok(Out::Value(self.read_field(*field, span))),
            ExprKind::TestCapability => Err(unsupported(span, "test mode")),
        }
    }

    /// Неявная граница (THIR [`Coercion`]).
    fn coercion(
        &mut self,
        coercion: &Coercion,
        value: &'r thir::Expr,
        expr: &thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let Some(typed) = self.value(value)? else {
            return Ok(Out::Diverged);
        };
        let target = self.expr_type(expr)?;
        Ok(Out::Value(match coercion {
            Coercion::Take => self.take(typed, span),
            Coercion::Retag => match &target {
                Type::Map(..) => self.emit(InstKind::MapRetag(typed.value), target, span),
                _ => self.emit(InstKind::Retag(typed.value), target, span),
            },
            Coercion::RoleWrap => {
                self.role_wrap(typed, value.ty.as_ref(), expr.ty.as_ref(), target, span)?
            }
            Coercion::RoleNarrow { message } => self.emit(
                InstKind::RoleNarrow {
                    value: typed.value,
                    message: message.clone(),
                },
                target,
                span,
            ),
            Coercion::Box => {
                let typed = self.take(typed, span);
                self.emit(InstKind::AnyBox(typed.value), Type::Any, span)
            }
        }))
    }

    /// Граница роли: объект становится ограниченной ссылкой (`role.wrap`).
    /// Анонимное action: снимок захватов (`self` окружающего action — первым)
    /// и функция `[lambda]`, которую lowering выпускает отдельной задачей.
    fn closure(
        &mut self,
        body: &'r thir::Body,
        captures: &[(LocalId, LocalId)],
        expr: &thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let mut values = Vec::new();
        let self_capture = match (self.self_value, self.object) {
            (Some(receiver), Some(_)) if self.kind != Kind::Ctor => {
                values.push(receiver);
                true
            }
            _ => false,
        };
        let mut bound = Vec::new();
        for (outer, inner) in captures {
            let var = self.local_var(*outer, span)?;
            let value = self.vars[var]
                .value
                .clone()
                .expect("captured locals hold a value");
            let value = self.take(value, span);
            values.push(value.value);
            bound.push((*inner, value.ty));
        }
        let ty = self.expr_type(expr)?;
        let Type::Fn(sig) = &ty else {
            return Err(LoweringError::invalid(
                span,
                "internal compiler error: an anonymous action is not a fn value",
            ));
        };
        let index = self.ctx.closures.len();
        self.ctx.closures.push(super::ctx::ClosureTask {
            body,
            env: self.env.clone(),
            object: self.object,
            self_capture,
            captures: bound,
            sig: (**sig).clone(),
        });
        let func = self
            .ctx
            .allocate(Task::Closure { index }, format!("lambda#{index}"));
        Ok(Out::Value(self.emit(
            InstKind::FnClosure {
                func,
                captures: values,
            },
            ty,
            span,
        )))
    }

    /// `receiver.action` без вызова: `fn.closure` над action с receiver.
    fn bound_action(
        &mut self,
        receiver: &'r thir::Expr,
        method: usize,
        expr: &thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let Some(target) = self.value(receiver)? else {
            return Ok(Out::Diverged);
        };
        let target = self.take(target, receiver.span);
        let Type::Object(object) = target.ty else {
            return Err(unsupported(span, "bound action values of this receiver"));
        };
        let info = &self.ctx.reg.objects[self.ctx.objects[object.0].decl.0].methods[method];
        if info.successor {
            return Err(unsupported(span, "bound successor actions"));
        }
        let is_async = info.is_async;
        let ty = self.expr_type(expr)?;
        // Программа запускает значения-action в воркерах: синхронное bound
        // action несёт вход воркера и поля receiver для его восстановления.
        let worker = self.ctx.reg.worker_actions && !is_async;
        if worker && self.ctx.worker_ctor(object).is_some() {
            let bind = self.ctx.worker_bind(object, method);
            return Ok(Out::Value(self.emit(
                InstKind::Call {
                    func: bind,
                    args: vec![target.value],
                },
                ty,
                span,
            )));
        }
        let func = self.ctx.method_func(object, method, span)?;
        Ok(Out::Value(self.emit(
            InstKind::FnClosure {
                func,
                captures: vec![target.value],
            },
            ty,
            span,
        )))
    }

    /// `spawn` синхронного действия (§18.1): аргументы вычисляются здесь, само
    /// действие исполняет процесс-воркер.
    fn worker_spawn(
        &mut self,
        inner: &'r thir::Expr,
        expr: &thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let ty = self.expr_type(expr)?;
        self.worker_spawn_values(inner, ty, span)
    }

    fn worker_spawn_values(&mut self, inner: &'r thir::Expr, ty: Type, span: Span) -> Result<Out> {
        match &inner.kind {
            ExprKind::Call {
                callee:
                    thir::Callee::Method {
                        receiver, method, ..
                    },
                args,
            } => {
                let ExprKind::Call {
                    callee:
                        thir::Callee::Ctor {
                            object,
                            type_args,
                            ctor,
                            given,
                        },
                    args: ctor_args,
                } = &receiver.kind
                else {
                    return Err(unsupported(span, "spawn of this receiver"));
                };
                let type_args: Vec<Ty> = type_args.iter().map(|arg| arg.subst(&self.env)).collect();
                let instance = self.ctx.object_instance(*object, type_args, span)?;
                let info =
                    &self.ctx.reg.objects[self.ctx.objects[instance.0].decl.0].methods[*method];
                if info.successor {
                    return Err(unsupported(span, "spawn of successor actions"));
                }
                let Some(mut values) = self.arguments(ctor_args, span)? else {
                    return Ok(Out::Diverged);
                };
                let Some(rest) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                values.extend(rest);
                // Анализ проверил конкретные типы; здесь — параметры типа после подстановки.
                let mut crossing: Vec<Type> = values
                    .iter()
                    .map(|value| self.b.value_type(*value).clone())
                    .collect();
                if let Type::Future(result) = &ty {
                    if let ResultType::Value(result) = &**result {
                        crossing.push(result.clone());
                    }
                }
                if let Some(bad) = crossing.iter().find(|value| !transferable(value)) {
                    return Err(LoweringError::invalid(
                        span,
                        format!("spawn cannot transfer value of type {bad}"),
                    ));
                }
                let func = self
                    .ctx
                    .worker_entry(instance, *ctor, given.clone(), *method, false);
                let Type::Future(result) = &ty else {
                    return Err(LoweringError::invalid(
                        span,
                        "internal compiler error: spawn does not produce a future",
                    ));
                };
                let params = values
                    .iter()
                    .map(|value| self.b.value_type(*value).clone())
                    .collect();
                let entry = self.emit(
                    InstKind::FnClosure {
                        func,
                        captures: Vec::new(),
                    },
                    Type::Fn(Box::new(ir::FnSig {
                        params,
                        result: (**result).clone(),
                    })),
                    span,
                );
                Ok(Out::Value(self.emit(
                    InstKind::WorkerSpawn {
                        entry: entry.value,
                        args: values,
                        direct: true,
                    },
                    ty,
                    span,
                )))
            }
            ExprKind::CallValue { callee, args } => {
                let Some(target) = self.value(callee)? else {
                    return Ok(Out::Diverged);
                };
                let target = self.take(target, callee.span);
                let Some(values) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                Ok(Out::Value(self.emit(
                    InstKind::WorkerSpawn {
                        entry: target.value,
                        args: values,
                        direct: false,
                    },
                    ty,
                    span,
                )))
            }
            _ => Err(unsupported(span, "spawn of this call form")),
        }
    }

    /// Bound action ограниченной ссылки роли: замыкание над thunk, который
    /// вызывает действие роли через таблицу ссылки.
    fn bound_role_action(
        &mut self,
        receiver: &'r thir::Expr,
        role: crate::analysis::ty::RoleId,
        slot: usize,
        expr: &thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let Some(target) = self.value(receiver)? else {
            return Ok(Out::Diverged);
        };
        let target = self.take(target, receiver.span);
        let methods = &self.ctx.reg.roles[role.0].methods;
        let method = &methods[slot];
        let (name, arity) = (method.name.clone(), method.params.len());
        if methods
            .iter()
            .filter(|other| other.name == name && other.params.len() == arity)
            .count()
            > 1
        {
            return Err(unsupported(
                span,
                "bound action values of overloaded role actions",
            ));
        }
        let func = self.ctx.member_thunk(target.ty.clone(), &name, arity, true);
        let ty = self.expr_type(expr)?;
        Ok(Out::Value(self.emit(
            InstKind::FnClosure {
                func,
                captures: vec![target.value],
            },
            ty,
            span,
        )))
    }

    /// Граница роли: объект оборачивается сам, массив и скаляр — через
    /// адаптер (`adapters.rs`); `from`/`to` — типы анализа значения и роли.
    pub(super) fn role_wrap(
        &mut self,
        typed: Typed,
        from: Option<&Ty>,
        to: Option<&Ty>,
        target: Type,
        span: Span,
    ) -> Result<Typed> {
        if matches!(target, Type::Successor(_)) {
            return Err(unsupported(span, "successor role actions"));
        }
        let typed = self.take(typed, span);
        if typed.ty == target {
            return Ok(typed);
        }
        if matches!(typed.ty, Type::Object(_)) {
            return Ok(self.emit(InstKind::RoleWrap(typed.value), target, span));
        }
        let (Some(from), Some(to)) = (from, to) else {
            return Err(unsupported(
                span,
                "builtin role adapters (arrays and scalars as roles)",
            ));
        };
        let (from, to) = (from.subst(&self.env).taken(), to.subst(&self.env));
        let Some((decl, args)) = self.adapter_for(&from, &to) else {
            return Err(unsupported(
                span,
                "builtin role adapters (arrays and scalars as roles)",
            ));
        };
        let adapter = self.ctx.object_instance(decl, args, span)?;
        // `[]` без тега получает тип поля адаптера.
        let field = self.ctx.objects[adapter.0].fields[0].1.clone();
        let value = if typed.ty == field {
            typed
        } else {
            self.emit(InstKind::Retag(typed.value), field, span)
        };
        let ctor = self.ctx.adapter_ctor(adapter);
        let object = self.emit(
            InstKind::Call {
                func: ctor,
                args: vec![value.value],
            },
            Type::Object(adapter),
            span,
        );
        Ok(self.emit(InstKind::RoleWrap(object.value), target, span))
    }

    /// Адаптер встроенного `from` к роли `to` и его специализация.
    fn adapter_for(&self, from: &Ty, to: &Ty) -> Option<(crate::analysis::ty::ObjectId, Vec<Ty>)> {
        let Ty::Role(role, args) = to else {
            return None;
        };
        let reg = self.ctx.reg;
        let (standard, adapters) = (reg.standard, reg.adapters);
        match from {
            Ty::Array(element) if Some(*role) == standard.byte_source && **element == Ty::Byte => {
                Some((adapters.byte_array_source?, Vec::new()))
            }
            Ty::Array(element)
                if [standard.sequencing, standard.iterating, standard.sizing]
                    .contains(&Some(*role)) =>
            {
                let mut target = args.first().cloned().unwrap_or_else(|| (**element).clone());
                // `[]` как `Sizing`: тип элемента не наблюдаем — `@int` после retag.
                if target == Ty::Never {
                    target = Ty::Int;
                }
                // `[]` — элементы типа роли.
                let source = if **element == Ty::Never {
                    target.clone()
                } else {
                    (**element).clone()
                };
                Some((adapters.array_sequence?, vec![source, target]))
            }
            Ty::Str if Some(*role) == standard.byte_source => {
                Some((adapters.string_source?, Vec::new()))
            }
            Ty::Int | Ty::Float | Ty::Rat | Ty::Byte | Ty::Str | Ty::Char
                if Some(*role) == standard.comparing =>
            {
                Some((adapters.scalar_order?, vec![from.clone()]))
            }
            _ => None,
        }
    }

    fn if_expr(
        &mut self,
        condition: &'r thir::Expr,
        then_block: &'r thir::Block,
        else_block: Option<&'r thir::Block>,
        want: bool,
        span: Span,
    ) -> Result<Out> {
        // Литеральное условие: исполняемая ветвь известна, другая не выпускается.
        if let Some(known) = literal(condition) {
            if known {
                return self.branch(then_block, want);
            }
            return match else_block {
                Some(block) => self.branch(block, want),
                None => Ok(Out::Unit),
            };
        }
        let Some(check) = self.value(condition)? else {
            return Ok(Out::Diverged);
        };
        let base = self.vars.len();
        let then_start = self.b.new_block();
        let else_start = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: check.value,
                then_target: Target::new(then_start),
                else_target: Target::new(else_start),
            },
            condition.span,
        );
        let env = self.snapshot();
        self.b.switch_to(then_start);
        let then_out = self.branch(then_block, want)?;
        let mut arms: Vec<Arm> = self.arm(then_out).into_iter().collect();
        self.restore(&env);
        self.b.switch_to(else_start);
        let else_out = match else_block {
            Some(block) => self.branch(block, want)?,
            None => Out::Unit,
        };
        arms.extend(self.arm(else_out));
        self.join(arms, base, want, span)
    }

    // ----- циклы -----

    /// Переменные, которые тело присваивает: они станут параметрами заголовка.
    /// Значение `^C` гасится до входа: параметр заголовка — `C`.
    fn carried_vars(&mut self, body: &thir::Block, span: Span) -> Result<Vec<usize>> {
        let mut locals = Vec::new();
        let mut fields = Vec::new();
        assigned_in_block(body, &mut locals, &mut fields);
        let mut carried: Vec<usize> = Vec::new();
        for local in locals {
            if let Some(var) = self.local_vars[local.0] {
                if !carried.contains(&var) {
                    carried.push(var);
                }
            }
        }
        for field in fields {
            if let Some(var) = self.field_vars.get(field).copied() {
                if !carried.contains(&var) {
                    carried.push(var);
                }
            }
        }
        carried.sort_unstable();
        for var in &carried {
            match (self.vars[*var].value.clone(), self.vars[*var].kind) {
                (None, VarKind::Field) => {
                    return Err(LoweringError::invalid(
                        span,
                        "internal compiler error: a loop rebinds an unbound field",
                    ));
                }
                (None, VarKind::Staged(field)) => {
                    let original = self.read_field(field, span);
                    self.vars[*var].value = Some(original);
                }
                (Some(value), _) if matches!(value.ty, Type::Successor(_)) => {
                    self.take(value, span);
                }
                _ => {}
            }
        }
        Ok(carried)
    }

    /// Новый блок с параметрами `extra` и параметрами для `carried`.
    fn block_for_vars(&mut self, extra: &[Type], carried: &[usize]) -> (BlockId, Vec<ValueId>) {
        let block = self.b.new_block();
        let mut params = Vec::new();
        for ty in extra {
            params.push(self.b.add_param(block, ty.clone()));
        }
        for var in carried {
            let ty = self.vars[*var]
                .value
                .as_ref()
                .expect("carried vars hold values")
                .ty
                .clone();
            params.push(self.b.add_param(block, ty));
        }
        (block, params)
    }

    /// Значения переносимых переменных (типа заголовка: `^C` гасится).
    fn carried_args(&mut self, carried: &[usize], span: Span) -> Vec<ValueId> {
        let mut args = Vec::new();
        for var in carried {
            let value = self.vars[*var]
                .value
                .clone()
                .expect("carried vars hold values");
            let value = if matches!(value.ty, Type::Successor(_)) {
                self.take(value, span)
            } else {
                value
            };
            args.push(value.value);
        }
        args
    }

    fn bind_carried(&mut self, carried: &[usize], params: &[ValueId]) {
        for (var, param) in carried.iter().zip(params) {
            let ty = self.b.value_type(*param).clone();
            self.vars[*var].value = Some(Typed { value: *param, ty });
        }
        self.sync_unwind();
    }

    fn loop_expr(&mut self, body: &'r thir::Block, want: bool, span: Span) -> Result<Out> {
        let base = self.vars.len();
        let carried = self.carried_vars(body, span)?;
        let (head, params) = self.block_for_vars(&[], &carried);
        let args = self.carried_args(&carried, span);
        self.b
            .terminate(TermKind::Br(Target { block: head, args }), span);
        self.b.switch_to(head);
        self.bind_carried(&carried, &params);
        self.loops.push(LoopFrame {
            next: head,
            state: Vec::new(),
            carried: carried.clone(),
            exits: Vec::new(),
            base,
            depth: self.scopes.len(),
            want,
        });
        // Значение последнего оператора тела `loop` отбрасывается.
        let body_out = self.block(body, false);
        if body_out.is_ok() && self.b.is_open() {
            let args = self.carried_args(&carried, span);
            self.b
                .terminate(TermKind::Br(Target { block: head, args }), span);
        }
        let frame = self.loops.pop().expect("loop frame pushed above");
        body_out?;
        self.join(frame.exits, frame.base, want, span)
    }

    fn next(&mut self, span: Span) -> Result<Out> {
        let frame = self.loops.len() - 1;
        let depth = self.loops[frame].depth;
        if !self.exit_scopes(depth, None, span)? {
            return Ok(Out::Diverged);
        }
        let target = self.loops[frame].next;
        let carried = self.loops[frame].carried.clone();
        let mut args = self.loops[frame].state.clone();
        args.extend(self.carried_args(&carried, span));
        self.b.terminate(
            TermKind::Br(Target {
                block: target,
                args,
            }),
            span,
        );
        Ok(Out::Diverged)
    }

    fn last(&mut self, value: Option<&'r thir::Expr>, span: Span) -> Result<Out> {
        let frame = self.loops.len() - 1;
        let want = self.loops[frame].want;
        let out = match value {
            Some(expr) => {
                // Значение вычисляется только на этом пути.
                match self.expr(expr, want)? {
                    Out::Diverged => return Ok(Out::Diverged),
                    Out::Value(typed) if want => Out::Value(typed),
                    _ => Out::Unit,
                }
            }
            None => Out::Unit,
        };
        let depth = self.loops[frame].depth;
        let payload = match &out {
            Out::Value(value) => Some(value.clone()),
            _ => None,
        };
        if !self.exit_scopes(depth, payload.as_ref(), span)? {
            return Ok(Out::Diverged);
        }
        // Значение `last` освобождено регионами итерации: его учитывает
        // область, содержащая цикл.
        if let Some(value) = &payload {
            if let Some(region) = self.region_at(depth.saturating_sub(1)) {
                if !self.ctx.is_plain(&value.ty) {
                    self.b.emit_void(
                        InstKind::RegionTrack {
                            region,
                            value: value.value,
                        },
                        span,
                    );
                }
            }
        }
        let arm = self.arm(out).expect("`last` is reached in an open block");
        self.loops[frame].exits.push(arm);
        self.b.close();
        Ok(Out::Diverged)
    }

    // ----- блоки с параметрами (try, lookup) -----

    /// Блок с параметрами для `tracked` (типы `types`) и дополнительными.
    fn block_with(&mut self, types: &[Type], extra: &[Type]) -> (BlockId, Vec<ValueId>) {
        let block = self.b.new_block();
        let params = types
            .iter()
            .chain(extra)
            .map(|ty| self.b.add_param(block, ty.clone()))
            .collect();
        (block, params)
    }

    /// Параметры блока для отслеживаемых переменных наследуют связь
    /// погашенного значения с исходным `^C` (см. `taken_from`).
    pub(super) fn inherit_taken(&mut self, tracked: &[usize], params: &[ValueId]) {
        for (var, param) in tracked.iter().zip(params) {
            let Some(value) = self.vars[*var].value.as_ref().map(|typed| typed.value) else {
                continue;
            };
            if let Some(original) = self.taken_from.get(&value).copied() {
                self.taken_from.insert(*param, original);
            }
        }
    }

    fn bind_tracked(&mut self, tracked: &[usize], types: &[Type], params: &[ValueId]) {
        for ((var, ty), param) in tracked.iter().zip(types).zip(params) {
            self.vars[*var].value = Some(Typed {
                value: *param,
                ty: ty.clone(),
            });
        }
        self.sync_unwind();
    }

    fn tracked_args(&self, tracked: &[usize]) -> Vec<ValueId> {
        tracked
            .iter()
            .map(|var| self.vars[*var].value.as_ref().expect("tracked").value)
            .collect()
    }

    /// Закончить область отказа: следующий код — в новом блоке вне её.
    fn leave_try(&mut self) {
        self.tries.pop();
        if self.b.is_open() {
            let next = self.b.new_block();
            self.b
                .terminate(TermKind::Br(Target::new(next)), Span::empty(0));
            self.b.switch_to(next);
        }
        self.sync_unwind();
    }
}

/// `true`/`false` литерального условия.
fn literal(expr: &thir::Expr) -> Option<bool> {
    match &expr.kind {
        ExprKind::Constant(thir::Constant::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn unsupported(span: Span, what: &str) -> LoweringError {
    LoweringError::unsupported(
        span,
        format!("IR lowering of {what} is not implemented yet."),
    )
}

/// Локальные и поля, которым тело присваивает значение (rebind, поле, `^field`).
fn assigned_in_block(block: &thir::Block, locals: &mut Vec<LocalId>, fields: &mut Vec<usize>) {
    for stmt in &block.stmts {
        assigned_in_stmt(stmt, locals, fields);
    }
}

fn assigned_in_stmt(stmt: &thir::Stmt, locals: &mut Vec<LocalId>, fields: &mut Vec<usize>) {
    match &stmt.kind {
        StmtKind::Rebind { local, value } => {
            locals.push(*local);
            assigned_in_expr(value, locals, fields);
        }
        StmtKind::BindField { field, value } | StmtKind::StageField { field, value } => {
            fields.push(*field);
            assigned_in_expr(value, locals, fields);
        }
        StmtKind::Let { value, .. }
        | StmtKind::Expr(value)
        | StmtKind::Defer(value)
        | StmtKind::Throw(value) => assigned_in_expr(value, locals, fields),
        StmtKind::Guarded { condition, stmt } => {
            assigned_in_expr(condition, locals, fields);
            assigned_in_stmt(stmt, locals, fields);
        }
        StmtKind::Return(Some(value)) | StmtKind::Last(Some(value)) => {
            assigned_in_expr(value, locals, fields)
        }
        StmtKind::Confirm {
            condition,
            fallback,
        } => {
            assigned_in_expr(condition, locals, fields);
            if let Some(thir::Fallback::Throw(value) | thir::Fallback::Panic(value)) = fallback {
                assigned_in_expr(value, locals, fields);
            }
        }
        StmtKind::Return(None) | StmtKind::Last(None) | StmtKind::Next | StmtKind::Rethrow => {}
    }
}

fn assigned_in_expr(expr: &thir::Expr, locals: &mut Vec<LocalId>, fields: &mut Vec<usize>) {
    match &expr.kind {
        ExprKind::Block(block) | ExprKind::Loop(block) => assigned_in_block(block, locals, fields),
        ExprKind::For { body, .. } => assigned_in_block(body, locals, fields),
        ExprKind::If {
            condition,
            then_block,
            else_block,
        } => {
            assigned_in_expr(condition, locals, fields);
            assigned_in_block(then_block, locals, fields);
            if let Some(block) = else_block {
                assigned_in_block(block, locals, fields);
            }
        }
        ExprKind::Match(body) => {
            for (_, block) in &body.cases {
                assigned_in_block(block, locals, fields);
            }
            if let Some(block) = &body.else_block {
                assigned_in_block(block, locals, fields);
            }
        }
        ExprKind::Try(body) => {
            assigned_in_block(&body.body, locals, fields);
            for catch in &body.catches {
                assigned_in_block(&catch.body, locals, fields);
            }
        }
        ExprKind::Functional(functional) => assigned_in_block(&functional.body, locals, fields),
        // Тело анонимного действия — отдельная функция.
        _ => {}
    }
}

/// Тип пересекает границу процесса-воркера (§18.1).
fn transferable(ty: &Type) -> bool {
    match ty {
        Type::Int
        | Type::Float
        | Type::Str
        | Type::Char
        | Type::Byte
        | Type::Bool
        | Type::Never => true,
        Type::Array(element) => transferable(element),
        Type::Map(key, value) => transferable(key) && transferable(value),
        _ => false,
    }
}
