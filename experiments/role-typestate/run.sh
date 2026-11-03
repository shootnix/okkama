#!/usr/bin/env bash
# Runs every experiment and compares the outcome with its `// EXPECT:` header.
#
#   EXPECT: run-ok                  `okkama check` passes and every engine exits 0
#                                   (optional `// STDOUT: a|b` lines joined by `|`)
#   EXPECT: check-error: TEXT       `okkama check` fails and reports TEXT
#   EXPECT: runtime-error: TEXT     `okkama check` passes, the run fails with TEXT
#   EXPECT: blocked: TEXT           known implementation defect; TEXT appears in
#                                   the check or run output (reproduced = PASS)
#
# Environment:
#   OKKAMA        compiler binary (default: `okkama` on PATH)
#   ENGINES       engines for run-ok programs (default: "ir"; e.g. "ir jvm")
#   OKKAMA_STDLIB_DIR  defaults to this repository's std/
#
# Exact diagnostics are kept in logs/ (overwritten on each run).
set -u
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
okkama="${OKKAMA:-okkama}"
engines="${ENGINES:-ir}"
export OKKAMA_STDLIB_DIR="${OKKAMA_STDLIB_DIR:-$repo/std}"

cd "$here"
rm -rf logs && mkdir -p logs
pass=0
fail=0
summary="logs/summary.txt"
: > "$summary"

header() { sed -n "s|^// $1: ||p" "$2" | head -n 1; }

record() {
    local status="$1" file="$2" note="$3"
    printf '%-6s %-48s %s\n' "$status" "$file" "$note" | tee -a "$summary"
    if [ "$status" = PASS ]; then pass=$((pass + 1)); else fail=$((fail + 1)); fi
}

for file in positive/*.ok negative/*.ok advanced/*.ok; do
    [ -f "$file" ] || continue
    expect="$(header EXPECT "$file")"
    log="logs/${file//\//__}"
    "$okkama" check "$file" > "$log.check" 2>&1
    checked=$?
    case "$expect" in
    run-ok)
        if [ $checked -ne 0 ]; then
            record FAIL "$file" "check failed: $(head -n 1 "$log.check")"
            continue
        fi
        want="$(sed -n 's|^// STDOUT: ||p' "$file" | paste -sd '|' -)"
        ok=1
        for engine in $engines; do
            "$okkama" run --engine "$engine" "$file" > "$log.$engine.out" 2> "$log.$engine.err"
            status=$?
            got="$(paste -sd '|' - < "$log.$engine.out")"
            if [ $status -ne 0 ]; then
                record FAIL "$file" "[$engine] exit $status: $(head -n 1 "$log.$engine.err")"
                ok=0
            elif [ -n "$want" ] && [ "$got" != "$want" ]; then
                record FAIL "$file" "[$engine] stdout: $got"
                ok=0
            fi
        done
        [ $ok -eq 1 ] && record PASS "$file" "run-ok [$engines]"
        ;;
    check-error:*)
        text="${expect#check-error: }"
        if [ $checked -eq 0 ]; then
            record FAIL "$file" "check passed, expected: $text"
        elif grep -qF -- "$text" "$log.check"; then
            record PASS "$file" "rejected: $text"
        else
            record FAIL "$file" "wrong diagnostic: $(head -n 1 "$log.check")"
        fi
        ;;
    runtime-error:*)
        text="${expect#runtime-error: }"
        if [ $checked -ne 0 ]; then
            record FAIL "$file" "check failed: $(head -n 1 "$log.check")"
            continue
        fi
        ok=1
        for engine in $engines; do
            "$okkama" run --engine "$engine" "$file" > "$log.$engine.out" 2> "$log.$engine.err"
            status=$?
            if [ $status -eq 0 ]; then
                record FAIL "$file" "[$engine] ran successfully, expected: $text"
                ok=0
            elif ! grep -qF -- "$text" "$log.$engine.err"; then
                record FAIL "$file" "[$engine] wrong error: $(head -n 1 "$log.$engine.err")"
                ok=0
            fi
        done
        [ $ok -eq 1 ] && record PASS "$file" "runtime: $text"
        ;;
    blocked:*)
        text="${expect#blocked: }"
        "$okkama" run --engine ir "$file" > "$log.ir.out" 2> "$log.ir.err"
        if cat "$log.check" "$log.ir.err" | grep -qF -- "$text"; then
            record PASS "$file" "blocked (reproduced): $text"
        else
            record FAIL "$file" "defect not reproduced (check=$checked)"
        fi
        ;;
    *)
        record FAIL "$file" "missing or unknown EXPECT header"
        ;;
    esac
done

echo "----"
echo "passed: $pass  failed: $fail  (okkama: $("$okkama" --version), engines: $engines)" | tee -a "$summary"
[ $fail -eq 0 ]
