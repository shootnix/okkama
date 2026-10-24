"""Contract inventory loading and contract -> tests -> results traceability."""

import os

from .model import ASPECTS, CONTRACT_RE, CONTRACTS_SCHEMA, CaseError, load_json

CONTRACT_STATUSES = ("mandatory", "unresolved", "deferred", "informative")


def load_contracts(suite_root):
    cdir = os.path.join(suite_root, "contracts")
    contracts = {}
    errors = []
    for fname in sorted(os.listdir(cdir)):
        if not fname.endswith(".json"):
            continue
        path = os.path.join(cdir, fname)
        data = load_json(path)
        if data.get("schema") != CONTRACTS_SCHEMA:
            continue  # other inventories (stdlib API listing, ledger) live here too
        for c in data.get("contracts", []):
            cid = c.get("id")
            where = "%s: contract %r" % (fname, cid)
            if not isinstance(cid, str) or not CONTRACT_RE.match(cid):
                errors.append("%s: invalid id" % where)
                continue
            if cid in contracts:
                errors.append("duplicate contract id %s (%s and %s)" % (cid, contracts[cid]["_file"], fname))
                continue
            if c.get("status") not in CONTRACT_STATUSES:
                errors.append("%s: status must be one of %s" % (where, CONTRACT_STATUSES))
            aspects = c.get("aspects")
            if not isinstance(aspects, dict) or set(aspects) != set(ASPECTS):
                errors.append("%s: aspects must state every one of %s" % (where, ASPECTS))
            else:
                for k, v in aspects.items():
                    if v != "required" and not (isinstance(v, str) and v.startswith("n/a:") and len(v) > 8):
                        errors.append("%s: aspect %s must be 'required' or 'n/a: <justification>'" % (where, k))
            if c.get("status") == "unresolved" and not c.get("discrepancies"):
                errors.append("%s: unresolved contracts must cite discrepancy-ledger entries" % where)
            if not c.get("title"):
                errors.append("%s: missing title" % where)
            c = dict(c)
            c["_file"] = fname
            contracts[cid] = c
    return contracts, errors


def compute(contracts, cases, results=None):
    """Return (matrix, problems). results: {case_id: record} optional."""
    problems = {"orphan_mappings": [], "gaps": [], "unresolved": [], "uncovered_contracts": [],
                "deferred": [], "duplicate_mappings": []}
    by_contract = {cid: {a: [] for a in ASPECTS} for cid in contracts}
    for case in cases:
        seen = set()
        for entry in case["contracts"]:
            cid = entry["contract"]
            if cid in seen:
                problems["duplicate_mappings"].append("%s maps %s twice" % (case.id, cid))
            seen.add(cid)
            if cid not in contracts:
                problems["orphan_mappings"].append("%s -> %s (no such contract)" % (case.id, cid))
                continue
            for aspect in entry["aspects"]:
                by_contract[cid][aspect].append(case)

    matrix = []
    for cid in sorted(contracts):
        c = contracts[cid]
        row = {"contract": cid, "title": c.get("title"), "status": c.get("status"),
               "family": c.get("family"), "spec_refs": c.get("spec_refs", []),
               "discrepancies": c.get("discrepancies", []), "aspects": {}}
        any_case = False
        for aspect in ASPECTS:
            requirement = c["aspects"][aspect] if isinstance(c.get("aspects"), dict) else "required"
            cs = by_contract[cid][aspect]
            mand = [x for x in cs if x.mandatory]
            any_case = any_case or bool(cs)
            cell = {"requirement": requirement,
                    "cases": [x.id for x in cs],
                    "mandatory_cases": [x.id for x in mand]}
            if results is not None:
                cell["results"] = {x.id: results.get(x.id, {}).get("status", "NOT_RUN") for x in cs}
            if c.get("status") == "mandatory" and requirement == "required" and not mand:
                problems["gaps"].append("%s [%s]" % (cid, aspect))
                cell["gap"] = True
            row["aspects"][aspect] = cell
        if results is not None:
            sts = [results.get(x.id, {}).get("status", "NOT_RUN")
                   for a in ASPECTS for x in by_contract[cid][a] if x.mandatory]
            if not sts:
                row["observed"] = "NO_MANDATORY_CASES"
            elif all(s == "PASS" for s in sts):
                row["observed"] = "ALL_PASS"
            elif any(s == "FAIL" for s in sts):
                row["observed"] = "VIOLATED"
            else:
                row["observed"] = "INCOMPLETE"
        if c.get("status") == "unresolved":
            problems["unresolved"].append("%s (%s)" % (cid, ", ".join(c.get("discrepancies", []))))
        if c.get("status") == "deferred":
            problems["deferred"].append(cid)
        if c.get("status") == "mandatory" and not any_case:
            problems["uncovered_contracts"].append(cid)
        matrix.append(row)
    return matrix, problems
