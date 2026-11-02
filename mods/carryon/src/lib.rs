// Carry On for Horizon: the rules, without any game code. Written from the public source of the mod (Tschipp/CarryOn, branch 1.20,
// licence GNU LGPLv3, v2.1.4; the jar the pack plays is carryon-forge-1.20.1-2.1.2.7): `PickupHandler`, `PlacementHandler`, `ListHandler`,
// `CarryOnCommon`, `CarryOnData`. the pack's values come from carryon-common.toml / carryon-client.toml.
// This file is pasted into the game as `crate::game::carryon::engine` (tools/hnmod.py bundle) and is also the
// `horizon_carryon` crate the tests use: plain Rust, no game types, no inner attributes (`#![...]`).
pub type Pos = [i32; 3];

/// Valorile pachetului (profilul the 1.20.1 pack/config), generate de tools/nou_mod.py.
/// Păstrează doar ce folosește portul; scrie sursa fiecărei valori folosite.
#[allow(dead_code)]
pub mod config {
    // ---- carryon-client.toml
    /// If the front of the Tile Entities should face the player or should face outward
    pub const FACE_PLAYER: bool = false; // carryon-client.toml:2 [] facePlayer
    /// Arms should render on sides when carrying. Set to false if you experience issues with mods that replace the player model (like MoBends, etc)
    pub const RENDER_ARMS: bool = true; // carryon-client.toml:4 [] renderArms
    /// Model Overrides based on NBT or Meta. Advanced users only! Read about the format here: https://github.com/Tschipp/CarryOn/wiki/Model-Override-Config
    pub const MODEL_OVERRIDES: &[&str] = &["minecraft:redstone_wire->(item)minecraft:redstone", "minecraft:bamboo_sapling->(block)minecraft:bamboo", "minecraft:candle_cake->(block)minecraft:cake"]; // carryon-client.toml:6 [] modelOverrides
    // ---- carryon-common.toml
    /// General Settings Maximum distance from where Blocks and Entities can be picked up Range: 0.0 ~ 1.7976931348623157E308
    pub const SETTINGS_MAX_DISTANCE: f64 = 2.5; // carryon-common.toml:6 [settings] maxDistance
    /// Max width of entities that can be picked up in survival mode Range: 0.0 ~ 10.0
    pub const SETTINGS_MAX_ENTITY_WIDTH: f64 = 1.5; // carryon-common.toml:9 [settings] maxEntityWidth
    /// Max height of entities that can be picked up in survival mode Range: 0.0 ~ 10.0
    pub const SETTINGS_MAX_ENTITY_HEIGHT: f64 = 2.5; // carryon-common.toml:12 [settings] maxEntityHeight
    /// Slowness multiplier for blocks Range: 0.0 ~ 1.7976931348623157E308
    pub const SETTINGS_BLOCK_SLOWNESS_MULTIPLIER: f64 = 1.0; // carryon-common.toml:15 [settings] blockSlownessMultiplier
    /// Slowness multiplier for entities Range: 0.0 ~ 1.7976931348623157E308
    pub const SETTINGS_ENTITY_SLOWNESS_MULTIPLIER: f64 = 1.0; // carryon-common.toml:18 [settings] entitySlownessMultiplier
    /// Maximum stack limit for entities Range: > 1
    pub const SETTINGS_MAX_ENTITY_STACK_LIMIT: i64 = 10; // carryon-common.toml:21 [settings] maxEntityStackLimit
    /// More complex Tile Entities slow down the player more
    pub const SETTINGS_HEAVY_TILES: bool = true; // carryon-common.toml:23 [settings] heavyTiles
    /// Allow all blocks to be picked up, not just Tile Entites. White/Blacklist will still be respected.
    pub const SETTINGS_PICKUP_ALL_BLOCKS: bool = false; // carryon-common.toml:25 [settings] pickupAllBlocks
    /// Whether Blocks and Entities slow the creative player down when carried
    pub const SETTINGS_SLOWNESS_IN_CREATIVE: bool = true; // carryon-common.toml:27 [settings] slownessInCreative
    /// Whether hostile mobs should be able to picked up in survival mode
    pub const SETTINGS_PICKUP_HOSTILE_MOBS: bool = false; // carryon-common.toml:29 [settings] pickupHostileMobs
    /// Larger Entities slow down the player more
    pub const SETTINGS_HEAVY_ENTITIES: bool = true; // carryon-common.toml:31 [settings] heavyEntities
    /// Allow babies to be carried even when adult mob is blacklisted (or not whitelisted)
    pub const SETTINGS_ALLOW_BABIES: bool = false; // carryon-common.toml:33 [settings] allowBabies
    /// Use Whitelist instead of Blacklist for Blocks
    pub const SETTINGS_USE_WHITELIST_BLOCKS: bool = false; // carryon-common.toml:35 [settings] useWhitelistBlocks
    /// Use Whitelist instead of Blacklist for Entities
    pub const SETTINGS_USE_WHITELIST_ENTITIES: bool = false; // carryon-common.toml:37 [settings] useWhitelistEntities
    /// Use Whitelist instead of Blacklist for Stacking
    pub const SETTINGS_USE_WHITELIST_STACKING: bool = false; // carryon-common.toml:39 [settings] useWhitelistStacking
    /// Whether the player can hit blocks and entities while carrying or not
    pub const SETTINGS_HIT_WHILE_CARRYING: bool = false; // carryon-common.toml:41 [settings] hitWhileCarrying
    /// Whether the player drops the carried object when hit or not
    pub const SETTINGS_DROP_CARRIED_WHEN_HIT: bool = false; // carryon-common.toml:43 [settings] dropCarriedWhenHit
    /// Use custom Pickup Scripts. Having this set to false, will not allow you to run scripts, but will increase your performance
    pub const SETTINGS_USE_SCRIPTS: bool = false; // carryon-common.toml:45 [settings] useScripts
    /// Allows entities to be stacked on top of each other
    pub const SETTINGS_STACKABLE_ENTITIES: bool = true; // carryon-common.toml:47 [settings] stackableEntities
    /// Whether entities' size matters when stacking or not. This means that larger entities cannot be stacked on smaller ones
    pub const SETTINGS_ENTITY_SIZE_MATTERS_STACKING: bool = true; // carryon-common.toml:49 [settings] entitySizeMattersStacking
    /// Usually all the block state information is retained when placing a block that was picked up. But some information is changed to a modified property, like rotation or orientation. I
    // SETTINGS_PLACEMENT_STATE_EXCEPTIONS = ["minecraft:chest[type]", "minecraft:stone_button[face]", "minecraft:vine[north,east,south,west,up]", "minecraft:creeper  (carryon-common.toml:51 [settings] placementStateExceptions: valoare complexă, de tradus de mână)
    /// Whether Players can be picked up. Creative players can't be picked up in Survival Mode
    pub const SETTINGS_PICKUP_PLAYERS: bool = true; // carryon-common.toml:53 [settings] pickupPlayers
    /// Whether players in Survival Mode can pick up unbreakable blocks. Creative players always can.
    pub const SETTINGS_PICKUP_UNBREAKABLE_BLOCKS: bool = false; // carryon-common.toml:55 [settings] pickupUnbreakableBlocks
    /// Whitelist. Read about the format here: https://github.com/Tschipp/CarryOn/wiki/Black---and-Whitelist-Config Entities that CAN be picked up (useWhitelistEntities must be true)
    pub const WHITELIST_ALLOWED_ENTITIES: &[&str] = &[]; // carryon-common.toml:60 [whitelist] allowedEntities
    /// Blocks that CAN be picked up (useWhitelistBlocks must be true)
    pub const WHITELIST_ALLOWED_BLOCKS: &[&str] = &[]; // carryon-common.toml:62 [whitelist] allowedBlocks
    /// Entities that CAN have other entities stacked on top of them (useWhitelistStacking must be true)
    pub const WHITELIST_ALLOWED_STACKING: &[&str] = &[]; // carryon-common.toml:64 [whitelist] allowedStacking
    /// Blacklist. Read about the format here: https://github.com/Tschipp/CarryOn/wiki/Black---and-Whitelist-Config Blocks that cannot be picked up
    pub const BLACKLIST_FORBIDDEN_TILES: &[&str] = &["#forge:immovable", "#forge:relocation_not_supported", "minecraft:end_portal", "minecraft:piston_head", "minecraft:end_gateway", "minecraft:tall_grass", "minecraft:large_fern", "minecraft:peony", "minecraft:rose_bush", "minecraft:lilac", "minecraft:sunflower", "minecraft:*_bed", "minecraft:*_door", "minecraft:big_dripleaf_stem", "minecraft:waterlily", "minecraft:cake", "minecraft:nether_portal", "minecraft:tall_seagrass", "animania:block_trough", "animania:block_invisiblock", "colossalchests:*", "ic2:*", "bigreactors:*", "forestry:*", "tconstruct:*", "rustic:*", "botania:*", "astralsorcery:*", "quark:colored_bed_*", "immersiveengineering:*", "embers:block_furnace", "embers:ember_bore", "embers:ember_activator", "embers:mixer", "embers:heat_coil", "embers:large_tank", "embers:crystal_cell", "embers:alchemy_pedestal", "embers:boiler", "embers:combustor", "embers:catalzyer", "embers:field_chart", "embers:inferno_forge", "storagedrawers:framingtable", "skyresources:*", "lootbags:*", "exsartagine:*", "aquamunda:tank", "opencomputers:*", "malisisdoors:*", "industrialforegoing:*", "minecolonies:*", "thaumcraft:pillar*", "thaumcraft:infernal_furnace", "thaumcraft:placeholder*", "thaumcraft:infusion_matrix", "thaumcraft:golem_builder", "thaumcraft:thaumatorium*", "magneticraft:oil_heater", "magneticraft:solar_panel", "magneticraft:steam_engine", "magneticraft:shelving_unit", "magneticraft:grinder", "magneticraft:sieve", "magneticraft:solar_tower", "magneticraft:solar_mirror", "magneticraft:container", "magneticraft:pumpjack", "magneticraft:solar_panel", "magneticraft:refinery", "magneticraft:oil_heater", "magneticraft:hydraulic_press", "magneticraft:multiblock_gap", "refinedstorage:*", "mcmultipart:*", "enderstorage:*", "betterstorage:*", "practicallogistics2:*", "wearablebackpacks:*", "rftools:screen", "rftools:creative_screen", "create:*", "magic_doorknob:*", "iceandfire:*", "ftbquests:*", "waystones:*", "contact:*", "framedblocks:*", "securitycraft:*", "forgemultipartcbe:*", "integrateddynamics:cable", "mekanismgenerators:wind_generator", "cookingforblockheads:cabinet", "cookingforblockheads:corner", "cookingforblockheads:counter", "cookingforblockheads:oven", "cookingforblockheads:toaster", "cookingforblockheads:milk_jar", "cookingforblockheads:cow_jar", "cookingforblockheads:fruit_basket", "cookingforblockheads:cooking_table", "cookingforblockheads:fridge", "cookingforblockheads:sink", "powah:*", "advancementtrophies:trophy", "mekanismgenerators:heat_generator", "mna:filler_block"]; // carryon-common.toml:69 [blacklist] forbiddenTiles
    /// Entities that cannot be picked up
    pub const BLACKLIST_FORBIDDEN_ENTITIES: &[&str] = &["minecraft:end_crystal", "minecraft:ender_dragon", "minecraft:ghast", "minecraft:shulker", "minecraft:leash_knot", "minecraft:armor_stand", "minecraft:item_frame", "minecraft:painting", "minecraft:shulker_bullet", "animania:hamster", "animania:ferret*", "animania:hedgehog*", "animania:cart", "animania:wagon", "mynko:*", "pixelmon:*", "mocreatures:*", "quark:totem", "vehicle:*", "securitycraft:*", "taterzens:npc", "easy_npc:*", "bodiesbodies:dead_body"]; // carryon-common.toml:71 [blacklist] forbiddenEntities
    /// Entities that cannot have other entities stacked on top of them
    pub const BLACKLIST_FORBIDDEN_STACKING: &[&str] = &["minecraft:horse"]; // carryon-common.toml:73 [blacklist] forbiddenStacking
    /// Custom Pickup Conditions. Read about the format here: https://github.com/Tschipp/CarryOn/wiki/Custom-Pickup-Condition-Config Custom Pickup Conditions for Blocks
    pub const CUSTOM_PICKUP_CONDITIONS_CUSTOM_PICKUP_CONDITIONS_BLOCKS: &[&str] = &[]; // carryon-common.toml:78 [customPickupConditions] customPickupConditionsBlocks
    /// Custom Pickup Conditions for Entities
    pub const CUSTOM_PICKUP_CONDITIONS_CUSTOM_PICKUP_CONDITIONS_ENTITIES: &[&str] = &[]; // carryon-common.toml:80 [customPickupConditions] customPickupConditionsEntities
}


// ------------------------------------------------------------------------------------------------
// Lists (ListHandler, StringHelper.matchesWildcards)
// ------------------------------------------------------------------------------------------------

/// `ListHandler.addWithWildcards` + `doCheck`: an entry with `*` is cut at the stars and the name must contain the
/// pieces in order (`StringHelper.matchesWildcards`; `split(",")` drops empty pieces at the end); an entry without `*` is
/// the exact name; `#tag` entries are tags (looked up by `in_tag`).
pub fn list_matches(entries: &[&str], name: &str, in_tag: &dyn Fn(&str) -> bool) -> bool {
    entries.iter().any(|e| {
        if let Some(tag) = e.strip_prefix('#') {
            in_tag(tag)
        } else if e.contains('*') {
            let mut pieces: Vec<&str> = e.split('*').collect();
            while pieces.last().is_some_and(|p| p.is_empty()) {
                pieces.pop();
            }
            wildcards(name, &pieces)
        } else {
            *e == name
        }
    })
}
fn wildcards(name: &str, pieces: &[&str]) -> bool {
    let mut rest = name;
    for p in pieces {
        match rest.find(p) {
            Some(i) => rest = &rest[i + p.len()..],
            None => return false,
        }
    }
    true
}

/// `ListHandler.isPermitted(block)`.
pub fn block_permitted(name: &str, in_tag: &dyn Fn(&str) -> bool) -> bool {
    if config::SETTINGS_USE_WHITELIST_BLOCKS {
        list_matches(config::WHITELIST_ALLOWED_BLOCKS, name, in_tag)
    } else {
        !list_matches(config::BLACKLIST_FORBIDDEN_TILES, name, in_tag)
    }
}
/// `ListHandler.isPermitted(entity)`.
pub fn entity_permitted(kind: &str, in_tag: &dyn Fn(&str) -> bool) -> bool {
    if config::SETTINGS_USE_WHITELIST_ENTITIES {
        list_matches(config::WHITELIST_ALLOWED_ENTITIES, kind, in_tag)
    } else {
        !list_matches(config::BLACKLIST_FORBIDDEN_ENTITIES, kind, in_tag)
    }
}
/// `ListHandler.isStackingPermitted(entity)`.
pub fn stacking_permitted(kind: &str, in_tag: &dyn Fn(&str) -> bool) -> bool {
    if config::SETTINGS_USE_WHITELIST_STACKING {
        list_matches(config::WHITELIST_ALLOWED_STACKING, kind, in_tag)
    } else {
        !list_matches(config::BLACKLIST_FORBIDDEN_STACKING, kind, in_tag)
    }
}

/// `settings.placementStateExceptions`: (block name, property names) pairs, "modid:block[prop,prop]".
pub fn property_exceptions() -> Vec<(String, Vec<String>)> {
    parse_exceptions(PLACEMENT_STATE_EXCEPTIONS)
}
/// `carryon-common.toml:51 [settings] placementStateExceptions`.
pub const PLACEMENT_STATE_EXCEPTIONS: &[&str] = &[
    "minecraft:chest[type]",
    "minecraft:stone_button[face]",
    "minecraft:vine[north,east,south,west,up]",
    "minecraft:creeper_head[rotation]",
    "minecraft:glow_lichen[north,east,south,west,up,down]",
    "minecraft:oak_sign[rotation]",
    "minecraft:oak_trapdoor[half]",
];
pub fn parse_exceptions(list: &[&str]) -> Vec<(String, Vec<String>)> {
    list.iter()
        .filter_map(|s| {
            let (a, b) = (s.find('[')?, s.find(']')?);
            if b <= a {
                return None;
            }
            Some((s[..a].to_owned(), s[a + 1..b].split(',').map(str::to_owned).collect()))
        })
        .collect()
}
/// `ListHandler.isPropertyException(prop)`: the original keeps the `Property` objects named by the list, and a property
/// object is shared by every block that uses it (`ROTATION_16` by all signs, heads and banners, `CHEST_TYPE` by the
/// chests, `HALF` by stairs and trapdoors, `ATTACH_FACE` by buttons, levers and grindstones, `NORTH`... by vines, fences,
/// panes...). The registry does not name the property objects, so a property is an exception when its name is listed and
/// its value is of the kind of that property.
pub fn is_property_exception(prop: &str, value: &str, exceptions: &[(String, Vec<String>)]) -> bool {
    if !exceptions.iter().any(|(_, props)| props.iter().any(|p| p == prop)) {
        return false;
    }
    match prop {
        "type" => matches!(value, "single" | "left" | "right"),
        "face" => matches!(value, "floor" | "wall" | "ceiling"),
        "half" => matches!(value, "top" | "bottom"),
        "rotation" => value.parse::<u8>().is_ok(),
        "north" | "east" | "south" | "west" | "up" | "down" => matches!(value, "true" | "false"),
        _ => true,
    }
}

// ------------------------------------------------------------------------------------------------
// Pickup (PickupHandler)
// ------------------------------------------------------------------------------------------------

/// What the pickup of a block needs to know about it.
#[derive(Clone, Debug, Default)]
pub struct BlockFacts {
    pub name: String,
    /// The block has a block entity (`level.getBlockEntity(pos) != null`).
    pub has_block_entity: bool,
    /// `state.getDestroySpeed(level, pos) == -1`.
    pub unbreakable: bool,
    /// The block entity's `Lock` tag is not empty.
    pub locked: bool,
}
/// Why a pickup did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deny {
    Hands,
    Distance,
    AlreadyCarrying,
    Key,
    SameTick,
    GameMode,
    Forbidden,
    Unbreakable,
    NotATile,
    Locked,
    Hostile,
    TooBig,
    OtherOwner,
    Invulnerable,
    Removed,
    Players,
}

/// What `PickupHandler.canCarryGeneral` reads.
#[derive(Clone, Debug)]
pub struct Carrier {
    pub hands_empty: bool,
    pub distance: f64,
    pub carrying: bool,
    pub key_pressed: bool,
    /// `player.tickCount == carry.getTick()`: picked up or placed in this very tick.
    pub same_tick: bool,
    /// Adventure and spectator cannot carry.
    pub game_mode_allows: bool,
    pub creative: bool,
}
/// `PickupHandler.canCarryGeneral`.
pub fn can_carry_general(c: &Carrier) -> Result<(), Deny> {
    if !c.hands_empty {
        return Err(Deny::Hands);
    }
    if c.distance > config::SETTINGS_MAX_DISTANCE {
        return Err(Deny::Distance);
    }
    if c.carrying {
        return Err(Deny::AlreadyCarrying);
    }
    if !c.key_pressed {
        return Err(Deny::Key);
    }
    if c.same_tick {
        return Err(Deny::SameTick);
    }
    if !c.game_mode_allows {
        return Err(Deny::GameMode);
    }
    Ok(())
}

/// `PickupHandler.tryPickUpBlock`'s checks (after `canCarryGeneral`).
pub fn can_pick_block(f: &BlockFacts, creative: bool, in_tag: &dyn Fn(&str) -> bool) -> Result<(), Deny> {
    if !block_permitted(&f.name, in_tag) {
        return Err(Deny::Forbidden);
    }
    if f.unbreakable && !creative && !config::SETTINGS_PICKUP_UNBREAKABLE_BLOCKS {
        return Err(Deny::Unbreakable);
    }
    if !f.has_block_entity && !config::SETTINGS_PICKUP_ALL_BLOCKS {
        return Err(Deny::NotATile);
    }
    if f.locked {
        return Err(Deny::Locked);
    }
    Ok(())
}

/// What the pickup of an entity needs to know about it.
#[derive(Clone, Debug, Default)]
pub struct EntityFacts {
    pub kind: String,
    pub is_monster: bool,
    pub width: f64,
    pub height: f64,
    /// `entity.invulnerableTime != 0`.
    pub invulnerable_time: bool,
    pub removed: bool,
    /// A tamed animal whose owner is somebody else.
    pub owned_by_other: bool,
    /// An ageable mob that is a baby (`getAge() < 0 || isBaby()`).
    pub is_baby: bool,
    pub is_player: bool,
    pub other_player_creative: bool,
}
/// `PickupHandler.tryPickupEntity`'s checks (after `canCarryGeneral`).
pub fn can_pick_entity(f: &EntityFacts, creative: bool, in_tag: &dyn Fn(&str) -> bool) -> Result<(), Deny> {
    if f.invulnerable_time {
        return Err(Deny::Invulnerable);
    }
    if f.removed {
        return Err(Deny::Removed);
    }
    if f.owned_by_other {
        return Err(Deny::OtherOwner);
    }
    if !entity_permitted(&f.kind, in_tag) && !(config::SETTINGS_ALLOW_BABIES && f.is_baby) {
        return Err(Deny::Forbidden);
    }
    if !creative {
        if !config::SETTINGS_PICKUP_HOSTILE_MOBS && f.is_monster {
            return Err(Deny::Hostile);
        }
        if config::SETTINGS_MAX_ENTITY_HEIGHT < f.height || config::SETTINGS_MAX_ENTITY_WIDTH < f.width {
            return Err(Deny::TooBig);
        }
    }
    if f.is_player && (!config::SETTINGS_PICKUP_PLAYERS || (!creative && f.other_player_creative)) {
        return Err(Deny::Players);
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// Slowness (CarryOnCommon.potionLevel)
// ------------------------------------------------------------------------------------------------

/// What is carried, for the slowness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Load {
    Player,
    /// `entity.getBbHeight() * entity.getBbWidth()`.
    Entity { area: f64 },
    /// The length of the carried data's string form.
    Block { nbt_len: usize },
}
/// `CarryOnCommon.potionLevel`: the amplifier of the Slowness effect.
pub fn slowness_amplifier(load: Load) -> i32 {
    match load {
        Load::Player => 1,
        Load::Entity { area } => {
            let mut i = area as i32;
            if i > 4 {
                i = 4;
            }
            if !config::SETTINGS_HEAVY_ENTITIES {
                i = 1;
            }
            (f64::from(i) * config::SETTINGS_ENTITY_SLOWNESS_MULTIPLIER) as i32
        }
        Load::Block { nbt_len } => {
            let mut i = (nbt_len / 500) as i32;
            if i > 4 {
                i = 4;
            }
            if !config::SETTINGS_HEAVY_TILES {
                i = 1;
            }
            (f64::from(i) * config::SETTINGS_BLOCK_SLOWNESS_MULTIPLIER) as i32
        }
    }
}
/// `CarryOnCommon.onCarryTick`: whether the effect is applied this tick.
pub fn slows(creative: bool) -> bool {
    config::SETTINGS_SLOWNESS_IN_CREATIVE || !creative
}
/// `CarryOnCommon.onTryBreakBlock` / `onAttackedByPlayer`: whether the player may hit while carrying.
pub fn may_hit(carrying: bool) -> bool {
    config::SETTINGS_HIT_WHILE_CARRYING || !carrying
}

// ------------------------------------------------------------------------------------------------
// Placement (PlacementHandler)
// ------------------------------------------------------------------------------------------------

/// `PlacementHandler.getPlacementState`: the carried properties, with the ones of the state a block of that kind
/// would get if placed there (`placed`) for the directions, the axes and the listed exceptions; `waterlogged` too.
/// Properties are (name, value) pairs; the carried block's properties give the order.
pub fn merge_placement_state(carried: &[(String, String)], placed: &[(String, String)], exceptions: &[(String, Vec<String>)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = carried.to_vec();
    for (name, value) in placed {
        let take = name == "facing" || name == "axis" || name == "waterlogged" || is_property_exception(name, value, exceptions);
        if take && let Some(slot) = out.iter_mut().find(|(n, _)| n == name) {
            slot.1 = value.clone();
        }
    }
    out
}

/// `PlacementHandler.getDeathPlacementPos`: the positions within 15 of the player (as the original builds them: the
/// offsets 0, 1, -1, 2, -2... per axis up to 14), nearest to the player's center first (a stable sort).
pub fn death_candidates(p: Pos, center: [f64; 3]) -> Vec<Pos> {
    const DISTANCE: i32 = 15;
    let mut out = Vec::new();
    for j in 0..DISTANCE * 2 {
        for i in 0..DISTANCE * 2 {
            for k in 0..DISTANCE * 2 {
                let x = if i % 2 == 0 { i / 2 } else { -(i / 2) };
                let y = if j % 2 == 0 { j / 2 } else { -(j / 2) };
                let z = if k % 2 == 0 { k / 2 } else { -(k / 2) };
                out.push([p[0] + x, p[1] + y, p[2] + z]);
            }
        }
    }
    let d2 = |q: &Pos| {
        let c = [f64::from(q[0]) + 0.5, f64::from(q[1]) + 0.5, f64::from(q[2]) + 0.5];
        (0..3).map(|a| (c[a] - center[a]).powi(2)).sum::<f64>()
    };
    out.sort_by(|a, b| d2(a).total_cmp(&d2(b)));
    out
}

/// The result of asking whether a carried entity may be stacked on `clicked`'s pile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stack {
    /// `stackableEntities` is off, or nothing is carried.
    No,
    /// The pile is full (`maxEntityStackLimit`) or the top may not hold entities / is too small: the lava-pop sound.
    Refused,
    /// The carried entity rides the top of the pile; `near` is `distance < 6` (the entity first appears above).
    Stacked { near: bool },
}
/// What `PlacementHandler.tryStackEntity` reads.
#[derive(Clone, Debug)]
pub struct StackFacts {
    /// Passengers already on the pile (`getPassengerCount(lowestEntity)`).
    pub passengers: usize,
    /// The top of the pile is the carried entity itself.
    pub top_is_held: bool,
    pub top_kind: String,
    pub top_area: f64,
    pub held_area: f64,
    /// `entityClicked.blockPosition().distSqr(player.blockPosition())`.
    pub distance_sq: f64,
}
pub fn stack_decision(f: &StackFacts, in_tag: &dyn Fn(&str) -> bool) -> Stack {
    if !config::SETTINGS_STACKABLE_ENTITIES {
        return Stack::No;
    }
    if f.passengers as i64 >= config::SETTINGS_MAX_ENTITY_STACK_LIMIT - 1 {
        return Stack::Refused;
    }
    if f.top_is_held {
        return Stack::No;
    }
    if !stacking_permitted(&f.top_kind, in_tag) {
        return Stack::No;
    }
    if !config::SETTINGS_ENTITY_SIZE_MATTERS_STACKING || f.held_area <= f.top_area {
        Stack::Stacked { near: f.distance_sq < 6.0 }
    } else {
        Stack::Refused
    }
}

/// The reach/position rule of `tryPlaceBlock` / `tryPlaceEntity`: the position the carried object goes to.
/// `replaceable(clicked)` says whether the clicked block can be replaced; if not the neighbour on `face` is used.
pub fn placement_pos(clicked: Pos, face: [i32; 3], replaceable: bool) -> Pos {
    if replaceable { clicked } else { [clicked[0] + face[0], clicked[1] + face[1], clicked[2] + face[2]] }
}

// ------------------------------------------------------------------------------------------------
// The carried object (CarryOnData)
// ------------------------------------------------------------------------------------------------

/// `CarryOnData.CarryType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CarryType {
    Block,
    Entity,
    Player,
    #[default]
    Invalid,
}
impl CarryType {
    pub fn name(self) -> &'static str {
        match self {
            CarryType::Block => "BLOCK",
            CarryType::Entity => "ENTITY",
            CarryType::Player => "PLAYER",
            CarryType::Invalid => "INVALID",
        }
    }
    pub fn from_name(s: &str) -> CarryType {
        match s {
            "BLOCK" => CarryType::Block,
            "ENTITY" => CarryType::Entity,
            "PLAYER" => CarryType::Player,
            _ => CarryType::Invalid,
        }
    }
}
