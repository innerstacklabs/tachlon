// A stand-in for the chart (`src/index.ts`), for the React, Vue and Angular wrapper specs (gantt#1405): `mount` settles, then its chart fires `change` and `load`, after the wrapper's own continuation. It records the options each `mount` gets in `globalThis.mounts` (gantt#1494), each chart it makes in `globalThis.charts` (gantt#1522), each `destroy` in `globalThis.destroyed` and each event a chart is listened to for in `globalThis.listened` (gantt#1875) and each `setReadOnly` in `globalThis.switched` (gantt#2085), each `setCapabilities` in `globalThis.profiled`, each `update` in `globalThis.updates` and in its own chart's `got` (gantt#2087; a first record with the key `bad` is refused) and each `setData` in `globalThis.sets` (a task with the key `new` is refused as `rowsChanged`). A promise in `globalThis.gate` holds `mount` until it settles.
type H = (d: unknown) => void;
export async function mount(_root?: unknown, options?: unknown) {
  ((globalThis as { mounts?: unknown[] }).mounts ??= []).push(options);
  if ((options as { schedule?: { fail?: number } } | undefined)?.schedule?.fail) throw new Error("the start failed"); // gantt#1079: the schedule `{ fail: 1 }` makes `mount` reject
  const gate = (globalThis as { gate?: Promise<void> }).gate;
  if (gate) await gate;
  const hs = new Set<[string, H]>();
  let loaded = false;
  Promise.resolve().then(() => Promise.resolve().then(() => { for (const n of ["change", "load"]) hs.forEach(([k, h]) => k === n && h({})); loaded = true; }));
  const chart = { destroy() { ((globalThis as { destroyed?: unknown[] }).destroyed ??= []).push(chart); }, setReadOnly(on: boolean) { ((globalThis as { switched?: boolean[] }).switched ??= []).push(on); }, setCapabilities(profile?: object) { if ((profile as { bad?: unknown } | undefined)?.bad) throw new Error("bad profile"); ((globalThis as { profiled?: unknown[] }).profiled ??= []).push(profile ?? null); }, on(n: string, h: H) { ((globalThis as { listened?: string[] }).listened ??= []).push(n); const e: [string, H] = [n, h]; hs.add(e); if (n === "load" && loaded) queueMicrotask(() => hs.has(e) && h({})); return () => void hs.delete(e); },
    update(records: { key?: string }[]) { ((chart as { got?: unknown[] }).got ??= []).push(records); ((globalThis as { updates?: unknown[] }).updates ??= []).push(records); return records[0]?.key === "bad" ? { ok: false, reason: "unknownKey", message: "unknownKey", record: 0, key: "bad" } : { ok: true, updated: records.length }; },
    setData(data: { tasks?: { key?: string }[] }) { ((globalThis as { sets?: unknown[] }).sets ??= []).push(data); return data.tasks?.some((task) => task.key === "new") ? { ok: false, reason: "rowsChanged", message: "rowsChanged" } : { ok: true, updated: 1 }; } };
  ((globalThis as { charts?: unknown[] }).charts ??= []).push(chart);
  return chart;
}
