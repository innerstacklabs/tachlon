import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import type { Plugin } from "vite";
import config from "../vite.config.ts";
import { PRODUCT_NAME } from "../../../packages/gantt/src/product.ts";

// The source page keeps a neutral <title> (decision 0013); the config's transform writes the product's name at build time.
test("the_built_page_title_is_the_product_name", async () => {
  const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
  const transform = (config.plugins as Plugin[] | undefined)?.find((p) => p.name === "product-title")?.transformIndexHtml;
  expect(typeof transform, "vite.config.ts has the product-title plugin").toBe("function");
  const out = await (transform as (html: string) => string | Promise<string>)(html);
  expect(/<title>([^<]*)<\/title>/.exec(out)?.[1]).toBe(PRODUCT_NAME);
});
