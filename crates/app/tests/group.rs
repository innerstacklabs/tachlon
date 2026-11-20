//! Rows grouped by an activity code (gs#703). The expected rows come from a walk of a copy of
//! the tasks' codes that the test keeps and of the outline's leaves; nothing here asks
//! `group_by_code` which rows a group holds.

use crate::common;

use app::{
    RenderFacts, Session,
    edit::EditError,
    group::{Group, default_code_type, group_by_code},
    outline::{Outline, demo_layout},
};
use common::clock;
use common::collapse::N;
use engine::TaskId;
use import::ActivityCode;
use std::collections::BTreeMap;
use timeline::{HIDDEN_ROW, Timeline};

fn code(code_type: &str, value: &str) -> ActivityCode {
    ActivityCode {
        code_type: code_type.to_owned(),
        value: value.to_owned(),
    }
}

/// Task `i`'s codes: a Phase (`Civil` or `MEP`) on all but every seventh task, summaries too,
/// and a Zone, one of whose values is also spelled `Civil`, on every third.
/// Every fifth task also has the other Phase value, after its first.
fn codes_of(i: usize) -> Vec<ActivityCode> {
    let phase = (!i.is_multiple_of(7)).then(|| code("Phase", ["Civil", "MEP"][i % 2]));
    let other = i
        .is_multiple_of(5)
        .then(|| code("Phase", ["MEP", "Civil"][i % 2]));
    let zone = i
        .is_multiple_of(3)
        .then(|| code("Zone", ["North", "Civil"][i % 2]));
    phase.into_iter().chain(other).chain(zone).collect()
}

fn coded_session() -> (Session, Vec<Vec<ActivityCode>>) {
    let mut layout = demo_layout(N);
    let codes: Vec<_> = (0..layout.meta.len()).map(codes_of).collect();
    for (meta, own) in layout.meta.iter_mut().zip(&codes) {
        meta.activity_codes.clone_from(own);
    }
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let (schedule, solution, meta) = (layout.schedule, layout.solution, layout.meta);
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let session = Session::new(schedule, solution, timeline, meta, clock);
    let session = session.with_outline(outline).expect("a matching outline");
    (session, codes)
}

/// Each leaf under its first value of `code_type`, the values in order (plain words here, so
/// by bytes), the leaves with no value last; no summaries.
fn oracle(s: &Session, codes: &[Vec<ActivityCode>], code_type: &str) -> Vec<Group> {
    let outline = s.outline().expect("an outline");
    let mut groups: BTreeMap<_, Vec<TaskId>> = BTreeMap::new();
    for &t in outline
        .preorder()
        .iter()
        .filter(|&&t| !outline.is_summary(t))
    {
        let first = codes[t.0 as usize]
            .iter()
            .find(|c| c.code_type == code_type);
        let value = first.map(|c| c.value.clone());
        groups.entry((value.is_none(), value)).or_default().push(t);
    }
    let group = |((_, value), rows)| Group { value, rows };
    groups.into_iter().map(group).collect()
}

fn groups(s: &Session, code_type: &str, kept: Option<&[u32]>) -> Vec<Group> {
    group_by_code(s.outline().expect("an outline"), s.meta(), code_type, kept)
}

/// The table rows, the canvas rows, the task-to-row table and the frame drawn.
fn rows_now(s: &Session) -> (String, Vec<u32>, Vec<u32>, timeline::Scene) {
    let timeline = s.timeline();
    let json = s.table_rows_json(0, u32::MAX);
    (
        json,
        timeline.rows().to_vec(),
        timeline.row_of().to_vec(),
        frame(s),
    )
}

#[test]
fn with_no_grouping_the_rows_and_frame_are_todays() {
    let (mut session, _) = coded_session();
    assert!(session.collapse_to_level(2));
    let (today, top) = (rows_now(&session), session.timeline().viewport.scroll_y);
    assert_eq!(today.1, session.timeline().order(), "[ungrouped-view]");
    assert!(!today.3.bar_rects.is_empty() && !today.3.bar_labels.is_empty());
    assert!(session.set_group_by(Some("Phase")));
    assert_ne!(rows_now(&session), today);
    assert!(session.set_group_by(None));
    // Ungrouping keeps the top task in place, as clearing a filter does: scroll back to compare.
    session.scroll_by(top - session.timeline().viewport.scroll_y);
    assert_eq!(rows_now(&session), today, "[ungrouped-today]");
}

#[test]
fn a_leaf_sits_under_its_first_value_after_the_filters_and_no_value_comes_last() {
    let (session, codes) = coded_session();
    let found = groups(&session, "Phase", None);
    assert_eq!(found, oracle(&session, &codes, "Phase"), "[group-rows]");
    let values: Vec<_> = found.iter().map(|g| g.value.as_deref()).collect();
    assert_eq!(values, [Some("Civil"), Some("MEP"), None]);

    // The filters first: keep the Civil leaves only and the other headers are gone.
    let mut kept = vec![0];
    for t in session.outline().expect("an outline").preorder() {
        kept.push(kept[kept.len() - 1] + u32::from(found[0].rows.contains(t)));
    }
    let filtered = groups(&session, "Phase", Some(&kept));
    assert_eq!(filtered, found[..1], "[group-filter-first]");

    // The default key is the first type; Zone also has a value `Civil`, told apart by its type.
    let types = session.activity_codes();
    assert_eq!(default_code_type(&types), Some("Phase"), "[default-key]");
    assert_eq!(default_code_type(&[]), None);
    let found = groups(&session, "Zone", None);
    assert_eq!(found, oracle(&session, &codes, "Zone"), "[zone-rows]");
    let unknown = groups(&session, "Nowhere", None);
    assert!(unknown.len() == 1 && unknown[0].value.is_none());
}

fn grouped() -> (Session, Vec<Group>) {
    let (mut session, codes) = coded_session();
    assert!(session.set_group_by(Some("Phase")));
    let expected = oracle(&session, &codes, session.group_by().expect("grouped"));
    (session, expected)
}

#[test]
fn grouped_rows_are_each_header_then_its_leaves() {
    let (session, groups) = grouped();
    let leaves = |g: &Group| g.rows.iter().map(|t| t.0).collect::<Vec<_>>();
    let header_then = |g: &Group| [vec![HIDDEN_ROW], leaves(g)].concat();
    let rows: Vec<u32> = groups.iter().flat_map(header_then).collect();
    assert_eq!(session.timeline().rows(), rows, "[grouped-rows]");
    let order: Vec<u32> = groups.iter().flat_map(leaves).collect();
    assert_eq!(session.timeline().order(), order, "[grouped-order]");
}

#[test]
fn a_header_is_kind_3_with_its_label_and_leaf_count() {
    let (session, groups) = grouped();
    let rows: Vec<serde_json::Value> = serde_json::from_str(&rows_now(&session).0).expect("rows");
    let mut row = 0;
    for g in &groups {
        let (label, n) = (
            serde_json::to_string(&g.value).expect("a label"),
            g.rows.len(),
        );
        let header = format!("[null,{label},{n},null,false,3,null,null,null,null]");
        assert_eq!(rows[row].to_string(), header, "[header-kind-3]");
        row += n + 1;
    }
    assert_eq!(row, rows.len());
}

#[test]
fn a_collapsed_group_hides_its_leaves_and_keeps_its_header_and_count() {
    let (mut s, groups) = grouped();
    let (before, value) = (s.timeline().rows().to_vec(), groups[1].value.as_deref());
    assert!(!s.toggle_group(Some("Nowhere")), "[group-unknown]");
    assert!(s.toggle_group(value));
    let rows = |g: &Group| {
        let shown = g.rows.iter().filter(|_| g.value.as_deref() != value);
        [vec![HIDDEN_ROW], shown.map(|t| t.0).collect()].concat()
    };
    let expected: Vec<u32> = groups.iter().flat_map(rows).collect();
    assert_eq!(s.timeline().rows(), expected, "[group-collapsed-rows]");
    let row = groups[0].rows.len() + 1;
    let (label, n) = (serde_json::to_string(&value), groups[1].rows.len());
    let json = |s: &Session| s.table_rows_json(row as u32, row as u32 + 1);
    let header = |kind: u32| {
        format!(
            "[[null,{},{n},null,false,{kind},null,null,null,null]]",
            label.as_ref().expect("a label")
        )
    };
    assert_eq!(json(&s), header(4), "[group-collapsed-kind-4]");
    assert!(s.toggle_group(value));
    assert_eq!(s.timeline().rows(), before, "[group-reopened]");
    assert_eq!(json(&s), header(3));
}

#[test]
fn ungrouping_forgets_group_collapse_and_keeps_the_outlines() {
    let (mut s, _) = coded_session();
    assert!(s.collapse_to_level(2));
    let flags = |s: &Session| {
        let outline = s.outline().expect("an outline");
        let all = outline.preorder().iter();
        all.map(|&t| s.is_collapsed(t)).collect::<Vec<_>>()
    };
    let (today, kept) = (s.timeline().rows().to_vec(), flags(&s));
    assert!(s.set_group_by(Some("Phase")));
    let grouped = s.timeline().rows().to_vec();
    assert!(s.toggle_group(Some("Civil")));
    assert_ne!(s.timeline().rows(), grouped);
    assert_eq!(flags(&s), kept, "[group-outline-collapse]");
    assert!(s.set_group_by(None));
    assert!(!s.toggle_group(Some("Civil")), "[group-ungrouped]");
    assert_eq!((s.timeline().rows(), flags(&s)), (today.as_slice(), kept));
    assert!(s.set_group_by(Some("Phase")));
    assert_eq!(s.timeline().rows(), grouped, "[group-forgotten]");
}

#[test]
fn the_canvas_finds_each_task_on_its_row_below_the_headers() {
    let (session, _) = grouped();
    let timeline = session.timeline();
    let rows = timeline.rows().iter().enumerate();
    for (row, &task) in rows.filter(|(_, task)| **task != HIDDEN_ROW) {
        assert_eq!(timeline.row_of_task(task), Some(row), "[canvas-row]");
    }
}

/// Grouped, `edit` on a selected leaf is refused with `error`; ungrouped, it is not that refusal.
fn refused_while_grouped(edit: fn(&mut Session) -> Result<TaskId, EditError>, error: EditError) {
    let (mut session, groups) = grouped();
    session.select(groups[0].rows[1].0);
    assert_eq!(edit(&mut session), Err(error.clone()), "[grouped-refusal]");
    assert!(session.set_group_by(None));
    assert_ne!(edit(&mut session), Err(error));
}

macro_rules! refusals {
    ($($name:ident: $edit:path => $error:ident;)*) => {$(
        #[test]
        fn $name() {
            refused_while_grouped($edit, EditError::$error);
        }
    )*};
}

refusals! {
    insert_is_refused_while_grouped: Session::insert_below_selection => InsertGrouped;
    indent_is_refused_while_grouped: Session::indent_selected => ReparentGrouped;
    outdent_is_refused_while_grouped: Session::outdent_selected => ReparentGrouped;
    move_is_refused_while_grouped: Session::move_selected_up => ReparentGrouped;
}

/// A frame, bars and names, with every name a fixed width.
fn frame(s: &Session) -> timeline::Scene {
    s.frame_scene(|_, _, _| 40.0)
}

/// Red with any one header skip removed: the bars' and names' in the scene, the hit test's.
#[test]
fn a_grouped_frame_draws_and_hit_tests_past_its_headers() {
    let (session, _) = grouped();
    let timeline = session.timeline();
    let v = &timeline.viewport;
    assert_eq!(timeline.rows().first(), Some(&HIDDEN_ROW));
    assert!(!frame(&session).bar_labels.is_empty(), "[grouped-frame]");
    for r in v.visible_rows() {
        let hit = timeline.hit_test(session.display_dates(), v.width / 2.0, v.row_top(r) + 1.0);
        let header = timeline.rows()[r] == HIDDEN_ROW;
        assert!(!header || hit.is_none(), "[grouped-hit]");
    }
}

/// The last leaf of the first group and the first leaf of the second, a header between them.
fn across(groups: &[Group]) -> (u32, u32) {
    let last = groups[0].rows.len() - 1;
    (groups[0].rows[last].0, groups[1].rows[0].0)
}

fn status(s: &Session) -> serde_json::Value {
    serde_json::from_str(&s.status_json(&RenderFacts::default())).expect("status is JSON")
}

/// One test per reader over the grouped session, each red with its mapping removed.
macro_rules! readers {
    ($($name:ident: |$s:ident, $g:ident| $body:block)*) => {$(
        #[test]
        fn $name() {
            let ($s, $g) = &mut grouped();
            $body
        }
    )*};
}

readers! {
    arrows_step_over_a_header: |s, g| {
        let (last, next) = across(g);
        s.select(last);
        assert_eq!(s.move_selection(1), Some(next), "[grouped-arrow-down]");
        assert_eq!(s.move_selection(-1), Some(last), "[grouped-arrow-up]");
    }
    alt_left_and_right_close_and_open_the_group_of_the_selected_leaf: |s, g| {
        let rows = s.timeline().rows().len();
        s.select(g[1].rows[0].0);
        assert!(s.collapse_selected(), "[grouped-alt-left]");
        assert_eq!(s.timeline().rows().len(), rows - g[1].rows.len());
        assert!(!s.collapse_selected(), "a closed group is not closed twice");
        assert!(s.expand_selected(), "[grouped-alt-right]");
        assert_eq!(s.timeline().rows().len(), rows);
        // The task is under a closed header: the cursor stands on that header, so Down lands past it.
        assert!(s.collapse_selected());
        assert_eq!(s.move_selection(1), Some(g[2].rows[0].0), "[grouped-closed-cursor]");
    }
    extending_a_range_steps_over_a_header: |s, g| {
        let (last, next) = across(g);
        s.select(last);
        assert_eq!(s.extend_selection(1), Some(next), "[grouped-extend]");
    }
    a_range_to_the_last_row_holds_every_leaf_and_no_header: |s, g| {
        assert!(s.select_all_rows());
        let leaves: Vec<u32> = g.iter().flat_map(|g| g.rows.iter().map(|t| t.0)).collect();
        assert_eq!(s.range_tasks(), leaves, "[grouped-range-bottom]");
    }
    the_critical_jump_lands_on_a_critical_task: |s, _g| {
        for _ in 0..3 {
            let row = s.next_critical().expect("a critical task") as usize;
            let task = s.timeline().task_at_row(row).expect("a task row");
            assert!(s.solution().dates[task as usize].is_critical(), "[grouped-jump]");
        }
    }
    the_status_row_count_is_the_rows_on_screen: |s, _g| {
        assert_eq!(status(s)["rowCount"], s.timeline().rows().len(), "[grouped-row-count]");
    }
    a_resize_that_shows_every_row_fits_over_the_leaves: |s, _g| {
        s.resize(1_200.0, 1e6, 1.0);
        assert_eq!(s.timeline().viewport.visible_rows().len(), s.timeline().rows().len());
    }
    the_link_terminal_is_on_the_hovered_row_and_a_press_on_it_links: |s, g| {
        let task = g[1].rows[0].0;
        let row = s.timeline().row_of_task(task).expect("a row");
        let es = s.solution().dates[task as usize].es as f64;
        s.fit(es, es + 10_000.0);
        s.scroll_to_row(row);
        s.set_hover_row(Some(row));
        let mut scene = s.scene();
        let before = scene.rects.len();
        s.add_link_overlay(&mut scene);
        let dot = scene.rects[before..].iter().find(|r| r.radius > 0.0).expect("a terminal");
        let [x, y] = [dot.pos[0] + dot.size[0] / 2.0, dot.pos[1] + dot.size[1] / 2.0].map(f64::from);
        let v = &s.timeline().viewport;
        assert!((v.row_top(row)..v.row_top(row + 1)).contains(&y), "[grouped-terminal]");
        s.pointer_down(x, y, false);
        s.pointer_move(x + 60.0, y);
        assert!(s.linking(), "[grouped-link-press]");
    }
    the_offscreen_note_reads_the_last_rows: |s, _g| {
        s.scroll_by(1e9);
        common::zoom_to(s, 100.0, 1e8);
        assert!(s.offscreen().is_some(), "[grouped-offscreen]");
    }
    a_collapse_keeps_the_task_at_the_top: |s, g| {
        let top = g[1].rows[3].0;
        s.scroll_to_row(s.timeline().row_of_task(top).expect("a row"));
        s.toggle(common::collapse::summaries(s)[0]);
        let row = s.timeline().viewport.visible_rows().start;
        assert_eq!(s.timeline().task_at_row(row), Some(top), "[grouped-keep-place]");
    }
    go_to_date_scrolls_to_the_view_row: |s, _g| {
        let from = |t: u32| t != HIDDEN_ROW && s.solution().dates[t as usize].es >= 4 * engine::WEEK;
        let row = s.timeline().rows().iter().position(|&t| from(t)).expect("a bar");
        assert!(s.go_to_date("2026-02-02"));
        assert_eq!(s.timeline().viewport.visible_rows().start, row, "[grouped-go-to-date]");
    }
}

#[test]
fn deleting_a_selected_leaf_selects_the_next_row_on_screen() {
    let (_, g) = grouped();
    let last = |i: usize| *g[i].rows.last().expect("a leaf");
    let picks = [g[0].rows[0], last(0), last(1), last(g.len() - 1)];
    let old = [g[0].rows[1], g[1].rows[1], g[2].rows[1]];
    for picked in picks.into_iter().chain(old).map(|t| t.0) {
        let (mut s, _) = grouped();
        let rows = s.timeline().rows().to_vec();
        let at = rows.iter().position(|&t| t == picked).expect("a row");
        let task = |t: &&u32| **t != HIDDEN_ROW;
        let below = rows[at + 1..].iter().find(task);
        let next = below
            .or_else(|| rows[..at].iter().rev().find(task))
            .copied();
        s.select(picked);
        s.delete_selected().expect("deleted");
        let now = s.selection().map(|selected| selected.task);
        assert_eq!(now, next, "[grouped-delete] task {picked}");
    }
}

#[test]
fn ungrouped_the_readers_give_todays_rows() {
    let (mut s, _) = coded_session();
    let order = s.timeline().order().to_vec();
    s.select(order[3]);
    assert_eq!(s.move_selection(2), Some(order[5]));
    assert_eq!(s.move_selection(-9), Some(order[0]));
    assert!(s.select_all_rows());
    assert_eq!(s.range_tasks(), order);
    assert_eq!(status(&s)["rowCount"], order.len());
}
