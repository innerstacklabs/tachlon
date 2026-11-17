// Prepares what the recording shows. Two steps, run by run.sh:
//   node build-pages.mjs app <work dir>    edits the new app's src/App.tsx, keeping the source the code view shows
//   node build-pages.mjs terms <work dir>  builds the terminal views from the real command output run.sh captured (cap-*.txt)
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [step, work] = process.argv.slice(2);
const pkg = process.env.PKG; // the package name, read from packages/gantt/package.json by run.sh
const www = join(work, "www");
const appDir = join(work, "demo", "my-app");
const files = join(www, "files");
mkdirSync(files, { recursive: true });

if (step === "app") {
  // Adds `import <name>` after the last import and `<jsx />` below <section id="center">, as a reader would.
  const edit = (name, from, jsx) => {
    const file = join(appDir, "src", "App.tsx");
    const lines = readFileSync(file, "utf8").split("\n");
    const last = lines.findLastIndex((l) => l.startsWith("import "));
    const slot = lines.findIndex((l) => l.includes('<section id="center">'));
    if (last < 0 || slot < 0) throw new Error("src/App.tsx has no import or no <section id=\"center\">: the Vite template changed");
    lines.splice(slot + 1, 0, jsx);
    lines.splice(last + 1, 0, `import ${name} from "${from}";`);
    writeFileSync(file, lines.join("\n"));
  };
  copyFileSync(join(appDir, "src", `${pkg}-chart.tsx`), join(files, `${pkg}-chart.tsx`));
  edit("Chart", `./${pkg}-chart`, '        <div style={{ width: "100%" }}><Chart /></div>');
  copyFileSync(join(appDir, "src", "App.tsx"), join(files, "App.stage1.tsx"));
  edit("OpenSchedule", "./open-schedule", "        <OpenSchedule />");
  copyFileSync(join(appDir, "src", "open-schedule.tsx"), join(files, "open-schedule.tsx"));
  // The page the plain-HTML segment shows and loads: pinned to the version npm installed, as a customer's link is.
  const version = JSON.parse(readFileSync(join(appDir, "node_modules", pkg, "package.json"), "utf8")).version;
  const cdn = join(www, "cdn", "index.html");
  writeFileSync(cdn, readFileSync(cdn, "utf8").replaceAll("__PKG__", pkg).replaceAll("__VERSION__", version));
  copyFileSync(cdn, join(files, "cdn-index.html"));
} else if (step === "terms") {
  // The captured text without the lines `drop` matches (a scratch path, the funding notice, the dev server's console
  // warnings), without doubled blank lines and without blank lines at either end.
  const cap = (name, drop = /(?!)/) => {
    const lines = readFileSync(join(work, `cap-${name}.txt`), "utf8").split("\n").filter((l) => !drop.test(l));
    return lines.filter((l, i) => l !== "" || lines[i - 1] !== "").join("\n").replace(/^\n+|\n+$/g, "").split("\n");
  };
  const cwd = "~/demo/my-app";
  const view = (n, steps) => writeFileSync(join(www, `term-${n}.json`), JSON.stringify(steps, null, 1));
  view(1, [
    { cwd: "~/demo", cmd: "npm create vite@latest my-app -- --template react-ts --no-interactive", out: cap("create", /^> |Scaffolding|^│$/) },
    { cwd: "~/demo", cmd: "cd my-app", out: [] },
    { cwd, cmd: `npm install ${pkg}`, out: cap("install", /funding|npm fund/) },
    { cwd, cmd: `npx ${pkg} init`, out: cap("init") },
  ]);
  view(3, [{ cwd, cmd: "npm run dev", out: cap("dev", /\[vite\]/) }]);
  view(6, [{ cwd, cmd: `npx ${pkg} doctor`, out: cap("doctor") }]);
} else {
  throw new Error("usage: node build-pages.mjs app|terms <work dir>");
}
