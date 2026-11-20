//! One pre-check per kind of value the engine takes (`engine::MAX_DATE` and its neighbours): the
//! value, or what the import keeps instead, and one counted note for the kind.

use engine::{Instant, MAX_DATE, MAX_DURATION, MAX_LAG, MIN_DATE, Minutes};

use crate::notes::{Notes, Section};

/// A task's date, or `None` when it is outside `MIN_DATE..=MAX_DATE` from the origin.
pub(crate) fn date(value: Instant, id: &str, notes: &mut Notes) -> Option<Instant> {
    let kept = (MIN_DATE..=MAX_DATE).contains(&value).then_some(value);
    if kept.is_none() {
        notes.add_once(Section::Activity, "Tasks with a date more than 1,000 years from the schedule start: the date was not read, so their dates can differ from the file's", id);
    }
    kept
}

/// The data date, or `None` when it is outside the range of [`date`].
pub(crate) fn data_date(value: Instant, id: Option<&str>, notes: &mut Notes) -> Option<Instant> {
    let kept = (MIN_DATE..=MAX_DATE).contains(&value).then_some(value);
    if kept.is_none() {
        notes.add(Section::Project, "The data date is more than 1,000 years from the schedule start and was not read; progress is measured without it, so dates can differ from the file's", id);
    }
    kept
}

/// A duration, set to `MAX_DURATION` when it is longer.
pub(crate) fn duration(value: Minutes, id: &str, notes: &mut Notes) -> Minutes {
    if value > MAX_DURATION {
        notes.add(Section::Activity, "Tasks with a duration over 100 years: the duration was set to 100 years of 366 days, so their dates can differ from the file's", Some(id));
    }
    value.min(MAX_DURATION)
}

/// A started task's remaining duration, or `None` when it is longer than `MAX_DURATION`.
pub(crate) fn remaining(value: Minutes, id: &str, notes: &mut Notes) -> Option<Minutes> {
    let kept = (value <= MAX_DURATION).then_some(value);
    if kept.is_none() {
        notes.add(Section::Activity, "Tasks with a remaining duration over 100 years: the remaining duration was not read, so their dates can differ from the file's", Some(id));
    }
    kept
}

/// A link's lag, or 0 when its size is over `MAX_LAG`.
pub(crate) fn lag(value: Minutes, id: &str, notes: &mut Notes) -> Minutes {
    if value.unsigned_abs() > MAX_LAG.unsigned_abs() {
        notes.add(Section::Relationship, "Links with a lag over 100 years either way: the lag was not read, so dates can differ from the file's", Some(id));
        return 0;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each value is kept (and not noted) or changed (and noted once); `keeps` says which it was.
    fn check(cases: &[(i64, bool)], keeps: impl Fn(i64, &mut Notes) -> bool) {
        for &(value, kept) in cases {
            let mut notes = Notes::new();
            assert_eq!(keeps(value, &mut notes), kept, "{value}");
            assert_eq!(notes.finish().len(), usize::from(!kept), "{value}");
        }
    }

    #[test]
    fn each_bound_keeps_its_own_value_and_changes_the_next_one() {
        let both = |top: i64| {
            [
                (-top - 1, false),
                (-top, true),
                (top, true),
                (top + 1, false),
            ]
        };
        let above = |top: i64| [(top - 1, true), (top, true), (top + 1, false)];
        check(&both(MAX_DATE), |v, n| date(v, "1", n).is_some());
        check(&both(MAX_DATE), |v, n| data_date(v, None, n).is_some());
        check(&both(MAX_LAG), |v, n| lag(v, "1", n) == v);
        check(&above(MAX_DURATION), |v, n| remaining(v, "1", n).is_some());
        check(&above(MAX_DURATION), |v, n| duration(v, "1", n) == v);
    }
}
