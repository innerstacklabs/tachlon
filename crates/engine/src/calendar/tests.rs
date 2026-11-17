use super::*;

fn std_cal() -> Calendar {
    let day = vec![(480, 1020)];
    Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .unwrap()
}

/// Std week with a Saturday morning, 08:00-12:00.
fn sat_week() -> [Vec<DayPeriod>; 7] {
    let mut week = std_cal().week().clone();
    week[5] = vec![(480, 720)];
    week
}

/// The periods that apply on `day`, read straight from the inputs: the
/// exception's if there is one, else the weekday's.
#[expect(
    clippy::cast_sign_loss,
    reason = "the tests pass non-negative days, so day % 7 is in 0..7"
)]
fn day_periods(cal: &Calendar, day: i64) -> Vec<DayPeriod> {
    match cal.exceptions().iter().find(|(d, _)| *d == day) {
        Some((_, periods)) => periods.clone(),
        None => cal.week()[(day % 7) as usize].clone(),
    }
}

/// Is `t` working, by a direct scan of the day's periods?
#[expect(clippy::cast_lossless, reason = "u16 widens to i64 exactly")]
fn scan_working(cal: &Calendar, t: Instant) -> bool {
    let m = t % DAY;
    day_periods(cal, t / DAY)
        .iter()
        .any(|&(s, e)| s as i64 <= m && m < e as i64)
}

fn holidays(days: &[i64]) -> Vec<(i64, Vec<DayPeriod>)> {
    days.iter().map(|&d| (d, Vec::new())).collect()
}

#[test]
fn rejects_bad_calendars() {
    let empty: [Vec<DayPeriod>; 7] = Default::default();
    assert_eq!(
        Calendar::new(empty, &[]).unwrap_err(),
        CalendarError::NoWorkingTime
    );
    // Exceptions cannot rescue a week with no working minute.
    let empty: [Vec<DayPeriod>; 7] = Default::default();
    assert_eq!(
        Calendar::with_exceptions(empty, &[(3, vec![(0, 1440)])]).unwrap_err(),
        CalendarError::NoWorkingTime
    );
    let mut w: [Vec<DayPeriod>; 7] = Default::default();
    w[2] = vec![(600, 700), (650, 800)];
    assert!(matches!(
        Calendar::new(w, &[]),
        Err(CalendarError::BadPeriod { weekday: 2, .. })
    ));
    let mut w: [Vec<DayPeriod>; 7] = Default::default();
    w[0] = vec![(0, 1441)];
    assert!(Calendar::new(w, &[]).is_err());

    let week = sat_week;
    assert_eq!(
        Calendar::new(week(), &[4, -2]).unwrap_err(),
        CalendarError::NegativeException(-2)
    );
    assert_eq!(
        Calendar::with_exceptions(week(), &[(-1, vec![(480, 600)])]).unwrap_err(),
        CalendarError::NegativeException(-1)
    );
    for period in [(600, 600), (700, 600), (0, 1441)] {
        assert_eq!(
            Calendar::with_exceptions(week(), &[(9, vec![period])]).unwrap_err(),
            CalendarError::BadException { day: 9, period }
        );
    }
    assert_eq!(
        Calendar::with_exceptions(week(), &[(9, vec![(600, 700), (650, 800)])]).unwrap_err(),
        CalendarError::BadException {
            day: 9,
            period: (650, 800)
        }
    );
    // Two exceptions on one day are refused, whatever their periods and
    // order; a holiday and a working day on one date included.
    for pair in [
        [(12, vec![]), (12, vec![])],
        [(12, vec![(480, 720)]), (12, vec![(480, 720)])],
        [(12, vec![(480, 720)]), (12, vec![])],
    ] {
        let mut exceptions = vec![(3, vec![]), (40, vec![(0, 1440)])];
        exceptions.extend(pair);
        assert_eq!(
            Calendar::with_exceptions(week(), &exceptions).unwrap_err(),
            CalendarError::DuplicateException(12)
        );
    }
    // `new` keeps merging a holiday given twice, as before.
    assert_eq!(
        Calendar::new(week(), &[9, 3, 9]).unwrap().holidays(),
        [3, 9]
    );
    // A day whose end does not fit in an `Instant` is refused, not a
    // panic or a wrap; the day before it is the last one accepted.
    let daily = || -> [Vec<DayPeriod>; 7] { std::array::from_fn(|_| vec![(0, 1440)]) };
    let late = i64::MAX / DAY;
    assert_eq!(late, MAX_EXCEPTION_DAY + 1);
    assert_eq!(
        Calendar::with_exceptions(daily(), &[(late, vec![(0, 1)])]).unwrap_err(),
        CalendarError::ExceptionTooLate(late)
    );
    assert_eq!(
        Calendar::new(daily(), &[3, late]).unwrap_err(),
        CalendarError::ExceptionTooLate(late)
    );
    let cal = Calendar::with_exceptions(daily(), &[(MAX_EXCEPTION_DAY, vec![(0, 1)])]).unwrap();
    let start = MAX_EXCEPTION_DAY * DAY;
    assert!(cal.is_working(start));
    assert!(!cal.is_working(start + 1));
    assert_eq!(cal.count(start + 1), start + 1);
    assert_eq!(cal.count(start + DAY), start + 1);
}

#[test]
fn accessors_report_the_exceptions_sorted() {
    let cal = Calendar::with_exceptions(
        sat_week(),
        &[
            (12, vec![(480, 1020)]),
            (3, vec![]),
            (6, vec![]), // a Sunday: no effect, still reported
            (9, vec![(480, 720)]),
        ],
    )
    .unwrap();
    assert_eq!(
        cal.exceptions(),
        [
            (3, vec![]),
            (6, vec![]),
            (9, vec![(480, 720)]),
            (12, vec![(480, 1020)]),
        ]
    );
    assert_eq!(cal.holidays(), [3, 6]);
    assert_eq!(cal.weekly_minutes(), 5 * 540 + 240, "the pattern only");
    assert_eq!(cal.week(), &sat_week());
    let plain = Calendar::new(sat_week(), &[9, 3]).unwrap();
    assert_eq!(plain.exceptions(), holidays(&[3, 9]));
}

/// The week indexes give the same exception positions as a binary search,
/// and `count`, `is_working` and `nth` agree with a direct scan of each
/// day's periods, for holidays, shortened days and working weekend days:
/// sparse, clustered, in runs, around the end of the dense table and past
/// the index cap.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test walks every case of the exception index against the binary search"
)]
fn exception_index_matches_binary_search() {
    #[expect(clippy::cast_possible_wrap, reason = "INDEX_CAP is the constant 65536")]
    let far = 7 * INDEX_CAP as i64;
    #[expect(
        clippy::cast_possible_wrap,
        reason = "TABLE_DAYS is the constant 36600"
    )]
    let cap = TABLE_DAYS as i64;
    // Day cap is a Friday, so cap + 2 is a Sunday.
    assert_eq!(cap % 7, 4);
    let sets: Vec<Vec<(i64, Vec<DayPeriod>)>> = vec![
        holidays(&[]),
        holidays(&[3, 9, 10, 40]),
        holidays(&(20..60).collect::<Vec<_>>()),
        holidays(&(0..200).map(|i| i * 3 + i % 2).collect::<Vec<_>>()),
        holidays(&[1, 2, far - 3, far + 4, far + 5, far + 40]),
        holidays(&[6, cap - 2]),
        holidays(&[cap - 2, cap - 1, cap, cap + 2]),
        holidays(&[cap - 1]),
        holidays(&[cap - 2, cap + 2]),
        // 9: shortened and lengthened weekdays; days 10 and 17 repeat the
        // pattern's minutes (17 split in two) and are left out.
        vec![
            (3, vec![(480, 720)]),
            (9, vec![(600, 660), (700, 800)]),
            (10, vec![(480, 1020)]),
            (17, vec![(480, 700), (700, 1020)]),
            (24, vec![(0, 1440)]),
            (40, vec![(480, 600), (600, 1020)]),
        ],
        // 10: working weekend days, a weekend holiday and a short Saturday.
        vec![
            (5, vec![(480, 1020)]),
            (6, vec![(0, 1440)]),
            (12, vec![(600, 700)]),
            (13, vec![(480, 1020)]),
            (19, vec![]),
            (27, vec![(0, 1), (1439, 1440)]),
        ],
        // 11: a run of mixed exceptions on consecutive days.
        (20..80)
            .map(|d| match d % 3 {
                0 => (d, vec![]),
                1 => (d, vec![(480, 600)]),
                _ => (d, vec![(300, 1300)]),
            })
            .collect(),
        // 12: past the index cap.
        vec![
            (1, vec![(0, 1440)]),
            (2, vec![]),
            (far - 3, vec![(480, 600)]),
            (far + 4, vec![(0, 1440)]),
            (far + 5, vec![]),
            (far + 40, vec![(100, 200), (300, 400)]),
        ],
        // 13: around the table end, with a working Sunday past it.
        vec![
            (cap - 2, vec![(480, 600)]),
            (cap - 1, vec![(0, 1440)]),
            (cap, vec![]),
            (cap + 2, vec![(480, 1020)]),
        ],
        // 14: the tightest complete table, ending on a working exception.
        vec![(cap - 1, vec![(0, 1440)])],
        // 15: every day an exception for 400 days.
        (0..400)
            .map(|d| match d % 4 {
                0 => (d, vec![(0, 1440)]),
                1 => (d, vec![]),
                2 => (d, vec![(480, 540)]),
                _ => (d, vec![(60, 120), (1200, 1440)]),
            })
            .collect(),
    ];
    for (set_index, exceptions) in sets.into_iter().enumerate() {
        let cal = Calendar::with_exceptions(sat_week(), &exceptions).unwrap();
        let label = format!("set {set_index}");
        let near = exceptions
            .iter()
            .flat_map(|&(d, _)| d.max(8) - 8..d + 9)
            .collect::<Vec<_>>();
        let around_cap = exceptions
            .iter()
            .any(|&(d, _)| (d - cap).abs() < 10)
            .then_some(cap - 10..cap + 11)
            .into_iter()
            .flatten();
        for day in (0..300).chain(near).chain(around_cap) {
            let want = cal.exdays.partition_point(|x| x.day < day);
            assert_eq!(cal.exception_index(day), want, "{label} day {day}");
            let start = day * DAY;
            let periods = day_periods(&cal, day);
            // The day's total, from the periods alone.
            assert_eq!(
                cal.count(start + DAY) - cal.count(start),
                minutes_before(&periods, DAY),
                "{label} minutes of day {day}"
            );
            for offset in [0, 1, 300, 480, 600, 719, 720, 1019, 1300, 1439] {
                let t = start + offset;
                assert_eq!(cal.count(t), cal.count_search(t), "{label} count {t}");
                assert_eq!(
                    cal.count(t) - cal.count(start),
                    minutes_before(&periods, offset),
                    "{label} count within day {t}"
                );
                assert_eq!(
                    cal.is_working(t),
                    scan_working(&cal, t),
                    "{label} is_working {t}"
                );
            }
            for k in [cal.count(start), cal.count(start + 600)] {
                let want = cal.key.partition_point(|&f| f <= k);
                let got = first_not_before(&cal.key, &cal.by_key, k / cal.weekly, |&f| f <= k);
                assert_eq!(got, want, "{label} k {k}");
                let t = cal.nth(k);
                assert!(scan_working(&cal, t), "{label} nth {k} = {t} is working");
                assert_eq!(cal.count(t), k, "{label} nth {k}");
            }
        }
        match set_index {
            5 => {
                assert_eq!(cal.day_table.len(), TABLE_DAYS);
                assert!(cal.tail_removed.is_some());
                // day 6 is a Sunday: a holiday on a non-working day sets no bit.
                assert_eq!(cal.day_table[6] & 1, 0);
                for day in [cap - 1, cap] {
                    let t = day * DAY + 600;
                    assert_eq!(cal.count(t), cal.count_search(t));
                }
            }
            6 | 13 => {
                assert_eq!(cal.day_table.len(), TABLE_DAYS);
                assert!(cal.tail_removed.is_none());
            }
            7 | 14 => {
                // The tightest complete table: the last entry carries the bit.
                assert!(cal.tail_removed.is_some());
                assert_eq!(cal.day_table.len(), TABLE_DAYS);
                assert_eq!(cal.day_table[TABLE_DAYS - 1] & 1, 1);
            }
            8 => {
                // Day cap + 2 is a Sunday: it is dropped and does not truncate.
                assert_eq!(cal.exdays.len(), 1);
                assert!(cal.tail_removed.is_some());
            }
            9 => {
                let days: Vec<i64> = cal.exdays.iter().map(|x| x.day).collect();
                assert_eq!(days, [3, 9, 24, 40], "pattern repeats are left out");
                // Day 24 (a Thursday, 24 h) and day 40 (a 9 h Saturday) add time.
                assert!(cal.removed.last().unwrap() < &0);
            }
            10 => {
                assert!(cal.removed.last().unwrap() < &0, "the weekends add time");
            }
            _ => {}
        }
    }
}

/// `count` and `nth` invert each other on every working minute, and
/// `count` rises by one exactly on the minutes a direct scan calls
/// working: a holiday calendar, and one with a working Saturday, a working
/// Sunday and a 4 h day.
#[test]
fn count_and_nth_are_inverse_on_working_minutes() {
    let cals = [
        Calendar::new(sat_week(), &[3, 9, 10, 40]).unwrap(),
        Calendar::with_exceptions(
            sat_week(),
            &[
                (3, vec![]),
                (5, vec![(480, 1020)]),
                (6, vec![(480, 720)]),
                (9, vec![(480, 720)]),
                (10, vec![]),
                (12, vec![(0, 1440)]),
                (40, vec![(600, 840)]),
                (41, vec![(0, 60), (1380, 1440)]),
            ],
        )
        .unwrap(),
    ];
    for (i, cal) in cals.iter().enumerate() {
        let mut k = 0;
        for t in 0..20 * WEEK {
            assert_eq!(cal.count(t), k, "calendar {i} count at {t}");
            let working = scan_working(cal, t);
            assert_eq!(cal.is_working(t), working, "calendar {i} is_working {t}");
            if working {
                assert_eq!(cal.nth(k), t, "calendar {i} nth {k}");
                k += 1;
            }
        }
    }
}
