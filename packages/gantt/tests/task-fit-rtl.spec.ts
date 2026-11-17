// gantt#1875: in a right-to-left chart the Task column fits the pane as the mirrored left-to-right chart does.
import { expect, test } from "./support/gantt";

const task = (uid: number, name: string, level: number): string =>
  `<Task><UID>${uid}</UID><ID>${uid}</ID><Name>${name}</Name><OutlineLevel>${level}</OutlineLevel><Summary>${level === 1 ? 1 : 0}</Summary><Duration>PT${level === 1 ? 0 : 40}H0M0S</Duration></Task>`;
const tasks = [task(1, "Works", 1), task(2, "Commission and balance the building management system", 2)].join("");
const buffer = Buffer.from(`<?xml version="1.0"?><Project xmlns="http://schemas.microsoft.com/project"><Name>Fit</Name><StartDate>2027-01-04T08:00:00</StartDate><Tasks>${tasks}</Tasks></Project>`);

test("the same file at the same width gives --gantt-task-fit with dir=rtl as with dir=ltr", async ({ g }) => {
  await g.open({ n: 200 });
  const fit = async (dir: string, epoch: number): Promise<string> => {
    await g.page.evaluate((to) => document.getElementById("app")!.setAttribute("dir", to), dir);
    await g.page.setInputFiles("#open-input", { name: "fit.xml", mimeType: "application/xml", buffer });
    await g.page.waitForFunction((at) => window.gantt!.status().epoch === at, epoch, { timeout: 60_000 });
    await g.nextFrame(3);
    return g.page.evaluate(() => document.getElementById("app")!.style.getPropertyValue("--gantt-task-fit"));
  };
  const ltr = await fit("ltr", 1);
  expect(ltr, "the name needs more than the token, so the fit is set").toMatch(/^\d+px$/);
  expect(await fit("rtl", 2)).toBe(ltr);
});
