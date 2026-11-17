//! `load_bytes` and `timed_load_bytes`: the open timings and the MSPDI golden results (gs#87 PR 1).
//!
//! The other `open_*.rs` files cover project selection, the data date, activity IDs and file
//! dates; the fixture helpers they share are in `common::xer_text`.

use crate::common;

use std::cell::Cell;

use app::{
    OpenError, OpenTimings, Session, document, document::Extras, fitted_timeline, load_bytes,
    meta::outline_path, timed_load_bytes,
};
use common::xer_text::loaded;
use import::ProjectSelection;
use xer::synth::{Spec, write};

// ---- open timings -------------------------------------------------------------------------

#[test]
fn the_fake_clock_fills_the_open_timings() {
    let readings = [100.0, 110.0, 125.0, 150.5];
    let next = Cell::new(0);
    let clock = || {
        let at = next.get();
        next.set(at + 1);
        readings[at]
    };
    let opened = load_bytes(
        &write(&Spec::small(3)),
        &ProjectSelection::Unspecified,
        &clock,
    )
    .expect("loads");
    assert_eq!(next.get(), 4, "the clock is read four times");
    assert_eq!(
        opened.timings,
        OpenTimings {
            parse: 10.0,
            build: 15.0,
            compute: 25.5,
        }
    );
}

#[test]
fn a_clock_that_goes_backwards_or_is_not_a_number_gives_an_invalid_phase_not_a_zero() {
    let backwards = [50.0, 40.0, 30.0, 20.0];
    let next = Cell::new(0);
    let clock = || {
        let at = next.get();
        next.set(at + 1);
        backwards[at]
    };
    let opened = load_bytes(
        &write(&Spec::small(3)),
        &ProjectSelection::Unspecified,
        &clock,
    )
    .expect("loads");
    assert_eq!(
        opened.timings,
        OpenTimings {
            parse: -10.0,
            build: -10.0,
            compute: -10.0,
        }
    );

    let nan = load_bytes(
        &write(&Spec::small(3)),
        &ProjectSelection::Unspecified,
        &|| f64::NAN,
    )
    .expect("loads");
    assert!(nan.timings.parse.is_nan());
    assert!(nan.timings.build.is_nan());
    assert!(nan.timings.compute.is_nan());
}

#[test]
fn a_failed_load_reads_the_clock_at_most_twice() {
    let reads = Cell::new(0);
    let clock = || {
        reads.set(reads.get() + 1);
        0.0
    };
    let result = load_bytes(b"not a schedule", &ProjectSelection::Unspecified, &clock);
    assert!(result.is_err());
    assert!(reads.get() <= 2);
}

#[test]
fn the_timings_json_carries_the_three_phases_and_the_span_the_page_reads() {
    let timings = OpenTimings {
        parse: 10.0,
        build: 15.5,
        compute: 0.25,
    };
    assert_eq!(
        timings.to_json(25.75),
        r#"{"parse":10,"build":15.5,"compute":0.25,"span":25.75}"#
    );
    assert_eq!(
        OpenTimings::default().to_json(0.0),
        r#"{"parse":0,"build":0,"compute":0,"span":0}"#
    );
}

#[test]
fn an_invalid_phase_stays_invalid_in_the_json_and_is_never_turned_into_zero() {
    // NaN and infinity have no JSON form, so they are `null`; a negative value (a clock that went
    // backwards) is kept, so the page can see the measurement is wrong instead of reading a clean 0.
    let odd = OpenTimings {
        parse: f64::NAN,
        build: f64::INFINITY,
        compute: -3.0,
    };
    assert_eq!(
        odd.to_json(f64::NEG_INFINITY),
        r#"{"parse":null,"build":null,"compute":-3,"span":null}"#
    );
}

#[test]
fn the_span_is_timed_outside_load_bytes_and_disagrees_with_a_phase_that_loses_time() {
    // Six reads: one before, the four inside `load_bytes`, one after. With nothing between the reads
    // the phases add up to the span. A gap between two inside reads cannot happen here (the phases are
    // back to back), so the gap is placed outside: 3 ms before the first inside read and 2 ms after
    // the last, which the span sees and the phases do not.
    let readings = [0.0, 3.0, 13.0, 15.0, 40.0, 42.0];
    let next = Cell::new(0);
    let clock = || {
        let at = next.get();
        next.set(at + 1);
        readings[at]
    };
    let (opened, span) = timed_load_bytes(
        &write(&Spec::small(5)),
        &ProjectSelection::Unspecified,
        &clock,
    )
    .expect("loads");
    let timings = opened.timings;
    assert_eq!(
        (timings.parse, timings.build, timings.compute),
        (10.0, 2.0, 25.0)
    );
    assert_eq!(span, 42.0);
    assert_eq!(
        span - (timings.parse + timings.build + timings.compute),
        5.0,
        "the five milliseconds outside the phases are visible as a difference"
    );
}

#[test]
fn a_failed_timed_load_returns_the_error_and_no_span() {
    let clock = || 0.0;
    let error = timed_load_bytes(b"not a schedule", &ProjectSelection::Unspecified, &clock)
        .map(|_| ())
        .expect_err("fails");
    assert!(matches!(error, OpenError::Message(_)));
}

#[test]
fn the_phases_do_not_overlap_and_leave_nothing_out_between_the_four_clock_reads() {
    // Four reads bound three windows: a phase that ended before the next began would lose the time
    // between them, and a phase that began before the last ended would count it twice. The sum of
    // the three is exactly the span from the first read to the last.
    let readings = [7.0, 19.0, 21.5, 40.0];
    let next = Cell::new(0);
    let clock = || {
        let at = next.get();
        next.set(at + 1);
        readings[at]
    };
    let opened = load_bytes(
        &write(&Spec::small(5)),
        &ProjectSelection::Unspecified,
        &clock,
    )
    .expect("loads");
    let timings = opened.timings;
    assert_eq!(
        timings.parse + timings.build + timings.compute,
        readings[3] - readings[0]
    );
}

// ---- MSPDI is unchanged -------------------------------------------------------------------

/// What `load_schedule` returned for these fixtures before gs#87 PR 1 (captured from `main` at
/// 7b092c6): the `unsupported` lines, every task's early start and finish, the row order, and
/// each task's outline path and outline level.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test lists everything the MS Project result holds"
)]
fn the_mspdi_result_is_what_it_was_before() {
    struct Golden {
        xml: &'static str,
        unsupported: &'static [&'static str],
        dates: &'static [(i64, i64)],
        order: &'static [u32],
        meta: &'static [(&'static str, u8)],
    }
    let goldens = [
        Golden {
            xml: include_str!("../../import/tests/fixtures/constraints.xml"),
            unsupported: &[
                "Task 3: the finish no later than constraint (type 7) has no date and was not read; the task is scheduled without it, so its dates can differ from the file's.",
                "1 task has resource assignments; only the resource names were read, and no dates are affected.",
            ],
            dates: &[(13440, 13920), (1920, 2400), (480, 960)],
            order: &[2, 1, 0],
            meta: &[("SNET", 1), ("MSO", 1), ("FNLT", 1)],
        },
        Golden {
            xml: include_str!("../../import/tests/fixtures/holidays.xml"),
            unsupported: &[],
            dates: &[(480, 11040)],
            order: &[0],
            meta: &[("Four working days", 1)],
        },
        Golden {
            xml: include_str!("../../import/tests/fixtures/links.xml"),
            unsupported: &[],
            dates: &[
                (480, 2400),
                (4800, 11040),
                (1920, 3840),
                (10560, 11040),
                (4800, 5280),
                (12000, 12480),
            ],
            order: &[0, 2, 1, 4, 3, 5],
            meta: &[("A", 1), ("B", 1), ("C", 1), ("D", 1), ("E", 1), ("F", 1)],
        },
        Golden {
            xml: include_str!("../../import/tests/fixtures/summary.xml"),
            unsupported: &[],
            dates: &[
                (480, 480),
                (480, 960),
                (1920, 2400),
                (480, 480),
                (3360, 3840),
            ],
            order: &[0, 1, 3, 2, 4],
            meta: &[
                ("First summary", 1),
                ("First summary / First child", 2),
                ("First summary / Second child", 2),
                ("Second summary", 1),
                ("Second summary / Third child", 2),
            ],
        },
    ];
    for golden in goldens {
        let opened = loaded(golden.xml.as_bytes(), &ProjectSelection::All);
        assert_eq!(opened.unsupported, golden.unsupported);
        let dates: Vec<(i64, i64)> = opened.solution.dates.iter().map(|d| (d.es, d.ef)).collect();
        assert_eq!(dates, golden.dates);
        assert_eq!(opened.order, golden.order);
        // The names are the file's own now; the table still shows the path they derive.
        let meta: Vec<(String, u8)> = opened
            .meta
            .iter()
            .enumerate()
            .map(|(index, meta)| (outline_path(&opened.meta, index), meta.outline_level))
            .collect();
        let golden_meta: Vec<(String, u8)> = golden
            .meta
            .iter()
            .map(|&(path, level)| (path.to_string(), level))
            .collect();
        assert_eq!(meta, golden_meta);
        for (meta, &(path, _)) in opened.meta.iter().zip(golden.meta) {
            assert!(
                path.ends_with(&meta.name),
                "{path:?} ends with {:?}",
                meta.name
            );
        }
        assert_eq!(opened.data_date, None);
    }
}

// ---- a large document -----------------------------------------------------------------------

#[test]
fn a_large_document_opens_with_the_dates_a_full_compute_gives() {
    // `read` drops the parsed tree before the first solve (gantt#1328); the dates must not move.
    let xer = write(&Spec::with_activities(7, 5_000));
    let loaded = load_bytes(&xer, &ProjectSelection::Unspecified, &|| 0.0).expect("loads");
    let timeline = fitted_timeline(loaded.order, &loaded.solution, 1000.0, 400.0, 1.0);
    let session = Session::new(
        loaded.schedule,
        loaded.solution,
        timeline,
        loaded.meta,
        || 0.0,
    )
    .with_outline(loaded.outline)
    .expect("the outline matches")
    .with_origin(loaded.origin)
    .with_data_date(loaded.data_date);
    let extras = Extras {
        name: "large",
        notes: &[],
        source: None,
    };
    let text = document::write(&session, &extras);
    let mut opened = document::read(&text, &|| 0.0).expect("the document reads");
    let fresh = opened.loaded.schedule.compute().expect("solves");
    assert!(fresh.dates.len() >= 5_000, "a large document");
    assert_eq!(opened.loaded.solution, fresh);
    // The document renumbers the tasks, so only the project finish is compared with the first open.
    assert_eq!(
        opened.loaded.solution.project_finish,
        session.solution().project_finish
    );
}
