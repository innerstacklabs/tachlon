import { type Gantt, expect, test } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

// The Resources column, from an inline MS Project file (plain data cannot carry names yet): last in the list, names joined by commas, cut with an ellipsis.
const NAMES = ["Ana", "Ben", "Concrete crew C", "Concrete crew D", "Concrete crew E"];
const task = (uid: number, name: string, level = 1, summary = 0): string => `<Task><UID>${uid}</UID><Name>${name}</Name><OutlineLevel>${level}</OutlineLevel><Summary>${summary}</Summary><Duration>PT8H0M0S</Duration></Task>`;
const assign = (uid: number, count: number): string => NAMES.slice(0, count).map((_, i) => `<Assignment><TaskUID>${uid}</TaskUID><ResourceUID>${i + 1}</ResourceUID></Assignment>`).join("");
const FILE = Buffer.from(`<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-01-05T08:00:00</StartDate><Tasks>${task(1, "Five")}${task(2, "Two")}${task(3, "None")}${task(4, "Group", 1, 1)}${task(5, "Child", 2)}</Tasks><Resources>${NAMES.map((name, i) => `<Resource><UID>${i + 1}</UID><Name>${name}</Name></Resource>`).join("")}</Resources><Assignments>${assign(1, 5)}${assign(2, 2)}${assign(4, 1)}</Assignments></Project>`);

async function openFile(g: Gantt): Promise<void> {
  await g.open({ n: 50 });
  await g.page.setInputFiles("#open-input", { name: "resources.xml", mimeType: "application/xml", buffer: FILE });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1 && document.querySelector("#table-rows .row") !== null, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
}

/** The Resources cell of each row by task name, the head's words and right-most word, and whether every row has a grid track for each cell and the gutter. */
const read = (g: Gantt) =>
  g.page.evaluate(() => {
    const right = (el: Element): number => el.getBoundingClientRect().right;
    const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row")];
    const cells: Record<string, { text: string; title: string; cut: boolean; last: boolean }> = {};
    for (const row of rows) {
      const c = row.querySelector<HTMLElement>(":scope > .resources");
      if (c) cells[row.querySelector(".name")!.textContent!] = { text: c.textContent!, title: c.title, cut: c.scrollWidth > c.clientWidth && getComputedStyle(c).textOverflow === "ellipsis", last: [...row.children].every((x) => right(x) <= right(c) + 0.5) };
    }
    const heads = [...document.querySelectorAll("#table-head .cols > span")];
    const lastHead = heads.reduce((a, b) => (right(b) > right(a) ? b : a)).textContent;
    return { cells, heads: heads.map((h) => h.textContent), lastHead, aligned: rows.every((r) => getComputedStyle(r).gridTemplateColumns.split(" ").length === r.children.length + 1) };
  });

test("a schedule with names shows the Resources column last: the names, the whole list as the title, a long list cut", async ({ g }) => {
  await openFile(g);
  const list = await read(g);
  expect(list).toMatchObject({ lastHead: "Resources", aligned: true });
  expect(list.cells["Two"]).toEqual({ text: "Ana, Ben", title: "Ana, Ben", cut: false, last: true });
  expect(list.cells["Five"]).toMatchObject({ text: NAMES.join(", "), title: NAMES.join(", "), cut: true });
  expect(list.cells["Group"], "a summary row shows no names, though the file assigns it one").toMatchObject({ text: "", title: "" });
  const { x, y } = await g.box();
  const at = x + (await g.page.evaluate(() => window.gantt!.app.x_of(window.gantt!.app.row_start(0)!))) + 8;
  await g.page.mouse.move(at, y + AXIS + ROW / 2);
  await g.nextFrame(2);
  await g.page.mouse.down();
  for (let k = 1; k <= 9; k++) await g.page.mouse.move(at + 15 * k, y + AXIS + ROW / 2);
  await g.nextFrame(3);
  expect((await g.status()).drag?.active, "the first task's bar is being dragged").toBe(true);
  expect((await read(g)).cells["Two"], "every cell is empty while a bar is dragged").toMatchObject({ text: "", title: "" });
  await g.page.mouse.up();
  await g.nextFrame(3);
  expect((await read(g)).cells["Two"], "the names return at the release").toMatchObject({ text: "Ana, Ben" });
});

test("a schedule with no names has no Resources column and no track beyond its cells", async ({ g }) => {
  await g.open({ n: 50 });
  const list = await read(g);
  expect([list.cells, list.aligned, list.heads.includes("Resources")]).toEqual([{}, true, false]);
});

test("with the progress and variance columns on, Resources is still last, after a redraw", async ({ g }) => {
  await openFile(g);
  await g.page.locator("#progress-toggle").click();
  await g.page.locator("#baseline-set").click();
  await g.page.evaluate(() => window.gantt!.app.set_duration(0, "3d"));
  await g.nextFrame(3);
  const list = await read(g);
  expect(list).toMatchObject({ heads: expect.arrayContaining(["% complete", "ΔStart"]), lastHead: "Resources", aligned: true, cells: { Two: { text: "Ana, Ben", last: true } } });
});
