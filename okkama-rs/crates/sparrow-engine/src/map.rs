//! Ordered maps use Rust's BTreeMap; keys own only immutable scalar data.
use crate::{
    Error, Integer,
    scalar::float_text,
    value::{BIG, BOOL, BYTE, CHAR, FLOAT, Heap, MAP, MAP_BUILDER, SMALL, STR, Slot},
};
use std::{cmp::Ordering, collections::BTreeMap, rc::Rc};
#[derive(Debug, Clone)]
pub(crate) enum Key {
    Int(Integer),
    Float(f64),
    Bool(u64),
    Byte(u64),
    Char(u64),
    Str(Rc<str>),
    Enum(u64),
}
impl Key {
    fn rank(&self) -> u8 {
        match self {
            Self::Int(_) => 0,
            Self::Float(_) => 1,
            Self::Bool(_) => 2,
            Self::Byte(_) => 3,
            Self::Char(_) => 4,
            Self::Str(_) => 5,
            Self::Enum(_) => 6,
        }
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => match (a, b) {
                (Integer::Small(a), Integer::Small(b)) => a.cmp(b),
                (Integer::Big(a), Integer::Big(b)) => a.cmp(b),
                (Integer::Small(a), Integer::Big(b)) => num_bigint::BigInt::from(*a).cmp(b),
                (Integer::Big(a), Integer::Small(b)) => {
                    a.as_ref().cmp(&num_bigint::BigInt::from(*b))
                }
            },
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b).expect("finite float key"),
            (Self::Bool(a), Self::Bool(b))
            | (Self::Byte(a), Self::Byte(b))
            | (Self::Char(a), Self::Char(b)) => a.cmp(b),
            (Self::Str(a), Self::Str(b)) => a.cmp(b),
            (Self::Enum(a), Self::Enum(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}
impl PartialOrd for Key {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl PartialEq for Key {
    fn eq(&self, b: &Self) -> bool {
        self.cmp(b).is_eq()
    }
}
impl Eq for Key {}
pub(crate) type Map = BTreeMap<Key, (Slot, Slot)>;
#[derive(Debug, Clone, Copy)]
pub(crate) enum MapOp {
    Length,
    Has,
    Get,
    Keys,
    Values,
    BuilderNew,
    Put,
    Finish,
}
impl Heap {
    pub(crate) fn key(&self, value: Slot) -> Result<Key, Error> {
        Ok(match value.tag {
            crate::value::ENUM => Key::Enum(value.bits),
            SMALL | BIG => Key::Int(self.integer(value)?),
            FLOAT => Key::Float(f64::from_bits(value.bits)),
            BOOL => Key::Bool(value.bits),
            BYTE => Key::Byte(value.bits),
            CHAR => Key::Char(value.bits),
            STR => Key::Str(self.strings.get(value.bits)?.clone()),
            _ => return Err(Error("map key must be a scalar, enum or string".into())),
        })
    }
    fn put_entry(map: &mut Map, key: Key, original: Slot, value: Slot) {
        map.entry(key)
            .and_modify(|entry| entry.1 = value)
            .or_insert((original, value));
    }
    pub fn map_new(&mut self, entries: impl Iterator<Item = (Slot, Slot)>) -> Result<Slot, Error> {
        let mut map = Map::new();
        for (k, v) in entries {
            Self::put_entry(&mut map, self.key(k)?, k, v);
        }
        Ok(Slot {
            bits: self.maps.allocate(map)?,
            tag: MAP,
        })
    }
    fn map(&self, value: Slot) -> Result<&Map, Error> {
        if value.tag != MAP {
            return Err(Error("expected map handle".into()));
        }
        self.maps.get(value.bits)
    }
    pub fn map_lookup(&self, map: Slot, key: Slot) -> Result<Option<Slot>, Error> {
        Ok(self.map(map)?.get(&self.key(key)?).map(|(_, v)| *v))
    }
    pub fn map_op(&mut self, op: MapOp, args: [Slot; 3]) -> Result<Slot, Error> {
        use MapOp::*;
        match op {
            Length => Ok(Slot::small(self.map(args[0])?.len() as i64)),
            Has => Ok(Slot::boolean(self.map_lookup(args[0], args[1])?.is_some())),
            Get => {
                if let Some(value) = self.map_lookup(args[0], args[1])? {
                    return Ok(value);
                }
                let value = args[1];
                let key = match value.tag {
                    crate::value::ENUM => {
                        let name = self
                            .enum_names
                            .as_ref()
                            .and_then(|names| names.get((value.bits >> 32) as usize))
                            .and_then(|cases| cases.get(value.bits as u32 as usize))
                            .ok_or_else(|| Error("missing enum descriptor".into()))?;
                        format!("\"{name}\"")
                    }
                    STR => format!("\"{}\"", self.text(value)?),
                    CHAR => format!(
                        "\"{}\"",
                        char::from_u32(value.bits as u32).expect("verified char")
                    ),
                    BOOL | BYTE => value.bits.to_string(),
                    FLOAT => float_text(f64::from_bits(value.bits)),
                    _ => self.integer(value)?.to_string(),
                };
                Err(Error(format!("Map key not found: {key}")))
            }
            Keys | Values => {
                let items = self
                    .map(args[0])?
                    .values()
                    .map(|(k, v)| if matches!(op, Keys) { *k } else { *v })
                    .collect();
                self.allocate_array(items)
            }
            BuilderNew => Ok(Slot {
                bits: self.map_builders.allocate(Map::new())?,
                tag: MAP_BUILDER,
            }),
            Put => {
                if args[0].tag != MAP_BUILDER {
                    return Err(Error("expected map builder handle".into()));
                }
                let key = self.key(args[1])?;
                Self::put_entry(
                    self.map_builders.get_mut(args[0].bits)?,
                    key,
                    args[1],
                    args[2],
                );
                Ok(args[0])
            }
            Finish => {
                if args[0].tag != MAP_BUILDER {
                    return Err(Error("expected map builder handle".into()));
                }
                let map = std::mem::take(self.map_builders.get_mut(args[0].bits)?);
                Ok(Slot {
                    bits: self.maps.allocate(map)?,
                    tag: MAP,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_signed_zero_keeps_original_key_and_latest_value() {
        let mut heap = Heap::new(&[], &[]);
        let negative = Slot {
            bits: (-0.0f64).to_bits(),
            tag: FLOAT,
        };
        let positive = Slot {
            bits: 0.0f64.to_bits(),
            tag: FLOAT,
        };
        let map = heap
            .map_new([(negative, Slot::small(1)), (positive, Slot::small(2))].into_iter())
            .unwrap();
        assert_eq!(heap.map_lookup(map, positive).unwrap().unwrap().bits, 2);
        let keys = heap
            .map_op(MapOp::Keys, [map, Slot::default(), Slot::default()])
            .unwrap();
        assert_eq!(heap.elements(keys).unwrap().len(), 1);
        assert_eq!(heap.elements(keys).unwrap()[0].bits, negative.bits);
    }
    #[test]
    fn map_roots_keep_original_key_and_nested_values_alive() {
        let mut heap = Heap::new(&[], &[]);
        let key = heap.allocate_text("key".into()).unwrap();
        let value = heap.allocate_text("value".into()).unwrap();
        let object = heap.object_new(0, [value].into_iter()).unwrap();
        let builder = heap
            .map_op(MapOp::BuilderNew, [Slot::default(); 3])
            .unwrap();
        heap.map_op(MapOp::Put, [builder, key, object]).unwrap();
        for _ in 0..1000 {
            heap.map_new(std::iter::empty()).unwrap();
            if heap.needs_collection() {
                heap.collect([builder].into_iter()).unwrap();
            }
        }
        let map = heap
            .map_op(MapOp::Finish, [builder, Slot::default(), Slot::default()])
            .unwrap();
        heap.collect([map].into_iter()).unwrap();
        assert_eq!(heap.text(key).unwrap(), "key");
        let held = heap.map_lookup(map, key).unwrap().unwrap();
        assert_eq!(
            heap.text(heap.object_get(held, 0).unwrap()).unwrap(),
            "value"
        );
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.text(key).is_err());
        assert!(heap.text(value).is_err());
    }
}
