//! Period and bucket-index helpers behind the calendar tables.

use super::{DAY, DayPeriod, INDEX_CAP};

/// The first period of `periods` that is empty, outside `0..=1440`, out of
/// order or overlapping the one before it.
pub(super) fn bad_period(periods: &[DayPeriod]) -> Option<DayPeriod> {
    let mut prev_end = 0u16;
    for (i, &p) in periods.iter().enumerate() {
        #[expect(clippy::cast_lossless, reason = "u16 widens to i64 exactly")]
        let ok = p.0 < p.1 && p.1 as i64 <= DAY && (i == 0 || p.0 >= prev_end);
        if !ok {
            return Some(p);
        }
        prev_end = p.1;
    }
    None
}

/// Valid periods with touching neighbours joined, so two lists of the same
/// working minutes compare equal.
pub(super) fn joined(periods: &[DayPeriod]) -> Vec<DayPeriod> {
    let mut out: Vec<DayPeriod> = Vec::with_capacity(periods.len());
    for &(s, e) in periods {
        match out.last_mut() {
            Some(last) if last.1 == s => last.1 = e,
            _ => out.push((s, e)),
        }
    }
    out
}

/// Working minutes of `periods` in `[0, offset)` of their day.
#[expect(clippy::cast_lossless, reason = "u16 widens to i64 exactly")]
pub(super) fn minutes_before(periods: &[DayPeriod], offset: i64) -> i64 {
    periods
        .iter()
        .map(|&(s, e)| offset.clamp(s as i64, e as i64) - s as i64)
        .sum()
}

/// Day offset of the `r`-th working minute of `periods` (`r` below their
/// total).
pub(super) fn minute_at(periods: &[DayPeriod], mut r: i64) -> i64 {
    for &(s, e) in periods {
        #[expect(clippy::cast_lossless, reason = "u16 widens to i64 exactly")]
        let len = (e - s) as i64;
        if r < len {
            #[expect(clippy::cast_lossless, reason = "u16 widens to i64 exactly")]
            return s as i64 + r;
        }
        r -= len;
    }
    unreachable!("minute {r} past the exception's periods")
}

/// `index[b]` = how many of the sorted `values` are below `b * step`, for
/// every bucket `b` up to the one past the last value (at most [`INDEX_CAP`]).
pub(super) fn bucket_index(values: impl Iterator<Item = i64> + Clone, step: i64) -> Vec<u32> {
    let last = values.clone().last().unwrap_or(0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "last is a non-negative day or key and step is positive; on a 32-bit target a \
                  quotient past 2^32 wraps to a shorter index, which only slows lookups \
                  (unchecked)"
    )]
    let buckets = ((last / step + 2) as usize).min(INDEX_CAP);
    let mut index = Vec::with_capacity(buckets);
    let mut it = values.peekable();
    let mut below = 0u32;
    #[expect(
        clippy::cast_possible_wrap,
        reason = "buckets is at most INDEX_CAP = 65536"
    )]
    for b in 0..buckets as i64 {
        while it.next_if(|&v| v < b * step).is_some() {
            below += 1;
        }
        index.push(below);
    }
    index
}

/// The first position `>= start` in a sorted slice whose element fails
/// `before`: a forward scan from an index entry, or a binary search past the
/// end of the index. A week bucket of exception days holds at most seven, so
/// that scan is short. A key bucket can hold many: every day of a run of
/// consecutive holidays starts at the same working count, so the run shares
/// one key, and the scan walks the whole run.
pub(super) fn first_not_before<T>(
    items: &[T],
    index: &[u32],
    bucket: i64,
    before: impl Fn(&T) -> bool,
) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "callers pass day / 7 or k / weekly, both non-negative; on a 32-bit target a \
                  bucket past 2^32 wraps to an earlier one, whose forward scan still reaches the \
                  right slot (unchecked)"
    )]
    let entry = index.get(bucket as usize);
    let Some(&start) = entry else {
        let start = *index.last().expect("an index has at least one bucket") as usize;
        return start + items[start..].partition_point(before);
    };
    let mut n = start as usize;
    while n < items.len() && before(&items[n]) {
        n += 1;
    }
    n
}
