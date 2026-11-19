// The typed constraint at the boundary (decision 0022, rule 9): a task has one member, `constraint`, `{ type, date }` or `null`, and the engine holds the same limit as the six members `snet`, `snlt`,
// `fnet`, `fnlt`, `mso` and `mfo`. These three functions turn one spelling into the other.
// It imports nothing: the plain-data reader, which the command-line tool builds without the page's types, uses it.

/** The kind of a date limit (decision 0022, rule 9): `snet`, `snlt`, `fnet` and `fnlt` are the floors and ceilings of a start and a finish, `mso` and `mfo` the pins. */
export type ConstraintType = "snet" | "snlt" | "fnet" | "fnlt" | "mso" | "mfo";

/** A task's one date limit, `{ type, date }`, or `null` for none. */
export type Constraint = { type: ConstraintType; date: string } | null;

type Row = Record<string, unknown>;

/** The six limit members, one per `ConstraintType`; `setData` compares each of them. */
export const TYPES = ["snet", "snlt", "fnet", "fnlt", "mso", "mfo"] as const satisfies readonly ConstraintType[];
/** The four members `update` takes; `mso` and `mfo` it answers `notUpdatable`. */
const FLOORS_AND_CEILINGS = ["snet", "snlt", "fnet", "fnlt"] as const;
/** The order a task with several bounds reads its one constraint in (rule 9.5). */
const READING = ["mso", "mfo", "snlt", "fnlt", "snet", "fnet"] as const satisfies readonly ConstraintType[];

const isConstraint = (value: unknown): value is Constraint => value === null || (typeof value === "object" && !Array.isArray(value) && TYPES.includes((value as Row).type as ConstraintType) && typeof (value as Row).date === "string");

/** The first of the six limit members that `task` gives beside a `constraint` (two spellings of one limit, rule 9.2), or `undefined`. */
export const spelledTwice = (task: Row): ConstraintType | undefined => (task.constraint === undefined ? undefined : TYPES.find((type) => task[type] !== undefined));

/**
 * `task` with its `constraint` written as the member its type names; a task with no `constraint` is returned as it is.
 * `forUpdate` is for `update`: a `start` beside the `constraint` is dropped, since the record's `constraint` wins (rule 9.5), and `null`, or a type, also sets the other three of `snet`, `snlt`, `fnet` and `fnlt` to `null`, so the one limit replaces every bound (a pin names no such members: `update` refuses it).
 * A row at mount keeps its `start`, which the plain-data reader requires and holds as `snet`.
 * Throws an `Error` for a `constraint` that is not `{ type, date }` or `null`, and for a task that gives it with one of the six members (two spellings of one limit, rule 9.2).
 */
export function toSeparate(task: Row, forUpdate = false): Row {
  if (typeof task !== "object" || task === null || task.constraint === undefined) return task; // a row that is not an object is for the chart to refuse
  const { constraint } = task;
  const rest = Object.fromEntries(Object.entries(task).filter(([name]) => name !== "constraint" && !(forUpdate && name === "start")));
  const twice = spelledTwice(task);
  if (twice !== undefined) throw new Error(`a task gives both "constraint" and "${twice}", two spellings of one date limit; give one.`);
  if (!isConstraint(constraint)) throw new Error('"constraint" is { type, date } with a type of snet, snlt, fnet, fnlt, mso or mfo, or null (with a type member and a date member of your own, both or neither).');
  const cleared = forUpdate &&(constraint === null || !["mso", "mfo"].includes(constraint.type)) ? Object.fromEntries(FLOORS_AND_CEILINGS.map((type) => [type, null])) : {};
  return { ...rest, ...cleared, ...(constraint === null ? {} : { [constraint.type]: constraint.date }) };
}

/** The one constraint a set of members reads as, and how many bounds it left out: the first of `mso`, `mfo`, `snlt`, `fnlt`, `snet`, `fnet` that is a date, the rest counted in `leftOut`; `null` when members name limits and none is a date; `undefined` when `members` names none. */
export function toTyped(members: Row): { constraint: Constraint | undefined; leftOut: number } {
  if (!TYPES.some((type) => type in members)) return { constraint: undefined, leftOut: 0 };
  const held = READING.filter((type) => typeof members[type] === "string");
  const [first] = held;
  return { constraint: first === undefined ? null : { type: first, date: members[first] as string }, leftOut: Math.max(0, held.length - 1) };
}

const folded = (members: Row | undefined): Row | undefined => {
  if (members === undefined) return undefined;
  const { constraint } = toTyped(members);
  return constraint === undefined ? members : { ...Object.fromEntries(Object.entries(members).filter(([name]) => !TYPES.includes(name as ConstraintType))), constraint };
};

/** A task change whose `set` and `was` write the six limit members, written with one `constraint` instead (`typed` is `constraintForm`'s default); any other change as it is. */
export function foldConstraint<C extends { record: string; set?: Row; was?: Row }>(change: C): C {
  if (change.record !== "task") return change;
  return { ...change, ...(change.set ? { set: folded(change.set) } : {}), ...(change.was ? { was: folded(change.was) } : {}) };
}
