#!/usr/bin/env bash
# Records the narrated setup screencast: a new Vite app, `npm install <package>` from the npm registry, `<package> init`,
# the dev server, a file opened, the plain-HTML page from jsDelivr, `<package> doctor`. Host: hub (Chromium with software
# GL, ffmpeg, Node 22.18 or later); the video shows a setup, not motion, so docs/DEMOS.md's MacBook-for-motion rule does
# not apply. Needs the network, INWORLD_API_KEY for the voice (scripts/narrate.ts), and the published package.
#   bash demos/setup-screencast/run.sh      writes book/src/media/install-setup.mp4 and install-setup.webp
# Env: OUT (output folder), WORK (scratch folder), APP_PORT (the new app's dev server, 5173).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
here="$root/demos/setup-screencast"
out="${OUT:-$root/book/src/media}"
work="${WORK:-$HOME/.cache/setup-screencast}"
export APP_PORT="${APP_PORT:-5173}"
: "${INWORLD_API_KEY:?run.sh: set INWORLD_API_KEY, the voice key scripts/narrate.ts reads}"

rm -rf "$work"
mkdir -p "$work/demo" "$out"
cp -r "$here/www" "$work/www"
(cd "$root/packages/gantt" && pnpm install --frozen-lockfile)   # the recorder drives the repo's Playwright

# What a customer types, captured as it runs. The registry serves the package; nothing here is a local build.
pkg="$(node -p "JSON.parse(require('node:fs').readFileSync(process.argv[1], 'utf8')).name" "$root/packages/gantt/package.json")"
export PKG="$pkg"
cd "$work/demo"
npm create vite@latest my-app -- --template react-ts --no-interactive > "$work/cap-create.txt" 2>&1
cd my-app
npm install "$pkg" > "$work/cap-install.txt" 2>&1
sed "s/__PKG__/$pkg/g" "$here/open-schedule.tsx" > src/open-schedule.tsx
npx "$pkg" init > "$work/cap-init.txt" 2>&1
node "$here/build-pages.mjs" app "$work"
setsid npm run dev -- --port "$APP_PORT" --strictPort > "$work/cap-dev.txt" 2>&1 < /dev/null &
trap 'kill -- -$! 2>/dev/null || true' EXIT
for _ in $(seq 60); do grep -q "Local:" "$work/cap-dev.txt" && break; sleep 1; done
grep -q "Local:" "$work/cap-dev.txt" || { echo "run.sh: the dev server did not start:" >&2; cat "$work/cap-dev.txt" >&2; exit 1; }
npx "$pkg" doctor > "$work/cap-doctor.txt" 2>&1
node "$here/build-pages.mjs" terms "$work"

node "$here/record.mjs" "$work"
node "$here/join.mjs" "$work" "$out/install-setup.mp4" "$out/install-setup.webp"
stat -c '%s bytes %n' "$out/install-setup.mp4" "$out/install-setup.webp"
