use std::fmt::Write as _;

use engine::LagCalendar;

use super::{activity_task_id, has_note, import, task};
use crate::Imported;
use crate::xer_import::ProjectSelection;

const WEEK: &str = concat!(
    "(0||CalendarData()((0||DaysOfWeek()(",
    "(0||2()((0||0(s|08:00|f|17:00)())))",
    "))))"
);

const DEFAULT_NOTE: &str = "The file does not say which calendar a link's lag follows, so the Predecessor calendar is used (the default; Oracle does not document it); dates can differ from the file's";

#[test]
fn each_stated_spelling_sets_its_lag_calendar() {
    for (value, expected) in [
        ("rcal_Successor", LagCalendar::Successor),
        ("rcal_Predecessor", LagCalendar::Predecessor),
        ("  rcal_Predecessor  ", LagCalendar::Predecessor),
        ("rcal_24Hour", LagCalendar::TwentyFourHour),
    ] {
        let imported = lag_import(&[format!("1\t{value}").as_str()]);
        assert_eq!(imported.schedule.lag_calendar(), expected, "{value}");
        assert_no_lag_note(&imported, value);
    }
    // `rcal_ProjDefault` names the calendar of the project, which a calendar-less task uses.
    let imported = lag_import(&["1\trcal_ProjDefault"]);
    let default_calendar = imported.schedule.tasks()[0].calendar;
    assert_eq!(
        imported.schedule.lag_calendar(),
        LagCalendar::Fixed(default_calendar)
    );
    assert_no_lag_note(&imported, "rcal_ProjDefault");
}

#[test]
fn an_absent_setting_takes_the_default_and_says_so_once_per_project() {
    let expected = format!("{DEFAULT_NOTE}: 1 (first: 1)");
    for (label, options) in [
        ("no SCHEDOPTIONS table", Vec::new()),
        ("another project's row only", vec!["2\trcal_Successor"]),
        ("an empty value", vec!["1\t"]),
        ("a blank project id", vec!["\trcal_Successor"]),
        ("a row cut short", vec!["1"]),
    ] {
        let imported = lag_import(&options);
        assert_eq!(
            imported.schedule.lag_calendar(),
            LagCalendar::Predecessor,
            "{label}"
        );
        assert!(has_note(&imported, &expected), "{label}");
    }
}

#[test]
fn an_unknown_spelling_is_not_guessed_at() {
    for (value, shown) in [
        ("rcal_Bogus", "rcal_Bogus"),
        ("rcal_successor", "rcal_successor"),
        ("rcal_\u{e9}", "rcal_?"),
    ] {
        let imported = lag_import(&[format!("1\t{value}").as_str()]);
        assert_eq!(
            imported.schedule.lag_calendar(),
            LagCalendar::Predecessor,
            "{value}"
        );
        assert!(
            has_note(
                &imported,
                &format!(
                    "The lag calendar {shown} is not one this import knows, so the Predecessor calendar is used (the default; Oracle does not document it); dates can differ from the file's: 1 (first: 1)"
                )
            ),
            "{value}: {:?}",
            imported.unsupported
        );
    }
}

const DIFFERS: &str = "The lag calendar differs from the first project's; the first project's is used for all, so dates can differ from the file's: 1 (first: 2)";

#[test]
fn projects_that_disagree_use_the_first_and_name_the_other() {
    let imported = two_projects(
        ["10", "10"],
        &["1\trcal_Successor", "2\trcal_Predecessor"],
        &ProjectSelection::All,
    );
    assert_eq!(imported.schedule.lag_calendar(), LagCalendar::Successor);
    assert!(has_note(&imported, DIFFERS), "{:?}", imported.unsupported);

    let agreeing = two_projects(
        ["10", "10"],
        &["1\trcal_Predecessor", "2\trcal_Predecessor"],
        &ProjectSelection::All,
    );
    assert_eq!(agreeing.schedule.lag_calendar(), LagCalendar::Predecessor);
    assert!(
        !agreeing
            .unsupported
            .iter()
            .any(|l| l.contains("differs from the first"))
    );

    // The first selected project wins whatever its id, and a selection of one never disagrees.
    let only_two = two_projects(
        ["10", "10"],
        &["1\trcal_Successor", "2\trcal_Predecessor"],
        &ProjectSelection::Only(vec![2]),
    );
    assert_eq!(only_two.schedule.lag_calendar(), LagCalendar::Predecessor);
    assert!(
        !only_two
            .unsupported
            .iter()
            .any(|l| l.contains("differs from the first"))
    );
}

#[test]
fn project_default_counts_as_agreeing_only_with_the_same_calendar() {
    let same = two_projects(
        ["10", "10"],
        &["1\trcal_ProjDefault", "2\trcal_ProjDefault"],
        &ProjectSelection::All,
    );
    assert!(matches!(
        same.schedule.lag_calendar(),
        LagCalendar::Fixed(_)
    ));
    assert!(
        !same
            .unsupported
            .iter()
            .any(|l| l.contains("differs from the first"))
    );

    let different = two_projects(
        ["10", "11"],
        &["1\trcal_ProjDefault", "2\trcal_ProjDefault"],
        &ProjectSelection::All,
    );
    assert!(matches!(
        different.schedule.lag_calendar(),
        LagCalendar::Fixed(_)
    ));
    assert!(has_note(&different, DIFFERS), "{:?}", different.unsupported);
}

#[test]
fn a_project_that_states_nothing_is_noted_and_compared_after_its_default() {
    let imported = two_projects(
        ["10", "10"],
        &["2\trcal_Predecessor"],
        &ProjectSelection::All,
    );
    assert_eq!(imported.schedule.lag_calendar(), LagCalendar::Predecessor);
    assert!(has_note(
        &imported,
        &format!("{DEFAULT_NOTE}: 1 (first: 1)")
    ));
    assert!(
        !imported
            .unsupported
            .iter()
            .any(|l| l.contains("differs from the first"))
    );
}

#[test]
fn the_stated_lag_calendar_decides_the_successor_start() {
    // A1 runs Monday 08:00-09:00 on a Monday-only calendar; A2 is on a Monday-to-Friday one.
    // A 10 h lag: the predecessor's calendar has 8 h left on Monday, so the lag ends the
    // next Monday (Jan 8) 10:00; the successor's has 8 h left on Monday, so it ends Tuesday 10:00.
    let start_of_a2 = |setting: &str| {
        let mut imported = skewed_import(setting);
        let a2 = activity_task_id(&imported, 2);
        imported.schedule.compute().expect("computes").get(a2).es
    };
    assert_eq!(start_of_a2("rcal_Predecessor"), 7 * 1_440 + 600);
    assert_eq!(start_of_a2("rcal_Successor"), 1_440 + 600);
    // Twenty-four hour: 10 elapsed hours from 09:00 is 19:00 Monday; A2's next working
    // instant is Tuesday 08:00.
    assert_eq!(start_of_a2("rcal_24Hour"), 1_440 + 480);
    // Absent: the default is Predecessor.
    assert_eq!(start_of_a2(""), 7 * 1_440 + 600);
}

fn week_of(days: &[u8]) -> String {
    let mut out = String::from("(0||CalendarData()((0||DaysOfWeek()(");
    for day in days {
        write!(out, "(0||{day}()((0||0(s|08:00|f|17:00)())))").expect("a String takes writes");
    }
    out.push_str("))))");
    out
}

fn skewed_import(setting: &str) -> Imported {
    let options = format!("1\t{setting}");
    let options: Vec<&str> = if setting.is_empty() {
        Vec::new()
    } else {
        vec![options.as_str()]
    };
    let monday = format!("11\tCA_Project\t{}", week_of(&[2]));
    let weekdays = format!("12\tCA_Project\t{}", week_of(&[2, 3, 4, 5, 6]));
    import(
        &["1\tONE\tY\t2024-01-01 08:00\t12"],
        &["10\t1\t\tY\tRoot"],
        &[task(1, 1, 10, "11", "A1"), task(2, 1, 10, "12", "A2")],
        &["2\t1\tPR_FS\t10"],
        &options,
        &[monday.as_str(), weekdays.as_str()],
        &ProjectSelection::Unspecified,
    )
    .expect("skewed calendars import")
}

fn lag_import(options: &[&str]) -> Imported {
    let calendar = format!("10\tCA_Project\t{WEEK}");
    import(
        &[
            "1\tONE\tY\t2024-01-01 08:00\t10",
            "2\tTWO\tN\t2024-01-01 08:00\t10",
        ],
        &["10\t1\t\tY\tRoot"],
        &[task(1, 1, 10, "", "A1"), task(2, 1, 10, "", "A2")],
        &["2\t1\tPR_FS\t1"],
        options,
        &[calendar.as_str()],
        &ProjectSelection::Unspecified,
    )
    .expect("lag calendar import")
}

fn two_projects(defaults: [&str; 2], options: &[&str], selection: &ProjectSelection) -> Imported {
    let one = format!("1\tONE\tY\t2024-01-01 08:00\t{}", defaults[0]);
    let two = format!("2\tTWO\tY\t2024-01-01 08:00\t{}", defaults[1]);
    let calendar_10 = format!("10\tCA_Project\t{WEEK}");
    let calendar_11 = format!("11\tCA_Project\t{WEEK}");
    import(
        &[one.as_str(), two.as_str()],
        &["10\t1\t\tY\tOne", "20\t2\t\tY\tTwo"],
        &[task(1, 1, 10, "", "A1"), task(2, 2, 20, "", "B1")],
        &[],
        options,
        &[calendar_10.as_str(), calendar_11.as_str()],
        selection,
    )
    .expect("two projects import")
}

fn assert_no_lag_note(imported: &Imported, value: &str) {
    assert!(
        !imported
            .unsupported
            .iter()
            .any(|line| line.starts_with("lag calendar")),
        "{value}: {:?}",
        imported.unsupported
    );
}
