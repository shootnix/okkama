#!/usr/bin/env python3
"""Build the Sparrow assembly books from their sources.

    python3 docs/SparrowBook/build.py [--check]

SparrowBook.src.md and SparrowBook-RU.src.md are the chapters; `{{example:NAME}}`
inlines examples/NAME.spa, `{{reference}}` the instruction reference, whose
operand order comes from the instruction table of okkama-sparrow (lib.rs).
With --check every example is assembled, checked and run with `sparrow`
(SPARROW, default the release build) and its output compared with the
`$ sparrow …` block that follows it in the English book.
"""
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SPARROW = os.environ.get("SPARROW", os.path.join(ROOT, "okkama-rs", "target", "release", "sparrow"))

# mnemonic: (English, Russian)
DESCRIPTIONS = {
    "const": ("constant: `int N`, `bool`, `byte`, `float 0xBITS`, `char CODE`, `str \"…\"`", "константа: `int N`, `bool`, `byte`, `float 0xБИТЫ`, `char КОД`, `str \"…\"`"),
    "move": ("copy a register (also a successor taken, a retagged value, a cap)", "копия регистра (также взятый successor, перетегированное значение, cap)"),
    "undef": ("a placeholder of the destination's type, read only under a binding check", "заглушка типа назначения; читается только под проверкой связывания"),
    "int.add": ("exact sum", "точная сумма"),
    "int.sub": ("exact difference", "точная разность"),
    "int.mul": ("exact product", "точное произведение"),
    "int.mod": ("remainder of truncated division; fails on 0", "остаток усечённого деления; 0 — отказ"),
    "int.and": ("bitwise and (two's complement)", "побитовое и (дополнительный код)"),
    "int.or": ("bitwise or", "побитовое или"),
    "int.xor": ("bitwise exclusive or", "побитовое исключающее или"),
    "int.shl": ("shift left", "сдвиг влево"),
    "int.shr": ("arithmetic shift right", "арифметический сдвиг вправо"),
    "int.ushr": ("logical shift right of the 64-bit word", "логический сдвиг вправо 64-битного слова"),
    "int.neg": ("negation", "смена знака"),
    "int.not": ("bitwise not", "побитовое не"),
    "int.abs": ("absolute value", "модуль"),
    "sign": ("−1, 0 or 1 for an int, float or rat", "−1, 0 или 1 для int, float или rat"),
    "int.cmp": ("compare (`eq ne lt le gt ge`) to a bool", "сравнение (`eq ne lt le gt ge`) в bool"),
    "byte.from": ("int, float or rat to byte; outside 0..255 fails with the given context", "int, float или rat в byte; вне 0..255 — отказ с указанным контекстом"),
    "cvt": ("convert to the destination's type (numbers, char, text)", "преобразование к типу назначения (числа, char, текст)"),
    "bits": ("`bit_count`, `leading_zeros`, `trailing_zeros`, `rotl`, `rotr`, `reverse_bits`, `swap_bytes` of an int word or a byte (`true`); rotations take a count", "`bit_count`, `leading_zeros`, `trailing_zeros`, `rotl`, `rotr`, `reverse_bits`, `swap_bytes` над словом int или byte (`true`); вращения берут счётчик"),
    "byte.and": ("bitwise and", "побитовое и"),
    "byte.or": ("bitwise or", "побитовое или"),
    "byte.xor": ("bitwise exclusive or", "побитовое исключающее или"),
    "byte.shl": ("shift left", "сдвиг влево"),
    "byte.shr": ("arithmetic shift right", "арифметический сдвиг вправо"),
    "byte.ushr": ("logical shift right", "логический сдвиг вправо"),
    "byte.not": ("bitwise not", "побитовое не"),
    "byte.cmp": ("compare bytes", "сравнение byte"),
    "char.cmp": ("compare characters", "сравнение char"),
    "bool.not": ("negation", "отрицание"),
    "bool.cmp": ("compare booleans", "сравнение bool"),
    "float.add": ("sum; a non-finite result fails", "сумма; не конечный результат — отказ"),
    "float.sub": ("difference", "разность"),
    "float.mul": ("product", "произведение"),
    "float.div": ("quotient; division by zero fails", "частное; деление на ноль — отказ"),
    "float.mod": ("remainder", "остаток"),
    "float.pow": ("power", "степень"),
    "float.neg": ("negation", "смена знака"),
    "float.abs": ("absolute value", "модуль"),
    "float.sqrt": ("square root", "квадратный корень"),
    "float.cmp": ("compare floats", "сравнение float"),
    "rat.new": ("fraction of two ints; a zero denominator fails", "дробь из двух int; нулевой знаменатель — отказ"),
    "rat.add": ("exact sum", "точная сумма"),
    "rat.sub": ("exact difference", "точная разность"),
    "rat.mul": ("exact product", "точное произведение"),
    "rat.div": ("exact quotient", "точное частное"),
    "rat.mod": ("remainder", "остаток"),
    "rat.neg": ("negation", "смена знака"),
    "rat.abs": ("absolute value", "модуль"),
    "rat.cmp": ("compare fractions", "сравнение дробей"),
    "rat.part": ("numerator, or denominator (`true`), as a str", "числитель или знаменатель (`true`) как str"),
    "str.concat": ("concatenation", "конкатенация"),
    "str.cmp": ("compare texts", "сравнение строк"),
    "str.len": ("length in Unicode scalar values", "длина в скалярных значениях Unicode"),
    "str.slice": ("substring start..end", "подстрока start..end"),
    "str.split": ("split by a separator into an array of str", "разбиение по разделителю в массив str"),
    "str.replace": ("replace every occurrence", "замена всех вхождений"),
    "str.replace_first": ("replace the first occurrence", "замена первого вхождения"),
    "str.trim": ("strip surrounding white space", "обрезка пробелов по краям"),
    "str.chomp": ("strip one trailing line end", "удаление одного конца строки"),
    "str.utf8": ("the UTF-8 bytes (`array byte`)", "байты UTF-8 (`array byte`)"),
    "str.fn": ("a string function: `find`, `find_last`, `starts_with`, `ends_with`, `trim_start`, `trim_end`, `lower`, `upper`, `repeat`, `characters`", "строковая функция: `find`, `find_last`, `starts_with`, `ends_with`, `trim_start`, `trim_end`, `lower`, `upper`, `repeat`, `characters`"),
    "global.get": ("the value of a module constant, built once by its init function", "значение константы модуля, построенное один раз её функцией init"),
    "region.settle": ("close a region in an async function: destroys run from the last identity, each async destroy is awaited, a cancellation waits until the close ends", "закрыть регион в async-функции: destroy идут с последней идентичности, каждый async destroy ожидается, отмена ждёт конца закрытия"),
    "region.settle_unwind": ("`region.settle` on a failure path; the result is the failure with destroy failures as secondary", "`region.settle` на пути отказа; результат — отказ с отказами destroy как вторичными"),
    "val.cmp": ("deep equality, `eq` or `ne` only", "глубокое равенство, только `eq` и `ne`"),
    "enum.case": ("a case of an enum by number", "вариант enum по номеру"),
    "enum.cmp": ("compare enum cases", "сравнение вариантов enum"),
    "any.box": ("box a value as `any`", "упаковка значения в `any`"),
    "range.new": ("integer range start..end (inclusive with `true`)", "диапазон целых start..end (включительно при `true`)"),
    "range.part": ("start, or exclusive end (`true`), of a range", "начало или исключающий конец (`true`) диапазона"),
    "obj.new": ("an object from its fields in declaration order", "объект из полей в порядке объявления"),
    "obj.init": ("bind a field of an object under construction whose `self` escaped", "связать поле строящегося объекта, чей `self` уже ушёл значением"),
    "obj.get": ("read a field by number", "чтение поля по номеру"),
    "obj.next": ("successor: a copy with fields replaced (`[N: rX, …]`)", "successor: копия с заменой полей (`[N: rX, …]`)"),
    "role.wrap": ("an object as a reference of a role it does", "объект как ссылка роли, которую он делает"),
    "role.narrow": ("a role reference as another role; fails with the message otherwise", "ссылка роли как другая роль; иначе отказ с сообщением"),
    "variant.new": ("a variant value: an object of case N from its fields", "значение варианта: объект случая N из его полей"),
    "variant.tag": ("the number of the active case (an int)", "номер активного случая (int)"),
    "variant.get": ("a payload field: the case, then the field; another active case fails", "поле полезной нагрузки: сначала случай, затем поле; другой активный случай — отказ"),
    "variant.of": ("a case object as a value of its variant", "объект случая как значение его варианта"),
    "arr.new": ("an array of the listed registers", "массив из перечисленных регистров"),
    "arr.len": ("length", "длина"),
    "arr.get": ("element by index; a bad index fails", "элемент по индексу; неверный индекс — отказ"),
    "arr.slice": ("subarray start..end", "подмассив start..end"),
    "arr.replace": ("copy with element index replaced (#250)", "копия с заменённым элементом index (#250)"),
    "arr.remove": ("copy without element index (#251)", "копия без элемента index (#251)"),
    "arr.join": ("join an array of str with a separator", "склейка массива str через разделитель"),
    "arr.concat": ("`a + b + …`: one array allocated for the sum of the parts' lengths", "`a + b + …`: один массив, выделенный под сумму длин частей"),
    "arr.diff": ("`a - b - …`: the elements of the first array equal to no element of the others, in order", "`a - b - …`: элементы первого массива, не равные ни одному элементу остальных, по порядку"),
    "builder.new": ("an empty array builder", "пустой сборщик массива"),
    "builder.push": ("append one element; the result is the builder to use next", "добавить элемент; результат — сборщик для дальнейшей работы"),
    "builder.append": ("append an array", "добавить массив"),
    "builder.range": ("append the integers of start..end", "добавить целые start..end"),
    "builder.finish": ("the built array", "собранный массив"),
    "map.new": ("a map of `[rK: rV, …]`", "словарь из `[rK: rV, …]`"),
    "map.len": ("number of entries", "число записей"),
    "map.has": ("does the key exist", "есть ли ключ"),
    "map.get": ("value by key; a missing key fails", "значение по ключу; нет ключа — отказ"),
    "map.keys": ("keys in their natural order", "ключи в естественном порядке"),
    "map.values": ("values in key order", "значения в порядке ключей"),
    "map.merge": ("`a + b + …`: the entries of every part; a repeated key takes the rightmost value", "`a + b + …`: записи всех частей; при повторе ключа — значение самой правой"),
    "map.diff": ("`a - b - …`: the entries of the first map whose keys are in none of the others", "`a - b - …`: записи первого словаря, ключей которых нет ни в одном из остальных"),
    "mbuilder.new": ("an empty map builder", "пустой сборщик словаря"),
    "mbuilder.put": ("put one entry; the result is the builder to use next", "добавить запись; результат — сборщик для дальнейшей работы"),
    "mbuilder.finish": ("the built map", "собранный словарь"),
    "call": ("direct call; an async callee gives a cold future", "прямой вызов; async-функция даёт холодный future"),
    "fn.closure": ("a closure: captures fill the function's first parameters", "замыкание: захваты заполняют первые параметры функции"),
    "call.fn": ("call a closure with the remaining arguments", "вызов замыкания с остальными аргументами"),
    "call.role": ("call slot N of a role on a role reference", "вызов слота N роли на ссылке роли"),
    "call.cap": ("compute a cap field over its owner", "вычислить поле cap над владельцем"),
    "await": ("run or wait for a future; its value", "выполнить future или дождаться его; его значение"),
    "fut.spawn": ("start a future now; the same future", "запустить future сейчас; тот же future"),
    "fut": ("`cancel`, `done` or `cancelled` on a future", "`cancel`, `done` или `cancelled` над future"),
    "worker.spawn": ("run a synchronous action in a worker process", "выполнить синхронный action в процессе-воркере"),
    "worker.bind": ("bind an action and its receiver's fields for a worker", "связать action и поля его receiver для воркера"),
    "region.enter": ("open an ownership region", "открыть регион владения"),
    "region.adopt": ("the region owns the value", "регион владеет значением"),
    "region.transfer": ("move ownership to another region", "передать владение другому региону"),
    "region.track": ("record a new identity in the region", "записать новую идентичность в регион"),
    "region.leave": ("close: run `destroy` of what it owns, except what the kept value reaches", "закрыть: `destroy` всего, чем владеет, кроме достижимого из сохраняемого значения"),
    "region.unwind": ("close on the failure path; the failure with secondaries of `destroy`", "закрыть на пути отказа; отказ с вторичными отказами `destroy`"),
    "fail.secondary": ("attach a cleanup failure to a primary one", "присоединить отказ очистки к основному"),
    "fail.caused": ("a new failure caused by a handled one", "новый отказ, вызванный обработанным"),
    "jmp": ("jump, with edge moves", "переход с присваиваниями ребра"),
    "br": ("branch on a bool", "ветвление по bool"),
    "ret": ("return (`_` from a void function)", "возврат (`_` из void-функции)"),
    "panic": ("fail with a str message", "отказ с сообщением str"),
    "trap": ("program error: `confirm`, `range`, `empty`, `unbound`, `unreachable`, `pending` (an action on an object under construction) with a message", "программная ошибка: `confirm`, `range`, `empty`, `unbound`, `unreachable`, `pending` (действие у недостроенного объекта) с сообщением"),
    "unreachable": ("control never gets here", "управление сюда не приходит"),
    "throw": ("throw an object that does the builtin Error role", "бросить объект, делающий встроенную роль Error"),
    "raise": ("send a failure on", "передать отказ дальше"),
    "catch": ("match a failure against `object N` or `role N`: the value on the edge to the first target, else the second", "сопоставить отказ с `object N` или `role N`: значение на ребре к первой цели, иначе вторая"),
    "arr.lookup": ("element by index on the first edge, or the second if missing", "элемент по индексу на первом ребре, иначе второе"),
    "map.lookup": ("value by key on the first edge, or the second if missing", "значение по ключу на первом ребре, иначе второе"),
}

SHAPES = {
    "Dst": "rD", "Option<Dst>": "rD/_", "Register": "rX", "Constant": "KIND VALUE", "Cmp": "CMP",
    "Option<String>": "\"…\"/_", "String": "\"…\"", "bool": "true/false", "u32": "N", "BitOp": "OP",
    "Option<Register>": "rX/_", "EnumRef": "enumN", "ObjectRef": "objN", "RoleRef": "roleN", "CapRef": "capN", "VariantRef": "variantN",
    "FuncRef": "fN", "Vec<Register>": "[rX, …]", "Vec<(u32, Register)>": "[N: rX, …]",
    "Vec<(Register, Register)>": "[rK: rV, …]", "Target": "@T", "FutOp": "OP", "CatchKind": "object/role/variant",
    "TrapCode": "CODE", "StrFn": "FN",
}


def split_fields(fields):
    """`a: T, b: Vec<(u32, R)>` into fields, keeping commas inside brackets."""
    parts, depth, current = [], 0, ""
    for character in fields:
        depth += character in "<("
        depth -= character in ">)"
        if character == "," and depth == 0:
            parts.append(current)
            current = ""
        else:
            current += character
    parts.append(current)
    return [part for part in parts if part.strip()]


def reference(language):
    source = open(os.path.join(ROOT, "okkama-rs", "crates", "okkama-sparrow", "src", "lib.rs")).read()
    body = source[source.index("instructions! {"):]
    rows = re.findall(r'^\s+\w+ = (\d+), "([a-z_.]+)" \{([^}]*)\}', body, re.M)
    head = "| Code | Instruction | Meaning |" if language == "en" else "| Код | Инструкция | Значение |"
    lines = [head, "|---:|---|---|"]
    for code, mnemonic, fields in rows:
        operands = [SHAPES[field.split(":", 1)[1].strip()] for field in split_fields(fields)]
        text = DESCRIPTIONS[mnemonic][0 if language == "en" else 1]
        lines.append(f"| {code} | `{mnemonic} {', '.join(operands)}` | {text} |".replace(" ` |", "` |"))
    return "\n".join(lines)


def build(source, target, language):
    text = open(os.path.join(HERE, source)).read()
    text = re.sub(r"\{\{example:(\w+)\}\}", lambda m: "```text\n" + open(os.path.join(HERE, "examples", m.group(1) + ".spa")).read().rstrip() + "\n```", text)
    text = text.replace("{{reference}}", reference(language))
    banner = "<!-- Generated by build.py from " + source + "; edit the source. -->\n"
    open(os.path.join(HERE, target), "w").write(banner + text)


def check():
    """Assemble, check and run every example; compare with the book."""
    book = open(os.path.join(HERE, "SparrowBook.src.md")).read()
    failures = 0
    with tempfile.TemporaryDirectory() as directory:
        for name in sorted(os.listdir(os.path.join(HERE, "examples"))):
            stem = name[:-4]
            spa, spb = os.path.join(HERE, "examples", name), os.path.join(directory, stem + ".spb")
            steps = [[SPARROW, "--assemble", spa, "-o", spb], [SPARROW, "check", spb]]
            for step in steps:
                p = subprocess.run(step, capture_output=True, text=True)
                if p.returncode != 0:
                    print(f"{stem}: {' '.join(step[1:2])} failed: {p.stderr.strip()}")
                    failures += 1
            command = [SPARROW, "test" if stem == "testing" else "run", spb]
            p = subprocess.run(command, capture_output=True, text=True, timeout=60)
            shown = re.search(r"\{\{example:" + stem + r"\}\}\n\n```text\n(.*?)```", book, re.S)
            expected = shown.group(1).splitlines()[1:] if shown else []
            output = p.stdout.splitlines()
            if "echo $?" in (shown.group(1) if shown else ""):
                output = output + [str(p.returncode)]
            if output != expected:
                print(f"{stem}: printed {output}, the book shows {expected}")
                failures += 1
            else:
                print(f"{stem}: ok")
    return failures


if __name__ == "__main__":
    build("SparrowBook.src.md", "SparrowBook.md", "en")
    if os.path.exists(os.path.join(HERE, "SparrowBook-RU.src.md")):
        build("SparrowBook-RU.src.md", "SparrowBook-RU.md", "ru")
    if "--check" in sys.argv:
        sys.exit(1 if check() else 0)
