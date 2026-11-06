#!/usr/bin/env bash
# The release gate in a container (tools/docker/Dockerfile): the working tree
# is copied into the container, built and checked there without network and
# with CPU, memory and process limits; the logs come back to
# conformance/reports/runs/.
#
#     tools/gate/docker.sh [--quick]
#
# Volumes keep the cargo registry and build outputs between runs
# (okkama-cargo-registry, okkama-gate-target). Crates must already be in the
# registry volume (the container is offline); after a dependency change run
# once with OKKAMA_GATE_NETWORK=bridge.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
docker build -q -t okkama-dev "$ROOT/tools/docker" >/dev/null
mkdir -p "$ROOT/conformance/reports/runs"
exec docker run --rm \
    --network "${OKKAMA_GATE_NETWORK:-none}" \
    --cpus "${OKKAMA_GATE_CPUS:-6}" --memory "${OKKAMA_GATE_MEMORY:-12g}" --pids-limit 4096 \
    -e CARGO_NET_OFFLINE="$([[ "${OKKAMA_GATE_NETWORK:-none}" == none ]] && echo true || echo false)" \
    -v "$ROOT":/src:ro \
    -v "$ROOT/conformance/reports/runs":/out \
    -v okkama-cargo-registry:/usr/local/cargo/registry \
    -v okkama-gate-target:/work/okkama-rs/target \
    -v okkama-gate-adapter:/work/conformance/adapters/rust/.target \
    okkama-dev bash -c '
        rsync -a --delete --exclude /okkama-rs/target --exclude /conformance/adapters/rust/.target \
            --exclude /conformance/reports/runs --exclude .git /src/ /work/
        cd /work && status=0 && tools/gate/gate.sh "$@" || status=$?
        cp -R /work/conformance/reports/runs/gate-* /out/ 2>/dev/null || true
        exit $status' gate "$@"
