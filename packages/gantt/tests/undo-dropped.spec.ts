import { expect, test, type Gantt } from "./support/gantt";

// gantt#2087: Ctrl+Z drops a step the engine refuses and says so once in the alert line; a press the engine or the view refuses shows its own reason.
test.use({ webgl: true });

const NOTICE = "A step could no longer be undone or redone, so the chart dropped it from the list.";

/** `inserts` (2 or more) inserts, an indent, an insert and an outdent: the host then gives a row a child with no undo step, so undoing that row's insert is refused. */
async function build(g: Gantt, inserts: number, tag: string): Promise<void> {
  await g.page.evaluate(([count, key]) => {
    const app = window.gantt!.app;
    const selected = () => (JSON.parse(app.status()) as { selected: { task: number } }).selected.task;
    app.select_task(5);
    for (let i = 0; i < Number(count); i += 1) app.insert_task();
    app.indent_selected();
    app.insert_task();
    app.assign_task_key(selected(), `parent${key}`);
    app.outdent_selected();
    app.add_task(JSON.stringify({ key: `child${key}`, name: "Child" }), `parent${key}`, null, false);
  }, [inserts, tag] as const);
}

async function refuseOneStep(g: Gantt, inserts: number): Promise<void> {
  await g.open({ n: 60 });
  await build(g, inserts, "");
  await g.page.locator("#timeline").click({ position: { x: 20, y: 400 } });
}

const press = async (g: Gantt) => {
  await g.page.keyboard.press("Control+z");
  await g.nextFrame(3);
};

test("a step the engine refuses is dropped by Ctrl+Z, the notice shows once, and the next Ctrl+Z undoes the step below", async ({ g }) => {
  await refuseOneStep(g, 4);
  const alert = g.page.locator("#edit-error");
  for (let i = 1; i <= 3; i += 1) {
    await press(g);
    await expect(alert, `press ${i} undoes a step and says nothing`).toBeHidden();
  }
  const before = (await g.status()).n;
  await press(g);
  expect((await g.status()).n, "the refused step is dropped and the step below it is undone").toBe(before - 1);
  await expect(g.page.locator("#edit-error-text")).toHaveText(NOTICE);
  await press(g);
  expect((await g.status()).n, "the next press undoes the step below").toBe(before - 2);
  await expect(alert, "the notice does not show again").toBeHidden();
});

test("two presses in a row that each drop a step and undo one show the notice after each", async ({ g }) => {
  await g.open({ n: 60 });
  await build(g, 3, "a");
  // Its top three steps undone, the first block leaves [good, refused]; the second block builds [good, refused, ...] above it.
  await g.page.evaluate(() => { for (let i = 0; i < 3; i += 1) window.gantt!.app.undo(); });
  await build(g, 3, "b");
  await g.page.locator("#timeline").click({ position: { x: 20, y: 400 } });
  for (let i = 0; i < 3; i += 1) await press(g);
  const notice = g.page.locator("#edit-error-text");
  await press(g);
  await expect(notice, "the first drop is said").toHaveText(NOTICE);
  await press(g);
  await expect(notice, "the second drop, worded the same, is said too").toHaveText(NOTICE);
  await g.page.evaluate(() => { const app = window.gantt!.app; app.select_task(5); app.insert_task(); app.insert_task(); app.indent_selected(); app.set_critical_only(true); });
  await press(g);
  await expect(notice, "a press the filter refuses shows its reason, not the last report").toHaveText("Clear the row filter or window to indent, outdent or move a row.");
});
