// What a host's answer means (decision 0022, rule 3.5). `judge` reads one throw or one resolved `Response` of `save` or a handler, in this order:
// a thrown `SaveRefused` is a refusal; then `classify(error)` for each other throw; then the status, by the table; then no status is a failure.
// A number `classify` returns from 100 to 599 is judged by the table, and after `classify` returns a number outside that range the chart goes to the last step, a failure.
// The table: a resolved 2xx is saved and a thrown one a failure; 401 stops; 408, 429 and 5xx fail; any other status, a 403 included, is a refusal.

/** A handlers object's `classify`: what a throw means. A number is read as a status, `undefined` leaves the error to the table. */
export type Classify = (error: unknown) => number | "refused" | "stopped" | "failed" | undefined;

/** What one answer means: saved; failed (repeat it); stopped (the person signs in again); refused (the server said no, with the line to show and the row and field it names). */
export type Verdict = { is: "saved" | "failed" | "stopped" } | { is: "refused"; message: string; key?: string; field?: string };

const inRange = (n: unknown): n is number => typeof n === "number" && Number.isInteger(n) && n >= 100 && n <= 599;
const record = (v: unknown): Record<string, unknown> | undefined => (typeof v === "object" && v !== null ? (v as Record<string, unknown>) : undefined);
const text = (v: unknown): string | undefined => (typeof v === "string" && v !== "" ? v : undefined);

/** The first number from 100 to 599: a `Response`'s `status`, else a thrown error's `status`, `response.status` or `statusCode`. */
export function statusOf(value: unknown): number | undefined {
  if (value instanceof Response) return inRange(value.status) ? value.status : undefined;
  const error = record(value);
  return [error?.status, record(error?.response)?.status, error?.statusCode].find(inRange);
}

/** The table of rule 3.5, for an answer that resolved or was thrown. */
function byStatus(status: number | undefined, thrown: boolean): "saved" | "failed" | "stopped" | "refused" {
  if (status === undefined) return "failed";
  if (status >= 200 && status < 300) return thrown ? "failed" : "saved";
  return status === 401 ? "stopped" : status === 408 || status === 429 || status >= 500 ? "failed" : "refused";
}

/** A refusal's line, from the thrown error's `message` or the `message` of a `Response`'s JSON body, else the chart's own with the status; `key` and `field` from the body. */
async function refusal(value: unknown, status: number | undefined): Promise<Verdict> {
  const body = value instanceof Response && !value.bodyUsed ? record(await value.clone().json().catch(() => undefined)) : undefined;
  const message = text(record(value)?.message) ?? text(body?.message) ?? (status === undefined ? "The server refused the change" : `The server refused the change (${status})`);
  return { is: "refused", message, ...keyed(body) };
}

/** The row and field a refusal names, as the members `Verdict` has. */
function keyed(from: Record<string, unknown> | undefined): { key?: string; field?: string } {
  const [key, field] = [text(from?.key), text(from?.field)];
  return { ...(key === undefined ? {} : { key }), ...(field === undefined ? {} : { field }) };
}

/** Judge one throw (`thrown`) or one resolved `Response`. */
export async function judge(value: unknown, thrown: boolean, classify?: Classify): Promise<Verdict> {
  // A `SaveRefused` (save-queue.ts) is found by its name, so two copies of the class agree.
  if (value instanceof Error && value.name === "SaveRefused") return { is: "refused", message: value.message, ...keyed(record(value)) };
  const named = thrown ? classify?.(value) : undefined;
  const status = typeof named === "number" ? (inRange(named) ? named : undefined) : statusOf(value);
  const is = typeof named === "string" ? named : byStatus(status, thrown);
  return is === "refused" ? refusal(value, status) : { is };
}
