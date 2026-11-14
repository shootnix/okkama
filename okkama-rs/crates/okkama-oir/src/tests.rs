use crate::*;

#[test]
fn identical_types_share_ids() -> Result<(), ArenaError> {
    let mut types = TypeTable::new();
    assert!(types.is_empty());
    let int = types.intern(Type::Int)?;
    assert_eq!(types.intern(Type::Int)?, int);
    let array = types.intern(Type::Array(int))?;
    assert_eq!(types.intern(Type::Array(int))?, array);
    let map = types.intern(Type::Map(int, array))?;
    assert_eq!(types.intern(Type::Map(int, array))?, map);
    let function = Type::Fn(FnType { params: vec![map], result: Some(array), is_async: false });
    let fn_id = types.intern(function.clone())?;
    assert_eq!(types.intern(function)?, fn_id);
    assert_eq!(types.get(map)?, &Type::Map(int, array));
    assert_eq!(types.len(), 4);
    assert_eq!(types.iter().map(|(id, _)| id).collect::<Vec<_>>(), [int, array, map, fn_id]);
    assert_eq!(types.get(TypeId::from_u32(4)), Err(ArenaError::InvalidIndex { index: 4, len: 4 }));
    Ok(())
}

#[test]
fn service_types_and_lexical_scopes_are_distinct() -> Result<(), ArenaError> {
    let mut types = TypeTable::new();
    let first = types.intern(Type::Service(ServiceType::Scope(ScopeId::from_u32(0))))?;
    let second = types.intern(Type::Service(ServiceType::Scope(ScopeId::from_u32(1))))?;
    assert_ne!(first, second);
    assert_eq!(types.intern(Type::Service(ServiceType::Scope(ScopeId::from_u32(0))))?, first);
    for service in [ServiceType::Own, ServiceType::Deps, ServiceType::Pending,
        ServiceType::Closed, ServiceType::UniqueCache, ServiceType::Failure] {
        let id = types.intern(Type::Service(service))?;
        assert_ne!(id, first);
        assert_ne!(id, second);
        assert_eq!(types.intern(Type::Service(service))?, id);
    }
    assert_eq!(types.len(), 8);
    Ok(())
}

#[test]
fn generic_parameters_and_function_types_keep_their_identity() -> Result<(), ArenaError> {
    let mut types = TypeTable::new();
    let parameter = TypeParameterRef {
        owner: GenericOwner::Function(FuncId::from_u32(0)), index: 0,
    };
    let first = types.intern(Type::Parameter(parameter))?;
    assert_eq!(types.intern(Type::Parameter(parameter))?, first);
    let second = types.intern(Type::Parameter(TypeParameterRef {
        owner: GenericOwner::Declaration(DeclId::from_u32(0)), index: 0,
    }))?;
    assert_ne!(first, second);
    let sync_fn = types.intern(Type::Fn(FnType {
        params: vec![first], result: None, is_async: false,
    }))?;
    let async_fn = types.intern(Type::Fn(FnType {
        params: vec![first], result: None, is_async: true,
    }))?;
    assert_ne!(sync_fn, async_fn);
    let void_future = types.intern(Type::Future(None))?;
    assert_ne!(void_future, types.intern(Type::Future(Some(first)))?);
    Ok(())
}

#[test]
fn construct_minimal_module_by_hand() -> Result<(), ArenaError> {
    let mut module = Module {
        name: "example".into(),
        version: Some("v1.0.0".into()),
        profile: Profile::High,
        requires: vec![Feature::Core],
        imports: Vec::new(),
        declarations: Arena::new(),
        types: TypeTable::new(),
        scopes: Arena::new(),
        functions: Arena::new(),
        entry: None,
        tests: None,
        files: Arena::new(),
    };
    let int = module.types.intern(Type::Int)?;
    let file = module.files.alloc(SourceFileDecl { path: "example/main.ok".into() })?;
    let loc = Some(DebugLoc { file, line: 1, column: 1 });
    let main = module.functions.alloc(Function {
        symbol: "main".into(),
        signature: Signature {
            params: Vec::new(), results: vec![int], ownership_abi: OwnershipAbi::Unelaborated,
        },
        attrs: vec![FunctionAttr::Entry],
        type_params: Vec::new(),
        body: None,
        loc,
    })?;
    let scope = module.scopes.alloc(ScopeDecl {
        function: main, parent: None, kind: ScopeKind::Action, loc,
    })?;
    let mut blocks = Arena::new();
    let entry = blocks.alloc(Block {
        lexical_scope: scope,
        params: Vec::new(),
        unwind: None,
        insts: Vec::new(),
        term: Terminator { kind: TermKind::Ret { values: Vec::new() }, loc },
    })?;
    let mut insts = Arena::new();
    let inst = insts.alloc(Instruction {
        results: Vec::new(), op: Operation::Const(Literal::Int("42".into())), loc,
    })?;
    let mut values = Arena::new();
    let result = values.alloc(ValueData {
        ty: int, def: Definition::InstResult { inst, index: 0 },
    })?;
    insts.get_mut(inst)?.results.push(result);
    let block = blocks.get_mut(entry)?;
    block.insts.push(inst);
    block.term.kind = TermKind::Ret { values: vec![result] };
    let mut bindings = Arena::new();
    let binding = bindings.alloc(BindingDecl { name: "answer".into(), scope, ty: int, loc })?;
    module.functions.get_mut(main)?.body = Some(Body { entry, values, blocks, insts, bindings });
    module.entry = Some(main);

    let function = module.functions.get(module.entry.unwrap())?;
    let body = function.body.as_ref().unwrap();
    let block = body.blocks.get(body.entry)?;
    assert_eq!(function.signature.results, [int]);
    assert_eq!(body.values.get(result)?.def, Definition::InstResult { inst, index: 0 });
    assert_eq!(body.insts.get(block.insts[0])?.results, [result]);
    assert_eq!(block.term.kind, TermKind::Ret { values: vec![result] });
    assert_eq!(body.bindings.get(binding)?.scope, scope);
    assert_eq!(module.scopes.get(block.lexical_scope)?.function, main);
    assert_eq!(module.files.get(function.loc.unwrap().file)?.path, "example/main.ok");
    Ok(())
}
