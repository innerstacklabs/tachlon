//! `load_bytes`: a file's data date is the floor of unstarted work from the first compute (gs#736).

use crate::common;

use common::xer_text::{MINUTES_PER_DAY, loaded, rewrite, text};
use engine::TaskId;
use import::ProjectSelection;
use xer::synth::Spec;

fn set_recalc(text: &str, date: &str) -> String {
    rewrite(text, "PROJECT", "last_recalc_date", |_, _| {
        Some(date.to_string())
    })
}

#[test]
fn unstarted_work_starts_no_earlier_than_the_data_date_the_file_carries() {
    // Nothing has started: a started or finished activity is fixed at its actuals, not floored.
    let base = rewrite(&text(&Spec::small(3)), "TASK", "status_code", |_, _| {
        Some("TK_NotStart".to_string())
    });
    let before = loaded(base.as_bytes(), &ProjectSelection::Unspecified);
    let first = before.solution.get(TaskId(0)).es;
    assert!(
        first < 3 * MINUTES_PER_DAY,
        "the fixture starts before Thursday"
    );

    // Thursday 2024-01-04 08:00 from the origin Monday.
    let thursday = 3 * MINUTES_PER_DAY + 8 * 60;
    let text = set_recalc(&base, "2024-01-04 08:00");
    let opened = loaded(text.as_bytes(), &ProjectSelection::Unspecified);
    assert_eq!(opened.data_date, Some(thursday));
    assert_eq!(opened.schedule.data_date(), Some(thursday));
    assert_eq!(opened.solution.get(TaskId(0)).es, thursday);
    for dates in &opened.solution.dates {
        assert!(dates.es >= thursday, "{dates:?}");
    }
}

#[test]
fn a_file_without_a_data_date_opens_with_no_floor() {
    let base = set_recalc(&text(&Spec::small(3)), "");
    let opened = loaded(base.as_bytes(), &ProjectSelection::Unspecified);
    assert_eq!(opened.data_date, None);
    assert_eq!(opened.schedule.data_date(), None);
    assert!(opened.solution.get(TaskId(0)).es < 3 * MINUTES_PER_DAY);
}
