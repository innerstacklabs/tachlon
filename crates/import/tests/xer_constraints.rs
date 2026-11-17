//! gantt#1875: a P6 activity's primary and secondary constraints reach the engine as limits.

use engine::{Limit, Pin, TaskId};
use import::{Imported, ProjectSelection, parse_xer};

// Instants from the origin Monday 2024-01-01; the default calendar works 08:00 to 17:00.
const WED_0800: i64 = 3_360;
const TUE_1700: i64 = 1_440 + 1_020;
const WED: &str = "2024-01-03 08:00";

/// Import activities given as (code, hours, the four constraint cells).
fn import(rows: &[(&str, &str, &str)]) -> Imported {
    let mut text = String::from(
        "ERMHDR\t19.12\t2024-01-01 00:00\n%T\tPROJECT\n%F\tproj_id\tproj_short_name\texport_flag\tplan_start_date\n\
         %R\t1\tSHORT\tY\t2024-01-01 08:00\n%T\tPROJWBS\n%F\twbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_short_name\twbs_name\n\
         %R\t10\t1\t\t1\tY\tROOT\tRoot\n%T\tTASK\n%F\ttask_id\tproj_id\twbs_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2\tcstr_date2\n",
    );
    for (id, (code, hours, cells)) in (1..).zip(rows) {
        let row = format!(
            "%R\t{id}\t1\t10\t{code}\t{code}\tTT_Task\tTK_NotStart\t{hours}\t{hours}\t{cells}\n"
        );
        text.push_str(&row);
    }
    text.push_str("%E\n");
    parse_xer(text.as_bytes(), &ProjectSelection::Unspecified).expect("constraints import")
}

fn index(imported: &Imported, code: &str) -> usize {
    imported
        .tasks
        .iter()
        .position(|task| task.code.as_deref() == Some(code))
        .expect("activity present")
}

fn limit(imported: &Imported, code: &str) -> Limit {
    imported.schedule.tasks()[index(imported, code)].limit
}

/// The notes about constraints; the file's other notes are not under test.
fn notes(imported: &Imported) -> Vec<&String> {
    let all = &imported.unsupported;
    all.iter()
        .filter(|line| line.contains("constraint"))
        .collect()
}

/// A limit with one field set.
fn with(set: impl Fn(&mut Limit)) -> Limit {
    let mut limit = Limit::default();
    set(&mut limit);
    limit
}

#[test]
fn a_finish_on_or_before_gives_the_activity_a_late_finish() {
    // B runs a week and sets the project finish, so A's late finish would be Friday evening.
    let mut imported = import(&[
        ("A", "8", "CS_MEOB\t2024-01-02 17:00\t\t"),
        ("B", "40", "\t\t\t"),
    ]);
    assert_eq!(limit(&imported, "A").fnlt, Some(TUE_1700));
    let id = TaskId(u32::try_from(index(&imported, "A")).expect("fits"));
    let late_finish = imported.schedule.compute().expect("computes").get(id).lf;
    assert!(late_finish <= TUE_1700, "{late_finish}");
    assert!(notes(&imported).is_empty(), "{:?}", notes(&imported));
}

#[test]
fn the_secondary_constraint_applies_beside_the_primary() {
    let imported = import(&[(
        "A",
        "8",
        &format!("CS_MSOA\t{WED}\tCS_MEOB\t2024-01-04 17:00"),
    )]);
    let task = &imported.schedule.tasks()[index(&imported, "A")];
    assert_eq!(task.snet, Some(WED_0800));
    assert_eq!(task.limit.fnlt, Some(3 * 1_440 + 1_020));
    assert!(notes(&imported).is_empty(), "{:?}", notes(&imported));
}

#[test]
fn every_constraint_type_maps_to_its_limit() {
    let cell = |kind: &str| format!("{kind}\t{WED}\t\t");
    let cases = [
        (
            "MSO",
            cell("CS_MSO"),
            with(|l| l.pin = Some(Pin::Start(WED_0800))),
        ),
        ("MSOB", cell("CS_MSOB"), with(|l| l.snlt = Some(WED_0800))),
        (
            "MEO",
            cell("CS_MEO"),
            with(|l| l.pin = Some(Pin::Finish(WED_0800))),
        ),
        ("MEOA", cell("CS_MEOA"), with(|l| l.fnet = Some(WED_0800))),
        ("ALAP", "CS_ALAP\t\t\t".to_owned(), with(|l| l.alap = true)),
        (
            "MANDS",
            cell("CS_MANDSTART"),
            with(|l| l.pin = Some(Pin::Start(WED_0800))),
        ),
        (
            "MANDF",
            cell("CS_MANDFIN"),
            with(|l| l.pin = Some(Pin::Finish(WED_0800))),
        ),
    ];
    let rows: Vec<_> = cases
        .iter()
        .map(|(code, cells, _)| (*code, "8", cells.as_str()))
        .collect();
    let imported = import(&rows);
    for (code, _, expected) in &cases {
        assert_eq!(limit(&imported, code), *expected, "{code}");
    }
    assert!(notes(&imported).is_empty(), "{:?}", notes(&imported));
}

#[test]
fn a_constraint_the_reader_cannot_apply_stays_a_note() {
    let two_pins = format!("CS_MSO\t{WED}\tCS_MANDFIN\t2024-01-04 17:00");
    let imported = import(&[
        ("X", "8", "CS_XYZ\t\t\t"),
        ("NODATE", "8", "CS_MEOB\t\t\t"),
        ("TWOPINS", "8", &two_pins),
    ]);
    let text = notes(&imported)
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    for line in [
        "Activities with the unrecognised (CS_XYZ) constraint: the constraint was not read",
        "Activities with the finish on or before constraint but no date: the constraint was not read",
        "Activities with the secondary mandatory finish constraint: the constraint was not read",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
    // The first pin holds; the one it contradicts is left out.
    assert_eq!(limit(&imported, "TWOPINS").pin, Some(Pin::Start(WED_0800)));
    assert_eq!(limit(&imported, "NODATE"), Limit::default());
}

/// The XER writer has a second slot only for `CS_MSOA` and `CS_MEOA`, so a task whose rules do not
/// fit is listed once, with the count of tasks, as the baseline and the deadline are.
#[test]
fn an_export_lists_the_limits_it_does_not_write() {
    let imported = import(&[
        (
            "A",
            "8",
            "CS_MEOB\t2024-01-02 17:00\tCS_MSOB\t2024-01-02 08:00",
        ),
        ("B", "8", "CS_ALAP\t\t\t"),
        ("C", "8", "CS_MSOA\t2024-01-03 08:00\t\t"),
        ("D", "8", "\t\t\t"),
    ]);
    let options = import::XerOptions {
        exported_at: 0,
        name: "Exported".to_owned(),
        layout: import::XerLayout::FromXer,
    };
    let left_out = import::export_xer(&imported, &options)
        .expect("export works")
        .left_out;
    assert!(
        left_out.contains(&"the constraint on 1 task was not written".to_owned()),
        "{left_out:?}"
    );
}
