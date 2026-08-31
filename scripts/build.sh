#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
npm --prefix "$project_dir/decky" run build
cargo build --manifest-path "$project_dir/host/Cargo.toml" --all-targets
cargo build --manifest-path "$project_dir/tools/decky-my-rig-test/Cargo.toml" --all-targets
"$project_dir/scripts/decky/build-plugin.sh"
echo "Portable WSL/Linux build: PASS"
echo "Windows host, WinUI, and installer: NOT EXECUTED — REQUIRES WINDOWS VALIDATION"
