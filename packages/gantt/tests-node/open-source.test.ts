// The product is a gift (A devteam-1-1328): MIT, no key, no trial and no mark on the chart. Read as text, no page.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { PRODUCT_NAME } from "../src/product.ts";

const root = new URL("../../../", import.meta.url);
const read = (path: string): string => readFileSync(new URL(path, root), "utf8");

test("the_license_is_the_mit_text_in_inner_stack_labs_name_and_there_is_no_contributor_agreement", () => {
  const license = read("LICENSE");
  expect(license.startsWith("MIT License\n\nCopyright (c) 2026 Inner Stack Labs LLC\n")).toBe(true);
  expect(license).toContain("Permission is hereby granted, free of charge");
  expect(license).not.toMatch(/all rights reserved/i);
  expect(existsSync(new URL("CLA.md", root)), "CLA.md is deleted").toBe(false);
  expect(read("docs/CONTRIBUTING.md")).not.toMatch(/closed source|proprietary|contributor agreement \(CLA/i);
});

test("every_package_in_the_workspace_says_it_is_mit", () => {
  // The hosted-service code stays out of the public repository, so the floor is three: the package and two apps.
  const manifests = ["packages/gantt", ...readdirSync(new URL("apps/", root)).map((name) => `apps/${name}`)].filter((dir) => existsSync(new URL(`${dir}/package.json`, root)));
  expect(manifests.length, "the package and the apps with a package.json").toBeGreaterThanOrEqual(3);
  for (const dir of manifests) expect(JSON.parse(read(`${dir}/package.json`)).license, dir).toBe("MIT");
});

test("the_chart_has_no_license_key_option_no_trial_state_and_no_mark", () => {
  for (const gone of ["src/license.ts", "src/watermark.ts", "src/styles/watermark.css", "tests/watermark.spec.ts"]) expect(existsSync(new URL(`packages/gantt/${gone}`, root)), gone).toBe(false);
  const entries = ["index.ts", "react.tsx", "vue.ts", "angular.ts", "gantt-types.ts", "cli/skill.ts"].map((name) => read(`packages/gantt/src/${name}`)).join("\n");
  expect(entries).not.toMatch(/licenseKey|licenseState|installWatermark|watermark|Unlicensed/i);
  expect(read("packages/gantt/AGENTS.md")).not.toMatch(/licenseKey|watermark|Unlicensed/i);
});

test("the_package_readme_says_mit_and_names_no_key_or_trial", () => {
  const readme = read("packages/gantt/README.md");
  expect(readme).toContain(`${PRODUCT_NAME} is free and open source under the MIT license`);
  expect(readme).toContain("Copyright (c) 2026 Inner Stack Labs LLC.");
  expect(readme).not.toMatch(/licenseKey|Unlicensed|proprietary|license key|UNLICENSED/);
});

test("the_package_and_the_readme_name_the_public_repository", () => {
  const repo = `https://github.com/innerstacklabs/${PRODUCT_NAME.toLowerCase()}`;
  expect(JSON.parse(read("packages/gantt/package.json")).repository.url).toBe(`git+${repo}.git`);
  expect(read("README.md")).toContain(`${repo}/issues`);
  expect(read("crates/xer/README.md")).toContain("[top README](../../README.md)");
  expect(read("README.md") + read("crates/xer/README.md")).not.toMatch(new RegExp(`innerstacklabs/${["gan", "tt"].join("")}|daveremy/${PRODUCT_NAME.toLowerCase()}`));
});
