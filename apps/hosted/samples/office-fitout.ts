// The hosted app's small realistic sample (gs#511 S6b): a 50-activity office fit-out as a typed table, and a writer that
// turns the table into MS Project XML (MSPDI), so no XER writer is duplicated in TypeScript. `scripts/build-samples.mjs`
// imports this file (Node 22.18 or later runs TypeScript that only uses type annotations) and gzips what `toMspdi`
// returns into `public/samples/office-fitout.xml.gz`; nothing it writes is committed.
//
// The data is made up: no client, company or person is named. Durations are working days of 8 hours. Every link joins
// two activities, never a summary task, because the importer drops a link that touches a summary and lists it as
// unsupported. A task with a `percent` is the sample's "in progress": it carries an actual start (`ACTUALS`), a finished one
// an actual finish, and the rest a remaining duration from the percent.
// Every activity but a milestone has one crew or two (`CREWS`), which `toMspdi` writes as `Resources` and `Assignments`.
// Every row also carries a saved baseline (`BASELINE` for an activity, the span of its activities for a phase), so the chart opens with
// the baseline set and no row reads as added.
// The file's `StatusDate` is the data date. The MS Project reader applies the actuals, as the P6 reader does (gs#860).
//
// To change the table: a uid names one row, and a link names its predecessor by uid. A row that another row links to
// cannot be deleted without deleting the link, or the importer refuses the file; the last row, which nothing follows,
// can.

/** How a link joins its predecessor: finish to start, start to start, finish to finish, start to finish. */
export type LinkType = "FS" | "SS" | "FF" | "SF";

/** A link from the activity `pred` to the row that carries it. */
export interface Link {
  /** The uid of the predecessor activity. */
  pred: number;
  /** Defaults to finish to start. */
  type?: LinkType;
  /** Working days of lag after the link's event; defaults to 0. */
  lagDays?: number;
}

/** One row of the table: an activity (a leaf) or a summary task that groups the activities after it. */
export interface Row {
  /** Unique in the table; the summary tasks are 100, 200, ... and the activities count up from them. */
  uid: number;
  /** Outline level: 1 for a phase, 2 for an activity. */
  level: 1 | 2;
  name: string;
  /** A summary task has no duration of its own; the chart rolls it up from the rows under it. */
  summary?: true;
  /** Working days; 0 for a milestone. Absent on a summary task. */
  days?: number;
  milestone?: true;
  /** Percent complete, 1 to 100; absent when the activity has not started. */
  percent?: number;
  after?: Link[];
}

const phase = (uid: number, name: string): Row => ({ uid, level: 1, name, summary: true });
const task = (uid: number, name: string, days: number, after: Link[] = [], percent?: number): Row => ({
  uid,
  level: 2,
  name,
  days,
  after,
  ...(percent === undefined ? {} : { percent }),
});
const milestone = (uid: number, name: string, after: Link[], percent?: number): Row => ({
  ...task(uid, name, 0, after, percent),
  milestone: true,
});
const fs = (pred: number, lagDays = 0): Link => ({ pred, lagDays });
const ss = (pred: number, lagDays = 0): Link => ({ pred, type: "SS", lagDays });

/** The project starts on this Monday, at 08:00. */
export const START_DATE = "2027-01-04T08:00:00";

/** The data date: the Friday that ends the fifth week, at the close of the working day. */
export const STATUS_DATE = "2027-02-05T17:00:00";

/**
 * Where each started activity actually began, and for a finished one (100 percent) when it ended. The dates are working
 * days in the first five weeks, none after the data date. A percent moves no date: the remaining time comes from it.
 */
export const ACTUALS: Record<number, { start: string; finish?: string }> = {
  101: { start: "2027-01-04T08:00:00", finish: "2027-01-08T17:00:00" },
  102: { start: "2027-01-11T08:00:00", finish: "2027-01-21T17:00:00" },
  103: { start: "2027-01-22T08:00:00", finish: "2027-01-22T08:00:00" },
  104: { start: "2027-01-22T08:00:00" },
  105: { start: "2027-01-22T08:00:00" },
  106: { start: "2027-02-01T08:00:00" },
  201: { start: "2027-02-01T08:00:00" },
  202: { start: "2027-02-01T08:00:00" },
  203: { start: "2027-02-03T08:00:00" },
  301: { start: "2027-02-01T08:00:00" },
  302: { start: "2027-02-02T08:00:00" },
  303: { start: "2027-02-04T08:00:00" },
  304: { start: "2027-02-02T08:00:00" },
};

/**
 * The saved baseline (MS Project's Baseline 0): each activity's start and finish as the engine schedules this table with the
 * actuals and the status date left out, one pair per activity. A phase has no pair of its own: `baselineXml` gives it the span of its
 * activities' pairs (the earliest start, the latest finish). The pairs are measured, not derived here: open the table without those
 * fields and read the Start and Finish columns.
 */
export const BASELINE: Record<number, [start: string, finish: string]> = {
  101: ["2027-01-04T08:00:00", "2027-01-08T17:00:00"],
  102: ["2027-01-11T08:00:00", "2027-01-21T17:00:00"],
  103: ["2027-01-21T17:00:00", "2027-01-21T17:00:00"],
  104: ["2027-01-22T08:00:00", "2027-02-11T17:00:00"],
  105: ["2027-01-22T08:00:00", "2027-02-08T17:00:00"],
  106: ["2027-02-12T08:00:00", "2027-02-17T17:00:00"],
  201: ["2027-02-18T08:00:00", "2027-02-23T17:00:00"],
  202: ["2027-02-18T08:00:00", "2027-03-03T17:00:00"],
  203: ["2027-02-24T08:00:00", "2027-03-16T17:00:00"],
  204: ["2027-03-16T17:00:00", "2027-03-16T17:00:00"],
  301: ["2027-02-18T08:00:00", "2027-03-03T17:00:00"],
  302: ["2027-02-18T08:00:00", "2027-02-22T17:00:00"],
  303: ["2027-02-23T08:00:00", "2027-04-05T17:00:00"],
  304: ["2027-02-18T08:00:00", "2027-02-22T17:00:00"],
  305: ["2027-02-23T08:00:00", "2027-03-29T17:00:00"],
  401: ["2027-03-17T08:00:00", "2027-03-19T17:00:00"],
  402: ["2027-03-22T08:00:00", "2027-03-29T17:00:00"],
  403: ["2027-03-24T08:00:00", "2027-03-29T17:00:00"],
  404: ["2027-03-30T08:00:00", "2027-03-31T17:00:00"],
  405: ["2027-04-01T08:00:00", "2027-04-01T17:00:00"],
  501: ["2027-04-02T08:00:00", "2027-04-05T17:00:00"],
  502: ["2027-04-06T08:00:00", "2027-04-15T17:00:00"],
  503: ["2027-04-07T08:00:00", "2027-04-14T17:00:00"],
  504: ["2027-04-16T08:00:00", "2027-04-20T17:00:00"],
  505: ["2027-04-21T08:00:00", "2027-04-23T17:00:00"],
  506: ["2027-04-29T08:00:00", "2027-05-10T17:00:00"],
  601: ["2027-04-06T08:00:00", "2027-04-13T17:00:00"],
  602: ["2027-04-14T08:00:00", "2027-04-22T17:00:00"],
  603: ["2027-04-14T08:00:00", "2027-04-20T17:00:00"],
  604: ["2027-04-16T08:00:00", "2027-04-27T17:00:00"],
  605: ["2027-04-16T08:00:00", "2027-04-26T17:00:00"],
  606: ["2027-04-15T08:00:00", "2027-04-20T17:00:00"],
  607: ["2027-04-16T08:00:00", "2027-04-21T17:00:00"],
  608: ["2027-04-21T08:00:00", "2027-04-23T17:00:00"],
  701: ["2027-04-26T08:00:00", "2027-04-26T17:00:00"],
  702: ["2027-04-28T08:00:00", "2027-04-28T17:00:00"],
  703: ["2027-06-14T08:00:00", "2027-06-14T17:00:00"],
  801: ["2027-05-11T08:00:00", "2027-05-18T17:00:00"],
  802: ["2027-05-14T08:00:00", "2027-05-19T17:00:00"],
  803: ["2027-05-19T08:00:00", "2027-05-27T17:00:00"],
  804: ["2027-05-24T08:00:00", "2027-06-01T17:00:00"],
  805: ["2027-06-01T08:00:00", "2027-06-08T17:00:00"],
  806: ["2027-05-20T08:00:00", "2027-05-26T17:00:00"],
  807: ["2027-05-24T08:00:00", "2027-05-27T17:00:00"],
  808: ["2027-05-20T08:00:00", "2027-05-26T17:00:00"],
  809: ["2027-06-09T08:00:00", "2027-06-11T17:00:00"],
  901: ["2027-06-15T08:00:00", "2027-06-17T17:00:00"],
  902: ["2027-06-18T08:00:00", "2027-06-24T17:00:00"],
  903: ["2027-05-27T08:00:00", "2027-06-03T17:00:00"],
  904: ["2027-06-24T17:00:00", "2027-06-24T17:00:00"],
};

/** Days off, from the project calendar: the public holidays that fall on a Monday in the plan's span. */
export const HOLIDAYS: { name: string; date: string }[] = [
  { name: "Winter holiday", date: "2027-01-18" },
  { name: "Presidents holiday", date: "2027-02-15" },
  { name: "Memorial holiday", date: "2027-05-31" },
  { name: "Independence holiday", date: "2027-07-05" },
];

/**
 * The crews: each crew's name and the uids of the activities it works on. Every activity except a milestone has one crew
 * or two (a uid listed under two crews has both), so no task is unassigned. `toMspdi` writes a crew as a resource and each
 * uid under it as an assignment, in row order and then crew order. The names are trades and teams, never a person or a company.
 */
export const CREWS: Record<string, number[]> = {
  "Survey crew": [101, 405],
  "Design team": [102, 104, 106, 903],
  "MEP engineers": [105],
  "Permit agent": [201, 202, 203],
  "Procurement team": [301, 302, 304],
  Suppliers: [303, 305],
  "Demolition crew": [401, 402, 403, 404],
  "Framing crew": [501, 502, 503, 504, 505],
  "Fire protection crew": [504, 606, 607],
  "Board fixers": [506, 801, 802],
  Electricians: [601, 602, 607, 806, 807],
  "Data cablers": [603, 608, 807],
  "Mechanical crew": [604, 605, 808],
  Decorators: [803],
  Joiners: [804, 809],
  "Floor layers": [805],
  "Building inspector": [701, 702, 703],
  "Snagging team": [901, 902],
};

/** 9 summary tasks and 50 activities, in outline order. */
export const ROWS: Row[] = [
  phase(100, "Design"),
  task(101, "Site survey and measured drawings", 5, [], 100),
  task(102, "Space planning and layout options", 8, [fs(101)], 100),
  milestone(103, "Layout approved", [fs(102)], 100),
  task(104, "Detailed design and specification", 15, [fs(103)], 90),
  task(105, "MEP engineering design", 12, [fs(103)], 75),
  task(106, "Design coordination review", 3, [fs(104), fs(105)], 30),

  phase(200, "Permits"),
  task(201, "Prepare permit application", 4, [fs(106)], 50),
  task(202, "Landlord alterations consent", 10, [fs(106)], 40),
  task(203, "Building permit review", 15, [fs(201)], 10),
  milestone(204, "Permit issued", [fs(202), fs(203)]),

  phase(300, "Procurement"),
  task(301, "Tender and award main contractors", 10, [fs(106)], 60),
  task(302, "Order switchgear and air handling units", 3, [fs(106)], 25),
  task(303, "Switchgear and air handling delivery", 30, [fs(302)], 5),
  task(304, "Order glazing and joinery", 3, [fs(106)], 45),
  task(305, "Glazing and joinery fabrication", 25, [fs(304)]),

  phase(400, "Demolition"),
  task(401, "Site mobilisation and protection", 3, [fs(204), fs(301)]),
  task(402, "Strip out ceilings and partitions", 6, [fs(401)]),
  task(403, "Remove floor finishes", 4, [ss(402, 2)]),
  task(404, "Waste removal and floor clean", 2, [fs(402), fs(403)]),
  task(405, "Structural survey of the exposed slab", 1, [fs(404)]),

  phase(500, "Framing and boards"),
  task(501, "Set out new partitions", 2, [fs(405)]),
  task(502, "Steel stud framing", 8, [fs(501)]),
  task(503, "Ceiling grid framing", 6, [ss(501, 3)]),
  task(504, "Fire stopping", 3, [fs(502)]),
  task(505, "Wall insulation", 3, [fs(504)]),
  task(506, "Plasterboard fixing", 8, [fs(505), fs(701), fs(702)]),

  phase(600, "MEP rough-in"),
  task(601, "Containment and cable tray", 6, [fs(501)]),
  task(602, "Power first fix", 7, [fs(601)]),
  task(603, "Data cabling first fix", 5, [fs(601)]),
  task(604, "Ductwork installation", 8, [fs(303), fs(502)]),
  task(605, "Heating and cooling pipework", 7, [fs(502)]),
  task(606, "Sprinkler head relocation", 4, [fs(503)]),
  task(607, "Fire alarm first fix", 4, [ss(602, 2)]),
  task(608, "Controls cabling", 3, [fs(603)]),

  phase(700, "Inspections"),
  task(701, "Electrical rough-in inspection", 1, [fs(602), fs(603), fs(607), fs(608)]),
  task(702, "Mechanical and plumbing rough-in inspection", 1, [fs(604), fs(605), fs(606)]),
  task(703, "Final building inspection", 1, [fs(806), fs(807), fs(808), fs(809)]),

  phase(800, "Finishes"),
  task(801, "Tape and joint plasterboard", 6, [fs(506)]),
  task(802, "Ceiling tile installation", 4, [ss(801, 3)]),
  task(803, "Prime and paint walls", 7, [fs(801)]),
  task(804, "Joinery and glazing installation", 6, [fs(305), ss(803, 3)]),
  task(805, "Carpet tile flooring", 6, [fs(802), fs(803, 1)]),
  task(806, "Light fittings and power second fix", 5, [fs(802)]),
  task(807, "Data outlets and AV second fix", 4, [ss(806, 2)]),
  task(808, "Grilles and HVAC commissioning", 5, [fs(802), fs(604)]),
  task(809, "Signage and ironmongery", 3, [fs(804), fs(805)]),

  phase(900, "Handover"),
  task(901, "Snagging inspection", 3, [fs(703)]),
  task(902, "Rectify snag items", 5, [fs(901)]),
  task(903, "Operating manuals and as-built drawings", 5, [fs(808)]),
  milestone(904, "Practical completion", [fs(902), fs(903)]),
];

/** The activities: the rows that are not summary tasks. */
export function activities(rows: readonly Row[] = ROWS): Row[] {
  return rows.filter((row) => row.summary !== true);
}

const MINUTES_PER_DAY = 8 * 60;

const escape = (text: string): string => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

const LINK_CODE: Record<LinkType, number> = { FF: 0, FS: 1, SF: 2, SS: 3 };

/** The working week: Monday to Friday, 08:00 to 12:00 and 13:00 to 17:00. MSPDI's DayType 1 is Sunday. */
function weekDays(): string {
  const day = (type: number, working: boolean): string =>
    working
      ? `<WeekDay><DayType>${type}</DayType><DayWorking>1</DayWorking><WorkingTimes>` +
        "<WorkingTime><FromTime>08:00:00</FromTime><ToTime>12:00:00</ToTime></WorkingTime>" +
        "<WorkingTime><FromTime>13:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime>" +
        "</WorkingTimes></WeekDay>"
      : `<WeekDay><DayType>${type}</DayType><DayWorking>0</DayWorking></WeekDay>`;
  return [day(1, false), day(2, true), day(3, true), day(4, true), day(5, true), day(6, true), day(7, false)].join("");
}

function exceptions(): string {
  return HOLIDAYS.map(
    (holiday) =>
      `<Exception><Name>${escape(holiday.name)}</Name><EnteredByOccurrences>0</EnteredByOccurrences><DayWorking>0</DayWorking>` +
      `<TimePeriod><FromDate>${holiday.date}T00:00:00</FromDate><ToDate>${holiday.date}T23:59:00</ToDate></TimePeriod></Exception>`,
  ).join("");
}

function link(link: Link): string {
  const lag = (link.lagDays ?? 0) * MINUTES_PER_DAY * 10; // MSPDI states a lag in tenths of a minute
  return (
    `<PredecessorLink><PredecessorUID>${link.pred}</PredecessorUID><Type>${LINK_CODE[link.type ?? "FS"]}</Type>` +
    `${lag === 0 ? "" : `<LinkLag>${lag}</LinkLag>`}</PredecessorLink>`
  );
}

/** A started activity's actual start, its actual finish when it is done, else the working time still to do (from its percent), to a whole working day: the work resumes at the 17:00 data date, so a day's multiple finishes at a day boundary and every date after it falls on the hour. */
function actualsXml(row: Row): string {
  const actual = ACTUALS[row.uid];
  if (actual === undefined || row.percent === undefined) return "";
  const start = `<ActualStart>${actual.start}</ActualStart>`;
  if (actual.finish !== undefined) return `${start}<ActualFinish>${actual.finish}</ActualFinish>`;
  const minutes = Math.round(((row.days ?? 0) * MINUTES_PER_DAY * (100 - row.percent)) / 100 / MINUTES_PER_DAY) * MINUTES_PER_DAY;
  return `${start}<RemainingDuration>PT${minutes / 60}H0M0S</RemainingDuration>`;
}

/** One `<Baseline>` numbered 0 with a start and a finish. */
function baselineOf(saved: readonly [start: string, finish: string]): string {
  return `<Baseline><Number>0</Number><Start>${saved[0]}</Start><Finish>${saved[1]}</Finish></Baseline>`;
}

/** An activity's saved baseline, from `BASELINE`; a phase's, which spans the baselines of the activities after it up to the next phase. */
function baselineXml(rows: readonly Row[], index: number): string {
  const row = rows[index]!;
  if (row.summary !== true) {
    const saved = BASELINE[row.uid];
    return saved === undefined ? "" : baselineOf(saved);
  }
  const children: [string, string][] = [];
  for (let next = index + 1; next < rows.length && rows[next]!.summary !== true; next++) {
    const saved = BASELINE[rows[next]!.uid];
    if (saved !== undefined) children.push(saved);
  }
  if (children.length === 0) return "";
  const start = children.map((pair) => pair[0]).reduce((a, b) => (a < b ? a : b));
  const finish = children.map((pair) => pair[1]).reduce((a, b) => (a > b ? a : b));
  return baselineOf([start, finish]);
}

function taskXml(rows: readonly Row[], index: number): string {
  const row = rows[index]!;
  const head = `<UID>${row.uid}</UID><ID>${index + 1}</ID><Name>${escape(row.name)}</Name><OutlineLevel>${row.level}</OutlineLevel>`;
  if (row.summary === true) return `<Task>${head}<Summary>1</Summary><Duration>PT0H0M0S</Duration>${baselineXml(rows, index)}</Task>`;
  const hours = (row.days ?? 0) * 8;
  const percent = row.percent === undefined ? "" : `<PercentComplete>${row.percent}</PercentComplete>`;
  return (
    `<Task>${head}<Summary>0</Summary><Milestone>${row.milestone === true ? 1 : 0}</Milestone>` +
    `<Duration>PT${hours}H0M0S</Duration>${percent}${actualsXml(row)}${baselineXml(rows, index)}${(row.after ?? []).map(link).join("")}</Task>`
  );
}

/** How many tasks in `rows` have at least one crew: the count in the reader's note about resource assignments. */
export function assignedTasks(rows: readonly Row[] = ROWS): number {
  const uids = new Set(Object.values(CREWS).flat());
  return rows.filter((row) => uids.has(row.uid)).length;
}

/** The crews as `<Resources>`, a resource to a crew (uid 1 up), then `<Assignments>`, one to each crew a task is in, after the rows' tasks. */
function crewsXml(rows: readonly Row[]): string {
  const names = Object.keys(CREWS);
  const resources = names.map((name, index) => `<Resource><UID>${index + 1}</UID><ID>${index + 1}</ID><Name>${escape(name)}</Name></Resource>`);
  const assignments = rows.flatMap((row) =>
    names.flatMap((name, index) => (CREWS[name]!.includes(row.uid) ?[`<TaskUID>${row.uid}</TaskUID><ResourceUID>${index + 1}</ResourceUID>`] : [])),
  );
  return `<Resources>${resources.join("")}</Resources><Assignments>${assignments.map((pair, index) => `<Assignment><UID>${index + 1}</UID>${pair}</Assignment>`).join("")}</Assignments>`;
}

/** The table as an MS Project XML document: one project calendar with the holidays, then every row as a task, then the crews. */
export function toMspdi(rows: readonly Row[] = ROWS): string {
  return (
    '<?xml version="1.0" encoding="UTF-8"?>\n' +
    '<Project xmlns="http://schemas.microsoft.com/project">' +
    "<Name>Office fit-out</Name>" +
    `<StartDate>${START_DATE}</StartDate><StatusDate>${STATUS_DATE}</StatusDate><CalendarUID>1</CalendarUID>` +
    "<Calendars><Calendar><UID>1</UID><Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID>" +
    `<WeekDays>${weekDays()}</WeekDays><Exceptions>${exceptions()}</Exceptions></Calendar></Calendars>` +
    `<Tasks>${rows.map((_, index) => taskXml(rows, index)).join("")}</Tasks>${crewsXml(rows)}</Project>\n`
  );
}
