// Writes fit-out.xer, the P6 file the README story opens (gs#393): a made-up office fit-out of 26 activities, five weeks in.
// Nothing real is named. It has what the story shows and the hosted samples lack: actual starts and finishes, a data date, links of
// all the kinds a scheduler uses (FS, SS, FF, with lag) and one activity code type, "Trade", with seven values.
//
//   node demos/screencast/fixtures/build-fit-out.mjs        rewrites demos/screencast/fixtures/fit-out.xer
//
// The file is committed; this script is how it was made, so a change to the table is a change here and a rewrite. Every date is a
// working day (Monday to Friday, 8 hours) counted from START; the actuals of the started activities come from the same pass that
// places them, so the file is consistent without asking the importer to reconcile it.

import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const START = Date.UTC(2026, 8, 21); // Monday 21 September 2026
const DATA_DAY = 25; // working day 25 is Monday 26 October 2026: the data date, 08:00

const TRADES = ["General", "Demolition", "Framing", "Electrical", "Mechanical", "Drywall", "Finishes"];
const WBS = [
  ["10", "Mobilise"],
  ["20", "Demolition"],
  ["30", "Framing and rough-in"],
  ["40", "Drywall"],
  ["50", "Ceilings and finishes"],
  ["60", "Commissioning and handover"],
];

// [id, wbs, name, trade, days, status, preds, remaining days (active only)]. A pred is [id, type, lag in days].
// Rows are in the order a pass can place them: every pred comes before the activity that follows it.
const T = "FS";
const ROWS = [
  ["A100", "10", "Permit and mobilise", "General", 2, "done", []],
  ["A110", "10", "Site survey and as-builts", "General", 3, "done", [["A100", T, 0]]],
  ["A200", "20", "Strip ceilings and partitions", "Demolition", 5, "done", [["A110", T, 0]]],
  ["A210", "20", "Remove old flooring", "Demolition", 3, "done", [["A200", "SS", 2]]],
  ["A220", "20", "Haul away debris", "Demolition", 2, "done", [["A210", T, 0]]],
  ["A300", "30", "Frame new partitions", "Framing", 6, "done", [["A200", T, 0]]],
  ["A310", "30", "Frame soffits and door openings", "Framing", 4, "done", [["A300", "SS", 3]]],
  ["A340", "30", "Plumbing rough-in", "Mechanical", 5, "done", [["A300", "SS", 2]]],
  ["A320", "30", "Electrical rough-in", "Electrical", 8, "active", [["A300", "SS", 3]], 3],
  ["A330", "30", "HVAC ductwork", "Mechanical", 10, "active", [["A310", T, 0]], 6],
  ["A350", "30", "Fire sprinkler drops", "Mechanical", 4, "todo", [["A330", "SS", 8]]],
  ["A360", "30", "Rough-in inspection", "General", 1, "todo", [["A320", T, 0], ["A330", T, 0], ["A340", T, 0], ["A350", T, 0]]],
  ["A400", "40", "Hang drywall", "Drywall", 7, "todo", [["A360", T, 0]]],
  ["A410", "40", "Tape and finish", "Drywall", 6, "todo", [["A400", "SS", 3]]],
  ["A420", "40", "Prime walls", "Finishes", 3, "todo", [["A410", T, 0]]],
  ["A500", "50", "Ceiling grid", "Framing", 5, "todo", [["A400", T, 0]]],
  ["A510", "50", "Light fixtures", "Electrical", 4, "todo", [["A500", T, 0]]],
  ["A520", "50", "Diffusers and grilles", "Mechanical", 3, "todo", [["A500", T, 0]]],
  ["A600", "50", "Paint walls", "Finishes", 6, "todo", [["A420", T, 0]]],
  ["A610", "50", "Install flooring", "Finishes", 7, "todo", [["A600", T, 0], ["A510", "FF", 0]]],
  ["A620", "50", "Install casework", "Finishes", 5, "todo", [["A610", "SS", 3]]],
  ["A630", "50", "Electrical devices and trim", "Electrical", 4, "todo", [["A600", T, 2]]],
  ["A700", "60", "Test and balance HVAC", "Mechanical", 3, "todo", [["A520", T, 0], ["A610", T, 0]]],
  ["A710", "60", "Final cleaning", "General", 2, "todo", [["A620", T, 0], ["A630", T, 0], ["A700", T, 0]]],
  ["A720", "60", "Punch list walk", "General", 2, "todo", [["A710", T, 0]]],
  ["A730", "60", "Substantial completion", "General", 0, "todo", [["A720", T, 0]]],
];

/** Calendar date of working day `n` (Monday to Friday). */
function dayDate(n) {
  const date = new Date(START);
  let left = n;
  while (left > 0) {
    date.setUTCDate(date.getUTCDate() + 1);
    if (date.getUTCDay() !== 0 && date.getUTCDay() !== 6) left--;
  }
  return date;
}
const text = (date, time) => `${date.toISOString().slice(0, 10)} ${time}`;

// The pass: where each activity starts and finishes (working days, finish inclusive), by the links.
const place = new Map();
for (const [id, , , , days, status, preds, remaining] of ROWS) {
  let start = 0;
  for (const [pred, type, lag] of preds) {
    const p = place.get(pred);
    if (p === undefined) throw new Error(`${id}: predecessor ${pred} is not placed before it`);
    if (type === "FS") start = Math.max(start, p.finish + 1 + lag);
    else if (type === "SS") start = Math.max(start, p.start + lag);
    else start = Math.max(start, p.finish + lag - days + 1); // FF
  }
  const finish = start + Math.max(days, 1) - 1;
  if (status === "done" && finish >= DATA_DAY) throw new Error(`${id}: finishes on day ${finish}, not before the data date`);
  if (status === "active" && (start >= DATA_DAY || start + days - remaining > DATA_DAY)) throw new Error(`${id}: started on day ${start} with ${remaining} d left does not fit the data date`);
  place.set(id, { start, finish, days, status, remaining });
}

const ids = new Map(ROWS.map(([id], index) => [id, 1000 + index]));
const header = (name, fields) => `%T\t${name}\n%F\t${fields.join("\t")}\n`;
const row = (...cells) => `%R\t${cells.join("\t")}\n`;
const WEEK = [2, 3, 4, 5, 6].map((d) => `(0||${d}()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))`).join("");
const CALENDAR = `(0||CalendarData()((0||DaysOfWeek()(${WEEK}))(0||Exceptions()())))`;

let out = "ERMHDR\t19.12\t2026-10-26\tProject\tadmin\tadmin\tdb\tProject Management\tUSD\n";
out += header("PROJECT", ["proj_id", "proj_short_name", "last_recalc_date", "plan_start_date", "clndr_id"]);
out += row(1, "FITOUT", text(dayDate(DATA_DAY), "08:00"), text(dayDate(0), "08:00"), 1);
out += header("CALENDAR", ["clndr_id", "default_flag", "clndr_name", "proj_id", "clndr_type", "day_hr_cnt", "week_hr_cnt", "clndr_data"]);
out += row(1, "Y", "Standard 5 day", 1, "CA_Project", 8, 40, CALENDAR);
out += header("PROJWBS", ["wbs_id", "proj_id", "parent_wbs_id", "seq_num", "proj_node_flag", "wbs_short_name", "wbs_name"]);
out += row(10, 1, "", 1, "Y", "FITOUT", "Level 3 office fit-out");
WBS.forEach(([code, name], index) => { out += row(10 + Number(code), 1, 10, index + 1, "N", code, name); });
// The lag calendar and the percent complete type are stated, so the notes about those two do not appear. The page still lists one note,
// that the file's other scheduling options are not read: any real P6 file has a SCHEDOPTIONS row, so any real file shows it.
out += header("SCHEDOPTIONS", ["schedoptions_id", "proj_id", "sched_calendar_on_relationship_lag"]);
out += row(1, 1, "rcal_ProjDefault");
out += header("TASK", ["task_id", "proj_id", "wbs_id", "clndr_id", "task_code", "task_name", "task_type", "status_code", "complete_pct_type", "phys_complete_pct", "target_drtn_hr_cnt", "remain_drtn_hr_cnt", "act_start_date", "act_end_date"]);
for (const [id, wbs, name, , days, status] of ROWS) {
  const p = place.get(id);
  const started = status !== "todo";
  const remain = status === "done" ? 0 : status === "active" ? p.remaining * 8 : days * 8;
  const percent = status === "done" ? 100 : status === "active" ? Math.round((100 * (days - p.remaining)) / days) : 0;
  out += row(
    ids.get(id), 1, 10 + Number(wbs), 1, id, name, days === 0 ? "TT_FinMile" : "TT_Task",
    { done: "TK_Complete", active: "TK_Active", todo: "TK_NotStart" }[status], "CP_Phys", percent, days * 8, remain,
    started ? text(dayDate(p.start), "08:00") : "", status === "done" ? text(dayDate(p.finish), "17:00") : "",
  );
}
out += header("TASKPRED", ["task_pred_id", "task_id", "pred_task_id", "proj_id", "pred_proj_id", "pred_type", "lag_hr_cnt"]);
let link = 1;
for (const [id, , , , , , preds] of ROWS) {
  for (const [pred, type, lag] of preds) out += row(link++, ids.get(id), ids.get(pred), 1, 1, `PR_${type}`, lag * 8);
}
out += header("ACTVTYPE", ["actv_code_type_id", "actv_short_len", "seq_num", "actv_code_type", "actv_code_type_scope"]);
out += row(7000, 12, 1, "Trade", "AS_Global");
out += header("ACTVCODE", ["actv_code_id", "parent_actv_code_id", "actv_code_type_id", "actv_code_name", "short_name", "seq_num"]);
TRADES.forEach((name, index) => { out += row(7100 + index, "", 7000, name, name, index + 1); });
out += header("TASKACTV", ["task_id", "actv_code_type_id", "actv_code_id", "proj_id"]);
for (const [id, , , trade] of ROWS) out += row(ids.get(id), 7000, 7100 + TRADES.indexOf(trade), 1);
out += "%E\n";

const target = fileURLToPath(new URL("./fit-out.xer", import.meta.url));
writeFileSync(target, out);
console.log(`wrote ${target}: ${ROWS.length} activities, ${link - 1} links`);
for (const [id, , name, , days, status] of ROWS) {
  const p = place.get(id);
  console.log(`${id} ${status.padEnd(6)} ${String(days).padStart(2)} d  day ${String(p.start).padStart(2)}-${String(p.finish).padStart(2)}  ${text(dayDate(p.start), "").trim()}  ${name}`);
}
