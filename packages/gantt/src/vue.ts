// The Vue 3 wrapper of the chart (gantt#1875), the package's `/vue` entry: `<Gantt>` calls `mount` when it mounts, `destroy()` when it unmounts, takes the
// mount options as props and emits the chart's events. It uses only what the entry exports, `mount` and its types, imported by relative path, so
// the page loads the chart once, whether it imports the package's main entry as well or not.
import { camelize, defineComponent, getCurrentInstance, h, onBeforeUnmount, onMounted, onUpdated, shallowRef, toHandlerKey, watch, type PropType } from "vue";
import { liveSave } from "./save-queue.js";
import { mount, type Fields, type Gantt as Chart, type GanttEvents, type MountOptions, type UpdateResult } from "./index.js";
export type { Handlers, SaveContext } from "./index.js";

/** The events of `Gantt.on`, each one an emit of the component: `@select`, `@move`, `@toggle-status` and the rest of `GanttEvents`. */
const EVENTS = { select: null, edit: null, move: null, open: null, load: null, change: null, error: null, find: null, "toggle-status": null, "choose-file": null, "toggle-keys": null } as const satisfies Record<keyof GanttEvents, null>;
type Emits = { [K in keyof GanttEvents]: (detail: GanttEvents[K]) => true } & { ready: (chart: Chart<Fields | undefined>) => true; update: (result: UpdateResult) => true };
type Listen = (name: string, handler: (detail: unknown) => void) => () => void;

const component = defineComponent({
  name: "Gantt",
  props: {
    schedule: { type: null as unknown as PropType<NonNullable<MountOptions["schedule"]>>, default: undefined },
    document: { type: null as unknown as PropType<NonNullable<MountOptions["document"]>>, default: undefined },
    tasks: { type: Array as PropType<readonly object[]>, default: undefined },
    links: { type: Array as PropType<readonly object[]>, default: undefined },
    dataDate: { type: String, default: undefined },
    mustFinishBy: { type: String, default: undefined },
    calendars: { type: Array as PropType<NonNullable<MountOptions["calendars"]>>, default: undefined },
    durationUnit: { type: String as PropType<NonNullable<MountOptions["durationUnit"]>>, default: undefined },
    dateForm: { type: String as PropType<NonNullable<MountOptions["dateForm"]>>, default: undefined },
    lagCalendar: { type: [String, Object] as PropType<NonNullable<MountOptions["lagCalendar"]>>, default: undefined },
    options: { type: Object as PropType<NonNullable<MountOptions["options"]>>, default: undefined },
    forceWebgl: { type: Boolean, default: undefined },
    readOnly: { type: Boolean, default: undefined },
    capabilities: { type: Object as PropType<NonNullable<MountOptions["capabilities"]>>, default: undefined },
    beforeEdit: { type: Function as PropType<NonNullable<MountOptions["beforeEdit"]>>, default: undefined },
    fields: { type: Object as PropType<Fields>, default: undefined },
    records: { type: Array as PropType<readonly object[]>, default: undefined },
    save: { type: [Function, Object] as PropType<NonNullable<MountOptions["save"]>>, default: undefined },
    wasmUrl: { type: null as unknown as PropType<NonNullable<MountOptions["wasmUrl"]>>, default: undefined },
    locale: { type: String, default: undefined },
    texts: { type: Object as PropType<NonNullable<MountOptions["texts"]>>, default: undefined },
    fonts: { type: Array as PropType<NonNullable<MountOptions["fonts"]>>, default: undefined },
  },
  emits: { ...EVENTS, ready: null, update: null } as unknown as Emits,
  setup(props, { emit }) {
    const instance = getCurrentInstance()!;
    const root = shallowRef<HTMLDivElement | null>(null);
    const failure = shallowRef<{ error: unknown } | null>(null);
    let queue: Promise<void> = Promise.resolve();
    let live: { chart: Chart<Fields | undefined>; heard: Map<string, () => void>; loaded: boolean } | null = null;
    let stopStart = (): void => {};
    // Listen for the events that have a listener (and stop for the ones that lost theirs), on the chart that is mounted. It runs as soon as `mount`
    // settles, so an event the first frames fire is not lost, and a listener added later does not make the others listen again: `load` is told to
    // a listener added after it, so a second `Gantt.on` for it would emit `load` a second time.
    const listen = (): void => {
      const now = live;
      if (now === null) return;
      const wanted = (Object.keys(EVENTS) as (keyof GanttEvents)[]).filter((name) => instance.vnode.props?.[toHandlerKey(camelize(name))] !== undefined);
      for (const [name, stop] of now.heard) if (!wanted.includes(name as keyof GanttEvents)) (stop(), now.heard.delete(name));
      for (const name of wanted) {
        if (now.heard.has(name)) continue;
        const handler = (detail: unknown): void => {
          if (name !== "load" || !now.loaded) (emit as (name: string, detail: unknown) => void)(name, detail);
          now.loaded ||= name === "load";
        };
        now.heard.set(name, (now.chart.on as Listen)(name, handler));
      }
    };
    // `update` is synchronous, so the chart that is mounted answers at once; with none mounted (a start is pending) the start applies `records` when it has settled.
    const send = (records: readonly object[] | undefined): void => { if (live !== null && records !== undefined) emit("update", live.chart.update(records as never)); };
    // One start at a time: a start that began while the one before it was still mounting waits for it, so two charts never start in one root.
    // A start that a later one (or the unmount) has stopped never mounts, and a chart that mounts after that is destroyed.
    const start = (): void => {
      stopStart();
      failure.value = null;
      const given = props.records;
      let started: Chart<Fields | undefined> | undefined;
      let stopped = false;
      queue = queue.then(async () => {
        if (stopped) return;
        try {
          started = await mount<Fields | undefined>(root.value as HTMLDivElement, { schedule: props.schedule, document: props.document, tasks: props.tasks, links: props.links, dataDate: props.dataDate, mustFinishBy: props.mustFinishBy, calendars: props.calendars, durationUnit: props.durationUnit, dateForm: props.dateForm, lagCalendar: props.lagCalendar, options: props.options, forceWebgl: props.forceWebgl, wasmUrl: props.wasmUrl, locale: props.locale, texts: props.texts, fonts: props.fonts, fields: props.fields, readOnly: props.readOnly, capabilities: props.capabilities, beforeEdit: (change) => props.beforeEdit?.(change), save: liveSave(() => props.save) });
        } catch (error) {
          return void (stopped || (failure.value = { error }));
        }
        if (stopped) return started.destroy();
        live = { chart: started, heard: new Map(), loaded: false };
        started.setReadOnly(props.readOnly === true); // a change while `mount` was pending found no chart to switch
        try { started.setCapabilities(props.capabilities); } catch (error) { return void (failure.value = { error }); } // a profile the chart rejects fails like a start
        listen();
        emit("ready", started);
        if (props.records !== given) send(props.records);
      });
      stopStart = () => {
        (stopped = true), live?.heard.forEach((stop) => stop()), (live = null);
        started?.destroy();
      };
    };
    onMounted(start);
    watch(() => [props.schedule, props.document, props.tasks, props.links, props.dataDate, props.mustFinishBy, props.calendars, props.durationUnit, props.dateForm, props.lagCalendar, props.options, props.forceWebgl, props.wasmUrl, props.locale, props.texts, props.fonts, props.records], (now, before) => (now.slice(0, -1).some((value, i) => value !== before[i]) ? start() : send(props.records))); // `records` is last: any other prop that changed mounts again, in whatever order the host wrote them
    // `readOnly` and `capabilities` switch the chart that is mounted and never mount it again.
    watch(() => props.readOnly, (on) => live?.chart.setReadOnly(on === true));
    watch(() => props.capabilities, (profile) => live?.chart.setCapabilities(profile));
    onUpdated(listen);
    onBeforeUnmount(() => stopStart());
    return () => {
      if (failure.value !== null) throw failure.value.error;
      return h("div", { ref: root, class: "gantt", tabindex: 0 });
    };
  },
});

type Props = InstanceType<typeof component>["$props"];
type VueProps<F extends Fields | undefined> = Omit<Props, "tasks" | "links" | "fields" | "records" | "onReady"> & { tasks?: MountOptions<F>["tasks"]; links?: MountOptions<F>["links"]; fields?: F; records?: Parameters<Chart<F>["update"]>[0]; onReady?: (chart: Chart<F>) => void };
/**
 * The mount options, one prop each, and an emit for each event: `@select` gets `{ task, key }`, `@move` gets `{ task, key, start }` and so on through `GanttEvents`.
 * Give a listener for `find` or `choose-file` only when the app answers the key, since a listener takes that key from the browser, as `Gantt.on` does.
 * The chart mounts again when `schedule`, `document`, `tasks`, `links`, `dataDate`, `mustFinishBy`, `calendars`, `durationUnit`, `dateForm`, `lagCalendar`, `options`, `wasmUrl`, `locale`, `texts`, `fonts` or `forceWebgl` changes (compared by reference), so keep `tasks`, `links`, `texts` and `fonts` stable.
 * `readOnly` and `capabilities` are not among them: changing one calls `Gantt.setReadOnly` or `Gantt.setCapabilities` on the chart that is mounted, and a chart that mounts again starts on the latest `capabilities`.
 * `fields` is read when the chart mounts, never as it changes: a new value is used the next time the chart mounts for another prop.
 * `save` is read as each change set is sent: a function added or removed later takes effect at the next edit, and none mounts the chart again.
 * `records` is a new array of `Gantt.update` records: each one that arrives calls `update` on the chart that is mounted, in place, and `@update` gets its answer, a refusal too. One that arrives while the chart starts is applied once it has, the latest only. The first value, and a value that comes with a change that mounts the chart again, is not applied: put those values in `tasks`.
 * `@ready` gets the chart `mount` resolved to, once each time the chart mounts (to call `load` or `open` on it), and never for a chart that was removed first.
 * `class` and `style` land on the chart's root, which has no size of its own, so give it one. The page imports the chart's stylesheet. A start that fails is
 * thrown while rendering, for an `onErrorCaptured` hook or the app's `errorHandler`, until the next start clears it.
 * `F` is the type of `fields`: with a map, `tasks` and `links` take the host's own rows (`Rows`) as they are; with none every prop is what `MountOptions` says.
 */
export const Gantt = component as unknown as <F extends Fields | undefined = undefined>(props: VueProps<F>) => ReturnType<typeof h>;
