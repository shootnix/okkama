#!/bin/sh
# Release-сборка с оптимизацией по профилю (PGO) в обычный target/release.
#
# 1. Инструментированная сборка okkama и sparrow (`-Cprofile-generate`).
# 2. Обучение: JSON-нагрузка tools/pgo/json.ok, бенчмарки benchmarks/NN-*
#    на Sparrow и IR и inline-тесты модулей std (компилятор, оба движка, std).
# 3. `llvm-profdata merge` и сборка всех бинарников с `-Cprofile-use`.
#
# Нужен llvm-profdata той же версии LLVM, что у rustc:
#     rustup component add llvm-tools
# Профиль не хранится в репозитории: он годится только для того исходного
# кода, с которого снят, поэтому снимается заново при каждой сборке.
set -eu
rs=$(cd "$(dirname "$0")/../.." && pwd)
root=$(dirname "$rs")
profdata=$(find "$(rustc --print sysroot)" -name llvm-profdata -type f 2>/dev/null | head -1)
if [ -z "$profdata" ]; then
    echo "pgo: llvm-profdata not found; run: rustup component add llvm-tools" >&2
    exit 1
fi
work="$rs/target/pgo"
rm -rf "$work/data"
mkdir -p "$work/data"

echo "==> instrumented build"
(cd "$rs" && RUSTFLAGS="-Cprofile-generate=$work/data" CARGO_TARGET_DIR="$work/instrumented" \
    cargo build --release -p okkama-cli -p sparrow-cli)

echo "==> training"
okkama="$work/instrumented/release/okkama"
export OKKAMA_STDLIB_DIR="$root/std"
for engine in sparrow ir; do
    "$okkama" run --engine "$engine" "$rs/tools/pgo/json.ok" > /dev/null
    for main in "$root"/benchmarks/[0-9][0-9]-*/main.ok; do
        "$okkama" run --engine "$engine" "$main" > /dev/null
    done
done
for module in json runtime/json strings/parse \
    url/form url/percent template/html template/compile toml/parse plist/parse csv csv/input \
    encoding/utf encoding/pem numbers/float config/ini xml/node message/header mime/transfer \
    checksum/adler32 digest/blocks compression/read i18n/plural web/request web/reply http/auth; do
    "$okkama" run -t "$root/std/$module.ok" > /dev/null
done

echo "==> merge profiles"
"$profdata" merge -o "$work/merged.profdata" "$work/data"

echo "==> optimized build"
(cd "$rs" && RUSTFLAGS="-Cprofile-use=$work/merged.profdata" \
    cargo build --release -p okkama-cli -p okkama-native -p sparrow-cli -p blade-cli)
echo "pgo: built $rs/target/release with $(ls "$work/data" | wc -l | tr -d ' ') profiles"
