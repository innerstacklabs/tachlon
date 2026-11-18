import { expect, test } from "@playwright/test";
import { createContext, runScene } from "../../../demos/screencast/ctx.mjs";
import * as features from "../../../demos/screencast/steps-features.mjs";
import * as story from "../../../demos/screencast/steps.mjs";
import { pageErrors, trackLogs, waitReady, waitSampleSettled } from "./support/page";

// The journey (gs#396, moved to the hosted build by gs#541): every scene of the README story (demos/screencast/steps.mjs)
// and of the feature clips (steps-features.mjs) runs here, in order, on the hosted build at ?sample=demo-50k. A screencast
// scene is also a test: the recorder runs the same list, with the same assertions, on the MacBook, on the same URL. Here the
// context is in `test` mode, so a hold is one frame and no caption is drawn, but a caption is still resolved (a function
// caption is called and must return text) and a scene's `pre` is still checked. Scenes carry state forward, so each list is
// one test, and a test.step per scene names the scene that failed. A scene that throws fails the test; nothing is skipped.
//
// The sample is a knob, so the spec can be pointed at another: JOURNEY_SAMPLE=medium-5k makes the first scene, `open`,
// fail by name, because its check is that the schedule has 50,000 tasks.
const SAMPLE = process.env.JOURNEY_SAMPLE ?? "demo-50k";
// The close card's caption reads the repository from this variable (run.sh sets it for a recording); a test needs only text.
process.env.SCREENCAST_REPO ??= "owner/name";

// The recorder films 1280x720 (demos/screencast/record.mjs): the scenes aim at that layout.
test.use({ viewport: { width: 1280, height: 720 } });

const lists = [
  ["story", story],
  ["features", features],
] as const;

for (const [label, list] of lists) {
  test(`journey: the ${label} scenes pass in order`, async ({ page }) => {
    const logs = trackLogs(page);
    expect(list.steps.length, `the ${label} list has scenes`).toBeGreaterThan(0);
    await page.goto(`/?sample=${SAMPLE}`);
    await waitReady(page, { frames: 30, timeout: 180_000 });
    // A file sample is still being fetched and opened here; the scenes start on the schedule it leaves.
    await waitSampleSettled(page, 120_000);
    const ctx = await createContext(page, { mode: "test" });
    await test.step("set-up (the opening view)", () => list.setup(ctx));

    const ran: string[] = [];
    for (const [i, scene] of list.steps.entries()) {
      await test.step(`${i + 1}/${list.steps.length} ${scene.name}`, async () => {
        await runScene(ctx, scene);
        ran.push(scene.name);
      });
    }
    expect(ran, "every scene ran").toEqual(list.steps.map((scene) => scene.name));
    expect(pageErrors(logs), "the page logs no error").toEqual([]);
  });
}
