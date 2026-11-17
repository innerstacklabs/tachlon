//! Deterministic construction names and outline metadata for demo tasks.

use crate::meta::TaskMeta;
use engine::{Rng, TaskId};

const PHASES: &[(&str, &[&str])] = &[
    (
        "Sitework",
        &[
            "Clear site",
            "Lay out work",
            "Excavate area",
            "Install drainage",
            "Rough grade",
            "Compact subgrade",
        ],
    ),
    (
        "Foundations",
        &[
            "Excavate footings",
            "Set footing forms",
            "Place footing rebar",
            "Pour footings",
            "Strip footing forms",
            "Backfill foundations",
        ],
    ),
    (
        "Concrete",
        &[
            "Prepare subbase",
            "Set slab forms",
            "Place rebar",
            "Pour slab",
            "Finish slab",
            "Cure and test",
            "Strip formwork",
        ],
    ),
    (
        "Structural steel",
        &[
            "Set base plates",
            "Erect columns",
            "Erect beams",
            "Install metal deck",
            "Bolt connections",
            "Inspect welds",
        ],
    ),
    (
        "Framing",
        &[
            "Lay out walls",
            "Frame exterior walls",
            "Frame interior walls",
            "Set door frames",
            "Install backing",
            "Inspect framing",
        ],
    ),
    (
        "Roofing",
        &[
            "Install roof deck",
            "Place insulation",
            "Apply roof membrane",
            "Install flashing",
            "Set roof drains",
            "Test roof assembly",
        ],
    ),
    (
        "MEP rough-in",
        &[
            "Install duct mains",
            "Run plumbing mains",
            "Run electrical conduit",
            "Set equipment curbs",
            "Install branch piping",
            "Inspect rough-in",
        ],
    ),
    (
        "Drywall",
        &[
            "Hang wall board",
            "Hang ceiling board",
            "Tape joints",
            "Apply finish coats",
            "Sand surfaces",
            "Inspect assemblies",
        ],
    ),
    (
        "Finishes",
        &[
            "Prime walls",
            "Paint walls",
            "Install ceilings",
            "Lay flooring",
            "Install casework",
            "Fit finish hardware",
        ],
    ),
    (
        "Glazing",
        &[
            "Set window frames",
            "Install curtain wall",
            "Set glass panels",
            "Seal perimeter joints",
            "Install entrances",
            "Test water tightness",
        ],
    ),
    (
        "Commissioning",
        &[
            "Start air systems",
            "Balance water systems",
            "Test life safety",
            "Verify controls",
            "Train operators",
            "Complete performance tests",
        ],
    ),
];

/// Generate one item of app-owned task metadata for each demo task. A name is the task's own
/// (`Pour slab`); the outline path (`Level 3 / Concrete / Pour slab`) comes from the parents.
///
/// The result is deterministic for `seed`; item `i` describes task `i`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "index is a task slot (Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does); Rng::range(lo, hi) returns lo to hi inclusive, so the slot counts are 3 to 6 and 4 to 12, and the other values are indexes into the constant PHASES table or one of its lists"
)]
#[must_use]
pub fn generate(seed: u64, n: usize) -> Vec<TaskMeta> {
    let mut rng = Rng::new(seed);
    let mut result = Vec::with_capacity(n);

    let mut level_one_count = 0;
    let mut level_one_id = TaskId(0);
    let mut level_two_id = TaskId(0);
    let mut level_two_phase = 0;
    let mut level_two_slots = 0;
    let mut level_three_slots = 0;

    for index in 0..n {
        let id = TaskId(index as u32);
        if index == 0 || (level_two_slots == 0 && level_three_slots == 0) {
            let level_number = level_one_count % 30 + 1;
            level_one_count += 1;
            level_one_id = id;
            level_two_slots = rng.range(3, 6) as usize;
            level_three_slots = 0;
            result.push(TaskMeta {
                name: format!("Level {level_number}"),
                parent: None,
                outline_level: 1,
                code: None,
                activity_codes: Vec::new(),
                wbs_code: None,
                uid: None,
                floor: None,
                color: None,
                label: None,
                key: None,
            });
        } else if level_three_slots == 0 {
            level_two_id = id;
            level_two_phase = rng.range(0, PHASES.len() as i64 - 1) as usize;
            level_two_slots -= 1;
            level_three_slots = rng.range(4, 12) as usize;
            result.push(TaskMeta {
                name: PHASES[level_two_phase].0.to_string(),
                parent: Some(level_one_id),
                outline_level: 2,
                code: None,
                activity_codes: Vec::new(),
                wbs_code: None,
                uid: None,
                floor: None,
                color: None,
                label: None,
                key: None,
            });
        } else {
            let activities = PHASES[level_two_phase].1;
            let activity = activities[rng.range(0, activities.len() as i64 - 1) as usize];
            level_three_slots -= 1;
            result.push(TaskMeta {
                name: activity.to_string(),
                parent: Some(level_two_id),
                outline_level: 3,
                code: None,
                activity_codes: Vec::new(),
                wbs_code: None,
                uid: None,
                floor: None,
                color: None,
                label: None,
                key: None,
            });
        }
    }

    result
}

/// The six colors the scene's tests read a name on (`scene/tests/bar_color.rs`): gold, navy, teal, amber, violet, gray.
const PALETTE: [[u8; 4]; 6] = [
    [0xff, 0xd7, 0x00, 255],
    [0x00, 0x33, 0x66, 255],
    [0x2a, 0x9d, 0x8f, 255],
    [0xd9, 0x77, 0x06, 255],
    [0x8e, 0x44, 0xad, 255],
    [0xa4, 0xa4, 0xa4, 255],
];

/// Give two in five tasks one of the `PALETTE` colors, drawn from `seed`, so every run colors the same bars.
pub fn paint(meta: &mut [TaskMeta], seed: u64) {
    let mut rng = Rng::new(seed);
    for task in meta {
        if rng.chance(0.4) {
            task.color = PALETTE
                .get(usize::try_from(rng.range(0, 5)).unwrap_or_default())
                .copied();
        }
    }
}
