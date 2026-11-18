import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, type Link, type Task } from "./api";

// The host's own page: it reads the schedule and gives it to the chart. The spec asks for the server-keys copy with `?keys=server`.
const { Schedule } = await (new URLSearchParams(location.search).get("keys") === "server" ? import("./main-server-keys") : import("./main"));

function App() {
  const [data, setData] = useState<{ tasks: Task[]; links: Link[] }>();
  const reload = () => api.readSchedule().then(setData);
  useEffect(() => void reload(), []);
  return data && <Schedule tasks={data.tasks} links={data.links} reload={reload} />;
}

createRoot(document.getElementById("root")!).render(<App />);
