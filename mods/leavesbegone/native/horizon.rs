// Leaves Be Gone inside Horizon's game (nightly 142388e1c). The rules are the engine below; this
// part runs the game's scheduled block ticks one at a time (as `Game::tick_scheduled` does) to see
// what each did to a leaves block, and runs the engine's queue right after the block ticks.
use super::{BlockPos, BlockStateId, Dim, Game, Result};
#[allow(dead_code)]
mod engine {
    include!("leavesbegone_engine.rs");
}
use engine::{MAX_TICKS_PER_TICK, Outcome, Queues, Settings};
use horizon_core::{position::ChunkPos, random::RandomSource};
use horizon_world::level::LevelAccess;
use std::cell::RefCell;

thread_local! {
    /// The extra random ticks that wait, per level of the world this thread plays.
    static QUEUES: RefCell<Queues> = RefCell::new(Queues::default());
}

/// What one scheduled block tick did, for the leaves check: the position, the block, the state
/// before, whether it was randomly ticking before, and whether the state after is.
type Step = (BlockPos, u32, BlockStateId, bool, bool);

impl Game<'_> {
    /// `ServerLevel.tick`'s scheduled block ticks, the extra random ticks of Leaves Be Gone right
    /// after them, then the scheduled fluid ticks. The block ticks run one at a time, exactly as
    /// the game's own `tick_scheduled` runs them (the side effects of each happen before the next).
    pub(super) fn leavesbegone_tick_scheduled(&mut self) -> Result<()> {
        let settings = Settings::default();
        // The original runs on the server, where the level is: a remote server's world is its own.
        let active = self.remote.is_none();
        if active {
            let (seed, now) = (self.level.seed, self.world.game_time);
            QUEUES.with(|q| q.borrow_mut().sync(seed, now));
        }
        for fluids in [false, true] {
            if self.with_level(|b, level, ticks| b.scheduled_begin(level, ticks, fluids)) {
                loop {
                    let more = if fluids || !active {
                        self.with_level(|b, level, ticks| b.scheduled_step(level, ticks, fluids))
                    } else {
                        self.leavesbegone_block_step(&settings)
                    };
                    if !self.level_events.is_empty() {
                        self.process_level_events()?;
                    }
                    if !more {
                        break;
                    }
                }
                self.with_level(|b, _, ticks| b.scheduled_finish(ticks, fluids));
            }
            if !fluids && active {
                self.leavesbegone_run_due()?;
            }
        }
        Ok(())
    }

    /// 0 overworld, 1 nether, 2 end: the level a queue belongs to.
    fn leavesbegone_level(&self) -> u8 {
        match self.dim {
            Dim::Overworld => 0,
            Dim::Nether => 1,
            Dim::End => 2,
        }
    }

    /// `Behaviours::scheduled_step` for the block ticks, which also tells what the tick did to the
    /// block (`ServerLevel.tickBlock` with the original's two injections). False when no tick is left.
    fn leavesbegone_block_step(&mut self, settings: &Settings) -> bool {
        let step: Option<Option<Step>> = self.with_level(|b, level, ticks| {
            let Ok(Some(tick)) = ticks.blocks.next_tick() else {
                return None;
            };
            let before = level.block_state(tick.pos);
            let block = u32::from(b.block(before).0);
            if block != tick.kind.0 {
                // `tickBlock` only ticks a position that still holds the scheduled block.
                return Some(None);
            }
            let was = b.is_randomly_ticking(before);
            b.scheduled_block_tick(level, ticks, tick.pos);
            let after = level.block_state(tick.pos);
            Some(Some((tick.pos, block, before, was, b.is_randomly_ticking(after))))
        });
        let Some(step) = step else {
            return false;
        };
        let Some((pos, block, before, was, now)) = step else {
            return true;
        };
        // Only a block that was not randomly ticking and is now can be a decay to queue.
        if !was && now {
            let name = self.res.facts().get(before).name.clone();
            let leaves = self.res.items().block_in_tag("minecraft:leaves", &name);
            let outcome = Outcome {
                leaves,
                was_randomly_ticking: was,
                now_randomly_ticking: now,
            };
            let level = self.leavesbegone_level();
            let time = self.world.game_time;
            let random = &mut self.world.random;
            QUEUES.with(|q| {
                engine::after_block_tick(
                    settings,
                    q.borrow_mut().level(level),
                    [pos.x, pos.y, pos.z],
                    block,
                    time,
                    outcome,
                    |bound| random.next_int_bound(bound).unwrap_or(0),
                )
            });
        }
        true
    }

    /// The original's `leavesbegone$tick`: the entries that are due, collected first (what is queued
    /// meanwhile waits for the next game tick), each given the block's random tick if the position
    /// still holds that block.
    fn leavesbegone_run_due(&mut self) -> Result<()> {
        let level_id = self.leavesbegone_level();
        let now = self.world.game_time;
        let ran = self.with_level(|b, level, ticks| {
            let due = QUEUES.with(|q| {
                q.borrow_mut().level(level_id).drain_due(now, MAX_TICKS_PER_TICK, |p| {
                    level.is_ticking_chunk(ChunkPos::new(p[0] >> 4, p[2] >> 4))
                })
            });
            for e in &due {
                let pos = BlockPos::new(e.pos[0], e.pos[1], e.pos[2]);
                let state = level.block_state(pos);
                if u32::from(b.block(state).0) == e.block {
                    // `blockstate.randomTick(level, pos, random)`: a leaves block decays only if it
                    // is still not persistent and at distance 7.
                    b.random_tick(level, ticks, pos);
                }
            }
            !due.is_empty()
        });
        if ran && !self.level_events.is_empty() {
            self.process_level_events()?;
        }
        Ok(())
    }
}
