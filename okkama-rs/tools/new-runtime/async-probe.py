#!/usr/bin/env python3
"""Focused runtime probes (L08 by default) using the existing OCS judge, not the full OCS gate.

Only successful run cases with no fixtures, arguments, stdin or external-resource
requirements outside the explicitly allowed set are selected. Filesystem probes
use independent directories per backend and validate declared artifacts. Compiler and runtime paths are explicit and separate.
Unsupported engine capabilities remain visible; parse/check/test cases are not
credited to the new engine. Each subprocess has the case timeout.
"""
import argparse
import collections
import hashlib
import json
import os
import shutil
from pathlib import Path
from probe_process import run_bounded, ResourceLimitExceeded, positive_int, map_bounded
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "conformance/runner"))
from ocs.judge import Observation, judge
from ocs.model import Case


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--level", action="append", choices=["L%02d" % i for i in range(14)])
    parser.add_argument("--allow-environment", action="append", default=[], choices=["filesystem", "env-vars", "args", "process-spawn", "stdin", "real-random", "loopback-network"])
    parser.add_argument("--jobs", type=positive_int, default=1)
    args = parser.parse_args()
    compiler, runtime = args.compiler.resolve(), args.runtime.resolve()
    selected, omitted = [], []
    allowed = {"stdout", "stderr", "real-clock", "worker-process"} | set(args.allow_environment)
    levels = args.level or ["L08"]
    paths = [path for level in levels for path in (ROOT / "conformance/cases").glob(level + "-*/*/case.json")]
    for path in sorted(paths):
        case = Case(str(path.parent), json.loads(path.read_text()))
        supported = (case.mode == "run" and case["expect"]["outcome"] == "success"
                     and set(case.environment) <= allowed
                     and not case.fixtures and (not case.get("args") or "args" in allowed) and (not case.get("stdin") or "stdin" in allowed)
                     and (not case.get("env") or "env-vars" in allowed))
        (selected if supported else omitted).append(case)

    with tempfile.TemporaryDirectory(prefix="sparrow-async-probe-") as work:
        snapshot = Path(work) / "sparrow-next"
        shutil.copy2(runtime, snapshot)
        runtime = snapshot
        runtime_digest = hashlib.sha256(runtime.read_bytes()).hexdigest()
        def run(case):
            case_work = Path(work) / case.id
            shutil.copytree(case.dir, case_work)
            if case.get("harness"):
                shutil.copytree(ROOT / "conformance/harness/okkama/conformance", case_work / "conformance")
            binary = case_work / "case.spb"
            source = case_work / case["entry"]
            timeout = case["budget"].get("timeout_s", 30)
            record = {"id": case.id, "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(), "runs": []}
            try:
                built = run_bounded([str(compiler), "build", str(source), "--target", "sparrow", "-o", str(binary)], cwd=case_work, capture_output=True, timeout=timeout, env=dict(os.environ, OKKAMA_STDLIB_DIR=str(ROOT / "std")))
            except (subprocess.TimeoutExpired, ResourceLimitExceeded) as error:
                record.update(status=getattr(error, "reason", "TIMEOUT"), stage="compiler")
                return record
            if built.returncode:
                record.update(status="FAIL", stage="compiler", stderr=built.stderr.decode("utf-8", "replace"))
                return record
            for mode in ["cold", "jit"]:
                # Each backend starts from the same inputs, never the other
                # backend's artifacts or filesystem mutations.
                mode_work = Path(work) / (case.id + "-" + mode)
                shutil.copytree(case.dir, mode_work)
                if case.get("harness"):
                    shutil.copytree(ROOT / "conformance/harness/okkama/conformance", mode_work / "conformance")
                temporary = mode_work / ".tmp"
                temporary.mkdir()
                command = [str(runtime)] + (["--cold"] if mode == "cold" else []) + [str(binary)] + case.get("args", [])
                try:
                    ran = run_bounded(command, input=(mode_work / case["stdin"]).read_bytes() if case.get("stdin") else b"", cwd=mode_work, capture_output=True, timeout=timeout, env=dict(os.environ, OKKAMA_TRACE="off", TMPDIR=str(temporary), TZ="UTC", **case.get("env", {})))
                    response = {"protocol": "ocs-adapter/1", "mode": "run", "outcome": "completed", "exit_status": ran.returncode}
                    error = ran.stderr.decode("utf-8", "replace")
                    if "unsupported " in error and "new engine" in error:
                        response = {"protocol": "ocs-adapter/1", "mode": "run", "outcome": "unsupported", "unsupported_reason": error.strip()}
                    if ran.returncode < 0:
                        response = None
                    artifacts = {}
                    for artifact in case["expect"].get("artifacts", []):
                        path = mode_work / artifact["path"]
                        if path.is_file():
                            artifacts[artifact["path"]] = path.read_bytes()
                    observation = Observation(artifacts=artifacts, response=response, stdout=ran.stdout, stderr=ran.stderr, process_exit=ran.returncode, process_signal=(-ran.returncode if ran.returncode < 0 else None))
                    status, reasons = judge(case, observation)
                    record["runs"].append({"mode": mode, "status": status, "reasons": reasons, "exit_status": ran.returncode, "stdout": ran.stdout.decode("utf-8", "replace"), "stderr": error})
                except (subprocess.TimeoutExpired, ResourceLimitExceeded) as error:
                    record["runs"].append({"mode": mode, "status": getattr(error, "reason", "TIMEOUT")})
            return record

        records = []
        for record in map_bounded(run, selected, args.jobs):
            records.append(record)
            print(record["id"], record.get("status") or "/".join(r["status"] for r in record["runs"]), flush=True)
    counts = collections.Counter(r["status"] for record in records for r in record["runs"])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({"scope": "focused successful run cases; not full conformance", "levels": levels, "allowed_environment": sorted(allowed), "compiler": str(compiler), "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(), "runtime_sha256": runtime_digest, "counts_by_mode_run": dict(counts), "omitted": [c.id for c in omitted], "cases": sorted(records, key=lambda r: r["id"])}, indent=2) + "\n")
    print(dict(counts))
    if any(record.get("status") or any(run["status"] != "PASS" for run in record["runs"])
           for record in records):
        raise SystemExit(1)

if __name__ == "__main__":
    main()
