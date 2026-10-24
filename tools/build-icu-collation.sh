#!/bin/sh
# Reproducible optional collation backend. No system installation or host ICU.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
prefix=${OKKAMA_ICU_PREFIX:-"$root/.runtime/icu-76.1"}
work=$(mktemp -d "${TMPDIR:-/tmp}/okkama-icu76.XXXXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
url=https://github.com/unicode-org/icu/releases/download/release-76-1/icu4c-76_1-src.tgz
curl --fail --location --retry 2 "$url" -o "$work/icu.tgz"
printf '%s  %s\n' dfacb46bfe4747410472ce3e1144bf28a102feeaa4e3875bac9b4c6cf30f4f3e "$work/icu.tgz" | shasum -a 256 -c -
tar -xzf "$work/icu.tgz" -C "$work"
mkdir "$work/build"
cd "$work/build"
case $(uname -s) in
    Darwin) platform=MacOSX ;;
    Linux) platform=Linux ;;
    *) printf '%s\n' 'Pinned ICU build supports Darwin and Linux.' >&2; exit 1 ;;
esac
../icu/source/runConfigureICU "$platform" --prefix="$prefix" --enable-rpath --disable-tests --disable-samples
make -j "${OKKAMA_BUILD_JOBS:-4}"
make install
printf 'Pinned ICU installed in %s\n' "$prefix"
