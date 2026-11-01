#!/usr/bin/env bash
# Ворота релиза (H-08): всё, что должно пройти перед выпуском версии.
#
#     tools/gate/gate.sh            # полный прогон (~30 минут)
#     tools/gate/gate.sh --quick    # без тестов Rust и без порогов 2 и 10
#     make gate
#
# Шаги:
#   1. release-сборка okkama и sparrow, тесты Rust всего workspace (release);
#   2. conformance на IR, JVM и Sparrow, а Sparrow — ещё в режимах JIT
#      eager и с порогами горячести 2 и 10 (смешанное исполнение и OSR);
#   3. подтверждение, что JIT и OSR действительно работали (OKKAMA_JIT_STATS);
#   4. встроенные тесты всех модулей std на IR и Sparrow (tools/gate/std-tests.py).
#
# Журналы — в conformance/reports/runs/gate-<время>/. Код выхода 0 — всё прошло.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
QUICK=0
[[ "${1:-}" == "--quick" ]] && QUICK=1
LOGS="$ROOT/conformance/reports/runs/gate-$(date +%Y%m%dT%H%M%S)"
mkdir -p "$LOGS"
OKKAMA="$ROOT/okkama-rs/target/release/okkama"
export OKKAMA_STDLIB_DIR="$ROOT/std"
FAILED=()

step() { printf '\n==> %s\n' "$1"; }
fail() { FAILED+=("$1"); printf '!! %s\n' "$1"; }

step "build"
(cd "$ROOT/okkama-rs" && cargo build --release -p okkama-cli -p sparrow-cli >"$LOGS/build.log" 2>&1) || fail "build"
echo "$("$OKKAMA" --version)"

if [[ $QUICK -eq 0 ]]; then
    step "Rust tests (release, workspace)"
    if (cd "$ROOT/okkama-rs" && cargo test --release --workspace --no-fail-fast >"$LOGS/cargo-test.log" 2>&1); then
        echo "ok: $(grep -c '^test result: ok' "$LOGS/cargo-test.log") test binaries"
    else
        grep -E "^test .* FAILED|panicked" "$LOGS/cargo-test.log" | head -20
        fail "Rust tests"
    fi
fi

conformance() {
    local name="$1"; shift
    (cd "$ROOT/conformance" && ./ocs run --adapter adapters/rust/adapter.json --profile full-release \
        --stdlib-root ../std -j 6 "$@" >"$LOGS/conformance-$name.log" 2>&1)
    local summary
    summary="$(tail -1 "$LOGS/conformance-$name.log")"
    echo "$name: $summary"
    if ! [[ "$summary" =~ ([0-9]+)/([0-9]+)\ PASS && ${BASH_REMATCH[1]} == "${BASH_REMATCH[2]}" ]]; then
        grep -E "^\[.*(FAIL|ERROR)" "$LOGS/conformance-$name.log" | head -10
        fail "conformance $name"
    fi
}

step "conformance"
conformance ir --adapter-env OCS_RUST_ENGINE=ir
conformance jvm --adapter-env OCS_RUST_ENGINE=jvm
conformance sparrow --adapter-env OCS_RUST_ENGINE=sparrow
conformance sparrow-eager --adapter-env OCS_RUST_ENGINE=sparrow --adapter-env OKKAMA_JIT=eager
if [[ $QUICK -eq 0 ]]; then
    conformance sparrow-threshold-2 --adapter-env OCS_RUST_ENGINE=sparrow --adapter-env OKKAMA_JIT_THRESHOLD=2
    conformance sparrow-threshold-10 --adapter-env OCS_RUST_ENGINE=sparrow --adapter-env OKKAMA_JIT_THRESHOLD=10
fi

step "JIT and OSR actually ran"
compiled=0; osr=0
for bench in 01-arith 02-calls 07-objects 09-closures; do
    line="$(OKKAMA_JIT_STATS=1 OKKAMA_JIT_THRESHOLD=2 "$OKKAMA" run --engine sparrow "$ROOT/benchmarks/$bench/main.ok" 2>&1 >/dev/null \
        | grep '^jit: compiled' || true)"
    echo "$bench: ${line:-no JIT statistics}"
    if [[ "$line" =~ compiled\ ([0-9]+),.*osr\ ([0-9]+) ]]; then
        compiled=$((compiled + BASH_REMATCH[1])); osr=$((osr + BASH_REMATCH[2]))
    fi
done
if [[ $compiled -eq 0 || $osr -eq 0 ]]; then
    fail "JIT evidence (compiled $compiled, osr $osr)"
fi

step "std inline tests"
python3 "$ROOT/tools/gate/std-tests.py" | tee "$LOGS/std-tests.log" || fail "std inline tests"

printf '\nlogs: %s\n' "$LOGS"
if [[ ${#FAILED[@]} -gt 0 ]]; then
    printf 'GATE FAILED: %s\n' "${FAILED[*]}"
    exit 1
fi
echo "GATE PASSED"
