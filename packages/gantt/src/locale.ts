// The language and region a chart runs in (gantt#1875): the page's `locale` option, a BCP 47 tag such as `de` or `pt-BR`.
// It sets the root's `lang` and `dir` and the chart's number format; with no locale the numbers read as `en-US` and the root's `lang` and `dir` are the page's own.
// The same module writes the chart's own words from `texts` and gives its font bytes to the engine.
// The developer page's status line (`status-line.ts`) keeps `en-US`: it is not part of a host page's chart.

import { TEXTS_EN } from "./texts-en.ts"; // `.ts`, so the Node tier can load this file
import type { PublicApp } from "./api.js";
import type { MountOptions } from "./gantt-types.js";

/** The region numbers show in when the page gives no `locale`. */
export const DEFAULT_LOCALE = "en-US";

/** Add the host's font faces and give the text engine its locale. */
export function setFonts(app: Pick<PublicApp, "set_fonts">, locale: string | undefined, fonts: MountOptions["fonts"]): void {
  if (locale === undefined && fonts === undefined) return;
  app.set_fonts(locale ?? DEFAULT_LOCALE, (fonts ?? []).map((f) => (f instanceof Uint8Array ? f : new Uint8Array(f))));
}

/** The number format of `locale` (`1.234,5` for `de`). Throws a `RangeError` for a tag that is not a language tag, so `mount` rejects before it touches the root. */
export function numberFormat(locale: string | undefined): Intl.NumberFormat {
  return new Intl.NumberFormat(locale ?? DEFAULT_LOCALE);
}

// The languages written right to left (a locale's first subtag).
const RTL_LANGUAGES: ReadonlySet<string> = new Set(["ar", "he", "fa", "ur", "ps", "sd", "yi", "ckb", "dv", "ug"]);

/** The direction `locale` reads in: `rtl` for Arabic, Hebrew, Persian, Urdu, Pashto, Sindhi, Yiddish, Sorani Kurdish, Dhivehi and Uyghur (`ar-EG`), `ltr` for the rest and for none. */
export function dirOf(locale: string | undefined): "ltr" | "rtl" {
  return RTL_LANGUAGES.has((locale ?? "").split("-")[0]!.toLowerCase()) ? "rtl" : "ltr";
}

/** Write `locale` as the root's `lang` and its direction as `dir` (the table, toolbar and panels read from it; the timeline stays left to right); with none the root keeps the page's own. */
export function setLang(root: Pick<HTMLElement, "lang" | "dir">, locale: string | undefined): void {
  if (locale === undefined) return;
  root.lang = locale;
  root.dir = dirOf(locale);
}

// The attributes `chart.html` takes a key in, and what each sets: `data-t` the element's text, the others the attribute of that name.
const SLOTS: ReadonlyArray<readonly [slot: string, attribute: string | null]> = [["data-t", null], ["data-t-title", "title"], ["data-t-aria-label", "aria-label"], ["data-t-placeholder", "placeholder"]];

/** Write the chart's own words (`TEXTS_EN`, with the host's `bundle` laid over it) into the markup in `root`; a key the bundle leaves out stays English. */
export function applyTexts(root: ParentNode, bundle: Readonly<Record<string, string>> | undefined): void {
  const texts = { ...TEXTS_EN, ...bundle };
  for (const el of root.querySelectorAll<HTMLElement>(SLOTS.map(([slot]) => `[${slot}]`).join())) {
    for (const [slot, attribute] of SLOTS) {
      const value = texts[el.getAttribute(slot) ?? ""];
      if (value === undefined) continue;
      if (attribute === null) el.textContent = value;
      else el.setAttribute(attribute, value);
    }
  }
}
