#!/usr/bin/env bash
# Ворота релиза (H-08): всё, что должно пройти перед выпуском версии.
#
#     tools/gate/gate.sh            # полный прогон (~30 минут)
#     tools/gate/gate.sh --quick    # без тестов Rust и без прогона sparrow-origin
#     make gate                     # в контейнере (tools/gate/docker.sh)
#
# Шаги:
#   1. release-сборка okkama и sparrow (провал — ворота останавливаются),
#      тесты Rust всего workspace (release и debug: глубина стека и
#      debug-проверки видны только там), clippy без предупреждений;
#   2. conformance на IR, JVM, Sparrow (ядро; компилирует каждую функцию
#      целиком), на IR без IR-проходов (OKKAMA_IR_PASSES=none), на IR со
#      сборкой циклов на каждом кандидате (OKKAMA_GC=stress) и, до 0.5, на прежнем VM (`sparrow-origin`, только для
#      сведения: у него нет нативов, добавленных с 0.4.0);
#   3. встроенные тесты всех модулей std на IR и Sparrow (tools/gate/std-tests.py).
#
# Статус каждого шага — код выхода его команды (ocs: 0 только если прошли все
# обязательные кейсы). Итог — gate.json в каталоге журналов: коммит, шаги со
# статусами и временем, известные провалы std. Process workers и сеть
# настоящие: гонять в контейнере или на CI-раннере.
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
STEPS="$LOGS/steps.tsv"
: >"$STEPS"

step() { printf '\n==> %s\n' "$1"; }
fail() { FAILED+=("$1"); printf '!! %s\n' "$1"; }
# record NAME STATUS SECONDS: pass, fail or informative (a failure that does
# not fail the gate).
record() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$STEPS"; }

# The summary for people and for the release workflow.
finish() {
    python3 - "$LOGS" "$ROOT" "${#FAILED[@]}" <<'EOF'
import json, os, subprocess, sys
logs, root, failed = sys.argv[1], sys.argv[2], int(sys.argv[3])
def git(*args):
    try:
        return subprocess.run(["git", "-C", root, *args], capture_output=True, text=True).stdout.strip() or None
    except OSError:
        return None
def toolchain():
    try:
        return subprocess.run(["rustc", "-V"], capture_output=True, text=True, cwd=os.path.join(root, "okkama-rs")).stdout.strip() or None
    except OSError:
        return None
steps = []
with open(os.path.join(logs, "steps.tsv")) as rows:
    for row in rows:
        name, status, seconds = row.rstrip("\n").split("\t")
        entry = {"name": name, "status": status, "seconds": int(seconds)}
        summary = os.path.join(logs, f"{name.replace(' ', '-')}", "summary.json")
        if os.path.exists(summary):
            mandatory = json.load(open(summary)).get("mandatory", {})
            entry["mandatory"] = {"total": mandatory.get("total"), "counts": mandatory.get("counts")}
        steps.append(entry)
known = []
std = os.path.join(logs, "std-tests.log")
if os.path.exists(std):
    known = [line.split("known:", 1)[1].strip() for line in open(std) if line.strip().startswith("known:")]
report = {
    "commit": os.environ.get("GITHUB_SHA") or os.environ.get("GATE_COMMIT") or git("rev-parse", "HEAD"),
    "toolchain": toolchain(),
    "passed": failed == 0,
    "steps": steps,
    "known_std_failures": known,
}
json.dump(report, open(os.path.join(logs, "gate.json"), "w"), indent=2)
EOF
    printf '\nlogs: %s\n' "$LOGS"
    if [[ ${#FAILED[@]} -gt 0 ]]; then
        printf 'GATE FAILED: %s\n' "${FAILED[*]}"
        exit 1
    fi
    echo "GATE PASSED"
    exit 0
}

step "build"
started=$SECONDS
if (cd "$ROOT/okkama-rs" && cargo build --release -p okkama-cli -p sparrow-cli >"$LOGS/build.log" 2>&1); then
    record build pass $((SECONDS - started))
    echo "$("$OKKAMA" --version)"
else
    tail -20 "$LOGS/build.log"
    record build fail $((SECONDS - started))
    fail "build"
    # Nothing later means anything without the binaries.
    finish
fi

if [[ $QUICK -eq 0 ]]; then
    step "Rust tests (release, workspace)"
    started=$SECONDS
    if (cd "$ROOT/okkama-rs" && cargo test --release --workspace --no-fail-fast >"$LOGS/cargo-test.log" 2>&1); then
        record "rust tests" pass $((SECONDS - started))
        echo "ok: $(grep -c '^test result: ok' "$LOGS/cargo-test.log") test binaries"
    else
        record "rust tests" fail $((SECONDS - started))
        grep -E "^test .* FAILED|panicked|^error" "$LOGS/cargo-test.log" | head -20
        fail "Rust tests"
    fi
fi

if [[ $QUICK -eq 0 ]]; then
    # Debug frames are larger and debug assertions are on: stack depth (drops
    # of deep value graphs, #28) and invariants fail here first (#42).
    step "Rust tests (debug, workspace)"
    started=$SECONDS
    if (cd "$ROOT/okkama-rs" && cargo test --workspace --no-fail-fast >"$LOGS/cargo-test-debug.log" 2>&1); then
        record "rust tests debug" pass $((SECONDS - started))
        echo "ok: $(grep -c '^test result: ok' "$LOGS/cargo-test-debug.log") test binaries"
    else
        record "rust tests debug" fail $((SECONDS - started))
        grep -E "^test .* FAILED|panicked|^error|SIGABRT|overflowed its stack" "$LOGS/cargo-test-debug.log" | head -20
        fail "Rust tests (debug)"
    fi

    step "clippy"
    started=$SECONDS
    if (cd "$ROOT/okkama-rs" && cargo clippy --workspace --all-targets -- -D warnings >"$LOGS/clippy.log" 2>&1); then
        record clippy pass $((SECONDS - started))
    else
        record clippy fail $((SECONDS - started))
        grep -E "^(warning|error)" -A4 "$LOGS/clippy.log" | head -30
        fail "clippy"
    fi
fi

conformance() {
    local name="$1"; shift
    local out="$LOGS/conformance-$name"
    started=$SECONDS
    (cd "$ROOT/conformance" && ./ocs run --adapter adapters/rust/adapter.json --profile full-release \
        --stdlib-root ../std -j 6 --out "$out" "$@" >"$LOGS/conformance-$name.log" 2>&1)
    local status=$?
    echo "$name: $(tail -1 "$LOGS/conformance-$name.log")"
    if [[ $status -eq 0 ]]; then
        record "conformance $name" pass $((SECONDS - started))
        return
    fi
    grep -E "^\[.*(FAIL|ERROR|CRASH|UNSUPPORTED|TIMEOUT)" "$LOGS/conformance-$name.log" | head -10
    # The previous VM lacks natives added since 0.4.0 (removed in 0.5, #14).
    if [[ "$name" == "sparrow-origin" ]]; then
        record "conformance $name" informative $((SECONDS - started))
        echo "(informative: not a gate failure)"
    else
        record "conformance $name" fail $((SECONDS - started))
        fail "conformance $name"
    fi
}

step "conformance"
conformance ir --adapter-env OCS_RUST_ENGINE=ir
conformance jvm --adapter-env OCS_RUST_ENGINE=jvm
conformance sparrow --adapter-env OCS_RUST_ENGINE=sparrow
# Every engine runs the passed OIR: the IR suite again with the passes off
# shows a pass that changes meaning (#29).
conformance ir-passes-off --adapter-env OCS_RUST_ENGINE=ir --adapter-env OKKAMA_IR_PASSES=none
# The IR cycle collector at every candidate: an edge it miscounts frees a
# live value at once instead of thousands of candidates later (#43).
conformance ir-gc-stress --adapter-env OCS_RUST_ENGINE=ir --adapter-env OKKAMA_GC=stress
if [[ $QUICK -eq 0 ]]; then
    conformance sparrow-origin --adapter-env OCS_RUST_ENGINE=sparrow-origin
fi

step "std inline tests"
started=$SECONDS
# Every std module compiled to Sparrow also passes the independent register
# allocation check (#30).
if OKKAMA_CHECK_REGISTERS=1 python3 "$ROOT/tools/gate/std-tests.py" | tee "$LOGS/std-tests.log"; [[ ${PIPESTATUS[0]} -eq 0 ]]; then
    record "std tests" pass $((SECONDS - started))
else
    record "std tests" fail $((SECONDS - started))
    fail "std inline tests"
fi

finish
