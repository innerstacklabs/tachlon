# Screencast scripts

The scripts that record the README screencast and the book's chapter clips. How the recording works is in
[docs/DEMOS.md](../../docs/DEMOS.md), "Recording the screencast".

## Where to record

Anything that shows motion or a frame rate is recorded on the MacBook Pro, on its GPU. The hub records on
software GL (the hub's page reports WebGL2), so it cannot show the frame rate the product has, and a clip of a
scroll, a drag or a zoom from it would misstate it.

| What the clip shows | Record it on | Command |
|---|---|---|
| A drag, a scroll, a zoom, a fling, or a caption that claims speed | The MacBook | `demos/screencast/run.sh` (the story and the feature clips) |
| A still or near-still clip with no animation claim, such as `open-a-schedule.mp4` | The hub is fine, into a scratch folder; an agent copies it into the book by hand (see below) | `demos/screencast/clip.mjs` |
| A book chapter still (`j2-navigate`, `j4-checks`, `j5-print`, `j6-build`, `j7-change`, `j8-progress`, `j9-save`, `keys`) | The hub is fine, into a scratch folder; copy by hand as above | `demos/screencast/stills.mjs --out <scratch dir>` (`--dry` lists the stills with no browser; `--only <still>` writes one) |
| The product page's nine pictures of the chart (`apps/site/shots/*.webp`, not `video-poster.webp`) | The hub is fine: a still, no frame rate. It writes into `apps/site/shots` (or `--out`) at the sizes and crops the page's `<img>` tags name | `demos/screencast/site-shots.mjs` (`--dry` lists the names; `--only <name>` writes one). It takes the chart's colors from the two `.gantt[data-theme]` rules in `apps/site/home.css`, so a change to the page's identity is one run |
| A draft you look at and throw away | The hub, into a folder outside `book/src/media` | `demos/screencast/run.sh --local` writes `demos/screencast/draft/` |

Two guards keep a hub recording of motion out of the repo:

- `run.sh` copies nothing into the repo unless `screencast.json` and `features/clips.json` both say backend `WebGPU` and
  50 fps or more (`copy-clips.mjs guard`).
- `clip.mjs` applies the same check when `--out` is `book/src/media` or a folder inside it: it reads the backend from the
  page after the scene has run, measures the frame rate before it, and refuses with the backend or the rate it saw. It
  writes nothing when it refuses. With any other `--out` it checks nothing, so a hub run into a scratch folder still works.

The check is of the machine, not of the clip's content: a still clip passes only if the page reports WebGPU at 50 fps or
more. The hub's page reports WebGL2, so `clip.mjs` on the hub with `--out book/src/media` is refused even for a still.
On the hub, record the still into a scratch folder (for example `--out /tmp/clip-draft`), never into `book/src/media`.
An agent then copies the files into `book/src/media` by hand, only after checking that the clip shows no motion and no
frame rate (the gs#633 rule, applied as a deliberate copy and not as a tool default). Or record it on the MacBook.

## Tests

Neither test opens a browser:

```sh
node demos/screencast/test-clip-guard.mjs
node demos/screencast/test-story.mjs
```

`test-clip-guard.mjs` checks the `clip.mjs` guard with fake recordings: software GL into `book/src/media` is refused,
the same into a scratch folder is allowed, and WebGPU at 50 fps is allowed. It is not in the `web` job of CI;
`test-clips.mjs` needs ffmpeg and is not either.
