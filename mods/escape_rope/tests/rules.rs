use horizon_escape_rope::*;
use std::collections::{BTreeMap, BTreeSet};

/// A flat world: the surface height of every column (default 64) and some light-stopping blocks.
#[derive(Default)]
struct Land {
    surface: BTreeMap<(i32, i32), i32>,
    opaque: BTreeSet<Pos>,
}
impl Sky for Land {
    fn surface(&self, x: i32, z: i32) -> i32 {
        *self.surface.get(&(x, z)).unwrap_or(&64)
    }
    fn blocks_light_not_liquid(&self, p: Pos) -> bool {
        self.opaque.contains(&p)
    }
}

#[test]
fn above_ground_sees_the_sky_underground_does_not() {
    let w = Land::default();
    // the surface block is at y 63 (height 64): standing on it sees the sky, a block below does not
    assert!(can_see_sky(&w, [0, 64, 0]));
    assert!(can_see_sky(&w, [0, 100, 0]));
    assert!(!can_see_sky(&w, [0, 63, 0]));
    assert!(!can_see_sky(&w, [0, 30, 0]));
}

#[test]
fn at_the_sea_level_and_above_it_is_plain_can_see_sky() {
    let mut w = Land::default();
    w.surface.insert((5, 5), 80); // a hill
    assert!(can_see_sky_from_below_water(&w, [5, 80, 5]));
    assert!(!can_see_sky_from_below_water(&w, [5, 79, 5]));
    // y 63 is the sea level itself: with the surface at 63 there it is the plain test, with 64 it is not
    w.surface.insert((0, 0), 63);
    assert!(can_see_sky_from_below_water(&w, [0, 63, 0]));
    w.surface.insert((0, 0), 64);
    assert!(!can_see_sky_from_below_water(&w, [0, 63, 0]));
}

#[test]
fn below_the_sea_level_the_column_up_to_the_sea_must_be_open() {
    // a sea: the surface (WORLD_SURFACE counts water) is at 63 so y 63 sees the sky; water does not stop light
    let mut w = Land::default();
    w.surface.insert((1, 1), 63);
    assert!(can_see_sky_from_below_water(&w, [1, 40, 1]), "under water, nothing in the way");
    // a block that stops light under the sea level: a ledge at y 50 hides y 40 but not y 50 or 51
    w.opaque.insert([1, 50, 1]);
    assert!(!can_see_sky_from_below_water(&w, [1, 40, 1]));
    assert!(can_see_sky_from_below_water(&w, [1, 50, 1]), "the loop stops above the player's block");
    assert!(can_see_sky_from_below_water(&w, [1, 51, 1]));
}

#[test]
fn a_column_closed_at_the_sea_level_is_closed_below() {
    let mut w = Land::default();
    w.surface.insert((2, 2), 90); // a roof/hill above the sea level
    assert!(!can_see_sky_from_below_water(&w, [2, 62, 2]));
    assert!(!can_see_sky_from_below_water(&w, [2, 10, 2]));
}

#[test]
fn the_position_is_floored() {
    assert_eq!(containing([1.9, 64.0, -0.1]), [1, 64, -1]);
    assert_eq!(containing([-3.0, 0.999, 7.5]), [-3, 0, 7]);
}

#[test]
fn tracking_remembers_the_exact_place_under_the_sky_only() {
    let w = Land::default();
    let mut last = Last::default();
    assert!(!track(&w, [0.5, 40.0, 0.5], &mut last), "underground: nothing");
    assert_eq!(last, Last::default());
    assert!(track(&w, [10.25, 70.0, -3.75], &mut last));
    assert_eq!((last.x, last.y, last.z, last.set), (10.25, 70.0, -3.75, true));
    assert!(!track(&w, [10.25, 70.0, -3.75], &mut last), "same place: unchanged");
    assert!(!track(&w, [20.0, 20.0, 20.0], &mut last), "going underground keeps the last place");
    assert_eq!((last.x, last.y, last.z), (10.25, 70.0, -3.75));
}

#[test]
fn the_rope_takes_you_to_the_last_place_you_saw_the_sky() {
    let w = Land::default();
    let last = Last { x: 10.25, y: 70.0, z: -3.75, set: true };
    assert_eq!(use_rope(&w, [0.5, 20.0, 0.5], &last), Rope::Teleport { to: [10.25, 70.0, -3.75] });
}

#[test]
fn the_rope_does_not_work_under_the_sky() {
    let w = Land::default();
    let last = Last { x: 1.0, y: 70.0, z: 1.0, set: true };
    assert_eq!(use_rope(&w, [0.5, 70.0, 0.5], &last), Rope::Blocked);
}

#[test]
fn without_a_stored_place_the_rope_does_nothing() {
    let w = Land::default();
    assert_eq!(use_rope(&w, [0.5, 20.0, 0.5], &Last::default()), Rope::NowhereToGo);
}

#[test]
fn the_stored_place_round_trips_exactly() {
    let last = Last { x: -1234.567890123, y: 63.00000000000001, z: 0.1 + 0.2, set: true };
    assert_eq!(Last::load(Some(last.save())), last);
    assert_eq!(Last::load(None), Last::default());
    assert_eq!(Last::load(Some("junk\n1\n2\n".into())), Last::default());
    assert_eq!(Last::load(Some("NaN\n1\n2\n".into())), Last::default());
    assert_eq!(Last::default().save(), "0.0\n0.0\n0.0\nfalse\n");
}

#[test]
fn texts_are_the_originals() {
    assert_eq!(USED, "You used the escape rope!");
    assert_eq!(BLOCKED, "You cannot use that here!");
    assert_eq!(TOOLTIP, "This handy rope will take you to the last place you could see the sky.");
    assert_eq!(SEA_LEVEL, 63);
    assert_eq!(ITEM, "escape_rope:escape_rope");
}
