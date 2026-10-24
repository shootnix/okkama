#!/usr/bin/env python3
"""Generate the native ABI table from the std provider declarations.

Reads every `native` declaration of std/runtime/*.ok and writes
crates/okkama-runtime-abi/src/table.rs: one `natives!` entry per provider
symbol `module@major::name/arity` with its Okkama signature. Wire IDs are
stable: an existing symbol keeps its ID and name (read from the current
table); a new symbol gets the next free ID. Removing a declaration retires
its ID — it is never reused.

    tools/engines/gen-abi.py
"""

import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUST_ROOT = HERE.parent.parent
REPO_ROOT = RUST_ROOT.parent
TABLE = RUST_ROOT / "crates" / "okkama-runtime-abi" / "src" / "table.rs"

DECLARATION = re.compile(
    r"\bnative\s+(async\s+)?(?!object\b)([a-z_0-9]+)\s*\((.*?)\)\s*(?:is\s+([^;{]+?))?\s*;",
    re.S,
)
ENTRY = re.compile(r'^\s*(\w+)\s*=\s*(\d+),\s*"([^"]+)"')


def camel(text):
    return "".join(part[:1].upper() + part[1:] for part in re.split(r"[_/]", text) if part)


def main():
    known = {}
    retired = set()
    if TABLE.is_file():
        for line in TABLE.read_text().splitlines():
            found = ENTRY.match(line)
            if found:
                known[found.group(3)] = (found.group(1), int(found.group(2)))
            retired.update(int(n) for n in re.findall(r"retired:\s*([\d, ]+)", line) for n in n.split(",") if n.strip())
    used_ids = {entry[1] for entry in known.values()} | retired
    used_names = {entry[0] for entry in known.values()}
    entries = []
    for path in sorted((REPO_ROOT / "std" / "runtime").glob("*.ok")):
        text = path.read_text()
        header = re.search(r"^module\s+(\S+)\s+v(\d+)\.", text, re.M)
        module, major = header.group(1), header.group(2)
        code = re.sub(r"//[^\n]*", "", text)
        for found in DECLARATION.finditer(code):
            is_async, name, params, result = found.groups()
            types = []
            for param in filter(None, (p.strip() for p in params.split(","))):
                types.append(param.split(" is ", 1)[1].strip())
            symbol = f"{module}@{major}::{name}/{len(types)}"
            signature = f"({', '.join(types)}) -> {(result or 'void').strip()}"
            if is_async:
                signature = "async " + signature
            entries.append((symbol, signature, module, name))
    symbols = {entry[0] for entry in entries}
    lines = [
        "// Сгенерировано tools/engines/gen-abi.py из объявлений std/runtime/*.ok.",
        "// Wire ID стабильны: не менять вручную номера существующих символов.",
        "natives! {",
    ]
    next_id = max(used_ids, default=0) + 1
    rows = []
    for symbol, signature, module, name in entries:
        if symbol in known:
            variant, wire = known[symbol]
        else:
            variant = camel(module.removeprefix("runtime/")) + camel(name)
            while variant in used_names:
                variant += "X"
            used_names.add(variant)
            wire = next_id
            next_id += 1
        rows.append((wire, variant, symbol, signature))
    gone = sorted(entry[1] for symbol, entry in known.items() if symbol not in symbols)
    retired.update(gone)
    for wire, variant, symbol, signature in sorted(rows):
        lines.append(f'    {variant} = {wire}, "{symbol}", "{signature}";')
    lines.append("}")
    if retired:
        lines.append(f"// retired: {', '.join(str(n) for n in sorted(retired))}")
    TABLE.write_text("\n".join(lines) + "\n")
    print(f"{len(rows)} natives, {len(gone)} retired now, next id {next_id}")


if __name__ == "__main__":
    main()
