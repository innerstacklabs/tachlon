# How it is built and checked

This chapter is for people who work on Tachlon itself. To put the chart in your own product, start at
[Install it in your app](install.md). The other developer references are `docs/DEMOS.md` (how to build,
test and run the demos), `docs/MEASURING.md` (frame times), `docs/ARCHITECTURE.md` and the
[Architecture](architecture.md) chapter.

## Run it

```sh
demos/50k/run.sh    # builds the wasm and the bundle, serves http://127.0.0.1:4173/
```

The page opens a generated 50,000-task schedule. To start it with a file already open, pass the
file on the command line: `demos/50k/run.sh --schedule <file.xml or file.xer>`. `docs/DEMOS.md`
has the build requirements and the URL options, and `docs/MEASURING.md` the frame-time
measurements.

## Where the chapters are checked

For the [Open a schedule](open-a-schedule.md) chapter,
`apps/hosted/tests/j1-open.spec.ts` opens an imported schedule and checks its rows, its task names and
the closable panel of what was not read; `packages/gantt/tests/open.spec.ts` opens files from the page and
records every request the page makes. The XER mapping is described in
[Architecture](architecture.md) and in `docs/IMPORT.md`. The other chapters
name their specs in their own "Where it is checked" sections until they are rewritten in the new
form.

## How this book is built

`mdbook build book` writes the book to `book/book/`, which is git-ignored. CI builds it on every
push (the `book` job in `.github/workflows/ci.yml`) and fails if a chapter file or an include is
missing, or if the [Keys](keys.md) page lost the table it includes from `docs/KEYS.md`. The `hosted-browser` job's book spec and the `deploy` job use its build, not their own.

The look is the book's own mdBook theme in `book/theme/`: `index.hbs` and `css/variables.css`
override the stock files (two themes, Light and Dark, in the chart's calm palette and the
mascot's colors), and `book.css` adds the components a chapter uses (the clip, the
screenshots, the tip from Tachlon, the front page). The mascot files are in `book/src/media/`.

### Writing a chapter

A chapter is one task the reader wants to do. The shape, in order:

1. The H1 is the task in plain words, with the journey id in an HTML comment under it.
2. One or two sentences on what you can do.
3. The clip: a `<video class="clip">` with an MP4 and a poster, recorded by
   `demos/screencast/clip.mjs` from a step in `demos/screencast/steps-features.mjs`.
4. Numbered steps, each one action.
5. One or two screenshots, as `<figure class="shot">`, each with a caption that says what the
   numbers in the picture point at.
6. A tip from Tachlon, as `<div class="book-tip">`, and a "Good to know" list for limits.

To record the clip and the screenshots for a step, run these in order:

1. `bash scripts/build-web.sh --no-bundle`
2. `(cd apps/hosted && pnpm install --frozen-lockfile && node scripts/build-samples.mjs && pnpm build)`
3. `node demos/screencast/clip.mjs --step open-file --name open-a-schedule --out book/src/media --file packages/gantt/tests/fixtures/ledger-timeline.xer`

`clip.mjs` plays one step from `steps-features.mjs` in a headless browser, writes
`<name>.mp4` (1280x720, 30 fps) and `<name>.webp` (the poster, the last frame), and the
screenshots the step takes through `ctx.shot`. The media budget per chapter: the clip is 10
to 20 seconds and under 3 MB; each poster or screenshot is under 300 KB. `clip.mjs` refuses a
clip outside the budget. The main screencast on the front page is `book/src/media/screencast.mp4`, a
copy of `docs/media/screencast.mp4` (the same file).
