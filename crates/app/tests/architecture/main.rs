//! The architecture tests: the wasm module's public surface is the one list in
//! `docs/WASM_API.md`, and the source tree stays inside its size budget.
//!
//! These tests need no wasm build and no node. They parse the wasm module (every file under
//! `crates/app/src/web/`) with `syn` and read the other files as text, like
//! `crates/engine/tests/dependency_direction.rs`. Each failure names the file and line. One
//! module holds each check and its self-tests: `exports`, `web_sources`, `entry`, `entry_types`, `theming`, `embedding`,
//! `style_scope`, `size`, `hosting`, `name`, `chrome`, `lints`, `history_writes`, `late_start`, `key_lookups` and `gate`; `support` holds what they share.
//!
//! 1. Every export in the wasm module is a row under "Public" or "Test-only"
//!    on the page, and every row has an export. An export is a `pub fn` in a
//!    `#[wasm_bindgen] impl`, a free fn with `#[wasm_bindgen]`, or a `pub` field of a
//!    `#[wasm_bindgen]` struct (`Struct.field`). The syntax tree is walked, so comments,
//!    wrapped lines, `where` clauses, multi-line strings and inline modules cannot hide
//!    an export or fake one.
//! 2. Every export has a doc comment (a `doc` attribute with text).
//! 3. A "Test-only" name starts `test_`; a "Public" name does not.
//! 4. `wasm_bindgen` appears in no other `crates/*/src` file (a line scan). In
//!    the wasm module it appears only as the bare `#[wasm_bindgen]` attribute and in `use`
//!    items without an `as` alias; both are read from the syntax tree, so doc
//!    strings, string literals, comments and a wrapped `use` cannot trip it. The
//!    reader fails closed: a `#[wasm_bindgen]` on an enum, a const or any item that is
//!    not an impl, fn or struct fails, as does `wasm_bindgen` inside a macro body or a
//!    `cfg_attr`, and a module file that does not parse.
//! 5. `packages/gantt/src/**` (except the generated `packages/gantt/src/wasm/`) names a `test_*`
//!    identifier only in the page's "Test-side files", and names the test hook
//!    (`window.gantt`, `GanttHook`, `gantt.app`) only in `packages/gantt/src/test-hook.ts`.
//! 6. The parse is not empty: no exports, no doc rows or no TypeScript files
//!    fail the test, so a moved path cannot turn it into a false pass.
//! 7. Every `--gantt-` token the CSS (`packages/gantt/index.html`, `packages/gantt/src/chart.html`, `packages/gantt/src/**/*.css`, the
//!    TypeScript sources) or the scene uses is in the page's
//!    "Theming" section, and every token listed there is used.
//! 8. The chart can be embedded in a box of any size and takes keys only while focus is
//!    inside it (decision 0005, gs#100 PR 4). `packages/gantt/src/**` (except `measure.ts`, which
//!    records the window's size in its report), the page's CSS, `packages/gantt/index.html` and `packages/gantt/src/chart.html` do not
//!    read the window's size: `innerWidth`, `innerHeight`, `outerWidth`, `outerHeight`, and
//!    the units `vw`, `vh`, `vi`, `vb`, `vmin`, `vmax` and their small, large and dynamic
//!    forms, in any case. No
//!    `keydown`, `keyup` or `keypress` listener (`addEventListener` or an `on…` property)
//!    is on `window`, `document`, `document.body` or `document.documentElement`, and
//!    neither the `<html>` nor the `<body>` tag has such an attribute.
//! 9. Every selector in the `<style>` blocks of `packages/gantt/index.html`, `packages/gantt/src/chart.html` and in `packages/gantt/src/**/*.css`
//!    starts with `.gantt` or `:where(...)`, where every comma-separated item inside starts
//!    with `.gantt` in the same way, so a stylesheet of the chart cannot restyle
//!    the page around it. `@media`-style blocks are read through; the page's own reset is
//!    a `style` attribute on `<html>` and `<body>`, which is not a selector.
//! 10. File size (gs#160, `size.rs`): no source file or test module is past its limit, and the
//!     files that were past it when the budget began are listed at their size, which may only
//!     shrink.
//! 11. The page entry (gs#534, `entry.rs`): every export of `packages/gantt/src/index.ts` is a row under
//!     "Page entry" on the page, and every row has an export, and no export names a measure or page-knobs
//!     type (gs#552). A form whose names the reader cannot
//!     see (`export *`, `export default`) fails closed, and an empty parse fails.
//! 12. The hosted app is a separate project (gs#537, `hosting.rs`, decision 0007): the component
//!     (`crates/*/src`, `packages/gantt/src`, `packages/gantt/index.html`, the build files) names nothing about hosting;
//!     `apps/hosted` imports only `@gantt/chart` (`packages/gantt/src/index.ts`), its own files and the
//!     packages it declares, and both the Vite alias and the `tsconfig.json` path point at the
//!     entry. An empty parse fails.
//! 13. Retired project names stay out (gs#609, gs#1120, `name.rs`): no tracked file
//!     (`git ls-files -z`) and no tracked path names the working code name (hyphen or
//!     underscore) or the product name that followed it, in any case. Exempt: `docs/decisions/`,
//!     an entry of `name/allowlist.txt` (a whole file, or `path | text # why` for one exact text;
//!     sorted, only ever shrinking), and a line of a `docs/**/*.md` file that ends with
//!     `<!-- verbatim -->`. A failed `git`, an empty list, an unreadable, unsorted or malformed
//!     allowlist, an entry that no longer matches and a listed path that is not tracked fail. The
//!     product's name is written only in `name.rs`'s `BRAND_FILES` and under `examples/`; anywhere else fails.
//! 14. The chart does not import the app's chrome (gs#1087, decision 0012, `chrome.rs`): no file of
//!     `packages/gantt/src` (but the generated `packages/gantt/src/wasm/` and the developer page `main.ts`) imports, re-exports or
//!     dynamically imports a relative path whose file is `toolbar`, `filters-menu`, `find` or `status-line`.
//!     Those live in `apps/chrome/` and use only the chart's public API. The imports main had when the
//!     check began are `EXEMPT` (`path | import`, sorted bytewise, only shrinking: a listed import that is
//!     gone fails until its line is deleted). An empty parse fails.
//! 15. The chart's folder is named in one place (tl#1089, `web_root.rs`): `WEB_ROOT` is its one definition; any other
//!     code line of this directory's `.rs` files that spells the folder out fails.
//! 16. Every workspace member is on the strict lints (gantt#408, `lints.rs`): each `Cargo.toml` named in
//!     the root `members` holds `workspace = true` under `[lints]`. A member without it fails by name,
//!     and a root that names no member fails.
//! 17. The history stacks change in one place (gantt#2077, decision 0019, `history_writes.rs`): in
//!     `crates/app/src/history.rs` and `crates/app/src/history/`, a push, pop or clear of the undo or redo
//!     stack, or a call to `History::push`, `move_to_redo` or `move_to_undo`, outside `Session::commit` and `impl History` fails and names the
//!     file and line. An empty read fails.
//! 18. A late start is read in one place (decision 0018, rule 7, `late_start.rs`): in `crates/*/src`, the name `ls` is
//!     allowed only in the engine's `cpm`, the error's own field and `Schedule::late_start_shown`; any other use fails.
//! 19. A host key is looked up in one place (decision 0016, rule 2, `key_lookups.rs`): in `crates/app/src`,
//!     code outside `host/keys.rs` that reads a field of the key map, or searches or indexes by a record's `key`, fails
//!     and names the file and line. An empty read fails.
//! 20. The edit gate runs once per action (gantt#2085, decision 0019, `gate.rs`): the functions that call
//!     `gate` are exactly the ten actions it lists, and none is `run`, `apply`, `run_group` or in `edit.rs`.
//!
//! Threat model: a good-faith author who forgets. A name built at run time
//! (`app["test_" + n]`, `window["gantt"]`) is not caught; this is a text test.
//! A `--gantt-` token built at run time (as `packages/gantt/src/theme.ts` does with a
//! template literal) is not seen either; the tokens it reads come from the
//! scene's token list, which is checked.

#[macro_use]
mod web_root;

mod chrome;
mod embedding;
mod entry;
mod entry_types;
mod exports;
mod gate;
mod history_writes;
mod hosting;
mod key_lookups;
mod late_start;
mod lints;
mod name;
mod palette;
mod size;
mod style_scope;
mod support;
mod theming;
mod web_sources;

use embedding::check_embedding;
use entry::check_entry;
use exports::{check_surface, hidden_exports};
use style_scope::check_style_scope;
use support::*;
use theming::{LITERAL_COLOURS, THEME_RS, check_literal_colours, check_theming, used_tokens};
use web_sources::check_web_sources;

/// Every failure over the real files.
fn run_all() -> Vec<String> {
    let page_text = read(PAGE);
    let page = parse_page(&page_text);

    let rust = rust_sources();
    let mut failures = Vec::new();
    if rust.is_empty() {
        failures.push("crates/*/src: found 0 Rust files; the scan reads nothing".to_string());
    }
    let (web_module, others): (Vec<_>, Vec<_>) =
        rust.into_iter().partition(|file| is_web_module(&file.path));
    failures.extend(check_surface(&web_module, &page));
    for file in &others {
        failures.extend(hidden_exports(&file.path, &file.text));
    }

    let web = web_sources();
    failures.extend(check_web_sources(&web, &page.side_files));

    let html = web_html();
    let css = web_css();
    let used = used_tokens(&read(THEME_RS), &html, &css, &web);
    failures.extend(check_theming(&page.tokens, &used));
    failures.extend(check_literal_colours(&css, LITERAL_COLOURS));
    failures.extend(check_embedding(&web, &css, &html));
    failures.extend(check_style_scope(&css, &html));
    failures.extend(check_entry(&read(INDEX_TS), &page.entry));
    failures
}

#[test]
fn the_public_surface_matches_the_page() {
    let failures = run_all();
    assert!(
        failures.is_empty(),
        "the wasm public surface and {PAGE} disagree ({} problems; the page says how each \
         is checked under \"How it is enforced\"):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
