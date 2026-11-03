//! The shared SPB verifier checks references and initialization, not the complete
//! typed machine ABI. Never expose unchecked operand types to native code.
use crate::{
    Error, Op, kind,
    scalar::Kind,
    value::{BIG, BOOL, BYTE, CHAR, FLOAT, SMALL, Unary},
};
use okkama_sparrow::{Program, Target};

pub(crate) fn function(
    program: &Program,
    index: usize,
    plan: &[Op],
    kinds: &[Kind],
) -> Result<(), Error> {
    let source = &program.functions[index];
    for op in plan {
        if let Op::ObjectNew(_, object, _) = op {
            if let Some(destroy) = program.objects[*object as usize].destroy {
                let f = &program.functions[destroy.0 as usize];
                if f.is_async
                    || f.result.is_some()
                    || f.params.len() != 1
                    || kind(program, f.registers[f.params[0].0 as usize])
                        != Some(Kind::Object(*object))
                {
                    return Err(Error("invalid destroy signature".into()));
                }
            }
        }
    }
    let result = source.result.and_then(|t| kind(program, t));
    let mut params = source.params.clone();
    params.sort_unstable();
    if params.windows(2).any(|p| p[0] == p[1]) {
        return Err(Error(format!(
            "{}: duplicate parameter register",
            source.symbol
        )));
    }
    let edge = |t: &Target| {
        t.moves.iter().all(|(dst, src)| {
            copy_compatible(program, kinds[src.0 as usize], kinds[dst.0 as usize])
        })
    };
    for h in &source.handlers {
        if kinds[h.failure.0 as usize] != Kind::Failure || !edge(&h.target) {
            return Err(Error("invalid handler operand types".into()));
        }
    }
    for (pc, op) in plan.iter().enumerate() {
        let is = |register: usize, expected: Kind| kinds[register] == expected;
        let valid = match op {
            // Only synthesized by the separately checked intrinsic loader.
            Op::Intrinsic(..) => true,
            Op::WorkerSpawn {
                dst, entry, args, ..
            } => {
                if let Kind::Function(t) = kinds[*entry] {
                    let Some(okkama_sparrow::Type::Fn { params, result }) =
                        program.ty(okkama_sparrow::TypeRef(t))
                    else {
                        return Err(Error("invalid worker action type".into()));
                    };
                    kinds[*dst] == Kind::Future(result.map(|t| t.0))
                        && args.len() == params.len()
                        && args
                            .iter()
                            .zip(params)
                            .all(|(r, t)| Some(kinds[*r]) == kind(program, *t))
                } else {
                    false
                }
            }
            Op::WorkerBind {
                dst,
                action,
                entry,
                fields,
            } => {
                if let (Kind::Function(a), Kind::Function(e)) = (kinds[*action], kinds[*entry]) {
                    let (
                        Some(okkama_sparrow::Type::Fn {
                            params: action_params,
                            result: action_result,
                        }),
                        Some(okkama_sparrow::Type::Fn {
                            params: entry_params,
                            result: entry_result,
                        }),
                    ) = (
                        program.ty(okkama_sparrow::TypeRef(a)),
                        program.ty(okkama_sparrow::TypeRef(e)),
                    )
                    else {
                        return Err(Error("invalid worker binding type".into()));
                    };
                    kinds[*dst] == kinds[*action]
                        && action_result == entry_result
                        && entry_params.len() == fields.len() + action_params.len()
                        && fields
                            .iter()
                            .map(|r| Some(kinds[*r]))
                            .chain(action_params.iter().map(|t| kind(program, *t)))
                            .zip(entry_params)
                            .all(|(a, b)| a == kind(program, *b))
                } else {
                    false
                }
            }
            Op::FutureSpawn(dst, future) => {
                matches!(kinds[*future], Kind::Future(_)) && kinds[*dst] == kinds[*future]
            }
            Op::FutureControl(op, dst, future) => {
                matches!(kinds[*future], Kind::Future(_))
                    && if *op == okkama_sparrow::FutOp::Cancel {
                        dst.is_none()
                    } else {
                        dst.is_none_or(|d| is(d, Kind::Bool))
                    }
            }
            Op::Await { dst, future } => {
                source.is_async
                    && matches!(kinds[*future],Kind::Future(t) if dst.is_none_or(|d| t.and_then(|t|kind(program,okkama_sparrow::TypeRef(t)))==Some(kinds[d])))
            }
            Op::RegionNew(dst) => is(*dst, Kind::Region),
            Op::RegionRegister(region, _, _) => is(*region, Kind::Region),
            Op::RegionTransfer(from, to, _) => is(*from, Kind::Region) && is(*to, Kind::Region),
            Op::RegionClose {
                region, failure, ..
            } => {
                is(*region, Kind::Region)
                    && failure.is_none_or(|(d, s)| is(d, Kind::Failure) && is(s, Kind::Failure))
            }
            Op::FailureLink(dst, a, b, _) => {
                is(*dst, Kind::Failure) && is(*a, Kind::Failure) && is(*b, Kind::Failure)
            }
            Op::Throw(src) => matches!(kinds[*src], Kind::Object(_) | Kind::Role(_)),
            Op::Raise(src) => is(*src, Kind::Failure),
            Op::Catch {
                failure,
                kind,
                ty,
                value,
                found,
                missing,
            } => {
                is(*failure, Kind::Failure)
                    && is(
                        *value,
                        match kind {
                            okkama_sparrow::CatchKind::Object => Kind::Object(*ty),
                            okkama_sparrow::CatchKind::Role => Kind::Role(*ty),
                        },
                    )
                    && edge(found)
                    && edge(missing)
            }
            Op::AnyBox(dst, _) => is(*dst, Kind::Any),
            Op::Equal(op, dst, _, _) => {
                is(*dst, Kind::Bool)
                    && matches!(op, okkama_sparrow::Cmp::Eq | okkama_sparrow::Cmp::Ne)
            }
            Op::Trap(..) | Op::Unreachable => true,
            Op::Panic(src) => is(*src, Kind::Str),
            Op::Lookup {
                map,
                array,
                index,
                value,
                found,
                missing,
            } => {
                let types = if *map {
                    matches!(kinds[*array],Kind::Map(k,v) if kind(program,okkama_sparrow::TypeRef(k))==Some(kinds[*index])&&kind(program,okkama_sparrow::TypeRef(v))==Some(kinds[*value]))
                } else {
                    is(*index, Kind::Int)
                        && matches!(kinds[*array],Kind::Array(t) if kind(program,okkama_sparrow::TypeRef(t))==Some(kinds[*value]))
                };
                types && edge(found) && edge(missing)
            }
            Op::RangeNew(dst, a, b, _) => {
                is(*dst, Kind::Range) && is(*a, Kind::Int) && is(*b, Kind::Int)
            }
            Op::RangePart(dst, a, _) => is(*dst, Kind::Int) && is(*a, Kind::Range),
            Op::BuilderRange(dst, builder, start, end, _) => {
                is(*start, Kind::Int)
                    && is(*end, Kind::Int)
                    && kinds[*dst] == kinds[*builder]
                    && matches!(kinds[*builder],Kind::Builder(t) if kind(program,okkama_sparrow::TypeRef(t))==Some(Kind::Int))
            }
            Op::MapNew(dst, entries) => {
                if let Kind::Map(k, v) = kinds[*dst] {
                    entries.iter().all(|(a, b)| {
                        Some(kinds[*a]) == kind(program, okkama_sparrow::TypeRef(k))
                            && Some(kinds[*b]) == kind(program, okkama_sparrow::TypeRef(v))
                    })
                } else {
                    false
                }
            }
            Op::Map(op, dst, args) => {
                use crate::map::MapOp::*;
                let a = args.map(|r| r.map(|r| kinds[r]));
                let ty = |t| kind(program, okkama_sparrow::TypeRef(t));
                match (*op, kinds[*dst], a) {
                    (Length, Kind::Int, [Some(Kind::Map(_, _)), None, None]) => true,
                    (Has, Kind::Bool, [Some(Kind::Map(k, _)), Some(key), None]) => {
                        ty(k) == Some(key)
                    }
                    (Get, out, [Some(Kind::Map(k, v)), Some(key), None]) => {
                        ty(k) == Some(key) && ty(v) == Some(out)
                    }
                    (Keys, Kind::Array(t), [Some(Kind::Map(k, _)), None, None]) => t == k,
                    (Values, Kind::Array(t), [Some(Kind::Map(_, v)), None, None]) => t == v,
                    (BuilderNew, Kind::MapBuilder(_, _), [None, None, None]) => true,
                    (
                        Put,
                        Kind::MapBuilder(k, v),
                        [Some(Kind::MapBuilder(a, b)), Some(key), Some(value)],
                    ) => k == a && v == b && ty(k) == Some(key) && ty(v) == Some(value),
                    (Finish, Kind::Map(k, v), [Some(Kind::MapBuilder(a, b)), None, None]) => {
                        k == a && v == b
                    }
                    _ => false,
                }
            }
            Op::ArrayNew(dst, elements) => {
                if let Kind::Array(element) = kinds[*dst] {
                    elements
                        .iter()
                        .all(|r| Some(kinds[*r]) == kind(program, okkama_sparrow::TypeRef(element)))
                } else {
                    false
                }
            }
            Op::Array(op, dst, args) => {
                use crate::array::ArrayOp::*;
                let a = args.map(|r| r.map(|r| kinds[r]));
                let element = |id| kind(program, okkama_sparrow::TypeRef(id));
                match (*op, kinds[*dst], a) {
                    (Length, Kind::Int, [Some(Kind::Array(_)), None, None]) => true,
                    (Get, out, [Some(Kind::Array(t)), Some(Kind::Int), None]) => {
                        Some(out) == element(t)
                    }
                    (
                        Slice,
                        Kind::Array(t),
                        [Some(Kind::Array(s)), Some(Kind::Int), Some(Kind::Int)],
                    ) => t == s,
                    (Join, Kind::Str, [Some(Kind::Array(t)), Some(Kind::Str), None]) => {
                        element(t) == Some(Kind::Str)
                    }
                    (BuilderNew, Kind::Builder(_), [None, None, None]) => true,
                    (Push, Kind::Builder(t), [Some(Kind::Builder(s)), Some(v), None]) => {
                        t == s && element(t) == Some(v)
                    }
                    (
                        Append,
                        Kind::Builder(t),
                        [Some(Kind::Builder(s)), Some(Kind::Array(v)), None],
                    ) => t == s && t == v,
                    (Finish, Kind::Array(t), [Some(Kind::Builder(s)), None, None]) => t == s,
                    (Split, Kind::Array(t), [Some(Kind::Str), Some(Kind::Str), None]) => {
                        element(t) == Some(Kind::Str)
                    }
                    (Utf8, Kind::Array(t), [Some(Kind::Str), None, None]) => {
                        element(t) == Some(Kind::Byte)
                    }
                    _ => false,
                }
            }
            Op::Text(op, dst, args) => {
                use crate::text::TextOp;
                let expected = match op {
                    TextOp::Length => (Kind::Int, [Some(Kind::Str), None, None]),
                    TextOp::Concat => (Kind::Str, [Some(Kind::Str), Some(Kind::Str), None]),
                    TextOp::Trim | TextOp::Chomp => (Kind::Str, [Some(Kind::Str), None, None]),
                    TextOp::Slice => (
                        Kind::Str,
                        [Some(Kind::Str), Some(Kind::Int), Some(Kind::Int)],
                    ),
                    TextOp::Replace | TextOp::ReplaceFirst => (Kind::Str, [Some(Kind::Str); 3]),
                };
                is(*dst, expected.0)
                    && args
                        .iter()
                        .zip(expected.1)
                        .all(|(r, k)| r.map(|r| kinds[r]) == k)
            }
            Op::RatPart(dst, src, _) => is(*dst, Kind::Str) && is(*src, Kind::Rat),
            Op::Capture(_) => {
                return Err(Error("capture is an internal lowering operation".into()));
            }
            Op::Const(_, slot) if slot.tag == crate::value::UNBOUND => true,
            Op::Const(dst, slot) => is(
                *dst,
                match slot.tag {
                    crate::value::ENUM => Kind::Enum((slot.bits >> 32) as u32),
                    SMALL | BIG => Kind::Int,
                    BOOL => Kind::Bool,
                    BYTE => Kind::Byte,
                    CHAR => Kind::Char,
                    FLOAT => Kind::Float,
                    crate::value::STR => Kind::Str,
                    _ => return Err(Error("invalid constant tag".into())),
                },
            ),
            Op::ObjectNew(dst, object, fields) => {
                let declaration = &program.objects[*object as usize];
                is(*dst, Kind::Object(*object))
                    && fields.len() == declaration.fields.len()
                    && fields
                        .iter()
                        .zip(&declaration.fields)
                        .all(|(r, (_, ty))| Some(kinds[*r]) == kind(program, *ty))
            }
            Op::ObjectNext(dst, object, updates) => {
                if let Kind::Object(descriptor) = kinds[*object] {
                    kinds[*dst] == kinds[*object]
                        && updates.iter().all(|(field, r)| {
                            program.objects[descriptor as usize]
                                .fields
                                .get(*field as usize)
                                .is_some_and(|(_, ty)| Some(kinds[*r]) == kind(program, *ty))
                        })
                } else {
                    false
                }
            }
            Op::ObjectInit(object, field, value) => {
                if let Kind::Object(descriptor) = kinds[*object] {
                    program.objects[descriptor as usize]
                        .fields
                        .get(*field as usize)
                        .is_some_and(|(_, ty)| Some(kinds[*value]) == kind(program, *ty))
                } else {
                    false
                }
            }
            Op::ObjectGet(dst, object, field) => {
                if let Kind::Object(descriptor) = kinds[*object] {
                    program.objects[descriptor as usize]
                        .fields
                        .get(*field as usize)
                        .is_some_and(|(_, ty)| Some(kinds[*dst]) == kind(program, *ty))
                } else {
                    false
                }
            }
            Op::Copy(dst, src) => {
                copy_compatible(program, kinds[*src], kinds[*dst])
                    || matches!((kinds[*src],kinds[*dst]),(Kind::Successor(a),Kind::Object(b))|(Kind::Object(a),Kind::Successor(b)) if a==b)
                    || matches!((kinds[*src],kinds[*dst]),(Kind::Object(owner),Kind::Cap(cap)) if program.caps[cap as usize].owner.0==owner)
            }
            Op::ReturnVoid => source.result.is_none(),
            Op::Return(src) => {
                result == Some(kinds[*src])
                    || matches!((kinds[*src],result),(Kind::Object(a),Some(Kind::Successor(b))) if a==b)
            }
            Op::Not(dst, src) => is(*dst, Kind::Bool) && is(*src, Kind::Bool),
            Op::Jump(t) => edge(t),
            Op::Branch(c, a, b) => is(*c, Kind::Bool) && edge(a) && edge(b),
            Op::RatNew(dst, a, b) => is(*dst, Kind::Rat) && is(*a, Kind::Int) && is(*b, Kind::Int),
            Op::RatBinary(_, dst, a, b) => {
                is(*dst, Kind::Rat) && is(*a, Kind::Rat) && is(*b, Kind::Rat)
            }
            Op::RatUnary(_, dst, src) => is(*dst, Kind::Rat) && is(*src, Kind::Rat),
            Op::ByteNot(dst, src) => is(*dst, Kind::Byte) && is(*src, Kind::Byte),
            Op::ByteBinary(op, dst, a, b) => {
                is(*dst, Kind::Byte)
                    && is(*a, Kind::Byte)
                    && is(
                        *b,
                        if matches!(
                            op,
                            crate::value::Arithmetic::ShiftLeft
                                | crate::value::Arithmetic::ShiftRight
                                | crate::value::Arithmetic::ShiftUnsigned
                        ) {
                            Kind::Int
                        } else {
                            Kind::Byte
                        },
                    )
            }
            Op::Bits(op, byte, dst, value, count) => {
                is(*value, if *byte { Kind::Byte } else { Kind::Int })
                    && is(
                        *dst,
                        if *byte && !op.counts() {
                            Kind::Byte
                        } else {
                            Kind::Int
                        },
                    )
                    && count.is_none_or(|r| is(r, Kind::Int))
            }
            Op::Arithmetic(_, dst, a, b) => {
                is(*dst, Kind::Int) && is(*a, Kind::Int) && is(*b, Kind::Int)
            }
            Op::Unary(Unary::Sign, dst, src) => {
                is(*dst, Kind::Int) && matches!(kinds[*src], Kind::Int | Kind::Float | Kind::Rat)
            }
            Op::Unary(_, dst, src) => is(*dst, Kind::Int) && is(*src, Kind::Int),
            Op::FloatBinary(_, dst, a, b) => {
                is(*dst, Kind::Float) && is(*a, Kind::Float) && is(*b, Kind::Float)
            }
            Op::FloatUnary(_, dst, src) => is(*dst, Kind::Float) && is(*src, Kind::Float),
            Op::Compare(_, dst, a, b) => {
                use okkama_sparrow::Instruction as I;
                let expected = match source.code[pc] {
                    I::EnumCmp { op, .. } => {
                        if !matches!(op, okkama_sparrow::Cmp::Eq | okkama_sparrow::Cmp::Ne)
                            || !matches!(kinds[*a], Kind::Enum(_))
                        {
                            return Err(Error("invalid enum comparison".into()));
                        }
                        kinds[*a]
                    }
                    I::IntCmp { .. } => Kind::Int,
                    I::StrCmp { .. } => Kind::Str,
                    I::FloatCmp { .. } => Kind::Float,
                    I::RatCmp { .. } => Kind::Rat,
                    I::ByteCmp { .. } => Kind::Byte,
                    I::CharCmp { .. } => Kind::Char,
                    I::BoolCmp { .. } => Kind::Bool,
                    _ => return Err(Error("invalid comparison plan".into())),
                };
                is(*dst, Kind::Bool) && is(*a, expected) && is(*b, expected)
            }
            Op::Convert(target, dst, src) => {
                is(*dst, *target)
                    && matches!(
                        (kinds[*src], *target),
                        (
                            Kind::Int | Kind::Bool | Kind::Byte | Kind::Float | Kind::Rat,
                            Kind::Str
                        ) | (Kind::Byte, Kind::Int | Kind::Float | Kind::Rat)
                            | (Kind::Int, Kind::Float)
                            | (Kind::Float, Kind::Int)
                            | (Kind::Int | Kind::Float, Kind::Rat)
                            | (Kind::Rat, Kind::Int | Kind::Float)
                    )
            }
            Op::ByteFrom(dst, src, _) => {
                is(*dst, Kind::Byte) && matches!(kinds[*src], Kind::Int | Kind::Float | Kind::Rat)
            }
            Op::CallCap {
                dst,
                receiver,
                function,
                ty,
            } => {
                let owner = program.caps[*ty as usize].owner.0;
                let f = &program.functions[*function];
                is(*receiver, Kind::Cap(*ty))
                    && f.params.len() == 1
                    && kind(program, f.registers[f.params[0].0 as usize])
                        == Some(Kind::Object(owner))
                    && f.result.and_then(|t| kind(program, t)) == Some(kinds[*dst])
            }
            Op::Role(dst, src, role, message) => {
                is(*dst, Kind::Role(*role))
                    && if message.is_some() {
                        matches!(kinds[*src], Kind::Role(_))
                    } else {
                        matches!(kinds[*src],Kind::Object(owner) if program.objects[owner as usize].does.iter().any(|w|w.role.0==*role))
                    }
            }
            Op::CallRole {
                dst,
                receiver,
                role,
                slot,
                args,
                ..
            } => {
                let declaration = &program.roles[*role as usize];
                if let Some(method) = declaration.methods.get(*slot) {
                    let signature = is(*receiver, Kind::Role(*role))
                        && args.len() == method.params.len()
                        && args
                            .iter()
                            .zip(&method.params)
                            .all(|(r, t)| Some(kinds[*r]) == kind(program, *t))
                        && dst.is_none_or(|d| {
                            Some(kinds[d]) == method.result.and_then(|t| kind(program, t))
                        });
                    let witnesses = role_witnesses(program, *role, *slot);
                    signature && witnesses
                } else {
                    false
                }
            }
            Op::Closure {
                dst,
                function,
                captures,
            } => {
                if let Kind::Function(t) = kinds[*dst] {
                    let Some(okkama_sparrow::Type::Fn { params, result }) =
                        program.ty(okkama_sparrow::TypeRef(t))
                    else {
                        return Err(Error("invalid function type".into()));
                    };
                    let target = &program.functions[*function];
                    let given = captures
                        .iter()
                        .map(|r| Some(kinds[*r]))
                        .chain(params.iter().map(|t| kind(program, *t)));
                    target.params.len() == captures.len() + params.len()
                        && given
                            .zip(&target.params)
                            .all(|(a, p)| a == kind(program, target.registers[p.0 as usize]))
                        && result.and_then(|t| kind(program, t)) == call_result(program, target)
                } else {
                    false
                }
            }
            Op::CallFn {
                dst, callee, args, ..
            } => {
                if let Kind::Function(t) = kinds[*callee] {
                    let Some(okkama_sparrow::Type::Fn { params, result }) =
                        program.ty(okkama_sparrow::TypeRef(t))
                    else {
                        return Err(Error("invalid function type".into()));
                    };
                    args.len() == params.len()
                        && args
                            .iter()
                            .zip(params)
                            .all(|(a, t)| Some(kinds[*a]) == kind(program, *t))
                        && dst
                            .is_none_or(|d| Some(kinds[d]) == result.and_then(|t| kind(program, t)))
                } else {
                    false
                }
            }
            Op::Call {
                dst,
                function,
                args,
                ..
            } => {
                let callee = &program.functions[*function];
                args.len() == callee.params.len()
                    && args.iter().zip(&callee.params).all(|(a, p)| {
                        Some(kinds[*a]) == kind(program, callee.registers[p.0 as usize])
                    })
                    && dst.is_none_or(|d| Some(kinds[d]) == call_result(program, callee))
            }
        };
        if !valid {
            return Err(Error(format!(
                "{}:{pc}: invalid operand types or call signature in new engine",
                source.symbol
            )));
        }
    }
    Ok(())
}

fn call_result(program: &Program, function: &okkama_sparrow::Function) -> Option<Kind> {
    if function.is_async {
        Some(Kind::Future(function.result.map(|t| t.0)))
    } else {
        function.result.and_then(|t| kind(program, t))
    }
}

/// Only immutable containers may widen bottom-typed empty leaves. Builders are
/// invariant: widening a shared mutable builder could admit the wrong values.
fn copy_compatible(program: &Program, from: Kind, to: Kind) -> bool {
    if from == to {
        return true;
    }
    let mut pending = match (from, to) {
        (Kind::Array(a), Kind::Array(b)) => vec![(a, b)],
        (Kind::Map(a, b), Kind::Map(c, d)) => vec![(a, c), (b, d)],
        _ => return false,
    };
    while let Some((from, to)) = pending.pop() {
        if from == to {
            continue;
        }
        use okkama_sparrow::{Type, TypeRef};
        match (program.ty(TypeRef(from)), program.ty(TypeRef(to))) {
            (Some(Type::Never), Some(_)) => {}
            (Some(Type::Array(a)), Some(Type::Array(b))) => pending.push((a.0, b.0)),
            (Some(Type::Map(a, b)), Some(Type::Map(c, d))) => {
                pending.extend([(a.0, c.0), (b.0, d.0)]);
            }
            (Some(a), Some(b)) if a == b => {}
            _ => return false,
        }
    }
    true
}

/// Synthetic intrinsic plans must validate the same dispatch boundary as SPB.
pub(crate) fn role_witnesses(program: &Program, role: u32, slot: usize) -> bool {
    let Some(method) = program
        .roles
        .get(role as usize)
        .and_then(|r| r.methods.get(slot))
    else {
        return false;
    };
    program.objects.iter().enumerate().all(|(owner, object)| {
        object.does.iter().filter(|w| w.role.0 == role).all(|w| {
            let f = &program.functions[w.slots[slot].0 as usize];
            f.params.len() == method.params.len() + 1
                && kind(program, f.registers[f.params[0].0 as usize])
                    == Some(Kind::Object(owner as u32))
                && f.params
                    .iter()
                    .skip(1)
                    .zip(&method.params)
                    .all(|(r, t)| kind(program, f.registers[r.0 as usize]) == kind(program, *t))
                && call_result(program, f) == method.result.and_then(|t| kind(program, t))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use okkama_sparrow::{Type, TypeRef};

    #[test]
    fn bottom_widening_is_directional_and_does_not_widen_mutable_builders() {
        let program = Program {
            types: vec![
                Type::Never,
                Type::Int,
                Type::Str,
                Type::Array(TypeRef(0)),
                Type::Array(TypeRef(1)),
            ],
            ..Program::default()
        };
        assert!(copy_compatible(&program, Kind::Array(0), Kind::Array(1)));
        assert!(copy_compatible(&program, Kind::Array(3), Kind::Array(4)));
        assert!(copy_compatible(&program, Kind::Map(0, 0), Kind::Map(2, 1)));
        assert!(!copy_compatible(&program, Kind::Array(1), Kind::Array(0)));
        assert!(!copy_compatible(&program, Kind::Array(1), Kind::Array(2)));
        assert!(!copy_compatible(
            &program,
            Kind::Builder(0),
            Kind::Builder(1)
        ));
        assert!(!copy_compatible(
            &program,
            Kind::MapBuilder(0, 0),
            Kind::MapBuilder(2, 1)
        ));
    }
}
