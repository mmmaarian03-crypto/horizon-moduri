use horizon_clumps::{Box3, Clump, Clumps, Orb, payout, scan_due, single, total};

/// An experience orb of the fake world.
#[derive(Clone, Debug, PartialEq)]
struct O {
    id: u32,
    pos: [f64; 3],
    value: u32,
    age: u32,
    ticks: u32,
    count: u32,
}
impl O {
    fn new(id: u32, pos: [f64; 3], value: u32) -> Self {
        O { id, pos, value, age: 0, ticks: 0, count: 1 }
    }
}
impl Orb for O {
    fn id(&self) -> u32 {
        self.id
    }
    fn value(&self) -> u32 {
        self.value
    }
    fn set_value(&mut self, value: u32) {
        self.value = value;
    }
    fn age(&self) -> u32 {
        self.age
    }
    fn set_age(&mut self, age: u32) {
        self.age = age;
    }
    fn ticks(&self) -> u32 {
        self.ticks
    }
    fn count(&self) -> u32 {
        self.count
    }
    fn set_count(&mut self, count: u32) {
        self.count = count;
    }
    /// ExperienceOrb: 0.5 x 0.5 x 0.5, feet at the position.
    fn bounds(&self) -> Box3 {
        Box3 {
            min: [self.pos[0] - 0.25, self.pos[1], self.pos[2] - 0.25],
            max: [self.pos[0] + 0.25, self.pos[1] + 0.5, self.pos[2] + 0.25],
        }
    }
}

fn map(pairs: &[(u32, u32)]) -> Clump {
    pairs.iter().copied().collect()
}

/// The player's pickup box (1.0, 0.5, 1.0 around a 0.6 x 1.8 player at `feet`).
fn player_box(feet: [f64; 3]) -> Box3 {
    Box3 {
        min: [feet[0] - 0.3 - 1.0, feet[1] - 0.5, feet[2] - 0.3 - 1.0],
        max: [feet[0] + 0.3 + 1.0, feet[1] + 1.8 + 0.5, feet[2] + 0.3 + 1.0],
    }
}

// ---- tryMergeToExisting (award) ----------------------------------------------------------

#[test]
fn a_fresh_orb_is_a_clump_of_itself() {
    let c = Clumps::default();
    let o = O::new(1, [0.0; 3], 17);
    assert_eq!(c.clump_of(&o), map(&[(17, 1)]));
    assert_eq!(single(17), map(&[(17, 1)]));
    assert_eq!(total(&map(&[(17, 1), (3, 4)])), 29);
}

#[test]
fn an_awarded_orb_joins_any_orb_in_the_box_whatever_its_size() {
    let mut c = Clumps::default();
    let mut orbs = vec![O::new(1, [10.0, 64.0, 10.0], 3)];
    orbs[0].age = 1234;
    // Vanilla needs the same value and ids a multiple of 40 apart; the original wants neither.
    assert!(c.award(&mut orbs, [10.2, 64.0, 10.0], 17));
    assert_eq!(orbs.len(), 1, "no new orb");
    assert_eq!(orbs[0].value, 20, "the orb is one orb of 3 and one of 17");
    assert_eq!(orbs[0].count, 1);
    assert_eq!(orbs[0].age, 0, "its age restarts");
    assert_eq!(c.clump_of(&orbs[0]), map(&[(3, 1), (17, 1)]));
    // And a third, equal to the first.
    assert!(c.award(&mut orbs, [10.0, 64.0, 10.0], 3));
    assert_eq!(orbs[0].value, 23);
    assert_eq!(c.clump_of(&orbs[0]), map(&[(3, 2), (17, 1)]));
}

#[test]
fn the_box_around_the_spot_is_one_block_wide_with_strict_edges() {
    // `AABB.ofSize(pos, 1, 1, 1)` against an orb box of 0.5: a centre up to 0.75 away overlaps.
    let at = [10.0, 64.0, 10.0];
    let hit = |dx: f64, dy: f64| {
        let mut c = Clumps::default();
        let mut orbs = vec![O::new(1, [10.0 + dx, 64.0 + dy, 10.0], 1)];
        c.award(&mut orbs, at, 1)
    };
    assert!(hit(0.74, 0.0));
    assert!(!hit(0.75, 0.0), "touching is not overlapping");
    assert!(hit(-0.74, 0.0));
    assert!(!hit(-0.75, 0.0));
    // Vertically the orb box stands on its position: feet .. +0.5, against the box at y +- 0.5.
    assert!(hit(0.0, 0.49));
    assert!(!hit(0.0, 0.5));
    assert!(hit(0.0, -0.99));
    assert!(!hit(0.0, -1.0));
}

#[test]
fn nothing_in_the_box_means_a_fresh_orb() {
    let mut c = Clumps::default();
    let mut orbs = vec![O::new(1, [20.0, 64.0, 20.0], 3)];
    assert!(!c.award(&mut orbs, [10.0, 64.0, 10.0], 17));
    assert_eq!(orbs[0].value, 3, "untouched");
    assert!(c.is_empty());
}

#[test]
fn the_first_orb_of_the_list_takes_the_amount() {
    let mut c = Clumps::default();
    let mut orbs = vec![
        O::new(1, [10.0, 64.0, 10.0], 3),
        O::new(2, [10.1, 64.0, 10.0], 7),
    ];
    assert!(c.award(&mut orbs, [10.0, 64.0, 10.0], 1));
    assert_eq!((orbs[0].value, orbs[1].value), (4, 7));
}

// ---- scanForMerges -----------------------------------------------------------------------

#[test]
fn the_scan_runs_on_the_first_tick_and_every_twentieth_after() {
    assert!(scan_due(1));
    assert!(!scan_due(2));
    assert!(!scan_due(20));
    assert!(scan_due(21));
    assert!(scan_due(41));
    assert!(!scan_due(0), "tickCount 0 never ticks");
}

#[test]
fn due_orbs_take_in_any_orb_within_half_a_block() {
    let mut c = Clumps::default();
    // ticks counts the ticks gone by; the orb is on its first tick (ticks 0) so it scans now.
    let mut a = O::new(1, [10.0, 64.0, 10.0], 3);
    a.age = 50;
    let mut b = O::new(2, [10.6, 64.0, 10.0], 17);
    b.age = 20;
    let mut orbs = vec![a, b];
    let taken = c.before_tick(&mut orbs, None);
    assert!(taken.is_empty());
    assert_eq!(orbs.len(), 1);
    assert_eq!(orbs[0].id, 1, "the orb that scans keeps its place");
    assert_eq!(orbs[0].value, 20);
    assert_eq!(orbs[0].age, 20, "the smaller age");
    assert_eq!(c.clump_of(&orbs[0]), map(&[(3, 1), (17, 1)]));
    assert_eq!(c.len(), 1, "the orb that was taken in keeps no clump");
}

#[test]
fn an_orb_not_due_scans_nothing_but_can_be_taken_in() {
    let mut c = Clumps::default();
    let mut a = O::new(1, [10.0, 64.0, 10.0], 3);
    a.ticks = 5;
    let b = O::new(2, [10.6, 64.0, 10.0], 17);
    // a's tick count would be 6, b's 1: only b scans, and it takes a in.
    let mut orbs = vec![a, b];
    c.before_tick(&mut orbs, None);
    assert_eq!(orbs.len(), 1);
    assert_eq!(orbs[0].id, 2);
    assert_eq!(orbs[0].value, 20);
    // Nobody due: nothing happens.
    let mut x = O::new(3, [0.0; 3], 1);
    let mut y = O::new(4, [0.1, 0.0, 0.0], 1);
    (x.ticks, y.ticks) = (6, 7);
    let mut orbs = vec![x, y];
    c.before_tick(&mut orbs, None);
    assert_eq!(orbs.len(), 2);
}

#[test]
fn the_twenty_first_tick_scans_again() {
    let mut c = Clumps::default();
    let mut a = O::new(1, [0.0; 3], 1);
    let mut b = O::new(2, [0.3, 0.0, 0.0], 1);
    (a.ticks, b.ticks) = (19, 19);
    let mut orbs = vec![a.clone(), b.clone()];
    c.before_tick(&mut orbs, None);
    assert_eq!(orbs.len(), 2, "tick 20: not due");
    (a.ticks, b.ticks) = (20, 20);
    let mut orbs = vec![a, b];
    c.before_tick(&mut orbs, None);
    assert_eq!(orbs.len(), 1, "tick 21: due");
}

#[test]
fn the_reach_is_half_a_block_beyond_the_orbs_box() {
    // The scanning orb's box inflated by 0.5, against the other's box: centres 1.5 apart touch.
    let near = |dx: f64| {
        let mut c = Clumps::default();
        let mut orbs = vec![O::new(1, [0.0; 3], 1), O::new(2, [dx, 0.0, 0.0], 1)];
        c.before_tick(&mut orbs, None);
        orbs.len() == 1
    };
    assert!(near(0.99));
    assert!(near(0.9999));
    assert!(!near(1.0), "boxes 0.5 wide, 0.5 apart: touching only");
    assert!(!near(1.2));
}

#[test]
fn a_crowd_becomes_one_orb() {
    let mut c = Clumps::default();
    let mut orbs: Vec<O> = (0..10)
        .map(|i| O::new(i + 1, [f64::from(i) * 0.05, 64.0, 0.0], [1, 3, 7][(i % 3) as usize]))
        .collect();
    let sum: u32 = orbs.iter().map(|o| o.value).sum();
    c.before_tick(&mut orbs, None);
    assert_eq!(orbs.len(), 1);
    assert_eq!(orbs[0].value, sum);
    assert_eq!(total(&c.clump_of(&orbs[0])), sum);
    assert_eq!(c.len(), 1, "one clump, none left for the orbs that were taken in");
}

#[test]
fn an_orb_taken_in_does_not_scan_after_it_is_gone() {
    // [A, B, C] all due. A (0) takes B (0.9 away). B would reach C (1.8), but B is gone and does
    // not scan; C is out of A's reach and A is out of C's.
    let mut c = Clumps::default();
    let mut orbs = vec![
        O::new(1, [0.0, 0.0, 0.0], 1),
        O::new(2, [0.9, 0.0, 0.0], 3),
        O::new(3, [1.8, 0.0, 0.0], 7),
    ];
    c.before_tick(&mut orbs, None);
    assert_eq!(orbs.iter().map(|o| (o.id, o.value)).collect::<Vec<_>>(), [(1, 4), (3, 7)]);
    assert_eq!(c.clump_of(&orbs[0]), map(&[(1, 1), (3, 1)]));
    assert_eq!(c.clump_of(&orbs[1]), single(7));
}

#[test]
fn far_orbs_stay_apart() {
    let mut c = Clumps::default();
    let mut orbs = vec![O::new(1, [0.0; 3], 1), O::new(2, [5.0, 0.0, 0.0], 1)];
    c.before_tick(&mut orbs, None);
    assert_eq!(orbs.len(), 2);
}

// ---- playerTouch (pickup) ----------------------------------------------------------------

#[test]
fn every_orb_that_touches_the_player_is_taken_at_once() {
    let mut c = Clumps::default();
    let mut orbs = vec![
        O::new(1, [0.0, 64.0, 0.0], 3),
        O::new(2, [5.0, 64.0, 5.0], 7),
        O::new(3, [0.5, 64.0, 0.5], 17),
    ];
    for o in &mut orbs {
        o.ticks = 5; // nobody scans this tick
    }
    let area = player_box([0.0, 64.0, 0.0]);
    let taken = c.before_tick(&mut orbs, Some(&area));
    assert_eq!(taken.iter().map(|t| t.orb.id).collect::<Vec<_>>(), [1, 3], "in the list's order, no delay");
    assert_eq!(orbs.iter().map(|o| o.id).collect::<Vec<_>>(), [2], "the far orb stays");
}

#[test]
fn a_taken_clump_comes_with_its_map_and_leaves_nothing_behind() {
    let mut c = Clumps::default();
    let mut orbs = vec![O::new(1, [0.0, 64.0, 0.0], 3)];
    orbs[0].ticks = 5;
    assert!(c.award(&mut orbs, [0.0, 64.0, 0.0], 7));
    assert!(c.award(&mut orbs, [0.0, 64.0, 0.0], 7));
    assert_eq!(c.len(), 1);
    let taken = c.before_tick(&mut orbs, Some(&player_box([0.0, 64.0, 0.0])));
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].clump, map(&[(3, 1), (7, 2)]));
    assert_eq!(taken[0].orb.value, 17);
    assert!(c.is_empty());
    assert!(orbs.is_empty());
}

#[test]
fn nobody_to_take_them_leaves_the_orbs() {
    // A dead player or a spectator has no pickup box.
    let mut c = Clumps::default();
    let mut orbs = vec![O::new(1, [0.0, 64.0, 0.0], 3)];
    orbs[0].ticks = 5;
    assert!(c.before_tick(&mut orbs, None).is_empty());
    assert_eq!(orbs.len(), 1);
}

#[test]
fn the_pickup_gives_every_orb_of_the_clump_through_the_repair() {
    let clump = map(&[(1, 2), (3, 1), (17, 1)]);
    let mut calls = Vec::new();
    // No Mending: each orb comes back whole.
    let give = payout(&clump, |size| {
        calls.push(size);
        size
    });
    assert_eq!(give, 2 + 3 + 17);
    assert_eq!(calls, [1, 1, 3, 17], "one repair per orb, by size");
    // Mending that eats half of each orb (what is left over is given).
    let give = payout(&clump, |size| size - size / 2);
    assert_eq!(give, (1 - 0) * 2 + (3 - 1) + (17 - 8));
    // Mending that eats everything.
    assert_eq!(payout(&clump, |_| 0), 0);
}

#[test]
fn an_orb_of_size_zero_gives_nothing() {
    // A summoned orb has value 0: `if (this.value != 0 || resolve())` finds nothing to give.
    let mut asked = 0;
    assert_eq!(
        payout(&single(0), |size| {
            asked += 1;
            size
        }),
        0
    );
    assert_eq!(asked, 0);
}

// ---- loading, bookkeeping ----------------------------------------------------------------

#[test]
fn an_orb_loaded_with_a_count_becomes_the_clump_the_original_reads() {
    // `Value` 7, `Count` 3 -> {7: 3}, value 21; the game's own count goes back to 1.
    let mut c = Clumps::default();
    let mut o = O::new(1, [0.0; 3], 7);
    o.count = 3;
    o.ticks = 5;
    let mut orbs = vec![o, O::new(2, [9.0, 0.0, 0.0], 5)];
    orbs[1].ticks = 5;
    c.before_tick(&mut orbs, None);
    assert_eq!((orbs[0].value, orbs[0].count), (21, 1));
    assert_eq!(c.clump_of(&orbs[0]), map(&[(7, 3)]));
    assert_eq!((orbs[1].value, orbs[1].count), (5, 1), "an ordinary orb is left as it is");
}

#[test]
fn a_map_that_no_longer_adds_up_is_not_trusted() {
    let mut c = Clumps::default();
    let mut orbs = vec![O::new(1, [0.0; 3], 3)];
    assert!(c.award(&mut orbs, [0.0; 3], 7));
    // Something else changed the value of the orb.
    orbs[0].value = 100;
    assert_eq!(c.clump_of(&orbs[0]), single(100));
}

#[test]
fn a_clump_is_dropped_when_the_orb_goes() {
    let mut c = Clumps::default();
    let mut orbs = vec![O::new(1, [0.0; 3], 3), O::new(2, [50.0, 0.0, 0.0], 3)];
    c.award(&mut orbs, [0.0; 3], 7);
    c.award(&mut orbs, [50.0, 0.0, 0.0], 7);
    assert_eq!(c.len(), 2);
    c.forget(2);
    assert_eq!(c.len(), 1);
    c.prune(|id| id != 1);
    assert!(c.is_empty());
}

#[test]
fn a_clump_is_stored_only_while_it_holds_more_than_one_orb() {
    let mut c = Clumps::default();
    let mut orbs = vec![O::new(1, [0.0; 3], 3)];
    c.award(&mut orbs, [0.0; 3], 7);
    assert_eq!(c.len(), 1);
}

// ---- nothing is lost ---------------------------------------------------------------------

struct Lcg(u64);
impl Lcg {
    fn next(&mut self, bound: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) % u64::from(bound)) as u32
    }
}

#[test]
fn the_experience_is_the_same_however_the_orbs_merged() {
    for seed in 1..40u64 {
        let mut rng = Lcg(seed);
        let mut c = Clumps::default();
        let mut orbs: Vec<O> = Vec::new();
        let mut next_id = 1;
        let (mut awarded, mut given) = (0u64, 0u64);
        for tick in 0..300 {
            // New experience somewhere in a 4 x 4 block patch, in the sizes the game uses.
            for _ in 0..rng.next(3) {
                let size = [1, 3, 7, 17, 37, 73, 149][rng.next(7) as usize];
                let at = [f64::from(rng.next(40)) / 10.0, 64.0, f64::from(rng.next(40)) / 10.0];
                awarded += u64::from(size);
                if !c.award(&mut orbs, at, size) {
                    orbs.push(O::new(next_id, at, size));
                    next_id += 1;
                }
            }
            // Every orb gets older.
            for o in &mut orbs {
                o.ticks += 1;
                o.age += 1;
            }
            // Now and then the player stands in the patch.
            let area = (tick % 37 == 36).then(|| player_box([2.0, 64.0, 2.0]));
            for t in c.before_tick(&mut orbs, area.as_ref()) {
                assert_eq!(total(&t.clump), t.orb.value);
                given += u64::from(payout(&t.clump, |size| size));
            }
        }
        // The player gathers whatever is left.
        let rest = Box3 { min: [-100.0; 3], max: [100.0; 3] };
        for t in c.before_tick(&mut orbs, Some(&rest)) {
            assert_eq!(total(&t.clump), t.orb.value);
            given += u64::from(payout(&t.clump, |size| size));
        }
        assert!(orbs.is_empty());
        assert!(c.is_empty(), "no clump outlives its orb (seed {seed})");
        assert_eq!(given, awarded, "seed {seed}: nothing gained, nothing lost");
    }
}

#[test]
fn the_pack_never_makes_a_count_above_one() {
    // The game's own pickup would give `value` once for each count; a clump is one pickup.
    let mut c = Clumps::default();
    let mut orbs: Vec<O> = (0..6).map(|i| O::new(i + 1, [0.1 * f64::from(i), 0.0, 0.0], 3)).collect();
    c.before_tick(&mut orbs, None);
    c.award(&mut orbs, [0.0; 3], 3);
    for o in &orbs {
        assert_eq!(o.count, 1);
    }
}

#[test]
fn boxes_use_strict_edges() {
    let a = Box3::around([0.0; 3], 1.0);
    assert_eq!(a, Box3 { min: [-0.5; 3], max: [0.5; 3] });
    let touching = Box3 { min: [0.5, -0.1, -0.1], max: [1.0, 0.1, 0.1] };
    assert!(!a.intersects(&touching));
    let overlapping = Box3 { min: [0.4, -0.1, -0.1], max: [1.0, 0.1, 0.1] };
    assert!(a.intersects(&overlapping));
    assert_eq!(a.inflate(0.5), Box3 { min: [-1.0; 3], max: [1.0; 3] });
}
