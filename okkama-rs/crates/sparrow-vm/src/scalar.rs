//! Conservative local scalar replacement plan. No bytecode or PC rewriting.
use okkama_sparrow::{Access, Function, Instruction as I, Program, Register, Type};

#[derive(Clone, Debug)]
pub(crate) enum Action {
    New { dst: Register, fields: Vec<Register> },
    Get { allocation: usize, field: usize, dst: Register },
}

fn fallthrough(pc: usize, instruction: &I) -> bool {
    matches!(instruction, I::Jump { target } if target.pc as usize == pc + 1 && target.moves.is_empty())
}

fn body(function: &Function) -> Vec<&I> {
    function.code.iter().enumerate().filter_map(|(pc, instruction)| (!fallthrough(pc, instruction)).then_some(instruction)).collect()
}

fn constructor(program: &Program, function: usize, args: &[Register]) -> Option<(usize, Vec<Register>)> {
    let f = program.functions.get(function)?;
    if f.code.len() > MAX_CODE || f.is_async || f.native.is_some() || f.intrinsic.is_some() || !f.handlers.is_empty() || f.params.len() != args.len() {
        return None;
    }
    let [I::ObjNew { dst, object, fields }, I::Return { value: Some(result) }] = body(f).as_slice() else { return None; };
    if dst.0 != *result { return None; }
    let sources = fields.iter().map(|r| f.params.iter().position(|p| p == r).map(|i| args[i])).collect::<Option<Vec<_>>>()?;
    Some((object.0 as usize, sources))
}

fn getter(program: &Program, function: usize, object: Register, args: &[Register]) -> Option<usize> {
    let f = program.functions.get(function)?;
    if f.code.len() > MAX_CODE || f.is_async || f.native.is_some() || f.intrinsic.is_some() || !f.handlers.is_empty() || args != [object] || f.params.len() != 1 {
        return None;
    }
    let [I::ObjGet { dst, object: receiver, field }, I::Return { value: Some(result) }] = body(f).as_slice() else { return None; };
    (dst.0 == *result && *receiver == f.params[0]).then_some(*field as usize)
}

// Bound compile-time analysis and the number of SSA fields in this first POC.
const MAX_CODE: usize = 256;
const MAX_FIELDS: usize = 8;

pub(crate) fn plan(program: &Program, function: &Function) -> Vec<Option<Action>> {
    let mut actions = vec![None; function.code.len()];
    // No alternate entries, OSR, handlers or branches can bypass a virtual allocation.
    if function.code.len() > MAX_CODE || function.is_async || !function.handlers.is_empty() || function.code.is_empty()
        || !matches!(function.code.last(), Some(I::Return { .. }))
        || function.code[..function.code.len()-1].iter().enumerate().any(|(pc, i)| i.is_terminator() && !fallthrough(pc, i)) {
        return actions;
    }
    for (pc, inst) in function.code.iter().enumerate() {
        let (dst, declaration, fields) = match inst {
            I::ObjNew { dst, object, fields } => (dst.0, object.0 as usize, fields.clone()),
            I::Call { dst: Some(dst), function: callee, args } => {
                let Some((declaration, fields)) = constructor(program, callee.0 as usize, args) else { continue; };
                (dst.0, declaration, fields)
            }
            _ => continue,
        };
        let Some(object) = program.objects.get(declaration) else { continue; };
        if object.builtin || object.destroy.is_some() || object.fields.len() != fields.len() || fields.is_empty() || fields.len() > MAX_FIELDS
            || object.fields.iter().any(|(_, ty)| !matches!(program.types.get(ty.0 as usize), Some(Type::Int))) {
            continue;
        }
        let mut reads = Vec::new();
        let mut safe = true;
        for (use_pc, next) in function.code.iter().enumerate().skip(pc+1) {
            let mut used = false;
            let mut defined = false;
            next.visit(&mut |a| match a {
                Access::Use(r) if r == dst => used = true,
                Access::Def(r) if r == dst => defined = true,
                _ => {},
            });
            // Register reuse is deliberately excluded even if the former object is dead.
            if defined { safe = false; break; }
            if !used { continue; }
            let read = match next {
                I::ObjGet { dst: out, object: receiver, field } if *receiver == dst => Some((out.0, *field as usize)),
                I::Call { dst: Some(out), function: callee, args } => getter(program, callee.0 as usize, dst, args).map(|field| (out.0, field)),
                _ => None,
            };
            match read {
                Some((out, field)) if field < fields.len() => reads.push((use_pc, out, field)),
                _ => { safe = false; break; }
            }
        }
        if safe && !reads.is_empty() {
            actions[pc] = Some(Action::New { dst, fields });
            for (use_pc, dst, field) in reads {
                actions[use_pc] = Some(Action::Get { allocation: pc, field, dst });
            }
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn fixture() -> Program {
        okkama_sparrow::assemble(r#"sparrow 11
module "scalar" "v1.0.0"
type t0 int
type t1 object obj0
object obj0 "Point" {
 field "x" t0
 field "y" t0
}
function f0 "Point.new" {
 registers [t0,t0,t1]
 params [r0,r1]
 result t1
 0: obj.new r2, obj0, [r0,r1]
 1: ret r2
}
function f1 "Point.first" {
 registers [t1,t0]
 params [r0]
 result t0
 0: obj.get r1, r0, 0
 1: ret r1
}
function f2 "work" {
 registers [t0,t0,t1,t0]
 params [r0,r1]
 result t0
 0: call r2, f0, [r0,r1]
 1: const r0, int 99
 2: call r3, f1, [r2]
 3: obj.get r1, r2, 1
 4: int.add r0, r3, r1
 5: ret r0
}
"#).unwrap()
    }

    #[test]
    fn accepts_snapshot_reads_but_rejects_escape_destroy_and_control_flow() {
        let p = fixture();
        let actions = plan(&p, &p.functions[2]);
        assert!(matches!(actions[0], Some(Action::New { .. })));
        assert!(matches!(actions[2], Some(Action::Get { field: 0, .. })));
        assert!(matches!(actions[3], Some(Action::Get { field: 1, .. })));
        let mut large = p.clone();
        large.functions[2].code.resize(MAX_CODE + 1, I::Return { value: None });
        assert!(plan(&large, &large.functions[2]).iter().all(Option::is_none));
        let mut escape = p.clone();
        escape.functions[2].code[5] = I::Return { value: Some(Register(2)) };
        assert!(plan(&escape, &escape.functions[2]).iter().all(Option::is_none));
        let mut destroy = p.clone();
        destroy.objects[0].destroy = Some(okkama_sparrow::FuncRef(1));
        assert!(plan(&destroy, &destroy.functions[2]).iter().all(Option::is_none));
        let mut branch = p.clone();
        branch.functions[2].code[1] = I::Jump { target: okkama_sparrow::Target { pc: 3, moves: vec![] } };
        assert!(plan(&branch, &branch.functions[2]).iter().all(Option::is_none));
        let mut alias = p.clone();
        alias.functions[2].code[1] = I::Move { dst: okkama_sparrow::Dst(Register(2)), source: Register(2) };
        assert!(plan(&alias, &alias.functions[2]).iter().all(Option::is_none));
        let mut effects = p.clone();
        effects.functions[0].code.insert(0, I::Const { dst: okkama_sparrow::Dst(Register(0)), value: okkama_sparrow::Constant::Int("7".into()) });
        assert!(plan(&effects, &effects.functions[2]).iter().all(Option::is_none));
    }

    #[test]
    fn compiled_virtual_fields_survive_source_overwrite_and_big_int_fallback() {
        use sparrow_runtime::{IntValue, Value};
        let p = fixture();
        p.verify().unwrap();
        let engine = crate::machine::new_engine(&p);
        let jit = engine.jit.as_ref().expect("test requires JIT");
        let code = jit.build(&p, 2, &engine.prepared(2).unwrap().constants).unwrap();
        for (a,b,expected) in [("7","8","15"), ("9223372036854775808","3","9223372036854775811"), ("9223372036854775807","1","9223372036854775808")] {
            let mut registers = vec![Value::Int(IntValue::parse(a).unwrap()),Value::Int(IntValue::parse(b).unwrap()),Value::Undef,Value::Undef];
            let value = jit.enter(code, &mut registers, 0, true).unwrap().unwrap();
            let Value::Int(value) = value else { panic!("integer result"); };
            assert_eq!(value.to_string(), expected);
        }
    }
}
