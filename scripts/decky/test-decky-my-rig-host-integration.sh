#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d)"
host_pid=""
cleanup() {
  if [[ -n "$host_pid" ]]; then kill "$host_pid" 2>/dev/null || true; fi
  rm -rf "$test_dir"
}
trap cleanup EXIT

printf 'port = 47991\n' > "$test_dir/DeckyMyRigHost.toml"
host_executable="${DECKY_MY_RIG_HOST_EXECUTABLE:-$project_dir/host/target/debug/decky-my-rig-host}"
[[ -x "$host_executable" ]] || { echo "Missing host test executable: $host_executable" >&2; exit 1; }
"$host_executable" \
  --dev --mock-shutdown --ephemeral-port --config "$test_dir/DeckyMyRigHost.toml" \
  --pairing-code-value 739104 > "$test_dir/host.log" 2>&1 &
host_pid="$!"

port=""
for _ in {1..50}; do
  port="$(sed -n 's/^DECKY_MY_RIG_LISTEN_PORT=//p' "$test_dir/host.log" | tail -1)"
  if [[ -n "$port" ]] && bash -c "exec 3<>/dev/tcp/127.0.0.1/$port" 2>/dev/null; then break; fi
  if ! kill -0 "$host_pid" 2>/dev/null; then cat "$test_dir/host.log" >&2; exit 1; fi
  sleep 0.1
done
[[ -n "$port" ]] || { cat "$test_dir/host.log" >&2; exit 1; }

PYTHONPATH="$project_dir/decky/py_modules" python3 -B \
  "$project_dir/decky/tests/integration/production_host_client.py" --code 739104 --port "$port"
grep -q "Mock mode enabled" "$test_dir/host.log"
