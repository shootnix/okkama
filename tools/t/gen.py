#!/usr/bin/env python3
"""Generated t/ files (#245): combinations, not repetitions.

    tools/t/gen.py [--out t/gen] [--family NAME ...] [--list]

Every family crosses a few dimensions whose values are equivalence classes —
one representative per class (zero, one, minus one, a small value, the edge
of a machine word, past it, a big integer; an empty string, an astral
character; a statement, an expression, a closure, an async action) — so no
two checks of a file differ only in an incidental value. Expected results
come from an oracle written here in Python with Okkama's semantics
(truncating division and remainder, exact fractions, 64-bit shifts, shortest
float text), never from running Okkama.

Each family writes t/gen/<family>/NN.ok: one program, checks grouped forty
to an action (the JVM limits the size of a method), TAP through t/support/
tap.ok. The output is deterministic; t/gen is not committed — the runner
generates it.
"""
import argparse
import fractions
import os
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(ROOT, "t", "gen")

CHUNK = 40
PER_FILE = 600


# ----- literals and the oracle's text of values ---------------------------------

def lit_int(n):
    # A unary minus, not parentheses: a map value cannot start with `(` (#263).
    return str(n)


def lit_float(x):
    return repr(x)


def receiver(literal):
    """A literal as the receiver of an action call: `(-5).abs()`."""
    return f"({literal})" if literal.startswith("-") else literal


def lit_rat(q):
    return f"rat({q.numerator}, {q.denominator})"


def escape(text, quote):
    out = []
    for c in text:
        code = ord(c)
        if c == "\\":
            out.append("\\\\")
        elif c == quote:
            out.append("\\" + quote)
        elif c == "\n":
            out.append("\\n")
        elif c == "\t":
            out.append("\\t")
        elif c == "\r":
            out.append("\\r")
        elif code < 0x20 or code == 0x7F:
            out.append("\\u{%x}" % code)
        else:
            out.append(c)
    return "".join(out)


def lit_str(s):
    return '"' + escape(s, '"') + '"'


def lit_char(c):
    return "'" + escape(c, "'") + "'"


def text_float(x):
    """`float.string()`: the shortest digits that read back, in fixed notation
    for exponents -4..5 and `d.ddde±XX` otherwise; an integral value has no
    `.0`."""
    if x == 0:
        return "-0" if str(x).startswith("-") else "0"
    sign = "-" if x < 0 else ""
    mantissa, _, exponent = f"{abs(x):.17e}".partition("e")
    # The shortest round-trip digits: Python's repr knows them.
    shortest = repr(abs(x))
    if "e" in shortest:
        digits_part, _, exp_part = shortest.partition("e")
        exp = int(exp_part)
        digits = digits_part.replace(".", "")
        exp10 = exp + (len(digits_part.split(".")[0]) - 1)
    else:
        whole, _, frac = shortest.partition(".")
        if frac == "0":
            frac = ""
        raw = (whole + frac).lstrip("0")
        if whole.strip("0"):
            exp10 = len(whole.lstrip("0")) - 1
        else:
            exp10 = -(len(frac) - len(frac.lstrip("0")) + 1)
        digits = raw
    digits = digits.rstrip("0") or "0"
    if -4 <= exp10 < 6:
        if exp10 >= 0:
            whole = digits[: exp10 + 1].ljust(exp10 + 1, "0")
            frac = digits[exp10 + 1:]
        else:
            whole = "0"
            frac = "0" * (-exp10 - 1) + digits
        return sign + whole + ("." + frac if frac else "")
    head, tail = digits[0], digits[1:]
    return f"{sign}{head}{'.' + tail if tail else ''}e{'+' if exp10 >= 0 else '-'}{abs(exp10):02d}"


def text_rat(q):
    return str(q.numerator) if q.denominator == 1 else f"{q.numerator}/{q.denominator}"


def name_of(text):
    text = text.replace("\\", "/").replace('"', "'").replace("\n", " ")
    return text if len(text) <= 90 else text[:87] + "..."


# ----- a generated program ---------------------------------------------------------

class Program:
    def __init__(self, title, imports=(), declarations=(), asynchronous=False):
        self.title = title
        self.imports = list(imports)
        self.declarations = list(declarations)
        self.helpers = []
        self.checks = []
        self.asynchronous = asynchronous
        self.counter = 0
        # Checks held back by an open issue: {issue: count}.
        self.skipped = {}
        # Checks of one group share setup and stay in one part.
        self.group_id = 0

    def group(self):
        self.group_id += 1

    def skip(self, issue):
        self.skipped[issue] = self.skipped.get(issue, 0) + 1

    def fresh(self, prefix):
        self.counter += 1
        return f"{prefix}{self.counter}"

    def is_(self, got, expected, name, setup=()):
        if setup:
            self.group()
        self.checks.append((list(setup), f't = t.is({got}, {expected}, "{name_of(name)}");', self.group_id))

    def ok(self, condition, name, setup=()):
        if setup:
            self.group()
        self.checks.append((list(setup), f't = t.ok({condition}, "{name_of(name)}");', self.group_id))

    def render(self):
        lines = ["module t v1.0.0;", "",
                 f"// Generated by tools/t/gen.py (#245): {self.title}. Do not edit."]
        for issue, count in sorted(self.skipped.items()):
            lines.append(f"// Held back until #{issue} is fixed: {count} check(s).")
        lines.append('import "tap" take Tap;')
        lines += self.imports
        lines.append("")
        lines += self.declarations
        lines += ["", "object Checks {", "    Checks();", ""]
        for helper in self.helpers:
            lines += ["    " + line for line in helper.splitlines()]
        # Forty checks to a part, cut only between groups.
        parts, part = [], []
        for index, check in enumerate(self.checks):
            if len(part) >= CHUNK and check[2] != part[-1][2]:
                parts.append(part)
                part = []
            part.append(check)
        if part:
            parts.append(part)
        prefix = "async " if self.asynchronous else ""
        for index, part in enumerate(parts):
            lines.append(f"    {prefix}part{index}(start is Tap) is Tap {{")
            lines.append("        let t = start;")
            for setup, check, _ in part:
                lines += ["        " + line for line in setup]
                lines.append("        " + check)
            lines += ["        t;", "    }"]
        lines += ["}", ""]
        lines.append(f"{prefix}main() is int {{")
        lines.append("    let checks = Checks();")
        lines.append(f"    let t = Tap({len(self.checks)});")
        for index in range(len(parts)):
            lines.append(f"    t = {'await ' if self.asynchronous else ''}checks.part{index}(t);")
        lines += ["    t.done();", "}", ""]
        return "\n".join(lines)


def split(program_factory, checks_into, total_name, out):
    """Write `program` files, PER_FILE checks each; returns (files, checks)."""
    raise NotImplementedError


# ----- family: values through syntactic contexts ---------------------------------

# One representative per class of every type; `other` gives a second value of
# the same type for contexts that choose between two.
VALUE_TYPES = [
    # (type, literals, comparison, map key, constant, needs)
    ("int", [lit_int(0), lit_int(-1), lit_int(2 ** 63 - 1), lit_int(2 ** 63), lit_int(-(10 ** 30))], "is", True, True),
    ("float", [lit_float(0.5), lit_float(-1e-05), lit_float(1e16), lit_float(5e-324)], "is", False, True),
    # `rat` is not a map key (#296).
    ("rat", [lit_rat(fractions.Fraction(1, 3)), lit_rat(fractions.Fraction(-7, 2)), lit_rat(fractions.Fraction(10 ** 20, 3))], "is", False, True),
    ("byte", ["byte(0)", "byte(255)"], "is", False, True),
    ("bool", ["true", "false"], "is", True, True),
    ("string", [lit_str(""), lit_str("héllo"), lit_str("😀\n\"q\"")], "is", True, True),
    ("char", [lit_char("x"), lit_char("😀"), lit_char("'")], "is", True, True),
    ("@int", ["[]", "[1, -2, 3]"], "eq", False, True),
    ("@string", ['[""]', '["a", "b"]'], "eq", False, True),
    # A literal mixing `[]` with a typed sibling is inferred as `any` (#297).
    ("@@int", ["[[1], [-2]]", "[[2, 3]]"], "eq", False, True),
    ("%string:int", ["[:]", '["a": 1, "b": -2]'], "eq", False, True),
    ("%int:@string", ['[1: ["x"]]', '[-5: ["w"], 2: ["y", "z"]]'], "eq", False, True),
    ("Point", ["Point(x: 1, y: -2)", "Point(x: 0, y: 0)"], "eq", True, False),
    ("Status", ["Status.ready", "Status.done"], "eq", True, True),
    ("Reading", ["Reading.empty", "Reading.pair(1, 2)", "Reading.value(-7)"], "eq", False, False),
    ("Maybe<int>", ["Maybe.some(3)", "Maybe<int>.none"], "eq", False, False),
    ("Box<string>", ['Box(value: "boxed")', 'Box(value: "")'], "eq", False, False),
]

VALUE_DECLARATIONS = """enum Status = (ready, failed, done);
enum Reading = (empty, value(item is int), pair(left is int, right is int));
enum Slot<T> = (vacant, full(item is T));

object Point {
    has x is int;
    has y is int;
    Point(:x, :y);
}

object Box<T> {
    has value is T;
    Box(:value);
    get() is T { value; }
}""".splitlines()


def type_slug(t):
    return "".join(c if c.isalnum() else "_" for c in t)


def values_family():
    program = Program("every class of value through every syntactic context",
                      imports=['import "cell" take Cell;'], declarations=VALUE_DECLARATIONS)
    holders = []
    for type_name, literals, compare, key, constant in VALUE_TYPES:
        slug = type_slug(type_name)
        # A plain object, a successor and identity actions for this type.
        holders += [f"object Hold_{slug} {{",
                    f"    has value is {type_name};",
                    f"    Hold_{slug}(:value);",
                    f"    get() is {type_name} {{ value; }}",
                    f"    put(replacement is {type_name}) is ^Hold_{slug} {{ ^value = replacement; }}",
                    "}", ""]
        program.helpers.append(f"same_{slug}(value is {type_name}) is {type_name} {{ value; }}")
        for index, literal in enumerate(literals):
            other = literals[(index + 1) % len(literals)]
            label = f"{type_name} {literal}"
            const_name = f"C_{slug.upper()}_{index}"
            if constant:
                program.declarations.append(f"const {const_name} = {literal};")
            program.helpers.append(f"default_{slug}_{index}(value is {type_name} = {literal}) is {type_name} {{ value; }}")

            empty = "[]" in literal or "[:]" in literal
            typed = f"<{type_name}>" if empty else ""

            def check(result, context, setup=()):
                if compare == "is":
                    program.is_(f"({result})", literal, f"{label} via {context}", setup)
                else:
                    program.ok(f"({result}) == {literal}", f"{label} via {context}", setup)

            v = program.fresh("v")
            check(v, "untyped let", [f"let {v} = {literal};"])
            v = program.fresh("v")
            check(v, "typed let", [f"let {v} is {type_name} = {literal};"])
            check(f"Hold_{slug}(value: {literal}).get()", "a field")
            check(f"Hold_{slug}(value: {other}).put({literal}).get()", "a successor")
            check(f"self.same_{slug}({literal})", "a parameter and a result")
            check(f"self.same_{slug}(value: {literal})", "a named argument")
            check(f"self.default_{slug}_{index}()", "a default parameter value")
            check(f"[{literal}][0]", "an array element")
            v = program.fresh("pair")
            check(f"{v}.last()", "the last element", [f"let {v} is @{type_name} = [{other}, {literal}];"])
            check(f"[[{literal}]][0][0]", "a nested array element")
            check(f'["k": {literal}]["k"]', "a map value")
            check(f'["k": {literal}]["missing"] else {literal}', "a map fallback")
            if empty:
                v = program.fresh("box")
                check(f"{v}.get()", "a generic field", [f"let {v} is Box<{type_name}> = Box(value: {literal});"])
            else:
                check(f"Box(value: {literal}).get()", "a generic field")
            v, f = program.fresh("c"), program.fresh("f")
            check(f"{f}()", "a closure capture", [f"let {v} = {literal};", f"let {f} = () is {type_name} {{ {v}; }};"])
            f = program.fresh("f")
            check(f"{f}({literal})", "a closure parameter", [f"let {f} = (x is {type_name}) is {type_name} {{ x; }};"])
            check(f"Maybe{typed}.some({literal}) else {other}", "Maybe else")
            check(f"Maybe{typed}.some({literal}) ?? .some(x): x !! .none: {other}", "a choice on Maybe")
            check(f"match Slot<{type_name}>.full({literal}) {{ case Slot.full(x) {{ x; }} case Slot.vacant {{ {other}; }} }}", "a variant payload")
            check(f"if 1 < 2 {{ {literal}; }} else {{ {other}; }}", "the then branch")
            check(f"if 2 < 1 {{ {other}; }} else {{ {literal}; }}", "the else branch")
            check(f"match 2 {{ case 1 {{ {other}; }} case 2 {{ {literal}; }} else {{ {other}; }} }}", "a match arm")
            if empty:
                program.skip(298)
            else:
                check(f"false ?? {other} !! {literal}", "a boolean choice")
            check(f"loop {{ last {literal}; }}", "loop last")
            check(f"(0..0).reduce({literal}) -> acc, i {{ acc; }}", "an initial reduce value")
            check(f"try {{ {literal}; }} catch e is Error {{ {other}; }}", "a try value")
            c = program.fresh("cell")
            check(f"{c}.value()", "a cell", [f"let {c} is Cell<{type_name}> = Cell({literal});"])
            c = program.fresh("cell")
            check(f"{c}.value()", "a for loop body",
                  [f"let {c} is Cell<{type_name}> = Cell({other});", f"for [{literal}] -> x {{ {c}.replace(x); }}"])
            c = program.fresh("cell")
            check(f"{c}.value()", "a deferred statement",
                  [f"let {c} is Cell<{type_name}> = Cell({other});", f"if true {{ defer {c}.replace({literal}); }}"])
            if constant:
                check(const_name, "a module constant")
            if key:
                program.ok(f"[{literal}: 1].keys()[0] == {literal}", f"{label} as a map key")
                program.ok(f"[{literal}: 1].has({literal})", f"{label} finds its own key")
    program.declarations += [""] + holders
    return [program]


# ----- programs of a family, split into files ------------------------------------

class Batch:
    """Programs of one family; a new file starts every PER_FILE checks."""

    def __init__(self, title, **options):
        self.title = title
        self.options = options
        self.programs = []
        self.current = None

    def program(self):
        if self.current is None or len(self.current.checks) >= PER_FILE:
            self.current = Program(f"{self.title}, part {len(self.programs) + 1}", **self.options)
            self.programs.append(self.current)
        return self.current

    def is_(self, *args, **kwargs):
        self.program().is_(*args, **kwargs)

    def ok(self, *args, **kwargs):
        self.program().ok(*args, **kwargs)


# ----- family: operators over classes of operands ----------------------------------

MASK = (1 << 64) - 1


def signed64(n):
    n &= MASK
    return n - (1 << 64) if n >= 1 << 63 else n


def trunc_div(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def trunc_mod(a, b):
    return a - b * trunc_div(a, b)


def lit_bool(b):
    return "true" if b else "false"


COMPARISONS = [("==", lambda a, b: a == b), ("!=", lambda a, b: a != b), ("<", lambda a, b: a < b),
               ("<=", lambda a, b: a <= b), (">", lambda a, b: a > b), (">=", lambda a, b: a >= b)]
ORDER = {-1: "Order.less", 0: "Order.equal", 1: "Order.greater"}


def cmp(a, b):
    return (a > b) - (a < b)


INT_CLASSES = [0, 1, -1, 7, -7, 2 ** 31, 2 ** 62, 2 ** 63 - 1, -(2 ** 63), 2 ** 63, -(2 ** 63) - 1, 10 ** 20, -(10 ** 20)]


def int_family():
    batch = Batch("int operators and builtins over classes of operands", imports=['import "compare" take Order;'])
    for a in INT_CLASSES:
        A = receiver(lit_int(a))
        for b in INT_CLASSES:
            B = lit_int(b)
            batch.is_(f"{A} + {B}", lit_int(a + b), f"{a} + {b}")
            batch.is_(f"{A} - {B}", lit_int(a - b), f"{a} - {b}")
            batch.is_(f"{A} * {B}", lit_int(a * b), f"{a} * {b}")
            if b != 0:
                batch.is_(f"{A} % {B}", lit_int(trunc_mod(a, b)), f"{a} % {b}")
                batch.is_(f"({A} / {B}).string()", lit_str(text_rat(fractions.Fraction(a, b))), f"{a} / {b}")
            for op, f in COMPARISONS:
                batch.is_(f"{A} {op} {B}", lit_bool(f(a, b)), f"{a} {op} {b}")
            batch.ok(f"{A}.order({B}) == {ORDER[cmp(a, b)]}", f"{a}.order({b})")
            batch.is_(f"{A}.less({B})", lit_bool(a < b), f"{a}.less({b})")
            batch.is_(f"{A}.greater_or_equal({B})", lit_bool(a >= b), f"{a}.greater_or_equal({b})")
        batch.is_(f"-{A}", lit_int(-a), f"-({a})")
        batch.is_(f"{A}.abs()", lit_int(abs(a)), f"{a}.abs()")
        batch.is_(f"{A}.sign()", lit_int(cmp(a, 0)), f"{a}.sign()")
        batch.is_(f"{A}.string()", lit_str(str(a)), f"{a}.string()")
        batch.is_(f"string({lit_int(a)})", lit_str(str(a)), f"string({a})")
        batch.is_(f"{A}.rat().string()", lit_str(str(a)), f"{a}.rat()")
        batch.is_(f"{A}.float()", lit_float(float(a)), f"{a}.float()")
        batch.is_(f"float({lit_int(a)})", lit_float(float(a)), f"float({a})")
        for e in [0, 1, 2, 3, 5]:
            batch.is_(f"{A}.pow({e})", lit_int(a ** e), f"{a}.pow({e})")
        if a >= 0:
            import math
            batch.is_(f"{A}.sqrt()", lit_float(math.sqrt(float(a))), f"{a}.sqrt()")
        if 0 <= a <= 255:
            batch.is_(f"{A}.byte().int()", lit_int(a), f"{a}.byte()")
        batch.is_(f"{A} == {A}", "true", f"{a} == itself")
    return batch.programs


FLOAT_CLASSES = [0.0, -0.0, 0.5, -1.5, 0.1, 3.0, 1e16, 1e-300, 1e300, 5e-324, 123456.789, -2.5e-07, 2.0 ** 53 + 2]


def float_family():
    import math
    batch = Batch("float operators, builtins and text over classes of operands", imports=['import "compare" take Order;'])
    for a in FLOAT_CLASSES:
        A = receiver(lit_float(a))
        for b in FLOAT_CLASSES:
            B = lit_float(b)
            results = [("+", a + b), ("-", a - b), ("*", a * b)]
            if b != 0:
                results.append(("/", a / b))
            for op, r in results:
                if math.isfinite(r):
                    batch.is_(f"{A} {op} {B}", lit_float(r), f"{a!r} {op} {b!r}")
                    batch.is_(f"({A} {op} {B}).string()", lit_str(text_float(r)), f"({a!r} {op} {b!r}).string()")
            for op, f in COMPARISONS:
                batch.is_(f"{A} {op} {B}", lit_bool(f(a, b)), f"{a!r} {op} {b!r}")
            batch.ok(f"{A}.order({B}) == {ORDER[cmp(a, b)]}", f"{a!r}.order({b!r})")
        batch.is_(f"-{A}", lit_float(-a), f"-({a!r})")
        batch.is_(f"(-{A}).string()", lit_str(text_float(-a)), f"(-({a!r})).string()")
        batch.is_(f"{A}.abs()", lit_float(abs(a)), f"{a!r}.abs()")
        batch.is_(f"{A}.sign()", lit_int(cmp(a, 0)), f"{a!r}.sign()")
        batch.is_(f"{A}.string()", lit_str(text_float(a)), f"{a!r}.string()")
        batch.is_(f"string({lit_float(a)})", lit_str(text_float(a)), f"string({a!r})")
        if abs(a) < 2 ** 63:
            batch.is_(f"{A}.int()", lit_int(int(a)), f"{a!r}.int()")
        if a >= 0:
            batch.is_(f"{A}.sqrt()", lit_float(math.sqrt(a)), f"{a!r}.sqrt()")
        batch.is_(f"{A}.rat().string()", lit_str(text_rat(fractions.Fraction(repr(a)))), f"{a!r}.rat()")
    return batch.programs


RAT_CLASSES = [fractions.Fraction(0), fractions.Fraction(1, 3), fractions.Fraction(-7, 2), fractions.Fraction(5),
               fractions.Fraction(10 ** 20, 3), fractions.Fraction(1, 10 ** 20), fractions.Fraction(-1, 2)]


def rat_family():
    batch = Batch("rat operators and builtins over classes of operands", imports=['import "compare" take Order;'])
    for a in RAT_CLASSES:
        A = lit_rat(a)
        for b in RAT_CLASSES:
            B = lit_rat(b)
            results = [("+", a + b), ("-", a - b), ("*", a * b)]
            if b != 0:
                results.append(("/", a / b))
            for op, r in results:
                batch.is_(f"({A} {op} {B}).string()", lit_str(text_rat(r)), f"{a} {op} {b}")
                batch.is_(f"{A} {op} {B}", lit_rat(r), f"{a} {op} {b} as a value")
            for op, f in COMPARISONS:
                batch.is_(f"{A} {op} {B}", lit_bool(f(a, b)), f"{a} {op} {b}")
            batch.ok(f"{A}.order({B}) == {ORDER[cmp(a, b)]}", f"{a}.order({b})")
        batch.is_(f"(-{A}).string()", lit_str(text_rat(-a)), f"-({a})")
        batch.is_(f"{A}.abs().string()", lit_str(text_rat(abs(a))), f"{a}.abs()")
        batch.is_(f"{A}.sign()", lit_int(cmp(a, 0)), f"{a}.sign()")
        batch.is_(f"{A}.int()", lit_int(int(a)), f"{a}.int()")
        batch.is_(f"{A}.numerator()", lit_str(str(a.numerator)), f"{a}.numerator()")
        batch.is_(f"{A}.denominator()", lit_str(str(a.denominator)), f"{a}.denominator()")
        batch.is_(f"{A}.float()", lit_float(float(a)), f"{a}.float()")
        batch.is_(f"string({A})", lit_str(text_rat(a)), f"string({a})")
        for e in [-2, -1, 0, 1, 3]:
            if e < 0 and a == 0:
                continue
            batch.is_(f"{A}.pow({lit_int(e)}).string()", lit_str(text_rat(a ** e)), f"{a}.pow({e})")
    return batch.programs


BYTE_CLASSES = [0, 1, 127, 128, 254, 255]


def byte_family():
    batch = Batch("byte operators, bits and conversions over classes of operands", imports=['import "compare" take Order;'])
    for a in BYTE_CLASSES:
        A = f"byte({a})"
        for b in BYTE_CLASSES:
            B = f"byte({b})"
            batch.is_(f"{A} + {B}", lit_int(a + b), f"byte {a} + {b} is an int")
            batch.is_(f"{A} - {B}", lit_int(a - b), f"byte {a} - {b}")
            batch.is_(f"{A} * {B}", lit_int(a * b), f"byte {a} * {b}")
            if b:
                batch.is_(f"({A} / {B}).string()", lit_str(text_rat(fractions.Fraction(a, b))), f"byte {a} / {b} is a rat")
                batch.is_(f"{A} % {B}", lit_int(a % b), f"byte {a} % {b}")
            batch.is_(f"{A} & {B}", f"byte({a & b})", f"byte {a} & {b}")
            batch.is_(f"{A} | {B}", f"byte({a | b})", f"byte {a} | {b}")
            batch.is_(f"{A} ^ {B}", f"byte({a ^ b})", f"byte {a} ^ {b}")
            for op, f in COMPARISONS:
                batch.is_(f"{A} {op} {B}", lit_bool(f(a, b)), f"byte {a} {op} {b}")
        batch.is_(f"~{A}", f"byte({~a & 255})", f"~byte {a}")
        for n in range(8):
            batch.is_(f"{A} << {n}", f"byte({(a << n) & 255})", f"byte {a} << {n}")
            batch.is_(f"{A} >> {n}", f"byte({a >> n})", f"byte {a} >> {n}")
            batch.is_(f"{A} >>> {n}", f"byte({a >> n})", f"byte {a} >>> {n}")
            batch.is_(f"{A}.rotate_left({n})", f"byte({((a << n) | (a >> (8 - n))) & 255})", f"byte {a} rotate_left {n}")
            batch.is_(f"{A}.rotate_right({n})", f"byte({((a >> n) | (a << (8 - n))) & 255})", f"byte {a} rotate_right {n}")
        batch.is_(f"{A}.reverse_bits()", f"byte({int(format(a, '08b')[::-1], 2)})", f"byte {a} reverse_bits")
        batch.is_(f"{A}.swap_bytes()", A, f"byte {a} swap_bytes")
        batch.is_(f"{A}.bit_count()", lit_int(bin(a).count('1')), f"byte {a} bit_count")
        batch.is_(f"{A}.leading_zeros()", lit_int(8 - a.bit_length()), f"byte {a} leading_zeros")
        batch.is_(f"{A}.trailing_zeros()", lit_int(8 if a == 0 else (a & -a).bit_length() - 1), f"byte {a} trailing_zeros")
        batch.is_(f"{A}.int()", lit_int(a), f"byte {a}.int()")
        batch.is_(f"{A}.float()", lit_float(float(a)), f"byte {a}.float()")
        batch.is_(f"{A}.rat().string()", lit_str(str(a)), f"byte {a}.rat()")
        batch.is_(f"{A}.string()", lit_str(str(a)), f"byte {a}.string()")
        batch.is_(f"byte({a}.5)", A, f"byte({a}.5) truncates")
        batch.ok(f"{A}.order(byte(128)) == {ORDER[cmp(a, 128)]}", f"byte {a}.order(128)")
    return batch.programs


BIT_CLASSES = [0, 1, -1, 2 ** 62, -(2 ** 63), 2 ** 63 - 1, 0x5555555555555555, signed64(0xAAAAAAAAAAAAAAAA), 255, -256]
SHIFTS = [0, 1, 7, 31, 32, 63]


def bits_family():
    batch = Batch("int bitwise operators over 64-bit patterns")
    for a in BIT_CLASSES:
        A = receiver(lit_int(a))
        for b in BIT_CLASSES:
            B = lit_int(b)
            batch.is_(f"{A} & {B}", lit_int(signed64(a & b)), f"{a} & {b}")
            batch.is_(f"{A} | {B}", lit_int(signed64(a | b)), f"{a} | {b}")
            batch.is_(f"{A} ^ {B}", lit_int(signed64(a ^ b)), f"{a} ^ {b}")
        batch.is_(f"~{A}", lit_int(signed64(~a)), f"~{a}")
        u = a & MASK
        for n in SHIFTS:
            batch.is_(f"{A} << {n}", lit_int(signed64(a << n)), f"{a} << {n}")
            batch.is_(f"{A} >> {n}", lit_int(a >> n), f"{a} >> {n}")
            batch.is_(f"{A} >>> {n}", lit_int(signed64(u >> n)), f"{a} >>> {n}")
            batch.is_(f"{A}.rotate_left({n})", lit_int(signed64((u << n) | (u >> (64 - n)))), f"{a} rotate_left {n}")
            batch.is_(f"{A}.rotate_right({n})", lit_int(signed64((u >> n) | (u << (64 - n)))), f"{a} rotate_right {n}")
        batch.is_(f"{A}.reverse_bits()", lit_int(signed64(int(format(u, '064b')[::-1], 2))), f"{a} reverse_bits")
        batch.is_(f"{A}.swap_bytes()", lit_int(signed64(int.from_bytes(u.to_bytes(8, 'big'), 'little'))), f"{a} swap_bytes")
        batch.is_(f"{A}.bit_count()", lit_int(bin(u).count('1')), f"{a} bit_count")
        batch.is_(f"{A}.leading_zeros()", lit_int(64 - u.bit_length()), f"{a} leading_zeros")
        batch.is_(f"{A}.trailing_zeros()", lit_int(64 if u == 0 else (u & -u).bit_length() - 1), f"{a} trailing_zeros")
    return batch.programs


# ----- family: precedence and associativity -----------------------------------------

# Binary operators by precedence level (Book §11.1, loosest first): the level,
# operand types and result type. `==`/`!=` take ints or bools.
BINARY = {
    "||": (3, "bool", "bool"), "&&": (4, "bool", "bool"),
    "==": (6, "eq", "bool"), "!=": (6, "eq", "bool"),
    "<": (8, "int", "bool"), "<=": (8, "int", "bool"), ">": (8, "int", "bool"), ">=": (8, "int", "bool"),
    "|": (9, "int", "int"), "^": (10, "int", "int"), "&": (11, "int", "int"),
    "<<": (12, "int", "int"), ">>": (12, "int", "int"), ">>>": (12, "int", "int"),
    "+": (13, "int", "int"), "-": (13, "int", "int"), "*": (14, "int", "int"), "%": (14, "int", "int"),
}
UNARY = {"-": "int", "~": "int", "!": "bool"}


class Invalid(Exception):
    pass


def apply(op, a, b):
    level, operand, _ = BINARY[op]
    if operand == "bool" and not (isinstance(a, bool) and isinstance(b, bool)):
        raise Invalid
    if operand == "int" and (isinstance(a, bool) or isinstance(b, bool)):
        raise Invalid
    if operand == "eq" and isinstance(a, bool) != isinstance(b, bool):
        raise Invalid
    if op == "||":
        return a or b
    if op == "&&":
        return a and b
    if op == "==":
        return a == b
    if op == "!=":
        return a != b
    if op in ("<", "<=", ">", ">="):
        return {"<": a < b, "<=": a <= b, ">": a > b, ">=": a >= b}[op]
    if op in ("|", "^", "&", "<<", ">>", ">>>"):
        if not (-(2 ** 63) <= a < 2 ** 63 and -(2 ** 63) <= b < 2 ** 63):
            raise Invalid
    if op == "|":
        return signed64(a | b)
    if op == "^":
        return signed64(a ^ b)
    if op == "&":
        return signed64(a & b)
    if op in ("<<", ">>", ">>>"):
        if not 0 <= b <= 63:
            raise Invalid
        return {"<<": signed64(a << b), ">>": a >> b, ">>>": signed64((a & MASK) >> b)}[op]
    if op == "+":
        return a + b
    if op == "-":
        return a - b
    if op == "*":
        return a * b
    if op == "%":
        if b == 0:
            raise Invalid
        return trunc_mod(a, b)
    raise AssertionError(op)


def evaluate(tree):
    if tree[0] == "leaf":
        return tree[1]
    if tree[0] == "unary":
        value = evaluate(tree[2])
        if tree[1] == "!":
            if not isinstance(value, bool):
                raise Invalid
            return not value
        if isinstance(value, bool):
            raise Invalid
        if tree[1] == "~":
            if not -(2 ** 63) <= value < 2 ** 63:
                raise Invalid
            return signed64(~value)
        return -value
    return apply(tree[1], evaluate(tree[2]), evaluate(tree[3]))


def trees(ops, leaves):
    """Every binary tree over the leaves in order with the operators between."""
    if len(leaves) == 1:
        return [leaves[0]]
    out = []
    for split_at in range(len(ops)):
        for left in trees(ops[:split_at], leaves[:split_at + 1]):
            for right in trees(ops[split_at + 1:], leaves[split_at + 1:]):
                out.append(("bin", ops[split_at], left, right))
    return out


def parse(ops, leaves):
    """Okkama's tree: the loosest operator splits, the rightmost of equal ones
    (left associativity)."""
    if len(leaves) == 1:
        return leaves[0]
    loosest = min(BINARY[op][0] for op in ops)
    at = max(i for i, op in enumerate(ops) if BINARY[op][0] == loosest)
    return ("bin", ops[at], parse(ops[:at], leaves[:at + 1]), parse(ops[at + 1:], leaves[at + 1:]))


def render(tree, parenthesize=False):
    if tree[0] == "leaf":
        value = tree[1]
        return lit_bool(value) if isinstance(value, bool) else lit_int(value)
    if tree[0] == "unary":
        return tree[1] + render(tree[2], True)
    text = f"{render(tree[2], parenthesize)} {tree[1]} {render(tree[3], parenthesize)}"
    return f"({text})" if parenthesize else text


def lit_value(value):
    return lit_bool(value) if isinstance(value, bool) else lit_int(value)


INT_LEAVES = [0, 1, 2, 3, 5, 7, 12, -1, -3, -9, 62]


def precedence_family():
    import itertools
    import random
    batch = Batch("precedence and associativity: every pair and triple of operators, discriminating values")
    rng = random.Random(245)
    ops = list(BINARY)

    def leaf_choices(count):
        pools = [[("leaf", v) for v in INT_LEAVES] + [("leaf", True), ("leaf", False)]] * count
        return pools

    def emit(op_seq, unary=None):
        n = len(op_seq) + 1
        for _ in range(400):
            leaves = [("leaf", rng.choice(INT_LEAVES + [True, False])) for _ in range(n)]
            if unary:
                leaves[0] = ("unary", unary, leaves[0])
            correct = parse(list(op_seq), leaves)
            try:
                value = evaluate(correct)
            except Invalid:
                continue
            others = [t for t in trees(list(op_seq), leaves) if t != correct]
            if unary:
                # The unary operator could also cover the whole left operand.
                bare = [leaves[0][2]] + leaves[1:]
                inner = parse(list(op_seq), bare)
                others.append(("unary", unary, inner))
            distinct = False
            for other in others:
                try:
                    if evaluate(other) != value:
                        distinct = True
                except Invalid:
                    distinct = True
            if not distinct:
                continue
            text = render(correct)
            if unary:
                text = unary + render(leaves[0][2], True) + text[len(render(leaves[0])):]
            name = "precedence: " + text
            batch.is_(text, lit_value(value), name)
            batch.is_(render(correct, True), lit_value(value), "grouping: " + render(correct, True))
            return True
        return False

    for a, b in itertools.product(ops, repeat=2):
        emit((a, b))
    for a, b, c in itertools.product(ops, repeat=3):
        emit((a, b, c))
    for u in UNARY:
        for a in ops:
            emit((a,), unary=u)
    return batch.programs


# ----- family: roles inside data structures -------------------------------------

ROLE_DECLARATIONS = """role Shape {
    area() is int;
    name() is string;
    scaled(k is int) is ^Shape;
}

role Named {
    name() is string;
}

role Source<T> {
    item() is T;
}

object Square does Shape, Named {
    has side is int;
    Square(:side);
    area() is int { side * side; }
    name() is string { "square " + side.string(); }
    scaled(k is int) is ^Square { ^side = side * k; }
}

object Rect does Shape {
    has w is int;
    has h is int;
    Rect(:w, :h);
    area() is int { w * h; }
    name() is string { "rect " + w.string() + "x" + h.string(); }
    scaled(k is int) is ^Rect { ^w = w * k; ^h = h * k; }
}

object Dot does Shape {
    has mark is string = ".";
    Dot();
    area() is int { 0; }
    name() is string { "dot"; }
    scaled(k is int) is ^Dot { ^mark = mark; }
}

object Constant<T> does Source<T> {
    has value is T;
    Constant(:value);
    item() is T { value; }
}

object Holder {
    has shape is Shape;
    Holder(:shape);
    get() is Shape { shape; }
    put(next_shape is Shape) is ^Holder { ^shape = next_shape; }
}

object Pair<T> {
    has first is T;
    has second is T;
    Pair(:first, :second);
    left() is T { first; }
    right() is T { second; }
}

enum Held = (nothing, one(shape is Shape));""".splitlines()


class Shape:
    def __init__(self, kind, *dims):
        self.kind, self.dims = kind, dims

    def literal(self):
        if self.kind == "square":
            return f"Square(side: {self.dims[0]})"
        if self.kind == "rect":
            return f"Rect(w: {self.dims[0]}, h: {self.dims[1]})"
        return "Dot()"

    def area(self):
        return {"square": lambda: self.dims[0] ** 2, "rect": lambda: self.dims[0] * self.dims[1], "dot": lambda: 0}[self.kind]()

    def name(self):
        return {"square": lambda: f"square {self.dims[0]}", "rect": lambda: f"rect {self.dims[0]}x{self.dims[1]}",
                "dot": lambda: "dot"}[self.kind]()

    def scaled(self, k):
        return self if self.kind == "dot" else Shape(self.kind, *[d * k for d in self.dims])

    def __eq__(self, other):
        return self.kind == other.kind and self.dims == other.dims


SHAPES = [Shape("square", 2), Shape("rect", 2, 3), Shape("dot")]


def roles_family():
    batch = Batch("roles inside data structures and control flow",
                  imports=['import "cell" take Cell;'], declarations=ROLE_DECLARATIONS)
    for shape in SHAPES:
        V = shape.literal()
        other = SHAPES[(SHAPES.index(shape) + 1) % len(SHAPES)]
        O = other.literal()
        program = batch.program()
        f = program.fresh

        def containers():
            yield "a typed local", [], V, "let"
            a = f("a")
            yield "an array element", [f"let {a} is @Shape = [{V}];"], f"{a}[0]", "let"
            a = f("a")
            yield "the last of a mixed array", [f"let {a} is @Shape = [{O}, {V}];"], f"{a}.last()", "let"
            m = f("m")
            yield "a map value", [f'let {m} is %string:Shape = ["k": {V}];'], f'{m}["k"]', "let"
            m, fb = f("m"), f("fb")
            yield "a map fallback", [f'let {m} is %string:Shape = ["k": {O}];', f"let {fb} is Shape = {V};"], f'{m}["zz"] else {fb}', "let"
            mb = f("mb")
            yield "Maybe else", [f"let {mb} is Maybe<Shape> = Maybe.some({V});"], f"{mb} else {O}", "let"
            mb = f("mb")
            yield "a choice on Maybe", [f"let {mb} is Maybe<Shape> = Maybe.some({V});"], f"{mb} ?? .some(s): s !! .none: {O}", "let"
            yield "a variant payload", [], f"match Held.one({V}) {{ case Held.one(s) {{ s; }} case Held.nothing {{ {O}; }} }}", "let"
            p = f("p")
            yield "a generic pair", [f"let {p} is Pair<Shape> = Pair(first: {V}, second: {O});"], f"{p}.left()", "let"
            p = f("p")
            yield "the other side of a pair", [f"let {p} is Pair<Shape> = Pair(first: {O}, second: {V});"], f"{p}.right()", "let"
            c = f("cell")
            yield "a cell", [f"let {c} is Cell<Shape> = Cell({O});", f"{c}.replace({V});"], f"{c}.value()", "let"
            fn = f("f")
            yield "a closure result", [f"let {fn} = () is Shape {{ {V}; }};"], f"{fn}()", "let"
            fn = f("g")
            yield "a closure parameter", [f"let {fn} = (s is Shape) is Shape {{ s; }};"], f"{fn}({V})", "let"
            n = f("n")
            yield "a nested array", [f"let {n} is @@Shape = [[{O}], [{V}]];"], f"{n}[1][0]", "let"
            ma = f("ma")
            yield "a map of arrays", [f'let {ma} is %string:@Shape = ["k": [{O}, {V}]];'], f'{ma}["k"][1]', "let"
            yield "a field", [], f"Holder(shape: {V}).get()", "let"
            yield "a successor field", [], f"Holder(shape: {O}).put({V}).get()", "let"
            a = f("a")
            yield "reduce keeping the last", [f"let {a} is @Shape = [{O}, {V}];"], f"{a}.reduce({O}) -> acc, s {{ s; }}", "let"
            a = f("a")
            yield "map identity", [f"let {a} is @Shape = [{V}];"], f"({a}.map() -> s {{ s; }})[0]", "let"
            a = f("a")
            yield "find", [f"let {a} is @Shape = [{O}, {V}];"], f"{a}.find() -> s {{ s == {V}; }} else {O}", "let"
            yield "an if branch", [], f"if 2 > 1 {{ {V}; }} else {{ {O}; }}", "typed"
            yield "a match arm", [], f"match 3 {{ case 3 {{ {V}; }} else {{ {O}; }} }}", "typed"
            yield "loop last", [], f"loop {{ last {V}; }}", "typed"
            yield "a try value", [], f"try {{ {V}; }} catch e is Error {{ {O}; }}", "typed"
            src = f("src")
            yield "a generic role over a role", [f"let {src} is Source<Shape> = Constant(value: {V});"], f"{src}.item()", "let"

        for where, setup, expr, mode in containers():
            r, vr, orr = f("r"), f("vr"), f("or")
            # The role on both sides: an implementer is not wrapped in == or
            # else (#299).
            setup = [f"let {vr} is Shape = {V};", f"let {orr} is Shape = {O};"] + setup + [f"let {r} is Shape = {expr};"]
            setup = [line.replace(f"else {O}", f"else {orr}").replace(f"s == {V}", f"s == {vr}") for line in setup]
            label = f"{shape.name()} in {where}"
            program.is_(f"{r}.area()", lit_int(shape.area()), f"{label}: area", setup)
            program.is_(f"{r}.name()", lit_str(shape.name()), f"{label}: name")
            program.is_(f"{r}.scaled(3).area()", lit_int(shape.scaled(3).area()), f"{label}: successor through the role")
            program.ok(f"{r} == {vr}", f"{label}: equal to itself")
            program.is_(f"{r} == {orr}", lit_bool(shape == other), f"{label}: against another implementation")
            program.ok(f"{r}.scaled(1) == {vr}", f"{label}: a successor with no change")
        if shape.kind == "square":
            batch.is_(f"(n is Named) is string {{ n.name(); }}({V})", lit_str(shape.name()), "a closure taking the second role")
    # Mixed arrays of implementations under the functional operations.
    import itertools
    for size in range(0, 4):
        for combo in itertools.product(SHAPES, repeat=size):
            items = "[" + ", ".join(s.literal() for s in combo) + "]"
            program = batch.program()
            a, last, first_one = program.fresh("arr"), program.fresh("last"), program.fresh("first")
            setup = [f"let {a} is @Shape = {items};"]
            if combo:
                setup += [f"let {last} is Shape = {combo[-1].literal()};", f"let {first_one} is Shape = {combo[0].literal()};"]
            desc = "[" + ", ".join(s.name() for s in combo) + "]"
            program.is_(f"{a}.reduce(0) -> sum, s {{ sum + s.area(); }}", lit_int(sum(s.area() for s in combo)), f"{desc}: reduce over areas", setup)
            program.is_(f"({a}.filter() -> s {{ s.area() > 3; }}).size()", lit_int(sum(1 for s in combo if s.area() > 3)), f"{desc}: filter")
            names = "|".join(s.name() for s in combo)
            program.is_(f'({a}.map() -> s {{ s.name(); }}).join("|")', lit_str(names), f"{desc}: map to names")
            program.is_(f"{a}.any() -> s {{ s.area() == 0; }}", lit_bool(any(s.area() == 0 for s in combo)), f"{desc}: any")
            program.is_(f"{a}.all() -> s {{ s.area() > 0; }}", lit_bool(all(s.area() > 0 for s in combo)), f"{desc}: all")
            first = next((s for s in combo if s.area() > 3), None)
            program.is_(f'{a}.find() -> s {{ s.area() > 3; }} ?? .some(s): s.name() !! .none: "none"', lit_str(first.name() if first else "none"), f"{desc}: find")
            program.is_(f"({a}.map() -> s {{ s.scaled(2); }}).reduce(0) -> sum, s {{ sum + s.area(); }}", lit_int(sum(s.scaled(2).area() for s in combo)), f"{desc}: successors in a map")
            program.is_(f"{a}.size()", lit_int(len(combo)), f"{desc}: size")
            if combo:
                program.ok(f"{a}.reversed()[0] == {last}", f"{desc}: reversed")
                program.ok(f"{a}.contains({first_one})", f"{desc}: contains")
                program.is_(f"{a}.index_of({last}) else -1", lit_int(combo.index(combo[-1])), f"{desc}: index_of finds the first equal element")
    return batch.programs


# ----- family: string builtins over classes of input ------------------------------

STRING_CLASSES = ["", "a", "abc", "héllo", "Жук", "😀x😀", "a\tb\nc", " pad ", "\u00a0nbsp\u2003", "x\r\n",
                  "aaa", "a,b,,c,", "e\u0301", "ǆ", "ß", "İ", "ab😀ab"]
PART_CLASSES = ["", "a", ",", "aa", "😀", "\n", "zz", "b", "ab"]
REPLACEMENTS = ["", "-", "XY", "😀"]


def chomp(s):
    for end in ("\r\n", "\n", "\r"):
        if s.endswith(end):
            return s[: -len(end)]
    return s


def okkama_split(s, sep):
    return list(s) if sep == "" else s.split(sep)


def strings_family():
    batch = Batch("string builtins over classes of input", imports=['import "compare" take Order;'])
    for text in STRING_CLASSES:
        S = lit_str(text)
        R = receiver(S)
        n = len(text)
        batch.is_(f"{R}.length()", lit_int(n), f"{S}.length()")
        batch.is_(f"{R}.empty()", lit_bool(n == 0), f"{S}.empty()")
        batch.is_(f"{R}.trim()", lit_str(text.strip()), f"{S}.trim()")
        batch.is_(f"{R}.trim_start()", lit_str(text.lstrip()), f"{S}.trim_start()")
        batch.is_(f"{R}.trim_end()", lit_str(text.rstrip()), f"{S}.trim_end()")
        batch.is_(f"{R}.chomp()", lit_str(chomp(text)), f"{S}.chomp()")
        batch.is_(f"{R}.upper()", lit_str(text.upper()), f"{S}.upper()")
        batch.is_(f"{R}.lower()", lit_str(text.lower()), f"{S}.lower()")
        batch.is_(f"{R}.string()", S, f"{S}.string()")
        batch.is_(f"string({S})", S, f"string({S})")
        batch.is_(f"{R}.characters().size()", lit_int(n), f"{S}.characters().size()")
        if n:
            batch.is_(f"{R}.characters()[{n - 1}]", lit_char(text[-1]), f"{S}: the last character")
            batch.is_(f"{R}.characters()[0].code()", lit_int(ord(text[0])), f"{S}: the first code point")
        for count in (0, 1, 3):
            batch.is_(f"{R}.repeat({count})", lit_str(text * count), f"{S}.repeat({count})")
        for i, j in sorted({(0, 0), (0, n), (0, min(1, n)), (min(1, n), n), (max(n - 1, 0), n), (n, n)}):
            batch.is_(f"{R}.slice({i}, {j})", lit_str(text[i:j]), f"{S}.slice({i}, {j})")
        for part in PART_CLASSES:
            P = lit_str(part)
            label = f"{S} / {P}"
            batch.is_(f"{R}.contains({P})", lit_bool(part in text), f"{label}: contains")
            batch.is_(f"{R}.starts_with({P})", lit_bool(text.startswith(part)), f"{label}: starts_with")
            batch.is_(f"{R}.ends_with({P})", lit_bool(text.endswith(part)), f"{label}: ends_with")
            batch.is_(f"{R}.find({P}) else -1", lit_int(text.find(part)), f"{label}: find")
            batch.is_(f"{R}.find_last({P}) else -1", lit_int(text.rfind(part)), f"{label}: find_last")
            pieces = okkama_split(text, part)
            batch.is_(f"{R}.split({P}).size()", lit_int(len(pieces)), f"{label}: split size")
            batch.is_(f'{R}.split({P}).join("|")', lit_str("|".join(pieces)), f"{label}: split")
            for replacement in REPLACEMENTS:
                W = lit_str(replacement)
                batch.is_(f"{R}.replace({P}, {W})", lit_str(text.replace(part, replacement)), f"{label}: replace with {W}")
                batch.is_(f"{R}.replace_first({P}, {W})", lit_str(text.replace(part, replacement, 1)), f"{label}: replace_first with {W}")
    for left in STRING_CLASSES:
        for right in STRING_CLASSES:
            L, Rr = lit_str(left), lit_str(right)
            label = f"{L} vs {Rr}"
            batch.is_(f"{L} + {Rr}", lit_str(left + right), f"{label}: +")
            batch.is_(f"{L} == {Rr}", lit_bool(left == right), f"{label}: ==")
            batch.ok(f"{receiver(L)}.order({Rr}) == {ORDER[cmp(left, right)]}", f"{label}: order by code point")
            batch.is_(f"{receiver(L)}.less({Rr})", lit_bool(len(left) < len(right)), f"{label}: less compares lengths (#281)")
            batch.is_(f"{receiver(L)}.less_or_equal({Rr})", lit_bool(len(left) <= len(right)), f"{label}: less_or_equal")
            batch.is_(f"[{L}, {Rr}].join({lit_str(',')})", lit_str(left + "," + right), f"{label}: join")
    return batch.programs


# ----- family: arrays and maps over element types and shapes ---------------------

F = fractions.Fraction
# (type, values, literal, sort key)
ELEMENTS = [
    ("int", [3, -1, 10 ** 20], lit_int, lambda v: v),
    ("string", ["b", "", "😀"], lit_str, lambda v: v),
    ("float", [0.5, -2.0, 1e16], lit_float, lambda v: v),
    ("bool", [True, False], lit_bool, lambda v: v),
    ("char", ["x", "😀", "A"], lit_char, lambda v: ord(v)),
    ("rat", [F(1, 3), F(-7, 2), F(5)], lit_rat, lambda v: v),
    ("byte", [0, 255, 7], lambda v: f"byte({v})", lambda v: v),
    ("@int", [(1,), (), (2, 3)], lambda v: "[" + ", ".join(map(lit_int, v)) + "]", lambda v: v),
    ("Point", [(1, 2), (0, 0), (-1, 5)], lambda v: f"Point(x: {lit_int(v[0])}, y: {lit_int(v[1])})", lambda v: v),
    ("Status", ["ready", "done", "failed"], lambda v: f"Status.{v}", lambda v: ["ready", "failed", "done"].index(v)),
    ("Maybe<int>", [1, None, -2], lambda v: "Maybe<int>.none" if v is None else f"Maybe.some({lit_int(v)})", None),
    ("Reading", [("empty",), ("value", 1), ("pair", 1, 2)],
     lambda v: "Reading.empty" if v[0] == "empty" else f"Reading.{v[0]}({', '.join(map(lit_int, v[1:]))})", None),
]
MAP_KEYS = ["int", "string", "bool", "char", "Status", "float", "Point"]
SHAPES_ARRAY = [(), (0,), (0, 1), (0, 1, 0), (2, 1, 0, 1)]
SHAPES_MAP = [(), (0,), (1, 0), (0, 1, 2), (2, 0)]


def collections_family():
    batch = Batch("arrays and maps over element types and shapes", declarations=VALUE_DECLARATIONS)
    for type_name, values, literal, key in ELEMENTS:
        def array(shape):
            return "[" + ", ".join(literal(values[i % len(values)]) for i in shape) + "]"

        def items(shape):
            return [values[i % len(values)] for i in shape]

        for shape in SHAPES_ARRAY:
            program = batch.program()
            xs = program.fresh("xs")
            setup = [f"let {xs} is @{type_name} = {array(shape)};"]
            vals = items(shape)
            desc = f"@{type_name} {array(shape)}"
            n = len(vals)

            def expect(sub_shape, prefix="e"):
                name = program.fresh(prefix)
                setup.append(f"let {name} is @{type_name} = {array(sub_shape)};")
                return name

            # Expected arrays are declared up front, in the group's setup.
            sliced = expect(shape[1:]) if n else None
            taken = {k: expect(shape[:k]) for k in (0, 1, n + 1)}
            dropped = {k: expect(shape[k:]) for k in (0, 1, n + 1)}
            backwards = expect(tuple(reversed(shape)))
            nothing = expect(())
            program.is_(f"{xs}.size()", lit_int(n), f"{desc}.size()", setup)
            if n:
                program.ok(f"{xs}.first() == {literal(vals[0])}", f"{desc}.first()")
                program.ok(f"{xs}.last() == {literal(vals[-1])}", f"{desc}.last()")
                program.ok(f"{xs}[{n - 1}] == {literal(vals[-1])}", f"{desc}[{n - 1}]")
                program.ok(f"{xs}.item(0) == {literal(vals[0])}", f"{desc}.item(0)")
                program.ok(f"{xs}.slice(1, {n}) == {sliced}", f"{desc}.slice(1, {n})")
            program.ok(f"({xs}[{n}] else {literal(values[0])}) == {literal(values[0])}", f"{desc}[{n}] else")
            for k in (0, 1, n + 1):
                program.ok(f"{xs}.take({k}) == {taken[k]}", f"{desc}.take({k})")
                program.ok(f"{xs}.drop({k}) == {dropped[k]}", f"{desc}.drop({k})")
            program.ok(f"{xs}.reversed() == {backwards}", f"{desc}.reversed()")
            for v in values:
                if type_name in ("Maybe<int>", "Reading"):
                    program.skip(300)
                    program.skip(300)
                    continue
                program.is_(f"{xs}.contains({literal(v)})", lit_bool(v in vals), f"{desc}.contains({literal(v)})")
                program.is_(f"{xs}.index_of({literal(v)}) else -1", lit_int(vals.index(v) if v in vals else -1), f"{desc}.index_of({literal(v)})")
            program.ok(f"({xs}.map() -> v {{ v; }}) == {xs}", f"{desc}: map identity")
            program.ok(f"({xs}.filter() -> v {{ false; }}) == {nothing}", f"{desc}: filter nothing")
            program.is_(f"{xs}.reduce(0) -> count, v {{ count + 1; }}", lit_int(n), f"{desc}: reduce counts")
            program.is_(f"{xs}.any() -> v {{ v == {literal(values[0])}; }}", lit_bool(values[0] in vals), f"{desc}: any")
            program.is_(f"{xs}.all() -> v {{ v == {literal(values[0])}; }}", lit_bool(all(v == values[0] for v in vals)), f"{desc}: all")
            if type_name == "string":
                program.is_(f'{xs}.join(",")', lit_str(",".join(vals)), f"{desc}.join")
        for left in SHAPES_ARRAY:
            for right in SHAPES_ARRAY:
                program = batch.program()
                a, b, plus, minus = program.fresh("a"), program.fresh("b"), program.fresh("plus"), program.fresh("minus")
                lv, rv = items(left), items(right)
                kept = tuple(i for i in left if values[i % len(values)] not in rv)
                setup = [f"let {a} is @{type_name} = {array(left)};", f"let {b} is @{type_name} = {array(right)};",
                         f"let {plus} is @{type_name} = {array(left + right)};",
                         f"let {minus} is @{type_name} = {array(kept)};"]
                desc = f"@{type_name} {array(left)} and {array(right)}"
                program.ok(f"{a} + {b} == {plus}", f"{desc}: +", setup)
                program.ok(f"{a} - {b} == {minus}", f"{desc}: - removes every equal element")
                program.is_(f"{a} == {b}", lit_bool(lv == rv), f"{desc}: ==")
    for type_name in MAP_KEYS:
        _, values, literal, key = next(e for e in ELEMENTS if e[0] == type_name)

        def mapping(shape):
            return "[" + ", ".join(f"{literal(values[i])}: {i + 1}" for i in shape) + "]" if shape else "[:]"

        def ordered(shape):
            return sorted(set(shape), key=lambda i: key(values[i]))

        shapes = sorted({tuple(i for i in shape if i < len(values)) for shape in SHAPES_MAP})
        for shape in shapes:
            program = batch.program()
            m, ks, vs = program.fresh("m"), program.fresh("ks"), program.fresh("vs")
            order = ordered(shape)
            setup = [f"let {m} is %{type_name}:int = {mapping(shape)};",
                     f"let {ks} is @{type_name} = [{', '.join(literal(values[i]) for i in order)}];",
                     f"let {vs} is @int = [{', '.join(str(i + 1) for i in order)}];"]
            desc = f"%{type_name}:int {mapping(shape)}"
            program.is_(f"{m}.size()", lit_int(len(set(shape))), f"{desc}.size()", setup)
            program.ok(f"{m}.keys() == {ks}", f"{desc}.keys() in key order")
            program.ok(f"{m}.values() == {vs}", f"{desc}.values() follow the keys")
            for i, v in enumerate(values):
                program.is_(f"{m}.has({literal(v)})", lit_bool(i in shape), f"{desc}.has({literal(v)})")
                if type_name == "float":
                    program.skip(301)
                else:
                    program.is_(f"{m}[{literal(v)}] else -1", lit_int(i + 1 if i in shape else -1), f"{desc}[{literal(v)}] else")
            program.is_(f"{m}.reduce(0) -> sum, k, v {{ sum + v; }}", lit_int(sum(i + 1 for i in set(shape))), f"{desc}: reduce over values")
            program.is_(f"({m}.filter() -> k, v {{ v > 1; }}).size()", lit_int(sum(1 for i in set(shape) if i + 1 > 1)), f"{desc}: filter")
            program.ok(f"({m}.map() -> k, v {{ v; }}) == {vs}", f"{desc}: map visits keys in order")
        for left in shapes:
            for right in shapes:
                program = batch.program()
                a, b, plus, minus = program.fresh("a"), program.fresh("b"), program.fresh("plus"), program.fresh("minus")
                merged = {i: i + 1 for i in left}
                merged.update({i: i + 1 for i in right})
                remaining = tuple(i for i in left if i not in right)
                setup = [f"let {a} is %{type_name}:int = {mapping(left)};", f"let {b} is %{type_name}:int = {mapping(right)};",
                         f"let {plus} is %{type_name}:int = {mapping(tuple(merged))};",
                         f"let {minus} is %{type_name}:int = {mapping(remaining)};"]
                desc = f"%{type_name}:int {mapping(left)} and {mapping(right)}"
                program.ok(f"{a} + {b} == {plus}", f"{desc}: + (the right map wins)", setup)
                program.ok(f"{a} - {b} == {minus}", f"{desc}: - removes keys")
                program.is_(f"{a} == {b}", lit_bool(set(left) == set(right)), f"{desc}: ==")
    return batch.programs


# ----- family: constructs nested in each other ------------------------------------

# A wrapper takes the inner expression, its type, an alternative value of the
# type and a depth (for unique binding names) and returns an expression with
# the inner value.
def tail(e):
    """The last statement of a block: `match` and `try` take no `;` (#302)."""
    return e if e.startswith(("match ", "try ")) else e + ";"


WRAPPERS = {
    "paren": lambda e, t, alt, d: f"({e})",
    "if-then": lambda e, t, alt, d: f"if 1 < 2 {{ {tail(e)} }} else {{ {alt}; }}",
    "if-else": lambda e, t, alt, d: f"if 2 < 1 {{ {alt}; }} else {{ {tail(e)} }}",
    "else-if": lambda e, t, alt, d: f"if false {{ {alt}; }} else if true {{ {tail(e)} }} else {{ {alt}; }}",
    "match-int": lambda e, t, alt, d: f"match 2 {{ case 1 {{ {alt}; }} case 2, 3 {{ {tail(e)} }} else {{ {alt}; }} }}",
    "match-enum": lambda e, t, alt, d: f"match Status.done {{ case Status.ready {{ {alt}; }} case Status.done {{ {tail(e)} }} case Status.failed {{ {alt}; }} }}",
    "choice-bool": lambda e, t, alt, d: f"(true ?? {e} !! {alt})",
    "choice-maybe": lambda e, t, alt, d: f"(Maybe.some(1) ?? .some(n{d}): {e} !! .none: {alt})",
    # `last if …` is read as a postfix condition (#303): an `if` goes in parentheses.
    "loop-last": lambda e, t, alt, d: f"loop {{ last {f'({e})' if e.startswith('if ') else e}; }}",
    "closure": lambda e, t, alt, d: f"(() is {t} {{ {tail(e)} }})()",
    "closure-arg": lambda e, t, alt, d: f"((v{d} is {t}) is {t} {{ v{d}; }})({e})",
    "maybe-else": lambda e, t, alt, d: f"(Maybe.some({e}) else {alt})",
    "array": lambda e, t, alt, d: f"[{alt}, {e}][1]",
    "map": lambda e, t, alt, d: f'["k": {e}]["k"]',
    "box": lambda e, t, alt, d: f"Box(value: {e}).get()",
    "reduce": lambda e, t, alt, d: f"(0..1).reduce({alt}) -> acc{d}, i{d} {{ {tail(e)} }}",
    "map-op": lambda e, t, alt, d: f"((0..1).map() -> i{d} {{ {tail(e)} }})[0]",
    "try": lambda e, t, alt, d: f"try {{ {tail(e)} }} catch e{d} is Error {{ {alt}; }}",
    "variant": lambda e, t, alt, d: f"match Slot.full({e}) {{ case Slot.full(x{d}) {{ x{d}; }} case Slot.vacant {{ {alt}; }} }}",
    "block-let": lambda e, t, alt, d: f"if true {{ let w{d} = {e}; w{d}; }} else {{ {alt}; }}",
}

NEST_TYPES = [
    ("int", "42", "0", "is"),
    ("string", lit_str("s😀"), lit_str(""), "is"),
    ("@int", "[1, 2]", "[9]", "eq"),
    ("Point", "Point(x: 1, y: 2)", "Point(x: 0, y: 0)", "eq"),
    ("Maybe<int>", "Maybe.some(7)", "Maybe<int>.none", "eq"),
]


def nesting_family():
    import itertools
    batch = Batch("constructs nested in each other: every pair for five types, every triple for int",
                  declarations=VALUE_DECLARATIONS)
    names = list(WRAPPERS)

    def build(chain, value, type_name, alt):
        expr = value
        for depth, name in enumerate(reversed(chain)):
            expr = WRAPPERS[name](expr, type_name, alt, depth)
        return expr

    starts_with_paren = {name for name in WRAPPERS if WRAPPERS[name]("x", "int", "0", 0).startswith("(")}

    def held_back(chain):
        """The open issue a chain runs into, or None."""
        for outer, inner in zip(chain, chain[1:]):
            # A map value cannot start with `(` (#263).
            if outer == "map" and inner in starts_with_paren:
                return 263
        # A generic enum case passes T down to loop/last, else and reduce, through
        # the constructs that hand their expected type on (#304).
        passing = {"paren", "if-then", "if-else", "else-if", "match-int", "match-enum", "choice-bool",
                   "choice-maybe", "try", "loop-last", "maybe-else", "variant"}
        for start, outer in enumerate(chain):
            if outer not in ("variant", "maybe-else"):
                continue
            for inner in chain[start + 1:]:
                if inner in ("loop-last", "maybe-else", "reduce"):
                    return 304
                if inner not in passing:
                    break
        return None

    def forbidden(chain):
        # Compact choices never nest, not even through other constructs (Book §24.3).
        return sum(1 for name in chain if name.startswith("choice")) > 1

    for type_name, value, alt, compare in NEST_TYPES:
        for chain in itertools.product(names, repeat=2):
            if forbidden(chain):
                continue
            if held_back(chain):
                batch.program().skip(held_back(chain))
                continue
            expr = build(chain, value, type_name, alt)
            label = f"{type_name}: " + " > ".join(chain)
            if compare == "is":
                batch.is_(f"({expr})", value, label)
            else:
                batch.ok(f"({expr}) == {value}", label)
    for chain in itertools.product(names, repeat=3):
        if forbidden(chain):
            continue
        if held_back(chain):
            batch.program().skip(held_back(chain))
            continue
        expr = build(chain, "42", "int", "0")
        batch.is_(f"({expr})", "42", "int: " + " > ".join(chain))
    return batch.programs


# ----- family: control flow with an oracle ---------------------------------------

MATCH_CONFIGS = [
    [[0], [1, 2], [3]],
    [[5]],
    [[-1, 1], [0]],
    [[1, 2, 3, 4]],
    [[100], [7], [0, 2]],
    [[2], [3], [5], [7]],
]
SUBJECTS = [-1, 0, 1, 2, 3, 5, 7, 100]
RANGES = [(0, 5, False), (0, 5, True), (3, 3, False), (3, 3, True), (-2, 2, False), (0, 0, True), (1, 10, True)]


def control_family():
    import itertools
    batch = Batch("control flow: match, for with next and last, nested loops, loop values, early return",
                  imports=['import "cell" take Cell;'])
    # match over int case lists, with else.
    for index, config in enumerate(MATCH_CONFIGS):
        program = batch.program()
        name = program.fresh("pick")
        cases = " ".join(f"case {', '.join(lit_int(v) for v in values)} {{ {10 * (k + 1)}; }}" for k, values in enumerate(config))
        program.helpers.append(f"{name}(v is int) is int {{ match v {{ {cases} else {{ 99; }} }} }}")
        program.helpers.append(f"{name}_stmt(v is int) is int {{\n    let out = Cell(0);\n    match v {{ {' '.join(f'case {chr(44).join(lit_int(x) for x in values)} {{ out.replace({10 * (k + 1)}); }}' for k, values in enumerate(config))} else {{ out.replace(99); }} }}\n    out.value();\n}}")
        for subject in SUBJECTS:
            expected = next((10 * (k + 1) for k, values in enumerate(config) if subject in values), 99)
            program.is_(f"self.{name}({lit_int(subject)})", lit_int(expected), f"match {config} on {subject}")
            program.is_(f"self.{name}_stmt({lit_int(subject)})", lit_int(expected), f"statement match {config} on {subject}")
    # for over ranges with next/last placed before or after the work.
    variants = []
    for (a, b, inclusive) in RANGES:
        for skip in (None, (2, 0), (3, 1)):
            for stop in (None, 2, 4):
                for order in ("skip-first", "stop-first"):
                    if skip is None and stop is None and order == "stop-first":
                        continue
                    variants.append((a, b, inclusive, skip, stop, order))
    for a, b, inclusive, skip, stop, order in variants:
        program = batch.program()
        name = program.fresh("walk")
        rng = f"{lit_int(a)}..{'=' if inclusive else ''}{lit_int(b)}"
        lines = []
        skip_line = f"next if i % {skip[0]} == {skip[1]};" if skip else None
        stop_line = f"last if i == {stop};" if stop is not None else None
        guards = [skip_line, stop_line] if order == "skip-first" else [stop_line, skip_line]
        lines += [g for g in guards if g]
        body = " ".join(lines + ["acc.replace(acc.value() * 10 + i + 3);"])
        program.helpers.append(f"{name}() is int {{\n    let acc = Cell(0);\n    for {rng} -> i {{ {body} }}\n    acc.value();\n}}")
        acc = 0
        for i in range(a, b + (1 if inclusive else 0)):
            stopped = False
            for guard in ([("skip", skip), ("stop", stop)] if order == "skip-first" else [("stop", stop), ("skip", skip)]):
                kind, value = guard
                if kind == "skip" and value and trunc_mod(i, value[0]) == value[1]:
                    break
                if kind == "stop" and value is not None and i == value:
                    stopped = True
                    break
            else:
                acc = acc * 10 + i + 3
                continue
            if stopped:
                break
        program.is_(f"self.{name}()", lit_int(acc), f"for {rng}, next {skip}, last {stop}, {order}")
    # Nested loops: last leaves the inner loop only; next skips an outer turn.
    for outer, inner in itertools.product([(0, 3), (1, 4)], [(0, 3), (0, 5)]):
        for limit in (0, 1, 2):
            program = batch.program()
            name = program.fresh("nest")
            program.helpers.append(
                f"{name}() is int {{\n    let acc = Cell(0);\n    for {outer[0]}..{outer[1]} -> i {{\n        next if i == {limit};\n"
                f"        for {inner[0]}..{inner[1]} -> j {{ last if j > i; acc.replace(acc.value() + i * 10 + j); }}\n    }}\n    acc.value();\n}}")
            acc = 0
            for i in range(*outer):
                if i == limit:
                    continue
                for j in range(*inner):
                    if j > i:
                        break
                    acc += i * 10 + j
            program.is_(f"self.{name}()", lit_int(acc), f"nested loops {outer} {inner}, next at {limit}")
    # loop with a counter and `last value if`.
    for start, step, bound in [(0, 1, 5), (1, 2, 10), (10, -3, -5), (0, 7, 0)]:
        program = batch.program()
        name = program.fresh("until")
        program.helpers.append(
            f"{name}() is int {{\n    let n = Cell({lit_int(start)});\n    loop {{\n        last n.value() if {'n.value() >= ' + lit_int(bound) if step > 0 else 'n.value() <= ' + lit_int(bound)};\n"
            f"        n.replace(n.value() + {lit_int(step)});\n    }}\n}}")
        n = start
        while not (n >= bound if step > 0 else n <= bound):
            n += step
        program.is_(f"self.{name}()", lit_int(n), f"loop from {start} by {step} to {bound}")
    # Early return from inside constructs.
    for where in ("for", "match", "if", "nested"):
        program = batch.program()
        name = program.fresh("early")
        bodies = {
            "for": "for 0..10 -> i { return i * 2 if i * i > v; }\n    -1;",
            "match": "match v { case 0 { return 100; } else { } }\n    v + 1;",
            "if": "if v > 3 { return v - 3; } else if v < 0 { return 0 - v; }\n    v;",
            "nested": "for 0..5 -> i { for 0..5 -> j { return i * 10 + j if i + j == v; } }\n    -1;",
        }
        program.helpers.append(f"{name}(v is int) is int {{\n    {bodies[where]}\n}}")
        for v in (-2, 0, 3, 5, 9, 20):
            if where == "for":
                expected = next((i * 2 for i in range(10) if i * i > v), -1)
            elif where == "match":
                expected = 100 if v == 0 else v + 1
            elif where == "if":
                expected = v - 3 if v > 3 else (-v if v < 0 else v)
            else:
                expected = next((i * 10 + j for i in range(5) for j in range(5) if i + j == v), -1)
            program.is_(f"self.{name}({lit_int(v)})", lit_int(expected), f"return from {where} with {v}")
    return batch.programs


FAMILIES = {
    "values": values_family,
    "int": int_family,
    "float": float_family,
    "rat": rat_family,
    "byte": byte_family,
    "bits": bits_family,
    "precedence": precedence_family,
    "roles": roles_family,
    "strings": strings_family,
    "collections": collections_family,
    "nesting": nesting_family,
    "control": control_family,
}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", default=OUT)
    parser.add_argument("--family", action="append")
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()
    names = args.family or list(FAMILIES)
    if args.list:
        print("\n".join(FAMILIES))
        return 0
    total = 0
    for name in names:
        directory = os.path.join(args.out, name)
        shutil.rmtree(directory, ignore_errors=True)
        os.makedirs(directory)
        programs = FAMILIES[name]()
        for index, program in enumerate(programs):
            with open(os.path.join(directory, f"{index:02d}.ok"), "w", encoding="utf-8") as out:
                out.write(program.render())
            total += len(program.checks)
        print(f"{name}: {len(programs)} files, {sum(len(p.checks) for p in programs)} checks")
    print(f"total: {total} checks")
    return 0


if __name__ == "__main__":
    sys.exit(main())
