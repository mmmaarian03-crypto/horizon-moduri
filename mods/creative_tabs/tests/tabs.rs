use horizon_creative_tabs::*;
use std::collections::{BTreeMap, HashMap, HashSet};

fn s(v: &str) -> String {
    v.to_owned()
}
fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}
fn lang(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (s(k), s(v))).collect()
}
fn mod_info(id: &str, name: &str, extra: &[&str]) -> ModInfo {
    let mut namespaces = vec![s(id)];
    namespaces.extend(extra.iter().map(|x| s(x)));
    ModInfo {
        id: s(id),
        name: s(name),
        namespaces,
    }
}

/// A fake `items.json`: vanilla's items, then the block items of the mods (mod by mod, blocks
/// in id order), then the declared items: the order the content root writes.
fn registry() -> Vec<String> {
    strings(&[
        "minecraft:stone",
        "minecraft:oak_planks",
        "minecraft:diamond",
        "zeta:zeta_block",
        "alpha:b_stairs",
        "alpha:a_slab",
        "alpha:c_wall",
        "mid:one",
        "alpha:wrench",
        "mid:two",
        "ghost:thing",
        "alpha:a_slab",
        "tt:tower_stone",
        "kaisyn:castle_stone",
    ])
}

fn mods() -> Vec<ModInfo> {
    vec![
        mod_info("zeta", "Zeta Things", &[]),
        mod_info("alpha", "Alpha's Blocks", &[]),
        mod_info("mid", "Mid", &[]),
        mod_info("tt", "Towers", &["kaisyn"]),
    ]
}

// --- the registry's keys ---------------------------------------------------------------------

#[test]
fn top_level_keys_come_in_file_order_without_reading_the_values() {
    let json = br#" {"minecraft:stone": {"a": [1, 2, {"x": "}"}], "b": "q\"uote"},
        "mod:item_2":1, "mod:item_1": "text", "z:flag": true, "z:none": null, "z:neg": -1.5e3,
        "z:last": []} "#;
    assert_eq!(
        top_level_keys(json),
        strings(&[
            "minecraft:stone",
            "mod:item_2",
            "mod:item_1",
            "z:flag",
            "z:none",
            "z:neg",
            "z:last"
        ])
    );
}

#[test]
fn top_level_keys_decode_escapes_and_skip_a_byte_order_mark() {
    let mut json = vec![0xEF, 0xBB, 0xBF];
    json.extend_from_slice(br#"{"a\u0062c":1,"tab\tnew\\line\/":2,"\ud83d\ude00":3,"":4}"#);
    assert_eq!(
        top_level_keys(&json),
        strings(&["abc", "tab\tnew\\line/", "\u{1F600}", ""])
    );
    // Raw UTF-8 passes through.
    assert_eq!(top_level_keys("{\"caf\u{e9}:x\":0}".as_bytes()), strings(&["caf\u{e9}:x"]));
}

#[test]
fn top_level_keys_stop_at_what_they_cannot_follow() {
    assert!(top_level_keys(b"").is_empty());
    assert!(top_level_keys(b"[1,2]").is_empty());
    assert!(top_level_keys(b"{}").is_empty());
    assert!(top_level_keys(b"  {  }  ").is_empty());
    // A cut-off document keeps the keys whose values were complete.
    assert_eq!(top_level_keys(br#"{"a":1,"b":{"c":2},"d":{"e""#), strings(&["a", "b"]));
    assert_eq!(top_level_keys(br#"{"a":1,"b""#), strings(&["a"]));
    assert_eq!(top_level_keys(br#"{"a":1 "b":2}"#), strings(&["a"]));
    assert_eq!(top_level_keys(br#"{"a":,"b":2}"#), Vec::<String>::new());
    // A bad escape ends it.
    assert_eq!(top_level_keys(br#"{"a":1,"b\q":2}"#), strings(&["a"]));
}

#[test]
fn top_level_keys_read_a_large_registry_quickly_and_in_order() {
    let mut json = String::from("{");
    for i in 0..50_000 {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "\"chipped:block_{i}\":{{\"minecraft:item_name\":{{\"translate\":\"block.chipped.block_{i}\"}},\"x\":[1,2,3]}}"
        ));
    }
    json.push('}');
    let keys = top_level_keys(json.as_bytes());
    assert_eq!(keys.len(), 50_000);
    assert_eq!(keys[0], "chipped:block_0");
    assert_eq!(keys[49_999], "chipped:block_49999");
}

// --- items into groups -----------------------------------------------------------------------

#[test]
fn groups_hold_one_mods_items_in_registry_order() {
    let content = strings(&["alpha", "mid", "zeta", "tt", "kaisyn"]);
    let groups = group_items(&registry(), &content, &mods());
    let keys: Vec<&str> = groups.iter().map(|g| g.key.as_str()).collect();
    // Mod id order.
    assert_eq!(keys, ["alpha", "mid", "tt", "zeta"]);
    // Registry order inside a mod (the block items first, then the declared ones), no repeats.
    assert_eq!(
        groups[0].items,
        strings(&["alpha:b_stairs", "alpha:a_slab", "alpha:c_wall", "alpha:wrench"])
    );
    assert_eq!(groups[1].items, strings(&["mid:one", "mid:two"]));
    // A mod's listed namespaces join its group.
    assert_eq!(
        groups[2].items,
        strings(&["tt:tower_stone", "kaisyn:castle_stone"])
    );
    assert_eq!(groups[2].namespaces, strings(&["tt", "kaisyn"]));
    assert_eq!(groups[3].name, "Zeta Things");
}

#[test]
fn vanilla_and_namespaces_no_content_mod_owns_are_not_tabs() {
    let content = strings(&["alpha"]);
    let groups = group_items(&registry(), &content, &mods());
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].key, "alpha");
    // `ghost` is not in the root's list: its item has no tab.
    assert!(group_items(&registry(), &strings(&["mid"]), &mods())[0].items.len() == 2);
    // No content mods: no groups at all.
    assert!(group_items(&registry(), &[], &mods()).is_empty());
    assert!(group_items(&[], &content, &mods()).is_empty());
}

#[test]
fn a_namespace_no_manifest_claims_gets_its_own_group() {
    let groups = group_items(&registry(), &strings(&["ghost", "mid"]), &mods());
    let ghost = groups.iter().find(|g| g.key == "ghost").unwrap();
    assert_eq!(ghost.items, strings(&["ghost:thing"]));
    assert_eq!(ghost.name, "");
    assert_eq!(group_title(ghost, &BTreeMap::new()), "Ghost");
}

// --- titles and icons ------------------------------------------------------------------------

fn group(key: &str, name: &str, extra: &[&str], items: &[&str]) -> Group {
    let mut namespaces = vec![s(key)];
    namespaces.extend(extra.iter().map(|x| s(x)));
    Group {
        key: s(key),
        name: s(name),
        namespaces,
        items: strings(items),
    }
}

#[test]
fn a_title_is_the_mods_own_item_group_key() {
    let g = group("mcwdoors", "Macaw's Doors", &[], &["mcwdoors:oak_door"]);
    assert_eq!(
        group_title(&g, &lang(&[("itemGroup.mcwdoors", "Macaw's Doors (lang)")])),
        "Macaw's Doors (lang)"
    );
    // comforts has the key twice, plain and ".general"; the plain one wins.
    let g = group("comforts", "Comforts", &[], &[]);
    assert_eq!(
        group_title(
            &g,
            &lang(&[("itemGroup.comforts", "Comforts"), ("itemGroup.comforts.general", "General")])
        ),
        "Comforts"
    );
    // The other key spellings.
    let g = group("hexxit", "Hexxit", &[], &[]);
    assert_eq!(
        group_title(&g, &lang(&[("item_group.hexxit", "Hexxit Gear")])),
        "Hexxit Gear"
    );
    let g = group("volt", "Volt", &[], &[]);
    assert_eq!(group_title(&g, &lang(&[("creativetab.volt", "Voltaic")])), "Voltaic");
    // A colour code stays: the screen draws it.
    let g = group("enigmatic", "E", &[], &[]);
    assert_eq!(
        group_title(&g, &lang(&[("itemGroup.enigmatic", "\u{a7}5Enigmatic Legacy")])),
        "\u{a7}5Enigmatic Legacy"
    );
    // Blank values do not count.
    let g = group("blank", "Blank Mod", &[], &[]);
    assert_eq!(group_title(&g, &lang(&[("itemGroup.blank", "  ")])), "Blank Mod");
}

#[test]
fn a_single_sub_key_names_the_tab() {
    // Chipped, Ecologics: `itemGroup.<ns>.main` / `.tab`.
    let g = group("chipped", "Chipped", &[], &[]);
    assert_eq!(group_title(&g, &lang(&[("itemGroup.chipped.main", "Chipped")])), "Chipped");
    let g = group("ecologics", "Ecologics (manifest)", &[], &[]);
    assert_eq!(
        group_title(&g, &lang(&[("itemGroup.ecologics.tab", "Ecologics")])),
        "Ecologics"
    );
    // Structure Gel: the access note is not a title.
    let g = group("structure_gel", "Structure Gel", &[], &[]);
    let l = lang(&[
        ("itemGroup.structure_gel.structure_items", "Structure Items"),
        (
            "itemGroup.structure_gel.structure_items.no_access",
            "You need access to commands to access items in this tab.",
        ),
    ]);
    assert_eq!(group_title(&g, &l), "Structure Items");
}

#[test]
fn several_sub_keys_pick_the_main_one_or_fall_back_to_the_mods_name() {
    // Statues has a blocks tab and an items tab and one tab here: the mod's name.
    let g = group("statues", "Statues", &[], &[]);
    let l = lang(&[
        ("itemGroup.statues.blocks", "Statue Blocks"),
        ("itemGroup.statues.items", "Statue Items"),
    ]);
    assert_eq!(group_title(&g, &l), "Statues");
    // With a main tab among them, that one.
    let g = group("multi", "Multi", &[], &[]);
    let l = lang(&[
        ("itemGroup.multi.extras", "Multi Extras"),
        ("itemGroup.multi.main", "Multi Main"),
    ]);
    assert_eq!(group_title(&g, &l), "Multi Main");
    // Another mod's keys are not looked at.
    let g = group("other", "Other One", &[], &[]);
    assert_eq!(group_title(&g, &l), "Other One");
    // `itemGroup.multi` must not match `itemGroup.multiplayer`.
    let g = group("multi", "Multi Name", &[], &[]);
    assert_eq!(
        group_title(&g, &lang(&[("itemGroup.multiplayer", "Wrong")])),
        "Multi Name"
    );
}

#[test]
fn a_listed_namespace_can_carry_the_title() {
    let g = group("t_and_t", "Towns and Towers", &["towns_and_towers"], &[]);
    assert_eq!(
        group_title(&g, &lang(&[("itemGroup.towns_and_towers", "Towns & Towers")])),
        "Towns & Towers"
    );
    // The mod's own key comes before a listed namespace's.
    assert_eq!(
        group_title(
            &g,
            &lang(&[
                ("itemGroup.towns_and_towers", "Second"),
                ("itemGroup.t_and_t", "First")
            ])
        ),
        "First"
    );
}

#[test]
fn without_a_key_or_a_manifest_the_id_is_made_readable() {
    assert_eq!(pretty_name("mr_dungeons_andtaverns"), "Mr Dungeons Andtaverns");
    assert_eq!(pretty_name("structure-gel"), "Structure Gel");
    assert_eq!(pretty_name("x"), "X");
    assert_eq!(pretty_name("__"), "");
    let g = group("mr_dungeons_andtaverns", "", &[], &[]);
    assert_eq!(group_title(&g, &BTreeMap::new()), "Mr Dungeons Andtaverns");
}

#[test]
fn the_icon_is_the_hinted_item_the_mod_has_else_its_first() {
    let g = group(
        "ironchest",
        "Iron Chests",
        &[],
        &["ironchest:copper_chest", "ironchest:iron_chest"],
    );
    assert_eq!(group_icon(&g, ICON_HINTS), "ironchest:iron_chest");
    // A hinted item the port does not have: the first item.
    let g = group("torchmaster", "Torchmaster", &[], &["torchmaster:dreadlamp"]);
    assert_eq!(group_icon(&g, ICON_HINTS), "torchmaster:dreadlamp");
    let g = group("chipped", "Chipped", &[], &["chipped:acacia_barrel", "chipped:x"]);
    assert_eq!(group_icon(&g, ICON_HINTS), "chipped:acacia_barrel");
    assert_eq!(group_icon(&g, &[]), "chipped:acacia_barrel");
    assert_eq!(group_icon(&group("e", "", &[], &[]), &[]), "");
}

// --- pages -----------------------------------------------------------------------------------

#[test]
fn slots_fill_five_on_top_then_five_on_the_bottom_page_by_page() {
    let at = |i| {
        let t = slot_of(i);
        (t.page, t.top, t.column)
    };
    assert_eq!(at(0), (1, true, 0));
    assert_eq!(at(4), (1, true, 4));
    assert_eq!(at(5), (1, false, 0));
    assert_eq!(at(9), (1, false, 4));
    assert_eq!(at(10), (2, true, 0));
    assert_eq!(at(19), (2, false, 4));
    assert_eq!(at(20), (3, true, 0));
    // Every slot is a distinct (page, row, column).
    let all: HashSet<(usize, bool, u32)> = (0..100).map(at).collect();
    assert_eq!(all.len(), 100);
    // The columns are the ones tab sprites exist for.
    assert!((0..100).all(|i| slot_of(i).column < 5));
}

#[test]
fn page_counts() {
    assert_eq!(page_count(0), 1);
    assert_eq!(page_count(1), 2);
    assert_eq!(page_count(10), 2);
    assert_eq!(page_count(11), 3);
    assert_eq!(page_count(25), 4);
    assert_eq!(page_count(40), 5);
}

/// Vanilla's tabs as the screen has them (hotbar and operator tabs left out, as it does).
fn vanilla_tabs() -> Vec<(&'static str, u32, bool, bool)> {
    vec![
        ("minecraft:building_blocks", 0, false, false),
        ("minecraft:colored_blocks", 1, false, false),
        ("minecraft:natural_blocks", 2, false, false),
        ("minecraft:functional_blocks", 3, false, false),
        ("minecraft:redstone_blocks", 4, false, false),
        ("minecraft:search", 6, true, false),
        ("minecraft:tools_and_utilities", 0, false, false),
        ("minecraft:combat", 1, false, false),
        ("minecraft:food_and_drinks", 2, false, false),
        ("minecraft:ingredients", 3, false, false),
        ("minecraft:spawn_eggs", 4, false, false),
        ("minecraft:inventory", 6, false, true),
    ]
}
fn metas(tabs: &[(&'static str, u32, bool, bool)]) -> Vec<TabMeta<'static>> {
    tabs.iter()
        .map(|&(id, column, search, inventory)| TabMeta {
            id,
            column,
            search,
            inventory,
        })
        .collect()
}
fn planned(n: usize) -> Vec<PlannedTab> {
    (0..n)
        .map(|i| PlannedTab {
            id: tab_id(&format!("mod{i:02}")),
            title: format!("Mod {i}"),
            icon: format!("mod{i:02}:icon"),
            items: vec![format!("mod{i:02}:item")],
            slot: slot_of(i),
        })
        .collect()
}

#[test]
fn without_content_mods_the_screen_is_vanilla() {
    let tabs = vanilla_tabs();
    let merge = merge_layout(&metas(&tabs), &[]);
    assert_eq!(merge.pages, 1);
    assert!(merge.tab_page.is_empty());
    assert_eq!(merge.insert_at, tabs.len());
    let mut paging = Paging::new(&merge);
    assert!(!paging.paged());
    assert!(tabs.iter().all(|t| paging.visible(t.0)));
    // No arrows to press, no page to turn.
    assert_eq!(paging.arrow_at(5.0, -40.0), None);
    assert!(!paging.press(5.0, -40.0));
    assert!(!paging.step(1));
    assert_eq!(paging.page(), 0);
    assert_eq!(paging.label(), "1 / 1");
    // The empty paging the game starts with is the same.
    let empty = Paging::empty();
    assert!(empty.visible("anything") && !empty.paged());
    assert_eq!(empty, Paging::new(&merge));
}

#[test]
fn mod_tabs_go_after_vanillas_paged_tabs_and_before_the_fixed_ones() {
    let tabs = vanilla_tabs();
    let merge = merge_layout(&metas(&tabs), &planned(3));
    // After spawn eggs (index 10), before the survival inventory (11).
    assert_eq!(merge.insert_at, 11);
    assert_eq!(merge.pages, 2);
    // A vanilla layout where the fixed tab is last in a short list.
    let short = [("a", 0, false, false), ("search", 6, true, false)];
    assert_eq!(merge_layout(&metas(&short), &planned(1)).insert_at, 1);
    // No paged tab at all: the front.
    let only_fixed = [("search", 6, true, false)];
    assert_eq!(merge_layout(&metas(&only_fixed), &planned(1)).insert_at, 0);
    assert_eq!(merge_layout(&[], &planned(1)).insert_at, 0);
}

#[test]
fn each_page_shows_its_own_tabs_and_the_fixed_ones_everywhere() {
    let tabs = vanilla_tabs();
    let plan = planned(25);
    let merge = merge_layout(&metas(&tabs), &plan);
    assert_eq!(merge.pages, 4);
    let mut paging = Paging::new(&merge);
    let vanilla_paged: Vec<&str> = tabs
        .iter()
        .filter(|t| !t.2 && !t.3)
        .map(|t| t.0)
        .collect();
    assert_eq!(vanilla_paged.len(), 10);
    let fixed = ["minecraft:search", "minecraft:inventory"];
    // Page 1: vanilla's, none of the mods'.
    assert_eq!(paging.page(), 0);
    assert!(vanilla_paged.iter().all(|id| paging.visible(id)));
    assert!(fixed.iter().all(|id| paging.visible(id)));
    assert!(plan.iter().all(|p| !paging.visible(&p.id)));
    // Every later page: only its ten (five on the last), plus the fixed ones.
    for page in 1..4 {
        assert!(paging.set_page(page));
        let shown: Vec<usize> = (0..25).filter(|&i| paging.visible(&plan[i].id)).collect();
        let expected: Vec<usize> = (0..25).filter(|&i| slot_of(i).page == page).collect();
        assert_eq!(shown, expected);
        assert_eq!(shown.len(), if page == 3 { 5 } else { 10 });
        assert!(vanilla_paged.iter().all(|id| !paging.visible(id)));
        assert!(fixed.iter().all(|id| paging.visible(id)));
    }
    // A tab the screen has never heard of is not hidden.
    assert!(paging.visible("someone:else"));
}

#[test]
fn turning_pages_stops_at_the_ends() {
    let merge = merge_layout(&metas(&vanilla_tabs()), &planned(25));
    let mut p = Paging::new(&merge);
    assert!(!p.step(-1));
    assert_eq!(p.page(), 0);
    assert!(p.step(1) && p.step(1) && p.step(1));
    assert_eq!(p.page(), 3);
    assert!(!p.step(1));
    assert_eq!(p.page(), 3);
    assert!(p.step(-1));
    assert_eq!(p.page(), 2);
    assert!(p.step(-10));
    assert_eq!(p.page(), 0);
    // set_page keeps within the pages.
    assert!(p.set_page(99));
    assert_eq!(p.page(), 3);
    assert!(!p.set_page(99));
    assert_eq!(p.label(), "4 / 4");
    p.set_page(0);
    assert_eq!(p.label(), "1 / 4");
}

#[test]
fn a_tab_brings_its_page_up() {
    let plan = planned(25);
    let merge = merge_layout(&metas(&vanilla_tabs()), &plan);
    let mut p = Paging::new(&merge);
    // The pad selects mod tab 13 (page 2): its page comes up.
    assert!(p.show(&plan[13].id));
    assert_eq!(p.page(), 2);
    assert!(p.visible(&plan[13].id));
    // Already there: no change.
    assert!(!p.show(&plan[12].id));
    // Fixed and unknown tabs leave the page alone.
    assert!(!p.show("minecraft:search"));
    assert!(!p.show("minecraft:inventory"));
    assert!(!p.show("nobody:nothing"));
    assert_eq!(p.page(), 2);
    // A vanilla tab goes back to page 1.
    assert!(p.show("minecraft:combat"));
    assert_eq!(p.page(), 0);
}

#[test]
fn the_arrows_sit_over_the_screens_corners_and_turn_the_page_when_clicked() {
    let merge = merge_layout(&metas(&vanilla_tabs()), &planned(25));
    let mut p = Paging::new(&merge);
    assert_eq!(arrow_rect(Arrow::Previous), [0.0, -50.0, 20.0, 20.0]);
    assert_eq!(arrow_rect(Arrow::Next), [175.0, -50.0, 20.0, 20.0]);
    // Edges: the first pixel in and the first pixel out.
    assert_eq!(p.arrow_at(0.0, -50.0), Some(Arrow::Previous));
    assert_eq!(p.arrow_at(19.9, -30.1), Some(Arrow::Previous));
    assert_eq!(p.arrow_at(20.0, -40.0), None);
    assert_eq!(p.arrow_at(10.0, -30.0), None);
    assert_eq!(p.arrow_at(10.0, -50.1), None);
    assert_eq!(p.arrow_at(-0.1, -40.0), None);
    assert_eq!(p.arrow_at(175.0, -50.0), Some(Arrow::Next));
    assert_eq!(p.arrow_at(194.9, -30.1), Some(Arrow::Next));
    assert_eq!(p.arrow_at(195.0, -40.0), None);
    assert_eq!(p.arrow_at(174.9, -40.0), None);
    // Nothing else of the screen is an arrow: the tabs, the panel, the label.
    assert_eq!(p.arrow_at(97.0, -44.0), None);
    assert_eq!(p.arrow_at(30.0, -10.0), None);
    assert_eq!(p.arrow_at(100.0, 50.0), None);
    // Clicking turns the page; at the end the click is still the button's but nothing turns.
    assert!(p.press(185.0, -40.0));
    assert_eq!(p.page(), 1);
    assert!(p.press(5.0, -45.0));
    assert_eq!(p.page(), 0);
    assert!(p.press(5.0, -45.0));
    assert_eq!(p.page(), 0);
    assert!(!p.press(100.0, 20.0));
    for _ in 0..10 {
        p.press(185.0, -40.0);
    }
    assert_eq!(p.page(), 3);
}

#[test]
fn the_label_is_centred_over_the_screen() {
    // drawCenteredString at the middle of the 195 wide screen.
    assert_eq!(label_x(0.0), 97.0);
    assert_eq!(label_x(23.0), 86.0);
    assert_eq!(label_x(195.0), 0.0);
    assert_eq!(LABEL_TOP, -44.0);
    // The label sits between the buttons.
    let w = 30.0;
    let x = label_x(w);
    assert!(x > arrow_rect(Arrow::Previous)[0] + ARROW_SIZE);
    assert!(x + w < arrow_rect(Arrow::Next)[0]);
}

#[test]
fn off_screen_is_out_of_every_tabs_reach() {
    // Whatever the tab and the screen's place, a tab at OFF_SCREEN is neither drawn nor hit:
    // its 26 by 32 rectangle is far from any window.
    assert!(OFF_SCREEN.0 + 26.0 < -1000.0 && OFF_SCREEN.1 + 32.0 < -1000.0);
}

// --- planning ---------------------------------------------------------------------------------

#[test]
fn tabs_are_planned_with_titles_icons_items_and_slots() {
    let content = strings(&["alpha", "mid", "zeta", "tt", "kaisyn"]);
    let l = lang(&[
        ("itemGroup.alpha", "Alpha Blocks"),
        ("itemGroup.mid.main", "Middle"),
        ("itemGroup.kaisyn", "Kaisyn Castles"),
    ]);
    let plan = plan_tabs(&registry(), &content, &mods(), &l, &[("mid", "mid:two")]);
    let summary: Vec<(String, String, String, usize)> = plan
        .iter()
        .map(|p| (p.id.clone(), p.title.clone(), p.icon.clone(), p.items.len()))
        .collect();
    assert_eq!(
        summary,
        vec![
            (s("alpha:creative_tab"), s("Alpha Blocks"), s("alpha:b_stairs"), 4),
            (s("mid:creative_tab"), s("Middle"), s("mid:two"), 2),
            (s("tt:creative_tab"), s("Kaisyn Castles"), s("tt:tower_stone"), 2),
            (s("zeta:creative_tab"), s("Zeta Things"), s("zeta:zeta_block"), 1),
        ]
    );
    let slots: Vec<(usize, bool, u32)> = plan
        .iter()
        .map(|p| (p.slot.page, p.slot.top, p.slot.column))
        .collect();
    assert_eq!(slots, [(1, true, 0), (1, true, 1), (1, true, 2), (1, true, 3)]);
    // A tab's id never clashes with a vanilla tab's.
    assert!(plan.iter().all(|p| !p.id.starts_with("minecraft:")));
}

#[test]
fn a_big_pack_plans_a_tab_per_mod_over_several_pages() {
    // 33 content mods, as the pack, each with a few items among vanilla's.
    let mut ids = strings(&["minecraft:stone", "minecraft:dirt"]);
    let mut content = Vec::new();
    let mut manifests = Vec::new();
    for i in 0..33 {
        let id = format!("m{i:02}");
        for j in 0..3 {
            ids.push(format!("{id}:item_{j}"));
        }
        manifests.push(mod_info(&id, &format!("Mod {i}"), &[]));
        content.push(id);
    }
    let plan = plan_tabs(&ids, &content, &manifests, &BTreeMap::new(), &[]);
    assert_eq!(plan.len(), 33);
    assert_eq!(plan[0].id, "m00:creative_tab");
    assert_eq!(plan[32].id, "m32:creative_tab");
    assert!(plan.iter().all(|p| p.items.len() == 3));
    assert_eq!(page_count(plan.len()), 5);
    let merge = merge_layout(&metas(&vanilla_tabs()), &plan);
    assert_eq!(merge.pages, 5);
    // Every tab is on exactly one page, and no two tabs of a page share a place.
    let mut places: HashSet<(usize, bool, u32)> = HashSet::new();
    for p in &plan {
        assert!(places.insert((p.slot.page, p.slot.top, p.slot.column)));
        assert_eq!(merge.tab_page[&p.id], p.slot.page);
    }
    // The vanilla tabs of page 1 and the first page of mods share their places, never a page.
    assert!(plan.iter().all(|p| p.slot.page >= 1));
}

#[test]
fn nothing_installed_plans_nothing() {
    assert!(plan_tabs(&registry(), &[], &mods(), &BTreeMap::new(), ICON_HINTS).is_empty());
    assert!(plan_tabs(&strings(&["minecraft:stone"]), &strings(&["alpha"]), &mods(), &BTreeMap::new(), ICON_HINTS).is_empty());
}

// --- the search tab ---------------------------------------------------------------------------

fn tag_table(rows: &[(&str, &[&str])]) -> HashMap<String, HashSet<String>> {
    rows.iter()
        .map(|(tag, items)| (s(tag), items.iter().map(|i| s(i)).collect()))
        .collect()
}

#[test]
fn every_mod_item_reaches_the_search_tab_in_tab_order() {
    let content = strings(&["alpha", "mid", "zeta", "tt", "kaisyn"]);
    let plan = plan_tabs(&registry(), &content, &mods(), &BTreeMap::new(), &[]);
    let rows = search_rows(&plan, &HashMap::new());
    let items: Vec<&str> = rows.iter().map(|r| r.item.as_str()).collect();
    assert_eq!(
        items,
        [
            "alpha:b_stairs",
            "alpha:a_slab",
            "alpha:c_wall",
            "alpha:wrench",
            "mid:one",
            "mid:two",
            "tt:tower_stone",
            "kaisyn:castle_stone",
            "zeta:zeta_block"
        ]
    );
    // As many rows as items in the tabs: none lost, none repeated.
    let in_tabs: usize = plan.iter().map(|p| p.items.len()).sum();
    assert_eq!(rows.len(), in_tabs);
    assert!(rows.iter().all(|r| r.tags.is_empty()));
}

#[test]
fn mod_items_carry_their_item_tags_for_hash_searches() {
    let content = strings(&["alpha", "mid"]);
    let plan = plan_tabs(&registry(), &content, &mods(), &BTreeMap::new(), &[]);
    let tags = tag_table(&[
        ("minecraft:slabs", &["minecraft:stone_slab", "alpha:a_slab"]),
        ("minecraft:stairs", &["alpha:b_stairs"]),
        ("c:tools/wrenches", &["alpha:wrench", "othermod:wrench"]),
        ("alpha:walls", &["alpha:c_wall", "alpha:a_slab"]),
        ("minecraft:unrelated", &["minecraft:dirt"]),
    ]);
    let rows = search_rows(&plan, &tags);
    let of = |item: &str| {
        rows.iter()
            .find(|r| r.item == item)
            .map(|r| r.tags.clone())
            .unwrap()
    };
    // Sorted by tag id, so the index does not depend on the map's order.
    assert_eq!(of("alpha:a_slab"), strings(&["alpha:walls", "minecraft:slabs"]));
    assert_eq!(of("alpha:b_stairs"), strings(&["minecraft:stairs"]));
    assert_eq!(of("alpha:wrench"), strings(&["c:tools/wrenches"]));
    assert_eq!(of("alpha:c_wall"), strings(&["alpha:walls"]));
    assert!(of("mid:one").is_empty());
    // Tags of items no tab lists (vanilla's, other mods') add no row.
    assert_eq!(rows.len(), 6);
}

#[test]
fn the_search_rows_are_the_same_whatever_order_the_tag_map_iterates_in() {
    let content = strings(&["alpha"]);
    let plan = plan_tabs(&registry(), &content, &mods(), &BTreeMap::new(), &[]);
    let names = ["t:a", "t:b", "t:c", "t:d", "t:e", "t:f", "t:g", "t:h"];
    let first = {
        let tags: HashMap<String, HashSet<String>> = names
            .iter()
            .map(|n| (s(n), HashSet::from([s("alpha:a_slab")])))
            .collect();
        search_rows(&plan, &tags)
    };
    for _ in 0..20 {
        let tags: HashMap<String, HashSet<String>> = names
            .iter()
            .rev()
            .map(|n| (s(n), HashSet::from([s("alpha:a_slab")])))
            .collect();
        assert_eq!(search_rows(&plan, &tags), first);
    }
    let slab = first.iter().find(|r| r.item == "alpha:a_slab").unwrap();
    assert_eq!(slab.tags, strings(&names));
}

// --- the whole build, as the game does it ----------------------------------------------------

/// The screen's tab list, as the adapter edits it.
#[derive(Clone, Debug, PartialEq)]
struct ScreenTab {
    id: String,
    column: u32,
    search: bool,
    inventory: bool,
}

#[test]
fn building_the_screen_from_a_registry_puts_every_tab_where_it_should_be() {
    // Vanilla's tabs, a registry with vanilla's items and 25 content mods.
    let mut screen: Vec<ScreenTab> = vanilla_tabs()
        .iter()
        .map(|&(id, column, search, inventory)| ScreenTab {
            id: s(id),
            column,
            search,
            inventory,
        })
        .collect();
    let mut ids = strings(&["minecraft:stone", "minecraft:diamond"]);
    let mut content = Vec::new();
    let mut manifests = Vec::new();
    for i in 0..25 {
        let id = format!("pack{i:02}");
        ids.push(format!("{id}:thing"));
        ids.push(format!("{id}:other"));
        manifests.push(mod_info(&id, &format!("Pack {i}"), &[]));
        content.push(id);
    }
    let plan = plan_tabs(&ids, &content, &manifests, &BTreeMap::new(), &[]);
    let merge = {
        let existing: Vec<TabMeta> = screen
            .iter()
            .map(|t| TabMeta {
                id: &t.id,
                column: t.column,
                search: t.search,
                inventory: t.inventory,
            })
            .collect();
        merge_layout(&existing, &plan)
    };
    // The game was on the survival inventory tab (the last one) when the mods were added.
    let selected = selected_after(screen.len() - 1, merge.insert_at, plan.len());
    let added = plan.iter().map(|p| ScreenTab {
        id: p.id.clone(),
        column: p.slot.column,
        search: false,
        inventory: false,
    });
    screen.splice(merge.insert_at..merge.insert_at, added);
    assert_eq!(screen.len(), 12 + 25);
    // Vanilla's tabs keep their order, the mods' follow spawn eggs, the survival inventory is last.
    assert_eq!(screen[5].id, "minecraft:search");
    assert_eq!(screen[10].id, "minecraft:spawn_eggs");
    assert_eq!(screen[11].id, "pack00:creative_tab");
    assert_eq!(screen[35].id, "pack24:creative_tab");
    assert_eq!(screen.last().unwrap().id, "minecraft:inventory");
    // The selected tab is still the one that was selected, a tab before the insertion is not moved.
    assert_eq!(screen[selected].id, "minecraft:inventory");
    assert_eq!(selected_after(2, merge.insert_at, plan.len()), 2);
    assert_eq!(selected_after(10, merge.insert_at, plan.len()), 10);
    assert_eq!(selected_after(11, merge.insert_at, plan.len()), 36);
    // On each page the screen shows exactly its tabs, with no two on one place.
    let mut paging = Paging::new(&merge);
    for page in 0..paging.pages() {
        paging.set_page(page);
        let shown: Vec<&ScreenTab> = screen.iter().filter(|t| paging.visible(&t.id)).collect();
        let fixed = shown.iter().filter(|t| t.search || t.inventory).count();
        assert_eq!(fixed, 2);
        let expected = if page == 0 { 10 } else if page == 3 { 5 } else { 10 };
        assert_eq!(shown.len() - fixed, expected, "page {page}");
    }
}
