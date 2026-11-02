// More Bows' items in the content root (horizon_client::mod_content). The eight bows copy the components of
// `minecraft:bow`; this sets the numbers of the original (`MoreBows.getAllItems`, `ConfigBows`):
// durability, the repair material (`CustomBow.isValidRepairItem`) and, for the flame bow,
// `fireResistant()`. A custom bow is not `Items.BOW`, so it is not a furnace fuel as the bow is.
use super::J;
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

/// Niciun fișier de config pentru acest mod în profilul pachetului (`morebows_bowstats.json` și
/// `morebows_general.json` lipsesc): valorile sunt cele implicite din `ConfigBows` / `ConfigGeneral`.
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

/// `items.json` after the mods' items were copied in: their components get the original's numbers.
pub(super) fn patch_items(map: &mut J, components: bool) {
    if !components {
        return;
    }
    for n in engine::item_numbers() {
        let Some(entry) = map.get_mut(&n.id) else {
            continue;
        };
        entry.set("minecraft:max_damage", J::Num(n.max_damage.into()));
        let repair: Vec<J> = n.repair.iter().map(|r| J::Str((*r).to_owned())).collect();
        entry.set(
            "minecraft:repairable",
            J::Obj(vec![("items".to_owned(), J::Arr(repair))]),
        );
        if n.fire_resistant {
            entry.set(
                "minecraft:damage_resistant",
                J::Obj(vec![("types".to_owned(), J::Str("#minecraft:is_fire".to_owned()))]),
            );
        }
        if let J::Obj(o) = entry {
            o.retain(|(k, _)| k != "minecraft:cooking_fuel");
        }
    }
}
