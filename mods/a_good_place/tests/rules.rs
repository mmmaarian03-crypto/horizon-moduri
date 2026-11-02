use horizon_a_good_place::*;

struct View {
    double: bool,
    collision: bool,
    id: &'static str,
}
impl StateView for View {
    fn is_double_block(&self) -> bool {
        self.double
    }
    fn has_collision(&self) -> bool {
        self.collision
    }
    fn is_block(&self, id: &str) -> bool {
        id == self.id
    }
    fn in_tag(&self, _tag: &str) -> bool {
        false
    }
}
/// Every neighbour empty except the one below (full block): standing on the ground.
struct Ground;
impl Around for Ground {
    fn extent(&self, dir: Dir) -> Option<[f64; 2]> {
        if dir == Dir::Down { Some([0.0, 1.0]) } else { None }
    }
}
struct Boxed;
impl Around for Boxed {
    fn extent(&self, _dir: Dir) -> Option<[f64; 2]> {
        Some([0.0, 1.0])
    }
}

fn solid() -> View {
    View { double: false, collision: true, id: "minecraft:stone" }
}
fn placer() -> Placer {
    Placer { yaw: 0.0, pitch: 30.0, right_arm: true, main_hand: true }
}

#[test]
fn default_pack_is_the_all_blocks_json() {
    let pack = default_pack();
    assert_eq!(pack.len(), 1);
    let p = &pack[0];
    assert_eq!(p.duration, 3);
    assert_eq!(p.scale_curve, -0.7);
    assert_eq!(p.translation, [0.1, 0.2, 0.2]);
    assert_eq!(p.translation_curve, 0.9);
    assert_eq!(p.rotation_curve, -0.08);
    assert_eq!(p.pivot, [0.5, -0.5, 0.5]);
    assert!(p.restrict_direction);
}

#[test]
fn pack_applies_to_solid_single_blocks_only() {
    let pack = default_pack();
    assert!(select(&pack, &solid()).is_some());
    assert!(select(&pack, &View { double: true, ..solid() }).is_none());
    assert!(select(&pack, &View { collision: false, ..solid() }).is_none());
}

#[test]
fn priority_picks_the_first_matching_pack() {
    let mut a = Params::default();
    a.priority = 1;
    a.duration = 9;
    a.predicates = vec![Predicate::MatchingBlocks(vec!["minecraft:dirt".into()])];
    let mut b = Params::default();
    b.duration = 5;
    let pack = vec![b, a];
    assert_eq!(select(&pack, &View { id: "minecraft:dirt", ..solid() }).unwrap().duration, 9);
    assert_eq!(select(&pack, &solid()).unwrap().duration, 5);
}

#[test]
fn the_animation_ends_at_the_block_and_starts_away_from_it() {
    let params = default_pack().remove(0);
    let pose = Pose::new(&params, &placer(), &Ground);
    let end = pose.matrix(1.0, [0.0; 3]);
    // At the end: the unit cube scaled by 0.999 around its centre, no rotation (the curve is 1.0 at t = 1).
    let a = transform(&end, [0.0, 0.0, 0.0]);
    let b = transform(&end, [1.0, 1.0, 1.0]);
    for i in 0..3 {
        assert!((a[i] - 0.0005).abs() < 2e-3, "{a:?}");
        assert!((b[i] - 0.9995).abs() < 2e-3, "{b:?}");
    }
    let start = pose.matrix(0.0, [0.0; 3]);
    let c = transform(&start, [0.5, 0.5, 0.5]);
    let moved = (c[0] - 0.5).abs() + (c[1] - 0.5).abs() + (c[2] - 0.5).abs();
    assert!(moved > 0.05, "the block starts displaced: {c:?}");
    // It never slides in from the ground it stands on (the slide comes from above or the sides).
    assert!(c[1] >= 0.5 - 1e-4, "{c:?}");
}

#[test]
fn closed_in_blocks_do_not_move() {
    let params = default_pack().remove(0);
    let pose = Pose::new(&params, &placer(), &Boxed);
    assert_eq!(pose.slide_start, [0.0, 0.0]);
}

#[test]
fn fancy_exponent_is_a_line_at_zero_and_bends_otherwise() {
    assert!((fancy_exponent(0.5, 0.0) - 0.5).abs() < 1e-6);
    assert!((fancy_exponent(0.0, 0.9)).abs() < 1e-6);
    assert!((fancy_exponent(1.0, 0.9) - 1.0).abs() < 1e-6);
    assert!(fancy_exponent(0.5, 0.9) < 0.5);
    assert!(fancy_exponent(0.5, -0.7) > 0.5);
}

#[test]
fn nearest_directions_follow_the_look() {
    // Looking south (yaw 0) and level: south first, north last of the horizontal ones.
    let order = ordered_by_nearest(0.0, 0.0);
    assert_eq!(order[0], Dir::South);
    assert_eq!(order[5], Dir::North);
    let down = ordered_by_nearest(0.0, 90.0);
    assert_eq!(down[0], Dir::Down);
}

#[test]
fn manager_lifecycle_hides_then_shows_the_block() {
    let params = default_pack().remove(0);
    let pose = Pose::new(&params, &placer(), &Ground);
    let mut m = Manager::default();
    let pos = [5, 64, 5];
    assert_eq!(m.add(pos, 7, pose.clone(), true, true), vec![Effect::Hide(pos)]);
    assert!(m.is_hidden(pos));
    // First tick: a fresh particle is not ticked.
    assert!(m.tick(|_| 7).is_empty());
    assert_eq!(m.particles()[0].age, 0);
    // Ticks 2 and 3: age 1, 2, still hidden. At age 3 (= duration) it is shown again.
    assert!(m.tick(|_| 7).is_empty());
    assert!(m.tick(|_| 7).is_empty());
    let e = m.tick(|_| 7);
    assert_eq!(e, vec![Effect::Unhide { pos, was_hidden: true }]);
    assert!(!m.is_hidden(pos));
    // It lives two more ticks.
    assert_eq!(m.particles().len(), 1);
    m.tick(|_| 7);
    assert_eq!(m.particles().len(), 1);
    m.tick(|_| 7);
    assert!(m.particles().is_empty());
}

#[test]
fn a_changed_block_cancels_the_animation() {
    let params = default_pack().remove(0);
    let pose = Pose::new(&params, &placer(), &Ground);
    let mut m = Manager::default();
    let pos = [0, 0, 0];
    m.add(pos, 7, pose, true, false);
    let e = m.tick(|_| 0);
    assert_eq!(e, vec![Effect::Unhide { pos, was_hidden: true }]);
    assert!(m.particles().is_empty());
    assert_eq!(m.hidden_count(), 0);
}

#[test]
fn a_second_placement_replaces_the_first() {
    let params = default_pack().remove(0);
    let pose = Pose::new(&params, &placer(), &Ground);
    let mut m = Manager::default();
    m.add([1, 1, 1], 7, pose.clone(), true, false);
    m.add([1, 1, 1], 8, pose, true, false);
    m.tick(|_| 8);
    assert_eq!(m.particles().len(), 1);
    assert_eq!(m.particles()[0].state, 8);
    m.clear();
    assert_eq!(m.hidden_count(), 0);
}

#[test]
fn time_is_the_fraction_of_the_lifetime() {
    let params = default_pack().remove(0);
    let pose = Pose::new(&params, &placer(), &Ground);
    let mut m = Manager::default();
    m.add([0, 0, 0], 1, pose, true, false);
    let p = &m.particles()[0];
    assert!((p.time(0.0)).abs() < 1e-6);
    assert!((p.time(2.0) - 0.5).abs() < 1e-6);
    assert_eq!(p.time(100.0), 1.0);
}
