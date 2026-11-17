//! The summary against the baseline over many edits and over a drag's frames (gs#703): at rest it
//! equals a full recount of two `compute()` results after every step, and during a drag it is
//! computed at most once per interval, with the line keeping the last value in between.

use crate::common;

use app::RenderFacts;
use app::baseline::{BaselineSummary, DRAG_SUMMARY_INTERVAL_MS};
use app::edit::Edit;
use common::baseline::{brute_force, live_set, set};
use common::{TRAVEL_PX, outline_session, setup_with_clock};
use engine::{DAY, TaskId};
use std::sync::atomic::{AtomicU64, Ordering};

const N: usize = 60;
const STEPS: usize = 50;

/// A small deterministic generator, so a failing step can be replayed.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % below
    }
}

#[test]
fn the_summary_equals_a_full_recount_after_fifty_random_edits_and_undos() {
    let (mut session, _) = outline_session(N);
    let leaves: Vec<TaskId> = {
        let outline = session.outline().unwrap();
        outline
            .preorder()
            .iter()
            .copied()
            .filter(|&t| !outline.is_summary(t) && outline.parent(t).is_some())
            .collect()
    };
    let baseline = set(&mut session);
    let (then, live_then) = (session.solution().clone(), live_set(&session));
    let mut rng = Lcg(703);
    let (mut applied, mut seen) = (0, Vec::new());
    for step in 0..STEPS {
        let task = leaves[rng.next(leaves.len())];
        let op = rng.next(9);
        let done = match op {
            0..=3 => {
                let minutes = 480 * (1 + rng.next(8)) as i64;
                session.edit(Edit::SetDuration { task, minutes }).is_ok()
            }
            4 | 5 => {
                let snet = Some(DAY * (1 + rng.next(40)) as i64);
                session.edit(Edit::Move { task, snet }).is_ok()
            }
            6 => session.edit(Edit::Delete { task }).is_ok(),
            7 => session.undo() == Ok(true),
            _ => session.redo() == Ok(true),
        };
        applied += usize::from(done);
        let got = session.baseline_diff().expect("a baseline is set");
        assert_eq!(
            got,
            brute_force(&session, &then, &live_then),
            "step {step}, op {op} on task {}",
            task.0
        );
        seen.push(got);
    }
    assert!(applied >= STEPS / 2, "most steps changed the schedule");
    assert!(
        seen.iter().any(|s| s.slipped > 0) && seen.iter().any(|s| s.removed > 0),
        "the sequence moved the counts, so the equality is not a comparison of zeros"
    );
    assert!(
        seen.iter().any(|s| s.joined + s.left > 0),
        "the critical path changed along the way"
    );
    assert!(
        std::sync::Arc::ptr_eq(session.baseline().unwrap(), &baseline),
        "the baseline was never replaced"
    );
}

static NOW: AtomicU64 = AtomicU64::new(0);

fn clock() -> f64 {
    f64::from_bits(NOW.load(Ordering::Relaxed))
}

fn advance(ms: f64) {
    NOW.store((clock() + ms).to_bits(), Ordering::Relaxed);
}

#[test]
fn a_drag_computes_the_summary_at_most_once_per_interval_and_the_release_is_exact() {
    let facts = RenderFacts::default();
    let (mut session, _, _, point) = setup_with_clock(true, clock);
    let baseline = set(&mut session);
    let (then, live_then) = (session.solution().clone(), live_set(&session));
    assert_eq!(session.baseline_diff(), Some(BaselineSummary::default()));
    assert_eq!(baseline.diffs_computed(), 1);

    advance(1_000.0);
    session.pointer_down(point.0, point.1, false);
    // Each frame asks for a later start, so each is a new solution and a new key.
    let (mut reads, mut last): (Vec<BaselineSummary>, Option<BaselineSummary>) = (Vec::new(), None);
    for frame in 0..30 {
        let x = point.0 + TRAVEL_PX * (frame + 1) as f64 / 30.0;
        session.pointer_move(x, point.1);
        session.apply_drag();
        let computed = baseline.diffs_computed();
        session.status_json(&facts);
        let got = session.baseline_diff().expect("set");
        if frame == 0 {
            assert_eq!(
                got,
                brute_force(&session, &then, &live_then),
                "the first frame is due: the last computation is 1000 ms old"
            );
        } else if baseline.diffs_computed() == computed {
            // Between computations the line keeps the last value.
            assert_eq!(Some(got), last, "frame {frame} reads the last value");
        }
        last = Some(got);
        reads.push(got);
        advance(16.0);
    }
    // 30 frames of 16 ms are 480 ms: the first frame, then one per 150 ms.
    let during = baseline.diffs_computed() - 1;
    assert!(
        (2..=1 + (480.0 / DRAG_SUMMARY_INTERVAL_MS).ceil() as u32).contains(&during),
        "{during} computations in 30 drag frames"
    );
    assert!(
        reads.iter().any(|s| s.slipped > 0),
        "the drag slipped tasks"
    );

    // A read that is due is exact for the solution it reads.
    advance(DRAG_SUMMARY_INTERVAL_MS);
    session.pointer_move(point.0 + TRAVEL_PX / 2.0, point.1);
    session.apply_drag();
    assert_eq!(
        session.baseline_diff(),
        Some(brute_force(&session, &then, &live_then)),
        "a due read mid-drag"
    );

    // The release is read at once, however young the last computation is. The due read above was
    // at this very clock, so a move now is a throttled frame: the line keeps the last value.
    let before = baseline.diffs_computed();
    session.pointer_move(point.0 + TRAVEL_PX / 4.0, point.1);
    session.apply_drag();
    session.baseline_diff();
    assert_eq!(
        baseline.diffs_computed(),
        before,
        "the frame before the release is throttled, so the release meets a young computation"
    );
    session.pointer_up();
    assert_eq!(
        session.baseline_diff(),
        Some(brute_force(&session, &then, &live_then)),
        "after the release"
    );
    assert_eq!(
        baseline.diffs_computed(),
        before + 1,
        "the release computed the exact summary"
    );
    session.baseline_diff();
    assert_eq!(
        baseline.diffs_computed(),
        before + 1,
        "a second read at rest computes nothing"
    );
}
