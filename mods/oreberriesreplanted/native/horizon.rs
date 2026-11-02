// Oreberries Replanted inside Horizon's game (nightly 142388e1c136). The rules are the
// engine below; this part reads the world the game's way: the bushes' random ticks
// (OreBerryBushBlock.tick), what a click does (use, attack), what a bush does to whoever
// stands in it (entityInside), where a bush may be placed (canSurvive) and the essence
// berry (EssenceBerryItem.use). The bushes are plain blocks here: see content.rs.
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("oreberriesreplanted_engine.rs");
}
use engine::{Ore, Prng, Settings, Support};
use horizon_client::items::ItemStack;
use horizon_core::geometry::Direction;
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::registry::BlockStateId;
use std::{cell::RefCell, rc::Rc};

/// The level's random as the world-bound draws use it.
struct Draws<'a>(&'a mut LegacyRandom);
impl engine::Rng for Draws<'_> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
    }
    fn next_boolean(&mut self) -> bool {
        self.0.next_boolean()
    }
}

/// The game's states of the bushes: for each state the ore and the age, and the
/// state of each ore at each age.
struct Ids {
    len: usize,
    /// Per state: 0 when it is not a bush, else `1 + ore index` and the age in the
    /// high bits.
    info: Vec<u8>,
    states: [[Option<BlockStateId>; 4]; 12],
    any: bool,
}
impl Ids {
    fn build(game: &Game<'_>) -> Self {
        let registry = game.pack.assets.registry;
        let len = registry.len();
        let mut info = vec![0u8; len];
        let mut states = [[None; 4]; 12];
        let mut any = false;
        for (i, slot) in info.iter_mut().enumerate() {
            let Some(s) = registry.state(BlockStateId(i as u32)) else {
                continue;
            };
            if !s.name.starts_with("oreberriesreplanted:") {
                continue;
            }
            let Some(ore) = Ore::from_bush(&s.name) else {
                continue;
            };
            let age = s
                .properties
                .get("age")
                .and_then(|a| a.parse::<usize>().ok())
                .filter(|a| *a < 4)
                .unwrap_or(0);
            *slot = (1 + ore.index() as u8) | ((age as u8) << 4);
            states[ore.index()][age] = Some(BlockStateId(i as u32));
            any = true;
        }
        Self { len, info, states, any }
    }
    fn is_bush(&self, state: u32) -> bool {
        self.info.get(state as usize).is_some_and(|i| *i != 0)
    }
    /// The ore and the age of a bush state.
    fn bush(&self, state: BlockStateId) -> Option<(Ore, i32)> {
        let i = *self.info.get(state.0 as usize)?;
        if i == 0 {
            return None;
        }
        Some((engine::ORES[usize::from(i & 15) - 1], i32::from(i >> 4)))
    }
    fn state(&self, ore: Ore, age: i32) -> Option<BlockStateId> {
        self.states[ore.index()].get(usize::try_from(age).ok()?).copied().flatten()
    }
}

struct Runtime {
    ids: Option<Rc<Ids>>,
    rng: Prng,
}
thread_local! {
    static RT: RefCell<Runtime> = RefCell::new(Runtime {
        ids: None,
        rng: Prng(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64),
        ),
    });
}

impl Game<'_> {
    fn oreberries_ids(&self) -> Option<Rc<Ids>> {
        let len = self.pack.assets.registry.len();
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            if rt.ids.as_ref().is_none_or(|i| i.len != len) {
                rt.ids = Some(Rc::new(Ids::build(self)));
            }
            rt.ids.clone()
        })
        .filter(|ids| ids.any)
    }

    /// Once a tick, where the game ticks its blocks (`tick_blocks`): the random ticks of
    /// the bushes, and the bushes' touch.
    pub(super) fn oreberries_tick(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(ids) = self.oreberries_ids() else {
            return;
        };
        self.oreberries_grow(&ids);
        self.oreberries_touch(&ids);
    }

    /// `ServerLevel.tickChunk`'s random ticks, for the bushes only: `randomTickSpeed`
    /// positions in each section that holds one, each a tick of the bush on it
    /// (`OreBerryBushBlock.tick`).
    fn oreberries_grow(&mut self, ids: &Ids) {
        let speed = self.ui.rules.int("random_tick_speed");
        if speed <= 0 {
            return;
        }
        let levels = self.tickets.levels();
        let keys: Vec<(i32, i32)> = self
            .world
            .chunks
            .keys()
            .copied()
            .filter(|&(x, z)| levels.random_ticking(x, z) && super::ticking_chunk(&self.world, x, z))
            .collect();
        let mut ticked: Vec<([i32; 3], BlockStateId)> = Vec::new();
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            for key in &keys {
                let Some(chunk) = self.world.chunks.get(key) else {
                    continue;
                };
                for (s, section) in chunk.sections.iter().enumerate() {
                    if !section.blocks.maybe_has(|v| ids.is_bush(v)) {
                        continue;
                    }
                    let y0 = horizon_client::chunk::MIN_Y + s as i32 * 16;
                    for _ in 0..speed {
                        let (dx, dy, dz) = {
                            use engine::Rng;
                            (
                                rt.rng.next_int_bound(16),
                                rt.rng.next_int_bound(16),
                                rt.rng.next_int_bound(16),
                            )
                        };
                        let state = chunk.block(dx, y0 + dy, dz);
                        if ids.is_bush(state.0) {
                            ticked.push(([key.0 * 16 + dx, y0 + dy, key.1 * 16 + dz], state));
                        }
                    }
                }
            }
        });
        let settings = Settings::default();
        for (pos, state) in ticked {
            if self.world.block(pos[0], pos[1], pos[2]) != state {
                continue;
            }
            let Some((ore, age)) = ids.bush(state) else {
                continue;
            };
            let light = self.world.light(pos[0], pos[1], pos[2]);
            let brightness = engine::raw_brightness(light >> 4, light & 15);
            let grown = RT.with(|rt| engine::grow(&settings, ore, age, brightness, &mut rt.borrow_mut().rng));
            if let Some(next) = grown.and_then(|a| ids.state(ore, a)) {
                let _ = self.level_set(pos, next);
            }
        }
    }

    /// `OreBerryBushBlock.entityInside` for the player: 1 point of cactus damage for each
    /// tick spent in the block of a bush (the invulnerability frames make it one
    /// hit every half second). Mobs are not hurt here.
    fn oreberries_touch(&mut self, ids: &Ids) {
        let Some(p) = self.player.as_ref() else {
            return;
        };
        if self.spectator() {
            return;
        }
        let w = f64::from(p.dimensions.width) / 2.0;
        let h = f64::from(p.dimensions.height);
        let min = [p.position.x - w, p.position.y, p.position.z - w];
        let max = [p.position.x + w, p.position.y + h, p.position.z + w];
        let touching = engine::cells_touched(min, max)
            .into_iter()
            .any(|c| ids.is_bush(self.world.block(c[0], c[1], c[2]).0));
        if touching {
            self.hurt_player_by(engine::HURT_SOURCE, engine::HURT_AMOUNT);
        }
    }

    /// `harvestBerry` on the bush at `pos`: a grown one goes back one age and gives
    /// 1 to 3 berries. Whether it did.
    fn oreberries_harvest(&mut self, ids: &Ids, pos: [i32; 3]) -> bool {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let Some((ore, age)) = ids.bush(state) else {
            return false;
        };
        let harvested = {
            let mut draws = Draws(&mut self.world.random);
            engine::harvest(age, &mut draws)
        };
        let Some(h) = harvested else {
            return false;
        };
        let Some(next) = ids.state(ore, h.new_age) else {
            return false;
        };
        let _ = self.level_set(pos, next);
        self.pop_resource(pos, ItemStack::new(&ore.berry(), h.berries as u32));
        true
    }

    /// `OreBerryBushBlock.attack`: the start of a left click on a grown bush.
    pub(super) fn oreberries_attack(&mut self, pos: [i32; 3]) {
        if self.remote.is_some() {
            return;
        }
        let Some(ids) = self.oreberries_ids() else {
            return;
        };
        self.oreberries_harvest(&ids, pos);
    }

    /// Whether the bush item may be put where `BlockItem.place` would put it
    /// (`OreBerryBushBlock.canSurvive`, asked when the block is placed).
    fn oreberries_may_place(&mut self, ids: &Ids, ore: Ore, hit: &super::Hit, item: &str) -> bool {
        let face = match hit.face {
            [0, 1, 0] => Direction::Up,
            [0, -1, 0] => Direction::Down,
            [0, 0, -1] => Direction::North,
            [0, 0, 1] => Direction::South,
            [-1, 0, 0] => Direction::West,
            _ => Direction::East,
        };
        let ctx = horizon_world::level::PlaceContext {
            clicked_pos: horizon_core::position::BlockPos::new(hit.pos[0], hit.pos[1], hit.pos[2]),
            face,
            hit: horizon_core::geometry::Vec3::new(hit.point[0], hit.point[1], hit.point[2]),
            inside: false,
            yaw: self.yaw,
            pitch: self.pitch,
            sneaking: self.keys.contains(&super::KeyCode::ShiftLeft),
            item: item.to_owned(),
            can_use_game_master_blocks: self.can_use_game_master_blocks(),
        };
        let Some((pos, _)) = self.with_level(|b, level, ticks| b.placement_state(level, ticks, &ctx)) else {
            // The game refuses by itself.
            return true;
        };
        let light = self.world.light(pos.x, pos.y, pos.z);
        let brightness = engine::raw_brightness(light >> 4, light & 15);
        let below = self.world.block(pos.x, pos.y - 1, pos.z);
        let support = match ids.bush(below) {
            Some((_, age)) => Support::Bush { age },
            None => Support::Block {
                sturdy_top: self.with_level(|b, _, _| b.is_face_sturdy(below, Direction::Up)),
            },
        };
        engine::can_place(&Settings::default(), ore, brightness, support)
    }

    /// `EssenceBerryItem.use`: an experience orb of 6 to 19 points, of the whole stack's
    /// when sneaking.
    fn oreberries_essence(&mut self) {
        let sneaking = self.keys.contains(&super::KeyCode::ShiftLeft);
        let count = self.held().map_or(1, |s| s.count);
        let xp = {
            use engine::Rng;
            let mut draws = Draws(&mut self.world.random);
            let mut xp = draws.next_int_bound(14) + 6;
            if sneaking {
                xp = 0;
                for _ in 0..count {
                    xp += draws.next_int_bound(14) + 6;
                }
            }
            xp
        };
        if let Some(p) = self.player.as_ref() {
            let at = [p.position.x, p.position.y, p.position.z];
            self.spawn_orbs(at, xp as u32);
        }
        if !self.creative {
            if sneaking {
                *self.held_mut() = None;
            } else {
                self.consume_held();
            }
        }
        self.swing_arm();
    }

    /// `OreBerryBushBlock.use` on the main hand's turn, called where the game tries the
    /// uses of the blocks it knows (`use_composter`, with the clicked block's position): a
    /// grown bush gives its berries. The original answers PASS, so the click goes on.
    pub(super) fn oreberries_click(&mut self, pos: [i32; 3]) {
        if self.remote.is_some() || self.hand != super::Hand::Main {
            return;
        }
        let Some(ids) = self.oreberries_ids() else {
            return;
        };
        if self.oreberries_harvest(&ids, pos) {
            self.swing_arm();
        }
    }

    /// The held item, where the game tries the items that act on their own (`throw_item`, after
    /// the clicked block had its say): the essence berry is used, and a bush item is not placed
    /// where `canSurvive` says no (`FAIL`: the click ends).
    pub(super) fn oreberries_item(&mut self, stack: &ItemStack) -> core::ops::ControlFlow<Result<bool>> {
        use core::ops::ControlFlow::{Break, Continue};
        if self.remote.is_some() || !stack.item.starts_with("oreberriesreplanted:") {
            return Continue(());
        }
        let Some(ids) = self.oreberries_ids() else {
            return Continue(());
        };
        if &*stack.item == "oreberriesreplanted:essence_berry" {
            self.oreberries_essence();
            return Break(Ok(true));
        }
        let Some(ore) = Ore::from_bush(&stack.item) else {
            return Continue(());
        };
        let Some(hit) = self.pick() else {
            return Continue(());
        };
        if self.oreberries_may_place(&ids, ore, &hit, &stack.item) {
            Continue(())
        } else {
            Break(Ok(true))
        }
    }
}
