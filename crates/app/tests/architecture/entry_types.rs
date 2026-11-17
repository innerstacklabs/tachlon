//! Check 11, the page-only types (gs#552): the self-tests of `check_entry_types` in `entry.rs`. No exported
//! declaration of the entry names a type it imports from `./measure.js` or `./page-knobs.js`, directly,
//! through a local `interface` or `type`, or by re-export; a page-only module used in a body does not fail.

use crate::architecture::entry::check_entry_types;
use crate::architecture::support::*;

const KNOBS_IMPORT: &str = "import { Metrics, type MeasureConfig } from \"./measure.js\";\nimport type { PageKnobs } from \"./page-knobs.js\";\nimport { other } from \"./other.js\";\n";

#[test]
fn a_measure_type_in_an_exported_interface_fails() {
    let text = format!(
        "{KNOBS_IMPORT}export interface MountOptions {{\n  schedule: File;\n  measure?: MeasureConfig;\n}}\n"
    );
    let failures = check_entry_types(&text);
    one(
        &failures,
        web!(
            "src/index.ts:4 the export `MountOptions` names `MeasureConfig`, which comes from `./measure.js`"
        ),
    );
    assert_eq!(failures.len(), 1, "{failures:#?}");
}

#[test]
fn page_knobs_in_an_exported_interface_fail() {
    let text = format!("{KNOBS_IMPORT}export interface MountOptions {{\n  page?: PageKnobs;\n}}\n");
    let failures = check_entry_types(&text);
    one(
        &failures,
        "the export `MountOptions` names `PageKnobs`, which comes from `./page-knobs.js`",
    );
    assert_eq!(failures.len(), 1, "{failures:#?}");
}

#[test]
fn a_measure_type_behind_a_local_type_fails_and_names_the_way() {
    // The shape `MountOptions` had: a local `PageKnobs` that names the measure types.
    let text = format!(
        "{KNOBS_IMPORT}interface Knobs {{\n  measure: MeasureConfig | null;\n}}\ntype Wrapped = {{ k: Knobs }};\nexport interface MountOptions {{\n  page?: Wrapped;\n}}\n"
    );
    let failures = check_entry_types(&text);
    one(
        &failures,
        web!(
            "src/index.ts:8 the export `MountOptions` names `MeasureConfig` (through `Wrapped`, `Knobs`)"
        ),
    );
    assert_eq!(failures.len(), 1, "{failures:#?}");
}

#[test]
fn a_measure_type_in_an_exported_signature_or_alias_fails() {
    let text = format!(
        "{KNOBS_IMPORT}export async function mount(\n  root: HTMLDivElement,\n  page?: PageKnobs,\n): Promise<void> {{\n  return;\n}}\nexport type Done = (outcome: MeasureConfig) => void;\n"
    );
    let failures = check_entry_types(&text);
    one(
        &failures,
        web!("src/index.ts:4 the export `mount` names `PageKnobs`"),
    );
    one(
        &failures,
        web!("src/index.ts:10 the export `Done` names `MeasureConfig`"),
    );
    assert_eq!(failures.len(), 2, "{failures:#?}");
}

#[test]
fn a_page_only_module_re_exported_fails() {
    let text = "import type { PageKnobs } from \"./page-knobs.js\";\nexport type { PageKnobs };\nexport type { Config } from \"./measure.js\";\n";
    let failures = check_entry_types(text);
    one(&failures, web!("src/index.ts:2 exports `PageKnobs`"));
    one(
        &failures,
        web!("src/index.ts:3 re-exports from `./measure.js`"),
    );
    assert_eq!(failures.len(), 2, "{failures:#?}");
}

#[test]
fn a_page_only_module_used_in_a_body_or_by_a_local_value_passes() {
    // `mount` takes the knobs inside its body; no exported declaration names one.
    let text = format!(
        "import {{ takePageKnobs }} from \"./page-knobs.js\";\n{KNOBS_IMPORT}interface Local {{ a: number }}\nexport interface MountOptions {{\n  schedule: File;\n  local?: Local;\n}}\nexport async function mount(root: HTMLDivElement, options: MountOptions): Promise<void> {{\n  const page: PageKnobs | undefined = takePageKnobs();\n  const m = new Metrics({{}}, page?.measure ?? null);\n}}\n"
    );
    assert_eq!(check_entry_types(&text), Vec::<String>::new());
}
