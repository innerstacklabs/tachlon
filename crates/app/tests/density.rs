//! The row height a host picks by density name (`Session::set_density`).

use crate::common::demo_session;
use app::{Loaded, PageSize, RenderFacts, Session, load_bytes};
use import::ProjectSelection;
use xer::synth::{Spec, write};

fn height(session: &Session) -> f64 {
    session.timeline().viewport.row_height
}

#[test]
fn each_density_gives_the_viewport_status_hit_testing_and_bars_one_height() {
    for (name, h) in [("compact", 20.0), ("normal", 24.0), ("roomy", 40.0)] {
        let (mut session, _) = demo_session(200);
        session.set_density(name).expect("a known density");
        let status = session.status_json(&RenderFacts::default());
        let status: serde_json::Value = serde_json::from_str(&status).expect("JSON");
        assert_eq!(
            (height(&session), status["rowHeight"].as_f64()),
            (h, Some(h))
        );

        let v = &session.timeline().viewport;
        assert_eq!(v.row_at(v.axis_height + h * 3.5), Some(3), "{name}");
        let scene = session.frame_scene(|_, text, px| text.len() as f32 * px * 0.5);
        let bar = (h * v.bar_ratio).round();
        for &(task, index) in &scene.bar_rects {
            let row = session.timeline().order().iter().position(|&t| t == task);
            let rect = scene.rects[index];
            let top = v.row_top(row.expect("a bar's row")) + ((h - bar) / 2.0).round();
            assert_eq!(
                (f64::from(rect.pos[1]), f64::from(rect.size[1])),
                (top, bar),
                "{name}"
            );
        }
        assert!(scene.bar_rects.len() > 3, "{name}: bars to compare");
    }
}

#[test]
fn an_unknown_density_is_refused_by_name_and_a_shorter_row_clamps_the_scroll() {
    let (mut session, _) = demo_session(20);
    let error = session.set_density("tall").expect_err("not a density");
    assert!(error.contains("tall") && error.contains("roomy"), "{error}");
    assert!(session.set_density("Roomy").is_err());
    assert_eq!(height(&session), 24.0);
    session.scroll_by(1.0e9);
    session.set_density("compact").expect("compact");
    let v = &session.timeline().viewport;
    assert!(
        v.scroll_y <= v.max_scroll(),
        "a shorter row pulls the scroll back"
    );
}

#[test]
fn a_printed_page_holds_the_whole_rows_of_the_height_on_both_sheets() {
    for (name, rows) in [("compact", 30), ("normal", 25), ("roomy", 15)] {
        let (mut session, _) = demo_session(200);
        session.set_density(name).expect("a known density");
        for size in [PageSize::Letter, PageSize::A4] {
            let plan = session.print_plan_on(size, None, None);
            assert_eq!(plan.rows_per_page, rows, "{name} on {size:?}");
        }
    }
}

#[test]
fn a_printed_list_cell_sits_in_the_middle_third_of_its_roomy_row() {
    let (mut session, _) = demo_session(60);
    session.set_density("roomy").expect("roomy");
    let page = &session.print_pages("Plan", "")[0];
    let cells = page.split("<clipPath id=\"pc").skip(1);
    let y = |cell: &str| -> f64 {
        let at = cell
            .split("<text ")
            .nth(1)
            .and_then(|t| t.split("y=\"").nth(1));
        at.and_then(|t| t[..t.find('"')?].parse().ok())
            .expect("a baseline")
    };
    let mut ys: Vec<f64> = cells.map(y).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    let under = ys[1] - session.timeline().viewport.axis_height; // the first row; ys[0] is the head
    assert!(
        (40.0 / 3.0..=80.0 / 3.0).contains(&under),
        "the baseline is {under} px under the row top"
    );
}

#[test]
fn the_density_survives_opening_another_file() {
    let (mut session, _) = demo_session(20);
    session.set_density("roomy").expect("roomy");
    let xer = write(&Spec::with_activities(2, 30));
    let next = load_bytes(&xer, &ProjectSelection::All, &|| 0.0).expect("an XER loads");
    #[rustfmt::skip]
    let Loaded { schedule, solution, order, meta, outline, origin, .. } = next;
    let opened = session.reopen(schedule, solution, order, meta, Some(outline), None, origin);
    assert_eq!(height(&opened.expect("the outline fits")), 40.0);
}
