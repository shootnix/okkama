//! Per-fake mutable storage. State belongs to its object and participates in GC;
//! no global registry, interpreter Value or test-instance aliasing is involved.
use crate::{Code, Error, FunctionPlan, Op, Program, Type, scalar::Kind, value::*};
#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    Set(Kind),
    Get(Kind),
    Has,
    Remove,
    Clear,
    Size,
}
pub(crate) type Storage = std::collections::BTreeMap<crate::map::Key, (Slot, Slot, Kind)>;
pub(crate) fn lower(program: &Program, index: usize) -> Result<FunctionPlan, Error> {
    let f = &program.functions[index];
    let meta = f.intrinsic.as_ref().expect("fake intrinsic");
    let invalid = || Error("invalid fake storage intrinsic signature".into());
    if f.is_async || !meta.helpers.is_empty() || !meta.objects.is_empty() || f.params.is_empty() {
        return Err(invalid());
    }
    let params: Vec<_> = f.params.iter().map(|r| r.0 as usize).collect();
    let Some(Type::Object(owner)) = program.ty(f.registers[params[0]]) else {
        return Err(invalid());
    };
    if !program.objects[owner.0 as usize].name.ends_with("$fake") {
        return Err(invalid());
    }
    let kind = |t| crate::kind(program, t).ok_or_else(invalid);
    let (op, count, result) = match meta.name.as_str() {
        "okkama.fake.storage.set" if params.len() == 3 => {
            (Operation::Set(kind(f.registers[params[2]])?), 3, None)
        }
        "okkama.fake.storage.get" => (
            Operation::Get(kind(f.result.ok_or_else(invalid)?)?),
            2,
            f.result,
        ),
        "okkama.fake.storage.has" => (
            Operation::Has,
            2,
            f.result.filter(|t| program.ty(*t) == Some(&Type::Bool)),
        ),
        "okkama.fake.storage.remove" => (Operation::Remove, 2, None),
        "okkama.fake.storage.clear" => (Operation::Clear, 1, None),
        "okkama.fake.storage.size" => (
            Operation::Size,
            1,
            f.result.filter(|t| program.ty(*t) == Some(&Type::Int)),
        ),
        _ => return Err(invalid()),
    };
    if params.len() != count
        || f.result != result
        || matches!(op, Operation::Has | Operation::Size) && result.is_none()
    {
        return Err(invalid());
    }
    if count >= 2
        && !matches!(
            kind(f.registers[params[1]])?,
            Kind::Int | Kind::Bool | Kind::Str | Kind::Char | Kind::Enum(_)
        )
    {
        return Err(invalid());
    }
    let dst = f.registers.len();
    let plan = vec![
        Op::Intrinsic(
            crate::intrinsic::Operation::FakeStorage(op),
            dst,
            [
                params.first().copied(),
                params.get(1).copied(),
                params.get(2).copied(),
            ],
        ),
        if result.is_some() {
            Op::Return(dst)
        } else {
            Op::ReturnVoid
        },
    ];
    let live = crate::liveness::analyze(&plan, &[]);
    let call_saved = crate::liveness::call_saved(&plan, &live, &[]);
    Ok(FunctionPlan {
        native: None,
        is_async: false,
        cold_entry: 0,
        cached: false,
        symbol: f.symbol.clone(),
        recurrence_loop: false,
        elided_objects: 0,
        params,
        registers: dst + 1,
        plan,
        compiled: Default::default(),
        jit_kinds: None,
        live,
        call_saved,
        handlers: vec![],
    })
}
impl Heap {
    pub(crate) fn fake_storage_operation(
        &mut self,
        op: Operation,
        args: [Option<Slot>; 3],
        code: &Code,
    ) -> Result<Slot, Error> {
        let receiver = args[0].ok_or_else(|| Error("missing fake receiver".into()))?;
        let owner = self.object_descriptor(receiver)?;
        if !code.objects[owner].name.ends_with("$fake") {
            return Err(Error("fake storage requires a role fake".into()));
        }
        if matches!(op, Operation::Clear) {
            self.fake_storage.remove(&receiver.bits);
            return Ok(Slot::default());
        }
        if matches!(op, Operation::Size) {
            return Ok(Slot::small(
                self.fake_storage.get(&receiver.bits).map_or(0, |s| s.len()) as i64,
            ));
        }
        let key = args[1].ok_or_else(|| Error("missing fake key".into()))?;
        let indexed = self.key(key)?;
        if let Operation::Set(kind) = op {
            let value = args[2].ok_or_else(|| Error("missing fake value".into()))?;
            self.fake_storage
                .entry(receiver.bits)
                .or_default()
                .insert(indexed, (key, value, kind));
            return Ok(Slot::default());
        }
        let found = self
            .fake_storage
            .get(&receiver.bits)
            .and_then(|s| s.get(&indexed))
            .copied();
        if matches!(op, Operation::Has) {
            return Ok(Slot::boolean(found.is_some()));
        }
        let Some((_, value, stored)) = found else {
            return Err(Error(format!(
                "Fake storage has no key {}",
                self.test_show(key, &code.objects)?
            )));
        };
        match op {
            Operation::Remove => {
                self.fake_storage
                    .get_mut(&receiver.bits)
                    .expect("existing storage")
                    .remove(&indexed);
                Ok(Slot::default())
            }
            Operation::Get(expected) => {
                if stored == expected {
                    return Ok(value);
                }
                if expected == Kind::Any {
                    return self.any_box(value);
                }
                if let Kind::Role(role) = expected {
                    if value.tag == OBJECT
                        && code.objects[self.object_descriptor(value)?]
                            .does
                            .iter()
                            .any(|w| w.role.0 == role)
                    {
                        return Ok(value);
                    }
                }
                // An unchecked heterogeneous read must never enter typed JIT SSA.
                Err(Error(
                    "Fake storage value does not match the requested type".into(),
                ))
            }
            _ => Err(Error("invalid fake storage operation".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unreachable_storage_cycles_do_not_keep_objects_keys_or_values_alive() {
        let mut heap = Heap::new(&[], &[]);
        let object = heap.object_new(0, std::iter::empty()).unwrap();
        let key = heap.allocate_text("cycle".into()).unwrap();
        let value = heap.allocate_array(vec![object]).unwrap();
        let indexed = heap.key(key).unwrap();
        heap.fake_storage
            .entry(object.bits)
            .or_default()
            .insert(indexed, (key, value, Kind::Array(0)));
        heap.collect([object].into_iter()).unwrap();
        assert_eq!(heap.elements(value).unwrap()[0].bits, object.bits);
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.fake_storage.is_empty());
        assert!(heap.elements(value).is_err());
        assert!(heap.text(key).is_err());
    }
}
