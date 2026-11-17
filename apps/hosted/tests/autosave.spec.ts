import { expect, test, type Page } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The standalone app keeps a copy of the open schedule after each edit and, when the page is opened again, asks once whether to bring it back (gantt#2077).
test.use({ viewport: { width: 1280, height: 800 } });

const NAME = "Pour the slab";
let asked: string[] = [];
let accept = true;

test.beforeEach(({ page }) => {
  asked = [];
  page.on("dialog", (dialog) => {
    // The chart's own leave-the-page question (an unsaved edit) is always answered Leave; the page's question is the confirm.
    if (dialog.type() === "beforeunload") return void dialog.accept();
    asked.push(dialog.message());
    void (accept ? dialog.accept() : dialog.dismiss());
  });
});

/** Whether the page's copy is in IndexedDB; it never opens a database that is not there, which would make an empty one the page then could not set up. */
const copyKept = (page: Page): Promise<boolean> =>
  page.evaluate(async () => {
    if (!(await indexedDB.databases()).some((db) => db.name === "gantt-autosave")) return false;
    const opening = indexedDB.open("gantt-autosave");
    const db = await new Promise<IDBDatabase>((resolve, reject) => ((opening.onsuccess = () => resolve(opening.result)), (opening.onerror = () => reject(opening.error))));
    const key = await new Promise<IDBValidKey[]>((resolve) => {
      const keys = db.transaction("copy").objectStore("copy").getAllKeys();
      keys.onsuccess = () => resolve(keys.result);
    });
    db.close();
    return key.length > 0;
  });

/** What the page's copy holds right now. */
const copyValue = (page: Page): Promise<unknown> =>
  page.evaluate(async () => {
    const opening = indexedDB.open("gantt-autosave");
    const db = await new Promise<IDBDatabase>((resolve) => (opening.onsuccess = () => resolve(opening.result)));
    const get = db.transaction("copy").objectStore("copy").get("schedule");
    const value = await new Promise((resolve) => (get.onsuccess = () => resolve(get.result)));
    db.close();
    return value;
  });

async function open(page: Page, load: () => Promise<unknown>): Promise<void> {
  await load();
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
}

/** A new schedule with one task named NAME, and the page's copy of it written. */
async function editAndWaitForCopy(page: Page): Promise<void> {
  await open(page, () => page.goto("/"));
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count"), "no tasks").toHaveText("0 rows");
  await page.locator("#app").focus();
  await page.keyboard.press("Insert");
  await page.keyboard.press("F2");
  await page.keyboard.press("Control+A");
  await page.keyboard.type(NAME);
  await page.keyboard.press("Enter");
  await expect(page.locator("#table-rows .row").first(), "the row has its name").toContainText(NAME);
  await expect.poll(() => copyKept(page), { message: "the edit is kept in this browser" }).toBe(true);
  expect(asked, "a first visit asks nothing").toEqual([]);
}

test("a reload offers the edited schedule and Bring back puts the edit back", async ({ page }) => {
  accept = true;
  await editAndWaitForCopy(page);
  await open(page, () => page.reload());
  await expect(page.locator("#row-count"), "the edit is back").toHaveText("1 rows");
  await expect(page.locator("#table-rows .row").first()).toContainText(NAME);
  expect(asked, "asked once").toHaveLength(1);
});

test("Cancel drops the copy, so the next load asks nothing", async ({ page }) => {
  accept = false;
  await editAndWaitForCopy(page);
  await open(page, () => page.reload());
  expect(asked, "the reload asked once").toHaveLength(1);
  await expect.poll(() => copyKept(page), { message: "Cancel dropped the copy" }).toBe(false);
  await open(page, () => page.reload());
  await page.waitForTimeout(1000);
  expect(asked, "the next load asked nothing").toHaveLength(1);
  await expect(page.locator("#row-count"), "the cancelled edit is not back").not.toHaveText("1 rows");
});

/** A copy the chart cannot load, so the next OK is refused. */
const makeUnreadable = (page: Page): Promise<void> =>
  page.evaluate(async () => {
    const opening = indexedDB.open("gantt-autosave");
    const db = await new Promise<IDBDatabase>((resolve) => (opening.onsuccess = () => resolve(opening.result)));
    const put = db.transaction("copy", "readwrite").objectStore("copy").put({ unreadable: true }, "schedule");
    await new Promise((resolve) => (put.onsuccess = resolve));
    db.close();
  });

test("a load the chart refuses keeps the copy", async ({ page }) => {
  accept = true;
  await editAndWaitForCopy(page);
  await makeUnreadable(page);
  await page.reload();
  await expect.poll(() => asked.length, { message: "the reload asked once", timeout: 180_000 }).toBe(1);
  await expect(page.locator("#sample-status"), "the refusal is said").not.toHaveText("");
  await expect(page.locator("#row-count"), "the refused copy did not load").not.toHaveText("1 rows");
  expect(await copyKept(page), "the refused copy is still kept").toBe(true);
});

test("after a refused load, an edit on screen leaves the held copy as it was and the status says so", async ({ page }) => {
  accept = true;
  await editAndWaitForCopy(page);
  await makeUnreadable(page);
  await page.reload();
  await expect(page.locator("#sample-status"), "the copy is said to be held").toContainText("held");
  await page.locator("#app").focus();
  await page.keyboard.press("Insert");
  await page.waitForTimeout(1500);
  expect(await copyValue(page), "the edit did not overwrite the held copy").toEqual({ unreadable: true });
});

test("the question says what OK and Cancel do", async ({ page }) => {
  accept = false;
  await editAndWaitForCopy(page);
  await open(page, () => page.reload());
  expect(asked[0], "the question names OK and Cancel").toMatch(/OK.*Cancel/);
});

test("a schedule brought back shows as unsaved, so opening another file asks first and Cancel keeps the copy", async ({ page }) => {
  accept = true;
  await editAndWaitForCopy(page);
  await open(page, () => page.reload());
  await expect(page.locator("#table-rows .row").first(), "the edit is back").toContainText(NAME);
  await expect(page.locator("#unsaved-mark"), "the brought-back schedule shows as unsaved").toBeVisible();
  accept = false;
  const other = Buffer.from(JSON.stringify(await copyValue(page)));
  await page.setInputFiles("#open-input", { name: "other.json", mimeType: "application/json", buffer: other });
  await expect.poll(() => asked.length, { message: "opening another file asked the chart's question" }).toBe(2);
  expect(asked[1], "the chart's own question").toContain("not saved");
  expect(await copyKept(page), "Cancel kept the copy").toBe(true);
});

test("Save drops the copy, so the next load asks nothing", async ({ page }) => {
  accept = true;
  await editAndWaitForCopy(page);
  await page.locator("#open-button").click();
  const downloading = page.waitForEvent("download");
  await page.locator("#sample-list").getByRole("menuitem", { name: /^Save(?! to cloud| image| layout)/ }).click();
  await downloading;
  await expect.poll(() => copyKept(page), { message: "Save dropped the copy" }).toBe(false);
  await open(page, () => page.reload());
  await page.waitForTimeout(1000);
  expect(asked, "the load after a save asked nothing").toEqual([]);
});
