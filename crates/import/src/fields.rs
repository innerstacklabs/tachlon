use crate::ImportError;
use crate::dom::Node;

pub(crate) fn invalid(element: &str, value: &str) -> ImportError {
    ImportError::Invalid {
        element: element.to_owned(),
        value: value.to_owned(),
    }
}

pub(crate) fn required_text<'a>(node: &'a Node, element: &str) -> Result<&'a str, ImportError> {
    node.child_text(element)
        .ok_or_else(|| ImportError::Missing {
            element: element.to_owned(),
        })
}

pub(crate) fn required_u32(node: &Node, element: &str) -> Result<u32, ImportError> {
    let value = required_text(node, element)?;
    value.parse().map_err(|_| invalid(element, value))
}

pub(crate) fn optional_u32(node: &Node, element: &str) -> Result<Option<u32>, ImportError> {
    node.child_text(element)
        .map(|value| value.parse().map_err(|_| invalid(element, value)))
        .transpose()
}

pub(crate) fn optional_i64(node: &Node, element: &str) -> Result<Option<i64>, ImportError> {
    node.child_text(element)
        .map(|value| value.parse().map_err(|_| invalid(element, value)))
        .transpose()
}

pub(crate) fn flag(node: &Node, element: &str) -> Result<Option<bool>, ImportError> {
    match node.child_text(element) {
        None => Ok(None),
        Some("0") => Ok(Some(false)),
        Some("1") => Ok(Some(true)),
        Some(value) => Err(invalid(element, value)),
    }
}

pub(crate) fn format_code(node: &Node, element: &str) -> Result<Option<(u32, u32)>, ImportError> {
    optional_u32(node, element).map(|code| {
        code.map(|raw| {
            let normalized = if raw >= 35 { raw - 32 } else { raw };
            (raw, normalized)
        })
    })
}

/// Whether a normalized `DurationFormat` or `LagFormat` code is an elapsed unit: 4 minutes,
/// 6 hours, 8 days, 10 weeks, 12 months and 20 percent. A task's duration in one of them counts
/// in the all-minutes calendar (`Calendars::elapsed`); a link's lag is still counted in working
/// time.
pub(crate) fn elapsed_format(code: u32) -> bool {
    matches!(code, 4 | 6 | 8 | 10 | 12 | 20)
}

pub(crate) fn percent_format(code: u32) -> bool {
    matches!(code, 19 | 20)
}
