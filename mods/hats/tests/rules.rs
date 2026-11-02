use horizon_hats::*;
use serde_json::Value;
use std::path::Path;

fn index() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/mods/hats/carry/assets/hats/horizon/hats-index.json");
    serde_json::from_slice(&std::fs::read(p).expect("carry hats-index.json")).unwrap()
}

fn reg() -> Registry {
    Registry::from_index(&index())
}

#[test]
fn java_random_matches_the_jvm() {
    // Values printed by JBR 17 (`tools/jvm/Check.java`, `java Check.java`).
    let mut r = JavaRandom::new(0);
    assert_eq!(r.next_double(), 0.730967787376657);
    let mut r = JavaRandom::new(42);
    assert_eq!(r.next_int(10), 0);
    assert_eq!(r.next_int(7), 5);
    assert_eq!(r.next_float(), 0.683_223_4);
    assert_eq!(java_hash("Adventurer's Hat"), -621_161_591);
    // UTF-16 units: one per BMP char, two for an astral one.
    assert_eq!(java_hash("\u{e9}"), 0xe9);
    assert_eq!(java_hash("\u{1F3A9}"), 0xD83C * 31 + 0xDFA9);
    let u = [0x1f2e_3d4c, 0x5b6a_4978u32 as i32, 0x8695_a4b3u32 as i32, 0xc2d1_e0f9u32 as i32];
    assert_eq!(uuid_string(u), "1f2e3d4c-5b6a-4978-8695-a4b3c2d1e0f9");
    assert_eq!(uuid_hash(u), 12414);
    // assignMobHat's seed and first draws for that UUID.
    let seed = i64::from(java_hash(&uuid_string(u)).wrapping_abs()).wrapping_mul(425_480_085);
    assert_eq!(seed, 544_147_649_074_813_410);
    let mut m = JavaRandom::new(seed);
    assert_eq!(m.next_double(), 0.180_942_371_079_073_07);
    assert_eq!(m.next_int(37), 3);
    let rs = i64::from(java_hash("Top Hat").wrapping_abs()).wrapping_mul(154_041_013);
    assert_eq!(JavaRandom::new(rs).next_double(), 0.175_184_837_678_624_8);
    assert_eq!(i64::from(i32::MIN.wrapping_abs()).wrapping_mul(53_579_997_854), -4_381_704_809_082_781_696);
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn the_registry_holds_every_hat_with_its_accessories() {
    let r = reg();
    assert_eq!(r.defs.len(), 529);
    let hats = r.defs.iter().filter(|d| !d.is_accessory()).count();
    assert_eq!(hats, 235);
    // Every accessory hangs under its hat (none left over, as HatLoader would warn).
    let attached: usize = r.defs.iter().map(|d| d.accessories.len()).sum();
    assert_eq!(attached, 294);
    let adv = r.hat_named("Adventurer's Hat").unwrap();
    assert_eq!(adv.forced_rarity, Some(Rarity::Uncommon));
    let names: Vec<&str> = adv.accessories.iter().map(|&i| r.defs[i].name.as_str()).collect();
    assert_eq!(names, ["Forgotten Adventurer", "Master Adventurer", "Pirate Adventurer"]);
    let forgotten = &r.defs[adv.accessories[0]];
    assert_eq!(forgotten.full_name(), "Adventurer's Hat:Forgotten Adventurer");
    assert!(r.get("Adventurer's Hat:Forgotten Adventurer").is_some());
    // The 1980s hat's Tabula notes make the antenna its accessory.
    let eighties = r.hat_named("1980s is Calling").unwrap();
    assert!(eighties.accessories.iter().any(|&i| r.defs[i].name == "Extended Antenna"));
    // Legacy hats load before the new ones.
    assert!(r.hat_named("1980s is Calling").unwrap().order < adv.order);
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn unforced_rarities_come_from_the_name_seed() {
    let r = reg();
    for d in &r.defs {
        if d.forced_rarity.is_none() {
            let seed = i64::from(java_hash(&d.full_name()).wrapping_abs()).wrapping_mul(154_041_013);
            assert_eq!(d.rarity, rarity_for_chance(JavaRandom::new(seed).next_double()), "{}", d.name);
        }
    }
    // Thresholds 21/50, 34/50, 42/50, 47/50 of the weights.
    assert_eq!(rarity_for_chance(0.42), Rarity::Common);
    assert_eq!(rarity_for_chance(0.4201), Rarity::Uncommon);
    assert_eq!(rarity_for_chance(0.67), Rarity::Uncommon);
    assert_eq!(rarity_for_chance(0.83), Rarity::Rare);
    assert_eq!(rarity_for_chance(0.94), Rarity::Epic);
    assert_eq!(rarity_for_chance(0.99), Rarity::Legendary);
    // Every rarity has hats, so every roll can land.
    for (i, pools) in r.pools.iter().enumerate() {
        assert!(!pools.is_empty(), "rarity {i}");
    }
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn mob_hats_follow_tuned_chances() {
    let r = reg();
    let mut counts = [0usize; 6];
    let mut rnd = JavaRandom::new(1234);
    let n = 40_000;
    for _ in 0..n {
        let u = [rnd.next_int(i32::MAX), rnd.next_int(i32::MAX), rnd.next_int(i32::MAX), rnd.next_int(i32::MAX)];
        match roll_mob_hat(&r, u, "minecraft:zombie") {
            Some(h) => {
                let d = r.hat_named(&h.name).unwrap();
                counts[d.rarity.index()] += 1;
                assert!(h.showing);
            }
            None => counts[5] += 1,
        }
    }
    // common 10%, uncommon 8%, rare 5%, epic 3%, legendary 2%, nothing 72%.
    let expect = [0.10, 0.08, 0.05, 0.03, 0.02, 0.72];
    for i in 0..6 {
        let got = counts[i] as f64 / n as f64;
        assert!((got - expect[i]).abs() < 0.008, "bucket {i}: {got}");
    }
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn a_mobs_hat_is_a_function_of_its_uuid() {
    let r = reg();
    let mut found = 0;
    for k in 0..2000 {
        let u = [k, 7 * k, -k, 99];
        let a = roll_mob_hat(&r, u, "minecraft:skeleton");
        assert_eq!(a, roll_mob_hat(&r, u, "minecraft:skeleton"));
        if let Some(h) = a {
            found += 1;
            for acc in &h.accessories {
                let d = r.def_of(acc).expect("accessory key");
                assert_eq!(d.accessory_for.as_deref(), Some(h.name.as_str()));
                assert!(acc.showing);
            }
        }
    }
    assert!(found > 400, "{found}");
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn accessories_roll_and_one_wins_per_layer() {
    let r = reg();
    let adv = r.by_full_name["Adventurer's Hat"];
    // All three Adventurer accessories share the "Head" layer: at most one is worn.
    let mut with_acc = 0;
    for k in 0..30_000i32 {
        let u = [k, k ^ 0x5555, 3, k.wrapping_mul(31)];
        let mut part = HatPart::new("Adventurer's Hat");
        part.showing = true;
        attach_accessories_to_part(&r, adv, &mut part, u, &uuid_string(u));
        assert!(part.accessories.len() <= 1, "{:?}", part.accessories);
        with_acc += usize::from(!part.accessories.is_empty());
    }
    assert!(with_acc > 1000, "{with_acc}");
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn pickups_unlock_once_and_count() {
    let r = reg();
    let mut s = Save::default();
    let p = "00000000-0000-0000-0000-000000000001";
    let mut hat = as_hat_part(&r, r.by_full_name["Adventurer's Hat"], 1);
    hat.accessories.truncate(1);
    let t = s.pickup(p, hat.clone());
    assert_eq!(t, vec![Unlock { hat: HatPart::new("Adventurer's Hat"), accessory: false }]);
    let owned = &s.players[p].inventory[0];
    assert!(owned.is_new && owned.count == 1 && !owned.accessories[0].showing);
    // The same hat again: counted, no toast.
    let mut plain = HatPart::new("Adventurer's Hat");
    plain.showing = true;
    assert!(s.pickup(p, plain).is_empty());
    assert_eq!(s.players[p].inventory[0].count, 2);
    // With a new accessory: its own toast.
    let mut with = HatPart::new("Adventurer's Hat");
    let mut acc = HatPart::new("Pirate Adventurer");
    acc.registry_key = Some("Adventurer's Hat:Pirate Adventurer".into());
    with.accessories.push(acc.clone());
    let t = s.pickup(p, with);
    assert_eq!(t.len(), 1);
    assert!(t[0].accessory && t[0].hat.name == "Pirate Adventurer");
    assert_eq!(s.players[p].inventory[0].accessories.len(), 2);
}

#[test]
fn taking_and_launching_hats() {
    let mut d = PlayerData::default();
    for n in ["A", "B", "C"] {
        d.add_hat(HatPart::new(n));
    }
    d.inventory[1].favourite = true;
    assert_eq!(d.take_specific("B"), None, "favourites are not thrown");
    let a = d.take_specific("A").unwrap();
    assert_eq!(a.count, 1);
    assert!(!d.has_hat("A"));
    let mut rnd = JavaRandom::new(5);
    assert_eq!(d.take_random_non_favourite(&mut rnd).unwrap().name, "C");
    assert_eq!(d.take_random_non_favourite(&mut rnd), None);
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn save_round_trips_through_json() {
    let r = reg();
    let mut s = Save::default();
    let p = "11111111-2222-3333-4444-555555555555";
    s.pickup(p, as_hat_part(&r, 3, 2));
    s.player(p).equipped = Some(HatPart::new("Top Hat"));
    s.entity_hats.insert("aaaa".into(), Some(HatPart::new("Garland")));
    s.entity_hats.insert("bbbb".into(), None);
    let back = Save::from_json(&s.to_json());
    assert_eq!(back, s);
}

struct Floor(f64);
impl HatWorld for Floor {
    fn solid(&self, p: [f64; 3]) -> bool {
        p[1] < self.0
    }
}

#[test]
fn a_launched_hat_flies_falls_and_lands() {
    let mut rnd = JavaRandom::new(9);
    let mut t = launch(HatPart::new("Top Hat"), [0.0, 65.62, 0.0], [0.0, 0.0, 1.0], "me", &mut rnd);
    assert_eq!(t.pos, [0.0, 65.47, 0.5]);
    assert_eq!(t.motion, [0.0, 0.0, 1.2]);
    let shooter = Target { uuid: "me".into(), min: [-0.3, 64.0, -0.3], max: [0.3, 65.8, 0.3], spectator: false };
    let world = Floor(64.0);
    let mut landed_at = None;
    for tick in 0..200 {
        assert_eq!(tick_thrown(&mut t, &world, std::slice::from_ref(&shooter)), Step::Flying);
        if t.on_ground && landed_at.is_none() {
            landed_at = Some(tick);
        }
    }
    let tick = landed_at.expect("lands");
    assert!(tick > 5 && tick < 40, "{tick}");
    assert_eq!(t.pos[1], 64.0);
    assert!(t.pos[2] > 5.0, "{:?}", t.pos);
    assert!(t.motion[2].abs() < 1e-3);
}

#[test]
fn a_launched_hat_hits_a_mob_but_not_its_shooter() {
    let mut rnd = JavaRandom::new(9);
    let mut t = launch(HatPart::new("Top Hat"), [0.0, 65.62, 0.0], [0.0, 0.0, 1.0], "me", &mut rnd);
    let me = Target { uuid: "me".into(), min: [-0.3, 64.0, -0.3], max: [0.3, 65.8, 0.3], spectator: false };
    let zombie = Target { uuid: "z".into(), min: [-0.3, 64.0, 4.7], max: [0.3, 65.95, 5.3], spectator: false };
    let world = Floor(64.0);
    let targets = [me, zombie];
    let mut hit = None;
    for _ in 0..20 {
        if let Step::Hit(i) = tick_thrown(&mut t, &world, &targets) {
            hit = Some(i);
            break;
        }
    }
    assert_eq!(hit, Some(1));
    let (worn, bounced) = hit_mob(&t, Some(HatPart::new("Garland")), "z", &mut rnd);
    assert!(worn.showing && worn.name == "Top Hat");
    let b = bounced.unwrap();
    assert_eq!(b.hat.name, "Garland");
    assert_eq!(b.launcher.as_deref(), Some("z"));
    assert!(b.motion[1] >= 0.2);
    assert!(hit_mob(&t, None, "z", &mut rnd).1.is_none());
}

#[test]
fn a_hat_expires_after_its_lifespan() {
    let mut rnd = JavaRandom::new(1);
    let mut t = launch(HatPart::new("x"), [0.0, 70.0, 0.0], [1.0, 0.0, 0.0], "me", &mut rnd);
    let world = Floor(64.0);
    let mut steps = 0;
    while tick_thrown(&mut t, &world, &[]) == Step::Flying {
        steps += 1;
    }
    assert_eq!(steps, config::HAT_ENTITY_LIFESPAN - 1);
}

#[test]
fn picking_a_lying_hat_with_the_look_ray() {
    let mut rnd = JavaRandom::new(1);
    let mut a = launch(HatPart::new("a"), [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], "me", &mut rnd);
    a.pos = [0.0, 64.0, 3.0];
    let mut b = a.clone();
    b.pos = [0.0, 64.0, 2.0];
    let hats = [a, b];
    assert_eq!(picked(&hats, [0.0, 64.25, 0.0], [0.0, 0.0, 1.0], 4.5), Some(1));
    assert_eq!(picked(&hats, [0.0, 64.25, 0.0], [1.0, 0.0, 0.0], 4.5), None);
    assert_eq!(picked(&hats, [0.0, 64.25, -5.0], [0.0, 0.0, 1.0], 4.5), None);
}

#[test]
fn head_offsets_of_a_player_and_a_pig() {
    // Steve's head: one 8x8x8 box above the neck pivot.
    let steve = [([-4.0, -8.0, -4.0], [4.0, 0.0, 4.0])];
    assert_eq!(head_offsets(&steve), Some(([0.0, -0.5, 0.0], 1.0)));
    // A pig's head: the 8x8x8 head and the 4x3x1 snout (12 < 32, so not the main box).
    let pig = [([-4.0, -4.0, -8.0], [4.0, 4.0, 0.0]), ([-2.0, 0.0, -9.0], [2.0, 3.0, -8.0])];
    assert_eq!(head_offsets(&pig), Some(([0.0, -0.25, -0.25], 1.0)));
    // A bee-sized head: only small boxes, so the top ones are averaged.
    let small = [([-1.0, -2.0, -1.0], [1.0, 0.0, 1.0]), ([1.0, -1.0, -1.0], [2.0, 0.0, 0.0])];
    let (c, s) = head_offsets(&small).unwrap();
    assert_eq!(c[1], -2.0 / 16.0);
    assert_eq!(s, 0.375);
    assert_eq!(head_offsets(&[]), None);
}

#[test]
fn boxes_come_back_from_baked_quads() {
    let quad = |y: f32, x0: f32, x1: f32, z0: f32, z1: f32| [[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]];
    let side = [[-4.0, -8.0, -4.0], [-4.0, 0.0, -4.0], [-4.0, 0.0, 4.0], [-4.0, -8.0, 4.0]];
    let quads = [quad(-8.0, -4.0, 4.0, -4.0, 4.0), quad(0.0, -4.0, 4.0, -4.0, 4.0), side];
    assert_eq!(boxes_from_quads(&quads), vec![([-4.0, -8.0, -4.0], [4.0, 0.0, 4.0])]);
}

#[test]
fn placements_and_toast_timing() {
    assert_eq!(placement("minecraft:ender_dragon"), (4.0, true));
    assert_eq!(placement("minecraft:rabbit"), (0.8, false));
    assert_eq!(placement("minecraft:zombie"), (1.0, false));
    assert_eq!(toast_slide(0), (0.0, false));
    assert_eq!(toast_slide(300), (0.25, false));
    assert_eq!(toast_slide(3000), (1.0, false));
    assert_eq!(toast_slide(5600 + 300).0, 0.75);
    assert!(toast_slide(6200).1);
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn the_menu_lists_owned_hats_sorted_and_searched() {
    let r = reg();
    let mut d = PlayerData::default();
    for n in ["Top Hat", "Adventurer's Hat", "Garland"] {
        d.add_hat(HatPart::new(n));
    }
    d.inventory[2].favourite = true;
    let names = |v: Vec<usize>| v.into_iter().map(|i| d.inventory[i].name.clone()).collect::<Vec<_>>();
    assert_eq!(names(menu_list(&r, &d.inventory, "", Sort::Alpha)), ["Garland", "Adventurer's Hat", "Top Hat"]);
    assert_eq!(names(menu_list(&r, &d.inventory, "HAT", Sort::Alpha)), ["Adventurer's Hat", "Top Hat"]);
    assert_eq!(Sort::Alpha.next(), Sort::Rarity);
    let e = equip_copy(&d.inventory[0]);
    assert!(e.showing && !e.is_new && e.count == 1);
}

#[test]
#[ignore = "needs your generated Hats content in content/mods/hats"]
fn every_hat_has_a_flat_item_definition_and_its_model() {
    // Horizon loads `assets/<ns>/items/*.json` without sub-folders and `models/**` with them.
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/mods/hats/carry/assets/hats");
    for d in &reg().defs {
        let (ns, path) = d.item.split_once(':').unwrap();
        assert_eq!(ns, "hats");
        assert!(!path.contains('/'), "{}", d.item);
        let def: Value = serde_json::from_slice(&std::fs::read(assets.join(format!("items/{path}.json"))).unwrap()).unwrap();
        let model = def["model"]["model"].as_str().unwrap();
        assert_eq!(model, d.item_model);
        let rel = model.strip_prefix("hats:").unwrap();
        assert!(assets.join(format!("models/{rel}.json")).is_file(), "{model}");
        let m: Value = serde_json::from_slice(&std::fs::read(assets.join(format!("models/{rel}.json"))).unwrap()).unwrap();
        assert!(m["display"]["head"].is_object(), "{model}");
        let tex = m["textures"]["0"].as_str().or(m["textures"]["particle"].as_str()).unwrap();
        let t = tex.strip_prefix("hats:").unwrap();
        assert!(assets.join(format!("textures/{t}.png")).is_file(), "{tex}");
    }
}
