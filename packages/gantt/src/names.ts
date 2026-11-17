// The month and weekday names of a locale (gantt#1875), from `Intl`, in the text `App.set_locale` takes:
// `{"months":[12 names],"weekdays":[7 names],"weekStart":0}`, January and Monday first, the short form the locale writes (`Mär` for `de`); `weekStart` is the day its weeks start on, 0 Monday to 6 Sunday.

type Place = Intl.Locale & { getWeekInfo?: () => { firstDay: number }; weekInfo?: { firstDay: number } };

/** The short names of `locale` as `App.set_locale` reads them, with the host's `texts` (keys of `App.default_texts`) beside them. Throws a `RangeError` for a tag that is not a language tag. */
export function localeNames(locale: string, texts: Readonly<Record<string, string>> = {}): string {
  const name = (options: Intl.DateTimeFormatOptions, date: Date): string => new Intl.DateTimeFormat(locale, { ...options, timeZone: "UTC" }).format(date);
  const months = Array.from({ length: 12 }, (_, i) => name({ month: "short" }, new Date(Date.UTC(2026, i, 15))));
  // 2026-01-05 is a Monday.
  const weekdays = Array.from({ length: 7 }, (_, i) => name({ weekday: "short" }, new Date(Date.UTC(2026, 0, 5 + i))));
  // `firstDay` is 1 for Monday to 7 for Sunday; an engine that reports none gets Monday.
  const place = new Intl.Locale(locale) as Place;
  return JSON.stringify({ months, weekdays, weekStart: ((place.getWeekInfo?.() ?? place.weekInfo)?.firstDay ?? 1) - 1, texts });
}
