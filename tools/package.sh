#!/usr/bin/env bash
#
# Релизный архив Okkama для одной платформы.
#
#     tools/package.sh                                  # платформа этой машины
#     tools/package.sh --target x86_64-apple-darwin     # кросс-сборка cargo
#     tools/package.sh --no-build --out dist --version 0.1.0-nightly.20261029
#     tools/package.sh --installer                      # ещё .pkg (macOS) или .deb и .rpm (Linux)
#
# Версия по умолчанию — из okkama-rs/Cargo.toml. Результат —
# dist/okkama-<версия>-<платформа>.tar.gz (+ .pkg или .deb и .rpm) и .sha256 к
# каждому. `.pkg` ставит в /usr/local, `.deb` и `.rpm` — в /usr; `.rpm`
# собирается, если есть `rpmbuild` (#313). Архив
# раскладывается как установка, поэтому распакованный `bin/okkama` сразу
# находит std и нативную библиотеку относительно себя:
#
#     okkama-<версия>-<платформа>/
#         bin/okkama bin/sparrow bin/blade
#         lib/okkama/std/
#         lib/okkama/native/<os>-<arch>/libokkama_native.{dylib,so}
#         install.sh README.md LICENSE

set -euo pipefail
# macOS: без AppleDouble-файлов `._*` и расширенных атрибутов в tar и pkg.
export COPYFILE_DISABLE=1

ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET=""
OUT="$ROOT/dist"
BUILD=1
INSTALLER=0
VERSION=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target) TARGET="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        --no-build) BUILD=0; shift ;;
        --installer) INSTALLER=1; shift ;;
        --version) VERSION="$2"; shift 2 ;;
        -h|--help) sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "package.sh: unknown option $1" >&2; exit 2 ;;
    esac
done

# Платформа архива: из тройки cargo или с этой машины.
triple="${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$triple" in
    aarch64-apple-darwin) PLATFORM=macos-aarch64; NATIVE=libokkama_native.dylib ;;
    x86_64-apple-darwin) PLATFORM=macos-x86_64; NATIVE=libokkama_native.dylib ;;
    x86_64-unknown-linux-gnu) PLATFORM=linux-x86_64; NATIVE=libokkama_native.so ;;
    aarch64-unknown-linux-gnu) PLATFORM=linux-aarch64; NATIVE=libokkama_native.so ;;
    *) echo "package.sh: unsupported target $triple" >&2; exit 1 ;;
esac

if [[ -n "$TARGET" ]]; then
    RELEASE="$ROOT/okkama-rs/target/$TARGET/release"
    CARGO_TARGET=(--target "$TARGET")
else
    RELEASE="$ROOT/okkama-rs/target/release"
    CARGO_TARGET=()
fi

if [[ $BUILD -eq 1 ]]; then
    (cd "$ROOT/okkama-rs" && cargo build --release --locked "${CARGO_TARGET[@]}" \
        -p okkama-cli -p okkama-native -p sparrow-cli -p blade-cli)
fi

VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/okkama-rs/Cargo.toml" | head -1)}"
NAME="okkama-$VERSION-$PLATFORM"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
DEST="$STAGE/$NAME"

mkdir -p "$DEST/bin" "$DEST/lib/okkama/native/$PLATFORM"
for tool in okkama sparrow blade; do
    install -m 755 "$RELEASE/$tool" "$DEST/bin/$tool"
done
install -m 644 "$RELEASE/$NATIVE" "$DEST/lib/okkama/native/$PLATFORM/$NATIVE"
cp -R "$ROOT/std" "$DEST/lib/okkama/std"
printf '%s\n' "$VERSION" > "$DEST/lib/okkama/VERSION"
# The compiler the binaries were built with (#41).
(cd "$ROOT/okkama-rs" && rustc -Vv) > "$DEST/lib/okkama/TOOLCHAIN"
cp "$ROOT/README.md" "$ROOT/LICENSE" "$DEST/"

cat > "$DEST/install.sh" <<'EOF'
#!/usr/bin/env bash
# Копирует Okkama из этого архива в префикс (по умолчанию ~/.local):
#     ./install.sh                  # ~/.local/bin, ~/.local/lib/okkama
#     ./install.sh --prefix /opt/okkama
set -euo pipefail
HERE="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="$HOME/.local"
case "${1:-}" in
    --prefix) PREFIX="${2:?--prefix needs a directory}" ;;
    --prefix=*) PREFIX="${1#--prefix=}" ;;
    "") ;;
    *) echo "install.sh: unknown option $1" >&2; exit 2 ;;
esac
PREFIX="${PREFIX%/}"
mkdir -p "$PREFIX/bin" "$PREFIX/lib"
rm -rf "$PREFIX/lib/okkama"
cp -R "$HERE/lib/okkama" "$PREFIX/lib/okkama"
for tool in okkama sparrow blade; do
    # Через временный файл: замена работающего бинарника ломает подпись на macOS.
    cp "$HERE/bin/$tool" "$PREFIX/bin/.$tool.tmp"
    mv -f "$PREFIX/bin/.$tool.tmp" "$PREFIX/bin/$tool"
done
cat > "$PREFIX/lib/okkama/uninstall.sh" <<UNINSTALL
#!/usr/bin/env bash
rm -f "$PREFIX/bin/okkama" "$PREFIX/bin/sparrow" "$PREFIX/bin/blade"
rm -rf "$PREFIX/lib/okkama"
echo "Okkama removed from $PREFIX"
UNINSTALL
chmod 755 "$PREFIX/lib/okkama/uninstall.sh"
echo "Okkama $(cat "$HERE/lib/okkama/VERSION") installed into $PREFIX"
case ":$PATH:" in
    *":$PREFIX/bin:"*) ;;
    *) echo "add it to PATH: export PATH=\"$PREFIX/bin:\$PATH\"" ;;
esac
EOF
chmod 755 "$DEST/install.sh"

if command -v xattr >/dev/null 2>&1; then
    xattr -cr "$STAGE"
fi

mkdir -p "$OUT"
ARCHIVE="$OUT/$NAME.tar.gz"
tar -C "$STAGE" -czf "$ARCHIVE" "$NAME"
echo "$ARCHIVE"

# Установщик системы: раскладка установки, но в системном префиксе.
# `okkama` находит std и native относительно себя (<prefix>/lib/okkama).
if [[ $INSTALLER -eq 1 ]]; then
    case "$PLATFORM" in
        macos-*)
            ROOTFS="$STAGE/pkgroot"
            mkdir -p "$ROOTFS/usr/local"
            cp -R "$DEST/bin" "$DEST/lib" "$ROOTFS/usr/local/"
            # Версия pkg — только цифры и точки.
            PKG_VERSION="$(printf '%s' "$VERSION" | sed 's/[^0-9.].*//')"
            pkgbuild --quiet --root "$ROOTFS" --identifier dev.okkama.okkama \
                --version "${PKG_VERSION:-0}" --install-location / "$OUT/$NAME.pkg"
            echo "$OUT/$NAME.pkg"
            ;;
        linux-*)
            case "$PLATFORM" in
                linux-x86_64) DEB_ARCH=amd64 ;;
                linux-aarch64) DEB_ARCH=arm64 ;;
            esac
            ROOTFS="$STAGE/debroot"
            mkdir -p "$ROOTFS/usr" "$ROOTFS/DEBIAN"
            cp -R "$DEST/bin" "$DEST/lib" "$ROOTFS/usr/"
            mkdir -p "$ROOTFS/usr/share/doc/okkama"
            cp "$ROOT/LICENSE" "$ROOTFS/usr/share/doc/okkama/copyright"
            # Debian: `-` в версии — разделитель ревизии, пре-релиз — через `~`.
            DEB_VERSION="$(printf '%s' "$VERSION" | sed 's/-/~/')"
            cat > "$ROOTFS/DEBIAN/control" <<CONTROL
Package: okkama
Version: $DEB_VERSION
Architecture: $DEB_ARCH
Maintainer: Alexander Ponomarev <shootnix@users.noreply.github.com>
Section: devel
Priority: optional
Depends: libc6 (>= 2.17)
Suggests: libsqlite3-0 (>= 3.41), openjdk-21-jdk-headless
Homepage: https://github.com/shootnix/okkama
Description: Okkama programming language
 Compiler and runner for Okkama (IR interpreter, Sparrow VM with JIT,
 JVM backend), the standard library, the language server, the formatter
 and the blade project tool.
CONTROL
            # xz, not the zstd Ubuntu's dpkg-deb picks by default: dpkg before
            # Debian 12 cannot read zstd members (#485).
            dpkg-deb -Zxz --root-owner-group --build "$ROOTFS" "$OUT/$NAME.deb" >/dev/null
            echo "$OUT/$NAME.deb"
            # RPM from the same tree (#313). Release binaries are built against
            # glibc 2.17 (manylinux2014, #485): every glibc distribution since
            # 2014 installs them — CentOS 7, Amazon Linux 2, RHEL 8 and newer.
            if command -v rpmbuild >/dev/null 2>&1; then
                case "$PLATFORM" in
                    linux-x86_64) RPM_ARCH=x86_64 ;;
                    linux-aarch64) RPM_ARCH=aarch64 ;;
                esac
                # RPM: no `-` in a version; `~` sorts a pre-release first.
                RPM_VERSION="$(printf '%s' "$VERSION" | sed 's/-/~/')"
                TOPDIR="$STAGE/rpmbuild"
                mkdir -p "$TOPDIR/SPECS"
                cat > "$TOPDIR/SPECS/okkama.spec" <<SPEC
Name: okkama
Version: $RPM_VERSION
Release: 1
Summary: Okkama programming language
License: GPL-3.0-only
URL: https://github.com/shootnix/okkama
BuildArch: $RPM_ARCH
Requires: glibc >= 2.17
Recommends: sqlite-libs
Suggests: java-21-openjdk-headless
AutoReqProv: no
%global debug_package %{nil}
%global __os_install_post %{nil}
%global _build_id_links none

%description
Compiler and runner for Okkama (IR interpreter, Sparrow VM with JIT,
JVM backend), the standard library, the language server, the formatter
and the blade project tool.

%install
mkdir -p %{buildroot}
cp -a $ROOTFS/usr %{buildroot}/

%files
/usr/bin/okkama
/usr/bin/sparrow
/usr/bin/blade
/usr/lib/okkama
%doc /usr/share/doc/okkama/copyright
SPEC
                rpmbuild --quiet --define "_topdir $TOPDIR" -bb "$TOPDIR/SPECS/okkama.spec"
                cp "$TOPDIR/RPMS/$RPM_ARCH/okkama-$RPM_VERSION-1.$RPM_ARCH.rpm" "$OUT/$NAME.rpm"
                echo "$OUT/$NAME.rpm"
            fi
            ;;
    esac
fi

(cd "$OUT" && for file in "$NAME".tar.gz "$NAME".pkg "$NAME".deb "$NAME".rpm; do
    [[ -f "$file" ]] && shasum -a 256 "$file" > "$file.sha256"
done; true)
