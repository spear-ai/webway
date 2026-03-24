#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CARGO_TOMLS=(
    "$REPO_ROOT/crates/spear-gen/Cargo.toml"
    "$REPO_ROOT/crates/spear-lib/Cargo.toml"
    "$REPO_ROOT/crates/spear-gateway/Cargo.toml"
    "$REPO_ROOT/crates/header-gen/Cargo.toml"
)

# Read current version from spear-gen (source of truth).
CURRENT=$(grep '^version' "$REPO_ROOT/crates/spear-gen/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')

# Increment patch.
MAJOR=$(echo "$CURRENT" | cut -d. -f1)
MINOR=$(echo "$CURRENT" | cut -d. -f2)
PATCH=$(echo "$CURRENT" | cut -d. -f3)
NEXT="$MAJOR.$MINOR.$((PATCH + 1))"

echo "Bumping $CURRENT → $NEXT"

for TOML in "${CARGO_TOMLS[@]}"; do
    perl -i -pe "s/^version = \"$CURRENT\"/version = \"$NEXT\"/" "$TOML"
done

cargo update --workspace --manifest-path "$REPO_ROOT/Cargo.toml"

echo "$NEXT"
