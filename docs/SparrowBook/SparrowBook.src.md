# The Sparrow Assembly Book

Sparrow is the bytecode of Okkama: `okkama build --target sparrow` writes it,
Tomato, the current Sparrow runtime (`sparrow`, `okkama exec`), verifies it
and runs it as machine code. This book teaches its text form, Sparrow assembly
(`.spa`), well enough to read what the compiler produces and to write programs
by hand. It describes SPB version 12, the format of Okkama 0.4.1.

Every example lives in [examples/](examples/) and is assembled, checked and
run as part of writing this book; the result shown after each one is what
`sparrow run` printed.

Contents:

1. Tools
2. A first program
3. The shape of a file
4. Types
5. Functions and registers
6. Arithmetic and comparisons
7. Control flow and edge moves
8. Strings
9. Objects, successors and roles
10. Arrays, builders and maps
11. Calls, closures and natives
12. Failures: throw, handlers and catch
13. Resources: regions and destroy
14. Async functions and futures
15. Tests
16. What the verifier guarantees
17. How Tomato runs it
18. Instruction reference

## 1. Tools

```sh
sparrow --assemble prog.spa -o prog.spb    # text -> bytecode (verifies)
sparrow --disassemble prog.spb             # bytecode -> text
sparrow check prog.spb                     # verify, check intrinsic contracts
sparrow run prog.spb [ARGS...]             # verify and run
sparrow test prog.spb                      # run the tests of a test profile
okkama build prog.ok --target sparrow -o prog.spb   # compile Okkama to SPB
okkama exec prog.spb                       # run SPB inside okkama
```

Assembly and disassembly are exact inverses: `--disassemble` of a file and
`--assemble` of the text give the same bytes. The quickest way to learn an
idiom is therefore to write it in Okkama, build it, and read the disassembly.

`sparrow run` returns `main`'s integer result (0–255) as the exit status; an
uncaught failure is printed on stderr and the exit status is 1.

## 2. A first program

{{example:hello}}

```text
$ sparrow --assemble hello.spa -o hello.spb && sparrow run hello.spb
Hello, Sparrow!
```

A Sparrow program is a list of tables — types, then functions — and an
`entry`. `f0` has no body: it is a native, provided by the host and named by
its ABI symbol. `f1` is ordinary code: four numbered instructions over three
registers whose types are listed in `registers`. `call _, …` discards the
result. Comments start with `;`.

## 3. The shape of a file

```text
sparrow 14                      ; format version
module "name" "version"         ; the module's name and version (may be "")
type t0 …                       ; the type table, in order
enum enum0 "Name" ["a", "b"]    ; enums with their cases
role role0 "Name" { method "m" [t…] -> t… … }
object obj0 "Name" { field "f" t… … does role0 [f…] destroy f… }
handle "Name"                   ; opaque host types (handles)
cap cap0 "Name" obj0 { field "f" f… }
global g0 t3 f2                ; a module constant: type and init (#153)
function f0 "symbol" [async] [native "…"] [intrinsic "…" [obj…] [f…]] { … }
entry f3                        ; the program's main
tests { test "name" f5 … }      ; or a test profile
```

Every table is indexed by position, and the index is written out: `t3`,
`obj2`, `role0`, `f5`, `enum1`, `cap0`. The assembler checks that the indices
are consecutive, so a declaration cannot silently move. A type may refer only
to earlier types (`type t3 array t0`).

The disassembler prints declarations in this order: types, enums, roles,
objects, handles, caps, functions, then `entry` or `tests`; the assembler
accepts them in any order, each table numbered on its own.

## 4. Types

| Type | Meaning | Machine form |
|---|---|---|
| `int` | exact integer of any size | tagged word, or a pointer to a BigInt |
| `bool`, `char`, `byte` | | raw word |
| `float` | IEEE binary64 | float register |
| `rat` | exact fraction | pointer |
| `str` | immutable UTF-8 text | pointer |
| `object objN` | an object of a declared kind | pointer |
| `successor objN` | an object under construction by `obj.next` | pointer |
| `role roleN` | a reference through a role | pointer |
| `enum enumN` | a payload-less enum case | raw word |
| `variant variantN` | a value of an enum with payloads: an object of one of its cases | pointer |
| `array tN`, `map tK tV` | immutable collections | pointer |
| `builder tN`, `mbuilder tK tV` | linear builders of them | pointer |
| `fn [t…] -> t` | a closure | pointer |
| `future t`, `future void` | the result of an async call | pointer |
| `failure` | a failure in flight | pointer |
| `region` | an ownership region | pointer |
| `any`, `range`, `handle N`, `cap capN`, `never` | boxed value, integer range, host handle, cap, no value | |

Tomato takes every register's machine form from its declared type. That is
why types are checked so strictly (chapter 16): a register declared `int`
that held a string would be read as a number.

## 5. Functions and registers

```text
function f2 "Lab.total" {
    registers [t2, t3, t3, t0, t0, t0]   ; r0.. and their types
    params [r0, r1, r2]                  ; the parameters, in order
    result t0                            ; or `void`
    handler 0 3 @5 r4                    ; zero or more failure handlers
    0: …                                 ; numbered instructions
}
```

Registers are not SSA: a register may be written many times, as long as its
type stays the one declared. Every read must see a write on every path to it
(the verifier checks this). Instruction numbers start at 0 and must be
consecutive; control must not fall off the last instruction.

An object's method is a function whose first parameter is the object; the
compiler names them `Type.method`, constructors `Type.new#N`.

## 6. Arithmetic and comparisons

Integer operations never overflow: results that leave 64 bits become BigInt.

{{example:factorial}}

```text
$ sparrow run factorial.spb
265252859812191058636308480000000
```

`int.cmp le, r4, r0, r3` writes a `bool`; the comparisons are `eq ne lt le gt
ge`. Each numeric kind has its own instructions — `int.*`, `float.*`, `rat.*`,
`byte.*` — and `cvt` converts between kinds, to the type of its destination
(`int` to `str` above). `int.mod` is the remainder of truncated division and
fails on a zero divisor, like `rat.new` with a zero denominator.

## 7. Control flow and edge moves

`jmp @N`, `br rC, @THEN, @ELSE` and `ret rX` end a block. A jump target may
carry *edge moves*: `@5(r0 <- r1, r1 <- r5)` assigns registers as the edge is
taken, all at once — every source is read before any destination is written.

{{example:fibonacci}}

```text
$ sparrow run fibonacci.spb; echo $?
55
```

The back edge at 10 shifts `a, b` to `b, a + b` in one step, which needs no
temporary for `a`. Edge moves are how the compiler passes block parameters;
reading them is the key to reading compiled code.

## 8. Strings

`str.concat`, `str.len`, `str.slice`, `str.split`, `str.replace`,
`str.replace_first`, `str.trim`, `str.chomp`, `str.cmp` and `str.utf8` (to
`array byte`) work on immutable text; `str.fn` names the rest — searches,
edges, case, repetition and the characters of a string. Lengths and offsets count Unicode
scalar values. String constants use `\n`, `\t`, `\"`, `\\` and `\u{…}`.

## 9. Objects, successors and roles

{{example:shapes}}

```text
$ sparrow run shapes.spb; echo $?
17
```

- `obj.new r2, obj0, [r0]` makes an object from its fields in declaration
  order; `obj.get r1, r0, 0` reads field 0.
- Objects are immutable. `obj.next r4, r3, [1: r0]` makes a successor: a copy
  of `r3` with field 1 replaced.
- `does role0 [f0]` lists, for each method of the role, the function that
  implements it. `role.wrap` turns an object into a role reference;
  `call.role r7, r5, role0, 0, []` calls slot 0 of the role on it.
- `role.narrow` converts between roles and fails with its message when the
  object does not do the target role.
- An enum with payloads is a `variant variantN "Name" { case "c" objK … }`
  declaration: each case has its own object, whose fields are that case's
  payload and nothing else. `variant.new rD, variant0, 1, [r0]` makes case 1,
  `variant.tag` gives the active case number, `variant.get rD, rV, 1, 0`
  reads field 0 of case 1 and fails if another case is active, and
  `variant.of` turns a case object into a value of its variant. Tomato
  needs the cases of a variant to be consecutive objects: it reads the case
  number from the object header.

## 10. Arrays, builders and maps

{{example:collections}}

```text
$ sparrow run collections.spb; echo $?
88
```

Arrays are immutable; a `builder` collects elements and `builder.finish`
makes the array. A builder is linear: every `builder.push` returns the
builder to use next (here the same register). `arr.get` fails on a bad index,
while `arr.lookup` and `map.lookup` end the block and branch: the element is
written to their destination register on the *found* edge only. Maps
(`map.new r11, [r10: r12]`) keep keys in their natural order.

The operators `+` and `-` of Okkama collections compile to one instruction
per chain: `arr.concat rD, [rA, rB, rC]` allocates its result once for the sum
of the parts' lengths, `arr.diff rD, rA, [rB, rC]` keeps the elements of `rA`
equal to none of the others, and `map.merge` and `map.diff` do the same for
maps (a repeated key takes the rightmost value). `arr.replace rD, rA, rI, rV`
is `replaced(index, value)` (#250): a copy of `rA` whose element `rI` is
`rV`, failing on a bad index as `arr.get` does, and `arr.remove rD, rA, rI`
is `removed_at(index)` (#251), the copy without element `rI` (format
version 14).

## 11. Calls, closures and natives

- `call rD, fN, [args]` calls a function directly; `call _, …` drops the
  result.
- `fn.closure rD, fN, [captures]` makes a closure: the captures fill the
  first parameters of `fN`, and `call.fn rD, rClosure, [args]` supplies the
  rest.

{{example:closures}}

```text
$ sparrow run closures.spb; echo $?
42
```

- A *native* function has no body: `native "runtime/stdio@1::stdout_write/1"`
  names a host provider in the ABI table, and the verifier checks the
  function's signature against it. `sparrow --native-capabilities` lists all
  natives.
- An *intrinsic* function (`intrinsic "okkama.atomic.Atomic.get" [obj…]
  [f…]`) is provided by Tomato itself — atomics, events, channels, timeouts
  — with the objects and helper functions it works on. Tomato checks each
  intrinsic's contract when it loads the program.

## 12. Failures: throw, handlers and catch

{{example:errors}}

```text
$ sparrow run errors.spb; echo $?
7
```

- Only an object that does the builtin role `Error` can be thrown
  (`throw r3`); `panic rS` fails with a message, `trap` with a fixed code and
  message.
- `handler START END @TARGET rF` covers instructions `START..END-1`: a failure
  there jumps to `TARGET` with the failure in `rF` (type `failure`).
- `catch rF, object, N, rV, @MATCH, @OTHER` tests the failure: if it is an
  `objN` (with `role`, does `roleN`; with `variant`, is a case of
  `variantN`), the caught value goes to `rV` on
  the edge to `MATCH`; otherwise control goes to `OTHER`.
- `raise rF` sends a failure on unchanged. `fail.caused` and
  `fail.secondary` build failures that carry the one they replaced.

Note the `jmp @7` at 2: without it the successful path would fall into the
handler and read `r2`, which the verifier rejects:

```text
sparrow: Sparrow assembly line 0: Sparrow @main instruction 2: register r2 may be read before it is written
```

## 13. Resources: regions and destroy

{{example:resources}}

```text
$ sparrow run resources.spb
closing data.txt
```

An object kind with `destroy fN` owns a resource. A scope opens a region
(`region.enter`); a new resource is tracked (`region.track`) and owned
(`region.adopt`); `region.leave` runs the `destroy` of everything the region
owns, newest first. On the failure path `region.unwind` does the same and
returns the failure, with any failures of `destroy` attached as secondaries.
`region.transfer` moves ownership to another region, and `region.leave r, rK`
keeps what is reachable from `rK` (a value being returned).

## 14. Async functions and futures

{{example:tasks}}

```text
$ sparrow run tasks.spb; echo $?
11
```

Calling an `async` function returns a *cold* future without running it.
`await` runs it (or waits for it) and gives its value; `fut.spawn` starts it
now, so it runs while the caller goes on; `fut cancel|done|cancelled` cancels
or observes one. An async function is compiled as a resumable state machine;
`main` may be async.

## 15. Tests

{{example:testing}}

```text
$ sparrow test testing.spb
=== RUN   doubles small numbers
--- PASS: doubles small numbers
PASS
```

A file with `tests` instead of `entry` is a test profile: each test is a
function with no parameters, run on a fresh runtime; a failure fails it.
`okkama build -t` produces such files from Okkama tests.

## 16. What the verifier guarantees

Bytecode is verified when it is assembled, decoded or loaded, before anything
runs. A file the verifier accepts has:

- tables in bounds: types refer to earlier types, objects, roles, enums, caps
  and handles that exist; every role a kind claims to do has a witness with
  one function per method;
- registers, jump targets, handlers (sorted, disjoint) and table indices in
  bounds; no instruction falls off the end of the code;
- every register written before it is read, on every path, including the
  sources of edge moves and handler targets;
- types that fit: the operands and result of every instruction, the arity and
  argument and result types of every call, edge moves, `ret`, role slots, cap
  fields, `destroy`, the entry (no parameters, `int` result), tests, and
  natives against the ABI.

```text
sparrow: Sparrow assembly line 0: Sparrow @main instruction 2: r1 has type str, expected int
```

Decoding and verification are bounded in memory, and `sparrow check` also
checks the contracts of intrinsics. What remains open is listed in the trust
model (`docs/security-model.md`).

## 17. How Tomato runs it

Tomato compiles each function to machine code with Cranelift on its first
call; there is no interpreter. Registers live in machine registers or the
frame according to their types. Objects are allocated inline from size
classes; a precise, non-moving collector finds pointers through stack maps at
calls (safepoints). Failures travel as a status that every fallible
instruction checks, so handlers cost nothing when nothing fails. Budgets
(`--max-steps`, `--max-memory`, `--max-output`, `--timeout`) make machine
code poll at function entries and loop back-edges.

## 18. Instruction reference

`rD` is the destination, `_` an omitted one; `@T` a target with optional
edge moves; `[…]` a list.

{{reference}}
