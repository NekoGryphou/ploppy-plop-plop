#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
package_dir="$project_dir/out/RemotePCPower"
archive="$project_dir/out/RemotePCPower.zip"

if [[ ! -d "$project_dir/py_modules/spake2" || ! -d "$project_dir/py_modules/cryptography" ]]; then
  echo "Backend dependencies are missing. Run: npm run backend:deps" >&2
  exit 1
fi

npm run build
rm -rf "$package_dir"
rm -f "$archive"
mkdir -p "$package_dir/dist"
cp "$project_dir/dist/index.js" "$package_dir/dist/index.js"
cp "$project_dir/main.py" "$project_dir/package.json" "$project_dir/plugin.json" "$project_dir/README.md" "$project_dir/LICENSE" "$package_dir/"
cp -R "$project_dir/py_modules" "$package_dir/py_modules"
find "$package_dir/py_modules" -type d -name __pycache__ -prune -exec rm -rf {} +
cd "$project_dir/out"
python3 -m zipfile -c "$(basename "$archive")" "$(basename "$package_dir")"
echo "Created $archive"
