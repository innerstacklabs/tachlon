import { createElement as h } from "react";
import { createRoot } from "react-dom/client";
import { Gantt } from "../src/react.tsx";
const seen = { change: 0, load: 0 }, root = createRoot(document.body.appendChild(document.createElement("div"))), schedule = { demo: 1 };
const onChange = () => void (seen.change += 1), onLoad = () => void (seen.load += 1);
Object.assign(window, { run: async (more: boolean) => (root.render(h(Gantt, { schedule, onChange, onLoad, ...(more ? { onSelect() {} } : {}) })), await new Promise((done) => setTimeout(done, 300)), { ...seen }) });
