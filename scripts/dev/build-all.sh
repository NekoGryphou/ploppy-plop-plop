#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
decky_dir="$project_dir/decky"
run_visual=true
if [[ "${1:-}" == "--skip-ui" ]]; then run_visual=false; fi
if [[ $# -gt 1 || ($# -eq 1 && "$1" != "--skip-ui") ]]; then
  echo "Usage: $0 [--skip-ui]" >&2
  exit 2
fi

command -v npm >/dev/null || { echo "Node.js/npm is required: https://nodejs.org/en/download" >&2; exit 1; }
command -v python3 >/dev/null || { echo "Python 3.11 is required: https://www.python.org/downloads/" >&2; exit 1; }
command -v powershell.exe >/dev/null || { echo "Run this script in WSL with Windows interoperability enabled." >&2; exit 1; }

npm --prefix "$decky_dir" ci
"$project_dir/scripts/decky/build-decky-deps.sh"
npm --prefix "$decky_dir" run lint
npm --prefix "$decky_dir" test
"$project_dir/scripts/decky/build-plugin.sh"
if $run_visual; then npm --prefix "$decky_dir" run visual; fi

windows_script="$(wslpath -w "$project_dir/scripts/windows/build-windows.ps1")"
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$windows_script"
echo "All local artifacts are available under $project_dir/out"
