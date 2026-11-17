// The v3 copy on the public page (tl#1048v3, set in the v4 layout): the AI section leads with the prompt for an AI assistant, the package README holds the same prompt,
// and the four sentences that waited on a check ship only where the check holds. Read as text, no page. The rest of the page is site-page.test.ts.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { PRODUCT_NAME } from "../src/product.ts";
import { DEFAULT_RESULTS } from "../../../scripts/bench-table.ts";

const read = (path: string): string => readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8");
const site = (name: string): string => read(`apps/site/${name}`);
const N = PRODUCT_NAME;
const n = N.toLowerCase();
const PROMPT = `Add a ${N} Gantt chart to this app. Install the npm package ${n} and follow the AGENTS.md inside it (more at ${n}.com/llms.txt). Put the chart on one existing page and change nothing else. Run the build and npx ${n} doctor until both pass. Commit on a new branch; do not push.`;
const results = JSON.parse(readFileSync(DEFAULT_RESULTS, "utf8"));

test("the_ai_section_leads_with_the_prompt_in_a_copy_box_then_the_by_hand_install", () => {
  const html = site("index.html");
  const ai = /<section class="ai" id="ai">[\s\S]*?<\/section>/.exec(html)?.[0] ?? "";
  expect(ai).toContain(`<pre id="prompt">${PROMPT}</pre>`);
  expect(ai, "one button copies the prompt").toContain('<button class="btn light small" type="button" data-copy="prompt"');
  expect(html, "the button's script is loaded").toContain('<script src="copy.js"></script>');
  const by = ai.indexOf("Or add it by hand:");
  expect(by, "the by-hand block follows the prompt").toBeGreaterThan(ai.indexOf(PROMPT));
  expect(ai.slice(by)).toContain(`<pre>npm install ${n}</pre>`);
  expect(ai.slice(by)).toContain(`import { mount } from "${n}";\nimport "${n}/gantt.css";\n\nconst chart = await mount(el, { tasks, links });`);
  expect(ai).toContain(`${N} is free and open source under the MIT license: no key, no trial and no mark on the chart.`);
});

test("the_readme_holds_the_same_prompt_as_the_site", () => {
  const readme = /Using an AI assistant\? Paste this:\n\n```text\n(.*)\n```/.exec(read("packages/gantt/README.md"))?.[1];
  const page = /<pre id="prompt">(.*)<\/pre>/.exec(site("index.html"))?.[1];
  expect(readme, "the README's prompt").toBe(PROMPT);
  expect(page, "the page's prompt").toBe(readme);
});

test("no_confirm_marker_ships_and_the_cut_sentences_stay_cut", () => {
  const all = ["index.html", "how-it-works.html", "llms.txt"].map(site).join("\n");
  expect(all).not.toMatch(/\[confirm/i);
  expect(all, "no code or page shows we hold no Bryntum license").not.toMatch(/hold a Bryntum license/);
  expect(all, "Syncfusion's own page does not name the two settings").not.toMatch(/auto-scheduling|critical path switched on/i);
});

test("the_shipped_confirm_items_hold_in_code", () => {
  const html = site("index.html");
  const types = read("packages/gantt/src/gantt-types.ts");
  expect(html).toContain("It writes both formats back out, and lists anything a format cannot carry.");
  expect(types, "file export is in the package types").toMatch(/exportXer\(\): ExportResult;[\s\S]*exportMspdi\(\): ExportResult;/);
  expect(html).toContain("click the chart and press <code>s</code> to see the open time and frame times on your own machine.");
  expect(read("README.md"), "the keys table says s shows the open time and the frame times").toContain("| `s` | Show or hide a line of live numbers: the open time and the frame times on the hosted page");
  expect(read("packages/gantt/src/events.ts"), "the chart reports the s key").toContain("\"toggle-status\"");
});

test("the_syncfusion_gap_uses_the_results", () => {
  const html = site("index.html");
  const [lo, hi] = [results.medians["tree4-50000"].dhtmlx.loadMedianMs, results.medians["chain-50000"].dhtmlx.loadMedianMs].map((ms: number) => (ms / 1000).toFixed(2));
  expect(html).toContain(`(54.2 s against ${lo} to ${hi} s)`);
  expect(html).toContain("<strong>What Syncfusion measured.</strong>");
  expect(html).not.toMatch(/never wait at all|Data centers/i);
  expect(html).not.toMatch(/SVAR Gantt \(MIT\)|not run/);
});

test("the_how_it_works_page_and_llms_txt_carry_the_v3_edits", () => {
  const how = site("how-it-works.html");
  expect(how).toContain("they make the browser lay them out again on every scroll.");
  expect(how).toContain("<h3>Is it open source? What does it cost?</h3>");
  expect(how).toContain(`Yes. ${N} is open source under the MIT license, free to use in any product, with no key and no trial.`);
  expect(site("llms.txt")).toContain(`> ${N} is a Gantt chart component for the browser, for any product with a schedule in it.`);
  expect(site("llms.txt")).toContain('It is open source under the MIT license, free to use, with no key and no trial.');
});
