#!/usr/bin/env bash
# Coverage-guided fuzzing of the toolchain (H-17, #34).
#
#     tools/fuzz/run.sh [SECONDS] [--coverage]
#
# Runs every target of okkama-rs/fuzz for SECONDS (default 300) each:
# lex, parse (drops the AST, re-parses formatted source), analyze (lowering
# verifies), ir_verify (mutated OIR), spb (decode, verify, intrinsic
# contracts), exec (programs on the Sparrow core under budgets, no host
# natives). Needs nightly Rust and cargo-fuzz; mass runs belong in a container
# or on CI (.github/workflows/fuzz.yml). Corpora grow in
# okkama-rs/fuzz/corpus/<target> (seeded from std, conformance, examples and
# compiled SPB), findings land in okkama-rs/fuzz/artifacts/<target>, and with
# --coverage a summary per target in okkama-rs/fuzz/coverage/<target>.txt;
# each run's output is in okkama-rs/fuzz/logs/<target>.log.
# Exit status 1 when any target found something, 2 when the targets do not
# build (logs/build.log).
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TIME="${1:-300}"
COVERAGE=0
[[ "${2:-}" == "--coverage" ]] && COVERAGE=1
FUZZ="$ROOT/okkama-rs/fuzz"
TARGETS=(lex parse analyze ir_verify spb exec)
cd "$FUZZ"

# Seeds: every Okkama source of the repository for the source targets.
for target in lex parse analyze ir_verify exec; do
    mkdir -p "corpus/$target"
    find "$ROOT/std" "$ROOT/conformance/cases" "$ROOT/okkama-rs/examples" "$FUZZ/seeds" -name '*.ok' -print0 |
        while IFS= read -r -d '' file; do
            cp -n "$file" "corpus/$target/$(cksum <"$file" | cut -d' ' -f1).ok"
        done
done
# SPB seeds: the examples and the fuzz seeds, compiled.
mkdir -p corpus/spb
(cd "$ROOT/okkama-rs" && cargo build --release -q -p okkama-cli)
for file in "$ROOT/okkama-rs/examples"/*.ok "$FUZZ/seeds"/*.ok; do
    "$ROOT/okkama-rs/target/release/okkama" build "$file" --target sparrow -o "corpus/spb/$(basename "$file" .ok).spb" 2>/dev/null || true
done

# Build every target first: a build killed for memory must not be reported as
# a finding. An ASan build of cranelift-codegen with one codegen unit needs
# several gigabytes on its own, so a failed parallel build is retried with one
# job, continuing from the crates already built.
mkdir -p logs
if ! CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" cargo +nightly fuzz build >logs/build.log 2>&1 &&
    ! CARGO_BUILD_JOBS=1 cargo +nightly fuzz build >>logs/build.log 2>&1; then
    grep -E "^error|SIGKILL|could not compile" logs/build.log | head -10
    echo "BUILD FAILED (okkama-rs/fuzz/logs/build.log)"
    exit 2
fi

found=()
for target in "${TARGETS[@]}"; do
    echo "==> $target (${TIME}s)"
    mkdir -p "artifacts/$target"
    if ! cargo +nightly fuzz run "$target" "corpus/$target" -- \
        -max_total_time="$TIME" -rss_limit_mb=2048 -timeout=30 -max_len=65536 -print_final_stats=1 \
        >"logs/$target.log" 2>&1; then
        found+=("$target")
        grep -E "^==[0-9]+==|panicked|ERROR|SUMMARY|Test unit written|error" "logs/$target.log" | head -20
    fi
    grep -E "^Done|stat::(number_of_executed_units|new_units_added|peak_rss_mb)" "logs/$target.log"
    if [[ $COVERAGE -eq 1 ]]; then
        mkdir -p coverage
        if cargo +nightly fuzz coverage "$target" "corpus/$target" >/dev/null 2>&1; then
            llvm="$(rustc +nightly --print sysroot)/lib/rustlib/$(rustc +nightly -vV | sed -n 's/^host: //p')/bin"
            binary="$(find target -path "*/coverage/*/release/$target" -type f | head -1)"
            "$llvm/llvm-cov" report "$binary" -instr-profile="coverage/$target/coverage.profdata" \
                -ignore-filename-regex='(\.cargo|rustc)/' >"coverage/$target.txt" 2>&1 || true
            tail -1 "coverage/$target.txt"
        fi
    fi
done
if [[ ${#found[@]} -gt 0 ]]; then
    echo "FINDINGS: ${found[*]} (okkama-rs/fuzz/artifacts)"
    exit 1
fi
echo "no findings"
