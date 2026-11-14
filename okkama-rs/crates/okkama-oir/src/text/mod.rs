//! Canonical OIR-2 text. This codec checks representation, not ownership,
//! lifetimes or dominance. Input spans and source DebugLoc are distinct.
//!
//! Types are inline; unused interned types are not semantic. Operand codecs
//! retain exhaustive schemas while spelling SSA operands positionally. Only
//! referenced instructions carry labels. Parsing checks representation only.
mod lexer;
mod parser;
mod print;
mod diagnostic;
mod surface;
mod types;
mod declarations;
mod elaboration;

pub use diagnostic::{Diagnostic, DiagnosticKind, Span, PrintError, print_diagnostic};
pub use parser::{parse, parse_with_limits};
pub use print::{print, normalized_eq};
use crate::*;
use parser::Reader;
use print::Writer;

/// Resource bounds apply before recursive decoding. The hard stack-safety
/// ceiling is 128 even when max_nesting is configured larger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_text_bytes: usize,
    pub max_entities: usize,
    pub max_tokens: usize,
    pub max_literal_bytes: usize,
    pub max_nesting: usize,
    pub max_type_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_text_bytes: 16 * 1024 * 1024,
            max_entities: 1_000_000,
            max_tokens: 1_000_000,
            max_literal_bytes: 1024 * 1024,
            max_nesting: 64,
            max_type_depth: 64,
        }
    }
}

pub(crate) trait Codec: Sized {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic>;
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError>;
    fn default_value() -> Option<Self> {
        None
    }
    fn is_default(&self) -> bool {
        false
    }
    fn read_field(r: &mut Reader<'_>, name: &str) -> Result<Self, Diagnostic> {
        if r.is(name) {
            r.field(name)?;
            let value = Self::read(r)?;
            r.comma()?;
            Ok(value)
        } else {
            Self::default_value().ok_or_else(|| r.error(DiagnosticKind::Syntax, format!("expected {name}")))
        }
    }
    fn write_field(&self, w: &mut Writer<'_>, name: &str) -> Result<(), PrintError> {
        if !self.is_default() {
            w.field(name);
            self.write(w)?;
            w.push(", ");
        }
        Ok(())
    }
    fn positional(_name: &str) -> bool {
        false
    }
    fn read_operand(r: &mut Reader<'_>, name: &str, first: &mut bool) -> Result<Self, Diagnostic> {
        if !Self::positional(name) && !r.operand_keyword(name) {
            return Self::default_value().ok_or_else(|| r.error(DiagnosticKind::Syntax, format!("expected {name}")));
        }
        if !*first {
            r.expect(",")?;
        }
        *first = false;
        if !Self::positional(name) {
            r.expect(name)?;
        }
        Self::read(r)
    }
    fn write_operand(&self, w: &mut Writer<'_>, name: &str, first: &mut bool) -> Result<(), PrintError> {
        if self.is_default() {
            return Ok(());
        }
        w.push(if *first { " " } else { ", " });
        *first = false;
        if !Self::positional(name) {
            w.push(name);
            w.push(" ");
        }
        self.write(w)
    }
}
macro_rules! record {
    ($ty:ty, $tag:literal, { $($field:ident),* }) => {
        impl Codec for $ty {
            fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
                r.expect($tag)?;
                r.expect("(")?;
                $(
                    let $field = Codec::read_field(r, stringify!($field))?;
                )*
                r.expect(")")?;
                Ok(Self { $($field),* })
            }

            fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
                w.push(concat!($tag, "("));
                $(
                    self.$field.write_field(w, stringify!($field))?;
                )*
                w.close_record();
                Ok(())
            }
        }
    };
}

// Each variant has a distinct tag; field and variant matches are exhaustive.
macro_rules! variants {
    ($ty:ty {
        $($variant:ident $( { $($field:ident),* } )? $( ( $($item:ident),* ) )? => $tag:literal),* $(,)?
    }) => {
        #[cfg(test)]
        impl $ty {
            pub(crate) const TEXT_VARIANTS: &'static [&'static str] = &[$($tag),*];
        }

        impl Codec for $ty {
            fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
                let token = r.next()?;
                match token.text.as_str() {
                    $(
                        $tag if !token.quoted => {
                            $(
                                r.expect("(")?;
                                $(
                                    let $field = Codec::read_field(r, stringify!($field))?;
                                )*
                                r.expect(")")?;
                            )?
                            $(
                                r.expect("(")?;
                                $(
                                    let $item = Codec::read(r)?;
                                    r.comma()?;
                                )*
                                r.expect(")")?;
                            )?
                            Ok(Self::$variant $( { $($field),* } )? $( ( $($item),* ) )?)
                        }
                    ),*
                    _ => Err(r.at(&token, DiagnosticKind::Syntax, concat!("unknown ", stringify!($ty), " variant"))),
                }
            }

            fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
                w.enter()?;
                let result = match self {
                    $(
                        Self::$variant $( { $($field),* } )? $( ( $($item),* ) )? => {
                            w.push($tag);
                            $(
                                w.push("(");
                                $(
                                    $field.write_field(w, stringify!($field))?;
                                )*
                                w.close_record();
                            )?
                            $(
                                w.push("(");
                                $(
                                    $item.write(w)?;
                                    w.push(", ");
                                )*
                                w.close_record();
                            )?
                            Ok(())
                        }
                    ),*
                };
                w.leave();
                result
            }
        }
    };
}

macro_rules! opcodes {
    ($ty:ty { $($variant:ident { $($field:ident),* } => $tag:literal),* $(,)? }) => {
        impl Codec for $ty {
            fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
                let token = r.next()?;
                match token.text.as_str() {
                    $(
                        $tag if !token.quoted => {
                            #[allow(unused_mut)]
                            let mut first = true;
                            $(
                                let mut $field = None;
                            )*
                            $(
                                surface::read_phase(&mut $field, r, stringify!($field), true, &mut first)?;
                            )*
                            $(
                                surface::read_phase(&mut $field, r, stringify!($field), false, &mut first)?;
                            )*
                            $(
                                let $field = $field.ok_or_else(|| {
                                    r.error(DiagnosticKind::Structure, "missing operand")
                                })?;
                            )*
                            let _ = first;
                            Ok(Self::$variant { $($field),* })
                        }
                    ),*,
                    _ => Err(r.at(&token, DiagnosticKind::Syntax, "unknown opcode")),
                }
            }

            fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
                match self {
                    $(
                        Self::$variant { $($field),* } => {
                            w.push($tag);
                            #[allow(unused_mut)]
                            let mut first = true;
                            $(
                                surface::write_phase($field, w, stringify!($field), true, &mut first)?;
                            )*
                            $(
                                surface::write_phase($field, w, stringify!($field), false, &mut first)?;
                            )*
                            let _ = first;
                            Ok(())
                        }
                    ),*
                }
            }
        }
    };
}

impl Codec for String {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        let token = r.next()?;
        if token.quoted && !token.character {
            Ok(token.text)
        } else {
            Err(r.at(&token, DiagnosticKind::Syntax,
            "expected string"))
        }
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.push("\"");
        for c in self.chars() {
            match c {
                '\n' => w.push("\\n"), '\r' => w.push("\\r"), '\t' => w.push("\\t"), '\\' => w.push("\\\\"),
                '"' => w.push("\\\""),
                c if c.is_control() => w.push(&format!("\\u{{{:X}}}", u32::from(c))), c => w.push(&c.to_string())
            }
        }
        w.push("\"");
        Ok(())
    }
}
macro_rules! unsigned { ($($ty:ty),*) => {
    $(impl Codec for $ty {
        fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
            let token = r.next()?;
            if token.quoted || !token.text.bytes().all(|b| b.is_ascii_digit()) {
                return Err(r.at(&token,
                    DiagnosticKind::Syntax, "expected unsigned decimal"));
            }
            token.text.parse().map_err(|_| r.at(&token, DiagnosticKind::Literal, "integer out of range"))
        }
        fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
            w.push(&self.to_string());
            Ok(())
        }
    })*
};
}
unsigned!(u8, u16, u32, u64);
impl Codec for bool {
    fn default_value() -> Option<Self> {
        Some(false)
    }
    fn is_default(&self) -> bool {
        !*self
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        let t = r.next()?;
        match t.text.as_str() {
            "true" if !t.quoted => Ok(true),
            "false" if !t.quoted => Ok(false),
            _ => Err(r.at(&t, DiagnosticKind::Syntax, "expected bool")),
        }
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.push(if *self { "true" } else { "false" });
        Ok(())
    }
}
impl<T: Codec> Codec for Vec<T> {
    fn positional(name: &str) -> bool {
        T::positional(name)
    }
    fn read_operand(r: &mut Reader<'_>, name: &str, first: &mut bool) -> Result<Self, Diagnostic> {
        if !Self::positional(name) {
            if !r.operand_keyword(name) {
                return Ok(vec![]);
            }
        } else if !r.operand_keyword("[") {
            return Ok(vec![]);
        }
        if !*first {
            r.expect(",")?;
        }
        *first = false;
        if !Self::positional(name) {
            r.expect(name)?;
        }
        Self::read(r)
    }
    fn default_value() -> Option<Self> {
        Some(vec![])
    }
    fn is_default(&self) -> bool {
        self.is_empty()
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        r.expect("[")?;
        let mut items = vec![];
        while !r.is("]") {
            r.entity()?;
            items.push(T::read(r)?);
            r.comma()?;
        }
        r.expect("]")?;
        Ok(items)
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.push("[");
        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                w.push(", ");
            }
            item.write(w)?;
        }
        w.push("]");
        Ok(())
    }
}
impl<T: Codec> Codec for Option<T> {
    fn default_value() -> Option<Self> {
        Some(None)
    }
    fn is_default(&self) -> bool {
        self.is_none()
    }
    fn read_operand(r: &mut Reader<'_>, name: &str, first: &mut bool) -> Result<Self, Diagnostic> {
        if !r.operand_keyword(name) {
            return Ok(None);
        }
        if !*first {
            r.expect(",")?;
        }
        *first = false;
        r.expect(name)?;
        Ok(Some(T::read(r)?))
    }
    fn write_operand(&self, w: &mut Writer<'_>, name: &str, first: &mut bool) -> Result<(), PrintError> {
        if let Some(value) = self {
            w.push(if *first { " " } else { ", " });
            *first = false;
            w.push(name);
            w.push(" ");
            value.write(w)?;
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        if r.eat("none") {
            Ok(None)
        } else {
            r.expect("some")?;
            r.expect("(")?;
            let value = T::read(r)?;
            r.expect(")")?;
            Ok(Some(value))
        }
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        if let Some(value) = self {
            w.push("some(");
            value.write(w)?;
            w.push(")");
        } else {
            w.push("none");
        }
        Ok(())
    }
}
impl<T: Codec> Codec for Box<T> {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        Ok(Box::new(T::read(r)?))
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        (**self).write(w)
    }
}
impl<A: Codec, B: Codec> Codec for (A, B) {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        r.expect("(")?;
        let a = A::read(r)?;
        r.expect(",")?;
        let b = B::read(r)?;
        r.expect(")")?;
        Ok((a,b))
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.push("(");
        self.0.write(w)?;
        w.push(", ");
        self.1.write(w)?;
        w.push(")");
        Ok(())
    }
}
macro_rules! ids {
    ($($ty:ty => ($ns:literal, $prefix:literal, $local:literal)),* $(,)?) => {
        $(impl Codec for $ty {
            fn positional(name: &str) -> bool {
                $ns == "value" || ($ns == "scope" && name == "scope")
                    || ($ns == "decl" && name == "declaration") || ($ns == "func" && name == "function")
            }
            fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
                Ok(Self::from_u32(r.reference($ns, $prefix,
                    $local)?))
            }
            fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
                w.reference($ns, self.as_u32(), $local)
            }
        })*
    };
}
ids!(DeclId => ("decl", "", false), FuncId => ("func", "@", false),
    ScopeId => ("scope", "S", false), FileId => ("file", "", false), SummaryId => ("summary", "M", false),
    ProtocolId => ("protocol", "", false),
    CleanupPlanId => ("plan", "P", true), CleanupOrderId => ("order", "O", true),
    CleanupRootsId => ("roots", "R", true),
    BlockId => ("block", "^", true), InstId => ("inst", "i", true),
    BindingId => ("binding", "b", true), BoundaryId => ("boundary", "B", true),
    DepExprId => ("dependency", "d", true), KeyExprId => ("key", "k", true));
impl Codec for CreationSiteId {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        Ok(Self::from_u32(u32::read(r)?))
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        self.as_u32().write(w)
    }
}
impl Codec for DebugLoc {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        r.expect("!")?;
        let file = FileId::read(r)?;
        r.expect(":")?;
        let line = u32::read(r)?;
        r.expect(":")?;
        let column = u32::read(r)?;
        Ok(Self { file, line, column })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.push("!");
        self.file.write(w)?;
        w.push(":");
        self.line.write(w)?;
        w.push(":");
        self.column.write(w)
    }
}
variants!(Feature {
    Core => "core",
    Async => "async",
    Process => "process",
    Test => "test"
});
record!(Import, "import", { path, major, alias, take });
record!(TestDecl, "test_decl", { name, function });
record!(SourceFileDecl, "source_file_decl", { path });
record!(Declaration, "declaration", { name, type_params, kind, loc });
variants!(DeclarationKind {
    Object { fields, witnesses, destroy, builtin } => "object",
    Role { methods, builtin } => "role",
    Enum { cases, is_error } => "enum",
    Variant { cases, is_error } => "variant",
    Handle => "handle",
    Cap { owner, fields } => "cap",
    Global { ty, init } => "global"
});
record!(Field, "field", { name, ty, default, once });
record!(Witness, "witness", { role, slots });
record!(RoleMethod, "role_method", { name, signature });
record!(VariantCase, "variant_case", { name, layout });
record!(ComputedField, "computed_field", { name, function });

variants!(OwnershipAbi {
    Unelaborated => "unelaborated",
    Explicit { input_packet, output_packet } => "explicit",
    Observation => "observation"
});
variants!(FunctionAttr {
    Entry => "entry",
    Method(x0) => "method",
    Ctor(x0) => "ctor",
    Successor(x0) => "successor",
    Default { object, field } => "default",
    Destroy(x0) => "destroy",
    Extern(x0) => "extern",
    Hidden => "hidden",
    Intrinsic { name, objects, helpers } => "intrinsic",
    Async => "async",
    DeferEntry { replacements } => "defer",
    Cleanup => "cleanup",
    Lambda { captures, owner } => "lambda"
});
record!(ScopeDecl, "scope_decl", { function, parent, kind, loc });
variants!(ScopeKind {
    Action => "action",
    Block => "block",
    Branch => "branch",
    Iteration => "iteration",
    Try => "try",
    Catch => "catch",
    Defer => "defer"
});
record!(BindingDecl, "binding_decl", { name, scope, ty, loc });
#[cfg(test)]
impl Type {
    pub(crate) const TEXT_VARIANTS: &'static [&'static str] = &["int", "bool", "byte", "float", "rat",
        "char", "str", "range", "any", "never", "object", "role", "enum", "variant", "handle", "cap", "successor",
        "array", "map", "builder", "map_builder", "fn", "future", "parameter", "descriptor", "frame", "low_cell",
    "service"];
}
record!(FnType, "fn_type", { params, result, is_async });
variants!(ServiceType {
    Own => "own",
    Deps => "deps",
    Scope(x0) => "scope",
    Pending => "pending",
    Closed => "closed",
    UniqueCache => "unique-cache",
    Failure => "failure",
    FailureState => "failure-state",
    ExitState => "exit-state",
    StableKey => "stable-key",
    Owner => "owner",
    Holder => "holder",
    OwnerSnapshot => "owner-snapshot",
    CacheState => "cache-state",
    CarrierState => "carrier-state",
    TaskState => "task-state",
    Pin => "pin",
    Receipt => "receipt",
    CleanupState => "cleanup-state"
});
variants!(GenericOwner {
    Declaration(x0) => "declaration",
    Function(x0) => "function"
});
record!(TypeParameterRef, "type_parameter_ref", { owner, index });
record!(TypeParameter, "type_parameter", { name, bounds });
variants!(Outcome {
    Normal => "normal",
    Unwind => "unwind",
    Cancel => "cancel",
    DeferContinue => "defer_continue",
    DeferReplace(index) => "defer_replace",
    Dispatch(index) => "dispatch"
});
record!(EdgeResult, "edge_result", { outcome, slot });
impl Codec for SwitchCase {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        let value = Literal::read(r)?;
        r.expect(":")?;
        let target = Target::read(r)?;
        Ok(Self { value, target })
    }

    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        self.value.write(w)?;
        w.push(": ");
        self.target.write(w)
    }
}
variants!(LookupKind {
    Array => "array",
    Map => "map"
});
variants!(TrapCode {
    Confirm => "confirm",
    Range => "range",
    Empty => "empty",
    Unbound => "unbound",
    Unreachable => "unreachable",
    Pending => "pending"
});
variants!(OperandMode {
    Borrow => "borrow",
    Consume => "consume",
    Observe => "observe",
    DestroyBorrow => "destroy_borrow",
    Forward => "forward"
});
variants!(OperandConvention {
    Borrow => "borrow",
    Consume => "consume"
});
record!(OperandUse, "operand_use", { value, mode });
variants!(OperandRef {
    Value(x0) => "value",
    Selection(x0) => "selection",
    Dependency(x0) => "dependency",
    Key(x0) => "key"
});
record!(ReferencedOperand, "referenced_operand", { reference, mode });
variants!(ValueFacts {
    Unelaborated => "unelaborated",
    Plain { certificate } => "plain",
    Managed { deps } => "managed",
    Service => "service"
});
variants!(PlainCertificate {
    Scalar => "scalar",
    ClosedGraph { ty } => "closed_graph",
    Descriptor(x0) => "descriptor"
});
record!(InstructionFacts, "instruction_facts", { result_deps, retention, provenance });
record!(ProvenanceFact, "provenance_fact", { obligations, state });
variants!(AdmissionProvenance {
    UnboundResult => "unbound_result",
    Bound => "bound",
    StagedOrCarrierOwned => "staged_or_carrier_owned",
    CacheOwned => "cache_owned",
    ExtractedAtExistingBoundary => "extracted_at_existing_boundary",
    TemporaryAwaitResult => "temporary_await_result"
});
record!(ResourceValue, "resource_value", { value, packet });
variants!(KeyExpr {
    Created { site, instance } => "created",
    ServiceOf { value, kind } => "service_of",
    Runtime(x0) => "runtime"
});
variants!(ServiceKeyKind {
    Cache => "cache",
    Frame => "frame",
    Channel => "channel",
    Carrier => "carrier",
    Pin => "pin",
    Group => "group"
});
variants!(DepExpr {
    Empty => "empty",
    Key(x0) => "key",
    Union(x0) => "union",
    FromSsa(x0) => "from_ssa",
    Retention { value, selection } => "retention"
});
variants!(SelectionExpr {
    Admissible(x0) => "admissible",
    Value(x0) => "value",
    Projection(x0) => "projection",
    OwnerGraph(x0) => "owner_graph",
    ViewOwner(x0) => "view_owner",
    Union(x0) => "union",
    Descriptor { descriptor, value } => "descriptor"
});
variants!(Projection {
    Field { value, field } => "field",
    Index { value, index } => "index",
    Slice { value, start, end } => "slice",
    Map { value, keys } => "map"
});
record!(FieldRef, "field_ref", { declaration, index });
record!(Boundary, "boundary", { scope, kind });
variants!(BoundaryKind {
    Lexical => "lexical",
    Call => "call",
    Loop => "loop",
    AbortBeforeDestroy => "abort_before_destroy",
    AbortAfterPrefix => "abort_after_prefix"
});
variants!(ResultType {
    Exact(x0) => "exact",
    ExactEnum(x0) => "exact_enum",
    Scalar(x0) => "scalar",
    Service(x0) => "service",
    SameAs(x0) => "same_as",
    Element(x0) => "element",
    MapKey(x0) => "map_key",
    MapValue(x0) => "map_value",
    Field(x0) => "field",
    ArrayOf(x0) => "array_of",
    Object { decl, args } => "object",
    FutureOf(x0) => "future_of",
    Awaited(x0) => "awaited",
    SuccessorOf(x0) => "successor_of",
    BuilderOf(x0) => "builder_of",
    MapBuilderOf(x0, x1) => "map_builder_of",
    Finished(x0) => "finished",
    Frame(x0) => "frame",
    CellOf(x0) => "cell_of",
    CellValue(x0) => "cell_value"
});
variants!(ScalarType {
    Int => "int",
    Bool => "bool",
    Byte => "byte",
    Float => "float",
    Rat => "rat",
    Char => "char",
    Str => "str",
    Range => "range",
    Any => "any",
    Descriptor => "descriptor"
});
variants!(SlotName {
    Value => "value",
    Packet => "packet",
    Deps => "deps",
    Rest => "rest",
    Selected => "selected",
    ExternalDeps => "external_deps",
    State => "state",
    From => "from",
    To => "to",
    Failure => "failure",
    Closed => "closed",
    Pending => "pending",
    Buffer => "buffer",
    Unique => "unique",
    Receipt => "receipt",
    Task => "task",
    Pin => "pin",
    Snapshot => "snapshot",
    Slot(x0) => "slot"
});
record!(ResultSlot, "result_slot", { name, ty });
variants!(Retention {
    Global(x0) => "global",
    Value(x0) => "value",
    Field { base, field } => "field",
    Index { base, index } => "index",
    Slice { base, start, end } => "slice",
    MapEntry { map, key } => "map_entry",
    MapPosition { map, index, key } => "map_position",
    Keys(x0) => "keys",
    Values(x0) => "values",
    ArrayReplace { array, index, value } => "array_replace",
    ArrayRemove { array, index } => "array_remove",
    Difference { base, removed } => "difference",
    MapMerge(x0) => "map_merge",
    Union(x0) => "union",
    ObjectFields { object, updates } => "object_fields",
    Entries(x0) => "entries",
    OwnerGraph(x0) => "owner_graph",
    ViewOwner(x0) => "view_owner",
    Descriptor { descriptor, value } => "descriptor",
    Cache(x0) => "cache",
    TakenOutcome { future, boundary } => "taken_outcome",
    AwaitFailure { future, take } => "await_failure",
    CarrierGraph(x0) => "carrier_graph"
});
record!(RetentionRule, "retention_rule", { result, selection });
variants!(ProvenanceTransition {
    Preserve => "preserve",
    Set(x0) => "set"
});
record!(OutcomeContract, "outcome_contract", { results, transfers, postconditions, retention });
variants!(TransferRule {
    CloseReserve { scope, reserve, output } => "close_reserve",
    Preserve { input, output } => "preserve",
    Move { inputs, outputs } => "move",
    Partition { input, selection, rest, selected } => "partition",
    Admit { state, packet, output, provenance } => "admit",
    ScopeTransfer { states, selection } => "scope_transfer",
    Settle { inputs } => "settle",
    Close { input } => "close",
    Protocol { protocol, transition, inputs, outputs } => "protocol",
    Publish { input, holder, provenance } => "publish",
    Extract { future, capability, boundary } => "extract",
    Reserve { packet, boundary } => "reserve",
    ExtendReservation { pending, packet } => "extend_reservation",
    ReplaceReservation { pending, packet } => "replace_reservation",
    AbortReservation { pending, cleanup, phase } => "abort_reservation",
    FrameSave { frame, values } => "frame_save",
    FrameResume { frame, slots } => "frame_resume",
    CompleteIdentity { identity, state, output } => "complete_identity"
});
variants!(Postcondition {
    OwnerServiceDuty { value, owner, kind } => "owner_service_duty",
    Closed(x0) => "closed",
    CacheConsumed(x0) => "cache_consumed",
    AdmissionStaysCaller => "admission_stays_caller",
    PreserveFullDependencies => "preserve_full_dependencies",
    TransferableScopeDomainOnly => "transferable_scope_domain_only",
    FreshAdmission(x0) => "fresh_admission",
    EpochOnRealTransfer => "epoch_on_real_transfer",
    ProvesUniqueCache(x0) => "proves_unique_cache",
    WorkerSerializationOnly => "worker_serialization_only",
    CarrierGraph(x0) => "carrier_graph",
    CleanupCompleteBeforeCommit => "cleanup_complete_before_commit",
    CancellationDeliveredBeforeCommit => "cancellation_delivered_before_commit",
    ResultRelation(x0) => "result_relation",
    NoPublication => "no_publication",
    CalleeInputsSettled => "callee_inputs_settled",
    UniqueCacheRecheckAfterCoro => "unique_cache_recheck_after_coro",
    DestroyingBeforeCall => "destroying_before_call",
    DeadAfterCall(x0) => "dead_after_call",
    ContinueCleanup => "continue_cleanup",
    Shielded => "shielded",
    ServiceSettleBeforeSerialPlan => "service_settle_before_serial_plan",
    SerialOrder => "serial_order",
    NoFreshAdmission => "no_fresh_admission",
    ReceiptDeterminesHolder => "receipt_determines_holder",
    TaskDeterminesHolder => "task_determines_holder",
    PreserveProvenance => "preserve_provenance",
    CacheConsumedIfSettled(x0) => "cache_consumed_if_settled",
    NonLinearSlotOnly => "non_linear_slot_only",
    Dynamic { target, reason, protocol } => "dynamic",
    LexicalExit { kind, boundary, selection } => "lexical_exit",
    CacheRehome(x0) => "cache_rehome",
    ConstructorFieldWrite(x0) => "constructor_field_write",
    OnceValuePlain => "once_value_plain",
    IncompleteIdentity => "incomplete_identity",
    CompleteIdentity(x0) => "complete_identity",
    DischargeMustUse(x0) => "discharge_must_use",
    CreatesMustUse => "creates_must_use",
    NoActiveFailureReplacement => "no_active_failure_replacement",
    FullCarrierGraphOwner => "full_carrier_graph_owner",
    NativeTransferableOnly => "native_transferable_only",
    ProtocolState { holder, state } => "protocol_state",
    ProtocolAlternatives { protocol, outcome, states } => "protocol_alternatives"
});
variants!(FailureKind {
    Thrown => "thrown",
    Program => "program",
    Panic => "panic",
    Native => "native",
    Cancelled => "cancelled"
});
record!(Effects, "effects", { failures, suspends, allocates, publishes, cleanup, mutates });
variants!(RootKind {
    Operands => "operands",
    ActiveSelf => "active_self",
    CleanupRemainder => "cleanup_remainder",
    PendingExit => "pending_exit",
    CarrierGraph => "carrier_graph",
    Pins => "pins",
    FrameSlots => "frame_slots",
    Cache => "cache",
    ServiceSchedule => "service_schedule"
});
record!(RootContract, "root_contract", { roots, through_suspend });
record!(CancellationContract, "cancellation_contract", { outcome, delivery });
variants!(CancellationDelivery {
    Unwind => "unwind",
    AfterShieldedCleanup => "after_shielded_cleanup"
});
record!(OpContract, "op_contract", { operands, normal, unwind, cancellation, effects, roots, entry_kind });
variants!(EntryKind {
    CallBoundary => "call-boundary",
    ObservationAdmissionStaysCaller => "observation",
    Internal => "internal"
});
variants!(FreshConvention {
    Borrow => "borrow",
    Take => "take"
});
record!(ParameterEffect, "parameter_effect", { convention, retention, deps, destination });
variants!(SummarySelection {
    Parameter(x0) => "parameter",
    Field { parameter, field } => "field",
    Path { parameter, fields } => "path",
    OwnerGraph(x0) => "owner_graph",
    Descriptor(x0) => "descriptor",
    Union(x0) => "union"
});
variants!(TransferDestination {
    None => "none",
    Result => "result",
    Scope(x0) => "scope",
    Protocol(x0) => "protocol",
    Cache => "cache",
    FailureContext => "failure_context"
});
record!(ResultRelation, "result_relation", { fresh, aliases, external_deps, provenance });
record!(Summary, "summary", { entry_kind, parameters, results, contract });
record!(CallContract, "call_contract", { results, failures, suspends, async_destroy, publication, protocol });
variants!(Publication {
    Atomic => "atomic",
    ChannelReceipt => "channel_receipt",
    TaskState => "task-state"
});
variants!(HolderKind {
    Packet => "packet",
    Scope => "scope",
    Frame => "frame",
    Cache => "cache",
    Channel => "channel",
    Carrier => "carrier",
    PendingExit => "pending_exit",
    Pin => "pin",
    Group => "group",
    Diagnostic => "diagnostic",
    Receipt => "receipt"
});
variants!(HolderState {
    Open => "open",
    Unaccepted => "unaccepted",
    Accepted => "accepted",
    Delivered => "delivered",
    Running => "running",
    Settled => "settled",
    Consumed => "consumed",
    Released => "released",
    Closed => "closed"
});
variants!(ProtocolTransition {
    Attach => "attach",
    Transfer => "transfer",
    Rehome => "rehome",
    Release => "release",
    Publish => "publish",
    Settle => "settle",
    Save => "save",
    Resume => "resume",
    Cancel => "cancel",
    Pin => "pin",
    Unpin => "unpin",
    Dynamic => "dynamic"
});
variants!(ProtocolCheck {
    LiveDependencies => "live_dependencies",
    Snapshot => "snapshot",
    Owner => "owner",
    Transferable => "transferable",
    UniqueCache => "unique-cache",
    NoReentry => "no_reentry",
    SufficientBoundary => "sufficient_boundary"
});
record!(ProtocolPoststate, "protocol_poststate", { holder, state });
record!(ProtocolRule, "protocol_rule", { transition, from, to, checks, normal, unwind, cancel });
record!(ProtocolDecl, "protocol_decl", { kind, holders, rules, roots });
variants!(ProtocolKind {
    Storage => "storage", Callable => "callable", Atomic => "atomic", Cell => "cell",
    Channel => "channel", Group => "group", Any => "any", Closure => "closure", Services => "services"
});
impl Codec for ScopeEffectEnvelope {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        r.expect("scope_effects")?;
        r.expect("(")?;
        let failures = Codec::read_field(r, "failures")?;
        let suspends = Codec::read_field(r, "suspends")?;
        let cancel = Codec::read_field(r, "cancel")?;
        r.expect(")")?;
        Ok(Self { failures, suspends, cancel })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.push("scope_effects(failures=");
        self.failures.write(w)?;
        w.push(", suspends=");
        self.suspends.write(w)?;
        w.push(", cancel=");
        self.cancel.write(w)?;
        w.push(")");
        Ok(())
    }
}
variants!(FutureEnvironment {
    Deferred { function, arguments, descriptors } => "deferred", Frame(value) => "frame"
});
impl Codec for CallCleanupMode {
    fn default_value() -> Option<Self> { Some(Self::Ordinary) }
    fn is_default(&self) -> bool { *self == Self::Ordinary }
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        if r.eat("ordinary") { return Ok(Self::Ordinary); }
        r.expect("shielded")?;
        Ok(Self::Shielded)
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.push(if *self == Self::Ordinary { "ordinary" } else { "shielded" });
        Ok(())
    }
}

record!(DescriptorContract, "descriptor_contract", { managed, retention, cleanup, async_destroy, summary });
variants!(DescriptorManaged {
    Plain => "plain",
    Managed => "managed",
    Runtime => "runtime"
});
variants!(NumericType {
    Int => "int",
    Byte => "byte",
    Float => "float",
    Rat => "rat",
    Bool => "bool",
    Str => "str"
});
record!(FieldValue, "field_value", { field, value });
variants!(ConstructionStatus {
    Complete => "complete",
    Incomplete => "incomplete"
});
record!(ConstructionCompletion, "construction_completion", { state });
record!(CallArgument, "call_argument", { value, convention });
record!(CallSite, "call_site", { args, packet, deps, descriptors, type_args, summary, cleanup });
variants!(ResultDemand {
    Want => "want",
    Discard => "discard"
});
record!(DeferAction, "defer_action", { function, captures, bindings, replacements });
record!(DeferLabel, "defer_label", { label, result });
record!(DeferRegistration, "defer_registration", { label, kind, boundary });
variants!(HighEdgeSelector { Normal(index) => "normal", Unwind => "unwind" });

variants!(TransferStates {
    Same(x0) => "same",
    Between { from, to } => "between"
});
variants!(DynamicReason {
    Atomic => "atomic",
    Cell => "cell",
    Channel => "channel",
    Group => "group",
    Any => "any",
    Closure => "closure",
    Merge => "merge"
});
record!(AccessContext, "access_context", { owner, holder, authority });
record!(ElaboratedBinding, "elaborated_binding", { binding, resource, state, original_scope });
record!(ElaboratedStage, "elaborated_stage", { action, field, resource, states });
variants!(TakeBoundary {
    Existing { owner } => "existing",
    Temporary { awaiting } => "temporary"
});
record!(UniqueCacheProof, "unique_cache_proof", { future, scope, region, evidence });
variants!(CreationOrigin {
    Instruction(x0) => "instruction",
    Invoke { block, outcome, slot } => "invoke"
});
record!(UniqueCacheEvidence, "unique_cache_evidence", { creation, aliases, frame_slots, no_external_storage,
    no_other_waiters, no_live_views, no_collector_retention, scheduler_cannot_publish });
record!(FrameSlot, "frame_slot", { slot, value, mode });
record!(SavedSlot, "saved_slot", { slot, ty, linear });
variants!(CarrierDestination {
    FailureContext(x0) => "failure_context",
    Cache(x0) => "cache",
    Diagnostic(x0) => "diagnostic"
});
variants!(ExitKind {
    Return => "return",
    Last => "last",
    Next => "next",
    Fallthrough => "fallthrough"
});
variants!(ExitDestination {
    Caller => "caller",
    Loop(x0) => "loop",
    Scope(x0) => "scope"
});
variants!(AbortPhase {
    BeforeDestroy => "before_destroy",
    AfterExecutedPrefix => "after_executed_prefix"
});
record!(PreparedExit, "prepared_exit", { kind, value, packet, deps, selection, boundary });
opcodes!(CoreOp {
    IntAdd { left, right } => "int.add",
    IntSub { left, right } => "int.sub",
    IntMul { left, right } => "int.mul",
    IntDiv { left, right } => "int.div",
    IntMod { left, right } => "int.mod",
    IntAnd { left, right } => "int.and",
    IntOr { left, right } => "int.or",
    IntXor { left, right } => "int.xor",
    IntShl { left, right } => "int.shl",
    IntShr { left, right } => "int.shr",
    IntUshr { left, right } => "int.ushr",
    IntNeg { value } => "int.neg",
    IntNot { value } => "int.not",
    IntAbs { value } => "int.abs",
    IntSign { value } => "int.sign",
    IntBitCount { value } => "int.bit_count",
    IntLeadingZeros { value } => "int.leading_zeros",
    IntTrailingZeros { value } => "int.trailing_zeros",
    IntReverseBits { value } => "int.reverse_bits",
    IntSwapBytes { value } => "int.swap_bytes",
    IntRotateLeft { value, count } => "int.rotl",
    IntRotateRight { value, count } => "int.rotr",
    ByteAnd { left, right } => "byte.and",
    ByteOr { left, right } => "byte.or",
    ByteXor { left, right } => "byte.xor",
    ByteShl { left, right } => "byte.shl",
    ByteShr { left, right } => "byte.shr",
    ByteUshr { left, right } => "byte.ushr",
    ByteNot { value } => "byte.not",
    ByteBitCount { value } => "byte.bit_count",
    ByteLeadingZeros { value } => "byte.leading_zeros",
    ByteTrailingZeros { value } => "byte.trailing_zeros",
    ByteReverseBits { value } => "byte.reverse_bits",
    ByteSwapBytes { value } => "byte.swap_bytes",
    ByteRotateLeft { value, count } => "byte.rotl",
    ByteRotateRight { value, count } => "byte.rotr",
    FloatAdd { left, right } => "float.add",
    FloatSub { left, right } => "float.sub",
    FloatMul { left, right } => "float.mul",
    FloatDiv { left, right } => "float.div",
    FloatMod { left, right } => "float.mod",
    FloatPow { left, right } => "float.pow",
    FloatNeg { value } => "float.neg",
    FloatAbs { value } => "float.abs",
    FloatSign { value } => "float.sign",
    FloatSqrt { value } => "float.sqrt",
    RatAdd { left, right } => "rat.add",
    RatSub { left, right } => "rat.sub",
    RatMul { left, right } => "rat.mul",
    RatDiv { left, right } => "rat.div",
    RatMod { left, right } => "rat.mod",
    RatNeg { value } => "rat.neg",
    RatAbs { value } => "rat.abs",
    RatSign { value } => "rat.sign",
    IntEq { left, right } => "int.eq",
    IntNe { left, right } => "int.ne",
    IntLt { left, right } => "int.lt",
    IntLe { left, right } => "int.le",
    IntGt { left, right } => "int.gt",
    IntGe { left, right } => "int.ge",
    ByteEq { left, right } => "byte.eq",
    ByteNe { left, right } => "byte.ne",
    ByteLt { left, right } => "byte.lt",
    ByteLe { left, right } => "byte.le",
    ByteGt { left, right } => "byte.gt",
    ByteGe { left, right } => "byte.ge",
    FloatEq { left, right } => "float.eq",
    FloatNe { left, right } => "float.ne",
    FloatLt { left, right } => "float.lt",
    FloatLe { left, right } => "float.le",
    FloatGt { left, right } => "float.gt",
    FloatGe { left, right } => "float.ge",
    RatEq { left, right } => "rat.eq",
    RatNe { left, right } => "rat.ne",
    RatLt { left, right } => "rat.lt",
    RatLe { left, right } => "rat.le",
    RatGt { left, right } => "rat.gt",
    RatGe { left, right } => "rat.ge",
    BoolEq { left, right } => "bool.eq",
    BoolNe { left, right } => "bool.ne",
    EnumEq { left, right } => "enum.eq",
    EnumNe { left, right } => "enum.ne",
    CharEq { left, right } => "char.eq",
    CharNe { left, right } => "char.ne",
    CharLt { left, right } => "char.lt",
    CharLe { left, right } => "char.le",
    CharGt { left, right } => "char.gt",
    CharGe { left, right } => "char.ge",
    StrEq { left, right } => "str.eq",
    StrNe { left, right } => "str.ne",
    StrLt { left, right } => "str.lt",
    StrLe { left, right } => "str.le",
    StrGt { left, right } => "str.gt",
    StrGe { left, right } => "str.ge",
    ValEq { left, right } => "val.eq",
    ValNe { left, right } => "val.ne",
    BoolNot { value } => "bool.not",
    RatNew { numerator, denominator } => "rat.new",
    RatNumerator { value } => "rat.numerator",
    RatDenominator { value } => "rat.denominator",
    Convert { value, from, to } => "cvt",
    ByteFrom { value, context } => "byte.from",
    Undef { ty } => "undef",
    Global { declaration, ty } => "global",
    StrConcat { left, right } => "str.concat",
    StrLen { value } => "str.len",
    StrUtf8 { value } => "str.utf8",
    StrSlice { value, start, end } => "str.slice",
    StrSplit { value, separator } => "str.split",
    StrReplace { value, target, replacement } => "str.replace",
    StrReplaceFirst { value, target, replacement } => "str.replace_first",
    StrRepeat { value, count } => "str.repeat",
    StrTrim { value } => "str.trim",
    StrChomp { value } => "str.chomp",
    StrTrimStart { value } => "str.trim_start",
    StrTrimEnd { value } => "str.trim_end",
    StrLower { value } => "str.lower",
    StrUpper { value } => "str.upper",
    StrFind { value, part } => "str.find",
    StrFindLast { value, part } => "str.find_last",
    StrStartsWith { value, part } => "str.starts_with",
    StrEndsWith { value, part } => "str.ends_with",
    StrCharacters { value } => "str.characters",
    RangeNew { start, end, inclusive } => "range.new",
    RangeStart { value } => "range.start",
    RangeEnd { value } => "range.end",
    RangeLen { value } => "range.len",
    ObjNew { declaration, type_args, fields, packet, construction } => "obj.new",
    ObjNext { object, updates, packet } => "obj.next",
    ObjInit { object, field, value, completion } => "obj.init",
    ObjGet { object, field } => "obj.get",
    ObjPending { object, field } => "obj.pending",
    OnceLoad { object, field } => "once.load",
    OnceStore { object, field, value } => "once.store",
    SuccTake { value, ty } => "succ.take",
    RoleWrap { value, ty } => "role.wrap",
    RoleNarrow { value, ty } => "role.narrow",
    VariantOf { value, ty } => "variant.of",
    Retag { value, ty } => "retag",
    VariantNew { ty, case, fields, packet } => "variant.new",
    VariantTag { value } => "variant.tag",
    VariantGet { value, case, field } => "variant.get",
    CapMake { owner, ty, packet } => "cap.make",
    FnClosure { function, captures, ty, packet } => "fn.closure",
    FnBind { receiver, function, ty, packet } => "fn.bind",
    FnBindRole { receiver, role, slot, ty, packet } => "fn.bind.role",
    AnyBox { value, ty, packet } => "any.box",
    FailSecondary { primary, cleanup } => "fail.secondary",
    FailCaused { new, handled } => "fail.caused",
    FailIsThrown { failure } => "fail.is_thrown",
    TypeOf { ty, descriptor } => "type.of",
    TestOk { condition } => "test.ok",
    TestIs { left, right } => "test.is",
    TestIsnt { left, right } => "test.isnt",
    TestFail { message } => "test.fail",
    EnumCase { declaration, case } => "enum.case",
    FakeNew { declaration, packet } => "fake.new",
});
opcodes!(CallOp {
    Direct { function, call } => "call",
    Role { receiver, role, slot, call } => "call.role",
    Closure { callee, call } => "fn.call",
    Cap { cap, field, call } => "call.cap",
    Native { function, call } => "call.native",
    Intrinsic { function, call } => "call.intrinsic",
    Resume { frame, function, call } => "call.resume",
});
opcodes!(CollectionOp {
    ArrayNew { values, ty, packet } => "array.new",
    ArraySlice { array, start, end, packet } => "array.slice",
    ArrayConcat { arrays, ty, packet } => "array.concat",
    ArrayReplace { array, index, value, packet } => "array.replace",
    ArrayRemove { array, index, packet } => "array.remove",
    ArrayDifference { array, removed, packet } => "array.difference",
    MapNew { entries, ty, packet } => "map.new",
    MapMerge { maps, ty, packet } => "map.merge",
    MapDifference { map, removed, packet } => "map.difference",
    ArrayLen { array } => "array.len",
    ArrayGet { array, index } => "array.get",
    ArrayJoin { array, separator } => "array.join",
    ArrayRetag { array, ty } => "array.retag",
    MapLen { map } => "map.len",
    MapHas { map, key } => "map.has",
    MapGet { map, key } => "map.get",
    MapKeys { map } => "map.keys",
    MapValues { map } => "map.values",
    MapKeyAt { map, index } => "map.key_at",
    MapValueAt { map, index } => "map.value_at",
    MapRetag { map, ty } => "map.retag",
    BuilderNew { element } => "builder.new",
    BuilderPush { buffer, value, packet } => "builder.push",
    BuilderAppend { buffer, array, packet } => "builder.append",
    BuilderAppendRange { buffer, start, end, inclusive, packet } => "builder.append.range",
    BuilderFinish { buffer, packet } => "builder.finish",
    BuilderAbandon { buffer, packet } => "builder.abandon",
    MapBuilderNew { key, value } => "mbuilder.new",
    MapBuilderPut { buffer, key, value, packet } => "mbuilder.put",
    MapBuilderAppend { buffer, map, packet } => "mbuilder.append",
    MapBuilderFinish { buffer, packet } => "mbuilder.finish",
    MapBuilderAbandon { buffer, packet } => "mbuilder.abandon",
});
opcodes!(LexicalOp {
    Bind { binding, value } => "bind",
    Rebind { binding, value, target } => "rebind",
    Stage { action, field, value } => "stage",
    Temp { value, scope } => "temp",
    DeferRegister { scope, action } => "defer.register",
    Want { value } => "want",
    Discard { value } => "discard",
});
opcodes!(OwnershipOp {
    Empty {  } => "own.empty",
    Move { packet } => "own.move",
    Join { packets } => "own.join",
    Admit { state, packet } => "own.admit",
    Accept { state, packet } => "own.accept",
    Bind { binding } => "own.bind",
    Rebind { binding } => "own.rebind",
    Stage { stage } => "own.stage",
    Split { packet, selection } => "own.split",
    SplitChecked { packet, selection } => "own.split.checked",
    ScopeBegin { scope } => "scope.begin",
    ScopeSplit { state, selection } => "scope.split",
    ScopeTransfer { states, selection } => "scope.transfer",
    Dynamic { state, packet, target, reason, protocol } => "own.dynamic",
    DynamicCheck { key, snapshot } => "dynamic.check",
    DynamicRefresh { key, access } => "dynamic.refresh",
    ScopeEnd { state, reserve, effects } => "scope.end",
    ScopeUnwind { state, failure, reserve, effects } => "scope.unwind",
    End { packet, boundary } => "end",
});
opcodes!(AsyncOp {
    Await { future, result } => "await",
    AwaitShielded { future, result } => "await.shielded",
    AwaitTake { future, result, unique, boundary } => "await.take",
    UniqueCacheProof { proof } => "unique_cache.proof",
    CacheRehome { state, target, protocol } => "cache.rehome",
    FutureSpawn { future } => "fut.spawn",
    FutureCancel { future } => "fut.cancel",
    FutureDone { future } => "fut.done",
    FutureCancelled { future } => "fut.cancelled",
    FutureNew { packet, deps, owner_state, environment, result, owner } => "fut.new",
    FuturePublish { task, cache, value, packet, deps, failure, protocol } => "fut.publish",
    FutureSettle { task, protocol } => "fut.settle",
    FrameAlloc { function } => "frame.alloc",
    FrameLoad { frame, slot, ty } => "frame.load",
    FrameStore { frame, slot, value } => "frame.store",
    FrameSave { frame, slots, continuation } => "frame.save",
    FrameResume { frame, slots } => "frame.resume",
    FrameCancel { frame, cleanup, protocol } => "frame.cancel",
    Pin { value, deps, owner, protocol } => "pin",
    PinTransfer { pin, owner, protocol } => "pin.transfer",
    Unpin { pin, owner, protocol } => "unpin",
    WorkerBind { action, entry, fields, ty } => "worker.bind",
    WorkerSpawn { entry, args, direct, result } => "worker.spawn",
    ProcessSpawn { constructor, constructor_args, action, args, result } => "proc.spawn",
});
opcodes!(CarrierOp {
    Attach { failure, packet, owner_state, protocol } => "carrier.attach",
    Transfer { state, target, protocol } => "carrier.transfer",
    Rehome { state, owner_state, protocol } => "carrier.rehome",
    Release { state, owner_state, protocol } => "carrier.release",
});
opcodes!(ExitOp {
    Prepare { exit } => "exit.prepare",
    Replace { pending, replacement } => "exit.replace",
    Commit { pending, destination, result } => "exit.commit",
    Extend { pending, packet } => "exit.extend",
    Abort { pending, cleanup, phase } => "exit.abort",
});
opcodes!(CleanupOp {
    SettleEmpty { cleanup, schedule } => "cleanup.settle.empty",
    PlanChecked { cleanup, plan } => "cleanup.plan.checked",
    DestroySelected { cleanup, selected } => "cleanup.destroy",
    Begin { state, pending, failure, schedule } => "cleanup.begin",
    Defer { cleanup, action } => "cleanup.defer",
    Settle { cleanup, holder, protocol } => "cleanup.settle",
    Plan { cleanup } => "cleanup.plan",
    Next { cleanup } => "cleanup.next",
    DestroyDirect { cleanup, identity, descriptor, destroy } => "destroy.direct",
    RecordFailure { cleanup, failure } => "cleanup.failure",
    Shield { cleanup } => "cleanup.shield",
    DeliverCancel { cleanup } => "cleanup.deliver_cancel",
    Finish { cleanup } => "cleanup.finish",
    CellNew { value, ty } => "cell.new",
    CellLoad { cell } => "cell.load",
    CellStore { cell, value } => "cell.store",
});

fn decimal(text: &str) -> Option<String> {
    let (negative, digits) = text.strip_prefix('-').map_or((false, text), |s| (true, s));
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        Some("0".into())
    } else {
        Some(format!("{}{digits}", if negative { "-" } else { "" }))
    }
}
impl Codec for Literal {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        let token = r.next()?;
        if token.character {
            let mut chars = token.text.chars();
            let value = chars.next().filter(|_| chars.next().is_none())
                .ok_or_else(|| r.at(&token, DiagnosticKind::Literal, "expected one Unicode scalar"))?;
            return Ok(Self::Char(value));
        }
        if token.quoted {
            return Ok(Self::Str(token.text));
        }
        if matches!(token.text.as_str(), "true" | "false") {
            return Ok(Self::Bool(token.text == "true"));
        }
        if token.text == "int" {
            r.expect("(")?;
            let value = r.decimal()?;
            r.expect(")")?;
            return Ok(Self::Int(value));
        }
        if token.text == "byte" {
            r.expect("(")?;
            let value = u8::read(r)?;
            r.expect(")")?;
            return Ok(Self::Byte(value));
        }
        if token.text == "rat" {
            r.expect("(")?;
            let numerator = r.decimal()?;
            r.expect(",")?;
            let denominator = r.decimal()?;
            r.expect(")")?;
            return Ok(Self::Rat { numerator, denominator });
        }
        if token.text == "enum" {
            r.expect("(")?;
            let decl = DeclId::read(r)?;
            r.expect(",")?;
            let case = u32::read(r)?;
            r.expect(")")?;
            return Ok(Self::Enum { decl, case });
        }
        if let Some((owner, member)) = token.text.rsplit_once('.')
            && owner.chars().next().is_some_and(char::is_alphabetic)
        {
            let mut owner_token = token.clone();
            owner_token.text = owner.into();
            let decl = DeclId::from_u32(r.named_reference("decl", &owner_token, false)?);
            let case = if r.first {
                0
            } else {
                r.names.get(&(decl.as_u32() as usize, "case", member.into())).copied()
                    .ok_or_else(|| r.at(&token, DiagnosticKind::UnknownName, "unknown enum case"))?
            };
            return Ok(Self::Enum { decl, case });
        }
        if token.text.contains(['.', 'e', 'E']) || matches!(token.text.as_str(), "NaN" | "inf" | "-inf") {
            let value = token.text.parse::<f64>().ok().filter(|v| v.is_finite())
                .ok_or_else(|| r.at(&token, DiagnosticKind::Literal, "expected finite binary64 decimal"))?;
            return Ok(Self::Float(value.to_bits()));
        }
        let decimal = decimal(&token.text)
            .ok_or_else(|| r.at(&token, DiagnosticKind::Literal, "invalid literal"))?;
        if r.literal_type.and_then(|id| r.types.get(id).ok()) == Some(&Type::Byte) {
            return Ok(Self::Byte(decimal.parse().map_err(|_| r.at(&token, DiagnosticKind::Literal, "byte range"))?));
        }
        Ok(Self::Int(decimal))
    }

    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        fn integer(w: &mut Writer<'_>, text: &str) -> Result<(), PrintError> {
            if decimal(text).as_deref() != Some(text) {
                return Err(PrintError::new("non-normalized decimal integer"));
            }
            w.push(text);
            Ok(())
        }
        match self {
            Self::Int(text) => {
                let needs_tag = w.literal_type.and_then(|id| w.module.types.get(id).ok()) == Some(&Type::Byte);
                if needs_tag {
                    w.push("int(");
                }
                integer(w, text)?;
                if needs_tag {
                    w.push(")");
                }
            },
            Self::Bool(value) => value.write(w)?,
            Self::Byte(value) => {
                if w.literal_type.and_then(|id| w.module.types.get(id).ok()) == Some(&Type::Byte) {
                    value.write(w)?;
                } else {
                    w.push("byte(");
                    value.write(w)?;
                    w.push(")");
                }
            }
            Self::Float(bits) => {
                let value = f64::from_bits(*bits);
                if !value.is_finite() {
                    return Err(PrintError::new("non-finite float"));
                }
                w.push(&format!("{value:?}"));
            }
            Self::Rat { numerator, denominator } => {
                w.push("rat(");
                integer(w, numerator)?;
                w.push(", ");
                integer(w, denominator)?;
                w.push(")");
            }
            Self::Char(value) => {
                w.push("'");
                match value {
                    '\\' => w.push("\\\\"), '\'' => w.push("\\'"), '\n' => w.push("\\n"),
                    '\r' => w.push("\\r"), '\t' => w.push("\\t"),
                    c if c.is_control() => w.push(&format!("\\u{{{:X}}}", u32::from(*c))),
                    c => w.push(&c.to_string()),
                }
                w.push("'");
            }
            Self::Str(text) => text.write(w)?,
            Self::Enum { decl, case } => {
                let name = w.module.declarations.get(*decl).ok().and_then(|decl| match &decl.kind {
                    DeclarationKind::Enum { cases, .. } => cases.get(*case as usize).map(String::as_str),
                    _ => None,
                });
                if let Some(name) = name {
                    decl.write(w)?;
                    w.push(".");
                    w.word(name)?;
                } else {
                    w.push("enum(");
                    decl.write(w)?;
                    w.push(", ");
                    case.write(w)?;
                    w.push(")");
                }
            }
        }
        Ok(())
    }
}
impl Codec for Operation {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        if let Some(op) = surface::read_operation(r)? {
            return Ok(op);
        }
        let opcode = r.peek().text.as_str();
        if opcode == "const" {
            r.expect("const")?;
            return Ok(Self::Const(Literal::read(r)?));
        }
        macro_rules! groups { ($($group:ty => $variant:ident),*) => {
            $(
                if <$group>::OPCODES.contains(&opcode) {
                    return Ok(Self::$variant(<$group>::read(r)?));
                }
            )*
        };
        }
        groups!(CoreOp => Core, LexicalOp => Lexical, OwnershipOp => Ownership, CallOp => Call,
            CollectionOp => Collection, AsyncOp => Async, CarrierOp => Carrier, ExitOp => Exit, CleanupOp => Cleanup);
        Err(r.error(DiagnosticKind::Syntax, "unknown opcode"))
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        if surface::write_operation(self, w)? {
            return Ok(());
        }
        match self {
            Self::Const(op) => {
                w.push("const ");
                op.write(w)
            }
            Self::Core(op) => op.write(w), Self::Lexical(op) => op.write(w), Self::Ownership(op) => op.write(w),
            Self::Call(op) => op.write(w), Self::Collection(op) => op.write(w), Self::Async(op) => op.write(w),
            Self::Carrier(op) => op.write(w), Self::Exit(op) => op.write(w), Self::Cleanup(op) => op.write(w),
        }
    }
}

pub(crate) fn slot_name(name: SlotName) -> String {
    match name {
        SlotName::Value => "value", SlotName::Packet => "packet", SlotName::Deps => "deps", SlotName::Rest => "rest",
        SlotName::Selected => "selected", SlotName::ExternalDeps => "external_deps", SlotName::State => "state",
        SlotName::From => "from", SlotName::To => "to", SlotName::Failure => "failure", SlotName::Closed => "closed",
        SlotName::Buffer => "buffer", SlotName::Unique => "unique", SlotName::Receipt => "receipt",
        SlotName::Task => "task",
        SlotName::Pending => "pending", SlotName::Pin => "pin", SlotName::Snapshot => "snapshot", SlotName::Slot(i) => return format!("slot{i}"),
    }
        .into()
}

impl Codec for Signature {
    fn read(r: &mut Reader<'_>) -> Result<Self, Diagnostic> {
        r.expect("(")?;
        let mut params = vec![];
        while !r.is(")") {
            r.entity()?;
            params.push(TypeId::read(r)?);
            r.comma()?;
        }
        r.expect(")")?;
        r.expect("->")?;
        let results = r.results()?;
        let ownership_abi = if r.eat("abi") {
            OwnershipAbi::read(r)?
        } else {
            OwnershipAbi::Unelaborated
        };
        Ok(Self { params, results, ownership_abi })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), PrintError> {
        w.push("(");
        for (i, ty) in self.params.iter().enumerate() {
            if i > 0 {
                w.push(", ");
            }
            ty.write(w)?;
        }
        w.push(") -> ");
        w.results(&self.results)?;
        if self.ownership_abi != OwnershipAbi::Unelaborated {
            w.push(" abi ");
            self.ownership_abi.write(w)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) const TEST_NAMES: &[(&str, &str)] = &[("type", "t0"), ("decl", "D0"), ("func", "@f0"), ("scope", "S0"),
    ("file", "0"), ("summary", "M0"), ("protocol", "P0"), ("block", "^b0"), ("inst", "i0"), ("value", "%0"),
    ("binding", "b0"), ("boundary", "B0"), ("dependency", "d0"), ("key", "k0"), ("selection", "s0")];
#[cfg(test)]
pub(crate) fn payload_roundtrip<T: Codec + std::fmt::Debug + PartialEq + 'static>(value: T) {
    let text = print::payload_print(&value);
    // Nominal spelling is resolved by its declaration, never by an inline tag.
    let nominal_kind = match (&value as &dyn std::any::Any).downcast_ref::<Type>() {
        Some(Type::Role { .. }) => 1, Some(Type::Enum(_)) => 2, Some(Type::Variant(_)) => 3,
        Some(Type::Handle(_)) => 4, Some(Type::Cap {
                ..
        }) => 5, _ => 0,
    };
    let parsed: T = parser::payload_read(&text, nominal_kind);
    assert_eq!(value, parsed, "payload text: {text}");
}

// Defaults are representational facts, not inferred managed dependencies.
fn value_default(profile: Profile, ty: Option<&Type>) -> ValueFacts {
    if profile == Profile::High {
        return ValueFacts::Unelaborated;
    }
    match ty {
        Some(Type::Service(_)) => ValueFacts::Service,
        Some(Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char
            | Type::Str | Type::Range | Type::Descriptor) => {
            ValueFacts::Plain { certificate: PlainCertificate::Scalar }
        }
        _ => ValueFacts::Unelaborated,
    }
}

record!(CleanupPlan, "cleanup_plan", { entries, merge, order, roots, effects });
record!(CleanupOrder, "cleanup_order", { creations });
record!(CleanupRoots, "cleanup_roots", { values, kinds, through_suspend });
