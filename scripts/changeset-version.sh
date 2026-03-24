#!/usr/bin/env bash
set -euo pipefail

npx changeset version
bash "$(dirname "${BASH_SOURCE[0]}")/sync-cargo-versions.sh"
