# The Okkama Book


**A complete guide to the Okkama programming language**

Edition: 19 July 2026 — Okkama 1.0 implementation line
Companion documents: `docs/okkama-spec-11072026.md` (normative specification),
`docs/okkama-grammar-1.0.md` (EBNF grammar), `docs/okkama-llm-context-1.0.md`
(code-generation context pack).

This book is a textbook. It teaches the language from first principles,
explains *why* Okkama is shaped the way it is, and walks through every part of
the language: its philosophy, types, objects, roles, and constructs. Where the
book and the specification disagree, the specification and the test suite
(`okkama-pl/t/`) are the source of truth.

## Rust toolchain commands

```sh
okkama run test.ok
okkama build test.ok --target jvm -o out.class
okkama build test.ok --target sparrow -o out.spb
sparrow out.spb
```

`run` executes source with the Rust interpreter. `build` emits the selected
bytecode; standalone Sparrow executes `.spb`. `--backend` remains an alias
for `--target`. `sparrow check out.spb` verifies without execution.
See the [Rust implementation guide](../okkama-rs/README.md) for building the
tools and current runtime coverage.

---

## Table of Contents

**Part I — Philosophy and First Steps**
1. Why Okkama
2. A First Program
3. Lexical Structure

**Part II — Values and Types**
4. The Type System at a Glance
5. Numbers: `int`, `float`, `rat`, and `byte`
6. Strings and Characters
7. Booleans and the Absence of Null
8. Arrays, Maps, and Ranges
9. The Top Type `any`
10. Builtin Value Behavior

**Part III — Expressions and Statements**
11. Expressions
12. Statements and Control Flow
13. Functional Collection Operations
14. Failure: `throw`, `panic`, `try`/`catch`
15. `defer`

**Part IV — Objects**
16. Objects and Fields
17. Constructors
18. Actions
19. Successor Actions and Immutability
20. Ownership, Scope, and `destroy`
21. Lexical Scope Boundaries

**Part V — Abstraction**
22. Roles
23. Role Fakes and Testing
24. Enums
25. Generic Type Parameters

**Part VI — Programs in the Large**
26. Modules and Imports
27. The Native Runtime Boundary (Overview)
28. Debugging and Diagnostics
29. What Okkama Deliberately Leaves Out
30. Design Patterns: Composition over Inheritance
31. The Road Ahead: Designed and Planned Features

**Appendices**
A. Reserved and Removed Words
B. Implementation Status
C. Quick Reference Card

---

# Part I — Philosophy and First Steps

## Chapter 1. Why Okkama

Okkama is a statically typed object language built around four ideas:

```text
Objects expose behavior, not state.
Object state is declared with has.
Objects are immutable after construction.
State changes are represented by successor objects.
```

If you have read *Elegant Objects* by Yegor Bugayenko, this list will feel
familiar. Okkama is not merely *influenced* by Elegant Objects — it takes the
EO discipline that in Java or C# must be enforced by convention and code
review, and turns it into language rules that the runtime checks for you.

### 1.1 The Elegant Objects heritage

The mapping between EO principles and Okkama language features is direct:

| Elegant Objects principle | Okkama language rule |
|---|---|
| Never use `null` | There is no `null`, `none`, or `nil`. The words are reserved errors. |
| No getters and setters | Fields are private, always. External field access is a runtime error. There is no setter syntax at all. |
| Objects are immutable | Fields can only be bound during construction. After that, no mutation. |
| State change returns a new object | Successor actions (`is ^Type`) return a new object with staged updates. |
| No inheritance | There is no `extends`, `isa`, `super`, `abstract`, or `protected`. Composition and decoration are the tools. |
| Interfaces everywhere | Roles are nominal contracts, implemented explicitly with `does`. |
| Fake objects instead of mocks | Roles carry their own test-only `fake` block, so every contract ships with its reference test double. |
| Constructors bind and validate | Constructors bind fields and check invariants with `confirm ... else throw`. |
| Fail fast | `confirm` asserts invariants; failure without `else` panics, while `else throw` raises a typed error. There is no partial or "best effort" result. |
| Objects manage resources | `destroy` blocks run deterministically when an owning scope exits — not when a garbage collector feels like it. |

The consequence is a language where the "right" object-oriented style is not a
matter of taste; it is the only style the compiler and runtime accept.

### 1.2 What Okkama refuses to have

Okkama is defined as much by what it removes as by what it provides:

- **No null.** A missing value is a design error, not a runtime state. Where
  other languages return null, Okkama panics, or the API is designed so the
  question does not arise (for example, map indexing takes an explicit
  `else` fallback).
- **No truthiness.** Conditions require real `bool` values. `if 1 { ... }` is
  a runtime error, not a convenience.
- **No inheritance.** Behavior is shared through roles (contracts) and
  composition (holding other objects in fields), never through parent classes.
- **No implicit conversions.** `1 == 1.0` is an error: `int` and `float` are
  distinct runtime categories. `byte(1) == 1` is an error too. Numeric
  widening happens only where arithmetic explicitly defines promotion.
- **No mutable variables on objects.** `has` is the only way to declare object
  state, and it is fixed at construction.
- **No hidden state changes.** An action that "changes" state must say so in
  its signature (`is ^Type`) and the caller must use the result. Discarding a
  successor is an error.

### 1.3 The current implementation

The reference implementation, `okkama-pl`, is an experimental tree-walking
interpreter written in modern Perl (5.42, using the native `class` feature).
It is a complete lexer → parser → static analyzer → interpreter pipeline with
an extensive test suite (`okkama-pl/t/lexer`, `t/parser`, `t/interpreter`,
`t/suite`). The analyzer expands imports, registers declarations, and rejects
everything decidable from source alone — undeclared types, duplicate names,
role conformance, misplaced context-sensitive statements — before any code
runs (Chapter 28.2); the interpreter keeps only value-dependent checks. A
native provider layer connects the language to the host operating system
through a strictly portable ABI (Chapter 27).

When an embedder reuses Analyzer, each independent analysis starts a fresh body
traversal. A previous semantic error does not disable successor must-use in the
next source. A published semantic model may outlive its analyzer; discarded
intermediate models must not stay alive through its walkers.


Run a program with:

```sh
okkama-pl/bin/okkama program.ok [program arguments...]
okkama-pl/bin/okkama toaster/pets/*.ok [program arguments...]
```

When several existing `.ok` paths are supplied before the program arguments,
they form one module. Every file is parsed separately, so imports remain
file-local, while object, role, enum, native, and `main` declarations share one
module declaration space. All files must declare exactly the same module name
and version, and the combined module may declare at most one `main`.

---

## Chapter 2. A First Program

### 2.1 Modules and `main`

Every Okkama source file starts with a module declaration, and every
executable module declares `main`:

```okkama
module app v1.0.0;

main() is int {
    42;
}
```

Rules to remember:

- The `module` declaration must be the first thing in the file.
- The module name is one or more lowercase identifiers separated by `/`
  (`app`, `money/cash`).
- The version is optional (`module app;`); when present it must be spelled
  `vMAJOR.MINOR.PATCH`. Modules of a blade project declare none: their
  version is set in `blade.toml`.
- Executable modules must declare `main()`. Library modules may omit it.
- Only *declarations* are allowed at module scope: objects, roles, enums,
  native declarations, actions, and test blocks. There are no top-level
  executable statements.

The entry point must be exactly `main() is int` or `async main() is int`,
with no parameters. Its result is the process exit status: `0` means success,
and `1..255` are application-defined error codes. Use `return 0;` or a final
`0;` for successful completion. The CLI does not print the result.

The ordinary implicit last-statement result still works: the example above
exits with status `42`. Missing results, other value types and integers outside
`0..255` are errors; there is no automatic zero or byte truncation.
An explicit return, including one inside a loop, still runs the reached
`defer` expressions and owned-object `destroy` blocks. The CLI exits only after
task and native-resource shutdown; cleanup failures are reported as failures.
The `-t` runner does not execute `main` and keeps its own test status.
An ordinary object method named `main` can declare another result type.

Actions may use `return expression;` to exit early. Without it, the last
statement's value remains the result in actions, `main`, branches and blocks.

### 2.2 A real program

```okkama
module app v1.0.0;

object Bottle {
    has opened is bool = false;

    Bottle() {}

    open() is ^Bottle {
        confirm !opened;
        ^opened = true;
    }

    is_open() is bool {
        opened;
    }
}

main() is int {
    let bottle = Bottle();
    let opened = bottle.open();
    opened.is_open();   // true
    return 0;
}
```

Everything essential about Okkama is already visible here:

- `has opened is bool = false;` declares private state with a default.
- `Bottle() {}` — every object must declare at least one explicit
  constructor; nothing is generated from fields.
- `open() is ^Bottle` declares a **successor action**: it does not mutate the
  bottle, it returns a *new* `Bottle` whose `opened` field is `true`.
- `confirm !opened;` fails fast if the bottle is already open.
- `bottle` itself is never changed. `bottle.is_open()` would still be `false`.

### 2.3 Comments

Okkama supports line comments and (non-nesting) block comments:

```okkama
// a line comment runs to the end of the line

/* a block comment
   may span lines */
```

---

## Chapter 3. Lexical Structure

### 3.1 Identifiers

Identifier rules depend on their syntactic position; names are case-sensitive:

- **Local bindings** (`let`, `for`, `catch`) accept `[A-Za-z_][A-Za-z0-9_]*`:
  `let USER_NAME="sdfdf";` is valid, as are `A` and `UserName`. Uppercase
  spelling does not create a constant or change binding behavior.
- **Lowercase identifiers** (`name`, `total_cents`, `_private_local`) name
  fields, parameters, actions, and module path segments. The
  pattern is `[a-z_][a-zA-Z0-9_]*`.
- **Type names** (`Cash`, `Bottle`, `InvoiceStatus`) start with an uppercase
  letter: `[A-Z][a-zA-Z0-9_]*`. They name objects, roles, enums, generic
  parameters, and native object types.

Reserved keywords cannot be used as ordinary identifiers even when their
lexeme has a valid lowercase shape. Module path segments and action labels are
contextual namespaces, so unambiguous forms such as `module test/more`, action
`is(...)`, `assertions.is(...)`, and action `has(...)` are valid. In an object
body, `has(` starts that action while `has field is T;` declares state.
Built-in forms such as
`value.last()`, `test.ok()`, and `Role.fake()` follow the same contextual rule.
Removed and deferred reserved words remain forbidden everywhere.

Action names have one additional guard: `with` and every `with_*` spelling
are forbidden in declarations and member calls. `withdrawal` is unrelated and
remains legal. This blocks the conventional copy-setter API at the source
boundary; fluent builders disguised as other verbs are rejected during API
design review.

### 3.2 Literals

The runtime-supported literals are:

````okkama
10          // int
3.14        // float
"hello"     // string
```raw "text" \n remains raw``` // string
'x'         // char
true        // bool
false       // bool
````

String literals support the escape sequences `\n` (newline), `\r` (carriage
return), `\t` (tab), `\"` (double quote), `\\` (backslash) and `\u{H}` — the
Unicode scalar value with 1 to 6 hexadecimal digits, such as `"\u{1b}[1m"` or
`"\u{e9}"`. Character literals support `\n`, `\r`, `\t`, `\'`, `\\` and
`\u{H}`, and must contain exactly
one character *after* escape decoding. An escaped quote does not terminate its literal. Unknown escapes and
a trailing lone backslash are lexer errors.

Three backticks provide a fenced raw spelling for the same `string` type:

````okkama
let document = ```
{
    "name": "Ada",
    "escaped-looking": "\n"
}
```;
````

Everything between the delimiters is retained exactly. Quotes, backslashes,
comment-looking text, indentation and newlines have no special treatment.
There is no interpolation, escape processing, dedenting, trimming or fence
modifier. The first following three-backtick sequence closes the literal, so
that exact sequence cannot occur inside this initial form. One or two
backticks outside a fence and a missing closing fence are lexer errors.

A decimal token type exists in the grammar, but decimal runtime semantics are
deliberately deferred; do not rely on them.

There is **no byte literal** — bytes are constructed with the checked call
`byte(value)` (Chapter 5) — and **no regex literal**: portable regular
expressions are implemented as an ordinary stdlib object over plain string
patterns (`Regex("[a-z]+")`, Chapter 31.3), never as lexer syntax.

### 3.3 Reserved and removed words

The following words are not valid Okkama 1.0 syntax and exist only so the
parser can produce clear errors:

```text
null none nil optional ?. isa super abstract protected with setters mutable var
```

Additionally, every action/member spelling matching `with_*` is a parse error;
this is an action-name rule rather than a general identifier reservation.

`destroy` is *not* reserved in this sense — it is the object lifecycle block
keyword. Appendix A has the full picture.

---

# Part II — Values and Types

## Chapter 4. The Type System at a Glance

Okkama is statically typed with **nominal** typing for objects and roles, and
strict runtime category checking for built-in values. The public type
spellings you can write in source are:

```okkama
int          // integer
byte         // integer restricted to 0..255, a distinct category
float        // floating point
string       // Unicode text
char         // a single character
bool         // true / false
any          // the explicit top type (opaque transport)
@int         // array of int         (@T in general)
@byte        // array of byte — the raw binary sequence type
%string:int  // map from string to int (%K:V in general)
Cash         // a concrete object type
Package      // a role type
Box<int>     // an applied generic type
^Cash        // a successor type: "returns a new Cash"
```

Types appear at *typed boundaries*: `let x is T = ...`, `has field is T`,
constructor and action parameters, action return types, and `catch` clauses.
The static analyzer validates every annotation *reference* before execution
(undeclared type names and wrong generic arity are semantic errors, even in
code that never runs). At each boundary the runtime then validates the actual
value — recursively, for collections; value–type mismatches are precise
runtime errors because values, overload selection, and generic inference are
dynamic.

Key properties of the type system:

- **No implicit numeric unification.** `int`, `float`, `rat`, and `byte` are
  distinct categories. Nothing ever silently converts between them.
- **No subtype hierarchy between objects.** A concrete object type is
  compatible with a role type only when the object explicitly declares
  `does Role`.
- **`any` is a boundary, not an escape hatch.** Values crossing into `any`
  become opaque (Chapter 9).
- **Inference is local and exact.** Untyped `let x = expr;` gives `x` the
  exact runtime type of `expr`. Generic inference (Chapter 25) solves exact
  equations and fails rather than widening.

## Chapter 5. Numbers: `int`, `float`, `rat`, and `byte`

### 5.1 `int` and `float`

Integers and floats are written as ordinary literals (`42`, `3.14`). The
arithmetic operators are `+ - * / %`. Rules:

- `-`, `*`, `/`, `%` require numeric operands;
- for `+`, `-`, `*`, and `%`, the result is `rat` when a `rat` participates,
  otherwise `int` unless a `float` operand forces `float`;
- `/` returns `float` when a `float` participates and exact `rat` otherwise,
  including `6 / 2`;
- division and modulo by zero are runtime errors.

Numeric and scalar conversions are explicit builtin actions:

- `int.float()` returns a `float`, rounding to binary64 precision;
- `float.int()` truncates toward zero and returns `int`;
- `int.byte()` checks `0..255`; `float.byte()` first truncates toward zero,
  then performs the same range check;
- `byte.int()` returns an ordinary `int`;
- `int.string()`, `float.string()`, and `byte.string()` return canonical
  numeric text; an integral float therefore renders without a `.0` suffix;
- `bool.string()` returns `"1"` for `true` and `"0"` for `false`.

These actions take no arguments and never introduce implicit conversion at a
typed boundary. A failed byte conversion is a runtime error.

### 5.1.1 Scalar casts and array casts `@T(...)`

Use `T(value)` for one primitive value and `@T(...)` for an immutable array.
The supported targets are `int`, `float`, `byte`, and `string`:

```okkama
let label is string = string(101);        // "101"
let whole is int = int(-3.9);             // -3
let packet = @byte(122, 11, 22);           // @byte
let singleton = @byte(112);               // one-element @byte
let empty = @byte();                      // typed empty @byte
let labels = @string(101, byte(2), true);  // ["101", "2", "1"]
```

Numeric targets accept `int`, `byte`, `float`, and `rat`. Integer casts
truncate toward zero, then check the signed-64 range for `int` or `0..255`
for `byte`. Thus `byte(255.9)` is 255 and `byte(-0.5)` is 0. Float casts
round to finite binary64 and can lose integer precision. String casts accept
numeric primitives, bool (`"1"`/`"0"`), and string; rational text is canonical.

An empty `@byte()` also provides exact `T = byte` evidence to a generic
constructor accepting `@T`; inference does not require a first element.

Scalar calls require exactly one positional argument. Array casts accept any
number, including zero, and retain the exact element type even without an
annotation. Neither form accepts named arguments. Array casts accept `name...`
from arrays and variadic bindings, including multiple spreads in any position:
`@byte(0, nums..., 255, other...)`. Scalar casts remain fixed-shape.
Call shape is checked before operands evaluate. Operands evaluate once,
left-to-right, followed by conversions in that order. A failed conversion
returns no partial array and suppresses the enclosing call; diagnostics name
the one-based argument index. Already performed operand effects are not rolled
back. Failures use the runtime type-error channel, not nominal `Error` values.

`int("101")` does not parse text; `@byte("abc")` does not encode text;
`@byte([1, 2])` does not spread arrays. Ranges are not expanded, `any` is not
unboxed, and object methods are not invoked. Existing cast methods share these
rules, including range checks; byte also supports `.float()`. Ordinary literals
and typed boundaries retain strict type checks without implicit casts.
Bare primitive calls take precedence over same-named locals; native declarations
cannot reuse these four names. Member methods retain ordinary action dispatch.
The two-argument `rat(numerator, denominator)` constructor is unchanged.

Run [primitive-casts.ok](../examples/primitive-casts.ok) for a complete example.
The [specification](okkama-spec-11072026.md#661-primitive-cast-calls-and-array-casts)
contains the normative conversion table.

### 5.2 `rat` — an exact rational

`rat` is an immutable arbitrary-precision fraction. `rat(numerator,
denominator)` accepts exactly two `int`/`byte` operands, reduces them by their
greatest common divisor, keeps the denominator positive, and rejects zero
denominators. Thus `rat(2, 6)` and `rat(1, 3)` have one canonical value.

Integer division produces `rat`, so `(1 / 3 + 2 / 3).string()` is exactly
`"1"`. Arithmetic may mix `rat` with `int` or `byte` and remains `rat`.
Mixing `rat` with `float` is an error until `.float()` is called explicitly.
Equality remains category-strict, while ordering accepts `rat` with exact
integer categories.

`rat.string()` returns canonical `n/d` text (or just `n` for denominator
one), `.int()` truncates toward zero, `.byte()` truncates and checks `0..255`,
and `.float()` is the explicit lossy boundary. `.numerator()` and
`.denominator()` return exact decimal strings
because their arbitrary-precision values can exceed the current `int`
representation.

`int.rat()` and `byte.rat()` are exact conversions from integer categories.
`float.rat()` explicitly rationalizes the float's canonical decimal value
(`0.125.rat()` is `1/8`); it does not expose an IEEE-754 storage fraction.
These conversions are required when strict equality needs aligned categories:
`1 / 2 + 1 / 2 == 1.rat()`.

### 5.3 `byte` — a checked octet

`byte` is a distinct integer-like value whose range is exactly `0..255`. It
exists for binary data — file contents, network payloads, raw paths — and it
is deliberately *not* a small `int`:

```okkama
let zero is byte = byte(0);
let maximum is byte = byte(255);
let payload is @byte = @byte(0, 127, maximum);
```

- Explicit `byte(value)` accepts `int`, `byte`, `float`, and `rat`:
  truncate toward zero, then check `0..255`. An existing byte is preserved.
  `@byte(...)` constructs an immutable array with the same conversions.
- There is no implicit narrowing, modulo wrapping, saturation, or byte literal
  syntax. `byte(256)` fails.
- `byte` satisfies the numeric umbrella `number`, but it does not satisfy
  `int`, and `int` does not satisfy `byte`.
- Arithmetic accepts `byte` operands mixed with `byte`/`int`/`rat`/`float`,
  but the *result* is never `byte` — it promotes to `int`, `rat` for exact
  division, or `float`. So byte
  arithmetic can never silently overflow within `0..255`; the result simply
  becomes an ordinary number and must be re-checked with `byte(...)` if you
  want a byte back.
- Equality is exact-category: `byte(1) == 1` is a runtime error. Ordering
  operators (`<`, `<=`, `>`, `>=`) are broader and accept mixed numeric
  operands under the promotion rules.
- A `byte` is a valid array index (interpreted by its integer value) and a
  valid range bound (normalized to `int`). Byte-valued **map keys** are
  currently unsupported.

`@byte` is an ordinary immutable typed array of bytes — the language's raw
binary sequence. It is not a mutable buffer and not a second string type.

### 5.4 Bitwise operations and mask literals

`&`, `|`, `^`, `~`, `<<`, `>>`, and `>>>` operate on fixed-width words:
signed 64-bit `int` or unsigned 8-bit `byte`. Binary `& | ^` require matching
categories; all bitwise operators preserve the left operand's category.
Mixing `byte` and `int` requires explicit `byte(...)` or `.int()` conversion.
Other categories are rejected; ordinary arithmetic promotion does not apply.

A shift count must be `int`, in `0..63` for `int` or `0..7` for `byte`.
Out-of-range counts fail rather than wrapping or being masked. `>>` preserves
the sign of an `int`; `>>>` fills with zeros. Both right shifts fill with
zeros for `byte`. `<<` discards bits shifted out of the word. The result is
still signed for `int`: `-1 >>> 0` is `-1`, and `1 << 63` is
`-9223372036854775808`. `~0` is `int(-1)`; `~byte(0)` is `byte(255)`.

Literals `0b1010_0011` and `0xFF` have type `int`; either prefix accepts
uppercase letters too. They are nonnegative and limited to INT64_MAX;
`_` is allowed only between digits. Bitwise `int` operands must fit signed
64-bit range. These rules do not redefine overflow for ordinary arithmetic.

```okkama
let enabled = 0b1010 | (1 << 2);        // 14
let cleared = enabled & ~(1 << 2);     // 10
let toggled = enabled ^ (1 << 2);      // 10
let present = (enabled & (1 << 2)) != 0;
let shifted = -1 >>> 1;                // 9223372036854775807
let rotated = byte(128).rotate_left(1); // byte(1)
```

The immutable `int`/`byte` methods `bit_count()`, `leading_zeros()`, and
`trailing_zeros()` return `int`. For zero, both zero counters return the word
width. `rotate_left(count)`, `rotate_right(count)`, `reverse_bits()`, and
`swap_bytes()` preserve the receiver's category. Rotations validate counts
like shifts. `swap_bytes()` reverses eight bytes for `int` and leaves `byte`
unchanged. Arguments are positional only; the receiver is never modified.

See Section 11.1 for precedence and evaluation, and the
[bitwise guide (Russian)](bitwise-guide.md) for the complete contract.

## Chapter 6. Strings and Characters

`string` is immutable Unicode text; `char` is a distinct single-character
value. They are separate categories: `'a' == "a"` is a runtime error.

Strings concatenate with `+`, and only with other strings:

```okkama
"foo" + "bar";   // "foobar"
"n = " + 1;      // runtime error: no implicit stringification
```

There is no string repetition operator and no interpolation in 1.0.

Portable builtin behaviors are available on every string (Chapter 10):

```okkama
"héllo".length();      // 5 — counts Unicode code points, not bytes
"hello".slice(1, 3);   // "el" — end-exclusive, code-point indexed
"a,b,".split(",");     // ["a", "b", ""]
"banana".replace("na", "NA");       // "baNANA"
"banana".replace_first("na", "NA"); // "baNAna"
["red", "green"].join(","); // "red,green"
```

Text meets binary data only through explicit, fallible conversion at the
native boundary (for example `runtime/fs.path_encode(string) is @byte`);
nothing decodes bytes into text implicitly.

## Chapter 7. Booleans and the Absence of Null

`true` and `false` are the two `bool` values, represented distinctly at
runtime — `true == 1` is an error.

**There is no truthiness.** Every condition — `if`, `match` against bool
logic, `confirm`, the operands of `!`, `&&`, `||`, postfix `if` — requires an
actual `bool`. Numbers, strings, collections, ranges, and objects are never
implicitly converted.

**There is no null.** No `null`, `none`, `nil`, no optional types, no `?.`
chaining. Where would-be-null situations arise, Okkama forces an explicit
decision:

- a missing map key is a runtime error unless you write an `else` fallback:
  `prices["missing"] else 0;`
- a failed lookup or invalid state is expressed by throwing a nominal Error
  object, caught by a typed `catch`;
- "not yet computed" is modeled with objects and roles, not sentinel values.

Some runtime operations may internally produce an *undefined* result (for
example an `if` without `else` whose condition is false). User code cannot
spell or store that value; it is not a language-level null.

## Chapter 8. Arrays, Maps, and Ranges

### 8.1 Arrays

```okkama
let xs = [1, 2, 3];
let typed is @int = [1, 2, 3,];   // trailing comma allowed
let empty = [];
```

- Arrays are immutable values.
- `@T` at a typed boundary validates every element recursively.
- Indexing uses brackets and `int` (or `byte`) indexes:
  `[10, 20, 30][1]` is `20`. Out-of-bounds indexing is a runtime error unless
  an `else` fallback is present: `xs[9] else 0;`
- Ranges expand inside array literals: `[0, 2..=4, 9]` is `[0, 2, 3, 4, 9]`.

### 8.2 Maps

Map literals use **brackets**, not braces, and the keys are *expressions*:

```okkama
let prices is %string:int = ["apple": 120, "pear": 140];
let empty_map = [:];
```

- `[]` is always the empty array; `[:]` is always the empty map.
- `%K:V` means keys of type `K`, values of type `V`, validated recursively.
- **There is no implicit string-key shorthand.** `[name: "Alex"]` looks up
  the *variable* `name`; write `["name": "Alex"]` for a string key.
- Missing keys are runtime errors unless an `else` fallback is given:
  `prices["fig"] else 0;`
- Map keys must be scalar values. Keys cannot be `any` or `byte` (the current
  map backend cannot preserve typed key identity for them). Map *values* may
  be any type including `any`.

### 8.3 Typed collection identity

Arrays and maps are backed by an immutable typed runtime representation
(`ArrayValue<T>` / `MapValue<K,V>`). Once a collection crosses a declared
`@T`/`%K:V` boundary, it is *tagged* with that type — and even an **empty**
result of `slice`/`keys`/`values` remembers its true element type. An
untagged empty literal (`[]`, `[:]`) has no contradictory evidence and may
cross any compatible explicit boundary; that first boundary establishes its
tag.

### 8.4 Ranges

```okkama
1..5     // exclusive: 1, 2, 3, 4
1..=5    // inclusive: 1, 2, 3, 4, 5
```

- Bounds must be integer-like (`int` or `byte`; byte bounds normalize to
  `int`).
- `start` must be `<=` `end`.
- Chained ranges are forbidden.
- Ranges are first-class values: they compare by `start`, `end`, and
  `inclusive`; they serve as `for` sources, expand in array literals, and
  accept the functional operations of Chapter 13 (materializing arrays).

### 8.5 Equality of composites

- arrays compare deeply by length and element order;
- maps compare deeply by key set and value equality;
- ranges compare by `start`, `end`, `inclusive`;
- objects compare by concrete runtime type and *all* fields, including
  private ones;
- runtime-owned effect capabilities compare by intrinsic state identity;
  detached BarrierState snapshots compare their four observed coordinates;
- Futures and action values compare by handle identity without running them.
  Aliases remain equal after completion; separate invocations, closures and
  independently obtained bound references are distinct;
- mixed runtime categories are runtime errors, never `false`.

## Chapter 9. The Top Type `any`

`any` is the explicit top type — a transport boundary for heterogeneous APIs,
*not* a switch to dynamic typing:

```okkama
let value is any = 42;
let values is @any = [1, "two", 3.0];
let row is %string:any = ["id": 7, "name": "Ada"];
```

The rules:

- Any ordinary concrete value may cross into a declared `any` boundary; it is
  stored as an **opaque immutable box** that remembers its concrete category.
- `any` is *never* implicitly assignable back to a concrete type, even when
  the hidden value has exactly that type. (Explicit narrowing syntax is a
  future feature.)
- Okkama code may **pass, store, return, catch, unpack, and iterate** boxed
  values — but may not *inspect* them: arithmetic, comparison, boolean logic,
  ranges, member access, calls, indexing, use as a map key, and `match` on an
  `any` value are all rejected.
- Repeated `any` boundaries do not nest boxes.
- `@any` and `%K:any` preserve the same opacity per element. Map *keys*
  cannot be `any`.
- Untyped heterogeneous literals (`[1, "two"]`) keep concrete elements; they
  do not infer `any` automatically. Only an explicit `any` boundary boxes.
- Role and fake signatures must spell `any` exactly; it is not variance.
- Trusted native dispatch is the one place concrete payloads are visible: a
  native parameter declared exactly `any` (or `...any`) receives the unboxed
  logical value. Nested boxes inside `@any` stay opaque even there.

Use `any` sparingly: it is for rows, generic transports, format-style APIs —
places where heterogeneity is the point. Everywhere else, prefer concrete
types, roles, or generics.

## Chapter 10. Builtin Value Behavior

Built-in values — arrays, maps, strings, and exact numbers — are receivers
with a small portable set of immutable behaviors. This is language semantics
(a fixed dispatch table in the runtime), not host method lookup and not a
native provider.

Runtime-supported today:

```okkama
[1, 2, 3].size();            // 3
[1, 2, 3, 4].slice(1, 3);    // [2, 3]        end-exclusive
"héllo".length();            // 5             Unicode code points
"hello".slice(0, 2);         // "he"
"a,,b,".split(",");          // ["a", "", "b", ""]
let words is @string = ["red", "green"];
words.join(" / ");           // "red / green"
"banana".replace("na", "NA");       // "baNANA"
"banana".replace_first("na", "NA"); // "baNAna"
prices.size();               // number of entries
prices.has("apple");         // true
prices.keys();               // @K, sorted by key
prices.values();             // @V, same pairing order as keys()
1.less(2);                   // true
2.less_or_equal(2);          // true
3.more(1);                   // true
3.more_or_equal(4);          // false
```

Rules:

- `slice` is end-exclusive; a negative index, `start > end`, or `end` beyond
  the length are hard errors — never clamped.
- `split`, `replace`, and `replace_first` use literal, left-to-right,
  non-overlapping matching — never regular expressions. `split` preserves
  empty fields at every edge and between adjacent separators.
- An empty separator splits into Unicode code points. An empty replacement
  target means code-point boundaries: `"ab".replace("", "-")` returns
  `"-a-b-"`; `replace_first("", "-")` prefixes once.
- `join` exists on exact `@string` values only, including typed empty arrays;
  joining an empty array returns `""`. Removal is
  `text.replace(target, "")`, not a separate builtin.
- Every transformation is immutable: it returns a new string or array.
- The numeric comparisons `.less`, `.less_or_equal`, `.more`,
  `.more_or_equal` require the **exact same** numeric category on both sides:
  `1.less(2.0)` fails. (The operators `<`/`>` are broader; the builtin action
  form is deliberately strict.)
- Builtin calls are positional-only and exact-arity.
- Builtin dispatch activates only for non-object receivers, so an object's
  own action named `size` or `less` is never shadowed and never shadows.
- Receivers with no builtin support at all (`bool`, `range`, `enum`, `any`)
  still report `Can only call actions on objects` when called.

---

# Part III — Expressions and Statements

## Chapter 11. Expressions

### 11.1 Operator summary

In precedence order (loosest first), as parsed:

```text
||                     boolean or        (bool operands only)
&&                     boolean and       (bool operands only)
<-                     stream write
==  !=                 equality          (strict same-category)
..  ..=                range construction
<  <=  >  >=           numeric ordering  (numeric promotion applies)
|                      bitwise or
^                      bitwise exclusive or
&                      bitwise and
<<  >>  >>>             fixed-width shifts
+  -                   addition/concat, subtraction
*  /  %                multiplication, division, modulo
!  -  ~                unary not, unary minus, bitwise complement
postfix                calls, member access, indexing, unpack `...`
```

Binary bitwise operators are left-associative. `flags & mask != 0` means
`(flags & mask) != 0`; `1 << 2 + 1` means `1 << (2 + 1)`.
Both operands evaluate exactly once, left to right, before type checks;
there is no short-circuiting. A failure or cancellation on the left prevents
evaluation on the right. These rules also apply to awaited operands.

Infix `^` is XOR; `is ^Type` and `^field = ...` retain their successor meaning:
`^bits = bits ^ mask;`. `xor` is an ordinary identifier. Compound assignments
such as `&=` and `<<=` are not supported. In type positions, adjacent `>>`
or `>>>` close nested generic types; in expressions they are shifts.

Stream writing uses `<-`; `<<` always means a shift. For example,
`writer <- (value << 2).string();` writes the shifted value as text.
`a<-b` is a write; comparison with a negative value requires `a < -b`.
Neither binary `~` nor `<<<` is a stream-write operator.

### 11.2 Arithmetic and concatenation

- `number + number` adds; `string + string` concatenates; mixed
  string/number `+` is a runtime error.
- `- * / %` require numeric operands.
- There is no string or scalar repetition via `*`, and no `**` power
  operator.

### 11.3 Equality vs ordering

Equality (`==`, `!=`) is strict per category (Chapter 4): `1 == 1.0` and
`byte(1) == 1` are errors, not `false`. Ordering (`<` etc.) accepts mixed
numeric operands under promotion. Boolean operators require `bool` operands
and never coerce.

### 11.4 Calls

```okkama
cash.total();
cash.add_bonus(10);
cash.add_bonus(bonus: 10);
```

- Positional and named arguments must not be mixed in one call.
- Duplicate named arguments, wrong arity, and unknown action names are
  errors.
- Receiver and arguments evaluate exactly once, left to right, in the
  *caller's* scope.
- Anonymous actions are ordinary expression values and use the same call
  syntax; their grammar, lexical capture and explicit signatures are covered
  in Section 18.5.

### 11.5 Indexing with fallback

```okkama
xs[2];
prices["apple"];
prices["missing"] else "fallback";
```

The `else` fallback expression is evaluated only when the index/key is
missing. Without it, a missing index/key is a runtime error.

### 11.6 `if` and `match` as expressions

`if` is an expression when both branches are present; branches are blocks and
the branch's last statement is its value:

```okkama
let value = if ready { 1; } else { 0; };
```

`match` may also be used in expression position with the same syntax as the
match statement (Chapter 12).

## Chapter 12. Statements and Control Flow

### 12.1 `let`

```okkama
let x = 10;           // inferred
let y is int = 10;    // checked against the declared type
```

- A `let` must have an initializer.
- Re-declaring a name in the *same* scope is an error; shadowing in a nested
  scope is allowed.
- A typed `let` is a full typed boundary: collections validate recursively,
  role types check conformance, `any` boxes.

### 12.1a Local rebinding

A bare `name = expr;` (no `let`) rebinds an existing local instead of
declaring a new one:

```okkama
let counter = Counter(value: 0);
counter = counter.increment();
```

This is legal only when `name` already denotes an existing local (otherwise:
`Undefined variable "name"`, exactly like reading it) *and* that local's
declared type is a concrete object declaring at least one **actual `^`-marked
successor action** whose return type is the type itself or a role it
implements (otherwise: `Cannot rebind variable "name": type "Type" has no
successor actions`). This is deliberately a hard rule, not an inferred one: a
*plain* (non-`^`) action that happens to return the same type does **not**
qualify, and neither does a role-typed declared local, even if some concrete
implementer of that role has a successor action of its own. A
persistent-structure method such as `List<T>.prepended(...)` — which returns
plain `List<T>`, because `EmptyList<T>.prepended` must return a *different*
concrete object (`LinkedList<T>`) that `^` cannot express, since `^` can only
stage a copy of the *same* receiver's own fields — does not make `List<T>`
rebind-eligible. Eligibility is an explicit per-action opt-in by the type's
author (`is ^Type`, Chapter 19), not something inferred from return-type
shape; see `docs/todo/local-rebinding-22072026.md` for the earlier, broader
rule that was tried and reverted. A plain scalar, array, map, or enum never
qualifies either way — `let n = 1; n = 2;` stays illegal.

The new value must satisfy the same declared type as the existing binding,
using the same rules as `let`. A rebind always updates the scope that
actually declared the name, even from a nested scope — the motivating case
is accumulating across loop iterations, where each iteration runs in its own
fresh child scope:

```okkama
object Counter {
    has value is int;
    Counter(:value);
    increment() is ^Counter { ^value = value + 1; }
}

let counter = Counter(value: 0);
for 1..=3 -> n {
    counter = counter.increment();
}
```

The previous value is not eagerly destroyed; if it was scope-owned with a
`destroy` block, that block still runs at the eventual natural scope exit as
usual — rebinding changes only which value a name currently denotes, never
when destruction happens.

Inside a constructor body, `name = expr;` binds a declared field first when
`name` is one of the object's own fields (Chapter 17); it is a local rebind
for any other name, including a same-named constructor parameter or a
`let`-bound local that is not itself a field.

### 12.2 Blocks

A bare block is a statement, creates a nested environment, and its bindings
do not leak:

```okkama
{
    let local = 1;
}   // local is gone; objects owned here run destroy (Chapter 20)
```

### 12.3 `if` statements

```okkama
if a {
    1;
} else if b {
    2;
} else {
    3;
}
```

The condition must be `bool`. `else if` chains are supported.

### 12.4 `match`

```okkama
match status {
    case Status.ready {
        1;
    }
    case Status.failed, Status.done {
        2;
    }
    else {
        0;
    }
}
```

- Case values are expressions compared with ordinary (strict) equality; a
  case may list several values.
- `else` handles the unmatched remainder.
- Matching *on* an opaque `any` value is rejected.
- Exhaustiveness checking and branch type compatibility are future semantic
  analysis work.

### 12.5 `for`

```okkama
for users -> user {
    user.name();
}

for 1..=3 -> n {
    n;
}
```

The source may be an array, a range, a variadic unpack (`values...`, Chapter
18), or an object implementing the standard `Iterable<T>` role from
`collections/iteration`. The current runtime supports one binding. Iteration
order is source order. Both the source and `->` binding are mandatory; use
`loop`, not a source-less `for`, for an unbounded loop.

User-defined collections expose an immutable cursor:

```okkama
role Cursor<T> {
    available() is bool;
    current() is T;
    advanced() is Cursor<T>;
}

role Iterable<T> {
    cursor() is Cursor<T>;
}
```

This is nominal, not duck typing: the collection must explicitly `does
Iterable<T>`. `for` asks for one cursor, checks `available()`, binds
`current()`, and replaces its private cursor reference with `advanced()`.
Neither the collection nor an existing cursor changes. Normal completion and
`next` advance; `last` exits immediately without requesting another cursor.
A false `available()` is ordinary exhaustion, with no sentinel, null or
exception. Consumable I/O streams are deliberately a separate protocol.

### 12.6 `loop`

`loop` is an unbounded loop and an expression. Its result is supplied
explicitly by `last value`:

```okkama
integer() is int {
    loop {
        let text = input.prompt("Enter an integer:");
        try {
            last value ParsedInt(text);
        }
        catch error is InvalidInteger {
            output.writeln("Not a valid integer");
        }
    }
}

let hundred = loop {
    last 100;
};
```

The ordinary tail value of the body is ignored, and each iteration gets a
fresh lexical scope. Bare `last;` exits without a public value: that is valid
when the loop result is discarded, but `let value = loop { last; };` is an
error because a variable cannot be initialized with no value.

### 12.7 `next` and `last`

```okkama
next;                  // skip to the next iteration
last;                  // exit the loop
next if should_skip;   // postfix form
last if done;
last result;           // exit and produce a loop result
last result if done;   // the result is lazy when the condition is false
```

Postfix `if` also works on ordinary expression statements:

```okkama
counter.bump() if enabled;
```

The postfix condition must be `bool`, like every condition. It is evaluated
before a `last` payload; a false condition does not evaluate that payload.
Control targets the nearest `for` or `loop`. A functional
`map`/`filter`/`reduce` block cannot control an enclosing loop directly, but a
nested explicit `loop` establishes its own boundary. Placement is checked
before execution: `next`/`last` outside any loop, or directly inside a
functional block, is a semantic error even in code that never runs.

## Chapter 13. Functional Collection Operations

Arrays, ranges, typed maps, and objects nominally implementing the standard
`Iterable<T>` role support exactly three **eager, immutable** operations,
written with a dedicated block syntax:

```okkama
let doubled = values.map() -> value {
    value * 2;
};

let selected = values.filter() -> value {
    value.more(0);
};

let total = values.reduce(0) -> accumulator, value {
    accumulator + value;
};
```

They chain naturally:

```okkama
let total = values
    .filter() -> value { value.more(0); }
    .map() -> value { value * 2; }
    .reduce(0) -> sum, value { sum + value; };
```

The `-> bindings { ... }` part belongs to its call like a further member
call, so a functional expression is an ordinary operand:

```okkama
let shipped = base + orders.reduce(0) -> sum, order { sum + order.count(); };
if names.filter() -> name { name == wanted; }.size() > 0 { ... }
```

Only at the top level of a `for` source does the arrow belong to the loop;
there, group the functional source in parentheses.

Semantics:

- **Bindings.** Arrays, ranges, and `Iterable<T>` objects bind one element.
  Typed maps bind `key, value` — and map `reduce` binds
  `accumulator, key, value`.
- **Result shapes.** Array/range/Iterable `map` and map `map` return a typed
  array. Array/range/Iterable `filter` returns a typed array; map `filter`
  preserves the typed map. Range and Iterable results are eagerly
  materialized arrays. An empty Iterable filter retains exact `T`.
- **`map` result type.** `map` produces one exact output element type. A
  direct typed `@U` target can supply it (essential for empty sources);
  otherwise the first result establishes `U` and every later result is deeply
  checked against it. `@any` output happens only with an explicit `@any`
  target.
- **`filter`** predicates must return exact `bool`.
- **`reduce`** always takes one initial value. The accumulator keeps one
  exact type, supplied by a direct declared target or the initial value.
  Reducing an empty source returns the checked initial value without running
  the block.
- The receiver and the initial value evaluate exactly once; items are visited
  deterministically in source order (maps in sorted-key order); the first
  failing item stops the operation and propagates through ordinary typed
  throw handling.
- The operation block is **dedicated lexical syntax**, not an anonymous action
  value and not an ordinary call argument. It captures the surrounding lexical
  scope, receives fresh iteration bindings, and cannot use `next` or `last`.

Custom iteration is nominal, exactly as for `for`: matching `cursor()` methods
or a local role also named `Iterable` are insufficient. The runtime obtains
one standard cursor and processes each item in `available`, `current`, body,
`advanced` order. A failing body does not advance that item. All cursor calls
cross their ordinary public action and generic return boundaries.

An identity map with a direct target is the general linear materialization
idiom for a custom collection:

```okkama
let materialized is @int = values.map() -> value { value; };
```

One grammar caveat: inside a `for` source, a top-level `->` belongs to the
`for` pattern — parenthesize a functional expression used as a loop source.

Keep the parentheses in `values.map() -> value { ... }`: this is the
dedicated functional collection form. Ordinary actions take anonymous values
inside their argument parentheses, as described in Section 18.5.

Not in this milestone: `grep`/`fold` aliases, lazy streams, parallelism,
map-entry transformation, and mutable accumulators.

## Chapter 14. Failure: `throw`, `panic`, `try`/`catch`

Okkama splits failure into two channels:

1. **Expected typed failures** — raised by `throw ErrorObject(...)`, by
   `confirm ... else throw ErrorObject(...)`, and by normalized native
   failures. They carry a nominal `Error` and can be caught by typed `catch`.
2. **Uncatchable failures** — `panic "message"`, a plain failed `confirm`,
   undefined variables, type mismatches, out-of-bounds indexing and invalid
   operations. They unwind cleanup and then terminate. A bug is not a value.

### 14.1 `confirm`

```okkama
confirm condition;                             // uncatchable on failure
confirm condition else throw ErrorObject();    // expected typed failure
confirm condition else panic "Invariant lost"; // uncatchable emergency
```

`confirm` asserts an invariant. The condition must be `bool`. Use the bare
form for internal invariants, `else throw` for failures a caller is expected
to handle, and `else panic` for an emergency with a deterministic string.

### 14.2 `Error`, `throw`, and `panic`

```okkama
object InvalidValue does Error {
    has value is int;
    InvalidValue(:value);
    text() is string { "invalid value"; }
}

throw InvalidValue(value: 10);
throw;                  // rethrow — lexically inside a catch body only
panic "Unreachable";   // exact string, never catchable
```

`Error` is an automatically available nominal role requiring public
`text() is string`. Merely having a `text()` action is not enough: a source
object must explicitly `does Error`. `throw` accepts only such objects and the
system `NativeFailure`. Bare `throw;` preserves the active thrown object's
identity and origin; it is a *lexical* capability of catch bodies, validated
before execution — an action called from a catch cannot rethrow implicitly.
`panic` accepts only an exact string; bare `panic;` is invalid and panic never
participates in catch matching.

### 14.3 `try` / `catch`

```okkama
let message is string = try {
    operation();
    "accepted";
}
catch error is InvalidValue {
    error.text();
};
```

Rules:

- `try` is dual-purpose, like `if`: it may stand as a block-level statement or
  yield the selected body value in an initializer, argument, grouping or
  another expression. Both forms require at least one typed catch clause.
  There is **no `finally`, no catch-all and no guards**.
- Only `Thrown` failures participate. Panic, programming errors,
  `next`/`last`, and normal completion never match a catch.
- Catch type references are validated before execution; the `Error` guarantee
  is enforced before the try body runs. A catch type must guarantee `Error`:
  `Error`, a concrete/applied object explicitly doing `Error`, or
  `NativeFailure`. Scalars, collections, enums, `any`, and arbitrary roles are
  invalid catch boundaries.
- Valid clauses are examined in source order; the first compatible clause
  wins.
- A non-matching failure propagates unchanged.
- The try body and each catch body are separate lexical child scopes. The
  catch binding exists only in its own body; try locals are not visible in
  catches.
- **Cleanup ordering:** unrelated objects owned by the try scope are destroyed
  *before* catch selection, in reverse construction order. Owned Error payloads
  move into the matching catch scope instead. Catch-owned objects are
  destroyed before an expression yields. A body failure has precedence over
  a cleanup failure.

Design guidance, in the EO spirit: throw dedicated nominal Error objects with
context, catch concretely or by `Error`, and rethrow (`throw;`) when you cannot
fully handle a failure. Reserve panic for broken invariants.

### 14.4 Adding context without losing an error

A new typed Error escaping a catch retains the handled failure as a runtime
cause, including its exact value, origin, stack and native details. This also
works after `await`. Bare `throw;` preserves the existing carrier. Cleanup
failures remain separate secondaries; panic and programming errors remain
uncatchable. A normally completed catch leaves no cause attached to later
throws. The link records an error during handling, even when it comes from a
helper rather than an explicit wrapper constructor.

`Error` still requires only `text()`. For a cause that must travel as an ordinary
application value, use a separate wrapper with a required Error field:

```okkama
object Contextual does Error {
    has original is Error;
    Contextual(:original);
    text() is string { "invoice i1"; }
    cause() is Error { original; }
}
```

Plain errors have no such field, so no null or sentinel is necessary. This
explicit value protocol works outside catch too; the runtime does not inspect
fields or call `cause()` automatically. Host diagnostics traverse the private
carrier's `cause()` metadata and render `Caused by:` entries with source stacks.
Default CLI diagnostics show the concrete Error type, public message, optional
application context, causes, source stacks and secondary cleanup failures.
`-d` additionally shows the evaluator trace; it is not needed to see errors.
For example, `text() is string { code; }` and optional
`context() is string { label; }` expose explicitly chosen diagnostic text.
The renderer uses bounded data-only projections (fields, literals, string
concatenation, conditions and simple helpers), without executing arbitrary
application effects or enumerating private fields. If the projection is
unsupported, the message is marked unavailable while type, stack and causes
remain visible. Control characters are escaped; long strings are marked as
truncated. Array bounds diagnostics also show `index` and `length`.
Embedding APIs preserve the original failure objects; host tools can obtain a
separate structured model through `Interpreter.diagnostic(failure)`.

## Chapter 15. `defer`

```okkama
defer cleanup();
```

A reached `defer` runs exactly once when its **current lexical scope exits**,
on success or failure: normal completion, `throw`, `panic`, runtime error,
`next`, `last`, `return`, and async cancellation. It belongs to the current block,
not necessarily the entire action. Leaving an inner scope drains its own
stack before execution continues in the enclosing scope.

- Registration records the expression without evaluating it. The receiver
  and arguments are evaluated at scope exit, not at registration time.
- Only reached statements register cleanup; an unvisited branch registers none.
- Cleanup runs in **LIFO order**, with local values still alive, before
  automatic `destroy` of objects owned by that scope.
- Constructors, actions, `destroy`, try/catch bodies, and loop iterations
  follow the same lexical rule.
- Every remaining defer is attempted even if another defer fails. An existing
  body failure has priority; a pending return is cancelled by cleanup failure.
  Otherwise the first defer failure in
  LIFO execution order wins, followed by automatic destruction failures.
  A propagated typed throw is catchable; panic is not. An enclosing catch
  observes the failure after the exited scope has finished cleanup.
- Discarded-result rules still apply: `defer` cannot silently discard a
  must-use successor.

In an async action, cleanup may suspend: `defer await audit.finished();`.
Cancellation also unwinds registered defers; the Future does not finish until
cleanup completes. Requesting cancellation is not itself completion.

For resource ownership use `destroy`; for additional scope-local work use
`defer`. Neither promises cleanup after forced process termination or when
execution never leaves the scope.

This behavior is covered by `t/interpreter/54-defer.t`, the async suites, and
`t/audit/09-async-lexical-blocks.t`. See also [async cleanup](async-guide.md).

---

# Part IV — Objects

## Chapter 16. Objects and Fields

### 16.1 Declaring an object

```okkama
object Cash {
    has digit is int;
    has cents is int = 0;
    has currency is string = "USD";

    Cash(digit is int, cents is int) {
        digit = digit;
        cents = cents;
    }

    total_cents() is int {
        digit * 100 + cents;
    }
}
```

Object names are type names (uppercase). Two objects with the same name in
one module are forbidden. Object headers take no parameters (state is never
declared in the header).

An object body may contain: `has` and `cap` declarations, constructors, at most one
`destroy` block, test blocks, public actions, and a `private:` section of
private actions.

### 16.2 Fields (`has`)

`has` is the **only** way to declare object state:

```okkama
has digit is int;              // required: must be bound by construction
has cents is int = 0;          // optional: default value
has digit, cents is int;       // several names may share one declaration
```

- Field names must be unique within the object (across all `has`
  declarations).
- Field defaults are evaluated per construction, *before* the constructor
  body, and are type-checked. Defaults execute in **declaration scope**: they
  may use literals and globally resolved declarations (objects, enums,
  imports) but never see caller locals or sibling fields of the instance
  being built.
- **Fields are private through ordinary references.** `Cash(digit: 1).digit`
  is a runtime error; peers call public owner behavior.
  Grants control field reads and never make private actions public. There is
  no setter syntax.
- Inside the object's own code, fields are read bare (`digit`) or explicitly
  (`self.digit`). A parameter or local may shadow a bare field name; the
  explicit `self.field` spelling always means the field.

This is Elegant Objects' "no getters" rule made mechanical: if a caller needs
something derived from state, the object exposes a *behavior* (`total_cents()`)
— never the raw state.

### 16.3 Object capabilities (`cap`)

An object may declare a named contract of synchronous computed fields:

```okkama
cap BusinessCard {
    full_name is string { given_name + " " + family_name; }
}
```

A cap may also be declared at module scope for an object owned by that module:

```okkama
cap Customer::BusinessCard {
    full_name is string { Customer::given_name + " " + Customer::family_name; }
}
```

The external body requires owner-type qualification for private fields.
`Customer::given_name` reads the selected instance; it does not expose a receiver,
owner methods or sibling caps. Internal declarations keep short field names.
Both forms share one nominal namespace, so duplicates are errors. Declaration
order is irrelevant, and linked files of the same module may differ; imports
resolve in the cap's defining file. Imported objects cannot be extended.
An external `cap Box::Public` inherits the parameters of `object Box<T>` and
uses `Box::item` in its body; consumers still use `Box<int>::Public`.

Inside `Customer`, this declares the nominal type `Customer::BusinessCard`.
Acquire it explicitly with `customer::BusinessCard`; read with `card::full_name`
or `customer::BusinessCard::full_name`. The owner expression runs once per
acquisition. Operand type resolves `::`; identifier case does not select the
operation. A full owner never converts implicitly. Caps cannot become owners,
sibling caps or roles, cannot be constructed directly, and cannot be acquired
by unwrapping a restricted role reference.

Bodies use ordinary Okkama blocks, local bindings, control flow, result checks
and error handling. Private fields of the immediate owner are readable by name.
A same-named computed field reads the owner field; locals may shadow it.
There is no `self`, including in nested closures, no implicit owner method
receiver or substitute owner binding, and no implicit sibling-field lookup.
Nested objects retain their privacy. Owner writes and successor writes are
forbidden. Each field has a synchronous context with no `await`; explicit
capabilities may still have effects. A field may explicitly return a Future.

Acquisition is lazy; every read recomputes its body, with no value/error cache.
Typed errors and panic retain their existing categories. A cap retains the
entire owner lifetime graph, including hidden resources, and remains bound to
its original immutable snapshot after a successor. Locals, parameters, returns,
object fields, collections, generics, closures and local async/Future transport
preserve the exact cap type. `any` remains opaque. Returned values keep their
own contracts, including restricted roles and explicitly broad object results.

Identity includes the defining owner, module/ABI major and owner type arguments
(`Box<int>::Public`). Existing imports and aliases apply. Version 0.0.1 permits
only unique computed fields: no methods, constructors, roles, `has`, lifecycle
blocks, nested caps or async fields. Ordinary dot access does not provide an
alternative API. Equality, ordering, hashing, automatic formatting,
serialization and owner reflection are unavailable; structural/dynamic paths
cannot unwrap the owner. Diagnostic formatting may safely name the cap type.
This is an interface boundary with no general secrecy, purity or revocation
promise. Other full references retain their capabilities.

Object views have been removed. Roles and `#projected` keep their
separate contracts. OIR Profile 0 remains int-only and explicitly refuses
objects and caps. See the [working example](../examples/object-caps.ok) and
[full guide](object-caps-0.0.1-ru.md).

## Chapter 17. Constructors

### 17.1 Constructors are mandatory and explicit

Every object must declare at least one constructor. Constructors are never
generated from `has` fields — an object without one is a registration error
even if never constructed. A stateless object still writes one:

```okkama
object Marker {
    Marker() {}
}
```

Construction is call-based: `Cash(digit: 1, cents: 25)`. (A construction
*block* form exists in the parser for future use; call syntax is the 1.0
idiom.)

### 17.2 Full constructors

A full constructor has typed parameters and a body of field binds:

```okkama
Cash(value is int) {
    self.digit = value;
    self.cents = 0;
}
```

- The constructor's name must equal the object's name.
- Calls use named **or** positional arguments — never mixed. Positional
  arguments bind to constructor *parameters* (never to field order).
- The body consists of ordinary statements plus **field bind statements**:
  `field = expr;` or `self.field = expr;`. Field binds are valid only in
  constructor bodies (and their lexical children); a bind anywhere else is a
  semantic error before execution. `other.field = expr` is not a thing; there
  is no general assignment in the language.
- By the end of construction every field must be initialized (bound or
  defaulted): `Object "Cash" field "cents" was not initialized` otherwise.
- Full constructors overload like actions (18.3): by arity and parameter
  types. Equally specific compatible signatures are ambiguous; declaration order never breaks ties; there is no
  fallback that derives a signature from `has` fields.

Keep constructors focused on binding, simple normalization and explicit
`confirm ... else throw` checks (17.5). Unrelated effects belong in actions.

### 17.3 Field shorthand constructors

An object may expose selected typed fields as a **named-only** construction
allow-list:

```okkama
object Member {
    has name is string;
    has id is int;
    has internal is int = 40;

    Member(:id, :name);
}

let member = Member(name: "Alice", id: 7);
```

- Each `:field` names an explicitly typed `has` field of the same object
  (declared before *or* after the constructor — validation happens after the
  whole body is collected).
- The list is a closed allow-list: unlisted fields cannot be supplied by the
  caller. Listed fields without defaults are required; with defaults,
  optional.
- Shorthand calls accept **named arguments only**, rejected before argument
  evaluation if positional.
- A bodyless shorthand is a declaration error if some *unlisted* field has no
  default (it could never produce complete state).
- A shorthand may also have a body: automatic bindings happen first, then the
  body runs and may initialize unlisted required fields or deliberately
  overwrite a bound one:

```okkama
Member(:id, :name) {
    self.internal = id + 1;
}
```

- `Marker();` is the zero-field shorthand — a distinct, valid form for
  stateless objects.
- Selection: full constructors match by arity and parameter types (18.3);
  shorthands match by named-field compatibility (all supplied names allowed,
  all required names present, values fit the field types). Equally specific
  signatures are ambiguous errors.

### 17.4 Variadic constructors

A full constructor may declare one variadic parameter as its unique final
parameter, exactly like a variadic action tail (Chapter 18):

```okkama
object Min<T> {
    has first is T;
    has rest is @T;

    Min(first is T, rest is ...T) {
        first = first;
        rest = rest...;
    }
}
```

- Calls are positional-only; named calls are rejected
  before evaluating arguments.
- The fixed prefix sets the minimum arity; each tail value is checked left to
  right against the element type.
- A variadic constructor overloads with other constructors. A fixed or
  shorthand constructor that fits the call wins, so `Bag()` next to
  `Bag(items is ...int)` takes the empty constructor, and `Bag(1, 2)` the
  variadic one.
- Inside the body the parameter is an opaque `varargs<T>`; unpack it with
  `rest...` as a `let` value, a complete field-bind value, or a `for` source.
- Exact generic inference runs across the fixed prefix and every tail value.

### 17.5 Constructor validation

```okkama
object InvalidScore does Error {
    has value is int;
    InvalidScore(:value);
    text() is string { "invalid score: " + value.string(); }
}
object Score {
    has value is int;
    Score(value is int) {
        self.value = value;
        confirm value >= 0 else throw InvalidScore(value: value);
    }
}
```

`confirm` is an ordinary constructor statement. It can read parameters, locals,
`self` and initialized fields. Each full or shorthand constructor explicitly
performs its checks. `else throw` raises a typed error and prevents construction
from returning or dispatching a projection. Constructor defers and owned-resource
cleanup run on failure. Successor actions validate their own transitions;
constructors are not implicitly invoked for successors.

A caller's expected construction type helps resolve generic parameters before
the body starts; it does not become the type of an internal `reduce`. Typed
locals and field bindings provide their own target types. For example, a bool
check inside a constructor stays bool even when the constructed object is
received as a role. The outer context is restored on success and failure.
Likewise, the bool result of a comparison does not retag its integer operands.

There is no separate post-constructor validation phase. A shorthand with
checks needs a body, for example `Score(:value) { confirm value >= 0 else
throw InvalidScore(value: value); }`. Checks are explicit in each overload;
they are not inherited from another overload. A bare `confirm condition;`
panics on failure instead of raising a catchable typed error.

On constructor failure, registered defers run before owned-resource cleanup.
The unfinished receiver is not returned and its own `destroy` is not invoked
as though it were a completed object. Resources owned by the constructor,
including newly created field values, are released; borrowed caller-owned
resources remain the caller's responsibility. See Chapter 15 and
`t/interpreter/258-constructor-confirm.t`.

### 17.6 Named constructor projections

An object can expose one or more actions as readable constructor projections.
`#projected` uses the action name as the prefix; `#projected(alias)` provides a
different prefix:

```okkama
object ParsedInt {
    has source is string;
    ParsedInt(:source);

    #projected(value)
    integer() is int {
        42; // abbreviated derivation from source
    }
}

value ParsedInt(answer)                 // ParsedInt(answer).integer()
```

The prefix is resolved only after the following constructor target has been
resolved to a concrete object. It is not a free function or a dynamic operator,
and it cannot project variables, calls, or arbitrary parenthesized expressions.
The projection action must be public,
synchronous, zero-argument, explicitly typed, non-successor, and return a type
different from the enclosing object. A generic result such as `T` is valid;
returning the enclosing `Object<T>` is not. An object may mark several actions,
but each effective prefix must be unique within that object.

The named prefix works with every constructor surface:

```okkama
of Cash(digit: 1, cents: 25)
value numbers.ParsedInt(answer)
```

Empty, full, shorthand, variadic, overloaded, generic, and
namespace-qualified constructors retain their ordinary selection rules.
Construction, including its `confirm` checks, must succeed before the projection action is called.
Arguments evaluate exactly once, left to right, and the result can continue
through normal postfix operations.

The directive selects a declaration, not an entire action name. Other fixed
overloads remain available:

```okkama
#projected
value() is int { 10; }
value(base is int) is int { base * 10; }
```

`value Object()` calls the zero-argument overload, while
`Object().value(16)` calls the one-argument overload. The ordinary rule that
all overloads of one name share a return type still applies. A duplicate
zero-argument signature remains ambiguous; the projection directive is not an
overload tie-breaker. The removed single-`&` syntax is a parse error; `&&`
continues to mean boolean conjunction.

## Chapter 18. Actions

### 18.1 Public actions

Actions define behavior:

```okkama
total_cents() is int {
    digit * 100 + cents;
}
```

- Actions read fields directly (or as `self.field`) and call other actions
  through `self`, including private ones.
- Parameters are typed and checked; the result is checked against the
  declared return type.
- A caller's contextual expected type stops at the action boundary; it cannot
  retag a nested expression inside the callee.
- A typed action returns its final statement's value or an explicit `return` value.

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

Action types permit `(T) is void`; anonymous actions declare their own
result explicitly, for example `(value is T) is void { ... }`. `async ... is void` and `async (T) is void` are supported for named actions,
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

Explicit receiver result:

```okkama
touch() is Cash {
    self;
}

Cash(digit: 1, cents: 25).touch().touch();
```


#### Explicit early return

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

### 18.2 Private actions

A `private:` section marker starts the private actions:

```okkama
object Cash {
    has digit is int;

    Cash(digit is int) { digit = digit; }

    reveal() is int {
        self.secret();
    }

    private:
    secret() is int {
        digit;
    }
}
```

Private actions are callable only through `self` from the same object's
actions. A constructor cannot call any action on its unfinished receiver,
public, private or async, including inside `confirm`. Aliases, role references,
bound actions and collaborators cannot bypass this rule. Constructors initialize
fields and validate invariants directly with `confirm`; they may read initialized
fields and inspect parameters or other completed objects. A captured callback
may invoke `self` after construction finishes. Private actions cannot be exported
as values.
External calls fail *after* signature selection (so overload
resolution is uniform): `Action "secret" on object "Cash" is private`.

### 18.3 Typed action overloads

Actions overload by arity **and** parameter types. A variadic tail counts
as one parameter, and `...int` is a different type from `int`:

```okkama
print(value is int) is int { 1; }
print(value is string) is int { 2; }
print(value is any) is int { 3; }
```

Resolution:

1. Receiver and arguments evaluate exactly once, left to right.
2. Candidates filter by arity and named-parameter shape (a variadic one
   takes at least its fixed prefix); then argument types are checked against
   declared types.
   If a fixed candidate fits, variadic candidates are not considered.
3. Specificity: exact concrete/applied types beat role-compatible types, and
   both beat `any`; a concrete collection element type beats a shape
   containing `any`.
4. A candidate wins only when it is no less specific in **every** parameter
   position and strictly more specific in at least one (positional
   dominance). So `(int, any)` vs `(any, int)` is ambiguous for `(int, int)`.
5. Declaration order never breaks ties. Ambiguity and no-compatible-candidate
   have distinct diagnostics that include the actual argument types.
6. An opaque `any` value is *not* narrowed back to its stored concrete type
   during selection.
7. Named calls additionally filter by parameter names, so same-type
   signatures can be distinguished by name where a positional call cannot.
8. Overloads of one name and arity must declare the same return type.
9. A variadic tail compares its element type with every tail value. A call
   with an empty tail is ambiguous between `f(xs is ...int)` and
   `f(xs is ...string)`; declare `f()` to give it a meaning.

```okkama
total(values is ...int) is int { 0; }
total(a is int) is int { a; }      // total(1) — this one; total(), total(1, 2) — the tail
```

Constructors use the same selection (17.2). Native functions do not overload.

### 18.4 Variadic actions

An action (or role signature) may declare one typed variadic tail after any
fixed prefix:

```okkama
format(pattern is string, values is ...any) is string {
    let materialized is @any = values...;
    pattern;
}
```

- The variadic parameter is unique, final, explicitly typed, and defaultless.
- Calls are positional-only and must supply the whole fixed prefix; the tail
  takes zero or more values, each deeply checked against the element type.
  `...any` boxes each heterogeneous tail value independently.
- Inside the body, the bare parameter is an opaque `varargs<Type>` binding —
  not an array, not usable in ordinary expressions.
- `values...` is a dedicated unpack. It is allowed as the complete RHS of a
  `let`, as the complete value of a constructor field bind, as a `for` source,
  or as the final positional argument of another variadic action or full
  constructor:

```okkama
let checked is @int = values...;

for values... -> value {
    value;
}

delegate(pattern is string, values is ...any) is string {
    string Formatted(pattern, values...);
}
```

`name...` accepts an array or a real variadic binding and expands one level:

```okkama
let nums is @int = [1, 2, 3, 4, 5];
let bytes = @byte(nums...);
let framed = @byte(0, nums..., 255);
```

Ordinary actions and constructors require a variadic tail and the complete
explicit fixed prefix. They accept one final positional spread and check each
element strictly against the tail type. Array casts accept multiple spreads
in any position and explicitly convert each expanded element. No recursive
flattening or any unboxing occurs. Empty sources contribute zero arguments;
their retained element type still supplies generic constructor evidence.
An array-valued expression may be unpacked directly, for example
`@byte(@int(1, 2)...);`. It evaluates once. Named, array-literal and return
spread remain invalid.
On the right of a stream write, `out <- @string(1, 2, 3)... <- endl;`
writes `123` followed by a newline. The complete array expression evaluates
once, then each element uses ordinary stream-source validation. An empty
source makes no writes. Failure stops subsequent elements and chain sources;
completed writes are not rolled back. Use parentheses for an awaited array:
`out <- (await source.words())...;`. Postfix `...` already binds above `<-`.
The same expansion rules hold across `await`. See the
[array spread guide](array-spread.md) and [runnable example](../examples/array-spread.ok).

### 18.5 Anonymous actions and action-value types

An **anonymous action** uses the same declaration syntax as a named action,
with the action name omitted. Its parameters are explicitly typed, and its
result is mandatory, including `is void` for an effect-only action.

```okkama
let answer = () is int { 42; };
let double = (value is int) is int { value * 2; };
let add = (left is int, right is int) is int { left + right; };

answer();
double(10);
add(7, 8);
```

It is not a second global function model. Conceptually, the result is an
anonymous immutable object with one callable action. Internally the runtime
stores the parameter list, body, source position, and an immutable snapshot of
the visible bindings referenced freely in the body, including nested actions.

That environment makes anonymous actions lexical closures:

```okkama
let make_adder is (int) is (int) is int = (amount is int) is (int) is int {
    (value is int) is int { value + amount; };
};

let add_ten = make_adder(10);
add_ten(5); // 15, even after make_adder's call has ended
```

Parameters and local declarations shadow captured names. Each action captures
values and declared types at creation: later rebinding outside the action does
not change the snapshot, and declarations appearing later are not captured.
Rebinding a captured name inside the action is an error. Capturing an object
preserves its identity; it does not copy the object.

An escaping action transfers ownership of captured local resources through the
same rules as an object containing those resources. Unreferenced locals are
not retained. A reference to `self`, or to the implicit receiver of
`fake.storage`, retains that receiver. Access permissions from the declaring
action remain in force even when the receiver is not captured.

A `let` initialized directly with an anonymous action (optionally parenthesized)
can refer recursively to its own name when no outer binding of that name is
visible. That name is supplied in each invocation, without retaining the
original scope. Mutual recursion through later declarations is not supported.

The public type of an action value is its exact signature:

```okkama
() is int
(int) is int
(int, int) is int
((int) is int, (int) is int) is (int) is int
```

Each anonymous action declares its own immutable signature. A typed `let`,
an object or role action parameter, or a declared action result checks that
signature:

```okkama
object Apply {
    has input is int;
    Apply(:input);

    value(operation is (int) is int) is int {
        operation(input);
    }
}

let double is (int) is int = (value is int) is int { value * 2; };
Apply(input: 10).value(double); // 20
```

Signatures may nest and may contain collection, generic, object, and role
types. They participate in generic substitution, role conformance, action
results, and fixed overload selection. Compatibility is exact: an already
typed action value cannot be silently retyped. Explicit anonymous signatures
participate in overload selection; parameter/result variance is not added.

Pass anonymous actions as ordinary arguments, including named arguments:

```okkama
Apply(input: 21).value(operation: (value is int) is int { value * 2; });
```

Ordinary parentheses still group expressions: `(value)`, `(a + b)`, and
`(receiver.value())`. A parameter name followed by `is`, or an empty parameter
list, identifies an anonymous declaration; its result and block are required.
Callable types omit parameter names: `(int) is int`, `() is void`, and
`() is (User) is bool`. `void` is legal only in the result position.

Old `-> value { ... }` declarations, `(T) -> R` types, and trailing member-call
sugar are rejected. The arrows in `values.map() -> value { ... }` and
`for values -> value { ... }` belong to separate constructs and remain valid.

Current boundaries are intentionally narrow:

- action-value calls are positional and exact-arity; named calls are rejected;
- anonymous parameters require explicit types and cannot be variadic;
- a trailing comma in an anonymous parameter list is rejected;
- no implicit currying, partial application, or async anonymous actions;
- action values cannot cross `any` or the native ABI;
- there is no automatic conversion to an arbitrary user role;
- each call is a control boundary: `next`/`last` cannot escape into an outer
  loop, while `throw`, `catch`, and per-invocation `defer` follow ordinary
  action rules.

## Chapter 19. Successor Actions and Immutability

Objects are immutable after construction. The *only* way to express a state
change is a **successor action** — an action whose return type is `^Type`:

```okkama
object Bottle {
    has opened is bool = false;

    Bottle() {}

    open() is ^Bottle {
        confirm !opened else throw BottleAlreadyOpened();
        ^opened = true;
    }
}
```

Semantics:

- `^field = expr;` (**successor assignment**) stages a field update. It is
  valid *only* inside `is ^Type` actions — never in ordinary actions,
  constructors, `destroy`, or module code. Misplacement is a
  semantic error before execution.
- The staged field must exist and the value must match its declared type.
- When the action completes, the runtime builds a **new object**: current
  fields copied, staged updates applied. The original is untouched.
- The successor is a fresh identity with an independent lifetime (Chapter
  20).
- Locally owned objects retained by staged fields stay alive until the
  successor action finishes, including objects introduced in nested blocks.
  A caught failure in a nested block does not invalidate those fields.
  Successful completion transfers ownership of the final fields to the caller;
  the body's last expression does not determine which objects escape.
- If the action fails midway, no partial successor is produced and the
  staging context is cleared. Its owned resources are destroyed, including
  staged field objects; resources borrowed from the original remain borrowed.

**The result must be consumed.** Statically visible loss is a semantic error,
because the caller would silently keep operating on stale state:

```okkama
let bottle = Bottle();
bottle.open();               // error: successor result must be used
let opened = bottle.open();
let saved = [opened];        // storing in an aggregate consumes it
```

Successor actions chain, since each returns a real object:

```okkama
let closed = Bottle().open().close();
let saved = [closed];
```

A binding alone is not enough: `let unused = bottle.open();` fails if it dies
without use. A receiver call, argument, return, field binding, eligible rebind,
or aggregate storage consumes it. Aliases share the obligation; bare discarded
reads do not count. Every normal branch must consume the value. Loops cannot
prove use merely by mentioning it in a possibly unexecuted body.

Throw/panic may abandon pending values. Catches conservatively begin with the
try-entry obligations, so use only inside a try may need an additional use on
the handler path. A reached defer can consume a binding at its own scope exit,
after intervening rebinds. An early return from a nested scope conservatively
cannot rely on an enclosing defer.
Closure creation alone is not use: a closure carrying successor captures must
be invoked, returned, passed, or stored. Await transfers a successor future's
obligation to its result. Runtime discard guards remain for dynamic cases.
These checks concern language-level consumption, not persistence or business
usefulness; they do not forbid using an already consumed object again.

A successor names a real domain transition or its resulting state, such as
`opened()`, `advanced()`, or `enqueued(value)`. It is not a license to expose
one successor per field. Immutable copy-setters and stepwise configuration
builders still model public setters; use complete constructor state, composed
value objects, or decorators instead.

Successor types compose with roles: `^Bottle` satisfies `^Package` when
`Bottle does Package` (Chapter 22). Private successor actions are not
supported.

This is the EO immutability discipline with runtime teeth: state transitions
are explicit in signatures (`is ^Type`), visible at call sites (`let opened =
bottle.open()`), and impossible to fake through hidden mutation.

## Chapter 20. Ownership, Scope, and `destroy`

### 20.1 `destroy` blocks

```okkama
object File {
    has path is string;

    File(path is string) {
        path = path;
    }

    destroy {
        // cleanup side effects go here
        path;
    }
}

main() is int {
    {
        let file = File(path: "data.txt");
    }   // file.destroy runs here
    return 0;
}
```

- At most one `destroy` per object. It is not callable; it runs automatically
  when the owning environment exits.
- It may read `self` and fields; it must not use successor assignment.
- Destruction is deterministic and tied to lexical scope — **not** to the
  host garbage collector.
- If a destroy block fails, the runtime reports it as a runtime error; a body
  failure that triggered the unwind takes precedence over cleanup failures.

### 20.2 Ownership follows identity, not names

A lazy ordinary async call retains its receiver and evaluated arguments in the
ownership graph before it starts. An escaping Future transfers fresh owned
dependencies; borrowed outer resources retain their existing owner. Cancelling
before startup or never awaiting the Future still permits exactly-once cleanup
at the owning scope's exit.

Fresh identities in one acquired aggregate register in creation order, so map
key sorting and closure capture enumeration cannot change reverse cleanup order.
Owned typed Error payloads move through unwind to the matching catch scope;
bare rethrow and automatic causes retain them. Other try locals are cleaned
before catch. Uncaught payloads are drained before native registry shutdown.

Objects bound with `let` are owned by the current environment. When the
environment exits, owned objects run `destroy` in **LIFO** (reverse
construction) order. The precise rules:

- Introducing another binding for an object *already reachable* from the
  current lexical environment creates a **borrowed alias**, not a second
  owner. No double destruction.
- Existing caller-owned receivers and parameters remain borrows. A fresh
  receiver or argument with no owner is adopted by the called action before
  bindings are defined; anonymous actions adopt fresh arguments as well.
- Arrays, map values, object fields, and `any` transport preserve the
  identities they contain; repeated references never duplicate ownership.
- Fresh object identities first introduced inside a newly bound aggregate are
  owned by that binding's environment.
- A lazy async action started by `await` transfers fresh result ownership to
  that awaiting lexical scope, even when a continuation resumes after suspension.
  Extracting and returning a child value does not keep its containing owner alive;
  the owner is destroyed at scope exit while the returned child escapes.
- Every owned identity enters destruction **at most once**, even if its
  destroy block fails, and aliasing `self` inside `destroy` cannot re-enter
  destruction.
- Successor objects have fresh identities and independent lifetimes.
- Unbound expression values receive a lexical owner. Fresh action inputs are
  cleaned at action exit unless they escape through the result. Standalone
  object expressions are cleaned when their lexical scope exits.
- Async continuations register expression results in their explicit lexical
  environment, including conditional tails and deferred expressions.
- Channel messages pin their actual resource graph until delivery and receiving
  scope completion. Returning a received value transfers that pin. Existing
  source aliases retain their owner; repeated messages never duplicate destroy.
  Closing preserves buffered values; cancellation releases undelivered pins.
  When all channel capabilities finish, unread messages are cleaned.

Completed resource constructors remain tracked until a binding or action takes
ownership. If a later argument, array element or operator fails, earlier fresh
resources are still cleaned at scope exit. Unfinished constructor self is never
published into this journal. VM-created successor identities and fresh Channel
endpoints use the same admission journal. An async successor fixes its result
owner at action startup; an unrelated action signalling its wait does not acquire
that result. A valued `last resource;` transfers the reachable
resource out of its loop; `last;` without a value keeps ordinary loop control.

The runtime can omit ownership walks for a completed immutable object only when
it has no destroy bodies, intrinsic state or fake storage, and every actual field
is already proven resource-free. Its detached field table is frozen before that
proof is retained. Declared types alone are insufficient. Futures, actions,
capability-bearing fields and uncertified host objects keep conservative traversal.

A role or array cursor can retain this proof only for its entire certified
origin. Selecting safe fields does not hide a resource in other fields. The
wrapper metadata is fixed before retaining the proof; role access
boundaries remain unchanged.

The standard HTTP1 reader drains a separate supervision scope for each physical
read. Its memoized result and cancellation remain intact, and it does not close
the borrowed source. This bounds retained Future history; it does not establish
lower RSS or network latency.

### 20.3 Cleanup interaction summary

On every block exit, including return and failure, the runtime performs
deferred expressions (LIFO), then owned-object destruction (LIFO). Inside
`try`/`catch`, unrelated try-owned objects are destroyed before catch selection;
owned Error payloads move into the matching handler.

## Chapter 21. Lexical Scope Boundaries

Okkama is lexically scoped, strictly. A declaration body never closes over
the runtime environment of its caller. The boundaries:

- `main` executes in its own root environment.
- Receiver, action arguments, constructor arguments, and construction field
  expressions evaluate in **caller** scope, left to right.
- An action body sees only `self`, receiver fields, its declared parameters,
  and locals it (or its lexical children) introduces.
- A constructor body sees `self`, its parameters, its field bindings, and its
  own locals.
- `destroy` sees only `self`, receiver fields, and their own
  locals.
- Field defaults execute in declaration scope (Chapter 16).
- Nested blocks, branches, and loop iterations are lexical children of the
  body containing them.
- Imported declarations never capture locals of the importing module.
- An anonymous action is an explicit expression value, not a declaration
  body: it retains the enclosing locals, parameters, and `self` visible where
  it is created. A later caller's locals are never added to that capture.

So this fails — and that is the point:

```okkama
object Reader {
    Reader() {}

    read() is int {
        leaked;   // Runtime error: Undefined variable "leaked"
    }
}

main() is int {
    let leaked = 42;
    Reader().read();
}
```

Data crosses a declaration boundary only through explicit arguments,
construction fields, and receiver fields. Pass what you need:

```okkama
let amount = 42;
Sink().push(amount);
```

---

# Part V — Abstraction

## Chapter 22. Roles

Roles are Okkama's contracts — nominal, explicit, and behavioral:

```okkama
role Package {
    open() is ^Package;
    close() is ^Package;
}
```

An object implements a role explicitly with `does` (multiple roles are
comma-separated):

```okkama
object Bottle does Package {
    has opened is bool = false;

    Bottle() {}

    open() is ^Bottle {
        ^opened = true;
    }

    close() is ^Bottle {
        ^opened = false;
    }
}
```

Conformance is checked at registration:

- every required role action must exist on the object;
- parameter types and fixed/variadic shape must match **exactly**;
- return types must be *compatible* — in particular `^Bottle` satisfies
  `^Package` because `Bottle does Package`;
- a role may declare same-name/same-arity signatures when their complete
  parameter type vectors differ and the return contract is the same; exact
  duplicates are errors.

Once an object conforms:

- concrete values are assignable to role-typed boundaries (`let p is Package
  = Bottle();`, role-typed fields, parameters, returns, catch clauses,
  collection elements);
- role-typed calls dispatch to the concrete object with the same typed
  overload rules;
- nominality matters: an object with the right shape but no `does` clause
  does **not** satisfy the role. Contracts are declared intent, not
  structural accident.

Roles are the backbone of composition (Chapter 30): depend on `Package`, not
on `Bottle`.

## Chapter 23. Role Fakes and Testing

Elegant Objects argues that an interface should ship its own fake so tests
never build ad-hoc mocks. Okkama builds that into the role:

```okkama
role Package {
    open() is ^Package;

    fake {
        has opened is bool = false;

        open() is ^Package {
            ^opened = true;
        }
    }
}
```

- A role may contain at most one `fake` block, registered as role metadata.
- `Role.fake()` constructs the fake — **only in explicit test mode**. In
  production mode the call is rejected.
- Fake `has` declarations define typed receiver state; every field must have
  a default, because fakes take no constructor parameters. Defaults evaluate
  per construction.
- Fake actions must satisfy the role contract exactly (name, fixed/variadic
  shape, parameter types, return type) and may read fields and stage
  successor updates through `^field`.

### 23.1 Hidden fake storage

Fake actions may also use a small mutable KV store for test scenarios where a
double must remember calls or emulate a repository:

```okkama
role Books {
    remember(id is int, title is string) is void;
    title(id is int) is string;

    fake {
        remember(id is int, title is string) is void {
            fake.storage.set(id, title);
        }

        title(id is int) is string {
            fake.storage.get(id);
        }
    }
}
```

The complete positional-only API is `set(key, value)`, `get(key)`,
`has(key)`, `remove(key)`, `clear()`, and `size()`. Missing `get` and `remove`
keys are programming errors—there is no null or optional result. Keys follow
ordinary map-key restrictions; `any`, `byte`, and composite keys are invalid.
Values may include objects and opaque `any`.

This is a test-runtime intrinsic, not a mutable Okkama map. It is available
only as a direct `fake.storage.<operation>(...)` call inside that fake's action
bodies. It cannot be stored, returned, passed, exposed as `self.storage`, or
observed through fields, equality, debug output, conformance, or nominal type.

Each `Role.fake()` call creates an isolated storage identity. A fake and all
of its successors share one identity, so repository state survives an
ordinary `is ^Role` transition without copying the KV contents.

### 23.2 Test blocks

Test blocks are declared at module, object, or role level:

```okkama
test "open then close returns original" {
    let bottle = Bottle();
    test.is(bottle.open().close(), bottle);
}
```

Use explicit `async test` when the body needs `await`:

```okkama
object Worker {
    Worker();
    async value() is int { 7; }
}
async test "async result" {
    let result = await Worker().value();
    test.is(result, 7);
}
```

This declaration is available at module, object and role scope and uses the
same `-t` command. Ordinary tests remain synchronous. Async tests use the normal
scheduler and await/typed-failure machinery; assertions may contain await
directly, and the contextual `test` capability remains available to async
helpers. Repeated awaits follow the ordinary reusable-Future rules.

An async test completes without producing a value; its ordinary tail expression
is discarded, and `return` remains restricted to actions. The runner waits for
body completion and defer/destroy cleanup before recording pass/fail or starting
the next case. Pending spawned tasks are cancelled and drained under the normal
root-task policy; unobserved failures belong to that test. Throw, panic and
runtime failures fail the current case while subsequent cases still run.
There is no implicit test timeout. Async tests are pruned in production and
share discovery, naming and assertion rules with sync tests. No generated main
wrapper is needed.

Production parsing consumes the header and balanced body but emits no Test or
Fake AST, so test-only inner grammar cannot affect production. Run tests with:

```text
perl bin/okkama -t path/to/module.ok
```

The runner retains test/fake AST across imports but executes only tests owned
by the root file. It does not run or require `main`. Object/role test names are
displayed as `Owner :: name`; this is namespacing only, with no implicit
`self` or private-access exception. Test names must be unique within the
module (per owner for object/role tests); duplicates are semantic errors
before any test runs.

Each test uses its declaring file's imports, including object/role-owned tests.
When several files form one module, their tests keep separate import scopes;
an import in one file does not become visible to a test in another file.

Every running test receives an automatic `test` binding—no import:

```okkama
test.ok(condition);
test.is(actual, expected);
test.isnt(actual, expected);
test.fail("explanation");
```

`ok` requires exact bool; `is` and `isnt` use strict/deep Okkama equality;
`fail` records its string. At this trusted assertion boundary, values carried
through `any` parameters can be compared recursively without becoming visible
or narrowed in source code. Ordinary `any` equality remains forbidden.

The binding follows the complete dynamic execution of one test, so an action
from an imported module can delegate to it. It is still contextual rather than
first-class, fresh for every case, and absent outside test execution. Failed
assertions do not abort the block. An uncaught throw, panic, runtime, defer, or
destroy failure ends only the current test, cleanup finishes, and the next test
runs. The Go-like reporter returns process status 0 only for an entirely green
suite and 1 otherwise. There is no `test.panics`; typed `try`/`catch` covers
throw behavior, while panic is intentionally uncatchable.

Reusable assertions come from the bundled module:

```okkama
import "test/more" take Assertions, AssertionFailure;

test "total" {
    Assertions()
        .ok(true)
        .is([1, 2], [1, 2])
        .isnt("draft", "paid");
}
```

`Assertions` is stateless. Its `ok`, `is`, and `isnt` actions record checks in
the active test; `fails(error is Error)` records `error.text()` unconditionally.
`AssertionFailure(message)` only retains its message and computes the stable
`assertion failed: ...` text in its action. Assertion actions cannot be called
outside a running test.

## Chapter 24. Enums

```okkama
enum Status = (ready, failed, done);
```

- Enum names are type names; cases are lowercase identifiers.
- Cases are referenced with member syntax: `Status.ready`.
- Enum declarations register before `main` runs; duplicate enums and
  duplicate cases are errors.
- An enum value infers as its enum type, and typed boundaries accept only the
  matching enum type.
- Equality compares enum type and case — two different enums' cases never
  compare equal; they error like any mixed category.
- Enum values work directly in `match` cases:

```okkama
match status {
    case Status.ready  { "go"; }
    case Status.failed { "stop"; }
    else               { "wait"; }
}
```

Enums are values, not integers: there is no ordinal, no implicit numeric
conversion, and no case fall-through.

## Chapter 25. Generic Type Parameters

Objects and roles may declare ordered type parameters:

```okkama
object Box<T> {
    has value is T;
    Box(:value);
    get() is T { value; }
}

role Echo<T> {
    echo(value is T) is T;
}
```

- Parameters are uppercase type names, unique within the declaration, and may
  not shadow the declaration's own name.
- Object parameter scope covers fields, constructors (full and shorthand),
  actions, successor actions, and `destroy`. Role parameter scope
  covers signatures and the fake block. Parameters never leak outside.

### 25.1 Exact concrete identity

The whole generic model rests on one rule: **one `T` is one concrete type per
specialization**.

- Repeated occurrences of `T` must resolve to the identical type.
- `int`, `float`, `rat`, and `byte` are distinct and never unify:
  `Min(1, 2, 3)`
  infers `T = int`; `Min(1, 2.0)` **fails** — no widening, no union, no
  fallback `any`.
- Object parameters may have one nominal role bound, for example
  `object Heap<T: Comparing<T>>`. After exact constructor inference, the
  inferred type must implement the substituted role before the constructor
  body runs. Bounds never widen values or supply missing inference evidence.
  Role parameters, category bounds such as `T: number`, multiple bounds,
  variance, wildcards, defaults, and higher-kinded types are not supported.
  A parameter is a concrete type, so `T<int>` is invalid.
- Applied types take exactly the declared arity: `Pair<int>` is invalid for
  `Pair<K, V>`; a non-generic type takes no arguments.

### 25.2 Construction inference

There is no explicit `Box<int>(...)` call form — specialization is
**inferred** at construction from exact equations collected across direct
parameters, nested arrays, maps, and specialized objects (full and shorthand
constructors both participate; variadic constructors run inference across
prefix and tail):

```okkama
let box = Box(value: 7);                      // Box<int>
let bucket is Bucket<int> = Bucket(values: []); // target supplies T for []
```

Empty collections contribute no equation; a direct declared target may supply
the missing evidence — but expected types never *weaken* actual evidence: if
values contradict the target, construction fails before the body runs.
Calls that pass analysis evaluate arguments once, left to right, before
solving their runtime value-dependent equations.

The declared expected type also reaches constructor field assignments,
explicit returns, action tail results, and arguments with an unambiguous
parameter type. For example, `has states is Channel<int>;` allows
`states = Channel(capacity: 1);` in its constructor, without a temporary typed
local. A result declared as `Empty<int>` may use `return Empty();` or a tail
`Empty();`. This works in async actions too.

Nested construction uses only parameters already determined by the expected
type: `let x is Box<Empty<int>> = Box(item: Empty());` supplies `int` to the inner
constructor. The analyzer retains this context and the runtime checks actual
argument evidence against it. Conflicting numeric types, ambiguous role
specializations, and missing evidence remain errors. The compiler does not
look ahead to later uses or guess overloads to obtain a type. An untagged empty
collection adds no conflict to a known collection slot, such as `Box<@int>`.

The same rule applies recursively to array and map literals. If `Box` takes
`value is T`, then `let x is Box<@int> = Box(value: [1, 2, 3]);` supplies
`T=@int` and checks each element against `int`. An empty `[]` also works in
that context; without an expected type or other evidence it cannot determine
`T`. Nested arrays, map values and generic objects receive the corresponding
element types. Known element conflicts are reported during analysis, while
values whose types depend on execution are still checked at runtime.

Explicit role evidence remains a role specialization when a concrete
implementation arrives through another equation. A `List<Json>` prepended
with `JsonBool` therefore stays `List<Json>`; it is not narrowed to
`List<JsonBool>`. This rule does not search for a common role between two
concrete values, so unrelated concrete equations remain an error.

Builtin arrays are `Iterating<T>`, `Sequencing<T>` and `Sizing`, and their
cursors are `Cursor<T>`, so a plain array is evidence for a role-typed
generic parameter: `Maximum(values: [3, 9, 4])` infers `T = int` from the
element tag (or, for an untagged literal, from its elements). An untagged
empty literal still has no evidence.

### 25.3 Specialization is identity

Each runtime object retains its specialization. `Box<int>` and `Box<float>`
are distinct types across fields, parameters, results, variadic tails,
successors, lifecycle bodies, typed variables, and imports.

Role conformance is exact and substituted:

```okkama
object EchoBox<T> does Echo<T> {
    EchoBox();
    echo(value is T) is T { value; }
}
```

`EchoBox<int>` is assignable to `Echo<int>` and not to `Echo<string>`, and
that assignability survives collections, catch clauses, fields, calls,
successors, and imports.

### 25.4 Generic fakes

A generic fake must conform *symbolically* — a fake written with `int` never
satisfies a signature written with `T`. Since a fake has no constructor
arguments to infer from, a declared target supplies the specialization:

```okkama
let echo is Echo<int> = Echo.fake();   // test mode only
```

Without a target, fake construction fails rather than guessing.

---

# Part VI — Programs in the Large

## Chapter 26. Modules and Imports

### 26.1 Import forms

```okkama
import "money";
import "money" take Cash;
import "io/stdout" as out take println, printf;   // selected names under out
```

- Imports appear after the module declaration and before other declarations.
- No wildcard imports.
- Alias (`as`) imports replace the basename namespace with the chosen name.
  Combined `as ... take` exposes only selected directly owned declarations in
  that namespace; aliases do not create bare bindings.

### 26.2 Path resolution

Import paths are *source paths*, not module names. `import "money";`
resolves `money.ok`; `import "foo/bar";` resolves `foo/bar.ok` (explicit
`.ok` suffixes are accepted as-is). Lookup is local-first:

1. the importing file's own directory — the entry script's directory for its
   imports, and each loaded module's directory for that module's imports;
2. the root of the importing file's package — the nearest directory above it
   with `blade.toml` — when it has one;
3. system import directories (`OKKAMA_IMPORT_DIRS`);
4. the bundled Okkama standard library directory.

The working directory the program is started from plays no part, so a module
tree behaves identically wherever it is run from. Two modules that import the
same relative path from different directories load different files. A local
module shadows a bundled stdlib module with the same path for the program's
own imports; the standard library resolves its own imports only inside itself.

In a blade project, dependencies are installed in `pkg/<name>/` of the project
root and imported by that path: `import "pkg/web/router" take Router;`. Only
dependencies declared in `[dependencies]` of the importing package's
`blade.toml` can be imported this way. Module files of a project declare no
version (`module app;`): the version is set in `blade.toml` (spec §5.2.2).

### 26.3 What an import exposes

- A **plain import** exposes declarations owned directly by the module only
  through a basename namespace derived from the final path component:
  `import "io/stdout";` permits
  `stdout.Stdout()`, and the same namespace resolves type annotations and
  applied generics (`shapes.Area`, `boxes.Box<int>`). The namespace only
  selects the declaration. Its internal nominal identity is qualified by the
  defining module and ABI major version (for example,
  `shapes#1::Area` or `boxes#1::Box<int>`); this internal prefix is omitted
  from ordinary language-facing output and diagnostics.
  Declarations brought into that module by its own imports are not re-exported.
- An **`as` import** replaces that basename namespace:
  `import "io/stdout" as io;` exposes `io.Stdout()` but not
  `stdout.Stdout()`.
- A **`take` import** resolves listed names only among directly owned
  declarations, exposes exactly those names as bare bindings, and creates no
  namespace. Missing `take` names are semantic errors, reported before
  execution. Contracts and implementation dependencies are retained privately,
  never as implicit consumer bindings.
- **Dependency closure:** selected declarations keep the transitive closure
  needed for registration and execution, but it creates no consumer bindings.
  Constructors, actions, defaults, lifecycle blocks, and role fakes resolve
  names in their defining module scope rather than the caller's scope.

### 26.4 Hygiene rules

- Duplicate bare names introduced by `take` or by local declarations are
  semantic errors, reported before execution. Namespace-qualified and hidden
  declarations from different modules do not collide: their module-qualified
  nominal identities remain distinct even when their source names match.
  A narrow `take name` retains every same-name declaration candidate, so an
  invalid module cannot hide a duplicate by declaration order.
- Repeated imports of one module are idempotent.
- An imported module's `main()` never runs on import.
- Cyclic imports are rejected.

### 26.5 Standard libraries: current std and legacy stdlib

The current library is the root `std/` tree. Its contract is
[roadmap v4](std/stdlib-roadmap-v4.md), with [design principles](std/stdlib-design-principles.md)
and the [completion guide](std/stdlib-completion-todo-v4.md).
`File` owns closing; `File.binary()` and `File.text()` expose separate stream roles.
Tree deletion and temporary-file preservation are explicit void commands.
`Formatted(pattern: "{}", values: ["Ada"]).string()` performs typed interpolation;
`CompactJson(origin: document).string()` uses `JsonValue.print(JsonWriting)` without reflection.
See the [migration guide](std/stdlib-v4-migration.md) for removed APIs.

The historical inventory and examples below describe **legacy `stdlib/`**.
They remain executable with that library, but are not API proposals or completion
claims for `std/`. Select the library root explicitly; do not combine their modules.


The stdlib is intentionally narrow while the language stabilizes:

- `okkama` — `Dump`, the debug formatter (Chapter 28);
- `math` — small generic examples such as `Max<T>`;
- `strings/parse` — strict, pure-Okkama conversion from strings to `int`,
  `float`, `bool`, and `byte`;
- `collections/iteration`, `collections/list`, `collections/queue`, and
  `collections/stack` — nominal immutable traversal plus persistent linked,
  FIFO, and LIFO values;
- `format` — positional and named immutable projections with typed pattern
  and missing-value failures;
- `template/plain` — pure named string templates with closed
  ERB/Mojolicious-style lexical tags and typed source/value failures;
- `template/html` — HTML rendering over the same lexical tags, where a
  binding's `SafeHtml` type — not its tag — decides whether it is escaped;
- `encoding/binary` — cursor-free endian-explicit reads and composed
  fixed-width binary values;
- `encoding/utf8` — explicit strict conversion between Unicode `string` and
  raw `@byte`, with typed invalid-sequence offsets;
- `encoding/base64` — canonical padded RFC 4648 conversion between raw
  `@byte` and ASCII `string`, with strict typed decoding failures;
- `encoding/hex` — lowercase canonical hexadecimal output and strict
  case-insensitive ASCII decoding with typed source offsets;
- `encoding/json` — nominal exact JSON values, strict recursive parsing,
  deterministic compact encoding and located Unicode errors;
- `encoding/csv` — RFC 4180-shaped streaming rows over `Input`/`Output`
  under an explicit dialect, with a located `InvalidCsv`;
- `encoding/toml` — a practical single-line-document TOML v1.0.0 subset
  with dotted-path lookup and deterministic `[table]`-sectioned rendering;
- `crypto/random` — stateless OS-CSPRNG byte capability with a deterministic
  test fake and typed size/unavailability failures;
- `crypto/digest` — reviewed SHA-256/384/512 digests over bytes or a full
  `Input` stream, with hex/base64 projections and sanitized provider
  failures;
- `crypto/hmac` — keyed HMAC-SHA256/384/512 with constant-time `matches()`
  verification over the reviewed `secure_equal` primitive;
- `socket/socket` — the shared `Socket` role, explicit option enums and
  timeout vocabulary ahead of concrete TCP/UDP/Unix socket resources;
- `sql/value` — exact bindable SQL scalar wrappers, including an explicit
  `SqlNull`, ahead of SQL statement/query building;
- `http/status` — a validated `Status(code, reason)` with status-class
  queries, plus `Ok`/`Created`/`BadRequest`/`NotFoundStatus`/
  `InternalServerError` projecting into it;
- `http/headers` — immutable case-insensitive `Headers`, `ExtendedHeaders`/
  `FilteredHeaders` composition, and CRLF-injection-rejecting `Header`
  values;
- `log/event` — immutable structured `LogEvent` (level, message, fields,
  instant) and the six standard `LogLevel` severities;
- `db/result` — non-row execution outcome (`affected()`, an optional
  `last_id()`) for `INSERT`/`UPDATE`/`DELETE`;
- `db/row` — name/index access to one query-result row's `SqlValue`s;
- `fs/file` — whole-file `File` (`bytes()`/`string()`/`write()`/`append()`/
  `remove()`) plus streaming `OpenedFile` (`Input`/`Output`/`Closeable`);
- `fs/dir` — directory creation/listing/removal, recursive
  `DirectoryWalk`, and symlink-safe recursive `remove_all()`;
- `socket/tcp` — real TCP `TcpSocket`/`TcpListener`, and
  `TcpConnection`'s DNS-driven ordered address attempts;
- `socket/udp` — real UDP `UdpSocket` datagram send/receive with genuine
  sender identity;
- `socket/unix` — `UnixSocket`/`UnixListener`, honestly reporting
  `UnsupportedUnixSocket` since the native provider has no `AF_UNIX`
  support at all yet;
- `crypto/tls` — `TlsPolicy`, an encrypted `TlsSocket`, and peer
  `Certificate` inspection;
- `sql/dialect` — reviewed SQLite/Postgres/MySQL placeholder, identifier
  and boolean rendering differences;
- `sql/statement` — `Insert`/`Update`/`Delete`/`RawStatement`, rendered
  through any `Dialect`;
- `sql/query` — `Select`/`RawQuery`, with explicit `EveryRow`/
  `UnlimitedRows`/`RowWindow` vocabulary;
- `http/request` — immutable `HttpRequest` and the `Get`/`Post`/`Put`/
  `Patch`/`Delete` convenience constructors;
- `http/response` — streamed `HttpResponse` plus explicit
  bytes/string/JSON materialization;
- `http/handler` — the `Handler` role plus `HandlerChain`'s
  chain-of-responsibility dispatch, where declining is spelled by throwing
  `HandlerFailure` rather than returning;
- `http/router` — `Router`/`GetRoute`/etc. dispatch by method and
  `PathPattern`, with `:name` segments extracted as `@PathParameter` (not a
  native map — Okkama cannot build one with a runtime-determined entry
  count from pure code);
- `http/client` — a hand-rolled HTTP/1.1 `Client` over `socket/tcp`/
  `crypto/tls`, serializing requests and parsing responses byte-by-byte,
  following `Location` redirects up to a configured limit;
- `http/server` — a sequential `Server` accepting one connection at a time
  over plain TCP, parsing requests and writing responses the same
  byte-by-byte way as `http/client`, in reverse; `ServerPolicy.tls()` is
  stored but not yet wired up, since `crypto/tls` has no server-side
  wrapper at all today, only the client side;
- `log/formatter` — deterministic `LineFormatter`/`JsonFormatter`
  rendering of a `LogEvent`;
- `log/logger` — `OutputLogger`/`FileLogger`/`SilentLogger`/
  `ContextLogger`/`MemoryLogger` behind one `Logger` role; `MemoryLogger`
  round-trips events by serializing them into a `runtime/memory` cell, since
  Okkama has no mutable-cell primitive for arbitrary values, only bytes;
- `fs/temp` — `TempFile`/`TempDir` remove their owned entry on destroy
  unless `keep()` transfers ownership out; `TempRoot` generates collision-free
  unique names via real exclusive creation, not a racy check-then-create;
- `db/transaction` — the `Transaction` role and `InactiveTransaction`; every
  concrete implementer is a still-ahead database driver, since transaction
  framing is inherently database-specific — this, the sixth and final
  Phase 7 module, closes "owned workflows";
- `db/connection` — the `Connection` role and `ConnectionClosed`/
  `DbFailure`, the same shape as `db/transaction` for the same reason; this,
  the third and final Phase 8 module, closes "composition layers" alongside
  `http/router` and `http/server`;
- `db/driver` — the DBI-style `Driver` role (`connect() is Connection`,
  `dialect() is Dialect`) and `DriverFailure`, the same role-and-errors-only
  shape as `db/connection`/`db/transaction` for the same reason; opens Phase 9
  ("application-level batteries") ahead of the concrete `db/sqlite`/
  `db/postgres`/`db/mysql` drivers;
- `db/database` — `Database`, a thin application-facing facade constructed
  with a `Driver`; `connect()` delegates and translates any `DriverFailure`
  into one stable `DatabaseUnavailable`, so application code never depends
  on a specific driver's own failure type;
- `db/sqlite`/`db/postgres`/`db/mysql` — explicitly **deferred**: a real
  SQLite file-format engine or Postgres/MySQL wire-protocol driver is a
  multi-week undertaking on its own, and there is no structured-AST
  shortcut around parsing SQL text — `Statement.sql()`/`Query.sql()` always
  render to plain SQL strings. See `db-sqlite-22072026.md`;
- `db/migration` — `Migrations`, a real (not role-only) implementation
  composed against any `Connection`: rejects duplicate migration versions
  at construction, applies/rolls back each migration in its own
  transaction (most-recently-applied first on rollback); its own applied-
  migration history lives in an owned `runtime/memory` cell rather than
  being read back from the database, since this stdlib has no runtime
  role-to-concrete downcast from a queried `Row` and `sql/query`/
  `sql/statement` have no `WHERE`-clause support at all yet — a known,
  documented limitation, not durable across a process restart; building it
  also surfaced a real interpreter bug (`.map()` used as a bare action-body
  tail expression infers its element type from the first result's *concrete*
  type rather than the declared role return type, wrongly rejecting later,
  differently-typed-but-role-compatible elements), root-caused but not
  fixed, worked around with an explicit `let` — see
  `db-migration-22072026.md`;
- `random` — secure ranges/fractions plus explicitly injectable deterministic
  choices and immutable permutations;
- `os` — typed host and explicit POSIX/Windows descriptions with closed
  platform, architecture, and path-style enums;
- `fs/path` — host-selected and explicit POSIX/Windows immutable lexical paths
  with exact UTF-8 projection and typed failures;
- `fs/meta` — portable `FileKind`/`Permissions` metadata read through
  `lstat`, so symlinks are never silently followed;
- `time/duration` — signed checked nanosecond spans and exact named-unit
  projections;
- `time/instant`, `time/clock`, and `time/date` — clock-bound points,
  injectable wall/monotonic clocks, Gregorian dates and UTC civil projection;
- `time/timer` — explicitly acquired owned one-shot and fixed-rate periodic
  timers;
- `test/fake_clock` — a `Clock` implementation tests can explicitly advance,
  rewind or set, so `sleep` never blocks;
- `test/benchmark` — injectable-clock repeatable duration measurement over a
  `BenchmarkCase`, so statistics are asserted exactly against `FakeClock`
  instead of tolerated against real timing noise;
- `test/suite` — `Suite`, constructed with an explicit `@TestCase` list, an
  explicit `@Fixture` list and an injected `Clock`; pass/fail is purely
  exception-based (any thrown `Error` fails a case), since the language's
  own contextual `test` capability that `test/more.Assertions` delegates to
  only exists inside a real `test { }` block, never in ordinary runtime
  code a `Suite` executes; every `Fixture.after()` runs even when a case
  throws, without a `finally` clause — this language's grammar has none —
  by structuring the per-case `try`/`catch` to fully absorb any `Error`
  into a `bool` first, so the `after()` loop that follows it is always
  reached;
- `env/var` — nominal environment variables, exact raw-byte access, strict
  UTF-8 text projections and an isolated role fake;
- `env/cwd` — the explicit, stateless ambient working-directory capability,
  with an immutable `FixedCwd` that rejects every change;
- `proc/args` — immutable application argv with exact byte and strict UTF-8
  views plus indexed failures;
- `proc/cmd` — `Cmd`/`RunningCmd` build, run and supervise a child process
  over real OS pipes with no shell and no ambient environment/directory
  inheritance unless explicitly requested (POSIX only);
- `net/address` — immutable validated IPv4/IPv6, socket endpoint and unresolved
  host/port values without exposing native endpoint envelopes;
- `test/more` — stateless reusable assertions over the contextual test
  capability, plus the `AssertionFailure` Error value;
- `io/input`, `io/output`, and `io/closeable` — host-independent transport and
  lifecycle capabilities with typed protocol errors;
- `io/buffer` — explicitly acquired owned byte/string buffers and non-owning
  input/output buffering decorators;
- `io/stdin`, `io/stdout`, and `io/stderr` — concrete process standard-stream
  objects over the native byte boundary;
- `regex` — the immutable `Regex` object over the portable regex substrate
  (Chapter 31.3);
- `runtime/*` — declaration-only native substrate modules (Chapter 27):
  `runtime/time`, `runtime/fs`, `runtime/process`, `runtime/net`,
  `runtime/poll`, `runtime/environment`, `runtime/system`, `runtime/debug`,
  `runtime/regex`, `runtime/stdio`, `runtime/memory`, `runtime/format`, `runtime/crypto`,
  `runtime/tls`, `runtime/utf8`.

Operating-system conventions are values, not raw host strings or constants:

```okkama
import "os" take
    Host, OperatingSystem, Posix,
    Platform, Architecture, PathStyle;

main() is int {
    let host is OperatingSystem = Host();
    let foreign is OperatingSystem = Posix(
        Platform.freebsd,
        Architecture.arm64,
    );

    host.platform();
    host.path_style();
    foreign.path_list_separator(); // ":"
    return 0;
}
```

`Host()` has no retained state and resolves each property through
`runtime/system` only when requested. Known non-Windows hosts use POSIX paths,
LF, and `:`; Windows uses Windows paths, CRLF, and `;`. An unknown host still
has the typed identity `Platform.unknown`, but asking it for path or line
conventions throws `UnsupportedPlatform` instead of making a guess. `Posix`
and `Windows` describe an explicit foreign or test target; their overloaded
constructors only retain enum state.

Paths are syntax values before they become filesystem arguments:

```okkama
import "fs/path" take PathValue, Path, PosixPath, WindowsPath;

main() is int {
    let log is PathValue = Path("var")
        .joined("log")
        .joined("okkama.log");
    let archive = PosixPath("/srv//data/../archive").normalized();
    let artifact = WindowsPath("C:/build/./out/app.exe").normalized();

    log.bytes();
    archive.string();  // "/srv/archive"
    artifact.string(); // "C:\\build\\out\\app.exe"
    return 0;
}
```

`Path` uses `Host().path_style()` when an action is evaluated. It never chooses
a syntax in its constructor, and an unknown host throws `UnsupportedPlatform`.
`PosixPath` and `WindowsPath` are explicit foreign grammars and never inspect
the host. All three constructors only retain their source; no path action
checks existence, follows links, resolves a real path or changes cwd.

`string()` preserves valid source spelling and `bytes()` is its exact UTF-8
encoding. `normalized()` is the only canonicalizing action: it removes
repeated separators and `.`, resolves `..` without crossing a root, removes a
non-root trailing separator and maps an empty relative path to `.`. `parent()`
uses that normalized structure. Roots, `.`, empty normalized values, bare
drive contexts and UNC share roots throw `MissingPathParent`.

`joined(part)` accepts a relative fragment and never discards its base; rooted,
drive-qualified and UNC fragments throw `InvalidPath`. POSIX rejects NUL and
treats backslash as an ordinary character. Windows accepts both separators,
canonicalizes them to backslash, and distinguishes drive-rooted (`C:\\x`),
drive-relative (`C:x`), root-relative (`\\x`) and UNC paths. Only drive-rooted
and complete UNC paths are absolute. Raw non-UTF-8 directory entries from
`runtime/fs` remain `@byte`; this text-originated API does not rewrite them.

`fs/meta` normalizes `stat`/`lstat` into portable, typed metadata:

```okkama
import "fs/meta" take FileKind, Meta, Permissions, MissingPath;
import "fs/path" take Path;

main() is int {
    try {
        let meta is Meta = Meta(Path("README.md"));

        if meta.kind() == FileKind.file {
            meta.size();
            meta.modified();               // Instant, ClockIdentity.system
            meta.permissions().readable();
        }
    }
    catch error is MissingPath {
        error.text();
    }
    return 0;
}
```

`Meta` reads through `lstat`, not `stat`: a followed symlink always resolves
to what it points at, so only `lstat` can ever report `FileKind.link`.
`FileKind` is deliberately narrow — file, directory, link, or other — so
device files, fifos and sockets all fall under `other` rather than growing
the enum. `Permissions` decodes only the owner's read/write/execute bits
from the raw POSIX mode; the provider exposes no ownership or process
identity, so "can the current process access this" cannot be computed
portably, and owner bits are the closest consistently meaningful reading
across POSIX and Windows. `Meta`'s path is caller-supplied, so a
`not_found` failure is the catchable `MissingPath` above; any other native
failure (permission denied, unexpected) is an invariant panic instead, since
this module declares no broader error vocabulary for it.

Durations use one signed, exact nanosecond representation:

```okkama
import "time/duration" take
    Duration, Milliseconds, Seconds, Minutes;

main() is int {
    let timeout is Duration = duration Seconds(30);
    let retry = timeout.plus(duration Milliseconds(500));
    let rewind = (duration Minutes(2)).times(-1);

    retry.nanoseconds();        // 30500000000
    rewind.less(Duration(0));   // true
    return 0;
}
```

`Duration`, `Nanoseconds`, `Milliseconds`, `Seconds`, `Minutes`, and `Hours`
have state-only constructors. Validation, arithmetic, and unit scaling happen
in actions. Negative spans are valid, but every result must fit signed 64-bit
nanoseconds. Overflow throws `InvalidDuration` before arithmetic can wrap or
cross a float boundary. The pure-Okkama module has no clock or sleep side
effect; `time/clock` decides where a nonnegative relative span is
required.

Instants never lose the clock that gives their integer meaning:

```okkama
import "time/duration" take Duration, Milliseconds, Seconds;
import "time/instant" take ClockIdentity, Instant;
import "time/clock" take Clock, MonotonicClock, OffsetClock;
import "time/date" take Date, TimeZone;
import "time/timer" take
    ScheduledTimer, Timer,
    ScheduledPeriodicTimer, PeriodicTimer;

main() is int {
    let clock is Clock = MonotonicClock();
    let started = clock.now();
    let shifted = OffsetClock(clock, duration Seconds(1)).now();
    let elapsed = shifted.since(started);

    let leap = Date(2024, 2, 28).plus_days(1);
    let epoch = TimeZone("UTC").at(
        Instant(0, ClockIdentity.system)
    );

    let timeout is Timer = timer ScheduledTimer(duration Milliseconds(10));
    timeout.wait();

    let ticker is PeriodicTimer =
        timer ScheduledPeriodicTimer(duration Seconds(1));
    ticker.cancel();

    leap.string();       // "2024-02-29"
    epoch.string();      // "1970-01-01T00:00:00Z"
    elapsed.nanoseconds();
    ticker.cancelled();  // true
    return 0;
}
```

`Instant` stores signed-64 nanoseconds plus `ClockIdentity.system` or
`ClockIdentity.monotonic`. Shifts preserve that identity; ordering or
subtracting different identities throws `IncompatibleClocks`. `SystemClock`
reads Unix time, `MonotonicClock` is for measurements/deadlines, and
`OffsetClock` decorates either without changing its identity. Negative spans
remain valid values, while `Clock.sleep` rejects an invalid relative sleep as
`InvalidSleepDuration`.

`Date` implements the proleptic Gregorian calendar for years `1..9999`.
`DateTime` retains its exact source instant. The initial `TimeZone` policy is
deliberately UTC-only; other names throw `UnknownTimeZone` until versioned
time-zone data is bundled rather than inherited silently from the host.

Timers are live resources, not immutable values. `ScheduledTimer` and
`ScheduledPeriodicTimer` are state-only schedule values; their `#projected timer()`
projection acquires an owned resource. `cancel()` is an idempotent command and
`cancelled()` is its query. Waiting after cancellation throws
`CancelledTimer`. Periodic timers use an anchored fixed-rate schedule and skip
missed ticks; destruction closes their opaque runtime handle.

Code written against the `Clock` role can be tested without a real clock
through `test/fake_clock`:

```okkama
import "test/fake_clock" take FakeClock;
import "time/clock" take Clock;
import "time/duration" take Duration;
import "time/instant" take ClockIdentity, Instant;

main() is int {
    let clock is Clock = FakeClock(Instant(0, ClockIdentity.monotonic));

    clock.sleep(duration Seconds(1));      // advances instantly, never blocks
    clock.advance(Duration(-500)); // rewinding is allowed; it is a test tool
    clock.set(Instant(1000000000, ClockIdentity.monotonic));

    clock.now();
    return 0;
}
```

`FakeClock(start)` fixes its simulated `ClockIdentity` for its lifetime.
`sleep` is `advance` by another name, so it never blocks a test, but it still
rejects a negative duration as `InvalidSleepDuration` like a real clock
would; `advance` itself accepts a signed `Duration` since it is a test-only
control, not part of `Clock`. `set` throws the existing `IncompatibleClocks`
if given an instant with a different identity. Since Okkama fields cannot be
reassigned outside a constructor, the current instant is not stored as one:
it lives in an owned `runtime/memory` byte cell, the same way `io/buffer`
owns its byte content, encoded through `encoding/binary`'s big-endian codec.

`test/benchmark` times a `BenchmarkCase` against an injected `Clock` and
summarizes the samples:

```okkama
import "test/benchmark" take BenchmarkCase, Benchmark, BenchmarkResult;
import "time/clock" take MonotonicClock;
import "strings/parse" take ParsedInt;

object ParseWorkload does BenchmarkCase {
    ParseWorkload();
    run() is void {
        value ParsedInt("12345");
    }
}

main() is int {
    let result is BenchmarkResult =
        Benchmark(ParseWorkload(), MonotonicClock(), 1000).measure();

    result.iterations();
    result.average();
    result.minimum();
    result.maximum();
    result.total();
    return 0;
}
```

The clock is a constructor argument rather than fixed to `MonotonicClock` so
`Benchmark` can be driven by `test/fake_clock` in tests and asserted against
exact statistics instead of tolerating real timing noise. `BenchmarkResult`
is independently constructible from an already-computed `@Duration`, so its
statistics — including `average()`, which avoids the float-precision risk of
plain integer division the same way `encoding/hex`/`encoding/base64` do —
are unit-tested without any clock at all. `InvalidBenchmark` covers a
non-positive iteration count and a run whose clock reports an earlier
instant afterward than before, which is a real, reachable case (not just
defensive code) when a `test/fake_clock`-backed case rewinds its own clock.

Environment variables preserve the raw host boundary without making ordinary
application code procedural:

```okkama
import "env/var" take Environment, Env, EnvVar, EnvEntry;

main() is int {
    let environment is Environment = Env();
    let home = EnvVar("HOME");

    if environment.has(home) {
        environment.value(home); // strict UTF-8
        environment.bytes(home); // exact bytes
    }

    let snapshot is @EnvEntry = environment.variables();
    snapshot.size();
}
```

`EnvVar` and `EnvEntry` constructors only retain their arguments. Names are
validated when observed: they are nonempty UTF-8 without NUL or `=`. Values
may be empty or non-UTF-8 raw bytes, but cannot contain NUL. Text projection
throws `InvalidEnvEncoding`; no replacement or normalization occurs.
`MissingEnvVar` distinguishes absence from a present empty value. `Env`
mutates the ambient process, whereas `Environment.fake()` is test-only,
instance-local and returns detached byte-sorted snapshots.

The current working directory is its own explicit capability, `env/cwd`, not
an incidental static function on `fs`:

```okkama
import "env/cwd" take WorkingDirectory, Cwd, FixedCwd, InvalidWorkingDirectory;
import "fs/path" take Path;

main() is int {
    let cwd is WorkingDirectory = Cwd();
    let previous is Path = cwd.path();

    try {
        cwd.change(Path("/tmp"));
    }
    catch error is InvalidWorkingDirectory {
        error.text();
    }
    return 0;
}
```

`Cwd` is stateless: `path()` always re-reads the host, and `change()` always
calls `chdir` directly, matching how `SystemClock`/`Env`/`Dns` never cache
ambient state either. A `chdir` failure — a missing target, a permission
error, anything the caller could plausibly retry with a different path —
becomes one sanitized `InvalidWorkingDirectory`; failing to *read* the
ambient directory is instead an invariant panic, the same treatment
`SystemClock.now()` gives an unreadable host clock, since no caller input
could be at fault. `FixedCwd(start)` is immutable rather than a mutable test
double: `change()` always throws `InvalidWorkingDirectory`, which doubles as
a way to assert in a test that some code path never tries to change
directories at all.

Application arguments use a separate stateless object over the immutable
runtime snapshot:

```okkama
import "proc/args" take Args;

main() is int {
    let arguments = Args();
    arguments.size();
    arguments.bytes();

    if arguments.size() > 0 {
        arguments.at(0);
    }
    return 0;
}
```

`bytes()` preserves every exact argument, including empty and non-UTF-8
values. `strings()` decodes the complete snapshot strictly and reports the
first malformed value as `InvalidArgumentEncoding(index)`. `at(index)` uses
the same policy for one value and throws `MissingArgument(index)` for negative
or upper-bound indices. The source path used by the CLI is not included.
`Args()` performs no native work in its constructor; the runtime provider
already owns the defensive snapshot.

Persistent queues and stacks start from explicitly typed empty values. Every
derived value leaves the earlier version unchanged, and both work directly
with `for`:

```okkama
import "collections/queue" take Queue, EmptyQueue;
import "collections/stack" take Stack, EmptyStack;

main() is int {
    let empty_queue is Queue<int> = EmptyQueue();
    let queue = empty_queue.enqueued(1).enqueued(2);
    queue.head();             // 1: FIFO

    let empty_stack is Stack<int> = EmptyStack();
    let stack = empty_stack.pushed(1).pushed(2);
    stack.top();              // 2: LIFO

    for queue -> value { value; } // 1, then 2
    for stack -> value { value; } // 2, then 1
    return 0;
}
```

Queue uses a private two-list representation and reverses its rear list only
when the front is exhausted, giving amortized O(1) enqueue/tail. Stack is a
semantic wrapper over `List<T>` and reuses its storage; its cursor traverses
only the public Stack contract. Empty
`head`/`tail` and `top`/`popped` throw typed contextual errors.

Formatting is an immutable projection over direct positional values or an
explicit named-value map:

```okkama
import "format" take
    Formatted, NamedFormatted, PrintfFormatted,
    InvalidFormat, MissingFormatValue, InvalidPrintf;

main() is int {
    let named is %string:any = ["name": "Ada", "count": 3];

    string Formatted("{}: {}", "Ada", 3);            // "Ada: 3"
    string NamedFormatted("{name}: {count}", named);  // "Ada: 3"
    string PrintfFormatted("exact=%8r hex=%04x", rat(1, 3), 15);
    // "exact=     1/3 hex=000f"
    return 0;
}
```

`Formatted` consumes only empty `{}` fields in order. `NamedFormatted` accepts
ASCII identifier fields such as `{name}` and `{count_2}`; they may repeat.
Both forms use `{{` and `}}` for literal braces and permit unused supplied
values. The `Formatted` variadic constructor retains positional values as
private `@any` state, so the caller creates no auxiliary array. Malformed syntax throws
`InvalidFormat` with the first zero-based
Unicode code-point offset. A missing value throws `MissingFormatValue` whose
text retains the positional index or named key. Constructors only retain the
pattern and collection; parsing and native rendering happen in `string()`.
`PrintfFormatted` provides the portable sprintf-compatible subset
`%s/%c/%d/%i/%u/%o/%x/%X/%b/%f/%e/%E/%g/%G`, `%%`, and exact `%r`.
Numeric width/precision and ordinary flags are supported. Dynamic `*`,
positional `$`, `%n`, `%p`, `%v`, and host length modifiers are rejected as
`InvalidPrintf`; `%r` accepts width and `-` but never precision.

Plain templates are reusable string objects, not embedded Okkama programs:

```okkama
import "template/plain" take Template, TemplateBinding, TemplateValues;

main() is int {
    let values = TemplateValues(
        TemplateBinding("name", "Ada"),
        TemplateBinding("language", "Okkama"),
    );
    Template("Hello, <%= name %> from <%= language %>!").string(values);
    return 0;
}
```

`<%= name %>` uses the template escaping policy and `<%== name %>` is the raw
channel. For `template/plain` that policy is identity, so both preserve exact
strings; `template/html` (below) keeps both tags but decides escaping by the
bound value's type instead, so they stay lexically identical there too.
`<%# ... %>` is a non-nesting comment ending at the first `%>`, and `<%%`
emits literal `<%`. Ordinary braces remain literal, making JSON and source-code
templates usable without brace escaping. Other `<% ... %>` forms are reserved
and invalid in v1—templates never evaluate Okkama or host code.

Placeholder names are exact ASCII identifiers with optional ASCII space/tab around them.
The whole source is validated before lookup; invalid syntax reports a
zero-based Unicode code-point offset. Missing and duplicate bindings are
separate typed errors, surplus bindings are allowed, and inserted strings are
never interpreted recursively. All constructors only retain state; parsing
and production happen in the noun query `string(values)`.

`template/html` renders the same tag grammar into HTML, escaping by default:

```okkama
import "template/html" take
    HtmlTemplate, HtmlBinding, HtmlValues, SafeHtml, EscapedHtml;

main() is int {
    let comment is string = "<script>alert('xss')</script>";
    let values is HtmlValues = HtmlValues([
        HtmlBinding("comment", safe EscapedHtml(comment)),
    ]);
    HtmlTemplate("<div><%= comment %></div>").render(values);
    // "<div>&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;</div>"
    return 0;
}
```

`HtmlBinding` takes a `SafeHtml` value, not a plain `string` — there is no
code path for binding unescaped text at all. `safe EscapedHtml(text)` escapes
`&`, `<`, `>`, `"` and `'` (ampersand first, so text that already contains
`&amp;` is not doubled into `&amp;amp;`); `SafeHtml(text)` is the explicit,
visible assertion that some text — typically the already-rendered output of
a nested `HtmlTemplate.render()` — is safe to insert as-is:

```okkama
let partial is string = HtmlTemplate("<em><%= word %></em>")
    .render(HtmlValues([HtmlBinding("word", safe EscapedHtml("safe & sound"))]));
HtmlTemplate("<section><%== body %></section>")
    .render(HtmlValues([HtmlBinding("body", SafeHtml(partial))]));
// "<section><em>safe &amp; sound</em></section>"
```

`HtmlTemplate.render` runs the source through the exact same `Template`
lexer `template/plain` uses; `InvalidTemplate` becomes `InvalidHtmlTemplate`,
while `MissingTemplateValue`/`DuplicateTemplateValue` are reused unchanged,
since a missing or duplicate binding name is not an HTML-specific concept.

Binary protocols remain ordinary immutable Okkama code. A reader always names
its offset and endianness; output is composed from values whose types name the
representation:

```okkama
import "encoding/binary" take
    BinaryReader, BinaryPart, BinarySequence,
    Unsigned16BigEndian, Signed32LittleEndian;

main() is int {
    let parts is @BinaryPart = [
        Unsigned16BigEndian(258),
        Signed32LittleEndian(-2),
    ];
    let packet is @byte = bytes BinarySequence(parts);

    let reader = BinaryReader(bytes: packet);
    reader.u16_be(0);               // 258
    reader.i32_le(2);               // -2
    reader.bytes(0, 2);             // [byte(1), byte(2)]
    return 0;
}
```

The `_be` and `_le` suffixes are mandatory; host byte order is never consulted.
Unsigned parts reject values outside their width, signed values use exact
two's-complement boundaries, and short reads throw `EndOfBinaryData`. Negative
offsets, negative slice sizes, and out-of-range values throw
`InvalidBinaryValue`. Constructors only retain their source values; checked
encoding happens in `bytes()`. `BinarySequence` reduces immutable parts into
persistent byte chunks and performs one linear `Iterable<byte>` identity map
into `@byte`.

Text and bytes cross through an explicit UTF-8 projection. No call silently
chooses a character encoding:

```okkama
import "encoding/utf8" take Utf8Encoded, Utf8Decoded, InvalidUtf8;

main() is int {
    let bytes is @byte = bytes Utf8Encoded("Hello, Привет, 猫, 😀");
    let text is string = string Utf8Decoded(bytes);

    try {
        string Utf8Decoded([byte(65), byte(255)]);
    }
    catch error is InvalidUtf8 {
        error.offset(); // 1: byte offset of the malformed sequence
    }
    return 0;
}
```

Encoding emits canonical UTF-8 without a BOM or normalization. Decoding is
strict: it never inserts replacement characters, and overlong forms,
surrogates, values above `U+10FFFF`, invalid continuations and truncated tails
all throw `InvalidUtf8`. The retained offset is the start of the first
ill-formed byte sequence. Both projection objects are immutable, so repeated
conversion is deterministic and does not cache hidden mutable state.

Base64 is a byte encoding, not a character encoding and not encryption. Its
two projections therefore compose explicitly with UTF-8 when text is
involved:

```okkama
import "encoding/base64" take Base64Encoded, Base64Decoded, InvalidBase64;
import "encoding/utf8" take Utf8Encoded, Utf8Decoded;

main() is int {
    let encoded is string = string Base64Encoded(bytes Utf8Encoded("Привет"));
    let decoded is string = string Utf8Decoded(bytes Base64Decoded(encoded));

    try {
        bytes Base64Decoded("Zh==");
    }
    catch error is InvalidBase64 {
        error.offset(); // 1: non-zero unused bits
    }
    return 0;
}
```

The first codec version accepts exactly canonical padded RFC 4648 with the
standard `+` and `/` alphabet. It does not ignore whitespace and does not
accept URL-safe, unpadded or MIME forms. Padding shape and unused tail bits
are validated, so two different accepted strings cannot represent the same
byte sequence. Decoding composes immutable `BinaryBytes` parts into one
`BinarySequence`; no mutable output buffer or hidden native Base64 codec is
introduced.

Hexadecimal uses the same explicit byte boundary with a smaller canonical
surface:

```okkama
import "encoding/hex" take HexEncoded, HexDecoded, InvalidHex;

main() is int {
    let encoded is string = string HexEncoded([
        byte(222), byte(173), byte(190), byte(239),
    ]); // "deadbeef"
    let decoded is @byte = bytes HexDecoded("DEADBEEF");

    try {
        bytes HexDecoded("0x");
    }
    catch error is InvalidHex {
        error.offset(); // 1: prefixes are not part of the codec
    }
    return 0;
}
```

The encoder always emits two lowercase characters per byte. The decoder also
accepts uppercase `A-F`, but rejects odd lengths, whitespace, `0x` prefixes,
separators and non-ASCII characters. Odd length reports the missing nibble
position at `text.length()`; other failures identify the first invalid
code-point offset. Both transformations are immutable, linear and written in
Okkama without a native codec.

JSON is represented by nominal objects instead of maps containing `any`:

```okkama
import "encoding/json" take
    Json, JsonKind, JsonMember, JsonObject, JsonArray,
    JsonString, JsonNumber, JsonBool, JsonNull, JsonSource, JsonReader,
    InvalidJson, UnexpectedJsonKind;

main() is int {
    let values is @Json = [
        JsonString("Ada"),
        JsonNumber("1e400"),
        JsonBool(true),
        JsonNull(),
    ];
    let generated = JsonObject([
        JsonMember("values", JsonArray(values)),
    ]);
    let parsed is Json = json JsonSource(
        "{\"emoji\":\"\\uD83D\\uDE00\",\"count\":3}",
    );
    [generated.encoded(), parsed.encoded()];
    return 0;
}
```

`Json`, implemented by the six JSON value families, exposes `kind()` and
`encoded()`. `kind()` returns the exact `JsonKind` without inspecting encoded
text. `JsonMember`, `JsonObject` and `JsonArray` preserve order and
nominal types. Duplicate member names are rejected rather than acquiring a
silent first-wins or last-wins policy; missing members and array indexes also
have distinct typed failures.

A `JsonNumber` stores its exact RFC 8259 decimal spelling. It does not parse
through an Okkama numeric primitive, so `1e400` remains valid JSON and
re-encodes verbatim. `int()` and `float()` are explicit checked views;
`InvalidJsonNumberConversion` means only that the requested bounded view is
unavailable, not that the JSON number is invalid.

The parser accepts all JSON escapes and valid UTF-16 surrogate pairs, rejects
lone surrogates and unescaped controls, and encodes controls canonically.
`InvalidJson` reports a zero-based code-point offset into the original source
plus one-based line and column. CRLF is one line break while the offset still
counts both original code points. Construction only retains state; parsing,
number validation and encoding occur in queries, entirely in Okkama without a
host JSON codec.

For schema-oriented reading, `JsonReader` parses a source and acts as an
immutable cursor:

````okkama
let reader = JsonReader(```
{
    "name": "Ada",
    "address": {"city": "London"},
    "users": [{"age": 37}],
    "active": null
}
```);

let name is string = reader.required_string("name");
let city is string = reader.required("address").required_string("city");
let age is int = reader.required("users").at(0).required_int("age");
let active is bool = reader.optional_bool("active", false);
````

`required(name)` and `at(index)` return new readers; the original reader never
changes. `string()`, `int()`, `float()` and `bool()` project the current node.
`required_*` requires a present member of the requested kind.
`optional_*(name, fallback)` returns its same-typed fallback only when the
member is absent or JSON null. A present non-null wrong kind throws
`UnexpectedJsonKind`, whose path identifies locations such as
`$.users[0].age`. This facade reads JSON only. Encoding remains the explicit
construction of `JsonObject`, `JsonMember`, `JsonArray` and scalar JSON
values; private domain-object state is never reflected automatically.

`encoding/csv` reads and writes RFC 4180-shaped rows over `Input`/`Output`
under an explicit, injectable dialect:

```okkama
import "encoding/csv" take CsvDialect, CsvReader, CsvWriter, CsvRow;
import "io/buffer" take Buffer, BufferAllocation;

main() is int {
    let dialect is CsvDialect = CsvDialect(); // ",", "\"", "\r\n", header: true

    let sink is Buffer = buffer BufferAllocation();
    let writer is CsvWriter = CsvWriter(sink, dialect);
    writer.write(["id", "bio"]);
    writer.write(["1", "wrote the first algorithm, comma included"]);
    writer.flush();

    let rows is @@string = CsvReader(sink, dialect).rows();
    let row is CsvRow = CsvRow(rows[0], rows[1], 2);
    row.value("bio");
    return 0;
}
```

`CsvDialect()` supplies RFC 4180 defaults; the full constructor takes an
explicit delimiter, quote character, line separator and header flag — all
four as `string`, not `char`: `char` has no defined conversion to or from
`string` in this implementation (confirmed directly — concatenation,
comparison and every action call against a bare `char` all fail), and is
used nowhere else in the stdlib, so a single-character `string` is used
instead, matching every other place that already represents one character
this way. A doubled quote (`""`) inside a quoted field is a literal quote;
content between a closing quote and the next delimiter throws `InvalidCsv`.
`CsvReader.rows()` returns every row undifferentiated — `CsvDialect.header()`
is metadata a caller acts on, not something `CsvReader` branches on — so
`CsvRow` is a separate step that pairs an already-read header with an
already-read data row for named/indexed access.

`encoding/toml` covers a practical, single-line-document subset of TOML
v1.0.0 — the shape real configuration files actually use, not the complete
grammar:

```okkama
import "encoding/toml" take TomlDocument, TomlSource, TomlValue;

main() is int {
    let document is TomlDocument = document TomlSource(
        "name = \"app\"\n"
            + "\n"
            + "[server]\n"
            + "port = 8080\n"
            + "tags = [\"a\", \"b\"]\n",
    );
    document.value("name");            // TomlString
    document.value("server.port");     // TomlInteger, by dotted path
    document.value("server.tags");     // TomlArray
    document.string();                 // re-renders with [server] headers
    return 0;
}
```

Multi-line strings/arrays, `[[array of tables]]`, and dotted keys inside a
`[table]` block exploding into further nested tables (`[a]\nb.c = 1` stays
one entry literally named `"b.c"`; only an explicit `[a.b]` header nests)
are all out of scope for this version — each is a deliberate reduction, not
an oversight, recorded with its reasoning in `encoding-toml-21072026.md`.
`TomlDateTime` wraps `time/date.Date` — TOML's Local Date form only; the
other three date/time forms don't map cleanly onto an existing Okkama value
yet. Dotted-path lookup could not be built as a tree of nested `TomlTable`
values descended one segment at a time: once an intermediate value comes
back as the role-typed `TomlValue`, nothing lets it be treated as a
`TomlTable` again to keep descending, since Okkama has no runtime
role-to-concrete downcast. The parser instead emits a flat list of
fully-expanded `(header, key, value)` records — used directly for `.value()`
lookup by path equality — while `.string()` groups that same list by header
to render `[table]` sections, so descending through an opaque `TomlValue`
is never needed.

Cryptographic randomness is exposed as an injectable capability rather than
an object that stores one request size:

```okkama
import "crypto/random" take
    SecureByteSource, SecureRandom,
    InvalidSecureRandomSize, UnavailableSecureRandom;

main() is int {
    let source is SecureByteSource = SecureRandom();

    try {
        source.bytes(32); // exact OS-CSPRNG bytes
    }
    catch error is InvalidSecureRandomSize {
        error.requested();
    }
    catch error is UnavailableSecureRandom {
        error.text();
    }
    return 0;
}
```

Each `bytes(size)` call is independent. The portable range is zero through
16 MiB; zero needs no entropy device, while an invalid request fails before
native dispatch. Provider failures are sanitized to one
`UnavailableSecureRandom` object, and no language PRNG is used as a fallback.
In test mode `SecureByteSource.fake()` returns an exact, predictable sequence
of zero bytes, making dependency injection explicit without pretending the
fake is secure.

`crypto/digest` hashes bytes or a full `Input` stream over the reviewed
SHA-2 primitive:

```okkama
import "crypto/digest" take
    Digest, Sha256, Sha384, Sha512, DigestMismatch, UnsupportedDigest;

main() is int {
    let digest is Digest = Sha256([byte(1), byte(2), byte(3)]);
    digest.bytes();  // exact 32-byte SHA-256 output
    digest.hex();    // lowercase canonical hexadecimal
    digest.base64(); // RFC 4648 padded base64

    let expected is @byte = [];
    try {
        confirm digest.bytes() == expected else throw DigestMismatch();
    }
    catch error is DigestMismatch {
        error.text();
    }
    return 0;
}
```

`Sha256`, `Sha384` and `Sha512` compute their result once at construction and
implement `Digest` identically from there; `hex()` and `base64()` compose the
existing `encoding/hex`/`encoding/base64` codecs rather than reimplementing
either alphabet. Each also has a streaming constructor,
`Sha256(source is Input, chunk_size is int)`, that reads the input to
completion in `chunk_size`-sized reads before hashing — the reviewed
`runtime/crypto` ABI is one-shot, so there is no incremental digest handle to
update as bytes arrive. The explicit chunk size doubles as the reason the
streaming constructor can coexist with the bytes constructor at all: Okkama
constructor overload resolution is arity-based, not type-based, so two
same-arity constructors of different parameter types are always ambiguous.

Every native digest-provider failure is translated to one
`UnsupportedDigest()` without leaking a native code or message, mirroring how
`crypto/random` sanitizes provider failures. `DigestMismatch` is exported
verification vocabulary rather than something the module throws itself:
comparing digest bytes needs no dedicated action, since `@byte == @byte` is
already structural equality, so callers use `confirm ... else throw
DigestMismatch();` directly, as shown above.

`crypto/hmac` adds a keyed variant with constant-time verification:

```okkama
import "crypto/hmac" take HmacSha256, InvalidMacKey;

main() is int {
    let mac is HmacSha256 = HmacSha256(
        [byte(1), byte(2), byte(3)], // key
        [byte(4), byte(5), byte(6)], // message
    );
    mac.bytes();
    mac.hex();
    mac.matches(mac.bytes()); // true

    try {
        HmacSha256([], [byte(1)]);
    }
    catch error is InvalidMacKey {
        error.text(); // empty keys are rejected before native dispatch
    }
    return 0;
}
```

`matches` calls `runtime/crypto.secure_equal`, not `==`: unlike a plain
digest, a MAC is verified against a value an attacker may have crafted, and
a length/timing-leaking comparison could help forge one byte at a time.
`HmacSha256`/`384`/`512` reuse `crypto/digest`'s `UnsupportedDigest` for
native failure translation rather than declaring a duplicate — they call
`runtime/crypto.hmac` with one of their own three fixed algorithm strings
exactly the way `crypto/digest`'s objects do, so the same "any native
failure here is a provider invariant, not a caller mistake" reasoning
applies unchanged.

`socket/socket` is pure shared vocabulary ahead of the concrete socket
resources (`socket/tcp`/`udp`/`unix`) that will depend on it: the `Socket`
role (`local() is SocketAddress`), three explicit option enums in place of
boolean toggles —

```okkama
import "socket/socket" take
    AddressReuse, KeepAlivePolicy, PacketDelay, SocketOptions, SocketTimeout;
import "time/duration" take Seconds;

main() is int {
    let options is SocketOptions = SocketOptions(
        AddressReuse.enabled,
        KeepAlivePolicy.enabled,
        PacketDelay.disabled, // TCP_NODELAY
    );
    let timeout is SocketTimeout = SocketTimeout(
        duration Seconds(5), duration Seconds(30), duration Seconds(30),
    );
    return 0;
}
```

— plus `SocketClosed` (fixed text) and `SocketFailure(reason)` for
`socket/tcp`/`udp`/`unix` to throw once they exist. It has no native
surface of its own.

`sql/value` is the same kind of pure vocabulary module ahead of
`sql/statement`/`sql/query`: eight thin `SqlValue` wrappers (`SqlInt`,
`SqlFloat`, `SqlString`, `SqlBytes`, `SqlBool`, `SqlDate`, `SqlDateTime`,
`SqlNull`), each reporting its own fixed `SqlType`, plus `SqlValues` to
hold a mixed, role-typed bound-parameter list —

```okkama
import "sql/value" take SqlType, SqlValue, SqlInt, SqlString, SqlNull, SqlValues;

main() is int {
    let bound is @SqlValue = [SqlInt(42), SqlString("Ada"), SqlNull()];
    let params = SqlValues(bound);

    let final_value = params.values()[params.values().size() - 1];
    if final_value.sql_type() == SqlType.null_value {
        "last bound parameter is an explicit SQL NULL";
    }
    else {
        "unexpected";
    }
    return 0;
}
```

`SqlNull` is a real, explicit database value a caller constructs and
matches on deliberately — never Okkama's own (nonexistent) implicit null.
`SqlType`'s eighth member is named `null_value` rather than the draft's
`null`, since `null` is a reserved keyword in this null-free language.

`http/status` validates an HTTP status code/reason pair and classifies it
by numeric range, with named shortcuts for the handful of statuses used
everywhere:

```okkama
import "http/status" take Status, Ok, NotFoundStatus, InvalidHttpStatus;

main() is int {
    let found is Status = status Ok();
    let missing is Status = status NotFoundStatus();

    if found.successful() && missing.client_error() {
        "both classified correctly";
    }
    else {
        "unexpected";
    }
    return 0;
}
```

`Ok`, `Created`, `BadRequest`, `NotFoundStatus` and `InternalServerError`
are zero-argument objects, each projecting (`status Ok()`, `status NotFoundStatus()`,
...) into a full `Status` — the same named-constructor projection `time/duration`
uses for `duration Seconds(5)`/`duration Hours(5)`, just with no argument to carry. Any
other status code is `Status(code, reason)` directly; the constructor
rejects a code outside `100..599` or an empty reason with one
`InvalidHttpStatus`.

`http/headers` is an immutable, case-insensitive header collection that
never loses a header's original spelling, plus a small add/remove
composition pair:

```okkama
import "http/headers" take HeaderName, Header, Headers, ExtendedHeaders, FilteredHeaders;

main() is int {
    let base is Headers = Headers([
        Header(HeaderName("Content-Type"), "text/html"),
        Header(HeaderName("X-Request-Id"), "abc123"),
    ]);

    let with_auth is Headers = headers ExtendedHeaders(
        base,
        Header(HeaderName("Authorization"), "Bearer token"),
    );
    let redacted is Headers = headers FilteredHeaders(with_auth, HeaderName("authorization"));

    redacted.has(HeaderName("Authorization")); // false — filtered out
    redacted.first(HeaderName("content-type")); // "text/html" — untouched
    return 0;
}
```

Lookups (`has`/`first`/`all`) compare `HeaderName`s case-insensitively, but
`entries()` always reports each header's original spelling: `Headers`
never compares two `HeaderName`s with `==` directly (Okkama's default
object equality is deep-structural over every field, so two names that
only differ by case aren't `==`) — it folds both sides to lowercase first.
`Header`'s constructor trims a value's surrounding whitespace and rejects
any `\r`/`\n` it still contains as `InvalidHeaderValue`, closing off
classic HTTP header/response-splitting injection. `ExtendedHeaders` adds
one header to the end; `FilteredHeaders` removes every header matching a
given name — both return a fresh `Headers` and never modify their source.

`log/event` is the simplest module in this whole stdlib: an immutable
structured log event with no validation beyond one non-empty field name:

```okkama
import "log/event" take LogLevel, LogField, LogEvent;
import "time/instant" take Instant, ClockIdentity;

main() is int {
    let fields is @LogField = [LogField("user", "ada"), LogField("attempt", "3")];
    let event = LogEvent(
        LogLevel.warning,
        "retrying request",
        fields,
        Instant(0, ClockIdentity.system),
    );

    event.level();    // LogLevel.warning
    event.message();  // "retrying request"
    return 0;
}
```

`LogEvent`'s constructor stores its four values verbatim — an empty
`message` or an empty `fields` array are both legitimate events, and every
`LogLevel` (`trace`/`debug`/`info`/`warning`/`error`/`fatal`) is already an
exhaustive, closed set, so there is nothing left to validate at this
layer. Only `LogField` rejects anything, and only an empty `name` (with
`InvalidLogField`) — a nameless field would be useless for structured
lookup by the `log/formatter` this module is designed to feed.

`db/result` is the outcome of a non-row-producing statement
(`INSERT`/`UPDATE`/`DELETE`), and uses arity to encode whether a last
insert id even exists rather than a nullable field:

```okkama
import "db/result" take DbResult, MissingLastId;

main() is int {
    let inserted = DbResult(1, 42); // affected rows, last insert id
    let updated = DbResult(3);      // affected rows only — no last id here

    inserted.last_id();      // 42
    updated.has_last_id();   // false

    try {
        updated.last_id();
    }
    catch error is MissingLastId {
        error.text(); // "this statement did not produce a last insert id"
    }
    return 0;
}
```

The two-argument constructor is for statements that produce a real last
id; the one-argument constructor is for the ones that never do (`UPDATE`,
`DELETE`). `has_last_id()` reports which shape built the value, and
`last_id()` throws `MissingLastId` rather than silently returning `0` when
called on a result that never had one — the same explicit-over-implicit
choice already used for `SqlNull` and `fs/meta.MissingPath`.

`db/row` closes out this phase of the stdlib: name/index access to one
query-result row's `SqlValue`s —

```okkama
import "db/row" take Row, MissingColumn;
import "sql/value" take SqlValue, SqlInt, SqlString, SqlType;

main() is int {
    let row = Row(["id", "name"], [SqlInt(1), SqlString("Ada")]);

    row.value("id").sql_type();  // SqlType.integer
    row.at(1).sql_type();        // SqlType.string

    try {
        row.value("missing");
    }
    catch error is MissingColumn {
        error.text(); // "missing column: missing"
    }
    return 0;
}
```

`Row.value()`/`at()` return the `SqlValue` role, not a concrete
`SqlInt`/`SqlString` — Okkama has no runtime downcast from a role back to
a concrete type, so `Row` can inspect no more than `sql_type()`. This is
exactly why the module also exports `InvalidColumnType`: not something
`Row` ever throws itself, but ready-made vocabulary for a domain adapter
built on top (the kind of `UserRow(row).user()` wrapper this module
deliberately leaves to callers) to throw once *it* checks a looked-up
value's `sql_type()` against what it expected — the same "exported, not
internally thrown" shape `crypto/digest.DigestMismatch` already uses.

`fs/file` is the first module to touch real file I/O rather than pure
in-memory vocabulary — a whole-file convenience object plus a streaming
handle:

```okkama
import "fs/file" take File, OpenedFile, MissingFile;
import "fs/path" take Path;
import "encoding/utf8" take Utf8Encoded, Utf8Decoded;

main() is int {
    let file = File(Path("notes.txt"));
    file.write_string("Hello, Okkama!");
    file.string();     // "Hello, Okkama!"
    file.exists();     // true
    file.remove();

    let opened = OpenedFile(Path("stream.bin"));
    opened.write(bytes Utf8Encoded("streamed"));
    opened.seek(0);
    string Utf8Decoded(opened.bytes(64));  // "streamed"
    opened.close();
    return 0;
}
```

`File.bytes()` reads a whole file in one native call sized exactly by
`Meta(path).size()`, rather than looping and concatenating chunks — arrays
have no append, so merging several would need the same
`List<T>`-building idiom used elsewhere, and doing that byte-by-byte for
an entire file's contents would be far too slow. A real local read
essentially always returns the full requested size or genuine EOF, never
an arbitrary short read, so a mismatch panics as a provider-invariant
violation instead of silently returning truncated content.
`OpenedFile.bytes(limit)` needs none of this: `Input`'s own contract is
already "at most `limit` bytes, EOF means empty," exactly what one native
read call provides directly.

`OpenedFile`'s `closed()` has no native counterpart to query — a
`FileHandle` only knows how to open/read/write/seek/flush/close, never
"am I closed?" — so it's backed by a `runtime/memory.ByteMemoryHandle`
sentinel, the same technique `test/fake_clock` uses for mutable state
outside a constructor. Both the real native `close()` and the sentinel's
own close are already idempotent, so calling `OpenedFile.close()` twice is
harmless.

`fs/dir` rounds out Phase 6's filesystem work with creation, listing,
recursive walking and removal:

```okkama
import "fs/dir" take Dir, DirEntry, DirectoryWalk;
import "fs/file" take File;
import "fs/path" take Path;

main() is int {
    let dir = Dir(Path("project/nested"));
    dir.create_all();
    File(Path("project/a.txt")).write_string("a");
    File(Path("project/nested/b.txt")).write_string("b");

    let walked is @DirEntry = DirectoryWalk(Path("project")).entries();
    walked.size(); // 3 — the file, the nested directory, and its file

    Dir(Path("project")).remove_all();
    return 0;
}
```

Both `remove_all()` and `DirectoryWalk` classify every entry through
`Meta`'s `lstat`-backed `kind()`, never `stat` — a symlink pointing at a
directory always reports `FileKind.link`, never `FileKind.directory`, so
it is always treated as a leaf (deleted or listed, never descended into).
Recursion happens once per directory *nesting level* (realistically single
digits), never once per *entry* (which can be arbitrarily large) — breadth
at each level is handled iteratively, not recursively.

Building this module surfaced a genuine, previously unknown **quadratic
performance bug in the interpreter itself**: any `let` statement executed
while a large persistent structure (like a growing `List<T>`) is in scope
pays a cost proportional to that structure's current size, because the
interpreter's ownership-tracking machinery re-walks every live variable's
entire reachable object graph on every single `let`, without pruning
already-accounted-for subgraphs. Over a loop, that's O(n²). It reproduces
for `int`, `string`, and `@byte` alike (composite types cost more per
node, but the same shape) — it is not specific to generics or to
`collections/list`. `fs/dir.ok` routes around it by keeping loop bodies
free of intermediate `let`s while an accumulator is in scope (confirmed
directly: 500/1000/2000-file directory listings now cost
3.57s/7.67s/16.6s — genuinely linear). The bug itself was root-caused to
specific lines in `Interpreter.pm` but deliberately left unfixed here —
patching core ownership/lifetime semantics is a different kind of change
than shipping a stdlib module.

`socket/tcp` is the first module to touch real network I/O — a client
socket, a listener, and a DNS-driven ordered-address-attempt helper, all
exercised in this session's tests over real loopback TCP connections:

```okkama
import "socket/tcp" take TcpSocket, TcpListener;
import "net/address" take AddressFamily, IpAddress, SocketAddress;

main() is int {
    let listener = TcpListener(SocketAddress(IpAddress(AddressFamily.ipv4, "127.0.0.1"), 0));
    let bound is SocketAddress = listener.address(); // the real OS-assigned port

    let client = TcpSocket(SocketAddress(IpAddress(AddressFamily.ipv4, "127.0.0.1"), bound.port()));
    client.connect();
    let server_side = listener.socket(); // blocks until accept() returns

    server_side.write([byte(72), byte(105)]);
    client.bytes(2); // [byte(72), byte(105)]

    client.close();
    server_side.close();
    listener.close();
    return 0;
}
```

Requesting port `0` means "OS, pick a free port" — `TcpListener.address()`
queries the real bound address once at construction time rather than
echoing back the constructor's request, so a caller can always discover
which port was actually assigned. `TcpConnection(resolver, host)` resolves
`host` through any `Resolver` (the real `Dns` or the injectable
`FixedDns`) and tries each candidate address in order, moving on past a
refused one — confirmed directly with two loopback addresses, one
refused, one reachable. It deliberately does **not** accept a connection
timeout: the underlying native surface has no way to make a socket
non-blocking before `connect()`, so there's no way to bound an individual
blocking attempt from Okkama code — a real substrate gap, documented
rather than silently unenforced.

`socket/udp` mirrors `socket/tcp`'s shape for datagrams — send to an
explicit target, receive with genuine sender identity:

```okkama
import "socket/udp" take UdpSocket, Datagram;
import "net/address" take AddressFamily, IpAddress, SocketAddress;

main() is int {
    let receiver = UdpSocket(SocketAddress(IpAddress(AddressFamily.ipv4, "127.0.0.1"), 0));
    let receiver_address is SocketAddress = receiver.local();

    let sender = UdpSocket(SocketAddress(IpAddress(AddressFamily.ipv4, "127.0.0.1"), 0));
    sender.send(receiver_address, [byte(72), byte(105)]);

    let received is Datagram = receiver.datagram(64);
    received.bytes();          // [byte(72), byte(105)]
    received.source().port();  // the real sender socket's own bound port

    sender.close();
    receiver.close();
    return 0;
}
```

`connect(peer)` sets the socket's OS-level default peer, but it's a
separate, independent capability from `send()` — confirmed directly (a
raw native-level probe, bypassing this module) that calling `connect()`
and then explicit-target `send()` to that *same* already-connected peer
fails with a real, platform-specific errno. A caller uses one or the
other on a given socket, not both.

`socket/unix` turned out to be entirely about honest failure: the current
native provider has **zero** `AF_UNIX` support at all — confirmed directly
by reading `Okkama::Runtime::Native::Core::Net` and by a real probe
(`socket(3, 1, 0)` cleanly returns `unsupported`, not a crash):

```okkama
import "socket/unix" take UnixSocket, UnsupportedUnixSocket;
import "fs/path" take Path;

main() is int {
    try {
        UnixSocket(Path("/tmp/app.sock"));
    }
    catch error is UnsupportedUnixSocket {
        error.text(); // "Unix-domain sockets are not supported by this native provider"
    }
    return 0;
}
```

Both constructors attempt the real native call rather than refusing
pre-emptively, so a future native provider update adding `AF_UNIX` support
would very likely make this module work with no changes at all. Since
both constructors always throw, no instance can ever exist — every other
action (`connect()`, `bytes()`, `write()`, `socket()`, `path()`) still has
a type-correct body to satisfy its role, but each one is confirmed,
provably unreachable dead code, not an untested guess: Okkama allows a
constructor that always throws without ever assigning its declared
fields, verified directly before relying on that shape.

`crypto/tls` closes out Phase 6: TLS policy, an encrypted client socket,
and peer certificate inspection.

```okkama
import "crypto/tls" take
    CertificateAuthority, ServerIdentity, TlsPolicy, PeerVerification,
    TlsSocket, TlsFailure;
import "net/address" take AddressFamily, IpAddress, SocketAddress;
import "fs/path" take Path;

main() is int {
    let policy = TlsPolicy(
        CertificateAuthority(Path("/etc/ssl/cert.pem")),
        ServerIdentity("example.test"),
        PeerVerification.required,
    );

    let socket = TlsSocket(
        SocketAddress(IpAddress(AddressFamily.ipv4, "203.0.113.1"), 443),
        policy,
    );
    socket.handshake();
    socket.peer_certificate().subject();
    socket.close();
    return 0;
}
```

`TlsSocket` connects its own raw TCP socket directly (via `runtime/net`)
rather than wrapping an existing `socket/tcp.TcpSocket` — that object's
underlying handle is intentionally never exposed publicly, matching this
stdlib's "no raw native handle leaks out" rule everywhere else.
`CertificateAuthority.path()` is passed straight through as a
byte-encoded *file path* to the native TLS layer (confirmed by reading
the native handler directly), not inline certificate content; an empty
path falls back to the system's default CA bundle.
`peer_certificate()` throws a typed `CertificateFailure` if called before
`handshake()` completes. The current native TLS layer needs
`IO::Socket::SSL`, which this development environment doesn't have
installed — every real TLS attempt cleanly reports `TlsFailure` rather
than crashing (confirmed directly), the same pre-existing, environment-
only gap the native TLS test suite already lives with.

`sql/dialect` covers the three points where SQL engines actually differ
at the syntax level:

```okkama
import "sql/dialect" take Dialect, SqliteDialect, PostgresDialect, MysqlDialect;

main() is int {
    let sqlite is Dialect = SqliteDialect();
    let postgres is Dialect = PostgresDialect();

    sqlite.placeholder(1);              // "?"
    postgres.placeholder(1);            // "$1"
    sqlite.identifier("user name");     // "\"user name\""
    postgres.boolean(true);             // "TRUE"
    return 0;
}
```

SQLite and MySQL both use a plain `?` placeholder (position is implicit);
Postgres numbers them (`$1`, `$2`, ...). SQLite and Postgres double-quote
identifiers; MySQL backtick-quotes them — both escape an embedded quote
by doubling it. Postgres has a real boolean type (`TRUE`/`FALSE`); SQLite
and MySQL don't, and render `1`/`0`. All three throw `UnsupportedSqlFeature`
for a non-positive placeholder index — the one real, universal SQL rule
none of them relax.

`sql/statement` renders data-changing statements through any `Dialect`:

```okkama
import "sql/statement" take Statement, Insert;
import "sql/value" take SqlValues, SqlString, SqlInt;
import "sql/dialect" take Dialect, SqliteDialect, PostgresDialect;

main() is int {
    let insert is Statement = Insert(
        "users",
        ["name", "age"],
        SqlValues([SqlString("Ada"), SqlInt(36)]),
    );

    insert.sql(SqliteDialect());
    // INSERT INTO "users" ("name", "age") VALUES (?, ?)
    insert.sql(PostgresDialect());
    // INSERT INTO "users" ("name", "age") VALUES ($1, $2)
    return 0;
}
```

`Insert`/`Update` validate that `columns.size()` matches the bound
`SqlValues`' count *at construction*, not at render time — a caller who
mismatches gets `InvalidStatement` immediately. `Update`/`Delete` are
deliberately unconditional (no `WHERE` clause): no predicate/condition
abstraction exists anywhere in this stdlib yet (`sql/query`'s `Select`
only ever takes the single `EveryRow` predicate value), so row-level
filtering for data-changing statements is out of scope until one does.
`RawStatement` is the escape hatch — it returns its stored SQL verbatim
regardless of which `Dialect` is passed, confirmed directly.

`sql/query` completes the `sql/*` namespace with row-producing `SELECT`:

```okkama
import "sql/query" take Query, Select, EveryRow, RowWindow, TableName, ColumnName;
import "sql/dialect" take Dialect, SqliteDialect;

main() is int {
    let query is Query = Select(
        [ColumnName("id"), ColumnName("name")],
        TableName("users"),
        EveryRow(),
        RowWindow(10, 20),
    );

    query.sql(SqliteDialect());
    // SELECT "id", "name" FROM "users" LIMIT 10 OFFSET 20
    return 0;
}
```

`EveryRow` is `Select`'s only concrete predicate value in this whole
stdlib — there's no `WHERE`/condition abstraction defined anywhere yet,
so it's a plain concrete type, not a role, matching `sql/statement`'s
`Update`/`Delete` being unconditional for the same reason. `UnlimitedRows`
and `RowWindow` share a small internal `Rows` role (`clause() is
string`) so `Select` never has to branch on which one it was given — an
`UnlimitedRows` renders no `LIMIT`/`OFFSET` suffix at all.

`http/request` is an immutable HTTP request — method, URL, headers, and
body — plus the five most common method shortcuts:

```okkama
import "http/request" take Method, Request, Get;
import "http/headers" take Headers;
import "net/url" take Url;
import "io/buffer" take BufferAllocation;

main() is int {
    let request is Request = Get(
        Url("https://example.test/search?q=ada"),
        Headers([]),
        buffer BufferAllocation([]),
    );

    request.method();       // Method.get
    request.url().string(); // "https://example.test/search?q=ada"
    return 0;
}
```

`Get`/`Post`/`Put`/`Patch`/`Delete` each fix their own `Method` and
delegate everything else to one internally held `HttpRequest` — the
actual field-holding logic lives in exactly one place, not duplicated
five times. `InvalidRequest` is exported vocabulary with no internal
throw site in this module: neither `HttpRequest` nor any named
constructor validates anything (any method/URL/headers/body combination
is accepted) — it exists for a future `http/client`/`http/server` module
to throw once it has enough context to judge a request malformed, the
same "exported, not necessarily thrown here" shape used for
`sql/dialect.UnsupportedSqlFeature` and `crypto/digest.DigestMismatch`.

`http/response` is the other half: a streamed response plus explicit
body materialization —

```okkama
import "http/response" take Response, HttpResponse, ResponseJson;
import "http/status" take Ok;
import "http/headers" take Headers;
import "io/buffer" take BufferAllocation;
import "encoding/utf8" take Utf8Encoded;
import "encoding/json" take Json;

main() is int {
    let response is Response = HttpResponse(
        status Ok(),
        Headers([]),
        buffer BufferAllocation(bytes Utf8Encoded("{\"ok\":true}")),
    );

    let parsed is Json = json ResponseJson(response);
    return 0;
}
```

Materialization is explicit (`ResponseBytes`/`ResponseString`/
`ResponseJson`, each a `&`-projectable one-shot reader) because it may
consume a stream and allocate a large value — draining an arbitrary
`Input` can't reuse the same bounded-window-retry trick `fs/dir` used for
directory listings, since a single `bytes(limit)` call can already return
up to `limit` bytes (unlike one-entry-per-call `readdir()`) and a stream
can't be rewound the way a directory can. `ResponseBytes` instead does
exactly two reads — one bounded at 16 MiB (the same cap
`runtime/tls`'s native provider already uses for `tls_read`), then one
confirming the stream is now empty — throwing `InvalidResponseBody`
rather than silently truncating if the body turns out to be larger.
`ResponseString`/`ResponseJson` build on `ResponseBytes`/`ResponseString`
by calling their marked actions as plain, ordinary methods internally
(`&`-projection is sugar for exactly that call, never required to invoke
it).

`log/formatter` renders a `LogEvent` two ways — a human line, and a
complete machine-readable JSON record:

```okkama
import "log/formatter" take LogFormatter, LineFormatter, JsonFormatter;
import "log/event" take LogLevel, LogField, LogEvent;
import "time/instant" take Instant, ClockIdentity;

main() is int {
    let event = LogEvent(
        LogLevel.warning,
        "retrying request",
        [LogField("attempt", "3")],
        Instant(0, ClockIdentity.system),
    );

    LineFormatter().format(event);
    // [warning] retrying request attempt=3

    JsonFormatter().format(event);
    // {"level":"warning","message":"retrying request","instant":0,"fields":{"attempt":"3"}}
    return 0;
}
```

`LineFormatter` deliberately omits the instant (a line is for humans; most
destinations already timestamp on their own) while `JsonFormatter`
includes everything, since a JSON record's whole point is complete,
machine-readable data — `fields` always renders as its own nested JSON
object, empty rather than absent when there are none. Two fields sharing
a name surface as `encoding/json.DuplicateJsonMember` when `JsonFormatter`
encodes them — this module adds no duplicate check of its own, reusing
the JSON encoder's existing one instead.

The top-level `random` module builds domain-level random values without
introducing another native boundary:

```okkama
import "random" take
    RandomSource, Random, SeededRandom, Choice, Shuffled,
    InvalidRandomRange;

main() is int {
    let secure is RandomSource = Random();
    let repeatable is RandomSource = SeededRandom(42);

    let roll is int = secure.int(1, 6);       // both endpoints included
    let flag is bool = secure.bool();
    let ratio is float = secure.float();      // 0.0 <= ratio < 1.0
    let color is string = value Choice(
        ["red", "green", "blue"],
        repeatable,
    );
    let order is @int = values Shuffled([1, 2, 3, 4], repeatable);
    return 0;
}
```

`Random` implements every `RandomSource` action from secure bytes. Integer
sampling is unbiased and inclusive across the complete signed 64-bit domain;
it does not convert bounds through `float`. Equal endpoints need no entropy.
`bool()` selects equally between false and true, and `float()` produces one of
the exactly representable 53-bit fractions in `[0.0, 1.0)`.

`SeededRandom(seed)` is deliberately non-cryptographic. It is a pure query
object, not a mutable stream: the same seed, action and arguments always give
the same value, and `bytes(4)` is the prefix of `bytes(12)`. Sequential state
would require a future explicit successor contract. `Choice<T>` and
`Shuffled<T>` use `Random()` by default or retain an explicit source through
constructor overloading. Their constructors only bind state; projection does
the selection. Empty choice and reversed ranges throw `InvalidRandomRange`,
while singleton choice/shuffle do not query the source. Shuffling creates a
new array and never changes the candidates.

Network addresses are validated values before they reach DNS or sockets:

```okkama
import "net/address" take
    AddressFamily, IpAddress, SocketAddress, HostPort,
    InvalidAddress, InvalidPort;

main() is int {
    let ip = IpAddress(AddressFamily.ipv6, "0:0:0:0:0:0:0:1");
    let local = SocketAddress(ip, 8080);
    let upstream = HostPort("api.example.test", 443);

    local.string();  // "[::1]:8080"
    ip.bytes();      // exactly sixteen address octets
    upstream.host(); // no DNS query has happened
    return 0;
}
```

These constructors only retain their arguments; they do no parsing,
normalization, validation or native calls. The actions perform that work.
`IpAddress.string()` is canonical. Its `bytes()` omits the native magic,
version, family and port fields. Ports range from zero through 65535;
`SocketAddress` brackets IPv6 to keep the port boundary unambiguous.
`HostPort` deliberately retains an unresolved host, so name lookup policy
stays in `net/dns`. Invalid input is reported as `InvalidAddress` or
`InvalidPort`, never as a raw native failure. `AddressFamily.unix` is reserved
for a future Unix-domain address object and cannot construct an `IpAddress`.

URLs and DNS keep syntax, naming and transport as three separate concerns:

```okkama
import "net/address" take HostPort;
import "net/dns" take Resolver, Dns;
import "net/url" take Url, UrlReference, QueryParameter, QueryParameters;

main() is int {
    let url = Url("HTTPS://Example.COM:443/a/b?q=one&q=two");
    let target = url.resolved(UrlReference("../report#latest"));
    let resolver is Resolver = Dns();
    let endpoints = resolver.addresses(
        HostPort(url.authority().host(), url.authority().port())
    );
    let query = QueryParameters([
        QueryParameter("name", "Ada Lovelace"),
        QueryParameter("tag", "one"),
        QueryParameter("tag", "two"),
    ]);

    target.string();     // "https://example.com:443/report#latest"
    query.all("tag");    // ["one", "two"]
    query.string();      // "name=Ada%20Lovelace&tag=one&tag=two"
    endpoints[0].port(); // 443
}
```

`Url`, `UrlReference`, `Authority`, `QueryParameter` and `QueryParameters`
constructors retain source state only. Their actions perform strict parsing,
UTF-8 percent conversion and canonical rendering. `Url` represents an
absolute network URL with `scheme://authority`; `UrlReference` owns relative
input. Resolution follows RFC 3986 dot-segment rules. Query parameters remain
ordered and duplicate-preserving. A plus is a literal plus, not a form-space;
a programmatic space renders as `%20`. Missing explicit ports raise
`MissingUrlPort` rather than producing a sentinel integer.

`Resolver` keeps name service injectable. `Dns` performs transport-neutral
lookup and returns validated `SocketAddress` values carrying the requested
port; it does not decide TCP versus UDP. `FixedDns` holds immutable maps for
deterministic tests. Host resolver details are normalized to `UnresolvedHost`
and `MissingReverseName`.

`strings/parse` uses object projections, so construction and conversion are
one expression:

```okkama
module app v1.0.0;

import "strings/parse" take
    ParsedInt, ParsedFloat, ParsedBool, ParsedByte,
    InvalidInteger, InvalidFloat, InvalidBoolean, InvalidByte;

main() is int {
    let count is int = value ParsedInt("-42");
    let ratio is float = value ParsedFloat("3.0");
    let enabled is bool = value ParsedBool("true");
    let octet is byte = value ParsedByte("255");

    try {
        value ParsedInt(" 42");
    }
    catch error is InvalidInteger {
        error.source(); // the unchanged input: " 42"
    }
    return 0;
}
```

The parsers accept only ASCII scalar spellings: `-?[0-9]+` for `int`,
`-?[0-9]+\.[0-9]+` for `float`, exactly `true` or `false` for `bool`, and an
unsigned decimal value in `0..255` for `byte`. They do not trim, recognize a
leading `+`, use locale digits, or accept exponent/`NaN`/`Inf` float forms.
Each failure is a typed `Error` carrying the original string.

The I/O roles follow the Elegant Objects naming rule: a query names the value
it returns, while a command uses a verb and exposes no infrastructure result.
Lifecycle ownership is independent from transport:

```okkama
module app v1.0.0;

import "io/input" take StringInput, EndOfInput;
import "io/output" take StringOutput;

object CopyOneLine {
    has source is StringInput;
    has target is StringOutput;

    CopyOneLine(:source, :target);

    copy() is void {
        try {
            target.writeln(source.line());
            target.flush();
        }
        catch error is EndOfInput {
            target.flush();
        }
    }
}
```

`Input.bytes(limit)` requires a positive byte maximum; an empty result means
EOF. `StringInput.line()` distinguishes EOF from an empty line by throwing
`EndOfInput`. `Output.write(bytes)`, `StringOutput.write(values...)`,
`writeln(values...)`, `writef(pattern, values...)`, and `flush()` are
commands declared `is void`; invoke them separately. `writef` emits no implicit newline. They do not return
native write counts. A partial failure is `IncompleteWrite`, whose noun
`written()` reports the accepted prefix. Owned files, sockets and pipes may
also implement `Closeable`; borrowed standard streams do not. Every role has
a deterministic fake, and `Closeable.fake()` uses identity-local
`fake.storage` for an idempotent lifecycle.

Memory buffers are acquired explicitly from state-only descriptions:

```okkama
module app v1.0.0;

import "io/buffer" take StringBuffer, StringBufferAllocation;

main() is int {
    let buffer is StringBuffer = buffer StringBufferAllocation("first\nsecond");
    buffer.line();        // "first"
    buffer.writeln("!", 3);
    buffer.string();      // "second!3\n"
    return 0;
}
```

`Buffer` consumes unread bytes through `bytes(limit)` while its zero-argument
`bytes()` is a snapshot. `StringBuffer.line()` returns the final unterminated
tail once and then throws `EndOfInput`. `BufferedInput` prefetches only into an
empty private cache; `BufferedOutput` drains at its capacity and on
`flush()`/`close()`. Decorators own that cache, not the wrapped source or
target. A partial output failure retains the exact unwritten suffix for retry.

Concrete standard streams are ordinary objects:

```okkama
module app v1.0.0;

import "io/input" take EndOfInput;
import "io/stdin" take Stdin;
import "io/stdout" take Stdout;
import "io/stderr" take Stderr;

main() is int {
    Stdout().write("name: ");
    try {
        Stdout().writeln("hello, ", Stdin().line());
    }
    catch error is EndOfInput {
        Stderr().writeln(error.text());
    }
    return 0;
}
```

`Stdin.line()` strictly decodes UTF-8 and removes one LF, CRLF, or CR ending.
`StdinBytes.bytes(limit)` exposes raw octets. `StdoutBytes` and `StderrBytes`
write raw octets; formatted `Stdout` and `Stderr` concatenate display forms,
encode UTF-8, and keep the native counters private to `runtime/stdio`.

### URL and cookie policy in the active v4 stdlib

Active v4 uses `url/value`, superseding historical `net/url` examples above.
ParsedUrl(source).value() and ParsedUrlReference(source).value() defer parsing;
Url(parts)/UrlReference(parts) accept validated coordinates. base.url(reference)
resolves RFC 3986 references. Typed authority distinguishes DNS/IP/IPvFuture,
empty hosts and absent/empty ports and userinfo. Query and fragment preserve
absence separately from explicit emptiness. There is no DNS or transport I/O.
HostName uses bounded UTS46/IDNA; PublicSuffixList represents pinned ICANN+PRIVATE
policy. [Current URL contract](std/url-value.md),
[executable example](../examples/std-v4/url-resolution.ok).

Cookie values remain immutable. CookieExpiry() means session, never an epoch
sentinel. Cookie/Attributes validate grammar, prefixes and Secure/SameSite/
Partitioned policy. string() deliberately projects wire data; diagnostic() redacts
payloads. Time, origin and browser-site context are supplied explicitly:

```okkama
module app v1.0.0;
import "http/cookie_jar" take MemoryCookieJar;
import "http/cookie_policy" take CookieContext;
import "http/header" take Header, HeaderName, Headers;
import "http/method" take Method;
import "url/value" take ParsedUrl;
import "time/clock" take SystemClock;
import "io/stdout" take Stdout;
main() is int {
    let jar=MemoryCookieJar(SystemClock());defer jar.close();
    let origin=ParsedUrl("https://example.com/a/b").value();
    let context=CookieContext(origin,true,Method("GET"));
    jar.store(origin,Headers([Header(HeaderName("Set-Cookie"),"session=opaque; Secure; HttpOnly; SameSite=Lax; Path=/")]),context);
    let cookies=jar.cookies(origin,context);confirm cookies.size()==1;
    Stdout().writeln(cookies.item(0).diagnostic());
    0;
}
```

CookieStoring exposes only cookies/store/clear to its receiver. The jar retains
state in Atomic, returns independent snapshots and applies PSL/domain/path,
expiry and explicit context rules. A role does not grant access to additional
concrete-object actions. This is an infrastructure effect owner, not a dependency
inside a domain Customer. SavedCookieJar(jar,file).result() and
LoadedCookieJar(file,clock).result() perform explicit filesystem effects: a
versioned private snapshot, exact deadlines/partition keys and session exclusion.
The file is a distinct protocol boundary, without arbitrary private-field reflection.

[Cookie contract](std/http-cookie.md),
[persistence example](../examples/std-v4/http-cookie-persistence.ok).
Full HTTP transport/client/server remains subsequent roadmap work; these examples
verify policy/storage and do not perform a network request.

HTTP/1 heads can now be read asynchronously with
`Http1ResponseHeadReading(stream).result()`. The result retains typed head(),
exact binary remainder() and consumed() count. The session owns the borrowed
stream; cancellation drains child Future cleanup. See the
[contract](std/http1-framing.md) and [example](../examples/std-v4/http1-reading.ok).


`Http1BodyReading(source, owner, plan, prefix, headers)` consumes the typed framing
plan and adopts an explicit Closing owner. Fixed/chunked/close-delimited payload
is bounded; trailers require clean EOF. `Http1IncomingBody(reader)` supplies the
one-shot BodyOpening role. The owner closes on EOF or abort; pool reuse requires
a separate lease. See [contract](std/http1-body-reading.md) and
[stream example](../examples/std-v4/http1-body.ok).


`Http1RequestWriting(output, request).result()` and
`Http1ResponseWriting(output, response, requestMethod).result()` emit complete
messages. Known body contradictions fail before any head byte. Constructors
leave body roles unobserved; started futures own opened-reader/sink cleanup.
These operate on supplied streams; network connection and pooling are separate.
[Contract](std/http1-message-writing.md),
[example](../examples/std-v4/http1-message-writing.ok).


`TcpListening(IpEndpoint(..., 0)).acceptance()` acquires a listener with an actual
OS-selected port. `TcpConnecting(listener.local_endpoint()).connection()` acquires
a numeric connection without DNS. The Closing owner exposes async input/output;
closing output drains started writes and shuts down only that direction. Reads
may be short: accumulate the logical payload rather than expecting packet-sized
reads. [Ownership contract](std/tcp-stream.md),
[loopback example](../examples/std-v4/tcp-stream.ok).

`Resolving` separates name lookup from numeric connection. `FixedResolution`
snapshots typed HostAddresses/AddressName records; `SystemResolution` uses async
host-resolver workers. Both preserve first-occurrence address order and attach
the requested port. Cancelling a system lookup stops waiting and publishing;
a libc call already running in its worker may finish later.
[Configured resolution](std/fixed-resolution.md),
[system resolution](std/system-resolution.md).

`Http1Session(input, output, owner).exchange(request)` owns a supplied connection.
Its one-shot response body holds admission until verified EOF; early close or
cancellation discards the connection. An old completed reader cannot close a
later exchange. Empty responses release admission before publication.
[Session contract](std/http1-session.md),
[two TCP exchanges](../examples/std-v4/http1-session.ok).

With `Expect: 100-continue`, the session writes headers before opening the body.
A 100 reply permits upload; an early final reply leaves the source unopened and
discards reuse. One absolute waiting deadline spans intermediate replies;
timeout/cancellation drains reading. [Contract](std/http1-continue.md),
[TCP handshake](../examples/std-v4/http1-continue.ok).

Certificate metadata keeps exact wall-clock validity and distinguishes absent CA
path depth from present zero. Distinguished names keep ordered RDN groups and
repeated identifiers; a flat display/hash cannot replace those coordinates.
These values do not establish trust. [Values](std/x509-values.md),
[grouped names](std/x509-name.md).

`ParsedCertificate(bytes).value()` now reads a bounded DER envelope and asks the
host provider to decode the certificate/public key. Certificate retains exact
DER and exposes grouped subject/issuer names, wall-clock validity and PublicKey
as complete SPKI bytes. Repeated parsing preserves structural identity; host
pointers are freed before publication. Unsupported name encodings fail explicitly
when observed. Parsing establishes no signature, hostname or chain trust.
[Contract](std/x509-certificate.md), [example](../examples/std-v4/x509-certificate.ok).

CertificateNames projects SAN DNS strings and nominal IP addresses separately,
without CN fallback or matching. CertificateChain snapshots supplied untrusted
certificates; TrustedCertificates/PemTrustStore select explicit roots. Empty roots
mean no anchors and never select defaults. PEM cursor successors share a source
snapshot. Verification and system-root acquisition remain separate operations.
[Contract](std/certificate-observers.md), [example](../examples/std-v4/certificate-observers.ok).

CertificateExtensions decodes CA/depth, the nine named key-usage bits and EKU
purpose OIDs. Unknown extensions retain their identifier, critical flag and exact
payload; unknown purposes remain OID objects. Metadata observation grants no
verified permission. [Contract](std/certificate-extensions.md),
[example](../examples/std-v4/certificate-extensions.ok).

SystemTrustStore() creates a request; result() explicitly reads linked OpenSSL CA
sources and returns an immutable TrustStore. Keep that snapshot for the lifetime
of a client policy. Each acquisition reads actual sources again and reports typed
failures; it does not verify a peer. [Contract](std/system-trust-store.md),
[example](../examples/std-v4/system-trust-store.ok).

VerifiedCertificateChain snapshots explicit roots and takes a nominal certificate
identity, Instant and purpose. result() returns the verified leaf; path() returns the
selected leaf-to-anchor sequence. DNS uses DnsCertificateIdentity(HostName), IP
uses IpCertificateIdentity(IpAddress). Time is exact and inclusive; SAN matching,
signatures and path constraints are verified together. Trust-anchor payload policy
stays separate from wire-certificate syntax. [Contract](std/certificate-verification.md),
[example](../examples/std-v4/certificate-verification.ok).

TlsClientPolicy keeps explicit root, TLS-version and ALPN preference snapshots.
Defaults enable TLS 1.2/1.3 and advertise nothing. Protocol names preserve opaque
8-bit values through Latin-1 strings; the policy never acquires roots or performs
a handshake. [Contract](std/tls-client-policy.md),
[example](../examples/std-v4/tls-client-policy.ok).

TlsClientHandshake consumes supplied managed TCP when its Future starts and
publishes a TlsConnection only after verifying the actual peer against explicit
roots, nominal DNS/IP reference and current clock instant. The connection owns
AsyncReading/AsyncWriting halves and immutable TLS metadata. Read cancellation
preserves the owner; cancelled partial output aborts it. Empty input requires
close_notify. close() aborts immediately; async shutdown() performs orderly TLS
closure and disposes the descriptor. [Contract](std/tls-client-transport.md),
[actual HTTPS example](../examples/std-v4/https-session.ok).

Http1TlsSession adopts verified TLS only for exact http/1.1 or absent ALPN.
h2 and other selections fail before HTTP bytes and retain caller ownership.
Successful exchange/body leases use the existing session contract.
[Contract](std/http1-tls-session.md).

HttpEndpointSelection validates an HTTP/HTTPS URL, resolves a name once and
checks every exact numeric endpoint through EndpointAllowing. Numeric hosts skip
DNS; selected ports cannot be replaced by the resolver. Explicit CIDR lists are
immutable; PublicEndpoints uses pinned affirmative global IP properties. The
result acquires no socket. [Contract](std/http-endpoints.md).

Http1Connecting opens a conversation using only a checked route's numeric
endpoints. HTTPS verifies that URL's DNS/IP reference and negotiated HTTP1 ALPN;
ready unclaimed and unpublished results are disposed. Bounded connection racing is now implemented; shared pools remain separate work.
[Contract](std/http1-connecting.md).

TcpRacing removes checked-address head-of-line waiting: it interleaves families,
staggers relative starts and advances after failure. It cancels pending attempts
and closes ready losers before handing out a connection. Http1Connecting now
uses it; DNS streaming and strict resolver policy remain separate.
[Contract](std/tcp-racing.md).

## Chapter 27. The Native Runtime Boundary (Overview)

Okkama's connection to the host system is deliberately thin, explicit, and
portable. This chapter is an overview; the full contracts live in the
specification (§5.3–§5.8) and `docs/native-functions-tutorial.md`.

### 27.1 Native declarations

A runtime substrate module declares required host functions with `native` —
module-scoped, explicitly typed, bodyless, semicolon-terminated:

```okkama
module runtime/time v1.0.0;

native wall_clock() is int;
native monotonic_clock() is int;
native sleep(duration is int) is bool;
```

Each declaration has a canonical identity
`module/path@ABI_MAJOR::name/fixed_arity` (e.g.
`runtime/time@1::monotonic_clock/0`) that a host **provider** must implement.
A declaration without a matching provider is a runtime error — there is no
Okkama fallback body. A provider without a visible declaration creates no
callable name. Ordinary object actions can never be replaced by host code;
only these explicit bodyless declarations cross the boundary.

The ABI is positional, typed, and value-based: `int`, `float`, `bool`,
`string`, `char`, `byte`, enums, typed arrays/maps, `@byte` sequences, and
`any` cross; ordinary objects and interpreter internals never do. Results are
validated and materialized into fresh language storage.

### 27.2 `NativeFailure` — normalized errors

Expected host failures cross as a normalized value: a provider returns
`NativeFailure(domain, code, message)`; the runtime attaches the canonical
`operation` and raises it through the ordinary typed catchable channel:

```okkama
try {
    open(path, 1, 0);
}
catch failure is NativeFailure {
    failure.domain();      // e.g. "core.fs"
    failure.code();        // e.g. "not_found", "already_closed"
    failure.operation();   // canonical symbol
    failure.message();     // diagnostic text — never branch on this
}
```

Branch on stable `domain`/`code` only. Raw errno, host exceptions and file
descriptors are not public language values; unexpected host errors stay
uncatchable. Filesystem diagnostics retain the failed operation, exact path
operands and available native code/reason internally, including through cause
chains and cleanup failures. The canonical symbol remains `native_symbol` in
diagnostics and the result of public `operation()`. Host codes and localized OS
messages are diagnostic only. See [filesystem errors](std/filesystem-errors.md).

### 27.3 Opaque handles

`native object FileHandle;` declares an opaque nominal capability type.
Source code can use it only as a declared type — it cannot construct,
inspect, compare, reflect, or erase it through `any`. Provider-owned tables
validate type, authority, token, generation, and open/closed state on every
use. Close is idempotent; use-after-close is catchable `already_closed`; and
runtime shutdown finalizes every remaining handle exactly once, even after an
exceptional exit.

### 27.4 The bundled substrate modules

| Module | Provides |
|---|---|
| `runtime/time` | `wall_clock()` (Unix-epoch ns), `monotonic_clock()` (ordering-only ns), `sleep(ns)` (interruption-retrying) |
| `runtime/fs` | `@byte` paths with fallible `path_encode`/`path_decode`; binary `open/read/write/seek/flush/truncate/sync/close` and advisory shared/exclusive locks over `FileHandle`; stable metadata/mutation/directory operations |
| `runtime/process` | opaque pipes; shell-free `spawn`; isolated `spawn_configured` with exact child cwd/replacement environment; portable wait/kill; argv/pids; deterministic child reaping |
| `runtime/net` | opaque blocking IPv4/IPv6 sockets, versioned `@byte` endpoint/datagram envelopes, stream/datagram I/O, DNS `resolve`/`reverse_name`, `ip_parse`/`ip_format`, closed boolean option set |
| `runtime/poll` | typed readiness polling over file/pipe/socket handles: registration ids and tags, readable/writable masks, absolute monotonic deadlines, consumed cancellation |
| `runtime/environment` | raw-`@byte` environment get/set/remove/all (mutation is process-global) |
| `runtime/system` | portable platform/architecture strings; `secure_random(size)` from the OS CSPRNG with no PRNG fallback |
| `runtime/debug` | `data_dump(any) is string` backing `okkama.Dump`; `data_kind`, `data_type`, `data_size`, `data_leaf`, `data_names`, `data_keys`, `data_items` backing `debug/dumper.Dumper` |
| `runtime/regex` | opaque `RegexHandle` patterns: compile, exact invalid offset, boolean search/whole match, first/all leftmost-longest code-point spans, idempotent close |
| `runtime/format` | opaque-`any` concatenation plus positional `{}` and named `{field}` formatting with escaped braces |
| `runtime/crypto` | closed SHA-256/384/512 digest/HMAC set and constant-work byte comparison |
| `runtime/tls` | owning TLS sessions over consumed stream sockets; strict client verification, server PEM credentials, encrypted bytes, fixed subject/issuer/validity/DER projection |
| `runtime/stdio` | shared-buffer stdin bytes/lines and distinct stdout/stderr byte channels |
| `runtime/memory` | owned generic byte state: snapshot/size/consume/append/reset/lifecycle; no I/O buffering policy |
| `runtime/utf8` | strict UTF-8 encode/decode plus the exact valid-byte-prefix length used by the typed stdlib adapter |

Usage looks like any other import:

```okkama
import "runtime/time" take monotonic_clock, sleep;

main() is int {
    let started = monotonic_clock();
    sleep(1000000);
    monotonic_clock() - started;
}
```

Common design threads across all of them: raw bytes at the boundary (no
implicit text decoding), single-attempt I/O with visible partial progress
(empty read is EOF), idempotent close, no host constants or structures
leaking through, and runtime-owned resource lifecycles.

## Chapter 28. Debugging and Diagnostics

### 28.1 `Dump`

The bundled `okkama` module provides a compact deterministic debug formatter:

```okkama
import "okkama" take Dump;

let values is @int = [1..10];
Dump(values).string();
// [1, 2, 3, 4, 5, 6, 7, 8, 9]
```

`Dump(value).string()` supports numbers, `byte`, `bool`, `char`, `string`,
enums, typed arrays, and typed maps — recursively, including typed values
boxed in `any`. Strings and chars come out in escaped source-like form; maps
in deterministic ordered-pair form. It is for diagnostics, **not**
serialization: it never reflects object fields, never exposes native handles,
and does not add a universal `.dump()` action to values.

### 28.2 Error formats

Diagnostics come from two phases. The static analyzer rejects everything
decidable from source alone — before any code runs, including code that would
never execute:

```text
Semantic error at line N: message
```

```text
Variable "leaked" uses undeclared type "T"
Successor assignment is not allowed inside destroy
Object "Cash" does role "Priced" but does not implement action "price"
Duplicate test "Box :: same"
```

Genuinely dynamic failures — those that depend on values — remain runtime
errors:

```text
Runtime error at line N: message
```

```text
Runtime error at line 4: Undefined variable "missing"
Runtime error at line 7: Object "Box" has no action "missing"
Runtime error at line 10: Duplicate construction field "digit"
Object "Cash" field "cents" was not initialized
Action "secret" on object "Cash" is private
Successor action result must be used
Cannot rebind variable "n": type "int" has no successor actions
Cannot rebind variable "counter" of type Counter with int
```

In both phases the location prefix appears only when a source token is
available. Source snippets, caret rendering, and file paths are future work.

## Chapter 29. What Okkama Deliberately Leaves Out

Explicitly not part of Okkama 1.0 — these are decisions, not gaps:

- **null / none / nil** — no null in any spelling (Chapter 7).
- **Optional types and `?.`** — absence is modeled, not typed away.
- **Inheritance, `isa`, `super`, abstract/protected** — composition and roles
  instead (Chapter 30).
- **Mutable fields after construction** — successors instead (Chapter 19).
- **Truthiness** — conditions are `bool` (Chapter 7).
- **Implicit numeric conversions** — categories are exact (Chapters 4–5).
- **Implicit string keys in maps** — keys are expressions (Chapter 8).
- **A parallel global function model** — named behavior lives on objects.
  Okkama does have narrowly scoped anonymous action values (Section 18.5),
  modeled as immutable callable objects; it does not add `fn`/`function`
  declarations, mutable closure cells, or automatic role conversion.
- **Wildcard imports** — dependencies are named (Chapter 26).
- **Getters/setters and public fields** — behavior, not state (Chapter 16).

Alias imports and named construction are implemented.
Decimal literal semantics remain deferred. The test runner and `async`/`await`
are implemented; Appendix B records their current boundaries.

## Chapter 30. Design Patterns: Composition over Inheritance

Without inheritance, Okkama programs grow by **wrapping**. The idioms:

### 30.1 Decorator

Add behavior around an existing contract by holding it and re-exposing it:

```okkama
object StoredInvoice does PayableInvoice {
    has origin is PayableInvoice;
    has storage is InvoiceStorage;

    StoredInvoice(origin is PayableInvoice, storage is InvoiceStorage) {
        origin = origin;
        storage = storage;
    }

    payout() is ^StoredInvoice {
        let paid = origin.payout();
        storage.save(paid);
        ^origin = paid;
    }
}
```

`StoredInvoice` *is* a `PayableInvoice` (nominally, via `does`) and *has* a
`PayableInvoice`. Note how the successor pattern composes: the wrapped
object's successor becomes the wrapper's staged field update.

### 30.2 Composition with successor propagation

An outer object exposes a state change by delegating and staging:

```okkama
object Water {
    has bottle is Package = Bottle();

    Water() {}

    open() is ^Water {
        ^bottle = bottle.open();
    }

    close() is ^Water {
        ^bottle = bottle.close();
    }
}
```

### 30.3 Typed errors as objects

Throw dedicated nominal Error objects that carry context, and catch by their
concrete type or by `Error`:

```okkama
object InvalidCents does Error {
    has cents is int;
    InvalidCents(:cents);
    cents_value() is int { cents; }
    text() is string { "invalid cents"; }
}

object Cash {
    has digit is int;
    has cents is int;
    Cash(:digit, :cents) {
        confirm cents >= 0 && cents < 100 else throw InvalidCents(cents: cents);
    }
}
```

### 30.4 Test with fakes, not mocks

Depend on roles, and let the role's own `fake` stand in during tests
(Chapter 23). Production code never knows the difference, because it only
ever sees the role.

### 30.5 Small objects, fail fast

- Constructors bind and validate; actions behave; successors
  transition; `destroy` cleans up. One job per construct.
- Prefer `confirm ... else throw TypedError(...)` at the earliest point an
  expected invalid input can be checked; use panic only for broken invariants.
- Let scope own resources: bind them with `let` in the narrowest block, and
  put cleanup in `destroy`.

## Chapter 31. The Road Ahead: Designed and Planned Features

The development plan (`docs/TODO.md`) tracks the language milestone by
milestone. This chapter describes every feature that is designed or planned
but not yet (fully) implemented, so that a reader of this book knows both
what the language *is* and what it is *becoming*. None of the code shown in
this chapter should be relied on in current programs unless explicitly noted.

### 31.1 Exact collection type inference (implemented)

Untyped `let` retains exact scalar, nominal and homogeneous collection types.
Exact identity belongs to the computed value, so direct arguments and named
bindings give the same generic and native evidence. Runtime dispatch still uses
the `array`/`map` category; this does not erase exact collection tags.

```okkama
let ints = [1, 2, 3];                        // exact @int
let rows = [[1, 2], [3, 4]];                 // exact @@int
let names = ["first": "Ada", "last": "L"];   // exact %string:string
```

The fixed rules follow the language's existing philosophy:

- **Exactness, not widening.** Inference only recognizes one identical exact
  element type. `[1, 2.0]` stays untagged — no `@number`, no union, no
  automatic `@any`.
- **Heterogeneous collections stay legal and untagged.** `[1, "two", 3.0]`
  keeps concrete elements; `@any` remains an explicit boundary.
- **Empty collections stay untagged without context.** `[]` and `[:]` still
  take their tag from the first explicit boundary; emptiness proves nothing.
- Inference is recursive through nesting, and the exact identity becomes
  available in direct expressions too (`Dump([1, 2, 3])`, generic inference
  from literals, overload resolution).

Map keys preserve exact scalar or enum identity, including `byte`, `char` and
`bool`. A same-spelled key of another type is distinct. Native/worker transport
retains these identities; objects, handles and Any boxes are not map keys.
An action's own declared `@U` return also supplies its `.map()` result target,
including empty, mixed concrete role implementations, branches and await paths.

### 31.2 Explicit `any` narrowing (future)

`any` values are fully opaque today (Chapter 9). Explicit scalar casts are
implemented as the builtin actions documented in Chapter 5, but they cannot
inspect an `any` box. The specification still reserves explicit narrowing as
the only future path from `any` to a concrete type. Inference never narrows or
casts automatically; no `any`-narrowing syntax is fixed yet.

### 31.3 Portable regular expressions

Regexes are implemented as a two-layer feature with a strict portability rule:
the dialect is a reviewed cross-runtime subset, *never* "whatever the host
engine accepts". The design (TODO.md, Milestone 13A) fixes the architecture:

- a declaration-only `runtime/regex` module behind a native provider, with a
  compiled pattern as an opaque nominal handle — no `qr//`, no host engine
  objects, no capture globals ever visible to Okkama code;
- a high-level `regex` stdlib module written in Okkama over those primitives,
  with construction from an ordinary string (literal syntax remains
  deferred):

```okkama
import "regex" take Regex, Match, InvalidRegex, MissingMatch;

let pattern = Regex("([a-z]+)");
pattern.matches("okkama");       // true

let first is Match = pattern.first("123okkama!");
first.string();                  // "okkama"
first.start();                   // 3
first.groups();                  // ["okkama"]

pattern.all("one 2 two")
    .map() -> found { found.string(); }; // ["one", "two"]
pattern.replaced("one two", "_"); // "_ _"
```

- explicit immutable results — no magic "last match" state, no null for a
  failed match, typed `MissingMatch`, and a documented empty-string
  representation for unmatched optional groups;
- safety commitments: resistance to catastrophic backtracking, a closed
  flags model, sanitized compile errors through `NativeFailure`, code-point
  offsets consistent with `string.length()`/`.slice()`.

`Regex(pattern)` only retains the pattern string. Each action creates one
private owning prepared value, compiles through `runtime/regex`, performs the
operation and deterministically closes its handle. Thus constructors remain
state-only, while returned `Match` objects are detached string/span values.

The runtime vertical includes boolean `contains`/`matches`, `spans`,
`all_spans`, and `invalid_offset`. Spans contain the leftmost-longest full
range followed by captures in source order. Offsets count code points,
unmatched groups are `[-1,-1]`, and equal-end ambiguity follows source/NFA
priority. All-match scanning is non-overlapping; a zero-length match advances
one code point and the terminal boundary appears once. The default provider
uses capture-tagged Thompson instructions rather than Perl regex, retaining
bounded state-set execution.

`first` throws `MissingMatch`; malformed patterns become `InvalidRegex` at the
first action with an exact source offset. `replaced` is global and literal:
`$1`, backslashes and braces are not expanded. There is no regex literal or
flags argument. Split and capture-expanding/callback replacement remain
deferred. The normative contract is
`docs/todo/portable-regex-15072026.md`.

### 31.4 Private fields and owner operations

Only lexical owner fields and `self.field` read private state. Other objects,
role implementations and cap computations have no access grants. Move an
operation into the owner; adapters delegate and retain that owner for resource
lifetime. Return a deliberate typed value/resource rather than a private native
handle.

The former `allow` declaration is removed and rejected without a compatibility
mode. See [stdlib and application migration](allow-removal.md).

### 31.5 `async` / `await`

An async action declares its eventual result `T`, while a call returns a lazy
`Future<T>`:

```okkama
async loaded(service is Service) is string {
    let prefix = "loaded: ";
    let value = await service.load();
    prefix + value;
}
```

Calling `loaded(service)` does not start the body. First await starts it;
later awaits reuse the cached value or typed failure. There is no implicit
await, and await is valid only inside an async action. Local variables and
ordinary lexical scopes remain available across suspension.

Async does not imply a thread or make CPU-bound work nonblocking. One
`IO::Async::Loop` is used by an async program; synchronous main follows the
unchanged synchronous path. Use `spawn` for the restricted process-parallel
contract. Future cancellation and the std `async/*` modules provide structured
cleanup and composition. Spawning an async action instead starts cooperative
work in the current runtime; it does not move that work to a process.

### 31.5.1 Process-parallel `spawn`

`spawn` eagerly starts a synchronous object action in an isolated worker
process and returns `Future<T>`:

```okkama
async main() is int {
    let first is Future<int> =
        spawn Worker(seed: 7).work(input: 13);
    let second is Future<int> =
        spawn Worker(seed: 7).work(input: 15);

    let left = await first;
    let right = await second;
    left + right;
}
```

Both workers start before the first await, so CPU-bound or blocking work can
overlap. Await still caches success or failure; awaiting `first` twice does not
run `work` twice.

Process dispatch accepts `spawn Worker(...).action(...)` and a synchronous
object action selected through an exact action-type boundary, for example
`let work is (int) is int = Worker(seed: 7).work; spawn work(13);`.
The selected action's concrete result type is retained in the returned Future,
including when dispatch occurs inside a generic caller.
It does not accept arbitrary anonymous closures. Constructor and action
arguments are evaluated once in the parent. Transferable values are supported
primitive values plus recursively transferable
arrays and maps. Objects, Futures, closures and native handles cannot cross the
boundary. Workers have no shared mutable state.

Cancelling a process worker requests termination. Its Future remains pending
until the child has exited, has been reaped and its streams have closed; only
then does await observe Cancelled. Repeated cancel requests are idempotent, and
cancelling an already settled result or failure leaves it unchanged. Structured
groups and root shutdown await that same completion. The current Perl runtime
sends TERM, then KILL after 250 ms if needed. This waits for the worker itself,
not arbitrary descendants; forced process termination cannot guarantee execution
of the worker's Okkama defers.

The CLI uses a separate process for a single source file and passes an explicit
working directory to it. Source filenames are made absolute in the parent before
the worker changes directory, so relative CLI source paths work too.
Embedded and multi-file programs currently retain the
local executor. The protocol is versioned JSON, but that is an implementation
detail: a future Rust VM can use its native child-process runtime while keeping
the same source syntax and value contract. This is process isolation, not
exposed POSIX `fork` semantics.

### 31.6 Decimal (deferred) and exact money

A `DECIMAL` token type exists in the grammar for future exact-decimal
arithmetic (money is a core Okkama use case), but decimal runtime semantics
are unspecified; today decimal-looking source behaves as int/float/string.
The decimal design pass is explicitly postponed.

The bundled `money` module already covers booked monetary values without
waiting for a decimal primitive. `Money` stores a signed 64-bit count of minor
units together with a `Currency`; `rat` is used for exact rates and
intermediate division.

Money operations follow Okkama's object-composition model. `MoneySum`,
`MoneyDifference`, and `MoneyNegated` are `Monetary` values themselves, so
calculations can be nested without adding arithmetic methods to `Money`.
`MoneyScale` and `MoneyDivide` are exact `MonetaryCalculation` objects. A
rational calculation becomes posted minor units only through an explicit
stateful `Rounding` object and `RoundedMoney`:

```okkama
import "money" take
    Currency, Money, Monetary,
    MoneySum, MoneyScale, RoundedMoney, HalfEven,
    MoneyAllocation;

let usd = Currency("USD", 100);
let subtotal is Monetary = MoneySum(Money(1299, usd), Money(700, usd));
let discount = MoneyScale(subtotal, rat(85, 100));
let discounted is Monetary = RoundedMoney(
    HalfEven(discount.value()),
    discount.currency(),
);
let shares is @Money = MoneyAllocation(discounted, 3).shares();
```

Sum and difference reject mixed currencies and signed 64-bit overflow.
`MoneyAllocation` distributes indivisible minor units deterministically while
preserving the exact total. No `float` enters the API implicitly or
explicitly: callers that start from external decimal text must first parse it
into an exact integer or `rat` representation.

### 31.7 Stdlib MVP (Milestone 13 — intentionally blocked)

The standard library stays narrow on purpose: the milestone is blocked on
runtime foundations so that stdlib APIs are not grown around temporary
interpreter shortcuts. The defined minimum for examples is:

- intrinsic test assertions are implemented as contextual `test.is`,
  `test.isnt`, `test.ok`, and `test.fail`; bundled `test/more` provides the
  reusable object facade;
- simple string behaviors such as `.uppercase()` / `.lower()`;
- array helpers only if 1.0 examples keep needing them.

The foundations it was blocked on — generics, variadic constructors,
builtin value behavior, functional collections, the native substrate — are
now complete, so this milestone is the natural next growth point.

### 31.8 Test runner (Milestone 10 — implemented)

`okkama -t file.ok` provides production-pruned/test-retained parsing,
root-module discovery, intrinsic assertions, per-test lexical state, failure
isolation, cleanup ordering, deterministic reporting and 0/1 exit status.
Imported tests are not executed, while imported fakes remain available. See
Chapter 23 and `docs/todo/test-framework-15072026.md`.

### 31.9 Match completeness and static semantics (Milestones 16–17)

The **static semantic pass (Milestone 17) is implemented**: `Okkama::Analyzer`
runs before the interpreter and diagnoses field-bind placement,
successor-assignment context, `next`/`last` placement, bare `throw;`
placement, variadic-unpack positions, undeclared types (declarations plus
`let`/`catch` annotations), duplicate names (including tests), role
conformance, import validity, and empty typed action bodies — all before
execution, as `Semantic error` diagnostics (28.2). Runtime errors remain for
genuinely dynamic failures: panics, bounds, value–type mismatches, overload
selection, generic inference, and full-constructor field completeness.

Still planned: **match completeness (Milestone 16)** — type-checking the
match subject against case values, deciding whether a match *expression* must
have `else`, and enforcing branch result type compatibility. A stronger
return-path analysis depends on it. Current checks reject empty typed bodies
and obvious incompatible async tail values; recognized unconditional return
paths bypass the tail heuristic. This is not a complete control-flow analysis:
a trailing `if` without `else` or an exhaustive enum `match` can still produce
a value dynamically. The runtime validates the actual action result.

### 31.10 CLI and runner modes (Milestone 19)

`bin/okkama` runs production code by default; `-d` enables debug and `-t`
runs tests. Consecutive .ok files can form one module. Production main returns
an exit code0..255; executable diagnostics and failed tests yield failure.
The test runner supports report, workers, seed, filter, timeout and capture-limit
options; see [testing](std/testing.md). Parse/AST-print modes and a usable
interactive prompt remain future work. Source: Okkama.pm and cli/01,02,04,07 tests.

### 31.11 Functional collections, next steps

The eager `map`/`filter`/`reduce` milestone is complete (Chapter 13).
Explicitly deferred follow-ups: `grep`/`fold` aliases, lazy streams,
file-line iterators, parallel execution, map-entry transformation, and any
form of hidden mutable accumulator.

### 31.12 Alias imports

Alias imports use the same qualified lookup as basename namespaces:

- `import "sql/pool" as db_pool;` exposes the module-owned surface through
  `db_pool` instead of `pool`;
- combining `as` with `take` limits that alias namespace to the selected
  directly owned declarations;
- aliases never create bare bindings.

---

# Appendices

## Appendix A. Reserved and Removed Words

Removed words (parse errors, never valid):

```text
null none nil optional ?. isa super abstract protected with setters mutable var
```

Keyword inventory in active use: `module`, `import`, `as`, `take`, `object`,
`role`, `enum`, `has`, `is`, `does`, `self`, `let`, `if`, `else`, `match`,
`case`, `for`, `loop`, `next`, `last`, `return`, `confirm`, `throw`, `panic`, `try`, `catch`, `defer`,
`test`, `fake`, `destroy`, `private`, `native`, `async`,
`await`, `spawn`, `true`, `false`, `main` (contextually).

Contextual namespace names (not ordinary identifiers) include keyword-shaped
module path segments such as `test` in `test/more`, declared action labels such
as `is`, `fake` in `Role.fake()`, assertion names in `test.ok(...)`, and
builtin action names such as `size`, `slice`, `length`, `trim`, `chomp`,
`split`, `join`, `replace`, `replace_first`, `keys`, `values`, `has`, `less`,
`more`.

## Appendix B. Implementation Status

Runtime-supported and covered by tests (`okkama-pl/t/interpreter`,
`t/suite`):

- modules, `main`, imports (plain/`take`/basename namespaces, dependency
  closure), local-first resolution;
- all literals, escapes, arithmetic, comparison, strict equality, boolean
  operators, ranges;
- arrays, maps, indexing with `else` fallback, typed collection identity;
- `let` (typed/untyped), block scope, shadowing, `if`, `match`, `for`, `loop`,
  `next`/`last`, postfix `if`;
- `confirm`, valued/bare `throw`, string-only `panic`, typed `try`/`catch`;
- optional single-value `return` from the current action;
- `defer` (LIFO on normal, return, failure and loop-control exits);
- mandatory explicit constructors: full (exact arity), field shorthand
  (named-only), variadic; field defaults; object projections;
- actions: typed parameters, named arguments, return checks, `self`
  chaining default, private sections, typed overload resolution with
  specificity dominance, variadic tails with `values...` unpack;
- anonymous action expressions, lexical capture, action-value calls, exact
  `(T, ...) is R` signatures, explicit parameter/result declarations, and
  ordinary argument passing;
- successor actions with must-use results;
- ownership by identity and automatic `destroy` on scope exit;
- strict lexical scope boundaries;
- enums; roles, conformance, role-typed dispatch; role fakes in test mode;
- exact generics on objects and roles, construction inference, generic
  fakes with declared targets, and explicit role evidence retained across
  concrete implementers;
- `byte`, `@byte`, `any` boxing and opacity;
- builtin value behavior (`size`/`slice`/`length`/`trim`/`chomp`/`split`/
  `join`/`replace`/`replace_first`/`keys`/`values`/`has`, exact numeric
  comparisons);
- functional `map`/`filter`/`reduce` on arrays, ranges, typed maps, and
  nominal standard `Iterable<T>` objects;
- native declarations, canonical providers, `NativeFailure`, opaque handles,
  and the bundled `runtime/*` verticals (time, fs, process, net, poll,
  environment, system, debug, regex) plus `okkama.Dump` and the high-level
  immutable `regex.Regex` object;
- pure-Okkama exact `encoding/json` values, recursive parsing, deterministic
  compact encoding and located source failures.

Not implemented in the runtime:

- decimal literal semantics.

Alias (`as`) imports are checked against module boundaries.
Private fields are accessible only to their owner.

The static semantic pass and builtin scalar casts are implemented. Casts do not
inspect an `any` box. Still planned: explicit `any` narrowing, decimal literal
semantics, static match completeness, parse/AST-print modes and an interactive
prompt. Production/debug/test runner modes already work. Active stdlib scope
and remaining work are tracked in the [v4 roadmap](std/stdlib-roadmap-v4.md),
not the historical MVP milestone.

## Appendix C. Quick Reference Card

```okkama
// ---- module shape ----
module app v1.0.0;
import "money" take Cash;

// ---- object ----
object Cash {
    has digit is int;                 // private field, required
    has cents is int = 0;             // private field, defaulted

    Cash(digit is int, cents is int) {  // full ctor: exact arity
        digit = digit;
        cents = cents;
        confirm self.digit >= 0;
    }
    Cash(:digit) {                    // shorthand ctor: named-only
        confirm self.digit >= 0;
    }

    total() is int { digit * 100 + cents; }
    touch() is Cash { self; }                 // explicit receiver result

    spend() is ^Cash {                // successor: immutable transition
        ^cents = 0;
    }

    destroy { digit; }                // deterministic scope cleanup

    private:
    fee() is int { 10; }              // self.fee() only
}

// ---- contracts ----
role Wallet { total() is int; }
object Purse does Wallet { ... }

enum Status = (ready, failed, done);

object Failed does Error {
    Failed();
    text() is string { "failed"; }
}

// ---- main ----
main() is int {
    let cash is Cash = Cash(digit: 7, cents: 65);
    let prices is %string:int = ["apple": 120];
    let bytes is @byte = [byte(0), byte(255)];
    let boxed is any = 42;

    let spent = cash.spend();          // must consume successor
    spent.total();

    for 1..=3 -> n { n; }

    let doubled = [1, 2, 3].map() -> v { v * 2; };

    let add_one is (int) is int = (value is int) is int { value + 1; };
    add_one(41);

    try { throw Failed(); }
    catch error is Error { error.text(); }

    prices["fig"] else 0;              // explicit fallback, no null
    return 0;
}
```

> The delivery notes below record successive implementation stages. Their test
> counts and open limitations describe those stages. Use the linked module docs
> and current bug register for current status.

## Shared infrastructure snapshots with Atomic

`async/atomic` provides an initialized `Atomic<T>` capability:
`Atomic(value: initial)`. `value()` observes, `replace(value)` publishes and
returns void, `compare_and_replace(expected, revised)` publishes conditionally
and returns bool. Each synchronous operation is atomic among tasks sharing one
interpreter. A sequence of calls, especially with await between them, is not a
transaction. No cross-process synchronization is provided.

CAS uses ordinary equality: mismatch returns false; incomparable values fail
before publication. Atomic capabilities themselves compare by cell identity,
including inside ordinary objects and collections. Value CAS permits ABA; an
application requiring monotonicity stores and validates its own revision.

`closed`/`close` implement Closing. Closing is idempotent and shared by aliases;
subsequent reads and writes throw AtomicClosed. Dropping an alias never closes
the cell. Stored resource capabilities remain borrowed and retain their
original owner/defer/destroy lifetime. Prefer immutable snapshots. See the
invoice adapter.

### Explicit HTTP client policy

HttpLimits, ClientTimeouts and ClientPolicy collect bounded reusable inputs without opening sockets. Each timeout is independent: negative rejects, zero disables. The HTTP1 protocol adapter rejects H2-only/mixed policies and incompatible ALPN before acquisition. See [contract](std/http-client-policy.md). Enforcement belongs to the consuming client.

### A direct HTTP client owns response lifetimes

HttpClient sends an absolute HTTP(S) Request through checked numeric endpoints and returns ClientResponse at final headers. HttpDirectRequest derives origin-form and Host without observing the body. Read a nonempty body to verified EOF to enable reuse; closing or dropping it discards transport. Empty responses release admission immediately. An explicit async owner retains idle sessions across caller scopes: borrowed Atomic storage alone cannot do this. Client close cancels queued requests before releasing permits. Independent deadlines cover connection acquisition, exchange through headers, and each body read; zero disables a stage. [Contract and executable HTTPS example](std/http-client.md), [example](../examples/std-v4/http-client.ok).

### Redirects are an explicit exchange capability

Wrap HttpClient with Redirecting(client,RedirectPolicy()) to enable bounded follows. 303 changes POST to GET; 307/308 require a replayable preserved body. Each Location resolves against the effective URL; cross-origin credentials are removed and every new route is checked, even on an idle connection. Intermediate bodies are closed and their metadata appears in history; the final body remains owned by the caller. [Contract](std/http-redirect.md), [real loopback example](../examples/std-v4/http-redirect.ok).

### Cookie policy is an explicit adapter

Use Redirecting(CookieExchanging(client,jar,FirstPartyCookies()),policy) for fresh cookies on each redirect hop. FirstPartyCookies deliberately chooses top-level first-party context for the current method. A caller Cookie is rejected by default; preserve/replace are explicit policies, and duplicates never merge. Capture context once, store each Set-Cookie separately, and keep generated jar headers internal to the hop. On storage failure, dispose the acquired response body. The jar remains caller-owned. [Contract](std/http-cookie-client.md), [real loopback example](../examples/std-v4/http-cookie-client.ok).

### Secret capabilities and scoped HTTP auth

SecretBytes owns opaque native memory and exposes size/close metadata. ExportedSecret explicitly copies bytes and retains its owner across scopes. Ordinary byte/text/equality observations are unavailable. Basic/Bearer restrict preemptive credentials to verified HTTPS consumers and normalized origin/path. Redirecting belongs outside the auth adapter. [Secret contract](std/secret-bytes.md), [auth profile](std/http-auth.md). Full frozen-source provider regression: 672 files / 12185 assertions PASS.

### HTTP2 starts at a bounded wire boundary

Http2Settings describes local advertised table/window/frame/stream capacities; zero window/table/streams is meaningful. Internal framing checks a nine-byte header before requesting payload. Connection faults abort immediately; stream faults consume their bounded payload so the following frame remains aligned. A memoized frame reader borrows transport and drains cancelled reads with one group at a time. [Contract](std/http2-wire-foundation.md). The negotiated H2 engine and HPACK remain open.

### HTTP2 preface and peer settings

[Client preface and peer SETTINGS](std/http2-preface.md) verified with lazy exact wire emission, sequential peer observations, ACK, cancellation cleanup and one real loopback TCP example. Focused 477/478/479: six/ten/three assertions. Std-only; no repeat full regression. Public H2 exchanges, live streams/HPACK/flow and outgoing settings ACK tracking remain open.

### HPACK bounded wire primitives

[Integer/string/Huffman primitives](std/hpack-primitives.md) verified with all RFC octet codes and provider bounds: 480/runtime53 eight/thirteen assertions, inline/example PASS. Measured slow per-bit std decoding moved to a narrow native bulk provider (PERF-22). Full regression PASS: 680 files / 12,266 assertions / 1084 seconds; table/block/header semantics remain the next slice.

### HPACK tables and compression contexts

[Bounded tables and blocks](std/hpack-blocks.md) verified: complete static/dynamic decoding, exact context across RFC request/response sequences, minimum table shrink and conservative never-indexed encoder. 481/482 four/ten assertions; combined compatibility thirty-five PASS, inline/example PASS. Std-only after native full gate; no repeat full run. Live H2 transport remains open.

### HTTP2 HEADERS and typed field metadata

[Block and field mapping](std/http2-headers.md) verified: exact continuation sequence, cancellation, preflighted fragmentation, pseudo-field validation and normalized Host/authority agreement on input/output without body observations. 483/484/485 nine/nine/three assertions; combined 480..485 six files / forty-three PASS, inline/real TCP example PASS. Std-only; no repeat full gate. Live flow/streams/ALPN remain open.

### HTTP2 flow accounting

[Signed window/DATA accounting](std/http2-flow.md) verified: padded payload charge, negative SETTINGS credit, scoped overflow, dual-window send preflight and retained connection charge on stream error. 486 eight cases; compatibility twenty-one PASS, inline/example PASS. Std-only, no full rerun. Async credit/scheduler/deadlock prevention remain open.

### HTTP2 control values

[Reset/PING/GOAWAY metadata](std/http2-controls.md) verified: exact unknown codes, no ACK echo loop, monotone admission stop and explicitly opaque diagnostics. 487 five cases; compatibility twelve PASS, inline/example PASS. Std-only, no full rerun. Live stream/connection effects and retry remain open.

### HTTP wire byte serialization

[Bounded internal bulk joins](std/http-wire-bytes.md) replace per-byte maps through the existing buffer provider; exact 16 KiB DATA/HPACK payloads verified. PERF-23 records about 2.59x fixture improvement with residual VM traversal open. Std-only; no full rerun.

### HTTP2 stream lifecycle

[Half-close/reset/ID state](std/http2-stream-life.md) verified: independent stream states, final accepted payload distinction, minimal late-frame processing and odd uint31 exhaustion. 489 five cases; compatibility ten PASS, inline/example PASS. Std-only, no full rerun. Actual multiplexing/sibling physical reset remains open.

### HTTP2 consuming connection reader

[Internal input loop](std/http2-input.md) routes complete fields/DATA/control frames, retains HPACK before stream validation, tracks initial local ACK and queues replies without blocking on output. 490 eight plus 483 nine assertions PASS; nested continuation cancellation settles. Std-only, no full rerun. Owning writer/flow/public H2 client remain open.

### HTTP2 connection writer

[Bounded output and tickets](std/http2-output.md) serialize whole header blocks and isolate producer cancellation from physical writes. 491 eight cases; 483/490/491 compatibility 25 PASS after common preflight refactor. Std-only, no full rerun. Fair DATA/flow scheduling and physical session integration remain open.

HTTP2 duplex: `examples/std-v4/http2-duplex.ok` verifies concurrent reader/writer, sibling responses in reverse order, padded DATA and PING ACK on one real socket. 492 three assertions PASS; live body/flow/public client still follow.

### HTTP2 outbound DATA

[Sender scheduler](std/http2-sending.md) separates reservation/wire debit, waits outside the writer, queues one frame per producer, handles SETTINGS/credit notifications and retires closed slots. 493 eleven cases; flow/writer/actual TCP compatibility 30 PASS after cleanup refactor. Std-only, no full rerun. Incoming body flow and public session remain open.

### HTTP2 incoming body buffer

[Body storage/reading](std/http2-body.md) bounds storage, credits padding/consumption separately, preserves final DATA and validates length/trailers. 494 eleven PASS; shared trailer refactor compatibility 76 PASS. No runtime changes/full rerun. Aggregate receiver/session ownership follow. Fresh probe startup measured separately in PERF-24.

### HTTP2 aggregate incoming flow

[Receive ledger](std/http2-receiving.md) preserves connection debit on stream failure, credits discard once, separates request registration/response attachment and retires body storage at application EOF. 494/495/496: 22 PASS, including real TCP WINDOW_UPDATE after app reads. Std-only, no full rerun; full owned session/client/ALPN remain open.


### Owned HTTP2 session

[Owned session](std/http2-session.md): `Http2Session(input,output,owner,local,peer,limits)` performs typed exchanges on one previously selected H2 transport. Supervised stream holders preserve native bodies across reverse response arrival, per-stream cancellation/reset, GOAWAY and physical close. Completed buffered bodies remain readable until application EOF/close/drop. Final compatibility: nine files / 79 assertions PASS, including actual multiplexed TCP with PING and exact consumption credit. Interpreter cancellation races have independent red/green evidence; the required frozen-source full regression passed: 699 files / 12455 assertions, 1125 seconds, unchanged 1324-file source inventory. H2 HTTPS/opening/client policy and the remaining HTTP roadmap are still open.


### Ownership after an async continuation

An owned result created by a nested if/match/try/functional block after await transfers to its lexical caller. A synchronous signal which resumes the await does not acquire ownership of that result. Direct lifetime/owning-scope compatibility: 15 assertions PASS; full regression: 700 files / 12465 assertions PASS. Investigation.


### HTTP2 verified HTTPS and shared client pool

[Opening](std/http2-connecting.md) verifies URL-derived certificate identity/SNI and actual h2 ALPN before preface. An explicit H2 policy in the normal [HttpClient](std/http2-client.md) enables shared physical owners, multiplexed leases, stream-local abandonment/body deadlines, checked-peer reuse and GOAWAY replacement. Final root H2 ten files /83 assertions PASS; unchanged H1 client/decorator compatibility eleven files /89 PASS. Interpreter ownership correction full gate: 700/12465 PASS; std-only client changes use focused gates. Mixed ALPN, replay-safe retry and remaining HTTP modules stay open. The recorded PERF-25 warnings were later fixed by the completion trampoline.


### Mixed HTTP1/H2 ALPN and shared conversation admission

[Negotiation contract](std/http-alpn.md): HttpClientProtocols validates exact enabled/advertised agreement; HttpSelectedConnecting adopts one checked verified transport according to actual ALPN. HttpClient supports a mixed H1.1/H2 policy; absent ALPN selects enabled H1, ordinary cleartext uses H1, H2-only remains explicit prior knowledge. The common physical pool serializes actual H1 body leases and multiplexes H2. Unsent queued requests are re-admitted after conversation retirement; emitted requests are not replayed. Whole admission tasks are supervised so client close cancels queues before releasing references. Final H1/H2/decorator/mixed compatibility: 28 files /240 assertions PASS. Std-only; the last interpreter full gate stays 700/12465 PASS. Replay-safe stale retry and remaining HTTP modules follow.


### Replay-safe stale HTTP1 retry

[Optional Retrying](std/http-retry.md) performs at most one replacement exchange for
an idempotent request with a replayable body after zero-response-byte failure on
a reused H1 conversation. Partial/1xx bytes, fresh errors, POST, one-shot bodies,
timeout/cancellation and broad TLS failures do not qualify. Direct session errors
remain compatible. Real TCP, mixed H1 and verified close_notify TLS are covered;
affected H1/H2/decorator compatibility: 33 files /295 assertions PASS. Std-only,
no repeated full regression. H2 unprocessed replay and other HTTP modules remain
open.


### Scoped Digest SHA-256

[Digest profile](std/http-digest.md) supports SHA-256/SHA-256-sess, qop=auth, userhash,
fresh secure cnonce, atomic bounded nonce counts, direct HTTPS scope, one bounded
stale refresh and optional header/trailer rspauth verification. Generic received
challenge parsing handles combined schemes, token68, quoted commas and bounded
empty list members. Final client/decorator/secret/Digest compatibility: 18 files
/144 assertions PASS; inline and independent actual HTTPS CLI PASS. No interpreter
or provider code changed, so no full regression repeated. SHA-512/256, NFC
credentials, auth-int and preemptive nonce caching remain open.


### Digest SHA-512/256 and UTF-8/NFC

[Modern Digest profile](std/http-digest.md) adds SHA-512-256/-sess, explicit UTF-8
credential NFC normalization, userhash/username* and exact UTF-8 realm wire
handling. Existing ASCII profiles remain covered. Correct algorithm vectors
agree with independent OpenSSL; RFC7616's truncated-SHA-512 example has a
reported erratum and is documented. Final focused 3/35 PASS; required native
full gate 727 files /12773 assertions PASS, unchanged 2079-file inventory.
Auth-int and preemptive nonce caching remain open. Proxy work is separate.

### WebSocket frame foundation

The internal reader consumes one exact frame and borrows its input. It rejects
invalid RSV/mask/opcode/control prefixes before reading extended length, bounds
payloads, and drains pending reads on cancellation. Connection ownership and
handshake follow separately. Evidence and performance finding.

### WebSocket handshake and Origin

Handshake validation binds the nonce/accept and offered subprotocol. Server
Origin approval precedes 101; compare against a trusted configured destination,
not the caller's Host. Fragmented text is decoded only at message completion.
[Protocol contracts](std/websocket.md); actual owned connection follows.

### Owned WebSocket connections

[WebSocketConnecting](std/websocket.md) opens checked direct ws/wss with explicit
resolver, endpoint access, trust and deadlines. Await every send/close. A single
writer preserves fragment ordering; close releases receive backpressure, waits
for actual wire close and peer reply, then drains tasks before socket disposal.
Server Upgrade authorizes Origin before 101. Actual TCP/TLS examples and 13/70
focused checks pass. Compression, H2 CONNECT and complete server remain open.

### Server handling capabilities

[Server contexts and handling results](std/http-server.md) preserve observed peer/
TLS facts and restricted cancellation. Routing parameters are successors. A
normal response and a WebSocket upgrade/application are distinct result variants.
Zero disables a deadline, while connection/range budgets must remain positive.
Concrete server lifetime follows these configuration contracts.

### Serving and draining HTTP1.1

[HttpServer](std/http-server.md) adopts a plaintext listener, validates request
framing and preserves pipeline order. Header/body-idle/keep-alive/application
deadlines are independent. close cancels; ServerDrain closes idle peers while
finishing active responses, then cancels at its deadline. Never drain an unsent
body after deciding to close. Trusted WebSocket origin is configured explicitly.
7files/26assertions and independent CLI pass; TLS/H2 server profiles remain open.


The internal H2 server preface now verifies exact client magic/SETTINGS and acknowledges without consuming the next frame. The direction-aware reader can await owned body acquisition before DATA; foundation tests pass 35 assertions. This foundation alone does not provide a server session.


[Http2ServerSession](std/http2-server.md) now owns independent H2 request tasks, body buffers and shared flow control. RST/EOF cancel handlers; streaming response trailers retain END_STREAM ordering. A Perl TCP peer independently verifies SETTINGS, HPACK and exact DATA. The listener/TLS/full timeout profile remains separate work.


`HttpServer` now also composes an explicit H2-only plaintext listener with `ServerDrain`; 14 listener/H1 compatibility checks pass. TLS/mixed selection and full H2 idle policy remain open.


H2 server supervision now covers shared HEADERS/CONTINUATION deadlines, physical DATA idle budgets, inactivity after the last handler, bounded protocol GOAWAY and unexpected task failures. 11 files /47 checks pass. Normal zero-disabled deadlines remain independent; failure GOAWAY cleanup has a separate one-second bound.


[Immutable Routes](std/http-routing.md) compile literal/named/final-wildcard patterns, reject same-method intersections, route HEAD through GET and produce405 Allow. Raw segments decode once; encoded slash remains a parameter coordinate. Successor contexts preserve transport/cancellation facts. Actual H1/H2 HEAD tests verify body suppression; PERF-31 records constructor cost.


`runtime/collections.string_map_snapshot`: required native-change full gate 779files /13,120checks PASS; frozen2172inputs and root runtime are byte-verified. Later std-only changes use separate focused gates.


[HTTP forms](std/http-form.md) own an HTTP-delimited body and expose one streaming part at a time. Read to EOF before advancing; early close aborts and settled joins cancellation. UTF-8 metadata uses HTTP octet headers. Text projection, bounded URL-encoded decoding and caller-selected temporary spooling are explicit; filenames never choose paths.


[SSE](std/http-sse.md) uses one-shot EventBody over a bounded channel and owned ServerEvents parsing. Keepalive comments are explicit; incomplete EOF blocks are discarded. UTF-8 replacement, CRLF/CR/LF and persistent id/retry are tested on real H1. Metadata/size limits are explicit; reconnect remains client policy.


Routing validation also partitions larger families on later common literal
positions, retaining exact pair checks when no safe separator exists. An explicit
worklist handles maximum depth; separate name sorting preserves capture grammar
and original parts. These validation indexes do not reorder request dispatch.

[Robots policy](std/http-robots.md) compiles bounded groups and wildcard rules, preserves reserved percent escapes and chooses allow on a specificity tie. Lazy FetchedRobots owns response reading and cancellation; redirect/cache behavior belongs to the supplied exchange. Crawl-delay is explicit coordination, never authorization.

RobotsPolicy.allowed() selects indexed rule ranges for each chosen agent group; repeated selected ids do not repeat rule matching. The public selected() result keeps its original records, and wildcard budgets remain unchanged. Each agent record is scored once per selection; parsed Crawl-delay values and indexed group ranges are reused while preserving zero, maximum-delay and absence behavior. Wildcard rules use a bounded scalar star primitive with byte cursors, preserving exact retries and the original logical-step quota; literal matching retains its cached prefix/equality path.


[XML-RPC](std/http-xmlrpc.md) passes typed values through a one-event printing capability, preserving role confinement. Immutable member sequences, typed faults, strict XML1/Base64 and explicit budgets/nil compose with supplied HTTP exchange/handler. Parsed doubles preserve decimal wire values; bodies are bounded snapshots. Real H1 and cancellation checks pass.

[Rooted filesystem access](std/fs-root.md) starts from an explicitly acquired directory descriptor. Each decoded coordinate is opened relative to the retained parent without following symlinks; regular-file entries own independent descriptors. This closes the path-validation/open race for static HTTP files. A descriptor remains valid after root closure, but it does not freeze mutable file contents.

[Static HTTP files](std/http-static.md) borrow a descriptor root and hand each response its own one-shot reader. Preconditions and HEAD decide from metadata before content reads; byte ranges have exact framing. Listing and precompressed artifacts require explicit policy. The metadata ETag is weak and file contents can still change in place.

[Portable database values](std/db-values.md) print exactly one typed event through a confined role. NULL stays distinct; Statement snapshots bindings separately from SQL and leaves parsing to the backend. Ordered Row observations preserve duplicate names and fail ambiguous lookup. Captured printers are sealed before publishing; the [owned SQLite layer](std/db-sqlite.md) supplies asynchronous readers, transactions, prepared statements, BLOB, scripts and backup. [DbPool](std/db-pool.md) supplies FIFO leases and asynchronous reset/shutdown. Domain objects need no embedded infrastructure dependency.


### Owned database work

Use an explicit connector at the application boundary. Bind immutable values
rather than exposing a domain object's private fields to SQL. A receiver passed
DbConnection sees only its portable role; SQLite preparation/BLOB/script/backup
are separate capabilities available on concrete acquisitions.

```okkama
let transaction=await connection.transaction(TransactionOptions());
await transaction.execution(Statement("INSERT INTO customers VALUES(?,?)",
    [DbInteger(BigInt("7")),DbText("Ada")]));
await transaction.commit();
```

Confirmed commit/rollback is terminal. A lost or cancelled COMMIT acknowledgment
is indeterminate, so the connection closes and the body is not retried. Readers
own a bounded live cursor and return immutable Row copies; close them before
another incompatible operation. For pooled work, lease close revokes readers,
rolls back an active transaction and releases its FIFO slot after reset. Explicit
close schedules cleanup; settled joins it. The complete
[database example](../examples/std-v4/database-stack.ok) also demonstrates native
prepared reuse, incremental BLOB, script parsing and a bounded online backup.

### Application arguments, settings and terminals

The [application/CLI contract](std/application-cli.md) keeps application policy
in std objects. `ProcessArguments()` observes the runner's exact remaining argv;
`Utf8Arguments` decodes each argument separately without interpreting shell text.
`ParsedCommand` validates an explicit command tree and returns an `Invocation`.
It never prints help or exits; the application handles `CliFailure` and declared
help/version flags itself. [Executable example](../examples/std-v4/application-cli.ok).

Settings retains an index of actual segment keys for binary lookup. keys() keeps
source order and DuplicateSetting keeps the earliest original repeated key.
Child command validation sorts auxiliary names while retaining children order.
Layering finds and copies one balanced index path in a single traversal per update and publishes original
insertion order once. CLI reads this validated overlay directly for value
options. Maximum cardinality is not a latency guarantee; CommandSyntax prepares exact alias and segment-key indexes while retaining
first-error priority. Parser alias queries use binary lookup and return original
positions; options, flags and operands keep declaration order.

Settings use exact nonempty key segments. Later sources replace a complete entry,
including its source; they preserve the key's first position. Scalar fallbacks
apply only to absence. Environment mappings and JSON/TOML/INI adapters are explicit.
These objects represent configuration entities; they do not inspect domain fields.

```okkama
let key=SettingKey(["greeting","name"]);
let defaults=Settings([SettingEntry(key,["world"],
    SettingSource(SettingSourceKind.defaults,"application"))]);
let settings=LayeredSettings([defaults,invocation.settings()]);
let name=ScalarSetting(settings,key).value();
```

An async terminal acquisition returns one owned session. Its child input/output,
mode and resize capabilities are revoked on parent closure. TerminalOutput retains
Stage07 write/writeln/flush and `<-`; StyledText renders plain when policy disables
color. Interactivity of selected input and output is independent. A controlling
terminal is opened independently of redirection, and secret prompts require it.

```okkama
let session=await OpenedStandardTerminal(
    TerminalOutputKind.standard_output,ColorPolicy.automatic).session();
defer session.close();
session.output().writeln("Hello, ",name,"!");
```

Consumers can accept only `TerminalModeApplying`, `TerminalScreenApplying` or
`TerminalResizeOpening`; unrelated session actions remain outside their role.
Mode leases are nested and restore preceding modes. Cancellation, errors and
panic restore outstanding host state. Screen control remains typed and requires
interactive VT output, even with forced color. DisplayWidth and TruncatedDisplay
use a deterministic Unicode16 grapheme profile; actual terminal rendering can vary.
POSIX behavior has live PTY coverage; Windows console behavior has fake API
coverage and a separate Windows-only live test. Performance/platform limits are
recorded in the delivery audit.

### Explicit logging capabilities (stdlib v4)

Accept `Logging` in collaborators and build NamedLogging at the composition root.
Names are exact segments, fields are ordered immutable JsonValue snapshots, and
call fields override context in place. No global manager or ambient task context
is installed. A destination receives one LogRecord and propagates failures;
BestEffort and Fallback make loss/alternate delivery explicit.

```okkama
let logger=NamedLogging(LogName(["service"]),JsonStreamLogs(Stdout()),
    SystemClock(),EmptyLogFields().value());
logger.log(LogLevel.info,"started");
```

Place RedactedLogs before any broadcast, queue or persistence boundary. A JSON
line contains reserved metadata and nested application fields; terminal styling
uses explicit LogStyle entries and ColorPolicy. Files append lazily and have
explicit close/reopen; one record is never divided by rotation. Only one process
may own rotation of a path. Fallback can duplicate a partially delivered record.
See the [complete contract](std/logging.md),
[example](../examples/std-v4/logging-stack.ok) and
measured limitations.

QueuedLogs keeps a bounded persistent FIFO and starts one worker in a supplied
Supervising<int>. Capacity applies to waiting records, with one extra in flight.
Call `await queue.drain(); queue.close();` before waiting for the scope. Close
cancels only its pending delivery, not the scope or unrelated children. Overflow
is reject/drop_newest/drop_oldest with a BigInt loss count. An eager scope/Event
can run formatting or synchronous I/O in write(); the queue promises no capacity
wait, not a caller latency bound. SyslogLogs borrows a connected UDP owner and
reuses this lifecycle. Successful drain is local send completion, not delivery
acknowledgment. See [logging queue and syslog](std/logging.md).

### Locales and message catalogs

Pass locale policy explicitly. `FilteredLocales` and `LookedUpLocale` choose
only from supplied candidates; CLDR data fallback and `LikelyLocale` are separate
operations. Selection prepares immutable prefix keys: filtering preserves the
first preference, lookup preserves the first original available representative,
and repeated lookup reuses candidate normalization. Small sets use cached-key
scans; large lookup intervals retain original-position minima. Index preparation
has its own CPU/memory cost and introduces no host locale or CLDR policy.
`PluralNumber("1.0")` preserves visible precision that `Decimal`
canonicalizes. Catalogs distinguish singular/plural entries, require an `other`
form, and throw a typed error for an absent key.

```scss
import "locale/tag" take Locale;
import "i18n/plural" take PluralNumber, PluralCategory;
import "i18n/catalog" take MessageKey, PluralForm, PluralMessage, MessageCatalog;
let key = MessageKey("", "files");
let catalog = MessageCatalog(Locale("en"), [
    PluralMessage(key, [
        PluralForm(PluralCategory.one, "file"),
        PluralForm(PluralCategory.other, "files"),
    ]),
]);
catalog.plural(key, PluralNumber("1.0")); // "files"
```

[Contract and pinned-data evidence](std/locale-i18n.md).

`LocalizedList(values,locale,ListKind.conjunction,ListStyle.long).string()`
formats literal items through pinned CLDR list patterns. Choose disjunction/unit
and short/narrow explicitly; regional patterns and root fallback are deterministic.
[Contract and bounds](std/locale-list.md).


### Checks, manual time and properties

`testing/*` uses the current test context; checkers and mutable testing owners
throw `MissingTestContext` outside it. `Eventually` receives its clock explicitly.
`ManualClock.advance()` wakes waits without sleeping or reading host time.
`RecordedCalls` preserves call snapshots and `Stub.next()` supplies configured
values. Generated values carry reproduction text and reductions; `Property`
uses a private seeded PCG and shrinks the first counterexample.

```scss
import "testing/check" take TextEquals;
import "testing/generation" take IntegerGeneration;
import "testing/property" take Property;
test "integer bounds" {
    TextEquals("same", "same").check();
    let property is Property<int> = Property(
        IntegerGeneration(-10, 11), 42, 100, 10, 64,
        (value is int) is bool { value >= -10 && value <= 10; },
    );
    property.check();
}
```


`SequenceGeneration<T>(generation,minimum,maximum)` generates immutable sequences
with inclusive length bounds. Size controls length growth; elements share the
explicit random stream in order. Reductions shorten prefixes and simplify one
element at a time, retaining reproduction text. Candidate sequences are built
only when requested; use `Property<Sequencing<T>>` for sequence invariants.

[Contracts, result models and TAP/JSON reports](std/testing.md).


`JUnitTestReport(report,suite).string()` builds XML values and preserves exact
nanosecond durations, case order, seed and byte captures (Base64). Timeout/crash
are errors; failed assertions are failures. The [documented JUnit dialect](std/testing.md)
uses suite/case properties. [XML values](std/xml-values.md) also preserve mixed
content and namespace coordinates; source XML parsing remains a separate task.


Locale now supports complete bounded BCP 47 syntax and pinned IANA preferred
values: Locale("iw-IL").string() is "he-IL". LocaleExtensions exposes Unicode
attributes/keywords, transformed and private-use coordinates without discarding
identity. CLDR fallback removes the suffix; LikelyLocale preserves it.
[Complete tag contract](std/locale-tag-guide.md).


When Decimal must retain a float's binary64 identity, use
ShortestFloat(value).value() from numbers/float; the builtin float.string() does
not guarantee that roundtrip. negative() retains negative zero separately.
[Exact conversion contract](std/shortest-float.md).


Localized numeric text represents a value together with an explicit Locale and
NumberFormat. LocalizedDecimal rounds exactly before CLDR grouping and digit
substitution; LocalizedFloat uses verified ShortestFloat and retains negative zero.
Scientific, engineering, compact and percent representations share precision/sign
policies. LocalizedDecimalFromString and LocalizedIntegerFromString are strict
bounded inverses for ordinary/scientific values; compact text has no inverse.
[Contract and example](std/locale-number.md).


Currency("USD") identifies an explicit pinned ISO currency; Locale never chooses
it. LocalizedCurrency(amount,currency,locale,CurrencyFormat(...)) rounds Decimal
exactly and applies CLDR symbols, names, plural rules and accounting. Undefined
ISO minor units are typed; explicit precision can still format those identities.
[Currency contract and example](std/locale-currency.md).


LocalizedDate/LocalizedTime/LocalizedLocalDateTime select pinned Gregorian CLDR
styles for immutable wall values. LocalizedInstant additionally requires an
explicit TimeZone; Locale never supplies one. Unsupported calendars fail rather
than changing years. Wall full/long time omits zone fields; instant output uses
compatible localized names or localized GMT, preserving offset seconds.
[Date/time contract and example](std/locale-date-time.md).


Localized ordering is explicit: wrap each string in CollatedString with Locale
and CollationOptions, then use its Comparing role. Numeric ordering can place
file2 before file10 while builtin string order remains locale-independent.
Keys compare only within the same locale, policy and pinned ICU data version;
they are ephemeral representations, not persisted identities.
[Collation contract and example](std/locale-collation.md).


GNU catalogs select indexed plural forms through their own parsed formula.
GettextCatalog receives MO bytes and an explicit locale; GettextTranslation
adapts it to Translating. ParsedPo and PoString represent ordered active source
without installing global translation functions. MessageKey("",id,true) keeps
an explicit empty context distinct from absence. [Contract/example](std/gettext.md).


### Completed test execution and snapshots

Test blocks now support `test.skip("reason")`. Assertions keep their external
source position, and a mismatched error assertion retains the original typed
cause. Private actions remain available to the declaring async body after await
and in deferred cleanup. Cancelling an owned awaited call waits for its cleanup
before unwinding its caller.

```sh
OKKAMA_STDLIB_DIR=std okkama-pl/bin/okkama -t examples/std-v4/testing-values.ok --workers 2 --seed 42 --report junit
```

Explicit runner options use fresh process workers, bounded byte capture, literal
name filters and deadlines. Human, TAP, JSON and JUnit render the same nominal
TestReport, in declaration order. The seed controls execution order; it does not
replace a property's own PCG seed. The legacy -t invocation remains available.

Snapshot checking and updating are separate commands:

```okkama
import "testing/snapshot" take SnapshotFile,TextSnapshot,Snapshot,SnapshotUpdate;
// directory is an explicit DirectoryLocating capability.
let expected=SnapshotFile(directory,"invoice.txt");
let actual=TextSnapshot("invoice\r\n");
Snapshot(actual,expected,4096).check();
// Use only when intentionally accepting new expected data:
SnapshotUpdate(actual,expected,4096).update();
```

TextSnapshot chooses UTF-8/LF normalization; RawSnapshot keeps exact bytes and
JsonSnapshot prints explicit ordered JSON values. Checks never create missing
files. Updates use a no-follow root descriptor and atomic sibling replacement.
Different ManualClock owners carry distinct monotonic domains, so accidental
cross-clock subtraction or comparison raises IncompatibleClock.

Inside role fakes use fake.reading.get/has/size and fake.writing.set/remove/clear.
The writing commands do not return a storage object; remove returns whether a
key existed. Neither contextual capability can escape as a value. Existing
fake.storage is deprecated but compatible.

[Full testing contract](std/testing.md) specifies limits, cleanup grace, backend
requirements and report cause projections. [Line diffs](std/diff.md) use bounded
LCS matching and immutable std representations. LocaleDataVersion publishes the
separate ISO snapshot, ICU, UCA and collation-CLDR coordinates in addition to
ordinary CLDR/Unicode/IANA data versions.

### Keep a parsed MIME result for navigation

Retain an explicit result when a consumer needs the MIME tree:

```okkama
import "mime/part" take ParsedMime;
let parsed=ParsedMime(wire).result();
let part=parsed.part();
let children=parsed.children(); // direct multipart children; [] for a leaf
```

children() observes already parsed immutable nodes in wire order without
serializing or parsing bodies. Each new ParsedMime.result() call parses again;
the existing part() delegates to result().part(). The result retains parser
nodes()/decoded() accounting and checked limits. SMTP DATA walks this tree for
8bit/binary CTE inspection; MimeBody still exposes bytes()/validate().
See the [MIME contract](std/mime-part.md) for limits and navigation boundaries.

### Separate work guards from elapsed-time measurements

The interpreter CSV/TOML regression fixtures check exact results, recursion
warnings, interpreter call counts and depth. A separate sequential job, run as
`perl script/check-parser-performance.pl` from `okkama-pl`, retains the original
10/25-second budgets using one warmup and five fresh-VM samples and their median.
Measurements include parsing/imports/execution. Use an idle fixed machine for
comparative baselines; ordinary parallel regression retains structural guards.
See the [testing contract](std/testing.md).

### Grant structured MIME rendering explicitly

A complete MultipartBody can supply its tree explicitly:

```okkama
let body=MultipartBody("mixed","boundary",children);
let part=body.part(headers,limits);
let message=MailMessage(part);
```

MimeBody still exposes only bytes()/validate(); legacy MimePart constructors
retain representation checks. The explicit part() factory and ParsedMime result
retain the separately granted concrete capability. Known nested multipart
rendering reuses checked structure, while leaves/opaque bodies retain bounded
charset/default validation. ParsedMail and DeliveredMail preserve the part when
replacing public headers immutably.

Compact byte arrays retain fresh immutable wrappers across any boxing and strict
array equality without reconstructing every Byte. Any remains opaque; explicit
elements()/iteration may still materialize data. See the [MIME contract](std/mime-part.md).
