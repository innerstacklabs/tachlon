//! `Session::print_pages` (gs#703): each printed page as SVG.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{PageSize, Session, meta::TaskMeta};
use common::{clock, collapse::standard_calendar, outline_session};
use engine::{DAY, Schedule, Task, TaskId};
use timeline::Timeline;

const ROWS_PER_PAGE: usize = 25;

/// Two tasks named `names`, on a session with no outline.
fn named(names: [&str; 2]) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for _ in 0..2 {
        let task = Task {
            duration: DAY,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("calendar is the schedule's");
    }
    let solution = schedule.clone().compute().expect("computes");
    let meta = names
        .map(|name| TaskMeta {
            name: name.to_string(),
            ..TaskMeta::default()
        })
        .to_vec();
    let timeline = Timeline::new(vec![0, 1], 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
}

/// The `(x, content)` of each `<text>` in `svg`, in document order.
fn texts(svg: &str) -> Vec<(f64, String)> {
    svg.split("<text x=\"")
        .skip(1)
        .map(|rest| {
            let x = rest.split('"').next().unwrap().parse().unwrap();
            let body = rest.split_once('>').unwrap().1;
            (x, body.split("</text>").next().unwrap().to_string())
        })
        .collect()
}

/// Asserts every visible row's name is on `pages`, in row order.
fn assert_names_in_order(session: &Session, pages: &[String]) {
    let printed: Vec<String> = pages
        .iter()
        .flat_map(|page| texts(page))
        .map(|(_, body)| body)
        .collect();
    let mut at = 0;
    for &task in session.timeline().order() {
        let name = session.task_label(task).replace('&', "&amp;");
        // A name too long for its column prints cut short, ending in an ellipsis.
        let found = printed[at..].iter().position(|body| {
            let cut = body.trim_end_matches('…');
            *body == name || (body.ends_with('…') && !cut.is_empty() && name.starts_with(cut))
        });
        at += found.unwrap_or_else(|| panic!("row {task} {name:?} is on a page in order")) + 1;
    }
}

#[test]
fn the_names_on_all_pages_are_the_visible_rows_in_order() {
    let session = outline_session(60).0;
    let pages = session.print_pages("Plan", "");
    assert_eq!(pages.len(), session.print_plan().pages);
    assert!(pages.len() > 1);
    assert_names_in_order(&session, &pages);
    assert!(
        pages
            .iter()
            .all(|p| p.starts_with("<svg") && p.contains("viewBox=\"0 0 1056 816\""))
    );
}

#[test]
fn an_a4_print_has_the_a4_page_width_and_the_same_names_as_letter() {
    let session = outline_session(60).0;
    let letter = session.print_pages("Plan", "");
    let a4 = session.print_pages_on("Plan", "", PageSize::A4, None, None, None);
    assert_names_in_order(&session, &a4);
    assert!(a4.iter().all(|p| p.contains("viewBox=\"0 0 1123 794\"")));
    assert!(a4.iter().all(|p| !p.contains("1056")), "no Letter width");
    assert_eq!(
        a4.len(),
        session.print_plan_on(PageSize::A4, None, None).pages
    );
    assert_eq!(
        a4.len(),
        letter.len(),
        "A4 is as tall as Letter, so as many pages"
    );
}

#[test]
fn an_a4_page_holds_as_many_rows_as_its_height_allows() {
    let (width, height) = PageSize::A4.dims();
    assert_eq!(
        (width, height),
        (1123.0, 794.0),
        "A4 at 96 px to the inch, as Letter is"
    );
    let plan = outline_session(200)
        .0
        .print_plan_on(PageSize::A4, None, None);
    // 794 less margins 72, header 44, footer 20 and the 48 px axis leaves 610 px: 25 rows of 24.
    assert_eq!(plan.rows_per_page, ROWS_PER_PAGE);
    assert_eq!(plan.page_rows(0).len(), ROWS_PER_PAGE);
}

#[test]
fn a_page_size_is_letter_or_a4_in_any_case() {
    assert_eq!(PageSize::parse("A4"), Some(PageSize::A4));
    assert_eq!(PageSize::parse("letter"), Some(PageSize::Letter));
    assert_eq!(PageSize::parse("legal"), None);
    assert_eq!(PageSize::default(), PageSize::Letter);
}

#[test]
fn a_body_of_25_rows_prints_the_visible_rows_over_25_pages_rounded_up() {
    for n in [10, 60, 200] {
        let session = outline_session(n).0;
        let rows = session.timeline().order().len();
        let pages = session.print_pages("Title", "");
        assert_eq!(pages.len(), rows.div_ceil(ROWS_PER_PAGE), "{rows} rows");
        assert!(pages.iter().all(|page| page.starts_with("<svg")));
    }
}

#[test]
fn a_name_with_an_ampersand_and_a_bracket_is_escaped() {
    let session = named(["R&D <phase>", "Plain"]);
    let page = &session.print_pages("T", "")[0];
    assert!(page.contains(">R&amp;D &lt;phase&gt;</text>"), "{page}");
    assert!(!page.contains("R&D"));
}

#[test]
fn page_two_of_three_says_so() {
    let session = outline_session(60).0;
    let plan = session.print_plan();
    assert_eq!(plan.pages, 3, "the fixture takes three pages");
    let pages = session.print_pages("Plan", "");
    assert!(pages[1].contains(">Page 2 of 3</text>"));
    assert!(!pages[0].contains("Page 2 of 3"));
}

#[test]
fn the_header_carries_the_data_date_and_the_filter() {
    let plain = outline_session(60).0;
    let page = &plain.print_pages("My plan", "")[0];
    assert!(page.contains(">My plan</text>"));
    assert!(!page.contains("Data date:") && !page.contains("Filter:"));

    let mut session = outline_session(60).0.with_data_date(Some(8 * 60));
    assert!(session.set_critical_only(true));
    let page = &session.print_pages("My plan", "")[0];
    let expected = format!("Data date: {}", session.data_date_text().unwrap());
    assert!(page.contains(&expected), "{expected}");
    assert!(page.contains("Filter: critical"));
}

/// The attributes and `y` of the `<text>` element of `svg` whose content is `body`.
fn text_at<'a>(svg: &'a str, body: &str) -> (&'a str, f64) {
    let tag = svg
        .split("<text ")
        .skip(1)
        .find(|tag| tag.contains(&format!(">{body}</text>")))
        .unwrap_or_else(|| panic!("{body:?} is on the page"));
    let attrs = tag.split('>').next().unwrap();
    let y = attrs
        .split("y=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    (attrs, y.parse().unwrap())
}

#[test]
fn the_header_carries_the_print_date() {
    let session = outline_session(60).0;
    let pages = session.print_pages("Plan", "2026-11-06");
    assert!(pages.len() > 1);
    for (i, page) in pages.iter().enumerate() {
        let (number, number_y) = text_at(page, &format!("Page {} of {}", i + 1, pages.len()));
        let (date, date_y) = text_at(page, "Printed 2026-11-06");
        assert!(date_y > number_y, "the date is under the page number");
        assert_eq!(date.split("y=").next(), number.split("y=").next());
        assert!(date.contains("text-anchor=\"end\"") && number.contains("text-anchor=\"end\""));
    }

    let page = &session.print_pages("Plan", "")[0];
    assert!(!page.contains("Printed"), "an empty date draws nothing");
    let page = &session.print_pages("Plan", "<b>")[0];
    assert!(
        page.contains(">Printed &lt;b&gt;</text>"),
        "the date is escaped"
    );
}

#[test]
fn a_summarys_children_are_indented_one_level() {
    let session = outline_session(60).0;
    let outline = session.outline().expect("outline");
    let summary = session
        .timeline()
        .order()
        .iter()
        .copied()
        .find(|&t| outline.is_summary(TaskId(t)))
        .expect("a summary");
    let child = outline.children(TaskId(summary))[0].0;
    let x_of = |task: u32| {
        let name = session.task_label(task).replace('&', "&amp;");
        texts(&session.print_pages("T", "")[0])
            .into_iter()
            .find(|(_, body)| *body == name)
            .unwrap_or_else(|| panic!("{name:?} on page one"))
            .0
    };
    assert_eq!(x_of(child) - x_of(summary), 12.0);
}

#[test]
fn the_last_page_counts_the_rows_left_out() {
    let session = outline_session(3_000).0;
    let pages = session.print_pages("T", "");
    let left = session.print_plan().left_out;
    assert_eq!(pages.len(), 100);
    let note = format!(">{left} rows not printed. Select a range to print them.</text>");
    assert!(pages[99].contains(&note));
    assert!(!pages[98].contains("rows not printed"));
}

#[test]
fn a_selected_range_prints_only_its_rows() {
    let mut session = outline_session(3_000).0;
    let order = session.timeline().order().to_vec();
    session.select(order[0]);
    assert!(session.extend_to(order[4]));
    let pages = session.print_pages("T", "");
    assert_eq!(pages.len(), 1);
    let printed: Vec<String> = texts(&pages[0]).into_iter().map(|(_, body)| body).collect();
    for &task in &order[..5] {
        let name = session.task_label(task).replace('&', "&amp;");
        assert!(printed.contains(&name), "row {task} {name:?} is printed");
    }
    assert_eq!(session.print_plan().rows, order[..5]);
    assert!(!pages[0].contains("rows not printed"));
}

/// The number after `key` in `s`.
fn attr(s: &str, key: &str) -> f64 {
    let rest = s.split_once(key).unwrap().1;
    rest[..rest.find('"').unwrap()].parse().unwrap()
}

/// The `(x, y, clip width, text)` of each list cell on `svg`, in document order.
fn list_cells(svg: &str) -> Vec<(f64, f64, f64, String)> {
    let cells = svg.split("<clipPath id=\"pc").skip(1);
    cells
        .map(|cell| {
            let body = cell.split_once("<text ").unwrap().1;
            let body = body.split_once('>').unwrap().1.split("</text>").next();
            let (x, y, w) = (
                attr(cell, "x=\""),
                attr(cell, "y=\""),
                attr(cell, "width=\""),
            );
            (x, y, w, body.unwrap().to_string())
        })
        .collect()
}

/// Px a printed `body` needs of its clip: letters and digits at 0.6 em of the 11 px font, the rest at 4 px, and the 2 px the text is set in.
fn needs(body: &str) -> f64 {
    let wide = body.chars().filter(|c| c.is_alphanumeric()).count() as f64;
    wide * 6.6 + (body.chars().count() as f64 - wide) * 4.0 + 2.0
}

#[test]
fn a_printed_date_duration_and_float_print_whole_and_stop_short_of_the_next_cell() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    // 2d 4h 30m of 8-hour days, a one-day task, and a task 12,000 days long that gives it its float.
    for duration in [2 * 480 + 4 * 60 + 30, 480, 12_000 * 480] {
        let task = Task {
            duration,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("calendar is the schedule's");
    }
    let solution = schedule.clone().compute().expect("computes");
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    let meta = vec![TaskMeta::default(); 3];
    let session = Session::new(schedule, solution, timeline, meta, clock);
    let (long, float) = (session.task_duration(0), session.task_float(1));
    assert_eq!(long, "2d 4h 30m");
    assert!(float.len() >= 6, "{float:?} is five digits and a d");

    let cells = list_cells(&session.print_pages("Plan", "")[0]);
    let whole = |body: &str| {
        body == long || body == float || (body.len() == 10 && body.matches('-').count() == 2)
    };
    assert!(cells.iter().filter(|c| whole(&c.3)).count() >= 8);
    for (_, _, width, body) in cells.iter().filter(|c| whole(&c.3)) {
        assert!(needs(body) <= *width, "{body:?} is cut at {width} px");
    }
    for pair in cells.windows(2).filter(|p| p[0].1 == p[1].1) {
        assert!(pair[0].0 + pair[0].2 < pair[1].0, "{:?} runs on", pair[0].3);
    }
}

#[test]
fn a_name_is_cut_only_when_its_measured_width_overflows_its_column() {
    let long = "Excavate and pour the footings for the east wing";
    let session = named([long, "Short"]);
    let at = |px: f64| {
        let width = |body: &str, _: f64| body.chars().count() as f64 * px;
        list_cells(
            &session.print_pages_measured("Plan", "", PageSize::Letter, &width, None, None, None)
                [0],
        )
    };
    // Cell 5 is the first row's name: the header takes Task, Dur, Start, Finish and Float.
    assert_eq!(at(3.0)[5].3, long);
    let (_, _, clip, cut) = at(10.0)[5].clone();
    let shown = cut.chars().count() as f64 * 10.0;
    assert!(cut.ends_with('…') && shown <= clip - 2.0 && shown + 10.0 > clip - 2.0);
    // Even 10 px a letter cuts no value, whose columns the estimate sizes: the name is the one cut cell.
    let cut_cells = at(10.0).iter().filter(|c| c.3.ends_with('…')).count();
    assert_eq!(cut_cells, 1);
}

#[test]
fn a_page_with_id_and_wbs_columns_keeps_every_value_whole_with_float_on_the_page() {
    let session_of = |durations: [i64; 3]| {
        let mut schedule = Schedule::new();
        let calendar = schedule.add_calendar(standard_calendar());
        for duration in durations {
            let task = Task {
                duration,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            };
            schedule.add_task(task).expect("calendar is the schedule's");
        }
        let solution = schedule.clone().compute().expect("computes");
        let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
        let meta = (0..3)
            .map(|i| TaskMeta {
                code: Some(format!("ACT-000{i}-LONG")),
                wbs_code: Some(format!("HAND.WORK.PACKAGE.{i}")),
                ..TaskMeta::default()
            })
            .collect();
        Session::new(schedule, solution, timeline, meta, clock)
    };
    let session = session_of([2 * 480 + 4 * 60 + 30, 480, 12_000 * 480]);

    let plan = session.print_plan();
    let cells = list_cells(&session.print_pages("Plan", "")[0]);
    let head: Vec<_> = cells.iter().take_while(|c| c.1 == cells[0].1).collect();
    let names: Vec<_> = head.iter().map(|c| c.3.as_str()).collect();
    assert_eq!(
        names,
        ["ID", "WBS", "Task", "Dur", "Start", "Finish", "Float"]
    );
    for (_, _, width, body) in &cells {
        assert!(needs(body) <= *width, "{body:?} is cut at {width} px");
    }
    // The name keeps 180 px; ID shrinks and ends in an ellipsis, WBS keeps its end behind one.
    assert!(head[2].2 >= 176.0, "the name column is {} px", head[2].2);
    let (xs, top) = (head.iter().map(|c| c.0).collect::<Vec<_>>(), head[0].1);
    let body = |col: usize| {
        let x = xs[col];
        cells.iter().filter(move |c| c.0 == x && c.1 != top)
    };
    assert!(body(0).all(|c| c.3.ends_with('…')));
    assert!(body(1).all(|c| c.3.starts_with('…')));
    assert!(
        body(3)
            .chain(body(4))
            .chain(body(5))
            .chain(body(6))
            .all(|c| !c.3.ends_with('…'))
    );
    assert!(cells.iter().any(|c| c.3 == session.task_float(1)));
    let list_end = head.last().map(|c| c.0 + c.2 + 4.0).unwrap();
    assert!(
        list_end <= 1056.0 - 72.0 - plan.panel_w + 0.01,
        "Float is on the page"
    );
    // A widest duration of 120d and float of 119d print whole, not as "12…".
    let cells = list_cells(&session_of([120 * 480, 480, 30 * 480]).print_pages("Plan", "")[0]);
    let whole = |v: &str| cells.iter().any(|c| c.3 == v);
    assert!(whole("120d") && whole("119d"));
}
