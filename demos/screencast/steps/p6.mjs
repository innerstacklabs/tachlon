// The P6 steps of the README story (gs#393): open a P6 file that has progress, filter to one value of an activity code, change a link's type
// and lag, and press Show moved on that edit. They run on fixtures/fit-out.xer (built by fixtures/build-fit-out.mjs): a made-up office fit-out of
// 26 activities five weeks in, with actual dates, a data date, links of three kinds and one activity code, "Trade". The hosted
// samples have no actuals and no codes (the MS Project sample ignores them), so the story brings its own file, which `open-p6`
// opens through the page's Open button like a person would. On the MacBook only demos/screencast/ is copied; the file lives in it.
import { fileURLToPath } from "node:url";
import { acceptOpenAsk, barAt, openFilters } from "./helpers.mjs";

const FIT_OUT = fileURLToPath(new URL("../fixtures/fit-out.xer", import.meta.url));

/** The activities the steps name, and the link they edit: HVAC ductwork -> Fire sprinkler drops, start to start 8 days, to finish to start 2 days. */
export const PLAN = { pred: "HVAC ductwork", succ: "Fire sprinkler drops", from: "SS+8d", to: "FS+2d", trade: "Trade", value: "Mechanical", rows: 33, dataDate: "2026-10-26" };

/** The row of the task named `name` in the rows now listed, or an error naming it. */
async function rowOf(ctx, name) {
  const row = await ctx.page.evaluate((wanted) => {
    const app = window.gantt.app;
    const rows = JSON.parse(app.table_rows(0, window.gantt.status().rowCount));
    return rows.findIndex((r) => app.task_details(r[0])?.name === wanted);
  }, name);
  if (row < 0) throw new Error(`p6 steps: the schedule has no task named "${name}" in its rows`);
  return row;
}

/** Wheel the table sideways until its scroll is at `left` px, `frames` events at one a frame, like a person scrolling it. */
async function scrollTable(ctx, left, frames) {
  const box = await ctx.page.locator("#table-body").boundingBox();
  const { from, max } = await ctx.page.evaluate(() => {
    const body = document.getElementById("table-body");
    return { from: body.scrollLeft, max: body.scrollWidth - body.clientWidth };
  });
  // The table cannot scroll past its end: since gs#821 the Task column takes the spare width, so a column's left edge
  // can sit beyond the last scroll position. Aim at the furthest the table goes.
  left = Math.max(0, Math.min(left, max));
  await ctx.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let k = 0; k < frames; k++) {
    await ctx.page.mouse.wheel((left - from) / frames, 0);
    await ctx.frames(1);
  }
  await ctx.until(({ want }) => Math.abs(document.getElementById("table-body").scrollLeft - want) <= 2, { want: left });
}

export const openP6 = {
  name: "open-p6",
  journey: "J1",
  caption: "Open a P6 file: actual dates and the data date apply",
  run: async (ctx) => {
    const previous = await ctx.page.evaluate(() => window.gantt.opened?.name ?? null);
    await ctx.ring("#open-button", { ms: 1000, pulses: 1 });
    acceptOpenAsk(ctx.page);
    await ctx.page.locator("#open-input").setInputFiles(FIT_OUT);
    await ctx.until((was) => {
      const opened = window.gantt.opened;
      return opened !== undefined && opened.name !== was && window.gantt.status().n === opened.tasks;
    }, previous);
    const opened = await ctx.page.evaluate(() => window.gantt.opened);
    if (opened.name !== "fit-out.xer" || opened.tasks !== PLAN.rows) {
      throw new Error(`open-p6 step: opened ${opened.name} with ${opened.tasks} rows, not fit-out.xer with ${PLAN.rows}`);
    }
    // The data date is read, and the file's progress is on the page: the toolbar names the date and the table has the progress columns.
    // `window.gantt.opened` is set when the open ends; the toolbar line and the table head follow at the next frame, which on a loaded
    // machine is not yet drawn when the wait above returns (gs#1125). This waits for the state the check below reads; a wait that
    // runs out is not an error here, the check below then fails by its own name with what the page shows.
    await ctx.page
      .waitForFunction(
        (dataDate) =>
          (document.getElementById("data-date")?.textContent ?? "").includes(dataDate) &&
          ["actual-start", "actual-finish", "remaining", "percent"].every((name) => document.querySelector(`#table-head .${name}`) !== null),
        PLAN.dataDate,
        { timeout: 60_000 },
      )
      .catch(() => {});
    const facts = await ctx.page.evaluate(() => ({
      dataDate: document.getElementById("data-date")?.textContent ?? "",
      columns: ["actual-start", "actual-finish", "remaining", "percent"].filter((name) => document.querySelector(`#table-head .${name}`) !== null),
    }));
    if (!facts.dataDate.includes(PLAN.dataDate) || facts.columns.length !== 4) {
      throw new Error(`open-p6 step: the toolbar reads "${facts.dataDate}" and the table has the progress columns ${JSON.stringify(facts.columns)}, not the data date ${PLAN.dataDate} and four columns`);
    }
    // The page notes what it left out of a P6 file (every real file has scheduling options it does not read) as a one-line
    // "N import notes" button in the toolbar row; the list stays folded, so nothing covers the chart (gs#818). Point at the button.
    await ctx.until(() => document.getElementById("import-notes")?.hidden === false && document.getElementById("unsupported")?.hidden === true);
    await ctx.ring("#import-notes", { ms: 800, pulses: 1 });
    // Fit the whole project on the chart, as large as it goes: the first bar near the left edge, then Ctrl+wheel in at that bar until
    // the project finish is at 95% of the chart's width. A wheel step of 30 changes the scale by a factor of 0.94 (measured on
    // this page), so the step that gets there is sized from the ratio, and a second corrects the rest.
    const box = await ctx.canvasBox();
    // The first row's bar is not the project's first start in a file with progress (a summary draws from the data date), so the
    // start is the earliest of the rows' starts.
    const [first, finish] = await ctx.page.evaluate(() => {
      const app = window.gantt.app;
      const starts = Array.from({ length: window.gantt.status().rowCount }, (_, row) => app.row_start(row)).filter((minutes) => minutes !== undefined);
      return [Math.min(...starts), app.test_project_finish()];
    });
    const ends = () => ctx.page.evaluate(([a, b]) => [window.gantt.app.x_of(a), window.gantt.app.x_of(b)], [first, finish]);
    await ctx.page.mouse.move(box.x + box.width / 2, box.y + box.height * 0.6); // the wheel pans the chart only with the pointer over it
    for (let i = 0; i < 10; i++) {
      const dx = (await ends())[0] - box.width * 0.03;
      if (Math.abs(dx) < 1) break;
      await ctx.page.mouse.wheel(dx, 0);
      await ctx.frames(3);
    }
    await ctx.page.mouse.move(box.x + box.width * 0.03, box.y + box.height / 2);
    // A zoom starts a few frames after the wheel event and glides to its target, so a step waits for the finish to start moving,
    // then to hold still for 6 frames.
    const zoomBy = async (delta) => {
      const was = (await ends())[1];
      await ctx.page.keyboard.down("Control");
      try {
        await ctx.page.mouse.wheel(0, delta);
      } finally {
        await ctx.page.keyboard.up("Control");
      }
      for (let i = 0; i < 240 && (await ends())[1] === was; i++) await ctx.frames(1);
      let last = Number.NaN;
      let still = 0;
      for (let i = 0; i < 480 && still < 6; i++) {
        const x = (await ends())[1];
        still = x === last ? still + 1 : 0;
        last = x;
        await ctx.frames(1);
      }
      if (still < 6) throw new Error("open-p6 step: the zoom did not come to rest");
    };
    for (let i = 0; i < 3; i++) {
      const [x0, x1] = await ends();
      const delta = Math.log((box.width * 0.95 - x0) / (x1 - x0)) / (Math.log(0.94) / 30);
      if (Math.abs(delta) < 8) break;
      await zoomBy(Math.max(-400, Math.min(400, delta)));
    }
    const [fitted0, fitted1] = await ends();
    if (fitted0 < 0 || fitted1 > box.width) throw new Error(`open-p6 step: the project does not fit the chart (x ${fitted0.toFixed(0)} to ${fitted1.toFixed(0)} of ${box.width.toFixed(0)})`);
    // Park the pointer on empty chart, so no hover card covers a row.
    await ctx.page.mouse.move(box.x + box.width * 0.85, box.y + ctx.AXIS + ctx.ROW * 3);
    await ctx.hold(1.5);
    // The progress columns sit to the right of the dates; the table scrolls sideways under the frozen ID, WBS and Task.
    const { actual, end } = await ctx.page.evaluate(() => {
      const body = document.getElementById("table-body");
      const head = document.querySelector("#table-head .actual-start");
      const frozen = document.querySelector("#table-head .name").getBoundingClientRect().right;
      const rect = head.getBoundingClientRect();
      return { actual: Math.round(rect.left + body.scrollLeft - frozen), end: body.scrollWidth - body.clientWidth };
    });
    await ctx.setCaption("The table scrolls sideways; ID, WBS and Task stay put");
    await scrollTable(ctx, actual, Math.round(ctx.fps * 0.5));
    await ctx.hold(1.2);
    await ctx.setCaption("Actual start and finish, remaining, % complete");
    await scrollTable(ctx, end, Math.round(ctx.fps * 0.4));
    await ctx.hold(1.5);
    await scrollTable(ctx, 0, Math.round(ctx.fps * 0.5));
    await ctx.hold(0.6);
  },
};

const CODE_CAPTION = `Filter by activity code: ${PLAN.trade} = ${PLAN.value}`;

export const codeFilter = {
  name: "code-filter",
  journey: "J2",
  caption: CODE_CAPTION,
  pre: 'document.getElementById("code-filter")?.hidden === false', // the file's codes put the choice in the toolbar
  run: async (ctx) => {
    const before = await ctx.status();
    await openFilters(ctx.page); // the activity code choice sits in the Filters menu (gs#808)
    await ctx.ring("#code-type", { ms: 1000, pulses: 1 });
    await ctx.page.locator("#code-type").selectOption({ label: PLAN.trade });
    await ctx.until(() => document.getElementById("code-value")?.hidden === false);
    await ctx.hold(0.8);
    await openFilters(ctx.page); // choosing a type moves focus to the chart, which shuts the menu
    await ctx.page.locator("#code-value").selectOption({ label: PLAN.value });
    await ctx.until(() => window.gantt.status().filter === "code");
    await ctx.until((was) => window.gantt.status().rowCount < was, before.rowCount);
    const kept = (await ctx.status()).rowCount;
    // The count joins the caption while the filter is on and leaves it before Clear, so it never sits over the whole table; the page's own
    // label says the same, "· 9 of 33 rows".
    await ctx.extendCaption(`(${kept} of ${before.rowCount} rows)`);
    await ctx.ring("#row-count", { ms: 1000, pulses: 1 });
    await ctx.hold(2.2);
    await ctx.setCaption(CODE_CAPTION);
    await openFilters(ctx.page); // so does choosing the value
    await ctx.page.locator("#code-clear").click();
    await ctx.until((n) => window.gantt.status().filter === "off" && window.gantt.status().rowCount === n, before.rowCount);
    await ctx.hold(0.6);
    return { kept, of: before.rowCount };
  },
};

export const editLink = {
  name: "edit-link",
  journey: "J6",
  caption: `Change a link's type and lag: ${PLAN.from} to ${PLAN.to}`,
  run: async (ctx) => {
    await ctx.page.evaluate(() => {
      document.getElementById("table-body").scrollTop = 0;
      window.gantt.app.clear_selection();
    });
    await ctx.until(() => window.gantt.status().scrollY === 0 && window.gantt.status().selected === null);
    const box = await ctx.canvasBox();
    const row = await rowOf(ctx, PLAN.succ);
    // The press lands 6 px before the successor's bar, just above its row's middle: on the link's last vertical run, in air.
    const arrow = await barAt(ctx, box, row, -6, -8);
    await ctx.page.mouse.move(arrow.x, arrow.y, { steps: 20 });
    await ctx.hold(0.4);
    await ctx.page.mouse.click(arrow.x, arrow.y);
    await ctx.until(() => window.gantt.status().link.selected !== null);
    const link = (await ctx.status()).link.selected;
    const text = await ctx.page.evaluate((dep) => window.gantt.app.link_text(dep), link);
    if (text !== PLAN.from) throw new Error(`edit-link step: the press selected a link that reads "${text}", not the ${PLAN.pred} -> ${PLAN.succ} link at ${PLAN.from}`);
    await ctx.hold(1);
    await ctx.page.keyboard.press("Enter");
    await ctx.until(() => document.getElementById("link-edit")?.hidden === false && document.activeElement?.id === "link-edit-input");
    await ctx.hold(0.8);
    for (const character of PLAN.to) {
      await ctx.page.keyboard.type(character);
      await ctx.frames(4);
    }
    await ctx.hold(0.6);
    const revision = (await ctx.status()).revision;
    await ctx.page.keyboard.press("Enter");
    await ctx.until(({ dep, want }) => window.gantt.app.link_text(dep) === want, { dep: link, want: PLAN.to });
    await ctx.until((was) => window.gantt.status().revision > was && window.gantt.status().moved !== null, revision);
    const moved = (await ctx.status()).moved.text;
    // The page's note counts the tasks first ("16 tasks moved · finish +8 days · critical 1 joined, 0 left"); the caption keeps what a
    // scheduler reads next, the finish and the critical path, so it stays short enough to leave the bars in sight.
    const [, ...rest] = moved.split(" · ");
    await ctx.extendCaption(`(${(rest.length > 0 ? rest : [moved]).join(" · ")})`);
    await ctx.hold(2.5);
    return { moved };
  },
};

/**
 * The Show moved button (#795), pressed on the link edit's report: the task list keeps only the tasks the edit moved, with their
 * summaries, and the bars sit where they are now. On the 50,000-task schedule the same press changed
 * nothing a viewer could see: the drag's view is the bottom of the list, where every visible row had moved.
 */
export const showMoved = {
  name: "show-moved",
  journey: "J7",
  pre: 'window.gantt.status().moved !== null && window.gantt.status().filter === "off"', // the link edit's report is still the last one
  caption: "Show moved keeps only the tasks the edit moved",
  run: async (ctx) => {
    const before = await ctx.status();
    await ctx.until(() => document.getElementById("moved-show")?.hidden === false);
    await ctx.ring("#moved-show", { ms: 1000, pulses: 1 });
    await ctx.page.locator("#moved-show").click();
    await ctx.until(() => window.gantt.status().filter === "moved");
    await ctx.until((was) => window.gantt.status().rowCount < was, before.rowCount);
    const kept = (await ctx.status()).rowCount;
    await ctx.extendCaption(`(${kept} of ${before.rowCount} rows)`);
    await ctx.ring("#row-count", { ms: 1000, pulses: 1 });
    await ctx.hold(2.2);
    return { kept, of: before.rowCount };
  },
};
