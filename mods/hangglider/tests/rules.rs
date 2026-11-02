use horizon_hangglider::*;

fn glider_stack(id: &str, damage: u32) -> Stack {
    Stack { item: id.into(), damage }
}

/// A player in the air, falling slowly, facing south (yaw 0).
fn falling() -> Player {
    Player { position: [0.5, 100.0, 0.5], velocity: [0.0, -0.2, 0.0], ..Default::default() }
}

#[test]
fn use_toggles_a_valid_glider_only() {
    let mut s = State::default();
    let g = glider_stack(HANG_GLIDER, 0);
    assert_eq!(on_use(&mut s, Some(&g), false), Use::Toggled);
    assert!(s.deployed);
    assert_eq!(on_use(&mut s, Some(&g), false), Use::Toggled);
    assert!(!s.deployed);
    // With an elytra on: the warning icon, nothing changes.
    assert_eq!(on_use(&mut s, Some(&g), true), Use::Elytra);
    assert!(!s.deployed);
    // Broken (damage max - 1): FAIL.
    let broken = glider_stack(HANG_GLIDER, 817);
    assert_eq!(on_use(&mut s, Some(&broken), false), Use::Fail);
    assert!(valid(Some(&glider_stack(HANG_GLIDER, 816))));
    assert!(!valid(Some(&broken)));
    assert!(valid(Some(&glider_stack(REINFORCED_HANG_GLIDER, 2200))));
    assert!(!valid(Some(&glider_stack(REINFORCED_HANG_GLIDER, 2201))));
    // Not a glider.
    assert_eq!(on_use(&mut s, Some(&glider_stack("minecraft:stick", 0)), false), Use::Pass);
    assert_eq!(on_use(&mut s, None, false), Use::Pass);
}

#[test]
fn deployed_glider_glides_only_when_allowed() {
    let g = glider_stack(HANG_GLIDER, 0);
    let mut s = State { deployed: true, gliding: false };
    let out = end_tick(&mut s, Some(&g), None, &falling(), false, |_| 1);
    assert!(s.gliding && s.deployed);
    assert!(out.velocity.is_some() && out.fall_distance_one);
    for blocked in [
        Player { on_ground: true, ..falling() },
        Player { passenger: true, ..falling() },
        Player { levitation: true, ..falling() },
        Player { flying: true, ..falling() },
        Player { in_water: true, ..falling() },
        Player { velocity: [0.0, 0.0, 0.0], ..falling() },
    ] {
        let mut s = State { deployed: true, gliding: true };
        let out = end_tick(&mut s, Some(&g), None, &blocked, false, |_| 1);
        assert!(!s.gliding && s.deployed, "{blocked:?}");
        assert_eq!(out, Outcome::default());
    }
}

#[test]
fn letting_go_of_the_glider_folds_it() {
    let mut s = State { deployed: true, gliding: true };
    end_tick(&mut s, None, None, &falling(), false, |_| 1);
    assert_eq!(s, State::default());
    // A broken one in the hand folds too.
    let mut s = State { deployed: true, gliding: true };
    end_tick(&mut s, Some(&glider_stack(HANG_GLIDER, 817)), None, &falling(), false, |_| 1);
    assert_eq!(s, State::default());
    // An elytra put on folds it.
    let mut s = State { deployed: true, gliding: true };
    end_tick(&mut s, Some(&glider_stack(HANG_GLIDER, 0)), None, &Player { chest_elytra: true, ..falling() }, false, |_| 1);
    assert_eq!(s, State::default());
}

#[test]
fn the_main_hand_glider_is_the_one_used() {
    let main = glider_stack(HANG_GLIDER, 817); // broken
    let off = glider_stack(REINFORCED_HANG_GLIDER, 0);
    assert_eq!(glider_hand(Some(&main), Some(&off)), Some(Hand::Main));
    let mut s = State { deployed: true, gliding: false };
    end_tick(&mut s, Some(&main), Some(&off), &falling(), false, |_| 1);
    assert!(!s.deployed, "getGliderInHand finds the broken main hand one");
    let mut s = State { deployed: true, gliding: false };
    end_tick(&mut s, Some(&glider_stack("minecraft:stone", 0)), Some(&off), &falling(), false, |_| 1);
    assert!(s.gliding, "the off hand's");
}

#[test]
fn movement_is_the_originals_formula() {
    let g = glider(HANG_GLIDER).unwrap();
    let stack = glider_stack(HANG_GLIDER, 0);
    let p = Player { position: [0.0, 0.0, 0.0], velocity: [0.1, -0.3, 0.0], yaw: 0.0, ..Default::default() };
    // At (0, 0) the noise is 0 and at y 0 the height factor is 0: no wind.
    assert_eq!(wind(&p, &stack, g), 0.0);
    let (v, yaw) = gliding_movement(&p, &stack, g);
    assert_eq!(yaw, 0.0);
    // cos(90 deg) * 0.025, sin(90 deg) * 0.025; vy * 0.55; air resistance 0.985 on x and z.
    let expect = [(0.1 + (90f64.to_radians()).cos() * 0.025) * 0.985, -0.3 * 0.55, (0.0 + 0.025) * 0.985];
    for i in 0..3 {
        assert!((v[i] - expect[i]).abs() < 1e-12, "{i}: {} vs {}", v[i], expect[i]);
    }
    // Shift: the fast numbers.
    let fast = Player { descending: true, ..p };
    let (v, _) = gliding_movement(&fast, &stack, g);
    assert!((v[1] - -0.3 * 0.675).abs() < 1e-12);
    assert!((v[2] - 0.05 * 0.985).abs() < 1e-12);
    // The reinforced glider: its own numbers.
    let r = glider(REINFORCED_HANG_GLIDER).unwrap();
    let (v, _) = gliding_movement(&p, &glider_stack(REINFORCED_HANG_GLIDER, 0), r);
    assert!((v[2] - 0.04 * 0.99).abs() < 1e-12);
    // Fall distance 1 only while falling slower than 0.5 a tick (after the 0.55 factor).
    let mut s = State { deployed: true, gliding: false };
    let slow = Player { velocity: [0.0, -0.9, 0.0], ..falling() };
    assert!(end_tick(&mut s, Some(&stack), None, &slow, false, |_| 1).fall_distance_one);
    let quick = Player { velocity: [0.0, -0.95, 0.0], ..falling() };
    assert!(!end_tick(&mut s, Some(&stack), None, &quick, false, |_| 1).fall_distance_one);
}

#[test]
fn yaw_math_matches_java() {
    // Java: (float) (37.25f + 0.123456789) = 37.373455
    let p = Player { yaw: 37.25, ..Default::default() };
    assert_eq!((f64::from(p.yaw) + 0.123456789) as f32, 37.373455_f32);
    // Java: Math.cos(Math.toRadians(37.25f + 90)), Math.sin(..)
    let a = f64::from(37.25f32 + 90.0).to_radians();
    assert!((a.cos() - -0.6052939880428942).abs() < 1e-15);
    assert!((a.sin() - 0.796002002534622).abs() < 1e-15);
}

#[test]
fn noise_matches_the_java_class() {
    // `new OpenSimplexNoise().eval(x, y)`, printed by the Java class of the mod (JBR 17).
    let reference = [
        (0.0, 0.0, 0.0),
        (1.5, -2.25, -0.5845488622067716),
        (10.3 / 19.0, -4.7 / 19.0, -0.5717654956297166),
        (123.4 / 19.0, 567.8 / 19.0, -0.20633779840999808),
        (-1000.0 / 19.0, 33.0 / 19.0, -0.2529664603060105),
        (0.25, 0.75, -0.6959276729463076),
        (-7.9, 3.3, 0.6707212589226336),
    ];
    for (x, y, v) in reference {
        let got = noise().eval(x, y);
        assert!((got - v).abs() < 1e-12, "({x}, {y}): {got} vs {v}");
    }
}

#[test]
fn wind_grows_with_height_rain_wear_and_glider() {
    let new = glider_stack(HANG_GLIDER, 0);
    let g = glider(HANG_GLIDER).unwrap();
    let at = |y: f64| Player { position: [10.3, y, -4.7], velocity: [0.0, -0.1, 0.0], ..Default::default() };
    let base = wind(&at(128.0), &new, g);
    // noise * 0.15 / (0 * 0.4 + 1) * (128 / 256 * 1.5) * 1 * 1.4
    let expect = -0.5717654956297166 * 0.15 * (128.0 / 256.0 * 1.5) * 1.4;
    assert!((base - expect).abs() < 1e-12, "{base} vs {expect}");
    // Above 256 the height factor stays 1.5.
    assert!((wind(&at(300.0), &new, g) - wind(&at(256.0), &new, g)).abs() < 1e-15);
    // Rain: x3.
    let rain = Player { raining: true, ..at(128.0) };
    assert!((wind(&rain, &new, g) - base * 3.0).abs() < 1e-12);
    // A half worn glider: x(1 + 0.7 * 0.5).
    let worn = glider_stack(HANG_GLIDER, 409);
    assert!((wind(&at(128.0), &worn, g) - base * (1.0 + 0.7 * 409.0 / 818.0)).abs() < 1e-12);
    // The reinforced one: 0.75 instead of 1.4.
    let r = glider(REINFORCED_HANG_GLIDER).unwrap();
    let rs = glider_stack(REINFORCED_HANG_GLIDER, 0);
    assert!((wind(&at(128.0), &rs, r) - base / 1.4 * 0.75).abs() < 1e-12);
    // Speed steadies it: / (speed * 0.4 + 1).
    let fast = Player { velocity: [3.0, -0.1, 4.0], ..at(128.0) };
    assert!((wind(&fast, &new, g) - base / 3.0).abs() < 1e-12);
}

#[test]
fn wear_rolls_one_in_two_hundred_on_the_server_only() {
    let g = glider_stack(HANG_GLIDER, 0);
    let mut s = State { deployed: true, gliding: false };
    let mut asked = None;
    let out = end_tick(&mut s, Some(&g), None, &falling(), true, |n| {
        asked = Some(n);
        0
    });
    assert_eq!(asked, Some(200));
    assert_eq!(out.wear, Some(Hand::Main));
    let out = end_tick(&mut s, Some(&g), None, &falling(), true, |_| 7);
    assert_eq!(out.wear, None);
    // The client never wears it.
    let out = end_tick(&mut s, Some(&g), None, &falling(), false, |_| 0);
    assert_eq!(out.wear, None);
    // Off hand.
    let out = end_tick(&mut s, None, Some(&g), &falling(), true, |_| 0);
    assert_eq!(out.wear, Some(Hand::Off));
}

#[test]
fn fov_widens_only_while_gliding() {
    let gliding = State { deployed: true, gliding: true };
    assert_eq!(fov(&gliding, false, 1.0), 1.05);
    assert_eq!(fov(&gliding, true, 1.0), 1.1);
    assert_eq!(fov(&State { deployed: true, gliding: false }, true, 1.0), 1.0);
}

#[test]
fn item_numbers_are_the_jars() {
    let ops = item_ops();
    let find = |id: &str| ops.iter().find(|(i, _)| *i == id).unwrap().1.clone();
    let h = find(HANG_GLIDER);
    assert!(h.contains(&(&["minecraft:max_damage"][..], Val::Int(818))));
    assert!(h.contains(&(&["minecraft:repairable", "items"][..], Val::Str("minecraft:leather"))));
    let r = find(REINFORCED_HANG_GLIDER);
    assert!(r.contains(&(&["minecraft:max_damage"][..], Val::Int(2202))));
    assert!(r.contains(&(&["minecraft:repairable", "items"][..], Val::Str("minecraft:phantom_membrane"))));
    assert!(r.contains(&(&["minecraft:max_stack_size"][..], Val::Int(1))));
}

#[test]
fn carry_gliders_are_not_elytras() {
    let carry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/mods/hangglider/carry");
    if !carry.is_dir() {
        return;
    }
    for id in ["hang_glider", "reinforced_hang_glider"] {
        let decl = std::fs::read_to_string(carry.join(format!("data/hangglider/horizon/item/{id}.json"))).unwrap();
        assert!(decl.contains("\"minecraft:paper\""), "{decl}");
    }
    let equippable = carry.join("data/minecraft/tags/item/enchantable/equippable.json");
    assert!(!equippable.exists());
    let recipe = std::fs::read_to_string(carry.join("data/hangglider/recipe/reinforced_hang_glider.json")).unwrap();
    assert!(recipe.contains("minecraft:crafting_transmute"), "{recipe}");
}
