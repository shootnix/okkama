# Anonymous actions and action types

An anonymous action uses the same declaration syntax as a named action,
with the action name omitted. Action values always have an explicit result
declaration, including `is void` for effect-only actions.

| Form | Declaration |
| --- | --- |
| Named action | `check(user is User) is bool { user.active(); }` |
| Anonymous action | `(user is User) is bool { user.active(); }` |
| Action type | `(User) is bool` |
| Anonymous effect | `() is void { logger.write("done"); }` |
| Effect action type | `() is void` |

Types work in fields, local declarations, parameters, role signatures and
results. Nested results associate to the right: `() is (User) is bool` is a
zero-argument action returning an action of type `(User) is bool`.
Generic signatures such as `(T) is R` use the enclosing generic scope.

This complete example returns `42`:

```okkama
module example v1.0.0;

object User {
    has age is int;
    User(:age);
    active() is bool { age >= 18; }
    score() is int { age; }
    check(user is User) is bool { user.active(); }
}

object Processor {
    Processor();
    process(
        user is User,
        limit is int,
        predicate is (User) is bool,
        mapper is (User) is int,
        done is () is void
    ) is int {
        done();
        if predicate(user) { mapper(user) + limit; } else { 0; }
    }
}

main() is int {
    let minimum = 18;
    let make_predicate is () is (User) is bool = () is (User) is bool {
        (user is User) is bool { user.score() >= minimum; };
    };
    Processor().process(
        user: User(age: 21),
        limit: 21,
        predicate: make_predicate(),
        mapper: (user is User) is int { user.score(); },
        done: () is void { 99; }
    );
}
```

The same syntax handles multiple callable parameters alongside ordinary ones:

```okkama
process(
    input is Input,
    mapper is (Input) is Output,
    on_error is (Error) is void
) is Result {
    // Application-specific body.
}
```

The last block is a signature illustration; its domain types and body must be
provided by an application.

Anonymous parameters require types, unique names and no trailing comma.
Results and blocks are mandatory. `(value)` and `(a + b)` remain grouping;
`(value is int) is int { value; }` is a declaration. Anonymous values can be
stored, passed, returned, nested, put in typed arrays/maps and immediately
invoked. Calls of action values are positional and exact-arity; ordinary
member calls also accept anonymous values as named arguments.

`void` is a result marker, never an input type or ordinary value.
`(void) is int` is invalid. An `is void` body evaluates its final expression
and discards its value; `let result = callback();` is a semantic error when
`callback` returns void. Existing result validation, `defer` and `destroy`
rules apply without a separate anonymous-action mechanism.

Captures retain the existing lexical snapshot and object identity rules.
Later rebinding outside the action does not alter the captured binding.
Escaping captures participate in existing ownership reachability; unused
locals are not retained. Explicit signatures are exact and cannot be silently
retyped. Values cannot cross `any` or the native ABI.

Old `-> value { ... }` anonymous declarations, `(T) -> R` action types and
`receiver.apply -> value { ... }` trailing calls are rejected. Replace the
last form with `receiver.apply((value is T) is R { ... })`.
The arrows in `for values -> value { ... }` and dedicated functional
`values.map() -> value { ... }` / `filter()` / `reduce(...)` remain valid.

Anonymous async actions, anonymous variadic parameters and anonymous successor
results remain unsupported. Bound async actions retain `async (T) is R` and
must be awaited; `async (T) is void` awaits completion without a value. This migration adds no
variance, currying, additional call path or new closure runtime type.
