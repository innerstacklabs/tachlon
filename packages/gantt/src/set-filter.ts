// Every filter the toolbar has, as one host command (gs#1194): `Commands.setFilter` maps a `Filter` to the app's own call
// for it, the same call the toolbar, the Filters menu and the keys make, so a host's control and the chart's do the same thing.
import type { PublicApp, Status } from "./api.js";
import { publicFilterName, type RowFilterName } from "./filter-names.js";
import type { CheckKind, CodeValue, StatusKind } from "./view-state.js";

/** The row filters that take no value. */
export type PlainFilter = Exclude<RowFilterName, "status" | "check" | "code" | "resource">;

/**
 * What `Commands.setFilter` takes. A row filter replaces the one that is on; `"off"` turns the row filter off. `{ status }`,
 * `{ check }`, `{ code }` and `{ resource }` are the row filters that take a value. `{ lookahead }` sets the window (a day `YYYY-MM-DD` and 1 to 520 weeks) or,
 * with null, removes it; the window combines with the row filter and leaves it as it is. `setFilter(state().filter)` does not
 * type-check for `"status"`, `"check"`, `"code"` and `"resource"` by design: those need their value (`{ status }`, `{ check }`, `{ code }`, `{ resource }`).
 */
export type Filter = PlainFilter | { status: StatusKind } | { check: CheckKind } | { code: CodeValue } | { resource: string } | { lookahead: { start: string; weeks: number } | null };

const STATUS_KINDS: readonly unknown[] = ["in-progress", "not-started", "complete"];
const CHECK_KINDS: readonly unknown[] = ["missing-logic", "open-start", "open-end", "high-float", "high-duration", "invalid-dates", "missing-resources", "missed", "hard-constraints", "negative-float"];
/** The longest lookahead window `App.set_lookahead` takes, in weeks; checked here, before the number reaches the module's u32. */
const MAX_WEEKS = 520;

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null;

/** Each row filter's on and off: the app's setter for it. */
function rowSetters(app: PublicApp): Record<Exclude<PlainFilter, "off">, (on: boolean) => boolean> {
  return {
    critical: (on) => app.set_critical_only(on),
    longestPath: (on) => app.set_longest_path(on),
    nearCritical: (on) => app.set_near_critical(on),
    slipped: (on) => app.set_slipped_only(on),
    floatLost: (on) => app.set_float_lost(on),
    joinedCritical: (on) => app.set_joined_critical(on),
    leftCritical: (on) => app.set_left_critical(on),
    moved: (on) => app.set_moved_only(on),
    startConstraint: (on) => app.set_start_constraint_filter(on),
  };
}

/** Turn the row filter that is on off, leaving the lookahead window. False when the filters cannot change now. */
function rowFilterOff(app: PublicApp, current: RowFilterName): boolean {
  if (current === "status") return app.set_status_filter("");
  if (current === "check") return app.set_check_filter("");
  if (current === "code") return app.clear_activity_code_filter();
  if (current === "resource") return app.set_resource_filter("");
  // With nothing on, the critical filter's off still says whether the filters can change now (no outline, or a bar press).
  return rowSetters(app)[current === "off" ? "critical" : current](false);
}

/**
 * Apply `filter`. True when the chart took it (also when it was already on); false, changing nothing, when it refused.
 * @internal
 */
export function setFilter(app: PublicApp, status: () => Status, filter: Filter): boolean {
  if (typeof filter === "string") {
    if (filter === "off") return rowFilterOff(app, publicFilterName(status().filter));
    const setters = rowSetters(app);
    // A name a host made up (`"toString"` included) is refused, not looked up on the object's prototype.
    return Object.hasOwn(setters, filter) && setters[filter as keyof typeof setters](true);
  }
  // A host's value is checked for its shape here, so a wrong one is refused with false, never thrown.
  const given: unknown = filter;
  if (!isRecord(given)) return false;
  if ("status" in given) return STATUS_KINDS.includes(given.status) && app.set_status_filter(given.status as string);
  if ("check" in given) return CHECK_KINDS.includes(given.check) && app.set_check_filter(given.check as string);
  if ("resource" in given) return typeof given.resource === "string" && given.resource !== "" && app.set_resource_filter(given.resource);
  if ("code" in given) {
    const code = given.code;
    return isRecord(code) && typeof code.type === "string" && typeof code.value === "string" && app.set_activity_code_filter(code.type, code.value);
  }
  if ("lookahead" in given) {
    const span = given.lookahead;
    if (span === null) return app.clear_lookahead();
    if (!isRecord(span) || typeof span.start !== "string" || typeof span.weeks !== "number") return false;
    return Number.isInteger(span.weeks) && span.weeks >= 1 && span.weeks <= MAX_WEEKS && app.set_lookahead(span.start, span.weeks);
  }
  return false;
}
