// Forja inside Horizon's game (nightly 142388e1c): the shared API of the tech mods.
// This module lives in `game::machines` (see the [[module]] in horizon-mixin.toml) so its item
// handler can use the hopper machinery's container views; `crate::game::forja` (alias.rs) re-exports
// it. Every mod compiled into the same game binary calls it as `crate::game::forja::...` (see
// FUNDATIE-FORJA.md). The rules are the engine below (src/lib.rs).
//
// What lives here:
// - the registries: machine specs (side-table energy/tanks/slots + a tick), other mods' energy,
//   fluid and item providers, mod buckets, machine screens;
// - the dispatch: "the energy/fluid/item handler of the block at p seen from face" whoever
//   provides it (forja's table, a registered provider, a vanilla container);
// - the machine table of each world and dimension (`<world>/data/forja/<dimension>.txt`, the
//   kit_state pattern), saved with the world; machines whose block is gone drop their items;
// - the screen helper for machines with a container template (background, bars, labels).
use super::{Game, View};
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_core::geometry::Direction;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
pub(in crate::game) mod engine {
    include!("forja_engine.rs");
}
#[allow(unused_imports)]
pub(in crate::game) use engine::{
    Access, Bar, BucketUse, Dir, EnergyHandler, EnergyInfo, EnergyStorage, FluidHandler, FluidStack,
    FluidTank, Grow, Host, ItemHandler, Machine, NO_DROP, Part, PartShape, PartSpec, Pos, Rel, Sides, Slot,
    SlotRole, Spec, TankInfo, TankRole,
};

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
const MOD_ID: &str = "forja";

include!("kit_state.rs");

impl engine::Stack for ItemStack {
    fn item(&self) -> &str {
        &self.item
    }
    fn count(&self) -> u32 {
        self.count
    }
    fn set_count(&mut self, count: u32) {
        self.count = count;
    }
    fn same_kind(&self, other: &Self) -> bool {
        self.same_item_same_components(other)
    }
}

/// Item stacks in the side file: the game's own item NBT (all components), as hex.
struct NbtCodec;
impl engine::StackCodec<ItemStack> for NbtCodec {
    fn encode(&self, s: &ItemStack) -> String {
        use horizon_world::nbt::{JavaString, Limits, NamedTag, write_named};
        let root = NamedTag {
            name: JavaString::from(""),
            tag: horizon_client::save::item_to_nbt(s),
        };
        let bytes = write_named(&root, Limits::default()).unwrap_or_default();
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
            out.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
        }
        out
    }
    fn decode(&self, text: &str) -> Option<ItemStack> {
        use horizon_world::nbt::{Limits, read_named};
        let t = text.as_bytes();
        if t.len() % 2 != 0 {
            return None;
        }
        let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
        let bytes: Vec<u8> = t
            .chunks(2)
            .map(|p| Some(hex(p[0])? << 4 | hex(p[1])?))
            .collect::<Option<_>>()?;
        let (named, _) = read_named(&bytes, Limits::default()).ok()?;
        horizon_client::save::item_from_nbt(&named.tag)
    }
}

// ---- registries (shared by every mod of the binary; plain fn pointers) ----

/// A provider of energy for blocks forja does not keep: call `f` with the handler of the block at
/// `pos` seen from `face` (`None` = the block itself) and return true, or return false when the
/// block is not yours / has no energy on that side.
pub(in crate::game) type EnergyFn =
    for<'a, 'g> fn(&'a mut Game<'g>, Pos, Option<Dir>, &mut dyn FnMut(&mut dyn EnergyHandler)) -> bool;
pub(in crate::game) type FluidFn =
    for<'a, 'g> fn(&'a mut Game<'g>, Pos, Option<Dir>, &mut dyn FnMut(&mut dyn FluidHandler)) -> bool;
pub(in crate::game) type ItemFn = for<'a, 'g> fn(
    &'a mut Game<'g>,
    Pos,
    Option<Dir>,
    &mut dyn FnMut(&mut dyn ItemHandler<ItemStack>),
) -> bool;
/// The cheap probe of a provider: the block at `pos` has this capability on `face` (no handler is
/// made; this is what pipes and cables ask to draw their connections).
pub(in crate::game) type ClaimFn = for<'a, 'g> fn(&'a Game<'g>, Pos, Option<Dir>) -> bool;
/// A machine's work for one tick (`BlockEntityTicker`); reach its state with `with_machine`.
pub(in crate::game) type TickFn = for<'a, 'g> fn(&'a mut Game<'g>, Pos);

/// What a bar on a machine screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Gauge {
    /// `progress / max_progress` (an arrow).
    Progress,
    /// Stored energy / capacity.
    Energy,
    /// Tank `i`: amount / capacity.
    Tank(usize),
    /// `data[value] / data[max]` (a burn flame: "burn" / "burn_total").
    Data(&'static str, &'static str),
}

/// A machine screen drawn over a vanilla container menu (ironchest's approach): the mod's
/// texture under `assets/<ns>/textures/block/...` (named `<ns>:block/...`), its size, the
/// bars, and optionally where its container slots sit.
#[derive(Clone, Debug)]
pub(in crate::game) struct Screen {
    pub texture: &'static str,
    pub texture_size: (i32, i32),
    /// `imageWidth`, `imageHeight` (176 wide: the vanilla inventory geometry).
    pub size: (i32, i32),
    pub title_key: &'static str,
    pub title_fallback: &'static str,
    pub bars: Vec<(Bar, Gauge)>,
    /// The container slots' positions (menu slots 300..); `None` = the vanilla layout.
    pub slots: Option<Vec<(i32, i32)>>,
}

struct Kind {
    spec: Arc<Spec>,
    tick: Option<TickFn>,
    screen: Option<Arc<Screen>>,
}
#[derive(Default)]
struct Registry {
    kinds: BTreeMap<String, Kind>,
    energy: Vec<(&'static str, ClaimFn, EnergyFn)>,
    fluid: Vec<(&'static str, ClaimFn, FluidFn)>,
    items: Vec<(&'static str, ClaimFn, ItemFn)>,
    buckets: Vec<(String, String)>,
}
static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);
fn registry<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    let mut g = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Registry::default))
}

/// Registers (or replaces) a machine block: its capabilities kept by forja and its tick.
/// Call it from your mod's world-load hook (it is idempotent) or before the first tick.
pub(in crate::game) fn register_machine(spec: Spec, tick: Option<TickFn>) {
    registry(|r| {
        let screen = r.kinds.get(&spec.block).and_then(|k| k.screen.clone());
        r.kinds.insert(
            spec.block.clone(),
            Kind {
                spec: Arc::new(spec),
                tick,
                screen,
            },
        );
    });
}
/// The screen of a registered machine block.
pub(in crate::game) fn register_screen(block: &str, screen: Screen) {
    registry(|r| {
        if let Some(k) = r.kinds.get_mut(block) {
            k.screen = Some(Arc::new(screen));
        }
    });
}
/// Another mod's energy blocks (its own storage). `id` = your mod id; registering again replaces.
pub(in crate::game) fn register_energy(id: &'static str, claims: ClaimFn, f: EnergyFn) {
    registry(|r| {
        r.energy.retain(|e| e.0 != id);
        r.energy.push((id, claims, f));
    });
}
pub(in crate::game) fn register_fluid(id: &'static str, claims: ClaimFn, f: FluidFn) {
    registry(|r| {
        r.fluid.retain(|e| e.0 != id);
        r.fluid.push((id, claims, f));
    });
}
pub(in crate::game) fn register_items(id: &'static str, claims: ClaimFn, f: ItemFn) {
    registry(|r| {
        r.items.retain(|e| e.0 != id);
        r.items.push((id, claims, f));
    });
}
/// A mod fluid's bucket item (`ForgeFlowingFluid` buckets) for `bucket_use`.
pub(in crate::game) fn register_bucket(bucket: &str, fluid: &str) {
    registry(|r| {
        if !r.buckets.iter().any(|b| b.0 == bucket) {
            r.buckets.push((bucket.to_owned(), fluid.to_owned()));
        }
    });
}
pub(in crate::game) fn spec(block: &str) -> Option<Arc<Spec>> {
    registry(|r| r.kinds.get(block).map(|k| k.spec.clone()))
}
fn screen_of(block: &str) -> Option<Arc<Screen>> {
    registry(|r| r.kinds.get(block).and_then(|k| k.screen.clone()))
}

// ---- the machine table of the current world and dimension ----

type Table = engine::Machines<ItemStack>;
/// One table per (world folder, dimension). Global, so the screen drawn by the window and the
/// ticks see the same machines; `IN_TABLE` turns a re-entrant call into "not here" instead of a
/// deadlock (a machine's tick or a provider calling forja while the table is held).
static STATE: Mutex<kit_state::PerWorld<Table>> = Mutex::new(kit_state::PerWorld::new());
thread_local! {
    static IN_TABLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn locked<R>(f: impl FnOnce(&mut kit_state::PerWorld<Table>) -> R) -> Option<R> {
    if IN_TABLE.with(|b| b.replace(true)) {
        return None;
    }
    let r = {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s)
    };
    IN_TABLE.with(|b| b.set(false));
    Some(r)
}
fn load(text: Option<String>) -> Table {
    text.map(|t| engine::decode(&t, &NbtCodec)).unwrap_or_default()
}
/// The table, unless it is already borrowed (a re-entrant call: answered as "not here", never a
/// panic) or there is no local world (a remote server's client). A write marks the level for the
/// next save (handlers change energy and fluids without telling); a read marks it only when the
/// table itself changed (a machine made on first sight, a removal, drops).
fn table<R>(game: &Game<'_>, write: bool, f: impl FnOnce(&mut Table) -> R) -> Option<R> {
    // A server's client: the machine of the forja menu it shows (the server's copy).
    if game.remote.is_some() {
        return guest_table(f);
    }
    kit_state::dir(game)?;
    locked(|s| {
        let t = s.level(game, load);
        let r = f(t);
        if write || t.dirty() {
            t.clean();
            s.dirty(game);
        }
        r
    })
}
fn with_table<R>(game: &Game<'_>, f: impl FnOnce(&mut Table) -> R) -> Option<R> {
    table(game, true, f)
}
fn read_table<R>(game: &Game<'_>, f: impl FnOnce(&mut Table) -> R) -> Option<R> {
    table(game, false, f)
}

/// The block at `p` by name (loaded chunks only); a host's signal twin reads as its host block.
fn block_name(game: &Game<'_>, p: Pos) -> Option<Arc<str>> {
    if !game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4)) {
        return None;
    }
    let name = game.res.facts().get(game.world.block(p[0], p[1], p[2])).name.clone();
    match twin_base(&name) {
        Some(base) => Some(Arc::from(base.as_str())),
        None => Some(name),
    }
}
/// The registered machine spec of the block standing at `p` (loaded chunks only).
pub(in crate::game) fn machine_spec_at(game: &Game<'_>, p: Pos) -> Option<Arc<Spec>> {
    spec(&block_name(game, p)?)
}

/// The state of the machine at `pos` (made on first use), if a registered machine block stands
/// there. Do not call other forja functions inside `f` (the table is borrowed): read what you
/// need, return, then move things.
pub(in crate::game) fn with_machine<R>(
    game: &Game<'_>,
    pos: Pos,
    f: impl FnOnce(&mut Machine<ItemStack>) -> R,
) -> Option<R> {
    // A server's client: the forja menu's machine as the server sent it (changes stay here).
    if game.remote.is_some() {
        return guest_table(|t| t.map.get_mut(&pos).map(f)).flatten();
    }
    let spec = machine_spec_at(game, pos)?;
    with_table(game, |t| f(t.ensure(pos, &spec)))
}

// ---- the dispatch: a capability of any block ----

fn direction(d: Dir) -> Direction {
    match d {
        Dir::Down => Direction::Down,
        Dir::Up => Direction::Up,
        Dir::North => Direction::North,
        Dir::South => Direction::South,
        Dir::West => Direction::West,
        Dir::East => Direction::East,
    }
}

/// The energy handler of the block at `pos` seen from `face`, for one operation.
pub(in crate::game) fn with_energy(
    game: &mut Game<'_>,
    pos: Pos,
    face: Option<Dir>,
    f: &mut dyn FnMut(&mut dyn EnergyHandler),
) -> bool {
    if let Some(spec) = machine_spec_at(game, pos) {
        return with_table(game, |t| match t.ensure(pos, &spec).energy_handler(face) {
            Some(mut h) => {
                f(&mut h);
                true
            }
            None => false,
        })
        .unwrap_or(false);
    }
    let providers = registry(|r| r.energy.clone());
    providers.into_iter().any(|(_, claims, p)| claims(game, pos, face) && p(game, pos, face, f))
}
pub(in crate::game) fn with_fluid(
    game: &mut Game<'_>,
    pos: Pos,
    face: Option<Dir>,
    f: &mut dyn FnMut(&mut dyn FluidHandler),
) -> bool {
    if let Some(spec) = machine_spec_at(game, pos) {
        return with_table(game, |t| match t.ensure(pos, &spec).fluid_handler(face) {
            Some(mut h) => {
                f(&mut h);
                true
            }
            None => false,
        })
        .unwrap_or(false);
    }
    let providers = registry(|r| r.fluid.clone());
    providers.into_iter().any(|(_, claims, p)| claims(game, pos, face) && p(game, pos, face, f))
}
/// Items: a forja machine's side-table slots, another mod's provider, or the block's vanilla
/// container (chests, barrels, furnaces with their faces, hoppers, Iron Chests, Ender Chests...).
pub(in crate::game) fn with_items(
    game: &mut Game<'_>,
    pos: Pos,
    face: Option<Dir>,
    f: &mut dyn FnMut(&mut dyn ItemHandler<ItemStack>),
) -> bool {
    if let Some(spec) = machine_spec_at(game, pos)
        && !spec.slots.is_empty()
    {
        let items = game.res.items();
        let max = |s: &ItemStack| items.get(&s.item).map_or(64, |i| i.max_stack).max(1);
        return with_table(game, |t| match t.ensure(pos, &spec).item_handler(face, &max) {
            Some(mut h) => {
                f(&mut h);
                true
            }
            None => false,
        })
        .unwrap_or(false);
    }
    let providers = registry(|r| r.items.clone());
    if providers.into_iter().any(|(_, claims, p)| claims(game, pos, face) && p(game, pos, face, f)) {
        return true;
    }
    if block_name(game, pos).is_none() {
        return false;
    }
    let Some(view) = game.containers().view((pos[0], pos[1], pos[2]), true) else {
        return false;
    };
    if matches!(view.kind, super::Kind::Composter(_)) {
        // A composter has no item handler in 1.20.1 (Pipez skips it too).
        return false;
    }
    let mut h = Vanilla {
        game,
        slots: view.slots_for_face(face.map(direction)),
        view,
        face: face.map(direction),
    };
    f(&mut h);
    true
}

/// `InvWrapper` / `SidedInvWrapper` over a vanilla container view.
struct Vanilla<'a, 'g> {
    game: &'a mut Game<'g>,
    view: View,
    slots: Vec<usize>,
    face: Option<Direction>,
}
impl ItemHandler<ItemStack> for Vanilla<'_, '_> {
    fn slots(&self) -> usize {
        self.slots.len()
    }
    fn stack_in_slot(&self, slot: usize) -> Option<ItemStack> {
        let real = *self.slots.get(slot)?;
        self.game.containers().get(&self.view, real).cloned()
    }
    fn slot_limit(&self, _slot: usize) -> u32 {
        64
    }
    fn is_item_valid(&self, slot: usize, stack: &ItemStack) -> bool {
        self.slots
            .get(slot)
            .is_some_and(|r| self.game.containers().can_place(&self.view, *r, stack))
    }
    fn insert_item(&mut self, slot: usize, stack: ItemStack, simulate: bool) -> Option<ItemStack> {
        if stack.count == 0 {
            return None;
        }
        let Some(&real) = self.slots.get(slot) else {
            return Some(stack);
        };
        let (max, current) = {
            let c = self.game.containers();
            if !c.can_place(&self.view, real, &stack) {
                return Some(stack);
            }
            (c.max_stack(&stack), c.get(&self.view, real).cloned())
        };
        let room = match &current {
            None => max,
            Some(cur) if cur.same_item_same_components(&stack) => max.saturating_sub(cur.count),
            Some(_) => 0,
        };
        let n = stack.count.min(room);
        if n == 0 {
            return Some(stack);
        }
        if !simulate {
            let Some(cell) = self.game.slot_mut_at(&self.view, real) else {
                return Some(stack);
            };
            match cell {
                Some(cur) => cur.count += n,
                None => {
                    *cell = Some(ItemStack {
                        count: n,
                        ..stack.clone()
                    })
                }
            }
            self.game.view_changed(&self.view);
        }
        (stack.count > n).then(|| ItemStack {
            count: stack.count - n,
            ..stack
        })
    }
    fn extract_item(&mut self, slot: usize, amount: u32, simulate: bool) -> Option<ItemStack> {
        let &real = self.slots.get(slot)?;
        let got = {
            let c = self.game.containers();
            let stack = c.get(&self.view, real)?;
            // WorldlyContainer.canTakeItemThroughFace (SidedInvWrapper.extractItem).
            if let Some(face) = self.face
                && !c.can_take(&self.view, real, stack, face)
            {
                return None;
            }
            let n = stack.count.min(amount).min(c.max_stack(stack));
            if n == 0 {
                return None;
            }
            ItemStack {
                count: n,
                ..stack.clone()
            }
        };
        if !simulate {
            let cell = self.game.slot_mut_at(&self.view, real)?;
            let current = cell.as_mut()?;
            current.count -= got.count;
            if current.count == 0 {
                *cell = None;
            }
            self.game.view_changed(&self.view);
        }
        Some(got)
    }
}

/// The game as transport sees it (engine::Grid over the dispatch above).
struct Live<'a, 'g> {
    game: &'a mut Game<'g>,
}
impl engine::Grid for Live<'_, '_> {
    type Stack = ItemStack;
    fn energy(&mut self, p: Pos, face: Option<Dir>, f: &mut dyn FnMut(&mut dyn EnergyHandler)) -> bool {
        with_energy(self.game, p, face, f)
    }
    fn fluid(&mut self, p: Pos, face: Option<Dir>, f: &mut dyn FnMut(&mut dyn FluidHandler)) -> bool {
        with_fluid(self.game, p, face, f)
    }
    fn items(&mut self, p: Pos, face: Option<Dir>, f: &mut dyn FnMut(&mut dyn ItemHandler<ItemStack>)) -> bool {
        with_items(self.game, p, face, f)
    }
}

// ---- the API of single operations (what a cable or a machine calls) ----

/// The block at `p` has energy on `face` (forja machine side rules, or a provider's claim).
/// Only reads: usable from `&Game` (a pipe deciding its connections).
pub(in crate::game) fn has_energy(game: &Game<'_>, p: Pos, face: Option<Dir>) -> bool {
    if let Some(spec) = machine_spec_at(game, p) {
        return read_table(game, |t| t.ensure(p, &spec).energy_handler(face).is_some()).unwrap_or(false);
    }
    registry(|r| r.energy.clone()).into_iter().any(|(_, claims, _)| claims(game, p, face))
}
pub(in crate::game) fn energy_info(game: &mut Game<'_>, p: Pos, face: Option<Dir>) -> Option<EnergyInfo> {
    engine::energy_info(&mut Live { game }, p, face)
}
pub(in crate::game) fn energy_receive(game: &mut Game<'_>, p: Pos, face: Option<Dir>, max: i32, simulate: bool) -> i32 {
    engine::energy_receive(&mut Live { game }, p, face, max, simulate)
}
pub(in crate::game) fn energy_extract(game: &mut Game<'_>, p: Pos, face: Option<Dir>, max: i32, simulate: bool) -> i32 {
    engine::energy_extract(&mut Live { game }, p, face, max, simulate)
}
pub(in crate::game) fn has_fluid(game: &Game<'_>, p: Pos, face: Option<Dir>) -> bool {
    if let Some(spec) = machine_spec_at(game, p) {
        return read_table(game, |t| t.ensure(p, &spec).fluid_handler(face).is_some()).unwrap_or(false);
    }
    registry(|r| r.fluid.clone()).into_iter().any(|(_, claims, _)| claims(game, p, face))
}
pub(in crate::game) fn fluid_tanks(game: &mut Game<'_>, p: Pos, face: Option<Dir>) -> Option<Vec<TankInfo>> {
    engine::fluid_tanks(&mut Live { game }, p, face)
}
pub(in crate::game) fn fluid_fill(game: &mut Game<'_>, p: Pos, face: Option<Dir>, s: &FluidStack, simulate: bool) -> i32 {
    engine::fluid_fill(&mut Live { game }, p, face, s, simulate)
}
pub(in crate::game) fn fluid_drain(
    game: &mut Game<'_>,
    p: Pos,
    face: Option<Dir>,
    s: &FluidStack,
    simulate: bool,
) -> FluidStack {
    engine::fluid_drain(&mut Live { game }, p, face, s, simulate)
}
pub(in crate::game) fn fluid_drain_max(
    game: &mut Game<'_>,
    p: Pos,
    face: Option<Dir>,
    max: i32,
    simulate: bool,
) -> FluidStack {
    engine::fluid_drain_max(&mut Live { game }, p, face, max, simulate)
}
pub(in crate::game) fn has_items(game: &Game<'_>, p: Pos, face: Option<Dir>) -> bool {
    if let Some(spec) = machine_spec_at(game, p)
        && !spec.slots.is_empty()
    {
        return read_table(game, |t| t.ensure(p, &spec).item_sides.get(face) != Access::None).unwrap_or(false);
    }
    if registry(|r| r.items.clone()).into_iter().any(|(_, claims, _)| claims(game, p, face)) {
        return true;
    }
    block_name(game, p).is_some()
        && game
            .containers()
            .view((p[0], p[1], p[2]), true)
            .is_some_and(|v| !matches!(v.kind, super::Kind::Composter(_)))
}
/// `ItemHandlerHelper.insertItemStacked` into the block at `p`: the remainder.
pub(in crate::game) fn item_insert(
    game: &mut Game<'_>,
    p: Pos,
    face: Option<Dir>,
    stack: ItemStack,
    simulate: bool,
) -> Option<ItemStack> {
    let mut rest = Some(stack.clone());
    with_items(game, p, face, &mut |h| rest = engine::insert_item_stacked(h, stack.clone(), simulate));
    rest
}
pub(in crate::game) fn item_extract(
    game: &mut Game<'_>,
    p: Pos,
    face: Option<Dir>,
    slot: usize,
    n: u32,
    simulate: bool,
) -> Option<ItemStack> {
    let mut out = None;
    with_items(game, p, face, &mut |h| out = h.extract_item(slot, n, simulate));
    out
}

/// Energy between two blocks (simulated on both sides first). Returns the FE moved.
pub(in crate::game) fn transfer_energy(game: &mut Game<'_>, from: (Pos, Option<Dir>), to: (Pos, Option<Dir>), max: i32) -> i32 {
    engine::transfer_energy(&mut Live { game }, from, to, max)
}
pub(in crate::game) fn transfer_fluid(
    game: &mut Game<'_>,
    from: (Pos, Option<Dir>),
    to: (Pos, Option<Dir>),
    resource: Option<&FluidStack>,
    max: i32,
) -> FluidStack {
    engine::transfer_fluid(&mut Live { game }, from, to, resource, max)
}
/// Items between two blocks; anything that could go neither in nor back is dropped at `from`.
pub(in crate::game) fn transfer_items(game: &mut Game<'_>, from: (Pos, Option<Dir>), to: (Pos, Option<Dir>), max: u32) -> u32 {
    let (n, stray) = engine::transfer_items(&mut Live { game }, from, to, max);
    for s in stray {
        drop_at(game, from.0, s);
    }
    n
}
/// A generator/battery output: up to `max` FE from `p` to every neighbour that takes it.
pub(in crate::game) fn push_energy_around(game: &mut Game<'_>, p: Pos, max: i32) -> i32 {
    engine::push_energy_around(&mut Live { game }, p, max)
}
pub(in crate::game) fn energy_neighbours(game: &mut Game<'_>, p: Pos) -> Vec<Dir> {
    engine::energy_neighbours(&mut Live { game }, p)
}
/// `FluidUtil.interactWithFluidHandler` with the held bucket on the block at `p`: what happened
/// (the caller swaps the held item and plays the sound, as enderstorage's tank does).
pub(in crate::game) fn bucket_use(game: &mut Game<'_>, p: Pos, face: Option<Dir>, held: &str) -> BucketUse {
    let extra = registry(|r| r.buckets.clone());
    let mut out = BucketUse::Nothing;
    with_fluid(game, p, face, &mut |h| out = engine::bucket_interact(held, h, &extra));
    out
}

fn drop_at(game: &mut Game<'_>, p: Pos, stack: ItemStack) {
    game.spawn_item(
        stack,
        [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5],
        super::super::dispenser::new_item_motion(),
        10,
    );
}

// ---- the hooks ----

impl Game<'_> {
    fn forja_active(&self) -> bool {
        self.remote.is_none() && !self.world_root.as_os_str().is_empty()
    }
    /// `BlockItem.place`'s last step: a placed machine gets its state at once.
    pub(in crate::game) fn forja_placed(&mut self, trigger: &str, pos: [i32; 3]) {
        if trigger != "minecraft:placed_block" || !self.forja_active() {
            return;
        }
        with_machine(self, pos, |_| ());
    }
    /// Each tick, before the block entities: machines whose block is gone drop their items, the
    /// table's pending drops are spawned, then every registered machine in a block-ticking chunk
    /// runs its tick.
    pub(in crate::game) fn forja_tick(&mut self) {
        // Model shapes are the window's too (a LAN guest's client draws and predicts with them).
        self.forja_model_shapes();
        if !self.forja_active() {
            return;
        }
        self.forja_hosts_tick();
        self.forja_redstone_prune();
        let Some(positions) = read_table(self, |t| t.map.iter().map(|(p, m)| (*p, m.kind.clone())).collect::<Vec<_>>())
        else {
            return;
        };
        let mut gone = Vec::new();
        let mut ticking = Vec::new();
        for (p, kind) in positions {
            let Some(name) = block_name(self, p) else {
                continue; // unloaded: nothing changes there
            };
            if *name != *kind {
                gone.push(p);
                continue;
            }
            if !self.tickets.levels().block_ticking_at(p[0], p[2]) {
                continue;
            }
            if let Some(tick) = registry(|r| r.kinds.get(&*kind).and_then(|k| k.tick)) {
                ticking.push((p, tick));
            }
        }
        let drops = read_table(self, |t| {
            for p in &gone {
                t.remove(*p);
            }
            t.take_drops()
        })
        .unwrap_or_default();
        for (p, s) in drops {
            drop_at(self, p, s);
        }
        for (p, tick) in ticking {
            tick(self, p);
        }
    }
    /// `save_game_metadata`: the machine tables of this world.
    pub(in crate::game) fn forja_save(&mut self) {
        let game: &Game<'_> = self;
        locked(|s| s.save(game, |t| engine::encode(t, &NbtCodec)));
    }
    /// `load_world_state`: another world opens; its tables load on first use. The registries
    /// stay (they belong to the game binary, not to a world).
    pub(in crate::game) fn forja_world_opened() {
        locked(|s| s.clear());
        // The guests and the signals were the other world's.
        *SERVER_NET.lock().unwrap_or_else(|e| e.into_inner()) = None;
        redstone_forget_all();
    }

    // ---- the screen of an open machine (vanilla container menu + the mod's texture) ----

    fn forja_open_screen(&self) -> Option<(Pos, Arc<Screen>)> {
        let key = self.container?;
        let p = [key.0, key.1, key.2];
        Some((p, screen_of(&block_name(self, p)?)?))
    }
    pub(in crate::game) fn forja_screen_height(&self) -> Option<f32> {
        self.forja_open_screen().map(|(_, s)| s.size.1 as f32)
    }
    /// Container slots where the screen puts them; the player's inventory under the image as in
    /// every 176-wide container screen (`imageHeight - 82`, hotbar 58 lower).
    pub(in crate::game) fn forja_slot_position(&self, i: usize) -> Option<Option<(f32, f32)>> {
        let (_, s) = self.forja_open_screen()?;
        let slots = s.slots.as_ref()?;
        let inv_y = (s.size.1 - 82) as f32;
        Some(match i {
            0..9 => Some((8.0 + i as f32 * 18.0, inv_y + 58.0)),
            9..36 => Some((8.0 + ((i - 9) % 9) as f32 * 18.0, inv_y + ((i - 9) / 9) as f32 * 18.0)),
            300..500 => slots.get(i - 300).map(|&(x, y)| (x as f32, y as f32)),
            _ => None,
        })
    }
    /// The background, the bars (`blit` of the part of the texture the value shows) and labels.
    pub(in crate::game) fn forja_draw_screen(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some((p, s)) = self.forja_open_screen() else {
            return false;
        };
        let atlas = &self.gui.atlas;
        let (tw, th) = (s.texture_size.0 as f32, s.texture_size.1 as f32);
        let Some(rect) = atlas.sprites.get(s.texture).copied() else {
            // The texture is not in the GUI atlas: a plain panel, as ironchest does.
            batch.fill(atlas, ox, oy, s.size.0 as f32, s.size.1 as f32, horizon_client::hud::rgba(198, 198, 198, 255));
            return true;
        };
        let blit = |batch: &mut HudBatch, x: i32, y: i32, u: i32, v: i32, w: i32, h: i32| {
            let mut r = rect;
            let (du, dv) = ((rect.u1 - rect.u0) / tw, (rect.v1 - rect.v0) / th);
            r.u0 = rect.u0 + u as f32 * du;
            r.v0 = rect.v0 + v as f32 * dv;
            r.u1 = r.u0 + w as f32 * du;
            r.v1 = r.v0 + h as f32 * dv;
            batch.rect_uv(ox + x as f32, oy + y as f32, w as f32, h as f32, r, u32::MAX);
        };
        blit(batch, 0, 0, 0, 0, s.size.0, s.size.1);
        let values: Vec<(i32, i32)> = read_table(self, |t| {
            let m = t.map.get(&p);
            s.bars
                .iter()
                .map(|(_, g)| match (g, m) {
                    (_, None) => (0, 0),
                    (Gauge::Progress, Some(m)) => (m.progress, m.max_progress),
                    (Gauge::Energy, Some(m)) => m.energy.map_or((0, 0), |e| (e.energy, e.capacity)),
                    (Gauge::Tank(i), Some(m)) => m.tanks.get(*i).map_or((0, 0), |t| (t.amount(), t.capacity)),
                    (Gauge::Data(v, max), Some(m)) => (m.get(v) as i32, m.get(max) as i32),
                })
                .collect()
        })
        .unwrap_or_default();
        for ((bar, _), (v, max)) in s.bars.iter().zip(values) {
            if let Some([x, y, u, vv, w, h]) = bar.part(v, max) {
                blit(batch, x, y, u, vv, w, h);
            }
        }
        let grey = horizon_client::hud::rgba(64, 64, 64, 255);
        let title = self.container_title(self.translate(s.title_key, s.title_fallback));
        let title_x = (s.size.0 as f32 - horizon_client::hud::HudBatch::text_width(atlas, &title)) / 2.0;
        batch.text(atlas, &title, ox + title_x, oy + 6.0, grey, false);
        let inventory = self.translate("container.inventory", "Inventory");
        batch.text(atlas, &inventory, ox + 8.0, oy + s.size.1 as f32 - 94.0, grey, false);
        true
    }
}

// ---- compound blocks (multipart): hosts, parts, shapes, the crosshair, placing and breaking ----
//
// A host is a block of the owning mod (`ae2:cable_bus`, template `minecraft:barrier`: invisible,
// full outline and collision that the hooks replace) plus a `Host` record in forja's table (saved
// with the machines, `h`/`hp` lines). The library module `mesher::forja_shapes` holds what the
// mesher, the collision scan and the outline renderer need; this side keeps it in step.

use horizon_client::mesher::forja_shapes;

/// A host block kind, registered by the mod that owns it (`register_host`).
#[derive(Clone, Debug)]
pub(in crate::game) struct HostSpec {
    pub block: &'static str,
    /// Mining breaks every part at once (AE2 15.4.11: `CableBusBlock.getDrops` = every part's
    /// item); false: only the targeted part goes.
    pub break_whole: bool,
    /// Sneak + use with one of these items (or an item of `wrench_tag`) takes the targeted part
    /// into the inventory (AE2 `WrenchHook` -> `disassembleWithWrench`).
    pub wrench_tag: Option<&'static str>,
    pub wrench_items: &'static [&'static str],
    /// Played when a part is placed (AE2: the cable bus's place sound, volume 1, pitch 0.8).
    pub place_sound: &'static str,
    /// The twin block the host becomes while one of its parts emits redstone (`set_part_redstone`):
    /// the same block declared with template `minecraft:trapped_chest`, the only kind of state whose
    /// signal Horizon asks per position (FUNDATIE-FORJA.md, "Redstone"). `None`: the host never emits.
    pub signal_block: Option<&'static str>,
}
/// Does the centre part `part` of the host at `pos` link towards `side`? `neighbour` = the centre
/// of the host there (`None`: an ordinary block, e.g. an ME Drive). Both hosts' faces are already
/// known free. Without a callback, centres of the same host block link and blocks do not.
pub(in crate::game) type JoinFn = for<'a, 'g, 'p> fn(&'a Game<'g>, Pos, &'p Part, Dir, Option<&'p Part>) -> bool;

#[derive(Default)]
struct Parts {
    hosts: BTreeMap<String, HostSpec>,
    kinds: BTreeMap<String, (Arc<PartSpec>, Option<JoinFn>)>,
    /// Item -> part id.
    items: BTreeMap<String, String>,
}
static PARTS: Mutex<Option<Parts>> = Mutex::new(None);
fn parts_registry<R>(f: impl FnOnce(&mut Parts) -> R) -> R {
    let mut g = PARTS.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Parts::default))
}
/// Changes whenever any host changes (mods resync their own records when it moves).
static HOSTS_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn bump() {
    HOSTS_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Registers (or replaces) a host block kind. Idempotent: call it from your world-load hook.
pub(in crate::game) fn register_host(spec: HostSpec) {
    if let Some(twin) = spec.signal_block {
        let mut t = TWINS.lock().unwrap_or_else(|e| e.into_inner());
        t.retain(|(b, _)| b != spec.block);
        t.push((spec.block.to_owned(), twin.to_owned()));
        HAS_TWINS.store(true, Ordering::Relaxed);
    }
    parts_registry(|r| {
        r.hosts.insert(spec.block.to_owned(), spec);
    });
}
/// Registers (or replaces) a part kind, placed by `spec.item`.
pub(in crate::game) fn register_part(spec: PartSpec, join: Option<JoinFn>) {
    parts_registry(|r| {
        r.items.insert(spec.item.clone(), spec.id.clone());
        r.kinds.insert(spec.id.clone(), (Arc::new(spec), join));
    });
}
pub(in crate::game) fn part_spec(id: &str) -> Option<Arc<PartSpec>> {
    parts_registry(|r| r.kinds.get(id).map(|k| k.0.clone()))
}
fn host_spec(block: &str) -> Option<HostSpec> {
    parts_registry(|r| r.hosts.get(block).cloned())
}
type Specs = BTreeMap<String, Arc<PartSpec>>;
fn specs() -> Specs {
    parts_registry(|r| r.kinds.iter().map(|(k, v)| (k.clone(), v.0.clone())).collect())
}
fn spec_fn(s: &Specs) -> impl Fn(&str) -> Option<PartSpec> + '_ {
    move |id: &str| s.get(id).map(|p| (**p).clone())
}
/// `hosts_generation()` changes on every host change.
pub(in crate::game) fn hosts_generation() -> u64 {
    HOSTS_GEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// The host at `p` (a copy), if its block stands there (loaded chunks only).
pub(in crate::game) fn host_at(game: &Game<'_>, p: Pos) -> Option<Host> {
    let name = block_name(game, p)?;
    let h = read_table(game, |t| t.hosts.get(&p).cloned())??;
    (*h.block == *name).then_some(h)
}
/// The id of the part in `slot` of the host at `p` (cheaper than `host_at`: nothing is copied).
pub(in crate::game) fn part_at(game: &Game<'_>, p: Pos, slot: Slot) -> Option<String> {
    let name = block_name(game, p)?;
    read_table(game, |t| t.hosts.get(&p).filter(|h| *h.block == *name).and_then(|h| h.get(slot)).map(|x| x.id.clone()))?
}
/// Every host of `block` in the current dimension (loaded or not).
pub(in crate::game) fn hosts_of(game: &Game<'_>, block: &str) -> Vec<(Pos, Host)> {
    read_table(game, |t| t.hosts.iter().filter(|(_, h)| h.block == block).map(|(p, h)| (*p, h.clone())).collect())
        .unwrap_or_default()
}
/// Changes a part's numbers (`Part::set`); false when there is no such part.
pub(in crate::game) fn set_part_data(game: &Game<'_>, p: Pos, slot: Slot, key: &str, v: i64) -> bool {
    with_table(game, |t| match t.hosts.get_mut(&p).and_then(|h| h.get_mut(slot)) {
        Some(part) => {
            part.set(key, v);
            true
        }
        None => false,
    })
    .unwrap_or(false)
}
/// The links of the host's centre (what its arms reach).
pub(in crate::game) fn links_of(game: &Game<'_>, p: Pos) -> [bool; 6] {
    match host_at(game, p) {
        Some(h) => host_links(game, p, &h, &|q| host_at(game, q)),
        None => [false; 6],
    }
}
fn host_links(game: &Game<'_>, pos: Pos, host: &Host, lookup: &dyn Fn(Pos) -> Option<Host>) -> [bool; 6] {
    let Some(center) = &host.center else {
        return [false; 6];
    };
    let join = parts_registry(|r| r.kinds.get(&center.id).and_then(|k| k.1));
    engine::links(host, |d| {
        let q = engine::offset(pos, d);
        if block_name(game, q).is_none() {
            return false;
        }
        match lookup(q) {
            Some(n) => {
                if n.blocked(d.opposite()) {
                    return false;
                }
                let Some(nc) = &n.center else { return false };
                match join {
                    Some(f) => f(game, pos, center, d, Some(nc)),
                    None => n.block == host.block,
                }
            }
            None => join.is_some_and(|f| f(game, pos, center, d, None)),
        }
    })
}

/// The block state named `name` with `props` (the template's other properties at their defaults).
fn state_of(game: &Game<'_>, name: &str, props: &[(String, String)]) -> Option<u32> {
    let registry = &game.pack.assets.registry;
    let mut s = registry.state(registry.default_state(name)?).cloned()?;
    for (k, v) in props {
        if s.properties.contains_key(k) {
            s.properties.insert(k.clone(), v.clone());
        }
    }
    registry.resolve(&s).ok().map(|id| id.0)
}
fn mark_section(game: &mut Game<'_>, p: Pos) {
    let s = (p[1] - horizon_client::chunk::MIN_Y) >> 4;
    if (0..horizon_client::chunk::SECTIONS as i32).contains(&s) {
        game.world.dirty_sections.insert((p[0] >> 4, s, p[2] >> 4));
    }
}
fn slot_code(s: Slot) -> forja_shapes::SlotCode {
    s.map_or(forja_shapes::CENTER, |d| d.idx() as u8)
}
fn code_slot(c: forja_shapes::SlotCode) -> Slot {
    Dir::ALL.get(usize::from(c)).copied()
}

/// What the library draws and collides for one host: the part models' states, outline boxes
/// with slots, collision boxes. A hash of it tells whether the meshes must be rebuilt.
fn host_view(game: &Game<'_>, pos: Pos, host: &Host, specs: &Specs, lookup: &dyn Fn(Pos) -> Option<Host>) -> (Vec<u32>, Vec<(u8, [f64; 6])>, Vec<[f64; 6]>) {
    let spec_of = spec_fn(specs);
    let links = host_links(game, pos, host, lookup);
    let arms = engine::center_arms(host, &spec_of, links);
    let mut states = Vec::new();
    for (slot, part) in host.parts() {
        if let Some(spec) = spec_of(&part.id) {
            let (block, props) = engine::look(&spec, slot, arms);
            if let Some(id) = state_of(game, &block, &props) {
                states.push(id);
            }
        }
    }
    let outline = engine::host_boxes(host, &spec_of, links, false).into_iter().map(|(s, b)| (slot_code(s), b)).collect();
    let collision = engine::host_boxes(host, &spec_of, links, true).into_iter().map(|(_, b)| b).collect();
    (states, outline, collision)
}

thread_local! {
    /// The world/dimension the library's host table belongs to, the looks already given to it,
    /// and the key of the model shapes (registry size, model catalog).
    static MP: std::cell::RefCell<MpState> = std::cell::RefCell::new(MpState::default());
}
#[derive(Default)]
struct MpState {
    key: String,
    looks: std::collections::HashMap<Pos, u64>,
    models: (usize, usize),
    host_kinds: usize,
    tick: u64,
}
fn view_hash(v: &(Vec<u32>, Vec<(u8, [f64; 6])>, Vec<[f64; 6]>)) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.0.hash(&mut h);
    for (s, b) in &v.1 {
        s.hash(&mut h);
        b.map(f64::to_bits).hash(&mut h);
    }
    for b in &v.2 {
        b.map(f64::to_bits).hash(&mut h);
    }
    h.finish()
}
/// `p` and its six neighbours (their arms may change).
pub(in crate::game) fn refresh_parts(game: &mut Game<'_>, p: Pos) {
    let specs = specs();
    let around: Vec<Pos> = std::iter::once(p).chain(Dir::ALL.iter().map(|d| engine::offset(p, *d))).collect();
    for q in around {
        let view = host_at(game, q).map(|h| {
            let v = host_view(game, q, &h, &specs, &|r| host_at(game, r));
            (view_hash(&v), v)
        });
        apply_view(game, q, view);
    }
}
fn apply_view(game: &mut Game<'_>, p: Pos, view: Option<(u64, (Vec<u32>, Vec<(u8, [f64; 6])>, Vec<[f64; 6]>))>) {
    match view {
        Some((hash, v)) => {
            if MP.with(|m| m.borrow().looks.get(&p) == Some(&hash)) {
                return;
            }
            MP.with(|m| m.borrow_mut().looks.insert(p, hash));
            forja_shapes::set_host(p, v.0, v.1, &v.2);
        }
        None => {
            if MP.with(|m| m.borrow_mut().looks.remove(&p)).is_none() {
                return;
            }
            forja_shapes::remove_host(p);
        }
    }
    mark_section(game, p);
}

/// Puts `part` in `slot` of the host at `p`, placing the host block first when the position has
/// none (the block there must be replaceable). False when it does not fit.
pub(in crate::game) fn put_part(game: &mut Game<'_>, p: Pos, slot: Slot, part: Part) -> bool {
    let Some(spec) = part_spec(&part.id) else { return false };
    let host_block = spec.host.clone();
    let mut placed = false;
    if host_at(game, p).is_none() {
        if !game.forja_replaceable(p) {
            return false;
        }
        let Some(state) = game.pack.assets.registry.default_state(&host_block) else { return false };
        // The host record first, so the tick never sees a host block without parts.
        let stale = with_table(game, |t| t.hosts.insert(p, Host::new(&host_block))).flatten();
        if let Some(mut old) = stale {
            // A record whose block went this very tick: its parts drop now.
            let specs = specs();
            let items = engine::part_drops(&old.take_all(), &spec_fn(&specs));
            game.forja_drop_items(p, items);
        }
        let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
        game.with_level(|b, level, ticks| b.set_block(level, ticks, at, state, horizon_world::level::flags::UPDATE_ALL));
        let _ = game.process_level_events();
        if block_name(game, p).as_deref() != Some(host_block.as_str()) {
            with_table(game, |t| t.hosts.remove(&p));
            return false;
        }
        placed = true;
    }
    let ok = with_table(game, |t| match t.hosts.get_mut(&p) {
        Some(h) => {
            let specs = specs();
            let ok = engine::can_add(h, &spec, slot, &spec_fn(&specs)) && h.put(slot, part).is_ok();
            t.changed = true;
            ok
        }
        None => false,
    })
    .unwrap_or(false);
    if !ok && placed {
        game.forja_clear_host(p);
    }
    bump();
    refresh_parts(game, p);
    ok
}
/// Takes the part in `slot` out of the host at `p` (no drop: the caller gives the items); the
/// host block goes when the host is empty.
pub(in crate::game) fn take_part(game: &mut Game<'_>, p: Pos, slot: Slot) -> Vec<(Slot, Part)> {
    if host_at(game, p).is_none() {
        return Vec::new();
    }
    let (removed, empty) = with_table(game, |t| {
        let Some(h) = t.hosts.get_mut(&p) else { return (Vec::new(), false) };
        let r = h.remove(slot);
        let empty = h.is_empty();
        if empty {
            t.hosts.remove(&p);
        }
        t.changed = true;
        (r, empty)
    })
    .unwrap_or_default();
    if empty {
        game.forja_clear_host(p);
    }
    bump();
    refresh_parts(game, p);
    removed
}
/// The part the crosshair holds: (host position, slot, hit).
pub(in crate::game) fn targeted_part(game: &Game<'_>) -> Option<(Pos, Slot)> {
    game.forja_target().map(|(p, s, _)| (p, s))
}
fn item_stack(game: &Game<'_>, item: &str) -> Option<ItemStack> {
    game.res.items().get(item)?;
    Some(ItemStack::new(item, 1))
}

impl Game<'_> {
    /// A block a new host may replace (`canPlacePartHost`: air, plants, water...).
    fn forja_replaceable(&self, p: Pos) -> bool {
        if block_name(self, p).is_none() {
            return false;
        }
        let state = self.world.block(p[0], p[1], p[2]);
        self.res.facts().is_air(state) || self.res.facts().get(state).replaceable
    }
    /// An emptied host: its block becomes air (neighbours are told, nothing drops: the parts were
    /// handled already).
    fn forja_clear_host(&mut self, p: Pos) {
        with_table(self, |t| t.hosts.remove(&p));
        redstone_forget(self, p);
        let air = self.world.air;
        let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, air, horizon_world::level::flags::UPDATE_ALL));
        let _ = self.process_level_events();
    }
    /// The part's own block state for its break particles (the centre's arms do not matter).
    fn forja_part_state(&self, part: &Part, slot: Slot) -> Option<u32> {
        let spec = part_spec(&part.id)?;
        let (block, props) = engine::look(&spec, slot, [None; 6]);
        state_of(self, &block, &props)
    }
    fn forja_drop_items(&mut self, p: Pos, items: Vec<String>) {
        for item in items {
            if let Some(s) = item_stack(self, &item) {
                drop_at(self, p, s);
            }
        }
    }

    /// The crosshair: the vanilla voxel walk (`clip_ray`), but a host's cell is tested against its
    /// parts' boxes and a model-shaped block against its model's boxes; a miss there goes on to the
    /// next cell. Also tells the outline renderer what to draw.
    pub(in crate::game) fn forja_pick(&self) -> Option<Option<crate::game::Hit>> {
        if self.forced_hit.is_some() || !forja_shapes::any() {
            forja_shapes::set_picked(None);
            return None;
        }
        let Some(origin) = self.eye_position() else {
            forja_shapes::set_picked(None);
            return None;
        };
        let dir = crate::game::view_vector(self.yaw, self.pitch);
        let (hit, picked) = self.forja_clip(origin, dir, self.reach());
        forja_shapes::set_picked(picked.map(|(p, _, boxes)| (p, boxes)));
        Some(hit.map(|(pos, face, point)| crate::game::Hit { pos, face, point }))
    }
    /// The targeted part of a host: (position, slot, hit point).
    pub(in crate::game) fn forja_target(&self) -> Option<(Pos, Slot, [f64; 3])> {
        if self.forced_hit.is_some() {
            // A LAN guest's hit: the part under its point.
            let hit = self.forced_hit.as_ref()?;
            let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
            let boxes = forja_shapes::host_outline(state.0, hit.pos)?;
            let local = std::array::from_fn(|k| hit.point[k] - f64::from(hit.pos[k]));
            let tagged: Vec<(Slot, [f64; 6])> = boxes.into_iter().map(|(c, b)| (code_slot(c), b)).collect();
            return engine::select(&tagged, local).map(|s| (hit.pos, s, hit.point));
        }
        let origin = self.eye_position()?;
        let dir = crate::game::view_vector(self.yaw, self.pitch);
        let ((pos, _, point), picked) = match self.forja_clip(origin, dir, self.reach()) {
            (Some(h), Some(p)) => (h, p),
            _ => return None,
        };
        let slot = code_slot(picked.1?);
        Some((pos, slot, point))
    }
    /// The walk itself: the hit (cell, face normal, point) and, when it is a host or a model
    /// shape, (cell, the hit part's slot code, the boxes to outline in world space).
    #[allow(clippy::type_complexity)]
    fn forja_clip(&self, origin: [f64; 3], dir: [f64; 3], reach: f64) -> (Option<(Pos, [i32; 3], [f64; 3])>, Option<(Pos, Option<u8>, Vec<[f64; 6]>)>) {
        let mut pos = origin.map(|v| v.floor() as i32);
        let step = dir.map(|d| if d > 0.0 { 1 } else { -1 });
        let delta = dir.map(|d| if d == 0.0 { f64::INFINITY } else { (1.0 / d).abs() });
        let mut t_max = [0.0; 3];
        for i in 0..3 {
            let boundary = if dir[i] > 0.0 { f64::from(pos[i]) + 1.0 } else { f64::from(pos[i]) };
            t_max[i] = if dir[i] == 0.0 { f64::INFINITY } else { (boundary - origin[i]) / dir[i] };
        }
        let mut face = [0; 3];
        let mut t = 0.0;
        let shift = |b: &[f64; 6], p: Pos| -> [f64; 6] { std::array::from_fn(|i| b[i] + f64::from(p[i % 3])) };
        while t <= reach {
            let state = self.world.block(pos[0], pos[1], pos[2]);
            let f = self.res.facts().get(state);
            let candidate = !self.res.facts().is_air(state)
                && (f.fluid == horizon_client::blocks::Fluid::None
                    || f.waterlogged
                    || &*f.name == "minecraft:kelp"
                    || f.render_shape == horizon_client::blocks::RenderShape::Model)
                && !matches!(&*f.name, "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column");
            if candidate {
                let point = |t: f64| [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t];
                if let Some(parts) = forja_shapes::host_outline(state.0, pos) {
                    let boxes: Vec<[f64; 6]> = parts.iter().map(|(_, b)| shift(b, pos)).collect();
                    if let Some((t, i, n)) = engine::ray_boxes(origin, dir, reach, &boxes, face) {
                        let code = parts[i].0;
                        let mine = parts.iter().filter(|(c, _)| *c == code).map(|(_, b)| shift(b, pos)).collect();
                        return (Some((pos, n, point(t))), Some((pos, Some(code), mine)));
                    }
                } else if let Some(model) = forja_shapes::model_outline(state.0) {
                    let boxes: Vec<[f64; 6]> = model.iter().map(|b| shift(b, pos)).collect();
                    if let Some((t, _, n)) = engine::ray_boxes(origin, dir, reach, &boxes, face) {
                        return (Some((pos, n, point(t))), Some((pos, None, boxes)));
                    }
                } else {
                    let boxes = self.outline_boxes(pos);
                    if let Some((t, _, n)) = engine::ray_boxes(origin, dir, reach, &boxes, face) {
                        return (Some((pos, n, point(t))), None);
                    }
                }
            }
            let axis = if t_max[0] < t_max[1] {
                if t_max[0] < t_max[2] { 0 } else { 2 }
            } else if t_max[1] < t_max[2] {
                1
            } else {
                2
            };
            t = t_max[axis];
            t_max[axis] += delta[axis];
            pos[axis] += step[axis];
            face = [0; 3];
            face[axis] = -step[axis];
        }
        (None, None)
    }

    /// `PartItem.useOn` -> `PartPlacement.place`, and the wrench's `disassembleWithWrench`.
    /// `None` = not a part item nor a wrench on a host: the game goes on.
    pub(in crate::game) fn forja_use_part(&mut self, stack: &ItemStack) -> Option<horizon_client::Result<bool>> {
        if !self.forja_active() {
            return None;
        }
        let sneaking = self.player.as_ref().is_some_and(|p| p.shift_key_down);
        if sneaking && let Some(done) = self.forja_wrench(stack) {
            return Some(Ok(done));
        }
        let id = parts_registry(|r| r.items.get(&*stack.item).cloned())?;
        let spec = part_spec(&id)?;
        // From here the item is a part: the block it also is never gets placed (true even when the
        // part does not fit: `false` would let the game place the item's block).
        self.forja_place(&spec);
        Some(Ok(true))
    }
    fn forja_wrench(&mut self, stack: &ItemStack) -> Option<bool> {
        let (pos, slot, _) = self.forja_target()?;
        let host = host_at(self, pos)?;
        let hs = host_spec(&host.block)?;
        let wrench = hs.wrench_items.contains(&&*stack.item)
            || hs.wrench_tag.is_some_and(|t| self.res.items().item_tags.get(t).is_some_and(|s| s.contains(&*stack.item)));
        if !wrench || !self.may_build() {
            return None;
        }
        let part = host.get(slot)?.clone();
        let particles = self.forja_part_state(&part, slot);
        let removed = take_part(self, pos, slot);
        let specs = specs();
        for item in engine::part_drops(&removed, &spec_fn(&specs)) {
            if let Some(s) = item_stack(self, &item)
                && let Some(rest) = self.give(s)
            {
                let at = self.player.as_ref().map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]);
                self.spawn_item(rest, at, [0.0, 0.0, 0.0], 0);
            }
        }
        if let Some(st) = particles {
            self.level_event_at(2001, pos, st as i32);
        }
        self.audio.play("minecraft:entity.item_frame.remove_item", Some([f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5]), 0.7, 1.0);
        self.swing_arm();
        Some(true)
    }
    /// `PartPlacement.getPartPlacement` + `placePart`: true when the part went in.
    fn forja_place(&mut self, spec: &PartSpec) -> bool {
        let Some(hit) = self.pick() else { return false };
        let Some(face) = Dir::from_normal(hit.face) else { return false };
        if !self.may_build() {
            return false;
        }
        let specs = specs();
        let spec_of = spec_fn(&specs);
        let pos = hit.pos;
        let clicked = host_at(self, pos);
        let clicked_free = clicked.is_none() && self.forja_replaceable(pos);
        let local: [f64; 3] = std::array::from_fn(|k| hit.point[k] - f64::from(pos[k]));
        let arm = clicked.as_ref().and_then(|h| {
            let links = host_links(self, pos, h, &|q| host_at(self, q));
            engine::arm_at(h, &spec_of, links, local)
        });
        let beside = engine::offset(pos, face);
        let near = host_at(self, beside);
        let beside_free = near.is_none() && self.forja_replaceable(beside);
        let Some((target, slot)) =
            engine::placement(spec, &spec_of, clicked.as_ref(), clicked_free, arm, face, near.as_ref(), beside_free)
        else {
            return false;
        };
        let at = match target {
            engine::Target::Clicked => pos,
            engine::Target::Beside => beside,
        };
        // `level.isUnobstructed`: the new part's boxes may not cut into the player.
        if let Some(p) = &self.player {
            let w = f64::from(p.dimensions.width) / 2.0;
            let h = f64::from(p.dimensions.height);
            let body = [p.position.x - w, p.position.y, p.position.z - w, p.position.x + w, p.position.y + h, p.position.z + w];
            let boxes = engine::part_boxes(spec, slot, [None; 6], true);
            let hits = boxes.iter().any(|b| (0..3).all(|k| b[k] + f64::from(at[k]) < body[k + 3] && b[k + 3] + f64::from(at[k]) > body[k]));
            if hits {
                return false;
            }
        }
        if !put_part(self, at, slot, Part::new(&spec.id)) {
            return false;
        }
        if !self.creative {
            self.consume_held();
        }
        self.swing_arm();
        if let Some(hs) = host_spec(&spec.host) {
            self.audio.play(hs.place_sound, Some([f64::from(at[0]) + 0.5, f64::from(at[1]) + 0.5, f64::from(at[2]) + 0.5]), 1.0, 0.8);
        }
        true
    }

    /// Mining a host: every part (hosts that break whole: the game then removes the block as usual,
    /// with tool wear and statistics) or the targeted part (the break ends here).
    pub(in crate::game) fn forja_break_part(&mut self, pos: Pos) -> Option<horizon_client::Result<()>> {
        if !self.forja_active() || self.player.is_none() {
            return None;
        }
        let host = host_at(self, pos)?;
        if !self.may_build() || !self.res.items().can_destroy_block(self.held(), self.creative) {
            return None;
        }
        let hs = host_spec(&host.block);
        let specs = specs();
        let spec_of = spec_fn(&specs);
        if hs.as_ref().is_none_or(|h| h.break_whole) {
            let parts = with_table(self, |t| {
                t.changed = true;
                t.hosts.remove(&pos).map(|mut h| h.take_all()).unwrap_or_default()
            })
            .unwrap_or_default();
            if !self.creative {
                let items = engine::part_drops(&parts, &spec_of);
                self.forja_drop_items(pos, items);
            }
            bump();
            refresh_parts(self, pos);
            return None;
        }
        let slot = match self.forja_target() {
            Some((p, s, _)) if p == pos => s,
            _ => host.parts().first().map(|(s, _)| *s)?,
        };
        let part = host.get(slot)?.clone();
        let particles = self.forja_part_state(&part, slot);
        let removed = take_part(self, pos, slot);
        if let Some(st) = particles {
            self.level_event_at(2001, pos, st as i32);
        }
        if !self.creative {
            let items = engine::part_drops(&removed, &spec_of);
            self.forja_drop_items(pos, items);
        }
        Some(Ok(()))
    }

    /// Every tick: the library follows the world/dimension; hosts whose block is gone (explosion,
    /// piston, command) drop every part; once a second the looks follow the neighbours.
    fn forja_hosts_tick(&mut self) {
        let key = format!("{}|{}", self.world_root.display(), kit_state::dim_key(self));
        let (fresh, tick) = MP.with(|m| {
            let mut m = m.borrow_mut();
            m.tick += 1;
            let fresh = m.key != key;
            if fresh {
                m.key = key.clone();
                m.looks.clear();
            }
            (fresh, m.tick)
        });
        if fresh {
            forja_shapes::clear_hosts();
        }
        // The host blocks' states (again when a mod registers its host later).
        let kinds = parts_registry(|r| r.hosts.len());
        if fresh || MP.with(|m| m.borrow().host_kinds != kinds) {
            MP.with(|m| m.borrow_mut().host_kinds = kinds);
            let len = self.pack.assets.registry.len();
            let mut blocks: Vec<String> = parts_registry(|r| r.hosts.keys().cloned().collect());
            // A host's signal twin draws, collides and is aimed at as the host.
            blocks.extend(twin_names());
            let mut ids = Vec::new();
            for i in 0..len {
                let id = horizon_world::registry::BlockStateId(i as u32);
                if blocks.iter().any(|b| *self.res.facts().get(id).name == **b) {
                    ids.push(i as u32);
                }
            }
            forja_shapes::set_host_states(&ids, len);
        }
        // Gone hosts: every part drops (CableBusBlock.getDrops), like a broken machine.
        let game: &Game<'_> = self;
        let Some(gone) = read_table(game, |t| {
            t.hosts
                .iter()
                .filter(|(p, h)| h.is_empty() || block_name(game, **p).is_some_and(|n| *n != *h.block))
                .map(|(p, h)| (*p, h.block.clone()))
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        if !gone.is_empty() {
            let specs = specs();
            let spec_of = spec_fn(&specs);
            for (p, block) in &gone {
                let parts = with_table(self, |t| {
                    t.changed = true;
                    t.hosts.remove(p).map(|mut h| h.take_all()).unwrap_or_default()
                })
                .unwrap_or_default();
                let items = engine::part_drops(&parts, &spec_of);
                self.forja_drop_items(*p, items);
                if block_name(self, *p).is_some_and(|n| *n == **block) {
                    // An empty record under a host block: the block goes too.
                    self.forja_clear_host(*p);
                }
            }
            bump();
            for (p, _) in &gone {
                refresh_parts(self, *p);
            }
        }
        if !fresh && tick % 20 != 0 {
            return;
        }
        let Some(all) = read_table(self, |t| t.hosts.clone()) else { return };
        let specs = specs();
        let views: Vec<(Pos, Option<(u64, (Vec<u32>, Vec<(u8, [f64; 6])>, Vec<[f64; 6]>))>)> = {
            let game: &Game<'_> = self;
            let lookup = |q: Pos| {
                let name = block_name(game, q)?;
                all.get(&q).filter(|h| *h.block == *name).cloned()
            };
            all.keys()
                .filter(|p| block_name(game, **p).is_some())
                .map(|p| {
                    let v = lookup(*p).map(|h| {
                        let v = host_view(game, *p, &h, &specs, &lookup);
                        (view_hash(&v), v)
                    });
                    (*p, v)
                })
                .collect()
        };
        for (p, v) in views {
            apply_view(self, p, v);
        }
    }

    /// "Forme din model": once per registry and model catalog, the content-mod block states whose
    /// model is not their template's shape get their model's boxes (FUNDATIE-FORJA.md §8).
    fn forja_model_shapes(&mut self) {
        let len = self.pack.assets.registry.len();
        let catalog = &self.pack.render.model_catalog;
        let key = (len, catalog as *const _ as usize);
        if MP.with(|m| m.borrow().models == key) {
            return;
        }
        MP.with(|m| m.borrow_mut().models = key);
        let Ok(library) = self.pack.render.model_library.as_ref() else {
            forja_shapes::set_models(Vec::new(), len);
            return;
        };
        let facts = self.res.facts();
        let mut cache: std::collections::HashMap<String, Option<Vec<[f64; 6]>>> = std::collections::HashMap::new();
        let mut list = Vec::new();
        for i in 0..len {
            let id = horizon_world::registry::BlockStateId(i as u32);
            let f = facts.get(id);
            if f.name.starts_with("minecraft:")
                || f.render_shape != horizon_client::blocks::RenderShape::Model
                || f.random_offset
                || f.outline_offset
            {
                continue;
            }
            let Ok(models) = catalog.collect(id, horizon_core::position::BlockPos::new(0, 0, 0)) else { continue };
            if models.is_empty() {
                continue;
            }
            let mut variants = Vec::new();
            let mut ok = true;
            for (variant, _) in &models {
                let elements = cache.entry(variant.model.clone()).or_insert_with(|| {
                    library.resolve(&variant.model).ok().map(|m| {
                        m.elements()
                            .iter()
                            .flatten()
                            .map(|e| {
                                [
                                    f64::from(e.from[0]),
                                    f64::from(e.from[1]),
                                    f64::from(e.from[2]),
                                    f64::from(e.to[0]),
                                    f64::from(e.to[1]),
                                    f64::from(e.to[2]),
                                ]
                            })
                            .collect()
                    })
                });
                match elements {
                    Some(e) if !e.is_empty() => variants.push((e.clone(), variant.rotations)),
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            let boxes = engine::model_boxes(&variants);
            let collision: Option<Vec<[f64; 6]>> = self.collision.cached_shapes().cached_shape(id).ok().flatten().map(|s| {
                use horizon_core::geometry::Axis;
                s.boxes()
                    .iter()
                    .map(|b| [b.min(Axis::X), b.min(Axis::Y), b.min(Axis::Z), b.max(Axis::X), b.max(Axis::Y), b.max(Axis::Z)])
                    .collect()
            });
            let classes: Vec<&str> = f.classes.iter().map(|c| &**c).collect();
            if let Some(rule) = engine::model_shape_rule(&f.name, &boxes, &f.outline, collision.as_deref(), &classes) {
                list.push((i as u32, rule.outline, rule.collide));
            }
        }
        forja_shapes::set_models(list, len);
    }
}

// =================================================================================================
// ---- mod networking ("Rețea mod"): a typed channel between a mod's server half and its client
// half over Horizon's own connection, and the forja menu built on it ----
//
// Single player is an integrated server: the window joins it as a protocol client (a guest), as LAN
// friends do. A guest only gets vanilla menus over the protocol; a forja menu runs the mod's own host
// menu code on the server *as the guest* (`as_guest`) and mirrors what the screen needs to the
// guest's client, which draws the mod's own screen from the mirror and sends its input back.
//
// The packets: custom payloads on `forja:net` (engine::NET_CHANNEL), split in frames
// (engine::frames). On the server, a HEAD on `Hub::tick_level` (library module `forja_net`) takes
// them out of the guests' inboxes before the hub hands the inbox to the game (whose packet function
// has macros and cannot be hooked); a HEAD on `lan_living_flags` (every tick_lan, with the hub in
// hand) runs them and syncs the open menus. On the client, a RETURN on `Session::poll` takes them
// out of the received events, a HEAD on `tick_remote` applies them.

use super::super::lan::LanHost;
use horizon_protocol::packets::configuration::CustomPayload as NetPayload;
use horizon_protocol::packets::play::{Clientbound as NetC, Serverbound as NetS};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

#[allow(unused_imports)]
pub(in crate::game) use engine::{
    Channel, MenuInput, MenuToClient, MenuToServer, Reader, Redstone, Signal, Wire, Writer, from_bytes, to_bytes,
};

/// A mod's server half: a message from guest `conn` (the hub in hand, outside any guest's turn).
pub(in crate::game) type ServerChannelFn = for<'a, 'g, 'h> fn(&'a mut Game<'g>, &'h mut LanHost, u32, &[u8]);
/// A mod's client half: a message from the server.
pub(in crate::game) type ClientChannelFn = for<'a, 'g> fn(&'a mut Game<'g>, &[u8]);

#[derive(Default)]
struct NetRegistry {
    server: BTreeMap<String, ServerChannelFn>,
    client: BTreeMap<String, ClientChannelFn>,
}
static NET_REGISTRY: Mutex<Option<NetRegistry>> = Mutex::new(None);
fn net_registry<R>(f: impl FnOnce(&mut NetRegistry) -> R) -> R {
    let mut g = NET_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(NetRegistry::default))
}
/// The messages of client channels nobody registered a handler for (a mod polls them).
static CLIENT_INBOX: Mutex<BTreeMap<String, Vec<Vec<u8>>>> = Mutex::new(BTreeMap::new());
static NEXT_MESSAGE: AtomicU32 = AtomicU32::new(1);

/// Registers the server half of a channel (idempotent; call it from your world-load hook).
#[allow(dead_code)]
pub(in crate::game) fn register_server_channel(id: &str, f: ServerChannelFn) {
    net_registry(|r| {
        r.server.insert(id.to_owned(), f);
    });
}
/// Registers the client half of a channel. A pure LAN guest never loads a world, so call it from a
/// hook the client runs too (or poll with `client_take` instead).
#[allow(dead_code)]
pub(in crate::game) fn register_client_channel(id: &str, f: ClientChannelFn) {
    net_registry(|r| {
        r.client.insert(id.to_owned(), f);
    });
}
/// The messages that arrived on a client channel without a handler, oldest first.
#[allow(dead_code)]
pub(in crate::game) fn client_take(channel: &str) -> Vec<Vec<u8>> {
    CLIENT_INBOX.lock().unwrap_or_else(|e| e.into_inner()).remove(channel).unwrap_or_default()
}
fn payloads(channel: &str, body: &[u8]) -> Vec<NetPayload> {
    let id = NEXT_MESSAGE.fetch_add(1, Ordering::Relaxed);
    engine::frames(id, channel, body)
        .into_iter()
        .map(|data| NetPayload { channel: engine::NET_CHANNEL.to_owned(), data })
        .collect()
}
/// Server -> one guest (outside a guest's turn: the hub in hand).
pub(in crate::game) fn send_to_guest(host: &LanHost, conn: u32, channel: &str, body: &[u8]) {
    for p in payloads(channel, body) {
        host.hub.send_to(conn, &NetC::CustomPayload(p));
    }
}
/// Server -> the guest whose turn it is (inside `as_guest`: the hub is not in `game.lan`).
#[allow(dead_code)]
pub(in crate::game) fn send_to_acting(game: &mut Game<'_>, channel: &str, body: &[u8]) -> bool {
    if game.lan_acting.is_none() {
        return false;
    }
    for p in payloads(channel, body) {
        game.guest_packets.push(NetC::CustomPayload(p));
    }
    true
}
/// Client -> its server. False when this window is not a server's client.
pub(in crate::game) fn send_to_server(game: &Game<'_>, channel: &str, body: &[u8]) -> bool {
    if game.remote.is_none() {
        return false;
    }
    for p in payloads(channel, body) {
        game.send(NetS::CustomPayload(p));
    }
    true
}
/// The guest's client answered forja's hello (it shows forja menus and hears mod channels).
pub(in crate::game) fn guest_has_forja(conn: u32) -> bool {
    server_net(|s| s.capable.contains(&conn))
}
/// This client's server greeted it (a Horizon server with forja).
#[allow(dead_code)]
pub(in crate::game) fn server_has_forja() -> bool {
    client_net(|c| c.server_forja)
}

// ---- forja menus: the kinds mods register ----

/// A position's guest of `conn` with the game; `Pos` the menu's block.
pub(in crate::game) type MenuFn = for<'a, 'g> fn(&'a mut Game<'g>, u32, Pos);
pub(in crate::game) type MenuClaimFn = for<'a, 'g> fn(&'a mut Game<'g>, u32, Pos) -> bool;
pub(in crate::game) type MenuSectionsFn = for<'a, 'g> fn(&'a mut Game<'g>, u32, Pos) -> Vec<Vec<u8>>;
pub(in crate::game) type MenuInputFn = for<'a, 'g, 'i> fn(&'a mut Game<'g>, u32, Pos, &'i MenuInput) -> bool;
pub(in crate::game) type MenuValidFn = for<'a, 'g> fn(&'a Game<'g>, u32, Pos) -> bool;

/// A kind of forja menu. On the server the mod's own host code runs it as the guest: forja sets
/// `container = Some(pos)` and `screen = Inventory` for the guest's turn, runs `enter`, the input
/// (`Game::menu_ops` for slot clicks, `input` for the rest), `leave`. The client mirrors the
/// container's block-entity slots (`slots`), the forja machine at `pos` (`machine`) and the mod's
/// `sections`, and shows the same screen the host sees.
#[derive(Clone, Copy)]
pub(in crate::game) struct MenuKind {
    pub id: &'static str,
    /// Mirror the block entity's slots at `pos` (game slots 300..).
    pub slots: bool,
    /// Mirror the forja machine at `pos` (energy, progress, tanks, data) for its bars.
    pub machine: bool,
    /// A lid with openers (chests): closing counts one opener less, with the close sound.
    pub lid: bool,
    /// `lan_open_container` after a guest's turn: this kind opens for the block at `pos` (may
    /// prepare the block entity).
    pub claims: Option<MenuClaimFn>,
    /// Around every turn and snapshot of the guest's menu (put the guest's own window state in
    /// place, then back).
    pub enter: Option<MenuFn>,
    pub leave: Option<MenuFn>,
    /// Once a tick, inside enter/leave (refreshes).
    pub tick: Option<MenuFn>,
    /// The mod's own state of the menu, as sections (each mirrored when it changes).
    pub sections: Option<MenuSectionsFn>,
    /// Mouse, wheel, keys, text, buttons (the cursor, window size and Shift are set as the guest's).
    pub input: Option<MenuInputFn>,
    /// The menu closed (by the guest, by the server, or the guest left).
    pub closed: Option<MenuFn>,
    /// Still open? (the block is there; the reach is checked by forja).
    pub valid: Option<MenuValidFn>,
}
impl MenuKind {
    pub const fn new(id: &'static str) -> Self {
        MenuKind {
            id,
            slots: true,
            machine: false,
            lid: false,
            claims: None,
            enter: None,
            leave: None,
            tick: None,
            sections: None,
            input: None,
            closed: None,
            valid: None,
        }
    }
}
static MENU_KINDS: Mutex<Vec<MenuKind>> = Mutex::new(Vec::new());
/// Registers (or replaces) a forja menu kind. Idempotent.
#[allow(dead_code)]
pub(in crate::game) fn register_menu(kind: MenuKind) {
    let mut k = MENU_KINDS.lock().unwrap_or_else(|e| e.into_inner());
    k.retain(|x| x.id != kind.id);
    k.push(kind);
}
fn menu_kind(id: &str) -> Option<MenuKind> {
    MENU_KINDS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|k| k.id == id).copied()
}
fn menu_kinds() -> Vec<MenuKind> {
    MENU_KINDS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
/// The generic machine menu: a registered forja machine with a screen (`register_screen`) over a
/// vanilla container template; mods with widgets register their own kind first.
fn machine_menu_claims(game: &mut Game<'_>, _conn: u32, p: Pos) -> bool {
    block_name(game, p).is_some_and(|n| screen_of(&n).is_some())
}
const MACHINE_MENU: MenuKind = MenuKind {
    machine: true,
    claims: Some(machine_menu_claims),
    ..MenuKind::new("forja:machine")
};

// ---- server state ----

struct ServerMenu {
    kind: &'static str,
    window: i32,
    pos: Pos,
    dimension: String,
    gate: engine::InputGate,
    slots: engine::Mirror<Vec<u8>>,
    machine: Option<String>,
    sections: engine::Mirror<Vec<u8>>,
    carried: Option<Vec<u8>>,
}
#[derive(Default)]
struct ServerNet {
    frames: HashMap<u32, engine::Reassembler>,
    greeted: HashSet<u32>,
    capable: HashSet<u32>,
    menus: HashMap<u32, ServerMenu>,
    windows: HashMap<u32, i32>,
    /// Each guest's screen size in GUI pixels (`MenuToServer::Size`).
    sizes: HashMap<u32, [u32; 2]>,
}
static SERVER_NET: Mutex<Option<ServerNet>> = Mutex::new(None);
fn server_net<R>(f: impl FnOnce(&mut ServerNet) -> R) -> R {
    let mut g = SERVER_NET.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(ServerNet::default))
}

fn item_bytes(s: &ItemStack) -> Vec<u8> {
    use horizon_world::nbt::{JavaString, Limits, NamedTag, write_named};
    let root = NamedTag { name: JavaString::from(""), tag: horizon_client::save::item_to_nbt(s) };
    write_named(&root, Limits::default()).unwrap_or_default()
}
fn bytes_item(b: &[u8]) -> Option<ItemStack> {
    use horizon_world::nbt::{Limits, read_named};
    let (named, _) = read_named(b, Limits::default()).ok()?;
    horizon_client::save::item_from_nbt(&named.tag)
}
fn input_code(i: horizon_client::menu::Input) -> u8 {
    use horizon_client::menu::Input;
    match i {
        Input::Pickup => 0,
        Input::QuickMove => 1,
        Input::Swap => 2,
        Input::Clone => 3,
        Input::Throw => 4,
        Input::QuickCraft => 5,
        Input::PickupAll => 6,
    }
}
fn input_of(c: u8) -> Option<horizon_client::menu::Input> {
    use horizon_client::menu::Input;
    Some(match c {
        0 => Input::Pickup,
        1 => Input::QuickMove,
        2 => Input::Swap,
        3 => Input::Clone,
        4 => Input::Throw,
        5 => Input::QuickCraft,
        6 => Input::PickupAll,
        _ => return None,
    })
}
/// The keys a forja window takes from the keyboard, by code (`MenuInput::Key`).
const KEYS: [winit::keyboard::KeyCode; 14] = {
    use winit::keyboard::KeyCode as K;
    [
        K::Enter,
        K::NumpadEnter,
        K::Escape,
        K::Backspace,
        K::Delete,
        K::Tab,
        K::ArrowLeft,
        K::ArrowRight,
        K::ArrowUp,
        K::ArrowDown,
        K::Home,
        K::End,
        K::PageUp,
        K::PageDown,
    ]
};
/// A key's code for `MenuInput::Key` (None: not one a window takes).
#[allow(dead_code)]
pub(in crate::game) fn key_code(k: winit::keyboard::KeyCode) -> Option<u16> {
    KEYS.iter().position(|x| *x == k).map(|i| i as u16)
}
/// The key of a `MenuInput::Key` code.
#[allow(dead_code)]
pub(in crate::game) fn key_of(code: u16) -> Option<winit::keyboard::KeyCode> {
    KEYS.get(usize::from(code)).copied()
}
/// A mouse button's code (`MenuInput::Mouse`).
pub(in crate::game) fn button_code(b: winit::event::MouseButton) -> u8 {
    use winit::event::MouseButton;
    match b {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(_) => 5,
    }
}
#[allow(dead_code)]
pub(in crate::game) fn button_of(c: u8) -> winit::event::MouseButton {
    use winit::event::MouseButton;
    match c {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        _ => MouseButton::Other(0),
    }
}

thread_local! {
    /// The guest's window size while the server replays their mouse or wheel input.
    static REPLAY_SIZE: std::cell::Cell<Option<[u32; 2]>> = const { std::cell::Cell::new(None) };
}
/// The size of the screen in GUI pixels (`HudBatch` width and height) the window is laid out in:
/// this window's, the guest's while the server replays the guest's input, or the guest's whose
/// turn it is (the size their client last sent). Mods measure their widgets with it (with
/// `hud_batch`, never `self.renderer`, which a server does not have).
pub(in crate::game) fn hud_size(game: &Game<'_>) -> Option<[u32; 2]> {
    if let Some(s) = REPLAY_SIZE.with(|r| r.get()) {
        return Some(s);
    }
    if let Some(conn) = game.lan_acting
        && let Some(s) = server_net(|n| n.sizes.get(&conn).copied())
    {
        return Some(s);
    }
    game.renderer.as_ref().map(|r| {
        let b = HudBatch::new([r.config.width, r.config.height]);
        [b.width as u32, b.height as u32]
    })
}
/// A `HudBatch` of that size (to compute the screen's origin as the drawing does): this window's
/// own batch, or one of the guest's GUI size.
#[allow(dead_code)]
pub(in crate::game) fn hud_batch(game: &Game<'_>) -> Option<HudBatch> {
    let replayed = REPLAY_SIZE.with(|r| r.get()).is_some() || game.lan_acting.is_some_and(|c| server_net(|n| n.sizes.contains_key(&c)));
    if !replayed && let Some(r) = game.renderer.as_ref() {
        return Some(HudBatch::new([r.config.width, r.config.height]));
    }
    let [w, h] = hud_size(game)?;
    let mut b = HudBatch::new([w.max(1), h.max(1)]);
    b.scale = 1.0;
    b.width = w as f32;
    b.height = h as f32;
    Some(b)
}

fn dim_code(game: &Game<'_>) -> u8 {
    match game.dim {
        super::super::Dim::Overworld => 0,
        super::super::Dim::Nether => 1,
        super::super::Dim::End => 2,
    }
}

impl Game<'_> {
    /// HEAD `lan_living_flags` (every tick_lan pass, the hub in hand): forja's hello to new guests,
    /// the frames the guests of this level sent, then the open forja menus of this level.
    pub(in crate::game) fn forja_net_server(&mut self, host: &mut LanHost) {
        if !self.forja_active() {
            return;
        }
        // Who is here; greet the newcomers, forget who left.
        let here: Vec<u32> = host.hub.guests.iter().filter(|g| host.hub.here(g)).map(|g| g.conn).collect();
        let all: HashSet<u32> = host.hub.guests.iter().map(|g| g.conn).collect();
        let (new, gone) = server_net(|s| {
            let gone: Vec<u32> = s.menus.keys().copied().filter(|c| !all.contains(c)).collect();
            s.greeted.retain(|c| all.contains(c));
            s.capable.retain(|c| all.contains(c));
            s.frames.retain(|c, _| all.contains(c));
            s.windows.retain(|c, _| all.contains(c));
            s.sizes.retain(|c, _| all.contains(c));
            let new: Vec<u32> = host
                .hub
                .guests
                .iter()
                .filter(|g| g.placed && !s.greeted.contains(&g.conn))
                .map(|g| g.conn)
                .collect();
            s.greeted.extend(new.iter().copied());
            (new, gone)
        });
        for conn in gone {
            // A guest who left: their menu closes on the server only.
            if let Some(m) = server_net(|s| s.menus.remove(&conn))
                && let Some(kind) = menu_kind(m.kind)
            {
                if let Some(f) = kind.closed {
                    f(self, conn, m.pos);
                }
                if kind.lid && self.container_openers_changed((m.pos[0], m.pos[1], m.pos[2]), false) {
                    self.container_edge_sound((m.pos[0], m.pos[1], m.pos[2]), false);
                }
            }
        }
        let hello = to_bytes(&MenuToClient::Hello { version: engine::MENU_VERSION });
        for conn in new {
            send_to_guest(host, conn, engine::MENU_CHANNEL, &hello);
        }
        // The frames.
        let frames = horizon_client::forja_net::take_server();
        for (conn, frame) in frames {
            let done = server_net(|s| s.frames.entry(conn).or_default().push(&frame));
            match done {
                Ok(Some((channel, body))) => self.forja_net_dispatch(host, conn, &channel, &body),
                Ok(None) => {}
                Err(_) => {} // a broken message is dropped whole
            }
        }
        // The open menus of this level.
        let open: Vec<u32> = server_net(|s| s.menus.keys().copied().filter(|c| here.contains(c)).collect());
        for conn in open {
            self.forja_menu_tick(host, conn);
        }
    }
    fn forja_net_dispatch(&mut self, host: &mut LanHost, conn: u32, channel: &str, body: &[u8]) {
        if channel == engine::MENU_CHANNEL {
            match from_bytes::<MenuToServer>(body) {
                Some(MenuToServer::Hello { version }) if version == engine::MENU_VERSION => {
                    server_net(|s| s.capable.insert(conn));
                }
                Some(MenuToServer::Input { window, seq, input }) => self.forja_menu_input(host, conn, window, seq, input),
                Some(MenuToServer::Close { window }) => {
                    let open = server_net(|s| s.menus.get(&conn).is_some_and(|m| m.window == window));
                    if open {
                        self.forja_menu_close(host, conn, false);
                    }
                }
                Some(MenuToServer::Size { w, h }) => {
                    server_net(|s| s.sizes.insert(conn, [w.clamp(1, 1 << 15), h.clamp(1, 1 << 15)]));
                }
                _ => {}
            }
            return;
        }
        if let Some(f) = net_registry(|r| r.server.get(channel).copied()) {
            f(self, host, conn, body);
        }
    }

    // ---- server: forja menus ----

    /// Opens a forja menu of `kind` on the block at `pos` for guest `conn` (whose client speaks
    /// forja: `guest_has_forja`). The guest's vanilla menu closes. False when nothing opened.
    pub(in crate::game) fn forja_menu_open(&mut self, host: &mut LanHost, conn: u32, kind: &str, pos: Pos) -> bool {
        let Some(kind) = menu_kind(kind).or_else(|| (kind == MACHINE_MENU.id).then_some(MACHINE_MENU)) else {
            return false;
        };
        if !guest_has_forja(conn) {
            return false;
        }
        let Some(g) = host.hub.guest(conn) else { return false };
        let dimension = g.dimension.clone();
        let vanilla = g.menu.as_ref().map(|m| m.window);
        // ServerPlayer.openMenu closes what was open.
        if server_net(|s| s.menus.contains_key(&conn)) {
            self.forja_menu_close(host, conn, true);
        }
        self.guest_container_closed(host, conn);
        self.lan_station_closed(host, conn, None);
        host.hub.close_container(conn);
        let key = (pos[0], pos[1], pos[2]);
        let size = if kind.slots {
            self.world.block_entities.get(&key).map_or(0, |e| e.slots().len()).min(engine::MAX_MENU_SLOTS)
        } else {
            0
        };
        let title = self.world.block_names.get(&key).cloned().unwrap_or_default();
        let window = server_net(|s| {
            let w = engine::next_window(s.windows.get(&conn).copied().unwrap_or(0), vanilla);
            s.windows.insert(conn, w);
            s.menus.insert(
                conn,
                ServerMenu {
                    kind: kind.id,
                    window: w,
                    pos,
                    dimension,
                    gate: engine::InputGate::new(w),
                    slots: engine::Mirror::new(size),
                    machine: None,
                    sections: engine::Mirror::new(0),
                    carried: None,
                },
            );
            w
        });
        let open = MenuToClient::Open { window, kind: kind.id.to_owned(), pos, size: size as u32, title };
        send_to_guest(host, conn, engine::MENU_CHANNEL, &to_bytes(&open));
        self.forja_menu_sync(host, conn);
        true
    }
    /// Closes guest `conn`'s forja menu: `replaced` = another menu opens (no close packet needed:
    /// the open replaces it on the client).
    pub(in crate::game) fn forja_menu_close(&mut self, host: &mut LanHost, conn: u32, replaced: bool) {
        let Some(m) = server_net(|s| s.menus.remove(&conn)) else { return };
        if !replaced {
            send_to_guest(host, conn, engine::MENU_CHANNEL, &to_bytes(&MenuToClient::Close { window: m.window }));
        }
        let key = (m.pos[0], m.pos[1], m.pos[2]);
        if let Some(kind) = menu_kind(m.kind).or_else(|| (m.kind == MACHINE_MENU.id).then_some(MACHINE_MENU)) {
            if let Some(f) = kind.closed {
                f(self, conn, m.pos);
            }
            if kind.lid && self.container_openers_changed(key, false) {
                self.container_edge_sound(key, false);
            }
        }
        // AbstractContainerMenu.removed: the carried stack goes back to the inventory.
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.guest_carried_back(host, gi);
        }
    }
    fn forja_menu_input(&mut self, host: &mut LanHost, conn: u32, window: i32, seq: u32, input: MenuInput) {
        let Some((kind, pos)) = server_net(|s| {
            let m = s.menus.get_mut(&conn)?;
            m.gate.accept(window, seq).then_some((m.kind, m.pos))
        }) else {
            return;
        };
        let Some(kind) = menu_kind(kind).or_else(|| (kind == MACHINE_MENU.id).then_some(MACHINE_MENU)) else { return };
        if !self.forja_menu_valid(host, conn, &kind, pos) {
            self.forja_menu_close(host, conn, false);
            return;
        }
        let key = (pos[0], pos[1], pos[2]);
        self.as_guest(host, conn, |g| {
            g.container = Some(key);
            g.screen = super::super::Screen::Inventory;
            if let Some(f) = kind.enter {
                f(g, conn, pos);
            }
            match &input {
                MenuInput::Ops(ops) => {
                    let ops: Vec<(Option<usize>, i32, horizon_client::menu::Input)> = ops
                        .iter()
                        .filter_map(|&(slot, button, code)| {
                            let slot = if slot == engine::SLOT_OUTSIDE { None } else { Some(usize::try_from(slot).ok()?) };
                            Some((slot, button, input_of(code)?))
                        })
                        .collect();
                    if !ops.is_empty() {
                        g.menu_ops(&ops);
                    }
                }
                other => {
                    let cursor = g.cursor;
                    let keys = g.physical_keys.clone();
                    let (at, size, shift) = match *other {
                        MenuInput::Mouse { x, y, w, h, shift, .. } | MenuInput::Scroll { x, y, w, h, shift, .. } => {
                            (Some((x, y)), Some([w.max(1), h.max(1)]), shift)
                        }
                        MenuInput::Key { shift, .. } => (None, None, shift),
                        _ => (None, None, false),
                    };
                    if let Some(at) = at {
                        g.cursor = at;
                    }
                    if shift {
                        g.physical_keys.insert(winit::keyboard::KeyCode::ShiftLeft);
                    }
                    REPLAY_SIZE.with(|r| r.set(size));
                    if let Some(f) = kind.input {
                        f(g, conn, pos, other);
                    }
                    REPLAY_SIZE.with(|r| r.set(None));
                    g.cursor = cursor;
                    g.physical_keys = keys;
                }
            }
            if let Some(f) = kind.leave {
                f(g, conn, pos);
            }
            // What the turn opened is ours, not a vanilla menu for leave_guest to open.
            g.container = None;
            g.screen = super::super::Screen::Playing;
        });
        self.forja_menu_sync(host, conn);
    }
    fn forja_menu_valid(&mut self, host: &LanHost, conn: u32, kind: &MenuKind, pos: Pos) -> bool {
        let Some(g) = host.hub.guest(conn) else { return false };
        let Some(dimension) = server_net(|s| s.menus.get(&conn).map(|m| m.dimension.clone())) else { return false };
        if g.dimension != dimension || g.dead || !g.within_block_range(pos, 4.0) {
            return false;
        }
        if kind.slots && !self.world.block_entities.contains_key(&(pos[0], pos[1], pos[2])) {
            return false;
        }
        kind.valid.is_none_or(|f| f(self, conn, pos))
    }
    fn forja_menu_tick(&mut self, host: &mut LanHost, conn: u32) {
        let Some((kind, pos)) = server_net(|s| s.menus.get(&conn).map(|m| (m.kind, m.pos))) else { return };
        let Some(kind) = menu_kind(kind).or_else(|| (kind == MACHINE_MENU.id).then_some(MACHINE_MENU)) else {
            self.forja_menu_close(host, conn, false);
            return;
        };
        if !self.forja_menu_valid(host, conn, &kind, pos) {
            self.forja_menu_close(host, conn, false);
            return;
        }
        if let Some(f) = kind.tick {
            if let Some(e) = kind.enter {
                e(self, conn, pos);
            }
            f(self, conn, pos);
            if let Some(l) = kind.leave {
                l(self, conn, pos);
            }
        }
        self.forja_menu_sync(host, conn);
    }
    /// `broadcastChanges` for a guest's forja menu: what changed of the container slots, the
    /// machine, the mod's sections and the carried stack.
    fn forja_menu_sync(&mut self, host: &mut LanHost, conn: u32) {
        let Some((kind, pos, window)) = server_net(|s| s.menus.get(&conn).map(|m| (m.kind, m.pos, m.window))) else { return };
        let Some(kind) = menu_kind(kind).or_else(|| (kind == MACHINE_MENU.id).then_some(MACHINE_MENU)) else { return };
        let key = (pos[0], pos[1], pos[2]);
        if let Some(f) = kind.enter {
            f(self, conn, pos);
        }
        let slots: Option<Vec<Option<Vec<u8>>>> = kind.slots.then(|| {
            self.world
                .block_entities
                .get(&key)
                .map(|e| e.slots().iter().take(engine::MAX_MENU_SLOTS).map(|s| s.as_ref().map(item_bytes)).collect())
                .unwrap_or_default()
        });
        let machine = if kind.machine {
            read_table(self, |t| t.map.get(&pos).cloned()).flatten().map(|m| {
                let mut one = Table::default();
                one.map.insert(pos, m);
                engine::encode(&one, &NbtCodec)
            })
        } else {
            None
        };
        let sections: Vec<Option<Vec<u8>>> = kind.sections.map(|f| f(self, conn, pos)).unwrap_or_default().into_iter().map(Some).collect();
        if let Some(f) = kind.leave {
            f(self, conn, pos);
        }
        let carried = host.hub.guest(conn).and_then(|g| g.carried.as_ref().map(item_bytes));
        let size = slots.as_ref().map_or(0, Vec::len) as u32;
        let (slot_changes, machine_changed, section_changes, carried_changed) = server_net(|s| {
            let Some(m) = s.menus.get_mut(&conn) else { return (Vec::new(), None, Vec::new(), false) };
            let sc = slots.map(|v| m.slots.diff(&v)).unwrap_or_default();
            let mc = (m.machine != machine).then(|| {
                m.machine = machine.clone();
                machine.clone()
            });
            let xc = m.sections.diff(&sections);
            let cc = m.carried != carried;
            m.carried = carried.clone();
            (sc, mc, xc, cc)
        });
        if !slot_changes.is_empty() {
            let msg = MenuToClient::Slots { window, size, changes: slot_changes };
            send_to_guest(host, conn, engine::MENU_CHANNEL, &to_bytes(&msg));
        }
        if let Some(text) = machine_changed {
            send_to_guest(host, conn, engine::MENU_CHANNEL, &to_bytes(&MenuToClient::Machine { window, text }));
        }
        for (index, data) in section_changes {
            let msg = MenuToClient::Section { window, index, data: data.unwrap_or_default() };
            send_to_guest(host, conn, engine::MENU_CHANNEL, &to_bytes(&msg));
        }
        if carried_changed {
            let net = {
                let c = host.hub.guest(conn).and_then(|g| g.carried.clone());
                host.hub.stack(&c)
            };
            host.hub.send_to(conn, &NetC::SetCursorItem(horizon_protocol::packets::play::SetCursorItem { item: net }));
        }
    }
    /// HEAD cancellable `lan_open_container` (after a guest's turn opened a container): a forja
    /// menu kind that claims the block opens instead of a vanilla menu, for a guest whose client
    /// speaks forja.
    pub(in crate::game) fn forja_lan_open(&mut self, host: &mut LanHost, conn: u32, pos: (i32, i32, i32)) -> Option<bool> {
        if !self.forja_active() || !guest_has_forja(conn) {
            return None;
        }
        let p = [pos.0, pos.1, pos.2];
        let mut kinds = menu_kinds();
        kinds.push(MACHINE_MENU);
        for kind in kinds {
            if kind.claims.is_some_and(|f| f(self, conn, p)) {
                return Some(self.forja_menu_open(host, conn, kind.id, p));
            }
        }
        None
    }
}

/// The forja menu guest `conn` has open on the server: `(kind, pos)`.
#[allow(dead_code)]
pub(in crate::game) fn server_menu_of(conn: u32) -> Option<(&'static str, Pos)> {
    server_net(|s| s.menus.get(&conn).map(|m| (m.kind, m.pos)))
}

// ---- client state ----

struct GuestMenu {
    window: i32,
    kind: String,
    pos: Pos,
    /// The mirrored slots now (the Chest put at `pos` has as many).
    size: usize,
    /// What stood in the client's world at `pos` before the menu's slots were put there.
    before: Option<horizon_client::containers::BlockEntity>,
    name_before: Option<String>,
    machine: Option<engine::Machine<ItemStack>>,
    sections: Vec<Vec<u8>>,
    generation: u64,
}
#[derive(Default)]
struct ClientNet {
    frames: engine::Reassembler,
    server_forja: bool,
    menu: Option<GuestMenu>,
    seq: u32,
}
static CLIENT_NET: Mutex<Option<ClientNet>> = Mutex::new(None);
fn client_net<R>(f: impl FnOnce(&mut ClientNet) -> R) -> R {
    let mut g = CLIENT_NET.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(ClientNet::default))
}
static SECTIONS_GEN: AtomicU64 = AtomicU64::new(1);
/// The screen size last sent to the server (w << 32 | h).
static SENT_SIZE: AtomicU64 = AtomicU64::new(0);
/// A guest menu is shown: a cheap check for the hooks.
static GUEST_OPEN: AtomicBool = AtomicBool::new(false);

/// The forja menu this window shows as a guest: `(kind, pos)`.
#[allow(dead_code)]
pub(in crate::game) fn guest_menu(game: &Game<'_>) -> Option<(String, Pos)> {
    if game.remote.is_none() || !GUEST_OPEN.load(Ordering::Relaxed) {
        return None;
    }
    let (kind, pos) = client_net(|c| c.menu.as_ref().map(|m| (m.kind.clone(), m.pos)))?;
    (game.screen == super::super::Screen::Inventory && game.container == Some((pos[0], pos[1], pos[2]))).then_some((kind, pos))
}
/// The guest menu's sections as the server last sent them, and their generation (it changes
/// whenever one of them does).
#[allow(dead_code)]
pub(in crate::game) fn guest_sections(game: &Game<'_>) -> Option<(u64, Vec<Vec<u8>>)> {
    guest_menu(game)?;
    client_net(|c| c.menu.as_ref().map(|m| (m.generation, m.sections.clone())))
}
/// Sends an input of the guest menu to the server (no local effect).
pub(in crate::game) fn guest_send(game: &Game<'_>, input: MenuInput) -> bool {
    if guest_menu(game).is_none() {
        return false;
    }
    let Some((window, seq)) = client_net(|c| {
        let m = c.menu.as_ref()?;
        c.seq = c.seq.wrapping_add(1);
        Some((m.window, c.seq))
    }) else {
        return false;
    };
    send_to_server(game, engine::MENU_CHANNEL, &to_bytes(&MenuToServer::Input { window, seq, input }))
}
/// A widget input on the guest menu: `local` runs the mod's own handler on this client's mirror
/// (its UI state follows at once: focus, scroll, tabs), but every item stack and the machine
/// mirror stay as they were (the server is the authority); when `local` says it took the input,
/// the input goes to the server, which runs the same handler as the guest.
#[allow(dead_code)]
pub(in crate::game) fn guest_widget(game: &mut Game<'_>, input: MenuInput, local: impl FnOnce(&mut Game<'_>) -> bool) -> bool {
    let carried = game.carried.clone();
    let inventory = game.inventory.clone();
    let armor = game.armor.clone();
    let offhand = game.offhand.clone();
    let machine = client_net(|c| c.menu.as_ref().and_then(|m| m.machine.clone()));
    let took = local(game);
    game.carried = carried;
    game.inventory = inventory;
    game.armor = armor;
    game.offhand = offhand;
    client_net(|c| {
        if let Some(m) = c.menu.as_mut() {
            m.machine = machine;
        }
    });
    if took {
        guest_send(game, input);
    }
    took
}
/// The mouse press as a `MenuInput::Mouse` of this window (cursor, size, Shift), with the mod's
/// own extra bytes (its client-only UI state the server needs to find the same widget).
#[allow(dead_code)]
pub(in crate::game) fn mouse_input(game: &Game<'_>, button: winit::event::MouseButton, extra: Vec<u8>) -> MenuInput {
    let [w, h] = hud_size(game).unwrap_or([1, 1]);
    MenuInput::Mouse { button: button_code(button), x: game.cursor.0, y: game.cursor.1, w, h, shift: game.has_shift_down(), extra }
}
/// The wheel as a `MenuInput::Scroll`.
#[allow(dead_code)]
pub(in crate::game) fn scroll_input(game: &Game<'_>, delta: f32, slot: Option<usize>) -> MenuInput {
    let [w, h] = hud_size(game).unwrap_or([1, 1]);
    MenuInput::Scroll {
        delta,
        slot: slot.and_then(|s| i32::try_from(s).ok()).unwrap_or(-1),
        x: game.cursor.0,
        y: game.cursor.1,
        w,
        h,
        shift: game.has_shift_down(),
    }
}

impl Game<'_> {
    /// HEAD `tick_remote` (a server's client, every tick): the frames the server sent.
    pub(in crate::game) fn forja_net_client(&mut self) {
        if self.remote.is_none() {
            return;
        }
        let frames = horizon_client::forja_net::take_client();
        for frame in frames {
            let done = client_net(|c| c.frames.push(&frame));
            if let Ok(Some((channel, body))) = done {
                self.forja_client_dispatch(&channel, &body);
            }
        }
        // The window closed some other way (a vanilla menu replaced it, the screen changed).
        if GUEST_OPEN.load(Ordering::Relaxed) && guest_menu(self).is_none() {
            self.forja_guest_closed(true);
        }
        // The server lays out the windows it runs for this player in this screen's size.
        if client_net(|c| c.server_forja)
            && let Some([w, h]) = hud_size(self)
        {
            let packed = (u64::from(w) << 32) | u64::from(h);
            if SENT_SIZE.swap(packed, Ordering::Relaxed) != packed {
                send_to_server(self, engine::MENU_CHANNEL, &to_bytes(&MenuToServer::Size { w, h }));
            }
        }
    }
    fn forja_client_dispatch(&mut self, channel: &str, body: &[u8]) {
        if channel != engine::MENU_CHANNEL {
            match net_registry(|r| r.client.get(channel).copied()) {
                Some(f) => f(self, body),
                None => {
                    let mut inbox = CLIENT_INBOX.lock().unwrap_or_else(|e| e.into_inner());
                    let q = inbox.entry(channel.to_owned()).or_default();
                    // A mod that never polls does not fill the memory.
                    if q.len() < 256 {
                        q.push(body.to_vec());
                    }
                }
            }
            return;
        }
        let Some(msg) = from_bytes::<MenuToClient>(body) else { return };
        match msg {
            MenuToClient::Hello { version } => {
                // A (new) forja server: what this client knew of another one goes (that world is
                // gone, nothing is put back in this one).
                client_net(|c| c.menu = None);
                GUEST_OPEN.store(false, Ordering::Relaxed);
                client_net(|c| {
                    c.server_forja = version == engine::MENU_VERSION;
                    c.seq = 0;
                });
                if version == engine::MENU_VERSION {
                    let hello = to_bytes(&MenuToServer::Hello { version: engine::MENU_VERSION });
                    send_to_server(self, engine::MENU_CHANNEL, &hello);
                    SENT_SIZE.store(0, Ordering::Relaxed);
                }
            }
            MenuToClient::Open { window, kind, pos, size, title } => {
                self.forja_guest_closed(false);
                let key = (pos[0], pos[1], pos[2]);
                let before = self.world.block_entities.remove(&key);
                let name_before = self.world.block_names.remove(&key);
                if !title.is_empty() {
                    self.world.block_names.insert(key, title);
                }
                self.world
                    .block_entities
                    .insert(key, horizon_client::containers::BlockEntity::Chest(vec![None; size as usize]));
                client_net(|c| {
                    c.menu = Some(GuestMenu {
                        window,
                        kind,
                        pos,
                        size: size as usize,
                        before,
                        name_before,
                        machine: None,
                        sections: Vec::new(),
                        generation: SECTIONS_GEN.fetch_add(1, Ordering::Relaxed),
                    });
                });
                GUEST_OPEN.store(true, Ordering::Relaxed);
                if let Some(r) = self.remote.as_mut() {
                    r.menu = None;
                    r.menu_title = None;
                }
                self.recipe_ui.reset_screen();
                self.station = None;
                self.craft_size = 2;
                self.container = Some(key);
                self.screen = super::super::Screen::Inventory;
                self.capture(false);
            }
            MenuToClient::Slots { window, size, changes } => {
                let Some(pos) = client_net(|c| c.menu.as_ref().filter(|m| m.window == window).map(|m| m.pos)) else { return };
                let key = (pos[0], pos[1], pos[2]);
                client_net(|c| {
                    if let Some(m) = c.menu.as_mut() {
                        m.size = size as usize;
                    }
                });
                if let Some(horizon_client::containers::BlockEntity::Chest(items)) = self.world.block_entities.get_mut(&key) {
                    items.resize(size as usize, None);
                    for (i, item) in changes {
                        if let Some(s) = items.get_mut(i as usize) {
                            *s = item.as_deref().and_then(bytes_item);
                        }
                    }
                }
            }
            MenuToClient::Machine { window, text } => {
                let machine = text.and_then(|t| {
                    let table = engine::decode(&t, &NbtCodec);
                    table.map.into_values().next()
                });
                client_net(|c| {
                    if let Some(m) = c.menu.as_mut().filter(|m| m.window == window) {
                        m.machine = machine;
                    }
                });
            }
            MenuToClient::Section { window, index, data } => {
                client_net(|c| {
                    if let Some(m) = c.menu.as_mut().filter(|m| m.window == window) {
                        let i = index as usize;
                        if i < 64 {
                            if m.sections.len() <= i {
                                m.sections.resize(i + 1, Vec::new());
                            }
                            m.sections[i] = data;
                            m.generation = SECTIONS_GEN.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                });
            }
            MenuToClient::Close { window } => {
                let ours = client_net(|c| c.menu.as_ref().is_some_and(|m| m.window == window));
                if ours {
                    let pos = client_net(|c| c.menu.as_ref().map(|m| m.pos));
                    let shown = pos.is_some_and(|p| self.screen == super::super::Screen::Inventory && self.container == Some((p[0], p[1], p[2])));
                    self.forja_guest_closed(false);
                    if shown {
                        self.carried = None;
                        self.container = None;
                        self.screen = super::super::Screen::Playing;
                        self.capture(true);
                    }
                }
            }
        }
    }
    /// The guest menu is gone from this client: the world at its block as it was; `tell` = the
    /// server is told (the window closed here).
    fn forja_guest_closed(&mut self, tell: bool) {
        let Some(m) = client_net(|c| c.menu.take()) else {
            GUEST_OPEN.store(false, Ordering::Relaxed);
            return;
        };
        GUEST_OPEN.store(false, Ordering::Relaxed);
        if tell {
            send_to_server(self, engine::MENU_CHANNEL, &to_bytes(&MenuToServer::Close { window: m.window }));
        }
        let key = (m.pos[0], m.pos[1], m.pos[2]);
        // Only over our own mirror (after a reconnection the world is another one).
        let ours = matches!(
            self.world.block_entities.get(&key),
            Some(horizon_client::containers::BlockEntity::Chest(items)) if items.len() == m.size
        );
        if !ours {
            if self.container == Some(key) {
                self.container = None;
            }
            return;
        }
        match m.before {
            Some(e) => {
                self.world.block_entities.insert(key, e);
            }
            None => {
                self.world.block_entities.remove(&key);
            }
        }
        match m.name_before {
            Some(n) => {
                self.world.block_names.insert(key, n);
            }
            None => {
                self.world.block_names.remove(&key);
            }
        }
        if self.container == Some(key) {
            self.container = None;
        }
    }
    /// HEAD cancellable `menu_ops` (before every mod's own): on a guest menu, the clicks go to the
    /// server, which runs them (the mod's menu code, as this player); nothing changes here.
    pub(in crate::game) fn forja_guest_menu_ops(&mut self, ops: &[(Option<usize>, i32, horizon_client::menu::Input)]) -> Option<bool> {
        guest_menu(self)?;
        let ops: Vec<(i32, i32, u8)> = ops
            .iter()
            .map(|&(slot, button, input)| {
                (slot.and_then(|s| i32::try_from(s).ok()).unwrap_or(engine::SLOT_OUTSIDE), button, input_code(input))
            })
            .collect();
        Some(guest_send(self, MenuInput::Ops(ops)))
    }
    /// HEAD cancellable `remote_quick_move_all` (a Shift double click): on a guest menu the second
    /// shift-click goes to the server like the first (no local guess on the inventory menu).
    pub(in crate::game) fn forja_guest_quick_all(&mut self, slot: usize) -> bool {
        if guest_menu(self).is_none() {
            return false;
        }
        let Ok(slot) = i32::try_from(slot) else { return true };
        guest_send(self, MenuInput::Ops(vec![(slot, 0, 1)]));
        true
    }
    /// HEAD `close_container`: the guest menu closes on the server too.
    pub(in crate::game) fn forja_guest_close(&mut self) {
        if GUEST_OPEN.load(Ordering::Relaxed) && self.remote.is_some() {
            self.forja_guest_closed(true);
        }
    }
}

/// The client's copy of the guest menu's machine (forja's table on a server's client).
fn guest_table<R>(f: impl FnOnce(&mut Table) -> R) -> Option<R> {
    if IN_TABLE.with(|b| b.replace(true)) {
        return None;
    }
    let r = client_net(|c| {
        let m = c.menu.as_mut()?;
        let mut t = Table::default();
        if let Some(machine) = m.machine.take() {
            t.map.insert(m.pos, machine);
        }
        let r = f(&mut t);
        m.machine = t.map.remove(&m.pos);
        Some(r)
    });
    IN_TABLE.with(|b| b.set(false));
    r
}

// =================================================================================================
// ---- redstone ("Redstone"): signals mod blocks and parts emit, read through Horizon's power
// queries ----
//
// Horizon evaluates every signal source in `impl<L: LevelAccess + ?Sized> Sim<'_, L>`
// (`horizon_world::level::redstone`: `state_signal`, `state_direct_signal`, `signal`...), a generic
// impl horizon-mixins refuses. Its one non-generic call out per position is
// `LevelAccess::block_entity_signal(pos, state, direction)`, which `GameLevel` implements in
// game.rs (the trapped chest's openers, the jukebox): forja hooks it (HEAD, cancellable). It is
// reached for states of the TrappedChest / Jukebox class with the SIGNAL_SOURCE flag: weak signal on
// every side, strong (`getDirectSignal`) only towards the block below. A mod block that emits
// needs such a template (`minecraft:trapped_chest`); a forja host (template barrier) swaps to its
// `signal_block` twin while one of its parts emits.

static REDSTONE: Mutex<[engine::Redstone; 3]> = Mutex::new([engine::Redstone::new(), engine::Redstone::new(), engine::Redstone::new()]);
static REDSTONE_ANY: AtomicBool = AtomicBool::new(false);
/// What each part of a host emits, combined per position (max per side).
static REDSTONE_PARTS: Mutex<BTreeMap<(u8, Pos), BTreeMap<u8, Signal>>> = Mutex::new(BTreeMap::new());

/// `block_entity_signal`'s answer for an emitting block (None: not forja's, the game answers).
pub(in crate::game) fn redstone_answer(
    dimension: horizon_world::level::Dimension,
    pos: Pos,
    direction: Direction,
) -> Option<i32> {
    if !REDSTONE_ANY.load(Ordering::Relaxed) {
        return None;
    }
    let d: usize = match dimension {
        horizon_world::level::Dimension::Overworld => 0,
        horizon_world::level::Dimension::Nether => 1,
        horizon_world::level::Dimension::End => 2,
    };
    let query = match direction {
        Direction::Down => Dir::Down,
        Direction::Up => Dir::Up,
        Direction::North => Dir::North,
        Direction::South => Dir::South,
        Direction::West => Dir::West,
        Direction::East => Dir::East,
    };
    let t = REDSTONE.lock().unwrap_or_else(|e| e.into_inner());
    let s = t[d].get(pos);
    (!s.is_off()).then(|| engine::horizon_signal(&s, query))
}
/// What the block at `pos` emits (this level).
#[allow(dead_code)]
pub(in crate::game) fn redstone_output(game: &Game<'_>, pos: Pos) -> Signal {
    let d = usize::from(dim_code(game));
    REDSTONE.lock().unwrap_or_else(|e| e.into_inner())[d].get(pos)
}
/// The block at `pos` emits `signal` from now on (`Signal::off()` stops it). Its neighbours are
/// updated when it changed (the block itself, and through each side whose strong signal changed).
/// A forja host with a `signal_block` swaps to it while it emits.
#[allow(dead_code)]
pub(in crate::game) fn set_redstone(game: &mut Game<'_>, pos: Pos, signal: Signal) {
    if !game.forja_active() {
        return;
    }
    let d = usize::from(dim_code(game));
    let notify = {
        let mut t = REDSTONE.lock().unwrap_or_else(|e| e.into_inner());
        let n = t[d].set(pos, signal);
        REDSTONE_ANY.store(t.iter().any(|r| !r.is_empty()), Ordering::Relaxed);
        n
    };
    // The twin follows even when the value did not change (a world saved with the twin standing).
    game.forja_signal_twin(pos, !signal.is_off());
    if notify.is_empty() {
        return;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    game.with_level(|b, level, ticks| {
        let block = b.block(state);
        for q in &notify {
            b.update_neighbors_at(level, ticks, horizon_core::position::BlockPos::new(q[0], q[1], q[2]), block);
        }
    });
    let _ = game.process_level_events();
}
/// What part `slot` of the host at `pos` emits; the host emits the max of its parts, side by side.
#[allow(dead_code)]
pub(in crate::game) fn set_part_redstone(game: &mut Game<'_>, pos: Pos, slot: Slot, signal: Signal) {
    if !game.forja_active() {
        return;
    }
    let key = (dim_code(game), pos);
    let code = slot_code(slot);
    let combined = {
        let mut parts = REDSTONE_PARTS.lock().unwrap_or_else(|e| e.into_inner());
        let e = parts.entry(key).or_default();
        if signal.is_off() {
            e.remove(&code);
        } else {
            e.insert(code, signal);
        }
        let c = e.values().fold(Signal::off(), |a, s| a.max(s));
        if e.is_empty() {
            parts.remove(&key);
        }
        c
    };
    set_redstone(game, pos, combined);
}
/// The strongest signal reaching the block at `pos` from its neighbours
/// (`SignalGetter.getBestNeighborSignal`).
#[allow(dead_code)]
pub(in crate::game) fn redstone_input(game: &mut Game<'_>, pos: Pos) -> i32 {
    let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
    game.with_level(|b, level, ticks| b.best_neighbor_signal(level, ticks, at))
}
/// The block at `pos` is powered (`SignalGetter.hasNeighborSignal`).
#[allow(dead_code)]
pub(in crate::game) fn has_redstone_input(game: &mut Game<'_>, pos: Pos) -> bool {
    let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
    game.with_level(|b, level, ticks| b.has_neighbor_signal(level, ticks, at))
}
impl Game<'_> {
    /// Each tick: a part that emitted and is gone (taken out, broken) stops emitting.
    fn forja_redstone_prune(&mut self) {
        let d = dim_code(self);
        let entries: Vec<(Pos, Vec<u8>)> = {
            let parts = REDSTONE_PARTS.lock().unwrap_or_else(|e| e.into_inner());
            if parts.is_empty() {
                return;
            }
            parts.iter().filter(|((dd, _), _)| *dd == d).map(|((_, p), m)| (*p, m.keys().copied().collect())).collect()
        };
        for (p, codes) in entries {
            if block_name(self, p).is_none() {
                continue; // unloaded: as it was
            }
            for code in codes {
                let slot = code_slot(code);
                if part_at(self, p, slot).is_none() {
                    set_part_redstone(self, p, slot, Signal::off());
                }
            }
        }
    }
}
fn redstone_forget_all() {
    REDSTONE.lock().unwrap_or_else(|e| e.into_inner()).iter_mut().for_each(|r| *r = engine::Redstone::new());
    REDSTONE_PARTS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    REDSTONE_ANY.store(false, Ordering::Relaxed);
}
/// A position that stopped emitting without telling (its host is gone): forgotten quietly.
fn redstone_forget(game: &Game<'_>, pos: Pos) {
    let d = dim_code(game);
    REDSTONE_PARTS.lock().unwrap_or_else(|e| e.into_inner()).remove(&(d, pos));
    let mut t = REDSTONE.lock().unwrap_or_else(|e| e.into_inner());
    t[usize::from(d)].clear(pos);
    REDSTONE_ANY.store(t.iter().any(|r| !r.is_empty()), Ordering::Relaxed);
}

/// Host block name -> its signal twin, and back.
static TWINS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
static HAS_TWINS: AtomicBool = AtomicBool::new(false);
/// The host block a twin stands for (`ae2:cable_bus_signal` -> `ae2:cable_bus`).
fn twin_base(name: &str) -> Option<String> {
    if !HAS_TWINS.load(Ordering::Relaxed) {
        return None;
    }
    TWINS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(_, t)| t == name).map(|(b, _)| b.clone())
}
fn twin_of(base: &str) -> Option<String> {
    if !HAS_TWINS.load(Ordering::Relaxed) {
        return None;
    }
    TWINS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(b, _)| b == base).map(|(_, t)| t.clone())
}
/// Every signal twin's name (their states are host states too).
fn twin_names() -> Vec<String> {
    TWINS.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(_, t)| t.clone()).collect()
}
/// `true` when `name` is a host block or its twin standing for `block`.
#[allow(dead_code)]
pub(in crate::game) fn is_host_block(name: &str, block: &str) -> bool {
    name == block || twin_base(name).is_some_and(|b| b == block)
}

impl Game<'_> {
    /// A host at `pos` whose kind has a signal twin: the twin while it emits, the host block when
    /// it stops (the parts, their records and shapes stay: forja reads both names as the host).
    fn forja_signal_twin(&mut self, pos: Pos, emitting: bool) {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let raw = self.res.facts().get(state).name.clone();
        let base = twin_base(&raw).unwrap_or_else(|| raw.to_string());
        let Some(twin) = twin_of(&base) else { return };
        let want = if emitting { twin } else { base };
        if *raw == *want {
            return;
        }
        let registry = &self.pack.assets.registry;
        let Some(default) = registry.default_state(&want) else { return };
        // The waterlogging stays.
        let wet = registry.state(state).and_then(|s| s.properties.get("waterlogged").cloned());
        let target = match (wet, registry.state(default).cloned()) {
            (Some(w), Some(mut s)) if s.properties.contains_key("waterlogged") => {
                s.properties.insert("waterlogged".to_owned(), w);
                registry.resolve(&s).ok().unwrap_or(default)
            }
            _ => default,
        };
        let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        // The twin's class has a block entity: replacing it would remove what stands at the host's
        // place (AE2's window slots). Nothing spills (`pre_remove` is by name), and it stays.
        let key = (pos[0], pos[1], pos[2]);
        let kept = self.world.block_entities.remove(&key);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, target, horizon_world::level::flags::UPDATE_ALL));
        let _ = self.process_level_events();
        if let Some(e) = kept {
            self.world.block_entities.insert(key, e);
        }
        bump();
    }
}
