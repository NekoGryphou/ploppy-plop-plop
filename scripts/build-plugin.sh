#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
staging_dir="$(mktemp -d)"
trap 'rm -rf "$staging_dir"' EXIT
package_dir="$staging_dir/RemotePCPower"
archive="$project_dir/out/RemotePCPower.zip"
decky_dir="$project_dir/decky"

if [[ ! -d "$decky_dir/py_modules/spake2" || ! -d "$decky_dir/py_modules/cryptography" ]]; then
  echo "Backend dependencies are missing. Run: npm run backend:deps" >&2
  exit 1
fi

npm --prefix "$decky_dir" run build
rm -f "$archive"
mkdir -p "$package_dir/dist"
cp "$decky_dir/dist/index.js" "$package_dir/dist/index.js"
cp "$decky_dir/main.py" "$decky_dir/package.json" "$decky_dir/plugin.json" "$project_dir/README.md" "$project_dir/LICENSE" "$package_dir/"
cp -R "$decky_dir/py_modules" "$package_dir/py_modules"
find "$package_dir/py_modules" -type d -name __pycache__ -prune -exec rm -rf {} +
cd "$staging_dir"
python3 -m zipfile -c "$archive" "$(basename "$package_dir")"
echo "Created $archive"
