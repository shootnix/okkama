#!/usr/bin/env python3
"""Give every build input the time its content was first seen by a gate (#333).

    tools/gate/stamp_sources.py ROOT MANIFEST

The gate copies the tree into its container keeping the files' times, and
builds into volumes shared by every gate run (`okkama-gate-target`,
`okkama-gate-adapter`). Cargo rebuilds a crate when a source is newer than
its last build, so a tree edited before another tree was gated looked older
than that tree's artifacts, and the gate ran conformance on binaries built
from someone else's sources.

Here each file under ROOT/okkama-rs (build outputs aside) gets, as its
modification time, the moment a gate first saw its present content: the
time kept in MANIFEST when the content is unchanged, now when it is new or
changed. Every artifact built from older content is then older than the
change, in any volume and whatever steps of earlier gates ran, and nothing
unchanged is rebuilt. The manifest lives in the target volume; without it
every file is new once.
"""
import hashlib
import json
import os
import sys
import time

SKIP = {"target", ".target", ".git"}


def main():
    root, manifest_path = sys.argv[1], sys.argv[2]
    try:
        with open(manifest_path) as f:
            seen = json.load(f)
    except (OSError, ValueError):
        seen = {}
    now = time.time()
    stamps, changed = {}, 0
    base = os.path.join(root, "okkama-rs")
    for directory, dirnames, filenames in os.walk(base):
        dirnames[:] = [d for d in dirnames if d not in SKIP]
        for name in filenames:
            path = os.path.join(directory, name)
            if os.path.islink(path):
                continue
            with open(path, "rb") as f:
                digest = hashlib.sha256(f.read()).hexdigest()
            key = os.path.relpath(path, root)
            previous = seen.get(key)
            if previous and previous[0] == digest:
                when = previous[1]
            else:
                when, changed = now, changed + 1
            os.utime(path, (when, when))
            stamps[key] = [digest, when]
    os.makedirs(os.path.dirname(manifest_path), exist_ok=True)
    with open(manifest_path + ".tmp", "w") as f:
        json.dump(stamps, f)
    os.replace(manifest_path + ".tmp", manifest_path)
    print(f"stamp_sources: {changed} of {len(stamps)} build inputs new or changed since the last gate")


if __name__ == "__main__":
    main()
