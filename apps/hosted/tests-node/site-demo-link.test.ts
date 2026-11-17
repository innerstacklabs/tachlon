import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { PRODUCT_NAME } from "../../../packages/gantt/src/product.ts";

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), "utf8");

// The product page's button opens the hosted app on its 50,000-task schedule (gantt#1366), and the app builds a sample of that id.
test("the_product_pages_button_opens_the_50k_sample_the_app_builds", () => {
  const href = `https://app.${PRODUCT_NAME.toLowerCase()}.com/?sample=demo-50k`;
  expect(read("../../site/index.html")).toContain(`href="${href}">Open the 50,000-task demo</a>`);
  expect(read("../scripts/build-samples.mjs"), "the sample the link names exists").toContain('id: "demo-50k"');
});
