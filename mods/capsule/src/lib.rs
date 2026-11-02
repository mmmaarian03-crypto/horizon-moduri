// Capsule (Lythom, MIT; `Capsule-1.20.1-8.0.105.jar`) for Horizon: the rules, without game code. Written from the
// 1.20.1 jar (decompiled: CapsuleItem, helpers/Capsule, helpers/Spacial, StructureSaver, structure/CapsuleTemplate,
// network/CapsuleThrowQueryToServer, enchantments/RecallEnchant, dispenser/DispenseCapsuleBehavior) and the public
// source (github.com/Lythom/capsule, branch 1.20, for names and comments), with the pack's `capsule-common.toml`.
// Pasted into the game as `crate::game::capsule::engine` (tools/hnmod.py bundle); also the crate the tests use.
use std::collections::BTreeMap;

pub type Pos = [i32; 3];

/// the pack's `config/capsule-common.toml` (the values the port uses).
pub mod config {
    /// [balance] previewDisplayDuration: ticks an activated capsule stays activated.
    pub const PREVIEW_DISPLAY_DURATION: i64 = 120;
    /// [balance] capsuleUpgradesLimit.
    pub const UPGRADE_LIMIT: i32 = 10;
    /// [balance] excludedBlocks: ids, `mod:` prefixes (every block of the mod) or block tags.
    pub const EXCLUDED_BLOCKS: &[&str] = &[
        "minecraft:spawner",
        "minecraft:end_portal",
        "minecraft:end_portal_frame",
        "minecraft:air",
        "minecraft:structure_void",
        "minecraft:bedrock",
        "ic2:",
        "refinedstorage:",
        "superfactorymanager:",
        "gregtech:machine",
        "gtadditions:",
        "bloodmagic:alchemy_table",
        "mekanism:machineblock",
        "mekanism:boundingblock",
        "tombstone:player_graves",
    ];
    /// [balance] opExcludedBlocks (overpowered capsules).
    pub const OP_EXCLUDED_BLOCKS: &[&str] = &[
        "minecraft:air",
        "minecraft:structure_void",
        "minecraft:bedrock",
        "ic2:",
        "refinedstorage:",
        "superfactorymanager:",
        "gregtech:machine",
        "gtadditions:",
        "bloodmagic:alchemy_table",
        "mekanism:machineblock",
        "mekanism:boundingblock",
        "tombstone:player_graves",
    ];
    /// [loot] starterMode: each new player gets one random starter capsule.
    pub const STARTER_MODE: &str = "random";
    /// [enchants] recallEnchantRarity = RARE, recallEnchantType = null (only capsules take Recall).
    pub const RECALL_RARITY: &str = "RARE";
}

/// `CapsuleItem.CAPSULE_MAX_CAPTURE_SIZE`, `UNDEPLOY_DELAY` (setUndeployDelay +5), `Spacial` constants.
pub const MAX_SIZE: i32 = 255;
pub const UNDEPLOY_DELAY: i64 = 5;
pub const PREVIEW_REACH: f64 = 18.0;
pub const GRAVITY_PER_TICK: f64 = 0.04f32 as f64;
pub const MAX_BLOCKS_PER_TICK_THROW: f64 = 1.2f32 as f64;
pub const TO_RAD: f32 = 0.017_453_292;

/// `CapsuleItem.CapsuleState` and its saved values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Empty,
    EmptyActivated,
    Activated,
    Linked,
    Deployed,
    OneUse,
    OneUseActivated,
    Blueprint,
}
impl State {
    pub fn from_value(v: i32) -> Option<State> {
        Some(match v {
            0 => State::Empty,
            4 => State::EmptyActivated,
            1 => State::Activated,
            2 => State::Linked,
            3 => State::Deployed,
            5 => State::OneUse,
            6 => State::OneUseActivated,
            7 => State::Blueprint,
            _ => return None,
        })
    }
    pub fn value(self) -> i32 {
        match self {
            State::Empty => 0,
            State::EmptyActivated => 4,
            State::Activated => 1,
            State::Linked => 2,
            State::Deployed => 3,
            State::OneUse => 5,
            State::OneUseActivated => 6,
            State::Blueprint => 7,
        }
    }
    /// The item model of the state (the 1.20.1 `capsule:state` overrides of `models/item/capsule.json`).
    pub fn model(self) -> &'static str {
        match self {
            State::Empty => "empty",
            State::EmptyActivated => "empty_activated",
            State::Activated => "activated",
            State::Linked => "linked",
            State::Deployed => "deployed",
            State::OneUse => "one_use",
            State::OneUseActivated => "one_use_activated",
            State::Blueprint => "blueprint",
        }
    }
}

/// What a capsule stack holds (its 1.20.1 tag, now `minecraft:custom_data`; the base colour is
/// `minecraft:dyed_color`, the 1.20.1 `display.color`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capsule {
    /// `state` (raw: unknown values read as EMPTY, like `CapsuleState.valueOf`... returning null in Java).
    pub state: i32,
    /// `size` as saved (see `size()`).
    pub size: i32,
    /// `color`: the material colour (label surround).
    pub material_color: Option<i32>,
    /// `display.color`: the base colour.
    pub base_color: Option<i32>,
    pub structure: Option<String>,
    pub label: Option<String>,
    pub author: Option<String>,
    pub overpowered: bool,
    pub one_use: bool,
    pub is_reward: bool,
    /// `sourceInventory` present: a blueprint.
    pub blueprint: bool,
    /// `spawnPosition` {x, y, z, dim}.
    pub spawn: Option<(Pos, String)>,
    /// `activetimer.starttime` (game time).
    pub timer_start: Option<i64>,
    /// `deployAt` (BlockPos.asLong).
    pub deploy_at: Option<i64>,
    /// `undeployAt` (game time from which an instant capsule may undeploy).
    pub undeploy_at: i64,
    pub upgraded: i32,
    pub y_offset: i32,
}

impl Capsule {
    pub fn state(&self) -> State {
        State::from_value(self.state).unwrap_or(State::Empty)
    }
    pub fn has(&self, s: State) -> bool {
        self.state == s.value()
    }
    /// `getSize`: 1 by default, at most 255, always odd (an even size grows by one).
    pub fn size(&self) -> i32 {
        let s = if self.size == 0 { 1 } else { self.size };
        if s > MAX_SIZE {
            MAX_SIZE
        } else if s % 2 == 0 {
            s + 1
        } else {
            s
        }
    }
    pub fn extend(&self) -> i32 {
        (self.size() - 1) / 2
    }
    pub fn has_structure_link(&self) -> bool {
        self.structure.is_some()
    }
    pub fn is_activated(&self) -> bool {
        self.has(State::Activated) || self.has(State::EmptyActivated) || self.has(State::OneUseActivated)
    }
    /// `isInstantAndUndeployed`: blueprints, and size-1 capsules not deployed.
    pub fn is_instant_and_undeployed(&self) -> bool {
        self.has(State::Blueprint) || (self.size() == 1 && !self.has(State::Deployed))
    }
    /// `isReward`: `isReward` and one-use.
    pub fn is_reward(&self) -> bool {
        self.is_reward && self.one_use
    }
    /// `canUndeploy` (the later form of the jar's `undeployDelay < tickCount`, on the game time).
    pub fn can_undeploy(&self, now: i64) -> bool {
        now >= self.undeploy_at || self.undeploy_at - now > UNDEPLOY_DELAY
    }
    /// `revertStateFromActivated`.
    pub fn revert_from_activated(&mut self) {
        self.state = if self.blueprint {
            State::Blueprint
        } else if self.one_use {
            State::OneUse
        } else if self.has_structure_link() {
            State::Linked
        } else {
            State::Empty
        }
        .value();
        self.timer_start = None;
    }
    /// `inventoryTick`: an activated capsule goes back after `previewDisplayDuration` ticks (or when its timer is
    /// missing, after a crash). True when it went back (the click-off sound plays when the timer ran out).
    pub fn timer_tick(&mut self, now: i64) -> Option<bool> {
        if !self.is_activated() {
            return None;
        }
        match self.timer_start {
            None => {
                self.revert_from_activated();
                Some(false)
            }
            Some(start) if now >= start + config::PREVIEW_DISPLAY_DURATION || start > now => {
                self.revert_from_activated();
                Some(true)
            }
            _ => None,
        }
    }
}

/// What a right click with a capsule does on the server (`CapsuleItem.use` + `activateCapsule`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseAction {
    /// Offhand, or nothing to do.
    Nothing,
    /// The label screen (Shift + right click on a capsule with content).
    Label,
    /// EMPTY / LINKED / ONE_USE become activated: timer started, click-on sound (0.2, 0.9).
    Activate(State),
    /// DEPLOYED: the content comes back (`resentToCapsule`), click-off sound (0.2, 0.4).
    Undeploy,
    /// Instant capsule (size 1 or blueprint): capture or deploy at the aimed block now.
    Instant,
    /// Activated: thrown (`CapsuleThrowQueryToServer`, not instant).
    Throw,
}
pub fn use_action(c: &Capsule, main_hand: bool, sneaking: bool, now: i64) -> UseAction {
    if !main_hand {
        return UseAction::Nothing;
    }
    if sneaking && (c.has(State::Linked) || c.has(State::Deployed) || c.has(State::OneUse) || c.has(State::Blueprint)) {
        return UseAction::Label;
    }
    // The client side of `use` sends the throw query after the server side ran in the same click: an activated
    // capsule is thrown, an instant one acts at once.
    if c.is_instant_and_undeployed() {
        return UseAction::Instant;
    }
    if c.is_activated() {
        return UseAction::Throw;
    }
    if !c.can_undeploy(now) {
        return UseAction::Nothing;
    }
    match c.state() {
        State::Empty => UseAction::Activate(State::EmptyActivated),
        State::Linked => UseAction::Activate(State::Activated),
        State::OneUse => UseAction::Activate(State::OneUseActivated),
        State::Deployed if c.spawn.is_some() => UseAction::Undeploy,
        _ => UseAction::Nothing,
    }
}

/// `BlockPos.asLong` / `BlockPos.of` (26 bits x, 26 bits z, 12 bits y).
pub fn pos_to_long(p: Pos) -> i64 {
    ((i64::from(p[0]) & 0x3FF_FFFF) << 38) | ((i64::from(p[2]) & 0x3FF_FFFF) << 12) | (i64::from(p[1]) & 0xFFF)
}
pub fn pos_of_long(v: i64) -> Pos {
    [(v >> 38) as i32, ((v << 52) >> 52) as i32, ((v << 26) >> 38) as i32]
}

/// The aimed deploy position (1.20.1 jar): the block hit plus its face, plus `yOffset`.
pub fn deploy_target(hit: Pos, face: Pos, y_offset: i32) -> Pos {
    [hit[0] + face[0], hit[1] + face[1] + y_offset, hit[2] + face[2]]
}

/// `Spacial.findBottomBlock` (jar): the block of `betweenClosed((x, y-1, z), (x+1, y+1, z+1))` whose centre is
/// nearest to the item (the first of equals in iteration order: x, then y, then z).
pub fn find_bottom_block(x: f64, y: f64, z: f64) -> Pos {
    let lo = [x.floor() as i32, (y - 1.0).floor() as i32, z.floor() as i32];
    let hi = [(x + 1.0).floor() as i32, (y + 1.0).floor() as i32, (z + 1.0).floor() as i32];
    let mut best = lo;
    let mut best_d = f64::MAX;
    for pz in lo[2]..=hi[2] {
        for py in lo[1]..=hi[1] {
            for px in lo[0]..=hi[0] {
                let d = (f64::from(px) + 0.5 - x).powi(2) + (f64::from(py) + 0.5 - y).powi(2) + (f64::from(pz) + 0.5 - z).powi(2);
                if d < best_d {
                    best_d = d;
                    best = [px, py, pz];
                }
            }
        }
    }
    best
}

/// `Spacial.getAnchor`: from the capture base (its `facing`) to the anchor block of the area.
pub fn anchor(base: Pos, facing: Pos, size: i32) -> Pos {
    let s = f64::from(size);
    let x = f64::from(base[0]) + f64::from(facing[0]) * (0.5 + s * 0.5);
    let y = base[1] + facing[1] + if facing[1] < 0 { -size } else { -1 };
    let z = f64::from(base[2]) + f64::from(facing[2]) * (0.5 + s * 0.5);
    [x.floor() as i32, y, z.floor() as i32]
}
/// `captureContentIntoCapsule`: the area's lowest corner above the anchor.
pub fn capture_source(anchor: Pos, extend: i32) -> Pos {
    [anchor[0] - extend, anchor[1] + 1, anchor[2] - extend]
}
/// `deployCapsule`: the content's lowest corner (`deployAt` centre, else one above the anchor).
pub fn deploy_dest(anchor: Pos, extend: i32, deploy_at: Option<Pos>) -> Pos {
    match deploy_at {
        Some(c) => [c[0] - extend, c[1], c[2] - extend],
        None => [anchor[0] - extend, anchor[1] + 1, anchor[2] - extend],
    }
}
/// `Spacial.findSpecificBlock`: the cubes of growing range 1..maxRange around the item, in `betweenClosed` order;
/// the first block that matches.
pub fn search_order(x: f64, y: f64, z: f64, max_range: i32) -> impl Iterator<Item = Pos> {
    (1..max_range).flat_map(move |r| {
        let r = f64::from(r);
        let lo = [(x - r).floor() as i32, (y - r).floor() as i32, (z - r).floor() as i32];
        let hi = [(x + r).floor() as i32, (y + r).floor() as i32, (z + r).floor() as i32];
        (lo[2]..=hi[2]).flat_map(move |pz| (lo[1]..=hi[1]).flat_map(move |py| (lo[0]..=hi[0]).map(move |px| [px, py, pz])))
    })
}

/// `Config.excludedBlocks` (or the overpowered list) and the `capsule:excluded` tag: never captured.
pub fn excluded(block: &str, overpowered: bool, in_excluded_tag: bool) -> bool {
    let list = if overpowered { config::OP_EXCLUDED_BLOCKS } else { config::EXCLUDED_BLOCKS };
    in_excluded_tag
        || list.iter().any(|e| if e.ends_with(':') { block.starts_with(e) } else { block == *e })
}

/// The positions of a size³ area, in `BlockPos.betweenClosed` order (x, then y, then z).
pub fn area(start: Pos, size: i32) -> impl Iterator<Item = Pos> {
    (0..size).flat_map(move |z| (0..size).flat_map(move |y| (0..size).map(move |x| [start[0] + x, start[1] + y, start[2] + z])))
}

/// What a capture takes (`CapsuleTemplate.snapshotBlocksFromWorld`): every block of the area except air, the
/// excluded ones, and on an undeploy the blocks that were already there when it deployed (same block).
pub fn capture_positions(
    start: Pos,
    size: i32,
    block_at: impl Fn(Pos) -> String,
    is_air: impl Fn(&str) -> bool,
    is_excluded: impl Fn(&str) -> bool,
    occupied: Option<&BTreeMap<Pos, String>>,
) -> Vec<Pos> {
    area(start, size)
        .filter(|p| {
            let b = block_at(*p);
            !is_air(&b) && !is_excluded(&b) && occupied.is_none_or(|o| o.get(p) != Some(&b))
        })
        .collect()
}

/// Why a deploy cannot happen (`StructureSaver.checkDestination`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeployError {
    NotLoaded,
    /// `capsule.error.cantMergeWithDestination` at this position.
    Block(Pos),
    /// `capsule.error.cantMergeWithDestinationEntity` (a living non-player entity).
    Entity(String),
}

/// `checkDestination`: from the top layer down, each position of the size³ area: a world block that is not air
/// nor `#capsule:overridable` blocks a template block that is not overridable (air counts as nothing); a living
/// non-player entity blocks a template block. Returns the occupied positions (non-air world blocks).
pub fn check_destination(
    dest: Pos,
    size: i32,
    template_at: impl Fn(Pos) -> Option<String>,
    world_at: impl Fn(Pos) -> Option<String>,
    is_air: impl Fn(&str) -> bool,
    overridable: impl Fn(&str) -> bool,
    entity_at: impl Fn(Pos) -> Option<String>,
) -> Result<BTreeMap<Pos, String>, DeployError> {
    let mut occupied = BTreeMap::new();
    for y in (0..size).rev() {
        for x in 0..size {
            for z in 0..size {
                let p = [dest[0] + x, dest[1] + y, dest[2] + z];
                let src = template_at(p);
                let Some(world) = world_at(p) else {
                    return Err(DeployError::NotLoaded);
                };
                let world_air = is_air(&world);
                let world_occupied = !world_air && !overridable(&world);
                if !world_air {
                    occupied.insert(p, world.clone());
                }
                let src_air = src.as_deref().is_none_or(&is_air);
                let src_overridable = src.as_deref().is_some_and(&overridable);
                let src_occupied = !src_air && !src_overridable;
                if src_occupied && let Some(e) = entity_at(p) {
                    return Err(DeployError::Entity(e));
                }
                // `templateBlockState.is(overridable)`: air is not in the tag, so an empty cell is blocked too.
                if world_occupied && !src_overridable {
                    return Err(DeployError::Block(p));
                }
            }
        }
    }
    Ok(occupied)
}

/// `spawnBlocksAndEntities`: a template block goes where the world was empty or overridable.
pub fn deploy_places(occupied: &BTreeMap<Pos, String>, p: Pos, overridable: impl Fn(&str) -> bool) -> bool {
    occupied.get(&p).is_none_or(|b| overridable(b))
}

/// `Capsule.throwCapsule`: the item's speed. With a target: toward its centre, up then down.
pub fn throw_velocity(player_block: Pos, dest: Option<Pos>, item_pos: [f64; 3], yaw: f32, pitch: f32, player_motion: [f64; 3]) -> [f64; 3] {
    match dest {
        Some(d) => {
            let (vx, vz) = move_toward(item_pos, [0.0; 3], d, false);
            let dx = f64::from(d[0]) + 0.5 - f64::from(player_block[0]);
            let dz = f64::from(d[2]) + 0.5 - f64::from(player_block[2]);
            let flat = (dx * dx + dz * dz).sqrt();
            let dy = f64::from(d[1] - player_block[1]) + (flat / 3.0).min(1.0);
            let vy = dy / 10.0 - 0.5 * 10.0 * -1.0 * GRAVITY_PER_TICK;
            [vx, vy.max(0.05), vz]
        }
        None => {
            let f = 0.5f32;
            let (sy, cy) = ((yaw * TO_RAD).sin(), (yaw * TO_RAD).cos());
            let (sp, cp) = ((pitch * TO_RAD).sin(), (pitch * TO_RAD).cos());
            [
                f64::from(-sy * cp * f) + player_motion[0],
                f64::from(-sp * f + 0.1) + player_motion[1],
                f64::from(cy * cp * f) + player_motion[2],
            ]
        }
    }
}

/// `Spacial.moveItemEntityToDeployPos`: the horizontal speed toward the target's centre, slower when close
/// (distance / 10, at most 1.2 a tick); with momentum 0.9 of the old speed + 0.1 of that.
pub fn move_toward(item: [f64; 3], v: [f64; 3], dest: Pos, keep_momentum: bool) -> (f64, f64) {
    let dx = f64::from(dest[0]) + 0.5 - item[0];
    let dz = f64::from(dest[2]) + 0.5 - item[2];
    let distance = (dx * dx + dz * dz).sqrt();
    let velocity = (distance / 10.0).min(MAX_BLOCKS_PER_TICK_THROW);
    let (nx, nz) = if distance > 0.0 { (dx / distance, dz / distance) } else { (0.0, 0.0) };
    if keep_momentum {
        (0.9 * v[0] + 0.1 * nx * velocity, 0.9 * v[2] + 0.1 * nz * velocity)
    } else {
        (nx * velocity, nz * velocity)
    }
}

/// `StructureSaver.getUniqueName`: `c-<player in lower case, only id characters>-<count>`.
pub fn unique_name(player: &str, count: u64) -> String {
    let p: String = player
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/'))
        .collect();
    format!("c-{p}-{count}")
}

/// `Capsule.labelFromPath`: the file name, `_` as spaces, each word capitalised (`WordUtils.capitalize`).
pub fn label_from_path(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path).replace('_', " ");
    name.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `CapsuleItem.getName`: (state key, content: Label / Empty / Unlabeled, name key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    Empty,
    Unlabeled,
    Label(String),
}
pub fn name_parts(c: &Capsule) -> (Option<&'static str>, Option<Content>, &'static str) {
    let mut name = "items.capsule.name";
    let state = match c.state() {
        State::Activated | State::EmptyActivated | State::OneUseActivated => Some("items.capsule.state_activated"),
        State::Linked => None,
        State::Deployed => {
            if c.blueprint {
                name = "items.capsule.state_blueprint";
                None
            } else {
                Some("items.capsule.state_deployed")
            }
        }
        State::OneUse => Some(if c.is_reward() { "items.capsule.state_one_use" } else { "items.capsule.state_recovery" }),
        State::Blueprint => {
            name = "items.capsule.state_blueprint";
            None
        }
        State::Empty => None,
    };
    // `getLabel`.
    let content = if !c.has_structure_link() && !c.has(State::Linked) {
        Some(Content::Empty)
    } else if let Some(l) = c.label.as_ref().filter(|l| !l.is_empty()) {
        Some(Content::Label(l.clone()))
    } else {
        Some(Content::Unlabeled)
    };
    (state, content, name)
}

/// `appendHoverText` (the non-advanced lines): (key or literal, argument). `size` line: `S×S×S`, upgrades, instant.
pub fn tooltip(c: &Capsule) -> Vec<Line> {
    let mut out = Vec::new();
    if let Some(a) = &c.author {
        out.push(Line::Author(a.clone()));
    }
    if c.has(State::OneUse) {
        out.push(Line::Key("capsule.tooltip.one_use"));
    }
    if c.overpowered {
        out.push(Line::Overpowered);
    }
    out.push(Line::Size { size: c.size(), upgraded: c.upgraded, instant: c.is_instant_and_undeployed() || c.blueprint });
    out
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Author(String),
    Key(&'static str),
    Overpowered,
    /// `Size: S×S×S[ (u/10 Upgrades)][ (Instantaneous)]`.
    Size { size: i32, upgraded: i32, instant: bool },
}

/// The counter of `CapsuleSavedData` (`getNextCount`), saved with the world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State2 {
    pub count: u64,
    /// Players who got their starter capsule (UUID as text).
    pub starters: Vec<String>,
}
impl State2 {
    pub fn load(text: Option<String>) -> Self {
        let mut s = State2::default();
        for line in text.unwrap_or_default().lines() {
            if let Some(n) = line.strip_prefix("count ") {
                s.count = n.trim().parse().unwrap_or(0);
            } else if let Some(u) = line.strip_prefix("starter ") {
                s.starters.push(u.trim().to_owned());
            }
        }
        s
    }
    pub fn save(&self) -> String {
        let mut out = format!("count {}\n", self.count);
        for u in &self.starters {
            out.push_str(&format!("starter {u}\n"));
        }
        out
    }
    /// `getNextCount`.
    pub fn next(&mut self) -> u64 {
        let n = self.count;
        self.count += 1;
        n
    }
}

/// `StarterLoot`: a reward capsule's size from its template (`max(x, y, z)`, made odd).
pub fn size_of_template(size: [i32; 3]) -> i32 {
    let s = size[0].max(size[1]).max(size[2]);
    if s % 2 == 0 { s + 1 } else { s }
}


/// The original's plain data, read from the 1.20.1 jar by tools/jar_data.py: constants, enum values, static
/// tables, numbers given to builders, constant getters, chances, the numbers of each method, and pure static
/// methods translated from bytecode (each checked against the JVM). Use `jar_data::NAME` and cite it; delete what
/// the port does not need.
#[allow(dead_code, non_upper_case_globals, non_camel_case_types, non_snake_case, clippy::all)]
pub mod jar_data {
    // Data read from the 1.20.1 jar `Capsule-1.20.1-8.0.105.jar` by tools/jar_data.py (first pass, generated).
    // Each constant names the Java class and the field / method it comes from. Values that depend on a
    // branch, a parameter or the config are left out. Keep what the rules need, cite it, delete the rest.

    // ---- static final constants (ConstantValue)
    /// `capsule.CapsuleMod.MODID`
    pub const CAPSULE_MOD_MODID: &'static str = "capsule";
    /// `capsule.Config.CATEGORY_BALANCE`
    pub const CONFIG_CATEGORY_BALANCE: &'static str = "balance";
    /// `capsule.Config.CATEGORY_LOOT`
    pub const CONFIG_CATEGORY_LOOT: &'static str = "loot";
    /// `capsule.Config.CATEGORY_ENCHANTS`
    pub const CONFIG_CATEGORY_ENCHANTS: &'static str = "enchants";
    /// `capsule.StructureSaver.BLUEPRINT_PREFIX`
    pub const STRUCTURE_SAVER_BLUEPRINT_PREFIX: &'static str = "b-";
    /// `capsule.client.CapsulePreviewHandler.NS_TO_MS`
    pub const CAPSULE_PREVIEW_HANDLER_NS_TO_MS: f64 = 1.0e-6;
    /// `capsule.client.render.vbo.MultiVBORenderer.BUFFER_SIZE`
    pub const MULTI_VBORENDERER_BUFFER_SIZE: i32 = 6291456;
    /// `capsule.helpers.Spacial.MAX_BLOCKS_PER_TICK_THROW`
    pub const SPACIAL_MAX_BLOCKS_PER_TICK_THROW: f32 = 1.2;
    /// `capsule.items.CapsuleItem.CAPSULE_MAX_CAPTURE_SIZE`
    pub const CAPSULE_ITEM_CAPSULE_MAX_CAPTURE_SIZE: i32 = 255;
    /// `capsule.items.CapsuleItem.TO_RAD`
    pub const CAPSULE_ITEM_TO_RAD: f32 = 0.017453292;
    /// `capsule.items.CapsuleItem.GRAVITY_PER_TICK`
    pub const CAPSULE_ITEM_GRAVITY_PER_TICK: f32 = 0.04;
    /// `capsule.items.CapsuleItems.UPGRADE_STEP`
    pub const CAPSULE_ITEMS_UPGRADE_STEP: i32 = 2;
    /// `capsule.loot.CapsuleLootEntry.DEFAULT_WEIGHT`
    pub const CAPSULE_LOOT_ENTRY_DEFAULT_WEIGHT: i32 = 3;
    /// `capsule.network.CapsuleFullContentAnswerToClient.BUFFER_MAX_SIZE`
    pub const CAPSULE_FULL_CONTENT_ANSWER_TO_CLIENT_BUFFER_MAX_SIZE: i64 = 1000000;
    /// `capsule.network.CapsuleNetwork.PROTOCOL_VERSION`
    pub const CAPSULE_NETWORK_PROTOCOL_VERSION: &'static str = "1";

    // ---- enum values with their constructor arguments
    /// enum `capsule.items.CapsuleItem$CapsuleState`: constructor `(Ljava/lang/String;II)V`; one row per value
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct CapsuleState {
        /// the Java constant (`name()`)
        pub java_name: &'static str,
        pub ordinal: i32,
        /// `capsule.items.CapsuleItem$CapsuleState.value`
        pub value: i32,
    }
    pub const CAPSULE_STATE_VALUES: [CapsuleState; 8] = [
        CapsuleState { java_name: "EMPTY", ordinal: 0, value: 0 },
        CapsuleState { java_name: "EMPTY_ACTIVATED", ordinal: 1, value: 4 },
        CapsuleState { java_name: "ACTIVATED", ordinal: 2, value: 1 },
        CapsuleState { java_name: "LINKED", ordinal: 3, value: 2 },
        CapsuleState { java_name: "DEPLOYED", ordinal: 4, value: 3 },
        CapsuleState { java_name: "ONE_USE", ordinal: 5, value: 5 },
        CapsuleState { java_name: "ONE_USE_ACTIVATED", ordinal: 6, value: 6 },
        CapsuleState { java_name: "BLUEPRINT", ordinal: 7, value: 7 },
    ];

    // ---- static arrays, lists and tables built in <clinit> (or filled by a static method of the class)
    /// `capsule.StructureSaver.preventItemDrop` (static)
    pub const STRUCTURE_SAVER_PREVENT_ITEM_DROP: bool = false;
    /// `capsule.client.CapsulePreviewHandler.lastSize` (static)
    pub const CAPSULE_PREVIEW_HANDLER_LAST_SIZE: i32 = 0;
    /// `capsule.client.CapsulePreviewHandler.lastColor` (static)
    pub const CAPSULE_PREVIEW_HANDLER_LAST_COLOR: i32 = 0;
    /// `capsule.client.CapsulePreviewHandler.uncompletePreviewsCount` (static)
    pub const CAPSULE_PREVIEW_HANDLER_UNCOMPLETE_PREVIEWS_COUNT: i32 = 0;
    /// `capsule.client.CapsulePreviewHandler.completePreviewsCount` (static)
    pub const CAPSULE_PREVIEW_HANDLER_COMPLETE_PREVIEWS_COUNT: i32 = 0;
    /// `capsule.client.CapsulePreviewHandler.time` (static)
    pub const CAPSULE_PREVIEW_HANDLER_TIME: f64 = 0.0;
    /// `capsule.gui.LabelGui.GUI_WIDTH` (static)
    pub const LABEL_GUI_GUI_WIDTH: i32 = 250;
    /// `capsule.gui.LabelGui.GUI_HEIGHT` (static)
    pub const LABEL_GUI_GUI_HEIGHT: i32 = 20;
    /// `capsule.items.CapsuleItem.lastRotationTime` (static)
    pub const CAPSULE_ITEM_LAST_ROTATION_TIME: i64 = 0;
    /// `capsule.loot.CapsuleLootEntry.COLOR_PALETTE` (static)
    pub const CAPSULE_LOOT_ENTRY_COLOR_PALETTE: [&'static str; 11] = ["0xCCCCCC", "0x549b57", "0xe08822", "0x5e8eb7", "0x6c6c6c", "0xbd5757", "0x99c33d", "0x4a4cba", "0x7b2e89", "0x95d5e7", "0xffffff"];
    /// `capsule.network.CapsuleNetwork.CAPSULE_CHANNEL_MESSAGE_ID` (static)
    pub const CAPSULE_NETWORK_CAPSULE_CHANNEL_MESSAGE_ID: i32 = 1;

    // ---- numbers passed to builders and constructors (durability, damage, speed, cooldowns...)
    /// `capsule.Config.configureCapture`: `ForgeConfigSpec.Builder.defineInRange('previewDisplayDuration', 120, 0, 2147483647)`
    pub const CONFIG_DEFINE_IN_RANGE: (i32, i32, i32) = (120, 0, 2147483647);
    /// `capsule.Config.configureCapture`: `ForgeConfigSpec.Builder.defineInRange('capsuleUpgradesLimit', 10, 0, 2147483647)`
    pub const CONFIG_DEFINE_IN_RANGE_2: (i32, i32, i32) = (10, 0, 2147483647);
    /// `capsule.blocks.BlockCapsuleMarker.<init>`: `BlockBehaviour.Properties.strength(2.5)`
    pub const BLOCK_CAPSULE_MARKER_STRENGTH: f32 = 2.5;
    /// `capsule.client.render.vbo.MultiVBORenderer.sort`: `CustomBufferBuilder.new(6291456)`
    pub const MULTI_VBORENDERER_CUSTOM_BUFFER_BUILDER: i32 = 6291456;
    /// `capsule.client.render.vbo.MultiVBORenderer.lambda$of$0`: `CustomBufferBuilder.new(6291456)`
    pub const MULTI_VBORENDERER_CUSTOM_BUFFER_BUILDER_2: i32 = 6291456;
    /// `capsule.command.CapsuleCommand.register`: `IntegerArgumentType.integer(1, 255)`
    pub const CAPSULE_COMMAND_INTEGER: (i32, i32) = (1, 255);
    /// `capsule.command.CapsuleCommand.lambda$register$44`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$42`: `CommandSourceStack.hasPermission(0)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_2: i32 = 0;
    /// `capsule.command.CapsuleCommand.lambda$register$40`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_3: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$38`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_4: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$36`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_5: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$34`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_6: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$32`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_7: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$29`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_8: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$27`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_9: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$24`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_10: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$21`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_11: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$19`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_12: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$17`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_13: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$14`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_14: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$10`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_15: i32 = 2;
    /// `capsule.command.CapsuleCommand.lambda$register$6`: `CommandSourceStack.hasPermission(2)`
    pub const CAPSULE_COMMAND_HAS_PERMISSION_16: i32 = 2;
    /// `capsule.helpers.Blueprint.getBlockItemCost`: `FluidStack.new(?, 1000)`
    pub const BLUEPRINT_FLUID_STACK: i32 = 1000;
    /// `capsule.helpers.Capsule.throwCapsule`: `ItemEntity.setPickUpDelay(20)`
    pub const CAPSULE_SET_PICK_UP_DELAY: i32 = 20;
    /// `capsule.helpers.Spacial.isImmergedInLiquid`: `Entity.isFree(0.0, 1.5, 0.0)`
    pub const SPACIAL_IS_FREE: (f64, f64, f64) = (0.0, 1.5, 0.0);
    /// `capsule.helpers.Spacial.isEntityCollidingLiquid`: `ItemEntity.isFree(0.0, -0.1, 0.0)`
    pub const SPACIAL_IS_FREE_2: (f64, f64, f64) = (0.0, -0.1, 0.0);
    /// `capsule.items.CapsuleItem$CapsuleState.<clinit>`: `CapsuleItem.CapsuleState.new('EMPTY', 0, 0)`
    pub const CAPSULE_STATE_CAPSULE_ITEM_CAPSULE_STATE: (i32, i32) = (0, 0);
    /// `capsule.items.CapsuleItem$CapsuleState.<clinit>`: `CapsuleItem.CapsuleState.new('EMPTY_ACTIVATED', 1, 4)`
    pub const CAPSULE_STATE_CAPSULE_ITEM_CAPSULE_STATE_2: (i32, i32) = (1, 4);
    /// `capsule.items.CapsuleItem$CapsuleState.<clinit>`: `CapsuleItem.CapsuleState.new('ACTIVATED', 2, 1)`
    pub const CAPSULE_STATE_CAPSULE_ITEM_CAPSULE_STATE_3: (i32, i32) = (2, 1);
    /// `capsule.items.CapsuleItem$CapsuleState.<clinit>`: `CapsuleItem.CapsuleState.new('LINKED', 3, 2)`
    pub const CAPSULE_STATE_CAPSULE_ITEM_CAPSULE_STATE_4: (i32, i32) = (3, 2);
    /// `capsule.items.CapsuleItem$CapsuleState.<clinit>`: `CapsuleItem.CapsuleState.new('DEPLOYED', 4, 3)`
    pub const CAPSULE_STATE_CAPSULE_ITEM_CAPSULE_STATE_5: (i32, i32) = (4, 3);
    /// `capsule.items.CapsuleItem$CapsuleState.<clinit>`: `CapsuleItem.CapsuleState.new('ONE_USE', 5, 5)`
    pub const CAPSULE_STATE_CAPSULE_ITEM_CAPSULE_STATE_6: (i32, i32) = (5, 5);
    /// `capsule.items.CapsuleItem$CapsuleState.<clinit>`: `CapsuleItem.CapsuleState.new('ONE_USE_ACTIVATED', 6, 6)`
    pub const CAPSULE_STATE_CAPSULE_ITEM_CAPSULE_STATE_7: (i32, i32) = (6, 6);
    /// `capsule.items.CapsuleItem$CapsuleState.<clinit>`: `CapsuleItem.CapsuleState.new('BLUEPRINT', 7, 7)`
    pub const CAPSULE_STATE_CAPSULE_ITEM_CAPSULE_STATE_8: (i32, i32) = (7, 7);
    /// `capsule.items.CapsuleItem.<init>`: `Item.Properties.stacksTo(1)`
    pub const CAPSULE_ITEM_STACKS_TO: i32 = 1;
    /// `capsule.items.CapsuleItem.<init>`: `Item.Properties.durability(0)`
    pub const CAPSULE_ITEM_DURABILITY: i32 = 0;
    /// `capsule.loot.CapsuleLootEntry.<init>`: `LootPoolSingletonContainer.new(?, 0, ?, ?)`
    pub const CAPSULE_LOOT_ENTRY_LOOT_POOL_SINGLETON_CONTAINER: i32 = 0;
    /// `capsule.loot.CapsuleLootTableHook.hookCapsulesOnLootTable`: `ConstantValue.exactly(0.0)`
    pub const CAPSULE_LOOT_TABLE_HOOK_EXACTLY: f32 = 0.0;
    /// `capsule.loot.CapsuleLootTableHook.hookCapsulesOnLootTable`: `ConstantValue.exactly(1.0)`
    pub const CAPSULE_LOOT_TABLE_HOOK_EXACTLY_2: f32 = 1.0;
    /// `capsule.network.CapsuleNetwork.setup`: `SimpleChannel.registerMessage(1, 'capsule.network.LabelEditedMessageToServer', ?, ?, ?)`
    pub const CAPSULE_NETWORK_REGISTER_MESSAGE: i32 = 1;
    /// `capsule.network.CapsuleNetwork.setup`: `SimpleChannel.registerMessage(1, 'capsule.network.CapsuleContentPreviewQueryToServer', ?, ?, ?)`
    pub const CAPSULE_NETWORK_REGISTER_MESSAGE_2: i32 = 1;
    /// `capsule.network.CapsuleNetwork.setup`: `SimpleChannel.registerMessage(1, 'capsule.network.CapsuleThrowQueryToServer', ?, ?, ?)`
    pub const CAPSULE_NETWORK_REGISTER_MESSAGE_3: i32 = 1;
    /// `capsule.network.CapsuleNetwork.setup`: `SimpleChannel.registerMessage(1, 'capsule.network.CapsuleLeftClickQueryToServer', ?, ?, ?)`
    pub const CAPSULE_NETWORK_REGISTER_MESSAGE_4: i32 = 1;
    /// `capsule.network.CapsuleNetwork.setup`: `SimpleChannel.registerMessage(1, 'capsule.network.CapsuleContentPreviewAnswerToClient', ?, ?, ?)`
    pub const CAPSULE_NETWORK_REGISTER_MESSAGE_5: i32 = 1;
    /// `capsule.network.CapsuleNetwork.setup`: `SimpleChannel.registerMessage(1, 'capsule.network.CapsuleUndeployNotifToClient', ?, ?, ?)`
    pub const CAPSULE_NETWORK_REGISTER_MESSAGE_6: i32 = 1;
    /// `capsule.network.CapsuleNetwork.setup`: `SimpleChannel.registerMessage(1, 'capsule.network.CapsuleFullContentAnswerToClient', ?, ?, ?)`
    pub const CAPSULE_NETWORK_REGISTER_MESSAGE_7: i32 = 1;
    /// `capsule.structure.CapsuleTemplate$BasicPalette.<init>`: `IdMapper.new(16)`
    pub const BASIC_PALETTE_ID_MAPPER: i32 = 16;
    /// `capsule.structure.CapsuleTemplate.spawnBlocksAndEntities`: `StructureTemplate.updateShapeAtEdge(?, 2, ?, ?, ?, ?)`
    pub const CAPSULE_TEMPLATE_UPDATE_SHAPE_AT_EDGE: i32 = 2;

    // ---- methods that only return a constant (data overrides)
    /// `capsule.blocks.CaptureBER.shouldRenderOffScreen(BlockEntityCapture)` returns it
    pub const CAPTURE_BER_SHOULD_RENDER_OFF_SCREEN: bool = true;
    /// `capsule.client.render.FakeWorld.isFluidAtPosition(BlockPos, Predicate)` returns it
    pub const FAKE_WORLD_IS_FLUID_AT_POSITION: bool = false;
    /// `capsule.client.render.FakeWorld.setBlock(BlockPos, BlockState, I, I)` returns it
    pub const FAKE_WORLD_SET_BLOCK: bool = false;
    /// `capsule.client.render.FakeWorld.destroyBlock(BlockPos, Z, Entity, I)` returns it
    pub const FAKE_WORLD_DESTROY_BLOCK: bool = false;
    /// `capsule.client.render.FakeWorld.getBrightness(BlockPos)` returns it
    pub const FAKE_WORLD_GET_BRIGHTNESS: f32 = 15.0;
    /// `capsule.client.render.FakeWorld.getBrightness(LightLayer, BlockPos)` returns it
    pub const FAKE_WORLD_GET_BRIGHTNESS_2: i32 = 15;
    /// `capsule.client.render.FakeWorld.getRawBrightness(BlockPos, I)` returns it
    pub const FAKE_WORLD_GET_RAW_BRIGHTNESS: i32 = 15;
    /// `capsule.client.render.FakeWorld.canSeeSky(BlockPos)` returns it
    pub const FAKE_WORLD_CAN_SEE_SKY: bool = true;
    /// `capsule.client.render.FakeWorld.getShade(Direction, Z)` returns it
    pub const FAKE_WORLD_GET_SHADE: f32 = 1.0;
    /// `capsule.command.CapsuleCommand.executeDownloadTemplate(ServerPlayer)` returns it
    pub const CAPSULE_COMMAND_EXECUTE_DOWNLOAD_TEMPLATE: i32 = 0;
    /// `capsule.enchantments.RecallEnchant.getMinCost(I)` returns it
    pub const RECALL_ENCHANT_GET_MIN_COST: i32 = 1;
    /// `capsule.enchantments.RecallEnchant.getMaxLevel()` returns it
    pub const RECALL_ENCHANT_GET_MAX_LEVEL: i32 = 1;
    /// `capsule.items.CapsuleItem.getEnchantmentValue()` returns it
    pub const CAPSULE_ITEM_GET_ENCHANTMENT_VALUE: i32 = 5;
    /// `capsule.recipes.BlueprintCapsuleRecipe.isSpecial()` returns it
    pub const BLUEPRINT_CAPSULE_RECIPE_IS_SPECIAL: bool = true;
    /// `capsule.recipes.CapsuleIngredient.isSimple()` returns it
    pub const CAPSULE_INGREDIENT_IS_SIMPLE: bool = false;
    /// `capsule.recipes.RecoveryCapsuleRecipe.isSpecial()` returns it
    pub const RECOVERY_CAPSULE_RECIPE_IS_SPECIAL: bool = true;
    /// `capsule.recipes.UpgradeCapsuleRecipe.isSpecial()` returns it
    pub const UPGRADE_CAPSULE_RECIPE_IS_SPECIAL: bool = true;

    // ---- numbers in each method body (bytecode order, once each, without 0 / 1 / -1): a checklist of the
    // values the original uses there, not named constants. Ints exact; f32 values written with their f32 digits.
    /// `capsule.CapsuleModEventSubscriber.lambda$clientSetup$0(ItemStack, I)`: integers
    pub const CAPSULE_MOD_EVENT_SUBSCRIBER_LAMBDA_CLIENT_SETUP_0_INTS: &[i64] = &[16777215];
    /// `capsule.Config.configureCapture(ForgeConfigSpec$Builder)`: integers
    pub const CONFIG_CONFIGURE_CAPTURE_INTS: &[i64] = &[120, 2147483647, 10, 3, 2, 9, 4, 5, 6, 7, 8];
    /// `capsule.Config.configureLoot(ForgeConfigSpec$Builder)`: integers
    pub const CONFIG_CONFIGURE_LOOT_INTS: &[i64] = &[25, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24];
    /// `capsule.StructureSaver$ItemStackKey.hashCode()`: integers
    pub const ITEM_STACK_KEY_HASH_CODE_INTS: &[i64] = &[29];
    /// `capsule.StructureSaver.removeTransferedBlockFromWorld(List, ServerLevel, Player)`: integers
    pub const STRUCTURE_SAVER_REMOVE_TRANSFERED_BLOCK_FROM_WORLD_INTS: &[i64] = &[20, 2, 3];
    /// `capsule.StructureSaver.checkDestination(CapsuleTemplate, StructurePlaceSettings, ServerLevel, BlockPos, I, Map, List)`: integers
    pub const STRUCTURE_SAVER_CHECK_DESTINATION_INTS: &[i64] = &[2];
    /// `capsule.StructureSaver.lambda$undeployBlueprint$3(StructureTemplate$StructureBlockInfo)`: integers
    pub const STRUCTURE_SAVER_LAMBDA_UNDEPLOY_BLUEPRINT_3_INTS: &[i64] = &[10];
    /// `capsule.blocks.BlockCapsuleMarker.<init>()`: floats
    pub const BLOCK_CAPSULE_MARKER_INIT_FLOATS: &[f64] = &[2.5];
    /// `capsule.blocks.BlockCapsuleMarker.neighborChanged(BlockState, Level, BlockPos, Block, BlockPos, Z)`: integers
    pub const BLOCK_CAPSULE_MARKER_NEIGHBOR_CHANGED_INTS: &[i64] = &[4];
    /// `capsule.blocks.BlockDeployCrasher.setPlacedBy(Level, BlockPos, BlockState, LivingEntity, ItemStack)`: integers
    pub const BLOCK_DEPLOY_CRASHER_SET_PLACED_BY_INTS: &[i64] = &[70];
    /// `capsule.blocks.CaptureBER.<init>(BlockEntityRendererProvider$Context)`: floats
    pub const CAPTURE_BER_INIT_FLOATS: &[f64] = &[10000.0];
    /// `capsule.blocks.CaptureBER.render(BlockEntityCapture, F, PoseStack, MultiBufferSource, I, I)`: integers
    pub const CAPTURE_BER_RENDER_INTS: &[i64] = &[2];
    /// `capsule.client.CapsulePreviewHandler.tryPreviewDeploy(LocalPlayer, F, ItemStack, PoseStack)`: integers
    pub const CAPSULE_PREVIEW_HANDLER_TRY_PREVIEW_DEPLOY_INTS: &[i64] = &[2];
    /// `capsule.client.CapsulePreviewHandler.tryPreviewDeploy(LocalPlayer, F, ItemStack, PoseStack)`: floats
    pub const CAPSULE_PREVIEW_HANDLER_TRY_PREVIEW_DEPLOY_FLOATS: &[f64] = &[0.01, 1.01];
    /// `capsule.client.CapsulePreviewHandler.DisplayFullPreview(LocalPlayer, ItemStack, PoseStack, I, BlockPos, String)`: integers
    pub const CAPSULE_PREVIEW_HANDLER_DISPLAY_FULL_PREVIEW_INTS: &[i64] = &[2];
    /// `capsule.client.CapsulePreviewHandler.DisplayFullPreview(LocalPlayer, ItemStack, PoseStack, I, BlockPos, String)`: floats
    pub const CAPSULE_PREVIEW_HANDLER_DISPLAY_FULL_PREVIEW_FLOATS: &[f64] = &[0.10000000149011612, 0.14000000059604645, 0.11999999731779099, 0.3, 0.05, 0.15000000596046448, 0.12, 0.5, 0.32, 0.2, 0.8];
    /// `capsule.client.CapsulePreviewHandler.DisplayWireframePreview(LocalPlayer, ItemStack, I, BlockHitResult, I, BlockPos, String, AABB, Z, PoseStack)`: integers
    pub const CAPSULE_PREVIEW_HANDLER_DISPLAY_WIREFRAME_PREVIEW_INTS: &[i64] = &[14540253, 11141120, 255, 160, 16777215];
    /// `capsule.client.CapsulePreviewHandler.DisplayWireframePreview(LocalPlayer, ItemStack, I, BlockHitResult, I, BlockPos, String, AABB, Z, PoseStack)`: floats
    pub const CAPSULE_PREVIEW_HANDLER_DISPLAY_WIREFRAME_PREVIEW_FLOATS: &[f64] = &[0.01, 2.0, 5.0];
    /// `capsule.client.CapsulePreviewHandler.tryPreviewLinkedInventory(LocalPlayer, ItemStack, PoseStack)`: floats
    pub const CAPSULE_PREVIEW_HANDLER_TRY_PREVIEW_LINKED_INVENTORY_FLOATS: &[f64] = &[3600.0];
    /// `capsule.client.CapsulePreviewHandler.previewLinkedInventory(BlockPos, PoseStack)`: integers
    pub const CAPSULE_PREVIEW_HANDLER_PREVIEW_LINKED_INVENTORY_INTS: &[i64] = &[6003967, 80];
    /// `capsule.client.CapsulePreviewHandler.previewRecall(ItemStack, PoseStack)`: integers
    pub const CAPSULE_PREVIEW_HANDLER_PREVIEW_RECALL_INTS: &[i64] = &[2];
    /// `capsule.client.CapsulePreviewHandler.setCaptureTESizeColor(I, I, Level)`: integers
    pub const CAPSULE_PREVIEW_HANDLER_SET_CAPTURE_TESIZE_COLOR_INTS: &[i64] = &[2];
    /// `capsule.client.CapsulePreviewHandler.renderRecallBox(PoseStack, I, AABB, VertexConsumer, D)`: integers
    pub const CAPSULE_PREVIEW_HANDLER_RENDER_RECALL_BOX_INTS: &[i64] = &[16, 255, 8, 5];
    /// `capsule.client.CapsulePreviewHandler.renderRecallBox(PoseStack, I, AABB, VertexConsumer, D)`: floats
    pub const CAPSULE_PREVIEW_HANDLER_RENDER_RECALL_BOX_FLOATS: &[f64] = &[0.78431374, 255.0, 0.0015, 0.0017, 0.01, 0.05];
    /// `capsule.client.CapsulePreviewHandler.getMovingEffectPos(AABB, I, D, F)`: floats
    pub const CAPSULE_PREVIEW_HANDLER_GET_MOVING_EFFECT_POS_FLOATS: &[f64] = &[0.5];
    /// `capsule.client.RendererUtils.doPositionPrologue(Camera, PoseStack)`: floats
    pub const RENDERER_UTILS_DO_POSITION_PROLOGUE_FLOATS: &[f64] = &[1.8e2];
    /// `capsule.client.RendererUtils.doWirePrologue()`: floats
    pub const RENDERER_UTILS_DO_WIRE_PROLOGUE_FLOATS: &[f64] = &[3.0];
    /// `capsule.client.RendererUtils.setColor(I, I)`: integers
    pub const RENDERER_UTILS_SET_COLOR_INTS: &[i64] = &[16, 255, 8];
    /// `capsule.client.RendererUtils.setColor(I, I)`: floats
    pub const RENDERER_UTILS_SET_COLOR_FLOATS: &[f64] = &[255.0];
    /// `capsule.client.render.CapsuleTemplateRenderer.renderFluid(PoseStack, BlockPos, BlockAndTintGetter, VertexConsumer, FluidState)`: integers
    pub const CAPSULE_TEMPLATE_RENDERER_RENDER_FLUID_INTS: &[i64] = &[16, 255, 8];
    /// `capsule.client.render.CapsuleTemplateRenderer.renderFluid(PoseStack, BlockPos, BlockAndTintGetter, VertexConsumer, FluidState)`: floats
    pub const CAPSULE_TEMPLATE_RENDERER_RENDER_FLUID_FLOATS: &[f64] = &[255.0];
    /// `capsule.client.render.CapsuleTemplateRenderer.vertex(VertexConsumer, F, F, F, F, F, Matrix4f, I, I, I)`: integers
    pub const CAPSULE_TEMPLATE_RENDERER_VERTEX_INTS: &[i64] = &[256];
    /// `capsule.client.render.CapsuleTemplateRenderer.changeTemplateIfDirty(CapsuleTemplate, Level, BlockPos, BlockPos, StructurePlaceSettings, I)`: integers
    pub const CAPSULE_TEMPLATE_RENDERER_CHANGE_TEMPLATE_IF_DIRTY_INTS: &[i64] = &[2147483647, -2147483648, 20, 5, 2, 3, 4, -2, 16];
    /// `capsule.client.render.FakeWorld.getBrightness(BlockPos)`: floats
    pub const FAKE_WORLD_GET_BRIGHTNESS_FLOATS: &[f64] = &[15.0];
    /// `capsule.client.render.FakeWorld.getBrightness(LightLayer, BlockPos)`: integers
    pub const FAKE_WORLD_GET_BRIGHTNESS_INTS: &[i64] = &[15];
    /// `capsule.client.render.FakeWorld.getRawBrightness(BlockPos, I)`: integers
    pub const FAKE_WORLD_GET_RAW_BRIGHTNESS_INTS: &[i64] = &[15];
    /// `capsule.client.render.vbo.CustomBufferBuilder.<init>(I)`: integers
    pub const CUSTOM_BUFFER_BUILDER_INIT_INTS: &[i64] = &[4];
    /// `capsule.client.render.vbo.CustomBufferBuilder.roundUpPositive(I)`: integers
    pub const CUSTOM_BUFFER_BUILDER_ROUND_UP_POSITIVE_INTS: &[i64] = &[2097152];
    /// `capsule.client.render.vbo.CustomBufferBuilder.sortVertexData(F, F, F)`: integers
    pub const CUSTOM_BUFFER_BUILDER_SORT_VERTEX_DATA_INTS: &[i64] = &[4, 6];
    /// `capsule.client.render.vbo.CustomBufferBuilder.limitToVertex(FloatBuffer, I)`: integers
    pub const CUSTOM_BUFFER_BUILDER_LIMIT_TO_VERTEX_INTS: &[i64] = &[4];
    /// `capsule.client.render.vbo.CustomBufferBuilder.getDistanceSq(FloatBuffer, F, F, F, I, I)`: integers
    pub const CUSTOM_BUFFER_BUILDER_GET_DISTANCE_SQ_INTS: &[i64] = &[2, 3];
    /// `capsule.client.render.vbo.CustomBufferBuilder.getDistanceSq(FloatBuffer, F, F, F, I, I)`: floats
    pub const CUSTOM_BUFFER_BUILDER_GET_DISTANCE_SQ_FLOATS: &[f64] = &[0.25];
    /// `capsule.client.render.vbo.CustomBufferBuilder.vertex(F, F, F, F, F, F, F, F, F, I, I, F, F, F)`: integers
    pub const CUSTOM_BUFFER_BUILDER_VERTEX_INTS: &[i64] = &[4, 8, 12, 13, 14, 15, 16, 20, 24, 65535, 26, 28, 2, 5, 6];
    /// `capsule.client.render.vbo.CustomBufferBuilder.vertex(F, F, F, F, F, F, F, F, F, I, I, F, F, F)`: floats
    pub const CUSTOM_BUFFER_BUILDER_VERTEX_FLOATS: &[f64] = &[255.0];
    /// `capsule.client.render.vbo.CustomVertexBuffer.bindBuffer()`: integers
    pub const CUSTOM_VERTEX_BUFFER_BIND_BUFFER_INTS: &[i64] = &[34962];
    /// `capsule.client.render.vbo.CustomVertexBuffer.uploadRaw(CustomBufferBuilder)`: integers
    pub const CUSTOM_VERTEX_BUFFER_UPLOAD_RAW_INTS: &[i64] = &[34962, 35044];
    /// `capsule.client.render.vbo.CustomVertexBuffer.unbindBuffer()`: integers
    pub const CUSTOM_VERTEX_BUFFER_UNBIND_BUFFER_INTS: &[i64] = &[34962];
    /// `capsule.client.render.vbo.MultiVBORenderer.sort(F, F, F)`: integers
    pub const MULTI_VBORENDERER_SORT_INTS: &[i64] = &[6291456];
    /// `capsule.client.render.vbo.MultiVBORenderer.lambda$of$0(RenderType)`: integers
    pub const MULTI_VBORENDERER_LAMBDA_OF_0_INTS: &[i64] = &[6291456];
    /// `capsule.command.CapsuleCommand.register(CommandDispatcher)`: integers
    pub const CAPSULE_COMMAND_REGISTER_INTS: &[i64] = &[255];
    /// `capsule.command.CapsuleCommand.executeGiveEmpty(ServerPlayer, I, Z)`: integers
    pub const CAPSULE_COMMAND_EXECUTE_GIVE_EMPTY_INTS: &[i64] = &[16777215];
    /// `capsule.command.CapsuleCommand.executeGiveBlueprint(ServerPlayer, String)`: integers
    pub const CAPSULE_COMMAND_EXECUTE_GIVE_BLUEPRINT_INTS: &[i64] = &[2, 3949738, 16777215];
    /// `capsule.command.CapsuleCommand.executeFromExistingReward(ServerPlayer, String)`: integers
    pub const CAPSULE_COMMAND_EXECUTE_FROM_EXISTING_REWARD_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.executeFromStructure(ServerPlayer, String)`: integers
    pub const CAPSULE_COMMAND_EXECUTE_FROM_STRUCTURE_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.executeExportSeenBlock(ServerPlayer)`: integers
    pub const CAPSULE_COMMAND_EXECUTE_EXPORT_SEEN_BLOCK_INTS: &[i64] = &[50];
    /// `capsule.command.CapsuleCommand.lambda$register$44(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_44_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$40(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_40_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$38(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_38_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$36(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_36_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$34(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_34_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$32(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_32_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$29(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_29_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$27(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_27_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$24(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_24_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$21(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_21_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$19(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_19_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$17(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_17_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$14(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_14_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$10(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_10_INTS: &[i64] = &[2];
    /// `capsule.command.CapsuleCommand.lambda$register$7(CommandContext)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_7_INTS: &[i64] = &[3];
    /// `capsule.command.CapsuleCommand.lambda$register$6(CommandSourceStack)`: integers
    pub const CAPSULE_COMMAND_LAMBDA_REGISTER_6_INTS: &[i64] = &[2];
    /// `capsule.dispenser.DispenseCapsuleBehavior.execute(BlockSource, ItemStack)`: integers
    pub const DISPENSE_CAPSULE_BEHAVIOR_EXECUTE_INTS: &[i64] = &[2];
    /// `capsule.dispenser.DispenseCapsuleBehavior.execute(BlockSource, ItemStack)`: floats
    pub const DISPENSE_CAPSULE_BEHAVIOR_EXECUTE_FLOATS: &[f64] = &[0.2, 0.4];
    /// `capsule.dispenser.DispenseCapsuleBehavior.playSound(BlockSource)`: integers
    pub const DISPENSE_CAPSULE_BEHAVIOR_PLAY_SOUND_INTS: &[i64] = &[1000];
    /// `capsule.enchantments.RecallEnchant.getMaxCost(I)`: integers
    pub const RECALL_ENCHANT_GET_MAX_COST_INTS: &[i64] = &[40];
    /// `capsule.gui.LabelGui.init()`: integers
    pub const LABEL_GUI_INIT_INTS: &[i64] = &[2, 32, 200, 10, 20];
    /// `capsule.gui.LabelGui.keyPressed(I, I, I)`: integers
    pub const LABEL_GUI_KEY_PRESSED_INTS: &[i64] = &[256, 257];
    /// `capsule.gui.LabelGui.render(GuiGraphics, I, I, F)`: integers
    pub const LABEL_GUI_RENDER_INTS: &[i64] = &[2, 70, 16777215];
    /// `capsule.helpers.Blueprint.getBlockItemCost(StructureTemplate$StructureBlockInfo)`: integers
    pub const BLUEPRINT_GET_BLOCK_ITEM_COST_INTS: &[i64] = &[2, 1000];
    /// `capsule.helpers.Blueprint.sortTemplatesByIngredients(List, CapsuleTemplateManager)`: integers
    pub const BLUEPRINT_SORT_TEMPLATES_BY_INGREDIENTS_INTS: &[i64] = &[5, 2];
    /// `capsule.helpers.Capsule.notifyUndeploy(Player, BlockPos, I, String)`: integers
    pub const CAPSULE_NOTIFY_UNDEPLOY_INTS: &[i64] = &[2];
    /// `capsule.helpers.Capsule.showUndeployParticules(ClientLevel, BlockPos, BlockPos, I)`: integers
    pub const CAPSULE_SHOW_UNDEPLOY_PARTICULES_INTS: &[i64] = &[8];
    /// `capsule.helpers.Capsule.showUndeployParticules(ClientLevel, BlockPos, BlockPos, I)`: floats
    pub const CAPSULE_SHOW_UNDEPLOY_PARTICULES_FLOATS: &[f64] = &[0.5, 0.10000000149011612, 0.03999999910593033];
    /// `capsule.helpers.Capsule.showDeployParticules(ServerLevel, BlockPos, I)`: integers
    pub const CAPSULE_SHOW_DEPLOY_PARTICULES_INTS: &[i64] = &[8];
    /// `capsule.helpers.Capsule.showDeployParticules(ServerLevel, BlockPos, I)`: floats
    pub const CAPSULE_SHOW_DEPLOY_PARTICULES_FLOATS: &[f64] = &[0.5, 0.25, 0.01, 0.05];
    /// `capsule.helpers.Capsule.throwCapsule(ItemStack, Player, BlockPos)`: integers
    pub const CAPSULE_THROW_CAPSULE_INTS: &[i64] = &[20];
    /// `capsule.helpers.Capsule.throwCapsule(ItemStack, Player, BlockPos)`: floats
    pub const CAPSULE_THROW_CAPSULE_FLOATS: &[f64] = &[0.3, 0.5, 3.0, 10.0, -0.19999999552965164, 0.05, 0.017453292, 0.1, 0.2];
    /// `capsule.helpers.Capsule.handleItemEntityOnGround(ItemEntity, ItemStack)`: integers
    pub const CAPSULE_HANDLE_ITEM_ENTITY_ON_GROUND_INTS: &[i64] = &[2];
    /// `capsule.helpers.Capsule.handleItemEntityOnGround(ItemEntity, ItemStack)`: floats
    pub const CAPSULE_HANDLE_ITEM_ENTITY_ON_GROUND_FLOATS: &[f64] = &[0.4, 0.1];
    /// `capsule.helpers.Capsule.createLinkedCapsuleFromReward(String, ServerPlayer)`: integers
    pub const CAPSULE_CREATE_LINKED_CAPSULE_FROM_REWARD_INTS: &[i64] = &[2];
    /// `capsule.helpers.Capsule.lambda$handleItemEntityOnGround$3(BlockPos, I, ServerLevel)`: integers
    pub const CAPSULE_LAMBDA_HANDLE_ITEM_ENTITY_ON_GROUND_3_INTS: &[i64] = &[200];
    /// `capsule.helpers.Capsule.lambda$notifyUndeploy$0(BlockPos, I, Player)`: integers
    pub const CAPSULE_LAMBDA_NOTIFY_UNDEPLOY_0_INTS: &[i64] = &[200];
    /// `capsule.helpers.MinecraftNBT.hasColor(ItemStack)`: integers
    pub const MINECRAFT_NBT_HAS_COLOR_INTS: &[i64] = &[99];
    /// `capsule.helpers.MinecraftNBT.getColor(ItemStack)`: integers
    pub const MINECRAFT_NBT_GET_COLOR_INTS: &[i64] = &[99, 16777215];
    /// `capsule.helpers.Spacial.isImmergedInLiquid(Entity)`: floats
    pub const SPACIAL_IS_IMMERGED_IN_LIQUID_FLOATS: &[f64] = &[1.5];
    /// `capsule.helpers.Spacial.clientRayTracePreview(Player, F, I)`: integers
    pub const SPACIAL_CLIENT_RAY_TRACE_PREVIEW_INTS: &[i64] = &[18];
    /// `capsule.helpers.Spacial.getAnchor(BlockPos, BlockState, I)`: floats
    pub const SPACIAL_GET_ANCHOR_FLOATS: &[f64] = &[0.5];
    /// `capsule.helpers.Spacial.mergeVoxels(List)`: integers
    pub const SPACIAL_MERGE_VOXELS_INTS: &[i64] = &[8000];
    /// `capsule.helpers.Spacial.isEntityCollidingLiquid(ItemEntity)`: floats
    pub const SPACIAL_IS_ENTITY_COLLIDING_LIQUID_FLOATS: &[f64] = &[-0.1];
    /// `capsule.helpers.Spacial.moveItemEntityToDeployPos(ItemEntity, ItemStack, Z)`: floats
    pub const SPACIAL_MOVE_ITEM_ENTITY_TO_DEPLOY_POS_FLOATS: &[f64] = &[0.5, 10.0, 1.2000000476837158, 0.9, 0.1];
    /// `capsule.helpers.Spacial.getBB(D, D, D, I, I)`: floats
    pub const SPACIAL_GET_BB_FLOATS: &[f64] = &[0.01, 1.01];
    /// `capsule.items.CapsuleItem.getSize(ItemStack)`: integers
    pub const CAPSULE_ITEM_GET_SIZE_INTS: &[i64] = &[255, 2];
    /// `capsule.items.CapsuleItem.setSize(ItemStack, I)`: integers
    pub const CAPSULE_ITEM_SET_SIZE_INTS: &[i64] = &[255, 2];
    /// `capsule.items.CapsuleItem.getEnchantmentValue()`: integers
    pub const CAPSULE_ITEM_GET_ENCHANTMENT_VALUE_INTS: &[i64] = &[5];
    /// `capsule.items.CapsuleItem.onLeftClickBlock(PlayerInteractEvent$LeftClickBlock)`: integers
    pub const CAPSULE_ITEM_ON_LEFT_CLICK_BLOCK_INTS: &[i64] = &[60];
    /// `capsule.items.CapsuleItem.activateCapsule(ItemStack, ServerLevel, Player)`: floats
    pub const CAPSULE_ITEM_ACTIVATE_CAPSULE_FLOATS: &[f64] = &[0.2, 0.4];
    /// `capsule.items.CapsuleItem.startTimer(Level, Player, ItemStack)`: floats
    pub const CAPSULE_ITEM_START_TIMER_FLOATS: &[f64] = &[0.2, 0.9];
    /// `capsule.items.CapsuleItem.setUndeployDelay(ItemStack, Player)`: integers
    pub const CAPSULE_ITEM_SET_UNDEPLOY_DELAY_INTS: &[i64] = &[5];
    /// `capsule.items.CapsuleItem.inventoryTick(ItemStack, Level, Entity, I, Z)`: floats
    pub const CAPSULE_ITEM_INVENTORY_TICK_FLOATS: &[f64] = &[0.2, 0.4];
    /// `capsule.items.CapsuleItem.onEntityItemUpdate(ItemStack, ItemEntity)`: integers
    pub const CAPSULE_ITEM_ON_ENTITY_ITEM_UPDATE_INTS: &[i64] = &[2];
    /// `capsule.items.CapsuleItem.getOccupiedSourcePos(ItemStack)`: integers
    pub const CAPSULE_ITEM_GET_OCCUPIED_SOURCE_POS_INTS: &[i64] = &[10];
    /// `capsule.items.CapsuleItem.getColorFromItemstack(ItemStack, I)`: integers
    pub const CAPSULE_ITEM_GET_COLOR_FROM_ITEMSTACK_INTS: &[i64] = &[16777215, 8176874, 2, 3912700];
    /// `capsule.items.CapsuleItems.compare(ItemStack)`: floats
    pub const CAPSULE_ITEMS_COMPARE_FLOATS: &[f64] = &[1.0e-12];
    /// `capsule.items.CapsuleItems.getUpgradedCapsule(ItemStack, I)`: integers
    pub const CAPSULE_ITEMS_GET_UPGRADED_CAPSULE_INTS: &[i64] = &[2];
    /// `capsule.loot.CapsuleLootEntry.findConfiguredWeight(String)`: integers
    pub const CAPSULE_LOOT_ENTRY_FIND_CONFIGURED_WEIGHT_INTS: &[i64] = &[3];
    /// `capsule.network.CapsuleContentPreviewAnswerToClient.<init>(FriendlyByteBuf)`: integers
    pub const CAPSULE_CONTENT_PREVIEW_ANSWER_TO_CLIENT_INIT_INTS: &[i64] = &[32767];
    /// `capsule.network.CapsuleContentPreviewAnswerToClient.toBytes(FriendlyByteBuf)`: integers
    pub const CAPSULE_CONTENT_PREVIEW_ANSWER_TO_CLIENT_TO_BYTES_INTS: &[i64] = &[32767];
    /// `capsule.network.CapsuleContentPreviewQueryToServer.<init>(FriendlyByteBuf)`: integers
    pub const CAPSULE_CONTENT_PREVIEW_QUERY_TO_SERVER_INIT_INTS: &[i64] = &[32767];
    /// `capsule.network.CapsuleFullContentAnswerToClient.<init>(FriendlyByteBuf)`: integers
    pub const CAPSULE_FULL_CONTENT_ANSWER_TO_CLIENT_INIT_INTS: &[i64] = &[32767];
    /// `capsule.network.CapsuleFullContentAnswerToClient.toBytes(FriendlyByteBuf)`: integers
    pub const CAPSULE_FULL_CONTENT_ANSWER_TO_CLIENT_TO_BYTES_INTS: &[i64] = &[1000000];
    /// `capsule.network.CapsuleThrowQueryToServer.lambda$onServer$1(ItemStack, ServerPlayer, ServerLevel)`: integers
    pub const CAPSULE_THROW_QUERY_TO_SERVER_LAMBDA_ON_SERVER_1_INTS: &[i64] = &[2];
    /// `capsule.network.CapsuleThrowQueryToServer.lambda$onServer$1(ItemStack, ServerPlayer, ServerLevel)`: floats
    pub const CAPSULE_THROW_QUERY_TO_SERVER_LAMBDA_ON_SERVER_1_FLOATS: &[f64] = &[0.4, 0.1];
    /// `capsule.network.CapsuleThrowQueryToServer.lambda$onServer$0(BlockPos, I, ServerPlayer)`: integers
    pub const CAPSULE_THROW_QUERY_TO_SERVER_LAMBDA_ON_SERVER_0_INTS: &[i64] = &[200];
    /// `capsule.network.LabelEditedMessageToServer.<init>(FriendlyByteBuf)`: integers
    pub const LABEL_EDITED_MESSAGE_TO_SERVER_INIT_INTS: &[i64] = &[32767];
    /// `capsule.plugins.jei.CapsulePlugin.registerRecipes(IRecipeRegistration)`: integers
    pub const CAPSULE_PLUGIN_REGISTER_RECIPES_INTS: &[i64] = &[8, 3, 2];
    /// `capsule.recipes.BlueprintCapsuleRecipe.assemble(CraftingContainer, RegistryAccess)`: integers
    pub const BLUEPRINT_CAPSULE_RECIPE_ASSEMBLE_INTS: &[i64] = &[16777215];
    /// `capsule.recipes.BlueprintChangeRecipe.getResultItem(RegistryAccess)`: integers
    pub const BLUEPRINT_CHANGE_RECIPE_GET_RESULT_ITEM_INTS: &[i64] = &[3949738];
    /// `capsule.recipes.BlueprintChangeRecipe.canCraftInDimensions(I, I)`: integers
    pub const BLUEPRINT_CHANGE_RECIPE_CAN_CRAFT_IN_DIMENSIONS_INTS: &[i64] = &[2];
    /// `capsule.recipes.DyeCapsuleRecipe.dyeItem(ItemStack, List)`: integers
    pub const DYE_CAPSULE_RECIPE_DYE_ITEM_INTS: &[i64] = &[3, 16, 255, 8, 2];
    /// `capsule.recipes.DyeCapsuleRecipe.dyeItem(ItemStack, List)`: floats
    pub const DYE_CAPSULE_RECIPE_DYE_ITEM_FLOATS: &[f64] = &[255.0];
    /// `capsule.recipes.DyeCapsuleRecipe.canCraftInDimensions(I, I)`: integers
    pub const DYE_CAPSULE_RECIPE_CAN_CRAFT_IN_DIMENSIONS_INTS: &[i64] = &[2];
    /// `capsule.recipes.PrefabsBlueprintAggregatorRecipe$PrefabsBlueprintCapsuleRecipe.<init>(ResourceLocation, JsonObject, Triple)`: integers
    pub const PREFABS_BLUEPRINT_CAPSULE_RECIPE_INIT_INTS: &[i64] = &[4, 2];
    /// `capsule.recipes.PrefabsBlueprintAggregatorRecipe$PrefabsBlueprintCapsuleRecipe.FixIngredient(JsonObject, StructureSaver$ItemStackKey, String)`: integers
    pub const PREFABS_BLUEPRINT_CAPSULE_RECIPE_FIX_INGREDIENT_INTS: &[i64] = &[3];
    /// `capsule.recipes.PrefabsBlueprintAggregatorRecipe$PrefabsBlueprintCapsuleRecipe.<init>(ResourceLocation, ShapedRecipe)`: integers
    pub const PREFABS_BLUEPRINT_CAPSULE_RECIPE_INIT_INTS_2: &[i64] = &[4, 2];
    /// `capsule.recipes.PrefabsBlueprintAggregatorRecipe.canCraftInDimensions(I, I)`: integers
    pub const PREFABS_BLUEPRINT_AGGREGATOR_RECIPE_CAN_CRAFT_IN_DIMENSIONS_INTS: &[i64] = &[2];
    /// `capsule.recipes.UpgradeCapsuleRecipe.assemble(CraftingContainer, RegistryAccess)`: integers
    pub const UPGRADE_CAPSULE_RECIPE_ASSEMBLE_INTS: &[i64] = &[2, 255];
    /// `capsule.recipes.UpgradeCapsuleRecipe.canCraftInDimensions(I, I)`: integers
    pub const UPGRADE_CAPSULE_RECIPE_CAN_CRAFT_IN_DIMENSIONS_INTS: &[i64] = &[2];
    /// `capsule.structure.CapsuleTemplate$BasicPalette.<init>()`: integers
    pub const BASIC_PALETTE_INIT_INTS: &[i64] = &[16];
    /// `capsule.structure.CapsuleTemplate.processEntityInfos(CapsuleTemplate, LevelAccessor, BlockPos, StructurePlaceSettings, List)`: integers
    pub const CAPSULE_TEMPLATE_PROCESS_ENTITY_INFOS_INTS: &[i64] = &[2];
    /// `capsule.structure.CapsuleTemplate.save(CompoundTag)`: integers
    pub const CAPSULE_TEMPLATE_SAVE_INTS: &[i64] = &[3, 2];
    /// `capsule.structure.CapsuleTemplate.load(CompoundTag, String)`: integers
    pub const CAPSULE_TEMPLATE_LOAD_INTS: &[i64] = &[3, 2, 10, 9, 6];
    /// `capsule.structure.CapsuleTemplate.loadPalette(ListTag, ListTag)`: integers
    pub const CAPSULE_TEMPLATE_LOAD_PALETTE_INTS: &[i64] = &[3, 2];
    /// `capsule.structure.CapsuleTemplate.spawnBlocksAndEntities(ServerLevelAccessor, BlockPos, StructurePlaceSettings, Map, List, List)`: integers
    pub const CAPSULE_TEMPLATE_SPAWN_BLOCKS_AND_ENTITIES_INTS: &[i64] = &[2, 2147483647, -2147483648, 20, 5, 3, 4, -2, 16];
    /// `capsule.structure.CapsuleTemplate.calculateDeployPositions(Level, BlockPos, StructurePlaceSettings)`: integers
    pub const CAPSULE_TEMPLATE_CALCULATE_DEPLOY_POSITIONS_INTS: &[i64] = &[2];
    /// `capsule.structure.CapsuleTemplate.readSchematic(CompoundTag)`: integers
    pub const CAPSULE_TEMPLATE_READ_SCHEMATIC_INTS: &[i64] = &[3, 7, 99, 500, 4095, 10, 6, 2];
    /// `capsule.structure.CapsuleTemplate.getSchematicBlockEntities(CompoundTag, I)`: integers
    pub const CAPSULE_TEMPLATE_GET_SCHEMATIC_BLOCK_ENTITIES_INTS: &[i64] = &[10, 2, 3];
    /// `capsule.structure.CapsuleTemplate.getSchematicBlocks(CompoundTag, I, [Lnet/minecraft/world/level/block/state/BlockState;, I, I, I)`: integers
    pub const CAPSULE_TEMPLATE_GET_SCHEMATIC_BLOCKS_INTS: &[i64] = &[2, 3, 127, 7, 5, 128, 255, 240, 4, 15, 8];
    /// `capsule.structure.CapsuleTemplate.getSchematicBlocks(CompoundTag, I, [Lnet/minecraft/world/level/block/state/BlockState;, I, I, I)`: floats
    pub const CAPSULE_TEMPLATE_GET_SCHEMATIC_BLOCKS_FLOATS: &[f64] = &[2.0];
    /// `capsule.structure.CapsuleTemplate.readSchematicPalette(CompoundTag, I, I)`: integers
    pub const CAPSULE_TEMPLATE_READ_SCHEMATIC_PALETTE_INTS: &[i64] = &[10, 2, 3];
    /// `capsule.structure.CapsuleTemplateManager.readFromNBT(CompoundTag, String)`: integers
    pub const CAPSULE_TEMPLATE_MANAGER_READ_FROM_NBT_INTS: &[i64] = &[99, 500];

    // ---- pure static methods translated from their bytecode (tools/pure_java.py): each one gave exactly
    // the JVM's result (JBR 17, bit for bit, NaN == NaN, exceptions == panics) on every input of the proof.
    /// `capsule.client.render.vbo.CustomBufferBuilder.roundUpPositive(I)`: equal to the JVM on 3000 inputs.
    pub fn custom_buffer_builder_round_up_positive(p0: i32) -> i32 {
        #![allow(unused_mut, unused_assignments, unused_variables, unreachable_code, clippy::all)]

        #[allow(dead_code)]
        fn jv_minf(a: f32, b: f32) -> f32 { if a.is_nan() { a } else if a == 0.0 && b == 0.0 && b.is_sign_negative() { b } else if a <= b { a } else { b } }
        #[allow(dead_code)]
        fn jv_maxf(a: f32, b: f32) -> f32 { if a.is_nan() { a } else if a == 0.0 && b == 0.0 && a.is_sign_negative() { b } else if a >= b { a } else { b } }
        #[allow(dead_code)]
        fn jv_mind(a: f64, b: f64) -> f64 { if a.is_nan() { a } else if a == 0.0 && b == 0.0 && b.is_sign_negative() { b } else if a <= b { a } else { b } }
        #[allow(dead_code)]
        fn jv_maxd(a: f64, b: f64) -> f64 { if a.is_nan() { a } else if a == 0.0 && b == 0.0 && a.is_sign_negative() { b } else if a >= b { a } else { b } }
        #[allow(dead_code)]
        fn jv_floordiv_i(a: i32, b: i32) -> i32 { let q = a.wrapping_div(b); if (a.wrapping_rem(b) != 0) && ((a ^ b) < 0) { q - 1 } else { q } }
        #[allow(dead_code)]
        fn jv_floormod_i(a: i32, b: i32) -> i32 { a.wrapping_sub(jv_floordiv_i(a, b).wrapping_mul(b)) }
        #[allow(dead_code)]
        fn jv_cmpf(a: f32, b: f32, nan: i32) -> i32 { if a > b { 1 } else if a == b { 0 } else if a < b { -1 } else { nan } }
        #[allow(dead_code)]
        fn jv_cmpd(a: f64, b: f64, nan: i32) -> i32 { if a > b { 1 } else if a == b { 0 } else if a < b { -1 } else { nan } }
        #[allow(dead_code)]
        fn jv_idiv(a: i32, b: i32) -> i32 { if b == 0 { panic!("/ by zero") } a.wrapping_div(b) }
        #[allow(dead_code)]
        fn jv_irem(a: i32, b: i32) -> i32 { if b == 0 { panic!("/ by zero") } a.wrapping_rem(b) }
        #[allow(dead_code)]
        fn jv_ldiv(a: i64, b: i64) -> i64 { if b == 0 { panic!("/ by zero") } a.wrapping_div(b) }
        #[allow(dead_code)]
        fn jv_lrem(a: i64, b: i64) -> i64 { if b == 0 { panic!("/ by zero") } a.wrapping_rem(b) }
        let mut l0i: i32 = p0;
        let mut l1i: i32 = 0;
        let mut l2i: i32 = 0;
        let mut s0i: i32 = 0;
        let mut s1i: i32 = 0;
        let mut pc: u32 = 0;
        loop {
            match pc {
                0 => {
                    s0i = 2097152i32;
                    l1i = s0i;
                    s0i = l0i;
                    if s0i != 0 { pc = 9; continue; }
                    pc = 7;
                }
                7 => {
                    s0i = l1i;
                    return s0i;
                }
                9 => {
                    s0i = l0i;
                    if s0i >= 0 { pc = 17; continue; }
                    pc = 13;
                }
                13 => {
                    s0i = l1i;
                    s1i = -1;
                    s0i = s0i.wrapping_mul(s1i);
                    l1i = s0i;
                    pc = 17;
                }
                17 => {
                    s0i = l0i;
                    s1i = l1i;
                    s0i = jv_irem(s0i, s1i);
                    l2i = s0i;
                    s0i = l2i;
                    if s0i != 0 { pc = 29; continue; }
                    pc = 25;
                }
                25 => {
                    s0i = l0i;
                    pc = 34; continue;
                }
                29 => {
                    s0i = l0i;
                    s1i = l1i;
                    s0i = s0i.wrapping_add(s1i);
                    s1i = l2i;
                    s0i = s0i.wrapping_sub(s1i);
                    pc = 34;
                }
                34 => {
                    return s0i;
                }
                _ => unreachable!(),
            }
        }
    }
}
