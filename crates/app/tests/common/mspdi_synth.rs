//! Writes an MS Project XML (MSPDI) file of a chosen size for the open-time tests.
//!
//! It mirrors `xer::synth` so the two files are the same schedule in two formats: one root
//! summary, `children` summaries under it (a twentieth of the activities, 1 to 200), and the
//! activities under those, round-robin. Activity `i` links from an earlier activity, and every
//! seventh activity gets a second link. Durations come from the same seeded generator. About
//! a quarter of the activities carry actuals, which the importer lists as a note each.
//!
//! A test input writer, not an MS Project exporter. Each test binary that includes it uses part of it.
#![allow(dead_code, reason = "each test binary uses part of the writer")]

use std::fmt::Write as _;

/// The calendar and project start every generated file carries: Monday 2026-01-05, 08:00 to 16:00.
const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Project xmlns="http://schemas.microsoft.com/project">
<StartDate>2026-01-05T08:00:00</StartDate>
<CalendarUID>1</CalendarUID>
<Calendars><Calendar><UID>1</UID><Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID><WeekDays>
<WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>
<WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
<WeekDay><DayType>3</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
<WeekDay><DayType>4</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
<WeekDay><DayType>5</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
<WeekDay><DayType>6</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>16:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
<WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay>
</WeekDays></Calendar></Calendars>
<Tasks>
"#;

const FOOTER: &str = "</Tasks>\n</Project>\n";

/// A complete deterministic description of a generated file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    /// Seed for the durations and the choice of predecessors.
    pub seed: u64,
    /// Number of activities (leaf tasks).
    pub activities: usize,
}

/// What a file written from a [`Spec`] holds, worked out without writing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    /// Summary tasks: the root and the groups under it.
    pub summaries: usize,
    /// Leaf tasks.
    pub activities: usize,
    /// Predecessor links.
    pub links: usize,
    /// Activities with actuals, each one an "actuals ignored" note.
    pub actuals: usize,
}

impl Spec {
    pub fn with_activities(seed: u64, activities: usize) -> Spec {
        Spec { seed, activities }
    }

    /// Summaries directly under the root.
    fn children(&self) -> usize {
        (self.activities / 20).clamp(1, 200)
    }

    pub fn counts(&self) -> Counts {
        let n = self.activities;
        let some = usize::from(n != 0);
        let complete = n.saturating_sub(1) / 11 + some;
        let started = n.saturating_sub(1) / 5 + some;
        let both = n.saturating_sub(1) / 55 + some;
        Counts {
            summaries: 1 + self.children(),
            activities: n,
            links: n.saturating_sub(1) + n.saturating_sub(1) / 7,
            actuals: complete + (started - both),
        }
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn below(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next() % upper as u64) as usize
        }
    }
}

/// One activity's drawn values: the generator runs once per activity, in activity order.
struct Draw {
    hours: usize,
    /// Predecessor activity of the first link (none for activity 0).
    first: usize,
    /// The seventh-activity link: predecessor, MSPDI link type, lag in hours.
    extra: Option<(usize, u8, i32)>,
}

fn draw(spec: &Spec) -> Vec<Draw> {
    let mut rng = SplitMix64(spec.seed);
    (0..spec.activities)
        .map(|activity| {
            let milestone = is_milestone(activity);
            let hours = if milestone {
                0
            } else {
                (rng.below(10) + 1) * 8
            };
            let width = activity.min(5);
            let mut offset = rng.below(width);
            if activity.is_multiple_of(7) && offset == 2 {
                offset = (offset + 1) % width;
            }
            let first = activity.saturating_sub(1 + offset);
            let extra = (activity >= 3 && activity.is_multiple_of(7)).then(|| {
                // MSPDI link types: 0 FF, 1 FS, 2 SF, 3 SS.
                let kind = match rng.below(4) {
                    0 => 1,
                    1 => 3,
                    2 => 0,
                    _ => 2,
                };
                (activity - 3, kind, (rng.below(8) as i32 - 2) * 8)
            });
            Draw {
                hours,
                first,
                extra,
            }
        })
        .collect()
}

fn is_milestone(activity: usize) -> bool {
    activity % 10 == 9 && activity % 25 != 24
}

/// Generates the MSPDI text for `spec`.
pub fn write(spec: &Spec) -> Vec<u8> {
    let children = spec.children();
    let draws = draw(spec);
    let uid_of = |activity: usize| 2 + children + activity;
    let mut out = String::with_capacity(HEADER.len() + spec.activities * 420);
    out.push_str(HEADER);
    summary(&mut out, 1, "Project", 1);
    for group in 0..children {
        summary(&mut out, 2 + group, &format!("Group {}", group + 1), 2);
        for activity in (group..spec.activities).step_by(children) {
            task(&mut out, &draws, activity, uid_of);
        }
    }
    out.push_str(FOOTER);
    out.into_bytes()
}

fn summary(out: &mut String, uid: usize, name: &str, level: u8) {
    let _ = writeln!(
        out,
        "<Task><UID>{uid}</UID><Name>{name}</Name><OutlineLevel>{level}</OutlineLevel><Summary>1</Summary><Duration>PT0S</Duration><DurationFormat>7</DurationFormat></Task>"
    );
}

fn task(out: &mut String, draws: &[Draw], activity: usize, uid_of: impl Fn(usize) -> usize) {
    let draw = &draws[activity];
    let number = activity + 1;
    let cafe = if activity % 50 == 49 { " Café" } else { "" };
    let _ = write!(
        out,
        "<Task><UID>{}</UID><Name>Activity {number}{cafe}</Name><OutlineLevel>3</OutlineLevel><Summary>0</Summary><Duration>PT{}H0M0S</Duration><DurationFormat>7</DurationFormat>",
        uid_of(activity),
        draw.hours
    );
    if is_milestone(activity) {
        out.push_str("<Milestone>1</Milestone>");
    }
    let complete = activity.is_multiple_of(11);
    if complete {
        out.push_str("<PercentComplete>100</PercentComplete><ActualStart>2026-01-05T08:00:00</ActualStart><ActualFinish>2026-01-06T16:00:00</ActualFinish>");
    } else if activity.is_multiple_of(5) {
        // Started, not finished: the file states dates that the engine's chain does not repeat.
        out.push_str("<PercentComplete>50</PercentComplete><ActualStart>2026-01-05T08:00:00</ActualStart><Start>2026-01-05T08:00:00</Start><Finish>2026-01-06T16:00:00</Finish>");
    }
    if activity > 0 {
        link(out, uid_of(draw.first), 1, 0);
    }
    if let Some((pred, kind, lag_hours)) = draw.extra {
        link(out, uid_of(pred), kind, lag_hours);
    }
    out.push_str("</Task>\n");
}

/// A predecessor link. `LinkLag` is in tenths of a minute.
fn link(out: &mut String, pred_uid: usize, kind: u8, lag_hours: i32) {
    let _ = write!(
        out,
        "<PredecessorLink><PredecessorUID>{pred_uid}</PredecessorUID><Type>{kind}</Type><LinkLag>{}</LinkLag><LagFormat>7</LagFormat></PredecessorLink>",
        lag_hours * 600
    );
}
