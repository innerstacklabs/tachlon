// The import notes panel is set in the app font, like the rest of the app, and no monospace remains in it (gs#1016).
// The fixture is the one import-notes.spec.ts uses.
import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

const FIXTURE = readFileSync(new URL("./fixtures/one-project.xer", import.meta.url));

test("the open import notes panel uses the app font in its body, heading and list", async ({ g }) => {
  await g.open({ n: 300 });
  await g.page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: FIXTURE });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);

  await g.page.locator("#import-notes").click();
  await expect(g.page.locator("#unsupported")).toBeVisible();

  const fonts = await g.page.evaluate(() => {
    const family = (selector: string): string => getComputedStyle(document.querySelector(selector)!).fontFamily;
    return {
      panel: family("#unsupported"),
      title: family("#unsupported-title"),
      list: family("#unsupported-list"),
      app: getComputedStyle(document.querySelector("#unsupported-head button")!).fontFamily,
    };
  });
  for (const [part, family] of Object.entries(fonts)) {
    expect(family, `${part} font-family`).not.toMatch(/monospace|Gantt Mono/i);
  }
  expect(fonts.panel, "the panel matches its own buttons").toBe(fonts.app);
  expect(fonts.title).toBe(fonts.app);
  expect(fonts.list).toBe(fonts.app);

  // One line per note stays.
  const lines = await g.page.evaluate(() => document.querySelector("#unsupported-list")!.textContent!.split("\n"));
  // Four notes, plus the counted note for the actual dates after the data date (gantt#2107).
  expect(lines, "one line per note").toHaveLength(5);
});
