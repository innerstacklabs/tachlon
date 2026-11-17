//! The order of the activities under one WBS node (gs#819): start date, then activity code with
//! its digit runs compared as numbers, so `A1-2` comes before `A1-10`.

use std::cmp::Ordering;

use xer::TaskRow;

/// Sort `activities` in place: by the file's start date (an activity with none last), then by
/// activity code in natural order, then by `task_id`. Called once per node at open.
pub(crate) fn sort(activities: &mut [&TaskRow]) {
    activities.sort_by(|left, right| {
        compare_start(left, right)
            .then_with(|| compare_code(left.task_code.as_deref(), right.task_code.as_deref()))
            .then_with(|| left.task_id.cmp(&right.task_id))
    });
}

/// The start P6 shows: actual start, else early start, else target start. A started activity's
/// early start is its remaining work, near the data date.
fn start(row: &TaskRow) -> Option<xer::Minutes> {
    row.act_start_date
        .or(row.early_start_date)
        .or(row.target_start_date)
}

fn compare_start(left: &TaskRow, right: &TaskRow) -> Ordering {
    match (start(left), start(right)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Text compared with each run of digits as a number. A missing code sorts first, as it did.
fn compare_code(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(a), Some(b)) => natural(a, b),
        (a, b) => a.cmp(&b),
    }
}

/// Compare two strings with their digit runs read as numbers, so `L2` comes before `L10`.
#[must_use]
pub fn natural(left: &str, right: &str) -> Ordering {
    let (mut a, mut b) = (left, right);
    while !a.is_empty() && !b.is_empty() {
        let (x, y) = (a.as_bytes()[0], b.as_bytes()[0]);
        if x.is_ascii_digit() && y.is_ascii_digit() {
            let (run_a, rest_a) = split_digits(a);
            let (run_b, rest_b) = split_digits(b);
            let (num_a, num_b) = (run_a.trim_start_matches('0'), run_b.trim_start_matches('0'));
            let order = num_a
                .len()
                .cmp(&num_b.len())
                .then_with(|| num_a.cmp(num_b))
                .then_with(|| run_a.len().cmp(&run_b.len()));
            if order != Ordering::Equal {
                return order;
            }
            (a, b) = (rest_a, rest_b);
        } else {
            let (ca, cb) = (
                a.chars().next().unwrap_or(' '),
                b.chars().next().unwrap_or(' '),
            );
            if ca != cb {
                return ca.cmp(&cb);
            }
            (a, b) = (&a[ca.len_utf8()..], &b[cb.len_utf8()..]);
        }
    }
    a.len().cmp(&b.len())
}

fn split_digits(text: &str) -> (&str, &str) {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    text.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::natural;
    use std::cmp::Ordering::{Greater, Less};

    #[test]
    fn digit_runs_compare_as_numbers() {
        assert_eq!(natural("A1-2", "A1-10"), Less);
        assert_eq!(natural("A1-201", "A1-1001"), Less);
        assert_eq!(natural("A1-10", "A1-2"), Greater);
        assert_eq!(natural("A1-1", "A1-1"), std::cmp::Ordering::Equal);
        assert_eq!(natural("A1", "A1-1"), Less);
    }

    #[test]
    fn leading_zeros_do_not_change_the_value_and_break_a_tie_by_width() {
        assert_eq!(natural("A01", "A2"), Less);
        assert_eq!(natural("A02", "A1"), Greater);
        assert_eq!(natural("A1", "A01"), Less);
        assert_eq!(natural("A01", "A01"), std::cmp::Ordering::Equal);
    }
}
