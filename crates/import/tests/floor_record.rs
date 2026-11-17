use import::{Imported, parse_mspdi};

fn project(tasks: &str) -> Imported {
    let xml = format!(
        "<Project xmlns=\"http://schemas.microsoft.com/project\"><StartDate>2026-01-07T08:00:00</StartDate>\
<Tasks>{tasks}</Tasks></Project>"
    );
    parse_mspdi(&xml).expect("the project should import")
}

fn task(uid: u32, constraint: &str) -> String {
    format!(
        "<Task><UID>{uid}</UID><Name>T{uid}</Name><Duration>PT8H0M0S</Duration>{constraint}</Task>"
    )
}

#[test]
fn a_task_with_no_constraint_has_its_project_start_as_floor() {
    let imported = project(&task(1, ""));
    assert!(
        imported.project_start.is_some(),
        "the fixture states a start"
    );
    assert_eq!(imported.tasks[0].floor, imported.project_start);
}

#[test]
fn a_later_constraint_stays_above_its_floor() {
    let later =
        "<ConstraintType>4</ConstraintType><ConstraintDate>2026-01-08T08:00:00</ConstraintDate>";
    let imported = project(&task(1, later));
    let snet = imported.schedule.tasks()[0].snet;
    assert!(imported.tasks[0].floor.is_some(), "the task has a floor");
    assert!(
        snet > imported.tasks[0].floor,
        "{snet:?} is above the floor"
    );
}
