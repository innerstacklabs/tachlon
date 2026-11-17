use app::{Loaded, load_bytes};
use import::ProjectSelection;

/// Open every project the file holds, as the page does today.
fn load_all(bytes: &[u8]) -> Result<Loaded, String> {
    load_bytes(bytes, &ProjectSelection::All, &|| 0.0).map_err(|error| error.to_string())
}

const SUMMARY: &str = include_str!("../../import/tests/fixtures/summary.xml");

#[test]
fn the_load_keeps_the_lag_calendar_the_file_names() {
    // The load keeps what the importer chose, so the file solves as the tool that saved it did.
    let bytes = xer::synth::write(&xer::synth::Spec::small(5));
    let read = import::parse_bytes(&bytes, &ProjectSelection::All).expect("reads");
    assert_ne!(read.schedule.lag_calendar(), engine::LagCalendar::default());
    let loaded = load_all(&bytes).expect("loads");
    assert_eq!(loaded.schedule.lag_calendar(), read.schedule.lag_calendar());
}

#[test]
fn an_elapsed_lag_solves_as_the_file_saved_it() {
    // Two elapsed days (LagFormat 8) on the link into "Third child".
    let original =
        "<LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink>\n    </Task>\n  </Tasks>";
    let patched = SUMMARY.replacen(original, "<LinkLag>9600</LinkLag><LagFormat>8</LagFormat></PredecessorLink>\n    </Task>\n  </Tasks>", 1);
    assert_ne!(patched, SUMMARY);
    let loaded = load_all(patched.as_bytes()).expect("loads");
    let mut reset = loaded.schedule.clone();
    reset
        .set_lag_calendar(engine::LagCalendar::default())
        .expect("sets");
    for index in 0..reset.dependencies().len() as u32 {
        reset
            .set_dependency_lag_calendar(engine::DepId(index), None)
            .expect("clears");
    }
    let default_solution = reset.compute().expect("solves");
    assert_ne!(loaded.solution, default_solution);
}

#[test]
fn a_dependency_on_a_summary_task_is_dropped_and_listed() {
    let original = "<Name>Second summary</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary><Duration>PT0S</Duration><DurationFormat>7</DurationFormat></Task>";
    let replacement = "<Name>Second summary</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary><Duration>PT0S</Duration><DurationFormat>7</DurationFormat><PredecessorLink><PredecessorUID>3</PredecessorUID><Type>1</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink></Task>";
    let patched = SUMMARY.replacen(original, replacement, 1);
    assert_ne!(patched, SUMMARY);

    let loaded = load_all(patched.as_bytes()).expect("the patched fixture should load");
    let unpatched = load_all(SUMMARY.as_bytes()).expect("the fixture should load");
    assert_eq!(unpatched.schedule.live_dependencies().count(), 2);
    assert_eq!(
        loaded.schedule.live_dependencies().count(),
        unpatched.schedule.live_dependencies().count()
    );
    let dropped: Vec<&String> = loaded
        .unsupported
        .iter()
        .filter(|line| line.starts_with("dependency 3 -> 4 (FS) dropped:"))
        .collect();
    assert_eq!(dropped.len(), 1);
    assert!(dropped[0].contains("task 4 is a summary task"));
    // The panel holds that one line and nothing else: no "scheduled later" note, no second line for the same link.
    assert_eq!(
        loaded.unsupported,
        ["dependency 3 -> 4 (FS) dropped: task 4 is a summary task"]
    );
    assert_eq!(loaded.meta[1].name, "First child");
    assert_eq!(
        app::meta::outline_path(&loaded.meta, 1),
        "First summary / First child"
    );
    assert_eq!(loaded.meta[2].name, "Second child");
}

#[test]
fn a_dependency_from_a_summary_task_is_dropped_and_names_the_summary() {
    let original = "<PredecessorUID>3</PredecessorUID><Type>1</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink>\n    </Task>\n  </Tasks>";
    let replacement = "<PredecessorUID>3</PredecessorUID><Type>1</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink><PredecessorLink><PredecessorUID>1</PredecessorUID><Type>1</Type><LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink>\n    </Task>\n  </Tasks>";
    let patched = SUMMARY.replacen(original, replacement, 1);
    assert_ne!(patched, SUMMARY);

    let loaded = load_all(patched.as_bytes()).expect("the patched fixture should load");
    assert_eq!(loaded.schedule.live_dependencies().count(), 2);
    let dropped: Vec<&String> = loaded
        .unsupported
        .iter()
        .filter(|line| line.starts_with("dependency 1 -> 5 (FS) dropped:"))
        .collect();
    assert_eq!(dropped.len(), 1);
    assert!(dropped[0].contains("task 1 is a summary task"));
    assert_eq!(
        loaded.unsupported,
        ["dependency 1 -> 5 (FS) dropped: task 1 is a summary task"]
    );
}

#[test]
fn the_unpatched_fixture_loads_with_its_two_dependencies_and_a_solved_schedule() {
    let loaded = load_all(SUMMARY.as_bytes()).expect("the fixture should load");
    assert_eq!(loaded.schedule.dependencies().len(), 2);
    assert_eq!(loaded.solution.dates.len(), 5);
    assert_eq!(loaded.order.len(), 5);
    assert!(loaded.outline.is_summary(engine::TaskId(0)));
    assert!(!loaded.outline.is_summary(engine::TaskId(1)));
}

#[test]
fn malformed_xml_is_an_error_message_not_a_panic() {
    for xml in [
        "<Project><Tasks>",
        "not xml at all",
        "<Project xmlns=\"http://schemas.microsoft.com/project\"></Project>",
    ] {
        let Err(message) = load_all(xml.as_bytes()) else {
            panic!("malformed or empty XML should be rejected");
        };
        assert_ne!(message, "");
    }
}

#[test]
fn a_file_with_no_tasks_is_an_error_message() {
    let start = SUMMARY.find("<Tasks>").expect("the fixture has tasks") + "<Tasks>".len();
    let end = SUMMARY.find("</Tasks>").expect("the fixture has tasks");
    let empty = format!("{}{}", &SUMMARY[..start], &SUMMARY[end..]);
    let Err(message) = load_all(empty.as_bytes()) else {
        panic!("a file without tasks should be rejected");
    };
    assert_eq!(message, "the file has no tasks");
}

#[test]
fn a_name_with_the_path_separator_is_kept_byte_for_byte() {
    let patched = SUMMARY.replacen("<Name>First child</Name>", "<Name>A / B</Name>", 1);
    assert_ne!(patched, SUMMARY);

    let loaded = load_all(patched.as_bytes()).expect("the patched fixture should load");
    assert_eq!(loaded.meta[1].name, "A / B");
    assert_eq!(
        app::meta::outline_path(&loaded.meta, 1),
        "First summary / A / B"
    );
}
