// EnderStorage inside Horizon's game (nightly 142388e1c). The rules are in `horizon_client::enderstorage`
// (src/lib.rs + native/library.rs, mixins/enderstorage/shared.rs); this part is the game's side.
//
// What the original keeps in `EnderStorageManager` (one inventory and one tank per frequency, in the
// world folder) is `Store`, written to `data/enderstorage/store.nbt` beside the saves. Each Ender
// Chest block also has Horizon's own `BlockEntity::Chest(Vec<..>)` as a working copy of its
// frequency's inventory, so hoppers, comparators and the vanilla chest menu work on it
// (the ironchest pattern); `enderstorage_tick` keeps every copy and the inventory equal
// (`reconcile`). The Ender Pouch opens its frequency's inventory on a placeholder container under a
// vehicle-style key, as a chest minecart does. The Ender Tank has no block entity: its tank is in
// `Store`. Through foundry (`crate::game::foundry`, a native mod that must be enabled with this one) the
// tank is the fluid handler of its block on every side (`TileEnderTank`'s FLUID_HANDLER), so Pipez
// fluid pipes and foundry machines fill and drain the frequency's tank.
use super::{Game, Screen};
use horizon_client::{containers::BlockEntity, enderstorage as lib, items::ItemStack};
use horizon_core::position::BlockPos;
use horizon_core::random::RandomSource;
use horizon_world::level::flags as level_flags;
use horizon_world::nbt::{JavaString, Limits, NamedTag, Tag, read_named, write_named};
use horizon_world::registry::BlockStateId;
use std::collections::{BTreeMap, HashSet};
use std::sync::Mutex;
use winit::keyboard::KeyCode;

// The module that the tests run; the game uses the same rules through `horizon_client::enderstorage`.
#[allow(dead_code, unused_imports)]
mod engine {
    // EnderStorage for Horizon: the rules, without any game code.
// Written from EnderStorage 1.20.1-2.11.0.188 (covers1624 / ChickenBones, MIT), the jar the pack plays;
// the source is branch 1.20.1 of TheCBProject/EnderStorage (sources/enderstorage) and, for the colours,
// CodeChickenLib 1.20.1 (sources/codechickenlib: EnumColour, Rotation).
//
// The game side (native/horizon.rs) keeps what the original keeps in `EnderStorageManager`
// (one inventory / tank per frequency, in the world folder) and gives each Ender Chest a
// `BlockEntity::Chest(Vec<..>)` that is a working copy of its frequency's inventory (so hoppers,
// comparators and the vanilla chest menu work on it); `reconcile` keeps the copies and the
// frequency's inventory equal. Everything here works on plain data so the tests can run it.

use std::collections::BTreeMap;

pub type Pos = [i32; 3];

// ---- the pack's EnderStorage.cfg (config/EnderStorage.cfg) ----
/// `personalItem`: the item that locks Ender Chests and Tanks.
pub const PERSONAL_ITEM: &str = "minecraft:diamond";
/// `anarchyMode`.
pub const ANARCHY_MODE: bool = false;
/// `item_storage_size`: 0 = 3x3, 1 = 3x9, 2 = 6x9.
pub const ITEM_STORAGE_SIZE: usize = 1;
/// `useVanillaEnderChestsSounds`.
pub const USE_VANILLA_ENDER_CHEST_SOUNDS: bool = false;

pub const CHEST: &str = "enderstorage:ender_chest";
pub const TANK: &str = "enderstorage:ender_tank";
pub const POUCH: &str = "enderstorage:ender_pouch";

// ---- colours (CodeChickenLib EnumColour) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Colour {
    White,
    Orange,
    Magenta,
    LightBlue,
    Yellow,
    Lime,
    Pink,
    Gray,
    LightGray,
    Cyan,
    Purple,
    Blue,
    Brown,
    Green,
    Red,
    Black,
}
use Colour::*;

impl Colour {
    pub const ALL: [Colour; 16] = [
        White, Orange, Magenta, LightBlue, Yellow, Lime, Pink, Gray, LightGray, Cyan, Purple, Blue,
        Brown, Green, Red, Black,
    ];
    /// `getSerializedName`.
    pub fn name(self) -> &'static str {
        [
            "white",
            "orange",
            "magenta",
            "light_blue",
            "yellow",
            "lime",
            "pink",
            "gray",
            "light_gray",
            "cyan",
            "purple",
            "blue",
            "brown",
            "green",
            "red",
            "black",
        ][self as usize]
    }
    pub fn from_name(name: &str) -> Option<Colour> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
    /// `getWoolMeta` (the ordinal).
    pub fn wool_meta(self) -> i32 {
        self as i32
    }
    /// `fromWoolMeta`; an id out of 0..16 is the original's array error, None here.
    pub fn from_wool_meta(id: i32) -> Option<Colour> {
        usize::try_from(id).ok().and_then(|i| Self::ALL.get(i).copied())
    }
    /// `getUnlocalizedName`.
    pub fn lang_key(self) -> String {
        format!("item.minecraft.firework_star.{}", self.name())
    }
    /// `fromDyeStack`: the `forge:dyes/<colour>` tag, i.e. the sixteen dyes.
    pub fn from_dye_item(item: &str) -> Option<Colour> {
        item.strip_prefix("minecraft:")?
            .strip_suffix("_dye")
            .and_then(Self::from_name)
    }
    /// `fromWoolStack`: the `forge:wool/<colour>` tag, i.e. the sixteen wool blocks.
    pub fn from_wool_item(item: &str) -> Option<Colour> {
        item.strip_prefix("minecraft:")?
            .strip_suffix("_wool")
            .and_then(Self::from_name)
    }
    /// `EnumColour.mix` (the symmetric table of dye mixes).
    pub fn mix(a: Colour, b: Colour) -> Option<Colour> {
        if a == b {
            return Some(a);
        }
        const TABLE: [(Colour, Colour, Colour); 9] = [
            (Yellow, Red, Orange),
            (Pink, Purple, Magenta),
            (White, Blue, LightBlue),
            (White, Green, Lime),
            (White, Red, Pink),
            (White, Black, Gray),
            (White, Gray, LightGray),
            (Blue, Green, Cyan),
            (Blue, Red, Purple),
        ];
        const MORE: [(Colour, Colour, Colour); 2] =
            [(Orange, Red, Brown), (Yellow, Blue, Green)];
        TABLE
            .iter()
            .chain(MORE.iter())
            .find(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a))
            .map(|t| t.2)
    }
}

// ---- frequencies (api/Frequency) ----

/// The owner of a personal frequency: `putUUID` (four ints) and the player's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub uuid: [i32; 4],
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frequency {
    pub left: Colour,
    pub middle: Colour,
    pub right: Colour,
    pub owner: Option<Owner>,
}

impl Default for Frequency {
    fn default() -> Self {
        Self::new(White, White, White)
    }
}

/// `UUID.toString` of `putUUID`'s four ints.
pub fn uuid_string(u: [i32; 4]) -> String {
    let (a, b, c, d) = (u[0] as u32, u[1] as u32, u[2] as u32, u[3] as u32);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:04x}{:08x}",
        a,
        b >> 16,
        b & 0xffff,
        c >> 16,
        c & 0xffff,
        d
    )
}

impl Frequency {
    pub fn new(left: Colour, middle: Colour, right: Colour) -> Self {
        Self {
            left,
            middle,
            right,
            owner: None,
        }
    }
    pub fn has_owner(&self) -> bool {
        self.owner.is_some()
    }
    /// `toString`: the storage key (the owner's UUID, not their name).
    pub fn key(&self) -> String {
        let mut s = format!(
            "left={},middle={},right={}",
            self.left.name(),
            self.middle.name(),
            self.right.name()
        );
        if let Some(o) = &self.owner {
            s.push_str(",owner=");
            s.push_str(&uuid_string(o.uuid));
        }
        s
    }
    /// The three colours as `set(EnumColour[])` takes them (None leaves one alone).
    pub fn set(&mut self, colours: [Option<Colour>; 3]) {
        if let Some(c) = colours[0] {
            self.left = c;
        }
        if let Some(c) = colours[1] {
            self.middle = c;
        }
        if let Some(c) = colours[2] {
            self.right = c;
        }
    }
    pub fn colours(&self) -> [Colour; 3] {
        [self.left, self.middle, self.right]
    }
    /// `Frequency.getTooltip`'s three language keys.
    pub fn tooltip_keys(&self) -> [String; 3] {
        self.colours().map(Colour::lang_key)
    }
    /// The custom model data strings of an item: the three colours, then `owned`.
    pub fn model_strings(&self) -> [&'static str; 3] {
        self.colours().map(Colour::name)
    }
}

// ---- the item inventory of a frequency (storage/EnderItemStorage) ----

/// `EnderItemStorage.sizes`.
pub const SIZES: [usize; 3] = [9, 27, 54];

#[derive(Clone, Debug, PartialEq)]
pub struct ItemStorage<S> {
    /// An index into [`SIZES`].
    pub size: usize,
    pub items: Vec<Option<S>>,
}

impl<S: Clone> ItemStorage<S> {
    pub fn new(config: usize) -> Self {
        let size = config.min(2);
        Self {
            size,
            items: vec![None; SIZES[size]],
        }
    }
    /// `loadFromTag`: the saved size index and items (padded or cut to that size), then aligned.
    pub fn from_saved(size: i32, mut items: Vec<Option<S>>, config: usize) -> Self {
        let size = usize::try_from(size).ok().filter(|s| *s <= 2).unwrap_or(config.min(2));
        items.resize(SIZES[size], None);
        let mut s = Self { size, items };
        if s.size != config {
            s.align(config);
        }
        s
    }
    /// `alignSize`: a bigger configured size grows the inventory; a smaller one shrinks it only
    /// when the stacks fit (they are packed to the front).
    pub fn align(&mut self, config: usize) {
        let config = config.min(2);
        if config > self.size {
            self.items.resize(SIZES[config], None);
            self.size = config;
        } else {
            let stacks = self.items.iter().filter(|i| i.is_some()).count();
            if stacks <= SIZES[config] {
                let mut packed: Vec<Option<S>> = self.items.iter().filter(|i| i.is_some()).cloned().collect();
                packed.resize(SIZES[config], None);
                self.items = packed;
                self.size = config;
            }
        }
    }
    /// `saveToTag` aligns first when nobody has it open.
    pub fn before_save(&mut self, config: usize, open: bool) {
        if self.size != config.min(2) && !open {
            self.align(config);
        }
    }
}

// ---- the tank of a frequency (storage/EnderLiquidStorage) ----

pub const BUCKET: u32 = 1000;
/// `EnderLiquidStorage.CAPACITY` = 16 * FluidUtils.B.
pub const TANK_CAPACITY: u32 = 16 * BUCKET;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Liquid {
    pub fluid: Option<String>,
    pub amount: u32,
}

impl Liquid {
    /// `FluidTank.fill(EXECUTE)`.
    pub fn fill(&mut self, fluid: &str, amount: u32) -> u32 {
        if amount == 0 {
            return 0;
        }
        match &self.fluid {
            None => {
                let filled = amount.min(TANK_CAPACITY);
                self.fluid = Some(fluid.to_owned());
                self.amount = filled;
                filled
            }
            Some(f) if f == fluid => {
                let filled = amount.min(TANK_CAPACITY - self.amount);
                self.amount += filled;
                filled
            }
            Some(_) => 0,
        }
    }
    /// What `fill` would take, without taking it.
    pub fn can_fill(&self, fluid: &str, amount: u32) -> u32 {
        self.clone().fill(fluid, amount)
    }
    /// `FluidTank.drain(max, EXECUTE)`.
    pub fn drain(&mut self, max: u32) -> Option<(String, u32)> {
        let fluid = self.fluid.clone()?;
        let drained = max.min(self.amount);
        if drained == 0 {
            return None;
        }
        self.amount -= drained;
        if self.amount == 0 {
            self.fluid = None;
        }
        Some((fluid, drained))
    }
    /// `TileEnderTank.comparatorInput`.
    pub fn comparator(&self) -> u8 {
        (self.amount * 14 / TANK_CAPACITY + u32::from(self.amount > 0)) as u8
    }
}

/// The bucket items the tank can fill from or empty into (`FluidBucketWrapper`):
/// (filled bucket, fluid, fill sound, empty sound).
pub const BUCKETS: [(&str, &str, &str, &str); 3] = [
    ("minecraft:water_bucket", "minecraft:water", "minecraft:item.bucket.fill", "minecraft:item.bucket.empty"),
    ("minecraft:lava_bucket", "minecraft:lava", "minecraft:item.bucket.fill_lava", "minecraft:item.bucket.empty_lava"),
    ("minecraft:milk_bucket", "minecraft:milk", "minecraft:item.bucket.fill", "minecraft:item.bucket.empty"),
];
pub const EMPTY_BUCKET: &str = "minecraft:bucket";

/// What `FluidUtil.interactWithFluidHandler` does with the held item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bucket {
    /// Nothing happens (the click falls through to the original's `activate`, which is false).
    None,
    /// An empty bucket took 1000 mB: it becomes this filled bucket; the sound.
    Filled { bucket: &'static str, sound: &'static str },
    /// A filled bucket gave its 1000 mB: it becomes an empty bucket; the sound.
    Emptied { sound: &'static str },
}

/// `FluidUtil.interactWithFluidHandler(player, hand, tank)`: first the held item is filled from
/// the tank (an empty bucket and at least 1000 mB), else emptied into it (the whole bucket must fit).
pub fn bucket_interact(held: &str, tank: &mut Liquid) -> Bucket {
    if held == EMPTY_BUCKET
        && let Some(fluid) = tank.fluid.clone()
        && tank.amount >= BUCKET
        && let Some(b) = BUCKETS.iter().find(|b| b.1 == fluid)
    {
        tank.drain(BUCKET);
        return Bucket::Filled { bucket: b.0, sound: b.2 };
    }
    if let Some(b) = BUCKETS.iter().find(|b| b.0 == held)
        && tank.can_fill(b.1, BUCKET) == BUCKET
    {
        tank.fill(b.1, BUCKET);
        return Bucket::Emptied { sound: b.3 };
    }
    Bucket::None
}

// ---- keeping the block entities' copies equal to the frequency's inventory ----

/// One block entity's working copy of the inventory. `fresh`: just made or loaded, so it
/// only ever receives (its items are not a change made by someone).
pub struct Working<'a, S> {
    pub items: &'a mut Vec<Option<S>>,
    pub fresh: bool,
}

/// Makes the inventory and every copy equal. A change made to the inventory itself (a pouch, a
/// command) since `last` wins, else the first copy that differs from `last` (a hopper or a menu
/// on that chest). Returns whether the inventory changed (it must be saved).
pub fn reconcile<S: Clone + PartialEq>(
    storage: &mut ItemStorage<S>,
    last: &mut Option<Vec<Option<S>>>,
    copies: &mut [Working<'_, S>],
) -> bool {
    let known = last.take().unwrap_or_else(|| storage.items.clone());
    let mut changed = false;
    if storage.items != known {
        changed = true;
    } else if let Some(c) = copies
        .iter()
        .find(|c| !c.fresh && c.items.len() == storage.items.len() && **c.items != known)
    {
        storage.items = c.items.clone();
        changed = true;
    }
    for c in copies.iter_mut() {
        if **c.items != storage.items {
            c.items.clone_from(&storage.items);
        }
        c.fresh = false;
    }
    *last = Some(storage.items.clone());
    changed
}

// ---- the crafting recipes (recipe/CreateRecipe, recipe/ReColourRecipe) ----

/// `CreateRecipe.assemble`: the first wool found going down each column in turn (x outer, y
/// inner); white when there is none. The frequency is that colour three times.
pub fn create_frequency(grid: &[Option<&str>]) -> Frequency {
    let mut colour = White;
    'found: for x in 0..3 {
        for y in 0..3 {
            if let Some(Some(item)) = grid.get(x + y * 3)
                && let Some(c) = Colour::from_wool_item(item)
            {
                colour = c;
                break 'found;
            }
        }
    }
    Frequency::new(colour, colour, colour)
}

/// `ReColourRecipe.matches` on a 3x3 grid for the recipe whose ingredient is `target`.
pub fn recolour_matches(grid: &[Option<&str>], target: &str) -> bool {
    if grid.len() != 9 || grid.iter().all(Option::is_none) {
        return false;
    }
    let at = |col: usize, row: usize| grid[col + row * 3];
    let mut found_row = None;
    for row in 1..3 {
        if at(1, row) == Some(target) {
            found_row = Some(row);
            break;
        }
    }
    let Some(found_row) = found_row else {
        return false;
    };
    let mut colours: [Option<Colour>; 3] = [None; 3];
    let mut has_dye = false;
    for col in 0..3 {
        for row in 0..found_row {
            if let Some(item) = at(col, row)
                && let Some(colour) = Colour::from_dye_item(item)
            {
                if let Some(have) = colours[col] {
                    match Colour::mix(have, colour) {
                        Some(merge) if merge != colour => colours[col] = Some(merge),
                        _ => return false,
                    }
                } else {
                    has_dye = true;
                    colours[col] = Some(colour);
                }
            }
        }
    }
    if has_dye {
        for col in 0..3 {
            for row in 0..3 {
                if let Some(item) = at(col, row) {
                    let dye = Colour::from_dye_item(item);
                    if row >= found_row && dye.is_some() {
                        return false;
                    }
                    if item != target && dye.is_none() {
                        return false;
                    }
                }
            }
        }
    }
    has_dye
}

/// `ReColourRecipe.assemble`: the frequency of the ingredient with the dyed columns changed
/// (the owner is kept). `current` reads the frequency of the stack at the ingredient's cell.
pub fn recolour_result(
    grid: &[Option<&str>],
    target: &str,
    current: impl FnOnce(usize) -> Frequency,
) -> Frequency {
    let at = |col: usize, row: usize| grid[col + row * 3];
    let mut found_row = 0;
    let mut freq = Frequency::default();
    for row in 1..3 {
        if at(1, row) == Some(target) {
            found_row = row;
            freq = current(1 + row * 3);
            break;
        }
    }
    let mut colours: [Option<Colour>; 3] = [None; 3];
    for col in 0..3 {
        for row in 0..found_row {
            if let Some(item) = at(col, row)
                && let Some(colour) = Colour::from_dye_item(item)
            {
                colours[col] = match colours[col] {
                    None => Some(colour),
                    Some(have) => Colour::mix(have, colour),
                };
            }
        }
    }
    freq.set(colours);
    freq
}

// ---- where a click landed (BlockEnderChest / BlockEnderTank shapes, EnderKnobSlot) ----

type Box3 = [[f64; 3]; 2];

/// `Rotation.quarterRotations[k]` about the vertical axis through the origin.
fn quarter(k: u8, x: f64, z: f64) -> (f64, f64) {
    match k % 4 {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    }
}
fn turn_box(b: Box3, k: u8, about: (f64, f64)) -> Box3 {
    let a = quarter(k, b[0][0] - about.0, b[0][2] - about.1);
    let c = quarter(k, b[1][0] - about.0, b[1][2] - about.1);
    [
        [a.0.min(c.0) + about.0, b[0][1], a.1.min(c.1) + about.1],
        [a.0.max(c.0) + about.0, b[1][1], a.1.max(c.1) + about.1],
    ]
}
/// A click on face `face` of the block lands at `local` (block coordinates): is it on `b`?
/// The box counts a sixteenth of a block deeper and further along the face's own axis, because
/// the game's shape for the block is the plain chest, which the buttons and the latch stick out of.
fn on_box(b: Box3, face: [i32; 3], local: [f64; 3]) -> bool {
    (0..3).all(|a| {
        let (mut lo, mut hi) = (b[0][a], b[1][a]);
        if face[a] != 0 {
            lo -= 0.0625;
            hi += 0.0625;
        }
        local[a] >= lo - 1e-6 && local[a] <= hi + 1e-6
    })
}

/// `BlockEnderChest.BUTTONS[rot][i]`: `SELECTION_BUTTON` moved by `buttonT[i]`, then turned by
/// `-90 * rot` degrees about the block's centre.
pub fn chest_button(rot: u8, i: usize) -> Box3 {
    let dx = -(3.0 / 16.0) + (3.0 / 16.0) * i as f64;
    let b = [
        [dx - 1.0 / 16.0, 14.0 / 16.0, -2.0 / 16.0],
        [dx + 1.0 / 16.0, 15.0 / 16.0, 2.0 / 16.0],
    ];
    let t = turn_box(b, rot, (0.0, 0.0));
    [[t[0][0] + 0.5, t[0][1], t[0][2] + 0.5], [t[1][0] + 0.5, t[1][1], t[1][2] + 0.5]]
}
/// `EnderKnobSlot(rot).getSelectionBB()`: the latch on the front.
pub fn chest_latch(rot: u8) -> Box3 {
    let b = [[-0.0625, 0.4375, -0.5], [0.0625, 0.6875, -0.4375]];
    let t = turn_box(b, (rot + 2) % 4, (0.0, 0.0));
    [[t[0][0] + 0.5, t[0][1], t[0][2] + 0.5], [t[1][0] + 0.5, t[1][1], t[1][2] + 0.5]]
}

/// The part of an Ender Chest a click landed on (`subHit`): 0 the chest, 1 to 3 the left, middle
/// and right button, 4 the latch. Buttons and latch are only there while the lid is closed.
pub fn chest_sub_hit(rot: u8, face: [i32; 3], local: [f64; 3], lid_closed: bool) -> u8 {
    if !lid_closed {
        return 0;
    }
    for i in 0..3 {
        if on_box(chest_button(rot, i), face, local) {
            return i as u8 + 1;
        }
    }
    if on_box(chest_latch(rot), face, local) {
        return 4;
    }
    0
}

/// `BlockEnderTank.BUTTONS[rot][i]`: scaled by 0.6, moved, turned by `rot ^ 2` quarter turns about the centre.
pub fn tank_button(rot: u8, i: usize) -> Box3 {
    let c = (0.35 + (2 - i) as f64 * 0.15, 0.91, 0.5);
    let b = [
        [c.0 - 0.0375, c.1, c.2 - 0.075],
        [c.0 + 0.0375, c.1 + 0.0375, c.2 + 0.075],
    ];
    turn_box(b, (rot ^ 2) % 4, (0.5, 0.5))
}
/// `BlockEnderTank.DIAL[rot]`: the pressure dial.
pub fn tank_dial(rot: u8) -> Box3 {
    turn_box([[0.358, 0.268, 0.05], [0.662, 0.565, 0.15]], (rot ^ 2) % 4, (0.5, 0.5))
}
pub fn tank_sub_hit(rot: u8, face: [i32; 3], local: [f64; 3]) -> u8 {
    for i in 0..3 {
        if on_box(tank_button(rot, i), face, local) {
            return i as u8 + 1;
        }
    }
    if on_box(tank_dial(rot), face, local) {
        return 4;
    }
    0
}

/// `Math.floor(yaw * 4 / 360 + 2.5) & 3` (`onPlaced`): the rotation of a block placed by an
/// entity looking along `yaw`.
pub fn rotation_from_yaw(yaw: f32) -> u8 {
    ((f64::from(yaw) * 4.0 / 360.0 + 2.5).floor() as i64 & 3) as u8
}
/// The rotation of a chest whose `facing` property is this (south 0, west 1, north 2, east 3).
pub fn rotation_of_facing(facing: &str) -> u8 {
    match facing {
        "south" => 0,
        "west" => 1,
        "east" => 3,
        _ => 2,
    }
}

// ---- the blocks of a world (what the block entities' frequencies are) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Chest,
    Tank,
}
impl Kind {
    pub fn of(block: &str) -> Option<Kind> {
        match block {
            CHEST => Some(Kind::Chest),
            TANK => Some(Kind::Tank),
            _ => None,
        }
    }
    pub fn block(self) -> &'static str {
        match self {
            Kind::Chest => CHEST,
            Kind::Tank => TANK,
        }
    }
}

/// `TileFrequencyOwner.activate`'s checks before the menu: what a click with `held` on `sub_hit`
/// of a block with frequency `freq` does, for the player `who` (`BlockEnderStorage.use`).
#[derive(Clone, Debug, PartialEq)]
pub enum Use {
    /// Not ours: the block's own action (open the chest, fill the tank).
    Pass,
    /// A new frequency, the held item used up (unless creative); the click does nothing else.
    Recolour { freq: Frequency, consume: bool },
    /// The owner is set; a personal item used up (unless creative).
    Lock { freq: Frequency, consume: bool },
    /// Sneaking on the latch of an owned block: the owner goes, the personal item is given back
    /// (a full inventory fails the click).
    Unlock { freq: Frequency },
}

pub fn use_block(
    freq: &Frequency,
    sub_hit: u8,
    held: Option<&str>,
    sneaking: bool,
    who: &Owner,
) -> Use {
    if sub_hit == 4 {
        if sneaking && freq.has_owner() {
            let mut f = freq.clone();
            f.owner = None;
            return Use::Unlock { freq: f };
        }
        if held == Some(PERSONAL_ITEM) && !freq.has_owner() {
            let mut f = freq.clone();
            f.owner = Some(who.clone());
            return Use::Lock { freq: f, consume: true };
        }
    } else if (1..=3).contains(&sub_hit)
        && let Some(dye) = held.and_then(Colour::from_dye_item)
    {
        let mut f = freq.clone();
        let mut colours = [None; 3];
        colours[usize::from(sub_hit) - 1] = Some(dye);
        f.set(colours);
        return Use::Recolour { freq: f, consume: true };
    }
    Use::Pass
}

// ---- everything the game keeps for a world ----

/// The world's part of the mod: the inventories and tanks by frequency key, and each ender block's frequency.
#[derive(Clone, Debug)]
pub struct Store<S> {
    pub items: BTreeMap<String, ItemStorage<S>>,
    pub liquids: BTreeMap<String, Liquid>,
    /// (dimension, position) of each block of the mod: what it is and its frequency.
    pub blocks: BTreeMap<(String, Pos), (Kind, Frequency)>,
    /// The inventories as they were at the last `reconcile`.
    pub last: BTreeMap<String, Option<Vec<Option<S>>>>,
    /// Something to write.
    pub dirty: bool,
}

impl<S: Clone + PartialEq> Store<S> {
    pub fn storage(&mut self, freq: &Frequency) -> &mut ItemStorage<S> {
        self.items
            .entry(freq.key())
            .or_insert_with(|| ItemStorage::new(ITEM_STORAGE_SIZE))
    }
    pub fn liquid(&mut self, freq: &Frequency) -> &mut Liquid {
        self.liquids.entry(freq.key()).or_default()
    }
    pub fn set_block(&mut self, dim: &str, pos: Pos, kind: Kind, freq: Frequency) {
        let value = (kind, freq);
        let entry = self.blocks.entry((dim.to_owned(), pos)).or_insert_with(|| value.clone());
        if *entry != value {
            *entry = value;
        }
        self.dirty = true;
    }
    pub fn block(&self, dim: &str, pos: Pos) -> Option<&(Kind, Frequency)> {
        self.blocks.get(&(dim.to_owned(), pos))
    }
    pub fn remove_block(&mut self, dim: &str, pos: Pos) -> Option<(Kind, Frequency)> {
        let r = self.blocks.remove(&(dim.to_owned(), pos));
        if r.is_some() {
            self.dirty = true;
        }
        r
    }
    /// The copies of one frequency's inventory, equalled.
    pub fn reconcile(&mut self, freq: &Frequency, copies: &mut [Working<'_, S>]) -> bool {
        let key = freq.key();
        let storage = self
            .items
            .entry(key.clone())
            .or_insert_with(|| ItemStorage::new(ITEM_STORAGE_SIZE));
        let last = self.last.entry(key).or_default();
        let changed = reconcile(storage, last, copies);
        if changed {
            self.dirty = true;
        }
        changed
    }
}

impl<S> Default for Store<S> {
    fn default() -> Self {
        Self {
            items: BTreeMap::new(),
            liquids: BTreeMap::new(),
            blocks: BTreeMap::new(),
            last: BTreeMap::new(),
            dirty: false,
        }
    }
}

// ---- the crafting recipes that make a new block or pouch (data/enderstorage/recipes/*.json) ----

const WOOLS: [&str; 16] = [
    "minecraft:white_wool",
    "minecraft:orange_wool",
    "minecraft:magenta_wool",
    "minecraft:light_blue_wool",
    "minecraft:yellow_wool",
    "minecraft:lime_wool",
    "minecraft:pink_wool",
    "minecraft:gray_wool",
    "minecraft:light_gray_wool",
    "minecraft:cyan_wool",
    "minecraft:purple_wool",
    "minecraft:blue_wool",
    "minecraft:brown_wool",
    "minecraft:green_wool",
    "minecraft:red_wool",
    "minecraft:black_wool",
];

/// The three `enderstorage:create_recipe`s on a 3x3 grid (the tags of the 1.20.1 recipes,
/// forge:wool, forge:obsidian, forge:chests/wooden, forge:leather and forge:ender_pearls, as vanilla's
/// members): the item they make.
pub fn create_recipe(cells: &[Option<&str>]) -> Option<&'static str> {
    if cells.len() != 9 {
        return None;
    }
    fn chest(l: u8, i: &str) -> bool {
        match l {
            b'B' => i == "minecraft:blaze_rod",
            b'W' => WOOLS.contains(&i),
            b'O' => i == "minecraft:obsidian",
            b'C' => i == "minecraft:chest" || i == "minecraft:trapped_chest",
            _ => i == "minecraft:ender_pearl",
        }
    }
    fn tank(l: u8, i: &str) -> bool {
        if l == b'C' {
            i == "minecraft:cauldron"
        } else {
            chest(l, i)
        }
    }
    fn pouch(l: u8, i: &str) -> bool {
        match l {
            b'B' => i == "minecraft:blaze_powder",
            b'L' => i == "minecraft:leather",
            b'W' => WOOLS.contains(&i),
            _ => i == "minecraft:ender_pearl",
        }
    }
    type Test = fn(u8, &str) -> bool;
    let rules: [(&[u8; 9], Test, &'static str); 3] = [
        (b"BWBOCOBPB", chest, CHEST),
        (b"BWBOCOBPB", tank, TANK),
        (b"BLBLPLBWB", pouch, POUCH),
    ];
    for (pattern, test, result) in rules {
        if (0..9).all(|i| matches!(cells[i], Some(item) if test(pattern[i], item))) {
            return Some(result);
        }
    }
    None
}

}

/// The placeholder container of the open pouch (the vehicles' key: no reach check, no block).
const POUCH_KEY: (i32, i32, i32) = (i32::MAX - 15, super::minecarts::CART_KEY_Y, 0x4553_5031);
/// The sprite the chest item's model asks for (`{"type": "minecraft:chest", "texture": "enderstorage:ender_chest"}`).
const ITEM_SPRITE: &str = "entity/chest/enderstorage:ender_chest";

static STORE: Mutex<Option<lib::Store<ItemStack>>> = Mutex::new(None);
/// The copies that `reconcile` has seen (dimension, position; the pouch is `("pouch", [0; 3])`).
static SEEN: Mutex<Option<HashSet<(String, [i32; 3])>>> = Mutex::new(None);
/// The frequency of the open pouch.
static POUCH: Mutex<Option<lib::Frequency>> = Mutex::new(None);

fn with_store<R>(f: impl FnOnce(&mut lib::Store<ItemStack>) -> R) -> R {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Default::default))
}
fn with_seen<R>(f: impl FnOnce(&mut HashSet<(String, [i32; 3])>) -> R) -> R {
    let mut guard = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashSet::new))
}
fn key_of(p: [i32; 3]) -> (i32, i32, i32) {
    (p[0], p[1], p[2])
}
fn pouch_seen_key() -> (String, [i32; 3]) {
    ("pouch".to_owned(), [0; 3])
}

// ---- the world file ----

fn kind_text(kind: lib::Kind) -> &'static str {
    match kind {
        lib::Kind::Chest => "chest",
        lib::Kind::Tank => "tank",
    }
}
fn store_to_tag(s: &lib::Store<ItemStack>) -> Tag {
    let items = s
        .items
        .iter()
        .map(|(key, st)| {
            Tag::compound([
                ("key", Tag::String(key.as_str().into())),
                ("size", Tag::Int(st.size as i32)),
                ("Items", horizon_client::save::encode_items(&st.items)),
            ])
        })
        .collect();
    let liquids = s
        .liquids
        .iter()
        .filter(|(_, l)| l.fluid.is_some() && l.amount > 0)
        .map(|(key, l)| {
            Tag::compound([
                ("key", Tag::String(key.as_str().into())),
                ("fluid", Tag::String(l.fluid.clone().unwrap_or_default().as_str().into())),
                ("amount", Tag::Int(l.amount as i32)),
            ])
        })
        .collect();
    let blocks = s
        .blocks
        .iter()
        .map(|((dim, p), (kind, freq))| {
            Tag::compound([
                ("dim", Tag::String(dim.as_str().into())),
                ("x", Tag::Int(p[0])),
                ("y", Tag::Int(p[1])),
                ("z", Tag::Int(p[2])),
                ("kind", Tag::String(kind_text(*kind).into())),
                ("Frequency", lib::frequency_tag(freq)),
            ])
        })
        .collect();
    Tag::compound([
        ("items", Tag::List(items)),
        ("liquids", Tag::List(liquids)),
        ("blocks", Tag::List(blocks)),
    ])
}
fn store_from_tag(tag: &Tag) -> lib::Store<ItemStack> {
    use horizon_client::save::{number, text};
    let mut s = lib::Store::<ItemStack>::default();
    let list = |k: &str| match tag.get(k) {
        Some(Tag::List(l)) => l.clone(),
        _ => Vec::new(),
    };
    for e in list("items") {
        let (Some(key), Some(size)) = (text(e.get("key")), number(e.get("size"))) else {
            continue;
        };
        let size = size as i32;
        let slots = lib::SIZES[usize::try_from(size).ok().filter(|s| *s <= 2).unwrap_or(lib::ITEM_STORAGE_SIZE)];
        let items = horizon_client::save::decode_items(e.get("Items"), slots);
        s.items.insert(
            key,
            lib::ItemStorage::from_saved(size, items, lib::ITEM_STORAGE_SIZE),
        );
    }
    for e in list("liquids") {
        let (Some(key), Some(fluid), Some(amount)) =
            (text(e.get("key")), text(e.get("fluid")), number(e.get("amount")))
        else {
            continue;
        };
        s.liquids.insert(
            key,
            lib::Liquid {
                fluid: Some(fluid),
                amount: (amount as u32).min(lib::TANK_CAPACITY),
            },
        );
    }
    for e in list("blocks") {
        let (Some(dim), Some(x), Some(y), Some(z), Some(kind)) = (
            text(e.get("dim")),
            number(e.get("x")),
            number(e.get("y")),
            number(e.get("z")),
            text(e.get("kind")),
        ) else {
            continue;
        };
        let kind = match kind.as_str() {
            "chest" => lib::Kind::Chest,
            "tank" => lib::Kind::Tank,
            _ => continue,
        };
        let freq = e.get("Frequency").map(lib::frequency_from_tag).unwrap_or_default();
        s.blocks.insert((dim, [x as i32, y as i32, z as i32]), (kind, freq));
    }
    s
}

impl Game<'_> {
    fn es_dim(&self) -> String {
        self.dim.id().to_owned()
    }
    fn es_kind_at(&self, p: [i32; 3]) -> Option<lib::Kind> {
        let name = &self.res.facts().get(self.world.block(p[0], p[1], p[2])).name;
        lib::Kind::of(name)
    }
    fn es_prop(&self, p: [i32; 3], key: &str) -> Option<String> {
        let state = self.world.block(p[0], p[1], p[2]);
        self.pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get(key).cloned())
    }
    fn es_rotation(&self, p: [i32; 3]) -> u8 {
        lib::rotation_of_facing(self.es_prop(p, "facing").as_deref().unwrap_or("north"))
    }
    fn es_sneaking(&self) -> bool {
        self.keys.contains(&KeyCode::ShiftLeft)
    }
    fn es_owner(&mut self) -> lib::Owner {
        lib::Owner {
            uuid: self.acting_player_uuid(),
            name: self.acting_player_name().to_owned(),
        }
    }
    /// The frequency of the block at `p` (a block that came from `/setblock` or an old save has
    /// the default one, white three times).
    fn es_known(&mut self, p: [i32; 3], kind: lib::Kind) -> lib::Frequency {
        let dim = self.es_dim();
        with_store(|s| match s.block(&dim, p) {
            Some((k, f)) if *k == kind => f.clone(),
            _ => {
                let f = lib::Frequency::default();
                s.set_block(&dim, p, kind, f.clone());
                f
            }
        })
    }
    /// A new frequency for the block at `p`: its copy takes the new inventory.
    fn es_set_freq(&mut self, p: [i32; 3], kind: lib::Kind, freq: lib::Frequency) {
        let dim = self.es_dim();
        with_store(|s| {
            s.storage(&freq);
            s.set_block(&dim, p, kind, freq);
        });
        with_seen(|seen| seen.remove(&(dim, p)));
    }
    /// The block entity of the chest at `p`, made to the size of the inventory.
    fn es_ensure_be(&mut self, p: [i32; 3]) {
        let key = key_of(p);
        let size = lib::SIZES[lib::ITEM_STORAGE_SIZE];
        match self.world.block_entities.get_mut(&key) {
            Some(BlockEntity::Chest(items)) => {
                if items.len() < size {
                    items.resize(size, None);
                }
            }
            Some(_) => {}
            None => {
                self.world
                    .block_entities
                    .insert(key, BlockEntity::Chest(vec![None; size]));
                self.world.modified.insert((key.0 >> 4, key.2 >> 4));
            }
        }
    }
    fn es_say_give(&mut self, p: [i32; 3], stack: ItemStack) {
        if let Some(rest) = self.give(stack) {
            self.pop_resource(p, rest);
        }
    }
    fn es_sound(&mut self, p: [i32; 3], name: &str) {
        let pitch = (self.world.random.next_double() * 0.1 + 0.9) as f32;
        self.audio.play(
            name,
            Some([
                f64::from(p[0]) + 0.5,
                f64::from(p[1]) + 0.5,
                f64::from(p[2]) + 0.5,
            ]),
            1.0,
            pitch,
        );
    }
    /// Chests never join (`BlockEnderChest` has no `type`): a placed chest the game paired with a
    /// neighbour of its name goes back to single, and so does the neighbour.
    fn es_unpair(&mut self, p: [i32; 3]) {
        for d in [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]] {
            let q = [p[0] + d[0], p[1], p[2] + d[1]];
            if self.es_kind_at(q) != Some(lib::Kind::Chest) {
                continue;
            }
            if self.es_prop(q, "type").is_none_or(|t| t == "single") {
                continue;
            }
            let state = self.world.block(q[0], q[1], q[2]);
            if let Some(fixed) = self.with_props(state, &[("type", "single")]) {
                let at = BlockPos::new(q[0], q[1], q[2]);
                let flags =
                    level_flags::UPDATE_ALL | level_flags::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS;
                self.with_level(|b, level, ticks| b.set_block(level, ticks, at, fixed, flags));
                let _ = self.process_level_events();
            }
        }
    }

    // ---- placing (trigger_block "minecraft:placed_block", BlockItem.place's end) ----

    pub(super) fn enderstorage_placed(&mut self, trigger: &str, pos: [i32; 3], stack: &ItemStack) {
        if trigger != "minecraft:placed_block" {
            return;
        }
        let Some(kind) = self.es_kind_at(pos) else {
            return;
        };
        // ItemEnderStorage.place: the frequency of the item (`Frequency.readFromStack`).
        let freq = lib::read_frequency(stack);
        if kind == lib::Kind::Chest {
            self.es_unpair(pos);
            self.es_ensure_be(pos);
        }
        self.es_set_freq(pos, kind, freq);
    }

    // ---- clicks (BlockEnderStorage.use) ----

    /// A right click on an Ender Chest or Tank, before the block's own use. The buttons set the
    /// colours with dyes, the latch locks with the personal item (sneak to unlock); the rest of the
    /// tank's use is the bucket, the rest of the chest's the menu (`open_menu`).
    pub(super) fn enderstorage_use(&mut self, pos: [i32; 3], held: Option<&ItemStack>) -> bool {
        let Some(kind) = self.es_kind_at(pos) else {
            return false;
        };
        let Some(hit) = self.pick() else {
            return false;
        };
        if hit.pos != pos {
            return false;
        }
        let freq = self.es_known(pos, kind);
        let local = [
            hit.point[0] - f64::from(pos[0]),
            hit.point[1] - f64::from(pos[1]),
            hit.point[2] - f64::from(pos[2]),
        ];
        let rot = self.es_rotation(pos);
        let sub = match kind {
            lib::Kind::Chest => {
                let closed = self.lid_openers.get(&key_of(pos)).copied().unwrap_or(0) == 0;
                lib::chest_sub_hit(rot, hit.face, local, closed)
            }
            lib::Kind::Tank => lib::tank_sub_hit(rot, hit.face, local),
        };
        let sneaking = self.es_sneaking();
        let who = self.es_owner();
        match lib::use_block(&freq, sub, held.map(|h| &*h.item), sneaking, &who) {
            lib::Use::Recolour { freq, consume } | lib::Use::Lock { freq, consume } => {
                self.es_set_freq(pos, kind, freq);
                if consume && !self.creative {
                    self.consume_held();
                }
                self.swing_arm();
                return true;
            }
            lib::Use::Unlock { freq } => {
                // A full inventory fails the click.
                if !self.creative {
                    let back = ItemStack::new(lib::PERSONAL_ITEM, 1);
                    if self.give(back).is_some() {
                        return true;
                    }
                }
                self.es_set_freq(pos, kind, freq);
                self.swing_arm();
                return true;
            }
            lib::Use::Pass => {}
        }
        // `!player.isCrouching() && owner.activate(..)`
        if sneaking {
            return true;
        }
        match kind {
            lib::Kind::Chest => false,
            lib::Kind::Tank => {
                self.es_tank_use(pos, &freq, sub, held);
                true
            }
        }
    }
    /// `TileEnderTank.activate`: the dial flips the pressure (no pump here), else a bucket.
    fn es_tank_use(&mut self, pos: [i32; 3], freq: &lib::Frequency, sub: u8, held: Option<&ItemStack>) {
        if sub == 4 {
            return;
        }
        let Some(held) = held else {
            return;
        };
        let item = held.item.to_string();
        let result = with_store(|s| {
            let r = lib::bucket_interact(&item, s.liquid(freq));
            if r != lib::Bucket::None {
                s.dirty = true;
            }
            r
        });
        match result {
            lib::Bucket::None => {}
            lib::Bucket::Filled { bucket, sound } => {
                if !self.creative {
                    self.consume_held();
                    self.es_say_give(pos, ItemStack::new(bucket, 1));
                }
                self.es_sound(pos, sound);
                self.swing_arm();
            }
            lib::Bucket::Emptied { sound } => {
                if !self.creative {
                    self.consume_held();
                    self.es_say_give(pos, ItemStack::new(lib::EMPTY_BUCKET, 1));
                }
                self.es_sound(pos, sound);
                self.swing_arm();
            }
        }
    }

    /// `BlockEnderChest.use` after the frequency checks: the menu of the frequency's inventory
    /// (open_menu reaches `open_station` for a block `BlockEntity::for_block` does not know).
    /// The Ender Tank never opens a menu.
    pub(super) fn enderstorage_open(&mut self, pos: [i32; 3], name: &str) -> bool {
        if name == lib::TANK {
            return true;
        }
        if name != lib::CHEST {
            return false;
        }
        let key = key_of(pos);
        self.es_known(pos, lib::Kind::Chest);
        self.es_ensure_be(pos);
        // The copy shows the inventory as it is now.
        self.enderstorage_tick();
        self.container = Some(key);
        let first = self.container_opener_event(key, true);
        self.post_block_game_event(
            "minecraft:container_open",
            pos,
            Some(horizon_client::mobs::Who::Player),
            None,
        );
        if first {
            self.container_edge_sound(key, true);
        }
        true
    }
    /// The open chest or pouch menu: the game draws it as a vanilla ender chest (a generic 9-wide
    /// chest with the inventory's rows); its own title is `enderstorage_title`.
    pub(super) fn enderstorage_container_block(&self) -> Option<std::sync::Arc<str>> {
        let key = self.container?;
        let ours = key == POUCH_KEY
            || lib::Kind::of(&self.res.facts().get(self.world.block(key.0, key.1, key.2)).name)
                == Some(lib::Kind::Chest);
        ours.then(|| std::sync::Arc::from("minecraft:ender_chest"))
    }
    /// The title: the block's (or item's) description.
    pub(super) fn enderstorage_title(&self) -> Option<String> {
        let key = self.container?;
        if key == POUCH_KEY {
            return Some(self.translate("item.enderstorage.ender_pouch", "Ender Pouch"));
        }
        let name = &self.res.facts().get(self.world.block(key.0, key.1, key.2)).name;
        (lib::Kind::of(name) == Some(lib::Kind::Chest))
            .then(|| self.translate("block.enderstorage.ender_chest", "Ender Chest"))
    }

    // ---- the pouch (ItemEnderPouch) ----

    /// `onItemUseFirst` (sneaking at an Ender Chest copies its frequency) and `use` (opens the
    /// frequency's inventory), at the place where books are used: after the blocks reacted.
    pub(super) fn enderstorage_pouch(&mut self, stack: &ItemStack) -> bool {
        if &*stack.item != lib::POUCH {
            return false;
        }
        if self.es_sneaking() {
            let Some(hit) = self.pick() else {
                return false;
            };
            if self.es_kind_at(hit.pos) != Some(lib::Kind::Chest) {
                return false;
            }
            let freq = self.es_known(hit.pos, lib::Kind::Chest);
            if let Some(held) = self.held_mut() {
                lib::write_frequency(held, &freq);
            }
            self.swing_arm();
            return true;
        }
        let freq = lib::read_frequency(stack);
        if self.container.is_some() {
            return false;
        }
        let items = with_store(|s| s.storage(&freq).items.clone());
        self.world
            .block_entities
            .insert(POUCH_KEY, BlockEntity::Chest(items));
        with_seen(|seen| seen.remove(&pouch_seen_key()));
        *POUCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(freq);
        self.container = Some(POUCH_KEY);
        self.screen = Screen::Inventory;
        self.capture(false);
        self.right = false;
        true
    }
    /// The pouch's menu closes: its copy goes to the inventory before the placeholder goes.
    pub(super) fn enderstorage_closing(&mut self, key: (i32, i32, i32)) {
        if key != POUCH_KEY {
            return;
        }
        self.enderstorage_tick();
        *POUCH.lock().unwrap_or_else(|e| e.into_inner()) = None;
        with_seen(|seen| seen.remove(&pouch_seen_key()));
    }

    // ---- keeping the copies equal (TileEnderChest works on the inventory itself) ----

    pub(super) fn enderstorage_tick(&mut self) {
        let dim = self.es_dim();
        let blocks: Vec<([i32; 3], lib::Frequency)> = with_store(|s| {
            s.blocks
                .iter()
                .filter(|((d, _), (k, _))| *d == dim && *k == lib::Kind::Chest)
                .map(|((_, p), (_, f))| (*p, f.clone()))
                .collect()
        });
        let pouch = POUCH.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if blocks.is_empty() && pouch.is_none() {
            return;
        }
        let mut groups: BTreeMap<String, (lib::Frequency, Vec<[i32; 3]>)> = BTreeMap::new();
        for (p, f) in blocks {
            if matches!(self.world.block_entities.get(&key_of(p)), Some(BlockEntity::Chest(_))) {
                groups.entry(f.key()).or_insert_with(|| (f, Vec::new())).1.push(p);
            }
        }
        if let Some(f) = &pouch {
            groups.entry(f.key()).or_insert_with(|| (f.clone(), Vec::new()));
        }
        for (_, (freq, places)) in groups {
            // (key, seen-key, items, fresh)
            let mut taken: Vec<((i32, i32, i32), (String, [i32; 3]), Vec<Option<ItemStack>>, bool)> =
                Vec::new();
            let mut wanted: Vec<((i32, i32, i32), (String, [i32; 3]))> =
                places.iter().map(|p| (key_of(*p), (dim.clone(), *p))).collect();
            if pouch.as_ref().is_some_and(|f| f.key() == freq.key()) {
                wanted.push((POUCH_KEY, pouch_seen_key()));
            }
            for (key, seen_key) in wanted {
                match self.world.block_entities.remove(&key) {
                    Some(BlockEntity::Chest(items)) => {
                        let fresh = !with_seen(|seen| seen.contains(&seen_key));
                        taken.push((key, seen_key, items, fresh));
                    }
                    Some(other) => {
                        self.world.block_entities.insert(key, other);
                    }
                    None => {}
                }
            }
            let changed = {
                let mut works: Vec<lib::Working<'_, ItemStack>> = taken
                    .iter_mut()
                    .map(|(_, _, items, fresh)| lib::Working {
                        items,
                        fresh: *fresh,
                    })
                    .collect();
                with_store(|s| s.reconcile(&freq, &mut works))
            };
            for (key, seen_key, items, _) in taken {
                self.world.block_entities.insert(key, BlockEntity::Chest(items));
                if changed && key.1 != super::minecarts::CART_KEY_Y {
                    self.world.modified.insert((key.0 >> 4, key.2 >> 4));
                }
                with_seen(|seen| seen.insert(seen_key));
            }
        }
    }

    // ---- breaking (BlockEnderStorage.getDrops: the item keeps the frequency) ----

    pub(super) fn enderstorage_loot(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        value: Vec<ItemStack>,
    ) -> Vec<ItemStack> {
        let Some(kind) = lib::Kind::of(&self.res.facts().get(state).name) else {
            return value;
        };
        let dim = self.es_dim();
        let freq = with_store(|s| s.remove_block(&dim, pos))
            .map(|(_, f)| f)
            .unwrap_or_default();
        with_seen(|seen| seen.remove(&(dim, pos)));
        // `createItem`: anarchyMode takes the owner off.
        let mut freq = freq;
        if lib::ANARCHY_MODE {
            freq.owner = None;
        }
        let mut stack = ItemStack::new(kind.block(), 1);
        lib::write_frequency(&mut stack, &freq);
        let mut out = vec![stack];
        if lib::ANARCHY_MODE {
            out.push(ItemStack::new(lib::PERSONAL_ITEM, 1));
        }
        out
    }

    // ---- crafting (CreateRecipe, ReColourRecipe) ----

    pub(super) fn enderstorage_craft(&self) -> Option<(String, ItemStack)> {
        let n = self.craft_size;
        let grid: Vec<Option<ItemStack>> = (0..n * n).map(|i| self.craft_grid[i].clone()).collect();
        lib::crafting(&grid, n).map(|(name, stack)| (name.to_owned(), stack))
    }

    // ---- the world file ----

    pub(super) fn enderstorage_load(&mut self) {
        // The Ender Tank is a fluid handler for pipes and machines (foundry, enabled with this mod).
        crate::game::foundry::register_fluid("enderstorage", es_tank_claims, es_tank_fluid);
        let path = self.world_root.join("data/enderstorage/store.nbt");
        let loaded = std::fs::read(path)
            .ok()
            .and_then(|bytes| read_named(&bytes, Limits::default()).ok())
            .map(|(named, _)| store_from_tag(&named.tag))
            .unwrap_or_default();
        with_store(|s| *s = loaded);
        with_seen(|seen| seen.clear());
        *POUCH.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    pub(super) fn enderstorage_save(&mut self) {
        if self.world_root.as_os_str().is_empty() {
            return;
        }
        // Make the inventories current (a hopper may have run since the last tick).
        self.enderstorage_tick();
        let dim = self.es_dim();
        // Forget blocks that are gone: only in loaded chunks of this dimension.
        let gone: Vec<(String, [i32; 3])> = with_store(|s| {
            s.blocks
                .keys()
                .filter(|(d, p)| {
                    *d == dim
                        && self.world.chunks.get(&(p[0] >> 4, p[2] >> 4)).is_some()
                        && self.es_kind_at(*p).is_none()
                })
                .cloned()
                .collect()
        });
        let dir = self.world_root.join("data/enderstorage");
        let file = dir.join("store.nbt");
        let tag = with_store(|s| {
            for g in &gone {
                s.remove_block(&g.0, g.1);
            }
            if !s.dirty && file.is_file() {
                return None;
            }
            s.dirty = false;
            Some(store_to_tag(s))
        });
        let Some(tag) = tag else {
            return;
        };
        let root = NamedTag {
            name: JavaString::from(""),
            tag,
        };
        let Ok(bytes) = write_named(&root, Limits::default()) else {
            return;
        };
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let tmp = dir.join("store.nbt.tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &file);
        }
    }

    /// The chest item's model sprite, put in the GUI atlas's names after each (re)load of it.
    pub(super) fn enderstorage_sprites(&mut self) {
        let sprites = &mut self.gui.atlas.sprites;
        if sprites.contains_key(ITEM_SPRITE) {
            return;
        }
        if let Some(rect) = sprites.get(lib::CHEST_TEXTURE).copied() {
            sprites.insert(ITEM_SPRITE.to_owned(), rect);
        }
    }
}

// ---- the Ender Tank as a fluid handler (TileEnderTank.getCapability(FLUID_HANDLER), any side) ----

fn es_tank_claims(game: &Game<'_>, p: [i32; 3], _face: Option<crate::game::foundry::Dir>) -> bool {
    game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4)) && game.es_kind_at(p) == Some(lib::Kind::Tank)
}

/// The frequency's tank (`EnderLiquidStorage`, 16 buckets) as a foundry `FluidTank` for one operation;
/// what the operation changed is written back to the frequency.
fn es_tank_fluid(
    game: &mut Game<'_>,
    p: [i32; 3],
    _face: Option<crate::game::foundry::Dir>,
    f: &mut dyn FnMut(&mut dyn crate::game::foundry::FluidHandler),
) -> bool {
    use crate::game::foundry::{FluidStack, FluidTank};
    if !es_tank_claims(game, p, None) {
        return false;
    }
    let freq = game.es_known(p, lib::Kind::Tank);
    with_store(|s| {
        let before = s.liquid(&freq).clone();
        let mut tank = FluidTank::new(lib::TANK_CAPACITY as i32);
        if let Some(fluid) = &before.fluid
            && before.amount > 0
        {
            tank.fluid = FluidStack::new(fluid, before.amount as i32);
        }
        f(&mut tank);
        let after = if tank.fluid.is_empty() {
            lib::Liquid::default()
        } else {
            lib::Liquid {
                fluid: Some(tank.fluid.fluid.clone()),
                amount: tank.fluid.amount as u32,
            }
        };
        if after != before {
            *s.liquid(&freq) = after;
            s.dirty = true;
        }
    });
    true
}
