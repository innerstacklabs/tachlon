//! The 50 000-task demo (`?n=50000`, the README benchmark) starts and finishes every task on a
//! whole working hour inside its calendar's working day, and no summary duration carries minutes
//! (gs#972; the fit-out sample got the same fix in gs#901).

use crate::common;

use engine::{DAY, TaskId};

const MINUTES_PER_HOUR: i64 = 60;

/// A leaf starts on a whole hour at which its calendar works, and finishes on a whole hour at the
/// close of a working hour. A milestone only has to sit on a whole hour.
#[test]
fn every_start_and_finish_of_the_50k_demo_is_a_whole_working_hour() {
    let (session, _) = common::outline_session(50_000);
    let schedule = session.schedule();
    let outline = session.outline().expect("the demo has an outline");
    let mut off_hour = Vec::new();
    let mut outside = Vec::new();
    for (id, task) in schedule.tasks().iter().enumerate() {
        if outline.is_summary(TaskId(id as u32)) {
            continue;
        }
        let dates = &session.solution().dates[id];
        let calendar = schedule.calendar_of(TaskId(id as u32));
        let (es, ef) = (dates.es, dates.ef);
        if es % MINUTES_PER_HOUR != 0 || ef % MINUTES_PER_HOUR != 0 {
            off_hour.push((id, session.date_text(es), session.date_text(ef)));
            continue;
        }
        if task.duration == 0 {
            continue;
        }
        let periods = |t: i64| &calendar.week()[t.div_euclid(DAY).rem_euclid(7) as usize];
        let minute = |t: i64| t.rem_euclid(DAY);
        let starts_inside = periods(es)
            .iter()
            .any(|&(open, close)| (open as i64..close as i64).contains(&minute(es)));
        // A finish at midnight closes the previous day.
        let finish_day = ef - 1;
        let finishes_inside = periods(finish_day).iter().any(|&(open, close)| {
            (open as i64 + 1..=close as i64).contains(&(minute(finish_day) + 1))
        });
        if !starts_inside || !finishes_inside {
            outside.push((id, session.date_text(es), session.date_text(ef)));
        }
    }
    assert!(
        off_hour.is_empty(),
        "{} tasks off the hour, first: {:?}",
        off_hour.len(),
        &off_hour[..off_hour.len().min(5)]
    );
    assert!(
        outside.is_empty(),
        "{} tasks outside the working day, first: {:?}",
        outside.len(),
        &outside[..outside.len().min(5)]
    );
}

#[test]
fn no_summary_duration_of_the_50k_demo_carries_minutes() {
    let (session, meta) = common::outline_session(50_000);
    let outline = session.outline().expect("the demo has an outline");
    let mut with_minutes = Vec::new();
    for id in 0..meta.len() as u32 {
        if !outline.is_summary(TaskId(id)) {
            continue;
        }
        let text = session.task_duration(id);
        if text.split(' ').any(|part| part.ends_with('m')) {
            with_minutes.push((meta[id as usize].name.clone(), text));
        }
    }
    assert!(
        with_minutes.is_empty(),
        "{} summaries with minutes, first: {:?}",
        with_minutes.len(),
        &with_minutes[..with_minutes.len().min(5)]
    );
}
