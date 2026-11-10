#!/usr/bin/env python3
"""Give every build input the time its content was first seen by a gate (#333).

    tools/gate/stamp_sources.py stamp ROOT MANIFEST --outputs DIR … [--stale FILE …]
    tools/gate/stamp_sources.py seal MANIFEST

The gate copies the tree into its container keeping the files' times, and
builds into volumes shared by every gate run (`okkama-gate-target`,
`okkama-gate-adapter`). Cargo rebuilds a crate when a source is newer than
its last build, so a tree edited before another tree was gated looked older
than that tree's artifacts, and the gate ran conformance on binaries built
from someone else's sources.

`stamp` gives each file under ROOT/okkama-rs (build outputs aside), as its
modification time, the moment a gate first saw its present content: the
time kept in MANIFEST when the content is unchanged, now when it is new or
changed. Every artifact built from older content is then older than the
change, in any volume and whatever steps of earlier gates ran, and nothing
unchanged is rebuilt. The manifest lives in the target volume; without it
every file is new once. When anything changed, each STALE file is removed:
the OCS adapter's probe cache, which maps a hash of the sources to "built"
and so would skip cargo for sources it once saw, even if that build reused
another tree's binary.

The manifest describes the tree the volumes were last built from only if
no one else built there since (#334): a gate from a tree older than #333
builds into the same volumes without stamping. So every gate `seal`s the
manifest when it ends, failed or not, and `stamp` compares the seal with
the build outputs (each output directory, the entries in it, and the
entries one level below: profiles, `deps`, `build`, `.fingerprint`,
binaries). An output changed after the seal, or no seal (a gate killed
before its end, an older manifest), makes every file new: one full
rebuild instead of a run on stale binaries.
"""
import argparse
import hashlib
import json
import os
import time

SKIP = {"target", ".target", ".git"}


def load(manifest_path):
    try:
        with open(manifest_path) as f:
            manifest = json.load(f)
    except (OSError, ValueError):
        return {"files": {}, "sealed": None}
    # A manifest from before the seal (#333) is a bare map of files.
    if "files" not in manifest:
        return {"files": manifest, "sealed": None}
    return manifest


def save(manifest_path, manifest):
    os.makedirs(os.path.dirname(manifest_path), exist_ok=True)
    with open(manifest_path + ".tmp", "w") as f:
        json.dump(manifest, f)
    os.replace(manifest_path + ".tmp", manifest_path)


def newest_output(directories, skip):
    """The latest change among the build outputs, two levels down."""
    newest = 0.0
    for top in directories:
        try:
            entries = list(os.scandir(top))
        except OSError:
            continue
        for entry in entries:
            if entry.name in skip:
                continue
            newest = max(newest, entry.stat(follow_symlinks=False).st_mtime)
            if entry.is_dir(follow_symlinks=False):
                for inner in os.scandir(entry.path):
                    newest = max(newest, inner.stat(follow_symlinks=False).st_mtime)
    return newest


def stamp(args):
    manifest = load(args.manifest)
    skip = {os.path.basename(args.manifest), os.path.basename(args.manifest) + ".tmp"}
    sealed = manifest["sealed"]
    seen = manifest["files"]
    if sealed is None or newest_output(args.outputs, skip) > sealed:
        why = "no gate sealed the record" if sealed is None else "the build outputs changed after the last sealed gate"
        print(f"stamp_sources: {why}: every input is new")
        seen = {}
    now = time.time()
    stamps, changed = {}, 0
    base = os.path.join(args.root, "okkama-rs")
    for directory, dirnames, filenames in os.walk(base):
        dirnames[:] = [d for d in dirnames if d not in SKIP]
        for name in filenames:
            path = os.path.join(directory, name)
            if os.path.islink(path):
                continue
            with open(path, "rb") as f:
                digest = hashlib.sha256(f.read()).hexdigest()
            key = os.path.relpath(path, args.root)
            previous = seen.get(key)
            if previous and previous[0] == digest:
                when = previous[1]
            else:
                when, changed = now, changed + 1
            os.utime(path, (when, when))
            stamps[key] = [digest, when]
    # Unsealed until this gate ends: if it never does, the next one rebuilds.
    save(args.manifest, {"files": stamps, "sealed": None})
    print(f"stamp_sources: {changed} of {len(stamps)} build inputs new or changed since the last gate")
    if changed:
        for stale in args.stale:
            if os.path.exists(stale):
                os.remove(stale)
                print(f"stamp_sources: removed {stale}")


def seal(args):
    manifest = load(args.manifest)
    manifest["sealed"] = time.time()
    save(args.manifest, manifest)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    s = commands.add_parser("stamp")
    s.add_argument("root")
    s.add_argument("manifest")
    s.add_argument("--outputs", nargs="+", default=[])
    s.add_argument("--stale", nargs="*", default=[])
    s.set_defaults(run=stamp)
    z = commands.add_parser("seal")
    z.add_argument("manifest")
    z.set_defaults(run=seal)
    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
