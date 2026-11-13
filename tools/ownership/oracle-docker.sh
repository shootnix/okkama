#!/usr/bin/env bash
# Usage: oracle-docker.sh MODE [--results HOST_DIR] [--bench HOST_PATH] [oracle options]
# Requires the existing gate image and a populated offline cargo registry volume.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
if [[ $# == 0 || "$1" == --help || "$1" == -h ]]; then
    echo 'Usage: oracle-docker.sh {engines|ab|stats|tap} [--results HOST_DIR] [--bench HOST_PATH] [oracle options]'
    echo 'Default results: tools/ownership/results; oracle --json paths are inside /out.'
    exit 0
fi
MODE=$1; shift
case "$MODE" in engines|ab|stats|tap) ;; *) echo "Unknown mode: $MODE" >&2; exit 2 ;; esac
RESULTS="$ROOT/tools/ownership/results"
BENCH=
ARGS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --results|--bench)
            [[ $# -ge 2 && -n "$2" ]] || { echo "Missing value for $1" >&2; exit 2; }
            if [[ "$1" == --results ]]; then RESULTS=$2; else BENCH=$2; fi
            shift 2 ;;
        --root|--root=*|--toolchain|--toolchain=*|--allow-host)
            echo "$1 is managed by the Docker wrapper" >&2; exit 2 ;;
        *) ARGS+=("$1"); shift ;;
    esac
done
mkdir -p "$RESULTS"
RESULTS="$(cd "$RESULTS" && pwd)"
MOUNTS=()
if [[ -n "$BENCH" ]]; then
    if [[ -d "$BENCH" ]]; then
        BENCH="$(cd "$BENCH" && pwd)"
        MOUNTS+=(-v "$BENCH:/bench:ro")
        ARGS+=(--bench /bench)
    elif [[ -f "$BENCH" ]]; then
        BENCH_DIR="$(cd "$(dirname "$BENCH")" && pwd)"
        MOUNTS+=(-v "$BENCH_DIR:/bench:ro")
        ARGS+=(--bench "/bench/$(basename "$BENCH")")
    else
        echo "Benchmark path does not exist: $BENCH" >&2; exit 2
    fi
fi
exec docker run --rm --network none \
    --cpus "${ORACLE_CPUS:-3}" --memory "${ORACLE_MEMORY:-6g}" --pids-limit 512 \
    -e LANG=C.UTF-8 -e LC_ALL=C.UTF-8 \
    -e CARGO_BUILD_JOBS="${ORACLE_CARGO_JOBS:-2}" \
    -e ORACLE_BUILD_TIMEOUT="${ORACLE_BUILD_TIMEOUT:-1800}" \
    -v "$ROOT:/src:ro" -v "$RESULTS:/out" \
    -v okkama-cargo-registry:/usr/local/cargo/registry \
    ${MOUNTS[@]+"${MOUNTS[@]}"} -w /out okkama-dev bash -c '
        set -euo pipefail
        /src/tools/ownership/toolchain.sh
        exec python3 /work/tools/ownership/oracle.py "$@" --root /work --toolchain /tc
    ' oracle "$MODE" --out /out/artifacts --json "/out/$MODE.json" ${ARGS[@]+"${ARGS[@]}"}
