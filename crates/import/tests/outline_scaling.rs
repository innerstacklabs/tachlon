//! A task's parent is the nearest earlier task with a smaller outline level; finding it is linear.
//! Scaling run: `cargo test --release -p import --test outline_scaling -- --ignored --nocapture`.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use std::fmt::Write as _;
use std::time::Instant;

use import::parse_mspdi;

fn parents(levels: &[u8]) -> Vec<Option<u32>> {
    let mut xml = String::from("<Project><Tasks>");
    for (i, level) in levels.iter().enumerate() {
        write!(
            xml,
            "<Task><UID>{}</UID><OutlineLevel>{level}</OutlineLevel></Task>",
            i + 1
        )
        .expect("a String takes writes");
    }
    let imported = parse_mspdi(&(xml + "</Tasks></Project>")).expect("outline should import");
    imported
        .tasks
        .iter()
        .map(|t| t.parent.map(|id| id.0))
        .collect()
}

/// The old lookup, kept as the oracle: scan back for the nearest smaller level.
fn scan(levels: &[u8]) -> Vec<Option<u32>> {
    (0..levels.len())
        .map(|i| {
            (0..i)
                .rev()
                .find(|&j| levels[j] < levels[i])
                .map(|j| j as u32)
        })
        .collect()
}

#[test]
fn deep_then_back_up_keeps_the_nearest_shallower_parent() {
    let levels = [1, 2, 3, 1, 2];
    assert_eq!(parents(&levels), [None, Some(0), Some(1), None, Some(3)]);
    assert_eq!(parents(&levels), scan(&levels));
}

#[test]
fn mixed_level_sequences_match_the_backward_scan() {
    let mut state = 12345u64;
    for round in 0..300 {
        let levels: Vec<u8> = (0..60)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                1 + ((state >> 33) % (1 + round % 8)) as u8
            })
            .collect();
        assert_eq!(parents(&levels), scan(&levels), "levels {levels:?}");
    }
}

fn flat_parse_seconds(tasks: usize) -> f64 {
    let mut levels = vec![2u8; tasks];
    levels[0] = 1;
    let started = Instant::now();
    parents(&levels);
    started.elapsed().as_secs_f64()
}

#[test]
#[ignore = "timing run; run by hand in release mode"]
fn parse_time_grows_linearly_with_task_count() {
    let (small, large) = (flat_parse_seconds(25_000), flat_parse_seconds(100_000));
    eprintln!(
        "25k {small:.3}s, 100k {large:.3}s, ratio {:.2}",
        large / small
    );
    assert!(large / small < 7.0, "ratio {:.2}", large / small);
}
