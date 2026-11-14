//! Finite P1 std specialization. Imported bodies are discovered from resolved calls.
//! Ordinary std bodies remain THIR and use the same high producer as application bodies.
use std::collections::{HashMap, HashSet, VecDeque};

use crate::analysis::{Analysis, Registry, SourceModule, registry, thir, ty};
use crate::source::Span;
use super::{LowerError, Result};

pub(super) fn prepare<'a>(input: &Analysis<'a>) -> Result<Analysis<'a>> {
    let sources = input.registry.modules.iter().map(|module| SourceModule {
        path: module.path.clone(),
        program: module.program,
        imports: module.imports.clone(),
    }).collect();
    let mut registry = Registry::build_files(sources, input.registry.modules.len() - input.registry.root_start);
    for object in &mut registry.objects {
        if object.module < registry.root_start {
            object.decl = None;
        }
    }
    for role in &mut registry.roles {
        if role.module < registry.root_start {
            role.decl = None;
        }
    }
    for enumeration in &mut registry.enums {
        if enumeration.module < registry.root_start {
            enumeration.span = Span::default();
        }
    }
    let mut slice = Slice {
        input,
        registry,
        instances: HashMap::new(),
        queued: HashSet::new(),
        pending: VecDeque::new(),
        bodies: vec![],
    };
    for body in &input.bodies {
        if body.module >= input.registry.root_start {
            slice.queue(body.owner.clone(), body.owner.clone(), vec![]);
        }
    }
    for (index, source) in input.registry.objects.iter().enumerate() {
        if source.module < input.registry.root_start || source.decl.is_none() {
            continue;
        }
        for (field, info) in source.fields.iter().enumerate() {
            slice.registry.objects[index].fields[field].ty = slice.ty(&info.ty, info.span)?;
        }
        for (method, info) in source.methods.iter().enumerate() {
            slice.registry.objects[index].methods[method].result = info.result.as_ref()
                .map(|ty| slice.ty(ty, source.decl.expect("source object").span)).transpose()?;
        }
    }
    while let Some(BodyInstance { source: owner, target, substitution: env }) = slice.pending.pop_front() {
        let mut body = input.bodies.iter().find(|body| body.owner == owner).cloned().ok_or_else(|| {
            LowerError::unsupported(Span::default(), format!("missing std body {owner:?}"))
        })?;
        body.owner = target;
        for local in &mut body.locals {
            local.ty = slice.ty(&local.ty.subst(&env), local.span)?;
        }
        slice.block(&mut body.block, &env)?;
        slice.bodies.push(body);
    }
    Ok(Analysis { registry: slice.registry, bodies: slice.bodies, warnings: input.warnings.clone() })
}

struct Slice<'a, 'b> {
    input: &'b Analysis<'a>,
    registry: Registry<'a>,
    instances: HashMap<(ty::ObjectId, Vec<ty::Ty>), ty::ObjectId>,
    queued: HashSet<thir::BodyOwner>,
    pending: VecDeque<BodyInstance>,
    bodies: Vec<thir::Body>,
}

struct BodyInstance {
    source: thir::BodyOwner,
    target: thir::BodyOwner,
    substitution: Vec<(ty::ParamId, ty::Ty)>,
}

impl Slice<'_, '_> {
    fn queue(&mut self, owner: thir::BodyOwner, target: thir::BodyOwner, env: Vec<(ty::ParamId, ty::Ty)>) {
        if self.queued.insert(target.clone()) {
            self.pending.push_back(BodyInstance { source: owner, target, substitution: env });
        }
    }

    fn instance(&mut self, object: ty::ObjectId, args: &[ty::Ty], span: Span) -> Result<ty::ObjectId> {
        let key = (object, args.to_vec());
        if let Some(&instance) = self.instances.get(&key) {
            return Ok(instance);
        }
        let source = &self.input.registry.objects[object.0];
        if source.module >= self.registry.root_start {
            if !args.is_empty() {
                return Err(LowerError::unsupported(span, "generic application object"));
            }
            return Ok(object);
        }
        let module = self.input.registry.modules[source.module].name.as_str();
        let generic = matches!((module, source.name.as_str()), ("async/atomic", "Atomic") | ("cell", "Cell"));
        let ordinary = matches!((module, source.name.as_str()),
            ("io/stdout", "Stdout") | ("encoding/utf", "Utf8Encoded"));
        if (!generic && !ordinary) || (generic && !matches!(args, [ty::Ty::Int] | [ty::Ty::Str])) {
            return Err(LowerError::unsupported(span, format!("std object {module}.{}<{args:?}>", source.name)));
        }
        if module == "async/atomic" {
            for (index, enumeration) in self.input.registry.enums.iter().enumerate() {
                if enumeration.module == source.module && enumeration.name == "AtomicClosed" {
                    self.registry.enums[index].span = enumeration.span;
                }
            }
        }
        let instance = if generic { ty::ObjectId(self.registry.objects.len()) } else { object };
        self.instances.insert(key, instance);
        let suffix = if args == [ty::Ty::Int] { "_int" } else if args == [ty::Ty::Str] { "_string" } else { "" };
        let mut info = registry::ObjectInfo {
            name: format!("{}{suffix}", source.name),
            module: source.module,
            params: vec![],
            fields: source.fields.clone(),
            ctors: source.ctors.clone(),
            methods: source.methods.clone(),
            // Only direct calls of this finite slice are emitted. Unused std
            // role witnesses are omitted; role dispatch stays unsupported.
            does: vec![],
            destroy: source.destroy,
            caps: vec![],
            decl: source.decl,
        };
        if generic {
            // Reserve the ID before specializing fields (Cell refers to Atomic).
            self.registry.objects.push(registry::ObjectInfo {
                name: info.name.clone(), module: info.module, params: vec![], fields: vec![],
                ctors: vec![], methods: vec![], does: vec![], destroy: None, caps: vec![], decl: info.decl,
            });
        }
        let env: Vec<_> = source.params.iter().copied().zip(args.iter().cloned()).collect();
        for field in &mut info.fields {
            field.ty = self.ty(&field.ty.subst(&env), field.span)?;
        }
        for method in &mut info.methods {
            for (_, ty) in &mut method.params {
                *ty = self.ty(&ty.subst(&env), span)?;
            }
            method.result = method.result.as_ref().map(|ty| self.ty(&ty.subst(&env), span)).transpose()?;
        }
        self.registry.objects[instance.0] = info;
        for (field, info) in source.fields.iter().enumerate() {
            if info.default.is_some() {
                self.queue(thir::BodyOwner::Default { object, field },
                    thir::BodyOwner::Default { object: instance, field }, env.clone());
            }
        }
        if source.destroy.is_some() {
            self.queue(thir::BodyOwner::Destroy { object }, thir::BodyOwner::Destroy { object: instance }, env);
        }
        Ok(instance)
    }

    fn ty(&mut self, value: &ty::Ty, span: Span) -> Result<ty::Ty> {
        Ok(match value {
            ty::Ty::Object(object, args) => ty::Ty::object(self.instance(*object, args, span)?),
            ty::Ty::Array(inner) => ty::Ty::array(self.ty(inner, span)?),
            ty::Ty::Varargs(inner) => ty::Ty::Varargs(Box::new(self.ty(inner, span)?)),
            ty::Ty::Successor(inner) => ty::Ty::Successor(Box::new(self.ty(inner, span)?)),
            ty::Ty::Enum(id) | ty::Ty::Variant(id, _) => {
                self.registry.enums[id.0].span = self.input.registry.enums[id.0].span;
                value.clone()
            }
            _ => value.clone(),
        })
    }

    fn block(&mut self, block: &mut thir::Block, env: &[(ty::ParamId, ty::Ty)]) -> Result<()> {
        for stmt in &mut block.stmts {
            self.stmt(stmt, env)?;
        }
        Ok(())
    }

    fn stmt(&mut self, stmt: &mut thir::Stmt, env: &[(ty::ParamId, ty::Ty)]) -> Result<()> {
        use thir::StmtKind as S;
        match &mut stmt.kind {
            S::Let { value, .. } | S::Expr(value) | S::Throw(value) | S::BindField { value, .. }
            | S::StageField { value, .. } | S::Rebind { value, .. } | S::Defer(value) => self.expr(value, env)?,
            S::Return(value) | S::Last(value) => {
                if let Some(value) = value {
                    self.expr(value, env)?;
                }
            }
            S::Guarded { condition, stmt } => {
                self.expr(condition, env)?;
                self.stmt(stmt, env)?;
            }
            S::Confirm { condition, fallback } => {
                self.expr(condition, env)?;
                if let Some(thir::Fallback::Throw(value) | thir::Fallback::Panic(value)) = fallback {
                    self.expr(value, env)?;
                }
            }
            S::Rethrow | S::Next => {}
        }
        Ok(())
    }

    fn expr(&mut self, expr: &mut thir::Expr, env: &[(ty::ParamId, ty::Ty)]) -> Result<()> {
        use thir::ExprKind as E;
        expr.ty = expr.ty.as_ref().map(|ty| self.ty(&ty.subst(env), expr.span)).transpose()?;
        match &mut expr.kind {
            E::Call { callee, args } => {
                match callee {
                    thir::Callee::Method { receiver, object, method } => {
                        let original = *object;
                        let args = match receiver.ty.as_ref().map(|ty| ty.taken().subst(env)) {
                            Some(ty::Ty::Object(_, args)) => args,
                            _ => vec![],
                        };
                        let instance = self.instance(original, &args, expr.span)?;
                        self.expr(receiver, env)?;
                        *object = instance;
                        let source = &self.input.registry.objects[original.0];
                        let subst = source.params.iter().copied().zip(args).collect();
                        self.queue(thir::BodyOwner::Method { object: original, method: *method },
                            thir::BodyOwner::Method { object: instance, method: *method }, subst);
                    }
                    thir::Callee::Ctor { object, type_args, ctor, .. } => {
                        let original = *object;
                        let args: Vec<_> = type_args.iter().map(|ty| ty.subst(env)).collect();
                        let instance = self.instance(original, &args, expr.span)?;
                        *object = instance;
                        type_args.clear();
                        let source = &self.input.registry.objects[original.0];
                        let subst = source.params.iter().copied().zip(args).collect();
                        self.queue(thir::BodyOwner::Ctor { object: original, ctor: *ctor },
                            thir::BodyOwner::Ctor { object: instance, ctor: *ctor }, subst);
                    }
                    thir::Callee::Builtin { receiver, .. } | thir::Callee::Role { receiver, .. } => {
                        self.expr(receiver, env)?;
                    }
                    _ => {}
                }
                for arg in args {
                    self.expr(arg, env)?;
                }
            }
            E::Unary { operand: value, .. } | E::Coerce { value, .. } | E::Cast { value, .. }
            | E::Panic(value) | E::Unpack(value) | E::VariantTag(value) | E::VariantGet { value, .. } => {
                self.expr(value, env)?;
            }
            E::Binary { left, right, .. } | E::Logical { left, right, .. }
            | E::Range { start: left, end: right, .. } => {
                self.expr(left, env)?;
                self.expr(right, env)?;
            }
            E::If { condition, then_block, else_block } => {
                self.expr(condition, env)?;
                self.block(then_block, env)?;
                if let Some(block) = else_block {
                    self.block(block, env)?;
                }
            }
            E::Block(block) | E::Loop(block) => self.block(block, env)?,
            E::For { source, body, .. } => {
                match source {
                    thir::Source::Range { start, end, .. } => {
                        self.expr(start, env)?;
                        self.expr(end, env)?;
                    }
                    thir::Source::Array(value) => self.expr(value, env)?,
                    _ => {}
                }
                self.block(body, env)?;
            }
            E::Try(found) => {
                self.block(&mut found.body, env)?;
                for catch in &mut found.catches {
                    self.block(&mut catch.body, env)?;
                }
            }
            E::Match(found) => {
                self.expr(&mut found.subject, env)?;
                for (patterns, block) in &mut found.cases {
                    for pattern in patterns {
                        self.expr(pattern, env)?;
                    }
                    self.block(block, env)?;
                }
                if let Some(block) = &mut found.else_block {
                    self.block(block, env)?;
                }
            }
            E::VariantNew { values, .. } | E::Pack { items: values, spread: None } => {
                for value in values {
                    self.expr(value, env)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
