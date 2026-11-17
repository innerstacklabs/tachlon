import { mount } from "tachlon";
import "tachlon/gantt.css";

const tasks = [
  { id: 1, name: "Design", start: "2026-11-02", duration: 3 },
  { id: 2, name: "Build", start: "2026-11-05", duration: 5 },
  { id: 3, name: "Ship", start: "2026-11-12", duration: 1 },
];
const links = [{ from: 1, to: 2 }, { from: 2, to: 3 }];

// forceWebgl: this page needs no GPU. Drop it to let the chart use WebGPU when the browser has it.
const chart = await mount(document.getElementById("chart"), { tasks, links, forceWebgl: true });
chart.on("select", ({ task }) => {
  document.getElementById("picked").textContent = `Picked: ${tasks.find((t) => t.id === task).name}`;
});
// destroy() stops the frame loop and empties the chart's element.
document.getElementById("stop").addEventListener("click", () => chart.destroy());
