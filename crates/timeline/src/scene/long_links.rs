//! The links of more than `LONG_ROWS` rows, found without testing every one.
//!
//! A link can be in view only if it meets the visible rows, and then one of
//! three things holds, which rows alone tell apart:
//!
//! * its predecessor is on a visible row or next to one (the stub and the
//!   boundary run), found from the links bucketed by the predecessor's row;
//! * its successor is on a visible row (the entry), found from the links
//!   bucketed by the successor's row;
//! * both ends are off the window, one above and one below it (the link spans
//!   the window), and only its vertical run can show, at the x where it enters
//!   its successor. That x is a property of the successor's bar, so it depends
//!   on this frame's dates and is not stored.
//!
//! Of the spanning links, how many there are is known from the rows. When few,
//! a tree over the links sorted by upper row, which holds the greatest lower
//! row below each node, finds just those. When many (on the stress fixture most
//! long links span the window), the links that enter the same bar at the same
//! end are a group, tested once: by the rows of its links, then by the bar's
//! x, then only the links whose predecessor is on the far side of the window,
//! found by binary search because a group is sorted by predecessor row (all
//! its links share the successor's row). The work is then bounded by the bar
//! ends that long links enter, not by the links, however many enter the same
//! bars. The pass still tests every bar end that long links enter, visible or
//! not (gs#374).

use super::links::{LONG_ROWS, Link};
use core::ops::Range;

/// A spanning-link search by tree costs about this many times as much per
/// link as a pass over a group; the pass is chosen when the tree would find
/// more than a `1 / TREE_COST` share as many links as there are groups.
const TREE_COST: usize = 4;

/// The links that enter one bar at one end.
#[derive(Debug, Clone, Copy)]
struct Group {
    /// The successor.
    succ: u32,
    /// They enter the successor's finish, not its start.
    finish: bool,
    /// Where the group's links begin in `sorted`; the next group's start ends it.
    from: u32,
    /// The least upper row and the greatest lower row of its links: a group
    /// with `lo >= start` or `hi < end` has no link that spans the window.
    lo: u32,
    hi: u32,
}

/// Positions in `LongLinks::sorted`, bucketed by a row (a counting sort).
#[derive(Debug, Clone, Default)]
struct Buckets {
    /// `offsets[r]` is how many positions have a row below `r`.
    offsets: Vec<u32>,
    /// The positions, by row, and by position within a row.
    items: Vec<u32>,
}

impl Buckets {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an item index is below the link count, which fits u32: one link per dependency, whose id is a u32"
    )]
    fn new(rows: impl Iterator<Item = u32> + Clone, row_count: usize) -> Buckets {
        let mut offsets = vec![0u32; row_count + 1];
        for r in rows.clone() {
            offsets[r as usize + 1] += 1;
        }
        for r in 0..row_count {
            offsets[r + 1] += offsets[r];
        }
        let mut next = offsets.clone();
        let mut items = vec![0; offsets[row_count] as usize];
        for (i, r) in rows.enumerate() {
            items[next[r as usize] as usize] = i as u32;
            next[r as usize] += 1;
        }
        Buckets { offsets, items }
    }

    /// How many positions have a row below `row`.
    fn before(&self, row: u32) -> usize {
        self.offsets[(row as usize).min(self.offsets.len() - 1)] as usize
    }

    /// The positions whose row is in `from..to`.
    fn range(&self, from: u32, to: u32) -> &[u32] {
        &self.items[self.before(from)..self.before(to)]
    }
}

/// Long links, indexed by row and by successor.
#[derive(Debug, Clone, Default)]
pub(super) struct LongLinks {
    /// All of them, grouped by (successor, end entered), each group in order
    /// of the predecessor's row.
    sorted: Vec<Link>,
    /// One per run of `sorted`.
    groups: Vec<Group>,
    /// By predecessor's row, successor's row, upper row and lower row.
    by_pred: Buckets,
    by_succ: Buckets,
    by_lo: Buckets,
    by_hi: Buckets,
    /// A binary tree in heap layout over `by_lo.items`, padded to a power of
    /// two: each node holds the greatest lower row below it. A pad leaf is 0,
    /// which no window reaches (it ends after row 0).
    max_hi: Vec<u32>,
}

impl LongLinks {
    /// Index `links`, which are in order of id.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "lo and hi widen u32 row fields, and a group or link index is below the link count, which fits u32: one link per dependency, whose id is a u32"
    )]
    pub(super) fn new(links: Vec<Link>) -> LongLinks {
        // Group by (successor, end entered), and within a group by the
        // predecessor's row (the sort is stable, so ties stay in order of id).
        let mut links = links;
        links.sort_by_key(|l| (l.succ, l.enters_finish(), l.pred_row));
        let mut groups: Vec<Group> = Vec::new();
        for (i, l) in links.iter().enumerate() {
            let (lo, hi, finish) = (l.lo() as u32, l.hi() as u32, l.enters_finish());
            match groups.last_mut() {
                Some(g) if (g.succ, g.finish) == (l.succ, finish) => {
                    g.lo = g.lo.min(lo);
                    g.hi = g.hi.max(hi);
                }
                _ => groups.push(Group {
                    succ: l.succ,
                    finish,
                    from: i as u32,
                    lo,
                    hi,
                }),
            }
        }
        let rows = links.iter().map(|l| l.hi() + 1).max().unwrap_or(0);
        let by = |row: fn(&Link) -> u32| Buckets::new(links.iter().map(row), rows);
        let by_lo = by(|l| l.lo() as u32);
        let cap = links.len().next_power_of_two();
        let mut max_hi = vec![0; 2 * cap];
        for (i, &p) in by_lo.items.iter().enumerate() {
            max_hi[cap + i] = links[p as usize].hi() as u32;
        }
        for node in (1..cap).rev() {
            max_hi[node] = max_hi[2 * node].max(max_hi[2 * node + 1]);
        }
        LongLinks {
            by_pred: by(|l| l.pred_row),
            by_succ: by(|l| l.succ_row),
            by_hi: by(|l| l.hi() as u32),
            by_lo,
            max_hi,
            sorted: links,
            groups,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.sorted.len()
    }

    /// Call `f` once for each link that could be in view of the visible `rows`:
    /// `f(link, false)` when an end of it is on or next to a visible row (all
    /// of its path may show), and `f(link, true)` when it spans the rows and
    /// its run into the successor meets the canvas, which is all that can show
    /// of it. `entry_in_view(succ, finish)` says whether a run into that end of
    /// that task's bar meets the canvas, from this frame's dates. Every link in
    /// view is passed once, and others may be passed too. Returns how many
    /// links the search for spanning ones looked at, which a test bounds.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the visible rows are indexes below the row count, which fits u32: task ids are u32"
    )]
    pub(super) fn candidates(
        &self,
        rows: &Range<usize>,
        entry_in_view: impl Fn(u32, bool) -> bool,
        mut f: impl FnMut(&Link, bool),
    ) -> usize {
        if self.sorted.is_empty() || rows.is_empty() {
            return 0;
        }
        let mut visited = 0;
        let (start, end) = (rows.start as u32, rows.end as u32);
        // A predecessor on the row above the window, or the row after it, can
        // still turn onto a visible row.
        let (pred_from, pred_to) = (start.saturating_sub(1), end);
        let pred_in = |l: &Link| (pred_from..=pred_to).contains(&l.pred_row);
        for &i in self.by_pred.range(pred_from, pred_to + 1) {
            f(&self.sorted[i as usize], false);
        }
        for &i in self.by_succ.range(start, end) {
            let link = &self.sorted[i as usize];
            if !pred_in(link) {
                f(link, false);
            }
        }
        // Spanning: upper row above the window, lower row below it.
        let above = self.by_lo.before(start);
        // A long link is longer than the window unless the window is very
        // tall; then each link that ends above the window starts above it.
        let spanning = (rows.len() <= LONG_ROWS).then(|| above - self.by_hi.before(end));
        if spanning.is_some_and(|n| n * TREE_COST < self.groups.len()) {
            let mut seen = |link: &Link| {
                visited += 1;
                if !pred_in(link) && entry_in_view(link.succ, link.enters_finish()) {
                    f(link, true);
                }
            };
            self.tree(1, 0, self.max_hi.len() / 2, above, end, &mut seen);
            return visited;
        }
        for (g, group) in self.groups.iter().enumerate() {
            if group.lo >= start || group.hi < end || !entry_in_view(group.succ, group.finish) {
                continue;
            }
            let to = self
                .groups
                .get(g + 1)
                .map_or(self.sorted.len(), |n| n.from as usize);
            let members = &self.sorted[group.from as usize..to];
            // The members share a successor row, so one spans the window
            // exactly when its predecessor is on the far side of it, and the
            // members are sorted by that row. A predecessor next to the window
            // (`pred_in`) is already passed from `by_pred`.
            let succ_row = members[0].succ_row;
            let spanning = if succ_row >= end {
                &members[..members.partition_point(|l| l.pred_row < pred_from)]
            } else if succ_row < start {
                &members[members.partition_point(|l| l.pred_row <= pred_to)..]
            } else {
                &members[..0]
            };
            visited += spanning.len();
            for link in spanning {
                debug_assert!(link.lo() < rows.start && link.hi() >= rows.end && !pred_in(link));
                f(link, true);
            }
        }
        visited
    }

    /// Visit the links among the first `above` of `by_lo` whose lower row is
    /// at least `end`, under `node` (which covers `len` leaves from `first`).
    fn tree(
        &self,
        node: usize,
        first: usize,
        len: usize,
        above: usize,
        end: u32,
        f: &mut impl FnMut(&Link),
    ) {
        if first >= above || self.max_hi[node] < end {
            return;
        }
        if len == 1 {
            f(&self.sorted[self.by_lo.items[first] as usize]);
            return;
        }
        let half = len / 2;
        self.tree(2 * node, first, half, above, end, f);
        self.tree(2 * node + 1, first + half, half, above, end, f);
    }
}
