#!/usr/bin/env bash
# Where the time of a drag goes: callgrind over a criterion bench of
# `Schedule::recompute_cone` (default: the finish-moving drag at 50,000 tasks),
# collecting only inside that call, so schedule generation and the setup of
# every group stay out of the profile. Targets Linux hosts with valgrind
# (perf is often barred there: perf_event_paranoid). Function names carry the
# first caller of identical generic code, so `recompute_cone_with` and
# `compute` names both appear for shared passes.
# Usage: scripts/profile-drag.sh [bench filter, e.g. drag_step_cone/50000]
set -euo pipefail

cd "$(dirname "$0")/.."
log=$(mktemp)
trap 'rm -f "$log"' EXIT
CARGO_PROFILE_BENCH_DEBUG=line-tables-only cargo bench -p engine --bench cpm --no-run 2>&1 | tee "$log"
bin=$(sed -n 's|^[[:space:]]*Executable benches/cpm.rs (\(.*\))$|\1|p' "$log" | tail -1)
[[ -n "$bin" ]] || { echo "profile-drag: cargo did not report the cpm bench executable" >&2; exit 1; }

out=${CALLGRIND_OUT:-${CARGO_TARGET_DIR:-target}/callgrind-drag.out}
valgrind --tool=callgrind --callgrind-out-file="$out" --collect-atstart=no \
  --toggle-collect='<engine::model::Schedule>::recompute_cone' \
  "$bin" --bench --profile-time 2 "${1:-drag_finish_cone/50000}"
callgrind_annotate "$out" > "$out.self.txt" 2> /dev/null
callgrind_annotate --inclusive=yes "$out" > "$out.incl.txt" 2> /dev/null
echo "== self cost (Ir), top 12 =="; sed -n '/file:function/,$p' "$out.self.txt" | head -14
echo "== inclusive cost (Ir) of the engine's own functions, top 12 =="
grep -E ' crates/engine/src/' "$out.incl.txt" | head -12 || true
