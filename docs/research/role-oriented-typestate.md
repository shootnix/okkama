# Role-oriented typestate in Okkama

Research report, 2026-11-03. Branch `nom` at `57d8b0aa` (okkama 0.3.4).
Branch `develop` does not yet contain payload enums, `Maybe`/`Presence` or
compact `?? / !!` choices, so the work was done on `nom`, which has them.
No compiler, runtime or standard library file was changed.

Experiments: [experiments/role-typestate/](../../experiments/role-typestate/README.md)
(92 checked programs and expectations in the original run, `run.sh`, exact
diagnostics in [results.md](../../experiments/role-typestate/results.md)).

**Update, same day: compiler fixes applied.** The research itself was done on
the unchanged compiler, and the sections below describe what it found. Four
defects it reported were then fixed (CHANGELOG, Unreleased → Fixed):
object-level `is ^Role` is now a clear semantic error instead of an internal
compiler error; a successor body ending with a value of its own type is
rejected; role-bounded generics with a role argument run on every engine;
the rebind message for role-typed locals is corrected. `okkama lint` also
warns about non-exhaustive matches over payload-free enums. The experiment
suite (91 programs after the update) is a regression check: `make
experiments`. Remarks marked *(fixed)* below refer to these changes.

## 1. Executive summary

**Yes: Okkama already supports a useful form of typestate with roles,
successors and ADTs, and needs no new syntax for it. It does not provide
full static protocol safety.**

The working pattern ("state views") is: one immutable class implements a role
per state; each role exposes only that state's operations; transitions are
successors declared on the roles as `^NextRole`; alternative outcomes are a
payload ADT whose payloads are role-typed views; a private payload ADT field
records the real state, and `confirm` checks it again in each transition.

The current compiler provides:

| Guarantee | Provided? |
|---|---|
| **Interface restriction**: a state view exposes only its operations, through aliases, parameters, generics, ADT payloads, `Maybe`/`Presence` and `any` | Yes, statically, and the restricted reference also exists at runtime |
| **Transition result must be used**: a successor result cannot be discarded or left unused on any path | Yes, statically (successors only) |
| **Exhaustive handling of alternative outcomes** | Yes, statically (payload/generic enums) |
| **State restriction**: an operation is possible only in the real state | Only at runtime, and only if the model keeps a private state field checked with `confirm` |
| **Transition consumption**: a state value cannot be used after its transition | No |
| **No duplicate transitions** (double authorization/capture) | No. Inside one process this needs consumption; across processes it needs durable state and idempotency, whatever the type system |

So Okkama gives the "states as types" part of typestate (state-specific APIs,
typed transitions, typed outcomes). It does not give the linear part, where
each state value can be used only once.

## 2. Current language capabilities (as implemented)

Each claim below was checked in the Rust implementation and by an experiment.

**Roles are nominal, and role-typed values are restricted references.**
A method call is resolved against the static type: `object_method` for
classes and `role_method` for roles, which reports
`Role "X" has no action "y"`
([call.rs:1192, 1270, 1282](../../okkama-rs/crates/okkama-compiler/src/analysis/check/call.rs)).
A concrete value crosses into a role only if the class `does` that role. A
role crosses into another role only if the target's actions are a subset of
the source's actions and some object implements both (`assignable`, `narrows`,
[registry.rs:2275, 2340](../../okkama-rs/crates/okkama-compiler/src/analysis/registry.rs)).
Nothing converts a role reference back to a class: there is no cast syntax,
and `any` is opaque (`advanced/b_role_to_concrete`, `advanced/b_any_roundtrip`).
Lowering emits an explicit `RoleWrap`
([adapters.rs:446](../../okkama-rs/crates/okkama-compiler/src/lowering/body/adapters.rs)).
*Correction:* only Sparrow executes it as a real wrapper. The IR interpreter
returns the same object (`okkama-runtime/src/interpreter.rs`, `RoleWrap`) and
the JVM backend emits a plain load (`codegen/jvm/function.rs`), so on those
engines the restriction is enforced by the checker alone. This is sound as
long as no runtime path inspects the concrete class, which `catch` does
(§7).

**Successors always produce the same class.** A successor result must name an
object or role ([registry.rs:1512](../../okkama-rs/crates/okkama-compiler/src/analysis/registry.rs)).
On an object it must be `^Self` or a role (registry.rs:1843). The body only
stages fields, and `return value;` is rejected
([stmt.rs:391](../../okkama-rs/crates/okkama-compiler/src/analysis/check/stmt.rs)).
A successor therefore always produces "the receiver with updated fields".
It cannot change class.

**Covariant role successors.** A role signature `authorize() is ^Authorized`
may be implemented by `authorize() is ^Payment` when `Payment does Authorized`
(`result_conforms`, registry.rs:2042). A call through the role returns a
restricted `^Authorized`. This is the mechanism that makes role-oriented
typestate possible.

**Object-level `is ^Role` is broken.** The checker accepts `is ^SomeRole`
on an object action, even when the object does not do that role (registry.rs:1843
only tests `Ty::Role`). The IR verifier then rejects it: "a successor action
must return ^ of its object"
([verify.rs:325](../../okkama-rs/crates/okkama-compiler/src/ir/verify.rs)).
The result is an internal compiler error. The limitation is in IR
verification and lowering, not in parsing, name resolution or type checking.
*(fixed: the checker now rejects object-level `^Role`, see §8 B1.)*

**Successor must-use is a static, path-sensitive flow check**
([flow.rs:1-12, 207, 243](../../okkama-rs/crates/okkama-compiler/src/analysis/flow.rs)).
Every `^T` value creates an obligation. Aliases share it. Using the value as
a receiver, an argument, a return value, a rebind, a field, an aggregate
element or a `throw` discharges it. A discarded statement, a binding that
leaves scope or an unconsumed path is an error. The obligation survives bound
action values (`negative/g_bound_successor`). It is "use at least once"
(relevant typing), not "use at most once": any use discharges it, and nothing
stops further use.
(`docs/okkama-llm-context-1.0.md` still describes a discarded successor as a
runtime error; the Rust front end rejects it at compile time.)

**Rebinding** (`x = expr;`) is allowed only for locals of a concrete class
that has successor actions (`rebindable`,
[check/mod.rs:414](../../okkama-rs/crates/okkama-compiler/src/analysis/check/mod.rs)).
The value must keep the variable's type, so a variable can never change from
one state view to another (`negative/c_rebind_across_states`).

**Constructors are public, but the field allow-list is enforced.**
`Payment(:key)` cannot receive the defaulted `stage` field
(`advanced/b_forge_state_ctor`). There are no private constructors and no
module-private declarations. `take` imports hide unlisted names, but a plain
import of the same module exposes everything (`advanced/b_take_hides_class`,
`advanced/b_forge_state_class`). Private successor actions are explicitly
unsupported (registry.rs:1858, verify.rs:328).

**Payload ADTs.** Enums with payloads or type parameters are "variant" enums
(registry.rs:695). Their matches (statement, expression and compact
`?? / !!`) must be exhaustive or have `else`
([variants.rs:122-260](../../okkama-rs/crates/okkama-compiler/src/analysis/check/variants.rs)).
A pattern binding gets its payload's declared type, including role types and
substituted generic arguments. Payload-free, non-generic enums keep the
legacy rules: a statement `match` on them is not checked for exhaustiveness.

**Maybe/Presence** are ordinary nominal generic variant enums. Fallback
`else` works only for `Maybe`. `Presence` must be folded explicitly
(variants.rs:486-492). Compact choices reject branches with different types
([expr.rs:673](../../okkama-rs/crates/okkama-compiler/src/analysis/check/expr.rs)),
so they never widen two state views into a common type.

**Generics.** `T` inferred from a role reference is the role
(`advanced/b_generic_box`). A bounded generic `object Audit<T: Identified>`
type-checks with a role argument, but lowering a bound call on a role-typed
`T` fails ("builtin role adapters … not implemented",
[lowering/body/mod.rs:2608](../../okkama-rs/crates/okkama-compiler/src/lowering/body/mod.rs)).
*(fixed: the value now narrows to the bound with `RoleNarrow`.)*

## 3. Experimental results

Experiment A: [results.md](../../experiments/role-typestate/results.md#experiment-a--minimal-transitions).
All 17 required invalid transitions are rejected by `okkama check`, in the
three static patterns (A, B1, B2/D). Every negative program uses correct
syntax, and the diagnostic names the missing action, for example
`Role "Created" has no action "capture"`. Valid sequences run on the IR
interpreter, JVM and Sparrow.

Experiment B
([results](../../experiments/role-typestate/results.md#experiment-b--interface-restriction-vs-state-restriction)):
role restriction holds through aliases, parameters, generic containers and
`any`. It does not survive access through the concrete class. In B2/D a
fresh concrete `Payment` can be assigned to *any* role it implements: a
role is a claim about the state, not evidence of it
(`advanced/b_concrete_to_wrong_role`). The private stage field then rejects
the call at runtime. One true escape exists: a role that exposes `text()`
can be narrowed to `Error`, thrown, and caught as the concrete class,
which gives back every hidden action (`advanced/b_throw_catch_recover`).

Experiment C
([results](../../experiments/role-typestate/results.md#experiment-c--successor-semantics)):
a successor cannot change class. Role-level `^NextRole` transitions work.
Object-level `^Role` was blocked by the defect described in §2 *(fixed)*. Must-use is
static and path-sensitive. Old states remain usable, and one state value can
produce several successors (`advanced/c_reuse_old_state`). Transitions
written as ordinary actions (Patterns A/B1) can be discarded silently.

Experiment D
([results](../../experiments/role-typestate/results.md#experiment-d--adt-outcomes)):
an outcome ADT with role-typed payloads gives each branch a different state
type. Invalid operations are rejected per branch, and missing variants are
rejected by name. Generic (`Outcome<Authorized>`) and nested ADTs work. The
outcome value itself has no must-use obligation.

Experiment E
([results](../../experiments/role-typestate/results.md#experiment-e--maybe-presence-)):
`Maybe<Role>` and `Presence<Role>` keep the role through `else`, compact
choices and `fold()`. They are useful at the boundary (an optional token, a
missing record), not as a way to model states. Nothing widens or loses
state information silently.

Experiment F: four patterns compared in §4 and in the table below.

Experiment G
([results](../../experiments/role-typestate/results.md#experiment-g--adversarial-pattern-d-unless-noted)):
the adversarial matrix is summarized in §5–§7.

Defects found (classified BLOCKED, not language limits): object-level
`^Role` successors cause an internal compiler error, and the checker
accepts `^Role` for a role the object does not do; a tail value in a
successor body is silently ignored; a bounded generic over a role type
fails in lowering. *(All fixed, see the update note at the top.)* Minor issues: `okkama fmt` reflows multi-line payload
enums badly; the rebind diagnostic says a role "has no successor actions"
even when the role declares successors to other roles.

### Pattern comparison

| | A: class per state | B1: role + class per state | B2: one class, role views | C: ADT state | D: hybrid (B2 + ADT stage + ADT outcomes) |
|---|---|---|---|---|---|
| Invalid call rejected statically | yes | yes | yes (via roles) | no (runtime) | yes (via roles) |
| Transition result must be used | no | no | yes | no | yes for in-class transitions; no for outcome ADT |
| Forging a later state | possible (construct class) | possible (construct class) | prevented (ctor allow-list) | possible (construct variant) | prevented, except public internal successors |
| Runtime state check possible | no history to check | no history to check | yes | yes | yes, with per-state data |
| Bypass via concrete class | n/a (each class is one state) | construct any state | static bypass, runtime caught | n/a | static bypass, runtime caught |
| Old-state reuse prevented | no | no | no | no | no |
| Per-state data | natural (fields) | natural | awkward (shared fields) | natural (payloads) | natural (stage payloads) |
| Alternative outcomes | throw or extra wrapper | throw or extra wrapper | throw | in the ADT | outcome ADT with views |
| Boilerplate | low | medium (role + class per state) | low–medium | low | medium–high |
| Generics | fine | fine | fine (bounded-over-role fixed) | fine | fine (bounded-over-role fixed) |
| Fits current compiler | fully | fully | fully | fully | fully |
| Runtime overhead | not measured | not measured (role wrap) | not measured (role wrap + one field) | not measured | not measured |

## 4. Recommended idiomatic pattern

Pattern D. The program below is
[positive/recommended_payment.ok](../../experiments/role-typestate/positive/recommended_payment.ok).
It runs on IR, JVM and Sparrow and prints
`p1: refunded p1 (auth-p1)`, `p2: declined: insufficient funds`,
`p3: failed: gateway timeout`.

```okkama
module app;
import "io/stdout" take Stdout;

object InvalidTransition does Error {
    has action is string;
    InvalidTransition(:action);
    text() is string { "invalid transition: " + action; }
}

object PaymentError does Error {
    has reason is string;
    PaymentError(:reason);
    text() is string { reason; }
}

// ---- boundary: what the external gateway says --------------------------

enum GatewayReply = (approved(code is string), declined(reason is string), unavailable(detail is string));

role Gateway {
    authorize(id is string, amount is int) is GatewayReply;
}

// ---- state views ---------------------------------------------------------

role Created {
    id() is string;
    authorize(gateway is Gateway) is Authorization;
}
role Authorized {
    id() is string;
    code() is string;
    capture() is ^Captured;
}
role Captured {
    id() is string;
    refund() is ^Refunded;
}
role Refunded {
    id() is string;
}
role Declined {
    id() is string;
    reason() is string;
}

// Alternative outcomes of authorize(): each payload is a different view.
enum Authorization = (
    approved(payment is Authorized),
    declined(payment is Declined),
    failed(error is PaymentError)
);

// ---- the single implementation ------------------------------------------

enum Stage = (
    created,
    authorized(code is string),
    captured(code is string),
    refunded(code is string),
    declined(reason is string)
);

object Payment does Created, Authorized, Captured, Refunded, Declined {
    has key is string;
    has amount is int;
    has stage is Stage = Stage.created;   // not in the constructor allow-list

    Payment(:key, :amount) {
        confirm self.amount > 0 else throw PaymentError(reason: "amount must be positive");
    }

    id() is string { key; }

    authorize(gateway is Gateway) is Authorization {
        confirm stage == Stage.created else throw InvalidTransition(action: "authorize");
        match gateway.authorize(key, amount) {
            case GatewayReply.approved(code) { Authorization.approved(self.approve(code)); }
            case GatewayReply.declined(reason) { Authorization.declined(self.decline(reason)); }
            case GatewayReply.unavailable(detail) { Authorization.failed(PaymentError(reason: detail)); }
        }
    }

    code() is string {
        match stage {
            case Stage.authorized(code) { code; }
            else { throw InvalidTransition(action: "code"); }
        }
    }

    reason() is string {
        match stage {
            case Stage.declined(reason) { reason; }
            else { throw InvalidTransition(action: "reason"); }
        }
    }

    capture() is ^Payment {
        let code = match stage {
            case Stage.authorized(code) { code; }
            else { throw InvalidTransition(action: "capture"); }
        };
        ^stage = Stage.captured(code);
    }

    refund() is ^Payment {
        let code = match stage {
            case Stage.captured(code) { code; }
            else { throw InvalidTransition(action: "refund"); }
        };
        ^stage = Stage.refunded(code);
    }

    // Internal transitions: private successors, callable only through `self`.
    private:
    approve(code is string) is ^Payment {
        confirm stage == Stage.created else throw InvalidTransition(action: "approve");
        ^stage = Stage.authorized(code);
    }
    decline(reason is string) is ^Payment {
        confirm stage == Stage.created else throw InvalidTransition(action: "decline");
        ^stage = Stage.declined(reason);
    }
}

// The only intended entry point: callers start from the Created view.
object Payments {
    Payments();
    open(id is string, amount is int) is Created { Payment(key: id, amount: amount); }
}

// ---- a client that depends only on views -------------------------------

object Checkout {
    Checkout();
    run(id is string, gateway is Gateway) is string {
        let outcome = Payments().open(id, 1000).authorize(gateway);
        let text = match outcome {
            case Authorization.approved(payment) {
                let code = payment.code();
                let refunded = payment.capture().refund();
                "refunded " + refunded.id() + " (" + code + ")";
            }
            case Authorization.declined(payment) { "declined: " + payment.reason(); }
            case Authorization.failed(error) { "failed: " + error.text(); }
        };
        id + ": " + text;
    }
}

object ApprovingGateway does Gateway {
    ApprovingGateway();
    authorize(id is string, amount is int) is GatewayReply { GatewayReply.approved("auth-" + id); }
}
object DecliningGateway does Gateway {
    DecliningGateway();
    authorize(id is string, amount is int) is GatewayReply { GatewayReply.declined("insufficient funds"); }
}
object DownGateway does Gateway {
    DownGateway();
    authorize(id is string, amount is int) is GatewayReply { GatewayReply.unavailable("gateway timeout"); }
}

main() is int {
    let out = Stdout();
    out.writeln(Checkout().run("p1", ApprovingGateway()));
    out.writeln(Checkout().run("p2", DecliningGateway()));
    out.writeln(Checkout().run("p3", DownGateway()));
    0;
}
```

Rules that make the pattern work, each tied to an experiment:

1. **Clients depend only on roles** and obtain the first view from a factory
   (`Payments.open() is Created`). In a library module, consumers should
   `take` the factory, the roles and the outcome ADT, but not the class.
   This is a convention (`advanced/b_take_hides_class`).
2. **Declare transitions on the roles as `^NextRole`** and implement them as
   `^Self`. That gives static must-use and a restricted next view.
3. **Keep the real state in a private payload ADT field that is not in the
   constructor allow-list**, and check it with `confirm` / `match … else
   throw` in every transition and state-specific query. This catches the
   concrete-class bypass at runtime and makes later states unforgeable by
   construction (`advanced/g_concrete_skip`, `advanced/g_wrong_view`,
   `advanced/b_forge_state_ctor`).
4. **Use a payload enum (not a payload-free one) for the stage**, so that
   exhaustiveness applies (`advanced/d_plain_enum_nonexhaustive`).
5. **Model alternative outcomes as an ADT with role-typed payloads**, and
   match it exhaustively. Avoid `else` in such matches, so that adding an
   outcome breaks callers at compile time.
6. **Do not let state classes `does Error`**, and do not give state roles a
   `text()` action (`advanced/b_throw_catch_recover`).

## 5. Compile-time guarantees

`okkama check` rejects these programs (with the diagnostics listed in
results.md):

- calling an operation the current state view does not declare, through
  locals, aliases, parameters, fields, generic containers, ADT payloads,
  `Maybe`/`Presence` extraction and compact-choice branches;
- converting a state view to a role it cannot fully expose, to the concrete
  class, or back out of `any`;
- calling anything on an `any` box;
- discarding a successor transition result, leaving it unused on some path,
  or discarding the result of a bound successor action;
- rebinding a state-view variable to another state;
- a successor that tries to produce another class or a replacement value;
- a role transition implemented with the wrong result (`got Authorized` /
  `got ^Fresh`);
- an action that returns the previous state where it promised the next one;
- packing the wrong state view into an outcome variant, or matching with
  the wrong number of bindings;
- a non-exhaustive match (statement, expression or compact) over a payload or
  generic enum, including `Maybe` and `Presence`;
- a `Presence` fallback without an explicit `fold()`;
- a compact choice whose branches produce different state views;
- supplying the private state field to the constructor;
- a bounded generic body calling an action outside its bound.

## 6. Remaining runtime responsibilities

- **The real state.** A role is a statically checked claim. For callers that
  hold the concrete class it can be false (`advanced/b_concrete_to_wrong_role`),
  so each transition must check the private stage.
- **Duplicate transitions.** Old immutable states stay valid, so
  `created.authorize()` twice is two authorizations
  (`advanced/g_double_authorize`). Inside one body this could be linted. In
  a real system the same payment is loaded twice by two requests or two
  workers, and no in-process type rule can see that. Use a durable state row
  with a conditional update (compare-and-set on the stage), idempotency keys
  at the gateway, and unique constraints.
- **Identity.** Types track states, not entities: the capture of payment `a`
  can be paired with the refund of payment `b` (`advanced/g_mixed_entities`).
- **External outcomes.** Whether a gateway failure allows a retry, a void or
  a manual review is a domain decision. The ADT forces the caller to handle
  `failed`, not to handle it correctly (`advanced/g_retry_after_failure`).
- **Data entering from outside** (JSON, SQL, queues) must be turned into a
  state through validating constructors or factories. `Presence`/`Maybe`
  describe missing fields; they do not say whether the result is a valid
  state.

## 7. Limitations and escape hatches

| Escape | Effect | Mitigation today |
|---|---|---|
| Name the concrete class (plain import) | every state action visible; any view assignable | *(fixed across modules: `hidden object`, `negative/h_sealed_forge`)*; inside the module, the runtime stage check |
| Public internal successors (`approve`) | authorize without a gateway | *(fixed: private successors, `negative/g_internal_successor_leak`)* |
| Class-per-state / ADT-variant construction (A, B1, C) | forge any state | prefer B2/D |
| throw/catch through `Error` | recovers the concrete object | state classes must not `does Error` |
| Reuse of an old state value | forked history, double transitions | durable state and idempotency |
| Must-use laundering | any use discharges it; through a generic `T` it disappears (`advanced/g_generic_launder`); outcome ADTs and ordinary actions have none | review discipline |
| Payload-free enum matches | silently non-exhaustive statements | use payload enums for state |

## 8. Possible language extensions

Ranked by benefit relative to complexity and architectural risk. Each item
names the experiment that shows the gap.

### Category A — no language change

Everything in §4. The pattern works today. Write it up as a documented
idiom, and keep `experiments/role-typestate/run.sh` as a regression check
for its guarantees.

### Category B — small semantic improvements (no new syntax)

1. **Support object-level `^Role` successors** (`negative/c_successor_role_direct`).
   *Status: the immediate defect is fixed by rejecting the form with a clear
   error; supporting it remains an option. Its value is smaller than first
   estimated: while the concrete class is public, callers still reach every
   action on it directly, so `^Role` alone does not close the main bypass.
   It matters mostly together with items 2 and 7.*
   The checker already accepts `capture() is ^Captured` on the class, and the
   intended meaning is clear: the staged copy of `self`, returned as a
   restricted `^Captured`. Lowering would emit `RoleWrap` after building the
   successor, and the IR verifier would allow `^Role` when the owner does
   that role. The checker must also require that the object does the role
   (it did not: `negative/c_successor_role_not_done`). Benefit:
   after its first transition, even a holder of the concrete class gets a
   restricted view. Impact: checker (registry.rs:1843), lowering, IR
   verifier; Sparrow and JVM already execute `RoleWrap`. Fully compatible:
   such programs currently crash the compiler. Complexity for programmers:
   none. **Highest value, lowest risk.**
2. **Allow private successor actions** (`negative/g_internal_successor_leak`).
   *Status: done; `approve`/`decline` in the recommended pattern are private.*
   The `private:` section already exists. The restriction is enforced in the
   checker (registry.rs:1858) and the IR verifier (verify.rs:328). Lifting it
   would hide internal transitions like `approve()`. Impact: checker,
   verifier, private-call dispatch on all backends. Compatible.
3. **Diagnostics and defects:** give a precise error instead of an internal
   compiler error; make bounded generics over role arguments work
   (`positive/f_bounded_generic_role`); report a tail value in a successor
   body (`negative/c_successor_tail_replacement`); reword the role rebind
   message (`negative/c_rebind_across_states`). *Status: all fixed.*
4. **Exhaustiveness for payload-free enums** (`advanced/d_plain_enum_nonexhaustive`):
   start as a `okkama lint` warning, and make it an error later if the
   corpus allows. *Status: the lint warning `non-exhaustive-match` is
   implemented.* Compatibility risk for existing statement matches, which
   is why the payload-enum POC left it out.
5. **Stale-state lint**: warn when a binding is used after a successor was
   derived from it in the same body (`advanced/c_reuse_old_state`).
   `analysis/flow.rs` already tracks per-binding obligations, so this fits
   the existing analysis. It is a lint, not a guarantee: aliases stored in
   fields or collections escape it.
6. **Concrete `catch` of a value thrown through a restricted reference**
   (`advanced/b_throw_catch_recover`): bind the restricted contract, or do
   not match concrete catch clauses for such values. A small semantic
   change to an intentionally kept rule (spec §12.1, "concrete error
   routing"), so it needs a decision rather than a fix.

### Category B/C boundary — access control

7. **Module-private declarations or constructors**
   (`advanced/b_forge_state_class`, `advanced/b_take_hides_class`).
   *Status: done as `hidden object/role/enum` (spec §5.1.1,
   [design](../POC/private-objects.md)); `negative/h_sealed_*` show the
   concrete-class bypass rejected statically across modules.* This is
   the only change that removes the concrete-class bypass instead of
   catching it at runtime. It needs a visibility notation or a module export
   policy (an experimental qualified-export prototype exists:
   `docs/qualified-export-prototype.md`). Impact: parser (if syntax), module
   resolver, checker; backends unchanged. Medium complexity. Benefit: high
   for all encapsulation, not only typestate.

### Category C — fundamental extensions (not recommended now)

8. **Affine consumption of state views** (`advanced/c_reuse_old_state`,
   `advanced/g_double_authorize`). For example: a successor called through a
   role consumes the receiver binding. It would forbid in-process reuse of
   old states. To be sound it needs move semantics across aliases, fields,
   collections, closures, async tasks and generics; that is a new ownership
   discipline in the checker and the runtime, and changes how immutable
   values are reasoned about. It would not prevent the duplicates that
   matter in payment systems (two processes loading the same row). **The
   benefit does not justify the complexity.**
9. Protocol/session types or state-indexed generics: not justified. Roles
   with covariant successors already express the state graph; what is
   missing is consumption, not expressiveness.

## 9. Recommendation

**Existing Okkama is sufficient, with minor semantic improvements.**

The experiments show that state-specific interfaces, typed transitions,
mandatory use of transition results and exhaustive outcome handling all work
with current syntax and the current compiler, on all three backends. The
gaps that matter in practice are not about the type system's power. They
are an IR defect (object-level `^Role`), two visibility gaps (private
successors, module-private classes) that let callers bypass the views, and
missing linearity. The first three are small and compatible. Linearity
would be expensive, and the part of duplicate-transition safety that
matters (across processes) is outside what a type system can guarantee.

## 10. Language design implications

Role-oriented typestate deserves to be documented as an official pattern
("state views"). It uses only features that are already central to the
language: nominal roles, immutable objects, successors, and payload ADTs.
It also benefits from one property that interface-based typestate in
mainstream object-oriented languages usually lacks: there is no cast back
from a role to the class. The `throw`/`catch` route is the one exception
found. (The restriction is static: only Sparrow keeps a runtime wrapper.)

The documentation must state its limits with the same precision as this
report: the views restrict interfaces, successors guarantee that results are
used, ADTs guarantee that outcomes are handled; the true state is still
checked at runtime, and nothing prevents the reuse of an old state. Whether
it becomes a distinguishing concept depends mostly on Category B items 1, 2
and 7. With them, holding the concrete class would no longer bypass the
views, and the pattern would be enforced by the compiler rather than by
convention.
