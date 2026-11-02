// Cave Dust inside Horizon's game (nightly 142388e1c136). The rules are the engine below
// (src/lib.rs); this part reads the world and the player once a tick and adds the particles.
//
// Cave Dust is a client mod: it runs on the game that has the window (`native_window`), once per
// client tick (the original listens to Architectury's `ClientTickEvent.CLIENT_POST`), and not on a
// server's own game.
use super::{Dim, Game};
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_core::random::{LegacyRandom, RandomSource};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    include!("cavedust_engine.rs");
}

/// `ThreadLocalRandom` stand-in: the game's own random generator type, seeded from the clock, so
/// the dust never touches the world's or the particle engine's random sequences.
impl engine::Rng for LegacyRandom {
    fn next_f64(&mut self) -> f64 {
        self.next_double()
    }
}

/// What the mod remembers: the original's static `timer` and a random generator.
struct Memory {
    dust: engine::Dust,
    rng: LegacyRandom,
}
thread_local! {
    static MEMORY: RefCell<Memory> = RefCell::new(Memory {
        dust: engine::Dust::default(),
        rng: LegacyRandom::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0x5DEE_CE66, |d| d.as_nanos() as i64),
        ),
    });
}

/// The world as the dust sees it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::Level for Live<'_, '_> {
    fn paused(&self) -> bool {
        self.game.pause
    }
    /// Only the overworld among the vanilla dimension types has `bed_works`.
    fn bed_works(&self) -> bool {
        self.game.dim == Dim::Overworld
    }
    fn bottom_y(&self) -> i32 {
        self.game.dim.min_y()
    }
    fn lush_caves(&self, p: engine::Pos) -> bool {
        let g = self.game;
        g.world
            .chunks
            .get(&(p[0] >> 4, p[2] >> 4))
            .is_some_and(|chunk| {
                g.res.biomes().get(chunk.biome(p[0], p[1], p[2])).name == engine::LUSH_CAVES
            })
    }
    /// Horizon has no flat-world flag on the client; with `superFlatStatus` off (the pack's
    /// setting) the original never asks anyway.
    fn flat_world(&self) -> bool {
        false
    }
    /// `canSeeSky`: sky light 15 (the high half of the packed light).
    fn sky_visible(&self, p: engine::Pos) -> bool {
        self.game.world.light(p[0], p[1], p[2]) >> 4 == 15
    }
    fn is_air(&self, p: engine::Pos) -> bool {
        let g = self.game;
        if p[1] < g.dim.min_y() || p[1] > g.dim.max_y() {
            return false;
        }
        let state = g.world.block(p[0], p[1], p[2]);
        matches!(
            &*g.res.facts().get(state).name,
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        )
    }
}

impl Game<'_> {
    /// One client tick of Cave Dust: dust in the dark air around a player who is underground.
    pub(super) fn cavedust_tick(&mut self) {
        // The window's game only, in a world, not behind the title screen.
        if !self.native_window() || self.title.is_some() {
            return;
        }
        let Some(feet) = self
            .player
            .as_ref()
            .map(|p| [p.position.x, p.position.y, p.position.z])
        else {
            return;
        };
        let settings = engine::Settings::default();
        let places = MEMORY.with(|m| {
            let m = &mut *m.borrow_mut();
            m.dust
                .tick(&settings, &Live { game: &*self }, &mut m.rng, feet)
        });
        if places.is_empty() {
            return;
        }
        // `particleID`, falling back to white ash when it names nothing the engine can make.
        let ty = ParticleType::from_name(engine::particle_name(settings.particle_id))
            .unwrap_or(ParticleType::WhiteAsh);
        let options = ParticleOptions::simple(ty);
        for p in places {
            // `addParticle(particle, x, y, z, 0, 0, 0)` at the corner of the block.
            self.particles
                .add(&options, p.map(f64::from), [0.0, 0.0, 0.0]);
        }
    }
}
