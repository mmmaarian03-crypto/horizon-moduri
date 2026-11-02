// More Overlays inside Horizon's game (nightly 142388e1c136). The rules are the engine below (the light/spawn
// scanner, the chunk bounds lines, the config); this part gives them the game's world and pushes the lines they
// return into the world's line pass (the one the fishing line and F3+G use).
use super::Game;
use horizon_client::gizmos::Line;
use horizon_client::{
    mobs::{
        Species,
        spawn_rules::{self, SpawnLevel},
        spawn_selection::{Category, NaturalLevel},
    },
    text::Text,
};
use horizon_core::geometry::{Aabb, Vec3};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    include!("moreoverlays_engine.rs");
}

pub const ID: &str = "moreoverlays";
pub const TITLE: &str = "More Overlays Updated 1.22.7";
pub const DESCRIPTION: &str = "Mob spawn light overlay (crosses where mobs can spawn) and chunk bounds lines. The item search overlay needs JEI and is not ported.";

struct State {
    config: Option<engine::Config>,
    light: engine::Light,
    chunks: engine::ChunkMode,
    region: engine::RegionInfo,
}
thread_local! {
    /// Inside the hook's own call of `fishing_line`.
    static INNER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static STATE: RefCell<State> = RefCell::new(State {
        config: None,
        light: engine::Light::default(),
        chunks: engine::ChunkMode::None,
        region: engine::RegionInfo::default(),
    });
}

/// What the scanner asks of the level, answered by the game's natural-spawning level.
struct Level<'g, 'a> {
    level: super::natural_spawning::GameLevel<'g, 'a>,
    monsters: Vec<Species>,
}
fn boxed(b: engine::Bounds) -> Aabb {
    Aabb::new(Vec3::new(b.min[0], b.min[1], b.min[2]), Vec3::new(b.max[0], b.max[1], b.max[2]))
}
impl engine::SpawnWorld for Level<'_, '_> {
    fn block_light(&self, x: i32, y: i32, z: i32) -> i32 {
        SpawnLevel::block_light(&self.level, x, y, z)
    }
    fn sky_light(&self, x: i32, y: i32, z: i32) -> i32 {
        SpawnLevel::sky_light(&self.level, x, y, z)
    }
    fn is_air(&self, x: i32, y: i32, z: i32) -> bool {
        let s = SpawnLevel::state(&self.level, x, y, z);
        self.level.game.world.is_air(s)
    }
    /// `isCollisionShapeFullBlock`: the eight octants of the block all collide (a slab or a stair leaves one free).
    fn full_collision(&self, x: i32, y: i32, z: i32) -> bool {
        let (fx, fy, fz) = (f64::from(x), f64::from(y), f64::from(z));
        let e = 0.01;
        [0.25, 0.75].iter().all(|dx| {
            [0.25, 0.75].iter().all(|dy| {
                [0.25, 0.75].iter().all(|dz| {
                    let c = Vec3::new(fx + dx, fy + dy, fz + dz);
                    !self.level.no_collision(Aabb::new(
                        Vec3::new(c.x - e, c.y - e, c.z - e),
                        Vec3::new(c.x + e, c.y + e, c.z + e),
                    ))
                })
            })
        })
    }
    fn liquid(&self, x: i32, y: i32, z: i32) -> bool {
        let s = SpawnLevel::state(&self.level, x, y, z);
        self.level.facts().get(s).fluid != horizon_client::blocks::Fluid::None
    }
    fn collides(&self, bounds: engine::Bounds) -> bool {
        !self.level.no_collision(boxed(bounds))
    }
    fn monster_types(&self) -> usize {
        self.monsters.len()
    }
    fn valid_spawn(&self, x: i32, y: i32, z: i32, monster: Option<usize>) -> bool {
        let species = monster.and_then(|i| self.monsters.get(i).copied()).unwrap_or(Species::Zombie);
        spawn_rules::is_valid_spawn(species, &self.level, x, y, z)
    }
}

impl Game<'_> {
    fn moreoverlays_config(&self) -> engine::Config {
        STATE.with(|s| {
            s.borrow_mut()
                .config
                .get_or_insert_with(|| engine::Config::read(|key| self.native_setting(ID, key)))
                .clone()
        })
    }
    /// The action bar line; the original's `§e` / `§c` colour prefix is dropped (plain text here).
    fn moreoverlays_say(&mut self, message: &str) {
        let plain: String = match message.strip_prefix('\u{a7}') {
            Some(rest) => rest.chars().skip(1).collect(),
            None => message.to_owned(),
        };
        self.titles.set_overlay(Text::literal(plain), false);
    }
    fn moreoverlays_level_height(&self) -> i32 {
        if self.dim == super::Dim::Overworld { 384 } else { 256 }
    }

    /// The keys (`F7` light overlay, `F9` chunk bounds: `KeyBindings`).
    pub(super) fn moreoverlays_key(&mut self, key: &str, pressed: bool) {
        if !pressed || !self.native_on(ID) || !self.native_playing() || self.player.is_none() {
            return;
        }
        if key == self.native_mod_key(ID, "light", "key.keyboard.f7") {
            let msg = STATE.with(|s| {
                let mut s = s.borrow_mut();
                let on = !s.light.enabled;
                s.light.set_enabled(on)
            });
            if let Some(m) = msg {
                self.moreoverlays_say(m);
            }
        } else if key == self.native_mod_key(ID, "chunks", "key.keyboard.f9") {
            let mode = STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.chunks = s.chunks.next();
                s.chunks
            });
            self.moreoverlays_say(&mode.message());
        }
    }

    /// `LightOverlayHandler` / `ChunkBoundsHandler` client tick.
    pub(super) fn moreoverlays_tick(&mut self) {
        if !self.native_on(ID) {
            return;
        }
        let Some(p) = self.player.as_ref() else {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.light.world_gone();
                s.chunks = engine::ChunkMode::None;
            });
            return;
        };
        let (px, py, pz) = (p.position.x, p.position.y, p.position.z);
        let (enabled, mode) = STATE.with(|s| {
            let s = s.borrow();
            (s.light.enabled, s.chunks)
        });
        STATE.with(|s| {
            s.borrow_mut().region.tick(mode, (px.floor() as i32) >> 4, (pz.floor() as i32) >> 4);
        });
        if !enabled {
            return;
        }
        let cfg = self.moreoverlays_config();
        let mut monsters: Vec<Species> = self
            .res
            .mob_types()
            .values()
            .filter(|t| Category::from_name(&t.category) == Category::Monster)
            .map(|t| t.species)
            .collect();
        monsters.sort_by_key(|s| format!("{s:?}"));
        monsters.dedup();
        let spawns = {
            let world = Level { level: self.natural_level(), monsters };
            engine::scan(&cfg, &world, engine::ScanPlayer { x: px, y: py, z: pz, yaw: self.yaw })
        };
        STATE.with(|s| s.borrow_mut().light.cache = spawns);
    }

    /// The `fishing_line` hook: with an overlay on, the vanilla list (and the other mods' hooks) plus ours. The guard
    /// lets the inner call run the original.
    pub(super) fn moreoverlays_fishing(&self, alpha: f64, size: [u32; 2]) -> Option<Vec<Line>> {
        if INNER.with(|i| i.get()) || !self.native_on(ID) {
            return None;
        }
        let active = STATE.with(|s| {
            let s = s.borrow();
            s.light.enabled || s.chunks != engine::ChunkMode::None
        });
        if !active {
            return None;
        }
        INNER.with(|i| i.set(true));
        let mut lines = self.fishing_line(alpha, size);
        INNER.with(|i| i.set(false));
        self.moreoverlays_lines(&mut lines);
        Some(lines)
    }

    /// The lines of both overlays, for the world's line pass.
    fn moreoverlays_lines(&self, out: &mut Vec<Line>) {
        if !self.native_on(ID) {
            return;
        }
        let Some(p) = self.player.as_ref() else { return };
        let cfg = self.moreoverlays_config();
        let (spawns, mode) = STATE.with(|s| {
            let s = s.borrow();
            (if s.light.enabled { s.light.cache.clone() } else { Vec::new() }, s.chunks)
        });
        let mut lines = Vec::new();
        if !spawns.is_empty() {
            lines.extend(engine::light_lines(&cfg, &spawns, p.position.y, |x, y, z| {
                let s = self.world.block(x, y, z);
                matches!(
                    &*self.res.facts().get(s).name,
                    "minecraft:snow" | "minecraft:snow_block" | "minecraft:powder_snow"
                )
            }));
        }
        if mode != engine::ChunkMode::None {
            let player = engine::ChunkPlayer { x: p.position.x, y: p.position.y, z: p.position.z };
            lines.extend(engine::chunk_lines(&cfg, mode, player, self.moreoverlays_level_height()));
        }
        out.extend(lines.into_iter().map(|l| Line {
            start: l.from,
            end: l.to,
            color: 0xFF00_0000 | l.rgb,
            width: l.width,
        }));
    }
}
