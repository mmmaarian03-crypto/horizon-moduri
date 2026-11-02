use horizon_enchantinginfuser::*;

/// The sword's enchantments of 26.3 (weights, max levels, min costs, exclusive sets) and two treasures.
struct Fake;

fn e(id: &str, weight: u32, max: u32, base: i32, per: i32) -> Ench {
    let treasure = matches!(id, "minecraft:mending" | "minecraft:vanishing_curse" | "minecraft:soul_speed");
    Ench {
        id: id.into(),
        rarity: Rarity::from_weight(weight),
        max_level: max,
        min_cost_base: base,
        min_cost_per_level: per,
        curse: id == "minecraft:vanishing_curse",
        treasure,
        tradeable: id != "minecraft:soul_speed",
        discoverable: id != "minecraft:soul_speed",
    }
}

impl Catalog for Fake {
    fn enchantments(&self) -> Vec<Ench> {
        vec![
            e("minecraft:sharpness", 10, 5, 1, 11),
            e("minecraft:smite", 5, 5, 5, 8),
            e("minecraft:bane_of_arthropods", 5, 5, 5, 8),
            e("minecraft:knockback", 5, 2, 5, 20),
            e("minecraft:fire_aspect", 2, 2, 10, 20),
            e("minecraft:looting", 2, 3, 15, 9),
            e("minecraft:sweeping_edge", 2, 3, 5, 9),
            e("minecraft:unbreaking", 5, 3, 5, 8),
            e("minecraft:mending", 2, 1, 0, 25),
            e("minecraft:vanishing_curse", 1, 1, 25, 0),
            e("minecraft:soul_speed", 1, 3, 10, 10),
            e("minecraft:efficiency", 10, 5, 1, 10),
        ]
    }
    fn compatible(&self, a: &str, b: &str) -> bool {
        let damage = ["minecraft:sharpness", "minecraft:smite", "minecraft:bane_of_arthropods"];
        !(a != b && damage.contains(&a) && damage.contains(&b))
    }
    fn applies(&self, ench: &str, item: &str, anvil: bool) -> bool {
        let sword = item == "minecraft:diamond_sword";
        let axe = item == "minecraft:diamond_axe";
        match ench {
            "minecraft:efficiency" => axe,
            "minecraft:soul_speed" => false,
            "minecraft:sharpness" | "minecraft:smite" | "minecraft:bane_of_arthropods" => sword || (anvil && axe),
            "minecraft:unbreaking" | "minecraft:mending" | "minecraft:vanishing_curse" => sword || axe,
            _ => sword,
        }
    }
}

fn sword(damage: u32, enchantments: &[(&str, u32)]) -> Item {
    Item {
        id: "minecraft:diamond_sword".into(),
        count: 1,
        max_stack: 1,
        max_damage: 1561,
        damage,
        enchantments: enchantments.iter().map(|(i, l)| ((*i).into(), *l)).collect(),
        tool_or_armor: true,
    }
}

fn index(s: &Session, id: &str) -> usize {
    s.enchants.iter().position(|e| e.id == id).unwrap()
}

#[test]
fn normal_infuser_offers_table_enchantments_without_treasures() {
    let s = Session::new(&config::NORMAL, &Fake, Some(sword(0, &[])), 15);
    let ids: Vec<&str> = s.enchants.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, [
        "minecraft:sharpness", "minecraft:smite", "minecraft:bane_of_arthropods", "minecraft:knockback",
        "minecraft:fire_aspect", "minecraft:looting", "minecraft:sweeping_edge", "minecraft:unbreaking",
    ]);
    // An axe in the normal one: no sharpness (anvil only); in the advanced one (anvil enchantments on): yes.
    let mut axe = sword(0, &[]);
    axe.id = "minecraft:diamond_axe".into();
    let n = Session::new(&config::NORMAL, &Fake, Some(axe.clone()), 15);
    assert!(!n.enchants.iter().any(|e| e.id == "minecraft:sharpness"));
    let a = Session::new(&config::ADVANCED, &Fake, Some(axe), 15);
    assert!(a.enchants.iter().any(|e| e.id == "minecraft:sharpness"));
}

#[test]
fn what_may_go_in() {
    let book = Item { id: "minecraft:book".into(), count: 1, max_stack: 64, ..Default::default() };
    let ebook = Item { id: "minecraft:enchanted_book".into(), count: 1, max_stack: 1, enchantments: vec![("minecraft:sharpness".into(), 2)], ..Default::default() };
    let stick = Item { id: "minecraft:stick".into(), count: 1, max_stack: 64, ..Default::default() };
    let enchanted = sword(0, &[("minecraft:sharpness", 2)]);
    let worn = sword(10, &[("minecraft:sharpness", 2)]);
    // Normal: unenchanted gear only, no books.
    assert!(may_enchant(&config::NORMAL, &sword(0, &[])));
    assert!(!may_enchant(&config::NORMAL, &enchanted));
    assert!(!may_enchant(&config::NORMAL, &book));
    assert!(!may_enchant(&config::NORMAL, &stick));
    // Advanced: enchanted gear too, books and enchanted books.
    assert!(may_enchant(&config::ADVANCED, &enchanted));
    assert!(may_enchant(&config::ADVANCED, &worn));
    assert!(may_enchant(&config::ADVANCED, &book));
    assert!(may_enchant(&config::ADVANCED, &ebook));
    assert!(!may_enchant(&config::ADVANCED, &stick));
    assert!(!modifiable(Modifiable::FullDurability, &worn));
    // A stick: no list.
    let s = Session::new(&config::ADVANCED, &Fake, Some(stick), 15);
    assert!(s.enchants.is_empty());
}

#[test]
fn levels_unlock_with_power() {
    let s = |power| Session::new(&config::NORMAL, &Fake, Some(sword(0, &[])), power);
    // Common (sharpness): -0.2 * 15 = -3, range 6 over 4 levels.
    let sharp = |p| { let s = s(p); s.max_level(index(&s, "minecraft:sharpness")) };
    assert_eq!(sharp(0), (Some(2), 3));
    assert_eq!(sharp(2), (Some(3), 4));
    assert_eq!(sharp(3), (Some(15), 5));
    assert_eq!(sharp(15), (Some(15), 5));
    // Rare (fire aspect): from 0.4 * 15 = 6, max at 12.
    let fire = |p| { let s = s(p); s.max_level(index(&s, "minecraft:fire_aspect")) };
    assert_eq!(fire(5), (Some(6), 0));
    assert_eq!(fire(6), (Some(12), 1));
    assert_eq!(fire(12), (Some(15), 2));
    // More power than the maximum counts as the maximum.
    assert_eq!(s(40).current_power(), 15);
}

#[test]
fn treasure_and_untradeable_need_their_share_of_power() {
    let mut cfg = config::ADVANCED;
    cfg.allow_treasure = true;
    cfg.allow_untradeable = true;
    cfg.allow_undiscoverable = true;
    cfg.allow_curses = true;
    let mut boots = sword(0, &[]);
    boots.id = "minecraft:diamond_sword".into();
    let s = |p| Session::new(&cfg, &Fake, Some(boots.clone()), p);
    let at = |p: i32, id: &str| { let s = s(p); s.max_level(index(&s, id)) };
    // treasure 0.95 * 15 = 14.25 -> 14; curse 1.0 -> 15.
    assert_eq!(at(13, "minecraft:mending"), (Some(14), 0));
    assert_eq!(at(14, "minecraft:mending"), (None, 1));
    assert_eq!(at(14, "minecraft:vanishing_curse"), (Some(15), 0));
}

#[test]
fn costs_scale_down_to_the_maximum_cost() {
    let mut s = Session::new(&config::NORMAL, &Fake, Some(sword(0, &[])), 15);
    // 15 (smite/bane/sharpness merged, the dearer kept) + 6 + 8 + 12 + 12 + 9.
    assert_eq!(s.total_costs(), 62);
    assert_eq!(s.base_cost, 0);
    let sh = index(&s, "minecraft:sharpness");
    for level in 1..=5 {
        assert_eq!(s.click_level(sh, true), Some(level));
    }
    assert_eq!(s.click_level(sh, true), None, "beyond the max level");
    // round(10 * 30 / 62) = 5, at least the 5 levels.
    assert_eq!(s.enchant_cost, 5);
    assert!(s.changed);
    let fire = index(&s, "minecraft:fire_aspect");
    s.click_level(fire, true);
    s.click_level(fire, true);
    // all = 10 + 8 = 18 -> round(18 * 30 / 62) = 9 (>= 7 levels).
    assert_eq!(s.enchant_cost, 9);
    // Smite clashes with the sharpness now on.
    let smite = index(&s, "minecraft:smite");
    assert_eq!(s.click_level(smite, true), None);
    assert_eq!(s.incompatible_with(smite), vec![sh]);
    assert!(!s.buttons(smite).incr_active);
    // Back to nothing: no change, no cost.
    for _ in 0..5 {
        s.click_level(sh, false);
    }
    s.click_level(fire, false);
    s.click_level(fire, false);
    assert!(!s.changed);
    assert_eq!(s.enchant_cost, 0);
    assert!(!s.can_enchant(100, false));
}

#[test]
fn a_change_costs_at_least_one_and_removing_gives_experience() {
    let mut s = Session::new(&config::ADVANCED, &Fake, Some(sword(0, &[("minecraft:sharpness", 5), ("minecraft:mending", 1)])), 15);
    let sh = index(&s, "minecraft:sharpness");
    assert_eq!(s.levels[sh], 5);
    // Mending is not on offer (treasure not allowed): it stays on the item.
    assert!(!s.enchants.iter().any(|e| e.id == "minecraft:mending"));
    s.click_level(sh, false);
    s.click_level(sh, false);
    // Cheaper than before: a negative cost (experience back).
    assert!(s.enchant_cost < 0, "{}", s.enchant_cost);
    // min cost 45 (V) - 23 (III) = 22 -> ceil(11) = 11 + nextInt(11)
    assert_eq!(s.experience_delta(|n| { assert_eq!(n, 11); 3 }), 14);
    let (id, levels) = s.result().unwrap();
    assert_eq!(id, "minecraft:diamond_sword");
    assert_eq!(levels, vec![("minecraft:sharpness".to_owned(), 3), ("minecraft:mending".to_owned(), 1)]);
}

#[test]
fn shift_click_runs_to_the_limit() {
    let mut s = Session::new(&config::NORMAL, &Fake, Some(sword(0, &[])), 2);
    let sh = index(&s, "minecraft:sharpness");
    assert!(s.click_level_repeat(sh, true, true));
    assert_eq!(s.levels[sh], 4, "power 2 allows IV");
    assert!(!s.click_level_repeat(sh, true, true));
    assert!(s.click_level_repeat(sh, false, false));
    assert_eq!(s.levels[sh], 3);
}

#[test]
fn books_turn_into_enchanted_books_and_back() {
    let book = Item { id: "minecraft:book".into(), count: 1, max_stack: 64, ..Default::default() };
    let mut s = Session::new(&config::ADVANCED, &Fake, Some(book), 15);
    // On a book everything (not treasure) is on offer, and costs are not scaled.
    assert!(s.enchants.iter().any(|e| e.id == "minecraft:efficiency"));
    let eff = index(&s, "minecraft:efficiency");
    s.click_level(eff, true);
    assert_eq!(s.result().unwrap(), ("minecraft:enchanted_book".to_owned(), vec![("minecraft:efficiency".to_owned(), 1)]));
    let ebook = Item { id: "minecraft:enchanted_book".into(), count: 1, max_stack: 1, enchantments: vec![("minecraft:efficiency".into(), 1)], ..Default::default() };
    let mut s = Session::new(&config::ADVANCED, &Fake, Some(ebook), 15);
    let eff = index(&s, "minecraft:efficiency");
    s.click_level(eff, false);
    assert_eq!(s.result().unwrap(), ("minecraft:book".to_owned(), vec![]));
}

#[test]
fn repair_costs_levels_by_quarters() {
    // ceil(ceil(800 / 390.25) * 2) = 6
    assert_eq!(repair_cost(&config::ADVANCED, Some(&sword(800, &[]))), 6);
    assert_eq!(repair_cost(&config::ADVANCED, Some(&sword(1, &[]))), 2);
    assert_eq!(repair_cost(&config::ADVANCED, Some(&sword(0, &[]))), 0);
    assert_eq!(repair_cost(&config::NORMAL, Some(&sword(800, &[]))), 0, "the normal one does not repair");
    let mut elytra = sword(100, &[]);
    elytra.tool_or_armor = false;
    assert!(!repair_allowed(&config::ADVANCED, Some(&elytra)), "TOOLS_AND_ARMOR only");
    let s = Session::new(&config::ADVANCED, &Fake, Some(sword(800, &[])), 15);
    assert!(s.can_repair(6, false));
    assert!(!s.can_repair(5, false));
    assert!(s.can_repair(0, true));
}

#[test]
fn power_counts_shelves_that_see_the_infuser() {
    let offsets = bookshelf_offsets();
    assert_eq!(offsets.len(), 32);
    let full = |_| Spot { bonus: 1.0, ..Default::default() };
    assert_eq!(power(&config::NORMAL, full), 15, "32 shelves, 15 at most");
    // Three shelves, one hidden behind a full block.
    let some = |o: Pos| match o {
        [2, 0, 0] | [-2, 0, 0] => Spot { bonus: 1.0, ..Default::default() },
        [0, 1, 2] => Spot { bonus: 1.0, between_full: true, ..Default::default() },
        _ => Spot::default(),
    };
    assert_eq!(power(&config::NORMAL, some), 2);
    // Chiseled bookshelves: their books / 3, added up first.
    let chiseled = |o: Pos| match o {
        [2, 0, 1] | [2, 0, -1] => Spot { chiseled_books: 4, ..Default::default() },
        _ => Spot::default(),
    };
    assert_eq!(power(&config::NORMAL, chiseled), 2);
    assert!(chiseled_faces_infuser([2, 0, 1], "west"));
    assert!(!chiseled_faces_infuser([2, 0, 1], "east"));
    assert!(chiseled_faces_infuser([0, 1, -2], "south"));
    assert!(!chiseled_faces_infuser([1, 0, 2], "west"), "on the ring but facing along it");
}

#[test]
fn view_round_trips_and_rejects_garbage() {
    let mut s = Session::new(&config::NORMAL, &Fake, Some(sword(0, &[])), 7);
    let sh = index(&s, "minecraft:sharpness");
    s.click_level(sh, true);
    let order = s.sorted(|id| id.to_owned());
    let v = View::of(&s, &order);
    let bytes = v.encode();
    assert_eq!(View::decode(&bytes), Some(v.clone()));
    assert_eq!(View::decode(&bytes[..bytes.len() - 1]), None);
    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(View::decode(&longer), None);
    // Sorted: common first, then by name.
    assert_eq!(v.rows[0].id, "minecraft:sharpness");
    assert!(v.rows.iter().position(|r| r.id == "minecraft:fire_aspect") > v.rows.iter().position(|r| r.id == "minecraft:unbreaking"));
    assert!(v.can_enchant(30, false));
    assert!(!v.can_enchant(0, false));
    assert!(v.can_enchant(0, true));
    assert!(!v.can_repair(30, false), "the normal infuser does not repair");
}

#[test]
fn widgets_and_scrolling() {
    let cfg = &config::ADVANCED;
    let shown = |_| Buttons { decr_visible: true, incr_visible: true, decr_active: true, incr_active: true };
    assert_eq!(widget_at(cfg, 8.0, 45.0, 0, 8, shown), Some(Widget::Enchant));
    assert_eq!(widget_at(cfg, 8.0, 67.0, 0, 8, shown), Some(Widget::Repair));
    assert_eq!(widget_at(&config::NORMAL, 8.0, 56.0, 0, 8, shown), Some(Widget::Enchant));
    assert_eq!(widget_at(&config::NORMAL, 8.0, 80.0, 0, 8, shown), None);
    // Row 1's minus and row 2's plus, scrolled by 3.
    assert_eq!(widget_at(cfg, 30.0, 36.0, 3, 8, shown), Some(Widget::Level { entry: 4, increase: false }));
    assert_eq!(widget_at(cfg, 29.0 + 150.0, 54.0, 3, 8, shown), Some(Widget::Level { entry: 5, increase: true }));
    // The middle of a row is no button.
    assert_eq!(widget_at(cfg, 100.0, 36.0, 0, 8, shown), None);
    // A row past the end.
    assert_eq!(widget_at(cfg, 30.0, 70.0, 0, 2, shown), None);
    assert_eq!(widget_at(cfg, 200.0, 50.0, 0, 8, shown), Some(Widget::Scrollbar));
    assert_eq!(scroll_rows(1.0, 8), 4);
    assert_eq!(scroll_rows(0.5, 8), 2);
    assert_eq!(scroll_rows(1.0, 3), 0);
    assert_eq!(wheel(0.0, -1.0, 8), 0.25);
    assert_eq!(wheel(0.0, 1.0, 8), 0.0);
    assert_eq!(bar_position(17.0 + 7.5), 0.0);
    assert_eq!(bar_position(89.0), 1.0);
}

#[test]
fn slots_are_where_the_screen_draws_them() {
    assert_eq!(slot_position(&config::ADVANCED, CONTAINER_SLOT), Some((8.0, 23.0)));
    assert_eq!(slot_position(&config::NORMAL, CONTAINER_SLOT), Some((8.0, 34.0)));
    assert_eq!(slot_position(&config::NORMAL, 9), Some((30.0, 103.0)));
    assert_eq!(slot_position(&config::NORMAL, 35), Some((174.0, 139.0)));
    assert_eq!(slot_position(&config::NORMAL, 0), Some((30.0, 161.0)));
    assert_eq!(slot_position(&config::NORMAL, 301), None);
}

#[derive(Clone, Debug, PartialEq)]
struct S(&'static str, u32);
impl Stack for S {
    fn count(&self) -> u32 {
        self.1
    }
    fn set_count(&mut self, count: u32) {
        self.1 = count;
    }
    fn same_kind(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[test]
fn the_slot_holds_one_item_and_nothing_is_lost() {
    let empty: Vec<Option<S>> = vec![None; 45];
    // A click put a stack of 16 books in: one stays, 15 back on the cursor.
    let mut after = empty.clone();
    after[0] = Some(S("book", 16));
    let (out, carried) = settle_one(&empty, &after, &None, None).unwrap();
    assert_eq!(out[0], Some(S("book", 1)));
    assert_eq!(carried, Some(S("book", 15)));
    // A Shift-click from slot 20 spread 64 over the slot and the stand-ins: one stays, the rest goes back.
    let mut before = empty.clone();
    before[20] = Some(S("book", 64));
    let mut after = empty.clone();
    after[0] = Some(S("book", 64));
    let (out, carried) = settle_one(&before, &after, &None, Some(20)).unwrap();
    assert_eq!(out[0], Some(S("book", 1)));
    assert_eq!(out[20], Some(S("book", 63)));
    assert_eq!(carried, None);
    // Stand-ins filled, cursor holding something else, no source: refused.
    let mut after = empty.clone();
    after[3] = Some(S("book", 2));
    assert!(settle_one(&empty, &after, &Some(S("stick", 1)), None).is_none());
    // Total items never change.
    let total = |v: &[Option<S>], c: &Option<S>| v.iter().flatten().map(|s| s.1).sum::<u32>() + c.as_ref().map_or(0, |s| s.1);
    let mut after = empty.clone();
    after[0] = Some(S("sword", 1));
    let (out, carried) = settle_one(&empty, &after, &Some(S("book", 3)), None).unwrap();
    assert_eq!(total(&out, &carried), 4);
}

#[test]
fn block_tooltips() {
    assert_eq!(block_tooltip_keys(&config::NORMAL), ("block.enchantinginfuser.description.choose", false));
    assert_eq!(block_tooltip_keys(&config::ADVANCED), ("block.enchantinginfuser.description.chooseAndModify", true));
}

#[test]
fn carry_is_an_enchanting_table_with_the_screen_texture_in_the_block_atlas() {
    let carry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/mods/enchantinginfuser/carry");
    if !carry.is_dir() {
        return;
    }
    for id in ["enchanting_infuser", "advanced_enchanting_infuser"] {
        let decl = std::fs::read_to_string(carry.join(format!("data/enchantinginfuser/horizon/block/{id}.json"))).unwrap();
        assert!(decl.contains("\"minecraft:enchanting_table\""), "{decl}");
        assert!(decl.contains("\"light\": 7"), "{decl}");
    }
    assert!(carry.join("assets/enchantinginfuser/textures/block/gui_enchanting_infuser.png").is_file());
    assert!(!carry.join("data/enchantinginfuser/datapacks").exists());
    // The helpers name the same blocks as the rules.
    let shared = include_str!("../mixins/enchantinginfuser/shared.rs");
    assert!(shared.contains(NORMAL) && shared.contains(ADVANCED));
    let pre_remove = include_str!("../mixins/enchantinginfuser/pre_remove.rs");
    assert!(pre_remove.contains(NORMAL) && pre_remove.contains(ADVANCED) && pre_remove.contains("slots: 1"));
}
