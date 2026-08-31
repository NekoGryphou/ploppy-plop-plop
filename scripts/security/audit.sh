#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
requirements="$project_dir/decky/backend/requirements.txt"

command -v cargo-audit >/dev/null 2>&1 || {
  echo "cargo-audit is required. Install it with: cargo install cargo-audit --locked" >&2
  exit 1
}
cargo audit --file "$project_dir/host/Cargo.lock"
cargo audit --file "$project_dir/tools/decky-my-rig-test/Cargo.lock"
npm --prefix "$project_dir/decky" audit --audit-level=low

pip_audit_args=(--requirement "$requirements" --no-deps --disable-pip --strict)
if python3 -c 'import pip_audit' >/dev/null 2>&1; then
  python3 -m pip_audit "${pip_audit_args[@]}"
elif command -v docker >/dev/null 2>&1 && docker version >/dev/null 2>&1; then
  docker run --rm \
    -v "$requirements:/audit/requirements.txt:ro" \
    python:3.14-slim@sha256:cae66f2ef0ec51a9891263eeee7f987dacf0a9879e8aa9353d5606e0530619a5 \
    sh -c 'python -m pip install --quiet pip-audit==2.10.1 && python -m pip_audit --requirement /audit/requirements.txt --no-deps --disable-pip --strict'
else
  echo "pip-audit or a working Docker daemon is required for the Python dependency audit." >&2
  exit 1
fi

echo "Rust, npm, and vendored Python dependency advisories: PASS"
