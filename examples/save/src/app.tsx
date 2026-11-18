import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, type Link, type Task } from "./api";

// The host's own page: it reads the schedule and gives it to the chart. The spec asks for the server-keys copy with `?keys=server`.
const { Schedule } = await (new URLSearchParams(location.search).get("keys") === "server" ? import("./main-server-keys") : import("./main"));

function App() {
  const [data, setData] = useState<{ tasks: Task[]; links: Link[] }>();
  const reload = () => api.readSchedule().then(setData);
  useEffect(() => void reload(), []);
  // The rows the server changed on its own, as it pushes them: each one replaces the member it names, or is a new row.
  useEffect(() => {
    const events = new EventSource("/api/events");
    events.onmessage = (m) => {
      const rows: Task[] = JSON.parse(m.data).tasks;
      setData((d) => d && { ...d, tasks: [...d.tasks.map((t) => ({ ...t, ...rows.find((r) => r.id === t.id) })), ...rows.filter((r) => !d.tasks.some((t) => t.id === r.id))] });
    };
    return () => events.close();
  }, []);
  return data && <><button id="refresh" onClick={reload}>Read again</button><Schedule tasks={data.tasks} links={data.links} reload={reload} /></>;
}

createRoot(document.getElementById("root")!).render(<App />);
