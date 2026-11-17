//! Change a link's type and lag through `Session::edit` (gs#703, story 9): the dates match a
//! full solve, undo and redo put back the type, lag and dates exactly, and the move report names
//! the tasks the new type moved (`tests/links.rs` covers add, remove and restore).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Loaded, Session, edit::Edit, edit::EditError, load_bytes};
use common::clock;
use engine::{DepId, LagCalendar, LinkType, Minutes};
use import::ProjectSelection;
use timeline::Timeline;

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

/// The 43-row fixture the move-report tests use.
fn ledger() -> Session {
    ledger_counting_lag_in(None)
}

/// The fixture with the schedule's lag calendar set, so a lag is not counted in the successor's days.
fn ledger_counting_lag_in(lag: Option<LagCalendar>) -> Session {
    let Loaded {
        mut schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(LEDGER_XER, &ProjectSelection::Unspecified, &clock).expect("the file loads");
    let solution = match lag {
        Some(lag) => {
            schedule.set_lag_calendar(lag).expect("a known calendar");
            schedule.clone().compute().expect("the fixture solves")
        }
        None => solution,
    };
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

/// The link the tests change: finish-to-start from task 9 to task 11 would be a new link in
/// `tests/moved.rs`; here the fixture's own link `LINK` becomes `NEW_KIND` with `NEW_LAG`.
const LINK: DepId = DepId(0);
const NEW_KIND: LinkType = LinkType::SS;
const NEW_LAG: Minutes = 2 * 1440;

fn early(s: &Session) -> Vec<(i64, i64)> {
    s.solution().dates.iter().map(|d| (d.es, d.ef)).collect()
}

fn link(s: &Session, dep: DepId) -> (LinkType, Minutes) {
    let d = s.schedule().dependencies()[dep.0 as usize];
    (d.kind, d.lag)
}

fn set(s: &mut Session) -> Result<(), EditError> {
    s.edit(Edit::SetLink {
        dep: LINK,
        kind: NEW_KIND,
        lag: NEW_LAG,
    })
}

fn assert_equals_compute(s: &Session) {
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
}

#[test]
fn set_link_undo_and_redo_are_exact() {
    let mut s = ledger();
    let (kind, lag) = link(&s, LINK);
    assert_ne!(
        (kind, lag),
        (NEW_KIND, NEW_LAG),
        "the edit changes the link"
    );
    let dates = s.solution().clone();
    let finish = s.solution().project_finish;
    assert_eq!(s.history_len(), (0, 0));

    set(&mut s).expect("sets");
    assert_eq!(link(&s, LINK), (NEW_KIND, NEW_LAG));
    assert_ne!(*s.solution(), dates, "the new type moves something");
    assert_equals_compute(&s);
    let set_dates = s.solution().clone();
    assert_eq!(s.history_len(), (1, 0));

    assert!(s.undo().expect("undoes"));
    assert_eq!(link(&s, LINK), (kind, lag));
    assert_eq!(*s.solution(), dates);
    assert_eq!(s.solution().project_finish, finish);
    assert_equals_compute(&s);
    assert_eq!(s.history_len(), (0, 1));

    assert!(s.redo().expect("redoes"));
    assert_eq!(link(&s, LINK), (NEW_KIND, NEW_LAG));
    assert_eq!(*s.solution(), set_dates);
    assert_equals_compute(&s);
    assert_eq!(s.history_len(), (1, 0));

    // The raw edit returns the exact inverse, and applying that returns the edit.
    let inverse = s.apply(Edit::SetLink {
        dep: LINK,
        kind,
        lag,
    });
    assert_eq!(
        inverse,
        Ok(Edit::SetLink {
            dep: LINK,
            kind: NEW_KIND,
            lag: NEW_LAG
        })
    );
}

#[test]
fn a_set_link_report_names_the_moved_tasks() {
    let mut s = ledger();
    let start = early(&s);
    assert!(s.move_report().is_none());
    set(&mut s).expect("sets");
    let after = early(&s);
    let moved: Vec<usize> = (0..start.len()).filter(|&i| start[i] != after[i]).collect();
    assert!(!moved.is_empty(), "the new type moves tasks");
    let r = s.move_report().expect("a link edit moved tasks");
    // No task is the edited one: every task that moved is counted and listed, with its old dates.
    assert_eq!(
        (r.others as usize, r.edited),
        (moved.len(), None),
        "{}",
        r.text
    );
    let listed: Vec<usize> = r.was.iter().map(|m| m.task.0 as usize).collect();
    assert_eq!(listed, moved);
    for m in &r.was {
        assert_eq!((m.was_es, m.was_ef), start[m.task.0 as usize]);
    }
    assert!(
        r.text.starts_with(&format!("{} tasks moved", moved.len())),
        "{}",
        r.text
    );

    // Undo reports the same tasks with was and now swapped.
    let seq = r.seq;
    assert!(s.undo().unwrap());
    let r = s.move_report().expect("undo moved them back");
    assert_ne!(r.seq, seq);
    assert_eq!(r.others as usize, moved.len());
    for m in &r.was {
        assert_eq!((m.was_es, m.was_ef), after[m.task.0 as usize]);
    }
    assert_eq!(early(&s), start);
}

#[test]
fn a_refused_set_link_changes_nothing() {
    let mut s = ledger();
    let before = (s.solution().clone(), s.history_len(), s.revision());
    s.edit(Edit::RemoveLink { dep: LINK }).expect("removes");
    let after_remove = (s.solution().clone(), s.history_len(), s.revision());
    for dep in [LINK, DepId(9999)] {
        let error = s
            .edit(Edit::SetLink {
                dep,
                kind: NEW_KIND,
                lag: NEW_LAG,
            })
            .expect_err("a removed or unknown link is refused");
        assert_eq!(error, EditError::UnknownLink(dep));
        assert_eq!(
            (s.solution().clone(), s.history_len(), s.revision()),
            after_remove
        );
    }
    assert_ne!(before.1, after_remove.1);
}

#[test]
fn link_text_names_a_type_and_a_signed_lag() {
    use LinkType::{FF, FS, SF, SS};
    use app::edit::{LinkTextError, parse_link_text};
    for (text, want) in [
        ("FS", (FS, 0)),
        ("ss+2d", (SS, 2 * 480)),
        (" FF-1d ", (FF, -480)),
        ("SF + 1d 4h", (SF, 480 + 240)),
        ("FS+0", (FS, 0)),
        ("SS+2", (SS, 2 * 480)),
        ("FS-30m", (FS, -30)),
    ] {
        assert_eq!(parse_link_text(text, 480), Ok(want), "{text}");
    }
    for (text, want) in [
        ("", LinkTextError::Kind),
        ("2d", LinkTextError::Kind),
        ("XS+1d", LinkTextError::Kind),
        ("FS2d", LinkTextError::Sign),
        ("SS 2d", LinkTextError::Sign),
    ] {
        assert_eq!(parse_link_text(text, 480), Err(want), "{text}");
    }
    assert!(matches!(
        parse_link_text("FS+", 480),
        Err(LinkTextError::Lag(_))
    ));
    assert!(matches!(
        parse_link_text("FS+soon", 480),
        Err(LinkTextError::Lag(_))
    ));
}

#[test]
fn set_link_text_reads_a_day_in_the_lag_calendar_and_refuses_bad_text() {
    let mut s = ledger();
    let succ = s.schedule().dependencies()[LINK.0 as usize].succ;
    let day = app::edit::day_minutes(s.schedule().calendar_of(succ));
    s.set_link_text(LINK, "SS+2d").expect("sets");
    assert_eq!(link(&s, LINK), (LinkType::SS, 2 * day));
    assert_equals_compute(&s);
    assert_eq!(s.history_len(), (1, 0));

    let before = (s.solution().clone(), s.history_len(), s.revision());
    for text in ["", "SS 2d", "ZZ+1d", "FS+x"] {
        let error = s.set_link_text(LINK, text).expect_err(text);
        assert!(matches!(error, EditError::LinkText(_)), "{text}");
        assert_eq!(
            (s.solution().clone(), s.history_len(), s.revision()),
            before
        );
        assert_eq!(link(&s, LINK), (LinkType::SS, 2 * day));
    }
    assert_eq!(
        s.set_link_text(DepId(9999), "FS"),
        Err(EditError::UnknownLink(DepId(9999)))
    );
}

#[test]
fn link_text_round_trips_through_the_parser() {
    use LinkType::{FF, FS, SF, SS};
    // A zero lag prints the type alone, a part day prints hours, a negative lag its sign.
    let mut s = ledger();
    for (kind, lag, want) in [
        (FS, 0, "FS"),
        (SS, 2 * 480, "SS+2d"),
        (FF, -480, "FF-1d"),
        (SF, 240, "SF+4h"),
        (FS, -(480 + 90), "FS-1d 1h 30m"),
    ] {
        s.edit(Edit::SetLink {
            dep: LINK,
            kind,
            lag,
        })
        .expect("sets");
        assert_eq!(s.link_text(LINK).as_deref(), Some(want));
    }

    // The same round trip with the lag counted in a day that is not the successor's.
    for lag_calendar in [
        None,
        Some(LagCalendar::TwentyFourHour),
        Some(LagCalendar::Predecessor),
    ] {
        let mut s = ledger_counting_lag_in(lag_calendar);
        for kind in [FS, SS, FF, SF] {
            for lag in [
                0,
                240,
                480,
                2 * 480,
                1440,
                1440 + 90,
                -90,
                -480,
                -1440 - 240,
            ] {
                s.edit(Edit::SetLink {
                    dep: LINK,
                    kind,
                    lag,
                })
                .expect("sets");
                let text = s.link_text(LINK).expect("a live link has text");
                // Move the link away, then set it back from its text alone.
                let other = if kind == FS { SS } else { FS };
                s.edit(Edit::SetLink {
                    dep: LINK,
                    kind: other,
                    lag: 7,
                })
                .expect("sets");
                s.set_link_text(LINK, &text).expect(&text);
                assert_eq!(link(&s, LINK), (kind, lag), "{lag_calendar:?} {text}");
                assert_equals_compute(&s);
            }
        }
    }
    let mut s = ledger_counting_lag_in(Some(LagCalendar::TwentyFourHour));
    s.edit(Edit::SetLink {
        dep: LINK,
        kind: SS,
        lag: 1440,
    })
    .expect("sets");
    assert_eq!(
        s.link_text(LINK).as_deref(),
        Some("SS+1d"),
        "a day of 24 hours"
    );
}

#[test]
fn a_removed_link_has_no_text() {
    let mut s = ledger();
    assert!(s.link_text(LINK).is_some());
    s.edit(Edit::RemoveLink { dep: LINK }).expect("removes");
    assert_eq!(s.link_text(LINK), None);
    assert_eq!(s.link_text(DepId(9999)), None);
    assert!(s.undo().expect("undoes"));
    assert!(
        s.link_text(LINK).is_some(),
        "a restored link has text again"
    );
}
