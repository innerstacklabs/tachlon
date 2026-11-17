# Fonts shipped with the page

| File | Font | Size | CSS family |
|---|---|---|---|
| `LiberationSans-Regular.woff2` | Liberation Sans Regular 2.1.5 | 147,036 bytes | `Gantt Sans`, weight 400 |
| `LiberationSans-Bold.woff2` | Liberation Sans Bold 2.1.5 | 148,576 bytes | `Gantt Sans`, weight 600 to 700 |
| `LiberationMono-Regular.woff2` | Liberation Mono Regular 2.1.5 | 126,124 bytes | `Gantt Mono`, weight 400 |

The Liberation fonts are copyright (c) 2012 Red Hat, Inc. (Reserved Font Name "Liberation"). Their digitized
data is copyright (c) 2010 Google Corporation (Reserved Font Names "Arimo", "Tinos" and "Cousine"), the upstream
fonts that Liberation Sans and Liberation Mono build on: Liberation Sans on Arimo, Liberation Mono on Cousine
(Tinos is the serif, which this page does not ship). Both parts are under the SIL Open Font License 1.1. The
licence text is `OFL.txt` in this directory; it permits bundling and redistribution with software. The same
Liberation Sans Regular is already in the repo as `crates/timeline/assets/LiberationSans-Regular.ttf`, which
the chart's text drawing uses.

The woff2 files are a format change of the 2.1.5 TTFs from the Debian `fonts-liberation` package (the Regular
file's TTF is byte-identical to the one in `crates/timeline/assets/`): the same font data repacked as woff2, with
no change to a glyph, a metric or an internal name. Under the OFL that makes them a Modified Version, which keeps
the licence and this notice with it. `Gantt Sans` and `Gantt Mono` are only the CSS family names the page loads
them under, an alias; neither the files nor their name tables carry them, and no Reserved Font Name ("Liberation",
"Arimo", "Tinos", "Cousine") is used as a name for the page's fonts. The npm package ships `OFL.txt` and this
notice beside the woff2 files in `dist-lib/`.

To rebuild a file: `wawoff2` (npm) `compress` over the TTF.
