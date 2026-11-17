//! Self-tests of check 13. Each plants one mistake in a copy of a clean set of tracked files and
//! wants the failure that names file and line. Every fixture builds the old name at run time.

use super::*;
use crate::architecture::web_root::WEB_ROOT;

/// The old name, hyphenated, in lower case. Built apart from `needles()`, so a change to the
/// needles cannot change the fixtures with it.
fn old() -> String {
    format!("{}-{}", "gantt", "spike")
}

/// The old name with underscores, built the same way.
fn old_snake() -> String {
    format!("{}_{}", "gantt", "spike")
}

/// The old name hyphenated, with both words capitalised.
fn old_title() -> String {
    format!("{}-{}", "Gantt", "Spike")
}

/// The second retired word, in lower case. Built apart from `needles()`, like the others.
fn word() -> String {
    format!("{}{}", "man", "tis")
}

/// The second retired word, capitalised.
fn word_title() -> String {
    format!("{}{}", "Man", "tis")
}

fn tracked(path: &str, text: &str) -> Tracked {
    Tracked {
        path: path.to_string(),
        text: Some(text.to_string()),
    }
}

/// A clean set of files, plus `extra`.
fn set(extra: Vec<Tracked>) -> Vec<Tracked> {
    let mut files = vec![
        tracked("CLAUDE.md", "# A title\n"),
        tracked(web!("src/a.ts"), "export const a = 1;\n"),
    ];
    files.extend(extra);
    files
}

/// `check_name` with an empty allowlist: nothing may name the old project.
fn check(files: &[Tracked]) -> Vec<String> {
    check_name(files, &[])
}

/// `check_name` with `listed` as the allowlist.
fn check_listed(files: &[Tracked], listed: &[&str]) -> Vec<String> {
    let listed: Vec<String> = listed.iter().map(ToString::to_string).collect();
    check_name(files, &listed)
}

fn hit(path: &str, line: usize) -> String {
    format!("{path}:{line}: {HINT}")
}

/// The failure for a listed file that no longer names the old project.
fn stale(path: &str) -> String {
    format!(
        "{path}: on the allowlist but names no retired project name; delete the line from {ALLOWLIST}"
    )
}

/// The failure for a listed path that git does not track.
fn untracked(path: &str) -> String {
    format!("{path}: on the allowlist but not tracked; delete or correct the line in {ALLOWLIST}")
}

/// A fresh empty directory under the test target directory.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("name-check-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the scratch directory is writable");
    dir
}

#[test]
fn a_clean_set_passes() {
    assert_eq!(check(&set(vec![])), Vec::<String>::new());
}

#[test]
fn the_name_in_rust_ts_json_and_markdown_fails_with_path_and_line() {
    let name = old();
    let files = set(vec![
        tracked(
            "crates/x/src/lib.rs",
            &format!("//! a\n//! b\n//! The {name} crate.\n"),
        ),
        tracked(
            web!("src/b.ts"),
            &format!("const a = 1;\nconsole.log(\"{name}: ready\");\n"),
        ),
        tracked(
            web!("package.json"),
            &format!("{{\n  \"name\": \"{name}-web\"\n}}\n"),
        ),
        tracked("docs/y.md", &format!("one\n\n\n\nsee {name}\n")),
    ]);
    assert_eq!(
        check(&files),
        vec![
            hit("crates/x/src/lib.rs", 3),
            hit(web!("src/b.ts"), 2),
            hit(web!("package.json"), 2),
            hit("docs/y.md", 5),
        ]
    );
}

#[test]
fn the_underscore_and_title_case_spellings_fail() {
    let files = set(vec![
        tracked("scripts/a.sh", &format!("export X={}\n", old_snake())),
        tracked("docs/b.md", &format!("# {}\n", old_title())),
        tracked(
            web!("src/c.ts"),
            &format!("const {} = 1;\n", old_snake().to_uppercase()),
        ),
    ]);
    assert_eq!(
        check(&files),
        vec![
            hit("scripts/a.sh", 1),
            hit("docs/b.md", 1),
            hit(web!("src/c.ts"), 1)
        ]
    );
}

#[test]
fn the_name_as_the_last_token_of_a_file_with_no_final_newline_fails() {
    let extra = vec![
        tracked(web!("src/one.ts"), &format!("// the project is {}", old())),
        tracked(
            "docs/two.md",
            &format!("a\nb\nthe project is {}", old_title()),
        ),
        tracked("scripts/three.sh", &old_snake()),
    ];
    for file in &extra {
        let text = file.text.as_deref().expect("text");
        assert!(
            !text.ends_with('\n'),
            "{} is a fixture with no final newline",
            file.path
        );
    }
    let files = set(extra);
    assert_eq!(
        check(&files),
        vec![
            hit(web!("src/one.ts"), 1),
            hit("docs/two.md", 3),
            hit("scripts/three.sh", 1)
        ]
    );
}

#[test]
fn a_crlf_file_reports_the_right_line() {
    let files = set(vec![tracked(
        "docs/crlf.md",
        &format!("a\r\nb\r\n{}\r\n", old()),
    )]);
    assert_eq!(check(&files), vec![hit("docs/crlf.md", 3)]);
}

#[test]
fn every_hit_is_listed_not_the_first() {
    let name = old();
    let files = set(vec![
        tracked(
            "docs/a.md",
            &format!("{name}\nclean\n{name} again, {name}\n"),
        ),
        tracked(web!("src/b.ts"), &format!("{name}\n")),
    ]);
    assert_eq!(
        check(&files),
        vec![
            hit("docs/a.md", 1),
            hit("docs/a.md", 3),
            hit(web!("src/b.ts"), 1)
        ]
    );
}

#[test]
fn the_message_points_at_the_rule_and_the_list_and_does_not_hold_a_retired_word() {
    let message = hit("docs/a.md", 4);
    assert!(
        message.starts_with("docs/a.md:4: names a retired project name; see decision 0013"),
        "{message}"
    );
    assert!(message.contains(ALLOWLIST), "{message}");
    assert!(message.ends_with("may end its line with <!-- verbatim -->."));
    let lower = message.to_ascii_lowercase();
    assert!(
        !lower.contains(&old()) && !lower.contains(&old_snake()) && !lower.contains(&word()),
        "{message}"
    );
}

#[test]
fn a_tracked_file_naming_the_second_word_and_not_on_the_allowlist_is_one_failure() {
    let files = set(vec![tracked(
        web!("src/b.ts"),
        &format!("const a = 1;\nconst b = \"{}\";\n", word_title()),
    )]);
    assert_eq!(check(&files), vec![hit(web!("src/b.ts"), 2)]);
}

#[test]
fn the_second_word_fails_in_every_case_and_in_a_scoped_package_name() {
    let (lower, title) = (word(), word_title());
    let files = set(vec![
        tracked(web!("a.ts"), &format!("x {lower}\n")),
        tracked(web!("b.ts"), &format!("x {title}\n")),
        tracked(web!("c.ts"), &format!("x {}\n", lower.to_uppercase())),
        tracked(web!("package.json"), &format!("\"@{lower}/pkg\"\n")),
        tracked(web!("d.css"), &format!(":root {{ --{lower}-x: 1; }}\n")),
    ]);
    assert_eq!(
        check(&files),
        vec![
            hit(web!("a.ts"), 1),
            hit(web!("b.ts"), 1),
            hit(web!("c.ts"), 1),
            hit(web!("package.json"), 1),
            hit(web!("d.css"), 1),
        ]
    );
}

#[test]
fn a_file_whose_path_names_the_second_word_fails_unless_listed() {
    let path = format!("docs/media/{}-icon.png", word());
    let files = set(vec![Tracked {
        path: path.clone(),
        text: None,
    }]);
    assert_eq!(
        check(&files),
        vec![format!(
            "{path}: the file name names the old project name; rename the file"
        )]
    );
    assert_eq!(check_listed(&files, &[path.as_str()]), Vec::<String>::new());
}

#[test]
fn a_listed_file_passes_but_an_unlisted_file_beside_it_fails() {
    let name = old();
    let files = set(vec![
        tracked("docs/notes/0007-other.md", &format!("# 0007\n{name}\n")),
        tracked(
            "docs/notes/0006-listed.md",
            &format!("{name} was the name\nAmended: {name}\n"),
        ),
        tracked(
            "docs/decisions/0005-history.md",
            &format!("# 0005\n{name}\n"),
        ),
    ]);
    assert_eq!(
        check_listed(&files, &["docs/notes/0006-listed.md"]),
        vec![hit("docs/notes/0007-other.md", 2)]
    );
}

#[test]
fn a_listed_file_that_no_longer_names_it_is_a_failure() {
    let files = set(vec![
        tracked(web!("src/clean.ts"), "export const c = 1;\n"),
        tracked(web!("src/dirty.ts"), &format!("// {}\n", word())),
    ]);
    assert_eq!(
        check_listed(&files, &[web!("src/clean.ts"), web!("src/dirty.ts")]),
        vec![stale(web!("src/clean.ts"))]
    );
}

#[test]
fn a_listed_docs_file_with_only_verbatim_lines_is_a_failure() {
    let files = set(vec![tracked(
        "docs/q.md",
        &format!("said {} <!-- verbatim -->\n", word()),
    )]);
    assert_eq!(
        check_listed(&files, &["docs/q.md"]),
        vec![stale("docs/q.md")]
    );
}

#[test]
fn a_listed_path_that_is_not_tracked_is_a_failure() {
    let files = set(vec![tracked(
        web!("src/dirty.ts"),
        &format!("// {}\n", word()),
    )]);
    assert_eq!(
        check_listed(&files, &["gone/after-a-rename.ts", web!("src/dirty.ts")]),
        vec![untracked("gone/after-a-rename.ts")]
    );
}

#[test]
fn an_allowlist_that_is_malformed_fails() {
    let sorted = "a/b.md\nc.md\nd/e.md\n";
    assert_eq!(
        parse_allowlist(sorted),
        Ok(vec![
            "a/b.md".to_string(),
            "c.md".to_string(),
            "d/e.md".to_string()
        ])
    );
    assert_eq!(parse_allowlist(""), Ok(vec![]));
    assert_eq!(
        parse_allowlist("# a comment\na.md\na.md | t # why\n"),
        Ok(vec!["a.md".to_string(), "a.md | t".to_string()])
    );
    for (text, reason) in [
        ("a.md\n\nb.md\n", "a blank line"),
        ("a.md\n b.md\n", "a line with spaces around the path"),
        ("a.md\nb.md \n", "a line with spaces around the path"),
        ("b.md\na.md\n", "not sorted"),
        ("a.md\na.md\n", "a repeated path"),
        ("a.md\nB.md\n", "not sorted: bytewise, capitals first"),
        ("a.md\nA.md\n", "not sorted"),
        ("a.md | text\n", "a text entry with no reason"),
        ("a.md |  # why\n", "a text entry with no text"),
    ] {
        let failure = parse_allowlist(text).expect_err(reason);
        assert!(failure.starts_with(&format!("{ALLOWLIST}:")), "{failure}");
    }
}

#[test]
fn an_allowlist_that_cannot_be_read_fails() {
    let dir = scratch("allowlist-missing");
    let failure = load_allowlist(&dir).expect_err("no such file");
    assert!(
        failure.starts_with(&format!("{ALLOWLIST}: cannot read")),
        "{failure}"
    );
    let path = dir.join(ALLOWLIST);
    fs::create_dir_all(path.parent().expect("a parent")).expect("writable");
    fs::write(&path, b"ok.md\n\xff\xfe\n").expect("writable");
    assert!(load_allowlist(&dir).is_err_and(|e| e.contains("cannot read")));
    fs::write(&path, "a.md\nb.md\n").expect("writable");
    assert_eq!(
        load_allowlist(&dir),
        Ok(vec!["a.md".to_string(), "b.md".to_string()])
    );
}

#[test]
fn a_verbatim_line_in_a_docs_markdown_file_passes() {
    let name = old();
    let files = set(vec![
        tracked(
            "docs/a.md",
            &format!("> \"it was {name}\" <!-- verbatim -->\n"),
        ),
        tracked(
            "docs/deep/er/b.md",
            &format!("said {name} <!-- verbatim -->  \n"),
        ),
        tracked("docs/c.md", &format!("a\nsaid {name} <!-- verbatim -->")),
    ]);
    assert_eq!(check(&files), Vec::<String>::new());
}

#[test]
fn the_marker_exempts_only_its_own_line_and_only_at_its_end() {
    let name = old();
    let files = set(vec![
        tracked("docs/a.md", &format!("<!-- verbatim -->\n{name}\n")),
        tracked("docs/b.md", &format!("{name} <!-- verbatim --> and more\n")),
        tracked(
            "docs/c.md",
            &format!("{name}\n{name} <!-- verbatim -->\n{name}\n"),
        ),
    ]);
    assert_eq!(
        check(&files),
        vec![
            hit("docs/a.md", 2),
            hit("docs/b.md", 1),
            hit("docs/c.md", 1),
            hit("docs/c.md", 3)
        ]
    );
}

#[test]
fn the_marker_outside_docs_markdown_exempts_nothing() {
    let name = old();
    let line = format!("{name} <!-- verbatim -->\n");
    let files = set(vec![
        tracked("CLAUDE.md", &line),
        tracked(web!("src/x.ts"), &line),
        tracked("crates/x/src/lib.rs", &line),
        tracked(web!("package.json"), &line),
        tracked("docs/notes.txt", &line),
        tracked("notdocs/a.md", &line),
        tracked("book/src/a.md", &line),
    ]);
    assert_eq!(
        check(&files),
        vec![
            hit("CLAUDE.md", 1),
            hit(web!("src/x.ts"), 1),
            hit("crates/x/src/lib.rs", 1),
            hit(web!("package.json"), 1),
            hit("docs/notes.txt", 1),
            hit("notdocs/a.md", 1),
            hit("book/src/a.md", 1),
        ]
    );
}

#[test]
fn a_path_that_names_the_old_project_fails() {
    let files = set(vec![tracked(&format!("docs/{}.md", old()), "clean\n")]);
    assert_eq!(
        check(&files),
        vec![format!(
            "docs/{}.md: the file name names the old project name; rename the file",
            old()
        )]
    );
}

#[test]
fn an_empty_list_fails() {
    let failures = check(&[]);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert!(failures[0].contains("listed 0 files"), "{failures:#?}");
}

#[test]
fn a_file_that_is_not_utf8_is_skipped_without_a_panic() {
    let dir = scratch("non-utf8");
    let mut bytes = old().into_bytes();
    bytes.extend([0xff, 0xfe, 0x00, 0x80]);
    fs::write(dir.join("blob.bin"), bytes).expect("writable");
    let (files, unread) = load(&dir, &["blob.bin".to_string()]);
    assert!(unread.is_empty(), "{unread:#?}");
    assert!(files[0].text.is_none());
    assert_eq!(check(&set(files)), Vec::<String>::new());
}

#[test]
fn a_file_git_lists_but_cannot_be_read_fails() {
    let dir = scratch("unreadable");
    let (files, unread) = load(&dir, &["gone.md".to_string()]);
    assert!(files.is_empty());
    assert_eq!(unread.len(), 1, "{unread:#?}");
    assert!(
        unread[0].starts_with("gone.md: cannot read a file git lists:"),
        "{unread:#?}"
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_is_read_as_its_target_path_and_a_dangling_one_does_not_fail_the_read() {
    let dir = scratch("symlink");
    std::os::unix::fs::symlink("../elsewhere/missing.mp4", dir.join("dangling.mp4"))
        .expect("symlinks work");
    let target = format!("../{}/media.mp4", old());
    std::os::unix::fs::symlink(&target, dir.join("named.mp4")).expect("symlinks work");
    let paths = ["dangling.mp4".to_string(), "named.mp4".to_string()];
    let (files, unread) = load(&dir, &paths);
    assert!(unread.is_empty(), "{unread:#?}");
    assert_eq!(check(&set(files)), vec![hit("named.mp4", 1)]);
}

#[test]
fn the_listing_splits_on_nul_and_names_a_path_that_is_not_utf8() {
    let (paths, failures) = parse_listing(b"a.md\0dir/b.ts\0");
    assert_eq!(paths, vec!["a.md".to_string(), "dir/b.ts".to_string()]);
    assert_eq!(failures, [] as [String; 0]);
    let (paths, failures) = parse_listing(b"ok.md\0bad\xff.md\0");
    assert_eq!(paths, vec!["ok.md".to_string()]);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert!(failures[0].ends_with("the path is not UTF-8; this check cannot read it"));
    assert_eq!(parse_listing(b""), (vec![], vec![]));
}

#[test]
fn a_git_that_is_missing_or_exits_non_zero_is_an_error() {
    let dir = scratch("git");
    let missing = listing("no-such-program-for-the-name-check", &["ls-files"], &dir);
    assert!(missing.is_err_and(|e| e.contains("cannot run")));
    let failed = listing(
        "git",
        &["ls-files", "--no-such-flag-for-the-name-check"],
        &dir,
    );
    assert!(failed.is_err_and(|e| e.starts_with("git ls-files --no-such-flag")));
    assert!(listing("git", &["--version"], &dir).is_ok());
}

#[test]
fn a_repository_whose_listing_fails_reports_one_failure_and_scans_nothing() {
    let dir = scratch("no-directory").join("absent");
    let failures = check_repository(&dir);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert!(failures[0].contains("cannot run"), "{failures:#?}");
}

/// The product's name, capitalised. Built at run time, so this file does not name it.
fn product() -> String {
    format!("{}{}", "Tach", "lon")
}

#[test]
fn a_line_entry_exempts_its_exact_text_and_nothing_else_on_the_file() {
    let infra = format!("{}-gantt-book", word());
    let files = set(vec![tracked(
        ".github/workflows/ci.yml",
        &format!(
            "run: deploy --project-name {infra}\nname: {}\n",
            word_title()
        ),
    )]);
    let entry = format!(".github/workflows/ci.yml | {infra}");
    assert_eq!(
        check_listed(&files, &[entry.as_str()]),
        vec![hit(".github/workflows/ci.yml", 2)]
    );
    let gone = format!(".github/workflows/ci.yml | {}-old", word());
    assert_eq!(
        check_listed(&files, &[entry.as_str(), gone.as_str()]),
        vec![
            hit(".github/workflows/ci.yml", 2),
            format!(
                "{gone}: on the allowlist but no line holds that text; delete the line from {ALLOWLIST}"
            ),
        ]
    );
}

#[test]
fn the_product_name_outside_the_brand_files_fails() {
    let name = product();
    let line = format!("the {name} app\n");
    let files = set(vec![
        tracked("README.md", &line),
        tracked("book/src/index.md", &line),
        tracked("docs/PRODUCT.md", &line),
        tracked("docs/decisions/0013-x.md", &line),
        tracked(web!("src/product.ts"), &line),
        tracked(web!("index.html"), &line),
        tracked(web!("src/tab.ts"), &format!("a\n{}\n", name.to_lowercase())),
        tracked("docs/ARCHITECTURE.md", &line),
        tracked(
            &format!("{WEB_ROOT}/src/{}.css", name.to_lowercase()),
            "a\n",
        ),
    ]);
    let brand = |at: &str| format!("{at}: {BRAND_HINT}");
    assert_eq!(
        check(&files),
        vec![
            brand(web!("src/tab.ts:2")),
            brand("docs/ARCHITECTURE.md:1"),
            brand(&format!("{WEB_ROOT}/src/{}.css", name.to_lowercase())),
        ]
    );
}

#[test]
fn the_product_name_under_examples_passes_and_the_same_import_elsewhere_fails() {
    let name = product().to_lowercase();
    let line = format!("import {{ Gantt }} from \"{name}/react\";\n");
    let files = set(vec![
        tracked("examples/react/src/main.tsx", &line),
        tracked(web!("src/tab.tsx"), &line),
        tracked("a/examples/x.md", &line),
    ]);
    let brand = |at: &str| format!("{at}: {BRAND_HINT}");
    assert_eq!(
        check(&files),
        vec![brand(web!("src/tab.tsx:1")), brand("a/examples/x.md:1")]
    );
}
