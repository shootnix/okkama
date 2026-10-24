"""Isolated execution of one case through an external adapter process."""

import base64
import hashlib
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time

from .judge import Observation, judge

ADAPTER_CONFIG_SCHEMA = "ocs-adapter-config/1"
FIXTURE_SCHEMA = "ocs-fixture/1"
OUTPUT_LIMIT = 16 * 1024 * 1024

# The environment every case starts from. Host variables are NOT inherited
# except those an adapter configuration explicitly lists in pass_env (needed
# to launch the implementation itself, e.g. PATH or PERL5LIB).
BASE_ENV = {
    "LC_ALL": "C",
    "LANG": "C",
    "TZ": "UTC",
}


class AdapterConfigError(Exception):
    pass


class Adapter(object):
    def __init__(self, path, overrides=None):
        self.path = os.path.abspath(path)
        self.dir = os.path.dirname(self.path)
        with open(self.path, "rb") as fh:
            self.config = json.loads(fh.read().decode("utf-8"))
        cfg = self.config
        if cfg.get("schema") != ADAPTER_CONFIG_SCHEMA:
            raise AdapterConfigError("%s: schema must be %s" % (path, ADAPTER_CONFIG_SCHEMA))
        if cfg.get("protocol") != "ocs-adapter/1":
            raise AdapterConfigError("%s: unsupported protocol %r" % (path, cfg.get("protocol")))
        for key in ("name", "command"):
            if key not in cfg:
                raise AdapterConfigError("%s: missing %s" % (path, key))
        self.name = cfg["name"]
        self.overrides = dict(overrides or {})

    def expand(self, value, suite_root):
        value = value.replace("{adapter_dir}", self.dir).replace("{suite_root}", suite_root)
        # Launch configuration may need the host's tool locations (e.g. rustup
        # toolchains under ~/.rustup); the program itself still gets an
        # isolated HOME.
        value = value.replace("{host_home}", os.path.expanduser("~"))
        return value

    def command(self, suite_root):
        return [self.expand(part, suite_root) for part in self.config["command"]]

    def env(self, suite_root):
        env = {}
        for name in self.config.get("pass_env", []):
            if name.endswith("*"):
                for key, val in os.environ.items():
                    if key.startswith(name[:-1]):
                        env[key] = val
            elif name in os.environ:
                env[name] = os.environ[name]
        for key, val in self.config.get("env", {}).items():
            env[key] = self.expand(val, suite_root)
        env.update(self.overrides)
        return env

    def info(self, suite_root):
        """Ask the adapter to describe the implementation (revision, config)."""
        cmd = self.command(suite_root) + ["--info"]
        env = dict(BASE_ENV)
        env.update(self.env(suite_root))
        env["HOME"] = tempfile.gettempdir()
        try:
            proc = subprocess.run(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, env=env, timeout=300)
        except Exception as exc:
            return {"error": "adapter --info failed: %s" % exc}
        try:
            data = json.loads(proc.stdout.decode("utf-8"))
        except Exception:
            return {"error": "adapter --info returned non-JSON", "stdout": proc.stdout.decode("utf-8", "replace")[:2000],
                    "stderr": proc.stderr.decode("utf-8", "replace")[:2000], "exit": proc.returncode}
        data["command"] = cmd
        return data


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def tree_hashes(root):
    out = {}
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for fname in sorted(filenames):
            full = os.path.join(dirpath, fname)
            if os.path.islink(full):
                out[os.path.relpath(full, root)] = "symlink:" + os.readlink(full)
            else:
                out[os.path.relpath(full, root)] = sha256_file(full)
    return out


class FixtureError(Exception):
    pass


class RunningFixture(object):
    def __init__(self, name, proc, values, log_path):
        self.name = name
        self.proc = proc
        self.values = values
        self.log_path = log_path

    def stop(self):
        if self.proc is None:
            return None
        if self.proc.poll() is None:
            try:
                os.killpg(self.proc.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(self.proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.proc.wait(timeout=5)
        return self.proc.returncode


def start_fixture(suite_root, name, work, scratch, env):
    fdir = os.path.join(suite_root, "fixtures", name)
    meta_path = os.path.join(fdir, "fixture.json")
    if not os.path.isfile(meta_path):
        raise FixtureError("missing fixture %s (no fixtures/%s/fixture.json)" % (name, name))
    with open(meta_path, "rb") as fh:
        meta = json.loads(fh.read().decode("utf-8"))
    if meta.get("schema") != FIXTURE_SCHEMA:
        raise FixtureError("fixture %s: bad schema" % name)
    kind = meta.get("kind")
    if kind == "files":
        src = os.path.join(fdir, meta["path"])
        if not os.path.isdir(src):
            raise FixtureError("fixture %s: files directory missing" % name)
        dest = os.path.join(work, meta.get("target", "."))
        for dirpath, dirnames, filenames in os.walk(src):
            rel = os.path.relpath(dirpath, src)
            os.makedirs(os.path.join(dest, rel), exist_ok=True)
            for fname in filenames:
                target = os.path.join(dest, rel, fname)
                if os.path.exists(target):
                    raise FixtureError("fixture %s would overwrite %s" % (name, os.path.relpath(target, work)))
                shutil.copy2(os.path.join(dirpath, fname), target)
        return RunningFixture(name, None, {}, None)
    if kind == "service":
        cmd = [part.replace("{fixture_dir}", fdir).replace("{python}", sys.executable)
               for part in meta["command"]]
        log_path = os.path.join(scratch, "fixture-%s.log" % name)
        log = open(log_path, "wb")
        fenv = dict(env)
        fenv["OCS_FIXTURE_STATE"] = os.path.join(scratch, "fixture-%s-state" % name)
        os.makedirs(fenv["OCS_FIXTURE_STATE"], exist_ok=True)
        proc = subprocess.Popen(cmd, cwd=scratch, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=log, env=fenv, start_new_session=True)
        ready = re.compile(meta.get("ready_regex", r"^READY(.*)$"))
        deadline = time.time() + float(meta.get("ready_timeout_s", 20))
        line_holder = []

        def reader():
            line_holder.append(proc.stdout.readline())

        t = threading.Thread(target=reader, daemon=True)
        t.start()
        t.join(max(0.1, deadline - time.time()))
        line = line_holder[0].decode("utf-8", "replace").strip() if line_holder else ""
        m = ready.match(line)
        if not m:
            fx = RunningFixture(name, proc, {}, log_path)
            fx.stop()
            raise FixtureError("fixture %s did not become ready (first line %r)" % (name, line))
        values = dict(kv.split("=", 1) for kv in m.group(1).split() if "=" in kv)
        return RunningFixture(name, proc, values, log_path)
    raise FixtureError("fixture %s: unknown kind %r" % (name, kind))


def substitute(text, fixtures):
    def repl(m):
        fx, key = m.group(1), m.group(2)
        for f in fixtures:
            if f.name == fx and key in f.values:
                return f.values[key]
        raise FixtureError("unresolved placeholder %s" % m.group(0))
    return re.sub(r"\{fixture\.([a-z0-9-]+)\.([a-z0-9_]+)\}", repl, text)


def b64(data):
    return base64.b64encode(data).decode("ascii")


def text_or_none(data):
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def run_case(case, adapter, suite_root, stdlib_root, options):
    """Execute one case and return a JSON-serialisable result record."""
    timeout = float(case["budget"]["timeout_s"]) * options.timeout_scale
    scratch = os.path.realpath(tempfile.mkdtemp(prefix="ocs-", dir=options.tmp_root))
    work = os.path.join(scratch, "work")
    home = os.path.join(scratch, "home")
    os.makedirs(home)
    record = {
        "id": case.id, "level": case.level, "mode": case.mode, "status_class": case["status"],
        "contracts": case["contracts"], "scratch": scratch,
    }
    before = tree_hashes(case.dir)
    record["source_sha256"] = {k: v for k, v in before.items()}
    fixtures = []
    try:
        shutil.copytree(case.dir, work, symlinks=True,
                        ignore=shutil.ignore_patterns("case.json", "expected"))
        if case.get("harness", False):
            hdir = os.path.join(suite_root, "harness", "okkama", "conformance")
            dest = os.path.join(work, "conformance")
            if os.path.exists(dest):
                raise FixtureError("case directory already contains conformance/; harness would shadow it")
            shutil.copytree(hdir, dest)
            record["harness_sha256"] = tree_hashes(hdir)

        env = dict(BASE_ENV)
        env["HOME"] = home
        env["TMPDIR"] = os.path.join(scratch, "tmp")
        os.makedirs(env["TMPDIR"])
        launch_env = adapter.env(suite_root)
        for name in case.fixtures:
            fixtures.append(start_fixture(suite_root, name, work, scratch, env))
        args = [substitute(a, fixtures) for a in case.get("args", [])]
        case_env = {k: substitute(v, fixtures) for k, v in case.get("env", {}).items()}

        request = {
            "protocol": "ocs-adapter/1",
            "mode": case.mode,
            "sources": [os.path.join(work, s) for s in case["sources"]],
            "args": args,
            "cwd": work,
            "stdlib_root": stdlib_root,
            "timeout_s": timeout,
        }
        req_path = os.path.join(scratch, "request.json")
        resp_path = os.path.join(scratch, "response.json")
        with open(req_path, "w") as fh:
            json.dump(request, fh, indent=2, sort_keys=True)
        cmd = adapter.command(suite_root) + ["--request", req_path, "--response", resp_path]
        full_env = dict(env)
        full_env.update(launch_env)
        full_env.update(case_env)
        record["command"] = cmd
        record["request"] = request
        record["env"] = {"case": case_env, "base": env,
                         "adapter_keys": sorted(launch_env)}

        stdin = open(os.path.join(case.dir, case["stdin"]), "rb") if "stdin" in case.data else subprocess.DEVNULL
        started = time.time()
        proc = subprocess.Popen(cmd, cwd=work, stdin=stdin, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, env=full_env, start_new_session=True)
        timed_out = False
        try:
            out, err = proc.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            out, err = proc.communicate()
        finally:
            if stdin is not subprocess.DEVNULL:
                stdin.close()
        elapsed = time.time() - started
        # Kill anything the implementation left behind in its process group.
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass

        rc = proc.returncode
        sig = -rc if rc is not None and rc < 0 else None
        response, response_error = None, None
        if os.path.exists(resp_path):
            try:
                with open(resp_path, "rb") as fh:
                    response = json.loads(fh.read().decode("utf-8"))
            except Exception as exc:
                response_error = "response file is not valid JSON: %s" % exc
        else:
            response_error = "adapter wrote no response file"

        artifacts = {}
        for art in case["expect"].get("artifacts", []):
            p = os.path.join(work, art["path"])
            artifacts[art["path"]] = open(p, "rb").read() if os.path.isfile(p) else None

        obs = Observation(response=response, response_error=response_error,
                          stdout=out[:OUTPUT_LIMIT], stderr=err[:OUTPUT_LIMIT],
                          process_exit=None if sig else rc, process_signal=sig,
                          timed_out=timed_out, artifacts=artifacts, elapsed_s=elapsed)
        status, reasons = judge(case, obs)
        after = tree_hashes(case.dir)
        if after != before:
            status, reasons = "HARNESS_ERROR", ["case source tree changed during execution"] + reasons

        record.update({
            "status": status,
            "reasons": reasons,
            "elapsed_s": round(elapsed, 3),
            "timeout_s": timeout,
            "process": {"exit": rc if not sig else None, "signal": sig, "timed_out": timed_out},
            "response": response,
            "response_error": response_error,
            "stdout_b64": b64(out[:OUTPUT_LIMIT]),
            "stderr_b64": b64(err[:OUTPUT_LIMIT]),
            "stdout_text": text_or_none(out[:65536]),
            "stderr_text": text_or_none(err[:65536]),
            "artifacts_sha256": {k: (hashlib.sha256(v).hexdigest() if v is not None else None)
                                 for k, v in artifacts.items()},
        })
    except FixtureError as exc:
        record.update({"status": "BLOCKED", "reasons": [str(exc)]})
    except Exception as exc:  # runner bug or host problem: never a pass
        record.update({"status": "HARNESS_ERROR", "reasons": ["runner exception: %r" % (exc,)]})
    finally:
        fixture_logs = {}
        for fx in fixtures:
            code = fx.stop()
            if fx.log_path and os.path.exists(fx.log_path):
                with open(fx.log_path, "rb") as fh:
                    fixture_logs[fx.name] = {"exit": code, "log": fh.read()[-20000:].decode("utf-8", "replace")}
        if fixture_logs:
            record["fixture_logs"] = fixture_logs
        keep = options.keep == "all" or (options.keep == "failed" and record.get("status") != "PASS")
        if not keep:
            shutil.rmtree(scratch, ignore_errors=True)
            record["scratch"] = None
    return record
