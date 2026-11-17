# Format readers are separate crates, licensed MIT OR Apache-2.0

Status: accepted. Decided by Dave, 2026-10-23 10:13.

## Context

Decision 0003 makes a native Rust P6 XER reader the first format and licenses the project AGPL-3.0-only. No Rust XER crate exists (research/2026-10-market.md, Formats). XER changes across P6 versions, and vendors' own importers fail on some of those variants, so a reader is only as good as the files it has been tested against. What is new in this project, and hard to copy, is the engine and the GPU timeline. Competitors can already read XER through MPXJ (Java, LGPL).
Superseded by gs#1044: Tachlon became closed source (2026-10-31); decision 0014 later made it MIT.

## Decision

1. The XER reader is its own crate, `crates/xer`. It does not depend on `engine`. Its model mirrors XER's tables, and the mapping into the engine model lives outside it (in `app` or an import module).
2. `crates/xer` is licensed `MIT OR Apache-2.0`. The engine, the timeline, the app and the web shell stay `AGPL-3.0-only`. `CLA.md` covers contributions to both.
Superseded by gs#1044: Tachlon became closed source (2026-10-31); decision 0014 later made it MIT.
2026-10-31: the rest of the project is now closed source, not AGPL; crates/xer stays MIT OR Apache-2.0 and is open-sourced at launch (gs#1044).
3. It is read-only and "best effort" at first. Writing XER comes with the scheduler journeys (`docs/PRODUCT.md`, J9).
4. Fixtures are public sample XERs or generated files, never a real person's schedule.
5. Once it round-trips files from several P6 versions, it moves to its own repository and is published to crates.io. The name `xer` is taken by an unrelated crate; the name is chosen then.

## Consequences

- People who hit a parse failure can use the crate in their own tools and send the file that broke it, which is how a format reader gets good. An AGPL reader would put those people off.
Superseded by gs#1044: Tachlon became closed source (2026-10-31); decision 0014 later made it MIT.
- The licence boundary is a crate boundary, so `cargo metadata` shows it, and a format crate never imports AGPL code.
Superseded by gs#1044: Tachlon became closed source (2026-10-31); decision 0014 later made it MIT.
- A second format reader (P6 XML, MSPDI) follows the same rule when it is split out. The existing MSPDI importer (`crates/import`) stays as it is until it is.
- Publishing adds upkeep: issues and odd files from strangers. The "best effort" label sets that expectation.
