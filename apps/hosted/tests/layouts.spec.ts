import { expect, test, type Page } from "@playwright/test";
import type { Commands } from "gantt-core";
import { waitReady, waitSampleSettled } from "./support/page";

// "Save layout..." and "Apply layout..." in the File menu (gantt#703): a print setup kept by name in this browser, put back on another schedule after a reload.
test.use({ viewport: { width: 1280, height: 800 } });

declare global {
  interface Window {
    hostCommands?: Commands;
  }
}

async function load(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
}

const columns = (page: Page): Promise<string[]> =>
  page.evaluate(() => (JSON.parse(window.hostCommands!.layoutJson("x") as string) as { columns: string[] }).columns);

async function choose(page: Page, name: RegExp, answer: string): Promise<string> {
  let asked = "";
  page.once("dialog", (dialog) => void ((asked = dialog.message()), dialog.accept(answer)));
  await page.locator("#open-button").click();
  await page.getByRole("menuitem", { name }).click();
  await expect(page.locator("#sample-list"), "the menu closes").toBeHidden();
  return asked;
}

test("a layout saved from the File menu is still kept after a reload, and applying it on a second sample gives the same columns", async ({ page }) => {
  await page.addInitScript(() => {
    document.addEventListener("gantt:ready", (e) => void (window.hostCommands = (e as CustomEvent<Commands>).detail));
  });
  await load(page, "/");
  // Hide Float and move Finish up before Start, as a person would from the Columns menu.
  await page.locator("#columns-button").click();
  await page.locator("#columns-float").click();
  await page.locator('#columns-panel [data-row="finish"] button[data-move="-1"]').click();
  await page.keyboard.press("Escape");
  const saved = await columns(page);
  expect(saved, "Float is off and Finish is before Start").not.toContain("Float");
  expect(saved.indexOf("Finish")).toBeLessThan(saved.indexOf("Start"));

  await choose(page, /^Save layout\.\.\./, "Mine");
  await expect(page.locator("#sample-status")).toHaveText('Saved the layout "Mine" in this browser.');

  // The page keeps hidden columns by itself across a reload (hidden-columns.ts); forget that, so the second sample starts with every column and the apply is what changes it.
  await page.evaluate(() => localStorage.removeItem("gantt-hidden-columns"));
  await load(page, "/?sample=medium-5k");
  await expect(page.locator("#table")).not.toHaveAttribute("data-hide-float", /.*/);
  expect(await columns(page), "a fresh page shows the columns in their own order").not.toEqual(saved);

  const asked = await choose(page, /^Apply layout\.\.\./, "Mine");
  expect(asked, "the prompt lists the layouts kept").toContain("Mine");
  await expect(page.locator("#sample-status")).toHaveText(
    'Applied the layout "Mine". Not applied: Start variance, Finish variance, Float variance (the file has no such column).',
  );
  await expect(page.locator("#table")).toHaveAttribute("data-hide-float", "");
  expect(await columns(page), "the same columns in the same order").toEqual(saved);
});

test("Apply layout... with nothing saved says so", async ({ page }) => {
  await load(page, "/");
  await choose(page, /^Apply layout\.\.\./, "");
  await expect(page.locator("#sample-status")).toContainText("No layout is kept in this browser yet");
});
