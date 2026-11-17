//! One day's working periods, cleaned the same way for XER and MSPDI.

use engine::DayPeriod;

/// The latest minute of a day: a period may end at 24:00.
const END_OF_DAY: u16 = 1_440;

/// Sort `periods` by start and drop each one the engine would refuse: empty, ending after
/// 24:00, or starting before the previous kept period ends. Returns the kept periods and how
/// many were dropped. A period that does not overlap another is always kept.
pub(crate) fn keep_valid(mut periods: Vec<DayPeriod>) -> (Vec<DayPeriod>, usize) {
    periods.sort_unstable_by_key(|period| period.0);
    let mut kept: Vec<DayPeriod> = Vec::with_capacity(periods.len());
    let mut previous_end = None;
    let mut dropped = 0;
    for period in periods {
        let invalid = period.0 >= period.1
            || period.1 > END_OF_DAY
            || previous_end.is_some_and(|end| period.0 < end);
        if invalid {
            dropped += 1;
        } else {
            previous_end = Some(period.1);
            kept.push(period);
        }
    }
    (kept, dropped)
}

#[cfg(test)]
mod tests {
    use super::keep_valid;

    #[test]
    fn keeps_non_overlapping_periods_sorted() {
        assert_eq!(
            keep_valid(vec![(780, 1_020), (480, 720)]),
            (vec![(480, 720), (780, 1_020)], 0)
        );
    }

    #[test]
    fn touching_periods_are_not_an_overlap() {
        assert_eq!(
            keep_valid(vec![(480, 720), (720, 1_020)]),
            (vec![(480, 720), (720, 1_020)], 0)
        );
    }

    #[test]
    fn drops_the_later_of_two_overlapping_periods_and_keeps_the_rest() {
        assert_eq!(
            keep_valid(vec![(480, 720), (660, 780), (780, 1_020)]),
            (vec![(480, 720), (780, 1_020)], 1)
        );
    }

    #[test]
    fn drops_empty_reversed_and_past_midnight_periods() {
        assert_eq!(
            keep_valid(vec![(480, 480), (600, 540), (900, 1_500), (60, 120)]),
            (vec![(60, 120)], 3)
        );
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert_eq!(keep_valid(Vec::new()), (Vec::new(), 0));
    }
}
