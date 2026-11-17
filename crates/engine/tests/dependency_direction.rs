//! The engine depends on neither `timeline` nor `app`.
//!
//! Cargo already refuses a cycle in normal dependencies (`timeline` and `app`
//! both depend on `engine`), but it allows one through `[dev-dependencies]`,
//! so a test helper could pull the renderer into the engine unnoticed. This
//! test reads `crates/engine/Cargo.toml` and fails if any dependency table
//! (normal, dev, build, target-specific, or the `[dependencies.<name>]` form)
//! names either crate, by key, by `package = "..."` rename, or by path.
//!
//! It reads text, not `cargo metadata`. A rename inherited from the workspace
//! (`viz.workspace = true`, where the root `[workspace.dependencies]` points
//! `viz` at `crates/timeline`) is not caught.

/// Workspace crates the engine must not depend on.
const FORBIDDEN: [&str; 2] = ["timeline", "app"];

/// Names from `FORBIDDEN` that `manifest` depends on, one entry per finding.
fn forbidden_deps(manifest: &str) -> Vec<String> {
    let mut found = Vec::new();
    // The dependency name a `[dependencies.<name>]` section stands for.
    let mut table_dep: Option<String> = None;
    let mut in_deps = false;
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_end_matches(']').trim_matches('[').trim();
            table_dep = None;
            in_deps = false;
            // `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`,
            // `[target.'cfg(..)'.dependencies]`: the last dotted segment ends
            // in `dependencies`. `[dependencies.timeline]` names the crate.
            let segments: Vec<&str> = header.rsplitn(2, '.').collect();
            if segments[0].ends_with("dependencies") {
                in_deps = true;
            } else if segments.len() == 2 && segments[1].ends_with("dependencies") {
                table_dep = Some(segments[0].trim_matches(['"', '\'']).to_string());
            }
            if let Some(name) = &table_dep
                && FORBIDDEN.contains(&name.as_str())
            {
                found.push(format!("[{header}]"));
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches(['"', '\'']);
        let value = value.trim();
        if table_dep.is_some() {
            // Inside `[dependencies.<name>]`: only `package` and `path` matter.
            if renames_to_forbidden(key, value) {
                found.push(line.to_string());
            }
            continue;
        }
        if !in_deps {
            continue;
        }
        // `viz.path = "../timeline"`: the first dotted segment is the
        // dependency name, the last is the table key it sets.
        let name = key.split('.').next().unwrap_or(key);
        let field = key.rsplit('.').next().unwrap_or(key);
        if FORBIDDEN.contains(&name)
            || value_names_forbidden(value)
            || renames_to_forbidden(field, value)
        {
            found.push(line.to_string());
        }
    }
    found
}

/// `package = "timeline"` as a table key.
fn renames_to_forbidden(key: &str, value: &str) -> bool {
    (key == "package"
        && FORBIDDEN
            .iter()
            .any(|f| value.trim().trim_matches(['"', '\'']) == *f))
        || (key == "path" && path_is_forbidden(value))
}

/// An inline table (`{ path = "../timeline" }` or `{ package = "app" }`).
fn value_names_forbidden(value: &str) -> bool {
    let Some(inner) = value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) else {
        return false;
    };
    inner.split(',').any(|part| {
        part.split_once('=')
            .is_some_and(|(k, v)| renames_to_forbidden(k.trim(), v.trim()))
    })
}

/// A path value whose last component is a forbidden crate directory.
fn path_is_forbidden(value: &str) -> bool {
    let path = value.trim().trim_matches(['"', '\'']);
    let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    FORBIDDEN.contains(&last)
}

#[test]
fn engine_depends_on_neither_timeline_nor_app() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest = std::fs::read_to_string(path).expect("the engine manifest is readable");
    assert!(
        manifest.contains("name = \"engine\""),
        "{path} is not the engine manifest"
    );
    assert!(
        manifest.contains("[dependencies]"),
        "{path} has no [dependencies] table; the parser would pass on nothing"
    );
    let found = forbidden_deps(&manifest);
    assert!(
        found.is_empty(),
        "crates/engine must not depend on timeline or app (docs/ARCHITECTURE.md): {found:?}"
    );
}

#[test]
fn the_parser_finds_every_way_to_name_a_forbidden_crate() {
    let cases = [
        ("[dependencies]\ntimeline = { path = \"../timeline\" }\n", 1),
        ("[dev-dependencies]\napp = { path = \"../app\" }\n", 1),
        ("[build-dependencies]\ntimeline.workspace = true\n", 1),
        (
            "[target.'cfg(target_arch = \"wasm32\")'.dependencies]\napp = \"0.1\"\n",
            1,
        ),
        ("[dependencies.timeline]\npath = \"../timeline\"\n", 2),
        (
            "[dependencies]\nviz = { package = \"timeline\", version = \"0.1\" }\n",
            1,
        ),
        ("[dependencies]\nviz = { path = \"../timeline/\" }\n", 1),
        ("[dev-dependencies]\nviz = { path = \"../app\" }\n", 1),
        (
            "[dependencies]\nviz = { package = 'timeline', version = \"0.1\" }\n",
            1,
        ),
        ("[dependencies.viz]\npackage = 'app'\n", 1),
        // Dotted keys inside a dependency table.
        ("[dev-dependencies]\nviz.path = \"../timeline\"\n", 1),
        ("[dev-dependencies]\nviz.package = \"timeline\"\n", 1),
        ("[dependencies]\nviz.path = \"../app\"\n", 1),
        ("[dependencies]\nviz.package = 'app'\n", 1),
        // Inline tables, in both tables.
        ("[dev-dependencies]\nviz = { path = \"../timeline\" }\n", 1),
        ("[dev-dependencies]\nviz = { package = \"app\" }\n", 1),
        ("[dependencies]\nviz = { path = \"../app\" }\n", 1),
        ("[dependencies]\nviz = { package = \"timeline\" }\n", 1),
    ];
    for (manifest, at_least) in cases {
        let found = forbidden_deps(manifest);
        assert!(
            found.len() >= at_least,
            "missed a forbidden dependency in {manifest:?}: {found:?}"
        );
    }
}

#[test]
fn the_parser_passes_what_the_engine_may_depend_on() {
    let clean = "[package]\nname = \"engine\"\n\n[dependencies]\n\n\
                 [dev-dependencies]\ncriterion = { version = \"0.5\", default-features = false }\n\n\
                 [[bench]]\nname = \"timeline\"\nharness = false\n";
    assert_eq!(forbidden_deps(clean), Vec::<String>::new());
}
