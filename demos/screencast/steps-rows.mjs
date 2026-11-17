// The Insert and Delete keys step (gs#50). Kept apart from steps-features.mjs, which has a 400-line budget.

export const insertDelete = {
  name: "insert-delete",
  journey: "J6",
  caption: "Insert adds a row below the selection, Delete removes it; the schedule is re-solved each time",
  run: async (ctx) => {
    const total = (await ctx.status()).n;
    // Select the first leaf by searching its path, so the row is on screen at the top of the table.
    await ctx.page.evaluate(() => {
      document.getElementById("table-body").scrollTop = 0;
      const app = window.gantt.app;
      const task = JSON.parse(app.table_rows(0, 40)).find((row) => row[5] === 0)[0];
      const found = app.find(app.task_details(task).outlinePath);
      for (let k = 0; k < found; k++) if (app.find_go(k) === task) break;
      app.find("");
    });
    await ctx.until(() => window.gantt.status().selected !== null);
    await ctx.page.keyboard.press("Insert");
    await ctx.until((n) => window.gantt.status().n === n + 1, total);
    await ctx.hold(1.5);
    await ctx.shot?.("j6-build", []);
    await ctx.page.keyboard.press("Delete");
    await ctx.until((n) => window.gantt.status().n === n, total);
    await ctx.hold(1);
  },
};
