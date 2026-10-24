"""Turn adapter observations into conformance statuses.

``judge`` is a pure function over (case, observation) so the runner self-tests
can exercise every branch with synthetic observations. Its rules are the
normative comparison rules of SPECIFICATION.md section 7.
"""

import base64
import os

from .model import CATEGORIES, STAGES

STATUSES = ("PASS", "FAIL", "UNSUPPORTED", "NOT_RUN", "BLOCKED", "TIMEOUT", "CRASH", "HARNESS_ERROR")

PROTOCOL = "ocs-adapter/1"
ADAPTER_OUTCOMES = ("completed", "accepted", "rejected", "failed", "internal-error", "unsupported")


class Observation(object):
    """Everything the runner saw for one execution attempt."""

    def __init__(self, response=None, response_error=None, stdout=b"", stderr=b"",
                 process_exit=None, process_signal=None, timed_out=False,
                 artifacts=None, elapsed_s=0.0):
        self.response = response
        self.response_error = response_error
        self.stdout = stdout
        self.stderr = stderr
        self.process_exit = process_exit
        self.process_signal = process_signal
        self.timed_out = timed_out
        self.artifacts = artifacts or {}
        self.elapsed_s = elapsed_s


def validate_response(resp, requested_mode):
    """Return an error string when the adapter response violates the protocol."""
    if not isinstance(resp, dict):
        return "response is not a JSON object"
    if resp.get("protocol") != PROTOCOL:
        return "response protocol is %r, expected %r" % (resp.get("protocol"), PROTOCOL)
    if resp.get("mode") != requested_mode:
        return "response mode %r does not echo requested mode %r" % (resp.get("mode"), requested_mode)
    outcome = resp.get("outcome")
    if outcome not in ADAPTER_OUTCOMES:
        return "unknown adapter outcome %r" % (outcome,)
    if "verdict" in resp or "pass" in resp or "status" in resp:
        return "adapters report observations, never verdicts (found verdict-like key)"
    if outcome == "unsupported":
        if not isinstance(resp.get("unsupported_reason"), str) or not resp["unsupported_reason"]:
            return "unsupported responses must explain why"
        return None
    if outcome == "completed":
        if requested_mode not in ("run", "test"):
            return "'completed' is only valid for run/test; use 'accepted'"
        es = resp.get("exit_status")
        if not isinstance(es, int) or isinstance(es, bool) or not 0 <= es <= 255:
            return "completed responses need exit_status 0..255"
    if outcome == "accepted" and requested_mode not in ("parse", "check"):
        return "'accepted' is only valid for parse/check"
    if outcome in ("rejected", "failed"):
        failure = resp.get("failure")
        if not isinstance(failure, dict):
            return "rejected/failed responses need a failure object"
        if failure.get("stage") not in STAGES:
            return "failure.stage must be one of %s" % (STAGES,)
        if failure.get("category") not in CATEGORIES + ("unclassified",):
            return "failure.category %r is not a logical category" % (failure.get("category"),)
        if outcome == "rejected" and failure["stage"] == "runtime":
            return "a rejection cannot happen at runtime; report 'failed'"
        if outcome == "failed" and failure["stage"] != "runtime":
            return "'failed' means a runtime failure; report 'rejected' for earlier stages"
    if outcome == "internal-error":
        if not isinstance(resp.get("failure"), dict):
            return "internal-error responses need a failure object with raw detail"
    return None


def _expected_bytes(spec, case_dir):
    if "text" in spec:
        return spec["text"].encode("utf-8")
    if "file" in spec:
        with open(os.path.join(case_dir, spec["file"]), "rb") as fh:
            return fh.read()
    return base64.b64decode(spec["bytes_b64"])


def _describe_diff(name, actual, expected):
    limit = 400
    try:
        a = actual.decode("utf-8")
        e = expected.decode("utf-8")
    except UnicodeDecodeError:
        return "%s bytes differ (actual %d bytes, expected %d bytes)" % (name, len(actual), len(expected))
    i = 0
    while i < min(len(a), len(e)) and a[i] == e[i]:
        i += 1
    return "%s differs at character %d: expected %r, got %r" % (
        name, i, e[max(0, i - 20):i + 60][:limit], a[max(0, i - 20):i + 60][:limit])


def compare_stream(name, actual, spec, case_dir):
    """Return None when the stream matches its expectation, else a reason."""
    if spec == "ignore":
        return None
    if "partial_order" in spec:
        po = spec["partial_order"]
        try:
            text = actual.decode("utf-8")
        except UnicodeDecodeError:
            return "%s is not UTF-8 text" % name
        if text and not text.endswith("\n"):
            return "%s does not end with a newline" % name
        lines = text.split("\n")[:-1] if text else []
        if sorted(lines) != sorted(po["lines"]):
            return "%s lines %r are not a permutation of %r" % (name, lines, po["lines"])
        for a, b in po.get("before", []):
            if lines.index(a) > lines.index(b):
                return "%s violates required order %r before %r" % (name, a, b)
        return None
    expected = _expected_bytes(spec, case_dir)
    if actual != expected:
        return _describe_diff(name, actual, expected)
    return None


def judge(case, obs):
    """Return (status, [reasons])."""
    reasons = []
    expect = case["expect"]
    mode = case["mode"]

    if obs.timed_out:
        return "TIMEOUT", ["no observation within the time budget"]
    resp = obs.response
    if resp is None:
        if obs.process_signal is not None:
            return "CRASH", ["adapter process terminated by signal %s without a response" % obs.process_signal]
        return "HARNESS_ERROR", [obs.response_error or "adapter produced no response"]
    problem = validate_response(resp, mode)
    if problem:
        return "HARNESS_ERROR", ["malformed adapter response: " + problem]
    outcome = resp["outcome"]
    if outcome == "unsupported":
        return "UNSUPPORTED", [resp["unsupported_reason"]]
    if outcome == "internal-error":
        detail = resp.get("failure", {}).get("message", "")
        return "CRASH", ["implementation internal error: " + detail[:500]]
    if obs.process_signal is not None:
        return "CRASH", ["adapter process terminated by signal %s" % obs.process_signal]

    # The adapter process exit code must agree with what it reports.
    if outcome == "completed" and obs.process_exit != resp["exit_status"]:
        return "HARNESS_ERROR", ["adapter reported exit_status %s but its process exited %s" % (
            resp["exit_status"], obs.process_exit)]
    if outcome == "failed" and obs.process_exit == 0:
        return "HARNESS_ERROR", ["adapter reported a runtime failure but its process exited 0"]
    if outcome == "accepted" and obs.process_exit != 0:
        return "HARNESS_ERROR", ["adapter reported acceptance but its process exited %s" % obs.process_exit]

    want = expect["outcome"]
    failure = resp.get("failure") or {}
    if want == "success":
        good = "completed" if mode in ("run", "test") else "accepted"
        if outcome != good:
            reasons.append("expected %s, observed %s%s" % (good, outcome, _failure_text(failure)))
        elif mode in ("run", "test") and resp["exit_status"] != expect["exit_status"]:
            reasons.append("exit status %s, expected %s" % (resp["exit_status"], expect["exit_status"]))
    else:
        spec = expect.get("rejection") if want == "reject" else expect.get("failure") if want == "failure" \
            else (expect.get("failure") or expect.get("rejection"))
        allowed_outcomes = {"reject": ("rejected",), "failure": ("failed",),
                            "error": ("rejected", "failed")}[want]
        if outcome not in allowed_outcomes:
            reasons.append("expected %s, observed %s" % ("/".join(allowed_outcomes), outcome)
                           + (" with exit status %s" % resp.get("exit_status") if outcome == "completed" else ""))
        else:
            if failure.get("stage") not in spec["stages"]:
                reasons.append("failure at stage %r, contract allows %s" % (failure.get("stage"), spec["stages"]))
            if failure.get("category") not in spec["categories"]:
                reasons.append("failure category %r (type %r), contract requires %s" % (
                    failure.get("category"), failure.get("error_type"), spec["categories"]))
            if "error_type" in spec and failure.get("error_type") != spec["error_type"]:
                reasons.append("uncaught error type %r, expected %r" % (failure.get("error_type"), spec["error_type"]))

    default_stdout = {"text": ""}
    if want != "success" or mode in ("run", "test"):
        stdout_spec = expect.get("stdout", default_stdout)
    else:
        stdout_spec = expect.get("stdout", default_stdout)
    r = compare_stream("stdout", obs.stdout, stdout_spec, case.dir)
    if r:
        reasons.append(r)
    stderr_spec = expect.get("stderr", {"text": ""} if want == "success" else "ignore")
    r = compare_stream("stderr", obs.stderr, stderr_spec, case.dir)
    if r:
        reasons.append(r)

    for art in expect.get("artifacts", []):
        actual = obs.artifacts.get(art["path"])
        if "absent" in art:
            if actual is not None:
                reasons.append("artifact %s must not exist" % art["path"])
            continue
        if actual is None:
            reasons.append("declared artifact %s was not produced" % art["path"])
            continue
        r = compare_stream("artifact " + art["path"], actual, art, case.dir)
        if r:
            reasons.append(r)

    return ("FAIL", reasons) if reasons else ("PASS", [])


def _failure_text(failure):
    if not failure:
        return ""
    return " (%s/%s %s: %s)" % (failure.get("stage"), failure.get("category"),
                                 failure.get("error_type") or "", (failure.get("message") or "")[:300])
