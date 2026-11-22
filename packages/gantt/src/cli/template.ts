// The chart module `init` writes into a project: three sample tasks, the stylesheet, and the call that removes the chart.
// `name` is the package's own name, read at run time (the product's name is written in one place only).

export type Kind = "plain" | "react";

const SAMPLE = `const tasks = [
  { id: 1, name: "Design", start: "2026-11-02", duration: 3 },
  { id: 2, name: "Build", start: "2026-11-05", duration: 5 },
  { id: 3, name: "Ship", start: "2026-11-12", duration: 1 },
];
const links = [
  { from: 1, to: 2 },
  { from: 2, to: 3 },
];
`;

/** The module's text: `kind` is a React component or a function for any page; `ts` adds the types; `next` marks it a client component and names the engine file in public/. */
export function chartModule(kind: Kind, ts: boolean, name: string, next = false): string {
  const use = next ? `"use client";\n` : "";
  const options = next ? `{ tasks, links, wasmUrl: "/app_bg.wasm" }` : "{ tasks, links }";
  if (kind === "plain") {
    return `${use}// Draws the chart in a div with the class "gantt" and a height. Call it once the page has the div; keep what it returns and call that to remove the chart.
// It needs no framework, but its two imports below resolve only through a bundler or an import map for "${name}" (with a <link rel="stylesheet"> for ${name}/gantt.css on the page).
import { mount } from "${name}";
import "${name}/gantt.css";

${SAMPLE}
export async function mountChart(element${ts ? ": HTMLDivElement" : ""})${ts ? ": Promise<() => void>" : ""} {
  const chart = await mount(element, ${options});
  return () => chart.destroy();
}
`;
  }
  return `${use}// The chart as a React component: <Chart /> draws it when it mounts and removes it when it unmounts.
// Pass onReady={(chart) => ...} to get the chart once it is drawn, to call chart.load({ file }) or chart.open(file) on it.
import { useEffect, useRef } from "react";
import { mount${ts ? ", type Gantt" : ""} } from "${name}";
import "${name}/gantt.css";

${SAMPLE}
export default function Chart({ onReady }${ts ? ": { onReady?: (chart: Gantt) => void }" : ""}) {
  const element = useRef${ts ? "<HTMLDivElement>" : ""}(null);
  useEffect(() => {
    let chart${ts ? ": Gantt | undefined" : ""};
    let unmounted = false;
    mount(element.current${ts ? " as HTMLDivElement" : ""}, ${options}).then((made) => {
      if (unmounted) made.destroy();
      else {
        chart = made;
        onReady?.(made);
      }
    }).catch((error) => console.error(error.message, error.fix));
    return () => {
      unmounted = true;
      chart?.destroy();
    };
  }, []);
  return <div ref={element} className="gantt" style={{ width: "100%", height: 300 }} />;
}
`;
}
