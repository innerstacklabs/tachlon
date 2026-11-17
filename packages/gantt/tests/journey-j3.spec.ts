import { readFileSync } from "node:fs";
import { expect, openFilters, test } from "./support/gantt";

// Journey J3, "Compare an update with its baseline or a previous update", walked on one page with one
// step per journey step. The comparison is a baseline: the one the Baseline button keeps of the schedule as it is (the
// previous update), or the one an opened file carries. An edit after the button stands in for the later update. What the
// counts, variances and filtered rows are is checked on `Session` in `crates/app/tests/baseline*.rs` and `filter_*.rs`; this
// checks that the reader sees them and can press them. Choosing a second file as the comparison is the hosting page's File menu item,
// "Compare with a file...", and its spec is there; switching to another file of the same project is not built yet.

const SIZE = 2000;
const fmt = new Intl.NumberFormat("en-US");
const days = (n: number): string => `${fmt.format(n)} ${n === 1 ? "day" : "days"}`;
const baselined = readFileSync(new URL("./fixtures/baselined.xml", import.meta.url));

test("J3: the reader compares an update with its baseline", async ({ page, g }) => {
  test.setTimeout(240_000);
  const set = page.locator("#baseline-set");
  const chip = page.locator("#baseline-summary");
  const line = page.locator("#status-line");
  const table = page.locator("#table");
  const slipped = page.locator("#slipped-toggle");
  const joined = page.locator("#joined-critical-toggle");
  const left = page.locator("#left-critical-toggle");
  const floatLost = page.locator("#float-lost-toggle");
  const filtersButton = page.locator("#filters-button");
  let all = 0;

  await test.step("1. opens the update and keeps it as the comparison; the next update moves some tasks", async () => {
    await g.open({ n: SIZE });
    all = (await g.status()).rowCount;
    await page.keyboard.press("s");
    await expect(line).toBeVisible();
    await expect(line).not.toContainText("vs baseline");
    await expect(set).toHaveAttribute("aria-pressed", "false");
    await expect(chip, "no comparison, no summary").toBeHidden();
    await expect(table).not.toHaveAttribute("data-variance", /.*/);
    await openFilters(page);
    for (const button of [slipped, joined, left, floatLost]) await expect(button, "no comparison, no filter").toBeHidden();
    await page.keyboard.press("Escape");

    await set.click();
    await g.nextFrame(2);
    const says = `Baseline set: ${fmt.format((await g.status()).n)} rows`;
    await expect(set).toHaveAttribute("aria-pressed", "true");
    await expect(set).toHaveAttribute("title", new RegExp(`^${says}`));
    await expect(line).toContainText(says);
    await expect(chip).toHaveText("vs baseline: 0 slipped");

    // The later update: lengthen leaves in turn until one moves the critical path and costs some task five days of float.
    const moved = await page.evaluate((count) => {
      const { app } = window.gantt!;
      const rows = JSON.parse(app.table_rows(0, count)) as [number, string, string, number | null, boolean, number][];
      for (const leaf of rows.filter((r) => r[5] === 0).slice(0, 200)) {
        app.set_duration(leaf[0], "400d");
        const b = window.gantt!.status().baseline!;
        if (b.joined > 0 && b.left > 0 && app.set_float_lost(true)) {
          const kept = window.gantt!.status().rowCount;
          app.set_float_lost(false);
          if (kept > 0 && kept < count) return true;
        }
        app.undo();
      }
      return false;
    }, all);
    expect(moved, "an edit that slips tasks, moves the critical path and loses float").toBe(true);
    await g.nextFrame(2);
  });

  await test.step("2. reads the comparison summary", async () => {
    const b = (await g.status()).baseline!;
    expect(b.slipped).toBeGreaterThan(0);
    expect(b.finish_days, "the finish moved later").toBeGreaterThan(0);
    const counts = [`${fmt.format(b.slipped)} slipped`, `finish +${days(b.finish_days)}`];
    const critical = `critical ${fmt.format(b.joined)} joined, ${fmt.format(b.left)} left`;
    await expect(chip).toHaveText(`vs baseline: ${[...counts, critical].join(" · ")}`);
    await expect(line).toContainText(`vs baseline: ${[...counts, critical].join(", ")}`);
    await expect(table).toHaveAttribute("data-variance", "true");
    for (const head of ["var-start", "var-finish", "var-float"]) await expect(page.locator(`#table-head .${head}`)).toBeVisible();
  });

  await test.step("3. filters to moved, joined and left the critical path, and float lost", async () => {
    const b = (await g.status()).baseline!;
    await openFilters(page);
    const ids = await page.locator("#filters-panel button").evaluateAll((els) => els.map((el) => el.id));
    const at = ids.indexOf("slipped-toggle");
    expect(ids.slice(at, at + 4)).toEqual(["slipped-toggle", "joined-critical-toggle", "left-critical-toggle", "float-lost-toggle"]);

    await slipped.click();
    await g.nextFrame(2);
    expect((await g.status()).filter).toBe("slipped");
    expect((await g.status()).rowCount, "the summaries above the slipped tasks stay").toBeGreaterThan(b.slipped);
    await expect(slipped).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#row-count"), "the count is of tasks, not rows").toHaveText(`${fmt.format(b.slipped)} slipped`);

    await openFilters(page);
    await joined.click();
    expect((await g.status()).filter).toBe("joined");
    expect((await g.status()).rowCount).toBeLessThan(all);
    await expect(joined).toHaveAttribute("aria-pressed", "true");
    await expect(slipped, "one filter at a time").toHaveAttribute("aria-pressed", "false");
    await expect(filtersButton).toHaveText("Filters · 1");

    await left.click();
    expect((await g.status()).filter).toBe("left");
    await expect(joined).toHaveAttribute("aria-pressed", "false");
    await expect(left).toHaveAttribute("aria-pressed", "true");

    await expect(floatLost).toBeEnabled();
    await floatLost.click();
    expect((await g.status()).filter).toBe("float");
    expect((await g.status()).rowCount).toBeLessThan(all);
    await expect(floatLost).toHaveAttribute("aria-pressed", "true");
    await expect(left).toHaveAttribute("aria-pressed", "false");
    await expect(filtersButton).toHaveText("Filters · 1");

    await floatLost.click();
    expect((await g.status()).filter).toBe("off");
    expect((await g.status()).rowCount, "every row is back").toBe(all);
    await page.keyboard.press("Escape");
  });

  await test.step("1, from a file: an update that carries its own baseline opens with it as the comparison", async () => {
    const epoch = (await g.status()).epoch;
    page.once("dialog", (dialog) => void dialog.accept()); // the edit above is unsaved, so the open asks first
    await page.setInputFiles("#open-input", { name: "baselined.xml", mimeType: "application/octet-stream", buffer: baselined });
    await page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch + 1, { timeout: 60_000 });
    await g.nextFrame(3);
    await expect(set).toHaveAttribute("aria-pressed", "true");
    await expect(set).toHaveAttribute("title", /^Baseline set/);
    // The file's baseline finish is a working day before the schedule's: one task slipped.
    await expect(line).toContainText("vs baseline: 1 slipped");
    await expect(chip).toContainText("vs baseline: 1 slipped");
    await openFilters(page);
    await expect(slipped).toBeVisible();
    // A file's baseline states dates and no float, so there is no float to compare.
    await expect(floatLost).toBeDisabled();
    await expect(floatLost).toHaveAttribute("title", "This baseline has no float to compare");
  });
});
