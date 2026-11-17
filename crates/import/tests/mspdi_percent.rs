use import::parse_mspdi;

const PERCENT: &str = include_str!("fixtures/percent.xml");
const NO_PERCENT: &str = include_str!("fixtures/percent_none.xml");

fn progress(xml: &str) -> (Vec<Option<u8>>, Vec<String>) {
    let imported = parse_mspdi(xml).expect("fixture should import");
    (
        imported.tasks.iter().map(|task| task.progress).collect(),
        imported.unsupported,
    )
}

#[test]
fn percent_complete_is_read_per_task_and_a_task_without_one_has_none() {
    let (values, notes) = progress(PERCENT);
    assert_eq!(values, [Some(100), Some(50), Some(0), None]);
    assert!(
        notes.is_empty(),
        "percent complete is no longer ignored: {notes:?}"
    );
}

#[test]
fn a_file_without_percentages_has_no_progress() {
    assert_eq!(progress(NO_PERCENT).0, [None; 4]);
}

#[test]
fn percentages_do_not_move_any_date_or_duration() {
    let with = parse_mspdi(PERCENT).expect("fixture should import");
    let without = parse_mspdi(NO_PERCENT).expect("fixture should import");
    let shape = |imported: &import::Imported| {
        imported
            .schedule
            .tasks()
            .iter()
            .map(|task| (task.duration, task.snet))
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&with), shape(&without));
    let mut a = with.schedule;
    let mut b = without.schedule;
    assert_eq!(
        a.compute().expect("solves").dates,
        b.compute().expect("solves").dates
    );
}

#[test]
fn a_percentage_outside_zero_to_one_hundred_is_clamped_and_noted_once() {
    let xml = PERCENT
        .replace("<PercentComplete>50<", "<PercentComplete>250<")
        .replace("<PercentComplete>0<", "<PercentComplete>-5<");
    let (values, notes) = progress(&xml);
    assert_eq!(values, [Some(100), Some(100), Some(0), None]);
    assert_eq!(
        notes,
        [
            "A percent complete outside 0 to 100 was changed to the nearest limit; no dates are affected."
        ]
    );
}
