// The typed constraint at the boundary (decision 0022, rule 9): a task has one member, `constraint`, `{ type, date }` or `null`, and the engine holds the same limit as the six members `snet`, `snlt`,
// `fnet`, `fnlt`, `mso` and `mfo`. These three functions turn one spelling into the other.
// It imports nothing: the plain-data reader, which the command-line tool builds without the page's types, uses it.

/** The kind of a date limit (decision 0022, rule 9): `snet`, `snlt`, `fnet` and `fnlt` are the floors and ceilings of a start and a finish, `mso` and `mfo` the pins, and `so` and `fo` ("start on", "finish on") a floor and a ceiling on one date, which does not move the bar and lets float go negative. */
export type ConstraintType = "snet" | "snlt" | "fnet" | "fnlt" | "mso" | "mfo" | "so" | "fo";

/** A task's one date limit, `{ type, date }`, or `null` for none. */
export type Constraint = { type: ConstraintType; date: string } | null;

type Row = Record<string, unknown>;

/** The six limit members, which the engine holds; `setData` compares each of them. `so` and `fo` are no member: each is two of these on one date. */
export const MEMBERS = ["snet", "snlt", "fnet", "fnlt", "mso", "mfo"] as const satisfies readonly ConstraintType[];
/** Every `ConstraintType`. */
const TYPES = [...MEMBERS, "so", "fo"] as const satisfies readonly ConstraintType[];
/** The four members `update` sets; `mso` and `mfo` it reads and sets no pin from (a pin the task holds changes nothing, any other is `notUpdatable`). */
const FLOORS_AND_CEILINGS = ["snet", "snlt", "fnet", "fnlt"] as const;
/** The order a task with several bounds reads its one constraint in (rule 9.5). */
const READING = ["mso", "mfo", "so", "fo", "snlt", "fnlt", "snet", "fnet"] as const satisfies readonly ConstraintType[];

const isConstraint = (value: unknown): value is Constraint => value === null || (typeof value === "object" && !Array.isArray(value) && TYPES.includes((value as Row).type as ConstraintType) && typeof (value as Row).date === "string");

/** The first of the six limit members that `task` gives beside a `constraint` (two spellings of one limit, rule 9.2), or `undefined`. */
export const spelledTwice = (task: Row): ConstraintType | undefined => (task.constraint === undefined ? undefined : MEMBERS.find((type) => task[type] !== undefined));

/**
 * `task` with its `constraint` written as the member its type names; a task with no `constraint` is returned as it is.
 * `forUpdate` is for `update`: a `start` beside the `constraint` is dropped, since the record's `constraint` wins (rule 9.5), and `null`, or a type, also sets the other three of `snet`, `snlt`, `fnet` and `fnlt` to `null`, so the one limit replaces every bound (a pin names no such members: `update` takes its own member only when the task holds that pin).
 * A row at mount keeps its `start`, which the plain-data reader requires and holds as `snet`.
 * Throws an `Error` for a `constraint` that is not `{ type, date }` or `null`, and for a task that gives it with one of the six members (two spellings of one limit, rule 9.2).
 */
export function toSeparate(task: Row, forUpdate = false): Row {
  if (typeof task !== "object" || task === null || task.constraint === undefined) return task; // a row that is not an object is for the chart to refuse
  const { constraint } = task;
  const rest = Object.fromEntries(Object.entries(task).filter(([name]) => name !== "constraint" && !(forUpdate && name === "start")));
  const twice = spelledTwice(task);
  if (twice !== undefined) throw new Error(`a task gives both "constraint" and "${twice}", two spellings of one date limit; give one.`);
  if (!isConstraint(constraint)) throw new Error('"constraint" is { type, date } with a type of snet, snlt, fnet, fnlt, mso, mfo, so or fo, or null (with a type member and a date member of your own, both or neither).');
  const cleared = forUpdate &&(constraint === null || !["mso", "mfo"].includes(constraint.type)) ? Object.fromEntries(FLOORS_AND_CEILINGS.map((type) => [type, null])) : {};
  return { ...rest, ...cleared, ...(constraint === null ? {} : membersOf(constraint)) };
}

/** The members a constraint is: the one its type names, or for `so` and `fo` the floor and the ceiling on its date. */
const membersOf = ({ type, date }: NonNullable<Constraint>): Row => (type === "so" ? { snet: date, snlt: date } : type === "fo" ? { fnet: date, fnlt: date } : { [type]: date });

/** The one constraint a set of members reads as, and how many bounds it left out: the first of `mso`, `mfo`, `so` (`snet` and `snlt` on one date), `fo` (`fnet` and `fnlt` on one date), `snlt`, `fnlt`, `snet`, `fnet` that is a date, the rest counted in `leftOut`; `null` when members name limits and none is a date; `undefined` when `members` names none. `kinds` is the host's `capabilities.kinds`: `so` and `fo` are read only when it is left out or lists them, as the engine's profile counts a floor and a ceiling on one instant as one kind only then. */
export function toTyped(members: Row, kinds?: readonly string[]): { constraint: Constraint | undefined; leftOut: number } {
  if (!MEMBERS.some((type) => type in members)) return { constraint: undefined, leftOut: 0 };
  const dated = (type: string): boolean => typeof members[type] === "string";
  const pair = (floor: string, ceiling: string): boolean => dated(floor) && members[floor] === members[ceiling];
  const allowed = (type: string): boolean => kinds === undefined || kinds.includes(type);
  const first = READING.find((type) => (type === "so" ? allowed(type) && pair("snet", "snlt") : type === "fo" ? allowed(type) && pair("fnet", "fnlt") : dated(type)));
  if (first === undefined) return { constraint: null, leftOut: 0 };
  const used = first === "so" || first === "fo" ? 2 : 1;
  const date = members[first === "so" ? "snet" : first === "fo" ? "fnet" : first] as string;
  return { constraint: { type: first, date }, leftOut: MEMBERS.filter(dated).length - used };
}

const folded = (members: Row | undefined, kinds?: readonly string[]): Row | undefined => {
  if (members === undefined) return undefined;
  const { constraint } = toTyped(members, kinds);
  return constraint === undefined ? members : { ...Object.fromEntries(Object.entries(members).filter(([name]) => !(MEMBERS as readonly string[]).includes(name))), constraint };
};

/** A task change whose `set` and `was` write the six limit members, written with one `constraint` instead (`typed` is `constraintForm`'s default); any other change as it is. `kinds` is as for `toTyped`. */
export function foldConstraint<C extends { record: string; set?: Row; was?: Row }>(change: C, kinds?: readonly string[]): C {
  if (change.record !== "task") return change;
  return { ...change, ...(change.set ? { set: folded(change.set, kinds) } : {}), ...(change.was ? { was: folded(change.was, kinds) } : {}) };
}
