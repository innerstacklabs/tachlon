//! The activity ID of a row the person inserted (decision 0009, item 4).

/// The ID a row inserted after `preceding` takes. `preceding` is the nearest preceding live row's
/// ID that has one, `None` when no such row exists; `live` is every live row's ID.
///
/// The ID is split into a prefix and its trailing digits (`A1-2` is `A1-` and `2`, `A1010` is `A`
/// and `1010`). The result has the same prefix, at least the same digit width, and the number one
/// above the largest any live ID has with that prefix. If a live row already holds the result,
/// one is added until none does. With no `preceding` the result is `A1000` and, while a live row
/// holds that, `A1010`, `A1020` and so on.
pub fn next_activity_id<'a>(
    preceding: Option<&str>,
    live: impl IntoIterator<Item = &'a str>,
) -> String {
    let live: Vec<&str> = live.into_iter().collect();
    let Some(preceding) = preceding else {
        let mut number = 1000;
        while live.contains(&format!("A{number}").as_str()) {
            number += 10;
        }
        return format!("A{number}");
    };
    let (prefix, digits) = split(preceding);
    let width = digits.len();
    // The digits stay text so a suffix of any length keeps its value and its width.
    let largest = live
        .iter()
        .map(|id| split(id))
        .filter(|(p, d)| *p == prefix && !d.is_empty())
        .map(|(_, d)| d.trim_start_matches('0'))
        .max_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
        .unwrap_or("");
    let mut number = increment(largest);
    loop {
        let id = format!("{prefix}{number:0>width$}");
        if !live.contains(&id.as_str()) {
            return id;
        }
        number = increment(&number);
    }
}

/// `id` as its prefix and its trailing digits; the digits are empty when it ends in none.
fn split(id: &str) -> (&str, &str) {
    id.split_at(id.trim_end_matches(|c: char| c.is_ascii_digit()).len())
}

/// The decimal text `digits` plus one, without leading zeros (`""` counts as zero).
fn increment(digits: &str) -> String {
    let digits = digits.trim_start_matches('0');
    let mut out: Vec<u8> = digits.bytes().collect();
    for digit in out.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return String::from_utf8(out).expect("ASCII digits");
        }
    }
    out.insert(0, b'1');
    String::from_utf8(out).expect("ASCII digits")
}

#[cfg(test)]
mod tests {
    use super::next_activity_id as next;

    #[test]
    fn one_above_the_largest_with_the_prefix() {
        assert_eq!(next(Some("A1-2"), ["A1-2", "A1-9", "B-40"]), "A1-10");
        assert_eq!(next(Some("A1010"), ["A1010", "A1030"]), "A1031");
        assert_eq!(next(Some("A007"), ["A007"]), "A008");
    }

    #[test]
    fn a_blank_schedule_starts_at_a1000_and_steps_ten() {
        assert_eq!(next(None, []), "A1000");
        assert_eq!(next(None, ["A1000", "A1010"]), "A1020");
    }

    #[test]
    fn a_prefix_with_no_digits_counts_from_one() {
        assert_eq!(next(Some("START"), ["START"]), "START1");
    }

    #[test]
    fn a_suffix_of_any_length_keeps_its_value_and_width() {
        assert_eq!(
            next(Some("A1"), ["A1", "A1000000000000000000"]),
            "A1000000000000000001"
        );
        assert_eq!(
            next(Some("A1"), ["A1", "A99999999999999999999999"]),
            "A100000000000000000000000"
        );
        assert_eq!(next(Some("A0009"), ["A0009"]), "A0010");
    }
}
