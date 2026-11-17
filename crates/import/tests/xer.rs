use std::fs;
use std::path::{Path, PathBuf};

use engine::{LinkType, TaskDates, TaskId};
use import::{ImportError, Imported, ProjectSelection, parse_xer};
use xer::Encoding;
use xer::synth::{LineEnding, Spec, write};

const WEEK: &str = "(0||CalendarData()((0||DaysOfWeek()((0||1()())(0||2()((0||0(s|08:00|f|17:00)())))(0||3()((0||0(s|08:00|f|17:00)())))(0||4()((0||0(s|08:00|f|17:00)())))(0||5()((0||0(s|08:00|f|17:00)())))(0||6()((0||0(s|08:00|f|17:00)())))(0||7()())))(0||Exceptions()())))";
const PROJECT_FIELDS: &str = "proj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id";
const WBS_FIELDS: &str = "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name";
const TASK_FIELDS: &str = "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2";
const PRED_FIELDS: &str = "task_id\tpred_task_id\tpred_type\tlag_hr_cnt";

fn task_id(imported: &Imported, uid: u32) -> TaskId {
    let index = imported
        .tasks
        .iter()
        .position(|task| task.uid == uid && task.code.is_some())
        .expect("activity should be imported");
    TaskId(u32::try_from(index).expect("activity position should fit in TaskId"))
}

fn table(text: &mut String, name: &str, fields: &str, rows: &[&str]) {
    text.push_str("%T\t");
    text.push_str(name);
    text.push_str("\n%F\t");
    text.push_str(fields);
    text.push('\n');
    for row in rows {
        text.push_str("%R\t");
        text.push_str(row);
        text.push('\n');
    }
}

fn handbuilt(start: &str, tasks: &[&str], links: &[&str], calendar: &str) -> Vec<u8> {
    let mut text = String::from(
        "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tProject Management\tUSD\n",
    );
    let project = format!("1\tHAND\tY\t{start}\t1");
    table(&mut text, "PROJECT", PROJECT_FIELDS, &[&project]);
    table(&mut text, "PROJWBS", WBS_FIELDS, &["10\t1\t\t1\tY\tRoot"]);
    table(&mut text, "TASK", TASK_FIELDS, tasks);
    table(&mut text, "TASKPRED", PRED_FIELDS, links);
    table(
        &mut text,
        "SCHEDOPTIONS",
        "proj_id\tsched_calendar_on_relationship_lag",
        &["1\trcal_Successor"],
    );
    let calendar = format!("1\tCA_Project\t{calendar}");
    table(
        &mut text,
        "CALENDAR",
        "clndr_id\tclndr_type\tclndr_data",
        &[&calendar],
    );
    text.push_str("%E\n");
    text.into_bytes()
}

fn regular_task(id: u32, code: &str, hours: &str) -> String {
    format!("{id}\t1\t10\t1\t{code}\tName {code}\tTT_Task\tTK_NotStart\t{hours}\t{hours}\t\t\t")
}

fn import_handbuilt(start: &str, tasks: &[&str], links: &[&str], calendar: &str) -> Imported {
    parse_xer(
        &handbuilt(start, tasks, links, calendar),
        &ProjectSelection::Unspecified,
    )
    .expect("hand-built XER should import")
}

fn dates(imported: &mut Imported, uid: u32) -> TaskDates {
    let id = task_id(imported, uid);
    *imported
        .schedule
        .compute()
        .expect("schedule should compute")
        .get(id)
}

fn activity_count(imported: &Imported) -> usize {
    imported
        .tasks
        .iter()
        .filter(|task| task.code.is_some())
        .count()
}

fn note_count(imported: &Imported, prefix: &str) -> usize {
    imported
        .unsupported
        .iter()
        .filter(|line| line.starts_with(prefix))
        .count()
}

#[test]
fn synth_small_file_maps_every_row() {
    let spec = Spec::small(7);
    let counts = spec.counts();
    let mut imported = parse_xer(&write(&spec), &ProjectSelection::Unspecified)
        .expect("synthetic XER should import");
    assert_eq!(imported.schedule.tasks().len(), counts.activities + 3);
    assert_eq!(imported.schedule.dependencies().len(), counts.links);
    assert_eq!(activity_count(&imported), counts.activities);
    for (index, task) in imported.tasks.iter().enumerate() {
        if let Some(parent) = task.parent {
            assert!((parent.0 as usize) < index);
            assert_eq!(
                task.outline_level,
                imported.tasks[parent.0 as usize].outline_level + 1
            );
        }
    }
    assert!(imported.schedule.compute().is_ok());
    let tables = imported
        .unsupported
        .iter()
        .find(|line| line.starts_with("Parts of the file that are not read"))
        .expect("unread tables should be reported");
    assert!(tables.contains("TASKRSRC"));
    assert!(
        !["ACTVTYPE", "ACTVCODE", "TASKACTV"]
            .iter()
            .any(|table| tables.contains(table)),
        "the activity code tables are read: {tables}"
    );
    assert!(!imported.unsupported.iter().any(|line| {
        line.starts_with("ragged") || line.to_ascii_lowercase().contains("short row")
    }));
}

#[test]
fn synth_two_projects() {
    let spec = Spec {
        projects: 2,
        cross_project_links: true,
        ..Spec::small(9)
    };
    let bytes = write(&spec);
    let error = parse_xer(&bytes, &ProjectSelection::Unspecified)
        .expect_err("multiple projects should require a choice");
    let ImportError::ProjectChoice { projects } = error else {
        panic!("expected ProjectChoice");
    };
    assert_eq!(projects.len(), 2);
    assert_eq!((projects[0].proj_id, projects[0].activities), (1_000, 40));
    assert_eq!((projects[1].proj_id, projects[1].activities), (1_001, 40));

    let all = parse_xer(&bytes, &ProjectSelection::All).expect("all projects should import");
    let roots: Vec<_> = all
        .tasks
        .iter()
        .filter(|task| task.parent.is_none() && task.code.is_none())
        .collect();
    assert_eq!(roots.len(), 2);
    assert!(roots.iter().all(|task| task.outline_level == 1));
    assert_eq!(all.schedule.dependencies().len(), spec.counts().links);

    let one = parse_xer(&bytes, &ProjectSelection::Only(vec![1_000]))
        .expect("first project should import");
    assert_eq!(
        note_count(
            &one,
            "Links to activities in another project were not read; dates can differ from the file's: 1"
        ),
        1
    );
    assert_eq!(
        note_count(
            &one,
            "Projects that were not chosen, or are not marked for export, were not read; the dates of the projects that were read are not affected: 1"
        ),
        1
    );
    assert_eq!(
        parse_xer(&bytes, &ProjectSelection::Only(vec![999_999])).unwrap_err(),
        ImportError::UnknownProject { proj_id: 999_999 }
    );
}

#[test]
fn synth_windows_1252_crlf() {
    let encoded = Spec {
        encoding: Encoding::Windows1252,
        line_ending: LineEnding::CrLf,
        ..Spec::small(11)
    };
    let imported = parse_xer(&write(&encoded), &ProjectSelection::Unspecified)
        .expect("Windows-1252 CRLF should import");
    assert!(
        imported
            .unsupported
            .iter()
            .any(|line| line.contains("not saved as UTF-8"))
    );
    let utf8 = parse_xer(&write(&Spec::small(11)), &ProjectSelection::Unspecified)
        .expect("UTF-8 should import");
    assert!(
        !utf8
            .unsupported
            .iter()
            .any(|line| line.contains("not saved as UTF-8"))
    );
}

#[test]
fn synth_5000_activities_computes() {
    let mut imported = parse_xer(
        &write(&Spec::with_activities(42, 5_000)),
        &ProjectSelection::Unspecified,
    )
    .expect("large synthetic XER should import");
    assert_eq!(activity_count(&imported), 5_000);
    assert!(imported.schedule.compute().is_ok());
}

#[test]
fn every_truncation_and_garbage_is_panic_free() {
    let bytes = write(&Spec::small(3));
    for step in 0..=200 {
        let end = bytes.len().saturating_mul(step) / 200;
        if let Ok(imported) = parse_xer(&bytes[..end], &ProjectSelection::Unspecified)
            && end < bytes.len()
        {
            assert!(
                imported
                    .unsupported
                    .iter()
                    .any(|line| line == "The file has no end marker, so it may stop early and some of it may not have been read; dates can differ from the file's: 1")
            );
        }
    }
    for step in 0..50 {
        let mut damaged = bytes.clone();
        let position = (step * bytes.len() / 50 + 17) % bytes.len();
        damaged[position] = 0xff;
        let _ = parse_xer(&damaged, &ProjectSelection::Unspecified);
    }
    let _ = parse_xer(&[], &ProjectSelection::Unspecified);
    let _ = parse_xer(b"ERMHDR", &ProjectSelection::Unspecified);
}

#[test]
fn handbuilt_fs_with_lag_has_exact_dates() {
    let a = regular_task(1, "A", "8");
    let b = regular_task(2, "B", "8");
    let mut imported = import_handbuilt("2024-01-01 08:00", &[&a, &b], &["2\t1\tPR_FS\t4"], WEEK);
    let first = dates(&mut imported, 1);
    assert_eq!((first.es, first.ef), (480, 960));
    assert_eq!(dates(&mut imported, 2).es, 2_100);
}

#[test]
fn handbuilt_negative_lag_has_exact_dates() {
    let a = regular_task(1, "A", "8");
    let b = regular_task(2, "B", "8");
    let mut imported = import_handbuilt("2024-01-01 08:00", &[&a, &b], &["2\t1\tPR_FS\t-4"], WEEK);
    assert_eq!(imported.schedule.dependencies()[0].lag, -240);
    assert_eq!(dates(&mut imported, 2).es, 720);
}

#[test]
fn handbuilt_every_link_type_maps() {
    let tasks: Vec<_> = (1..=5)
        .map(|id| regular_task(id, &format!("A{id}"), "1"))
        .collect();
    let rows: Vec<_> = tasks.iter().map(String::as_str).collect();
    let links = [
        "2\t1\tPR_FS\t0",
        "3\t1\tPR_SS\t0",
        "4\t1\tPR_FF\t0",
        "5\t1\tPR_SF\t0",
    ];
    let mut imported = import_handbuilt("2024-01-01 08:00", &rows, &links, WEEK);
    assert_eq!(
        imported
            .schedule
            .dependencies()
            .iter()
            .map(|link| link.kind)
            .collect::<Vec<_>>(),
        [LinkType::FS, LinkType::SS, LinkType::FF, LinkType::SF]
    );
    assert!(imported.schedule.compute().is_ok());
}

#[test]
fn handbuilt_bad_links_are_dropped_once_each() {
    let a = regular_task(1, "A", "1");
    let b = regular_task(2, "B", "1");
    let links = ["1\t1\tPR_FS\t0", "2\t999\tPR_FS\t0", "2\t1\tPR_XX\t0"];
    let mut imported = import_handbuilt("2024-01-01 08:00", &[&a, &b], &links, WEEK);
    assert_eq!(imported.schedule.dependencies(), []);
    assert_eq!(
        note_count(
            &imported,
            "Links from an activity to itself were not read; no dates are affected: 1"
        ),
        1
    );
    assert_eq!(
        note_count(
            &imported,
            "Links to an activity that is not in the file were not read; dates can differ from the file's: 1"
        ),
        1
    );
    assert_eq!(
        note_count(
            &imported,
            "Links of type PR_XX, which this import does not know, were not read; dates can differ from the file's: 1"
        ),
        1
    );
    assert!(imported.schedule.compute().is_ok());
}

#[test]
fn handbuilt_sunday_week_and_holiday_delay_finish() {
    let calendar = WEEK.replacen(
        "(0||Exceptions()())",
        "(0||Exceptions()((0||0(d|45293)())))",
        1,
    );
    let task = regular_task(1, "A", "16");
    let mut imported = import_handbuilt("2024-01-01 08:00", &[&task], &[], &calendar);
    assert_eq!(dates(&mut imported, 1).ef, 3_780);
}

#[test]
fn handbuilt_working_saturday_exception_moves_the_finish() {
    // 45297 is Saturday 2024-01-06. A 12 h task from Friday 08:00 works
    // 9 h on Friday and 3 h on the working Saturday, so it finishes there.
    let calendar = WEEK.replacen(
        "(0||Exceptions()())",
        "(0||Exceptions()((0||0(d|45297)((0||0(s|08:00|f|12:00)())))))",
        1,
    );
    let task = regular_task(1, "A", "12");
    let mut imported = import_handbuilt("2024-01-05 08:00", &[&task], &[], &calendar);
    assert_eq!(dates(&mut imported, 1).ef, 5 * 1_440 + 660);
    assert_eq!(note_count(&imported, "calendar 1: working exceptions"), 0);

    let mut plain = import_handbuilt("2024-01-05 08:00", &[&task], &[], WEEK);
    // Without the exception the same task finishes Monday 11:00.
    assert_eq!(dates(&mut plain, 1).ef, 7 * 1_440 + 660);
}

#[test]
fn handbuilt_shortened_day_exception_pushes_the_finish() {
    // 45293 is Tuesday 2024-01-02, cut to 08:00-12:00. An 8 h task from
    // Monday 13:00 works 4 h on Monday, 4 h on the short Tuesday.
    let calendar = WEEK.replacen(
        "(0||Exceptions()())",
        "(0||Exceptions()((0||0(d|45293)((0||0(s|08:00|f|12:00)())))))",
        1,
    );
    let task = regular_task(1, "A", "8");
    let mut imported = import_handbuilt("2024-01-01 13:00", &[&task], &[], &calendar);
    assert_eq!(dates(&mut imported, 1).ef, 1_440 + 720);
}

#[test]
fn handbuilt_duplicate_exception_dates_import_with_one_note() {
    let calendar = WEEK.replacen(
        "(0||Exceptions()())",
        "(0||Exceptions()((0||0(d|45297)((0||0(s|08:00|f|12:00)())))(0||1(d|45297)())(0||2(d|45293)())(0||3(d|45293)())))",
        1,
    );
    let task = regular_task(1, "A", "12");
    let mut imported = import_handbuilt("2024-01-05 08:00", &[&task], &[], &calendar);
    assert_eq!(dates(&mut imported, 1).ef, 5 * 1_440 + 660);
    assert!(
        imported.unsupported.iter().any(|line| line
            .starts_with("Calendar 1: two exceptions fall on one date and only one was read; working days can differ from the file's")
            && line.ends_with(": 1")),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn handbuilt_unparsable_exception_keeps_base_week_hours_and_is_noted() {
    // 45293 is Tuesday 2024-01-02. Neither of its two periods can be read, so the day
    // keeps its 08:00-17:00 hours: an 8 h task from Monday 13:00 works 4 h on
    // each of Monday and Tuesday. Read as a holiday it would finish on Wednesday.
    let calendar = WEEK.replacen(
        "(0||Exceptions()())",
        "(0||Exceptions()((0||0(d|45293)((0||0(s|8am|f|12:00)())(0||1(s|13:00|f|99:99)())))))",
        1,
    );
    let task = regular_task(1, "A", "8");
    let mut imported = import_handbuilt("2024-01-01 13:00", &[&task], &[], &calendar);
    assert_eq!(dates(&mut imported, 1).ef, 1_440 + 720);
    let notes: Vec<_> = imported
        .unsupported
        .iter()
        .filter(|line| {
            line.starts_with("Calendar 1: an exception whose working periods could not be read")
        })
        .collect();
    assert_eq!(notes.len(), 1, "{:?}", imported.unsupported);
    assert!(notes[0].ends_with(": 1 (first: 2024-01-02)"), "{notes:?}");
}

#[test]
fn handbuilt_partly_parsable_exception_keeps_its_parsable_periods() {
    // 45293 is Tuesday 2024-01-02: 09:00-11:00 parses, the second period does not.
    // Mon 13:00 works 4 h, Tuesday 2 h, Wednesday 08:00-10:00 the last 2 h.
    let calendar = WEEK.replacen(
        "(0||Exceptions()())",
        "(0||Exceptions()((0||0(d|45293)((0||0(s|09:00|f|11:00)())(0||1(s|13:00|f|99:99)())))))",
        1,
    );
    let task = regular_task(1, "A", "8");
    let mut imported = import_handbuilt("2024-01-01 13:00", &[&task], &[], &calendar);
    assert_eq!(dates(&mut imported, 1).ef, 2 * 1_440 + 600);
    assert!(
        imported.unsupported.iter().any(|line| line
            .starts_with("Calendar 1: some working periods of an exception could not be read and were left out; working hours on that day can differ from the file's")
            && line.ends_with(": 1 (first: 2024-01-02)")),
        "{:?}",
        imported.unsupported
    );
    assert_eq!(
        note_count(&imported, "calendar 1: exception with no readable"),
        0
    );
}

#[test]
fn handbuilt_constraints_map() {
    let msoa = "1\t1\t10\t1\tA\tMSOA\tTT_Task\tTK_NotStart\t1\t1\tCS_MSOA\t2024-01-03 08:00\t";
    let meo = "2\t1\t10\t1\tB\tMEO\tTT_Task\tTK_NotStart\t1\t1\tCS_MEO\t2024-01-05 08:00\t";
    let mut imported = import_handbuilt("2024-01-01 08:00", &[msoa, meo], &[], WEEK);
    assert_eq!(
        imported.schedule.tasks()[task_id(&imported, 1).0 as usize].snet,
        Some(3_360)
    );
    assert_eq!(
        imported.schedule.tasks()[task_id(&imported, 2).0 as usize]
            .limit
            .pin,
        Some(engine::Pin::Finish(6_240))
    );
    assert_eq!(
        note_count(&imported, "Activities with the must finish on"),
        0
    );
    assert!(imported.schedule.compute().is_ok());
}

#[test]
fn handbuilt_project_start_is_respected() {
    let task = regular_task(1, "A", "1");
    let mut imported = import_handbuilt("2024-01-03 08:00", &[&task], &[], WEEK);
    assert_eq!(dates(&mut imported, 1).es, 3_360);
}

#[test]
fn handbuilt_root_name_and_activity_code_map() {
    let task = regular_task(1, "CODE-1", "1");
    let mut imported = import_handbuilt("2024-01-01 08:00", &[&task], &[], WEEK);
    assert_eq!(imported.tasks[0].name, "HAND");
    assert_eq!(imported.tasks[1].code.as_deref(), Some("CODE-1"));
    assert!(imported.schedule.compute().is_ok());
}

#[test]
fn handbuilt_status_selects_duration() {
    let active = "1\t1\t10\t1\tA\tActive\tTT_Task\tTK_Active\t2\t9\t\t\t";
    let waiting = "2\t1\t10\t1\tB\tWaiting\tTT_Task\tTK_NotStart\t9\t3\t\t\t";
    let mut imported = import_handbuilt("2024-01-01 08:00", &[active, waiting], &[], WEEK);
    assert_eq!(
        imported.schedule.tasks()[task_id(&imported, 1).0 as usize].duration,
        120
    );
    assert_eq!(
        imported.schedule.tasks()[task_id(&imported, 2).0 as usize].duration,
        180
    );
    assert!(imported.schedule.compute().is_ok());
}

fn corpus_files(root: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(root).expect("XER_CORPUS_DIR should be readable");
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.expect("corpus entry should be readable");
        let kind = entry
            .file_type()
            .expect("corpus entry type should be readable");
        if kind.is_file() {
            files.push(entry.path());
        } else if kind.is_dir() {
            for child in fs::read_dir(entry.path()).expect("corpus subdirectory should be readable")
            {
                let child = child.expect("corpus child should be readable");
                if child.file_type().is_ok_and(|kind| kind.is_file()) {
                    files.push(child.path());
                }
            }
        }
    }
    files.retain(|path| {
        path.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("xer"))
    });
    files
}

#[test]
fn corpus_dir_files_import_without_error() {
    let Some(root) = std::env::var_os("XER_CORPUS_DIR") else {
        eprintln!("XER_CORPUS_DIR unset; corpus test skipped");
        return;
    };
    let files = corpus_files(Path::new(&root));
    assert!(!files.is_empty(), "XER_CORPUS_DIR contains no .xer files");
    for path in files {
        let bytes = fs::read(&path).expect("regular corpus file should be readable");
        let imported = parse_xer(&bytes, &ProjectSelection::All)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        eprintln!(
            "{}: {} tasks, {} unsupported",
            path.display(),
            imported.tasks.len(),
            imported.unsupported.len()
        );
    }
}

/// MPXJ's four P6 8.2.0 lag-calendar files (gs#111, gs#148): each sets the one setting that
/// differs between them. Each file has one activity and no link, so there is no computed date to
/// compare, and this checks only the mapping of the setting.
#[test]
fn corpus_lag_calendar_files_map_their_setting() {
    let Some(root) = std::env::var_os("XER_CORPUS_DIR") else {
        eprintln!("XER_CORPUS_DIR unset; lag calendar corpus test skipped");
        return;
    };
    let mut checked = 0;
    for path in corpus_files(Path::new(&root)) {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let expect: fn(engine::LagCalendar) -> bool = match name {
            n if n.ends_with("PredecessorCalendar.xer") => {
                |lag| lag == engine::LagCalendar::Predecessor
            }
            n if n.ends_with("SuccessorCalendar.xer") => {
                |lag| lag == engine::LagCalendar::Successor
            }
            n if n.ends_with("TwentyFourHourCalendar.xer") => {
                |lag| lag == engine::LagCalendar::TwentyFourHour
            }
            n if n.ends_with("ProjectDefaultCalendar.xer") => {
                |lag| matches!(lag, engine::LagCalendar::Fixed(_))
            }
            _ => continue,
        };
        let bytes = fs::read(&path).expect("regular corpus file should be readable");
        let imported = parse_xer(&bytes, &ProjectSelection::All)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            expect(imported.schedule.lag_calendar()),
            "{name}: {:?}",
            imported.schedule.lag_calendar()
        );
        assert!(
            !imported
                .unsupported
                .iter()
                .any(|line| line.starts_with("lag calendar")),
            "{name}: {:?}",
            imported.unsupported
        );
        assert!(imported.schedule.dependencies().is_empty(), "{name}");
        checked += 1;
    }
    eprintln!("lag calendar corpus: {checked} of 4 files checked");
}
