// The Iron Chests rules on a fake world (values from Iron Chests 1.20.1-14.4.4).
use horizon_ironchest::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
struct S {
    item: String,
    count: u32,
    /// Stands for the components: two stacks are the same kind with the same tag.
    tag: u32,
}
fn s(item: &str, count: u32) -> S {
    S {
        item: item.into(),
        count,
        tag: 0,
    }
}
impl Stack for S {
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
        self.item == other.item && self.tag == other.tag
    }
}

#[derive(Default)]
struct Fake {
    blocks: BTreeMap<Pos, Block>,
    items: BTreeMap<Pos, Vec<Option<S>>>,
    names: BTreeMap<Pos, String>,
    openers: BTreeMap<Pos, i32>,
    /// Block changes made, in order.
    sets: Vec<Pos>,
}
impl World for Fake {
    type Stack = S;
    fn block(&self, p: Pos) -> Block {
        self.blocks.get(&p).copied().unwrap_or(Block::Other)
    }
    fn set_chest(&mut self, p: Pos, chest: Chest, facing: Facing, half: Half, waterlogged: bool) {
        // Like the game's set_block with block-entity side effects skipped: a block
        // entity still there would be dropped silently, so the rules must take it first.
        if !matches!(self.blocks.get(&p), Some(Block::Iron { chest: c, .. }) if *c == chest) {
            assert!(
                !self.items.contains_key(&p),
                "the block entity at {p:?} would be lost"
            );
        }
        self.blocks.insert(
            p,
            Block::Iron {
                chest,
                facing,
                half,
                waterlogged,
            },
        );
        self.sets.push(p);
    }
    fn items(&self, p: Pos) -> Option<Vec<Option<S>>> {
        self.items.get(&p).cloned()
    }
    fn take_items(&mut self, p: Pos) -> Option<Vec<Option<S>>> {
        self.items.remove(&p)
    }
    fn put_items(&mut self, p: Pos, items: Vec<Option<S>>) {
        self.items.insert(p, items);
    }
    fn custom_name(&self, p: Pos) -> Option<String> {
        self.names.get(&p).cloned()
    }
    fn set_custom_name(&mut self, p: Pos, name: Option<String>) {
        match name {
            Some(n) => self.names.insert(p, n),
            None => self.names.remove(&p),
        };
    }
    fn openers(&self, p: Pos) -> i32 {
        self.openers.get(&p).copied().unwrap_or(0)
    }
}
fn chest(tier: Tier, trapped: bool) -> Chest {
    Chest { tier, trapped }
}
fn iron_block(c: Chest, facing: Facing, half: Half) -> Block {
    Block::Iron {
        chest: c,
        facing,
        half,
        waterlogged: false,
    }
}
fn total(items: &[Option<S>]) -> BTreeMap<String, u32> {
    let mut m = BTreeMap::new();
    for st in items.iter().flatten() {
        *m.entry(st.item.clone()).or_default() += st.count;
    }
    m
}

#[test]
fn tiers_are_iron_chests_types() {
    // IronChestsTypes(size, rowLength, xSize, ySize, guiTexture, textureXSize, textureYSize).
    let expect = [
        (Tier::Iron, 54, 9, 184, 222, "iron", 256, 256),
        (Tier::Gold, 81, 9, 184, 276, "gold", 256, 276),
        (Tier::Diamond, 108, 12, 238, 276, "diamond", 256, 276),
        (Tier::Copper, 45, 9, 184, 204, "copper", 256, 256),
        (Tier::Crystal, 108, 12, 238, 276, "diamond", 256, 276),
        (Tier::Obsidian, 108, 12, 238, 276, "diamond", 256, 276),
        (Tier::Dirt, 1, 1, 184, 184, "dirt", 256, 256),
    ];
    for (tier, size, row, w, h, tex, tw, th) in expect {
        assert_eq!(tier.size(), size, "{tier:?}");
        assert_eq!(tier.row_length(), row, "{tier:?}");
        assert_eq!(tier.rows() * tier.row_length(), size, "{tier:?}");
        let gui = tier.gui();
        assert_eq!((gui.width, gui.height), (w, h), "{tier:?}");
        assert_eq!(gui.sprite, format!("ironchest:block/gui/{tex}_container"));
        assert_eq!((gui.texture_width, gui.texture_height), (tw, th), "{tier:?}");
        assert!(gui.width <= gui.texture_width && gui.height <= gui.texture_height);
    }
    assert!(Tier::Crystal.transparent());
    assert!(Tier::ALL.iter().filter(|t| t.transparent()).count() == 1);
}

#[test]
fn the_fourteen_blocks_and_their_names() {
    let all: Vec<Chest> = Chest::all().collect();
    assert_eq!(all.len(), 14);
    for c in &all {
        assert_eq!(Chest::parse(&c.name()), Some(*c));
        assert_eq!(size_of(&c.name()), Some(c.tier.size()));
    }
    assert_eq!(
        Chest::parse("ironchest:trapped_crystal_chest"),
        Some(chest(Tier::Crystal, true))
    );
    assert_eq!(chest(Tier::Dirt, false).name(), "ironchest:dirt_chest");
    assert_eq!(
        chest(Tier::Obsidian, true).texture(),
        "ironchest:block/model/trapped_obsidian_chest"
    );
    assert_eq!(
        chest(Tier::Gold, true).item_sprite(),
        "entity/chest/ironchest:trapped_gold_chest"
    );
    for not in [
        "minecraft:chest",
        "minecraft:copper_chest",
        "ironchest:silver_chest",
        "ironchest:netherite_chest",
        "ironchest:iron_to_gold_chest_upgrade",
        "ironchest:iron",
        "ironchest:trapped_",
    ] {
        assert_eq!(Chest::parse(not), None, "{not}");
    }
    // Plain and trapped chests share the title; the stat differs.
    assert_eq!(chest(Tier::Iron, true).title_key(), "ironchest.container.iron_chest");
    assert_eq!(chest(Tier::Iron, false).stat(), "open_chest");
    assert_eq!(chest(Tier::Iron, true).stat(), "trigger_trapped_chest");
    assert_eq!(chest(Tier::Dirt, false).title_fallback(), "DirtChest 9000!");
}

#[test]
fn upgrade_items_are_iron_chests_upgrade_types() {
    let expect = [
        ("ironchest:iron_to_gold_chest_upgrade", Some(Tier::Iron), Tier::Gold),
        ("ironchest:gold_to_diamond_chest_upgrade", Some(Tier::Gold), Tier::Diamond),
        ("ironchest:copper_to_iron_chest_upgrade", Some(Tier::Copper), Tier::Iron),
        ("ironchest:diamond_to_crystal_chest_upgrade", Some(Tier::Diamond), Tier::Crystal),
        ("ironchest:wood_to_iron_chest_upgrade", None, Tier::Iron),
        ("ironchest:wood_to_copper_chest_upgrade", None, Tier::Copper),
        ("ironchest:diamond_to_obsidian_chest_upgrade", Some(Tier::Diamond), Tier::Obsidian),
    ];
    for (item, source, target) in expect {
        let up = Upgrade::parse(item).expect(item);
        assert_eq!((up.source(), up.target()), (source, target), "{item}");
        assert_eq!(up.item(), item);
    }
    assert_eq!(Upgrade::parse("ironchest:iron_chest"), None);
    // Every upgrade goes to as many slots or more: nothing is ever cut off.
    for up in Upgrade::ALL {
        let from = up.source().map_or(27, Tier::size);
        assert!(up.target().size() >= from, "{up:?}");
    }
}

#[test]
fn an_upgrade_keeps_items_name_facing_and_trap() {
    let mut w = Fake::default();
    let p = [3, 64, -7];
    w.blocks.insert(p, iron_block(chest(Tier::Iron, true), Facing::East, Half::Single));
    let mut items = vec![None; 54];
    items[0] = Some(s("minecraft:diamond", 64));
    items[17] = Some(s("minecraft:stone", 3));
    items[53] = Some(s("minecraft:apple", 1));
    w.items.insert(p, items);
    w.names.insert(p, "Loot".into());
    let out = upgrade(&mut w, p, Upgrade::IronToGold);
    assert_eq!(
        out,
        UpgradeOutcome::Upgraded {
            to: chest(Tier::Gold, true)
        }
    );
    let Block::Iron { chest: c, facing, half, .. } = w.block(p) else {
        panic!("no chest")
    };
    assert_eq!((c, facing, half), (chest(Tier::Gold, true), Facing::East, Half::Single));
    let after = &w.items[&p];
    assert_eq!(after.len(), 81);
    // setItems keeps each stack in its slot.
    assert_eq!(after[0], Some(s("minecraft:diamond", 64)));
    assert_eq!(after[17], Some(s("minecraft:stone", 3)));
    assert_eq!(after[53], Some(s("minecraft:apple", 1)));
    assert!(after[54..].iter().all(Option::is_none));
    assert_eq!(w.names.get(&p).map(String::as_str), Some("Loot"));
}

#[test]
fn wood_upgrades_take_a_vanilla_chest() {
    for trapped in [false, true] {
        let mut w = Fake::default();
        let p = [0, 70, 0];
        w.blocks.insert(
            p,
            Block::Vanilla {
                trapped,
                facing: Facing::West,
            },
        );
        let items: Vec<Option<S>> = (0..27).map(|i| Some(s("minecraft:dirt", i + 1))).collect();
        let before = total(&items);
        w.items.insert(p, items);
        let out = upgrade(&mut w, p, Upgrade::WoodToCopper);
        assert_eq!(
            out,
            UpgradeOutcome::Upgraded {
                to: chest(Tier::Copper, trapped)
            }
        );
        assert_eq!(w.items[&p].len(), 45);
        assert_eq!(total(&w.items[&p]), before);
        assert!(matches!(
            w.block(p),
            Block::Iron { facing: Facing::West, half: Half::Single, .. }
        ));
    }
}

#[test]
fn upgrades_that_do_not_apply_pass() {
    let p = [1, 1, 1];
    let iron = || {
        let mut w = Fake::default();
        w.blocks.insert(p, iron_block(chest(Tier::Iron, false), Facing::North, Half::Single));
        w.items.insert(p, vec![None; 54]);
        w
    };
    // Wrong source tier, wood on an iron chest.
    for up in [Upgrade::GoldToDiamond, Upgrade::WoodToIron, Upgrade::DiamondToCrystal] {
        let mut w = iron();
        assert_eq!(upgrade(&mut w, p, up), UpgradeOutcome::Pass, "{up:?}");
        assert!(w.sets.is_empty());
    }
    // An iron upgrade on a vanilla chest.
    let mut w = Fake::default();
    w.blocks.insert(p, Block::Vanilla { trapped: false, facing: Facing::North });
    w.items.insert(p, vec![None; 27]);
    assert_eq!(upgrade(&mut w, p, Upgrade::IronToGold), UpgradeOutcome::Pass);
    // Someone has it open.
    let mut w = iron();
    w.openers.insert(p, 1);
    assert_eq!(upgrade(&mut w, p, Upgrade::IronToGold), UpgradeOutcome::Pass);
    // No block entity: no new chest (newChest == null).
    let mut w = iron();
    w.items.clear();
    assert_eq!(upgrade(&mut w, p, Upgrade::IronToGold), UpgradeOutcome::Pass);
    // A block entity holding more than the target has slots for is left alone.
    let mut w = Fake::default();
    w.blocks.insert(p, iron_block(chest(Tier::Diamond, false), Facing::North, Half::Single));
    let mut big = vec![None; 110];
    big[109] = Some(s("minecraft:emerald", 2));
    w.items.insert(p, big.clone());
    assert_eq!(upgrade(&mut w, p, Upgrade::DiamondToCrystal), UpgradeOutcome::Pass);
    assert_eq!(w.items[&p], big);
    // Air, another block.
    let mut w = Fake::default();
    assert_eq!(upgrade(&mut w, p, Upgrade::WoodToIron), UpgradeOutcome::Pass);
}

#[test]
fn the_whole_upgrade_chain_keeps_every_item() {
    let mut w = Fake::default();
    let p = [9, 9, 9];
    w.blocks.insert(p, Block::Vanilla { trapped: false, facing: Facing::South });
    let items: Vec<Option<S>> = (0..27)
        .map(|i| (i % 2 == 0).then(|| s(&format!("minecraft:item{i}"), 64)))
        .collect();
    let before = total(&items);
    w.items.insert(p, items);
    for (up, size) in [
        (Upgrade::WoodToCopper, 45),
        (Upgrade::CopperToIron, 54),
        (Upgrade::IronToGold, 81),
        (Upgrade::GoldToDiamond, 108),
        (Upgrade::DiamondToObsidian, 108),
    ] {
        assert!(matches!(upgrade(&mut w, p, up), UpgradeOutcome::Upgraded { .. }), "{up:?}");
        assert_eq!(w.items[&p].len(), size);
        assert_eq!(total(&w.items[&p]), before, "{up:?}");
    }
    assert!(matches!(w.block(p), Block::Iron { chest: Chest { tier: Tier::Obsidian, trapped: false }, .. }));
}

#[test]
fn placed_chests_never_pair() {
    // The chest template paired the new chest (left, facing north) with the one east of it.
    let mut w = Fake::default();
    let (a, b) = ([0, 64, 0], [1, 64, 0]);
    let c = chest(Tier::Iron, false);
    w.blocks.insert(a, iron_block(c, Facing::North, Half::Left));
    w.blocks.insert(b, iron_block(c, Facing::North, Half::Right));
    w.items.insert(b, vec![Some(s("minecraft:coal", 5)); 54]);
    assert_eq!(placed(&mut w, a), Some(true));
    assert_eq!(w.block(a), iron_block(c, Facing::North, Half::Single));
    assert_eq!(w.block(b), iron_block(c, Facing::North, Half::Single));
    // Neither lost its block entity, and the old one kept its items.
    assert_eq!(w.items[&a].len(), 54);
    assert_eq!(total(&w.items[&b])["minecraft:coal"], 54 * 5);
    // A right half pairs counter-clockwise (west of a north-facing chest).
    let mut w = Fake::default();
    let (a, west) = ([0, 64, 0], [-1, 64, 0]);
    w.blocks.insert(a, iron_block(c, Facing::North, Half::Right));
    w.blocks.insert(west, iron_block(c, Facing::North, Half::Left));
    placed(&mut w, a);
    assert_eq!(w.block(west), iron_block(c, Facing::North, Half::Single));
    // A single chest is left alone, and a different chest next to it too.
    let mut w = Fake::default();
    w.blocks.insert(a, iron_block(c, Facing::South, Half::Single));
    w.blocks.insert(west, iron_block(chest(Tier::Gold, false), Facing::South, Half::Left));
    placed(&mut w, a);
    assert_eq!(w.sets, Vec::<Pos>::new());
    assert_eq!(placed(&mut w, [5, 5, 5]), None);
}

#[test]
fn a_smaller_block_entity_grows_without_loss() {
    // A vanilla path took ironchest:copper_chest for a 27-slot copper chest.
    let mut w = Fake::default();
    let p = [2, 2, 2];
    w.blocks.insert(p, iron_block(chest(Tier::Copper, false), Facing::North, Half::Single));
    let items: Vec<Option<S>> = (0..27).map(|i| Some(s("minecraft:iron_ingot", i + 1))).collect();
    let before = total(&items);
    w.items.insert(p, items);
    assert_eq!(ensure_entity(&mut w, p), Some(false));
    assert_eq!(w.items[&p].len(), 45);
    assert_eq!(total(&w.items[&p]), before);
    assert_eq!(w.items[&p][26], Some(s("minecraft:iron_ingot", 27)));
    // A bigger one is never cut down.
    let mut big = vec![None; 60];
    big[59] = Some(s("minecraft:gold_ingot", 1));
    w.items.insert(p, big);
    ensure_entity(&mut w, p);
    assert_eq!(w.items[&p].len(), 60);
    assert_eq!(w.items[&p][59], Some(s("minecraft:gold_ingot", 1)));
    // A missing one is made with all the tier's slots.
    w.items.clear();
    assert_eq!(ensure_entity(&mut w, p), Some(true));
    assert_eq!(w.items[&p], vec![None; 45]);
}

#[test]
fn the_dirt_chest_gets_its_guide_once() {
    let mut w = Fake::default();
    let p = [0, 0, 0];
    let book = s("minecraft:written_book", 1);
    w.blocks.insert(p, iron_block(chest(Tier::Dirt, false), Facing::North, Half::Single));
    placed(&mut w, p);
    assert!(give_guide_book(&mut w, p, false, book.clone()));
    assert_eq!(w.items[&p], vec![Some(book.clone())]);
    // Not twice, and not over what is there.
    assert!(!give_guide_book(&mut w, p, false, book.clone()));
    w.items.insert(p, vec![Some(s("minecraft:dirt", 64))]);
    assert!(!give_guide_book(&mut w, p, false, book.clone()));
    assert_eq!(w.items[&p], vec![Some(s("minecraft:dirt", 64))]);
    // ChestPlacedAlready: no book.
    w.items.insert(p, vec![None]);
    assert!(!give_guide_book(&mut w, p, true, book.clone()));
    // Only the dirt chest.
    let q = [1, 0, 0];
    w.blocks.insert(q, iron_block(chest(Tier::Iron, false), Facing::North, Half::Single));
    placed(&mut w, q);
    assert!(!give_guide_book(&mut w, q, false, book.clone()));
    // Breaking takes the guide, and only the guide.
    w.items.insert(p, vec![Some(book.clone())]);
    let is_guide = |st: &S| st.item == "minecraft:written_book" && st.tag == 0;
    assert!(strip_guide_book(&mut w, p, is_guide));
    assert_eq!(w.items[&p], vec![None]);
    let mut players = book.clone();
    players.tag = 7;
    w.items.insert(p, vec![Some(players.clone())]);
    assert!(!strip_guide_book(&mut w, p, is_guide));
    assert_eq!(w.items[&p], vec![Some(players)]);
    w.items.insert(p, vec![Some(s("minecraft:dirt", 3))]);
    assert!(!strip_guide_book(&mut w, p, is_guide));
    assert_eq!(book_pages().len(), 5);
    assert_eq!(book_pages()[4], "ironchest.book.dirtchest9000.5");
    assert_eq!((BOOK_TITLE, BOOK_AUTHOR, BOOK_GENERATION), ("How to use your DirtChest 9000!", "cpw", 3));
}

#[test]
fn menu_slots_map_both_ways() {
    for tier in Tier::ALL {
        let n = menu_container(tier);
        assert!(n >= tier.size() && n % 9 == 0, "{tier:?}");
        let mut seen = std::collections::BTreeSet::new();
        let game: Vec<usize> = (0..36).chain(CONTAINER_SLOT..CONTAINER_SLOT + tier.size()).collect();
        for g in game {
            let m = to_menu(tier, g).unwrap_or_else(|| panic!("{tier:?} {g}"));
            assert!(m < n + 36);
            assert!(seen.insert(m), "{tier:?}: menu slot {m} twice");
            assert_eq!(to_game(tier, m), Some(g), "{tier:?} {g}");
        }
        // IronChestMenu: the chest, then the inventory rows, then the hotbar.
        assert_eq!(to_menu(tier, CONTAINER_SLOT), Some(0));
        assert_eq!(to_menu(tier, 9), Some(n));
        assert_eq!(to_menu(tier, 0), Some(n + 27));
        assert_eq!(to_menu(tier, CONTAINER_SLOT + tier.size()), None);
        assert_eq!(to_menu(tier, 36), None);
        assert_eq!(to_game(tier, n + 36), None);
    }
    // The dirt chest's stand-in slots are nobody's.
    for m in 1..9 {
        assert_eq!(to_game(Tier::Dirt, m), None);
    }
    assert_eq!(menu_rows(Tier::Diamond), 12);
    assert_eq!(menu_rows(Tier::Gold), 9);
    assert_eq!(menu_rows(Tier::Copper), 5);
}

#[test]
fn slots_sit_where_iron_chest_menu_puts_them() {
    // new Slot(inventory, col + row * rowLength, 12 + col * 18, 18 + row * 18)
    assert_eq!(slot_position(Tier::Iron, 300), Some((12, 18)));
    assert_eq!(slot_position(Tier::Iron, 353), Some((12 + 8 * 18, 18 + 5 * 18)));
    assert_eq!(slot_position(Tier::Diamond, 300 + 11), Some((12 + 11 * 18, 18)));
    assert_eq!(slot_position(Tier::Diamond, 300 + 107), Some((12 + 11 * 18, 18 + 8 * 18)));
    assert_eq!(slot_position(Tier::Gold, 300 + 80), Some((12 + 8 * 18, 18 + 8 * 18)));
    assert_eq!(slot_position(Tier::Dirt, 300), Some((84, 44)));
    assert_eq!(slot_position(Tier::Dirt, 301), None);
    // leftCol = (xSize - 162) / 2 + 1; rows at ySize - (4 - row) * 18 - 10, hotbar ySize - 24.
    assert_eq!(slot_position(Tier::Iron, 9), Some((12, 222 - 72 - 10)));
    assert_eq!(slot_position(Tier::Iron, 0), Some((12, 198)));
    assert_eq!(slot_position(Tier::Diamond, 0), Some((39, 276 - 24)));
    assert_eq!(slot_position(Tier::Diamond, 35), Some((39 + 8 * 18, 276 - 2 * 18 - 10)));
    // Every slot inside the image, none on another.
    for tier in Tier::ALL {
        let gui = tier.gui();
        let mut at = std::collections::BTreeSet::new();
        for g in (0..36).chain(300..300 + tier.size()) {
            let (x, y) = slot_position(tier, g).unwrap();
            assert!(x >= 0 && y >= 0 && x + 16 <= gui.width && y + 16 <= gui.height, "{tier:?} {g}");
            assert!(at.insert((x, y)), "{tier:?} {g} overlaps");
        }
        assert_eq!(label_positions(tier)[1], (8, gui.height - 94));
    }
}

/// A menu row of the dirt chest: slot 0, eight stand-ins, then 36 player slots.
fn dirt_menu() -> Vec<Option<S>> {
    vec![None; 45]
}

#[test]
fn the_dirt_slot_takes_only_dirt() {
    assert!(dirt_accepts("minecraft:dirt"));
    assert!(!dirt_accepts("minecraft:coarse_dirt"));
    // A shift-click of 64 dirt onto 10: slot 0 fills, the rest stays in the inventory.
    let mut before = dirt_menu();
    before[0] = Some(s("minecraft:dirt", 10));
    before[20] = Some(s("minecraft:dirt", 64));
    let mut after = before.clone();
    after[0] = Some(s("minecraft:dirt", 64));
    after[20] = None;
    after[1] = Some(s("minecraft:dirt", 10));
    let kept = dirt_settle(&before, &after, Some(20)).expect("kept");
    assert_eq!(kept[0], Some(s("minecraft:dirt", 64)));
    assert_eq!(kept[20], Some(s("minecraft:dirt", 10)));
    assert!(kept[1..9].iter().all(Option::is_none));
    assert_eq!(total(&kept), total(&before));
    // Stone into an empty dirt slot: refused, nothing moves.
    let mut before = dirt_menu();
    before[30] = Some(s("minecraft:stone", 5));
    let mut after = before.clone();
    after[0] = after[30].take();
    assert_eq!(dirt_settle(&before, &after, Some(30)), None);
    assert_eq!(dirt_settle(&before, &after, None), None);
    // Stone shift-clicked while dirt fills slot 0: it lands in a stand-in and comes back.
    let mut before = dirt_menu();
    before[0] = Some(s("minecraft:dirt", 64));
    before[30] = Some(s("minecraft:stone", 5));
    let mut after = before.clone();
    after[2] = after[30].take();
    assert_eq!(dirt_settle(&before, &after, Some(30)), Some(before.clone()));
    // A stand-in filled any other way: the click is dropped.
    assert_eq!(dirt_settle(&before, &after, None), None);
    // Taking dirt out, or leaving a stone a hopper put there, is fine.
    let mut before = dirt_menu();
    before[0] = Some(s("minecraft:stone", 1));
    let mut after = before.clone();
    after[10] = Some(s("minecraft:dirt", 1));
    assert!(dirt_settle(&before, &after, None).is_some());
    after[0] = None;
    assert!(dirt_settle(&before, &after, None).is_some());
}

#[test]
fn crystal_top_items_follow_sort_top_stacks() {
    let mut items: Vec<Option<S>> = vec![None; 108];
    items[0] = Some(s("minecraft:stone", 10));
    items[5] = Some(s("minecraft:dirt", 64));
    items[6] = Some(s("minecraft:stone", 20)); // counts differ: grows to 30
    items[7] = Some(s("minecraft:dirt", 64)); // same count: not added (the original's rule)
    items[50] = Some(s("minecraft:apple", 30));
    let tops = top_stacks(&items);
    let shown: Vec<(&str, u32)> = tops.iter().map(|t| (t.item.as_str(), t.count)).collect();
    // Most first; equal counts keep their order (stone came first).
    assert_eq!(
        shown,
        [("minecraft:dirt", 64), ("minecraft:stone", 30), ("minecraft:apple", 30)]
    );
    // Components tell kinds apart.
    let mut a = s("minecraft:potion", 1);
    a.tag = 1;
    let b = s("minecraft:potion", 1);
    assert_eq!(top_stacks(&[Some(a), Some(b)]).len(), 2);
    // At most eight, and only the crystal chest's 108 slots.
    let many: Vec<Option<S>> = (0..120).map(|i| Some(s(&format!("minecraft:i{i}"), (i % 50) + 1))).collect();
    let tops = top_stacks(&many);
    assert_eq!(tops.len(), 8);
    assert!(tops.windows(2).all(|w| w[0].count >= w[1].count));
    assert!(top_stacks::<S>(&vec![None; 108]).is_empty());
    assert_eq!(TOP_ITEM_SPOTS.len(), 8);
    assert!((TOP_ITEM_SCALE - 0.1875).abs() < 1e-6);
    assert_eq!(top_item_rotation(0, 0.0), 0.0);
    assert!((top_item_rotation(0x3FFF, 0.0) - 360.0).abs() < 1e-3);
    // Only the low 14 bits of the clock count.
    let half = 360.0 * 0x1FFF as f32 / 0x3FFF as f32 - 0.5;
    assert!((top_item_rotation(0x4000 + 0x1FFF, 0.5) - half).abs() < 1e-3);
}

// ---- the game library's copy of the sizes (mixins/ironchest/shared.rs) and the world hook ----

mod shared {
    include!("../mixins/ironchest/shared.rs");
}
mod pre_remove_hook {
    #[derive(Debug, PartialEq, Eq)]
    pub enum PreRemove {
        Contents { slots: usize, furnace: bool },
    }
    include!("../mixins/ironchest/pre_remove.rs");
    pub fn slots(name: &str) -> Option<usize> {
        match ironchest_pre_remove(name) {
            ::core::ops::ControlFlow::Break(PreRemove::Contents { slots, furnace }) => {
                assert!(!furnace);
                Some(slots)
            }
            ::core::ops::ControlFlow::Continue(()) => None,
        }
    }
}

#[test]
fn the_library_copy_and_the_spill_hook_agree_with_the_rules() {
    let all: Vec<Chest> = Chest::all().collect();
    assert_eq!(shared::BLOCKS.len(), all.len());
    for (i, c) in all.iter().enumerate() {
        let (name, size, texture) = shared::BLOCKS[i];
        assert_eq!(name, c.name());
        assert_eq!(size, c.tier.size(), "{name}");
        assert_eq!(texture, c.texture());
        assert_eq!(shared::texture(i), texture);
        assert_eq!(shared::index(name), Some(i));
        assert_eq!(shared::size(name), Some(size));
        assert_eq!(pre_remove_hook::slots(name), Some(size), "{name}");
    }
    for not in ["minecraft:chest", "minecraft:copper_chest", "ironchest:silver_chest", "stone"] {
        assert_eq!(shared::size(not), None);
        assert_eq!(pre_remove_hook::slots(not), None);
        assert_eq!(shared::code(not, Some("north")), 0);
    }
}

#[test]
fn the_state_table_answers_by_state_id() {
    let states = [
        ("minecraft:stone", None),
        ("ironchest:iron_chest", Some("south")),
        ("ironchest:iron_chest", Some("east")),
        ("ironchest:trapped_dirt_chest", Some("west")),
        ("minecraft:chest", Some("north")),
    ];
    shared::set_states(states.iter().map(|(n, f)| shared::code(n, *f)).collect());
    assert_eq!(shared::chest_state(0), None);
    assert_eq!(shared::chest_state(1), Some((0, 0)));
    assert_eq!(shared::chest_state(2), Some((0, 3)));
    assert_eq!(shared::chest_state(3), Some((13, 1)));
    assert_eq!(shared::chest_state(4), None);
    assert_eq!(shared::chest_state(99), None);
    assert_eq!(shared::FACINGS[2], "north");
}
