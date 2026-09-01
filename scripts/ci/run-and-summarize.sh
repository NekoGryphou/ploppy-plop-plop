#!/usr/bin/env bash
set -uo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <report title> <command> [arguments...]" >&2
  exit 2
fi

report_title="$1"
shift
report_output="$(mktemp)"
trap 'rm -f -- "$report_output"' EXIT

set +e
"$@" 2>&1 | tee "$report_output"
command_status=${PIPESTATUS[0]}
set -e

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  if [[ $command_status -eq 0 ]]; then
    report_status="✅ Passed"
  else
    report_status="❌ Failed (exit code $command_status)"
  fi

  {
    printf '## %s\n\n%s\n\n' "$report_title" "$report_status"
    printf '<details><summary>Test output</summary>\n\n```text\n'
    tail -n 500 "$report_output" | sed -E $'s/\x1B\[[0-9;]*[[:alpha:]]//g'
    printf '```\n\n</details>\n'
  } >> "$GITHUB_STEP_SUMMARY"
fi

exit "$command_status"
