//! No alias for the chart in the hosted app's `vite.config.ts` and `tsconfig.json` (check 12).

use super::{PACKAGE_NAME, TSCONFIG, VITE};
use crate::architecture::support::*;

/// The two files name neither `@gantt/chart` nor `gantt-core`: the app finds the built package in
/// its own install, as a customer does, so no alias can send it to the source.
pub(super) fn alias_failures(files: &[SourceFile]) -> Vec<String> {
    let mut failures = Vec::new();
    for path in [VITE, TSCONFIG] {
        let Some(file) = files.iter().find(|f| f.path == path) else {
            failures.push(format!(
                "{path}: not found; the check cannot see whether the app aliases the chart"
            ));
            continue;
        };
        for (i, line) in strip_ts_comments(&file.text).lines().enumerate() {
            for name in ["@gantt/chart", PACKAGE_NAME] {
                if line.contains(name) {
                    failures.push(format!(
                        "{path}:{} names {name}; the app loads the built package from its own \
                         install, so no alias names the chart",
                        i + 1
                    ));
                }
            }
        }
    }
    failures
}
