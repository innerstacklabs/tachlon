#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{
    SEED, Session,
    edit::Edit,
    outline::{FULL_ROLLUP_DIVISOR, Outline, RollupScratch, demo_layout},
};
use common::{clock, outline_session};
use engine::{DAY, Moved, Rng, TaskId};
use std::time::Instant as Clock;
use timeline::{Summaries, Timeline};

const MINUTES_PER_PX: f64 = 100.0;
#[rustfmt::skip]
const BUCKET_LABELS: [&str; 10] = ["<1/512", "1/512..1/256", "1/256..1/128", "1/128..1/64", "1/64..1/32", "1/32..1/16", "1/16..1/8", "1/8..1/4", "1/4..1/2", ">=1/2"];

/// Independent oracle: scan every leaf descendant, never direct children.
fn brute(session: &Session) -> Summaries {
    let outline = session.outline().expect("test session has an outline");
    let mut answer = Summaries::new(outline.len());
    for &task in outline.preorder() {
        if !outline.is_summary(task) {
            continue;
        }
        let span = outline
            .subtree(task)
            .iter()
            .copied()
            .filter(|&descendant| !outline.is_summary(descendant))
            .map(|leaf| {
                let dates = session.solution().get(leaf);
                (dates.es, dates.ef)
            })
            .reduce(|(min_es, max_ef), (es, ef)| (min_es.min(es), max_ef.max(ef)))
            .expect("a summary has a leaf descendant");
        answer.set(task.0, Some(span));
    }
    answer
}

fn assert_exact(session: &Session) {
    assert_eq!(session.timeline().summaries(), &brute(session));
    let outline = session.outline().expect("test session has an outline");
    let mut full = Summaries::new(outline.len());
    outline.rollup_full(&session.solution().dates, &mut full);
    assert_eq!(session.timeline().summaries(), &full);
    assert_eq!(
        session.solution(),
        &session.schedule().clone().compute().expect("fresh compute")
    );
}

fn leaves(session: &Session) -> Vec<TaskId> {
    let outline = session.outline().unwrap();
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| !outline.is_summary(task))
        .collect()
}

fn random_leaf(session: &Session, rng: &mut Rng) -> TaskId {
    let candidates = leaves(session);
    candidates[rng.range(0, candidates.len() as i64 - 1) as usize]
}

/// The ids of a moved list: what `Outline::rollup_moved` takes.
fn ids(moved: &[Moved]) -> Vec<TaskId> {
    moved.iter().map(|m| m.task).collect()
}

fn replay_move(session: &Session, task: TaskId, snet: Option<i64>) {
    let outline = session.outline().unwrap();
    let mut schedule = session.schedule().clone();
    let mut solution = session.solution().clone();
    let mut incremental = session.timeline().summaries().clone();
    schedule.set_snet(task, snet).expect("known task");
    let mut moved = Vec::new();
    schedule
        .recompute_cone_moved(&mut solution, task, &mut moved)
        .expect("valid move");
    outline.rollup_moved(
        &solution.dates,
        &ids(&moved),
        &mut RollupScratch::default(),
        &mut incremental,
    );
    let mut full = Summaries::new(outline.len());
    outline.rollup_full(&solution.dates, &mut full);
    assert_eq!(incremental, full, "moved-only replay differs from full");
}

fn visible_bars(session: &Session) -> Vec<(TaskId, usize)> {
    leaves(session)
        .into_iter()
        .filter_map(|task| {
            let row = session.timeline().row_of_task(task.0)?;
            let dates = session.solution().get(task);
            (dates.ef - dates.es >= 800).then_some((task, row))
        })
        .collect()
}

fn drag_random_leaf(session: &mut Session, rng: &mut Rng, cancel: bool) {
    let mut candidates = visible_bars(session);
    if candidates.is_empty() {
        let collapsed: Vec<_> = session
            .outline()
            .unwrap()
            .preorder()
            .iter()
            .copied()
            .filter(|&task| session.is_collapsed(task))
            .collect();
        for task in collapsed {
            assert!(session.toggle(task));
        }
        candidates = visible_bars(session);
    }
    let (task, row) = candidates[rng.range(0, candidates.len() as i64 - 1) as usize];
    let es = session.solution().get(task).es;
    common::zoom_to(session, MINUTES_PER_PX, es as f64 - 500.0 * MINUTES_PER_PX);
    session.scroll_to_row(row);
    let viewport = &session.timeline().viewport;
    let point = (
        viewport.x_of(es as f64) + 2.0,
        viewport.row_top(row) + viewport.row_height / 2.0,
    );
    assert_eq!(
        session
            .timeline()
            .hit_test(&session.solution().dates, point.0, point.1),
        Some(task.0)
    );
    session.pointer_down(point.0, point.1, false);
    session.pointer_move(point.0 + rng.range(20, 400) as f64, point.1);
    session.apply_drag();
    if cancel {
        session.pointer_cancel();
    } else {
        session.pointer_up();
    }
}

#[test]
fn summaries_match_brute_force_after_with_outline() {
    let (session, _) = outline_session(2_000);
    let outline = session.outline().unwrap();
    assert!(
        outline
            .preorder()
            .iter()
            .any(|&task| outline.is_summary(task))
    );
    assert!(outline.preorder().iter().any(|&task| {
        outline.is_summary(task)
            && outline
                .children(task)
                .iter()
                .any(|&child| outline.is_summary(child))
    }));
    assert_eq!(session.timeline().summaries(), &brute(&session));
    for &task in outline.preorder() {
        if !outline.is_summary(task) {
            assert_eq!(session.timeline().summaries().get(task.0), None);
        }
    }

    let layout = demo_layout(20);
    let timeline = Timeline::new(layout.order, 100.0, 100.0, 1.0);
    let bare = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    );
    assert_eq!(bare.timeline().summaries().len(), 0);
}

#[test]
fn random_edit_sequence_keeps_summaries_exact() {
    let (mut session, _) = outline_session(800);
    let mut rng = Rng::new(SEED ^ 0x47D1_B00B);
    let mut operations = Vec::new();
    while operations.len() < 300 {
        let mut random_prefix = [0, 1, 2, 3, 4];
        for i in (1..random_prefix.len()).rev() {
            let j = rng.range(0, i as i64) as usize;
            random_prefix.swap(i, j);
        }
        operations.extend(random_prefix);
        operations.extend([5, 6, 7]);
    }
    operations.truncate(300);

    let mut counts = [0_usize; 8];
    let (mut undo_changed, mut redo_changed) = (false, false);
    for operation in operations {
        let oracle_before = brute(&session);
        counts[operation] += 1;
        match operation {
            0 => drag_random_leaf(&mut session, &mut rng, false),
            1 => drag_random_leaf(&mut session, &mut rng, true),
            2 => {
                let task = random_leaf(&session, &mut rng);
                let minutes = if counts[operation] % 4 == 1 {
                    0
                } else {
                    rng.range(1, 3 * DAY)
                };
                session
                    .edit(Edit::SetDuration { task, minutes })
                    .expect("duration edit");
            }
            3 => {
                let task = random_leaf(&session, &mut rng);
                replay_move(&session, task, None);
                session
                    .edit(Edit::Move { task, snet: None })
                    .expect("clear constraint");
            }
            4 => {
                let summaries: Vec<_> = session
                    .outline()
                    .unwrap()
                    .preorder()
                    .iter()
                    .copied()
                    .filter(|&task| session.outline().unwrap().is_summary(task))
                    .collect();
                let task = summaries[rng.range(0, summaries.len() as i64 - 1) as usize];
                assert!(session.toggle(task));
            }
            5 => {
                let task = random_leaf(&session, &mut rng);
                let es = session.solution().get(task).es;
                let k = (session.solution().project_finish - es) / DAY + rng.range(2, 12);
                let snet = Some(es + k * DAY);
                replay_move(&session, task, snet);
                session.edit(Edit::Move { task, snet }).expect("move edit");
            }
            6 => {
                assert!(session.undo().expect("undo"));
                undo_changed |= brute(&session) != oracle_before;
            }
            7 => {
                assert!(session.redo().expect("redo"));
                redo_changed |= brute(&session) != oracle_before;
            }
            _ => unreachable!(),
        }
        assert_exact(&session);
    }
    assert!(counts.iter().all(|&count| count >= 5), "counts: {counts:?}");
    assert!(undo_changed, "an undo must change a summary span");
    assert!(redo_changed, "a redo must change a summary span");
}

fn leaf_with_successor_and_parent(session: &Session) -> TaskId {
    let outline = session.outline().unwrap();
    leaves(session)
        .into_iter()
        .find(|&task| {
            outline.parent(task).is_some()
                && session
                    .schedule()
                    .dependencies()
                    .iter()
                    .any(|dependency| dependency.pred == task)
        })
        .expect("demo has a nested leaf with a successor")
}

#[test]
fn undo_and_redo_restore_summaries_exactly() {
    let (mut session, _) = outline_session(800);
    let task = leaf_with_successor_and_parent(&session);
    let before_move = session.timeline().summaries().clone();
    let es = session.solution().get(task).es;
    let k = (session.solution().project_finish - es) / DAY + 30;
    session
        .edit(Edit::Move {
            task,
            snet: Some(es + k * DAY),
        })
        .unwrap();
    let after_move = session.timeline().summaries().clone();
    assert_ne!(after_move, before_move);
    assert!(session.undo().unwrap());
    assert_eq!(session.timeline().summaries(), &before_move);
    assert_eq!(session.timeline().summaries(), &brute(&session));
    assert!(session.redo().unwrap());
    assert_eq!(session.timeline().summaries(), &after_move);

    let before_duration = session.timeline().summaries().clone();
    let minutes = session.schedule().tasks()[task.0 as usize].duration + 100 * DAY;
    session.edit(Edit::SetDuration { task, minutes }).unwrap();
    let after_duration = session.timeline().summaries().clone();
    assert_ne!(after_duration, before_duration);
    assert!(session.undo().unwrap());
    assert_eq!(session.timeline().summaries(), &before_duration);
    assert_eq!(session.timeline().summaries(), &brute(&session));
    assert!(session.redo().unwrap());
    assert_eq!(session.timeline().summaries(), &after_duration);
    assert_eq!(session.timeline().summaries(), &brute(&session));
}

#[test]
fn a_failed_edit_leaves_summaries_alone() {
    let (mut session, _) = outline_session(200);
    let before = session.timeline().summaries().clone();
    let task = random_leaf(&session, &mut Rng::new(47));
    let edit = Edit::SetDuration { task, minutes: -1 };
    assert!(session.edit(edit).is_err());
    assert_eq!(session.timeline().summaries(), &before);
}

fn leaf_ancestor_unrelated(outline: &Outline) -> (TaskId, TaskId, TaskId) {
    let root = |mut task| {
        while let Some(parent) = outline.parent(task) {
            task = parent;
        }
        task
    };
    for &leaf in outline.preorder() {
        let Some(parent) = outline.parent(leaf) else {
            continue;
        };
        if outline.is_summary(leaf) {
            continue;
        }
        if let Some(&unrelated) = outline
            .preorder()
            .iter()
            .find(|&&summary| outline.is_summary(summary) && root(summary) != root(leaf))
        {
            return (leaf, parent, unrelated);
        }
    }
    panic!("demo has separate summary branches");
}

#[test]
fn rollup_moved_with_an_empty_or_leaf_only_moved_list() {
    let (session, _) = outline_session(200);
    let outline = session.outline().unwrap();
    let dates = &session.solution().dates;
    let mut summaries = session.timeline().summaries().clone();
    let unchanged = summaries.clone();
    let mut scratch = RollupScratch::default();
    outline.rollup_moved(dates, &[], &mut scratch, &mut summaries);
    assert_eq!(summaries, unchanged);

    let (leaf, ancestor, unrelated) = leaf_ancestor_unrelated(outline);
    summaries.set(ancestor.0, None);
    summaries.set(unrelated.0, None);
    outline.rollup_moved(dates, &[leaf], &mut scratch, &mut summaries);
    assert!(summaries.get(ancestor.0).is_some());
    assert_eq!(summaries.get(unrelated.0), None);
    outline.rollup_full(dates, &mut summaries);
    assert!(summaries.get(unrelated.0).is_some());
}

#[test]
fn hybrid_switches_to_the_full_pass_above_the_threshold() {
    let (session, _) = outline_session(200);
    let outline = session.outline().unwrap();
    let (leaf, _, unrelated) = leaf_ancestor_unrelated(outline);
    let mut summaries = session.timeline().summaries().clone();
    let mut scratch = RollupScratch::default();
    summaries.set(unrelated.0, None);
    let dates = &session.solution().dates;
    outline.rollup(dates, &[leaf], &mut scratch, &mut summaries);
    assert_eq!(summaries.get(unrelated.0), None, "incremental path");
    let at_threshold = vec![leaf; outline.len() / FULL_ROLLUP_DIVISOR];
    outline.rollup(dates, &at_threshold, &mut scratch, &mut summaries);
    assert_eq!(
        summaries.get(unrelated.0),
        None,
        "still incremental at the threshold"
    );
    let large = vec![leaf; outline.len() / FULL_ROLLUP_DIVISOR + 1];
    outline.rollup(dates, &large, &mut scratch, &mut summaries);
    assert!(summaries.get(unrelated.0).is_some(), "full path");
}

fn bucket(moved: usize, n: usize) -> usize {
    [512, 256, 128, 64, 32, 16, 8, 4, 2]
        .into_iter()
        .position(|denominator| moved * denominator < n)
        .unwrap_or(9)
}

fn median(samples: &mut [f64]) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// Best of three runs of `work`, in milliseconds (the work is idempotent).
fn best_ms(mut work: impl FnMut()) -> f64 {
    (0..3)
        .map(|_| {
            let started = Clock::now();
            work();
            started.elapsed().as_secs_f64() * 1_000.0
        })
        .fold(f64::INFINITY, f64::min)
}

fn sweep_buckets(n: usize, drags: usize) -> Vec<Vec<(f64, f64)>> {
    let mut buckets = vec![Vec::<(f64, f64)>::new(); BUCKET_LABELS.len()];
    let layout = demo_layout(n);
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).unwrap();
    let (mut schedule, mut solution) = (layout.schedule, layout.solution);
    let mut leaf_ids: Vec<_> = outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| !outline.is_summary(task))
        .collect();
    // Early leaves have the largest cones; half the drags come from the
    // earliest 2% so every bucket gets samples.
    leaf_ids.sort_by_key(|&task| solution.get(task).es);
    let early = (leaf_ids.len() / 50).max(1);
    let mut summaries = Summaries::new(n);
    outline.rollup_full(&solution.dates, &mut summaries);
    let mut scratch = RollupScratch::default();
    let mut rng = Rng::new(SEED ^ n as u64);
    for drag in 0..drags {
        let pool = if drag % 2 == 0 { early } else { leaf_ids.len() };
        let task = leaf_ids[rng.range(0, pool as i64 - 1) as usize];
        let old_snet = schedule.tasks()[task.0 as usize].snet;
        schedule
            .set_snet(task, Some(solution.get(task).es + 5 * DAY))
            .unwrap();
        let mut moved = Vec::new();
        schedule
            .recompute_cone_moved(&mut solution, task, &mut moved)
            .unwrap();
        let moved_ids = ids(&moved);
        let mut incremental = summaries.clone();
        let incremental_ms = best_ms(|| {
            outline.rollup_moved(&solution.dates, &moved_ids, &mut scratch, &mut incremental);
        });
        let mut full = summaries.clone();
        let full_ms = best_ms(|| outline.rollup_full(&solution.dates, &mut full));
        assert_eq!(incremental, full);
        buckets[bucket(moved.len(), n)].push((incremental_ms, full_ms));

        schedule.set_snet(task, old_snet).unwrap();
        schedule
            .recompute_cone_moved(&mut solution, task, &mut moved)
            .unwrap();
        outline.rollup_moved(&solution.dates, &ids(&moved), &mut scratch, &mut summaries);
    }
    buckets
}

#[test]
#[ignore = "a timing sweep, run by hand with --ignored --nocapture"]
fn hybrid_threshold_sweep() {
    for n in [10_000, 50_000, 100_000] {
        println!("n = {n}");
        let buckets = sweep_buckets(n, 600);
        let mut crossover = None;
        for (index, samples) in buckets.iter().enumerate() {
            if samples.is_empty() {
                println!("  {}: count=0", BUCKET_LABELS[index]);
                continue;
            }
            let mut incremental: Vec<_> = samples.iter().map(|sample| sample.0).collect();
            let mut full: Vec<_> = samples.iter().map(|sample| sample.1).collect();
            let (incremental_ms, full_ms) = (median(&mut incremental), median(&mut full));
            let winner = if full_ms < incremental_ms {
                crossover.get_or_insert(index);
                "full"
            } else {
                "incremental"
            };
            println!(
                "  {}: count={}, median incremental={incremental_ms:.4} ms, median full={full_ms:.4} ms, {winner} wins",
                BUCKET_LABELS[index],
                samples.len()
            );
        }
        println!(
            "  crossover: {}",
            crossover.map_or("none observed", |index| BUCKET_LABELS[index])
        );
    }
}
