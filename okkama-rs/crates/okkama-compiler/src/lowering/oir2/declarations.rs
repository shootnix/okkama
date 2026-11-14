use std::collections::HashMap;

use super::{LowerError, Result};
use crate::analysis::{Analysis, thir, ty};
use crate::oir2 as ir;
use crate::source::Span;
use crate::syntax::ast;

pub(super) struct Declarations {
    pub builder: ir::ModuleBuilder,
    pub functions: HashMap<thir::BodyOwner, ir::FuncId>,
    pub types: HashMap<ty::Ty, ir::TypeId>,
    objects: HashMap<ty::ObjectId, ir::DeclId>,
}

impl Declarations {
    pub fn new(analysis: &Analysis<'_>) -> Result<Self> {
        let reg = &analysis.registry;
        let source = &reg.modules[reg.entry_module];
        let mut builder = ir::ModuleBuilder::new(&source.name, ir::Profile::High);
        builder.raw_mut().requires.push(ir::Feature::Core);
        builder.raw_mut().version = source.program.module.version.as_ref().map(|v| {
            format!("{}.{}.{}", v.major, v.minor, v.patch)
        });
        let mut this = Self {
            builder,
            functions: HashMap::new(),
            types: HashMap::new(),
            objects: HashMap::new(),
        };
        for (ty, lowered) in [
            (ty::Ty::Int, ir::Type::Int),
            (ty::Ty::Bool, ir::Type::Bool),
            (ty::Ty::Str, ir::Type::Str),
            (ty::Ty::Range, ir::Type::Range),
        ] {
            this.types.insert(ty, this.builder.intern_type(lowered)?);
        }
        for role in &reg.roles {
            if let Some(decl) = role.decl {
                return Err(LowerError::unsupported(decl.span, "role declaration"));
            }
        }
        if let Some(native) = reg.natives.first() {
            return Err(LowerError::unsupported(native.decl.span, "native action declaration"));
        }
        if let Some(handle) = reg.handles.first() {
            return Err(LowerError::unsupported(handle.span, "native object declaration"));
        }
        for (index, object) in reg.objects.iter().enumerate() {
            let Some(decl) = object.decl else {
                continue;
            };
            if !object.params.is_empty() || !object.caps.is_empty() || !object.does.is_empty() {
                return Err(LowerError::unsupported(decl.span, "generic object, capability or role witness"));
            }
            let id = this.builder.declare(ir::Declaration {
                name: format!("{}.{}", reg.modules[object.module].name, object.name),
                type_params: vec![],
                kind: ir::DeclarationKind::Object {
                    fields: vec![], witnesses: vec![], destroy: None, builtin: false,
                },
                loc: None,
            })?;
            this.objects.insert(ty::ObjectId(index), id);
            let lowered = this.builder.intern_type(ir::Type::Object { decl: id, args: vec![] })?;
            this.types.insert(ty::Ty::object(ty::ObjectId(index)), lowered);
        }
        for (index, enumeration) in reg.enums.iter().enumerate() {
            if enumeration.span.is_empty() {
                continue;
            }
            if enumeration.payloads.iter().any(|payload| !payload.is_empty()) || !enumeration.params.is_empty() {
                return Err(LowerError::unsupported(enumeration.span, "payload or generic enum"));
            }
            let id = this.builder.declare(ir::Declaration {
                name: format!("{}.{}", reg.modules[enumeration.module].name, enumeration.name),
                type_params: vec![],
                kind: ir::DeclarationKind::Enum {
                    cases: enumeration.cases.clone(), is_error: enumeration.error,
                },
                loc: None,
            })?;
            let lowered = this.builder.intern_type(ir::Type::Enum(id))?;
            this.types.insert(ty::Ty::Enum(ty::EnumId(index)), lowered);
            this.types.insert(ty::Ty::Variant(ty::EnumId(index), vec![]), lowered);
        }
        // Declare all symbols before bodies: forward references and stable field/default links.
        for body in &analysis.bodies {
            if body.is_async {
                return Err(LowerError::unsupported(body.span, "async action or cleanup"));
            }
            let mut params = vec![];
            let (symbol, attrs, result) = match body.owner {
                thir::BodyOwner::Entry => {
                    let result = reg.entry.map(|entry| &entry.result);
                    let result = match result {
                        Some(ast::ResultSpec::Void(_)) => None,
                        _ => Some(ty::Ty::Int),
                    };
                    ("main".into(), vec![ir::FunctionAttr::Entry], result)
                }
                thir::BodyOwner::Method { object, method } => {
                    let info = &reg.objects[object.0];
                    let decl = this.objects[&object];
                    let method_info = &info.methods[method];
                    if method_info.successor || method_info.variadic {
                        return Err(LowerError::unsupported(body.span, "successor or variadic action"));
                    }
                    params.push(this.ty(&ty::Ty::object(object), body.span)?);
                    let mut attrs = vec![ir::FunctionAttr::Method(decl)];
                    if method_info.hidden {
                        attrs.push(ir::FunctionAttr::Hidden);
                    }
                    let name = &this.builder.module().declarations.get(decl)?.name;
                    (format!("{name}.{}#{method}", method_info.name), attrs, method_info.result.clone())
                }
                thir::BodyOwner::Ctor { object, ctor } => {
                    let decl = this.objects[&object];
                    let name = &this.builder.module().declarations.get(decl)?.name;
                    (format!("{name}.new#{ctor}"), vec![ir::FunctionAttr::Ctor(decl)], Some(ty::Ty::object(object)))
                }
                thir::BodyOwner::Default { object, field } => {
                    let decl = this.objects[&object];
                    let name = &this.builder.module().declarations.get(decl)?.name;
                    let attr = ir::FunctionAttr::Default { object: decl, field: field as u32 };
                    let result = reg.objects[object.0].fields[field].ty.clone();
                    (format!("{name}.default#{field}"), vec![attr], Some(result))
                }
                thir::BodyOwner::Destroy { object } => {
                    let decl = this.objects[&object];
                    params.push(this.ty(&ty::Ty::object(object), body.span)?);
                    let name = &this.builder.module().declarations.get(decl)?.name;
                    (format!("{name}.destroy"), vec![ir::FunctionAttr::Destroy(decl)], None)
                }
                _ => return Err(LowerError::unsupported(body.span, "test, closure or capability body")),
            };
            for param in &body.params {
                let local = &body.locals[param.0];
                if local.variadic {
                    return Err(LowerError::unsupported(local.span, "variadic parameter"));
                }
                params.push(this.ty(&local.ty, local.span)?);
            }
            let results = result.as_ref().map(|ty| this.ty(ty, body.span)).transpose()?.into_iter().collect();
            let function = this.builder.declare_function(ir::Function {
                symbol,
                signature: ir::Signature { params, results, ownership_abi: ir::OwnershipAbi::Unelaborated },
                attrs,
                type_params: vec![],
                summary: None,
                async_creation_summary: None,
                async_body_summary: None,
                body: None,
                loc: None,
            })?;
            if body.owner == thir::BodyOwner::Entry {
                this.builder.raw_mut().entry = Some(function);
            }
            this.functions.insert(body.owner.clone(), function);
        }
        for (&object, &decl) in &this.objects {
            let info = &reg.objects[object.0];
            let mut fields = vec![];
            for (index, field) in info.fields.iter().enumerate() {
                fields.push(ir::Field {
                    name: field.name.clone(),
                    ty: this.ty(&field.ty, field.span)?,
                    default: this.functions.get(&thir::BodyOwner::Default { object, field: index }).copied(),
                    once: false,
                });
            }
            let destroy = this.functions.get(&thir::BodyOwner::Destroy { object }).copied();
            this.builder.raw_mut().declarations.get_mut(decl)?.kind = ir::DeclarationKind::Object {
                fields, witnesses: vec![], destroy, builtin: false,
            };
        }
        Ok(this)
    }

    fn ty(&self, ty: &ty::Ty, span: Span) -> Result<ir::TypeId> {
        self.types.get(ty).copied().ok_or_else(|| LowerError::unsupported(span, format!("type {ty:?}")))
    }
}
