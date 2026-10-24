//! Области Okkama: регионы владения, `defer` и выходы (OIR-1 §7, §19.3).
//!
//! Каждая блочная область (и корень функции) получает регион, если в модуле
//! есть `destroy`. Цель отказа области — цепочка `F_n … F_0`: `F_k`
//! исполняет `D_k` и переходит в `F_(k-1)`, отказ `D_k` присоединяется
//! вторичным (`fail.secondary`), `F_0` закрывает регион `region.unwind` и
//! поднимает отказ дальше. Блоки тела после регистрации `D_k` уходят при
//! отказе в `F_k`, поэтому defer, до которого исполнение не дошло, не
//! выполняется. Изменяемые переменные передаются целям аргументами, как в
//! `try`, — defer читает их значения на момент выхода.
//!
//! Нормальный выход (конец блока, `return`, `next`, `last`) исполняет
//! `D_n … D_1`; отказ `D_k` на этом пути становится основным и продолжает
//! цепочку с `F_(k-1)`. Затем `region.leave` с payload выхода: его
//! идентичности освобождаются и переходят к адресату.
//!
//! Владение (как в Perl-интерпретаторе): результат вызова учитывается
//! регионом области без владения (`region.track`); его принимает
//! вызванное следом действие (параметры и `self` — `region.adopt` корня)
//! либо он уничтожается при выходе из области. `let`, rebind и поля
//! строящегося объекта принимают значение во владение своей области.

use super::{BodyLowerer, TryCtx, Typed};
use crate::analysis::thir;
use crate::ir::{BlockId, InstKind, Target, TermKind, Type, ValueId};
use crate::lowering::Result;
use crate::source::Span;

/// Цель отказа области: `F_k` с параметрами отслеживаемых переменных.
pub(super) struct FailBlock {
    block: BlockId,
    params: Vec<ValueId>,
    tracked: Vec<usize>,
    types: Vec<Type>,
    env: Vec<Option<Typed>>,
}

impl FailBlock {
    fn ctx(&self) -> TryCtx {
        TryCtx {
            target: self.block,
            tracked: self.tracked.clone(),
            handled: None,
        }
    }
}

/// Очистка одной области `scopes`.
#[derive(Default)]
pub(super) struct Cleanup<'r> {
    region: Option<ValueId>,
    /// Слот области в `tries`, если у неё есть цель отказа.
    slot: Option<usize>,
    defers: Vec<&'r thir::Expr>,
    /// `F_0 … F_n`.
    fails: Vec<FailBlock>,
}

impl<'r> BodyLowerer<'_, 'r, '_> {
    /// Область без собственной очистки (привязки `for`, `catch`, блоков `map`).
    pub(super) fn push_scope(&mut self) {
        self.scopes.push(Default::default());
        self.cleanups.push(Cleanup::default());
    }

    pub(super) fn pop_scope(&mut self) {
        self.scopes.pop();
        let cleanup = self
            .cleanups
            .pop()
            .expect("scopes and cleanups are parallel");
        debug_assert!(cleanup.slot.is_none(), "plain scopes have no cleanup");
    }

    /// Открыть регион последней области и цель отказа `F_0`.
    pub(super) fn open_cleanup(&mut self, span: Span) {
        if !self.ctx.ownership {
            return;
        }
        let region = self.emit(InstKind::RegionEnter, Type::Region, span).value;
        let index = self.cleanups.len() - 1;
        self.cleanups[index].region = Some(region);
        self.add_fail_block(index);
        for value in std::mem::take(&mut self.pending_adopt) {
            self.adopt(region, &value, span);
        }
    }

    /// Корень функции: регион и `region.adopt` для не-plain параметров.
    pub(super) fn open_root(&mut self, span: Span) {
        self.open_cleanup(span);
        let Some(region) = self.cleanups[0].region else {
            return;
        };
        // `self` в `destroy` уже уничтожается рантаймом: владеть нечем.
        if self.kind == super::Kind::Destroy {
            return;
        }
        for param in self.params.clone() {
            let ty = self.b.value_type(param).clone();
            self.adopt(region, &Typed { value: param, ty }, span);
        }
    }

    pub(super) fn adopt(&mut self, region: ValueId, value: &Typed, span: Span) {
        if !self.ctx.is_plain(&value.ty) {
            self.b.emit_void(
                InstKind::RegionAdopt {
                    region,
                    value: value.value,
                },
                span,
            );
        }
    }

    /// Регион ближайшей области с регионом не глубже `scope`.
    pub(super) fn region_at(&self, scope: usize) -> Option<ValueId> {
        self.cleanups[..=scope.min(self.cleanups.len() - 1)]
            .iter()
            .rev()
            .find_map(|cleanup| cleanup.region)
    }

    /// Принять значение во владение области `scope` (`let`, rebind, поле).
    pub(super) fn adopt_into(&mut self, scope: usize, value: &Typed, span: Span) {
        if let Some(region) = self.region_at(scope) {
            self.adopt(region, value, span);
        }
    }

    /// Передать корню идентичности `value`, принятые открытыми вложенными
    /// областями (как Perl при выходе из области строящегося объекта).
    pub(super) fn transfer_to_root(&mut self, value: &Typed, span: Span) {
        if self.ctx.is_plain(&value.ty) {
            return;
        }
        let Some(root) = self.region_at(0) else {
            return;
        };
        let nested: Vec<ValueId> = self
            .cleanups
            .iter()
            .skip(1)
            .filter_map(|cleanup| cleanup.region)
            .filter(|region| *region != root)
            .collect();
        for from in nested.into_iter().rev() {
            self.b.emit_void(
                InstKind::RegionTransfer {
                    from,
                    to: root,
                    value: value.value,
                },
                span,
            );
        }
    }

    /// Учитывать свежее значение в текущей области без владения.
    pub(super) fn track(&mut self, value: &Typed, span: Span) {
        if self.ctx.is_plain(&value.ty) {
            return;
        }
        if let Some(region) = self.region_at(self.cleanups.len() - 1) {
            self.b.emit_void(
                InstKind::RegionTrack {
                    region,
                    value: value.value,
                },
                span,
            );
        }
    }

    /// Изменяемые переменные, видимые сейчас: их получают цели отказа.
    fn visible_mutables(&self) -> Vec<usize> {
        let mut tracked: Vec<usize> = self
            .visible_vars()
            .chain(self.field_vars.iter().copied())
            .filter(|var| {
                let var = &self.vars[*var];
                var.value.is_some()
                    && match var.kind {
                        super::VarKind::Local => var.rebindable,
                        super::VarKind::Field | super::VarKind::Staged(_) => true,
                    }
            })
            .collect();
        tracked.sort_unstable();
        tracked.dedup();
        tracked
    }

    /// Новая цель отказа `F_k` области `index`; код дальше — в новом блоке.
    fn add_fail_block(&mut self, index: usize) {
        let tracked = self.visible_mutables();
        let types: Vec<Type> = tracked
            .iter()
            .map(|var| self.vars[*var].value.as_ref().expect("tracked").ty.clone())
            .collect();
        let (block, params) = self.block_with(&types, &[Type::Failure]);
        self.inherit_taken(&tracked, &params);
        let fail = FailBlock {
            block,
            params,
            tracked,
            types,
            env: self.snapshot(),
        };
        let ctx = fail.ctx();
        match self.cleanups[index].slot {
            Some(slot) => {
                debug_assert_eq!(slot + 1, self.tries.len(), "the scope owns the top slot");
                self.tries[slot] = ctx;
            }
            None => {
                self.cleanups[index].slot = Some(self.tries.len());
                self.tries.push(ctx);
            }
        }
        self.cleanups[index].fails.push(fail);
        self.retarget();
    }

    /// Следующий код — в новом блоке с текущей целью unwind.
    pub(super) fn retarget(&mut self) {
        self.split();
        self.sync_unwind();
    }

    /// `defer expr;`: регистрация в текущей области, без вычисления.
    pub(super) fn defer_stmt(&mut self, value: &'r thir::Expr) -> Result<super::Out> {
        let index = self.cleanups.len() - 1;
        if self.cleanups[index].fails.is_empty() {
            self.add_fail_block(index); // F_0 без региона: только `raise`
        }
        self.cleanups[index].defers.push(value);
        self.add_fail_block(index);
        Ok(super::Out::Unit)
    }

    /// Нормальный выход из областей `from..`: defer в обратном порядке, затем
    /// `region.leave` с payload. `false` — выход не завершился (defer ушёл).
    pub(super) fn exit_scopes(
        &mut self,
        from: usize,
        payload: Option<&Typed>,
        span: Span,
    ) -> Result<bool> {
        let saved = self.tries.clone();
        for index in (from..self.cleanups.len()).rev() {
            let region = self.cleanups[index].region;
            if let Some(slot) = self.cleanups[index].slot {
                for k in (1..=self.cleanups[index].defers.len()).rev() {
                    self.tries.truncate(slot);
                    let ctx = self.cleanups[index].fails[k - 1].ctx();
                    self.tries.push(ctx);
                    self.retarget();
                    let defer = self.cleanups[index].defers[k - 1];
                    if !self.lower_defer(defer)? {
                        self.tries = saved;
                        return Ok(false);
                    }
                }
                self.tries.truncate(slot);
                self.retarget();
            }
            if let Some(region) = region {
                let keep = payload
                    .filter(|value| !self.ctx.is_plain(&value.ty))
                    .map(|value| value.value);
                self.b
                    .emit_void(InstKind::RegionLeave { region, keep }, span);
            }
        }
        self.tries = saved;
        Ok(true)
    }

    /// Defer-выражение как отброшенный оператор; `false` — управление ушло.
    fn lower_defer(&mut self, defer: &'r thir::Expr) -> Result<bool> {
        let out = self.expr(defer, false)?;
        Ok(!matches!(out, super::Out::Diverged) && self.b.is_open())
    }

    /// Закрыть последнюю область: построить её цепочку `F_n … F_0`.
    pub(super) fn close_cleanup(&mut self, span: Span) -> Result<()> {
        let cleanup = self
            .cleanups
            .pop()
            .expect("scopes and cleanups are parallel");
        let Some(slot) = cleanup.slot else {
            return Ok(());
        };
        let current = self.b.current();
        let env = self.snapshot();
        for k in (0..cleanup.fails.len()).rev() {
            let fail = &cleanup.fails[k];
            self.b.switch_to(fail.block);
            self.restore(&fail.env);
            self.tries.truncate(slot);
            let failure = *fail.params.last().expect("fail blocks take a failure");
            if k == 0 {
                self.bind_tracked(&fail.tracked, &fail.types, &fail.params);
                let failure = match cleanup.region {
                    Some(region) => {
                        self.emit(
                            InstKind::RegionUnwind { region, failure },
                            Type::Failure,
                            span,
                        )
                        .value
                    }
                    None => failure,
                };
                self.b.terminate(TermKind::Raise(failure), span);
                continue;
            }
            // `D_k` на пути отказа: его собственный отказ — вторичный.
            let (secondary, secondary_params) = self.block_with(&fail.types, &[Type::Failure]);
            self.tries.push(TryCtx {
                target: secondary,
                tracked: fail.tracked.clone(),
                handled: None,
            });
            self.bind_tracked(&fail.tracked, &fail.types, &fail.params);
            let previous = &cleanup.fails[k - 1];
            if self.lower_defer(cleanup.defers[k - 1])? {
                let mut args = self.tracked_args(&previous.tracked);
                args.push(failure);
                self.b.terminate(
                    TermKind::Br(Target {
                        block: previous.block,
                        args,
                    }),
                    span,
                );
            }
            self.b.switch_to(secondary);
            self.restore(&fail.env);
            self.tries.truncate(slot);
            self.bind_tracked(&fail.tracked, &fail.types, &secondary_params);
            let cleanup_failure = *secondary_params.last().expect("secondary takes a failure");
            let joined = self
                .emit(
                    InstKind::FailSecondary {
                        primary: failure,
                        cleanup: cleanup_failure,
                    },
                    Type::Failure,
                    span,
                )
                .value;
            let mut args = self.tracked_args(&previous.tracked);
            args.push(joined);
            self.b.terminate(
                TermKind::Br(Target {
                    block: previous.block,
                    args,
                }),
                span,
            );
        }
        self.tries.truncate(slot);
        self.restore(&env);
        if let Some(block) = current {
            self.b.switch_to(block);
            if self.b.is_open() {
                let next = self.b.new_block();
                self.b.terminate(TermKind::Br(Target::new(next)), span);
                self.b.switch_to(next);
            }
        }
        self.sync_unwind();
        Ok(())
    }

    /// `ret` через очистку всех областей функции.
    pub(super) fn ret(&mut self, value: Option<Typed>, span: Span) -> Result<()> {
        if self.exit_scopes(0, value.as_ref(), span)? {
            self.b
                .terminate(TermKind::Ret(value.map(|value| value.value)), span);
        }
        Ok(())
    }
}
