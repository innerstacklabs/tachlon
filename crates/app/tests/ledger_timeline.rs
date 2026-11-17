//! What the session hands the scene for the Ledger timeline (gs#110 PR 4): the file's data date
//! and the calendar whose non-working days are shaded.

use crate::common;

use app::{Loaded, Session, load_bytes};
use common::clock;
use engine::{Calendar, DAY, Schedule, Task};
use import::ProjectSelection;
use timeline::Timeline;
use xer::synth::{Spec, write};

/// The synthetic project's `last_recalc_date` set to `date` (`YYYY-MM-DD HH:MM`).
fn with_recalc(bytes: &[u8], date: &str) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).expect("UTF-8");
    let mut in_project = false;
    let mut column = None;
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let cells: Vec<&str> = line.trim_end_matches('\n').split('\t').collect();
        match cells[0] {
            "%T" => in_project = cells.get(1) == Some(&"PROJECT"),
            "%F" if in_project => {
                column = cells.iter().position(|name| *name == "last_recalc_date");
            }
            "%R" if in_project => {
                let mut cells = cells.clone();
                cells[column.expect("PROJECT has last_recalc_date")] = date;
                out.push_str(&cells.join("\t"));
                out.push('\n');
                continue;
            }
            _ => {}
        }
        out.push_str(line);
    }
    out.into_bytes()
}

/// A session over `loaded`, as `App::create` builds it, fitted to the first 28 days.
fn session(loaded: Loaded) -> Session {
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        data_date,
        ..
    } = loaded;
    let mut timeline = Timeline::new(order, 1200.0, 600.0, 1.0);
    timeline.viewport.fit(0.0, 28.0 * DAY as f64);
    Session::new(schedule, solution, timeline, Vec::new(), clock)
        .with_outline(outline)
        .expect("a valid outline")
        .with_data_date(data_date)
}

fn load(bytes: &[u8]) -> Loaded {
    load_bytes(bytes, &ProjectSelection::Unspecified, &clock).expect("the fixture loads")
}

#[test]
fn the_data_date_of_a_file_is_a_line_in_the_scene() {
    let bytes = with_recalc(&write(&Spec::small(3)), "2024-01-10 08:00");
    let session = session(load(&bytes));
    let at = 9 * DAY + 8 * 60;
    assert_eq!(session.data_date(), Some(at));
    let scene = session.scene();
    assert_eq!(
        scene.data_date_x,
        Some(session.timeline().viewport.x_of(at as f64))
    );
    let viewport = &session.timeline().viewport;
    let today = session.theme().today;
    let line = scene
        .rects
        .iter()
        .find(|rect| rect.color == today && rect.size[0] == 1.0)
        .expect("the data-date line");
    assert_eq!(line.pos[0], viewport.x_of(at as f64).round() as f32);
    assert_eq!(line.pos[1], (viewport.axis_height / 2.0) as f32);
    // Dashed (tl#1215): the last dash ends within one dash and gap of the bottom.
    let last = scene
        .rects
        .iter()
        .rfind(|rect| rect.color == today && rect.size[0] == 1.0)
        .expect("the data-date line");
    let (dash, gap) = viewport.today_dash;
    let end = f64::from(last.pos[1] + last.size[1]);
    assert!(
        end <= viewport.height && end + dash + gap > viewport.height,
        "the line runs below the axis to the bottom"
    );
    let tag_box = scene
        .rects
        .iter()
        .find(|rect| rect.color == today && rect.size[0] > 1.0)
        .expect("the data-date tag box");
    assert!(tag_box.pos[1] + tag_box.size[1] <= viewport.axis_height as f32);
    assert!(scene.labels.iter().any(|label| label.text == "Data date"));
}

#[test]
fn a_file_without_a_data_date_draws_no_line() {
    let mspdi = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../import/tests/fixtures/links.xml"
    ))
    .expect("fixture");
    let session = session(load(&mspdi));
    assert_eq!(session.data_date(), None);
    let scene = session.scene();
    assert_eq!(scene.data_date_x, None);
    assert!(
        scene
            .rects
            .iter()
            .all(|rect| rect.color != session.theme().today)
    );
}

#[test]
fn the_project_calendar_of_an_imported_file_is_shaded() {
    // The synthetic project calendar is Monday to Friday: four weeks, four weekends.
    let session = session(load(&write(&Spec::small(3))));
    let scene = session.scene();
    assert!(scene.shaded >= 4, "weekends shaded, got {}", scene.shaded);
    assert!(
        scene
            .rects
            .iter()
            .any(|rect| rect.color == session.theme().nonwork)
    );
}

/// Four tasks over two calendars: one 7-day and one Monday to Friday.
fn two_calendars(on_seven_day: usize, on_weekdays: usize) -> Session {
    let mut schedule = Schedule::new();
    let all_days = vec![(0, 24 * 60)];
    let seven = Calendar::new(std::array::from_fn(|_| all_days.clone()), &[]).expect("calendar");
    let seven = schedule.add_calendar(seven);
    let day = vec![(480, 1020)];
    let weekdays = Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("calendar");
    let weekdays = schedule.add_calendar(weekdays);
    for (calendar, count) in [(seven, on_seven_day), (weekdays, on_weekdays)] {
        for _ in 0..count {
            schedule
                .add_task(Task {
                    duration: 60,
                    calendar,
                    snet: None,
                    limit: engine::Limit::default(),
                    deleted: false,
                    actual_start: None,
                    actual_finish: None,
                    remaining: None,
                })
                .expect("task");
        }
    }
    let solution = schedule.compute().expect("solved");
    let order = timeline::order_by_start(&solution.dates);
    let mut timeline = Timeline::new(order, 1200.0, 600.0, 1.0);
    timeline.viewport.fit(0.0, 28.0 * DAY as f64);
    Session::new(schedule, solution, timeline, Vec::new(), clock)
}

#[test]
fn the_calendar_most_tasks_use_is_the_one_shaded() {
    assert_eq!(
        two_calendars(1, 3).scene().shaded,
        4,
        "three of four tasks use Monday to Friday"
    );
    assert_eq!(
        two_calendars(3, 1).scene().shaded,
        0,
        "three of four tasks work every day"
    );
    assert_eq!(
        two_calendars(2, 2).scene().shaded,
        0,
        "a tie takes the lowest calendar id"
    );
}
