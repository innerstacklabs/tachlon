//! The what-moved note names a data date change (gs#703, story 11, the note half): set it a week
//! later, clear it, undo and redo, and the words say which way the date went.

use crate::common;

use app::{Loaded, Session, edit::Edit, load_bytes};
use common::clock;
use engine::Instant;
use import::ProjectSelection;
use timeline::Timeline;

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");
const DAY: i64 = 24 * 60;

fn ledger() -> Session {
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(LEDGER_XER, &ProjectSelection::Unspecified, &clock).expect("the file loads");
    Session::new(
        schedule,
        solution,
        Timeline::new(order, 1200.0, 600.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("a valid outline")
    .with_origin(origin)
}

fn set(s: &mut Session, date: Option<Instant>) {
    s.edit(Edit::SetDataDate { date }).expect("the date moves");
}

fn note(s: &Session) -> String {
    s.move_report()
        .expect("the change moved tasks")
        .text
        .clone()
}

#[test]
fn a_data_date_change_names_the_date_in_the_note() {
    let mut s = ledger();
    let base = s.solution().project_finish / DAY * DAY / 2;

    set(&mut s, Some(base));
    assert_eq!(note(&s), "data date set · 39 tasks moved · finish +41 days");
    set(&mut s, Some(base + 7 * DAY));
    assert_eq!(
        note(&s),
        "data date +7 days · 39 tasks moved · finish +7 days"
    );
    set(&mut s, None);
    assert_eq!(
        note(&s),
        "data date cleared · 39 tasks moved · finish -54 days"
    );

    s.undo().expect("undo the clear");
    assert_eq!(note(&s), "data date set · 39 tasks moved · finish +54 days");
    s.undo().expect("undo the week");
    assert_eq!(
        note(&s),
        "data date -7 days · 39 tasks moved · finish -7 days"
    );
    s.redo().expect("redo the week");
    assert_eq!(
        note(&s),
        "data date +7 days · 39 tasks moved · finish +7 days"
    );
}

#[test]
fn a_shift_within_one_calendar_day_reads_data_date_moved() {
    let mut s = ledger();
    let day = s.solution().project_finish / DAY * DAY / 2 / DAY * DAY;
    set(&mut s, Some(day + 60));
    set(&mut s, Some(day + 20 * 60));
    assert_eq!(
        note(&s),
        "data date moved · 39 tasks moved · finish +3 days"
    );
}

/// A guard, not failing-first evidence: it passes on the tree before the change too. It fails if
/// the cause outlives the data date change that set it.
#[test]
fn an_edit_after_a_data_date_change_does_not_carry_the_words() {
    let mut s = ledger();
    let base = s.solution().project_finish / DAY * DAY / 2;
    set(&mut s, Some(base));
    s.edit(Edit::SetDuration {
        task: engine::TaskId(16),
        minutes: 4800,
    })
    .expect("applies");
    assert!(!note(&s).contains("data date"), "{}", note(&s));
}
