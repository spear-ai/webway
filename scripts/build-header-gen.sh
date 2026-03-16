#!/usr/bin/env bash
# Build header-gen and bundle libclang.so + libLLVM.so alongside it.
#
# libclang-XX.so.XX depends on libLLVM.so.XX, so both must travel together.
# RPATH is patched to $ORIGIN on the binary and on libclang so each finds
# its dependency in the same directory — no system LLVM install needed on
# the deployment machine.
#
# Note: Ubuntu does not ship the monolithic libclang.a required for fully-static
# linking. Bundling the shared libraries is the practical alternative.
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
#   target/release/libLLVM*.so*      (bundled libLLVM)

set -euo pipefail

BINARY="target/release/header-gen"
RELDIR="target/release"

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

# Bundle libclang.so.
LIBCLANG_SO=$(ls "${LIBDIR}"/libclang-*.so.* 2>/dev/null | head -1 || echo "")
if [ -z "${LIBCLANG_SO}" ]; then
    LIBCLANG_SO="${LIBDIR}/libclang.so"
fi
echo "Bundling libclang: ${LIBCLANG_SO}"
cp "${LIBCLANG_SO}" "${RELDIR}/"

# Bundle libLLVM.so (libclang depends on it).
LIBLLVM_SO=$(ls "${LIBDIR}"/libLLVM.so.* 2>/dev/null | head -1 || true)
if [ -z "${LIBLLVM_SO}" ]; then
    LIBLLVM_SO=$(ls "${LIBDIR}"/libLLVM-*.so 2>/dev/null | head -1 || true)
fi
if [ -n "${LIBLLVM_SO}" ]; then
    echo "Bundling libLLVM: ${LIBLLVM_SO}"
    cp "${LIBLLVM_SO}" "${RELDIR}/"
    LIBLLVM_BASENAME=$(basename "${LIBLLVM_SO}")
else
    echo "WARNING: libLLVM.so not found in ${LIBDIR} — skipping"
    LIBLLVM_BASENAME=""
fi

# Patch RPATH on the binary and on libclang so each finds its deps next to itself.
patchelf --set-rpath '$ORIGIN' "${BINARY}"
patchelf --set-rpath '$ORIGIN' "${RELDIR}/$(basename "${LIBCLANG_SO}")"

echo ""
echo "Binary:   ${BINARY}"
echo "Library:  ${RELDIR}/$(basename "${LIBCLANG_SO}")"
[ -n "${LIBLLVM_BASENAME}" ] && echo "Library:  ${RELDIR}/${LIBLLVM_BASENAME}"
echo ""
echo "Transfer all files to the target machine (same directory):"
SCP_FILES="${BINARY} ${RELDIR}/$(basename "${LIBCLANG_SO}")"
[ -n "${LIBLLVM_BASENAME}" ] && SCP_FILES="${SCP_FILES} ${RELDIR}/${LIBLLVM_BASENAME}"
echo "  scp ${SCP_FILES} <host>:/path/to/"
