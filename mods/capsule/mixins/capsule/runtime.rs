// Capsule inside Horizon's game (nightly 142388e1c). The rules are the engine (src/lib.rs); this part reads and
// changes the world. Everything that changes the world runs where the world is: on the integrated server in single
// player (the window is its guest; the right click arrives in the guest's turn), never on a remote client.
// Templates are vanilla structure files in `<world>/capsules/<name>.nbt`, like the original's CapsuleTemplateManager.
#[allow(unused_imports)]
use super::{Game, Result, lan::LanHost};
#[allow(dead_code)]
mod engine {
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


/// The original's plain data, read from the 1.20.1 jar by tools/date_jar.py: constants, enum values, static
/// tables, numbers given to builders, constant getters, chances, the numbers of each method, and pure static
/// methods translated from bytecode (each checked against the JVM). Use `date_jar::NAME` and cite it; delete what
/// the port does not need.
#[allow(dead_code, non_upper_case_globals, non_camel_case_types, non_snake_case, clippy::all)]
pub mod date_jar {
    // Data read from the 1.20.1 jar `Capsule-1.20.1-8.0.105.jar` by tools/date_jar.py (first pass, generated).
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

}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "capsule";

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
// kit_notice (tools/kit): a line over the hotbar (action bar) for whoever acts, as fallingtree
// does: a LAN guest gets a `SystemChat { overlay: true }` packet, the host the game's own
// `message` line. Pasted by `tools/hnmod.py bundle` in place of `include!("kit_notice.rs");`.
// Use: `kit_notice::action_bar(self, "Tree too big".to_owned())`, `kit_notice::chat(self, ..)`.
#[allow(dead_code)]
mod kit_notice {
    use crate::game::Game;
    use horizon_protocol::packets::play::{Clientbound, SystemChat};

    fn send(g: &mut Game<'_>, text: String, overlay: bool) {
        if g.lan_acting.is_some() {
            g.guest_packets.push(Clientbound::SystemChat(SystemChat {
                content: horizon_protocol::text::literal(&text),
                overlay,
            }));
        } else {
            // The host's window has one line over the hotbar for both (see `chat`).
            g.message = Some((text, std::time::Instant::now()));
        }
    }
    /// `player.displayClientMessage(text, true)`: the action bar.
    pub(super) fn action_bar(g: &mut Game<'_>, text: String) {
        send(g, text, true);
    }
    /// `player.displayClientMessage(text, false)`. For a guest this is a chat line; the host
    /// gets the same action-bar line (this pin has no plain host-side chat append without
    /// going through `chat.rs` internals; see TRADUCATOR.md).
    pub(super) fn chat(g: &mut Game<'_>, text: String) {
        send(g, text, false);
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
use horizon_core::position::BlockPos;
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::collections::{BTreeMap, HashMap, HashSet};

const CAPSULE: &str = "capsule:capsule";
const MARKER: &str = "capsule:capsulemarker";
const CLICK_ON: &str = "minecraft:block.stone_button.click_on";
const CLICK_OFF: &str = "minecraft:block.stone_button.click_off";
const ARROW_SHOOT: &str = "minecraft:entity.arrow.shoot";
/// `CapsuleLootEntry.COLOR_PALETTE`.
const PALETTE: [i32; 11] = [0xCCCCCC, 0x549b57, 0xe08822, 0x5e8eb7, 0x6c6c6c, 0xbd5757, 0x99c33d, 0x4a4cba, 0x7b2e89, 0x95d5e7, 0xffffff];

thread_local! {
    /// The world's counter and the players who got their starter (`CapsuleSavedData`, `capsule:receivedStarter`):
    /// `<world>/data/capsule/capsule.txt`, for the whole world (all dimensions).
    static SAVED: std::cell::RefCell<Option<(std::path::PathBuf, engine::State2, bool)>> = const { std::cell::RefCell::new(None) };
    /// Thrown capsules: the last horizontal speed (for the wall hit) and whether the lifetime was extended.
    static THROWN: std::cell::RefCell<HashMap<u32, ([f64; 2], bool)>> = std::cell::RefCell::new(HashMap::new());
}

// ---- the stack ----

fn cd(stack: &ItemStack) -> Option<&Compound> {
    match stack.data()?.other("minecraft:custom_data")? {
        Tag::Compound(c) => Some(c),
        _ => None,
    }
}
fn int(c: &Compound, k: &str) -> Option<i32> {
    match c.get(&JavaString::from(k))? {
        Tag::Int(v) => Some(*v),
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Byte(v) => Some(i32::from(*v)),
        Tag::Long(v) => Some(*v as i32),
        _ => None,
    }
}
fn long(c: &Compound, k: &str) -> Option<i64> {
    match c.get(&JavaString::from(k))? {
        Tag::Long(v) => Some(*v),
        Tag::Int(v) => Some(i64::from(*v)),
        _ => None,
    }
}
fn text(c: &Compound, k: &str) -> Option<String> {
    match c.get(&JavaString::from(k))? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}
fn sub<'a>(c: &'a Compound, k: &str) -> Option<&'a Compound> {
    match c.get(&JavaString::from(k))? {
        Tag::Compound(s) => Some(s),
        _ => None,
    }
}
fn s(v: &str) -> Tag {
    Tag::String(v.into())
}

/// The capsule's tag (1.20.1 names), as the engine sees it.
fn read(stack: &ItemStack) -> engine::Capsule {
    let empty = Compound::new();
    let c = cd(stack).unwrap_or(&empty);
    let spawn = sub(c, "spawnPosition").and_then(|p| {
        Some(([int(p, "x")?, int(p, "y")?, int(p, "z")?], text(p, "dim").unwrap_or_else(|| "minecraft:overworld".into())))
    });
    engine::Capsule {
        state: int(c, "state").unwrap_or(0),
        size: int(c, "size").unwrap_or(1),
        material_color: int(c, "color"),
        base_color: stack.data().and_then(|d| d.dyed_color),
        structure: text(c, "structureName"),
        label: text(c, "label"),
        author: text(c, "author"),
        overpowered: int(c, "overpowered") == Some(1),
        one_use: int(c, "oneUse").is_some_and(|v| v != 0),
        is_reward: int(c, "isReward").is_some_and(|v| v != 0),
        blueprint: c.contains_key(&JavaString::from("sourceInventory")),
        spawn,
        timer_start: sub(c, "activetimer").and_then(|t| long(t, "starttime")),
        deploy_at: long(c, "deployAt"),
        undeploy_at: long(c, "undeployAt").unwrap_or(0),
        upgraded: int(c, "upgraded").unwrap_or(0),
        y_offset: int(c, "yOffset").unwrap_or(0),
    }
}

/// Writes the engine's capsule back into the stack's tag (unknown keys stay), plus what the 26.3 item model reads
/// (`custom_model_data`: the state's model, the material colour, the third layer's colour) and the name
/// (`item_name`, `CapsuleItem.getName`).
fn write(game: &Game<'_>, stack: &mut ItemStack, c: &engine::Capsule) {
    let mut tag = cd(stack).cloned().unwrap_or_default();
    let mut put = |k: &str, v: Option<Tag>| match v {
        Some(v) => {
            tag.insert(JavaString::from(k), v);
        }
        None => {
            tag.remove(&JavaString::from(k));
        }
    };
    put("state", Some(Tag::Int(c.state)));
    put("size", Some(Tag::Int(c.size())));
    put("color", c.material_color.map(Tag::Int));
    put("structureName", c.structure.as_deref().map(s));
    put("label", c.label.as_deref().map(s));
    put("author", c.author.as_deref().map(s));
    put("overpowered", c.overpowered.then_some(Tag::Byte(1)));
    put("oneUse", c.one_use.then_some(Tag::Byte(1)));
    put("isReward", c.is_reward.then_some(Tag::Byte(1)));
    put(
        "spawnPosition",
        c.spawn.as_ref().map(|(p, d)| {
            Tag::compound([("x", Tag::Int(p[0])), ("y", Tag::Int(p[1])), ("z", Tag::Int(p[2])), ("dim", s(d))])
        }),
    );
    put("activetimer", c.timer_start.map(|t| Tag::compound([("starttime", Tag::Long(t))])));
    put("deployAt", c.deploy_at.map(Tag::Long));
    put("undeployAt", (c.undeploy_at != 0).then_some(Tag::Long(c.undeploy_at)));
    put("upgraded", (c.upgraded != 0).then_some(Tag::Int(c.upgraded)));
    put("yOffset", (c.y_offset != 0).then_some(Tag::Int(c.y_offset)));
    let d = stack.data_mut();
    d.set_other("minecraft:custom_data", Some(Tag::Compound(tag)));
    // `getColorFromItemstack`: layer 1 the material colour (a deployed blueprint 0x7CC4EA), layer 2 white (a
    // blueprint 0x3BB3FC); layer 0 is the dye (`display.color`, now `dyed_color`).
    let deployed_blueprint = c.blueprint && c.has(engine::State::Deployed);
    let model = if deployed_blueprint { "deployed" } else { c.state().model() };
    let layer1 = if deployed_blueprint { 0x7CC4EA } else { c.material_color.unwrap_or(0xFFFFFF) };
    let layer2 = if c.blueprint { 0x3BB3FC } else { 0xFFFFFF };
    d.set_other(
        "minecraft:custom_model_data",
        Some(Tag::compound([
            ("strings", Tag::List(vec![s(model)])),
            ("colors", Tag::List(vec![Tag::Int(layer1), Tag::Int(layer2)])),
        ])),
    );
    d.set_other("minecraft:item_name", Some(name_component(game, c)));
}

/// `CapsuleItem.getName` as a text component: `[state ][content ]name`.
fn name_component(_game: &Game<'_>, c: &engine::Capsule) -> Tag {
    let (state, content, name) = engine::name_parts(c);
    let tr = |k: &str| Tag::compound([("translate", s(k))]);
    let mut extra = Vec::new();
    if let Some(st) = state {
        extra.push(Tag::compound([("translate", s(st)), ("color", s("dark_green"))]));
        extra.push(s(" "));
    }
    match content {
        Some(engine::Content::Label(l)) => {
            extra.push(s("«"));
            extra.push(Tag::compound([("text", s(&l)), ("italic", Tag::Byte(1))]));
            extra.push(s("»"));
            extra.push(s(" "));
        }
        Some(engine::Content::Empty) => {
            extra.push(tr("items.capsule.content_empty"));
            extra.push(s(" "));
        }
        Some(engine::Content::Unlabeled) => {
            extra.push(tr("items.capsule.content_unlabeled"));
            extra.push(s(" "));
        }
        None => {}
    }
    extra.push(tr(name));
    Tag::compound([("text", s("")), ("extra", Tag::List(extra))])
}

fn is_capsule(stack: &ItemStack) -> bool {
    &*stack.item == CAPSULE
}

fn dir_of(name: &str) -> Option<[i32; 3]> {
    Some(match name {
        "down" => [0, -1, 0],
        "up" => [0, 1, 0],
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => return None,
    })
}

/// What a template holds, read back.
struct Template {
    size: [i32; 3],
    /// (relative position, state, block entity nbt without x/y/z).
    blocks: Vec<([i32; 3], horizon_world::registry::BlockStateId, Option<Compound>)>,
    /// (relative position, block position, nbt).
    entities: Vec<([f64; 3], [i32; 3], Compound)>,
    occupied: Option<BTreeMap<engine::Pos, String>>,
    author: Option<String>,
}

impl Game<'_> {
    // ---- files ----

    fn capsule_dir(&self) -> Option<std::path::PathBuf> {
        (self.remote.is_none() && !self.world_root.as_os_str().is_empty()).then(|| self.world_root.join("capsules"))
    }
    fn capsule_template_path(&self, name: &str) -> Option<std::path::PathBuf> {
        // Names come from `getUniqueName` (id characters only): never a path out of the folder.
        if name.is_empty() || name.contains("..") || name.contains(':') || name.starts_with('/') || name.contains('\\') {
            return None;
        }
        Some(self.capsule_dir()?.join(format!("{name}.nbt")))
    }
    fn capsule_saved<R>(&mut self, f: impl FnOnce(&mut engine::State2) -> R) -> Option<R> {
        let root = self.world_root.clone();
        if self.remote.is_some() || root.as_os_str().is_empty() {
            return None;
        }
        let fresh = SAVED.with(|cell| cell.borrow().as_ref().is_none_or(|(r, _, _)| *r != root));
        if fresh {
            let text = kit_state::read(self, "capsule.txt");
            SAVED.with(|cell| *cell.borrow_mut() = Some((root.clone(), engine::State2::load(text), false)));
        }
        SAVED.with(|cell| {
            let mut cell = cell.borrow_mut();
            let (_, st, dirty) = cell.as_mut()?;
            let before = st.clone();
            let r = f(st);
            if *st != before {
                *dirty = true;
            }
            Some(r)
        })
    }
    /// HEAD `save_game_metadata`.
    pub(super) fn capsule_save(&mut self) {
        let snapshot = SAVED.with(|cell| {
            let mut cell = cell.borrow_mut();
            match cell.as_mut() {
                Some((root, st, dirty)) if *dirty && *root == self.world_root => {
                    *dirty = false;
                    Some(st.save())
                }
                _ => None,
            }
        });
        if let Some(text) = snapshot {
            kit_state::write(self, "capsule.txt", &text);
        }
    }
    /// HEAD `load_world_state`: another world opens.
    pub(super) fn capsule_world_opened() {
        SAVED.with(|c| *c.borrow_mut() = None);
        THROWN.with(|t| t.borrow_mut().clear());
    }

    /// Reads a template file (vanilla structure NBT: `Name`/`Properties` or `id`/`properties` palettes).
    fn capsule_read_template(&self, path: &std::path::Path) -> Option<Template> {
        let tag = horizon_client::save::read_dat(path)?;
        let Tag::Compound(root) = tag else { return None };
        let list = |c: &Compound, k: &str| match c.get(&JavaString::from(k)) {
            Some(Tag::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        let ints = |t: Option<&Tag>| -> [i32; 3] {
            match t {
                Some(Tag::List(l)) if l.len() == 3 => std::array::from_fn(|i| match &l[i] {
                    Tag::Int(v) => *v,
                    _ => 0,
                }),
                Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
                _ => [0; 3],
            }
        };
        let registry = self.pack.assets.registry;
        let mut palette = Vec::new();
        let first_palette = match root.get(&JavaString::from("palettes")) {
            Some(Tag::List(ps)) => match ps.first() {
                Some(Tag::List(p)) => p.clone(),
                _ => Vec::new(),
            },
            _ => list(&root, "palette"),
        };
        for entry in first_palette {
            let Tag::Compound(e) = entry else { continue };
            let name = text(&e, "Name").or_else(|| text(&e, "id")).unwrap_or_else(|| "minecraft:air".into());
            let props = sub(&e, "Properties").or_else(|| sub(&e, "properties"));
            let id = registry.default_state(&name).and_then(|d| {
                let mut st = registry.state(d)?.clone();
                if let Some(props) = props {
                    for (k, v) in props {
                        let (k, Tag::String(v)) = (k.to_string_lossy(), v) else { continue };
                        if st.properties.contains_key(&k) {
                            st.properties.insert(k, v.to_string_lossy());
                        }
                    }
                }
                registry.resolve(&st).ok()
            });
            palette.push(id.unwrap_or(self.res.facts().air));
        }
        let mut blocks = Vec::new();
        for b in list(&root, "blocks") {
            let Tag::Compound(b) = b else { continue };
            let Some(state) = int(&b, "state").and_then(|i| palette.get(i as usize).copied()) else { continue };
            let nbt = sub(&b, "nbt").cloned();
            blocks.push((ints(b.get(&JavaString::from("pos"))), state, nbt));
        }
        let mut entities = Vec::new();
        for e in list(&root, "entities") {
            let Tag::Compound(e) = e else { continue };
            let pos = match e.get(&JavaString::from("pos")) {
                Some(Tag::List(l)) if l.len() == 3 => std::array::from_fn(|i| match &l[i] {
                    Tag::Double(v) => f64::from_bits(*v),
                    _ => 0.0,
                }),
                _ => [0.0; 3],
            };
            let Some(nbt) = sub(&e, "nbt").cloned() else { continue };
            entities.push((pos, ints(e.get(&JavaString::from("blockPos"))), nbt));
        }
        let occupied = match root.get(&JavaString::from("capsule_occupiedSources")) {
            Some(Tag::List(l)) => Some(
                l.iter()
                    .filter_map(|t| match t {
                        Tag::Compound(c) => Some((engine::pos_of_long(long(c, "pos")?), text(c, "block")?)),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        };
        Some(Template {
            size: ints(root.get(&JavaString::from("size"))),
            blocks,
            entities,
            occupied,
            author: text(&root, "author"),
        })
    }

    /// Writes a template (`CapsuleTemplate.save`): size, one palette, blocks, entities, DataVersion, the occupied
    /// positions of a deployment (by block name), author.
    fn capsule_write_template(&self, path: &std::path::Path, t: &Template) -> bool {
        let registry = self.pack.assets.registry;
        let mut index: HashMap<u32, i32> = HashMap::new();
        let mut palette = Vec::new();
        let mut blocks = Vec::new();
        for (pos, state, nbt) in &t.blocks {
            let i = *index.entry(state.0).or_insert_with(|| {
                let st = registry.state(*state);
                let mut e = Compound::new();
                e.insert("Name".into(), s(st.map_or("minecraft:air", |b| b.name.as_str())));
                if let Some(st) = st
                    && !st.properties.is_empty()
                {
                    let mut p = Compound::new();
                    for (k, v) in &st.properties {
                        p.insert(k.as_str().into(), s(v));
                    }
                    e.insert("Properties".into(), Tag::Compound(p));
                }
                palette.push(Tag::Compound(e));
                palette.len() as i32 - 1
            });
            let mut b = Compound::new();
            b.insert("pos".into(), Tag::List(pos.iter().map(|v| Tag::Int(*v)).collect()));
            b.insert("state".into(), Tag::Int(i));
            if let Some(n) = nbt {
                b.insert("nbt".into(), Tag::Compound(n.clone()));
            }
            blocks.push(Tag::Compound(b));
        }
        let entities = t
            .entities
            .iter()
            .map(|(pos, bp, nbt)| {
                Tag::compound([
                    ("pos", Tag::List(pos.iter().map(|v| Tag::Double(v.to_bits())).collect())),
                    ("blockPos", Tag::List(bp.iter().map(|v| Tag::Int(*v)).collect())),
                    ("nbt", Tag::Compound(nbt.clone())),
                ])
            })
            .collect();
        let mut root = Compound::new();
        root.insert("size".into(), Tag::List(t.size.iter().map(|v| Tag::Int(*v)).collect()));
        root.insert("palette".into(), Tag::List(palette));
        root.insert("blocks".into(), Tag::List(blocks));
        root.insert("entities".into(), Tag::List(entities));
        root.insert("DataVersion".into(), Tag::Int(horizon_world::DATA_VERSION));
        if let Some(o) = &t.occupied {
            let list = o
                .iter()
                .map(|(p, b)| Tag::compound([("pos", Tag::Long(engine::pos_to_long(*p))), ("block", s(b))]))
                .collect();
            root.insert("capsule_occupiedSources".into(), Tag::List(list));
        }
        root.insert("author".into(), s(t.author.as_deref().unwrap_or("?")));
        match horizon_client::save::write_dat(path, Tag::Compound(root)) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("capsule: template {} not written: {e}", path.display());
                false
            }
        }
    }

    // ---- the world ----

    fn capsule_block_name(&self, p: engine::Pos) -> Option<String> {
        kit_blocks::name_at(self, p).map(|n| n.to_string())
    }
    fn capsule_is_air(&self, name: &str) -> bool {
        matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
    }
    /// The full block entity data at `p` (`blockentity.saveWithId()` without x/y/z): a modelled block entity with
    /// its custom name, else the raw tag Horizon keeps for block entities it does not model.
    fn capsule_block_entity_nbt(&self, p: engine::Pos) -> Option<Compound> {
        let key = (p[0], p[1], p[2]);
        let mut tag = if let Some(e) = self.world.block_entities.get(&key) {
            let name = self.capsule_block_name(p).unwrap_or_default();
            let mut t = horizon_client::save::encode_block_entity(key, e, &name);
            if let Some(chunk) = self.world.chunks.get(&(p[0] >> 4, p[2] >> 4))
                && let Some((_, n)) = chunk.block_names.iter().find(|(q, _)| *q == key)
                && let Tag::Compound(m) = &mut t
            {
                m.insert("CustomName".into(), s(n));
            }
            t
        } else {
            let chunk = self.world.chunks.get(&(p[0] >> 4, p[2] >> 4))?;
            chunk.saved.as_ref()?.block_entities.iter().find(|t| raw_pos(t) == Some(p))?.clone()
        };
        if let Tag::Compound(m) = &mut tag {
            for k in ["x", "y", "z"] {
                m.remove(&JavaString::from(k));
            }
            return Some(m.clone());
        }
        None
    }
    /// Removes the block entity at `p` without spilling (its data is in the template already).
    fn capsule_take_block_entity(&mut self, p: engine::Pos) {
        let key = (p[0], p[1], p[2]);
        self.world.block_entities.remove(&key);
        let ck = (p[0] >> 4, p[2] >> 4);
        let touched = self.world.chunks.get(&ck).is_some_and(|c| {
            c.block_names.iter().any(|(q, _)| *q == key)
                || c.saved.as_ref().is_some_and(|x| x.block_entities.iter().any(|t| raw_pos(t) == Some(p)))
        });
        if !touched {
            return;
        }
        if let Some(chunk) = self.world.chunks.get_mut(&ck) {
            let chunk = std::sync::Arc::make_mut(chunk);
            chunk.block_names.retain(|(q, _)| *q != key);
            if let Some(saved) = chunk.saved.as_mut() {
                let extras = std::sync::Arc::make_mut(saved);
                extras.block_entities.retain(|t| raw_pos(t) != Some(p));
            }
        }
    }
    /// Puts a template's block entity at `p`: decoded when Horizon models it, else kept raw for the chunk's save.
    fn capsule_put_block_entity(&mut self, p: engine::Pos, nbt: &Compound) {
        let mut tag = nbt.clone();
        tag.insert("x".into(), Tag::Int(p[0]));
        tag.insert("y".into(), Tag::Int(p[1]));
        tag.insert("z".into(), Tag::Int(p[2]));
        let name = text(&tag, "CustomName");
        let key = (p[0], p[1], p[2]);
        if let Some((_, e)) = horizon_client::save::decode_block_entity(&Tag::Compound(tag.clone())) {
            self.world.block_entities.insert(key, e);
            if let Some(n) = name
                && let Some(chunk) = self.world.chunks.get_mut(&(p[0] >> 4, p[2] >> 4))
            {
                let chunk = std::sync::Arc::make_mut(chunk);
                chunk.block_names.retain(|(q, _)| *q != key);
                chunk.block_names.push((key, n));
            }
        } else if let Some(chunk) = self.world.chunks.get_mut(&(p[0] >> 4, p[2] >> 4)) {
            let chunk = std::sync::Arc::make_mut(chunk);
            let saved = chunk.saved.get_or_insert_with(Default::default);
            let extras = std::sync::Arc::make_mut(saved);
            extras.block_entities.retain(|t| raw_pos(t) != Some(p));
            extras.block_entities.push(Tag::Compound(tag));
        }
    }
    fn capsule_set(&mut self, p: engine::Pos, state: horizon_world::registry::BlockStateId, flags: i32) {
        let at = BlockPos::new(p[0], p[1], p[2]);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, state, flags));
    }

    /// The non-living entities of a box, and armor stands (`snapshotNonLivingEntitiesFromWorld`): (target, nbt).
    fn capsule_entities_in(&mut self, lo: [f64; 3], hi: [f64; 3]) -> Vec<(super::commands::Target, Compound)> {
        use super::commands::Target;
        let mut targets: Vec<Target> = Vec::new();
        targets.extend(self.armor_stands.iter().map(|a| Target::Stand(a.id)));
        targets.extend(self.item_frames.iter().map(|f| Target::Frame(f.id)));
        targets.extend(self.paintings.iter().map(|p| Target::Painting(p.id)));
        targets.extend(self.boats.iter().map(|b| Target::Boat(b.id)));
        targets.extend(self.carts.iter().map(|c| Target::Cart(c.id)));
        targets.extend(self.others.iter().map(|o| Target::Other(o.id)));
        let mut out = Vec::new();
        for t in targets {
            let Some(Tag::Compound(nbt)) = self.entity_nbt(t) else { continue };
            let Some(p) = horizon_client::entity_nbt::position(&Tag::Compound(nbt.clone())) else { continue };
            if p.x >= lo[0] && p.x <= hi[0] && p.y >= lo[1] && p.y <= hi[1] && p.z >= lo[2] && p.z <= hi[2] {
                out.push((t, nbt));
            }
        }
        out
    }
    fn capsule_remove_entity(&mut self, t: super::commands::Target) {
        use super::commands::Target;
        match t {
            Target::Stand(id) => self.armor_stands.retain(|a| a.id != id),
            Target::Frame(id) => self.item_frames.retain(|f| f.id != id),
            Target::Painting(id) => self.paintings.retain(|p| p.id != id),
            Target::Boat(id) => self.boats.retain(|b| b.id != id),
            Target::Cart(id) => self.carts.retain(|c| c.id != id),
            Target::Other(id) => self.others.retain(|o| o.id != id),
            _ => {}
        }
    }
    /// `EntityType.loadEntityRecursive` + `addFreshEntity` (as `/summon` and `/place template` do).
    fn capsule_spawn_entity(&mut self, tag: Tag) {
        let Some(p) = horizon_client::entity_nbt::position(&tag) else { return };
        let key = ((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4);
        let mut waiting = std::mem::take(&mut self.world.loaded_entities);
        self.world.loaded_entities.push((key, vec![tag]));
        self.spawn_loaded_entities();
        waiting.append(&mut self.world.loaded_entities);
        self.world.loaded_entities = waiting;
    }

    /// A chat line (translated here, as the original's `Component.translatable` for the player who acts).
    fn capsule_tell(&mut self, key: &str, args: &[String]) {
        let pattern = self.pack.names.display_get(key).unwrap_or_else(|| key.to_owned());
        let mut out = String::new();
        let mut rest = pattern.as_str();
        let mut i = 0;
        while let Some(at) = rest.find("%s") {
            out.push_str(&rest[..at]);
            out.push_str(args.get(i).map_or("", String::as_str));
            i += 1;
            rest = &rest[at + 2..];
        }
        out.push_str(rest);
        kit_notice::chat(self, out);
    }

    // ---- capture and deploy ----

    /// `StructureSaver.undeploy` + `Capsule.captureAtPosition` / `resentToCapsule`: the area into the template
    /// `name`, written before anything is removed; then the blocks go (no drops, no neighbour updates: flag 2),
    /// block entities first, then the captured entities.
    fn capsule_capture(&mut self, c: &mut engine::Capsule, start: engine::Pos, name: &str, occupied: Option<&BTreeMap<engine::Pos, String>>, author: Option<String>) -> bool {
        let Some(path) = self.capsule_template_path(name) else { return false };
        let size = c.size();
        let overpowered = c.overpowered;
        let positions = {
            let at = |p: engine::Pos| self.capsule_block_name(p).unwrap_or_else(|| "minecraft:air".into());
            let air = |b: &str| self.capsule_is_air(b);
            let ex = |b: &str| engine::excluded(b, overpowered, kit_blocks::block_in_tag(self, "capsule:excluded", b));
            engine::capture_positions(start, size, at, air, ex, occupied)
        };
        let mut blocks = Vec::with_capacity(positions.len());
        for p in &positions {
            let state = self.world.block(p[0], p[1], p[2]);
            let rel = [p[0] - start[0], p[1] - start[1], p[2] - start[2]];
            blocks.push((rel, state, self.capsule_block_entity_nbt(*p)));
        }
        let lo = start.map(f64::from);
        let hi = [lo[0] + f64::from(size), lo[1] + f64::from(size), lo[2] + f64::from(size)];
        let entities = self.capsule_entities_in(lo, hi);
        let template = Template {
            size: [size; 3],
            blocks,
            entities: entities
                .iter()
                .map(|(_, nbt)| {
                    let p = horizon_client::entity_nbt::position(&Tag::Compound(nbt.clone())).map_or([0.0; 3], |v| [v.x, v.y, v.z]);
                    let rel = [p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]];
                    (rel, rel.map(|v| v.floor() as i32), nbt.clone())
                })
                .collect(),
            occupied: None,
            author,
        };
        if !self.capsule_write_template(&path, &template) {
            self.capsule_tell("capsule.error.technicalError", &[]);
            c.revert_from_activated();
            return false;
        }
        let air = self.res.facts().air;
        for p in &positions {
            self.capsule_take_block_entity(*p);
            self.capsule_set(*p, air, 2);
        }
        let _ = self.process_level_events();
        for (t, _) in entities {
            self.capsule_remove_entity(t);
        }
        true
    }

    /// `Capsule.captureAtPosition`: a new template for an empty capsule.
    fn capsule_capture_new(&mut self, c: &mut engine::Capsule, start: engine::Pos, actor: Option<String>) -> bool {
        let who = actor.clone().unwrap_or_else(|| "CapsuleMod".into());
        let Some(n) = self.capsule_saved(|s| s.next()) else { return false };
        let mut name = engine::unique_name(&who, n);
        while self.capsule_template_path(&name).is_some_and(|p| p.exists()) {
            let Some(n) = self.capsule_saved(|s| s.next()) else { return false };
            name = engine::unique_name(&who, n);
        }
        if self.capsule_capture(c, start, &name, None, actor) {
            c.state = engine::State::Linked.value();
            c.structure = Some(name);
            c.timer_start = None;
            true
        } else {
            c.revert_from_activated();
            false
        }
    }

    /// `Capsule.resentToCapsule`: the deployed content back into its template.
    fn capsule_undeploy(&mut self, c: &mut engine::Capsule) -> bool {
        let (Some((start, dim)), Some(name)) = (c.spawn.clone(), c.structure.clone()) else { return false };
        if dim != self.dim.id() {
            return false;
        }
        let Some(path) = self.capsule_template_path(&name) else { return false };
        let template = self.capsule_read_template(&path);
        let occupied = template.as_ref().and_then(|t| t.occupied.clone());
        let author = template.and_then(|t| t.author);
        if !self.capsule_capture(c, start, &name, occupied.as_ref(), author) {
            return false;
        }
        c.state = engine::State::Linked.value();
        c.spawn = None;
        true
    }

    /// `Capsule.deployCapsule` + `StructureSaver.deploy`: the template at the area above `anchor` (or around
    /// `deployAt`), merged with what is there (overridable blocks give way, others block the deploy).
    fn capsule_deploy(&mut self, c: &mut engine::Capsule, anchor: engine::Pos) -> bool {
        let extend = c.extend();
        let dest = engine::deploy_dest(anchor, extend, c.deploy_at.map(engine::pos_of_long));
        c.deploy_at = None;
        let Some(name) = c.structure.clone() else { return false };
        let Some(path) = self.capsule_template_path(&name) else { return false };
        let Some(mut template) = self.capsule_read_template(&path) else {
            self.capsule_tell("capsule.error.templateNotFound", &[name]);
            c.revert_from_activated();
            return false;
        };
        let size = c.size();
        let registry = self.pack.assets.registry;
        let by_pos: HashMap<engine::Pos, String> = template
            .blocks
            .iter()
            .map(|(rel, st, _)| {
                ([dest[0] + rel[0], dest[1] + rel[1], dest[2] + rel[2]], registry.state(*st).map_or_else(String::new, |b| b.name.clone()))
            })
            .collect();
        let checked = {
            let tat = |p: engine::Pos| by_pos.get(&p).cloned();
            let wat = |p: engine::Pos| self.capsule_block_name(p);
            let air = |b: &str| self.capsule_is_air(b);
            let over = |b: &str| kit_blocks::block_in_tag(self, "capsule:overridable", b);
            let mob = |p: engine::Pos| {
                let cell = horizon_core::geometry::Aabb::new(
                    horizon_core::geometry::Vec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])),
                    horizon_core::geometry::Vec3::new(f64::from(p[0] + 1), f64::from(p[1] + 1), f64::from(p[2] + 1)),
                );
                self.mobs.iter().find(|m| !m.removed && m.health > 0.0 && m.bounds().intersects(cell)).map(|m| {
                    let key = format!("entity.{}", m.kind.id.replace(':', "."));
                    self.pack.names.display_get(&key).unwrap_or_else(|| m.kind.id.clone())
                })
            };
            engine::check_destination(dest, size, tat, wat, air, over, mob)
        };
        let occupied = match checked {
            Ok(o) => o,
            Err(e) => {
                match e {
                    engine::DeployError::NotLoaded => self.capsule_tell("capsule.error.areaNotLoaded", &[]),
                    engine::DeployError::Block(p) => {
                        self.capsule_tell("capsule.error.cantMergeWithDestination", &[format!("BlockPos{{x={}, y={}, z={}}}", p[0], p[1], p[2])])
                    }
                    engine::DeployError::Entity(n) => self.capsule_tell("capsule.error.cantMergeWithDestinationEntity", &[n]),
                }
                c.revert_from_activated();
                return false;
            }
        };
        // `canPlace`: inside the build height.
        let (min_y, max_y) = (horizon_client::chunk::MIN_Y, horizon_client::chunk::MIN_Y + horizon_client::chunk::HEIGHT as i32);
        if by_pos.keys().any(|p| p[1] >= max_y || p[1] <= min_y) {
            self.capsule_tell("capsule.error.notAllowed", &[]);
            c.revert_from_activated();
            return false;
        }
        // `template.saveOccupiedPositions` + write: an undeploy leaves what was there before.
        template.occupied = Some(occupied.clone());
        if !c.is_reward() && !self.capsule_write_template(&path, &template) {
            self.capsule_tell("capsule.error.technicalError", &[]);
            c.revert_from_activated();
            return false;
        }
        let barrier = registry.default_state("minecraft:barrier");
        let over = |g: &Game<'_>, b: &str| kit_blocks::block_in_tag(g, "capsule:overridable", b);
        for (rel, state, nbt) in &template.blocks {
            let p = [dest[0] + rel[0], dest[1] + rel[1], dest[2] + rel[2]];
            if !engine::deploy_places(&occupied, p, |b| over(self, b)) {
                continue;
            }
            // `Clearable.tryClear(blockentity)`, barrier (flags 20), then the block (flags 2).
            self.capsule_take_block_entity(p);
            if let Some(b) = barrier {
                self.capsule_set(p, b, 20);
            }
            self.capsule_set(p, *state, 2);
            if let Some(n) = nbt {
                self.capsule_put_block_entity(p, n);
            }
        }
        let _ = self.process_level_events();
        for (pos, _, nbt) in &template.entities {
            let at = [f64::from(dest[0]) + pos[0], f64::from(dest[1]) + pos[1], f64::from(dest[2]) + pos[2]];
            let mut tag = nbt.clone();
            tag.insert("Pos".into(), Tag::List(at.iter().map(|v| Tag::Double(v.to_bits())).collect()));
            tag.remove(&JavaString::from("UUID"));
            if tag.contains_key(&JavaString::from("TileX")) {
                tag.insert("TileX".into(), Tag::Int(at[0].floor() as i32));
                tag.insert("TileY".into(), Tag::Int(at[1].floor() as i32));
                tag.insert("TileZ".into(), Tag::Int(at[2].floor() as i32));
            }
            self.capsule_spawn_entity(Tag::Compound(tag));
        }
        if !c.is_reward() {
            c.spawn = Some((dest, self.dim.id().to_owned()));
            c.state = engine::State::Deployed.value();
            c.timer_start = None;
            // `clearTemplate`: the content is in the world now (no copy through a recovery capsule).
            template.blocks.clear();
            template.entities.clear();
            self.capsule_write_template(&path, &template);
        }
        true
    }

    // ---- the right click ----

    /// HEAD `creatures::Game::throw_item` (`CapsuleItem.use`, in the air or after a block's use). `None` = not a
    /// capsule.
    pub(super) fn capsule_use(&mut self, stack: &ItemStack) -> Option<bool> {
        if !is_capsule(stack) {
            return None;
        }
        if self.remote.is_some() {
            // A server's client: the server answers the click.
            return None;
        }
        // `use` with the offhand: InteractionResult.FAIL, nothing changes.
        if !matches!(self.hand, horizon_client::first_person::Hand::Main) {
            return Some(false);
        }
        let main = true;
        let now = self.world.game_time;
        let mut c = read(stack);
        // `inventoryTick` may not have run since the timer ran out.
        if c.timer_tick(now) == Some(true) {
            self.capsule_sound_at_player(CLICK_OFF, 0.2, 0.4);
        }
        let action = engine::use_action(&c, main, kit_blocks::sneaking(self), now);
        let mut used_up = false;
        match action {
            engine::UseAction::Nothing => {}
            engine::UseAction::Label => {
                // The label screen is not ported (README): nothing changes.
            }
            engine::UseAction::Activate(st) => {
                c.state = st.value();
                c.timer_start = Some(now);
                self.capsule_sound_at_player(CLICK_ON, 0.2, 0.9);
            }
            engine::UseAction::Undeploy => {
                if self.capsule_undeploy(&mut c) {
                    self.capsule_sound_at_player(CLICK_OFF, 0.2, 0.4);
                }
            }
            engine::UseAction::Instant => used_up = self.capsule_instant(&mut c),
            engine::UseAction::Throw => {
                let dest = if c.has_structure_link() { self.capsule_aim(c.size()).map(|(h, f)| engine::deploy_target(h, f, c.y_offset)) } else { None };
                self.capsule_throw(&mut c, dest);
                return Some(main);
            }
        }
        let mut updated = self.capsule_held_cell().and_then(|cell| cell.clone());
        if let Some(s2) = updated.as_mut() {
            write(self, s2, &c);
        }
        if let Some(cell) = self.capsule_held_cell() {
            // `heldItem.shrink(1)` for a one-use capsule that deployed (capsules stack to 1).
            *cell = if used_up { None } else { updated };
        }
        Some(main)
    }
    fn capsule_held_cell(&mut self) -> Option<&mut Option<ItemStack>> {
        let slot = self.selected;
        let cell = self.inventory.get_mut(slot)?;
        cell.as_ref().is_some_and(is_capsule).then_some(cell)
    }
    fn capsule_sound_at_player(&mut self, sound: &str, volume: f32, pitch: f32) {
        if let Some(p) = self.player.as_ref() {
            let at = [p.position.x.floor() + 0.5, p.position.y.floor() + 0.5, p.position.z.floor() + 0.5];
            kit_sound::at(self, sound, at, volume, pitch);
        }
    }
    /// `Spacial.clientRayTracePreview`: the block aimed at within 18 + size (outline shapes), and the hit face.
    fn capsule_aim(&self, size: i32) -> Option<(engine::Pos, engine::Pos)> {
        let eye = self.eye_position()?;
        let hit = self.clip_ray(eye, super::view_vector(self.yaw, self.pitch), engine::PREVIEW_REACH + f64::from(size))?;
        Some((hit.pos, hit.face))
    }

    /// `CapsuleThrowQueryToServer` with `instant`: an empty size-1 capsule captures the aimed block, a linked one
    /// deploys on the aimed face.
    fn capsule_instant(&mut self, c: &mut engine::Capsule) -> bool {
        let Some((hit, face)) = self.capsule_aim(c.size()) else { return false };
        let actor = Some(self.acting_player_name().to_owned());
        if c.has(engine::State::Empty) {
            let dest = [hit[0], hit[1] + c.y_offset, hit[2]];
            self.capsule_capture_new(c, dest, actor);
        } else if c.has_structure_link() {
            let dest = engine::deploy_target(hit, face, c.y_offset);
            let anchor = [dest[0], dest[1] - 1, dest[2]];
            if self.capsule_deploy(c, anchor) {
                c.undeploy_at = self.world.game_time + engine::UNDEPLOY_DELAY;
                kit_sound::at(self, ARROW_SHOOT, [f64::from(dest[0]) + 0.5, f64::from(dest[1]) + 0.5, f64::from(dest[2]) + 0.5], 0.4, 0.1);
                return c.one_use;
            }
        }
        false
    }

    /// `Capsule.throwCapsule`: the capsule leaves the hand as an item thrown from the eyes (pickup delay 20),
    /// toward `dest` when there is one.
    fn capsule_throw(&mut self, c: &mut engine::Capsule, dest: Option<engine::Pos>) {
        let Some(p) = self.player.as_ref() else { return };
        let feet = [p.position.x, p.position.y, p.position.z];
        let motion = [p.velocity.x, p.velocity.y, p.velocity.z];
        let Some(eye) = self.eye_position() else { return };
        let start = [feet[0], eye[1] - 0.3, feet[2]];
        let block = feet.map(|v| v.floor() as i32);
        c.deploy_at = dest.map(engine::pos_to_long);
        let v = engine::throw_velocity(block, dest, start, self.yaw, self.pitch, motion);
        let Some(mut stack) = self.capsule_held_cell().and_then(|cell| cell.take()) else { return };
        write(self, &mut stack, c);
        self.spawn_item(stack, start, v, 20);
        let thrower = self.acting_player_uuid();
        if let Some(item) = self.dropped.last_mut() {
            item.thrower = Some(thrower);
            THROWN.with(|t| t.borrow_mut().insert(item.id, ([v[0], v[2]], false)));
        }
        kit_sound::at(self, ARROW_SHOOT, [block[0] as f64 + 0.5, block[1] as f64 + 0.5, block[2] as f64 + 0.5], 0.2, 0.1);
    }

    // ---- each tick ----

    /// HEAD `tick_block_entities`: thrown capsules (`onEntityItemUpdate`, `RecallEnchant`), the in-frame host's
    /// capsules (`inventoryTick`) and its starter capsule.
    pub(super) fn capsule_tick(&mut self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        self.capsule_thrown_tick();
        if self.player.is_some() && self.lan_acting.is_none() {
            self.capsule_inventory_tick();
            let uuid = self.acting_player_uuid();
            self.capsule_starter(uuid);
        }
    }

    /// `inventoryTick` for the player whose inventory is current.
    fn capsule_inventory_tick(&mut self) {
        let now = self.world.game_time;
        let mut sound = false;
        let mut changed: Vec<(usize, ItemStack)> = Vec::new();
        for (i, cell) in self.inventory.iter().enumerate() {
            let Some(stack) = cell.as_ref().filter(|s| is_capsule(s)) else { continue };
            let mut c = read(stack);
            if let Some(timed_out) = c.timer_tick(now) {
                sound |= timed_out;
                let mut s2 = stack.clone();
                write(self, &mut s2, &c);
                changed.push((i, s2));
            }
        }
        for (i, s2) in changed {
            self.inventory[i] = Some(s2);
        }
        if sound {
            self.capsule_sound_at_player(CLICK_OFF, 0.2, 0.4);
        }
    }

    /// HEAD `lan_actions::Game::tick_guests`: the guests' capsules (single player's window included) and their
    /// starter capsule. A guest's turn is entered only when there is work.
    pub(super) fn capsule_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        let now = self.world.game_time;
        for i in 0..host.hub.guests.len() {
            let g = &host.hub.guests[i];
            if !(host.hub.here(g) && g.loaded && g.placed && !g.dead) {
                continue;
            }
            let uuid = horizon_client::lan::playerdata::uuid_ints(g.uuid);
            let needs_starter = self.capsule_saved(|s| !s.starters.contains(&uuid_text(uuid))).unwrap_or(false);
            let timers = guest_has_due_capsule(&g.saved, now);
            if !needs_starter && !timers {
                continue;
            }
            let conn = g.conn;
            let _ = self.as_guest(host, conn, |game| {
                if timers {
                    game.capsule_inventory_tick();
                }
                if needs_starter {
                    game.capsule_starter(uuid);
                }
            });
        }
    }

    /// `StarterLoot.playerLogin` (starterMode = random): a player who never got one receives one starter capsule,
    /// a linked capsule of a copy of one random template of `config/capsule/starters` (the carry's
    /// `initialconfig/starters`), random colours, labelled after the file.
    fn capsule_starter(&mut self, uuid: [i32; 4]) {
        let key = uuid_text(uuid);
        if self.capsule_saved(|s| s.starters.contains(&key)) != Some(false) {
            return;
        }
        let dir = self.options.resources.join("data/capsule/initialconfig/starters");
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "nbt")).collect())
            .unwrap_or_default();
        files.sort();
        self.capsule_saved(|s| s.starters.push(key.clone()));
        if files.is_empty() {
            return;
        }
        use horizon_core::random::RandomSource;
        let pick = self.world.random.next_int_bound(files.len() as i32).unwrap_or(0) as usize;
        let src = files[pick].clone();
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("starter").to_owned();
        let Some(template) = self.capsule_read_template(&src) else { return };
        let player = self.acting_player_name().to_owned();
        let base = format!("{player}-config_capsule_starters_{stem}");
        let Some(n) = self.capsule_saved(|s| s.next()) else { return };
        let name = engine::unique_name(&base, n);
        let Some(dest) = self.capsule_template_path(&name) else { return };
        if !self.capsule_write_template(&dest, &template) {
            return;
        }
        let color = |g: &mut Game<'_>| PALETTE[g.world.random.next_int_bound(PALETTE.len() as i32).unwrap_or(0) as usize];
        let base_color = color(self);
        let material = color(self);
        let c = engine::Capsule {
            state: engine::State::Linked.value(),
            size: engine::size_of_template(template.size),
            material_color: Some(material),
            base_color: Some(base_color),
            structure: Some(name),
            label: Some(engine::label_from_path(&format!("config/capsule/starters/{stem}"))),
            upgraded: 0,
            ..Default::default()
        };
        let mut stack = ItemStack::new(CAPSULE, 1);
        stack.data_mut().dyed_color = Some(base_color);
        write(self, &mut stack, &c);
        self.give_or_drop(stack);
    }

    /// Thrown capsules each tick: they fly toward their target, act when they touch something, and come back
    /// to the thrower with Recall.
    fn capsule_thrown_tick(&mut self) {
        let candidates: Vec<u32> = self.dropped.iter().filter(|i| is_capsule(&i.stack) && read(&i.stack).is_activated()).map(|i| i.id).collect();
        let mut seen = HashSet::new();
        for id in candidates {
            seen.insert(id);
            let Some(i) = self.dropped.iter().position(|d| d.id == id) else { continue };
            let item = &self.dropped[i];
            let c = read(&item.stack);
            let (last, extended) = THROWN.with(|t| t.borrow().get(&id).copied()).unwrap_or(([0.0, 0.0], false));
            let v = item.velocity;
            // `horizontalCollision`: Horizon zeroes a speed the move could not make.
            let wall = (last[0].abs() > 1e-4 && v.x == 0.0) || (last[1].abs() > 1e-4 && v.z == 0.0);
            let block_here = self.world.block(item.position.x.floor() as i32, item.position.y.floor() as i32, item.position.z.floor() as i32);
            let in_liquid = self.res.facts().get(block_here).fluid != horizon_client::blocks::Fluid::None;
            let thrower_in_liquid = false;
            let touched = item.ticks > 2 && (item.on_ground || wall || (in_liquid && !thrower_in_liquid));
            if touched {
                // The thrower acts (claims, messages): a guest's landing runs in the guest's turn.
                let thrower = self.dropped[i].thrower;
                let conn = thrower.and_then(|u| {
                    self.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| horizon_client::lan::playerdata::uuid_ints(g.uuid) == u).map(|g| g.conn))
                });
                match conn {
                    Some(conn) if self.lan_acting.is_none() => {
                        if let Some(mut host) = self.lan.take() {
                            if self.as_guest(&mut host, conn, |g| g.capsule_landed(id)).is_none() {
                                self.lan = Some(host);
                                self.capsule_landed(id);
                            } else {
                                self.lan = Some(host);
                            }
                        }
                    }
                    _ => self.capsule_landed(id),
                }
            } else if let Some(d) = c.deploy_at.map(engine::pos_of_long) {
                let item = &mut self.dropped[i];
                let (vx, vz) = engine::move_toward([item.position.x, item.position.y, item.position.z], [item.velocity.x, item.velocity.y, item.velocity.z], d, true);
                item.velocity.x = vx;
                item.velocity.z = vz;
            }
            // `setExtendedLifetime`: a thrown capsule lasts 12000 ticks, not 6000.
            if let Some(item) = self.dropped.iter_mut().find(|d| d.id == id) {
                let mut ext = extended;
                if !ext && item.age >= 5999 {
                    item.age = 0;
                    ext = true;
                }
                let v = [item.velocity.x, item.velocity.z];
                THROWN.with(|t| t.borrow_mut().insert(id, (v, ext)));
            }
        }
        // Recall: a landed capsule with the enchantment goes back to its thrower (`pickupItemBack`).
        let recall: Vec<u32> = self
            .dropped
            .iter()
            .filter(|i| is_capsule(&i.stack) && i.stack.enchantment_level("capsule:recall") > 0 && i.thrower.is_some() && i.ticks > 2 && i.on_ground)
            .map(|i| i.id)
            .collect();
        for id in recall {
            let Some(i) = self.dropped.iter().position(|d| d.id == id) else { continue };
            let thrower = self.dropped[i].thrower;
            if let Some(at) = thrower.and_then(|u| self.capsule_player_position(u)) {
                let item = &mut self.dropped[i];
                item.position = horizon_core::geometry::Vec3::new(at[0], at[1], at[2]);
                item.pickup_delay = 0;
            }
        }
        THROWN.with(|t| t.borrow_mut().retain(|id, _| seen.contains(id) || self.dropped.iter().any(|d| d.id == *id)));
    }
    fn capsule_player_position(&self, uuid: [i32; 4]) -> Option<[f64; 3]> {
        if let Some(h) = self.lan.as_ref() {
            for g in &h.hub.guests {
                if horizon_client::lan::playerdata::uuid_ints(g.uuid) == uuid && h.hub.here(g) {
                    return Some(g.position);
                }
            }
        }
        let p = self.player.as_ref()?;
        (self.level.player_uuid == Some(uuid)).then(|| [p.position.x, p.position.y, p.position.z])
    }

    /// `Capsule.handleItemEntityOnGround`: a linked capsule deploys on the block under it (or at its target), an
    /// empty one captures the area of a capture base within size + 2.
    fn capsule_landed(&mut self, id: u32) {
        let Some(i) = self.dropped.iter().position(|d| d.id == id) else { return };
        self.dropped[i].velocity = horizon_core::geometry::Vec3::ZERO;
        let (pos, stack, thrower) = {
            let it = &self.dropped[i];
            ([it.position.x, it.position.y, it.position.z], it.stack.clone(), it.thrower)
        };
        let mut c = read(&stack);
        let size = c.size();
        let mut remove = false;
        if c.has_structure_link() {
            let anchor = engine::find_bottom_block(pos[0], pos[1], pos[2]);
            if self.capsule_deploy(&mut c, anchor) {
                kit_sound::at(self, ARROW_SHOOT, pos, 0.4, 0.1);
                remove = c.one_use;
            }
        } else {
            let base = engine::search_order(pos[0], pos[1], pos[2], size + 2)
                .find(|p| self.capsule_block_name(*p).as_deref() == Some(MARKER));
            match base {
                Some(b) => {
                    let state = self.world.block(b[0], b[1], b[2]);
                    let facing = kit_blocks::prop(self, state, "facing").and_then(|f| dir_of(&f)).unwrap_or([0, 1, 0]);
                    let anchor = engine::anchor(b, facing, size);
                    let who = thrower.and_then(|u| self.capsule_player_name(u));
                    let source = engine::capture_source(anchor, c.extend());
                    self.capsule_capture_new(&mut c, source, who);
                }
                None => {
                    c.revert_from_activated();
                    if let Some(u) = thrower {
                        self.capsule_tell_player(u, "capsule.error.noCaptureBase");
                    }
                }
            }
        }
        let Some(i) = self.dropped.iter().position(|d| d.id == id) else { return };
        if remove {
            self.dropped.remove(i);
        } else {
            let mut s2 = stack;
            write(self, &mut s2, &c);
            self.dropped[i].stack = s2;
        }
    }
    fn capsule_player_name(&self, uuid: [i32; 4]) -> Option<String> {
        if let Some(h) = self.lan.as_ref() {
            for g in &h.hub.guests {
                if horizon_client::lan::playerdata::uuid_ints(g.uuid) == uuid {
                    return Some(g.name.clone());
                }
            }
        }
        (self.level.player_uuid == Some(uuid)).then(|| super::chat::PLAYER_NAME.to_owned())
    }
    fn capsule_tell_player(&mut self, uuid: [i32; 4], key: &str) {
        let conn = self.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| horizon_client::lan::playerdata::uuid_ints(g.uuid) == uuid).map(|g| g.conn));
        match conn {
            Some(conn) if self.lan_acting.is_none() => {
                let Some(mut host) = self.lan.take() else { return };
                let _ = self.as_guest(&mut host, conn, |g| g.capsule_tell(key, &[]));
                self.lan = Some(host);
            }
            _ => self.capsule_tell(key, &[]),
        }
    }

    // ---- dispensers (DispenseCapsuleBehavior) ----

    /// HEAD cancellable `dispenser::Game::dispense_item`: a deployed capsule comes back, a linked one deploys in
    /// front of the dispenser (capture base); the capsule stays in the slot (a one-use one is used up).
    pub(super) fn capsule_dispense(&mut self, pos: [i32; 3], facing: [i32; 3], stack: &ItemStack) -> Option<Option<ItemStack>> {
        if !is_capsule(stack) || self.remote.is_some() {
            return None;
        }
        let mut c = read(stack);
        let mut out = Some(stack.clone());
        let center = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
        if c.has(engine::State::Deployed) && c.spawn.is_some() {
            if self.capsule_undeploy(&mut c) {
                kit_sound::at(self, CLICK_OFF, center, 0.2, 0.4);
            }
        } else if c.has_structure_link() {
            let size = c.size();
            let a = engine::anchor(pos, facing, size);
            let anchor = [a[0], a[1] + c.y_offset, a[2]];
            if self.capsule_deploy(&mut c, anchor) {
                kit_sound::at(self, ARROW_SHOOT, center, 0.2, 0.4);
                if c.one_use {
                    out = None;
                }
            }
        }
        if let Some(s2) = out.as_mut() {
            write(self, s2, &c);
        }
        // `DefaultDispenseItemBehavior.playSound`: the dispenser click.
        kit_sound::level_event(self, 1000, pos, 0);
        Some(out)
    }

    // ---- the tooltip ----

    /// RETURN `stations::Game::stack_tooltip` (`CapsuleItem.appendHoverText`).
    pub(super) fn capsule_tooltip(&self, stack: &ItemStack, mut lines: Vec<(horizon_client::text::Text, u32)>) -> Vec<(horizon_client::text::Text, u32)> {
        if !is_capsule(stack) {
            return lines;
        }
        let c = read(stack);
        let tr = |k: &str| self.pack.names.display_get(k).unwrap_or_else(|| k.to_owned());
        let mut add = Vec::new();
        for line in engine::tooltip(&c) {
            match line {
                engine::Line::Author(a) => add.push((format!("§3§o{} {a}§r", tr("capsule.tooltip.author")), u32::MAX)),
                engine::Line::Key(k) => add.push((tr(k).trim().to_owned(), u32::MAX)),
                engine::Line::Overpowered => add.push((format!("§5{}§r", tr("capsule.tooltip.overpowered")), u32::MAX)),
                engine::Line::Size { size, upgraded, instant } => {
                    let mut t = format!("{size}×{size}×{size}");
                    if upgraded > 0 {
                        t.push_str(&format!(" ({upgraded}/{} {})", engine::config::UPGRADE_LIMIT, tr("capsule.tooltip.upgraded")));
                    }
                    if instant {
                        t.push_str(&format!(" ({})", tr("capsule.tooltip.instant").trim()));
                    }
                    add.push((format!("{}: {t}", tr("capsule.tooltip.size")), u32::MAX));
                }
            }
        }
        let at = lines.len().min(1);
        for (i, (t, color)) in add.into_iter().enumerate() {
            lines.insert(at + i, (horizon_client::text::Text::legacy(&t), color));
        }
        lines
    }
}

/// The position of a raw block entity tag (`x`, `y`, `z`).
fn raw_pos(t: &Tag) -> Option<engine::Pos> {
    let n = |k: &str| horizon_client::save::number(t.get(k)).map(|v| v as i32);
    Some([n("x")?, n("y")?, n("z")?])
}

fn uuid_text(u: [i32; 4]) -> String {
    format!("{}-{}-{}-{}", u[0], u[1], u[2], u[3])
}

/// A guest's saved inventory holds an activated capsule whose timer ran out (or has none).
fn guest_has_due_capsule(saved: &Option<Tag>, now: i64) -> bool {
    let Some(Tag::List(inv)) = saved.as_ref().and_then(|t| t.get("Inventory")) else { return false };
    inv.iter().any(|slot| {
        let is = matches!(slot.get("id"), Some(Tag::String(s)) if s.to_string_lossy() == CAPSULE);
        let data = slot.get("components").and_then(|c| c.get("minecraft:custom_data"));
        let state = data.and_then(|d| match d.get("state") {
            Some(Tag::Int(v)) => Some(*v),
            _ => None,
        });
        let start = data.and_then(|d| d.get("activetimer")).and_then(|t| match t.get("starttime") {
            Some(Tag::Long(v)) => Some(*v),
            _ => None,
        });
        is && matches!(state, Some(1 | 4 | 6))
            && start.is_none_or(|s| now >= s + engine::config::PREVIEW_DISPLAY_DURATION || s > now)
    })
}
