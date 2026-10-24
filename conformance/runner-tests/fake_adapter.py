#!/usr/bin/env python3
"""A deliberately misbehaving ocs-adapter/1 implementation for runner self-tests.

Behaviour is chosen by OCS_FAKE_BEHAVIOR. The adapter never sees expected
results; each behaviour models one way a real implementation or adapter can
be wrong, and the self-tests assert that the runner does not turn it into a
PASS.
"""

import json
import os
import signal
import sys
import time


def respond(path, mode, **fields):
    doc = {"protocol": "ocs-adapter/1", "adapter": {"name": "fake", "version": "1"}, "mode": mode}
    doc.update(fields)
    with open(path, "w") as fh:
        json.dump(doc, fh)


def main(argv):
    if argv == ["--info"]:
        print(json.dumps({"implementation": "fake", "revision": {"behavior": os.environ.get("OCS_FAKE_BEHAVIOR")},
                          "modes": ["parse", "check", "run", "test"]}))
        return 0
    opts = dict(zip(argv[::2], argv[1::2]))
    req = json.load(open(opts["--request"]))
    resp = opts["--response"]
    mode = req["mode"]
    b = os.environ.get("OCS_FAKE_BEHAVIOR", "success")
    # Behaviours that read the program text only to emulate correct output
    # for the control case; they never read case.json (it is not even copied).
    if b == "success":
        sys.stdout.write("hello\n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "accept":
        respond(resp, mode, outcome="accepted")
        return 0
    if b == "wrong-output":
        sys.stdout.write("hullo\n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "extra-trailing-space":
        sys.stdout.write("hello \n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "crlf-output":
        sys.stdout.write("hello\r\n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "unexpected-stderr":
        sys.stdout.write("hello\n")
        sys.stderr.write("warning: something\n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "wrong-exit":
        sys.stdout.write("hello\n")
        respond(resp, mode, outcome="completed", exit_status=7)
        return 7
    if b == "lying-exit":
        sys.stdout.write("hello\n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 3
    if b == "reject-syntax-parse":
        respond(resp, mode, outcome="rejected", failure={"stage": "parse", "category": "syntax", "message": "x"})
        return 1
    if b == "reject-semantic-check":
        respond(resp, mode, outcome="rejected", failure={"stage": "check", "category": "semantic", "message": "x"})
        return 1
    if b == "reject-syntax-check":
        respond(resp, mode, outcome="rejected", failure={"stage": "check", "category": "syntax", "message": "x"})
        return 1
    if b == "runtime-error":
        respond(resp, mode, outcome="failed", failure={"stage": "runtime", "category": "runtime", "message": "x"})
        return 1
    if b == "uncaught-other-type":
        respond(resp, mode, outcome="failed", failure={"stage": "runtime", "category": "uncaught-error",
                                                       "error_type": "OtherError", "message": "x"})
        return 1
    if b == "uncaught-right-type":
        respond(resp, mode, outcome="failed", failure={"stage": "runtime", "category": "uncaught-error",
                                                       "error_type": "Broken", "message": "x"})
        return 1
    if b == "failed-exit-zero":
        respond(resp, mode, outcome="failed", failure={"stage": "runtime", "category": "runtime", "message": "x"})
        return 0
    if b == "timeout":
        time.sleep(120)
        return 0
    if b == "crash":
        os.kill(os.getpid(), signal.SIGABRT)
        return 0
    if b == "internal-error":
        respond(resp, mode, outcome="internal-error", failure={"stage": "runtime", "category": "internal",
                                                               "message": "Can't call method on undef at X.pm line 3."})
        return 70
    if b == "malformed":
        with open(resp, "w") as fh:
            fh.write("{not json")
        return 0
    if b == "no-response":
        sys.stdout.write("hello\n")
        return 0
    if b == "unsupported":
        respond(resp, mode, outcome="unsupported", unsupported_reason="fake implementation has no runtime")
        return 3
    if b == "verdict":
        sys.stdout.write("hello\n")
        respond(resp, mode, outcome="completed", exit_status=0, status="PASS")
        return 0
    if b == "wrong-protocol":
        sys.stdout.write("hello\n")
        with open(resp, "w") as fh:
            json.dump({"protocol": "ocs-adapter/0", "mode": mode, "outcome": "completed", "exit_status": 0}, fh)
        return 0
    if b == "missing-async-completion":
        sys.stdout.write("started\n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "artifact-corrupt":
        with open(os.path.join(req["cwd"], "out.txt"), "w") as fh:
            fh.write("corrupted")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "artifact-missing":
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "artifact-ok":
        with open(os.path.join(req["cwd"], "out.txt"), "w") as fh:
            fh.write("payload\n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "modify-source":
        # Try to corrupt the original case: the runner runs on a copy, so the
        # original must remain unchanged and the run must not crash.
        with open(req["sources"][0], "a") as fh:
            fh.write("// tampered\n")
        sys.stdout.write("hello\n")
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    if b == "accept-in-run-mode":
        respond(resp, mode, outcome="accepted")
        return 0
    if b == "partial-order":
        sys.stdout.write(os.environ.get("OCS_FAKE_STDOUT", "").replace("\\n", "\n"))
        respond(resp, mode, outcome="completed", exit_status=0)
        return 0
    raise SystemExit("unknown behaviour %s" % b)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
