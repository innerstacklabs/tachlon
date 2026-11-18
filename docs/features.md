# Feature matrix

This chart is the package in this repository, set beside Bryntum Gantt, DHTMLX Gantt and Syncfusion Gantt. Read on 2026-11-15 at commit `3135461a` of `main` ("a bar takes its own color (#2001)"), plus the pull requests merged since 2026-11-12.

The README's "How it compares" section is this table, and `node scripts/features-matrix.ts --check` fails when the two differ or when the **Counts.** line under the table does not match the table. Edit the table here, then run `node scripts/features-matrix.ts --write` to copy it into the README and rewrite the Counts line, where the first column's heading is the product's name (`packages/gantt/src/product.ts`).

- **This chart:** a cell is `Yes` only when a file at that commit or a merged pull request shows the feature reachable through the npm package as built from this repository (`mount`, the chart's built-in UI, `chart.*`, `chart.commands`, or a file or document that `chart.load` reads). Merged but not reachable is `In progress`. Planned but not merged is `No`.
- **Vendors:** each vendor's own docs, feature table or pricing page. Pages marked (15) were read again on 2026-11-15; the rest come from the 2026-11-12 read. Sources are listed under the table.
- **Editions in the cells.** Bryntum names its tiers Community (free, MIT), **Core** (was Business) and Enterprise: "All" means all three, "Core, Ent" means Core and Enterprise, "Ent only" means Enterprise. DHTMLX has Community (free, MIT) and PRO: "Both" means both editions. Syncfusion has one edition. "Not found" means the vendor pages named under the table show no such feature; it is not a claim that the vendor lacks it.
- **★** in this chart's cell marks a row where this chart has the feature and no vendor's docs show it in the same form. The facts behind each ★ are in "Where this chart leads" below.

| # | Feature | This chart | Bryntum Gantt | DHTMLX Gantt | Syncfusion Gantt |
|---|---|---|---|---|---|
| | **Scheduling** | | | | |
| 1 | Four link types (FS, SS, FF, SF) with lag or lead | Yes | All | Both | Yes |
| 2 | Auto-scheduling: dates recomputed from links on every edit | Yes | All | PRO only | Yes |
| 3 | Critical path | Yes | Ent only | PRO only | Yes |
| 4 | Eight constraint types (ASAP, ALAP, SNET, SNLT, FNET, FNLT, must start on, must finish on) | Yes | Core, Ent | PRO only | Yes |
| 5 | Working calendars with holidays, and a calendar per task | Yes | Project calendar: All; task calendars: Core, Ent | Whole-chart working time: Both; project, resource and task calendars: PRO only | Project working time only |
| 6 | Baseline with variance | Yes | Ent only | PRO only | Yes |
| 7 | Progress: percent complete, actual start and finish | Yes | All | Both | Yes |
| 8 | Summary tasks in an outline | Yes | All | Both | Yes |
| 9 | Milestones | Yes | All | Both | Yes |
| 10 | Split tasks | No | Ent only | PRO only | Yes |
| 11 | Resource names on tasks, and a filter by resource | Yes | Core, Ent | PRO only | Yes |
| 12 | Edit resource assignments | No | Core, Ent | PRO only | Yes |
| 13 | Resource workload or histogram view | No | Widget in the examples; not in the feature table | PRO only | Over-allocation mark; no histogram found |
| 14 | Schedule-quality checks (DCMA-style counts) | Yes ★ | Not found | Not found | Not found |
| 15 | What-if and "why does this task start here" | Yes ★ | Not found | Not found | Not found |
| 16 | Compare a schedule with an earlier file | Yes ★ | Not found | Not found | Not found |
| | **Editing** | | | | |
| 17 | Task editor dialog | Yes | Basic: All; full: Core, Ent | Both | Yes |
| 18 | Inline cell editing in the task list | Yes | All | Both | Yes |
| 19 | Drag a bar to move it, and drag to draw a link | Yes | All | Both | Yes |
| 20 | Drag a bar's edge to change its duration | Yes | All | Both | Yes |
| 21 | Undo and redo | Yes | Core, Ent | PRO only | Yes |
| 22 | Group tasks (for example by resource) | Yes; by activity code | Core, Ent | PRO only | Yes |
| | **Files** | | | | |
| 23 | Open MS Project XML | Yes ★ | Ent only | Both, through its export service or a module you host | Yes, through a remote service |
| 24 | Open Primavera P6 XER | Yes ★ | One example; not in the feature table | Yes (XER or XML), through its service or module; edition not stated | Not found |
| 25 | Save as MS Project XML | Yes ★ | Ent only | Both, through its service or module | Through a server |
| 26 | Save as Primavera P6 XER | Yes ★ | Not found | P6 XML only, through its service or module | Not found |
| 27 | Export to Excel | Yes | Ent only | Both, through its service or module | Yes |
| 28 | Export to PDF | Yes, through the browser's print dialog | Ent only; needs a server-side service | Both, through its service or module | Yes |
| 29 | Export a picture (PNG) | Yes | Ent only; needs a server-side service | Both, through its service or module | No (open feature request) |
| 30 | Print with page header, page number and legend | Yes | Ent only | No print method found | No print method found |
| 31 | Read another vendor's Gantt data | Yes ★ | Not found | Not found | Not found |
| | **Display** | | | | |
| 32 | 50,000 tasks open and scroll (virtual rendering) | Yes | No figure stated | Smart rendering: Both; dynamic loading: PRO only | Yes (vendor benchmark at 50,000) |
| 33 | Zoom from years to hours, and fit the project | Yes | All | Both | Yes |
| 34 | Light and dark themes, colors set from CSS | Yes | All | Both | Yes |
| 35 | A color per task bar | Yes | All | Yes; edition not stated | Yes |
| 36 | Custom bar content from a template | No | All | Yes; edition not stated | Yes |
| 37 | Keyboard navigation | Yes | Yes | Both | Yes |
| 38 | Screen-reader access to the timeline bars | Yes | WCAG 2.1 and Section 508 stated as a goal | WAI-ARIA on by default; bars not stated | WCAG 2.2 AA "for most criteria" |
| | **Languages** | | | | |
| 39 | Translated UI text and locale number and date formats | Yes | All (45 locales) | Both (50+ locales) | Yes |
| 40 | Right-to-left layout | Yes; the timeline stays left to right | Not documented | Both | Yes |
| | **Developer API** | | | | |
| 41 | TypeScript declarations | Yes | Yes | Yes | Yes |
| 42 | React, Vue and Angular components | Yes | All | Both | Yes (also Blazor) |
| 43 | Schedule engine in Node with no page | Yes ★ | Not found | A separate "Planning Engine", by quote | Not found |
| | **AI** | | | | |
| 44 | Typed engine tools for an AI assistant (MCP, Anthropic, OpenAI forms) | Yes ★ | Not found (its AI feature is an in-chart agent) | A guide to writing tool calls yourself | Not found |
| 45 | In-chart chat assistant | Yes; you supply the model call | Ent only | No (guide only) | Not found |
| 46 | Docs for coding assistants (llms.txt, AGENTS.md or skills) | Yes | Yes | Yes | Yes |
| | **Licensing** | | | | |
| 47 | Free open-source license | Yes: MIT | Community: MIT, commercial use allowed | Community: MIT (version 10 on; GPL v2 for 9.x and earlier) | No: a free Community License, not open source |
| 48 | Every feature with no paid tier, key or trial | Yes ★ | No: Core $699, Enterprise $1,100 per developer per year; OEM by quote | No: PRO $799 to $5,999, perpetual | No: $959 per developer per year; free only under $1M revenue, 5 or fewer developers and 10 or fewer employees |

**Counts.** 48 rows. This chart: 44 Yes, 4 No.

## Where this chart leads (facts only)

1. **P6 and MS Project files in the browser, both ways** (rows 23 to 26). This chart reads XER and MS Project XML and writes both, with a list of what each format cannot carry, and sends no file to a server. DHTMLX converts on its export service or a module you host, and its P6 export writes XML, not XER. Bryntum lists MS Project import and export at Enterprise and shows P6 only in an example. Syncfusion uses a remote service for MS Project and has no P6 page.
2. **Schedule review** (rows 14 to 16). Fourteen schedule-check counts, ten of them with a Show button that keeps those tasks; what-if for up to 10 tasks; the driving path of a task; and a compare with last month's file that matches tasks by ID. None of the three vendors' docs has a page on these.
3. **Engine tools and a Node engine** (rows 43, 44). `toolDefinitions` gives six typed tools in MCP, Anthropic or OpenAI form, `propose` holds an edit as a ghost for a person to accept, and the `headless` export answers the same questions in Node. Bryntum's AI feature is an in-chart agent at Enterprise (this chart has a chat panel that calls the model you supply, row 45); DHTMLX documents how to write tool calls; DHTMLX sells its engine separately.
4. **Rival data in** (row 31). `fromDhtmlx` and `fromBryntum` read those vendors' data.
5. **No paid tier** (row 48). Every feature in this table is in the one MIT package.
6. Speed is not a row here: the measured table is in the README.

## Evidence for this chart

Cites are paths at `origin/main` `3135461a` or merged PR numbers. "pkg README" is `packages/gantt/README.md`.

- 1: `crates/engine/src/model.rs` (`LinkType`, lag); pkg README "Quick start" (link `type`).
- 2, 3: `crates/engine/src/cpm.rs`; `chart.commands.criticalPath()` (the package's `describe commands`).
- 4: `crates/engine/src/model/limit.rs` (`Limit`: `snlt`, `fnet`, `fnlt`, `Pin::Start`, `Pin::Finish`, `alap`; SNET is `snet`, ASAP the default); readers and writers #1881, #1884, #1888, #1890, #1895, #1901, #1902, #1903, #1904; kept in a saved document, `crates/app/src/document/read/limit.rs` (#1900); set and undone in the app #1987; edited in the task dialog #1991. Reached through files, `chart.load({ document })` and the dialog; the plain `tasks` data has no constraint field (`packages/gantt/src/adapters/plain.schema.json`). Root README: a constraint with no date or of an unknown type is not applied.
- 5: `crates/engine/src/calendar.rs`; per-task calendar in `model.rs`; read from XER and MS Project files and from a document's `calendars`. The plain `tasks` data uses one calendar (`adapters/plain.ts`).
- 6: Baseline button and `chart.commands.setBaseline()`, `setBaselineFromFile()` (`packages/gantt/src/commands.ts`); DHTMLX planned dates open as the baseline #1979. One baseline; P6 baselines are not read (README, "Not yet").
- 7, 8, 9: docs/JOURNEYS.md, journeys J3, J8; `model.rs` ("Zero makes a milestone"); the site Features page (`apps/site/features.html`).
- 11: resource filter `{ resource }` (`packages/gantt/src/commands.ts`); names are display-only (`crates/app/src/resources.rs`).
- 14: 14 counts in `chart.commands.health()` (the package's `describe commands`; docs/JOURNEYS.md J4); Show buttons #1985, #1986.
- 15: `chart.commands.whatIf`, `whatIfLonger`, `explain` (pkg README).
- 16: Compare with a file, `setBaselineFromFile` (docs/JOURNEYS.md J3).
- 17: `packages/gantt/src/task-dialog.ts` (name, duration, start, finish, percent, actual dates, remaining, SNLT, FNET, FNLT; one undo step) #1909; opened by `Shift+Enter`, a double-click on a bar or an uneditable cell #1918; limits #1991. It has no links, resources or notes fields.
- 18: `packages/gantt/src/cell-edit.ts` (8 fields per the 2026-11-12 dialog plan).
- 19: bar drag and Alt+drag link (docs/KEYS.md).
- 20: right (finish) edge sets the duration, one undo step #1896, #1887, #1913; `Gesture::ResizePressed` in `crates/app/src/press/gesture.rs`. The left edge does not resize.
- 21: `chart.commands.undo()`, `redo()` (`commands.ts`); `crates/app/src/history.rs`.
- 23 to 26: pkg README "Load a P6 or MS Project file" (`chart.load`, `exportXer`, `exportMspdi`); all four run in the browser, and the no-upload test (#1877) checks that the page sends no data.
- 27: `chart.exportXlsx()` (`packages/gantt/src/gantt-types.ts`, `xlsx.ts`) #1905, #1907, #1908, #1919.
- 28, 30: `chart.printOn("letter" | "a4")`; page header, page number and legend #1940; print demo to a two-page PDF #1957. The PDF is saved by the browser's print dialog.
- 29: `chart.exportImage({ page })`, any printed page, 2112 by 1632 pixels #2000.
- 31: `fromDhtmlx`, `fromBryntum` (pkg README).
- 32: root README speed table (Bryntum 8 Community, DHTMLX 10 Community and SVAR at 10,000 and 50,000 tasks, measured 2026-11-13) #1958.
- 33: `book/src/j2-navigate.md`; `chart.commands.zoomIn`, `zoomOut`, `fit`.
- 34: 34 color tokens in light and dark (the package's `describe tokens`).
- 35: #2001 draws a bar in its own color, #2008 keeps a task's `color` and `label` in the schedule document that `chart.load` reads, #2012 takes a `color` (`#rrggbb`) on a task given to `fromTasks`, and #2016 adds `label` there and `color` to `fromDhtmlx` (pkg README; `docs/WASM_API.md`, `PlainTask` and `App.save`).
- 37: docs/KEYS.md.
- 38: the task list is an ARIA tree grid (`packages/gantt/src/table-aria.ts`); the timeline canvas is one `role="img"` (`packages/gantt/src/chart.html`). The bars are spoken through the `card-live` live region when the selection moves: dates, float, critical state, percent complete, link counts and baseline start (`packages/gantt/src/bar-speech.ts`) #2014, #2017. The wording was not tried with a screen reader.
- 39: `locale` and `texts` mount options (`gantt-types.ts`) #1906, #1925, #1929, #1934, #1941, #1943, #1945; month and weekday names #1911, #1980; week start #1930; passed by the React, Vue and Angular components #1983; host fonts for other scripts #1992, #1999.
- 40: `dirOf` sets `dir="rtl"` for Arabic, Hebrew, Persian, Urdu, Pashto, Sindhi, Yiddish, Sorani, Dhivehi and Uyghur (`packages/gantt/src/locale.ts`) #1939, #1948, #1950, #1951, #1952, #1953. The table, toolbar and panels mirror; the timeline stays left to right.
- 41: `packages/gantt/package.json` `types` for every export.
- 42: the `react`, `vue` (#1882, #1898) and `angular` (#1891) exports in `packages/gantt/package.json`.
- 43: the `headless` export, `openHeadless` (pkg README "Tools for an AI assistant").
- 44: `toolDefinitions`, `callTool`, `propose` (`packages/gantt/src/ai-tools.ts`, pkg README).
- 45: the `assistant` mount option (`packages/gantt/src/gantt-types.ts`, `packages/gantt/src/assistant/panel.ts`, pkg README "Tools for an AI assistant"); the chart has the panel and the host supplies the `send` that calls its own model.
- 46: `packages/gantt/AGENTS.md`; `apps/site/llms.txt`; the package's `init --skill` command.
- 47, 48: `LICENSE` (MIT); `packages/gantt/package.json` `"license": "MIT"`; decision 0014 (#1964); license key, trial and watermark removed #1959.

## Vendor sources

**Bryntum**
- Feature table and tier names, (15): https://bryntum.com/products/gantt/features/ (rows 3, 4, 5, 6, 10, 11, 12, 17 to 22, 23, 25, 27 to 30, 34 to 36, 39, 42, 45; "Custom task rendering" in every tier; P6, histogram, RTL and accessibility are not in the table)
- Store, (15): https://bryntum.com/store/gantt/ (row 47, 48: Community MIT including commercial use; Core $699 and Enterprise $1,100 per developer per year; OEM by quote)
- Examples index, (15): https://bryntum.com/products/gantt/examples/ (row 24 "Import MS Project & Primavera P6 files"; row 13 "Resource histogram widget", "Resource utilization widget")
- Localization, (15): https://bryntum.com/products/gantt/docs-llm/guide/Gantt/customization/localization.md (row 39, 45 locales; row 40, no RTL guide, only a `localeRtl` key in the locale example)
- Scheduling engine: https://bryntum.com/products/gantt/docs-llm/engine/gantt_tasks_scheduling.md (rows 1, 2, 4)
- PDF and PNG export: https://bryntum.com/products/gantt/docs-llm/api/Gantt/feature/export/PdfExport.md ("A server-side service is required") (rows 28, 29)
- Accessibility: https://bryntum.com/products/gantt/docs-llm/guide/Gantt/advanced/a11y.md (rows 37, 38)
- AI: https://bryntum.com/products/gantt/docs-llm/api/Gantt/feature/AI.md; https://bryntum.com/llms.txt (rows 44 to 46)

**DHTMLX**
- Edition table, (15): https://docs.dhtmlx.com/gantt/llms-md/en/guides/editions-comparison.md (every "Both" and "PRO only" mark; Community is MIT, "Previous free versions (v9.x and earlier) were distributed under the GPL v2 license")
- Pricing, (15): https://dhtmlx.com/docs/products/dhtmlxGantt/pricing.shtml (row 48: Individual $799, Commercial $1,599, Enterprise $2,999, Ultimate $5,999, perpetual; Gantt Community under MIT; row 43, the separate Planning Engine)
- Primavera, (15): https://docs.dhtmlx.com/gantt/llms-md/en/guides/export-primavera.md (rows 24, 26: XER or XML in, XML out; online service or local module; edition not stated)
- Resources, (15): https://docs.dhtmlx.com/gantt/llms-md/en/guides/resource-management.md (rows 12, 13: "available only in the PRO edition"; assignments in the lightbox)
- Task colors, (15): https://docs.dhtmlx.com/gantt/llms-md/en/guides/colouring-tasks.md (rows 35, 36: `color` property and `task_class` template; no edition stated)
- Accessibility, (15): https://docs.dhtmlx.com/gantt/llms-md/en/guides/accessibility.md (rows 37, 38)
- MS Project and common export: https://docs.dhtmlx.com/gantt/llms-md/en/guides/export-msproject.md, https://docs.dhtmlx.com/gantt/llms-md/en/guides/export-common.md (online service: 10 MB, 20 s; a module you host removes the limits)
- Print: no print method in https://docs.dhtmlx.com/gantt/llms-md/en/guides/export.md (row 30)
- Performance: https://docs.dhtmlx.com/gantt/llms-md/en/guides/performance.md (row 32)
- AI and frameworks: https://docs.dhtmlx.com/gantt/llms.txt (rows 42, 44 to 46)

**Syncfusion**
- Pricing, (15): https://www.syncfusion.com/sales/products (row 48, $959 per developer per year)
- Community License, (15): https://www.syncfusion.com/products/communitylicense (rows 47, 48: under $1M revenue, 5 or fewer developers, 10 or fewer employees; not described as open source)
- Scheduling, (15): https://help.syncfusion.com/gantt-sdk/react/gantt-chart/task-scheduling (rows 2, 5: project working time only)
- Localization and RTL, (15): https://help.syncfusion.com/gantt-sdk/react/gantt-chart/global-local (rows 39, 40)
- Taskbar, (15): https://help.syncfusion.com/gantt-sdk/react/gantt-chart/taskbar (rows 35, 36: `queryTaskbarInfo`, `taskbarTemplate`)
- PNG request: https://www.syncfusion.com/feedback/53120/ability-to-export-gantt-chart-as-image-jpef-png (row 29)
- Other pages under https://help.syncfusion.com/gantt-sdk/react/gantt-chart/: `task-dependency` (1), `critical-path` (3), `task-constraints` (4), `baseline` (6), `resource-view` (11 to 13, 22), `managing-tasks/editing-tasks` (17 to 20), `undo-redo` (21), `managing-tasks/splitting-and-merging-tasks` (10), `pdf-export/pdf-export` (28), `excel-export/excel-export` (27), `accessibility` (37, 38)
- MS Project: https://www.syncfusion.com/blogs/post/microsoft-project-files-in-javascript-gantt-chart.md (rows 23, 25)
- Benchmark (Syncfusion's own harness): https://www.syncfusion.com/blogs/post/react-gantt-chart-performance-benchmark.md (row 32)
- AI, (15): https://ai.syncfusion.com/llms.txt (rows 44, 46: agent skills and MCP servers for documentation retrieval; no tool that acts on a chart)

## Not checked, as of 2026-11-15

- Bryntum P6 import tier or mechanism (searched: features table, examples index): one example only.
- Bryntum right-to-left guide (searched: localization guide, Widget API page (first 100,000 characters), web search).
- DHTMLX edition of task colors and templates (searched: colouring-tasks guide, edition table).
- Open issue or merged PR for split tasks, grouping, resource assignment editing, a workload view or bar templates (searched: pull requests merged 2026-11-12 to 2026-11-15, 37 open issues). The workload view has a plan in #1875, not code.
- Vendor accessibility and benchmark figures are each vendor's own claims, not audited.
