#!/bin/sh
# Пакует TZif-файлы tzdb (`tools/data/tzdb-<версия>`) в ресурс рантайма
# (zip без меток времени): его встраивают JVM-рантайм и okkama-runtime-core.
set -eu
root=$(cd "$(dirname "$0")/../../.." && pwd)
version=${1:-2026c}
out="$root/okkama-rs/crates/okkama-compiler/runtime/jvm/src/okkama/rt/tzdb.dat"
rm -f "$out"
(cd "$root/tools/data/tzdb-$version" && zip -q -r -X "$out" .)
echo "packed tzdb $version into $out"
