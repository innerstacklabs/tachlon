//! The slide rules (gs#481): the ease, the stagger and the blend of `timeline::slide`.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{Moved, TaskDates, TaskId};
use timeline::slide::{self, SLIDE_MS, STAGGER_MS};

/// A task that is now at `es..ef`, with distinct late dates and float so a blend that touches them shows.
fn dates(es: i64, ef: i64) -> TaskDates {
    TaskDates {
        es,
        ef,
        ls: es + 7_000,
        lf: ef + 7_001,
        float: 7_002,
        finished: false,
    }
}

fn moved(task: u32, start: i64, finish: i64) -> Moved {
    Moved {
        task: TaskId(task),
        was_es: start,
        was_ef: finish,
    }
}

/// Four tasks. Tasks 0, 1 and 2 moved later; task 3 did not move and is not listed. The old starts
/// are 4,000 (task 0), 0 (task 1) and 2,000 (task 2), so the order by old start (1, 2, 0) differs
/// from the order by id (0, 1, 2), from its reverse (2, 1, 0) and from the list order (2, 0, 1).
fn fixture() -> (Vec<Moved>, Vec<TaskDates>) {
    let solution = vec![
        dates(10_000, 10_500),
        dates(12_000, 12_500),
        dates(14_000, 14_500),
        dates(500, 900),
    ];
    let entries = vec![
        moved(2, 2_000, 2_500),
        moved(0, 4_000, 4_500),
        moved(1, 0, 500),
    ];
    (entries, solution)
}

/// How far `now` is from `from` toward `to`, 0 to 1.
fn progress(from: i64, to: i64, now: i64) -> f64 {
    (now - from) as f64 / (to - from) as f64
}

#[test]
fn the_ease_is_a_cubic_that_settles() {
    assert_eq!(slide::ease(0.0), 0.0);
    assert_eq!(slide::ease(1.0), 1.0);
    assert_eq!(slide::ease(0.5), 0.875);
    assert_eq!(slide::ease(-3.0), 0.0);
    assert_eq!(slide::ease(3.0), 1.0);
    let mut last = 0.0;
    for i in 1..=20 {
        let e = slide::ease(f64::from(i) / 20.0);
        assert!(e > last, "ease is not rising at {i}/20");
        last = e;
    }
}

#[test]
fn the_slide_starts_at_the_old_extent_and_ends_at_the_new() {
    let (mut entries, solution) = fixture();
    slide::sort_by_old_start(&mut entries);
    let end = slide::total_ms(entries.len());

    let mut scratch = solution.clone();
    slide::apply(&entries, &solution, 0.0, &mut scratch);
    for m in &entries {
        let t = &scratch[m.task.0 as usize];
        assert_eq!(
            (t.es, t.ef),
            (m.was_es, m.was_ef),
            "task {} at t = 0",
            m.task.0
        );
    }

    slide::apply(&entries, &solution, end, &mut scratch);
    for m in &entries {
        let i = m.task.0 as usize;
        assert_eq!(
            (scratch[i].es, scratch[i].ef),
            (solution[i].es, solution[i].ef),
            "task {i} at the end"
        );
    }
    // Past the end nothing moves further.
    let at_end = scratch.clone();
    slide::apply(&entries, &solution, end + 5_000.0, &mut scratch);
    assert_eq!(scratch, at_end);
}

#[test]
fn the_midpoint_is_strictly_between_for_early_start_and_early_finish() {
    // One task: no stagger, so the slide is SLIDE_MS long and its midpoint is SLIDE_MS / 2.
    let solution = vec![dates(5_000, 5_900)];
    let entries = vec![moved(0, 1_000, 1_400)];
    let mut scratch = solution.clone();
    slide::apply(&entries, &solution, SLIDE_MS / 2.0, &mut scratch);
    let t = &scratch[0];
    assert!(
        1_000 < t.es && t.es < 5_000,
        "es {} is not between 1000 and 5000",
        t.es
    );
    assert!(
        1_400 < t.ef && t.ef < 5_900,
        "ef {} is not between 1400 and 5900",
        t.ef
    );
    // The ease-out cubic is 87.5% of the way at half the time: 1000 + 0.875 * 4000.
    assert_eq!(t.es, 4_500);
    // The duration changed (400 to 900 minutes), so the extent stretches on the way.
    let extent = t.ef - t.es;
    assert!(
        400 < extent && extent < 900,
        "extent {extent} is not between 400 and 900"
    );
}

#[test]
fn the_stagger_follows_the_old_start() {
    let (mut entries, solution) = fixture();
    // Listed as tasks 2, 0, 1 with old starts 2000, 4000, 0; the order by old start is 1, 2, 0.
    slide::sort_by_old_start(&mut entries);
    assert_eq!(
        entries.iter().map(|m| m.task.0).collect::<Vec<_>>(),
        [1, 2, 0]
    );

    assert_eq!(slide::delay(0, 3), 0.0);
    assert_eq!(slide::delay(1, 3), STAGGER_MS / 2.0);
    assert_eq!(slide::delay(2, 3), STAGGER_MS);

    // 100 ms in: rank 0 is 100/240 along, rank 1 is 40/240 along, rank 2 has not started.
    let mut scratch = solution.clone();
    slide::apply(&entries, &solution, 100.0, &mut scratch);
    let p: Vec<f64> = entries
        .iter()
        .map(|m| {
            let i = m.task.0 as usize;
            progress(m.was_es, solution[i].es, scratch[i].es)
        })
        .collect();
    assert!(
        p[0] > p[1] && p[1] > 0.0,
        "earlier old start should be further along: {p:?}"
    );
    assert_eq!(
        p[2], 0.0,
        "the last rank starts {STAGGER_MS} ms late: {p:?}"
    );
}

#[test]
fn the_stagger_is_at_most_stagger_ms_however_many_move() {
    for count in [2, 3, 10, 1_000] {
        assert_eq!(slide::delay(count - 1, count), STAGGER_MS);
        let mut last = -1.0;
        for rank in 0..count {
            let d = slide::delay(rank, count);
            assert!(d >= last && d <= STAGGER_MS, "delay({rank}, {count}) = {d}");
            last = d;
        }
    }
    assert_eq!(slide::delay(0, 1), 0.0);
    assert_eq!(slide::delay(0, 0), 0.0);
}

#[test]
fn the_end_is_slide_ms_plus_stagger_ms_for_two_or_more() {
    assert_eq!(slide::total_ms(0), 0.0);
    assert_eq!(slide::total_ms(1), SLIDE_MS);
    for count in [2, 3, 50] {
        assert_eq!(slide::total_ms(count), SLIDE_MS + STAGGER_MS);
    }

    // The last task is still short of its target just before the end, and on it at the end.
    let (mut entries, solution) = fixture();
    slide::sort_by_old_start(&mut entries);
    let end = slide::total_ms(entries.len());
    let mut scratch = solution.clone();
    slide::apply(&entries, &solution, end - 24.0, &mut scratch);
    assert_ne!(scratch[0].es, solution[0].es);
    slide::apply(&entries, &solution, end, &mut scratch);
    assert_eq!(scratch[0].es, solution[0].es);
}

#[test]
fn entries_absent_from_the_list_are_untouched() {
    let (mut entries, solution) = fixture();
    slide::sort_by_old_start(&mut entries);
    // Task 3 is not in the list; give its scratch slot values the solution does not hold.
    let mut scratch = solution.clone();
    scratch[3] = dates(-7, -3);
    let before = scratch[3];
    for t in [0.0, 50.0, 180.0, slide::total_ms(entries.len()), 9_999.0] {
        slide::apply(&entries, &solution, t, &mut scratch);
        assert_eq!(scratch[3], before, "task 3 changed at t = {t}");
    }
}

#[test]
fn late_dates_and_float_stay_the_solutions() {
    let (mut entries, solution) = fixture();
    slide::sort_by_old_start(&mut entries);
    let end = slide::total_ms(entries.len());
    for t in [0.0, 40.0, 120.0, 200.0, end] {
        let mut scratch = solution.clone();
        slide::apply(&entries, &solution, t, &mut scratch);
        for (i, (got, want)) in scratch.iter().zip(&solution).enumerate() {
            assert_eq!(
                (got.ls, got.lf, got.float),
                (want.ls, want.lf, want.float),
                "task {i} at t = {t}"
            );
        }
    }
}

#[test]
fn an_id_past_the_end_is_skipped() {
    let solution = vec![dates(100, 200)];
    let entries = vec![moved(0, 0, 50), moved(9, 0, 50)];
    let mut scratch = solution.clone();
    slide::apply(&entries, &solution, 0.0, &mut scratch);
    assert_eq!((scratch[0].es, scratch[0].ef), (0, 50));
}

#[test]
fn a_blended_minute_is_rounded_not_floored() {
    // Half the time is 0.875 of the way. Moving later by 10: 8.75 rounds to 9 (a floor gives 8).
    // Moving earlier by 6: -5.25 rounds to -5 (a floor gives -6).
    let solution = vec![dates(110, 194)];
    let entries = vec![moved(0, 100, 200)];
    let mut scratch = solution.clone();
    slide::apply(&entries, &solution, SLIDE_MS / 2.0, &mut scratch);
    assert_eq!((scratch[0].es, scratch[0].ef), (109, 195));
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not sorted by old start")]
fn an_unsorted_list_trips_the_debug_assert() {
    let solution = vec![dates(100, 200), dates(300, 400)];
    // Old starts 50 then 10: not ascending.
    let entries = vec![moved(0, 50, 60), moved(1, 10, 20)];
    let mut scratch = solution.clone();
    slide::apply(&entries, &solution, 0.0, &mut scratch);
}
