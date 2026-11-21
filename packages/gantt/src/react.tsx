// The React wrapper of the chart (gantt#1096), the package's `/react` entry: `<Gantt>` calls `mount` when it mounts, `destroy()` when it unmounts, takes the
// mount options as props and the chart's events as callbacks. It uses only what the entry exports, `mount` and its types, imported by relative path, so
// the page loads the chart once, whether it imports the package's main entry as well or not.
import { useEffect, useRef, useState, type CSSProperties, type JSX } from "react";
import { drawsLinks, liveSave } from "./save-queue.js";
import { mount, type Fields, type Gantt as Chart, type GanttEvents, type MountOptions, type UpdateResult } from "./index.js";
export type { ChangeSet, Handlers, Save, SaveContext, SaveError, SaveResult } from "./index.js";

/** The prop that takes each event of `Gantt.on`. */
const EVENTS = {
  select: "onSelect",
  edit: "onEdit",
  move: "onMove",
  open: "onOpen",
  load: "onLoad",
  change: "onChange",
  error: "onError",
  find: "onFind",
  "toggle-status": "onToggleStatus",
  "choose-file": "onChooseFile",
  "toggle-keys": "onToggleKeys",
} as const satisfies Record<keyof GanttEvents, string>;

/**
 * The mount options, one prop each, and a callback for each event: `onSelect({ task, key })`, `onMove({ task, key, start })` and the rest of `GanttEvents`.
 * A handler is called with the latest function the component was given. A handler for `find` or `choose-file` takes that key from the browser,
 * as `Gantt.on` does, so give one only when the app answers the key. The chart mounts again when `schedule`, `document`, `tasks`, `links`, `projectStart`, `mustFinishBy`, `calendars`, `durationUnit`, `dateForm`, `constraintForm`, `lagCalendar`, `options`, `wasmUrl`, `locale`, `texts`, `fonts` or
 * `forceWebgl` changes (compared by reference, as a hook's dependencies are), so keep `tasks`, `links`, `texts` and `fonts` stable between renders.
 * A new `dataDate` (a day written `YYYY-MM-DD`) is set on the chart that is mounted, as `Gantt.setData({ dataDate })` does, and mounts it again only when `setData` refuses it or the prop is taken away.
 * `readOnly` and `capabilities` are not among them: changing one calls `Gantt.setReadOnly` or `Gantt.setCapabilities` on the chart that is mounted, and a chart that mounts again starts on the latest `capabilities`.
 * `fields` is read when the chart mounts, never as it changes: a new value is used the next time the chart mounts for another prop.
 * `save` is read as each change set is sent: a function added or removed later takes effect at the next edit, and none mounts the chart again.
 * `records` is a new array of `Gantt.update` records: each one that arrives calls `update` on the chart that is mounted, in place, and `onUpdate(result)` gets its answer, a refusal too. One that arrives while the chart starts is applied once it has, the latest only. The first value, and a value that comes with a change that mounts the chart again, is not applied: put those values in `tasks`.
 * `wasmUrl` is where the page serves the engine file, for an app that does not serve it next to the chart (a Next.js app serves it from `public/`).
 * `onReady(chart)` is called once each time the chart mounts, with the chart `mount` resolved to (to call `load` or `open` on it), and never
 * for a chart that was removed first; the chart is gone at the unmount, so drop what you kept. `className` and `style` are the chart's root,
 * where the theme tokens go; the root has no size of its own, so give it one. The page imports the chart's stylesheet. A start that fails is thrown
 * while rendering, for an error boundary.
 */
export type GanttProps<F extends Fields | undefined = undefined> = MountOptions<F> & { className?: string; style?: CSSProperties; onReady?: (chart: Chart<F>) => void; records?: Parameters<Chart<F>["update"]>[0]; onUpdate?: (result: UpdateResult) => void } & {
  [K in keyof GanttEvents as (typeof EVENTS)[K]]?: (detail: GanttEvents[K]) => void;
};

type Listen = (name: string, handler: (detail: unknown) => void) => () => void;

/** `update` is synchronous, so a chart that is mounted answers at once; with none mounted (a start is pending) the start sends `records` when it has settled. */
function sendRecords<F extends Fields | undefined>(chart: Chart<F> | undefined, props: GanttProps<F>, applied: { current: GanttProps<F>["records"] }): void {
  if (chart !== undefined && props.records !== undefined) props.onUpdate?.(chart.update((applied.current = props.records)));
}

/** A changed `dataDate` is set on the chart that is mounted; one it refuses (`setData` holds none of your rows, or the day is not a day) or that is taken away starts the chart again (`count` goes up, and `ask` does it too). A start that is pending reads it when it has settled. */
function useDataDate<F extends Fields | undefined>(live: { current: { chart: Chart<F> } | null }, dataDate: string | undefined): { count: number; ask: () => void } {
  const [count, setCount] = useState(0); // the starts a `dataDate` that the chart could not take in place has asked for
  const ask = (): void => setCount((n) => n + 1);
  useEffect(() => {
    const chart = live.current?.chart;
    if (chart !== undefined && (dataDate === undefined || !chart.setData({ dataDate }).ok)) ask();
  }, [dataDate]);
  return { count, ask };
}

export function Gantt<F extends Fields | undefined = undefined>(props: GanttProps<F>): JSX.Element {
  const { schedule, document, tasks, links, projectStart, dataDate, mustFinishBy, calendars, durationUnit, dateForm, constraintForm, lagCalendar, options, forceWebgl, wasmUrl, locale, texts, fonts, readOnly, capabilities, records, className, style } = props;
  const root = useRef<HTMLDivElement>(null);
  const latest = useRef(props), applied = useRef<GanttProps<F>["records"]>(undefined); // `applied`: the `records` the chart started with or was last sent
  const queue = useRef<Promise<void>>(Promise.resolve());
  const live = useRef<{ chart: Chart<F>; heard: Map<string, () => void>; loaded: boolean } | null>(null);
  const [failure, setFailure] = useState<{ error: unknown } | null>(null);
  const redo = useDataDate(live, dataDate);
  if (failure !== null) throw failure.error;
  useEffect(() => { latest.current = props; });

  // Listen for the events that have a handler (and stop for the ones that lost theirs), on the chart that is mounted. It runs as soon as `mount`
  // settles, so an event the first frames fire is not lost, and a handler added later does not make the others listen again: `load` is told to
  // a handler added after it, so a second listener for it would call `onLoad` a second time.
  const listen = (): void => {
    const now = live.current;
    if (now === null) return;
    const wanted = (Object.keys(EVENTS) as (keyof GanttEvents)[]).filter((name) => latest.current[EVENTS[name]] !== undefined);
    for (const [name, stop] of now.heard) if (!wanted.includes(name as keyof GanttEvents)) (stop(), now.heard.delete(name));
    for (const name of wanted) {
      if (now.heard.has(name)) continue;
      const handler = (detail: unknown): void => {
        if (name === "load" && now.loaded) return;
        now.loaded ||= name === "load";
        (latest.current[EVENTS[name]] as ((d: unknown) => void) | undefined)?.(detail);
      };
      now.heard.set(name, (now.chart.on as Listen)(name, handler));
    }
  };

  // One start at a time: StrictMode runs this effect, its cleanup and the effect again before the first `mount` has settled, and two
  // charts must never start in one root. A start whose cleanup has already run never mounts.
  useEffect(() => {
    applied.current = latest.current.records;
    let started: Chart<F> | undefined;
    let stopped = false;
    queue.current = queue.current.then(async () => {
      if (stopped) return;
      try {
        started = await mount<F>(root.current as HTMLDivElement, { schedule, document, tasks, links, projectStart, dataDate, mustFinishBy, calendars, durationUnit, dateForm, constraintForm, lagCalendar, options, forceWebgl, wasmUrl, locale, texts, fonts, fields: latest.current.fields, keys: latest.current.keys, newKey: latest.current.newKey, readOnly: latest.current.readOnly, capabilities: latest.current.capabilities, beforeEdit: (change) => latest.current.beforeEdit?.(change), save: liveSave(() => latest.current.save) });
      } catch (error) {
        if (!stopped) setFailure({ error });
        return;
      }
      if (stopped) return started.destroy();
      live.current = { chart: started, heard: new Map(), loaded: false };
      try { started.setReadOnly(latest.current.readOnly === true), started.setCapabilities(latest.current.capabilities); } catch (error) { return setFailure({ error }); } // a change while `mount` was pending found no chart to switch; a profile the chart rejects fails like a start
      listen();
      for (const call of [() => latest.current.onReady?.(started as Chart<F>), () => !stopped && latest.current.records !== applied.current && sendRecords(live.current?.chart, latest.current, applied), () => !stopped && latest.current.dataDate !== dataDate && redo.ask()]) {
        try { call(); } catch (error) { reportError(error); } // a handler that throws must not stall the queue the next mount waits on
      }
    });
    return () => {
      stopped = true;
      live.current?.heard.forEach((stop) => stop());
      live.current = null;
      started?.destroy();
    };
  }, [schedule, document, tasks, links, projectStart, redo.count, mustFinishBy, calendars, durationUnit, dateForm, constraintForm, lagCalendar, options, forceWebgl, wasmUrl, locale, texts, fonts]);

  // `readOnly` switches the chart that is mounted and never mounts it again; a chart that mounts later starts on the latest value.
  useEffect(() => live.current?.chart.setReadOnly(readOnly === true), [readOnly]);
  useEffect(() => live.current?.chart.setCapabilities(capabilities), [capabilities, drawsLinks(props.save)]);
  useEffect(() => sendRecords(live.current?.chart, latest.current, applied), [records]);

  // Only the events that have a handler are listened for, since a handler for some of them changes what a key does.
  const names = (Object.keys(EVENTS) as (keyof GanttEvents)[]).filter((name) => props[EVENTS[name]] !== undefined).join();
  useEffect(listen, [names]);

  return <div ref={root} className={className === undefined ? "gantt" : `gantt ${className}`} style={style} tabIndex={0} />;
}
