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
import re
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

# Checks the family being generated has written: the same check over the same
# setup is written once, whichever dimensions led to it.
SEEN = set()


def canonical(lines):
    """Lines with the names their `let`s bind numbered in order."""
    text = "\n".join(lines)
    for index, name in enumerate(dict.fromkeys(re.findall(r"\blet (\w+)", text))):
        text = re.sub(rf"\b{name}\b", f"_{index}", text)
    return text


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
        self.setup = []
        # The setup of a dropped duplicate, for the rest of its group.
        self.carry = []

    def group(self):
        self.group_id += 1

    def skip(self, issue):
        self.skipped[issue] = self.skipped.get(issue, 0) + 1

    def fresh(self, prefix):
        self.counter += 1
        return f"{prefix}{self.counter}"

    def add(self, key, check, setup):
        if setup:
            self.group()
            self.setup, self.carry = list(setup), []
        key = canonical(self.setup + [key])
        if key in SEEN:
            self.carry += list(setup)
            return
        SEEN.add(key)
        self.checks.append((self.carry + list(setup), check, self.group_id))
        self.carry = []

    def is_(self, got, expected, name, setup=()):
        self.add(f"is {got} {expected}", f't = t.is({got}, {expected}, "{name_of(name)}");', setup)

    def ok(self, condition, name, setup=()):
        self.add(f"ok {condition}", f't = t.ok({condition}, "{name_of(name)}");', setup)

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


# ----- family: values through syntactic contexts ---------------------------------

# One representative per class of every type; `other` gives a second value of
# the same type for contexts that choose between two.
VALUE_TYPES = [
    # (type, literals, comparison, map key, constant, needs)
    ("int", [lit_int(0), lit_int(-1), lit_int(2 ** 63 - 1), lit_int(2 ** 63), lit_int(-(10 ** 30))], "is", True, True),
    ("float", [lit_float(0.5), lit_float(-1e-05), lit_float(1e16), lit_float(5e-324)], "is", False, True),
    ("rat", [lit_rat(fractions.Fraction(1, 3)), lit_rat(fractions.Fraction(-7, 2)), lit_rat(fractions.Fraction(10 ** 20, 3))], "is", True, True),
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
            batch.is_(f"{receiver(L)}.less({Rr})", lit_bool(left < right), f"{label}: less by code point")
            batch.is_(f"{receiver(L)}.less_or_equal({Rr})", lit_bool(left <= right), f"{label}: less_or_equal by code point")
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
MAP_KEYS = ["int", "string", "bool", "char", "Status", "float", "rat", "Point"]
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


# ----- family: arrays built step by step (0.4.3, #246–#251) -------------------------

# Arrays grown one append at a time take the engines' persistent paths (shared
# buffers, tries) where one-shot literals and builders do not. Sizes sit on
# both sides of every boundary those representations have: the chain
# threshold (32), buffer capacities (powers of two) and trie levels (32,
# 32 + 32², …).
PERSISTENCE_SIZES = [0, 1, 2, 31, 32, 33, 34, 63, 64, 65, 66, 100, 127, 128, 129, 1023, 1024, 1025, 1056, 1057, 2049]

PERSISTENCE_DECLARATIONS = """object Grow {
    Grow();
    chain(i is int, n is int, items is @int) is @int {
        if i == n { return items; }
        self.chain(i + 1, n, items + [i * 7 % 101]);
    }
    pairs(i is int, n is int, items is @int) is @int {
        if i == n { return items; }
        if i + 1 == n { return items + [i * 7 % 101]; }
        self.pairs(i + 2, n, items + [i * 7 % 101, (i + 1) * 7 % 101]);
    }
    front(i is int, n is int, items is @int) is @int {
        if i == n { return items; }
        self.front(i + 1, n, [i * 7 % 101] + items);
    }
    alternate(i is int, n is int, items is @int) is @int {
        if i == n { return items; }
        let grown = if i % 2 == 0 { items + [i * 7 % 101]; } else { [i * 7 % 101] + items; };
        self.alternate(i + 1, n, grown);
    }
    log(i is int, n is int, log is Log) is Log {
        if i == n { return log; }
        self.log(i + 1, n, log.add(i * 7 % 101));
    }
    texts(i is int, n is int, items is @string) is @string {
        if i == n { return items; }
        self.texts(i + 1, n, items + [(i * 7 % 101).string()]);
    }
    rows(i is int, n is int, items is @@int) is @@int {
        if i == n { return items; }
        self.rows(i + 1, n, items + [[i, i * 7 % 101]]);
    }
}

object Log {
    has items is @int;
    Log(:items);
    add(x is int) is ^Log { ^items = items + [x]; }
    items() is @int { items; }
}""".splitlines()


def persistence_family():
    batch = Batch("arrays built one append at a time", imports=['import "cell" take Cell;'],
                  declarations=PERSISTENCE_DECLARATIONS)

    def v(i):
        return i * 7 % 101

    def arr(values):
        return "[" + ", ".join(str(x) for x in values) + "]" if values else "@int()"

    for n in PERSISTENCE_SIZES:
        model = [v(i) for i in range(n)]
        builds = {
            "a chain of items + [x]": f"Grow().chain(0, {n}, @int())",
            "a chain of items + [x, y]": f"Grow().pairs(0, {n}, @int())",
            "a successor field": f"Grow().log(0, {n}, Log(items: @int())).items()",
        }
        for how, build in list(builds.items()) + [("a cell", None)]:
            program = batch.program()
            xs, ex = program.fresh("xs"), program.fresh("ex")
            if build is None:
                c = program.fresh("cell")
                setup = [f"let {c} = Cell(@int());",
                         f"for 0..{n} -> i {{ {c}.replace({c}.value() + [i * 7 % 101]); }}",
                         f"let {xs} = {c}.value();"]
            else:
                setup = [f"let {xs} = {build};"]
            setup.append(f"let {ex} is @int = (0..{n}).map() -> i {{ i * 7 % 101; }};")
            # Two versions appended to the same one, and each extended again:
            # every version keeps its own elements. One setup for the whole
            # group, so that no part of the file loses it.
            va, vb, vc, vd = (program.fresh(p) for p in "abcd")
            setup += [f"let {va} = {xs} + [1000];", f"let {vb} = {xs} + [2000];",
                      f"let {vc} = {va} + [3000];", f"let {vd} = {vb} + [4000, 5000];"]
            desc = f"{n} elements by {how}"
            program.ok(f"{xs} == {ex}", f"{desc} equal one map()", setup)
            program.is_(f"{xs}.size()", lit_int(n), f"{desc}: size")
            program.is_(f"{xs}.reduce(0) -> a, x {{ a + x; }}", lit_int(sum(model)), f"{desc}: reduce")
            for i in sorted({0, 1, 30, 31, 32, 33, n // 2, n - 2, n - 1}):
                if 0 <= i < n:
                    program.is_(f"{xs}[{i}]", lit_int(model[i]), f"{desc}: [{i}]")
            program.ok(f"({xs}[{n}] else -1) == -1", f"{desc}: [{n}] is out of bounds")
            for a, b in sorted({(0, n), (0, min(n, 33)), (min(n, 31), n), (n // 3, n - n // 3)}):
                if 0 <= a <= b <= n:
                    program.ok(f"{xs}.slice({a}, {b}) == {ex}.slice({a}, {b})", f"{desc}: slice({a}, {b})")
            for k in sorted({0, 1, 32, n // 2, n}):
                program.ok(f"{xs}.take({k}) == {ex}.take({k})", f"{desc}: take({k})")
                program.ok(f"{xs}.drop({k}) == {ex}.drop({k})", f"{desc}: drop({k})")
            program.ok(f"{xs}.reversed() == {ex}.reversed()", f"{desc}: reversed")
            program.ok(f"({xs}.map() -> x {{ x + 1; }}) == ({ex}.map() -> x {{ x + 1; }})", f"{desc}: map")
            program.is_(f"({xs}.filter() -> x {{ x % 2 == 0; }}).size()", lit_int(sum(1 for x in model if x % 2 == 0)),
                        f"{desc}: filter")
            if n:
                last = model[-1]
                program.is_(f"{xs}.index_of({last}) else -1", lit_int(model.index(last)), f"{desc}: index_of the last")
                program.ok(f"{xs}.contains({last})", f"{desc}: contains the last")
            program.ok(f"!{xs}.contains(1000)", f"{desc}: does not contain 1000")
            program.ok(f"{va} == {ex} + [1000] && {vb} == {ex} + [2000]", f"{desc}: two appends to one version")
            program.ok(f"{vc} == {ex} + [1000, 3000] && {vd} == {ex} + [2000, 4000, 5000]", f"{desc}: each extended again")
            program.ok(f"{xs} == {ex} && {xs}.size() == {n}", f"{desc}: the origin is unchanged")
            program.is_(f"{va}.size() + {vb}.size() + {vc}.size() + {vd}.size()", lit_int(4 * n + 7), f"{desc}: sizes")
            program.ok(f"{xs} + {xs} == {ex} + {ex}", f"{desc}: xs + xs")
            program.ok(f"{va} + {vb} == {ex} + [1000] + {ex} + [2000]", f"{desc}: two versions joined")

        # Other shapes of chains.
        program = batch.program()
        desc = f"{n} elements"
        program.ok(f"Grow().front(0, {n}, @int()) == {arr(list(reversed(model)))}", f"{desc} by [x] + items")
        # Prepends (#247): branches from one version, a version extended at
        # both ends, and the origin unchanged after all of them.
        alternated = []
        for i in range(n):
            alternated = alternated + [v(i)] if i % 2 == 0 else [v(i)] + alternated
        p, pa, pb, pc, both = (program.fresh(x) for x in ("p", "pa", "pb", "pc", "both"))
        rev = list(reversed(model))
        prepends = [f"let {p} = Grow().front(0, {n}, @int());", f"let {pa} = [1000] + {p};", f"let {pb} = [2000, 2001] + {p};",
                    f"let {pc} = [3000] + {pa};", f"let {both} = [4000] + {p} + [4001];"]
        program.ok(f"{pa} == {arr([1000] + rev)} && {pb} == {arr([2000, 2001] + rev)}", f"{desc}: two prepends to one version", prepends)
        program.ok(f"{pc} == {arr([3000, 1000] + rev)}", f"{desc}: a prepended version prepended again")
        program.ok(f"{both} == {arr([4000] + rev + [4001])}", f"{desc}: both ends at once")
        program.ok(f"{p} == {arr(rev)}", f"{desc}: the origin is unchanged")
        program.ok(f"Grow().alternate(0, {n}, @int()) == {arr(alternated)}", f"{desc} by alternating ends")
        texts = [str(x) for x in model]
        program.is_(f"Grow().texts(0, {n}, @string()).join(\",\")", lit_str(",".join(texts)), f"{desc}: strings by a chain")
        rows = program.fresh("rows")
        program.is_(f"{rows}.size()", lit_int(n), f"{desc}: rows by a chain", [f"let {rows} = Grow().rows(0, {n}, [[0, 0]].take(0));"])
        if n:
            program.ok(f"{rows}[{n - 1}] == [{n - 1}, {model[-1]}]", f"{desc}: the last row")
            program.ok(f"{rows}[0] == [0, 0]", f"{desc}: the first row")
    # Chains of replacements (#250): from the second one on, a large array
    # is a trie on engines that have them; every version keeps its elements.
    for n in [1, 2, 63, 64, 65, 100, 1056, 1057, 2049, 33000]:
        program = batch.program()
        model = [v(i) for i in range(n)]
        spots = sorted({0, n - 1, n // 2, min(31, n - 1), min(32, n - 1), max(n - 33, 0)})
        ex = program.fresh("ex")
        setup = [f"let {ex} is @int = (0..{n}).map() -> i {{ i * 7 % 101; }};"]
        names, models = [ex], [model]
        for step, spot in enumerate(spots):
            name = program.fresh("r")
            setup.append(f"let {name} = {names[-1]}.replaced({spot}, {1000 + step});")
            changed = list(models[-1])
            changed[spot] = 1000 + step
            names.append(name)
            models.append(changed)
        branch = program.fresh("b")
        setup.append(f"let {branch} = {names[1]}.replaced({n - 1}, 7777);")
        branched = list(models[1])
        branched[n - 1] = 7777
        last, final = names[-1], models[-1]
        desc = f"{n} elements after {len(spots)} replacements"

        def same(name, values, what):
            program.is_(f"{name}.reduce(0) -> a, x {{ a + x; }}", lit_int(sum(values)), f"{desc}: {what}, reduce")
            for i in sorted({0, n // 2, n - 1}):
                program.is_(f"{name}[{i}]", lit_int(values[i]), f"{desc}: {what}, [{i}]")

        program.is_(f"{last}.size()", lit_int(n), f"{desc}: size", setup)
        for name, values, what in zip(names, models, ["the origin"] + [f"version {k}" for k in range(1, len(names))]):
            same(name, values, what)
        same(branch, branched, "a branch from version 1")
        for i in spots:
            program.is_(f"{last}[{i}]", lit_int(final[i]), f"{desc}: [{i}]")
        for a, b in sorted({(0, n), (0, min(n, 33)), (n // 3, n - n // 3)}):
            program.is_(f"{last}.slice({a}, {b}).reduce(0) -> a, x {{ a + x; }}", lit_int(sum(final[a:b])), f"{desc}: slice({a}, {b})")
        program.is_(f"{last}.take(32).size() + {last}.drop(32).size()", lit_int(n), f"{desc}: take and drop")
        program.is_(f"{last}.reversed()[0]", lit_int(final[-1]), f"{desc}: reversed")
        program.ok(f"{last}.contains({1000 + len(spots) - 1})", f"{desc}: contains the last replacement")
        program.is_(f"{last}.index_of({final[n // 2]}) else -1", lit_int(final.index(final[n // 2])), f"{desc}: index_of")
        program.is_(f"({last}.map() -> x {{ x + 1; }}).reduce(0) -> a, x {{ a + x; }}", lit_int(sum(final) + n), f"{desc}: map")
        program.is_(f"({last} + [5, 6]).reduce(0) -> a, x {{ a + x; }}", lit_int(sum(final) + 11), f"{desc}: appended to")
        program.ok(f"{last} == {ex}.map() -> x {{ x; }}.replaced(0, {final[0]}).replaced({n - 1}, {final[n - 1]}).replaced({n // 2}, {final[n // 2]}).replaced({min(31, n - 1)}, {final[min(31, n - 1)]}).replaced({min(32, n - 1)}, {final[min(32, n - 1)]}).replaced({max(n - 33, 0)}, {final[max(n - 33, 0)]})",
                   f"{desc}: equal to the same replacements made again")
    return batch.programs


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
                    # The order of the guards matters only when there are both.
                    if (skip is None or stop is None) and order == "stop-first":
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


# ----- family: nested data structures over leaves of every kind -----------------

STRUCT_DECLARATIONS = """role Shape {
    area() is int;
}

object Square does Shape {
    has side is int;
    Square(:side);
    area() is int { side * side; }
}

object Point {
    has x is int;
    has y is int;
    Point(:x, :y);
}

object Box<T> {
    has value is T;
    Box(:value);
    get() is T { value; }
}

enum Slot<T> = (vacant, full(item is T));""".splitlines()

# A leaf: its type, a literal, another literal of the type, and how to check
# the leaf an access reaches.
STRUCT_LEAVES = [
    ("int", "7", "8", lambda e, v: ("is", e, v)),
    ("string", lit_str("s😀"), lit_str("t"), lambda e, v: ("is", e, v)),
    ("Point", "Point(x: 1, y: -2)", "Point(x: 0, y: 0)", lambda e, v: ("ok", f"{e} == {v}")),
    ("Shape", "Square(side: 3)", "Square(side: 4)", lambda e, v: ("is", f"{e}.area()", "9" if v == "Square(side: 3)" else "16")),
]
# A container: how its type, literal and access are written around the inner ones.
STRUCT_CONTAINERS = {
    "array": (lambda t: f"@{t}", lambda inner: f"[{inner}]", lambda e: f"{e}[0]"),
    "map": (lambda t: f"%string:{t}", lambda inner: f'["k": {inner}]', lambda e: f'{e}["k"]'),
    "maybe": (lambda t: f"Maybe<{t}>", lambda inner: f"Maybe.some({inner})",
              lambda e: f'(match {e} {{ case Maybe.some(x) {{ x; }} case Maybe.none {{ panic "none"; }} }})'),
    "box": (lambda t: f"Box<{t}>", lambda inner: f"Box(value: {inner})", lambda e: f"{e}.get()"),
    "slot": (lambda t: f"Slot<{t}>", lambda inner: f"Slot.full({inner})",
             lambda e: f'(match {e} {{ case Slot.full(x) {{ x; }} case Slot.vacant {{ panic "vacant"; }} }})'),
}


def structures_family():
    import itertools
    batch = Batch("nested data structures of depth one to three over int, string, object and role leaves",
                  declarations=STRUCT_DECLARATIONS)
    names = list(STRUCT_CONTAINERS)
    for depth in (1, 2, 3):
        for chain in itertools.product(names, repeat=depth):
            for leaf_type, leaf, other, check in STRUCT_LEAVES:
                type_name, literal, changed = leaf_type, leaf, other
                for container in reversed(chain):
                    wrap_type, wrap_literal, _ = STRUCT_CONTAINERS[container]
                    type_name = wrap_type(type_name)
                    literal = wrap_literal(literal)
                    changed = wrap_literal(changed)
                # The access goes from the outside in.
                expr = "{v}"
                for container in chain:
                    expr = STRUCT_CONTAINERS[container][2](expr)
                program = batch.program()
                v, same, diff = program.fresh("v"), program.fresh("same"), program.fresh("diff")
                setup = [f"let {v} is {type_name} = {literal};", f"let {same} is {type_name} = {literal};",
                         f"let {diff} is {type_name} = {changed};"]
                kind = check(expr.replace("{v}", v), leaf)
                label = f"{type_name}"
                if kind[0] == "is":
                    program.is_(kind[1], kind[2], f"{label}: the leaf", setup)
                else:
                    program.ok(kind[1], f"{label}: the leaf", setup)
                program.ok(f"{v} == {same}", f"{label}: equal to the same structure")
                program.ok(f"{v} != {diff}", f"{label}: a different leaf")
    return batch.programs


# ----- family: value objects as map keys --------------------------------------------

KEY_DECLARATIONS = """enum Level = (low, mid, high);

object KeyA {
    has a is int;
    has b is string;
    KeyA(:a, :b);
}

object KeyB {
    has r is rat;
    has c is char;
    KeyB(:r, :c);
}

object KeyC {
    has level is Level;
    has inner is KeyA;
    KeyC(:level, :inner);
}

object KeyD {
    has small is byte;
    has flag is bool;
    KeyD(:small, :flag);
}

object KeyE {
    has f is float;
    KeyE(:f);
}""".splitlines()

LEVELS = ["low", "mid", "high"]
# (type, values as Python tuples, literal, sort key)
KEY_OBJECTS = [
    ("KeyA", [(2, "b"), (1, "z"), (2, "a")], lambda v: f"KeyA(a: {lit_int(v[0])}, b: {lit_str(v[1])})", lambda v: v),
    ("KeyB", [(F(1, 2), "x"), (F(-1, 3), "😀"), (F(1, 2), "A")], lambda v: f"KeyB(r: {lit_rat(v[0])}, c: {lit_char(v[1])})",
     lambda v: (v[0], ord(v[1]))),
    ("KeyC", [("high", (1, "a")), ("low", (9, "q")), ("high", (0, "z"))],
     lambda v: f"KeyC(level: Level.{v[0]}, inner: KeyA(a: {lit_int(v[1][0])}, b: {lit_str(v[1][1])}))",
     lambda v: (LEVELS.index(v[0]), v[1])),
    ("KeyD", [(255, False), (0, True), (255, True)], lambda v: f"KeyD(small: byte({v[0]}), flag: {lit_bool(v[1])})", lambda v: v),
    ("KeyE", [(2.5,), (-1.0,), (1e16,)], lambda v: f"KeyE(f: {lit_float(v[0])})", lambda v: v),
]


def keys_family():
    import itertools
    batch = Batch("value objects as map keys: key order, lookups with equal objects, + and -", declarations=KEY_DECLARATIONS)
    for type_name, values, literal, key in KEY_OBJECTS:
        shapes = [(), (0,), (1, 0), (0, 1, 2), (2, 0, 1)]
        for shape in shapes:
            program = batch.program()
            m, ks = program.fresh("m"), program.fresh("ks")
            order = sorted(set(shape), key=lambda i: key(values[i]))
            entries = ", ".join(f"{literal(values[i])}: {i + 1}" for i in shape) if shape else ":"
            setup = [f"let {m} is %{type_name}:int = [{entries}];",
                     f"let {ks} is @{type_name} = [{', '.join(literal(values[i]) for i in order)}];"]
            desc = f"%{type_name}:int of {len(shape)}"
            program.ok(f"{m}.keys() == {ks}", f"{desc}: keys in field order", setup)
            for i, v in enumerate(values):
                program.is_(f"{m}[{literal(v)}] else -1", lit_int(i + 1 if i in shape else -1), f"{desc}: lookup {literal(v)}")
                program.is_(f"{m}.has({literal(v)})", lit_bool(i in shape), f"{desc}: has {literal(v)}")
        for left, right in itertools.product([(0,), (1, 2), (0, 1, 2)], repeat=2):
            program = batch.program()
            a, b, plus, minus = program.fresh("a"), program.fresh("b"), program.fresh("plus"), program.fresh("minus")
            def mapping(sh):
                return "[" + (", ".join(f"{literal(values[i])}: {i + 1}" for i in sh) if sh else ":") + "]"
            merged = tuple(dict.fromkeys(left + right))
            kept = tuple(i for i in left if i not in right)
            setup = [f"let {a} is %{type_name}:int = {mapping(left)};", f"let {b} is %{type_name}:int = {mapping(right)};",
                     f"let {plus} is %{type_name}:int = {mapping(merged)};", f"let {minus} is %{type_name}:int = {mapping(kept)};"]
            desc = f"%{type_name}:int {left} and {right}"
            program.ok(f"{a} + {b} == {plus}", f"{desc}: +", setup)
            program.ok(f"{a} - {b} == {minus}", f"{desc}: -")
    return batch.programs


# ----- family: casts --------------------------------------------------------------

F = fractions.Fraction
INT64 = (-(2 ** 63), 2 ** 63 - 1)

# (source type, value, the targets it is a class for; None: every target).
CAST_SOURCES = [
    ("int", 0, None), ("int", -1, None), ("int", 255, None), ("int", 256, None),
    ("int", 2 ** 53 + 1, None), ("int", INT64[1], None), ("int", INT64[0], None), ("int", 2 ** 63, None),
    ("int", 65, {"char"}), ("int", 0xD7FF, {"char"}), ("int", 0xE000, {"char"}),
    ("int", 0x1F600, {"char"}), ("int", 0x10FFFF, {"char"}),
    ("byte", 0, None), ("byte", 255, None),
    ("float", 0.0, None), ("float", -0.0, None), ("float", -3.9, None), ("float", 0.5, None), ("float", -0.5, None),
    ("float", 255.9, None), ("float", 1e-05, None), ("float", 1e16, None), ("float", 5e-324, None),
    ("float", 1.7976931348623157e308, None), ("float", -9.223372036854776e18, None), ("float", 9.223372036854776e18, None),
    ("rat", F(7, 2), None), ("rat", F(-7, 2), None), ("rat", F(1, 3), None), ("rat", F(-1, 2), None),
    ("rat", F(511, 2), None), ("rat", F(10 ** 20, 3), None), ("rat", F(10 ** 30 + 1, 3 * 10 ** 29), None),
    ("rat", F(10 ** 400 + 1, 10 ** 400), None),
    ("bool", True, None), ("bool", False, None),
    ("char", "x", None), ("char", "😀", None), ("char", "'", None), ("char", "\n", None),
    ("string", "", None), ("string", "héllo", None), ("string", "😀\n\"q\"", None),
]

# `x.T()` exists for these pairs (checked against the compiler).
CAST_METHODS = {
    "int": {"int", "float", "byte", "string", "rat", "char"},
    "byte": {"int", "float", "byte", "string", "rat"},
    "float": {"int", "float", "byte", "string", "rat"},
    "rat": {"int", "float", "byte", "string", "rat"},
    "bool": {"string"}, "char": {"string"}, "string": set(),
}
# A second operand of another type for `@T(other, x)`.
CAST_OTHER = {"int": "byte(1)", "float": "1", "byte": "1.5", "string": "true"}


def cast_literal(source, value):
    if source == "int":
        return lit_int(value)
    if source == "byte":
        return f"byte({value})"
    if source == "float":
        return lit_float(value)
    if source == "rat":
        return lit_rat(value)
    if source == "bool":
        return lit_bool(value)
    if source == "char":
        return cast_char(value)
    return lit_str(value)


def cast_char(c):
    # Code points that are not printable go as `\u{...}`.
    return f"'\\u{{{ord(c):x}}}'" if ord(c) in (0, 0xD7FF, 0xE000, 0x10FFFF) else lit_char(c)


class Held:
    """A check held back until an issue is settled."""

    def __init__(self, issue):
        self.issue = issue


def cast(source, value, target):
    """The value of `target(value)`, None where the cast fails, or Held."""
    if target == "string":
        return {"int": str, "byte": str, "float": text_float, "rat": text_rat,
                "bool": lambda b: "true" if b else "false"}.get(source, lambda v: v)(value)
    if target == "char":
        if source == "char":
            return value
        if source == "int" and 0 <= value <= 0x10FFFF and not 0xD800 <= value <= 0xDFFF:
            return chr(value)
        return None
    if source in ("bool", "char", "string"):
        return None
    if target == "float":
        try:
            return float(value)
        except OverflowError:
            return None
    if target == "rat":
        # `float.rat()` takes the canonical decimal text, not the binary fraction.
        return F(repr(value)) if source == "float" else F(value)
    exact = F(repr(value)) if source == "float" else F(value)
    whole = int(exact)  # truncates toward zero
    if target == "byte":
        return whole if 0 <= whole <= 255 else None
    # `int` is arbitrary-precision: only a float source has a range (#309).
    if source == "float" and not INT64[0] <= whole <= INT64[1]:
        return None
    return whole


def cast_expected(target, result):
    """The checked expression around a cast result and its expected value."""
    if target == "float":
        # The text tells -0.0 from 0.0.
        return (lambda got: f"string({got})"), lit_str(text_float(result))
    if target == "byte":
        return (lambda got: got), f"byte({result})"
    if target == "rat":
        return (lambda got: got), lit_rat(result)
    if target == "char":
        return (lambda got: got), cast_char(result)
    if target == "string":
        return (lambda got: got), lit_str(result)
    return (lambda got: got), lit_int(result)


def casts_family():
    batch = Batch("casts: every source class into every target, as T(x), x.T(), @T(...) and a spread, from a literal, "
                  "a variable and a generic call",
                  declarations="""object Box<T> {
    has value is T;
    Box(:value);
    get() is T { value; }
}""".splitlines())
    for source, value, targets in CAST_SOURCES:
        for target in ["int", "float", "byte", "string", "char", "rat"]:
            if targets is not None and target not in targets:
                continue
            result = cast(source, value, target)
            if result is None:
                continue
            literal = cast_literal(source, value)
            forms = []
            if target != "rat":
                forms.append(("T(x)", lambda x: f"{target}({x})"))
            if target in CAST_METHODS[source]:
                forms.append(("x.T()", lambda x: f"{receiver(x)}.{target}()"))
            if target in CAST_OTHER:
                forms.append(("@T(x)", lambda x: f"@{target}({x})[0]"))
                forms.append(("@T(other, x)", lambda x: f"@{target}({CAST_OTHER[target]}, {x})[1]"))
            for context in ["literal", "variable", "generic call"]:
                program = batch.program()
                if isinstance(result, Held):
                    for _ in forms:
                        program.skip(result.issue)
                    continue
                wrap, expected = cast_expected(target, result)
                setup = []
                if context == "literal":
                    operand = literal
                elif context == "variable":
                    operand = program.fresh("v")
                    setup.append(f"let {operand} is {source} = {literal};")
                else:
                    operand = f"Box(value: {literal}).get()"
                for form, make in forms:
                    program.is_(wrap(make(operand)), expected, f"{form} {source} {literal} -> {target} ({context})", setup)
                    setup = []
                if target in CAST_OTHER and context == "variable":
                    xs = program.fresh("xs")
                    program.is_(wrap(f"@{target}(1.5, {xs}..., {xs}...)[2]") if target != "string" else
                                f"@{target}(true, {xs}..., {xs}...)[2]", expected,
                                f"@T(other, xs..., xs...) {source} {literal} -> {target}",
                                [f"let {xs} is @{source} = [{literal}];"])
    return batch.programs


# ----- family: lexical spellings ---------------------------------------------------

# Every spelling of a value the lexer accepts (Book 3.2, 5.1, 5.4), canonical first.
INT_SPELLINGS = {
    0: ["0", "0x0", "0X0", "0b0", "0B0", "000", "0_0"],
    31: ["31", "0x1F", "0x1f", "0X1F", "0b11111", "0B1_1111", "3_1", "031"],
    65535: ["65535", "0xFFFF", "0xFF_FF", "0b1111_1111_1111_1111", "65_535"],
    2 ** 63 - 1: ["9223372036854775807", "0x7FFF_FFFF_FFFF_FFFF", "0x7fffffffffffffff", "0b" + "1" * 63,
                  "9_223_372_036_854_775_807"],
    # Past the word only decimal spellings exist: hex and binary stop at INT64_MAX.
    2 ** 63: ["9223372036854775808", "9_223_372_036_854_775_808"],
    10 ** 30: ["1000000000000000000000000000000", "1_000_000_000_000_000_000_000_000_000_000"],
}
FLOAT_SPELLINGS = {
    100000.0: ["100000.0", "1e5", "1E5", "1e+5", "1E+05", "1e05", "1.0e5", "0.1e6", "100_000.0", "1_0e4"],
    1e13: ["10000000000000.0", "1e13", "1_000e1_0"],
    0.0015: ["0.0015", "1.5e-3", "1.5E-3", "15e-4", "0.001_5"],
    0.0: ["0.0", "0e0", "0.0e0", "0e-5", "0E+5", "0.000"],
    0.1: ["0.1", "1e-1", "0.10", "0.01e1"],
    5e-324: ["5e-324", "4.9e-324", "0.5e-323", "3e-324"],
    1.7976931348623157e308: ["1.7976931348623157e308", "1.7976931348623157E+308", "179769313486231570e291"],
}
STRING_SPELLINGS = {
    "A": ['"A"', '"\\u{41}"', '"\\u{041}"', '"\\u{000041}"', "```A```"],
    "é": ['"é"', '"\\u{e9}"', '"\\u{E9}"', "```é```"],
    "😀": ['"😀"', '"\\u{1F600}"', '"\\u{1f600}"', "```😀```"],
    "\n": ['"\\n"', '"\\u{a}"', "```\n```"],
    "\t": ['"\\t"', '"\\u{9}"', "```\t```"],
    "\t\r": ['"\\t\\r"', '"\\u{9}\\u{D}"'],
    # A fenced raw string drops its CRs (#310).
    "\r\n": ['"\\r\\n"', '"\\u{d}\\u{a}"', "```\r\n```"],
    "\\": ['"\\\\"', '"\\u{5c}"', "```\\```"],
    '"': ['"\\""', '"\\u{22}"', '```"```'],
    "'": ['"\'"', '"\\u{27}"', "```'```"],
    "": ['""', "``````"],
    "\x00": ['"\\u{0}"', '"\\u{00}"'],
    "\U0010FFFF": ['"\\u{10FFFF}"', '"\\u{10ffff}"'],
    "/* x */ // y": ['"/* x */ // y"', "```/* x */ // y```"],
    "a`b": ['"a`b"', "```a`b```"],
    "\\n is two": ['"\\\\n is two"', "```\\n is two```"],
    "line1\n  line2\n": ['"line1\\n  line2\\n"', "```line1\n  line2\n```"],
}
CHAR_SPELLINGS = {
    "A": ["'A'", "'\\u{41}'"],
    "'": ["'\\''", "'\\u{27}'"],
    '"': ["'\"'", "'\\u{22}'"],
    "\\": ["'\\\\'", "'\\u{5C}'"],
    "\n": ["'\\n'", "'\\u{a}'"],
    "\t": ["'\\t'", "'\\u{9}'", "'\t'"],
    "😀": ["'😀'", "'\\u{1F600}'"],
    "\x00": ["'\\u{0}'", "'\\u{000}'"],
}


def spelling_contexts(kind, value, canonical):
    """(context name, expression, expected) for one spelling `{s}`."""
    contexts = [
        ("bare", "{s}", canonical),
        ("array element, trailing comma", "[{s},][0]", canonical),
        ("between comments", "/* a */ {s} /* b */", canonical),
        ("generic argument", "Box(value: {s}).get()", canonical),
        ("module constant", "{const}", canonical),
    ]
    if kind in ("int", "string", "char"):
        contexts.append(("map key", f"[{{s}}: 1][{canonical}] else 0", "1"))
    if kind in ("int", "float"):
        contexts.append(("map value", '["k": {s}]["k"] else ' + canonical, canonical))
        contexts.append(("receiver", "{s}.string()",
                         lit_str(str(value) if kind == "int" else text_float(value))))
        contexts.append(("unary minus", "-{s}", lit_int(-value) if kind == "int" else lit_float(-value)))
        contexts.append(("array cast", f"@{kind}({{s}})[0]", canonical))
    if kind == "string":
        contexts.append(("receiver", "{s}.length()", lit_int(len(value))))
        contexts.append(("concatenation", "{s} + {s}", lit_str(value + value)))
        contexts.append(("array cast", "@string({s})[0]", canonical))
    if kind == "char":
        contexts.append(("receiver", "{s}.code()", lit_int(ord(value))))
        contexts.append(("string cast", "string({s})", lit_str(value)))
    return contexts


# Expressions as tokens, with their values; the layout family joins the
# tokens with every kind of separator.
LAYOUT_EXPRESSIONS = [
    (["(", "1", "+", "2", ")", "*", "3"], "9"),
    (["-", "2", "-", "-", "3"], "1"),
    (["[", "1", ",", "2", ",", "]", ".", "size", "(", ")"], "2"),
    (["Box", "(", "value", ":", "7", ")", ".", "get", "(", ")"], "7"),
    (["(", "if", "1", "<", "2", "{", "10", ";", "}", "else", "{", "20", ";", "}", ")"], "10"),
    (['"a"', "+", '"b"'], '"ab"'),
    (["[", '"k"', ":", "1", "]", "[", '"k"', "]", "else", "0"], "1"),
    (["(", "match", "2", "{", "case", "1", "{", "5", ";", "}", "case", "2", "{", "6", ";", "}",
      "else", "{", "7", ";", "}", "}", ")"], "6"),
    (["(", "[", "3", ",", "1", "]", ".", "map", "(", ")", "->", "v", "{", "v", "*", "2", ";", "}", ")",
      "[", "1", "]"], "2"),
    (["(", "1", "<<", "3", ")", "|", "1"], "9"),
    (["0x1F", "&", "~", "0b1"], "30"),
    (["1.5e1", "+", "0.5"], "15.5"),
    (["'a'", ".", "code", "(", ")"], "97"),
    (["true", "&&", "!", "false"], "true"),
    (["rat", "(", "1", ",", "3", ")", "+", "rat", "(", "2", ",", "3", ")", "==", "1", ".", "rat", "(", ")"], "true"),
    (["```a```", "+", "```b```"], '"ab"'),
]
LAYOUT_SEPARATORS = [
    ("nothing", ""), ("a space", " "), ("a newline", "\n"), ("a tab", "\t"), ("CRLF", "\r\n"),
    ("a block comment", "/* c */"), ("an empty block comment", "/**/"), ("a line comment", "// c\n"),
    ("blank lines", "\n\n  \n"),
]


def layout(tokens, separator):
    out = tokens[0]
    for token in tokens[1:]:
        glue = separator
        # Words and numbers do not touch: `else0` is one name.
        if not glue and (out[-1].isalnum() or out[-1] in "_'\"") and (token[0].isalnum() or token[0] in "_'\""):
            glue = " "
        out += glue + token
    return out


# Binding names: uppercase starts only where Book 3.1 allows them (let, for, catch).
BINDING_NAMES = ["USER_NAME", "A", "Ab", "_x", "_Upper", "aB", "x1_2", "_", "__", "x__y",
                 "iffy", "letter", "matches", "format", "loops", "nexts", "lasts", "trying", "catcher",
                 "returned", "withdrawal", "is_x", "isa_x", "async_x", "awaits", "spawned", "deferred",
                 "constant", "hidden_x", "it", "self_x", "selfie", "true_x", "nonex", "nullable", "var_x",
                 "casey", "elsewhere", "forth", "throwable", "panicky", "confirmed", "objects", "enums",
                 "roles", "imported", "takes", "modules", "has_x", "anyway", "rats", "bytes", "strings"]
BINDING_POSITIONS = [
    # (position, upper-case names allowed, helper body with {n}, value)
    ("let", True, "let {n} = 7;\n{n} + 1;", 8),
    ("for", True, "let acc = Cell(0);\nfor 3..5 -> {n} {{ acc.replace(acc.value() + {n}); }}\nacc.value();", 7),
    ("catch", True, "try {{ self.fail(); }} catch {n} is Oops {{ if {n} == Oops.bad(4) {{ 9; }} else {{ 0; }} }}", 9),
    ("closure parameter", False, "let m = [1, 2].map() -> {n} {{ {n} * 2; }};\nm[1];", 4),
    ("reduce accumulator", False, "(1..=3).reduce(0) -> {n}, x {{ {n} + x; }};", 6),
    ("reduce item", False, "(1..=3).reduce(0) -> acc, {n} {{ acc + {n}; }};", 6),
    ("case binding", False, "match Held.full(5) {{ case Held.full({n}) {{ {n} + 1; }} else {{ 0; }} }}", 6),
]


def lexical_family():
    batch = Batch("lexical spellings: literals through contexts, token separators, binding names",
                  imports=['import "cell" take Cell;'],
                  declarations="""enum Oops is Error = (bad(n is int));
enum Held = (empty, full(n is int));

object Box<T> {
    has value is T;
    Box(:value);
    get() is T { value; }
}""".splitlines())
    tables = [("int", INT_SPELLINGS, lit_int), ("float", FLOAT_SPELLINGS, lit_float),
              ("string", STRING_SPELLINGS, lit_str), ("char", CHAR_SPELLINGS, lit_char)]
    for kind, table, literal in tables:
        for value, spellings in table.items():
            canonical = literal(value)
            for spelling in spellings:
                # The oracle checks its own table.
                if kind == "int":
                    assert int(spelling.replace("_", ""), 0 if spelling[:2].lower() in ("0x", "0b") else 10) == value
                if kind == "float":
                    assert float(spelling.replace("_", "")) == value, spelling
                for context, template, expected in spelling_contexts(kind, value, canonical):
                    program = batch.program()
                    expression = template.replace("{s}", spelling)
                    if "{const}" in expression:
                        name = program.fresh("LEX")
                        program.declarations.append(f"const {name} = {spelling};")
                        expression = expression.replace("{const}", name)
                    program.is_(expression, expected, f"{kind} {spelling!r} as {context}")
    for tokens, expected in LAYOUT_EXPRESSIONS:
        for name, separator in LAYOUT_SEPARATORS:
            batch.program().is_(layout(tokens, separator), expected, f"{' '.join(tokens)} separated by {name}")
    for name in BINDING_NAMES:
        upper = not (name[0].islower() or name[0] == "_")
        for position, upper_ok, body, value in BINDING_POSITIONS:
            if upper and not upper_ok:
                continue
            # In a pattern `_` ignores a field (Book 14.3); elsewhere it is a name.
            if name == "_" and position == "case binding":
                continue
            program = batch.program()
            if "fail() is int { throw Oops.bad(4); }" not in program.helpers:
                program.helpers.append("fail() is int { throw Oops.bad(4); }")
            helper = program.fresh("bind")
            program.helpers.append(f"{helper}() is int {{\n" + "\n".join(
                "    " + line for line in body.format(n=name).splitlines()) + "\n}")
            program.is_(f"self.{helper}()", lit_int(value), f"{name} as a {position}")
    return batch.programs


# ----- family: enums ---------------------------------------------------------------

COLORS = ["red", "green", "blue"]

# A payload field type: (literal of a value, an int measure of a binding `{a}`, the oracle's measure).
ENUM_FIELDS = {
    "int": (lit_int, "{a}", lambda v: v),
    "string": (lit_str, "{a}.length()", len),
    "@int": (lambda xs: "[" + ", ".join(map(lit_int, xs)) + "]", "{a}.size()", len),
    "%string:int": (lambda m: "[" + ", ".join(f"{lit_str(k)}: {lit_int(v)}" for k, v in m) + "]", "{a}.size()", len),
    "Color": (lambda c: f"Color.{c}",
              "(match {a} { case Color.red { 1; } case Color.green { 2; } case Color.blue { 3; } })",
              lambda c: COLORS.index(c) + 1),
    "Maybe<int>": (lambda m: "Maybe<int>.none" if m is None else f"Maybe.some({lit_int(m)})", "({a} else -1)",
                   lambda m: -1 if m is None else m),
    "Point": (lambda p: f"Point(x: {lit_int(p[0])}, y: {lit_int(p[1])})", "{a}.sum()", lambda p: p[0] * 10 + p[1]),
    "rat": (lit_rat, "int({a} * rat(6, 1))", lambda q: int(q * 6)),
    "float": (lit_float, "int({a})", int),
    "char": (lit_char, "{a}.code()", ord),
    "byte": (lambda b: f"byte({b})", "{a}.int()", lambda b: b),
    "bool": (lit_bool, "(if {a} { 1; } else { 0; })", int),
}

ENUM_DECLARATIONS = """enum Color = (red, green, blue);
enum Lone = (only);
enum Shape = (dot, circle(r is int), rect(w is int, h is int));
enum Reading<T> = (empty, value(item is T), pair(left is T, right is T));
enum Fault is Error = (minor, major(code is int));
enum Mixed = (num(n is int), text(s is string), list(xs is @int), table(m is %string:int), tone(c is Color),
    opt(m is Maybe<int>), spot(p is Point), ratio(q is rat), real(f is float), letter(c is char), octet(b is byte),
    flag(b is bool), two(c is Color, p is Point));

object Point {
    has x is int;
    has y is int;
    Point(:x, :y);
    sum() is int { x * 10 + y; }
}

object Box<T> {
    has value is T;
    Box(:value);
    get() is T { value; }
}""".splitlines()

P = fractions.Fraction


def reading(t):
    return [("empty", []), ("value", [t]), ("pair", [t, t])]


# (type as written, the constructor's name, cases [(case, [field types])], values [(case, payload)]).
ENUM_TYPES = [
    ("Color", "Color", [(c, []) for c in COLORS], [(c, ()) for c in COLORS]),
    ("Lone", "Lone", [("only", [])], [("only", ())]),
    ("Shape", "Shape", [("dot", []), ("circle", ["int"]), ("rect", ["int", "int"])],
     [("dot", ()), ("circle", (0,)), ("circle", (-3,)), ("rect", (2, 3)), ("rect", (3, 2))]),
    ("Fault", "Fault", [("minor", []), ("major", ["int"])], [("minor", ()), ("major", (0,)), ("major", (-7,))]),
    ("Reading<int>", "Reading", reading("int"),
     [("empty", ()), ("value", (5,)), ("pair", (1, 2)), ("pair", (2, 1))]),
    ("Reading<string>", "Reading", reading("string"),
     [("empty", ()), ("value", ("",)), ("value", ("😀",)), ("pair", ("a", "bc"))]),
    ("Reading<@int>", "Reading", reading("@int"), [("empty", ()), ("value", ((1,),)), ("pair", ((1, 2), (3,)))]),
    ("Reading<Color>", "Reading", reading("Color"),
     [("value", ("red",)), ("value", ("blue",)), ("pair", ("green", "red"))]),
    ("Reading<Maybe<int>>", "Reading", reading("Maybe<int>"),
     [("value", (None,)), ("value", (4,)), ("pair", (1, None))]),
    ("Reading<Point>", "Reading", reading("Point"), [("value", ((1, 2),)), ("pair", ((1, 2), (2, 1)))]),
    ("Mixed", "Mixed",
     [("num", ["int"]), ("text", ["string"]), ("list", ["@int"]), ("table", ["%string:int"]), ("tone", ["Color"]),
      ("opt", ["Maybe<int>"]), ("spot", ["Point"]), ("ratio", ["rat"]), ("real", ["float"]), ("letter", ["char"]),
      ("octet", ["byte"]), ("flag", ["bool"]), ("two", ["Color", "Point"])],
     [("num", (7,)), ("num", (-7,)), ("text", ("héllo",)), ("list", ((4, 5, 6),)), ("table", ((("a", 1), ("b", 2)),)),
      ("tone", ("green",)), ("opt", (None,)), ("opt", (9,)), ("spot", ((3, 4),)), ("ratio", (P(7, 2),)),
      ("ratio", (P(-7, 2),)), ("real", (2.75,)), ("real", (-2.75,)), ("letter", ("😀",)), ("octet", (255,)),
      ("flag", (True,)), ("flag", (False,)), ("two", ("blue", (0, 1)))]),
]


def enum_literal(type_name, ctor, cases, value):
    case, payload = value
    fields = dict(cases)[case]
    if not fields:
        return f"{type_name}.{case}"
    return f"{ctor}.{case}(" + ", ".join(ENUM_FIELDS[f][0](v) for f, v in zip(fields, payload)) + ")"


def enum_measure(cases, value, style):
    """The oracle's int for a match in `style` over `value`."""
    case, payload = value
    index = [c for c, _ in cases].index(case)
    fields = dict(cases)[case]
    if style in ("else", "choice wildcard") and index != 0:
        return 0
    used = range(len(fields)) if style != "underscore" else range(min(1, len(fields)))
    return 1000 * (index + 1) + sum(ENUM_FIELDS[fields[i]][2](payload[i]) * (i + 1) for i in used)


def enum_arm(ctor, type_name, case, fields, style, choice):
    """A pattern and its int result for one case."""
    names = [f"a{i}" for i in range(len(fields))]
    if style == "underscore":
        names = [n if i == 0 else "_" for i, n in enumerate(names)]
    result = " + ".join([str(1000 * (case[0] + 1))] + [
        f"{ENUM_FIELDS[f][1].replace('{a}', n)} * {i + 1}" for i, (f, n) in enumerate(zip(fields, names)) if n != "_"])
    head = f".{case[1]}" if choice else (f"{ctor}.{case[1]}" if fields else f"{type_name}.{case[1]}")
    if fields:
        head += "(" + ", ".join(names) + ")"
    return head, result


def enum_match(type_name, ctor, cases, subject, style):
    arms = [enum_arm(ctor, type_name, (i, c), f, style, style.startswith("choice")) for i, (c, f) in enumerate(cases)]
    if style == "choice":
        return f"({subject} ?? " + " !! ".join(f"{h}: {r}" for h, r in arms) + ")"
    if style == "choice wildcard":
        rest = " !! _: 0" if len(cases) > 1 else ""
        return f"({subject} ?? {arms[0][0]}: {arms[0][1]}{rest})"
    if style == "else":
        return f"(match {subject} {{ case {arms[0][0]} {{ {arms[0][1]}; }} else {{ 0; }} }})"
    return f"(match {subject} {{ " + " ".join(f"case {h} {{ {r}; }}" for h, r in arms) + " })"


ENUM_SOURCES = ["literal", "variable", "array element", "map value", "generic box", "if", "action result",
                "closure", "maybe else"]
ENUM_STYLES = ["full", "else", "underscore", "choice", "choice wildcard"]


def enums_family():
    batch = Batch("enums: plain, payload, generic and error enums from every source, matched, chosen, compared and "
                  "thrown", declarations=ENUM_DECLARATIONS)
    for type_name, ctor, cases, values in ENUM_TYPES:
        literals = [enum_literal(type_name, ctor, cases, v) for v in values]
        for index, (value, literal) in enumerate(zip(values, literals)):
            other = literals[(index + 1) % len(literals)]
            # Pairs: every value of the same case, and one of another case.
            same = [j for j, w in enumerate(values) if w[0] == value[0]]
            different = [j for j, w in enumerate(values) if w[0] != value[0]][:1]
            for source in ENUM_SOURCES:
                program = batch.program()
                setup = []
                if source == "literal":
                    subject = literal
                elif source == "variable":
                    subject = program.fresh("e")
                    setup.append(f"let {subject} is {type_name} = {literal};")
                elif source == "array element":
                    subject = f"[{literal}, {other}][0]"
                elif source == "map value":
                    subject = f'(["k": {literal}]["k"] else {other})'
                elif source == "generic box":
                    subject = f"Box(value: {literal}).get()"
                elif source == "if":
                    subject = f"(if true {{ {literal}; }} else {{ {other}; }})"
                elif source == "action result":
                    helper = program.fresh("make")
                    program.helpers.append(f"{helper}() is {type_name} {{ {literal}; }}")
                    subject = f"self.{helper}()"
                elif source == "closure":
                    subject = f"([{literal}].map() -> e {{ e; }})[0]"
                else:
                    subject = f"(Maybe.some({literal}) else {other})"
                desc = f"{literal} from {source}"
                for style in ENUM_STYLES:
                    program.is_(enum_match(type_name, ctor, cases, subject, style), lit_int(enum_measure(cases, value, style)),
                                f"{desc}: {style}", setup)
                    setup = []
                for j in (same + different if source in ("literal", "variable") else different or same):
                    program.is_(f"{subject} == {literals[j]}", lit_bool(values[j] == value), f"{desc} == {literals[j]}")
                program.ok(f"[{subject}, {other}] == [{literal}, {other}]", f"{desc}: in an array")
                if subject.startswith("("):
                    # A map value cannot start with `(` (#263).
                    program.skip(263)
                else:
                    program.ok(f'["k": {subject}] == ["k": {literal}]', f"{desc}: as a map value")
                if type_name == "Fault":
                    program.is_(f"(try {{ self.raise({subject}); }} catch Fault.minor {{ 1; }} catch Fault.major(c) {{ 2 + c; }})",
                                lit_int(1 if value[0] == "minor" else 2 + value[1][0]), f"{desc}: thrown, caught by case")
                    program.is_(f"(try {{ self.raise({subject}); }} catch failure is Fault {{ "
                                f"{enum_match(type_name, ctor, cases, 'failure', 'full')}; }})",
                                lit_int(enum_measure(cases, value, "full")), f"{desc}: thrown, caught by type")
                    if "raise(e is Fault) is int { throw e; }" not in program.helpers:
                        program.helpers.append("raise(e is Fault) is int { throw e; }")
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
    "structures": structures_family,
    "keys": keys_family,
    "casts": casts_family,
    "lexical": lexical_family,
    "enums": enums_family,
    "persistence": persistence_family,
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
        SEEN.clear()
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
