#!/usr/bin/env python3
"""OCS adapter for the developing Rust implementation (okkama-rs).

Protocol: ocs-adapter/1 (conformance/adapters/PROTOCOL.md).

The Rust implementation is expected to grow a native protocol endpoint:

    okkama-rs --ocs-capabilities
        -> prints {"protocol": "ocs-adapter/1", "modes": ["parse", ...]}
    okkama-rs --ocs-request REQUEST.json --ocs-response RESPONSE.json
        -> behaves exactly like an ocs-adapter/1 adapter for one request

This adapter builds the crate (into a cache directory, never into the
implementation tree), asks the binary for its capabilities and forwards only
the modes it declares. Everything else is reported as UNSUPPORTED with the
observed evidence. It never fabricates parse/check/run results, and it never
treats a parser as a runtime.
"""

import json
import os
import subprocess
import sys
import time

PROTOCOL = "ocs-adapter/1"
HERE = os.path.dirname(os.path.abspath(__file__))


def impl_root():
    root = os.environ.get("OCS_RUST_IMPLEMENTATION_ROOT")
    if not root:
        raise SystemExit("OCS_RUST_IMPLEMENTATION_ROOT is not set")
    return os.path.abspath(root)


def target_dir():
    return os.path.abspath(os.environ.get("OCS_RUST_TARGET_DIR") or os.path.join(HERE, ".target"))


def run(cmd, cwd=None, timeout=600, env=None):
    try:
        p = subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                           stdin=subprocess.DEVNULL, timeout=timeout, env=env)
        return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace")
    except FileNotFoundError as exc:
        return 127, "", str(exc)
    except subprocess.TimeoutExpired:
        return 124, "", "timeout"


def git(root, *args):
    code, out, _ = run(["git", "-C", root] + list(args), timeout=30)
    return out.strip() if code == 0 else None


def source_fingerprint(root):
    """Hash of the implementation's build inputs; the cached probe is reused while it is unchanged."""
    import hashlib
    h = hashlib.sha256()
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in ("target", ".git"))
        for fname in sorted(filenames):
            # Embedded runtime class files (include_bytes!) are build inputs too.
            if fname.endswith((".rs", ".toml", ".lock", ".class")):
                full = os.path.join(dirpath, fname)
                h.update(os.path.relpath(full, root).encode() + b"\0")
                with open(full, "rb") as fh:
                    h.update(hashlib.sha256(fh.read()).digest())
    return h.hexdigest()


def probe():
    """Build and query the implementation once per source fingerprint.

    Concurrent adapter invocations serialise on a lock file and reuse the
    cached evidence, so parallel cases neither race on cargo's target lock nor
    spend their time budget rebuilding an unchanged crate.
    """
    import fcntl
    root = impl_root()
    os.makedirs(target_dir(), exist_ok=True)
    cache = os.path.join(target_dir(), "ocs-probe.json")
    fingerprint = source_fingerprint(root) if os.path.isdir(root) else None
    engine = os.environ.get("OCS_RUST_ENGINE", "ir")
    with open(os.path.join(target_dir(), "ocs-probe.lock"), "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        engines = {}
        try:
            with open(cache) as fh:
                engines = json.load(fh).get("engines", {})
            cached = engines.get(engine, {})
            if (cached.get("fingerprint") == fingerprint and fingerprint is not None
                    and cached.get("engine") == engine):
                cached["cached"] = True
                return cached
        except (OSError, ValueError):
            pass
        ev = probe_uncached()
        ev["fingerprint"] = fingerprint
        ev["engine"] = engine
        engines[engine] = ev
        with open(cache, "w") as fh:
            json.dump({"engines": engines}, fh)
        return ev


def probe_uncached():
    """Build and query the implementation. Returns a dict of evidence."""
    root = impl_root()
    ev = {"root": root, "exists": os.path.isdir(root)}
    if not ev["exists"]:
        ev["modes"] = []
        ev["reason"] = "implementation root does not exist"
        return ev
    ev["git_head"] = git(root, "rev-parse", "--verify", "-q", "HEAD")
    ev["git_status_entries"] = len((git(root, "status", "--porcelain") or "").splitlines())
    ev["rust_sources"] = sorted(os.path.relpath(os.path.join(d, f), root)
                                for d, _, fs in os.walk(os.path.join(root, "src")) for f in fs if f.endswith(".rs"))
    code, out, err = run(["cargo", "--version"])
    ev["cargo"] = out.strip() if code == 0 else "unavailable: " + err.strip()
    if code != 0:
        ev["modes"] = []
        ev["reason"] = "cargo is not available"
        return ev
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = target_dir()
    code, out, err = run(["cargo", "build", "--quiet", "--offline"], cwd=root, env=env)
    ev["build_exit"] = code
    ev["build_stderr_tail"] = err[-2000:]
    if code != 0:
        ev["modes"] = []
        ev["reason"] = "cargo build failed"
        return ev
    name = "okkama-rs"
    try:
        import tomllib  # Python 3.11+
        with open(os.path.join(root, "Cargo.toml"), "rb") as fh:
            name = tomllib.load(fh)["package"]["name"]
    except Exception:
        for line in open(os.path.join(root, "Cargo.toml")):
            if line.strip().startswith("name"):
                name = line.split("=", 1)[1].strip().strip('"')
                break
    # A workspace has no [package]; its CLI crate builds the `okkama` binary.
    # Prefer it so a stale binary of an older single-crate layout is ignored.
    binary = os.path.join(target_dir(), "debug", "okkama")
    if not os.path.exists(binary):
        binary = os.path.join(target_dir(), "debug", name)
    ev["binary"] = binary
    code, out, err = run([binary, "--ocs-capabilities"], timeout=30)
    ev["capabilities_exit"] = code
    ev["capabilities_stdout"] = out[:2000]
    try:
        caps = json.loads(out)
        if caps.get("protocol") != PROTOCOL or not isinstance(caps.get("modes"), list):
            raise ValueError("wrong protocol")
        ev["modes"] = [m for m in caps["modes"] if m in ("parse", "check", "run", "test")]
        ev["reason"] = None
    except Exception:
        ev["modes"] = []
        ev["reason"] = ("binary does not implement the ocs-adapter/1 capability query "
                        "(`--ocs-capabilities` printed %r, exit %s); no parse/check/run/test contract is available"
                        % (out.strip()[:200], code))
    return ev


def main(argv):
    if argv == ["--info"]:
        ev = probe()
        print(json.dumps({
            "protocol": PROTOCOL,
            "adapter": {"name": "okkama-rust", "version": "1"},
            "implementation": "okkama-rs (Rust implementation, in development)",
            "revision": {"git_head": ev.get("git_head"), "status_entries": ev.get("git_status_entries")},
            "configuration": {k: ev.get(k) for k in ("root", "cargo", "binary", "rust_sources", "build_exit", "engine")},
            "modes": ev.get("modes", []),
            "unsupported_reason": ev.get("reason"),
        }, indent=2, sort_keys=True))
        return 0
    opts = dict(zip(argv[::2], argv[1::2]))
    req_path, resp_path = opts.get("--request"), opts.get("--response")
    if not req_path or not resp_path:
        print("usage: okkama_rust_adapter.py --info | --request R --response P", file=sys.stderr)
        return 2
    started = time.time()
    with open(req_path) as fh:
        req = json.load(fh)
    mode = req.get("mode")
    ev = probe()
    if mode not in ev.get("modes", []):
        reason = ev.get("reason") or ("mode %r is not declared by the implementation (declared: %s)" % (mode, ev["modes"]))
        doc = {"protocol": PROTOCOL, "adapter": {"name": "okkama-rust", "version": "1"}, "mode": mode,
               "outcome": "unsupported", "unsupported_reason": reason,
               "evidence": {k: ev.get(k) for k in ("root", "git_head", "rust_sources", "build_exit", "capabilities_exit")},
               "elapsed_ms": int((time.time() - started) * 1000)}
        with open(resp_path, "w") as fh:
            json.dump(doc, fh, indent=2, sort_keys=True)
        return 3
    # The implementation speaks the protocol natively: forward unchanged.
    p = subprocess.run([ev["binary"], "--ocs-request", req_path, "--ocs-response", resp_path], cwd=req["cwd"])
    return p.returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
