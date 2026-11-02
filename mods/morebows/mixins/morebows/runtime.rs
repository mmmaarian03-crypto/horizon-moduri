// More Bows Restrung inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs):
// `CustomBow` (the draw, the shot, the multi-shot, the damage), `ArrowSpawner` (the ender bow's delayed
// arrows) and `CustomArrow` (the frost arrow). The game knows a bow only by the id `minecraft:bow`
// (use, use tick, release, the length of the draw), so the eight bows get their own hooks on those.
// The items (model, recipes, lang) are content (content/mods/morebows); their numbers are native/items.rs.
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    // More Bows Restrung for Horizon: the rules, without any game code.
// Written from the public source of the mod (NeRdTheNed/More-bows, branch forge-1.20.x, licence CC0-1.0,
// commit 08a961b, the jar MoreBows-1.0.13+1.20.x.forge): `MoreBows`, `CustomBow`, `CustomArrow`,
// `ArrowSpawner`, `ConfigBows`, `ConfigGeneral`. the pack's profile has no `morebows_bowstats.json` and no
// `morebows_general.json`, so every value is the mod's default. This file is pasted into the game three
// times (`crate::game::morebows::engine`, the same in `mod_content` for the items' numbers) and is
// also the `horizon_morebows` crate the tests use: plain Rust, no game types, no inner attributes.

pub type Pos = [i32; 3];

/// The pack has no config file for this mod (`morebows_bowstats.json` and
/// `morebows_general.json` are missing): the values are the defaults of `ConfigBows` / `ConfigGeneral`.
pub mod config {
    /// `ConfigGeneral.frostArrowsShouldBeCold`.
    pub const FROST_ARROWS_SHOULD_BE_COLD: bool = true;
    /// `ConfigGeneral.oldFrostArrowMobSlowdown` (false: the Slowness potion effect on hit).
    pub const OLD_FROST_ARROW_MOB_SLOWDOWN: bool = false;
    /// `ConfigGeneral.customArrowMultiShot` = `AlwaysCustomArrows`: one arrow is used, every arrow shot is
    /// a custom one; the extra arrows can only be picked up in creative.
    pub const MULTI_SHOT_USES_ONE_ARROW: bool = true;
}

pub const MOD: &str = "morebows";

/// `MoreBows.ARROW_TYPE_*`: what the bow's arrows are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowType {
    NotCustom,
    Ender,
    Fire,
    Frost,
}

/// One bow: `MoreBows.getAllItems` (rarity, repair material, arrow type, multi-shot, fire resistance)
/// with `ConfigBows`' default `BowConfig(durability, damageMult, drawbackDiv)`.
#[derive(Clone, Copy, Debug)]
pub struct Bow {
    pub name: &'static str,
    pub durability: u32,
    pub damage_mult: f64,
    pub drawback_div: f32,
    pub arrow: ArrowType,
    pub multi: bool,
    pub rarity: &'static str,
    pub fire_resistant: bool,
    /// `repairIngredient`: item ids (the tag of the original resolved: `forge:gems/diamond`,
    /// `forge:ingots/gold`, `forge:ender_pearls`, `forge:stone`, `forge:ingots/iron`, `forge:ice`).
    pub repair: &'static [&'static str],
}

/// In the order of `MoreBows.getAllItems`: Diamond, Ender, Flame, Frost, Gold, Iron, Multi, Stone.
pub const BOWS: [Bow; 8] = [
    Bow { name: "diamond_bow", durability: 1016, damage_mult: 2.25, drawback_div: 6.0, arrow: ArrowType::NotCustom, multi: false, rarity: "rare", fire_resistant: false, repair: &["minecraft:diamond"] },
    Bow { name: "ender_bow", durability: 215, damage_mult: 1.0, drawback_div: 22.0, arrow: ArrowType::Ender, multi: true, rarity: "epic", fire_resistant: false, repair: &["minecraft:ender_pearl"] },
    Bow { name: "flame_bow", durability: 576, damage_mult: 2.0, drawback_div: 15.0, arrow: ArrowType::Fire, multi: false, rarity: "uncommon", fire_resistant: true, repair: &["minecraft:gold_ingot"] },
    Bow { name: "frost_bow", durability: 550, damage_mult: 1.0, drawback_div: 26.0, arrow: ArrowType::Frost, multi: false, rarity: "common", fire_resistant: false, repair: &["minecraft:ice", "minecraft:packed_ice", "minecraft:blue_ice"] },
    Bow { name: "gold_bow", durability: 68, damage_mult: 2.5, drawback_div: 6.0, arrow: ArrowType::NotCustom, multi: false, rarity: "uncommon", fire_resistant: false, repair: &["minecraft:gold_ingot"] },
    Bow { name: "iron_bow", durability: 550, damage_mult: 1.5, drawback_div: 17.0, arrow: ArrowType::NotCustom, multi: false, rarity: "common", fire_resistant: false, repair: &["minecraft:iron_ingot"] },
    Bow { name: "multi_bow", durability: 550, damage_mult: 1.0, drawback_div: 13.0, arrow: ArrowType::NotCustom, multi: true, rarity: "rare", fire_resistant: false, repair: &["minecraft:iron_ingot"] },
    Bow { name: "stone_bow", durability: 484, damage_mult: 1.15, drawback_div: 20.0, arrow: ArrowType::NotCustom, multi: false, rarity: "common", fire_resistant: false, repair: &["minecraft:stone", "minecraft:andesite", "minecraft:diorite", "minecraft:granite"] },
];

impl Bow {
    /// `morebows:iron_bow`.
    pub fn id(&self) -> String {
        format!("{MOD}:{}", self.name)
    }
}

/// The bow of an item id (`morebows:iron_bow`).
pub fn bow_of(item: &str) -> Option<&'static Bow> {
    let name = item.strip_prefix("morebows:")?;
    BOWS.iter().find(|b| b.name == name)
}

/// `Client.bowMaxUseDuration`: a drawn bow is used until released.
pub const MAX_USE_DURATION: i32 = 72000;

// -------------------------------------------------------------------------------------- the shot

/// `RandomSource` as the shot draws from it.
pub trait Rng {
    fn next_int_bound(&mut self, bound: i32) -> i32;
}

/// What the game tells the engine about the release (`CustomBow.releaseUsing`).
#[derive(Clone, Copy, Debug)]
pub struct Release {
    /// `getUseDuration(stack) - remainingUseTicks`: the ticks the bow was drawn.
    pub charge: i32,
    /// `Enchantments.POWER_ARROWS` bonus as `base + level * 0.5 + 0.5` (0 without the enchantment).
    pub power_bonus: f64,
    /// `Enchantments.FLAMING_ARROWS > 0`.
    pub flame: bool,
    /// `Enchantments.PUNCH_ARROWS`.
    pub punch: u32,
    /// `player.getAbilities().instabuild || arrowItem.isInfinite(..)`: the arrow is not used up.
    pub infinite_ammo: bool,
}

/// One arrow of a shot.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrowPlan {
    /// The arrow's place in the shot (`i`).
    pub index: usize,
    /// The speed handed to `shootFromRotation`: `shotVelocity * (k * 1.5F)`.
    pub speed: f32,
    /// A `CustomArrow` of this type instead of a vanilla arrow.
    pub custom: ArrowType,
    /// `baseDamage` after the enchantments, `damageMult` and the multi bow's per-arrow factor.
    pub base_damage: f64,
    /// `setSecondsOnFire` (0: not burning).
    pub fire_seconds: u32,
    /// `Pickup.CREATIVE_ONLY` (else the default, ALLOWED for a player's arrow).
    pub creative_only: bool,
    /// The arrow is placed `yaw / 180 * x_offset_sign` blocks aside (the multi bow; 0 for none).
    pub x_offset_sign: f64,
    /// The ender bow keeps the arrow back: ticks after the shot, 0 = at once.
    pub delay_ticks: i32,
    /// Where the delayed arrow turns up, from where it was shot (`ArrowSpawner.tick`).
    pub spawn_offset: [f64; 3],
}

/// A sound of the release: its pitch is `1.0F / (random * 0.4F + base) + shotVelocity * k` with a
/// fresh `random` (see `pitch_of`).
#[derive(Clone, Debug, PartialEq)]
pub struct ShotSound {
    pub event: &'static str,
    pub volume: f32,
    pub pitch_base: f32,
    pub pitch_k: f32,
    /// Where, from the player's feet: along x by `yaw / 180 * sign`.
    pub x_offset_sign: f64,
}
impl ShotSound {
    pub fn pitch_of(&self, random: f32, shot_velocity: f32) -> f32 {
        pitch(random, self.pitch_base, shot_velocity, self.pitch_k)
    }
}

/// What `releaseUsing` makes of a release.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub arrows: Vec<ArrowPlan>,
    /// `isCrit`: every arrow is a critical one.
    pub critical: bool,
    /// The arrows used up (`usedAmmo`; 0 when the ammo is infinite).
    pub ammo_used: i32,
    pub sounds: Vec<ShotSound>,
    /// The speed after `Mth`-less clamping, kept for the delayed sounds.
    pub shot_velocity: f32,
}

/// `shotVelocity = charge / powerDiv; (v * v + v * 2) / 3`, as the floats of the original.
pub fn raw_velocity(charge: i32, power_div: f32) -> f32 {
    let v = charge as f32 / power_div;
    ((v * v) + (v * 2.0)) / 3.0
}

/// The speed factors of `CustomBow.releaseUsing`, each `k * 1.5F` in float arithmetic.
pub fn velocity_factor(bow: &Bow, i: usize) -> f32 {
    if bow.multi && bow.arrow == ArrowType::Ender {
        match i {
            1 => 1.0f32 * 1.5f32,
            2 => 1.2f32 * 1.5f32,
            3 => 1.5f32 * 1.5f32,
            4 => 1.75f32 * 1.5f32,
            5 => 1.825f32 * 1.5f32,
            _ => 2.0f32 * 1.5f32,
        }
    } else if bow.multi {
        match i {
            1 => 1.65f32 * 1.5f32,
            2 => 1.275f32 * 1.5f32,
            _ => 2.0f32 * 1.5f32,
        }
    } else {
        2.0f32 * 1.5f32
    }
}

/// The multi bow's per-arrow damage factor (`damageMultiChoice`).
pub fn multi_damage_factor(i: usize) -> f64 {
    match i {
        1 => 1.3,
        2 => 1.15,
        _ => 1.5,
    }
}

/// The arrow's `baseDamage` the way `releaseUsing` builds it, in the order of the original: 2.0, plus
/// Power, times 1.25 for flame arrows with Flame, times `damageMult`, times the multi bow's factor.
pub fn base_damage(bow: &Bow, r: &Release, i: usize) -> f64 {
    let mut damage = 2.0;
    if r.power_bonus > 0.0 {
        damage += r.power_bonus;
    }
    if r.flame && bow.arrow == ArrowType::Fire {
        damage *= 1.25;
    }
    damage *= bow.damage_mult;
    if bow.multi && bow.arrow != ArrowType::Ender {
        damage *= multi_damage_factor(i);
    }
    damage
}

/// `setSecondsOnFire`: Flame sets 100 seconds, the flame bow's own arrows 50; a frost arrow clears it on
/// its first tick (`frostArrowsShouldBeCold`) so it is never burning.
pub fn fire_seconds(bow: &Bow, flame: bool) -> u32 {
    if bow.arrow == ArrowType::Frost && config::FROST_ARROWS_SHOULD_BE_COLD {
        0
    } else if flame {
        100
    } else if bow.arrow == ArrowType::Fire {
        50
    } else {
        0
    }
}

/// The pitch of a shot: `1.0F / (random * 0.4F + base) + shotVelocity * k`.
fn pitch(random: f32, base: f32, sv: f32, k: f32) -> f32 {
    (1.0 / ((random * 0.4) + base)) + (sv * k)
}

/// Where a delayed ender arrow turns up from where it was shot, and its sound (`ArrowSpawner.tick`, the
/// `switch (i)`): offset, volume, and the `random * 0.4F + base` base and the `shotVelocity` factor.
pub fn spawner_step(i: usize) -> ([f64; 3], f32, f32, f32) {
    match i {
        2 => ([-1.25, 1.0, 1.75], 1.0, 1.2, 0.5),
        3 => ([-2.25, 1.45, -0.75], 0.25, 1.0, 0.3),
        4 => ([0.25, 2.0, 2.5], 1.0, 1.2, 0.5),
        5 => ([1.75, 1.75, 1.5], 0.5, 1.0, 0.4),
        _ => ([0.0, 0.0, 0.0], 0.5, 1.0, 0.4),
    }
}
/// `ArrowSpawner.tick`: the first arrow at tick 1, the others at tick 61.
pub const ENDER_FIRST_TICK: i32 = 1;
pub const ENDER_REST_TICK: i32 = 61;

/// The sound a delayed arrow makes when it turns up: `(i & 1) != 0 ? ENDERMAN_TELEPORT : ARROW_SHOOT`.
pub fn spawner_sound(i: usize) -> ShotSound {
    let (_, volume, base, k) = spawner_step(i);
    ShotSound {
        event: if i & 1 != 0 { "minecraft:entity.enderman.teleport" } else { "minecraft:entity.arrow.shoot" },
        volume,
        pitch_base: base,
        pitch_k: k,
        x_offset_sign: 0.0,
    }
}

/// `CustomBow.releaseUsing`, from the draw to the list of arrows. `None` when the draw was too short
/// (`shotVelocity < 0.1`). `random` is the player's.
pub fn plan_shot(bow: &Bow, r: &Release, random: &mut impl Rng) -> Option<Shot> {
    let mut shot_velocity = raw_velocity(r.charge, bow.drawback_div);
    if shot_velocity < 0.1 {
        return None;
    }
    // `maxAmmo`: the multi bow shoots two arrows, one time in four three; the ender bow six.
    let max_ammo = if bow.multi {
        if bow.arrow == ArrowType::Ender {
            6
        } else if random.next_int_bound(4) == 0 {
            3
        } else {
            2
        }
    } else {
        1
    };
    // AlwaysCustomArrows: one arrow used, `maxAmmo` shot.
    let ammo_used = if r.infinite_ammo { 0 } else { 1 };
    let critical = shot_velocity >= 1.0;
    if critical {
        shot_velocity = 1.0;
    }
    // The multi bow's arrows are vanilla ones (`arrowHelper`), the ender, flame and frost bows' custom.
    let custom = bow.arrow;
    let mut arrows = Vec::new();
    for i in 0..max_ammo {
        let speed = shot_velocity * velocity_factor(bow, i);
        let delay_ticks = if bow.arrow == ArrowType::Ender && bow.multi {
            if i == 0 { ENDER_FIRST_TICK } else { ENDER_REST_TICK }
        } else {
            0
        };
        arrows.push(ArrowPlan {
            index: i,
            speed,
            custom,
            base_damage: base_damage(bow, r, i),
            fire_seconds: fire_seconds(bow, r.flame),
            // The first arrow of an infinite shot, and every extra one (`pickStatus` CREATIVE_ONLY).
            creative_only: (i == 0 && r.infinite_ammo) || i > 0,
            x_offset_sign: if bow.multi && bow.arrow != ArrowType::Ender && i > 0 {
                f64::from(((i as i32 & 1) << 1) - 1)
            } else {
                0.0
            },
            delay_ticks,
            spawn_offset: if delay_ticks == ENDER_REST_TICK { spawner_step(i).0 } else { [0.0; 3] },
        });
    }
    // The release sounds: the ender bow's arrows make theirs when they turn up (spawner_sound).
    let shoot = |x_offset_sign: f64| ShotSound {
        event: "minecraft:entity.arrow.shoot",
        volume: 1.0,
        pitch_base: 1.2,
        pitch_k: 0.5,
        x_offset_sign,
    };
    let mut sounds = Vec::new();
    if bow.multi && bow.arrow != ArrowType::Ender {
        // The multi bow: one sound at the player, a second beside him if two arrows go, a third on the other side.
        sounds.push(shoot(0.0));
        if max_ammo > 1 {
            sounds.push(shoot(1.0));
        }
        if max_ammo > 2 {
            sounds.push(shoot(-1.0));
        }
    } else {
        sounds.push(shoot(0.0));
    }
    Some(Shot { arrows, critical, ammo_used, sounds, shot_velocity })
}

// ---------------------------------------------------------------------------------- frost arrows

/// What a frost arrow does to the entity it hits (`CustomArrow.onHitEntity`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrostHit {
    /// A blaze takes three times the damage (`setBaseDamage(getBaseDamage() * 3.0)`).
    pub damage_factor: f64,
    /// `entityHit.clearFire()`.
    pub clear_fire: bool,
    /// Slowness (`MobEffects.MOVEMENT_SLOWDOWN`) for a living entity: ticks and amplifier.
    pub slowness: (i32, i32),
}
pub fn frost_hit(blaze: bool) -> FrostHit {
    FrostHit {
        damage_factor: if config::FROST_ARROWS_SHOULD_BE_COLD && blaze { 3.0 } else { 1.0 },
        clear_fire: config::FROST_ARROWS_SHOULD_BE_COLD,
        slowness: (300, 2),
    }
}

/// `inGroundTime` of a stuck frost arrow: the tick it stops being pick-up-able, the tick it freezes its
/// surroundings and the tick it is gone.
pub const FROST_NO_PICKUP_TICK: i32 = 1;
pub const FROST_FREEZE_TICK: i32 = 65;

/// The block a frost arrow sits in, as the freezing reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Air,
    Water,
    /// A snow layer block with this many layers (1..=8).
    Snow(u8),
    Other,
}
/// The column of blocks around the arrow.
pub trait Column {
    fn cell(&mut self, p: Pos) -> Cell;
    /// `Blocks.SNOW.defaultBlockState().canSurvive(level, pos)`.
    fn snow_can_survive(&mut self, p: Pos) -> bool;
}
/// What the freezing does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freeze {
    /// A snow layer (one layer) where the arrow is.
    Snow(Pos),
    /// Ice instead of the water.
    Ice(Pos),
    /// One more layer on this snow block (the new layer count).
    MoreSnow(Pos, u8),
}
/// `CustomArrow.tick`, `inGroundTime == 65`.
pub fn freeze(world: &mut impl Column, at: Pos) -> Option<Freeze> {
    match world.cell(at) {
        Cell::Air if world.snow_can_survive(at) => Some(Freeze::Snow(at)),
        Cell::Water => Some(Freeze::Ice(at)),
        Cell::Snow(_) => {
            // up while the layer is full (at most 1024 blocks), then the first with room, or air.
            let mut pos = at;
            let mut level = 8u8;
            let mut up = 0;
            let mut cell = world.cell(pos);
            while up < 1024 {
                match cell {
                    Cell::Snow(l) => {
                        level = l;
                        if l > 7 {
                            pos = [pos[0], pos[1] + 1, pos[2]];
                            cell = world.cell(pos);
                            up += 1;
                            continue;
                        }
                        break;
                    }
                    _ => break,
                }
            }
            if let (Cell::Snow(_), true) = (cell, level < 8) {
                Some(Freeze::MoreSnow(pos, level + 1))
            } else if cell == Cell::Air && world.snow_can_survive(pos) {
                Some(Freeze::Snow(pos))
            } else {
                None
            }
        }
        _ => None,
    }
}

// ------------------------------------------------------------------------------ the items' numbers

/// `Item.Properties.durability(..)` etc. as the item entries of `items.json` take them.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemNumbers {
    pub id: String,
    pub max_damage: u32,
    pub repair: Vec<&'static str>,
    pub fire_resistant: bool,
}
pub fn item_numbers() -> Vec<ItemNumbers> {
    BOWS.iter()
        .map(|b| ItemNumbers {
            id: b.id(),
            max_damage: b.durability,
            repair: b.repair.to_vec(),
            fire_resistant: b.fire_resistant,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------- the FOV

/// `Client.FOV` (ComputeFovModifierEvent): the multiplier a drawn bow puts on the field of view,
/// `1 - min(1, ticks / powerDiv)^2 * 0.15`.
pub fn fov_factor(bow: &Bow, ticks_using: i32) -> f32 {
    let mut custom = ticks_using as f32 / bow.drawback_div;
    if custom > 1.0 {
        custom = 1.0;
    } else {
        custom *= custom;
    }
    1.0 - (custom * 0.15)
}

}
use horizon_client::arrows::Pickup;
use horizon_client::items::ItemStack;
use horizon_client::mobs::Who;
use horizon_client::projectiles::{Projectile, ProjectileKind};
use horizon_core::geometry::Vec3;
use horizon_core::position::BlockPos;
use horizon_core::random::RandomSource;

// kit_blocks (tools/kit): block names, properties and tags as the finished mods read them
// (fallingtree, comforts, kleeslabs, respawn_anchor.rs on the pin). Pasted by `tools/hnmod.py bundle`
// in place of `include!("kit_blocks.rs");` inside the mod's runtime module `crate::game::<id>`.
// Use: `kit_blocks::name_at(self, pos)`, `kit_blocks::block_in_tag(self, "minecraft:logs", &name)`.
// Tag names are the 26.3 ones (`minecraft:logs`, `c:...`); 1.20 names (`forge:...`) do not exist.
#[allow(dead_code)]
mod kit_blocks {
    use crate::game::Game;
    use horizon_world::registry::BlockStateId;

    /// Inside the world height and in a loaded chunk.
    pub(super) fn loaded(g: &Game<'_>, p: [i32; 3]) -> bool {
        p[1] >= g.dim.min_y() && p[1] <= g.dim.max_y() && g.world.loaded(p[0], p[2])
    }
    /// The state at `p` (air outside the world, as `world.block` answers).
    pub(super) fn state_at(g: &Game<'_>, p: [i32; 3]) -> BlockStateId {
        g.world.block(p[0], p[1], p[2])
    }
    /// The block name of a state (`"minecraft:oak_log"`).
    pub(super) fn name_of(g: &Game<'_>, state: BlockStateId) -> std::sync::Arc<str> {
        g.res.facts().get(state).name.clone()
    }
    /// The block name at `p`; `None` when not loaded or outside the world.
    pub(super) fn name_at(g: &Game<'_>, p: [i32; 3]) -> Option<std::sync::Arc<str>> {
        loaded(g, p).then(|| name_of(g, state_at(g, p)))
    }
    /// A block state property (`facing`, `half`, `age`...).
    pub(super) fn prop(g: &Game<'_>, state: BlockStateId, key: &str) -> Option<String> {
        g.pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get(key).cloned())
    }
    /// The same block with some properties changed; `None` if a value is not valid.
    pub(super) fn with_props(
        g: &Game<'_>,
        state: BlockStateId,
        props: &[(&str, &str)],
    ) -> Option<BlockStateId> {
        g.with_props(state, props)
    }
    /// `state.is(BlockTags.X)`: block tag membership by block name.
    pub(super) fn block_in_tag(g: &Game<'_>, tag: &str, block: &str) -> bool {
        g.res
            .items()
            .block_tags
            .get(tag)
            .is_some_and(|s| s.contains(block))
    }
    /// `stack.is(ItemTags.X)`: item tag membership by item name.
    pub(super) fn item_in_tag(g: &Game<'_>, tag: &str, item: &str) -> bool {
        g.res
            .items()
            .item_tags
            .get(tag)
            .is_some_and(|s| s.contains(item))
    }
    /// The item in the hand that acts (`getItemInHand(hand)`, `Game::held`).
    pub(super) fn held_name(g: &Game<'_>) -> Option<String> {
        g.held().map(|s| s.item.to_string())
    }
    /// The held item is in an item tag (`minecraft:axes`, `minecraft:pickaxes`...).
    pub(super) fn held_in_tag(g: &Game<'_>, tag: &str) -> bool {
        g.held().is_some_and(|s| item_in_tag(g, tag, &s.item))
    }
    /// The player is sneaking (`isShiftKeyDown`), as fallingtree reads it.
    pub(super) fn sneaking(g: &Game<'_>) -> bool {
        g.player.as_ref().is_some_and(|p| p.shift_key_down)
    }
    /// A block of namespace `namespace` (`is_mod_block(name, "mcwwindows")`).
    pub(super) fn is_mod_block(name: &str, namespace: &str) -> bool {
        name.strip_prefix(namespace)
            .is_some_and(|rest| rest.starts_with(':'))
    }
}
// kit_sound (tools/kit): sounds, level events and particles at a block. Pasted by
// `tools/hnmod.py bundle` in place of `include!("kit_sound.rs");`.
// - `audio.play` is captured and sent to LAN guests too (`Audio::play` vs `play_unshared`).
// - `level_event_at` queues `Level.levelEvent` (2001 = block break particles + sound, 1000s =
//   sounds); everyone around sees/hears it.
// - `particles.add` is this window only (guests do not see it).
#[allow(dead_code)]
mod kit_sound {
    use crate::game::Game;

    fn center(p: [i32; 3]) -> [f64; 3] {
        [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]) + 0.5,
            f64::from(p[2]) + 0.5,
        ]
    }
    /// `level.playSound(null, pos, SoundEvents.X, SoundSource.BLOCKS, volume, pitch)`.
    pub(super) fn at_block(g: &mut Game<'_>, sound: &str, pos: [i32; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(center(pos)), volume, pitch);
    }
    /// A sound at an exact point (entities, items).
    pub(super) fn at(g: &mut Game<'_>, sound: &str, at: [f64; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(at), volume, pitch);
    }
    /// `level.levelEvent(id, pos, data)`; `2001` with the block state id = break effect.
    pub(super) fn level_event(g: &mut Game<'_>, id: i32, pos: [i32; 3], data: i32) {
        g.level_event_at(id, pos, data);
    }
    /// `level.addParticle(type, x, y, z, dx, dy, dz)` for this window only.
    pub(super) fn particle(
        g: &mut Game<'_>,
        ty: horizon_client::particles::ParticleType,
        at: [f64; 3],
        velocity: [f64; 3],
    ) {
        let options = horizon_client::particles::ParticleOptions::simple(ty);
        g.particles.add(&options, at, velocity);
    }
}

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
