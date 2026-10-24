#!/usr/bin/env python3
"""Inventory the public surface of an Okkama standard library tree.

Usage: tools/inventory_std.py [STD_ROOT] [--write]

Writes contracts/stdlib-inventory.json (every module, its directly owned
declarations, public actions and constructors) and contracts/stdlib.json (one
STD.* coverage contract per module). The scanner is a small token-level
reader written for this suite: it strips comments, strings and raw strings,
tracks braces, and records declarations at module scope and members at
object/role scope. It does not execute or type-check Okkama.

Classification:
  * runtime/*  - native substrate modules. Their contracts are exercised
                 through public modules; status "informative".
  * other      - public library modules; status "mandatory".
"""

import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SUITE = os.path.dirname(HERE)

TOKEN = re.compile(r"""
    (?P<raw>```.*?```)
  | (?P<str>"(?:\\.|[^"\\])*")
  | (?P<chr>'(?:\\.|[^'\\])')
  | (?P<lc>//[^\n]*)
  | (?P<bc>/\*.*?\*/)
  | (?P<ident>[A-Za-z_][A-Za-z0-9_]*)
  | (?P<punct>::|\.\.\.|\.\.=|\.\.|->|<-|[{}()\[\];:,.<>=^@%\#&|!*+/-])
  | (?P<num>[0-9][0-9_xXbBa-fA-F.]*)
  | (?P<ws>\s+)
  | (?P<other>.)
""", re.S | re.X)


def tokens(text):
    out = []
    line = 1
    for m in TOKEN.finditer(text):
        kind = m.lastgroup
        val = m.group(0)
        if kind not in ("ws", "lc", "bc"):
            out.append((kind, val, line))
        line += val.count("\n")
    return out


def parse_signature(toks, i):
    """toks[i] is an identifier followed by '('. Return (params_text, result, next_index)."""
    depth = 0
    j = i + 1
    params = []
    while j < len(toks):
        k, v, _ = toks[j]
        if v == "(":
            depth += 1
            if depth > 1:
                params.append(v)
        elif v == ")":
            depth -= 1
            if depth == 0:
                break
            params.append(v)
        else:
            params.append(v)
        j += 1
    j += 1
    result = None
    if j < len(toks) and toks[j][1] == "is":
        r = []
        j += 1
        d = 0
        while j < len(toks):
            v = toks[j][1]
            if d == 0 and v in ("{", ";"):
                break
            if v in ("(", "<"):
                d += 1
            if v in (")", ">"):
                d -= 1
            r.append(v)
            j += 1
        result = " ".join(r)
    return " ".join(params), result, j


def skip_block(toks, j):
    """toks[j] is '{'; return index after the matching '}'."""
    depth = 0
    while j < len(toks):
        if toks[j][1] == "{":
            depth += 1
        elif toks[j][1] == "}":
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    return j


def scan_body(toks, j, owner_name):
    """Scan an object/role body starting at '{'. Returns (members, end)."""
    members = {"actions": [], "private_actions": [], "constructors": [], "fields": [], "caps": [], "has_fake": False,
               "has_destroy": False, "tests": 0}
    end = skip_block(toks, j)
    j += 1
    private = False
    while j < end - 1:
        k, v, line = toks[j]
        if v == "private" and j + 1 < end and toks[j + 1][1] == ":":
            private = True
            j += 2
            continue
        if v == "has" and j + 1 < end and toks[j + 1][0] == "ident" and toks[j + 1][1] != "(":
            names = []
            j += 1
            while j < end and toks[j][1] not in (";", "is", "="):
                if toks[j][0] == "ident":
                    names.append(toks[j][1])
                j += 1
            members["fields"].extend(names)
            while j < end and toks[j][1] != ";":
                if toks[j][1] == "{":
                    j = skip_block(toks, j)
                    continue
                j += 1
            j += 1
            continue
        if v in ("test",) and j + 1 < end and toks[j + 1][0] == "str":
            members["tests"] += 1
            j += 2
            if toks[j][1] == "{":
                j = skip_block(toks, j)
            continue
        if v == "async" and j + 1 < end and toks[j + 1][1] == "test":
            members["tests"] += 1
            j += 3
            if j < end and toks[j][1] == "{":
                j = skip_block(toks, j)
            continue
        if v == "fake" and j + 1 < end and toks[j + 1][1] == "{":
            members["has_fake"] = True
            j = skip_block(toks, j + 1)
            continue
        if v == "destroy" and j + 1 < end and toks[j + 1][1] == "{":
            members["has_destroy"] = True
            j = skip_block(toks, j + 1)
            continue
        if v == "cap" and j + 1 < end and toks[j + 1][0] == "ident":
            members["caps"].append(toks[j + 1][1])
            j += 2
            if toks[j][1] == "{":
                j = skip_block(toks, j)
            continue
        if v == "#":
            # directive such as #projected or #projected(alias)
            j += 2
            if j < end and toks[j][1] == "(":
                while j < end and toks[j][1] != ")":
                    j += 1
                j += 1
            continue
        is_async = False
        if v == "async":
            is_async = True
            j += 1
            k, v, line = toks[j]
        if k == "ident" and j + 1 < end and toks[j + 1][1] == "(":
            params, result, nj = parse_signature(toks, j)
            if v == owner_name:
                members["constructors"].append({"params": params, "line": line})
            else:
                entry = {"name": v, "params": params, "result": result, "async": is_async, "line": line}
                (members["private_actions"] if private else members["actions"]).append(entry)
            j = nj
            if j < end and toks[j][1] == "{":
                j = skip_block(toks, j)
            elif j < end and toks[j][1] == ";":
                j += 1
            continue
        if v == "{":
            j = skip_block(toks, j)
            continue
        j += 1
    return members, end


def scan_module(path):
    with open(path, "rb") as fh:
        raw = fh.read()
    text = raw.decode("utf-8")
    toks = tokens(text)
    mod = {"declarations": [], "imports": [], "sha256": hashlib.sha256(raw).hexdigest(), "module": None, "version": None}
    i = 0
    if len(toks) > 2 and toks[0][1] == "module":
        name = []
        i = 1
        while i < len(toks) and not (toks[i][0] == "ident" and re.match(r"^v[0-9]", toks[i][1])):
            name.append(toks[i][1])
            i += 1
        mod["module"] = "".join(name)
        ver = []
        while i < len(toks) and toks[i][1] != ";":
            ver.append(toks[i][1])
            i += 1
        mod["version"] = "".join(ver)
        i += 1
    while i < len(toks):
        k, v, line = toks[i]
        if v == "import":
            j = i + 1
            path_tok = toks[j][1].strip('"') if j < len(toks) else ""
            takes = []
            while j < len(toks) and toks[j][1] != ";":
                if toks[j][1] == "take":
                    j += 1
                    while j < len(toks) and toks[j][1] != ";":
                        if toks[j][0] == "ident":
                            takes.append(toks[j][1])
                        j += 1
                    break
                j += 1
            mod["imports"].append({"path": path_tok, "take": takes})
            i = j + 1
            continue
        if v in ("object", "role") and i + 1 < len(toks) and toks[i + 1][0] == "ident":
            name = toks[i + 1][1]
            j = i + 2
            generics = []
            does = []
            if j < len(toks) and toks[j][1] == "<":
                d = 0
                while j < len(toks):
                    if toks[j][1] == "<":
                        d += 1
                    elif toks[j][1] == ">":
                        d -= 1
                        if d == 0:
                            j += 1
                            break
                    elif d == 1 and toks[j][0] == "ident":
                        generics.append(toks[j][1])
                    j += 1
            if j < len(toks) and toks[j][1] == "does":
                j += 1
                cur = []
                d = 0
                while j < len(toks) and not (toks[j][1] == "{" and d == 0):
                    t = toks[j][1]
                    if t == "<":
                        d += 1
                    elif t == ">":
                        d -= 1
                    if t == "," and d == 0:
                        does.append("".join(cur))
                        cur = []
                    else:
                        cur.append(t)
                    j += 1
                if cur:
                    does.append("".join(cur))
            while j < len(toks) and toks[j][1] != "{":
                j += 1
            members, end = scan_body(toks, j, name)
            decl = {"kind": v, "name": name, "line": line, "generics": generics, "does": does}
            decl.update(members)
            mod["declarations"].append(decl)
            i = end
            continue
        if v == "enum" and i + 1 < len(toks):
            name = toks[i + 1][1]
            cases = []
            j = i + 2
            while j < len(toks) and toks[j][1] != ";":
                if toks[j][0] == "ident":
                    cases.append(toks[j][1])
                j += 1
            mod["declarations"].append({"kind": "enum", "name": name, "line": line, "cases": cases})
            i = j + 1
            continue
        if v == "native":
            j = i + 1
            if toks[j][1] == "object":
                mod["declarations"].append({"kind": "native-object", "name": toks[j + 1][1], "line": line})
                i = j + 3
                continue
            is_async = toks[j][1] == "async"
            if is_async:
                j += 1
            params, result, nj = parse_signature(toks, j)
            mod["declarations"].append({"kind": "native-action", "name": toks[j][1], "params": params,
                                        "result": result, "async": is_async, "line": line})
            i = nj + 1
            continue
        if v == "cap" and i + 3 < len(toks) and toks[i + 2][1] == "::":
            mod["declarations"].append({"kind": "external-cap", "owner": toks[i + 1][1], "name": toks[i + 3][1], "line": line})
            j = i + 4
            while j < len(toks) and toks[j][1] != "{":
                j += 1
            i = skip_block(toks, j)
            continue
        if (v == "test" or v == "async") and i + 1 < len(toks) and (toks[i + 1][0] == "str" or toks[i + 1][1] == "test"):
            j = i + 1
            while j < len(toks) and toks[j][1] != "{":
                j += 1
            i = skip_block(toks, j)
            continue
        is_async = False
        if v == "async" and i + 1 < len(toks):
            is_async = True
            i += 1
            k, v, line = toks[i]
        if k == "ident" and i + 1 < len(toks) and toks[i + 1][1] == "(":
            params, result, nj = parse_signature(toks, i)
            mod["declarations"].append({"kind": "action", "name": v, "params": params, "result": result,
                                        "async": is_async, "line": line})
            i = nj
            if i < len(toks) and toks[i][1] == "{":
                i = skip_block(toks, i)
            continue
        i += 1
    return mod


def contract_id(module):
    return "STD." + module.upper().replace("/", ".")


def main(argv):
    write = "--write" in argv
    args = [a for a in argv if not a.startswith("--")]
    root = os.path.abspath(args[0] if args else os.path.join(SUITE, "..", "std"))
    modules = []
    problems = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for fname in sorted(filenames):
            if not fname.endswith(".ok"):
                continue
            full = os.path.join(dirpath, fname)
            rel = os.path.relpath(full, root)
            m = scan_module(full)
            expected = rel[:-3].replace(os.sep, "/")
            m["path"] = rel.replace(os.sep, "/")
            m["import_path"] = expected
            if m["module"] != expected:
                problems.append("%s declares module %r (import path %r)" % (rel, m["module"], expected))
            m["class"] = "substrate" if expected.startswith("runtime/") else "public"
            public = 0
            for d in m["declarations"]:
                if d["kind"] in ("object", "role", "enum", "action", "external-cap"):
                    public += 1
            m["public_declarations"] = public
            m["public_actions"] = sum(len(d.get("actions", [])) for d in m["declarations"]) + \
                sum(1 for d in m["declarations"] if d["kind"] == "action")
            modules.append(m)
    inv = {
        "schema": "ocs-stdlib-inventory/1",
        "root": os.path.relpath(root, SUITE),
        "generator": "tools/inventory_std.py",
        "modules": modules,
        "totals": {
            "modules": len(modules),
            "public_modules": sum(1 for m in modules if m["class"] == "public"),
            "substrate_modules": sum(1 for m in modules if m["class"] == "substrate"),
            "declarations": sum(len(m["declarations"]) for m in modules),
            "public_actions": sum(m["public_actions"] for m in modules),
        },
        "problems": problems,
    }
    contracts = []
    for m in modules:
        if m["class"] == "substrate":
            status = "informative"
            notes = "Native substrate module; its behaviour is observed through the public modules that compose it."
        else:
            status = "mandatory"
            notes = None
        decls = ", ".join(sorted(d["name"] for d in m["declarations"]
                                 if d["kind"] in ("object", "role", "enum", "action")))[:400]
        c = {
            "id": contract_id(m["import_path"]),
            "title": "std %s public contract (%s)" % (m["import_path"], decls or "no public declarations"),
            "family": "Standard library: " + m["import_path"].split("/")[0],
            "spec_refs": ["std/" + m["path"]],
            "status": status,
            "aspects": {
                "normal": "required",
                "boundary": "required",
                "invalid": "required",
                "interaction": "n/a: cross-module interactions are tracked as L12/L13 interaction contracts",
            },
        }
        if not m["public_declarations"]:
            c["aspects"]["boundary"] = "n/a: module owns no public declarations"
            c["aspects"]["invalid"] = "n/a: module owns no public declarations"
        if notes:
            c["notes"] = notes
        contracts.append(c)
    doc = {"schema": "ocs-contracts/1", "family": "Standard library",
           "description": "One coverage contract per std module, generated by tools/inventory_std.py from the inventory. "
                          "Importing a module is never sufficient coverage: a case must observe API behaviour.",
           "contracts": contracts}
    print("modules: %(modules)d (public %(public_modules)d, substrate %(substrate_modules)d), declarations %(declarations)d, public actions %(public_actions)d" % inv["totals"])
    for p in problems:
        print("problem: " + p)
    if write:
        with open(os.path.join(SUITE, "contracts", "stdlib-inventory.json"), "w") as fh:
            json.dump(inv, fh, indent=1, sort_keys=True)
            fh.write("\n")
        with open(os.path.join(SUITE, "contracts", "stdlib.json"), "w") as fh:
            json.dump(doc, fh, indent=2)
            fh.write("\n")
        print("written contracts/stdlib-inventory.json and contracts/stdlib.json")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
