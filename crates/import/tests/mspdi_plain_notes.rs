//! Every MS Project import note says in a scheduler's words what the reader did not read and
//! whether a date can change (gs#881): no format name stands in for the sentence.

use import::parse_mspdi;

/// Three tasks, no calendars: task 1 has a deadline that is not a date (no date moves),
/// task 2 has a finish-no-later-than constraint with no date (its date can move), task 3 has a percent complete
/// out of range (display only).
const FILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Project xmlns="http://schemas.microsoft.com/project">
  <StartDate>2026-01-05T08:00:00</StartDate>
  <Tasks>
    <Task><UID>1</UID><Name>A</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>
      <Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>
      <Deadline>soon</Deadline></Task>
    <Task><UID>2</UID><Name>B</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>
      <Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>
      <ConstraintType>7</ConstraintType></Task>
    <Task><UID>3</UID><Name>C</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>
      <Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>
      <PercentComplete>150</PercentComplete></Task>
  </Tasks>
</Project>"#;

#[test]
fn a_note_that_moves_no_date_says_so_in_plain_words() {
    let imported = parse_mspdi(FILE).expect("the file should import");
    assert!(
        imported
            .unsupported
            .contains(&"Tasks with a deadline that is not a date: the deadline was not read; no dates are affected: 1 (first: 1)".to_owned()),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn a_note_that_can_move_a_date_names_the_constraint_in_words() {
    let imported = parse_mspdi(FILE).expect("the file should import");
    assert!(
        imported.unsupported.contains(
            &"Task 2: the finish no later than constraint (type 7) has no date and was not read; the task is scheduled \
              without it, so its dates can differ from the file's."
                .to_owned()
        ),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn no_shown_sentence_carries_a_format_name_in_capitals() {
    let imported = parse_mspdi(FILE).expect("the file should import");
    assert_ne!(imported.unsupported, Vec::<String>::new());
    for line in &imported.unsupported {
        let shouted: Vec<&str> = line
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|word| word.len() >= 3 && word.chars().all(|c| c.is_ascii_uppercase()))
            .collect();
        assert!(shouted.is_empty(), "{line:?} has {shouted:?}");
    }
}
