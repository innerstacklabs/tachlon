use xer::{
    ConstraintType, Data, Encoding, Error, PredRow, PredType, ProjectRow, SchedOptionsRow,
    StatusCode, TaskRow, TaskType, Warning, WbsRow, parse, read,
};

const HEADER: &str =
    "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tProject Management\tUSD";

fn bytes(lines: &[&str], eol: &str) -> Vec<u8> {
    lines.join(eol).into_bytes()
}

fn typed(lines: &[&str], eol: &str) -> Data {
    let input = bytes(lines, eol);
    let xer = parse(&input).expect("the fixture parses");
    read(&xer).expect("the fixture reads")
}

fn task_error(fields: &str, row: &str) -> Error {
    let input = bytes(&[HEADER, "%T\tTASK", fields, row, "%E"], "\n");
    let xer = parse(&input).expect("the fixture parses");
    read(&xer).expect_err("the fixture is rejected")
}

fn bad_key(field: &str, value: &str) -> Error {
    Error::BadValue {
        table: "TASK".into(),
        field: field.into(),
        line: 4,
        value: value.into(),
    }
}

fn task_with_optional(field: &str, value: &str) -> Data {
    let fields = format!("%F\ttask_id\tproj_id\twbs_id\t{field}");
    let row = format!("%R\t1\t2\t3\t{value}");
    typed(&[HEADER, "%T\tTASK", &fields, &row, "%E"], "\n")
}

fn assert_bad_warning(data: &Data, field: &str, value: &str) {
    assert_eq!(
        data.warnings(),
        &[Warning::BadValue {
            table: "TASK".into(),
            field: field.into(),
            line: 4,
            value: value.into(),
        }]
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one fixture holds every typed table, and splitting it would repeat the fixture"
)]
fn reordered_columns_and_unknown_fields_read_all_five_typed_tables() {
    let data = typed(
        &[
            HEADER,
            "%T\tPROJECT",
            "%F\texport_flag\tunknown_project\tlast_recalc_date\tproj_id\tplan_end_date\tclndr_id\tproj_short_name\tplan_start_date",
            "%R\tY\tignored\t2024-03-06 10:30\t7\t2024-03-08 17:00\t42\tALPHA\t2024-03-04 08:00",
            "%T\tPROJWBS",
            "%F\twbs_name\tproj_node_flag\tunknown_wbs\tproj_id\tseq_num\twbs_id\twbs_short_name\tparent_wbs_id",
            "%R\tEngineering\tN\tignored\t7\t3\t70\tENG\t",
            "%T\tTASK",
            "%F\tfree_float_hr_cnt\ttarget_end_date\tunknown_task\ttask_name\twbs_id\tstatus_code\tact_end_date\tcstr_date2\tproj_id\ttask_type\ttarget_drtn_hr_cnt\tclndr_id\tearly_start_date\tcstr_type\tlate_end_date\ttask_id\tremain_drtn_hr_cnt\tcstr_date\tact_start_date\ttotal_float_hr_cnt\ttask_code\tearly_end_date\tcstr_type2\tlate_start_date\ttarget_start_date",
            "%R\t2.25\t2024-03-08 17:00\tignored\tExcavate\t70\tTK_Active\t2024-03-05 12:00\t2024-03-07 09:00\t7\tTT_Task\t16.5\t42\t2024-03-04 08:00\tCS_MSOA\t2024-03-08 18:00\t700\t4\t2024-03-04 08:00\t2024-03-04 09:00\t6.5\tA100\t2024-03-05 17:00\tCS_MEOB\t2024-03-04 07:00\t",
            "%T\tTASKPRED",
            "%F\tlag_hr_cnt\tunknown_pred\tpred_proj_id\ttask_id\tpred_type\tproj_id\tpred_task_id",
            "%R\t1.5\tignored\t8\t700\tPR_FS\t7\t699",
            "%T\tSCHEDOPTIONS",
            "%F\tunknown_option\tsched_calendar_on_relationship_lag\tproj_id",
            "%R\tignored\tPredecessor Activity Calendar\t7",
            "%E",
        ],
        "\r\n",
    );
    assert_eq!(
        data.project_rows[0],
        ProjectRow {
            proj_id: 7,
            proj_short_name: Some("ALPHA".into()),
            clndr_id: Some(42),
            plan_start_date: Some(28_492_320),
            plan_end_date: Some(28_498_620),
            last_recalc_date: Some(28_495_350),
            export_flag: Some(true),
            complete_pct_type: None,
        }
    );
    assert_eq!(
        data.wbs_rows[0],
        WbsRow {
            wbs_id: 70,
            proj_id: 7,
            parent_wbs_id: None,
            seq_num: Some(3),
            proj_node_flag: Some(false),
            wbs_short_name: Some("ENG".into()),
            wbs_name: Some("Engineering".into()),
        }
    );
    assert_eq!(
        data.task_rows[0],
        TaskRow {
            task_id: 700,
            proj_id: 7,
            wbs_id: 70,
            clndr_id: Some(42),
            task_code: Some("A100".into()),
            task_name: Some("Excavate".into()),
            task_type: Some(TaskType::Task),
            status_code: Some(StatusCode::Active),
            target_drtn_hr_cnt: Some(16.5),
            remain_drtn_hr_cnt: Some(4.0),
            cstr_type: Some(ConstraintType::StartOnOrAfter),
            cstr_date: Some(28_492_320),
            cstr_type2: Some(ConstraintType::FinishOnOrBefore),
            cstr_date2: Some(28_496_700),
            early_start_date: Some(28_492_320),
            early_end_date: Some(28_494_300),
            late_start_date: Some(28_492_260),
            late_end_date: Some(28_498_680),
            act_start_date: Some(28_492_380),
            act_end_date: Some(28_494_000),
            target_start_date: None,
            target_end_date: Some(28_498_620),
            total_float_hr_cnt: Some(6.5),
            free_float_hr_cnt: Some(2.25),
            complete_pct_type: None,
            phys_complete_pct: None,
            complete_pct: None,
        }
    );
    assert_eq!(
        data.pred_rows[0],
        PredRow {
            task_id: 700,
            pred_task_id: 699,
            proj_id: Some(7),
            pred_proj_id: Some(8),
            pred_type: Some(PredType::FinishToStart),
            lag_hr_cnt: Some(1.5),
        }
    );
    assert_eq!(
        data.sched_options_rows[0],
        SchedOptionsRow {
            proj_id: Some(7),
            sched_calendar_on_relationship_lag: Some("Predecessor Activity Calendar".into()),
        }
    );
}

#[test]
fn header_reads_full_and_version_only_forms() {
    let data = typed(&[HEADER, "%E"], "\n");
    assert_eq!(data.header.version.as_deref(), Some("19.12"));
    assert_eq!(data.header.export_date, Some(28_487_520));
    assert_eq!(data.header.currency.as_deref(), Some("USD"));

    let data = typed(&["ERMHDR\t19.12", "%E"], "\n");
    assert_eq!(data.header.version.as_deref(), Some("19.12"));
    assert_eq!(data.header.export_date, None);
    assert_eq!(data.header.currency, None);
}

#[test]
fn future_enum_codes_are_preserved_as_other_values() {
    let data = typed(
        &[
            HEADER,
            "%T\tTASK",
            "%F\ttask_id\tproj_id\twbs_id\ttask_type\tstatus_code\tcstr_type",
            "%R\t1\t2\t3\tTT_Future\tTK_Future\tCS_Future",
            "%T\tTASKPRED",
            "%F\ttask_id\tpred_task_id\tpred_type",
            "%R\t1\t4\tPR_Future",
            "%E",
        ],
        "\n",
    );
    let task = &data.task_rows[0];
    assert_eq!(task.task_type, Some(TaskType::Other("TT_Future".into())));
    assert_eq!(
        task.status_code,
        Some(StatusCode::Other("TK_Future".into()))
    );
    assert_eq!(
        task.cstr_type,
        Some(ConstraintType::Other("CS_Future".into()))
    );
    assert_eq!(
        data.pred_rows[0].pred_type,
        Some(PredType::Other("PR_Future".into()))
    );
}

#[test]
fn task_key_errors_and_empty_table_column_rules_are_enforced() {
    assert_eq!(
        task_error("%F\tproj_id\ttask_id\twbs_id", "%R\t2\t12x\t3"),
        bad_key("task_id", "12x")
    );
    assert_eq!(
        task_error("%F\ttask_id\tproj_id\twbs_id", "%R\t1\t2\t"),
        bad_key("wbs_id", "")
    );
    assert_eq!(
        task_error("%F\tproj_id\twbs_id", "%R\t2\t3"),
        Error::MissingColumn {
            table: "TASK".into(),
            field: "task_id".into(),
            line: 2,
        }
    );
    let data = typed(&[HEADER, "%T\tTASK", "%F\ttask_name", "%E"], "\n");
    assert_eq!(data.task_rows, [] as [xer::TaskRow; 0]);
}

#[test]
fn task_proj_id_bad_and_empty_values_are_errors() {
    assert_eq!(
        task_error("%F\ttask_id\tproj_id\twbs_id", "%R\t1\tx2\t3"),
        bad_key("proj_id", "x2")
    );
    assert_eq!(
        task_error("%F\ttask_id\tproj_id\twbs_id", "%R\t1\t\t3"),
        bad_key("proj_id", "")
    );
}

#[test]
fn sched_options_proj_id_is_optional_and_bad_values_warn() {
    let data = typed(
        &[
            HEADER,
            "%T\tSCHEDOPTIONS",
            "%F\tproj_id\tsched_calendar_on_relationship_lag",
            "%R\tx7\tPredecessor Activity Calendar",
            "%T\tTASK",
            "%F\ttask_id\tproj_id\twbs_id",
            "%R\t1\t2\t3",
            "%E",
        ],
        "\n",
    );
    assert_eq!(data.sched_options_rows[0].proj_id, None);
    assert_eq!(
        data.sched_options_rows[0]
            .sched_calendar_on_relationship_lag
            .as_deref(),
        Some("Predecessor Activity Calendar")
    );
    assert_eq!(data.task_rows.len(), 1);
    assert_eq!(
        data.warnings(),
        &[Warning::BadValue {
            table: "SCHEDOPTIONS".into(),
            field: "proj_id".into(),
            line: 4,
            value: "x7".into(),
        }]
    );

    let data = typed(
        &[
            HEADER,
            "%T\tSCHEDOPTIONS",
            "%F\tsched_calendar_on_relationship_lag",
            "%R\tPredecessor Activity Calendar",
            "%E",
        ],
        "\n",
    );
    assert_eq!(data.sched_options_rows[0].proj_id, None);
    assert_eq!(data.warnings(), []);
}

#[test]
fn optional_bad_values_warn_while_empty_values_do_not() {
    let bad_date = task_with_optional("target_start_date", "2024-13-45 08:00");
    assert_eq!(bad_date.task_rows[0].target_start_date, None);
    assert_bad_warning(&bad_date, "target_start_date", "2024-13-45 08:00");
    let comma = task_with_optional("target_drtn_hr_cnt", "1,5");
    assert_eq!(comma.task_rows[0].target_drtn_hr_cnt, None);
    assert_bad_warning(&comma, "target_drtn_hr_cnt", "1,5");

    let empty = task_with_optional("target_drtn_hr_cnt", "");
    assert_eq!(empty.task_rows[0].target_drtn_hr_cnt, None);
    assert_eq!(empty.warnings(), []);
}

#[test]
fn project_summaries_preserve_order_counts_and_export_flags() {
    let data = typed(
        &[
            HEADER,
            "%T\tPROJECT",
            "%F\tproj_id\tproj_short_name\texport_flag",
            "%R\t20\tReferenced\tN",
            "%R\t10\tExported\tY",
            "%T\tTASK",
            "%F\ttask_id\tproj_id\twbs_id",
            "%R\t1\t10\t100",
            "%R\t2\t20\t200",
            "%R\t3\t10\t100",
            "%E",
        ],
        "\n",
    );
    let projects = data.projects();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].proj_id, 20);
    assert_eq!(projects[0].proj_short_name.as_deref(), Some("Referenced"));
    assert_eq!(projects[0].activity_count, 1);
    assert!(!projects[0].exported);
    assert_eq!(projects[1].proj_id, 10);
    assert_eq!(projects[1].proj_short_name.as_deref(), Some("Exported"));
    assert_eq!(projects[1].activity_count, 2);
    assert!(projects[1].exported);
}

#[test]
fn not_read_lists_only_unsupported_tables_in_file_order() {
    let data = typed(
        &[
            HEADER,
            "%T\tTASKRSRC",
            "%F\tid",
            "%R\t1",
            "%R\t2",
            "%T\tPROJECT",
            "%F\tproj_id",
            "%R\t7",
            "%T\tRSRC",
            "%F\tid",
            "%R\t3",
            "%T\tPROJWBS",
            "%F\twbs_id\tproj_id",
            "%T\tTASK",
            "%F\ttask_id\tproj_id\twbs_id",
            "%T\tTASKPRED",
            "%F\ttask_id\tpred_task_id",
            "%T\tSCHEDOPTIONS",
            "%F\tproj_id",
            "%E",
        ],
        "\n",
    );
    let omitted: Vec<_> = data
        .not_read
        .iter()
        .map(|table| (table.name.as_str(), table.rows))
        .collect();
    assert_eq!(omitted, [("TASKRSRC", 2), ("RSRC", 1)]);
}

#[test]
fn table_warnings_precede_value_warnings_and_windows_1252_decodes_text() {
    let mut input = bytes(
        &[
            HEADER,
            "%T\tTASK",
            "%F\ttask_id\tproj_id\twbs_id\ttarget_start_date\ttask_name",
            "%R\t1\t2\t3",
        ],
        "\n",
    );
    input.extend_from_slice(b"\n%R\t2\t2\t3\tbad\t");
    input.push(0xe9);
    input.extend_from_slice(b"\n%E");
    let xer = parse(&input).unwrap();
    let data = read(&xer).unwrap();

    assert_eq!(data.encoding(), Encoding::Windows1252);
    assert_eq!(data.task_rows[1].task_name.as_deref(), Some("é"));
    assert_eq!(
        data.warnings(),
        &[Warning::BadValue {
            table: "TASK".into(),
            field: "target_start_date".into(),
            line: 5,
            value: "bad".into(),
        },]
    );
}

#[test]
fn u32_boundary_ids_are_checked_without_panicking() {
    let data = typed(
        &[
            HEADER,
            "%T\tTASK",
            "%F\ttask_id\tproj_id\twbs_id\tunknown_1\tunknown_2\tunknown_3\tunknown_4\tunknown_5",
            "%R\t4294967295\t2\t3\ta\tb\tc\td\te",
            "%E",
        ],
        "\n",
    );
    assert_eq!(data.task_rows[0].task_id, u32::MAX);

    assert_eq!(
        task_error("%F\ttask_id\tproj_id\twbs_id", "%R\t4294967296\t2\t3"),
        bad_key("task_id", "4294967296")
    );
}

#[test]
fn rows_short_only_in_optional_trailing_cells_read_empty_without_a_warning() {
    let data = typed(
        &[
            HEADER,
            "%T\tTASK",
            "%F\ttask_id\tproj_id\twbs_id\ttask_name\ttarget_start_date",
            "%R\t1\t2\t3",
            "%R\t2\t2\t3\tBuild",
            "%E",
        ],
        "\n",
    );
    assert_eq!(data.warnings(), &[]);
    assert_eq!(data.task_rows.len(), 2);
    assert_eq!(data.task_rows[0].task_name, None);
    assert_eq!(data.task_rows[0].target_start_date, None);
    assert_eq!(data.task_rows[1].task_name.as_deref(), Some("Build"));
}

#[test]
fn a_row_short_in_a_required_cell_fails_naming_table_row_and_column() {
    let error = task_error("%F\ttask_id\tproj_id\twbs_id\ttask_name", "%R\t2\t2");
    assert_eq!(error, bad_key("wbs_id", ""));
    assert_eq!(
        error.to_string(),
        "line 4: table TASK field wbs_id has a bad value \"\""
    );
}

#[test]
fn a_short_row_does_not_drop_a_task_or_warn_before_the_error() {
    let input = bytes(
        &[
            HEADER,
            "%T\tTASK",
            "%F\ttask_id\tproj_id\twbs_id\ttask_name",
            "%R\t1\t2\t3\tKept",
            "%R\t2\t2",
            "%E",
        ],
        "\n",
    );
    let xer = parse(&input).unwrap();
    assert_eq!(xer.warnings(), &[]);
    let expected = Error::BadValue {
        table: "TASK".into(),
        field: "wbs_id".into(),
        line: 5,
        value: String::new(),
    };
    assert_eq!(read(&xer).unwrap_err(), expected);
}

#[test]
fn every_typed_table_fails_on_a_row_missing_a_required_cell() {
    let cases = [
        ("PROJECT", "%F\tproj_id", "%R", "proj_id"),
        ("PROJWBS", "%F\twbs_id\tproj_id", "%R\t1", "proj_id"),
        (
            "TASKPRED",
            "%F\ttask_id\tpred_task_id",
            "%R\t1",
            "pred_task_id",
        ),
        ("CALENDAR", "%F\tclndr_id\tclndr_name", "%R", "clndr_id"),
    ];
    for (table, fields, row, column) in cases {
        let heading = format!("%T\t{table}");
        let input = bytes(&[HEADER, &heading, fields, row, "%E"], "\n");
        let xer = parse(&input).unwrap();
        let expected = Error::BadValue {
            table: table.into(),
            field: column.into(),
            line: 4,
            value: String::new(),
        };
        assert_eq!(read(&xer).unwrap_err(), expected, "{table}");
    }
}

#[test]
fn p6_8_2_rows_short_in_the_last_column_only_raise_no_warning() {
    // The shape of a P6 8.2.0 export: 41 rows, 16 of them without the trailing empty cell.
    let rows: Vec<String> = (1..=41)
        .map(|id| match id % 5 {
            0 | 2 => format!("%R\t{id}\t2\t3\tTask {id}"),
            _ => format!("%R\t{id}\t2\t3\tTask {id}\t"),
        })
        .collect();
    let short = rows.iter().filter(|row| !row.ends_with('\t')).count();
    assert_eq!(short, 16);
    let mut lines = vec![
        HEADER,
        "%T\tTASK",
        "%F\ttask_id\tproj_id\twbs_id\ttask_name\tnotes",
    ];
    lines.extend(rows.iter().map(String::as_str));
    lines.push("%E");
    let data = typed(&lines, "\n");
    assert_eq!(data.task_rows.len(), 41);
    assert_eq!(data.warnings(), &[]);
}
