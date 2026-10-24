"""Case discovery with explicit checks for undiscovered or orphaned sources.

A *case directory* is a directory containing ``case.json``. Every ``.ok``
file below ``cases/`` must belong to exactly one case directory and be listed
by that case as a source or support file. This prevents two silent failure
modes: a mandatory program that is never run because its metadata is missing,
and an imported/deliberately-invalid support module that is accidentally
executed as a standalone test.
"""

import os

from .model import LEVELS, CaseError, load_case


class DiscoveryReport(object):
    def __init__(self):
        self.cases = []
        self.errors = []

    @property
    def ok(self):
        return not self.errors


def _case_root_for(path, cases_root):
    """Nearest ancestor directory (inclusive) holding case.json, or None."""
    current = os.path.dirname(path)
    cases_root = os.path.abspath(cases_root)
    while True:
        if os.path.isfile(os.path.join(current, "case.json")):
            return current
        if os.path.abspath(current) == cases_root or len(current) <= len(cases_root):
            return None
        current = os.path.dirname(current)


def discover(suite_root):
    report = DiscoveryReport()
    cases_root = os.path.join(suite_root, "cases")
    seen_ids = {}
    case_dirs = []

    for name in sorted(os.listdir(cases_root)):
        full = os.path.join(cases_root, name)
        if not os.path.isdir(full):
            if not name.startswith("."):
                report.errors.append("stray file in cases/: %s" % name)
            continue
        if name not in LEVELS.values():
            report.errors.append("unknown level directory cases/%s" % name)
            continue

    for dirpath, dirnames, filenames in os.walk(cases_root):
        dirnames.sort()
        if "case.json" in filenames:
            case_dirs.append(dirpath)

    for case_dir in case_dirs:
        rel = os.path.relpath(case_dir, cases_root)
        level_dir = rel.split(os.sep)[0]
        # Case directories must not nest: a nested case.json would make the
        # outer case's support files ambiguous.
        parent = os.path.dirname(case_dir)
        while len(parent) > len(cases_root):
            if os.path.isfile(os.path.join(parent, "case.json")):
                report.errors.append("nested case directory %s inside %s" % (rel, os.path.relpath(parent, cases_root)))
                break
            parent = os.path.dirname(parent)
        try:
            case = load_case(case_dir, level_dir)
        except CaseError as exc:
            report.errors.append(str(exc))
            continue
        if case.id in seen_ids:
            report.errors.append("duplicate case id %s in %s and %s" % (
                case.id, os.path.relpath(seen_ids[case.id], cases_root), rel))
            continue
        seen_ids[case.id] = case_dir
        report.cases.append(case)

    by_dir = {os.path.abspath(c.dir): c for c in report.cases}
    for dirpath, dirnames, filenames in os.walk(cases_root):
        for fname in filenames:
            if not fname.endswith(".ok"):
                continue
            full = os.path.join(dirpath, fname)
            owner = _case_root_for(full, cases_root)
            relfile = os.path.relpath(full, cases_root)
            if owner is None:
                report.errors.append("undiscovered Okkama source (no case.json): %s" % relfile)
                continue
            case = by_dir.get(os.path.abspath(owner))
            if case is None:
                continue  # owner case already reported as invalid
            inside = os.path.relpath(full, owner).replace(os.sep, "/")
            listed = set(case["sources"]) | set(case.get("support", []))
            covered = inside in listed or any(
                inside.startswith(s.rstrip("/") + "/") for s in case.get("support", []))
            if not covered:
                report.errors.append("source %s is neither a declared source nor support file of %s" % (relfile, case.id))

    report.cases.sort(key=lambda c: c.id)
    return report
