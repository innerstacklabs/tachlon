// Open a P6 XER or MS Project XML file and draw it.
import { useState } from "react";
import { Gantt } from "__PKG__/react";
import "__PKG__/gantt.css";

export default function OpenSchedule() {
  const [file, setFile] = useState<File>();
  return (
    <div style={{ width: "100%" }}>
      <input type="file" accept=".xer,.xml" onChange={(e) => setFile(e.target.files?.[0])} />
      {file && <Gantt schedule={file} style={{ height: 480 }} />}
    </div>
  );
}
