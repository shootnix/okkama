//! `any` values for the bridged origin natives, and the parts of a value for
//! `runtime/debug` (`data_keys`, `data_items`).
//!
//! An `any` box holds a word and its program type. Its value converts to an
//! origin value through that type (objects, arrays and maps recursively), so
//! the origin `runtime/debug` describes it with the same texts as on the
//! other engines. Parts stay core words: each is boxed as `any` with its own
//! static type, and a role- or `any`-typed part resolves when it is inspected.
use crate::heap;
use crate::host::*;
use crate::ops::{AnyBox, MapValue};
use crate::program::Kind;
use crate::word::{UNBOUND, Word};
use okkama_runtime_core::Value;
use okkama_runtime_core::heap::{ArrayHeap, Closure, Map, MapKey, ObjectHeap};
use okkama_sparrow::{Type as T, TypeRef};
use std::rc::Rc;

/// Nesting beyond this is shown as a native value instead of recursing.
const DEPTH: usize = 64;

/// A value resolved from a word and its static type.
enum Node {
    Scalar(Word, Shape),
    /// A Tomato rat: a payload around the origin's own Rat (#489, #508).
    Rat(Word),
    Enum(u32, u32),
    Array(Word, TypeRef),
    Map(Word, TypeRef, TypeRef),
    Object(Word, u32),
    Action,
    Opaque,
}

impl Heap {
    /// Program enum and object names for the origin `runtime/debug`.
    pub(crate) fn origin_names(&mut self) {
        let program = self.program();
        let objects = program
            .objects
            .iter()
            .map(|o| (o.name.clone(), o.fields.iter().map(|(name, _)| name.clone()).collect()))
            .collect();
        let enums = program
            .enums
            .iter()
            .map(|e| (e.name.rsplit("::").next().unwrap_or(&e.name).to_string(), e.cases.clone()))
            .collect();
        self.origin.objects = objects;
        self.origin.enums = enums;
    }

    fn node(&self, word: Word, ty: TypeRef) -> Node {
        let program = self.program();
        match program.ty(ty) {
            Some(T::Int) => Node::Scalar(word, Shape::Int),
            Some(T::Float) => Node::Scalar(word, Shape::Float),
            Some(T::Bool) => Node::Scalar(word, Shape::Bool),
            Some(T::Byte) => Node::Scalar(word, Shape::Byte),
            Some(T::Char) => Node::Scalar(word, Shape::Char),
            Some(T::Str) => Node::Scalar(word, Shape::Str),
            Some(T::Rat) => Node::Rat(word),
            Some(T::Enum(e)) => Node::Enum(e.0, word as u32),
            Some(T::Array(element)) => Node::Array(word, *element),
            Some(T::Map(key, item)) => Node::Map(word, *key, *item),
            Some(T::Object(o) | T::Successor(o)) => Node::Object(word, o.0),
            Some(T::Fn { .. }) => Node::Action,
            Some(T::Any) => match self.core().payload::<AnyBox>(word) {
                Some(any) => self.node(any.value, any.ty),
                None => Node::Opaque,
            },
            Some(T::Role(_)) if word != UNBOUND => {
                let header = self.core().header_of(word);
                match heap::kind(header) {
                    heap::OBJECT => Node::Object(word, heap::payload(header)),
                    heap::CLOSURE => Node::Action,
                    _ => Node::Opaque,
                }
            }
            _ => Node::Opaque,
        }
    }

    /// The origin value of an `any` box (`slot` holds the box).
    pub(crate) fn any_to_origin(&self, slot: Slot) -> Result<Value, Error> {
        let any = self.core().payload::<AnyBox>(slot.bits).ok_or_else(|| Error("expected any".into()))?;
        self.value_to_origin(any.value, any.ty, 0)
    }

    fn value_to_origin(&self, word: Word, ty: TypeRef, depth: usize) -> Result<Value, Error> {
        if depth > DEPTH {
            return Ok(Value::Handle(Rc::new(())));
        }
        let program = self.program();
        Ok(match self.node(word, ty) {
            Node::Scalar(word, shape) => {
                let abi = match shape {
                    Shape::Int => okkama_runtime_abi::Type::Int,
                    Shape::Float => okkama_runtime_abi::Type::Float,
                    Shape::Bool => okkama_runtime_abi::Type::Bool,
                    Shape::Byte => okkama_runtime_abi::Type::Byte,
                    Shape::Char => okkama_runtime_abi::Type::Char,
                    _ => okkama_runtime_abi::Type::Str,
                };
                let slot = self.slot(word, &shape);
                self.to_origin(slot, &abi)?
            }
            Node::Rat(word) => match self.core().payload::<crate::ops::Rational>(word) {
                Some(rational) => Value::Rat(Rc::new(rational.0.clone())),
                None => Value::Handle(Rc::new(())),
            },
            Node::Enum(declaration, case) => Value::Enum { declaration: declaration as usize, case: case as usize },
            Node::Array(word, element) => {
                let items = self.core().elements(word).to_vec();
                ArrayHeap.allocate(items.into_iter().map(|w| self.value_to_origin(w, element, depth + 1)).collect::<Result<_, _>>()?)
            }
            Node::Map(word, key, item) => {
                let map = self.core().payload::<MapValue>(word).ok_or_else(|| Error("expected map".into()))?;
                let mut entries = std::collections::BTreeMap::new();
                for &(k, v) in map.entries.values() {
                    let k = self.value_to_origin(k, key, depth + 1)?;
                    let k = MapKey::from_value(&k).map_err(|e| Error(e.message))?;
                    entries.insert(k, self.value_to_origin(v, item, depth + 1)?);
                }
                Value::Map(Rc::new(Map::new(entries)))
            }
            Node::Object(word, descriptor) => {
                let fields = &program.objects[descriptor as usize].fields;
                let mut values = Vec::with_capacity(fields.len());
                for (index, (_, field)) in fields.iter().enumerate() {
                    let value = self.core().field(word, index);
                    values.push(if value == UNBOUND && Kind::of(program, *field) == Kind::Ref {
                        Value::Undef
                    } else {
                        self.value_to_origin(value, *field, depth + 1)?
                    });
                }
                ObjectHeap.allocate(descriptor as usize, values)
            }
            Node::Action => Value::Fn(Rc::new(Closure { function: 0, captures: Vec::new(), worker: None })),
            Node::Opaque => Value::Handle(Rc::new(())),
        })
    }

    /// `data_keys` / `data_items`: the parts of an `any` value as `@any`.
    pub(crate) fn debug_parts(&mut self, keys: bool, slot: Slot) -> Result<Slot, Error> {
        let any = self.core().payload::<AnyBox>(slot.bits).ok_or_else(|| Error("expected any".into()))?;
        let (word, ty) = (any.value, any.ty);
        let program = self.program();
        let parts: Vec<(Word, TypeRef)> = match self.node(word, ty) {
            Node::Array(word, element) if !keys => self.core().elements(word).iter().map(|&w| (w, element)).collect(),
            Node::Map(word, key, item) => {
                let map = self.core().payload::<MapValue>(word).ok_or_else(|| Error("expected map".into()))?;
                map.entries.values().map(|&(k, v)| if keys { (k, key) } else { (v, item) }).collect()
            }
            Node::Object(word, descriptor) if !keys => program.objects[descriptor as usize]
                .fields
                .iter()
                .enumerate()
                .map(|(index, (_, field))| (self.core().field(word, index), *field))
                .collect(),
            _ => Vec::new(),
        };
        let mut boxed = Vec::with_capacity(parts.len());
        for (word, ty) in parts {
            let program = self.program();
            let repr = Kind::of(program, ty).repr();
            let value = if matches!(program.ty(ty), Some(T::Any)) {
                word
            } else {
                self.core_mut().allocate_payload(Box::new(AnyBox { value: word, ty, repr }))
            };
            boxed.push(Slot { bits: value, tag: OBJECT });
        }
        self.allocate_array(boxed)
    }
}
