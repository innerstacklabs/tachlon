// The chart's locale (gantt#1875): the number format and the root's `lang` follow the page's `locale`; no page, no chart.
import { test } from "node:test";
import { expect } from "@playwright/test";
import { numberFormat, setLang } from "../src/locale.ts";
import { installRowPosition } from "../src/row-position.ts";

test("a locale decides the number format, and none reads as en-US", () => {
  expect(numberFormat("de").format(1234.5), "German writes a point for thousands and a comma for decimals").toBe("1.234,5");
  expect(numberFormat(undefined).format(1234.5), "no locale: the format the chart always had").toBe("1,234.5");
  expect(() => numberFormat("not a tag"), "a tag that is not a language tag throws, so `mount` rejects").toThrow(RangeError);
});

test("the locale is the root's `lang`, and no locale leaves the page's own", () => {
  const root = { lang: "fr", dir: "" };
  setLang(root, undefined);
  expect(root.lang).toBe("fr");
  setLang(root, "de");
  expect(root.lang).toBe("de");
});

test("the row position writes its numbers in the chart's format", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const label = { textContent: "", hidden: true };
  const root = { querySelector: () => label } as unknown as ParentNode;
  installRowPosition(root, numberFormat("de")).show(1000 * 20, 20, 2000);
  expect(label.textContent, "row 1.001 of 2.000, as a German reader writes it").toBe("Row 1.001 of 2.000");
});
