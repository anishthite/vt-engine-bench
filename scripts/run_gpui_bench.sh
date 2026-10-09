#!/bin/bash
# Run from a logged-in macOS desktop session with Screen Recording permission.
set -euo pipefail
cd "$(dirname "$0")/.."
root="$PWD"
PATH="$root/.context/zig:$PATH" cargo build --release --features gpui-bench --bin gpui_bench
capture="$(mktemp "${TMPDIR:-/tmp}/vt-capture-gpui.XXXXXX")"
trap 'rm -f "$capture"; if [[ -n "${app_pid:-}" ]]; then kill "$app_pid" 2>/dev/null || true; fi' EXIT
swiftc -parse-as-library scripts/capture_gpui.swift -o "$capture"
output="$(mktemp -d "$root/results/gpui-$(date +%F)-XXXXXX")"
printf 'Raw GPUI results: %s\n' "$output"
engines=(alacritty ghostty vt100 wezterm)
if (( $# )); then engines=("$1"); fi
for engine in "${engines[@]}"; do
  for run in 1 2 3; do
    prefix="$output/$engine-$run"
    printf '\n%s run %s/3\n' "$engine" "$run"
    "$root/target/release/gpui_bench" "$engine" "$prefix-events.csv" "$prefix-ready" >"$prefix-app.log" 2>&1 &
    app_pid=$!
    if ! "$capture" "$app_pid" "$prefix-events.csv" "$prefix-captures.csv" "$prefix-ready" >"$prefix-capture.log" 2>&1; then
      cat "$prefix-capture.log"; echo "Capture failed; raw diagnostics kept at $output" >&2; exit 1
    fi
    if ! wait "$app_pid"; then
      cat "$prefix-app.log"; echo "GPUI app failed; raw diagnostics kept at $output" >&2; exit 1
    fi
    app_pid=
    rm -f "$prefix-ready"
    cat "$prefix-app.log" "$prefix-capture.log"
    python3 scripts/summarize_gpui.py "$prefix-events.csv" "$prefix-captures.csv" || {
      echo "Invalid run; no comparison published. Diagnostics: $output" >&2; exit 1;
    }
  done
done
if (( $# == 0 )); then python3 scripts/summarize_gpui.py --aggregate "$output" > "$output/summary.csv"; fi
printf '\nValidated raw data: %s\n' "$output"
