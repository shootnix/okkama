# Okkama — LLM / AI Agent Context Pack (v1.0)

**Purpose.** This is the single self-contained context for AI agents and LLMs
that generate, review, refactor, or explain Okkama code. Load this file at the
start of a session instead of re-reading the full documentation.

**Edition.** 19 July 2026, Okkama 1.0 implementation line.

**Sources of truth, in priority order:**

1. the test suite (`okkama-pl/t/`) and the reference interpreter (`okkama-pl/`);
2. the specification (`docs/okkama-spec-11072026.md`) and grammar
   (`docs/okkama-grammar-1.0.md`);
3. the textbook (`docs/OkkamaBook.md`) — narrative and rationale.

When this pack and running tests disagree, the tests win.

**Toolchain:**

```sh
cd okkama-pl                          # stdlib resolves as ../stdlib relative to cwd
perl bin/okkama path/to/program.ok    # run a program (args after the file)
perl bin/okkama toaster/pets/*.ok     # combine matching files into one module
perl bin/okkama -t path/to/module.ok  # run tests (exit 0 = all green)
```

Source files use the `.ok` extension. Requires Perl ≥ 5.42 (native `class`
feature). Run from the `okkama-pl/` directory when the program imports stdlib
modules.

Multiple existing `.ok` paths before ordinary arguments share declarations
as one module. Imports remain file-local. Module names and exact versions must
match, and the combined module may contain at most one `main`.

---

## 1. Language identity — rules that define Okkama

Okkama is a statically typed object language that turns the *Elegant Objects*
discipline into enforced runtime rules. Internalize these before writing any
code:

1. **No null** in any spelling (`null`, `none`, `nil`, optionals, `?.` — all
   parse errors). Absence is modeled with `else` fallbacks, typed throws, or
   objects.
2. **No inheritance.** No `extends`, `isa`, `super`, `abstract`, `protected`.
   Behavior is shared via roles (contracts) and composition (fields).
3. **Objects are immutable after construction.** The only state declaration is
   `has`; fields bind during construction and never change.
4. **State change = successor.** An action that "changes" state is declared
   `is ^Type`, stages updates with `^field = expr;`, and returns a **new**
   object. Discarding a successor result is a runtime error.
5. **Fields are private through ordinary references.** There are no setters.
   External field access is a runtime error. Objects expose *behavior*.
6. **No truthiness.** Every condition (`if`, `confirm`, `!`, `&&`, `||`,
   postfix `if`) requires a real `bool`. `if 1 { }` is an error.
7. **No implicit conversions.** `int`, `float`, `rat`, `byte` are distinct categories.
   `1 == 1.0` is a runtime error, not `false`. `"n = " + 1` is an error.
8. **Optional explicit return.** `return expression;` exits the current action
   with one value, also from nested loops/blocks. Bare `return;` is for void
   and successor actions. Implicit final-expression results remain supported.
9. **Fail fast.** `confirm` asserts invariants in constructors and actions.
   Use `else throw` for typed errors; without `else`, a failed check panics.
10. **Strict lexical scope.** A callee never sees the caller's locals. Data
    crosses boundaries only through arguments, construction fields, and
    receiver fields.
11. **Contextual names stay narrow.** Keywords are not ordinary identifiers,
    but an unambiguous lowercase module path segment or action label may have
    keyword spelling: `module test/more`, action `is(...)`, and
    `assertions.is(...)`. In object bodies `has(...)` is an action while
    `has field is T;` is state. Removed/deferred reserved words remain
    forbidden.
12. **No copy-setter APIs.** Action names `with` and `with_*` are parse
    errors. Successors name domain transitions/results; complete state enters
    through constructors, value composition, or decorators. Do not disguise
    the same fluent builder as `without`, `in`, `from`, `where`, `limit`, an
    offset setter, or a boolean toggle.

## 2. Never generate

These do not exist in Okkama 1.0. Producing them is always a bug:

```text
null  none  nil  optional  T?  ?.          — no null, no optionals
isa  super  abstract  protected  extends   — no inheritance
with / with_* actions, setters, mutable    — removed/forbidden API forms
while / do-while                           — use `loop` with conditional `last`
`fn` / `function` / `lambda`, mutable captures — use anonymous actions: `(value is T) is R { ... }`; public object actions are also first-class bound values (`controller.response`) and may retain async type `async (Request) is Response`
string interpolation ("${x}", f"")         — concatenate strings explicitly
{ "key": value }                           — map literals use BRACKETS: ["key": value]
x.field = value                            — no assignment outside constructors
obj.field (from outside)                   — fields are private; call an action
`finally`, catch-all catch, catch guards  — try needs typed catches only
`panic` with non-string / `panic;`        — throw an Error / use `throw;`
catch scalar, collection, enum, `any`     — catch types must guarantee Error
regex literals (/.../, qr//)               — use stdlib: Regex("pattern")
byte literals (0x00)                       — construct with byte(0)
** power operator, string repetition *     — not in the language
Type:(args) / Type:{ ... }                 — removed; use a declared prefix such as `value Type(...)`
&action() / &Type(args)                    — removed anonymous projection syntax; use `#projected`
```

Also never generate: an action that stages `^field` without `is ^Type`; a call
mixing positional and named arguments; a `let` without an initializer;
top-level executable statements (module scope holds declarations only).

## 3. Program shape: modules, `main`, imports

Every file starts with a module declaration; executable modules declare
`main()`:

```okkama
module app v1.0.0;

import "money" take Cash;

main() is int {
    0;    // process exit status: success
}
```

Rules:

- `module` must be the first thing in the file. Module names are lowercase
  identifiers, optionally `/`-separated (`app`, `money/cash`). Version is
  exactly `vMAJOR.MINOR.PATCH`.
- Module scope allows only declarations: objects, roles, enums, native
  declarations, actions, test blocks. Library modules may omit `main`.

Only `main() is int` and `async main() is int` are entry points; neither
accepts parameters. Return an actual int in 0..255, explicitly or implicitly.
Zero is success, 1..255 are application-defined failures. Other types, absent
results and out-of-range values fail; no wrapping or automatic zero applies.
CLI returns this status without printing it, after defer/destroy and shutdown.
Cleanup failures still fail. `-t` runs tests instead of main. Ordinary object
methods named main can have other result types.

### Imports

```okkama
import "money";                  // plain import
import "money" take Cash;        // selective import
import "sql/pool" as db_pool;    // explicit namespace
```

- Paths are *source paths*: `import "foo/bar";` resolves `foo/bar.ok`.
  Lookup: explicit import dirs → the importing file's own directory (never
  the working directory) → bundled stdlib. Local files shadow stdlib.
- **Plain import** exposes declarations owned directly by the imported module
  only via a basename namespace: `import "io/stdout";` permits
  `stdout.Stdout()`; the namespace
  also resolves type annotations (`boxes.Box<int>`). Internal nominal identity
  is module- and ABI-major-qualified (`boxes#1::Box<int>`), while normal
  diagnostics display the source spelling. The imported module's own imports
  are not re-exported.
- **`take` import** resolves listed names only against directly owned
  declarations, exposes exactly those bare names, and creates no namespace.
  The transitive closure still runs internally, but never creates consumer
  bindings; declaration bodies and role fakes use defining-module scope rather
  than caller scope.
- **`as` import** replaces the basename namespace with the requested name.
- No wildcard imports. Duplicate bare `take` names error; same-named
  namespace-qualified or hidden declarations from different modules coexist.
  Re-imports are idempotent. Imported `main()` never runs. Cyclic imports are
  rejected.

## 4. Types

Type spellings usable in source:

```okkama
int          // integer
float        // floating point (distinct from int, never unified)
byte         // checked octet 0..255, distinct category (see §11)
string       // immutable Unicode text
char         // single character, distinct from string
bool         // true / false only
any          // explicit opaque top type (see §11)
@T           // array of T           (@int, @byte, @@int)
%K:V         // map from K to V      (%string:int)
Cash         // concrete object type (nominal)
Package      // role type (nominal contract)
Box<int>     // applied generic type
^Cash        // successor type: "returns a NEW Cash"
```

Types appear at *typed boundaries*: `let x is T = ...`, `has f is T`,
parameters, return types, `catch` clauses. Each boundary validates the runtime
value, recursively for collections. The semantic analyzer checks declaration-
derived facts before execution; value-dependent checks remain runtime errors.

- No subtyping between objects; a concrete type satisfies a role only via an
  explicit `does Role`.
- Untyped `let x = expr;` retains the exact runtime type. Nonempty homogeneous
  collection literals infer `@T`/`%K:V` recursively from identical exact types.
  Empty and heterogeneous literals stay untagged without a declared target;
  there is no numeric widening, invented common role, union or implicit `any`.

Map scalar keys include string/char/bool/byte/int/float and nominal enum cases.
Their type and value both identify the key, before host normalization; namespace
`::` is separate from the `%K:V` separator. Objects, handles, actions and
opaque Any values do not become map keys. Action-declared `@U` results supply
their own `.map()` target for tail/return/branch/await paths; caller types do not
widen the callee's contract.

## 5. Lexical rules and literals

- Local bindings (`let`, `for`, `catch`) accept `[A-Za-z_][A-Za-z0-9_]*`,
  including `USER_NAME`, `A`, and `UserName`; names remain case-sensitive.
- Lowercase identifiers `[a-z_][a-zA-Z0-9_]*` name fields,
  parameters, actions, module segments.
- Type names `[A-Z][a-zA-Z0-9_]*` name objects, roles, enums, generic
  parameters.
- Comments: `// line` and `/* block (non-nesting) */`.

Literals:

````okkama
10        // int
3.14      // float
"hello"   // string  — escapes: \n \r \t \" \\
```raw "text" \n remains raw``` // string — contents are exact
'x'       // char    — escapes: \n \r \t \' \\ ; exactly one char after decoding
true      // bool
false     // bool
````

Unknown escapes and a trailing lone backslash are lexer errors. There is no
byte literal (`byte(255)`), no regex literal (`Regex("[a-z]+")`), and decimal
literals have no runtime semantics yet (model money as `int` minor units).
Three backticks delimit an ordinary `string` whose contents are retained
exactly: no escapes, interpolation, dedenting, trimming or modifiers.
Multiline content is legal; the first next exact triple-backtick closes it.
One/two isolated backticks and an unterminated fence are lexer errors.

## 6. Operators and expressions

Precedence, loosest first:

```text
||                      boolean or   (bool operands only, no coercion)
&&                      boolean and  (bool operands only)
<-                      stream write (formerly <<)
==  !=                  strict same-category equality
..  ..=                 range construction
<  <=  >  >=            numeric ordering (mixed numeric ok, promotion applies)
|                       bitwise OR
^                     bitwise XOR
&                       bitwise AND
<< >> >>>               fixed-width shifts
+  -                    add/concat, subtract
*  /  %                 multiply, divide, modulo (division/modulo by zero = error)
!  -  ~                 unary not, unary minus, bitwise inversion
postfix                 calls, member access, indexing, unpack `...`
```

- Bitwise `& | ^` require int/int or byte/byte and preserve that category.
  `~` preserves its operand category. int words are signed 64-bit, byte words
  unsigned 8-bit; int operands outside signed-64 are rejected. Shift counts
  must be int in 0..W-1: `<<` discards outgoing bits, `>>` sign-extends int,
  `>>>` zero-fills. No string/bool/float/rat coercion. Both sides evaluate once,
  left-to-right, including async. Bitwise binds more tightly than comparisons.
  Based literals 0b/0B/0x/0X accept internal underscores and values 0..INT64_MAX.
  int/byte methods: bit_count, leading_zeros, trailing_zeros return int;
  rotate_left(count), rotate_right(count), reverse_bits, swap_bytes preserve type.
  Zero has W leading/trailing zeros. Rotation counts use the same strict range.
  Stream writes migrated from `<<` to `<-`; `a<-b` is a write, `a < -b` a comparison.
  `xor` is an ordinary identifier; infix `^` is XOR, prefix `^` retains successor syntax; no compound assignments.
- `+`, `-`, `*`, and `%` produce `rat` when a `rat` participates, otherwise
  `int` unless a `float` operand forces `float`; `/` produces `float` when a
  `float` participates and exact `rat` otherwise. `rat` and `float` require
  an explicit conversion before mixing. `string + string` concatenates;
  anything mixed is an error.
- Explicit zero-argument casts are `int.rat()`, `byte.rat()`, `float.rat()`
  (canonical-decimal rationalization), `int.float()`, `float.int()` (truncate
  toward zero), checked `int.byte()`/`float.byte()`, `byte.int()`/`byte.float()`,
  `rat.int()`/`rat.byte()`/`rat.float()`, numeric `.string()`, and `bool.string()`
  (`"1"`/`"0"`). `rat.numerator()`/`rat.denominator()` expose exact text.
- **Equality is strict per category:** `1 == 1.0`, `byte(1) == 1`,
  `'a' == "a"`, `true == 1` are all runtime **errors**, never `false`.
- Composite equality is deep: arrays by order, maps by key set + values,
  ranges by `start`/`end`/`inclusive`, objects by concrete type + all fields
  (including private).

Calls:

```okkama
cash.add_bonus(10);          // positional
cash.add_bonus(bonus: 10);   // named
// never mix positional and named in one call
```

Receiver and arguments evaluate exactly once, left to right, in caller scope.

Anonymous actions are first-class expressions and use the ordinary call
syntax:

```okkama
let double is (int) is int = (value is int) is int { value * 2; };
double(10);                                      // 20

let compose is ((int) is int, (int) is int) is (int) is int =
    (first is (int) is int, second is (int) is int) is (int) is int { (value is int) is int { second(first(value)); }; };
```

Anonymous actions use named-action syntax with the name omitted. Parameter
types and `is Result` (including `is void`) are mandatory. Their declared
`(T, ...) is R` signature is checked at typed boundaries. Calls
are positional and exact-arity. Action values capture lexical values and may
be passed, returned, nested, and called after their defining block exits.

Public named object actions are also first-class bound values:

```okkama
let endpoint is async (Request) is Response = controller.response;
let response = await endpoint(request);
```

The missing call parentheses are significant: `controller.response` captures
the controller/DI graph; `controller.response(request)` invokes immediately.
`async` is part of the exact action type. Private members cannot escape, and
the retained receiver participates in ownership reachability.

Anonymous values are ordinary arguments, also in named member calls:

```okkama
worker.apply(operation: (value is int) is int { value * 2; });
```

Old anonymous arrow declarations and trailing call sugar are rejected.
`values.map() -> value { ... }` is a dedicated collection expression;
`for values -> value { ... }` remains a loop binding. Anonymous parameters
cannot be variadic or end with a trailing comma. `void` is result-only:
`() is void` is valid, `(void) is int` is invalid. Anonymous async actions
remain unsupported; async bound callable types may declare a void result.

Indexing with explicit fallback (the no-null idiom):

```okkama
xs[2];                            // out of bounds = runtime error
prices["missing"] else 0;         // fallback evaluated only when missing
```

`if` and `match` work as expressions when complete:

```okkama
let v = if ready { 1; } else { 0; };
```

## 7. Statements and control flow

```okkama
let x = 10;             // inferred; initializer mandatory
let y is int = 10;      // typed boundary, validated

let counter = Counter(value: 0);            // Counter.increment() is ^Counter
for [1, 2, 3] -> n { counter = counter.increment(); }   // rebind, no `let`

{ let local = 1; }      // bare block: nested scope, bindings don't leak

if a { 1; } else if b { 2; } else { 3; }    // condition must be bool

match status {
    case Status.ready { 1; }
    case Status.failed, Status.done { 2; }  // multi-value case
    else { 0; }
}

for users -> user { user.name(); }          // arrays, ranges, Iterable<T>...
for 1..=3 -> n { n; }

let value = loop {                          // unbounded, value-producing
    next if should_retry;
    last 100;
};

next;                   // skip iteration       last;       // exit, no value
last value;             // exit with value      last if done;
next if should_skip;    // postfix if works on control/expressions
counter.bump() if enabled;
```

- Re-declaring a name in the same scope errors; shadowing in a nested scope is
  fine.
- `name = expr;` (no `let`) rebinds an existing local — legal only when
  `name` already exists (else `Undefined variable "name"`) and its type is a
  concrete object declaring at least one actual `^`-marked successor action
  returning the type itself or a role it implements (else
  `Cannot rebind variable "name": type "Type" has no successor actions`). A
  *plain* (non-`^`) same-type-returning action does NOT qualify — this is a
  deliberately hard rule, not inferred from return-type shape, even though it
  rejects a persistent-structure method such as `List<T>.prepended(...)`
  (plain return, since `EmptyList<T>.prepended` must return a different
  concrete `LinkedList<T>`, which `^` cannot express). A plain
  scalar/array/map/enum, or a role-typed declared local, never qualifies
  either way. Writes land in the scope that owns the binding, not a nested
  caller scope — this is what lets the `counter` example above accumulate
  across `for` iterations, each of which runs in its own fresh child scope.
  The previous value is not eagerly destroyed; `destroy` (if any) still runs
  at that value's own natural scope exit. Inside a constructor, a field name
  still wins over a same-named local. See
  `docs/todo/local-rebinding-22072026.md`.
- `match` compares case expressions with strict equality; matching on an
  opaque `any` is rejected; exhaustiveness is not checked yet.
- `for` always needs a source and `->` binding. There is **no `while`**:
  use `loop { ... }` with `last if condition;` for unbounded/conditional work.
- A `loop` body tail is ignored. Only `last value` supplies its result. Bare
  `last` is legal when that no-value result is discarded, but not in `let`,
  arguments, typed returns, operators, conditions, or collections.
- Conditional `last value if condition` checks the condition before lazily
  evaluating the value. `next`/`last` target the nearest loop and cannot be
  used directly in a functional `map`/`filter`/`reduce` block.

## 8. Collections: arrays, maps, ranges

```okkama
let xs = [1, 2, 3];                              // array (trailing comma ok)
let typed is @int = [1, 2, 3];
let empty = [];                                  // [] is ALWAYS an array

let prices is %string:int = ["apple": 120];      // map: BRACKETS, expression keys
let empty_map = [:];                             // [:] is the empty map

let r1 = 1..5;    // exclusive: 1,2,3,4
let r2 = 1..=5;   // inclusive: 1,2,3,4,5
```

Critical map rule: **keys are expressions**. `[name: "Alex"]` looks up the
*variable* `name`. For a string key write `["name": "Alex"]`.

- Arrays and maps are immutable values. There is no push/append/set; build new
  collections (or use successors on wrapping objects).
- Missing key / out-of-bounds index = runtime error unless `else fallback`.
- Map keys must be scalars; keys cannot be `any` or `byte`. Values can be any
  type including `any`.
- Ranges: integer-like bounds (`int` or `byte`, normalized to `int`),
  `start <= end`, no chaining. Ranges are first-class: `for` sources, expand
  in array literals (`[0, 2..=4, 9]` → `[0,2,3,4,9]`), accept map/filter/reduce.
- Typed identity: once a collection crosses a `@T`/`%K:V` boundary it is
  tagged, and even empty `slice()`/`keys()`/`values()` results remember their
  element type. Untagged empty literals take the tag of their first boundary.

### User-defined iteration

Repeatable immutable collections opt into `for` nominally through
`collections/iteration`:

```okkama
import "collections/iteration" take Cursor, Iterable;

role Cursor<T> {
    available() is bool;
    current() is T;
    advanced() is Cursor<T>;
}

role Iterable<T> {
    cursor() is Cursor<T>;
}
```

Matching method names alone do not make an object iterable: it must explicitly
`does Iterable<T>`. `for` evaluates the source and `cursor()` once. It checks
`available()` before each item, binds one `current()`, and calls `advanced()`
after normal fallthrough or `next`. `last` exits without advancing. Cursors
are immutable positions; false `available()` is normal exhaustion. The same
nominal protocol enables eager `map`/`filter`/`reduce`; consumable I/O streams
remain a separate future protocol.

Bundled persistent collections start from typed empty objects:

```okkama
import "collections/queue" take Queue, EmptyQueue;
import "collections/stack" take Stack, EmptyStack;

let queue is Queue<int> = EmptyQueue();
let fifo = queue.enqueued(1).enqueued(2);  // iteration: 1, 2

let stack is Stack<int> = EmptyStack();
let lifo = stack.pushed(1).pushed(2);      // iteration: 2, 1
```

## 9. Builtin value behavior

Built-in receivers have a small fixed set of immutable behaviors (language
semantics, not host methods):

```okkama
[1, 2, 3].size();            // 3
[1, 2, 3, 4].slice(1, 3);    // [2, 3]   end-exclusive, hard bounds errors
"héllo".length();            // 5        Unicode code points, not bytes
"hello".slice(0, 2);         // "he"
"a,,b,".split(",");          // ["a", "", "b", ""]
let words is @string = ["red", "green"];
words.join(" / ");           // "red / green"
"banana".replace("na", "NA");       // "baNANA"
"banana".replace_first("na", "NA"); // "baNAna"
prices.size();               // entries count
prices.has("apple");         // true
prices.keys();               // @K sorted by key
prices.values();             // @V paired with keys() order
1.less(2);                   // true  — also: less_or_equal, more, more_or_equal
```

- `slice` never clamps: negative index, `start > end`, `end` past length are
  errors.
- `split`, `replace`, and `replace_first` use literal left-to-right,
  non-overlapping matching, never regex matching. `split` preserves leading,
  adjacent, and trailing empty parts.
- Empty separator means Unicode code points. Empty replacement target means
  code-point boundaries: `"ab".replace("", "-")` is `"-a-b-"`, while
  `replace_first("", "-")` prefixes once.
- `join` requires exact string elements (or an empty value tagged
  `@string`) and returns `""` for an empty array. Removal is expressed as
  `replace(target, "")`; there is no separate `remove` builtin.
- All string transformations are immutable.
- `.less`/`.more` family requires the **exact same** numeric category on both
  sides (`1.less(2.0)` errors); the `<` operators are the broader form.
- Builtin calls are positional-only, exact arity. They activate only for
  non-object receivers — an object's own `size()` action is never shadowed.
- `bool`, `range`, `enum`, `any` receivers have no builtins at all.

## 10. Functional collection operations

Exactly three eager immutable operations on arrays, ranges, typed maps, and
objects nominally implementing the standard `Iterable<T>` role, with
**dedicated block syntax** (not lambdas, not values):

```okkama
let doubled = values.map() -> value { value * 2; };
let positive = values.filter() -> value { value.more(0); };
let total = values.reduce(0) -> accumulator, value { accumulator + value; };

let result = values
    .filter() -> v { v.more(0); }
    .map() -> v { v * 2; }
    .reduce(0) -> sum, v { sum + v; };
```

- Maps bind `key, value` (reduce: `accumulator, key, value`); arrays, ranges,
  and Iterable objects bind one value.
- `map`/`filter` on arrays, ranges, and Iterable objects return typed arrays;
  map `filter` preserves the typed map; map `map` returns an array. Iterable
  filter preserves exact `T`, including when empty.
- `map` fixes one exact output element type — from a direct `@U` target
  (needed for empty sources) or its first result; `@any` output only with an
  explicit `@any` target.
- `filter` predicates must return exact `bool`.
- `reduce` requires exactly one initial value; empty source returns the checked
  initial without running the block; the accumulator keeps one exact type.
- Blocks capture the surrounding lexical scope, cannot use `next`/`last`.
- Iterable traversal calls one cursor in `available`, `current`, body,
  `advanced` order through ordinary action dispatch; a body failure does not
  advance that item. Identity map with a direct `@T` target materializes an
  Iterable in one pass.
- Inside a `for` source, parenthesize a functional expression (top-level `->`
  belongs to the `for`).
- Not available: `grep`/`fold` aliases, lazy streams, parallelism.

## 11. `any` and `byte`

### `any` — opaque transport, not dynamic typing

```okkama
let value is any = 42;
let row is %string:any = ["id": 7, "name": "Ada"];
```

- Values crossing an explicit `any` boundary become **opaque boxes**. You may
  pass, store, return, catch, unpack, and iterate them — but never inspect:
  arithmetic, comparison, boolean logic, member access, calls, indexing,
  `match`, map-key use are all rejected.
- `any` never narrows back implicitly (explicit narrowing syntax is future).
- Untyped heterogeneous literals (`[1, "two"]`) do NOT infer `any`; only
  explicit boundaries box. Map keys cannot be `any`. Roles/fakes must spell
  `any` exactly. Boxes never nest.
- Action values cannot cross an `any` boundary and cannot cross the native
  ABI. Use an exact action type instead.
- Use it only for rows / heterogeneous transport. Prefer concrete types,
  roles, generics.

### `byte` — checked octet 0..255

```okkama
let b is byte = byte(255);
let payload is @byte = @byte(0, 127, b);
```

- `byte(value)` accepts `int`, `byte`, `float`, `rat`: truncate toward zero,
  then check `0..255`. `byte(255.9)` is 255; `byte(-0.5)` is 0;
  `byte(256)` fails. No wrapping, saturation or implicit narrowing.
- Primitive calls `int(x)`, `float(x)`, `byte(x)`, `string(x)` return scalars.
  `@int(...)`, `@float(...)`, `@byte(...)`, `@string(...)` always return typed
  immutable arrays, including zero/one arguments. No plural callable names.
  Numeric targets accept numeric primitives only. `int` checks signed-64 bounds;
  `float` rounds to finite binary64. String accepts numeric primitives, bool
  (`"1"`/`"0"`) and string. No string parsing/encoding, any unboxing, object
  conversion, implicit array/range expansion or named arguments. `@T(...)`
  accepts explicit `name...` from arrays/varargs, multiple times in any position.
  Scalar casts reject unpack. Errors index the flattened argument sequence.
  Validate shape first; evaluate operands once left-to-right, then convert.
  First failed cast reports its 1-based argument; no partial array, no rollback.
  Methods share these rules. Bare calls override local bindings; native
  declarations cannot reuse the four names. `rat(n,d)` stays a separate constructor.
  See the language specification section 6.6.1.
- `byte` and `rat` satisfy the numeric umbrella `number` but remain distinct
  exact categories. Arithmetic accepts byte/int/rat; exact division produces
  `rat`. Float mixing stays explicit, and the result is never `byte`.
- `byte(1) == 1` errors; ordering operators accept mixed numerics.
- Valid array index and range bound; **not** a valid map key.
- `@byte` is the raw binary sequence type (immutable array, not a buffer).

## 12. Failure model: `throw`, `panic`, `try`/`catch`

Two channels, never mixed:

1. **Expected typed failures** — `throw ErrorObject(...)`,
   `confirm cond else throw ErrorObject(...)`, and normalized native failures.
   Only nominal `Error` values are caught by typed `catch`.
2. **Uncatchable failures** — `panic "message"`, bare failed `confirm`, type
   mismatches, undefined variables, out-of-bounds. They unwind cleanup and
   terminate with a diagnostic.

```okkama
confirm condition;                              // uncatchable on failure
confirm condition else throw InvalidValue(10);  // expected typed failure
confirm condition else panic "Broken invariant"; // uncatchable emergency

panic "stop";              // exact string only; never caught

try {
    throw InvalidValue(10);
}
catch error is InvalidValue {
    handle(error);
}
catch error is Error {
    throw;                 // rethrow unchanged
}
```

- `try` is dual-purpose like `if`: it may stand at block level or yield the
  selected try/catch body value in expression position. Both forms need ≥1
  typed catch. No `finally`, catch-all, or guards.
- `Error` is a system nominal role requiring `text() is string`; throwable
  source objects must explicitly `does Error`.
- Catch types are preflight-validated and must guarantee Error: `Error`, a
  concrete/applied Error object, or `NativeFailure`. Arbitrary roles, scalars,
  collections, enums and `any` are rejected.
- First compatible valid clause in source order wins. Panic and programming
  errors bypass catch.
- Try body and catch bodies are separate lexical scopes; try locals invisible
  in catches; the catch binding lives only in its body.
- Try-owned objects are destroyed *before* catch selection (reverse
  construction order); catch-owned objects are destroyed before an
  expression result leaves the form; a body failure outranks cleanup failure.

Idiom: throw dedicated Error objects carrying context, catch concretely or by
`Error`, and use bare `throw;` when you cannot fully handle the failure.


## 13. `defer`

```okkama
defer cleanup();
```

- Lazily registers the expression; operands evaluate at scope exit, not at the
  `defer` statement.
- Runs in **LIFO** order when the current lexical block exits — on normal
  completion, throw/panic/runtime errors, `return`, `next`/`last`, and async
  cancellation — always *before* the scope's owned objects run `destroy`.
- All registered defers get a chance to run. Failure precedence: body failure
  first, then the first failing defer (in LIFO order), then destroy failures.
  An enclosing catch observes the winning failure only after cleanup.
- Deferred results are discarded, but must-use rules still apply: a deferred
  successor call without use is still an error.
- Rule of thumb: `destroy` owns resource cleanup; `defer` is scope-local
  "on the way out" work.

## 14. Objects: fields, constructors

```okkama
object Cash {
    has digit is int;               // required field (no default)
    has cents is int = 0;           // defaulted field
    has currency is string = "USD";

    Cash(digit is int, cents is int) {   // full constructor
        digit = digit;                   // field bind: name matches a field (§7: otherwise a rebind)
        cents = cents;
                                  // constructor invariant
        confirm self.digit >= 0 else throw InvalidCash(digit: self.digit);
    }
    Cash(:digit) {                              // constructor invariant
        confirm self.digit >= 0 else throw InvalidCash(digit: self.digit);
    }                        // field-shorthand constructor



    total() is int { digit * 100 + cents; }
}
```

### Fields (`has`)

- `has` is the ONLY state declaration. Names unique per object. Object headers
  take no parameters.
- Defaults evaluate per construction, before the body, in **declaration
  scope** — they see globals/imports, never caller locals or sibling fields.
- Fields are private through ordinary references: external `cash.digit` is an
  error.
  Inside the object read bare (`digit`) or explicit (`self.digit`; always the
  field even when shadowed by a parameter).

### Object capabilities (`cap`) 0.0.1

Inside an object: `cap Card { name is string { private_name; } }`.
At module scope: `cap Customer::Card { name is string { Customer::private_name; } }`.
The external owner must be declared in that module (forward and linked-file
owners work); imported objects cannot be extended. External private-field reads
require owner-type qualification, including nested closures; this provides no
owner receiver or owner methods. Both forms share one nominal namespace.
External caps inherit owner type parameters (`cap Box::Card`, body `Box::item`)
and resolve imports in the file containing their declaration.
Type `Owner::Card`; explicit acquisition `owner::Card`; field read `card::name`
or `owner::Card::name`. Type of the left operand resolves `::`, never casing.
Owner expression evaluates once, acquisition is lazy, every read recomputes.
Names are nominal including defining module/ABI major and owner generics:
`Box<int>::Public`. Imports/aliases work normally.

Only immediate owner fields are readable; ordinary locals/shadowing apply.
No `self` (also in closures), implicit owner receiver, owner alias, sibling
field lookup, owner assignment or successor writes. Nested privacy remains.
Bodies are ordinary synchronous blocks with normal result/error checks;
no await or async fields. Explicit capability calls may have effects.

No implicit owner-to-cap conversion, widening, sibling/structural conversion,
direct construction, or acquisition by unwrapping a restricted role reference.
Exact interface survives fields/collections/generics/captures/async transport;
`any` is opaque. Snapshot/lifetime retains the original owner and hidden
resources. Returned values retain their declared contracts. No equality,
ordering, hashing, automatic stringification/serialization/reflection;
container/dynamic operations must not expose owner. Debug may name the type.
Only unique computed fields: no methods, does, has, constructors, lifecycle or
nested caps. Object views removed; `view` and `cap` remain contextual ordinary
identifiers. OIR Profile 0 remains int-only. Full contract:
[Object Caps 0.0.1](object-caps-0.0.1-ru.md).

### Constructors

- **Mandatory and explicit.** Never generated from fields. Stateless objects
  still write `Marker() {}` (or the zero-field shorthand `Marker();`).
- **Full constructors:** typed parameters; body = ordinary statements + field
  binds `field = expr;` / `self.field = expr;`. Overload by **exact arity**
  only. Named or positional calls, never mixed; positionals bind to
  parameters, never to field order. Every field must be bound or defaulted by
  the end, or construction fails.
- **Field shorthand** `Cash(:digit, :cents);` — a closed **named-only**
  allow-list of explicitly typed fields. Unlisted fields cannot be supplied;
  listed fields without defaults are required. May carry a body (auto-binds
  happen first). Bodyless shorthand errors if an unlisted field lacks a
  default.
- **Variadic constructor:** one final typed tail (`Min(first is T, rest is
  ...T)`), positional-only calls, and it must be the object's ONLY
  constructor.
- **Named constructor projection:** `#projected` registers the zero-argument
  action name as a prefix; `#projected(alias)` registers an explicit lowercase
  prefix. `value ParsedInt(text)` means exactly `ParsedInt(text).value()`.
  Objects may register multiple unique prefixes. A projected action is public,
  synchronous, explicitly typed, non-successor, parameterless, and returns a
  type different from the object. Prefixes apply only to direct object
  constructors (including named, empty, variadic, generic and qualified forms),
  resolve statically from that constructor target, and are not role contracts
  or dynamic operators. Construction and its `confirm` checks finish before ordinary action
  dispatch. Single `&` projection syntax is removed; `&&` is unchanged.
- Constructor bodies bind, normalize and validate with `confirm ... else throw`.

### Constructor validation

Use `confirm condition else throw TypedError(...)` directly in each constructor.
Parameters, locals and initialized fields are in scope. A failure interrupts
construction before the object is returned or a projection runs. Defers and owned
resources are cleaned up. Successor actions check their own transitions.

## 15. Actions

### Explicit early return

`return expression;` evaluates exactly one expression and exits the current
named or anonymous action, including `main`. The expression may be a scalar,
array, map, object, or callable. `return 1, 2;` and `return args...;` are invalid;
return `[1, 2]` to return one array. The declared result type is still checked.
Implicit final-expression results remain supported. The module entry point
can return only an `int` in `0..255`; other actions follow their declared types.

```okkama
first_positive(values is @int) is int {
    for values -> value {
        if value > 0 { return value; }
    }
    return 0;
}
```

Blocks, loops, `if`, `match`, and `try`/`catch` do not form a return boundary.
A functional iteration block also returns from its enclosing action. An explicit
anonymous action forms its own boundary. `catch Error` cannot intercept return.
`return;` is valid in a `void` action; ordinary typed actions require a value.
In a successor action, bare `return;` finishes the staged fields and produces the
usual fresh successor; an explicit replacement expression is forbidden.
Return is invalid directly in constructors, `destroy`, module scope, or a test
body. An action declared inside such a body has its own return boundary.

The value is evaluated once **before cleanup**. All reached `defer` expressions
run from inner to outer scope, LIFO within each scope, before owned destruction.
Returned objects, including objects reached through collections and closure
captures, stay alive for the caller. Code after return does not run or register
defers. A cleanup error cancels a pending successful return; remaining cleanup
still runs and an abandoned owned result is destroyed. A return executed by a
deferred expression can replace a pending return, but cannot suppress an existing
error. Calling an action from `defer` only returns from that called action.

Async actions support `return await task;` and await deferred cleanup before
settling their Future. Cancellation while awaiting a return expression also
unwinds cleanup. Cancellation requested during deferred cleanup is delivered
after that cleanup completes, before any result escapes; the Future stays
pending meanwhile. See [runnable example](../examples/return-defer.ok) and the
implementation checklist.

Every named action, role signature and native action requires an explicit
result declaration: `is <type>` or `is void`. The module entry point is
restricted to `main() is int` (optionally `async`); `is void` is forbidden
there. Omitting a result declaration is a syntax error.
`void` is a result marker, not a type or a value: it cannot be used for fields,
parameters, variables, array elements, map entries or generic arguments.
A synchronous void call, or an await of an async void call, may run for its
effects. Using its absent result as a value (including an initializer, argument,
operator operand, chained receiver or typed result) is a semantic error when
its result contract is statically known, and is also guarded at runtime.

The final expression of a void body is evaluated and discarded. Its value
is not transferred to the caller; owned temporary objects are cleaned up at
the normal scope boundary, including `destroy` and `defer` behavior.
An ordinary typed action validates its implicit or explicit result.
Returning the receiver requires an explicit result type and `self` expression;
successor actions retain their existing `^Type` rules.

The existing callable syntax also permits `(T) is void`; anonymous action
syntax is unchanged. `async ... is void` and `async (T) is void` are supported for named actions,
roles, native declarations and bound callbacks. `void` is a result marker,
not a type: there is no user-visible `Future<void>`, null or Unit value.
`await command.commit();` waits for completion without producing a value.
Capturing that await result, passing it as an argument, or placing it in a
collection or value-returning tail is invalid. Typed throws, panic, cancellation,
`defer` and local cleanup follow the ordinary async rules.
A call creates a lazy reusable completion handle: `let pending = command.commit();`
may be followed by `await pending;` more than once. Its inferred internal
completion contract has no source-level type spelling. `spawn` starts it eagerly.
Async anonymous action expressions remain unsupported.


```okkama
total_cents() is int { digit * 100 + cents; }   // last statement = result

touch() is Cash { self; }        // explicit receiver result

private:                 // section marker: everything below is private
helper() is int { 42; }  // callable only as self.helper() from this object
```

- Parameters and results are type-checked at the boundary.
- A caller's contextual expected type applies only to the completed call
  result; action-body expressions do not inherit it.
- **Typed overloads** (fixed actions only): candidates filter by arity/named
  shape, then by runtime argument types. Specificity: exact concrete/applied
  beats role-compatible beats `any`; winner must dominate in every position
  (`(int, any)` vs `(any, int)` is ambiguous for `(int, int)`). Declaration
  order never breaks ties. Opaque `any` values are not narrowed during
  selection. All overloads of a name need compatible return types.
- **Variadic actions:** one explicitly typed, defaultless, final tail:

```okkama
format(pattern is string, values is ...any) is string {
    let materialized is @any = values...;
    pattern;
}
```

  Positional-only calls; the whole fixed prefix is required; each tail value
  checked against the element type (`...any` boxes each value). Inside the
  body the parameter is an opaque `varargs<T>`; `values...` unpack is legal as
  a complete `let` RHS, complete constructor field-bind value, `for` source,
  or final positional argument to a variadic action/full constructor. This
  forwarding supplies only the variadic tail: the fixed prefix stays explicit.
  Sources may be arrays or variadic bindings; ordinary calls allow one final
  positional spread. Array casts allow multiple spreads in any positional slot.
  Expand one level, once, in order; no any unboxing or implicit numeric casts
  at ordinary call boundaries. Typed empty sources provide generic constructor
  evidence. Named/literal spread and fixed/native/scalar
  targets remain invalid. `spawn` retains its separate transfer contract.
  Operands may be array-valued expressions, evaluated once and semantically
  checked. `out <- @string(1, 2, 3)... <- endl` writes 123 plus newline:
  postfix `...` applies to the whole array cast without a precedence change.
  Stream expansion validates each element normally, writes none for empties,
  and stops on failure without rolling back prior writes. Use parentheses in
  `(await source.words())...`. Variadic names cannot be overloaded.

- **Action-value types:** `(T, U) is R`; `() is R`; `async (T) is R` for an
  awaited callable. They may
  nest and contain collection, generic, object, or role types. An anonymous
  action acquires its signature from a typed boundary and cannot later be
  silently retyped. Action types participate in generic substitution, role
  conformance, and fixed overload selection. An untyped action value does not
  guess between otherwise equal overloads. Public `object.action` member
  access creates a bound value retaining its receiver and selecting an
  overload at the exact type boundary.
- **Anonymous action limitations:** parameter annotations and variadic
  parameters are rejected; calls are positional; no currying, partial
  application, async anonymous actions, automatic role conversion, `any`
  transport, or native ABI transport.

## 16. Successor actions — the only state change

```okkama
object Bottle {
    has opened is bool = false;

    Bottle() {}

    open() is ^Bottle {                       // successor action
        confirm !opened else throw BottleAlreadyOpened();
        ^opened = true;                       // stage an update
    }
}

let bottle = Bottle();
bottle.open();                 // RUNTIME ERROR: successor result must be used
let opened = bottle.open();    // correct — `bottle` itself is unchanged
let done = Bottle().open().close();   // successors chain
throw Denied(401).challenged("Bearer"); // throw consumes a successor like return does
```

- `^field = expr;` is valid ONLY inside `is ^Type` actions — never in ordinary
  actions, constructors, `destroy`.
- Completion builds a new object: current fields + staged updates. The
  original is untouched; the successor is a fresh identity with an
  independent lifetime. A mid-action failure produces no partial successor.
- `^Bottle` satisfies `^Package` when `Bottle does Package`.
- Private successor actions are not supported.
- Successors name real domain transitions/results (`opened`, `advanced`,
  `enqueued`), never copied fields. `with` and `with_*` action names are parse
  errors; renamed fluent configuration remains architecturally invalid.
- Composition idiom — propagate the inner successor:

```okkama
object Water {
    has bottle is Package = Bottle();
    Water() {}
    open() is ^Water { ^bottle = bottle.open(); }
}
```

## 17. Ownership, `destroy`, scope-exit order

```okkama
object File {
    has path is string;
    File(:path);
    destroy { path; }      // deterministic cleanup, runs on owning-scope exit
}

main() is int {
    {
        let file = File(path: "data.txt");
    }   // file's destroy runs HERE — deterministic, not GC-driven
    return 0;
}
```

- At most one `destroy`; not callable; reads `self`/fields; no `^field`.
- Owned objects destroy in LIFO (reverse construction) order on scope exit.
- **Ownership follows identity, not names.** Re-binding an already reachable
  object creates a borrowed alias — no double destruction. `self`, parameters,
  and receiver fields are borrows. Aggregates preserve contained identities.
- Successors have independent lifetimes. Unbound temporaries have unspecified
  destructor timing — `let`-bind resources you care about.
- Scope-exit order: deferred expressions (LIFO) → owned destroys (LIFO).
  In `try`, try-owned objects destroy before catch selection.

## 18. Lexical scope boundaries

A declaration body NEVER closes over its caller's runtime environment:

- Action bodies see: `self`, receiver fields, declared parameters, own locals.
- Constructor bodies see: `self`, parameters, field bindings, own locals.
- `destroy` sees: `self`, receiver fields, own locals.
- Receiver/argument/construction expressions evaluate in caller scope.
- Field defaults evaluate in declaration scope.
- Imports never capture importing-module locals.
- Anonymous actions are expression values: they capture enclosing locals,
  parameters, and `self` lexically. Each invocation has its own parameter
  scope and defer stack. It cannot send `next`/`last` to an enclosing loop.

Pass data explicitly; never generate code where a callee reads a caller local:

```okkama
let amount = 42;
Sink().push(amount);     // correct
// Reader().read() cannot see `amount` unless passed or stored
```

## 19. Roles, fakes, tests

### Roles — nominal contracts

```okkama
role Package {
    open() is ^Package;
    close() is ^Package;
}

object Bottle does Package {        // multiple roles: comma-separated
    has opened is bool = false;
    Bottle() {}
    open() is ^Bottle { ^opened = true; }
    close() is ^Bottle { ^opened = false; }
}
```

- Conformance checks at registration: every role action must exist; parameter
  types and fixed/variadic shape match **exactly**; return types compatibly
  (`^Bottle` satisfies `^Package`).
- **Nominal:** right shape without `does` does NOT satisfy the role.
- Role-typed boundaries retain a restricted reference to the original value:
  only declared role actions/overloads are accessible, never concrete fields
  or extra methods. Named arguments use role signature names. Implementations
  run on their full original `self`; other full references are unchanged.
- Restrictions survive aliases, fields/defaults, returns, callbacks, collections,
  bound actions, generics and local async. Covariant successors return the
  selected role contract and retain must-use. Built-in adapters obey this too.
- No ordinary recovery of a full object or widening to extra capabilities.
  Another role is allowed only if nominally implemented by the origin and all
  its signatures are already exposed (same parameters, async/void/variadic
  shape; compatible returns). Iteration needs an exposed nominal cursor contract.
- Bare generic `T` inference retains the role type. Opaque `any` preserves the
  restricted reference. Equality remains the origin's value equality; ownership
  retains its full hidden resource graph. Allowed results keep their own types.
- Restricted role references stay VM-owned: local async tasks can retain them,
  but synchronous process-worker serialization cannot transfer them.
- Checked `throw`/concrete typed `catch` retain existing concrete error routing;
  a role reference can enter `throw` only if it exposes the Error contract;
  `catch e is Error` binds the restricted `text()`-only contract. This is not an
  ordinary role-to-concrete conversion.
- Depend on roles, not concrete types (composition/decorator backbone).

### Fakes — the role ships its own test double

```okkama
role Package {
    open() is ^Package;

    fake {
        has opened is bool = false;         // every fake field needs a default
        open() is ^Package { ^opened = true; }
    }
}
```

- At most one `fake` per role. `Package.fake()` constructs it **only in test
  mode**; production rejects the call.
- Fake actions must satisfy the contract exactly; they may read fields and
  stage `^field`.
- Generic fakes need a declared target: `let e is Echo<int> = Echo.fake();`.
- Every fake instance owns hidden mutable test-runtime KV storage, available
  only through direct `fake.storage.<operation>(...)` calls in that fake's
  action bodies. Supported positional-only operations are `set(key, value)`,
  `get(key)`, `has(key)`, `remove(key)`, `clear()`, and `size()`; missing
  `get`/`remove` keys are programming errors.
- Independent `Role.fake()` calls have isolated storage. Successors of one
  fake share the same storage identity. Storage is not a field or first-class
  value, cannot be accessed as `self.storage`, and does not affect equality,
  successor fields, debug output, conformance, or nominal identity.
- Storage keys obey map-key restrictions (`any`, `byte`, and composites are
  invalid); values may be any supported runtime value, including objects and
  opaque `any`.

### Tests

```okkama
test "open then close returns original" {
    let bottle = Bottle();
    test.is(bottle.open().close(), bottle);
}
```

- Test blocks live at module, object, or role level. Run with
  `perl okkama-pl/bin/okkama -t file.ok`; only root-file tests run, `main`
  does not, imported fakes stay available.
- Automatic `test` binding (no import): `test.ok(bool)`,
  `test.is(actual, expected)`, `test.isnt(actual, expected)` (strict/deep), and
  `test.fail(string)`. It follows the dynamic execution of one test through
  imported actions, is fresh per case, and is absent outside tests. The trusted
  assertion boundary may compare boxed/nested `any` without exposing or
  narrowing it; ordinary `any` equality remains forbidden. Failed assertions
  don't abort the block; an uncaught throw, panic, or runtime error fails only
  the current test. Exit status 0 only when all pass. There is no
  `test.panics`; use `try`/`catch` only for typed throw behavior.
- Bundled `test/more`: stateless `Assertions().ok/is/isnt/fails`; `fails(Error)`
  records `error.text()` unconditionally. `AssertionFailure(message)` is a
  state-only Error whose action computes `assertion failed: ...`. These actions
  require an active test.
- Production parsing skips test/fake bodies entirely (no AST emitted).

## 20. Enums

```okkama
enum Status = (ready, failed, done);

let s is Status = Status.ready;

match s {
    case Status.ready  { "go"; }
    case Status.failed { "stop"; }
    else               { "wait"; }
}
```

Enum values are values, not integers: no ordinal, no numeric conversion, no
fall-through. Cross-enum comparison errors like any mixed category.

## 21. Generics

```okkama
object Box<T> {
    has value is T;
    Box(:value);
    get() is T { value; }
}

role Echo<T> { echo(value is T) is T; }

object EchoBox<T> does Echo<T> {
    EchoBox();
    echo(value is T) is T { value; }
}

let box = Box(value: 7);                        // inferred Box<int>
let bucket is Bucket<int> = Bucket(values: []); // target supplies T for []
```

- **One `T` = one exact concrete type per specialization.** `int`/`float`/
  `byte` never unify: `Min(1, 2.0)` fails — no widening, no unions, no `any`
  fallback.
- No bounds (`T is number` doesn't exist), no variance, no wildcards, no
  defaults, no higher-kinded types. Applied arity is exact.
- **No explicit call-site specialization** — `Box<int>(value: 7)` is invalid;
  specialization is inferred from construction arguments (or a direct declared
  target for empty collections). Contradicting evidence fails construction.
- Explicit generic role evidence is retained across concrete implementers:
  `List<Json>` plus a `JsonBool` value keeps `T = Json`. This does not infer a
  shared role from two concrete types; without explicit role evidence,
  different concrete equations still conflict.
- Builtin arrays are `Iterating<T>`/`Sequencing<T>`/`Sizing` and array
  cursors are `Cursor<T>`, and they are inference evidence: `value
  Maximum(values: [3, 9, 4])` infers `T = int` — no wrapper object needed. An
  untagged `[]` still needs a declared target.
- Specialization is identity: `Box<int>` ≠ `Box<float>` everywhere — fields,
  parameters, successors, catches, imports. `EchoBox<int>` satisfies
  `Echo<int>`, not `Echo<string>`.

## 22. `async` / `await`

```okkama
async loaded(service is Service) is string {
    let prefix = "loaded: ";
    let value = await service.load();
    prefix + value;
}
```

- `async action() is T` is called as `Future<T>`; its body still produces `T`.
- Futures are lazy, start once, and cache values or typed failures.
- `await Future<T>` is `T` and is legal only inside async actions.
- Nested await resumes the containing expression or statement at the exact
  operand without replay. Ordinary left-to-right and lazy-branch semantics are
  preserved across suspension.
- There is no implicit await. Async actions alone do not create worker threads
  or CPU parallelism.
- One cooperative scheduler belongs to the runtime and runs an async root.
- Async shape remains part of role/fake conformance.

### Concurrent and process-parallel `spawn`

```okkama
let job is Future<int> =
    spawn Worker(seed: 7).calculate(input: 35);
let result = await job;
```

- `spawn Worker(...).async_action(...)` eagerly schedules the action in the
  current runtime, reuses its `Future<T>` result, and may retain VM-owned
  values. It is cooperative rather than CPU-parallel.
- `spawn Worker(...).sync_action(...)` eagerly starts one synchronous action
  in an isolated worker process and has type `Future<T>`.
- Constructor/action argument expressions run once, left to right, in the
  parent before launch. Constructor arguments precede action arguments; a
  failure prevents later arguments and launch.
- Multiple spawned Futures start before await and can overlap CPU-bound or
  blocking work.
- Only primitives and recursively transferable arrays/maps cross the boundary.
  Objects, Futures, closures and native handles are rejected.
- Captured receivers, direct calls, and arbitrary blocks are outside the MVP.
  An async selected action is the supported local-task branch and does not
  cross the worker boundary.
- Success/failure is cached per Future; workers share no mutable VM state.

### Bounded typed channels and state owners

`async/channel` exports invariant `Channel<T>`, `Sender<T>`, `Receiver<T>`,
`ChannelClosed`, and `InvalidChannelCapacity`. Capacity must be positive.
Messages, blocked senders, and waiting receivers are FIFO. A send to a full
buffer suspends; close is explicit and idempotent, preserves buffered messages
until received, and fails remaining operations with a typed error. Either a
sender or receiver endpoint may initiate close.

A stateful component is one spawned async task whose local binding contains the
current immutable state. It rebinds that variable to successor values and
accepts typed commands. Immutable clients retain only a `Sender<Command>`.
Request-response uses a capacity-one reply channel carried by each command.

Owner failure does not automatically close channels in this version. Owners
translate expected command failures and register deferred close for the
command channel and active reply before fallible work. Callers perform an
explicit stop, await the owner, and close the command channel. Buffered
requests still require an application cancellation or supervision policy.
The complete language semantics and executable examples are in
[`async-channels.md`](async-channels.md).

## 23. Native boundary and stdlib

### Native declarations (runtime substrate modules only)

```okkama
module runtime/time v1.0.0;

native wall_clock() is int;
native sleep(duration is int) is bool;
native object FileHandle;      // opaque nominal capability type
```

- Bodyless, module-scoped, explicitly typed; a host **provider** implements
  the canonical identity `module/path@ABI_MAJOR::name/fixed_arity`. No
  provider → runtime error; no Okkama fallback. Ordinary object actions can
  NEVER be replaced by host code.
- ABI is positional and value-based: scalars, enums, typed arrays/maps,
  `@byte`, `any` cross; ordinary objects never do.
- Expected host failures are implicitly thrown as `NativeFailure` with
  `.domain()`, `.code()`, `.operation()`, `.message()`, and `.text()`. Branch
  only on `domain`/`code`.
- Opaque handles (`FileHandle`, …) are usable only as declared types: no
  construction, inspection, comparison, or `any` erasure. Close is idempotent;
  use-after-close is thrown as `already_closed`.

### Bundled modules

Historical delivery notes follow. They describe the implementation at the time
each module was first delivered and may retain superseded limits or workarounds.
For the current contracts use `docs/std/`, the language specification and
`docs/bugs-and-fixes-ru.md`. In particular, these notes do not override current
constructor-self restrictions, module isolation, SQLite/TLS/Unix support or
the verified collection inference and map-result rules above.

| Import | Provides |
|---|---|
| `okkama` | `Dump(value).string()` — deterministic debug formatting of scalars, enums, typed collections (recursive, sees through `any`); diagnostics only, never object reflection or serialization |
| `math` | small generic examples (`Max<T>`) |
| `collections/iteration` | nominal immutable `Iterable<T>` / `Cursor<T>` protocol for custom-object `for` sources |
| `collections/list` | iterable persistent `List<T>` with null-free `EmptyList<T>`, linked `prepended/head/tail`, O(1) `size`, and typed `EmptyListAccess` failures |
| `collections/queue` | iterable persistent `Queue<T>` with `enqueued/head/tail`, FIFO order, amortized O(1) operations, and typed `EmptyQueueAccess` |
| `collections/stack` | iterable persistent `Stack<T>` with `pushed/top/popped`, LIFO order, List-backed storage, and typed `EmptyStackAccess` |
| `format` | state-only `string Formatted(pattern, values...)` and `string NamedFormatted(pattern, %string:any)` projections; `{}`/`{name}`, escaped braces, exact Unicode error offsets and typed missing-value failures |
| `template/plain` | pure named string templates via `Template.string(values is TemplateValueSource)`; `<%=`, `<%==`, `<%#`, `<%%`, exact Unicode syntax offsets, typed missing/duplicate bindings, no embedded-code evaluation; `role TemplateValueSource { names() is @string; value(name is string) is string; }` was added later (`template/html` needed it — `TemplateValues`'s constructor is variadic, so it has no second constructor; `HtmlValues` implements the shared source role directly) |
| `template/html` | `HtmlBinding(name is string, value is SafeHtml)` — value is typed `SafeHtml`, never a plain `string`, so there is no code path to bind unescaped text; `safe EscapedHtml(text).safe()` escapes `&<>"'` (ampersand first, no double-encoding of existing `&amp;`); bare `SafeHtml(text)` is the explicit trust assertion (e.g. a nested `HtmlTemplate.render()` result); `<%=`/`<%==` stay lexically identical — escaping is decided by the binding's type, not the tag; `HtmlTemplate.render` reuses `template/plain`'s `Template` lexer via `HtmlValues does TemplateValueSource`; only `InvalidTemplate` becomes `InvalidHtmlTemplate`, `MissingTemplateValue`/`DuplicateTemplateValue` reused unchanged |
| `encoding/utf8` | `bytes Utf8Encoded(string) is @byte`, strict `string Utf8Decoded(@byte) is string`, and typed `InvalidUtf8.offset()`; no BOM, normalization, replacement, or implicit conversion |
| `encoding/binary` | cursor-free `BinaryReader` plus state-only `BinaryPart` values and `BinarySequence`; explicit u16/u32/i32/i64 big/little-endian representations, typed bounds/EOF failures, pure Okkama implementation |
| `encoding/base64` | pure-Okkama canonical padded RFC 4648 `string Base64Encoded(@byte) is string` and strict `bytes Base64Decoded(string) is @byte`; standard alphabet only, typed `InvalidBase64.offset()`, no whitespace or native codec |
| `encoding/hex` | pure-Okkama lowercase canonical `string HexEncoded(@byte) is string` and strict `bytes HexDecoded(string) is @byte`; decoder accepts ASCII `A-F` but rejects prefixes, separators, whitespace and Unicode with typed `InvalidHex.offset()` |
| `encoding/json` | pure-Okkama nominal `Json` values with exact `JsonKind`, strict `json JsonSource(string) is Json`, deterministic compact encoding, exact decimal lexemes, checked `int`/`float` projections, surrogate-pair handling, duplicate rejection and original-source line/column/offset failures; immutable `JsonReader` navigates via `required()`/`at()`, provides current-node and `required_*` scalar projections, and same-typed `optional_*(name, fallback)` for only missing/null members, with path-aware `UnexpectedJsonKind` |
| `encoding/csv` | `CsvDialect()` (RFC 4180 defaults) or full `(delimiter, quote, line_separator, header)` — all `string`, not the drafted `char` (confirmed `char` has zero interop with `string`: no `.string()`, `+`, or `==` across the two, and it's unused elsewhere in the stdlib); `CsvReader(Input, dialect).rows() is @@string` drains fully then parses (RFC 4180 quoting, `""` = literal quote, no spurious trailing empty row, blank line = one empty-field row); scanner finds next delimiter/quote via a bounded doubling-window slice, not a slice from the offset to the end of the whole document (that was O(document length) per field, quadratic overall — confirmed, then fixed); the whole document is one flat `.reduce()` processing one field at a time (not nested per-row/per-field reduces, which independently confirmed quadratic; not self-recursion, which reliably hit Perl's "Deep recursion" warning past ~50 rows; not `collections/queue` accumulation, whose `tail()` rear-reversal recurses as deep as the accumulated size on a build-then-drain access pattern) — three distinct scale bugs found and fixed directly, not merely anticipated; `CsvWriter(Output, dialect).write(row is @string)` quotes only fields containing delimiter/quote/separator; `CsvRow(header, values, line)` is a separate, `CsvReader`-independent pairing step — `CsvDialect.header()` is metadata for callers, not something `CsvReader` acts on; `InvalidCsv does Error, LocatedError` covers parse errors and `CsvRow` lookup errors alike |
| `encoding/toml` | practical single-line-document TOML v1.0.0 subset (no multi-line strings/arrays, no `[[array of tables]]`, dotted keys inside a `[table]` stay literal entry names — only explicit `[a.b]` headers nest); `TomlDateTime` wraps `time/date.Date` (Local Date only); `TomlDocument.value(path is string)`/`.has()` resolve via a flat list of fully-expanded `(header, key, value)` records built by the parser, not a nested `TomlTable` tree descended per segment — Okkama has no runtime role-to-concrete downcast, so an intermediate `TomlValue` can never be treated as a `TomlTable` again to keep descending; `.string()` groups the same flat list by header to render `[table]` sections; same `char`-has-no-`string()`-interop finding as `encoding/csv`; hit and fixed the identical `collections/queue` scale bug plus a second one (self-recursive array/inline-table element parsing, fixed via `.reduce()` over a line-length-bounded range mirroring how `TomlDocumentParser.entries()` already processes lines) |
| `crypto/random` | stateless `SecureRandom().bytes(size)` OS-CSPRNG capability, deterministic test-only `SecureByteSource.fake()`, typed size/unavailability errors, no PRNG fallback or native diagnostic leakage |
| `crypto/digest` | `Digest` role (`bytes()`/`hex()`/`base64()`, hex/base64 compose `encoding/hex`/`encoding/base64`); state-only `Sha256`/`Sha384`/`Sha512` compute once from a `(value is @byte)` ctor or a `(source is Input, chunk_size is int)` streaming ctor that drains to completion (one-shot `runtime/crypto` ABI, no incremental handle); two ctor arities because Okkama constructor overload resolution is arity-based, not type-based; every native failure becomes one sanitized `UnsupportedDigest`; `DigestMismatch` is exported verification vocabulary (`confirm digest.bytes() == expected else throw DigestMismatch();`), not thrown internally |
| `crypto/hmac` | `HmacSha256/384/512(key is @byte, value is @byte)` — `bytes()`, `hex()`, `matches(expected is @byte) is bool`; `matches` uses `runtime/crypto.secure_equal` (constant-time), not `==` — unlike `crypto/digest` (whose bytes aren't secret), a MAC is verified against attacker-controlled input where a leaky comparison could help forge it; reuses `crypto/digest.UnsupportedDigest` for native failure translation (same fixed-three-algorithm-strings reasoning) rather than declaring a duplicate; `InvalidMacKey` rejects an empty key before dispatch |
| `socket/socket` | pure vocabulary, no native surface at all (the API draft's `runtime/net`/`runtime/poll` dependency line was wrong — corrected to just `net/address`/`time/duration`/`errors`); `role Socket { local() is SocketAddress; }`; `enum AddressReuse/KeepAlivePolicy/PacketDelay = (enabled, disabled)` in place of boolean socket-option toggles; `SocketOptions(address_reuse, keep_alive, packet_delay)`, `SocketTimeout(connection, input, output is Duration)`; `SocketClosed` fixed text (like `ClosedBuffer`), `SocketFailure(reason is string)` message-carrying (like `errors.ok`'s general errors) for `socket/tcp`/`udp`/`unix` to throw once they exist |
| `sql/value` | pure vocabulary, no native surface; `enum SqlType = (integer, float, string, bytes, boolean, date, datetime, null_value)` — eighth member renamed from the draft's `null`, since `null` is a reserved lexer keyword in this null-free language; `role SqlValue { sql_type() is SqlType; }`; eight one-field state-only wrappers (`SqlInt`/`SqlFloat`/`SqlString`/`SqlBytes`/`SqlBool`/`SqlDate`/`SqlDateTime`/`SqlNull`), each a fixed `sql_type()` plus one type-named accessor (`SqlNull` has no accessor — it carries no payload); `SqlDate`/`SqlDateTime` wrap real `time/date.Date`/`DateTime`; `SqlValues(values is @SqlValue)` — `values() is @SqlValue`, same role-array-container shape as `Arguments`/`JsonArray`/`TomlArray`; `UnsupportedSqlValue(reason is string)` for `sql/statement`/`sql/query` (still ahead) to throw when translating an unmapped host value |
| `http/status` | `Status(code is int, reason is string)` — concrete, not a role (draft writes it unprefixed, unlike every genuine `role X` in the same doc); validates `code` in `100..599` with non-empty `reason`, else `InvalidHttpStatus`; `code()`/`reason()`/`informational()`/`successful()`/`redirect()`/`client_error()`/`server_error()`, the last five computed from `code` by numeric range, once; `Ok`/`Created`/`BadRequest`/`NotFoundStatus`/`InternalServerError` are zero-argument objects each projecting (`status Ok()`, ...) into a `Status` — the same named-constructor projection used by `time/duration`'s `duration Seconds(5)`/`duration Hours(5)`, confirmed zero-arg projection parses/evaluates before committing to the shape; named `NotFoundStatus` not `NotFound` to avoid colliding with `errors.ok`'s existing `NotFound`; module scope only allows `object`/`role`/`enum`/`native`/`main` (confirmed: a free top-level action is rejected with "Only main() may be declared at module scope."), which is why the named shortcuts can't be plain functions |
| `http/headers` | `HeaderName(name)` validates non-empty, no whitespace/`:`/CR/LF, else `InvalidHeaderName`; `string()` keeps original spelling, `folded()` gives an ASCII-lowercased form via an internal `CaseFold` helper (26 chained `.replace()` calls — no builtin `lower()`/`upper()` exists); `Header(name, value)` trims `value` via builtin `.trim()` (no `string/trim` module exists — corrected dependency), then rejects a trimmed value containing `\r`/`\n` as `InvalidHeaderValue` (header/response-splitting injection); `Headers(entries).has/first/all` compare via `folded()` strings with `==`, never comparing two `HeaderName` objects directly (default object `==` is deep-structural over every field including private ones, so two names differing only by case aren't `==`); `first` throws `MissingHeader`; `ExtendedHeaders`/`FilteredHeaders` are add-one/remove-by-name, mirroring each other; `ExtendedHeaders` needs `collections/list.List`/`EmptyList` (arrays have no push/append/concat) with the same build-reversed-then-reverse-again idiom as `encoding/csv`/`toml`, and hit the identical "bare tail `.reduce(EmptyList())` can't infer T from the enclosing action's declared return type" bug — fixed with an explicit `let ... is List<Header> = ...` binding; `FilteredHeaders` only needs plain `.filter()` since removal never grows the array |
| `log/event` | pure state-only vocabulary, simplest module this session; `enum LogLevel = (trace, debug, info, warning, error, fatal)`; `LogField(name, value)` rejects only an empty `name` as `InvalidLogField`; `LogEvent(level, message, fields is @LogField, instant is Instant)` validates nothing else — empty `message`/`fields` are legitimate, `LogLevel` is already an exhaustive closed enum — queries just expose the four constructor values verbatim |
| `db/result` | `DbResult(affected is int, last_id is int)` (two-arity, real last id) vs. `DbResult(affected is int)` (one-arity, no last id) — arity-based constructor overload used deliberately to encode "did this statement produce a last id" instead of a nullable field; `has_last_id() is bool` reports which shape was used; `last_id() is int` throws `MissingLastId` rather than returning `0` when built from the one-arg form |
| `db/row` | `Row(names is @string, values is @SqlValue)` pairs columns/values exactly like `encoding/csv.CsvRow`'s `(header, values, line)`; `value(name)`/`at(index)` both throw `MissingColumn`, `value` linear-scans via the same `(0..size).reduce(-1) -> found, index {...}` sentinel idiom `CsvRow.index_of` uses; both return the `SqlValue` **role**, never a concrete `SqlInt`/etc — no runtime downcast exists in this language, so `Row` can't inspect beyond `sql_type()`; `InvalidColumnType` is exported vocabulary for a future domain adapter (the draft's own `UserRow(row).user()` example) to throw, never thrown by `Row` itself — same "exported, not internally thrown" shape as `crypto/digest.DigestMismatch`; last module of Phase 5, whose gate (deterministic formats, fixed socket lifecycle, fixed SQL-null identity) is now satisfied |
| `fs/file` | first Phase 6 module, first real-file-I/O module this session; `runtime/fs.ok` already declared the full native surface (`open`/`read`/`write`/`seek`/`flush`/`close`/`unlink` on `FileHandle`) — no native provider work needed; confirmed directly (throwaway real-file probe) that an object action can safely be named the same as a same-named imported native free function it calls internally (`OpenedFile.write` calling native `write`) — the call unambiguously resolves to the native import, never self-recursing; `File.bytes()` sizes one `read()` call via `Meta(path).size()` rather than looping+concatenating chunks (arrays have no append; per-byte accumulation for a whole file would be too slow) — a short read panics as a provider-invariant violation, not a caller mistake; `OpenedFile.bytes(limit)` needs none of that since `Input`'s own "at most limit bytes, EOF=empty" contract already matches one native `read()` call directly; `OpenedFile.closed()` has no native FileHandle query to back it, so it's driven by a `runtime/memory.ByteMemoryHandle` sentinel via the already-idempotent `byte_memory_close`/`byte_memory_closed` pair — same mutable-state-outside-constructor workaround `test/fake_clock` established; `OpenedFile`'s one constructor always opens `READ\|WRITE\|CREATE` (never `TRUNCATE`) and is a sibling to `File`, not built on top of it — `File`'s own operations use their own distinct flag combinations instead; `MissingFile` (this module) is distinct from `fs/meta.MissingPath` (same ENOENT, different module boundary — metadata vs. content access) |
| `fs/dir` | last Phase 6 filesystem module; `Dir(path)` — `create()`/`create_all()` (idempotent, recurses via `PathValue.parent()`, base case = `fs/path.MissingPathParent` at the real root), `entries()` (single level), `remove()` (proactively checks `entries().size()==0` rather than trusting `ENOTEMPTY`'s errno mapping, which is bucketed into the same broad `invalid_input` code as several unrelated conditions), `remove_all()` (recursive); `DirEntry`/`DirectoryWalk` round out listing/walking; both `remove_all()` and `DirectoryWalk` classify entries via `Meta`'s `lstat`-backed `kind()` so a symlink-to-directory is always a leaf, never recursed into (verified with a real symlink pointing outside the removed tree); recursion happens once per directory **depth** (small, bounded), never once per **entry** (unbounded) — breadth handled iteratively; `Dir.entries()`'s scan uses the same bounded-window-doubling-retry idiom as `encoding/csv`/`toml`'s lookahead slicing, since directory entry count has no cheap upfront size hint. **Major finding, documented but deliberately NOT fixed here**: discovered and root-caused a genuine interpreter-level O(n²) bug — `Okkama::Interpreter::visit_let_decl` (`Interpreter.pm:800-815`) calls `unowned_objects_for_binding` on every `let`, which fully re-walks every live variable's entire reachable object graph from scratch each time (`Interpreter.pm:5197-5267`), un-memoized; any `let` executed once per loop iteration while a growing structure (e.g. a `List<T>` accumulator) is in scope costs O(current size) *every iteration* — generic-and-type-agnostic (confirmed for `int`/`string`/`@byte` alike; composite types just cost more per walked node, same O(n²) shape), not specific to `LinkedList<T>`'s own generic-checking (which is O(1) per call, confirmed via instrumentation). A bare `.reduce(EmptyList()) -> acc, i { acc.prepended(fresh); }` with **no** intermediate `let` in the callback is linear regardless of element type — that's the workaround this module uses throughout (collapsing per-iteration `let`+`try` into one `try` wrapped around the whole `reduce` call). Real-world trigger: `Dir.entries()` on 2000 files took 65s before the workaround, 16.6s after (now linear: 3.57s/7.67s/16.6s at 500/1000/2000). Likely affects every already-shipped module using `collections/list.List<T>` with non-scalar `T` inside a per-iteration `let` (`encoding/csv`, `encoding/toml`, `http/headers`, `db/row`, ...) — none observed hanging only because their tests stay within the "still fast enough" region of the curve. Fix is reportedly small (short-circuit the walk on an already-`$claimed_object_ids` object, confined to `Interpreter.pm:5197-5276`) but was left for a dedicated future session rather than bundled into a stdlib-module change. |
| `fs/temp` | first Phase 7 ("owned workflows") module; dropped the draft's `env/cwd` dependency — `TempRoot(directory is PathValue)` takes its containing directory explicitly rather than defaulting to the ambient cwd, matching Phase 7's own "no hidden ambient state" gate; `keep()` needs mutable-looking per-instance state (has `destroy` already fired a removal decision?) in an otherwise-immutable object, reusing `test/fake_clock`'s exact "owned `runtime/memory.ByteMemoryHandle` as a private mutable cell" idiom, but with a bare presence-marker byte (`[]`=not kept, `[byte(1)]`=kept) instead of `FakeClock`'s numeric `encoding/binary` encoding, so `encoding/binary` isn't pulled in; confirmed directly that `destroy` cannot call the object's own *private* actions (`self.is_kept()` from `destroy` failed with "is private" — `Interpreter.pm`'s private-call check requires `$action_context`, which `run_destroy_blocks`/`execute_block` never sets for `destroy`, unlike ordinary action dispatch) — fixed by inlining the byte-memory read directly in `destroy` rather than factoring it into a private helper, while `mark_kept()` stays private since it's only ever called from the public `keep()`; unique temp names use an 8-byte random hex suffix (`random`/`encoding/hex`) with real atomic collision avoidance, not a racy exists-then-create check — `TempDir` for free via POSIX `mkdir`'s existing atomicity (`Dir.create()` already throws `AlreadyExists`), `TempFile` via a direct `runtime/fs.open` call using the native `WRITE\|CREATE\|EXCLUSIVE` flag combination (`2+4+32=38`) since `File` itself exposes no public exclusive-create method; a missing `TempRoot` directory is reported as one unified `TemporaryEntryFailure`, not a panic, for both the file path (`runtime/fs`'s `not_found`) and the directory path (`fs/dir.MissingDirectory`); testing "did destroy actually remove this" needs the checked path available *outside* the scope owning the temp entry, but Okkama has no block *expression* (`{ ... }` is a statement only, confirmed by grammar: `primary` has no bare-block alternative) — worked around by binding the path in the *outer* scope first, then using an ordinary nested block *statement* to own the short-lived entry |
| `proc/cmd` | second Phase 7 module; dropped the draft's `time/duration` dependency (no checklist signature takes a timeout); `Cmd` always calls native `spawn_configured`, never plain `spawn` — `CommandEnvironment` is a closed explicit binding list with no "inherit ambient" sentinel, so a caller wanting inheritance builds it from `env/var.Env().variables()` themselves; confirmed by reading `Okkama::Runtime::Native::Core::Process::_spawn_process` that after a successful spawn the native provider already closes the parent's now-irrelevant pipe endpoint itself (input pipe's read end, output/errors pipes' write ends, since those moved into the child via `dup2`) — `proc/cmd.ok` only ever needs to write to `input()`'s pipe and read from `output()`/`errors()`'s, no extra endpoint bookkeeping; `RunningCmd` has an explicit public `close()` and *no* `destroy` block, deliberately following `fs/file.OpenedFile`'s explicit-`Closeable` precedent rather than `time/timer.Timer`'s destroy-only one (the checklist wants a public `close()` here, and a destroy block would also need to tolerate the native handle table's `already_closed` failure to avoid a panic on a double release — sidestepped entirely by only ever releasing one way); `try_wait()` throws `MissingExitStatus` when native `try_wait`'s `state == 0` ("still running") since `ExitStatus` has no representation for that, giving the checklist's second error type a real purpose (same "shape doesn't carry this information" pattern as `db/result.MissingLastId`); `terminate()`/`kill()` are portable signal 1 (SIGTERM) / 2 (SIGKILL) through one private `signalled()` helper, which also maps native `kill`'s `invalid_input` (process already exited) to `MissingExitStatus` rather than a generic panic; tests spawn real, always-present POSIX binaries (`/usr/bin/true`, `/usr/bin/false`, `/bin/echo`, `/bin/cat`, `/bin/pwd`) rather than a mock, matching this session's real-OS-boundary preference (`socket/tcp`, `crypto/tls`); one wrong assumption caught during development — a relative program name combined with `ExplicitDirectory` was expected to bypass `$PATH` resolution and fail, but Perl's `exec { $program } $program, @arguments` indirect-object form still searches `$PATH` regardless of the child's working directory — the test was corrected to check `ExplicitDirectory`'s actual real effect (the child's own reported cwd via `/bin/pwd`) instead of the wrong assumption; the whole module is POSIX-only since the native provider itself returns `unsupported` for spawning on Windows |
| `socket/tcp` | first real-network-I/O module; `runtime/net.ok` already declared the full native surface (`socket`/`bind`/`listen`/`accept`/`connect`/`send`/`recv`/`shutdown`/`local_address`/`peer_address`/`socket_close`, plus `ip_parse`/`ip_format`/`address_family`/`address_port`) — no native provider work needed; family numbers 1=ipv4/2=ipv6, socket-type 1=stream, shutdown direction 0=read/1=write/2=both, all confirmed from `Core::Net`; `net/address.IpAddress.envelope()` (the connect-ready wire-format builder) is private and hardcodes port 0, so this module calls `ip_parse` directly itself via a small `Envelope` helper — same "duplicate the family-number mapping rather than centralize it" precedent `net/dns.ok` already set; `TcpSocket` has two arity-distinguished constructors — `(address is SocketAddress)` creates but doesn't dial (separate `connect()` command), `(handle is SocketHandle, peer is SocketAddress)` wraps an already-`accept()`-ed handle for `TcpListener.socket()`; `TcpListener.address()` queries `local_address()` once at construction and stores the *real* bound address, not the constructor's request — matters when port 0 ("OS pick one") was requested, confirmed a bound listener reports a real nonzero port; confirmed directly (isolated probe) that a field assigned earlier in a constructor cannot be read back as a bare name OR via `self.field` later in that same constructor — only a public-action call or a `let`-bound local can carry it forward — `TcpListener`'s ctor was restructured around a local `created is SocketHandle` for exactly this reason; confirmed directly (raw `runtime/net`-level probe) that `shutdown(peer, SHUT_RD)` fails with `ENOTCONN` specifically after the *other side's* `SHUT_WR` on Darwin/macOS — a real BSD kernel TCP quirk, not a bug, so the test suite never chains shutdown across peers in that order; `TcpConnection(resolver, host)` does ordered-address-attempt recursion (confirmed with a `FixedDns` resolving `[127.0.0.2 (refused), 127.0.0.1 (reachable)]` against a real listener) but deliberately drops the draft's "with timeout" — `runtime/net`'s `connect` has no non-blocking-socket setup exposed to Okkama code at all, so no individual attempt can be bounded; a real, documented native-surface gap, not a silently-ignored parameter |
| `socket/udp` | mirrors `socket/tcp`'s shape (same `Envelope`/`ParsedAddress`/`FamilyNumber` helpers, same `ByteMemoryHandle`-backed `closed()`, independently duplicated rather than shared); reuses `io/output.IncompleteWrite`/`io/input.InvalidReadSize` for short-send/bad-limit, reserving `UdpFailure` for real native-call failures; `Datagram.source()` is a genuine independently-parsed `SocketAddress` from `recvfrom`'s envelope via `datagram_address`/`datagram_payload`, confirmed to match the real sender socket's own bound port across two independent loopback sockets; confirmed directly (raw `runtime/net`-level probe) that `connect(peer)` followed by explicit-target `send()` to that *same* already-connected peer fails with a real, platform-specific unmapped errno — a genuine OS-level UDP quirk, not a bug, so the test suite exercises `connect()` and `send()` independently rather than chained on one socket |
| `socket/unix` | current native provider has **zero** `AF_UNIX` support at all, confirmed directly (`Okkama::Runtime::Native::Core::Net::_host_family` only maps `ipv4`/`ipv6`; `socket(3, 1, 0)` cleanly returns `NativeFailure` code `unsupported`, not a crash) — a universal substrate gap today, not host-specific despite the draft's "unsupported-host" phrasing; both `UnixSocket`/`UnixListener` constructors attempt the real native call and translate that specific failure into `UnsupportedUnixSocket` (forward-compatible: a future native `AF_UNIX` addition would likely need zero changes here, since nothing short-circuits before the real native boundary); since both constructors always throw, no instance can ever exist, so `connect()`/`bytes()`/`write()`/`socket()`/`path()` all have type-correct `panic`-stub bodies that satisfy their roles but are confirmed unreachable — verified directly that a constructor may always-throw without ever assigning its declared fields, before relying on that shape for every dead-code action body |
| `crypto/tls` | final Phase 6 module; corrected the draft's dependency line — no `socket/tcp` — since `socket/tcp.TcpSocket`'s underlying `SocketHandle` is intentionally private (no public accessor, per "no raw handle leaks"), so `TlsSocket` connects its own raw TCP socket directly via `runtime/net` and wraps it with `runtime/tls.tls_client`, duplicating a small amount of connect logic (same precedent as `socket/udp`/`net/dns` duplicating rather than coupling); internal helpers named `TlsFamilyNumber`/`TlsEnvelope`/`TlsParsedAddress` (not the bare names `socket/tcp.ok`/`socket/udp.ok` use) — confirmed directly that importing `socket/tcp` (even via a narrow `take TcpListener` used only for this module's own test fixtures) pulls its *entire* internal declaration space into scope, causing a real "Object ... is already declared" collision until renamed; `CertificateAuthority.path().bytes()` is `tls_client`'s `ca_file` parameter directly — confirmed by reading `Core::TLS`'s handler that it's a byte-encoded *file path* (matching `runtime/fs`'s `path is @byte` convention), not inline PEM, empty means system default CAs; `IO::Socket::SSL` is not installed in this dev environment (the same reason `t/interpreter/native/53-native-tls.t` has failed on every full-suite run all session) — every real TLS native call cleanly returns `NativeFailure` code `unsupported`, confirmed directly, translated into typed `TlsFailure`; tests exercise the one path reachable here — a real TCP connection via a real `TcpListener` fixture, then a confirmed clean `TlsFailure` from the unavailable engine; `peer_certificate()` before `handshake()` completes throws `CertificateFailure` (native `invalid_input` for an unhandshaken `TlsHandle`, confirmed and translated) — the concrete, real trigger for that error type. **This was the last module of the session's continuous Phase 5 → Phase 6 run** (twelve modules: `sql/value` through `crypto/tls`), alongside the major `fs/dir`-discovered interpreter ownership-walk bug (see that entry), deliberately left unfixed as an out-of-scope, separately-flagged finding |
| `sql/dialect` | corrected the draft's dependency line — no `sql/value` (nothing here touches `SqlValue`; that's `sql/statement`/`sql/query`'s job); `role Dialect { placeholder(index) is string; identifier(value) is string; boolean(value) is string; }`; `SqliteDialect`/`MysqlDialect` use plain unnumbered `?`, `PostgresDialect` numbers `$1`/`$2`/...; `SqliteDialect`/`PostgresDialect` double-quote identifiers, `MysqlDialect` backtick-quotes, all three escape an embedded quote by doubling it; `PostgresDialect` renders real `TRUE`/`FALSE`, `SqliteDialect`/`MysqlDialect` (no native boolean type) render `1`/`0`; `UnsupportedSqlFeature` has one real trigger (all three throw it for a non-positive placeholder index — the one universal SQL rule none relax) plus exported headroom for a future, more limited dialect, same "exported, not necessarily thrown by every implementer" shape as `crypto/digest.DigestMismatch`/`db/row.InvalidColumnType` |
| `sql/statement` | `role Statement { sql(dialect is Dialect) is string; values() is SqlValues; }`; `Insert(table, columns is @string, parameters is SqlValues)`/`Update(...)` validate `columns.size() == parameters.values().size()` at construction (not render time), else `InvalidStatement`; `Update` additionally requires `columns.size() > 0`; both render numbered placeholders via `(0..columns.size()).map() -> index {...dialect.placeholder(index+1)...}`, no paired name/index helper object needed; `Delete(table)` is unconditional, `values()` always empty; `Update`/`Delete` have no `WHERE`/predicate support at all — searched the whole draft, the only predicate-shaped thing anywhere is `sql/query.Select`'s sole `EveryRow` value, no role/other implementer defined, so filtered data-changing statements are out of scope until one exists; `RawStatement.sql(dialect)` ignores `dialect` entirely, confirmed directly (`SqliteDialect()` and `PostgresDialect()` render byte-identical output) |
| `sql/query` | corrected the draft's dependency line — no `sql/statement` (nothing here is reused from it; `RawQuery` is structurally identical to `RawStatement` but independently declared, same duplicate-rather-than-couple precedent); added one small internal `role Rows { clause() is string; }` (not in the original draft) so `UnlimitedRows`/`RowWindow` share a shape and `Select.sql()` never branches on which one it got (`window.clause()` unconditionally — `""` vs. `" LIMIT n OFFSET m"`); `EveryRow` has no role at all — it's `Select`'s ONLY predicate value anywhere in the draft (no `WHERE`/condition role or other implementer defined), so it's a plain concrete parameter type; `TableName`/`ColumnName` throw `InvalidSqlIdentifier` for an empty string, `RowWindow` throws `InvalidQuery` for a negative limit/offset, `Select` throws `InvalidQuery` for an empty projection — all validated at construction, not render time; completes the entire `sql/*` namespace (`sql/value` → `sql/dialect` → `sql/statement` → `sql/query`, confirmed no other `sql/*` entry exists anywhere in the draft) |
| `http/request` | `enum Method = (get, head, post, put, patch, delete, options, connect, trace)`; `role Request { method() is Method; url() is Url; headers() is Headers; body() is Input; }`; `HttpRequest(method, address is Url, fields is Headers, content is Input)` is the one real field-holding implementer; `Get`/`Post`/`Put`/`Patch`/`Delete` each take `(url, headers, body)`, fix their own `Method`, and hold+delegate to one internal `HttpRequest` rather than duplicating its four fields/accessors five times; `InvalidRequest` has no internal throw site at all — neither `HttpRequest` nor any named constructor validates anything, it's exported purely for a future `http/client`/`http/server` to throw once it has context to judge a request malformed, same "exported, not necessarily thrown here" shape as `sql/dialect.UnsupportedSqlFeature`/`crypto/digest.DigestMismatch` |
| `http/response` | `role Response { status() is Status; headers() is Headers; body() is Input; }`; `HttpResponse` is the one field-holding implementer; `ResponseBytes(response).bytes()` does exactly TWO reads — one bounded at 16 MiB (matching `runtime/tls`'s own `$MAX_READ_SIZE` cap) then one confirming empty — NOT the bounded-window-doubling-retry idiom from `fs/dir`, because `Input.bytes(limit)` can return up to `limit` bytes per call (unlike one-entry-per-call `readdir()`) and a stream can't be rewound like a directory; throws `InvalidResponseBody` if the body exceeds the cap, never silently truncates; `ResponseString`/`ResponseJson` build on `ResponseBytes`/`ResponseString` by calling their projected actions as plain ordinary methods internally (`ResponseBytes(source).bytes()`), because `bytes ResponseBytes(source)` is exactly that call; both translate `InvalidUtf8`/`InvalidJson` into `InvalidResponseBody` |
| `http/handler` | HTTP handlers are first-class bound actions of exact type `async (Request) is Response`, not nominal `Handler` objects: `controller.response` retains the controller and its DI graph; `HandlerChain` takes an array of those actions and awaits each in order; a handler declines by *throwing* `HandlerFailure`, and the chain throws the same error if all decline; `HandlerFake(response).response` supplies a deterministic bound fixture action and `DecliningHandler().response` exercises fallthrough |
| `http/client` | fourth Phase 7 module, first hand-rolled wire-protocol client this session; connects via `socket/tcp.TcpConnection`(`http://`)/`crypto/tls.TlsSocket`(`https://`, first DNS candidate only — no multi-candidate retry helper exists for TLS the way `TcpConnection` gives for plain TCP), writes the request line/headers/drained-body directly through `Output.write(@byte)`, and parses the response byte-at-a-time for the status line and each header (`read_line_accumulated` recursing on `\r\n`) then via `Content-Length`-bounded `Input.bytes()` calls reassembled through `encoding/binary.BinarySequence` (arrays have no append, so multi-chunk byte reassembly always goes through this tool); no chunked encoding, no read-until-close fallback for a missing `Content-Length` (parses as a zero-byte body); found and fixed a real cross-module collision — `net/url.ok` and `fs/path.ok` each independently declare their own internal `AsciiLetter` helper (correct in isolation), which collided as soon as one consumer (this module, needing `net/url` directly plus `fs/path` transitively through `crypto/tls.CertificateAuthority`) imported both — fixed by renaming `net/url.ok`'s own `AsciiLetter` to `UrlAsciiLetter` (all 5 references), the same "rename the newer overlapping helper" precedent `crypto/tls.ok` already set for itself against `socket/tcp.ok`; found and fixed a second real defect — `exchanged_tcp` originally only caught `TcpFailure` around the initial connect, so a `TcpFailure` from `TcpSocket.write`/`.bytes()` failing *during* the exchange leaked past this module's declared two-error surface uncaught, fixed by wrapping the exchange step's `TcpFailure` into `HttpClientFailure` too; spent significant debugging effort on what looked like a serious recursion/ownership bug in the redirect path (`self.attempt()` calling itself) — isolated through incremental reproduction (sequential non-recursive calls: fine; true self-recursion with a fresh request: fine; only a *second real `net/dns.Dns()` resolution of the same literal host* within one process was slow, consistently ~10s) down to a real, environment-specific `getaddrinfo`-level resolver slowness in this sandbox, not a logic bug — confirmed by removing an artificially-short test alarm and watching the same "failing" run simply succeed given more time; no workaround applied to `Client` itself since normal network environments would not exhibit this; `ClientFake(response)` is a real constructable object for the same reason `http/handler.HandlerFake` is, not the built-in `HttpClient.fake()` role-fake mechanism; `ClientPolicy.timeout()` is stored and exposed but not enforced, the same pre-existing, already-documented gap as `socket/tcp.TcpConnection` (`runtime/net.connect` exposes no non-blocking-socket setup to Okkama code) |
| `log/formatter` | `role LogFormatter { format(event is LogEvent) is string; }`; `LineFormatter` renders `[level] message key=value ...` (no trailing space when empty) and deliberately OMITS the instant (human-oriented; most destinations timestamp on their own); `JsonFormatter` renders a COMPLETE record `{"level":...,"message":...,"instant":...,"fields":{...}}` with `fields` always a nested JSON object (empty `{}`, never absent, when there are none); one internal `LogLevelName` helper does the 6-way enum-to-string mapping via `if X {...} else if Y {...} else {...}` as a bare tail expression (confirmed `else if` parses fine as a bare action tail but NOT as a `let`'s right-hand side, matching `fs/meta.FileKindFromCode` precedent exactly); confirmed directly that `.map()` requires every iteration to produce a real value (`test.is(...)` alone as the body throws "Functional map result... has no value") — side-effecting iteration needs a plain `for` loop instead, matching `io/input.ok`'s own test precedent; two fields sharing a name surface as `encoding/json.DuplicateJsonMember` when `JsonFormatter` encodes them — `log/event.LogField` only rejects an empty name, not a duplicate, and this module adds no redundant check of its own, reusing the JSON encoder's existing one. **Sixth and final module of this continuation** (after `sql/dialect`/`sql/statement`/`sql/query`/`http/request`/`http/response`), itself following the twelve-module Phase 5→6 run — eighteen modules total this session |
| `log/logger` | fifth Phase 7 module; `MemoryLogger` needs to accumulate arbitrary `LogEvent` objects across calls on one instance, but the only instance-local mutable-state primitive (`runtime/memory.ByteMemoryHandle`) holds nothing but raw bytes, so events are serialized to one delimited, individually-Base64-escaped text line per event, joined by `\n` in the byte cell, and reconstructed on `events()`; confirmed directly that `=` between a field name and its Base64 value collides with Base64's own padding character (`=`) the first time a message/field triggered it — switched to `:` (outside the Base64 alphabet); found and fixed a SECOND cross-module internal-namespace collision the same day as `net/url`'s — this module's own bidirectional `LogLevel`/`ClockIdentity` name-mapping helpers collided with `log/formatter.ok`'s own internal `LogLevelName` (already an internal dependency here), fixed by prefixing this module's four helpers with `Memory` instead, since `log/formatter.ok`'s name came first; `ContextLogger`'s field-merge needs `[...base, ...extra]` array concatenation, which arrays don't have at all, so it reuses the exact double-reduce-then-map idiom from today's `http/client.ok` body reassembly; `MemoryLogger` has no explicit `close()`/`destroy` for its owned byte cell, matching `test/fake_clock.FakeClock`'s exact precedent (native handle table reclaims it at interpreter shutdown) |
| `db/transaction` | sixth and final Phase 7 module; deliberately declares only `role Transaction`/`InactiveTransaction`, no concrete implementation — transaction framing (`BEGIN`/`COMMIT`/`ROLLBACK` or a wire-protocol equivalent) is inherently database-specific, so every real implementer is a still-ahead Phase 9 driver; the checklist's prose contract ("destroying an active transaction rolls it back, never commits implicitly") can't be enforced by a bare role declaration, so it's demonstrated instead by this module's own embedded-test-only `FakeTransaction` fixture (not part of the public API) using the same owned-`runtime/memory`-cell instance-local-mutable-state idiom as `test/fake_clock`/`fs/temp`/`log/logger`, giving future drivers a concrete, tested pattern to mirror rather than only a prose description. **Phase 7 ("owned workflows") complete**: `fs/temp`, `proc/cmd`, `http/handler`, `http/client`, `log/logger`, `db/transaction` |
| `http/router` | `Router.response` dispatches routes whose targets are bound `async (Request) is Response` actions; method-specific objects cover the complete declared HTTP set: `GetRoute`, `HeadRoute`, `PostRoute`, `PutRoute`, `PatchRoute`, `DeleteRoute`, `OptionsRoute`, `ConnectRoute`, `TraceRoute`; the draft's unbuildable dynamic `%string:string` path parameters remain `@PathParameter`; `Route.response` never injects extracted parameters, so an endpoint needing them calls the same `PathPattern.parameters()` explicitly |
| `http/server` | second Phase 8 module, reverse side of `http/client`; plain TCP and currently sequential at runtime; `listen()` is the conventional blocking, unbounded accept loop, `async serve_one()` is the one-connection boundary, and the server accepts an `async (Request) is Response` handler action (normally `router.response`); the artificial request counter/limit and both former state implementations were removed; request parsing is iterative through operation-local buffers, eliminating prior per-byte/header/body recursion; handler errors become `500`, protocol failures remain `HttpServerFailure`; real concurrency still requires scheduler/task spawning/non-blocking sockets internally without changing `server.listen()` |
| `db/connection` | third and final Phase 8 module; same shape as `db/transaction` for the same reason — declares only `role Connection`/`ConnectionClosed`/`DbFailure`, no concrete implementation, since connection/protocol state is inherently database-specific; demonstrated by this module's own embedded-test-only `FixtureConnection`/`FixtureTransaction` fixtures (not part of the public API), reusing the same owned-`runtime/memory`-cell instance-local-mutable-state idiom as every other "needs mutable state in an immutable object" case this session. **Phase 8 ("composition layers") complete**: `http/router`, `http/server`, `db/connection` |
| `db/driver` | first Phase 9 ("application-level batteries") module; same role-and-errors-only shape as `db/connection`/`db/transaction` for the same reason — the real connection strategy (SQLite's file/locking protocol, or a real TCP/TLS wire-protocol session for Postgres/MySQL) is inherently driver-specific, so `role Driver { connect() is Connection; dialect() is Dialect; }` plus `DriverFailure` is all this module declares; demonstrated by this module's own embedded-test-only `FixtureDriver`/`FixtureConnection` fixtures (not part of the public API), which reuse the already-real `SqliteDialect` from `sql/dialect` rather than inventing a third fake `Dialect` object; drivers are composed by explicit import, no name-keyed registry or reflective/host-provided loading |
| `db/database` | second Phase 9 module, and unlike every other `db/*` module this session, a genuine concrete implementation rather than a role-only contract — `Database(driver is Driver)` is a plain state-only wrapper; `connect()`'s body is a single bare `try { target.connect(); } catch failure is DriverFailure { throw DatabaseUnavailable(failure.text()); }` used directly as the action's tail expression (no trailing `;` after the catch block's closing `}`, confirmed a bare `try`/`catch` can be a block's own tail expression, contributing the try-arm's value, exactly like `if`/`match`), translating the ONE failure type a `Driver` can throw into one stable `DatabaseUnavailable` so application code never depends on a specific driver's own failure type; `dialect()` delegates with no translation since dialect lookup can't itself fail; demonstrated by this module's own embedded-test-only `FixtureDriver`/`FixtureConnection`/`FailingDriver` fixtures (not part of the public API), where `FailingDriver` exists purely to exercise the `DriverFailure`→`DatabaseUnavailable` translation that `db/driver`'s own fixtures had no reason to cover |
| `db/sqlite`/`db/postgres`/`db/mysql` | explicitly deferred, raised directly and confirmed rather than attempted at reduced fidelity — a real, byte-for-byte-compatible SQLite driver (pager, B-tree with overflow pages, rollback journal/WAL, POSIX advisory locking) or a real Postgres/MySQL wire-protocol driver (startup/auth handshakes, prepared-statement framing, row decoding) is each individually a multi-week undertaking, categorically larger than every module implemented this session — those all wrapped existing OS/runtime primitives (sockets, TLS, filesystem syscalls) into typed vocabulary, none implemented a storage/query engine from first principles; critically, there is no structured-AST shortcut a driver could consume instead — `RawStatement`/`Insert`/`Update`/`Delete`/`Select` in `sql/statement`/`sql/query` all render down to plain SQL text via `.sql(dialect)`, so a real driver needs a genuine SQL tokenizer/parser/executor too; full design kept in this document (now marked deferred) and `docs/todo/db-sqlite-22072026.md` for a future pass |
| `db/migration` | fourth Phase 9 module, and — unlike `db/connection`/`db/transaction`/`db/driver` — a real, concrete `Migrations` implementation, not another role-only contract, composed against any `Connection`; constructor rejects two migrations sharing a `version()` as `DuplicateMigration` (validator made public, not private, same constructor-can't-call-private-actions reason as `http/router.PathPattern`); `apply()`/`rollback()` each run one migration's `up()`/`down()` statement list inside its own `connection.transaction()` (one transaction per migration, so a partial failure only rolls back the failing migration), leaning on `Transaction`'s own documented "destroying an active transaction rolls it back" contract instead of an explicit rollback call on error; `rollback(connection, count)` takes the tail of the applied-history array via `.slice(size-count, size)` then reverses it with the standard `reduce`+`prepended`+`map` idiom to undo most-recently-applied first; `ConnectionClosed`/`DbFailure`/`InactiveTransaction` are all caught at their call sites and re-thrown as one stable `MigrationFailure`, mirroring `db/database`'s `DriverFailure`→`DatabaseUnavailable` translation. Migration history is tracked entirely in the `Migrations` instance's own owned `runtime/memory` cell (one version per line, `log/logger.MemoryLogger`'s exact full-rewrite idiom) rather than read back from the database — confirmed as a hard wall, not a shortcut: this stdlib has no runtime role-to-concrete downcast (a queried `Row`'s `SqlValue` can never become an Okkama `string`/`int` here), and `sql/query`/`sql/statement` have zero `WHERE`-clause support today (`Select.predicate` is the unread placeholder `EveryRow`; `Delete` has no predicate at all) — so this is honestly documented as NOT durable across a process restart, deferred to whatever a real driver eventually does at the connection level. Also surfaced a genuine interpreter bug, found and root-caused via direct source reading (not fixed, matching the fs/dir O(n²) precedent of leaving interpreter-core changes for separate sign-off): `.map()` used as a bare action-body tail expression (no intervening `let`) throws `Functional map result at position N must be <first element's concrete type>, got <Nth element's concrete type>` even when the array is declared as a role type and every element legitimately implements that role — because `Okkama::Interpreter::execute_map_iterations` (`Interpreter.pm:2814`) only receives a target element type from `$expected_value_type`, which is deliberately reset to `undef` before every action body runs (`Interpreter.pm:1569`, with an explicit comment explaining why: the declared return type is checked only *after* the body produces its value, `:1622-1634`) — so `.map()` falls back to inferring its element type from the first result's own concrete runtime type instead of the declared role, exactly the same "one `let`/hint away from working" shape as the already-known `EmptyList()` generic-inference gap; worked around in `pending()` by binding through an explicit `let result is @Migration = ...map...; result;` before returning it |
| `test/suite` | final Phase 9 module and final module of this session's stdlib push; the draft named `Suite`/`SuiteResult`/`Fixture`/`TestFailure` but never a `TestCase`-equivalent, so `role TestCase { name() is string; run(); }` was added — same "draft implies an object without naming it" gap as `Migration` for `db/migration` and `Handler`'s chain-of-responsibility signal for `http/handler`; `Suite(tests is @TestCase, around is @Fixture, clock is Clock).run() is SuiteResult` executes independently of the language's OWN contextual `test` capability on purpose — `test/more.Assertions` (and bare `test.ok`/`.is`/`.fail`) only has that capability inside a real `test { }` block run by the language's own runner, calling it from ordinary runtime code (which is what `Suite.run()` does) fails explicitly per `test/more`'s own contract — so pass/fail here is purely exception-based: any thrown `Error` (not only the exported `TestFailure`) fails a case; confirmed directly from the grammar (`docs/okkama-grammar-1.0.ebnf`: `try_stmt = "try" block catch_clause+ ;`, `try_expr` identical) that this language has **no `finally` clause** at all despite `FINALLY` existing as a reserved-but-unused lexer token, and confirmed zero stdlib usage (`grep -rn finally stdlib/` empty) — worked around by wrapping every fixture's `before()` plus the case's own `run()` in one `try`/`catch` that fully absorbs any `Error` into a plain `bool` (never re-thrown), so the `for fixtures -> fixture { fixture.after(); }` loop immediately after is reached unconditionally in both outcomes, no `finally` needed; corrected two stale draft dependency lines (`test/more`, `log/logger` — neither actually used, `Suite` never touches the contextual test capability and has no logging hook) to the real ones (`time/clock`, `time/duration`, `time/instant`); this module's own tests inject `test/fake_clock.FakeClock` (test-only import, `SuiteResult.duration()` asserted as an exact `Duration(0)` rather than tolerated as "greater than zero", since nothing in the test cases/fixtures ever advances the clock) — completes Phase 9 alongside `db/driver`/`db/database`/`db/migration`, with `db/sqlite`/`db/postgres`/`db/mysql` explicitly deferred (see the `db/sqlite` entry above) |
| `random` | complete `RandomSource`; secure `Random`, pure non-cryptographic `SeededRandom(seed)`, inclusive unbiased signed-64 ranges, 53-bit `[0,1)` fractions, explicitly injectable generic `value Choice(...)`/`values Shuffled(...)`, typed reversed/empty-range failure; constructors are state-only and seeded queries have no hidden cursor |
| `os` | state-only `Host`/`Posix`/`Windows` objects implementing `OperatingSystem`; closed platform, architecture and path-style enums; exact LF/CRLF and path-list conventions; unknown hosts refuse convention guesses |
| `fs/path` | pure state-only `PathValue` objects: host-selected `Path` plus explicit `PosixPath`/`WindowsPath`; immutable lexical join/normalize/parent, exact UTF-8 bytes, POSIX/drive/root-relative/UNC grammar and typed invalid/missing-parent failures; no filesystem access |
| `fs/meta` | `Meta(path is PathValue)` reads through `lstat` (not `stat` — a followed symlink can never report `FileKind.link`); `enum FileKind = (file, directory, link, other)`, all non-file/dir/link raw stat kinds (device/fifo/socket/unrecognized) collapse to `other`; `modified()` is `Instant` at `ClockIdentity.system` from `modified_sec`; `Permissions(mode is int)` decodes only owner rwx bits (`value % (weight*2) / weight`, this arithmetic implementation predates bitwise operators; it can now use `&` masks); only a `not_found` lstat failure is catchable `MissingPath` (caller-supplied path, retriable), any other native failure panics since fs/meta declares no broader error vocabulary |
| `time/duration` | pure-Okkama state-only signed-64 `Duration` and named unit projections; exact checked plus/minus/times/order; typed overflow without float, wrap or saturation |
| `time/instant` | exact `Instant(nanoseconds, ClockIdentity)` values; identity-preserving shifts and typed rejection of system/monotonic mixing |
| `time/clock` | `Clock` role with state-only `SystemClock`, `MonotonicClock`, and composed `OffsetClock`; native access only in actions and typed invalid relative sleep |
| `time/date` | pure-Okkama proleptic Gregorian `Date` plus exact UTC `DateTime`; initial UTC-only zone policy, no implicit host tzdata |
| `time/timer` | state-only schedule projections acquiring owned `TimerHandle` resources; one-shot/fixed-rate periodic wait, command/query cancellation, deterministic skip-missed policy |
| `env/var` | nominal `EnvVar`, raw/text `EnvEntry`, ambient `Env` and isolated `Environment.fake()`; missing differs from empty and text projection is strict UTF-8 |
| `env/cwd` | `role WorkingDirectory { path() is Path; change(path is Path); }`; `Cwd` stateless, always re-reads host `cwd()`/`chdir` (never caches); `change()` failure (missing target, permission, ...) sanitizes to one `InvalidWorkingDirectory`, matching `crypto/random`'s translation; `path()` failure is instead an invariant panic (`Filesystem provider could not read the working directory`), same treatment as `SystemClock.now()`, since no caller input is at fault; `FixedCwd(start)` is immutable — `change()` always throws `InvalidWorkingDirectory`, doubling as a way to assert a code path never changes directories; not a mutable fake like `FakeClock` |
| `proc/args` | stateless `Args` over runtime-owned immutable argv; exact `@@byte`, strict indexed UTF-8, `InvalidArgumentEncoding` and `MissingArgument` |
| `net/address` | state-only constructors plus action-computed immutable `IpAddress`/`SocketAddress`/unresolved `HostPort`; canonical IPv4/IPv6, exact 4/16 public octets, bracketed IPv6 endpoints, ports `0..65535`, typed sanitized failures, no public runtime envelope |
| `test/more` | stateless reusable `Assertions.ok/is/isnt/fails` over the contextual test capability; boxed assertion equality; state-only `AssertionFailure` Error value |
| `test/fake_clock` | `FakeClock(start is Instant)` implements `Clock`; `sleep` is `advance` renamed (never blocks, still throws `InvalidSleepDuration` on a negative duration); `advance` accepts a signed `Duration` (test-only, may rewind); `set` throws `IncompatibleClocks` on identity mismatch; current instant lives in an owned `runtime/memory` byte cell (fields cannot be reassigned outside a constructor) via `encoding/binary`'s big-endian codec; module path is `fake_clock`, not `fake-clock` — `-` is not valid in an Okkama identifier |
| `test/benchmark` | `role BenchmarkCase { run(); }`; `Benchmark(subject is BenchmarkCase, clock is Clock, iterations is int).measure() is BenchmarkResult` times each run against the injected `Clock` (inject `test/fake_clock` for exact statistics instead of real timing noise); `BenchmarkResult(values is @Duration)` is independently constructible, `iterations/total/average/minimum/maximum`; `average()` uses the same non-float-lossy `((a - a % b) / b).int()` idiom as `encoding/hex`/`encoding/base64` rather than plain `/` (always `float` in this interpreter); `InvalidBenchmark` covers non-positive `iterations` and a run whose clock ends earlier than it started (real path via a rewinding `FakeClock` case); `case` is a reserved keyword so the field/param is `subject` |
| `io/input` | `Input`/`StringInput` capability roles; `EndOfInput`, `InvalidReadSize` |
| `io/output` | `Output`/`StringOutput` capability roles; `IncompleteWrite` with `.written()` |
| `io/closeable` | independent `Closeable` lifecycle capability for owned resources |
| `io/buffer` | explicit owned `Buffer`/`StringBuffer` acquisition plus non-owning cached `BufferedInput`/`BufferedOutput`; exact partial-write suffix retention |
| `io/stdin` | strict UTF-8 `Stdin.line()` and raw `StdinBytes.bytes(limit)` |
| `io/stdout` | formatted `Stdout.write/writeln` and raw `StdoutBytes.write` commands |
| `io/stderr` | formatted `Stderr.write/writeln` and raw `StderrBytes.write` commands |
| `regex` | state-only `Regex(pattern)` with `matches`/`contains`/`first`/`all`/literal `replaced`, detached `Match` values, exact-offset `InvalidRegex` and `MissingMatch`; operation-scoped owning handles over the portable Thompson NFA |
| `runtime/time` | `wall_clock()` (Unix ns), `monotonic_clock()` (ordering ns), `sleep(ns)` |
| `runtime/fs` | `@byte` paths + fallible `path_encode`/`path_decode`, binary open/read/write/seek/flush/truncate/sync/advisory locks/close over `FileHandle`, stat/lstat, mkdir/unlink/rename/links, cwd, chmod/chown, directory iteration |
| `runtime/process` | shell-free `spawn` and `spawn_configured` (explicit cwd and replacement environment), pipes, `wait`/`try_wait`, `kill`, argv/pids |
| `runtime/net` | blocking IPv4/IPv6 sockets, `@byte` envelopes, DNS resolve/reverse, ip parse/format |
| `runtime/poll` | readiness polling over file/pipe/socket handles, monotonic deadlines |
| `runtime/environment` | raw-`@byte` env get/set/remove/all |
| `runtime/system` | platform/arch strings, `secure_random(size)` (OS CSPRNG) |
| `runtime/debug` | `data_dump(any) is string` (backs `Dump`) |
| `runtime/regex` | `compile`/`invalid_offset`/`contains`/`matches`/`spans`/`all_spans`/`close` over `RegexHandle`; spans are leftmost-longest Unicode code-point ranges with ordered captures |
| `runtime/format` | privileged rendering of opaque `any`: concatenation, positional `{}` formatting, named `{name}` formatting and brace escaping |
| `runtime/crypto` | SHA-256/384/512 digests and HMAC over exact bytes plus constant-work `secure_equal` |
| `runtime/tls` | verified TLS client/server sessions over consumed connected `SocketHandle` capabilities, encrypted byte I/O and fixed peer-certificate projection |
| `runtime/stdio` | raw standard-stream bytes: shared-buffer `stdin_read`/`stdin_read_line`, distinct `stdout_write`/`stderr_write` |
| `runtime/memory` | owned generic byte state: snapshot/size/consume/append/reset/lifecycle only; no line or I/O buffering policy |
| `runtime/utf8` | strict `encode`/`decode` plus `valid_prefix(@byte)`, which returns size for valid input or the first ill-formed sequence's byte offset |

Use `Host()` for the current system and `Posix(platform, architecture)` or
`Windows(architecture)` for an explicit foreign/test description. Their
constructors only retain state; identity lookup and validation happen in
actions. Known Windows means `PathStyle.windows`, CRLF and `;`; every known
non-Windows platform means `PathStyle.posix`, LF and `:`. `Platform.unknown`
remains observable on `Host`, but convention actions throw
`UnsupportedPlatform` rather than silently selecting POSIX.

Use `Path(value)` for the current host and `PosixPath(value)` or
`WindowsPath(value)` for an explicit foreign grammar. Constructors only retain
text. `string()` preserves valid source, `bytes()` returns exact UTF-8, and
only `normalized()` canonicalizes separators plus lexical `.`/`..` without
I/O. `joined(part)` requires a relative fragment and never replaces its base;
`parent()` works from the normalized value and throws `MissingPathParent` at a
root/current context. Windows distinguishes drive-rooted, drive-relative,
root-relative and complete UNC paths. Unknown host `Path` actions propagate
`UnsupportedPlatform`; explicit path kinds never inspect the host.

Use `duration Nanoseconds(n)`, `duration Milliseconds(n)`, `duration Seconds(n)`, `duration Minutes(n)`, or
`duration Hours(n)` to project one of the state-only unit objects into `Duration`.
Durations are signed exact counts in
`-9223372036854775808..9223372036854775807` nanoseconds. Negative spans are
valid. `plus`, `minus`, `times`, and named-unit scaling throw
`InvalidDuration` on overflow before any integer operation can wrap; they
never convert through `float`. The module has no native import and no implicit
sleep behavior.

Use `Instant(value, ClockIdentity.system)` only for Unix wall time and
`ClockIdentity.monotonic` for measurement/deadlines. Shifts preserve identity;
`since`/`less` throw `IncompatibleClocks` across identities. `SystemClock` and
`MonotonicClock` implement `Clock`; `OffsetClock(source, offset)` decorates the
source and delegates relative sleep unchanged. Negative durations remain valid
values, but `Clock.sleep` throws `InvalidSleepDuration` for an invalid relative
deadline.

`Date(year, month, day)` is Gregorian in years `1..9999`, Monday-first, and
renders `YYYY-MM-DD`. `TimeZone("UTC").at(system_instant)` yields a UTC
`DateTime` while preserving the exact source instant. No other zone name and
no monotonic instant is accepted yet.

Timers are live resource identities. Never model cancellation as a successor
or invent `cancel() is bool`. Acquire explicitly with
`timer ScheduledTimer(duration)` or `timer ScheduledPeriodicTimer(interval)`. Use
`cancel()` as an idempotent command and `cancelled()` as the bool query.
Waiting after cancellation throws `CancelledTimer`; periodic waits are
fixed-rate and skip missed ticks. Resource destruction owns `TimerHandle`
close.

Use `Env()` as the ambient process capability and always pass nominal
`EnvVar` names:

```okkama
let environment is Environment = Env();
let variable = EnvVar("HOME");
if environment.has(variable) {
    environment.value(variable); // strict UTF-8
    environment.bytes(variable); // exact bytes
}
```

`EnvVar`/`EnvEntry` constructors are state-only. Names are nonempty UTF-8
without NUL or `=` when observed. Values may be empty or arbitrary non-NUL
bytes. `value()` and `EnvEntry.string()` throw `InvalidEnvEncoding` rather
than replacing malformed UTF-8. Missing uses `MissingEnvVar` and is distinct
from empty. `variables()` returns detached byte-sorted entries.
`Environment.fake()` is test-mode-only, instance-local and never mutates the
host environment.

Use `Args()` for application argv. `bytes()` returns exact `@@byte`, while
`strings()` and `at(index)` decode strictly as UTF-8. A malformed value throws
`InvalidArgumentEncoding` with its zero-based argument index; negative or
upper-bound access throws `MissingArgument` with the requested index. Empty
arguments remain present. The CLI source path is excluded, and `Args()` does
no constructor-time native work because `runtime/process` already owns the
defensive immutable snapshot.

Regex patterns are ordinary strings; never generate a regex literal:

```okkama
let words = Regex("([A-Za-z]+)");
let first is Match = words.first("Hi, Ada");

first.string();                  // "Hi"
first.groups();                  // ["Hi"]
words.all("Hi, Ada");           // @Match, ordered and non-overlapping
words.replaced("Hi, Ada", "_"); // "_, _"
```

`Regex(pattern)` performs no compilation. Each action owns and closes one
private prepared handle. `first` throws `MissingMatch`; malformed syntax
throws `InvalidRegex` with an exact Unicode source offset at action time.
Unmatched groups are `""` in `Match.groups()`. Replacement is global and
literal, with no `$1`, backslash or brace expansion. Zero-length matches
advance one code point and include the terminal boundary exactly once.

High-level formatting always uses projections. Positional values are passed
directly; named values use an explicit heterogeneous map:

```okkama
let named is %string:any = ["name": "Ada", "count": 3];

string Formatted("{}: {}", "Ada", 3);            // "Ada: 3"
string NamedFormatted("{name}: {count}", named);  // "Ada: 3"
```

Positional fields are exactly `{}`; named fields match ASCII
`[A-Za-z_][A-Za-z0-9_]*`; `{{`/`}}` emit braces. Extra supplied values are
allowed. The variadic positional tail is retained as private `@any` state;
callers do not create an auxiliary array. Malformed syntax throws
`InvalidFormat` at the first zero-based
Unicode code-point offset; missing values throw `MissingFormatValue` naming
the index or key. Constructors perform no parsing or native calls.

Plain templates use a separate closed lexical grammar:

```okkama
let values = TemplateValues(TemplateBinding("name", "Ada"));
Template("Hello, <%= name %>!").string(values);
```

`<%= name %>` and `<%== name %>` both preserve exact strings under the plain
identity-escaping policy; `<%# ... %>` is a non-nesting comment and `<%%`
emits literal `<%`. Every other `<% ... %>` form is reserved and invalid in
v1, never `eval`. Placeholder names are ASCII identifiers with optional
horizontal ASCII space. The whole source is validated before lookup, output is non-recursive,
extra bindings are allowed, and missing/duplicate names are nominal errors.
All constructors are state-only and the module is pure Okkama.

Boundary invariants: raw bytes (no implicit text decoding), single-attempt I/O
with visible partial progress (empty read = EOF), idempotent close, no host
constants or errno leaking.

The pure-Okkama I/O API follows Elegant Objects naming. Queries name returned
values: `Input.bytes(limit)`, `StringInput.line()`, `IncompleteWrite.written()`
and `Closeable.closed()`. Commands use verbs and expose no service result:
`write`, `writeln`, `flush`, and `close`. `bytes` requires a positive maximum;
empty bytes mean EOF. `line()` throws `EndOfInput` only when no bytes remain.
`StringOutput.write(values...)` concatenates display forms and `writeln`
appends one LF. `Stdin`/`Stdout`/`Stderr` are borrowed and not closeable;
owned resources compose the separate `Closeable` role. All roles have
deterministic fakes, and only `Closeable.fake()` uses identity-local storage
for lifecycle transitions.

`runtime/stdio` is the procedural byte ABI: `stdin_read(limit)`,
`stdin_read_line()`, `stdout_write(bytes)`, and `stderr_write(bytes)`. The two
stdin functions share one provider buffer. Native write counts are checked by
the adapters and never become public object results.

`io/buffer` acquires live storage explicitly: `buffer BufferAllocation(...)` and
`buffer StringBufferAllocation(...)` yield owned buffers, while
`input InputBuffering(source, capacity)` and `output OutputBuffering(target, capacity)`
yield decorators that own only their private cache. `Buffer.bytes(limit)`
consumes and `Buffer.bytes()` snapshots. String lines, UTF-8, prefetch,
threshold draining, flush and exact partial-write retry are Okkama policies;
`runtime/memory` supplies only authority-checked generic byte state.

## 24. Diagnostics

Runtime errors stringify as `Runtime error at line N: message`. Examples:

```text
Runtime error at line 4: Undefined variable "missing"
Runtime error at line 7: Object "Box" has no action "missing"
Object "Cash" field "cents" was not initialized
Action "secret" on object "Cash" is private
Successor action result must be used
```

No source snippets/carets/file paths yet. Declaration- and context-dependent
errors surface from the semantic analyzer before execution; value-dependent
type errors remain runtime diagnostics.

## 25. Implementation status

**Runtime-supported (safe to generate):** modules/imports (plain, `take`,
`as`, basename namespaces, module-qualified nominal identity, multi-file
modules with file-local imports), all literals/operators/strict equality, arrays/maps/
ranges + `else` fallback, `let`/blocks/`if`/`match`/`for` (including nominal
`Iterable<T>` sources)/`next`/`last`/
postfix `if`, `confirm`/`throw`/`panic`/`try`/`catch`, `return`, `defer`, objects (full/
shorthand/variadic constructors, defaults, named constructor projections),
actions (overloads, named args, private sections, variadics + `values...`), successors with
must-use, `destroy` + identity ownership, strict lexical scope, enums, roles +
fakes (test mode), exact generics + inference, `byte`/`@byte`, `any` boxing,
builtin value behavior (including literal `split`/`join`/`replace`), bundled
`strings/parse`, `collections/iteration`, iterable persistent
`collections/list`/`queue`/`stack`, `format`, `encoding/utf8`, `encoding/binary`,
`encoding/base64`, `encoding/hex`, `encoding/csv`, `encoding/toml`, `crypto/random`, `crypto/digest`, `crypto/hmac`, `socket/socket`, `random`, `os`, `fs/path`, `time/duration`, `time/instant`, `time/clock`, `time/date`, `time/timer`,
`net/address`, `test/more`, `test/fake_clock`, `test/benchmark`, `env/cwd`, `fs/meta`, and
`io/input`/`io/output` protocols, `io/buffer`, and `template/plain`/`template/html`,
`sql/value`, `http/status`, `http/headers`, `log/event`, `db/result`, `db/row`, `fs/file`, `fs/dir`, `socket/tcp`, `socket/udp`, `socket/unix`, `crypto/tls`, `sql/dialect`, `sql/statement`, `sql/query`, `http/request`, `http/response`, `log/formatter`, `fs/temp`, `proc/cmd`, `http/handler`, `http/client`, `log/logger`, `db/transaction`, `http/router`, `http/server`, `db/connection`, `db/driver`, `db/database`, `db/migration`, `test/suite`,
`map`/`filter`/`reduce`, `async`/`await`, native
substrate (`runtime/*`), `okkama.Dump`, `regex.Regex`, test runner (`-t`).

**Removed syntax (rejected):** `allow` access grants. Private fields are owner-only;
collaborators call public behavior. See `docs/allow-removal.md`.

**Parses but NOT runtime-supported (do not generate):** decimal literals,
alias (`as`) imports.

**Planned only (do not rely on):** exact collection type inference for
untyped literals, explicit `any` narrowing, explicit numeric casts, string
case helpers, match completeness, CLI runner modes.

## 26. Translation table — habits from other languages

| You want to write | Write in Okkama instead |
|---|---|
| `return x;` | `return x;` (one value), or implicit final `x;` |
| `x = null` / `Optional<T>` | `else` fallback, typed throw, or a role-based design |
| `class B extends A` | `object B does RoleA { has a is RoleA; ... }` (compose + decorate) |
| `obj.field` (read from outside) | add an action: `obj.field_value()` — behavior, not state |
| `obj.field = v` | compose the complete desired state, or expose a genuine domain transition named by its result; never publish a field copy-setter |
| `if (x)` with non-bool `x` | produce a real `bool`: `if x.size() > 0 { }` |
| `while (cond) { }` | `loop { last if !cond; ... }` |
| `{"key": 1}` | `["key": 1]` — brackets, quoted string keys |
| `f"count={n}"` / `"a" + 1` | build strings explicitly; no interpolation, no implicit stringification |
| `xs.push(v)` | arrays are immutable — build a new array or wrap in an object |
| `lambda x: ...` passed around | `(x is T) is R { ... }` with an exact declared signature |
| `throw new Error("msg")` | `throw ErrorObject(context: value);` — nominal `Error` object |
| `finally { cleanup() }` | `destroy` on an owning object (guaranteed) or `defer` |
| mocks in tests | `Role.fake()` — the role ships its own double |
| `Box<int>(...)` explicit generic call | plain construction; specialization is inferred |
| `1 == 1.0` cross-type compare | compare same categories; convert explicitly at design level |

## 27. Canonical example (all core features)

```okkama
module app v1.0.0;

import "okkama" take Dump;

role Wallet { total() is int; }

enum Status = (ready, failed, done);

object InvalidCents does Error {
    has cents is int;
    InvalidCents(:cents);
    cents_value() is int { cents; }
    text() is string { "invalid cents"; }
}

object Purse does Wallet {
    has digit is int;
    has cents is int = 0;

    Purse(digit is int, cents is int) {
        digit = digit;
        cents = cents;

        confirm self.cents >= 0 && self.cents < 100 else throw InvalidCents(cents: self.cents);
    }
    Purse(:digit){
        confirm self.cents >= 0 && self.cents < 100 else throw InvalidCents(cents: self.cents);
    }



    total() is int { digit * 100 + cents; }
    touch() is Purse { self; }                    // returns self — chainable

    spend() is ^Purse {                  // immutable transition
        ^cents = 0;
    }

    destroy { digit; }                   // deterministic scope cleanup

    private:
    fee() is int { 10; }                 // self.fee() only
}

main() is int {
    let purse is Purse = Purse(digit: 7, cents: 65);
    let prices is %string:int = ["apple": 120];
    let bytes is @byte = [byte(0), byte(255)];
    let boxed is any = 42;

    let spent = purse.spend();           // successor MUST be used

    for 1..=3 -> n { n; }

    let doubled = [1, 2, 3].map() -> v { v * 2; };
    let sum = doubled.reduce(0) -> acc, v { acc + v; };
    Dump(doubled).string();              // "[2, 4, 6]"

    try {
        confirm prices.has("fig") else throw InvalidCents(cents: 0);
    }
    catch error is InvalidCents {
        error.cents_value();
    }

    prices["fig"] else 0;                // explicit fallback — no null, ever
}
```
