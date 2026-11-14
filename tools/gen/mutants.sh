#!/usr/bin/env bash
# Self-test of the program generator (H-18): one seeded defect per engine in a
# scratch copy of the tree, built, then okgen.py must find a disagreement and
# minimize it. Runs many programs: use a container (`make gate`'s image).
#
#     tools/gen/mutants.sh [SEEDS]
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SEEDS="${1:-30}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# name | file | text | defect
MUTANTS=(
    "sparrow|okkama-rs/crates/sparrow-core/src/runtime.rs|            &*l * &*r|            &*l * &*r + 1"
    "jvm|okkama-rs/crates/okkama-compiler/src/codegen/jvm/function.rs|IntBinaryOp::Sub => Some(\"subtract\"),|IntBinaryOp::Sub => Some(\"add\"),"
)

failed=0
for mutant in "${MUTANTS[@]}"; do
    IFS='|' read -r name file text defect <<<"$mutant"
    echo "==> defect in $name: $file"
    rm -rf "$WORK/tree"
    rsync -a --exclude /okkama-rs/target --exclude /okkama-rs/fuzz --exclude .git --exclude /conformance/reports "$ROOT/" "$WORK/tree/"
    python3 - "$WORK/tree/$file" "$text" "$defect" <<'PY'
import sys
path, text, defect = sys.argv[1:]
source = open(path).read()
assert source.count(text) == 1, f"mutation point not unique in {path}"
open(path, "w").write(source.replace(text, defect))
PY
    (cd "$WORK/tree/okkama-rs" && CARGO_TARGET_DIR="$ROOT/okkama-rs/target/mutants" cargo build --release -q -p okkama-cli) || {
        echo "!! build failed"; failed=1; continue; }
    out="$ROOT/conformance/reports/runs/okgen-mutants/$name"
    rm -rf "$out"
    OKKAMA="$ROOT/okkama-rs/target/mutants/release/okkama" python3 "$ROOT/tools/gen/okgen.py" run --seeds "$SEEDS" --out "$out" | tail -3
    if ls "$out"/*/minimized.ok >/dev/null 2>&1; then
        first="$(ls -d "$out"/*/ | head -1)"
        echo "found and minimized: $(grep -c 'out.writeln' "$first/minimized.ok") statement(s) in $first"
    else
        echo "!! the $name defect was not found in $SEEDS seeds"
        failed=1
    fi
done
exit $failed
