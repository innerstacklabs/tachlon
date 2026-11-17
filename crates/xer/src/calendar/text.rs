//! Small text helpers for calendar blobs.

pub(super) fn attr<'a>(attrs: &'a str, wanted: &str) -> Option<&'a str> {
    let mut parts = attrs.split('|');
    while let Some(name) = parts.next() {
        let value = parts.next()?;
        if name == wanted {
            return Some(value);
        }
    }
    None
}

pub(super) fn short(value: &str) -> String {
    value.chars().take(40).collect()
}
