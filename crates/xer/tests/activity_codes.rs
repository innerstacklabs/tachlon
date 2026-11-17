//! The activity code tables: `ACTVTYPE`, `ACTVCODE` and `TASKACTV` become typed rows.

use xer::{Data, Error, Warning, parse, read};

const HEADER: &str =
    "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tProject Management\tUSD";

fn typed(lines: &[&str]) -> Result<Data, Error> {
    let mut all = vec![HEADER];
    all.extend_from_slice(lines);
    all.push("%E");
    let input = all.join("\n").into_bytes();
    read(&parse(&input).expect("the fixture parses"))
}

#[test]
fn the_three_tables_become_typed_rows_in_file_order() {
    let data = typed(&[
        "%T\tACTVTYPE",
        "%F\tactv_code_type_id\tactv_short_len\tseq_num\tactv_code_type\tproj_id\twbs_id\tactv_code_type_scope",
        "%R\t7000\t8\t1\tDiscipline\t\t\tAS_Global",
        "%R\t7001\t4\t2\tPhase\t12\t\tAS_Project",
        "%T\tACTVCODE",
        "%F\tactv_code_id\tparent_actv_code_id\tactv_code_type_id\tactv_code_name\tshort_name\tseq_num\tcolor",
        "%R\t7100\t\t7000\tElectrical\tELEC\t1\t",
        "%R\t7101\t7100\t7000\tLighting\tLGT\t2\t",
        "%T\tTASKACTV",
        "%F\ttask_id\tactv_code_type_id\tactv_code_id\tproj_id",
        "%R\t1\t7000\t7100\t12",
        "%R\t2\t7000\t7101\t12",
        "%R\t2\t7001\t7200\t",
    ])
    .unwrap();

    assert_eq!(data.actv_type_rows.len(), 2);
    let first = &data.actv_type_rows[0];
    assert_eq!(first.actv_code_type_id, 7000);
    assert_eq!(first.actv_code_type.as_deref(), Some("Discipline"));
    assert_eq!(first.actv_code_type_scope.as_deref(), Some("AS_Global"));
    assert_eq!((first.proj_id, first.seq_num), (None, Some(1)));
    let second = &data.actv_type_rows[1];
    assert_eq!(second.actv_code_type.as_deref(), Some("Phase"));
    assert_eq!(second.actv_code_type_scope.as_deref(), Some("AS_Project"));
    assert_eq!(second.proj_id, Some(12));

    assert_eq!(data.actv_code_rows.len(), 2);
    let child = &data.actv_code_rows[1];
    assert_eq!(child.actv_code_id, 7101);
    assert_eq!(child.actv_code_type_id, 7000);
    assert_eq!(child.parent_actv_code_id, Some(7100));
    assert_eq!(child.actv_code_name.as_deref(), Some("Lighting"));
    assert_eq!(child.short_name.as_deref(), Some("LGT"));
    assert_eq!(data.actv_code_rows[0].parent_actv_code_id, None);

    let assigned: Vec<_> = data
        .task_actv_rows
        .iter()
        .map(|row| {
            (
                row.task_id,
                row.actv_code_type_id,
                row.actv_code_id,
                row.proj_id,
            )
        })
        .collect();
    assert_eq!(
        assigned,
        [
            (1, 7000, 7100, Some(12)),
            (2, 7000, 7101, Some(12)),
            (2, 7001, 7200, None)
        ]
    );
    assert!(data.not_read.is_empty(), "{:?}", data.not_read);
}

#[test]
fn a_file_without_the_tables_has_no_rows() {
    let data = typed(&[]).unwrap();
    assert_eq!(data.actv_type_rows, []);
    assert_eq!(data.actv_code_rows, []);
    assert_eq!(data.task_actv_rows, []);
}

#[test]
fn a_missing_key_column_is_an_error_naming_the_table() {
    let error = typed(&["%T\tTASKACTV", "%F\ttask_id\tactv_code_id", "%R\t1\t7100"]).unwrap_err();
    assert_eq!(
        error,
        Error::MissingColumn {
            table: "TASKACTV".into(),
            field: "actv_code_type_id".into(),
            line: 2,
        }
    );
}

#[test]
fn a_bad_key_value_is_an_error_and_a_bad_optional_value_is_a_warning() {
    let error = typed(&[
        "%T\tACTVCODE",
        "%F\tactv_code_id\tactv_code_type_id",
        "%R\t7100\tx",
    ])
    .unwrap_err();
    assert_eq!(
        error,
        Error::BadValue {
            table: "ACTVCODE".into(),
            field: "actv_code_type_id".into(),
            line: 4,
            value: "x".into(),
        }
    );

    let data = typed(&[
        "%T\tACTVCODE",
        "%F\tactv_code_id\tactv_code_type_id\tparent_actv_code_id",
        "%R\t7100\t7000\tnope",
    ])
    .unwrap();
    assert_eq!(data.actv_code_rows[0].parent_actv_code_id, None);
    assert!(data.warnings().iter().any(|warning| matches!(
        warning,
        Warning::BadValue { table, field, .. }
            if table == "ACTVCODE" && field == "parent_actv_code_id"
    )));
}
