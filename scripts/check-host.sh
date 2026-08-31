#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo fmt --manifest-path "$project_dir/host/Cargo.toml" --all -- --check
cargo clippy --manifest-path "$project_dir/host/Cargo.toml" --all-targets -- -D warnings
cargo check --manifest-path "$project_dir/host/Cargo.toml" --target x86_64-pc-windows-msvc --all-targets
cargo test --manifest-path "$project_dir/host/Cargo.toml" --all-targets
cargo fmt --manifest-path "$project_dir/tools/decky-my-rig-test/Cargo.toml" --all -- --check
cargo clippy --manifest-path "$project_dir/tools/decky-my-rig-test/Cargo.toml" --all-targets -- -D warnings
cargo test --manifest-path "$project_dir/tools/decky-my-rig-test/Cargo.toml" --all-targets
PYTHONPATH="$project_dir/decky/py_modules" python3 -B -m unittest discover \
  -s "$project_dir/decky/tests/e2e" -p 'test_*.py' -v
"$project_dir/scripts/protocol/test-e2e.sh"
"$project_dir/scripts/decky/test-decky-my-rig-host-integration.sh"
echo "Host cross-platform validation: PASS"
