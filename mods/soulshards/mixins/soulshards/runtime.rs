// Soul Shards inside Horizon's game (nightly 142388e1c). The rules are the engine (src/lib.rs); this part reads
// and changes the world. World logic runs where the world is (the integrated server in single player: the window
// is its guest), never on a remote client; the tooltip runs on the window that draws it.
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    // Soul Shards (CreeperHost / TehNut, MIT; `soulshards-1.20-1.3.6.jar`) for Horizon: the rules, without game code.
// Written from the 1.3.6 jar (decompiled: EventHandler, BlockSoulCage, TileEntitySoulCage, ItemSoulShard, Binding,
// Tier, MultiblockPattern, SoulShardsAPI, ItemVileSword, EnchantmentSoulStealer) and the pack's
// `config/soulshards/soulshards.json` + `tiers.json`. The public 1.20 branch (1.3.10) differs (SoulSpawnerLogic,
// spawn rules, particles): the jar is followed. This file is pasted into the game as
// `crate::game::soulshards::engine` (tools/hnmod.py bundle) and is also the crate the tests use.
use std::collections::BTreeMap;

pub type Pos = [i32; 3];

/// the pack's `config/soulshards/soulshards.json` (the `balance` values the port uses).
pub mod config {
    pub const BALANCE_ALLOW_SPAWNER_ABSORPTION: bool = true; // soulshards.json [balance] allowSpawnerAbsorption
    pub const BALANCE_ABSORPTION_BONUS: i32 = 200; // soulshards.json [balance] absorptionBonus
    pub const BALANCE_ALLOW_BOSS_SPAWNS: bool = false; // soulshards.json [balance] allowBossSpawns
    pub const BALANCE_COUNT_CAGE_BORN_FOR_SHARD: bool = false; // soulshards.json [balance] countCageBornForShard
    pub const BALANCE_REQUIRE_OWNER_ONLINE: bool = false; // soulshards.json [balance] requireOwnerOnline
    pub const BALANCE_REQUIRE_REDSTONE_SIGNAL: bool = false; // soulshards.json [balance] requireRedstoneSignal
    pub const BALANCE_ALLOW_SHARD_COMBINATION: bool = true; // soulshards.json [balance] allowShardCombination
    pub const BALANCE_SPAWN_CAP: usize = 32; // soulshards.json [balance] spawnCap
    pub const BALANCE_DROP_EXPERIENCE: bool = false; // soulshards.json [balance] dropExperience
    pub const CLIENT_DISPLAY_DURABILITY_BAR: bool = true; // soulshards.json [client] displayDurabilityBar
}

/// One entry of the pack's `tiers.json` (`Tier`): kills needed, which checks the cage makes, mobs per spawn, ticks
/// between spawns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier {
    pub kill_requirement: i32,
    pub check_player: bool,
    pub check_light: bool,
    pub check_redstone: bool,
    pub spawn_amount: i32,
    pub cooldown: i32,
}

/// `config/soulshards/tiers.json` of the pack, in file order (`Tier.INDEXED`); equal to the jar's
/// `Tier.generateDefaults`.
pub const TIERS: [Tier; 6] = [
    Tier { kill_requirement: 0, check_player: true, check_light: false, check_redstone: false, spawn_amount: 0, cooldown: 0 },
    Tier { kill_requirement: 64, check_player: true, check_light: true, check_redstone: false, spawn_amount: 2, cooldown: 400 },
    Tier { kill_requirement: 128, check_player: true, check_light: true, check_redstone: false, spawn_amount: 4, cooldown: 200 },
    Tier { kill_requirement: 256, check_player: false, check_light: true, check_redstone: false, spawn_amount: 4, cooldown: 100 },
    Tier { kill_requirement: 512, check_player: false, check_light: false, check_redstone: false, spawn_amount: 4, cooldown: 100 },
    Tier { kill_requirement: 1024, check_player: false, check_light: false, check_redstone: true, spawn_amount: 6, cooldown: 40 },
];

/// `Tier.maxKills`: the largest kill requirement.
pub fn max_kills() -> i32 {
    TIERS.iter().map(|t| t.kill_requirement).max().unwrap_or(0)
}

/// `Binding.getTier`: `TIERS.floorEntry(kills)` (by requirement), and its index in file order (`getIndex`).
pub fn tier_index(kills: i32) -> usize {
    let mut best: Option<(i32, usize)> = None;
    for (i, t) in TIERS.iter().enumerate() {
        if t.kill_requirement <= kills && best.is_none_or(|(r, _)| t.kill_requirement > r) {
            best = Some((t.kill_requirement, i));
        }
    }
    // No entry below the kills (negative kills): floorEntry is null in Java; the tier-0 shard is the safe answer.
    best.map_or(0, |(_, i)| i)
}
pub fn tier(kills: i32) -> Tier {
    TIERS[tier_index(kills)]
}

/// The `binding` compound of a shard (`Binding.serializeNBT`): `bound` (entity id), `owner` (UUID), `kills`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Binding {
    pub bound: Option<String>,
    pub owner: Option<[i32; 4]>,
    pub kills: i32,
}
impl Binding {
    /// `Binding.addKills`: capped at `maxKills`.
    pub fn add_kills(&mut self, n: i32) {
        self.kills = max_kills().min(self.kills.saturating_add(n));
    }
    pub fn tier(&self) -> Tier {
        tier(self.kills)
    }
    pub fn tier_index(&self) -> usize {
        tier_index(self.kills)
    }
    /// `ItemSoulShard.isFoil`: the shard is full.
    pub fn is_full(&self) -> bool {
        self.kills >= max_kills()
    }
}

/// What the rules see of one stack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShardView {
    /// The stack is a `soulshards:soul_shard`.
    pub is_shard: bool,
    /// Its binding (`Binding.fromNBT`), if it has one.
    pub binding: Option<Binding>,
}

/// Where the shard that takes a kill is (`EventHandler.getFirstShard`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShardSlot {
    OffHand,
    Hotbar(usize),
}

/// A shard takes souls of `entity` when it has no binding or no bound entity, or is bound to it and not full.
fn accepts(b: &Option<Binding>, entity: &str) -> bool {
    match b {
        None => true,
        Some(b) => match &b.bound {
            None => true,
            Some(e) => e == entity && b.kills < max_kills(),
        },
    }
}

/// `EventHandler.getFirstShard`: the offhand first; when it holds no shard, the first fitting shard of the hotbar
/// (slots 0-8). An offhand shard that does not fit means no shard at all (the hotbar is not searched then).
pub fn first_shard(offhand: &Option<ShardView>, hotbar: &[Option<ShardView>; 9], entity: &str) -> Option<ShardSlot> {
    match offhand {
        Some(s) if s.is_shard => accepts(&s.binding, entity).then_some(ShardSlot::OffHand),
        _ => hotbar.iter().enumerate().find_map(|(i, s)| match s {
            Some(s) if s.is_shard && accepts(&s.binding, entity) => Some(ShardSlot::Hotbar(i)),
            _ => None,
        }),
    }
}

/// What the kill of a mob was, for `EventHandler.onEntityKill`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kill {
    pub entity: String,
    /// The killing blow came from a player (`source.getEntity() instanceof Player`).
    pub by_player: bool,
    /// `SoulShardsAPI.isAllowed`: the type exists and is not in `#soulshards:no_capture`.
    pub allowed: bool,
    /// In `#forge:bosses` (26.3 `#c:bosses`).
    pub boss: bool,
    /// `getPersistentData().getBoolean("cageBorn")`.
    pub cage_born: bool,
}

/// The kill counts for a shard (the gates of `onEntityKill`, in order; players never reach this).
pub fn kill_counts(k: &Kill) -> bool {
    k.allowed
        && (config::BALANCE_ALLOW_BOSS_SPAWNS || !k.boss)
        && (config::BALANCE_COUNT_CAGE_BORN_FOR_SHARD || !k.cage_born)
        && k.by_player
}

/// Souls a kill gives: 1 + the Soul Stealer level of the main hand + 1 for a Vile Sword (`ISoulWeapon`).
pub fn souls_for_kill(soul_stealer: u32, vile_sword: bool) -> i32 {
    1 + soul_stealer as i32 + i32::from(vile_sword)
}

/// The shard's binding after a kill (`binding.setBoundEntity` / `setOwner` when missing, then `addKills`).
pub fn bind_kill(old: Option<Binding>, entity: &str, owner: [i32; 4], souls: i32) -> Binding {
    let mut b = old.unwrap_or_default();
    if b.bound.is_none() {
        b.bound = Some(entity.to_owned());
    }
    if b.owner.is_none() {
        b.owner = Some(owner);
    }
    b.add_kills(souls);
    b
}

/// The anvil (`EventHandler.onAnvil`): two bound shards of the same entity become one with both kill counts, cost
/// `left tier index * 6`.
pub fn combine(left: &Binding, right: &Binding) -> Option<(Binding, i32)> {
    if !config::BALANCE_ALLOW_SHARD_COMBINATION || left.bound.is_none() || left.bound != right.bound {
        return None;
    }
    let mut out = left.clone();
    out.add_kills(right.kills);
    Some((out, left.tier_index() as i32 * 6))
}

/// The item model of a shard (`SoulShards.setupClient`): bound shards show their tier (`soul_shard_tier_N`).
pub fn model_name(b: &Option<Binding>) -> Option<String> {
    b.as_ref().map(|b| format!("tier_{}", b.tier_index()))
}

/// What the world gives the rules (the adapter implements it on the game, tests on a map).
pub trait World {
    /// The block name at `p`; `None` when not loaded.
    fn block(&self, p: Pos) -> Option<String>;
}

pub const SOUL_CAGE: &str = "soulshards:soul_cage";
/// The cage with `active=true`: an alias block (no item) with the active model; the template has no `active`.
pub const SOUL_CAGE_ACTIVE: &str = "soulshards:soul_cage_active";
pub fn is_cage(name: &str) -> bool {
    name == SOUL_CAGE || name == SOUL_CAGE_ACTIVE
}

/// `MultiblockPattern.DEFAULT` (1.3.6 reads no file: `ConfigSoulShards.handleMultiblock` only takes the default):
/// catalyst diamond; shape rows along Z, columns along X, origin (1, 1) = glowstone; O = obsidian, Q = quartz block,
/// G = glowstone (any state of the block).
pub const CATALYST: &str = "minecraft:diamond";
pub const SHAPE: [&str; 3] = ["OQO", "QGQ", "OQO"];
pub fn slot_block(c: char) -> &'static str {
    match c {
        'O' => "minecraft:obsidian",
        'Q' => "minecraft:quartz_block",
        _ => "minecraft:glowstone",
    }
}
/// `MultiblockPattern.isOriginBlock`.
pub fn is_origin(block: &str) -> bool {
    block == slot_block('G')
}
/// `MultiblockPattern.match`: the 9 positions when every block fits.
pub fn multiblock_match(world: &impl World, origin: Pos) -> Option<Vec<Pos>> {
    let mut out = Vec::with_capacity(9);
    for (y, line) in SHAPE.iter().enumerate() {
        for (x, c) in line.chars().enumerate() {
            let p = [origin[0] + x as i32 - 1, origin[1], origin[2] + y as i32 - 1];
            if world.block(p).as_deref() != Some(slot_block(c)) {
                return None;
            }
            out.push(p);
        }
    }
    Some(out)
}

/// What a cage tick needs to know (`TileEntitySoulCage.canSpawn`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CageCheck {
    /// The slot holds a shard and it is bound to an entity.
    pub binding: Option<Binding>,
    /// `SoulShardsAPI.isAllowed(bound)`.
    pub allowed: bool,
    /// `POWERED` (a neighbour gives a signal).
    pub powered: bool,
    /// A non-spectator player within 16 blocks of the cage's corner.
    pub player_near: bool,
    /// The owner is online (only read with `requireOwnerOnline`).
    pub owner_online: bool,
}

/// `TileEntitySoulCage.canSpawn`: the binding when the cage may spawn.
pub fn can_spawn(c: &CageCheck) -> Option<&Binding> {
    let b = c.binding.as_ref()?;
    b.bound.as_ref()?;
    let tier = b.tier();
    if tier.spawn_amount == 0 {
        return None;
    }
    if config::BALANCE_REQUIRE_OWNER_ONLINE && !c.owner_online {
        return None;
    }
    if !c.allowed {
        return None;
    }
    if !config::BALANCE_REQUIRE_REDSTONE_SIGNAL {
        if c.powered && tier.check_redstone {
            return None;
        }
    } else if !c.powered {
        return None;
    }
    if tier.check_player && !c.player_near {
        return None;
    }
    Some(b)
}

/// The state of one cage (`TileEntitySoulCage`: `inventory`, `activeTime`, `active`). The shard is the stack's
/// NBT (hex, written by the adapter), so nothing of it is lost.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cage {
    pub shard: Option<String>,
    pub active: bool,
    pub active_time: i32,
}

/// What one tick of a cage does (`TileEntitySoulCage.tick`, server side).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CageTick {
    /// Set the block to the active (`Some(true)`) or idle (`Some(false)`) cage, with neighbour updates.
    pub set_active: Option<bool>,
    /// Spawn this tier's mobs now.
    pub spawn: bool,
}
pub fn cage_tick(cage: &mut Cage, check: &CageCheck) -> CageTick {
    match can_spawn(check) {
        None => {
            let set_active = cage.active.then_some(false);
            cage.active = false;
            CageTick { set_active, spawn: false }
        }
        Some(b) => {
            let set_active = (!cage.active).then_some(true);
            cage.active = true;
            cage.active_time = cage.active_time.wrapping_add(1);
            let cooldown = b.tier().cooldown;
            let spawn = cooldown != 0 && cage.active_time % cooldown == 0;
            CageTick { set_active, spawn }
        }
    }
}

/// One spawn attempt's position (`spawnEntities`): `x = pos.x + (d1 - d2) * 4 + 0.5`, `y = pos.y + nextInt(3)`,
/// `z` like x, then `new BlockPos((int) x, (int) y, (int) z)` (Java casts truncate towards zero), one block up when
/// it is the cage itself. `draws` = (d1, d2, n, d3, d4) in the order the jar draws them.
pub fn spawn_pos(cage: Pos, d1: f64, d2: f64, n: i32, d3: f64, d4: f64) -> Pos {
    let x = f64::from(cage[0]) + (d1 - d2) * 4.0 + 0.5;
    let y = f64::from(cage[1] + n);
    let z = f64::from(cage[2]) + (d3 - d4) * 4.0 + 0.5;
    let mut p = [x as i32, y as i32, z as i32];
    if p == cage {
        p[1] += 1;
    }
    p
}

/// `canSpawnInLight`: block light at the cage (raw brightness with the sky darkened by 15) at most 8.
pub fn light_ok(block_light_at_cage: u8) -> bool {
    block_light_at_cage <= 8
}

/// The cages of one world and dimension, saved in `<world>/data/soulshards/<dimension>.txt`:
/// `x y z active activeTime shard-hex|-` per line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub cages: BTreeMap<Pos, Cage>,
}
impl State {
    pub fn load(text: Option<String>) -> Self {
        let mut s = State::default();
        for line in text.unwrap_or_default().lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            if w.len() != 6 {
                continue;
            }
            let num = |i: usize| w[i].parse::<i32>().ok();
            let (Some(x), Some(y), Some(z), Some(t)) = (num(0), num(1), num(2), num(4)) else {
                continue;
            };
            let shard = (w[5] != "-").then(|| w[5].to_owned());
            s.cages.insert([x, y, z], Cage { shard, active: w[3] == "1", active_time: t });
        }
        s
    }
    pub fn save(&self) -> String {
        let mut out = String::new();
        for (p, c) in &self.cages {
            out.push_str(&format!(
                "{} {} {} {} {} {}\n",
                p[0],
                p[1],
                p[2],
                u8::from(c.active),
                c.active_time,
                c.shard.as_deref().unwrap_or("-")
            ));
        }
        out
    }
}

/// `String.format`-like fill of a translation (`%s` / `%d`, in order).
pub fn fill(pattern: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    let mut i = 0;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        if tail.starts_with('s') || tail.starts_with('d') {
            out.push_str(args.get(i).map_or("", String::as_str));
            i += 1;
            rest = &tail[1..];
        } else if let Some(t) = tail.strip_prefix('%') {
            out.push('%');
            rest = t;
        } else {
            out.push('%');
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

/// `ItemSoulShard.appendHoverText`: (translation key, argument) lines of a bound shard; the owner only with
/// advanced tooltips.
pub fn tooltip(b: &Binding, entity_exists: bool, advanced: bool, owner_text: impl Fn([i32; 4]) -> String) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if let Some(e) = &b.bound
        && entity_exists
    {
        out.push(("tooltip.soulshards.bound", e.clone()));
    }
    out.push(("tooltip.soulshards.tier", b.tier_index().to_string()));
    out.push(("tooltip.soulshards.kills", b.kills.to_string()));
    if advanced && let Some(o) = b.owner {
        out.push(("tooltip.soulshards.owner", owner_text(o)));
    }
    out
}

/// A UUID's four ints as Java's `UUID.toString`.
pub fn uuid_text(u: [i32; 4]) -> String {
    let hi = (u64::from(u[0] as u32) << 32) | u64::from(u[1] as u32);
    let lo = (u64::from(u[2] as u32) << 32) | u64::from(u[3] as u32);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        hi >> 32,
        (hi >> 16) & 0xffff,
        hi & 0xffff,
        lo >> 48,
        lo & 0xffff_ffff_ffff
    )
}

/// The original's plain data, read from the 1.20.1 jar by tools/date_jar.py: constants, enum values, static
/// tables, numbers given to builders, constant getters, chances, the numbers of each method, and pure static
/// methods translated from bytecode (each checked against the JVM). Use `date_jar::NAME` and cite it; delete what
/// the port does not need.
#[allow(dead_code, non_upper_case_globals, non_camel_case_types, non_snake_case, clippy::all)]
pub mod date_jar {
    // Data read from the 1.20.1 jar `soulshards-1.20-1.3.6.jar` by tools/date_jar.py (first pass, generated).
    // Each constant names the Java class and the field / method it comes from. Values that depend on a
    // branch, a parameter or the config are left out. Keep what the rules need, cite it, delete the rest.

    // ---- static final constants (ConstantValue)
    /// `net.creeperhost.soulshardsrespawn.SoulShards.MODID`
    pub const SOUL_SHARDS_MODID: &'static str = "soulshards";
    /// `net.creeperhost.soulshardsrespawn.SoulShards.NAME`
    pub const SOUL_SHARDS_NAME: &'static str = "Soul Shards";

    // ---- numbers passed to builders and constructors (durability, damage, speed, cooldowns...)
    /// `net.creeperhost.soulshardsrespawn.block.BlockSoulCage.<init>`: `BlockBehaviour.Properties.strength(3.0)`
    pub const BLOCK_SOUL_CAGE_STRENGTH: f32 = 3.0;
    /// `net.creeperhost.soulshardsrespawn.block.TileEntitySoulCage.canSpawnInLight`: `LevelLightEngine.getRawBrightness(?, 15)`
    pub const TILE_ENTITY_SOUL_CAGE_GET_RAW_BRIGHTNESS: i32 = 15;
    /// `net.creeperhost.soulshardsrespawn.core.ConfigSoulShards$ConfigBalance.<init>`: `ConfigSoulShards.ConfigBalance.new(True, False, 200, False, False, False, False, True, 32, False)`
    pub const CONFIG_BALANCE_CONFIG_SOUL_SHARDS_CONFIG_BALANCE: (bool, bool, i32, bool, bool, bool, bool, bool, i32, bool) = (true, false, 200, false, false, false, false, true, 32, false);
    /// `net.creeperhost.soulshardsrespawn.core.EventHandler.onEntityKill`: `Binding.new(?, 0)`
    pub const EVENT_HANDLER_BINDING: i32 = 0;
    /// `net.creeperhost.soulshardsrespawn.core.RegistrarSoulShards.lambda$static$3`: `CreativeModeTab.builder(CreativeModeTab.Row.TOP, 0)`
    pub const REGISTRAR_SOUL_SHARDS_BUILDER: i32 = 0;
    /// `net.creeperhost.soulshardsrespawn.core.data.Tier.generateDefaults`: `Tier.new(0, True, False, False, 0, 0)`
    pub const TIER_TIER: (i32, bool, bool, bool, i32, i32) = (0, true, false, false, 0, 0);
    /// `net.creeperhost.soulshardsrespawn.core.data.Tier.generateDefaults`: `Tier.new(64, True, True, False, 2, 400)`
    pub const TIER_TIER_2: (i32, bool, bool, bool, i32, i32) = (64, true, true, false, 2, 400);
    /// `net.creeperhost.soulshardsrespawn.core.data.Tier.generateDefaults`: `Tier.new(128, True, True, False, 4, 200)`
    pub const TIER_TIER_3: (i32, bool, bool, bool, i32, i32) = (128, true, true, false, 4, 200);
    /// `net.creeperhost.soulshardsrespawn.core.data.Tier.generateDefaults`: `Tier.new(256, False, True, False, 4, 100)`
    pub const TIER_TIER_4: (i32, bool, bool, bool, i32, i32) = (256, false, true, false, 4, 100);
    /// `net.creeperhost.soulshardsrespawn.core.data.Tier.generateDefaults`: `Tier.new(512, False, False, False, 4, 100)`
    pub const TIER_TIER_5: (i32, bool, bool, bool, i32, i32) = (512, false, false, false, 4, 100);
    /// `net.creeperhost.soulshardsrespawn.core.data.Tier.generateDefaults`: `Tier.new(1024, False, False, True, 6, 40)`
    pub const TIER_TIER_6: (i32, bool, bool, bool, i32, i32) = (1024, false, false, true, 6, 40);
    /// `net.creeperhost.soulshardsrespawn.datagen.GeneratorBlockLoot.m_245126_`: `ConstantValue.exactly(1.0)`
    pub const GENERATOR_BLOCK_LOOT_EXACTLY: f32 = 1.0;
    /// `net.creeperhost.soulshardsrespawn.item.ItemVileSword.<init>`: `SwordItem.new(ItemVileSword.MATERIAL_VILE, 3, -2.4000000953674316, ?)`
    pub const ITEM_VILE_SWORD_SWORD_ITEM: (i32, f32) = (3, -2.4);

    // ---- methods that only return a constant (data overrides)
    /// `net.creeperhost.soulshardsrespawn.block.BlockSoulCage.canConnectRedstone(BlockState, BlockGetter, BlockPos, Direction)` returns it
    pub const BLOCK_SOUL_CAGE_CAN_CONNECT_REDSTONE: bool = true;
    /// `net.creeperhost.soulshardsrespawn.block.BlockSoulCage.hasAnalogOutputSignal(BlockState)` returns it
    pub const BLOCK_SOUL_CAGE_HAS_ANALOG_OUTPUT_SIGNAL: bool = true;
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiBlockDrawable.getWidth()` returns it
    pub const MULTI_BLOCK_DRAWABLE_GET_WIDTH: i32 = 43;
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiBlockDrawable.getHeight()` returns it
    pub const MULTI_BLOCK_DRAWABLE_GET_HEIGHT: i32 = 31;
    /// `net.creeperhost.soulshardsrespawn.core.util.EnchantmentSoulStealer.getMaxLevel()` returns it
    pub const ENCHANTMENT_SOUL_STEALER_GET_MAX_LEVEL: i32 = 5;
    /// `net.creeperhost.soulshardsrespawn.datagen.GeneratorItemModels.getName()` returns it
    pub const GENERATOR_ITEM_MODELS_GET_NAME: &'static str = "Item Models";
    /// `net.creeperhost.soulshardsrespawn.item.ItemVileSword.getSoulBonus(ItemStack, Player, LivingEntity)` returns it
    pub const ITEM_VILE_SWORD_GET_SOUL_BONUS: i32 = 1;

    // ---- numbers in each method body (bytecode order, once each, without 0 / 1 / -1): a checklist of the
    // values the original uses there, not named constants. Ints exact; f32 values written with their f32 digits.
    /// `net.creeperhost.soulshardsrespawn.block.BlockSoulCage.<init>()`: floats
    pub const BLOCK_SOUL_CAGE_INIT_FLOATS: &[f64] = &[3.0];
    /// `net.creeperhost.soulshardsrespawn.block.BlockSoulCage.tick(BlockState, ServerLevel, BlockPos, RandomSource)`: integers
    pub const BLOCK_SOUL_CAGE_TICK_INTS: &[i64] = &[3];
    /// `net.creeperhost.soulshardsrespawn.block.BlockSoulCage.getAnalogOutputSignal(BlockState, Level, BlockPos)`: floats
    pub const BLOCK_SOUL_CAGE_GET_ANALOG_OUTPUT_SIGNAL_FLOATS: &[f64] = &[15.0];
    /// `net.creeperhost.soulshardsrespawn.block.BlockSoulCage.createBlockStateDefinition(StateDefinition$Builder)`: integers
    pub const BLOCK_SOUL_CAGE_CREATE_BLOCK_STATE_DEFINITION_INTS: &[i64] = &[2];
    /// `net.creeperhost.soulshardsrespawn.block.BlockSoulCage.handleRedstoneChange(Level, BlockState, BlockPos)`: integers
    pub const BLOCK_SOUL_CAGE_HANDLE_REDSTONE_CHANGE_INTS: &[i64] = &[2];
    /// `net.creeperhost.soulshardsrespawn.block.TileEntitySoulCage.spawnEntities()`: integers
    pub const TILE_ENTITY_SOUL_CAGE_SPAWN_ENTITIES_INTS: &[i64] = &[5, 3];
    /// `net.creeperhost.soulshardsrespawn.block.TileEntitySoulCage.spawnEntities()`: floats
    pub const TILE_ENTITY_SOUL_CAGE_SPAWN_ENTITIES_FLOATS: &[f64] = &[4.0, 0.5, 3.6e2];
    /// `net.creeperhost.soulshardsrespawn.block.TileEntitySoulCage.canSpawn()`: floats
    pub const TILE_ENTITY_SOUL_CAGE_CAN_SPAWN_FLOATS: &[f64] = &[16.0];
    /// `net.creeperhost.soulshardsrespawn.block.TileEntitySoulCage.canSpawnInLight(LivingEntity, BlockPos)`: integers
    pub const TILE_ENTITY_SOUL_CAGE_CAN_SPAWN_IN_LIGHT_INTS: &[i64] = &[15, 8];
    /// `net.creeperhost.soulshardsrespawn.block.TileEntitySoulCage.hasReachedSpawnCap(LivingEntity)`: integers
    pub const TILE_ENTITY_SOUL_CAGE_HAS_REACHED_SPAWN_CAP_INTS: &[i64] = &[16];
    /// `net.creeperhost.soulshardsrespawn.block.TileEntitySoulCage.setState(Z)`: integers
    pub const TILE_ENTITY_SOUL_CAGE_SET_STATE_INTS: &[i64] = &[3];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiBlockDrawable.getWidth()`: integers
    pub const MULTI_BLOCK_DRAWABLE_GET_WIDTH_INTS: &[i64] = &[43];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiBlockDrawable.getHeight()`: integers
    pub const MULTI_BLOCK_DRAWABLE_GET_HEIGHT_INTS: &[i64] = &[31];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiBlockDrawable.draw(GuiGraphics, I, I)`: integers
    pub const MULTI_BLOCK_DRAWABLE_DRAW_INTS: &[i64] = &[13, 20, 4, 7, 8, 27, 12, 14, 15];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiBlockDrawable.draw(GuiGraphics, I, I)`: floats
    pub const MULTI_BLOCK_DRAWABLE_DRAW_FLOATS: &[f64] = &[1.0e2, 5.0];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiblockCategory.<init>(IGuiHelper)`: integers
    pub const MULTIBLOCK_CATEGORY_INIT_INTS: &[i64] = &[176, 14, 24, 17];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiblockCategory.getBackground()`: integers
    pub const MULTIBLOCK_CATEGORY_GET_BACKGROUND_INTS: &[i64] = &[114, 131];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiblockCategory.draw(MultiblockCategory$Recipe, IRecipeSlotsView, GuiGraphics, D, D)`: integers
    pub const MULTIBLOCK_CATEGORY_DRAW_INTS: &[i64] = &[10, 30, 60, 2, 70, 80, 90];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiblockCategory.getTooltipStrings(MultiblockCategory$Recipe, IRecipeSlotsView, D, D)`: integers
    pub const MULTIBLOCK_CATEGORY_GET_TOOLTIP_STRINGS_INTS: &[i64] = &[10, 30, 40, 36];
    /// `net.creeperhost.soulshardsrespawn.compat.jei.MultiblockCategory.setRecipe(IRecipeLayoutBuilder, MultiblockCategory$Recipe, IFocusGroup)`: integers
    pub const MULTIBLOCK_CATEGORY_SET_RECIPE_INTS: &[i64] = &[23, 90, 30];
    /// `net.creeperhost.soulshardsrespawn.core.ConfigSoulShards$ConfigBalance.<init>()`: integers
    pub const CONFIG_BALANCE_INIT_INTS: &[i64] = &[200, 32];
    /// `net.creeperhost.soulshardsrespawn.core.EventHandler.onAnvil(AnvilUpdateEvent)`: integers
    pub const EVENT_HANDLER_ON_ANVIL_INTS: &[i64] = &[6];
    /// `net.creeperhost.soulshardsrespawn.core.EventHandler.getFirstShard(Player, ResourceLocation)`: integers
    pub const EVENT_HANDLER_GET_FIRST_SHARD_INTS: &[i64] = &[9];
    /// `net.creeperhost.soulshardsrespawn.core.data.MultiblockPattern$1.<init>()`: integers
    pub const C_1_INIT_INTS: &[i64] = &[79, 81, 71];
    /// `net.creeperhost.soulshardsrespawn.core.data.MultiblockPattern.<init>(ItemStack, [Ljava/lang/String;, Point, Map)`: integers
    pub const MULTIBLOCK_PATTERN_INIT_INTS: &[i64] = &[32];
    /// `net.creeperhost.soulshardsrespawn.core.data.Tier.generateDefaults()`: integers
    pub const TIER_GENERATE_DEFAULTS_INTS: &[i64] = &[6, 64, 2, 400, 128, 4, 200, 3, 256, 100, 512, 5, 1024, 40];
    /// `net.creeperhost.soulshardsrespawn.core.util.EnchantmentSoulStealer.getMinCost(I)`: integers
    pub const ENCHANTMENT_SOUL_STEALER_GET_MIN_COST_INTS: &[i64] = &[11];
    /// `net.creeperhost.soulshardsrespawn.core.util.EnchantmentSoulStealer.getMaxCost(I)`: integers
    pub const ENCHANTMENT_SOUL_STEALER_GET_MAX_COST_INTS: &[i64] = &[20];
    /// `net.creeperhost.soulshardsrespawn.core.util.EnchantmentSoulStealer.getMaxLevel()`: integers
    pub const ENCHANTMENT_SOUL_STEALER_GET_MAX_LEVEL_INTS: &[i64] = &[5];
    /// `net.creeperhost.soulshardsrespawn.item.ItemSoulShard.getMaxStackSize(ItemStack)`: integers
    pub const ITEM_SOUL_SHARD_GET_MAX_STACK_SIZE_INTS: &[i64] = &[64];
    /// `net.creeperhost.soulshardsrespawn.item.ItemVileSword.<init>()`: integers
    pub const ITEM_VILE_SWORD_INIT_INTS: &[i64] = &[3];
    /// `net.creeperhost.soulshardsrespawn.item.ItemVileSword.<init>()`: floats
    pub const ITEM_VILE_SWORD_INIT_FLOATS: &[f64] = &[-2.4];
}

}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "soulshards";

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
// kit_state (tools/kit): per-world mod state in side files, the torchmaster pattern
// (`<world>/data/<MOD_ID>/<dimension>.txt`, written tmp + rename). Pasted by `tools/hnmod.py bundle` in
// place of `include!("kit_state.rs");`. The adapter must define `const MOD_ID: &str = "<id>";`.
//
// Wiring (both hooks are in HOOKS.md, verified by torchmaster):
//   HEAD `horizon_client::game::chat::Game<'_>::load_world_state`, bindings = []
//        -> helper calls `crate::game::Game::<id>_world_opened()` -> `STATE.with(|s| s.borrow_mut().clear())`
//   HEAD `horizon_client::game::Game<'_>::save_game_metadata`, bindings game = &mut *self
//        -> helper calls `game.<id>_save()` -> `PerWorld::save(..)`
// The state is loaded lazily on first use, keyed by (world folder, dimension), so it is right
// on the integrated server thread and on a dedicated LAN host alike; a client of a remote
// server (`remote.is_some()`) never reads or writes files.
//
//   thread_local! { static STATE: RefCell<kit_state::PerWorld<MyState>> = RefCell::new(kit_state::PerWorld::new()); }
//   STATE.with(|s| { let mut s = s.borrow_mut(); let st = s.level(self, MyState::load); st.x += 1; s.dirty(self); });
#[allow(dead_code)]
mod kit_state {
    use crate::game::Game;
    use std::path::PathBuf;

    /// `<world>/data/<MOD_ID>/`, or `None` when there is no local world (remote server, menu).
    pub(super) fn dir(g: &Game<'_>) -> Option<PathBuf> {
        if g.remote.is_some() || g.world_root.as_os_str().is_empty() {
            return None;
        }
        Some(g.world_root.join("data").join(super::MOD_ID))
    }
    /// The current dimension as a file name (`minecraft_overworld`).
    pub(super) fn dim_key(g: &Game<'_>) -> String {
        g.dim.id().replace([':', '/'], "_")
    }
    /// Reads `<world>/data/<MOD_ID>/<file>`.
    pub(super) fn read(g: &Game<'_>, file: &str) -> Option<String> {
        std::fs::read_to_string(dir(g)?.join(file)).ok()
    }
    /// Writes `<world>/data/<MOD_ID>/<file>` atomically; logs and returns false on failure.
    pub(super) fn write(g: &Game<'_>, file: &str, text: &str) -> bool {
        let Some(d) = dir(g) else { return false };
        let path = d.join(file);
        let tmp = d.join(format!("{file}.tmp"));
        let written = std::fs::create_dir_all(&d)
            .and_then(|()| std::fs::write(&tmp, text))
            .and_then(|()| std::fs::rename(&tmp, &path));
        if let Err(e) = &written {
            log::warn!("{}: {} not saved: {e}", super::MOD_ID, path.display());
        }
        written.is_ok()
    }

    struct Level<T> {
        root: PathBuf,
        dim: String,
        state: T,
        dirty: bool,
    }
    /// One `T` per (world folder, dimension), loaded from `<dim>.txt` on first use.
    pub(super) struct PerWorld<T> {
        levels: Vec<Level<T>>,
    }
    impl<T> PerWorld<T> {
        pub(super) const fn new() -> Self {
            Self { levels: Vec::new() }
        }
        fn index(&self, g: &Game<'_>) -> Option<usize> {
            let dim = dim_key(g);
            self.levels
                .iter()
                .position(|l| l.root == g.world_root && l.dim == dim)
        }
        /// The state of the current world and dimension; `load` gets the file text (if any).
        pub(super) fn level(&mut self, g: &Game<'_>, load: impl FnOnce(Option<String>) -> T) -> &mut T {
            let i = match self.index(g) {
                Some(i) => i,
                None => {
                    let dim = dim_key(g);
                    let text = read(g, &format!("{dim}.txt"));
                    self.levels.push(Level {
                        root: g.world_root.clone(),
                        dim,
                        state: load(text),
                        dirty: false,
                    });
                    self.levels.len() - 1
                }
            };
            &mut self.levels[i].state
        }
        /// Marks the current level's state for the next save.
        pub(super) fn dirty(&mut self, g: &Game<'_>) {
            if let Some(i) = self.index(g) {
                self.levels[i].dirty = true;
            }
        }
        /// Writes every dirty level of the current world (call from `save_game_metadata`).
        pub(super) fn save(&mut self, g: &Game<'_>, save: impl Fn(&T) -> String) {
            if dir(g).is_none() {
                return;
            }
            for l in self.levels.iter_mut().filter(|l| l.root == g.world_root && l.dirty) {
                if write(g, &format!("{}.txt", l.dim), &save(&l.state)) {
                    l.dirty = false;
                }
            }
        }
        /// Forgets everything (call when a world opens: `load_world_state`).
        pub(super) fn clear(&mut self) {
            self.levels.clear();
        }
    }
}

use horizon_client::items::ItemStack;
use horizon_client::mobs::{Mob, Who};
use horizon_core::geometry::{Aabb, Vec3};
use horizon_core::random::RandomSource;
use horizon_world::nbt::{JavaString, Tag};

const SHARD: &str = "soulshards:soul_shard";
const VILE_SWORD: &str = "soulshards:vile_sword";
const SOUL_STEALER: &str = "soulshards:soul_stealer";
/// `#soulshards:no_capture` of the jar (used when the tag is not loaded).
const NO_CAPTURE: [&str; 5] = [
    "minecraft:armor_stand",
    "minecraft:elder_guardian",
    "minecraft:ender_dragon",
    "minecraft:wither",
    "minecraft:player",
];
/// Forge 1.20.1 `#forge:bosses` (26.3 `#c:bosses`, used when the tag is not loaded).
const BOSSES: [&str; 2] = ["minecraft:ender_dragon", "minecraft:wither"];

thread_local! {
    static STATE: std::cell::RefCell<kit_state::PerWorld<engine::State>> =
        const { std::cell::RefCell::new(kit_state::PerWorld::new()) };
}

// ---- the shard's data (`binding` in minecraft:custom_data, like the 1.20.1 stack tag) ----

fn custom_data(stack: &ItemStack) -> Option<&Tag> {
    stack.data().and_then(|d| d.other("minecraft:custom_data"))
}

/// `Binding.fromNBT`: `custom_data.binding` = {bound: "ns:id", owner: [I; 4 ints], kills: int}.
fn binding_of(stack: &ItemStack) -> Option<engine::Binding> {
    let b = custom_data(stack)?.get("binding")?;
    let bound = match b.get("bound") {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    let owner = match b.get("owner") {
        Some(Tag::IntArray(v)) if v.len() == 4 => Some([v[0], v[1], v[2], v[3]]),
        _ => None,
    };
    let kills = match b.get("kills") {
        Some(Tag::Int(n)) => *n,
        Some(Tag::Short(n)) => i32::from(*n),
        Some(Tag::Byte(n)) => i32::from(*n),
        _ => 0,
    };
    Some(engine::Binding { bound, owner, kills })
}

/// `ItemSoulShard.updateBinding`, plus what the 26.3 item model reads: `custom_model_data.strings[0]` =
/// `tier_N` (the 1.20.1 `soulshards:bound` / `soulshards:tier` item properties) and the glint of a full shard
/// (`isFoil`).
fn set_binding(stack: &mut ItemStack, b: &engine::Binding) {
    let mut binding = Tag::compound([("kills", Tag::Int(b.kills))]);
    if let Tag::Compound(c) = &mut binding {
        if let Some(e) = &b.bound {
            c.insert(JavaString::from("bound"), Tag::String(e.as_str().into()));
        }
        if let Some(o) = b.owner {
            c.insert(JavaString::from("owner"), Tag::IntArray(o.to_vec()));
        }
    }
    let mut data = custom_data(stack).cloned().unwrap_or_else(|| Tag::compound(Vec::<(&str, Tag)>::new()));
    if let Tag::Compound(c) = &mut data {
        c.insert(JavaString::from("binding"), binding);
    }
    let d = stack.data_mut();
    d.set_other("minecraft:custom_data", Some(data));
    let model = engine::model_name(&Some(b.clone())).unwrap_or_default();
    d.set_other(
        "minecraft:custom_model_data",
        Some(Tag::compound([("strings", Tag::List(vec![Tag::String(model.as_str().into())]))])),
    );
    d.set_other(
        "minecraft:enchantment_glint_override",
        b.is_full().then_some(Tag::Byte(1)),
    );
}

fn view(stack: &Option<ItemStack>) -> Option<engine::ShardView> {
    let s = stack.as_ref()?;
    Some(engine::ShardView { is_shard: &*s.item == SHARD, binding: if &*s.item == SHARD { binding_of(s) } else { None } })
}

/// The cage's stack as the game's item NBT, in hex (forja's codec): every component kept.
fn encode_stack(s: &ItemStack) -> String {
    use horizon_world::nbt::{Limits, NamedTag, write_named};
    let root = NamedTag { name: JavaString::from(""), tag: horizon_client::save::item_to_nbt(s) };
    let bytes = write_named(&root, Limits::default()).unwrap_or_default();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
    }
    out
}
fn decode_stack(text: &str) -> Option<ItemStack> {
    use horizon_world::nbt::{Limits, read_named};
    let t = text.as_bytes();
    if t.len() % 2 != 0 {
        return None;
    }
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let bytes: Vec<u8> = t.chunks(2).map(|p| Some(hex(p[0])? << 4 | hex(p[1])?)).collect::<Option<_>>()?;
    let (named, _) = read_named(&bytes, Limits::default()).ok()?;
    horizon_client::save::item_from_nbt(&named.tag)
}

struct Live<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::World for Live<'_, '_> {
    fn block(&self, p: engine::Pos) -> Option<String> {
        kit_blocks::name_at(self.game, p).map(|n| n.to_string())
    }
}

impl Game<'_> {
    /// `SoulShardsAPI.isAllowed`: a known entity type outside `#soulshards:no_capture`.
    fn soulshards_allowed(&self, entity: &str) -> bool {
        let known = self.res.mob_types().contains_key(entity) || self.entity_type_exists(entity);
        let denied = match self.res.stations().enchantments.entity_tags.get("soulshards:no_capture") {
            Some(t) => t.contains(entity),
            None => NO_CAPTURE.contains(&entity),
        };
        known && !denied
    }
    fn soulshards_boss(&self, entity: &str) -> bool {
        match self.res.stations().enchantments.entity_tags.get("c:bosses") {
            Some(t) => t.contains(entity),
            None => BOSSES.contains(&entity),
        }
    }
    /// `getPersistentData().getBoolean("cageBorn")`: Forge keeps it in the entity's `ForgeData`; here it lives in
    /// the mob's saved NBT (`persist.raw`), so it survives a save.
    fn soulshards_cage_born(&self, id: u32) -> bool {
        matches!(
            self.persist.raw.get(&id).and_then(|t| t.get("ForgeData")).and_then(|f| f.get("cageBorn")),
            Some(Tag::Byte(b)) if *b != 0
        )
    }
    fn soulshards_mark_cage_born(&mut self, id: u32) {
        let raw = self.persist.raw.entry(id).or_insert_with(|| Tag::compound(Vec::<(&str, Tag)>::new()));
        if let Tag::Compound(c) = raw {
            let forge = c.entry(JavaString::from("ForgeData")).or_insert_with(|| Tag::compound(Vec::<(&str, Tag)>::new()));
            if let Tag::Compound(f) = forge {
                f.insert(JavaString::from("cageBorn"), Tag::Byte(1));
            }
        }
    }

    /// HEAD `creatures::Game::mob_died` (`LivingDeathEvent` + `LivingExperienceDropEvent`).
    pub(super) fn soulshards_mob_died(&mut self, m: &Mob) {
        if self.remote.is_some() {
            return;
        }
        let cage_born = self.soulshards_cage_born(m.id);
        // dropExperience = false: a cage-born mob drops no experience (`dropExperience` cancels the event).
        let no_xp = cage_born && !engine::config::BALANCE_DROP_EXPERIENCE;
        horizon_client::soulshards::NO_XP.store(if no_xp { m.id } else { u32::MAX }, std::sync::atomic::Ordering::Relaxed);
        let entity = m.kind.id.clone();
        let killer = self.last_hurt_by_player.get(&m.id).copied();
        let kill = engine::Kill {
            entity: entity.clone(),
            // `event.getSource().getEntity() instanceof Player`: the killing blow came from a player.
            by_player: killer.is_some() && matches!(m.last_damage, Some((d, _)) if d.attacker == Some(Who::Player)),
            allowed: self.soulshards_allowed(&entity),
            boss: self.soulshards_boss(&entity),
            cage_born,
        };
        if !engine::kill_counts(&kill) {
            return;
        }
        // The killer's inventory: a guest's own when the death comes outside their turn (as `credit_kill`).
        let guest = killer.flatten().filter(|_| self.lan_acting.is_none());
        match guest {
            Some(conn) => {
                let Some(mut host) = self.lan.take() else { return };
                let _ = self.as_guest(&mut host, conn, |g| g.soulshards_take_soul(&entity));
                self.lan = Some(host);
            }
            None => self.soulshards_take_soul(&entity),
        }
    }

    /// The shard part of `onEntityKill`, for the player whose inventory is current.
    fn soulshards_take_soul(&mut self, entity: &str) {
        let hotbar: [Option<engine::ShardView>; 9] = std::array::from_fn(|i| view(&self.inventory[i]));
        let Some(slot) = engine::first_shard(&view(&self.offhand), &hotbar, entity) else {
            return;
        };
        let main = self.inventory[self.selected].as_ref();
        let souls = engine::souls_for_kill(
            main.map_or(0, |s| s.enchantment_level(SOUL_STEALER)),
            main.is_some_and(|s| &*s.item == VILE_SWORD),
        );
        let owner = self.acting_player_uuid();
        let cell = match slot {
            engine::ShardSlot::OffHand => &mut self.offhand,
            engine::ShardSlot::Hotbar(i) => &mut self.inventory[i],
        };
        let Some(stack) = cell.as_mut() else { return };
        let old = binding_of(stack);
        // A stack of blank shards: one is peeled off, bound and given back (`shardItem.split(1)`).
        let mut peeled = None;
        if old.is_none() && stack.count > 1 {
            stack.count -= 1;
            let mut one = stack.clone();
            one.count = 1;
            peeled = Some(one);
        }
        let b = engine::bind_kill(old, entity, owner, souls);
        match peeled {
            Some(mut one) => {
                set_binding(&mut one, &b);
                self.give_or_drop(one);
            }
            None => set_binding(stack, &b),
        }
    }

    /// HEAD `copper_golems::Game::use_golem_statue`: the multiblock (`onBlockInteract`), the shard on a spawner or
    /// a cage (`ItemSoulShard.onItemUseFirst`) and the shard taken out of a cage (`BlockSoulCage.use`).
    pub(super) fn soulshards_use(&mut self, pos: [i32; 3]) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let Some(block) = kit_blocks::name_at(self, pos).map(|n| n.to_string()) else {
            return false;
        };
        let held = self.held().cloned();
        // The Soul Shard altar: a diamond on the glowstone of the 3x3 pattern.
        if held.as_ref().is_some_and(|h| &*h.item == engine::CATALYST) && engine::is_origin(&block) {
            let Some(blocks) = engine::multiblock_match(&Live { game: self }, pos) else {
                return false;
            };
            for p in blocks {
                let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
                self.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, false));
            }
            let _ = self.process_level_events();
            // `held.shrink(1)` (also in creative, as the event does).
            let cell = self.held_mut();
            if let Some(s) = cell.as_mut() {
                s.count -= 1;
                if s.count == 0 {
                    *cell = None;
                }
            }
            self.give_or_drop(ItemStack::new(SHARD, 1));
            return true;
        }
        if let Some(h) = held.as_ref().filter(|h| &*h.item == SHARD) {
            let Some(binding) = binding_of(h) else {
                return false;
            };
            if block == "minecraft:spawner" {
                return self.soulshards_absorb(pos, binding);
            }
            if engine::is_cage(&block) {
                if binding.bound.is_none() {
                    return true; // InteractionResult.FAIL: nothing else happens
                }
                let empty = STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.level(self, engine::State::load).cages.get(&pos).is_none_or(|c| c.shard.is_none())
                });
                if !empty {
                    return false;
                }
                let hex = encode_stack(h);
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    let cage = s.level(self, engine::State::load).cages.entry(pos).or_default();
                    cage.shard = Some(hex);
                    s.dirty(self);
                });
                // `context.getPlayer().setItemInHand(hand, EMPTY)` and `cage.setState(true)`.
                *self.held_mut() = None;
                self.soulshards_set_active(pos, true);
                return true;
            }
            return false;
        }
        // Sneaking with empty hands on a cage: the shard comes out (`extractItem(0, 1)`).
        if engine::is_cage(&block) && held.is_none() && self.offhand.is_none() && kit_blocks::sneaking(self) {
            let taken = STATE.with(|s| {
                let mut s = s.borrow_mut();
                let cages = &mut s.level(self, engine::State::load).cages;
                let hex = cages.get_mut(&pos).and_then(|c| c.shard.take());
                if hex.is_some() {
                    s.dirty(self);
                }
                hex
            });
            let Some(hex) = taken else {
                return false;
            };
            let Some(mut stack) = decode_stack(&hex) else {
                return false;
            };
            if stack.count > 1 {
                // A stack would stay in the slot minus one (bound shards stack to 1, so this is a safety net).
                let mut rest = stack.clone();
                rest.count -= 1;
                stack.count = 1;
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.level(self, engine::State::load).cages.entry(pos).or_default().shard = Some(encode_stack(&rest));
                    s.dirty(self);
                });
            }
            self.give_or_drop(stack);
            return true;
        }
        false
    }

    /// The shard on a mob spawner of its entity: +200 souls (the pack's `absorptionBonus`), the spawner goes.
    fn soulshards_absorb(&mut self, pos: [i32; 3], mut binding: engine::Binding) -> bool {
        if !engine::config::BALANCE_ALLOW_SPAWNER_ABSORPTION || binding.is_full() {
            return false;
        }
        let shown = match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(horizon_client::containers::BlockEntity::Spawner(sp)) => sp.next_data().and_then(|d| d.id()),
            _ => return true, // InteractionResult.FAIL
        };
        let Some(bound) = binding.bound.clone() else {
            return true;
        };
        if !self.soulshards_allowed(&bound) {
            return false;
        }
        if shown.as_deref() != Some(bound.as_str()) {
            return true; // FAIL
        }
        binding.add_kills(engine::config::BALANCE_ABSORPTION_BONUS);
        if let Some(stack) = self.held_mut().as_mut() {
            set_binding(stack, &binding);
        }
        let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, false));
        let _ = self.process_level_events();
        true
    }

    /// `TileEntitySoulCage.setState`: the block becomes the active or idle cage (`setBlock(.., 3)`).
    fn soulshards_set_active(&mut self, pos: [i32; 3], active: bool) {
        let name = if active { engine::SOUL_CAGE_ACTIVE } else { engine::SOUL_CAGE };
        if kit_blocks::name_at(self, pos).as_deref() == Some(name) {
            return;
        }
        let Some(state) = self.pack.assets.registry.default_state(name) else {
            return;
        };
        let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, state, 3));
        let _ = self.process_level_events();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(c) = s.level(self, engine::State::load).cages.get_mut(&pos) {
                c.active = active;
            }
            s.dirty(self);
        });
    }

    /// HEAD `block_entity_ticks::Game::tick_block_entities`: every cage of this dimension, like its block entity
    /// ticker. A cage whose block is gone drops its shard (`BlockSoulCage.onRemove`), on any removal.
    pub(super) fn soulshards_tick(&mut self) {
        if self.remote.is_some() || kit_state::dir(self).is_none() {
            return;
        }
        let cages: Vec<(engine::Pos, engine::Cage)> = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.level(self, engine::State::load).cages.iter().map(|(p, c)| (*p, c.clone())).collect()
        });
        for (p, mut cage) in cages {
            let Some(name) = kit_blocks::name_at(self, p).map(|n| n.to_string()) else {
                continue; // not loaded: nothing changes there
            };
            if !engine::is_cage(&name) {
                if let Some(stack) = cage.shard.as_deref().and_then(decode_stack) {
                    let at = [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5];
                    self.spawn_item(stack, at, super::dispenser::new_item_motion(), 10);
                }
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.level(self, engine::State::load).cages.remove(&p);
                    s.dirty(self);
                });
                continue;
            }
            if !self.tickets.levels().block_ticking_at(p[0], p[2]) {
                continue;
            }
            let binding = cage
                .shard
                .as_deref()
                .and_then(decode_stack)
                .filter(|s| &*s.item == SHARD)
                .and_then(|s| binding_of(&s))
                .filter(|b| b.bound.is_some());
            let check = engine::CageCheck {
                allowed: binding.as_ref().and_then(|b| b.bound.as_deref()).is_some_and(|e| self.soulshards_allowed(e)),
                powered: {
                    let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
                    self.with_level(|b, level, ticks| b.has_neighbor_signal(level, ticks, at))
                },
                player_near: {
                    let (x, y, z) = (f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
                    self.level_players().iter().any(|pl| {
                        !pl.spectator
                            && (pl.position.x - x).powi(2) + (pl.position.y - y).powi(2) + (pl.position.z - z).powi(2)
                                <= 16.0 * 16.0
                    })
                },
                owner_online: true,
                binding: binding.clone(),
            };
            let was_active = name == engine::SOUL_CAGE_ACTIVE;
            cage.active = was_active || cage.active;
            let t = engine::cage_tick(&mut cage, &check);
            let keep = cage.shard.is_some() || cage.active;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                let cages = &mut s.level(self, engine::State::load).cages;
                if keep {
                    cages.insert(p, cage.clone());
                } else {
                    cages.remove(&p);
                }
                s.dirty(self);
            });
            if let Some(active) = t.set_active {
                self.soulshards_set_active(p, active);
            }
            if t.spawn
                && let Some(b) = binding
            {
                self.soulshards_spawn(p, &b);
            }
        }
    }

    /// `TileEntitySoulCage.spawnEntities` (1.3.6): `spawnAmount` mobs, up to 5 tries each, around the cage.
    fn soulshards_spawn(&mut self, cage: engine::Pos, b: &engine::Binding) {
        let Some(kind) = b.bound.clone() else { return };
        let Some(t) = self.res.mob_types().get(&kind).cloned() else {
            return; // ForgeRegistries.ENTITY_TYPES.getValue == null
        };
        let tier = b.tier();
        let block_light = self.world.light(cage[0], cage[1], cage[2]) & 15;
        for _ in 0..tier.spawn_amount {
            for _attempt in 0..5 {
                let r = &mut self.world.random;
                let (d1, d2) = (r.next_double(), r.next_double());
                let n = r.next_int_bound(3).unwrap_or(0);
                let (d3, d4) = (r.next_double(), r.next_double());
                let p = engine::spawn_pos(cage, d1, d2, n, d3, d4);
                if tier.check_light && !engine::light_ok(block_light) {
                    continue;
                }
                let at = [f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 0.5];
                let half = t.width / 2.0;
                let bounds = Aabb::new(
                    Vec3::new(at[0] - half, at[1], at[2] - half),
                    Vec3::new(at[0] + half, at[1] + t.height, at[2] + half),
                );
                if self.soulshards_cap_reached(cage, &kind) {
                    continue;
                }
                // `level.isUnobstructed(entity)`: no living entity (mob or player) in its box.
                let blocked = self.mobs.iter().any(|m| !m.removed && m.bounds().intersects(bounds))
                    || self.level_players().iter().any(|pl| pl.alive && !pl.spectator && pl.bounds.intersects(bounds));
                if blocked {
                    continue;
                }
                if !engine::config::BALANCE_ALLOW_BOSS_SPAWNS && BOSSES.contains(&kind.as_str()) {
                    continue; // !canChangeDimensions
                }
                if let Some(id) = self.spawn_mob(&kind, at, false) {
                    self.soulshards_mark_cage_born(id);
                }
                break;
            }
        }
    }

    /// `hasReachedSpawnCap`: cage-born mobs of the same type within 16 blocks of the cage, at the pack's cap (32).
    fn soulshards_cap_reached(&self, cage: engine::Pos, kind: &str) -> bool {
        let lo = Vec3::new(f64::from(cage[0] - 16), f64::from(cage[1] - 16), f64::from(cage[2] - 16));
        let hi = Vec3::new(f64::from(cage[0] + 16), f64::from(cage[1] + 16), f64::from(cage[2] + 16));
        let area = Aabb::new(lo, hi);
        let n = self
            .mobs
            .iter()
            .filter(|m| !m.removed && m.kind.id == kind && m.bounds().intersects(area) && self.soulshards_cage_born(m.id))
            .count();
        n >= engine::config::BALANCE_SPAWN_CAP
    }

    /// RETURN `stations::Game::stack_tooltip` (`ItemSoulShard.appendHoverText`): the lines after the name.
    pub(super) fn soulshards_tooltip(
        &self,
        stack: &ItemStack,
        mut lines: Vec<(horizon_client::text::Text, u32)>,
    ) -> Vec<(horizon_client::text::Text, u32)> {
        if &*stack.item != SHARD {
            return lines;
        }
        let Some(b) = binding_of(stack) else {
            return lines;
        };
        let exists = b.bound.as_deref().is_some_and(|e| self.res.mob_types().contains_key(e) || self.entity_type_exists(e));
        let advanced = self.settings.file.bool("advancedItemTooltips");
        let at = lines.len().min(1);
        for (i, (key, arg)) in engine::tooltip(&b, exists, advanced, engine::uuid_text).into_iter().enumerate() {
            let pattern = self.pack.names.display_get(key).unwrap_or_else(|| key.to_owned());
            let text = engine::fill(&pattern, &[arg]);
            lines.insert(at + i, (horizon_client::text::Text::legacy(&text), u32::MAX));
        }
        lines
    }

    /// HEAD `save_game_metadata`: the cages are saved with the world.
    pub(super) fn soulshards_save(&mut self) {
        let game: &Game<'_> = self;
        STATE.with(|s| s.borrow_mut().save(game, engine::State::save));
    }
    /// HEAD `load_world_state` (no bindings): a world opens; its cages are read again on first use.
    pub(super) fn soulshards_world_opened() {
        STATE.with(|s| s.borrow_mut().clear());
    }
}
