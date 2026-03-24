#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CARGO_TOMLS=(
    "$REPO_ROOT/crates/spear-gen/Cargo.toml"
    "$REPO_ROOT/crates/spear-lib/Cargo.toml"
    "$REPO_ROOT/crates/spear-gateway/Cargo.toml"
    "$REPO_ROOT/crates/header-gen/Cargo.toml"
)

# Read current version from each Cargo.toml and the target version from package.json.
TARGET=$(node -p "require('$REPO_ROOT/package.json').version")

echo "Syncing Cargo.toml versions to $TARGET"

for TOML in "${CARGO_TOMLS[@]}"; do
    CURRENT=$(grep '^version' "$TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')
    perl -i -pe "s/^version = \"$CURRENT\"/version = \"$TARGET\"/" "$TOML"
    echo "  $TOML: $CURRENT → $TARGET"
done

cargo update --workspace --manifest-path "$REPO_ROOT/Cargo.toml"

echo "Done."
