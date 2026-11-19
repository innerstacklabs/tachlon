// The host app of the Vue spec (gantt#1875): a Vue page that renders the wrapper as an application would, on tests/react-fake-core.ts, and keeps what the spec reads.
import { createApp, h, shallowRef } from "vue";
import { Gantt } from "../src/vue.ts";

const g = globalThis as { mounts?: { schedule?: { demo: number }; mustFinishBy?: string; locale?: string; texts?: Record<string, string>; capabilities?: object; save?: Record<symbol, () => ((changeSet: object) => unknown) | undefined> }[]; destroyed?: unknown[]; listened?: string[]; switched?: boolean[]; profiled?: unknown[] };
const seen = { change: 0, load: 0, ready: 0 };
const schedule = shallowRef<{ demo: number; fail?: number }>({ demo: 1 });
const locale = shallowRef<string | undefined>(undefined);
const mustFinishBy = shallowRef<string | undefined>(undefined);
const texts = shallowRef<Record<string, string> | undefined>(undefined);
const readOnly = shallowRef<boolean | undefined>(undefined);
const capabilities = shallowRef<object | undefined>(undefined);
const save = shallowRef<(() => Promise<string>) | object | undefined>(undefined);
const handlers = shallowRef<Record<string, () => void>>({ onChange: () => void (seen.change += 1), onLoad: () => void (seen.load += 1), onReady: () => void (seen.ready += 1) });
const app = createApp({ render: () => h(Gantt, { schedule: schedule.value, locale: locale.value, mustFinishBy: mustFinishBy.value, texts: texts.value, readOnly: readOnly.value, capabilities: capabilities.value, save: save.value as never, ...handlers.value }) });
const problems = { warnings: [] as string[], errors: 0 };
console.warn = (...parts: unknown[]) => void problems.warnings.push(parts.join(" ")); // Vue's own warn handler only sees warnings raised while it renders; an emit after the mount warns on the console
app.config.errorHandler = () => void (problems.errors += 1);
const report = async () => (await new Promise((done) => setTimeout(done, 300)), { ...seen, mounts: (g.mounts ?? []).map((o) => o.schedule?.demo), destroyed: g.destroyed?.length ?? 0, listened: [...(g.listened ?? [])].sort() });
Object.assign(window, {
  start: () => (app.mount(document.body.appendChild(document.createElement("div"))), report()),
  change: (demo: number) => ((schedule.value = { demo }), report()),
  lang: (tag: string) => ((locale.value = tag), report().then((seen) => ({ ...seen, locales: (g.mounts ?? []).map((o) => o.locale ?? "-") }))),
  finish: (day: string) => ((mustFinishBy.value = day), report().then(() => (g.mounts ?? []).map((o) => o.mustFinishBy))),
  words: (value: Record<string, string>) => ((texts.value = value), report().then(() => (g.mounts ?? []).map((o) => o.texts))),
  handle: (names: string[]) => ((handlers.value = Object.fromEntries(names.map((n) => [n, () => {}]))), report()),
  fail: () => ((schedule.value = { demo: 0, fail: 1 }), report()), // the stand-in's `mount` rejects for `{ fail: 1 }`
  flip: (on: boolean) => ((readOnly.value = on), report().then(() => ({ mounts: (g.mounts ?? []).length, switched: [...(g.switched ?? [])] }))),
  hold: () => { let release!: () => void; (globalThis as { gate?: Promise<void> }).gate = new Promise<void>((done) => (release = done)); const wait = (ms: number) => new Promise((done) => setTimeout(done, ms)); schedule.value = { demo: 2 }; return wait(100).then(() => ((readOnly.value = true), wait(100))).then(() => (release(), report())).then(() => [...(g.switched ?? [])]); },
  caps: (value?: object) => ((capabilities.value = value), report().then(() => ({ mounts: (g.mounts ?? []).map((o) => o.capabilities ?? null), profiled: [...(g.profiled ?? [])] }))),
  later: (on: boolean) => ((save.value = on ? async () => "sent" : undefined), report().then(async () => ({ mounts: (g.mounts ?? []).length, answer: (await g.mounts![0]!.save![Symbol.for("gantt.liveSave")]!()?.({})) ?? "none" }))),
  handlersForm: () => { const called: string[] = []; save.value = { updateTask: async (key: string) => void called.push(key) }; return report().then(async () => (await (g.mounts![0]!.save as unknown as (set: object) => Promise<unknown>)({ id: "a", seq: 1, source: "typed", changes: [{ record: "task", op: "update", key: "t1", set: {}, once: "a:0" }] }), called)); },
  problems: () => ({ ...problems }),
  stop: () => (app.unmount(), report()),
});
