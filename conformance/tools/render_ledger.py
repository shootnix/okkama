#!/usr/bin/env python3
"""Render contracts/discrepancy-ledger.json (canonical) as contracts/DISCREPANCY-LEDGER.md."""
import json
import os
import sys

SUITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    doc = json.load(open(os.path.join(SUITE, "contracts", "discrepancy-ledger.json")))
    out = ["# Discrepancy ledger", "",
           "Generated from `contracts/discrepancy-ledger.json` by `tools/render_ledger.py`; edit the JSON.", "",
           doc["description"], "",
           "Implementation evidence: " + doc["implementation_observed"] + ". The interpreter is evidence, not an oracle:",
           "an entry is resolved only by a cited authority, never because an implementation behaves a certain way.", ""]
    counts = {}
    for e in doc["entries"]:
        key = e["resolution"].split(" ")[0]
        counts[key] = counts.get(key, 0) + 1
    out.append("Summary: " + ", ".join("%d %s" % (v, k) for k, v in sorted(counts.items())) + ".")
    out.append("")
    out.append("| ID | Subject | Resolution |")
    out.append("|---|---|---|")
    for e in doc["entries"]:
        out.append("| [%s](#%s) | %s | %s |" % (e["id"], e["id"].lower(), e["subject"], e["resolution"]))
    out.append("")
    for e in doc["entries"]:
        out.append("## %s" % e["id"])
        out.append("")
        out.append("**%s**" % e["subject"])
        out.append("")
        out.append("- Sources: " + "; ".join("`%s`" % s if s.startswith(("docs/", "std/", "okkama-pl", "improvements", "stdlib", "research")) else s for s in e["sources"]))
        out.append("- Observed: " + e["observed"])
        out.append("- Intended contract: " + e["intended_contract"])
        out.append("- Authority: " + e["authority"])
        out.append("- Conformance expectation: " + e["conformance_expectation"])
        out.append("- Resolution: **%s**" % e["resolution"])
        if e["contracts"]:
            out.append("- Contracts: " + ", ".join("`%s`" % c for c in e["contracts"]))
        if e["cases"]:
            out.append("- Cases: " + ", ".join("`%s`" % c for c in e["cases"]))
        out.append("")
    with open(os.path.join(SUITE, "contracts", "DISCREPANCY-LEDGER.md"), "w") as fh:
        fh.write("\n".join(out))
    print("rendered %d entries" % len(doc["entries"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
