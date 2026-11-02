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
