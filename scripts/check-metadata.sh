#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$project_dir/scripts/decky/generate-python-proto.py" --check
python3 "$project_dir/scripts/check-metadata.py"
echo "Generated protocol and project metadata validation: PASS"
