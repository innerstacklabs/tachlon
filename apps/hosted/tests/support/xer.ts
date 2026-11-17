// A three-activity XER the hosted specs open as "your own file": built here so no spec reads a file from packages/gantt/.
// The header line is what tells the importer it is an XER (gs#81 PR 6), not the name or the content type.
const table = (name: string, fields: string, rows: string[]): string =>
  `%T\t${name}\n%F\t${fields}\n${rows.map((row) => `%R\t${row}\n`).join("")}`;

/** The row count the chart shows for `ownXer()`: three activities under one root WBS row. */
export const OWN_XER_ROWS = 4;
export const OWN_XER_NAMES = ["XER A100", "XER A200", "XER A300"];

export function ownXer(): Buffer {
  const text =
    "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tProject Management\tUSD\n" +
    table("PROJECT", "proj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id", ["1\tVERIFY\tY\t2024-03-04 08:00\t1"]) +
    table("PROJWBS", "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name", ["10\t1\t\t1\tY\tRoot"]) +
    table(
      "TASK",
      "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2",
      ["A100", "A200", "A300"].map((code, i) => `${i + 1}\t1\t10\t1\t${code}\tXER ${code}\tTT_Task\tTK_NotStart\t16\t16\t\t\t`),
    ) +
    table("TASKPRED", "task_id\tpred_task_id\tpred_type\tlag_hr_cnt", ["2\t1\tPR_FS\t0", "3\t2\tPR_FS\t0"]) +
    table("CALENDAR", "clndr_id\tclndr_type\tclndr_data", ["1\tCA_Project\t"]) +
    "%E\n";
  return Buffer.from(text, "utf8");
}

/** An XER that stops after its header and one table header: the reader must refuse it with its own message. */
export function truncatedXer(): Buffer {
  return Buffer.from("ERMHDR\t19.12\n%T\tTASK\n", "utf8");
}
