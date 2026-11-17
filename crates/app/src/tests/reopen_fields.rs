//! `Session::reopen` decides every `Session` field (gs#141).
//!
//! The census below destructures the reopened session with no `..`, so a field added to
//! `Session` fails to compile (E0027) until it is named here and put in one of the blocks:
//! carried from the old session, taken from `reopen`'s arguments, or reset to what a new session
//! has. The public side of the same contract is in `tests/reopen.rs`.

use super::*;
use crate::dates::Origin;
use crate::edit::Edit;
use xer::synth::{Spec, write};

fn other_clock() -> f64 {
    1.0
}

fn loaded(spec: &Spec) -> Loaded {
    load_bytes(&write(spec), &import::ProjectSelection::All, &|| 0.0)
        .expect("a generated XER loads")
}

/// What `reopen` is meant to build from `next`, made by hand: `Session::new`, the data date, the
/// origin and the outline, on a timeline with the old canvas.
fn baseline(
    next: Loaded,
    old: &Timeline,
    date: Option<engine::Instant>,
    origin: Origin,
) -> Session {
    let (width, height, dpr) = (old.viewport.width, old.viewport.height, old.dpr);
    let mut timeline = fitted_timeline(next.order, &next.solution, width, height, dpr);
    timeline.viewport.set_metrics(old.viewport.metrics());
    Session::new(
        next.schedule,
        next.solution,
        timeline,
        next.meta,
        test_clock,
    )
    .with_data_date(date)
    .with_origin(origin)
    .with_outline(next.outline)
    .expect("the outline matches its schedule")
}

/// A session with every field off its default, each one asserted so a reset check cannot pass
/// vacuously.
#[expect(
    clippy::too_many_lines,
    reason = "one session with every field set away from its default"
)]
fn busy_session() -> Session {
    let first = loaded(&Spec::with_activities(1, 40));
    let timeline = fitted_timeline(first.order, &first.solution, 900.0, 150.0, 2.0);
    let mut old = Session::new(
        first.schedule,
        first.solution,
        timeline,
        first.meta,
        other_clock,
    )
    .with_data_date(Some(999))
    .with_origin(Origin::from_day(20_458 + 28))
    .with_outline(first.outline)
    .expect("the outline matches its schedule")
    .with_history_cap(7);
    old.timeline.viewport.row_height = 30.0;
    old.timeline.viewport.axis_height = 44.0;
    old.timeline.viewport.bar_ratio = 0.7;
    old.timeline.viewport.bar_radius = 5.0;
    old.set_theme(Theme::dark());
    let leaves: Vec<u32> = old
        .timeline()
        .order()
        .iter()
        .copied()
        // An activity with actuals is fixed, so only unstarted ones are lengthened (four,
        // so that the undo has something to move).
        .filter(|&task| {
            !old.outline().unwrap().is_summary(TaskId(task))
                && old.schedule().tasks()[task as usize].actual_start.is_none()
        })
        .take(4)
        .collect();
    for task in leaves {
        let minutes = old.schedule().tasks()[task as usize].duration + 480;
        let edit = Edit::SetDuration {
            task: TaskId(task),
            minutes,
        };
        old.edit(edit).expect("a longer duration applies");
    }
    assert_eq!(old.undo(), Ok(true));
    old.edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    // The undo's report slides: the first frame starts it, so the session holds a slide to drop.
    old.advance_motion();
    assert!(old.reports.motion.slide.is_some() && old.reports.motion.seen > 0);
    old.reports.motion.copy_ms = Some(0.5);
    assert!(old.collapse_to_level(1));
    assert!(old.set_critical_only(true));
    assert!(old.set_lookahead(0, 4));
    let task = old.timeline().order()[0];
    old.select(task);
    assert!(old.extend_selection(1).is_some());
    old.timeline.viewport.scroll_y = 30.0;
    // No behaviour reaches these without a bar press, so the test writes them.
    old.shading = Some(engine::CalendarId(99));
    old.moved.push(engine::Moved {
        task: TaskId(0),
        was_es: 0,
        was_ef: 0,
    });
    old.rows_stale = true;
    old.selection_stale = true;
    old.applied = Some((1, 2));
    old.finished_press = Some((0, (1.0, 2.0)));
    old.press = Some(Press {
        at: (1.0, 2.0),
        task: 0,
        before: Before::Snet(None),
        es: 0,
        revision: 0,
    });
    old.drag_started = true;
    old.links.selected = Some(engine::DepId(0));
    // Set last, because `set_reduced_motion` would drop the slide.
    old.reports.motion.reduced_motion = true;
    old.set_link_look(11.0, 5.0, 3.0, 4.0).unwrap();
    old.set_locale(r#"{"months":["a","b","c","d","e","f","g","h","i","j","k","l"],"weekdays":["a","b","c","d","e","f","g"],"weekStart":0,"texts":{"emptyName":"vide"}}"#)
        .unwrap();
    old.pointer = (3.0, 4.0);
    old.click = Some((1.0, 2.0, 3.0, Some(0)));
    old.found = vec![1, 2];
    old.proposal = Some(crate::proposal::Proposal::default());
    old.drag_info = Some(DragInfo {
        task: 0,
        wanted: 1,
        es: 0,
        held: true,
        recompute_ms: 1.0,
    });
    old
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test checks each session field after a reopen"
)]
fn reopen_decides_every_session_field() {
    let old = busy_session();
    assert!(old.history_len() != (0, 0) && old.collapsed.iter().any(|&c| c));
    assert!(
        old.range.is_some()
            && old.selection.is_some()
            && old.chain_builds > 0
            && old.revision > 0
            && old.stamp > 1
    );
    assert!(old.row_filter != RowFilter::Off && old.lookahead.is_some());
    assert!(old.timeline().viewport.scroll_y > 0.0);

    let next_date = Some(1_234_567);
    let next_origin = Origin::from_day(20_458 + 7);
    assert!(next_origin != old.origin && next_date != old.data_date);
    let next = loaded(&Spec::with_activities(2, 90));
    let next_meta = next.meta.clone();
    let base = baseline(
        loaded(&Spec::with_activities(2, 90)),
        old.timeline(),
        next_date,
        next_origin,
    );
    let fresh = old
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(next.outline),
            next_date,
            next_origin,
        )
        .expect("the outline matches its schedule");
    assert!(
        old.shading != base.shading,
        "the old shading is not the recomputed one"
    );

    // No `..`: a new field must be classified here.
    let Session {
        schedule,
        solution,
        timeline,
        theme,
        shading,
        data_date,
        origin,
        outline,
        moved,
        reports,
        rollup_scratch: _, // scratch, cleared before every rollup
        collapsed,
        row_filter,
        lookahead,
        group,
        rows_stale,
        kept_leaves,
        path_line,
        links,
        revision,
        stamp,
        drag_info,
        chain_builds,
        reveal: _, // the reveal of the old selection; a new file starts with none
        selection,
        selection_stale,
        range,
        applied,
        press,
        finished_press,
        drag_started,
        pointer,
        clock,
        click,
        proposal,
        history: _, // read through `history_len` below; `History`'s fields are private
        meta,
        found,
        progress_rollup: _, // a memo; `new` starts it empty, so a reopen carries none
        predecessors: _,    // a memo, as `progress_rollup` is
        baseline: carried_baseline,
        deadlines,
        resources,
        typed_percent,
        host,
        fit_pending,
        names,
        texts: _, // read through `say` below
    } = &fresh;

    // DROPPED: the percents typed into the old file belong to its tasks.
    assert!(typed_percent.is_empty());

    // DROPPED: the commits made in the old file are not the new file's.
    assert!(old.host.outbox.seq > 0 && host.outbox.seq == 0);

    // DROPPED: the old file's deadlines; the page installs the new file's with `with_deadlines`.
    assert_eq!(*deadlines, Vec::new());

    // DROPPED: the old file's resource names; the page installs the new file's with `with_resources`.
    assert_eq!(*resources, Vec::<Vec<crate::Assigned>>::new());

    // DERIVED: a new file opens with its opening fit still pending (a width change refits it, gs#821).
    assert!(*fit_pending);

    // CLEARED: the old file's report. CARRIED: its counter, so a page cannot take a new report
    // for one it has shown.
    assert!(old.reports.report.is_some() && reports.report.is_none());
    assert_eq!(reports.seq, old.reports.seq);

    // The slide is the old file's and is DROPPED, with the last seq seen and its copy time; reduced
    // motion is the page's setting, so it is CARRIED.
    assert!(old.reports.motion.slide.is_some() && reports.motion.slide.is_none());
    assert!(old.reports.motion.seen > 0 && reports.motion.seen == 0);
    assert!(old.reports.motion.copy_ms.is_some() && reports.motion.copy_ms.is_none());
    assert!(old.reports.motion.reduced_motion && reports.motion.reduced_motion);
    assert!(std::ptr::eq(
        fresh.display_dates().as_ptr(),
        fresh.solution().dates.as_ptr()
    ));

    // CARRIED: the host's month names, like the colours.
    assert_eq!(*names, old.names);
    assert_ne!(*names, timeline::names::Names::default());

    // CARRIED: the host's refusal sentences.
    assert_eq!(fresh.say(&crate::edit::EditError::EmptyName), "vide");

    // CARRIED from the old session: the canvas, the colours and the clock.
    assert_eq!(timeline.viewport.width, old.timeline().viewport.width);
    assert_eq!(timeline.viewport.height, old.timeline().viewport.height);
    assert_eq!(timeline.dpr, old.timeline().dpr);
    assert_eq!(timeline.viewport.row_height, 30.0);
    assert_eq!(timeline.viewport.axis_height, 44.0);
    assert_eq!(timeline.viewport.bar_ratio, 0.7);
    assert_eq!(timeline.viewport.bar_radius, 5.0);
    assert_eq!(*theme, old.theme);
    assert!(
        std::ptr::fn_addr_eq(*clock, old.clock)
            && !std::ptr::fn_addr_eq(*clock, test_clock as fn() -> f64)
    );

    // FROM ARGS: the new file's schedule, solution, display data, outline, data date and origin.
    assert_eq!(schedule.tasks().len(), base.schedule.tasks().len());
    assert_eq!(solution.dates, base.solution.dates);
    assert_eq!(meta, &next_meta);
    assert_eq!(
        outline.as_ref().map(Outline::preorder),
        base.outline.as_ref().map(Outline::preorder)
    );
    assert_eq!(*data_date, next_date);
    assert_eq!(*origin, next_origin);

    // RECOMPUTED from the new schedule and rows, or DERIVED.
    assert_eq!(*shading, shading_calendar(schedule));
    assert_eq!(links.index.len(), base.links.index.len());
    assert!(links.selected.is_none(), "no link stays selected");
    assert_eq!(
        links.look, old.links.look,
        "the link look is the page's, like the colours"
    );
    assert_ne!(
        links.look, base.links.look,
        "and the old session had set it"
    );
    assert_eq!(
        *stamp,
        base.stamp.wrapping_add(old.stamp).wrapping_add(1),
        "past the old stamp"
    );
    assert_eq!(
        timeline.viewport, base.timeline.viewport,
        "fitted to the new schedule"
    );
    assert_eq!(timeline.order(), base.timeline.order());
    assert_eq!(timeline.summaries(), base.timeline.summaries());

    // RESET to what a new session has.
    assert!(timeline.bar_drag().is_none());
    assert_eq!(moved.as_slice(), []);
    assert_eq!(collapsed, &base.collapsed);
    assert_eq!(*row_filter, RowFilter::Off);
    assert!(lookahead.is_none() && !rows_stale && kept_leaves.is_none() && path_line.is_none());
    assert!(group.is_none());
    assert_eq!(*revision, 0);
    assert!(drag_info.is_none());
    assert_eq!(*chain_builds, 0);
    assert!(selection.is_none() && !selection_stale && range.is_none());
    assert!(applied.is_none() && press.is_none() && finished_press.is_none() && click.is_none());
    assert!(!drag_started);
    assert_eq!(*pointer, (0.0, 0.0));
    // `with_history_cap` is lost on purpose: the cap is `HISTORY_CAP` again.
    assert_eq!(fresh.history_len(), (0, 0));
    assert_eq!(found.as_slice(), [] as [u32; 0]);
    // A proposal is the old file's tasks, so the new file starts without one.
    assert!(old.proposal.is_some() && proposal.is_none());
    // A baseline is a snapshot of the old schedule's tasks, so the new file starts without one.
    assert!(old.baseline.is_some() && carried_baseline.is_none());
}
