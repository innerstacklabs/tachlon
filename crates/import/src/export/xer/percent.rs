//! The `phys_complete_pct` cell of a `TASK` row.

/// The physical percent a task holds (hundredths of a percent, up to two decimals), else the
/// percent shown from its remaining work; empty when it has neither.
pub(super) fn cell(physical: Option<u16>, progress: Option<u8>) -> String {
    let Some(hundredths) = physical else {
        return progress.map_or_default(|value| value.to_string());
    };
    let (whole, fraction) = (hundredths / 100, hundredths % 100);
    match fraction {
        0 => whole.to_string(),
        _ => format!("{whole}.{fraction:02}")
            .trim_end_matches('0')
            .to_owned(),
    }
}
