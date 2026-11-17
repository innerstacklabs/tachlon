// The host serves each page at its address with no ".html" (it redirects the file name there), so every address the site names for itself takes that form.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { PRODUCT_NAME } from "../src/product.ts";

const site = (name: string): string => readFileSync(new URL(`../../../apps/site/${name}`, import.meta.url), "utf8");

test("every_address_the_site_names_for_itself_ends_with_no_html", () => {
  const named: string[] = [];
  for (const page of ["index.html", "how-it-works.html", "features.html"]) named.push(...[...site(page).matchAll(/<link rel="canonical" href="([^"]*)">|<meta property="og:url" content="([^"]*)">/g)].map((m) => m[1] ?? m[2]!));
  named.push(...[...site("sitemap.xml").matchAll(/<loc>([^<]*)<\/loc>/g)].map((m) => m[1]!));
  named.push(...[...site("llms.txt").matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]!).filter((url) => url.startsWith(`https://${PRODUCT_NAME.toLowerCase()}.com/`)));
  expect(named.length, "a canonical link and an og:url for each of three pages, three sitemap entries and two llms.txt links").toBe(11);
  expect(named.filter((url) => url.endsWith(".html")), "addresses that end with .html").toEqual([]);
});
