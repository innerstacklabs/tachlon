import { expect, test } from "./support/gantt";
import { EDITED, editDuration, fixture, freezeClock, muteNoteHover, note, openLedger, pressAndDrag, rename, rowOf, taskOfCode } from "./support/moved-note";
import { AXIS, ROW } from "./support/pixels";

// WebGL2 from the start: this file checks the note and its timing, not the GPU. On a loaded runner a WebGPU device lost just after
// the page was ready reloaded it onto WebGL2 under the test, and the hold check failed with alpha 1 on main (gantt#1163).
test.use({ webgl: true });

// The "what moved" note (gs#489, story S5a of #429): after an edit or a drag release that moved other tasks, one line of
// text at the top right of the canvas, shown at full opacity for 4 seconds and then faded out over 0.4 seconds by the
// app's own `status.moved.alpha`. The 3.9 s and 4.5 s checks run on Playwright's fake clock (`page.clock`), installed
// after `g.open()` and paused before the edit, so the report's age is exactly what the test advances. While the clock is
// paused `g.nextFrame()` would hang on a real rAF that never comes; the tests use `runFor`. The ledger, the edits, the
// fake clock, the read of the note and the press-and-drag are in `support/moved-note.ts`.

test("the app's moved.alpha follows the fake clock, and the note follows the alpha", async ({ g }) => {
  await openLedger(g);
  await freezeClock(g.page);
  await editDuration(g.page, "10d");
  await g.page.clock.runFor(64);
  const fresh = (await g.status()).moved;
  expect(fresh, "the edit moved other tasks").not.toBeNull();
  expect(fresh!.alpha, "a report that is 32 ms old is fully shown").toBe(1);

  // Age 4.2 s: the real clock has moved by a few hundred ms at most since the edit, so only the fake clock can put the
  // alpha between 0 and 1 here.
  await g.page.clock.fastForward(4100);
  await g.page.clock.runFor(32);
  const mid = (await g.status()).moved!;
  expect(mid.alpha, "the alpha fell with the fake clock, not the real one").toBeGreaterThan(0.3);
  expect(mid.alpha).toBeLessThan(0.7);
  const shown = await note(g.page);
  expect(shown.hidden, "the note is still there at 4.2 s").toBe(false);
  expect(Math.abs(Number(shown.opacity) - mid.alpha), "its opacity is the alpha, to within a frame").toBeLessThan(0.1);
});

test("after a duration edit the note says what moved: full for 3.9 s, gone by 4.5 s", async ({ g }) => {
  await openLedger(g);
  expect((await note(g.page)).hidden, "no note before any edit").toBe(true);
  await freezeClock(g.page);
  await editDuration(g.page, "10d");
  await g.page.clock.runFor(64);

  const status = await g.status();
  const at0 = await note(g.page);
  expect(at0.hidden, "the note shows").toBe(false);
  expect(at0.text, "its text is the report's text").toBe(status.moved!.text);
  expect(at0.opacity).toBe("1");
  expect(at0.parent, "the note is a child of the chart root").toBe("app");
  expect(at0.focusable, "the note takes no focus").toBe(false);

  await g.page.clock.fastForward(3836);
  await g.page.clock.runFor(32);
  const at39 = await note(g.page);
  expect(at39.hidden, "shown at 3.9 s").toBe(false);
  expect(at39.opacity, "opacity 1 at 3.9 s").toBe("1");

  await g.page.clock.fastForward(568);
  await g.page.clock.runFor(32);
  const gone = await note(g.page);
  expect(gone.hidden, "hidden at 4.5 s").toBe(true);
  expect(gone.text, "the live region's text is cleared once the note hides").toBe("");

  // The same report is not shown again by later frames.
  await g.page.clock.runFor(500);
  expect((await note(g.page)).hidden, "still hidden").toBe(true);
});

test("an undo shows a new note", async ({ g }) => {
  await openLedger(g);
  await freezeClock(g.page);
  await editDuration(g.page, "10d");
  await g.page.clock.runFor(64);
  await g.page.clock.fastForward(4500);
  await g.page.clock.runFor(32);
  expect((await note(g.page)).hidden, "the first note has gone").toBe(true);

  await g.page.keyboard.press("Control+Z");
  await g.page.clock.runFor(64);
  const undone = (await g.status()).moved;
  expect(undone, "the undo moved the tasks back and reports it").not.toBeNull();
  const shown = await note(g.page);
  expect(shown.hidden, "the undo's note shows").toBe(false);
  expect(shown.text).toBe(undone!.text);
});

test("a new report replaces the text of the one on show at once", async ({ g }) => {
  await openLedger(g);
  await freezeClock(g.page);
  await editDuration(g.page, "10d");
  await g.page.clock.runFor(64);
  expect((await note(g.page)).text).toContain("finish +7 days");
  await g.page.clock.fastForward(1000);
  await g.page.keyboard.press("Control+Z");
  await g.page.clock.runFor(64);
  const after = await note(g.page);
  expect(after.hidden).toBe(false);
  expect(after.text, "the undo's text, not the edit's").toContain("finish -7 days");
  expect(after.opacity, "a new report starts fully shown").toBe("1");
});

test("a bar drag release shows a note", async ({ g }) => {
  await openLedger(g);
  await pressAndDrag(g, false);
  expect((await note(g.page)).hidden, "no note while the drag is on: the report is made at the release").toBe(true);
  await g.page.mouse.up();
  await g.nextFrame(3);
  const moved = (await g.status()).moved;
  expect(moved, "the drag moved other tasks").not.toBeNull();
  const shown = await note(g.page);
  expect(shown.hidden, "the release shows a note").toBe(false);
  expect(shown.text).toBe(moved!.text);
});

test("a rename hides the note, and so does opening a file", async ({ g }) => {
  // The open is over an edit, so it asks (gs#703); the test answers yes.
  g.page.on("dialog", (dialog) => void dialog.accept());
  await openLedger(g);
  await editDuration(g.page, "10d");
  await g.nextFrame(3);
  expect((await note(g.page)).hidden, "the edit's note shows").toBe(false);

  await rename(g.page, "Renamed by the note spec");
  await g.nextFrame(3);
  expect((await note(g.page)).hidden, "the rename hides the note").toBe(true);

  await editDuration(g.page, "11d");
  await g.nextFrame(3);
  expect((await note(g.page)).hidden, "another edit shows a note").toBe(false);
  const epoch = (await g.status()).epoch;
  await g.page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: fixture });
  await g.page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch + 1, { timeout: 60_000 });
  await g.nextFrame(3);
  expect((await note(g.page)).hidden, "opening a file hides the note").toBe(true);
});

test("an idle frame makes no DOM mutation on the note", async ({ g }) => {
  await openLedger(g);
  await freezeClock(g.page);
  await editDuration(g.page, "10d");
  await g.page.clock.runFor(64);
  expect((await note(g.page)).hidden).toBe(false);
  // Count the mutations of the note and of nothing else.
  await g.page.evaluate(() => {
    const counter = { count: 0 };
    (window as unknown as { __noteMutations: typeof counter }).__noteMutations = counter;
    const watcher = new MutationObserver((records) => (counter.count += records.length));
    watcher.observe(document.getElementById("moved-note")!, { attributes: true, childList: true, characterData: true, subtree: true });
    (window as unknown as { __noteWatcher: MutationObserver }).__noteWatcher = watcher;
  });
  const count = (): Promise<number> =>
    g.page.evaluate(() => {
      const w = window as unknown as { __noteMutations: { count: number }; __noteWatcher: MutationObserver };
      return w.__noteMutations.count + w.__noteWatcher.takeRecords().length;
    });

  // Ages 32 ms to 2.5 s: the report is fully shown and the same, 150 frames.
  await g.page.clock.runFor(2500);
  expect((await g.status()).frames, "frames ran").toBeGreaterThan(100);
  expect(await count(), "idle frames while the note is shown wrote nothing").toBe(0);

  // The fade does write (the opacity); this is the check that the counter can see a write at all.
  await g.page.clock.fastForward(1500);
  await g.page.clock.runFor(32);
  await g.page.clock.runFor(160);
  expect(await count(), "the fade writes the opacity").toBeGreaterThan(0);

  // Hidden again, and idle again.
  await g.page.clock.fastForward(1000);
  await g.page.clock.runFor(32);
  expect((await note(g.page)).hidden).toBe(true);
  const settled = await count();
  await g.page.clock.runFor(1000);
  expect(await count(), "idle frames while the note is hidden wrote nothing").toBe(settled);
});

test("embedded in a host box the note sits inside the root, at the canvas's top right", async ({ g }) => {
  await openLedger(g);
  await g.page.evaluate(() => {
    const host = document.createElement("div");
    host.id = "host";
    host.style.cssText = "position: absolute; left: 20px; top: 60px; width: 1000px; height: 400px";
    const root = document.getElementById("app")!;
    document.body.append(host);
    host.append(root);
  });
  await g.nextFrame(3);
  await g.page.evaluate(() => window.gantt!.app.set_duration(12, "10d"));
  await g.nextFrame(3);
  expect((await note(g.page)).hidden, "the note shows in the host box").toBe(false);

  const place = (): Promise<{ root: DOMRect; canvas: DOMRect; note: DOMRect; parent: string | undefined }> =>
    g.page.evaluate(() => {
      const rect = (id: string): DOMRect => document.getElementById(id)!.getBoundingClientRect();
      return { root: rect("app"), canvas: rect("timeline"), note: rect("moved-note"), parent: document.getElementById("moved-note")!.parentElement?.id };
    });
  const check = (p: Awaited<ReturnType<typeof place>>, label: string): void => {
    expect(p.parent, `${label}: a child of the root`).toBe("app");
    expect(p.note.left, `${label}: inside the root, left`).toBeGreaterThanOrEqual(p.root.left);
    expect(p.note.top, `${label}: inside the root, top`).toBeGreaterThanOrEqual(p.root.top);
    expect(p.note.right, `${label}: inside the root, right`).toBeLessThanOrEqual(p.root.right + 0.5);
    expect(p.note.bottom, `${label}: inside the root, bottom`).toBeLessThanOrEqual(p.root.bottom + 0.5);
    expect(p.note.left, `${label}: inside the canvas, left`).toBeGreaterThanOrEqual(p.canvas.left - 0.5);
    expect(p.note.top, `${label}: inside the canvas, top`).toBeGreaterThanOrEqual(p.canvas.top - 0.5);
    expect(p.note.right, `${label}: 8 px from the canvas's right edge`).toBeCloseTo(p.canvas.right - 8, 0);
    expect(p.note.top, `${label}: 4 px below the canvas's top edge`).toBeCloseTo(p.canvas.top + 4, 0);
  };
  const before = await place();
  expect(before.root.left, "the root is in the host, not at the window's corner").toBe(20);
  check(before, "embedded");

  // The task list divider moves the canvas (the host is wide enough for the key to move it); the
  // note stays on the canvas's top right.
  await g.page.locator("#split").focus();
  await g.page.keyboard.press("ArrowRight");
  await g.nextFrame(3);
  const after = await place();
  expect(after.canvas.left, "the divider moved the canvas").not.toBe(before.canvas.left);
  check(after, "after the divider moved");
});

test("the note stays in the accessibility tree, and its text is written only after it is shown", async ({ g }) => {
  await openLedger(g);
  await freezeClock(g.page);
  // What a screen reader needs: the live region is never `hidden` or `display: none`, before a report, while one shows,
  // and after it faded. A region that comes back from `display: none` in the tick its text is set can go unannounced.
  const inTree = (): Promise<boolean> =>
    g.page.evaluate(() => {
      const el = document.getElementById("moved-note")!;
      return !el.hasAttribute("hidden") && getComputedStyle(el).display !== "none" && getComputedStyle(el).visibility !== "hidden" && el.getAttribute("role") === "status";
    });
  expect(await inTree(), "in the tree before any report").toBe(true);
  expect((await note(g.page)).text, "and empty").toBe("");

  // Record every write to the note, in order: a style write is the opacity, a text write is the words.
  await g.page.evaluate(() => {
    const log: string[] = [];
    (window as unknown as { __noteLog: string[] }).__noteLog = log;
    const watcher = new MutationObserver((records) => {
      for (const r of records) log.push(r.type === "attributes" ? `attr:${r.attributeName}` : "text");
    });
    watcher.observe(document.getElementById("moved-note")!, { attributes: true, childList: true, characterData: true, subtree: true });
    (window as unknown as { __noteWatcher: MutationObserver }).__noteWatcher = watcher;
  });
  await editDuration(g.page, "10d");
  // Step less than a frame at a time, so each frame's writes are seen on their own.
  const steps: { visible: boolean; text: string }[] = [];
  for (let i = 0; i < 12; i += 1) {
    await g.page.clock.runFor(8);
    const state = await note(g.page);
    steps.push({ visible: !state.hidden, text: state.text });
    expect(await inTree(), `in the tree at step ${i}`).toBe(true);
  }
  const firstVisible = steps.findIndex((s) => s.visible);
  const firstText = steps.findIndex((s) => s.text !== "");
  expect(firstVisible, "the note became visible").toBeGreaterThanOrEqual(0);
  expect(firstText, "and then got its text").toBeGreaterThanOrEqual(0);
  expect(firstText, "the text is written on a frame after the note is shown, not in the same tick").toBeGreaterThan(firstVisible);
  expect(steps[firstVisible].text, "the frame that shows it holds no text yet").toBe("");
  const log = await g.page.evaluate(() => {
    const w = window as unknown as { __noteLog: string[]; __noteWatcher: MutationObserver };
    for (const r of w.__noteWatcher.takeRecords()) w.__noteLog.push(r.type === "attributes" ? `attr:${r.attributeName}` : "text");
    return w.__noteLog;
  });
  expect(log.indexOf("text"), "the first text write comes after the first style write").toBeGreaterThan(log.indexOf("attr:style"));
  expect(log.indexOf("attr:style"), "and a style write came first").toBe(0);

  // After it fades the region is still there, empty and invisible.
  await g.page.clock.fastForward(4500);
  await g.page.clock.runFor(64);
  const gone = await note(g.page);
  expect(gone.hidden).toBe(true);
  expect(gone.text, "no text is left in the region").toBe("");
  expect(await inTree(), "in the tree after the fade").toBe(true);
});

test("the pointer over the note holds it; leaving lets it run out; hiding ends the hold", async ({ g }) => {
  await openLedger(g);
  await freezeClock(g.page);
  await editDuration(g.page, "10d");
  await g.page.clock.runFor(64);
  // Edits go through the app, so the pointer stays where the test put it. Both in one call make one frame's change.
  const edit = (renamed: string | null, duration: string | null): Promise<void> =>
    g.page.evaluate(
      ([task, name, text]) => {
        const app = window.gantt!.app;
        if (name !== null) app.rename_task(task as number, name as string);
        if (text !== null) app.set_duration(task as number, text as string);
      },
      [EDITED, renamed, duration],
    );
  const overNote = async (): Promise<void> => {
    const box = await g.page.locator("#moved-note").boundingBox();
    expect(box, "the note has a box").not.toBeNull();
    await g.page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  };
  const awayFromNote = async (): Promise<void> => {
    const box = await g.page.locator("#timeline").boundingBox();
    await g.page.mouse.move(box!.x + 60, box!.y + box!.height - 60);
  };
  const frames = (ms: number): Promise<void> => g.page.clock.runFor(ms);
  const alpha = async (): Promise<number> => (await g.status()).moved?.alpha ?? -1;

  // The pointer enters at 3.9 s. At 4.5 s the report is still on show, at full opacity.
  await g.page.clock.fastForward(3836);
  await frames(32);
  await overNote();
  await frames(32);
  await g.page.clock.fastForward(568);
  await frames(32);
  expect(await alpha(), "held past the fade").toBe(1);
  const held = await note(g.page);
  expect(held.hidden, "still shown at 4.5 s").toBe(false);
  expect(held.opacity).toBe("1");

  // The pointer leaves: shown 0.9 s later, hidden 1.5 s later.
  await awayFromNote();
  await frames(32);
  await g.page.clock.fastForward(836);
  await frames(32);
  expect(await alpha(), "0.9 s after the pointer left").toBe(1);
  expect((await note(g.page)).hidden, "shown 0.9 s after it left").toBe(false);
  await g.page.clock.fastForward(600);
  await frames(32);
  expect((await note(g.page)).hidden, "hidden 1.5 s after it left").toBe(true);

  // A rename hides the note under the pointer, and the hold goes with it: the next note fades.
  await edit(null, "11d");
  await frames(64);
  await overNote();
  await frames(32);
  await g.page.clock.fastForward(4500);
  await frames(32);
  expect(await alpha(), "held at 4.5 s").toBe(1);
  // The pointer does not move from here to the end of this part. The browser sends the note a pointerleave when it
  // empties and a pointerenter when the next one has text, on whichever of its own frames hit-tests next: before or
  // after any step below, and not on the fake clock (gs#595, a pointerenter that came after the next edit held it and
  // failed the alpha check at 4.5 s). Held back, the only thing that can end the hold at the hide is the page's own
  // reset, and the check below fails if that is gone.
  await muteNoteHover(g.page, true);
  await edit("Renamed by the hold spec", null);
  await frames(64);
  expect((await note(g.page)).hidden, "the rename hid the note").toBe(true);
  await edit(null, "12d");
  await frames(64);
  expect((await note(g.page)).hidden, "the next edit shows a note").toBe(false);
  await g.page.clock.fastForward(4500);
  await frames(32);
  expect(await alpha(), "the next note faded: the hold did not outlive the hide").toBe(0);
  expect((await note(g.page)).hidden).toBe(true);
  await muteNoteHover(g.page, false);

  // A rename and an edit between two frames, under the pointer: the new note is still held.
  await awayFromNote();
  await edit(null, "13d");
  await frames(64);
  await overNote();
  await frames(32);
  await edit("Renamed in one frame", "14d");
  await frames(64);
  await g.page.clock.fastForward(4500);
  await frames(32);
  expect(await alpha(), "held across a rename in the same frame").toBe(1);
  await awayFromNote();
  await frames(32);
});

// A link edit shows the note a duration edit shows (gs#703). Activity A1-23 to activity A1-27 moves a cone of tasks, the
// finish by five days and the critical path; the report's text and slide flag are checked on `Session` in
// `crates/app/tests/moved.rs`.
test("an Alt-drag link shows the note a duration edit shows", async ({ g }) => {
  await openLedger(g);
  const box = await g.box();
  const barPoint = async (task: number) => {
    const row = await rowOf(g.page, task);
    const x = await g.page.evaluate((r) => window.gantt!.app.x_of(window.gantt!.app.row_start(r)!), row);
    return { x: box.x + x + 6, y: box.y + AXIS + row * ROW + ROW / 2 };
  };
  const [from, to] = [await barPoint(await taskOfCode(g.page, "A1-23")), await barPoint(await taskOfCode(g.page, "A1-27"))];
  await g.page.mouse.move(from.x, from.y);
  await g.page.keyboard.down("Alt");
  await g.page.mouse.down();
  await g.page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
  await g.page.mouse.move(to.x, to.y, { steps: 4 });
  await g.nextFrame(2);
  await g.page.mouse.up();
  await g.page.keyboard.up("Alt");
  await g.nextFrame(3);
  const status = await g.status();
  expect(status.link.selected, "the drag added a link").not.toBeNull();
  expect(status.moved, "the link moved tasks").not.toBeNull();
  const shown = await note(g.page);
  expect(shown.hidden, "the link shows a note").toBe(false);
  expect(shown.text, "its text is the report's text").toBe(status.moved!.text);
});

// The Show moved button (gs#703): a sibling of the note that keeps the tasks the last edit moved. The rows it keeps and the
// change that ends it are checked on `Session` in `crates/app/tests/filter_moved.rs`.
test("clicking Show moved turns the Moved filter on and hides the button", async ({ g }) => {
  await openLedger(g);
  const button = g.page.locator("#moved-show");
  await expect(button, "no button before any edit").toBeHidden();
  await editDuration(g.page, "10d");
  await g.nextFrame(3);
  await expect(button, "the button shows after an edit that moved others").toBeVisible();
  expect(await button.evaluate((el) => el.closest("#moved-note")), "it is not inside the note").toBeNull();
  await button.click();
  await g.nextFrame(3);
  expect((await g.status()).filter, "the click reached the app").toBe("moved");
  await expect(button, "the button is gone while the rows are the moved ones").toBeHidden();
});

test("an idle frame makes no DOM mutation on the button, hidden or shown", async ({ g }) => {
  await openLedger(g);
  await freezeClock(g.page);
  // Watch the button from the start, while it is hidden: a write of `hidden = true` every frame would queue a record.
  await g.page.evaluate(() => {
    const counter = { count: 0 };
    (window as unknown as { __btnMutations: typeof counter }).__btnMutations = counter;
    const watcher = new MutationObserver((records) => (counter.count += records.length));
    watcher.observe(document.getElementById("moved-show")!, { attributes: true, childList: true, characterData: true, subtree: true });
    (window as unknown as { __btnWatcher: MutationObserver }).__btnWatcher = watcher;
  });
  const count = (): Promise<number> =>
    g.page.evaluate(() => {
      const w = window as unknown as { __btnMutations: { count: number }; __btnWatcher: MutationObserver };
      return w.__btnMutations.count + w.__btnWatcher.takeRecords().length;
    });
  await g.page.clock.runFor(1000);
  expect(await count(), "idle frames while the button is hidden wrote nothing").toBe(0);

  // The edit shows it: one write, then idle again through the fade.
  await editDuration(g.page, "10d");
  await g.page.clock.runFor(64);
  await expect(g.page.locator("#moved-show")).toBeVisible();
  const shown = await count();
  expect(shown, "showing the button wrote").toBeGreaterThan(0);
  await g.page.clock.runFor(2500);
  await g.page.clock.fastForward(2500);
  await g.page.clock.runFor(500);
  expect((await note(g.page)).hidden).toBe(true);
  expect(await count(), "idle frames while the button is shown, and the note fading, wrote nothing").toBe(shown);

  // A rename ends the report: the button hides with one write, then idle again.
  await rename(g.page, "Renamed");
  await g.page.clock.runFor(64);
  await expect(g.page.locator("#moved-show")).toBeHidden();
  const hidden = await count();
  expect(hidden, "hiding the button wrote").toBeGreaterThan(shown);
  await g.page.clock.runFor(1000);
  expect(await count(), "idle frames while the button is hidden again wrote nothing").toBe(hidden);
});
