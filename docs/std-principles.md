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
- **An object reveals itself through a cap** (the owner, 2026-11-11: «объект
  раскрывает себя с помощью публичного апи и только так, как он хочет»). What
  callers may see of an object is a cap of synchronous computed fields, named
  for the part it shows: `page::Head::status`, `date::Calendar::year`,
  `range::Bounds::start`. A view or an opening to read stays an action,
  because it is work: `file.text()`, `response.body()`, `page.reading()`.
- **What goes in a cap** (the owner, wave-2 review). A cap holds fields that
  are already computed, or computable statically and trivially: a
  concatenation, a multiplication. It does no runtime work and calls no
  other object. Preferably it exposes no field that is another object: a
  part made of objects stays an action. Parsed scalar parts are the ideal
  case: a query string, a URL's host, a date's year. Anything that needs
  work stays a method.
- **Constructors do no outside work** (the owner, wave-2 review). A
  constructor binds its fields. It may also use builtins, `confirm` its
  arguments, create its own runtime state and run an explicit parse of its
  own. It must not call methods of other objects or cause external effects.
  Work of that kind belongs to an action: `Number(text).int()`,
  `ParsedUrl(text).parts()`.
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
  do one thing (`Number(Utf8Arguments(ProcessArguments()).item(0)).int()`) is a
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
- **Defaults make the common case empty.** Every part with a sensible
  default has one, so the plain construction works out of the box and a
  special case names only what differs: `HttpClient()`,
  `HttpClient(endpoints: AllEndpoints())`, `Request(url: …)`,
  `MemoryCookieJar()`. The defaults are safe ones (public endpoints only, no
  redirects followed).
- **Shortcuts for the common verbs** are the one allowed second way: a
  shortcut builds the general form and nothing else. `client.get(url)` is
  `client.response(Request(url: url))`.
- **Named numbers are enums** (the owner: «200 надо заменить на enum»). A
  value with a registered name is a case (`Status.ok`, `Status.not_found`,
  `Method.put`); the rest go to `other(…)` (`Status.other(299)`). One object
  converts to and from the wire (`StatusCode(404).status()`,
  `StatusCode(Status.not_found).number()`, `MethodName`), and it refuses
  `other` of a named value, so every value has one spelling. Classes and
  phrases come from the converter: `StatusCode(status).class()`.
- **Iterate with `for`, not with closures.** A sequence is walked as
  `for source -> item`; a stream does `Batching<T>` (#465). No `each(fn)` and
  no `mapped(fn)`: an object is walked by its names
  (`for doc.names() -> name { doc.get(name) }`) and rebuilt with `map`.
  Lookups by name are indexed where a walk needs them (JSON objects from 16
  members).
- **Whole or in pieces, the same words everywhere.** A source that can be
  read whole and in pieces says so with the same actions: `content()`
  and `lines()` for a whole text file, `for file.text()` for its batches;
  `await page.text()`, `bytes()`, `json()` for a whole reply, `reading()` for
  its pieces.
- **One name, one meaning, one type.** The same action name never returns an
  iterator here and an array there. Actions that answer are nouns (`lines()`,
  `content()`); verbs are commands that answer nothing (`write`, `close`).
- **A computation names its answer** (the owner, 2026-11-11). An object that
  computes one thing has no `value()` or `result()`: its action is the noun
  of what it returns, `DaysBetween(left: a, right: b).count()`,
  `Crc32(bytes).checksum()`, and an adjective when it answers yes or no,
  `FileEncoding(name).supported()`. The object names what is asked about and
  the action names the answer, so neither repeats the other:
  `NativeFileMode(mode).code()`, not `FileModeCode(mode).code()`. A role
  names the noun for all who do it (`Checksumming` → `checksum()`). `value()`
  stays only where it is the domain's word: the value of a name/value pair
  (`Cookie`, `Header`, `QueryParameter`) and the content of a holder (`Cell`,
  `Atomic`). The review renames the rest module by module: 406 at 0.5.4
  start (`okkama-docs/std-review/inventory.py`, "answers").
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
  domain's error at the module boundary; no stringly-typed errors. The
  enum lives in a module below everything that throws it (`fs/error`), and
  its cases carry what the caller needs to act: the path, the pointer
  (`JsonError.missing(pointer)`), the refused value
  (`InvalidStatus.named(code)`).
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
- **Network and async tests talk to loopback** inside the test: a
  `TcpListening` on 127.0.0.1 port 0 stands in for the peer
  (`std/http/client.ok`). They run in Docker, like the gate.
- **Examples.** A module people use directly gets `examples/<module path>/`:
  one program per task with its `.expected` output, run by
  `examples/check.py` on IR and Sparrow (`examples/fs/file`,
  `examples/http/client`).

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
   (`json`), I/O (`fs/file`) and async network (the HTTP client). Done in
   0.5.4; this guide was refined from them with the owner, and their cards
   (`okkama-docs/std-review/cards/`) show the work in full.
3. **Waves bottom-up**, by import level, in place, all in 0.5.4
   (`okkama-docs/std-review/waves.md`: core; formats and system; security and
   network; platform; web; mail). There is no parallel "std2": two libraries
   mean double upkeep, and the second never catches up.
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

## 10. Checklist for a module

Each module in a wave goes through this list. Its card records what was
found and what changed; the public changes go to the owner as before/after
code before they are made.

1. **Pin it.** Characterization tests first, if it has none (§7).
2. **Formatted** with `okkama fmt -w` (§3).
3. **Surface.** Each public action is a command (verb), an answer (noun or
   adjective), or a view. Accessors become caps or move into the object as
   actions; records gain behaviour or fold away; withers and mutators go
   (§1).
4. **Answers are named.** No `value()` or `result()` outside name/value pairs
   and holders (§2); the `generic_answers` column of the inventory counts
   them.
5. **Constructors** take named arguments with defaults; one constructor with
   defaults instead of overloads; the plain construction works (§2).
6. **No matryoshkas.** The common task is one to three lines; the
   inventory's nesting column finds the rest (§2).
7. **Enums** for named numbers and for closed sets of choices (§2).
8. **Iteration** with `for` and `Batching`; no closure walkers (§2).
9. **Failures**: one domain enum, below its throwers, wrapping natives (§6).
10. **Layering**: no import cycles; helpers `hidden`; lower concepts below
    higher ones (§2).
11. **Efficient**: no quadratic growth; hot paths measured (§4).
12. **Documented**: a comment with the common use on each public object; an
    `examples/` folder for modules people call directly; the CHANGELOG
    entry with a before/after table for every public change (§3).

## 11. Tools

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
- Compatibility: std is pre-1.0. A changed API comes with a migration note in
  the CHANGELOG, not a deprecation period, unless the owner says otherwise.
