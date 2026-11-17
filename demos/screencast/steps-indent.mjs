// The indent and outdent keys step (gs#422). Kept apart from steps-features.mjs, which has a 400-line budget.

export const indentOutdent = {
  name: "indent-outdent",
  journey: "J6",
  caption: "Alt+Shift+Right indents the row under the one above; Alt+Shift+Left outdents it",
  run: async (ctx) => {
    // Select the first row whose previous sibling is a summary: an indent then moves it into that summary.
    const task = await ctx.page.evaluate(() => {
      document.getElementById("table-body").scrollTop = 0;
      const app = window.gantt.app;
      app.expand_all();
      const rows = JSON.parse(app.table_rows(0, window.gantt.status().rowCount));
      const level = (r) => app.task_outline_level(rows[r][0]);
      for (let r = 1; r < rows.length; r++) {
        let p = r - 1;
        while (p >= 0 && level(p) > level(r)) p--;
        if (p < 0 || level(p) !== level(r) || rows[p][5] === 0) continue;
        const found = app.find(app.task_details(rows[r][0]).outlinePath);
        for (let k = 0; k < found; k++) if (app.find_go(k) === rows[r][0]) break;
        app.find("");
        return rows[r][0];
      }
      return null;
    });
    if (task === null) throw new Error("no row under a summary sibling to indent");
    await ctx.until((t) => window.gantt.status().selected?.task === t, task);
    const level = () => window.gantt.app.task_outline_level(window.gantt.status().selected.task);
    const start = await ctx.page.evaluate(level);
    await ctx.hold(1);
    await ctx.page.keyboard.press("Alt+Shift+ArrowRight");
    await ctx.until((l) => window.gantt.app.task_outline_level(window.gantt.status().selected.task) === l + 1, start);
    await ctx.hold(1.5);
    await ctx.page.keyboard.press("Alt+Shift+ArrowLeft");
    await ctx.until((l) => window.gantt.app.task_outline_level(window.gantt.status().selected.task) === l, start);
    await ctx.hold(1);
  },
};
