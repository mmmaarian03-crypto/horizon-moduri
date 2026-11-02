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

// ---- the part that needs the game library's types (this file follows src/lib.rs in shared.rs) ----

use crate::items::ItemStack;
use horizon_world::nbt::{JavaString, Tag};
use std::sync::RwLock;

const CUSTOM_DATA: &str = "minecraft:custom_data";
const MODEL_DATA: &str = "minecraft:custom_model_data";

/// A plain `Component.Serializer.toJson(Component.literal(name))`.
fn name_json(name: &str) -> String {
    let mut s = String::from("\"");
    for c in name.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}
/// The text of a component's JSON, when it is a string or has a `text`.
fn name_of_json(json: &str) -> String {
    fn unquote(s: &str) -> Option<String> {
        let s = s.trim().strip_prefix('"')?;
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    'u' => {
                        let hex: String = chars.by_ref().take(4).collect();
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    c => out.push(c),
                },
                c => out.push(c),
            }
        }
        None
    }
    if let Some(name) = unquote(json) {
        return name;
    }
    if let Some(at) = json.find("\"text\"")
        && let Some(colon) = json[at..].find(':')
        && let Some(name) = unquote(&json[at + colon + 1..])
    {
        return name;
    }
    json.to_owned()
}

/// `Frequency.write_internal`: the colours as wool metas, the owner as a UUID and the player's name.
pub fn frequency_tag(f: &Frequency) -> Tag {
    let mut entries = vec![
        ("left", Tag::Int(f.left.wool_meta())),
        ("middle", Tag::Int(f.middle.wool_meta())),
        ("right", Tag::Int(f.right.wool_meta())),
    ];
    if let Some(o) = &f.owner {
        entries.push(("owner", Tag::IntArray(o.uuid.to_vec())));
        entries.push(("owner_name", Tag::String(name_json(&o.name).as_str().into())));
    }
    Tag::compound(entries)
}

/// `Frequency.read_internal` (a colour out of range is white, as a missing one is).
pub fn frequency_from_tag(tag: &Tag) -> Frequency {
    let colour = |key: &str| {
        match tag.get(key) {
            Some(Tag::Int(i)) => Colour::from_wool_meta(*i),
            Some(Tag::Byte(i)) => Colour::from_wool_meta(i32::from(*i)),
            Some(Tag::Short(i)) => Colour::from_wool_meta(i32::from(*i)),
            _ => None,
        }
        .unwrap_or(Colour::White)
    };
    let mut f = Frequency::new(colour("left"), colour("middle"), colour("right"));
    if let Some(Tag::IntArray(uuid)) = tag.get("owner")
        && let [a, b, c, d] = uuid[..]
    {
        // `hasOwner` needs both the UUID and the name.
        if let Some(Tag::String(name)) = tag.get("owner_name") {
            f.owner = Some(Owner {
                uuid: [a, b, c, d],
                name: name_of_json(&name.to_string_lossy()),
            });
        }
    }
    f
}

/// `Frequency.readFromStack`: the stack's `Frequency` in its custom data (the default when none).
pub fn read_frequency(stack: &ItemStack) -> Frequency {
    stack
        .data()
        .and_then(|d| d.other(CUSTOM_DATA))
        .and_then(|t| t.get("Frequency"))
        .map(frequency_from_tag)
        .unwrap_or_default()
}

/// `Frequency.writeToStack`; the pouch also gets the custom model data its item model reads
/// (strings: left, middle, right colour; flags: owned, open).
pub fn write_frequency(stack: &mut ItemStack, f: &Frequency) {
    let mut custom = match stack.data().and_then(|d| d.other(CUSTOM_DATA)) {
        Some(Tag::Compound(map)) => map.clone(),
        _ => Default::default(),
    };
    custom.insert(JavaString::from("Frequency"), frequency_tag(f));
    let pouch = &*stack.item == POUCH;
    let data = stack.data_mut();
    data.set_other(CUSTOM_DATA, Some(Tag::Compound(custom)));
    if pouch {
        let strings = f
            .model_strings()
            .iter()
            .map(|s| Tag::String((*s).into()))
            .collect();
        data.set_other(
            MODEL_DATA,
            Some(Tag::compound([
                ("strings", Tag::List(strings)),
                (
                    "flags",
                    Tag::List(vec![Tag::Byte(i8::from(f.has_owner())), Tag::Byte(0)]),
                ),
            ])),
        );
    }
}

/// The result of the mod's crafting recipes for a grid, when it is one of them: the three
/// `create_recipe`s (the colour of the wool) and the three `recolour_recipe`s (dyes above the item).
/// Called first by the recipe search; `None` leaves it to the game's own.
pub fn crafting(grid: &[Option<ItemStack>], size: usize) -> Option<(&'static str, ItemStack)> {
    if size != 3 || grid.len() != 9 {
        return None;
    }
    let cells: Vec<Option<&str>> = grid.iter().map(|c| c.as_ref().map(|s| &*s.item)).collect();
    if let Some(item) = create_recipe(&cells) {
        let mut stack = ItemStack::new(item, 1);
        write_frequency(&mut stack, &create_frequency(&cells));
        return Some((
            match item {
                CHEST => "enderstorage:ender_chest",
                TANK => "enderstorage:ender_tank",
                _ => "enderstorage:ender_pouch",
            },
            stack,
        ));
    }
    for (item, name) in [
        (CHEST, "enderstorage:recolour_ender_chest"),
        (TANK, "enderstorage:recolour_ender_tank"),
        (POUCH, "enderstorage:recolour_ender_pouch"),
    ] {
        if recolour_matches(&cells, item) {
            let freq = recolour_result(&cells, item, |i| {
                grid[i].as_ref().map(read_frequency).unwrap_or_default()
            });
            let mut stack = ItemStack::new(item, 1);
            write_frequency(&mut stack, &freq);
            return Some((name, stack));
        }
    }
    None
}

/// `appendHoverText`: the three colours (the owner's name goes after them: the game's tooltip
/// hook gives one line).
pub fn tooltip(stack: &ItemStack, translate: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    if !matches!(&*stack.item, CHEST | TANK | POUCH) {
        return None;
    }
    let f = read_frequency(stack);
    let [a, b, c] = f.tooltip_keys().map(|k| translate(&k).unwrap_or(k));
    Some(match &f.owner {
        Some(o) => format!("{a}/{b}/{c} ({})", o.name),
        None => format!("{a}/{b}/{c}"),
    })
}

// ---- the chest as the block entity renderer and the lid animation know it ----

/// `RenderTileEnderChest`'s texture (`textures/enderchest.png`, the vanilla chest's layout), where the
/// carried content keeps it so that the GUI atlas packs it.
pub const CHEST_TEXTURE: &str = "enderstorage:block/model/ender_chest";
/// The facings in the order of the renderer's y rotation (south 0, west 90, north 180, east 270).
pub const FACINGS: [&str; 4] = ["south", "west", "north", "east"];

/// Each block state's code: 0 not the chest, else 1 + the facing's index.
static STATES: RwLock<Vec<u8>> = RwLock::new(Vec::new());

pub fn state_code(name: &str, facing: Option<&str>) -> u8 {
    if name != CHEST {
        return 0;
    }
    let f = FACINGS.iter().position(|x| Some(*x) == facing).unwrap_or(2);
    1 + f as u8
}
pub fn set_states(codes: Vec<u8>) {
    if let Ok(mut s) = STATES.write() {
        *s = codes;
    }
}
/// The facing's index when the state is the chest.
pub fn chest_state(state: u32) -> Option<usize> {
    let s = STATES.read().ok()?;
    match s.get(state as usize).copied() {
        None | Some(0) => None,
        Some(c) => Some(usize::from(c) - 1),
    }
}
