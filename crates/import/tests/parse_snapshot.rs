//! The reader's output must not change when its speed does (gantt#1337). Every MS Project XML
//! fixture in the repository and a few inputs that stress the XML layer
//! (entities, CDATA, prefixes, comments, errors) are read, and a hash of the full `Debug` output
//! is compared with `tests/fixtures/parse_snapshot.txt`, written by the reader before the change.
//! `UPDATE_PARSE_SNAPSHOT=1 cargo test -p import --test parse_snapshot` rewrites it.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const SNAPSHOT: &str = "tests/fixtures/parse_snapshot.txt";

/// FNV-1a over the bytes: stable across runs and toolchains, unlike `DefaultHasher`.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn xml_files(directory: &Path, found: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(directory).unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.extension().is_some_and(|extension| extension == "xml") {
            found.push(path);
        }
    }
}

/// The snapshot names its files with `/`, whatever separator the system's paths use.
fn slash_name(name: &str) -> String {
    name.replace('\\', "/").replace("/../", "/")
}

fn cases() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    xml_files(&root.join("import/tests/fixtures"), &mut files);
    xml_files(&root.join("app/tests/fixtures"), &mut files);
    xml_files(&root.join("../packages/gantt/tests/fixtures"), &mut files);
    files.sort();
    let mut cases: Vec<(String, String)> = files
        .iter()
        .map(|path| {
            let name = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .display()
                .to_string();
            (
                slash_name(&name),
                fs::read_to_string(path).expect("a fixture should be readable"),
            )
        })
        .collect();
    let inline = |name: &str, xml: &str| (format!("inline:{name}"), xml.to_owned());
    cases.push(inline(
        "prefix-comment-entities",
        "<!-- c --><m:Project xmlns:m=\"x\"><m:StartDate> 2026-01-05T08:00:00 </m:StartDate>\
         <m:Tasks><m:Task><m:UID>1</m:UID><m:Name>A &#38; &lt;B&gt; &#x41;<!-- n -->C</m:Name>\
         <m:OutlineLevel>1</m:OutlineLevel><m:Duration>P0DT8H0M0S</m:Duration></m:Task></m:Tasks>\
         </m:Project>",
    ));
    cases.push(inline(
        "crlf-and-unread-elements",
        "<Project><Tasks><Task><UID>1</UID><Name>a\r\nb</Name><Extra><Name>x</Name>\
         <UID>9</UID></Extra><Duration>PT8H0M0S</Duration></Task></Tasks></Project>",
    ));
    let links =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/links.xml"));
    cases.push(inline(
        "byte-order-mark",
        &format!(
            "\u{feff}{}",
            links.expect("tests/fixtures/links.xml should be readable")
        ),
    ));
    cases.push(inline(
        "empty-elements",
        "<Project><Tasks><Task/><Task></Task></Tasks></Project>",
    ));
    cases.push(inline(
        "unknown-entity",
        "<Project><Name>&nope;</Name></Project>",
    ));
    cases.push(inline("mismatch", "<Project><Tasks></Project>"));
    cases.push(inline("not-project", "<Plan/>"));
    cases.push(inline("trailing", "<Project/><Project/>"));
    cases.push(inline(
        "bad-uid",
        "<Project><Tasks><Task><UID>x</UID></Task></Tasks></Project>",
    ));
    cases.push(inline("empty", ""));
    cases
}

fn snapshot() -> String {
    let mut text = String::new();
    for (name, xml) in cases() {
        let output = format!("{:?}", import::parse_mspdi(&xml));
        writeln!(text, "{:016x} {} {name}", fnv(&output), output.len())
            .expect("a String takes writes");
    }
    text
}

#[test]
fn reader_output_matches_the_snapshot() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT);
    let current = snapshot();
    if std::env::var_os("UPDATE_PARSE_SNAPSHOT").is_some() {
        fs::write(&path, &current).unwrap();
    }
    let hash_of = |name: &str| {
        let line = current.lines().find(|line| line.ends_with(name));
        line.and_then(|line| line.split(' ').next())
            .unwrap()
            .to_owned()
    };
    // A byte order mark changes nothing: the file reads as it does without one.
    assert_eq!(
        hash_of("inline:byte-order-mark"),
        hash_of("import/tests/fixtures/links.xml")
    );
    let expected = fs::read_to_string(&path).expect("snapshot file");
    assert_eq!(current, expected);
}

#[test]
fn file_names_use_slashes_on_every_system() {
    assert_eq!(
        slash_name("..\\packages\\gantt\\links.xml"),
        "../packages/gantt/links.xml"
    );
}
