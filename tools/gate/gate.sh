#!/usr/bin/env bash
# Ворота релиза (H-08): всё, что должно пройти перед выпуском версии.
#
#     tools/gate/gate.sh            # полный прогон (~30 минут)
#     tools/gate/gate.sh --quick    # без тестов Rust и без прогона sparrow-origin
#     make gate
#
# Шаги:
#   1. release-сборка okkama и sparrow, тесты Rust всего workspace (release);
#   2. conformance на IR, JVM, Sparrow (ядро; компилирует каждую функцию
#      целиком) и, до 0.5, на прежнем VM (`sparrow-origin`);
#   3. встроенные тесты всех модулей std на IR и Sparrow (tools/gate/std-tests.py).
#
# Process workers и сеть настоящие: гонять в контейнере (tools/gate/docker.sh).
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
if [[ $QUICK -eq 0 ]]; then
    conformance sparrow-origin --adapter-env OCS_RUST_ENGINE=sparrow-origin
fi

step "std inline tests"
python3 "$ROOT/tools/gate/std-tests.py" | tee "$LOGS/std-tests.log" || fail "std inline tests"

printf '\nlogs: %s\n' "$LOGS"
if [[ ${#FAILED[@]} -gt 0 ]]; then
    printf 'GATE FAILED: %s\n' "${FAILED[*]}"
    exit 1
fi
echo "GATE PASSED"
