# Okkama standard-library design principles

This is the design standard for `std/`, alongside [roadmap v4](stdlib-roadmap-v4.md).
It governs library design, not additions to language semantics. `stdlib/` is the
separate legacy library; its APIs are not automatically promoted into `std/`.

## Evidence, interpretation and constraints

The repository contains no complete copy of the Elegant Objects books. The
public [book extract](https://www.yegor256.com/images/books/elegant-objects/seven-pages.pdf)
and [contents](https://www.yegor256.com/images/books/elegant-objects/contents.pdf)
were checked, together with the author's articles below. No claim is made to
have read unavailable volumes during this review.

- **EO principle:** an object represents an entity; private coordinates remain
  stable even when the represented resource changes. File/network effects do
  not themselves make an object mutable.
  [Immutable Objects Are Not Dumb](https://www.yegor256.com/2014/12/22/immutable-objects-not-dumb.html).
- **EO principle:** objects should encapsulate behavior, avoid static utility
  methods and job-title classes, and extend through composition.
  [Seven Virtues](https://www.yegor256.com/2014/11/20/seven-virtues-of-good-object.html),
  [Composable Decorators](https://www.yegor256.com/2015/02/26/composable-decorators.html).
- **EO principle:** getters expose implementation decisions; an explicit
  printing protocol can let an object control how it presents itself.
  [Getters/Setters](https://www.yegor256.com/2014/09/16/getters-and-setters-are-evil.html),
  [Printers Instead of Getters](https://www.yegor256.com/2016/04/05/printers-instead-of-getters.html).
- **EO principle:** prefer assignment-only constructors and one primary
  constructor. Computation at construction prevents deferred composition.
  [Code-Free Constructors](https://www.yegor256.com/2015/05/07/ctors-must-be-code-free.html),
  [One Primary Constructor](https://www.yegor256.com/2015/05/28/one-primary-constructor.html).
  The later [Prestructors](https://www.yegor256.com/2021/08/04/prestructors.html)
  discusses explicit preprocessing; this does not justify arbitrary hidden I/O.
- **EO principle:** separate result-producing builders from void manipulators.
  [Builders and Manipulators](https://www.yegor256.com/2018/08/22/builders-and-manipulators.html).
  The [RAII article](https://www.yegor256.com/2017/08/08/raii-in-java.html) also
  demonstrates explicit acquisition followed by deterministic release.
- **EO principle:** avoid null and reflection-dependent coupling.
  [Why NULL Is Bad](https://www.yegor256.com/2014/05/13/why-null-is-bad.html),
  [Reflection Means Hidden Coupling](https://www.yegor256.com/2022/06/05/reflection-means-hidden-coupling.html).
- **Okkama constraint:** equality is the language's existing structural/nominal
  operation, not an overridable Java `equals(Object)`. The author's
  [alternative equality design](https://www.yegor256.com/2017/07/11/how-to-redesign-equals.html)
  informs the review but does not authorize changing interpreter equality.

The rules below are Okkama architectural decisions derived from these sources.
Numeric action-count limits, global uniqueness of class names, or a ban on all
primitive results are not substitutes for evaluating cohesion.

## Objects and contracts

1. State what each object represents and which private coordinates identify
   it. A container of foreign-operation methods is not an object model.
2. Private fields are not a public schema. Reject getters that expose handles,
   algorithm state or redundant stored facts. Do not merely rename `getData()`.
3. Primitive results are allowed for meaningful observations or explicit
   representations: `Path.string()`, `Duration.seconds()`, cursor `item()`,
   error `text()`, and encoding `bytes()`. A bag of unrelated fields has no
   such justification. The API audit must explain each retained family.
4. Value objects must enforce their relations. A storage observation cannot
   accept contradictory capacity/available/used facts; a fitted line must
   predict, not just export coefficients. Frozen transport results are allowed
   only at an explicit protocol/snapshot boundary and must preserve coherence.
5. A role specifies observable behavior. It is not an abstract base class,
   reflection marker or hypothetical generic framework. Prefer the smallest
   role useful to two concrete collaborators; native-specialized capabilities
   are explicit exceptions, not promises of arbitrary user implementations.
6. Prefer decorators preserving a role to unrelated operations on a facade.
   Persistent collections return new representations; they never mutate the
   receiver. `prefix`, `addition`, `difference`, `tail` and cursor `successor`
   describe values. Do not introduce mutable `push`, `set` or `remove`.
7. A stored action uses `(parameter is T) is R { ... }`; its type is `(T) is R`.
   A callback is an explicit dependency, not permission for ambient state.

## Construction, invariants and lifetime

The interpreter forbids every method call on unfinished constructor `self`,
including public/private/async methods and calls inside `confirm`. Aliases,
roles and captured actions do not grant an exception. Initialize fields and
check invariants directly with `confirm`; methods of parameters and other
completed collaborators remain available. This enforced rule does not by itself
prove a general effect system or constructor purity.

Prefer constructors that bind complete dependencies. Validate local invariants
with `confirm ... else throw` in each constructor. Constructors should not perform unrelated effects.
Okkama does not support Java-style `this(...)` constructor delegation; do not
invent it. Keep validation explicit in each constructor and use composed representations.

The existing interpreter owns opaque resource states and pure memo cells.
Allocation of an unopened file state or an uncompiled regex state is an
implementation mechanism, not file I/O or regex compilation. Keep it private.
Immutable snapshots may materialize input once to prevent later observations
from changing identity. This is a documented preconstruction compromise, not
an EO claim that eager work is generally desirable. Live filesystem/account
observations must be distinguished from snapshots.

Acquiring a temporary resource necessarily establishes ownership. Keep its
capability explicit, its cleanup deterministic and its failure atomic. Never
simulate laziness with mutable public flags or a native handle getter. A future
redesign of acquisition must preserve the existing `destroy` reachability rules.
Borrowed decorators do not close their origins; owners close their own states.
Views retaining the same state share position/closed status. Closing an unused
lazy file must not create it. Verify escapes, aliases, exceptions and repeated
close calls against actual behavior.

Native handles themselves are not ordinary ObjectValues in the scope ownership
graph. Registry teardown is a final safety net, not scope RAII. File,
TemporaryFile, TemporaryDirectory and FileLock implement `destroy { self.close(); }`;
views retain their owner object, so escaping a view retains cleanup responsibility.
A pure returned Path must not retain a resource. For other native-backed capabilities,
follow their explicit close/defer contract until scoped ownership is separately
implemented and tested; do not infer it merely from a `has state` field.

## Effects and naming

A value/query representation must not delete, persist, launch or overwrite as
an incidental projection. Commands on explicit capabilities use verbs and
`is void`. Returning a path after deleting it does not make deletion a query.

```okkama
// BAD: asking for a projected path secretly deletes the directory.
object RemovedDirectory {
    has origin is Directory;
    RemovedDirectory(:origin);
    #projected
    value() is Path { origin.remove(); origin.path(); }
}

// GOOD: this object denotes the tree on which the command operates.
object DirectoryTree {
    has origin is DirectoryLocating;
    DirectoryTree(:origin);
    remove() is void { /* remove this tree through a portable adapter */ }
}
```

`write`, `flush`, `close`, `remove` and `preserve` are legitimate capability
commands. `value()` is permitted when a projected object denotes exactly that
computed value (for example `ParsedInt` or `Minimum`), not for a record unwrap
or an effect disguised as a result. Do not rename every scalar to `value`.

Strict builder/manipulator separation needs declared adaptations for consuming
input, random draws, process acquisition and async synchronization: these
operations observe/advance external capabilities and must return data or an
owned capability. Their receiver and contract make the effect explicit. Effect-only async commands declare `is void`; awaiting them produces no value. Do not create `Future<void>`, `Unit`, or null-like sentinels.

## Native and serialization boundaries

The layering is host primitive → `runtime/*` substrate → behavioral `std/`
object → application. Native parameters may contain portable wire data and
opaque states; high-level actions must not return those states. No errno,
host structs, Perl names or reflection-driven field enumeration in public APIs.
Use controlled error objects and preserve meaningful failure details.

JSON null is an actual JSON literal, never absence of an Okkama object.
Serializers use explicit JSON protocols and control escaping themselves.
Formatters accept explicit text representations, not arbitrary object graphs.

```okkama
// BAD: hypothetical Formatter().process(anything) inspects private fields.
// GOOD: dependencies and their representations are explicit.
let label = Formatted(pattern: "user {}", values: [user.name()]);
output.write(label.string());
```

Missing members/items use typed errors or a documented fallback. Do not export
`-1`, empty strings or fake objects as universal absence. Empty collections and
EOF remain meaningful protocol values with their established contracts.

## Review and verification gate

For each public type/action record identity, responsibility, effects, lifetime,
representation boundary, native dependency and a keep/change/remove decision.
Inspect the implementation and tests; naming alone cannot establish compliance.

Before implementation: update v4, write behavior tests and demonstrate failure.
Tests use public roles, fakes with observable results and real resource
boundaries where appropriate. Do not test private layouts to freeze a design.
Cover invariants, empty inputs, failure paths, immutable aliases, borrowed/owned
resources and neighboring-module composition. Run focused, stdlib and complete
project suites; migrate examples and remove obsolete APIs without silent aliases.
A deferred protocol is not implemented merely because its enums exist.

## Shared infrastructure state

`async/atomic::Atomic<T>` denotes a synchronized cell with stable identity.
Its `value()` is a live observation of that cell, not a private-field getter.
`replace` is a void command. `compare_and_replace` is the explicit synchronization
exception to builder/manipulator separation: its boolean reports publication.
Neither operation changes Okkama fields or introduces mutable references.

Use `Atomic` rather than a one-slot Channel for shared snapshot state; retain
Channel for message transfer/backpressure and Lock for multi-operation critical
sections. Atomic operations do not suspend and synchronize only tasks in one
interpreter. Value CAS uses existing equality (and therefore permits ABA); an
application requiring monotonic publication must include and validate revision.
Two Atomic capabilities compare by cell identity, including when nested inside
an ordinary value. No user comparator or mutation callback executes inside CAS.

The cell borrows resource capabilities embedded in its payload. It does not
transfer scoped destroy responsibility or extend those resources' lifetimes.
Use immutable domain snapshots for repository state. `close` releases the payload
reference and closes all aliases; `closed` implements the existing Closing role.
See the full [contract](../../std/async/atomic.ok) and the
invoice integration.
