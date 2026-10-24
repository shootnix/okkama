# Joined string representation

VERIFIED: `strings/join` implements `Joined(separator is string, values is
Iterating<string>)` with `#projected string() is string`. It denotes a finite
snapshot of string values separated by exactly the supplied separator. Empty
input is empty text; one item has no separator; empty items remain items. Unicode,
control characters and empty separators are preserved. No conversion, escaping,
trimming or arbitrary-object formatting is implicit. Result matches builtin
@string.join(separator) for the same snapshot.

Construction observes the source once, freezes its strings, and borrows (never
closes) any external capability. This is the established snapshot-construction
compromise: string() is repeatable and performs no source traversal or I/O.
Source errors propagate unchanged. The optional full overload is
Joined(separator, values, maximum_items, maximum_length). Defaults are 10000 items
and 1048576 output Unicode scalars. Limits are nonnegative; zero permits empty
input. InvalidJoinLimits is raised before observation for negative budgets.
JoinLimitExceeded.kind() returns JoinLimit.items or JoinLimit.length; separators
count in length. Checks occur during traversal before retaining excess content.
At most maximum_items+1 item observations occur on an infinite source. No final
joined string is allocated until string(); source strings already allocated by
the caller are outside these budgets. No mutable fields or native entry points.


[Executable example](../../examples/std-v4/joined.ok) joins a generic persistent
List. RED/GREEN evidence.

From okkama-pl with Perl 5.42:

```sh
prove -l t/interpreter/263-std-v4-strings-join.t t/interpreter/250-std-v4-examples.t t/audit/14-library-syntax.t t/audit/16-stdlib-v4.t
```

Full regression: **386 files / 4177 tests, PASS**.
