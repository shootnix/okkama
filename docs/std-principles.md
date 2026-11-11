# Principles of the standard library

**Read this before any work on `std/`**, whether review, rewrite, a new
module or a fix. It records the owner's rules for the std review that starts
after 0.5.3 (2026-11-11), the plan, and the tools. Changing std's public API
still needs the owner's permission (`docs/LLM-GUIDE.md` §1); this document
says what a good proposal looks like, not that it is approved.

## Why the review

std grew to 537 modules (6.7 MB). Of those, 404 are code (3 MB), 64 are the
runtime substrate (`std/runtime`, natives) and 69 are generated CLDR data
(`std/locale/internal`). The owner's diagnosis:

- **Awkward to use.** The API is strange, and objects nest in deep
  "matryoshkas"; it is hard to read and hard to find one's way in.
- **Inefficient.** Odd structures and detours through who knows what.

The language will stay as it is for a while, so std is the next large piece
of work: every module looked at, simplified, made faster.

## 1. Objects are mechanisms, not records

The owner's rule, in his words: «мы пишем объектный код, поэтому избегать как
можно сильнее всяких аксессоров и мутаторов… делегировать объекту его
действия… не превращать объект просто в номинальную структуру данных. Это
должен быть действительно какой-то рабочий механизм — коробка, которая что-то
делает и даёт наружу результат.»

So:

- **Tell, don't ask.** An object is given work and returns a result. Code
  that reads three fields of an object and computes something from them
  belongs inside that object, as an action.
- **No accessors by default.** An action whose body only returns a field
  (`cents() is int { cents; }`) is an accessor. Add one only when there is no
  other way, and ask whether the caller should instead hand the object its
  task.
- **No mutators.** A successor or a "with" action that changes one field is
  a mutator in disguise. Prefer an action that does the operation the change
  was for (`money.plus(other)`, not `money.with_cents(…)`).
- **No records.** An object whose actions are mostly accessors is a data
  record with a constructor. Either give it behaviour, or fold it into the
  object that uses it.
- **Fields stay private.** Okkama already enforces this; do not route around
  it with accessors.
- **Helpers are `hidden`.** An action that only its own object calls is
  marked `hidden` (#365), one by one, so the public surface is exactly the
  unmarked actions. The same word keeps a helper object, role or enum inside
  its module.

Where data really is data — a parsed header value, a row — the object's
actions should still be the questions callers actually ask
(`header.allows("gzip")`), not a field per column.

## 2. Flat, short, one way

- **Short common path.** A common task takes one to three lines from the
  caller's side. If it takes more, the module is missing an entry point.
- **No matryoshkas.** Three or more constructors nested in one expression to
  do one thing (`ParsedInt(Utf8Arguments(ProcessArguments()).item(0))`) is a
  smell. Give the task one object that does it.
- **One way to do each thing**, and a small public surface. A second way to
  do the same task needs a reason in the module's comment.
- **Decorators earn their place.** Composition is the Okkama way (Chapter 14
  of the Bible), but a chain of wrappers that every caller has to assemble is
  a missing object.
- **Flat over nested** (the owner, 2026-11-11). An object that only forwards
  to another, or a view with two actions, folds into its neighbour; one
  constructor with defaults beats a family of overloads. Not at any price: an
  object with twenty-five actions is no better.
- **Constructors take named arguments** in std, in code and in docs:
  `File(path: Path("notes.txt"), mode: FileMode.appending)`. Positional
  arguments are for actions, and for a constructor of one value, where a
  name adds nothing: `Path("notes.txt")`.
- **Iterate with `for`, not with closures.** A sequence is walked as
  `for source -> item`; a stream does `Batching<T>` (#465). No `each(fn)`.
- **One name, one meaning, one type.** The same action name never returns an
  iterator here and an array there. Actions that answer are nouns (`lines()`,
  `content()`); verbs are commands that answer nothing (`write`, `close`).
- **No dependency cycles, and no workarounds for them.** A domain's errors
  live in a module below everything that throws them (`fs/error`); a lower
  concept never imports a higher one (`fs/entry` and `fs/lock` sit below
  `fs/file`).

## 3. Readable

- **Formatted.** Every module is formatted by `okkama fmt` (run
  `okkama fmt -w`). No one-line "minified" modules: 57 code modules still are.
- **Documented.** Each public object has a comment with a short example of
  the common use, which is checked to compile.
- **Named for what it does.** Names say what an object does, and actions say
  what they do to the world or return; no `Manager`, `Helper` or `Util`.

## 4. Efficient

- **No quadratic growth.** Build with builders, `reduce` accumulators or the
  persistent collections, never by copying in a loop (no O(n²) growth).
- **Measured.** Hot paths have a micro benchmark (`okkama-docs/benchmarks/micro/std`).
  Changes are judged on the release build (fat LTO), not the gate profile:
  the gate profile exaggerates and produces code-layout artifacts.
- **Natives only where measured.** A native only for a measured hot path or
  for what Okkama cannot do. Crypto, TLS, X.509 and PEM stay native.
- **No fixed costs on every call.** Parse a table, a pattern or a template
  once, not on every call.

## 5. Explicit authority, no global state

As in Chapter 21 of the Bible:
- what a module may touch arrives as a value: `DirectoryLocating`,
  `Environment`, a clock;
- no process-wide mutable state;
- no `chdir`-like operations;
- tests can pass fakes and manual clocks.

## 6. Failures and async

- **Typed failures.** One error enum per domain (`enum X is Error = (…)`)
  with cases that say what went wrong; `NativeFailure` is wrapped into the
  domain's error at the module boundary; no stringly-typed errors.
- **Async only where something waits** (I/O, a timer, another task).
  Cleanup guards stay `defer await guard.close()` until #329 decides
  otherwise.

## 7. Tests

- **Characterization tests first.** Before changing a module, add tests that
  pin its current behaviour.
- **Every public object** has at least one inline `test`; 180 code modules
  have none today.
- **Gate.** The std tests run in the gate on the IR engine and Sparrow
  (`tools/gate/known-failures.txt` lists the known failures).

## 8. Scope: foundation, domains, packages

- **Foundation.** The base modules everything stands on:
  - `encoding`, `io`, `collections`, `compare`, `error`;
  - `async`, `numbers`, `strings`, `time`;
  - `fs`, `os`, `cli`, `url`, `json`, `regex`.

  They are mostly in good shape (almost no accessors) and are depended on most
  (`encoding/utf` is imported by 114 modules, `collections/sequence` by 110,
  `io/stream` by 100).
- **Domains.** Large and in the worst shape:
  - `http`: 102 modules, 543 accessors;
  - `mail`: 236 accessors;
  - `mime`, `i18n`, `cli/spec`, `testing`.
- **Nothing leaves std.** The owner, 2026-11-11, at the start of the review:
  «сразу договариваемся, что мы ничего выбрасывать не будем, это все наша
  стандартная библиотека». No module is deleted or moved into a Blade
  package; the long tail (`mail`, `plist`, `http/xmlrpc`, `archive`, `xml`,
  `html`, `i18n`, `template`, `diff`, `statistics`, `calendar`) is improved
  in place like the rest. Two modules that do the same thing are merged,
  and the capability of each is kept.

## 9. How the work goes

1. **Mechanics.** `okkama fmt -w` over the one-line modules, gated; behaviour
   unchanged.
2. **Exemplars.** Three modules rewritten to these principles: a data module
   (`json`), I/O (`fs/file`) and async network (the HTTP client). This guide
   is refined from them, with the owner.
3. **Waves bottom-up**, by fan-in, one area per release, in place. There is
   no parallel "std2": two libraries mean double upkeep, and the second never
   catches up.
4. **One card per module, kept in its area's epic.** The card holds:
   - purpose;
   - public surface, accessors and records, nesting depth, the common use in
     lines;
   - tests, benchmarks, natives, fan-in;
   - the decision: keep / simplify / rewrite (merge into another module only
    when both do the same thing; nothing is deleted, §8).
5. **Performance**, as its own profile-driven track, on hot paths only.
6. **Tickets.** An epic per area with a checklist, not a ticket per module.
   The existing "stdlib new api" milestone (132 open issues) is triaged into
   the epics.

## 10. Tools

- **Inventory.** `python3 okkama-docs/std-review/inventory.py` parses every
  module with `okkama ast --format json` and writes `inventory.csv` (one row
  per module) and `summary.md`. It takes 3 s. Per module:
  - size, one-line lines;
  - objects, roles, natives, tests;
  - public actions, accessors, withers, record-like objects;
  - constructor nesting depth;
  - fan-in and fan-out.

  Rerun it after each wave and compare.
- **Baseline (0.5.1, code modules only):**
  - 2381 objects, 6076 public actions;
  - 1689 accessors (27%), 273 record-like objects;
  - 201 expressions nesting three or more constructors;
  - 57 one-line modules, 180 modules without tests.
- **Benchmarks.** `okkama-docs/benchmarks/micro` (std programs under
  `micro/std`), the ladder (`run.py`), `micro/coverage.py` for std modules
  without a benchmark.

## Open decisions (the owner's)

- Which modules form the foundation (none leave std, §8).
- This guide's rules, once the exemplars have tested them.
- Compatibility: std is pre-1.0. A changed API comes with a migration note in
  the CHANGELOG, not a deprecation period, unless the owner says otherwise.
