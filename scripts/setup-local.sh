#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v powershell.exe >/dev/null || { echo "Run this script in WSL with Windows interoperability enabled." >&2; exit 1; }
windows_script="$(wslpath -w "$project_dir/scripts/setup-windows-build.ps1")"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$windows_script"
