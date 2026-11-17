// The chart's direction (gantt#1875): a right-to-left locale sets `dir="rtl"` on the root; every other locale, and none, reads left to right.
import { test } from "node:test";
import { expect } from "@playwright/test";
import { dirOf, setLang } from "../src/locale.ts";

test("a right-to-left locale reads rtl and sets the root's dir; the rest and none read ltr", () => {
  for (const tag of ["ar", "ar-EG", "he", "fa", "ur", "ps", "sd", "yi", "ckb", "dv", "ug"]) expect(dirOf(tag), tag).toBe("rtl");
  expect([dirOf("de"), dirOf("pt-BR"), dirOf(undefined)], "de, pt-BR and no locale").toEqual(["ltr", "ltr", "ltr"]);
  const root = { lang: "", dir: "" };
  setLang(root, undefined);
  expect(root.dir, "no locale leaves the page's own").toBe("");
  setLang(root, "ar");
  expect(root.dir).toBe("rtl");
});
