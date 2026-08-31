#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
for tool in node npm python3 cargo rustc; do command -v "$tool" >/dev/null || { echo "Missing required tool: $tool" >&2; exit 1; }; done
node -e 'const major=Number(process.versions.node.split(".")[0]); if (major < 22) { throw new Error("Node.js 22 or newer is required") }'
"$project_dir/scripts/check-metadata.sh"
"$project_dir/scripts/check-security.sh"
"$project_dir/scripts/check-plugin.sh"
"$project_dir/scripts/check-host.sh"
"$project_dir/scripts/check-network.sh"
echo "Primary Linux quality gate: PASS"
