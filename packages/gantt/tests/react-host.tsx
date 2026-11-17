// The host app of the StrictMode spec: a React page that renders the wrapper as an application would, and keeps what the spec reads.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Gantt } from "../src/react.tsx";

const app = createRoot(document.getElementById("box")!);
const seen = { select: [] as number[], load: 0 };
const tree = (
  <StrictMode>
    <Gantt schedule={{ demo: 200 }} forceWebgl style={{ width: 1000, height: 500 }} onLoad={() => void (seen.load += 1)} onSelect={({ task }) => void seen.select.push(task)} />
  </StrictMode>
);
window.host = { seen, render: () => app.render(tree), unmount: () => app.unmount() };
