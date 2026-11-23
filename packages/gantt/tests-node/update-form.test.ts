// `timedDate` (decision 0017, rule 7), the check `update`, `hostEdit` and `setData` make before the app is called: under "day" a date with a time of day is refused as badValue.
// `update` itself imports the wasm glue, which node cannot load; tests/update.spec.ts runs the whole call, a constraint date included.
import { timedDate } from "../src/date-form.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const TIMED = [["start", "2026-03-09T08:00"], ["snet", "2026-03-09T08:00"], ["snlt", "2026-03-20T10:00"], ["fnet", "2026-03-27T17:00"], ["fnlt", "2026-03-27T17:00"], ["mso", "2026-03-09T08:00"], ["mfo", "2026-03-27T17:00"], ["hostStart", "2026-03-09T08:00"], ["hostFinish", "2026-03-27T16:00"], ["hostLateStart", "2026-03-09T08:00"], ["hostLateFinish", "2026-03-27T16:00"], ["actualStart", "2026-03-09T08:00"], ["actualFinish", "2026-03-27T17:00"]] as const;

test("under day, a date with a time of day is badValue, naming the task, the member and the value", () => {
  for (const [member, value] of TIMED) {
    const answer = timedDate("day", [{ key: "k0", name: "x" }, { key: "k1", [member]: value }]);
    expect(answer, member).toMatchObject({ ok: false, reason: "badValue", record: 1, member, key: "k1", message: expect.stringContaining(`task k1 gives ${member} as "${value}"; this chart mounted with dateForm: "day"`) });
  }
  expect(timedDate("day", [{ snet: "2026-03-09T08:00" }]), "a record with no key").toMatchObject({ record: 0, message: expect.stringContaining("record 0 gives snet") });
});

test("guard: under minute, with no dateForm, for a date alone and for a value that is not text, nothing is refused", () => {
  for (const [member, value] of TIMED) {
    for (const form of ["minute", undefined] as const) expect(timedDate(form, [{ key: "k0", [member]: value }]), `${member} under ${form}`).toBeUndefined();
  }
  expect(timedDate("day", [{ key: "k0", snet: "2026-03-09", fnlt: "2026-03-27", hostFinish: "2026-03-27", name: "Task T" }]), "dates alone, and a name with a T").toBeUndefined();
  expect(timedDate("day", [{ key: "k0", snet: 20260309, fnlt: null, start: undefined }]), "other types are left to the app").toBeUndefined();
});
