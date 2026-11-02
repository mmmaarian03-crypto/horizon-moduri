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
/// that the pack's mods move by pipe relies on it; see mods/forja/API.md).
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

// ---- shapes from models (shapes from models): a content-mod block whose model is not the shape
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

/// The rule of shapes from models for one block state. `None` = the template's shapes stay:
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

// ---- mod networking: a typed channel between a mod's server half and its client
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
