//! Pins the stress fixture (gs#320, ruling devteam-1-103): `stress` and
//! `stress_with_exceptions` are the measurement series, so their schedule must
//! not move when the demo's generated shape does. The values were read from
//! the generator as it stood on main (3f191ea6) at the seed and size the stress
//! timing uses (`SEED` and the 50,000-task stress).

use engine::{stress, stress_with_exceptions};

const SEED: u64 = 20_261_020;
const N: usize = 50_000;

#[test]
fn the_stress_fixture_is_the_schedule_it_was_on_main() {
    let (mut s, _head) = stress(SEED, N);
    let sol = s.compute().expect("the stress fixture is acyclic");
    assert_eq!(s.tasks().len(), 50_000, "task count");
    assert_eq!(s.dependencies().len(), 170_155, "link count");
    assert_eq!(sol.project_finish, 89_527_050, "project finish (minutes)");
}

#[test]
fn the_stress_fixture_with_exceptions_is_the_schedule_it_was_on_main() {
    let (mut s, _head) = stress_with_exceptions(SEED, N);
    let sol = s.compute().expect("the stress fixture is acyclic");
    assert_eq!(s.tasks().len(), 50_000, "task count");
    assert_eq!(s.dependencies().len(), 170_155, "link count");
    assert_eq!(sol.project_finish, 89_587_530, "project finish (minutes)");
}
