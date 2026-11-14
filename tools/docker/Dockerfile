# Isolated environment for the release gate, benchmarks and network/worker
# tests (process workers, sockets, PostgreSQL clients must not run on the
# developer's machine):
#
#     docker build -t okkama-dev tools/docker
#     tools/gate/docker.sh                      # the gate in this image
#
# Rust (the base image), Python with psycopg2,
# Perl and Ruby for benchmark ports, gdb for sampling profiles.
# The release toolchain (tools/release-toolchain.txt; docker.sh passes it, #41).
ARG RUST_VERSION=1.97.1
FROM rust:${RUST_VERSION}
# The gate runs clippy (#42); the slim toolchain of the image lacks it.
RUN rustup component add clippy
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      python3 python3-psycopg2 perl ruby gdb rsync \
 && rm -rf /var/lib/apt/lists/*
