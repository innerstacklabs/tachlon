#!/usr/bin/env bash
# Build and record the screencast from the hub: default records on the Mac,
# while --local records a software-GL draft on the hub. The README label names the
# branch you are on and its base commit (the merge-base with origin/main); a recording
# on main is labelled "from `main` at `<commit>`". --local (only) also runs from a detached HEAD (git worktree add --detach):
# the draft is then labelled "from detached HEAD based on `<commit>`".
#
# A recording is two runs of record.mjs: the README story (steps.mjs, screencast.gif/.mp4/.json) and the feature scenes
# (steps-features.mjs, --list features), which are cut into one clip per scene that sets clip: true. --local writes
# both into demos/screencast/draft/ (the clips in draft/features/). The Mac run is copied into the repo only if its
# screencast.json and features/clips.json say backend WebGPU and fps 50 or more (copy-clips.mjs guard); a clip is
# copied into book/src/media/features/ only when its scene's hash or the look epoch in that folder's clips.json changed.
#
# --upload (maintainers only, opt-in) uploads the new docs/media/screencast.mp4 as a GitHub attachment and writes the
# URL it reads back into the README's video line, with the scripts in demos/screencast/upload/. That directory holds
# the maintainers' own browser tooling and is not in the public repository, so --upload stops there. Without
# --upload a re-record leaves the README's old video URL in place and update-readme.mjs warns about it.
set -euo pipefail

usage() {
  echo "usage: demos/screencast/run.sh [--local | --upload]" >&2
}

mode="mac"
upload=0
if [[ $# -eq 1 && "$1" == "--local" ]]; then
  mode="local"
elif [[ $# -eq 1 && "$1" == "--upload" ]]; then
  upload=1
elif [[ $# -ne 0 ]]; then
  usage
  exit 2
fi

# A non-login shell (an ssh command, a systemd unit, an agent's shell) does not have ~/.cargo/bin on its PATH, where
# rustup puts cargo and `cargo install` puts wasm-bindgen (gs#493). Add it at the end, so a cargo or wasm-bindgen that
# is already on the PATH still runs first, and stop here with the missing tool's name, not at build-web.sh.
cargo_bin="${HOME:-}/.cargo/bin"
if [[ -n "${HOME:-}" && -d "$cargo_bin" && ":$PATH:" != *":$cargo_bin:"* ]]; then
  PATH="$PATH:$cargo_bin"
fi
missing=()
for tool in "${CARGO:-cargo}" wasm-bindgen; do
  command -v "$tool" >/dev/null || missing+=("$tool")
done
if (( ${#missing[@]} > 0 )); then
  echo "run.sh: not found on the PATH or in ~/.cargo/bin: ${missing[*]}" >&2
  echo "run.sh: install cargo with rustup, and wasm-bindgen with the version in Cargo.lock:" >&2
  echo "run.sh:   cargo install wasm-bindgen-cli --version <version in Cargo.lock> --locked" >&2
  exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

if ! timeout 60 git fetch origin main; then
  echo "warning: could not fetch origin main; using the existing origin/main" >&2
fi
detached=0
branch="$(git symbolic-ref --short -q HEAD || true)"
if [[ -z "$branch" ]]; then
  if [[ "$mode" != "local" ]]; then
    echo "run.sh: HEAD is detached, so the recording has no branch to be labelled with." >&2
    echo "run.sh: check out a branch (git switch <branch>) and run it again; --local accepts a detached HEAD." >&2
    exit 1
  fi
  # The same text as DETACHED_BRANCH in label.mjs, which words it as "from detached HEAD based on <commit>". Only --local gets here.
  branch="(detached)"
  detached=1
fi
if [[ "$detached" != "1" ]] && ! [[ "$branch" =~ ^[A-Za-z0-9._/-]+$ ]]; then
  echo "run.sh: branch name '$branch' has characters the README label cannot carry (allowed: letters, digits, . _ / -)." >&2
  echo "run.sh: rename it (git branch -m <new-name>) and run it again." >&2
  exit 1
fi
if [[ "$upload" == "1" ]]; then
  if [[ ! -f demos/screencast/upload/run.mjs ]]; then
    echo "run.sh: --upload needs the maintainers' upload scripts, which this repository does not carry." >&2
    exit 2
  fi
  # Fail in seconds, not after the recording: signs in, opens the Add Files chooser, uploads nothing.
  node demos/screencast/upload/run.mjs --dry-run
fi
base="$(git merge-base HEAD origin/main)"
commit="$(git rev-parse --short=7 "$base")"
tracked_paths=(
  crates packages/gantt/src packages/gantt/index.html packages/gantt/package.json packages/gantt/vite.config.ts apps/hosted
  packages/gantt/pnpm-lock.yaml scripts/build-web.sh Cargo.toml Cargo.lock
)
if ! git diff --quiet "$base" -- "${tracked_paths[@]}"; then
  echo "run.sh: bundle paths differ from main:" >&2
  git diff --name-only "$base" -- "${tracked_paths[@]}" >&2
  if [[ "${GS_SCREENCAST_ALLOW_DIRTY:-}" != "1" ]]; then
    echo "run.sh: the recording would show code that its base commit $commit does not contain." >&2
    echo "run.sh: merge the change and record on main, or run again with GS_SCREENCAST_ALLOW_DIRTY=1" >&2
    echo "run.sh: to record this branch anyway, labelled \"$branch\" based on $commit-dirty." >&2
    exit 1
  fi
  commit="$commit-dirty"
fi
# The close card's caption names the repository; the product's name stays in product.ts, so it is read from there at run time.
SCREENCAST_REPO="innerstacklabs/$(node --input-type=module -e 'import { PRODUCT_NAME } from "./packages/gantt/src/product.ts"; console.log(PRODUCT_NAME.toLowerCase())')"
export SCREENCAST_REPO

# The recording runs on the hosted build, opened on ?sample=demo-50k (gs#541): the wasm, then the samples
# (build-samples.mjs needs cargo on the PATH) and the hosted bundle. packages/gantt/dist is not built, the package (`pnpm build:lib`) is. The build directory and the
# address are in hosted-build.mjs, which record.mjs and clip.mjs read too (gs#601), so the README video and the book's
# clips show the same app.
hosted_dist="$(node demos/screencast/hosted-build.mjs dist)"
bash scripts/build-web.sh --no-bundle
(cd packages/gantt && pnpm install --frozen-lockfile && pnpm build:lib)
(cd apps/chrome && pnpm install --frozen-lockfile)
(cd apps/hosted && pnpm install --frozen-lockfile && node scripts/build-samples.mjs && pnpm build)

if [[ "$mode" == "local" ]]; then
  set +e
  node demos/screencast/record.mjs \
    --dist "$hosted_dist" --out demos/screencast/draft \
    --machine "hub (draft, software GL)" --commit "$commit" --branch "$branch"
  status=$?
  if [[ "$status" == "0" ]]; then
    node demos/screencast/record.mjs --list features \
      --dist "$hosted_dist" --out demos/screencast/draft \
      --machine "hub (draft, software GL)" --commit "$commit" --branch "$branch"
    status=$?
  fi
  set -e
  echo "draft only: demos/screencast/draft/ is git-ignored, committed media comes from the Mac"
  exit "$status"
fi

ssh_options=(-o BatchMode=yes -o ConnectTimeout=20)
rsync_ssh="ssh -o BatchMode=yes -o ConnectTimeout=20"
timeout 120 ssh "${ssh_options[@]}" mbp 'mkdir -p ~/gantt-probe/screencast/fixtures'
timeout 300 rsync -a --delete -e "$rsync_ssh" \
  "$hosted_dist/" mbp:gantt-probe/screencast/dist/
# Copy the whole directory so a new module is never forgotten; the excludes are in a file the test reads.
timeout 120 rsync -a -e "$rsync_ssh" \
  --exclude-from=demos/screencast/mac-copy-excludes.txt \
  demos/screencast/ mbp:gantt-probe/screencast/
# The open-file scene's schedule (steps-features.mjs) lives outside demos/screencast/.
timeout 120 rsync -a -e "$rsync_ssh" \
  packages/gantt/tests/fixtures/ledger-timeline.xer mbp:gantt-probe/screencast/fixtures/
timeout 900 ssh "${ssh_options[@]}" mbp \
  "mkdir -p ~/gantt-probe/tmp && cd ~/gantt-probe/screencast && TMPDIR=\$HOME/gantt-probe/tmp SCREENCAST_REPO=$SCREENCAST_REPO /opt/homebrew/bin/node record.mjs --dist dist --out out --machine 'MacBook Pro' --commit $commit --branch $branch"

timeout 900 ssh "${ssh_options[@]}" mbp \
  "mkdir -p ~/gantt-probe/tmp && cd ~/gantt-probe/screencast && TMPDIR=\$HOME/gantt-probe/tmp SCREENCAST_REPO=$SCREENCAST_REPO /opt/homebrew/bin/node record.mjs --list features --dist dist --out out --machine 'MacBook Pro' --commit $commit --branch $branch"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
timeout 180 rsync -a -e "$rsync_ssh" \
  mbp:gantt-probe/screencast/out/screencast.gif \
  mbp:gantt-probe/screencast/out/screencast.mp4 \
  mbp:gantt-probe/screencast/out/screencast.json "$stage/"
timeout 180 rsync -a -e "$rsync_ssh" mbp:gantt-probe/screencast/out/features/ "$stage/features/"
# Nothing is copied from a recording that was not made on a GPU at a real frame rate.
node demos/screencast/copy-clips.mjs guard "$stage/screencast.json" "$stage/features/clips.json"
gif_bytes="$(stat -c %s "$stage/screencast.gif")"
if (( gif_bytes >= 5000000 )); then
  echo "run.sh: screencast.gif is $gif_bytes bytes; limit is under 5000000" >&2
  exit 1
fi
# Validates every clip before it copies any (a clip over 600,000 bytes stops the run here), so the story media is not half-copied.
node demos/screencast/copy-clips.mjs copy "$stage/features" book/src/media/features
mkdir -p docs/media
cp "$stage/screencast.gif" "$stage/screencast.mp4" \
  "$stage/screencast.json" docs/media/
if [[ "$upload" == "1" ]]; then
  node demos/screencast/update-readme.mjs --upload
  node demos/screencast/upload/run.mjs
else
  node demos/screencast/update-readme.mjs
fi
stat -c '%s bytes %n' docs/media/screencast.gif \
  docs/media/screencast.mp4 docs/media/screencast.json
