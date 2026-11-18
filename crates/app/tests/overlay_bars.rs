//! A host's overlay (tl#2086): a second strip under the comparison bar, at dates by key; not an edit.

use crate::common::outline_session;
use app::host::KeyTarget;
use timeline::scene::span_extent;

/// A record for `key`, 6 to 8 January 2026.
fn rec(key: &str) -> String {
    format!(r#"{{"key":"{key}","start":"2026-01-06","finish":"2026-01-08"}}"#)
}

#[test]
fn the_overlay_is_drawn_at_its_dates_and_null_removes_it() {
    let (mut session, _) = outline_session(80);
    let outline = session.outline().expect("an outline");
    let (leaf, summary) =
        (outline.preorder().iter().copied()).partition::<Vec<_>, _>(|&t| !outline.is_summary(t));
    for (task, key) in [(leaf[0], "a"), (summary[0], "s")] {
        session
            .assign_key(KeyTarget::Task(task), key)
            .expect("a key");
    }
    session.fit(0.0, 40.0 * 1440.0);
    let (revision, steps) = (session.revision(), session.history_len());
    let drawn = |session: &app::Session| {
        let (overlay, mut scene) = (session.theme().overlay, session.frame_scene(|_, _, _| 8.0));
        scene.rects.retain(|r| r.color == overlay);
        scene.rects
    };
    let both = format!("[{},{}]", rec("a"), rec("s"));
    session.set_overlay_records(Some(&both)).unwrap();
    let got = drawn(&session);
    let day = |t: &str| session.origin().parse_day(t).unwrap();
    let (x0, x1) = span_extent(
        &session.timeline().viewport,
        day("2026-01-06"),
        day("2026-01-09"),
    );
    assert_eq!(got.len(), 1, "the leaf, not the summary");
    assert!((f64::from(got[0].pos[0]) - x0).abs() < 1.0);
    assert!((f64::from(got[0].pos[0] + got[0].size[0]) - x1).abs() < 1.0);
    assert_eq!(
        (session.revision(), session.history_len()),
        (revision, steps)
    );
    let refused = session.set_overlay_records(Some(&format!("[{}]", rec("zz"))));
    assert_eq!(refused.unwrap_err().kind, "unknownKey");
    assert_eq!(
        drawn(&session).len(),
        1,
        "a refusal leaves the overlay as it was"
    );
    session.set_overlay_records(None).unwrap();
    assert_eq!(drawn(&session).len(), 0);
}
