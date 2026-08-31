#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
toxiproxy_server="${TOXIPROXY_SERVER:-}"
if [[ -z "$toxiproxy_server" ]]; then toxiproxy_server="$(command -v toxiproxy-server || true)"; fi
if [[ -z "$toxiproxy_server" || ! -x "$toxiproxy_server" ]]; then
  echo "Set TOXIPROXY_SERVER to an executable official Toxiproxy server binary." >&2
  exit 2
fi

temporary_dir="$(mktemp -d -t decky-my-rig-toxiproxy-XXXXXX)"
host_pid=""
proxy_pid=""
cleanup() {
  if [[ -n "$proxy_pid" ]]; then kill "$proxy_pid" 2>/dev/null || true; wait "$proxy_pid" 2>/dev/null || true; fi
  if [[ -n "$host_pid" ]]; then kill "$host_pid" 2>/dev/null || true; wait "$host_pid" 2>/dev/null || true; fi
  rm -r -- "$temporary_dir"
}
trap cleanup EXIT

printf 'port = 58201\n' > "$temporary_dir/DeckyMyRigHost.toml"
printf '[{"name":"host-slow","listen":"127.0.0.1:58200","upstream":"127.0.0.1:58201","enabled":true}]\n' > "$temporary_dir/toxiproxy.json"

cargo build --quiet --manifest-path "$project_dir/host/Cargo.toml"
"$project_dir/host/target/debug/decky-my-rig-host" \
  --dev --mock-shutdown --config "$temporary_dir/DeckyMyRigHost.toml" \
  --pairing-code-value 333333 > "$temporary_dir/host.log" 2>&1 &
host_pid=$!
"$toxiproxy_server" -host 127.0.0.1 -port 58474 \
  -config "$temporary_dir/toxiproxy.json" > "$temporary_dir/toxiproxy.log" 2>&1 &
proxy_pid=$!

for _ in {1..50}; do
  if curl --fail --silent http://127.0.0.1:58474/proxies >/dev/null 2>&1; then break; fi
  if ! kill -0 "$host_pid" 2>/dev/null || ! kill -0 "$proxy_pid" 2>/dev/null; then
    echo "Standalone host or Toxiproxy exited during startup." >&2
    exit 1
  fi
  sleep 0.05
done
curl --fail --silent http://127.0.0.1:58474/proxies >/dev/null

DECKY_MY_RIG_TOXIPROXY=1 PYTHONPATH="$project_dir/decky/py_modules" \
  python3 -W error::ResourceWarning -B -m unittest \
  "$project_dir/decky/tests/e2e/test_toxiproxy.py" -v
echo "Standalone official Toxiproxy network-fault E2E: PASS"
