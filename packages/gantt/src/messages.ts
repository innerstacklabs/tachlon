// The sentences the chart writes while it runs (gantt#1875): opening, loading, saving, the selection, the row position, the baseline and a failed frame.
// Each is a template in `MESSAGES_EN` (`texts-en.ts`) with `{name}` holes, and the host's `texts` bundle, laid over it, gives its language.
// A hole that holds a number is written in the chart's number format. A template that depends on a count has one form per plural category of the
// chart's locale, in a key with the category after a dot (`bytes.one`, `bytes.few`, `bytes.many`, `bytes.other`); a language needs only the forms it uses.
import { DEFAULT_LOCALE, numberFormat } from "./locale.ts"; // `.ts`, so the Node tier can load this file
import { MESSAGES_EN } from "./texts-en.ts";

/** What a template's holes are filled with; a `count` that is a number also picks the template's plural form. */
export type Vars = Readonly<Record<string, string | number>>;

/** A sentence of the chart, by key. */
export type Say = (key: string, vars?: Vars) => string;

interface Kept {
  readonly bundle: Readonly<Record<string, string>>;
  readonly fmt: Intl.NumberFormat;
  readonly rules: Intl.PluralRules;
}

const kept = new WeakMap<object, Kept>();

/** The plural categories of the English templates: they are English, so the host's language does not pick among them. */
const ENGLISH = new Intl.PluralRules("en");

/** Keep the host's `bundle` and `locale` for the chart in `root`; `index.ts` calls it as the chart starts, and again on the restart. */
export function keepMessages(root: object, bundle: Readonly<Record<string, string>> | undefined, locale: string | undefined): void {
  kept.set(root, { bundle: bundle ?? {}, fmt: numberFormat(locale), rules: new Intl.PluralRules(locale ?? DEFAULT_LOCALE) });
}

/** The sentence `key` is in the chart in `root`, its holes filled from `vars`: the host's form for the count's plural category in the chart's locale, else its `.other` and its bare key, then the same three in English by English's plural categories, else `key` itself. */
export function say(root: object, key: string, vars: Vars = {}): string {
  const { bundle, fmt, rules } = kept.get(root) ?? { bundle: {}, fmt: numberFormat(undefined), rules: new Intl.PluralRules(DEFAULT_LOCALE) };
  const find = (texts: Readonly<Record<string, string>>, plural: Intl.PluralRules): string | undefined =>
    [typeof vars.count === "number" ? `${key}.${plural.select(vars.count)}` : "", `${key}.other`, key].map((form) => texts[form]).find((text) => text !== undefined);
  return (find(bundle, rules) ?? find(MESSAGES_EN, ENGLISH) ?? key).replace(/\{(\w+)\}/g, (hole, name: string) => {
    const value = vars[name];
    return value === undefined ? hole : typeof value === "number" ? fmt.format(value) : value;
  });
}

/** `say` for the chart in `root`, as a function a part can be handed. */
export const sayFor = (root: object): Say => (key, vars) => say(root, key, vars);
