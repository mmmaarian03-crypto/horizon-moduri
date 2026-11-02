// More Bows Restrung inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs):
// `CustomBow` (the draw, the shot, the multi-shot, the damage), `ArrowSpawner` (the ender bow's delayed
// arrows) and `CustomArrow` (the frost arrow). The game knows a bow only by the id `minecraft:bow`
// (use, use tick, release, the length of the draw), so the eight bows get their own hooks on those.
// The items (model, recipes, lang) are content (content/mods/morebows); their numbers are native/items.rs.
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("morebows_engine.rs");
}
use horizon_client::arrows::Pickup;
use horizon_client::items::ItemStack;
use horizon_client::mobs::Who;
use horizon_client::projectiles::{Projectile, ProjectileKind};
use horizon_core::geometry::Vec3;
use horizon_core::position::BlockPos;
use horizon_core::random::RandomSource;

include!("kit_blocks.rs");
include!("kit_sound.rs");

/// An ender-bow arrow kept back by the `ArrowSpawner`: it turns up `due` (game time), moved by `offset`.
struct Delayed {
    due: i64,
    projectile: Projectile,
    offset: [f64; 3],
    sound: Option<(engine::ShotSound, f32)>,
}
thread_local! {
    /// The boxes of the blazes at the start of the tick, noted while a frost arrow flies (the damage of an
    /// arrow is worked out where the mobs are not at hand).
    static BLAZES: std::cell::RefCell<Vec<([f64; 3], [f64; 3])>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The `ArrowSpawner` entities of the open world. In memory only: a world saved in the three seconds
    /// between the shot and the arrows loses them (the original saved the spawner with its arrows).
    static DELAYED: std::cell::RefCell<Vec<Delayed>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `Item.getUseDuration` of the bows (the model's `use_duration`, the draw animation): used by the helper
/// in `item_draw.rs`, which knows only the item model of a stack.
pub(super) fn is_bow_model(model: &str) -> bool {
    engine::bow_of(model).is_some()
}

/// The frost arrows are the arrows shot from a frost bow.
fn from_bow(arrow: &Projectile, bow: &str) -> bool {
    arrow
        .arrow
        .as_ref()
        .and_then(|a| a.fired_from_weapon.as_ref())
        .is_some_and(|s| &*s.item == bow)
}

/// The blocks around a stuck frost arrow as the freezing reads them.
struct Around<'a, 'g>(&'a mut Game<'g>);
impl engine::Column for Around<'_, '_> {
    fn cell(&mut self, p: engine::Pos) -> engine::Cell {
        match kit_blocks::name_at(self.0, p).as_deref() {
            None => engine::Cell::Other,
            Some("minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") => engine::Cell::Air,
            Some("minecraft:water") => engine::Cell::Water,
            Some("minecraft:snow") => {
                let state = kit_blocks::state_at(self.0, p);
                let layers = kit_blocks::prop(self.0, state, "layers")
                    .and_then(|l| l.parse::<u8>().ok())
                    .unwrap_or(1);
                engine::Cell::Snow(layers)
            }
            Some(_) => engine::Cell::Other,
        }
    }
    fn snow_can_survive(&mut self, p: engine::Pos) -> bool {
        let Some(snow) = self.0.pack.assets.registry.default_state("minecraft:snow") else {
            return false;
        };
        let at = BlockPos::new(p[0], p[1], p[2]);
        self.0.with_level(|b, level, ticks| b.can_survive(level, ticks, at, snow))
    }
}

/// `level.random` of the game.
struct LevelRandom<'a>(&'a mut horizon_core::random::LegacyRandom);
impl engine::Rng for LevelRandom<'_> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
    }
}

impl Game<'_> {
    /// HEAD on `throw_item` (an item used, after the block's own use): `CustomBow.use` -> `BowItem.use`.
    /// The ender bow shows a portal particle first; then, with an arrow (or in creative), the bow is
    /// drawn. `true` when the stack is one of the bows.
    pub(super) fn morebows_use(&mut self, stack: &ItemStack) -> bool {
        let Some(bow) = engine::bow_of(&stack.item) else {
            return false;
        };
        if self.remote.is_some() {
            return false;
        }
        if bow.arrow == engine::ArrowType::Ender {
            self.morebows_portal_particle();
        }
        if self.using.is_none() && (self.creative || self.projectile_slot("minecraft:bow").is_some()) {
            self.using = Some((self.held_slot(), 0));
        }
        true
    }

    /// `MoreBows.tryPart(level, user, PORTAL, true, 1.0)`: one portal particle around the user.
    fn morebows_portal_particle(&mut self) {
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let (at, w, h) = (p.position, p.dimensions.width, p.dimensions.height);
        let r = &mut self.world.random;
        let spread = [
            f64::from(r.next_float() * w * 2.0 - w),
            0.5 + f64::from(r.next_float() * h),
            f64::from(r.next_float() * w * 2.0 - w),
        ];
        let options = horizon_client::particles::ParticleOptions::simple(horizon_client::particles::ParticleType::Portal);
        self.particles.send_particles(&options, [at.x, at.y, at.z], spread, 1.0, 1);
    }

    /// HEAD on `tick_drinking` (the first of the use ticks that is not a shield): a bow in use stays in use
    /// until it is released. `true` when the stack is one of the bows.
    pub(super) fn morebows_tick_use(&mut self, slot: usize, ticks: u32, stack: &ItemStack) -> bool {
        if engine::bow_of(&stack.item).is_none() {
            return false;
        }
        self.using = Some((slot, ticks + 1));
        true
    }

    /// HEAD on `release_used_item`: `CustomBow.releaseUsing`. A hand change lets go of nothing.
    /// `true` when the stack is one of the bows.
    pub(super) fn morebows_release(&mut self, stack: &ItemStack, ticks: u32, hand_changed: bool) -> bool {
        let Some(bow) = engine::bow_of(&stack.item) else {
            return false;
        };
        if !hand_changed && self.remote.is_none() {
            self.morebows_shoot(bow, stack, ticks);
        }
        true
    }

    fn morebows_shoot(&mut self, bow: &'static engine::Bow, bow_stack: &ItemStack, ticks: u32) {
        // alwaysShoots = creative || Infinity; `ammo = player.getProjectile(stack)`.
        let slot = self.projectile_slot("minecraft:bow");
        let (power_bonus, infinity) = self.bow_enchantments("minecraft:arrow");
        let always = self.creative || infinity;
        if slot.is_none() && !always {
            return;
        }
        let Some(eye) = self.eye_position() else {
            return;
        };
        let ammo = slot
            .and_then(|i| self.stack_in(i).cloned())
            .unwrap_or_else(|| ItemStack::new("minecraft:arrow", 1));
        let release = engine::Release {
            charge: ticks as i32,
            power_bonus,
            flame: self.bow_projectile_ignite().is_some(),
            punch: bow_stack.enchantment_level("minecraft:punch") as u32,
            infinite_ammo: always,
        };
        let shot = {
            let mut random = self.world.random.clone();
            let shot = engine::plan_shot(bow, &release, &mut LevelRandom(&mut random));
            self.world.random = random;
            let Some(shot) = shot else {
                return;
            };
            shot
        };
        let (velocity, on_ground) = self
            .player
            .as_ref()
            .map_or((Vec3::ZERO, true), |pl| (pl.velocity, pl.ground.on_ground));
        let start = Vec3::new(eye[0], eye[1] - 0.1, eye[2]);
        let yaw = f64::from(self.yaw);
        let now = self.world.game_time;
        for plan in &shot.arrows {
            // arrow.createArrow / new CustomArrow, then shootFromRotation(player, xRot, yRot, 0, speed, 1).
            let kind = ProjectileKind::Arrow {
                base_damage: plan.base_damage,
                critical: shot.critical,
                effect: None,
            };
            let mut arrow = Projectile::new(kind, start, Some(Who::Player));
            if let Some(a) = arrow.arrow.as_mut() {
                a.set_item(&horizon_client::arrows::drawn_ammo(&ammo, always));
                a.set_owner(Some(Who::Player));
                if plan.creative_only {
                    a.pickup = Pickup::CreativeOnly;
                }
                if plan.fire_seconds > 0 {
                    a.ignite_for_seconds(plan.fire_seconds as f32);
                }
                a.fired_from_weapon = Some(bow_stack.clone());
            }
            arrow.bonus.punch = release.punch;
            arrow.shoot_from_rotation(
                &mut self.world.random,
                self.pitch,
                self.yaw,
                0.0,
                plan.speed,
                1.0,
                velocity,
                on_ground,
            );
            // The multi bow's other arrows are set a little aside: arr.setPos(x + yRot / 180 * negate, y, z).
            if plan.x_offset_sign != 0.0 {
                arrow.position.x += yaw / 180.0 * plan.x_offset_sign;
                arrow.previous = arrow.position;
            }
            if plan.delay_ticks == 0 {
                self.add_projectile(arrow);
            } else {
                let sound = (plan.index > 0).then(|| (engine::spawner_sound(plan.index), shot.shot_velocity));
                DELAYED.with(|d| {
                    d.borrow_mut().push(Delayed {
                        due: now + i64::from(plan.delay_ticks),
                        projectile: arrow,
                        offset: plan.spawn_offset,
                        sound,
                    });
                });
            }
        }
        self.player_projectile_shot();
        // The shot sounds, at the player's feet (beside him for the multi bow's extra arrows).
        if let Some(feet) = self.player.as_ref().map(|p| p.position) {
            for s in &shot.sounds {
                let pitch = s.pitch_of(self.world.random.next_float(), shot.shot_velocity);
                let at = [feet.x + yaw / 180.0 * s.x_offset_sign, feet.y, feet.z];
                kit_sound::at(self, s.event, at, s.volume, pitch);
            }
        }
        // ammo.shrink(usedAmmo); stack.hurtAndBreak(1); player.awardStat(ITEM_USED).
        if shot.ammo_used > 0
            && !self.creative
            && let Some(i) = slot
        {
            let cell = self.stack_in_mut(i);
            if let Some(s) = cell.as_mut() {
                s.count = s.count.saturating_sub(shot.ammo_used as u32);
                if s.count == 0 {
                    *cell = None;
                }
            }
        }
        self.damage_held(1);
        self.stats.add("used", &bow_stack.item, 1);
    }

    /// HEAD on `tick_blocks` (once per game tick): the ender bow's arrows that turn up, and the frost
    /// arrows that have lain in the ground long enough.
    pub(super) fn morebows_tick(&mut self) {
        let now = self.world.game_time;
        // ArrowSpawner.tick
        let due = DELAYED.with(|d| {
            let mut list = d.borrow_mut();
            let mut due = Vec::new();
            let mut i = 0;
            while i < list.len() {
                if now >= list[i].due {
                    due.push(list.remove(i));
                } else {
                    i += 1;
                }
            }
            due
        });
        for mut d in due {
            d.projectile.position = Vec3::new(
                d.projectile.position.x + d.offset[0],
                d.projectile.position.y + d.offset[1],
                d.projectile.position.z + d.offset[2],
            );
            d.projectile.previous = d.projectile.position;
            let at = d.projectile.position;
            self.add_projectile(d.projectile);
            if let Some((sound, sv)) = d.sound {
                let pitch = sound.pitch_of(self.world.random.next_float(), sv);
                kit_sound::at(self, sound.event, [at.x, at.y, at.z], sound.volume, pitch);
            }
        }
        // The blazes a frost arrow in flight could hit.
        let frost_flying = self.creatures.projectiles.iter().any(|p| from_bow(p, "morebows:frost_bow"));
        let blazes: Vec<([f64; 3], [f64; 3])> = if frost_flying {
            self.mobs
                .iter()
                .filter(|m| m.alive() && m.species() == horizon_client::mobs::Species::Blaze)
                .map(|m| {
                    let half = f64::from(m.width()) / 2.0;
                    let (p, h) = (m.position, f64::from(m.height()));
                    ([p.x - half, p.y, p.z - half], [p.x + half, p.y + h, p.z + half])
                })
                .collect()
        } else {
            Vec::new()
        };
        BLAZES.with(|b| *b.borrow_mut() = blazes);
        // CustomArrow.tick, frost: stuck in the ground.
        let mut freeze = Vec::new();
        for p in &mut self.creatures.projectiles {
            if !from_bow(p, "morebows:frost_bow") {
                continue;
            }
            let at = p.position;
            if let Some(a) = p.arrow.as_mut().filter(|a| a.in_ground) {
                if a.in_ground_time == engine::FROST_NO_PICKUP_TICK {
                    a.pickup = Pickup::Disallowed;
                }
                if a.in_ground_time >= engine::FROST_FREEZE_TICK {
                    if a.in_ground_time == engine::FROST_FREEZE_TICK {
                        freeze.push([at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32]);
                    }
                    // discard()
                    a.removed = true;
                }
            }
        }
        for at in freeze {
            self.morebows_freeze(at);
        }
    }

    /// `CustomArrow.tick` at `inGroundTime == 65`: snow where the arrow is, ice for water, one more layer
    /// on snow.
    fn morebows_freeze(&mut self, at: [i32; 3]) {
        let Some(change) = engine::freeze(&mut Around(self), at) else {
            return;
        };
        let (pos, name, layers, flags) = match change {
            engine::Freeze::Snow(p) => (p, "minecraft:snow", None, 3),
            engine::Freeze::Ice(p) => (p, "minecraft:ice", None, 3),
            engine::Freeze::MoreSnow(p, l) => (p, "minecraft:snow", Some(l), 10),
        };
        let Some(mut state) = self.pack.assets.registry.default_state(name) else {
            return;
        };
        if let Some(l) = layers {
            let current = kit_blocks::state_at(self, pos);
            if let Some(next) = kit_blocks::with_props(self, current, &[("layers", &l.to_string())]) {
                state = next;
            }
        }
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, state, flags));
        let _ = self.process_level_events();
    }

    /// HEAD on `finish_arrow_hit`: `CustomArrow.onHitEntity` for a frost arrow. The hit entity is the
    /// mob whose box the arrow ended in (the hook gets the arrow, not the victim): it is slowed and put
    /// out; a blaze's triple damage is `morebows_damage`.
    pub(super) fn morebows_arrow_hit(&mut self, arrow: &Projectile, outcome: horizon_client::arrows::EntityHit) {
        use horizon_client::arrows::EntityHit;
        if outcome == EntityHit::Deflected || !from_bow(arrow, "morebows:frost_bow") {
            return;
        }
        let Some(id) = self.mob_at(arrow.position) else {
            return;
        };
        let blaze = self
            .mobs
            .iter()
            .any(|m| m.id == id && m.species() == horizon_client::mobs::Species::Blaze);
        let hit = engine::frost_hit(blaze);
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
            if hit.clear_fire {
                m.fire_ticks = m.fire_ticks.min(0);
            }
            if !engine::config::OLD_FROST_ARROW_MOB_SLOWDOWN {
                m.add_effect("minecraft:slowness", hit.slowness.0, hit.slowness.1);
            }
        }
    }

    /// The live mob whose box holds (or nearly holds) the point: the one an arrow stopped in.
    fn mob_at(&self, at: Vec3) -> Option<u32> {
        let mut best: Option<(f64, u32)> = None;
        for m in &self.mobs {
            let half = f64::from(m.width()) / 2.0;
            let (p, h) = (m.position, f64::from(m.height()));
            let d = |lo: f64, hi: f64, v: f64| (lo - v).max(0.0).max(v - hi);
            let (dx, dy, dz) = (d(p.x - half, p.x + half, at.x), d(p.y, p.y + h, at.y), d(p.z - half, p.z + half, at.z));
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist < 1.0 && best.is_none_or(|(b, _)| dist < b) {
                best = Some((dist, m.id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// RETURN on `arrow_damage` (a free function: it sees the arrow, not the mobs): a frost arrow does
    /// three times the damage to a blaze (`setBaseDamage(getBaseDamage() * 3.0)` before the hit). The
    /// blazes are the boxes `morebows_tick` noted at the start of the tick.
    pub(super) fn morebows_blaze_factor(arrow: &Projectile, damage: f32) -> f32 {
        if !from_bow(arrow, "morebows:frost_bow") {
            return damage;
        }
        let at = arrow.position;
        let blaze = BLAZES.with(|b| {
            b.borrow().iter().any(|(lo, hi)| {
                let d = |lo: f64, hi: f64, v: f64| (lo - v).max(0.0).max(v - hi);
                let (dx, dy, dz) = (d(lo[0], hi[0], at.x), d(lo[1], hi[1], at.y), d(lo[2], hi[2], at.z));
                (dx * dx + dy * dy + dz * dz).sqrt() < 1.0
            })
        });
        damage * engine::frost_hit(blaze).damage_factor as f32
    }

    /// RETURN on `zoomed_fov_target` (the tick of the camera's field of view): the zoom of a drawn bow
    /// goes by its own draw time (`Client.FOV`). The game's own zoom is only for `minecraft:bow`.
    pub(super) fn morebows_fov(&self, target: f32) -> f32 {
        let Some((slot, ticks)) = self.using else {
            return target;
        };
        let Some(bow) = self.stack_in(slot).and_then(|s| engine::bow_of(&s.item)) else {
            return target;
        };
        let effect = self.fov_effect_scale();
        if effect == 0.0 {
            return target;
        }
        // target = lerp(effect, 1, modifier): the bow's factor goes into the modifier.
        let modifier = 1.0 + (target - 1.0) / effect;
        1.0 + effect * (modifier * engine::fov_factor(bow, ticks as i32) - 1.0)
    }

    /// HEAD on `load_world_state`: a world opens, the arrows kept back in the one before are gone.
    pub(super) fn morebows_world_opened() {
        DELAYED.with(|d| d.borrow_mut().clear());
        BLAZES.with(|b| b.borrow_mut().clear());
    }
}
