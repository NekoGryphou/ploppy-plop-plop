#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
if ! command -v docker >/dev/null || ! docker compose version >/dev/null 2>&1; then
  if [[ -n "${TOXIPROXY_SERVER:-}" ]] || command -v toxiproxy-server >/dev/null; then
    exec "$project_dir/scripts/network/test-toxiproxy-standalone.sh"
  fi
  if [[ "${REQUIRE_DOCKER:-0}" == "1" ]]; then echo "Docker Compose is required for Toxiproxy E2E." >&2; exit 1; fi
  echo "Toxiproxy E2E: NOT EXECUTED — Docker Compose and standalone Toxiproxy unavailable"
  exit 0
fi
cleanup() { docker compose -f "$project_dir/compose.e2e.yml" down --volumes --remove-orphans; }
trap cleanup EXIT
docker compose -f "$project_dir/compose.e2e.yml" up --build --wait --detach
DECKY_MY_RIG_TOXIPROXY=1 PYTHONPATH="$project_dir/decky/py_modules" python3 -B -m unittest \
  "$project_dir/decky/tests/e2e/test_toxiproxy.py" -v
