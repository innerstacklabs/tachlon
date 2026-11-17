#!/usr/bin/env bash
# Build the browser app: WASM for crates/app (profile wasm-release, see
# Cargo.toml), wasm-bindgen into the chart folder's src/wasm/, then the Vite bundle into
# its dist/. The folder is named in scripts/lib/web-root.ts.
#
# Needs: the wasm32-unknown-unknown target, wasm-bindgen-cli at exactly the
# wasm-bindgen crate version in Cargo.lock, and pnpm. Honours
# CARGO_TARGET_DIR. Pass --no-bundle to stop after wasm-bindgen (CI runs
# the pnpm steps itself). --profile wasm-dev is the faster, larger build for
# checks; the default stays wasm-release (what ships, and CI's size budget).
# Only wasm-release drops the wasm's `name` section (function names for stack
# traces and profilers, 17% of the file); wasm-dev keeps it. --keep-names keeps it on wasm-release too, and adds
# the app's `alloc-stats` feature (a counting allocator): the profile build of demos/bench/profile.ts, never shipped; the
# script checks the wasm has the allocator's export exactly when it was asked for.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
chart="$root/$(node "$root/scripts/lib/web-root.ts")"
cargo="${CARGO:-cargo}"
bundle=1
keep=0
profile=wasm-release
while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-bundle) bundle=0 ;;
    --keep-names) keep=1 ;;
    --profile) shift; profile="${1:-}" ;;
    *) echo "build-web: unknown argument '$1'" >&2; exit 2 ;;
  esac
  shift
done
[[ "$profile" =~ ^wasm-(release|dev)$ ]] || { echo "build-web: --profile is wasm-release or wasm-dev" >&2; exit 2; }

want="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/, ""); print; exit}' "$root/Cargo.lock")"
have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')" || true
if [[ "$have" != "$want" ]]; then
  echo "build-web: need wasm-bindgen-cli $want (found '${have:-none}'):" >&2
  echo "  cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

features=()
[[ "$keep" == 1 ]] && features+=(--features alloc-stats)
"$cargo" build --manifest-path "$root/Cargo.toml" --profile "$profile" --target wasm32-unknown-unknown -p app ${features[@]+"${features[@]}"}
target_dir="${CARGO_TARGET_DIR:-$root/target}"
bindgen_args=()
[[ "$profile" == wasm-release && "$keep" == 0 ]] && bindgen_args+=(--remove-name-section)
out="$chart/src/wasm"
wasm-bindgen --target web --out-dir "$out" --out-name app ${bindgen_args[@]+"${bindgen_args[@]}"} \
  "$target_dir/wasm32-unknown-unknown/$profile/app.wasm"

# The shipped build has no counting allocator and no export for it; only --keep-names does.
if grep -q test_alloc_stats "$out/app.js"; then has=1; else has=0; fi
[[ "$has" == "$keep" ]] || { echo "build-web: test_alloc_stats in the wasm is $has, expected $keep (--keep-names is $keep)" >&2; exit 1; }

if [[ "$bundle" == 1 ]]; then
  cd "$chart"
  pnpm install --frozen-lockfile
  pnpm build
fi
