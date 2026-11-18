# Install matrix

The README's one-prompt install, run by a fresh coding assistant in each of four fresh apps (a Vite and React starter, a Vue starter, a Next.js starter and a plain HTML page). A run is two fresh sessions in one app folder: the README's prompt alone, then the fit task (`npx tachlon describe fit`: the app's colors and its own records with keys). The checks, the caps, the flags, the model and the assistants' settings are in [CONTRIBUTING.md](CONTRIBUTING.md). A cell is whether every check passed, then the turns and minutes of the assistant's own result line.

To re-run it for a release, after the release is published, run this and replace the rows below with the lines it prints (`codex` in place of `claude` for that column):

```sh
for app in vite-react vue next html; do node scripts/agent-install-check.ts claude --app "$app" --fit --version <version> --record <folder>/"$app" | tail -n 1; done
```

Package version 0.3.0, built from `origin/main` at 93b7205 (the published 0.3.0 has no `describe`), run 2026-11-18.

| app | assistant | result |
|---|---|---|
| vite-react | claude | install: pass, 9 turns, 0.5 min<br>fit: pass, 13 turns, 0.5 min |
| vue | claude | install: pass, 7 turns, 0.6 min<br>fit: pass, 8 turns, 0.5 min |
| next | claude | install: pass, 9 turns, 0.7 min<br>fit: pass, 8 turns, 0.5 min |
| html | claude | install: FAIL (mount), 11 turns, 0.5 min<br>fit: FAIL (mount), 11 turns, 0.5 min |
| all | codex | not run (both accounts walled, 2026-11-18) |

The `html` failure is innerstacklabs/tachlon#56. The Codex column and a mid-size existing app (not run: `apps/hosted` links the local package and already draws a chart) are innerstacklabs/tachlon#57.
