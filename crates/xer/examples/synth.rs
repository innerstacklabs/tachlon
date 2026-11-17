//! Writes a synthetic file in the `%T`/`%F`/`%R` text form to a path. A test generator for the
//! open-time measurement (`demos/50k/measure.mjs --open`); it is not an XER exporter and its output
//! is never committed.
//!
//! ```text
//! cargo run --release -p xer --features synth --example synth -- \
//!     --out /path/to/50k.xer --activities 50000 [--projects 1] [--seed 1] [--names numbered|site]
//! ```
//!
//! `--activities` counts the activities of each project, so a two-project file of 25,000 each is
//! `--projects 2 --activities 25000`. `--names site` gives work packages and activities construction
//! names ("Pour slab, level 3, zone B"); the default is "Activity 42" and "Work package 7".

#[cfg(feature = "synth")]
#[expect(
    clippy::print_stderr,
    reason = "an example run from a terminal, which tells the person running it what went wrong"
)]
fn main() {
    if let Err(message) = run() {
        eprintln!("synth: {message}");
        std::process::exit(2);
    }
}

#[cfg(feature = "synth")]
#[expect(
    clippy::print_stdout,
    reason = "the example prints the path it wrote for the person running it"
)]
fn run() -> Result<(), String> {
    let mut out: Option<std::path::PathBuf> = None;
    let mut activities: usize = 1000;
    let mut projects: usize = 1;
    let mut seed: u64 = 1;
    let mut names = xer::synth::Names::Numbered;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--out" => out = Some(value("--out")?.into()),
            "--activities" => activities = number("--activities", &value("--activities")?)?,
            "--projects" => projects = number("--projects", &value("--projects")?)?,
            "--seed" => seed = number("--seed", &value("--seed")?)?,
            "--names" => names = naming(&value("--names")?)?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let out = out.ok_or("--out <path> is required")?;
    if projects == 0 {
        return Err("--projects must be at least 1".to_string());
    }
    if activities == 0 {
        return Err("--activities must be at least 1".to_string());
    }
    let spec = xer::synth::Spec {
        projects,
        names,
        ..xer::synth::Spec::with_activities(seed, activities)
    };
    let bytes = xer::synth::write(&spec);
    std::fs::write(&out, &bytes)
        .map_err(|error| format!("cannot write {}: {error}", out.display()))?;
    println!(
        "wrote {} ({} bytes): {} project(s) x {} activities, seed {}",
        out.display(),
        bytes.len(),
        projects,
        activities,
        seed
    );
    Ok(())
}

#[cfg(feature = "synth")]
fn naming(text: &str) -> Result<xer::synth::Names, String> {
    match text {
        "numbered" => Ok(xer::synth::Names::Numbered),
        "site" => Ok(xer::synth::Names::Site),
        _ => Err(format!("--names is numbered or site, got {text:?}")),
    }
}

#[cfg(feature = "synth")]
fn number<T: std::str::FromStr>(name: &str, text: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("{name} needs a whole number, got {text:?}"))
}

#[cfg(not(feature = "synth"))]
#[expect(
    clippy::print_stderr,
    reason = "an example run from a terminal, which tells the person running it what to do"
)]
fn main() {
    eprintln!(
        "synth: build with --features synth: cargo run -p xer --features synth --example synth -- --help"
    );
    std::process::exit(2);
}
