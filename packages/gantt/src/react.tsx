// The React wrapper of the chart (gantt#1096), the package's `/react` entry: `<Gantt>` calls `mount` when it mounts, `destroy()` when it unmounts, takes the
// mount options as props and the chart's events as callbacks. It uses only what the entry exports, `mount` and its types, imported by relative path, so
// the page loads the chart once, whether it imports the package's main entry as well or not.
import { useEffect, useRef, useState, type CSSProperties, type JSX } from "react";
import { mount, type Gantt as Chart, type GanttEvents, type MountOptions } from "./index.js";
export type { ChangeSet, Save, SaveError, SaveResult } from "./index.js";

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
 * as `Gantt.on` does, so give one only when the app answers the key. The chart mounts again when `schedule`, `document`, `tasks`, `links`, `dataDate`, `mustFinishBy`, `calendars`, `durationUnit`, `lagCalendar`, `options`, `wasmUrl`, `locale`, `texts`, `fonts` or
 * `forceWebgl` changes (compared by reference, as a hook's dependencies are), so keep `tasks`, `links`, `texts` and `fonts` stable between renders.
 * `readOnly` and `capabilities` are not among them: changing one calls `Gantt.setReadOnly` or `Gantt.setCapabilities` on the chart that is mounted, and a chart that mounts again starts on the latest `capabilities`.
 * `save` is called as each change set is sent, with the latest function the component was given, so a new function never mounts the chart again.
 * `wasmUrl` is where the page serves the engine file, for an app that does not serve it next to the chart (a Next.js app serves it from `public/`).
 * `onReady(chart)` is called once each time the chart mounts, with the chart `mount` resolved to (to call `load` or `open` on it), and never
 * for a chart that was removed first; the chart is gone at the unmount, so drop what you kept. `className` and `style` are the chart's root,
 * where the theme tokens go; the root has no size of its own, so give it one. The page imports the chart's stylesheet. A start that fails is thrown
 * while rendering, for an error boundary.
 */
export type GanttProps = MountOptions & { className?: string; style?: CSSProperties; onReady?: (chart: Chart) => void } & {
  [K in keyof GanttEvents as (typeof EVENTS)[K]]?: (detail: GanttEvents[K]) => void;
};

type Listen = (name: string, handler: (detail: unknown) => void) => () => void;

export function Gantt(props: GanttProps): JSX.Element {
  const { schedule, document, tasks, links, dataDate, mustFinishBy, calendars, durationUnit, lagCalendar, options, forceWebgl, wasmUrl, locale, texts, fonts, readOnly, capabilities, className, style } = props;
  const root = useRef<HTMLDivElement>(null);
  const latest = useRef(props);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const live = useRef<{ chart: Chart; heard: Map<string, () => void>; loaded: boolean } | null>(null);
  const [failure, setFailure] = useState<{ error: unknown } | null>(null);
  if (failure !== null) throw failure.error;

  useEffect(() => {
    latest.current = props;
  });

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
    let started: Chart | undefined;
    let stopped = false;
    queue.current = queue.current.then(async () => {
      if (stopped) return;
      try {
        started = await mount(root.current as HTMLDivElement, { schedule, document, tasks, links, dataDate, mustFinishBy, calendars, durationUnit, lagCalendar, options, forceWebgl, wasmUrl, locale, texts, fonts, readOnly: latest.current.readOnly, capabilities: latest.current.capabilities, beforeEdit: (change) => latest.current.beforeEdit?.(change), save: latest.current.save === undefined ? undefined : (changeSet) => latest.current.save!(changeSet) });
      } catch (error) {
        if (!stopped) setFailure({ error });
        return;
      }
      if (stopped) return started.destroy();
      live.current = { chart: started, heard: new Map(), loaded: false };
      try { started.setReadOnly(latest.current.readOnly === true), started.setCapabilities(latest.current.capabilities); } catch (error) { return setFailure({ error }); } // a change while `mount` was pending found no chart to switch; a profile the chart rejects fails like a start
      listen();
      try {
        latest.current.onReady?.(started);
      } catch (error) {
        reportError(error); // a handler that throws must not stall the queue the next mount waits on
      }
    });
    return () => {
      stopped = true;
      live.current?.heard.forEach((stop) => stop());
      live.current = null;
      started?.destroy();
    };
  }, [schedule, document, tasks, links, dataDate, mustFinishBy, calendars, durationUnit, lagCalendar, options, forceWebgl, wasmUrl, locale, texts, fonts]);

  // `readOnly` switches the chart that is mounted and never mounts it again; a chart that mounts later starts on the latest value.
  useEffect(() => live.current?.chart.setReadOnly(readOnly === true), [readOnly]);
  useEffect(() => live.current?.chart.setCapabilities(capabilities), [capabilities]);

  // Only the events that have a handler are listened for, since a handler for some of them changes what a key does.
  const names = (Object.keys(EVENTS) as (keyof GanttEvents)[]).filter((name) => props[EVENTS[name]] !== undefined).join();
  useEffect(listen, [names]);

  return <div ref={root} className={className === undefined ? "gantt" : `gantt ${className}`} style={style} tabIndex={0} />;
}
