#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
requirements="$project_dir/decky/backend/requirements.txt"
target="$project_dir/decky/py_modules"
staging="$(mktemp -d -t decky-my-rig-python-deps-XXXXXX)"
trap 'rm -rf -- "$staging"' EXIT

python3 -m pip --version >/dev/null 2>&1 || {
  echo "Python pip is required to refresh bundled Decky dependencies. Use the dev container or install python3-pip." >&2
  exit 1
}

version_of() {
  sed -n "s/^$1==\\([^[:space:]]*\\).*/\\1/p" "$requirements"
}

cryptography_version="$(version_of cryptography)"
cffi_version="$(version_of cffi)"
pycparser_version="$(version_of pycparser)"
spake2_version="$(version_of spake2)"

python3 -m pip install --target "$staging" --no-deps --only-binary=:all: --require-hashes \
  --platform manylinux_2_34_x86_64 --platform manylinux2014_x86_64 \
  --python-version 3.11 --implementation cp --abi cp311 \
  --requirement "$requirements"

rm -rf "$target/cryptography" "$target"/cryptography-*.dist-info \
  "$target/cffi" "$target"/cffi-*.dist-info "$target"/_cffi_backend.cpython-*.so \
  "$target/pycparser" "$target"/pycparser-*.dist-info \
  "$target/spake2" "$target"/spake2-*.dist-info
cp -a "$staging/." "$target/"

echo "Bundled Decky CPython 3.11 x86-64 backend dependencies in $target"
