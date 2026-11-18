// Every `npx <name> <command>` the site and the READMEs name is a command the CLI has (src/cli/run.ts: init, doctor, describe).
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { PRODUCT_NAME } from "../src/product.ts";

const root = new URL("../../../", import.meta.url);
const site = readdirSync(new URL("apps/site/", root)).filter((f) => f.endsWith(".html") || f === "llms.txt").map((f) => `apps/site/${f}`);
const files = [...site, "README.md", "packages/gantt/README.md", "packages/gantt/AGENTS.md"];

test("every_npx_command_named_on_the_site_and_in_the_readmes_is_init_doctor_or_describe", () => {
  expect(site.length, "the site pages are found").toBeGreaterThan(1);
  const named = files.flatMap((f) => [...readFileSync(new URL(f, root), "utf8").matchAll(new RegExp(`npx ${PRODUCT_NAME.toLowerCase()} ([a-z]+)`, "g"))].map((m) => `${f}: ${m[1]}`));
  expect(named.length, "the pages name commands").toBeGreaterThan(5);
  expect(named.filter((n) => !/: (init|doctor|describe)$/.test(n)), "a command the CLI does not have").toEqual([]);
});
