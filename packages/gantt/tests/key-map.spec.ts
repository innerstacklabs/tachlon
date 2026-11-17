import { readFileSync } from "node:fs";
import type { Status } from "../src/api";
import { expect, test, type Gantt } from "./support/gantt";
import { editDuration, openLedger } from "./support/moved-note";

// The key map (gs#1163): every key binding pressed once on one page, each checked by the one thing its command changes.
// The table is held to the code's own lists in two ways: every row of the keys list (`KEYS` in apps/chrome/src/keys-help.ts, read
// from the `?` dialog) must name a table row, and every key the chart's listener tests (`e.key === "..."` in
// packages/gantt/src/keys.ts) must be the key of a table row. A binding added to either list without a row here fails the first step.
// What a key does in a cell editor, the find box or the cell cursor, and the focus and layout a key moves, stay in the
// feature specs; the cell cursor's editing keys (cursor.ts) and the list divider's arrows (split.ts) are theirs too.
// The page is the ledger fixture with a baseline set and one edit that slips and moves tasks, so Shift+S and Shift+M act.

type Read = (g: Gantt) => Promise<unknown>;

interface Binding {
  /** What Playwright presses; the last `+`-separated part is the `e.key` that keys.ts tests. */
  key: string;
  /** The keys-list row this binding is, as the dialog shows it. */
  label?: string;
  /** The task selected before the press; null clears the selection; absent keeps it. */
  task?: number | null;
  /** More state the press needs, after the selection. */
  setup?: (g: Gantt) => Promise<unknown>;
  /** The one observable the command changes, read before and after the press. */
  read?: Read;
  /** The value `read` gives after the press; absent, it must differ from the value before. */
  want?: unknown;
  /** A browser event the press fires instead of a state change. */
  event?: "filechooser" | "download";
  /** Puts back what the press changed, for the rows after it. */
  reset?: (g: Gantt) => Promise<unknown>;
}

const status = (pick: (s: Status) => unknown): Read => async (g) => pick(await g.status());
const app = (code: string): ((g: Gantt) => Promise<unknown>) => (g) => g.page.evaluate(`window.gantt.app.${code}`);
const dom = (code: string): Read => (g) => g.page.evaluate(code);
const row = status((s) => s.selected?.row ?? null);
const rows = status((s) => s.rowCount);
const tasks = status((s) => s.n);
const range = status((s) => s.range);
const filter = status((s) => s.filter);
const link = status((s) => s.link.selected);
const view = status((s) => [s.t[0], s.scrollY]);
const span = dom(`(() => { const a = window.gantt.app; return a.time_at(document.getElementById("timeline").clientWidth) - a.time_at(0); })()`);
const column = dom(`document.querySelector('#table-rows [data-cursor="true"]')?.className ?? null`);
const outline = dom(`window.gantt.app.task_details(window.gantt.status().selected.task).outlinePath`);
const cell = (r: number, c: string) => (g: Gantt) => g.page.locator(`#table-rows .row[data-row="${r}"] > .${c}`).click();
const escape = (g: Gantt) => g.page.keyboard.press("Escape");

// In order: a row may rely on the state the rows above it leave (Alt+Right expands what Alt+Left collapsed, Delete removes
// the row Insert added, the undo keys walk that history).
const TABLE: Binding[] = [
  { key: "Control+f", label: "Ctrl+F", read: dom("document.activeElement?.id"), want: "find-input" },
  { key: "Control+o", event: "filechooser" },
  { key: "Control+s", event: "download" },
  { key: "Control+a", task: 5, read: range },
  { key: "c", label: "c", task: null, setup: app(`go_to_date("2030-01-01")`), read: view },
  { key: "Shift+C", label: "Shift+C", read: status((s) => s.criticalOnly), want: true, reset: app("set_critical_only(false)") },
  { key: "Shift+L", label: "Shift+L", read: status((s) => s.longestPath), want: true, reset: app("set_longest_path(false)") },
  { key: "Shift+S", read: filter, want: "slipped", reset: app("set_slipped_only(false)") },
  { key: "Shift+M", read: filter, want: "moved", reset: app("set_moved_only(false)") },
  { key: "2", read: rows, want: 3 },
  { key: "1", read: rows, want: 1 },
  { key: "0", read: rows, want: 43 },
  // `3` stands for `1` to `9`; the ledger has three levels, so from level 1 it opens all 43 rows.
  { key: "3", label: "1 to 9", setup: app("collapse_to_level(1)"), read: rows, want: 43 },
  { key: "Alt+ArrowLeft", label: "Alt+Left", task: 22, read: rows, want: 23 },
  { key: "Alt+ArrowRight", label: "Alt+Right", task: 22, read: rows, want: 43 },
  // The root of the ledger, with every summary collapsed: `*` opens all 43 rows, where Alt+Right opens its two children only.
  { key: "*", label: "*", task: 0, setup: app("collapse_to_level(1)"), read: rows, want: 43 },
  { key: "ArrowDown", task: 5, read: row, want: 6 },
  { key: "ArrowUp", task: 5, read: row, want: 4 },
  { key: "ArrowRight", task: 5, setup: cell(5, "name"), read: column },
  { key: "ArrowLeft", task: 5, setup: cell(5, "dur"), read: column },
  { key: "Home", task: 5, setup: cell(5, "dur"), read: column },
  { key: "End", task: 5, setup: cell(5, "name"), read: column },
  { key: "Control+End", task: 5, read: row, want: 42 },
  { key: "Control+Home", task: 5, read: row, want: 0 },
  { key: "Meta+ArrowDown", task: 5, read: row, want: 42 },
  { key: "Meta+ArrowUp", task: 5, read: row, want: 0 },
  { key: "PageDown", task: 5, read: row },
  { key: "PageUp", task: 30, read: row },
  { key: "Shift+ArrowDown", task: 5, read: range, want: [5, 6, 6] },
  { key: "Shift+ArrowUp", task: 5, read: range, want: [4, 5, 4] },
  { key: "Shift+Space", task: 5, read: range, want: [5, 5, 5] },
  { key: "]", task: 16, read: link },
  { key: "[", task: 16, read: link },
  { key: "Enter", task: 16, setup: app("step_link(true)"), read: dom(`document.getElementById("link-edit").hidden`), want: false, reset: escape },
  { key: "Alt+Enter", task: 16, setup: async (g) => {
      const live = g.page.locator("#card-live");
      await g.page.evaluate("window.gantt.app.clear_selection()");
      await g.nextFrame(2);
      await g.page.keyboard.press("ArrowRight");
      await expect(live).toHaveText("");
      await g.page.evaluate("window.gantt.app.select_task(16)");
      await expect(live).toContainText("Activity");
      await g.page.keyboard.press("ArrowRight");
      await expect(live).toHaveText("");
    }, read: dom(`document.getElementById("card-live").textContent`) },
  { key: "Shift+=", read: span },
  { key: "=", read: span },
  { key: "-", read: span },
  { key: "f", read: span },
  { key: "t", setup: app(`go_to_date("2030-01-01")`), read: view },
  { key: "s", label: "s", read: dom(`document.getElementById("status-line").hidden`) },
  { key: "?", read: dom(`document.getElementById("keys-help").open`), want: true, reset: escape },
  { key: "Shift+Enter", label: "Shift+Enter", task: 5, read: dom(`document.getElementById("task-dialog").open`), want: true, reset: escape },
  { key: "Escape", label: "Escape", task: 5, read: row, want: null },
  // The ledger's tasks are all linked, and a linked task cannot become a summary: the indent keys act on two inserted rows.
  { key: "Insert", label: "Insert", task: 5, read: tasks, want: 44 },
  { key: "Control+Enter", read: tasks, want: 45 },
  { key: "Alt+Shift+ArrowRight", read: outline },
  { key: "Alt+Shift+ArrowLeft", read: outline },
  { key: "Alt+Shift+ArrowUp", label: "Alt+Shift+Up", read: row },
  { key: "Alt+Shift+ArrowDown", label: "Alt+Shift+Down", read: row },
  { key: "Delete", label: "Delete", read: tasks, want: 44 },
  { key: "Control+z", label: "Ctrl+Z", read: tasks, want: 45 },
  { key: "Control+y", label: "Ctrl+Y", read: tasks, want: 44 },
  { key: "Control+Shift+z", setup: app("undo()"), read: tasks, want: 44 },
  { key: "Control+Backspace", task: 43, read: tasks, want: 43 },
];

/** The `e.key` a row presses: the last part of the chord, where a chord ending in `+` presses `+` itself. */
const eventKey = (key: string): string => (key.endsWith("+") ? "+" : key === "Shift+=" ? "+" : key.split("+").pop()!.replace("Space", " "));

test("every key binding, pressed once, runs its command", { tag: "@smoke" }, async ({ g }) => {
  test.setTimeout(180_000);
  await openLedger(g);
  await g.page.locator("#baseline-set").click();
  await editDuration(g.page, "20d");
  await g.nextFrame(3);

  await test.step("the table has a row for every binding the keys list and keys.ts name", async () => {
    await g.page.locator("#keys-button").click();
    const listed = await g.page.locator("#keys-help td:first-child").allTextContents();
    await g.page.keyboard.press("Escape");
    expect(listed.length, "the keys list has rows").toBeGreaterThan(5);
    for (const label of listed) expect(TABLE.map((b) => b.label), `a table row for the keys-list row ${label}`).toContain(label);
    const source = readFileSync(new URL("../src/keys.ts", import.meta.url), "utf8");
    const keys = new Set([...source.matchAll(/key(?:\.toLowerCase\(\))? === "([^"]+)"/g)].map((m) => m[1]).filter((k) => k !== "Alt"));
    expect(keys.size, "keys.ts tests keys").toBeGreaterThan(20);
    const pressed = new Set(TABLE.map((b) => eventKey(b.key)));
    for (const key of keys) expect([...pressed], `a table row presses ${JSON.stringify(key)}`).toContain(key);
  });

  for (const b of TABLE) {
    await test.step(`${b.key}${b.label ? ` (keys list: ${b.label})` : ""}`, async () => {
      // A press on the timeline puts the cell cursor away and gives the chart focus; then the row's selection and state.
      const keep = (await g.status()).selected?.task ?? null;
      await g.page.locator("#timeline").click({ position: { x: 20, y: 400 } });
      const task = b.task === undefined ? keep : b.task;
      await g.page.evaluate((t) => (t === null ? window.gantt!.app.clear_selection() : window.gantt!.app.select_task(t)), task);
      await g.nextFrame(2);
      await b.setup?.(g);
      await g.nextFrame(2);
      if (b.event !== undefined) {
        const fired = b.event === "download" ? g.page.waitForEvent("download", { timeout: 10_000 }) : g.page.waitForEvent("filechooser", { timeout: 10_000 });
        await g.page.keyboard.press(b.key);
        await fired;
        return;
      }
      const before = await b.read!(g);
      await g.page.keyboard.press(b.key);
      await g.nextFrame(3);
      const after = await b.read!(g);
      if (b.want !== undefined) expect(after, `${b.key}: the command ran`).toEqual(b.want);
      expect(after, `${b.key}: the observable changed`).not.toEqual(before);
      await b.reset?.(g);
    });
  }
});
