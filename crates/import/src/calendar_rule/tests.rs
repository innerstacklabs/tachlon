use std::collections::HashMap;

use super::*;

const DAY: DayPeriod = (480, 1_020);
const SHORT: DayPeriod = (480, 720);

fn layer(base: Option<u32>, days: &[(usize, &[DayPeriod])], exceptions: &[Exception]) -> Layer {
    let mut week: [Option<Vec<DayPeriod>>; 7] = std::array::from_fn(|_| None);
    for &(index, periods) in days {
        week[index] = Some(periods.to_vec());
    }
    Layer {
        base,
        week,
        exceptions: exceptions.to_vec(),
        work_week: Vec::new(),
    }
}

fn resolve_in(
    start: u32,
    layers: &HashMap<u32, Layer>,
    holidays: Holidays,
) -> Result<Resolved, ImportError> {
    resolve(start, |uid| layers.get(&uid), holidays)
}

fn standard() -> Layer {
    layer(
        None,
        &[
            (0, &[DAY]),
            (1, &[DAY]),
            (2, &[DAY]),
            (3, &[DAY]),
            (4, &[DAY]),
        ],
        &[],
    )
}

#[test]
fn a_derived_calendar_takes_its_own_days_and_the_rest_from_its_base() {
    let layers = HashMap::from([
        (1, standard()),
        (2, layer(Some(1), &[(2, &[SHORT]), (5, &[SHORT])], &[])),
    ]);
    let resolved = resolve_in(2, &layers, Holidays::Inherit).unwrap();
    assert_eq!(resolved.week[0], vec![DAY]);
    assert_eq!(resolved.week[2], vec![SHORT]);
    assert_eq!(resolved.week[4], vec![DAY]);
    assert_eq!(resolved.week[5], vec![SHORT]);
    assert_eq!(resolved.week[6], []);
    assert_eq!(resolved.warnings, []);
}

#[test]
fn a_stated_empty_day_is_a_day_off_not_a_gap() {
    let layers = HashMap::from([(1, standard()), (2, layer(Some(1), &[(4, &[])], &[]))]);
    let resolved = resolve_in(2, &layers, Holidays::Inherit).unwrap();
    assert_eq!(resolved.week[3], vec![DAY]);
    assert_eq!(resolved.week[4], []);
}

#[test]
fn a_derived_exception_replaces_its_base_on_the_same_date() {
    let layers = HashMap::from([
        (
            1,
            layer(None, &[(0, &[DAY])], &[(10, vec![]), (20, vec![SHORT])]),
        ),
        (2, layer(Some(1), &[], &[(10, vec![SHORT]), (30, vec![])])),
    ]);
    let resolved = resolve_in(2, &layers, Holidays::Inherit).unwrap();
    assert_eq!(
        resolved.exceptions,
        vec![(10, vec![SHORT]), (20, vec![SHORT]), (30, vec![])]
    );
}

#[test]
fn own_holidays_leave_out_the_base_calendars_exceptions_but_keep_its_week() {
    let layers = HashMap::from([
        (1, layer(None, &[(0, &[DAY])], &[(10, vec![])])),
        (2, layer(Some(1), &[], &[(20, vec![])])),
    ]);
    let resolved = resolve_in(2, &layers, Holidays::Own).unwrap();
    assert_eq!(resolved.week[0], vec![DAY]);
    assert_eq!(resolved.exceptions, vec![(20, vec![])]);
}

#[test]
fn a_missing_base_warns_and_keeps_the_calendars_reached() {
    let layers = HashMap::from([
        (1, layer(Some(9), &[(0, &[DAY])], &[])),
        (2, layer(Some(1), &[(1, &[SHORT])], &[])),
    ]);
    let resolved = resolve_in(2, &layers, Holidays::Inherit).unwrap();
    assert_eq!(
        resolved.warnings,
        vec![ChainWarning::BaseMissing { uid: 1, base: 9 }]
    );
    assert_eq!(resolved.week[0], vec![DAY]);
    assert_eq!(resolved.week[1], vec![SHORT]);
}

#[test]
fn a_cycle_warns_for_the_start_calendar_and_stops() {
    let layers = HashMap::from([
        (1, layer(Some(2), &[(0, &[DAY])], &[])),
        (2, layer(Some(3), &[(1, &[SHORT])], &[])),
        (3, layer(Some(1), &[(2, &[DAY])], &[])),
    ]);
    let resolved = resolve_in(1, &layers, Holidays::Inherit).unwrap();
    assert_eq!(resolved.warnings, vec![ChainWarning::Cycle { uid: 1 }]);
    assert_eq!(resolved.week[0], vec![DAY]);
    assert_eq!(resolved.week[1], vec![SHORT]);
    assert_eq!(resolved.week[2], vec![DAY]);
}

#[test]
fn a_calendar_based_on_itself_is_a_cycle() {
    let layers = HashMap::from([(1, layer(Some(1), &[(0, &[DAY])], &[]))]);
    let resolved = resolve_in(1, &layers, Holidays::Inherit).unwrap();
    assert_eq!(resolved.warnings, vec![ChainWarning::Cycle { uid: 1 }]);
    assert_eq!(resolved.week[0], vec![DAY]);
}

#[test]
fn a_start_calendar_that_is_not_there_is_an_error() {
    let layers = HashMap::from([(1, standard())]);
    assert!(matches!(
        resolve_in(7, &layers, Holidays::Inherit),
        Err(ImportError::Missing { .. })
    ));
}

#[test]
fn whole_week_layers_and_per_day_layers_give_the_same_week() {
    let base_days: [&[DayPeriod]; 7] = [&[DAY], &[DAY], &[DAY], &[DAY], &[DAY], &[], &[]];
    let derived_days: [&[DayPeriod]; 7] = [&[DAY], &[SHORT], &[DAY], &[DAY], &[], &[SHORT], &[]];

    // Every day stated, in both layers.
    let whole = HashMap::from([
        (1, layer(None, &stated(&base_days, 0..7), &[])),
        (2, layer(Some(1), &stated(&derived_days, 0..7), &[])),
    ]);
    // The derived layer states only the days that differ from its base.
    let differing: Vec<usize> = (0..7)
        .filter(|&i| base_days[i] != derived_days[i])
        .collect();
    let per_day = HashMap::from([
        (1, layer(None, &stated(&base_days, 0..7), &[])),
        (2, layer(Some(1), &stated(&derived_days, differing), &[])),
    ]);

    let from_whole = resolve_in(2, &whole, Holidays::Inherit).unwrap();
    let from_per_day = resolve_in(2, &per_day, Holidays::Inherit).unwrap();
    assert_eq!(from_whole.week, from_per_day.week);
    for (day, expected) in from_whole.week.iter().zip(derived_days) {
        assert_eq!(day.as_slice(), expected);
    }
}

fn stated<'a>(
    days: &[&'a [DayPeriod]; 7],
    indexes: impl IntoIterator<Item = usize>,
) -> Vec<(usize, &'a [DayPeriod])> {
    indexes
        .into_iter()
        .map(|index| (index, days[index]))
        .collect()
}

#[test]
fn the_fallback_week_is_monday_to_friday_eight_to_five() {
    let week = fallback_week();
    for day in &week[..5] {
        assert_eq!(day.as_slice(), [DAY]);
    }
    assert!(week[5].is_empty() && week[6].is_empty());
}

fn entry(day: i64, periods: &[DayPeriod]) -> Exception {
    (day, periods.to_vec())
}

#[test]
fn dedupe_keeps_the_first_and_counts_differing_repeats() {
    let mut collected = Notes::new();
    let kept = dedupe(
        vec![
            entry(3, &[(480, 720)]),
            entry(3, &[(540, 600)]),
            entry(3, &[(600, 660)]),
            entry(4, &[]),
        ],
        7,
        &mut collected,
    );
    let notes = collected.finish();

    assert_eq!(kept, [entry(3, &[(480, 720)]), entry(4, &[])]);
    assert_eq!(
        notes,
        ["Calendar 7: exceptions that repeat a date with different hours were not read (2); the first on each date is used, so some days can differ from the file's.".to_owned()]
    );
}

#[test]
fn dedupe_merges_identical_repeats_silently() {
    let mut collected = Notes::new();
    let kept = dedupe(vec![entry(2, &[]), entry(2, &[])], 1, &mut collected);
    let notes = collected.finish();

    assert_eq!(kept, [entry(2, &[])]);
    assert_eq!(notes, Vec::<String>::new());
}

#[test]
fn week_from_names_the_nearest_calendar_that_states_a_day() {
    let layers = HashMap::from([
        (1, standard()),
        (2, layer(Some(1), &[], &[])),
        (3, layer(Some(2), &[(0, &[SHORT])], &[])),
        (4, layer(Some(5), &[], &[])),
        (5, layer(Some(4), &[], &[])),
    ]);
    let from = |start| {
        resolve_in(start, &layers, Holidays::Own)
            .expect("calendar exists")
            .week_from
    };
    assert_eq!(from(1), Some(1));
    assert_eq!(from(2), Some(1));
    assert_eq!(from(3), Some(3));
    assert_eq!(from(4), None);
}
