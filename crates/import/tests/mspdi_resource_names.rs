//! The MS Project reader keeps the names of the resources assigned to each task, in the order the
//! file lists the assignments.

use import::{Imported, parse_mspdi};

const RESOURCES: &str = "<Resources><Resource><UID>0</UID><Name></Name></Resource>\
<Resource><UID>1</UID><Name>Ana</Name></Resource><Resource><UID>2</UID><Name>Crew B</Name></Resource>\
<Resource><UID>3</UID></Resource></Resources>";

const ASSIGNMENTS: &str = "<Assignments>\
<Assignment><TaskUID>1</TaskUID><ResourceUID>2</ResourceUID></Assignment>\
<Assignment><TaskUID>1</TaskUID><ResourceUID>1</ResourceUID></Assignment>\
<Assignment><TaskUID>1</TaskUID><ResourceUID>2</ResourceUID></Assignment>\
<Assignment><TaskUID>2</TaskUID><ResourceUID>3</ResourceUID></Assignment>\
<Assignment><TaskUID>2</TaskUID><ResourceUID>9</ResourceUID></Assignment>\
<Assignment><TaskUID>3</TaskUID><ResourceUID>-65535</ResourceUID></Assignment></Assignments>";

fn project(resources_first: bool) -> String {
    let task = |uid: u32| {
        format!("<Task><UID>{uid}</UID><Name>T{uid}</Name><Duration>PT8H0M0S</Duration></Task>")
    };
    let tail = if resources_first {
        format!("{RESOURCES}{ASSIGNMENTS}")
    } else {
        format!("{ASSIGNMENTS}{RESOURCES}")
    };
    format!(
        "<Project><StartDate>2026-01-07T08:00:00</StartDate><Tasks>{}{}{}</Tasks>{tail}</Project>",
        task(1),
        task(2),
        task(3)
    )
}

fn names(imported: &Imported) -> Vec<Vec<&str>> {
    imported
        .tasks
        .iter()
        .map(|task| task.resource_names.iter().map(String::as_str).collect())
        .collect()
}

#[test]
fn a_task_keeps_the_names_of_its_resources_in_assignment_order() {
    for resources_first in [true, false] {
        let imported = parse_mspdi(&project(resources_first)).expect("imports");
        // Task 2's resources have no name or are not listed, and task 3's is unassigned.
        assert_eq!(
            names(&imported),
            [vec!["Crew B", "Ana"], vec![], vec![]],
            "resources first: {resources_first}"
        );
    }
}
