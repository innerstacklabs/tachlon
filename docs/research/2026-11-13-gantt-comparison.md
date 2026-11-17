# Our chart against Bryntum 8, DHTMLX and SVAR at 10,000 and 50,000 tasks (gantt#1949)

Measured 2026-11-13 on the Mac mini (Apple M4, 16 GB, macOS 26.7), headed Chrome for Testing 156.0.8078.4 on the built-in display at 60 Hz, 1280 by 800, `demos/bench/run.ts` at `c1a136d9` (the Bryntum run: `b3d2fce8`, same files for the driver, phases and Bryntum side). The plan was 10 runs per cell; every figure is the median of 4 matched pairs, the number the harness requires (the pair's two runs have a load1 mean within 0.5, no swap growth, no warnings; mini load1 1.8 to 3.3), each run a fresh browser. Seed 20261020, shapes `tree4` and `chain`, the same generated schedule for every side (`dataset.json` of the cell, SHA-256 in each result). Raw results: the three `results-*` directories on the mini (`~/bench-tl1949bm`); the table below is condensed from `demos/dhtmlx-compare/table.ts` over them.

What each rival is, and what that does to the comparison:

- **Bryntum Gantt 8.0.1 Community** (`@bryntum/gantt-community`, MIT, npm): its scheduling engine is on, but it has no working calendar and no constraints, so its dates differ from ours. It stands for "Bryntum 8" here; the Core and Enterprise tiers need a license and are not measured.
- **DHTMLX Gantt 10.0.3 community** (`dhtmlx-gantt`, MIT): no scheduling. It draws the dates it is given and moves one task on a drag.
- **SVAR React Gantt 2.8.2** (`@svar-ui/react-gantt`): no scheduling in the free edition, as DHTMLX.
- **Not measured:** DHTMLX Gantt PRO (it needs a trial sign-up), SVAR at `chain` (the run stopped at 10:20), frappe-gantt (no side in the harness yet; help wanted below).

Columns, in ms: data to ready (component made, then the schedule loaded, to the first correct frame), load (whole load), scroll p95 (vertical, horizontal), drag p95 (frame time while dragging the first leaf in view), drag commit (release to the side's commit, sampled once per frame, so about one frame, 16.7 ms, is the floor). Memory is the footprint of Chrome's processes (vmmap) 20 s after load. Scroll and drag frame p50 is 16.7 ms in all 96 runs, the display's frame period, so only p95 is shown. Our row is ours in the pair run against that rival, so it differs a little between pairs.

| cell | side | pairs | data to ready | load | scroll-y p95 | scroll-x p95 | drag p95 | drag commit | memory |
|---|---|---|---|---|---|---|---|---|---|
| chain-10000 | bryntum8 | 4 | 165.3 | 291.4 | 18.7 | 18.7 | 17.6 | 266.0 | 386.8 MB |
| chain-10000 | ours (vs bryntum8) | 4 | 107.8 | 138.4 | 18.4 | 18.7 | 18.7 | 16.8 | 387.7 MB |
| chain-50000 | bryntum8 | 4 | 690.4 | 819.8 | 18.7 | 18.6 | 17.8 | 1303.5 | 465.9 MB |
| chain-50000 | ours (vs bryntum8) | 4 | 307.3 | 337.8 | 17.7 | 18.7 | 18.6 | 18.8 | 420.1 MB |
| tree4-10000 | bryntum8 | 4 | 160.8 | 289.4 | 18.6 | 18.7 | 16.8 | 97.1 | 381.7 MB |
| tree4-10000 | ours (vs bryntum8) | 4 | 117.3 | 147.8 | 18.6 | 18.7 | 18.6 | 16.7 | 388.7 MB |
| tree4-50000 | bryntum8 | 4 | 648.1 | 773.6 | 18.6 | 18.7 | 17.2 | 263.7 | 445.3 MB |
| tree4-50000 | ours (vs bryntum8) | 4 | 291.7 | 322.2 | 18.3 | 18.7 | 18.7 | 17.8 | 420.1 MB |
| chain-10000 | dhtmlx | 4 | 142.3 | 174.6 | 18.7 | 18.7 | 18.6 | 31.8 | 348.4 MB |
| chain-10000 | ours (vs dhtmlx) | 4 | 109.7 | 141.7 | 18.4 | 18.7 | 18.7 | 15.8 | 389.5 MB |
| chain-50000 | dhtmlx | 4 | 752.6 | 783.8 | 18.7 | 18.7 | 18.3 | 49.3 | 405.6 MB |
| chain-50000 | ours (vs dhtmlx) | 4 | 299.8 | 332.1 | 18.5 | 18.7 | 18.7 | 15.7 | 421.2 MB |
| tree4-10000 | dhtmlx | 4 | 71.6 | 105.7 | 18.7 | 18.7 | 18.7 | 30.1 | 342.7 MB |
| tree4-10000 | ours (vs dhtmlx) | 4 | 118.3 | 148.7 | 18.6 | 18.7 | 18.6 | 15.4 | 390.0 MB |
| tree4-50000 | dhtmlx | 4 | 301.9 | 337.0 | 18.7 | 18.7 | 18.7 | 63.8 | 378.7 MB |
| tree4-50000 | ours (vs dhtmlx) | 4 | 297.4 | 327.3 | 18.3 | 18.7 | 18.6 | 18.1 | 422.1 MB |
| tree4-10000 | ours (vs svar) | 4 | 111.2 | 139.8 | 18.6 | 18.7 | 18.6 | 16.4 | 391.1 MB |
| tree4-10000 | svar | 4 | 69.8 | 108.7 | 18.4 | 18.7 | 16.8 | 15.5 | 347.7 MB |
| tree4-50000 | ours (vs svar) | 4 | 300.4 | 329.2 | 18.4 | 18.7 | 18.7 | 17.9 | 419.2 MB |
| tree4-50000 | svar | 4 | 222.3 | 261.5 | 18.4 | 18.7 | 16.8 | 15.9 | 393.0 MB |

Reading it:

- **Where we are ahead:** against Bryntum 8 Community, data to ready at 50,000 tasks is 2.2 times faster (648 to 690 ms against 292 to 307; the whole load 2.4 times) and 1.4 to 1.5 times at 10,000; the commit after a drag, with its engine rescheduling what follows, is 97 to 1,304 ms against about one frame (15.4 to 18.8 ms) for us.
- **Where we are not ahead:** scroll and drag frames are a tie in every cell (the 60 Hz display caps them; Bryntum's drag p95 is 16.8 to 17.8 ms against our 18.6 to 18.7, 0.8 to 1.8 ms lower in all 4 cells, and SVAR's is 16.8 against our 18.6 to 18.7 in both cells). DHTMLX community is faster to load at `tree4` 10,000 (72 ms against 118) and level at `tree4` 50,000 (302 against 297); SVAR is faster to load at `tree4` 10,000 (70 against 111) and 50,000 (222 against 300); both use 16 to 47 MB less memory than ours in every cell measured; at `chain` 50,000 we load 2.5 times faster than DHTMLX. Our memory is higher than Bryntum's by 0.9 and 7.0 MB at 10,000 and 25 to 46 MB lower at 50,000.
- **Two of the three do not schedule.** DHTMLX community and SVAR do not reschedule, so their drag commit is only the store write; against them our win is the engine, not the speed. The one scheduling rival measured has no calendars or constraints. Whether the lead is at least 2 times against Bryntum 8 and DHTMLX PRO is answered for Bryntum 8 Community at 50,000 tasks (2.2 times at data to ready) and not for PRO.

Reproduce (per rival; the sides are installed from public npm into scratch, nothing of theirs is committed; the setup, adapter and `sides.json` steps are in `docs/MEASURING.md`):

```sh
node demos/bench/run.ts --sides sides-<rival>.json --cells tree4:10000,tree4:50000,chain:10000,chain:50000 --pairs 6 --max-load 4 --out results-<rival>
node demos/dhtmlx-compare/table.ts results-<rival>
```

Help wanted: run the same on `chain` for SVAR (`svar-setup.ts`, `svar-adapt.ts`, the `sides.json` above), and add a frappe-gantt side (install `frappe-gantt` from npm, a page that loads the cell, a probe like `sides/svar.ts`). Each rival at 10 runs instead of 4 matched pairs would narrow the spread.
