// Ender Ore inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs): the
// experience the ores drop (`DropExperienceBlock(.., UniformInt.of(2, 5))`). The ores in the biomes are
// another module of this mod, in the generator (native/worldgen.rs). The blocks themselves, their loot
// (an ender pearl, Fortune, Silk Touch) and the smelting are content (content/mods/enderore).
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("enderore_engine.rs");
}

/// `level.random` of the game.
struct LevelRandom<'a>(&'a mut horizon_core::random::LegacyRandom);
impl engine::Rng for LevelRandom<'_> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        use horizon_core::random::RandomSource;
        self.0.next_int_bound(bound).unwrap_or(0)
    }
}

impl Game<'_> {
    /// RETURN on `block_experience(name)`: the game knows the experience of its own blocks (it answers 0 for
    /// ours). It is asked only for a player's break with the right tool and without Silk Touch
    /// (`DropExperienceBlock.spawnAfterBreak`: `silkTouch == 0`, `Block.playerDestroy` only when harvested).
    pub(super) fn enderore_experience(&mut self, name: &str, value: u32) -> u32 {
        if value != 0 {
            return value;
        }
        engine::experience(name, &mut LevelRandom(&mut self.world.random)).unwrap_or(value)
    }
}
