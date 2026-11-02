use horizon_morebows::*;
use std::collections::BTreeMap;

/// `nextInt` answers in order.
struct Ints(Vec<i32>, Vec<i32>);
impl Ints {
    fn new(answers: &[i32]) -> Self {
        Ints(answers.to_vec(), Vec::new())
    }
}
impl Rng for Ints {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.1.push(bound);
        self.0.remove(0)
    }
}

fn bow(name: &str) -> &'static Bow {
    bow_of(&format!("morebows:{name}")).expect(name)
}
const FULL: Release = Release { charge: 200, power_bonus: 0.0, flame: false, punch: 0, infinite_ammo: false };

#[test]
fn the_eight_bows_have_the_defaults_of_the_config() {
    // ConfigBows: BowConfig(durability, damageMult, drawbackDiv)
    let table = [
        ("diamond_bow", 1016, 2.25, 6.0),
        ("ender_bow", 215, 1.0, 22.0),
        ("flame_bow", 576, 2.0, 15.0),
        ("frost_bow", 550, 1.0, 26.0),
        ("gold_bow", 68, 2.5, 6.0),
        ("iron_bow", 550, 1.5, 17.0),
        ("multi_bow", 550, 1.0, 13.0),
        ("stone_bow", 484, 1.15, 20.0),
    ];
    assert_eq!(BOWS.len(), 8);
    for (name, durability, mult, div) in table {
        let b = bow(name);
        assert_eq!((b.durability, b.damage_mult, b.drawback_div), (durability, mult, div), "{name}");
    }
    assert!(bow_of("morebows:nothing").is_none());
    assert!(bow_of("minecraft:bow").is_none());
}

#[test]
fn rarity_arrow_type_and_multishot_follow_get_all_items() {
    let kind = |n: &str| {
        let b = bow(n);
        (b.rarity, b.arrow, b.multi, b.fire_resistant)
    };
    assert_eq!(kind("diamond_bow"), ("rare", ArrowType::NotCustom, false, false));
    assert_eq!(kind("gold_bow"), ("uncommon", ArrowType::NotCustom, false, false));
    assert_eq!(kind("ender_bow"), ("epic", ArrowType::Ender, true, false));
    assert_eq!(kind("stone_bow"), ("common", ArrowType::NotCustom, false, false));
    assert_eq!(kind("iron_bow"), ("common", ArrowType::NotCustom, false, false));
    assert_eq!(kind("multi_bow"), ("rare", ArrowType::NotCustom, true, false));
    assert_eq!(kind("flame_bow"), ("uncommon", ArrowType::Fire, false, true));
    assert_eq!(kind("frost_bow"), ("common", ArrowType::Frost, false, false));
}

#[test]
fn the_draw_speed_is_the_charge_over_the_divisor() {
    // v = charge / powerDiv; (v*v + v*2) / 3
    assert!((raw_velocity(20, 20.0) - 1.0).abs() < 1e-6);
    assert!((raw_velocity(6, 6.0) - 1.0).abs() < 1e-6, "a gold bow is full at 6 ticks");
    assert!((raw_velocity(10, 20.0) - ((0.25 + 1.0) / 3.0)).abs() < 1e-6);
    assert!(raw_velocity(1, 26.0) < 0.1, "a frost bow needs a longer draw");
}

#[test]
fn a_draw_that_is_too_short_shoots_nothing() {
    let mut rng = Ints::new(&[]);
    // diamond bow: v = 1/6 -> (1/36 + 1/3)/3 = 0.1204 >= 0.1; 0 ticks -> 0
    assert!(plan_shot(bow("diamond_bow"), &Release { charge: 0, ..FULL }, &mut rng).is_none());
    assert!(plan_shot(bow("diamond_bow"), &Release { charge: 1, ..FULL }, &mut rng).is_some());
    assert!(plan_shot(bow("frost_bow"), &Release { charge: 1, ..FULL }, &mut rng).is_none());
    assert!(rng.1.is_empty(), "no draw for a single-arrow bow");
}

#[test]
fn a_full_draw_is_critical_and_clamped() {
    let shot = plan_shot(bow("iron_bow"), &FULL, &mut Ints::new(&[])).unwrap();
    assert!(shot.critical);
    assert_eq!(shot.shot_velocity, 1.0);
    assert_eq!(shot.arrows.len(), 1);
    assert_eq!(shot.arrows[0].speed, 3.0, "shotVelocity * (2.0F * 1.5F)");
    let half = plan_shot(bow("iron_bow"), &Release { charge: 8, ..FULL }, &mut Ints::new(&[])).unwrap();
    assert!(!half.critical);
    assert!(half.shot_velocity < 1.0);
}

#[test]
fn damage_is_two_plus_power_times_the_multiplier() {
    let r = Release { power_bonus: 3.0, ..FULL }; // Power V: 5 * 0.5 + 0.5
    assert_eq!(base_damage(bow("diamond_bow"), &r, 0), (2.0 + 3.0) * 2.25);
    assert_eq!(base_damage(bow("diamond_bow"), &FULL, 0), 2.0 * 2.25);
    assert_eq!(base_damage(bow("gold_bow"), &FULL, 0), 5.0);
    assert_eq!(base_damage(bow("stone_bow"), &FULL, 0), 2.0 * 1.15);
    assert_eq!(base_damage(bow("frost_bow"), &FULL, 0), 2.0);
}

#[test]
fn flame_makes_the_flame_bows_arrows_hit_a_quarter_harder() {
    let flame = Release { flame: true, ..FULL };
    assert_eq!(base_damage(bow("flame_bow"), &flame, 0), 2.0 * 1.25 * 2.0);
    assert_eq!(base_damage(bow("flame_bow"), &FULL, 0), 4.0);
    // other bows: Flame only sets the fire
    assert_eq!(base_damage(bow("iron_bow"), &flame, 0), 2.0 * 1.5);
}

#[test]
fn fire_seconds_follow_the_flame_enchantment_and_the_bow() {
    assert_eq!(fire_seconds(bow("flame_bow"), false), 50);
    assert_eq!(fire_seconds(bow("flame_bow"), true), 100);
    assert_eq!(fire_seconds(bow("iron_bow"), true), 100);
    assert_eq!(fire_seconds(bow("iron_bow"), false), 0);
    assert_eq!(fire_seconds(bow("frost_bow"), true), 0, "a frost arrow puts itself out on its first tick");
}

#[test]
fn the_multi_bow_shoots_two_arrows_or_three_one_time_in_four() {
    let two = plan_shot(bow("multi_bow"), &FULL, &mut Ints::new(&[1])).unwrap();
    assert_eq!(two.arrows.len(), 2);
    let mut rng = Ints::new(&[0]);
    let three = plan_shot(bow("multi_bow"), &FULL, &mut rng).unwrap();
    assert_eq!(three.arrows.len(), 3);
    assert_eq!(rng.1, [4], "nextInt(4) == 0");
    // speeds: sv * (2.0F*1.5F), (1.65F*1.5F), (1.275F*1.5F)
    let speeds: Vec<f32> = three.arrows.iter().map(|a| a.speed).collect();
    assert_eq!(speeds, [3.0, 1.65f32 * 1.5f32, 1.275f32 * 1.5f32]);
    // damage: baseDamage 2 * damageMult 1 * 1.5 / 1.3 / 1.15
    let d: Vec<f64> = three.arrows.iter().map(|a| a.base_damage).collect();
    assert_eq!(d, [2.0 * 1.5, 2.0 * 1.3, 2.0 * 1.15]);
    // the extra arrows go beside the first, left and right, and cannot be picked up
    let sides: Vec<f64> = three.arrows.iter().map(|a| a.x_offset_sign).collect();
    assert_eq!(sides, [0.0, 1.0, -1.0]);
    let pick: Vec<bool> = three.arrows.iter().map(|a| a.creative_only).collect();
    assert_eq!(pick, [false, true, true]);
    assert!(three.arrows.iter().all(|a| a.custom == ArrowType::NotCustom && a.delay_ticks == 0));
    assert_eq!(three.ammo_used, 1);
}

#[test]
fn the_multi_bow_makes_one_sound_per_arrow_beside_the_player() {
    let three = plan_shot(bow("multi_bow"), &FULL, &mut Ints::new(&[0])).unwrap();
    let sides: Vec<f64> = three.sounds.iter().map(|s| s.x_offset_sign).collect();
    assert_eq!(sides, [0.0, 1.0, -1.0]);
    let two = plan_shot(bow("multi_bow"), &FULL, &mut Ints::new(&[2])).unwrap();
    assert_eq!(two.sounds.len(), 2);
    let one = plan_shot(bow("iron_bow"), &FULL, &mut Ints::new(&[])).unwrap();
    assert_eq!(one.sounds.len(), 1);
    // pitch = 1 / (random * 0.4 + 1.2) + sv * 0.5
    let s = &one.sounds[0];
    assert_eq!(s.event, "minecraft:entity.arrow.shoot");
    assert!((s.pitch_of(0.0, 1.0) - (1.0 / 1.2 + 0.5)).abs() < 1e-6);
}

#[test]
fn the_ender_bow_shoots_six_arrows_the_first_at_once_the_rest_after_three_seconds() {
    let mut rng = Ints::new(&[]);
    let shot = plan_shot(bow("ender_bow"), &FULL, &mut rng).unwrap();
    assert!(rng.1.is_empty(), "six arrows, no draw");
    assert_eq!(shot.arrows.len(), 6);
    let speeds: Vec<f32> = shot.arrows.iter().map(|a| a.speed).collect();
    assert_eq!(
        speeds,
        [3.0, 1.0f32 * 1.5f32, 1.2f32 * 1.5f32, 1.5f32 * 1.5f32, 1.75f32 * 1.5f32, 1.825f32 * 1.5f32]
    );
    let delays: Vec<i32> = shot.arrows.iter().map(|a| a.delay_ticks).collect();
    assert_eq!(delays, [1, 61, 61, 61, 61, 61]);
    assert!(shot.arrows.iter().all(|a| a.custom == ArrowType::Ender && a.base_damage == 2.0));
    let pick: Vec<bool> = shot.arrows.iter().map(|a| a.creative_only).collect();
    assert_eq!(pick, [false, true, true, true, true, true]);
    // the ender bow does not offset its arrows sideways; the spawner moves the delayed ones
    assert!(shot.arrows.iter().all(|a| a.x_offset_sign == 0.0));
    assert_eq!(shot.arrows[2].spawn_offset, [-1.25, 1.0, 1.75]);
    assert_eq!(shot.arrows[3].spawn_offset, [-2.25, 1.45, -0.75]);
    assert_eq!(shot.arrows[4].spawn_offset, [0.25, 2.0, 2.5]);
    assert_eq!(shot.arrows[5].spawn_offset, [1.75, 1.75, 1.5]);
    assert_eq!(shot.arrows[1].spawn_offset, [0.0, 0.0, 0.0]);
    assert_eq!(shot.arrows[0].spawn_offset, [0.0, 0.0, 0.0]);
    // one release sound at the player
    assert_eq!(shot.sounds.len(), 1);
}

#[test]
fn the_ender_arrows_make_their_sound_when_they_turn_up() {
    // (i & 1) != 0: the enderman's teleport, else the arrow's shot; volumes 0.5, 0.25, 1, 0.5
    assert_eq!(spawner_sound(1).event, "minecraft:entity.enderman.teleport");
    assert_eq!(spawner_sound(2).event, "minecraft:entity.arrow.shoot");
    assert_eq!(spawner_sound(3).event, "minecraft:entity.enderman.teleport");
    assert_eq!(spawner_sound(5).event, "minecraft:entity.enderman.teleport");
    let volumes: Vec<f32> = (1..=5).map(|i| spawner_sound(i).volume).collect();
    assert_eq!(volumes, [0.5, 1.0, 0.25, 1.0, 0.5]);
    // arrow 3: 1 / (random * 0.4 + 1.0) + sv * 0.3
    assert!((spawner_sound(3).pitch_of(0.5, 1.0) - (1.0 / 1.2 + 0.3)).abs() < 1e-6);
}

#[test]
fn infinity_and_creative_keep_the_ammo_and_make_the_first_arrow_creative_only() {
    let inf = Release { infinite_ammo: true, ..FULL };
    let shot = plan_shot(bow("iron_bow"), &inf, &mut Ints::new(&[])).unwrap();
    assert_eq!(shot.ammo_used, 0);
    assert!(shot.arrows[0].creative_only);
    let normal = plan_shot(bow("iron_bow"), &FULL, &mut Ints::new(&[])).unwrap();
    assert_eq!(normal.ammo_used, 1);
    assert!(!normal.arrows[0].creative_only);
}

#[test]
fn the_flame_and_frost_bows_shoot_custom_arrows() {
    let flame = plan_shot(bow("flame_bow"), &FULL, &mut Ints::new(&[])).unwrap();
    assert_eq!(flame.arrows[0].custom, ArrowType::Fire);
    assert_eq!(flame.arrows[0].fire_seconds, 50);
    let frost = plan_shot(bow("frost_bow"), &FULL, &mut Ints::new(&[])).unwrap();
    assert_eq!(frost.arrows[0].custom, ArrowType::Frost);
    assert_eq!(frost.arrows[0].fire_seconds, 0);
    let plain = plan_shot(bow("gold_bow"), &FULL, &mut Ints::new(&[])).unwrap();
    assert_eq!(plain.arrows[0].custom, ArrowType::NotCustom);
}

#[test]
fn a_frost_arrow_slows_and_a_blaze_takes_triple() {
    let hit = frost_hit(false);
    assert_eq!(hit, FrostHit { damage_factor: 1.0, clear_fire: true, slowness: (300, 2) });
    assert_eq!(frost_hit(true).damage_factor, 3.0);
}

// ---- the freezing of a stuck frost arrow

#[derive(Default)]
struct Land {
    cells: BTreeMap<Pos, Cell>,
    survive: bool,
}
impl Column for Land {
    fn cell(&mut self, p: Pos) -> Cell {
        *self.cells.get(&p).unwrap_or(&Cell::Air)
    }
    fn snow_can_survive(&mut self, _p: Pos) -> bool {
        self.survive
    }
}

#[test]
fn a_frost_arrow_in_the_air_over_ground_leaves_a_snow_layer() {
    let mut w = Land { survive: true, ..Land::default() };
    assert_eq!(freeze(&mut w, [1, 64, 1]), Some(Freeze::Snow([1, 64, 1])));
    let mut w = Land { survive: false, ..Land::default() };
    assert_eq!(freeze(&mut w, [1, 64, 1]), None, "no ground under it");
}

#[test]
fn a_frost_arrow_in_water_makes_ice() {
    let mut w = Land::default();
    w.cells.insert([0, 60, 0], Cell::Water);
    assert_eq!(freeze(&mut w, [0, 60, 0]), Some(Freeze::Ice([0, 60, 0])));
}

#[test]
fn a_frost_arrow_in_snow_adds_a_layer_or_climbs_over_full_ones() {
    let mut w = Land::default();
    w.cells.insert([0, 64, 0], Cell::Snow(3));
    assert_eq!(freeze(&mut w, [0, 64, 0]), Some(Freeze::MoreSnow([0, 64, 0], 4)));
    // full layer: the next block up
    w.cells.insert([0, 64, 0], Cell::Snow(8));
    w.cells.insert([0, 65, 0], Cell::Snow(5));
    assert_eq!(freeze(&mut w, [0, 64, 0]), Some(Freeze::MoreSnow([0, 65, 0], 6)));
    // full layers up to air: a new layer there if it can stand
    w.cells.insert([0, 65, 0], Cell::Snow(8));
    w.survive = true;
    assert_eq!(freeze(&mut w, [0, 64, 0]), Some(Freeze::Snow([0, 66, 0])));
    w.survive = false;
    assert_eq!(freeze(&mut w, [0, 64, 0]), None);
    // something else above the full snow: nothing
    w.cells.insert([0, 66, 0], Cell::Other);
    w.survive = true;
    assert_eq!(freeze(&mut w, [0, 64, 0]), None);
    // snow with 7 layers gets the eighth, not a climb (the loop wants more than 7)
    w.cells.insert([0, 64, 0], Cell::Snow(7));
    assert_eq!(freeze(&mut w, [0, 64, 0]), Some(Freeze::MoreSnow([0, 64, 0], 8)));
}

#[test]
fn frost_arrow_ticks() {
    assert_eq!(FROST_NO_PICKUP_TICK, 1);
    assert_eq!(FROST_FREEZE_TICK, 65);
    assert_eq!(ENDER_FIRST_TICK, 1);
    assert_eq!(ENDER_REST_TICK, 61);
}

#[test]
fn the_items_get_the_durability_and_repair_materials_of_the_original() {
    let n = item_numbers();
    assert_eq!(n.len(), 8);
    let find = |id: &str| n.iter().find(|x| x.id == id).unwrap();
    assert_eq!(find("morebows:gold_bow").max_damage, 68);
    assert_eq!(find("morebows:diamond_bow").repair, ["minecraft:diamond"]);
    assert_eq!(find("morebows:frost_bow").repair, ["minecraft:ice", "minecraft:packed_ice", "minecraft:blue_ice"]);
    assert!(find("morebows:flame_bow").fire_resistant);
    assert!(!find("morebows:iron_bow").fire_resistant);
}

#[test]
fn the_fov_zoom_follows_the_divisor() {
    let gold = bow("gold_bow"); // div 6
    assert!((fov_factor(gold, 0) - 1.0).abs() < 1e-6);
    assert!((fov_factor(gold, 6) - 0.85).abs() < 1e-6, "full draw: 1 - 0.15");
    assert!((fov_factor(gold, 60) - 0.85).abs() < 1e-6, "clamped");
    let frost = bow("frost_bow"); // div 26
    assert!((fov_factor(frost, 13) - (1.0 - 0.25 * 0.15)).abs() < 1e-6);
}
