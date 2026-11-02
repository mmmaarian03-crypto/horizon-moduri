// Iron Chests for Horizon: the rules of the chests, without any game code.
// Written from Iron Chests 1.20.1-14.4.4 (progwml6, GPL-3.0), the jar the pack plays; its
// source is commit d5bc625 of the 1.20.x branch (surse/ironchest): IronChestsTypes,
// IronChestsBlocks, IronChestMenu, DirtChestSlot, ChestUpgradeItem, IronChestsUpgradeType,
// ICrystalChest, IronChestRenderer, DirtChestBlockEntity, Util.
//
// The game side (native/horizon.rs) keeps a chest's items in Horizon's own
// `BlockEntity::Chest(Vec<..>)` of the tier's size; everything here works on the
// `World` trait so the tests can run it on a fake world.

pub type Pos = [i32; 3];

/// `IronChestsTypes` without WOOD (WOOD only names the vanilla chest in upgrades).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    Iron,
    Gold,
    Diamond,
    Copper,
    Crystal,
    Obsidian,
    Dirt,
}

/// The menu screen of a tier (`IronChestScreen`): image size and its texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gui {
    /// `xSize`, `ySize`: the part of the texture drawn, GUI pixels.
    pub width: i32,
    pub height: i32,
    /// The texture as the game's GUI atlas names it (a copy of
    /// `ironchest:textures/gui/<name>.png` under `textures/block/gui/`).
    pub sprite: &'static str,
    /// `textureXSize`, `textureYSize`.
    pub texture_width: i32,
    pub texture_height: i32,
}

impl Tier {
    pub const ALL: [Tier; 7] = [
        Tier::Iron,
        Tier::Gold,
        Tier::Diamond,
        Tier::Copper,
        Tier::Crystal,
        Tier::Obsidian,
        Tier::Dirt,
    ];
    /// `IronChestsTypes.getId`.
    pub fn id(self) -> &'static str {
        match self {
            Tier::Iron => "iron",
            Tier::Gold => "gold",
            Tier::Diamond => "diamond",
            Tier::Copper => "copper",
            Tier::Crystal => "crystal",
            Tier::Obsidian => "obsidian",
            Tier::Dirt => "dirt",
        }
    }
    /// `IronChestsTypes.size`: IRON 54, GOLD 81, DIAMOND 108, COPPER 45, CRYSTAL 108,
    /// OBSIDIAN 108, DIRT 1.
    pub fn size(self) -> usize {
        match self {
            Tier::Iron => 54,
            Tier::Gold => 81,
            Tier::Diamond | Tier::Crystal | Tier::Obsidian => 108,
            Tier::Copper => 45,
            Tier::Dirt => 1,
        }
    }
    /// `IronChestsTypes.rowLength`: 12 for diamond, crystal and obsidian, 1 for dirt, else 9.
    pub fn row_length(self) -> usize {
        match self {
            Tier::Diamond | Tier::Crystal | Tier::Obsidian => 12,
            Tier::Dirt => 1,
            _ => 9,
        }
    }
    /// `IronChestsTypes.getRowCount`.
    pub fn rows(self) -> usize {
        self.size() / self.row_length()
    }
    /// `IronChestsTypes`' xSize, ySize, guiTexture, textureXSize and textureYSize.
    pub fn gui(self) -> Gui {
        let (width, height, sprite, texture_height) = match self {
            Tier::Iron => (184, 222, "ironchest:block/gui/iron_container", 256),
            Tier::Gold => (184, 276, "ironchest:block/gui/gold_container", 276),
            // CRYSTAL and OBSIDIAN draw the diamond chest's screen.
            Tier::Diamond | Tier::Crystal | Tier::Obsidian => {
                (238, 276, "ironchest:block/gui/diamond_container", 276)
            }
            Tier::Copper => (184, 204, "ironchest:block/gui/copper_container", 256),
            Tier::Dirt => (184, 184, "ironchest:block/gui/dirt_container", 256),
        };
        Gui {
            width,
            height,
            sprite,
            texture_width: 256,
            texture_height,
        }
    }
    /// `IronChestsTypes.isTransparent`: the crystal chest shows its top items.
    pub fn transparent(self) -> bool {
        self == Tier::Crystal
    }
}

/// One of the fourteen chest blocks: a tier, plain or trapped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chest {
    pub tier: Tier,
    pub trapped: bool,
}

impl Chest {
    /// All the blocks, in `IronChestsBlocks`' order: the seven plain ones, then the trapped.
    pub fn all() -> impl Iterator<Item = Chest> {
        [false, true]
            .into_iter()
            .flat_map(|trapped| Tier::ALL.into_iter().map(move |tier| Chest { tier, trapped }))
    }
    /// The block of `ironchest:<tier>_chest` or `ironchest:trapped_<tier>_chest`.
    pub fn parse(name: &str) -> Option<Chest> {
        let path = name.strip_prefix("ironchest:")?;
        let (trapped, rest) = match path.strip_prefix("trapped_") {
            Some(rest) => (true, rest),
            None => (false, path),
        };
        let id = rest.strip_suffix("_chest")?;
        let tier = Tier::ALL.into_iter().find(|t| t.id() == id)?;
        Some(Chest { tier, trapped })
    }
    /// The block id; also the block entity type id (`IronChestsBlockEntityTypes`).
    pub fn name(self) -> String {
        let trapped = if self.trapped { "trapped_" } else { "" };
        format!("ironchest:{trapped}{}_chest", self.tier.id())
    }
    /// The block-entity model texture (`IronChestsModels.chooseChestTexture`): a copy of
    /// `ironchest:textures/model/<block>.png` under `textures/block/model/`, which the
    /// game's GUI atlas packs (its `block` directory source).
    pub fn texture(self) -> String {
        let trapped = if self.trapped { "trapped_" } else { "" };
        format!("ironchest:block/model/{trapped}{}_chest", self.tier.id())
    }
    /// The sprite the chest's item model asks for: `{"type": "minecraft:chest",
    /// "texture": "ironchest:<block>"}` reads `entity/chest/<texture>`, which the
    /// game module maps onto [`Chest::texture`].
    pub fn item_sprite(self) -> String {
        format!("entity/chest/{}", self.name())
    }
    /// `AbstractIronChestBlockEntity.getDefaultName`: plain and trapped share it.
    pub fn title_key(self) -> String {
        format!("ironchest.container.{}_chest", self.tier.id())
    }
    /// The English name (en_us.json), when the key is missing.
    pub fn title_fallback(self) -> &'static str {
        match self.tier {
            Tier::Iron => "Iron Chest",
            Tier::Gold => "Gold Chest",
            Tier::Diamond => "Diamond Chest",
            Tier::Copper => "Copper Chest",
            Tier::Crystal => "Crystal Chest",
            Tier::Obsidian => "Obsidian Chest",
            Tier::Dirt => "DirtChest 9000!",
        }
    }
    /// `AbstractIronChestBlock.getOpenChestStat`: OPEN_CHEST, TRIGGER_TRAPPED_CHEST when trapped.
    pub fn stat(self) -> &'static str {
        if self.trapped {
            "trigger_trapped_chest"
        } else {
            "open_chest"
        }
    }
}

/// The chest's slot count for a block id, if it is one of the chests.
pub fn size_of(name: &str) -> Option<usize> {
    Chest::parse(name).map(|c| c.tier.size())
}

/// `IronChestsUpgradeType`: source and target tiers (None is WOOD, the vanilla chest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    IronToGold,
    GoldToDiamond,
    CopperToIron,
    DiamondToCrystal,
    WoodToIron,
    WoodToCopper,
    DiamondToObsidian,
}

impl Upgrade {
    pub const ALL: [Upgrade; 7] = [
        Upgrade::IronToGold,
        Upgrade::GoldToDiamond,
        Upgrade::CopperToIron,
        Upgrade::DiamondToCrystal,
        Upgrade::WoodToIron,
        Upgrade::WoodToCopper,
        Upgrade::DiamondToObsidian,
    ];
    pub fn source(self) -> Option<Tier> {
        match self {
            Upgrade::IronToGold => Some(Tier::Iron),
            Upgrade::GoldToDiamond => Some(Tier::Gold),
            Upgrade::CopperToIron => Some(Tier::Copper),
            Upgrade::DiamondToCrystal | Upgrade::DiamondToObsidian => Some(Tier::Diamond),
            Upgrade::WoodToIron | Upgrade::WoodToCopper => None,
        }
    }
    pub fn target(self) -> Tier {
        match self {
            Upgrade::IronToGold => Tier::Gold,
            Upgrade::GoldToDiamond => Tier::Diamond,
            Upgrade::CopperToIron | Upgrade::WoodToIron => Tier::Iron,
            Upgrade::DiamondToCrystal => Tier::Crystal,
            Upgrade::WoodToCopper => Tier::Copper,
            Upgrade::DiamondToObsidian => Tier::Obsidian,
        }
    }
    /// `IronChestsItems`: `name().toLowerCase() + "_chest_upgrade"`.
    pub fn item(self) -> String {
        let source = self.source().map_or("wood", Tier::id);
        format!("ironchest:{source}_to_{}_chest_upgrade", self.target().id())
    }
    pub fn parse(item: &str) -> Option<Upgrade> {
        Upgrade::ALL.into_iter().find(|u| u.item() == item)
    }
}

/// A horizontal facing (`HorizontalDirectionalBlock.FACING`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    North,
    South,
    West,
    East,
}
impl Facing {
    pub fn parse(s: &str) -> Facing {
        match s {
            "south" => Facing::South,
            "west" => Facing::West,
            "east" => Facing::East,
            _ => Facing::North,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Facing::North => "north",
            Facing::South => "south",
            Facing::West => "west",
            Facing::East => "east",
        }
    }
    /// `Direction.getClockWise`.
    pub fn clockwise(self) -> Facing {
        match self {
            Facing::North => Facing::East,
            Facing::East => Facing::South,
            Facing::South => Facing::West,
            Facing::West => Facing::North,
        }
    }
    pub fn counter_clockwise(self) -> Facing {
        self.clockwise().clockwise().clockwise()
    }
    pub fn step(self) -> Pos {
        match self {
            Facing::North => [0, 0, -1],
            Facing::South => [0, 0, 1],
            Facing::West => [-1, 0, 0],
            Facing::East => [1, 0, 0],
        }
    }
}

/// The `type` a chest-shaped block state has (the vanilla chest template's double-chest
/// halves). Iron chests are always single (`AbstractIronChestBlock.getBlockType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    Single,
    Left,
    Right,
}
impl Half {
    pub fn parse(s: &str) -> Half {
        match s {
            "left" => Half::Left,
            "right" => Half::Right,
            _ => Half::Single,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Half::Single => "single",
            Half::Left => "left",
            Half::Right => "right",
        }
    }
}

/// What stands at a position, as far as the chests care.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Iron {
        chest: Chest,
        facing: Facing,
        half: Half,
        waterlogged: bool,
    },
    /// `minecraft:chest` or `minecraft:trapped_chest` (`ChestBlock` in 1.20.1).
    Vanilla { trapped: bool, facing: Facing },
    Other,
}

/// An item stack, as the rules need it.
pub trait Stack: Clone {
    fn item(&self) -> &str;
    fn count(&self) -> u32;
    fn set_count(&mut self, count: u32);
    /// `ItemStack.isSameItemSameTags` (same item, same components).
    fn same_kind(&self, other: &Self) -> bool;
}

/// The level, as the rules see it. Items are a chest block entity's slots.
pub trait World {
    type Stack: Stack;
    fn block(&self, p: Pos) -> Block;
    /// Puts one of the chests at `p` with block-entity side effects skipped:
    /// whatever block entity was there must be taken first.
    fn set_chest(&mut self, p: Pos, chest: Chest, facing: Facing, half: Half, waterlogged: bool);
    /// A chest-like block entity's slots (None: there is none).
    fn items(&self, p: Pos) -> Option<Vec<Option<Self::Stack>>>;
    fn take_items(&mut self, p: Pos) -> Option<Vec<Option<Self::Stack>>>;
    fn put_items(&mut self, p: Pos, items: Vec<Option<Self::Stack>>);
    fn custom_name(&self, p: Pos) -> Option<String>;
    fn set_custom_name(&mut self, p: Pos, name: Option<String>);
    /// `ContainerOpenersCounter.getOpenerCount`: players with it open.
    fn openers(&self, p: Pos) -> i32;
}

fn offset(p: Pos, d: Pos) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// `items` with at least `size` slots, in place: slots are only ever added, so
/// nothing a smaller block entity held is lost.
pub fn grow<S>(items: &mut Vec<Option<S>>, size: usize) -> bool {
    if items.len() >= size {
        return false;
    }
    items.resize_with(size, || None);
    true
}

/// The chest at `p` has its block entity with all its tier's slots: made empty when
/// missing, widened when smaller (a vanilla path made a 27-slot one for
/// `ironchest:copper_chest`, whose name ends like the vanilla copper chests').
/// Returns whether it was missing.
pub fn ensure_entity<W: World>(w: &mut W, p: Pos) -> Option<bool> {
    let Block::Iron { chest, .. } = w.block(p) else {
        return None;
    };
    let size = chest.tier.size();
    match w.items(p) {
        None => {
            w.put_items(p, vec![None; size]);
            Some(true)
        }
        Some(mut items) => {
            if grow(&mut items, size) {
                w.put_items(p, items);
            }
            Some(false)
        }
    }
}

/// A chest was placed at `p`. The chest template pairs two equal chests side by side
/// into a double chest (`ChestBlock.getStateForPlacement`); iron chests never pair
/// (`getBlockType` is always SINGLE), so both halves go back to single. Then the block
/// entity (see [`ensure_entity`]). Returns whether the block entity was new.
pub fn placed<W: World>(w: &mut W, p: Pos) -> Option<bool> {
    let Block::Iron {
        chest,
        facing,
        half,
        waterlogged,
    } = w.block(p)
    else {
        return None;
    };
    if half != Half::Single {
        w.set_chest(p, chest, facing, Half::Single, waterlogged);
        // ChestBlock.getConnectedDirection: left is clockwise of the facing, right counter-clockwise.
        let toward = if half == Half::Left {
            facing.clockwise()
        } else {
            facing.counter_clockwise()
        };
        let other = offset(p, toward.step());
        if let Block::Iron {
            chest: c,
            facing: f,
            half: h,
            waterlogged: wl,
        } = w.block(other)
            && c == chest
            && h != Half::Single
        {
            w.set_chest(other, c, f, Half::Single, wl);
        }
    }
    ensure_entity(w, p)
}

/// `DirtChestBlockEntity.wasPlaced`: a new DirtChest 9000 holds its guide book, unless
/// the item says it was placed before (`ChestPlacedAlready` in its block entity data).
/// Unlike the original (`setItem(0, book)`), a slot that already holds something keeps it.
pub fn give_guide_book<W: World>(w: &mut W, p: Pos, placed_before: bool, book: W::Stack) -> bool {
    let Block::Iron { chest, .. } = w.block(p) else {
        return false;
    };
    if chest.tier != Tier::Dirt || placed_before {
        return false;
    }
    let mut items = w.items(p).unwrap_or_else(|| vec![None]);
    grow(&mut items, 1);
    if items[0].is_some() {
        return false;
    }
    items[0] = Some(book);
    w.put_items(p, items);
    true
}

/// `DirtChestBlockEntity.removeAdornments`, before the chest spills: the guide book in
/// the slot goes. The original takes any written book there (`isSameItem`); here only
/// the guide itself (`is_guide`), so no book of a player's is lost.
pub fn strip_guide_book<W: World>(w: &mut W, p: Pos, is_guide: impl Fn(&W::Stack) -> bool) -> bool {
    let Block::Iron { chest, .. } = w.block(p) else {
        return false;
    };
    if chest.tier != Tier::Dirt {
        return false;
    }
    let Some(mut items) = w.items(p) else {
        return false;
    };
    match items.first() {
        Some(Some(s)) if is_guide(s) => {
            items[0] = None;
            w.put_items(p, items);
            true
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeOutcome {
    /// Not a chest this upgrade takes, or someone has it open: the click goes on.
    Pass,
    /// The chest at the position is now `to`, with its items and name.
    Upgraded { to: Chest },
}

/// `ChestUpgradeItem.onItemUseFirst` on the block at `p`.
pub fn upgrade<W: World>(w: &mut W, p: Pos, up: Upgrade) -> UpgradeOutcome {
    let (trapped, facing) = match (w.block(p), up.source()) {
        // canUpgrade(WOOD): a ChestBlock with a ChestBlockEntity.
        (Block::Vanilla { trapped, facing }, None) => (trapped, facing),
        (Block::Iron { chest, facing, .. }, Some(source)) if chest.tier == source => {
            (chest.trapped, facing)
        }
        _ => return UpgradeOutcome::Pass,
    };
    // getOpenCount(...) > 0: PASS.
    if w.openers(p) > 0 {
        return UpgradeOutcome::Pass;
    }
    let to = Chest {
        tier: up.target(),
        trapped,
    };
    // A chest without its block entity: newChest stays null, PASS.
    let Some(items) = w.items(p) else {
        return UpgradeOutcome::Pass;
    };
    // setItems copies what fits; every upgrade is to as many slots or more, and a
    // chest holding more than the target would lose items, so it is left alone.
    if items.len() > to.tier.size() && items[to.tier.size()..].iter().any(Option::is_some) {
        return UpgradeOutcome::Pass;
    }
    let name = w.custom_name(p);
    let mut items = w.take_items(p).unwrap_or(items);
    items.truncate(to.tier.size());
    grow(&mut items, to.tier.size());
    // defaultBlockState().setValue(FACING, chestFacing): not waterlogged.
    w.set_chest(p, to, facing, Half::Single, false);
    w.put_items(p, items);
    w.set_custom_name(p, name);
    UpgradeOutcome::Upgraded { to }
}

// ---- the menu (IronChestMenu) ----

/// The game's slot ids: 0..9 the hotbar, 9..36 the inventory, 300.. the chest.
pub const CONTAINER_SLOT: usize = 300;

/// The rows of the engine's chest menu (`menu::Kind::Chest`, nine to a row) that holds
/// the chest's slots in order. The DirtChest 9000 takes a one-row menu whose slots 1..9
/// stand for nothing (see [`dirt_settle`]).
pub fn menu_rows(tier: Tier) -> usize {
    tier.size().div_ceil(9)
}
/// The menu slots before the player's: the chest's own.
pub fn menu_container(tier: Tier) -> usize {
    menu_rows(tier) * 9
}
/// Game slot to menu slot: the chest, then the inventory (9..36), then the hotbar
/// (`IronChestMenu`'s order).
pub fn to_menu(tier: Tier, game: usize) -> Option<usize> {
    let n = menu_container(tier);
    match game {
        g if (CONTAINER_SLOT..CONTAINER_SLOT + tier.size()).contains(&g) => Some(g - CONTAINER_SLOT),
        9..36 => Some(n + game - 9),
        0..9 => Some(n + 27 + game),
        _ => None,
    }
}
/// Menu slot to game slot; None for the dirt chest's stand-in slots.
pub fn to_game(tier: Tier, menu: usize) -> Option<usize> {
    let n = menu_container(tier);
    match menu {
        i if i < n => (i < tier.size()).then_some(CONTAINER_SLOT + i),
        i if i < n + 27 => Some(9 + i - n),
        i if i < n + 36 => Some(i - n - 27),
        _ => None,
    }
}

/// Where a slot sits in the screen (its 16x16 item's top left, GUI pixels from the
/// image's corner), as `IronChestMenu` lays them out.
pub fn slot_position(tier: Tier, game: usize) -> Option<(i32, i32)> {
    let gui = tier.gui();
    // (xSize - 162) / 2 + 1, integer division.
    let left = (gui.width - 162) / 2 + 1;
    match game {
        0..9 => Some((left + game as i32 * 18, gui.height - 24)),
        9..36 => {
            let i = (game - 9) as i32;
            let (row, col) = (i / 9, i % 9);
            Some((left + col * 18, gui.height - (4 - row) * 18 - 10))
        }
        g if (CONTAINER_SLOT..CONTAINER_SLOT + tier.size()).contains(&g) => {
            if tier == Tier::Dirt {
                // new DirtChestSlot(inventory, 0, 12 + 4 * 18, 8 + 2 * 18)
                return Some((12 + 4 * 18, 8 + 2 * 18));
            }
            let i = g - CONTAINER_SLOT;
            let (row, col) = ((i / tier.row_length()) as i32, (i % tier.row_length()) as i32);
            Some((12 + col * 18, 18 + row * 18))
        }
        _ => None,
    }
}
/// `IronChestScreen.renderLabels`: the title at (8, 6), "Inventory" at (8, ySize - 96 + 2).
pub fn label_positions(tier: Tier) -> [(i32, i32); 2] {
    [(8, 6), (8, tier.gui().height - 96 + 2)]
}
/// The labels' colour, 4210752.
pub const LABEL_COLOR: u32 = 0x404040;

/// `DirtChestSlot.mayPlace`: only dirt (`Item.byBlock(Blocks.DIRT)`).
pub fn dirt_accepts(item: &str) -> bool {
    item == "minecraft:dirt"
}

/// The dirt chest's menu after a click, run by the engine on a one-row chest menu:
/// `before`/`after` are its slots (0 the chest's, 1..9 stand-ins, then the player's).
/// Some(slots) keeps them, None drops the whole click (nothing changes, as when
/// `DirtChestSlot.mayPlace` refuses). A shift-click from the player's slot `quick_move`
/// fills slot 0 and may spill into the stand-ins: that rest goes back where it came
/// from, as `moveItemStackTo` stops at the one real slot.
pub fn dirt_settle<S: Stack>(
    before: &[Option<S>],
    after: &[Option<S>],
    quick_move: Option<usize>,
) -> Option<Vec<Option<S>>> {
    let mut out = after.to_vec();
    let changed = match (&before[0], &after[0]) {
        (None, None) => false,
        (Some(a), Some(b)) => !(a.same_kind(b) && a.count() == b.count()),
        _ => true,
    };
    if changed && after[0].as_ref().is_some_and(|s| !dirt_accepts(s.item())) {
        return None;
    }
    let spilled: Vec<S> = out[1..9].iter_mut().filter_map(Option::take).collect();
    if spilled.is_empty() {
        return Some(out);
    }
    let src = quick_move.filter(|&s| s >= 9 && s < out.len())?;
    for s in spilled {
        match &mut out[src] {
            None => out[src] = Some(s),
            Some(back) if back.same_kind(&s) => back.set_count(back.count() + s.count()),
            Some(_) => return None,
        }
    }
    Some(out)
}

// ---- the crystal chest's top items (ICrystalChest, IronChestRenderer) ----

/// `ICrystalChest.sortTopStacks`: the kinds of item inside, the most first, at most
/// eight. As in the original, a second stack of a kind adds its count only when the
/// two counts differ (`if (itemStack.getCount() != tempCopyStack.getCount()) grow`);
/// the counts only order the display.
pub fn top_stacks<S: Stack>(items: &[Option<S>]) -> Vec<S> {
    let mut kinds: Vec<S> = Vec::new();
    for stack in items.iter().take(Tier::Crystal.size()).flatten() {
        if stack.count() == 0 {
            continue;
        }
        match kinds.iter_mut().find(|k| k.same_kind(stack)) {
            Some(k) => {
                if k.count() != stack.count() {
                    k.set_count(k.count() + stack.count());
                }
            }
            None => kinds.push(stack.clone()),
        }
    }
    // List.sort is stable: equal counts keep their slot order.
    kinds.sort_by(|a, b| b.count().cmp(&a.count()));
    kinds.truncate(8);
    kinds
}
/// `IronChestRenderer.MODEL_ITEMS`: the eight spots drawn (the ninth is never used,
/// `j < MODEL_ITEMS.size() - 1`), in the block's own corner coordinates.
pub const TOP_ITEM_SPOTS: [[f32; 3]; 8] = [
    [0.3, 0.45, 0.3],
    [0.7, 0.45, 0.3],
    [0.3, 0.45, 0.7],
    [0.7, 0.45, 0.7],
    [0.3, 0.1, 0.3],
    [0.7, 0.1, 0.3],
    [0.3, 0.1, 0.7],
    [0.7, 0.1, 0.7],
];
/// `ModelItem(.., 3.0F).getSizeScaled()`: 3 / 16.
pub const TOP_ITEM_SCALE: f32 = 3.0 / 16.0;
/// Top items are drawn within this many blocks of the camera (`closerThan(.., 128)`).
pub const TOP_ITEM_DISTANCE: f64 = 128.0;
/// `(float) (360D * (System.currentTimeMillis() & 0x3FFFL) / 0x3FFFL) - partialTicks`, degrees.
pub fn top_item_rotation(millis: u64, partial: f32) -> f32 {
    (360.0 * (millis & 0x3FFF) as f64 / 0x3FFF as f64) as f32 - partial
}

// ---- the DirtChest 9000's guide book (Util.createDirtGuideBook) ----

pub const BOOK_TITLE: &str = "How to use your DirtChest 9000!";
pub const BOOK_AUTHOR: &str = "cpw";
/// `addTagElement("generation", IntTag.valueOf(3))`.
pub const BOOK_GENERATION: u8 = 3;
/// `addTranslatedPages(.., "ironchest.book.dirtchest9000", 5)`.
pub fn book_pages() -> Vec<String> {
    (1..=5)
        .map(|i| format!("ironchest.book.dirtchest9000.{i}"))
        .collect()
}
