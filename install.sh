#!/usr/bin/env bash
#
# Установка Okkama (Rust-реализация) из этого репозитория.
#
#     ./install.sh                     # сборка и установка в ~/.local
#     ./install.sh --prefix /opt/okkama
#     ./install.sh --no-build          # установить уже собранное
#     ./install.sh --uninstall         # удалить установку из префикса
#
# Раскладка установки (как у обычного языка):
#
#     <prefix>/bin/okkama                                  компилятор, LSP, fmt, lint
#     <prefix>/bin/sparrow                                 VM байткода Sparrow (.spb)
#     <prefix>/bin/blade                                   менеджер проектов (`blade new`)
#     <prefix>/lib/okkama/std/                             стандартная библиотека
#     <prefix>/lib/okkama/native/<os>-<arch>/libokkama_native.*   хостовая криптография
#     <prefix>/lib/okkama/icu-76.1/lib/                    ICU для locale/collation (если собрана)
#     <prefix>/lib/okkama/uninstall.sh
#
# `okkama` находит std, native и ICU относительно себя: переменные окружения не
# нужны (OKKAMA_STDLIB_DIR и др. по-прежнему переопределяют). Java-рантайм
# (`okkama.rt`) встроен в сам `okkama`; для `okkama run` нужен `java`
# (JDK 22+ — для crypto, SQLite, терминала и других FFM natives).
#
# Старые ссылки `okkama` на Perl-версию (`okkama-pl`) в PATH заменяются;
# ссылки, которые может снять только root, печатаются командой `sudo rm`.

set -euo pipefail

ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BUILD=1
UNINSTALL=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix) PREFIX="$2"; shift 2 ;;
        --prefix=*) PREFIX="${1#--prefix=}"; shift ;;
        --no-build) BUILD=0; shift ;;
        --uninstall) UNINSTALL=1; shift ;;
        -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "install.sh: unknown option $1" >&2; exit 2 ;;
    esac
done

PREFIX="${PREFIX%/}"
LIB="$PREFIX/lib/okkama"
BIN="$PREFIX/bin"

say() { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }

if [[ $UNINSTALL -eq 1 ]]; then
    if [[ -x "$LIB/uninstall.sh" ]]; then
        exec "$LIB/uninstall.sh"
    fi
    echo "install.sh: no Okkama installation in $PREFIX" >&2
    exit 1
fi

case "$(uname -s)" in
    Darwin) OS=macos; NATIVE=libokkama_native.dylib ;;
    Linux) OS=linux; NATIVE=libokkama_native.so ;;
    *) echo "install.sh: unsupported system $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
    arm64|aarch64) ARCH=aarch64 ;;
    x86_64|amd64) ARCH=x86_64 ;;
    *) ARCH="$(uname -m)" ;;
esac
PLATFORM="$OS-$ARCH"
TARGET="$ROOT/okkama-rs/target/release"

if [[ $BUILD -eq 1 ]]; then
    say "building (make build)"
    make -C "$ROOT" build
fi

for built in "$TARGET/okkama" "$TARGET/sparrow" "$TARGET/blade" "$TARGET/$NATIVE"; do
    if [[ ! -f "$built" ]]; then
        echo "install.sh: $built is missing; run without --no-build" >&2
        exit 1
    fi
done

say "installing into $PREFIX"
mkdir -p "$BIN" "$LIB/native/$PLATFORM"

# std — копия текущей версии репозитория (лишние старые файлы удаляются).
rsync -a --delete "$ROOT/std/" "$LIB/std/"
# Хостовая криптография (встраивается в JAR при `okkama build/run`).
cp "$TARGET/$NATIVE" "$LIB/native/$PLATFORM/$NATIVE.tmp"
mv -f "$LIB/native/$PLATFORM/$NATIVE.tmp" "$LIB/native/$PLATFORM/$NATIVE"
# ICU 76.1 для locale/collation, если собрана (tools/build-icu-collation.sh).
ICU="$ROOT/.runtime/icu-76.1/lib"
if [[ -d "$ICU" ]]; then
    mkdir -p "$LIB/icu-76.1"
    rsync -a --delete "$ICU/" "$LIB/icu-76.1/lib/"
else
    warn "ICU 76.1 is not built: locale/collation will report unsupported (tools/build-icu-collation.sh)"
fi

# Бинарник: через временный файл и rename — замена работающего файла на месте
# ломает подпись кода на macOS. Ссылка на Perl-версию заменяется файлом.
for tool in okkama sparrow blade; do
    cp "$TARGET/$tool" "$BIN/.$tool.tmp"
    chmod 755 "$BIN/.$tool.tmp"
    rm -f "$BIN/$tool"
    mv -f "$BIN/.$tool.tmp" "$BIN/$tool"
done

VERSION="$("$BIN/okkama" --version | awk '{print $2}')"
printf '%s\n' "$VERSION" > "$LIB/VERSION"

cat > "$LIB/uninstall.sh" <<EOF
#!/usr/bin/env bash
# Удаляет Okkama $VERSION из $PREFIX (создан install.sh).
set -euo pipefail
rm -f "$BIN/okkama" "$BIN/sparrow" "$BIN/blade"
rm -rf "$LIB"
echo "Okkama removed from $PREFIX"
EOF
chmod 755 "$LIB/uninstall.sh"

# Ссылки на Perl-версию в PATH.
IFS=: read -r -a DIRS <<< "$PATH"
declare -a ROOT_ONLY=()
for dir in "${DIRS[@]}"; do
    candidate="$dir/okkama"
    [[ "$candidate" == "$BIN/okkama" ]] && continue
    if [[ -L "$candidate" ]] && readlink "$candidate" | grep -q "okkama-pl"; then
        if rm -f "$candidate" 2>/dev/null; then
            say "removed Perl link $candidate"
        else
            ROOT_ONLY+=("$candidate")
        fi
    fi
done
if [[ -L /usr/local/lib/okkama/std ]] && readlink /usr/local/lib/okkama/std | grep -q "$ROOT"; then
    if ! rm -f /usr/local/lib/okkama/std 2>/dev/null; then
        ROOT_ONLY+=("/usr/local/lib/okkama/std")
    fi
fi

say "installed Okkama $VERSION"
"$BIN/okkama" env | sed 's/^/    /'

if [[ ${#ROOT_ONLY[@]} -gt 0 ]]; then
    warn "old links to the Perl version need root to remove:"
    echo "    sudo rm ${ROOT_ONLY[*]}" >&2
fi
case ":$PATH:" in
    *":$BIN:"*) ;;
    *) warn "$BIN is not in PATH; add it: export PATH=\"$BIN:\$PATH\"" ;;
esac
resolved="$(command -v okkama || true)"
if [[ -n "$resolved" && "$resolved" != "$BIN/okkama" ]]; then
    warn "\`okkama\` in PATH is $resolved, not $BIN/okkama"
fi
if ! command -v java >/dev/null 2>&1; then
    warn "java is not installed: \`okkama run --engine jvm\` needs a JDK (22+ for FFM natives)"
fi
