//! `Session::list_xlsx_sheet` (gs#1875): the visible list as an Excel sheet part.

use crate::common;

use app::dates::Origin;
use common::collapse::{N, session, summaries};
use common::demo_session_seeded;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashMap;

/// A cell of the sheet: its opening tag (`c r="C2" s="1"`), its text, and whether it holds an `<f>`.
#[derive(Default)]
struct Cell {
    tag: String,
    text: String,
    formula: bool,
}

/// The cells of `sheet` by reference (`C2`), parsed as XML: a malformed sheet panics here.
fn read(sheet: &str) -> HashMap<String, Cell> {
    let (mut reader, mut cells, mut at) = (Reader::from_str(sheet), HashMap::new(), String::new());
    loop {
        match reader.read_event().expect("the sheet is well-formed XML") {
            Event::Start(tag) if tag.name().as_ref() == b"c" => {
                let tag = String::from_utf8_lossy(&tag).to_string();
                at = tag.split('"').nth(1).expect("the reference").to_string(); // the first attribute is `r`
                cells.insert(
                    at.clone(),
                    Cell {
                        tag,
                        ..Cell::default()
                    },
                );
            }
            Event::Start(tag) if tag.name().as_ref() == b"f" => {
                cells.entry(at.clone()).or_default().formula = true;
            }
            Event::Text(text) => {
                let text = text.xml10_content().expect("text");
                cells.entry(at.clone()).or_default().text.push_str(&text);
            }
            Event::GeneralRef(name) => {
                let name = name.decode().expect("an entity");
                let text = quick_xml::escape::resolve_predefined_entity(&name);
                cells
                    .entry(at.clone())
                    .or_default()
                    .text
                    .push_str(text.expect("predefined"));
            }
            Event::Eof => return cells,
            _ => {}
        }
    }
}

/// The cells under the header `name`, one per row.
fn column(cells: &HashMap<String, Cell>, name: &str) -> Vec<(String, String)> {
    let head = (b'A'..=b'Z')
        .map(char::from)
        .find(|c| cells.get(&format!("{c}1")).is_some_and(|h| h.text == name));
    let head = head.expect("a column");
    (2..=cells.len())
        .map_while(|row| cells.get(&format!("{head}{row}")))
        .map(|c| (c.tag.clone(), c.text.clone()))
        .collect()
}

#[test]
fn a_name_with_markup_and_a_control_character_is_valid_xml_without_the_control_character() {
    let (session, _) = demo_session_seeded(3, |meta| meta[0].name = "a<b>&\"c\"\u{1}".into());
    let sheet = session.list_xlsx_sheet();
    assert_eq!(column(&read(&sheet), "Task")[0].1, "a<b>&\"c\"", "{sheet}");
}

#[test]
fn a_name_that_starts_with_an_equals_sign_is_an_inline_string_and_no_formula() {
    let (session, _) = demo_session_seeded(3, |meta| meta[0].name = "=1+1".into());
    let cells = read(&session.list_xlsx_sheet());
    let (tag, text) = column(&cells, "Task").remove(0);
    assert!(tag.contains("t=\"inlineStr\""), "{tag}");
    assert_eq!(text, "=1+1");
    assert!(cells.values().all(|cell| !cell.formula));
}

#[test]
fn a_start_date_is_an_excel_serial_day_with_the_time_as_its_fraction() {
    let (session, _) = demo_session_seeded(3, |_| {});
    let listed = session.list_text();
    let row = listed
        .lines()
        .skip(1)
        .position(|line| line.contains("\t2026-01-05 08:00\t"));
    let (tag, text) = column(&read(&session.list_xlsx_sheet()), "Start").remove(row.unwrap());
    assert_eq!(
        (tag.contains(" s=\"1\""), tag.contains(" t="), text.as_str()),
        (true, false, "46027.333333333336")
    );
}

#[test]
fn a_date_before_excels_first_counted_day_stays_text() {
    let (session, _) = demo_session_seeded(3, |_| {});
    let cells = read(
        &session
            .with_origin(Origin::from_day(-40_000))
            .list_xlsx_sheet(),
    );
    let (tag, text) = column(&cells, "Start").remove(0);
    assert!(
        text.starts_with("18") && tag.contains("t=\"inlineStr\""),
        "{tag} {text}"
    );
}

#[test]
fn the_rows_are_the_visible_rows_in_table_order_under_collapse_and_a_filter() {
    let all = session(N).list_text().lines().count();
    let collapsed = session(N);
    let first = summaries(&collapsed)[0];
    let collapsed = collapsed.with_collapsed(&[first]);
    let mut filtered = session(N);
    assert!(filtered.set_critical_only(true));
    for (shown, fewer) in [(session(N), false), (collapsed, true), (filtered, true)] {
        let listed = shown.list_text();
        let lines: Vec<Vec<&str>> = listed.lines().map(|l| l.split('\t').collect()).collect();
        assert_eq!(
            lines.len() < all,
            fewer,
            "collapse and the filter hide rows"
        );
        let at = lines[0]
            .iter()
            .position(|head| *head == "Task")
            .expect("Task");
        let names: Vec<&str> = lines[1..].iter().map(|line| line[at]).collect();
        let sheet: Vec<String> = column(&read(&shown.list_xlsx_sheet()), "Task")
            .into_iter()
            .map(|c| c.1)
            .collect();
        assert_eq!(sheet, names, "the same rows in the same order");
    }
}
