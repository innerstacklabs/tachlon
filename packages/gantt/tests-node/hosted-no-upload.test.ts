// The README says the hosted page sends an opened file nowhere (gantt#1866): only samples.ts fetches, and it sends no body.
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
const root = new URL("../../../", import.meta.url);
test("the_hosted_page_fetches_only_samples_and_sends_no_body", () => {
  const files = ["hosted", "chrome"].flatMap((app) =>
    readdirSync(new URL(`apps/${app}/src/`, root)).filter((name) => name.endsWith(".ts")).map((name) => [name, readFileSync(new URL(`apps/${app}/src/${name}`, root), "utf8")]));
  expect(files.length, "the page's sources are found").toBeGreaterThan(20);
  for (const [name, source] of files) {
    if (source!.includes("fetch(")) expect(name, "only samples.ts fetches").toBe("samples.ts");
    if (name === "samples.ts") expect(source, "samples.ts sends no body").not.toMatch(/\bbody\s*:/);
  }
  expect(readFileSync(new URL("README.md", root), "utf8"), "the README says what the page does").toContain("sends it nowhere");
});
test("the_hosted_page_names_no_beacon_background_request_or_live_connection", () => {
  const sources = ["hosted", "chrome"].flatMap((app) =>
    readdirSync(new URL(`apps/${app}/src/`, root)).filter((name) => name.endsWith(".ts")).map((name) => [`${app}/${name}`, readFileSync(new URL(`apps/${app}/src/${name}`, root), "utf8")]));
  expect(sources.length, "the page's sources are found").toBeGreaterThan(20);
  for (const [name, source] of sources) expect(source, `${name} sends data only by the samples fetch`).not.toMatch(/sendBeacon|XMLHttpRequest|WebSocket|EventSource/);
});
