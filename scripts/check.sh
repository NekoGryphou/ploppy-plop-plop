#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
for tool in node npm python3 cargo rustc; do command -v "$tool" >/dev/null || { echo "Missing required tool: $tool" >&2; exit 1; }; done
node -e 'const major=Number(process.versions.node.split(".")[0]); if (major < 22) { throw new Error("Node.js 22 or newer is required") }'
"$project_dir/scripts/decky/generate-python-proto.py" --check
python3 "$project_dir/scripts/check-metadata.py"
"$project_dir/scripts/security/audit.sh"
npm --prefix "$project_dir/decky" run lint
npm --prefix "$project_dir/decky" run build
cargo fmt --manifest-path "$project_dir/host/Cargo.toml" --all -- --check
cargo clippy --manifest-path "$project_dir/host/Cargo.toml" --all-targets -- -D warnings
cargo check --manifest-path "$project_dir/host/Cargo.toml" --target x86_64-pc-windows-msvc --all-targets
cargo fmt --manifest-path "$project_dir/tools/decky-my-rig-test/Cargo.toml" --all -- --check
cargo clippy --manifest-path "$project_dir/tools/decky-my-rig-test/Cargo.toml" --all-targets -- -D warnings
"$project_dir/scripts/test.sh"
"$project_dir/scripts/test-e2e.sh"
"$project_dir/scripts/build.sh"
echo "Primary portable quality gate: PASS"
