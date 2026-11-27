//! `Schedule::pin_broken_links`: the links a pin breaks, worked by hand on a Monday-to-Friday
//! 08:00-17:00 calendar (540 working minutes a day). The predecessor `p` is four days long and
//! runs Monday 08:00 to Thursday 17:00, so a finish-to-start successor could start Friday 08:00.

mod common;

use common::{at, dep, std_cal, task};
use engine::LinkType::{FF, FS, SF, SS};
use engine::{DepId, Limit, LinkType, Pin, Schedule, TaskId};

const D: i64 = 540;

/// `p` (four days), and a one-day successor `s` pinned by `pin`, linked by `kind` and `lag`.
fn pinned(pin: Option<Pin>, kind: LinkType, lag: i64) -> (Schedule, TaskId, TaskId) {
    let mut sch = Schedule::new();
    let cal = sch.add_calendar(std_cal(&[]));
    let p = task(&mut sch, cal, 4 * D, None);
    let s = task(&mut sch, cal, D, None);
    sch.set_limit(
        s,
        Limit {
            pin,
            ..Limit::default()
        },
    )
    .expect("a valid schedule");
    dep(&mut sch, p, s, kind, lag);
    (sch, p, s)
}

fn broken(sch: &mut Schedule) -> Vec<DepId> {
    let sol = sch.compute().expect("a valid schedule");
    sch.pin_broken_links(&sol)
}

#[test]
fn a_start_pin_before_the_predecessor_ends_breaks_the_link() {
    let (mut s, _, _) = pinned(Some(Pin::Start(at(2, 8, 0))), FS, 0);
    assert_eq!(broken(&mut s), vec![DepId(0)]);
}

#[test]
fn a_start_pin_where_the_predecessor_ends_breaks_none() {
    let (mut s, _, _) = pinned(Some(Pin::Start(at(4, 8, 0))), FS, 0);
    assert_eq!(broken(&mut s), vec![]);
    let (mut s, _, _) = pinned(Some(Pin::Start(at(7, 8, 0))), FS, 0);
    assert_eq!(broken(&mut s), vec![]);
}

#[test]
fn a_finish_pin_before_the_predecessor_ends_breaks_the_link() {
    let (mut s, _, _) = pinned(Some(Pin::Finish(at(2, 17, 0))), FS, 0);
    assert_eq!(broken(&mut s), vec![DepId(0)]);
}

#[test]
fn each_link_type_is_judged_on_its_own_bound() {
    // (kind, lag in days, broken): the pin is Wednesday 08:00.
    let cases = [
        (FS, 0, true),
        (SS, 0, false),
        (SS, 3, true),
        (FF, 0, true),
        (SF, 0, false),
        (SF, 4, true),
    ];
    for (kind, lag_days, want) in cases {
        let (mut s, _, _) = pinned(Some(Pin::Start(at(2, 8, 0))), kind, lag_days * D);
        let got = !broken(&mut s).is_empty();
        assert_eq!(got, want, "{kind:?} with a lag of {lag_days} days");
    }
}

#[test]
fn only_the_links_past_the_pin_are_listed() {
    let (mut s, _, succ) = pinned(Some(Pin::Start(at(2, 8, 0))), FS, 0);
    let cal = s.tasks()[0].calendar;
    let early = task(&mut s, cal, D, None);
    dep(&mut s, early, succ, FS, 0);
    let gone = task(&mut s, cal, 4 * D, None);
    dep(&mut s, gone, succ, FS, 0);
    s.remove_dependency(DepId(2)).expect("a valid schedule");
    assert_eq!(broken(&mut s), vec![DepId(0)]);
}

#[test]
fn a_started_or_finished_task_is_not_held() {
    // Started with no pin: it keeps its actual start though the predecessor ends later.
    let (mut s, _, succ) = pinned(None, FS, 0);
    s.set_actual_start(succ, Some(at(2, 8, 0)))
        .expect("a valid schedule");
    assert_eq!(broken(&mut s), vec![]);
    let (mut s, _, succ) = pinned(Some(Pin::Finish(at(2, 17, 0))), FS, 0);
    s.set_actual_start(succ, Some(at(2, 8, 0)))
        .expect("a valid schedule");
    assert_eq!(broken(&mut s), vec![]);
    s.set_actual_finish(succ, Some(at(2, 17, 0)))
        .expect("a valid schedule");
    assert_eq!(broken(&mut s), vec![]);
}
