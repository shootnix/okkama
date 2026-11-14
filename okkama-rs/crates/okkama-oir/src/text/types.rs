use super::*;

impl Codec for TypeId {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        r.type_depth += 1;
        if r.type_depth > r.type_depth_limit() {
            return Err(r.error(DiagnosticKind::Limit, "type nesting limit"));
        }
        let ty = Type::read(r)?;
        r.type_depth -= 1;
        r.entity()?;
        r.types.intern(ty).map_err(|e| r.error(DiagnosticKind::Limit, e.to_string()))
    }

    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.enter()?;
        let ty = w.module.types.get(*self).map_err(|e| PrintError::new(e.to_string()))?;
        let result = ty.write(w);
        w.leave();
        result
    }
}

impl Codec for Type {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        let t = r.next()?;
        if t.quoted {
            return Err(r.at(&t, DiagnosticKind::Syntax, "expected type"));
        }
        let ty = match t.text.as_str() {
            "int" => Self::Int,
            "bool" => Self::Bool,
            "byte" => Self::Byte,
            "float" => Self::Float,
            "rat" => Self::Rat,
            "char" => Self::Char,
            "str" => Self::Str,
            "range" => Self::Range,
            "any" => Self::Any,
            "never" => Self::Never,
            "type" => Self::Descriptor,
            "^" => Self::Successor(TypeId::read(r)?),
            "array" | "builder" | "cell" => {
                r.expect("<")?;
                let child = TypeId::read(r)?;
                r.expect(">")?;
                match t.text.as_str() {
                    "array" => Self::Array(child),
                    "builder" => Self::Builder(child),
                    _ => Self::LowCell(child),
                }
            }
            "map" | "mbuilder" => {
                r.expect("<")?;
                let key = TypeId::read(r)?;
                r.expect(",")?;
                let value = TypeId::read(r)?;
                r.expect(">")?;
                if t.text == "map" {
                    Self::Map(key, value)
                } else {
                    Self::MapBuilder(key, value)
                }
            }
            "future" => {
                r.expect("<")?;
                let result = if r.eat("void") {
                    None
                } else {
                    Some(TypeId::read(r)?)
                };
                r.expect(">")?;
                Self::Future(result)
            }
            "frame" => {
                r.expect("<")?;
                let function = FuncId::read(r)?;
                r.expect(">")?;
                Self::Frame(function)
            }
            "fn" | "async" => {
                let is_async = t.text == "async";
                if is_async {
                    r.expect("fn")?;
                }
                r.expect("(")?;
                let mut params = vec![];
                while !r.is(")") {
                    r.entity()?;
                    params.push(TypeId::read(r)?);
                    r.comma()?;
                }
                r.expect(")")?;
                r.expect("->")?;
                let result = if r.eat("void") {
                    None
                } else {
                    Some(TypeId::read(r)?)
                };
                Self::Fn(FnType { params, result, is_async })
            }
            "scope" => {
                r.expect("<")?;
                let scope = ScopeId::read(r)?;
                r.expect(">")?;
                Self::Service(ServiceType::Scope(scope))
            }
            "param" => {
                r.expect("<")?;
                let owner = GenericOwner::read(r)?;
                r.expect(",")?;
                let index = u32::read(r)?;
                r.expect(">")?;
                Self::Parameter(TypeParameterRef { owner, index })
            }
            name => {
                if let Some(service) = service_type(name) {
                    return Ok(Self::Service(service));
                }
                if let Some((owner, index)) = r.generic_parameter(name) {
                    return Ok(Self::Parameter(TypeParameterRef { owner, index }));
                }
                let decl = DeclId::from_u32(r.named_reference("decl", &t, false)?);
                let kind = r.declaration_kind(name);
                let args = r.type_arguments()?;
                match kind {
                    1 => Self::Role {
                        decl, args
                    },
                    2 => Self::Enum(decl),
                    3 => Self::Variant(decl),
                    4 => Self::Handle(decl),
                    5 => Self::Cap {
                        decl, args
                    },
                    _ => Self::Object {
                        decl, args
                    },
                }
            }
        };
        Ok(ty)
    }

    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        match self {
            Self::Int => w.push("int"),
            Self::Bool => w.push("bool"),
            Self::Byte => w.push("byte"),
            Self::Float => w.push("float"),
            Self::Rat => w.push("rat"),
            Self::Char => w.push("char"),
            Self::Str => w.push("str"),
            Self::Range => w.push("range"),
            Self::Any => w.push("any"),
            Self::Never => w.push("never"),
            Self::Descriptor => w.push("type"),
            Self::Object { decl, args } | Self::Role { decl, args } | Self::Cap { decl, args } => {
                check_nominal(self, *decl, w)?;
                decl.write(w)?;
                write_type_arguments(args, w)?;
            }
            Self::Enum(decl) | Self::Variant(decl) | Self::Handle(decl) => {
                check_nominal(self, *decl, w)?;
                decl.write(w)?;
            }
            Self::Successor(ty) => {
                w.push("^");
                ty.write(w)?;
            }
            Self::Array(ty) | Self::Builder(ty) | Self::LowCell(ty) => {
                w.push(match self {
                    Self::Array(_) => "array<", Self::Builder(_) => "builder<", _ => "cell<"
                });
                ty.write(w)?;
                w.push(">");
            }
            Self::Map(a, b) | Self::MapBuilder(a, b) => {
                w.push(if matches!(self, Self::Map(..)) { "map<" } else { "mbuilder<" });
                a.write(w)?;
                w.push(", ");
                b.write(w)?;
                w.push(">");
            }
            Self::Fn(f) => {
                if f.is_async {
                    w.push("async ");
                }
                w.push("fn(");
                write_types(&f.params, w)?;
                w.push(") -> ");
                write_result(f.result, w)?;
            }
            Self::Future(ty) => {
                w.push("future<");
                write_result(*ty, w)?;
                w.push(">");
            }
            Self::Frame(function) => {
                w.push("frame<");
                function.write(w)?;
                w.push(">");
            }
            Self::Service(ServiceType::Scope(scope)) => {
                w.push("scope<");
                scope.write(w)?;
                w.push(">");
            }
            Self::Service(service) => service.write(w)?,
            Self::Parameter(parameter) => {
                if Some(parameter.owner) == w.generic_owner {
                    let params = match parameter.owner {
                        GenericOwner::Function(id) => &w.module.functions.get(id)
                            .map_err(|e| PrintError::new(e.to_string()))?.type_params,
                        GenericOwner::Declaration(id) => &w.module.declarations.get(id)
                            .map_err(|e| PrintError::new(e.to_string()))?.type_params,
                    };
                    let param = params.get(parameter.index as usize)
                        .ok_or_else(|| PrintError::new("unknown generic parameter"))?;
                    if type_keyword(&param.name) {
                        return Err(PrintError::new("type parameter shadows a type keyword"));
                    }
                    w.word(&param.name)?;
                } else {
                    w.push("param<");
                    parameter.owner.write(w)?;
                    w.push(", ");
                    parameter.index.write(w)?;
                    w.push(">");
                }
            }
        }
        Ok(())
    }
}

fn write_result(result: Option<TypeId>, w: &mut Writer<'_>) -> Result<(), PrintError> {
    if let Some(ty) = result {
        ty.write(w)
    } else {
        w.push("void");
        Ok(())
    }
}

pub(super) fn write_types(types: &[TypeId], w: &mut Writer<'_>) -> Result<(), PrintError> {
    for (i, ty) in types.iter().enumerate() {
        if i > 0 {
            w.push(", ");
        }
        ty.write(w)?;
    }
    Ok(())
}

pub(super) fn write_type_arguments(types: &[TypeId], w: &mut Writer<'_>) -> Result<(), PrintError> {
    if !types.is_empty() {
        w.push("<");
        write_types(types, w)?;
        w.push(">");
    }
    Ok(())
}

fn service_type(name: &str) -> Option<ServiceType> {
    Some(match name {
        "own" => ServiceType::Own,
        "deps" => ServiceType::Deps,
        "pending" => ServiceType::Pending,
        "closed" => ServiceType::Closed,
        "unique-cache" => ServiceType::UniqueCache,
        "failure" => ServiceType::Failure,
        "failure-state" => ServiceType::FailureState,
        "exit-state" => ServiceType::ExitState,
        "stable-key" => ServiceType::StableKey,
        "owner" => ServiceType::Owner,
        "holder" => ServiceType::Holder,
        "owner-snapshot" => ServiceType::OwnerSnapshot,
        "cache-state" => ServiceType::CacheState,
        "carrier-state" => ServiceType::CarrierState,
        "task-state" => ServiceType::TaskState,
        "pin" => ServiceType::Pin,
        "receipt" => ServiceType::Receipt,
        "cleanup-state" => ServiceType::CleanupState,
        _ => return None,
    })
}

// A nominal name carries its declaration category. Reject shapes that would
// decode as a different Type variant rather than silently changing the IR.
fn check_nominal(ty: &Type, id: DeclId, w: &Writer<'_>) -> Result<(), PrintError> {
    let Ok(decl) = w.module.declarations.get(id) else {
        // Test payloads supply a name context without a module declaration arena.
        // Public printing still rejects missing IDs in DeclId::write.
        return Ok(());
    };
    let matching = matches!((ty, &decl.kind),
        (Type::Object { .. }, DeclarationKind::Object { .. })
            | (Type::Role { .. }, DeclarationKind::Role { .. })
            | (Type::Cap { .. }, DeclarationKind::Cap { .. })
            | (Type::Enum(_), DeclarationKind::Enum { .. })
            | (Type::Variant(_), DeclarationKind::Variant { .. })
            | (Type::Handle(_), DeclarationKind::Handle)
    );
    if !matching || type_keyword(&decl.name) {
        return Err(PrintError::new("nominal type cannot be represented by this declaration name"));
    }
    Ok(())
}

fn type_keyword(name: &str) -> bool {
    service_type(name).is_some() || matches!(name,
        "int" | "bool" | "byte" | "float" | "rat" | "char" | "str" | "range" | "any" | "never"
            | "type" | "array" | "builder" | "cell" | "map" | "mbuilder" | "future" | "frame"
            | "fn" | "async" | "scope" | "param" | "void"
    )
}
