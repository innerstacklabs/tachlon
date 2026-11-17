//! Self-tests of the module-specifier reader of check 12: the import forms it reads and the
//! specifiers it allows.

use super::super::specifiers::{css_imports, html_refs, module_specifiers};
use super::*;

#[test]
fn every_import_form_is_read() {
    let text = "import a from \"one\";\nimport type { B } from 'two';\nimport \"three\";\nimport {\n  c,\n  d as e,\n} from \"four\";\nexport * from \"five\";\nexport * as ns from \"six\";\nexport { f } from \"seven\";\nexport type { G } from \"eight\";\nconst m = await import(\"nine\");\nconst r = require(\"ten\");\nimport z = require(\"eleven\");\nimport * as all from \"twelve\";\nimport def, { h } from \"thirteen\";\n";
    let specs: Vec<_> = module_specifiers(text)
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    assert_eq!(
        specs,
        [
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
            "eleven", "twelve", "thirteen"
        ]
    );
    let lines: Vec<_> = module_specifiers(text)
        .into_iter()
        .map(|(l, _)| l)
        .collect();
    assert_eq!(&lines[..4], &[1, 2, 3, 7]);
}

#[test]
fn text_that_only_looks_like_an_import_is_not_read() {
    let text = "const a = Array.from(\"x\");\nimport.meta.url;\nexport const from = 1;\nexport function f() { return g(\"q\"); }\nobj.import(\"p\");\nconst important = \"z\";\nimport(name);\nimport(`./${x}`);\nconsole.log(\"import failed\");\n";
    assert_eq!(
        module_specifiers(text),
        vec![],
        "{:?}",
        module_specifiers(text)
    );
}

#[test]
fn the_allowed_imports_pass_and_the_rest_fail() {
    let deps: BTreeSet<String> = ["vite", "@playwright/test", "gantt-core"]
        .map(String::from)
        .into();
    let importer = "apps/hosted/src/main.ts";
    for ok in [
        "gantt-core",
        "gantt-core/gantt.css",
        "./samples",
        "../src/samples.js",
        "../vite.config",
        "node:fs",
        "vite",
        "vite/client",
        "@playwright/test",
        "./style.css?inline",
        "/src/main.ts",
    ] {
        assert_eq!(
            bad_specifier("apps/hosted", importer, ok, &deps),
            None,
            "{ok}"
        );
    }
    for bad in [
        "gantt-core/src/index",
        "gantt-core/dist-lib/index.js",
        "@gantt/chart",
        concat!("../../../", web_root!(), "/src/index.ts"),
        "../../x",
        "../../../crates/app/src/lib.rs",
        "../../../demos/50k/x.mjs",
        concat!("/", web_root!(), "/src/main.ts"),
        "./wasm/app.js",
        "left-pad",
        "@scope/pkg",
    ] {
        assert!(
            bad_specifier("apps/hosted", importer, bad, &deps).is_some(),
            "{bad}"
        );
    }
    let undeclared = bad_specifier("apps/hosted", importer, "left-pad", &deps).expect("fails");
    assert!(
        undeclared.contains("a package apps/hosted/package.json does not declare"),
        "{undeclared}"
    );
}

#[test]
fn the_config_files_are_read_too() {
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/vite.config.ts",
        concat!("import x from \"../../", web_root!(), "/vite.config\";\n"),
    );
    one(
        &check_tree(&t),
        concat!(
            "apps/hosted/vite.config.ts:6 imports ../../",
            web_root!(),
            "/vite.config;"
        ),
    );
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/playwright.config.ts",
        "import y from \"nope\";\n",
    );
    one(
        &check_tree(&t),
        "apps/hosted/playwright.config.ts:3 imports nope, a package",
    );
}

#[test]
fn a_package_the_manifest_declares_passes() {
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/src/main.ts",
        "import pad from \"left-pad\";\n",
    );
    assert_eq!(check_tree(&t), [] as [String; 0]);
    t.hosted.retain(|f| f.path != "apps/hosted/package.json");
    let failures = check_tree(&t);
    one(&failures, "apps/hosted/package.json: not found");
    one(
        &failures,
        "apps/hosted/src/main.ts:4 imports left-pad, a package",
    );
}

#[test]
fn the_page_references_are_read() {
    let refs = html_refs(PAGE_HTML);
    assert_eq!(
        refs,
        [(4, "data:,".to_string()), (12, "/src/main.ts".to_string())]
    );
    let page = concat!(
        "<!-- <script src=\"gone.js\"></script> -->\n<script\n  src='../../",
        web_root!(),
        "/src/main.ts'\n  type=module></script>\n<link href=../x.css rel=stylesheet>\n<script type=\"module\">\nimport \"../../",
        web_root!(),
        "/inline.js\";\n</script>\n<a href=\"../skip\">a</a>\n<script data-src=\"no\"></script>\n"
    );
    assert_eq!(
        html_refs(page),
        [
            (
                3,
                concat!("../../", web_root!(), "/src/main.ts").to_string()
            ),
            (5, "../x.css".to_string()),
            (7, concat!("../../", web_root!(), "/inline.js").to_string()),
        ]
    );
}

#[test]
fn the_host_page_may_not_leave_the_hosted_app() {
    let mut t = tree();
    edit(
        &mut t,
        "apps/hosted/index.html",
        "src=\"/src/main.ts\"",
        concat!("src=\"../../", web_root!(), "/src/main.ts\""),
    );
    one(
        &check_tree(&t),
        concat!(
            "apps/hosted/index.html:12 imports ../../",
            web_root!(),
            "/src/main.ts; apps/hosted imports only gantt-core"
        ),
    );
    let mut t = tree();
    edit(
        &mut t,
        "apps/hosted/index.html",
        "href=\"data:,\"",
        concat!("href=\"../../", web_root!(), "/favicon.ico\""),
    );
    one(
        &check_tree(&t),
        concat!(
            "apps/hosted/index.html:4 imports ../../",
            web_root!(),
            "/favicon.ico;"
        ),
    );
    let mut t = tree();
    edit(
        &mut t,
        "apps/hosted/index.html",
        "href=\"data:,\"",
        "href=\"https://example.com/a.css\"",
    );
    assert_eq!(check_tree(&t), [] as [String; 0]);
    t.hosted.retain(|f| f.path != "apps/hosted/index.html");
    one(&check_tree(&t), "apps/hosted/index.html: not found");
}

/// Round 1, finding 2: `import.meta.glob` loads modules, so its patterns are specifiers.
#[test]
fn import_meta_glob_patterns_are_read() {
    let text = concat!(
        "const a = import.meta.glob(\"./a/*.ts\");\nconst b = import.meta.glob([\n  \"../../../",
        web_root!(),
        "/src/*.ts\",\n  '!./c.ts',\n]);\nconst d = import.meta.globEager('./d.ts', { eager: true });\nconst e = import.meta.glob(pattern);\nconst f = import.meta.url;\n"
    );
    assert_eq!(
        module_specifiers(text),
        [
            (1, "./a/*.ts".to_string()),
            (
                3,
                concat!("../../../", web_root!(), "/src/*.ts").to_string()
            ),
            (4, "!./c.ts".to_string()),
            (6, "./d.ts".to_string()),
        ]
    );
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/src/main.ts",
        concat!(
            "const m = import.meta.glob(\"../../../",
            web_root!(),
            "/src/*.ts\");\n"
        ),
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        concat!(
            "apps/hosted/src/main.ts:4 imports ../../../",
            web_root!(),
            "/src/*.ts; apps/hosted imports only gantt-core"
        ),
    );
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/src/main.ts",
        "const m = import.meta.glob(\"./samples/*.ts\");\n",
    );
    assert_eq!(check_tree(&t), [] as [String; 0]);
}

/// `import.meta.glob<string>(...)` is the same call with a type argument: the reader skips a
/// balanced `<...>` (nested, with `=>` inside, or with a quoted `>`) and still reads the patterns.
#[test]
fn import_meta_glob_with_a_type_argument_is_read() {
    let text = concat!(
        "const a = import.meta.glob<string>(\"./a/*.ts\");\nconst b = import.meta.glob<Record<string, string>>([\n  \"../../../",
        web_root!(),
        "/src/*.ts\",\n  '!./c.ts',\n]);\nconst d = import.meta.globEager <() => void>('./d.ts', { eager: true });\nconst e = import.meta.glob<\"a>b\">(\"./e.ts\");\nconst f = import.meta.glob<string>(pattern);\n"
    );
    assert_eq!(
        module_specifiers(text),
        [
            (1, "./a/*.ts".to_string()),
            (
                3,
                concat!("../../../", web_root!(), "/src/*.ts").to_string()
            ),
            (4, "!./c.ts".to_string()),
            (6, "./d.ts".to_string()),
            (7, "./e.ts".to_string()),
        ]
    );
    // Text that is not a call with a closed type argument reads nothing.
    let text = "const a = import.meta.glob<string(\"./a.ts\");\nconst b = import.meta.glob < x;\n";
    assert_eq!(module_specifiers(text), [] as [(usize, String); 0]);
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/src/main.ts",
        concat!(
            "const m = import.meta.glob<string>(\"../../../",
            web_root!(),
            "/src/*.ts\");\n"
        ),
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        concat!(
            "apps/hosted/src/main.ts:4 imports ../../../",
            web_root!(),
            "/src/*.ts; apps/hosted imports only gantt-core"
        ),
    );
    let mut t = tree();
    append(
        &mut t,
        "apps/hosted/src/main.ts",
        "const m = import.meta.glob<string>(\"./samples/*.ts\");\n",
    );
    assert_eq!(check_tree(&t), [] as [String; 0]);
}

/// Round 1, finding 3: a CSS `@import` loads a file, so its target is a specifier.
#[test]
fn css_imports_are_read() {
    let css = "@import \"./a.css\";\n/* @import \"gone.css\"; */\n@IMPORT url(\"./b.css\") layer(x);\n@import url(../c.css);\n@import 'd.css' screen;\n.a { color: red; }\n";
    assert_eq!(
        css_imports(css),
        [
            (1, "./a.css".to_string()),
            (3, "./b.css".to_string()),
            (4, "../c.css".to_string()),
            (5, "d.css".to_string()),
        ]
    );
    for (path, import) in [
        (
            "apps/hosted/src/host.css",
            concat!(
                "@import \"../../../",
                web_root!(),
                "/src/styles/base.css\";\n"
            ),
        ),
        (
            "apps/hosted/src/host.css",
            concat!(
                "@import url(../../../",
                web_root!(),
                "/src/styles/base.css);\n"
            ),
        ),
    ] {
        let mut t = tree();
        t.hosted.push(file(path, &format!(".a {{}}\n{import}")));
        let failures = check_tree(&t);
        assert_eq!(failures.len(), 1, "{import}: {failures:#?}");
        one(
            &failures,
            concat!(
                "apps/hosted/src/host.css:2 imports ../../../",
                web_root!(),
                "/src/styles/base.css; apps/hosted imports only gantt-core"
            ),
        );
    }
    let mut t = tree();
    edit(
        &mut t,
        "apps/hosted/index.html",
        "  body { margin: 0; }",
        concat!(
            "  @import \"../../",
            web_root!(),
            "/src/styles/base.css\";\n  body { margin: 0; }"
        ),
    );
    one(
        &check_tree(&t),
        concat!(
            "apps/hosted/index.html:6 imports ../../",
            web_root!(),
            "/src/styles/base.css;"
        ),
    );
    let mut t = tree();
    t.hosted.push(file(
        "apps/hosted/src/host.css",
        "@import \"./other.css\";\n@import url(\"https://fonts.example/a.css\");\n@import \"left-pad/x.css\";\n",
    ));
    assert_eq!(check_tree(&t), [] as [String; 0]);
}
