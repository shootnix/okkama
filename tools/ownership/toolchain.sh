#!/usr/bin/env bash
# Inside okkama-dev only. Fresh release/fat-LTO installation, micro_toolchain.sh layout.
# /src: read-only checkout; /work: disposable copy; /tc: disposable installation.
set -euo pipefail
[[ -f /.dockerenv || -f /run/.containerenv ]] || {
    echo 'toolchain.sh must run inside a container' >&2; exit 2;
}
ln -sfn /usr/local/cargo "$HOME/.cargo"
ln -sfn /usr/local/rustup "$HOME/.rustup"
mkdir -p /work /tc
rsync -a --delete --exclude /okkama-rs/target --exclude /conformance/adapters/rust/.target \
    --exclude /conformance/reports/runs --exclude /tools/ownership/results \
    --exclude /.oracle-ref --exclude .git /src/ /work/
cd /work/okkama-rs
# A fresh target prevents stale worktree artifacts; the registry alone is shared.
CARGO_TARGET_DIR=/work/okkama-rs/target CARGO_NET_OFFLINE=true \
    timeout --kill-after=10s "${ORACLE_BUILD_TIMEOUT:-1800}" \
    cargo build -q --release -p okkama-cli -p sparrow-cli
mkdir -p "/tc/bin" "/tc/lib/okkama"
cp target/release/okkama target/release/sparrow /tc/bin/
rsync -a --delete /work/std/ /tc/lib/okkama/std/
