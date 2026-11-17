// The month and weekday names a locale gives the app (gantt#1875): from `Intl`, in the text `App.set_locale` takes.
import { test } from "node:test";
import { expect } from "@playwright/test";
import { localeNames } from "../src/names.ts";

const read = (locale: string): { months: string[]; weekdays: string[]; weekStart: number } => JSON.parse(localeNames(locale));

test("de writes March as Mär, and the week from Monday", () => {
  const { months, weekdays } = read("de");
  expect(months[2], "month 3 in German").toBe("Mär");
  expect(months).toHaveLength(12);
  expect(weekdays, "Monday first").toEqual(["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"]);
  expect(read("de").weekStart, "weeks start on Monday").toBe(0);
});

test("the host's texts go beside the names", () => {
  expect((JSON.parse(localeNames("de", { emptyName: "x" })) as { texts: unknown }).texts).toEqual({ emptyName: "x" });
});

test("en reads as the app's own names, and a tag that is not a language tag throws", () => {
  expect(read("en").months.slice(0, 3)).toEqual(["Jan", "Feb", "Mar"]);
  expect(read("en").weekdays[0]).toBe("Mon");
  expect(read("en").weekStart, "US weeks start on Sunday").toBe(6);
  expect(() => localeNames("not a tag"), "mount rejects such a tag before this runs").toThrow(RangeError);
});
