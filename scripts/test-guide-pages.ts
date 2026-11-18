// Checks the guide's rules, one fixture a rule (scripts/lib/guide-pages.ts). Run from anywhere: node scripts/test-guide-pages.ts
// Needs Node 22.18 or later (type stripping). Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { checkGuide } from "./lib/guide-pages.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-guide-pages: FAIL "${name}": got ${JSON.stringify(got)}, wanted ${JSON.stringify(want)}`);
};
// A guide of one feature file and its README, and the book page that includes the file's two anchors; each case changes one thing.
const control = "   <!-- control: printOn | opens the print dialog -->\n";
const file = `# Print\n<!-- needs: call:printOn -->\n<!-- ANCHOR: goal -->\n**Goal:** paper.\n<!-- ANCHOR_END: goal -->\n<!-- shot: page-1 | the first page -->\n\n## Steps\n<!-- ANCHOR: steps -->\n1. **Print.** Click **Save**.\n${control}2. **Wait.** The dialog opens.\n<!-- ANCHOR_END: steps -->\n`;
const page = "# Print\n{{#include ../../g/print.md:goal}}\n## By hand\n{{#include ../../g/print.md:steps}}\n";
const guide = (text = file, book = page, readme = "The format.\n"): string[] =>
  checkGuide({
    files: new Map([["g/print.md", text], ["g/README.md", readme]]),
    pages: new Map([["book/src/print.md", book]]),
    names: { option: new Set(["readOnly"]), event: new Set(["edit"]), call: new Set(["printOn"]) },
    labels: new Set(["Save"]),
    samples: ["office-fitout"],
  });

check("a good guide passes", guide(), []);
check("a README may show the comments and name no feature", guide(file, page, "<!-- needs: term --> and **Any word**\n"), []);
// Rule 1.
check("a picture fails", guide(file + '<img src="a.webp">\n'), ['g/print.md: "<img" is a picture, a class or an include: write a shot comment']);
check("a link to a page fails", guide(file + "[keys](keys.md)\n"), ['g/print.md: "](keys.md" is a sample link, or a link to a page or a path']);
check("a repository path fails", guide(file + "See `scripts/x.ts`.\n"), ['g/print.md: "scripts/" is a repository path, an issue id, a decision number or the proof: it stays in the book page']);
for (const bad of ["<video>", "<figure>", "media/a.webp", 'class="x"', "{{#include a}}", "?sample=a", "[a](/x)", "docs/x", "row #12", "decision 0021", "Where it is checked"]) check(`"${bad}" fails`, guide(`${file}${bad}\n`).length, 1);
check("a sample's name fails", guide(file + "Open the office fit-out.\n"), ['g/print.md: names the sample "office-fitout": write a neutral example']);
// Rule 2.
check("text under no needs comment fails", guide(file.replace("<!-- needs: call:printOn -->\n", "")), ['g/print.md: "Print" is under no needs comment', 'g/print.md: "Steps" is under no needs comment']);
check("a section's own needs comment covers that section only", guide(file.replace("<!-- needs: call:printOn -->\n", "").replace("## Steps\n", "## Steps\n<!-- needs: chart, edit, option:readOnly, event:edit -->\n")), ['g/print.md: "Print" is under no needs comment']);
check("a term that names nothing fails", guide(file.replace("call:printOn -->", "call:printOn, option:nope -->")), ['g/print.md: the needs term "option:nope" names nothing the package has']);
// Rule 3.
check("an anchor no page includes fails", guide(file, "# Print\n{{#include ../../g/print.md:goal}}\n"), ['g/print.md: the anchor "steps" is included by 0 book pages, not 1']);
check("a whole-file include fails", guide(file, page + "{{#include ../../g/print.md}}\n"), ["book/src/print.md: includes g/print.md whole: include it by anchor"]);
check("an include of a missing anchor fails", guide(file, page + "{{#include ../../g/print.md:gone}}\n"), ["a book page includes g/print.md:gone, an anchor that file does not have"]);
check("an anchor two pages include fails", guide(file, page + "{{#include ../../g/print.md:goal}}\n"), ['g/print.md: the anchor "goal" is included by 2 book pages, not 1']);
check("an anchor with no end fails", guide(file.replace("<!-- ANCHOR_END: goal -->\n", "")), ['g/print.md: the anchor "goal" has no ANCHOR_END after it']);
check("an end with no anchor fails", guide(file.replace("<!-- ANCHOR: goal -->\n", "")), ['g/print.md: the anchor "goal" has no ANCHOR before it', "a book page includes g/print.md:goal, an anchor that file does not have"]);
check("an end before its anchor fails", guide(file.replace("<!-- ANCHOR: goal -->\n**Goal:** paper.\n<!-- ANCHOR_END: goal -->\n", "<!-- ANCHOR_END: goal -->\n**Goal:** paper.\n<!-- ANCHOR: goal -->\n")), ['g/print.md: the anchor "goal" has its ANCHOR_END before its ANCHOR']);
check("a name used for two anchors fails", guide(file + "<!-- ANCHOR: goal -->\nMore.\n<!-- ANCHOR_END: goal -->\n"), ['g/print.md: the anchor "goal" is used for more than one anchor']);
// Rule 4.
check("a comment at the left margin of a list fails", guide(file.replace(control, control.trimStart())), [`g/print.md: the comment "${control.trim()}" at the left margin splits a list: indent it with its item`]);
check("an anchor line between two lists is not a comment in a list", guide(file + "- A bullet.\n"), []);
// Rule 5.
check("a call with no control fails", guide(file.replace(control, "")), ['g/print.md: "Print" needs a call and holds no control comment']);
check("a shot id used twice fails", guide(file + "<!-- shot: page-1 | again -->\n"), ['the shot id "page-1" is used 2 times, not once']);
// Rule 6.
check("a bold label with no text key fails", guide(file.replace("**Save**", "**Export\nall**")), ['g/print.md: the bold label "Export all" is the English of no text key']);
check("a bold label that opens a bullet is still a label", guide(file + "- **Export** saves a file.\n"), ['g/print.md: the bold label "Export" is the English of no text key']);
// Rule 7.
check("British spelling fails, in the README too", guide(file + "A colour.\n", page, "The centre.\n"), ['g/print.md: "colour" is British spelling', 'g/README.md: "centre" is British spelling']);

const run = spawnSync(process.execPath, [join(import.meta.dirname, "check-guide.ts"), join(import.meta.dirname, "..")], { encoding: "utf8" });
check("today's guide passes", [run.status, run.stderr], [0, ""]);

if (failed) process.exit(1);
console.log("test-guide-pages: ok");
