//! Self-tests of the cloud rules of check 12 (decision 0011): the component never names the cloud
//! API, and one file of the hosted app, `store.ts`, is the only door to `apps/cloud`.

use super::*;

const STORE: &str = "apps/hosted/src/store.ts";
const CLIENT: &str = "import { client } from \"../../cloud/src/client\";\n";

#[test]
fn a_file_of_the_component_naming_the_cloud_api_fails() {
    let mut t = tree();
    append(
        &mut t,
        concat!(web_root!(), "/src/open.ts"),
        "const url = \"/api/docs/abc\";\n",
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/open.ts:2 names /api/docs; the component knows nothing about the cloud API \
         (docs/decisions/0011-cloud-api-in-this-repository.md)"
        ),
    );
}

#[test]
fn a_hosted_file_other_than_the_store_importing_the_cloud_fails() {
    let mut t = tree();
    append(&mut t, "apps/hosted/src/samples.ts", CLIENT);
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "apps/hosted/src/samples.ts:2 imports ../../cloud/src/client, which is in apps/cloud; \
         only apps/hosted/src/store.ts reaches the cloud API (docs/decisions/0011-cloud-api-in-this-repository.md)",
    );
}

#[test]
fn the_store_importing_the_cloud_passes() {
    let mut t = tree();
    t.hosted.push(file(STORE, CLIENT));
    let failures = check_tree(&t);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_hosted_file_other_than_the_store_naming_the_cloud_api_fails() {
    let mut t = tree();
    append(&mut t, "apps/hosted/src/main.ts", "fetch(\"/api/docs\");\n");
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        "apps/hosted/src/main.ts:4 names /api/docs; only apps/hosted/src/store.ts reaches the \
         cloud API (docs/decisions/0011-cloud-api-in-this-repository.md)",
    );
    let mut t = tree();
    t.hosted.push(file(STORE, "fetch(\"/api/docs\");\n"));
    let failures = check_tree(&t);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_component_file_importing_the_cloud_by_path_fails() {
    let mut t = tree();
    append(
        &mut t,
        concat!(web_root!(), "/src/main.ts"),
        "import \"../../apps/cloud/src/client\";\n",
    );
    let failures = check_tree(&t);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    one(
        &failures,
        concat!(
            web_root!(),
            "/src/main.ts:3 imports ../../apps/cloud/src/client, which names apps/"
        ),
    );
}

/// Each manifest, and the path from its directory to `apps/cloud`.
const MANIFESTS: [(&str, &str); 2] = [
    ("apps/hosted/package.json", "../cloud"),
    (concat!(web_root!(), "/package.json"), "../../apps/cloud"),
];

fn set_dependencies(t: &mut Tree, path: &str, table: &str, name: &str, spec: &str) {
    // The tools the config files import, and the chart's package, stay declared, in a table the rule does not read.
    let text = format!(
        "{{\n  \"peerDependencies\": {{ \"vite\": \"8\", \"@playwright/test\": \"1\", \"gantt-core\": \"1\" }},\n  \
         \"{table}\": {{ \"{name}\": \"{spec}\" }}\n}}\n"
    );
    t.component
        .iter_mut()
        .chain(t.hosted.iter_mut())
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("no file {path}"))
        .text = text;
}

#[test]
fn a_manifest_depending_on_apps_cloud_fails() {
    for (path, to_cloud) in MANIFESTS {
        let by_path = [format!("file:{to_cloud}"), format!("link:{to_cloud}")];
        let cases = [
            ("dependencies", "@gantt/cloud", "workspace:*"),
            ("devDependencies", "cloud", by_path[0].as_str()),
            ("optionalDependencies", "cloud", by_path[1].as_str()),
            ("dependencies", "api", "npm:@gantt/cloud@1"),
        ];
        for (table, name, spec) in cases {
            let mut t = tree();
            set_dependencies(&mut t, path, table, name, spec);
            let failures = check_tree(&t);
            assert_eq!(failures.len(), 1, "{path} {spec}: {failures:#?}");
            one(
                &failures,
                &format!("{path} {table} names {name}: \"{spec}\", which is in apps/cloud;"),
            );
        }
    }
}

#[test]
fn a_manifest_with_other_workspace_dependencies_passes() {
    for (path, _) in MANIFESTS {
        for (name, spec) in [
            ("gantt-core", "workspace:*"),
            ("hosted", "file:../hosted"),
            ("cloudy", "1"),
        ] {
            let mut t = tree();
            set_dependencies(&mut t, path, "dependencies", name, spec);
            let failures = check_tree(&t);
            assert!(failures.is_empty(), "{path} {spec}: {failures:#?}");
        }
    }
}

#[test]
fn the_repository_manifests_name_no_cloud_dependency() {
    let tree = read_tree();
    let manifests: Vec<&SourceFile> = tree.component.iter().chain(&tree.hosted).collect();
    for (path, _) in MANIFESTS {
        assert!(manifests.iter().any(|f| f.path == path), "{path} not read");
    }
    let failures = super::super::cloud::dependency_failures(&manifests);
    assert!(failures.is_empty(), "{failures:#?}");
}
