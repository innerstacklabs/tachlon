//! A printed page does not depend on the window (gs#703): the same schedule prints the same pages
//! from a small window and from a large one.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use common::outline_session;
use timeline::Viewport;

/// The `transform` of the page's list-and-chart group, which carries its scale.
fn scale_of(page: &str) -> &str {
    let from = page.find("scale(").expect("the page scales its body");
    page[from..].split(')').next().unwrap()
}

#[test]
fn the_same_schedule_prints_the_same_pages_at_two_window_sizes() {
    let probe = Viewport::new(1.0, 1.0, 1);
    let height = |rows: f64| probe.axis_height + rows * probe.row_height;

    let mut small = outline_session(100).0;
    small.resize(1_000.0, height(25.0), 1.0);
    let mut large = outline_session(100).0;
    large.resize(1_900.0, height(40.0), 1.0);

    let (small, large) = (small.print_pages("Plan", ""), large.print_pages("Plan", ""));
    assert!(small.len() > 1, "the fixture takes more than one page");
    assert_eq!(small.len(), large.len(), "[pages]");
    for (a, b) in small.iter().zip(&large) {
        assert_eq!(scale_of(a), scale_of(b), "[scale]");
        assert_eq!(scale_of(a), "scale(1", "[scale-1]");
    }
    assert_eq!(small, large, "[same-pages]");
}
