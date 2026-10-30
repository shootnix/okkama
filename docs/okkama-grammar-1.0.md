# Okkama Grammar 1.0 - EBNF

This grammar is intended as a parser construction target for the first Okkama implementation.
It describes syntactic structure. Semantic checks such as constructor completeness, role conformance,
comparability, and successor path analysis are performed after parsing.

Collection type inference does not add syntax: nonempty homogeneous literals
retain their exact recursive `@T`/`%K:V` identity; empty and heterogeneous literals
remain untagged without an explicit type boundary. No widening or implicit `any`
is inferred. See the specification's builtin value behavior section.

`BINDING_NAME`, `IDENT`, and `TYPE_NAME` denote lexer identifier tokens, not keyword tokens with
a matching character shape. `CONTEXT_IDENT` denotes a lowercase word-shaped
token in an unambiguous namespace position; it includes active keyword tokens
but excludes removed/deferred reserved words. It is used only for module path
segments, source action labels, and member spellings after `.`. Keywords remain
invalid as variables, fields, parameters, constructors, types, import aliases,
native action names, enum names, and enum cases.

Every action-name position additionally rejects the exact spelling `with` and
every identifier whose spelling begins with `with_`. This lexical predicate
applies to declarations and member spellings; it does not reject `withdrawal`.
Within an object body the otherwise overlapping prefix is resolved by one
token of lookahead: `has(` starts an action whose contextual label is `has`,
while `has IDENT` starts `has_decl` state syntax. The same action label is
valid in role signatures and fake bodies.

Import visibility is semantic rather than syntactic: a module's importable
surface contains declarations written directly in that module, never
declarations brought in by its own imports. A plain import projects that owned
surface only through its basename namespace. `take` projects exactly the
requested owned names as bare bindings, while `as` replaces the basename with
an explicit namespace. The runtime may still load a transitive dependency
closure, but declaration bodies resolve names in their defining module scope
and do not publish that closure to consumers.

```ebnf
program             = module_decl import_decl* declaration* EOF ;

module_decl         = "module" module_name version? ";" ;
module_name         = CONTEXT_IDENT ("/" CONTEXT_IDENT)* ;
version             = "v" INT "." INT "." INT ;

import_decl         = "import" STRING ("as" IDENT)? take_clause? ";" ;
take_clause         = "take" ident_list ;
ident_list          = IDENT ("," IDENT)* ;

declaration         = external_cap_decl
                    | object_decl
                    | role_decl
                    | enum_decl
                    | test_block
                    | native_object_decl
                    | native_action_decl
                    | action_decl
                    ;

native_object_decl  = "native" "object" TYPE_NAME ";" ;
native_action_decl  = "native" async_mark? IDENT "(" param_list? ")" result_spec ";" ;

object_decl         = "object" TYPE_NAME generic_params? does_clause? object_body ;
generic_params      = "<" TYPE_NAME ("," TYPE_NAME)* ","? ">" ;
does_clause         = "does" type_ref ("," type_ref)* ;
object_body         = "{" object_member* "}" ;

object_member       = has_decl
                    | cap_decl
                    | constructor_decl
                    | destroy_block
                    | test_block
                    | private_section
                    | projection_action_decl
                    | action_decl
                    ;

private_section     = "private" ":" (projection_action_decl | action_decl)* ;

has_decl            = "has" attr_name_list has_type? has_default? ";" ;
attr_name_list      = IDENT ("," IDENT)* ;
has_type            = "is" type_ref ;
has_default         = "=" expr ;

cap_decl            = "cap" BINDING_NAME "{" computed_field* "}" ;
external_cap_decl   = "cap" TYPE_NAME "::" BINDING_NAME "{" computed_field* "}" ;
owner_field_read    = TYPE_NAME "::" BINDING_NAME ; (* external cap body only *)
computed_field      = BINDING_NAME "is" type_ref block ;

constructor_decl    = TYPE_NAME "(" param_list? ")" block
                    | TYPE_NAME "(" field_param_list? ")" (";" | block) ;
field_param_list    = field_param ("," field_param)* trailing_comma? ;
field_param         = ":" IDENT ;

destroy_block       = "destroy" block ;


role_decl           = "role" TYPE_NAME generic_params? role_body ;
role_body           = "{" role_member* "}" ;
role_member         = role_action_decl | fake_block | test_block ;
role_action_decl    = async_mark? CONTEXT_IDENT "(" param_list? ")" result_spec ";" ;
fake_block          = "fake" object_body ;

enum_decl           = "enum" IDENT "=" "(" enum_cases? ")" ";" ;
enum_cases          = IDENT ("," IDENT)* ;

test_block          = "test" STRING block ;

action_decl         = async_mark? CONTEXT_IDENT "(" param_list? ")" result_spec block ;
projection_action_decl = "#" "projected" ("(" IDENT ")")? action_decl ;
async_mark          = "async" ;
param_list          = param ("," param)* trailing_comma? ;
param               = IDENT "is" variadic_mark? type_ref ;
variadic_mark       = "..." ;
result_spec         = "is" type_ref | "is" successor_type | "is" "void" ;
successor_type      = "^" type_ref ;

type_ref            = collection_type | action_type | named_type ;
collection_type     = "@" type_ref | "%" type_ref ":" type_ref ;
action_type         = "(" action_type_params? ")" "is" (type_ref | "void")
                    | "async" "(" action_type_params? ")" "is" type_ref ;
action_type_params  = type_ref ("," type_ref)* ;
named_type          = qualified_type generic_args? ("::" BINDING_NAME)? ;
qualified_type      = TYPE_NAME | IDENT "." TYPE_NAME | IDENT "." IDENT ;
generic_args        = "<" type_ref ("," type_ref)* ">" ;

block               = "{" statement* "}" ;

statement           = let_stmt
                    | defer_stmt
                    | return_stmt
                    | confirm_stmt
                    | throw_stmt
                    | try_stmt
                    | successor_assign
                    | block_stmt
                    | field_bind_stmt
                    | if_stmt
                    | match_stmt
                    | for_stmt
                    | loop_stmt
                    | next_stmt
                    | last_stmt
                    | expr_stmt
                    ;

let_stmt            = "let" BINDING_NAME explicit_type? "=" expr ";" ;
explicit_type       = "is" type_ref ;

defer_stmt          = "defer" expr ";" ;
return_stmt         = "return" expr? ";" ;

confirm_stmt        = "confirm" expr confirm_else? ";" ;
confirm_else        = "else" ("throw" | "panic") expr ;

throw_stmt          = "throw" expr? ";" ;

try_stmt            = "try" block catch_clause+ ;
catch_clause        = "catch" BINDING_NAME "is" type_ref block ;

successor_assign    = "^" IDENT "=" expr ";" ;

block_stmt          = block ;

field_bind_stmt     = [ "self" "." ] IDENT "=" expr ";" ;
                    /* valid only in constructor bodies */

if_stmt             = "if" expr block ("else" (if_stmt | block))? ;

match_stmt          = "match" expr "{" match_case* match_else? "}" ;
match_case          = "case" case_values block ;
case_values         = expr ("," expr)* ;
match_else          = "else" block ;

for_stmt            = "for" for_source for_pattern block ;
for_source          = assignment_free_expr ;
for_pattern         = "->" pattern ;
pattern             = BINDING_NAME ("," BINDING_NAME)* | BINDING_NAME "[" INT "]" ;

loop_stmt           = loop_expr ";"? ;
next_stmt           = "next" postfix_if? ";" ;
last_stmt           = "last" ("if" expr | expr postfix_if?)? ";" ;
postfix_if          = "if" expr ;

expr_stmt           = expr postfix_if? ";" ;

expr                = assignment_free_expr arrow_suffix* ;
(* arrow_suffix applies only to an eligible functional member call, e.g. map(). *)
arrow_suffix        = "->" arrow_bindings? block postfix_op* ;
arrow_bindings      = IDENT ("," IDENT)* ;
assignment_free_expr= logic_or ;
logic_or            = logic_and ("||" logic_and)* ;
logic_and           = stream_write ("&&" stream_write)* ;
stream_write        = equality ("<-" equality)* ;
equality            = range_expr (("==" | "!=") range_expr)* ;
range_expr          = comparison ((".." | "..=") comparison)? ;
comparison          = bitwise_or (("<" | "<=" | ">" | ">=") bitwise_or)* ;
bitwise_or          = bitwise_xor ("|" bitwise_xor)* ;
bitwise_xor         = bitwise_and ("^" bitwise_and)* ;
bitwise_and         = shift_expr ("&" shift_expr)* ;
shift_expr          = term (("<<" | ">>" | ">>>") term)* ;
term                = factor (("+" | "-") factor)* ;
factor              = unary (("*" | "/" | "%") unary)* ;
unary               = ("!" | "-" | "~") unary | named_constructor_projection | await_expr | spawn_expr ;
await_expr          = "await" unary | postfix ;
spawn_expr          = "spawn" call_expr ;
call_expr           = postfix ;  // semantic form: direct Worker(...).action(...)
postfix             = primary postfix_op* ;
postfix_op          = call_args | member_access | cap_select | index_access | unpack_op ;
cap_select          = "::" BINDING_NAME ;
unpack_op           = "..." ;
member_access       = "." CONTEXT_IDENT ;
index_access        = "[" expr "]" lookup_else? ;
lookup_else         = "else" expr ;
call_args           = "(" arg_list? ")" ;
named_constructor_projection = IDENT (TYPE_NAME | IDENT "." TYPE_NAME) call_args ;
arg_list            = arg ("," arg)* trailing_comma? ;
arg                 = IDENT ":" expr | expr ;

primary             = literal
                    | IDENT
                    | TYPE_NAME
                    | qualified_name
                    | "self"
                    | "panic" expr
                    | anonymous_action
                    | "(" expr ")"
                    | bracket_lit
                    | array_cast
                    | try_expr
                    | if_expr
                    | match_expr
                    | loop_expr
                    ;

try_expr            = "try" block catch_clause+ ;
if_expr             = "if" expr block "else" block ;
match_expr          = match_stmt ;
loop_expr           = "loop" block ;
anonymous_action    = "(" anonymous_action_params? ")" "is" (type_ref | "void") block ;
anonymous_action_params = anonymous_action_param ("," anonymous_action_param)* ;
anonymous_action_param = IDENT "is" type_ref ;

qualified_name      = IDENT "." IDENT | IDENT "." TYPE_NAME | TYPE_NAME "." IDENT ;

array_cast          = "@" cast_target call_args ;
cast_target         = "int" | "float" | "byte" | "string" ;
// Array casts accept positional arguments and one-level expression unpack in any slot.

bracket_lit         = array_lit | map_lit ;
array_lit           = "[" (expr ("," expr)* trailing_comma?)? "]" ;
map_lit             = "[" (":" | map_entry ("," map_entry)* trailing_comma?) "]" ;
map_entry           = expr ":" expr ;

literal             = INT | FLOAT | DECIMAL | STRING | RAW_STRING | CHAR
                    | "true" | "false"
                    ;
trailing_comma      = "," ;

BINDING_NAME        = /[a-zA-Z_][a-zA-Z0-9_]*/ ;
IDENT               = /[a-z_][a-zA-Z0-9_]*/ ;
TYPE_NAME           = /[A-Z][a-zA-Z0-9_]*/ ;
INT                 = /[0-9]+/ | /0[bB][01]+(?:_[01]+)*/ | /0[xX][0-9a-fA-F]+(?:_[0-9a-fA-F]+)*/ ;
FLOAT               = /[0-9]+\.[0-9]+/ ;
DECIMAL             = implementation_defined_decimal_token ;
STRING              = double_quoted_string ;
RAW_STRING          = "```" raw_string_character* "```" ;
CHAR                = single_quoted_char ;
ELLIPSIS            = "..." ;
```

`raw_string_character` is any source character except the beginning of the
next exact triple-backtick delimiter. Raw strings have no escapes,
interpolation, dedenting or delimiter modifiers. Their contents may span
lines and produce the same runtime `string` value category as `STRING`.

## Context-Sensitive Restrictions

The parser may accept some forms that later semantic analysis rejects:

```text
field_bind_stmt binds a constructor field when its IDENT names one of the
object's own fields and it is currently constructing that object; otherwise
it rebinds an existing local variable, legal only when that local's declared
type is a concrete object declaring at least one actual "^"-marked successor
action returning the type itself or a role it implements (a plain, non-"^"
same-type-returning action does not qualify, nor does a role-typed declared
local) — see docs/todo/local-rebinding-22072026.md. The optional receiver is
literally `self`, never an arbitrary expression; `other.field = expr` is not
assignment syntax;
successor_assign is valid only inside is ^Type actions;
destroy is valid only directly inside object bodies and role fake bodies;
destroy blocks run automatically when a local object leaves its block/environment scope;
destroy blocks may read self and fields, but may not contain successor assignment;
fake is valid only directly inside role bodies;
production parsing consumes balanced test/fake bodies but omits their AST nodes;
test blocks and RoleName.fake(...) execute only under explicit `okkama -t`;
RoleName.fake(...) is valid only inside test code;
variadic parameters are valid only on object actions, role action signatures, fake actions, and full constructors;
a variadic parameter must be unique, final, explicitly typed, and have no default value;
a variadic full constructor is positional-call-only and rejects named calls;
an object that declares a variadic full constructor cannot declare any other constructor;
generic declaration parameters are valid on objects and roles;
generic parameters must be uppercase, ordered, unique, and may not shadow their declaration name;
applied object and role types must provide exactly the declaration's generic arity;
generic role fakes obtain their exact specialization from a declared target type;
every ordinary source object must declare at least one constructor;
constructors are never generated from has declarations;
full constructor calls require an exact explicit arity match;
constructor field shorthand is named-only and selected by allowed/required field compatibility;
constructor field shorthand entries must name explicitly typed fields on the same object;
constructor selection never falls back to all has fields or their order;
named constructor-projection syntax is valid only when a lowercase prefix is followed immediately by an object-name constructor call; it is equivalent to constructing the object and calling the action registered for that prefix;
`#projected` registers the action name as its prefix and `#projected(alias)` registers the lowercase alias; an object may register multiple projections, but each effective prefix must be unique within that object;
a projected action must be public, synchronous, zero-argument, explicitly typed, non-successor, and return a type different from the object type;
projection prefixes are resolved from the statically resolved constructor target during semantic analysis; they are not module bindings and do not apply to arbitrary expressions;
the directive marks one declaration and selects its action name; unmarked fixed overloads of that name remain available, and projection invokes the ordinary zero-argument overload;
single `&` projection declaration/expression syntax is removed; `&&` remains boolean conjunction;
actions with a variadic parameter do not participate in action overloading;
fixed action overloads are selected by arity, named-parameter shape, runtime
argument compatibility, and specificity; declaration order is never a
tie-breaker;
calls to variadic actions use positional arguments only;
postfix unpack accepts an array-valued expression or a variadic binding; its precedence remains above stream writing and its operand is evaluated once;
unpack is valid as a complete let value, complete constructor field-binding value, for source, the final positional argument of a variadic action/full constructor, any positional argument of an array cast, or the right source of a stream write;
ordinary call-site forwarding supplies only the target variadic tail, so every fixed-prefix argument must be explicit; one final positional spread is allowed; array casts additionally allow multiple spreads in any positional slots; named and array/map-literal spread are invalid;
functional suffix is valid only after a member call named map, filter, or reduce;
map/filter functional calls take no arguments and reduce takes one positional initial value;
functional receivers may be arrays, ranges, typed maps, or objects nominally
implementing the standard collections/iteration.Iterable<T> role; this adds no grammar production;
the top-level arrow in a for source belongs to the for pattern; group a functional for source;
for always requires both an array/range/Iterable source and an explicit binding;
custom object iteration is nominal through collections/iteration.Iterable<T>
and its immutable Cursor<T>; it does not add grammar productions;
loop is the source-less infinite-loop form and is also a value-producing expression;
in last if condition, if introduces the condition; group an if-expression used as a last payload;
isa, super, abstract, protected, null, none, nil, and with are reserved errors;
with_* identifiers are rejected specifically in every action declaration and
member-call name position;
action declarations require parentheses;
constructors are named exactly like their enclosing object;
a module entry point has exactly main() is int or async main() is int;
its explicit or implicit result is an int in 0..255 and becomes the process exit status;
object methods named main follow ordinary action rules; libraries may omit main.
native action declarations are valid only at module scope;
native action declarations are typed, bodyless, and require an explicit result;
native action declarations cannot be named main or byte;
native object declarations are module-scoped, nominal, bodyless, non-generic, and end with a semicolon;
async, variadic parameters, parameter defaults, and constructors are not valid native forms;
the executable native ABI supports fixed positional typed parameters and results through the portable logical value adapter;
native calls are positional and resolve a mandatory provider by declaration owner module, ABI major, name, and arity;
missing or mismatched native providers are runtime errors and never fall back to an Okkama body.
```

### Explicit action results

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

Constructors, `destroy`, and root test blocks retain their own grammar.
The `result_spec` alternatives apply equally to synchronous and async actions.

## Explicit return

`return expr;` exits the current action with exactly one value. The final
expression remains an implicit result. `return;` is restricted to void and
successor actions; successors finalize their staged fields and reject explicit
replacement values. Comma lists and variadic expansion are not return syntax.
Return is a statement, not an expression; it may occur inside expression blocks.
It is forbidden directly in constructors, destroy/test bodies, or module scope.
Nested blocks, loops and functional iteration propagate return; a named or
anonymous action establishes the boundary. Registered lexical defers and owned
destruction run before the result escapes. Cleanup failure cancels return.
See the contract for lifetime and async rules.

Object Capabilities 0.0.1: `cap` is contextual inside objects or at module scope.
External declarations require a locally owned object and Owner::Name; private
field reads in their bodies require Owner::field. Both forms share identity. `::` is resolved
by the available operand type, not identifier casing. Field bodies use ordinary
synchronous blocks without self or owner writes. Nominal boundaries, result
checks, lifetime and service-operation restrictions are defined in
[the caps contract](object-caps-0.0.1-ru.md). Object views have been removed.
