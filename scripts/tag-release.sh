#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

VERSION=$(node -p "require('$REPO_ROOT/package.json').version")

echo "Tagging release v$VERSION"

git tag "v$VERSION"
git push origin "v$VERSION"
