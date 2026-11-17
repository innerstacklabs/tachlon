//! A task note in an MS Project file is not read, and the import says so once (gs#703).

use import::parse_mspdi;

const SENTENCE: &str = "Task notes were not read; no dates are affected.";

fn file(first_notes: &str, second_notes: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Project xmlns="http://schemas.microsoft.com/project">
  <StartDate>2026-01-05T08:00:00</StartDate>
  <Tasks>
    <Task><UID>1</UID><Name>A</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>
      <Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>{first_notes}</Task>
    <Task><UID>2</UID><Name>B</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>
      <Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>{second_notes}</Task>
  </Tasks>
</Project>"#
    )
}

fn count(unsupported: &[String]) -> usize {
    unsupported.iter().filter(|line| *line == SENTENCE).count()
}

#[test]
fn a_document_with_a_task_note_says_the_notes_were_not_read_once() {
    let xml = file("<Notes>Ask the vendor first.</Notes>", "");
    let imported = parse_mspdi(&xml).expect("the file should import");
    assert_eq!(
        count(&imported.unsupported),
        1,
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn several_notes_give_the_sentence_once() {
    let xml = file("<Notes>One.</Notes>", "<Notes>Two.</Notes>");
    let imported = parse_mspdi(&xml).expect("the file should import");
    assert_eq!(
        count(&imported.unsupported),
        1,
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn a_document_with_no_notes_says_nothing() {
    let xml = file("", "<Notes></Notes>");
    let imported = parse_mspdi(&xml).expect("the file should import");
    assert_eq!(
        count(&imported.unsupported),
        0,
        "{:?}",
        imported.unsupported
    );
}
