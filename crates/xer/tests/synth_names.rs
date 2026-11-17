//! The `Site` naming mode of the synthetic writer: construction phrases for work packages and
//! activities, and nothing else in the file changed.

#![cfg(feature = "synth")]

use std::collections::HashSet;
use xer::synth::{Names, Spec, write};

/// The `%R` rows of one table, as the cells of each row (the leading `%R` dropped), with the
/// column names.
fn table(text: &str, name: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut lines = text
        .lines()
        .skip_while(|line| *line != format!("%T\t{name}"));
    lines.next().expect("table header line");
    let fields = lines
        .next()
        .expect("field line")
        .split('\t')
        .skip(1)
        .map(str::to_string)
        .collect();
    let rows = lines
        .take_while(|line| line.starts_with("%R"))
        .map(|line| line.split('\t').skip(1).map(str::to_string).collect())
        .collect();
    (fields, rows)
}

fn column(fields: &[String], name: &str) -> usize {
    fields.iter().position(|f| f == name).expect("column")
}

fn text_of(spec: &Spec) -> String {
    String::from_utf8(write(spec)).expect("utf-8")
}

#[test]
fn site_names_are_construction_phrases_unique_within_a_work_package() {
    let spec = Spec {
        names: Names::Site,
        ..Spec::with_activities(1, 5000)
    };
    let text = text_of(&spec);
    let (fields, rows) = table(&text, "TASK");
    let (name, wbs) = (column(&fields, "task_name"), column(&fields, "wbs_id"));
    assert_eq!(rows.len(), 5000);
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for row in &rows {
        let task_name = row[name].trim_end_matches(" Café");
        assert!(!task_name.contains("Activity"), "{task_name}");
        assert!(task_name.contains(", level "), "{task_name}");
        assert!(
            seen.insert((row[wbs].clone(), task_name.to_string())),
            "{task_name} repeats in work package {}",
            row[wbs]
        );
    }

    let (fields, rows) = table(&text, "PROJWBS");
    let name = column(&fields, "wbs_name");
    let names: HashSet<&str> = rows.iter().skip(1).map(|row| row[name].as_str()).collect();
    assert_eq!(names.len(), rows.len() - 1, "work package names repeat");
    assert!(names.iter().all(|n| !n.contains("Work package")));
}

#[test]
fn site_names_change_only_the_names() {
    let numbered = Spec::with_activities(7, 600);
    let site = Spec {
        names: Names::Site,
        ..numbered
    };
    let rest = |text: &str| {
        let (_, tasks) = table(text, "TASK");
        let (fields, _) = table(text, "TASK");
        let name = column(&fields, "task_name");
        let tasks = tasks
            .into_iter()
            .map(|mut row| {
                row.remove(name);
                row
            })
            .collect::<Vec<_>>();
        let (_, links) = table(text, "TASKPRED");
        (tasks, links)
    };
    let first = |spec: &Spec| {
        let text = text_of(spec);
        let (fields, rows) = table(&text, "TASK");
        rows[0][column(&fields, "task_name")].clone()
    };
    assert_eq!(first(&numbered), "Activity 1");
    let root = |spec: &Spec| {
        let text = text_of(spec);
        let (fields, rows) = table(&text, "PROJWBS");
        rows[0][column(&fields, "wbs_name")].clone()
    };
    assert_eq!(
        root(&numbered),
        "Project 1",
        "the default root row is as it was"
    );
    assert_ne!(first(&site), "Activity 1");
    assert_eq!(rest(&text_of(&numbered)), rest(&text_of(&site)));
    assert_eq!(
        text_of(&numbered).lines().count(),
        text_of(&site).lines().count()
    );
}
