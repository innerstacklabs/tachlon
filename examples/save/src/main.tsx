import { Gantt, type Handlers } from "tachlon/react";
import "tachlon/gantt.css";
import { api, type Task, type Link } from "./api"; // your client: it sends c.once and c.signal, and throws on a non-2xx with a numeric `status`

const fields = { // the chart's names on the left, your API's on the right; any other member of a row comes back untouched
  tasks: { key: "id", name: "title", start: "plannedStart", duration: "days", parent: "parentId" },
  links: { key: "id", from: "source", to: "target" },
};

const save: Handlers = { // one call into your API per change, in the order the person made them
  createTask: (row, c) => api.createTask(row, c),
  updateTask: (key, set, c) => api.updateTask(key, set, c),
  deleteTask: (key, c) => api.deleteTask(key, c),
  createLink: (row, c) => api.createLink(row, c),
  updateLink: (key, set, c) => api.updateLink(key, set, c),
  deleteLink: (key, c) => api.deleteLink(key, c),
  updateSchedule: (set, c) => api.updateSchedule(set, c),
};

export function Schedule({ tasks, links, reload }: { tasks: Task[]; links: Link[]; reload: () => void }) {
  return <Gantt tasks={tasks} links={links} fields={fields} dateForm="day"
    save={save} onError={(e) => e.source === "save" && reload()} style={{ height: 480 }} />;
}
