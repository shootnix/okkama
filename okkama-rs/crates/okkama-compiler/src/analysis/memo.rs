//! `memo let` (#491): a kept value must not depend on effects. A value that
//! reaches the clock, the file system, the network, a process, randomness or
//! shared changing state could be wrong on the next call, so the compiler
//! refuses it. Calls through a role or a function value are trusted: the
//! input answering the same each time is the contract of `memo let`.

use super::registry::Registry;
use super::thir::{self, Block, Body, BodyOwner, Callee, Expr, ExprKind, StmtKind};
use super::ty::ObjectId;

/// Runtime modules whose natives reach the world outside the program.
const EFFECTFUL: &[&str] = &[
    "runtime/account",
    "runtime/application",
    "runtime/crypto",
    "runtime/database",
    "runtime/db",
    "runtime/environment",
    "runtime/fs",
    "runtime/fs_root",
    "runtime/host_crypto",
    "runtime/net",
    "runtime/network",
    "runtime/postgres",
    "runtime/process",
    "runtime/random",
    "runtime/random_distribution",
    "runtime/secret",
    "runtime/stdio",
    "runtime/system",
    "runtime/testing",
    "runtime/time",
    "runtime/tls_transport",
    "runtime/x509",
];

/// std modules whose objects hold shared, changing state.
const STATEFUL: &[&str] = &["cell"];

/// Report every `memo let` of `bodies` whose value depends on an effect.
pub(super) fn check(reg: &mut Registry<'_>, bodies: &[Body]) {
    for body in bodies {
        if !matches!(body.owner, BodyOwner::Method { .. }) {
            continue;
        }
        for stmt in &body.block.stmts {
            let StmtKind::Let { value, memo: true, .. } = &stmt.kind else {
                continue;
            };
            if let Some(reason) = effect(reg, bodies, value) {
                reg.set_module(body.module);
                reg.error(
                    stmt.span,
                    format!("memo let cannot depend on effects: its value {reason}"),
                );
            }
        }
    }
}

/// What the computation of `start` reaches that is an effect, if anything.
fn effect(reg: &Registry<'_>, bodies: &[Body], start: &Expr) -> Option<String> {
    let mut walk = Walk { reg, bodies, seen: Vec::new() };
    walk.expr(start)
}

struct Walk<'r, 'a, 'b> {
    reg: &'r Registry<'a>,
    bodies: &'b [Body],
    /// Bodies already walked: a call graph may loop.
    seen: Vec<BodyOwner>,
}

impl<'b> Walk<'_, '_, 'b> {
    fn expr(&mut self, expr: &Expr) -> Option<String> {
        if let Some(reason) = self.reason(expr) {
            return Some(reason);
        }
        let mut found = None;
        thir::each_child(expr, &mut |child| {
            if found.is_none() {
                found = self.expr(child);
            }
        });
        found
    }

    fn block(&mut self, block: &Block) -> Option<String> {
        let mut found = None;
        thir::each_block_expr(block, &mut |expr| {
            if found.is_none() {
                found = self.expr(expr);
            }
        });
        found
    }

    /// The body of `owner`, walked once.
    fn body(&mut self, owner: BodyOwner) -> Option<String> {
        if self.seen.contains(&owner) {
            return None;
        }
        self.seen.push(owner.clone());
        let bodies = self.bodies;
        let body = bodies.iter().find(|body| body.owner == owner)?;
        self.block(&body.block)
    }

    /// Why `expr` itself is an effect; calls lead into their bodies.
    fn reason(&mut self, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Await(_) | ExprKind::Spawn(_) => Some("awaits a future".to_string()),
            // An anonymous action written in the value runs as part of it.
            ExprKind::Closure { body, .. } => self.block(&body.block),
            ExprKind::Call { callee, .. } => match callee {
                Callee::Native(index) => {
                    let native = &self.reg.natives[*index];
                    let module = &self.reg.modules[native.module].name;
                    EFFECTFUL
                        .contains(&module.as_str())
                        .then(|| format!("calls native {} of {module}", native.name))
                }
                Callee::Method { object, method, .. } => stateful(self.reg, *object)
                    .or_else(|| self.body(BodyOwner::Method { object: *object, method: *method })),
                Callee::Ctor { object, ctor, .. } => stateful(self.reg, *object)
                    .or_else(|| self.body(BodyOwner::Ctor { object: *object, ctor: *ctor })),
                _ => None,
            },
            _ => None,
        }
    }
}

/// The reason an object of a stateful std module is an effect.
fn stateful(reg: &Registry<'_>, object: ObjectId) -> Option<String> {
    let info = &reg.objects[object.0];
    let module = &reg.modules[info.module].name;
    (STATEFUL.contains(&module.as_str()) || module.starts_with("async/"))
        .then(|| format!("uses {}, which holds changing state", info.name))
}
