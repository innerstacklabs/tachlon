// The home page's script: the navigation bar, and the live chart in the hero, mounted from the package's own files (package/ is the
// package's dist-lib, copied beside the page when the site is staged: the deploy job's stage steps in .github/workflows/ci.yml). The Copy button is copy.js.

// The navigation bar: a rule under it once the page scrolls, the phone menu, and one open dropdown at a time.
const nav = document.getElementById("nav");
addEventListener("scroll", () => nav.classList.toggle("scrolled", scrollY > 8), { passive: true });
const toggle = nav.querySelector(".nav-toggle");
toggle.addEventListener("click", () => toggle.setAttribute("aria-expanded", String(nav.classList.toggle("open"))));
const menus = [...nav.querySelectorAll("details")];
for (const d of menus) d.addEventListener("toggle", () => { if (d.open) for (const o of menus) if (o !== d) o.open = false; });
document.addEventListener("click", (e) => {
  if (!e.target.closest(".nav-links details")) for (const d of menus) d.open = false;
  if (e.target.closest(".nav-links a")) { nav.classList.remove("open"); toggle.setAttribute("aria-expanded", "false"); }
});

// The live chart. A browser that opens the page as a file will not load a module or the engine, and a phone is too narrow for the
// task list (at least 420 px), so on those the screenshot stays and the note says why.
const stage = document.getElementById("demo-stage");
const status = document.getElementById("demo-status");
const note = document.getElementById("demo-note");
const bar = document.querySelector(".demo-bar");
const tools = [...bar.querySelectorAll(".tool")];
const crit = document.getElementById("crit");
const themeButton = document.getElementById("theme");
const LABELS = { fitout: "Office fit-out, 59 rows", 300: "300 tasks", 50000: "50,000 tasks" };
let chart = null, mount = null, theme = "light", busy = false;

const still = stage.innerHTML; // the screenshot, put back when the chart cannot start
const setTools = (on) => { for (const t of tools) t.disabled = !on; };
const fallback = (why) => {
  document.querySelector(".demo-frame").classList.add("still");
  setTools(false);
  status.textContent = "Screenshot";
  note.textContent = why;
};

async function show(kind) {
  if (busy) return;
  busy = true;
  setTools(false);
  status.textContent = `Opening ${LABELS[kind]}…`;
  try {
    let options = { schedule: { demo: Number(kind) } };
    if (kind === "fitout") {
      const res = await fetch("schedules/office-fitout.xml");
      if (!res.ok) throw new Error(`the sample answered ${res.status}`);
      options = { schedule: new File([await res.blob()], "office-fitout.xml", { type: "text/xml" }) };
    }
    if (chart) { chart.destroy(); chart = null; }
    const el = document.createElement("div");
    el.className = "gantt";
    el.dataset.theme = theme;
    stage.replaceChildren(el);
    await new Promise((r) => requestAnimationFrame(() => r()));
    const t0 = performance.now();
    chart = await mount(el, options);
    const seconds = (performance.now() - t0) / 1000;
    const backend = typeof chart.backend === "function" ? chart.backend() : chart.backend;
    status.innerHTML = `${LABELS[kind]} · opened in <strong>${seconds.toFixed(seconds < 0.1 ? 2 : 1)} s</strong> on this machine${typeof backend === "string" ? ` · ${backend}` : ""}`;
    for (const t of tools) if (t.dataset.load) t.setAttribute("aria-pressed", String(t.dataset.load === String(kind)));
    crit.setAttribute("aria-pressed", "false");
  } catch (err) {
    const why = err && err.message ? err.message : err;
    if (chart) status.textContent = `The chart did not start: ${why}`;
    else {
      stage.innerHTML = still;
      fallback(`This is a screenshot: the chart did not start (${why}).`);
    }
  } finally {
    busy = false;
    if (chart) setTools(true);
  }
}

bar.addEventListener("click", (e) => {
  const t = e.target.closest(".tool");
  if (!t || t.disabled || !chart) return;
  const c = chart.commands;
  if (t.dataset.load) show(t.dataset.load);
  else if (t.dataset.level) c.collapseToLevel(Number(t.dataset.level));
  else if (t.dataset.cmd) c[t.dataset.cmd]();
  else if (t === crit) {
    const on = crit.getAttribute("aria-pressed") !== "true";
    if (on) c.setFilter("critical"); else c.clearFilters();
    crit.setAttribute("aria-pressed", String(on));
  } else if (t === themeButton) {
    theme = theme === "light" ? "dark" : "light";
    themeButton.setAttribute("aria-pressed", String(theme === "dark"));
    const current = tools.find((x) => x.dataset.load && x.getAttribute("aria-pressed") === "true");
    show(current ? current.dataset.load : "fitout");
  }
});

if (matchMedia("(max-width: 700px)").matches) {
  fallback("This is a screenshot. The live chart runs on a wider screen, or open the demo.");
} else if (location.protocol === "file:") {
  fallback("This is a screenshot, because the page was opened as a file. Serve the staged site over http (docs/CONTRIBUTING.md says how) and the real chart mounts here.");
} else {
  try {
    ({ mount } = await import("./package/tachlon.js"));
    await show("fitout");
  } catch (err) {
    fallback(`This is a screenshot: the chart's script did not load (${err && err.message ? err.message : err}).`);
  }
}
