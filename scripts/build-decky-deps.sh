#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
requirements="$project_dir/decky/backend/requirements.txt"
target="$project_dir/decky/py_modules"

version_of() {
  sed -n "s/^$1==//p" "$requirements"
}

cryptography_version="$(version_of cryptography)"
cffi_version="$(version_of cffi)"
pycparser_version="$(version_of pycparser)"
spake2_version="$(version_of spake2)"

rm -rf "$target/cryptography" "$target/cryptography-$cryptography_version.dist-info" \
  "$target/cffi" "$target/cffi-$cffi_version.dist-info" "$target/_cffi_backend.cpython-"*.so \
  "$target/pycparser" "$target/pycparser-$pycparser_version.dist-info" \
  "$target/spake2" "$target/spake2-$spake2_version.dist-info"

python3 -m pip install --target "$target" --no-deps --only-binary=:all: \
  --platform manylinux_2_34_x86_64 --python-version 3.11 --implementation cp --abi cp311 \
  "cryptography==$cryptography_version"
python3 -m pip install --target "$target" --no-deps --only-binary=:all: \
  --platform manylinux2014_x86_64 --python-version 3.11 --implementation cp --abi cp311 \
  "cffi==$cffi_version"
python3 -m pip install --target "$target" --no-deps --only-binary=:all: \
  --platform manylinux2014_x86_64 --python-version 3.11 --implementation cp --abi cp311 \
  "pycparser==$pycparser_version" "spake2==$spake2_version"

echo "Bundled Decky CPython 3.11 x86-64 backend dependencies in $target"
