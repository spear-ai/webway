#!/usr/bin/env bash
# Build spear-gen as a static Linux musl binary for airgapped deployment.
#
# Prerequisites (one-time):
#   cargo install cargo-zigbuild
#   pip install ziglang        # or: brew install zig
#   rustup target add x86_64-unknown-linux-musl
#
# Usage:
#   ./scripts/build-gen.sh
#
# Output:
#   target/x86_64-unknown-linux-musl/release/spear-gen

set -euo pipefail

TARGET="x86_64-unknown-linux-musl"
BINARY="target/${TARGET}/release/spear-gen"

echo "Building spear-gen for ${TARGET}..."
cargo zigbuild --target "${TARGET}" --release -p spear-gen

echo ""
echo "Binary: ${BINARY}"
echo "Size:   $(du -sh "${BINARY}" | cut -f1)"
echo ""
echo "Transfer to classified side:"
echo "  scp ${BINARY} <host>:/path/to/spear-gen"
echo ""
echo "Verify static linking on target (Linux):"
echo "  file spear-gen"
echo "  ldd spear-gen   # should say: not a dynamic executable"
