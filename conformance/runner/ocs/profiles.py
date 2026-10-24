"""Fixed, reviewable profile definitions and case selection."""

import os
import socket

from .model import ENVIRONMENT_TAGS, LEVELS, MODES, PROFILE_SCHEMA, CaseError, load_json


class Profile(object):
    def __init__(self, path, data):
        self.path = path
        self.data = data
        self.name = data["name"]

    def selects(self, case):
        sel = self.data["select"]
        if case.level not in sel["levels"]:
            return False
        if case.mode not in sel.get("modes", list(MODES)):
            return False
        allowed = sel.get("environment_within")
        if allowed is not None and not set(case.environment) <= set(allowed):
            return False
        any_of = sel.get("environment_any_of")
        if any_of is not None and not set(case.environment) & set(any_of):
            return False
        statuses = sel.get("statuses")
        if statuses is not None:
            return case["status"] in statuses
        if sel.get("mandatory_only", True) and not case.mandatory:
            return False
        return True


def validate_profile(data, path):
    def req(c, m):
        if not c:
            raise CaseError("%s: %s" % (path, m))
    req(data.get("schema") == PROFILE_SCHEMA, "schema must be %s" % PROFILE_SCHEMA)
    for key in ("name", "description", "select", "kind"):
        req(key in data, "missing %s" % key)
    req(data["kind"] in ("milestone", "release-member", "release-gate", "informative", "benchmark"),
        "kind must be milestone/release-member/release-gate/informative/benchmark")
    sel = data["select"]
    req(isinstance(sel.get("levels"), list) and sel["levels"] and all(l in LEVELS for l in sel["levels"]),
        "select.levels must list known levels")
    for m in sel.get("modes", []):
        req(m in MODES, "unknown mode %s" % m)
    for t in sel.get("environment_within", []) or []:
        req(t in ENVIRONMENT_TAGS, "unknown environment tag %s" % t)
    for t in sel.get("environment_any_of", []) or []:
        req(t in ENVIRONMENT_TAGS, "unknown environment tag %s" % t)
    for st in sel.get("statuses", []) or []:
        req(st in ("mandatory", "informative", "unresolved"), "unknown case status %s" % st)
    req(set(sel) <= {"levels", "modes", "environment_within", "environment_any_of", "mandatory_only", "statuses"},
        "unknown select keys")
    if data["kind"] == "release-gate":
        req(isinstance(data.get("members"), list) and data["members"], "release gate must list member profiles")


def load_profiles(suite_root):
    out = {}
    pdir = os.path.join(suite_root, "profiles")
    for fname in sorted(os.listdir(pdir)):
        if not fname.endswith(".json"):
            continue
        path = os.path.join(pdir, fname)
        data = load_json(path)
        validate_profile(data, path)
        if data["name"] in out:
            raise CaseError("duplicate profile name %s" % data["name"])
        if fname != data["name"] + ".json":
            raise CaseError("%s: file name must be <name>.json" % path)
        out[data["name"]] = Profile(path, data)
    for p in out.values():
        for m in p.data.get("members", []):
            if m not in out:
                raise CaseError("%s: unknown member profile %s" % (p.path, m))
    return out


def probe_environment():
    """Best-effort host probe. Unavailable requirements BLOCK cases."""
    available = set(ENVIRONMENT_TAGS)
    reasons = {}
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.bind(("127.0.0.1", 0))
        s.close()
    except OSError as exc:
        available.discard("loopback-network")
        available.discard("tls")
        reasons["loopback-network"] = "cannot bind 127.0.0.1: %s" % exc
    try:
        import ssl  # noqa: F401
    except ImportError:
        available.discard("tls")
        reasons["tls"] = "python ssl module unavailable for TLS fixtures"
    return available, reasons
