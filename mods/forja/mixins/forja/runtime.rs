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
    // Forja ("Forge for Horizon"): the shared rules the big Forge 1.20.1 tech mods need, without any
// game code. Forge Energy (`net.minecraftforge.energy.IEnergyStorage` / `EnergyStorage`), fluids
// (`FluidStack`, `IFluidHandler`, `FluidTank`, `FluidUtil`), item handlers (`IItemHandler`,
// `ItemStackHandler`, `ItemHandlerHelper`), machine state per block, progress bars.
// The class and method names in the comments are those of Forge 47.x (Minecraft 1.20.1).
// Pasted into the game as `crate::game::machines::forja::engine` (tools/hnmod.py bundle); also the
// `horizon_forja` crate the tests use: plain Rust, no game types, no inner attributes.
use std::collections::BTreeMap;

pub type Pos = [i32; 3];

// ---- directions and sides ----

/// `Direction`, in `get3DDataValue` order (the same order as Pipez).
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
        match self {
            Dir::Down => "down",
            Dir::Up => "up",
            Dir::North => "north",
            Dir::South => "south",
            Dir::West => "west",
            Dir::East => "east",
        }
    }
    pub fn parse(s: &str) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.name() == s)
    }
    pub fn from_normal(n: [i32; 3]) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.step() == n)
    }
    /// Horizontal facing turned to the block's front: `Direction` relative to a `facing`
    /// property (`north` = front). Up and down stay.
    pub fn relative(self, facing: Dir) -> Rel {
        if matches!(self, Dir::Up) {
            return Rel::Top;
        }
        if matches!(self, Dir::Down) {
            return Rel::Bottom;
        }
        if self == facing {
            return Rel::Front;
        }
        if self == facing.opposite() {
            return Rel::Back;
        }
        // clockwise of the front (seen from above) is the block's left side.
        let cw = match facing {
            Dir::North => Dir::East,
            Dir::East => Dir::South,
            Dir::South => Dir::West,
            Dir::West => Dir::North,
            other => other,
        };
        if self == cw { Rel::Left } else { Rel::Right }
    }
}
pub fn offset(p: Pos, d: Dir) -> Pos {
    let s = d.step();
    [p[0] + s[0], p[1] + s[1], p[2] + s[2]]
}

/// A side of a block relative to its front (`RelativeSide` in Mekanism, `BlockSide` elsewhere).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rel {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

/// What one side of a block lets through (Mekanism/Thermal side configuration, reduced to its
/// effect on a capability): nothing, only in, only out, both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Access {
    None,
    In,
    Out,
    #[default]
    Both,
}
impl Access {
    pub fn can_in(self) -> bool {
        matches!(self, Access::In | Access::Both)
    }
    pub fn can_out(self) -> bool {
        matches!(self, Access::Out | Access::Both)
    }
    pub fn code(self) -> char {
        match self {
            Access::None => '-',
            Access::In => 'i',
            Access::Out => 'o',
            Access::Both => 'b',
        }
    }
    pub fn from_code(c: char) -> Option<Access> {
        Some(match c {
            '-' => Access::None,
            'i' => Access::In,
            'o' => Access::Out,
            'b' => Access::Both,
            _ => return None,
        })
    }
}

/// The access of one capability on the six sides (index = `Dir::idx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sides(pub [Access; 6]);
impl Default for Sides {
    fn default() -> Self {
        Sides([Access::Both; 6])
    }
}
impl Sides {
    pub const NONE: Sides = Sides([Access::None; 6]);
    pub fn all(a: Access) -> Sides {
        Sides([a; 6])
    }
    /// `face = None` is the block itself (Forge's `null` side): full access.
    pub fn get(&self, face: Option<Dir>) -> Access {
        face.map_or(Access::Both, |d| self.0[d.idx()])
    }
    pub fn set(&mut self, d: Dir, a: Access) {
        self.0[d.idx()] = a;
    }
    pub fn code(&self) -> String {
        self.0.iter().map(|a| a.code()).collect()
    }
    pub fn from_code(s: &str) -> Option<Sides> {
        let v: Vec<Access> = s.chars().map(Access::from_code).collect::<Option<_>>()?;
        <[Access; 6]>::try_from(v).ok().map(Sides)
    }
}

// ---- Forge Energy ----

/// `IEnergyStorage`.
pub trait EnergyHandler {
    /// `receiveEnergy(maxReceive, simulate)`: what was (or would be) accepted.
    fn receive_energy(&mut self, max: i32, simulate: bool) -> i32;
    /// `extractEnergy(maxExtract, simulate)`: what was (or would be) taken.
    fn extract_energy(&mut self, max: i32, simulate: bool) -> i32;
    fn energy_stored(&self) -> i32;
    fn max_energy_stored(&self) -> i32;
    fn can_extract(&self) -> bool;
    fn can_receive(&self) -> bool;
}

/// `net.minecraftforge.energy.EnergyStorage` (int FE).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EnergyStorage {
    pub energy: i32,
    pub capacity: i32,
    pub max_receive: i32,
    pub max_extract: i32,
}
impl EnergyStorage {
    /// `new EnergyStorage(capacity)`: transfer limits equal to the capacity.
    pub fn new(capacity: i32) -> Self {
        Self::with_rates(capacity, capacity, capacity)
    }
    /// `new EnergyStorage(capacity, maxReceive, maxExtract)`.
    pub fn with_rates(capacity: i32, max_receive: i32, max_extract: i32) -> Self {
        EnergyStorage {
            energy: 0,
            capacity: capacity.max(0),
            max_receive: max_receive.max(0),
            max_extract: max_extract.max(0),
        }
    }
    /// The block itself filling its buffer (a generator burning fuel): no `max_receive` limit,
    /// only the room left. What fitted.
    pub fn generate(&mut self, amount: i32) -> i32 {
        let n = amount.max(0).min(self.capacity - self.energy);
        self.energy += n;
        n
    }
    /// The block itself using energy (a machine working): all or nothing.
    pub fn consume(&mut self, amount: i32) -> bool {
        if amount < 0 || self.energy < amount {
            return false;
        }
        self.energy -= amount;
        true
    }
    /// `0..=len` pixels for a gauge (`getScaled`).
    pub fn scaled(&self, len: i32) -> i32 {
        scaled(self.energy, self.capacity, len)
    }
}
impl EnergyHandler for EnergyStorage {
    fn receive_energy(&mut self, max: i32, simulate: bool) -> i32 {
        if !self.can_receive() {
            return 0;
        }
        // Forge: min(capacity - energy, min(maxReceive, max)); a negative request would make it
        // negative and drain the storage, so it is treated as 0 here.
        let n = (self.capacity - self.energy).min(self.max_receive.min(max.max(0))).max(0);
        if !simulate {
            self.energy += n;
        }
        n
    }
    fn extract_energy(&mut self, max: i32, simulate: bool) -> i32 {
        if !self.can_extract() {
            return 0;
        }
        let n = self.energy.min(self.max_extract.min(max.max(0))).max(0);
        if !simulate {
            self.energy -= n;
        }
        n
    }
    fn energy_stored(&self) -> i32 {
        self.energy
    }
    fn max_energy_stored(&self) -> i32 {
        self.capacity
    }
    fn can_extract(&self) -> bool {
        self.max_extract > 0
    }
    fn can_receive(&self) -> bool {
        self.max_receive > 0
    }
}

/// A storage seen through one side: `In` refuses extraction, `Out` refuses receiving, `None`
/// refuses both (the capability is absent there: callers check `Access::None` first).
pub struct SidedEnergy<'a> {
    pub storage: &'a mut EnergyStorage,
    pub access: Access,
}
impl EnergyHandler for SidedEnergy<'_> {
    fn receive_energy(&mut self, max: i32, simulate: bool) -> i32 {
        if !self.access.can_in() {
            return 0;
        }
        self.storage.receive_energy(max, simulate)
    }
    fn extract_energy(&mut self, max: i32, simulate: bool) -> i32 {
        if !self.access.can_out() {
            return 0;
        }
        self.storage.extract_energy(max, simulate)
    }
    fn energy_stored(&self) -> i32 {
        self.storage.energy
    }
    fn max_energy_stored(&self) -> i32 {
        self.storage.capacity
    }
    fn can_extract(&self) -> bool {
        self.access.can_out() && self.storage.can_extract()
    }
    fn can_receive(&self) -> bool {
        self.access.can_in() && self.storage.can_receive()
    }
}

/// A snapshot of an energy handler (for gauges, tooltips, probes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnergyInfo {
    pub stored: i32,
    pub capacity: i32,
    pub can_receive: bool,
    pub can_extract: bool,
}
impl EnergyInfo {
    pub fn of(h: &dyn EnergyHandler) -> Self {
        EnergyInfo {
            stored: h.energy_stored(),
            capacity: h.max_energy_stored(),
            can_receive: h.can_receive(),
            can_extract: h.can_extract(),
        }
    }
}

/// `EnergyUtils.pushEnergy` (Pipez) with both handlers at hand: simulate both sides, then move
/// what both agree on. Never creates or destroys energy when the handlers keep their word.
pub fn push_energy(from: &mut dyn EnergyHandler, to: &mut dyn EnergyHandler, max: i32) -> i32 {
    let offer = from.extract_energy(max, true);
    if offer <= 0 {
        return 0;
    }
    let accepted = to.receive_energy(offer, true);
    if accepted <= 0 {
        return 0;
    }
    let taken = from.extract_energy(accepted, false);
    let put = to.receive_energy(taken, false);
    if put < taken {
        // The receiver broke its simulation: give the rest back rather than lose it.
        from.receive_energy(taken - put, false);
    }
    put
}

// ---- Fluids ----

/// `FluidType.BUCKET_VOLUME`.
pub const BUCKET_VOLUME: i32 = 1000;

/// `FluidStack` (fluid id + mB). The NBT tag of a FluidStack is not modelled (no 1.20.1 fluid
/// that the pack's mods move by pipe relies on it; see FUNDATIE-FORJA.md).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FluidStack {
    pub fluid: String,
    pub amount: i32,
}
impl FluidStack {
    pub fn new(fluid: &str, amount: i32) -> Self {
        FluidStack {
            fluid: fluid.to_owned(),
            amount,
        }
    }
    /// `FluidStack.EMPTY`.
    pub fn empty() -> Self {
        FluidStack::default()
    }
    /// `isEmpty`: no fluid (`minecraft:empty`) or no amount.
    pub fn is_empty(&self) -> bool {
        self.amount <= 0 || self.fluid.is_empty() || self.fluid == "minecraft:empty"
    }
    /// `isFluidEqual`.
    pub fn same_fluid(&self, other: &FluidStack) -> bool {
        !self.is_empty() && !other.is_empty() && self.fluid == other.fluid
    }
    pub fn with_amount(&self, amount: i32) -> Self {
        FluidStack {
            fluid: self.fluid.clone(),
            amount,
        }
    }
}

/// `IFluidHandler`.
pub trait FluidHandler {
    fn tanks(&self) -> usize;
    fn fluid_in_tank(&self, tank: usize) -> FluidStack;
    fn tank_capacity(&self, tank: usize) -> i32;
    fn is_fluid_valid(&self, tank: usize, stack: &FluidStack) -> bool;
    /// `fill(resource, action)`: mB accepted.
    fn fill(&mut self, resource: &FluidStack, simulate: bool) -> i32;
    /// `drain(resource, action)`: only that fluid.
    fn drain(&mut self, resource: &FluidStack, simulate: bool) -> FluidStack;
    /// `drain(maxDrain, action)`: whatever is first.
    fn drain_max(&mut self, max: i32, simulate: bool) -> FluidStack;
}

/// `FluidTank` (one fluid, a capacity, an optional validator by fluid id).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FluidTank {
    pub fluid: FluidStack,
    pub capacity: i32,
    /// `validator`: the fluids this tank takes (`None` = any).
    pub allowed: Option<Vec<String>>,
}
impl FluidTank {
    pub fn new(capacity: i32) -> Self {
        FluidTank {
            fluid: FluidStack::empty(),
            capacity: capacity.max(0),
            allowed: None,
        }
    }
    pub fn only(capacity: i32, fluids: &[&str]) -> Self {
        FluidTank {
            allowed: Some(fluids.iter().map(|f| (*f).to_owned()).collect()),
            ..FluidTank::new(capacity)
        }
    }
    pub fn amount(&self) -> i32 {
        if self.fluid.is_empty() { 0 } else { self.fluid.amount }
    }
    pub fn space(&self) -> i32 {
        (self.capacity - self.amount()).max(0)
    }
    pub fn is_valid(&self, stack: &FluidStack) -> bool {
        self.allowed
            .as_ref()
            .is_none_or(|a| a.iter().any(|f| *f == stack.fluid))
    }
    pub fn scaled(&self, len: i32) -> i32 {
        scaled(self.amount(), self.capacity, len)
    }
    /// `FluidTank.fill`: refuses another fluid (no mixing) and invalid fluids.
    pub fn fill(&mut self, resource: &FluidStack, simulate: bool) -> i32 {
        if resource.is_empty() || !self.is_valid(resource) {
            return 0;
        }
        if self.fluid.is_empty() {
            let n = resource.amount.min(self.capacity);
            if !simulate && n > 0 {
                self.fluid = resource.with_amount(n);
            }
            return n;
        }
        if !self.fluid.same_fluid(resource) {
            return 0;
        }
        let n = (self.capacity - self.fluid.amount).min(resource.amount).max(0);
        if !simulate {
            self.fluid.amount += n;
        }
        n
    }
    /// `FluidTank.drain(resource, action)`.
    pub fn drain(&mut self, resource: &FluidStack, simulate: bool) -> FluidStack {
        if resource.is_empty() || !resource.same_fluid(&self.fluid) {
            return FluidStack::empty();
        }
        self.drain_max(resource.amount, simulate)
    }
    /// `FluidTank.drain(maxDrain, action)`.
    pub fn drain_max(&mut self, max: i32, simulate: bool) -> FluidStack {
        if self.fluid.is_empty() || max <= 0 {
            return FluidStack::empty();
        }
        let n = self.fluid.amount.min(max);
        let out = self.fluid.with_amount(n);
        if !simulate {
            self.fluid.amount -= n;
            if self.fluid.amount <= 0 {
                self.fluid = FluidStack::empty();
            }
        }
        out
    }
}
impl FluidHandler for FluidTank {
    fn tanks(&self) -> usize {
        1
    }
    fn fluid_in_tank(&self, _tank: usize) -> FluidStack {
        self.fluid.clone()
    }
    fn tank_capacity(&self, _tank: usize) -> i32 {
        self.capacity
    }
    fn is_fluid_valid(&self, _tank: usize, stack: &FluidStack) -> bool {
        self.is_valid(stack)
    }
    fn fill(&mut self, resource: &FluidStack, simulate: bool) -> i32 {
        FluidTank::fill(self, resource, simulate)
    }
    fn drain(&mut self, resource: &FluidStack, simulate: bool) -> FluidStack {
        FluidTank::drain(self, resource, simulate)
    }
    fn drain_max(&mut self, max: i32, simulate: bool) -> FluidStack {
        FluidTank::drain_max(self, max, simulate)
    }
}

/// What each tank of a machine lets through from outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TankRole {
    /// Filled from outside, drained only by the machine (an input tank).
    Input,
    /// Drained from outside, filled only by the machine (an output tank).
    Output,
    /// Both (a plain tank).
    Both,
}

/// Several tanks behind one handler (`FluidHandlerItemStack`-like multi-tank machines), seen
/// from one side. Fill: tanks already holding the fluid first, then empty ones, in order.
/// Drain: the first tank holding the fluid (or anything, for `drain_max`).
pub struct TankSet<'a> {
    pub tanks: &'a mut [FluidTank],
    pub roles: &'a [TankRole],
    pub access: Access,
    /// The machine itself (no role or side limits).
    pub internal: bool,
}
impl TankSet<'_> {
    fn may_fill(&self, i: usize) -> bool {
        self.internal
            || (self.access.can_in()
                && matches!(self.roles.get(i), Some(TankRole::Input | TankRole::Both) | None))
    }
    fn may_drain(&self, i: usize) -> bool {
        self.internal
            || (self.access.can_out()
                && matches!(self.roles.get(i), Some(TankRole::Output | TankRole::Both) | None))
    }
}
impl FluidHandler for TankSet<'_> {
    fn tanks(&self) -> usize {
        self.tanks.len()
    }
    fn fluid_in_tank(&self, tank: usize) -> FluidStack {
        self.tanks.get(tank).map(|t| t.fluid.clone()).unwrap_or_default()
    }
    fn tank_capacity(&self, tank: usize) -> i32 {
        self.tanks.get(tank).map_or(0, |t| t.capacity)
    }
    fn is_fluid_valid(&self, tank: usize, stack: &FluidStack) -> bool {
        self.tanks.get(tank).is_some_and(|t| t.is_valid(stack))
    }
    fn fill(&mut self, resource: &FluidStack, simulate: bool) -> i32 {
        if resource.is_empty() {
            return 0;
        }
        let mut left = resource.clone();
        let mut filled = 0;
        // Tanks with this fluid, then empty ones.
        for pass in 0..2 {
            for i in 0..self.tanks.len() {
                if left.amount <= 0 {
                    return filled;
                }
                let holds = self.tanks[i].fluid.same_fluid(&left);
                let empty = self.tanks[i].fluid.is_empty();
                if (pass == 0 && !holds) || (pass == 1 && !empty) || !self.may_fill(i) {
                    continue;
                }
                let n = self.tanks[i].fill(&left, simulate);
                filled += n;
                left.amount -= n;
            }
        }
        filled
    }
    fn drain(&mut self, resource: &FluidStack, simulate: bool) -> FluidStack {
        for i in 0..self.tanks.len() {
            if self.may_drain(i) && self.tanks[i].fluid.same_fluid(resource) {
                let d = self.tanks[i].drain(resource, simulate);
                if !d.is_empty() {
                    return d;
                }
            }
        }
        FluidStack::empty()
    }
    fn drain_max(&mut self, max: i32, simulate: bool) -> FluidStack {
        for i in 0..self.tanks.len() {
            if self.may_drain(i) && !self.tanks[i].fluid.is_empty() {
                return self.tanks[i].drain_max(max, simulate);
            }
        }
        FluidStack::empty()
    }
}

/// A snapshot of one tank (for gauges and pipes choosing what to drain).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TankInfo {
    pub fluid: FluidStack,
    pub capacity: i32,
}
pub fn tank_infos(h: &dyn FluidHandler) -> Vec<TankInfo> {
    (0..h.tanks())
        .map(|i| TankInfo {
            fluid: h.fluid_in_tank(i),
            capacity: h.tank_capacity(i),
        })
        .collect()
}

/// `FluidUtil.tryFluidTransfer(dest, source, resource or max, true)` with both handlers at hand:
/// simulate the drain and the fill, then drain what fits and fill what was drained.
pub fn try_fluid_transfer(
    dest: &mut dyn FluidHandler,
    source: &mut dyn FluidHandler,
    resource: Option<&FluidStack>,
    max: i32,
) -> FluidStack {
    let offer = match resource {
        Some(r) => source.drain(&r.with_amount(r.amount.min(max)), true),
        None => source.drain_max(max, true),
    };
    if offer.is_empty() {
        return FluidStack::empty();
    }
    let fits = dest.fill(&offer, true);
    if fits <= 0 {
        return FluidStack::empty();
    }
    let drained = source.drain(&offer.with_amount(fits), false);
    if drained.is_empty() {
        return FluidStack::empty();
    }
    let put = dest.fill(&drained, false);
    if put < drained.amount {
        // The destination broke its simulation: the rest goes back where it came from.
        source.fill(&drained.with_amount(drained.amount - put), false);
    }
    drained.with_amount(put)
}

/// The fluid buckets of 1.20.1 (`BucketItem`, `MilkBucketItem` with Forge's milk fluid, which
/// mods enable): (filled bucket, fluid, fill sound, empty sound).
pub const BUCKETS: [(&str, &str, &str, &str); 3] = [
    ("minecraft:water_bucket", "minecraft:water", "minecraft:item.bucket.fill", "minecraft:item.bucket.empty"),
    ("minecraft:lava_bucket", "minecraft:lava", "minecraft:item.bucket.fill_lava", "minecraft:item.bucket.empty_lava"),
    ("minecraft:milk_bucket", "minecraft:milk", "minecraft:item.bucket.fill", "minecraft:item.bucket.empty"),
];
pub const EMPTY_BUCKET: &str = "minecraft:bucket";

/// Extra buckets of mod fluids (`ForgeFlowingFluid` buckets): (bucket item, fluid, fill sound,
/// empty sound). Registered by the mods (`register_bucket` in the game).
pub fn bucket_fluid(item: &str, extra: &[(String, String)]) -> Option<String> {
    BUCKETS
        .iter()
        .find(|b| b.0 == item)
        .map(|b| b.1.to_owned())
        .or_else(|| extra.iter().find(|b| b.0 == item).map(|b| b.1.clone()))
}
pub fn filled_bucket(fluid: &str, extra: &[(String, String)]) -> Option<String> {
    BUCKETS
        .iter()
        .find(|b| b.1 == fluid)
        .map(|b| b.0.to_owned())
        .or_else(|| extra.iter().find(|b| b.1 == fluid).map(|b| b.0.clone()))
}
/// The sound of filling (`true`) or emptying a bucket of `fluid`.
pub fn bucket_sound(fluid: &str, fill: bool) -> &'static str {
    let lava = fluid == "minecraft:lava";
    match (fill, lava) {
        (true, true) => "minecraft:item.bucket.fill_lava",
        (true, false) => "minecraft:item.bucket.fill",
        (false, true) => "minecraft:item.bucket.empty_lava",
        (false, false) => "minecraft:item.bucket.empty",
    }
}

/// What `FluidUtil.interactWithFluidHandler` did with the held bucket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BucketUse {
    Nothing,
    /// The empty bucket took 1000 mB and becomes this item.
    Filled { bucket: String, sound: &'static str },
    /// The bucket gave its 1000 mB and becomes an empty bucket.
    Emptied { sound: &'static str },
}

/// `FluidUtil.interactWithFluidHandler(player, hand, handler)` for one bucket: first try to fill
/// the held container from the handler (`tryFillContainer`), then to empty it into the handler
/// (`tryEmptyContainer`). A bucket moves exactly 1000 mB or nothing.
pub fn bucket_interact(held: &str, h: &mut dyn FluidHandler, extra: &[(String, String)]) -> BucketUse {
    if held == EMPTY_BUCKET {
        let offer = h.drain_max(BUCKET_VOLUME, true);
        if offer.amount == BUCKET_VOLUME
            && let Some(bucket) = filled_bucket(&offer.fluid, extra)
        {
            h.drain(&offer, false);
            return BucketUse::Filled {
                bucket,
                sound: bucket_sound(&offer.fluid, true),
            };
        }
        return BucketUse::Nothing;
    }
    if let Some(fluid) = bucket_fluid(held, extra) {
        let stack = FluidStack::new(&fluid, BUCKET_VOLUME);
        if h.fill(&stack, true) == BUCKET_VOLUME {
            h.fill(&stack, false);
            return BucketUse::Emptied {
                sound: bucket_sound(&fluid, false),
            };
        }
    }
    BucketUse::Nothing
}

// ---- Items ----

/// An item stack as the rules see it (the game's `ItemStack`; a fake one in the tests).
pub trait Stack: Clone {
    fn item(&self) -> &str;
    fn count(&self) -> u32;
    fn set_count(&mut self, count: u32);
    /// Same item and same components (`ItemHandlerHelper.canItemStacksStack`).
    fn same_kind(&self, other: &Self) -> bool;
}
fn with_count<S: Stack>(s: &S, n: u32) -> S {
    let mut c = s.clone();
    c.set_count(n);
    c
}

/// `IItemHandler`. A remainder of `None` means everything went in.
pub trait ItemHandler<S: Stack> {
    fn slots(&self) -> usize;
    fn stack_in_slot(&self, slot: usize) -> Option<S>;
    fn insert_item(&mut self, slot: usize, stack: S, simulate: bool) -> Option<S>;
    fn extract_item(&mut self, slot: usize, amount: u32, simulate: bool) -> Option<S>;
    fn slot_limit(&self, slot: usize) -> u32;
    fn is_item_valid(&self, slot: usize, stack: &S) -> bool;
}

/// What a slot of a machine lets through from outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotRole {
    /// Inserted from outside, taken only by the machine.
    Input,
    /// Taken from outside, filled only by the machine.
    Output,
    Both,
    /// Only the machine and its menu (upgrades, a battery slot).
    Internal,
}

/// `ItemStackHandler`: the slots of a side-table inventory.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Inventory<S> {
    pub slots: Vec<Option<S>>,
}
impl<S: Stack> Inventory<S> {
    pub fn new(n: usize) -> Self {
        Inventory {
            slots: vec![None; n],
        }
    }
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
    /// Every stack, emptying the inventory (what a broken machine drops).
    pub fn take_all(&mut self) -> Vec<S> {
        self.slots.iter_mut().filter_map(Option::take).collect()
    }
}

/// An inventory seen from one side: roles, side access, the stack limits of the game.
pub struct InvView<'a, S> {
    pub inv: &'a mut Inventory<S>,
    pub roles: &'a [SlotRole],
    pub access: Access,
    /// The machine itself or its menu: every slot, no side limits.
    pub internal: bool,
    /// `getMaxStackSize` of a stack (the game's item info; 64 in tests).
    pub max_stack: &'a dyn Fn(&S) -> u32,
    /// `isItemValid` of the machine (recipes, fuels); `None` = anything.
    pub valid: Option<&'a dyn Fn(usize, &S) -> bool>,
}
impl<S: Stack> InvView<'_, S> {
    fn role(&self, slot: usize) -> SlotRole {
        self.roles.get(slot).copied().unwrap_or(SlotRole::Both)
    }
    fn may_insert(&self, slot: usize) -> bool {
        self.internal
            || (self.access.can_in() && matches!(self.role(slot), SlotRole::Input | SlotRole::Both))
    }
    fn may_extract(&self, slot: usize) -> bool {
        self.internal
            || (self.access.can_out() && matches!(self.role(slot), SlotRole::Output | SlotRole::Both))
    }
}
impl<S: Stack> ItemHandler<S> for InvView<'_, S> {
    fn slots(&self) -> usize {
        self.inv.slots.len()
    }
    fn stack_in_slot(&self, slot: usize) -> Option<S> {
        self.inv.slots.get(slot).cloned().flatten()
    }
    fn slot_limit(&self, _slot: usize) -> u32 {
        64
    }
    fn is_item_valid(&self, slot: usize, stack: &S) -> bool {
        self.valid.is_none_or(|v| v(slot, stack))
    }
    /// `ItemStackHandler.insertItem`.
    fn insert_item(&mut self, slot: usize, stack: S, simulate: bool) -> Option<S> {
        if stack.count() == 0 {
            return None;
        }
        if slot >= self.inv.slots.len() || !self.may_insert(slot) || !self.is_item_valid(slot, &stack) {
            return Some(stack);
        }
        let limit = self.slot_limit(slot).min((self.max_stack)(&stack));
        let existing = self.inv.slots[slot].clone();
        let mut room = limit;
        if let Some(e) = &existing {
            if !e.same_kind(&stack) {
                return Some(stack);
            }
            room = room.saturating_sub(e.count());
        }
        if room == 0 {
            return Some(stack);
        }
        let n = stack.count().min(room);
        if !simulate {
            self.inv.slots[slot] = Some(match existing {
                Some(e) => with_count(&e, e.count() + n),
                None => with_count(&stack, n),
            });
        }
        (stack.count() > n).then(|| with_count(&stack, stack.count() - n))
    }
    /// `ItemStackHandler.extractItem`.
    fn extract_item(&mut self, slot: usize, amount: u32, simulate: bool) -> Option<S> {
        if amount == 0 || slot >= self.inv.slots.len() || !self.may_extract(slot) {
            return None;
        }
        let existing = self.inv.slots[slot].clone()?;
        let n = existing.count().min(amount).min((self.max_stack)(&existing));
        if !simulate {
            self.inv.slots[slot] = (existing.count() > n).then(|| with_count(&existing, existing.count() - n));
        }
        Some(with_count(&existing, n))
    }
}

/// `ItemHandlerHelper.insertItem(dest, stack, simulate)`: slot by slot; the remainder.
pub fn insert_item<S: Stack>(h: &mut dyn ItemHandler<S>, stack: S, simulate: bool) -> Option<S> {
    let mut rest = Some(stack);
    for slot in 0..h.slots() {
        let Some(s) = rest.take() else { return None };
        rest = h.insert_item(slot, s, simulate);
    }
    rest
}
/// `ItemHandlerHelper.insertItemStacked`: stacks of the same kind first, then empty slots.
pub fn insert_item_stacked<S: Stack>(h: &mut dyn ItemHandler<S>, stack: S, simulate: bool) -> Option<S> {
    let mut rest = Some(stack);
    for pass in 0..2 {
        for slot in 0..h.slots() {
            let Some(s) = rest.take() else { return None };
            let current = h.stack_in_slot(slot);
            let wanted = match pass {
                0 => current.as_ref().is_some_and(|c| c.same_kind(&s)),
                _ => current.is_none(),
            };
            rest = if wanted { h.insert_item(slot, s, simulate) } else { Some(s) };
        }
    }
    rest
}
/// Moves up to `max` items from `from` to `to`, first movable slot first (a hopper-like pull).
/// Simulates both sides before touching anything. Returns the number moved and what could go
/// neither in nor back (only when a handler breaks its simulation): the caller drops it.
pub fn move_items<S: Stack>(from: &mut dyn ItemHandler<S>, to: &mut dyn ItemHandler<S>, max: u32) -> (u32, Vec<S>) {
    let mut moved = 0;
    let mut stray = Vec::new();
    for slot in 0..from.slots() {
        if moved >= max {
            break;
        }
        let Some(offer) = from.extract_item(slot, max - moved, true) else {
            continue;
        };
        let offered = offer.count();
        let fits = offered - insert_item_stacked(to, offer, true).map_or(0, |r| r.count());
        if fits == 0 {
            continue;
        }
        let Some(taken) = from.extract_item(slot, fits, false) else {
            continue;
        };
        let got = taken.count();
        match insert_item_stacked(to, taken, false) {
            None => moved += got,
            Some(back) => {
                moved += got - back.count();
                if let Some(s) = from.insert_item(slot, back, false) {
                    stray.push(s);
                }
            }
        }
    }
    (moved, stray)
}

// ---- machine state (per block, in a side table) ----

/// One tank of a machine spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TankSpec {
    pub capacity: i32,
    pub role: TankRole,
    /// `None` = any fluid.
    pub fluids: Option<Vec<String>>,
}

/// What a kind of machine block has (registered once per block name by the mod).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    /// The block name (`"powah:energy_cell_basic"`).
    pub block: String,
    /// `None` = no energy capability.
    pub energy: Option<EnergyStorage>,
    pub tanks: Vec<TankSpec>,
    /// Side-table item slots (0 when the block's own container block entity holds the items).
    pub slots: Vec<SlotRole>,
    /// Default side access per capability.
    pub energy_sides: Sides,
    pub fluid_sides: Sides,
    pub item_sides: Sides,
}
impl Spec {
    pub fn new(block: &str) -> Self {
        Spec {
            block: block.to_owned(),
            energy: None,
            tanks: Vec::new(),
            slots: Vec::new(),
            energy_sides: Sides::default(),
            fluid_sides: Sides::default(),
            item_sides: Sides::default(),
        }
    }
    pub fn energy(mut self, capacity: i32, max_receive: i32, max_extract: i32) -> Self {
        self.energy = Some(EnergyStorage::with_rates(capacity, max_receive, max_extract));
        self
    }
    pub fn tank(mut self, capacity: i32, role: TankRole, fluids: Option<&[&str]>) -> Self {
        self.tanks.push(TankSpec {
            capacity,
            role,
            fluids: fluids.map(|f| f.iter().map(|s| (*s).to_owned()).collect()),
        });
        self
    }
    pub fn slots(mut self, roles: &[SlotRole]) -> Self {
        self.slots = roles.to_vec();
        self
    }
    pub fn sides(mut self, energy: Sides, fluid: Sides, items: Sides) -> Self {
        self.energy_sides = energy;
        self.fluid_sides = fluid;
        self.item_sides = items;
        self
    }
    /// A fresh machine of this kind (just placed).
    pub fn create<S: Stack>(&self) -> Machine<S> {
        Machine {
            kind: self.block.clone(),
            energy: self.energy,
            tanks: self
                .tanks
                .iter()
                .map(|t| FluidTank {
                    fluid: FluidStack::empty(),
                    capacity: t.capacity,
                    allowed: t.fluids.clone(),
                })
                .collect(),
            tank_roles: self.tanks.iter().map(|t| t.role).collect(),
            items: Inventory::new(self.slots.len()),
            slot_roles: self.slots.clone(),
            energy_sides: self.energy_sides,
            fluid_sides: self.fluid_sides,
            item_sides: self.item_sides,
            progress: 0,
            max_progress: 0,
            data: BTreeMap::new(),
            dirty: true,
        }
    }
}

/// The state of one machine block: what a `BlockEntity` with capabilities keeps.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine<S> {
    pub kind: String,
    pub energy: Option<EnergyStorage>,
    pub tanks: Vec<FluidTank>,
    pub tank_roles: Vec<TankRole>,
    pub items: Inventory<S>,
    pub slot_roles: Vec<SlotRole>,
    pub energy_sides: Sides,
    pub fluid_sides: Sides,
    pub item_sides: Sides,
    /// Work done and work needed (progress arrows).
    pub progress: i32,
    pub max_progress: i32,
    /// Anything else the mod wants saved (burn time, redstone mode, a selected recipe...).
    pub data: BTreeMap<String, i64>,
    /// Changed since the last save (`setChanged`).
    pub dirty: bool,
}
/// The data key of an energy capacity set by the mod (Mekanism energy upgrades: the storage
/// grows with the upgrades); 0 / absent = the spec's capacity.
pub const CAPACITY_KEY: &str = "forja:capacity";

impl<S: Stack> Machine<S> {
    /// Sets this machine's energy capacity (saved in `data`; `conform` keeps it). The stored
    /// energy is clamped to it (`MachineEnergyContainer`: the excess is lost when upgrades come
    /// out, as in Mekanism). `0` goes back to the spec's capacity at the next `conform`.
    pub fn set_energy_capacity(&mut self, capacity: i32) {
        self.set(CAPACITY_KEY, i64::from(capacity.max(0)));
        if capacity > 0
            && let Some(e) = self.energy.as_mut()
            && e.capacity != capacity
        {
            e.capacity = capacity;
            e.energy = e.energy.min(capacity);
            self.dirty = true;
        }
    }
    pub fn get(&self, key: &str) -> i64 {
        self.data.get(key).copied().unwrap_or(0)
    }
    pub fn set(&mut self, key: &str, v: i64) {
        if self.data.get(key) != Some(&v) {
            self.data.insert(key.to_owned(), v);
            self.dirty = true;
        }
    }
    /// The energy handler seen through `face` (`None` = the machine itself); `None` when the
    /// machine has no energy or that side has no access (`getCapability` returns empty).
    pub fn energy_handler(&mut self, face: Option<Dir>) -> Option<SidedEnergy<'_>> {
        let access = self.energy_sides.get(face);
        if access == Access::None {
            return None;
        }
        self.energy.as_mut().map(|storage| SidedEnergy { storage, access })
    }
    pub fn fluid_handler(&mut self, face: Option<Dir>) -> Option<TankSet<'_>> {
        let access = self.fluid_sides.get(face);
        if self.tanks.is_empty() || access == Access::None {
            return None;
        }
        Some(TankSet {
            tanks: &mut self.tanks,
            roles: &self.tank_roles,
            access,
            internal: face.is_none(),
        })
    }
    pub fn item_handler<'a>(
        &'a mut self,
        face: Option<Dir>,
        max_stack: &'a dyn Fn(&S) -> u32,
    ) -> Option<InvView<'a, S>> {
        let access = self.item_sides.get(face);
        if self.items.slots.is_empty() || access == Access::None {
            return None;
        }
        Some(InvView {
            inv: &mut self.items,
            roles: &self.slot_roles,
            access,
            internal: face.is_none(),
            max_stack,
            valid: None,
        })
    }
    /// Brings a loaded machine in line with its (possibly changed) spec: capacities, rates and
    /// sizes follow the spec; energy and fluids are clamped; items of removed slots and fluid of
    /// removed tanks are returned so the game drops them (fluid cannot be dropped: it is reported).
    pub fn conform(&mut self, spec: &Spec) -> (Vec<S>, Vec<FluidStack>) {
        let mut drops = Vec::new();
        let mut spilled = Vec::new();
        // A capacity the mod set (upgrades, `set_energy_capacity`) wins over the spec's.
        let own = self.get(CAPACITY_KEY);
        let energy = match (self.energy, spec.energy) {
            (_, None) => None,
            (old, Some(new)) => {
                let capacity = if own > 0 { own.min(i64::from(i32::MAX)) as i32 } else { new.capacity };
                Some(EnergyStorage {
                    energy: old.map_or(0, |o| o.energy).min(capacity).max(0),
                    capacity,
                    ..new
                })
            }
        };
        if energy != self.energy {
            self.energy = energy;
            self.dirty = true;
        }
        let mut tanks = Vec::new();
        for (i, t) in spec.tanks.iter().enumerate() {
            let mut tank = FluidTank {
                fluid: FluidStack::empty(),
                capacity: t.capacity,
                allowed: t.fluids.clone(),
            };
            if let Some(old) = self.tanks.get(i) {
                let mut f = old.fluid.clone();
                if f.amount > t.capacity {
                    spilled.push(f.with_amount(f.amount - t.capacity));
                    f.amount = t.capacity;
                }
                tank.fluid = f;
            }
            tanks.push(tank);
        }
        for old in self.tanks.iter().skip(spec.tanks.len()) {
            if !old.fluid.is_empty() {
                spilled.push(old.fluid.clone());
            }
        }
        let roles: Vec<TankRole> = spec.tanks.iter().map(|t| t.role).collect();
        if tanks != self.tanks || roles != self.tank_roles {
            self.tanks = tanks;
            self.tank_roles = roles;
            self.dirty = true;
        }
        if self.items.slots.len() != spec.slots.len() {
            let n = spec.slots.len();
            if self.items.slots.len() > n {
                drops.extend(self.items.slots.drain(n..).flatten());
            } else {
                self.items.slots.resize(n, None);
            }
            self.dirty = true;
        }
        if self.slot_roles != spec.slots {
            self.slot_roles = spec.slots.clone();
            self.dirty = true;
        }
        (drops, spilled)
    }
}

/// The machines of one dimension, by position.
#[derive(Clone, Debug, PartialEq)]
pub struct Machines<S> {
    pub map: BTreeMap<Pos, Machine<S>>,
    /// Items waiting to be dropped in the world at a position (a broken machine's contents, the
    /// slots a changed spec removed). Saved with the table until the game spawns them, so a save
    /// between the break and the drop loses nothing.
    pub drops: Vec<(Pos, S)>,
    /// Something changed that is not a machine's own `dirty` (a removal, drops, a host's parts).
    pub changed: bool,
    /// The compound blocks (multipart hosts) of the dimension: their parts, by position.
    pub hosts: BTreeMap<Pos, Host>,
}
impl<S> Default for Machines<S> {
    fn default() -> Self {
        Machines {
            map: BTreeMap::new(),
            drops: Vec::new(),
            changed: false,
            hosts: BTreeMap::new(),
        }
    }
}
impl<S: Stack> Machines<S> {
    /// The machine at `pos`, made from `spec` if the table does not know it yet (placed by a
    /// command, or the first use after the mod was added). A record of another kind at the same
    /// place (the block was replaced between two ticks) drops its items first.
    pub fn ensure(&mut self, pos: Pos, spec: &Spec) -> &mut Machine<S> {
        if self.map.get(&pos).is_some_and(|m| m.kind != spec.block) {
            self.remove(pos);
        }
        let m = self.map.entry(pos).or_insert_with(|| spec.create());
        let (items, _fluids) = m.conform(spec);
        for s in items {
            self.drops.push((pos, s));
            self.changed = true;
        }
        self.map.get_mut(&pos).expect("just inserted")
    }
    /// The block is gone: its items wait in `drops`; fluids and energy vanish, as when a Forge
    /// machine is broken without a wrench that keeps them. Returns how many stacks will drop.
    pub fn remove(&mut self, pos: Pos) -> usize {
        let Some(mut m) = self.map.remove(&pos) else {
            return 0;
        };
        let items = m.items.take_all();
        let n = items.len();
        self.drops.extend(items.into_iter().map(|s| (pos, s)));
        self.changed = true;
        n
    }
    /// The drops for the game to spawn (the table forgets them).
    pub fn take_drops(&mut self) -> Vec<(Pos, S)> {
        if !self.drops.is_empty() {
            self.changed = true;
        }
        std::mem::take(&mut self.drops)
    }
    pub fn dirty(&self) -> bool {
        self.changed || self.map.values().any(|m| m.dirty)
    }
    pub fn clean(&mut self) {
        self.changed = false;
        for m in self.map.values_mut() {
            m.dirty = false;
        }
    }
}

/// How a stack is written in the side file: one line without spaces (the game writes NBT bytes
/// as hex; the tests a plain `item*count`).
pub trait StackCodec<S> {
    fn encode(&self, s: &S) -> String;
    fn decode(&self, text: &str) -> Option<S>;
}

/// The side file of one dimension (`<world>/data/forja/<dimension>.txt`):
/// ```text
/// forja 1
/// m <x> <y> <z> <block>
/// e <energy> <capacity> <max_receive> <max_extract>
/// t <capacity> <role i/o/b> <amount> <fluid or -> [<allowed fluid> ...]
/// i <slots> <roles: one of i o b n per slot>
/// s <slot> <stack>
/// p <progress> <max_progress>
/// sides <energy> <fluid> <items>      (6 of - i o b each)
/// d <key> <value>
/// x <x> <y> <z> <stack>               (an item still to drop there)
/// h <x> <y> <z> <host block>          (a compound block)
/// hp <slot: center/down/up/...> <part id> [<key>=<value> ...]
/// ```
pub fn encode<S: Stack>(table: &Machines<S>, codec: &dyn StackCodec<S>) -> String {
    let mut out = String::from("forja 1\n");
    for (p, m) in &table.map {
        out.push_str(&format!("m {} {} {} {}\n", p[0], p[1], p[2], m.kind));
        if let Some(e) = m.energy {
            out.push_str(&format!(
                "e {} {} {} {}\n",
                e.energy, e.capacity, e.max_receive, e.max_extract
            ));
        }
        for (i, t) in m.tanks.iter().enumerate() {
            let role = match m.tank_roles.get(i) {
                Some(TankRole::Input) => 'i',
                Some(TankRole::Output) => 'o',
                _ => 'b',
            };
            let fluid = if t.fluid.is_empty() { "-" } else { t.fluid.fluid.as_str() };
            let mut line = format!("t {} {} {} {}", t.capacity, role, t.amount(), fluid);
            for a in t.allowed.iter().flatten() {
                line.push(' ');
                line.push_str(a);
            }
            out.push_str(&line);
            out.push('\n');
        }
        if !m.items.slots.is_empty() {
            let roles: String = (0..m.items.slots.len())
                .map(|i| match m.slot_roles.get(i) {
                    Some(SlotRole::Input) => 'i',
                    Some(SlotRole::Output) => 'o',
                    Some(SlotRole::Internal) => 'n',
                    _ => 'b',
                })
                .collect();
            out.push_str(&format!("i {} {}\n", m.items.slots.len(), roles));
            for (i, s) in m.items.slots.iter().enumerate() {
                if let Some(s) = s {
                    out.push_str(&format!("s {} {}\n", i, codec.encode(s)));
                }
            }
        }
        if m.progress != 0 || m.max_progress != 0 {
            out.push_str(&format!("p {} {}\n", m.progress, m.max_progress));
        }
        out.push_str(&format!(
            "sides {} {} {}\n",
            m.energy_sides.code(),
            m.fluid_sides.code(),
            m.item_sides.code()
        ));
        for (k, v) in &m.data {
            out.push_str(&format!("d {k} {v}\n"));
        }
    }
    for (p, st) in &table.drops {
        out.push_str(&format!("x {} {} {} {}\n", p[0], p[1], p[2], codec.encode(st)));
    }
    out.push_str(&encode_hosts(&table.hosts));
    out
}

/// Reads `encode`'s text. Unreadable lines are skipped (a damaged file never stops the game);
/// a machine's items are kept even when its stack line is partly damaged.
pub fn decode<S: Stack>(text: &str, codec: &dyn StackCodec<S>) -> Machines<S> {
    let mut table = Machines::default();
    let mut cur: Option<Pos> = None;
    let mut host: Option<Pos> = None;
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| w.get(i).and_then(|s| s.parse::<i64>().ok());
        let int = |i: usize| num(i).and_then(|v| i32::try_from(v).ok());
        match w.first().copied() {
            Some("h") => {
                cur = None;
                host = None;
                if let (Some(x), Some(y), Some(z), Some(block)) = (int(1), int(2), int(3), w.get(4)) {
                    table.hosts.insert([x, y, z], Host::new(block));
                    host = Some([x, y, z]);
                }
            }
            Some("hp") => {
                let (Some(h), Some(slot), Some(id)) =
                    (host.and_then(|p| table.hosts.get_mut(&p)), w.get(1).and_then(|s| parse_slot(s)), w.get(2))
                else {
                    continue;
                };
                let mut part = Part::new(id);
                for kv in &w[3..] {
                    if let Some((k, v)) = kv.split_once('=')
                        && let Ok(v) = v.parse::<i64>()
                    {
                        part.set(k, v);
                    }
                }
                // A second line for the same slot (a damaged file) keeps the first part.
                let _ = h.put(slot, part);
            }
            Some("x") => {
                if let (Some(x), Some(y), Some(z), Some(t)) = (int(1), int(2), int(3), w.get(4))
                    && let Some(st) = codec.decode(t)
                {
                    table.drops.push(([x, y, z], st));
                }
            }
            Some("m") => {
                cur = None;
                host = None;
                if let (Some(x), Some(y), Some(z), Some(kind)) = (int(1), int(2), int(3), w.get(4)) {
                    let p = [x, y, z];
                    table.map.insert(
                        p,
                        Machine {
                            kind: (*kind).to_owned(),
                            energy: None,
                            tanks: Vec::new(),
                            tank_roles: Vec::new(),
                            items: Inventory { slots: Vec::new() },
                            slot_roles: Vec::new(),
                            energy_sides: Sides::default(),
                            fluid_sides: Sides::default(),
                            item_sides: Sides::default(),
                            progress: 0,
                            max_progress: 0,
                            data: BTreeMap::new(),
                            dirty: false,
                        },
                    );
                    cur = Some(p);
                }
            }
            Some(tag) => {
                let Some(m) = cur.and_then(|p| table.map.get_mut(&p)) else {
                    continue;
                };
                match tag {
                    "e" => {
                        if let (Some(e), Some(c), Some(r), Some(x)) = (int(1), int(2), int(3), int(4)) {
                            let mut s = EnergyStorage::with_rates(c, r, x);
                            s.energy = e.clamp(0, s.capacity);
                            m.energy = Some(s);
                        }
                    }
                    "t" => {
                        if let (Some(c), Some(role), Some(a), Some(f)) = (int(1), w.get(2), int(3), w.get(4)) {
                            let mut t = FluidTank::new(c);
                            if *f != "-" && a > 0 {
                                t.fluid = FluidStack::new(f, a.min(t.capacity));
                            }
                            if w.len() > 5 {
                                t.allowed = Some(w[5..].iter().map(|s| (*s).to_owned()).collect());
                            }
                            m.tanks.push(t);
                            m.tank_roles.push(match *role {
                                "i" => TankRole::Input,
                                "o" => TankRole::Output,
                                _ => TankRole::Both,
                            });
                        }
                    }
                    "i" => {
                        if let Some(n) = num(1).and_then(|n| usize::try_from(n).ok()).filter(|n| *n <= 4096) {
                            m.items.slots = vec![None; n.max(m.items.slots.len())];
                            let roles = w.get(2).copied().unwrap_or("");
                            m.slot_roles = (0..n)
                                .map(|i| match roles.as_bytes().get(i) {
                                    Some(b'i') => SlotRole::Input,
                                    Some(b'o') => SlotRole::Output,
                                    Some(b'n') => SlotRole::Internal,
                                    _ => SlotRole::Both,
                                })
                                .collect();
                        }
                    }
                    "s" => {
                        if let (Some(i), Some(t)) = (num(1).and_then(|n| usize::try_from(n).ok()), w.get(2))
                            && i <= 4096
                            && let Some(s) = codec.decode(t)
                        {
                            if m.items.slots.len() <= i {
                                // A slot line before (or without) its `i` line: keep the item.
                                m.items.slots.resize(i + 1, None);
                            }
                            m.items.slots[i] = Some(s);
                        }
                    }
                    "p" => {
                        if let (Some(a), Some(b)) = (int(1), int(2)) {
                            m.progress = a;
                            m.max_progress = b;
                        }
                    }
                    "sides" => {
                        if let (Some(e), Some(f), Some(i)) = (
                            w.get(1).and_then(|s| Sides::from_code(s)),
                            w.get(2).and_then(|s| Sides::from_code(s)),
                            w.get(3).and_then(|s| Sides::from_code(s)),
                        ) {
                            m.energy_sides = e;
                            m.fluid_sides = f;
                            m.item_sides = i;
                        }
                    }
                    "d" => {
                        if let (Some(k), Some(v)) = (w.get(1), num(2)) {
                            m.data.insert((*k).to_owned(), v);
                        }
                    }
                    _ => {}
                }
            }
            None => {}
        }
    }
    table
}

// ---- moving things between blocks (cables, machines pushing out) ----

/// The world as transport sees it: every capability of every block, whoever provides it
/// (forja's machine table, another mod's registered provider, vanilla containers). The game
/// implements it by dispatching; the tests on maps. Each call gives the handler to `f` for the
/// length of one operation only, so two handlers are never borrowed at once.
pub trait Grid {
    type Stack: Stack;
    /// The energy capability of the block at `p` seen from `face`; false when there is none.
    fn energy(&mut self, p: Pos, face: Option<Dir>, f: &mut dyn FnMut(&mut dyn EnergyHandler)) -> bool;
    fn fluid(&mut self, p: Pos, face: Option<Dir>, f: &mut dyn FnMut(&mut dyn FluidHandler)) -> bool;
    fn items(
        &mut self,
        p: Pos,
        face: Option<Dir>,
        f: &mut dyn FnMut(&mut dyn ItemHandler<Self::Stack>),
    ) -> bool;
}

/// `receiveEnergy` on the block at `p` through `face` (0 when it has no energy there).
pub fn energy_receive<G: Grid + ?Sized>(g: &mut G, p: Pos, face: Option<Dir>, max: i32, simulate: bool) -> i32 {
    let mut n = 0;
    g.energy(p, face, &mut |h| n = h.receive_energy(max, simulate));
    n
}
pub fn energy_extract<G: Grid + ?Sized>(g: &mut G, p: Pos, face: Option<Dir>, max: i32, simulate: bool) -> i32 {
    let mut n = 0;
    g.energy(p, face, &mut |h| n = h.extract_energy(max, simulate));
    n
}
pub fn energy_info<G: Grid + ?Sized>(g: &mut G, p: Pos, face: Option<Dir>) -> Option<EnergyInfo> {
    let mut info = None;
    g.energy(p, face, &mut |h| info = Some(EnergyInfo::of(h)));
    info
}
pub fn fluid_fill<G: Grid + ?Sized>(g: &mut G, p: Pos, face: Option<Dir>, s: &FluidStack, simulate: bool) -> i32 {
    let mut n = 0;
    g.fluid(p, face, &mut |h| n = h.fill(s, simulate));
    n
}
pub fn fluid_drain<G: Grid + ?Sized>(g: &mut G, p: Pos, face: Option<Dir>, s: &FluidStack, simulate: bool) -> FluidStack {
    let mut out = FluidStack::empty();
    g.fluid(p, face, &mut |h| out = h.drain(s, simulate));
    out
}
pub fn fluid_drain_max<G: Grid + ?Sized>(g: &mut G, p: Pos, face: Option<Dir>, max: i32, simulate: bool) -> FluidStack {
    let mut out = FluidStack::empty();
    g.fluid(p, face, &mut |h| out = h.drain_max(max, simulate));
    out
}
pub fn fluid_tanks<G: Grid + ?Sized>(g: &mut G, p: Pos, face: Option<Dir>) -> Option<Vec<TankInfo>> {
    let mut out = None;
    g.fluid(p, face, &mut |h| out = Some(tank_infos(h)));
    out
}

/// Energy from `from` (seen from its side `from.1`) to `to`: simulate, then move. Conserves
/// energy exactly when both handlers keep their simulations (refunds otherwise).
pub fn transfer_energy<G: Grid + ?Sized>(g: &mut G, from: (Pos, Option<Dir>), to: (Pos, Option<Dir>), max: i32) -> i32 {
    let offer = energy_extract(g, from.0, from.1, max, true);
    if offer <= 0 {
        return 0;
    }
    let accepted = energy_receive(g, to.0, to.1, offer, true);
    if accepted <= 0 {
        return 0;
    }
    let taken = energy_extract(g, from.0, from.1, accepted, false);
    let put = energy_receive(g, to.0, to.1, taken, false);
    if put < taken {
        let mut back = 0;
        g.energy(from.0, from.1, &mut |h| back = h.receive_energy(taken - put, false));
        let _ = back;
    }
    put
}

/// `FluidUtil.tryFluidTransfer` between two blocks: `resource = None` drains whatever is first.
pub fn transfer_fluid<G: Grid + ?Sized>(
    g: &mut G,
    from: (Pos, Option<Dir>),
    to: (Pos, Option<Dir>),
    resource: Option<&FluidStack>,
    max: i32,
) -> FluidStack {
    let offer = match resource {
        Some(r) => fluid_drain(g, from.0, from.1, &r.with_amount(r.amount.min(max)), true),
        None => fluid_drain_max(g, from.0, from.1, max, true),
    };
    if offer.is_empty() {
        return FluidStack::empty();
    }
    let fits = fluid_fill(g, to.0, to.1, &offer, true);
    if fits <= 0 {
        return FluidStack::empty();
    }
    let drained = fluid_drain(g, from.0, from.1, &offer.with_amount(fits), false);
    if drained.is_empty() {
        return FluidStack::empty();
    }
    let put = fluid_fill(g, to.0, to.1, &drained, false);
    if put < drained.amount {
        fluid_fill(g, from.0, from.1, &drained.with_amount(drained.amount - put), false);
    }
    drained.with_amount(put)
}

/// Items from one block to another (up to `max`). Returns the number moved and what could go
/// neither in nor back to its slot (only when a handler breaks its simulation): the caller drops
/// it in the world, so nothing is ever lost.
pub fn transfer_items<G: Grid + ?Sized>(
    g: &mut G,
    from: (Pos, Option<Dir>),
    to: (Pos, Option<Dir>),
    max: u32,
) -> (u32, Vec<G::Stack>) {
    let mut stray = Vec::new();
    let mut slots = 0;
    if !g.items(from.0, from.1, &mut |h| slots = h.slots()) {
        return (0, stray);
    }
    let mut moved = 0;
    for slot in 0..slots {
        if moved >= max {
            break;
        }
        let mut offer = None;
        g.items(from.0, from.1, &mut |h| offer = h.extract_item(slot, max - moved, true));
        let Some(offer) = offer else { continue };
        let offered = offer.count();
        let mut rest = Some(offer.clone());
        if !g.items(to.0, to.1, &mut |h| rest = insert_item_stacked(h, offer.clone(), true)) {
            break;
        }
        let fits = offered - rest.map_or(0, |r| r.count());
        if fits == 0 {
            continue;
        }
        let mut taken = None;
        g.items(from.0, from.1, &mut |h| taken = h.extract_item(slot, fits, false));
        let Some(taken) = taken else { continue };
        let got = taken.count();
        let mut back = Some(taken.clone());
        g.items(to.0, to.1, &mut |h| back = insert_item_stacked(h, taken.clone(), false));
        match back {
            None => moved += got,
            Some(back) => {
                moved += got - back.count();
                let mut lost = Some(back.clone());
                g.items(from.0, from.1, &mut |h| lost = h.insert_item(slot, back.clone(), false));
                stray.extend(lost);
            }
        }
    }
    (moved, stray)
}

/// The sides of `p` whose neighbour has an energy capability facing back.
pub fn energy_neighbours<G: Grid + ?Sized>(g: &mut G, p: Pos) -> Vec<Dir> {
    Dir::ALL
        .into_iter()
        .filter(|d| energy_info(g, offset(p, *d), Some(d.opposite())).is_some())
        .collect()
}

/// A generator or battery pushing out (`Powah`'s `EnergyUtil`, Thermal's `transferEnergy`):
/// up to `max` FE from the machine at `p` (through each side it may output on) into every
/// neighbour that can receive, split as evenly as the receivers allow, in `Dir` order.
/// Returns the total moved.
pub fn push_energy_around<G: Grid + ?Sized>(g: &mut G, p: Pos, max: i32) -> i32 {
    let mut targets = Vec::new();
    for d in Dir::ALL {
        let n = offset(p, d);
        let can_out = energy_info(g, p, Some(d)).is_some_and(|i| i.can_extract);
        if can_out && energy_receive(g, n, Some(d.opposite()), 1, true) > 0 {
            targets.push(d);
        }
    }
    let mut left = max;
    let mut moved = 0;
    let mut remaining = targets.len() as i32;
    for d in targets {
        if left <= 0 {
            break;
        }
        let share = (left / remaining.max(1)).max(1);
        let n = transfer_energy(g, (p, Some(d)), (offset(p, d), Some(d.opposite())), share.min(left));
        left -= n;
        moved += n;
        remaining -= 1;
    }
    moved
}

// ---- progress bars and gauges (AbstractContainerScreen.blit with a scaled width/height) ----

/// `value * len / max` in whole pixels (`getScaled`), 0 when `max <= 0`.
pub fn scaled(value: i32, max: i32, len: i32) -> i32 {
    if max <= 0 || value <= 0 || len <= 0 {
        return 0;
    }
    ((i64::from(value.min(max)) * i64::from(len)) / i64::from(max)) as i32
}

/// The way a bar grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grow {
    /// Left to right (a furnace arrow).
    Right,
    /// Bottom to top (an energy gauge, a furnace flame).
    Up,
    Left,
    Down,
}

/// A bar on a machine screen: where it is drawn (`x`, `y` from the screen's corner) and where the
/// full picture is in the screen texture (`u`, `v`, `w`, `h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    pub x: i32,
    pub y: i32,
    pub u: i32,
    pub v: i32,
    pub w: i32,
    pub h: i32,
    pub grow: Grow,
}
impl Bar {
    /// The part to draw for `value / max`: (destination x, y, texture u, v, width, height), or
    /// `None` when nothing shows.
    pub fn part(&self, value: i32, max: i32) -> Option<[i32; 6]> {
        let len = match self.grow {
            Grow::Right | Grow::Left => self.w,
            Grow::Up | Grow::Down => self.h,
        };
        let n = scaled(value, max, len);
        if n <= 0 {
            return None;
        }
        Some(match self.grow {
            Grow::Right => [self.x, self.y, self.u, self.v, n, self.h],
            Grow::Left => [self.x + self.w - n, self.y, self.u + self.w - n, self.v, n, self.h],
            Grow::Down => [self.x, self.y, self.u, self.v, self.w, n],
            Grow::Up => [self.x, self.y + self.h - n, self.u, self.v + self.h - n, self.w, n],
        })
    }
}

/// Energy as Forge mods print it (`1,234 / 10,000 FE`).
pub fn fe_text(stored: i32, capacity: i32) -> String {
    format!("{} / {} FE", group(stored), group(capacity))
}
/// A fluid amount as `1,000 mB`.
pub fn mb_text(amount: i32) -> String {
    format!("{} mB", group(amount))
}
fn group(n: i32) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

// ---- compound blocks (multipart): one host block per position holding a centre part and up to
// six face parts (AE2's cable bus: the cable in the centre, terminals and buses on the faces) ----

/// Where a part sits in its host: `None` = the centre (AE2 `getPart(null)`), `Some(d)` = face `d`.
pub type Slot = Option<Dir>;
/// AE2 `Platform.DIRECTIONS_WITH_NULL`: the order parts are looked at (faces first, the centre last).
pub const SLOTS: [Slot; 7] = [
    Some(Dir::Down),
    Some(Dir::Up),
    Some(Dir::North),
    Some(Dir::South),
    Some(Dir::West),
    Some(Dir::East),
    None,
];
pub fn slot_name(s: Slot) -> &'static str {
    s.map_or("center", Dir::name)
}
pub fn parse_slot(s: &str) -> Option<Slot> {
    if s == "center" { Some(None) } else { Dir::parse(s).map(Some) }
}

/// `data` key of a part that gives no item back (a centre made by a migration, not by an item).
pub const NO_DROP: &str = "forja:nodrop";

/// A part placed in a host: its id (`ae2:fluix_glass_cable`) and small numbers its mod keeps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Part {
    pub id: String,
    pub data: BTreeMap<String, i64>,
}
impl Part {
    pub fn new(id: &str) -> Self {
        Part { id: id.to_owned(), data: BTreeMap::new() }
    }
    pub fn get(&self, k: &str) -> i64 {
        self.data.get(k).copied().unwrap_or(0)
    }
    /// 0 removes the key (the side file keeps only what is set).
    pub fn set(&mut self, k: &str, v: i64) {
        if v == 0 {
            self.data.remove(k);
        } else {
            self.data.insert(k.to_owned(), v);
        }
    }
    pub fn drops(&self) -> bool {
        self.get(NO_DROP) == 0
    }
}

/// The parts of one compound block (`CableBusContainer`'s storage).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Host {
    /// The host block standing at the position (`ae2:cable_bus`).
    pub block: String,
    pub center: Option<Part>,
    pub faces: [Option<Part>; 6],
}
impl Host {
    pub fn new(block: &str) -> Self {
        Host { block: block.to_owned(), ..Default::default() }
    }
    pub fn get(&self, s: Slot) -> Option<&Part> {
        match s {
            None => self.center.as_ref(),
            Some(d) => self.faces[d.idx()].as_ref(),
        }
    }
    pub fn get_mut(&mut self, s: Slot) -> Option<&mut Part> {
        self.slot_mut(s).as_mut()
    }
    fn slot_mut(&mut self, s: Slot) -> &mut Option<Part> {
        match s {
            None => &mut self.center,
            Some(d) => &mut self.faces[d.idx()],
        }
    }
    pub fn is_empty(&self) -> bool {
        self.center.is_none() && self.faces.iter().all(Option::is_none)
    }
    /// Every part with its slot, faces first (DOWN..EAST) then the centre.
    pub fn parts(&self) -> Vec<(Slot, &Part)> {
        SLOTS.iter().filter_map(|&s| self.get(s).map(|p| (s, p))).collect()
    }
    /// A face part sits on `d`: the centre does not reach out there (`CableBusContainer.updateConnections`).
    pub fn blocked(&self, d: Dir) -> bool {
        self.faces[d.idx()].is_some()
    }
    /// Puts `part` in a free slot; an occupied slot gives the part back.
    pub fn put(&mut self, s: Slot, part: Part) -> Result<(), Part> {
        let slot = self.slot_mut(s);
        if slot.is_some() {
            return Err(part);
        }
        *slot = Some(part);
        Ok(())
    }
    /// Takes the part at `s` out (`removePartFromSide`). When only parts without an item are left
    /// (a migration's centre), they go too: the host is then empty and disappears.
    pub fn remove(&mut self, s: Slot) -> Vec<(Slot, Part)> {
        let Some(p) = self.slot_mut(s).take() else {
            return Vec::new();
        };
        let mut out = vec![(s, p)];
        if self.parts().iter().all(|(_, p)| !p.drops()) {
            out.extend(self.take_all());
        }
        out
    }
    /// Every part out (the host block was broken or is gone).
    pub fn take_all(&mut self) -> Vec<(Slot, Part)> {
        let mut out = Vec::new();
        for s in SLOTS {
            if let Some(p) = self.slot_mut(s).take() {
                out.push((s, p));
            }
        }
        out
    }
}

/// The shape of a part.
#[derive(Clone, Debug, PartialEq)]
pub enum PartShape {
    /// AE2 `CablePart.addNonDenseBoxes` / `DenseCablePart.getBoxes`: a core cube from `min` to
    /// `max` on every axis (1/16) and an arm of the same section towards every linked side;
    /// `collide` replaces min/max for collisions (dense cables: 4.9..11.1 instead of 3..13);
    /// `attach_arms`: an arm also reaches the host's face parts up to their `cable_length`.
    Cable { min: f64, max: f64, collide: Option<(f64, f64)>, attach_arms: bool },
    /// Boxes (1/16) of a part sitting on the SOUTH face (AE2 `IPart.getBoxes` frame); forja turns
    /// them to the part's face like `BusCollisionHelper`. A centre part with boxes uses them as is.
    Boxes(Vec<[f64; 6]>),
}

/// A kind of part, registered by the mod that owns it.
#[derive(Clone, Debug, PartialEq)]
pub struct PartSpec {
    pub id: String,
    /// The host block it goes in (one per mod: `ae2:cable_bus`).
    pub host: String,
    /// The item that places it and that it drops.
    pub item: String,
    /// A centre part (a cable) or a face part.
    pub center: bool,
    pub shape: PartShape,
    /// Face parts: `getCableConnectionLength`, how far (1/16 from the face) the centre's arm reaches
    /// into the part; `<= 0` or `> 8`: no arm.
    pub cable_length: f64,
    /// Bus support (AE2 `BusSupport`): a centre's group bit (1 = cable, 2 = dense cable); a face
    /// part's accepted groups (most parts 1, the cable anchor 3).
    pub group: u8,
    /// The block whose states draw the part: a centre gets the six direction booleans (its arms),
    /// a face part `facing` = its side (or the opposite side with `facing_opposite`).
    pub model: String,
    pub facing_opposite: bool,
}
impl PartSpec {
    /// A centre part (cable) of `host`, drawn by `model`'s direction booleans.
    pub fn center(id: &str, host: &str, model: &str, shape: PartShape) -> Self {
        PartSpec {
            id: id.to_owned(),
            host: host.to_owned(),
            item: id.to_owned(),
            center: true,
            shape,
            cable_length: 0.0,
            group: 1,
            model: model.to_owned(),
            facing_opposite: false,
        }
    }
    /// A face part of `host` with its boxes on the SOUTH face, drawn by `model`'s `facing`.
    pub fn face(id: &str, host: &str, model: &str, boxes: &[[f64; 6]]) -> Self {
        PartSpec {
            id: id.to_owned(),
            host: host.to_owned(),
            item: id.to_owned(),
            center: false,
            shape: PartShape::Boxes(boxes.to_vec()),
            cable_length: 3.0,
            group: 1,
            model: model.to_owned(),
            facing_opposite: false,
        }
    }
    pub fn item(mut self, item: &str) -> Self {
        self.item = item.to_owned();
        self
    }
    pub fn cable_length(mut self, len: f64) -> Self {
        self.cable_length = len;
        self
    }
    pub fn group(mut self, group: u8) -> Self {
        self.group = group;
        self
    }
    pub fn facing_opposite(mut self) -> Self {
        self.facing_opposite = true;
        self
    }
}

/// `BusCollisionHelper.addBox`: a box given in 1/16 for a part on the SOUTH face, turned to `side`
/// (block units, min <= max).
pub fn face_box(b: [f64; 6], side: Dir) -> [f64; 6] {
    let (x, y, z) = match side {
        Dir::Down => (Dir::East, Dir::North, Dir::Down),
        Dir::Up => (Dir::East, Dir::South, Dir::Up),
        Dir::East => (Dir::South, Dir::Up, Dir::East),
        Dir::West => (Dir::North, Dir::Up, Dir::West),
        Dir::North => (Dir::West, Dir::Up, Dir::North),
        Dir::South => (Dir::East, Dir::Up, Dir::South),
    };
    let (sx, sy, sz) = (x.step(), y.step(), z.step());
    let lo = [b[0] / 16.0, b[1] / 16.0, b[2] / 16.0];
    let hi = [b[3] / 16.0, b[4] / 16.0, b[5] / 16.0];
    let turn = |v: [f64; 3]| -> [f64; 3] {
        std::array::from_fn(|k| v[0] * f64::from(sx[k]) + v[1] * f64::from(sy[k]) + v[2] * f64::from(sz[k]))
    };
    let (mut a, mut c) = (turn(lo), turn(hi));
    for k in 0..3 {
        if sx[k] + sy[k] + sz[k] < 0 {
            a[k] += 1.0;
            c[k] += 1.0;
        }
    }
    [a[0].min(c[0]), a[1].min(c[1]), a[2].min(c[2]), a[0].max(c[0]), a[1].max(c[1]), a[2].max(c[2])]
}

/// `CablePart.addConnectionBox` (1/16): the arm towards `d`, from `dist` away from that face to the
/// core `min..max`.
pub fn arm_box(d: Dir, min: f64, max: f64, dist: f64) -> [f64; 6] {
    match d {
        Dir::Down => [min, dist, min, max, min, max],
        Dir::East => [max, min, min, 16.0 - dist, max, max],
        Dir::North => [min, min, dist, max, max, min],
        Dir::South => [min, min, max, max, max, 16.0 - dist],
        Dir::Up => [min, max, min, max, 16.0 - dist, max],
        Dir::West => [dist, min, min, min, max, max],
    }
}
fn sixteenth(b: [f64; 6]) -> [f64; 6] {
    b.map(|v| v / 16.0)
}

/// What the centre reaches on each side: `Some(0)` = a linked neighbour (the arm goes to the
/// block's face), `Some(len)` = a face part of the host whose `cable_length` is in (0, 8].
pub fn center_arms(host: &Host, spec_of: &dyn Fn(&str) -> Option<PartSpec>, links: [bool; 6]) -> [Option<f64>; 6] {
    let attach = host
        .center
        .as_ref()
        .and_then(|c| spec_of(&c.id))
        .is_none_or(|s| matches!(s.shape, PartShape::Cable { attach_arms: true, .. }));
    std::array::from_fn(|i| {
        if let Some(p) = &host.faces[i] {
            if !attach {
                return None;
            }
            let len = spec_of(&p.id).map_or(3.0, |s| s.cable_length);
            return (len > 0.0 && len <= 8.0).then_some(len);
        }
        links[i].then_some(0.0)
    })
}

/// The boxes of one part in block units (`IPart.getBoxes` through `BusCollisionHelper`).
/// `collision`: the boxes entities collide with (dense cables are thinner there).
pub fn part_boxes(spec: &PartSpec, slot: Slot, arms: [Option<f64>; 6], collision: bool) -> Vec<[f64; 6]> {
    match &spec.shape {
        PartShape::Cable { min, max, collide, .. } => {
            let (lo, hi) = if collision { collide.unwrap_or((*min, *max)) } else { (*min, *max) };
            let mut out = vec![sixteenth([lo, lo, lo, hi, hi, hi])];
            for d in Dir::ALL {
                if let Some(dist) = arms[d.idx()] {
                    out.push(sixteenth(arm_box(d, lo, hi, dist)));
                }
            }
            out
        }
        PartShape::Boxes(boxes) => match slot {
            Some(side) => boxes.iter().map(|b| face_box(*b, side)).collect(),
            None => boxes.iter().map(|b| sixteenth(*b)).collect(),
        },
    }
}

/// A part whose kind is not registered (its mod was removed): a small cube in its slot, so it can
/// still be hit and taken out (its item is not lost).
pub fn unknown_boxes(slot: Slot) -> Vec<[f64; 6]> {
    match slot {
        None => vec![[0.25, 0.25, 0.25, 0.75, 0.75, 0.75]],
        Some(side) => vec![face_box([4.0, 4.0, 12.0, 12.0, 12.0, 16.0], side)],
    }
}

/// Every part's boxes in the host, in AE2's order (faces, then the centre): what the outline, the
/// crosshair and (with `collision`) entities use. `links` = the centre's links (see `links`).
pub fn host_boxes(host: &Host, spec_of: &dyn Fn(&str) -> Option<PartSpec>, links: [bool; 6], collision: bool) -> Vec<(Slot, [f64; 6])> {
    let arms = center_arms(host, spec_of, links);
    let mut out = Vec::new();
    for (slot, part) in host.parts() {
        let boxes = match spec_of(&part.id) {
            Some(spec) => part_boxes(&spec, slot, arms, collision),
            None => unknown_boxes(slot),
        };
        out.extend(boxes.into_iter().map(|b| (slot, b)));
    }
    out
}

fn contains(b: &[f64; 6], p: [f64; 3], grow: f64) -> bool {
    // AABB.contains: min inclusive, max exclusive.
    (0..3).all(|k| p[k] >= b[k] - grow && p[k] < b[k + 3] + grow)
}

/// `CableBusContainer.selectPartLocal`: the part whose box (grown by 0.002) holds the point
/// (block units, relative to the host), faces first.
pub fn select(boxes: &[(Slot, [f64; 6])], local: [f64; 3]) -> Option<Slot> {
    SLOTS
        .into_iter()
        .find(|&s| boxes.iter().any(|(bs, b)| *bs == s && contains(b, local, 0.002)))
}

/// `PartPlacement.tryReplaceCableSegment`: the side whose arm of the centre (grown by 0.02) holds
/// the point; `None` on the core or with no centre.
pub fn arm_at(host: &Host, spec_of: &dyn Fn(&str) -> Option<PartSpec>, links: [bool; 6], local: [f64; 3]) -> Option<Dir> {
    let spec = spec_of(&host.center.as_ref()?.id)?;
    let PartShape::Cable { min, max, .. } = spec.shape else {
        return None;
    };
    let arms = center_arms(host, spec_of, links);
    Dir::ALL.into_iter().find(|d| {
        arms[d.idx()].is_some_and(|dist| contains(&sixteenth(arm_box(*d, min, max, dist)), local, 0.02))
    })
}

/// The first of `boxes` (world space) the ray `origin + t * dir` enters within `reach`: the slab
/// test of Horizon's `clip_ray` (`BlockGetter.clip` against an outline). Gives t, the box and its
/// face normal; a ray starting inside a box hits it at once with `face` (the cell's entry face).
pub fn ray_boxes(origin: [f64; 3], dir: [f64; 3], reach: f64, boxes: &[[f64; 6]], face: [i32; 3]) -> Option<(f64, usize, [i32; 3])> {
    let mut best: Option<(f64, usize, [i32; 3])> = None;
    for (i, b) in boxes.iter().enumerate() {
        let (mut t0, mut t1, mut axis_in, mut sign) = (0.0f64, f64::INFINITY, None, 0);
        let mut miss = false;
        for a in 0..3 {
            if dir[a] == 0.0 {
                miss |= origin[a] < b[a] || origin[a] > b[a + 3];
                continue;
            }
            let (lo, hi) = ((b[a] - origin[a]) / dir[a], (b[a + 3] - origin[a]) / dir[a]);
            let (near, far) = if lo < hi { (lo, hi) } else { (hi, lo) };
            if near > t0 {
                t0 = near;
                axis_in = Some(a);
                sign = if dir[a] > 0.0 { -1 } else { 1 };
            }
            t1 = t1.min(far);
        }
        if miss || t0 > t1 || t0 > reach {
            continue;
        }
        let mut normal = face;
        if let Some(a) = axis_in {
            normal = [0; 3];
            normal[a] = sign;
        }
        if best.is_none_or(|(bt, _, _)| t0 < bt) {
            best = Some((t0, i, normal));
        }
    }
    best
}

/// `CableBusContainer.canAddPart`: the slot is free and the part works with the host's centre
/// (a cable accepts the face parts already there, a face part needs a fitting cable or none).
pub fn can_add(host: &Host, spec: &PartSpec, slot: Slot, spec_of: &dyn Fn(&str) -> Option<PartSpec>) -> bool {
    if host.block != spec.host || spec.center != slot.is_none() || host.get(slot).is_some() {
        return false;
    }
    if spec.center {
        host.faces
            .iter()
            .flatten()
            .all(|p| spec_of(&p.id).is_none_or(|f| f.group & spec.group != 0))
    } else {
        host.center
            .as_ref()
            .and_then(|c| spec_of(&c.id))
            .is_none_or(|c| c.group & spec.group != 0)
    }
}

/// Where a placed part goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// In the clicked block's position (a host there, or a replaceable block).
    Clicked,
    /// In the position beside the clicked face (like a torch).
    Beside,
}

/// `PartPlacement.getPartPlacement`: where a part item clicked on `face` goes. `clicked` / `beside`
/// = the host at the clicked position / beside it (if any) and whether a new host may stand there
/// (`canPlacePartHost`: the block is replaceable); `arm` = the clicked arm of the clicked host's
/// cable (`arm_at`).
#[allow(clippy::too_many_arguments)]
pub fn placement(
    spec: &PartSpec,
    spec_of: &dyn Fn(&str) -> Option<PartSpec>,
    clicked: Option<&Host>,
    clicked_free: bool,
    arm: Option<Dir>,
    face: Dir,
    beside: Option<&Host>,
    beside_free: bool,
) -> Option<(Target, Slot)> {
    // tryReplaceCableSegment: a part clicked on a cable's arm goes on that side.
    if let (Some(h), Some(side)) = (clicked, arm)
        && !spec.center
        && can_add(h, spec, Some(side), spec_of)
    {
        return Some((Target::Clicked, Some(side)));
    }
    let fits = |host: Option<&Host>, free: bool, slot: Slot| match host {
        Some(h) => can_add(h, spec, slot, spec_of),
        None => free,
    };
    let here = if spec.center { None } else { Some(face) };
    if fits(clicked, clicked_free, here) {
        return Some((Target::Clicked, here));
    }
    let there = if spec.center { None } else { Some(face.opposite()) };
    if fits(beside, beside_free, there) {
        return Some((Target::Beside, there));
    }
    None
}

/// The sides the centre reaches out of its host: no face part there and `joins(d)` (the neighbour
/// takes the connection: another host's centre of a fitting kind, a block of the mod's network).
pub fn links(host: &Host, joins: impl Fn(Dir) -> bool) -> [bool; 6] {
    std::array::from_fn(|i| {
        let d = Dir::ALL[i];
        host.center.is_some() && !host.blocked(d) && joins(d)
    })
}

/// The block state (name, properties) that draws a part.
pub fn look(spec: &PartSpec, slot: Slot, arms: [Option<f64>; 6]) -> (String, Vec<(String, String)>) {
    let props = match slot {
        Some(side) if !spec.center => {
            let f = if spec.facing_opposite { side.opposite() } else { side };
            vec![("facing".to_owned(), f.name().to_owned())]
        }
        _ => Dir::ALL
            .iter()
            .map(|d| (d.name().to_owned(), if arms[d.idx()].is_some() { "true" } else { "false" }.to_owned()))
            .collect(),
    };
    (spec.model.clone(), props)
}

/// The items a host's removed parts give back (`addPartDrops`): one item per part, none for parts
/// made without an item (`NO_DROP`). Unknown kinds give their id (the item usually has that name).
pub fn part_drops(parts: &[(Slot, Part)], spec_of: &dyn Fn(&str) -> Option<PartSpec>) -> Vec<String> {
    parts
        .iter()
        .filter(|(_, p)| p.drops())
        .map(|(_, p)| spec_of(&p.id).map_or_else(|| p.id.clone(), |s| s.item))
        .collect()
}

/// The hosts' lines of the side file (`encode`).
pub fn encode_hosts(hosts: &BTreeMap<Pos, Host>) -> String {
    let mut out = String::new();
    for (p, h) in hosts {
        if h.is_empty() {
            continue;
        }
        out.push_str(&format!("h {} {} {} {}\n", p[0], p[1], p[2], h.block));
        for (s, part) in h.parts() {
            let mut line = format!("hp {} {}", slot_name(s), part.id);
            for (k, v) in &part.data {
                line.push_str(&format!(" {k}={v}"));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

// ---- shapes from models ("forme din model"): a content-mod block whose model is not the shape
// of its vanilla template gets its model's boxes as outline (and collision), like a Java mod
// whose VoxelShape follows its model ----

/// The integer rotation of a blockstate variant's quarter turns (x, y, z), exactly as Horizon's
/// `BlockModelTransform` builds it (vanilla `BlockModelRotation`: negative quarter turns, X first).
pub fn turn_rows(turns: [u8; 3]) -> [[i8; 3]; 3] {
    let mut rows = [[1i8, 0, 0], [0, 1, 0], [0, 0, 1]];
    for (axis, n) in turns.into_iter().enumerate() {
        for _ in 0..(n % 4) {
            let a = (axis + 1) % 3;
            let b = (axis + 2) % 3;
            let old = rows[a];
            rows[a] = rows[b];
            rows[b] = old.map(|v| -v);
        }
    }
    rows
}

/// An element box (`from`/`to` in 1/16) turned by a variant (block units, around the centre).
pub fn turn_box(b: [f64; 6], turns: [u8; 3]) -> [f64; 6] {
    let rows = turn_rows(turns);
    let at = |v: [f64; 3]| -> [f64; 3] {
        let c = [v[0] / 16.0 - 0.5, v[1] / 16.0 - 0.5, v[2] / 16.0 - 0.5];
        std::array::from_fn(|i| {
            f64::from(rows[i][0]) * c[0] + f64::from(rows[i][1]) * c[1] + f64::from(rows[i][2]) * c[2] + 0.5
        })
    };
    let (a, c) = (at([b[0], b[1], b[2]]), at([b[3], b[4], b[5]]));
    [a[0].min(c[0]), a[1].min(c[1]), a[2].min(c[2]), a[0].max(c[0]), a[1].max(c[1]), a[2].max(c[2])]
}

/// The thinnest a box may be (1/16): a flat element (a grate's plane, a decal) still gets a
/// pickable, standable slab.
pub const MIN_THICKNESS: f64 = 1.0 / 16.0;

/// The model's boxes of a block state: every element of every selected variant (`from`/`to`, 1/16)
/// turned by its variant, flat ones thickened to 1/16 towards the block's inside, everything
/// clamped to the block (shapes of one cell). Element rotations (22.5 / 45 degrees) are ignored, as
/// most Java mods' VoxelShapes do.
pub fn model_boxes(variants: &[(Vec<[f64; 6]>, [u8; 3])]) -> Vec<[f64; 6]> {
    let mut out = Vec::new();
    for (elements, turns) in variants {
        for e in elements {
            let mut b = turn_box(*e, *turns);
            for k in 0..3 {
                b[k] = b[k].clamp(0.0, 1.0);
                b[k + 3] = b[k + 3].clamp(0.0, 1.0);
                if b[k + 3] - b[k] < MIN_THICKNESS {
                    if b[k] < 0.5 {
                        b[k + 3] = b[k] + MIN_THICKNESS;
                    } else {
                        b[k] = b[k + 3] - MIN_THICKNESS;
                    }
                }
            }
            if !out.contains(&b) {
                out.push(b);
            }
        }
    }
    out
}

fn bounds(boxes: &[[f64; 6]]) -> Option<[f64; 6]> {
    let mut it = boxes.iter();
    let first = *it.next()?;
    Some(it.fold(first, |a, b| {
        [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2]), a[3].max(b[3]), a[4].max(b[4]), a[5].max(b[5])]
    }))
}

/// Whether two box lists cover the same volume (checked on a 1/32 grid of cell centres, finer than
/// model coordinates usually go; equal bounds of single boxes are compared directly).
pub fn same_shape(a: &[[f64; 6]], b: &[[f64; 6]]) -> bool {
    const EPS: f64 = 1e-6;
    let (Some(ba), Some(bb)) = (bounds(a), bounds(b)) else {
        return a.is_empty() && b.is_empty();
    };
    if (0..6).any(|k| (ba[k] - bb[k]).abs() > EPS) {
        return false;
    }
    if a.len() == 1 && b.len() == 1 {
        return true;
    }
    let inside = |boxes: &[[f64; 6]], p: [f64; 3]| boxes.iter().any(|b| (0..3).all(|k| p[k] > b[k] && p[k] < b[k + 3]));
    const N: usize = 32;
    for x in 0..N {
        for y in 0..N {
            for z in 0..N {
                let p = [(x as f64 + 0.5) / N as f64, (y as f64 + 0.5) / N as f64, (z as f64 + 0.5) / N as f64];
                if inside(a, p) != inside(b, p) {
                    return false;
                }
            }
        }
    }
    true
}

/// Template classes whose shape is part of what the block does (doors and gates open, slabs stack,
/// stairs, fences, walls and panes connect, beds are two halves, scaffolding is climbed): such
/// blocks keep the template's shapes.
pub const KEEP_TEMPLATE_CLASSES: [&str; 10] = [
    "DoorBlock",
    "TrapDoorBlock",
    "FenceGateBlock",
    "FenceBlock",
    "WallBlock",
    "IronBarsBlock",
    "SlabBlock",
    "StairBlock",
    "BedBlock",
    "ScaffoldingBlock",
];

/// What a model shape changes for one block state.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelShape {
    /// The outline (crosshair, selection box): the model's boxes.
    pub outline: Vec<[f64; 6]>,
    /// Entities collide with the model's boxes too (otherwise the template's collision stays).
    pub collide: bool,
}

/// The rule of "forme din model" for one block state. `None` = the template's shapes stay:
/// vanilla blocks, models without elements (`builtin/entity`: `model` empty), models that are the
/// template's outline anyway, templates whose shape is behaviour (`KEEP_TEMPLATE_CLASSES`). The
/// collision follows the model only when the template's collision is its plain outline (not empty
/// like a plant's, not dynamic, not taller like a fence's, not lower like soul sand's).
pub fn model_shape_rule(
    name: &str,
    model: &[[f64; 6]],
    template_outline: &[[f64; 6]],
    template_collision: Option<&[[f64; 6]]>,
    classes: &[&str],
) -> Option<ModelShape> {
    if name.starts_with("minecraft:") || !name.contains(':') || model.is_empty() {
        return None;
    }
    if classes.iter().any(|c| KEEP_TEMPLATE_CLASSES.contains(c)) {
        return None;
    }
    if same_shape(model, template_outline) {
        return None;
    }
    let collide = template_collision.is_some_and(|c| !c.is_empty() && same_shape(c, template_outline));
    Some(ModelShape { outline: model.to_vec(), collide })
}

// ---- mod networking ("Rețea mod"): a typed channel between a mod's server half and its client
// half, over Horizon's own connection (custom payload packets on `forja:net`, as vanilla's
// `ServerboundCustomPayloadPacket` / `ClientboundCustomPayloadPacket`; a vanilla client or server
// ignores an unknown channel). The frames and the message codec are here; the game side sends and
// receives them (native/horizon.rs).

/// The custom payload channel every forja frame travels on, both ways.
pub const NET_CHANNEL: &str = "forja:net";
/// The frame format's version (first byte of every frame).
pub const NET_VERSION: u8 = 1;
/// Bytes of message data in one frame at most: a serverbound payload may hold 32 767 bytes
/// (`ServerboundCustomPayloadPacket.MAX_PAYLOAD_SIZE`), the header and the channel name fit in
/// the rest.
pub const FRAME_DATA: usize = 30_000;
/// The longest channel name.
pub const MAX_CHANNEL: usize = 128;
/// A message longer than this is refused (8 MiB: an AE2 terminal with every item of a big
/// network is a few hundred KiB).
pub const MAX_MESSAGE: usize = 8 << 20;
/// Unfinished messages kept per connection at most.
pub const MAX_PENDING: usize = 8;

/// A compact writer: LEB128 varints, zigzag for signed numbers, length-prefixed bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Writer {
    pub buf: Vec<u8>,
}
impl Writer {
    pub fn new() -> Self {
        Writer::default()
    }
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }
    pub fn uvar(&mut self, mut v: u64) -> &mut Self {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(b);
                return self;
            }
            self.buf.push(b | 0x80);
        }
    }
    pub fn ivar(&mut self, v: i64) -> &mut Self {
        self.uvar(((v << 1) ^ (v >> 63)) as u64)
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.ivar(i64::from(v))
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.uvar(u64::from(v))
    }
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_bits().to_be_bytes());
        self
    }
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_bits().to_be_bytes());
        self
    }
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.uvar(b.len() as u64);
        self.buf.extend_from_slice(b);
        self
    }
    pub fn str(&mut self, s: &str) -> &mut Self {
        self.bytes(s.as_bytes())
    }
    pub fn put<T: Wire>(&mut self, v: &T) -> &mut Self {
        v.put(self);
        self
    }
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// The reader of what [`Writer`] wrote; every read is `None` past the end or on a bad value
/// (never a panic: the bytes come from another machine).
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, at: 0 }
    }
    pub fn done(&self) -> bool {
        self.at == self.data.len()
    }
    pub fn rest(&self) -> &'a [u8] {
        &self.data[self.at..]
    }
    pub fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.at)?;
        self.at += 1;
        Some(b)
    }
    pub fn bool(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    pub fn uvar(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for i in 0..10 {
            let b = self.u8()?;
            v |= u64::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    pub fn ivar(&mut self) -> Option<i64> {
        let u = self.uvar()?;
        Some(((u >> 1) as i64) ^ -((u & 1) as i64))
    }
    pub fn i32(&mut self) -> Option<i32> {
        i32::try_from(self.ivar()?).ok()
    }
    pub fn u32(&mut self) -> Option<u32> {
        u32::try_from(self.uvar()?).ok()
    }
    pub fn f32(&mut self) -> Option<f32> {
        let b = self.data.get(self.at..self.at + 4)?;
        self.at += 4;
        Some(f32::from_bits(u32::from_be_bytes([b[0], b[1], b[2], b[3]])))
    }
    pub fn f64(&mut self) -> Option<f64> {
        let b = self.data.get(self.at..self.at + 8)?;
        self.at += 8;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Some(f64::from_bits(u64::from_be_bytes(a)))
    }
    pub fn bytes(&mut self) -> Option<&'a [u8]> {
        let n = usize::try_from(self.uvar()?).ok()?;
        if n > self.data.len() - self.at {
            return None;
        }
        let b = &self.data[self.at..self.at + n];
        self.at += n;
        Some(b)
    }
    pub fn str(&mut self) -> Option<&'a str> {
        std::str::from_utf8(self.bytes()?).ok()
    }
    pub fn get<T: Wire>(&mut self) -> Option<T> {
        T::get(self)
    }
}

/// A value the channel can carry.
pub trait Wire: Sized {
    fn put(&self, w: &mut Writer);
    fn get(r: &mut Reader<'_>) -> Option<Self>;
}
impl Wire for u8 {
    fn put(&self, w: &mut Writer) {
        w.u8(*self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.u8()
    }
}
impl Wire for bool {
    fn put(&self, w: &mut Writer) {
        w.bool(*self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.bool()
    }
}
impl Wire for u32 {
    fn put(&self, w: &mut Writer) {
        w.u32(*self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.u32()
    }
}
impl Wire for i32 {
    fn put(&self, w: &mut Writer) {
        w.i32(*self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.i32()
    }
}
impl Wire for u64 {
    fn put(&self, w: &mut Writer) {
        w.uvar(*self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.uvar()
    }
}
impl Wire for i64 {
    fn put(&self, w: &mut Writer) {
        w.ivar(*self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.ivar()
    }
}
impl Wire for usize {
    fn put(&self, w: &mut Writer) {
        w.uvar(*self as u64);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        usize::try_from(r.uvar()?).ok()
    }
}
impl Wire for f32 {
    fn put(&self, w: &mut Writer) {
        w.f32(*self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.f32()
    }
}
impl Wire for f64 {
    fn put(&self, w: &mut Writer) {
        w.f64(*self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.f64()
    }
}
impl Wire for String {
    fn put(&self, w: &mut Writer) {
        w.str(self);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        r.str().map(str::to_owned)
    }
}
impl<T: Wire> Wire for Option<T> {
    fn put(&self, w: &mut Writer) {
        match self {
            None => {
                w.u8(0);
            }
            Some(v) => {
                w.u8(1);
                v.put(w);
            }
        }
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        match r.u8()? {
            0 => Some(None),
            1 => Some(Some(T::get(r)?)),
            _ => None,
        }
    }
}
impl<T: Wire> Wire for Vec<T> {
    fn put(&self, w: &mut Writer) {
        w.uvar(self.len() as u64);
        for v in self {
            v.put(w);
        }
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        let n = usize::try_from(r.uvar()?).ok()?;
        // Every element takes at least one byte: a length past the rest is a lie.
        if n > r.rest().len() {
            return None;
        }
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(T::get(r)?);
        }
        Some(v)
    }
}
impl<A: Wire, B: Wire> Wire for (A, B) {
    fn put(&self, w: &mut Writer) {
        self.0.put(w);
        self.1.put(w);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        Some((A::get(r)?, B::get(r)?))
    }
}
impl<A: Wire, B: Wire, C: Wire> Wire for (A, B, C) {
    fn put(&self, w: &mut Writer) {
        self.0.put(w);
        self.1.put(w);
        self.2.put(w);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        Some((A::get(r)?, B::get(r)?, C::get(r)?))
    }
}
impl Wire for [i32; 3] {
    fn put(&self, w: &mut Writer) {
        for v in self {
            w.i32(*v);
        }
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        Some([r.i32()?, r.i32()?, r.i32()?])
    }
}

/// `value` as the bytes of one message.
pub fn to_bytes<T: Wire>(value: &T) -> Vec<u8> {
    let mut w = Writer::new();
    value.put(&mut w);
    w.finish()
}
/// The message in `bytes`: `None` when it is not a whole `T` (trailing bytes included).
pub fn from_bytes<T: Wire>(bytes: &[u8]) -> Option<T> {
    let mut r = Reader::new(bytes);
    let v = T::get(&mut r)?;
    r.done().then_some(v)
}

/// A typed channel: the id both halves agree on and the message type it carries.
pub struct Channel<T> {
    pub id: &'static str,
    _t: std::marker::PhantomData<fn() -> T>,
}
impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Channel<T> {}
impl<T: Wire> Channel<T> {
    pub const fn new(id: &'static str) -> Self {
        Channel { id, _t: std::marker::PhantomData }
    }
    pub fn encode(&self, value: &T) -> Vec<u8> {
        to_bytes(value)
    }
    pub fn decode(&self, body: &[u8]) -> Option<T> {
        from_bytes(body)
    }
}

/// One message as the custom payload frames that carry it: `[version][id][part][parts]`, then in
/// the first frame the channel and the total length, then up to [`FRAME_DATA`] bytes of the body.
/// Frames of one connection arrive in order (one TCP stream, or the integrated server's pipe).
pub fn frames(id: u32, channel: &str, body: &[u8]) -> Vec<Vec<u8>> {
    let parts = body.len().div_ceil(FRAME_DATA).max(1);
    let mut out = Vec::with_capacity(parts);
    for part in 0..parts {
        let mut w = Writer::new();
        w.u8(NET_VERSION).u32(id).uvar(part as u64).uvar(parts as u64);
        if part == 0 {
            w.str(channel).uvar(body.len() as u64);
        }
        let from = part * FRAME_DATA;
        let to = (from + FRAME_DATA).min(body.len());
        w.buf.extend_from_slice(&body[from..to]);
        out.push(w.finish());
    }
    out
}

/// Why a frame was refused (its message is dropped whole, nothing half-applied).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    Version,
    Malformed,
    TooBig,
    TooMany,
    OutOfOrder,
}

#[derive(Clone, Debug)]
struct Partial {
    channel: String,
    total: usize,
    parts: usize,
    next: usize,
    data: Vec<u8>,
}

/// The messages of one connection being put back together from their frames.
#[derive(Clone, Debug, Default)]
pub struct Reassembler {
    pending: BTreeMap<u32, Partial>,
}
impl Reassembler {
    pub const fn new() -> Self {
        Reassembler { pending: BTreeMap::new() }
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    /// One frame: the finished message `(channel, body)` when it was the last of its message.
    pub fn push(&mut self, frame: &[u8]) -> Result<Option<(String, Vec<u8>)>, FrameError> {
        let mut r = Reader::new(frame);
        if r.u8().ok_or(FrameError::Malformed)? != NET_VERSION {
            return Err(FrameError::Version);
        }
        let id = r.u32().ok_or(FrameError::Malformed)?;
        let part = usize::try_from(r.uvar().ok_or(FrameError::Malformed)?).map_err(|_| FrameError::Malformed)?;
        let parts = usize::try_from(r.uvar().ok_or(FrameError::Malformed)?).map_err(|_| FrameError::Malformed)?;
        if parts == 0 || part >= parts || parts > MAX_MESSAGE / FRAME_DATA + 1 {
            self.pending.remove(&id);
            return Err(FrameError::Malformed);
        }
        if part == 0 {
            let channel = r.str().ok_or(FrameError::Malformed)?;
            if channel.is_empty() || channel.len() > MAX_CHANNEL {
                return Err(FrameError::Malformed);
            }
            let total = usize::try_from(r.uvar().ok_or(FrameError::Malformed)?).map_err(|_| FrameError::Malformed)?;
            if total > MAX_MESSAGE {
                return Err(FrameError::TooBig);
            }
            if total.div_ceil(FRAME_DATA).max(1) != parts {
                return Err(FrameError::Malformed);
            }
            if self.pending.len() >= MAX_PENDING && !self.pending.contains_key(&id) {
                return Err(FrameError::TooMany);
            }
            self.pending.insert(
                id,
                Partial { channel: channel.to_owned(), total, parts, next: 0, data: Vec::with_capacity(total.min(FRAME_DATA * 4)) },
            );
        }
        let Some(p) = self.pending.get_mut(&id) else {
            return Err(FrameError::OutOfOrder);
        };
        if p.next != part || p.parts != parts {
            self.pending.remove(&id);
            return Err(FrameError::OutOfOrder);
        }
        let chunk = r.rest();
        let expected = if part + 1 == parts { p.total - part * FRAME_DATA } else { FRAME_DATA };
        if chunk.len() != expected {
            self.pending.remove(&id);
            return Err(FrameError::Malformed);
        }
        p.data.extend_from_slice(chunk);
        p.next += 1;
        if p.next < p.parts {
            return Ok(None);
        }
        let p = self.pending.remove(&id).expect("just seen");
        Ok(Some((p.channel, p.data)))
    }
}

// ---- forja menus: a mod's menu (any slot layout, its own drawn screen) for a guest ----
//
// The server keeps the menu (the mod's own host code runs it, as the guest); the guest's client
// mirrors what the server sends (container slots, the machine's state, the mod's sections) and sends
// its input back. Item stacks travel as the game's item NBT (bytes); the rules compare the bytes.

/// The menu messages' channel (inside [`NET_CHANNEL`] frames).
pub const MENU_CHANNEL: &str = "forja:menu";
/// The version of the menu protocol both halves must speak.
pub const MENU_VERSION: u32 = 1;
/// The slot of a click outside the window (`AbstractContainerMenu.SLOT_CLICKED_OUTSIDE`).
pub const SLOT_OUTSIDE: i32 = -999;
/// Container slots a menu may have at most (game slots 300..500).
pub const MAX_MENU_SLOTS: usize = 200;

/// What a guest did in the open menu. Slots are the game's slot ids (0..36 inventory, 300.. the
/// container, 500.. armour, 504 off hand, -999 outside), as `Game::menu_ops` takes them.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuInput {
    /// Clicks, drags and keys on slots (`(slot, button, ClickType ordinal)`).
    Ops(Vec<(i32, i32, u8)>),
    /// A press on the mod's own widgets: the cursor and the window size it was measured in.
    Mouse { button: u8, x: f32, y: f32, w: u32, h: u32, shift: bool, extra: Vec<u8> },
    /// The wheel over the window (`slot` = the slot under the cursor, -1 none).
    Scroll { delta: f32, slot: i32, x: f32, y: f32, w: u32, h: u32, shift: bool },
    /// A key the window takes (its code in the game side's key table).
    Key { key: u16, shift: bool },
    /// Typed text, or a backspace, into the window's focused field.
    Text { text: Option<String>, backspace: bool },
    /// A mod-defined action (a button with an argument).
    Button { id: u32, arg: i64 },
}
impl Wire for MenuInput {
    fn put(&self, w: &mut Writer) {
        match self {
            MenuInput::Ops(ops) => {
                w.u8(0).put(ops);
            }
            MenuInput::Mouse { button, x, y, w: ww, h, shift, extra } => {
                w.u8(1).u8(*button).f32(*x).f32(*y).u32(*ww).u32(*h).bool(*shift).bytes(extra);
            }
            MenuInput::Scroll { delta, slot, x, y, w: ww, h, shift } => {
                w.u8(2).f32(*delta).i32(*slot).f32(*x).f32(*y).u32(*ww).u32(*h).bool(*shift);
            }
            MenuInput::Key { key, shift } => {
                w.u8(3).u32(u32::from(*key)).bool(*shift);
            }
            MenuInput::Text { text, backspace } => {
                w.u8(4).put(text).bool(*backspace);
            }
            MenuInput::Button { id, arg } => {
                w.u8(5).u32(*id).ivar(*arg);
            }
        }
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        Some(match r.u8()? {
            0 => {
                let ops: Vec<(i32, i32, u8)> = r.get()?;
                // A drag is a few dozen clicks; more is not a person's input.
                if ops.len() > 512 {
                    return None;
                }
                MenuInput::Ops(ops)
            }
            1 => MenuInput::Mouse {
                button: r.u8()?,
                x: r.f32()?,
                y: r.f32()?,
                w: r.u32()?,
                h: r.u32()?,
                shift: r.bool()?,
                extra: r.bytes()?.to_vec(),
            },
            2 => MenuInput::Scroll { delta: r.f32()?, slot: r.i32()?, x: r.f32()?, y: r.f32()?, w: r.u32()?, h: r.u32()?, shift: r.bool()? },
            3 => MenuInput::Key { key: u16::try_from(r.u32()?).ok()?, shift: r.bool()? },
            4 => MenuInput::Text { text: r.get()?, backspace: r.bool()? },
            5 => MenuInput::Button { id: r.u32()?, arg: r.ivar()? },
            _ => return None,
        })
    }
}

/// Client -> server.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuToServer {
    /// The answer to the server's hello: this client shows forja menus.
    Hello { version: u32 },
    /// An input in window `window`; `seq` grows by one per input (a repeat is ignored).
    Input { window: i32, seq: u32, input: MenuInput },
    /// The guest closed the window.
    Close { window: i32 },
    /// The size of the guest's screen in GUI pixels (`HudBatch` width and height), for the windows
    /// the server lays out as the guest (sent with the hello and whenever it changes).
    Size { w: u32, h: u32 },
}
impl Wire for MenuToServer {
    fn put(&self, w: &mut Writer) {
        match self {
            MenuToServer::Hello { version } => {
                w.u8(0).u32(*version);
            }
            MenuToServer::Input { window, seq, input } => {
                w.u8(1).i32(*window).u32(*seq).put(input);
            }
            MenuToServer::Close { window } => {
                w.u8(2).i32(*window);
            }
            MenuToServer::Size { w: ww, h } => {
                w.u8(3).u32(*ww).u32(*h);
            }
        }
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        Some(match r.u8()? {
            0 => MenuToServer::Hello { version: r.u32()? },
            1 => MenuToServer::Input { window: r.i32()?, seq: r.u32()?, input: r.get()? },
            2 => MenuToServer::Close { window: r.i32()? },
            3 => MenuToServer::Size { w: r.u32()?, h: r.u32()? },
            _ => return None,
        })
    }
}

/// Server -> client.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuToClient {
    /// A forja server greets a guest; the guest answers with its own hello.
    Hello { version: u32 },
    /// A menu opens: its kind (`ironchest:chest`, `ae2:window`, `forja:machine`...), the block,
    /// the number of container slots and the title.
    Open { window: i32, kind: String, pos: Pos, size: u32, title: String },
    /// Container slots that changed: `(slot, item NBT or empty)`; `size` = how many there are now
    /// (a window's layout may change: AE2's dialogs).
    Slots { window: i32, size: u32, changes: Vec<(u32, Option<Vec<u8>>)> },
    /// The forja machine at the menu's block (its side-file text), or none.
    Machine { window: i32, text: Option<String> },
    /// One of the mod's own sections of the menu's state changed.
    Section { window: i32, index: u32, data: Vec<u8> },
    /// The server closed the window.
    Close { window: i32 },
}
impl Wire for MenuToClient {
    fn put(&self, w: &mut Writer) {
        match self {
            MenuToClient::Hello { version } => {
                w.u8(0).u32(*version);
            }
            MenuToClient::Open { window, kind, pos, size, title } => {
                w.u8(1).i32(*window).str(kind).put(pos).u32(*size).str(title);
            }
            MenuToClient::Slots { window, size, changes } => {
                w.u8(2).i32(*window).u32(*size).put(changes);
            }
            MenuToClient::Machine { window, text } => {
                w.u8(3).i32(*window).put(text);
            }
            MenuToClient::Section { window, index, data } => {
                w.u8(4).i32(*window).u32(*index).bytes(data);
            }
            MenuToClient::Close { window } => {
                w.u8(5).i32(*window);
            }
        }
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        Some(match r.u8()? {
            0 => MenuToClient::Hello { version: r.u32()? },
            1 => {
                let window = r.i32()?;
                let kind = r.str()?.to_owned();
                let pos = r.get()?;
                let size = r.u32()?;
                if size as usize > MAX_MENU_SLOTS {
                    return None;
                }
                MenuToClient::Open { window, kind, pos, size, title: r.str()?.to_owned() }
            }
            2 => {
                let window = r.i32()?;
                let size = r.u32()?;
                if size as usize > MAX_MENU_SLOTS {
                    return None;
                }
                MenuToClient::Slots { window, size, changes: r.get()? }
            }
            3 => MenuToClient::Machine { window: r.i32()?, text: r.get()? },
            4 => MenuToClient::Section { window: r.i32()?, index: r.u32()?, data: r.bytes()?.to_vec() },
            5 => MenuToClient::Close { window: r.i32()? },
            _ => return None,
        })
    }
}

/// What the client was last told of a list (container slots, sections), for change detection
/// (`AbstractContainerMenu.broadcastChanges`). It starts as the client starts: everything empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Mirror<T> {
    pub sent: Vec<Option<T>>,
}
impl<T: Clone + PartialEq> Mirror<T> {
    pub fn new(size: usize) -> Self {
        Mirror { sent: vec![None; size] }
    }
    /// The entries that differ from what was sent (the list may have grown or shrunk: what is
    /// gone is sent as empty), now marked as sent.
    pub fn diff(&mut self, now: &[Option<T>]) -> Vec<(u32, Option<T>)> {
        let n = now.len().max(self.sent.len());
        let mut out = Vec::new();
        for i in 0..n {
            let cur = now.get(i).cloned().flatten();
            if self.sent.get(i).cloned().flatten() != cur {
                out.push((i as u32, cur));
            }
        }
        self.sent = now.to_vec();
        out
    }
}
/// The client's side of [`Mirror::diff`]: the changes onto its copy (slots past `max` ignored).
pub fn apply_changes<T: Clone>(slots: &mut Vec<Option<T>>, changes: &[(u32, Option<T>)], max: usize) {
    for (i, v) in changes {
        let i = *i as usize;
        if i >= max {
            continue;
        }
        if slots.len() <= i {
            slots.resize(i + 1, None);
        }
        slots[i] = v.clone();
    }
}

/// The server's gate on a guest's inputs: one window, each input once, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputGate {
    pub window: i32,
    pub last: Option<u32>,
}
impl InputGate {
    pub fn new(window: i32) -> Self {
        InputGate { window, last: None }
    }
    /// The input may run: its window is the open one and it is newer than the last that ran.
    pub fn accept(&mut self, window: i32, seq: u32) -> bool {
        if window != self.window || self.last.is_some_and(|l| seq <= l) {
            return false;
        }
        self.last = Some(seq);
        true
    }
}

/// The next window id of a guest's forja menus (1..100, as `ServerPlayer.nextContainerCounter`),
/// never the window the hub's own vanilla menu uses.
pub fn next_window(current: i32, vanilla: Option<i32>) -> i32 {
    let mut w = current.rem_euclid(100) + 1;
    if Some(w) == vanilla {
        w = w % 100 + 1;
    }
    w
}

// ---- redstone ("Redstone"): signals that mod blocks and parts emit ----

/// The strongest redstone signal.
pub const SIGNAL_MAX: u8 = 15;

/// What a block emits, per side (`Dir` order): weak (`getSignal`) and strong (`getDirectSignal`,
/// which also powers through the block on that side), 0..15 each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Signal {
    pub weak: [u8; 6],
    pub strong: [u8; 6],
}
impl Signal {
    pub fn off() -> Self {
        Signal::default()
    }
    /// The same on every side.
    pub fn uniform(weak: u8, strong: u8) -> Self {
        Signal { weak: [weak.min(SIGNAL_MAX); 6], strong: [strong.min(SIGNAL_MAX); 6] }
    }
    /// Only out of `side` (AE2's level emitter on that face: weak and strong 15 when on).
    pub fn side(side: Dir, weak: u8, strong: u8) -> Self {
        let mut s = Signal::off();
        s.set(side, weak, strong);
        s
    }
    pub fn set(&mut self, side: Dir, weak: u8, strong: u8) {
        self.weak[side.idx()] = weak.min(SIGNAL_MAX);
        self.strong[side.idx()] = strong.min(SIGNAL_MAX);
    }
    pub fn weak(&self, side: Dir) -> u8 {
        self.weak[side.idx()]
    }
    pub fn strong(&self, side: Dir) -> u8 {
        self.strong[side.idx()]
    }
    pub fn is_off(&self) -> bool {
        self.weak.iter().chain(&self.strong).all(|&v| v == 0)
    }
    /// Max of two signals, side by side (several parts of one host emitting).
    pub fn max(&self, other: &Signal) -> Signal {
        let mut s = *self;
        for i in 0..6 {
            s.weak[i] = s.weak[i].max(other.weak[i]);
            s.strong[i] = s.strong[i].max(other.strong[i]);
        }
        s
    }
}

/// The emitting blocks of one level.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Redstone {
    map: BTreeMap<Pos, Signal>,
}
impl Redstone {
    pub const fn new() -> Self {
        Redstone { map: BTreeMap::new() }
    }
    pub fn get(&self, p: Pos) -> Signal {
        self.map.get(&p).copied().unwrap_or_default()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn positions(&self) -> Vec<Pos> {
        self.map.keys().copied().collect()
    }
    /// What the block at `p` emits from now on. Returns the blocks whose neighbours must hear of
    /// it (`Level.updateNeighborsAt`), nothing when it did not change: the block itself, then for
    /// each side whose strong signal changed the block on that side (it is powered through, so
    /// its own neighbours change: AE2's `notifyBlocksOfNeighbors(pos.relative(side))`).
    pub fn set(&mut self, p: Pos, s: Signal) -> Vec<Pos> {
        let old = self.get(p);
        if old == s {
            return Vec::new();
        }
        if s.is_off() {
            self.map.remove(&p);
        } else {
            self.map.insert(p, s);
        }
        let mut out = vec![p];
        for d in Dir::ALL {
            if old.strong(d) != s.strong(d) {
                let q = offset(p, d);
                if !out.contains(&q) {
                    out.push(q);
                }
            }
        }
        out
    }
    pub fn clear(&mut self, p: Pos) -> Vec<Pos> {
        self.set(p, Signal::off())
    }
}

/// Horizon's answer for an emitting block: its signal sources are evaluated in a generic
/// `Sim<L: LevelAccess>` (horizon-mixins refuses generic impls), whose only call out per position
/// is `LevelAccess::block_entity_signal(pos, state, direction)` for states of the TrappedChest (or
/// Jukebox) class. `query` is vanilla's `direction` (from the asking block towards the source):
/// the side asked is `query.opposite()`. Weak queries reach it on every side; the strong query
/// only from below (`TrappedChestBlock.getDirectSignal`: `direction == UP`), through the same call,
/// so the side below answers the stronger of its two values.
pub fn horizon_signal(s: &Signal, query: Dir) -> i32 {
    let side = query.opposite();
    let v = if side == Dir::Down { s.weak(side).max(s.strong(side)) } else { s.weak(side) };
    i32::from(v)
}
/// What a neighbour on `side` really gets on Horizon: `(weak, strong)`. Strong power leaves only
/// downwards (the limit above); the other sides give their weak signal.
pub fn horizon_delivers(s: &Signal, side: Dir) -> (u8, u8) {
    let v = horizon_signal(s, side.opposite()) as u8;
    (v, if side == Dir::Down { v } else { 0 })
}

}
#[allow(unused_imports)]
pub(in crate::game) use engine::{
    Access, Bar, BucketUse, Dir, EnergyHandler, EnergyInfo, EnergyStorage, FluidHandler, FluidStack,
    FluidTank, Grow, Host, ItemHandler, Machine, NO_DROP, Part, PartShape, PartSpec, Pos, Rel, Sides, Slot,
    SlotRole, Spec, TankInfo, TankRole,
};

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
const MOD_ID: &str = "forja";

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
