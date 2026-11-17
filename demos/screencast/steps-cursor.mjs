// The cell cursor step (gs#416). Kept apart from steps-features.mjs, which has a 400-line budget.

export const cellCursor = {
  name: "cell-cursor",
  journey: "J6",
  caption: "Click a cell to put the cursor on it; the arrows move it, Escape clears it",
  run: async (ctx) => {
    await ctx.page.evaluate(() => {
      document.getElementById("table-body").scrollTop = 0;
      window.gantt.app.clear_selection();
    });
    await ctx.page.locator('#table-rows .row[data-row="2"] > .dur').click();
    await ctx.until(() => window.gantt.status().selected?.row === 2);
    await ctx.hold(1);
    for (const key of ["ArrowDown", "ArrowDown", "ArrowRight", "ArrowRight"]) {
      await ctx.page.keyboard.press(key);
      await ctx.hold(0.6);
    }
    await ctx.page.keyboard.press("ArrowUp");
    await ctx.until(() => window.gantt.status().selected?.row === 3);
    await ctx.hold(1);
    await ctx.shot?.("keys", []);
    await ctx.page.keyboard.press("Escape");
    await ctx.until(() => window.gantt.status().selected === null);
    await ctx.hold(1);
  },
};
