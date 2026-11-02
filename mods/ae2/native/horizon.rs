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
    include!("ae2_engine.rs");
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

include!("kit_blocks.rs");
include!("kit_notice.rs");
include!("kit_sound.rs");
include!("kit_state.rs");

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
