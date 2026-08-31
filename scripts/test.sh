#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
npm --prefix "$project_dir/decky" test
cargo test --manifest-path "$project_dir/host/Cargo.toml" --all-targets
cargo test --manifest-path "$project_dir/tools/decky-power-test/Cargo.toml" --all-targets
echo "Portable automated tests: PASS"
