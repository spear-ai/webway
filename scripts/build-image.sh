#!/usr/bin/env bash
# scripts/build-image.sh — build the spear-dev container image for transfer
#
# Run this on an internet-connected machine. The output is a self-contained
# .tar.gz that can be loaded with `podman load` on an airgapped system.
#
# Usage:
#   ./scripts/build-image.sh [image-tag]
#
# Default tag: spear-dev:latest

set -euo pipefail

cd "$(dirname "$0")/.."

TAG="${1:-spear-dev:latest}"
ARCHIVE="spear-dev.tar.gz"

echo "==> Vendoring crate dependencies..."
cargo vendor vendor/

echo "==> Building image: $TAG (linux/amd64)"
docker build --platform linux/amd64 -t "$TAG" .

echo "==> Saving image to $ARCHIVE..."
docker save "$TAG" | gzip > "$ARCHIVE"

SIZE=$(du -sh "$ARCHIVE" | cut -f1)
echo ""
echo "Done. $ARCHIVE ($SIZE)"
echo ""
echo "Transfer to classified side, then:"
echo "  podman load < $ARCHIVE"
echo "  podman run -d --name spear-dev \\"
echo "    -v /path/to/workspace:/workspace \\"
echo "    $TAG"
echo "  podman exec -it spear-dev bash"
echo ""
echo "Inside the container:"
echo "  spear-gen --input /workspace/xsds \\"
echo "            --out-proto /workspace/types.proto \\"
echo "            --out-rust  /workspace/types.rs"
