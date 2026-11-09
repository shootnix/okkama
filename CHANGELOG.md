# Changelog

All notable changes to Okkama. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/). A release version lives in
`okkama-rs/Cargo.toml` and in the `vX.Y.Z` tag.

## [Unreleased]

### Fixed

- Sparrow core: a map lookup with `else` on a `float` key failed JIT
  verification (`arg has type f64, expected i64`): the key went to the
  runtime helper as a float register instead of its word (#301). The other
  helper calls were checked for the same mistake; none takes a float.
  Conformance case `L05.map.float-key-fallback`.

### Added

- `rat` is a map key type (#296): `%rat:V`, rat-key literals, indexing,
  `has()`, `keys()`/`values()`, `+`/`-` and generic maps work on every
  engine. A key is the reduced value, so `rat(1, 2)`, `rat(2, 4)` and `2 / 4`
  are one key, and keys sort by value. Numeric categories stay apart:
  `%rat:int` takes no `int` key. A missing rat key reads `Map key not found:
  1/2` on Sparrow and the IR engine (it printed a debug form or failed). The
  specification (8.8), the Book (EN/RU), the Bible (ch. 8) and the LLM
  context say so. Conformance: case `L05.map.rat-keys`.

### Changed

- **Breaking:** `string.less(other)` and `string.less_or_equal(other)`
  compare lexicographically by Unicode code point, the order of
  `string.order()` and `ordering()`. They compared lengths, so code that
  mixed the two orders — a binary search with `less` over an array sorted by
  `ordering()` — was silently wrong (#281). Compare lengths with `length()`.
  Conformance: contract VAL.STRING-ORDER, case `L02.string.order-consistent`
  (prefixes, equal strings, the empty string, no case folding, code points
  past the BMP, agreement with `order()` on every pair, the binary search).
- Variants — enums with payloads, `Maybe`, `Presence` and error enums — have
  a representation of their own (#290). OIR gains the variant type and
  `variant.new`, `variant.tag`, `variant.get` and `variant.of`; each case is
  an object holding only its own payload, so a value is one allocation, and
  building or reading one no longer depends on the fields of the other
  cases. Before, a variant was one object with a one-element array per
  payload field of every case. Sparrow (bytecode version 13: a variant table,
  four instructions, `catch … variant`), the IR engine and the JVM (an
  abstract class per variant, a subclass per case) store a value as the
  object of its case; the Sparrow verifier rejects a case or a field outside
  the layout, and reading the payload of an inactive case traps. Sparrow,
  10 M steps that build and match a value: `value`/`pair` 1.46 → 0.53 s and
  30 M → 0 arrays; a 13-case variant 7.58 → 0.64 s and 260 M → 0 arrays.
  Conformance case `L05.variants.case-layouts`; the Sparrow Book documents
  the instructions.

### Fixed

- Packages: the publish job downloads only the package artifacts
  (`okkama-*`). Since 0.4.1 the release gate uploads its report as an
  artifact too, and `gh release create dist/*` failed on that directory, so
  neither the v0.4.1 release nor the nightly was published.

### Documentation

- `byte` no longer mixes with other numbers in the documents either (#294,
  decision #230): the Book (5.3, 8.1, 8.4, the builtin rules; EN/RU), the
  specification (8.3, 8.4, ranges, 8.7) and the LLM context said that
  arithmetic and ordering accept `byte` with `int`/`rat`/`float`, that the
  ordering operators are broader than equality, and that a `byte` is an array
  index and a range bound. The compiler rejects all of these; the documents
  now say so: `byte op byte` gives `int` (`/` gives `rat`), conversions are
  explicit (`b.int()`), and indexes and range bounds are `int`.

## [0.4.1] — 2026-11-09

0.4.1 completes the language proof of concept and the first hardening
roadmap. The language gains `async destroy`, module constants (`const`),
operator roles (`+ - * / %` and unary `-` on objects through the std
`operators` module), structured `await`, `Cell<T>`, value objects as map
keys, checked generic map keys, default parameters, string and array
builtins, `else if` expressions and postfix conditions on `return` and
`throw`. `more`/`more_or_equal` became `greater`/`greater_or_equal`, and
`bool.string()` prints `true`/`false`. Hardening:01 is done: budgets on every
engine, bounded verification, decoding and region walks, constant-time RSA,
linear compilation of very large functions, and a containerized release gate
over five engine configurations. `config/ini` is the first std module
rewritten in Okkama. The benchmarks now also measure steady state. Tail
calls and closures no longer pay for a `destroy` elsewhere in the program
(#285, #292).

### Added

- Benchmarks: steady state is measured (#236). `run.py --steady N` runs every
  configuration once more, and the program repeats its work N times in one
  process (an Okkama program gets N as its first argument, a reference port
  as `BENCH_REPEAT=N`; the ladder first read it through `os/environment`,
  which brought a `destroy` into every measured program and made 09-closures
  6× slower on the JVM, #292), timing each repetition with its own
  monotonic clock (`iteration <ns>` on stderr; stdout must still match the
  cold runs). The ladder's Okkama programs repeat through a `Repeat` object
  (`10-app` through a loop in its async `main`). The Python, Perl and Ruby
  ports are unchanged; `steady/repeat.{py,pl,rb}` drive them. Results carry
  the first repetition and the median of the second half for each pair.
  `export.py` writes `steady.csv` and a REPORT.md section, and the HTML
  report has a "steady state" table. On `02-calls` the JVM's warm-up shows:
  cold 0.201 s, first repetition 0.121 s, steady 0.075 s. The ladder
  programs changed (sha256 of inputs), so a release report needs a new
  baseline. Docs: `okkama-docs/benchmarks/README.md`, LLM guide §6
  ([#236](https://github.com/shootnix/okkama/issues/236)).

- Language: operator roles (#215). The std module `operators` declares
  `Adding<T, R>` (`plus`), `Subtracting<T, R>` (`minus`), `Multiplying<T, R>`
  (`times`), `Dividing<T, R>` (`divided`), `Remaindering<T, R>` (`remainder`)
  and `Negating<R>` (`negated`). An object that names one of them in `does`
  can be an operand of `+ - * / %` or of unary `-`: `a + b` calls
  `a.plus(b)`, chosen by the type of `b` among the role instances the object
  declares; `< <= > >=` call `Comparing.order`. Roles are nominal — an action
  named `plus` without `does Adding<…>` does not make `+` work (`Operator +
  cannot be applied to Money and Money: Money does not do Adding`). The checker rewrites the operator into an
  ordinary call, so no engine changes. `BigInt` does all six and `Comparing`;
  `Duration` does `Adding`, `Subtracting`, `Negating`; `Instant` and
  `MonotonicInstant` add or subtract a `Duration` and subtract each other.
  The runtime helpers of `async/timeout.Before` now pick an overload by
  argument type (`deadline.minus(now)` is the `Duration` one), with OCS
  `L08.timeout.before-and-within`.
  OCS contract `OBJ.OPERATOR-ROLES` with `L04.operators.*` and
  `L09.time.operators`, std contract `STD.OPERATORS`; Book §22.3, spec §12.2,
  LLM context, BigInt guide (EN/RU)
  ([#215](https://github.com/shootnix/okkama/issues/215)).

- Language: `async destroy { … }` (#154). The block may await; it is awaited
  where its owner's scope ends — normal end, `return`, `last`, a failure or a
  cancellation — in reverse construction order, and a cancellation that
  arrives meanwhile waits for it and reaches the task's next await. An object
  with an async destroy, or one holding it in a field, an array or a map, can
  be owned only in async code: constructing it in a synchronous body is a
  compile error (`Conn has an async destroy; own it in an async action`); an
  engine that meets one in a synchronous scope through a role value reports a
  failure. In async functions of a program with an async destroy, scope exits
  lower to `region.settle` / `region.settle_unwind` (OIR, and Sparrow
  instructions 138/139, SPB format 11; the core re-enters the instruction
  while each destroy runs, the IR engine and the JVM await it in place);
  sparrow-origin reports it unsupported. OCS contract `LIFE.ASYNC-DESTROY`
  with five `L07.destroy.async-*` cases; Book §20.1.1, spec §11.5.1, grammar,
  async guide §7.2, LLM context, Sparrow book (EN/RU)
  ([#154](https://github.com/shootnix/okkama/issues/154)).

- Language: module constants (#153). `const NAME = value;` (optionally
  `is Type`) at module scope names a value the compiler computes from
  literals, payload-free enum cases, `rat(n, d)`, casts, operators, other
  constants (any declaration order) and arrays and maps of such values, with
  the semantics of the running program (a big integer stays exact, `1 / 2` is
  a `rat`); a call, a constructor, an object or a local is an error, and so is
  a failing computation (`1 % 0`) or a cycle. Constants are public (`take
  LANG`, `settings.LANG`) unless `hidden const`; upper-case names work. A
  scalar is inlined where it is used; an array or a map is a module global
  built once per run by an init function and shared: IR `global gN`, Sparrow
  `global.get` (SPB format 10: the program lists its globals), a static field
  of the JVM main class set in `<clinit>`; sparrow-origin reports it
  unsupported (#14). OCS `L06.const.*` and contract `MOD.CONST`; Book §26.3.2,
  spec §5.1.2, grammar, LLM context, Sparrow book (also `str.fn`), VS Code
  highlighting (EN/RU) ([#153](https://github.com/shootnix/okkama/issues/153)).

- Language: generic map keys are checked (#242). A type parameter used as a
  map key — `%T:V` in a field, a signature or a body, or a literal `[item: 1]`
  keyed by a `T` — takes only key types: `Index<@int>` is a compile error
  `Generic parameter "T" for object "Index" is a map key: Map key type @int
  is not a scalar key type`, also through a parameter passed on
  (`Wrap<U> { has inner is Index<U>; }`). The check runs once all bodies are
  typed, so declaration order does not matter. The core and the JVM refuse a
  non-key value under a key parameter at run time instead of comparing it by
  identity. New std `KeyedSet<T>` (`collections/set`) and
  `KeyedFrequencies<T>` (`collections/frequencies`): map-backed, O(log n)
  membership and counting, traversal in key order. OCS contract
  `GEN.KEY-PARAMS` with `L05.generic.key-param-values` and three rejected
  forms; Book §8.2, spec §6.5, LLM context (EN/RU)
  ([#242](https://github.com/shootnix/okkama/issues/242)).

- Language: string builtins (#210) — `contains`, `starts_with`, `ends_with`,
  `find` and `find_last` (a `Maybe<int>` code point index; an empty part is
  found at the start, respectively the end), `trim_start`, `trim_end`,
  `lower` and `upper` (full Unicode case mapping without a locale: `"ß"` →
  `"SS"`, a final sigma → `ς`) and `repeat(count)` (a negative count or a
  result over 1 GiB is a runtime error). Array builtins (#211) — `contains`
  and `index_of` (equality as `==`; element types `==` rejects are rejected),
  `reversed`, `take(count)` and `drop(count)` (at most `count`; a negative
  count is an error); they are loops in lowering, so every engine runs them
  unchanged. `char` becomes usable (#231): `c.code()`, `c.string()` and
  `string(c)`, `char(code)` and `code.char()` (a Unicode scalar value, else a
  runtime error), and `text.characters()`, the `@char` of a string. The new
  string functions are one Sparrow instruction, `str.fn` (opcode 86, the
  function as an operand), on the core; sparrow-origin reports it as
  unsupported (#14). OCS `L02.string.search`, `L02.string.case-and-repeat`,
  `L02.char.code-round-trip`, `L05.array.search-and-shape` and their invalid
  forms; `L02.char.string-cast` now expects the text. Book chapters 6 and 10,
  spec §6.6/§6.6.1, LSP completion (EN/RU)
  ([#210](https://github.com/shootnix/okkama/issues/210),
  [#211](https://github.com/shootnix/okkama/issues/211),
  [#231](https://github.com/shootnix/okkama/issues/231)).

- Language: `else if` chains in an `if` expression (#257): `let size = if n
  > 5 { "large" } else if n > 2 { "medium" } else { "small" };`, as an
  argument or a value anywhere. Every link has its `else` and all branches
  one type; a chain was a syntax error outside statement position. OCS
  `L03.if.expression-else-if` now expects the value (it expected the
  rejection); Book §11.6, spec §8.11, grammar `if_expr` (EN/RU)
  ([#257](https://github.com/shootnix/okkama/issues/257)).

- Language: a postfix condition on `return` and `throw` (#258): `return x if
  found;`, `return if done;` in a void action, `throw Empty.list if
  items.size() == 0;`. The condition is evaluated first and the value only
  when it holds, as for `last value if`. Right after the keyword, `if c;` is
  the bare form, while `return if a { 1; } else { 2; };` still returns an
  `if` expression. OCS `L03.return.postfix-if`; Book §12.7, spec, grammar
  (EN/RU); `okkama fmt` keeps the form
  ([#258](https://github.com/shootnix/okkama/issues/258)).

- Diagnostics (#256): `while` and `do … while`, compound assignments (`+=`,
  `-=`, `<<=`, …), `++`/`--`, `break` and `continue` name what Okkama writes
  instead (`loop` with `last if`, a new name, `reduce` or a successor, `last`,
  `next`) instead of "Expect expression" or "Undefined variable"
  ([#256](https://github.com/shootnix/okkama/issues/256)).

- Language: default values for trailing parameters of object actions and
  constructors (#149): `greet(name is string, mark is string = "!", times is
  int = 1)`. A call may leave them out positionally from the end or by name;
  each left-out value is the declared expression, evaluated at that call in
  the declaring module (the caller's locals and `self` are not visible), and
  its errors are reported once, at the declaration. A parameter after a
  defaulted one needs a default; variadic actions, role signatures, natives
  and enum payloads cannot have any. Overload selection treats a left-out
  parameter as neutral. OCS `L03.args.default-values`; Book §11.4 (EN/RU);
  `okkama fmt` prints the defaults ([#149](https://github.com/shootnix/okkama/issues/149)).

- Language: an anonymous action's declared result (`(job is Job) is int {
  … }`) fixes its type and supplies generic arguments, so
  `SortedBy(origin: jobs, key: (job is Job) is int { job.priority(); })` needs
  no `SortedBy<Job,int>` at the binding (#214). The checker already did this;
  a test now pins it, and `collections/ordering`'s documentation no longer
  asks for the annotation ([#214](https://github.com/shootnix/okkama/issues/214)).

- Language: `any`, `all` and `find` with early exit (#150), in the block
  syntax of `map`/`filter`/`reduce`: `orders.any() -> order { order.overdue();
  }`, `items.all() -> x { … }`, `users.find() -> user { … }` (a `Maybe<T>`).
  The loop leaves at the first decisive item — later items are not visited —
  and an empty source gives `false`, `true` and `Maybe.none`; `any`/`all`
  work on maps with `key, value`. Lowering reuses the value-mode loop of `for`
  with a `last`-like exit, so every engine runs it unchanged. OCS
  `L05.functional.early-exit`; Book (EN/RU); LSP completion
  ([#150](https://github.com/shootnix/okkama/issues/150)).

- Language: `test.throws(Type) { … }` (#216) asserts that a block throws a
  `Type` and yields the caught error: `let refused = test.throws(Refused) {
  Gate().check(0); };`. It is parsed as the `try` it means — the block, a
  panic that fails the test when nothing was thrown, and `catch e is Type`
  yielding `e` — so checking and every engine treat it as one; `okkama fmt`
  keeps the written form. 64 std assertions of the form
  `test.ok(try { …; false; } catch e is T { true; })` in 28 modules now read
  `test.throws(T) { …; };`; the ones that match an enum case keep their `try`.
  Book §14 (EN/RU) ([#216](https://github.com/shootnix/okkama/issues/216)).

- std `cell`: `Cell<T>`, the mutable value of synchronous code (#152) —
  `Cell(value)`, `value()`, `replace(value)`, no `close()`. The compiler keeps
  a cell in synchronous code: an async action cannot create, receive or use
  one, and a field cannot hold one (`@Cell<T>` and other containers included),
  so every operation is whole with respect to tasks; a sync action using a
  cell may be called from async code. `Atomic` stays for state shared between
  tasks. Thirteen local accumulators in std (`diff`, `db/postgres`,
  `db/postgres/wire`) moved from `Atomic` with `defer close()` to `Cell`; the
  four that live in async actions stay `Atomic`, as the new check requires.
  Book §19.1 (EN/RU) ([#152](https://github.com/shootnix/okkama/issues/152)).

- JVM: `--max-steps` and `--max-memory` (#237), so every budget now works on
  every engine with the same reports. Steps are counted alike on the IR
  engine, the core and the JVM — a function entry and a branch back to the
  same or an earlier block — and a test checks the exact count on all three.
  The JVM counts them in generated code (`okkama.rt.Rt.step`) emitted only
  when a step limit is requested: without one the program classes are
  byte-for-byte the same as before, and with one the ladder's 02-calls and
  07-objects run within noise of the unlimited run. The
  memory budget is the Java heap (`-Xmx`, which includes the runtime's own
  objects) and the bound of every thread stack; running out of either is the
  core's `Execution budget exhausted: memory`. Running out stops the JVM at
  once: `catch` does not see it and cleanup does not run, as on the core.
  `okkama build --target jvm --budgeted` writes classes that count steps; the
  limit comes from `java -Dokkama.steps=N` (no property, no limit), and
  `-Xmx` with `-Dokkama.memory=N` gives any JAR the memory report
  ([#237](https://github.com/shootnix/okkama/issues/237)).

- Language: exponent literals are floats (#57): `1e5`, `1.5e-3`, `2E+10`,
  `1_000e1_0` — `e` or `E`, an optional sign, digits that `_` may separate,
  with or without a fraction; an exponent never makes an `int`. The value is
  the nearest binary64, and a literal must be one: `1e400` (not finite) and a
  literal that is not zero but would round to `0.0` (`1e-400`, also
  `0.000…1` with hundreds of zeros) are compile errors; subnormals such as
  `4.9e-324` are kept. The lexer error of #45 is gone. The grammar, spec §4.2,
  the Book (EN/RU), the LLM context and the VS Code highlighting follow;
  `okkama fmt` keeps the spelling. OCS: `L01.lex.float.exponent` runs the forms
  on every engine and `L01.lex.float.exponent-out-of-range` (was
  `exponent-rejected`) rejects `1e400`
  ([#57](https://github.com/shootnix/okkama/issues/57)).

- Language: value objects are map keys (#212). An object whose fields are all
  scalars (`byte` and `rat` included), enums or such objects, with no `destroy`
  and no capabilities, can key a map: `%Card:int`. Objects are immutable, so a
  key compares by its fields exactly like `==` — `Card(10, Suit.hearts)` built
  twice is one key — and `keys()` orders object keys field by field in
  declaration order, identically on the IR engine, the core and the JVM (maps
  are ordered, so the structural identity is an order, not a hash). A type
  that cannot be a key is named with its reason, also through nested objects
  and generic arguments (`Map key type Outer cannot be a key: field "inner"
  (Tagged): field "tags" is @string`); a key type in a field declaration is
  checked once every object's fields are known. OCS: `L05.map.object-key` now
  runs object keys (it asserted the old refusal) and
  `L05.map.resource-key-rejected` rejects a `destroy` object. Book (EN/RU),
  spec §6.3/§8.8 and the LLM context follow. A map keyed by a type parameter
  is still checked only at run time (#242)
  ([#212](https://github.com/shootnix/okkama/issues/212)).

- Language: names before one type share it in every parameter list, as they
  already do in field declarations — `full(separator, suffix is string)`,
  `Person(given, family is string, age is int)`, enum case payloads
  `declined(id, line is int)`, role signatures, natives and anonymous actions
  `(a, b is int) is int { … }`. A group cannot be variadic ("give the
  variadic parameter its own type"). `okkama fmt` keeps the grouping the
  author wrote. Two OCS cases run the forms on every engine and reject a
  variadic group ([#232](https://github.com/shootnix/okkama/issues/232)).

- LSP: every syntax error at once (H-24). The editor's check parses in a
  recovering mode (`syntax::parse_recovering`): after an error in a statement
  it resumes after that statement, after an error in a declaration at the
  next one, and an unindented line inside an unclosed block counts as the
  next declaration, so a missing `}` costs one declaration instead of the
  rest of the file. Up to 50 errors are reported, lexical and syntax errors
  together, with codes (`lexical`, `syntax`) separate from the text, and the
  outline shows the declarations that parsed. A declaration with an error is
  left out of the tree, so nothing damaged reaches analysis; the CLI keeps
  the strict parse that stops at the first error. Tests cover two
  independent errors, statement errors in one body, a missing and a stray
  `}`, the cap, and 500 damaged copies of the all-syntax file
  ([#47](https://github.com/shootnix/okkama/issues/47)).

- Benchmarks, method (H-31): the runner (`okkama-docs/benchmarks/run.py`)
  builds every program first, warms each configuration up once and then runs
  all samples in a shuffled order (the seed is recorded), so drift does not
  bias the last configurations. Results carry the raw samples with their
  order, median, extremes, quartiles, σ and the interquartile range relative
  to the median; versions (`okkama --version`, rustc, JDK, interpreters),
  build parameters (release profile, RUSTFLAGS, the pinned toolchain, JVM
  flags), sha256 of every program's sources and of the binaries, and timer
  bounds. `sparrow-core-compile` measures compile+run in one process next to
  the prebuilt `sparrow-core`, and the start-up cost (`00-hello`) of each
  configuration is reported. Reference interpreters older than the current
  stable line (here Python 3.9, Ruby 2.6) are listed and the report says so.
  The report shows each time's spread and the start-up line; steady-state
  timing is #236 ([#54](https://github.com/shootnix/okkama/issues/54)).

- Sparrow core, profiling (H-32, O01): `--stats` attributes allocations —
  each inline allocation site in machine code counts its allocations and
  bytes (`alloc site Point.moved:3 obj.new Point`), and the runtime counts
  its own by kind (objects by type, strings, arrays, byte arrays, closures,
  big integers, payloads, async frames) — and reports the time spent
  collecting and the run's wall time, so machine-code time is the run less
  compile and collection time. `tools/profile/ab.py` compares two core
  binaries on one workload: shuffled runs, identical output required,
  median and interquartile range, peak memory and the `--stats`
  differences, with a verdict (faster beyond the spread, memory within 5 %)
  and a JSON record of commit, hashes, seed and settings
  ([#55](https://github.com/shootnix/okkama/issues/55)).

- Execution budgets on the IR engine and the JVM: `okkama run --engine ir`
  takes `--max-steps`, `--max-memory`, `--max-output` and `--timeout` with
  the Sparrow core's reports and exit status — steps are function entries
  and backward branches, memory what the allocator holds beyond the start
  of the run, output the bytes of stdout and stderr; running out is not a
  failure any handler catches, and a run blocked in a native is stopped a
  second after its timeout. `--engine jvm` takes `--max-output` and
  `--timeout`: `java` runs as a child whose output passes through counters
  and which is killed with the same report; steps and memory there wait for
  #237. A test runs every budget on ir and sparrow, and output and timeout on
  the JVM, expecting identical reports
  ([#71](https://github.com/shootnix/okkama/issues/71)).

- Compiler, OIR pass `fold` (H-36): `int.add`, `int.sub`, `int.mul`,
  `int.mod` by a nonzero constant, `int.neg` and integer comparisons of
  constants become constants, computed with the language's exact `int`:
  `9223372036854775807 + 1` is the big integer every engine would promote
  to. Nothing that can fail is folded (`7 % 0` stays and fails at run
  time), nor word-level bitwise operations, and a result longer than 1024
  digits stays a computation. It runs after `tail-calls`, before `hoist`;
  the gate's run with the passes off checks it like every pass
  ([#56](https://github.com/shootnix/okkama/issues/56)).

- Language: `+` and `-` on arrays and maps. `a + b + c` concatenates arrays
  (or merges maps, the rightmost value winning a repeated key); `a - b`
  removes from an array every element equal to an element of `b` (or from a
  map every key of `b`). Both operands must have exactly the same type —
  `@int + @@int` is a compile error — and an untagged `[]`/`[:]` takes the
  other's. A chain is one operation: `a + b + c` and `a + (b + c)` lower to
  one OIR instruction (`array.concat`, `map.merge`; `array.difference`,
  `map.difference` for `(a - b) - c`), and every engine allocates the result
  once for the sum of the parts. New SPB instructions `arr.concat`,
  `arr.diff`, `map.merge`, `map.diff` (96–99) with verifier rules; the IR
  engine, the JVM backend (`Rt.concat/difference/merge/without`), the
  Sparrow core and the previous VM implement them; seven OCS cases
  ([#145](https://github.com/shootnix/okkama/issues/145)).

- Sparrow core, debugging (H-21): a wrong result can be narrowed to one
  compiled function without rebuilding. `SPARROW_CORE_UNOPTIMIZED=symbols`
  compiles the chosen functions (`*`: all) without Cranelift's passes and
  `SPARROW_CORE_PLAIN=symbols` without the core's inline fast paths (those
  instructions take the shared slow path); bisecting the list finds the
  function, and `SPARROW_CORE_DUMP=symbols` prints its CLIF before the passes
  (with the helpers and functions it calls by name) and its machine code.
  `--stats` adds the compile time and machine-code size of each function and
  their totals ([#44](https://github.com/shootnix/okkama/issues/44)).

- Docs: the trust model is back in the repository (`docs/security-model.md`,
  H-01): for source, bytecode on the Sparrow core, programs, packages and
  embedding it says what is guaranteed, what is not (each with its open
  issue) and how to run untrusted input today
  ([#37](https://github.com/shootnix/okkama/issues/37)).

- Tools: `tools/gen/okgen.py` generates well-defined programs from a seed
  (BigInt promotion, strings, arrays, objects, a role, closures, try/catch,
  bounded recursion), runs them on the IR engine, the Sparrow core and the
  JVM, minimizes a disagreement by delta debugging over the statements of
  `main` and keeps seed, program, minimized program and every engine's
  output. `tools/gen/mutants.sh` seeds one defect per engine in a scratch
  copy and checks that the generator finds and minimizes it: with 30 seeds
  the IR defect is found by 2 programs, the core's by 6 and the JVM's by 8,
  each minimized to one statement
  ([#35](https://github.com/shootnix/okkama/issues/35)).

- Docs: the Sparrow Book (`docs/SparrowBook/SparrowBook.md`, Russian
  `SparrowBook-RU.md`) — a textbook of Sparrow assembly: the file layout,
  types, registers, edges, objects and roles, arrays and maps, calls and
  closures, failures and handlers, regions, async functions, tests, what the
  verifier guarantees and how the core runs it, and a reference of all 121
  instructions generated from `okkama-sparrow`. `build.py --check`
  assembles, verifies and runs the ten examples and compares their output
  with the book.

- Sparrow core, embedding: native permissions by group — `Limits::natives`
  lists the groups a program may use (`Fs`, `Net`, `Process`, `Env`, `Clock`,
  `Random`, `Terminal`; `None`, the CLIs' default, allows all; pure natives and
  stdout/stderr are always allowed). A native outside them fails the program
  ("Native … needs the Clock permission"), and process workers need
  `Process`. A panic inside a native provider no longer aborts the host: it
  stops the run with "internal error in native …", and the runtime refuses
  further runs; a fresh one works. `run_worker` is marked as the CLIs'
  worker entry, not an embedding API. No cost without restrictions
  ([#31](https://github.com/shootnix/okkama/issues/31)).

- Compiler: an independent check of Sparrow register allocation (H-10). The
  core trusts the allocation for its stack maps, and the SPB verifier checks
  only that registers are written before they are read. The check follows
  each register's definite value through the original function — reads,
  parallel block-parameter writes on edges, unwind edges from every point of
  a protected block — in debug builds and with `OKKAMA_CHECK_REGISTERS=1`.
  It passes on every function of the repository's 1509 programs and catches a
  live value overwritten, a value an unwind handler needs overwritten, and
  block parameters written in the wrong order
  ([#30](https://github.com/shootnix/okkama/issues/30)).

- Compiler: phase contracts (`docs/phases.md`) — what the lexer, parser,
  analysis, lowering, IR passes, both code generators and SPB loading take,
  guarantee and are checked by. `okkama ir --passes LIST` and
  `--after PASS` print the IR before or between passes. Tests show that every
  public consumer of OIR (both code generators, `execute`, `run_tests`)
  rejects a corrupted module, and that failures in short-circuit operands,
  `throw`, closures and region cleanup point at their source operation with
  every pass selection ([#36](https://github.com/shootnix/okkama/issues/36)).

- Compiler: `OKKAMA_IR_PASSES=none|all|tail-calls,hoist,regions,jumps,undef`
  selects the IR passes. Every engine runs the passed OIR, so engine
  comparisons cannot see a pass that changes meaning; the release gate now
  also runs the IR conformance suite with the passes off, and a test checks
  that programs (i64 edges, BigInt promotion, division by zero, failures with
  cleanup order, deep tail recursion) print the same under every selection
  and that a deliberately wrong pass (`sabotage-int-add`) is caught
  ([#29](https://github.com/shootnix/okkama/issues/29)).

- Sparrow core: runtime helpers called from machine code are described once
  (H-06): each entry names the helper's Rust signature, which is checked at
  compile time — a helper with another parameter or result type, or another
  arity, does not compile — the Cranelift signature is derived from it, and
  its effects (allocates, collects, re-enters machine code, may fail) are
  listed in `jit::HELPERS`. `--stats` counts calls of every helper, and a
  test runs programs that reach all 28. A panic in a helper aborts with its
  message at the `extern "C"` boundary; turning it into a failure status
  would cost every helper call. No cost: ±0.2% instructions
  ([#23](https://github.com/shootnix/okkama/issues/23)).

- Sparrow core: collector stress mode, `SPARROW_CORE_GC_STRESS=1` — every
  safepoint collects and reclaimed cells are poisoned, so a value kept alive
  by mistake fails at once. Exceptional edges are tested under it: the
  audit's handler-liveness program and variants (an allocating callee, a
  failing instruction that overwrites its operand, nested calls) keep the old
  register value, and a source program with handlers, cleanup regions and
  nested failing calls prints what the IR engine prints. The core needed no
  fix; with stack-map roots disabled the stressed tests fail
  ([#22](https://github.com/shootnix/okkama/issues/22)).

- Sparrow core: execution budgets for untrusted programs and services.
  `okkama exec`, `okkama run --engine sparrow` and `sparrow run` take
  `--max-steps N` (function entries and loop iterations), `--max-memory SIZE`
  (memory the process holds beyond the start of the run: heap, BigInt,
  strings, collections; it also bounds the machine stack), `--max-output SIZE`
  and `--timeout SECONDS` (also while awaiting); embedders set
  `sparrow_core::budget::Limits`, including an interrupt flag. Running out
  exits 1 with `Execution budget exhausted: steps|memory|output` or
  `Execution interrupted`; `catch` does not see it and a later run in the same
  process starts from the base memory. Machine code polls only when a budget
  is set: unbudgeted runs are unchanged (±0.1% instructions); with a step
  budget, +7–15%. IR/JVM: [#71](https://github.com/shootnix/okkama/issues/71);
  native operations and per-runtime memory:
  [#72](https://github.com/shootnix/okkama/issues/72)
  ([#21](https://github.com/shootnix/okkama/issues/21)).

- Blade: `blade build` and `blade run`. Every setting is in `blade.toml`:
  `[build] entry/target/output` (sparrow or jvm; default
  `target/<name>.spb`) and `[run] engine/args/env` (sparrow, ir or jvm;
  `blade run -- ARGS` appends arguments). `run` builds only when sources,
  `blade.toml` or `blade.lock` are newer than the output and returns the
  program's exit status; both refuse a project whose dependencies are not
  installed. `blade new` writes the sections with their defaults
  ([#68](https://github.com/shootnix/okkama/issues/68)).

### Changed

- std: `config/ini` is written in Okkama (#13, first module). The parser and
  the writer no longer use natives; `runtime/ini` and its 17 natives are gone
  from the IR engine, the Sparrow core and the JVM (ABI IDs 242–258 retired).
  The public API and the behaviour are unchanged. A differential test against
  the native codec, run on 3 000 generated documents and 2 000 emissions,
  found no difference. 220 of the native results, 10 per failure class and
  30 valid documents, stay as OCS `L09.ini.native-equivalence`, which passes
  on all three engines. Parsing is two passes: lines classified on their own,
  then the document rules in line order, with duplicates found by sorting.
  On 10 × 1 000 entries it takes 15 ms on the core instead of 3.3 s, 344 ms
  on the IR engine instead of 4.8 s, and 38 ms on the JVM instead of 117 ms.
  The old time went into copying the parsed document on every entry read and
  into the O(n²) duplicate check of `IniSection`, which now sorts as well.
  Rendering is slower than the native writer: 37 ms instead of 4 on the core,
  203 instead of 9 on the IR engine, 36 instead of 8 on the JVM. Benchmark:
  `okkama-docs/benchmarks/ini/bench.ok`; guide: `docs/std/ini-guide.md`
  ([#13](https://github.com/shootnix/okkama/issues/13)).

- Language: overloads may differ in their results (#215). Two actions with
  the same name and arity are compatible when their parameter types differ;
  their result types no longer have to match (`plus(int) is Duration` beside
  `plus(Duration) is Instant`). Calls that the parameter types cannot tell
  apart are still rejected. A role may also appear in `does` more than once
  with different type arguments (`does Subtracting<Duration, Instant>,
  Subtracting<Instant, Duration>`); the same instance twice is an error. OCS
  `L03.overloads.results-differ-by-parameter-types` replaces
  `overloads.incompatible-returns`; Book §11 rule 8, spec §11.8 (EN/RU)
  ([#215](https://github.com/shootnix/okkama/issues/215)).

- Std: `BigInt` arithmetic is operators (#215). The expression objects
  `BigSum`, `BigDifference`, `BigProduct`, `BigQuotient`, `BigRemainder`,
  `BigGcd`, `BigLcm` and `BigPower` are removed: write `a + b`, `a - b`,
  `a * b`, `a / b` (truncates toward zero), `a % b` (sign of the dividend),
  `-a`, and the actions `a.gcd(b)`, `a.lcm(b)`, `a.power(exponent)` and
  `a.power(exponent, maximum_bits)`. The errors (`BigDivisionByZero`,
  `InvalidBigExponent`, `InvalidBigPowerLimit`, `BigPowerLimitExceeded`) are
  unchanged and are thrown by the call. Std modules and OCS cases that used
  the objects are rewritten ([#215](https://github.com/shootnix/okkama/issues/215)).

- Language: the builtin comparisons `more` and `more_or_equal` are renamed
  `greater` and `greater_or_equal` (#284), the pair to `less` that `Order`
  already uses (`less, equal, greater`): `3.greater(1)`,
  `2.greater_or_equal(2)`. The old names are gone, with no alias. LSP
  completion, OCS `VAL.NUMERIC-METHODS`, Book §10 (EN/RU), spec, LLM context
  and OASM follow ([#284](https://github.com/shootnix/okkama/issues/284)).

- Async: await is structured (#147). Cancelling a task cancels the future it
  awaits — a lazy future its `await` started, a spawned task, a runtime
  request — unless another task also awaits it, and the task resumes with
  `Cancelled` only after that future has settled, deferred async cleanup
  included. Before, a spawned task survived the cancellation of its only
  awaiter, and on the IR engine and the JVM the awaiter resumed while the
  awaited future was still cleaning up; the IR engine could even crash
  (`a resumed task is suspended`) when that cleanup awaited. One rule on the
  IR engine, the Sparrow core and the JVM; OCS contract
  `ASYNC.STRUCTURED-AWAIT` with `L08.cancel.awaited-cleanup-settles`,
  `.awaited-spawned` and `.shared-awaited-survives`; scheduler tests; Book
  §31.5, spec §18, async guide §4.6, LLM context (EN/RU)
  ([#147](https://github.com/shootnix/okkama/issues/147)).

- Language: `bool.string()` (and `string(flag)`, `@string(..., flag)`) is
  `"true"` or `"false"`, no longer the `"1"`/`"0"` inherited from the Perl
  implementation; map keys in error messages read the same way, on every
  engine. No std module printed a boolean; OCS cases that did now
  expect the words. Book §5.1.1 and chapter 10, spec §6.6/§6.6.1, LLM context,
  LSP completion (EN/RU) ([#262](https://github.com/shootnix/okkama/issues/262)).

- JVM: the runtime (`okkama.rt`) is built for Java 22 as one source tree
  (the FFM natives in `src22` joined the rest), so the JVM engine needs JDK
  22+ — which the FFM natives already required in practice. `okkama run
  --engine jvm` with an older `java` stops with a message instead of an
  `UnsupportedClassVersionError`, and `okkama env` says the `java` is too
  old. The Java 8 workarounds are gone: `Process.pid()` and TLS ALPN are
  called directly, `repeat` uses `String.repeat`. Generated classes keep
  class-file version 52.

- Language: `int.pow(int)` is exact (#228): `(2).pow(100)` is
  `1267650600228229401496703205376`, not `1.2676506002282294e+30`. A negative
  exponent of an `int` is an error ("a rat base takes a negative one"); new
  `rat.pow(int)` is the exact rational, and a negative exponent inverts it
  (`(2.rat()).pow(-3)` is `1/8`); `float.pow(float)` is unchanged. The static
  type cannot depend on the exponent's sign, so the issue's "a negative
  exponent gives a rat" became `rat.pow`. Lowering raises by squaring with
  the exact `int`/`rat` products every engine already has. OCS
  `L02.int.pow-exact`; Book chapter 10 (EN/RU)
  ([#228](https://github.com/shootnix/okkama/issues/228)).

- `db/postgres`: a query is one round trip. Bind, Execute without a row limit
  and Sync go out together and the server streams the whole answer; the row
  reader decodes it `batch` rows at a time as it is read from the socket (TCP
  flow control bounds the buffer), instead of asking for each page with
  Execute+Flush and settling the portal with a second Sync. A reader closed
  early leaves the rest of the answer to be read and discarded by the next
  operation on the connection. An implicit commit's failure (a deferred
  constraint in autocommit) is still reported by the query it belongs to.
  Docker arm64, 500 lookups against psycopg2: a prepared point query 114 →
  68 µs (psycopg2 73 µs), a statement lookup 114 → 75 µs, streamed scans
  18–20% faster, the rest unchanged. A new live-server check
  (`okkama-docs/examples/std-v4/db-postgres-stream-check.ok`) covers paging,
  page boundaries, early close, failures mid-stream and in parsing, COPY
  refusal, transactions with a reader closed early, rollback and the implicit
  commit failure, on the core and the IR engine
  ([#11](https://github.com/shootnix/okkama/issues/11)).

- Compiler: building an array or map with `reduce` is linear (#243). When the
  block uses its accumulator only as its value — alone, or as the left end of
  `acc + ...` — the accumulator lives in a builder (the one `map` and `filter`
  already use on every engine): `acc + [x]` appends in place, amortized O(1),
  instead of copying everything gathered so far; a map accumulator grows the
  same way from `[:]` by literal entries. Nothing else can hold an
  intermediate accumulator, so results are unchanged; any other use of `acc`
  (a read, a call, a capture, a merge with a map value) keeps the copy.
  Building n integers, before → after: n = 100000 takes 8.5 s → 0.16 s on the
  core, 44 s → 0.25 s on the IR engine, 2.9 s → 0.28 s on the JVM; n = 10^6
  now takes 0.18 / 0.67 / 0.35 s. OCS `L05.reduce.accumulator-in-place`
  checks both paths on every engine; the Book (EN/RU) describes the rule
  ([#243](https://github.com/shootnix/okkama/issues/243)).

### Security

- RSA private operations are constant-time and blinded on every Rust engine:
  key generation, PSS and PKCS#1 v1.5 signing and OAEP decryption run in
  aws-lc (`aws-lc-rs` 1.18.2) instead of the `rsa` crate, which has no fix for
  the Marvin timing attack (RUSTSEC-2023-0071). Signature checks of `crypto/rsa`
  and X.509 go through aws-lc's own RSA verification, so certificates keep
  what the JDK accepts: SHA-224, PSS with any salt and the default SHA-1
  parameters, keys from 1024 to 16384 bits. The `rsa` crate and its advisory
  exception are gone (`cargo audit --deny warnings` passes with no
  exceptions). Generated keys and PSS/OAEP randomness now come from aws-lc's
  CSPRNG: the `entropy` arguments of `crypto/rsa` stay for the API but no
  longer make these outputs reproducible. Imported PKCS#8 private keys must
  be 2048 to 8192 bits (a smaller one is InvalidCryptoInput). Every
  operation got faster (2048-bit key on the core, M2 Max): PSS verification
  239 → 75 µs, OAEP decryption 5.5 → 3.5 ms, PKCS#1 v1.5 signing 4.6 → 3.3 ms,
  PSS signing 4.2 → 3.8 ms. OpenSSL vectors cover SHA-384/512 signatures,
  OAEP-SHA384 and the X.509 legacy forms
  ([#70](https://github.com/shootnix/okkama/issues/70)).

### Fixed

- Sparrow core: a TLS read that completed and was then cancelled before its
  result reached the program lost the plaintext it had taken (the core's
  part of the abandoned-read fix below; `L11.tls.cancelled-read-keeps-data`
  failed on CI). The read's result now puts undelivered bytes back in front
  of the plaintext buffer when it is dropped unpublished.

- JVM: splitting a large function no longer panics (`unwind targets take a
  failure`, #305). When the moved part of a function over the method limit
  held the handler of its own root — a `try` handler is dominated by the
  block that enters the `try` — the remaining call kept an unwind edge into
  the moved code, which the renumbering turned into the entry block. The
  call now keeps its handler only when the handler stays outside. Exposed in
  0.4.1 by block merging (#292); found by the generated t/ suite (#245).

- Gate: the `ir-passes-off` and `ir-gc-stress` conformance runs get four
  times the case budgets. Without the IR passes tail calls are real calls,
  and `L03.recursion.tail-constant-stack` (10^7 of them in the adapter's
  debug build) timed out on the CI runner.

- IR: closures paid region tracking in any program with a `destroy` (#292).
  Lowering judges plainness per type and treats every function value as
  possibly owning a resource, since a closure's captures are not in its
  type. So in a program that merely reads an environment variable
  (`os/environment` imports `fs/directory`), every `fn.closure` was
  tracked, every function parameter adopted, and every call of a composed
  closure entered and left a region. `benchmarks/09-closures` with that
  import ran 2× slower on the IR engine, 1.5× on the Sparrow core and 6× on
  the JVM. The `regions` pass now recomputes plainness over the finished
  module: an object is plain without a `destroy` and with plain fields, a
  role when only plain objects do it, a function value when no closure
  captures a non-plain value. It drops `adopt`/`track`/`transfer`/`keep` of
  plain values before removing empty regions. The `jumps` pass now runs
  after `undef`, and it also merges a block into its single predecessor, so
  the scope blocks of removed regions no longer cost a jump each. The pass
  order is `regions, tail-calls, fold, hoist, undef, jumps`. 09-closures
  with `os/environment` now runs as fast as without it on every engine.
  Tests in `lowering_ownership.rs`, including a closure that captures a
  resource and stays tracked; LLM guide and phases.md updated
  ([#292](https://github.com/shootnix/okkama/issues/292)).

- IR engine and JVM: a TLS read abandoned by a timeout or a cancellation
  could lose the record it received (#12 follow-up). The abandoned read's
  worker kept waiting on the socket, and what it got was put back into the
  plaintext buffer only by the completion on the main thread, after the
  reader lock was free. A next read that took the lock first found the
  buffer empty, read on, and met the end of the stream: `TlsFailure`
  `invalid_data`. Under the gate's load it failed
  `L11.tls.cancelled-read-keeps-data` once (IR engine, passes off). The
  worker now puts what it received into the buffer under the reader lock,
  and a live read takes it from there. The Sparrow core reads in the task
  itself and was not affected.

- IR: a self tail call stayed a call in any program with a `destroy`
  anywhere, even in an imported std module (#285). Lowering then opens a
  region in every scope, and the `tail-calls` pass, which ran first, saw the
  empty `region.leave` after the call and gave up; the `regions` pass removed
  the region only later. A 3 000 000-step tail recursion that imported
  `os/environment` took 241 MB on the Sparrow core and 1.9 GB on the IR
  engine instead of 10 MB. The passes now run `regions, tail-calls, fold,
  hoist, jumps, undef`, and the tail-position check follows empty
  trampoline blocks the result reaches `ret` through. A call before an owned
  object's destroy is still not a tail call. LLM guide and phases.md updated
  ([#285](https://github.com/shootnix/okkama/issues/285)).

- Sort: a stopped or failing `ordering()` no longer crashes the engine
  (#283). Since #238 a budget that fired in the middle of a sort made the
  remaining comparisons answer `Equal`, and the IR engine answered `Equal`
  after a `Comparing.order()` of the program threw. Both contradict earlier
  answers; Rust's sort may detect that and panic, and on the Sparrow core the
  panic happens inside an intrinsic and aborts the process (seen once in the
  wave-17 gate, `budgets::a_long_sort_stops_for_the_timeout`). The stop now
  leaves the sort by unwinding with a private marker, caught inside the
  intrinsic, and the sort stays Rust's stable sort. Unit tests stop it at
  several fixed comparisons. OCS `L09.compare.failing-order-stops-the-sort`
  ([#283](https://github.com/shootnix/okkama/issues/283)).

- JVM: `==` on objects that refer to each other no longer overflows the
  stack (#278). A constructor that hands its `self` to a collaborator makes a
  cycle; the generated `equals` and `hashCode` recursed around it until
  `StackOverflowError`, while the IR engine and the Sparrow core returned the
  answer. Classes of such objects (those with the hidden `$pending` field,
  #271) now compare through `Obj.cyclicEquals`, which assumes a pair already
  being compared further up the stack equal, as the other engines do, and
  hash through `Obj.cyclicHash`; other classes are unchanged. OCS
  `L04.equality.cyclic-graphs`
  ([#278](https://github.com/shootnix/okkama/issues/278)).

- Objects: an action called on an object whose constructor has not returned
  is rejected on every engine (#271, spec §11.4). A constructor may hand its
  `self` to a collaborator; a call through that reference read fields the
  constructor had not bound yet, and the result depended on the engine: an
  invalid-state error on the IR engine, a `NullPointerException` on the JVM,
  and a segmentation fault on the Sparrow core. Now it is
  `Action "describe" on object "Tree" was called before its construction
  completed`, through a collaborator, a role or a bound action. A direct
  call is rejected before its arguments are evaluated. An object whose
  constructor failed after `self` escaped never completes. Lowering gives
  only objects whose constructor uses `self` as a value a hidden last field
  `$pending` (unbound until the constructor returns) and checks it with the
  new OIR instruction `obj.pending` and trap code `pending` (Sparrow trap
  code 5, SPB format 12); other objects are unchanged. In std this concerns
  `Http2ServerStream` and `MultipartFormPart`. OCS: five
  `L04.invariant.escaped-self-*` cases. Book §18.2 (EN/RU), OIR-1 §16.1,
  Sparrow book ([#271](https://github.com/shootnix/okkama/issues/271)).

- Tests: a successor passed to `test.is` or `test.isnt` is an ordinary
  argument (#275). Passed directly it crashed the compiler (`lowering
  produced invalid IR: … parameter %0 cannot have a successor type`), and
  passed through a binding it did not count as used. The checker now
  consumes the argument as every call argument does. The Book's own example
  `test.is(bottle.open().close(), bottle)` works. OCS
  `L07.test-runner.successor-assertions`
  ([#275](https://github.com/shootnix/okkama/issues/275)).

- Docs: map keys follow decision D-007 everywhere (#274). The spec (§6.3),
  the LLM context and the Book (§5.3, §8.2, §31.1, EN/RU) no longer say that
  `byte` values are map keys; `%byte:V` stays a compile error, as the OCS
  case `L05.map.byte-keys-rejected` requires. Book §31.1 also no longer says
  that objects are not keys: value objects are keys since #212
  ([#274](https://github.com/shootnix/okkama/issues/274)).

- db/postgres: closing a reader early no longer costs the rest of a long
  answer (#240). Since #11 a query streams its whole answer, so a reader
  closed after a few rows of millions left the rest for the next operation to
  read and discard. The session now keeps the backend's `BackendKeyData`.
  Past 8192 discarded rows or 1 MiB, a connection with a cancel route sends a
  CancelRequest over a separate plain TCP connection and waits until the
  server closes it, so the signal reaches the backend before anything new is
  sent. It then drops the short remainder and its `57014`. New public API,
  additive: role `PgCancelRoute`, `PgCancelTcp(endpoint)` (passed by
  `PostgresTcp` and `PostgresTls`) and
  `PgTransport(input, output, origin, cancel)`; the three-argument
  constructor means no route, and the answer is read through as before.
  Checked against PostgreSQL in Docker
  (`okkama-docs/examples/std-v4/db-postgres-cancel-check.ok`): 10⁷ rows
  closed after the first one settle in 18 ms on the core, where reading 10⁶
  through takes 182 ms. The IR engine's drain itself is slow (#280).
  std guide `db-postgres.md`
  ([#240](https://github.com/shootnix/okkama/issues/240)).

- Terminal: input typed after a read was interrupted reaches the next read
  (#12). A terminal read that a timeout or a cancelled task gave up still
  had its blocking `read(2)` waiting on the reader thread, and every later
  read queued another one behind it. The cancelled one took the input and
  kept it, while the new read waited on an empty terminal: the program hung.
  The IR engine and the Sparrow core (`runtime-core`) and the JVM now keep
  one `read(2)` in flight, and its bytes go to whichever read is waiting
  (at most its limit, the rest kept). A pseudo-terminal test
  (`terminal_pty`, Python driver under `okkama-cli/tests/terminal`) covers
  this and the resize stream on all three engines: the first observation
  is the geometry the source opened with, the next waits for a change
  (now said in `terminal/screen`). OCS `L11.tls.cancelled-read-keeps-data`
  with the `tls-late-echo` fixture shows that a TLS read interrupted the
  same two ways loses nothing
  ([#12](https://github.com/shootnix/okkama/issues/12)).

- Runtime: cleanup regions no longer walk a large structure again for every
  value that points into it (#9). A region learns which resources to destroy
  by walking what a value reaches; the walk stopped at objects owned
  elsewhere, but arrays, maps and closures have no owner, so a fresh value
  pointing at 100 000 owned records cost a walk over all of them, on every
  track and every close: a loop that builds such values was quadratic. Arrays,
  maps, closures and `any` boxes are immutable, so what one reaches is at
  most as old as itself: a container whose walk meets no free resource is now
  remembered with the identity counter of that moment and skipped by later
  walks, until a resource at least as old becomes free again (a scope returns
  it), which forgets all of them; a walk through a closing region skips it
  only while every entry of that region is newer. The same rule on the Sparrow
  core (a header bit and a side table, dropped by the collector), the IR
  engine and the JVM (weak side tables); the JVM walk is no longer recursive.
  The #9 probe (40 000 records, 2 000 rounds) on the core: 1.40 s → 0.03 s.
  New benchmark `11-ownership` (100 000 records in an array and a map, 300
  rounds); OCS `L07.ownership.shared-structure-returned` and
  `.shared-structure-loop`
  ([#9](https://github.com/shootnix/okkama/issues/9)).

- IR engine: calls are as fast as before #153 again (#265). The `const`
  globals were built lazily from inside the instruction loop, which called
  the interpreter back from the instruction evaluator; that one call edge
  changed how the whole loop was compiled, and every call-heavy program lost
  8–15% (`fib`, role calls, closures) while executing the same instructions.
  Constants are now built when the engine starts, like the JVM's `<clinit>`
  (an init is pure and reads no other global, so the order is not visible),
  and `global gN` only reads. Two smaller causes went with it: the frame's
  register file is filled by a closure of its own (a second `vec![None; n]`
  had pushed `from_elem` out of line on every call), and awaiting an
  `async destroy` (#154) moved out of the loop into a cold function. PGO
  build against wave 10: calls +0.6%, roles +2.2%, arithmetic −0.6%,
  objects −1.1%, closures +5.6% (PGO compiles that path differently;
  without PGO both versions run the same instructions)
  ([#265](https://github.com/shootnix/okkama/issues/265)).

- Compiler: a `reduce` into an array or a map (#243) whose elements reach a
  `destroy` compiles (#266). It was an internal error (`region.track …:
  builder<Res> has no identities to own`): lowering tracked the builder
  itself, and closed each step keeping it. A builder is never walked, so
  each value pushed onto it is now handed to the scope of the `reduce` (the
  step's regions transfer what they own, that scope adopts what is free),
  and a builder is never tracked, adopted or kept. What the block gathers —
  made there or bound with `let` — lives as long as the result, as with
  `map`. Lowering test and OCS `L07.ownership.reduce-builder`
  ([#266](https://github.com/shootnix/okkama/issues/266)).

- Budgets: a long host operation no longer outlives `--timeout` (#238). On
  the Sparrow core a PBKDF2 derivation of 10⁹ rounds ran on after the time
  was up (the IR engine's command line already stopped it a second later).
  PBKDF2 now counts its rounds itself and looks at the run's interrupt flag
  every 4096 of them, on every engine (`okkama_native::interruptible`; RFC
  7914 vectors), and the run then ends interrupted, unseen by `catch`.
  Argon2 cannot look at the flag, so `okkama run --engine sparrow` and
  `sparrow run` now also end the run a second after the deadline, as the
  IR engine's run did; SQLite queries already stopped (they run on the
  connection's thread while the scheduler waits). CLI test
  `long_host_operations_stop_for_the_timeout`; Book, toolchain commands (EN/RU)
  ([#238](https://github.com/shootnix/okkama/issues/238)).

- Compiler: one function of many sequential loops compiles in linear time
  (#239). Three phases were quadratic on it. Lowering copied the value of
  every local at every branch; it now snapshots only the variables that can
  change (fields, staged fields, rebindable locals and `^C` values). Loop
  constants were hoisted to the function's entry block, so thousands of them
  lived at once and register liveness and colouring went quadratic — the
  Sparrow verifier even refused 25 000 loops (45 001 blocks × 97 503
  registers); they now go to the nearest block before their loop. And the
  successor flow check scanned every outer local at each scope exit; it now
  looks outside only when the scope's bindings hold an obligation. 25 000
  `reduce` blocks (10⁵ instructions): codegen 4.1 s at 2 500 blocks before,
  0.15 s at 25 000 now; the whole pipeline is about 0.5 s and grows linearly.
  `tests/large_functions.rs` gains the `loops` shape
  ([#239](https://github.com/shootnix/okkama/issues/239)).

- Docs and LSP: chapter 10 of the Book listed `less`/`more` for `char`, which
  has only `order`; LSP completion offered `int.pow` as a `float` (it is the
  exact `int` since #228) and described `bool.string()` as `"true"`/`"false"`
  (it is `"1"`/`"0"`).

- Docs: the Book (EN/RU) said that ordering accepts mixed numeric operands;
  it needs one category on both sides, like equality — `1 < 2.5` is an error
  (OCS `L02.ordering.int-float-rejected`), written `1.float() < 2.5` — and it
  now also says that `&&`/`||` short-circuit. Chapter 10 listed `bool` among
  receivers without builtins; it has `string()` (`"1"`/`"0"`). Found while
  checking the Okkama Bible, chapter 9.

- A long sort no longer outlasts `--timeout` (#238, first part). Sorting
  scalars or text ran to its end inside one native operation: three million
  strings took 3 s on the core and 8 s on the IR engine after a one-second
  timeout. The comparisons now look at the interrupt flag and the memory
  budget every 4096 steps and stop at once (1.25 s with `--timeout 1` on
  both); the program's `catch` does not see the stop and its cleanup does not
  run, as for every budget. A sort through `Comparing` was already
  interruptible: each comparison is a call. Unlimited runs are unchanged
  ([#238](https://github.com/shootnix/okkama/issues/238)).

- `db/postgres`: `COPY … FROM STDIN` no longer hangs the connection. The
  server ignores a Sync that arrives in copy-in mode, so after the driver's
  CopyFail it waited for another Sync that never came; the refusal now sends
  one ([#11](https://github.com/shootnix/okkama/issues/11)).

- Deep recursion no longer looks like a hang (#241). A failure thousands or
  millions of frames deep rendered one trace line per frame, each with a
  `getcwd` call, before collapsing them — tens of seconds after the run had
  ended, past `--timeout`. The trace is now built once per distinct place and
  only for the lines it shows (`stack_text_by`), and not at all with
  `OKKAMA_TRACE=off`: a failure 300000 frames deep prints in 0.17 s instead of
  12.6 s on the core. On the core, a stack overflow under `--max-memory`
  (which caps the machine stack) is now `Execution budget exhausted: memory`,
  as on the IR engine and the JVM, instead of `RuntimeError: Stack overflow`
  ([#241](https://github.com/shootnix/okkama/issues/241)).

- Book: chapter 10 (EN/RU) lists every builtin action the compiler has —
  string `empty`, `trim`, `chomp`; array `item`, `first`, `last`, `cursor`,
  `ordering`; numeric `abs`, `sign`, `pow`, `sqrt`, bit actions; rat
  `numerator`, `denominator`; Future `cancel`, `done`, `cancelled` — and a
  test reads the compiler's table from `analysis/check/builtin.rs`, so a new
  builtin fails until the Book names it
  ([#224](https://github.com/shootnix/okkama/issues/224)).

- Diagnostics say what the rule is and what to write instead: assigning a
  field after construction ("objects are immutable; return a successor",
  [#226](https://github.com/shootnix/okkama/issues/226)); `class`,
  `interface`, `extends`, `implements` and `static` from other languages
  name the Okkama construct — `object`, `role`, fields and `does`
  ([#227](https://github.com/shootnix/okkama/issues/227)); a `match` used as a
  value that leaves enum cases out names them instead of "Expression does
  not produce a value" ([#229](https://github.com/shootnix/okkama/issues/229));
  `ordering()` without the `Comparing` role loaded says which import is
  missing ([#233](https://github.com/shootnix/okkama/issues/233)). The words
  stay ordinary identifiers: only code that failed to parse before gets the
  new messages.

- LSP: completion after a dot follows every link of a chain. Besides calls
  on names, calls and literals, it now resolves a field reached through
  `self` (`self.total.`), an element after indexing (`all[0].`), the
  element type in generic builtin results (`all.first().`, `m.values().`),
  the bindings of functional blocks (`list.map() -> m { m. }`, `reduce`'s
  accumulator from its initial value) and locals declared since the last
  successful check (`let c = sum.cents();` then `c.`, or `let d is Money`)
  ([#225](https://github.com/shootnix/okkama/issues/225)).

- Compiler, imports (O06): a module imported from many modules is read and
  parsed once. Loaders now name a module's identity (`ModuleLoader::resolve`,
  the canonical path for files) before reading it, and the import walk
  reuses a module it has loaded, or reports a cycle, from that alone. `check`
  of 256 modules that all import one shared module took 2.9× the time of the
  same project with one such import; it now takes 1.3× (the remaining cost is
  path lookup and the import edges themselves). Loop constants are hoisted
  with one linear pass over the strongly connected components instead of a
  search per block, so functions of any size get it (the 4096-block cap is
  gone); the pass shares that routine with the recursion analysis
  ([#51](https://github.com/shootnix/okkama/issues/51)).

- Runtimes, strings (O07): slicing a non-ASCII string one code point at a
  time, and `length()` in a loop, are linear on every engine instead of
  quadratic. Each engine remembers, for its four most recently indexed long
  strings, the code point count and the byte (on the JVM, `char`) offset of
  the last position reached, and walks from the nearest of the start, the end
  and that position; strings stay code point indexed. Random access costs the
  distance from the nearest of those three points. Slicing 16 384 Cyrillic
  letters one by one took 0.84 s on the IR engine, 0.44 s on the Sparrow core
  and 0.68 s on the JVM; it now takes 0.13 s, 0.05 s and 0.12 s, and 65 536
  letters take 0.49 s, 0.13 s and 0.20 s (a debug build against the 0.4.0
  release). The core's cache is cleared at each collection, the only time a
  string's memory is freed; the IR engine and the JVM hold weak references
  ([#52](https://github.com/shootnix/okkama/issues/52)).

- Compiler, anonymous actions (H-33): an anonymous action written in an
  action captures `self` only when its body reads the receiver — `self`, a
  field, an action of `self`, `fake.storage`, or a nested anonymous action
  that does — as the Book (§18.5) already said: unreferenced values are not
  retained. Before, every such action captured `self` and kept its whole
  graph alive, and with it postponed the receiver's `destroy`. Twenty
  objects holding 200 000 integers each, of which only the returned adders
  are kept, peaked at 111 MB on the IR engine and 46 MB on the Sparrow core;
  now 33 MB and 31 MB. The walk over the body names every expression and
  statement form, so a new form must decide whether it reads the receiver.
  An OCS case shows the receiver destroyed at once when its action does not
  read it, and kept while it does
  ([#53](https://github.com/shootnix/okkama/issues/53)).

- Sparrow core, load-time contracts of generic intrinsics: payloads are
  checked by type, not only by representation. The object types a channel's
  state passes through (`sending`/`receiving`) form a class, and every
  `send` value and `item` result of a class must be one type, so a crafted
  SPB pairing `Sending<string>` with `Receiving<SomeObject>` is refused by
  `sparrow check`. `FutureScope.result` requires its receiver's action to
  take the group and yield the result type; raced, first-successful and
  timed results and `AllFutures` results must be the type of the futures
  they wait for; `fake.storage` reads compare stored and requested types
  structurally. Tests pair crafted copies of these intrinsics with compiler
  output ([#69](https://github.com/shootnix/okkama/issues/69)).

- Compiler, types (H-26): a local's type may be at most 64 levels deep. A
  chain of `let`s can deepen a generic type one level per step
  (`deeper() is Box<@T>`), and lowering keeps a full copy of each value's
  type: 2 000 levels took 4.7 s and 840 MB to build, 20 000 more than
  16 GB. Now the first local past the limit is one error. Interning types
  was measured and is not worth it: checking `std/web/app.ok` with its
  imports spends about 3 % of its working time on types, against lexing,
  parsing and allocation ([#49](https://github.com/shootnix/okkama/issues/49)).

- Compiler, very large functions (H-28, H-39): three phases were quadratic
  in the size of one function and are linear now. The ownership flow of the
  analysis copied the obligations of every local at every branch (now only
  the locals that hold any); register allocation copied and scanned a
  bitset as wide as the function at every instruction (now a set of the
  live values); the OIR verifier climbed the dominator tree for every use
  (now one interval comparison). A function of 100 000 `let`s builds to
  Sparrow in 0.2 s instead of 13 s, 100 000 `if`s in 0.5 s instead of
  62 s, and `check` of 25 000 `reduce` blocks takes 0.4 s instead of 9 s.
  `tools/gen/large.py` generates such functions and reports each phase's
  growth; a release test requires the whole pipeline to grow linearly. The
  lowering of very many branches over very many locals is still
  superlinear (0.6 s at 10⁵ instructions): #239
  ([#50](https://github.com/shootnix/okkama/issues/50)).

- Sparrow core, budgets: one operation whose result can be much larger than
  its operands is measured against the memory budget before it is built —
  string concatenation, `join`, `replace` (an empty pattern included) and
  the BigInt power native, next to the BigInt product checked already. A
  join of 200 copies of a 1 MiB string under `--max-memory 64M` used to
  finish with a 589 MB peak and exit 0, because no poll followed it; it now
  stops with "Execution budget exhausted: memory" before allocating. Long
  natives that cannot be interrupted (sorting, provider calls) and
  per-runtime memory accounting remain open in #238
  ([#72](https://github.com/shootnix/okkama/issues/72)).

- Runtimes: host state belongs to one run, not to a thread (Q03). Every
  engine entry (IR program and test runs, the previous Sparrow VM, each
  Sparrow core run) opens a host session — a copy of the process's working
  directory and environment plus the launcher's defaults — and the program's
  `chdir` and environment changes live and die with it, so a second program
  on the same thread no longer inherits them; the process is never changed.
  On the Sparrow core the session is also the single source of truth: its
  own natives and the bridged origin natives (SQLite paths, x509 files,
  collation) now see the same directory and environment after `chdir` or
  `env.set`, a spawned worker starts in the program's directory rather than
  the process's, and a core worker no longer changes its process's
  directory. A later `default_environment` for the same name replaces the
  earlier one ([#40](https://github.com/shootnix/okkama/issues/40)).

- Build: the declared MSRV is true again — `rust-version` is 1.97 (Cranelift
  0.137 needs it; it said 1.85), and a CI job *MSRV* checks every target on
  exactly that version. Releases no longer build on a moving `stable`: the
  gate image (`make gate`), the CI gate and the package builds install the
  toolchain pinned in
  `tools/release-toolchain.txt` (1.97.1), each package records its compiler
  in `lib/okkama/TOOLCHAIN`, and `gate.json` records `rustc -V`
  ([#41](https://github.com/shootnix/okkama/issues/41)).

- Build: debug tests and clippy are green and part of the release gate (Q05).
  The gate runs the whole workspace's tests unoptimized too (stack depth and
  debug assertions show only there; it replaces the debug drop-test subset)
  and `cargo clippy --workspace --all-targets -- -D warnings`. Clippy's one
  error (`approx_constant`: a literal τ that is bit-for-bit
  `std::f64::consts::TAU`) and its warnings are fixed or allowed where they
  are by design, each with its reason: `transmute` of `dlsym` addresses to
  their function-table types, the legacy VM's large public error, recursive
  regex and pattern helpers with many arguments
  ([#42](https://github.com/shootnix/okkama/issues/42)).

- IR runtime, cycle collector (H-13): `OKKAMA_GC=stress` collects at every
  new candidate, and the gate runs the IR conformance suite that way, so an
  edge the collector miscounts frees a live value at once. Debug builds check
  that every node found garbage is really freed once its late edges are
  broken (an enumerated edge that is not a strong reference fails there).
  Tests close a cycle through every kind of late edge — object field, Future
  value and failure, thrown object, failure cause and secondary, closure
  capture, array and map builder, intrinsic state, `any` box and array —
  collect it after its last holder goes, keep it while held, and check that
  the count of live objects returns to its base after repeated cycles, with
  and without stress. A CI job runs the heap and collector tests under Miri
  ([#43](https://github.com/shootnix/okkama/issues/43)).

- Compiler, lexer (H-22): malformed number literals are one error over the
  whole literal instead of a number followed by a stray identifier that
  failed somewhere later — `1__0` and `1_` ("`_` may only separate two
  digits"), `1.5_`, and exponent notation `1e5`, `1.5E-3`, `2e+10` (Okkama has
  none). A token table pins numbers next to `.`, `..`, `..=`, `...`, `_` and
  the 64-bit bounds with exact spans; two OCS cases reject `1__0` and `1e5`.
  The `INT64_MAX` limit of `0x…`/`0b…` literals stays as documented; whether
  to lift it is #235 ([#45](https://github.com/shootnix/okkama/issues/45)).

- Compiler, parser (H-23): binary operators live in one table — token, AST
  operation, precedence level and chaining (`LEVELS` in `parser/expr.rs`) —
  instead of a precedence enum, its successor function and a token match
  written separately; the range level's "at most one operator" is a property
  of its row. A test parses every pair of adjacent levels both ways and every
  level chained with itself ([#46](https://github.com/shootnix/okkama/issues/46)).

- LSP and diagnostics, positions (H-25): the language server negotiates the
  position encoding — UTF-8 when the client offers it (byte offsets, no
  conversion), else UTF-32, else the protocol's UTF-16 — and every position
  it sends or reads uses it. A column past the end of a CRLF line now stops
  before the `\r`, and diagnostics are published with the version of the
  text they were computed from, so an editor can drop stale ones. Terminal
  carets follow display width: an East Asian wide character or an emoji
  takes two cells, a combining mark none. Tests round-trip every character
  boundary of Cyrillic, emoji, combining marks, tabs, LF/CRLF and the end of
  the file through all three encodings
  ([#48](https://github.com/shootnix/okkama/issues/48)).

- Book: numeric operands never mix (owner decision, #230). The Book said that
  `rat` mixes with `int`/`byte` and that ordering promotes; the compiler has
  always rejected both (`1 + 1 / 2`, `1 / 2 < 1`, `byte(2) + 3`). §5 and the
  operator table now state the rule: same category on both sides, explicit
  conversion otherwise ([#230](https://github.com/shootnix/okkama/issues/230)).

- Sparrow core: compiling a function with k values live across k fallible
  instructions took about 0.3 KiB × k² before it ran (476 MiB at k = 1000,
  1.8 GiB at 2000), so a small crafted SPB could exhaust memory at its first
  call. Every fallible instruction emits several Cranelift blocks and the
  frontend's stack-map liveness and register allocation keep a set per block.
  The core now counts registers live at each instruction × blocks it emits
  while it generates the function (live registers come from a leader-based
  analysis, `okkama_sparrow::live_counts`, run only for functions large
  enough to matter) and refuses the function before finalization once the
  estimate passes 256 MiB, or the run's memory budget when it is lower:
  "deep: compiling it would take more than 256 MiB ...". Probes from 1000
  live values up are refused within a 200 MiB cap; the heaviest functions of
  the repository (CLDR tables, ~722k live-register instructions but few
  blocks) compile as before. Straight-line instructions now share one
  Cranelift block; only jump, handler and resume targets start one. Benchmarks
  on the core: instructions −2.4…+0.6% (most −1%), time within noise
  ([#73](https://github.com/shootnix/okkama/issues/73)).

- Fuzzing: `tools/fuzz/run.sh` reported a target as a finding when its build
  was killed for memory (an ASan build of cranelift-codegen with one codegen
  unit needs several gigabytes). Targets are now built first, retried with
  one job, and a failed build exits with status 2 and `logs/build.log`
  instead of a finding. A 20-second smoke run of all six targets in a 6 GB
  container has no findings
  ([#34](https://github.com/shootnix/okkama/issues/34)).

- Sparrow core: an uncaught failure printed only its report; the IR engine and
  the previous VM also print the place in the source and the call stack. The
  core already recorded the activations a failure unwinds through; they are
  now in `Uncaught::trace` (`run_described` returns them), `okkama run
  --engine sparrow` maps them to source like the other engines, and
  `okkama exec` and `sparrow` print them in Sparrow terms (function,
  instruction number and text); `OKKAMA_TRACE=off` leaves them out. Every OCS
  case that fails at run time, and every failing case of the cleanup and
  async levels, prints the same place and stack on the core and on the IR
  engine. Only the failure path
  changed ([#10](https://github.com/shootnix/okkama/issues/10)).

- IR runtime: dropping a value graph with repeated edges recursed through
  destructors — the release queue took only references whose count was
  already 1, so in `[child.clone(), child]` the second copy freed `child`
  recursively; a DAG 20 000 levels deep overflowed the stack in debug. A
  container now lets go of its shared references in order, so the last copy
  joins the queue, and `any` is never boxed twice, so boxes do not nest.
  Chains and DAGs of 200 000 levels through objects, arrays, maps, `any`,
  builders and closures drop on a 2 MiB stack in debug and release
  ([#28](https://github.com/shootnix/okkama/issues/28)).

- Compiler: a name conflict now names both sources — two `take` of one name
  (`Object "Thing" is already declared by module "lib/a"; this import takes
  another from module "lib/b"`), a `take` of a name the module declares, an
  `as` namespace bound twice, and a name declared in two files of one module
  (reported in both files). The files of a multi-file module are taken in
  name order, so `okkama check a.ok b.ok` and `okkama check b.ok a.ok` print
  the same bytes; every conformance case and std module checks identically
  run to run and with its files reversed
  ([#33](https://github.com/shootnix/okkama/issues/33)).

- Sparrow core on x86_64: the macOS x86_64 package now has a smoke test (run
  under Rosetta 2 on the arm64 runner: a program, the core under a stressed
  collector and with budget polls), and CI runs the std inline tests on the IR
  engine and the core on x86_64 Linux next to conformance. Checked locally
  under Rosetta: the handler-liveness programs under collector stress, budget
  polls and interruption, and all benchmarks print what arm64 prints
  ([#27](https://github.com/shootnix/okkama/issues/27)).

- Releases: packages were published without the release gate, which ran only
  by hand, judged steps by the last line of their logs and carried on after a
  failed build. A reusable `gate.yml` now runs the gate on the exact commit,
  and `packages.yml` publishes a release or a nightly only after it passes.
  Every step's status comes from its exit code (`ocs` exits 0 only when all
  mandatory cases pass); a failed build stops the gate; `gate.json` lists the
  commit, the steps with their status and time, and the known std failures,
  shown in the run summary ([#26](https://github.com/shootnix/okkama/issues/26)).

- Compiler: generic code that asks for ever deeper specializations —
  `Grow<T>` constructing `Grow<Box<T>>`, or a method returning `Node<Box<T>>`
  and calling itself on it — monomorphized without end and took over 1 GiB in
  17 seconds. A chain of requests deeper than 256, or more than 262 144
  specializations, is now a diagnostic naming the newest requests of the chain
  (`Grow<Box<…>>.deep <- Grow<Box<…>>.deep <- …`). The repository's programs
  stay at depth 20 and 2 416 specializations
  ([#32](https://github.com/shootnix/okkama/issues/32)).

- Sparrow core: a function's machine frame grew with the number of its helper
  calls — every call with arguments took its own stack slot — so a long
  function could reserve more than the stack headroom and reach the guard
  page before the prologue's stack check. All calls of a function now share
  one argument area, and a frame above 256 KiB (a quarter of the 1 MiB
  headroom) is refused when the function compiles. Recursion through Rust
  (`destroy`, closures) is tested to end as `Stack overflow` with the process
  usable afterwards. `--stats` reports the largest machine frame. Compile
  memory for functions with thousands of live values:
  [#73](https://github.com/shootnix/okkama/issues/73)
  ([#25](https://github.com/shootnix/okkama/issues/25)).

- SPB: decoding and verification are bounded in memory. The verifier kept a
  register set per instruction, so a 234 KiB file of 32 000 registers needed
  132 MiB to check; it now keeps one per block leader (6 MiB for that file)
  and refuses a function whose sets would exceed 64 MiB before allocating
  them. The decoder charges every count at its element size against a 512 MiB
  budget before allocating, so a small file cannot expand into a huge
  program. A corrupted byte could make the verifier panic (a call checked
  against a callee whose signature was not yet verified); every signature is
  now checked before any body. Tests truncate a program at every byte and
  corrupt every byte with every value; all 2035 SPB of the repository
  round-trip byte for byte ([#24](https://github.com/shootnix/okkama/issues/24)).

- Compiler: loading imports recursed once per nested import, so a chain of
  16 000 small modules overflowed the stack (SIGABRT). The import graph is now
  walked with an explicit stack and cycle membership is a set lookup; cycles
  keep their diagnostic. A program's import graph is bounded — 1024 nested
  imports, 16384 modules, 256 MiB of module sources — and the import that
  crosses a bound is a compile-time error
  ([#20](https://github.com/shootnix/okkama/issues/20)).

- Dependencies: `crypto-bigint` 0.7.5 → 0.7.10 (RUSTSEC-2026-0387) and
  `ctutils` 0.4.2 → 0.4.8 (RUSTSEC-2026-0385) — their constant-time
  operations were not constant-time. CI now runs `cargo audit` on every
  change and weekly and fails on any new advisory. The one exception is
  RUSTSEC-2023-0071 (Marvin, `rsa`): no version of the crate fixes it; private
  RSA operations on the Rust engines are not constant-time until a provider is
  chosen ([#70](https://github.com/shootnix/okkama/issues/70))
  ([#19](https://github.com/shootnix/okkama/issues/19)). The conformance
  adapter no longer caches a failed build: a gate run without a new crate in
  its offline registry kept reporting every case unsupported.

- SQLite on the JVM: a cancelled query read the connection pointer without
  synchronization and called `sqlite3_interrupt` on it while the worker could
  be closing the connection — a possible use of a freed connection. The pointer
  is now published and withdrawn under a lifetime lock that `sqlite3_close`
  holds, and interrupt checks it under the same lock, as rusqlite's
  `InterruptHandle` does for the IR runtime and the core. Tests: a handle
  loaded before close, interrupts racing close and worker exit, and a
  conformance case cancelling running queries around `close`
  ([#18](https://github.com/shootnix/okkama/issues/18)).

- X.509: building a certificate path had no work bound — ambiguous issuers
  (same name and key identifier) made the search exponential, 10 supplied
  intermediates took 5 s and 11 did not finish, synchronously in TLS
  `authorize`. The path builder now skips issuers from which no trust anchor
  is reachable and links whose signature does not verify (each signature is
  checked once), and spends at most 1024 steps (a considered issuer, or a link
  of a path validated at an anchor); past that, verification fails with
  `resource_exhausted` (`CertificateLimitExceeded`). The JVM runtime runs the
  same search instead of JDK `CertPathBuilder`, so every engine finds the
  same path and stops at the same point
  ([#17](https://github.com/shootnix/okkama/issues/17)).

- Sparrow: a crafted `.spb` could make the core read an integer as a pointer —
  the core takes each register's machine representation from the declared
  types, and the verifier checked only structure. The SPB verifier now checks
  types: the operands and result of every instruction, call arity and argument
  and result types (`call`, `call.fn`, `call.role`, `call.cap`, `fn.closure`,
  `worker.spawn`), edge moves, handler failure registers, `ret`, role slots,
  cap fields, `destroy`, the entry (`int`, no parameters) and tests, and
  natives against the ABI signatures. The core checks intrinsic contracts at
  load (arity, kinds, Atomic/Event cell receivers, related objects, timeout
  helpers; `sparrow check` runs them), recognizes Atomic and Event cells by
  their constructor intrinsics instead of by name, and keeps every channel's
  messages to one representation. Generic payloads checked by type:
  [#69](https://github.com/shootnix/okkama/issues/69)
  ([#16](https://github.com/shootnix/okkama/issues/16)).

- Blade: a package name is checked as a module path (identifiers separated by `/`)
  before it becomes a directory of `pkg/`; `../../victim` as a name of a package
  without `.ok` files replaced files outside the project. Names differing only in
  letter case are refused, and a nested package cannot land on its parent's files
  ([#15](https://github.com/shootnix/okkama/issues/15)).

- Blade: `pkg/` and `blade.lock` change together. Both are prepared next to
  the old ones and a marker is written before the renames; a blade
  interrupted at any step is finished (marker present) or rolled back by the
  next `blade install`, and `blade build`/`run` refuse a half-committed
  project. `blade install SOURCE` holds the project for the manifest edit as
  well as the install ([#39](https://github.com/shootnix/okkama/issues/39)).

## [0.4.0] — 2026-11-07

Sparrow runs on a new core: every function is compiled to machine code
(Cranelift) before it runs, calls are direct, async frames are stackless and the
collector is precise — on the engine benchmarks about twice the previous Sparrow
and 4–6× Python, Perl and Ruby. Errors are enums everywhere (language and std),
`Maybe`/`Presence`, compact choices and hidden declarations arrive in the
language, and wave A of the hardening roadmap is done (0.3.3 and 0.3.4 were not
released separately; their changes are here). The repository now holds the code,
the conformance suite and the Book; other documents, benchmarks and examples
moved out.

### Added

- Optional semicolons on the last expression before `}`, preserving its value;
  `if true { 1 } else { 0 };` is accepted. Other statement terminators remain required.

- Non-nested compact choices: `condition ?? yes !! no` and
  `value ?? .some(v): v !! .none: fallback`. Branches are lazy and typed;
  enum patterns are exhaustive, with an optional final `_:` wildcard.
  Parentheses cannot bypass the nesting prohibition. Presence conversion is
  named `fold()` (replacing the initial `collapse()` spelling).

- Nominal generic enum variants with positional typed payloads, exhaustive payload
  patterns, scoped bindings and `_` wildcards. Enum type arguments can be inferred
  or supplied explicitly. Variant equality includes the active payload.
- Standard `Maybe<T>` (`some`, `none`) and `Presence<T>` (`some`, `none`, `missing`),
  available without imports. `Maybe<T> else fallback` is lazy and accepts throwing
  or panicking branches. Presence requires explicit `.fold()`; Maybe supports
  `.as_presence()`. Empty strings, zero and false remain present values.
- IR, Sparrow/JIT and JVM execution and conformance coverage for variant payloads,
  generic inference, absence, errors, module boundaries and resource lifetimes.

- [Trust model](docs/security-model.md): what Okkama handles safely today for source
  code, `.spb` bytecode, natives and embedding, what it does not, and which roadmap
  tasks move each boundary (H-01).
- JIT debugging switches (H-21): `OKKAMA_JIT_THRESHOLD=N` sets the hotness threshold,
  `OKKAMA_JIT_ONLY` / `OKKAMA_JIT_SKIP` (comma-separated symbols) limit which functions
  are compiled — for bisecting which compiled function changes a result —, and
  `OKKAMA_JIT_DUMP` prints their Cranelift IR. `OKKAMA_JIT_STATS` now also prints how
  many functions were compiled and refused, and how many times machine code was entered,
  in the middle of a function (OSR) among them.
- Release gate (H-08): `make gate` (`tools/gate/gate.sh`) builds, runs the Rust tests
  in release, conformance on IR, JVM and Sparrow and on Sparrow again with JIT `eager`
  and hotness thresholds 2 and 10 (mixed execution and OSR), checks that machine code
  and OSR actually ran, and runs the inline tests of every std module on IR and Sparrow
  against a list of known failures (`tools/gate/known-failures.txt`).
- The Book ([EN](docs/OkkamaBook.md), [RU](docs/OkkamaBook-RU.md)) covers the new language:
  absence as `Maybe`/`Presence` (ch. 7, 24), compact choices and the optional `;`
  (ch. 11, 24), exhaustive matches (ch. 12), error enums, catch patterns and
  `try?`/`try!` (ch. 14), successor rules and private successors (ch. 19), access
  through roles and state views (ch. 22, 30.6), role arguments of bounded generics
  (ch. 25), `hidden` declarations (ch. 26.3.1), lint rules (ch. 28.3) and updated
  appendices.
- VS Code extension 0.7.0: highlighting for enum cases and variants (`Shape.circle`,
  `Maybe<int>.none`, `.paid(amount):` in compact choices, catch patterns), built-in
  `Maybe`/`Presence`/`Error`, `try?`/`try!` and the contextual `hidden`; 76 tokenization
  checks.
- **Hidden declarations** ([design](docs/POC/private-objects.md)): `hidden object`,
  `hidden role` and `hidden enum` are visible only inside their module. `take` and
  namespaces reject them (`Type "payments.Payment" is hidden in module "payments"`);
  public signatures, constructors, role signatures, enum payloads, generic bounds and
  cap fields cannot name hidden types, and a public object cannot do a hidden role.
  Values leave the module only as a role, `Error` or `any`, so a hidden class behind
  public state roles cannot be bypassed from other modules. `hidden` is a contextual
  keyword. New lint `hidden-error`: a public action throws a hidden error enum.
  Front end only (spec §5.1.1).
- **Error enums** (error handling phase 1, [proposal](docs/POC/error-handling.md)):
  `enum DatabaseError is Error = (connection(message is string), closed);`. Values are
  thrown with `throw DatabaseError.connection("refused")`, caught with
  `catch e is DatabaseError` (then inspected with an exhaustive `match`) or with
  `catch e is Error`, and rethrown with their type intact. `text()` is generated from
  the case and payload: `DatabaseError.connection("refused")`. Payloads are limited to
  strings, numbers, `bool` and enums with such payloads (an error enum can be the
  cause of another); error enums are not generic. Error objects keep working
  alongside. Implemented in the front end and lowering only: an error enum value is
  the variant object, which now does `Error` with a generated `text()`, so the IR
  interpreter, Sparrow and JVM needed no changes (spec §9.5.1).
- **`try?` and `try!`** (error handling phase 4): `let rows = try? db.query(sql) else 0;`
  yields `Maybe<T>` (`none` when an error is thrown, nested `Maybe` is kept);
  `try! value` turns a thrown error into the panic `Unexpected error: <text>`. They
  cover the expression to their right up to `else`/`??`; `try?` rejects void calls and
  successor results. Desugared to `try`/`catch Error` in the checker (spec §9.6.1).
- **Catch patterns and type lists**: `catch DbError.failed(code, _) { }`,
  `catch DbError.closed, NetError.reset, Denied { }` and `catch e is DbError, NetError { }`
  (`e` is `Error`). A thrown value of a matching type but another case continues with
  the next clause of the same `try` instead of leaving it (spec §9.6).
- **Private successor actions.** A successor declared after `private:` is an internal
  transition: callable only as `self.name(...)` inside its object, with the usual
  must-use obligation; it cannot implement a role action. Previously both the checker
  and the IR verifier rejected it without a stated reason (spec §11.7).
- `okkama lint` rule `non-exhaustive-match`: a `match` over a payload-free enum without
  `else` that misses cases is reported as a warning (payload enums already make it an
  error). It reports nothing in `std`, `examples` or the conformance cases.
- Research report [Role-oriented typestate](docs/research/role-oriented-typestate.md)
  ("state views": roles per state, successors as transitions, payload ADTs for
  outcomes) and its experiments in `experiments/role-typestate/`, run by
  `make experiments` (`EXPERIMENT_ENGINES="ir jvm sparrow"` for every engine).

- `encoding/bytes`: parsing primitives that read byte values in place.
  `ByteSearch(source, pattern).from(start)` finds a delimiter (-1 if absent);
  `ByteRange(source, start, end)` reads a range as strict UTF-8 (`text()`, failing
  with `InvalidUtf8.at(offset)`), an ASCII decimal (`decimal()`, failing with the
  new `InvalidByteDecimal.at(offset)`), a fixed-width integer of 1 to 8 bytes
  (`integer(signedness, order)`, as `IntegerFromBytes` reads it), compares it (`equals`) or copies it
  (`bytes()`). Bounds outside the source fail with `OutOfRange`. Backed by the new
  `runtime/bytes` natives (sparrow-core only for now). The memcached-style parser
  on them takes 0.94 ms per 2000-record reply: 3.4× faster than the slice-based
  version on the new core, 4.8× faster than the same loop in Python (4.5 ms).
- `io/input`: `ByteInput(source is AsyncReading)` buffers an asynchronous byte stream
  for protocol parsers: `await ensure(count)` reads until `count` bytes are buffered
  (false at the end of the stream), `available()`, `peek(at)`, `take(count)`,
  `skip(count)`, `range(start, end)` (an `encoding/bytes` view of the buffer, no copy)
  and `await line(terminator, limit)` (`Maybe.none` at the end of the stream, the new
  `LineTooLong.after(limit)` beyond the limit). Refills append to the unconsumed bytes
  in one copy (`runtime/bytes` `bytes_join`).

### Fixed

- The IR interpreter, the JVM backend and `--engine sparrow-origin` lacked the natives
  the new `std/db` and `encoding/bytes` use (byte ranges, SQLite and PostgreSQL result
  pages): SQLite and PostgreSQL programs and byte parsing failed with "native provider
  is not implemented" there. IR and JVM now implement them; the previous VM does not
  (it is removed in 0.5, [#14](https://github.com/shootnix/okkama/issues/14)).
- JVM: errors the runtime raises for async intrinsics (`ChannelClosed`,
  `InvalidChannelCapacity`, …) are built as error enums; since they became enums the
  task constructing them died and the program waited forever.
- IR and JVM: cancelling a task cancels the native request it awaits when no other
  task awaits it, as on the Sparrow core, so a SQLite cursor opened by a cancelled
  query is released instead of keeping the connection busy (`SQLITE_READER_ACTIVE`).
- Sparrow core: an external request (SQLite, network, files) always suspends its
  caller once, even when its reply is already there; a fast worker (seen on Linux
  x86_64) let a spawned fetch finish before `cancel` right after `spawn`.
- Sparrow core: failure reports read like the other engines' (`RuntimeError: …`,
  `Panic: …`, the error type without its module, "Modulo by zero", missing natives,
  a program without an entry function).
- Sparrow core: capturing a command's output (`os/command_capture`) aborted the VM
  (`spawn_local` outside a `LocalSet`); provider tasks now run in the core's reactor.
- Calling an action through a cap (`customer.wtf()` on a `Customer::Public` value)
  reported "Can only call actions on objects"; the error now names the cap, says that
  an owner's action is not available through it and lists the cap's computed fields
  (`::name`) ([#8](https://github.com/shootnix/okkama/issues/8)).
- **An object successor declared `is ^Role` is rejected with a clear error** instead of
  passing the checker and crashing in IR verification with an internal compiler error;
  the checker also accepted a role the object does not do. An object successor returns
  `^Self`; `^Role` results belong to role signatures (spec §11.9).
- **A successor body can no longer end with a replacement value.** A final expression of
  the successor's own type (`self`, `Box()`) was silently discarded, unlike
  `return value;`, which was already an error. Final values of other types are still
  evaluated and discarded, as in void actions (spec §11.9).
- **Role-bounded generics accept role arguments at runtime.** `object Tagged<T: Named>`
  with `T` inferred as another role (`Counted`) type-checked, but lowering a bound call
  failed with "builtin role adapters … not implemented yet"; the value now narrows to
  the bound like an ordinary role-to-role assignment, on IR, JVM and Sparrow.
- Rebinding a role-typed variable now says that role-typed locals cannot be rebound,
  instead of claiming the role has no successor actions.

- **Long expression chains no longer crash the compiler** (H-03). The parser bounds
  its own recursion, but builds left-associative chains (`a + a + … + a`, `x.a.b.c…`,
  `x.f().f()…`, `x[0][0]…`) by a loop, so the tree was as deep as the chain was long and
  the recursive phases overflowed the stack: `okkama check` at about 6 000 links,
  `okkama ast` at 100 000. The tree is now measured right after parsing, by a walk with
  an explicit stack: deeper than 128 levels (`ParseOptions::max_tree_depth`; the
  deepest file in the repository has 37) or larger than 2^20 nodes (`max_nodes`) is a
  syntax error at the offending expression. A file with more than four tokens per
  allowed node is rejected before the tree is built. A chain of a million links is
  rejected in 0.2 s by every command.
- Syntax trees are freed without recursion: `Expr` moves its sub-expressions to a
  queue, so dropping a deep tree (on an error path, or one built in code) no longer
  overflows the stack (H-03).
- **JIT address arithmetic and machine frames are bounded** (H-02). A register slot
  offset was `register × 24` in `i32` without a check, so a hand-made `.spb` function
  with more than about 89 million registers made machine code write outside its frame.
  The JIT now refuses functions with more than 2^14 registers before generating code,
  calls functions with more than 512 registers through the helper instead of an area in
  the caller's frame, and publishes code only when Cranelift's frame for it is at most
  1 MiB, a quarter of the stack margin checked before every call. A function over a
  limit stays with the interpreter (`OKKAMA_JIT_DEBUG` prints why).
- **Building Sparrow bytecode for a function with thousands of values live at once no
  longer takes minutes and gigabytes** (regression of 0.3.2, H-28). 20 000 such values
  made the register allocator's conflict graph hundreds of millions of edges: 132 s and
  10.6 GB for one 20 000-line method. Above 2^22 edges a function now keeps one register
  per value, as before 0.3.2.
- The IR verifier checked the linearity of every array builder version by walking the
  whole function again: quadratic in a function with many `builder.push` (an array
  literal of 20 000 elements took 6.5 s in `okkama ir`). Builder uses are now collected
  in one pass; the same program builds in 1 s instead of 132 s (H-39).

- New Sparrow runtime (`sparrow-next`): garbage collection ran a full mark/sweep every
  128 allocations regardless of the live heap, so programs holding many values were
  quadratic (300 000 live role objects: 3.6 s). The trigger now follows the work traced
  by the previous collection, and pinned constants are no longer rescanned: the same
  program runs in 0.32 s, string and collection benchmarks 2–2.5× faster, with peak
  memory no higher than the reference Sparrow. `--stats` prints JIT exits, served
  operations, calls and collections; `tools/new-runtime/overhead-bench.py` compares
  timings with sparrow-origin and checks that outputs match. Integer `%` on machine
  integers now runs in JIT code instead of leaving it for every operation: the
  arithmetic benchmark went from 0.18 s to 0.07 s. Object, array, map, text and
  equality instructions are executed from JIT code through one heap call instead
  of leaving it for the interpreter loop (objects benchmark 0.25 s → 0.19 s).
- New Sparrow runtime: SQLite provider (`runtime/database`) on rusqlite with bundled
  SQLite, one worker thread per connection, cancellation that interrupts SQLite and
  releases what a cancelled operation still acquired; rows reach programs as DB
  cells. The SQLite conformance cases pass on it. Syntax errors reported by SQLite
  while preparing a statement now have kind `syntax`; the previous runtime reported
  them as `unexpected`. On the DB benchmark it is 2–3× faster than at the start of
  this work (point lookups 155 ms → 52 ms) but still slower than the previous
  runtime: a collector that rescanned large post-peak arenas, a system call on
  every 4096-instruction slice, and per-task scans of the whole program were removed.

- New Sparrow execution core (`crates/sparrow-core`, in progress): synchronous
  functions compiled whole with direct calls, one-word values, a precise collector
  on Cranelift stack maps, roles through dispatch tables, errors and `destroy`
  regions. It runs the language-core conformance programs (L01–L07) with the same
  output as the previous Sparrow and is 1.5–7× faster on most benchmarks. Async is
  stackless: the heap frame of an async call is its Future, `spawn` starts eagerly,
  cancellation reaches the innermost waiting frame. The async primitives run on it
  (Atomic, Event, Cancellation, Lock, Semaphore, Barrier, Channel, FutureGroup and
  FutureScope, AllFutures/Raced/FirstSuccessful, FutureCompletions, Before/Within,
  timers), and the async conformance programs (L08) give the same output as the
  previous Sparrow; a spawn-heavy benchmark runs in 0.3 s instead of 10.5 s. All
  native providers of the previous engine (JSON, regex, encodings, numbers, crypto,
  SQLite and DB cells, files, environment, input, processes, networking) run on it,
  as do `--test` and the test assertions.

### Changed

- **The core is the Sparrow runtime.** `sparrow`, `okkama run --engine sparrow`,
  `okkama exec` and the conformance adapter (`OCS_RUST_ENGINE=sparrow`) run on
  `sparrow-core`; process workers of the core run `okkama --worker FILE.spb` or
  `sparrow --worker FILE.spb`. The previous VM stays for one release as
  `sparrow-origin` and `--engine sparrow-origin` (removed in 0.5,
  [#14](https://github.com/shootnix/okkama/issues/14)). Known limitation: an uncaught
  failure on the core prints its report without the source line and call stack
  ([#10](https://github.com/shootnix/okkama/issues/10)). `make gate` runs the release
  gate in a container (`tools/gate/docker.sh`, `tools/docker/Dockerfile`; `make
  gate-local` without it): conformance on ir, jvm, sparrow and sparrow-origin, std
  tests on ir and sparrow.
- **The repository keeps the code, the conformance suite and the Book.** The
  specification, grammar, std reference, guides, roadmap, audits, runtime documents,
  benchmarks with their reports, examples and experiments moved to a documents folder
  outside the repository; `docs/` holds the Book (EN/RU) and `docs/LLM-GUIDE.md`, a
  guide for LLM agents working on the code. PGO trains on
  `okkama-rs/tools/pgo/workloads`; the allocation counter is
  `okkama-rs/tools/allocation-counter`.
- **SQLite rows without per-value native calls.** A fetched batch reaches `std/db`
  as one page of plain values (cell kinds, integers, floats, texts, bytes) instead
  of a native cell object per value; `Row.int`, `Row.text` and `Row.maybe` read the
  page directly. On the DB benchmark the new core with this `std/db` reads a row
  2–2.5× faster than the previous Sparrow (projection 6.6 ms vs 16 ms, streaming
  scan 50–56 ms vs 95 ms). Rows built by hand (`Row(columns, values)`) and other
  drivers keep native cells. New natives: `sqlite_reply_kinds`, `_values`,
  `_floats`, `_texts`, `_blobs`, `_row_bytes`.
- Native calls on the new core cost about 40 ns of boundary overhead (natives are
  resolved once per function, providers by table, handles hashed cheaply).
- **Fast byte and text processing on the new core** — the base for protocol drivers
  and parsers written in Okkama (the PostgreSQL client, future network modules):
  `@byte` arrays are stored one byte per element (they took eight), element reads
  compile to single-byte loads, and UTF-8 encoding/decoding copies once; `Atomic`
  is a heap cell whose `value`/`replace` compile inline; integer `&`, `|`, `^`, `~`,
  negation, shifts and `byte()` conversion have inline paths; ASCII strings have
  constant-time length and slicing; `slice` no longer copies the whole array;
  decimal integers up to 18 digits parse without BigInt. A memcached-style reply
  parser runs 10× faster than on the previous Sparrow (159 ms vs 1619 ms).
- `AsyncReading` and `AsyncWriting` moved from `net/stream` to `io/stream`, so byte
  input does not depend on the network stack: import them from `io/stream`.
- `async/atomic`: `AtomicClosed` is an error enum, `AtomicClosed.closed`; its text is
  `AtomicClosed.closed` (was `atomic state is closed`). Errors that the runtime raises
  for intrinsics may now be error enums (the runtime raises their first case) on every
  engine, so the remaining `async/*` errors can migrate the same way.
- Cleanup regions no longer walk the resource graph when they hold nothing; the
  walk marks visited objects in place instead of hashing them. Regions, `Event`,
  enum construction and comparison compile inline; native handles need no Rust
  allocation; the scheduler and SQLite workers spin briefly before sleeping on a
  reply. The SQLite driver awaits replies without a task group per request and
  reuses result columns while a session repeats a query shape. On the DB
  benchmark the new core is 3–6× faster than the previous Sparrow; against
  Python's `sqlite3` it reads rows within 1.5–2.6× and runs point queries about
  12× slower (they still cross to a worker thread).
- Database values are captured from their printers without native calls (the
  same one-event protocol and errors), and cleanup regions keep their first
  entries inline: inserts on the DB benchmark take 8.4 ms instead of 19 ms.
- The new core runs the embedded tests of the standard library like the previous
  engine (198 modules with identical reports; 8 more pass only on the new core),
  supports test fakes' storage, and `sparrow-next --stats`. Fixed on the way:
  map and array literals with more than eight entries failed on the new core.
- Big integers count their digits towards garbage collection, so programs that
  produce many large intermediate integers no longer grow without bound.
- `SPARROW_CORE_STATS=1` makes the new core print counts of its runtime events
  (natives, intrinsics, slow-path instructions, regions, awaits) at exit.
- `sparrow-next` now runs on the new core. The previous engine's binary is renamed
  `sparrow-engine` and stays until its removal. Process workers, `--stats` and fake
  storage work on the new core too.
- **The new core collects correctly on Linux/AArch64.** The collector found the
  stack slots of a machine frame from the frame record of the helper it had called,
  which only matches where Apple targets put that record; on Linux (Graviton, Docker
  on Apple silicon) a collection read wrong slots and crashed (the PostgreSQL client
  crashed after connecting). It now takes the frame's SP from its own FP and the
  frame size Cranelift reports. Rust code is built with frame pointers on non-Apple
  targets (`okkama-rs/.cargo/config.toml`, also in the PGO build), so the walk can pass
  over Rust frames. std tests pass on the core in Docker (239 of 240; the last needs
  ICU).
- **Large async frames no longer allocate separately**: heap size classes of the new
  core reach 128 words (were 32), so a call of an async function with more than 24
  registers takes its frame from a free list instead of a separate allocation per
  call. The migrated `std/db` crossed that edge in `QueryRows.next` (row scans allocated
  50% more and inserts ran twice as slow in the full DB benchmark); now both
  allocate 28 MB instead of 63–95 MB. `--stats` also reports the heap bytes allocated
  and the mean live heap after collection.
- `benchmarks/run.py`: the `sparrow-core` configuration (new core), `--okkama`,
  `--sparrow`, `--sparrow-core` binary paths, and per-run limits of CPU seconds and
  wall time (`--cpu-seconds`, `--timeout`); `export.py` compares the new core with the
  Sparrow VM of the earlier report.
- **The new core runs the natives it had not ported** through the origin runtime's
  implementations (`okkama-runtime-core`): arguments and results cross as its
  values by the ABI types, `any` arguments through their program types, and an origin
  `native object` lives in a core handle. 182 natives (INI, TOML, templates, HTML,
  X.509, PEM, time zones, Unicode data, random, statistics, UUID, …) and
  `runtime/debug` (its parts stay core values) now work there; 238 std modules pass
  their tests on the core (the other two need ICU and a network). Not yet available:
  TLS and the terminal (async, over core sockets).
- **Process workers on the new core**: `spawn Worker(...).sync_action(...)` runs the
  action in a child `sparrow-next --worker FILE.spb`, with the previous engine's
  OKW1 transfer format, output limit, cancellation (the child's process group is
  killed and reaped) and failure texts; a child started for a worker refuses to run
  `main`. Programs, outputs and failures match `sparrow-engine` (Linux, Docker).
- **Machine code without cross-core barriers**: the new core allocates machine code
  in one reserved region and no longer follows every lazily compiled function with
  a `membarrier` IPI on AArch64 Linux (Graviton, Docker on Apple silicon); code runs
  only on the VM thread that wrote it. Start-up there: 7.9 → 1.4 ms; a program with
  two workers: 21 → 6 ms (the previous engine: 10 ms).
- **PostgreSQL rows as result pages.** The client reads DataRow messages on
  `io/input` byte primitives, keeps each run of them raw, and turns a result portion
  into one `db/read.DbPage` in a single native call (`runtime/postgres.pg_page`), the
  same flat columns SQLite rows use; `Row.int`, `Row.text` and `Row.maybe` read the
  page. No per-column objects remain; `DbPage` now also holds boolean and decimal
  cells.
- **Network replies are noticed at once on the new core.** While waiting for a native
  reply the scheduler spun for 50 µs before sleeping, but sockets only make progress
  when the VM thread turns its (single-threaded) Tokio reactor, so every network wait
  cost the whole spin. The spin now polls the reactor without blocking. PostgreSQL
  point queries: 275 → 94 µs; inserts in a transaction: 130 → 48 µs. Against Python's
  `psycopg2` (5000 rows, Docker): scans take 1.3–1.7× as long (streaming scan 9.9 ms
  vs 6.0 ms, `Query.all` 7.6 ms vs 6.0 ms), point queries 1.4–1.6× (94 µs vs 69 µs),
  inserts are faster (24 ms vs 29 ms). A point query still takes two round trips
  (the portal is synced after its end); SQLite is unchanged.
- **`Query.all` is linear on the new core.** Collecting rows was quadratic in the
  PostgreSQL program (5000 rows: about 5 s; 20000 rows ran out of memory): every list
  cursor of the collected rows got a resource identity and a cleanup-region walk over
  the rest of the list. The core now knows a role value is one of the objects doing
  that role and a native handle holds no objects, so such data gets no identity:
  20000 rows take 46 ms. The general cost of region tracking over large data is
  [#9](https://github.com/shootnix/okkama/issues/9).
- **TLS and the terminal on the new core**, the last natives it lacked (all 621 ABI
  natives now run there). `runtime/tls_transport` is a rustls client over the core's
  TCP sockets, driven by its own reactor like the sockets (no I/O threads), with the
  origin provider's suites, limits, deferred chain verification (`authorize`) and
  failure texts; received records and plaintext are buffered as they arrive, so a
  cancelled read loses nothing. The terminal (`runtime/application`) runs the origin
  implementation: its promises are settled by origin worker threads that now wake the
  core, which pumps their deliveries (`sched::pump`, `sched::set_wake`). Checked
  against `sparrow` in Docker: TLS 1.3/1.2 with ALPN, 100 KB echo, two-way shutdown,
  refused host names and trust; a pty session with a line, raw mode and a resize.

### Removed

- `sparrow-engine` and `sparrow-proto`, the intermediate runtimes of the core rebuild,
  and the `sparrow-next` binary (the core is `sparrow`).
- **Objects can no longer do `Error`** (error handling phase 2, compiler part): errors
  are error enums. `object X does Error` is a semantic error that suggests
  `enum X is Error = (...)`; `throw` accepts error
  enum values, `Error` references and `NativeFailure`; catch types are error enums,
  `Error` and the system failures. This closes the last way to recover a concrete
  object from a role (a concrete `catch` of a value thrown through a role). The base
  `std` used by `io/stdout` is migrated (`error`: `InvalidArgument.because(reason)`
  and siblings; `io/stream`: `IncompleteWrite.after(written)`,
  `InvalidReadLimit.limit(value)`; `encoding/utf`: `InvalidUtf8.at(offset)` and
  siblings).
- **All of `std` uses error enums** (error handling phase 2, std part): about 550
  error objects became error enums, so every `std` module compiles again. Each keeps its
  type name; each constructor form is a case named after its meaning
  (`InvalidDuration.because(reason)`, `InvalidCsv.at(offset)`, `EndOfInput.reached`,
  `ChannelClosed.closed`, `TimedOut.elapsed`), payloads are the public field names.
  Code that read fields of a caught error uses patterns
  (`catch FileSystemFailure.detailed(code, _)`) or `match`. Diagnostics are the
  generated texts (`EndOfArchive.reached`, `InvalidCsv.at(1)`) instead of hand-written
  sentences. Fields that error payloads cannot hold travel as text: value objects
  (`Path`, `HostName`, `TemplatePath`, …) as `.string()`, causes typed as `Error`
  (`QueryCleanupFailure`, `RowValueFailure`) as `.text()`; `HttpPeerGoneBeforeResponse`
  keeps its typed cause in three cases (`reset`, `closed`, `truncated`), so the HTTP
  client still rethrows the original failure. Constructor checks that enums cannot
  run were dropped (`InvalidCsv` offset, `DbFailure` diagnostic length,
  `LogDeliveryFailure` destination count, `DatagramTooLarge` size).
  `http/internal/http2_frame.Http2FaultView` reads the fields of an `Http2Fault`.
  The 10-app benchmark, examples and conformance cases are migrated with it.
- `web/exception`: `Exception` is an error enum, `throw Exception.status(404, "user 7
  not found")`; the reply is the new `Problem(status, detail)` (RFC 9457 body,
  `.header(name, value)`), which endpoints return and into which the framework turns a
  thrown `Exception`. Headers go on a returned `Problem`. 422 details of request JSON
  stay readable sentences (`JSON value at /age must be number, found string`).
- Action directives and constructor projections, including `#projected`. Use
  `Object(arguments).method()`; stdlib and positive conformance examples are migrated.
  Method signatures state the complete result type, with no implicit unwrapping.

## [0.3.2] — 2026-11-01

### Changed

- **One JSON module.** `import "json"` replaces `json/value`, `json/parse`, `json/read`,
  `json/write` and `json/codec` (removed). A value is one immutable `Json` over a node of
  the runtime's tree, parsed (`ParsedJson(text).value()`) or built in code (`Json(42)`,
  `Json([JsonField("id", 42)])`, `Json(items: [...])`, `Json()` for null, also from
  `BigInt` and `float`). Reads are one native call each: `text/int/number/boolean(name)`,
  `get`, `list`, `at`, `items`, `names`, `values`, `fields`, `has`, `optional(name)` with
  fallbacks; errors keep their RFC 6901 pointer, which the runtime computes only when
  asked. `string()` and `pretty(n)` write the whole tree in one native call, with the
  same text as before. A codec is a plain object with `value(Json)` and `json(T)`; the
  `JsonValue`/`JsonWriting`/`JsonReading` roles, `JsonView`, `JsonOption`, `CompactJson`,
  `PrettyJson` and `JsonCodec` are gone. See `docs/std/json.md` and the migration table in
  `docs/std/stdlib-v4-migration.md`.
- `web/reply.Json` is now `JsonReply(status, value)`; `web/request.RequestJson` wraps a
  `Json` (`value()` gives it) and gains `RequestJsonOptional`. `logging`, `testing`,
  `template/json`, `template/sections`, `template/html` and `config/document` take `Json`.
  `template/json` lost `InvalidTemplateJsonSize` and `InvalidTemplateJsonProtocol`, which
  only a custom `print()` could trigger.
- Imports: a std module resolves imports from the std root first, so `"json"` from
  `std/template/json.ok` is `std/json`; a file never imports itself, so `app/json.ok`
  importing `"json"` finds `std/json`.

### Added

- Natives `runtime/json.json_text_node`, `json_number_node`, `json_boolean_node`,
  `json_null_node`, `json_array_node`, `json_object_node`, `json_node_write`,
  `json_node_pointer`, `json_member_values`, `json_node_items`, `json_member_text`,
  `json_member_spelling`, `json_member_boolean`, `json_member_int` (Rust and JVM).
- `benchmarks/json`: parse, walk and write six document shapes (many small records, a
  few wide objects, deep nesting, long strings, numbers, orders) in Okkama on Sparrow
  and on the JVM, Python `json` and Perl `JSON::PP`, with a checksum check.

### Fixed

- Internal compiler error "block is unreachable" when both branches of an `if` end in a
  self tail call (`value Next(...)` in a `#projected` action, a recursive binary search):
  tail-call elimination now drops the join block it leaves without entries. It broke
  `collections/ordering` searches (`config/document`, `http/media`, `locale`), so seven
  cookbook labs failed to compile.

### Performance

- Parsing and writing JSON run in the runtime: on the six shapes Okkama parses at
  0.7–1.7× Python's time and writes at 0.3–1.3×, and is 7–1000× faster than JSON::PP.
  Walking a parsed document in Okkama code takes 2–7× Python's time on Sparrow and
  0.5–3× on the JVM.
- **Sparrow registers are shared.** Sparrow codegen gives SSA values whose lifetimes
  never overlap one register of their type, instead of one register per value: a
  `match` over six cases no longer keeps the registers of all six, and every call
  clears and releases fewer callee registers (`Sums.plus` of the JSON benchmark: 102
  registers before). A result never shares a register with its own operands.
- Sparrow JIT: `enum.case` and `enum.cmp` (enum constants, `match` and `==` on enums)
  run in machine code, `str.len` calls a direct helper; before, all three went through
  the generic interpreter step.
- Sparrow JIT code is smaller: registers of types without references (`bool`, `float`,
  `byte`, `char`, enums) are written without a release check, and the callee's
  registers after a call are released by a helper loop instead of inline checks.
  `Sums.plus` of the JSON benchmark compiles to 3618 Cranelift instructions instead of
  9333 (5 ms instead of 13).
- IR: unwind handlers that only pass a failure on (left by elided regions) are skipped,
  so calls carry no empty handler path, and `undef` values nobody reads (left by
  elided regions and removed blocks) are dropped instead of being executed on every
  call.
- Sparrow JIT: native calls pass their arguments as plain copies of the registers,
  without reference counting (natives only read them); the callee's registers after a
  call are released by a bitmask of its registers of reference types.
- JSON nodes keep text, numbers and member names as shared strings: `text()`, member
  names and number spellings no longer copy the string on every read, and the parser
  builds each string once. Duplicate member names of small objects are found without
  a hash set. Parsing is 10–25% faster (records 110 → 90 ms, orders 21 → 16).
- Release checks in Sparrow JIT code follow the register type: an `int` register is
  released only when it holds a long integer, a register of a reference type only when
  it is not empty.
- IR: empty jump blocks are skipped (an empty block with arguments only behind a
  single jump, so a linear array builder is never passed on two branches), and
  Sparrow codegen prefers one register for a jump argument and the parameter it
  becomes, so most edge moves disappear.
- `Json.names()` reads all member names in one native call (new native
  `runtime/json.json_member_names`, Rust and JVM) instead of one call per name.
- Walking JSON documents on Sparrow is about twice as fast as in 0.3.1: records 867 →
  405 ms, deep 301 → 145, numbers 161 → 81, orders 189 → 85, wide 85 → 36, strings
  35 → 16 (Python 3.9: 299, 94, 69, 56, 28, 5).

## [0.3.1] — 2026-10-31

### Added

- **PGO builds**: `make pgo` (and `make install-pgo`) builds the release binaries with
  profile-guided optimization (`okkama-rs/tools/pgo/build.sh`): an instrumented build, a
  training run of `tools/pgo/json.ok`, the benchmarks on Sparrow and IR and std inline
  tests, then the final build with the merged profile. Needs `rustup component add
  llvm-tools`. Against the plain release build: JSON 14–18% faster, the benchmarks 6%.
  `-C target-cpu`, thin LTO and `panic = "abort"` were measured too: no gain from the first
  two, and abort (5% on JSON) would turn internal errors into aborts while the JIT still
  catches panics as a fallback, so the profile keeps unwinding.
- **`\u{H}` escapes** in string and char literals: one to six hexadecimal digits name a
  Unicode scalar value, `"\u{1b}[1m"`, `"\u{e9}"`, `'\u{0}'`. Surrogates, values above
  `10FFFF`, missing braces and empty or overlong digit runs are lexer errors (spec §4.2).
  Until now control characters other than `\n`, `\r`, `\t` could not be written in source.
- **`json/codec`**: `role JsonCodec<T>` maps a domain type to JSON and back with a
  hand-written object, both directions side by side. `JsonList(item:)` handles arrays and
  nests, `JsonDecoded(codec:, source:)` and `JsonEncoded(codec:, origin:)` read and write
  text, and `JsonTextCodec`, `JsonIntCodec`, `JsonBooleanCodec`, `JsonDecimalCodec` cover
  arrays of scalars (`docs/std/json-codec.md`, case `L09.json.codec`).
- **Optional JSON members**: `JsonView.optional(name)` is a `JsonOption`; each reader takes
  the value for an absent or `null` member — `view.optional("nick").text("anonymous")`,
  `view.optional("address").optional("city").text("")`, `view.optional("tags").items()`.
  A present member of another kind is still `WrongJsonKind`. `JsonView.required(name)` is
  `member(name)`. There is no null-like value: a `JsonOption` holds zero or one view.
- **Scalar JSON members**: `JsonMember("id", 42)`, `JsonMember("name", "Ada")`,
  `JsonMember("vip", true)` and `Decimal` values wrap themselves by declared type;
  `JsonNumber(value is int)`.
- `json/parse.JsonNode` reads its own kind directly: `text()`, `spelling()`, `number()`,
  `boolean()`, `size()`, `item(index)`, `name(index)`, `names()`, `members()`,
  `membership(name)`, `member(name)`, `optional(name)`; `ParsedJson.node()` is the root node.
- Natives `runtime/json.json_member_position` (a member's index by name, one call),
  `runtime/json.json_quote` (a JSON string literal), `runtime/json.json_number_text`
  (the JSON spelling of a decimal), `runtime/json.json_repeated_name` (the first repeated
  name) and a native JSON output (`json_output`, `json_output_raw`, `json_output_text`,
  `json_output_number`, `json_output_member`, `json_output_string`; each append answers
  the UTF-8 size so far), in the Rust and JVM providers.
- `role json/value.JsonReading`: how `JsonView` reaches a value. `json/read` reads any
  `JsonValue` through `print()`, `json/parse` reads parsed nodes directly
  (`ParsedJson.reading()`, `JsonNode.reading()`). Readings report `JsonKindMismatch`,
  `MissingJsonMember` and `JsonNumberMismatch` without a position; `JsonView` reports
  them as `WrongJsonKind`, `MissingJsonValue` and `InvalidJsonNumber` at its pointer,
  with the same texts as before.
- `JsonObject(members is @JsonMember)`: a builtin array of members is kept as it is,
  without the defensive copy that `Sequencing<JsonMember>` still gets.

### Changed

- **Functional suffixes are postfix forms.** `-> bindings { … }` belongs to its
  `map`/`filter`/`reduce` call like a further member call, so it can stand inside any
  operator, argument or condition: `total + xs.reduce(0) -> s, x { s + x; }`,
  `ready && xs.filter() -> x { x > 3; }.size() > 0`. Before, it was accepted only as a
  whole expression. The top level of a `for` source still gives the arrow to the loop.
- `json/read.JsonView` of a value declared as `ParsedJson` or `JsonNode` reads the parsed
  tree directly instead of through `print()`, with the same values, pointers and errors
  (case `L09.json.parsed-reading`). Views of other `JsonValue`s are unchanged.

### Performance

Measured on an Apple M2 Max; all runs and tables are in
[benchmarks/reports/2026-10-31-v0.3.1](benchmarks/reports/2026-10-31-v0.3.1/README.md).
Reading 10 000 orders (2.7 MB) into domain objects and writing them back (1.5 MB), Sparrow:
7.0 s → 0.33 s (0.27 s with PGO); Python 3.9 with its C `json` takes 55 ms. For 50 000
orders (14 MB) 1.15 s (0.98 s with PGO) against Python's 0.38 s; parsing is faster than
Python's. The general benchmarks are unchanged except `05-strings` (1.23×); Sparrow with
the JIT is 2.36× faster than Python 3.9 on average over benchmarks 01–10.

| Stage, 10 000 orders | 0.3.0 | 0.3.1 | 0.3.1 PGO | Python 3.9 |
|---|---|---|---|---|
| parse | 21 ms | 21 ms | 17 ms | 19 ms |
| decode into objects | 1 934 ms | 156 ms | 128 ms | 12 ms |
| build the `JsonValue` tree and write the text | 5 030 ms | 150 ms | 125 ms | 24 ms |

- **Empty ownership regions are elided.** When a program has any `destroy`, every scope
  opened a region, and every call created one and walked its result on exit. A new OIR
  pass drops regions that nothing is ever adopted, tracked or transferred into (they
  can own nothing); `region.enter` and `region.unwind` become `undef`, and readers of
  the unwind result read the failure it passed on. All engines run the same OIR. Regions
  that are opened but stay empty at run time close without walking `keep`.
- **Native calls** cost about 46 ns instead of 200 ns on Sparrow: the provider of a
  native is resolved and checked against the ABI once per function, not per call;
  signatures, module and function names come from tables indexed by the native's
  number instead of a SipHash map and symbol splitting; providers are routed by a
  table, `runtime/json` by `match` on the native itself; and JIT code calls a native
  with arguments copied from its registers into an array on the stack, instead of a
  generic interpreter step and a vector per call.
- `JsonView` reads of parsed documents find a member by one native call, check scalar
  kinds in the native read itself, and spell their RFC 6901 pointer only when a
  failure or `pointer()` needs it.
- `CompactJson.string()` writes the whole document into one native JSON output, one
  native call per scalar or member name, instead of a byte buffer, a regex split per
  string and several `Decimal` natives per number. The output is byte-for-byte that of
  0.3.0.
- `JsonView` reads a parsed node with one object per value read: the node reading holds
  the native node itself, and a view is its children's link to the pointer.
- `JsonObject` finds a repeated member name by one native call instead of sorting names.
- The general benchmarks (`benchmarks/run.py`) are unchanged within noise; `05-strings`
  is 1.16× faster.

### Known limitations

- An array literal of values of different concrete types that share a role
  (`[JsonText("a"), JsonNull()]`) gets its element type from a single matching overload;
  when a constructor is overloaded for both `@T` and `Sequencing<Role>`, it is typed
  `@any` and matches neither. `JsonArray` therefore keeps its one `Sequencing<JsonValue>`
  constructor.

### Removed

- `conformance/contracts/stdlib-inventory.json`: the snapshot of the std public surface had
  fallen far behind `std/` and nothing read it. `tools/inventory_std.py` now writes only
  the per-module coverage contracts in `contracts/stdlib.json`.

## [0.3.0] — 2026-10-31

### Added

- **`std.` paths**: a standard library declaration can be named in place, without an
  import — `std.io.stdout.Stdout()`, `let p is std.fs.path.Path = …`,
  `std.compare.Order.less`, `std.runtime.debug.data_dump(v)`. A path is a hidden
  namespace import of that module (looked up in the bundled std only), so the module is
  loaded once per program and shares its types with explicit imports. `std` is reserved
  (spec §5.2.1).
- **Editor completion for `std.` paths** (`okkama lsp`, VS Code): after `std.` and each
  folder, the folders and modules of the bundled std at that level; after a module, its
  objects, roles, enums and native actions with their doc comments; after an enum, its
  cases. Whole paths (`io.stdout`, and `io.stdout.Stdout` once a letter is typed) are
  offered from anywhere in std.
- **Editor support for named arguments** (`okkama lsp`, VS Code): at the start of an
  argument (right after `(` or `,`, or while typing a name) the parameters not yet named
  are offered as `name:`, first in the list. This happens only where a named call is
  allowed: not after a positional argument, and not for variadic actions, builtin actions
  or action values. Signature help highlights the parameter by its name in a named call
  (`obj.action(zip: 1, name: …)`) and picks the overload by the names already given.
- **Optional module version**: `module app;` is valid. A module without a version has
  ABI major `0`. Files of one multi-file module declare the same version, or none in
  every file.
- **Blade projects** (spec §5.2.2). A directory with `blade.toml` is a package root.
  Its `[dependencies]` are installed in `pkg/<name>/` of the project root and are
  imported as `import "pkg/web/router" take Router;`.
  - Only dependencies declared by the importing package are importable this way;
    transitive dependencies are not.
  - Module files of a project declare no version, because it is set in `blade.toml`.
  - The compiler, `okkama lsp` and the REPL find the project from the file, so a
    program resolves the same way from any working directory.
- **blade manages dependencies** (`docs/blade.md`).
  - Commands: `blade install`, `blade install SOURCE [VERSION]`, `blade remove NAME`
    and `blade update [NAME...]`.
  - Sources: git repositories (`vX.Y.Z` tags) and local package directories
    (`path = "…"`).
  - Requirements: `12.3.0` (compatible), `12.*`, `=12.3.1`, `*`. Each package has one
    version per project, and a conflict names every requirement and who made it.
  - `blade.lock` records the version, git commit and SHA-256 of every package. A moved
    tag or changed files stop `install` until `update`.
  - Installation is all or nothing.
  - Packages with natives, module versions in files, foreign module names or symbolic
    links are refused.
- **Editor completion for dependency imports** (`okkama lsp`, VS Code). The completion
  list opens right after the quote and after each `/`:
  - `import "` offers `pkg/`;
  - `pkg/` offers the direct dependencies of the file's package, with their version
    requirement and source, and marks the ones not installed yet;
  - `pkg/web/` offers the folders and modules of the installed package.

  `"` and `/` open completion only inside import paths, so division does not.
- `blade new` writes `blade.toml` (it was `NAME.toml`), creates `pkg/` (it was `lib/`)
  and starts `src/main.ok` with `module NAME;`.
- **Variadic and typed overloads** (spec §11.3, §11.8).
  - An action or constructor may have several variadic overloads that differ in
    parameter types (`kind(values is ...int)`, `kind(values is ...string)`), next to
    fixed ones.
  - A variadic tail counts as one parameter of the arity, and `...int` differs from
    `int`.
  - A call that fits a fixed signature never selects a variadic one: next to
    `total(values is ...int)`, `total(a is int)` takes `total(1)`, while `total()`
    and `total(1, 2)` take the tail.
  - Constructors of one arity are selected by parameter types, like actions:
    `Duration(ms is int)` and `Duration(text is string)`.
  - A variadic constructor no longer has to be the only constructor:
    `Path(origin is string)` and `Path(parts is ...string)` can coexist.
  - A generic object infers its arguments from each constructor that fits the call.
  - Two variadic signatures with the same types are a declaration error, because names
    cannot tell them apart in a positional call. So are two constructors with identical
    signatures.
- Benchmarks compare Sparrow and the JVM with the same programs in Python, Perl and Ruby
  (`benchmarks/NN-*/main.{py,pl,rb}`, engines `python`, `perl`, `ruby` in
  `benchmarks/run.py`). The outputs of all engines must agree. `run.py` also records the
  install sizes of every engine.
- `benchmarks/export.py` turns a run into a release report: every run as CSV, statistics
  per benchmark and engine (mean, median, quartiles, min, max, standard deviation, CV, CPU,
  memory, instructions, IPC), speed ratios per engine and `REPORT.md`. Reports are kept
  in `benchmarks/reports/<date>-v<version>/`; the first one is for 0.3.0.

### Changed

- **Import lookup is local-first.** The order is now:
  1. the importing file's directory;
  2. its package root;
  3. `OKKAMA_IMPORT_DIRS`;
  4. the standard library.

  Previously `OKKAMA_IMPORT_DIRS` came first. The standard library resolves its own
  imports only inside itself, so a file such as `io/stream.ok` in a program or in
  `OKKAMA_IMPORT_DIRS` no longer breaks std.

- Overloads of one name and arity declare the same result also when one of them is
  variadic. An object's action that breaks a role signature shows a variadic tail as
  `...int` in the error (it was `@int`).
- Conformance: `L03.variadic.overloaded` and `L04.variadic-ctor.with-other-constructor`
  now expect success under the new overload rules, and DEF-002 is withdrawn.
- A program that would load two different files declaring the same module path (for
  example a local module shadowing std next to a `std.` path to it) is now rejected with a
  clear error instead of an internal compiler error.

### Removed

- **`fs/path`: `JoinedPath` is removed.** Its join is the variadic constructor
  `Path(parts is ...string)`: write `Path("etc", "okkama", "config.toml")` in place of
  `value JoinedPath("etc", "okkama", "config.toml")`. The rules are unchanged: the current
  platform's style, `InvalidPath` for an empty part list or an empty part, and a later
  absolute part replaces the parts before it. `Path(text)` and `Path(text, style)` keep
  their meaning.

### Performance

Results, measured by `benchmarks/run.py --runs 10` on an Apple M2 Max; all runs and
tables are in [benchmarks/reports/2026-10-31-v0.3.0](benchmarks/reports/2026-10-31-v0.3.0/README.md):

- Sparrow with the JIT is on average (geometric mean over benchmarks 01–10) 2.31× faster
  than Python 3.9, 2.34× faster than Perl 5.42, 1.72× faster than Ruby 2.6 and 1.81×
  faster than the JVM build of Okkama.
- Sparrow without the JIT went from 0.33× to 0.76× of Python.
- Biggest gains: `06-collections` is 4.0× faster with the JIT and 6.6× without it;
  `05-strings` is 2.4× and 4.3× faster; the interpreter is 3.9× faster on `01-arith`
  and 2.3× on `02-calls`.
- Still slower than the best of the other languages: `05-strings` (Perl is 1.9× faster),
  `06-collections` (Perl 1.8×) and `09-closures` (the JVM build 1.4×).
- Start-up of `00-hello` grew from 5.1 to 5.7 ms.

Changes:

- **Sparrow interpreter** (`OKKAMA_JIT=off`, and cold code under the JIT): constants,
  copies, small-integer arithmetic and comparisons, jumps, direct calls and returns run
  in the dispatch loop itself instead of the generic instruction step. Frame registers
  come from a pool instead of a new allocation per call. Branch moves of up to eight
  values snapshot on the stack, and plain values are copied and overwritten without
  `clone`/`drop` calls.
- **Loop constants are computed once.** A new OIR pass moves the `const` instructions of
  blocks that lie on a loop into the entry block of the function. Constants of
  branches outside loops stay in place. All engines run the same OIR, so error
  locations stay exact.
- **Sparrow JIT.**
  - Value equality of two small integers (generic `==` over `T = int`) is compiled
    inline.
  - A role call finds its target in a per-slot table indexed by the object
    declaration, without a helper call.
  - `builder.push` no longer copies the builder.
- **Freeing objects, arrays, maps and closures** queues only the last references to
  containers. Scalars and shared references are released in place, and an object
  without such fields frees no queue.
- **`collections/frequencies`** counts without rebuilding the frequency list for every
  input value (it was O(n·d) list rebuilds, now O(n·d) comparisons only).
- **`strings/parse::ParsedInt`** parses natively (`runtime/number_text`:
  `int_parse_valid`, `int_parse`) with the same accepted spellings and errors.
- Release builds use fat LTO with one codegen unit: `okkama` shrinks from 18.2 to
  14.3 MiB, `sparrow` from 12.3 to 10.2 MiB, and JIT code is faster.

### Fixed

- Editor: a name typed where the checked text had a different name with the same first
  letters (`out.` in place of `obj.action(…)`) was resolved as the old one, so completion
  showed the wrong members and signature help went missing.
- Editor: signatures of variadic actions show the tail as `values is ...string` (it was
  `@string`).

### Known limitations

- A `for` used as a value (`for xs -> x { last … if …; fallback; }` as the result of an
  action) over an empty array passes the checker but fails at run time with "A void
  result cannot be used as a value"; guard it with `if xs.size() == 0`.
- Calling a closure from JIT code still copies its captures; `09-closures` is slower on
  Sparrow than on the JVM build.

## [0.2.0] — 2026-10-30

### Added

- **Location and call stack of an uncaught failure** on the IR and Sparrow engines
  (JIT included): the source line with the failing span underlined, then the stack from
  the failure to `main`. Recursion collapses (`(×500)`); a long stack shows its start and
  end. `sparrow` and `okkama exec`, which have no sources, print the stack in Sparrow
  terms (function, instruction number and text). `OKKAMA_TRACE=off` prints the failure alone.
- **`debug/dumper`** — a dump of any value after Perl's Data::Printer:
  `Dumper(value).type()`, `.kind()`, `.size()`, `.fields()`, `.all()`, `.max_depth(n)`,
  `.max_items(n)`. Shows object fields (private ones too), array elements and map pairs;
  IR, Sparrow and JVM print the same text. New `runtime/debug` natives: `data_kind`,
  `data_type`, `data_size`, `data_leaf`, `data_names`, `data_keys`, `data_items`.
- **`okkama repl`** — interactive checks on the IR interpreter: statements, expressions
  (their value is printed with `Dumper`), declarations and imports. Variables live on
  between inputs; side effects of earlier inputs are not replayed. Commands: `:vars`,
  `:decls`, `:reset`, `:help`, `:quit`.
- Example `examples/debug-dumper.ok`.

### Changed

- Spec §6.7 describes `debug/dumper`, which, unlike `Dump`, shows object fields.
- An OIR module carries debug info (`DebugInfo`): the source of every function.
- `tools/engines/parity.py` compares engines with `OKKAMA_TRACE=off`.

### Known limitations

- The JVM engine still reports a failure without its location and stack.
- In the REPL, closures and futures are not kept between inputs, top-level `await` is not
  supported, and there is no line history.

## 0.1.0

First release of the Rust implementation: the compiler (lexer, parser, analysis, OIR), the
IR, Sparrow (VM with JIT) and JVM engines, the standard library, LSP, `fmt`, `lint`, the
`blade` project manager, and packages for macOS and Linux.

[Unreleased]: https://github.com/shootnix/okkama/compare/v0.4.1...HEAD
[0.4.1]: https://github.com/shootnix/okkama/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/shootnix/okkama/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/shootnix/okkama/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/shootnix/okkama/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/shootnix/okkama/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/shootnix/okkama/releases/tag/v0.2.0
