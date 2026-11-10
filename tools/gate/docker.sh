#!/usr/bin/env bash
# The release gate in a container (tools/docker/Dockerfile): the working tree
# is copied into the container, built and checked there without network and
# with CPU, memory and process limits; the logs come back to
# conformance/reports/runs/.
#
#     tools/gate/docker.sh [--quick]
#
# Volumes keep the cargo registry and build outputs between runs
# (okkama-cargo-registry, okkama-gate-target, okkama-gate-adapter); each source
# gets the time its content was first seen by a gate (stamp_sources.py, #333),
# so a tree edited before another was gated is not mistaken for built, and
# every gate seals the record when it ends, so that a build no gate recorded
# (a tree older than #333) makes the next one rebuild everything (#334). Crates
# must already be in the registry volume (the container is offline); after a
# dependency change run once with OKKAMA_GATE_NETWORK=bridge.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# The container has no .git: gate.json names the commit from here.
COMMIT="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
[[ -n "$COMMIT" && -n "$(git -C "$ROOT" status --porcelain 2>/dev/null)" ]] && COMMIT="$COMMIT+dirty"
docker build -q --build-arg RUST_VERSION="$(cat "$ROOT/tools/release-toolchain.txt")" \
    -t okkama-dev "$ROOT/tools/docker" >/dev/null
mkdir -p "$ROOT/conformance/reports/runs"
exec docker run --rm \
    --network "${OKKAMA_GATE_NETWORK:-none}" \
    --cpus "${OKKAMA_GATE_CPUS:-6}" --memory "${OKKAMA_GATE_MEMORY:-7g}" --pids-limit 4096 \
    -e LANG=C.UTF-8 -e LC_ALL=C.UTF-8 -e GATE_COMMIT="$COMMIT" \
    -e CARGO_BUILD_JOBS="${OKKAMA_GATE_JOBS:-2}" \
    -e CARGO_NET_OFFLINE="$([[ "${OKKAMA_GATE_NETWORK:-none}" == none ]] && echo true || echo false)" \
    -v "$ROOT":/src:ro \
    -v "$ROOT/conformance/reports/runs":/out \
    -v okkama-cargo-registry:/usr/local/cargo/registry \
    -v okkama-gate-target:/work/okkama-rs/target \
    -v okkama-gate-adapter:/work/conformance/adapters/rust/.target \
    okkama-dev bash -c '
        # The conformance adapter uses $HOME/.cargo and $HOME/.rustup.
        ln -sfn /usr/local/cargo "$HOME/.cargo"; ln -sfn /usr/local/rustup "$HOME/.rustup"
        rsync -a --delete --exclude /okkama-rs/target --exclude /conformance/adapters/rust/.target \
            --exclude /conformance/reports/runs --exclude .git /src/ /work/
        # The build volumes are shared by every gate run: give each source the
        # time its content was first seen, so cargo rebuilds what changed (#333).
        python3 /work/tools/gate/stamp_sources.py stamp /work /work/okkama-rs/target/.gate-sources.json \
            --outputs /work/okkama-rs/target /work/conformance/adapters/rust/.target \
            --stale /work/conformance/adapters/rust/.target/ocs-probe.json
        cd /work && status=0 && tools/gate/gate.sh "$@" || status=$?
        # Whatever the outcome, the volumes now hold what this tree built.
        python3 /work/tools/gate/stamp_sources.py seal /work/okkama-rs/target/.gate-sources.json
        cp -R /work/conformance/reports/runs/gate-* /out/ 2>/dev/null || true
        exit $status' gate "$@"
