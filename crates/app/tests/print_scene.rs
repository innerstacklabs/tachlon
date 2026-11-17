//! The print plan and the page scene (gs#703): `Session::print_plan` and `Session::print_scene`.

use crate::common;

use app::{PageSize, Session, meta::TaskMeta};
use common::{clock, collapse::standard_calendar, outline_session};
use engine::{DAY, Schedule, Task, TaskId};
use timeline::{Theme, Timeline};

const ROWS_PER_PAGE: usize = 25;
/// Tasks in the filtered-print session: enough that the critical rows fill more than a page.
const NODES: usize = 1_500;

fn is_summary(session: &Session, task: u32) -> bool {
    session
        .outline()
        .is_some_and(|outline| outline.is_summary(TaskId(task)))
}

#[test]
fn a_filter_that_hides_rows_prints_only_the_visible_ones_on_ceil_pages() {
    let mut session = outline_session(NODES).0;
    assert!(session.set_critical_only(true), "the filter turns on");
    let visible = session.timeline().order().to_vec();
    assert!(
        visible.len() < NODES && visible.len() > ROWS_PER_PAGE,
        "{} rows",
        visible.len()
    );

    let plan = session.print_plan();
    assert_eq!(plan.rows, visible, "[plan-rows]");
    assert_eq!(plan.rows_per_page, ROWS_PER_PAGE);
    assert_eq!(plan.pages, visible.len().div_ceil(ROWS_PER_PAGE), "[pages]");
    assert_eq!(plan.left_out, 0);
}

#[test]
fn every_bar_is_on_exactly_one_page_in_row_order() {
    let session = outline_session(120).0;
    let plan = session.print_plan();
    assert!(plan.pages > 1);

    let printed: Vec<u32> = (0..plan.pages)
        .flat_map(|page| plan.page_rows(page).to_vec())
        .collect();
    assert_eq!(printed, session.timeline().order(), "[row-order]");

    let leaves = printed
        .iter()
        .filter(|&&task| !is_summary(&session, task))
        .count();
    let bars: usize = (0..plan.pages)
        .map(|page| session.print_scene(&plan, page).bars)
        .sum();
    assert_eq!(bars, leaves, "[one-page-each]");
}

#[test]
fn a_plan_longer_than_five_years_still_fits_the_panel() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for snet in [None, Some(8 * 365 * DAY)] {
        let task = Task {
            duration: DAY,
            calendar,
            snet,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("calendar is the schedule's");
    }
    let solution = schedule.clone().compute().expect("computes");
    let meta = vec![TaskMeta::default(); 2];
    let timeline = Timeline::new(vec![0, 1], 1_000.0, 400.0, 1.0);
    let session = Session::new(schedule, solution, timeline, meta, clock);

    let plan = session.print_plan();
    let scene = session.print_scene(&plan, 0);
    assert_eq!(scene.bars, 2);
    let (_, last) = scene.bar_rects[1];
    let rect = &scene.rects[last];
    assert!(
        f64::from(rect.pos[0] + rect.size[0]) <= plan.panel_w,
        "the last bar ends at {} inside the panel",
        rect.pos[0] + rect.size[0]
    );
}

#[test]
fn a_dark_session_prints_in_the_light_theme() {
    let light = outline_session(60).0;
    let mut dark = outline_session(60).0;
    dark.set_theme(Theme::dark());

    let (plan_l, plan_d) = (light.print_plan(), dark.print_plan());
    assert_eq!(
        dark.print_scene(&plan_d, 0).rects,
        light.print_scene(&plan_l, 0).rects
    );
}

#[test]
fn a_plan_stops_at_one_hundred_pages_and_says_how_many_rows_were_left_out() {
    let session = outline_session(3_000).0;
    let rows = session.timeline().order().len();
    assert!(rows > 100 * ROWS_PER_PAGE);

    let plan = session.print_plan();
    assert_eq!(plan.pages, 100);
    assert_eq!(plan.left_out, rows - 100 * ROWS_PER_PAGE);
    assert_eq!(plan.rows.len(), 100 * ROWS_PER_PAGE);
}

#[test]
fn a_long_plan_prints_its_months_in_the_names_the_host_set() {
    let mut session = outline_session(60).0;
    let months = r#"["m1","m2","m3","m4","m5","m6","m7","m8","m9","m10","m11","m12"]"#;
    let days = r#"["a","b","c","d","e","f","g"]"#;
    let names = format!(r#"{{"months":{months},"weekdays":{days},"weekStart":0}}"#);
    session.set_locale(&names).unwrap();
    let window = Some((0, 400 * DAY));
    let page = &session.print_pages_on("Plan", "", PageSize::Letter, None, window, None)[0];
    let panel = page.split("<svg x=").nth(1).unwrap();
    let texts = panel.split("fill=\"#1f2328\">").skip(1);
    let printed: Vec<_> = texts.map(|t| t.split('<').next().unwrap()).collect();
    assert!(
        !printed.is_empty() && printed.iter().all(|m| m.starts_with('m')),
        "{printed:?}"
    );
}
