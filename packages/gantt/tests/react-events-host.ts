import { createElement as h } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { Gantt } from "../src/react.tsx";
const seen = { change: 0, load: 0 }, root = createRoot(document.body.appendChild(document.createElement("div"))), schedule = { demo: 1 };
const onChange = () => void (seen.change += 1), onLoad = () => void (seen.load += 1);
const props: Record<string, unknown> = {}, results: unknown[] = [], g = globalThis as { mounts?: unknown[]; updates?: unknown[] }; // gantt#2087: `put` keeps the props it was given, renders them with `onUpdate` and reports
const draw = () => h(Gantt, { schedule, ...props, onUpdate: (r: unknown) => void results.push(r), onReady: () => { const next = props.next; delete props.next; next && ((props.records = next), flushSync(() => root.render(draw()))); } }); // a `next` in `put` is the records the ready handler sets, in one synchronous render
Object.assign(window, { put: async (more: object) => (Object.assign(props, more), root.render(draw()), await new Promise((done) => setTimeout(done, 300)), { mounts: g.mounts?.length, updates: g.updates ?? [], results: [...results] }) });
Object.assign(window, { run: async (more: boolean) => (root.render(h(Gantt, { schedule, onChange, onLoad, ...(more ? { onSelect() {} } : {}) })), await new Promise((done) => setTimeout(done, 300)), { ...seen }) });
