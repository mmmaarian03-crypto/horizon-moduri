// Rules of the Curios port on a fake world: slot types from the real 1.20.1 data of the pack's jars,
// the inventory, the menu clicks, death, the screen data and the commands.
use horizon_curios::*;
use std::collections::{BTreeMap, BTreeSet};

// ---- a fake item world ------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct It {
    id: String,
    count: u32,
    /// Stands for the components (two stacks stack only with the same value).
    data: u32,
}
impl Stack for It {
    fn count(&self) -> u32 {
        self.count
    }
    fn set_count(&mut self, n: u32) {
        self.count = n;
    }
}
fn it(id: &str, count: u32) -> It {
    It { id: id.into(), count, data: 0 }
}

#[derive(Default)]
struct World {
    tags: BTreeMap<String, BTreeSet<String>>,
    max: BTreeMap<String, u32>,
    armor: BTreeMap<String, usize>,
    binding: BTreeSet<String>,
    vanishing: BTreeSet<String>,
    curios: BTreeSet<String>,
    no_unequip: BTreeSet<String>,
}
impl World {
    fn tag(mut self, tag: &str, items: &[&str]) -> Self {
        self.tags.entry(tag.into()).or_default().extend(items.iter().map(|s| s.to_string()));
        self
    }
}
impl Items<It> for World {
    fn max_stack(&self, s: &It) -> u32 {
        *self.max.get(&s.id).unwrap_or(&64)
    }
    fn same_item(&self, a: &It, b: &It) -> bool {
        a.id == b.id
    }
    fn same_components(&self, a: &It, b: &It) -> bool {
        a.id == b.id && a.data == b.data
    }
    fn stackable(&self, s: &It) -> bool {
        self.max_stack(s) > 1
    }
    fn in_tag(&self, s: &It, tag: &str) -> bool {
        self.tags.get(tag).is_some_and(|t| t.contains(&s.id))
    }
    fn has_curios_tag(&self, s: &It) -> bool {
        self.tags.iter().any(|(t, m)| t.starts_with("curios:") && m.contains(&s.id))
    }
    fn armor_slot(&self, s: &It) -> Option<usize> {
        self.armor.get(&s.id).copied()
    }
    fn offhand_item(&self, s: &It) -> bool {
        s.id == "minecraft:shield"
    }
    fn binding_curse(&self, s: &It) -> bool {
        self.binding.contains(&s.id)
    }
    fn vanishing_curse(&self, s: &It) -> bool {
        self.vanishing.contains(&s.id)
    }
    fn is_curio(&self, s: &It) -> bool {
        self.curios.contains(&s.id)
    }
    fn can_unequip(&self, _ctx: &SlotCtx, s: &It) -> bool {
        !self.no_unequip.contains(&s.id)
    }
}

// ---- the real data of the pack's jars -------------------------------------------------------------

fn file(ns: &str, path: &str, text: &str) -> DataFile {
    DataFile { namespace: ns.into(), path: path.into(), text: text.into() }
}
/// `data/*/curios/{slots,entities}` of curios-forge-5.14.1+1.20.1, L_Enders_Cataclysm-3.31 and
/// irons_spellbooks-1.20.1-3.16.3 (copied from the jars), and the IMC registrations read from the
/// bytecode of ars_nouveau-1.20.1-4.12.7 (`ModSetup`), goety-2.5.59.2 (`CuriosIntegration`) and
/// occultism-1.20.1-1.158.0 (`Occultism`).
fn tuned_sources() -> Sources {
    let curios = [
        ("back", r#"{"order":80,"icon":"curios:slot/empty_back_slot","validators":["curios:tag"]}"#),
        ("belt", r#"{"order":180,"icon":"curios:slot/empty_belt_slot","validators":["curios:tag"]}"#),
        ("body", r#"{"order":100,"icon":"curios:slot/empty_body_slot","validators":["curios:tag"]}"#),
        ("bracelet", r#"{"order":120,"icon":"curios:slot/empty_bracelet_slot","validators":["curios:tag"]}"#),
        ("charm", r#"{"order":200,"icon":"curios:slot/empty_charm_slot","validators":["curios:tag"]}"#),
        ("curio", r#"{"order":20,"icon":"curios:slot/empty_curio_slot"}"#),
        ("hands", r#"{"order":140,"icon":"curios:slot/empty_hands_slot","validators":["curios:tag"]}"#),
        ("head", r#"{"order":40,"icon":"curios:slot/empty_head_slot","validators":["curios:tag"]}"#),
        ("necklace", r#"{"order":60,"icon":"curios:slot/empty_necklace_slot","validators":["curios:tag"]}"#),
        ("ring", r#"{"order":160,"icon":"curios:slot/empty_ring_slot","validators":["curios:tag"]}"#),
    ];
    let mut slots: Vec<DataFile> = curios.iter().map(|(p, t)| file("curios", p, t)).collect();
    slots.push(file("cataclysm", "feet", r#"{"size":1,"order":190,"icon":"minecraft:item/empty_armor_slot_boots"}"#));
    slots.push(file("cataclysm", "hands", r#"{"size":2}"#));
    slots.push(file("cataclysm", "rings", r#"{"size":2,"order":110,"icon":"cataclysm:item/empty_ring_slot"}"#));
    slots.push(file("cataclysm", "talisman", r#"{"size":0,"order":180,"icon":"cataclysm:item/empty_talisman_slot"}"#));
    slots.push(file("cataclysm", "waist", r#"{"size":1,"order":170,"icon":"cataclysm:item/empty_waist_slot"}"#));
    slots.push(file("irons_spellbooks", "necklace", r#"{"size":1}"#));
    slots.push(file("irons_spellbooks", "ring", r#"{"size":2}"#));
    slots.push(file("irons_spellbooks", "spellbook", r#"{"replace":false,"size":1,"order":500,"icon":"curios:slot/spellbook_slot"}"#));
    let entities = vec![
        file("cataclysm", "slots", r#"{"entities":["player"],"slots":["head","necklace","rings","waist","talisman","hands","feet"]}"#),
        file("irons_spellbooks", "iss_entities", r#"{"entities":["player"],"slots":["spellbook","ring","necklace"]}"#),
    ];
    let p = |id: &str| LegacyMessage::preset(id).unwrap();
    let mut reg = Vec::new();
    for m in [
        p("necklace"),
        LegacyMessage { icon: Some("curios:slot/empty_curio_slot".into()), ..LegacyMessage::new("an_focus") },
        p("belt"),
        p("ring").with_size(2),
        p("charm"),
        p("head"),
    ] {
        reg.push(("ars_nouveau".to_string(), m));
    }
    for id in ["ring", "head", "necklace", "body", "back", "belt", "charm", "hands"] {
        reg.push(("goety".to_string(), LegacyMessage::new(id)));
    }
    for m in [p("hands"), p("ring"), p("head"), p("belt")] {
        reg.push(("occultism".to_string(), m));
    }
    Sources { slots, entities, legacy_register: reg, legacy_modify: Vec::new(), config: Vec::new() }
}

#[test]
fn tuned_player_slots_from_the_real_data() {
    let reg = build_registry(&tuned_sources());
    assert!(reg.errors.is_empty(), "{:?}", reg.errors);
    let got: Vec<(String, i32, i32)> = reg.player_slots().iter().map(|t| (t.id.clone(), t.order, t.size)).collect();
    let want = [
        ("head", 40, 1),
        ("necklace", 60, 1),
        ("back", 80, 1),
        ("body", 100, 1),
        ("rings", 110, 2),
        ("hands", 140, 2),
        ("ring", 160, 2),
        ("waist", 170, 1),
        ("belt", 180, 1),
        ("talisman", 180, 0),
        ("feet", 190, 1),
        ("charm", 200, 1),
        ("spellbook", 500, 1),
        ("an_focus", 1000, 1),
    ];
    let want: Vec<(String, i32, i32)> = want.iter().map(|(a, b, c)| (a.to_string(), *b, *c)).collect();
    assert_eq!(got, want);
    // bracelet and curio exist but no mod here gives them to players.
    assert!(reg.slots.contains_key("bracelet") && reg.slots.contains_key("curio"));
    let ring = &reg.slots["ring"];
    assert_eq!(ring.icon, "curios:slot/empty_ring_slot");
    assert_eq!(ring.validators, BTreeSet::from(["curios:tag".to_string()]));
    assert_eq!(reg.slots["an_focus"].icon, "curios:slot/empty_curio_slot");
    assert_eq!(reg.slots["feet"].icon, "minecraft:item/empty_armor_slot_boots");
    let inv: Inventory<It> = Inventory::new(&reg.player_slots());
    assert_eq!(inv.visible_slots(), 16);
}

#[test]
fn the_built_in_legacy_table_is_the_bytecode() {
    let s = tuned_sources();
    for sender in KNOWN_LEGACY_SENDERS {
        let want: Vec<LegacyMessage> = s.legacy_register.iter().filter(|(m, _)| m == sender).map(|(_, x)| x.clone()).collect();
        assert_eq!(known_legacy(sender), want, "{sender}");
    }
    assert!(known_legacy("minecraft").is_empty());
}

#[test]
fn only_the_curios_jar_gives_players_nothing() {
    let mut s = tuned_sources();
    s.slots.retain(|f| f.namespace == "curios");
    s.entities.clear();
    s.legacy_register.clear();
    let reg = build_registry(&s);
    assert_eq!(reg.slots.len(), 10);
    assert!(reg.player_slots().is_empty());
}

#[test]
fn builder_rules_match_slot_type_builder() {
    let mut b = SlotBuilder::new("ring");
    b.size(2, "SET", false);
    b.size(1, "SET", false);
    assert_eq!(b.build().size, 2, "SET keeps the biggest");
    b.size(1, "ADD", false);
    b.size(3, "ADD", false);
    assert_eq!(b.build().size, 6);
    b.size(1, "SET", true);
    assert_eq!(b.build().size, 1, "replace resets the additions");
    b.size(5, "REMOVE", false);
    assert_eq!(b.build().size, 0, "never below 0");
    b.order(50, false);
    b.order(70, false);
    assert_eq!(b.build().order, 50, "order keeps the smallest");
    b.order(70, true);
    assert_eq!(b.build().order, 70);
    b.native_gui(true, false);
    b.native_gui(false, false);
    assert!(!b.build().native_gui, "use_native_gui is and-ed");
    b.cosmetic(false, false);
    b.cosmetic(true, false);
    assert!(b.build().cosmetic, "add_cosmetic is or-ed");
    let t = SlotBuilder::new("x").build();
    assert_eq!((t.order, t.size, t.native_gui, t.cosmetic, t.render_toggle), (1000, 1, true, false, true));
}

#[test]
fn bad_files_are_reported_and_skipped() {
    let s = Sources {
        slots: vec![
            file("curios", "ring", r#"{"size":-1}"#),
            file("mod", "x", r#"{"operation":"MUL"}"#),
            file("mod", "y", "not json"),
            file("mod", "_hidden", r#"{"size":3}"#),
        ],
        entities: vec![file("mod", "e", r#"{"entities":["player"],"slots":["ring","nope","y","_hidden"]}"#)],
        ..Sources::default()
    };
    let reg = build_registry(&s);
    assert_eq!(reg.errors.len(), 3, "{:?}", reg.errors);
    assert!(!reg.slots.contains_key("_hidden"));
    let ids: Vec<String> = reg.player_slots().into_iter().map(|t| t.id).collect();
    assert_eq!(ids, vec!["ring".to_string(), "x".to_string()].into_iter().filter(|i| i != "x").collect::<Vec<_>>());
}

#[test]
fn config_slots_create_and_modify() {
    let s = Sources {
        slots: vec![file("curios", "ring", r#"{"order":160}"#)],
        config: vec!["id=ring;size=4".into(), "id=wings;size=1;order=5;add_cosmetic=true;drop_rule=ALWAYS_KEEP".into()],
        ..Sources::default()
    };
    let reg = build_registry(&s);
    let slots = reg.player_slots();
    assert_eq!(slots[0].id, "wings");
    assert!(slots[0].cosmetic);
    assert_eq!(slots[0].drop_rule, DropRule::AlwaysKeep);
    assert_eq!(slots[1].size, 4);
    assert!(build_registry(&Sources { config: vec!["size=2".into()], ..Sources::default() }).errors.len() == 1);
}

fn types(list: &[(&str, i32, i32)]) -> Vec<SlotType> {
    list.iter()
        .map(|(id, order, size)| {
            let mut b = SlotBuilder::new(id);
            b.order(*order, true);
            b.size(*size, "SET", true);
            b.build()
        })
        .collect()
}

fn world() -> World {
    let mut w = World::default()
        .tag("curios:ring", &["ars:ring", "ars:ring2"])
        .tag("curios:belt", &["ars:belt"])
        .tag("curios:head", &["minecraft:carved_pumpkin"])
        .tag("curios:charm", &["x:charm", "x:charm_stack"]);
    w.max.insert("ars:ring".into(), 1);
    w.max.insert("ars:ring2".into(), 1);
    w.max.insert("ars:belt".into(), 1);
    w.max.insert("x:charm".into(), 1);
    w.max.insert("minecraft:diamond_helmet".into(), 1);
    w.armor.insert("minecraft:diamond_helmet".into(), 0);
    w.armor.insert("minecraft:carved_pumpkin".into(), 0);
    w
}

#[test]
fn validity_follows_the_tags() {
    let w = world();
    let t = types(&[("ring", 160, 2), ("belt", 180, 1), ("curio", 20, 1)]);
    let ctx = |id: &str| SlotCtx { id: id.into(), index: 0, cosmetic: false, visible: true };
    assert!(may_equip(&w, &t, &t, &ctx("ring"), &it("ars:ring", 1)));
    assert!(!may_equip(&w, &t, &t, &ctx("belt"), &it("ars:ring", 1)));
    assert!(may_equip(&w, &t, &t, &ctx("curio"), &it("ars:ring", 1)), "the curio slot takes every curio");
    assert!(!may_equip(&w, &t, &t, &ctx("ring"), &it("minecraft:dirt", 1)));
    assert!(!may_equip(&w, &t, &t, &ctx("curio"), &it("minecraft:dirt", 1)));
    // The tooltip lists the slot types whose validators pass (the curio slot takes it anyway).
    assert_eq!(tooltip_slots(&w, &t, &it("ars:ring", 1)), vec!["ring".to_string()]);
    let w2 = world().tag("curios:curio", &["x:any"]).tag("curios:belt", &["x:any"]);
    assert_eq!(tooltip_slots(&w2, &t, &it("x:any", 1)), vec!["curio".to_string()], "curio alone when it is there");
    assert!(tooltip_slots(&w, &t, &it("minecraft:dirt", 1)).is_empty());
}

#[test]
fn loading_a_save_keeps_or_returns_every_item() {
    let w = world();
    let t = types(&[("ring", 160, 2), ("belt", 180, 1)]);
    let saved = vec![
        Saved {
            id: "ring".into(),
            stacks: Some(vec![Some(it("ars:ring", 1)), Some(it("minecraft:dirt", 3)), Some(it("ars:ring2", 1))]),
            cosmetics: Some(vec![None, None, None]),
            renders: Some((Some(3), vec![(0, false), (1, true), (2, true)])),
            ..Saved::default()
        },
        Saved { id: "gone".into(), stacks: Some(vec![Some(it("x:charm", 1))]), ..Saved::default() },
    ];
    let valid = |ctx: &SlotCtx, s: &It| may_equip(&w, &t, &t, ctx, s);
    let (inv, lost) = Inventory::load(&t, saved, valid);
    let ring = inv.handler("ring").unwrap();
    assert_eq!(ring.slots(), 2);
    assert_eq!(ring.stacks[0], Some(it("ars:ring", 1)));
    assert_eq!(ring.stacks[1], None, "dirt is not a ring");
    assert!(!ring.renders[0]);
    let mut lost_ids: Vec<String> = lost.iter().map(|s| s.id.clone()).collect();
    lost_ids.sort();
    assert_eq!(lost_ids, vec!["ars:ring2", "minecraft:dirt", "x:charm"]);
    assert_eq!(inv.handler("belt").unwrap().slots(), 1);
}

#[test]
fn saved_slot_modifiers_keep_the_size() {
    let t = types(&[("charm", 200, 1)]);
    let m = SlotModifier { id: "belt".into(), name: "belt".into(), amount: 2.0, operation: 0, uuid: None };
    let saved = vec![Saved {
        id: "charm".into(),
        stacks: Some(vec![Some(it("x:charm", 1)), Some(it("x:charm", 1)), Some(it("x:charm", 1))]),
        cached: vec![m.clone()],
        size_shift: None,
        ..Saved::default()
    }];
    let (mut inv, lost) = Inventory::load(&t, saved, |_, _| true);
    assert!(lost.is_empty());
    assert_eq!(inv.handler("charm").unwrap().slots(), 3);
    // The belt is still worn: the same modifier comes back on the first tick, nothing moves.
    let wanted = BTreeMap::from([("charm".to_string(), vec![m])]);
    let (_, lost) = inv.set_transient(&wanted);
    assert!(lost.is_empty());
    // The belt is gone: two slots go and give their items back.
    let (changed, lost) = inv.set_transient(&BTreeMap::new());
    assert!(changed);
    assert_eq!(lost.len(), 2);
    assert_eq!(inv.handler("charm").unwrap().slots(), 1);
    // `/curios add charm 2` (the legacy modifier) and the multiply operations.
    let h = inv.handler_mut("charm").unwrap();
    h.legacy_change(2);
    h.legacy_change(-1);
    assert_eq!(h.wanted_size(), 2);
    h.transient.push(SlotModifier { id: "x".into(), name: "x".into(), amount: 2.0, operation: 2, uuid: None });
    assert_eq!(h.wanted_size(), 4);
    set_slots(h, 5);
    assert_eq!(h.persistent.len(), 1);
}

#[test]
fn changes_report_equip_and_unequip_once() {
    let w = world();
    let t = types(&[("ring", 160, 2)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    let mut prev = Previous::new();
    assert!(changes(&inv, &mut prev, &w).is_empty());
    inv.handler_mut("ring").unwrap().stacks[1] = Some(it("ars:ring", 1));
    let c = changes(&inv, &mut prev, &w);
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].ctx.index, c[0].old.is_none(), c[0].new.is_some()), (1, true, true));
    assert!(changes(&inv, &mut prev, &w).is_empty(), "only once");
    // Shrinking the handler unequips what was there.
    inv.handler_mut("ring").unwrap().resize(1);
    let c = changes(&inv, &mut prev, &w);
    assert_eq!(c.len(), 1);
    assert!(c[0].old.is_some() && c[0].new.is_none());
}

#[test]
fn death_follows_keep_inventory_and_the_rules() {
    let mut w = world();
    w.vanishing.insert("x:charm".into());
    let t = types(&[("ring", 160, 2), ("charm", 200, 1)]);
    let fill = || {
        let mut inv: Inventory<It> = Inventory::new(&t);
        inv.handler_mut("ring").unwrap().stacks = vec![Some(it("ars:ring", 1)), Some(it("ars:ring2", 1))];
        inv.handler_mut("ring").unwrap().cosmetics = vec![Some(it("ars:ring", 1)), None];
        inv.handler_mut("charm").unwrap().stacks = vec![Some(it("x:charm", 1))];
        inv
    };
    let none = |_: &SlotCtx, _: &It| DropRule::Default;
    let mut inv = fill();
    let (drops, destroyed) = death_drops(&mut inv, &w, false, KeepCurios::Default, none);
    assert_eq!(drops.len(), 3);
    assert_eq!(destroyed.len(), 1, "curse of vanishing");
    assert!(inv.worn().is_empty());
    let mut inv = fill();
    let (drops, _) = death_drops(&mut inv, &w, true, KeepCurios::Default, none);
    assert!(drops.is_empty(), "keepInventory keeps the curios");
    assert_eq!(inv.worn().len(), 4);
    let mut inv = fill();
    let (drops, _) = death_drops(&mut inv, &w, true, KeepCurios::Off, none);
    assert_eq!(drops.len(), 3, "keepCurios OFF drops anyway");
    let mut inv = fill();
    let (drops, _) = death_drops(&mut inv, &w, false, KeepCurios::On, none);
    assert!(drops.is_empty());
    let mut inv = fill();
    inv.handlers[0].drop_rule = DropRule::AlwaysKeep;
    let (drops, _) = death_drops(&mut inv, &w, false, KeepCurios::Default, none);
    assert!(drops.is_empty(), "the slot type keeps them");
    let mut inv = fill();
    let rule = |ctx: &SlotCtx, _: &It| if ctx.index == 0 && !ctx.cosmetic { DropRule::Destroy } else { DropRule::Default };
    let (drops, destroyed) = death_drops(&mut inv, &w, false, KeepCurios::Default, rule);
    assert_eq!((drops.len(), destroyed.len()), (2, 2));
}

#[test]
fn layout_is_the_v2_panel() {
    let reg = build_registry(&tuned_sources());
    let inv: Inventory<It> = Inventory::new(&reg.player_slots());
    let l = layout(&inv, 0, false, config::MAX_SLOTS_PER_PAGE, config::MINIMUM_COLUMNS);
    assert_eq!((l.columns, l.panel_width, l.total_pages), (2, 50, 1));
    assert_eq!(l.slots.len(), 16);
    assert_eq!(l.grid, vec![8, 8]);
    assert_eq!((l.slots[0].x, l.slots[0].y), (7 - 50, 8));
    assert_eq!((l.slots[1].x, l.slots[1].y), (25 - 50, 8));
    assert_eq!((l.slots[2].x, l.slots[2].y), (7 - 50, 26));
    // 60 slots: two pages of 48, 6 columns, the second page with the rest.
    let many = types(&[("ring", 1, 60)]);
    let inv: Inventory<It> = Inventory::new(&many);
    let l = layout(&inv, 0, false, 48, 1);
    assert_eq!((l.columns, l.total_pages, l.slots.len(), l.proxies.len()), (6, 2, 48, 12));
    assert_eq!(l.slots[0].y, 16, "the page buttons push the slots down");
    let l = layout(&inv, 1, false, 48, 1);
    assert_eq!((l.columns, l.slots.len()), (2, 12));
}

/// A tiny player for menu tests.
struct P {
    inventory: Vec<Option<It>>,
    armor: Vec<Option<It>>,
    offhand: Option<It>,
    carried: Option<It>,
}
impl P {
    fn new() -> Self {
        P { inventory: vec![None; 36], armor: vec![None; 4], offhand: None, carried: None }
    }
}
fn with_menu<R>(w: &World, p: &mut P, inv: &mut Inventory<It>, t: &[SlotType], f: impl FnOnce(&mut Menu<'_, '_, It, World>) -> R) -> (R, Vec<It>) {
    let l = layout(inv, 0, false, 48, 1);
    let mut q = QuickCraft::default();
    let mut player = Player {
        inventory: &mut p.inventory,
        armor: &mut p.armor,
        offhand: &mut p.offhand,
        carried: &mut p.carried,
        creative: false,
        thrown: Vec::new(),
    };
    let r = {
        let mut m = Menu::new(w, &mut player, inv, &l, t, t, &mut q);
        f(&mut m)
    };
    (r, player.thrown)
}

#[test]
fn clicks_put_curios_in_their_slots_only() {
    let w = world();
    let t = types(&[("ring", 160, 2), ("belt", 180, 1)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    let mut p = P::new();
    p.inventory[9] = Some(it("ars:ring", 1));
    p.inventory[10] = Some(it("minecraft:dirt", 5));
    // Pick up the ring, put it in ring slot 0 (menu 46).
    with_menu(&w, &mut p, &mut inv, &t, |m| {
        m.click(9, 0, Click::Pickup);
        m.click(46, 0, Click::Pickup);
        assert_eq!(m.equipped, vec![0]);
    });
    assert!(p.carried.is_none());
    assert_eq!(inv.handler("ring").unwrap().stacks[0], Some(it("ars:ring", 1)));
    // Dirt cannot go into the belt slot (menu 48).
    with_menu(&w, &mut p, &mut inv, &t, |m| {
        m.click(10, 0, Click::Pickup);
        m.click(48, 0, Click::Pickup);
    });
    assert_eq!(p.carried, Some(it("minecraft:dirt", 5)));
    assert!(inv.handler("belt").unwrap().stacks[0].is_none());
    // Back down.
    with_menu(&w, &mut p, &mut inv, &t, |m| m.click(10, 0, Click::Pickup));
    assert!(p.carried.is_none());
}

#[test]
fn shift_click_moves_between_inventory_and_curios() {
    let w = world();
    let t = types(&[("ring", 160, 2), ("belt", 180, 1)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    let mut p = P::new();
    p.inventory[0] = Some(it("ars:belt", 1));
    p.inventory[20] = Some(it("minecraft:diamond_helmet", 1));
    p.inventory[21] = Some(it("minecraft:carved_pumpkin", 1));
    with_menu(&w, &mut p, &mut inv, &t, |m| {
        m.click(36, 0, Click::QuickMove); // hotbar 0 -> belt
        m.click(20, 0, Click::QuickMove); // helmet -> head armour
        m.click(21, 0, Click::QuickMove); // pumpkin: head armour taken, no head curio slot -> hotbar
    });
    assert_eq!(inv.handler("belt").unwrap().stacks[0], Some(it("ars:belt", 1)));
    assert_eq!(p.armor[0], Some(it("minecraft:diamond_helmet", 1)));
    assert_eq!(p.inventory[0], Some(it("minecraft:carved_pumpkin", 1)));
    // From the curio slot back into the inventory (9..45, first the inventory rows).
    with_menu(&w, &mut p, &mut inv, &t, |m| m.click(48, 0, Click::QuickMove));
    assert!(inv.handler("belt").unwrap().stacks[0].is_none());
    assert_eq!(p.inventory[9], Some(it("ars:belt", 1)));
}

#[test]
fn bound_and_locked_curios_stay() {
    let mut w = world();
    w.binding.insert("ars:ring".into());
    w.no_unequip.insert("ars:ring2".into());
    let t = types(&[("ring", 160, 2)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    inv.handlers[0].stacks = vec![Some(it("ars:ring", 1)), Some(it("ars:ring2", 1))];
    let mut p = P::new();
    with_menu(&w, &mut p, &mut inv, &t, |m| {
        m.click(46, 0, Click::Pickup);
        m.click(47, 0, Click::QuickMove);
        m.click(46, 0, Click::Throw);
    });
    assert!(p.carried.is_none());
    assert_eq!(inv.worn().len(), 2);
}

#[test]
fn swap_throw_clone_and_pickup_all() {
    let w = world();
    let t = types(&[("ring", 160, 2), ("charm", 200, 1)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    let mut p = P::new();
    p.inventory[3] = Some(it("ars:ring", 1));
    let (_, thrown) = with_menu(&w, &mut p, &mut inv, &t, |m| {
        m.click(46, 3, Click::Swap); // number key 4 over ring slot 0
    });
    assert!(thrown.is_empty());
    assert!(p.inventory[3].is_none());
    assert!(inv.handlers[0].stacks[0].is_some());
    let (_, thrown) = with_menu(&w, &mut p, &mut inv, &t, |m| m.click(46, 0, Click::Throw));
    assert_eq!(thrown, vec![it("ars:ring", 1)]);
    assert!(inv.worn().is_empty());
    // Stackable charms: double click gathers them from the inventory.
    p.inventory[9] = Some(it("x:charm_stack", 10));
    p.inventory[12] = Some(it("x:charm_stack", 5));
    with_menu(&w, &mut p, &mut inv, &t, |m| {
        m.click(9, 0, Click::Pickup);
        m.click(9, 0, Click::PickupAll);
    });
    assert_eq!(p.carried, Some(it("x:charm_stack", 15)));
    assert!(p.inventory[12].is_none());
    // Click outside: the carried stack is thrown.
    let (_, thrown) = with_menu(&w, &mut p, &mut inv, &t, |m| m.click(OUTSIDE, 0, Click::Pickup));
    assert_eq!(thrown.len(), 1);
    assert!(p.carried.is_none());
}

#[test]
fn shift_double_click_gathers_the_same_inventory_only() {
    let w = world();
    let t = types(&[("charm", 200, 2)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    inv.handlers[0].stacks = vec![Some(it("x:charm_stack", 3)), None];
    let mut p = P::new();
    p.inventory[9] = Some(it("minecraft:dirt", 10));
    p.inventory[12] = Some(it("minecraft:dirt", 20));
    p.inventory[13] = Some(it("x:charm_stack", 2));
    with_menu(&w, &mut p, &mut inv, &t, |m| m.quick_move_all(9, &it("minecraft:dirt", 1)));
    // As vanilla (hotbar and rows are one inventory): the dirt stacks went to the hotbar, merged,
    // and the merged stack went back to the first free row slot; the charms stayed.
    let dirt: Vec<(usize, u32)> = p.inventory.iter().enumerate().filter_map(|(i, s)| s.as_ref().filter(|s| s.id == "minecraft:dirt").map(|s| (i, s.count))).collect();
    assert_eq!(dirt, vec![(9, 30)]);
    assert_eq!(p.inventory[13], Some(it("x:charm_stack", 2)));
    assert_eq!(inv.handlers[0].stacks[0], Some(it("x:charm_stack", 3)));
}

#[test]
fn drag_splits_over_valid_slots_only() {
    let w = world();
    let t = types(&[("charm", 200, 1), ("ring", 160, 1)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    let mut p = P::new();
    p.carried = Some(it("x:charm_stack", 9));
    // Even split (type 0) over inventory 9, the ring slot (refused) and the charm slot.
    let ring_slot = 46; // ring (order 160) comes first
    let charm_slot = 47;
    with_menu(&w, &mut p, &mut inv, &t, |m| {
        m.click(OUTSIDE, 0, Click::QuickCraft);
        m.click(9, 1, Click::QuickCraft);
        m.click(ring_slot, 1, Click::QuickCraft);
        m.click(charm_slot, 1, Click::QuickCraft);
        m.click(OUTSIDE, 2, Click::QuickCraft);
    });
    assert_eq!(p.inventory[9].as_ref().map(|s| s.count), Some(4));
    assert_eq!(inv.handler("charm").unwrap().stacks[0].as_ref().map(|s| s.count), Some(4));
    assert!(inv.handler("ring").unwrap().stacks[0].is_none());
    assert_eq!(p.carried.as_ref().map(|s| s.count), Some(1));
}

/// Thousands of random clicks: no item is ever made or lost (thrown ones counted).
#[test]
fn random_clicks_keep_every_item() {
    let w = world();
    let t = types(&[("ring", 160, 2), ("belt", 180, 1), ("charm", 200, 3)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    let mut p = P::new();
    let pool = ["ars:ring", "ars:belt", "x:charm", "x:charm_stack", "minecraft:dirt", "minecraft:diamond_helmet"];
    for (i, id) in pool.iter().enumerate() {
        let n = if w.max_stack(&it(id, 1)) > 1 { 30 } else { 1 };
        p.inventory[i * 5] = Some(it(id, n));
    }
    let total = |p: &P, inv: &Inventory<It>, thrown: u32| -> u32 {
        let a: u32 = p.inventory.iter().chain(p.armor.iter()).flatten().map(|s| s.count).sum();
        let b = p.offhand.as_ref().map_or(0, |s| s.count) + p.carried.as_ref().map_or(0, |s| s.count);
        let c: u32 = inv.worn().iter().map(|(_, _, _, s)| s.count).sum();
        a + b + c + thrown
    };
    let start = total(&p, &inv, 0);
    let mut thrown_total = 0;
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut next = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    for _ in 0..5000 {
        let slots = 46 + inv.visible_slots() as u64;
        let slot = if next(20) == 0 { OUTSIDE } else { next(slots) as i32 };
        let click = [Click::Pickup, Click::QuickMove, Click::Swap, Click::Throw, Click::PickupAll, Click::QuickCraft][next(6) as usize];
        let button = match click {
            Click::Swap => [0, 3, 8, 40][next(4) as usize],
            Click::QuickCraft => [0, 1, 2, 4, 5, 6][next(6) as usize],
            _ => next(2) as i32,
        };
        let (_, thrown) = with_menu(&w, &mut p, &mut inv, &t, |m| m.click(slot, button, click));
        thrown_total += thrown.iter().map(|s| s.count).sum::<u32>();
        assert_eq!(total(&p, &inv, thrown_total), start, "after {click:?} {slot} {button}");
        for (hi, i, _, s) in inv.worn() {
            let ctx = inv.ctx(hi, i, false);
            assert!(may_equip(&w, &t, &t, &ctx, s), "{s:?} in {}", ctx.id);
        }
    }
}

#[test]
fn right_click_equips_into_the_first_free_slot_or_swaps() {
    let w = world();
    let t = types(&[("ring", 160, 2)]);
    let mut inv: Inventory<It> = Inventory::new(&t);
    let all = |_: &SlotCtx| true;
    assert_eq!(equip_from_use(&inv, &w, &t, &t, &it("ars:ring", 1), all), Some(Ok((0, 0))));
    inv.handlers[0].stacks = vec![Some(it("ars:ring2", 1)), None];
    assert_eq!(equip_from_use(&inv, &w, &t, &t, &it("ars:ring", 1), all), Some(Ok((0, 1))));
    inv.handlers[0].stacks = vec![Some(it("ars:ring2", 1)), Some(it("ars:ring2", 1))];
    assert_eq!(equip_from_use(&inv, &w, &t, &t, &it("ars:ring", 1), all), Some(Err((0, 0))));
    assert_eq!(equip_from_use(&inv, &w, &t, &t, &it("ars:ring", 1), |_| false), None);
    assert_eq!(equip_from_use(&inv, &w, &t, &t, &it("minecraft:dirt", 1), all), None);
}

#[test]
fn the_screen_data_round_trips() {
    let reg = build_registry(&tuned_sources());
    let mut inv: Inventory<It> = Inventory::new(&reg.player_slots());
    inv.handlers[0].renders[0] = false;
    let l = layout(&inv, 0, false, 48, 1);
    let mut icons = BTreeMap::new();
    for t in reg.player_slots() {
        icons.insert(t.id.clone(), t.icon.clone());
    }
    let v = View { layout: l, icons };
    let back = View::decode(&v.encode()).unwrap();
    assert_eq!(back, v);
    assert!(View::decode(b"v2 junk").is_none());
    // The widgets: the render toggle of the first slot, the button back to the inventory.
    let s0 = &v.layout.slots[0];
    assert_eq!(widget_at(&v, s0.x as f32 + 12.0, s0.y as f32 - 2.0), Some(Widget::Render(0)));
    let (bx, by) = button_offset(false);
    assert_eq!((bx, by), (26, 8));
    assert_eq!(widget_at(&v, bx as f32 + 1.0, by as f32 + 1.0), Some(Widget::Vanilla));
    assert_eq!(button_offset(true), (73, 6));
    assert!(widget_at(&v, 100.0, 100.0).is_none());
}

#[test]
fn slot_names_like_the_original() {
    let tr = |k: &str| match k {
        "curios.identifier.ring" => Some("Ring".to_string()),
        "curios.cosmetic" => Some("Cosmetic".to_string()),
        _ => None,
    };
    assert_eq!(slot_name("ring", false, &tr), "Ring");
    assert_eq!(slot_name("ring", true, &tr), "Cosmetic Ring");
    assert_eq!(slot_name("spellBook", false, &tr), "Spellbook");
}

#[test]
fn commands_parse() {
    assert_eq!(parse_command("give x"), None);
    assert_eq!(parse_command("curios list"), Some(Ok(Command::List)));
    assert_eq!(
        parse_command("curios add ring @s 2"),
        Some(Ok(Command::Add { slot: "ring".into(), target: "@s".into(), amount: 2 }))
    );
    assert_eq!(parse_command("curios set ring Alex"), Some(Ok(Command::Set { slot: "ring".into(), target: "Alex".into(), amount: 1 })));
    assert_eq!(
        parse_command("curios replace ring 0 @s with ars_nouveau:ring_of_potential 1"),
        Some(Ok(Command::Replace {
            slot: "ring".into(),
            index: 0,
            target: "@s".into(),
            item: "ars_nouveau:ring_of_potential".into(),
            count: 1
        }))
    );
    assert_eq!(parse_command("curios clear @s"), Some(Ok(Command::Clear { target: "@s".into(), slot: None })));
    assert_eq!(parse_command("curios drop @s ring"), Some(Ok(Command::Drop { target: "@s".into(), slot: Some("ring".into()) })));
    assert!(matches!(parse_command("curios add ring @s x"), Some(Err(_))));
    assert!(matches!(parse_command("curios"), Some(Err(_))));
}
