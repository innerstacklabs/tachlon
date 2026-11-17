//! The XER crate must stay dependency-free and licensed MIT, like the rest of the repository.
//!
//! This test reads `crates/xer/Cargo.toml` as text rather than using
//! `cargo metadata`. It understands dependency table headers and dotted keys,
//! but it is deliberately not a general TOML parser.

const REQUIRED_LICENSE: &str = "\"MIT\"";
const VALID_PACKAGE: &str = "[package]\nname = \"xer\"\nlicense = \"MIT\"\n";

/// Returns boundary violations found in the manifest text.
fn manifest_findings(manifest: &str) -> Vec<String> {
    let mut findings = Vec::new();
    let mut in_dependencies = false;
    let mut in_package = false;
    let mut license_values = Vec::new();

    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') {
            let header = line.trim_matches(['[', ']']).trim();
            let segments = dotted_segments(header);
            in_package = segments.len() == 1 && segments[0] == "package";
            in_dependencies = segments
                .last()
                .is_some_and(|segment| segment.ends_with("dependencies"));

            if segments.len() >= 2 && segments[segments.len() - 2].ends_with("dependencies") {
                findings.push(format!("dependency table [{header}] names a dependency"));
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        if in_dependencies {
            findings.push(format!("dependency declaration: {line}"));
        }

        if in_package {
            match key {
                "license" => license_values.push(value),
                "license-file" => findings.push("package uses license-file".to_string()),
                _ if key.starts_with("license.") => {
                    findings.push(format!("package uses dotted license key: {key}"));
                }
                _ => {}
            }
        }
    }

    if license_values.as_slice() != [REQUIRED_LICENSE] {
        findings.push(format!(
            "package license must be exactly {REQUIRED_LICENSE}, found {license_values:?}"
        ));
    }

    findings
}

/// Splits a TOML dotted header without splitting dots inside quoted segments.
fn dotted_segments(header: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut quote = None;

    for (index, character) in header.char_indices() {
        match (quote, character) {
            (None, '\'' | '"') => quote = Some(character),
            (Some(open), close) if open == close => quote = None,
            (None, '.') => {
                segments.push(normalize_segment(&header[start..index]));
                start = index + 1;
            }
            _ => {}
        }
    }
    segments.push(normalize_segment(&header[start..]));
    segments
}

fn normalize_segment(segment: &str) -> String {
    segment.trim().trim_matches(['\'', '"']).trim().to_string()
}

/// Returns a missing or empty `LICENSE-MIT`, or a leftover `LICENSE-APACHE`, as findings.
fn licence_file_findings(mit: Option<&[u8]>, apache: Option<&[u8]>) -> Vec<String> {
    let mut findings = Vec::new();
    match mit {
        Some(bytes) if !bytes.is_empty() => {}
        Some(_) => findings.push("LICENSE-MIT is empty".to_string()),
        None => findings.push("LICENSE-MIT is missing".to_string()),
    }
    if apache.is_some() {
        findings.push("LICENSE-APACHE must not exist: the license is MIT only".to_string());
    }
    findings
}

#[test]
fn dependency_declarations_are_rejected() {
    let cases = [
        "[dependencies]\nserde = \"1\"\n",
        "[dev-dependencies]\nserde = \"1\"\n",
        "[build-dependencies]\ncc = \"1\"\n",
        "[target.'cfg(target_arch = \"wasm32\")'.dependencies]\njs = \"1\"\n",
        "[dependencies.serde]\nversion = \"1\"\n",
        "[dependencies]\nserde.workspace = true\n",
        "[dependencies]\ndependencies.serde = \"1\"\n",
    ];

    for case in cases {
        let manifest = format!("{VALID_PACKAGE}\n{case}");
        assert!(
            !manifest_findings(&manifest).is_empty(),
            "missed dependency declaration in {case:?}"
        );
    }
}

#[test]
fn package_license_must_be_exact() {
    let cases = [
        "[package]\nname = \"xer\"\n",
        "[package]\nname = \"xer\"\nlicense = \"MIT OR Apache-2.0\"\n",
        "[package]\nname = \"xer\"\nlicense.workspace = true\n",
        "[package]\nname = \"xer\"\nlicense-file = \"LICENSE\"\n",
    ];

    for manifest in cases {
        assert!(
            !manifest_findings(manifest).is_empty(),
            "accepted invalid package licensing in {manifest:?}"
        );
    }
}

#[test]
fn licence_files_must_exist_and_not_be_empty() {
    assert_ne!(licence_file_findings(None, None), [] as [String; 0]);
    assert_ne!(licence_file_findings(Some(b""), None), [] as [String; 0]);
    assert_ne!(
        licence_file_findings(Some(b"MIT"), Some(b"Apache")),
        [] as [String; 0]
    );
    assert_eq!(licence_file_findings(Some(b"MIT"), None), [] as [String; 0]);
}

#[test]
fn real_crate_obeys_the_licence_boundary() {
    let directory = env!("CARGO_MANIFEST_DIR");
    let manifest_path = format!("{directory}/Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("{manifest_path} must be readable: {error}"));
    let mit = std::fs::read(format!("{directory}/LICENSE-MIT")).ok();
    let apache = std::fs::read(format!("{directory}/LICENSE-APACHE")).ok();

    let mut findings = manifest_findings(&manifest);
    findings.extend(licence_file_findings(mit.as_deref(), apache.as_deref()));
    assert!(
        findings.is_empty(),
        "crates/xer must be dependency-free and licensed exactly MIT: {findings:?}"
    );
}
