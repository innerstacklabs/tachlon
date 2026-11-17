//! The names the generator gives work packages and activities.

/// How the generator names work packages and activities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Names {
    /// "Work package 7" and "Activity 42": the default, which the pinned tests read.
    #[default]
    Numbered,
    /// Construction words: "Pour slab, level 3, zone B", unique within a work package.
    Site,
}

const PACKAGES: [&str; 12] = [
    "Site works",
    "Foundations",
    "Structural frame",
    "Building envelope",
    "Roofing",
    "Mechanical",
    "Electrical",
    "Plumbing",
    "Fire protection",
    "Interior partitions",
    "Finishes",
    "Commissioning",
];

const TRADES: [&str; 40] = [
    "Excavate",
    "Compact subgrade",
    "Form footings",
    "Place rebar",
    "Pour slab",
    "Cure and strip forms",
    "Erect columns",
    "Set steel beams",
    "Deck and weld",
    "Framing",
    "Sheathing",
    "Install windows",
    "Air barrier",
    "Brick veneer",
    "Waterproofing",
    "Roof insulation",
    "Rough-in ductwork",
    "Set air handlers",
    "Pull conduit",
    "Pull wire",
    "Install panels",
    "Rough-in plumbing",
    "Pressure test",
    "Run sprinkler mains",
    "Hang drywall",
    "Tape and finish",
    "Prime and paint",
    "Lay ceiling grid",
    "Set doors and frames",
    "Install flooring",
    "Millwork",
    "Set fixtures",
    "Install trim",
    "Test and balance",
    "Fire alarm test",
    "Elevator inspection",
    "Punch list",
    "Clean and protect",
    "Final inspection",
    "Hand over",
];

const ZONES: [&str; 8] = ["A", "B", "C", "D", "E", "F", "G", "H"];

/// The name of work package `child`; child 0 is the project root, which the numbered names call
/// "Project 1" in every project.
pub(super) fn wbs_name(names: Names, child: usize) -> String {
    match (names, child) {
        (Names::Numbered, 0) => "Project 1".to_string(),
        (Names::Numbered, _) => format!("Work package {child}"),
        (Names::Site, 0) => "Project".to_string(),
        (Names::Site, _) => {
            let index = child - 1;
            let package = PACKAGES[index % PACKAGES.len()];
            format!("{package}, block {}", index / PACKAGES.len() + 1)
        }
    }
}

/// The name of the activity at zero-based position `activity`, in a project with `children` work
/// packages. The accents in every fiftieth name are on purpose: they exercise the file's encoding.
pub(super) fn task_name(names: Names, children: usize, activity: usize) -> String {
    let accent = if activity % 50 == 49 { " Café" } else { "" };
    match names {
        Names::Numbered => format!("Activity {}{accent}", activity.saturating_add(1)),
        Names::Site => format!("{}{accent}", site_task(children, activity)),
    }
}

/// A phrase that no other activity of the same work package has. Activity `n` sits in work
/// package `1 + n % children`, so `n / children` is its place within the package; the package
/// number shifts the start so neighbouring packages do not read alike.
fn site_task(children: usize, activity: usize) -> String {
    let children = children.max(1);
    let place = activity / children + activity % children * 7;
    let trade = TRADES[place % TRADES.len()];
    let spot = place / TRADES.len();
    // 6 levels alone, then 6 levels by 8 zones, then the same again as a later phase.
    let (round, spot) = (spot / 54, spot % 54);
    let location = if spot < 6 {
        format!("level {}", spot + 1)
    } else {
        let spot = spot - 6;
        format!(
            "level {}, zone {}",
            spot % 6 + 1,
            ZONES[spot / 6 % ZONES.len()]
        )
    };
    if round == 0 {
        format!("{trade}, {location}")
    } else {
        format!("{trade}, {location}, phase {}", round + 1)
    }
}
