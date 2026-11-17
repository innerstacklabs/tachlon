//! Opening a generated MS Project file through `load_bytes`, the call `xer.rs` times for P6.
//!
//! The first test is part of the normal run: a 5,000-activity file opens with every task and
//! every link. The second prints how long a 50,000-activity MS Project file takes to open next to
//! a P6 file of the same size, with the machine load beside each figure (gs#703).
//!
//! Run: `cargo test --release -p app --test it -- --ignored --nocapture mspdi_timing::`

use app::{Loaded, load_bytes};
use import::ProjectSelection;
use std::time::Instant;

#[path = "common/mspdi_synth.rs"]
mod mspdi_synth;

use mspdi_synth::Spec;

/// Open every project the file holds, as the page does today.
fn load_all(bytes: &[u8]) -> Result<Loaded, String> {
    load_bytes(bytes, &ProjectSelection::All, &|| 0.0).map_err(|error| error.to_string())
}

/// The machine load average: `/proc/loadavg` on Linux, `sysctl vm.loadavg` on macOS, or a note
/// that this machine gives neither.
fn load_average() -> String {
    if let Ok(text) = std::fs::read_to_string("/proc/loadavg") {
        return text.trim().to_owned();
    }
    std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || "unavailable".to_owned(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        )
}

/// One timed `load_all`: the milliseconds, with the load before and after the call.
#[expect(
    clippy::print_stdout,
    reason = "the timing test reports its numbers; cargo test shows them with --nocapture"
)]
fn timed(label: &str, bytes: &[u8]) -> Loaded {
    let before = load_average();
    let started = Instant::now();
    let loaded = load_all(bytes).expect("the generated file loads");
    let elapsed = started.elapsed();
    let after = load_average();
    println!(
        "  {label:<5} {:>8.1} ms   load before [{before}]   after [{after}]",
        elapsed.as_secs_f64() * 1000.0
    );
    loaded
}

#[test]
fn a_5000_activity_mspdi_opens_with_every_task_and_every_link() {
    let spec = Spec::with_activities(42, 5_000);
    let counts = spec.counts();
    let loaded = load_all(&mspdi_synth::write(&spec)).expect("the generated MS Project file loads");

    assert_eq!(
        loaded.schedule.tasks().len(),
        counts.summaries + counts.activities
    );
    assert_eq!(loaded.meta.len(), loaded.schedule.tasks().len());
    assert_eq!(loaded.solution.dates.len(), loaded.schedule.tasks().len());
    // `load_bytes` drops a link with a summary at either end; the generated links join tasks only.
    assert_eq!(loaded.schedule.dependencies().len(), counts.links);
    let notes = loaded
        .unsupported
        .iter()
        .filter(|line| line.ends_with("actuals ignored"))
        .count();
    assert_eq!(notes, 0, "the importer applies actuals (gs#860)");
    // The one other line is the in-progress file-dates line `load_bytes` adds.
    let others: Vec<_> = loaded
        .unsupported
        .iter()
        .filter(|line| !line.ends_with("actuals ignored"))
        .collect();
    assert_eq!(others.len(), 1, "{others:?}");
    assert!(
        others[0].starts_with("in-progress activities:"),
        "{others:?}"
    );
    assert!(
        loaded
            .unsupported
            .iter()
            .all(|line| !line.contains("dropped")),
        "a generated link was dropped: {:?}",
        loaded.unsupported
    );
}

/// Prints; asserts no timing.
#[test]
#[ignore = "prints timings at 50,000 activities; run by hand on a quiet machine"]
fn timing_of_the_open_at_50k_activities_mspdi_beside_xer() {
    let mspdi_spec = Spec::with_activities(42, 50_000);
    let counts = mspdi_spec.counts();
    let mspdi = mspdi_synth::write(&mspdi_spec);
    let xer_spec = xer::synth::Spec::with_activities(42, 50_000);
    let xer = xer::synth::write(&xer_spec);

    println!(
        "mspdi 50k: {} bytes, {} summaries, {} tasks, {} links, {} actuals",
        mspdi.len(),
        counts.summaries,
        counts.activities,
        counts.links,
        counts.actuals
    );
    println!(
        "xer   50k: {} bytes, {} tasks",
        xer.len(),
        xer_spec.counts().activities
    );
    println!("load_bytes (all projects), three alternating calls:");
    for round in 1..=3 {
        println!("round {round}");
        let from_mspdi = timed("mspdi", &mspdi);
        let from_xer = timed("xer", &xer);
        assert_eq!(
            from_mspdi.schedule.tasks().len(),
            counts.summaries + counts.activities
        );
        assert_eq!(from_mspdi.schedule.dependencies().len(), counts.links);
        assert_eq!(
            from_xer.schedule.dependencies().len(),
            xer_spec.counts().links
        );
        if round == 1 {
            println!(
                "  notes listed: mspdi {}, xer {}",
                from_mspdi.unsupported.len(),
                from_xer.unsupported.len()
            );
        }
    }
}
