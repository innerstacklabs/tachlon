import { expect, test } from "./support/gantt";
import { fromTasks } from "../src/adapters/plain";

// A host rebuilds a deleted task's links from the `edit` event. Each task's `id` differs from its `key`, so a link record that
// named its ends by id would send the host looking for the wrong row: the ends in `was` are the keys, and `cascade` marks the
// links the task delete removed (decision 0022, section 2).

interface Change {
  record: string;
  op: string;
  key: string | null;
  was?: { from: string; to: string };
  cascade?: true;
}

test("a deleted task's links name their ends by key, and the ones it removed are marked cascade", async ({ g }) => {
  await g.open({ n: 20, params: { backend: "webgl" } });
  const tasks = ["dig", "pour", "cure", "seal"].map((name, i) => ({ id: `t${i + 1}`, key: `k-${name}`, name, start: "2026-03-02", duration: 2 }));
  const links = [{ from: "t1", to: "t2" }, { from: "t2", to: "t3" }, { from: "t3", to: "t4" }];
  await g.page.evaluate((document) => window.gantt!.api!.load({ document }), fromTasks({ tasks, links }).document);
  await g.nextFrame(3);
  await g.page.evaluate(() => {
    const w = window as unknown as { __edits: { commits: { changes: unknown[] }[] }[] };
    w.__edits = [];
    window.gantt!.on("edit", (detail: unknown) => w.__edits.push(detail as never));
  });
  await g.page.evaluate(() => (window.gantt!.app.select_task(1), window.gantt!.app.delete_selected()));
  await expect.poll(async () => (await g.status(), await g.page.evaluate(() => (window as unknown as { __edits: unknown[] }).__edits.length))).toBe(1);
  const changes = (await g.page.evaluate(() => (window as unknown as { __edits: { commits: { changes: unknown[] }[] }[] }).__edits[0]!.commits.flatMap((c) => c.changes))) as Change[];
  const ends = (c: Change) => [c.op, c.was!.from, c.was!.to, c.cascade];
  expect(changes.filter((c) => c.record === "task").map((c) => [c.op, c.key])).toEqual([["delete", "k-pour"]]);
  expect(changes.filter((c) => c.record === "link").map(ends)).toEqual([["delete", "k-dig", "k-pour", true], ["delete", "k-pour", "k-cure", true]]);
});
