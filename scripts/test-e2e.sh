#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo build --quiet --manifest-path "$project_dir/host/Cargo.toml"
PYTHONPATH="$project_dir/decky/py_modules" python3 -B -m unittest discover \
  -s "$project_dir/decky/tests/e2e" -p 'test_*.py' -v
"$project_dir/scripts/protocol/test-e2e.sh"
"$project_dir/scripts/decky/test-decky-host-integration.sh"
"$project_dir/scripts/network/test-toxiproxy.sh"
echo "Portable socket E2E: PASS"
echo "Windows/physical lifecycle: NOT EXECUTED — REQUIRES WINDOWS VALIDATION / PHYSICAL HARDWARE"
