// The names of the chart's columns and fields that the script writes while the chart runs (gantt#1875): a head cell the table adds, a row of the hover card,
// the name a screen reader says for a cell editor. `FIELDS_EN` and `TEXTS_EN` (`texts-en.ts`) give the English; the host's `texts` bundle laid over them
// gives its language. A column is told apart by its class (`start`, `actual-finish`), never by the text of its head.
import { FIELDS_EN, TEXTS_EN } from "./texts-en.ts"; // `.ts`, so the Node tier can load this file

const bundles = new WeakMap<object, Readonly<Record<string, string>>>();

/** Keep the host's `bundle` for the chart in `root`; `index.ts` calls it as the chart starts, and again on the restart. */
export function keepNames(root: object, bundle: Readonly<Record<string, string>> | undefined): void {
  bundles.set(root, bundle ?? {});
}

/** The name `key` has in the chart in `root`: the host's text, else the English. */
export function nameOf(root: object, key: string): string {
  return bundles.get(root)?.[key] ?? FIELDS_EN[key] ?? TEXTS_EN[key] ?? key;
}
