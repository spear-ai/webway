#!/usr/bin/env bash
# Build header-gen and bundle libclang.so alongside it.
#
# The binary's RPATH is patched to $ORIGIN so it finds libclang.so in the
# same directory as itself — no system libclang installation needed on the
# deployment machine.
#
# Note: Ubuntu does not ship the monolithic libclang.a required for fully-static
# linking. Bundling the shared library is the practical alternative.
#
# Prerequisites (one-time):
#   Linux:
#     sudo apt-get install -y libclang-dev clang patchelf
#   macOS (Homebrew):
#     brew install llvm
#     export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
#
# Usage:
#   ./scripts/build-header-gen.sh
#
# Output:
#   target/release/header-gen        (binary, RPATH=$ORIGIN)
#   target/release/libclang-*.so.*   (bundled libclang)

set -euo pipefail

BINARY="target/release/header-gen"

# Locate the versioned llvm-config (Ubuntu ships llvm-config-18, not llvm-config).
LLVM_CONFIG=$(ls /usr/bin/llvm-config-* 2>/dev/null | sort -V | tail -1 || true)
if [ -z "${LLVM_CONFIG}" ] && command -v llvm-config &>/dev/null; then
    LLVM_CONFIG=llvm-config
fi
if [ -z "${LLVM_CONFIG}" ]; then
    echo "ERROR: llvm-config not found. Install llvm-dev (Linux) or brew install llvm (macOS)."
    exit 1
fi

LIBDIR=$(${LLVM_CONFIG} --libdir)
echo "LLVM libdir: ${LIBDIR}"

LIBCLANG_PATH="${LIBDIR}" cargo build --release -p header-gen

# Bundle libclang.so and patch RPATH.
LIBCLANG_SO=$(ls "${LIBDIR}"/libclang-*.so.* 2>/dev/null | head -1 || echo "")
if [ -z "${LIBCLANG_SO}" ]; then
    LIBCLANG_SO="${LIBDIR}/libclang.so"
fi
echo "Bundling: ${LIBCLANG_SO}"
cp "${LIBCLANG_SO}" "target/release/"
patchelf --set-rpath '$ORIGIN' "${BINARY}"

echo ""
echo "Binary:  ${BINARY}"
echo "Library: target/release/$(basename "${LIBCLANG_SO}")"
echo ""
echo "Transfer both files to the target machine (same directory):"
echo "  scp ${BINARY} target/release/$(basename "${LIBCLANG_SO}") <host>:/path/to/"
