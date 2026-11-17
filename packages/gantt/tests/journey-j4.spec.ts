// Journey J4 ("J4. Run the DCMA 14-point checks") on one page, one `test.step` per journey step
// that the page has today. Only the journey's starting point is built: a schedule is open, with its activities and
// the logic the checks read. The Checks panel (steps 1 to 3: open Checks, read the 14 metrics, click one to filter)
// is not in the page yet; the PR that builds it adds its steps here. The metric counts are checked in Rust
// (`crates/app/tests/checks.rs`), not here.
import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

const FILE = {
  name: "one-project.xer",
  buffer: readFileSync(new URL("./fixtures/one-project.xer", import.meta.url)),
  tasks: 14,
};

test("J4: the reviewer opens a schedule to check, and its activities and logic are on screen", async ({ g }) => {
  await g.open({ n: 300 });

  await test.step("With a schedule open", async () => {
    await g.page.setInputFiles("#open-input", {
      name: FILE.name,
      mimeType: "application/octet-stream",
      buffer: FILE.buffer,
    });
    await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
    await g.nextFrame(3);
    await expect(g.page.locator("#open-info")).toContainText(FILE.name);
    await expect(g.page.locator("#table-rows .row").first()).toBeVisible();
    const s = await g.status();
    expect(s.n).toBe(FILE.tasks);
    expect(s.rowCount).toBe(FILE.tasks);
    expect(s.bars).toBeGreaterThan(0);
    expect(s.arrows, "the file's links are drawn: the logic the checks read").toBeGreaterThan(0);
  });
});
