// The row filters' public names (gs#1194), the ones `Gantt.state().filter` reads and `Commands.setFilter` takes, and the one
// place they are mapped to the names in `App.status()` JSON. The status names are internal: renaming one there changes this table,
// never what a host sees.
import type { Status } from "./api.js";

/** The row filter that is on, one at a time. `"status"`, `"check"`, `"code"` and `"resource"` carry their values in `statusFilter`, `checkFilter`, `codeFilter` and `resourceFilter`. */
export type RowFilterName =
  | "off"
  | "critical"
  | "longestPath"
  | "nearCritical"
  | "slipped"
  | "floatLost"
  | "joinedCritical"
  | "leftCritical"
  | "moved"
  | "startConstraint"
  | "status"
  | "check"
  | "code"
  | "resource";

/** Each status name and its public name. Checked both ways, so a name added on either side without the other fails to compile. */
const PUBLIC_NAME = {
  off: "off",
  critical: "critical",
  longest: "longestPath",
  near: "nearCritical",
  slipped: "slipped",
  float: "floatLost",
  joined: "joinedCritical",
  left: "leftCritical",
  moved: "moved",
  constraint: "startConstraint",
  status: "status",
  check: "check",
  code: "code",
  resource: "resource",
} as const satisfies Record<Status["filter"], RowFilterName>;

/** A public name no status name maps to; `never` while the table is complete. */
type Unmapped = Exclude<RowFilterName, (typeof PUBLIC_NAME)[keyof typeof PUBLIC_NAME]>;
const everyPublicNameMapped: [Unmapped] extends [never] ? true : never = true;
void everyPublicNameMapped;

/** The public name of a status filter name. */
export const publicFilterName = (name: Status["filter"]): RowFilterName => PUBLIC_NAME[name];
