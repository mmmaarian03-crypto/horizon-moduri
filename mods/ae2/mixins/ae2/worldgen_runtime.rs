// AE2 meteorites as a native worldgen step (the carried structure `ae2:meteorite` is a Java
// structure type Horizon cannot run). Module `ae2` in horizon-world's generator.rs; the rules
// (where a meteorite is, what each block becomes) are the engine's (src/lib.rs, pasted below by
// tools/bundle_extra.py). Each chunk places the meteorite whose centre it holds, into its 3x3
// region (MeteoritePlacer clips to the structure piece's box the same way).
use super::{BlockStateId, ProtoChunk};

#[allow(dead_code)]
mod engine {
    // Applied Energistics 2 for Horizon, stage 1 (item storage network): the rules, without game code.
// Written from AE2 forge 15.4.11 for Minecraft 1.20.1 (AppliedEnergistics, LGPL-3.0; the public
// source at tag forge/v15.4.11, cloned in surse/ae2, the same version as the pack's jar
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

/// An external inventory seen as slots (`IItemHandler` through forja), for storage, import and
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
// 11. The cable bus (CableBusBlock / CableBusContainer): every cable and part lives in a forja
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

fn state_of(region: &[ProtoChunk; 9], name: &str, props: &[(&str, &str)]) -> Option<BlockStateId> {
    let registry = region[4].data().registry();
    let id = registry.default_state(name)?;
    if props.is_empty() {
        return Some(id);
    }
    let mut s = registry.state(id).cloned()?;
    for (k, v) in props {
        s.properties.insert((*k).to_owned(), (*v).to_owned());
    }
    registry.resolve(&s).ok()
}

struct Area<'a> {
    chunks: &'a mut [ProtoChunk; 9],
    center: (i32, i32),
}
impl Area<'_> {
    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let (dx, dz) = ((x >> 4) - self.center.0, (z >> 4) - self.center.1);
        if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dz) {
            return None;
        }
        let i = ((dz + 1) * 3 + (dx + 1)) as usize;
        let t = &self.chunks[i].terrain;
        (y >= t.min_y() && y < t.min_y() + t.height()).then_some(i)
    }
    fn name(&self, x: i32, y: i32, z: i32) -> Option<&str> {
        let i = self.index(x, y, z)?;
        Some(self.chunks[i].block_name(x, y, z))
    }
    fn set(&mut self, x: i32, y: i32, z: i32, s: BlockStateId) {
        if let Some(i) = self.index(x, y, z) {
            let _ = self.chunks[i].terrain.set_block_raw(x, y, z, s);
        }
    }
    /// WORLD_SURFACE_WG: the first block from the top that is not air.
    fn surface(&self, x: i32, z: i32) -> Option<(i32, bool)> {
        let i = self.index(x, 0, z).or_else(|| self.index(x, 64, z))?;
        let t = &self.chunks[i].terrain;
        let (lo, hi) = (t.min_y(), t.min_y() + t.height() - 1);
        let mut y = hi;
        while y >= lo {
            let n = self.chunks[i].block_name(x, y, z);
            if !matches!(n, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") {
                return Some((y, n == "minecraft:water"));
            }
            y -= 1;
        }
        None
    }
}

/// The meteorite of the chunk `region[4]`, if its centre is there (overworld only).
pub(super) fn place_meteorite(region: &mut [ProtoChunk; 9]) {
    if region[4].terrain.min_y() != -64 {
        return;
    }
    let at = region[4].position();
    let Some(m) = engine::meteor_in_chunk(at.x, at.z) else { return };
    let (Some(sky), Some(air), Some(cube)) = (
        state_of(region, "ae2:sky_stone_block", &[]),
        state_of(region, "minecraft:air", &[]),
        state_of(region, "ae2:mysterious_cube", &[]),
    ) else {
        return;
    };
    let quartz: Vec<BlockStateId> = engine::METEOR_QUARTZ
        .iter()
        .filter_map(|n| state_of(region, n, &[]))
        .collect();
    let buds: Vec<BlockStateId> = engine::BUDS[..3]
        .iter()
        .filter_map(|n| state_of(region, n, &[("facing", "up"), ("waterlogged", "false")]))
        .collect();
    let mut area = Area { chunks: region, center: (at.x, at.z) };
    // MeteoriteStructure.generatePieces: the mean surface height around the centre, lowered on
    // rough ground, minus the radius.
    let scan = ((m.radius * 2.0) as i32).max(1).min(16);
    let mut hs = Vec::new();
    for dx in -scan..=scan {
        for dz in -scan..=scan {
            if let Some((h, _)) = area.surface(m.x + dx, m.z + dz) {
                hs.push(f64::from(h));
            }
        }
    }
    let Some((_, wet)) = area.surface(m.x, m.z) else { return };
    if hs.is_empty() || wet {
        // Ocean meteorites (OCEAN_FLOOR_WG) are left out: a raw crater would leave a hole in the sea.
        return;
    }
    let mean = hs.iter().sum::<f64>() / hs.len() as f64;
    let var = hs.iter().map(|h| (h - mean) * (h - mean)).sum::<f64>() / hs.len() as f64;
    let min = hs.iter().copied().fold(f64::MAX, f64::min);
    let mut cy = mean as i32;
    if var > 5.0 {
        cy -= ((mean - min) * 0.75) as i32;
    }
    let y_off = m.radius.ceil() as i32 + 1;
    cy = (cy - y_off).max(-64 + y_off);
    let mut rng = engine::Rng::new((m.x as i64 as u64) << 32 ^ (m.z as i64 as u64) ^ 0xAE2);
    let reach = ((m.radius as f64 * 2.0 + 5.0) as i32).min(31);
    for dx in -reach..=reach {
        for dz in -reach..=reach {
            for dy in -8..=(319 - cy) {
                let (x, y, z) = (m.x + dx, cy + dy, m.z + dz);
                let b = engine::meteor_block(m.radius, dx, dy, dz);
                match b {
                    engine::MeteorBlock::Keep => {
                        if dy > 8 && dx * dx + dz * dz > reach * reach {
                            break;
                        }
                    }
                    engine::MeteorBlock::SkyStone => area.set(x, y, z, sky),
                    engine::MeteorBlock::Cube => area.set(x, y, z, cube),
                    engine::MeteorBlock::Air => {
                        if area.name(x, y, z).is_some_and(|n| n != "minecraft:air") {
                            area.set(x, y, z, air);
                        }
                    }
                    engine::MeteorBlock::Quartz => {
                        if quartz.is_empty() {
                            continue;
                        }
                        let i = if engine::config::WORLD_GEN_SPAWN_FLAWLESS_ONLY { quartz.len() - 1 } else { rng.next_int(quartz.len() as i32) as usize };
                        area.set(x, y, z, quartz[i]);
                        // A bud on top of a budding block that is not in the middle (70%).
                        if i != 0 && (dx != 0 || dz != 0) && rng.next_float() <= 0.7 && !buds.is_empty() {
                            let b = buds[rng.next_int(buds.len() as i32) as usize];
                            area.set(x, y + 1, z, b);
                        }
                    }
                }
            }
        }
    }
}
