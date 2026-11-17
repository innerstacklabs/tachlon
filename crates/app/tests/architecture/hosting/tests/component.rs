//! Self-tests of the component scan of check 12: which files it reads and which names it refuses.

use super::*;

#[test]
fn the_component_scan_fails_when_it_reads_fewer_files_than_it_lists() {
    let mut t = tree();
    t.component
        .retain(|f| f.path != concat!(web_root!(), "/index.html"));
    one(
        &check_tree(&t),
        concat!(
            "the component scan found no file in ",
            web_root!(),
            "/index.html"
        ),
    );
    let mut t = tree();
    t.component
        .retain(|f| !f.path.starts_with(concat!(web_root!(), "/src/")));
    one(
        &check_tree(&t),
        concat!(
            "the component scan found no file in ",
            web_root!(),
            "/src/**"
        ),
    );
    let mut t = tree();
    t.component.clear();
    assert_eq!(check_tree(&t).len(), GROUPS.len());
}

#[test]
fn a_hosting_name_anywhere_in_the_component_fails_comments_included() {
    for (path, text, name) in [
        (
            concat!(web_root!(), "/src/main.ts"),
            "// served from apps/hosted today\n",
            "apps/hosted",
        ),
        (
            concat!(web_root!(), "/src/open.ts"),
            "const q = \"?sample=small\";\n",
            "sample=",
        ),
        (
            concat!(web_root!(), "/tests/open.spec.ts"),
            "goto(\"/samples/a.xer\");\n",
            "samples/",
        ),
        (
            concat!(web_root!(), "/index.html"),
            "<!-- Pages.Dev -->\n",
            "pages.dev",
        ),
        (
            concat!(web_root!(), "/vite.config.ts"),
            "const base = \"/gantt-app/\";\n",
            "gantt-app",
        ),
        (
            concat!(web_root!(), "/package.json"),
            "{ \"a\": \"samples/x\" }\n",
            "samples/",
        ),
        (
            "scripts/build-web.sh",
            "cp x apps/hosted/y\n",
            "apps/hosted",
        ),
        (
            "crates/app/src/lib.rs",
            "// see samples/small.xer\n",
            "samples/",
        ),
        ("crates/app/Cargo.toml", "# apps/hosted\n", "apps/hosted"),
    ] {
        let mut t = tree();
        append(&mut t, path, text);
        let failures = check_tree(&t);
        assert_eq!(failures.len(), 1, "{path}: {failures:#?}");
        assert!(
            failures[0].starts_with(&format!("{path}:"))
                && failures[0].contains(&format!("names {name};")),
            "{failures:#?}"
        );
    }
}

#[test]
fn a_component_import_naming_apps_fails() {
    let mut t = tree();
    append(
        &mut t,
        concat!(web_root!(), "/src/main.ts"),
        "import { x } from \"../../apps/other/x\";\nexport * from \"../../apps/y\";\n",
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 2, "{failures:#?}");
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:3 imports ../../apps/other/x, which names apps/;"
        ),
    );
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:4 imports ../../apps/y, which names apps/;"
        ),
    );
}

/// Round 1, finding 1: a name planted in any file under `packages/gantt/` fails, at any extension.
#[test]
fn a_hosting_name_in_any_file_under_web_fails() {
    for path in [
        concat!(web_root!(), "/tests/support/hosted.js"),
        concat!(web_root!(), "/tests/x.spec.mts"),
        concat!(web_root!(), "/vitest.config.mts"),
        concat!(web_root!(), "/scripts/verify.mjs"),
        concat!(web_root!(), "/.gitignore"),
        concat!(web_root!(), "/tests/fixtures/one.xer"),
        concat!(web_root!(), "/pnpm-lock.yaml"),
    ] {
        let mut t = tree();
        t.component.push(file(
            path,
            "ok\nconst u = \"https://gantt-app.pages.dev\";\n",
        ));
        let failures = check_tree(&t);
        assert_eq!(failures.len(), 2, "{path}: {failures:#?}");
        one(&failures, &format!("{path}:2 names gantt-app;"));
        one(&failures, &format!("{path}:2 names pages.dev;"));
    }
}

/// The walk of `packages/gantt/` reads every file and enters none of the paths `packages/gantt/.gitignore` names.
#[test]
fn the_walk_of_web_reads_every_file_but_the_generated_ones() {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hosting-walk");
    let _ = fs::remove_dir_all(&base);
    let all = [
        concat!(web_root!(), "/.gitignore"),
        concat!(web_root!(), "/pnpm-lock.yaml"),
        concat!(web_root!(), "/scripts/verify.mjs"),
        concat!(web_root!(), "/src/a.ts"),
        concat!(web_root!(), "/tests/fixtures/one.xer"),
        concat!(web_root!(), "/tests/support/hosted.js"),
        concat!(web_root!(), "/tests/x.spec.mts"),
        concat!(web_root!(), "/vitest.config.mts"),
        concat!(web_root!(), "/dist-notes/a.md"),
        concat!(web_root!(), "/src/target/a.ts"),
        concat!(web_root!(), "/src/pkg/a.ts"),
        concat!(web_root!(), "/node_modules/p/i.js"),
        concat!(web_root!(), "/src/wasm/app.js"),
        concat!(web_root!(), "/dist/a.js"),
        concat!(web_root!(), "/test-results/a.txt"),
        concat!(web_root!(), "/playwright-report/index.html"),
    ];
    for rel in all {
        let path = base.join(rel);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, "x\n").expect("a file");
    }
    let mut files = Vec::new();
    walk(&base, web_root!(), &WEB_SKIPPED, &mut files);
    let read: BTreeSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
    let want: BTreeSet<&str> = all[..11].iter().copied().collect();
    assert_eq!(read, want);
}
