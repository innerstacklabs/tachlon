// What the package does not export yet (decision 0022, rule 3.1). The row that exports it deletes this file and `main.tsx` imports from "tachlon/react" again.
export { Gantt } from "tachlon/react";
type Row = Record<string, unknown>;
export interface SaveContext { once: string; signal: AbortSignal; was?: Row; changeSet: unknown }
type Call<A extends unknown[]> = (...args: [...A, SaveContext]) => Promise<unknown>;
export interface Handlers {
  createTask?: Call<[Row]>; updateTask?: Call<[string, Row]>; deleteTask?: Call<[string]>;
  createLink?: Call<[Row]>; updateLink?: Call<[string, Row]>; deleteLink?: Call<[string]>; updateSchedule?: Call<[Row]>;
}
