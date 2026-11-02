// The EnderStorage rules on fake data (values from EnderStorage 1.20.1-2.11.0.188 and CodeChickenLib 1.20.1).
use horizon_enderstorage::*;

#[derive(Clone, Debug, PartialEq)]
struct S(&'static str, u32);

fn who() -> Owner {
    Owner {
        uuid: [0x12345678, 0x9abc_def0u32 as i32, 0x0fed_cba9, 0x8765_4321u32 as i32],
        name: "Alex".into(),
    }
}

#[test]
fn config_is_tuned() {
    assert_eq!(ITEM_STORAGE_SIZE, 1);
    assert_eq!(SIZES[ITEM_STORAGE_SIZE], 27);
    assert_eq!(PERSONAL_ITEM, "minecraft:diamond");
    assert!(!ANARCHY_MODE);
    assert!(!USE_VANILLA_ENDER_CHEST_SOUNDS);
    assert_eq!(TANK_CAPACITY, 16_000);
}

#[test]
fn colours_follow_enum_colour() {
    assert_eq!(Colour::ALL.len(), 16);
    assert_eq!(Colour::White.wool_meta(), 0);
    assert_eq!(Colour::LightBlue.wool_meta(), 3);
    assert_eq!(Colour::Black.wool_meta(), 15);
    assert_eq!(Colour::from_wool_meta(14), Some(Colour::Red));
    assert_eq!(Colour::from_wool_meta(16), None);
    assert_eq!(Colour::from_wool_meta(-1), None);
    assert_eq!(Colour::from_dye_item("minecraft:light_gray_dye"), Some(Colour::LightGray));
    assert_eq!(Colour::from_dye_item("minecraft:light_gray_wool"), None);
    assert_eq!(Colour::from_wool_item("minecraft:lime_wool"), Some(Colour::Lime));
    assert_eq!(Colour::from_wool_item("minecraft:lime_carpet"), None);
    assert_eq!(Colour::from_dye_item("minecraft:glow_ink_sac"), None);
    for c in Colour::ALL {
        assert_eq!(Colour::from_name(c.name()), Some(c));
        assert_eq!(Colour::from_dye_item(&format!("minecraft:{}_dye", c.name())), Some(c));
    }
    assert_eq!(Colour::Cyan.lang_key(), "item.minecraft.firework_star.cyan");
}

#[test]
fn mixes_are_the_original_table_both_ways() {
    use Colour::*;
    let table = [
        (Yellow, Red, Orange),
        (Pink, Purple, Magenta),
        (White, Blue, LightBlue),
        (White, Green, Lime),
        (White, Red, Pink),
        (White, Black, Gray),
        (White, Gray, LightGray),
        (Blue, Green, Cyan),
        (Blue, Red, Purple),
        (Orange, Red, Brown),
        (Yellow, Blue, Green),
    ];
    for (a, b, r) in table {
        assert_eq!(Colour::mix(a, b), Some(r));
        assert_eq!(Colour::mix(b, a), Some(r));
    }
    assert_eq!(Colour::mix(Red, Red), Some(Red));
    assert_eq!(Colour::mix(Red, Black), None);
}

#[test]
fn frequency_keys_name_the_owner_by_uuid() {
    let mut f = Frequency::new(Colour::Red, Colour::White, Colour::Blue);
    assert_eq!(f.key(), "left=red,middle=white,right=blue");
    assert_eq!(Frequency::default().key(), "left=white,middle=white,right=white");
    f.owner = Some(who());
    assert_eq!(
        f.key(),
        "left=red,middle=white,right=blue,owner=12345678-9abc-def0-0fed-cba987654321"
    );
    // The name is not part of the key.
    let mut g = f.clone();
    g.owner.as_mut().unwrap().name = "Other".into();
    assert_eq!(f.key(), g.key());
    // set() leaves a None alone (Frequency.setLeft(null)).
    let mut h = Frequency::default();
    h.set([None, Some(Colour::Pink), None]);
    assert_eq!(h.colours(), [Colour::White, Colour::Pink, Colour::White]);
}

#[test]
fn inventory_sizes_and_alignment() {
    assert_eq!(SIZES, [9, 27, 54]);
    let s: ItemStorage<S> = ItemStorage::new(1);
    assert_eq!((s.size, s.items.len()), (1, 27));
    // A saved 3x3 grows when the config says 3x9 (items stay where they are).
    let mut items = vec![None; 9];
    items[4] = Some(S("minecraft:stone", 5));
    let s = ItemStorage::from_saved(0, items, 1);
    assert_eq!((s.size, s.items.len()), (1, 27));
    assert_eq!(s.items[4], Some(S("minecraft:stone", 5)));
    // A saved 6x9 shrinks to 3x9 only if the stacks fit; they are packed to the front.
    let mut items: Vec<Option<S>> = vec![None; 54];
    items[40] = Some(S("a", 1));
    items[53] = Some(S("b", 2));
    let s = ItemStorage::from_saved(2, items, 1);
    assert_eq!((s.size, s.items.len()), (1, 27));
    assert_eq!(s.items[0], Some(S("a", 1)));
    assert_eq!(s.items[1], Some(S("b", 2)));
    assert!(s.items[2..].iter().all(Option::is_none));
    // Too many stacks to fit: it stays big.
    let full: Vec<Option<S>> = (0..54).map(|_| Some(S("a", 1))).collect();
    let s = ItemStorage::from_saved(2, full, 1);
    assert_eq!((s.size, s.items.len()), (2, 54));
    // ... until it is saved with nobody looking and room has been made.
    let mut s = s;
    for i in 27..54 {
        s.items[i] = None;
    }
    s.before_save(1, true);
    assert_eq!(s.size, 2);
    s.before_save(1, false);
    assert_eq!((s.size, s.items.len()), (1, 27));
    // A broken size byte takes the configured size.
    let s: ItemStorage<S> = ItemStorage::from_saved(9, vec![None; 3], 1);
    assert_eq!((s.size, s.items.len()), (1, 27));
}

#[test]
fn tank_fills_and_drains_like_a_fluid_tank() {
    let mut t = Liquid::default();
    assert_eq!(t.comparator(), 0);
    assert_eq!(t.fill("minecraft:water", 1000), 1000);
    assert_eq!(t.fill("minecraft:lava", 1000), 0);
    assert_eq!(t.comparator(), 1 + 0); // 1000 * 14 / 16000 = 0, plus 1
    assert_eq!(t.fill("minecraft:water", 20_000), 15_000);
    assert_eq!(t.amount, 16_000);
    assert_eq!(t.comparator(), 15);
    assert_eq!(t.drain(300), Some(("minecraft:water".into(), 300)));
    assert_eq!(t.drain(1_000_000), Some(("minecraft:water".into(), 15_700)));
    assert_eq!(t, Liquid::default());
    assert_eq!(t.drain(10), None);
    assert_eq!(t.fill("minecraft:lava", 0), 0);
}

#[test]
fn buckets_move_a_whole_bucket_or_nothing() {
    let mut t = Liquid::default();
    // Water in: the bucket is empty afterwards.
    assert_eq!(
        bucket_interact("minecraft:water_bucket", &mut t),
        Bucket::Emptied { sound: "minecraft:item.bucket.empty" }
    );
    assert_eq!(t.amount, 1000);
    // A lava bucket does not mix.
    assert_eq!(bucket_interact("minecraft:lava_bucket", &mut t), Bucket::None);
    assert_eq!(t.amount, 1000);
    // An empty bucket takes the water out again (fill is tried first).
    assert_eq!(
        bucket_interact("minecraft:bucket", &mut t),
        Bucket::Filled { bucket: "minecraft:water_bucket", sound: "minecraft:item.bucket.fill" }
    );
    assert_eq!(t, Liquid::default());
    assert_eq!(bucket_interact("minecraft:bucket", &mut t), Bucket::None);
    // Lava has its own sounds.
    t.fill("minecraft:lava", 2500);
    assert_eq!(
        bucket_interact("minecraft:bucket", &mut t),
        Bucket::Filled { bucket: "minecraft:lava_bucket", sound: "minecraft:item.bucket.fill_lava" }
    );
    assert_eq!(t.amount, 1500);
    // Less than a bucket left: the empty bucket takes nothing.
    t.drain(600);
    assert_eq!(bucket_interact("minecraft:bucket", &mut t), Bucket::None);
    // A bucket that does not fit whole is not emptied.
    let mut full = Liquid::default();
    full.fill("minecraft:water", 15_500);
    assert_eq!(bucket_interact("minecraft:water_bucket", &mut full), Bucket::None);
    assert_eq!(full.amount, 15_500);
    // Anything else does nothing.
    assert_eq!(bucket_interact("minecraft:stick", &mut full), Bucket::None);
}

fn grid<'a>(cells: [&'a str; 9]) -> Vec<Option<&'a str>> {
    cells.iter().map(|c| (!c.is_empty()).then_some(*c)).collect()
}

#[test]
fn create_recipe_takes_the_first_wool_by_columns() {
    // x outer, y inner: column 0 (top to bottom), then column 1...
    let g = grid([
        "minecraft:blaze_rod", "minecraft:red_wool", "minecraft:blaze_rod",
        "minecraft:obsidian", "minecraft:chest", "minecraft:obsidian",
        "minecraft:blaze_rod", "minecraft:ender_pearl", "minecraft:blaze_rod",
    ]);
    assert_eq!(create_frequency(&g).colours(), [Colour::Red; 3]);
    // Two wools: the one met first going down the left column wins (the recipe holds one wool).
    let g = grid([
        "", "minecraft:blue_wool", "",
        "", "", "",
        "minecraft:green_wool", "", "",
    ]);
    assert_eq!(create_frequency(&g).colours(), [Colour::Green; 3]);
    assert_eq!(create_frequency(&grid([""; 9])).colours(), [Colour::White; 3]);
    assert!(!create_frequency(&grid(["minecraft:stone"; 9])).has_owner());
}

#[test]
fn recolour_needs_dyes_above_the_item() {
    let t = "enderstorage:ender_chest";
    // One dye over the left, the chest below it in the middle column.
    let g = grid([
        "minecraft:red_dye", "", "",
        "", t, "",
        "", "", "",
    ]);
    assert!(recolour_matches(&g, t));
    let f = recolour_result(&g, t, |i| {
        assert_eq!(i, 4);
        let mut f = Frequency::new(Colour::Blue, Colour::Blue, Colour::Blue);
        f.owner = Some(who());
        f
    });
    assert_eq!(f.colours(), [Colour::Red, Colour::Blue, Colour::Blue]);
    assert_eq!(f.owner, Some(who()));
    // Chest on the bottom row: two rows of dyes work, and two dyes in a column mix.
    let g = grid([
        "minecraft:yellow_dye", "minecraft:white_dye", "",
        "minecraft:red_dye", "minecraft:green_dye", "minecraft:black_dye",
        "", t, "",
    ]);
    assert!(recolour_matches(&g, t));
    let f = recolour_result(&g, t, |_| Frequency::default());
    // yellow + red = orange; white + green = lime; black alone.
    assert_eq!(f.colours(), [Colour::Orange, Colour::Lime, Colour::Black]);
    // No dye: not a recipe. No chest: not a recipe. Empty grid: not a recipe.
    assert!(!recolour_matches(&grid(["", "", "", "", t, "", "", "", ""]), t));
    assert!(!recolour_matches(&grid(["minecraft:red_dye", "", "", "", "", "", "", "", ""]), t));
    assert!(!recolour_matches(&grid([""; 9]), t));
    // The chest in the top row cannot be recoloured (the search starts at the second row).
    assert!(!recolour_matches(&grid(["minecraft:red_dye", t, "", "", "", "", "", "", ""]), t));
    // A dye that does not mix, or mixes into itself, spoils it.
    let bad = grid([
        "minecraft:red_dye", "", "",
        "minecraft:black_dye", "", "",
        "", t, "",
    ]);
    assert!(!recolour_matches(&bad, t));
    // Same colour twice in a column: mix(a, a) == a == colour, invalid.
    let twice = grid([
        "minecraft:red_dye", "", "",
        "minecraft:red_dye", "", "",
        "", t, "",
    ]);
    assert!(!recolour_matches(&twice, t));
    // A dye below or beside the chest, or a stranger in the grid, spoils it.
    let below = grid([
        "minecraft:red_dye", "", "",
        "", t, "minecraft:blue_dye",
        "", "", "",
    ]);
    assert!(!recolour_matches(&below, t));
    let stranger = grid([
        "minecraft:red_dye", "", "",
        "", t, "minecraft:stone",
        "", "", "",
    ]);
    assert!(!recolour_matches(&stranger, t));
    // Another kind of storage in the middle column is not this recipe's ingredient.
    assert!(!recolour_matches(&grid(["minecraft:red_dye", "", "", "", "enderstorage:ender_tank", "", "", "", ""]), t));
}

#[test]
fn yaw_makes_the_rotation() {
    assert_eq!(rotation_from_yaw(0.0), 2);
    assert_eq!(rotation_from_yaw(90.0), 3);
    assert_eq!(rotation_from_yaw(180.0), 0);
    assert_eq!(rotation_from_yaw(270.0), 1);
    assert_eq!(rotation_from_yaw(-90.0), 1);
    assert_eq!(rotation_from_yaw(-180.0), 0);
    // A chest placed by a player facing south (yaw 0) faces north: rotation 2.
    assert_eq!(rotation_of_facing("north"), 2);
    assert_eq!(rotation_of_facing("south"), 0);
    assert_eq!(rotation_of_facing("east"), 3);
    assert_eq!(rotation_of_facing("west"), 1);
    // The vanilla chest facing is the opposite of the player's: yaw 90 (west) -> faces east.
    assert_eq!(rotation_from_yaw(90.0), rotation_of_facing("east"));
    assert_eq!(rotation_from_yaw(270.0), rotation_of_facing("west"));
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn chest_buttons_sit_on_the_lid_and_turn_with_the_chest() {
    // Facing north (rotation 2): the front is z = 0; seen from the front the left button is the east one.
    let left = chest_button(2, 0);
    let right = chest_button(2, 2);
    assert!(near(left[0][0], 0.5 + 2.0 / 16.0) && near(left[1][0], 0.5 + 4.0 / 16.0), "{left:?}");
    assert!(near(right[0][0], 0.5 - 4.0 / 16.0) && near(right[1][0], 0.5 - 2.0 / 16.0), "{right:?}");
    // Z is the 4/16 deep strip around the centre, y the 14/16..15/16 slice above the chest.
    assert!(near(left[0][2], 0.5 - 2.0 / 16.0) && near(left[1][2], 0.5 + 2.0 / 16.0));
    assert!(near(left[0][1], 14.0 / 16.0) && near(left[1][1], 15.0 / 16.0));
    // Facing south (rotation 0): the left button is the west one.
    let left = chest_button(0, 0);
    assert!(near(left[0][0], 0.5 - 4.0 / 16.0), "{left:?}");
    // Facing east (rotation 3): buttons run along z.
    let m = chest_button(3, 1);
    assert!(near(m[0][2], 0.5 - 1.0 / 16.0) && near(m[1][2], 0.5 + 1.0 / 16.0));
    assert!(near(m[0][0], 0.5 - 2.0 / 16.0) && near(m[1][0], 0.5 + 2.0 / 16.0));
    // The latch is on the front: north (rotation 2) -> z 0..1/16; south (0) -> z 15/16..1.
    let l = chest_latch(2);
    assert!(near(l[0][2], 0.0) && near(l[1][2], 0.0625), "{l:?}");
    assert!(near(l[0][0], 0.5 - 0.0625) && near(l[1][0], 0.5 + 0.0625));
    assert!(near(l[0][1], 0.4375) && near(l[1][1], 0.6875));
    let l = chest_latch(0);
    assert!(near(l[0][2], 1.0 - 0.0625) && near(l[1][2], 1.0), "{l:?}");
    let l = chest_latch(3);
    assert!(near(l[0][0], 1.0 - 0.0625) && near(l[1][0], 1.0), "{l:?}");
    let l = chest_latch(1);
    assert!(near(l[0][0], 0.0) && near(l[1][0], 0.0625), "{l:?}");
}

#[test]
fn clicks_find_the_button_the_latch_or_the_chest() {
    let up = [0, 1, 0];
    // North-facing chest, top face at y = 14/16: the middle button is over x = 0.5.
    assert_eq!(chest_sub_hit(2, up, [0.5, 0.875, 0.5], true), 2);
    // Left (east) button at x = 0.5 + 3/16.
    assert_eq!(chest_sub_hit(2, up, [0.5 + 3.0 / 16.0, 0.875, 0.5], true), 1);
    assert_eq!(chest_sub_hit(2, up, [0.5 - 3.0 / 16.0, 0.875, 0.5], true), 3);
    // Beside a button on the lid: the chest.
    assert_eq!(chest_sub_hit(2, up, [0.5 + 1.5 / 16.0, 0.875, 0.5], true), 0);
    assert_eq!(chest_sub_hit(2, up, [0.5, 0.875, 0.2], true), 0);
    // The front face (north, normal -z) at z = 1/16: the latch is at x 0.5, y 0.4375..0.6875.
    assert_eq!(chest_sub_hit(2, [0, 0, -1], [0.5, 0.55, 0.0625], true), 4);
    assert_eq!(chest_sub_hit(2, [0, 0, -1], [0.5, 0.3, 0.0625], true), 0);
    assert_eq!(chest_sub_hit(2, [0, 0, -1], [0.2, 0.55, 0.0625], true), 0);
    // The same click on the back face (south, normal +z) is not the latch.
    assert_eq!(chest_sub_hit(2, [0, 0, 1], [0.5, 0.55, 0.9375], true), 0);
    // A turned chest: facing south the latch is on the south face.
    assert_eq!(chest_sub_hit(0, [0, 0, 1], [0.5, 0.55, 0.9375], true), 4);
    // While the lid is open there are no buttons and no latch.
    assert_eq!(chest_sub_hit(2, up, [0.5, 0.875, 0.5], false), 0);
    assert_eq!(chest_sub_hit(2, [0, 0, -1], [0.5, 0.55, 0.0625], false), 0);
}

#[test]
fn tank_buttons_and_dial() {
    // rot 2 -> quarter turn 0: dial on the north face (z 0.05..0.15), buttons along x on top.
    let b0 = tank_button(2, 0);
    assert!(near(b0[0][0], 0.65 - 0.0375) && near(b0[1][0], 0.65 + 0.0375), "{b0:?}");
    assert!(near(b0[0][1], 0.91) && near(b0[1][1], 0.91 + 0.0375));
    let b2 = tank_button(2, 2);
    assert!(near(b2[0][0], 0.35 - 0.0375));
    let d = tank_dial(2);
    assert!(near(d[0][2], 0.05) && near(d[1][2], 0.15) && near(d[0][0], 0.358) && near(d[1][0], 0.662));
    // rot 0 -> half turn: the dial moves to the south side.
    let d = tank_dial(0);
    assert!(near(d[0][2], 0.85) && near(d[1][2], 0.95), "{d:?}");
    assert_eq!(tank_sub_hit(2, [0, 1, 0], [0.5, 0.875, 0.5]), 2);
    assert_eq!(tank_sub_hit(2, [0, 1, 0], [0.65, 0.875, 0.5]), 1);
    assert_eq!(tank_sub_hit(2, [0, 1, 0], [0.2, 0.875, 0.5]), 0);
    assert_eq!(tank_sub_hit(2, [0, 0, -1], [0.5, 0.4, 0.0625]), 4);
    assert_eq!(tank_sub_hit(2, [0, 0, -1], [0.5, 0.7, 0.0625]), 0);
}

#[test]
fn using_the_block() {
    let f = Frequency::new(Colour::White, Colour::Orange, Colour::Black);
    let me = who();
    // A dye on a button changes only that colour and is used up.
    match use_block(&f, 1, Some("minecraft:blue_dye"), false, &me) {
        Use::Recolour { freq, consume } => {
            assert_eq!(freq.colours(), [Colour::Blue, Colour::Orange, Colour::Black]);
            assert!(consume);
        }
        u => panic!("{u:?}"),
    }
    match use_block(&f, 3, Some("minecraft:blue_dye"), false, &me) {
        Use::Recolour { freq, .. } => assert_eq!(freq.colours(), [Colour::White, Colour::Orange, Colour::Blue]),
        u => panic!("{u:?}"),
    }
    // The same colour as it has is still used (the original's check compares a fresh array and never fires).
    assert!(matches!(
        use_block(&f, 2, Some("minecraft:orange_dye"), false, &me),
        Use::Recolour { consume: true, .. }
    ));
    // A non-dye on a button, or a dye on the body: the block's own action.
    assert_eq!(use_block(&f, 1, Some("minecraft:stick"), false, &me), Use::Pass);
    assert_eq!(use_block(&f, 0, Some("minecraft:red_dye"), false, &me), Use::Pass);
    // A diamond on the latch locks it to the player; a second diamond does nothing.
    let locked = match use_block(&f, 4, Some("minecraft:diamond"), false, &me) {
        Use::Lock { freq, consume } => {
            assert!(consume);
            assert_eq!(freq.owner, Some(me.clone()));
            assert_eq!(freq.colours(), f.colours());
            freq
        }
        u => panic!("{u:?}"),
    };
    assert_eq!(use_block(&locked, 4, Some("minecraft:diamond"), false, &me), Use::Pass);
    // Sneaking on the latch of an owned block gives the owner up (whoever clicks).
    let other = Owner { uuid: [1, 2, 3, 4], name: "Other".into() };
    match use_block(&locked, 4, None, true, &other) {
        Use::Unlock { freq } => assert!(!freq.has_owner()),
        u => panic!("{u:?}"),
    }
    // Sneaking on the latch of an unowned block with a diamond: the diamond locks it (the sneak
    // check only matters when it is owned).
    assert!(matches!(use_block(&f, 4, Some("minecraft:diamond"), true, &me), Use::Lock { .. }));
    assert_eq!(use_block(&f, 4, None, true, &me), Use::Pass);
}

#[test]
fn copies_follow_the_inventory() {
    let freq = Frequency::default();
    let mut store: Store<S> = Store::default();
    let mut a: Vec<Option<S>> = vec![None; 27];
    let mut b: Vec<Option<S>> = vec![None; 27];
    // Two new chests: nothing changes.
    {
        let mut copies = [Working { items: &mut a, fresh: true }, Working { items: &mut b, fresh: true }];
        assert!(!store.reconcile(&freq, &mut copies));
    }
    assert!(!store.dirty);
    // A hopper fills chest A: chest B and the inventory get it.
    a[3] = Some(S("minecraft:iron_ingot", 7));
    {
        let mut copies = [Working { items: &mut a, fresh: false }, Working { items: &mut b, fresh: false }];
        assert!(store.reconcile(&freq, &mut copies));
    }
    assert!(store.dirty);
    assert_eq!(b[3], Some(S("minecraft:iron_ingot", 7)));
    assert_eq!(store.storage(&freq).items[3], Some(S("minecraft:iron_ingot", 7)));
    // A pouch takes the stack out of the inventory itself: both chests lose it.
    store.storage(&freq).items[3] = None;
    {
        let mut copies = [Working { items: &mut a, fresh: false }, Working { items: &mut b, fresh: false }];
        assert!(store.reconcile(&freq, &mut copies));
    }
    assert_eq!((a[3].clone(), b[3].clone()), (None, None));
    // A chest loaded from the world (fresh) never overwrites the inventory with its stale items.
    let mut stale: Vec<Option<S>> = vec![None; 27];
    stale[0] = Some(S("minecraft:dirt", 1));
    {
        let mut copies = [Working { items: &mut stale, fresh: true }];
        assert!(!store.reconcile(&freq, &mut copies));
    }
    assert_eq!(stale[0], None);
    // A copy of another size is never read, only replaced.
    let mut odd: Vec<Option<S>> = vec![Some(S("x", 1)); 9];
    {
        let mut copies = [Working { items: &mut odd, fresh: false }];
        assert!(!store.reconcile(&freq, &mut copies));
    }
    assert_eq!(odd.len(), 27);
    assert!(odd.iter().all(Option::is_none));
    // Another frequency is another inventory.
    let green = Frequency::new(Colour::Green, Colour::Green, Colour::Green);
    assert!(store.storage(&green).items.iter().all(Option::is_none));
}

#[test]
fn a_new_frequency_has_its_own_inventory_and_owners_split_it() {
    let mut store: Store<S> = Store::default();
    let plain = Frequency::default();
    let mut owned = plain.clone();
    owned.owner = Some(who());
    store.storage(&plain).items[0] = Some(S("minecraft:stone", 1));
    assert!(store.storage(&owned).items[0].is_none());
    assert_eq!(store.items.len(), 2);
}

#[test]
fn blocks_are_tracked_by_dimension_and_position() {
    let mut store: Store<S> = Store::default();
    let f = Frequency::new(Colour::Red, Colour::Red, Colour::Red);
    store.set_block("minecraft:overworld", [1, 64, 2], Kind::Chest, f.clone());
    assert!(store.dirty);
    assert_eq!(store.block("minecraft:overworld", [1, 64, 2]), Some(&(Kind::Chest, f.clone())));
    assert_eq!(store.block("minecraft:the_nether", [1, 64, 2]), None);
    assert_eq!(Kind::of("enderstorage:ender_tank"), Some(Kind::Tank));
    assert_eq!(Kind::of("minecraft:ender_chest"), None);
    store.dirty = false;
    assert_eq!(store.remove_block("minecraft:overworld", [1, 64, 2]), Some((Kind::Chest, f)));
    assert!(store.dirty);
    assert_eq!(store.remove_block("minecraft:overworld", [1, 64, 2]), None);
}

#[test]
fn uuid_text_is_java_uuid_to_string() {
    assert_eq!(uuid_string([0, 0, 0, 0]), "00000000-0000-0000-0000-000000000000");
    assert_eq!(uuid_string([-1, -1, -1, -1]), "ffffffff-ffff-ffff-ffff-ffffffffffff");
}

#[test]
fn create_recipes_match_the_three_patterns() {
    let chest = grid([
        "minecraft:blaze_rod", "minecraft:red_wool", "minecraft:blaze_rod",
        "minecraft:obsidian", "minecraft:trapped_chest", "minecraft:obsidian",
        "minecraft:blaze_rod", "minecraft:ender_pearl", "minecraft:blaze_rod",
    ]);
    assert_eq!(create_recipe(&chest), Some(CHEST));
    let tank = grid([
        "minecraft:blaze_rod", "minecraft:lime_wool", "minecraft:blaze_rod",
        "minecraft:obsidian", "minecraft:cauldron", "minecraft:obsidian",
        "minecraft:blaze_rod", "minecraft:ender_pearl", "minecraft:blaze_rod",
    ]);
    assert_eq!(create_recipe(&tank), Some(TANK));
    let pouch = grid([
        "minecraft:blaze_powder", "minecraft:leather", "minecraft:blaze_powder",
        "minecraft:leather", "minecraft:ender_pearl", "minecraft:leather",
        "minecraft:blaze_powder", "minecraft:black_wool", "minecraft:blaze_powder",
    ]);
    assert_eq!(create_recipe(&pouch), Some(POUCH));
    // The wool colour is read by create_frequency, the recipe takes any of the sixteen.
    assert_eq!(create_frequency(&pouch).colours(), [Colour::Black; 3]);
    // A cauldron makes no chest; a chest makes no tank; a wrong item anywhere, or a hole, is nothing.
    let mut bad = chest.clone();
    bad[4] = Some("minecraft:cauldron");
    assert_eq!(create_recipe(&bad), Some(TANK));
    bad[4] = Some("minecraft:barrel");
    assert_eq!(create_recipe(&bad), None);
    let mut bad = chest.clone();
    bad[1] = Some("minecraft:white_carpet");
    assert_eq!(create_recipe(&bad), None);
    let mut bad = tank.clone();
    bad[8] = None;
    assert_eq!(create_recipe(&bad), None);
    assert_eq!(create_recipe(&chest[..4]), None);
}
