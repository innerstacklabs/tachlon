// A TypeScript host with a field map passes its own row type to `mount`, `update`, `setData` and each wrapper with no cast, and a host without one gets the types it had.
// The file has no runtime test: `tsc --noEmit` (`pnpm typecheck`) compiles it, and it fails there without `Fields`, `Rows` and the type parameter `F`.
import type * as Lib from "../src/index.ts";
import type { Fields, Gantt as Chart, TaskUpdate } from "../src/index.ts";
import type { Gantt as AngularGantt } from "../src/angular.ts";
import type { Gantt as ReactGantt } from "../src/react.tsx";
import type { Gantt as VueGantt } from "../src/vue.ts";

interface Row { id: string; title: string; plannedStart: string; days: number; parentId?: string }
interface Dep { source: string; target: string }
const fields = { tasks: { name: "title", start: "plannedStart", duration: "days", parent: "parentId", key: "id" }, links: { from: "source", to: "target" } } satisfies Fields;
type F = typeof fields;
declare const mount: typeof Lib.mount; // the chart itself loads in a browser: only its types are used here
declare const reactGantt: typeof ReactGantt;
declare const vueGantt: typeof VueGantt;
type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;

export async function hostTypes(root: HTMLDivElement, rows: Row[], deps: Dep[]): Promise<void> {
  const chart = await mount(root, { tasks: rows, links: deps, fields });
  chart.update([{ id: "a", days: 3 }]);
  chart.setData({ tasks: rows, links: deps });

  // The wrappers are called as a host does, with no `<F>`: `F` comes from `fields`, and a host with no map keeps the chart that `ready` hands it.
  reactGantt({ tasks: rows, links: deps, fields, onReady: (ready) => void ready.update([{ id: "a", days: 3 }]) });
  vueGantt({ tasks: rows, links: deps, fields, onReady: (ready) => void ready.setData({ tasks: rows }) });
  vueGantt({ tasks: [{ id: "a", name: "A", start: "2026-03-02" }], onReady: (ready) => void ready.update([{ key: "a", name: "B" }]) });
  vueGantt({ onReady: (ready) => void (ready satisfies Chart) });
  // @ts-expect-error host rows with no `fields` are refused
  vueGantt({ tasks: rows });
  // @ts-expect-error host rows with no `fields` are refused
  reactGantt({ tasks: rows });
  // @ts-expect-error the chart a no-map host is handed has no `days` member
  vueGantt({ onReady: (ready) => void ready.update([{ key: "a", days: 3 }]) });
  const angular = {} as AngularGantt<F>;
  angular.tasks = rows;
  angular.links = deps;
  angular.fields = fields;
  void angular;

  // @ts-expect-error "nme" is not a task field; with `[chartField: string]: string` this line compiled
  await mount(root, { tasks: rows, fields: { tasks: { nme: "title" } } });

  // With no map every type is what it was: the host's own rows are refused, and the chart's own are taken.
  // @ts-expect-error a `Row` is not a `PlainTask` when the chart has no map
  await mount(root, { tasks: rows });
  const plain = await mount(root, { tasks: [{ id: "a", name: "A", start: "2026-03-02" }] });
  // @ts-expect-error `days` is not a member of a `TaskUpdate`
  plain.update([{ key: "a", days: 3 }]);
  const same: [Equal<Parameters<Chart["update"]>[0], readonly TaskUpdate[]>, Equal<Parameters<typeof plain.update>[0], readonly TaskUpdate[]>] = [true, true];
  void same;
}
