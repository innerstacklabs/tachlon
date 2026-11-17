# The working code name is Mantis Gantt

Status: accepted, as a working code name. Decided by Dave, 2026-10-26 14:37.

Amended 2026-10-28 (gs#524): the project was called gantt-spike until the rename; the code, the book title and the docs now say Mantis Gantt; the `name` architecture check keeps it so.

## Context

"gantt-spike" was the working name (PRODUCT.md, "Name"): the name of a hobby spike, kept until the reader was usable. The README is the first thing an outside reader sees, and the first such reader (a construction-management professional who may help shape the product) was about to read it. "gantt-spike" reads as an engineering log, not a product, and the repository was about to move to `innerstacklabs/mantis-gantt`.

The name search in PRODUCT.md found "FastGantt" taken and no product named "Screaming Gantt" or "Speed of Light Gantt" (web search only, as of 2026-10-23). No trademark search has been done for "Mantis Gantt" either.

## Decision

1. **The working code name is "Mantis Gantt".** The repository is `innerstacklabs/mantis-gantt`; the README, the mascot and the icon use the name.
2. **Why the mantis shrimp.** It has the fastest strike in nature, so fast that the water around it flashes with light; it has the best colour vision of any animal; and it is a crustacean, like Ferris, the Rust mascot. Speed, light and sight, in a language-family in-joke.
3. **It is a code name, not a decided product name.** It can change before the product is named for release. The README carries the new name first.
4. **The mascot and the icon** are `docs/media/brand/mantis-mascot.jpg` and `docs/media/brand/mantis-icon.png` (the mascot from karpathy's brand brief, 2026-10-26; the icon redrawn on a transparent background at Dave's request, 2026-10-27). They are placeholders until the design work in gs#307 replaces them.

## Consequences

- A first-time reader meets a product with a name, a face and a one-line pitch before any engineering detail.
- The rename is carried out in gs#524. A link to the old repository path still reaches the repository, because GitHub redirects it to `innerstacklabs/mantis-gantt`.
- Before a public release, the name gets a trademark search and Dave's decision on whether it stays.
