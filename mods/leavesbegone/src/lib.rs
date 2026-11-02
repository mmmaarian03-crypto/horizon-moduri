// Leaves Be Gone for Horizon: the rules, without any game code.
// Written from the behaviour of Leaves Be Gone 8.0.0 for Minecraft 1.20.1 (Fuzs, MPL-2.0,
// https://github.com/Fuzss/leavesbegone, branch 1.20.1, read for private use only) and from the
// pack's `leavesbegone-server.toml`; no code of the original is used.
//
// What the original does: it keeps a second queue of ticks per level (a `LevelTicks<Block>` with a
// container per chunk, saved with the chunk). When a scheduled block tick (`ServerLevel.tickBlock`)
// turns a leaves block that was NOT randomly ticking into one that is (its `distance` reached 7 and
// it is not `persistent`: the log that held it is gone), the original puts that block in the queue
// with a delay of `minimum + nextInt(max(1, maximum - minimum))` ticks. Right after the block ticks
// of a level, the queue runs what is due: if the position still holds that block, the block gets
// the random tick vanilla would give it only now and then, so the leaves fall (drops included) a
// few ticks after the tree is cut instead of minutes later. Vanilla's random ticks go on as before.
use std::collections::{BTreeMap, BTreeSet};

pub type Pos = [i32; 3];

/// `leavesbegone-server.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// "Minimum ticks after which leaves will start to decay."
    pub minimum_decay_ticks: i32,
    /// "Maximum ticks it takes for leaves to decay."
    pub maximum_decay_ticks: i32,
}
impl Default for Settings {
    /// The pack's values (the file holds the original's defaults: `minimum_decay_ticks = 5`,
    /// `maximum_decay_ticks = 20`).
    fn default() -> Self {
        Self { minimum_decay_ticks: 5, maximum_decay_ticks: 20 }
    }
}

/// The delay of a decay: `minimum + random.nextInt(Math.max(1, maximum - minimum))`. `roll(bound)`
/// is `nextInt(bound)` of the level's random; the bound is always at least 1.
pub fn decay_delay(s: &Settings, roll: impl FnOnce(i32) -> i32) -> i64 {
    let bound = s.maximum_decay_ticks.saturating_sub(s.minimum_decay_ticks).max(1);
    i64::from(s.minimum_decay_ticks) + i64::from(roll(bound))
}

/// What a scheduled block tick did to the block it ticked, as `ServerLevel.tickBlock` sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Before the tick: the position held the ticked block and it is in `#minecraft:leaves`.
    pub leaves: bool,
    /// Before the tick: `state.isRandomlyTicking()`.
    pub was_randomly_ticking: bool,
    /// After the tick: `state.isRandomlyTicking()` of what is at the position now.
    pub now_randomly_ticking: bool,
}

/// A block waiting for its extra random tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub pos: Pos,
    /// The block's registry id (`ScheduledTick.type`).
    pub block: u32,
    /// The game time it comes due.
    pub due: i64,
    order: u64,
}

/// One level's queue (`RandomBlockTickerLevel.leavesbegone$getRandomBlockTicks`). Like vanilla's
/// tick queues, a block already waiting is not queued again, whatever the new timing.
#[derive(Clone, Debug, Default)]
pub struct Queue {
    entries: BTreeMap<(i64, u64), Entry>,
    waiting: BTreeSet<(Pos, u32)>,
    next_order: u64,
}

impl Queue {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn contains(&self, pos: Pos, block: u32) -> bool {
        self.waiting.contains(&(pos, block))
    }
    /// `LevelTicks.schedule`: false when the block is already waiting at that position.
    pub fn schedule(&mut self, pos: Pos, block: u32, due: i64) -> bool {
        if !self.waiting.insert((pos, block)) {
            return false;
        }
        let order = self.next_order;
        self.next_order += 1;
        self.entries.insert((due, order), Entry { pos, block, due, order });
        true
    }
    /// `LevelTicks.tick(gameTime, maxTicks, ..)` collecting step: the entries due at `now`, earliest
    /// first, at most `max`, leaving out (and keeping queued) those in a chunk that does not tick
    /// (`isPositionTickingWithEntitiesLoaded`). Nothing queued during the run is in the result.
    pub fn drain_due(
        &mut self,
        now: i64,
        max: usize,
        mut ticking: impl FnMut(Pos) -> bool,
    ) -> Vec<Entry> {
        let mut taken = Vec::new();
        for (&key, e) in self.entries.range(..(now.saturating_add(1), 0)) {
            if taken.len() >= max {
                break;
            }
            if ticking(e.pos) {
                taken.push(key);
            }
        }
        taken
            .into_iter()
            .filter_map(|k| {
                let e = self.entries.remove(&k)?;
                self.waiting.remove(&(e.pos, e.block));
                Some(e)
            })
            .collect()
    }
}

/// The original's two `tickBlock` injections: after a scheduled tick of `block` at `pos`, queue the
/// decay if a leaves block went from not randomly ticking to randomly ticking. Returns whether it
/// queued one (the level's random is drawn only then).
pub fn after_block_tick(
    s: &Settings,
    queue: &mut Queue,
    pos: Pos,
    block: u32,
    now: i64,
    outcome: Outcome,
    roll: impl FnOnce(i32) -> i32,
) -> bool {
    if outcome.leaves && !outcome.was_randomly_ticking && outcome.now_randomly_ticking {
        queue.schedule(pos, block, now.saturating_add(decay_delay(s, roll)))
    } else {
        false
    }
}

/// `ServerLevel.tick` hands the queue at most this many ticks per game tick.
pub const MAX_TICKS_PER_TICK: usize = 65536;

/// The queues of the levels of one world. A different world (or the clock running backwards, which
/// a new world's does) forgets everything: the original's queues belong to their level and chunks.
#[derive(Clone, Debug, Default)]
pub struct Queues {
    world: Option<i64>,
    last_time: i64,
    levels: BTreeMap<u8, Queue>,
}

impl Queues {
    /// Call once per game tick with the world's seed and the game time.
    pub fn sync(&mut self, world: i64, now: i64) {
        if self.world != Some(world) || now < self.last_time {
            self.levels.clear();
            self.world = Some(world);
        }
        self.last_time = now;
    }
    /// The queue of a level (0 overworld, 1 nether, 2 end).
    pub fn level(&mut self, level: u8) -> &mut Queue {
        self.levels.entry(level).or_default()
    }
}
