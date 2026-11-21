// The Angular wrapper of the chart (gantt#1875), the package's `/angular` entry: `Gantt` is a standalone component that calls `mount` once its view exists and
// `destroy()` when it is destroyed, takes the mount options as inputs and has an output for each event of `Gantt.on`. It uses only what the entry exports,
// `mount` and its types, imported by relative path, so the page loads the chart once, whether it imports the package's main entry as well or not.
// It is compiled by `ngc` (`pnpm build:ng`, tsconfig.ng.json), the way an Angular library is built: the partial compilation it writes is what an app's own build links.
import { Component, ElementRef, ErrorHandler, EventEmitter, Input, NgZone, Output, type AfterViewInit, type OnChanges, type OnDestroy, type SimpleChanges } from "@angular/core";
import { liveSave } from "./save-queue.js";
import { mount, type Fields, type Gantt as Chart, type GanttEvents, type MountOptions, type UpdateResult } from "./index.js";
export type { Handlers, SaveContext } from "./index.js";

const INPUTS = ["schedule", "document", "tasks", "links", "dataDate", "mustFinishBy", "calendars", "durationUnit", "dateForm", "constraintForm", "lagCalendar", "options", "forceWebgl", "wasmUrl", "locale", "texts", "fonts"] as const;
/** Each output and the event of `Gantt.on` it emits: `(chartToggleStatus)` is `toggle-status`. The `chart` in front keeps an output apart from the DOM event of the same name: Angular also listens on the element for `change` and `select`, which an input inside the chart fires and which bubble. */
const EVENTS = [["chartSelect", "select"], ["chartEdit", "edit"], ["chartMove", "move"], ["chartOpen", "open"], ["chartLoad", "load"], ["chartChange", "change"], ["chartError", "error"], ["chartFind", "find"], ["chartToggleStatus", "toggle-status"], ["chartChooseFile", "choose-file"], ["chartToggleKeys", "toggle-keys"]] as const satisfies readonly (readonly [string, keyof GanttEvents])[];
({}) as Record<(typeof EVENTS)[number][1], true> satisfies Record<keyof GanttEvents, true>; // an event of `GanttEvents` with no output here does not compile
type Listen = (name: string, handler: (detail: unknown) => void) => () => void;

/**
 * The mount options, one input each, and an output for each event, named `chart` and the event (see `EVENTS`): `(chartSelect)` gets `{ task, key }`, `(chartMove)` gets `{ task, key, start }` and so on through `GanttEvents`.
 * Give a listener for `chartFind` or `chartChooseFile` only when the app answers the key, since a listener takes that key from the browser, as `Gantt.on` does.
 * The chart mounts again when `schedule`, `document`, `tasks`, `links`, `dataDate`, `mustFinishBy`, `calendars`, `durationUnit`, `dateForm`, `constraintForm`, `lagCalendar`, `options`, `wasmUrl`, `locale`, `texts`, `fonts` or `forceWebgl` changes (compared by reference), so keep `tasks`, `links`, `texts` and `fonts` stable.
 * `readOnly` and `capabilities` are not among them: changing one calls `Gantt.setReadOnly` or `Gantt.setCapabilities` on the chart that is mounted, and a chart that mounts again starts on the latest `capabilities`.
 * `fields` is read when the chart mounts, never as it changes: a new value is used the next time the chart mounts for another prop.
 * `save` is read as each change set is sent: a function added or removed later takes effect at the next edit, and none mounts the chart again.
 * `records` is a new array of `Gantt.update` records: each one that arrives calls `update` on the chart that is mounted, in place, and `(update)` gets its answer, a refusal too. One that arrives while the chart starts is applied once it has, the latest only. The first value, and a value that comes with a change that mounts the chart again, is not applied: put those values in `tasks`.
 * `(ready)` gets the chart `mount` resolved to, once each time the chart mounts (to call `load` or `open` on it), and never for a chart that was removed first.
 * The component's element is the chart's root, which has no size of its own, so give it one. The page imports the chart's stylesheet. The chart runs outside
 * Angular's zone and each output emits inside it. A start that fails goes to the app's `ErrorHandler`.
 */
@Component({ selector: "gantt-chart", standalone: true, template: "", host: { tabindex: "0", class: "gantt" } })
export class Gantt<F extends Fields | undefined = undefined> implements OnChanges, AfterViewInit, OnDestroy {
  @Input() schedule?: MountOptions["schedule"];
  @Input() document?: MountOptions["document"];
  @Input() tasks?: MountOptions<F>["tasks"];
  @Input() links?: MountOptions<F>["links"];
  @Input() dataDate?: MountOptions["dataDate"];
  @Input() mustFinishBy?: MountOptions["mustFinishBy"];
  @Input() calendars?: MountOptions["calendars"];
  @Input() durationUnit?: MountOptions["durationUnit"];
  @Input() dateForm?: MountOptions["dateForm"];
  @Input() constraintForm?: MountOptions["constraintForm"];
  @Input() lagCalendar?: MountOptions["lagCalendar"];
  @Input() options?: MountOptions["options"];
  @Input() forceWebgl?: boolean;
  @Input() readOnly?: boolean;
  @Input() capabilities?: MountOptions["capabilities"];
  @Input() beforeEdit?: MountOptions["beforeEdit"];
  @Input() fields?: F;
  @Input() keys?: MountOptions["keys"];
  @Input() newKey?: MountOptions["newKey"];
  @Input() records?: Parameters<Chart<F>["update"]>[0];
  @Input() save?: MountOptions["save"];
  @Input() wasmUrl?: MountOptions["wasmUrl"];
  @Input() locale?: MountOptions["locale"];
  @Input() texts?: MountOptions["texts"];
  @Input() fonts?: MountOptions["fonts"];
  @Output() readonly chartSelect = new EventEmitter<GanttEvents["select"]>();
  @Output() readonly chartEdit = new EventEmitter<GanttEvents["edit"]>();
  @Output() readonly chartMove = new EventEmitter<GanttEvents["move"]>();
  @Output() readonly chartOpen = new EventEmitter<GanttEvents["open"]>();
  @Output() readonly chartLoad = new EventEmitter<GanttEvents["load"]>();
  @Output() readonly chartChange = new EventEmitter<GanttEvents["change"]>();
  @Output() readonly chartError = new EventEmitter<GanttEvents["error"]>();
  @Output() readonly chartFind = new EventEmitter<GanttEvents["find"]>();
  @Output() readonly chartToggleStatus = new EventEmitter<GanttEvents["toggle-status"]>();
  @Output() readonly chartChooseFile = new EventEmitter<GanttEvents["choose-file"]>();
  @Output() readonly chartToggleKeys = new EventEmitter<GanttEvents["toggle-keys"]>();
  @Output() readonly ready = new EventEmitter<Chart<F>>();
  @Output() readonly update = new EventEmitter<UpdateResult>();
  private queue: Promise<void> = Promise.resolve();
  private stopStart = (): void => {};
  private viewed = false;
  private applied?: Parameters<Chart<F>["update"]>[0]; // the `records` the chart started with or was last sent
  private chart?: Chart<F>;

  constructor(
    private readonly host: ElementRef<HTMLDivElement>,
    private readonly errors: ErrorHandler,
    private readonly zone: NgZone,
  ) {}

  ngOnChanges(changes: SimpleChanges): void {
    if (this.viewed && INPUTS.some((name) => name in changes)) this.start();
    if ("readOnly" in changes) this.chart?.setReadOnly(this.readOnly === true);
    if ("capabilities" in changes || "save" in changes) this.chart?.setCapabilities(this.capabilities); // a `save` that gains or drops `createLink` turns link drawing on or off
    if ("records" in changes) this.send();
  }
  ngAfterViewInit(): void {
    this.viewed = true;
    this.start();
  }
  ngOnDestroy(): void {
    this.stopStart();
  }

  // `update` is synchronous, so the chart that is mounted answers at once; with none mounted (a start is pending) the start applies `records` when it has settled.
  private send(): void {
    const { chart, records } = this;
    if (chart === undefined || records === undefined) return;
    const result = chart.update((this.applied = records));
    this.zone.run(() => this.update.emit(result));
  }

  // One start at a time: a start that began while the one before it was still mounting waits for it, so two charts never start in one root.
  // A start that a later one (or the destroy) has stopped never mounts, and a chart that mounts after that is destroyed. The outputs with a
  // subscriber when the chart mounts are listened for.
  private start(): void {
    this.stopStart();
    this.applied = this.records;
    let started: Chart<F> | undefined;
    let stopped = false;
    let stops: (() => void)[] = [];
    this.queue = this.queue.then(async () => {
      if (stopped) return;
      const { schedule, document, tasks, links, dataDate, mustFinishBy, calendars, durationUnit, dateForm, constraintForm, lagCalendar, options, forceWebgl, wasmUrl, locale, texts, fonts } = this;
      try {
        started = await this.zone.runOutsideAngular(() => mount<F>(this.host.nativeElement, { schedule, document, tasks, links, dataDate, mustFinishBy, calendars, durationUnit, dateForm, constraintForm, lagCalendar, options, forceWebgl, wasmUrl, locale, texts, fonts, fields: this.fields, keys: this.keys, newKey: this.newKey, readOnly: this.readOnly, capabilities: this.capabilities, beforeEdit: (change) => this.beforeEdit?.(change), save: liveSave(() => this.save) }));
      } catch (error) {
        return void (stopped || this.errors.handleError(error));
      }
      if (stopped) return started.destroy();
      const chart = (this.chart = started);
      chart.setReadOnly(this.readOnly === true); // a change while `mount` was pending found no chart to switch
      try { chart.setCapabilities(this.capabilities); } catch (error) { return void this.errors.handleError(error); } // a profile the chart rejects fails like a start
      stops = EVENTS.filter(([output]) => this[output].observed).map(([output, name]) => (chart.on as Listen)(name, (detail) => this.zone.run(() => (this[output] as EventEmitter<unknown>).emit(detail))));
      this.zone.run(() => this.ready.emit(chart));
      if (!stopped && this.records !== this.applied) this.send();
    });
    this.stopStart = () => {
      stopped = true;
      stops.forEach((stop) => stop());
      this.chart = undefined;
      started?.destroy();
    };
  }
}
