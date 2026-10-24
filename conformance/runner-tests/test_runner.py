#!/usr/bin/env python3
"""Runner self-tests: deliberate failures must never become PASS.

Run: python3 runner-tests/test_runner.py   (from the conformance directory)

Each test builds a throwaway mini-suite (the real runner code plus synthetic
cases, contracts and profiles) and drives the real `ocs` command line against
runner-tests/fake_adapter.py, whose behaviour models one class of wrong
implementation or broken adapter.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SUITE = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(SUITE, "runner"))

from ocs import model  # noqa: E402
from ocs.judge import Observation, judge, validate_response  # noqa: E402

LEVEL_DIR = "L00-bootstrap"
SRC_OK = "module app v1.0.0;\n\nmain() is int {\n    0;\n}\n"


def base_case(cid, **over):
    d = {
        "schema": "ocs-case/1", "id": "L00." + cid, "title": "t " + cid, "purpose": "self-test",
        "level": "L00", "contracts": [{"contract": "T.ONE", "aspects": ["normal"]}],
        "mode": "run", "sources": ["main.ok"], "entry": "main.ok",
        "expect": {"outcome": "success", "exit_status": 0, "stdout": {"text": "hello\n"}},
        "spec_refs": ["self-test"], "budget": {"timeout_s": 20},
        "provenance": {"origin": "runner self-test", "expected_basis": "synthetic"},
        "status": "mandatory",
    }
    d.update(over)
    return d


class MiniSuite(object):
    def __init__(self):
        self.root = os.path.realpath(tempfile.mkdtemp(prefix="ocs-selftest-"))
        shutil.copytree(os.path.join(SUITE, "runner"), os.path.join(self.root, "runner"),
                        ignore=shutil.ignore_patterns("__pycache__"))
        shutil.copy(os.path.join(SUITE, "ocs"), os.path.join(self.root, "ocs"))
        rel = json.load(open(os.path.join(SUITE, "release.json")))
        rel["stdlib"]["default_root"] = "stdlib"
        json.dump(rel, open(os.path.join(self.root, "release.json"), "w"))
        os.makedirs(os.path.join(self.root, "stdlib"))
        for lv in model.LEVELS.values():
            os.makedirs(os.path.join(self.root, "cases", lv))
        os.makedirs(os.path.join(self.root, "harness", "okkama", "conformance"))
        os.makedirs(os.path.join(self.root, "fixtures"))
        os.makedirs(os.path.join(self.root, "defects"))
        os.makedirs(os.path.join(self.root, "contracts"))
        os.makedirs(os.path.join(self.root, "profiles"))
        os.makedirs(os.path.join(self.root, "adapters", "fake"))
        shutil.copy(os.path.join(HERE, "fake_adapter.py"), os.path.join(self.root, "adapters", "fake", "fake_adapter.py"))
        json.dump({"schema": "ocs-adapter-config/1", "protocol": "ocs-adapter/1", "name": "fake",
                   "command": [sys.executable, "{adapter_dir}/fake_adapter.py"], "pass_env": ["PATH"]},
                  open(os.path.join(self.root, "adapters", "fake", "adapter.json"), "w"))
        json.dump({"schema": "ocs-ledger/1", "description": "self-test", "implementation_observed": "fake",
                   "entries": [{"id": "D-999", "subject": "synthetic", "sources": [], "observed": "", "intended_contract": "",
                                "authority": "", "conformance_expectation": "", "resolution": "unresolved", "cases": [], "contracts": []}]},
                  open(os.path.join(self.root, "contracts", "discrepancy-ledger.json"), "w"))
        self.contracts([{"id": "T.ONE", "title": "one", "status": "mandatory", "spec_refs": ["x"],
                         "aspects": {"normal": "required", "boundary": "n/a: synthetic contract",
                                     "invalid": "n/a: synthetic contract", "interaction": "n/a: synthetic contract"}}])
        self.profile("everything", "release-member", {"levels": list(model.LEVELS), "environment_within":
                                                      ["stdout", "stderr", "sqlite"]})
        self.profile("full-release", "release-gate", {"levels": list(model.LEVELS)}, members=["everything"])

    def contracts(self, items):
        json.dump({"schema": "ocs-contracts/1", "family": "T", "contracts": items},
                  open(os.path.join(self.root, "contracts", "t.json"), "w"))

    def profile(self, name, kind, select, members=None):
        doc = {"schema": "ocs-profile/1", "name": name, "kind": kind, "description": "self-test", "select": select}
        if members:
            doc["members"] = members
        json.dump(doc, open(os.path.join(self.root, "profiles", name + ".json"), "w"))

    def add(self, slug, data, files=None):
        d = os.path.join(self.root, "cases", LEVEL_DIR, slug)
        os.makedirs(d, exist_ok=True)
        for rel, text in (files or {"main.ok": SRC_OK}).items():
            p = os.path.join(d, rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            open(p, "w").write(text)
        json.dump(data, open(os.path.join(d, "case.json"), "w"))
        return d

    def ocs(self, *args, behavior=None, extra_env=None):
        cmd = [sys.executable, os.path.join(self.root, "ocs")] + list(args)
        if behavior is not None:
            cmd += ["--adapter-env", "OCS_FAKE_BEHAVIOR=" + behavior]
        for k, v in (extra_env or {}).items():
            cmd += ["--adapter-env", "%s=%s" % (k, v)]
        return subprocess.run(cmd, cwd=self.root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                              universal_newlines=True, timeout=300)

    def run_case(self, cid, behavior, extra_env=None, extra=()):
        out = os.path.join(self.root, "reports", "out-%s-%s" % (cid, behavior))
        p = self.ocs("run", "--adapter", "adapters/fake/adapter.json", "--profile", "full-release",
                     "--case", "L00." + cid, "--out", out, "--quiet", "--keep", "none", *extra,
                     behavior=behavior, extra_env=extra_env)
        recs = {}
        path = os.path.join(out, "results.jsonl")
        if not os.path.exists(path):
            raise AssertionError("no results: " + p.stdout)
        with open(path) as fh:
            for line in fh:
                r = json.loads(line)
                recs[r["id"]] = r
        return recs["L00." + cid], recs, p

    def close(self):
        shutil.rmtree(self.root, ignore_errors=True)


class RunnerBehaviour(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        s = cls.suite = MiniSuite()
        s.add("success", base_case("success"))
        s.add("reject", base_case("reject", expect={"outcome": "reject", "rejection": {"stages": ["parse"], "categories": ["syntax"]}}))
        s.add("runtime", base_case("runtime", expect={"outcome": "failure", "failure": {"stages": ["runtime"], "categories": ["runtime"]},
                                                      "stdout": {"text": ""}}))
        s.add("uncaught", base_case("uncaught", expect={"outcome": "failure", "failure": {"stages": ["runtime"], "categories": ["uncaught-error"],
                                                                                          "error_type": "Broken"}}))
        s.add("async", base_case("async", expect={"outcome": "success", "exit_status": 0, "stdout": {"text": "started\ndone\n"}}))
        s.add("artifact", base_case("artifact", expect={"outcome": "success", "exit_status": 0, "stdout": {"text": ""},
                                                        "artifacts": [{"path": "out.txt", "text": "payload\n"}]}))
        s.add("timeout", base_case("timeout", budget={"timeout_s": 2}))
        s.add("fixture", base_case("fixture", requires={"fixtures": ["missing-fixture"]}))
        s.add("needs-sqlite", base_case("needs-sqlite", requires={"environment": ["sqlite"]}))
        s.add("partial", base_case("partial", expect={"outcome": "success", "exit_status": 0, "stdout": {"partial_order": {
            "lines": ["a", "b", "c"], "before": [["a", "c"]]}, "justification": "concurrent writers"}}))
        s.add("check", base_case("check", mode="check", expect={"outcome": "success"}))

    @classmethod
    def tearDownClass(cls):
        cls.suite.close()

    def status(self, cid, behavior, **kw):
        rec, _, p = self.suite.run_case(cid, behavior, **kw)
        return rec["status"], rec, p

    def assertStatus(self, cid, behavior, expected, **kw):
        st, rec, p = self.status(cid, behavior, **kw)
        self.assertEqual(st, expected, "%s/%s -> %s %s\n%s" % (cid, behavior, st, rec.get("reasons"), p.stdout[-2000:]))
        return rec

    # Positive controls: without them a runner that fails everything would pass these tests.
    def test_control_success(self):
        self.assertStatus("success", "success", "PASS")

    def test_control_reject(self):
        self.assertStatus("reject", "reject-syntax-parse", "PASS")

    def test_control_uncaught(self):
        self.assertStatus("uncaught", "uncaught-right-type", "PASS")

    def test_control_runtime(self):
        self.assertStatus("runtime", "runtime-error", "PASS")

    def test_control_artifact(self):
        self.assertStatus("artifact", "artifact-ok", "PASS")

    def test_control_check_mode(self):
        self.assertStatus("check", "accept", "PASS")

    # Wrong program observations.
    def test_wrong_output(self):
        self.assertStatus("success", "wrong-output", "FAIL")

    def test_no_trimming(self):
        self.assertStatus("success", "extra-trailing-space", "FAIL")

    def test_no_line_ending_normalisation(self):
        self.assertStatus("success", "crlf-output", "FAIL")

    def test_unexpected_stderr(self):
        self.assertStatus("success", "unexpected-stderr", "FAIL")

    def test_wrong_exit_status(self):
        self.assertStatus("success", "wrong-exit", "FAIL")

    def test_invalid_program_executes(self):
        self.assertStatus("reject", "success", "FAIL")

    def test_wrong_error_category(self):
        self.assertStatus("reject", "reject-semantic-check", "FAIL")

    def test_wrong_error_stage(self):
        rec = self.assertStatus("reject", "reject-syntax-check", "FAIL")
        self.assertTrue(any("stage" in r for r in rec["reasons"]))

    def test_runtime_error_is_not_uncaught_error(self):
        self.assertStatus("uncaught", "runtime-error", "FAIL")

    def test_wrong_uncaught_type(self):
        self.assertStatus("uncaught", "uncaught-other-type", "FAIL")

    def test_missing_async_completion(self):
        self.assertStatus("async", "missing-async-completion", "FAIL")

    def test_artifact_corruption(self):
        self.assertStatus("artifact", "artifact-corrupt", "FAIL")

    def test_artifact_missing(self):
        self.assertStatus("artifact", "artifact-missing", "FAIL")

    def test_partial_order_accepts_permitted_schedule(self):
        self.assertStatus("partial", "partial-order", "PASS", extra_env={"OCS_FAKE_STDOUT": "b\\na\\nc\\n"})

    def test_partial_order_rejects_violation(self):
        self.assertStatus("partial", "partial-order", "FAIL", extra_env={"OCS_FAKE_STDOUT": "c\\nb\\na\\n"})

    def test_partial_order_rejects_missing_line(self):
        self.assertStatus("partial", "partial-order", "FAIL", extra_env={"OCS_FAKE_STDOUT": "a\\nc\\n"})

    # Execution problems never satisfy an expected language error.
    def test_timeout(self):
        rec = self.assertStatus("timeout", "timeout", "TIMEOUT")
        self.assertLess(rec["elapsed_s"], 30)

    def test_timeout_is_not_runtime_error(self):
        st, _, _ = self.status("runtime", "timeout", extra=("--timeout-scale", "0.1"))
        self.assertEqual(st, "TIMEOUT")

    def test_crash(self):
        self.assertStatus("success", "crash", "CRASH")

    def test_crash_is_not_expected_error(self):
        self.assertStatus("runtime", "crash", "CRASH")

    def test_internal_error_is_crash_not_runtime_error(self):
        self.assertStatus("runtime", "internal-error", "CRASH")

    # Adapter protocol violations.
    def test_malformed_response(self):
        self.assertStatus("success", "malformed", "HARNESS_ERROR")

    def test_missing_response(self):
        self.assertStatus("success", "no-response", "HARNESS_ERROR")

    def test_verdict_from_adapter_rejected(self):
        self.assertStatus("success", "verdict", "HARNESS_ERROR")

    def test_wrong_protocol(self):
        self.assertStatus("success", "wrong-protocol", "HARNESS_ERROR")

    def test_lying_exit_status(self):
        self.assertStatus("success", "lying-exit", "HARNESS_ERROR")

    def test_failure_with_zero_exit(self):
        self.assertStatus("runtime", "failed-exit-zero", "HARNESS_ERROR")

    def test_acceptance_in_run_mode(self):
        self.assertStatus("success", "accept-in-run-mode", "HARNESS_ERROR")

    def test_unsupported_mode(self):
        self.assertStatus("success", "unsupported", "UNSUPPORTED")

    # Environment and isolation.
    def test_missing_fixture_blocks(self):
        self.assertStatus("fixture", "success", "BLOCKED")

    def test_unavailable_environment_blocks(self):
        self.assertStatus("needs-sqlite", "success", "BLOCKED", extra=("--unavailable", "sqlite"))

    def test_source_isolation(self):
        before = open(os.path.join(self.suite.root, "cases", LEVEL_DIR, "success", "main.ok")).read()
        self.assertStatus("success", "modify-source", "PASS")
        after = open(os.path.join(self.suite.root, "cases", LEVEL_DIR, "success", "main.ok")).read()
        self.assertEqual(before, after)

    def test_omitted_execution_is_not_run(self):
        _, recs, p = self.suite.run_case("success", "success")
        self.assertEqual(recs["L00.reject"]["status"], "NOT_RUN")
        self.assertNotEqual(p.returncode, 0, "a run with NOT_RUN mandatory cases must not exit 0")


class GateBehaviour(unittest.TestCase):
    def test_gate_requires_everything(self):
        s = MiniSuite()
        try:
            s.add("one", base_case("one"))
            s.add("two", base_case("two"))
            out = os.path.join(s.root, "reports", "partial")
            s.ocs("run", "--adapter", "adapters/fake/adapter.json", "--profile", "full-release", "--case", "L00.one",
                  "--out", out, "--quiet", behavior="success")
            g = s.ocs("gate", "--results", out)
            self.assertNotEqual(g.returncode, 0)
            self.assertIn("NOT PASSED", g.stdout)
            self.assertIn("1/2 PASS", g.stdout)
            # Positive control: complete run with complete coverage passes the gate.
            full = os.path.join(s.root, "reports", "full")
            s.ocs("run", "--adapter", "adapters/fake/adapter.json", "--profile", "full-release", "--out", full, "--quiet",
                  behavior="success")
            g = s.ocs("gate", "--results", full)
            self.assertEqual(g.returncode, 0, g.stdout)
            self.assertIn("PASSED", g.stdout)
            # A coverage gap blocks the gate even when every case passes.
            s.contracts([
                {"id": "T.ONE", "title": "one", "status": "mandatory", "spec_refs": ["x"],
                 "aspects": {"normal": "required", "boundary": "n/a: synthetic contract",
                             "invalid": "n/a: synthetic contract", "interaction": "n/a: synthetic contract"}},
                {"id": "T.TWO", "title": "two", "status": "mandatory", "spec_refs": ["x"],
                 "aspects": {"normal": "required", "boundary": "n/a: synthetic contract",
                             "invalid": "required", "interaction": "n/a: synthetic contract"}}])
            full2 = os.path.join(s.root, "reports", "full2")
            s.ocs("run", "--adapter", "adapters/fake/adapter.json", "--profile", "full-release", "--out", full2, "--quiet",
                  behavior="success")
            g = s.ocs("gate", "--results", full2)
            self.assertNotEqual(g.returncode, 0)
            self.assertIn("coverage obligations incomplete", g.stdout)
            # Results from a different suite tree (changed expectations) are refused.
            g = s.ocs("gate", "--results", full)
            self.assertIn("suite trees", g.stdout)
            # Unresolved contracts block the gate.
            s.contracts([
                {"id": "T.ONE", "title": "one", "status": "unresolved", "discrepancies": ["D-999"], "spec_refs": ["x"],
                 "aspects": {"normal": "required", "boundary": "n/a: synthetic contract",
                             "invalid": "n/a: synthetic contract", "interaction": "n/a: synthetic contract"}}])
            full3 = os.path.join(s.root, "reports", "full3")
            s.ocs("run", "--adapter", "adapters/fake/adapter.json", "--profile", "full-release", "--out", full3, "--quiet",
                  behavior="success")
            g = s.ocs("gate", "--results", full3)
            self.assertIn("unresolved normative contracts", g.stdout)
            # A milestone profile is never reported as a release claim.
            s.profile("frontend", "milestone", {"levels": list(model.LEVELS), "modes": ["parse", "check"]})
            g = s.ocs("gate", "--profile", "frontend", "--results", full3)
            self.assertIn("not a release claim", g.stdout)
        finally:
            s.close()


class DiscoveryBehaviour(unittest.TestCase):
    def validate(self, setup):
        s = MiniSuite()
        try:
            setup(s)
            p = s.ocs("validate")
            return p.returncode, p.stdout
        finally:
            s.close()

    def test_clean_suite_validates(self):
        rc, out = self.validate(lambda s: s.add("ok", base_case("ok")))
        self.assertEqual(rc, 0, out)

    def test_duplicate_ids(self):
        def setup(s):
            s.add("a", base_case("same"))
            s.add("b", base_case("same"))
        rc, out = self.validate(setup)
        self.assertNotEqual(rc, 0)
        self.assertIn("duplicate case id", out)

    def test_undiscovered_source(self):
        def setup(s):
            s.add("ok", base_case("ok"))
            d = os.path.join(s.root, "cases", LEVEL_DIR, "forgotten")
            os.makedirs(d)
            open(os.path.join(d, "main.ok"), "w").write(SRC_OK)
        rc, out = self.validate(setup)
        self.assertNotEqual(rc, 0)
        self.assertIn("undiscovered", out)

    def test_unlisted_support_module(self):
        def setup(s):
            s.add("ok", base_case("ok"), files={"main.ok": SRC_OK, "helper.ok": SRC_OK})
        rc, out = self.validate(setup)
        self.assertNotEqual(rc, 0)
        self.assertIn("neither a declared source nor support", out)

    def test_invalid_expected_data(self):
        def setup(s):
            s.add("ok", base_case("ok", expect={"outcome": "success", "exit_status": 300, "stdout": {"text": ""}}))
        rc, out = self.validate(setup)
        self.assertNotEqual(rc, 0)
        self.assertIn("exit_status", out)

    def test_orphan_contract_mapping(self):
        def setup(s):
            s.add("ok", base_case("ok", contracts=[{"contract": "T.MISSING", "aspects": ["normal"]}]))
        rc, out = self.validate(setup)
        self.assertNotEqual(rc, 0)
        self.assertIn("orphan contract mapping", out)

    def test_mandatory_case_outside_profiles(self):
        def setup(s):
            s.add("ok", base_case("ok", requires={"environment": ["tls"]}))
        rc, out = self.validate(setup)
        self.assertNotEqual(rc, 0)
        self.assertIn("selected by no release-member profile", out)

    def test_unknown_ledger_reference(self):
        def setup(s):
            s.add("ok", base_case("ok"))
            s.contracts([{"id": "T.ONE", "title": "one", "status": "unresolved", "discrepancies": ["D-404"], "spec_refs": ["x"],
                          "aspects": {"normal": "required", "boundary": "n/a: synthetic contract",
                                      "invalid": "n/a: synthetic contract", "interaction": "n/a: synthetic contract"}}])
        rc, out = self.validate(setup)
        self.assertNotEqual(rc, 0)
        self.assertIn("unknown ledger entry D-404", out)

    def test_level_mismatch(self):
        def setup(s):
            c = base_case("ok")
            c["id"] = "L01.ok"
            c["level"] = "L01"
            s.add("ok", c)
        rc, out = self.validate(setup)
        self.assertNotEqual(rc, 0)
        self.assertIn("does not match directory", out)


class ModelValidation(unittest.TestCase):
    def bad(self, **over):
        d = tempfile.mkdtemp()
        try:
            open(os.path.join(d, "main.ok"), "w").write(SRC_OK)
            data = base_case("x", **over)
            with self.assertRaises(model.CaseError):
                model.validate_case_data(data, d)
        finally:
            shutil.rmtree(d)

    def test_unknown_outcome(self):
        self.bad(expect={"outcome": "maybe"})

    def test_reject_at_runtime(self):
        self.bad(expect={"outcome": "reject", "rejection": {"stages": ["runtime"], "categories": ["runtime"]}})

    def test_multi_stage_needs_justification(self):
        self.bad(expect={"outcome": "reject", "rejection": {"stages": ["parse", "check"], "categories": ["syntax"]}})

    def test_success_needs_stdout(self):
        self.bad(expect={"outcome": "success", "exit_status": 0})

    def test_ignore_stdout_on_success(self):
        self.bad(expect={"outcome": "success", "exit_status": 0, "stdout": "ignore"})

    def test_missing_basis(self):
        self.bad(provenance={"origin": "x"})

    def test_bad_id(self):
        self.bad(id="L00.Bad Id")

    def test_unknown_category(self):
        self.bad(expect={"outcome": "failure", "failure": {"stages": ["runtime"], "categories": ["crash"]}})

    def test_error_type_requires_uncaught(self):
        self.bad(expect={"outcome": "failure", "failure": {"stages": ["runtime"], "categories": ["runtime"], "error_type": "X"}})

    def test_unresolved_needs_ledger(self):
        self.bad(status="unresolved")

    def test_response_validation(self):
        self.assertIsNotNone(validate_response({"protocol": "ocs-adapter/1", "mode": "run", "outcome": "completed"}, "run"))
        self.assertIsNotNone(validate_response({"protocol": "ocs-adapter/1", "mode": "run", "outcome": "rejected",
                                                "failure": {"stage": "runtime", "category": "syntax"}}, "run"))
        self.assertIsNone(validate_response({"protocol": "ocs-adapter/1", "mode": "run", "outcome": "completed",
                                             "exit_status": 0}, "run"))

    def test_judge_direct_signal_without_response(self):
        d = tempfile.mkdtemp()
        try:
            case = model.Case(d, base_case("x"))
            st, _ = judge(case, Observation(response=None, process_signal=9))
            self.assertEqual(st, "CRASH")
            st, _ = judge(case, Observation(response=None, response_error="nothing"))
            self.assertEqual(st, "HARNESS_ERROR")
        finally:
            shutil.rmtree(d)


if __name__ == "__main__":
    unittest.main(verbosity=2)
