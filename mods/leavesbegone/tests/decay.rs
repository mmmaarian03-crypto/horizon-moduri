use horizon_leavesbegone::{
    MAX_TICKS_PER_TICK, Outcome, Pos, Queue, Queues, Settings, after_block_tick, decay_delay,
};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------------------------
// A fake world with vanilla's leaves rules (LeavesBlock of 1.20.1), driven the way the game
// drives it: scheduled block ticks, then Leaves Be Gone's queue, once per game tick.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum B {
    Air,
    Log,
    Stone,
    Leaves { distance: u8, persistent: bool },
}

/// `Block.id` of leaves in the fake registry (`ScheduledTick.type`).
const LEAVES: u32 = 7;

struct Lcg(u64);
impl Lcg {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) % bound as u64) as i32
    }
}

struct World {
    blocks: BTreeMap<Pos, B>,
    now: i64,
    /// `ServerLevel.blockTicks`: (trigger time, order) -> position; a position is queued once.
    ticks: BTreeMap<(i64, u64), Pos>,
    queued: BTreeSet<Pos>,
    order: u64,
    /// Leaves Be Gone: its settings and queue (`None` = the mod is not installed).
    lbg: Option<(Settings, Queue)>,
    random: Lcg,
    /// Which chunks tick; everything does when empty.
    frozen_chunks: BTreeSet<(i32, i32)>,
    /// When each leaves block became randomly ticking, and when it fell.
    became_decaying: BTreeMap<Pos, i64>,
    fell: Vec<(i64, Pos)>,
    /// How many times the level's random was drawn by Leaves Be Gone.
    rolls: u32,
}

fn around(p: Pos) -> [Pos; 6] {
    [
        [p[0] - 1, p[1], p[2]],
        [p[0] + 1, p[1], p[2]],
        [p[0], p[1] - 1, p[2]],
        [p[0], p[1] + 1, p[2]],
        [p[0], p[1], p[2] - 1],
        [p[0], p[1], p[2] + 1],
    ]
}

impl World {
    fn new(lbg: bool) -> Self {
        World {
            blocks: BTreeMap::new(),
            now: 0,
            ticks: BTreeMap::new(),
            queued: BTreeSet::new(),
            order: 0,
            lbg: lbg.then(|| (Settings::default(), Queue::default())),
            random: Lcg(12345),
            frozen_chunks: BTreeSet::new(),
            became_decaying: BTreeMap::new(),
            fell: Vec::new(),
            rolls: 0,
        }
    }
    fn get(&self, p: Pos) -> B {
        self.blocks.get(&p).copied().unwrap_or(B::Air)
    }
    /// `LeavesBlock.getDistanceAt`.
    fn distance_at(&self, p: Pos) -> u8 {
        match self.get(p) {
            B::Log => 0,
            B::Leaves { distance, .. } => distance,
            _ => 7,
        }
    }
    /// `LeavesBlock.updateDistance`.
    fn measured(&self, p: Pos) -> u8 {
        let mut i = 7;
        for n in around(p) {
            i = i.min(self.distance_at(n) + 1);
            if i == 1 {
                break;
            }
        }
        i
    }
    /// `BlockState.isRandomlyTicking` of leaves: distance 7 and not persistent.
    fn randomly_ticking(b: B) -> bool {
        matches!(b, B::Leaves { distance: 7, persistent: false })
    }
    fn schedule(&mut self, p: Pos, delay: i64) {
        if self.queued.insert(p) {
            self.ticks.insert((self.now + delay, self.order), p);
            self.order += 1;
        }
    }
    /// `Level.setBlock(pos, state, 3)`: every neighbour's `updateShape` runs.
    fn set(&mut self, p: Pos, b: B) {
        if b == B::Air {
            self.blocks.remove(&p);
        } else {
            self.blocks.insert(p, b);
        }
        for n in around(p) {
            if let B::Leaves { distance, .. } = self.get(n) {
                // `LeavesBlock.updateShape`: i = distance at the changed block + 1;
                // `if (i != 1 || distance != i) scheduleTick(pos, this, 1)`.
                let i = self.distance_at(p) + 1;
                if i != 1 || distance != i {
                    self.schedule(n, 1);
                }
            }
        }
    }
    /// Start from a settled world: every leaves block holds its measured distance.
    fn settle(&mut self) {
        loop {
            let leaves: Vec<Pos> = self
                .blocks
                .iter()
                .filter(|(_, b)| matches!(b, B::Leaves { .. }))
                .map(|(p, _)| *p)
                .collect();
            let mut changed = false;
            for p in leaves {
                let B::Leaves { distance, persistent } = self.get(p) else { continue };
                let m = self.measured(p);
                if m != distance {
                    self.blocks.insert(p, B::Leaves { distance: m, persistent });
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        self.ticks.clear();
        self.queued.clear();
    }
    fn ticking(&self, p: Pos) -> bool {
        !self.frozen_chunks.contains(&(p[0] >> 4, p[2] >> 4))
    }
    /// One game tick: `ServerLevel.tick`'s block ticks, Leaves Be Gone's queue, (no fluids).
    fn tick(&mut self) {
        self.now += 1;
        let due: Vec<(i64, u64)> = self.ticks.range(..(self.now + 1, 0)).map(|(k, _)| *k).collect();
        for k in due {
            let Some(p) = self.ticks.remove(&k) else { continue };
            self.queued.remove(&p);
            if !self.ticking(p) {
                // not ticking: stays queued (the game keeps it for later)
                self.ticks.insert(k, p);
                self.queued.insert(p);
                continue;
            }
            // ServerLevel.tickBlock: only if the position still holds the scheduled block.
            let before = self.get(p);
            let B::Leaves { persistent, .. } = before else { continue };
            // LeavesBlock.tick: setBlock(pos, updateDistance(state, level, pos), 3)
            let after = B::Leaves { distance: self.measured(p), persistent };
            self.set(p, after);
            if Self::randomly_ticking(after) && !Self::randomly_ticking(before) {
                self.became_decaying.entry(p).or_insert(self.now);
            }
            if let Some((settings, queue)) = self.lbg.as_mut() {
                let random = &mut self.random;
                let rolls = &mut self.rolls;
                after_block_tick(
                    settings,
                    queue,
                    p,
                    LEAVES,
                    self.now,
                    Outcome {
                        leaves: true,
                        was_randomly_ticking: Self::randomly_ticking(before),
                        now_randomly_ticking: Self::randomly_ticking(after),
                    },
                    |bound| {
                        *rolls += 1;
                        random.next_int(bound)
                    },
                );
            }
        }
        // Leaves Be Gone: right after the block ticks.
        if let Some((_, queue)) = self.lbg.as_mut() {
            let frozen = self.frozen_chunks.clone();
            let due = queue.drain_due(self.now, MAX_TICKS_PER_TICK, |p| {
                !frozen.contains(&(p[0] >> 4, p[2] >> 4))
            });
            for e in due {
                // blockstate.is(block) -> randomTick: LeavesBlock decays if distance 7 and not persistent.
                if let B::Leaves { .. } = self.get(e.pos) {
                    if Self::randomly_ticking(self.get(e.pos)) {
                        self.fell.push((self.now, e.pos));
                        self.set(e.pos, B::Air);
                    }
                }
            }
        }
    }
    fn run(&mut self, ticks: i64) {
        for _ in 0..ticks {
            self.tick();
        }
    }
    fn leaves(&self) -> usize {
        self.blocks.values().filter(|b| matches!(b, B::Leaves { .. })).count()
    }
}

/// An oak: a trunk of `height` logs at (x, 0, z), a 5x5 crown over its top two logs and a 3x3 cap.
fn oak(w: &mut World, x: i32, z: i32, height: i32, persistent: bool) {
    for y in 0..height {
        w.blocks.insert([x, y, z], B::Log);
    }
    for y in height - 2..height {
        for dx in -2..=2 {
            for dz in -2..=2 {
                w.blocks
                    .entry([x + dx, y, z + dz])
                    .or_insert(B::Leaves { distance: 1, persistent });
            }
        }
    }
    for dx in -1..=1 {
        for dz in -1..=1 {
            w.blocks
                .entry([x + dx, height, z + dz])
                .or_insert(B::Leaves { distance: 1, persistent });
        }
    }
}

fn cut(w: &mut World, x: i32, z: i32, height: i32) {
    for y in 0..height {
        w.set([x, y, z], B::Air);
    }
}

// ---------------------------------------------------------------------------------------------
// The whole behaviour: cut a tree, the crown falls a few ticks after each leaf lost its log.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_cut_trees_crown_falls_within_the_configured_delay() {
    let mut w = World::new(true);
    oak(&mut w, 0, 0, 5, false);
    w.settle();
    let crown = w.leaves();
    assert_eq!(crown, 24 + 24 + 9, "two 5x5 layers round the trunk and the 3x3 cap");
    // Nothing falls while the tree stands (every leaf is held at distance 1..2).
    w.run(100);
    assert_eq!(w.leaves(), crown);
    assert!(w.fell.is_empty());
    // Cut the whole trunk.
    cut(&mut w, 0, 0, 5);
    w.run(200);
    assert_eq!(w.leaves(), 0, "every leaf of the crown decayed");
    assert_eq!(w.fell.len(), crown);
    // Each leaf fell between minimum (5) and maximum - 1 (19) ticks after it became randomly ticking.
    for (when, pos) in &w.fell {
        let since = when - w.became_decaying[pos];
        assert!((5..=19).contains(&since), "{pos:?} fell {since} ticks after it began decaying");
    }
    // And no random tick of the world helped: the sim has none, so the queue did all of it.
    assert_eq!(w.rolls as usize, crown, "one roll of the level's random per leaf queued");
}

#[test]
fn without_the_mod_the_leaves_wait_for_random_ticks() {
    let mut w = World::new(false);
    oak(&mut w, 0, 0, 5, false);
    w.settle();
    let crown = w.leaves();
    cut(&mut w, 0, 0, 5);
    w.run(200);
    // They are all randomly ticking by now, but only vanilla's random ticks would remove them.
    assert_eq!(w.leaves(), crown);
    assert!(
        w.blocks.values().all(|b| matches!(b, B::Leaves { distance: 7, .. })),
        "all at distance 7"
    );
}

#[test]
fn leaves_placed_by_hand_never_decay() {
    let mut w = World::new(true);
    oak(&mut w, 0, 0, 5, true);
    w.settle();
    let crown = w.leaves();
    cut(&mut w, 0, 0, 5);
    w.run(300);
    // Persistent leaves reach distance 7 but are never randomly ticking: nothing is queued.
    assert_eq!(w.leaves(), crown);
    assert!(w.fell.is_empty());
    assert_eq!(w.rolls, 0);
}

#[test]
fn leaves_another_log_still_holds_stay() {
    let mut w = World::new(true);
    // Two trunks 2 apart share the middle of their crowns.
    oak(&mut w, 0, 0, 5, false);
    oak(&mut w, 2, 0, 5, false);
    w.settle();
    cut(&mut w, 0, 0, 5);
    w.run(200);
    // The second tree stands, so its whole crown is still there and nothing it holds fell.
    let held: BTreeSet<Pos> = w
        .blocks
        .iter()
        .filter(|(_, b)| matches!(b, B::Leaves { .. }))
        .map(|(p, _)| *p)
        .collect();
    assert!(!held.is_empty());
    for (p, b) in &w.blocks {
        if let B::Leaves { distance, .. } = b {
            assert!(*distance < 7, "{p:?} is held by the standing tree");
        }
    }
    for (_, pos) in &w.fell {
        assert!(!held.contains(pos));
    }
}

#[test]
fn a_leaf_held_again_before_its_turn_survives() {
    let mut w = World::new(true);
    oak(&mut w, 0, 0, 3, false);
    w.settle();
    cut(&mut w, 0, 0, 3);
    // Wait until some leaf is queued, then put a log back next to every leaf.
    while w.lbg.as_ref().unwrap().1.is_empty() {
        w.tick();
        assert!(w.now < 100);
    }
    let leaves: Vec<Pos> = w
        .blocks
        .iter()
        .filter(|(_, b)| matches!(b, B::Leaves { .. }))
        .map(|(p, _)| *p)
        .collect();
    // A scaffold of logs touching every leaf (each leaf is back at distance 1 once its tick runs).
    for p in &leaves {
        for n in around(*p) {
            if w.get(n) == B::Air {
                w.set(n, B::Log);
                break;
            }
        }
    }
    w.run(100);
    // Leaves queued earlier are given their random tick, but only decay if still at distance 7.
    for p in &leaves {
        if let B::Leaves { distance, .. } = w.get(*p) {
            assert_ne!(distance, 7, "{p:?} is held again");
        }
    }
}

#[test]
fn other_blocks_never_queue_anything() {
    let mut w = World::new(true);
    w.blocks.insert([0, 0, 0], B::Stone);
    w.blocks.insert([1, 0, 0], B::Log);
    w.set([1, 0, 0], B::Air);
    w.run(50);
    assert!(w.lbg.as_ref().unwrap().1.is_empty());
    assert_eq!(w.rolls, 0);
}

// ---------------------------------------------------------------------------------------------
// The rules, one by one.
// ---------------------------------------------------------------------------------------------

#[test]
fn the_delay_is_minimum_plus_a_roll_below_the_gap() {
    let pack = Settings::default();
    assert_eq!(pack, Settings { minimum_decay_ticks: 5, maximum_decay_ticks: 20 });
    let mut seen = BTreeSet::new();
    for r in 0..15 {
        seen.insert(decay_delay(&pack, |bound| {
            assert_eq!(bound, 15, "nextInt(20 - 5)");
            r
        }));
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), (5..=19).collect::<Vec<i64>>());
}

#[test]
fn the_roll_bound_is_never_below_one() {
    // Math.max(1, maximum - minimum): equal or inverted limits give the minimum itself.
    for (min, max) in [(10, 10), (10, 3), (0, 0), (0, 1)] {
        let s = Settings { minimum_decay_ticks: min, maximum_decay_ticks: max };
        let delay = decay_delay(&s, |bound| {
            assert_eq!(bound, 1);
            0
        });
        assert_eq!(delay, i64::from(min));
    }
    let wide = Settings { minimum_decay_ticks: 0, maximum_decay_ticks: 100 };
    assert_eq!(decay_delay(&wide, |bound| bound - 1), 99);
}

#[test]
fn only_a_leaf_that_just_began_to_decay_is_queued() {
    let s = Settings::default();
    let go = |leaves, was, now| {
        let mut q = Queue::default();
        let mut rolled = false;
        let queued = after_block_tick(
            &s,
            &mut q,
            [1, 2, 3],
            LEAVES,
            100,
            Outcome { leaves, was_randomly_ticking: was, now_randomly_ticking: now },
            |_| {
                rolled = true;
                0
            },
        );
        assert_eq!(queued, !q.is_empty());
        assert_eq!(rolled, queued, "the level's random is drawn only for a queued decay");
        queued
    };
    assert!(go(true, false, true), "not ticking -> ticking");
    assert!(!go(true, true, true), "already ticking: vanilla's random ticks have it");
    assert!(!go(true, false, false), "still held");
    assert!(!go(true, true, false), "became held");
    assert!(!go(false, false, true), "not leaves");
}

#[test]
fn a_decay_is_queued_at_game_time_plus_delay() {
    let s = Settings::default();
    let mut q = Queue::default();
    after_block_tick(
        &s,
        &mut q,
        [1, 2, 3],
        LEAVES,
        1000,
        Outcome { leaves: true, was_randomly_ticking: false, now_randomly_ticking: true },
        |_| 4,
    );
    assert!(q.contains([1, 2, 3], LEAVES));
    assert!(q.drain_due(1008, 10, |_| true).is_empty(), "due at 1000 + 5 + 4 = 1009");
    let due = q.drain_due(1009, 10, |_| true);
    assert_eq!(due.len(), 1);
    assert_eq!((due[0].pos, due[0].block, due[0].due), ([1, 2, 3], LEAVES, 1009));
    assert!(q.is_empty());
}

#[test]
fn a_block_already_waiting_is_not_queued_again() {
    let mut q = Queue::default();
    assert!(q.schedule([0, 0, 0], LEAVES, 50));
    assert!(!q.schedule([0, 0, 0], LEAVES, 10), "first schedule wins, whatever the timing");
    assert!(q.schedule([0, 0, 0], LEAVES + 1, 10), "another block at the position is another tick");
    assert_eq!(q.len(), 2);
    let due = q.drain_due(60, 10, |_| true);
    assert_eq!(due.iter().map(|e| e.due).collect::<Vec<_>>(), [10, 50]);
}

#[test]
fn due_entries_come_out_earliest_first_and_in_order() {
    let mut q = Queue::default();
    q.schedule([3, 0, 0], LEAVES, 12);
    q.schedule([1, 0, 0], LEAVES, 10);
    q.schedule([2, 0, 0], LEAVES, 10);
    q.schedule([4, 0, 0], LEAVES, 30);
    let due = q.drain_due(20, 10, |_| true);
    assert_eq!(due.iter().map(|e| e.pos[0]).collect::<Vec<_>>(), [1, 2, 3], "by time, then by when queued");
    assert_eq!(q.len(), 1, "the one due at 30 waits");
}

#[test]
fn at_most_the_limit_comes_out_per_tick() {
    let mut q = Queue::default();
    for x in 0..10 {
        q.schedule([x, 0, 0], LEAVES, 1);
    }
    assert_eq!(q.drain_due(1, 4, |_| true).len(), 4);
    assert_eq!(q.drain_due(1, 4, |_| true).len(), 4);
    assert_eq!(q.drain_due(1, 4, |_| true).len(), 2);
}

#[test]
fn a_chunk_that_does_not_tick_keeps_its_entries() {
    let mut q = Queue::default();
    q.schedule([0, 70, 0], LEAVES, 5); // chunk (0, 0)
    q.schedule([40, 70, 0], LEAVES, 5); // chunk (2, 0)
    let due = q.drain_due(10, 10, |p| p[0] >> 4 != 0);
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].pos, [40, 70, 0]);
    assert!(q.contains([0, 70, 0], LEAVES), "kept for when its chunk ticks again");
    assert_eq!(q.drain_due(11, 10, |_| true).len(), 1);
}

#[test]
fn leaves_in_a_frozen_chunk_fall_once_it_ticks_again() {
    let mut w = World::new(true);
    oak(&mut w, 0, 0, 3, false);
    w.settle();
    cut(&mut w, 0, 0, 3);
    w.run(2);
    w.frozen_chunks.insert((0, 0));
    w.run(100);
    assert!(w.leaves() > 0, "nothing ticks in a chunk that does not tick");
    w.frozen_chunks.clear();
    w.run(100);
    assert_eq!(w.leaves(), 0);
}

#[test]
fn a_new_world_forgets_the_old_queues() {
    let mut qs = Queues::default();
    qs.sync(42, 100);
    qs.level(0).schedule([0, 0, 0], LEAVES, 500);
    qs.level(1).schedule([1, 0, 0], LEAVES, 500);
    qs.sync(42, 101);
    assert_eq!(qs.level(0).len(), 1, "same world, time moves on");
    assert_eq!(qs.level(1).len(), 1, "each level has its own queue");
    qs.sync(42, 50);
    assert!(qs.level(0).is_empty(), "the clock went backwards: another world");
    qs.level(0).schedule([0, 0, 0], LEAVES, 500);
    qs.sync(43, 60);
    assert!(qs.level(0).is_empty(), "another seed");
}

#[test]
fn the_nether_and_the_overworld_do_not_share_a_queue() {
    let mut qs = Queues::default();
    qs.sync(1, 0);
    assert!(qs.level(0).schedule([5, 5, 5], LEAVES, 3));
    assert!(qs.level(1).schedule([5, 5, 5], LEAVES, 3), "the same position in another level");
}
