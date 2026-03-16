#!/usr/bin/env bash
# Build header-gen as a self-contained Linux x86_64 binary.
#
# Self-contained means: libclang is statically linked into the binary.
# No Docker, no clang, no libclang required on the deployment machine.
#
# Prerequisites (one-time, on the BUILD machine):
#   Linux:
#     sudo apt-get install -y llvm-dev libclang-dev clang
#   macOS (Homebrew):
#     brew install llvm
#     export LLVM_SYS_160_PREFIX="$(brew --prefix llvm)"  # adjust version number
#
# Usage:
#   ./scripts/build-header-gen.sh
#
# Output:
#   target/release/header-gen   (~80-100 MB, no runtime deps)

set -euo pipefail

BINARY="target/release/header-gen"

echo "Building header-gen (statically linked LLVM)..."
LLVM_LINK_STATIC=1 cargo build --release -p header-gen

echo ""
echo "Binary: ${BINARY}"
echo "Size:   $(du -sh "${BINARY}" | cut -f1)"
echo ""
echo "Verify no libclang runtime dependency (Linux):"
echo "  ldd ${BINARY}   # libclang should NOT appear"
echo ""
echo "Transfer to target machine:"
echo "  scp ${BINARY} <host>:/path/to/header-gen"
