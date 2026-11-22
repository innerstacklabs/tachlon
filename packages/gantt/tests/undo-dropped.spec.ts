import { expect, test, type Gantt } from "./support/gantt";

// gantt#2087: Ctrl+Z drops a step the engine refuses and says so once in the alert line; a press with no step left below it says the same.
test.use({ webgl: true });

const NOTICE = "A step could no longer be undone or redone, so the chart dropped it from the list.";

/** `inserts` (2 or more) inserts, an indent, an insert and an outdent: the host then gives a row a child with no undo step, so undoing that row's insert is refused. */
async function refuseOneStep(g: Gantt, inserts: number): Promise<void> {
  await g.open({ n: 60 });
  await g.page.evaluate((count) => {
    const app = window.gantt!.app;
    const selected = () => (JSON.parse(app.status()) as { selected: { task: number } }).selected.task;
    app.select_task(5);
    for (let i = 0; i < count; i += 1) app.insert_task();
    app.indent_selected();
    app.insert_task();
    app.assign_task_key(selected(), "parent");
    app.outdent_selected();
    app.add_task(JSON.stringify({ key: "child", name: "Child" }), "parent", null);
  }, inserts);
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

test("a press that drops a step and finds none below shows the same sentence", async ({ g }) => {
  await refuseOneStep(g, 2);
  for (let i = 0; i < 4; i += 1) await press(g);
  await expect(g.page.locator("#edit-error-text")).toHaveText(NOTICE);
});
