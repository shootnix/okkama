//! Проверки потока по THIR (после типизации).
//!
//! **Обязательство successor (§11.1).** Каждое значение `^C`, созданное
//! вызовом, получает своё обязательство. Привязки разделяют обязательство
//! значения (алиасы), погашает его явный узел `Take` (receiver, аргумент,
//! возврат, rebind, привязка поля, агрегат, `throw`). Отброшенный оператор с
//! непогашенным значением, привязка, покидающая область с непогашенным
//! значением, перезапись такой привязки и `return` с непогашенными
//! обязательствами — ошибки. В точке слияния обязательство непогашено, если
//! оно непогашено хоть на одном пути; тело цикла может не выполниться, поэтому
//! погашение внутри тела не снимает внешнее обязательство. `throw` и `panic`
//! вправе бросить обязательства: действие не завершилось успешно.
//!
//! **Поля конструктора (§11.3).** Поле, не связанное ни на одном пути к концу
//! конструктора, — ошибка; связанное не на всех путях проверяется во время
//! исполнения. Чтение `self.field` до связывания на всех путях — ошибка.

use std::collections::BTreeSet;

use super::registry::{CtorShape, Registry};
use super::thir::{
    ArrayPart, Block, Body, BodyOwner, Callee, CastPart, Coercion, Expr, ExprKind, Fallback,
    LocalId, Source, Stmt, StmtKind,
};
use super::ty::Ty;
use crate::source::Span;

type Obligations = BTreeSet<u32>;

/// Связано ли поле строящегося объекта.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Bound {
    No,
    Maybe,
    Yes,
}

impl Bound {
    fn meet(self, other: Bound) -> Bound {
        if self == other { self } else { Bound::Maybe }
    }
}

#[derive(Clone, Debug)]
struct State {
    /// Непогашенные обязательства.
    pending: Obligations,
    /// Обязательства значения каждой локальной.
    holds: Vec<Obligations>,
    fields: Vec<Bound>,
}

impl State {
    fn join(states: Vec<State>) -> Option<State> {
        let mut iter = states.into_iter();
        let mut merged = iter.next()?;
        for state in iter {
            merged.pending.extend(state.pending);
            for (held, other) in merged.holds.iter_mut().zip(state.holds) {
                held.extend(other);
            }
            for (field, other) in merged.fields.iter_mut().zip(state.fields) {
                *field = field.meet(other);
            }
        }
        Some(merged)
    }
}

/// Активный цикл: состояния выходов.
struct LoopFrame {
    /// Глубина областей при входе в тело.
    depth: usize,
    exits: Vec<State>,
    /// Состояния `next` и конца итерации (для `for` — тоже выходы).
    continues: Vec<State>,
}

struct Flow<'r, 'a> {
    reg: &'r mut Registry<'a>,
    module: usize,
    body: &'r Body,
    next_obligation: u32,
    /// Локальные открытых областей (внешние раньше).
    scopes: Vec<Vec<LocalId>>,
    /// `defer` открытых областей.
    defers: Vec<Vec<&'r Expr>>,
    loops: Vec<LoopFrame>,
    /// Объект строящегося конструктора.
    ctor_object: Option<super::ty::ObjectId>,
}

/// Проверки потока одного тела; ошибки — в `reg`.
pub(super) fn check(reg: &mut Registry<'_>, body: &Body) {
    let (fields, ctor_object) = initial_fields(reg, &body.owner);
    let mut flow = Flow {
        module: body.module,
        reg,
        body,
        next_obligation: 0,
        scopes: vec![body.params.clone()],
        defers: vec![Vec::new()],
        loops: Vec::new(),
        ctor_object,
    };
    let mut state = State {
        pending: Obligations::new(),
        holds: vec![Obligations::new(); body.locals.len()],
        fields,
    };
    let result = flow.block_inner(&body.block, body.returns_value, &mut state, false);
    if let Some(returned) = result {
        // Нормальный конец тела: `defer` корня засчитываются, затем возврат.
        flow.run_defers(&mut state);
        flow.check_return(&state, &returned, body.span);
        if flow.ctor_object.is_some() {
            flow.check_fields_complete(&state, body.span);
        }
    }
}

/// Литеральное условие `true`/`false`: другая ветвь не исполняется.
fn literal(expr: &Expr) -> Option<bool> {
    match &expr.kind {
        ExprKind::Constant(super::thir::Constant::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn initial_fields(
    reg: &Registry<'_>,
    owner: &BodyOwner,
) -> (Vec<Bound>, Option<super::ty::ObjectId>) {
    let BodyOwner::Ctor { object, ctor } = owner else {
        return (Vec::new(), None);
    };
    let info = &reg.objects[object.0];
    let listed: Vec<usize> = match &info.ctors[*ctor].shape {
        CtorShape::Shorthand { fields, .. } => fields.clone(),
        CtorShape::Full { .. } => Vec::new(),
    };
    let fields = info
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            if field.default.is_some() || listed.contains(&index) {
                Bound::Yes
            } else {
                Bound::No
            }
        })
        .collect();
    (fields, Some(*object))
}

impl<'r> Flow<'r, '_> {
    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.reg.set_module(self.module);
        self.reg.error(span, message);
    }

    fn fresh(&mut self) -> u32 {
        self.next_obligation += 1;
        self.next_obligation
    }

    fn local_name(&self, local: LocalId) -> &str {
        &self.body.locals[local.0].name
    }

    /// Обязательства, которые держат локальные областей `..depth` (и `except`).
    fn held_outside(&self, state: &State, depth: usize) -> Obligations {
        let mut held = Obligations::new();
        for scope in &self.scopes[..depth] {
            for local in scope {
                held.extend(state.holds[local.0].iter().copied());
            }
        }
        held
    }

    fn held_by_any(&self, state: &State, obligations: &Obligations) -> bool {
        self.scopes
            .iter()
            .flatten()
            .any(|local| !state.holds[local.0].is_disjoint(obligations))
    }

    /// Непогашенные обязательства привязок областей `depth..` не должны их
    /// пережить (кроме значения `kept`). Как Perl, называется самая поздняя
    /// привязка.
    fn check_scopes(&mut self, state: &mut State, depth: usize, kept: &Obligations, span: Span) {
        let outer = self.held_outside(state, depth);
        let mut locals: Vec<LocalId> = self.scopes[depth..].iter().flatten().copied().collect();
        locals.sort_by_key(|local| std::cmp::Reverse(local.0));
        for local in locals {
            let leaking: Obligations = state.holds[local.0]
                .iter()
                .copied()
                .filter(|ob| {
                    state.pending.contains(ob) && !outer.contains(ob) && !kept.contains(ob)
                })
                .collect();
            if !leaking.is_empty() {
                let message = format!(
                    "Successor action result must be used; binding \"{}\" can leave scope without consumption",
                    self.local_name(local)
                );
                self.error(span, message);
                // Одно сообщение на обязательство.
                for ob in leaking {
                    state.pending.remove(&ob);
                }
            }
        }
    }

    fn check_return(&mut self, state: &State, returned: &Obligations, span: Span) {
        let mut state = state.clone();
        self.check_scopes(&mut state, 0, returned, span);
    }

    fn check_fields_complete(&mut self, state: &State, span: Span) {
        let Some(object) = self.ctor_object else {
            return;
        };
        if let Some(field) = state.fields.iter().position(|bound| *bound == Bound::No) {
            let info = &self.reg.objects[object.0];
            let message = format!(
                "Object \"{}\" field \"{}\" was not initialized",
                info.name, info.fields[field].name
            );
            self.error(span, message);
        }
    }

    /// Отброшенное значение с непогашенным обязательством, которое не держит
    /// привязка, — ошибка.
    fn discard(&mut self, state: &mut State, value: &Obligations, span: Span) {
        let pending: Obligations = value.intersection(&state.pending).copied().collect();
        if !pending.is_empty() && !self.held_by_any(state, &pending) {
            self.error(span, "Successor action result must be used");
            for ob in pending {
                state.pending.remove(&ob);
            }
        }
    }

    // ----- блоки и операторы -----

    /// Блок с новой областью; `None` — управление ушло.
    fn block(&mut self, block: &'r Block, want: bool, state: &mut State) -> Option<Obligations> {
        self.block_inner(block, want, state, true)
    }

    fn block_inner(
        &mut self,
        block: &'r Block,
        want: bool,
        state: &mut State,
        scoped: bool,
    ) -> Option<Obligations> {
        if scoped {
            self.scopes.push(Vec::new());
            self.defers.push(Vec::new());
        }
        let mut value = Some(Obligations::new());
        let count = block.stmts.len();
        for (index, stmt) in block.stmts.iter().enumerate() {
            let last = index + 1 == count;
            match self.stmt(stmt, want && last, state) {
                Some(result) => {
                    if last {
                        value = Some(result);
                    }
                }
                None => {
                    // Мёртвый код после ухода управления: уже проверен типами.
                    value = None;
                    break;
                }
            }
        }
        if scoped {
            if let Some(kept) = &value {
                let kept = if want {
                    kept.clone()
                } else {
                    Obligations::new()
                };
                self.run_defers(state);
                let depth = self.scopes.len() - 1;
                self.check_scopes(state, depth, &kept, block.span);
            }
            self.scopes.pop();
            self.defers.pop();
        }
        value.map(|kept| if want { kept } else { Obligations::new() })
    }

    /// `defer` области засчитываются при её выходе (§11.1).
    fn run_defers(&mut self, state: &mut State) {
        let defers: Vec<&Expr> = self.defers.last().cloned().unwrap_or_default();
        for defer in defers.into_iter().rev() {
            if let Some(value) = self.expr(defer, false, state) {
                self.discard(state, &value, defer.span);
            }
        }
    }

    /// Оператор; `None` — управление ушло.
    fn stmt(&mut self, stmt: &'r Stmt, want: bool, state: &mut State) -> Option<Obligations> {
        match &stmt.kind {
            StmtKind::Let { local, value } => {
                let held = self.expr(value, true, state)?;
                state.holds[local.0] = held.clone();
                if let Some(scope) = self.scopes.last_mut() {
                    scope.push(*local);
                }
                Some(held)
            }
            StmtKind::Expr(expr) => {
                let value = self.expr(expr, want, state)?;
                if !want {
                    self.discard(state, &value, expr.span);
                    return Some(Obligations::new());
                }
                Some(value)
            }
            StmtKind::Guarded { condition, stmt } => {
                self.expr(condition, true, state)?;
                match literal(condition) {
                    Some(true) => {
                        self.stmt(stmt, false, state)?;
                        return Some(Obligations::new());
                    }
                    Some(false) => return Some(Obligations::new()),
                    None => {}
                }
                let mut taken = state.clone();
                let after = self.stmt(stmt, false, &mut taken).map(|_| taken);
                let mut arms = vec![state.clone()];
                arms.extend(after);
                *state = State::join(arms).expect("the skipped path is live");
                Some(Obligations::new())
            }
            StmtKind::Return(value) => {
                let returned = match value {
                    Some(value) => self.expr(value, true, state)?,
                    None => Obligations::new(),
                };
                self.check_return(state, &returned, stmt.span);
                None
            }
            StmtKind::Confirm {
                condition,
                fallback,
            } => {
                self.expr(condition, true, state)?;
                let mut failed = state.clone();
                match fallback {
                    Some(Fallback::Throw(value) | Fallback::Panic(value)) => {
                        let _ = self.expr(value, true, &mut failed);
                    }
                    None => {}
                }
                Some(Obligations::new())
            }
            StmtKind::Throw(value) => {
                let _ = self.expr(value, true, state);
                None
            }
            StmtKind::Rethrow => None,
            StmtKind::BindField { field, value } | StmtKind::StageField { field, value } => {
                self.expr(value, true, state)?;
                if let Some(bound) = state.fields.get_mut(*field) {
                    *bound = Bound::Yes;
                }
                Some(Obligations::new())
            }
            StmtKind::Rebind { local, value } => {
                let new = self.expr(value, true, state)?;
                // Перезапись не гасит прежнее значение, если его не держит алиас.
                let old: Obligations = state.holds[local.0]
                    .intersection(&state.pending)
                    .copied()
                    .collect();
                if !old.is_empty() {
                    let others = self
                        .scopes
                        .iter()
                        .flatten()
                        .filter(|other| **other != *local)
                        .any(|other| !state.holds[other.0].is_disjoint(&old));
                    if !others {
                        let message = format!(
                            "Successor action result must be used; binding \"{}\" can leave scope without consumption",
                            self.local_name(*local)
                        );
                        self.error(stmt.span, message);
                        for ob in old {
                            state.pending.remove(&ob);
                        }
                    }
                }
                state.holds[local.0] = new;
                Some(Obligations::new())
            }
            StmtKind::Defer(value) => {
                // Сам `defer` ничего не вычисляет; выражение — при выходе области.
                if let Some(scope) = self.defers.last_mut() {
                    scope.push(value);
                }
                Some(Obligations::new())
            }
            StmtKind::Next => {
                self.loop_jump(state, stmt.span, None);
                None
            }
            StmtKind::Last(value) => {
                let kept = match value {
                    Some(value) => self.expr(value, true, state)?,
                    None => Obligations::new(),
                };
                self.loop_jump(state, stmt.span, Some(kept));
                None
            }
        }
    }

    /// `next`/`last`: обязательства итерации гасятся до перехода.
    fn loop_jump(&mut self, state: &State, span: Span, last: Option<Obligations>) {
        let Some(depth) = self.loops.last().map(|frame| frame.depth) else {
            return;
        };
        let mut jumped = state.clone();
        let kept = last.clone().unwrap_or_default();
        self.check_scopes(&mut jumped, depth, &kept, span);
        let frame = self.loops.last_mut().expect("checked above");
        if last.is_some() {
            frame.exits.push(jumped);
        } else {
            frame.continues.push(jumped);
        }
    }

    // ----- выражения -----

    /// Обязательства значения выражения; `None` — управление ушло.
    fn expr(&mut self, expr: &'r Expr, want: bool, state: &mut State) -> Option<Obligations> {
        let value = self.expr_inner(expr, want, state)?;
        if expr.diverges {
            return None;
        }
        // Вызов, дающий `^C`, создаёт новое обязательство.
        let produces = matches!(
            expr.kind,
            ExprKind::Call { .. }
                | ExprKind::CallValue { .. }
                | ExprKind::Await(_)
                | ExprKind::Projection { .. }
        ) && matches!(expr.ty, Some(Ty::Successor(_)));
        if produces {
            let ob = self.fresh();
            state.pending.insert(ob);
            return Some(Obligations::from([ob]));
        }
        Some(value)
    }

    fn values(
        &mut self,
        exprs: impl IntoIterator<Item = &'r Expr>,
        state: &mut State,
    ) -> Option<()> {
        for expr in exprs {
            self.expr(expr, true, state)?;
        }
        Some(())
    }

    fn expr_inner(&mut self, expr: &'r Expr, want: bool, state: &mut State) -> Option<Obligations> {
        let none = || Some(Obligations::new());
        match &expr.kind {
            ExprKind::Constant(_)
            | ExprKind::SelfValue
            | ExprKind::Field(_)
            | ExprKind::EnumCase { .. }
            | ExprKind::OwnerField(_)
            | ExprKind::TestCapability
            | ExprKind::Unreachable => none(),
            ExprKind::PendingField(field) => {
                if state.fields.get(*field) == Some(&Bound::No) {
                    if let Some(object) = self.ctor_object {
                        let info = &self.reg.objects[object.0];
                        let message = format!(
                            "Object \"{}\" field \"{}\" is read before it is bound",
                            info.name, info.fields[*field].name
                        );
                        self.error(expr.span, message);
                    }
                }
                none()
            }
            ExprKind::Local(local) => Some(state.holds[local.0].clone()),
            ExprKind::Coerce { coercion, value } => {
                let held = self.expr(value, true, state)?;
                if *coercion == Coercion::Take {
                    for ob in &held {
                        state.pending.remove(ob);
                    }
                    return none();
                }
                Some(held)
            }
            ExprKind::Unary { operand, .. } => {
                self.expr(operand, true, state)?;
                none()
            }
            ExprKind::Binary { left, right, .. } => {
                self.expr(left, true, state)?;
                self.expr(right, true, state)?;
                none()
            }
            ExprKind::Logical { left, right, .. } => {
                self.expr(left, true, state)?;
                let mut rhs = state.clone();
                let after = self.expr(right, true, &mut rhs).map(|_| rhs);
                let mut arms = vec![state.clone()];
                arms.extend(after);
                *state = State::join(arms).expect("short circuit is live");
                none()
            }
            ExprKind::Cast { value, .. } => {
                self.expr(value, true, state)?;
                none()
            }
            ExprKind::RatNew {
                numerator,
                denominator,
            } => {
                self.expr(numerator, true, state)?;
                self.expr(denominator, true, state)?;
                none()
            }
            ExprKind::Panic(message) => {
                let _ = self.expr(message, true, state);
                None
            }
            ExprKind::Block(block) => self.block(block, want, state),
            ExprKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.expr(condition, true, state)?;
                match (literal(condition), else_block) {
                    (Some(true), _) => return self.block(then_block, want, state),
                    (Some(false), Some(block)) => return self.block(block, want, state),
                    (Some(false), None) => return Some(Obligations::new()),
                    _ => {}
                }
                let mut arms = Vec::new();
                let mut values = Obligations::new();
                let mut then_state = state.clone();
                if let Some(value) = self.block(then_block, want, &mut then_state) {
                    values.extend(value);
                    arms.push(then_state);
                }
                let mut else_state = state.clone();
                match else_block {
                    Some(block) => {
                        if let Some(value) = self.block(block, want, &mut else_state) {
                            values.extend(value);
                            arms.push(else_state);
                        }
                    }
                    None => arms.push(else_state),
                }
                *state = State::join(arms)?;
                Some(values)
            }
            ExprKind::Match(body) => {
                let _ = self.expr(&body.subject, true, state)?;
                let mut arms = Vec::new();
                let mut values = Obligations::new();
                for (patterns, block) in &body.cases {
                    let mut case = state.clone();
                    if self.values(patterns, &mut case).is_none() {
                        continue;
                    }
                    if let Some(value) = self.block(block, want, &mut case) {
                        values.extend(value);
                        arms.push(case);
                    }
                }
                match &body.else_block {
                    Some(block) => {
                        let mut other = state.clone();
                        if let Some(value) = self.block(block, want, &mut other) {
                            values.extend(value);
                            arms.push(other);
                        }
                    }
                    None if !body.exhaustive => arms.push(state.clone()),
                    None => {}
                }
                *state = State::join(arms)?;
                Some(values)
            }
            ExprKind::Try(body) => {
                // Каждый catch — из состояния входа в `try`: операция могла
                // отказать до любого использования.
                let entry = state.clone();
                let mut arms = Vec::new();
                let mut values = Obligations::new();
                let mut main = entry.clone();
                if let Some(value) = self.block(&body.body, want, &mut main) {
                    values.extend(value);
                    arms.push(main);
                }
                for catch in &body.catches {
                    let mut handler = entry.clone();
                    self.scopes.push(vec![catch.local]);
                    self.defers.push(Vec::new());
                    let result = self.block(&catch.body, want, &mut handler);
                    self.scopes.pop();
                    self.defers.pop();
                    if let Some(value) = result {
                        values.extend(value);
                        arms.push(handler);
                    }
                }
                *state = State::join(arms)?;
                Some(values)
            }
            ExprKind::Loop(block) => {
                let entry = state.clone();
                self.loops.push(LoopFrame {
                    depth: self.scopes.len(),
                    exits: Vec::new(),
                    continues: Vec::new(),
                });
                let mut body_state = entry;
                let _ = self.block(block, false, &mut body_state);
                let frame = self.loops.pop().expect("pushed above");
                // Выход — только по `last`.
                *state = State::join(frame.exits)?;
                Some(Obligations::new())
            }
            ExprKind::For {
                source,
                binding,
                body,
            } => {
                self.source(source, state)?;
                let entry = state.clone();
                self.loops.push(LoopFrame {
                    depth: self.scopes.len(),
                    exits: Vec::new(),
                    continues: Vec::new(),
                });
                self.check_loop_fields(&entry, body, expr.span);
                let mut body_state = entry.clone();
                self.scopes.push(vec![*binding]);
                self.defers.push(Vec::new());
                let end = self.block(body, want, &mut body_state);
                self.scopes.pop();
                self.defers.pop();
                let frame = self.loops.pop().expect("pushed above");
                // Тело может не выполниться ни разу: вход — тоже выход.
                let mut arms = vec![entry];
                if end.is_some() {
                    arms.push(body_state);
                }
                arms.extend(frame.exits);
                arms.extend(frame.continues);
                *state = State::join(arms).expect("the entry path is live");
                Some(Obligations::new())
            }
            ExprKind::Functional(functional) => {
                self.source(&functional.source, state)?;
                if let Some(initial) = &functional.initial {
                    self.expr(initial, true, state)?;
                }
                let entry = state.clone();
                let mut body_state = entry.clone();
                self.scopes.push(functional.bindings.clone());
                self.defers.push(Vec::new());
                let end = self.block(&functional.body, true, &mut body_state);
                if let Some(value) = &end {
                    // Значение блока уходит в результат операции: потреблено.
                    for ob in value {
                        body_state.pending.remove(ob);
                    }
                }
                self.scopes.pop();
                self.defers.pop();
                let mut arms = vec![entry];
                if end.is_some() {
                    arms.push(body_state);
                }
                *state = State::join(arms).expect("the entry path is live");
                none()
            }
            ExprKind::Call { callee, args } => {
                match callee {
                    Callee::Method { receiver, .. } | Callee::Role { receiver, .. } => {
                        self.expr(receiver, true, state)?;
                    }
                    Callee::Builtin { receiver, .. } => {
                        self.expr(receiver, true, state)?;
                    }
                    Callee::Ctor { .. }
                    | Callee::Native(_)
                    | Callee::Test(_)
                    | Callee::FakeStorage(_) => {}
                }
                self.values(args, state)?;
                none()
            }
            ExprKind::CallValue { callee, args } => {
                self.expr(callee, true, state)?;
                self.values(args, state)?;
                none()
            }
            ExprKind::Array(parts) => {
                for part in parts {
                    match part {
                        ArrayPart::Value(value) => {
                            self.expr(value, true, state)?;
                        }
                        ArrayPart::Range { start, end, .. } => {
                            self.expr(start, true, state)?;
                            self.expr(end, true, state)?;
                        }
                    }
                }
                none()
            }
            ExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.expr(key, true, state)?;
                    self.expr(value, true, state)?;
                }
                none()
            }
            ExprKind::Range { start, end, .. } => {
                self.expr(start, true, state)?;
                self.expr(end, true, state)?;
                none()
            }
            ExprKind::Index {
                object,
                index,
                fallback,
            } => {
                self.expr(object, true, state)?;
                self.expr(index, true, state)?;
                match fallback {
                    Some(fallback) => {
                        let mut missing = state.clone();
                        let value = self.expr(fallback, true, &mut missing);
                        let mut arms = vec![state.clone()];
                        if value.is_some() {
                            arms.push(missing);
                        }
                        *state = State::join(arms).expect("the found path is live");
                        Some(value.unwrap_or_default())
                    }
                    None => none(),
                }
            }
            ExprKind::ArrayCast { parts, .. } => {
                for part in parts {
                    let (CastPart::One(value) | CastPart::Spread(value)) = part;
                    self.expr(value, true, state)?;
                }
                none()
            }
            ExprKind::Projection { construction, .. } => {
                self.expr(construction, true, state)?;
                none()
            }
            ExprKind::Unpack(value) | ExprKind::Spawn(value) | ExprKind::Await(value) => {
                self.expr(value, true, state)?;
                none()
            }
            ExprKind::Pack { items, spread } => {
                self.values(items, state)?;
                if let Some(spread) = spread {
                    self.expr(spread, true, state)?;
                }
                none()
            }
            ExprKind::Closure { .. } => none(),
            ExprKind::BoundAction { receiver, .. } | ExprKind::BoundRoleAction { receiver, .. } => {
                self.expr(receiver, true, state)?;
                none()
            }
            ExprKind::StreamWrite { sink, source, .. } => {
                let held = self.expr(sink, true, state)?;
                self.expr(source, true, state)?;
                Some(held)
            }
            ExprKind::CapSelect { owner, .. } => {
                self.expr(owner, true, state)?;
                none()
            }
            ExprKind::CapRead { cap, .. } => {
                self.expr(cap, true, state)?;
                none()
            }
        }
    }

    fn source(&mut self, source: &'r Source, state: &mut State) -> Option<()> {
        match source {
            Source::Range { start, end, .. } => {
                self.expr(start, true, state)?;
                self.expr(end, true, state)?;
            }
            Source::Array(value) | Source::Map(value) | Source::Iterating(value) => {
                self.expr(value, true, state)?;
            }
            Source::Varargs(_) => {}
        }
        Some(())
    }

    /// Поле, которое тело цикла связывает, должно быть связано до цикла.
    fn check_loop_fields(&mut self, entry: &State, body: &Block, span: Span) {
        let Some(object) = self.ctor_object else {
            return;
        };
        let mut bound = Vec::new();
        bound_fields(body, &mut bound);
        for field in bound {
            if entry.fields.get(field) == Some(&Bound::No) {
                let message = format!(
                    "Field \"{}\" must be bound before a loop that rebinds it",
                    self.reg.objects[object.0].fields[field].name
                );
                self.error(span, message);
            }
        }
    }
}

/// Поля, которые блок связывает (`field = v`, `^field = v`).
fn bound_fields(block: &Block, out: &mut Vec<usize>) {
    for stmt in &block.stmts {
        bound_in_stmt(stmt, out);
    }
}

fn bound_in_stmt(stmt: &Stmt, out: &mut Vec<usize>) {
    match &stmt.kind {
        StmtKind::BindField { field, .. } | StmtKind::StageField { field, .. } => {
            if !out.contains(field) {
                out.push(*field);
            }
        }
        StmtKind::Guarded { stmt, .. } => bound_in_stmt(stmt, out),
        StmtKind::Expr(expr) => bound_in_expr(expr, out),
        _ => {}
    }
}

fn bound_in_expr(expr: &Expr, out: &mut Vec<usize>) {
    match &expr.kind {
        ExprKind::Block(block) | ExprKind::Loop(block) => bound_fields(block, out),
        ExprKind::For { body, .. } => bound_fields(body, out),
        ExprKind::If {
            then_block,
            else_block,
            ..
        } => {
            bound_fields(then_block, out);
            if let Some(block) = else_block {
                bound_fields(block, out);
            }
        }
        ExprKind::Match(body) => {
            for (_, block) in &body.cases {
                bound_fields(block, out);
            }
            if let Some(block) = &body.else_block {
                bound_fields(block, out);
            }
        }
        ExprKind::Try(body) => {
            bound_fields(&body.body, out);
            for catch in &body.catches {
                bound_fields(&catch.body, out);
            }
        }
        _ => {}
    }
}
