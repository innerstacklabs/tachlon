#!/usr/bin/env bash
# Build release WASM and the web shell, then serve the 50k demo with Vite
# preview at http://127.0.0.1:4173/ (add ?n=1000 or ?n=10000 for smaller
# schedules, ?backend=webgl to skip WebGPU). With --schedule <file.xml|file.xer>
# as the first argument pair, copy that file into the bundle under the fixed
# name schedule.dat and serve it instead; the app picks MS Project XML or
# Primavera P6 XER from the file's content (a first line starting ERMHDR is XER).
# Ctrl-C stops the server.
#
# With --measure it runs the scripted measurement instead (docs/MEASURING.md,
# "Measuring frame times"): it opens a browser, drives the page, writes a JSON result to
# demos/50k/results/ and exits. Other arguments go to measure.mjs. Ctrl-C there
# stops the preview server and the browser too (measure.mjs handles SIGINT and SIGTERM).
set -euo pipefail

schedule=""
if [[ "${1:-}" == "--schedule" ]]; then
  if [[ $# -lt 2 || ! -f "$2" || ! -r "$2" ]]; then
    echo "run.sh: --schedule needs a readable regular file" >&2
    exit 2
  fi
  schedule="$(realpath -- "$2")"
  shift 2
  for arg in "$@"; do
    if [[ "$arg" == "--measure" ]]; then
      echo "run.sh: --schedule cannot be combined with --measure" >&2
      exit 2
    fi
  done
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bash "$root/scripts/build-web.sh"
if [[ "${1:-}" == "--measure" ]]; then
  shift
  exec node "$root/demos/50k/measure.mjs" "$@"
fi
if [[ -n "$schedule" ]]; then
  cp -- "$schedule" "$root/packages/gantt/dist/schedule.dat"
  cd "$root/packages/gantt"
  echo "serving http://127.0.0.1:4173/?schedule=1 ($schedule)"
  exec pnpm preview
fi
cd "$root/packages/gantt"
echo "serving http://127.0.0.1:4173/ (50,000 tasks; ?n=10000 for fewer)"
exec pnpm preview
