import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";
import { dpr } from "./support/knobs";
import { editDuration, freezeClock, note, openLedger, pressAndDrag } from "./support/moved-note";
import { decodePng, type Png } from "./support/pixels";

// The "what moved" note under reduced motion and after a drag release (gs#516, stories S5c and S5d of gs#429; the
// reduced-motion drag release, gs#592). Split from `moved-note.spec.ts`, which holds the note's text, hold, fade, hold by
// the pointer and place (gs#592, story S3 of gs#571). The ledger, the edit, the fake clock, the read of the note and the
// press-and-drag are shared with that file in `support/moved-note.ts` (gs#671).

// The note under reduced motion and after a drag release (gs#516, stories S5c and S5d of gs#429). The pixels of the
// canvas tell whether the dashed outlines are drawn: two screenshots of the same canvas differ where the outlines are
// and where the note is, so the note's own box is left out of the count.

/** How many pixels differ by more than 6 in some channel, leaving out `skip` (CSS px, relative to the canvas). */
function differing(a: Png, b: Png, skip: { x: number; y: number; width: number; height: number }): number {
  expect([a.width, a.height], "two shots of the same canvas have the same size").toEqual([b.width, b.height]);
  const x0 = Math.floor((skip.x - 4) * dpr);
  const x1 = Math.ceil((skip.x + skip.width + 4) * dpr);
  const y0 = Math.floor((skip.y - 4) * dpr);
  const y1 = Math.ceil((skip.y + skip.height + 4) * dpr);
  let count = 0;
  for (let y = 0; y < a.height; y++) {
    for (let x = 0; x < a.width; x++) {
      if (x >= x0 && x < x1 && y >= y0 && y < y1) continue;
      const i = (y * a.width + x) * a.channels;
      let differs = false;
      for (let c = 0; c < 3; c++) differs ||= Math.abs(a.px[i + c] - b.px[i + c]) > 6;
      if (differs) count += 1;
    }
  }
  return count;
}

/** A screenshot of the canvas and the box of the note at that moment, relative to the canvas. */
async function canvasShot(page: Page): Promise<{ png: Png; noteBox: { x: number; y: number; width: number; height: number } }> {
  const canvas = await page.locator("#timeline").boundingBox();
  const noteAt = await page.locator("#moved-note").boundingBox();
  if (canvas === null || noteAt === null) throw new Error("the canvas and the note have boxes");
  const png = decodePng(await page.locator("#timeline").screenshot());
  return { png, noteBox: { x: noteAt.x - canvas.x, y: noteAt.y - canvas.y, width: noteAt.width, height: noteAt.height } };
}

/** At least this many pixels of outline show in a canvas shot of the moved tasks (eight rings of dashes, far more). */
const OUTLINE_PIXELS = 50;

test("with reduced motion the note is full at 3.9 s and gone at 4.05 s, with no slide and no fade", async ({ g }) => {
  await g.page.emulateMedia({ reducedMotion: "reduce" });
  await openLedger(g);
  await freezeClock(g.page);
  await editDuration(g.page, "10d");
  await g.page.clock.runFor(64);

  const first = await g.status();
  expect(first.slide, "reduced motion: the bars are at the new places on the first frame, nothing was copied").toEqual({
    active: false,
    copyMs: null,
  });
  const young = await canvasShot(g.page);
  expect((await note(g.page)).opacity).toBe("1");

  // The outlines show while the report does, and the method is steady: the same report 32 ms later draws the same pixels.
  await g.page.clock.runFor(32);
  const younger = await canvasShot(g.page);
  expect(differing(young.png, younger.png, young.noteBox), "the same report draws the same canvas").toBe(0);

  // Age 3.9 s: full.
  await g.page.clock.fastForward(3772);
  await g.page.clock.runFor(32);
  const at39 = await note(g.page);
  expect(at39.hidden, "shown at 3.9 s").toBe(false);
  expect(at39.opacity, "opacity 1 at 3.9 s").toBe("1");

  // Age 4.05 s: with motion on the alpha would be 0.88 here. With reduced motion it has stepped to 0.
  await g.page.clock.fastForward(118);
  await g.page.clock.runFor(32);
  const done = await g.status();
  expect((await note(g.page)).hidden, "hidden at 4.05 s").toBe(true);
  const old = await canvasShot(g.page);
  expect(differing(young.png, old.png, young.noteBox), "the outlines went with the note").toBeGreaterThan(OUTLINE_PIXELS);
  expect(done.slide.active, "still no slide").toBe(false);
});

test("a drag release shows the note and the outlines, and the bars do not slide", async ({ g }) => {
  await openLedger(g);
  await freezeClock(g.page);
  const box = await pressAndDrag(g, true);
  await g.page.mouse.up();
  // Keep the pointer off the canvas, so no hover tint changes between the two shots.
  await g.page.mouse.move(box.x + box.width + 5, box.y + box.height + 5);

  // Every frame after the release: no slide, nothing copied, and the report is a drag release's.
  for (let frame = 0; frame < 6; frame++) {
    await g.page.clock.runFor(16);
    const s = await g.status();
    expect(s.slide, `no slide on frame ${frame} after the release`).toEqual({ active: false, copyMs: null });
  }
  const moved = (await g.status()).moved;
  expect(moved, "the drag moved other tasks").not.toBeNull();
  const shown = await note(g.page);
  expect(shown.hidden, "the release shows the note").toBe(false);
  expect(shown.text).toBe(moved!.text);
  const young = await canvasShot(g.page);

  // Past the fade the note and the outlines are gone.
  await g.page.clock.fastForward(4500);
  await g.page.clock.runFor(32);
  expect((await note(g.page)).hidden, "hidden at 4.5 s").toBe(true);
  const old = await canvasShot(g.page);
  expect(differing(young.png, old.png, young.noteBox), "the outlines were drawn after the release").toBeGreaterThan(OUTLINE_PIXELS);
});

test("with reduced motion a drag release shows the note, with no slide, full at 3.9 s and gone at 4.05 s", async ({ g }) => {
  await g.page.emulateMedia({ reducedMotion: "reduce" });
  await openLedger(g);
  await freezeClock(g.page);
  const box = await pressAndDrag(g, true);
  await g.page.mouse.up();
  // Keep the pointer off the note, so no hold keeps the report shown past its time.
  await g.page.mouse.move(box.x + box.width + 5, box.y + box.height + 5);

  // Every frame after the release: no slide, and the report is a drag release's, shown at once.
  for (let frame = 0; frame < 6; frame++) {
    await g.page.clock.runFor(16);
    const s = await g.status();
    expect(s.slide, `no slide on frame ${frame} after the release`).toEqual({ active: false, copyMs: null });
  }
  const moved = (await g.status()).moved;
  expect(moved, "the drag moved other tasks").not.toBeNull();
  const shown = await note(g.page);
  expect(shown.hidden, "the release shows the note").toBe(false);
  expect(shown.text).toBe(moved!.text);
  expect(shown.opacity).toBe("1");

  // Age about 3.9 s: full.
  await g.page.clock.fastForward(3772);
  await g.page.clock.runFor(32);
  expect((await note(g.page)).hidden, "shown at 3.9 s").toBe(false);

  // Age about 4.05 s: with motion on the alpha would be 0.8 here. With reduced motion it has stepped to 0.
  await g.page.clock.fastForward(118);
  await g.page.clock.runFor(32);
  const done = await g.status();
  expect((await note(g.page)).hidden, "hidden at 4.05 s").toBe(true);
  expect(done.slide.active, "still no slide").toBe(false);
});
