// Applied Energistics 2 (stage 1) inside Horizon's game (nightly 142388e1c). The rules are the
// engine below (src/lib.rs); this is the game's side:
// - every AE2 block of a world is a record of the engine's `State` (side file
//   `<world>/data/ae2/<dimension>.txt`): its items (cells, interface storage, machine slots,
//   crafting grid), the cells' contents, ghost config, numbers, its share of the grid energy;
//   a record whose block is gone spills its items (cells keep their contents) at the next tick;
// - networks are rebuilt from the records when the topology changes (channels, controller
//   state), their energy is the sum of the members' shares, spread back after every operation;
// - energy enters through foundry (Energy Acceptor, Controller, the machines' buffers), items move
//   through foundry's item handlers (vanilla containers, other mods' blocks);
// - screens: a temporary `BlockEntity::Chest` window whose slots mirror the network or the
//   device (no real item ever lives in it: every click is handled here, hoppers cannot see it,
//   it is removed when the screen closes or a save happens).
use super::{Game, Screen};
use horizon_client::containers::BlockEntity;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_client::menu::Input;
use horizon_core::random::RandomSource;
use horizon_world::level::flags as level_flags;
use horizon_world::nbt::{JavaString, Tag};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

#[allow(dead_code)]
pub(in crate::game) mod engine {
    // Applied Energistics 2 for Horizon, stage 1 (item storage network): the rules, without game code.
// Written from AE2 forge 15.4.11 for Minecraft 1.20.1 (AppliedEnergistics, LGPL-3.0; the public
// source at tag forge/v15.4.11, cloned in sources/ae2, the same version as the pack's jar
// appliedenergistics2-forge-15.4.11.jar) and the pack's config (config/ae2/common.json, client.json).
// Every value names the Java class it comes from.
// This file is pasted into the game as `crate::game::ae2::engine` (tools/hnmod.py bundle) and is also the
// `horizon_ae2` crate the tests use: plain Rust, no game types, no inner attributes (`#![...]`).
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub type Pos = [i32; 3];
/// An item kind (`AEItemKey`): the adapter's canonical text of a stack of 1 (item + components).
/// Opaque here; never empty, no whitespace.
pub type Key = String;

/// the pack's values (profile the 1.20.1 pack, config/ae2) and AE2's code constants.
#[allow(dead_code)]
pub mod config {
    // ---- ae2\client.json
    pub const CLIENT_USE_TERMINAL_USE_LARGE_FONT: bool = false; // ae2\client.json [client] useTerminalUseLargeFont
    pub const CLIENT_CLEAR_GRID_ON_CLOSE: bool = false; // ae2\client.json [client] clearGridOnClose
    pub const CLIENT_TERMINAL_MARGIN: i64 = 25; // ae2\client.json [client] terminalMargin
    pub const TERMINALS_TERMINAL_STYLE: &str = "small"; // ae2\client.json [terminals] terminalStyle
    pub const SEARCH_REMEMBER_LAST_SEARCH: bool = true; // ae2\client.json [search] rememberLastSearch
    pub const SEARCH_AUTO_FOCUS_SEARCH: bool = false; // ae2\client.json [search] autoFocusSearch
    // ---- ae2\common.json
    pub const GENERAL_CHANNELS: &str = "default"; // ae2\common.json [general] channels
    pub const BATTERY_CHARGER_CHARGE_RATE: f64 = 1.0; // ae2\common.json [battery] chargerChargeRate
    pub const WORLD_GEN_SPAWN_PRESSES_IN_METEORITES: bool = true; // ae2\common.json [worldGen] spawnPressesInMeteorites
    pub const WORLD_GEN_SPAWN_FLAWLESS_ONLY: bool = false; // ae2\common.json [worldGen] spawnFlawlessOnly
    pub const POWER_RATIOS_FORGE_ENERGY: f64 = 0.5; // ae2\common.json [PowerRatios] ForgeEnergy
    pub const POWER_RATIOS_USAGE_MULTIPLIER: f64 = 1.0; // ae2\common.json [PowerRatios] UsageMultiplier
    pub const POWER_RATIOS_GRID_ENERGY_STORAGE_PER_NODE: f64 = 25.0; // ae2\common.json [PowerRatios] GridEnergyStoragePerNode
    pub const TICK_RATES_INTERFACE_MIN: i64 = 5; // ae2\common.json [tickRates] InterfaceMin
    pub const TICK_RATES_INTERFACE_MAX: i64 = 120; // ae2\common.json [tickRates] InterfaceMax
    pub const TICK_RATES_IMPORT_BUS_MIN: i64 = 5; // ae2\common.json [tickRates] ImportBusMin
    pub const TICK_RATES_IMPORT_BUS_MAX: i64 = 40; // ae2\common.json [tickRates] ImportBusMax
    pub const TICK_RATES_EXPORT_BUS_MIN: i64 = 5; // ae2\common.json [tickRates] ExportBusMin
    pub const TICK_RATES_EXPORT_BUS_MAX: i64 = 60; // ae2\common.json [tickRates] ExportBusMax
    pub const TICK_RATES_INSCRIBER_MIN: i64 = 1; // ae2\common.json [tickRates] InscriberMin
    pub const TICK_RATES_INSCRIBER_MAX: i64 = 20; // ae2\common.json [tickRates] InscriberMax
    pub const TICK_RATES_CHARGER_MIN: i64 = 10; // ae2\common.json [tickRates] ChargerMin
    pub const TICK_RATES_CHARGER_MAX: i64 = 10; // ae2\common.json [tickRates] ChargerMax
    pub const TICK_RATES_VIBRATION_CHAMBER_MIN: i64 = 10; // ae2\common.json [tickRates] VibrationChamberMin
    pub const TICK_RATES_VIBRATION_CHAMBER_MAX: i64 = 40; // ae2\common.json [tickRates] VibrationChamberMax
    pub const VIBRATION_CHAMBER_BASE_ENERGY_PER_FUEL_TICK: f64 = 5.0; // ae2\common.json [vibrationChamber] baseEnergyPerFuelTick
    pub const VIBRATION_CHAMBER_MIN_ENERGY_PER_GAME_TICK: i64 = 4; // ae2\common.json [vibrationChamber] minEnergyPerGameTick
    pub const VIBRATION_CHAMBER_BASE_MAX_ENERGY_PER_GAME_TICK: i64 = 40; // ae2\common.json [vibrationChamber] baseMaxEnergyPerGameTick

    // ---- AE2 code (forge/v15.4.11)
    /// ChannelMode.DEFAULT: getAdHocNetworkChannels() = 8, getCableCapacityFactor() = 1.
    pub const AD_HOC_CHANNELS: u32 = 8;
    /// GridNode.getMaxChannels: 8 (normal), 32 (DENSE_CAPACITY), times the capacity factor.
    pub const CABLE_CHANNELS: u32 = 8;
    /// CableBusBlock 15.4.11: mining a cable bus breaks it whole (`getDrops` = every part's item);
    /// one part comes out with a wrench (sneak + use, `disassembleWithWrench`).
    pub const BREAK_WHOLE_CABLE_BUS: bool = true;
    pub const DENSE_CHANNELS: u32 = 32;
    /// ControllerValidator.MAX_SIZE.
    pub const CONTROLLER_MAX_SIZE: i32 = 7;
    /// ControllerBlockEntity: setInternalMaxPower(8000), setIdlePowerUsage(3).
    pub const CONTROLLER_STORAGE: f64 = 8000.0;
    pub const CONTROLLER_IDLE: f64 = 3.0;
    /// EnergyService.publicPowerState: power-ups are buffered for 30 ticks.
    pub const POWER_UP_DELAY: u32 = 30;
    /// PathingService: channel power usage = channelsByBlocks / 128.
    pub const CHANNEL_POWER_DIVISOR: f64 = 128.0;
    /// AEKeyType.getAmountPerByte (items).
    pub const AMOUNT_PER_BYTE: u64 = 8;
    /// BasicCellInventory.MAX_ITEM_TYPES.
    pub const MAX_ITEM_TYPES: u64 = 63;
    /// InscriberBlockEntity: MAX_PROCESSING_STEPS = 200, setInternalMaxPower(1600).
    pub const INSCRIBER_MAX_STEPS: i32 = 200;
    pub const INSCRIBER_BUFFER: f64 = 1600.0;
    /// ChargerBlockEntity.POWER_MAXIMUM_AMOUNT.
    pub const CHARGER_BUFFER: f64 = 1600.0;
    /// CrankBlockEntity: POWER_PER_CRANK_TURN = 160, ticksPerRotation = 18.
    pub const CRANK_POWER_PER_TURN: f64 = 160.0;
    pub const CRANK_TICKS_PER_ROTATION: i32 = 18;
    /// ChestBlockEntity: setInternalMaxPower(PowerMultiplier.CONFIG.multiply(500)).
    pub const CHEST_BUFFER: f64 = 500.0;
    /// MEStorageScreen.MIN_ROWS; TerminalStyle.SMALL multiplier 1 of 4.
    pub const TERMINAL_MIN_ROWS: i32 = 2;
    /// BuddingCertusQuartzBlock: GROWTH_CHANCE = 5, DECAY_CHANCE = 12.
    pub const BUDDING_GROWTH_CHANCE: i32 = 5;
    pub const BUDDING_DECAY_CHANCE: i32 = 12;
    /// InterfaceLogic: 9 config slots and 9 storage slots (AEBaseInterface NUMBER_OF_SLOTS).
    pub const INTERFACE_SLOTS: usize = 9;
    /// IOBusPart.availableSlots: min(18 + 9 * capacity cards, config size); export bus without
    /// capacity card shows 1 config slot (UpgradeablePart / IOBusMenu: 1 + 4 per card).
    pub const IO_BUS_SLOTS: usize = 1;
    /// StorageBusPart: 63 config slots, 18 shown without capacity cards.
    pub const STORAGE_BUS_SLOTS: usize = 18;
}

// ---------------------------------------------------------------------------------------------
// Directions (Direction.get3DDataValue order: down, up, north, south, west, east).

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}
impl Dir {
    pub const ALL: [Dir; 6] = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East];
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn from_idx(i: usize) -> Option<Dir> {
        Dir::ALL.get(i).copied()
    }
    pub fn opposite(self) -> Dir {
        match self {
            Dir::Down => Dir::Up,
            Dir::Up => Dir::Down,
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
            Dir::East => Dir::West,
        }
    }
    pub fn step(self) -> Pos {
        match self {
            Dir::Down => [0, -1, 0],
            Dir::Up => [0, 1, 0],
            Dir::North => [0, 0, -1],
            Dir::South => [0, 0, 1],
            Dir::West => [-1, 0, 0],
            Dir::East => [1, 0, 0],
        }
    }
    pub fn name(self) -> &'static str {
        ["down", "up", "north", "south", "west", "east"][self.idx()]
    }
    pub fn parse(s: &str) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.name() == s)
    }
    pub fn from_normal(n: [i32; 3]) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.step() == n)
    }
}
pub fn offset(p: Pos, d: Dir) -> Pos {
    let s = d.step();
    [p[0] + s[0], p[1] + s[1], p[2] + s[2]]
}

/// Where an AE2 record lives: a whole block or the cable (centre) of a cable bus (`side: None`),
/// or a part on a face of a cable bus (`side: Some(face)`, AE2 `IPartHost.getPart(side)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Loc {
    pub pos: Pos,
    pub side: Option<Dir>,
}
impl Loc {
    pub fn block(pos: Pos) -> Loc {
        Loc { pos, side: None }
    }
    pub fn part(pos: Pos, side: Dir) -> Loc {
        Loc { pos, side: Some(side) }
    }
}
impl From<Pos> for Loc {
    fn from(pos: Pos) -> Loc {
        Loc::block(pos)
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Storage cells (BasicStorageCell + BasicCellInventory).

/// One item storage cell kind (AEItems: `new BasicStorageCell(props, component, housing,
/// idleDrain, kilobytes, bytesPerType, totalTypes, AEKeyType.items())`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellType {
    pub item: &'static str,
    /// `kilobytes * 1024` (BasicStorageCell constructor).
    pub bytes: u64,
    pub bytes_per_type: u64,
    pub types: u64,
    /// AE/t while mounted in a drive or chest (getIdleDrain).
    pub idle_drain: f64,
}
/// AEItems.ITEM_CELL_1K .. ITEM_CELL_256K.
pub const CELLS: [CellType; 5] = [
    CellType { item: "ae2:item_storage_cell_1k", bytes: 1024, bytes_per_type: 8, types: 63, idle_drain: 0.5 },
    CellType { item: "ae2:item_storage_cell_4k", bytes: 4 * 1024, bytes_per_type: 32, types: 63, idle_drain: 1.0 },
    CellType { item: "ae2:item_storage_cell_16k", bytes: 16 * 1024, bytes_per_type: 128, types: 63, idle_drain: 1.5 },
    CellType { item: "ae2:item_storage_cell_64k", bytes: 64 * 1024, bytes_per_type: 512, types: 63, idle_drain: 2.0 },
    CellType { item: "ae2:item_storage_cell_256k", bytes: 256 * 1024, bytes_per_type: 2048, types: 63, idle_drain: 2.5 },
];
pub fn cell_type(item: &str) -> Option<&'static CellType> {
    CELLS.iter().find(|c| c.item == item)
}

/// `CellState`: the drive's LED colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellStatus {
    Empty,
    NotEmpty,
    TypesFull,
    Full,
}

/// The contents of one cell (`keys`/`amts` in the cell item's tag), in insertion order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellInv {
    pub items: Vec<(Key, u64)>,
}
impl CellInv {
    pub fn amount(&self, key: &str) -> u64 {
        self.items.iter().find(|(k, _)| k == key).map_or(0, |e| e.1)
    }
    /// storedItemCount.
    pub fn stored_count(&self) -> u64 {
        self.items.iter().map(|e| e.1).sum()
    }
    /// storedItems.
    pub fn stored_types(&self) -> u64 {
        self.items.len() as u64
    }
    /// getUnusedItemCount: what is left of the last partly used byte.
    pub fn unused_item_count(&self) -> u64 {
        let div = self.stored_count() % config::AMOUNT_PER_BYTE;
        if div == 0 { 0 } else { config::AMOUNT_PER_BYTE - div }
    }
    /// getUsedBytes.
    pub fn used_bytes(&self, t: &CellType) -> u64 {
        let for_count = (self.stored_count() + self.unused_item_count()) / config::AMOUNT_PER_BYTE;
        self.stored_types() * t.bytes_per_type + for_count
    }
    /// getFreeBytes (signed in Java; never below zero for cells filled by these rules).
    pub fn free_bytes(&self, t: &CellType) -> i64 {
        t.bytes as i64 - self.used_bytes(t) as i64
    }
    /// getTotalItemTypes: min(totalTypes, 63), at least 1.
    pub fn total_types(t: &CellType) -> u64 {
        t.types.clamp(1, config::MAX_ITEM_TYPES)
    }
    /// getRemainingItemTypes.
    pub fn remaining_types(&self, t: &CellType) -> i64 {
        let on_storage = self.free_bytes(t) / t.bytes_per_type as i64;
        let on_total = Self::total_types(t) as i64 - self.stored_types() as i64;
        on_storage.min(on_total)
    }
    /// getRemainingItemCount.
    pub fn remaining_item_count(&self, t: &CellType) -> u64 {
        let r = self.free_bytes(t) * config::AMOUNT_PER_BYTE as i64 + self.unused_item_count() as i64;
        r.max(0) as u64
    }
    /// canHoldNewItem.
    pub fn can_hold_new_item(&self, t: &CellType) -> bool {
        let free = self.free_bytes(t);
        let bpt = t.bytes_per_type as i64;
        (free > bpt || free == bpt && self.unused_item_count() > 0) && self.remaining_types(t) > 0
    }
    /// getStatus.
    pub fn status(&self, t: &CellType) -> CellStatus {
        if self.stored_types() == 0 {
            CellStatus::Empty
        } else if self.can_hold_new_item(t) {
            CellStatus::NotEmpty
        } else if self.remaining_item_count(t) > 0 {
            CellStatus::TypesFull
        } else {
            CellStatus::Full
        }
    }
    /// insert + innerInsert (no partition, no void card: stage 1 has no cell workbench).
    /// `nested_cell` = the key is a storage cell that is not empty (`canFitInsideCell` false).
    pub fn insert(&mut self, t: &CellType, key: &Key, amount: u64, simulate: bool, nested_cell: bool) -> u64 {
        if amount == 0 || nested_cell {
            return 0;
        }
        let current = self.amount(key);
        let mut remaining = self.remaining_item_count(t) as i64;
        if current == 0 {
            if !self.can_hold_new_item(t) {
                return 0;
            }
            remaining -= (t.bytes_per_type * config::AMOUNT_PER_BYTE) as i64;
            if remaining <= 0 {
                return 0;
            }
        }
        // maxItemsPerType = Long.MAX_VALUE without an equal distribution card.
        let n = amount.min(remaining.max(0) as u64);
        if !simulate && n > 0 {
            match self.items.iter_mut().find(|(k, _)| k == key) {
                Some(e) => e.1 += n,
                None => self.items.push((key.clone(), n)),
            }
        }
        n
    }
    /// extract.
    pub fn extract(&mut self, key: &str, amount: u64, simulate: bool) -> u64 {
        let Some(i) = self.items.iter().position(|(k, _)| k == key) else {
            return 0;
        };
        let current = self.items[i].1;
        let n = amount.min(current);
        if !simulate {
            if n >= current {
                self.items.remove(i);
            } else {
                self.items[i].1 -= n;
            }
        }
        n
    }
    /// The cell's text in an item (`ae2:cell` custom data): `key amount` per line.
    pub fn encode(&self) -> String {
        let mut s = String::new();
        for (k, n) in &self.items {
            if *n > 0 {
                s.push_str(k);
                s.push(' ');
                s.push_str(&n.to_string());
                s.push('\n');
            }
        }
        s
    }
    /// loadCellItems: unreadable entries are dropped (as `corruptedTag`).
    pub fn decode(text: &str) -> CellInv {
        let mut c = CellInv::default();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            if let (Some(k), Some(n), None) = (it.next(), it.next().and_then(|n| n.parse::<u64>().ok()), it.next())
                && n > 0
            {
                match c.items.iter_mut().find(|(x, _)| x == k) {
                    Some(e) => e.1 += n,
                    None => c.items.push((k.to_owned(), n)),
                }
            }
        }
        c
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Network storage (NetworkStorage, MEInventoryHandler, StorageHelper).

/// The storages mounted on a network, by index (cells in drives and chests, storage buses).
/// The adapter implements it over the game; tests over plain vectors.
pub trait Mounts {
    fn mounts(&self) -> usize;
    /// The mount's priority (drive / chest / storage bus priority, 0 by default).
    fn priority(&self, i: usize) -> i32;
    fn insert(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64;
    fn extract(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64;
    /// The key is listed in the mount's whitelist partition.
    fn listed(&self, _i: usize, _key: &Key) -> bool {
        false
    }
    fn list(&mut self, i: usize, out: &mut BTreeMap<Key, u64>);
}

/// `priorityInventory`: mounts grouped by priority, highest first, each group in mount order.
fn priority_groups(m: &dyn Mounts) -> Vec<Vec<usize>> {
    let mut by: BTreeMap<std::cmp::Reverse<i32>, Vec<usize>> = BTreeMap::new();
    for i in 0..m.mounts() {
        by.entry(std::cmp::Reverse(m.priority(i))).or_default().push(i);
    }
    by.into_values().collect()
}

/// NetworkStorage.insert: per priority group (highest first), first the mounts that prefer the
/// key (`isPreferredStorageFor`: whitelisted, or already holding some), then the others.
pub fn net_insert(m: &mut dyn Mounts, key: &Key, amount: u64, simulate: bool) -> u64 {
    let mut remaining = amount;
    for group in priority_groups(m) {
        let mut second = Vec::new();
        for &i in &group {
            if remaining == 0 {
                break;
            }
            let preferred = m.listed(i, key) || m.extract(i, key, 1, true) > 0;
            if preferred {
                remaining -= m.insert(i, key, remaining, simulate).min(remaining);
            } else {
                second.push(i);
            }
        }
        for i in second {
            if remaining == 0 {
                break;
            }
            remaining -= m.insert(i, key, remaining, simulate).min(remaining);
        }
    }
    amount - remaining
}
/// NetworkStorage.extract: lowest priority first.
pub fn net_extract(m: &mut dyn Mounts, key: &Key, amount: u64, simulate: bool) -> u64 {
    let mut got = 0;
    for group in priority_groups(m).into_iter().rev() {
        for i in group {
            if got >= amount {
                return got;
            }
            got += m.extract(i, key, amount - got, simulate);
        }
    }
    got
}
/// NetworkStorage.getAvailableStacks.
pub fn net_list(m: &mut dyn Mounts) -> BTreeMap<Key, u64> {
    let mut out = BTreeMap::new();
    for i in 0..m.mounts() {
        m.list(i, &mut out);
    }
    out
}

/// What gives AE energy to an operation (the grid's energy service).
pub trait EnergySource {
    /// `extractAEPower(amt, mode, PowerMultiplier.ONE)`: what could be (or was) taken.
    fn extract_ae(&mut self, amount: f64, simulate: bool) -> f64;
}

/// PowerMultiplier.CONFIG: AE2 multiplies the cost by UsageMultiplier and divides the result.
fn extract_config(e: &mut dyn EnergySource, amount: f64, simulate: bool) -> f64 {
    let m = config::POWER_RATIOS_USAGE_MULTIPLIER;
    e.extract_ae(amount * m, simulate) / m
}

/// StorageHelper.poweredInsert (1 AE per item). Returns what went in.
pub fn powered_insert(e: &mut dyn EnergySource, m: &mut dyn Mounts, key: &Key, amount: u64, simulate: bool) -> u64 {
    let amount = net_insert(m, key, amount, true);
    if amount == 0 {
        return 0;
    }
    let available = extract_config(e, amount as f64, true);
    let amount = ((available + 0.9) as u64).min(amount);
    if amount == 0 {
        return 0;
    }
    if simulate {
        return amount;
    }
    extract_config(e, amount as f64, false);
    net_insert(m, key, amount, false)
}
/// StorageHelper.poweredExtraction (1 AE per item; charged for what the storage could give).
pub fn powered_extract(e: &mut dyn EnergySource, m: &mut dyn Mounts, key: &Key, amount: u64, simulate: bool) -> u64 {
    let retrieved = net_extract(m, key, amount, true);
    let available = extract_config(e, retrieved as f64, true);
    let n = ((available + 0.9) as u64).min(retrieved);
    if n == 0 {
        return 0;
    }
    if simulate {
        return n;
    }
    extract_config(e, retrieved as f64, false);
    net_extract(m, key, n, false)
}

/// An external inventory seen as slots (`IItemHandler` through foundry), for storage, import and
/// export buses and the interface.
pub trait Inv {
    fn slots(&mut self) -> usize;
    fn get(&mut self, slot: usize) -> Option<(Key, u64)>;
    /// `insertItem(slot, stack, simulate)`: how many went in.
    fn insert(&mut self, slot: usize, key: &Key, amount: u64, simulate: bool) -> u64;
    /// `extractItem(slot, amount, simulate)`: how many came out.
    fn extract(&mut self, slot: usize, amount: u64, simulate: bool) -> u64;
}
/// ItemHandlerHelper.insertItem: slot by slot, in order.
pub fn inv_insert(inv: &mut dyn Inv, key: &Key, amount: u64, simulate: bool) -> u64 {
    let mut done = 0;
    for s in 0..inv.slots() {
        if done >= amount {
            break;
        }
        done += inv.insert(s, key, amount - done, simulate);
    }
    done
}
/// The item handler facade's extract: every slot holding the key.
pub fn inv_extract(inv: &mut dyn Inv, key: &Key, amount: u64, simulate: bool) -> u64 {
    let mut done = 0;
    for s in 0..inv.slots() {
        if done >= amount {
            break;
        }
        if inv.get(s).is_some_and(|(k, _)| &k == key) {
            done += inv.extract(s, amount - done, simulate);
        }
    }
    done
}
pub fn inv_list(inv: &mut dyn Inv, out: &mut BTreeMap<Key, u64>) {
    for s in 0..inv.slots() {
        if let Some((k, n)) = inv.get(s)
            && n > 0
        {
            *out.entry(k).or_insert(0) += n;
        }
    }
}

/// Storage bus settings (StorageBusPart; partition = its config slots, READ_WRITE access,
/// FILTER_ON_EXTRACT enabled, so listing is unfiltered and extraction respects the partition).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BusFilter {
    pub priority: i32,
    pub partition: Vec<Key>,
}
impl BusFilter {
    /// passesBlackOrWhitelist (whitelist, only when something is listed).
    pub fn passes(&self, key: &Key) -> bool {
        self.partition.is_empty() || self.partition.contains(key)
    }
    pub fn listed(&self, key: &Key) -> bool {
        self.partition.contains(key)
    }
    /// MEInventoryHandler.insert over the inventory.
    pub fn insert(&self, inv: &mut dyn Inv, key: &Key, amount: u64, simulate: bool) -> u64 {
        if !self.passes(key) {
            return 0;
        }
        inv_insert(inv, key, amount, simulate)
    }
    /// MEInventoryHandler.extract (filterOnExtraction).
    pub fn extract(&self, inv: &mut dyn Inv, key: &Key, amount: u64, simulate: bool) -> u64 {
        if !self.passes(key) {
            return 0;
        }
        inv_extract(inv, key, amount, simulate)
    }
}

/// What a bus moved, and what neither side took back (AE2 voids it with a log line; the
/// adapter drops it in the world instead, so nothing is ever lost).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moved {
    pub items: u64,
    pub stray: Vec<(Key, u64)>,
}

/// StorageImportStrategy.transfer (import bus): up to `ops` items from the adjacent inventory,
/// slot by slot, only what the network takes. `filter` empty = everything.
pub fn import_transfer<W: Mounts + Inv>(w: &mut W, filter: &[Key], ops: u64) -> Moved {
    let mut remaining_ops = ops;
    let mut moved = Moved::default();
    let slots = Inv::slots(w);
    for s in 0..slots {
        if remaining_ops == 0 {
            break;
        }
        let Some((key, _)) = Inv::get(w, s) else {
            continue;
        };
        // context.isInFilter(what) == context.isInverted() -> skip (no inverter card).
        if !filter.is_empty() && !filter.contains(&key) {
            continue;
        }
        let fits = net_insert(w, &key, remaining_ops, true);
        let amount = inv_extract(w, &key, fits, false);
        if amount > 0 {
            let inserted = net_insert(w, &key, amount, false);
            if inserted < amount {
                // "Be nice and try to give the overflow back".
                let back = inv_insert(w, &key, amount - inserted, false);
                if back < amount - inserted {
                    moved.stray.push((key.clone(), amount - inserted - back));
                }
            }
            remaining_ops -= inserted.max(1).min(remaining_ops);
            moved.items += inserted;
        }
    }
    moved
}
/// ExportBusPart.doBusWork + StorageExportStrategy.transfer: the configured keys in order, up
/// to `ops` items, powered extraction. Returns moved items.
pub fn export_transfer<W: Mounts + Inv>(
    w: &mut W,
    e: &mut dyn EnergySource,
    config: &[Option<Key>],
    ops: u64,
) -> Moved {
    let mut remaining_ops = ops;
    let mut moved = Moved::default();
    for key in config.iter().flatten() {
        if remaining_ops == 0 {
            break;
        }
        let extracted = powered_extract(e, w, key, remaining_ops, true);
        let fits = inv_insert(w, key, extracted, true);
        if fits == 0 {
            continue;
        }
        let extracted = powered_extract(e, w, key, fits, false);
        let inserted = inv_insert(w, key, extracted, false);
        if inserted < extracted {
            let back = net_insert(w, key, extracted - inserted, false);
            if back < extracted - inserted {
                moved.stray.push((key.clone(), extracted - inserted - back));
            }
        }
        if inserted > 0 {
            remaining_ops -= inserted.max(1).min(remaining_ops);
            moved.items += inserted;
        }
    }
    moved
}
/// A network and a separate inventory seen as one bus world (tests; the adapter's live world
/// implements both traits on the game itself).
pub struct Both<'a, M: Mounts + ?Sized, I: Inv + ?Sized> {
    pub m: &'a mut M,
    pub i: &'a mut I,
}
impl<M: Mounts + ?Sized, I: Inv + ?Sized> Mounts for Both<'_, M, I> {
    fn mounts(&self) -> usize {
        self.m.mounts()
    }
    fn priority(&self, i: usize) -> i32 {
        self.m.priority(i)
    }
    fn insert(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        self.m.insert(i, key, amount, simulate)
    }
    fn extract(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        self.m.extract(i, key, amount, simulate)
    }
    fn listed(&self, i: usize, key: &Key) -> bool {
        self.m.listed(i, key)
    }
    fn list(&mut self, i: usize, out: &mut BTreeMap<Key, u64>) {
        self.m.list(i, out)
    }
}
impl<M: Mounts + ?Sized, I: Inv + ?Sized> Inv for Both<'_, M, I> {
    fn slots(&mut self) -> usize {
        self.i.slots()
    }
    fn get(&mut self, slot: usize) -> Option<(Key, u64)> {
        self.i.get(slot)
    }
    fn insert(&mut self, slot: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        self.i.insert(slot, key, amount, simulate)
    }
    fn extract(&mut self, slot: usize, amount: u64, simulate: bool) -> u64 {
        self.i.extract(slot, amount, simulate)
    }
}
/// IOBusPart.getOperationsPerTick without speed cards.
pub const IO_BUS_OPS: u64 = 1;

// ---------------------------------------------------------------------------------------------
// 3. Energy (EnergyService, GridEnergyStorage, PowerUnits).

/// PowerUnits.FE: 1 FE = ForgeEnergy (0.5) AE.
pub fn fe_to_ae(fe: f64) -> f64 {
    fe * config::POWER_RATIOS_FORGE_ENERGY
}
pub fn ae_to_fe(ae: f64) -> f64 {
    ae / config::POWER_RATIOS_FORGE_ENERGY
}

/// The energy service of one network: one pool made of the controllers' 8000 AE each and the
/// grid buffer (GridEnergyStoragePerNode per node), plus the idle draw and the power state.
#[derive(Clone, Debug, PartialEq)]
pub struct GridPower {
    pub stored: f64,
    pub capacity: f64,
    /// drainPerTick: the nodes' idle usage.
    pub idle: f64,
    /// PathingService.getChannelPowerUsage.
    pub channel_usage: f64,
    pub has_power: bool,
    pub ticks_since_change: u32,
    /// publicHasPower (isNetworkPowered).
    pub powered: bool,
}
impl GridPower {
    /// A new grid: hasPower false, ticksSinceHasPowerChange 900, not powered.
    pub fn new(stored: f64, capacity: f64) -> Self {
        GridPower {
            stored: stored.clamp(0.0, capacity.max(0.0)),
            capacity,
            idle: 0.0,
            channel_usage: 0.0,
            has_power: false,
            ticks_since_change: 900,
            powered: false,
        }
    }
    /// getIdlePowerUsage.
    pub fn idle_usage(&self) -> f64 {
        self.idle + self.channel_usage
    }
    /// injectPower: the leftover.
    pub fn inject(&mut self, amount: f64, simulate: bool) -> f64 {
        let room = (self.capacity - self.stored).max(0.0);
        let n = amount.min(room).max(0.0);
        if !simulate {
            self.stored += n;
        }
        amount - n
    }
    /// getEnergyDemand.
    pub fn demand(&self) -> f64 {
        (self.capacity - self.stored).max(0.0)
    }
    /// onServerEndTick: idle drain and the public power state (30 ticks of power-up buffer).
    pub fn end_tick(&mut self) {
        let now = if self.idle > 0.0001 {
            let drain = self.idle_usage();
            let drained = extract_config(self, drain, false);
            drained >= self.idle - 0.001
        } else {
            extract_config(self, 0.1, true) > 0.0
        };
        if now == self.has_power {
            self.ticks_since_change = self.ticks_since_change.saturating_add(1);
        } else {
            self.ticks_since_change = 0;
        }
        self.has_power = now;
        if self.has_power && self.ticks_since_change > config::POWER_UP_DELAY {
            self.powered = true;
        } else if !self.has_power {
            self.powered = false;
        }
    }
}
impl EnergySource for GridPower {
    fn extract_ae(&mut self, amount: f64, simulate: bool) -> f64 {
        let n = amount.min(self.stored).max(0.0);
        if !simulate {
            self.stored -= n;
        }
        n
    }
}

/// AEBasePoweredBlockEntity's internal buffer (inscriber, charger, chest).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Buffer {
    pub stored: f64,
    pub max: f64,
}
impl Buffer {
    pub fn new(max: f64) -> Self {
        Buffer { stored: 0.0, max }
    }
    /// injectAEPower: the leftover.
    pub fn inject(&mut self, amount: f64, simulate: bool) -> f64 {
        let n = amount.min(self.max - self.stored).max(0.0);
        if !simulate {
            self.stored += n;
        }
        amount - n
    }
    pub fn extract(&mut self, amount: f64, simulate: bool) -> f64 {
        let n = amount.min(self.stored).max(0.0);
        if !simulate {
            self.stored -= n;
        }
        n
    }
}
impl EnergySource for Buffer {
    fn extract_ae(&mut self, amount: f64, simulate: bool) -> f64 {
        self.extract(amount, simulate)
    }
}

/// ForgeEnergyAdapter.receiveEnergy: `min(maxReceive, floor(demand in FE))`, then injected.
/// `demand_ae` = what the receiver still takes in AE. Returns the FE accepted.
pub fn fe_receive(demand_ae: f64, max_receive: i32) -> i32 {
    let demand = ae_to_fe(demand_ae.max(0.0)).floor();
    let demand = if demand > i32::MAX as f64 { i32::MAX } else { demand as i32 };
    max_receive.min(demand).max(0)
}

// ---------------------------------------------------------------------------------------------
// 4. Network topology and channels (GridNode, GridConnection, PathingService,
//    PathingCalculation, ControllerValidator).

/// What a block is on the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// ME cables: `color` 0..15 = AEColor dye order, 16 = fluix (transparent, joins every
    /// colour); dense = DENSE_CAPACITY (32 channels).
    Cable { color: u8, dense: bool },
    /// ME Controller: CANNOT_CARRY + DENSE_CAPACITY, the channel source.
    Controller,
    /// Any other block. `channel` = REQUIRE_CHANNEL; `front` = the face of a part (bus,
    /// terminal) that faces its target and does not connect.
    Device { channel: bool, front: Option<Dir> },
}
pub const FLUIX: u8 = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    pub pos: Pos,
    pub kind: NodeKind,
    /// AE/t (GridNode.idlePowerUsage).
    pub idle: f64,
}
impl NodeSpec {
    fn opens(&self, d: Dir) -> bool {
        !matches!(self.kind, NodeKind::Device { front: Some(f), .. } if f == d)
    }
    pub fn needs_channel(&self) -> bool {
        matches!(self.kind, NodeKind::Device { channel: true, .. })
    }
    /// GridNode.getMaxChannels.
    pub fn max_channels(&self) -> u32 {
        match self.kind {
            NodeKind::Controller => 0,
            NodeKind::Cable { dense: true, .. } => config::DENSE_CHANNELS,
            _ => config::CABLE_CHANNELS,
        }
    }
    fn dense(&self) -> bool {
        matches!(self.kind, NodeKind::Cable { dense: true, .. } | NodeKind::Controller)
    }
}
/// Whether `a` and its neighbour `b` (in direction `d` from `a`) join: both faces open, and two
/// cables only when their colours match or one is fluix (AEColor.matches).
pub fn connects(a: &NodeSpec, b: &NodeSpec, d: Dir) -> bool {
    if !a.opens(d) || !b.opens(d.opposite()) {
        return false;
    }
    match (a.kind, b.kind) {
        (NodeKind::Cable { color: x, .. }, NodeKind::Cable { color: y, .. }) => x == y || x == FLUIX || y == FLUIX,
        _ => true,
    }
}

/// One connected network: indices into the node list and its connections.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Network {
    pub members: Vec<usize>,
    pub edges: Vec<(usize, usize)>,
}
/// Splits the nodes into networks (Grid). Deterministic: by position order.
pub fn networks(nodes: &[NodeSpec]) -> Vec<Network> {
    let at: BTreeMap<Pos, usize> = nodes.iter().enumerate().map(|(i, n)| (n.pos, i)).collect();
    let mut seen = vec![false; nodes.len()];
    let mut out = Vec::new();
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| nodes[i].pos);
    for start in order {
        if seen[start] {
            continue;
        }
        let mut net = Network::default();
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(i) = queue.pop_front() {
            net.members.push(i);
            for d in Dir::ALL {
                let Some(&j) = at.get(&offset(nodes[i].pos, d)) else {
                    continue;
                };
                if !connects(&nodes[i], &nodes[j], d) {
                    continue;
                }
                if i < j {
                    net.edges.push((i, j));
                }
                if !seen[j] {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        net.members.sort_unstable();
        net.edges.sort_unstable();
        out.push(net);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerState {
    NoController,
    Online,
    Conflict,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdHocError {
    TooManyChannels,
}
/// The result of a channel assignment.
#[derive(Clone, Debug, PartialEq)]
pub struct Channels {
    pub state: ControllerState,
    /// channelsInUse.
    pub in_use: u32,
    /// channelsByBlocks (power usage /128).
    pub by_blocks: u32,
    /// Channels passing through each node (usedChannels; smart cable lights).
    pub used: BTreeMap<usize, u32>,
    /// Nodes that need a channel and got one (meetsChannelRequirements).
    pub granted: BTreeSet<usize>,
    pub error: Option<AdHocError>,
}

/// ControllerValidator.calculateState.
pub fn controller_state(nodes: &[NodeSpec], net: &Network) -> ControllerState {
    let controllers: Vec<usize> =
        net.members.iter().copied().filter(|&i| nodes[i].kind == NodeKind::Controller).collect();
    let Some(&first) = controllers.first() else {
        return ControllerState::NoController;
    };
    let set: BTreeSet<Pos> = controllers.iter().map(|&i| nodes[i].pos).collect();
    // Visit adjacent controllers only, growing the bounding box.
    let (mut min, mut max) = (nodes[first].pos, nodes[first].pos);
    let mut found = 0;
    let mut seen = BTreeSet::from([nodes[first].pos]);
    let mut queue = VecDeque::from([nodes[first].pos]);
    while let Some(p) = queue.pop_front() {
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
        if (0..3).any(|k| max[k] - min[k] >= config::CONTROLLER_MAX_SIZE) {
            return ControllerState::Conflict;
        }
        found += 1;
        for d in Dir::ALL {
            let q = offset(p, d);
            if set.contains(&q) && seen.insert(q) {
                queue.push_back(q);
            }
        }
    }
    if found != controllers.len() {
        return ControllerState::Conflict;
    }
    // hasControllerCross.
    for p in &set {
        let line = |a: Dir| set.contains(&offset(*p, a)) && set.contains(&offset(*p, a.opposite()));
        let n = [Dir::North, Dir::East, Dir::Up].into_iter().filter(|&d| line(d)).count();
        if n > 1 {
            return ControllerState::Conflict;
        }
    }
    ControllerState::Online
}

/// PathingService.onServerEndTick (reboot): ad-hoc channels without a controller, none on a
/// conflict, PathingCalculation with controllers.
pub fn assign_channels(nodes: &[NodeSpec], net: &Network) -> Channels {
    assign_channels_grouped(nodes, net, &BTreeMap::new())
}
/// `assign_channels` with multiblocks (GridFlags.MULTIBLOCK, a formed crafting CPU): `groups`
/// maps a node index to its multiblock; the whole multiblock needs one channel
/// (PathingService.calculateAdHocChannels ignore list, PathingCalculation.multiblocksWithChannel).
pub fn assign_channels_grouped(nodes: &[NodeSpec], net: &Network, groups: &BTreeMap<usize, usize>) -> Channels {
    let state = controller_state(nodes, net);
    let mut out = Channels {
        state,
        in_use: 0,
        by_blocks: 0,
        used: BTreeMap::new(),
        granted: BTreeSet::new(),
        error: None,
    };
    match state {
        ControllerState::NoController => {
            let need: Vec<usize> = net.members.iter().copied().filter(|&i| nodes[i].needs_channel()).collect();
            // A multiblock counts once (the rest of it is on the ignore list).
            let mut seen_groups = BTreeSet::new();
            let counted = need.iter().filter(|i| groups.get(i).is_none_or(|g| seen_groups.insert(*g))).count();
            if counted as u32 > config::AD_HOC_CHANNELS {
                out.error = Some(AdHocError::TooManyChannels);
                return out;
            }
            out.in_use = counted as u32;
            out.by_blocks = net.members.len() as u32 * out.in_use;
            for &i in &net.members {
                out.used.insert(i, out.in_use);
            }
            if out.in_use > 0 {
                out.granted = need.into_iter().collect();
            }
        }
        ControllerState::Conflict => {}
        ControllerState::Online => pathing(nodes, net, groups, &mut out),
    }
    out
}

/// A path item: a node or a connection (by edge index).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Item {
    Node(usize),
    Edge(usize),
}

/// PathingCalculation: BFS from the controllers (dense, then cables, then devices) granting
/// channels through the bottlenecks, then a DFS propagating the counts upwards.
fn pathing(nodes: &[NodeSpec], net: &Network, groups: &BTreeMap<usize, usize>, out: &mut Channels) {
    let mut with_channel: BTreeSet<usize> = BTreeSet::new();
    let edges = &net.edges;
    let mut node_edges: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (e, &(a, b)) in edges.iter().enumerate() {
        node_edges.entry(a).or_default().push(e);
        node_edges.entry(b).or_default().push(e);
    }
    let is_controller = |i: usize| nodes[i].kind == NodeKind::Controller;
    let other = |e: usize, i: usize| if edges[e].0 == i { edges[e].1 } else { edges[e].0 };
    let mut route: BTreeMap<Item, Item> = BTreeMap::new();
    let mut hsa: BTreeMap<usize, Option<usize>> = BTreeMap::new();
    let mut subtree_max: BTreeMap<usize, u32> = BTreeMap::new();
    let mut bottleneck: BTreeMap<usize, u32> = BTreeMap::new();
    let mut visited: BTreeSet<Item> = BTreeSet::new();
    let mut queues: [VecDeque<Item>; 3] = [VecDeque::new(), VecDeque::new(), VecDeque::new()];
    let queue_of = |it: Item| match it {
        Item::Edge(_) => 0,
        Item::Node(i) if nodes[i].dense() => 0,
        Item::Node(i) if matches!(nodes[i].kind, NodeKind::Cable { .. }) => 1,
        Item::Node(_) => 2,
    };
    let controllers: Vec<usize> = net.members.iter().copied().filter(|&i| is_controller(i)).collect();
    for &c in &controllers {
        visited.insert(Item::Node(c));
    }
    for &c in &controllers {
        for &e in node_edges.get(&c).map(Vec::as_slice).unwrap_or(&[]) {
            if !is_controller(other(e, c)) && visited.insert(Item::Edge(e)) {
                route.insert(Item::Edge(e), Item::Node(c));
                queues[0].push_back(Item::Edge(e));
            }
        }
    }
    for qi in 0..3 {
        while let Some(it) = queues[qi].pop_front() {
            let options: Vec<Item> = match it {
                Item::Edge(e) => vec![Item::Node(edges[e].0), Item::Node(edges[e].1)],
                Item::Node(i) => {
                    node_edges.get(&i).map(|v| v.iter().map(|&e| Item::Edge(e)).collect()).unwrap_or_default()
                }
            };
            for pi in options {
                if visited.contains(&pi) {
                    continue;
                }
                route.insert(pi, it);
                if let (Item::Node(n), Item::Edge(e)) = (pi, it) {
                    // GridNode.setControllerRoute: the parent node is the edge's own parent.
                    if let Some(&Item::Node(parent)) = route.get(&Item::Edge(e)) {
                        if is_controller(parent) {
                            hsa.insert(n, None);
                            subtree_max.insert(n, nodes[n].max_channels());
                        } else {
                            let ph = hsa.get(&parent).copied().flatten();
                            let pmax = subtree_max.get(&parent).copied().unwrap_or(0);
                            let a = match ph {
                                None => Some(parent),
                                Some(h) if subtree_max.get(&h).copied().unwrap_or(0) == pmax => Some(h),
                                Some(_) => Some(parent),
                            };
                            hsa.insert(n, a);
                            subtree_max.insert(n, pmax.min(nodes[n].max_channels()));
                        }
                    }
                    if nodes[n].needs_channel() && !with_channel.contains(&n) {
                        // tryUseChannel.
                        let mut ok = true;
                        let mut walk = Some(n);
                        while let Some(p) = walk {
                            if bottleneck.get(&p).copied().unwrap_or(0) >= nodes[p].max_channels() {
                                ok = false;
                                break;
                            }
                            walk = hsa.get(&p).copied().flatten();
                        }
                        if ok {
                            let mut walk = Some(n);
                            while let Some(p) = walk {
                                *bottleneck.entry(p).or_insert(0) += 1;
                                walk = hsa.get(&p).copied().flatten();
                            }
                            out.granted.insert(n);
                            // The rest of a multiblock rides on this channel.
                            if let Some(g) = groups.get(&n) {
                                for (&m, &h) in groups {
                                    if h == *g && m != n && net.members.contains(&m) {
                                        with_channel.insert(m);
                                    }
                                }
                            }
                        }
                    }
                }
                visited.insert(pi);
                let idx = queue_of(pi).max(qi);
                queues[idx].push_back(pi);
            }
        }
    }
    // DFS (iterative post-order): usedChannels of every node = its child connections' channels
    // (+1 if granted); a connection carries its child node's count.
    let children = |cur: Item| -> Vec<Item> {
        match cur {
            Item::Edge(e) => {
                let (a, b) = edges[e];
                let side_b = if route.get(&cur) == Some(&Item::Node(a)) { b } else { a };
                if route.get(&Item::Node(side_b)) == Some(&cur) { vec![Item::Node(side_b)] } else { vec![] }
            }
            Item::Node(i) => node_edges
                .get(&i)
                .map(|v| v.iter().map(|&e| Item::Edge(e)).filter(|x| route.get(x) == Some(&cur)).collect())
                .unwrap_or_default(),
        }
    };
    let mut by_blocks = 0u32;
    let mut value: BTreeMap<Item, u32> = BTreeMap::new();
    for &c in &controllers {
        for &e in node_edges.get(&c).map(Vec::as_slice).unwrap_or(&[]) {
            if is_controller(other(e, c)) || route.get(&Item::Edge(e)) != Some(&Item::Node(c)) {
                continue;
            }
            let mut stack: Vec<(Item, bool)> = vec![(Item::Edge(e), false)];
            while let Some((cur, done)) = stack.pop() {
                let kids = children(cur);
                if !done {
                    stack.push((cur, true));
                    for k in kids {
                        stack.push((k, false));
                    }
                    continue;
                }
                let sum: u32 = kids.iter().map(|k| value.get(k).copied().unwrap_or(0)).sum();
                let v = match cur {
                    Item::Node(i) => {
                        let v = sum + u32::from(out.granted.contains(&i));
                        out.used.insert(i, v);
                        v
                    }
                    Item::Edge(_) => sum,
                };
                by_blocks += v;
                value.insert(cur, v);
            }
        }
    }
    out.in_use = out.granted.len() as u32;
    out.by_blocks = by_blocks;
    // incrementChannelCount(1) after the DFS: granted, not counted in channelsInUse.
    for m in with_channel {
        out.granted.insert(m);
        *out.used.entry(m).or_insert(0) += 1;
    }
}

// ---------------------------------------------------------------------------------------------
// 5. Dynamic tick rates (TickManagerService, TickTracker).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickRate {
    Urgent,
    Faster,
    Slower,
    Idle,
    Sleep,
    Same,
}
/// One ticking device: AE2 calls it every `current` ticks between `min` and `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticker {
    pub min: i32,
    pub max: i32,
    pub current: i32,
    pub last: u64,
    pub next: u64,
    pub sleeping: bool,
}
impl Ticker {
    /// TickingRequest: initial rate (min + max) / 2.
    pub fn new(min: i32, max: i32, now: u64) -> Self {
        let current = (min + max) / 2;
        Ticker { min, max, current, last: now, next: now + current as u64, sleeping: false }
    }
    /// Due this tick: the ticks since the last call.
    pub fn due(&self, now: u64) -> Option<u64> {
        (!self.sleeping && now >= self.next).then(|| now - self.last)
    }
    /// After a call: TickManagerService.tickQueue's rate change (FASTER -2, SLOWER +1).
    pub fn done(&mut self, now: u64, r: TickRate) {
        self.last = now;
        let rate = match r {
            TickRate::Urgent => self.min,
            TickRate::Faster => self.current - 2,
            TickRate::Idle | TickRate::Sleep => self.max,
            TickRate::Slower => self.current + 1,
            TickRate::Same => self.current,
        };
        self.current = rate.clamp(self.min, self.max);
        self.sleeping = r == TickRate::Sleep;
        self.next = now + self.current as u64;
    }
    /// alertDevice / wakeDevice: ticks on the next tick.
    pub fn wake(&mut self, now: u64) {
        if self.sleeping {
            self.sleeping = false;
            self.next = now + 1;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 6. Machines: inscriber, charger, crank, vibration chamber.

/// A recipe ingredient: item ids, or an item tag the adapter resolves (forge tags of 1.20.1
/// renamed to `c:`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ingredient {
    pub items: &'static [&'static str],
    pub tag: Option<&'static str>,
}
impl Ingredient {
    pub fn test(&self, item: &str, in_tag: &dyn Fn(&str, &str) -> bool) -> bool {
        self.items.contains(&item) || self.tag.is_some_and(|t| in_tag(t, item))
    }
}
const fn ing(items: &'static [&'static str]) -> Ingredient {
    Ingredient { items, tag: None }
}
const fn tag(items: &'static [&'static str], t: &'static str) -> Ingredient {
    Ingredient { items, tag: Some(t) }
}

/// An `ae2:inscriber` recipe (data/ae2/recipes/inscriber/*.json of the 15.4.11 jar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InscriberRecipe {
    pub id: &'static str,
    pub top: Option<Ingredient>,
    pub middle: Ingredient,
    pub bottom: Option<Ingredient>,
    /// mode "press": the plates are used up too (InscriberProcessType.PRESS).
    pub press: bool,
    pub result: &'static str,
}
const CERTUS: &[&str] = &["ae2:certus_quartz_crystal", "ae2:charged_certus_quartz_crystal"];
pub const INSCRIBER_RECIPES: &[InscriberRecipe] = &[
    InscriberRecipe { id: "calculation_processor", top: Some(ing(&["ae2:printed_calculation_processor"])), middle: ing(&["minecraft:redstone"]), bottom: Some(ing(&["ae2:printed_silicon"])), press: true, result: "ae2:calculation_processor" },
    InscriberRecipe { id: "calculation_processor_press", top: Some(ing(&["ae2:calculation_processor_press"])), middle: ing(&["minecraft:iron_block"]), bottom: None, press: false, result: "ae2:calculation_processor_press" },
    InscriberRecipe { id: "calculation_processor_print", top: Some(ing(&["ae2:calculation_processor_press"])), middle: ing(&["ae2:certus_quartz_crystal"]), bottom: None, press: false, result: "ae2:printed_calculation_processor" },
    InscriberRecipe { id: "certus_quartz_dust", top: None, middle: tag(CERTUS, "c:gems/certus_quartz"), bottom: None, press: false, result: "ae2:certus_quartz_dust" },
    InscriberRecipe { id: "ender_dust", top: None, middle: ing(&["minecraft:ender_pearl"]), bottom: None, press: false, result: "ae2:ender_dust" },
    InscriberRecipe { id: "engineering_processor", top: Some(ing(&["ae2:printed_engineering_processor"])), middle: ing(&["minecraft:redstone"]), bottom: Some(ing(&["ae2:printed_silicon"])), press: true, result: "ae2:engineering_processor" },
    InscriberRecipe { id: "engineering_processor_press", top: Some(ing(&["ae2:engineering_processor_press"])), middle: ing(&["minecraft:iron_block"]), bottom: None, press: false, result: "ae2:engineering_processor_press" },
    InscriberRecipe { id: "engineering_processor_print", top: Some(ing(&["ae2:engineering_processor_press"])), middle: tag(&["minecraft:diamond"], "c:gems/diamond"), bottom: None, press: false, result: "ae2:printed_engineering_processor" },
    InscriberRecipe { id: "fluix_dust", top: None, middle: tag(&["ae2:fluix_crystal"], "c:gems/fluix"), bottom: None, press: false, result: "ae2:fluix_dust" },
    InscriberRecipe { id: "logic_processor", top: Some(ing(&["ae2:printed_logic_processor"])), middle: ing(&["minecraft:redstone"]), bottom: Some(ing(&["ae2:printed_silicon"])), press: true, result: "ae2:logic_processor" },
    InscriberRecipe { id: "logic_processor_press", top: Some(ing(&["ae2:logic_processor_press"])), middle: ing(&["minecraft:iron_block"]), bottom: None, press: false, result: "ae2:logic_processor_press" },
    InscriberRecipe { id: "logic_processor_print", top: Some(ing(&["ae2:logic_processor_press"])), middle: tag(&["minecraft:gold_ingot"], "c:ingots/gold"), bottom: None, press: false, result: "ae2:printed_logic_processor" },
    InscriberRecipe { id: "silicon_press", top: Some(ing(&["ae2:silicon_press"])), middle: ing(&["minecraft:iron_block"]), bottom: None, press: false, result: "ae2:silicon_press" },
    InscriberRecipe { id: "silicon_print", top: Some(ing(&["ae2:silicon_press"])), middle: tag(&["ae2:silicon"], "c:silicon"), bottom: None, press: false, result: "ae2:printed_silicon" },
    InscriberRecipe { id: "sky_stone_dust", top: None, middle: ing(&["ae2:sky_stone_block"]), bottom: None, press: false, result: "ae2:sky_dust" },
];
/// InscriberRecipes.findRecipe with mirroring: empty plates match only recipes without them.
pub fn find_inscriber(
    top: Option<&str>,
    middle: &str,
    bottom: Option<&str>,
    in_tag: &dyn Fn(&str, &str) -> bool,
) -> Option<&'static InscriberRecipe> {
    let plate = |want: &Option<Ingredient>, have: Option<&str>| match (want, have) {
        (None, None) => true,
        (Some(i), Some(h)) => i.test(h, in_tag),
        _ => false,
    };
    INSCRIBER_RECIPES.iter().find(|r| {
        r.middle.test(middle, in_tag)
            && ((plate(&r.top, top) && plate(&r.bottom, bottom)) || (plate(&r.top, bottom) && plate(&r.bottom, top)))
    })
}
/// The inscriber's slot filters (InscriberBlockEntity.BaseFilter): a plate slot takes what is a
/// plate of some recipe, the middle slot what is a middle input.
pub fn inscriber_plate_ok(item: &str, in_tag: &dyn Fn(&str, &str) -> bool) -> bool {
    INSCRIBER_RECIPES
        .iter()
        .any(|r| r.top.is_some_and(|i| i.test(item, in_tag)) || r.bottom.is_some_and(|i| i.test(item, in_tag)))
}
pub fn inscriber_middle_ok(item: &str, in_tag: &dyn Fn(&str, &str) -> bool) -> bool {
    INSCRIBER_RECIPES.iter().any(|r| r.middle.test(item, in_tag))
}

/// The inscriber's work state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inscriber {
    pub progress: i32,
    pub smash: bool,
    pub final_step: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InscriberStep {
    Nothing,
    /// finalStep 8: put the result out and use up the inputs.
    Craft,
}
/// InscriberBlockEntity.tickingRequest (every tick: InscriberMin 1, URGENT while working).
/// `has_work` = a recipe matches and its result fits the output; speed factor 2 without cards
/// (116 ticks), 20 AE per step from the own buffer first, else from the grid.
pub fn inscriber_tick(
    st: &mut Inscriber,
    has_work: bool,
    buffer: &mut Buffer,
    grid: Option<&mut dyn EnergySource>,
) -> InscriberStep {
    if st.smash {
        st.final_step += 1;
        if st.final_step == 8 {
            return InscriberStep::Craft;
        } else if st.final_step == 16 {
            st.final_step = 0;
            st.smash = false;
        }
        return InscriberStep::Nothing;
    }
    if !has_work {
        return InscriberStep::Nothing;
    }
    let speed = 2;
    let cost = 10.0 * speed as f64;
    let threshold = cost - 0.01;
    if extract_config(buffer, cost, true) > threshold {
        extract_config(buffer, cost, false);
        st.progress += speed;
    } else if let Some(g) = grid
        && extract_config(g, cost, true) > threshold
    {
        extract_config(g, cost, false);
        st.progress += speed;
    }
    if st.progress > config::INSCRIBER_MAX_STEPS {
        st.progress = config::INSCRIBER_MAX_STEPS;
        // The result still fits (has_work): start the press animation.
        st.smash = true;
        st.final_step = 0;
    }
    InscriberStep::Nothing
}

/// ChargerRecipes (data/ae2/recipes/charger/*.json of the 15.4.11 jar).
pub const CHARGER_RECIPES: &[(&str, &str)] = &[
    ("ae2:certus_quartz_crystal", "ae2:charged_certus_quartz_crystal"),
    ("minecraft:book", "ae2:guide"),
    ("minecraft:compass", "ae2:meteorite_compass"),
];
pub fn charger_result(item: &str) -> Option<&'static str> {
    CHARGER_RECIPES.iter().find(|r| r.0 == item).map(|r| r.1)
}
/// ChargerRecipes.allowExtract: anything that is not an input of a recipe.
pub fn charger_may_take(item: &str) -> bool {
    charger_result(item).is_none()
}
/// ChargerBlockEntity.doWork (every 10 ticks): with a full buffer (> 1599), 20% chance per call
/// to use the whole buffer and charge the item; then refill up to 800 AE from the grid.
/// `roll` = random.nextFloat(). Returns the charged result when it happened.
pub fn charger_tick(
    buffer: &mut Buffer,
    item: Option<&str>,
    roll: f32,
    grid: Option<&mut dyn EnergySource>,
) -> Option<&'static str> {
    let mut out = None;
    if let Some(item) = item
        && buffer.stored > config::CHARGER_BUFFER - 1.0
        && let Some(result) = charger_result(item)
        && roll > 0.8
    {
        let all = buffer.max;
        extract_config(buffer, all, false);
        out = Some(result);
    }
    if buffer.stored < config::CHARGER_BUFFER - 1.0
        && let Some(g) = grid
    {
        let want = (buffer.max - buffer.stored).min(800.0);
        let got = g.extract_ae(want, false);
        buffer.inject(got, false);
    }
    out
}

/// CrankBlockEntity: a turn adds 18 ticks of rotation (only when < 3 left and the machine can
/// take power); each full rotation gives 160 AE to the machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crank {
    pub rotation: i32,
    pub charge: i32,
    pub hits: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrankUse {
    Turned,
    /// Nothing to turn (the machine is full): 10 such hits break the crank.
    Refused,
    Break,
    Busy,
}
impl Crank {
    /// CrankBlockEntity.power: `can_turn` = the machine's buffer is not full.
    pub fn turn(&mut self, can_turn: bool) -> CrankUse {
        if self.rotation >= 3 {
            return CrankUse::Busy;
        }
        if can_turn {
            self.hits = 0;
            self.rotation += config::CRANK_TICKS_PER_ROTATION;
            CrankUse::Turned
        } else {
            self.hits += 1;
            if self.hits > 10 { CrankUse::Break } else { CrankUse::Refused }
        }
    }
    /// CrankBlockEntity.tick: true when a full turn is applied this tick (`applyTurn`).
    pub fn tick(&mut self) -> bool {
        if self.rotation <= 0 {
            return false;
        }
        self.charge += 1;
        let applied = if self.charge >= config::CRANK_TICKS_PER_ROTATION {
            self.charge -= config::CRANK_TICKS_PER_ROTATION;
            true
        } else {
            false
        };
        self.rotation -= 1;
        applied
    }
}

/// VibrationChamberBlockEntity without energy cards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vibration {
    pub remaining: f64,
    pub item_ticks: f64,
    pub rate: f64,
}
impl Default for Vibration {
    fn default() -> Self {
        Vibration { remaining: 0.0, item_ticks: 0.0, rate: Vibration::initial_rate() }
    }
}
impl Vibration {
    fn energy_per_fuel_tick() -> f64 {
        config::VIBRATION_CHAMBER_BASE_ENERGY_PER_FUEL_TICK
    }
    pub fn min_rate() -> f64 {
        config::VIBRATION_CHAMBER_MIN_ENERGY_PER_GAME_TICK as f64 / Self::energy_per_fuel_tick()
    }
    pub fn max_rate() -> f64 {
        config::VIBRATION_CHAMBER_BASE_MAX_ENERGY_PER_GAME_TICK as f64 / Self::energy_per_fuel_tick()
    }
    /// initialEnergyRate = clamp(baseEnergyPerFuelTick, min, max) per game tick.
    pub fn initial_rate() -> f64 {
        let min = config::VIBRATION_CHAMBER_MIN_ENERGY_PER_GAME_TICK as f64;
        let max = config::VIBRATION_CHAMBER_BASE_MAX_ENERGY_PER_GAME_TICK as f64;
        Self::energy_per_fuel_tick().clamp(min, max) / Self::energy_per_fuel_tick()
    }
    fn set_rate(&mut self, r: f64) {
        self.rate = r.clamp(Self::min_rate(), Self::max_rate());
    }
    /// eatFuel: a fuel item of `burn` ticks enters the chamber.
    pub fn eat(&mut self, burn: i32) {
        self.remaining += burn as f64;
        self.item_ticks = self.remaining;
    }
    pub fn burning(&self) -> bool {
        self.remaining > 0.0
    }
    /// tickingRequest after the fuel check (`remaining > 0`). `grid` takes the power (the
    /// overflow is voided, as in AE2) and the rate adapts within 100 ticks.
    pub fn tick(&mut self, ticks: u64, grid: &mut GridPower) -> TickRate {
        let mut consumed = ticks as f64 * self.rate;
        self.remaining -= consumed;
        if self.remaining < 0.0 {
            consumed += self.remaining;
            self.remaining = 0.0;
        }
        let step = ticks as f64 * (Self::max_rate() - Self::min_rate()) / 100.0;
        if consumed.abs() < 0.01 {
            if grid.inject(1.0, true) == 0.0 {
                self.set_rate(self.rate + step);
                return TickRate::Faster;
            }
            return TickRate::Idle;
        }
        let overflow = grid.inject(consumed * Self::energy_per_fuel_tick(), false);
        if overflow > 0.0 {
            self.set_rate(self.rate - step);
            TickRate::Slower
        } else {
            self.set_rate(self.rate + step);
            TickRate::Faster
        }
    }
    /// The fuel is gone and none is left: back to the initial rate (SLEEP).
    pub fn out_of_fuel(&mut self) {
        self.rate = Self::initial_rate();
    }
}

// ---------------------------------------------------------------------------------------------
// 7. ME Interface stocking (InterfaceLogic.updatePlan / tryUsePlan, no crafting).

/// What one interface slot must do: `> 0` bring that many from the network, `< 0` send back.
pub fn interface_plan(config: Option<(&Key, u64)>, stored: Option<(&Key, u64)>) -> Option<(Key, i64)> {
    match (config, stored) {
        (None, Some((k, n))) => Some((k.clone(), -(n as i64))),
        (None, None) => None,
        (Some((k, n)), None) => Some((k.clone(), n as i64)),
        (Some((ck, cn)), Some((sk, sn))) if ck == sk => {
            (cn != sn).then(|| (ck.clone(), cn as i64 - sn as i64))
        }
        (Some(_), Some((sk, sn))) => Some((sk.clone(), -(sn as i64))),
    }
}

// ---------------------------------------------------------------------------------------------
// 8. Terminal: entries, search (RepoSearch), sort (KeySorters / Repo), amounts.

/// One line of the terminal's repo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub key: Key,
    pub amount: u64,
    /// Display name (getDisplayName().getString()).
    pub name: String,
    /// Item id (`minecraft:stone`).
    pub id: String,
    pub mod_id: String,
    pub mod_name: String,
    /// Tooltip lines (TooltipsSearchPredicate).
    pub tooltip: String,
    /// Item tags (`minecraft:logs`).
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortBy {
    #[default]
    Name,
    Amount,
    Mod,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortDir {
    #[default]
    Ascending,
    Descending,
}
impl SortBy {
    pub fn next(self) -> Self {
        match self {
            SortBy::Name => SortBy::Amount,
            SortBy::Amount => SortBy::Mod,
            SortBy::Mod => SortBy::Name,
        }
    }
    pub fn code(self) -> i64 {
        self as i64
    }
    pub fn from_code(c: i64) -> Self {
        match c {
            1 => SortBy::Amount,
            2 => SortBy::Mod,
            _ => SortBy::Name,
        }
    }
}

/// RepoSearch.getPredicates of one term.
fn term_matches(e: &Entry, part: &str) -> bool {
    if let Some(t) = part.strip_prefix('@') {
        // ModSearchPredicate: mod id, then mod name.
        e.mod_id.to_lowercase().contains(t) || e.mod_name.to_lowercase().contains(t)
    } else if let Some(t) = part.strip_prefix('#') {
        // TooltipsSearchPredicate: lower case, without spaces.
        let t = t.replace(' ', "");
        e.tooltip.to_lowercase().replace(' ', "").contains(&t)
    } else if let Some(t) = part.strip_prefix('$') {
        // TagSearchPredicate.
        e.tags.iter().any(|tag| {
            if t.contains(':') {
                tag.contains(t)
            } else {
                let (ns, path) = tag.split_once(':').unwrap_or(("", tag));
                ns.contains(t) || path.contains(t)
            }
        })
    } else if let Some(t) = part.strip_prefix('*') {
        e.id.to_lowercase().contains(t)
    } else {
        e.name.to_lowercase().contains(part)
    }
}
/// RepoSearch.fromString: `|` = or, spaces = and, all lower case.
pub fn search_matches(e: &Entry, search: &str) -> bool {
    search.split('|').any(|or| {
        let q = or.to_lowercase();
        let terms: Vec<&str> = q.split_whitespace().collect();
        // String.split("\\s+") of "" gives [""]: a NameSearchPredicate("") matches all.
        terms.iter().all(|t| term_matches(e, t))
    })
}
/// String.compareToIgnoreCase (char by char, upper then lower case).
fn cmp_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    let fold = |c: char| c.to_uppercase().next().unwrap_or(c).to_lowercase().next().unwrap_or(c);
    let mut ia = a.chars().map(fold);
    let mut ib = b.chars().map(fold);
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x != y => return x.cmp(&y),
            _ => {}
        }
    }
}
/// Repo.updateView: filter by the search, then a stable sort (ties keep key order).
pub fn view(entries: &[Entry], search: &str, by: SortBy, dir: SortDir) -> Vec<Entry> {
    view_keep(entries, &|e| e.amount > 0, search, by, dir)
}
/// `view` keeping what `keep` says (stage 2: craftable entries with nothing stored too,
/// ViewItems.ALL).
pub fn view_keep(entries: &[Entry], keep: &dyn Fn(&Entry) -> bool, search: &str, by: SortBy, dir: SortDir) -> Vec<Entry> {
    let mut v: Vec<Entry> = entries.iter().filter(|e| keep(e) && search_matches(e, search)).cloned().collect();
    v.sort_by(|a, b| a.key.cmp(&b.key));
    let name = |a: &Entry, b: &Entry| cmp_ignore_case(&a.name, &b.name);
    match (by, dir) {
        (SortBy::Amount, SortDir::Ascending) => v.sort_by(|a, b| a.amount.cmp(&b.amount)),
        (SortBy::Amount, SortDir::Descending) => v.sort_by(|a, b| b.amount.cmp(&a.amount)),
        (SortBy::Name, SortDir::Ascending) => v.sort_by(name),
        (SortBy::Name, SortDir::Descending) => v.sort_by(|a, b| name(b, a)),
        (SortBy::Mod, d) => {
            let m = |a: &Entry, b: &Entry| cmp_ignore_case(&a.mod_id, &b.mod_id).then_with(|| name(a, b));
            if d == SortDir::Ascending { v.sort_by(m) } else { v.sort_by(|a, b| m(b, a)) }
        }
    }
    v
}
/// ReadableNumberConverter.format(number, width): the slot amount (width 4, small font).
pub fn format_amount(n: u64, width: usize) -> String {
    let s = n.to_string();
    if s.len() <= width {
        return s;
    }
    let mut base = n;
    let mut last = base as f64 * 1000.0;
    let mut exponent: i32 = -1;
    let mut size = s.len();
    let mut post = ' ';
    while size > width {
        last = base as f64;
        base /= 1000;
        exponent += 1;
        size = base.to_string().len() + 1;
        post = ['K', 'M', 'G', 'T', 'P', 'E'][exponent as usize];
    }
    // DecimalFormat(".#") rounding DOWN: one decimal, no leading zero, no ".0".
    let v = last / 1000.0;
    let tenths = (v * 10.0).floor() as u64;
    let (int, frac) = (tenths / 10, tenths % 10);
    let mut with = String::new();
    if int > 0 {
        with.push_str(&int.to_string());
    }
    if frac > 0 {
        with.push('.');
        with.push_str(&frac.to_string());
    }
    if with.is_empty() {
        with.push('0');
    }
    with.push(post);
    if with.len() <= width { with } else { format!("{base}{post}") }
}
/// MEStorageScreen.init: rows = max(2, style rows of (screen height - 2 * margin)).
pub fn terminal_rows(gui_height: i32, header: i32, bottom: i32) -> i32 {
    let available = gui_height - 2 * config::CLIENT_TERMINAL_MARGIN as i32;
    let possible = (available - header - bottom) / 18;
    // TerminalStyle.SMALL: maxRows * 1 / 4.
    let style = possible.max(0) / 4;
    style.max(config::TERMINAL_MIN_ROWS)
}

// ---------------------------------------------------------------------------------------------
// 9. World rules: certus growth, in-water transforms, meteorites.

/// BuddingCertusQuartzBlock.randomTick. `face` = the random direction, `target` = what is there
/// (`None` = air or a full water source), `bud` = the bud stage there facing `face`
/// (0 small .. 2 large). Returns (the new bud stage to put, decay the budding block).
pub fn budding_tick(
    budding: &str,
    growth_roll: i32,
    target_free: bool,
    bud: Option<u8>,
    decay_roll: i32,
) -> Option<(u8, Option<&'static str>)> {
    if growth_roll != 0 {
        return None;
    }
    let next = if target_free {
        0
    } else {
        match bud {
            Some(s @ 0..=2) => s + 1,
            _ => return None,
        }
    };
    let decayed = if budding == "ae2:flawless_budding_quartz" || decay_roll != 0 {
        None
    } else {
        match budding {
            "ae2:flawed_budding_quartz" => Some("ae2:chipped_budding_quartz"),
            "ae2:chipped_budding_quartz" => Some("ae2:damaged_budding_quartz"),
            "ae2:damaged_budding_quartz" => Some("ae2:quartz_block"),
            _ => None,
        }
    };
    Some((next, decayed))
}
pub const BUDDING: [&str; 4] = [
    "ae2:flawless_budding_quartz",
    "ae2:flawed_budding_quartz",
    "ae2:chipped_budding_quartz",
    "ae2:damaged_budding_quartz",
];
pub const BUDS: [&str; 4] = ["ae2:small_quartz_bud", "ae2:medium_quartz_bud", "ae2:large_quartz_bud", "ae2:quartz_cluster"];

/// The in-water `ae2:transform` recipes of the 15.4.11 jar: every ingredient (one item each)
/// lying in the same water block makes the result.
pub const TRANSFORMS: &[(&[&str], &str, u32)] = &[
    (&["ae2:charged_certus_quartz_crystal", "minecraft:redstone", "minecraft:quartz"], "ae2:fluix_crystal", 2),
    (&["ae2:charged_certus_quartz_crystal", "ae2:certus_quartz_dust"], "ae2:certus_quartz_crystal", 2),
    (&["ae2:charged_certus_quartz_crystal", "ae2:fluix_dust"], "ae2:fluix_crystal", 1),
    (&["ae2:charged_certus_quartz_crystal", "ae2:quartz_block"], "ae2:damaged_budding_quartz", 1),
    (&["ae2:charged_certus_quartz_crystal", "ae2:damaged_budding_quartz"], "ae2:chipped_budding_quartz", 1),
    (&["ae2:charged_certus_quartz_crystal", "ae2:chipped_budding_quartz"], "ae2:flawed_budding_quartz", 1),
];
/// TransformLogic.tryTransform over the items in one water block (id, count, entity index):
/// the first recipe whose ingredients are all present; returns (one entity per ingredient,
/// result, count). Recipe order = the jar's file order is not kept by AE2 (a map), so the
/// longest recipes are tried first: they need the most items.
pub fn find_transform(items: &[(String, u32, usize)]) -> Option<(Vec<usize>, &'static str, u32)> {
    let mut recipes: Vec<&(&[&str], &str, u32)> = TRANSFORMS.iter().collect();
    recipes.sort_by_key(|r| std::cmp::Reverse(r.0.len()));
    'r: for (ingredients, result, count) in recipes {
        let mut used = Vec::new();
        for want in ingredients.iter() {
            match items.iter().find(|(id, n, i)| id == want && *n > 0 && !used.contains(i)) {
                Some((_, _, i)) => used.push(*i),
                None => continue 'r,
            }
        }
        return Some((used, result, *count));
    }
    None
}

/// A small seeded generator (no world seed is passed to native worldgen on this pin).
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x5DEE_CE66_D)
    }
    pub fn next_u64(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn next_int(&mut self, bound: i32) -> i32 {
        (self.next_u64() % bound.max(1) as u64) as i32
    }
    pub fn next_float(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The meteorite of a 32x32-chunk region (structure_set ae2:meteorite: random_spread, spacing
/// 32, separation 8, salt 124895654): its chunk, centre column and core radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meteor {
    pub chunk: (i32, i32),
    pub x: i32,
    pub z: i32,
    /// MeteoriteStructure: random.nextFloat() * 6 + 2.
    pub radius: f32,
}
pub fn meteor_of_region(rx: i32, rz: i32) -> Meteor {
    let mut r = Rng::new((rx as i64 as u64).wrapping_mul(341_873_128_712) ^ (rz as i64 as u64).wrapping_mul(132_897_987_541) ^ 124_895_654);
    let cx = rx * 32 + r.next_int(32 - 8);
    let cz = rz * 32 + r.next_int(32 - 8);
    let x = cx * 16 + r.next_int(16);
    let z = cz * 16 + r.next_int(16);
    let radius = r.next_float() * 6.0 + 2.0;
    Meteor { chunk: (cx, cz), x, z, radius }
}
/// The meteorite whose centre is in chunk (cx, cz), if any.
pub fn meteor_in_chunk(cx: i32, cz: i32) -> Option<Meteor> {
    let m = meteor_of_region(cx.div_euclid(32), cz.div_euclid(32));
    (m.chunk == (cx, cz)).then_some(m)
}
/// What MeteoritePlacer puts at an offset from the centre (`dy` up): sky stone in the
/// ellipsoid, the 3x3x3 room with the certus floor, the mysterious cube in the middle, air
/// in the crater above. `None` = leave the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeteorBlock {
    SkyStone,
    Air,
    /// The room's floor: one of the quartz blocks (index into METEOR_QUARTZ).
    Quartz,
    Cube,
    Keep,
}
pub const METEOR_QUARTZ: [&str; 5] = [
    "ae2:quartz_block",
    "ae2:damaged_budding_quartz",
    "ae2:chipped_budding_quartz",
    "ae2:flawed_budding_quartz",
    "ae2:flawless_budding_quartz",
];
pub fn meteor_block(radius: f32, dx: i32, dy: i32, dz: i32) -> MeteorBlock {
    let sq = (radius * radius) as f64;
    let (fx, fy, fz) = (dx as f64, dy as f64, dz as f64);
    let k = if dy > 0 { 1.4 } else { 0.8 };
    if dx.abs() <= 8 && dz.abs() <= 8 && (-8..8).contains(&dy) && fx * fx * 0.7 + fy * fy * k + fz * fz * 0.7 < sq {
        if dx.abs() <= 1 && dy.abs() <= 1 && dz.abs() <= 1 {
            if dx == 0 && dy == 0 && dz == 0 {
                return if config::WORLD_GEN_SPAWN_PRESSES_IN_METEORITES { MeteorBlock::Cube } else { MeteorBlock::Air };
            }
            return if dy == -1 { MeteorBlock::Quartz } else { MeteorBlock::Air };
        }
        return MeteorBlock::SkyStone;
    }
    // placeCrater (CraterType.NORMAL): air above h + distance² * 0.02, h = y - radius + 1.
    let h = -(radius as f64) + 1.0;
    let crater = radius as f64 * 2.0 + 5.0;
    let dist = fx * fx + fz * fz;
    if dy >= -5 && dist <= crater * crater && fy > h + dist * 0.02 {
        return MeteorBlock::Air;
    }
    MeteorBlock::Keep
}

// ---------------------------------------------------------------------------------------------
// 10. The world's AE2 state (one record per AE2 block), saved in <world>/data/ae2/<dim>.txt.

/// One AE2 block of the world: its id, facing, energy, real items, ghost config and numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    pub block: String,
    pub facing: Option<Dir>,
    /// Its share of the grid's energy (controller 8000 + 25 per node), or its own buffer
    /// (inscriber, charger, chest: key "buffer" in `f`).
    pub energy: f64,
    /// Real items held (cells, interface storage, machine slots, crafting grid).
    pub slots: Vec<Option<(Key, u32)>>,
    /// The contents of the cells in `slots` (by slot index).
    pub cells: BTreeMap<usize, CellInv>,
    /// Ghost config (interface stock amounts, bus filters, storage bus partition).
    pub config: Vec<Option<(Key, u64)>>,
    pub n: BTreeMap<String, i64>,
    pub f: BTreeMap<String, f64>,
    /// Named key lists (stage 2: a crafting CPU's inventory and job, a pattern provider's send
    /// list...). A name starting with `!` holds real items: they spill when the block is gone.
    pub lists: BTreeMap<String, Vec<(Key, u64)>>,
}
impl Device {
    pub fn new(block: &str, slots: usize, config: usize) -> Self {
        Device {
            block: block.to_owned(),
            slots: vec![None; slots],
            config: vec![None; config],
            ..Default::default()
        }
    }
    pub fn num(&self, k: &str) -> i64 {
        self.n.get(k).copied().unwrap_or(0)
    }
    pub fn set_num(&mut self, k: &str, v: i64) {
        if v == 0 {
            self.n.remove(k);
        } else {
            self.n.insert(k.to_owned(), v);
        }
    }
    pub fn fl(&self, k: &str) -> f64 {
        self.f.get(k).copied().unwrap_or(0.0)
    }
    pub fn set_fl(&mut self, k: &str, v: f64) {
        if v == 0.0 {
            self.f.remove(k);
        } else {
            self.f.insert(k.to_owned(), v);
        }
    }
    /// Every real item, for spilling when the block is gone (cells keep their contents: the
    /// adapter writes `cells[i]` into the cell item first).
    pub fn take_items(&mut self) -> Vec<(usize, Key, u32)> {
        let mut out = Vec::new();
        for (i, s) in self.slots.iter_mut().enumerate() {
            if let Some((k, n)) = s.take() {
                out.push((i, k, n));
            }
        }
        out
    }
    /// The real items of the `!` lists (taken out of the record).
    pub fn take_list_items(&mut self) -> Vec<(Key, u64)> {
        let names: Vec<String> = self.lists.keys().filter(|k| k.starts_with('!')).cloned().collect();
        let mut out = Vec::new();
        for n in names {
            if let Some(l) = self.lists.remove(&n) {
                out.extend(l.into_iter().filter(|(_, c)| *c > 0));
            }
        }
        out
    }
    pub fn list(&self, name: &str) -> &[(Key, u64)] {
        self.lists.get(name).map_or(&[], |v| v.as_slice())
    }
    pub fn set_list(&mut self, name: &str, v: Vec<(Key, u64)>) {
        let v: Vec<(Key, u64)> = v.into_iter().filter(|(_, n)| *n > 0).collect();
        if v.is_empty() {
            self.lists.remove(name);
        } else {
            self.lists.insert(name.to_owned(), v);
        }
    }
    /// Grows the record to a newer layout (more slots or config); never shrinks (nothing lost).
    pub fn grow(&mut self, slots: usize, config: usize) {
        if self.slots.len() < slots {
            self.slots.resize(slots, None);
        }
        if self.config.len() < config {
            self.config.resize(config, None);
        }
    }
}

/// The records by place (whole blocks and cable-bus cables by position, parts by position and face).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Devices(pub BTreeMap<Loc, Device>);
impl Devices {
    /// Adds a record (a plain position means the whole block / the cable bus's cable).
    pub fn insert(&mut self, at: impl Into<Loc>, d: Device) -> Option<Device> {
        self.0.insert(at.into(), d)
    }
    pub fn at(&self, at: impl Into<Loc>) -> Option<&Device> {
        self.0.get(&at.into())
    }
    pub fn at_mut(&mut self, at: impl Into<Loc>) -> Option<&mut Device> {
        self.0.get_mut(&at.into())
    }
}
impl std::ops::Deref for Devices {
    type Target = BTreeMap<Loc, Device>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for Devices {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// All AE2 blocks and parts of one world and dimension, and items waiting to drop.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub devices: Devices,
    /// A gone block's items (and cell contents), saved until the game spawns them.
    pub drops: Vec<(Pos, Key, u32)>,
}
fn f64_text(v: f64) -> String {
    // Shortest round-trip text of an f64 (Rust's Display is exact for parsing back).
    format!("{v}")
}
impl State {
    /// The side file: `D x y z block facing energy`, then `S i key n` (slot), `L i key n` (a cell
    /// line of slot i), `C i key n` (config), `N name v`, `F name v`; `X x y z key n` = a drop.
    pub fn save(&self) -> String {
        let mut o = String::new();
        for (l, d) in self.devices.iter() {
            let p = l.pos;
            let facing = d.facing.map_or("-", |f| f.name());
            // A part of a cable bus: its face as a 10th word.
            let side = l.side.map(|f| format!(" {}", f.name())).unwrap_or_default();
            o.push_str(&format!("D {} {} {} {} {} {} {} {}{side}\n", p[0], p[1], p[2], d.block, facing, f64_text(d.energy), d.slots.len(), d.config.len()));
            for (i, s) in d.slots.iter().enumerate() {
                if let Some((k, n)) = s {
                    o.push_str(&format!("S {i} {k} {n}\n"));
                }
            }
            for (i, c) in &d.cells {
                for (k, n) in &c.items {
                    o.push_str(&format!("L {i} {k} {n}\n"));
                }
            }
            for (i, s) in d.config.iter().enumerate() {
                if let Some((k, n)) = s {
                    o.push_str(&format!("C {i} {k} {n}\n"));
                }
            }
            for (k, v) in &d.n {
                o.push_str(&format!("N {k} {v}\n"));
            }
            for (k, v) in &d.f {
                o.push_str(&format!("F {k} {}\n", f64_text(*v)));
            }
            for (name, l) in &d.lists {
                for (k, n) in l {
                    o.push_str(&format!("E {name} {k} {n}\n"));
                }
            }
        }
        for (p, k, n) in &self.drops {
            o.push_str(&format!("X {} {} {} {k} {n}\n", p[0], p[1], p[2]));
        }
        o
    }
    /// Reads the side file; unreadable lines are skipped (a broken file never breaks the game).
    pub fn load(text: Option<String>) -> Self {
        let mut s = State::default();
        let mut cur: Option<Loc> = None;
        for line in text.unwrap_or_default().lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            let int = |i: usize| w.get(i).and_then(|v| v.parse::<i64>().ok());
            match w.first().copied() {
                Some("D") if w.len() == 9 || w.len() == 10 => {
                    let (Some(x), Some(y), Some(z), Some(ns), Some(nc)) = (int(1), int(2), int(3), int(7), int(8)) else {
                        cur = None;
                        continue;
                    };
                    // A 10th word: the face of a cable-bus part.
                    let side = match w.get(9) {
                        Some(t) => match Dir::parse(t) {
                            Some(d) => Some(d),
                            None => {
                                cur = None;
                                continue;
                            }
                        },
                        None => None,
                    };
                    let p = Loc { pos: [x as i32, y as i32, z as i32], side };
                    let mut d = Device::new(w[4], ns.clamp(0, 256) as usize, nc.clamp(0, 256) as usize);
                    d.facing = Dir::parse(w[5]);
                    d.energy = w[6].parse().unwrap_or(0.0);
                    s.devices.insert(p, d);
                    cur = Some(p);
                }
                Some(t @ ("S" | "L" | "C")) if w.len() == 4 => {
                    let Some(d) = cur.and_then(|p| s.devices.get_mut(&p)) else {
                        continue;
                    };
                    let (Some(i), Some(n)) = (int(1), w[3].parse::<u64>().ok()) else {
                        continue;
                    };
                    let i = i as usize;
                    match t {
                        "S" if i < d.slots.len() && n > 0 => d.slots[i] = Some((w[2].to_owned(), n.min(u32::MAX as u64) as u32)),
                        "L" if n > 0 => d.cells.entry(i).or_default().items.push((w[2].to_owned(), n)),
                        "C" if i < d.config.len() => d.config[i] = Some((w[2].to_owned(), n)),
                        _ => {}
                    }
                }
                Some("N") if w.len() == 3 => {
                    if let (Some(d), Some(v)) = (cur.and_then(|p| s.devices.get_mut(&p)), int(2)) {
                        d.n.insert(w[1].to_owned(), v);
                    }
                }
                Some("F") if w.len() == 3 => {
                    if let (Some(d), Ok(v)) = (cur.and_then(|p| s.devices.get_mut(&p)), w[2].parse::<f64>()) {
                        d.f.insert(w[1].to_owned(), v);
                    }
                }
                Some("E") if w.len() == 4 => {
                    if let (Some(d), Ok(n)) = (cur.and_then(|p| s.devices.get_mut(&p)), w[3].parse::<u64>())
                        && n > 0
                    {
                        d.lists.entry(w[1].to_owned()).or_default().push((w[2].to_owned(), n));
                    }
                }
                Some("X") if w.len() == 6 => {
                    if let (Some(x), Some(y), Some(z), Some(n)) = (int(1), int(2), int(3), int(5))
                        && n > 0
                    {
                        s.drops.push(([x as i32, y as i32, z as i32], w[4].to_owned(), n as u32));
                    }
                }
                _ => {}
            }
        }
        s
    }
}

// ---------------------------------------------------------------------------------------------
// 11. The cable bus (CableBusBlock / CableBusContainer): every cable and part lives in a foundry
//     compound block (`ae2:cable_bus`), cables in the centre, parts on the faces.

/// The host block of every cable and part.
pub const CABLE_BUS: &str = "ae2:cable_bus";

/// A cable kind from its id: (colour 0..15 / 16 = fluix, `AECableType` suffix, dense).
pub fn cable_of(id: &str) -> Option<(u8, &'static str, bool)> {
    const COLORS: [&str; 17] = [
        "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple",
        "blue", "brown", "green", "red", "black", "fluix",
    ];
    let path = id.strip_prefix("ae2:")?;
    for (suffix, dense) in [
        ("_glass_cable", false),
        ("_covered_cable", false),
        ("_smart_cable", false),
        ("_covered_dense_cable", true),
        ("_smart_dense_cable", true),
    ] {
        if let Some(color) = path.strip_suffix(suffix)
            && let Some(c) = COLORS.iter().position(|x| *x == color)
        {
            return Some((c as u8, suffix, dense));
        }
    }
    None
}
/// A cable's boxes (1/16): (min, max, collision min/max when different). GlassCablePart 6..10,
/// CoveredCablePart / SmartCablePart 5..11, DenseCablePart 3..13 (4.9..11.1 for collisions).
pub fn cable_size(suffix: &str) -> (f64, f64, Option<(f64, f64)>) {
    match suffix {
        "_glass_cable" => (6.0, 10.0, None),
        "_covered_dense_cable" | "_smart_dense_cable" => (3.0, 13.0, Some((4.9, 11.1))),
        _ => (5.0, 11.0, None),
    }
}

/// A part of the cable bus (15.4.11): its item, `getBoxes` (SOUTH face, 1/16),
/// `getCableConnectionLength`, the `BusSupport` it sits on (1 = cables, 2 = dense cables) and
/// whether it has a grid node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartDef {
    pub id: &'static str,
    pub boxes: &'static [[f64; 6]],
    pub cable_length: f64,
    pub groups: u8,
    pub node: bool,
    /// The block that draws it (stage-1 part blocks; `part__*` item-less model blocks).
    pub model: &'static str,
}
/// AbstractReportingPart (terminals), ImportBusPart, ExportBusPart, StorageBusPart, InterfacePart,
/// EnergyAcceptorPart, CableAnchorPart (no facade); lengths: AEBasePart 3, IOBusPart 5,
/// StorageBusPart 4, InterfacePart 4, EnergyAcceptorPart 2, CableAnchorPart 0.
pub const PARTS: [PartDef; 12] = [
    PartDef { id: "ae2:terminal", boxes: &[[2.0, 2.0, 14.0, 14.0, 14.0, 16.0], [4.0, 4.0, 13.0, 12.0, 12.0, 14.0]], cable_length: 3.0, groups: 1, node: true, model: "ae2:terminal" },
    PartDef { id: "ae2:crafting_terminal", boxes: &[[2.0, 2.0, 14.0, 14.0, 14.0, 16.0], [4.0, 4.0, 13.0, 12.0, 12.0, 14.0]], cable_length: 3.0, groups: 1, node: true, model: "ae2:crafting_terminal" },
    PartDef {
        id: "ae2:import_bus",
        boxes: &[[6.0, 6.0, 11.0, 10.0, 10.0, 13.0], [5.0, 5.0, 13.0, 11.0, 11.0, 14.0], [4.0, 4.0, 14.0, 12.0, 12.0, 16.0]],
        cable_length: 5.0,
        groups: 1,
        node: true,
        model: "ae2:import_bus",
    },
    PartDef {
        id: "ae2:export_bus",
        boxes: &[[4.0, 4.0, 12.0, 12.0, 12.0, 14.0], [5.0, 5.0, 14.0, 11.0, 11.0, 15.0], [6.0, 6.0, 15.0, 10.0, 10.0, 16.0], [6.0, 6.0, 11.0, 10.0, 10.0, 12.0]],
        cable_length: 5.0,
        groups: 1,
        node: true,
        model: "ae2:export_bus",
    },
    PartDef {
        id: "ae2:storage_bus",
        boxes: &[[3.0, 3.0, 15.0, 13.0, 13.0, 16.0], [2.0, 2.0, 14.0, 14.0, 14.0, 15.0], [5.0, 5.0, 12.0, 11.0, 11.0, 14.0]],
        cable_length: 4.0,
        groups: 1,
        node: true,
        model: "ae2:storage_bus",
    },
    PartDef { id: "ae2:cable_interface", boxes: &[[2.0, 2.0, 14.0, 14.0, 14.0, 16.0], [5.0, 5.0, 12.0, 11.0, 11.0, 14.0]], cable_length: 4.0, groups: 1, node: true, model: "ae2:cable_interface" },
    PartDef { id: "ae2:cable_energy_acceptor", boxes: &[[2.0, 2.0, 14.0, 14.0, 14.0, 16.0], [4.0, 4.0, 12.0, 12.0, 12.0, 14.0]], cable_length: 2.0, groups: 1, node: true, model: "ae2:cable_energy_acceptor" },
    PartDef { id: "ae2:cable_anchor", boxes: &[[7.0, 7.0, 10.0, 9.0, 9.0, 16.0]], cable_length: 0.0, groups: 3, node: false, model: "ae2:part__cable_anchor" },
    // Stage 2 (AbstractDisplayPart boxes and length 3; PatternProviderPart 4; AbstractLevelEmitterPart 16).
    PartDef { id: "ae2:pattern_encoding_terminal", boxes: &[[2.0, 2.0, 14.0, 14.0, 14.0, 16.0], [4.0, 4.0, 13.0, 12.0, 12.0, 14.0]], cable_length: 3.0, groups: 1, node: true, model: "ae2:part__pattern_encoding_terminal" },
    PartDef { id: "ae2:pattern_access_terminal", boxes: &[[2.0, 2.0, 14.0, 14.0, 14.0, 16.0], [4.0, 4.0, 13.0, 12.0, 12.0, 14.0]], cable_length: 3.0, groups: 1, node: true, model: "ae2:part__pattern_access_terminal" },
    PartDef { id: "ae2:cable_pattern_provider", boxes: &[[2.0, 2.0, 14.0, 14.0, 14.0, 16.0], [5.0, 5.0, 12.0, 11.0, 11.0, 14.0]], cable_length: 4.0, groups: 1, node: true, model: "ae2:part__cable_pattern_provider" },
    PartDef { id: "ae2:level_emitter", boxes: &[[7.0, 7.0, 11.0, 9.0, 9.0, 16.0]], cable_length: 16.0, groups: 1, node: true, model: "ae2:part__level_emitter" },
];
pub fn part_def(id: &str) -> Option<&'static PartDef> {
    PARTS.iter().find(|p| p.id == id)
}

/// A node of the cable-bus world.
#[derive(Clone, Debug, PartialEq)]
pub struct BusNode {
    pub loc: Loc,
    pub kind: NodeKind,
    pub idle: f64,
}
/// The grid connections of cables, parts and whole blocks: a part joins only its own cable bus's
/// cable (`CableBusContainer.addPart`: `createConnection(cableNode, partNode)`); a cable or a whole
/// block joins the cable / whole block beside it when neither side holds a part on that face
/// (`updateConnections`: the cable is not exposed where a part sits) and `connects` agrees
/// (colours, a device's front). `blocked` = every (position, face) holding a part, nodeless ones
/// (the cable anchor) included. Gives the nodes as `NodeSpec` (for `assign_channels`) and the
/// networks, members and edges by index, in place order.
pub fn bus_networks(nodes: &[BusNode], blocked: &BTreeSet<(Pos, Dir)>) -> (Vec<NodeSpec>, Vec<Network>) {
    let specs: Vec<NodeSpec> = nodes.iter().map(|n| NodeSpec { pos: n.loc.pos, kind: n.kind, idle: n.idle }).collect();
    let at: BTreeMap<Loc, usize> = nodes.iter().enumerate().map(|(i, n)| (n.loc, i)).collect();
    let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (i, n) in nodes.iter().enumerate() {
        if n.loc.side.is_some() {
            if let Some(&j) = at.get(&Loc::block(n.loc.pos)) {
                edges.insert((i.min(j), i.max(j)));
            }
            continue;
        }
        for d in Dir::ALL {
            let q = offset(n.loc.pos, d);
            let Some(&j) = at.get(&Loc::block(q)) else { continue };
            if blocked.contains(&(n.loc.pos, d)) || blocked.contains(&(q, d.opposite())) {
                continue;
            }
            if connects(&specs[i], &specs[j], d) {
                edges.insert((i.min(j), i.max(j)));
            }
        }
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for &(a, b) in &edges {
        adj[a].push(b);
        adj[b].push(a);
    }
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| nodes[i].loc);
    let mut seen = vec![false; nodes.len()];
    let mut nets = Vec::new();
    for start in order {
        if seen[start] {
            continue;
        }
        let mut net = Network::default();
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(i) = queue.pop_front() {
            net.members.push(i);
            for &j in &adj[i] {
                if !seen[j] {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        net.members.sort_unstable();
        let inside: BTreeSet<usize> = net.members.iter().copied().collect();
        net.edges = edges.iter().copied().filter(|(a, _)| inside.contains(a)).collect();
        nets.push(net);
    }
    (specs, nets)
}

/// The cable a stage-1 part block becomes part of (it carried 8 channels and joined every cable
/// colour on its other faces, like a fluix cable).
pub const LEGACY_CABLE: &str = "ae2:fluix_glass_cable";
/// What a stage-1 whole block becomes (stage 1 had no multipart).
#[derive(Clone, Debug, PartialEq)]
pub struct Migration {
    /// The cable bus's cable (id), and whether it is the stage-1 part's own stub (no item: the
    /// player never had one; it goes with the part).
    pub center: String,
    pub center_legacy: bool,
    /// The part and its face.
    pub part: Option<(Dir, String)>,
}
/// A stage-1 cable becomes the same cable in a cable bus; a stage-1 part block (barrel `facing`
/// towards the player who placed it) becomes the part on its work face (the opposite of `facing`,
/// so buses keep their target) with a legacy fluix glass cable behind it. Anything else: `None`.
pub fn stage1_migration(block: &str, facing: Option<Dir>) -> Option<Migration> {
    if cable_of(block).is_some() {
        return Some(Migration { center: block.to_owned(), center_legacy: false, part: None });
    }
    let def = part_def(block).filter(|d| d.node && d.model == d.id)?;
    let work = facing?.opposite();
    Some(Migration { center: LEGACY_CABLE.to_owned(), center_legacy: true, part: Some((work, def.id.to_owned())) })
}

// ---------------------------------------------------------------------------------------------
// 12. Autocrafting (stage 2): encoded patterns, the crafting calculation, crafting CPUs, pattern
//     providers, molecular assemblers, upgrade cards, the level emitter, cell partitions.
//     From appeng.crafting.* (CraftingCalculation, CraftingTreeNode, CraftingTreeProcess,
//     CraftingSimulationState, CraftingCpuLogic, CraftingCpuHelper, ExecutingCraftingJob),
//     appeng.crafting.pattern.*, PatternProviderLogic, MolecularAssemblerBlockEntity,
//     CraftingCPUCalculator / MBCalculator, CraftingService, InitUpgrades, IOBusPart,
//     StorageLevelEmitterPart, BasicCellInventory (forge/v15.4.11).

pub mod crafting {
    use super::{Device, Dir, EnergySource, Key, Mounts, Pos, extract_config, net_extract, net_insert, offset};
    use std::collections::{BTreeMap, BTreeSet};

    /// The item id of a key (fuzzy matching, FuzzyMode.IGNORE_ALL: same item, any components).
    pub type ItemOf<'a> = &'a dyn Fn(&Key) -> String;

    // ---- 12.1 encoded patterns -----------------------------------------------------------

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
    pub enum PatternKind {
        #[default]
        Crafting,
        Processing,
        Smithing,
        Stonecutting,
    }
    impl PatternKind {
        pub const ALL: [PatternKind; 4] = [PatternKind::Crafting, PatternKind::Processing, PatternKind::Smithing, PatternKind::Stonecutting];
        /// AEItems: CRAFTING_PATTERN, PROCESSING_PATTERN, SMITHING_TABLE_PATTERN, STONECUTTING_PATTERN.
        pub fn item(self) -> &'static str {
            match self {
                PatternKind::Crafting => "ae2:crafting_pattern",
                PatternKind::Processing => "ae2:processing_pattern",
                PatternKind::Smithing => "ae2:smithing_table_pattern",
                PatternKind::Stonecutting => "ae2:stonecutting_pattern",
            }
        }
        pub fn of_item(id: &str) -> Option<Self> {
            Self::ALL.into_iter().find(|k| k.item() == id)
        }
        fn code(self) -> &'static str {
            match self {
                PatternKind::Crafting => "c",
                PatternKind::Processing => "p",
                PatternKind::Smithing => "s",
                PatternKind::Stonecutting => "t",
            }
        }
        fn from_code(c: &str) -> Option<Self> {
            Self::ALL.into_iter().find(|k| k.code() == c)
        }
        /// The encoding terminal's input slots this mode uses (encodedInputsInv: 81, the crafting
        /// grid is its first 9, smithing its first 3, stonecutting its first 1).
        pub fn input_slots(self) -> usize {
            match self {
                PatternKind::Crafting => 9,
                PatternKind::Processing => MAX_PROCESSING_INPUTS,
                PatternKind::Smithing => 3,
                PatternKind::Stonecutting => 1,
            }
        }
        pub fn index(self) -> i64 {
            self as i64
        }
        pub fn from_index(i: i64) -> Self {
            Self::ALL.get(i.clamp(0, 3) as usize).copied().unwrap_or_default()
        }
    }
    pub const BLANK_PATTERN: &str = "ae2:blank_pattern";
    /// AEProcessingPattern.MAX_INPUT_SLOTS / MAX_OUTPUT_SLOTS.
    pub const MAX_PROCESSING_INPUTS: usize = 81;
    pub const MAX_PROCESSING_OUTPUTS: usize = 27;
    pub fn is_encoded_pattern(item: &str) -> bool {
        PatternKind::of_item(item).is_some()
    }

    /// The data of an encoded pattern item (the *PatternEncoding classes): sparse inputs and
    /// outputs, the recipe id, the substitution flags. Kept as text in the item's custom data.
    #[derive(Clone, Debug, PartialEq, Eq, Default)]
    pub struct Encoded {
        pub kind: PatternKind,
        pub inputs: Vec<Option<(Key, u64)>>,
        pub outputs: Vec<Option<(Key, u64)>>,
        pub recipe: String,
        pub substitute: bool,
        pub fluid_substitute: bool,
    }
    fn stacks_text(v: &[Option<(Key, u64)>]) -> String {
        v.iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|(k, n)| format!("{i}={k}*{n}")))
            .collect::<Vec<_>>()
            .join(",")
    }
    fn stacks_parse(t: &str, len: usize) -> Option<Vec<Option<(Key, u64)>>> {
        let mut v = vec![None; len];
        for part in t.split(',').filter(|p| !p.is_empty()) {
            let (i, rest) = part.split_once('=')?;
            let (k, n) = rest.rsplit_once('*')?;
            let i: usize = i.parse().ok()?;
            let n: u64 = n.parse().ok()?;
            if i >= len || k.is_empty() || n == 0 {
                return None;
            }
            v[i] = Some((k.to_owned(), n));
        }
        Some(v)
    }
    impl Encoded {
        pub fn encode(&self) -> String {
            let recipe: String = self.recipe.chars().filter(|c| *c != ';' && !c.is_whitespace()).collect();
            format!(
                "ae2p1;{};{}{};{};{};{}",
                self.kind.code(),
                u8::from(self.substitute),
                u8::from(self.fluid_substitute),
                recipe,
                stacks_text(&self.inputs),
                stacks_text(&self.outputs)
            )
        }
        pub fn decode(text: &str) -> Option<Encoded> {
            let w: Vec<&str> = text.split(';').collect();
            if w.len() != 6 || w[0] != "ae2p1" {
                return None;
            }
            let kind = PatternKind::from_code(w[1])?;
            let flags = w[2].as_bytes();
            let outs = if kind == PatternKind::Processing { MAX_PROCESSING_OUTPUTS } else { 1 };
            let e = Encoded {
                kind,
                substitute: flags.first() == Some(&b'1'),
                fluid_substitute: flags.get(1) == Some(&b'1'),
                recipe: w[3].to_owned(),
                inputs: stacks_parse(w[4], kind.input_slots())?,
                outputs: stacks_parse(w[5], outs)?,
            };
            e.inputs.iter().any(Option::is_some).then_some(e)
        }
    }
    /// PatternEncodingTermMenu.encodeProcessingPattern: at least one input, the first output
    /// slot required.
    pub fn encode_processing(inputs: &[Option<(Key, u64)>], outputs: &[Option<(Key, u64)>]) -> Option<Encoded> {
        if !inputs.iter().any(Option::is_some) || outputs.first().cloned().flatten().is_none() {
            return None;
        }
        let mut i = inputs.to_vec();
        i.resize(MAX_PROCESSING_INPUTS, None);
        let mut o = outputs.to_vec();
        o.resize(MAX_PROCESSING_OUTPUTS, None);
        Some(Encoded { kind: PatternKind::Processing, inputs: i, outputs: o, ..Default::default() })
    }
    /// encodeCraftingPattern / encodeSmithingTablePattern / encodeStonecuttingPattern: the inputs
    /// (one of each), the recipe the adapter found for them and its result. Smithing needs all
    /// three inputs, stonecutting a chosen recipe.
    pub fn encode_recipe(kind: PatternKind, inputs: &[Option<Key>], recipe: &str, output: Option<(Key, u64)>, substitute: bool, fluid_substitute: bool) -> Option<Encoded> {
        let n = kind.input_slots();
        if kind == PatternKind::Processing || inputs.len() < n {
            return None;
        }
        let ins: Vec<Option<(Key, u64)>> = inputs[..n].iter().map(|k| k.clone().map(|k| (k, 1))).collect();
        let valid = match kind {
            PatternKind::Smithing => ins.iter().all(Option::is_some),
            _ => ins.iter().any(Option::is_some),
        };
        let output = output.filter(|(_, c)| *c > 0)?;
        if !valid || recipe.is_empty() {
            return None;
        }
        Some(Encoded {
            kind,
            inputs: ins,
            outputs: vec![Some(output)],
            recipe: recipe.to_owned(),
            substitute,
            fluid_substitute: fluid_substitute && kind == PatternKind::Crafting,
        })
    }
    /// PatternEncodingTermMenu.encode: what happens to the blank-pattern slot (`blank` items)
    /// and the encoded-pattern slot (`out`, holding at most one item). `Some(blank left)` =
    /// the new pattern goes into the encoded slot; `None` = nothing happens.
    pub fn encode_slots(blank: u32, out: Option<&str>) -> Option<u32> {
        match out {
            None if blank > 0 => Some(blank - 1),
            None => None,
            Some(id) if is_encoded_pattern(id) || id == BLANK_PATTERN => Some(blank),
            Some(_) => None,
        }
    }

    /// IPatternDetails.IInput: the possible stacks (the encoded one first, then the recipe's
    /// alternatives when substitutions are on), how many templates per craft, the items a
    /// substitution accepts and the container item left behind (bucket).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Input {
        pub possible: Vec<(Key, u64)>,
        pub multiplier: u64,
        pub subs: BTreeSet<String>,
        pub remainder: Option<Key>,
    }
    impl Input {
        /// IInput.isValid: the exact encoded key, or (substitutions) any item of the ingredient.
        pub fn is_valid(&self, key: &Key, item_of: ItemOf) -> bool {
            self.possible.first().is_some_and(|(k, _)| k == key) || !self.subs.is_empty() && self.subs.contains(&item_of(key))
        }
        pub fn primary(&self) -> &Key {
            &self.possible[0].0
        }
    }
    /// IPatternDetails of a decoded pattern.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Details {
        /// The pattern item's key (getDefinition: patterns are equal by it).
        pub definition: Key,
        pub kind: PatternKind,
        pub inputs: Vec<Input>,
        pub outputs: Vec<(Key, u64)>,
        /// The sparse inputs (processing patterns push in this order).
        pub sparse: Vec<Option<(Key, u64)>>,
        /// Assembler grid slot -> input index (crafting: sparseToCompressed).
        pub grid: Vec<Option<usize>>,
    }
    impl Details {
        /// getPrimaryOutput.
        pub fn primary(&self) -> &(Key, u64) {
            &self.outputs[0]
        }
        /// IMolecularAssemblerSupportedPattern: crafting, smithing table, stonecutting.
        pub fn assembler(&self) -> bool {
            self.kind != PatternKind::Processing
        }
        pub fn output_count(&self, what: &Key) -> u64 {
            self.outputs.iter().filter(|(k, _)| k == what).map(|(_, n)| *n).sum()
        }
        /// Every key an input may be (PatternProviderLogic.patternInputs, blocking mode).
        pub fn input_keys(&self) -> BTreeSet<Key> {
            self.inputs.iter().flat_map(|i| i.possible.iter().map(|(k, _)| k.clone())).collect()
        }
        /// pushInputsToExternalInventory: the stacks in the order they are pushed (a condensed
        /// processing pattern in its sparse order).
        pub fn push_order(&self, table: &[Vec<(Key, u64)>]) -> Vec<(Key, u64)> {
            let flat: Vec<(Key, u64)> = table.iter().flatten().cloned().collect();
            if self.kind != PatternKind::Processing || self.sparse.iter().flatten().count() == self.inputs.len() {
                return flat;
            }
            let mut all: BTreeMap<Key, u64> = BTreeMap::new();
            for (k, n) in flat {
                *all.entry(k).or_insert(0) += n;
            }
            let mut out = Vec::new();
            for (k, n) in self.sparse.iter().flatten() {
                let have = all.get(k).copied().unwrap_or(0);
                let m = have.min(*n);
                if m > 0 {
                    out.push((k.clone(), m));
                    *all.get_mut(k).unwrap() -= m;
                }
            }
            out
        }
        /// fillCraftingGrid: one item per used grid slot, from the extracted inputs.
        pub fn fill_grid(&self, table: &[Vec<(Key, u64)>]) -> Vec<Option<Key>> {
            let mut lists: Vec<Vec<(Key, u64)>> = table.to_vec();
            let mut grid = vec![None; 9];
            for (slot, cell) in grid.iter_mut().enumerate() {
                let Some(Some(i)) = self.grid.get(slot) else { continue };
                if let Some(list) = lists.get_mut(*i)
                    && let Some(e) = list.iter_mut().find(|(_, n)| *n > 0)
                {
                    e.1 -= 1;
                    *cell = Some(e.0.clone());
                }
            }
            grid
        }
        /// isItemValid(slot, key) for the assembler's grid.
        pub fn grid_valid(&self, slot: usize, key: Option<&Key>, item_of: ItemOf) -> bool {
            match (self.grid.get(slot).copied().flatten(), key) {
                (None, None) => true,
                (Some(i), Some(k)) => self.inputs[i].is_valid(k, item_of),
                _ => false,
            }
        }
    }
    /// AEPatternHelper.condenseStacks: every key once, amounts summed, first-seen order.
    pub fn condense(sparse: &[Option<(Key, u64)>]) -> Vec<(Key, u64)> {
        let mut out: Vec<(Key, u64)> = Vec::new();
        for (k, n) in sparse.iter().flatten() {
            match out.iter_mut().find(|(x, _)| x == k) {
                Some(e) => e.1 = e.1.saturating_add(*n),
                None => out.push((k.clone(), *n)),
            }
        }
        out
    }
    /// AEProcessingPattern: condensed inputs (template 1, multiplier = amount) and outputs.
    pub fn processing_details(definition: &Key, e: &Encoded) -> Option<Details> {
        if e.kind != PatternKind::Processing {
            return None;
        }
        let ins = condense(&e.inputs);
        let outs = condense(&e.outputs);
        if ins.is_empty() || e.outputs.first().cloned().flatten().is_none() {
            return None;
        }
        Some(Details {
            definition: definition.clone(),
            kind: e.kind,
            inputs: ins
                .into_iter()
                .map(|(k, n)| Input { possible: vec![(k, 1)], multiplier: n, subs: BTreeSet::new(), remainder: None })
                .collect(),
            outputs: outs,
            sparse: e.inputs.clone(),
            grid: Vec::new(),
        })
    }
    /// AECraftingPattern / AESmithingTablePattern / AEStonecuttingPattern from the encoded
    /// inputs and what the adapter found in the recipe: `output` = the recipe's result for the
    /// encoded inputs, `alts[slot]` = the ingredient's other items (keys and item ids, used only
    /// with substitutions), `remainders[slot]` = the crafting remainder of the encoded item.
    pub fn recipe_details(
        definition: &Key,
        e: &Encoded,
        output: (Key, u64),
        alts: &[Vec<(Key, String)>],
        remainders: &[Option<Key>],
    ) -> Option<Details> {
        if e.kind == PatternKind::Processing || output.1 == 0 {
            return None;
        }
        let input_of = |slot: usize, key: &Key, multiplier: u64| {
            let mut possible = vec![(key.clone(), 1)];
            let mut subs = BTreeSet::new();
            if e.substitute {
                for (k, item) in alts.get(slot).map(Vec::as_slice).unwrap_or(&[]) {
                    subs.insert(item.clone());
                    if k != key {
                        possible.push((k.clone(), 1));
                    }
                }
            }
            Input { possible, multiplier, subs, remainder: remainders.get(slot).cloned().flatten() }
        };
        let mut inputs = Vec::new();
        let mut grid = vec![None; 9];
        match e.kind {
            PatternKind::Crafting => {
                // Condensed: one input per distinct key, the first slot that has it.
                let condensed = condense(&e.inputs);
                for (j, (k, n)) in condensed.iter().enumerate() {
                    let first = e.inputs.iter().position(|s| s.as_ref().is_some_and(|(x, _)| x == k))?;
                    inputs.push(input_of(first, k, *n));
                    for (slot, s) in e.inputs.iter().enumerate().take(9) {
                        if s.as_ref().is_some_and(|(x, _)| x == k) {
                            grid[slot] = Some(j);
                        }
                    }
                }
            }
            _ => {
                // One input per slot (template, base, addition / the stonecutter's input).
                for (slot, s) in e.inputs.iter().enumerate() {
                    if let Some((k, _)) = s {
                        grid[slot] = Some(inputs.len());
                        inputs.push(input_of(slot, k, 1));
                    }
                }
            }
        }
        if inputs.is_empty() {
            return None;
        }
        Some(Details { definition: definition.clone(), kind: e.kind, inputs, outputs: vec![output], sparse: e.inputs.clone(), grid })
    }

    // ---- 12.2 the crafting calculation ----------------------------------------------------

    /// The network's patterns (ICraftingService: getCraftingFor by primary output, priority
    /// order as given) and what level emitters can emit (canEmitFor).
    #[derive(Clone, Debug, Default)]
    pub struct Book {
        pub patterns: Vec<Details>,
        pub emitable: BTreeSet<Key>,
        by_output: BTreeMap<Key, Vec<usize>>,
    }
    impl Book {
        /// Patterns are equal by their definition: the first of each is kept (in priority order).
        pub fn new(patterns: Vec<Details>, emitable: BTreeSet<Key>) -> Book {
            let mut kept: Vec<Details> = Vec::new();
            for p in patterns {
                if p.outputs.is_empty() || kept.iter().any(|k| k.definition == p.definition) {
                    continue;
                }
                kept.push(p);
            }
            let mut by_output: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
            for (i, p) in kept.iter().enumerate() {
                by_output.entry(p.primary().0.clone()).or_default().push(i);
            }
            Book { patterns: kept, emitable, by_output }
        }
        pub fn for_key(&self, what: &Key) -> &[usize] {
            self.by_output.get(what).map_or(&[], Vec::as_slice)
        }
        /// isCraftable: a pattern makes it or an emitter can emit it.
        pub fn craftable(&self, what: &Key) -> bool {
            self.by_output.contains_key(what) || self.emitable.contains(what)
        }
        pub fn craftable_keys(&self) -> BTreeSet<Key> {
            self.by_output.keys().chain(self.emitable.iter()).cloned().collect()
        }
        pub fn index_of(&self, definition: &Key) -> Option<usize> {
            self.patterns.iter().position(|p| &p.definition == definition)
        }
        pub fn get(&self, definition: &Key) -> Option<&Details> {
            self.patterns.iter().find(|p| &p.definition == definition)
        }
    }

    /// CalculationStrategy.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Strategy {
        /// The terminal: a plan with the missing items when it cannot be done.
        ReportMissing,
        /// Machines and replans: the largest amount that can be done (binary search).
        CraftLess,
    }
    /// ICraftingPlan (CraftingPlan).
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Plan {
        pub output: (Key, u64),
        pub bytes: u64,
        pub simulation: bool,
        pub multiple_paths: bool,
        /// usedItems (requiredExtract): taken from the network when the job starts.
        pub used: BTreeMap<Key, u64>,
        pub emitted: BTreeMap<Key, u64>,
        pub missing: BTreeMap<Key, u64>,
        /// patternTimes: pattern definition -> crafts (book order).
        pub crafts: Vec<(Key, u64)>,
        /// The port's guard: the calculation stopped after PLAN_BUDGET steps (AE2 computes on a
        /// thread for as long as it takes).
        pub too_complex: bool,
    }
    /// One line of the confirm screen (CraftingPlanSummaryEntry).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SummaryEntry {
        pub key: Key,
        pub missing: u64,
        pub stored: u64,
        pub craft: u64,
    }
    impl Plan {
        /// CraftingPlanSummary.fromJob (`available` = the network's amounts now), sorted by
        /// missing, then craft, then stored amount, descending.
        pub fn summary(&self, book: &Book, available: &BTreeMap<Key, u64>) -> Vec<SummaryEntry> {
            let mut stats: BTreeMap<Key, (u64, u64)> = BTreeMap::new();
            for (k, n) in self.used.iter().chain(self.missing.iter()) {
                stats.entry(k.clone()).or_default().0 += n;
            }
            for (k, n) in &self.emitted {
                let e = stats.entry(k.clone()).or_default();
                e.0 += n;
                e.1 += n;
            }
            for (def, times) in &self.crafts {
                if let Some(p) = book.get(def) {
                    for (k, n) in &p.outputs {
                        stats.entry(k.clone()).or_default().1 += n.saturating_mul(*times);
                    }
                }
            }
            let mut out: Vec<SummaryEntry> = stats
                .into_iter()
                .map(|(key, (stored, craft))| {
                    let (stored, missing) = if self.simulation && !book.emitable.contains(&key) {
                        let s = available.get(&key).copied().unwrap_or(0).min(stored);
                        (s, stored - s)
                    } else {
                        (stored, 0)
                    };
                    SummaryEntry { key, missing, stored, craft }
                })
                .collect();
            out.sort_by(|a, b| (b.missing, b.craft, b.stored).cmp(&(a.missing, a.craft, a.stored)));
            out
        }
    }
    /// Steps (node requests) one calculation may take.
    pub const PLAN_BUDGET: u64 = 2_000_000;

    struct Env<'a> {
        book: &'a Book,
        item_of: ItemOf<'a>,
        net: &'a BTreeMap<Key, u64>,
        net_items: BTreeMap<String, Vec<Key>>,
        craft_items: BTreeMap<String, Vec<Key>>,
    }
    #[derive(Default)]
    struct Tally {
        simulate: bool,
        missing: BTreeMap<Key, u64>,
        steps: u64,
        over: bool,
    }
    /// CraftingSimulationState: the parent's amounts cached on first touch (unmodified and
    /// modifiable copies), what was emitted, bytes, crafts and the most each key went below
    /// its cached amount (requiredExtract).
    struct Sim<'p> {
        parent: Option<&'p Sim<'p>>,
        unmod: BTreeMap<Key, u64>,
        modi: BTreeMap<Key, u64>,
        emitted: BTreeMap<Key, u64>,
        bytes: f64,
        crafts: BTreeMap<usize, u64>,
        required: BTreeMap<Key, u64>,
    }
    struct Diff {
        unmod: BTreeMap<Key, u64>,
        modi: BTreeMap<Key, u64>,
        emitted: BTreeMap<Key, u64>,
        bytes: f64,
        crafts: BTreeMap<usize, u64>,
        required: BTreeMap<Key, u64>,
    }
    impl<'p> Sim<'p> {
        fn new(parent: Option<&'p Sim<'p>>) -> Self {
            Sim {
                parent,
                unmod: BTreeMap::new(),
                modi: BTreeMap::new(),
                emitted: BTreeMap::new(),
                bytes: 0.0,
                crafts: BTreeMap::new(),
                required: BTreeMap::new(),
            }
        }
        fn available(&self, env: &Env, k: &Key) -> u64 {
            match self.modi.get(k) {
                Some(v) => *v,
                None => match self.parent {
                    Some(p) => p.available(env, k),
                    None => env.net.get(k).copied().unwrap_or(0),
                },
            }
        }
        fn fuzzy(&self, env: &Env, item: &str, out: &mut BTreeSet<Key>) {
            for k in self.modi.keys() {
                if (env.item_of)(k) == item {
                    out.insert(k.clone());
                }
            }
            match self.parent {
                Some(p) => p.fuzzy(env, item, out),
                None => out.extend(env.net_items.get(item).into_iter().flatten().cloned()),
            }
        }
        fn touch(&mut self, env: &Env, k: &Key) {
            if !self.modi.contains_key(k) {
                let v = match self.parent {
                    Some(p) => p.available(env, k),
                    None => env.net.get(k).copied().unwrap_or(0),
                };
                self.unmod.insert(k.clone(), v);
                self.modi.insert(k.clone(), v);
            }
        }
        fn update_required(&mut self, k: &Key) {
            let delta = i128::from(self.unmod.get(k).copied().unwrap_or(0)) - i128::from(self.modi.get(k).copied().unwrap_or(0));
            if delta > 0 {
                let d = delta.min(i128::from(u64::MAX)) as u64;
                let e = self.required.entry(k.clone()).or_insert(0);
                *e = (*e).max(d);
            }
        }
        fn insert(&mut self, env: &Env, k: &Key, n: u64) {
            self.touch(env, k);
            let e = self.modi.get_mut(k).unwrap();
            *e = e.saturating_add(n);
        }
        fn extract(&mut self, env: &Env, k: &Key, n: u64, modulate: bool) -> u64 {
            self.touch(env, k);
            let cached = self.modi[k];
            if cached == 0 {
                return 0;
            }
            let e = cached.min(n);
            if modulate {
                *self.modi.get_mut(k).unwrap() -= e;
            }
            self.update_required(k);
            e
        }
        fn emit(&mut self, k: &Key, n: u64) {
            let e = self.emitted.entry(k.clone()).or_insert(0);
            *e = e.saturating_add(n);
        }
        fn ignore(&mut self, env: &Env, k: &Key) {
            self.touch(env, k);
            self.unmod.insert(k.clone(), 0);
            self.modi.insert(k.clone(), 0);
        }
        /// addStackBytes: amount * multiplier / amountPerByte(8) * 8 (crafting storage is 8
        /// times bigger than cells, ICraftingSimulationState).
        fn add_stack_bytes(&mut self, amount: u64, multiplier: u64) {
            self.bytes += amount as f64 * multiplier as f64;
        }
        fn into_diff(self) -> Diff {
            Diff { unmod: self.unmod, modi: self.modi, emitted: self.emitted, bytes: self.bytes, crafts: self.crafts, required: self.required }
        }
        /// CraftingSimulationState.applyDiff.
        fn apply(&mut self, env: &Env, d: Diff) -> Result<(), ()> {
            for (k, r) in &d.required {
                let delta = i128::from(self.unmod.get(k).copied().unwrap_or(0)) - i128::from(self.modi.get(k).copied().unwrap_or(0)) + i128::from(*r);
                if delta > 0 {
                    let v = delta.min(i128::from(u64::MAX)) as u64;
                    let e = self.required.entry(k.clone()).or_insert(0);
                    *e = (*e).max(v);
                }
            }
            for (k, v) in &d.modi {
                let before = d.unmod.get(k).copied().unwrap_or(0);
                if *v > before {
                    self.insert(env, k, v - before);
                } else if *v < before {
                    let want = before - v;
                    if self.extract(env, k, want, true) != want {
                        return Err(());
                    }
                }
            }
            for (k, n) in d.emitted {
                self.emit(&k, n);
            }
            self.bytes += d.bytes;
            for (p, n) in d.crafts {
                *self.crafts.entry(p).or_insert(0) += n;
            }
            Ok(())
        }
    }

    /// CraftingTreeNode.
    struct Node {
        what: Key,
        amount: u64,
        /// (pattern, input) this node feeds; None for the requested stack.
        input: Option<(usize, usize)>,
        can_emit: bool,
        procs: Option<Vec<Proc>>,
        /// The keys of the nodes above (notRecursive).
        ancestors: Vec<Key>,
    }
    /// CraftingTreeProcess.
    struct Proc {
        pattern: usize,
        children: Vec<(Node, u64)>,
        possible: bool,
        containers: bool,
        limit: bool,
    }
    impl Node {
        fn new(env: &Env, what: Key, amount: u64, input: Option<(usize, usize)>, ancestors: Vec<Key>) -> Node {
            let can_emit = env.book.emitable.contains(&what);
            let what = find_crafted(env, what, input);
            Node { what, amount, input, can_emit, procs: None, ancestors }
        }
        fn count(&self) -> u64 {
            1 + self.procs.iter().flatten().map(|p| p.children.iter().map(|(c, _)| c.count()).sum::<u64>()).sum::<u64>()
        }
        fn multiple(&self) -> bool {
            match &self.procs {
                None => false,
                Some(v) if v.len() > 1 => true,
                Some(v) => v.iter().any(|p| p.children.iter().any(|(c, _)| c.multiple())),
            }
        }
    }
    /// findCraftedStack: an emitable key as is; without a pattern for the exact input, a
    /// craftable substitute of the same amount (getFuzzyCraftable + isValid).
    fn find_crafted(env: &Env, what: Key, input: Option<(usize, usize)>) -> Key {
        if env.book.emitable.contains(&what) || !env.book.for_key(&what).is_empty() {
            return what;
        }
        let Some((p, i)) = input else { return what };
        let inp = &env.book.patterns[p].inputs[i];
        let acceptable = inp.possible[0].1;
        for (pk, amt) in &inp.possible {
            if *amt != acceptable {
                continue;
            }
            for ck in env.craft_items.get(&(env.item_of)(pk)).into_iter().flatten() {
                if inp.is_valid(ck, env.item_of) {
                    return ck.clone();
                }
            }
        }
        what
    }
    /// notRecursive: the pattern makes, or takes as its primary input, nothing of the nodes above.
    fn not_recursive(env: &Env, ancestors: &[Key], p: usize) -> bool {
        let d = &env.book.patterns[p];
        !ancestors.iter().any(|a| d.outputs.iter().any(|(k, _)| k == a) || d.inputs.iter().any(|i| i.primary() == a))
    }
    fn build_procs(env: &Env, node: &mut Node) {
        if node.procs.is_some() {
            return;
        }
        let mut v = Vec::new();
        for &p in env.book.for_key(&node.what) {
            if !not_recursive(env, &node.ancestors, p) {
                continue;
            }
            let d = &env.book.patterns[p];
            let mut anc = node.ancestors.clone();
            anc.push(node.what.clone());
            let mut limit = false;
            let mut containers = false;
            let mut children = Vec::new();
            for (x, input) in d.inputs.iter().enumerate() {
                if d.outputs.iter().any(|(k, _)| k == input.primary()) {
                    limit = true;
                }
                if input.remainder.is_some() {
                    limit = true;
                    containers = true;
                }
                let (k, a) = input.possible[0].clone();
                children.push((Node::new(env, k, a, Some((p, x)), anc.clone()), input.multiplier));
            }
            v.push(Proc { pattern: p, children, possible: true, containers, limit });
        }
        node.procs = Some(v);
    }
    /// CraftingCpuHelper.getValidItemTemplates for a node.
    fn templates(env: &Env, inv: &Sim, node: &Node) -> Vec<(Key, u64)> {
        let Some((p, i)) = node.input else { return vec![(node.what.clone(), 1)] };
        valid_templates(env.book, p, i, env.item_of, &mut |item, out| inv.fuzzy(env, item, out))
    }
    fn valid_templates(book: &Book, p: usize, i: usize, item_of: ItemOf, fuzzy: &mut dyn FnMut(&str, &mut BTreeSet<Key>)) -> Vec<(Key, u64)> {
        let input = &book.patterns[p].inputs[i];
        input_templates(input, item_of, fuzzy)
    }
    fn input_templates(input: &Input, item_of: ItemOf, fuzzy: &mut dyn FnMut(&str, &mut BTreeSet<Key>)) -> Vec<(Key, u64)> {
        let mut out: Vec<(Key, u64)> = Vec::new();
        for (pk, amt) in &input.possible {
            let mut keys = BTreeSet::new();
            fuzzy(&item_of(pk), &mut keys);
            // The encoded key itself first.
            let mut ordered: Vec<Key> = Vec::new();
            if keys.remove(pk) {
                ordered.push(pk.clone());
            }
            ordered.extend(keys);
            for k in ordered {
                if input.is_valid(&k, item_of) && !out.iter().any(|(x, a)| *x == k && a == amt) {
                    out.push((k, *amt));
                }
            }
        }
        out
    }
    /// CraftingCpuHelper.extractTemplates on a simulation.
    fn extract_templates(env: &Env, inv: &mut Sim, key: &Key, amount: u64, multiplier: u64) -> u64 {
        let max = amount.saturating_mul(multiplier);
        let got = inv.extract(env, key, max, false);
        if got == 0 {
            return 0;
        }
        let mult = got / amount.max(1);
        let total = amount * mult;
        if total == 0 {
            return 0;
        }
        inv.extract(env, key, total, true);
        mult
    }
    fn add_container(env: &Env, node: &Node, multiplier: u64, out: Option<&mut BTreeMap<Key, u64>>) {
        if let (Some(out), Some((p, i))) = (out, node.input)
            && let Some(r) = &env.book.patterns[p].inputs[i].remainder
        {
            *out.entry(r.clone()).or_insert(0) += multiplier;
        }
    }
    /// CraftingTreeNode.request.
    fn node_request(env: &Env, t: &mut Tally, inv: &mut Sim, node: &mut Node, mut requested: u64, mut containers: Option<&mut BTreeMap<Key, u64>>) -> Result<(), ()> {
        t.steps += 1;
        if t.steps > PLAN_BUDGET {
            t.over = true;
            return Err(());
        }
        inv.add_stack_bytes(node.amount, requested);
        for (k, amt) in templates(env, inv, node) {
            let got = extract_templates(env, inv, &k, amt, requested);
            if got > 0 {
                requested -= got.min(requested);
                add_container(env, node, got, containers.as_deref_mut());
                if requested == 0 {
                    return Ok(());
                }
            }
        }
        add_container(env, node, requested, containers.as_deref_mut());
        if node.can_emit {
            inv.emit(&node.what, node.amount.saturating_mul(requested));
            return Ok(());
        }
        build_procs(env, node);
        let what = node.what.clone();
        let mut total = requested.saturating_mul(node.amount);
        let procs = node.procs.as_mut().unwrap();
        if procs.len() == 1 {
            let pro = &mut procs[0];
            let per = env.book.patterns[pro.pattern].output_count(&what).max(1);
            while pro.possible && total > 0 {
                let times = if pro.limit { 1 } else { total.div_ceil(per) };
                proc_request(env, t, inv, pro, times)?;
                let available = inv.extract(env, &what, total, true);
                if available == 0 {
                    // "can't find created items": an AE2 bug path; the branch fails.
                    return Err(());
                }
                total -= available;
                if total == 0 {
                    return Ok(());
                }
            }
        } else if procs.len() > 1 {
            for pro in procs.iter_mut() {
                while pro.possible && total > 0 {
                    let mut child = Sim::new(Some(&*inv));
                    match proc_request(env, t, &mut child, pro, 1) {
                        Ok(()) => {
                            let available = child.extract(env, &what, total, true);
                            if available != 0 {
                                let d = child.into_diff();
                                inv.apply(env, d)?;
                                total -= available;
                                if total == 0 {
                                    return Ok(());
                                }
                            } else {
                                pro.possible = false;
                            }
                        }
                        Err(()) => {
                            if t.over {
                                return Err(());
                            }
                            pro.possible = true;
                            break;
                        }
                    }
                }
            }
        }
        if t.simulate {
            *t.missing.entry(what).or_insert(0) += total;
            Ok(())
        } else {
            Err(())
        }
    }
    /// CraftingTreeProcess.request.
    fn proc_request(env: &Env, t: &mut Tally, inv: &mut Sim, pro: &mut Proc, times: u64) -> Result<(), ()> {
        let mut cont: Option<BTreeMap<Key, u64>> = pro.containers.then(BTreeMap::new);
        for (child, mult) in pro.children.iter_mut() {
            node_request(env, t, inv, child, mult.saturating_mul(times), cont.as_mut())?;
        }
        if let Some(c) = cont {
            for (k, n) in c {
                inv.insert(env, &k, n);
                inv.add_stack_bytes(n, 1);
            }
        }
        for (k, n) in &env.book.patterns[pro.pattern].outputs {
            inv.insert(env, k, n.saturating_mul(times));
        }
        *inv.crafts.entry(pro.pattern).or_insert(0) += times;
        inv.bytes += times as f64;
        Ok(())
    }
    fn attempt(env: &Env, t: &mut Tally, tree: &mut Node, what: &Key, simulate: bool, amount: u64) -> Option<Plan> {
        t.simulate = simulate;
        let mut inv = Sim::new(None);
        inv.ignore(env, what);
        if node_request(env, t, &mut inv, tree, amount, None).is_err() {
            return None;
        }
        inv.bytes += (tree.count() * 8) as f64;
        let crafts = inv.crafts.iter().map(|(p, n)| (env.book.patterns[*p].definition.clone(), *n)).collect();
        Some(Plan {
            output: (what.clone(), amount),
            bytes: inv.bytes.ceil() as u64,
            simulation: simulate,
            multiple_paths: tree.multiple(),
            used: inv.required,
            emitted: inv.emitted,
            missing: t.missing.clone(),
            crafts,
            too_complex: false,
        })
    }
    /// CraftingCalculation.computePlan: the full amount; CRAFT_LESS tries less by binary
    /// search; otherwise (or when nothing works) a simulation with the missing items.
    pub fn plan(book: &Book, network: &BTreeMap<Key, u64>, what: &Key, amount: u64, strategy: Strategy, item_of: ItemOf) -> Plan {
        let mut net_items: BTreeMap<String, Vec<Key>> = BTreeMap::new();
        for k in network.keys() {
            net_items.entry(item_of(k)).or_default().push(k.clone());
        }
        let mut craft_items: BTreeMap<String, Vec<Key>> = BTreeMap::new();
        for p in &book.patterns {
            let k = &p.primary().0;
            let v = craft_items.entry(item_of(k)).or_default();
            if !v.contains(k) {
                v.push(k.clone());
            }
        }
        let env = Env { book, item_of, net: network, net_items, craft_items };
        let mut t = Tally::default();
        let amount = amount.max(1);
        let mut tree = Node::new(&env, what.clone(), 1, None, Vec::new());
        let too_complex = |what: &Key| Plan {
            output: (what.clone(), amount),
            simulation: true,
            missing: BTreeMap::from([(what.clone(), amount)]),
            too_complex: true,
            ..Default::default()
        };
        if let Some(p) = attempt(&env, &mut t, &mut tree, what, false, amount) {
            return p;
        }
        if t.over {
            return too_complex(what);
        }
        if strategy == Strategy::CraftLess {
            let mut ok_amount = 0u64;
            let mut ok_plan = None;
            let mut inc = 1u64 << (63 - amount.leading_zeros());
            while inc > 0 {
                let test = ok_amount + inc;
                if test < amount
                    && let Some(p) = attempt(&env, &mut t, &mut tree, what, false, test)
                {
                    ok_amount = test;
                    ok_plan = Some(p);
                }
                if t.over {
                    return too_complex(what);
                }
                inc /= 2;
            }
            if let Some(p) = ok_plan {
                return p;
            }
        }
        match attempt(&env, &mut t, &mut tree, what, true, amount) {
            Some(p) => p,
            None => too_complex(what),
        }
    }

    // ---- 12.3 crafting CPUs: the multiblock (CraftingCPUCalculator, MBCalculator) ----------

    /// CraftingUnitType.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Unit {
        Unit,
        Accelerator,
        /// Kilobytes: 1, 4, 16, 64, 256.
        Storage(u32),
        Monitor,
    }
    impl Unit {
        pub fn of(id: &str) -> Option<Unit> {
            Some(match id {
                "ae2:crafting_unit" => Unit::Unit,
                "ae2:crafting_accelerator" => Unit::Accelerator,
                "ae2:crafting_monitor" => Unit::Monitor,
                "ae2:1k_crafting_storage" => Unit::Storage(1),
                "ae2:4k_crafting_storage" => Unit::Storage(4),
                "ae2:16k_crafting_storage" => Unit::Storage(16),
                "ae2:64k_crafting_storage" => Unit::Storage(64),
                "ae2:256k_crafting_storage" => Unit::Storage(256),
                _ => return None,
            })
        }
        /// getStorageBytes: 1024 * kB.
        pub fn bytes(self) -> u64 {
            match self {
                Unit::Storage(kb) => 1024 * u64::from(kb),
                _ => 0,
            }
        }
        /// getAcceleratorThreads.
        pub fn threads(self) -> u32 {
            u32::from(self == Unit::Accelerator)
        }
    }
    /// A formed crafting CPU (CraftingCPUCluster).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Cluster {
        pub min: Pos,
        pub max: Pos,
        pub storage: u64,
        pub coprocessors: u32,
        pub members: Vec<Pos>,
    }
    impl Cluster {
        /// The block holding the CPU's state (the port keeps it in the lowest corner).
        pub fn core(&self) -> Pos {
            self.min
        }
    }
    /// CraftingCPUCalculator.checkMultiblockScale: at most 17 blocks on each axis.
    pub const CPU_MAX_SPAN: i32 = 16;
    /// MBCalculator.calculateMultiblock from every unit: the box walked from it (-x, -y, -z,
    /// then +x, +y, +z through units), no larger than 17 on an axis, no unit touching its faces
    /// from outside (verifyUnownedRegion), every block inside a unit and one storage at least
    /// (verifyInternalStructure). Units outside any valid box are not formed.
    pub fn clusters(units: &BTreeMap<Pos, Unit>) -> Vec<Cluster> {
        let is = |p: Pos| units.contains_key(&p);
        let mut done: BTreeSet<Pos> = BTreeSet::new();
        let mut out = Vec::new();
        for &p in units.keys() {
            if done.contains(&p) {
                continue;
            }
            let mut min = p;
            let mut max = p;
            for axis in 0..3 {
                loop {
                    let mut q = min;
                    q[axis] -= 1;
                    if !is(q) {
                        break;
                    }
                    min = q;
                }
            }
            for axis in 0..3 {
                loop {
                    let mut q = max;
                    q[axis] += 1;
                    if !is(q) {
                        break;
                    }
                    max = q;
                }
            }
            if (0..3).any(|a| max[a] - min[a] > CPU_MAX_SPAN) {
                continue;
            }
            // verifyUnownedRegion: the six slabs just outside the box.
            let mut owned = false;
            'faces: for axis in 0..3 {
                for side in [min[axis] - 1, max[axis] + 1] {
                    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
                    for u in min[a]..=max[a] {
                        for v in min[b]..=max[b] {
                            let mut q = [0; 3];
                            q[axis] = side;
                            q[a] = u;
                            q[b] = v;
                            if is(q) {
                                owned = true;
                                break 'faces;
                            }
                        }
                    }
                }
            }
            if owned {
                continue;
            }
            let mut members = Vec::new();
            let mut storage = 0u64;
            let mut coprocessors = 0u32;
            let mut full = true;
            for x in min[0]..=max[0] {
                for y in min[1]..=max[1] {
                    for z in min[2]..=max[2] {
                        match units.get(&[x, y, z]) {
                            Some(u) => {
                                storage += u.bytes();
                                coprocessors += u.threads();
                                members.push([x, y, z]);
                            }
                            None => full = false,
                        }
                    }
                }
            }
            if !full || storage == 0 {
                continue;
            }
            done.extend(members.iter().copied());
            out.push(Cluster { min, max, storage, coprocessors, members });
        }
        out
    }

    // ---- 12.4 the crafting CPU's job (CraftingCpuLogic, ExecutingCraftingJob) ------------

    /// ExecutingCraftingJob.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Job {
        /// The craft id (CraftingLink.craftId).
        pub id: i64,
        pub output: (Key, u64),
        pub remaining: u64,
        /// tasks: pattern definition -> crafts left.
        pub tasks: Vec<(Key, u64)>,
        pub waiting: BTreeMap<Key, u64>,
        /// The machine that asked (interface, export bus), None = a player (standalone job).
        pub requester: Option<(Pos, Option<Dir>)>,
        pub player: bool,
        /// ElapsedTimeTracker: the items of the whole job and those still to come.
        pub total: u64,
    }
    /// What a CPU reports when a job ends (CraftingJobStatusPacket).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Finished {
        pub output: (Key, u64),
        pub success: bool,
        pub player: bool,
    }
    /// One crafting CPU's logic and state.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Cpu {
        /// ListCraftingInventory: real items held by the CPU.
        pub inventory: BTreeMap<Key, u64>,
        pub job: Option<Job>,
        /// Crafting operations of the last 3 ticks.
        pub used_ops: [u32; 3],
        pub cant_store: bool,
        pub finished: Vec<Finished>,
    }
    /// ICraftingSubmitResult errors (CraftingSubmitResult / UnsuitableCpus).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum SubmitError {
        IncompletePlan,
        CpuBusy,
        CpuOffline,
        CpuTooSmall,
        MissingIngredient(Key, u64),
        NoCpuFound,
        NoSuitableCpu { offline: u32, busy: u32, too_small: u32 },
    }
    /// A CPU as CraftingService.findSuitableCraftingCPU sees it.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CpuInfo {
        pub active: bool,
        pub busy: bool,
        pub storage: u64,
        pub coprocessors: u32,
    }
    /// findSuitableCraftingCPU: active, idle, big enough; FAST_FIRST (players: most
    /// co-processors first) or FAST_LAST (machines), then the smallest storage.
    pub fn select_cpu(cpus: &[CpuInfo], bytes: u64, prioritize_power: bool) -> Result<usize, SubmitError> {
        let (mut offline, mut busy, mut small) = (0, 0, 0);
        let mut valid: Vec<usize> = Vec::new();
        for (i, c) in cpus.iter().enumerate() {
            if !c.active {
                offline += 1;
            } else if c.busy {
                busy += 1;
            } else if c.storage < bytes {
                small += 1;
            } else {
                valid.push(i);
            }
        }
        if valid.is_empty() {
            return Err(if offline + busy + small > 0 { SubmitError::NoSuitableCpu { offline, busy, too_small: small } } else { SubmitError::NoCpuFound });
        }
        valid.sort_by(|&a, &b| {
            let (x, y) = (&cpus[a], &cpus[b]);
            let by_ops = if prioritize_power { y.coprocessors.cmp(&x.coprocessors) } else { x.coprocessors.cmp(&y.coprocessors) };
            by_ops.then(x.storage.cmp(&y.storage))
        });
        Ok(valid[0])
    }
    /// What the CPU took of a network insertion (the CraftingServiceStorage interceptor).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Accept {
        /// Went into the CPU's inventory (an intermediate result it waited for).
        pub stored: u64,
        /// Counted as final output: it goes to the requester (the adapter inserts it there; a
        /// standalone job lets it continue into the network's storage).
        pub output: u64,
    }
    /// The pattern providers a CPU pushes to (CraftingService.getProviders).
    pub trait Machines {
        /// How many providers hold the pattern (priority order).
        fn providers(&mut self, definition: &Key) -> usize;
        /// ICraftingProvider.isBusy.
        fn busy(&mut self, provider: usize) -> bool;
        /// pushPattern: true when the provider took the inputs.
        fn push(&mut self, provider: usize, details: &Details, table: &[Vec<(Key, u64)>]) -> bool;
    }
    /// calculatePatternPower: one AE per input item (amountPerOperation 1).
    pub fn pattern_power(table: &[Vec<(Key, u64)>]) -> f64 {
        table.iter().flatten().map(|(_, n)| *n as f64).sum()
    }
    fn list_extract(inv: &mut BTreeMap<Key, u64>, k: &Key, n: u64, modulate: bool) -> u64 {
        let have = inv.get(k).copied().unwrap_or(0);
        let e = have.min(n);
        if modulate && e > 0 {
            if have == e {
                inv.remove(k);
            } else {
                *inv.get_mut(k).unwrap() -= e;
            }
        }
        e
    }
    fn list_add(inv: &mut BTreeMap<Key, u64>, k: &Key, n: u64) {
        if n > 0 {
            let e = inv.entry(k.clone()).or_insert(0);
            *e = e.saturating_add(n);
        }
    }
    /// CraftingCpuHelper.extractPatternInputs from a CPU inventory: the table of inputs, the
    /// expected outputs and container items; None (nothing taken) when an input is short.
    pub fn extract_pattern_inputs(d: &Details, inv: &mut BTreeMap<Key, u64>, item_of: ItemOf) -> Option<(Vec<Vec<(Key, u64)>>, Vec<(Key, u64)>)> {
        let mut table: Vec<Vec<(Key, u64)>> = Vec::new();
        let mut expected: BTreeMap<Key, u64> = BTreeMap::new();
        let mut found = true;
        for input in &d.inputs {
            let mut list: Vec<(Key, u64)> = Vec::new();
            let mut remaining = input.multiplier;
            let snapshot = inv.clone();
            let temps = input_templates(input, item_of, &mut |item, out| {
                out.extend(snapshot.keys().filter(|k| item_of(k) == item).cloned());
            });
            for (k, amt) in temps {
                let max = amt.saturating_mul(remaining);
                let got = list_extract(inv, &k, max, false);
                let mult = got / amt.max(1);
                if mult > 0 {
                    list_extract(inv, &k, amt * mult, true);
                    match list.iter_mut().find(|(x, _)| *x == k) {
                        Some(e) => e.1 += amt * mult,
                        None => list.push((k.clone(), amt * mult)),
                    }
                    if let Some(r) = &input.remainder {
                        *expected.entry(r.clone()).or_insert(0) += mult;
                    }
                    remaining -= mult;
                }
                if remaining == 0 {
                    break;
                }
            }
            table.push(list);
            if remaining > 0 {
                found = false;
                break;
            }
        }
        if !found {
            for (k, n) in table.iter().flatten() {
                list_add(inv, k, *n);
            }
            return None;
        }
        for (k, n) in &d.outputs {
            *expected.entry(k.clone()).or_insert(0) += n;
        }
        Some((table, expected.into_iter().collect()))
    }
    impl Cpu {
        /// CraftingCpuLogic.trySubmitJob (`net` = the network's storage, unpowered as
        /// grid.getStorageService().getInventory()). Ingredients that cannot go back stay in
        /// the CPU (AE2 voids them) and are stored later.
        #[allow(clippy::too_many_arguments)]
        pub fn submit(&mut self, plan: &Plan, storage: u64, active: bool, net: &mut dyn Mounts, id: i64, requester: Option<(Pos, Option<Dir>)>, player: bool) -> Result<(), SubmitError> {
            if plan.simulation {
                return Err(SubmitError::IncompletePlan);
            }
            if self.job.is_some() {
                return Err(SubmitError::CpuBusy);
            }
            if !active {
                return Err(SubmitError::CpuOffline);
            }
            if storage < plan.bytes {
                return Err(SubmitError::CpuTooSmall);
            }
            for (k, want) in &plan.used {
                let got = net_extract(net, k, *want, false);
                list_add(&mut self.inventory, k, got);
                if got < *want {
                    self.store(net);
                    return Err(SubmitError::MissingIngredient(k.clone(), want - got));
                }
            }
            let mut total = 0u64;
            for n in plan.emitted.values() {
                total = total.saturating_add(*n);
            }
            self.job = Some(Job {
                id,
                output: plan.output.clone(),
                remaining: plan.output.1,
                tasks: plan.crafts.clone(),
                waiting: plan.emitted.clone(),
                requester,
                player,
                total,
            });
            Ok(())
        }
        /// The job's whole item count (outputs of every task), for the progress line.
        pub fn set_total(&mut self, book: &Book) {
            if let Some(j) = self.job.as_mut() {
                let mut t = j.waiting.values().sum::<u64>();
                for (def, n) in &j.tasks {
                    if let Some(p) = book.get(def) {
                        t = t.saturating_add(p.outputs.iter().map(|(_, c)| c.saturating_mul(*n)).sum());
                    }
                }
                j.total = t;
            }
        }
        /// tickCraftingLogic: without a job the CPU stores its items; with one it pushes up to
        /// coprocessors + 1 patterns over 3 ticks.
        pub fn tick(&mut self, coprocessors: u32, book: &Book, machines: &mut dyn Machines, energy: &mut dyn EnergySource, net: &mut dyn Mounts, item_of: ItemOf) {
            if self.job.is_none() {
                self.idle(net);
            } else {
                self.run(coprocessors, book, machines, energy, item_of);
            }
        }
        /// The tick without a job: the CPU stores what it holds.
        pub fn idle(&mut self, net: &mut dyn Mounts) {
            self.store(net);
            self.cant_store = !self.inventory.is_empty();
        }
        /// The tick with a job: up to coprocessors + 1 pushes over the last 3 ticks.
        pub fn run(&mut self, coprocessors: u32, book: &Book, machines: &mut dyn Machines, energy: &mut dyn EnergySource, item_of: ItemOf) {
            self.cant_store = false;
            let started = i64::from(coprocessors) + 1 - i64::from(self.used_ops.iter().sum::<u32>());
            let mut remaining = started;
            while remaining > 0 {
                let pushed = self.execute(remaining as u32, book, machines, energy, item_of);
                if pushed == 0 {
                    break;
                }
                remaining -= i64::from(pushed);
            }
            self.used_ops[2] = self.used_ops[1];
            self.used_ops[1] = self.used_ops[0];
            self.used_ops[0] = (started - remaining).max(0) as u32;
        }
        /// executeCrafting.
        pub fn execute(&mut self, max: u32, book: &Book, machines: &mut dyn Machines, energy: &mut dyn EnergySource, item_of: ItemOf) -> u32 {
            let Some(job) = self.job.as_mut() else { return 0 };
            let mut pushed = 0u32;
            let mut ti = 0;
            'tasks: while ti < job.tasks.len() {
                if job.tasks[ti].1 == 0 {
                    job.tasks.remove(ti);
                    continue;
                }
                let def = job.tasks[ti].0.clone();
                let Some(details) = book.get(&def) else {
                    ti += 1;
                    continue;
                };
                let mut container = extract_pattern_inputs(details, &mut self.inventory, item_of);
                let n = machines.providers(&def);
                for prov in 0..n {
                    let Some((table, expected)) = container.as_ref() else { break };
                    if machines.busy(prov) {
                        continue;
                    }
                    let power = pattern_power(table);
                    if extract_config(energy, power, true) < power - 0.01 {
                        break;
                    }
                    if machines.push(prov, details, table) {
                        extract_config(energy, power, false);
                        pushed += 1;
                        for (k, c) in expected {
                            list_add(&mut job.waiting, k, *c);
                        }
                        container = None;
                        job.tasks[ti].1 -= 1;
                        if job.tasks[ti].1 == 0 {
                            job.tasks.remove(ti);
                            continue 'tasks;
                        }
                        if pushed == max {
                            break 'tasks;
                        }
                        container = extract_pattern_inputs(details, &mut self.inventory, item_of);
                    }
                }
                if let Some((table, _)) = container {
                    for (k, c) in table.iter().flatten() {
                        list_add(&mut self.inventory, k, *c);
                    }
                }
                ti += 1;
            }
            pushed
        }
        /// CraftingCpuLogic.insert: only what the job waits for; the final output is counted
        /// (and may finish the job), the rest goes into the CPU's inventory.
        pub fn insert(&mut self, key: &Key, amount: u64, simulate: bool) -> Accept {
            let Some(job) = self.job.as_mut() else { return Accept::default() };
            let waiting = job.waiting.get(key).copied().unwrap_or(0);
            if waiting == 0 || amount == 0 {
                return Accept::default();
            }
            let amount = amount.min(waiting);
            if !simulate {
                list_extract(&mut job.waiting, key, amount, true);
            }
            if *key == job.output.0 {
                if !simulate {
                    job.remaining = job.remaining.saturating_sub(amount);
                    if job.remaining == 0 {
                        self.finish(true);
                    }
                }
                Accept { stored: 0, output: amount }
            } else {
                if !simulate {
                    list_add(&mut self.inventory, key, amount);
                }
                Accept { stored: amount, output: 0 }
            }
        }
        fn finish(&mut self, success: bool) {
            if let Some(j) = self.job.take() {
                self.finished.push(Finished { output: j.output, success, player: j.player });
            }
        }
        /// cancel: the job ends; what the CPU holds is stored on the next tick (items already
        /// in machines come back through the providers into the network).
        pub fn cancel(&mut self) {
            self.finish(false);
        }
        /// storeItems: into the network; what does not fit stays (cantStoreItems).
        pub fn store(&mut self, net: &mut dyn Mounts) {
            let keys: Vec<(Key, u64)> = self.inventory.iter().map(|(k, n)| (k.clone(), *n)).collect();
            for (k, n) in keys {
                let ins = net_insert(net, &k, n, false);
                list_extract(&mut self.inventory, &k, ins, true);
            }
        }
        /// CraftingCPUMenu / CraftingStatus: per key stored, active (waited for) and pending
        /// (scheduled outputs), the keys in order.
        pub fn status(&self, book: &Book) -> Vec<(Key, u64, u64, u64)> {
            let mut m: BTreeMap<Key, (u64, u64, u64)> = BTreeMap::new();
            for (k, n) in &self.inventory {
                m.entry(k.clone()).or_default().0 += n;
            }
            if let Some(j) = &self.job {
                for (k, n) in &j.waiting {
                    m.entry(k.clone()).or_default().1 += n;
                }
                for (def, times) in &j.tasks {
                    if let Some(p) = book.get(def) {
                        for (k, c) in &p.outputs {
                            m.entry(k.clone()).or_default().2 += c.saturating_mul(*times);
                        }
                    }
                }
            }
            m.into_iter().map(|(k, (a, b, c))| (k, a, b, c)).collect()
        }
        /// Every key a job waits for (CraftingService.currentlyCrafting, isRequesting).
        pub fn requesting(&self) -> BTreeSet<Key> {
            self.job.iter().flat_map(|j| j.waiting.keys().cloned()).collect()
        }
        /// writeToNBT / readFromNBT into the core block's record.
        pub fn save_to(&self, d: &mut Device) {
            d.set_list("!cpu", self.inventory.iter().map(|(k, n)| (k.clone(), *n)).collect());
            for (i, u) in self.used_ops.iter().enumerate() {
                d.set_num(&format!("cpu_u{i}"), i64::from(*u));
            }
            for k in ["cpu_job", "cpu_rem", "cpu_req", "cpu_rx", "cpu_ry", "cpu_rz", "cpu_rs", "cpu_player", "cpu_total"] {
                d.set_num(k, 0);
            }
            match &self.job {
                None => {
                    d.set_list("cpu_wait", Vec::new());
                    d.set_list("cpu_tasks", Vec::new());
                    d.set_list("cpu_out", Vec::new());
                }
                Some(j) => {
                    d.set_num("cpu_job", j.id.max(1));
                    d.set_num("cpu_rem", j.remaining.min(i64::MAX as u64) as i64);
                    d.set_num("cpu_player", i64::from(j.player));
                    d.set_num("cpu_total", j.total.min(i64::MAX as u64) as i64);
                    if let Some((p, side)) = j.requester {
                        d.set_num("cpu_req", 1);
                        d.set_num("cpu_rx", i64::from(p[0]));
                        d.set_num("cpu_ry", i64::from(p[1]));
                        d.set_num("cpu_rz", i64::from(p[2]));
                        d.set_num("cpu_rs", side.map_or(0, |s| s.idx() as i64 + 1));
                    }
                    d.set_list("cpu_wait", j.waiting.iter().map(|(k, n)| (k.clone(), *n)).collect());
                    d.set_list("cpu_tasks", j.tasks.clone());
                    d.set_list("cpu_out", vec![j.output.clone()]);
                }
            }
        }
        pub fn load_from(d: &Device) -> Cpu {
            let mut c = Cpu::default();
            for (k, n) in d.list("!cpu") {
                list_add(&mut c.inventory, k, *n);
            }
            for i in 0..3 {
                c.used_ops[i] = d.num(&format!("cpu_u{i}")).clamp(0, i64::from(u32::MAX)) as u32;
            }
            let id = d.num("cpu_job");
            if id != 0
                && let Some((k, n)) = d.list("cpu_out").first().cloned()
            {
                let requester = (d.num("cpu_req") != 0).then(|| {
                    let p = [d.num("cpu_rx") as i32, d.num("cpu_ry") as i32, d.num("cpu_rz") as i32];
                    let s = d.num("cpu_rs");
                    (p, if s > 0 { Dir::from_idx(s as usize - 1) } else { None })
                });
                let mut waiting = BTreeMap::new();
                for (k, n) in d.list("cpu_wait") {
                    list_add(&mut waiting, k, *n);
                }
                c.job = Some(Job {
                    id,
                    output: (k, n),
                    remaining: d.num("cpu_rem").max(0) as u64,
                    tasks: d.list("cpu_tasks").to_vec(),
                    waiting,
                    requester,
                    player: d.num("cpu_player") != 0,
                    total: d.num("cpu_total").max(0) as u64,
                });
            }
            c
        }
    }

    // ---- 12.5 pattern providers (PatternProviderLogic) -------------------------------------

    /// AEBaseInterface / PatternProviderLogic: 9 pattern slots and a 9-slot return inventory.
    pub const PROVIDER_PATTERNS: usize = 9;
    pub const PROVIDER_RETURN: usize = 9;
    /// The adjacent inventories a provider pushes to (PatternProviderTarget), by side.
    pub trait Targets {
        /// getActiveSides with an item handler, in Direction order.
        fn sides(&mut self) -> Vec<Dir>;
        fn insert(&mut self, side: Dir, key: &Key, amount: u64, simulate: bool) -> u64;
        /// containsPatternInput (blocking mode).
        fn contains_any(&mut self, side: Dir, keys: &BTreeSet<Key>) -> bool;
    }
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Provider {
        /// sendList: pushed stacks that did not fit yet (real items).
        pub send: Vec<(Key, u64)>,
        pub send_dir: Option<Dir>,
        pub round_robin: usize,
        pub blocking: bool,
    }
    impl Provider {
        pub fn busy(&self) -> bool {
            !self.send.is_empty()
        }
        /// pushPattern to generic inventories (the adapter tries crafting machines, the
        /// molecular assemblers, first): round robin, blocking mode, a target that accepts some
        /// of every input; the rest of a push waits in the send list.
        pub fn push(&mut self, details: &Details, table: &[Vec<(Key, u64)>], targets: &mut dyn Targets, pattern_inputs: &BTreeSet<Key>) -> bool {
            if self.busy() {
                return false;
            }
            let mut sides = targets.sides();
            if sides.is_empty() {
                return false;
            }
            self.round_robin %= sides.len();
            sides.rotate_left(self.round_robin);
            for (i, side) in sides.iter().copied().enumerate() {
                if self.blocking && targets.contains_any(side, pattern_inputs) {
                    continue;
                }
                let accepts_all = table.iter().flatten().all(|(k, n)| targets.insert(side, k, *n, true) > 0);
                if !accepts_all {
                    continue;
                }
                for (k, n) in details.push_order(table) {
                    let ins = targets.insert(side, &k, n, false);
                    if ins < n {
                        self.send.push((k, n - ins));
                    }
                }
                self.send_dir = Some(side);
                self.flush(targets);
                self.round_robin += i + 1;
                return true;
            }
            false
        }
        /// sendStacksOut.
        pub fn flush(&mut self, targets: &mut dyn Targets) -> bool {
            let Some(side) = self.send_dir else { return false };
            let mut did = false;
            for e in self.send.iter_mut() {
                let ins = targets.insert(side, &e.0, e.1, false);
                if ins > 0 {
                    did = true;
                    e.1 -= ins.min(e.1);
                }
            }
            self.send.retain(|(_, n)| *n > 0);
            if self.send.is_empty() {
                self.send_dir = None;
            }
            did
        }
        pub fn save_to(&self, d: &mut Device) {
            d.set_list("!send", self.send.clone());
            d.set_num("send_dir", self.send_dir.map_or(0, |s| s.idx() as i64 + 1));
            d.set_num("rr", self.round_robin as i64);
            d.set_num("blocking", i64::from(self.blocking));
        }
        pub fn load_from(d: &Device) -> Provider {
            let s = d.num("send_dir");
            Provider {
                send: d.list("!send").to_vec(),
                send_dir: if s > 0 { Dir::from_idx(s as usize - 1) } else { None },
                round_robin: d.num("rr").max(0) as usize,
                blocking: d.num("blocking") != 0,
            }
        }
    }

    // ---- 12.6 the molecular assembler (MolecularAssemblerBlockEntity) ----------------------

    /// tickingRequest: progress per tick and the energy tax by installed speed cards.
    pub fn assembler_speed(cards: u32) -> (i32, f64) {
        match cards {
            0 => (10, 1.0),
            1 => (13, 1.3),
            2 => (17, 1.7),
            3 => (20, 2.0),
            4 => (25, 2.5),
            _ => (50, 5.0),
        }
    }
    /// userPower: the progress the grid's energy pays for (ticks * bonus * tax AE, / tax).
    pub fn assembler_progress(energy: &mut dyn EnergySource, ticks: u32, cards: u32) -> i32 {
        let (bonus, tax) = assembler_speed(cards);
        (extract_config(energy, f64::from(ticks) * f64::from(bonus) * tax, false) / tax) as i32
    }
    /// The progress one craft needs.
    pub const ASSEMBLER_PROGRESS: i32 = 100;
    /// A full grid (`assemble` gives the output): every used slot holds a valid item, the
    /// others nothing.
    pub fn assembler_ready(d: &Details, grid: &[Option<Key>], item_of: ItemOf) -> bool {
        (0..9).all(|s| d.grid_valid(s, grid.get(s).and_then(|x| x.as_ref()), item_of))
    }
    /// One craft: the output and the grid after it (getRemainingItems: each used slot keeps
    /// its input's container item).
    pub fn assembler_craft(d: &Details, grid: &[Option<Key>], item_of: ItemOf) -> Option<((Key, u64), Vec<Option<Key>>)> {
        if !assembler_ready(d, grid, item_of) {
            return None;
        }
        let mut after = vec![None; 9];
        for (s, cell) in after.iter_mut().enumerate() {
            if let Some(i) = d.grid.get(s).copied().flatten() {
                *cell = d.inputs[i].remainder.clone();
            }
        }
        Some((d.primary().clone(), after))
    }
    /// ejectHeldItems: the first grid item the plan does not accept (or every item without a
    /// plan) moves to the output slot.
    pub fn assembler_eject(d: Option<&Details>, grid: &[Option<Key>], item_of: ItemOf) -> Option<usize> {
        (0..9.min(grid.len())).find(|&s| {
            grid[s].as_ref().is_some_and(|k| match d {
                None => true,
                Some(d) => !d.grid_valid(s, Some(k), item_of),
            })
        })
    }

    // ---- 12.7 upgrade cards (InitUpgrades, IOBusPart, StorageBusPart) ---------------------

    pub const SPEED_CARD: &str = "ae2:speed_card";
    pub const CAPACITY_CARD: &str = "ae2:capacity_card";
    pub const FUZZY_CARD: &str = "ae2:fuzzy_card";
    pub const INVERTER_CARD: &str = "ae2:inverter_card";
    pub const CRAFTING_CARD: &str = "ae2:crafting_card";
    /// The devices taking cards in this port.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Host {
        ImportBus,
        ExportBus,
        StorageBus,
        Interface,
        LevelEmitter,
        Assembler,
        ItemCell,
    }
    /// getUpgradeSlots: IOBusPart 5, StorageBusPart 5, InterfaceLogic 1, StorageLevelEmitterPart 1,
    /// MolecularAssemblerBlockEntity 5, BasicStorageCell (items) 4.
    pub fn upgrade_slots(h: Host) -> usize {
        match h {
            Host::ImportBus | Host::ExportBus | Host::StorageBus | Host::Assembler => 5,
            Host::Interface | Host::LevelEmitter => 1,
            Host::ItemCell => 4,
        }
    }
    /// Upgrades.getMaxInstallable (InitUpgrades). Redstone, void and equal distribution cards
    /// are not done in this port (they are refused).
    pub fn upgrade_max(h: Host, card: &str) -> u32 {
        match (h, card) {
            (Host::ImportBus, FUZZY_CARD) | (Host::ImportBus, INVERTER_CARD) => 1,
            (Host::ImportBus | Host::ExportBus, CAPACITY_CARD) => 5,
            (Host::ImportBus | Host::ExportBus, SPEED_CARD) => 4,
            (Host::ExportBus, FUZZY_CARD) | (Host::ExportBus, CRAFTING_CARD) => 1,
            (Host::StorageBus, FUZZY_CARD) | (Host::StorageBus, INVERTER_CARD) => 1,
            (Host::StorageBus, CAPACITY_CARD) => 5,
            (Host::Interface, CRAFTING_CARD) | (Host::Interface, FUZZY_CARD) => 1,
            (Host::LevelEmitter, FUZZY_CARD) | (Host::LevelEmitter, CRAFTING_CARD) => 1,
            (Host::Assembler, SPEED_CARD) => 5,
            (Host::ItemCell, FUZZY_CARD) | (Host::ItemCell, INVERTER_CARD) => 1,
            _ => 0,
        }
    }
    /// A card may go into one more upgrade slot.
    pub fn may_install(h: Host, installed: &[String], card: &str) -> bool {
        let max = upgrade_max(h, card);
        max > 0 && (installed.iter().filter(|c| *c == card).count() as u32) < max
    }
    pub fn count_cards(installed: &[String], card: &str) -> u32 {
        installed.iter().filter(|c| *c == card).count() as u32
    }
    /// IOBusPart.getOperationsPerTick by speed cards.
    pub fn io_ops(speed: u32) -> u64 {
        match speed {
            0 => 1,
            1 => 8,
            2 => 32,
            3 => 64,
            _ => 96,
        }
    }
    /// The IO bus config slots in use (IOBusMenu: 1, then 4 more per capacity card, 9 at most).
    pub fn io_slots(capacity: u32) -> usize {
        (1 + 4 * capacity as usize).min(9)
    }
    /// StorageBusPart: 18 config slots, 9 more per capacity card, 63 at most.
    pub fn storage_bus_slots(capacity: u32) -> usize {
        (18 + 9 * capacity as usize).min(63)
    }
    /// A partition / filter with the fuzzy and inverter cards (FuzzyMode.IGNORE_ALL only).
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Filter {
        pub keys: Vec<Key>,
        pub inverted: bool,
        pub fuzzy: bool,
    }
    impl Filter {
        pub fn listed(&self, k: &Key, item_of: ItemOf) -> bool {
            if self.fuzzy {
                let item = item_of(k);
                self.keys.iter().any(|x| item_of(x) == item)
            } else {
                self.keys.contains(k)
            }
        }
        /// passesBlackOrWhitelist: an empty list lets everything through.
        pub fn passes(&self, k: &Key, item_of: ItemOf) -> bool {
            self.keys.is_empty() || self.listed(k, item_of) != self.inverted
        }
        /// The import bus: `isInFilter(what) == isInverted()` skips (an empty filter with an
        /// inverter imports nothing).
        pub fn imports(&self, k: &Key, item_of: ItemOf) -> bool {
            let in_filter = self.keys.is_empty() || self.listed(k, item_of);
            in_filter != self.inverted
        }
        /// isPreferredStorageFor: whitelisted.
        pub fn prefers(&self, k: &Key, item_of: ItemOf) -> bool {
            !self.inverted && !self.keys.is_empty() && self.listed(k, item_of)
        }
    }
    /// StorageImportStrategy with the bus's filter and speed (the stage-1 import_transfer with
    /// fuzzy / inverter cards).
    pub fn import_filtered<W: Mounts + super::Inv>(w: &mut W, filter: &Filter, item_of: ItemOf, ops: u64) -> super::Moved {
        let mut remaining_ops = ops;
        let mut moved = super::Moved::default();
        let slots = super::Inv::slots(w);
        for s in 0..slots {
            if remaining_ops == 0 {
                break;
            }
            let Some((key, _)) = super::Inv::get(w, s) else { continue };
            if !filter.imports(&key, item_of) {
                continue;
            }
            let fits = net_insert(w, &key, remaining_ops, true);
            let amount = super::inv_extract(w, &key, fits, false);
            if amount > 0 {
                let inserted = net_insert(w, &key, amount, false);
                if inserted < amount {
                    let back = super::inv_insert(w, &key, amount - inserted, false);
                    if back < amount - inserted {
                        moved.stray.push((key.clone(), amount - inserted - back));
                    }
                }
                remaining_ops -= inserted.max(1).min(remaining_ops);
                moved.items += inserted;
            }
        }
        moved
    }
    /// ExportBusPart.doBusWork with cards: the configured keys (fuzzy: every network key of
    /// the same item), up to `ops` items; `idle` = the config slots that moved nothing (the
    /// crafting card asks for those).
    pub fn export_filtered<W: Mounts + super::Inv>(w: &mut W, e: &mut dyn EnergySource, config: &[Option<Key>], fuzzy: bool, net_keys: &[Key], item_of: ItemOf, ops: u64) -> (super::Moved, Vec<usize>, u64) {
        let mut remaining = ops;
        let mut moved = super::Moved::default();
        let mut idle = Vec::new();
        for (slot, key) in config.iter().enumerate() {
            if remaining == 0 {
                break;
            }
            let Some(key) = key else { continue };
            let before = remaining;
            let keys: Vec<Key> = if fuzzy {
                let item = item_of(key);
                net_keys.iter().filter(|k| item_of(k) == item).cloned().collect()
            } else {
                vec![key.clone()]
            };
            for k in keys {
                if remaining == 0 {
                    break;
                }
                let m = super::export_transfer(w, e, &[Some(k)], remaining);
                remaining -= m.items.min(remaining);
                moved.items += m.items;
                moved.stray.extend(m.stray);
            }
            if remaining == before {
                idle.push(slot);
            }
        }
        (moved, idle, remaining)
    }

    // ---- 12.8 the level emitter (StorageLevelEmitterPart, AbstractLevelEmitterPart) -------

    /// updateReportingValue: the configured key's amount; with a fuzzy card every key of the
    /// same item; without a key all items (both stop counting once above the level).
    pub fn emitter_reported(net: &BTreeMap<Key, u64>, key: Option<&Key>, fuzzy: bool, item_of: ItemOf, level: u64) -> u64 {
        match key {
            Some(k) if !fuzzy => net.get(k).copied().unwrap_or(0),
            _ => {
                let item = key.map(|k| item_of(k));
                let mut v = 0u64;
                for (k, n) in net {
                    if item.as_ref().is_none_or(|i| item_of(k) == *i) {
                        v = v.saturating_add(*n);
                        if v > level {
                            break;
                        }
                    }
                }
                v
            }
        }
    }
    /// isLevelEmitterOn: off when inactive; with a crafting card "emit while crafting" (a CPU
    /// waits for the key, or for anything without one); otherwise HIGH_SIGNAL emits when the
    /// amount reaches the level, LOW_SIGNAL below it.
    pub fn emitter_on(active: bool, crafting_card: bool, requesting: bool, low_signal: bool, level: u64, reported: u64) -> bool {
        if !active {
            return false;
        }
        if crafting_card {
            return requesting;
        }
        if low_signal { level >= reported.saturating_add(1) } else { level < reported.saturating_add(1) }
    }

    // ---- 12.9 cell partitions and cell upgrades (CellWorkbench, BasicCellInventory) --------

    /// The cell's config inventory: 63 partition keys and the cards in the cell.
    pub const CELL_PARTITION: usize = 63;
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct CellConfig {
        pub partition: Vec<Option<Key>>,
        pub upgrades: Vec<String>,
    }
    impl CellConfig {
        pub fn encode(&self) -> String {
            let p: Vec<String> = self.partition.iter().enumerate().filter_map(|(i, k)| k.as_ref().map(|k| format!("{i}={k}"))).collect();
            format!("ae2c1;{};{}", p.join(","), self.upgrades.join(","))
        }
        pub fn decode(t: &str) -> CellConfig {
            let mut c = CellConfig { partition: vec![None; CELL_PARTITION], upgrades: Vec::new() };
            let w: Vec<&str> = t.split(';').collect();
            if w.len() != 3 || w[0] != "ae2c1" {
                return c;
            }
            for part in w[1].split(',') {
                if let Some((i, k)) = part.split_once('=')
                    && let Ok(i) = i.parse::<usize>()
                    && i < CELL_PARTITION
                    && !k.is_empty()
                {
                    c.partition[i] = Some(k.to_owned());
                }
            }
            c.upgrades = w[2].split(',').filter(|x| !x.is_empty()).map(str::to_owned).collect();
            c
        }
        pub fn is_empty(&self) -> bool {
            self.partition.iter().all(Option::is_none) && self.upgrades.is_empty()
        }
        /// The partition with the cell's fuzzy and inverter cards.
        pub fn filter(&self) -> Filter {
            Filter {
                keys: self.partition.iter().flatten().cloned().collect(),
                inverted: self.upgrades.iter().any(|c| c == INVERTER_CARD),
                fuzzy: self.upgrades.iter().any(|c| c == FUZZY_CARD),
            }
        }
        /// The workbench's "partition storage" action: the cell's stored keys.
        pub fn partition_from(contents: &[(Key, u64)]) -> Vec<Option<Key>> {
            let mut p: Vec<Option<Key>> = contents.iter().take(CELL_PARTITION).map(|(k, _)| Some(k.clone())).collect();
            p.resize(CELL_PARTITION, None);
            p
        }
    }

    /// The pattern provider pushes nowhere it would loop: other providers, and interfaces of
    /// the same network (getActiveSides).
    pub fn provider_skips(neighbour_is_provider: bool, neighbour_is_interface_same_net: bool) -> bool {
        neighbour_is_provider || neighbour_is_interface_same_net
    }
    /// The side a part or block pushes through (the provider block pushes to every side).
    pub fn neighbour(p: Pos, d: Dir) -> Pos {
        offset(p, d)
    }
}

}
use engine::{CellInv, Device, Dir, Key, Loc, NodeKind, NodeSpec, Pos};
use engine::crafting as cr;
use std::rc::Rc;

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
const MOD_ID: &str = "ae2";
/// The cable bus while one of its parts emits redstone (template `minecraft:trapped_chest`, the only
/// kind of state Horizon asks for a signal per position: mods/foundry/API.md section 11, "Redstone").
const CABLE_BUS_SIGNAL: &str = "ae2:cable_bus_signal";
/// The AE2 windows of guests (single player's window, LAN friends) as foundry menus.
const AE2_MENU: &str = "ae2:window";

/// The block at `p` by name; the cable bus's signal twin is the cable bus.
fn ae2_block_name(g: &Game<'_>, p: [i32; 3]) -> Option<std::sync::Arc<str>> {
    let n = kit_blocks::name_at(g, p)?;
    if *n == *CABLE_BUS_SIGNAL {
        return Some(std::sync::Arc::from(engine::CABLE_BUS));
    }
    Some(n)
}

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
    /// going through `chat.rs` internals; see TRANSLATOR.md).
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

thread_local! {
    /// The server's AE2 windows of guests, by connection (the host's own is `Rt::window`), and the
    /// host's window while a guest's is in place (`ae2_menu_enter` / `ae2_menu_leave`).
    static GUESTS: RefCell<HashMap<u32, Window>> = RefCell::new(HashMap::new());
    static STASH: RefCell<Vec<Option<Window>>> = const { RefCell::new(Vec::new()) };
    /// A guest's window: the device of the open window as the server sent it, and the rest.
    static GUEST_ST: RefCell<engine::State> = RefCell::new(engine::State::default());
    static GUEST_VIEW: RefCell<GuestView> = RefCell::new(GuestView::default());
    static ST: RefCell<kit_state::PerWorld<engine::State>> = const { RefCell::new(kit_state::PerWorld::new()) };
    static RT: RefCell<Rt> = RefCell::new(Rt::default());
    static KEYS: RefCell<HashMap<Key, ItemStack>> = RefCell::new(HashMap::new());
}

/// The world's AE2 records (loaded on first use). `None` when re-entered or without a local
/// world; every write marks the world for the next save.
fn with_st<R>(g: &Game<'_>, f: impl FnOnce(&mut engine::State) -> R) -> Option<R> {
    if g.remote.is_some() {
        // A server's client: the device of the AE2 window it shows (a foundry menu), as the server
        // sent it; what changes here is the copy's.
        return GUEST_ST.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)));
    }
    kit_state::dir(g)?;
    ST.with(|s| {
        let mut s = s.try_borrow_mut().ok()?;
        let r = f(s.level(g, engine::State::load));
        s.dirty(g);
        Some(r)
    })
}
fn with_rt<R>(f: impl FnOnce(&mut Rt) -> R) -> Option<R> {
    RT.with(|r| r.try_borrow_mut().ok().map(|mut r| f(&mut r)))
}

// ---------------------------------------------------------------------------------------------
// Item keys: the game's item NBT (all components) of a stack of one, as hex.

fn key_of(s: &ItemStack) -> Key {
    use horizon_world::nbt::{Limits, NamedTag, write_named};
    let one = ItemStack { count: 1, ..s.clone() };
    let root = NamedTag { name: JavaString::from(""), tag: horizon_client::save::item_to_nbt(&one) };
    let bytes = write_named(&root, Limits::default()).unwrap_or_default();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
    }
    KEYS.with(|k| {
        if let Ok(mut k) = k.try_borrow_mut() {
            k.entry(out.clone()).or_insert(one);
        }
    });
    out
}
fn decode_key(text: &str) -> Option<ItemStack> {
    use horizon_world::nbt::{Limits, read_named};
    let t = text.as_bytes();
    if t.is_empty() || t.len() % 2 != 0 {
        return None;
    }
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let bytes: Vec<u8> = t.chunks(2).map(|p| Some(hex(p[0])? << 4 | hex(p[1])?)).collect::<Option<_>>()?;
    let (named, _) = read_named(&bytes, Limits::default()).ok()?;
    horizon_client::save::item_from_nbt(&named.tag)
}
/// The stack of `n` items of a key.
fn stack_of(k: &str, n: u32) -> Option<ItemStack> {
    let cached = KEYS.with(|m| m.try_borrow().ok().and_then(|m| m.get(k).cloned()));
    let one = match cached {
        Some(s) => s,
        None => {
            let s = decode_key(k)?;
            KEYS.with(|m| {
                if let Ok(mut m) = m.try_borrow_mut() {
                    m.insert(k.to_owned(), s.clone());
                }
            });
            s
        }
    };
    Some(ItemStack { count: n, ..one })
}
fn item_of_key(k: &str) -> Option<String> {
    stack_of(k, 1).map(|s| s.item.to_string())
}

// ---------------------------------------------------------------------------------------------
// Cells: the contents travel in the cell item (custom data `ae2:cell`) and live in the record
// while the cell sits in a drive or chest.

fn custom(s: &ItemStack) -> Option<&Tag> {
    s.data().and_then(|d| d.other("minecraft:custom_data"))
}
fn cell_text(s: &ItemStack) -> Option<String> {
    match custom(s)?.get("ae2:cell")? {
        Tag::String(j) => Some(j.to_string_lossy()),
        _ => None,
    }
}
fn set_cell_text(s: &mut ItemStack, text: Option<&str>) {
    let mut map = match custom(s) {
        Some(Tag::Compound(m)) => m.clone(),
        _ => Default::default(),
    };
    map.remove(&JavaString::from("ae2:cell"));
    if let Some(t) = text.filter(|t| !t.is_empty()) {
        map.insert(JavaString::from("ae2:cell"), Tag::String(JavaString::from(t)));
    }
    let value = (!map.is_empty()).then_some(Tag::Compound(map));
    if value.is_none() && s.data().is_none() {
        return;
    }
    s.data_mut().set_other("minecraft:custom_data", value);
}
/// A storage cell that holds something (`canFitInsideCell` false).
fn is_full_cell(s: &ItemStack) -> bool {
    engine::cell_type(&s.item).is_some() && cell_text(s).is_some_and(|t| !t.trim().is_empty())
}
fn key_is_full_cell(k: &str) -> bool {
    stack_of(k, 1).is_some_and(|s| is_full_cell(&s))
}
/// A cell entering a drive: its key without contents, and the contents.
fn split_cell(s: &ItemStack) -> (Key, CellInv) {
    let inv = cell_text(s).map(|t| CellInv::decode(&t)).unwrap_or_default();
    let mut bare = s.clone();
    set_cell_text(&mut bare, None);
    (key_of(&bare), inv)
}
/// A cell leaving a drive: the item with its contents written in.
fn join_cell(k: &str, inv: Option<&CellInv>) -> Option<ItemStack> {
    let mut s = stack_of(k, 1)?;
    if let Some(inv) = inv.filter(|i| !i.items.is_empty()) {
        set_cell_text(&mut s, Some(&inv.encode()));
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// What each AE2 block is.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Cable(u8, bool),
    Controller,
    Drive,
    Chest,
    Interface(bool),
    Terminal,
    CraftingTerminal,
    ImportBus,
    ExportBus,
    StorageBus,
    Acceptor(bool),
    Charger,
    Inscriber,
    Vibration,
    Crank,
    // stage 2
    CraftUnit(cr::Unit),
    Assembler,
    Provider(bool),
    EncodingTerminal,
    AccessTerminal,
    LevelEmitter,
    Workbench,
}
const COLORS: [&str; 17] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple",
    "blue", "brown", "green", "red", "black", "fluix",
];
fn kind_of(name: &str) -> Option<Kind> {
    let path = name.strip_prefix("ae2:")?;
    Some(match path {
        "controller" | "controller__online" | "controller__conflicted" => Kind::Controller,
        "drive" => Kind::Drive,
        "chest" => Kind::Chest,
        "interface" => Kind::Interface(false),
        "cable_interface" => Kind::Interface(true),
        "terminal" => Kind::Terminal,
        "crafting_terminal" => Kind::CraftingTerminal,
        "import_bus" => Kind::ImportBus,
        "export_bus" => Kind::ExportBus,
        "storage_bus" => Kind::StorageBus,
        "energy_acceptor" => Kind::Acceptor(false),
        "cable_energy_acceptor" => Kind::Acceptor(true),
        "charger" => Kind::Charger,
        "inscriber" => Kind::Inscriber,
        "vibration_chamber" => Kind::Vibration,
        "crank" => Kind::Crank,
        "molecular_assembler" => Kind::Assembler,
        "pattern_provider" => Kind::Provider(false),
        "cable_pattern_provider" => Kind::Provider(true),
        "pattern_encoding_terminal" => Kind::EncodingTerminal,
        "pattern_access_terminal" => Kind::AccessTerminal,
        "level_emitter" => Kind::LevelEmitter,
        "cell_workbench" => Kind::Workbench,
        _ if cr::Unit::of(name).is_some() => Kind::CraftUnit(cr::Unit::of(name).unwrap()),
        _ => {
            for (dense, suffixes) in [(false, ["_glass_cable", "_covered_cable", "_smart_cable"].as_slice()), (true, &["_covered_dense_cable", "_smart_dense_cable"])] {
                for suf in suffixes {
                    if let Some(color) = path.strip_suffix(suf)
                        && let Some(c) = COLORS.iter().position(|x| *x == color)
                    {
                        return Some(Kind::Cable(c as u8, dense));
                    }
                }
            }
            return None;
        }
    })
}
impl Kind {
    /// (real slots, ghost config slots) of the record.
    fn sizes(self) -> (usize, usize) {
        match self {
            Kind::Drive => (10, 0),
            Kind::Chest => (1, 0),
            // Stage 2: one upgrade slot after the 9 storage slots.
            Kind::Interface(_) => (engine::config::INTERFACE_SLOTS + 1, engine::config::INTERFACE_SLOTS),
            Kind::CraftingTerminal => (9, 0),
            // Stage 2: 5 upgrade slots; the config grows with capacity cards (9 / 63).
            Kind::ImportBus | Kind::ExportBus => (5, 9),
            Kind::StorageBus => (5, cr::CELL_PARTITION),
            // Grid 0..8, output 9, pattern 10, speed cards 11..15.
            Kind::Assembler => (16, 0),
            // Patterns 0..8, return inventory 9..17.
            Kind::Provider(_) => (cr::PROVIDER_PATTERNS + cr::PROVIDER_RETURN, 0),
            // Blank 0, encoded 1; ghosts: 81 inputs then 27 outputs.
            Kind::EncodingTerminal => (2, cr::MAX_PROCESSING_INPUTS + cr::MAX_PROCESSING_OUTPUTS),
            Kind::LevelEmitter => (1, 1),
            Kind::Workbench => (1, 0),
            Kind::Inscriber => (4, 0),
            Kind::Charger | Kind::Vibration => (1, 0),
            _ => (0, 0),
        }
    }
    /// A part block (barrel template): its front is the opposite of the barrel's facing.
    fn part(self) -> bool {
        matches!(
            self,
            Kind::Terminal | Kind::CraftingTerminal | Kind::ImportBus | Kind::ExportBus | Kind::StorageBus | Kind::Interface(true) | Kind::Acceptor(true)
        )
    }
    fn needs_channel(self) -> bool {
        matches!(
            self,
            Kind::Drive
                | Kind::Chest
                | Kind::Interface(_)
                | Kind::Terminal
                | Kind::CraftingTerminal
                | Kind::ImportBus
                | Kind::ExportBus
                | Kind::StorageBus
                | Kind::CraftUnit(_)
                | Kind::Provider(_)
                | Kind::EncodingTerminal
                | Kind::AccessTerminal
        )
    }
    /// GridNode.idlePowerUsage of the block (cells are added on top for drives and chests).
    fn idle(self) -> f64 {
        match self {
            Kind::Controller => engine::config::CONTROLLER_IDLE,
            // DriveBlockEntity.updateState: 2 + cells.
            Kind::Drive => 2.0,
            // ChestBlockEntity: 1 + cell (the default 1 with none).
            Kind::Chest | Kind::Interface(_) | Kind::ImportBus | Kind::ExportBus | Kind::StorageBus => 1.0,
            // AbstractReportingPart with REQUIRE_CHANNEL: 1/2.
            Kind::Terminal | Kind::CraftingTerminal | Kind::EncodingTerminal | Kind::AccessTerminal => 0.5,
            // GridNode's default 1 (crafting blocks, providers, the level emitter); the assembler 0.
            Kind::CraftUnit(_) | Kind::Provider(_) | Kind::LevelEmitter => 1.0,
            _ => 0.0,
        }
    }
    fn is_node(self) -> bool {
        !matches!(self, Kind::Crank | Kind::Workbench)
    }
    /// The upgrade host (InitUpgrades) and where its cards are in the record.
    fn upgrades(self) -> Option<(cr::Host, std::ops::Range<usize>)> {
        Some(match self {
            Kind::ImportBus => (cr::Host::ImportBus, 0..5),
            Kind::ExportBus => (cr::Host::ExportBus, 0..5),
            Kind::StorageBus => (cr::Host::StorageBus, 0..5),
            Kind::Interface(_) => (cr::Host::Interface, 9..10),
            Kind::LevelEmitter => (cr::Host::LevelEmitter, 0..1),
            Kind::Assembler => (cr::Host::Assembler, 11..16),
            _ => return None,
        })
    }
    fn holds_power(self) -> f64 {
        match self {
            Kind::Inscriber => engine::config::INSCRIBER_BUFFER,
            Kind::Charger => engine::config::CHARGER_BUFFER,
            Kind::Chest => engine::config::CHEST_BUFFER,
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The networks of the current dimension (not saved: rebuilt from the records, as AE2 rebuilds
// grids when a world loads).

#[derive(Default)]
struct Rt {
    tick: u64,
    /// The node list the networks were built from (and the cable-bus faces holding parts).
    topology: Vec<(Loc, NodeKind)>,
    blocked: std::collections::BTreeSet<(Pos, Dir)>,
    nets: Vec<Net>,
    net_of: HashMap<Loc, usize>,
    tickers: HashMap<Loc, engine::Ticker>,
    /// foundry's host generation the records were last matched with.
    bus_gen: u64,
    /// The faces with parts at foundry's host generation `blocked_gen`.
    blocked_gen: u64,
    blocked_cache: std::collections::BTreeSet<(Pos, Dir)>,
    window: Option<Window>,
    /// rememberLastSearch (client.json: true).
    last_search: String,
    /// The world and dimension these networks belong to.
    world_key: String,
    /// The topology changed: the looks follow at once.
    looks_dirty: bool,
    // ---- stage 2
    /// The formed crafting CPUs (rebuilt with the topology).
    clusters: Vec<cr::Cluster>,
    /// Decoded patterns by item key (None = not a valid pattern).
    details: HashMap<Key, Option<Rc<cr::Details>>>,
    /// Each network's patterns, by network index.
    books: HashMap<usize, Rc<NetBook>>,
    /// Messages for the player (finished jobs).
    notices: Vec<String>,
    next_job: i64,
}
/// A network's crafting service: its patterns, the providers holding each, what CPUs wait for.
#[derive(Default)]
struct NetBook {
    tick: u64,
    book: Rc<cr::Book>,
    providers: Rc<HashMap<Key, Vec<Loc>>>,
    requesting: std::collections::BTreeSet<Key>,
}
#[derive(Clone)]
struct Net {
    members: Vec<Loc>,
    anchor: Loc,
    power: engine::GridPower,
    granted: std::collections::HashSet<Loc>,
    state: engine::ControllerState,
}

/// Node capacity in the grid pool (GridEnergyStoragePerNode, + the controller's 8000).
fn node_capacity(k: Kind) -> f64 {
    engine::config::POWER_RATIOS_GRID_ENERGY_STORAGE_PER_NODE
        + if k == Kind::Controller { engine::config::CONTROLLER_STORAGE } else { 0.0 }
}

/// The energy source of an operation: the network pool.
struct Pool<'a>(&'a mut engine::GridPower);
impl engine::EnergySource for Pool<'_> {
    fn extract_ae(&mut self, amount: f64, simulate: bool) -> f64 {
        self.0.extract_ae(amount, simulate)
    }
}
/// The ME chest's source: its own buffer first, then the network (ChestBlockEntity).
struct ChestSource<'a> {
    buffer: &'a mut engine::Buffer,
    grid: &'a mut engine::GridPower,
}
impl engine::EnergySource for ChestSource<'_> {
    fn extract_ae(&mut self, amount: f64, simulate: bool) -> f64 {
        let own = self.buffer.extract(amount, simulate);
        own + self.grid.extract_ae(amount - own, simulate)
    }
}

/// The block family a record keeps (the controller's looks are one block).
fn family(name: &str) -> String {
    match kind_of(name) {
        Some(Kind::Controller) => "ae2:controller".to_owned(),
        _ => name.to_owned(),
    }
}
fn center(p: Pos) -> [f64; 3] {
    [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5]
}

/// One storage on a network (`Mounts` index).
#[derive(Clone, Debug)]
enum MountRef {
    Cell { pos: Loc, slot: usize, prio: i32, ty: &'static engine::CellType, filter: cr::Filter },
    Bus { target: Pos, face: Dir, filter: engine::BusFilter, cards: cr::Filter },
    /// A crafting CPU with a job (CraftingServiceStorage: priority Integer.MAX_VALUE, takes
    /// only what it waits for).
    Cpu { core: Pos },
}
fn item_id(k: &Key) -> String {
    item_of_key(k).unwrap_or_default()
}

/// The game as the engine's bus world: the network's storages and, for a bus, its target.
struct Live<'a, 'g> {
    game: &'a mut Game<'g>,
    mounts: Vec<MountRef>,
    target: Option<(Pos, Dir)>,
    /// The providers of each pattern (a CPU pushing) and the pattern asked about.
    prov: Rc<HashMap<Key, Vec<Loc>>>,
    cur_def: Key,
}
impl<'a, 'g> Live<'a, 'g> {
    fn new(game: &'a mut Game<'g>, mounts: Vec<MountRef>) -> Self {
        Live { game, mounts, target: None, prov: Rc::new(HashMap::new()), cur_def: Key::new() }
    }
    fn with_target<R>(&mut self, f: impl FnOnce(&mut dyn super::foundry::ItemHandler<ItemStack>) -> R) -> Option<R> {
        let (p, face) = self.target?;
        let mut out = None;
        let mut f = Some(f);
        super::foundry::with_items(self.game, p, Some(fdir(face)), &mut |h| {
            if let Some(f) = f.take() {
                out = Some(f(h));
            }
        });
        out
    }
}
/// engine::Dir -> foundry::Dir (same order: Direction.get3DDataValue).
fn fdir(d: Dir) -> super::foundry::Dir {
    super::foundry::Dir::ALL[d.idx()]
}
fn bus_inv_op<R>(game: &mut Game<'_>, target: Pos, face: Dir, f: impl FnOnce(&mut HandlerInv<'_>) -> R) -> Option<R> {
    let mut out = None;
    let mut f = Some(f);
    super::foundry::with_items(game, target, Some(fdir(face)), &mut |h| {
        if let Some(f) = f.take() {
            out = Some(f(&mut HandlerInv { h }));
        }
    });
    out
}
/// A foundry item handler as the engine's `Inv`.
struct HandlerInv<'h> {
    h: &'h mut dyn super::foundry::ItemHandler<ItemStack>,
}
impl engine::Inv for HandlerInv<'_> {
    fn slots(&mut self) -> usize {
        self.h.slots()
    }
    fn get(&mut self, slot: usize) -> Option<(Key, u64)> {
        self.h.stack_in_slot(slot).filter(|s| s.count > 0).map(|s| (key_of(&s), u64::from(s.count)))
    }
    fn insert(&mut self, slot: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        let n = amount.min(u64::from(u32::MAX)) as u32;
        let Some(stack) = stack_of(key, n) else { return 0 };
        let rest = self.h.insert_item(slot, stack, simulate).map_or(0, |r| r.count);
        u64::from(n.saturating_sub(rest))
    }
    fn extract(&mut self, slot: usize, amount: u64, simulate: bool) -> u64 {
        let n = amount.min(u64::from(u32::MAX)) as u32;
        self.h.extract_item(slot, n, simulate).map_or(0, |s| u64::from(s.count))
    }
}
impl engine::Mounts for Live<'_, '_> {
    fn mounts(&self) -> usize {
        self.mounts.len()
    }
    fn priority(&self, i: usize) -> i32 {
        match &self.mounts[i] {
            MountRef::Cell { prio, .. } => *prio,
            MountRef::Bus { filter, .. } => filter.priority,
            MountRef::Cpu { .. } => i32::MAX,
        }
    }
    fn insert(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        match self.mounts[i].clone() {
            MountRef::Cpu { core } => self.game.ae2_cpu_accept(core, key, amount, simulate),
            MountRef::Cell { pos, slot, ty, filter, .. } => {
                // The cell's partition with its fuzzy / inverter cards (BasicCellInventory).
                if !filter.passes(key, &item_id) {
                    return 0;
                }
                let nested = key_is_full_cell(key);
                with_st(self.game, |s| {
                    let d = s.devices.at_mut(pos)?;
                    Some(d.cells.entry(slot).or_default().insert(ty, key, amount, simulate, nested))
                })
                .flatten()
                .unwrap_or(0)
            }
            MountRef::Bus { target, face, cards, .. } => {
                if !cards.passes(key, &item_id) {
                    return 0;
                }
                bus_inv_op(self.game, target, face, |inv| engine::inv_insert(inv, key, amount, simulate)).unwrap_or(0)
            }
        }
    }
    fn extract(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        match self.mounts[i].clone() {
            MountRef::Cpu { .. } => 0,
            MountRef::Cell { pos, slot, .. } => with_st(self.game, |s| {
                let d = s.devices.at_mut(pos)?;
                let c = d.cells.get_mut(&slot)?;
                let n = c.extract(key, amount, simulate);
                if c.items.is_empty() {
                    d.cells.remove(&slot);
                }
                Some(n)
            })
            .flatten()
            .unwrap_or(0),
            MountRef::Bus { target, face, cards, .. } => {
                // FILTER_ON_EXTRACT: the partition applies to extraction too.
                if !cards.passes(key, &item_id) {
                    return 0;
                }
                bus_inv_op(self.game, target, face, |inv| engine::inv_extract(inv, key, amount, simulate)).unwrap_or(0)
            }
        }
    }
    fn listed(&self, i: usize, key: &Key) -> bool {
        match &self.mounts[i] {
            MountRef::Bus { cards, .. } => cards.prefers(key, &item_id),
            MountRef::Cell { filter, .. } => filter.prefers(key, &item_id),
            MountRef::Cpu { .. } => true,
        }
    }
    fn list(&mut self, i: usize, out: &mut BTreeMap<Key, u64>) {
        match self.mounts[i].clone() {
            MountRef::Cpu { .. } => {}
            MountRef::Cell { pos, slot, .. } => {
                let items = with_st(self.game, |s| s.devices.at(pos).and_then(|d| d.cells.get(&slot)).map(|c| c.items.clone()))
                    .flatten()
                    .unwrap_or_default();
                for (k, n) in items {
                    *out.entry(k).or_insert(0) += n;
                }
            }
            MountRef::Bus { target, face, .. } => {
                bus_inv_op(self.game, target, face, |inv| engine::inv_list(inv, out));
            }
        }
    }
}
impl engine::Inv for Live<'_, '_> {
    fn slots(&mut self) -> usize {
        self.with_target(|h| h.slots()).unwrap_or(0)
    }
    fn get(&mut self, slot: usize) -> Option<(Key, u64)> {
        self.with_target(|h| HandlerInv { h }.get_slot(slot)).flatten()
    }
    fn insert(&mut self, slot: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        self.with_target(|h| engine::Inv::insert(&mut HandlerInv { h }, slot, key, amount, simulate)).unwrap_or(0)
    }
    fn extract(&mut self, slot: usize, amount: u64, simulate: bool) -> u64 {
        self.with_target(|h| engine::Inv::extract(&mut HandlerInv { h }, slot, amount, simulate)).unwrap_or(0)
    }
}
impl HandlerInv<'_> {
    fn get_slot(&mut self, slot: usize) -> Option<(Key, u64)> {
        engine::Inv::get(self, slot)
    }
}

impl Game<'_> {
    fn ae2_active(&self) -> bool {
        self.remote.is_none() && !self.world_root.as_os_str().is_empty()
    }
    /// The id of what stands at `at`: a whole block, or (in a cable bus) the cable / the part on
    /// that face. `None` when the chunk is not loaded or nothing is there.
    fn ae2_name_at(&self, at: impl Into<Loc>) -> Option<String> {
        let l = at.into();
        let name = ae2_block_name(self, l.pos)?;
        if *name == *engine::CABLE_BUS {
            return super::foundry::part_at(self, l.pos, l.side.map(fdir));
        }
        l.side.is_none().then(|| name.to_string())
    }
    fn ae2_kind_at(&self, at: impl Into<Loc>) -> Option<Kind> {
        kind_of(&self.ae2_name_at(at)?)
    }
    /// The face a device works on: a cable-bus part's own face; carved_pumpkin `facing`
    /// (machines); the opposite of the barrel `facing` (stage-1 part blocks, until migrated).
    fn ae2_front(&self, at: impl Into<Loc>, k: Kind) -> Option<Dir> {
        let l = at.into();
        if let Some(side) = l.side {
            return Some(side);
        }
        let st = kit_blocks::state_at(self, l.pos);
        let f = Dir::parse(&kit_blocks::prop(self, st, "facing")?)?;
        Some(if k.part() { f.opposite() } else { f })
    }
    /// The record of the AE2 block or part at `at`, made on first sight.
    fn ae2_ensure(&self, at: impl Into<Loc>) -> Option<Kind> {
        let l = at.into();
        let name = self.ae2_name_at(l)?;
        let k = kind_of(&name)?;
        let fam = family(&name);
        with_st(self, |s| {
            if s.devices.at(l).is_some_and(|d| d.block != fam) {
                return; // replaced: the tick spills the old record first
            }
            let (a, b) = k.sizes();
            let d = s.devices.entry(l).or_insert_with(|| {
                let mut d = Device::new(&fam, a, b);
                if k.holds_power() > 0.0 {
                    d.set_fl("buffer", 0.0);
                }
                d
            });
            // A record from an older layout gets the new slots (stage 2: upgrade cards).
            d.grow(a, b);
        });
        Some(k)
    }
    fn ae2_max_stack(&self, item: &str) -> u32 {
        self.res.items().get(item).map_or(64, |i| i.max_stack).max(1)
    }
    fn ae2_drop(&mut self, p: Pos, stack: ItemStack) {
        if stack.count == 0 {
            return;
        }
        self.spawn_item(stack, center(p), super::dispenser::new_item_motion(), 10);
    }
    fn ae2_give(&mut self, stack: ItemStack) {
        if let Some(rest) = self.give(stack)
            && let Some(p) = self.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z])
        {
            self.spawn_item(rest, p, [0.0, 0.0, 0.0], 0);
        }
    }

    /// Records whose block or part is gone spill everything (a cell keeps its contents); pending
    /// drops are spawned.
    fn ae2_gone(&mut self) {
        let records: Vec<(Loc, String)> =
            with_st(self, |s| s.devices.iter().map(|(l, d)| (*l, d.block.clone())).collect()).unwrap_or_default();
        let mut gone = Vec::new();
        for (l, block) in records {
            if !kit_blocks::loaded(self, l.pos) {
                continue;
            }
            let here = self.ae2_name_at(l).map(|n| family(&n));
            if here.as_deref() != Some(block.as_str()) {
                gone.push(l);
            }
        }
        if !gone.is_empty() {
            with_st(self, |s| {
                for l in &gone {
                    let Some(mut d) = s.devices.remove(l) else { continue };
                    // A CPU's inventory, a provider's send list (real items, any amount).
                    for (k, n) in d.take_list_items() {
                        let mut left = n;
                        while left > 0 {
                            let c = left.min(u64::from(u32::MAX)) as u32;
                            s.drops.push((l.pos, k.clone(), c));
                            left -= u64::from(c);
                        }
                    }
                    for (i, k, n) in d.take_items() {
                        // The cell's contents go into its item (a text the drop keeps).
                        let k = match d.cells.remove(&i) {
                            Some(inv) => join_cell(&k, Some(&inv)).map(|s| key_of(&s)).unwrap_or(k),
                            None => k,
                        };
                        s.drops.push((l.pos, k, n));
                    }
                }
            });
            with_rt(|rt| rt.topology.clear());
        }
        let drops = with_st(self, |s| std::mem::take(&mut s.drops)).unwrap_or_default();
        for (p, k, n) in drops {
            let max = item_of_key(&k).map_or(64, |i| self.ae2_max_stack(&i));
            match stack_of(&k, n) {
                Some(_) => {
                    let mut left = n;
                    while left > 0 {
                        let c = left.min(max);
                        if let Some(s) = stack_of(&k, c) {
                            self.ae2_drop(p, s);
                        }
                        left -= c;
                    }
                }
                // An unreadable key (a removed item): kept for the next save, never voided.
                None => {
                    with_st(self, |s| s.drops.push((p, k.clone(), n)));
                }
            }
        }
    }

    /// The cable buses' parts and cables get their records (foundry tells when hosts change), and
    /// stage-1 whole cables and part blocks become cable buses.
    fn ae2_bus_sync(&mut self) {
        let generation = super::foundry::hosts_generation();
        if with_rt(|rt| rt.bus_gen == generation) == Some(true) {
            return;
        }
        for (p, host) in super::foundry::hosts_of(self, engine::CABLE_BUS) {
            if !kit_blocks::loaded(self, p) {
                continue;
            }
            for (slot, part) in host.parts() {
                if kind_of(&part.id).is_some() {
                    let side = slot.and_then(|d| Dir::from_idx(d.idx()));
                    self.ae2_ensure(Loc { pos: p, side });
                }
            }
        }
        with_rt(|rt| {
            rt.bus_gen = generation;
            rt.topology.clear();
        });
    }
    /// Stage 1 had no multipart: its cables and part blocks still standing in a world become
    /// cable buses (engine::stage1_migration). The records keep everything they held: a part's
    /// record moves to its face, a cable's stays in the centre.
    fn ae2_migrate(&mut self) {
        let candidates: Vec<(Loc, String)> = with_st(self, |s| {
            s.devices
                .iter()
                .filter(|(l, d)| l.side.is_none() && (engine::cable_of(&d.block).is_some() || engine::part_def(&d.block).is_some()))
                .map(|(l, d)| (*l, d.block.clone()))
                .collect()
        })
        .unwrap_or_default();
        for (l, block) in candidates {
            let Some(name) = ae2_block_name(self, l.pos) else { continue };
            if *name != *block {
                continue; // already a cable bus (or gone: the gone pass spills it)
            }
            let facing = kit_blocks::prop(self, kit_blocks::state_at(self, l.pos), "facing").and_then(|f| Dir::parse(&f));
            let Some(m) = engine::stage1_migration(&block, facing) else { continue };
            self.ae2_migrate_one(l.pos, &m);
        }
    }
    fn ae2_migrate_one(&mut self, p: Pos, m: &engine::Migration) {
        if self.pack.assets.registry.default_state(engine::CABLE_BUS).is_none() || super::foundry::part_spec(&m.center).is_none() {
            return; // no cable bus in this build: the stage-1 block keeps working as it was
        }
        let old = ae2_block_name(self, p).map(|n| n.to_string()).unwrap_or_default();
        // The block goes first (no drops: set_block), then foundry's host takes its place.
        let air = self.world.air;
        let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, air, level_flags::UPDATE_CLIENTS));
        let _ = self.process_level_events();
        let mut center = super::foundry::Part::new(&m.center);
        if m.center_legacy {
            center.set(super::foundry::NO_DROP, 1);
        }
        if !super::foundry::put_part(self, p, None, center) {
            // Nothing is lost: the block's item drops (its record spills at the next tick).
            log::warn!("ae2: the stage-1 block at {p:?} could not become a cable bus");
            if let Some(s) = stack_of(&key_of(&ItemStack::new(&old, 1)), 1) {
                self.ae2_drop(p, s);
            }
            with_rt(|rt| rt.topology.clear());
            return;
        }
        if let Some((side, id)) = &m.part {
            if super::foundry::put_part(self, p, Some(fdir(*side)), super::foundry::Part::new(id)) {
                with_st(self, |s| {
                    if let Some(d) = s.devices.remove(&Loc::block(p)) {
                        s.devices.insert(Loc::part(p, *side), d);
                    }
                });
            } else {
                // The stub alone is no use: it goes, the part's item drops.
                super::foundry::take_part(self, p, None);
                if let Some(s) = stack_of(&key_of(&ItemStack::new(id, 1)), 1) {
                    self.ae2_drop(p, s);
                }
            }
        }
        with_rt(|rt| rt.topology.clear());
    }

    /// The node list of the loaded records, and the cable-bus faces holding parts.
    fn ae2_specs(&self) -> (Vec<(engine::BusNode, Kind)>, std::collections::BTreeSet<(Pos, Dir)>) {
        let records: Vec<(Loc, String)> =
            with_st(self, |s| s.devices.iter().map(|(l, d)| (*l, d.block.clone())).collect()).unwrap_or_default();
        let mut out = Vec::new();
        for (l, block) in records {
            if !kit_blocks::loaded(self, l.pos) {
                continue;
            }
            let Some(k) = kind_of(&block) else { continue };
            if !k.is_node() {
                continue;
            }
            let hosted = l.side.is_some() || matches!(k, Kind::Cable(..));
            let kind = match k {
                Kind::Cable(c, dense) => NodeKind::Cable { color: c, dense },
                Kind::Controller => NodeKind::Controller,
                _ => NodeKind::Device {
                    channel: k.needs_channel(),
                    // A stage-1 part block not yet migrated: its front does not connect.
                    front: if k.part() && !hosted { self.ae2_front(l, k) } else { None },
                },
            };
            out.push((engine::BusNode { loc: l, kind, idle: k.idle() }, k));
        }
        (out, self.ae2_blocked())
    }
    /// The cable-bus faces holding a part (nodeless ones too), again only when foundry's hosts
    /// changed.
    fn ae2_blocked(&self) -> std::collections::BTreeSet<(Pos, Dir)> {
        let generation = super::foundry::hosts_generation();
        if let Some(Some(b)) = with_rt(|rt| (rt.blocked_gen == generation).then(|| rt.blocked_cache.clone())) {
            return b;
        }
        let mut blocked = std::collections::BTreeSet::new();
        for (p, host) in super::foundry::hosts_of(self, engine::CABLE_BUS) {
            for (slot, _) in host.parts() {
                if let Some(d) = slot.and_then(|d| Dir::from_idx(d.idx())) {
                    blocked.insert((p, d));
                }
            }
        }
        with_rt(|rt| {
            rt.blocked_gen = generation;
            rt.blocked_cache = blocked.clone();
        });
        blocked
    }

    /// Rebuilds the networks when the topology changed: channels, controller state; the
    /// energy of the old networks goes back to their members first.
    fn ae2_networks(&mut self) {
        let (specs, blocked) = self.ae2_specs();
        let topology: Vec<(Loc, NodeKind)> = specs.iter().map(|(s, _)| (s.loc, s.kind)).collect();
        if with_rt(|rt| {
            rt.topology == topology && rt.blocked == blocked && !rt.nets.is_empty() || topology.is_empty() && rt.topology.is_empty()
        }) == Some(true)
        {
            return;
        }
        self.ae2_store_energy();
        let nodes: Vec<engine::BusNode> = specs.iter().map(|(s, _)| s.clone()).collect();
        let kinds: HashMap<Loc, Kind> = specs.iter().map(|(s, k)| (s.loc, *k)).collect();
        let energies: HashMap<Loc, f64> =
            with_st(self, |s| s.devices.iter().map(|(l, d)| (*l, d.energy)).collect()).unwrap_or_default();
        let old: HashMap<Loc, engine::GridPower> =
            with_rt(|rt| rt.nets.iter().map(|n| (n.anchor, n.power.clone())).collect()).unwrap_or_default();
        let (node_specs, networks) = engine::bus_networks(&nodes, &blocked);
        // Stage 2: the formed crafting CPUs; each takes one channel (GridFlags.MULTIBLOCK).
        let units: BTreeMap<Pos, cr::Unit> =
            specs.iter().filter(|(s, _)| s.loc.side.is_none()).filter_map(|(s, k)| if let Kind::CraftUnit(u) = k { Some((s.loc.pos, *u)) } else { None }).collect();
        let clusters = cr::clusters(&units);
        let mut groups: BTreeMap<usize, usize> = BTreeMap::new();
        for (ci, c) in clusters.iter().enumerate() {
            for (ni, n) in nodes.iter().enumerate() {
                if n.loc.side.is_none() && c.members.contains(&n.loc.pos) {
                    groups.insert(ni, ci);
                }
            }
        }
        let mut nets = Vec::new();
        let mut net_of = HashMap::new();
        for net in networks {
            let ch = engine::assign_channels_grouped(&node_specs, &net, &groups);
            let members: Vec<Loc> = net.members.iter().map(|&i| nodes[i].loc).collect();
            let anchor = *members.iter().min().unwrap_or(&Loc::block([0, 0, 0]));
            let capacity: f64 = members.iter().map(|l| node_capacity(kinds[l])).sum();
            let stored: f64 = members.iter().map(|l| energies.get(l).copied().unwrap_or(0.0)).sum();
            let mut power = engine::GridPower::new(stored, capacity);
            if let Some(o) = old.get(&anchor) {
                power.has_power = o.has_power;
                power.ticks_since_change = o.ticks_since_change;
                power.powered = o.powered;
            }
            power.channel_usage = f64::from(ch.by_blocks) / engine::config::CHANNEL_POWER_DIVISOR;
            for l in &members {
                net_of.insert(*l, nets.len());
            }
            nets.push(Net {
                granted: ch.granted.iter().map(|&i| nodes[i].loc).collect(),
                state: ch.state,
                members,
                anchor,
                power,
            });
        }
        with_rt(|rt| {
            rt.topology = topology;
            rt.blocked = blocked;
            rt.looks_dirty = true;
            rt.nets = nets;
            rt.net_of = net_of;
            rt.clusters = clusters;
            rt.books.clear();
        });
        self.ae2_cpu_validate();
    }
    /// Spreads each network's energy back over its members (by capacity).
    fn ae2_store_energy(&mut self) {
        let nets: Vec<(f64, f64, Vec<Loc>)> =
            with_rt(|rt| rt.nets.iter().map(|n| (n.power.stored, n.power.capacity, n.members.clone())).collect()).unwrap_or_default();
        let mut shares: Vec<(Loc, f64)> = Vec::new();
        for (stored, cap, members) in nets {
            let frac = if cap > 0.0 { (stored / cap).clamp(0.0, 1.0) } else { 0.0 };
            for l in members {
                if let Some(k) = self.ae2_kind_at(l) {
                    shares.push((l, frac * node_capacity(k)));
                }
            }
        }
        with_st(self, |s| {
            for (l, e) in &shares {
                if let Some(d) = s.devices.at_mut(*l) {
                    d.energy = *e;
                }
            }
        });
    }
    /// The network index of a node.
    fn ae2_net(&self, at: impl Into<Loc>) -> Option<usize> {
        let l = at.into();
        with_rt(|rt| rt.net_of.get(&l).copied()).flatten()
    }
    fn ae2_powered(&self, at: impl Into<Loc>) -> bool {
        let l = at.into();
        with_rt(|rt| rt.net_of.get(&l).is_some_and(|&n| rt.nets[n].power.powered)) == Some(true)
    }
    /// AE2 `isActive`: powered, and holding a channel when it needs one.
    fn ae2_device_active(&self, at: impl Into<Loc>, k: Kind) -> bool {
        let l = at.into();
        with_rt(|rt| {
            rt.net_of.get(&l).is_some_and(|&n| rt.nets[n].power.powered && (!k.needs_channel() || rt.nets[n].granted.contains(&l)))
        }) == Some(true)
    }
    /// The storages of the network of `at` (active drives, chests and storage buses).
    fn ae2_mounts(&self, at: impl Into<Loc>) -> Vec<MountRef> {
        let l = at.into();
        let Some(members) = with_rt(|rt| rt.net_of.get(&l).map(|&n| rt.nets[n].members.clone())).flatten() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for m in members {
            // The member's record tells its kind (no world lookup for every cable).
            let Some(k) = with_st(self, |s| s.devices.at(m).and_then(|d| kind_of(&d.block))).flatten() else { continue };
            if let Kind::CraftUnit(_) = k {
                // A CPU with a job intercepts what it waits for (its core holds the job).
                if with_rt(|rt| rt.clusters.iter().any(|c| c.core() == m.pos)) == Some(true)
                    && with_st(self, |s| s.devices.at(m).is_some_and(|d| d.num("cpu_job") != 0)) == Some(true)
                    && self.ae2_device_active(m, k)
                {
                    out.push(MountRef::Cpu { core: m.pos });
                }
                continue;
            }
            if !matches!(k, Kind::Drive | Kind::Chest | Kind::StorageBus) || !self.ae2_device_active(m, k) {
                continue;
            }
            let rec = with_st(self, |s| s.devices.at(m).cloned()).flatten();
            let Some(d) = rec else { continue };
            let prio = d.num("priority") as i32;
            match k {
                Kind::StorageBus => {
                    let Some(front) = self.ae2_front(m, k) else { continue };
                    let cards = device_cards(&d, k);
                    let shown = cr::storage_bus_slots(cr::count_cards(&cards, cr::CAPACITY_CARD));
                    let partition: Vec<Key> = d.config.iter().take(shown).flatten().map(|(k, _)| k.clone()).collect();
                    let filter = cr::Filter {
                        keys: partition.clone(),
                        inverted: cards.iter().any(|c| c == cr::INVERTER_CARD),
                        fuzzy: cards.iter().any(|c| c == cr::FUZZY_CARD),
                    };
                    out.push(MountRef::Bus {
                        target: engine::offset(m.pos, front),
                        face: front.opposite(),
                        filter: engine::BusFilter { priority: prio, partition },
                        cards: filter,
                    });
                }
                _ => {
                    for (i, s) in d.slots.iter().enumerate() {
                        if let Some((key, _)) = s
                            && let Some(ty) = item_of_key(key).and_then(|id| engine::cell_type(&id))
                        {
                            let filter = stack_of(key, 1).map(|st| cell_config(&st).filter()).unwrap_or_default();
                            out.push(MountRef::Cell { pos: m, slot: i, prio, ty, filter });
                        }
                    }
                }
            }
        }
        out
    }
    /// Runs `f` with the network of `at` (its storages and energy pool).
    fn ae2_with_network<R>(&mut self, at: impl Into<Loc>, f: impl FnOnce(&mut Live<'_, '_>, &mut engine::GridPower) -> R) -> Option<R> {
        let l = at.into();
        let n = self.ae2_net(l)?;
        let mut power = with_rt(|rt| rt.nets.get(n).map(|x| x.power.clone())).flatten()?;
        let mounts = self.ae2_mounts(l);
        let r = {
            let mut live = Live::new(self, mounts);
            f(&mut live, &mut power)
        };
        let stored = power.stored;
        with_rt(|rt| {
            if let Some(x) = rt.nets.get_mut(n) {
                x.power.stored = stored;
            }
        });
        Some(r)
    }
}

// ---- foundry providers: energy in, items of the machines and interfaces ----

/// The record a capability question reaches: in a cable bus the part on the asked face (AE2 parts
/// give capabilities for their own side only), otherwise the block itself.
fn ae2_provider_loc(game: &Game<'_>, p: Pos, face: Option<super::foundry::Dir>) -> Option<Loc> {
    let name = ae2_block_name(game, p)?;
    if *name == *engine::CABLE_BUS {
        let side = Dir::from_idx(face?.idx())?;
        return Some(Loc::part(p, side));
    }
    Some(Loc::block(p))
}
fn ae2_energy_claims(game: &Game<'_>, p: Pos, face: Option<super::foundry::Dir>) -> bool {
    let Some(l) = ae2_provider_loc(game, p, face) else { return false };
    matches!(game.ae2_kind_at(l), Some(Kind::Acceptor(_) | Kind::Controller | Kind::Inscriber | Kind::Charger | Kind::Chest))
}
/// AEBasePoweredBlockEntity's ForgeEnergyAdapter: the acceptor and the controller funnel
/// into the network, the machines into their own buffer.
struct FeIn {
    demand_ae: f64,
    stored_ae: f64,
    max_ae: f64,
    taken_ae: f64,
}
impl super::foundry::EnergyHandler for FeIn {
    fn receive_energy(&mut self, max: i32, simulate: bool) -> i32 {
        let fe = engine::fe_receive(self.demand_ae - self.taken_ae, max);
        if !simulate {
            self.taken_ae += engine::fe_to_ae(f64::from(fe));
        }
        fe
    }
    fn extract_energy(&mut self, _max: i32, _simulate: bool) -> i32 {
        0
    }
    fn energy_stored(&self) -> i32 {
        engine::ae_to_fe(self.stored_ae + self.taken_ae).min(f64::from(i32::MAX)) as i32
    }
    fn max_energy_stored(&self) -> i32 {
        engine::ae_to_fe(self.max_ae).min(f64::from(i32::MAX)) as i32
    }
    fn can_extract(&self) -> bool {
        false
    }
    fn can_receive(&self) -> bool {
        true
    }
}
fn ae2_energy(
    game: &mut Game<'_>,
    pos: Pos,
    face: Option<super::foundry::Dir>,
    f: &mut dyn FnMut(&mut dyn super::foundry::EnergyHandler),
) -> bool {
    let Some(p) = ae2_provider_loc(game, pos, face) else { return false };
    let Some(k) = game.ae2_ensure(p) else { return false };
    if matches!(k, Kind::Acceptor(_) | Kind::Controller) {
        game.ae2_networks();
        let Some(n) = game.ae2_net(p) else { return false };
        let Some((demand, stored, cap)) = with_rt(|rt| rt.nets.get(n).map(|x| (x.power.demand(), x.power.stored, x.power.capacity))).flatten() else {
            return false;
        };
        let mut h = FeIn { demand_ae: demand, stored_ae: stored, max_ae: cap, taken_ae: 0.0 };
        f(&mut h);
        let taken = h.taken_ae;
        with_rt(|rt| {
            if let Some(x) = rt.nets.get_mut(n) {
                x.power.inject(taken, false);
            }
        });
        return true;
    }
    let max = k.holds_power();
    let Some(stored) = with_st(game, |s| s.devices.at(p).map(|d| d.fl("buffer"))).flatten() else { return false };
    let mut h = FeIn { demand_ae: (max - stored).max(0.0), stored_ae: stored, max_ae: max, taken_ae: 0.0 };
    f(&mut h);
    let taken = h.taken_ae;
    if taken > 0.0 {
        with_st(game, |s| {
            if let Some(d) = s.devices.at_mut(p) {
                let v = (d.fl("buffer") + taken).min(max);
                d.set_fl("buffer", v);
            }
        });
    }
    true
}

fn ae2_item_claims(game: &Game<'_>, p: Pos, face: Option<super::foundry::Dir>) -> bool {
    // A block whose window is open: nothing reaches the window's temporary slots.
    if game.ae2_window_at((p[0], p[1], p[2]))
        && ae2_block_name(game, p).is_some_and(|n| kind_of(&n).is_some() || *n == *engine::CABLE_BUS)
    {
        return true;
    }
    let Some(l) = ae2_provider_loc(game, p, face) else { return false };
    matches!(game.ae2_kind_at(l), Some(Kind::Interface(_) | Kind::Inscriber | Kind::Charger | Kind::Vibration | Kind::Provider(_) | Kind::Assembler))
}
/// The slots a device shows to pipes and hoppers, with AE2's filters.
struct DevItems<'a> {
    kind: Kind,
    face: Option<Dir>,
    front: Option<Dir>,
    slots: Vec<Option<ItemStack>>,
    max: Vec<u32>,
    tags: &'a dyn Fn(&str, &str) -> bool,
    fuel: &'a dyn Fn(&ItemStack) -> bool,
    /// The assembler's plan accepts this item in this grid slot.
    grid_ok: &'a dyn Fn(usize, &ItemStack) -> bool,
    changed: bool,
}
impl DevItems<'_> {
    /// The record slots visible from `face` (InscriberBlockEntity.getExposedInventoryForSide:
    /// top = top plate, bottom = bottom plate, sides = middle in / result out).
    fn visible(&self) -> Vec<usize> {
        match self.kind {
            Kind::Interface(_) => (0..engine::config::INTERFACE_SLOTS.min(self.slots.len())).collect(),
            // PatternProviderReturnInventory: what machines give back, inserted only.
            Kind::Provider(_) => (cr::PROVIDER_PATTERNS..cr::PROVIDER_PATTERNS + cr::PROVIDER_RETURN).collect(),
            // gridInv: 9 inputs + the output (slot 9, extract only).
            Kind::Assembler => (0..10).collect(),
            Kind::Inscriber => match self.face {
                Some(Dir::Up) => vec![0],
                Some(Dir::Down) => vec![1],
                _ => vec![2, 3],
            },
            Kind::Charger | Kind::Vibration => vec![0],
            _ => vec![],
        }
    }
    fn may_insert(&self, slot: usize, s: &ItemStack) -> bool {
        match (self.kind, slot) {
            (Kind::Interface(_), _) => true,
            (Kind::Provider(_), _) => true,
            (Kind::Assembler, 0..9) => self.slots[slot].is_none() && (self.grid_ok)(slot, s),
            (Kind::Inscriber, 0 | 1) => engine::inscriber_plate_ok(&s.item, self.tags),
            (Kind::Inscriber, 2) => engine::inscriber_middle_ok(&s.item, self.tags),
            (Kind::Charger, 0) => engine::charger_result(&s.item).is_some(),
            (Kind::Vibration, 0) => (self.fuel)(s),
            _ => false,
        }
    }
    fn may_extract(&self, slot: usize) -> bool {
        match (self.kind, slot) {
            (Kind::Interface(_), _) => true,
            (Kind::Assembler, 9) => true,
            (Kind::Inscriber, 3) => true,
            (Kind::Charger, 0) => self.slots[0].as_ref().is_some_and(|s| engine::charger_may_take(&s.item)),
            _ => false,
        }
    }
}
impl super::foundry::ItemHandler<ItemStack> for DevItems<'_> {
    fn slots(&self) -> usize {
        self.visible().len()
    }
    fn stack_in_slot(&self, slot: usize) -> Option<ItemStack> {
        let real = *self.visible().get(slot)?;
        self.slots[real].clone()
    }
    fn insert_item(&mut self, slot: usize, stack: ItemStack, simulate: bool) -> Option<ItemStack> {
        let Some(&real) = self.visible().get(slot) else { return Some(stack) };
        if stack.count == 0 || !self.may_insert(real, &stack) {
            return Some(stack);
        }
        let _ = self.front;
        let limit = self.max[real];
        let room = match &self.slots[real] {
            None => limit,
            Some(c) if c.same_item_same_components(&stack) => limit.saturating_sub(c.count),
            Some(_) => 0,
        };
        let n = stack.count.min(room);
        if n == 0 {
            return Some(stack);
        }
        if !simulate {
            match &mut self.slots[real] {
                Some(c) => c.count += n,
                None => self.slots[real] = Some(ItemStack { count: n, ..stack.clone() }),
            }
            self.changed = true;
        }
        (stack.count > n).then(|| ItemStack { count: stack.count - n, ..stack })
    }
    fn extract_item(&mut self, slot: usize, amount: u32, simulate: bool) -> Option<ItemStack> {
        let &real = self.visible().get(slot)?;
        if !self.may_extract(real) {
            return None;
        }
        let cur = self.slots[real].as_mut()?;
        let n = cur.count.min(amount);
        if n == 0 {
            return None;
        }
        let out = ItemStack { count: n, ..cur.clone() };
        if !simulate {
            cur.count -= n;
            if cur.count == 0 {
                self.slots[real] = None;
            }
            self.changed = true;
        }
        Some(out)
    }
    fn slot_limit(&self, slot: usize) -> u32 {
        self.visible().get(slot).map_or(0, |&r| self.max[r])
    }
    fn is_item_valid(&self, slot: usize, stack: &ItemStack) -> bool {
        self.visible().get(slot).is_some_and(|&r| self.may_insert(r, stack))
    }
}
fn ae2_items(
    game: &mut Game<'_>,
    pos: Pos,
    face: Option<super::foundry::Dir>,
    f: &mut dyn FnMut(&mut dyn super::foundry::ItemHandler<ItemStack>),
) -> bool {
    if with_rt(|rt| rt.window.as_ref().is_some_and(|w| w.pos.pos == pos)) == Some(true) {
        // A block whose window is open: an empty handler (the window's slots are mirrors).
        let mut none = EmptyItems;
        f(&mut none);
        return true;
    }
    let Some(p) = ae2_provider_loc(game, pos, face) else { return false };
    let Some(k) = game.ae2_ensure(p) else { return false };
    let face = face.and_then(|d| Dir::from_idx(d.idx()));
    let front = game.ae2_front(p, k);
    let Some(slots) = with_st(game, |s| s.devices.at(p).map(|d| d.slots.clone())).flatten() else { return false };
    let stacks: Vec<Option<ItemStack>> = slots.iter().map(|s| s.as_ref().and_then(|(k, n)| stack_of(k, *n))).collect();
    let max: Vec<u32> = (0..stacks.len())
        .map(|i| match (k, i) {
            (Kind::Inscriber, 0 | 1) | (Kind::Charger, 0) => 1,
            // AppEngInternalInventory(9 + 1, 1): one item per grid slot.
            (Kind::Assembler, 0..9) => 1,
            _ => stacks[i].as_ref().map_or(64, |s| game.ae2_max_stack(&s.item)),
        })
        .collect();
    let items = game.res.items();
    let providers = game.res.providers();
    let tags = |t: &str, i: &str| items.item_tags.get(t).is_some_and(|s| s.contains(i));
    let fuel = |s: &ItemStack| providers.burn_time(items, s, false) > 0;
    let plan = if k == Kind::Assembler { game.ae2_assembler_plan(p) } else { None };
    let grid_ok = |slot: usize, s: &ItemStack| plan.as_ref().is_some_and(|d| d.grid_valid(slot, Some(&key_of(s)), &item_id));
    let mut h = DevItems { kind: k, face, front, slots: stacks, max, tags: &tags, fuel: &fuel, grid_ok: &grid_ok, changed: false };
    if matches!(k, Kind::Interface(_) | Kind::Inscriber | Kind::Charger | Kind::Vibration | Kind::Provider(_) | Kind::Assembler) {
        f(&mut h);
    } else {
        // A block with an open window: an empty handler.
        let mut none = DevItems { kind: Kind::Crank, ..h };
        f(&mut none);
        return true;
    }
    if h.changed {
        let new: Vec<Option<(Key, u32)>> = h.slots.iter().map(|s| s.as_ref().map(|s| (key_of(s), s.count))).collect();
        with_st(game, |s| {
            if let Some(d) = s.devices.at_mut(p) {
                d.slots = new;
            }
        });
        with_rt(|rt| {
            if let Some(t) = rt.tickers.get_mut(&p) {
                t.wake(rt.tick);
            }
        });
    }
    true
}

/// No slots (an AE2 block whose window is open).
struct EmptyItems;
impl super::foundry::ItemHandler<ItemStack> for EmptyItems {
    fn slots(&self) -> usize {
        0
    }
    fn stack_in_slot(&self, _slot: usize) -> Option<ItemStack> {
        None
    }
    fn insert_item(&mut self, _slot: usize, stack: ItemStack, _simulate: bool) -> Option<ItemStack> {
        Some(stack)
    }
    fn extract_item(&mut self, _slot: usize, _amount: u32, _simulate: bool) -> Option<ItemStack> {
        None
    }
    fn slot_limit(&self, _slot: usize) -> u32 {
        0
    }
    fn is_item_valid(&self, _slot: usize, _stack: &ItemStack) -> bool {
        false
    }
}

fn item_tags<'a>(game: &'a Game<'_>) -> impl Fn(&str, &str) -> bool + 'a {
    let items = game.res.items();
    move |t: &str, i: &str| items.item_tags.get(t).is_some_and(|s| s.contains(i))
}
fn slot_item(slots: &[Option<(Key, u32)>], i: usize) -> Option<String> {
    slots.get(i).and_then(|s| s.as_ref()).and_then(|(k, _)| item_of_key(k))
}

impl Game<'_> {
    /// HEAD `tick_block_entities`: everything AE2 does in a tick.
    pub(super) fn ae2_tick(&mut self) {
        if !self.ae2_active() {
            return;
        }
        let key = format!("{}|{}", self.world_root.display(), kit_state::dim_key(self));
        let now = with_rt(|rt| {
            if rt.world_key != key {
                // Another world or dimension: its networks are built anew.
                let search = std::mem::take(&mut rt.last_search);
                *rt = Rt { world_key: key.clone(), last_search: search, ..Rt::default() };
            }
            rt.tick += 1;
            rt.tick
        })
        .unwrap_or(0);
        if now % 20 == 3 {
            self.ae2_migrate();
        }
        self.ae2_bus_sync();
        self.ae2_gone();
        if now % 20 == 1 {
            self.ae2_stale_windows();
        }
        self.ae2_networks();
        self.ae2_idle();
        let records: Vec<(Loc, Kind)> = with_st(self, |s| s.devices.iter().filter_map(|(l, d)| kind_of(&d.block).map(|k| (*l, k))).collect())
            .unwrap_or_default();
        for (l, k) in records {
            let p = l.pos;
            if !kit_blocks::loaded(self, p) || !self.tickets.levels().block_ticking_at(p[0], p[2]) {
                continue;
            }
            match k {
                Kind::Inscriber => self.ae2_inscriber(p),
                Kind::Charger => self.ae2_ticked(l, now, 10, 10, |g, l, t| g.ae2_charger(l.pos, t)),
                Kind::Vibration => self.ae2_ticked(l, now, 10, 40, |g, l, t| g.ae2_vibration(l.pos, t)),
                Kind::ImportBus | Kind::ExportBus | Kind::Interface(_) => {
                    let (min, max) = match k {
                        Kind::ImportBus => (engine::config::TICK_RATES_IMPORT_BUS_MIN, engine::config::TICK_RATES_IMPORT_BUS_MAX),
                        Kind::ExportBus => (engine::config::TICK_RATES_EXPORT_BUS_MIN, engine::config::TICK_RATES_EXPORT_BUS_MAX),
                        _ => (engine::config::TICK_RATES_INTERFACE_MIN, engine::config::TICK_RATES_INTERFACE_MAX),
                    };
                    if self.ae2_device_active(l, k) {
                        self.ae2_ticked(l, now, min as i32, max as i32, move |g, l, _| match k {
                            Kind::ImportBus => g.ae2_import(l),
                            Kind::ExportBus => g.ae2_export(l),
                            _ => g.ae2_interface(l),
                        });
                    }
                }
                Kind::Crank => self.ae2_crank_tick(p),
                Kind::Provider(_) => {
                    if self.ae2_device_active(l, k) {
                        let (min, max) = (engine::config::TICK_RATES_INTERFACE_MIN, engine::config::TICK_RATES_INTERFACE_MAX);
                        self.ae2_ticked(l, now, min as i32, max as i32, |g, l, _| g.ae2_provider_tick(l));
                    }
                }
                Kind::Assembler => self.ae2_assembler_tick(p),
                Kind::LevelEmitter if now % 5 == 0 => self.ae2_emitter_tick(l),
                _ => {}
            }
        }
        self.ae2_cpus_tick();
        self.ae2_notices();
        with_rt(|rt| {
            for n in &mut rt.nets {
                n.power.end_tick();
            }
        });
        self.ae2_visuals(now);
        if now % 20 == 0 {
            self.ae2_store_energy();
        }
        if now % 5 == 0 {
            self.ae2_transforms();
        }
        self.ae2_window_tick();
    }
    /// A window's temporary block entity left behind (only after a crash: AE2 block entities
    /// are saved under the block's id and never loaded back, see save_load.rs): removed. Its
    /// slots only ever mirrored the network, so nothing real is in them.
    fn ae2_stale_windows(&mut self) {
        let open = self.ae2_window().map(|(l, _)| l.pos);
        let records: Vec<Pos> = with_st(self, |s| s.devices.keys().map(|l| l.pos).collect()).unwrap_or_default();
        for p in records {
            let key = (p[0], p[1], p[2]);
            if Some(p) == open || self.ae2_window_at(key) {
                continue;
            }
            if matches!(self.world.block_entities.get(&key), Some(BlockEntity::Chest(_))) {
                self.world.block_entities.remove(&key);
            }
        }
    }
    /// A device on AE2's dynamic tick rate (TickManagerService).
    fn ae2_ticked(&mut self, p: Loc, now: u64, min: i32, max: i32, f: impl FnOnce(&mut Self, Loc, u64) -> engine::TickRate) {
        let due = with_rt(|rt| {
            let t = rt.tickers.entry(p).or_insert_with(|| engine::Ticker::new(min, max, now));
            t.due(now)
        })
        .flatten();
        if let Some(ticks) = due {
            let r = f(self, p, ticks);
            with_rt(|rt| {
                if let Some(t) = rt.tickers.get_mut(&p) {
                    t.done(now, r);
                }
            });
        }
    }
    /// The networks' idle draw: every node's idlePowerUsage, the cells of drives and chests.
    fn ae2_idle(&mut self) {
        let idle: HashMap<Loc, f64> = with_st(self, |s| {
            s.devices
                .iter()
                .filter_map(|(p, d)| {
                    let k = kind_of(&d.block)?;
                    let mut v = k.idle();
                    if matches!(k, Kind::Drive | Kind::Chest) {
                        for (key, _) in d.slots.iter().flatten() {
                            if let Some(t) = item_of_key(key).and_then(|i| engine::cell_type(&i)) {
                                v += t.idle_drain;
                            }
                        }
                    }
                    Some((*p, v))
                })
                .collect()
        })
        .unwrap_or_default();
        with_rt(|rt| {
            for n in &mut rt.nets {
                n.power.idle = n.members.iter().map(|p| idle.get(p).copied().unwrap_or(0.0)).sum();
            }
        });
    }

    // ---- machines ----

    /// InscriberBlockEntity: slots 0 top plate, 1 bottom plate, 2 middle input, 3 output.
    fn ae2_inscriber(&mut self, p: Pos) {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return };
        let tags = item_tags(self);
        let (top, bottom, middle) = (slot_item(&d.slots, 0), slot_item(&d.slots, 1), slot_item(&d.slots, 2));
        let recipe = middle.as_deref().and_then(|m| engine::find_inscriber(top.as_deref(), m, bottom.as_deref(), &tags));
        drop(tags);
        let fits = recipe.is_some_and(|r| match &d.slots[3] {
            None => true,
            Some((k, n)) => item_of_key(k).as_deref() == Some(r.result) && *n < self.ae2_max_stack(r.result),
        });
        let mut st = engine::Inscriber {
            progress: d.num("progress") as i32,
            smash: d.num("smash") != 0,
            final_step: d.num("final") as i32,
        };
        // A changed recipe starts over.
        let rid = recipe.map_or(-1, |r| engine::INSCRIBER_RECIPES.iter().position(|x| x.id == r.id).unwrap_or(0) as i64);
        if rid != d.num("recipe") - 1 && !st.smash {
            st.progress = 0;
        }
        let mut buf = engine::Buffer { stored: d.fl("buffer"), max: engine::config::INSCRIBER_BUFFER };
        let n = self.ae2_net(p);
        let step = with_rt(|rt| match n.and_then(|n| rt.nets.get_mut(n)) {
            Some(net) => engine::inscriber_tick(&mut st, recipe.is_some() && fits, &mut buf, Some(&mut Pool(&mut net.power))),
            None => engine::inscriber_tick(&mut st, recipe.is_some() && fits, &mut buf, None),
        })
        .unwrap_or(engine::InscriberStep::Nothing);
        let mut slots = d.slots.clone();
        if step == engine::InscriberStep::Craft
            && let Some(r) = recipe
            && fits
            && let Some(out) = stack_of(&key_of(&ItemStack::new(r.result, 1)), 1)
        {
            let k = key_of(&out);
            slots[3] = Some(match slots[3].take() {
                Some((ok, n)) if ok == k => (ok, n + 1),
                _ => (k, 1),
            });
            let used: &[usize] = if r.press { &[0, 1, 2] } else { &[2] };
            for &i in used {
                if let Some((_, n)) = slots[i].as_mut() {
                    *n -= 1;
                    if *n == 0 {
                        slots[i] = None;
                    }
                }
            }
            st.progress = 0;
        }
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(p) {
                dd.slots = slots;
                dd.set_num("progress", i64::from(st.progress));
                dd.set_num("smash", i64::from(st.smash));
                dd.set_num("final", i64::from(st.final_step));
                dd.set_num("recipe", rid + 1);
                dd.set_fl("buffer", buf.stored);
            }
        });
    }
    /// ChargerBlockEntity.doWork, every 10 ticks.
    fn ae2_charger(&mut self, p: Pos, _ticks: u64) -> engine::TickRate {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return engine::TickRate::Same };
        let item = slot_item(&d.slots, 0);
        let mut buf = engine::Buffer { stored: d.fl("buffer"), max: engine::config::CHARGER_BUFFER };
        let roll = self.world.random.next_float();
        let n = self.ae2_net(p);
        let out = with_rt(|rt| match n.and_then(|n| rt.nets.get_mut(n)) {
            Some(net) => engine::charger_tick(&mut buf, item.as_deref(), roll, Some(&mut Pool(&mut net.power))),
            None => engine::charger_tick(&mut buf, item.as_deref(), roll, None),
        })
        .flatten();
        let new_slot = out.map(|r| (key_of(&ItemStack::new(r, 1)), 1u32));
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(p) {
                if let Some(ns) = new_slot {
                    dd.slots[0] = Some(ns);
                }
                dd.set_fl("buffer", buf.stored);
            }
        });
        engine::TickRate::Faster
    }
    /// VibrationChamberBlockEntity.tickingRequest.
    fn ae2_vibration(&mut self, p: Pos, ticks: u64) -> engine::TickRate {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return engine::TickRate::Sleep };
        let mut v = engine::Vibration { remaining: d.fl("fuel"), item_ticks: d.fl("fuel_item"), rate: d.f.get("rate").copied().unwrap_or_else(engine::Vibration::initial_rate) };
        let mut slots = d.slots.clone();
        if !v.burning() {
            // eatFuel: one item; a bucket of lava leaves its bucket.
            if let Some((k, n)) = slots[0].clone()
                && let Some(stack) = stack_of(&k, 1)
            {
                let burn = self.res.providers().burn_time(self.res.items(), &stack, false);
                if burn > 0 {
                    v.eat(burn);
                    slots[0] = if n > 1 {
                        Some((k, n - 1))
                    } else {
                        self.res.items().get(&stack.item).and_then(|i| i.crafting_remainder.clone()).map(|r| (key_of(&ItemStack::new(&r, 1)), 1))
                    };
                }
            }
        }
        let rate = if !v.burning() {
            v.out_of_fuel();
            engine::TickRate::Sleep
        } else if let Some(n) = self.ae2_net(p) {
            with_rt(|rt| rt.nets.get_mut(n).map(|net| v.tick(ticks, &mut net.power))).flatten().unwrap_or(engine::TickRate::Same)
        } else {
            engine::TickRate::Same
        };
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(p) {
                dd.slots = slots;
                dd.set_fl("fuel", v.remaining);
                dd.set_fl("fuel_item", v.item_ticks);
                dd.set_fl("rate", v.rate);
            }
        });
        rate
    }
    /// The machine a crank turns: the block it sits on, else a charger/inscriber beside it.
    fn ae2_crank_target(&self, p: Pos) -> Option<Pos> {
        let below = engine::offset(p, Dir::Down);
        if matches!(self.ae2_kind_at(below), Some(Kind::Charger | Kind::Inscriber)) {
            return Some(below);
        }
        Dir::ALL.into_iter().map(|d| engine::offset(p, d)).find(|q| matches!(self.ae2_kind_at(*q), Some(Kind::Charger | Kind::Inscriber)))
    }
    fn ae2_crank_tick(&mut self, p: Pos) {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return };
        let mut c = engine::Crank { rotation: d.num("rot") as i32, charge: d.num("charge") as i32, hits: d.num("hits") as i32 };
        if c.rotation <= 0 {
            return;
        }
        let applied = c.tick();
        let target = if applied { self.ae2_crank_target(p) } else { None };
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(p) {
                dd.set_num("rot", i64::from(c.rotation));
                dd.set_num("charge", i64::from(c.charge));
            }
            if let Some(t) = target
                && let Some(m) = s.devices.at_mut(t)
            {
                // applyTurn: injectExternalPower(AE, 160) into the machine's buffer.
                let max = kind_of(&m.block).map_or(0.0, Kind::holds_power);
                let v = (m.fl("buffer") + engine::config::CRANK_POWER_PER_TURN).min(max);
                m.set_fl("buffer", v);
            }
        });
    }

    // ---- buses and interface (active devices only) ----

    fn ae2_strays(&mut self, p: Pos, stray: &[(Key, u64)]) {
        for (k, n) in stray {
            let mut left = *n;
            while left > 0 {
                let c = left.min(64) as u32;
                if let Some(s) = stack_of(k, c) {
                    self.ae2_drop(p, s);
                }
                left -= u64::from(c);
            }
        }
    }
    // ---- looks: the controller's state, the cables' arms ----

    fn ae2_visuals(&mut self, now: u64) {
        let (nodes, nets): (HashMap<Loc, NodeKind>, Vec<(Vec<Loc>, engine::ControllerState, bool)>) = match with_rt(|rt| {
            (
                rt.topology.iter().copied().collect(),
                rt.nets.iter().map(|n| (n.members.clone(), n.state, n.power.powered)).collect(),
            )
        }) {
            Some(v) => v,
            None => return,
        };
        // Every second, or right after a change (the topology was rebuilt this tick).
        let dirty = with_rt(|rt| std::mem::take(&mut rt.looks_dirty)).unwrap_or(false);
        if now % 20 != 0 && !dirty {
            return;
        }
        for (members, state, powered) in nets {
            let look = match state {
                engine::ControllerState::Conflict => "ae2:controller__conflicted",
                engine::ControllerState::Online if powered => "ae2:controller__online",
                _ => "ae2:controller",
            };
            for l in members {
                // The cables' arms are the cable bus's (foundry draws them from `ae2_join`).
                if nodes.get(&l) == Some(&NodeKind::Controller)
                    && ae2_block_name(self, l.pos).as_deref() != Some(look)
                    && let Some(st) = self.pack.assets.registry.default_state(look)
                {
                    self.ae2_set_state(l.pos, st);
                }
            }
        }
        if dirty {
            // The cables' arms follow the new topology at once.
            let hosts: Vec<Pos> = nodes.keys().filter(|l| l.side.is_none()).map(|l| l.pos).collect();
            for p in hosts {
                if ae2_block_name(self, p).as_deref() == Some(engine::CABLE_BUS) {
                    super::foundry::refresh_parts(self, p);
                }
            }
        }
    }
    /// A look-only state change: clients are told, neighbours are not updated (a neighbour
    /// update would make the mushroom template reset the cables' arms).
    fn ae2_set_state(&mut self, p: Pos, st: horizon_world::registry::BlockStateId) {
        let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
        self.with_level(|b, level, ticks| {
            b.set_block(level, ticks, at, st, level_flags::UPDATE_CLIENTS | level_flags::UPDATE_KNOWN_SHAPE)
        });
        let _ = self.process_level_events();
    }

    // ---- the world: certus growth, transforms in water ----

    /// HEAD `tick_blocks`: BuddingCertusQuartzBlock.randomTick (Horizon has no random tick of
    /// its own for content blocks), the oreberries pattern.
    pub(super) fn ae2_random_ticks(&mut self) {
        if !self.ae2_active() {
            return;
        }
        let speed = self.ui.rules.int("random_tick_speed");
        if speed <= 0 {
            return;
        }
        let reg = &self.pack.assets.registry;
        let budding: Vec<(horizon_world::registry::BlockStateId, &'static str)> =
            engine::BUDDING.iter().filter_map(|b| reg.default_state(b).map(|s| (s, *b))).collect();
        if budding.is_empty() {
            return;
        }
        let levels = self.tickets.levels();
        let keys: Vec<(i32, i32)> = self.world.chunks.keys().copied().filter(|&(x, z)| levels.random_ticking(x, z)).collect();
        let mut hits: Vec<(Pos, &'static str)> = Vec::new();
        for key in keys {
            let Some(chunk) = self.world.chunks.get(&key) else { continue };
            for (s, section) in chunk.sections.iter().enumerate() {
                if !section.blocks.maybe_has(|v| budding.iter().any(|(b, _)| b.0 == v)) {
                    continue;
                }
                let y0 = horizon_client::chunk::MIN_Y + s as i32 * 16;
                for _ in 0..speed {
                    let r = self.world.random.next_int();
                    let (dx, dy, dz) = (r & 15, (r >> 4) & 15, (r >> 8) & 15);
                    let state = chunk.block(dx, y0 + dy, dz);
                    if let Some((_, name)) = budding.iter().find(|(b, _)| *b == state) {
                        hits.push(([key.0 * 16 + dx, y0 + dy, key.1 * 16 + dz], name));
                    }
                }
            }
        }
        for (p, name) in hits {
            let growth = (self.world.random.next_float() * engine::config::BUDDING_GROWTH_CHANCE as f32) as i32;
            if growth != 0 {
                continue;
            }
            let d = Dir::ALL[(self.world.random.next_float() * 6.0) as usize % 6];
            let q = engine::offset(p, d);
            let tstate = kit_blocks::state_at(self, q);
            let tname = kit_blocks::name_of(self, tstate);
            let water = &*tname == "minecraft:water" && kit_blocks::prop(self, tstate, "level").as_deref() == Some("0");
            let free = matches!(&*tname, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") || water;
            let bud = engine::BUDS.iter().position(|b| *b == &*tname).filter(|_| kit_blocks::prop(self, tstate, "facing").as_deref() == Some(d.name())).map(|i| i as u8);
            let decay = (self.world.random.next_float() * engine::config::BUDDING_DECAY_CHANCE as f32) as i32;
            let Some((stage, decayed)) = engine::budding_tick(name, 0, free, bud, decay) else { continue };
            let was_water = water || kit_blocks::prop(self, tstate, "waterlogged").as_deref() == Some("true");
            if let Some(st) = self.pack.assets.registry.default_state(engine::BUDS[stage as usize])
                && let Some(st) = self.with_props(st, &[("facing", d.name()), ("waterlogged", if was_water { "true" } else { "false" })])
            {
                let _ = self.level_set(q, st);
            }
            if let Some(nb) = decayed
                && let Some(st) = self.pack.assets.registry.default_state(nb)
            {
                let _ = self.level_set(p, st);
            }
        }
    }
    /// TransformLogic (in-water recipes): items lying in the same water block.
    fn ae2_transforms(&mut self) {
        let mut groups: BTreeMap<Pos, Vec<usize>> = BTreeMap::new();
        for (i, e) in self.dropped.iter().enumerate() {
            if !e.stack.item.starts_with("ae2:") && !matches!(&*e.stack.item, "minecraft:redstone" | "minecraft:quartz") {
                continue;
            }
            let p = [e.position.x.floor() as i32, e.position.y.floor() as i32, e.position.z.floor() as i32];
            let st = kit_blocks::state_at(self, p);
            let wet = &*kit_blocks::name_of(self, st) == "minecraft:water" || kit_blocks::prop(self, st, "waterlogged").as_deref() == Some("true");
            if wet {
                groups.entry(p).or_default().push(i);
            }
        }
        let mut spawn = Vec::new();
        for (p, idx) in groups {
            for _ in 0..16 {
                let items: Vec<(String, u32, usize)> = idx.iter().map(|&i| (self.dropped[i].stack.item.to_string(), self.dropped[i].stack.count, i)).collect();
                let Some((used, result, n)) = engine::find_transform(&items) else { break };
                for i in used {
                    self.dropped[i].stack.count -= 1;
                }
                spawn.push((p, ItemStack::new(result, n)));
            }
        }
        if spawn.is_empty() {
            return;
        }
        self.dropped.retain(|e| e.stack.count > 0);
        for (p, s) in spawn {
            self.spawn_item(s, center(p), [0.0, 0.0, 0.0], 10);
        }
    }

    // ---- hooks: use, place, save, world ----

    /// HEAD `use_golem_statue` (a block's use in `use_hand`): AE2's onActivated.
    pub(super) fn ae2_use(&mut self, pos: Pos) -> bool {
        // A cable bus that emits redstone stands as its signal twin, a trapped-chest-class block:
        // a use nothing of AE2 took ends here (the template would try to open a chest).
        self.ae2_use_at(pos) || (self.ae2_active() && kit_blocks::name_at(self, pos).as_deref() == Some(CABLE_BUS_SIGNAL))
    }
    fn ae2_use_at(&mut self, pos: Pos) -> bool {
        if !self.ae2_active() {
            return false;
        }
        // A cable bus: the part under the crosshair (CableBusContainer.activate -> selectPart).
        let at = if ae2_block_name(self, pos).as_deref() == Some(engine::CABLE_BUS) {
            match super::foundry::targeted_part(self) {
                Some((p, slot)) if p == pos => Loc { pos, side: slot.and_then(|d| Dir::from_idx(d.idx())) },
                _ => return false,
            }
        } else {
            Loc::block(pos)
        };
        let Some(k) = self.ae2_ensure(at) else { return false };
        with_rt(|rt| rt.topology.clear());
        match k {
            Kind::Crank => {
                let Some(target) = self.ae2_crank_target(pos) else { return true };
                let full = with_st(self, |s| s.devices.at(target).map(|d| d.fl("buffer") >= kind_of(&d.block).map_or(0.0, Kind::holds_power)))
                    .flatten()
                    .unwrap_or(true);
                let Some(mut c) = with_st(self, |s| s.devices.at(pos).map(|d| engine::Crank { rotation: d.num("rot") as i32, charge: d.num("charge") as i32, hits: d.num("hits") as i32 })).flatten() else {
                    return true;
                };
                let r = c.turn(!full);
                with_st(self, |s| {
                    if let Some(d) = s.devices.at_mut(pos) {
                        d.set_num("rot", i64::from(c.rotation));
                        d.set_num("hits", i64::from(c.hits));
                    }
                });
                if r == engine::CrankUse::Break {
                    let _ = self.destroy_block_dropping(pos);
                }
                true
            }
            Kind::Charger => {
                // ChargerBlock.onActivated: put a chargeable item in, or take what is there.
                let slot = with_st(self, |s| s.devices.at(pos).and_then(|d| d.slots[0].clone())).flatten();
                if let Some((k, n)) = slot {
                    if let Some(s) = stack_of(&k, n) {
                        with_st(self, |st| {
                            if let Some(d) = st.devices.at_mut(pos) {
                                d.slots[0] = None;
                            }
                        });
                        self.ae2_give(s);
                    }
                } else if let Some(held) = self.held().cloned()
                    && engine::charger_result(&held.item).is_some()
                {
                    let one = ItemStack { count: 1, ..held };
                    with_st(self, |s| {
                        if let Some(d) = s.devices.at_mut(pos) {
                            d.slots[0] = Some((key_of(&one), 1));
                        }
                    });
                    if !self.creative {
                        self.consume_held();
                    }
                }
                true
            }
            Kind::Cable(..) | Kind::Controller | Kind::Acceptor(_) => false,
            Kind::CraftUnit(_) => {
                // AbstractCraftingUnitBlock.onActivated: a formed CPU opens its status.
                let core = with_rt(|rt| rt.clusters.iter().find(|c| c.members.contains(&pos)).map(|c| c.core())).flatten();
                match core {
                    Some(core) => {
                        self.ae2_open(at, k);
                        with_rt(|rt| {
                            if let Some(w) = rt.window.as_mut() {
                                w.cpu = Some(core);
                            }
                        });
                        self.ae2_window_refresh();
                        true
                    }
                    None => false,
                }
            }
            _ => {
                self.ae2_open(at, k);
                true
            }
        }
    }
    /// HEAD `trigger_block("minecraft:placed_block")`: a placed AE2 block joins at once.
    pub(super) fn ae2_placed(&mut self, trigger: &str, pos: Pos) {
        if trigger != "minecraft:placed_block" || !self.ae2_active() {
            return;
        }
        if self.ae2_ensure(pos).is_some() {
            with_rt(|rt| rt.topology.clear());
        }
    }
    /// HEAD `save_game_metadata`: the window's temporary slots are emptied, energy goes back to
    /// the records, the records are written.
    pub(super) fn ae2_save(&mut self) {
        if !self.ae2_active() {
            return;
        }
        self.ae2_store_energy();
        if let Some(p) = with_rt(|rt| rt.window.as_ref().map(|w| w.pos.pos)).flatten()
            && let Some(BlockEntity::Chest(slots)) = self.world.block_entities.get_mut(&(p[0], p[1], p[2]))
        {
            slots.iter_mut().for_each(|s| *s = None);
        }
        let game: &Game<'_> = self;
        ST.with(|s| {
            if let Ok(mut s) = s.try_borrow_mut() {
                s.save(game, |st| st.save());
            }
        });
    }
    /// HEAD `load_world_state`: another world opens.
    pub(super) fn ae2_world_opened() {
        Self::ae2_register();
        ST.with(|s| {
            if let Ok(mut s) = s.try_borrow_mut() {
                s.clear();
            }
        });
        with_rt(|rt| {
            let search = std::mem::take(&mut rt.last_search);
            *rt = Rt { last_search: search, ..Rt::default() };
        });
    }
    /// The providers, the cable bus and its parts are registered once (foundry keeps one entry per id).
    pub(super) fn ae2_register() {
        super::foundry::register_energy("ae2", ae2_energy_claims, ae2_energy);
        super::foundry::register_items("ae2", ae2_item_claims, ae2_items);
        ae2_register_parts();
        ae2_register_menu();
    }
}

// ---------------------------------------------------------------------------------------------
// The cable bus: AE2's cables and parts as foundry compound blocks.

/// `ae2:cable_bus` (CableBusBlock): mining it breaks every part (`getDrops` = every part's item),
/// a wrench (sneak + use) takes the targeted part into the inventory (`disassembleWithWrench`);
/// cable bus sounds are glass (`glassProps`).
fn ae2_register_parts() {
    use super::foundry::{HostSpec, PartShape, PartSpec};
    super::foundry::register_host(HostSpec {
        block: engine::CABLE_BUS,
        break_whole: engine::config::BREAK_WHOLE_CABLE_BUS,
        wrench_tag: Some("c:tools/wrench"),
        wrench_items: &["ae2:certus_quartz_wrench", "ae2:nether_quartz_wrench"],
        place_sound: "minecraft:block.glass.place",
        signal_block: Some(CABLE_BUS_SIGNAL),
    });
    for color in COLORS {
        for suffix in ["_glass_cable", "_covered_cable", "_smart_cable", "_covered_dense_cable", "_smart_dense_cable"] {
            let id = format!("ae2:{color}{suffix}");
            let Some((_, _, dense)) = engine::cable_of(&id) else { continue };
            let (min, max, collide) = engine::cable_size(suffix);
            // The stage-1 cable block draws the cable: its six booleans are the arms.
            let spec = PartSpec::center(&id, engine::CABLE_BUS, &id, PartShape::Cable { min, max, collide, attach_arms: !dense })
                .group(if dense { 2 } else { 1 });
            super::foundry::register_part(spec, Some(ae2_join));
        }
    }
    for def in engine::PARTS {
        // The stage-1 part blocks (barrel `facing` towards the viewer) draw the parts; the anchor
        // has its own item-less model block.
        let spec = PartSpec::face(def.id, engine::CABLE_BUS, def.model, def.boxes)
            .cable_length(def.cable_length)
            .group(def.groups)
            .facing_opposite();
        super::foundry::register_part(spec, None);
    }
}
/// Whether a cable bus's cable links towards `side` (GridNode exposure + `connects`): another
/// cable bus's cable of a fitting colour, or an AE2 block of the network (drive, controller...).
fn ae2_join(game: &Game<'_>, pos: Pos, part: &super::foundry::Part, side: super::foundry::Dir, neighbour: Option<&super::foundry::Part>) -> bool {
    let Some(d) = Dir::from_idx(side.idx()) else { return false };
    let Some(Kind::Cable(c, dense)) = kind_of(&part.id) else { return false };
    let me = NodeSpec { pos, kind: NodeKind::Cable { color: c, dense }, idle: 0.0 };
    let q = engine::offset(pos, d);
    let other = match neighbour {
        Some(n) => match kind_of(&n.id) {
            Some(Kind::Cable(c2, dense2)) => NodeKind::Cable { color: c2, dense: dense2 },
            _ => return false,
        },
        None => match game.ae2_kind_at(q) {
            Some(Kind::Controller) => NodeKind::Controller,
            // A stage-1 part block until it is migrated: its front does not connect.
            Some(k) if k.is_node() && !matches!(k, Kind::Cable(..)) => {
                NodeKind::Device { channel: k.needs_channel(), front: if k.part() { game.ae2_front(q, k) } else { None } }
            }
            Some(Kind::Cable(c2, dense2)) => NodeKind::Cable { color: c2, dense: dense2 },
            _ => return false,
        },
    };
    engine::connects(&me, &NodeSpec { pos: q, kind: other, idle: 0.0 }, d)
}

// ---------------------------------------------------------------------------------------------
// Stage 2: autocrafting. Patterns are items with their encoding in custom data (`ae2:pattern`),
// a cell's partition and cards in `ae2:cfg`; a crafting CPU's state lives in the record of its
// lowest corner block; a network's patterns (its "book") are rebuilt every second or when a
// provider's patterns change.

/// A custom-data text of an item.
fn custom_text(s: &ItemStack, name: &str) -> Option<String> {
    match custom(s)?.get(name)? {
        Tag::String(j) => Some(j.to_string_lossy()),
        _ => None,
    }
}
fn set_custom_text(s: &mut ItemStack, name: &str, text: Option<&str>) {
    let mut map = match custom(s) {
        Some(Tag::Compound(m)) => m.clone(),
        _ => Default::default(),
    };
    map.remove(&JavaString::from(name));
    if let Some(t) = text.filter(|t| !t.is_empty()) {
        map.insert(JavaString::from(name), Tag::String(JavaString::from(t)));
    }
    let value = (!map.is_empty()).then_some(Tag::Compound(map));
    if value.is_none() && s.data().is_none() {
        return;
    }
    s.data_mut().set_other("minecraft:custom_data", value);
}
/// An encoded pattern item.
fn pattern_stack(e: &cr::Encoded) -> ItemStack {
    let mut s = ItemStack::new(e.kind.item(), 1);
    set_custom_text(&mut s, "ae2:pattern", Some(&e.encode()));
    s
}
fn pattern_of(s: &ItemStack) -> Option<cr::Encoded> {
    let kind = cr::PatternKind::of_item(&s.item)?;
    let e = cr::Encoded::decode(&custom_text(s, "ae2:pattern")?)?;
    (e.kind == kind).then_some(e)
}
/// A storage cell's partition and cards (CellWorkbench's config, kept in the cell item).
fn cell_config(s: &ItemStack) -> cr::CellConfig {
    cr::CellConfig::decode(&custom_text(s, "ae2:cfg").unwrap_or_default())
}
/// The upgrade cards in a device's record.
fn device_cards(d: &Device, k: Kind) -> Vec<String> {
    let Some((_, range)) = k.upgrades() else { return Vec::new() };
    range.filter_map(|i| slot_item(&d.slots, i)).collect()
}
fn plain_key(item: &str) -> Key {
    key_of(&ItemStack::new(item, 1))
}

/// The game as a CPU's pattern providers (CraftingService.getProviders).
impl cr::Machines for Live<'_, '_> {
    fn providers(&mut self, definition: &Key) -> usize {
        self.cur_def = definition.clone();
        self.prov.get(definition).map_or(0, Vec::len)
    }
    fn busy(&mut self, provider: usize) -> bool {
        let Some(l) = self.prov.get(&self.cur_def).and_then(|v| v.get(provider)).copied() else { return true };
        with_st(self.game, |s| s.devices.at(l).is_some_and(|d| !d.list("!send").is_empty())).unwrap_or(true)
    }
    fn push(&mut self, provider: usize, details: &cr::Details, table: &[Vec<(Key, u64)>]) -> bool {
        let Some(l) = self.prov.get(&details.definition).and_then(|v| v.get(provider)).copied() else { return false };
        self.game.ae2_provider_push(l, details, table)
    }
}
/// The inventories around a pattern provider (PatternProviderTarget through foundry).
struct ProvTargets<'a, 'g> {
    game: &'a mut Game<'g>,
    pos: Pos,
    sides: Vec<Dir>,
}
impl cr::Targets for ProvTargets<'_, '_> {
    fn sides(&mut self) -> Vec<Dir> {
        self.sides.clone()
    }
    fn insert(&mut self, side: Dir, key: &Key, amount: u64, simulate: bool) -> u64 {
        bus_inv_op(self.game, engine::offset(self.pos, side), side.opposite(), |inv| engine::inv_insert(inv, key, amount, simulate)).unwrap_or(0)
    }
    fn contains_any(&mut self, side: Dir, keys: &std::collections::BTreeSet<Key>) -> bool {
        bus_inv_op(self.game, engine::offset(self.pos, side), side.opposite(), |inv| {
            (0..engine::Inv::slots(inv)).any(|s| engine::Inv::get(inv, s).is_some_and(|(k, _)| keys.contains(&k)))
        })
        .unwrap_or(false)
    }
}

impl Game<'_> {
    // ---- patterns ----

    /// The decoded pattern of an item key (cached until another world opens).
    fn ae2_details(&self, key: &Key) -> Option<Rc<cr::Details>> {
        if let Some(Some(v)) = with_rt(|rt| rt.details.get(key).cloned()) {
            return v;
        }
        let d = self.ae2_decode(key).map(Rc::new);
        with_rt(|rt| {
            rt.details.insert(key.clone(), d.clone());
        });
        d
    }
    /// PatternDetailsHelper.decodePattern: the recipe must still match the encoded inputs.
    fn ae2_decode(&self, key: &Key) -> Option<cr::Details> {
        let s = stack_of(key, 1)?;
        let e = pattern_of(&s)?;
        let stack = |k: &Option<(Key, u64)>| k.as_ref().and_then(|(k, _)| stack_of(k, 1));
        match e.kind {
            cr::PatternKind::Processing => cr::processing_details(key, &e),
            cr::PatternKind::Crafting => {
                let grid: Vec<Option<ItemStack>> = (0..9).map(|i| e.inputs.get(i).and_then(|x| stack(x))).collect();
                let recipes = self.res.recipes();
                if !recipes.recipe_matches(&e.recipe, &grid, 3) {
                    return None;
                }
                let (_, out) = recipes.find_with(&grid, 3, &|_| None)?;
                let alts = self.ae2_crafting_alts(&e.recipe, &e.inputs);
                let rem: Vec<Option<Key>> = recipes.remainders(&e.recipe, &grid, 3).iter().map(|r| r.as_ref().map(key_of)).collect();
                cr::recipe_details(key, &e, (key_of(&out), u64::from(out.count)), &alts, &rem)
            }
            cr::PatternKind::Smithing => {
                let [t, b, a] = [0, 1, 2].map(|i| e.inputs.get(i).and_then(|x| stack(x)));
                let (t, b, a) = (t?, b?, a?);
                let (id, ings) = self.ae2_smithing_recipe(&t, &b, &a)?;
                if id != e.recipe {
                    return None;
                }
                let data = self.res.stations();
                let out = data.recipes.smithing_result(&data.enchantments, Some(&t), Some(&b), Some(&a))?;
                let alts: Vec<Vec<(Key, String)>> = ings.iter().map(|l| l.iter().map(|i| (plain_key(i), i.clone())).collect()).collect();
                cr::recipe_details(key, &e, (key_of(&out), u64::from(out.count)), &alts, &[])
            }
            cr::PatternKind::Stonecutting => {
                let input = e.inputs.first().and_then(|x| stack(x))?;
                let data = self.res.stations();
                let r = data.recipes.stonecutter(&input).into_iter().find(|r| r.id == e.recipe)?;
                let alts = vec![r.input.1.iter().map(|i| (plain_key(i), i.clone())).collect::<Vec<_>>()];
                cr::recipe_details(key, &e, (key_of(&r.result), u64::from(r.result.count)), &alts, &[])
            }
        }
    }
    /// AECraftingPattern.getRecipeIngredient for every used grid slot: the ingredient's items.
    fn ae2_crafting_alts(&self, recipe: &str, inputs: &[Option<(Key, u64)>]) -> Vec<Vec<(Key, String)>> {
        let mut out = vec![Vec::new(); 9];
        let Some(pl) = self.res.recipes().placement(recipe) else { return out };
        let filled = |i: usize| inputs.get(i).is_some_and(Option::is_some);
        let items = |ing: i32| -> Vec<(Key, String)> {
            usize::try_from(ing).ok().and_then(|i| pl.ingredients.get(i)).map(|g| g.1.iter().map(|it| (plain_key(it), it.clone())).collect()).unwrap_or_default()
        };
        match pl.shaped {
            Some((w, _)) => {
                let mut top = 0;
                if !filled(0) && !filled(1) && !filled(2) {
                    top += 1;
                    if !filled(3) && !filled(4) && !filled(5) {
                        top += 1;
                    }
                }
                let mut left = 0;
                if !filled(0) && !filled(3) && !filled(6) {
                    left += 1;
                    if !filled(1) && !filled(4) && !filled(7) {
                        left += 1;
                    }
                }
                for (slot, o) in out.iter_mut().enumerate() {
                    if !filled(slot) {
                        continue;
                    }
                    let (x, y) = ((slot % 3) as i32 - left, (slot / 3) as i32 - top);
                    if x < 0 || y < 0 {
                        continue;
                    }
                    if let Some(&ing) = pl.slots.get((y * w as i32 + x) as usize) {
                        *o = items(ing);
                    }
                }
            }
            None => {
                let mut n = 0;
                for (slot, o) in out.iter_mut().enumerate() {
                    if filled(slot) {
                        if let Some(&ing) = pl.slots.get(n) {
                            *o = items(ing);
                        }
                        n += 1;
                    }
                }
            }
        }
        out
    }
    /// The smithing recipe for three inputs: its id and the items of its three ingredients.
    fn ae2_smithing_recipe(&self, t: &ItemStack, b: &ItemStack, a: &ItemStack) -> Option<(String, [Vec<String>; 3])> {
        use horizon_client::stations::Smithing;
        for r in &self.res.stations().recipes.smithing {
            let (id, x, y, z) = match r {
                Smithing::Transform { id, template, base, addition, .. } | Smithing::Trim { id, template, base, addition, .. } => (id, template, base, addition),
            };
            if x.test(t) && y.test(b) && z.test(a) {
                return Some((id.clone(), [x.1.clone(), y.1.clone(), z.1.clone()]));
            }
        }
        None
    }

    // ---- a network's crafting service ----

    /// The network's patterns (providers by priority, then place), the level emitters that can
    /// emit, and what its CPUs wait for. Rebuilt every second.
    fn ae2_netbook(&mut self, n: usize) -> Rc<NetBook> {
        let now = with_rt(|rt| rt.tick).unwrap_or(0);
        if let Some(Some(b)) = with_rt(|rt| rt.books.get(&n).filter(|b| now.saturating_sub(b.tick) < 20).cloned()) {
            return b;
        }
        let members = with_rt(|rt| rt.nets.get(n).map(|x| x.members.clone())).flatten().unwrap_or_default();
        let mut provs: Vec<(i64, Loc, Vec<Key>)> = Vec::new();
        let mut emitable = std::collections::BTreeSet::new();
        let mut requesting = std::collections::BTreeSet::new();
        let cores: Vec<Pos> = with_rt(|rt| rt.clusters.iter().map(|c| c.core()).collect()).unwrap_or_default();
        for m in members {
            let Some(d) = with_st(self, |s| s.devices.at(m).cloned()).flatten() else { continue };
            match kind_of(&d.block) {
                Some(k @ Kind::Provider(_)) if self.ae2_device_active(m, k) => {
                    let keys = d.slots.iter().take(cr::PROVIDER_PATTERNS).flatten().map(|(k, _)| k.clone()).collect();
                    provs.push((d.num("priority"), m, keys));
                }
                Some(Kind::LevelEmitter) if self.ae2_device_active(m, Kind::LevelEmitter) => {
                    let cards = device_cards(&d, Kind::LevelEmitter);
                    if cards.iter().any(|c| c == cr::CRAFTING_CARD)
                        && d.num("craft_redstone") != 0
                        && let Some(Some((k, _))) = d.config.first()
                    {
                        emitable.insert(k.clone());
                    }
                }
                Some(Kind::CraftUnit(_)) if m.side.is_none() && cores.contains(&m.pos) => {
                    requesting.extend(cr::Cpu::load_from(&d).requesting());
                }
                _ => {}
            }
        }
        provs.sort_by(|a, b| b.0.cmp(&a.0));
        let mut details = Vec::new();
        let mut providers: HashMap<Key, Vec<Loc>> = HashMap::new();
        for (_, l, keys) in provs {
            for k in keys {
                if let Some(d) = self.ae2_details(&k) {
                    let v = providers.entry(k.clone()).or_default();
                    if !v.contains(&l) {
                        v.push(l);
                    }
                    details.push((*d).clone());
                }
            }
        }
        let nb = Rc::new(NetBook { tick: now, book: Rc::new(cr::Book::new(details, emitable)), providers: Rc::new(providers), requesting });
        with_rt(|rt| {
            rt.books.insert(n, nb.clone());
        });
        nb
    }
    fn ae2_books_dirty(&self) {
        with_rt(|rt| rt.books.clear());
    }
    /// `ae2_with_network` without one CPU's interception (the CPU storing its own items).
    fn ae2_with_network_ex<R>(&mut self, at: impl Into<Loc>, exclude: Option<Pos>, f: impl FnOnce(&mut Live<'_, '_>, &mut engine::GridPower) -> R) -> Option<R> {
        let l = at.into();
        let n = self.ae2_net(l)?;
        let mut power = with_rt(|rt| rt.nets.get(n).map(|x| x.power.clone())).flatten()?;
        let mut mounts = self.ae2_mounts(l);
        mounts.retain(|m| !matches!(m, MountRef::Cpu { core } if Some(*core) == exclude));
        let r = {
            let mut live = Live::new(self, mounts);
            f(&mut live, &mut power)
        };
        let stored = power.stored;
        with_rt(|rt| {
            if let Some(x) = rt.nets.get_mut(n) {
                x.power.stored = stored;
            }
        });
        Some(r)
    }
    /// The network's items as the crafting calculation sees them.
    fn ae2_net_items(&mut self, at: Loc) -> BTreeMap<Key, u64> {
        self.ae2_with_network(at, |live, _| engine::net_list(live)).unwrap_or_default()
    }
    /// CraftingCalculation for `amount` of `key` on the network of `at`.
    fn ae2_plan(&mut self, at: Loc, key: &Key, amount: u64, strategy: cr::Strategy) -> Option<cr::Plan> {
        let n = self.ae2_net(at)?;
        let nb = self.ae2_netbook(n);
        let items = self.ae2_net_items(at);
        Some(cr::plan(&nb.book, &items, key, amount, strategy, &item_id))
    }

    // ---- crafting CPUs ----

    fn ae2_cpu(&self, core: Pos) -> cr::Cpu {
        with_st(self, |s| s.devices.at(core).map(cr::Cpu::load_from)).flatten().unwrap_or_default()
    }
    fn ae2_cpu_save(&self, core: Pos, cpu: &cr::Cpu) {
        let max = with_rt(|rt| rt.clusters.iter().find(|c| c.core() == core).map(|c| c.max)).flatten();
        with_st(self, |s| {
            if let Some(d) = s.devices.at_mut(core) {
                cpu.save_to(d);
                let m = max.unwrap_or(core);
                d.set_num("cpu_x2", i64::from(m[0]));
                d.set_num("cpu_y2", i64::from(m[1]));
                d.set_num("cpu_z2", i64::from(m[2]));
            }
        });
    }
    /// The CPUs of a network (CraftingService.getCpus).
    fn ae2_cpus_of(&self, n: usize) -> Vec<(Pos, cr::CpuInfo)> {
        let clusters = with_rt(|rt| rt.clusters.clone()).unwrap_or_default();
        let mut out = Vec::new();
        for c in clusters {
            let core = c.core();
            if self.ae2_net(core) != Some(n) {
                continue;
            }
            let active = self.ae2_kind_at(core).is_some_and(|k| self.ae2_device_active(core, k));
            let busy = with_st(self, |s| s.devices.at(core).is_some_and(|d| d.num("cpu_job") != 0)).unwrap_or(false);
            out.push((core, cr::CpuInfo { active, busy, storage: c.storage, coprocessors: c.coprocessors }));
        }
        out
    }
    /// A CPU whose cluster changed or broke (CraftingBlockEntity.breakCluster): the job is
    /// cancelled and its items drop (only when the old cluster's chunks are loaded: an unloaded
    /// part is not a break).
    fn ae2_cpu_validate(&mut self) {
        let clusters = with_rt(|rt| rt.clusters.clone()).unwrap_or_default();
        let stored: Vec<(Pos, Pos)> = with_st(self, |s| {
            s.devices
                .iter()
                .filter(|(l, d)| l.side.is_none() && (d.num("cpu_job") != 0 || !d.list("!cpu").is_empty()))
                .map(|(l, d)| (l.pos, [d.num("cpu_x2") as i32, d.num("cpu_y2") as i32, d.num("cpu_z2") as i32]))
                .collect()
        })
        .unwrap_or_default();
        for (p, max) in stored {
            if clusters.iter().any(|c| c.core() == p && c.max == max) {
                continue;
            }
            let corners = [[p[0], p[1], p[2]], [max[0], p[1], p[2]], [p[0], p[1], max[2]], [max[0], p[1], max[2]]];
            if !corners.iter().all(|c| kit_blocks::loaded(self, *c)) {
                continue;
            }
            let mut cpu = self.ae2_cpu(p);
            cpu.cancel();
            let items = std::mem::take(&mut cpu.inventory);
            self.ae2_cpu_save(p, &cpu);
            with_st(self, |s| {
                for (k, n) in items {
                    let mut left = n;
                    while left > 0 {
                        let c = left.min(u64::from(u32::MAX)) as u32;
                        s.drops.push((p, k.clone(), c));
                        left -= u64::from(c);
                    }
                }
            });
        }
    }
    /// tickCraftingLogic of every active CPU.
    fn ae2_cpus_tick(&mut self) {
        let clusters = with_rt(|rt| rt.clusters.clone()).unwrap_or_default();
        for c in clusters {
            let core = c.core();
            if !kit_blocks::loaded(self, core) {
                continue;
            }
            let Some(k) = self.ae2_kind_at(core) else { continue };
            if !self.ae2_device_active(core, k) {
                continue;
            }
            let mut cpu = self.ae2_cpu(core);
            if cpu.job.is_none() && cpu.inventory.is_empty() {
                continue;
            }
            let Some(n) = self.ae2_net(core) else { continue };
            if cpu.job.is_none() {
                self.ae2_with_network_ex(core, Some(core), |live, _| cpu.idle(live));
            } else {
                let nb = self.ae2_netbook(n);
                let (book, prov) = (nb.book.clone(), nb.providers.clone());
                self.ae2_with_network_ex(core, Some(core), |live, power| {
                    live.prov = prov;
                    cpu.run(c.coprocessors, &book, live, &mut Pool(power), &item_id);
                });
            }
            self.ae2_cpu_save(core, &cpu);
        }
    }
    /// The CPU interceptor of network insertions: what the job waits for; its final output
    /// goes to the machine that asked (an interface, an export bus) or on into storage.
    fn ae2_cpu_accept(&mut self, core: Pos, key: &Key, amount: u64, simulate: bool) -> u64 {
        let mut cpu = self.ae2_cpu(core);
        let Some((id, req)) = cpu.job.as_ref().map(|j| (j.id, j.requester)) else { return 0 };
        let a = cpu.insert(key, amount, simulate);
        if !simulate && (a.stored > 0 || a.output > 0) {
            self.ae2_cpu_save(core, &cpu);
        }
        let mut took = a.stored;
        if a.output > 0
            && let Some(r) = req
        {
            took += self.ae2_requester_insert(r, id, key, a.output, simulate);
        }
        took
    }
    /// ICraftingRequester.insertCraftedItems.
    fn ae2_requester_insert(&mut self, req: (Pos, Option<Dir>), job: i64, key: &Key, amount: u64, simulate: bool) -> u64 {
        let l = Loc { pos: req.0, side: req.1 };
        match self.ae2_kind_at(l) {
            Some(Kind::Interface(_)) => {
                let item = item_id(key);
                let max = u64::from(self.ae2_max_stack(&item));
                with_st(self, |s| {
                    let d = s.devices.at_mut(l)?;
                    let slot = (0..engine::config::INTERFACE_SLOTS).find(|i| d.num(&format!("lj{i}")) == job)?;
                    let cur = d.slots.get_mut(slot)?;
                    let room = match cur {
                        None => max,
                        Some((k, c)) if k == key => max.saturating_sub(u64::from(*c)),
                        _ => 0,
                    };
                    let n = room.min(amount);
                    if !simulate && n > 0 {
                        match cur {
                            Some((_, c)) => *c += n as u32,
                            None => *cur = Some((key.clone(), n as u32)),
                        }
                    }
                    Some(n)
                })
                .flatten()
                .unwrap_or(0)
            }
            Some(k @ Kind::ExportBus) if self.ae2_device_active(l, k) => {
                let Some(front) = self.ae2_front(l, k) else { return 0 };
                bus_inv_op(self, engine::offset(l.pos, front), front.opposite(), |inv| engine::inv_insert(inv, key, amount, simulate)).unwrap_or(0)
            }
            _ => 0,
        }
    }
    fn ae2_next_job(&self) -> i64 {
        let base = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_millis() as i64);
        with_rt(|rt| {
            rt.next_job = rt.next_job.max(base * 16) + 1;
            rt.next_job
        })
        .unwrap_or(base)
    }
    /// CraftingService.submitJob: the chosen CPU or the best one (players: fastest first).
    fn ae2_submit(&mut self, at: Loc, plan: &cr::Plan, chosen: Option<Pos>, requester: Option<(Pos, Option<Dir>)>, player: bool) -> Result<(Pos, i64), cr::SubmitError> {
        if plan.simulation {
            return Err(cr::SubmitError::IncompletePlan);
        }
        let n = self.ae2_net(at).ok_or(cr::SubmitError::NoCpuFound)?;
        let cpus = self.ae2_cpus_of(n);
        let idx = match chosen {
            Some(core) => cpus.iter().position(|(c, _)| *c == core).ok_or(cr::SubmitError::CpuOffline)?,
            None => cr::select_cpu(&cpus.iter().map(|(_, i)| i.clone()).collect::<Vec<_>>(), plan.bytes, player)?,
        };
        let (core, info) = cpus[idx].clone();
        let mut cpu = self.ae2_cpu(core);
        let id = self.ae2_next_job();
        let r = self
            .ae2_with_network_ex(core, Some(core), |live, _| cpu.submit(plan, info.storage, info.active, live, id, requester, player))
            .unwrap_or(Err(cr::SubmitError::CpuOffline));
        if r.is_ok() {
            let book = self.ae2_netbook(n).book.clone();
            cpu.set_total(&book);
            self.ae2_books_dirty();
        }
        self.ae2_cpu_save(core, &cpu);
        r.map(|_| (core, id))
    }
    fn ae2_job_alive(&self, id: i64) -> bool {
        let cores: Vec<Pos> = with_rt(|rt| rt.clusters.iter().map(|c| c.core()).collect()).unwrap_or_default();
        cores.into_iter().any(|c| with_st(self, |s| s.devices.at(c).is_some_and(|d| d.num("cpu_job") == id)) == Some(true))
    }
    /// MultiCraftingTracker: the slot waits for a job of its own.
    fn ae2_slot_busy(&self, l: Loc, slot: usize) -> bool {
        let id = with_st(self, |s| s.devices.at(l).map(|d| d.num(&format!("lj{slot}")))).flatten().unwrap_or(0);
        if id == 0 {
            return false;
        }
        if self.ae2_job_alive(id) {
            return true;
        }
        with_st(self, |s| {
            if let Some(d) = s.devices.at_mut(l) {
                d.set_num(&format!("lj{slot}"), 0);
            }
        });
        false
    }
    /// MultiCraftingTracker.handleCrafting: a CRAFT_LESS plan submitted with this machine as
    /// the requester (the calculation runs at once here; AE2 runs it on a thread).
    fn ae2_machine_craft(&mut self, l: Loc, slot: usize, key: &Key, amount: u64) -> bool {
        if amount == 0 || self.ae2_slot_busy(l, slot) {
            return false;
        }
        let Some(plan) = self.ae2_plan(l, key, amount, cr::Strategy::CraftLess) else { return false };
        if plan.simulation {
            return false;
        }
        match self.ae2_submit(l, &plan, None, Some((l.pos, l.side)), false) {
            Ok((_, id)) => {
                with_st(self, |s| {
                    if let Some(d) = s.devices.at_mut(l) {
                        d.set_num(&format!("lj{slot}"), id);
                    }
                });
                true
            }
            Err(_) => false,
        }
    }
    fn ae2_notices(&mut self) {
        let notes = with_rt(|rt| std::mem::take(&mut rt.notices)).unwrap_or_default();
        if let Some(last) = notes.into_iter().last() {
            kit_notice::action_bar(self, last);
        }
    }

    // ---- pattern providers ----

    /// getActiveSides: the block pushes to every side, a part through its own face; never into
    /// another provider or an interface of the same network; only where an inventory is.
    fn ae2_provider_sides(&self, p: Loc) -> Vec<Dir> {
        let sides: Vec<Dir> = match p.side {
            Some(s) => vec![s],
            None => Dir::ALL.to_vec(),
        };
        let net = self.ae2_net(p);
        sides
            .into_iter()
            .filter(|&d| {
                let q = engine::offset(p.pos, d);
                let other = ae2_provider_loc(self, q, Some(fdir(d.opposite())));
                let kind = other.and_then(|o| self.ae2_kind_at(o));
                let skip = match kind {
                    Some(Kind::Provider(_)) => true,
                    Some(Kind::Interface(_)) => other.is_some_and(|o| self.ae2_net(o).is_some() && self.ae2_net(o) == net),
                    _ => false,
                };
                !skip && super::foundry::has_items(self, q, Some(fdir(d.opposite())))
            })
            .collect()
    }
    /// PatternProviderLogic.pushPattern: molecular assemblers first (ICraftingMachine), then
    /// any inventory.
    fn ae2_provider_push(&mut self, p: Loc, d: &cr::Details, table: &[Vec<(Key, u64)>]) -> bool {
        let Some(k) = self.ae2_kind_at(p) else { return false };
        if !self.ae2_device_active(p, k) {
            return false;
        }
        let Some(dev) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return false };
        let mut st = cr::Provider::load_from(&dev);
        if st.busy() || !dev.slots.iter().take(cr::PROVIDER_PATTERNS).flatten().any(|(k, _)| *k == d.definition) {
            return false;
        }
        let sides = self.ae2_provider_sides(p);
        let mut generic = Vec::new();
        for side in sides {
            let q = engine::offset(p.pos, side);
            if self.ae2_kind_at(Loc::block(q)) == Some(Kind::Assembler) && self.ae2_assembler_accepts_plans(q) {
                if self.ae2_assembler_accept(q, d, table, side.opposite()) {
                    return true;
                }
                continue;
            }
            generic.push(side);
        }
        let mut inputs = std::collections::BTreeSet::new();
        for (key, _) in dev.slots.iter().take(cr::PROVIDER_PATTERNS).flatten() {
            if let Some(x) = self.ae2_details(key) {
                inputs.extend(x.input_keys());
            }
        }
        let ok = {
            let mut t = ProvTargets { game: self, pos: p.pos, sides: generic };
            st.push(d, table, &mut t, &inputs)
        };
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(p) {
                st.save_to(dd);
            }
        });
        if ok {
            with_rt(|rt| {
                if let Some(t) = rt.tickers.get_mut(&p) {
                    t.wake(rt.tick);
                }
            });
        }
        ok
    }
    /// The provider's work: the return inventory into the network (CPUs take what they wait
    /// for), then the send list.
    fn ae2_provider_tick(&mut self, p: Loc) -> engine::TickRate {
        let Some(dev) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return engine::TickRate::Sleep };
        let mut did = false;
        for i in cr::PROVIDER_PATTERNS..cr::PROVIDER_PATTERNS + cr::PROVIDER_RETURN {
            let Some((k, n)) = dev.slots.get(i).cloned().flatten() else { continue };
            let ins = self.ae2_with_network(p, |live, _| engine::net_insert(live, &k, u64::from(n), false)).unwrap_or(0) as u32;
            if ins > 0 {
                did = true;
                with_st(self, |s| {
                    if let Some(d) = s.devices.at_mut(p)
                        && let Some(slot) = d.slots.get_mut(i)
                        && let Some((sk, c)) = slot.as_mut()
                        && *sk == k
                    {
                        *c -= ins.min(*c);
                        if *c == 0 {
                            *slot = None;
                        }
                    }
                });
            }
        }
        let dev = with_st(self, |s| s.devices.at(p).cloned()).flatten().unwrap_or(dev);
        let mut st = cr::Provider::load_from(&dev);
        if st.busy() {
            let sides = self.ae2_provider_sides(p);
            let flushed = {
                let mut t = ProvTargets { game: self, pos: p.pos, sides };
                st.flush(&mut t)
            };
            did |= flushed;
            with_st(self, |s| {
                if let Some(dd) = s.devices.at_mut(p) {
                    st.save_to(dd);
                }
            });
        }
        let work = st.busy() || dev.slots.iter().skip(cr::PROVIDER_PATTERNS).any(Option::is_some);
        match (work, did) {
            (false, _) => engine::TickRate::Sleep,
            (true, true) => engine::TickRate::Urgent,
            (true, false) => engine::TickRate::Slower,
        }
    }

    // ---- molecular assemblers ----

    /// myPlan: the pattern a provider pushed, or the one in the assembler's pattern slot.
    fn ae2_assembler_plan(&self, p: Loc) -> Option<Rc<cr::Details>> {
        let d = with_st(self, |s| s.devices.at(p).cloned()).flatten()?;
        if let Some((k, _)) = d.list("plan").first() {
            return self.ae2_details(k);
        }
        let (k, _) = d.slots.get(10)?.as_ref()?;
        self.ae2_details(k).filter(|x| x.assembler())
    }
    /// acceptsPlans: no pattern of its own.
    fn ae2_assembler_accepts_plans(&self, q: Pos) -> bool {
        with_st(self, |s| s.devices.at(q).is_some_and(|d| d.slots.get(10).is_some_and(Option::is_none))).unwrap_or(false)
    }
    /// pushPattern: an empty assembler takes the inputs into its grid and pushes the result
    /// back towards `back`.
    fn ae2_assembler_accept(&mut self, q: Pos, d: &cr::Details, table: &[Vec<(Key, u64)>], back: Dir) -> bool {
        if !d.assembler() {
            return false;
        }
        self.ae2_ensure(q);
        let grid = d.fill_grid(table);
        let total: u64 = table.iter().flatten().map(|(_, n)| *n).sum();
        if grid.iter().flatten().count() as u64 != total {
            return false; // never drop an input the grid would not hold
        }
        let ok = with_st(self, |s| {
            let dev = s.devices.at_mut(q)?;
            if dev.slots.iter().take(11).any(Option::is_some) || !dev.list("plan").is_empty() {
                return None;
            }
            for (i, g) in grid.iter().enumerate() {
                dev.slots[i] = g.clone().map(|k| (k, 1));
            }
            dev.set_list("plan", vec![(d.definition.clone(), 1)]);
            dev.set_num("push_dir", back.idx() as i64 + 1);
            dev.set_num("progress", 0);
            Some(())
        })
        .flatten()
        .is_some();
        ok
    }
    /// pushOut: into the push direction, or every side.
    fn ae2_assembler_push_out(&mut self, p: Pos, key: &Key, n: u32) -> u32 {
        let dir = with_st(self, |s| s.devices.at(p).map(|d| d.num("push_dir"))).flatten().unwrap_or(0);
        let dirs: Vec<Dir> = if dir > 0 { Dir::from_idx(dir as usize - 1).into_iter().collect() } else { Dir::ALL.to_vec() };
        let mut left = u64::from(n);
        for d in dirs {
            if left == 0 {
                break;
            }
            let q = engine::offset(p, d);
            left -= bus_inv_op(self, q, d.opposite(), |inv| engine::inv_insert(inv, key, left, false)).unwrap_or(0).min(left);
        }
        left as u32
    }
    /// MolecularAssemblerBlockEntity.tickingRequest.
    fn ae2_assembler_tick(&mut self, p: Pos) {
        let l = Loc::block(p);
        let Some(d) = with_st(self, |s| s.devices.at(l).cloned()).flatten() else { return };
        if d.slots.len() < 16 {
            self.ae2_ensure(l);
            return;
        }
        if let Some((k, n)) = d.slots[9].clone() {
            let left = self.ae2_assembler_push_out(p, &k, n);
            with_st(self, |s| {
                if let Some(dd) = s.devices.at_mut(l) {
                    dd.slots[9] = (left > 0).then(|| (k.clone(), left));
                    if left == 0 && !dd.list("plan").is_empty() {
                        // forcePlan off: the plan comes from the pattern slot again.
                        dd.set_list("plan", Vec::new());
                        dd.set_num("push_dir", 0);
                    }
                    dd.set_num("progress", 0);
                }
            });
            if left == 0 {
                self.ae2_assembler_eject(l);
            }
            return;
        }
        let Some(plan) = self.ae2_assembler_plan(l) else { return };
        let grid: Vec<Option<Key>> = (0..9).map(|i| d.slots[i].as_ref().map(|(k, _)| k.clone())).collect();
        if !cr::assembler_ready(&plan, &grid, &item_id) {
            self.ae2_assembler_eject(l);
            return;
        }
        let cards = cr::count_cards(&device_cards(&d, Kind::Assembler), cr::SPEED_CARD);
        let gained = self.ae2_with_network(l, |_, power| cr::assembler_progress(&mut Pool(power), 1, cards)).unwrap_or(0);
        let progress = d.num("progress") as i32 + gained;
        if progress < cr::ASSEMBLER_PROGRESS {
            with_st(self, |s| {
                if let Some(dd) = s.devices.at_mut(l) {
                    dd.set_num("progress", i64::from(progress));
                }
            });
            return;
        }
        let Some(((out, count), after)) = cr::assembler_craft(&plan, &grid, &item_id) else { return };
        let left = self.ae2_assembler_push_out(p, &out, count as u32);
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(l) {
                for (i, a) in after.iter().enumerate() {
                    dd.slots[i] = a.clone().map(|k| (k, 1));
                }
                dd.slots[9] = (left > 0).then(|| (out.clone(), left));
                dd.set_num("progress", 0);
                if dd.slots[10].is_none() {
                    dd.set_list("plan", Vec::new());
                    dd.set_num("push_dir", 0);
                }
            }
        });
        self.ae2_assembler_eject(l);
    }
    /// ejectHeldItems.
    fn ae2_assembler_eject(&mut self, l: Loc) {
        let plan = self.ae2_assembler_plan(l);
        with_st(self, |s| {
            let Some(d) = s.devices.at_mut(l) else { return };
            if d.slots.len() < 10 || d.slots[9].is_some() {
                return;
            }
            let grid: Vec<Option<Key>> = (0..9).map(|i| d.slots[i].as_ref().map(|(k, _)| k.clone())).collect();
            if let Some(i) = cr::assembler_eject(plan.as_deref(), &grid, &item_id) {
                d.slots[9] = d.slots[i].take();
            }
        });
    }

    // ---- level emitter ----

    /// The emitter's state (isLevelEmitterOn). Without redstone output on this pin it is shown
    /// in its window only.
    fn ae2_emitter_tick(&mut self, l: Loc) {
        let Some(d) = with_st(self, |s| s.devices.at(l).cloned()).flatten() else { return };
        let active = self.ae2_device_active(l, Kind::LevelEmitter);
        let cards = device_cards(&d, Kind::LevelEmitter);
        let key = d.config.first().cloned().flatten().map(|(k, _)| k);
        let on = if !active {
            false
        } else if cards.iter().any(|c| c == cr::CRAFTING_CARD) {
            let req = self.ae2_net(l).map(|n| self.ae2_netbook(n).requesting.clone()).unwrap_or_default();
            let requesting = match &key {
                Some(k) => req.contains(k),
                None => !req.is_empty(),
            };
            cr::emitter_on(true, true, requesting, false, 0, 0)
        } else {
            let level = d.num("level").max(0) as u64;
            let items = self.ae2_net_items(l);
            let reported = cr::emitter_reported(&items, key.as_ref(), cards.iter().any(|c| c == cr::FUZZY_CARD), &item_id, level);
            cr::emitter_on(true, false, false, d.num("low") != 0, level, reported)
        };
        if on != (d.num("on") != 0) {
            with_st(self, |s| {
                if let Some(dd) = s.devices.at_mut(l) {
                    dd.set_num("on", i64::from(on));
                }
            });
        }
        // AbstractLevelEmitterPart.isProvidingStrongPower / isProvidingWeakPower: 15 out of its face
        // while on, and updateState's notifyBlocksOfNeighbors (the host, the block it powers):
        // foundry's redstone, the host turning into its signal twin while it emits.
        if let Some(side) = l.side {
            let s = if on { super::foundry::Signal::side(fdir(side), 15, 15) } else { super::foundry::Signal::off() };
            super::foundry::set_part_redstone(self, l.pos, Some(fdir(side)), s);
        }
    }

    // ---- buses and interface with upgrade cards (replace the stage-1 versions) ----

    fn ae2_import(&mut self, p: Loc) -> engine::TickRate {
        let Some(front) = self.ae2_front(p, Kind::ImportBus) else { return engine::TickRate::Idle };
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return engine::TickRate::Sleep };
        let cards = device_cards(&d, Kind::ImportBus);
        let shown = cr::io_slots(cr::count_cards(&cards, cr::CAPACITY_CARD));
        let filter = cr::Filter {
            keys: d.config.iter().take(shown).flatten().map(|(k, _)| k.clone()).collect(),
            inverted: cards.iter().any(|c| c == cr::INVERTER_CARD),
            fuzzy: cards.iter().any(|c| c == cr::FUZZY_CARD),
        };
        let ops = cr::io_ops(cr::count_cards(&cards, cr::SPEED_CARD));
        let moved = self
            .ae2_with_network(p, |live, _| {
                live.target = Some((engine::offset(p.pos, front), front.opposite()));
                cr::import_filtered(live, &filter, &item_id, ops)
            })
            .unwrap_or_default();
        self.ae2_strays(p.pos, &moved.stray);
        if moved.items > 0 { engine::TickRate::Faster } else { engine::TickRate::Slower }
    }
    fn ae2_export(&mut self, p: Loc) -> engine::TickRate {
        let Some(front) = self.ae2_front(p, Kind::ExportBus) else { return engine::TickRate::Idle };
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return engine::TickRate::Sleep };
        let cards = device_cards(&d, Kind::ExportBus);
        let shown = cr::io_slots(cr::count_cards(&cards, cr::CAPACITY_CARD));
        let config: Vec<Option<Key>> = d.config.iter().take(shown).map(|c| c.as_ref().map(|(k, _)| k.clone())).collect();
        if config.iter().all(Option::is_none) {
            return engine::TickRate::Sleep;
        }
        let fuzzy = cards.iter().any(|c| c == cr::FUZZY_CARD);
        let crafting = cards.iter().any(|c| c == cr::CRAFTING_CARD);
        let ops = cr::io_ops(cr::count_cards(&cards, cr::SPEED_CARD));
        let net_keys: Vec<Key> = if fuzzy { self.ae2_net_items(p).into_keys().collect() } else { Vec::new() };
        let target = (engine::offset(p.pos, front), front.opposite());
        let (moved, idle, remaining) = self
            .ae2_with_network(p, |live, power| {
                live.target = Some(target);
                cr::export_filtered(live, &mut Pool(power), &config, fuzzy, &net_keys, &item_id, ops)
            })
            .unwrap_or_default();
        self.ae2_strays(p.pos, &moved.stray);
        let mut asked = false;
        if crafting {
            // attemptCrafting: as much as the target takes of what is left of the operations.
            for slot in idle {
                let Some(key) = config.get(slot).cloned().flatten() else { continue };
                let amount = bus_inv_op(self, target.0, target.1, |inv| engine::inv_insert(inv, &key, remaining.max(1), true)).unwrap_or(0);
                asked |= self.ae2_machine_craft(p, slot, &key, amount);
            }
        }
        if moved.items > 0 || asked { engine::TickRate::Faster } else { engine::TickRate::Slower }
    }
    /// InterfaceLogic.updateStorage with the crafting and fuzzy cards.
    fn ae2_interface(&mut self, p: Loc) -> engine::TickRate {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return engine::TickRate::Sleep };
        let cards = device_cards(&d, Kind::Interface(p.side.is_some()));
        let crafting = cards.iter().any(|c| c == cr::CRAFTING_CARD);
        let fuzzy = cards.iter().any(|c| c == cr::FUZZY_CARD);
        let mut work = false;
        let mut did = false;
        for x in 0..engine::config::INTERFACE_SLOTS.min(d.config.len()) {
            let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { break };
            let cfg = d.config[x].as_ref().map(|(k, n)| {
                let max = item_of_key(k).map_or(64, |i| self.ae2_max_stack(&i));
                (k.clone(), (*n).min(u64::from(max)))
            });
            let stored = d.slots[x].as_ref().map(|(k, n)| (k.clone(), u64::from(*n)));
            let Some((key, amount)) = engine::interface_plan(cfg.as_ref().map(|(k, n)| (k, *n)), stored.as_ref().map(|(k, n)| (k, *n))) else {
                continue;
            };
            work = true;
            if amount < 0 {
                let n = (-amount) as u64;
                let inserted = self.ae2_with_network(p, |live, power| engine::powered_insert(&mut Pool(power), live, &key, n, false)).unwrap_or(0);
                if inserted > 0 {
                    did = true;
                    self.ae2_slot_add(p, x, &key, -(inserted as i64));
                }
                continue;
            }
            if self.ae2_slot_busy(p, x) {
                continue; // waiting for a crafting job of this slot
            }
            let n = amount as u64;
            let mut got = self.ae2_with_network(p, |live, power| engine::powered_extract(&mut Pool(power), live, &key, n, false)).unwrap_or(0);
            if got > 0 {
                self.ae2_slot_add(p, x, &key, got as i64);
            } else if fuzzy && stored.is_none() {
                // A fuzzy import when the slot is empty: any key of the same item.
                let item = item_id(&key);
                let others: Vec<Key> = self.ae2_net_items(p).into_keys().filter(|k| *k != key && item_id(k) == item).collect();
                for k in others {
                    let g = self.ae2_with_network(p, |live, power| engine::powered_extract(&mut Pool(power), live, &k, n, false)).unwrap_or(0);
                    if g > 0 {
                        self.ae2_slot_add(p, x, &k, g as i64);
                        got = g;
                        break;
                    }
                }
            }
            if got > 0 {
                did = true;
            } else if crafting && self.ae2_machine_craft(p, x, &key, n) {
                did = true;
            }
        }
        match (work, did) {
            (false, _) => engine::TickRate::Sleep,
            (true, true) => engine::TickRate::Urgent,
            (true, false) => engine::TickRate::Slower,
        }
    }
    /// Adds (or with `delta` < 0 removes) items of `key` in a record slot.
    fn ae2_slot_add(&self, p: Loc, slot: usize, key: &Key, delta: i64) {
        with_st(self, |s| {
            let Some(cur) = s.devices.at_mut(p).and_then(|d| d.slots.get_mut(slot)) else { return };
            match cur {
                Some((k, c)) if k == key => {
                    let v = (i64::from(*c) + delta).max(0) as u32;
                    if v == 0 {
                        *cur = None;
                    } else {
                        *c = v;
                    }
                }
                None if delta > 0 => *cur = Some((key.clone(), delta as u32)),
                _ => {}
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Screens.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WinKind {
    Terminal,
    Crafting,
    ChestTerm,
    ChestCell,
    Drive,
    Interface,
    ImportBus,
    ExportBus,
    StorageBus,
    Inscriber,
    Vibration,
    // stage 2
    Encoding,
    Access,
    Provider,
    Assembler,
    Workbench,
    Emitter,
    CraftAmount,
    CraftConfirm,
    CraftCpu,
}
impl WinKind {
    /// A terminal: the network's grid with search and sort.
    fn terminal(self) -> bool {
        matches!(self, WinKind::Terminal | WinKind::Crafting | WinKind::ChestTerm | WinKind::Encoding)
    }
    /// The crafting dialogs have no player inventory.
    fn no_inventory(self) -> bool {
        matches!(self, WinKind::CraftAmount | WinKind::CraftConfirm | WinKind::CraftCpu)
    }
    /// The height of a terminal's part below its grid (TerminalStyle bottom).
    fn bottom(self) -> f32 {
        match self {
            WinKind::Crafting => 170.0,
            WinKind::Encoding => 178.0,
            _ => 97.0,
        }
    }
    /// A number typed into the window (craft amount, emitter level).
    fn number(self) -> bool {
        matches!(self, WinKind::CraftAmount | WinKind::Emitter)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Grid(usize),
    Real(usize),
    Ghost(usize),
    Result,
    /// Display only (plan and CPU tables, recipe results, stonecutter choices).
    Show(usize),
    /// A pattern slot of a provider in the pattern access terminal.
    Remote(usize),
    /// The partition of the cell in the workbench, and its cards.
    CellCfg(usize),
    CellUpg(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Btn {
    SortBy,
    SortDir,
    Status,
    Mode(u8),
    Encode,
    Substitute,
    FluidSub,
    Clear,
    Blocking,
    Add(i64),
    Next,
    Back,
    Start,
    Cancel,
    CpuCycle,
    CancelJob,
    LowHigh,
    CraftVia,
    Partition,
    ClearCfg,
}
#[derive(Clone)]
struct Window {
    pos: Loc,
    kind: WinKind,
    rows: usize,
    scroll: usize,
    view: Vec<(Key, u64)>,
    slots: Vec<(Slot, f32, f32)>,
    w: f32,
    h: f32,
    search: String,
    focused: bool,
    last_shift: Option<(std::time::Instant, usize)>,
    active: bool,
    // stage 2
    /// Processing pattern scroll, table and access-terminal rows.
    page: usize,
    /// The number being typed (craft amount, emitter level).
    amount: String,
    /// What the craft dialogs are about.
    craft: Option<Key>,
    plan: Option<cr::Plan>,
    /// The chosen CPU (None = automatic) / the CPU shown.
    cpu: Option<Pos>,
    /// The terminal the craft dialogs return to.
    back: Option<WinKind>,
    error: Option<String>,
    /// Show(i) contents and the table lines (amount lines per cell).
    shown: Vec<Option<(Key, u64)>>,
    lines: Vec<Vec<String>>,
    /// Access terminal: Remote(i) -> (provider, slot); the group names (row y, text).
    remote: Vec<(Loc, usize)>,
    labels: Vec<(f32, String)>,
    craftable: std::collections::BTreeSet<Key>,
}
impl Window {
    fn key(&self) -> (i32, i32, i32) {
        (self.pos.pos[0], self.pos.pos[1], self.pos.pos[2])
    }
    fn max_scroll(&self) -> usize {
        let rows = self.view.len().div_ceil(9);
        rows.saturating_sub(self.rows)
    }
}
const TERM_HEADER: f32 = 17.0;
/// The crafting tables (AbstractTableRenderer): 3 columns of 67 x 22 cells from (9, 19).
const TABLE_X: f32 = 9.0;
const TABLE_Y: f32 = 19.0;
const CELL_W: f32 = 67.0;
const CELL_H: f32 = 22.0;
fn texture(kind: WinKind) -> &'static str {
    match kind {
        WinKind::Terminal | WinKind::ChestTerm => "ae2:block/gui/terminal",
        WinKind::Crafting => "ae2:block/gui/crafting",
        WinKind::ChestCell => "ae2:block/gui/chest",
        WinKind::Drive => "ae2:block/gui/drive",
        WinKind::Interface => "ae2:block/gui/interface",
        WinKind::ImportBus | WinKind::ExportBus | WinKind::StorageBus => "ae2:block/gui/storagebus",
        WinKind::Inscriber => "ae2:block/gui/inscriber",
        WinKind::Vibration => "ae2:block/gui/vibchamber",
        WinKind::Encoding => "ae2:block/gui/pattern",
        WinKind::Access => "ae2:block/gui/patternaccessterminal",
        WinKind::Provider => "ae2:block/gui/pattern_provider",
        WinKind::Assembler => "ae2:block/gui/molecular_assembler",
        WinKind::Workbench => "ae2:block/gui/cellworkbench",
        WinKind::Emitter => "ae2:block/gui/level_emitter",
        WinKind::CraftAmount => "ae2:block/gui/craft_amt",
        WinKind::CraftConfirm => "ae2:block/gui/craftingreport",
        WinKind::CraftCpu => "ae2:block/gui/craftingcpu",
    }
}
fn title(kind: WinKind) -> (&'static str, &'static str) {
    match kind {
        WinKind::Terminal => ("gui.ae2.Terminal", "Terminal"),
        WinKind::Crafting => ("gui.ae2.CraftingTerminal", "Crafting Terminal"),
        WinKind::ChestTerm | WinKind::ChestCell => ("block.ae2.chest", "ME Chest"),
        WinKind::Drive => ("block.ae2.drive", "ME Drive"),
        WinKind::Interface => ("gui.ae2.Interface", "ME Interface"),
        WinKind::ImportBus => ("gui.ae2.ImportBus", "Import Bus"),
        WinKind::ExportBus => ("gui.ae2.ExportBus", "Export Bus"),
        WinKind::StorageBus => ("gui.ae2.StorageBus", "Storage Bus"),
        WinKind::Inscriber => ("gui.ae2.Inscriber", "Inscriber"),
        WinKind::Vibration => ("gui.ae2.VibrationChamber", "Vibration Chamber"),
        WinKind::Encoding => ("gui.ae2.Terminal", "Terminal"),
        WinKind::Access => ("gui.ae2.PatternAccessTerminalShort", "Pattern A. Terminal"),
        WinKind::Provider => ("gui.ae2.CraftingInterface", "ME Pattern Provider"),
        WinKind::Assembler => ("gui.ae2.MolecularAssembler", "Molecular Assembler"),
        WinKind::Workbench => ("gui.ae2.CellWorkbench", "Cell Workbench"),
        WinKind::Emitter => ("gui.ae2.LevelEmitter", "ME Level Emitter"),
        WinKind::CraftAmount => ("gui.ae2.SelectAmount", "Select Amount"),
        WinKind::CraftConfirm => ("gui.ae2.CraftingPlan", "Crafting Plan - %s"),
        WinKind::CraftCpu => ("gui.ae2.CraftingStatus", "Crafting Status"),
    }
}
/// UpgradesPanel: right of the window, slots from (w + 2 + 7 + 1, 8).
fn panel_slot(w: f32, i: usize) -> (f32, f32) {
    (w + 10.0, 8.0 + i as f32 * 18.0)
}
fn fmt1(t: &str, v: &str) -> String {
    t.replacen("%s", v, 1).replacen("%d", v, 1)
}

impl Game<'_> {
    fn ae2_gui_height(&self) -> i32 {
        super::foundry::hud_batch(self).map_or(240, |b| b.height as i32)
    }
    fn ae2_open(&mut self, pos: Loc, k: Kind) {
        if self.remote.is_some() {
            return;
        }
        // A guest's turn (their click reached the server): their own window, shown on their client
        // as a foundry menu. A client without foundry cannot show it.
        let guest = self.lan_acting;
        if let Some(conn) = guest
            && !super::foundry::guest_has_foundry(conn)
        {
            kit_notice::action_bar(self, "AE2: this window needs Horizon with foundry on your side".to_owned());
            return;
        }
        ae2_register_menu();
        let host_window = if guest.is_some() { with_rt(|rt| rt.window.take()).flatten() } else { None };
        let kind = match k {
            Kind::Terminal => WinKind::Terminal,
            Kind::CraftingTerminal => WinKind::Crafting,
            Kind::Chest if kit_blocks::sneaking(self) => WinKind::ChestCell,
            Kind::Chest => WinKind::ChestTerm,
            Kind::Drive => WinKind::Drive,
            Kind::Interface(_) => WinKind::Interface,
            Kind::ImportBus => WinKind::ImportBus,
            Kind::ExportBus => WinKind::ExportBus,
            Kind::StorageBus => WinKind::StorageBus,
            Kind::Inscriber => WinKind::Inscriber,
            Kind::Vibration => WinKind::Vibration,
            Kind::EncodingTerminal => WinKind::Encoding,
            Kind::AccessTerminal => WinKind::Access,
            Kind::Provider(_) => WinKind::Provider,
            Kind::Assembler => WinKind::Assembler,
            Kind::Workbench => WinKind::Workbench,
            Kind::LevelEmitter => WinKind::Emitter,
            Kind::CraftUnit(_) => WinKind::CraftCpu,
            _ => return,
        };
        let gh = self.ae2_gui_height();
        let rows = if kind.terminal() {
            engine::terminal_rows(gh, TERM_HEADER as i32, kind.bottom() as i32) as usize
        } else if kind == WinKind::Access {
            // PatternAccessTermScreen: (height - header - footer - 54) / 18 rows.
            (((gh - 17 - 97 - 54) / 18).max(2)) as usize
        } else {
            0
        };
        let key = (pos.pos[0], pos.pos[1], pos.pos[2]);
        self.world.block_entities.insert(key, BlockEntity::Chest(Vec::new()));
        let search = if engine::config::SEARCH_REMEMBER_LAST_SEARCH { with_rt(|rt| rt.last_search.clone()).unwrap_or_default() } else { String::new() };
        let level = with_st(self, |s| s.devices.at(pos).map(|d| d.num("level"))).flatten().unwrap_or(0);
        with_rt(|rt| {
            rt.window = Some(Window {
                pos,
                kind,
                rows,
                scroll: 0,
                view: Vec::new(),
                slots: Vec::new(),
                w: 176.0,
                h: 166.0,
                search,
                focused: engine::config::SEARCH_AUTO_FOCUS_SEARCH && kind.terminal(),
                last_shift: None,
                active: false,
                page: 0,
                amount: if kind == WinKind::Emitter { level.to_string() } else { String::new() },
                craft: None,
                plan: None,
                cpu: None,
                back: None,
                error: None,
                shown: Vec::new(),
                lines: Vec::new(),
                remote: Vec::new(),
                labels: Vec::new(),
                craftable: Default::default(),
            })
        });
        self.container = Some(key);
        self.screen = Screen::Inventory;
        if guest.is_none() {
            self.capture(false);
            self.right = false;
        }
        if kind == WinKind::CraftCpu {
            self.ae2_pick_cpu(None);
        }
        self.ae2_window_refresh();
        if let Some(conn) = guest {
            // Kept apart; after the turn `lan_open_container` opens it as a foundry menu
            // (`ae2_menu_claims`). The host's own window is back in place.
            if let Some(w) = with_rt(|rt| rt.window.take()).flatten() {
                GUESTS.with(|g| g.borrow_mut().insert(conn, w));
            }
            let refill = host_window.as_ref().is_some_and(|w| w.key() == key);
            with_rt(|rt| rt.window = host_window);
            if refill {
                self.ae2_window_fill();
            }
        }
    }
    /// Another dialog in the same window (the terminal's craft amount, plan, CPU status).
    fn ae2_switch(&mut self, kind: WinKind, setup: impl FnOnce(&mut Window)) {
        with_rt(|rt| {
            if let Some(w) = rt.window.as_mut() {
                if w.kind.terminal() {
                    rt.last_search = w.search.clone();
                }
                w.kind = kind;
                w.page = 0;
                w.error = None;
                w.focused = kind == WinKind::CraftAmount;
                setup(w);
            }
        });
        self.ae2_window_refresh();
    }
    fn ae2_window(&self) -> Option<(Loc, WinKind)> {
        ae2_guest_sync(self);
        with_rt(|rt| rt.window.as_ref().map(|w| (w.pos, w.kind))).flatten()
    }
    /// The open window, when it is the screen being shown.
    fn ae2_window_open(&self) -> Option<(Loc, WinKind)> {
        let (p, k) = self.ae2_window()?;
        (self.screen == Screen::Inventory && self.container == Some((p.pos[0], p.pos[1], p.pos[2]))).then_some((p, k))
    }
    fn ae2_window_tick(&mut self) {
        let Some((p, _)) = self.ae2_window() else { return };
        if self.container != Some((p.pos[0], p.pos[1], p.pos[2])) || self.ae2_kind_at(p).is_none() {
            self.ae2_window_closed();
            return;
        }
        if with_rt(|rt| rt.tick % 5 == 0) == Some(true) {
            self.ae2_window_refresh();
        }
    }
    fn ae2_window_closed(&mut self) {
        let Some(w) = with_rt(|rt| rt.window.take()).flatten() else { return };
        // A guest's window on the same block keeps the temporary slots.
        if matches!(self.world.block_entities.get(&w.key()), Some(BlockEntity::Chest(_))) && !self.ae2_window_at(w.key()) {
            self.world.block_entities.remove(&w.key());
        }
        if w.kind.terminal() {
            with_rt(|rt| rt.last_search = w.search.clone());
        }
    }
    /// The CPU the status window shows: the asked one, else the first busy one, else the first.
    fn ae2_pick_cpu(&mut self, want: Option<Pos>) {
        let Some((p, _)) = self.ae2_window() else { return };
        let cpus = self.ae2_net(p).map(|n| self.ae2_cpus_of(n)).unwrap_or_default();
        let cur = with_rt(|rt| rt.window.as_ref().and_then(|w| w.cpu)).flatten();
        let pick = want.or(cur).filter(|c| cpus.iter().any(|(x, _)| x == c)).or_else(|| cpus.iter().find(|(_, i)| i.busy).or(cpus.first()).map(|(c, _)| *c));
        with_rt(|rt| {
            if let Some(w) = rt.window.as_mut() {
                w.cpu = pick.or(w.cpu);
            }
        });
    }
    /// The terminal's repo (Repo.updateView, stored and craftable items), the dialogs' tables,
    /// the access terminal's rows; then every window slot shows its stack.
    fn ae2_window_refresh(&mut self) {
        // A guest's window: the network is the server's (its view comes with the window).
        if self.remote.is_some() {
            return;
        }
        let Some((p, kind)) = self.ae2_window() else { return };
        let (by, dir) = with_st(self, |s| {
            s.devices.at(p).map(|d| (engine::SortBy::from_code(d.num("sort")), if d.num("dir") != 0 { engine::SortDir::Descending } else { engine::SortDir::Ascending }))
        })
        .flatten()
        .unwrap_or_default();
        if kind.terminal() {
            let active = self.ae2_term_active(p, kind);
            let list = if active { self.ae2_term_list(p, kind) } else { BTreeMap::new() };
            let craftable: std::collections::BTreeSet<Key> = if active && kind != WinKind::ChestTerm {
                self.ae2_net(p).map(|n| self.ae2_netbook(n).book.craftable_keys()).unwrap_or_default()
            } else {
                Default::default()
            };
            let search = with_rt(|rt| rt.window.as_ref().map(|w| w.search.clone())).flatten().unwrap_or_default();
            let needs_tooltip = search.contains('#');
            let needs_tags = search.contains('$');
            let mut all = list.clone();
            for k in &craftable {
                all.entry(k.clone()).or_insert(0);
            }
            let mut entries = Vec::with_capacity(all.len());
            for (key, amount) in all {
                let Some(s) = stack_of(&key, 1) else { continue };
                let id = s.item.to_string();
                let mod_id = id.split(':').next().unwrap_or("minecraft").to_owned();
                let name = horizon_client::stations::hover_name(&|i| self.pack.names.display_item(i), &s);
                let tooltip = if needs_tooltip {
                    self.stack_tooltip(&s).iter().map(|(t, _)| t.plain()).collect::<Vec<_>>().join("\n")
                } else {
                    String::new()
                };
                let tags = if needs_tags {
                    self.res.items().item_tags.iter().filter(|(_, set)| set.contains(id.as_str())).map(|(t, _)| t.to_string()).collect()
                } else {
                    Vec::new()
                };
                let mod_name = match mod_id.as_str() {
                    "minecraft" => "Minecraft".to_owned(),
                    "ae2" => "Applied Energistics 2".to_owned(),
                    m => m.to_owned(),
                };
                entries.push(engine::Entry { key, amount, name, id, mod_id, mod_name, tooltip, tags });
            }
            // ViewItems.ALL: stored items and craftable ones.
            let keep = |e: &engine::Entry| e.amount > 0 || craftable.contains(&e.key);
            let view: Vec<(Key, u64)> = engine::view_keep(&entries, &keep, &search, by, dir).into_iter().map(|e| (e.key, e.amount)).collect();
            with_rt(|rt| {
                if let Some(w) = rt.window.as_mut() {
                    w.view = view;
                    w.active = active;
                    w.craftable = craftable;
                    w.scroll = w.scroll.min(w.max_scroll());
                }
            });
        }
        match kind {
            WinKind::CraftConfirm => self.ae2_confirm_table(p),
            WinKind::CraftCpu => self.ae2_cpu_table(p),
            WinKind::Access => self.ae2_access_rows(p),
            WinKind::Encoding => self.ae2_encoding_shown(p),
            _ => {}
        }
        self.ae2_relayout();
        self.ae2_window_fill();
    }
    /// The encoding area's result (crafting, smithing) or the stonecutter's choices.
    fn ae2_encoding_shown(&mut self, p: Loc) {
        let d = with_st(self, |s| s.devices.at(p).cloned()).flatten().unwrap_or_default();
        let (res, choices) = self.ae2_encoding_result(&d);
        let shown: Vec<Option<(Key, u64)>> = if cr::PatternKind::from_index(d.num("mode")) == cr::PatternKind::Stonecutting {
            choices.iter().map(|(_, s)| Some((key_of(s), u64::from(s.count)))).collect()
        } else {
            vec![res.map(|(_, s)| (key_of(&s), u64::from(s.count)))]
        };
        with_rt(|rt| {
            if let Some(w) = rt.window.as_mut() {
                w.shown = shown;
            }
        });
    }
    /// The confirm dialog's table (CraftingPlanSummary).
    fn ae2_confirm_table(&mut self, p: Loc) {
        let Some(plan) = with_rt(|rt| rt.window.as_ref().and_then(|w| w.plan.clone())).flatten() else { return };
        let book = self.ae2_net(p).map(|n| self.ae2_netbook(n).book.clone()).unwrap_or_default();
        let items = self.ae2_net_items(p);
        let entries = plan.summary(&book, &items);
        let (fs, miss, tc) = (
            self.translate("gui.ae2.FromStorage", "Available: %s"),
            self.translate("gui.ae2.Missing", "Missing: %s"),
            self.translate("gui.ae2.ToCraft", "To Craft: %s"),
        );
        let fa = |n: u64| engine::format_amount(n, 6);
        let mut shown = Vec::new();
        let mut lines = Vec::new();
        for e in entries {
            let mut l = Vec::new();
            if e.stored > 0 {
                l.push(fmt1(&fs, &fa(e.stored)));
            }
            if e.missing > 0 {
                l.push(fmt1(&miss, &fa(e.missing)));
            }
            if e.craft > 0 {
                l.push(fmt1(&tc, &fa(e.craft)));
            }
            shown.push(Some((e.key, e.missing)));
            lines.push(l);
        }
        with_rt(|rt| {
            if let Some(w) = rt.window.as_mut() {
                w.shown = shown;
                w.lines = lines;
            }
        });
    }
    /// The CPU window's table (CraftingStatus: stored, crafting, scheduled).
    fn ae2_cpu_table(&mut self, p: Loc) {
        let core = with_rt(|rt| rt.window.as_ref().and_then(|w| w.cpu)).flatten();
        let Some(core) = core else {
            with_rt(|rt| {
                if let Some(w) = rt.window.as_mut() {
                    w.shown.clear();
                    w.lines.clear();
                }
            });
            return;
        };
        let book = self.ae2_net(p).map(|n| self.ae2_netbook(n).book.clone()).unwrap_or_default();
        let mut status = self.ae2_cpu(core).status(&book);
        status.retain(|(_, a, b, c)| a + b + c > 0);
        status.sort_by(|x, y| (y.2, y.3, y.1).cmp(&(x.2, x.3, x.1)));
        let (fs, cr_, sc) = (
            self.translate("gui.ae2.FromStorage", "Available: %s"),
            self.translate("gui.ae2.Crafting", "Crafting: %s"),
            self.translate("gui.ae2.Scheduled", "Scheduled: %s"),
        );
        let fa = |n: u64| engine::format_amount(n, 6);
        let mut shown = Vec::new();
        let mut lines = Vec::new();
        for (k, stored, active, pending) in status {
            let mut l = Vec::new();
            if stored > 0 {
                l.push(fmt1(&fs, &fa(stored)));
            }
            if active > 0 {
                l.push(fmt1(&cr_, &fa(active)));
            }
            if pending > 0 {
                l.push(fmt1(&sc, &fa(pending)));
            }
            shown.push(Some((k, 0)));
            lines.push(l);
        }
        with_rt(|rt| {
            if let Some(w) = rt.window.as_mut() {
                w.shown = shown;
                w.lines = lines;
            }
        });
    }
    /// The pattern access terminal's rows: per provider a name row and its 9 pattern slots.
    fn ae2_access_rows(&mut self, p: Loc) {
        let members = self.ae2_net(p).and_then(|n| with_rt(|rt| rt.nets.get(n).map(|x| x.members.clone())).flatten()).unwrap_or_default();
        let mut provs: Vec<(String, Loc)> = Vec::new();
        for m in members {
            if let Some(k @ Kind::Provider(_)) = with_st(self, |s| s.devices.at(m).and_then(|d| kind_of(&d.block))).flatten()
                && self.ae2_device_active(m, k)
            {
                provs.push((self.ae2_provider_name(m), m));
            }
        }
        provs.sort();
        let (rows, page) = with_rt(|rt| rt.window.as_ref().map(|w| (w.rows, w.page))).flatten().unwrap_or((2, 0));
        let total = provs.len() * 2;
        let page = page.min(total.saturating_sub(rows));
        let mut remote = Vec::new();
        let mut labels = Vec::new();
        for r in 0..rows {
            let i = page + r;
            let Some((name, l)) = provs.get(i / 2) else { break };
            if i % 2 == 0 {
                labels.push((TERM_HEADER + r as f32 * 18.0, name.clone()));
            } else {
                for s in 0..cr::PROVIDER_PATTERNS {
                    remote.push((*l, s));
                }
            }
        }
        with_rt(|rt| {
            if let Some(w) = rt.window.as_mut() {
                w.page = page;
                w.remote = remote;
                w.labels = labels;
            }
        });
    }
    /// PatternContainerGroup: the adjacent machine's name, or the provider's own.
    fn ae2_provider_name(&self, l: Loc) -> String {
        let sides = self.ae2_provider_sides(l);
        if let [d] = sides.as_slice()
            && let Some(name) = ae2_block_name(self, engine::offset(l.pos, *d))
        {
            let (ns, path) = name.split_once(':').unwrap_or(("minecraft", &name));
            return self.translate(&format!("block.{ns}.{path}"), path);
        }
        self.translate("gui.ae2.CraftingInterface", "ME Pattern Provider")
    }
    /// The window's slots and size now (they change with modes, cards, rows).
    fn ae2_relayout(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some((p, kind, rows, page, remote_n, shown_n)) =
            with_rt(|rt| rt.window.as_ref().map(|w| (w.pos, w.kind, w.rows, w.page, w.remote.len(), w.shown.len()))).flatten()
        else {
            return;
        };
        let d = with_st(self, |s| s.devices.at(p).cloned()).flatten().unwrap_or_default();
        let k = kind_of(&d.block);
        let cards = k.map(|k| device_cards(&d, k)).unwrap_or_default();
        let capacity = cr::count_cards(&cards, cr::CAPACITY_CARD);
        let mut s = Vec::new();
        let (w, h) = match kind {
            WinKind::Terminal | WinKind::ChestTerm | WinKind::Crafting | WinKind::Encoding => {
                let h = TERM_HEADER + rows as f32 * 18.0 + kind.bottom();
                for i in 0..rows * 9 {
                    s.push((Slot::Grid(i), 8.0 + (i % 9) as f32 * 18.0, 18.0 + (i / 9) as f32 * 18.0));
                }
                if kind == WinKind::Crafting {
                    for i in 0..9 {
                        s.push((Slot::Real(i), 37.0 + (i % 3) as f32 * 18.0, h - 154.0 + (i / 3) as f32 * 18.0));
                    }
                    s.push((Slot::Result, 131.0, h - 136.0));
                }
                if kind == WinKind::Encoding {
                    s.push((Slot::Real(0), 147.0, h - 161.0));
                    s.push((Slot::Real(1), 147.0, h - 118.0));
                    match cr::PatternKind::from_index(d.num("mode")) {
                        cr::PatternKind::Crafting => {
                            for i in 0..9 {
                                s.push((Slot::Ghost(i), 18.0 + (i % 3) as f32 * 18.0, h - 156.0 + (i / 3) as f32 * 18.0));
                            }
                            s.push((Slot::Show(0), 110.0, h - 138.0));
                        }
                        cr::PatternKind::Processing => {
                            for r in 0..3 {
                                for c in 0..3 {
                                    s.push((Slot::Ghost((page + r) * 3 + c), 26.0 + c as f32 * 18.0, h - 156.0 + r as f32 * 18.0));
                                }
                                s.push((Slot::Ghost(cr::MAX_PROCESSING_INPUTS + page + r), 110.0, h - 156.0 + r as f32 * 18.0));
                            }
                        }
                        cr::PatternKind::Smithing => {
                            for i in 0..3 {
                                s.push((Slot::Ghost(i), 21.0 + i as f32 * 18.0, h - 138.0));
                            }
                            s.push((Slot::Show(0), 111.0, h - 138.0));
                        }
                        cr::PatternKind::Stonecutting => {
                            s.push((Slot::Ghost(0), 21.0, h - 139.0));
                            for i in 0..shown_n.min(12) {
                                s.push((Slot::Show(i), 52.0 + (i % 4) as f32 * 16.0, h - 157.0 + (i / 4) as f32 * 18.0));
                            }
                        }
                    }
                }
                (195.0, h)
            }
            WinKind::ChestCell => {
                s.push((Slot::Real(0), 80.0, 37.0));
                (176.0, 166.0)
            }
            WinKind::Drive => {
                for i in 0..10 {
                    s.push((Slot::Real(i), 71.0 + (i % 2) as f32 * 18.0, 14.0 + (i / 2) as f32 * 18.0));
                }
                (176.0, 199.0)
            }
            WinKind::Interface => {
                for i in 0..9 {
                    s.push((Slot::Ghost(i), 8.0 + i as f32 * 18.0, 53.0));
                }
                for i in 0..9 {
                    s.push((Slot::Real(i), 8.0 + i as f32 * 18.0, 71.0));
                }
                let (x, y) = panel_slot(176.0, 0);
                s.push((Slot::Real(9), x, y));
                (176.0, 205.0)
            }
            WinKind::ImportBus | WinKind::ExportBus | WinKind::StorageBus => {
                let n = if kind == WinKind::StorageBus { cr::storage_bus_slots(capacity) } else { cr::io_slots(capacity) };
                for i in 0..n {
                    s.push((Slot::Ghost(i), 8.0 + (i % 9) as f32 * 18.0, 29.0 + (i / 9) as f32 * 18.0));
                }
                for i in 0..5 {
                    let (x, y) = panel_slot(176.0, i);
                    s.push((Slot::Real(i), x, y));
                }
                (176.0, 251.0)
            }
            WinKind::Inscriber => {
                s.push((Slot::Real(0), 45.0, 16.0));
                s.push((Slot::Real(2), 63.0, 39.0));
                s.push((Slot::Real(1), 45.0, 62.0));
                s.push((Slot::Real(3), 113.0, 40.0));
                (176.0, 176.0)
            }
            WinKind::Vibration => {
                s.push((Slot::Real(0), 80.0, 37.0));
                (176.0, 166.0)
            }
            WinKind::Access => {
                let h = TERM_HEADER + rows as f32 * 18.0 + 97.0;
                // Remote slots fill the inventory rows (every second visible row).
                let mut i = 0;
                for r in 0..rows {
                    let row_has_slots = (page + r) % 2 == 1;
                    if !row_has_slots {
                        continue;
                    }
                    for c in 0..9 {
                        if i < remote_n {
                            s.push((Slot::Remote(i), 8.0 + c as f32 * 18.0, 18.0 + r as f32 * 18.0));
                            i += 1;
                        }
                    }
                }
                (195.0, h)
            }
            WinKind::Provider => {
                for i in 0..9 {
                    s.push((Slot::Real(i), 8.0 + i as f32 * 18.0, 45.0));
                    s.push((Slot::Real(9 + i), 8.0 + i as f32 * 18.0, 97.0));
                }
                (176.0, 213.0)
            }
            WinKind::Assembler => {
                for i in 0..9 {
                    s.push((Slot::Real(i), 29.0 + (i % 3) as f32 * 18.0, 30.0 + (i / 3) as f32 * 18.0));
                }
                s.push((Slot::Real(10), 126.0, 16.0));
                s.push((Slot::Real(9), 126.0, 48.0));
                for i in 0..5 {
                    let (x, y) = panel_slot(176.0, i);
                    s.push((Slot::Real(11 + i), x, y));
                }
                (176.0, 197.0)
            }
            WinKind::Workbench => {
                s.push((Slot::Real(0), 152.0, 8.0));
                for i in 0..cr::CELL_PARTITION {
                    s.push((Slot::CellCfg(i), 8.0 + (i % 9) as f32 * 18.0, 29.0 + (i / 9) as f32 * 18.0));
                }
                for i in 0..cr::upgrade_slots(cr::Host::ItemCell) {
                    let (x, y) = panel_slot(176.0, i);
                    s.push((Slot::CellUpg(i), x, y));
                }
                (176.0, 251.0)
            }
            WinKind::Emitter => {
                s.push((Slot::Ghost(0), 137.0, 40.0));
                let (x, y) = panel_slot(176.0, 0);
                s.push((Slot::Real(0), x, y));
                (176.0, 184.0)
            }
            WinKind::CraftAmount => {
                s.push((Slot::Show(0), 23.0, 53.0));
                (176.0, 107.0)
            }
            WinKind::CraftConfirm | WinKind::CraftCpu => {
                let table_rows = if kind == WinKind::CraftConfirm { 5 } else { 6 };
                for i in 0..table_rows * 3 {
                    let (c, r) = (i % 3, i / 3);
                    let x = TABLE_X + c as f32 * (CELL_W + 1.0) + CELL_W - 19.0;
                    let y = TABLE_Y + r as f32 * (CELL_H + 1.0) + 3.0;
                    s.push((Slot::Show(page * 3 + i), x, y));
                }
                (238.0, if kind == WinKind::CraftConfirm { 206.0 } else { 184.0 })
            }
        };
        with_rt(|rt| {
            if let Some(win) = rt.window.as_mut() {
                win.slots = s;
                win.w = w;
                win.h = h;
            }
        });
    }
    /// Writes what every window slot shows into the temporary block entity.
    fn ae2_window_fill(&mut self) {
        // A guest's window: its slots are foundry's mirror of the server's.
        if self.remote.is_some() {
            return;
        }
        let Some((p, kind, slots, view, scroll, shown, remote)) = with_rt(|rt| {
            rt.window.as_ref().map(|w| (w.pos, w.kind, w.slots.clone(), w.view.clone(), w.scroll, w.shown.clone(), w.remote.clone()))
        })
        .flatten() else {
            return;
        };
        let d = with_st(self, |s| s.devices.at(p).cloned()).flatten().unwrap_or_default();
        let result = if kind == WinKind::Crafting { self.ae2_craft_result(&d).map(|(_, r)| r) } else { None };
        let cell = if kind == WinKind::Workbench { d.slots.first().cloned().flatten().and_then(|(k, _)| stack_of(&k, 1)) } else { None };
        let table = matches!(kind, WinKind::CraftConfirm | WinKind::CraftCpu);
        let cfg = cell.as_ref().map(cell_config);
        let mut out: Vec<Option<ItemStack>> = Vec::with_capacity(slots.len());
        for (s, _, _) in &slots {
            out.push(match *s {
                Slot::Grid(i) => view.get(scroll * 9 + i).and_then(|(k, _)| stack_of(k, 1)),
                Slot::Real(i) => d.slots.get(i).and_then(|x| x.as_ref()).and_then(|(k, n)| stack_of(k, *n)),
                Slot::Ghost(i) => d.config.get(i).and_then(|x| x.as_ref()).and_then(|(k, _)| stack_of(k, 1)),
                Slot::Result => result.clone(),
                Slot::Show(i) => shown.get(i).cloned().flatten().and_then(|(k, n)| stack_of(&k, if table { 1 } else { n.clamp(1, 64) as u32 })),
                Slot::Remote(i) => remote.get(i).and_then(|(l, slot)| {
                    with_st(self, |st| st.devices.at(*l).and_then(|dd| dd.slots.get(*slot).cloned().flatten())).flatten().and_then(|(k, n)| stack_of(&k, n))
                }),
                Slot::CellCfg(i) => cfg.as_ref().and_then(|c| c.partition.get(i).cloned().flatten()).and_then(|k| stack_of(&k, 1)),
                Slot::CellUpg(i) => cfg.as_ref().and_then(|c| c.upgrades.get(i)).map(|id| ItemStack::new(id, 1)),
            });
        }
        if let Some(BlockEntity::Chest(v)) = self.world.block_entities.get_mut(&(p.pos[0], p.pos[1], p.pos[2])) {
            *v = out;
        }
    }
    /// A terminal works when its device is active; a chest's own terminal when powered.
    fn ae2_term_active(&self, p: Loc, kind: WinKind) -> bool {
        match kind {
            WinKind::ChestTerm => {
                self.ae2_powered(p) || with_st(self, |s| s.devices.at(p).map(|d| d.fl("buffer") >= 1.0)).flatten().unwrap_or(false)
            }
            _ => self.ae2_kind_at(p).is_some_and(|k| self.ae2_device_active(p, k)),
        }
    }
    fn ae2_term_list(&mut self, p: Loc, kind: WinKind) -> BTreeMap<Key, u64> {
        if kind == WinKind::ChestTerm {
            return with_st(self, |s| s.devices.at(p).and_then(|d| d.cells.get(&0)).map(|c| c.items.iter().cloned().collect()))
                .flatten()
                .unwrap_or_default();
        }
        self.ae2_with_network(p, |live, _| engine::net_list(live)).unwrap_or_default()
    }
    /// Insert into what the terminal shows (MEStorageMenu: StorageHelper.poweredInsert).
    fn ae2_term_insert(&mut self, p: Loc, kind: WinKind, key: &Key, n: u64) -> u64 {
        if kind == WinKind::ChestTerm {
            return self.ae2_chest_op(p, |cell, ty, src| {
                let mut one = ChestCellMount { cell, ty };
                engine::powered_insert(src, &mut one, key, n, false)
            });
        }
        self.ae2_with_network(p, |live, power| engine::powered_insert(&mut Pool(power), live, key, n, false)).unwrap_or(0)
    }
    fn ae2_term_extract(&mut self, p: Loc, kind: WinKind, key: &Key, n: u64) -> u64 {
        if kind == WinKind::ChestTerm {
            return self.ae2_chest_op(p, |cell, ty, src| {
                let mut one = ChestCellMount { cell, ty };
                engine::powered_extract(src, &mut one, key, n, false)
            });
        }
        self.ae2_with_network(p, |live, power| engine::powered_extract(&mut Pool(power), live, key, n, false)).unwrap_or(0)
    }
    /// The ME chest's own cell with its energy (buffer, then network).
    fn ae2_chest_op(&mut self, p: Loc, f: impl FnOnce(&mut CellInv, &'static engine::CellType, &mut ChestSource<'_>) -> u64) -> u64 {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return 0 };
        let Some(ty) = slot_item(&d.slots, 0).and_then(|i| engine::cell_type(&i)) else { return 0 };
        let mut cell = d.cells.get(&0).cloned().unwrap_or_default();
        let mut buffer = engine::Buffer { stored: d.fl("buffer"), max: engine::config::CHEST_BUFFER };
        let n = self.ae2_net(p);
        let mut grid = n.and_then(|n| with_rt(|rt| rt.nets.get(n).map(|x| x.power.clone())).flatten()).unwrap_or_else(|| engine::GridPower::new(0.0, 0.0));
        let r = f(&mut cell, ty, &mut ChestSource { buffer: &mut buffer, grid: &mut grid });
        let stored = grid.stored;
        if let Some(n) = n {
            with_rt(|rt| {
                if let Some(x) = rt.nets.get_mut(n) {
                    x.power.stored = stored;
                }
            });
        }
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(p) {
                if cell.items.is_empty() {
                    dd.cells.remove(&0);
                } else {
                    dd.cells.insert(0, cell);
                }
                dd.set_fl("buffer", buffer.stored);
            }
        });
        r
    }

    // ---- buttons ----

    /// The window's buttons: (button, x, y, w, h, label, enabled).
    fn ae2_buttons(&self) -> Vec<(Btn, f32, f32, f32, f32, String, bool)> {
        let Some((p, kind, w, h, plan, cpu)) = with_rt(|rt| rt.window.as_ref().map(|w| (w.pos, w.kind, w.w, w.h, w.plan.clone(), w.cpu))).flatten() else {
            return Vec::new();
        };
        let d = with_st(self, |s| s.devices.at(p).cloned()).flatten().unwrap_or_default();
        let mut b = Vec::new();
        if kind.terminal() {
            let by = d.num("sort").clamp(0, 2) as usize;
            b.push((Btn::SortBy, -20.0, 8.0, 18.0, 18.0, ["N", "#", "@"][by].to_owned(), true));
            b.push((Btn::SortDir, -20.0, 28.0, 18.0, 18.0, if d.num("dir") == 0 { "\u{2191}" } else { "\u{2193}" }.to_owned(), true));
            if kind != WinKind::ChestTerm {
                b.push((Btn::Status, -20.0, 48.0, 18.0, 18.0, "C".to_owned(), true));
            }
        }
        match kind {
            WinKind::Encoding => {
                let mode = d.num("mode").clamp(0, 3);
                for (i, l) in ["C", "P", "S", "T"].iter().enumerate() {
                    let sel = mode == i as i64;
                    b.push((Btn::Mode(i as u8), 173.0, h - 170.0 + i as f32 * 20.0, 20.0, 20.0, if sel { format!("[{l}]") } else { (*l).to_owned() }, true));
                }
                b.push((Btn::Encode, 147.0, h - 142.0, 16.0, 16.0, "\u{2193}".to_owned(), true));
                b.push((Btn::Clear, 73.0, h - 164.0, 10.0, 10.0, "x".to_owned(), true));
                if mode == 0 || mode == 2 {
                    b.push((Btn::Substitute, 84.0, h - 164.0, 10.0, 10.0, if d.num("sub") != 0 { "S" } else { "s" }.to_owned(), true));
                }
                if mode == 0 {
                    b.push((Btn::FluidSub, 95.0, h - 164.0, 10.0, 10.0, if d.num("fsub") != 0 { "F" } else { "f" }.to_owned(), true));
                }
            }
            WinKind::Provider => {
                b.push((Btn::Blocking, -20.0, 8.0, 18.0, 18.0, if d.num("blocking") != 0 { "B" } else { "-" }.to_owned(), true));
            }
            WinKind::CraftAmount => {
                for (i, n) in [1i64, 10, 100, 1000].iter().enumerate() {
                    b.push((Btn::Add(*n), 20.0 + i as f32 * 35.0, 26.0, 33.0, 18.0, format!("+{n}"), true));
                    b.push((Btn::Add(-*n), 20.0 + i as f32 * 35.0, 76.0, 33.0, 18.0, format!("-{n}"), true));
                }
                b.push((Btn::Next, 120.0, 51.0, 38.0, 20.0, self.translate("gui.ae2.Next", "Next"), true));
                b.push((Btn::Back, 154.0, 2.0, 16.0, 14.0, "<".to_owned(), true));
            }
            WinKind::CraftConfirm => {
                let name = match cpu {
                    None => self.translate("gui.ae2.Automatic", "Automatic"),
                    Some(c) => format!("CPU {} {} {}", c[0], c[1], c[2]),
                };
                b.push((Btn::CpuCycle, 19.0, h - 68.0, 180.0, 20.0, fmt1(&self.translate("gui.ae2.SelectedCraftingCPU", "Crafting CPU: %s"), &name), true));
                let ok = plan.as_ref().is_some_and(|p| !p.simulation);
                b.push((Btn::Start, 162.0, h - 25.0, 50.0, 20.0, self.translate("gui.ae2.Start", "Start"), ok));
                b.push((Btn::Cancel, 6.0, h - 25.0, 50.0, 20.0, self.translate("gui.ae2.Cancel", "Cancel"), true));
            }
            WinKind::CraftCpu => {
                let name = cpu.map_or_else(|| "-".to_owned(), |c| format!("CPU {} {} {}", c[0], c[1], c[2]));
                b.push((Btn::CpuCycle, 8.0, h - 25.0, 150.0, 20.0, name, true));
                let busy = cpu.is_some_and(|c| self.ae2_cpu(c).job.is_some());
                b.push((Btn::CancelJob, 163.0, h - 25.0, 50.0, 20.0, self.translate("gui.ae2.Cancel", "Cancel"), busy));
            }
            WinKind::Emitter => {
                b.push((Btn::LowHigh, -20.0, 8.0, 18.0, 18.0, if d.num("low") != 0 { "<" } else { "\u{2265}" }.to_owned(), true));
                if device_cards(&d, Kind::LevelEmitter).iter().any(|c| c == cr::CRAFTING_CARD) {
                    b.push((Btn::CraftVia, -20.0, 28.0, 18.0, 18.0, if d.num("craft_redstone") != 0 { "R" } else { "-" }.to_owned(), true));
                }
                for (i, n) in [1i64, 10, 100, 1000].iter().enumerate() {
                    b.push((Btn::Add(*n), 20.0 + i as f32 * 26.0, 18.0, 24.0, 18.0, format!("+{n}"), true));
                    b.push((Btn::Add(-*n), 20.0 + i as f32 * 26.0, 60.0, 24.0, 18.0, format!("-{n}"), true));
                }
            }
            WinKind::Workbench => {
                b.push((Btn::Partition, -20.0, 8.0, 18.0, 18.0, "P".to_owned(), true));
                b.push((Btn::ClearCfg, -20.0, 28.0, 18.0, 18.0, "x".to_owned(), true));
            }
            _ => {}
        }
        let _ = w;
        b
    }
    fn ae2_button(&mut self, p: Loc, kind: WinKind, btn: Btn, right: bool) {
        self.audio.play_button_click();
        match btn {
            Btn::SortBy | Btn::SortDir => {
                with_st(self, |s| {
                    if let Some(d) = s.devices.at_mut(p) {
                        if btn == Btn::SortBy {
                            let next = engine::SortBy::from_code(d.num("sort")).next();
                            d.set_num("sort", next.code());
                        } else {
                            let v = 1 - d.num("dir");
                            d.set_num("dir", v);
                        }
                    }
                });
            }
            Btn::Status => {
                self.ae2_switch(WinKind::CraftCpu, |w| {
                    w.back = Some(kind);
                    w.cpu = None;
                });
                self.ae2_pick_cpu(None);
            }
            Btn::Mode(m) => {
                with_st(self, |s| {
                    if let Some(d) = s.devices.at_mut(p) {
                        d.set_num("mode", i64::from(m));
                    }
                });
                with_rt(|rt| {
                    if let Some(w) = rt.window.as_mut() {
                        w.page = 0;
                    }
                });
            }
            Btn::Encode => self.ae2_encode(p),
            Btn::Clear => {
                with_st(self, |s| {
                    if let Some(d) = s.devices.at_mut(p) {
                        d.config.iter_mut().for_each(|c| *c = None);
                    }
                });
            }
            Btn::Substitute | Btn::FluidSub | Btn::Blocking | Btn::LowHigh | Btn::CraftVia => {
                let name = match btn {
                    Btn::Substitute => "sub",
                    Btn::FluidSub => "fsub",
                    Btn::Blocking => "blocking",
                    Btn::LowHigh => "low",
                    _ => "craft_redstone",
                };
                with_st(self, |s| {
                    if let Some(d) = s.devices.at_mut(p) {
                        let v = 1 - d.num(name).clamp(0, 1);
                        d.set_num(name, v);
                    }
                });
                self.ae2_books_dirty();
            }
            Btn::Add(n) => {
                with_rt(|rt| {
                    if let Some(w) = rt.window.as_mut() {
                        let cur: i64 = w.amount.parse().unwrap_or(0);
                        let min = if w.kind == WinKind::CraftAmount { 1 } else { 0 };
                        let v = (cur.saturating_add(n)).max(min);
                        // NumberEntryWidget: the first +n from 1 gives n.
                        let v = if w.kind == WinKind::CraftAmount && cur == 1 && n > 1 { n } else { v };
                        w.amount = v.to_string();
                    }
                });
                if kind == WinKind::Emitter {
                    self.ae2_emitter_level();
                }
            }
            Btn::Next => self.ae2_craft_next(),
            Btn::Back | Btn::Cancel => {
                let back = with_rt(|rt| rt.window.as_ref().and_then(|w| w.back)).flatten().unwrap_or(WinKind::Terminal);
                self.ae2_switch(back, |w| {
                    w.plan = None;
                    w.craft = None;
                });
            }
            Btn::Start => self.ae2_craft_start(),
            Btn::CpuCycle => {
                let cpus = self.ae2_net(p).map(|n| self.ae2_cpus_of(n)).unwrap_or_default();
                let cores: Vec<Pos> = cpus.iter().map(|(c, _)| *c).collect();
                with_rt(|rt| {
                    if let Some(w) = rt.window.as_mut() {
                        let auto = w.kind == WinKind::CraftConfirm;
                        let mut options: Vec<Option<Pos>> = if auto { vec![None] } else { Vec::new() };
                        options.extend(cores.iter().map(|c| Some(*c)));
                        if !options.is_empty() {
                            let i = options.iter().position(|o| *o == w.cpu).unwrap_or(0);
                            let n = options.len();
                            w.cpu = options[if right { (i + n - 1) % n } else { (i + 1) % n }];
                        }
                    }
                });
            }
            Btn::CancelJob => {
                if let Some(core) = with_rt(|rt| rt.window.as_ref().and_then(|w| w.cpu)).flatten() {
                    let mut cpu = self.ae2_cpu(core);
                    cpu.cancel();
                    self.ae2_cpu_save(core, &cpu);
                    self.ae2_books_dirty();
                }
            }
            Btn::Partition | Btn::ClearCfg => {
                self.ae2_cell_edit(p, |cfg, cell| {
                    cfg.partition = if btn == Btn::Partition { cr::CellConfig::partition_from(&cell) } else { vec![None; cr::CELL_PARTITION] };
                });
            }
        }
        self.ae2_window_refresh();
    }

    // ---- crafting requests from the terminal ----

    fn ae2_craft_open(&mut self, key: Key) {
        let Some((_, kind)) = self.ae2_window() else { return };
        self.ae2_switch(WinKind::CraftAmount, |w| {
            w.back = Some(kind);
            w.craft = Some(key.clone());
            w.amount = "1".to_owned();
            w.shown = vec![Some((key, 1))];
        });
    }
    /// CraftAmountMenu: the plan for the amount (REPORT_MISSING_ITEMS), then the confirm dialog.
    fn ae2_craft_next(&mut self) {
        let Some((p, key, amount)) = with_rt(|rt| rt.window.as_ref().and_then(|w| Some((w.pos, w.craft.clone()?, w.amount.parse::<u64>().unwrap_or(1).max(1))))).flatten() else {
            return;
        };
        let Some(plan) = self.ae2_plan(p, &key, amount, cr::Strategy::ReportMissing) else { return };
        self.ae2_switch(WinKind::CraftConfirm, |w| {
            w.plan = Some(plan);
            w.cpu = None;
        });
    }
    /// CraftConfirmMenu.startJob.
    fn ae2_craft_start(&mut self) {
        let Some((p, plan, cpu, back)) = with_rt(|rt| rt.window.as_ref().and_then(|w| Some((w.pos, w.plan.clone()?, w.cpu, w.back)))).flatten() else {
            return;
        };
        match self.ae2_submit(p, &plan, cpu, None, true) {
            Ok(_) => {
                let back = back.unwrap_or(WinKind::Terminal);
                self.ae2_switch(back, |w| {
                    w.plan = None;
                    w.craft = None;
                });
            }
            Err(e) => {
                let msg = match e {
                    cr::SubmitError::IncompletePlan => self.translate("gui.ae2.CraftErrorIncompletePlan", "An incomplete plan cannot be submitted."),
                    cr::SubmitError::CpuBusy => self.translate("gui.ae2.CraftErrorCpuBusy", "The selected CPU is already working on another job."),
                    cr::SubmitError::CpuOffline => self.translate("gui.ae2.CraftErrorCpuOffline", "The selected CPU is offline."),
                    cr::SubmitError::CpuTooSmall => self.translate("gui.ae2.CraftErrorCpuTooSmall", "The selected CPU does not have enough storage."),
                    cr::SubmitError::MissingIngredient(..) => self.translate("gui.ae2.CraftErrorMissingIngredient", "Some ingredients could not be extracted from the network."),
                    cr::SubmitError::NoCpuFound => self.translate("gui.ae2.CraftErrorNoCpuFound", "There are no crafting CPUs on the network."),
                    cr::SubmitError::NoSuitableCpu { .. } => self.translate("gui.ae2.CraftErrorNoSuitableCpu", "None of the crafting CPUs are suitable for this job."),
                };
                with_rt(|rt| {
                    if let Some(w) = rt.window.as_mut() {
                        w.error = Some(msg);
                    }
                });
            }
        }
    }
    fn ae2_emitter_level(&mut self) {
        let Some((p, v)) = with_rt(|rt| rt.window.as_ref().map(|w| (w.pos, w.amount.parse::<i64>().unwrap_or(0).max(0)))).flatten() else { return };
        with_st(self, |s| {
            if let Some(d) = s.devices.at_mut(p) {
                d.set_num("level", v);
            }
        });
    }

    // ---- the pattern encoding terminal ----

    /// What the encoding area makes now: (recipe id, result) for crafting, smithing and the
    /// chosen stonecutting recipe; the stonecutter's choices.
    fn ae2_encoding_result(&self, d: &Device) -> (Option<(String, ItemStack)>, Vec<(String, ItemStack)>) {
        let key_stack = |i: usize| d.config.get(i).cloned().flatten().and_then(|(k, _)| stack_of(&k, 1));
        match cr::PatternKind::from_index(d.num("mode")) {
            cr::PatternKind::Crafting => {
                let grid: Vec<Option<ItemStack>> = (0..9).map(key_stack).collect();
                (self.res.recipes().find_with(&grid, 3, &|_| None).map(|(n, r)| (n.to_owned(), r)), Vec::new())
            }
            cr::PatternKind::Smithing => {
                let (Some(t), Some(b), Some(a)) = (key_stack(0), key_stack(1), key_stack(2)) else { return (None, Vec::new()) };
                let data = self.res.stations();
                let out = data.recipes.smithing_result(&data.enchantments, Some(&t), Some(&b), Some(&a));
                let id = self.ae2_smithing_recipe(&t, &b, &a).map(|(id, _)| id);
                (id.zip(out), Vec::new())
            }
            cr::PatternKind::Stonecutting => {
                let Some(input) = key_stack(0) else { return (None, Vec::new()) };
                let choices: Vec<(String, ItemStack)> =
                    self.res.stations().recipes.stonecutter(&input).into_iter().map(|r| (r.id.clone(), r.result.clone())).collect();
                let chosen = d.num("stonecut");
                let pick = (chosen > 0).then(|| choices.get(chosen as usize - 1).cloned()).flatten();
                (pick, choices)
            }
            cr::PatternKind::Processing => (None, Vec::new()),
        }
    }
    /// PatternEncodingTermMenu.encode.
    fn ae2_encode(&mut self, p: Loc) {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return };
        if d.slots.len() < 2 || d.config.len() < cr::MAX_PROCESSING_INPUTS + cr::MAX_PROCESSING_OUTPUTS {
            return;
        }
        let mode = cr::PatternKind::from_index(d.num("mode"));
        let encoded = match mode {
            cr::PatternKind::Processing => cr::encode_processing(&d.config[..cr::MAX_PROCESSING_INPUTS], &d.config[cr::MAX_PROCESSING_INPUTS..]),
            _ => {
                let (res, _) = self.ae2_encoding_result(&d);
                let inputs: Vec<Option<Key>> = (0..mode.input_slots()).map(|i| d.config.get(i).cloned().flatten().map(|(k, _)| k)).collect();
                let (recipe, out) = res.map(|(id, s)| (id, Some((key_of(&s), u64::from(s.count))))).unwrap_or_default();
                cr::encode_recipe(mode, &inputs, &recipe, out, d.num("sub") != 0, d.num("fsub") != 0)
            }
        };
        let blank = d.slots[0].as_ref().map_or(0, |(_, n)| *n);
        let out_item = slot_item(&d.slots, 1);
        let Some(e) = encoded else {
            // clearPattern: an encoded pattern in the output becomes a blank one.
            if out_item.as_deref().is_some_and(cr::is_encoded_pattern) {
                let k = plain_key(cr::BLANK_PATTERN);
                with_st(self, |s| {
                    if let Some(dd) = s.devices.at_mut(p) {
                        let n = dd.slots[1].as_ref().map_or(1, |(_, n)| *n);
                        dd.slots[1] = Some((k, n));
                    }
                });
            }
            return;
        };
        let Some(blank_left) = cr::encode_slots(blank, out_item.as_deref()) else { return };
        let k = key_of(&pattern_stack(&e));
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(p) {
                dd.slots[0] = (blank_left > 0).then(|| (dd.slots[0].as_ref().map(|(k, _)| k.clone()).unwrap_or_default(), blank_left)).filter(|(k, _)| !k.is_empty());
                dd.slots[1] = Some((k, 1));
            }
        });
    }
    /// An encoded pattern put into the output slot loads into the encoding area.
    fn ae2_load_pattern(&mut self, p: Loc) {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return };
        let Some(e) = d.slots.get(1).cloned().flatten().and_then(|(k, _)| stack_of(&k, 1)).and_then(|s| pattern_of(&s)) else { return };
        with_st(self, |s| {
            let Some(dd) = s.devices.at_mut(p) else { return };
            dd.config = vec![None; cr::MAX_PROCESSING_INPUTS + cr::MAX_PROCESSING_OUTPUTS];
            for (i, x) in e.inputs.iter().enumerate().take(cr::MAX_PROCESSING_INPUTS) {
                dd.config[i] = x.clone();
            }
            if e.kind == cr::PatternKind::Processing {
                for (i, x) in e.outputs.iter().enumerate().take(cr::MAX_PROCESSING_OUTPUTS) {
                    dd.config[cr::MAX_PROCESSING_INPUTS + i] = x.clone();
                }
            }
            dd.set_num("mode", e.kind.index());
            dd.set_num("sub", i64::from(e.substitute));
            dd.set_num("fsub", i64::from(e.fluid_substitute));
        });
    }

    // ---- the cell workbench ----

    /// Edits the config of the cell in the workbench (written back into the cell item).
    fn ae2_cell_edit(&mut self, p: Loc, f: impl FnOnce(&mut cr::CellConfig, Vec<(Key, u64)>)) {
        let Some((k, n)) = with_st(self, |s| s.devices.at(p).and_then(|d| d.slots.first().cloned().flatten())).flatten() else { return };
        let Some(mut st) = stack_of(&k, n) else { return };
        let mut cfg = cell_config(&st);
        if cfg.partition.len() < cr::CELL_PARTITION {
            cfg.partition.resize(cr::CELL_PARTITION, None);
        }
        let contents = cell_text(&st).map(|t| CellInv::decode(&t).items).unwrap_or_default();
        f(&mut cfg, contents);
        let text = (!cfg.is_empty()).then(|| cfg.encode());
        set_custom_text(&mut st, "ae2:cfg", text.as_deref());
        let nk = key_of(&st);
        with_st(self, |s| {
            if let Some(d) = s.devices.at_mut(p) {
                d.slots[0] = Some((nk, n));
            }
        });
    }

    // ---- clicks ----

    /// HEAD `sort_mouse_press` (every press on a screen, before the slot clicks).
    pub(super) fn ae2_mouse(&mut self, button: winit::event::MouseButton) -> bool {
        if self.remote.is_some() {
            if !self.ae2_guest_open() {
                return false;
            }
            let input = super::foundry::mouse_input(self, button, Vec::new());
            return self.ae2_guest_input(input, |g| g.ae2_mouse_at(button));
        }
        self.ae2_mouse_at(button)
    }
    /// The press itself (the host's window, or the server replaying a guest's press).
    fn ae2_mouse_at(&mut self, button: winit::event::MouseButton) -> bool {
        use winit::event::MouseButton;
        let Some((p, kind)) = self.ae2_window_open() else { return false };
        let (right, middle) = match button {
            MouseButton::Left => (false, false),
            MouseButton::Right => (true, false),
            MouseButton::Middle => (false, true),
            _ => return false,
        };
        let Some(batch) = super::foundry::hud_batch(self) else { return false };
        let (ox, oy) = self.inventory_origin(&batch);
        let (mx, my) = (self.cursor.0 - ox, self.cursor.1 - oy);
        let shift = self.has_shift_down();
        let inside = |x: f32, y: f32, w: f32, h: f32| mx >= x && mx < x + w && my >= y && my < y + h;
        let rows = with_rt(|rt| rt.window.as_ref().map(|w| w.rows)).flatten().unwrap_or(0);
        if !middle {
            for (btn, x, y, w, h, _, enabled) in self.ae2_buttons() {
                if inside(x, y, w, h) {
                    if enabled {
                        self.ae2_button(p, kind, btn, right);
                    }
                    return true;
                }
            }
        }
        if kind.terminal() && !middle {
            // The search box (base_terminal.json: left 80, top 4, 89 x 12): right click clears.
            if inside(80.0, 4.0, 89.0, 12.0) {
                with_rt(|rt| {
                    if let Some(w) = rt.window.as_mut() {
                        w.focused = true;
                        if right {
                            w.search.clear();
                            w.scroll = 0;
                        }
                    }
                });
                self.ae2_window_refresh();
                return true;
            }
            with_rt(|rt| {
                if let Some(w) = rt.window.as_mut() {
                    w.focused = false;
                }
            });
            // The scrollbar (left 175, top 18).
            if inside(175.0, 18.0, 12.0, rows as f32 * 18.0) {
                with_rt(|rt| {
                    if let Some(w) = rt.window.as_mut() {
                        let max = w.max_scroll();
                        let f = ((my - 18.0) / (rows as f32 * 18.0)).clamp(0.0, 1.0);
                        w.scroll = (f * max as f32).round() as usize;
                    }
                });
                self.ae2_window_fill();
                return true;
            }
        }
        if kind.number() && !middle {
            // The number field takes the keys while focused.
            let field = if kind == WinKind::CraftAmount { (48.0, 55.0, 66.0, 12.0) } else { (23.0, 42.0, 103.0, 12.0) };
            let hit = inside(field.0, field.1, field.2, field.3);
            with_rt(|rt| {
                if let Some(w) = rt.window.as_mut() {
                    w.focused = hit || w.kind == WinKind::CraftAmount;
                    if hit && right {
                        w.amount.clear();
                    }
                }
            });
            if hit {
                return true;
            }
        }
        // Priority (drives, chests, storage buses, providers): clicks on the title line.
        if matches!(kind, WinKind::Drive | WinKind::ChestCell | WinKind::StorageBus | WinKind::Provider) && inside(100.0, 2.0, 74.0, 12.0) {
            let step = if shift { 10 } else { 1 } * if right { -1 } else { 1 };
            with_st(self, |s| {
                if let Some(d) = s.devices.at_mut(p) {
                    let v = d.num("priority") + step;
                    d.set_num("priority", v);
                }
            });
            self.ae2_books_dirty();
            self.audio.play_button_click();
            return true;
        }
        let (slot, _) = self.inventory_target();
        match slot {
            Some(s) if (300..500).contains(&s) => {
                let which = with_rt(|rt| rt.window.as_ref().and_then(|w| w.slots.get(s - 300).map(|x| x.0))).flatten();
                if let Some(which) = which {
                    if middle {
                        // Middle click on a craftable entry: craft (MEStorageScreen, CLONE).
                        if let Slot::Grid(i) = which
                            && let Some((k, _)) = with_rt(|rt| rt.window.as_ref().and_then(|w| w.view.get(w.scroll * 9 + i).cloned())).flatten()
                            && with_rt(|rt| rt.window.as_ref().is_some_and(|w| w.craftable.contains(&k))) == Some(true)
                        {
                            self.ae2_craft_open(k);
                        }
                        return true;
                    }
                    self.ae2_slot_click(p, kind, which, right, shift);
                    self.ae2_window_refresh();
                }
                true
            }
            _ if middle || kind.no_inventory() => true,
            Some(s) if s < 36 && shift => {
                self.ae2_quick_move(p, kind, s);
                self.ae2_window_refresh();
                true
            }
            // A click into the empty part of the grid puts the carried stack in.
            None if kind.terminal() && self.carried.is_some() && inside(7.0, 17.0, 162.0, rows as f32 * 18.0) => {
                self.ae2_slot_click(p, kind, Slot::Grid(usize::MAX), right, shift);
                self.ae2_window_refresh();
                true
            }
            _ => false,
        }
    }
    /// A shift click on the player's inventory: into the network (terminals) or the device.
    fn ae2_quick_move(&mut self, p: Loc, kind: WinKind, slot: usize) {
        let Some(stack) = self.inventory[slot].clone() else { return };
        let now = std::time::Instant::now();
        let double = with_rt(|rt| {
            let w = rt.window.as_mut()?;
            let d = w.last_shift.is_some_and(|(t, s)| s == slot && now.duration_since(t).as_millis() < 250);
            w.last_shift = Some((now, slot));
            Some(d)
        })
        .flatten()
        .unwrap_or(false);
        if kind == WinKind::Encoding && &*stack.item == cr::BLANK_PATTERN {
            // Blank patterns go to their slot first.
            let rest = self.ae2_real_put(p, kind, 0, stack, u32::MAX);
            self.inventory[slot] = rest;
            return;
        }
        if kind.terminal() {
            // A double shift click moves every stack of the same item (MEStorageScreen).
            let slots: Vec<usize> = if double {
                (0..36).filter(|&i| self.inventory[i].as_ref().is_some_and(|s| s.same_item_same_components(&stack))).collect()
            } else {
                vec![slot]
            };
            for i in slots {
                let Some(s) = self.inventory[i].clone() else { continue };
                let k = key_of(&s);
                let n = self.ae2_term_insert(p, kind, &k, u64::from(s.count));
                let left = s.count - n as u32;
                self.inventory[i] = (left > 0).then(|| ItemStack { count: left, ..s });
            }
            return;
        }
        if kind == WinKind::Access {
            // The first free pattern slot shown.
            let remote = with_rt(|rt| rt.window.as_ref().map(|w| w.remote.clone())).flatten().unwrap_or_default();
            if cr::is_encoded_pattern(&stack.item) {
                for (l, s) in remote {
                    if with_st(self, |st| st.devices.at(l).is_some_and(|d| d.slots.get(s).is_some_and(Option::is_none))) == Some(true) {
                        let one = ItemStack { count: 1, ..stack.clone() };
                        with_st(self, |st| {
                            if let Some(d) = st.devices.at_mut(l) {
                                d.slots[s] = Some((key_of(&one), 1));
                            }
                        });
                        self.inventory[slot] = (stack.count > 1).then(|| ItemStack { count: stack.count - 1, ..stack.clone() });
                        self.ae2_books_dirty();
                        return;
                    }
                }
            }
            return;
        }
        if kind == WinKind::Workbench && self.ae2_real_rule(p, kind, 0, &stack) == 0 {
            // A card into the cell.
            if self.ae2_cell_card(p, &stack) {
                self.inventory[slot] = (stack.count > 1).then(|| ItemStack { count: stack.count - 1, ..stack.clone() });
            }
            return;
        }
        let mut reals: Vec<usize> = with_rt(|rt| rt.window.as_ref().map(|w| w.slots.iter().filter_map(|(s, _, _)| if let Slot::Real(i) = s { Some(*i) } else { None }).collect()))
            .flatten()
            .unwrap_or_default();
        // An upgrade card goes to the upgrade slots first.
        if let Some((_, range)) = self.ae2_kind_at(p).and_then(Kind::upgrades)
            && stack.item.ends_with("_card")
        {
            reals.sort_by_key(|i| !range.contains(i));
        }
        let mut rest = Some(stack);
        for i in reals {
            let Some(r) = rest.take() else { break };
            rest = self.ae2_real_put(p, kind, i, r, u32::MAX);
        }
        self.inventory[slot] = rest;
    }
    /// A card into the workbench's cell (cell upgrades), when the cell takes it.
    fn ae2_cell_card(&mut self, p: Loc, card: &ItemStack) -> bool {
        let mut ok = false;
        self.ae2_cell_edit(p, |cfg, _| {
            if cfg.upgrades.len() < cr::upgrade_slots(cr::Host::ItemCell) && cr::may_install(cr::Host::ItemCell, &cfg.upgrades, &card.item) {
                cfg.upgrades.push(card.item.to_string());
                ok = true;
            }
        });
        ok
    }
    /// What a real slot may hold and how many.
    fn ae2_real_rule(&self, p: Loc, kind: WinKind, i: usize, s: &ItemStack) -> u32 {
        let tags = item_tags(self);
        let max = self.ae2_max_stack(&s.item);
        let card = |host: cr::Host, range: std::ops::Range<usize>| -> u32 {
            let others: Vec<String> = with_st(self, |st| {
                st.devices.at(p).map(|d| range.clone().filter(|&j| j != i).filter_map(|j| slot_item(&d.slots, j)).collect())
            })
            .flatten()
            .unwrap_or_default();
            u32::from(cr::may_install(host, &others, &s.item))
        };
        match kind {
            WinKind::Drive | WinKind::ChestCell => u32::from(engine::cell_type(&s.item).is_some()),
            WinKind::Inscriber => match i {
                0 | 1 if engine::inscriber_plate_ok(&s.item, &tags) => 1,
                2 if engine::inscriber_middle_ok(&s.item, &tags) => max,
                _ => 0,
            },
            WinKind::Vibration => {
                if self.res.providers().burn_time(self.res.items(), s, false) > 0 { max } else { 0 }
            }
            WinKind::Interface if i == 9 => card(cr::Host::Interface, 9..10),
            WinKind::ImportBus => card(cr::Host::ImportBus, 0..5),
            WinKind::ExportBus => card(cr::Host::ExportBus, 0..5),
            WinKind::StorageBus => card(cr::Host::StorageBus, 0..5),
            WinKind::Emitter => card(cr::Host::LevelEmitter, 0..1),
            WinKind::Assembler => match i {
                0..=8 => u32::from(self.ae2_assembler_plan(p).is_some_and(|d| d.grid_valid(i, Some(&key_of(s)), &item_id))),
                10 => u32::from(cr::is_encoded_pattern(&s.item) && pattern_of(s).is_some_and(|e| e.kind != cr::PatternKind::Processing)),
                11..16 => card(cr::Host::Assembler, 11..16),
                _ => 0,
            },
            WinKind::Provider => u32::from(i < cr::PROVIDER_PATTERNS && cr::is_encoded_pattern(&s.item)),
            WinKind::Encoding => match i {
                0 if &*s.item == cr::BLANK_PATTERN => max,
                1 if &*s.item == cr::BLANK_PATTERN || cr::is_encoded_pattern(&s.item) => 1,
                _ => 0,
            },
            WinKind::Workbench => u32::from(i == 0 && engine::cell_type(&s.item).is_some()),
            _ => max,
        }
    }
    /// Puts up to `n` of `s` into real slot `i`; returns what is left.
    fn ae2_real_put(&mut self, p: Loc, kind: WinKind, i: usize, s: ItemStack, n: u32) -> Option<ItemStack> {
        let limit = self.ae2_real_rule(p, kind, i, &s);
        if limit == 0 {
            return Some(s);
        }
        let cell = matches!(kind, WinKind::Drive | WinKind::ChestCell);
        let (key, inv) = if cell { let (k, inv) = split_cell(&ItemStack { count: 1, ..s.clone() }); (k, Some(inv)) } else { (key_of(&s), None) };
        let put = with_st(self, |st| {
            let d = st.devices.at_mut(p)?;
            let slot = d.slots.get_mut(i)?;
            let room = match slot {
                None => limit,
                Some((k, c)) if *k == key && !cell => limit.saturating_sub(*c),
                _ => 0,
            };
            let m = room.min(n).min(s.count);
            if m == 0 {
                return Some(0);
            }
            match slot {
                Some((_, c)) => *c += m,
                None => *slot = Some((key.clone(), m)),
            }
            if let Some(inv) = &inv
                && !inv.items.is_empty()
            {
                d.cells.insert(i, inv.clone());
            }
            Some(m)
        })
        .flatten()
        .unwrap_or(0);
        if put > 0 {
            with_rt(|rt| {
                if let Some(t) = rt.tickers.get_mut(&p) {
                    t.wake(rt.tick);
                }
            });
            self.ae2_slot_changed(p, kind, i);
        }
        (s.count > put).then(|| ItemStack { count: s.count - put, ..s })
    }
    /// After a real slot changed: patterns invalidate the network's book, an encoded pattern
    /// loads into the encoding area.
    fn ae2_slot_changed(&mut self, p: Loc, kind: WinKind, i: usize) {
        match kind {
            WinKind::Provider | WinKind::Emitter => self.ae2_books_dirty(),
            WinKind::Encoding if i == 1 => self.ae2_load_pattern(p),
            _ => {}
        }
    }
    /// Takes up to `n` from real slot `i` (a cell comes out with its contents).
    fn ae2_real_take(&mut self, p: Loc, i: usize, n: u32) -> Option<ItemStack> {
        let (k, got, inv) = with_st(self, |st| {
            let d = st.devices.at_mut(p)?;
            let (k, c) = d.slots.get_mut(i)?.as_mut()?;
            let m = (*c).min(n);
            let key = k.clone();
            *c -= m;
            if *c == 0 {
                d.slots[i] = None;
            }
            let inv = if d.slots[i].is_none() { d.cells.remove(&i) } else { None };
            Some((key, m, inv))
        })
        .flatten()?;
        let mut s = match inv {
            Some(inv) => join_cell(&k, Some(&inv))?,
            None => stack_of(&k, 1)?,
        };
        s.count = got;
        if let Some((_, kind)) = self.ae2_window()
            && matches!(kind, WinKind::Provider | WinKind::Emitter)
        {
            self.ae2_books_dirty();
        }
        Some(s)
    }
    fn ae2_slot_click(&mut self, p: Loc, kind: WinKind, slot: Slot, right: bool, shift: bool) {
        match slot {
            Slot::Grid(i) => {
                if !with_rt(|rt| rt.window.as_ref().is_some_and(|w| w.active)).unwrap_or(false) {
                    return;
                }
                if let Some(c) = self.carried.take() {
                    // Insert the carried stack (left) or one of it (right).
                    let k = key_of(&c);
                    let n = if right { 1 } else { u64::from(c.count) };
                    let took = self.ae2_term_insert(p, kind, &k, n) as u32;
                    let left = c.count - took;
                    self.carried = (left > 0).then(|| ItemStack { count: left, ..c });
                    self.ae2_window_refresh();
                    return;
                }
                let entry = with_rt(|rt| rt.window.as_ref().and_then(|w| w.view.get(w.scroll * 9 + i).cloned())).flatten();
                let Some((k, amount)) = entry else { return };
                // Nothing stored but craftable: the craft amount dialog.
                if amount == 0 {
                    if with_rt(|rt| rt.window.as_ref().is_some_and(|w| w.craftable.contains(&k))) == Some(true) {
                        self.ae2_craft_open(k);
                    }
                    return;
                }
                let Some(proto) = stack_of(&k, 1) else { return };
                let max = u64::from(self.ae2_max_stack(&proto.item));
                let want = if right && !shift { (amount.min(max) + 1) / 2 } else { amount.min(max) };
                let got = self.ae2_term_extract(p, kind, &k, want) as u32;
                if got == 0 {
                    return;
                }
                let s = ItemStack { count: got, ..proto };
                if shift {
                    // SHIFT_CLICK: into the inventory; what does not fit goes back.
                    if let Some(rest) = self.give(s) {
                        let back = self.ae2_term_insert(p, kind, &k, u64::from(rest.count)) as u32;
                        if back < rest.count {
                            let left = ItemStack { count: rest.count - back, ..rest };
                            self.ae2_give(left);
                        }
                    }
                } else {
                    self.carried = Some(s);
                }
                self.ae2_window_refresh();
            }
            Slot::Ghost(i) => {
                // The ghost amount: the carried stack's size (left) or one (right); processing
                // patterns keep it, the other slots hold one.
                let processing = kind == WinKind::Encoding && with_st(self, |s| s.devices.at(p).map(|d| d.num("mode"))).flatten() == Some(1);
                let set = self.carried.as_ref().map(|c| {
                    let n = if right { 1 } else { u64::from(c.count) };
                    let n = match kind {
                        WinKind::Interface => n,
                        WinKind::Encoding if processing => n,
                        _ => 1,
                    };
                    (key_of(&ItemStack { count: 1, ..c.clone() }), n)
                });
                with_st(self, |s| {
                    if let Some(d) = s.devices.at_mut(p)
                        && let Some(c) = d.config.get_mut(i)
                    {
                        *c = set;
                        if kind == WinKind::Encoding && i == 0 {
                            d.set_num("stonecut", 0);
                        }
                    }
                });
                if kind == WinKind::Emitter {
                    self.ae2_books_dirty();
                }
                with_rt(|rt| {
                    if let Some(t) = rt.tickers.get_mut(&p) {
                        t.wake(rt.tick);
                    }
                });
            }
            Slot::Show(i) => {
                // The stonecutter's choices pick the recipe.
                if kind == WinKind::Encoding {
                    with_st(self, |s| {
                        if let Some(d) = s.devices.at_mut(p) {
                            d.set_num("stonecut", i as i64 + 1);
                        }
                    });
                }
            }
            Slot::Remote(i) => {
                let Some((l, s)) = with_rt(|rt| rt.window.as_ref().and_then(|w| w.remote.get(i).copied())).flatten() else { return };
                let have = with_st(self, |st| st.devices.at(l).and_then(|d| d.slots.get(s).cloned().flatten())).flatten();
                match (self.carried.take(), have) {
                    (None, Some(_)) => {
                        let got = self.ae2_real_take(l, s, 1);
                        if shift {
                            if let Some(g) = got {
                                self.ae2_give(g);
                            }
                        } else {
                            self.carried = got;
                        }
                    }
                    (Some(c), None) if cr::is_encoded_pattern(&c.item) => {
                        let one = ItemStack { count: 1, ..c.clone() };
                        with_st(self, |st| {
                            if let Some(d) = st.devices.at_mut(l) {
                                d.slots[s] = Some((key_of(&one), 1));
                            }
                        });
                        self.carried = (c.count > 1).then(|| ItemStack { count: c.count - 1, ..c });
                    }
                    (c, _) => self.carried = c,
                }
                self.ae2_books_dirty();
            }
            Slot::CellCfg(i) => {
                let set = self.carried.as_ref().map(|c| key_of(&ItemStack { count: 1, ..c.clone() }));
                self.ae2_cell_edit(p, |cfg, _| {
                    if let Some(x) = cfg.partition.get_mut(i) {
                        *x = set;
                    }
                });
            }
            Slot::CellUpg(i) => match self.carried.take() {
                None => {
                    let mut out = None;
                    self.ae2_cell_edit(p, |cfg, _| {
                        if i < cfg.upgrades.len() {
                            out = Some(cfg.upgrades.remove(i));
                        }
                    });
                    self.carried = out.map(|id| ItemStack::new(&id, 1));
                }
                Some(c) => {
                    let ok = self.ae2_cell_card(p, &c);
                    self.carried = if ok { (c.count > 1).then(|| ItemStack { count: c.count - 1, ..c }) } else { Some(c) };
                }
            },
            Slot::Real(i) => {
                // The assembler's output and grid, the provider's return slots: taken only.
                let take_only = matches!((kind, i), (WinKind::Assembler, 0..10) | (WinKind::Provider, 9..18));
                if shift {
                    if kind == WinKind::Crafting
                        && let Some(s) = self.ae2_real_take(p, i, u32::MAX)
                    {
                        // A crafting grid slot goes back to the network first.
                        let k = key_of(&s);
                        let took = self.ae2_term_insert(p, kind, &k, u64::from(s.count)) as u32;
                        if took < s.count {
                            self.ae2_give(ItemStack { count: s.count - took, ..s });
                        }
                    } else if let Some(s) = self.ae2_real_take(p, i, u32::MAX)
                        && let Some(rest) = self.give(s)
                    {
                        let back = if take_only { Some(rest) } else { self.ae2_real_put(p, kind, i, rest, u32::MAX) };
                        if let Some(b) = back {
                            self.ae2_give(b);
                        }
                    }
                    self.ae2_slot_changed(p, kind, i);
                    return;
                }
                match self.carried.take() {
                    None => {
                        let have = with_st(self, |s| s.devices.at(p).and_then(|d| d.slots.get(i).cloned().flatten()).map(|x| x.1)).flatten();
                        if let Some(n) = have {
                            let take = if right { n.div_ceil(2) } else { n };
                            self.carried = self.ae2_real_take(p, i, take);
                            self.ae2_slot_changed(p, kind, i);
                        }
                    }
                    Some(c) if take_only => {
                        // Stack the slot's items onto the carried ones when they match.
                        let have = with_st(self, |s| s.devices.at(p).and_then(|d| d.slots.get(i).cloned().flatten())).flatten();
                        if let Some((k, n)) = have
                            && k == key_of(&c)
                        {
                            let room = self.ae2_max_stack(&c.item).saturating_sub(c.count).min(n);
                            if let Some(t) = self.ae2_real_take(p, i, room) {
                                self.carried = Some(ItemStack { count: c.count + t.count, ..c });
                                return;
                            }
                        }
                        self.carried = Some(c);
                    }
                    Some(c) => {
                        let rest = self.ae2_real_put(p, kind, i, c.clone(), if right { 1 } else { u32::MAX });
                        match rest {
                            Some(r) if r.count == c.count && !right => {
                                // Nothing went in: swap with the slot when allowed.
                                let rule = self.ae2_real_rule(p, kind, i, &c);
                                if rule > 0 && c.count <= rule {
                                    let old = self.ae2_real_take(p, i, u32::MAX);
                                    if old.is_some() {
                                        let back = self.ae2_real_put(p, kind, i, c, u32::MAX);
                                        self.carried = old;
                                        if let Some(b) = back {
                                            self.ae2_give(b);
                                        }
                                        return;
                                    }
                                }
                                self.carried = Some(r);
                            }
                            other => self.carried = other,
                        }
                    }
                }
            }
            Slot::Result => self.ae2_craft_click(p, shift),
        }
    }

    // ---- the crafting terminal ----

    fn ae2_craft_result(&self, d: &Device) -> Option<(String, ItemStack)> {
        let grid: Vec<Option<ItemStack>> = (0..9).map(|i| d.slots.get(i).and_then(|x| x.as_ref()).and_then(|(k, n)| stack_of(k, *n))).collect();
        self.res.recipes().find_with(&grid, 3, &|_| None).map(|(n, r)| (n.to_owned(), r))
    }
    /// CraftingTermSlot.craftItem: every ingredient comes from the network when it has the
    /// same item, else from the grid; remainders stay in the grid or go to the player.
    fn ae2_craft_once(&mut self, p: Loc) -> Option<ItemStack> {
        let d = with_st(self, |s| s.devices.at(p).cloned()).flatten()?;
        let (name, result) = self.ae2_craft_result(&d)?;
        let grid: Vec<Option<ItemStack>> = (0..9).map(|i| d.slots.get(i).and_then(|x| x.as_ref()).and_then(|(k, n)| stack_of(k, *n))).collect();
        let remainders = self.res.recipes().remainders(&name, &grid, 3);
        let active = self.ae2_term_active(p, WinKind::Crafting);
        let mut slots = d.slots.clone();
        for i in 0..9 {
            let Some((k, _)) = slots[i].clone() else { continue };
            let from_net = active && self.ae2_term_extract(p, WinKind::Crafting, &k, 1) == 1;
            if !from_net
                && let Some((_, n)) = slots[i].as_mut()
            {
                *n -= 1;
                if *n == 0 {
                    slots[i] = None;
                }
            }
        }
        let mut give = Vec::new();
        for (i, r) in remainders.into_iter().enumerate() {
            let Some(r) = r else { continue };
            if slots[i].is_none() {
                slots[i] = Some((key_of(&r), r.count));
            } else {
                give.push(r);
            }
        }
        with_st(self, |s| {
            if let Some(dd) = s.devices.at_mut(p) {
                dd.slots = slots;
            }
        });
        for g in give {
            self.ae2_give(g);
        }
        Some(result)
    }
    fn ae2_craft_click(&mut self, p: Loc, shift: bool) {
        let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return };
        let Some((_, first)) = self.ae2_craft_result(&d) else { return };
        let max = self.ae2_max_stack(&first.item);
        if shift {
            // CRAFT_SHIFT: floor(maxStack / perCraft) times, while the result stays the same.
            let times = (max / first.count.max(1)).max(1);
            for _ in 0..times {
                let Some(d) = with_st(self, |s| s.devices.at(p).cloned()).flatten() else { return };
                match self.ae2_craft_result(&d) {
                    Some((_, r)) if r.same_item_same_components(&first) => {}
                    _ => break,
                }
                // Stop when the inventory has no room for one more result.
                let room: u32 = self.inventory.iter().map(|s| match s {
                    None => max,
                    Some(x) if x.same_item_same_components(&first) => max.saturating_sub(x.count),
                    _ => 0,
                }).sum();
                if room < first.count {
                    break;
                }
                if let Some(out) = self.ae2_craft_once(p) {
                    self.ae2_give(out);
                }
            }
        } else {
            match self.carried.clone() {
                None => self.carried = self.ae2_craft_once(p),
                Some(c) if c.same_item_same_components(&first) && c.count + first.count <= max => {
                    if let Some(out) = self.ae2_craft_once(p) {
                        self.carried = Some(ItemStack { count: c.count + out.count, ..c });
                    }
                }
                _ => {}
            }
        }
        self.ae2_window_refresh();
    }

    // ---- menu ops: nothing the vanilla menu would do may touch the window's slots ----

    /// HEAD `menu_ops`: clicks the game routes to the menu engine. A click on a window slot,
    /// a quick move or a collect-all is ours; plain clicks on the inventory stay vanilla.
    pub(super) fn ae2_menu_ops(&mut self, ops: &[(Option<usize>, i32, Input)]) -> Option<bool> {
        let (p, kind) = self.ae2_window_open()?;
        let ours = ops.iter().any(|&(s, _, i)| s.is_some_and(|s| s >= 300) || matches!(i, Input::QuickMove | Input::PickupAll));
        if !ours {
            return None;
        }
        if let [(Some(s), _, Input::PickupAll)] = ops
            && *s < 36
        {
            // Double click: collect from the inventory only (never from the window).
            if let Some(c) = self.carried.as_mut() {
                let max = self.res.items().get(&c.item).map_or(64, |i| i.max_stack);
                for i in 0..36 {
                    if c.count >= max {
                        break;
                    }
                    if let Some(x) = self.inventory[i].as_mut()
                        && x.same_item_same_components(c)
                    {
                        let n = x.count.min(max - c.count);
                        x.count -= n;
                        c.count += n;
                        if x.count == 0 {
                            self.inventory[i] = None;
                        }
                    }
                }
            }
            return Some(true);
        }
        if let [(Some(s), _, Input::QuickMove)] = ops
            && *s < 36
        {
            self.ae2_quick_move(p, kind, *s);
            self.ae2_window_refresh();
        }
        Some(true)
    }

    // ---- typing in the search box / number fields (the anvil's rename-box path) ----

    pub(super) fn ae2_typing(&self) -> bool {
        self.ae2_window_open().is_some() && with_rt(|rt| rt.window.as_ref().is_some_and(|w| w.focused)) == Some(true)
    }
    pub(super) fn ae2_key(&mut self, key: winit::keyboard::KeyCode) -> Option<bool> {
        if self.remote.is_some() {
            if !self.ae2_guest_open() {
                return None;
            }
            let Some(code) = super::foundry::key_code(key) else { return self.ae2_key_at(key) };
            let input = super::foundry::MenuInput::Key { key: code, shift: self.has_shift_down() };
            let mut r = None;
            self.ae2_guest_input(input, |g| {
                r = g.ae2_key_at(key);
                r == Some(true)
            });
            return r;
        }
        self.ae2_key_at(key)
    }
    fn ae2_key_at(&mut self, key: winit::keyboard::KeyCode) -> Option<bool> {
        use winit::keyboard::KeyCode;
        if !self.ae2_typing() {
            return None;
        }
        if matches!(key, KeyCode::Enter | KeyCode::NumpadEnter) {
            let kind = self.ae2_window().map(|(_, k)| k);
            if kind == Some(WinKind::CraftAmount) {
                self.ae2_craft_next();
                return Some(true);
            }
            with_rt(|rt| {
                if let Some(w) = rt.window.as_mut() {
                    w.focused = false;
                }
            });
            return Some(true);
        }
        Some(false)
    }
    pub(super) fn ae2_text(&mut self, text: Option<&str>, backspace: bool) -> bool {
        if self.remote.is_some() {
            if !self.ae2_guest_open() {
                return false;
            }
            let input = super::foundry::MenuInput::Text { text: text.map(str::to_owned), backspace };
            return self.ae2_guest_input(input, |g| g.ae2_text_at(text, backspace));
        }
        self.ae2_text_at(text, backspace)
    }
    fn ae2_text_at(&mut self, text: Option<&str>, backspace: bool) -> bool {
        if !self.ae2_typing() {
            return false;
        }
        let kind = self.ae2_window().map(|(_, k)| k);
        let number = kind.is_some_and(WinKind::number);
        with_rt(|rt| {
            if let Some(w) = rt.window.as_mut() {
                let field = if number { &mut w.amount } else { &mut w.search };
                if backspace {
                    field.pop();
                } else if let Some(t) = text {
                    field.extend(t.chars().filter(|c| !c.is_control() && (!number || c.is_ascii_digit())));
                    field.truncate(if number { 12 } else { 256 });
                }
                if !number {
                    w.scroll = 0;
                }
            }
        });
        if kind == Some(WinKind::Emitter) {
            self.ae2_emitter_level();
        }
        self.ae2_window_refresh();
        true
    }
    /// HEAD `tweak_scroll`: the wheel scrolls grids, tables and the processing slots; over an
    /// interface stock slot or a processing slot it changes the amount.
    pub(super) fn ae2_scroll(&mut self, delta: f32, slot: Option<usize>) -> Option<bool> {
        if self.remote.is_some() {
            if !self.ae2_guest_open() {
                return None;
            }
            let input = super::foundry::scroll_input(self, delta, slot);
            let mut r = None;
            self.ae2_guest_input(input, |g| {
                r = g.ae2_scroll_at(delta, slot);
                r == Some(true)
            });
            return r;
        }
        self.ae2_scroll_at(delta, slot)
    }
    fn ae2_scroll_at(&mut self, delta: f32, slot: Option<usize>) -> Option<bool> {
        let (p, kind) = self.ae2_window_open()?;
        let which = slot.filter(|s| *s >= 300).and_then(|s| with_rt(|rt| rt.window.as_ref().and_then(|w| w.slots.get(s - 300).map(|x| x.0))).flatten());
        let processing = kind == WinKind::Encoding && with_st(self, |s| s.devices.at(p).map(|d| d.num("mode"))).flatten() == Some(1);
        if let Some(Slot::Ghost(i)) = which
            && (kind == WinKind::Interface || processing)
        {
            with_st(self, |st| {
                if let Some(d) = st.devices.at_mut(p)
                    && let Some(Some((_, n))) = d.config.get_mut(i)
                {
                    *n = if delta > 0.0 { *n + 1 } else { n.saturating_sub(1).max(1) };
                }
            });
            self.ae2_window_fill();
            return Some(true);
        }
        let up = delta > 0.0;
        let step = |v: usize, max: usize| if up { v.saturating_sub(1) } else { (v + 1).min(max) };
        if processing && matches!(which, Some(Slot::Ghost(_))) || kind == WinKind::Encoding && which.is_none() && processing && self.ae2_over_encoding() {
            with_rt(|rt| {
                if let Some(w) = rt.window.as_mut() {
                    w.page = step(w.page, cr::MAX_PROCESSING_INPUTS / 3 - 3);
                }
            });
            self.ae2_window_refresh();
            return Some(true);
        }
        if kind.terminal() {
            with_rt(|rt| {
                if let Some(w) = rt.window.as_mut() {
                    let max = w.max_scroll();
                    w.scroll = step(w.scroll, max);
                }
            });
            self.ae2_window_fill();
            return Some(true);
        }
        match kind {
            WinKind::Access => {
                with_rt(|rt| {
                    if let Some(w) = rt.window.as_mut() {
                        w.page = step(w.page, usize::MAX / 2);
                    }
                });
                self.ae2_window_refresh();
            }
            WinKind::CraftConfirm | WinKind::CraftCpu => {
                with_rt(|rt| {
                    if let Some(w) = rt.window.as_mut() {
                        let rows = if w.kind == WinKind::CraftConfirm { 5 } else { 6 };
                        let max = w.shown.len().div_ceil(3).saturating_sub(rows);
                        w.page = step(w.page, max);
                    }
                });
                self.ae2_window_refresh();
            }
            _ => {}
        }
        // Any other AE2 window: the wheel does nothing (no mouse-tweak moves over its slots).
        Some(true)
    }
    /// The cursor is over the encoding panel.
    fn ae2_over_encoding(&self) -> bool {
        let Some(batch) = super::foundry::hud_batch(self) else { return false };
        let (ox, oy) = self.inventory_origin(&batch);
        let h = self.ae2_size().map_or(0.0, |s| s.1);
        let (mx, my) = (self.cursor.0 - ox, self.cursor.1 - oy);
        mx >= 9.0 && mx < 135.0 && my >= h - 164.0 && my < h - 96.0
    }

    // ---- the screen's geometry and drawing ----

    fn ae2_size(&self) -> Option<(f32, f32)> {
        self.ae2_window_open()?;
        with_rt(|rt| rt.window.as_ref().map(|w| (w.w, w.h))).flatten()
    }
    pub(super) fn ae2_screen_height(&self) -> Option<f32> {
        self.ae2_size().map(|s| s.1)
    }
    pub(super) fn ae2_origin(&self, batch: &HudBatch) -> Option<(f32, f32)> {
        let (w, h) = self.ae2_size()?;
        Some(batch.centered_origin(w, h))
    }
    pub(super) fn ae2_image(&self, batch: &HudBatch) -> Option<horizon_client::container_screen::Image> {
        let (w, h) = self.ae2_size()?;
        let (x, y) = batch.centered_origin(w, h);
        // The side buttons and the upgrade panel count as part of the window.
        let kind = self.ae2_window_open().map(|(_, k)| k);
        let left = if kind.is_some_and(|k| k.terminal() || matches!(k, WinKind::Provider | WinKind::Emitter | WinKind::Workbench)) { 22.0 } else { 0.0 };
        let right = if kind.is_some_and(|k| {
            matches!(k, WinKind::Interface | WinKind::ImportBus | WinKind::ExportBus | WinKind::StorageBus | WinKind::Assembler | WinKind::Workbench | WinKind::Emitter)
        }) {
            32.0
        } else {
            0.0
        };
        Some(horizon_client::container_screen::Image { left: (x - left) as i32, top: y as i32, width: (w + left + right) as i32, height: h as i32 })
    }
    pub(super) fn ae2_slot_position(&self, i: usize) -> Option<Option<(f32, f32)>> {
        let (_, h) = self.ae2_size()?;
        let hidden = self.ae2_window_open().is_some_and(|(_, k)| k.no_inventory());
        Some(match i {
            // The crafting dialogs have no player inventory: its slots go off the screen.
            0..36 if hidden => Some((-10000.0, -10000.0)),
            0..9 => Some((8.0 + i as f32 * 18.0, h - 24.0)),
            9..36 => Some((8.0 + ((i - 9) % 9) as f32 * 18.0, h - 82.0 + ((i - 9) / 9) as f32 * 18.0)),
            300..500 => with_rt(|rt| rt.window.as_ref().and_then(|w| w.slots.get(i - 300).map(|s| (s.1, s.2)))).flatten(),
            _ => None,
        })
    }
    /// HEAD `draw_machine_screen`: the AE2 background, title and widgets.
    pub(super) fn ae2_draw(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some((p, kind)) = self.ae2_window_open() else { return false };
        let Some((rows, w, h, search, focused, amount, page, plan, error, labels)) = with_rt(|rt| {
            rt.window.as_ref().map(|w| (w.rows, w.w, w.h, w.search.clone(), w.focused, w.amount.clone(), w.page, w.plan.clone(), w.error.clone(), w.labels.clone()))
        })
        .flatten() else {
            return false;
        };
        let atlas = &self.gui.atlas;
        let blit_tex = |batch: &mut HudBatch, tex: &str, x: f32, y: f32, u: f32, v: f32, bw: f32, bh: f32| {
            if let Some(rect) = atlas.sprites.get(tex).copied() {
                let mut r = rect;
                let (du, dv) = ((rect.u1 - rect.u0) / 256.0, (rect.v1 - rect.v0) / 256.0);
                r.u0 = rect.u0 + u * du;
                r.v0 = rect.v0 + v * dv;
                r.u1 = r.u0 + bw * du;
                r.v1 = r.v0 + bh * dv;
                batch.rect_uv(ox + x, oy + y, bw, bh, r, u32::MAX);
            } else {
                batch.fill(atlas, ox + x, oy + y, bw, bh, rgba(198, 198, 198, 255));
            }
        };
        let tex = texture(kind);
        let blit = |batch: &mut HudBatch, x: f32, y: f32, u: f32, v: f32, bw: f32, bh: f32| blit_tex(batch, tex, x, y, u, v, bw, bh);
        let d = with_st(self, |s| s.devices.at(p).cloned()).flatten().unwrap_or_default();
        let grey = rgba(64, 64, 64, 255);
        if kind.terminal() {
            // TerminalStyle: header, first/middle/last rows, bottom.
            let bottom = kind.bottom();
            blit(batch, 0.0, 0.0, 0.0, 0.0, 195.0, TERM_HEADER);
            for r in 0..rows {
                let v = if r == 0 { 17.0 } else if r + 1 == rows { 53.0 } else { 35.0 };
                blit(batch, 0.0, TERM_HEADER + r as f32 * 18.0, 0.0, v, 195.0, 18.0);
            }
            blit(batch, 0.0, TERM_HEADER + rows as f32 * 18.0, 0.0, 71.0, 195.0, bottom);
            // The search text.
            let shown = if focused && (self.ui_millis() / 500) % 2 == 0 { format!("{search}_") } else { search.clone() };
            batch.text(atlas, &shown, ox + 82.0, oy + 6.0, u32::MAX, false);
            if kind == WinKind::Encoding {
                // The mode panel (pattern_modes.png) and its title.
                let (u, v) = match cr::PatternKind::from_index(d.num("mode")) {
                    cr::PatternKind::Crafting => (0.0, 0.0),
                    cr::PatternKind::Processing => (0.0, 70.0),
                    cr::PatternKind::Smithing => (128.0, 70.0),
                    cr::PatternKind::Stonecutting => (0.0, 141.0),
                };
                blit_tex(batch, "ae2:block/gui/pattern_modes", 9.0, h - 164.0, u, v, 126.0, 68.0);
                let t = self.translate("gui.ae2.PatternEncoding", "Pattern Encoding");
                batch.text(atlas, &t, ox + 8.0, oy + h - 175.0, grey, false);
            }
        } else if kind == WinKind::Access {
            blit(batch, 0.0, 0.0, 0.0, 0.0, 195.0, TERM_HEADER);
            for r in 0..rows {
                let text_row = (page + r) % 2 == 0;
                let v = if text_row { 17.0 } else { 35.0 };
                blit(batch, 0.0, TERM_HEADER + r as f32 * 18.0, 0.0, v, 195.0, 18.0);
            }
            blit(batch, 0.0, TERM_HEADER + rows as f32 * 18.0, 0.0, 125.0, 195.0, 97.0);
            for (y, name) in &labels {
                batch.text(atlas, name, ox + 10.0, oy + y + 5.0, grey, false);
            }
        } else {
            blit(batch, 0.0, 0.0, 0.0, 0.0, w, h);
        }
        // The upgrade panel (UpgradesPanel, extra_panels.png is not carried: plain frames).
        let panel = match kind {
            WinKind::Interface | WinKind::Emitter => 1,
            WinKind::ImportBus | WinKind::ExportBus | WinKind::StorageBus | WinKind::Assembler => 5,
            WinKind::Workbench => cr::upgrade_slots(cr::Host::ItemCell),
            _ => 0,
        };
        if panel > 0 {
            let (px, py) = (ox + w + 2.0, oy);
            let ph = 14.0 + panel as f32 * 18.0;
            batch.fill(atlas, px, py, 32.0, ph, rgba(85, 85, 85, 255));
            batch.fill(atlas, px + 1.0, py + 1.0, 30.0, ph - 2.0, rgba(198, 198, 198, 255));
            for i in 0..panel {
                let (sx, sy) = panel_slot(w, i);
                batch.fill(atlas, ox + sx - 1.0, oy + sy - 1.0, 18.0, 18.0, rgba(55, 55, 55, 255));
                batch.fill(atlas, ox + sx, oy + sy, 17.0, 17.0, rgba(255, 255, 255, 255));
                batch.fill(atlas, ox + sx, oy + sy, 16.0, 16.0, rgba(139, 139, 139, 255));
            }
        }
        // Buttons.
        for (_, x, y, bw, bh, label, enabled) in self.ae2_buttons() {
            let sprite = if enabled { "gui/sprites/widget/button" } else { "gui/sprites/widget/button_disabled" };
            horizon_client::ui::nine_slice(batch, atlas, sprite, ox + x, oy + y, bw, bh, u32::MAX);
            let tw = HudBatch::text_width(atlas, &label);
            let color = if enabled { u32::MAX } else { rgba(160, 160, 160, 255) };
            if bh < 12.0 {
                let start = batch.vertices.len();
                let (tx, ty) = (ox + x + (bw - tw * 0.75) / 2.0, oy + y + (bh - 6.0) / 2.0);
                batch.text(atlas, &label, tx, ty, color, true);
                batch.scale_since(start, tx, ty, 0.75, 0.75);
            } else {
                batch.text(atlas, &label, ox + x + (bw - tw) / 2.0, oy + y + (bh - 8.0) / 2.0 + 1.0, color, true);
            }
        }
        // Title.
        let (key, fallback) = title(kind);
        let t = match kind {
            WinKind::CraftConfirm => {
                let detail = match &plan {
                    Some(p) if p.simulation => self.translate("gui.ae2.Simulation", "Simulation"),
                    Some(p) => fmt1(&self.translate("gui.ae2.BytesUsed", "%s Bytes Used"), &p.bytes.to_string()),
                    None => String::new(),
                };
                fmt1(&self.translate(key, fallback), &detail)
            }
            _ => self.translate(key, fallback),
        };
        batch.text(atlas, &t, ox + 8.0, oy + 6.0, grey, false);
        if !kind.terminal() && !kind.no_inventory() && kind != WinKind::Access || kind == WinKind::Crafting || kind == WinKind::Encoding {
            let inv = self.translate("container.inventory", "Inventory");
            batch.text(atlas, &inv, ox + 8.0, oy + h - 93.0, grey, false);
        }
        match kind {
            WinKind::Drive | WinKind::ChestCell | WinKind::StorageBus | WinKind::Provider => {
                let pr = format!("{}: {}", self.translate("gui.ae2.Priority", "Priority"), d.num("priority"));
                let tw = HudBatch::text_width(atlas, &pr);
                batch.text(atlas, &pr, ox + w - 8.0 - tw, oy + 6.0, grey, false);
                if kind == WinKind::Provider {
                    batch.text(atlas, &self.translate("gui.ae2.Patterns", "Patterns"), ox + 8.0, oy + 34.0, grey, false);
                    batch.text(atlas, &self.translate("gui.ae2.ReturnInventory", "Return Inventory"), ox + 8.0, oy + 86.0, grey, false);
                }
            }
            WinKind::Interface => {
                batch.text(atlas, &self.translate("gui.ae2.Config", "Config"), ox + 8.0, oy + 24.0, grey, false);
                batch.text(atlas, &self.translate("gui.ae2.StoredItems", "Stored Items"), ox + 8.0, oy + 91.0, grey, false);
            }
            WinKind::Inscriber => {
                // The progress bar (inscriber.json: at 135,39; srcRect 135,177,6,18; fills up).
                let prog = (d.num("progress") as f32 / engine::config::INSCRIBER_MAX_STEPS as f32).clamp(0.0, 1.0);
                let ph = (18.0 * prog).floor();
                if ph > 0.0 {
                    blit(batch, 135.0, 39.0 + 18.0 - ph, 135.0, 177.0 + 18.0 - ph, 6.0, ph);
                }
            }
            WinKind::Assembler => {
                // molecular_assembler.json: progressBar at 148,48, srcRect 148,201,6,18.
                let prog = (d.num("progress") as f32 / cr::ASSEMBLER_PROGRESS as f32).clamp(0.0, 1.0);
                let ph = (18.0 * prog).floor();
                if ph > 0.0 {
                    blit(batch, 148.0, 48.0 + 18.0 - ph, 148.0, 201.0 + 18.0 - ph, 6.0, ph);
                }
            }
            WinKind::CraftAmount | WinKind::Emitter => {
                let (fx, fy) = if kind == WinKind::CraftAmount { (50.0, 57.0) } else { (25.0, 44.0) };
                let shown = if focused && (self.ui_millis() / 500) % 2 == 0 { format!("{amount}_") } else { amount.clone() };
                batch.text(atlas, &shown, ox + fx, oy + fy, u32::MAX, false);
                if kind == WinKind::Emitter {
                    let on = d.num("on") != 0;
                    batch.text(atlas, if on { "\u{25cf} ON" } else { "\u{25cb} OFF" }, ox + 120.0, oy + 6.0, if on { rgba(200, 40, 40, 255) } else { grey }, false);
                }
            }
            WinKind::CraftConfirm => {
                if let Some(p) = &plan {
                    let avail = self.ae2_avail();
                    let text = match avail {
                        Some(a) => format!("{} / {}", p.bytes, a),
                        None => format!("{}", p.bytes),
                    };
                    let tw = HudBatch::text_width(atlas, &text);
                    batch.text(atlas, &text, ox + 109.0 - tw / 2.0, oy + 165.0, grey, false);
                }
            }
            WinKind::CraftCpu => {
                let text = self.ae2_cpu_line();
                batch.text(atlas, &text, ox + 8.0, oy + h - 40.0, grey, false);
            }
            _ => {}
        }
        if let Some(e) = &error {
            batch.text(atlas, e, ox + 8.0, oy + h + 4.0, rgba(255, 80, 80, 255), true);
        }
        true
    }
    /// HEAD `draw_carried`: what goes over the slots (amounts, stock sizes, table lines).
    pub(super) fn ae2_overlay(&self, batch: &mut HudBatch) {
        let Some((p, kind)) = self.ae2_window_open() else { return };
        let Some((slots, view, scroll, active, w, craftable, lines, shown, page)) = with_rt(|rt| {
            rt.window.as_ref().map(|w| (w.slots.clone(), w.view.clone(), w.scroll, w.active, w.w, w.craftable.clone(), w.lines.clone(), w.shown.clone(), w.page))
        })
        .flatten() else {
            return;
        };
        let (ox, oy) = batch.centered_origin(w, self.ae2_size().map_or(166.0, |s| s.1));
        let atlas = &self.gui.atlas;
        let d = with_st(self, |s| s.devices.at(p).cloned()).flatten().unwrap_or_default();
        let small = |batch: &mut HudBatch, text: &str, x: f32, y: f32| {
            // StackSizeRenderer: scale 0.5, right-aligned at the slot's corner, white, shadow.
            let start = batch.vertices.len();
            let tw = HudBatch::text_width(atlas, text);
            let (sx, sy) = (x + 16.0 - 1.0 - tw * 0.5, y + 16.0 - 1.0 - 7.0 * 0.5);
            batch.text(atlas, text, sx, sy, u32::MAX, true);
            batch.scale_since(start, sx, sy, 0.5, 0.5);
        };
        let processing = kind == WinKind::Encoding && d.num("mode") == 1;
        for (s, x, y) in &slots {
            match *s {
                Slot::Grid(i) => {
                    if let Some((k, n)) = view.get(scroll * 9 + i) {
                        let text = if *n == 0 && craftable.contains(k) {
                            self.translate("gui.ae2.Craft", "Craft")
                        } else {
                            engine::format_amount(*n, if engine::config::CLIENT_USE_TERMINAL_USE_LARGE_FONT { 3 } else { 4 })
                        };
                        small(batch, &text, ox + x, oy + y);
                    }
                }
                Slot::Ghost(i) if kind == WinKind::Interface || processing => {
                    if let Some(Some((_, n))) = d.config.get(i) {
                        small(batch, &engine::format_amount(*n, 4), ox + x, oy + y);
                    }
                }
                Slot::Show(i) if matches!(kind, WinKind::CraftConfirm | WinKind::CraftCpu) => {
                    // The cell's lines, right-aligned before the item (AbstractTableRenderer).
                    let Some(l) = lines.get(i) else { continue };
                    let missing = shown.get(i).cloned().flatten().is_some_and(|(_, m)| m > 0) && kind == WinKind::CraftConfirm;
                    let cell_x = ox + x - (CELL_W - 19.0);
                    let cell_y = oy + y - 3.0;
                    if missing {
                        batch.fill(atlas, cell_x, cell_y, CELL_W, CELL_H, rgba(255, 0, 0, 40));
                    }
                    let line_h = 4.5;
                    let total = l.len() as f32 * line_h;
                    let mut ty = (cell_y + (CELL_H - total) / 2.0).round();
                    for text in l {
                        let start = batch.vertices.len();
                        let tw = HudBatch::text_width(atlas, text);
                        let tx = ox + x - 2.0 - tw * 0.5;
                        batch.text(atlas, text, tx, ty, rgba(64, 64, 64, 255), false);
                        batch.scale_since(start, tx, ty, 0.5, 0.5);
                        ty += line_h;
                    }
                }
                _ => {}
            }
        }
        let _ = page;
        if kind.terminal() && !active {
            let msg = self.translate("gui.ae2.NoPower", "No Power / No Channel");
            let tw = HudBatch::text_width(atlas, &msg);
            batch.text(atlas, &msg, ox + (w - tw) / 2.0 - 10.0, oy + 22.0, rgba(255, 80, 80, 255), true);
        }
    }
    fn ui_millis(&self) -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis())
    }
    /// HEAD `close_container`: the window's temporary block entity goes away.
    pub(super) fn ae2_close(&mut self) {
        if self.ae2_window().is_some() {
            self.ae2_window_closed();
        }
    }
    /// Hoppers never see a window's temporary slots.
    pub(super) fn ae2_window_at(&self, pos: (i32, i32, i32)) -> bool {
        with_rt(|rt| rt.window.as_ref().is_some_and(|w| w.key() == pos)) == Some(true)
            || GUESTS.with(|g| g.borrow().values().any(|w| w.key() == pos))
            || STASH.with(|s| s.borrow().iter().flatten().any(|w| w.key() == pos))
    }
}

/// The ME chest's cell as a single mount.
struct ChestCellMount<'a> {
    cell: &'a mut CellInv,
    ty: &'static engine::CellType,
}
impl engine::Mounts for ChestCellMount<'_> {
    fn mounts(&self) -> usize {
        1
    }
    fn priority(&self, _: usize) -> i32 {
        0
    }
    fn insert(&mut self, _: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        let nested = key_is_full_cell(key);
        self.cell.insert(self.ty, key, amount, simulate, nested)
    }
    fn extract(&mut self, _: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        self.cell.extract(key, amount, simulate)
    }
    fn list(&mut self, _: usize, out: &mut BTreeMap<Key, u64>) {
        for (k, n) in &self.cell.items {
            *out.entry(k.clone()).or_insert(0) += n;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// AE2 windows for guests (foundry menus, foundry's mod networking, mods/foundry/API.md section 10). Single player's window
// is a guest of its integrated server, as LAN friends are: the server keeps each guest's `Window`
// apart (`GUESTS`) and puts it in `Rt::window` for the guest's turns (`ae2_menu_enter` / `_leave`),
// so every handler of this file runs unchanged as the guest; foundry mirrors the window's temporary
// slots, and the window itself, its device and the dialogs' numbers go as sections. The guest's
// client draws from the mirror (`ae2_guest_sync`, `with_st` on a client) and sends its input back.

/// What a guest's client knows besides the window and its device.
#[derive(Default)]
struct GuestView {
    /// foundry's generation of the sections applied (0: apply again).
    generation: u64,
    shown: bool,
    avail: Option<u64>,
    cpu_line: String,
}

fn ae2_register_menu() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::foundry::register_menu(super::foundry::MenuKind {
        claims: Some(ae2_menu_claims),
        enter: Some(ae2_menu_enter),
        leave: Some(ae2_menu_leave),
        tick: Some(ae2_menu_tick),
        sections: Some(ae2_menu_sections),
        input: Some(ae2_menu_input),
        closed: Some(ae2_menu_closed),
        valid: Some(ae2_menu_valid),
        ..super::foundry::MenuKind::new(AE2_MENU)
    });
}
/// After the guest's turn that opened it (`lan_open_container`): the window is theirs.
fn ae2_menu_claims(_game: &mut Game<'_>, conn: u32, p: Pos) -> bool {
    GUESTS.with(|g| g.borrow().get(&conn).is_some_and(|w| w.pos.pos == p))
}
fn ae2_menu_enter(game: &mut Game<'_>, conn: u32, _p: Pos) {
    let w = GUESTS.with(|g| g.borrow_mut().remove(&conn));
    let host = with_rt(|rt| std::mem::replace(&mut rt.window, w)).flatten();
    STASH.with(|s| s.borrow_mut().push(host));
    // The temporary slots show this guest's view (the host's may share the block).
    game.ae2_window_fill();
}
fn ae2_menu_leave(game: &mut Game<'_>, conn: u32, _p: Pos) {
    let w = with_rt(|rt| rt.window.take()).flatten();
    let key = w.as_ref().map(Window::key);
    if let Some(w) = w {
        GUESTS.with(|g| g.borrow_mut().insert(conn, w));
    }
    let host = STASH.with(|s| s.borrow_mut().pop()).flatten();
    let refill = host.as_ref().is_some_and(|h| Some(h.key()) == key);
    with_rt(|rt| rt.window = host);
    if refill {
        game.ae2_window_fill();
    }
}
/// The window's refresh, as `ae2_window_tick` does for the host's (every 5 ticks).
fn ae2_menu_tick(game: &mut Game<'_>, _conn: u32, _p: Pos) {
    if with_rt(|rt| rt.tick % 5 == 0) == Some(true) {
        game.ae2_window_refresh();
    }
}
/// The sections: the window (without its view), its view (the grid's keys and amounts, the
/// craftable keys), its device (the side-file text of that one record), the dialogs' numbers.
fn ae2_menu_sections(game: &mut Game<'_>, _conn: u32, _p: Pos) -> Vec<Vec<u8>> {
    let Some(w) = with_rt(|rt| rt.window.clone()).flatten() else { return Vec::new() };
    let mut st = engine::State::default();
    if let Some(d) = with_st(game, |s| s.devices.at(w.pos).cloned()).flatten() {
        st.devices.insert(w.pos, d);
    }
    let avail = if w.kind == WinKind::CraftConfirm { game.ae2_avail() } else { None };
    let cpu_line = if w.kind == WinKind::CraftCpu { game.ae2_cpu_line() } else { String::new() };
    vec![
        ae2_encode_window(&w),
        ae2_encode_view(&w),
        st.save().into_bytes(),
        super::foundry::to_bytes(&(avail, cpu_line)),
    ]
}
fn ae2_menu_input(game: &mut Game<'_>, _conn: u32, _p: Pos, input: &super::foundry::MenuInput) -> bool {
    use super::foundry::MenuInput;
    match input {
        MenuInput::Mouse { button, .. } => game.ae2_mouse_at(super::foundry::button_of(*button)),
        MenuInput::Scroll { delta, slot, .. } => game.ae2_scroll_at(*delta, usize::try_from(*slot).ok()).unwrap_or(false),
        MenuInput::Key { key, .. } => super::foundry::key_of(*key).and_then(|k| game.ae2_key_at(k)).unwrap_or(false),
        MenuInput::Text { text, backspace } => game.ae2_text_at(text.as_deref(), *backspace),
        _ => false,
    }
}
fn ae2_menu_closed(game: &mut Game<'_>, conn: u32, p: Pos) {
    let gone = GUESTS.with(|g| g.borrow_mut().remove(&conn));
    let key = gone.as_ref().map_or((p[0], p[1], p[2]), Window::key);
    if !game.ae2_window_at(key) && matches!(game.world.block_entities.get(&key), Some(BlockEntity::Chest(_))) {
        game.world.block_entities.remove(&key);
    }
}
fn ae2_menu_valid(game: &Game<'_>, conn: u32, _p: Pos) -> bool {
    GUESTS.with(|g| g.borrow().get(&conn).map(|w| w.pos)).is_some_and(|l| game.ae2_kind_at(l).is_some())
}

/// A guest's client: the window, its device and the numbers of the foundry menu it shows, put in
/// place whenever the server sent new ones; the window goes when the menu does.
fn ae2_guest_sync(game: &Game<'_>) {
    if game.remote.is_none() {
        return;
    }
    let ours = super::foundry::guest_menu(game).is_some_and(|(k, _)| k == AE2_MENU);
    if !ours {
        let was = GUEST_VIEW.with(|v| {
            let mut v = v.borrow_mut();
            v.generation = 0;
            std::mem::take(&mut v.shown)
        });
        if was {
            with_rt(|rt| rt.window = None);
            GUEST_ST.with(|s| {
                if let Ok(mut s) = s.try_borrow_mut() {
                    *s = engine::State::default();
                }
            });
        }
        return;
    }
    let Some((generation, sections)) = super::foundry::guest_sections(game) else { return };
    if GUEST_VIEW.with(|v| v.borrow().generation) == generation {
        return;
    }
    let window = match (sections.first(), sections.get(1)) {
        (Some(head), Some(view)) => ae2_decode_window(head, view),
        _ => None,
    };
    if with_rt(|rt| rt.window = window).is_none() {
        return; // re-entered: next time
    }
    if let Some(text) = sections.get(2).and_then(|b| std::str::from_utf8(b).ok()) {
        let st = engine::State::load(Some(text.to_owned()));
        GUEST_ST.with(|s| {
            if let Ok(mut s) = s.try_borrow_mut() {
                *s = st;
            }
        });
    }
    let extra: Option<(Option<u64>, String)> = sections.get(3).and_then(|b| super::foundry::from_bytes(b));
    GUEST_VIEW.with(|v| {
        let mut v = v.borrow_mut();
        v.generation = generation;
        v.shown = true;
        if let Some((avail, line)) = extra {
            v.avail = avail;
            v.cpu_line = line;
        }
    });
}

impl Game<'_> {
    /// This window shows an AE2 window as a guest.
    fn ae2_guest_open(&self) -> bool {
        super::foundry::guest_menu(self).is_some_and(|(k, _)| k == AE2_MENU)
    }
    /// A guest's input: the handler runs here on the mirror only to see whether the window takes it
    /// (the mirror is put back right after: the server's answer is the window), then it goes to the
    /// server, which runs the same handler as this player.
    fn ae2_guest_input(&mut self, input: super::foundry::MenuInput, local: impl FnOnce(&mut Game<'_>) -> bool) -> bool {
        self.ae2_window();
        let took = super::foundry::guest_widget(self, input, local);
        GUEST_VIEW.with(|v| v.borrow_mut().generation = 0);
        took
    }
    /// The chosen CPU's storage (the craft confirm screen's "bytes / available").
    fn ae2_avail(&self) -> Option<u64> {
        if self.remote.is_some() {
            return GUEST_VIEW.with(|v| v.borrow().avail);
        }
        with_rt(|rt| rt.window.as_ref().and_then(|w| w.cpu))
            .flatten()
            .and_then(|c| with_rt(|rt| rt.clusters.iter().find(|x| x.core() == c).map(|x| x.storage)).flatten())
    }
    /// The CPU status screen's line (its job, or none).
    fn ae2_cpu_line(&self) -> String {
        if self.remote.is_some() {
            return GUEST_VIEW.with(|v| v.borrow().cpu_line.clone());
        }
        let core = with_rt(|rt| rt.window.as_ref().and_then(|w| w.cpu)).flatten();
        match core.map(|c| self.ae2_cpu(c)) {
            Some(c) => match &c.job {
                Some(j) => match stack_of(&j.output.0, 1) {
                    Some(s) => format!("{} x{} ({})", horizon_client::stations::hover_name(&|i| self.pack.names.display_item(i), &s), j.output.1, j.remaining),
                    None => String::new(),
                },
                None => self.translate("gui.ae2.NoCraftingJobs", "No Crafting Job Active"),
            },
            None => self.translate("gui.ae2.NoCraftingCPUs", "No Crafting CPUs are Available"),
        }
    }
}

// ---- the window as bytes (foundry's codec) ----

const WIN_KINDS: [WinKind; 20] = [
    WinKind::Terminal,
    WinKind::Crafting,
    WinKind::ChestTerm,
    WinKind::ChestCell,
    WinKind::Drive,
    WinKind::Interface,
    WinKind::ImportBus,
    WinKind::ExportBus,
    WinKind::StorageBus,
    WinKind::Inscriber,
    WinKind::Vibration,
    WinKind::Encoding,
    WinKind::Access,
    WinKind::Provider,
    WinKind::Assembler,
    WinKind::Workbench,
    WinKind::Emitter,
    WinKind::CraftAmount,
    WinKind::CraftConfirm,
    WinKind::CraftCpu,
];
fn win_code(k: WinKind) -> u8 {
    WIN_KINDS.iter().position(|x| *x == k).unwrap_or(0) as u8
}
fn win_of(c: u8) -> Option<WinKind> {
    WIN_KINDS.get(usize::from(c)).copied()
}
fn put_loc(w: &mut super::foundry::Writer, l: &Loc) {
    w.put(&l.pos).u8(l.side.map_or(255, |d| d.idx() as u8));
}
fn get_loc(r: &mut super::foundry::Reader<'_>) -> Option<Loc> {
    let pos: Pos = r.get()?;
    let side = match r.u8()? {
        255 => None,
        s => Some(Dir::from_idx(usize::from(s))?),
    };
    Some(Loc { pos, side })
}
fn slot_parts(s: Slot) -> (u8, usize) {
    match s {
        Slot::Grid(i) => (0, i),
        Slot::Real(i) => (1, i),
        Slot::Ghost(i) => (2, i),
        Slot::Result => (3, 0),
        Slot::Show(i) => (4, i),
        Slot::Remote(i) => (5, i),
        Slot::CellCfg(i) => (6, i),
        Slot::CellUpg(i) => (7, i),
    }
}
fn slot_from(t: u8, i: usize) -> Option<Slot> {
    Some(match t {
        0 => Slot::Grid(i),
        1 => Slot::Real(i),
        2 => Slot::Ghost(i),
        3 => Slot::Result,
        4 => Slot::Show(i),
        5 => Slot::Remote(i),
        6 => Slot::CellCfg(i),
        7 => Slot::CellUpg(i),
        _ => return None,
    })
}
fn map_list(m: &BTreeMap<Key, u64>) -> Vec<(Key, u64)> {
    m.iter().map(|(k, v)| (k.clone(), *v)).collect()
}
fn put_plan(w: &mut super::foundry::Writer, p: &cr::Plan) {
    w.put(&p.output)
        .uvar(p.bytes)
        .bool(p.simulation)
        .bool(p.multiple_paths)
        .put(&map_list(&p.used))
        .put(&map_list(&p.emitted))
        .put(&map_list(&p.missing))
        .put(&p.crafts)
        .bool(p.too_complex);
}
fn get_plan(r: &mut super::foundry::Reader<'_>) -> Option<cr::Plan> {
    let output = r.get()?;
    let bytes = r.uvar()?;
    let simulation = r.bool()?;
    let multiple_paths = r.bool()?;
    let used: Vec<(Key, u64)> = r.get()?;
    let emitted: Vec<(Key, u64)> = r.get()?;
    let missing: Vec<(Key, u64)> = r.get()?;
    let crafts = r.get()?;
    let too_complex = r.bool()?;
    Some(cr::Plan {
        output,
        bytes,
        simulation,
        multiple_paths,
        used: used.into_iter().collect(),
        emitted: emitted.into_iter().collect(),
        missing: missing.into_iter().collect(),
        crafts,
        too_complex,
    })
}
fn ae2_encode_window(win: &Window) -> Vec<u8> {
    let mut w = super::foundry::Writer::new();
    put_loc(&mut w, &win.pos);
    w.u8(win_code(win.kind)).uvar(win.rows as u64).uvar(win.scroll as u64);
    w.uvar(win.slots.len() as u64);
    for (s, x, y) in &win.slots {
        let (t, i) = slot_parts(*s);
        w.u8(t).uvar(i as u64).f32(*x).f32(*y);
    }
    w.f32(win.w).f32(win.h).str(&win.search).bool(win.focused).bool(win.active).uvar(win.page as u64).str(&win.amount);
    w.put(&win.craft);
    match &win.plan {
        Some(p) => {
            w.u8(1);
            put_plan(&mut w, p);
        }
        None => {
            w.u8(0);
        }
    }
    w.put(&win.cpu).put(&win.back.map(win_code)).put(&win.error).put(&win.shown).put(&win.lines);
    w.uvar(win.remote.len() as u64);
    for (l, i) in &win.remote {
        put_loc(&mut w, l);
        w.uvar(*i as u64);
    }
    w.put(&win.labels);
    w.finish()
}
fn ae2_encode_view(win: &Window) -> Vec<u8> {
    let craftable: Vec<Key> = win.craftable.iter().cloned().collect();
    super::foundry::to_bytes(&(win.view.clone(), craftable))
}
fn ae2_decode_window(head: &[u8], view: &[u8]) -> Option<Window> {
    let mut r = super::foundry::Reader::new(head);
    let pos = get_loc(&mut r)?;
    let kind = win_of(r.u8()?)?;
    let rows = usize::try_from(r.uvar()?).ok()?;
    let scroll = usize::try_from(r.uvar()?).ok()?;
    let n = usize::try_from(r.uvar()?).ok()?;
    if n > 1024 {
        return None;
    }
    let mut slots = Vec::with_capacity(n);
    for _ in 0..n {
        let t = r.u8()?;
        let i = usize::try_from(r.uvar()?).ok()?;
        slots.push((slot_from(t, i)?, r.f32()?, r.f32()?));
    }
    let (w, h) = (r.f32()?, r.f32()?);
    let search = r.str()?.to_owned();
    let (focused, active) = (r.bool()?, r.bool()?);
    let page = usize::try_from(r.uvar()?).ok()?;
    let amount = r.str()?.to_owned();
    let craft: Option<Key> = r.get()?;
    let plan = match r.u8()? {
        0 => None,
        1 => Some(get_plan(&mut r)?),
        _ => return None,
    };
    let cpu: Option<Pos> = r.get()?;
    let back: Option<u8> = r.get()?;
    let error: Option<String> = r.get()?;
    let shown: Vec<Option<(Key, u64)>> = r.get()?;
    let lines: Vec<Vec<String>> = r.get()?;
    let rn = usize::try_from(r.uvar()?).ok()?;
    if rn > 4096 {
        return None;
    }
    let mut remote = Vec::with_capacity(rn);
    for _ in 0..rn {
        let l = get_loc(&mut r)?;
        remote.push((l, usize::try_from(r.uvar()?).ok()?));
    }
    let labels: Vec<(f32, String)> = r.get()?;
    if !r.done() {
        return None;
    }
    let (view, craftable): (Vec<(Key, u64)>, Vec<Key>) = super::foundry::from_bytes(view)?;
    Some(Window {
        pos,
        kind,
        rows,
        scroll,
        view,
        slots,
        w,
        h,
        search,
        focused,
        last_shift: None,
        active,
        page,
        amount,
        craft,
        plan,
        cpu,
        back: back.and_then(win_of),
        error,
        shown,
        lines,
        remote,
        labels,
        craftable: craftable.into_iter().collect(),
    })
}
