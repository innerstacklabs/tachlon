//! Long links (gs#349): the index finds exactly the links a scan of every link
//! would, and tests a number of bar ends that does not grow with the links.

use super::*;
use crate::scene::links::{Link, anchors, link_in_view};
use crate::scene::long_links::LongLinks;
use crate::viewport::{AXIS_HEIGHT, ROW_HEIGHT};
use engine::{Dependency, LinkType, TaskId};

const TASKS: usize = 1500;

/// A small deterministic generator (xorshift).
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// Task i on row i, on a random day of 400, a milestone one time in five.
fn random_dates(rng: &mut Rng) -> Vec<TaskDates> {
    (0..TASKS)
        .map(|_| {
            let es = rng.below(400) as i64 * DAY;
            let ef = es
                + if rng.below(5) == 0 {
                    0
                } else {
                    rng.below(20) as i64 * DAY
                };
            TaskDates {
                es,
                ef,
                ls: es,
                lf: ef,
                float: 0,
                finished: false,
            }
        })
        .collect()
}

/// `count` random links, each on rows `0..TASKS`; the links and the
/// dependencies that make them.
fn random_links(rng: &mut Rng, count: usize) -> (Vec<Link>, Vec<Dependency>) {
    let kinds = [LinkType::FS, LinkType::SS, LinkType::FF, LinkType::SF];
    let deps: Vec<Dependency> = (0..count)
        .map(|_| Dependency {
            pred: TaskId(rng.below(TASKS) as u32),
            succ: TaskId(rng.below(TASKS) as u32),
            kind: kinds[rng.below(4)],
            lag: 0,
        })
        .collect();
    let links = deps
        .iter()
        .enumerate()
        .map(|(i, d)| Link {
            dep: i as u32,
            pred: d.pred.0,
            succ: d.succ.0,
            pred_row: d.pred.0,
            succ_row: d.succ.0,
            kind: d.kind,
        })
        .collect();
    (links, deps)
}

/// A viewport of `rows` rows from row `first`, `days` days wide from day `day`.
fn window(first: usize, rows: usize, day: i64, days: i64) -> Viewport {
    let mut v = Viewport::new(1000.0, AXIS_HEIGHT + rows as f64 * ROW_HEIGHT, TASKS);
    v.px_per_minute = 1000.0 / (days * DAY) as f64;
    v.t0 = (day * DAY) as f64;
    v.scroll_to_row(first);
    v
}

/// What a scan of every link finds: the row test, then the full path test.
fn scanned(v: &Viewport, links: &[Link], d: &[TaskDates]) -> Vec<Placed> {
    let rows = v.visible_rows();
    links
        .iter()
        .filter(|l| l.hi() >= rows.start && l.lo() < rows.end)
        .filter_map(|&link| {
            let (xa, xb) = anchors(v, &link, d);
            link_in_view(v, &rows, &link, xa, xb).then_some(Placed { link, xa, xb })
        })
        .collect()
}

fn by_dep(mut placed: Vec<Placed>) -> Vec<Placed> {
    placed.sort_by_key(|p| p.link.dep);
    placed
}

#[test]
fn visible_finds_what_a_scan_of_every_link_finds() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut compared, mut shown) = (0, 0);
    for count in [600, 3000] {
        let d = random_dates(&mut rng);
        let (links, deps) = random_links(&mut rng, count);
        let order: Vec<u32> = (0..TASKS as u32).collect();
        let index = LinkIndex::new(&deps, |_| true, &order);
        // Rows at the ends and in the middle; 17 rows, and 300 (more than a
        // long link's rows); wide and narrow in time.
        for first in [0, 3, 100, 600, 700, 1100, 1480] {
            for rows in [17, 300] {
                for (day, days) in [(0, 400), (150, 40), (-30, 10), (380, 60)] {
                    let v = window(first, rows, day, days);
                    let got = index.visible(&v, &d);
                    let long = |p: &Placed| p.link.hi() - p.link.lo() > 256;
                    // The short links come first, then the long ones by id.
                    let firsts = got.iter().take_while(|p| !long(p)).count();
                    assert!(
                        got[firsts..].iter().all(long),
                        "short link after a long one"
                    );
                    assert!(
                        got[firsts..]
                            .windows(2)
                            .all(|w| w[0].link.dep < w[1].link.dep)
                    );
                    assert_eq!(
                        by_dep(got),
                        by_dep(scanned(&v, &links, &d)),
                        "{first} {rows} {day}"
                    );
                    compared += 1;
                    shown += index.visible(&v, &d).len();
                }
            }
        }
    }
    assert_eq!(compared, 2 * 7 * 2 * 4);
    assert!(
        shown > 1000,
        "only {shown} links shown: the comparison is too easy"
    );
}

#[test]
fn long_links_are_passed_once_each() {
    let mut rng = Rng(7);
    let (links, _) = random_links(&mut rng, 3000);
    let long: Vec<Link> = links
        .into_iter()
        .filter(|l| l.hi() - l.lo() > 256)
        .collect();
    let index = LongLinks::new(long.clone());
    assert_eq!(index.len(), long.len());
    for first in [0, 300, 700, 1480] {
        let rows = first..first + 17;
        let mut seen = Vec::new();
        index.candidates(&rows, |_, _| true, |l, _| seen.push(l.dep));
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(before, seen.len(), "a link passed twice at {first}");
        // Every link that meets the rows is among them.
        for l in long
            .iter()
            .filter(|l| l.hi() >= rows.start && l.lo() < rows.end)
        {
            assert!(seen.contains(&l.dep), "link {} missing at {first}", l.dep);
        }
    }
}

#[test]
fn bar_ends_tested_do_not_grow_with_the_links() {
    let mut rng = Rng(99);
    let bound = 2 * TASKS;
    for count in [3000, 30_000, 300_000] {
        let (links, _) = random_links(&mut rng, count);
        let long: Vec<Link> = links
            .into_iter()
            .filter(|l| l.hi() - l.lo() > 256)
            .collect();
        let index = LongLinks::new(long);
        for first in [0, 5, 700, 1480] {
            let tested = std::cell::Cell::new(0);
            index.candidates(
                &(first..first + 17),
                |_, _| {
                    tested.set(tested.get() + 1);
                    false
                },
                |_, _| {},
            );
            assert!(
                tested.get() <= bound,
                "{} bar ends for {count} links",
                tested.get()
            );
        }
    }
}

/// One group of `count` links into the milestone on row `succ_row`, all
/// finish-to-start so they enter its start: one predecessor on `far_row`, the
/// rest on rows that keep them from spanning a window of rows 500..517.
fn one_big_group(
    count: usize,
    succ_row: u32,
    far_row: u32,
    rest: core::ops::Range<u32>,
) -> Vec<Link> {
    let width = rest.end - rest.start;
    (0..count)
        .map(|i| {
            let pred_row = if i == count / 2 {
                far_row
            } else {
                rest.start + (i as u32 * 7) % width
            };
            Link {
                dep: i as u32,
                pred: pred_row,
                succ: succ_row,
                pred_row,
                succ_row,
                kind: LinkType::FS,
            }
        })
        .collect()
}

#[test]
fn a_big_group_costs_the_links_that_span_not_its_size() {
    let rows = 500..517;
    // The successor is below the window with the spanning predecessor above
    // it; then the successor is above with the spanning predecessor below.
    let cases = [(1000, 0, 1300..2000), (10, 1300, 0..400)];
    for (succ_row, far_row, rest) in cases {
        let mut visited = Vec::new();
        for count in [2000, 20_000, 200_000] {
            let long = one_big_group(count, succ_row, far_row, rest.clone());
            let index = LongLinks::new(long.clone());
            let mut found = Vec::new();
            let looked = index.candidates(
                &rows,
                |_, _| true,
                |l, spans| {
                    assert!(spans, "link {} is not next to the window", l.dep);
                    found.push(l.dep);
                },
            );
            let expected: Vec<u32> = long
                .iter()
                .filter(|l| l.lo() < rows.start && l.hi() >= rows.end)
                .map(|l| l.dep)
                .collect();
            assert_eq!(expected, [count as u32 / 2]);
            found.sort_unstable();
            assert_eq!(found, expected, "{count} links into row {succ_row}");
            visited.push(looked);
        }
        // The same few links are looked at however many the group holds.
        assert_eq!(visited, [1, 1, 1], "links looked at into row {succ_row}");
    }
}
