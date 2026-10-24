"""Case metadata model and validation for the Okkama Conformance Suite.

Only the Python standard library is used. The JSON Schemas under ``schema/``
document the same rules for other tooling; this module is the executable
validator the runner relies on, so the two must be kept in sync
(``runner-tests/test_schema_sync.py`` checks the enumerations).
"""

import base64
import json
import os
import re

CASE_SCHEMA = "ocs-case/1"
CONTRACTS_SCHEMA = "ocs-contracts/1"
PROFILE_SCHEMA = "ocs-profile/1"

LEVELS = {
    "L00": "L00-bootstrap",
    "L01": "L01-lexical-and-grammar",
    "L02": "L02-values-and-operators",
    "L03": "L03-control-flow-and-calls",
    "L04": "L04-objects-roles-caps-successors",
    "L05": "L05-types-generics-and-collections",
    "L06": "L06-modules-and-native-boundaries",
    "L07": "L07-errors-cleanup-and-lifetimes",
    "L08": "L08-async-and-concurrency",
    "L09": "L09-stdlib-pure",
    "L10": "L10-stdlib-host-resources",
    "L11": "L11-protocols-and-persistence",
    "L12": "L12-cross-feature-adversarial",
    "L13": "L13-application-scenarios",
}

# Execution modes the adapter protocol defines.
#   parse - lexical + syntactic acceptance of the listed sources only
#   check - parse + static analysis (imports, declarations, static rules);
#           never executes program code
#   run   - production execution of the entry point
#   test  - embedded test execution (the language's `-t` contract)
MODES = ("parse", "check", "run", "test")

OUTCOMES = ("success", "reject", "failure", "error")

# Stages at which an implementation may report a rejection or failure.
STAGES = ("parse", "check", "runtime")

# Logical error categories. They are deliberately coarse: exact diagnostic
# prose, stack traces and internal exception classes are never normative.
CATEGORIES = (
    "syntax",          # lexical or grammatical rejection
    "semantic",        # static rejection of syntactically valid source
    "resolution",      # module lookup, import selection, cycle, native link
    "runtime",         # uncatchable programming/runtime error (not a panic)
    "panic",           # explicit panic / confirm ... else panic
    "uncaught-error",  # a typed Error value escaped the entry point
)

ASPECTS = ("normal", "boundary", "invalid", "interaction")

CASE_STATUSES = ("mandatory", "informative", "unresolved")

# Environment requirements. A profile that selects a case must provide every
# requirement or the case is BLOCKED (never silently skipped).
ENVIRONMENT_TAGS = (
    "stdout", "stderr", "stdin", "args", "env-vars", "filesystem",
    "process-spawn", "worker-process", "real-clock", "real-random",
    "loopback-network", "tls", "sqlite", "unicode-data", "locale-data",
    "timezone-data",
)

ID_RE = re.compile(r"^L[0-9]{2}(\.[a-z0-9][a-z0-9_-]*)+$")
CONTRACT_RE = re.compile(r"^[A-Z][A-Z0-9]*(\.[A-Z0-9][A-Z0-9_-]*)+$")

CASE_KEYS_REQUIRED = (
    "schema", "id", "title", "purpose", "level", "contracts", "mode",
    "sources", "entry", "expect", "spec_refs", "budget", "provenance",
    "status",
)
CASE_KEYS_OPTIONAL = (
    "support", "args", "stdin", "requires", "harness", "env", "notes",
    "defects", "features",
)


class CaseError(Exception):
    """A case definition is invalid. Invalid cases are never executed."""


class Case(object):
    def __init__(self, path, data):
        self.dir = path
        self.data = data

    def __getitem__(self, key):
        return self.data[key]

    def get(self, key, default=None):
        return self.data.get(key, default)

    @property
    def id(self):
        return self.data["id"]

    @property
    def level(self):
        return self.data["level"]

    @property
    def mode(self):
        return self.data["mode"]

    @property
    def mandatory(self):
        return self.data["status"] == "mandatory"

    @property
    def environment(self):
        return list(self.data.get("requires", {}).get("environment", []))

    @property
    def fixtures(self):
        return list(self.data.get("requires", {}).get("fixtures", []))

    def contract_ids(self):
        return [entry["contract"] for entry in self.data["contracts"]]


def _require(condition, message):
    if not condition:
        raise CaseError(message)


def _is_rel_path(value):
    return (isinstance(value, str) and value and not os.path.isabs(value)
            and ".." not in value.replace("\\", "/").split("/"))


def validate_output_spec(spec, where, case_dir, allow_ignore):
    """Validate a stdout/stderr expectation."""
    if spec == "ignore":
        _require(allow_ignore, "%s: 'ignore' is only allowed for reject/failure/error outcomes" % where)
        return
    _require(isinstance(spec, dict), "%s must be an object or 'ignore'" % where)
    kinds = [k for k in ("text", "file", "bytes_b64", "partial_order") if k in spec]
    _require(len(kinds) == 1, "%s needs exactly one of text/file/bytes_b64/partial_order" % where)
    extra = set(spec) - {"text", "file", "bytes_b64", "partial_order", "justification"}
    _require(not extra, "%s has unknown keys %s" % (where, sorted(extra)))
    kind = kinds[0]
    if kind == "text":
        _require(isinstance(spec["text"], str), "%s.text must be a string" % where)
    elif kind == "file":
        _require(_is_rel_path(spec["file"]), "%s.file must be a relative path" % where)
        _require(os.path.isfile(os.path.join(case_dir, spec["file"])),
                 "%s.file %s does not exist" % (where, spec["file"]))
    elif kind == "bytes_b64":
        try:
            base64.b64decode(spec["bytes_b64"], validate=True)
        except Exception:
            raise CaseError("%s.bytes_b64 is not valid base64" % where)
    else:
        po = spec["partial_order"]
        _require(isinstance(po, dict) and isinstance(po.get("lines"), list),
                 "%s.partial_order needs a 'lines' list" % where)
        _require(all(isinstance(x, str) for x in po["lines"]),
                 "%s.partial_order.lines must be strings" % where)
        before = po.get("before", [])
        _require(isinstance(before, list) and all(
            isinstance(p, list) and len(p) == 2 and all(isinstance(x, str) for x in p)
            for p in before), "%s.partial_order.before must be [[a, b], ...]" % where)
        for a, b in before:
            _require(a in po["lines"] and b in po["lines"],
                     "%s.partial_order.before refers to an unknown line" % where)
        _require(isinstance(spec.get("justification"), str) and spec["justification"],
                 "%s.partial_order requires a justification" % where)


def validate_case_data(data, case_dir, level_dir_name=None):
    _require(isinstance(data, dict), "case.json must contain an object")
    missing = [k for k in CASE_KEYS_REQUIRED if k not in data]
    _require(not missing, "missing required keys: %s" % ", ".join(missing))
    unknown = set(data) - set(CASE_KEYS_REQUIRED) - set(CASE_KEYS_OPTIONAL)
    _require(not unknown, "unknown keys: %s" % ", ".join(sorted(unknown)))
    _require(data["schema"] == CASE_SCHEMA, "schema must be %s" % CASE_SCHEMA)

    cid = data["id"]
    _require(isinstance(cid, str) and ID_RE.match(cid), "invalid id %r" % (cid,))
    _require(data["level"] in LEVELS, "unknown level %r" % (data["level"],))
    _require(cid.startswith(data["level"] + "."), "id must start with its level")
    if level_dir_name is not None:
        _require(LEVELS[data["level"]] == level_dir_name,
                 "level %s does not match directory %s" % (data["level"], level_dir_name))
    for key in ("title", "purpose"):
        _require(isinstance(data[key], str) and data[key].strip(), "%s must be a non-empty string" % key)

    contracts = data["contracts"]
    _require(isinstance(contracts, list) and contracts, "contracts must be a non-empty list")
    for entry in contracts:
        _require(isinstance(entry, dict) and set(entry) <= {"contract", "aspects"},
                 "contract entries are {contract, aspects}")
        _require(isinstance(entry.get("contract"), str) and CONTRACT_RE.match(entry["contract"]),
                 "invalid contract id %r" % (entry.get("contract"),))
        aspects = entry.get("aspects")
        _require(isinstance(aspects, list) and aspects and all(a in ASPECTS for a in aspects),
                 "contract %s: aspects must be a non-empty subset of %s" % (entry.get("contract"), ASPECTS))

    _require(data["mode"] in MODES, "mode must be one of %s" % (MODES,))
    _require(data["status"] in CASE_STATUSES, "status must be one of %s" % (CASE_STATUSES,))

    sources = data["sources"]
    _require(isinstance(sources, list) and sources, "sources must be a non-empty list")
    for src in sources:
        _require(_is_rel_path(src) and src.endswith(".ok"), "invalid source path %r" % (src,))
        _require(os.path.isfile(os.path.join(case_dir, src)), "source %s does not exist" % src)
    _require(data["entry"] == sources[0], "entry must be the first listed source")
    _require(len(set(sources)) == len(sources), "duplicate source path")
    for src in data.get("support", []):
        _require(_is_rel_path(src), "invalid support path %r" % (src,))
        _require(os.path.exists(os.path.join(case_dir, src)), "support path %s does not exist" % src)

    args = data.get("args", [])
    _require(isinstance(args, list) and all(isinstance(a, str) for a in args), "args must be strings")
    if "stdin" in data:
        _require(_is_rel_path(data["stdin"]) and os.path.isfile(os.path.join(case_dir, data["stdin"])),
                 "stdin must name an existing file in the case directory")
    req = data.get("requires", {})
    _require(isinstance(req, dict) and set(req) <= {"environment", "fixtures"}, "requires = {environment, fixtures}")
    for tag in req.get("environment", []):
        _require(tag in ENVIRONMENT_TAGS, "unknown environment requirement %r" % (tag,))
    for name in req.get("fixtures", []):
        _require(isinstance(name, str) and re.match(r"^[a-z0-9-]+$", name), "invalid fixture name %r" % (name,))
    _require(isinstance(data.get("harness", False), bool), "harness must be a bool")
    env = data.get("env", {})
    _require(isinstance(env, dict) and all(isinstance(k, str) and isinstance(v, str) for k, v in env.items()),
             "env must map strings to strings")

    validate_expect(data["expect"], data["mode"], case_dir)

    refs = data["spec_refs"]
    _require(isinstance(refs, list) and refs and all(isinstance(r, str) and r for r in refs),
             "spec_refs must be a non-empty list of strings")
    budget = data["budget"]
    _require(isinstance(budget, dict) and isinstance(budget.get("timeout_s"), (int, float))
             and 0 < budget["timeout_s"] <= 3600, "budget.timeout_s must be in (0, 3600]")
    prov = data["provenance"]
    _require(isinstance(prov, dict) and isinstance(prov.get("expected_basis"), str) and prov["expected_basis"].strip(),
             "provenance.expected_basis must explain how the expectation was justified")
    _require(isinstance(prov.get("origin"), str) and prov["origin"].strip(), "provenance.origin is required")
    if data["status"] == "unresolved":
        _require(isinstance(data.get("notes"), str) and "D-" in data["notes"],
                 "unresolved cases must reference a discrepancy-ledger entry (D-NNN) in notes")


def validate_expect(expect, mode, case_dir):
    _require(isinstance(expect, dict), "expect must be an object")
    outcome = expect.get("outcome")
    _require(outcome in OUTCOMES, "expect.outcome must be one of %s" % (OUTCOMES,))
    allowed = {"outcome", "exit_status", "stdout", "stderr", "rejection", "failure",
               "artifacts", "justification"}
    unknown = set(expect) - allowed
    _require(not unknown, "expect has unknown keys %s" % sorted(unknown))
    # Diagnostic prose and the embedded test runner's report format are not
    # normative, so those streams may be ignored; successful program output
    # is always compared exactly.
    lenient = outcome != "success" or mode == "test"
    if lenient and mode == "test" and expect.get("stdout") == "ignore":
        _require(isinstance(expect.get("justification"), str) and expect["justification"].strip(),
                 "ignoring test-mode stdout requires expect.justification")
    if outcome == "success":
        if mode in ("run", "test"):
            _require(isinstance(expect.get("exit_status"), int) and 0 <= expect["exit_status"] <= 255,
                     "success in run/test mode needs exit_status 0..255")
        else:
            _require("exit_status" not in expect, "parse/check acceptance has no exit status")
        _require("rejection" not in expect and "failure" not in expect,
                 "success cannot declare rejection/failure")
    elif outcome == "reject":
        rej = expect.get("rejection")
        _require(isinstance(rej, dict), "reject needs a rejection object")
        _check_stage_categories(rej, "rejection", ("parse", "check"))
        _require("exit_status" not in expect, "rejections have no program exit status")
    elif outcome == "failure":
        _require(mode in ("run", "test"), "runtime failure expectations need run/test mode")
        fail = expect.get("failure")
        _require(isinstance(fail, dict), "failure needs a failure object")
        _check_stage_categories(fail, "failure", ("runtime",))
        _require("exit_status" not in expect, "failure status is checked as nonzero, not declared")
    else:
        err = expect.get("failure") or expect.get("rejection")
        _require(isinstance(err, dict) and not (expect.get("failure") and expect.get("rejection")),
                 "error needs exactly one of failure/rejection describing allowed stages")
        _check_stage_categories(err, "error", STAGES)
        _require(isinstance(err.get("justification"), str) and err["justification"].strip(),
                 "an 'error' outcome must justify why more than one stage is permitted")
    for stream in ("stdout", "stderr"):
        if stream in expect:
            validate_output_spec(expect[stream], "expect." + stream, case_dir, lenient)
    if outcome == "success" and mode in ("run", "test"):
        _require("stdout" in expect, "successful run/test cases must state stdout exactly")
    for art in expect.get("artifacts", []):
        _require(isinstance(art, dict) and _is_rel_path(art.get("path", "")),
                 "artifact entries need a relative path")
        kinds = [k for k in ("text", "file", "bytes_b64", "absent") if k in art]
        _require(len(kinds) == 1, "artifact %s needs exactly one of text/file/bytes_b64/absent" % art.get("path"))
        if "file" in art:
            _require(os.path.isfile(os.path.join(case_dir, art["file"])),
                     "artifact expectation file %s missing" % art["file"])


def _check_stage_categories(obj, where, allowed_stages):
    stages = obj.get("stages")
    _require(isinstance(stages, list) and stages and all(s in allowed_stages for s in stages),
             "%s.stages must be a non-empty subset of %s" % (where, allowed_stages))
    cats = obj.get("categories")
    _require(isinstance(cats, list) and cats and all(c in CATEGORIES for c in cats),
             "%s.categories must be a non-empty subset of %s" % (where, CATEGORIES))
    if "error_type" in obj:
        _require(isinstance(obj["error_type"], str) and obj["error_type"], "%s.error_type must be a string" % where)
        _require(cats == ["uncaught-error"], "%s.error_type only applies to uncaught-error" % where)
    unknown = set(obj) - {"stages", "categories", "error_type", "justification"}
    _require(not unknown, "%s has unknown keys %s" % (where, sorted(unknown)))
    if len(stages) > 1 or len(cats) > 1:
        _require(isinstance(obj.get("justification"), str) and obj["justification"].strip(),
                 "%s permits several stages/categories; add a justification citing the contract" % where)


def load_json(path):
    with open(path, "rb") as fh:
        raw = fh.read()
    try:
        return json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, ValueError) as exc:
        raise CaseError("%s: invalid JSON: %s" % (path, exc))


def load_case(case_dir, level_dir_name=None):
    path = os.path.join(case_dir, "case.json")
    data = load_json(path)
    try:
        validate_case_data(data, case_dir, level_dir_name)
    except CaseError as exc:
        raise CaseError("%s: %s" % (path, exc))
    return Case(case_dir, data)
