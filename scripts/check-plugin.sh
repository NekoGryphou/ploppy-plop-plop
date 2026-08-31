#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
npm --prefix "$project_dir/decky" run lint
npm --prefix "$project_dir/decky" run test:frontend
npm --prefix "$project_dir/decky" run test:backend
"$project_dir/scripts/decky/build-plugin.sh"
echo "Plugin Linux validation: PASS"
