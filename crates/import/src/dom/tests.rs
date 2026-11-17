use super::*;

fn parse_tree(xml: &str) -> Result<Node<'_>, ImportError> {
    parse(xml, |_, _| {})
}

#[test]
fn parses_nested_elements_and_helpers() {
    let root = parse_tree("<Project><Name> First </Name><Name>Second</Name></Project>")
        .expect("test XML should parse");

    assert_eq!(root.name, "Project");
    assert_eq!(root.child_text("Name"), Some("First"));
    assert_eq!(root.children_named("Name").count(), 2);
}

#[test]
fn strips_namespace_prefixes() {
    let root = parse_tree("<m:Project><m:Task /></m:Project>").expect("test XML should parse");

    assert_eq!(root.name, "Project");
    assert_eq!(root.children.first().map(|node| node.name), Some("Task"));
}

#[test]
fn combines_cdata_and_entities() {
    let root =
        parse_tree("<Project>A &amp; <![CDATA[B < C]]></Project>").expect("test XML should parse");

    assert_eq!(root.text, "A & B < C");
}

#[test]
fn rejects_mismatched_tag() {
    assert!(matches!(
        parse_tree("<Project><Task></Project>"),
        Err(ImportError::Xml { .. })
    ));
}

#[test]
fn rejects_unclosed_tag() {
    assert!(matches!(
        parse_tree("<Project><Task></Task>"),
        Err(ImportError::Xml { .. })
    ));
}

#[test]
fn rejects_empty_input() {
    assert!(matches!(parse_tree(""), Err(ImportError::Xml { .. })));
}

#[test]
fn rejects_sixty_five_deep_elements() {
    let xml = format!("{}{}", "<x>".repeat(65), "</x>".repeat(65));

    assert_eq!(parse_tree(&xml), Err(ImportError::TooDeep));
}

#[test]
fn ignores_doctype() {
    let root = parse_tree("<!DOCTYPE Project><Project />").expect("test XML should parse");

    assert_eq!(root.name, "Project");
}

#[test]
fn hands_each_task_to_the_sink_and_keeps_none() {
    let mut seen = Vec::new();
    let root = parse(
        "<Project><Tasks><Task><UID>1</UID></Task><Task><UID>2</UID></Task></Tasks>\
         <Tasks><Task><UID>3</UID></Task></Tasks></Project>",
        |_, item| match item {
            Item::Task(node) => seen.push(node.child_text("UID").map(str::to_owned)),
            _ => panic!("only tasks are in this file"),
        },
    )
    .expect("test XML should parse");

    assert_eq!(seen, [Some("1".to_owned()), Some("2".to_owned())]);
    assert!(
        root.child("Tasks")
            .is_some_and(|tasks| tasks.children.is_empty())
    );
    assert_eq!(root.children_named("Tasks").count(), 1);
}
