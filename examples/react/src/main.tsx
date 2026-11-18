import { useState } from "react";
import { createRoot } from "react-dom/client";
import "tachlon/gantt.css";
import { Gantt } from "tachlon/react";

// Keep `tasks` and `links` the same objects between renders: the chart starts again when they change.
const tasks = [
  { id: 1, name: "Design", key: "design", start: "2026-11-02", duration: 3 },
  { id: 2, name: "Build", key: "build", start: "2026-11-05", duration: 5 },
  { id: 3, name: "Ship", key: "ship", start: "2026-11-12", duration: 1 },
];
const links = [{ from: 1, to: 2 }, { from: 2, to: 3 }];

function App() {
  const [picked, setPicked] = useState<string>();
  const [shown, setShown] = useState(true);
  return (
    <>
      {/* forceWebgl: this app needs no GPU. Drop it to let the chart use WebGPU when the browser has it. */}
      {shown && <Gantt tasks={tasks} links={links} forceWebgl style={{ width: 800, height: 300 }} onSelect={({ key }) => setPicked(tasks.find((t) => t.key === key)?.name)} />}
      <p id="picked">{picked ? `Picked: ${picked}` : "Pick a task."}</p>
      {/* Taking <Gantt> out of the tree calls destroy(). */}
      <button id="stop" onClick={() => setShown(false)}>Stop</button>
    </>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
