//! Opening an MS Project file that carries its own baseline (gs#703, row 1 of the change-and-
//! re-baseline plan): the file's Baseline 0 is the baseline, set as the session opens.

use app::baseline::BaselineSummary;
use app::{Loaded, Session, fitted_timeline, load_bytes};
use engine::TaskId;
use import::ProjectSelection;

const BASELINED: &str = include_str!("fixtures/baselined.xml");
/// The file the page's baseline spec opens: Dug's Baseline 0 is the working day before its start.
const PAGE_BASELINED: &str = include_str!("../../../packages/gantt/tests/fixtures/baselined.xml");

fn open(text: &str) -> (Session, Loaded) {
    let mut loaded =
        load_bytes(text.as_bytes(), &ProjectSelection::Unspecified, &|| 0.0).expect("loads");
    let baseline = loaded.baseline.take();
    let timeline = fitted_timeline(loaded.order.clone(), &loaded.solution, 1000.0, 400.0, 1.0);
    let mut session = Session::new(
        loaded.schedule.clone(),
        loaded.solution.clone(),
        timeline,
        loaded.meta.clone(),
        || 0.0,
    )
    .with_origin(loaded.origin);
    if let Some(baseline) = baseline {
        session = session.with_baseline(baseline);
    }
    (session, loaded)
}

#[test]
fn opening_a_file_with_a_saved_baseline_sets_it_without_float() {
    let (session, _) = open(BASELINED);
    let baseline = session.baseline().expect("the file's baseline is set");
    assert_eq!(baseline.len(), 1, "only the task that states one");
    let held = baseline.task(TaskId(0)).expect("Dug has an entry");
    let dug = session.solution().get(TaskId(0));
    assert_eq!((held.es, held.ef), (dug.es, dug.ef), "the file's own dates");
    assert_eq!(held.float, None, "a file states no float");
    assert!(baseline.task(TaskId(1)).is_none(), "Poured has none");
    assert_eq!(baseline.project_finish(), dug.ef);
    assert!(!session.can_undo(), "setting it is not an edit");
}

#[test]
fn a_task_that_ends_later_than_its_saved_baseline_slipped() {
    // The file's baseline finish is a day before where the schedule puts it.
    let earlier = BASELINED.replace(
        "<Finish>2026-01-05T16:00:00</Finish>",
        "<Finish>2026-01-02T16:00:00</Finish>",
    );
    assert_ne!(earlier, BASELINED);
    let (session, _) = open(&earlier);
    let summary = session.baseline_diff().expect("a baseline is set");
    assert_eq!(summary.slipped, 1);
    assert_eq!(
        (summary.joined, summary.left),
        (0, 0),
        "no float to compare"
    );
    assert_eq!((summary.added, summary.removed), (1, 0));
}

#[test]
fn a_file_with_a_baseline_with_one_date_or_none_has_no_baseline() {
    let no_finish = BASELINED.replace("<Finish>2026-01-05T16:00:00</Finish>", "");
    let cost = BASELINED.replace(
        "<Start>2026-01-05T08:00:00</Start><Finish>2026-01-05T16:00:00</Finish>",
        "<Cost>100</Cost>",
    );
    for text in [no_finish, cost] {
        assert_ne!(text, BASELINED);
        let (session, loaded) = open(&text);
        assert!(session.baseline().is_none());
        assert!(loaded.baseline.is_none());
    }
}

#[test]
fn the_page_fixture_opens_with_one_entry_and_a_three_day_slip() {
    let (session, _) = open(PAGE_BASELINED);
    assert_eq!(session.baseline().expect("set as it opens").len(), 1);
    // A Friday baseline against a Monday finish: three calendar days, one slipped task, and
    // Poured, with no entry, added.
    assert_eq!(
        session.baseline_diff(),
        Some(BaselineSummary {
            slipped: 1,
            finish_days: 3,
            joined: 0,
            left: 0,
            added: 1,
            removed: 0,
        })
    );
}
