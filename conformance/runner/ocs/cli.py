"""Command line interface: validate, list, run, coverage, gate, diff, mutate."""

import argparse
import concurrent.futures
import datetime
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time

from . import coverage as cov
from .discovery import discover
from .execute import Adapter, run_case, tree_hashes, sha256_file
from .judge import STATUSES
from .model import CaseError, load_case, load_json
from .profiles import load_profiles, probe_environment

SUITE_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
EXCLUDED_FROM_SUITE_HASH = ("reports", "coverage", "__pycache__", ".pytest_cache", ".target")
EXCLUDED_FILES = (".DS_Store",)


def suite_tree_hash(root=SUITE_ROOT):
    h = hashlib.sha256()
    for dirpath, dirnames, filenames in os.walk(root):
        rel = os.path.relpath(dirpath, root)
        parts = rel.split(os.sep)
        dirnames[:] = sorted(d for d in dirnames if d not in EXCLUDED_FROM_SUITE_HASH)
        if parts[0] in EXCLUDED_FROM_SUITE_HASH:
            continue
        if any(p in EXCLUDED_FROM_SUITE_HASH for p in parts):
            continue
        for fname in sorted(filenames):
            if fname.endswith(".pyc") or fname in EXCLUDED_FILES:
                continue
            full = os.path.join(dirpath, fname)
            h.update(os.path.relpath(full, root).encode("utf-8") + b"\0")
            h.update((os.readlink(full) if os.path.islink(full) else sha256_file(full)).encode() + b"\0")
    return h.hexdigest()


def stdlib_tree_hash(root):
    h = hashlib.sha256()
    count = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for fname in sorted(filenames):
            full = os.path.join(dirpath, fname)
            h.update(os.path.relpath(full, root).encode("utf-8") + b"\0" + sha256_file(full).encode() + b"\0")
            count += 1
    return {"sha256": h.hexdigest(), "files": count}


def git_info(path):
    def g(*args):
        try:
            return subprocess.run(["git", "-C", path] + list(args), stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, timeout=30).stdout.decode().strip()
        except Exception:
            return ""
    head = g("rev-parse", "HEAD")
    if not head:
        return {"git": None}
    dirty = g("status", "--porcelain", "--", path)
    return {"git_head": head, "dirty_paths": len([l for l in dirty.splitlines() if l.strip()])}


def release_meta():
    return load_json(os.path.join(SUITE_ROOT, "release.json"))


def default_stdlib_root():
    env = os.environ.get("OCS_STDLIB_ROOT")
    if env:
        return os.path.abspath(env)
    return os.path.abspath(os.path.join(SUITE_ROOT, release_meta()["stdlib"]["default_root"]))


def cmd_validate(args):
    errors = []
    rep = discover(SUITE_ROOT)
    errors.extend(rep.errors)
    try:
        profiles = load_profiles(SUITE_ROOT)
    except CaseError as exc:
        errors.append(str(exc))
        profiles = {}
    contracts, cerrs = cov.load_contracts(SUITE_ROOT)
    errors.extend(cerrs)
    matrix, problems = cov.compute(contracts, rep.cases)
    errors.extend("orphan contract mapping: " + x for x in problems["orphan_mappings"])
    errors.extend("duplicate contract mapping: " + x for x in problems["duplicate_mappings"])
    gate = [p for p in profiles.values() if p.data["kind"] == "release-gate"]
    if gate:
        members = [profiles[m] for m in gate[0].data["members"]]
        for case in rep.cases:
            if case.mandatory and not any(p.selects(case) for p in members):
                errors.append("mandatory case %s is selected by no release-member profile" % case.id)
    for case in rep.cases:
        for d in case.get("defects", []):
            if not os.path.isfile(os.path.join(SUITE_ROOT, "defects", d + ".md")):
                errors.append("%s references missing defect record %s" % (case.id, d))
    ledger_path = os.path.join(SUITE_ROOT, "contracts", "discrepancy-ledger.json")
    if os.path.isfile(ledger_path):
        import re as _re
        ledger = load_json(ledger_path)
        ids = {e["id"] for e in ledger["entries"]}
        case_ids = {c.id for c in rep.cases}
        for cid, c in contracts.items():
            for d in c.get("discrepancies", []):
                if d not in ids:
                    errors.append("contract %s cites unknown ledger entry %s" % (cid, d))
        for case in rep.cases:
            for d in _re.findall(r"D-[0-9]{3}", case.get("notes", "") or ""):
                if d not in ids:
                    errors.append("case %s cites unknown ledger entry %s" % (case.id, d))
        for e in ledger["entries"]:
            for cid in e.get("cases", []):
                if cid not in case_ids:
                    errors.append("ledger %s cites unknown case %s" % (e["id"], cid))
            for k in e.get("contracts", []):
                if k not in contracts:
                    errors.append("ledger %s cites unknown contract %s" % (e["id"], k))
    else:
        errors.append("contracts/discrepancy-ledger.json is missing")
    print("cases: %d (%d mandatory)" % (len(rep.cases), sum(1 for c in rep.cases if c.mandatory)))
    print("contracts: %d" % len(contracts))
    print("profiles: %s" % ", ".join(sorted(profiles)))
    print("coverage gaps: %d, uncovered mandatory contracts: %d, unresolved contracts: %d" % (
        len(problems["gaps"]), len(problems["uncovered_contracts"]), len(problems["unresolved"])))
    if errors:
        print("VALIDATION FAILED (%d problems):" % len(errors))
        for e in errors:
            print("  - " + e)
        return 1
    print("validation OK")
    return 0


def select_cases(rep, profile, args):
    cases = [c for c in rep.cases if profile.selects(c)] if profile else list(rep.cases)
    if args.case:
        wanted = set(args.case)
        cases = [c for c in cases if c.id in wanted or any(c.id.startswith(w + ".") for w in wanted)]
    if args.level:
        cases = [c for c in cases if c.level in args.level]
    return cases


def cmd_list(args):
    rep = discover(SUITE_ROOT)
    profiles = load_profiles(SUITE_ROOT)
    profile = profiles[args.profile] if args.profile else None
    for c in select_cases(rep, profile, args):
        print("%-60s %-6s %-9s %s" % (c.id, c.mode, c["status"], c["title"]))
    return 0


def cmd_run(args):
    rep = discover(SUITE_ROOT)
    if rep.errors:
        print("refusing to run: suite validation errors (run `ocs validate`)")
        for e in rep.errors:
            print("  - " + e)
        return 2
    profiles = load_profiles(SUITE_ROOT)
    profile = profiles[args.profile]
    if profile.data["kind"] == "release-gate":
        members = [profiles[m] for m in profile.data["members"]]
        selected_all = [c for c in rep.cases if any(m.selects(c) for m in members)]
    else:
        selected_all = [c for c in rep.cases if profile.selects(c)]
    filtered = select_cases(rep, None, args)
    filtered_ids = {c.id for c in filtered}
    to_run = [c for c in selected_all if c.id in filtered_ids]
    overrides = dict(kv.split("=", 1) for kv in (args.adapter_env or []))
    adapter = Adapter(args.adapter, overrides)
    stdlib_root = os.path.abspath(args.stdlib_root) if args.stdlib_root else default_stdlib_root()
    available, unavailable_reasons = probe_environment()
    for tag in args.unavailable or []:
        available.discard(tag)
        unavailable_reasons[tag] = "declared unavailable on the command line"

    stamp = datetime.datetime.utcnow().strftime("%Y%m%dT%H%M%SZ")
    out_dir = os.path.abspath(args.out or os.path.join(SUITE_ROOT, "reports", "runs",
                                                        "%s-%s-%s" % (stamp, adapter.name, profile.name)))
    os.makedirs(out_dir, exist_ok=True)

    class Opts(object):
        pass
    opts = Opts()
    opts.timeout_scale = args.timeout_scale
    opts.keep = args.keep
    opts.tmp_root = args.tmp_root or None

    info = adapter.info(SUITE_ROOT)
    meta = {
        "schema": "ocs-run/1",
        "release": release_meta(),
        "suite_tree_sha256": suite_tree_hash(),
        "suite_git": git_info(SUITE_ROOT),
        "profile": profile.name,
        "profile_sha256": sha256_file(profile.path),
        "profile_members": profile.data.get("members", []),
        "adapter": {"name": adapter.name, "config": os.path.relpath(adapter.path, SUITE_ROOT)
                    if adapter.path.startswith(SUITE_ROOT) else adapter.path,
                    "config_sha256": sha256_file(adapter.path), "overrides": overrides, "info": info},
        "stdlib": {"root": stdlib_root, "tree": stdlib_tree_hash(stdlib_root), "git": git_info(stdlib_root)},
        "environment": {"platform": platform.platform(), "machine": platform.machine(),
                        "python": sys.version.split()[0], "available": sorted(available),
                        "unavailable": unavailable_reasons},
        "timeout_scale": args.timeout_scale,
        "jobs": args.jobs,
        "command": sys.argv,
        "started_utc": stamp,
        "filters": {"case": args.case, "level": args.level},
    }

    records = {}
    blocked = []
    runnable = []
    for c in to_run:
        missing = [t for t in c.environment if t not in available]
        if missing:
            records[c.id] = {"id": c.id, "level": c.level, "mode": c.mode, "status_class": c["status"],
                             "contracts": c["contracts"], "status": "BLOCKED",
                             "reasons": ["environment unavailable: %s (%s)" % (
                                 t, unavailable_reasons.get(t, "")) for t in missing]}
        else:
            runnable.append(c)
    started = time.time()
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        futs = {pool.submit(run_case, c, adapter, SUITE_ROOT, stdlib_root, opts): c for c in runnable}
        done = 0
        for fut in concurrent.futures.as_completed(futs):
            c = futs[fut]
            try:
                rec = fut.result()
            except Exception as exc:
                rec = {"id": c.id, "status": "HARNESS_ERROR", "reasons": ["runner crash: %r" % (exc,)]}
            records[c.id] = rec
            done += 1
            if not args.quiet:
                print("[%4d/%4d] %-13s %s%s" % (done, len(runnable), rec["status"], c.id,
                                                ("  -- " + rec["reasons"][0][:160]) if rec.get("reasons") else ""))
                sys.stdout.flush()
    for c in selected_all:
        if c.id not in records:
            records[c.id] = {"id": c.id, "level": c.level, "mode": c.mode, "status_class": c["status"],
                             "contracts": c["contracts"], "status": "NOT_RUN",
                             "reasons": ["selected by profile but excluded by command-line filters"]}
    meta["elapsed_s"] = round(time.time() - started, 2)
    write_run(out_dir, meta, records, rep.cases)
    print("results: %s" % out_dir)
    summary = load_json(os.path.join(out_dir, "summary.json"))
    print(summary_line(summary))
    return 0 if summary["mandatory"]["counts"].get("PASS", 0) == summary["mandatory"]["total"] else 1


def summary_line(summary):
    m = summary["mandatory"]
    parts = ["%d %s" % (m["counts"].get(s, 0), s) for s in STATUSES if m["counts"].get(s)]
    return "mandatory cases in profile %s: %d/%d PASS (%s)" % (
        summary["profile"], m["counts"].get("PASS", 0), m["total"], ", ".join(parts))


def write_run(out_dir, meta, records, all_cases):
    case_by_id = {c.id: c for c in all_cases}
    with open(os.path.join(out_dir, "results.jsonl"), "w") as fh:
        for cid in sorted(records):
            fh.write(json.dumps(records[cid], sort_keys=True) + "\n")
    def count(ids):
        out = {}
        for i in ids:
            s = records[i]["status"]
            out[s] = out.get(s, 0) + 1
        return out
    mand = [i for i in records if case_by_id.get(i) is not None and case_by_id[i].mandatory]
    other = [i for i in records if i not in mand]
    by_level = {}
    for i in mand:
        lv = records[i].get("level", i[:3])
        by_level.setdefault(lv, []).append(i)
    summary = dict(meta)
    summary.update({
        "mandatory": {"total": len(mand), "counts": count(mand)},
        "non_mandatory": {"total": len(other), "counts": count(other)},
        "by_level": {lv: {"total": len(ids), "counts": count(ids)} for lv, ids in sorted(by_level.items())},
        "non_pass": sorted(([i, records[i]["status"], (records[i].get("reasons") or [""])[0][:300]]
                            for i in records if records[i]["status"] != "PASS"), key=lambda x: x[0]),
    })
    with open(os.path.join(out_dir, "summary.json"), "w") as fh:
        json.dump(summary, fh, indent=2, sort_keys=True)
    with open(os.path.join(out_dir, "summary.md"), "w") as fh:
        fh.write(render_summary_md(summary))


def render_summary_md(s):
    lines = ["# Conformance run: %s via %s" % (s["profile"], s["adapter"]["name"]), ""]
    lines.append("- Suite version: %s (tree sha256 `%s`, git %s)" % (
        s["release"]["suite_version"], s["suite_tree_sha256"][:16], s["suite_git"]))
    lines.append("- Language contract: %s" % s["release"]["language_contract"]["name"])
    lines.append("- Standard library: `%s` (tree sha256 `%s`, %d files)" % (
        s["stdlib"]["root"], s["stdlib"]["tree"]["sha256"][:16], s["stdlib"]["tree"]["files"]))
    info = s["adapter"]["info"]
    lines.append("- Implementation: %s" % json.dumps({k: info.get(k) for k in ("implementation", "revision", "configuration", "error") if k in info}, sort_keys=True))
    lines.append("- Environment: %s, python %s" % (s["environment"]["platform"], s["environment"]["python"]))
    lines.append("- Timeout scale: %s, jobs: %s, elapsed: %ss" % (s["timeout_scale"], s["jobs"], s["elapsed_s"]))
    lines.append("")
    lines.append("**" + summary_line(s) + "**")
    lines.append("")
    lines.append("This is a profile run report, not a release-gate verdict; see `ocs gate`.")
    lines.append("")
    lines.append("| Level | Mandatory | " + " | ".join(STATUSES) + " |")
    lines.append("|---|---|" + "---|" * len(STATUSES))
    for lv, v in sorted(s["by_level"].items()):
        lines.append("| %s | %d | %s |" % (lv, v["total"], " | ".join(str(v["counts"].get(st, 0)) for st in STATUSES)))
    lines.append("")
    if s["non_pass"]:
        lines.append("## Non-passing cases")
        lines.append("")
        for cid, st, why in s["non_pass"]:
            lines.append("- `%s` **%s** — %s" % (cid, st, why.replace("\n", " ")))
    lines.append("")
    return "\n".join(lines)


def load_results(dirs):
    merged = {}
    runs = []
    for d in dirs:
        summary = load_json(os.path.join(d, "summary.json"))
        runs.append(summary)
        with open(os.path.join(d, "results.jsonl")) as fh:
            for line in fh:
                rec = json.loads(line)
                merged[rec["id"]] = rec
    return merged, runs


def cmd_coverage(args):
    rep = discover(SUITE_ROOT)
    contracts, cerrs = cov.load_contracts(SUITE_ROOT)
    results = None
    if args.results:
        results, _ = load_results(args.results)
    matrix, problems = cov.compute(contracts, rep.cases, results)
    out = os.path.abspath(args.out or os.path.join(SUITE_ROOT, "coverage"))
    os.makedirs(out, exist_ok=True)
    doc = {"schema": "ocs-coverage/1", "suite_tree_sha256": suite_tree_hash(),
           "contract_errors": cerrs, "problems": problems, "matrix": matrix,
           "results_from": args.results or []}
    with open(os.path.join(out, "matrix.json"), "w") as fh:
        json.dump(doc, fh, indent=1, sort_keys=True)
    with open(os.path.join(out, "COVERAGE.md"), "w") as fh:
        fh.write(render_coverage_md(doc, contracts))
    print("contracts: %d; gaps: %d; uncovered mandatory contracts: %d; unresolved: %d; orphan mappings: %d; contract errors: %d" % (
        len(contracts), len(problems["gaps"]), len(problems["uncovered_contracts"]),
        len(problems["unresolved"]), len(problems["orphan_mappings"]), len(cerrs)))
    complete = not (problems["gaps"] or problems["uncovered_contracts"] or problems["unresolved"]
                    or problems["orphan_mappings"] or cerrs or problems["duplicate_mappings"])
    print("coverage obligations %s" % ("COMPLETE" if complete else "INCOMPLETE"))
    return 0 if complete else 1


def render_coverage_md(doc, contracts):
    p = doc["problems"]
    lines = ["# Coverage matrix", "", "Generated by `ocs coverage`; do not edit by hand.", ""]
    lines.append("- Contracts: %d" % len(doc["matrix"]))
    for key in ("gaps", "uncovered_contracts", "unresolved", "deferred", "orphan_mappings", "duplicate_mappings"):
        lines.append("- %s: %d" % (key.replace("_", " "), len(p[key])))
    lines.append("")
    fams = {}
    for row in doc["matrix"]:
        fams.setdefault(row.get("family") or row["contract"].split(".")[0], []).append(row)
    for fam in sorted(fams):
        lines.append("## %s" % fam)
        lines.append("")
        lines.append("| Contract | Status | normal | boundary | invalid | interaction | Observed |")
        lines.append("|---|---|---|---|---|---|---|")
        for row in fams[fam]:
            cells = []
            for a in ("normal", "boundary", "invalid", "interaction"):
                cell = row["aspects"][a]
                if cell["requirement"] != "required":
                    cells.append("n/a")
                elif cell.get("gap"):
                    cells.append("**GAP**")
                else:
                    cells.append(str(len(cell["mandatory_cases"])))
            lines.append("| `%s` %s | %s | %s | %s |" % (row["contract"], row["title"], row["status"],
                                                        " | ".join(cells), row.get("observed", "")))
        lines.append("")
    if p["gaps"]:
        lines.append("## Required coverage gaps")
        lines.append("")
        lines.extend("- " + g for g in p["gaps"])
        lines.append("")
    if p["unresolved"]:
        lines.append("## Unresolved contracts (block the release gate)")
        lines.append("")
        lines.extend("- " + g for g in p["unresolved"])
        lines.append("")
    return "\n".join(lines)


def cmd_gate(args):
    rep = discover(SUITE_ROOT)
    profiles = load_profiles(SUITE_ROOT)
    gate = profiles[args.profile]
    contracts, cerrs = cov.load_contracts(SUITE_ROOT)
    results, runs = load_results(args.results)
    reasons = []
    current = suite_tree_hash()
    hashes = {r["suite_tree_sha256"] for r in runs}
    if hashes != {current}:
        reasons.append("results come from suite trees %s but the current suite tree is %s (expectations must be identical)" % (
            sorted(h[:12] for h in hashes), current[:12]))
    adapters = {r["adapter"]["name"] for r in runs}
    if len(adapters) != 1:
        reasons.append("results mix adapters %s" % sorted(adapters))
    is_release = gate.data["kind"] == "release-gate"
    members = gate.data.get("members", [gate.name])
    if is_release:
        exercised = set()
        for r in runs:
            exercised.add(r["profile"])
            exercised.update(r.get("profile_members", []))
        missing_profiles = [m for m in members if m not in exercised and gate.name not in exercised]
        if missing_profiles:
            reasons.append("mandatory profiles not exercised: %s" % ", ".join(missing_profiles))
    member_profiles = [profiles[m] for m in members]
    mandatory = [c for c in rep.cases if c.mandatory and any(p.selects(c) for p in member_profiles)]
    counts = {}
    for c in mandatory:
        st = results.get(c.id, {}).get("status", "NOT_RUN")
        counts[st] = counts.get(st, 0) + 1
    passed = counts.get("PASS", 0)
    if passed != len(mandatory):
        reasons.append("mandatory cases: %d/%d PASS (%s)" % (passed, len(mandatory), ", ".join(
            "%d %s" % (counts[s], s) for s in STATUSES if s != "PASS" and counts.get(s))))
    matrix, problems = cov.compute(contracts, rep.cases, results)
    if is_release and (cerrs or problems["gaps"] or problems["uncovered_contracts"] or problems["orphan_mappings"]):
        reasons.append("coverage obligations incomplete: %d aspect gaps, %d uncovered mandatory contracts, %d orphan mappings, %d contract errors" % (
            len(problems["gaps"]), len(problems["uncovered_contracts"]), len(problems["orphan_mappings"]), len(cerrs)))
    if is_release and problems["unresolved"]:
        reasons.append("%d unresolved normative contracts" % len(problems["unresolved"]))
    rel = release_meta()
    claim = {
        "suite_version": rel["suite_version"], "suite_tree_sha256": current,
        "language_contract": rel["language_contract"], "stdlib": [r["stdlib"] for r in runs][:1],
        "gate_profile": gate.name, "member_profiles": members,
        "implementation": [r["adapter"] for r in runs][:1],
        "environment": [r["environment"]["platform"] for r in runs],
        "mandatory_cases": len(mandatory), "counts": counts,
    }
    verdict = "PASSED" if not reasons else "NOT PASSED"
    doc = {"schema": "ocs-gate/1", "gate": gate.name, "kind": gate.data["kind"], "verdict": verdict,
           "reasons": reasons, "claim": claim}
    if args.out:
        with open(args.out, "w") as fh:
            json.dump(doc, fh, indent=2, sort_keys=True)
    label = "Release gate" if gate.data["kind"] == "release-gate" else "Profile %s (%s, not a release claim)" % (gate.name, gate.data["kind"])
    print("%s %s: %s" % (label, gate.name, verdict))
    for r in reasons:
        print("  - " + r)
    return 0 if not reasons else 1


DIFF_CLASSES = ("both-conform", "left-only-conforms", "right-only-conforms", "both-violate",
                "contract-unresolved", "execution-unavailable")


def cmd_diff(args):
    rep = discover(SUITE_ROOT)
    left, lruns = load_results([args.left])
    right, rruns = load_results([args.right])
    if lruns[0]["suite_tree_sha256"] != rruns[0]["suite_tree_sha256"]:
        print("warning: the two runs used different suite trees; comparison is not differential")
    unavailable = ("UNSUPPORTED", "BLOCKED", "NOT_RUN")
    rows = []
    counts = {k: 0 for k in DIFF_CLASSES}
    for c in rep.cases:
        if c.id not in left and c.id not in right:
            continue
        ls = left.get(c.id, {}).get("status", "NOT_RUN")
        rs = right.get(c.id, {}).get("status", "NOT_RUN")
        if c["status"] == "unresolved":
            cls = "contract-unresolved"
        elif ls in unavailable or rs in unavailable:
            cls = "execution-unavailable"
        elif ls == "PASS" and rs == "PASS":
            cls = "both-conform"
        elif ls == "PASS":
            cls = "left-only-conforms"
        elif rs == "PASS":
            cls = "right-only-conforms"
        else:
            cls = "both-violate"
        counts[cls] += 1
        rows.append({"id": c.id, "left": ls, "right": rs, "class": cls})
    doc = {"schema": "ocs-diff/1", "left": {"adapter": lruns[0]["adapter"]["name"], "dir": args.left},
           "right": {"adapter": rruns[0]["adapter"]["name"], "dir": args.right},
           "counts": counts, "rows": rows,
           "note": "Agreement between engines is not evidence of correctness; every row is judged against the shared expectations."}
    out = args.out or os.path.join(SUITE_ROOT, "reports", "differential.json")
    with open(out, "w") as fh:
        json.dump(doc, fh, indent=2, sort_keys=True)
    for k in DIFF_CLASSES:
        print("%-24s %d" % (k, counts[k]))
    print("written: %s" % out)
    return 0


def cmd_mutate(args):
    """Apply recorded mutations to copies of cases; every mutant must not PASS."""
    spec = load_json(args.mutations)
    adapter = Adapter(args.adapter)
    stdlib_root = default_stdlib_root()
    rep = discover(SUITE_ROOT)
    by_id = {c.id: c for c in rep.cases}

    class Opts(object):
        timeout_scale = args.timeout_scale
        keep = "none"
        tmp_root = None
    rows = []
    survived = 0
    for m in spec["mutations"]:
        base = by_id[m["case"]]
        tmp = tempfile.mkdtemp(prefix="ocs-mutant-")
        mdir = os.path.join(tmp, os.path.basename(base.dir))
        shutil.copytree(base.dir, mdir)
        target = os.path.join(mdir, m["file"])
        with open(target, "rb") as fh:
            text = fh.read().decode("utf-8")
        if text.count(m["find"]) != 1:
            rows.append({"mutation": m["name"], "result": "INVALID", "why": "find text must occur exactly once"})
            survived += 1
            continue
        with open(target, "wb") as fh:
            fh.write(text.replace(m["find"], m["replace"]).encode("utf-8"))
        try:
            case = load_case(mdir)
        except CaseError as exc:
            rows.append({"mutation": m["name"], "kind": m["kind"], "result": "KILLED",
                         "status": "INVALID_CASE", "why": str(exc)[:300]})
            shutil.rmtree(tmp, ignore_errors=True)
            continue
        rec = run_case(case, adapter, SUITE_ROOT, stdlib_root, Opts())
        killed = rec["status"] != "PASS"
        if not killed:
            survived += 1
        rows.append({"mutation": m["name"], "kind": m["kind"], "case": m["case"],
                     "result": "KILLED" if killed else "SURVIVED", "status": rec["status"],
                     "why": (rec.get("reasons") or [""])[0][:300]})
        print("%-9s %-50s %s" % (rows[-1]["result"], m["name"], rows[-1]["why"][:120]))
        shutil.rmtree(tmp, ignore_errors=True)
    doc = {"schema": "ocs-mutation/1", "adapter": adapter.name, "suite_tree_sha256": suite_tree_hash(),
           "rows": rows, "survived": survived, "total": len(rows)}
    out = args.out or os.path.join(SUITE_ROOT, "reports", "mutation-%s.json" % adapter.name)
    with open(out, "w") as fh:
        json.dump(doc, fh, indent=2, sort_keys=True)
    print("mutants: %d, survived: %d -> %s" % (len(rows), survived, out))
    return 0 if survived == 0 else 1


def main(argv=None):
    ap = argparse.ArgumentParser(prog="ocs", description="Okkama Conformance Suite runner")
    sub = ap.add_subparsers(dest="cmd")
    sub.add_parser("validate", help="validate cases, contracts and profiles")
    p = sub.add_parser("list", help="list cases")
    p.add_argument("--profile")
    p.add_argument("--case", action="append")
    p.add_argument("--level", action="append")
    p = sub.add_parser("run", help="run a profile through an adapter")
    p.add_argument("--adapter", required=True)
    p.add_argument("--profile", required=True)
    p.add_argument("--case", action="append", help="case id or id prefix (repeatable)")
    p.add_argument("--level", action="append")
    p.add_argument("-j", "--jobs", type=int, default=4)
    p.add_argument("--timeout-scale", type=float, default=1.0)
    p.add_argument("--keep", choices=("none", "failed", "all"), default="failed")
    p.add_argument("--out")
    p.add_argument("--tmp-root")
    p.add_argument("--stdlib-root")
    p.add_argument("--adapter-env", action="append", help="KEY=VALUE override for the adapter launch environment")
    p.add_argument("--unavailable", action="append", help="declare an environment tag unavailable")
    p.add_argument("--quiet", action="store_true")
    p = sub.add_parser("coverage", help="compute contract coverage")
    p.add_argument("--results", action="append")
    p.add_argument("--out")
    p = sub.add_parser("gate", help="evaluate a release gate or milestone profile")
    p.add_argument("--profile", default="full-release")
    p.add_argument("--results", action="append", required=True)
    p.add_argument("--out")
    p = sub.add_parser("diff", help="differential comparison of two runs")
    p.add_argument("--left", required=True)
    p.add_argument("--right", required=True)
    p.add_argument("--out")
    p = sub.add_parser("mutate", help="run mutation controls")
    p.add_argument("--adapter", required=True)
    p.add_argument("--mutations", default=os.path.join(SUITE_ROOT, "runner-tests", "mutations.json"))
    p.add_argument("--timeout-scale", type=float, default=1.0)
    p.add_argument("--out")
    args = ap.parse_args(argv)
    if not args.cmd:
        ap.print_help()
        return 2
    return {"validate": cmd_validate, "list": cmd_list, "run": cmd_run, "coverage": cmd_coverage,
            "gate": cmd_gate, "diff": cmd_diff, "mutate": cmd_mutate}[args.cmd](args)
