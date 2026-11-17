//! App-owned display data for tasks (names, activity IDs and outline).

use engine::TaskId;
use import::ActivityCode;

/// App-owned display data for one task, indexed like [`engine::Schedule::tasks`].
/// `parent` records only the outline; summary bars are a later feature, and the
/// engine deliberately knows nothing about task names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskMeta {
    /// The task's own name as the file gave it (`Pour slab`), `" / "` and all. The full path the
    /// table and hover card show is derived from the parents: see [`outline_path`].
    pub name: String,
    /// The enclosing outline task, `None` at the top level. The tasks form a
    /// forest (see [`crate::outline::Outline`]); a parent need not have a lower id.
    pub parent: Option<TaskId>,
    /// Outline depth, 1 (area) to 3 (activity).
    pub outline_level: u8,
    /// The P6 activity ID shown in the ID column (`A1010`). `None` for MSPDI tasks, XER summaries
    /// and the demo.
    pub code: Option<String>,
    /// The P6 activity codes assigned to the task (code type and value), in file order. Empty for
    /// MSPDI tasks, XER summaries, the demo and a task the file gives no activity code row.
    pub activity_codes: Vec<ActivityCode>,
    /// The WBS code shown in the WBS column (`HAND.1.2`): for an XER task the `wbs_short_name` of
    /// each WBS node from the project root down to its own, joined with a dot; for an MSPDI task
    /// its `WBS` text. `None` when the file states none, and for the demo and tasks inserted here.
    pub wbs_code: Option<String>,
    /// The task UID an import gave the task: the MS Project `UID` or the P6 `task_id`. `None` for the
    /// demo, for a task inserted here and for a task read back from a saved document (which does not
    /// keep it yet). [`Session::compare_file`](crate::Session::compare_file)
    /// matches the tasks of two MS Project files by it.
    pub uid: Option<u32>,
    /// The instant an import floored the task at: its project's start (see
    /// `import::ImportedTask::floor`). A start-no-earlier-than counts as a real constraint only
    /// above it ([`Session::start_constraint`](crate::Session::start_constraint)). `None` for the
    /// demo, a task inserted here, an XER summary and a project that states no start. A saved
    /// document keeps it, except that it leaves out a floor the reader gives back unasked: the
    /// earliest start-no-earlier-than in the file (`constraint::fallback_floor`), or any floor of a
    /// task with no start-no-earlier-than.
    pub floor: Option<engine::Instant>,
    /// The color of the task's bar, opaque sRGB bytes (`#rrggbb` in a document), or `None` for the
    /// theme's. Only a saved document gives one.
    pub color: Option<[u8; 4]>,
    /// The text the task's bar shows in place of its name, or `None` for the name. The table still
    /// shows the name. Only a saved document gives one.
    pub label: Option<String>,
    /// The opaque key the host gave the task, such as a database id. The app never reads it. Only
    /// plain data and a saved document give one.
    pub key: Option<String>,
}

impl TaskMeta {
    /// The text of the ID column: the activity ID, or an empty string when the task has none.
    #[must_use]
    pub fn code_text(&self) -> &str {
        self.code.as_deref().unwrap_or_default()
    }

    /// The text of the WBS column: the WBS code, or an empty string when the task has none.
    #[must_use]
    pub fn wbs_code_text(&self) -> &str {
        self.wbs_code.as_deref().unwrap_or_default()
    }
}

/// The outline path of task `task`: the names of its ancestors, outermost first, then its own,
/// joined by `" / "`. Empty when `task` is past `meta`. A name keeps any `" / "` it was imported
/// with, so the path is for display and search only: it does not split back into names. The walk
/// stops after `meta.len()` steps so a malformed parent cycle ends.
#[must_use]
pub fn outline_path(meta: &[TaskMeta], task: usize) -> String {
    let mut names = Vec::new();
    let mut at = task;
    while let Some(entry) = meta.get(at) {
        names.push(entry.name.as_str());
        match entry.parent {
            Some(parent) if names.len() < meta.len() => at = parent.0 as usize,
            _ => break,
        }
    }
    names.reverse();
    names.join(" / ")
}

/// Whether any task carries an activity ID. The table shows the ID column only then: an MSPDI file,
/// the demo and the stress fixture have none.
#[must_use]
pub fn has_codes(meta: &[TaskMeta]) -> bool {
    meta.iter().any(|task| task.code.is_some())
}

/// Whether any task carries a WBS code. The table shows the WBS column only then: the demo and the
/// stress fixture have none. Every task counts, deleted or not, so the column stays while a file is
/// open.
#[must_use]
pub fn has_wbs_codes(meta: &[TaskMeta]) -> bool {
    meta.iter().any(|task| task.wbs_code.is_some())
}

/// Percent complete (0 to 100) for each of `task_count` tasks of the demo, which has no progress
/// of its own: a fixed spread, from the task id alone, so every run draws the same bars. About a
/// quarter are done, a sixth have not started and the rest are somewhere between.
#[must_use]
pub fn demo_progress(task_count: usize) -> Vec<u8> {
    (0..task_count as u64)
        .map(|task| {
            // splitmix64 finaliser: well spread, no state, no dependency.
            let mut z = task.wrapping_add(0x9e37_79b9_7f4a_7c15);
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^= z >> 31;
            match z % 100 {
                0..=24 => 100,
                25..=40 => 0,
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "n is 41 to 99 here (the arms above take 0 to 40), so n - 40 is 1 to 59; (z >> 8) as u8 keeps the low 8 bits of z >> 8, and only its parity is used"
                )]
                n => (n - 40) as u8 + (z >> 8) as u8 % 2,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{TaskMeta, demo_progress, has_wbs_codes};
    use import::{ActivityCode, ProjectSelection};

    /// One project, one activity `A1` (task 101) with a `Discipline` code and a `Phase` code, one
    /// activity `A2` (task 102) with none.
    fn xer_with_codes() -> Vec<u8> {
        let week = "(0||CalendarData()((0||DaysOfWeek()((0||2()((0||0(s|08:00|f|17:00)())))))(0||Exceptions()())))";
        let rows = [
            "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tPM\tUSD".to_string(),
            "%T\tPROJECT\n%F\tproj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id".to_string(),
            "%R\t1\tHAND\tY\t2024-01-01 08:00\t1".to_string(),
            "%T\tPROJWBS\n%F\twbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name".to_string(),
            "%R\t10\t1\t\t1\tY\tRoot".to_string(),
            "%T\tTASK\n%F\ttask_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2".to_string(),
            "%R\t101\t1\t10\t1\tA1\tFirst\tTT_Task\tTK_NotStart\t24\t24\t\t\t".to_string(),
            "%R\t102\t1\t10\t1\tA2\tSecond\tTT_Task\tTK_NotStart\t24\t24\t\t\t".to_string(),
            "%T\tACTVTYPE\n%F\tactv_code_type_id\tactv_code_type".to_string(),
            "%R\t7001\tDiscipline".to_string(),
            "%R\t7002\tPhase".to_string(),
            "%T\tACTVCODE\n%F\tactv_code_id\tactv_code_type_id\tshort_name".to_string(),
            "%R\t7101\t7001\tELEC".to_string(),
            "%R\t7201\t7002\tP1".to_string(),
            "%T\tTASKACTV\n%F\ttask_id\tactv_code_type_id\tactv_code_id".to_string(),
            "%R\t101\t7001\t7101".to_string(),
            "%R\t101\t7002\t7201".to_string(),
            "%T\tCALENDAR\n%F\tclndr_id\tclndr_type\tclndr_data".to_string(),
            format!("%R\t1\tCA_Project\t{week}"),
            "%E".to_string(),
        ];
        (rows.join("\n") + "\n").into_bytes()
    }

    #[test]
    fn a_loaded_xer_task_carries_its_activity_codes() {
        let loaded = crate::load_bytes(&xer_with_codes(), &ProjectSelection::Unspecified, &|| 0.0)
            .unwrap_or_else(|_| panic!("the file should load"));
        let by_code = |code: &str| -> &TaskMeta {
            loaded
                .meta
                .iter()
                .find(|task| task.code.as_deref() == Some(code))
                .expect("activity")
        };
        let code = |code_type: &str, value: &str| ActivityCode {
            code_type: code_type.to_owned(),
            value: value.to_owned(),
        };
        assert_eq!(
            by_code("A1").activity_codes,
            [code("Discipline", "ELEC"), code("Phase", "P1")]
        );
        assert_eq!(
            by_code("A2").activity_codes,
            [] as [import::ActivityCode; 0]
        );
        assert!(
            loaded
                .meta
                .iter()
                .filter(|task| task.code.is_none())
                .all(|task| task.activity_codes.is_empty())
        );
    }

    #[test]
    fn a_loaded_xer_task_carries_its_wbs_code_and_the_column_shows_only_then() {
        let load = |bytes: &[u8]| {
            crate::load_bytes(bytes, &ProjectSelection::Unspecified, &|| 0.0)
                .unwrap_or_else(|_| panic!("the file should load"))
        };
        let plain = load(&xer_with_codes());
        assert!(!has_wbs_codes(&plain.meta), "no WBS short name in the file");
        assert!(
            plain
                .meta
                .iter()
                .all(|task| task.wbs_code_text().is_empty())
        );

        let text = String::from_utf8(xer_with_codes()).expect("the fixture is text");
        let text = text
            .replace(
                "proj_node_flag\twbs_name",
                "proj_node_flag\twbs_short_name\twbs_name",
            )
            .replace("\t1\tY\tRoot", "\t1\tY\tHAND\tRoot");
        let loaded = load(text.as_bytes());
        assert!(has_wbs_codes(&loaded.meta));
        assert!(!has_wbs_codes(&[]));
        let first = loaded
            .meta
            .iter()
            .find(|task| task.code.as_deref() == Some("A1"))
            .expect("activity");
        assert_eq!(first.wbs_code_text(), "HAND");
        assert!(
            loaded
                .meta
                .iter()
                .all(|task| task.wbs_code_text() == "HAND")
        );
    }

    #[test]
    fn demo_progress_is_a_fixed_spread_in_range() {
        let progress = demo_progress(10_000);
        assert_eq!(progress.len(), 10_000);
        assert_eq!(progress, demo_progress(10_000), "same every run");
        assert!(progress.iter().all(|&p| p <= 100));
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count of at most 10,000 progress values, exact in f64"
        )]
        let share =
            |keep: fn(&u8) -> bool| progress.iter().filter(|p| keep(p)).count() as f64 / 1e4;
        assert!((0.2..0.3).contains(&share(|&p| p == 100)), "done");
        assert!((0.12..0.22).contains(&share(|&p| p == 0)), "not started");
        assert!(share(|&p| (1..100).contains(&p)) > 0.5, "under way");
    }
}
