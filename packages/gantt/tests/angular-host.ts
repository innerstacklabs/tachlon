// The host of the Angular spec (gantt#1875): what Angular does with a component, in its order (the inputs, `ngOnChanges`, the subscribers of the outputs, `ngAfterViewInit`, later
// changes, `ngOnDestroy`), on tests/angular-fake-core.ts and tests/react-fake-core.ts, and keeps what the spec reads.
import { Gantt } from "../src/angular.ts";

type Schedule = { demo?: number; fail?: number };
type Emitter = { subscribe(handler: () => void): void };
const g = globalThis as { mounts?: { schedule?: { demo: number }; locale?: string; texts?: Record<string, string>; fonts?: unknown; capabilities?: object; save?: Record<symbol, () => ((changeSet: object) => unknown) | undefined> }[]; destroyed?: unknown[]; listened?: string[]; switched?: boolean[]; profiled?: unknown[]; declared?: Record<string, { inputs: object; outputs: object }> };
const seen = { change: 0, load: 0, ready: 0 };
const zone = { outside: 0, inside: 0, runOutsideAngular: <T>(f: () => T): T => (zone.outside++, f()), run: <T>(f: () => T): T => (zone.inside++, f()) };
const errors: string[] = [];
const chart = new Gantt({ nativeElement: document.body } as never, { handleError: (e: Error) => errors.push(e.message) } as never, zone as never);
const outputs = chart as unknown as Record<string, Emitter>;
const report = async () => (await new Promise((done) => setTimeout(done, 300)), { ...seen, errors, zone: [zone.outside, zone.inside], mounts: (g.mounts ?? []).map((o) => o.schedule?.demo), destroyed: g.destroyed?.length ?? 0, listened: [...(g.listened ?? [])].sort() });
const set = (schedule: Schedule) => ((chart.schedule = schedule as never), chart.ngOnChanges({ schedule } as never));
const declaration = () => ({ ...g.declared!.component!, emitters: Object.keys(chart).filter((k) => typeof (chart as unknown as Record<string, { emit?: unknown }>)[k]?.emit === "function").sort(), options: Object.keys((g.mounts ?? [])[0] ?? {}).sort() });
Object.assign(window, {
  declaration,
  start: () => (set({ demo: 1 }), outputs.chartChange!.subscribe(() => void (seen.change += 1)), outputs.chartLoad!.subscribe(() => void (seen.load += 1)), outputs.ready!.subscribe(() => void (seen.ready += 1)), chart.ngAfterViewInit(), report()),
  change: (demo: number) => (set({ demo }), report()),
  words: (locale: string, texts: Record<string, string>, fonts: unknown) => ((chart.locale = locale), (chart.texts = texts), (chart.fonts = fonts as never), chart.ngOnChanges({ locale, texts, fonts } as never), report().then(() => (g.mounts ?? []).map((o) => [o.locale, o.texts, o.fonts]))),
  listen: () => (outputs.chartFind!.subscribe(() => {}), report()),
  flip: (on: boolean) => ((chart.readOnly = on), chart.ngOnChanges({ readOnly: on } as never), report().then(() => ({ mounts: (g.mounts ?? []).length, switched: [...(g.switched ?? [])] }))),
  caps: (value?: object) => ((chart.capabilities = value), chart.ngOnChanges({ capabilities: value } as never), report().then(() => ({ mounts: (g.mounts ?? []).map((o) => o.capabilities ?? null), profiled: [...(g.profiled ?? [])] }))),
  later: (on: boolean) => ((chart.save = on ? (async () => "sent") as never : undefined), chart.ngOnChanges({ save: chart.save } as never), report().then(async () => ({ mounts: (g.mounts ?? []).length, answer: (await g.mounts![0]!.save![Symbol.for("gantt.liveSave")]!()?.({})) ?? "none" }))),
  handlersForm: () => { const called: string[] = []; chart.save = { updateTask: async (key: string) => void called.push(key) } as never; chart.ngOnChanges({ save: chart.save } as never); return report().then(async () => (await (g.mounts![0]!.save as unknown as (set: object) => Promise<unknown>)({ id: "a", seq: 1, source: "typed", changes: [{ record: "task", op: "update", key: "t1", set: {}, once: "a:0" }] }), called)); },
  badCaps: () => { let release!: () => void; (globalThis as { gate?: Promise<void> }).gate = new Promise<void>((done) => (release = done)); const wait = (ms: number) => new Promise((done) => setTimeout(done, ms)); set({ demo: 2 }); return wait(100).then(() => ((chart.capabilities = { bad: 1 } as never), release(), report())).then(() => (set({ demo: 3 }), report())); },
  hold: () => { let release!: () => void; (globalThis as { gate?: Promise<void> }).gate = new Promise<void>((done) => (release = done)); const wait = (ms: number) => new Promise((done) => setTimeout(done, ms)); set({ demo: 2 }); return wait(100).then(() => ((chart.readOnly = true), chart.ngOnChanges({ readOnly: true } as never), wait(100))).then(() => (release(), report())).then(() => [...(g.switched ?? [])]); },
  fail: () => (set({ fail: 1 }), report()),
  stop: () => (chart.ngOnDestroy(), report()),
});
