import { fromTasks, type PlainTask } from "../src/adapters/plain";
import { expect, test, type Gantt } from "./support/gantt";

// The task list's Predecessors column: shown for a schedule whose tasks have IDs, and a typed list such as `3, 5SS+1d` makes the links in one undo step.
// The schedule is our own: IDs "1" to "5" are leaves, "S" is a summary over "6".

const tasks: PlainTask[] = [1, 2, 3, 4, 5].map((id) => ({ id: String(id), name: `T${id}`, start: "2026-03-02", duration: 2 }));
tasks.push({ id: "S", name: "Phase", start: "2026-03-02", duration: 2 }, { id: "6", name: "T6", start: "2026-03-02", duration: 2, parent: "S" });
const links = [{ from: "1", to: "2" }];

const open = async (g: Gantt): Promise<void> => {
  await g.open({ n: 20 });
  await g.page.evaluate((d) => window.gantt!.api!.load({ document: d }), fromTasks({ tasks, links }).document);
  await g.nextFrame(3);
};
const row = (g: Gantt, name: string) => g.page.locator(`#table-rows .row:has(> .name:text-is("${name}"))`);
const count = async (g: Gantt, name: string): Promise<number> => {
  const slot = Number(await row(g, name).getAttribute("data-task"));
  return g.page.evaluate((t) => window.gantt!.app.task_details(t).predecessorCount, slot);
};
/** Put the cursor on the task's Predecessors cell the way a person does: from its Finish cell, Enter then Tab; then open the editor. */
async function openPred(g: Gantt, name: string): Promise<void> {
  await row(g, name).locator("> .finish").click();
  await g.nextFrame(2);
  await g.page.keyboard.press("Enter");
  await g.page.keyboard.press("Tab");
  await g.nextFrame(2);
  await g.page.keyboard.press("Enter");
}

test("a schedule whose tasks have no IDs shows no Predecessors column", async ({ g }) => {
  await g.open({ n: 20 });
  await expect(g.page.locator("#table-head .cols > .pred")).toHaveCount(0);
  await expect(g.page.locator("#table-rows .row > .pred")).toHaveCount(0);
});

test("with IDs the column shows each task's list; a typed list makes the links, and Undo removes both in one step", async ({ g }) => {
  await open(g);
  await expect(g.page.locator("#table-head .cols > .pred")).toHaveText("Predecessors");
  await expect(row(g, "T2").locator("> .pred")).toHaveText("1");
  await openPred(g, "T4");
  await g.page.keyboard.type("3, 5SS+1d");
  const stamp = (await g.status()).stamp;
  await g.page.keyboard.press("Enter");
  await expect(row(g, "T4").locator("> .pred")).toHaveText("3, 5 SS+1d");
  expect(await count(g, "T4"), "both links made").toBe(2);
  expect((await g.status()).stamp, "the chart redrew").toBeGreaterThan(stamp);
  await g.page.keyboard.press("ControlOrMeta+Z");
  await expect(row(g, "T4").locator("> .pred")).toHaveText("");
  expect(await count(g, "T4"), "one Undo removes both").toBe(0);
});

test("bad text keeps the cell open with the reason and changes nothing; a summary's cell takes no editor", async ({ g }) => {
  await open(g);
  await openPred(g, "T4");
  await g.page.keyboard.type("9");
  const stamp = (await g.status()).stamp;
  await g.page.keyboard.press("Enter");
  await expect(g.page.locator("#table-rows .cell-error")).toContainText("9");
  await expect(g.page.locator("#table-rows input.cell-edit")).toBeVisible();
  expect((await g.status()).stamp, "the schedule is unchanged").toBe(stamp);
  await g.page.keyboard.press("Escape");
  await openPred(g, "Phase");
  await expect(g.page.locator("#table-rows input.cell-edit")).toHaveCount(0);
});
