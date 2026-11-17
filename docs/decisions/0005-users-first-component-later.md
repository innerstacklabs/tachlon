# Users first; the developer component later, behind a public API

Status: superseded by [decision 0021](0021-gantt-component-for-developers.md) (Dave, 2026-11-17). Was accepted, decided by Dave, 2026-10-23 14:37 (interview after the developer-market research).
Superseded by [decision 0014](0014-mit-no-paid-tier.md) for its license item (item 4): Tachlon is MIT, with no paid tier (2026-11-13).

## Context

Browser Gantt components are sold to developers: Bryntum, DHTMLX, Syncfusion, Telerik and SVAR charge about $750 to $1,000 per developer, with the larger money in OEM and SaaS terms, and none of them sells its own end-user Gantt app (research/2026-10-developer-market.md). Only Syncfusion publishes speed at 50,000 tasks, in a benchmark of its own product, and its best initial render there is 10.2 s; Bryntum's forum shows 40,000-task loads taking about 30 s. Sagan's read is that few developers ever load 10,000 tasks (unsourced). Decision 0003 aims the product at people who read and review P6 schedules. The question was whether to aim at those users, at developers, or at both.

Three facts weigh against a developer-first product now: the WASM module is 2.0 MB gzipped, strict enterprise Content Security Policies must allow `'wasm-unsafe-eval'`, and a GPU canvas cannot take the HTML or CSS bar templates developers use for custom bars. AGPL code is banned at Google and in companies that copy its policy, which sagan reads as the buyers with the largest schedules (unsourced); the market leaders (AG Grid, SVAR, DHTMLX since v10) use an MIT core with paid features.
Superseded by gs#1044: Tachlon became closed source (2026-10-31); decision 0014 later made it MIT.

## Decision

1. **Users first.** The reader app for P6 readers and reviewers stays the product (decision 0003, PRODUCT.md milestones M4 to M7).
2. **The developer component comes later**, as a thin publication of what the app already uses. No date is set for packaging it (npm, a custom element, a React wrapper, docs, live samples). Sagan suggests pitching it as the engine for large schedules rather than a general Gantt; that is a proposal, not part of this decision.
3. **The app uses only a public API from now on.** `web/` calls only the documented public surface of the WASM module; an architecture test enforces it (gs#100). So the later package is a wrapper, not a rewrite.
4. **The license stays AGPL-3.0 plus a commercial license, re-decided before the component is published**, at the M5 architecture pause at the latest. Sagan suggests weighing the open-core alternative then (a permissive viewer with paid features, as SVAR and AG Grid do). The CLA keeps every option open.
Superseded by gs#1044: Tachlon became closed source (2026-10-31); decision 0014 later made it MIT.

## Consequences

- Each P6 reviewer who uses the app is evidence for the later developer pitch: the engine runs real 50,000-activity files.
- The public API costs a little now: a new feature the app needs is added to the documented surface first.
- Bundle size, CSP and custom bar rendering are known gaps for the developer audience; they are listed for the M5 pause, not worked now.
- If the license changes to open core, contributors' code is covered by the CLA.
