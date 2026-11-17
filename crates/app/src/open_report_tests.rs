use super::open_report;
use crate::{OpenError, load_bytes};
use import::ProjectSelection::Unspecified;

fn oracle(bytes: &[u8]) -> Result<crate::Loaded, OpenError> {
    load_bytes(bytes, &Unspecified, &|| 0.0)
}

fn refused_with(bytes: &[u8]) {
    let report = open_report(bytes);
    let Err(OpenError::Message(sentence)) = oracle(bytes) else {
        panic!("the importer must refuse")
    };
    assert_eq!(report.verdict, "refused");
    assert_eq!(report.refusal.as_deref(), Some(sentence.as_str()));
    assert_ne!(sentence, "");
}

#[test]
fn every_fixture_reads_as_load_bytes_does() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/gantt/tests/fixtures"
    );
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("xer" | "xml")
        ) {
            continue;
        }
        seen += 1;
        let bytes = std::fs::read(&path).unwrap();
        let report = open_report(&bytes);
        let name = path.display();
        match oracle(&bytes) {
            Ok(loaded) => {
                let verdict = if loaded.unsupported.is_empty() {
                    "opened"
                } else {
                    "opened-with-notes"
                };
                assert_eq!(report.verdict, verdict, "{name}");
                assert_eq!(report.tasks, loaded.schedule.tasks().len(), "{name}");
                assert_eq!(
                    report.links,
                    loaded.schedule.live_dependencies().count(),
                    "{name}"
                );
                assert_eq!(report.notes, loaded.unsupported, "{name}");
            }
            Err(OpenError::ChooseProject(_)) => {
                assert_eq!(report.verdict, "choose-project", "{name}");
            }
            Err(OpenError::Message(_)) => assert_eq!(report.verdict, "refused", "{name}"),
        }
        let line = report.to_string();
        assert!(line.starts_with(report.verdict), "{name}: {line}");
        assert!(
            line.contains(&format!("tasks={}", report.tasks)),
            "{name}: {line}"
        );
        assert!(
            line.contains(&format!("links={}", report.links)),
            "{name}: {line}"
        );
    }
    assert_eq!(seen, 9);
}

#[test]
fn two_projects_wait_for_a_choice() {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/gantt/tests/fixtures/two-projects.xer"
    ))
    .unwrap();
    assert_eq!(open_report(&bytes).verdict, "choose-project");
}

#[test]
fn broken_inputs_are_refused_with_the_importers_sentence() {
    refused_with(b"<Project><Tasks><Task><UID>1</UID><Na");
    refused_with(b"ERMHDR\t20.1\t2024-01-01\tProject");
    refused_with(b"");
    refused_with(b"<?xml version=\"1.0\"?><Project xmlns=\"http://schemas.microsoft.com/project\"><Tasks/></Project>");
}
