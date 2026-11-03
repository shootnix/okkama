//! Owned diagnostic snapshots never retain VM values or recurse through causes.
use crate::{
    Code, Error,
    value::{Heap, Slot},
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureSite {
    pub function: String,
    pub pc: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureCategory {
    Thrown,
    Panic,
    Program,
    Native,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureNode {
    pub category: FailureCategory,
    pub name: String,
    /// Cached Error.text, evaluated only when a thrown failure reaches the root.
    pub message: Option<String>,
    pub native: Option<okkama_runtime_abi::NativeFailure>,
    pub trace: Vec<FailureSite>,
    pub cause: Option<usize>,
    pub secondary: Vec<usize>,
}
/// Node zero is the primary failure. Edges are indices, allowing cycles and
/// shared causes without recursive ownership, formatting or destruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureReport {
    pub nodes: Vec<FailureNode>,
}
impl FailureReport {
    pub(crate) fn snapshot(code: &Code, heap: &Heap, root: Slot) -> Result<Self, Error> {
        let mut handles = vec![root];
        let mut known = HashMap::from([(root.bits, 0)]);
        let mut nodes = Vec::new();
        while nodes.len() < handles.len() {
            let failure = heap.failure(handles[nodes.len()])?;
            let mut edge = |value: Slot| {
                *known.entry(value.bits).or_insert_with(|| {
                    let index = handles.len();
                    handles.push(value);
                    index
                })
            };
            let cause = failure.cause.map(&mut edge);
            let secondary = failure.secondary.iter().copied().map(&mut edge).collect();
            let (category, name, message) = match &failure.kind {
                crate::failure::FailureKind::Thrown(value) => (
                    FailureCategory::Thrown,
                    code.objects[heap.object_descriptor(*value)?].name.clone(),
                    failure.message.clone(),
                ),
                crate::failure::FailureKind::Cancelled => (
                    FailureCategory::Thrown,
                    "Cancelled".into(),
                    Some("Operation was cancelled".into()),
                ),
                crate::failure::FailureKind::Native(value) => (
                    FailureCategory::Native,
                    "NativeFailure".into(),
                    Some(
                        heap.text(heap.native_failure_fields(*value)?[3])?
                            .to_owned(),
                    ),
                ),
                crate::failure::FailureKind::Panic(message) => (
                    FailureCategory::Panic,
                    "Panic".into(),
                    Some(message.clone()),
                ),
                crate::failure::FailureKind::Program(message) => (
                    FailureCategory::Program,
                    "RuntimeError".into(),
                    Some(message.clone()),
                ),
            };
            let trace = failure
                .trace
                .iter()
                .map(|&(function, pc)| FailureSite {
                    function: code.function(function).symbol.clone(),
                    pc,
                })
                .collect();
            nodes.push(FailureNode {
                native: if let crate::failure::FailureKind::Native(value) = failure.kind {
                    Some(heap.native_failure(value)?)
                } else {
                    None
                },
                category,
                name,
                message,
                trace,
                cause,
                secondary,
            });
        }
        Ok(Self { nodes })
    }
}

pub(crate) fn error_methods(
    program: &okkama_sparrow::Program,
) -> Result<Vec<Option<usize>>, Error> {
    use okkama_sparrow::{ObjectRef, Type};
    let error = program.roles.iter().enumerate().find(|(_, r)| r.is_error());
    program
        .objects
        .iter()
        .enumerate()
        .map(|(owner, object)| {
            let Some((role, declaration)) = error else {
                return Ok(None);
            };
            let Some(witness) = object.does.iter().find(|w| w.role.0 as usize == role) else {
                return Ok(None);
            };
            let slot = declaration
                .methods
                .iter()
                .position(|m| m.name == "text")
                .ok_or_else(|| Error("Error.text is missing".into()))?;
            let function = witness.slots[slot].0 as usize;
            let target = &program.functions[function];
            if target.is_async
                || target.params.len() != 1
                || target.result.and_then(|t| program.ty(t)) != Some(&Type::Str)
                || program.ty(target.registers[target.params[0].0 as usize])
                    != Some(&Type::Object(ObjectRef(owner as u32)))
            {
                return Err(Error("invalid Error.text witness signature".into()));
            }
            Ok(Some(function))
        })
        .collect()
}

impl FailureReport {
    /// Format the complete failure graph in linear space. Shared/cyclic links are
    /// shown as references, so diagnostic formatting cannot recurse forever.
    pub fn message(&self) -> String {
        use std::fmt::Write;
        let mut output = String::new();
        let mut pending = vec![(0, "")];
        let mut seen = std::collections::HashSet::new();
        while let Some((index, label)) = pending.pop() {
            if !label.is_empty() {
                let _ = write!(output, "\n{label}\n  ");
            }
            let Some(node) = self.nodes.get(index) else {
                output.push_str("<invalid failure reference>");
                continue;
            };
            if !seen.insert(index) {
                let _ = write!(output, "[failure #{index} already shown]");
                continue;
            }
            match (&node.category, &node.message) {
                (FailureCategory::Thrown, None) => {
                    let _ = write!(output, "Uncaught {}", node.name);
                }
                (_, Some(message))
                    if index == 0
                        && matches!(
                            node.category,
                            FailureCategory::Panic | FailureCategory::Program
                        )
                        && node.secondary.is_empty()
                        && node.cause.is_none() =>
                {
                    output.push_str(message)
                }
                (_, message) => {
                    let _ = write!(
                        output,
                        "{}: {}",
                        node.name,
                        message.as_deref().unwrap_or("")
                    );
                }
            }
            if let Some(cause) = node.cause {
                pending.push((cause, "Caused by:"));
            }
            pending.extend(
                node.secondary
                    .iter()
                    .rev()
                    .map(|index| (*index, "Secondary cleanup failure:")),
            );
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deep_cyclic_failure_graph_is_exported_and_dropped_without_recursion() {
        let p =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/caught-error.asm")).unwrap();
        let code = crate::Executable::new(&p, crate::Mode::Cold).unwrap();
        let mut heap = Heap::new(&[], &[]);
        let object = heap.object_new(0, [].into_iter()).unwrap();
        let first = heap
            .failure_new(crate::failure::FailureKind::Thrown(object))
            .unwrap();
        let mut root = first;
        for _ in 1..10_000 {
            let fresh = heap
                .failure_new(crate::failure::FailureKind::Thrown(object))
                .unwrap();
            heap.failure_link(fresh, root, false).unwrap();
            root = fresh;
        }
        heap.failure_link(first, root, false).unwrap();
        heap.failure_link(root, first, true).unwrap();
        let report = FailureReport::snapshot(&code.code, &heap, root).unwrap();
        assert_eq!(report.nodes.len(), 10_000);
        let mut cursor = 0;
        for _ in 0..10_000 {
            cursor = report.nodes[cursor].cause.unwrap();
        }
        assert_eq!(cursor, 0);
        assert_eq!(report.nodes[report.nodes[0].secondary[0]].cause, Some(0));
        let message = report.message();
        assert!(message.contains("already shown"));
        assert!(message.len() < 1_000_000);
        drop(heap);
        drop(report);
    }
}
