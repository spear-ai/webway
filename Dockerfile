# spear-dev — offline development environment
#
# Build this image on an internet-connected machine using scripts/build-image.sh
# which vendors all crate dependencies first. The resulting image works
# fully offline (no crate registry, no apt).
#
# Classified-side usage:
#   podman load < spear-dev.tar.gz
#   podman run -d --name spear-dev \
#     -v /path/to/classified-workspace:/workspace \
#     spear-dev:latest
#   podman exec -it spear-dev bash
#
# Inside the container:
#   spear-gen --input /workspace/xsds --out-proto /workspace/types.proto --out-rust /workspace/types.rs
#   header-gen --input /workspace/headers --endian little --word-size 32 \
#              --out-rust /workspace/rust --out-proto /workspace/proto --out-mapping /workspace/mapping
#   cp /workspace/types.rs /spear/crates/spear-gateway/src/types.rs
#   # uncomment include!("types.rs") in main.rs, add decode call
#   cargo build --offline --release -p spear-gateway

FROM --platform=linux/amd64 rust:1.85-slim-bookworm

# System build dependencies.
# cmake + libcurl are required to compile rdkafka-sys.
# llvm-dev + libclang-dev + clang are required to compile header-gen.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        cmake \
        libcurl4-openssl-dev \
        pkg-config \
        libssl-dev \
        ca-certificates \
        vim \
        llvm-dev \
        libclang-dev \
        clang \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /spear

# Copy the pre-vendored source tree.
# scripts/build-image.sh runs `cargo vendor vendor/` before `docker build`.
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY vendor vendor

# Configure cargo to use the vendored sources — no network needed.
RUN mkdir -p .cargo && printf '\
[source.crates-io]\n\
replace-with = "vendored-sources"\n\
\n\
[source.vendored-sources]\n\
directory = "vendor"\n\
' > .cargo/config.toml

# Pre-compile the entire workspace so all C extensions (librdkafka, openssl,
# libclang) are already compiled in the image layer. On the classified side,
# only changed Rust files need recompilation.
RUN LLVM_CONFIG=$(ls /usr/bin/llvm-config-* 2>/dev/null | sort -V | tail -1) && \
    LIBCLANG_PATH=$(${LLVM_CONFIG} --libdir) && \
    LIBCLANG_PATH="${LIBCLANG_PATH}" cargo build --offline --release 2>&1

# Expose binaries on PATH for convenience.
RUN ln -s /spear/target/release/spear-gen   /usr/local/bin/spear-gen && \
    ln -s /spear/target/release/header-gen  /usr/local/bin/header-gen

# Default: sleep so the container stays alive for `podman exec`.
CMD ["sleep", "infinity"]
