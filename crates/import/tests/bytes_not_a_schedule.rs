//! A file that is not a schedule says what it is and what to open, never an XML parser message.

use import::{ProjectSelection, parse_bytes};

const OPEN: &str = "not a schedule; open a P6 XER file or an MS Project XML file";

fn refusal(bytes: &[u8]) -> String {
    let error = parse_bytes(bytes, &ProjectSelection::default()).expect_err("not a schedule");
    error.to_string()
}

#[test]
fn a_pdf_says_it_is_a_pdf_file() {
    assert_eq!(
        refusal(b"%PDF-1.7\n1 0 obj"),
        format!("this is a PDF file, {OPEN}")
    );
}

#[test]
fn a_zip_says_it_is_a_zip_archive() {
    let text = refusal(b"PK\x03\x04\x14\x00\x06\x00");
    assert_eq!(
        text,
        format!(
            "this is a ZIP archive (an .xlsx file, or a zipped schedule: unzip it first), {OPEN}"
        )
    );
}

#[test]
fn an_xml_file_with_another_root_names_the_root() {
    let text = refusal(b"<APIBusinessObjects/>");
    assert_eq!(
        text,
        format!("this is an XML file whose root is APIBusinessObjects, {OPEN}")
    );
    assert!(!text.contains("XML error"));
}

#[test]
fn a_long_root_name_is_cut_at_40_characters() {
    let text = refusal(format!("<{}/>", "R".repeat(60)).as_bytes());
    assert_eq!(
        text,
        format!(
            "this is an XML file whose root is {}, {OPEN}",
            "R".repeat(40)
        )
    );
}
