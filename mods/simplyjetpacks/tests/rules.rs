use horizon_simplyjetpacks::*;

const CONFIG: &str = r"../../content/config/simplyjetpacks-common.toml";
const CARRY: &str = r"../../content/mods/simplyjetpacks/carry";

fn jet(item: &str) -> &'static Kind {
    kind(&format!("simplyjetpacks:{item}")).expect(item)
}
fn air(up: bool) -> Input {
    Input { up, ..Input::default() }
}

#[test]
fn every_jetpack_has_tuned_tuning() {
    assert_eq!(KINDS.len(), 35);
    for k in KINDS {
        assert!(TUNINGS.iter().any(|t| t.key == k.config), "{}", k.item);
    }
    // Spot checks against the config file itself, when it is there.
    if let Ok(text) = std::fs::read_to_string(CONFIG) {
        for t in TUNINGS {
            let at = text.find(&format!(".{}]", t.key)).unwrap_or_else(|| panic!("{}", t.key));
            let section = &text[at..];
            let value = |name: &str| -> String {
                let s = &section[section.find(&format!("{name} = ")).unwrap() + name.len() + 3..];
                s.split_whitespace().next().unwrap().to_string()
            };
            assert_eq!(value("energyCapacity"), t.energy_capacity.to_string(), "{}", t.key);
            assert_eq!(value("energyUsage"), t.energy_usage.to_string(), "{}", t.key);
            assert_eq!(value("speedVertical").parse::<f64>().unwrap(), t.speed_vertical, "{}", t.key);
            assert_eq!(value("hoverMode"), t.hover_mode.to_string(), "{}", t.key);
        }
    }
    assert_eq!(jet("jetpack_vanilla4").rarity(), "rare");
    assert_eq!(jet("jetpack_te5_enderium").rarity(), "epic");
    assert_eq!(jet("jetpack_vanilla3").rarity(), "uncommon");
    assert_eq!(jet("jetpack_vanilla1").enchantability(), 14);
    assert_eq!(jet("jetpack_potato").enchantability(), 0);
    assert_eq!(jet("jetpack_te5").material.numbers().0, 12.0);
    assert_eq!(jet("jetpack_mek2_armored").material.numbers().0, 4.0);
    assert!(jet("jetpack_creative_armored").creative());
    assert_eq!(kind("simplyjetpacks:thruster_vanilla1"), None);
}

#[test]
fn engine_off_does_nothing() {
    let k = jet("jetpack_vanilla2");
    let mut p = Pack { energy: 1000, ..Pack::default() };
    assert_eq!(fly(k, &mut p, &air(true), false, 0), Fly::default());
    assert_eq!(p.energy, 1000);
}

#[test]
fn thrust_climbs_to_the_tier_speed_and_uses_energy() {
    let k = jet("jetpack_vanilla2"); // speedVertical 0.3, accel 0.12, usage 50
    let mut p = Pack { engine: true, energy: 1000, ..Pack::default() };
    let f = fly(k, &mut p, &Input { up: true, vy: 0.0, ..Input::default() }, false, 0);
    // accel doubles (x2.5) while vy < 0.3: min(0 + 0.3, 0.3)
    assert_eq!(f.vy, Some(0.3));
    assert_eq!((f.used, p.energy), (50, 950));
    let f = fly(k, &mut p, &Input { up: true, vy: 0.31, ..Input::default() }, false, 0);
    assert_eq!(f.vy, Some(0.3));
    // In water the top speed is 40 %.
    let f = fly(k, &mut p, &Input { up: true, vy: 0.0, in_water: true, ..Input::default() }, false, 0);
    assert!((f.vy.unwrap() - 0.12).abs() < 1e-12);
    // Sprinting costs speed * sprintEnergyModifier (te3: 2.3).
    let te3 = jet("jetpack_te3");
    let mut q = Pack { engine: true, energy: 10_000, ..Pack::default() };
    let f = fly(te3, &mut q, &Input { up: true, sprinting: true, ..Input::default() }, false, 0);
    assert_eq!(f.used, 460);
}

#[test]
fn energy_runs_out() {
    let k = jet("jetpack_vanilla1");
    let mut p = Pack { engine: true, energy: 20, ..Pack::default() };
    let f = fly(k, &mut p, &air(true), false, 0);
    // useEnergy first, then no push at 0.
    assert_eq!((f.used, p.energy, f.vy, f.pushed), (20, 0, None, false));
    // The creative jetpack never uses any.
    let c = jet("jetpack_creative");
    let mut q = Pack { engine: true, ..Pack::default() };
    let f = fly(c, &mut q, &air(true), false, 0);
    assert_eq!((f.used, f.vy), (0, Some(0.375f64.min(0.9))));
}

#[test]
fn hover_holds_and_descends() {
    let k = jet("jetpack_vanilla3"); // hover 0.34, slow 0.03, accel 0.13
    let mut p = Pack { engine: true, hover: true, energy: 100_000, ..Pack::default() };
    // In the air without keys: sinks slowly (min(vy + accel, -hoverSpeed)); not inverted, not holding down -> slow.
    let f = fly(k, &mut p, &Input { vy: -0.5, ..Input::default() }, false, 0);
    assert!((f.vy.unwrap() - (-0.5 + 0.325)).abs() < 1e-12);
    let f = fly(k, &mut p, &Input { vy: 0.0, ..Input::default() }, false, 0);
    assert_eq!(f.vy, Some(-0.03));
    // Holding down: the fast hover speed.
    let f = fly(k, &mut p, &Input { vy: 0.0, down: true, ..Input::default() }, false, 0);
    assert_eq!(f.vy, Some(-0.34));
    // Up while hovering: climbs at the hover speed, up + down: descends slowly.
    let f = fly(k, &mut p, &Input { vy: 0.0, up: true, ..Input::default() }, false, 0);
    assert_eq!(f.vy, Some(0.325f64.min(0.34)));
    let f = fly(k, &mut p, &Input { vy: 0.0, up: true, down: true, ..Input::default() }, false, 0);
    assert_eq!(f.vy, Some(-0.03));
    // On the ground with hover and no key: nothing, no energy used.
    let before = p.energy;
    let f = fly(k, &mut p, &Input { on_ground: true, ..Input::default() }, false, 0);
    assert_eq!((f.vy, p.energy), (None, before));
}

#[test]
fn sideways_pushes_follow_the_throttle() {
    let k = jet("jetpack_vanilla4"); // sideways 0.16, sprint speed 1.9
    let mut p = Pack { engine: true, energy: 1_000_000, throttle: Some(50), ..Pack::default() };
    let f = fly(k, &mut p, &Input { up: true, forwards: true, left: true, ..Input::default() }, false, 0);
    let side = (0.16f32 * 0.5) as f64;
    let fwd = ((0.16f32 * 0.5) * 0.5) as f64;
    assert_eq!(f.pushes, vec![[0.0, fwd], [side, 0.0]]);
    let w = move_relative([0.0, 1.0], 0.0);
    assert!((w[0]).abs() < 1e-9 && (w[1] - 1.0).abs() < 1e-9);
    let w = move_relative([0.0, 1.0], 90.0);
    assert!((w[0] + 1.0).abs() < 1e-6 && w[1].abs() < 1e-6);
}

#[test]
fn fuel_efficiency_cuts_usage() {
    assert_eq!(energy_usage(450, 0), 450);
    assert_eq!(energy_usage(450, 1), 360);
    assert_eq!(energy_usage(450, 4), 90);
    assert_eq!(energy_usage(32, 3), 13);
}

#[test]
fn keys_toggle_only_what_the_tier_has() {
    let v1 = jet("jetpack_vanilla1"); // hover yes, ehover no, charger no
    let mut p = Pack::default();
    assert_eq!(apply(v1, &mut p, Action::Engine), Some(Toggled { key: "chat.simplyjetpacks.itemJetpack.engineMode", on: true }));
    assert!(p.engine);
    assert!(apply(v1, &mut p, Action::Hover).is_some_and(|t| t.on));
    assert_eq!(apply(v1, &mut p, Action::EHover), None);
    assert_eq!(apply(v1, &mut p, Action::Charger), None);
    assert!(!p.ehover && !p.charger);
    let v4 = jet("jetpack_vanilla4");
    assert!(apply(v4, &mut p, Action::Charger).is_some_and(|t| t.on));
    for _ in 0..3 {
        apply(v4, &mut p, Action::ThrottleUp);
    }
    assert_eq!(p.throttle(), 100);
    for _ in 0..12 {
        apply(v4, &mut p, Action::ThrottleDown);
    }
    assert_eq!(p.throttle(), 0);
    assert_eq!(Action::from_code(Action::ThrottleDown.code()), Some(Action::ThrottleDown));
    assert_eq!(Action::from_code(9), None);
}

#[test]
fn emergency_hover_catches_deadly_falls() {
    let k = jet("jetpack_vanilla3"); // emergencyHoverMode = true
    let p = Pack { ehover: true, energy: 10, ..Pack::default() };
    let fall = |fd: f64, ground: bool| Danger { y: 64.0, fall_distance: fd, health: 20.0, creative_mode: false, on_ground: ground, in_water: false };
    assert!(emergency(k, &p, &fall(21.2, false)));
    assert!(!emergency(k, &p, &fall(10.0, false)));
    assert!(!emergency(k, &p, &fall(30.0, true)));
    assert!(emergency(k, &p, &Danger { y: -70.0, ..fall(0.0, false) }));
    // Already hovering with the engine on: nothing to do.
    let on = Pack { engine: true, hover: true, ..p.clone() };
    assert!(!emergency(k, &on, &fall(30.0, false)));
    // No energy: nothing.
    assert!(!emergency(k, &Pack { energy: 0, ..p.clone() }, &fall(30.0, false)));
    let mut q = p.clone();
    do_emergency(&mut q);
    assert!(q.engine && q.hover);
}

#[test]
fn the_potato_launches_after_fifty_ticks_and_explodes_empty() {
    let k = jet("jetpack_potato");
    let mut p = Pack { engine: true, energy: 1200, ..Pack::default() };
    let still = Input::default();
    let first = potato(k, &mut p, &still, true);
    assert!(p.timer_set && p.timer == 50 && !first.launched);
    let mut launched = 0;
    for _ in 0..50 {
        if potato(k, &mut p, &still, true).launched {
            launched += 1;
        }
    }
    assert_eq!((launched, p.fired), (1, true));
    // Fired: it pushes up at full speed until the 1200 FE (45 per call) are gone, then explodes.
    let mut calls = 0;
    loop {
        calls += 1;
        let s = potato(k, &mut p, &still, false);
        if s.explodes {
            break;
        }
        assert!(s.fly.vy.is_some());
        assert!(calls < 100);
    }
    assert_eq!(calls, 27);
    assert!(is_flying(k, &p, false, true));
}

#[test]
fn hud_text_is_the_originals() {
    let tr = |key: &str, args: &[String]| -> String {
        let pattern = match key {
            "hud.simplyjetpacks.throttle" => "Throttle: %s%%",
            "hud.simplyjetpacks.energyDisplay" => "Energy: %s",
            "hud.simplyjetpacks.energyDepleted" => "Depleted",
            "hud.simplyjetpacks.infiniteEnergy" => "Infinite",
            "hud.simplyjetpacks.jetpackStates.2" => "%s - %s",
            "hud.simplyjetpacks.jetpackStates.4" => "%s - %s - %s - %s",
            "hud.simplyjetpacks.engine" => "Engine",
            "hud.simplyjetpacks.hover" => "Hover",
            "hud.simplyjetpacks.eHover" => "EHover",
            "hud.simplyjetpacks.charger" => "Charger",
            other => other,
        };
        fill(pattern, args)
    };
    let k = jet("jetpack_vanilla1");
    let p = Pack { engine: true, energy: 60_000, ..Pack::default() };
    assert_eq!(hud_lines(k, &p, &tr), vec!["Throttle: 100%", "Energy: §a75%§r", "§aEngine§r - §cHover§r"]);
    let empty = Pack::default();
    assert_eq!(hud_lines(k, &empty, &tr)[1], "Energy: §cDepleted§r");
    let c = jet("jetpack_creative");
    let lines = hud_lines(c, &Pack { ehover: true, ..Pack::default() }, &tr);
    assert_eq!(lines[1], "Energy: §dInfinite§r");
    assert_eq!(lines[2], "§cEngine§r - §cHover§r - §aEHover§r - §cCharger§r");
    assert_eq!(energy_percent(1, 80_000), 1);
    assert_eq!(fill("%2$s/%1$s", &["a".into(), "b".into()]), "b/a");
}

#[test]
fn particles_leave_the_two_nozzles() {
    let n = nozzles(false, 0.0, false);
    assert_eq!(n.len(), 3);
    assert_eq!(n[0], [-0.18, -0.9, -0.3]);
    assert_eq!(nozzles(true, 0.0, true), vec![[-0.18, -1.0, -0.6], [0.18, -1.0, -0.6]]);
    let r = rotate_yaw([1.0, 0.0, 0.0], 90.0);
    assert!(r[0].abs() < 1e-9 && (r[2] - 1.0).abs() < 1e-9);
    assert_eq!(jet("jetpack_vanilla1").default_particle(), Particle::Flame);
    assert_eq!(jet("jetpack_potato").default_particle(), Particle::Rainbow);
    assert_eq!(Pack { particle: Some(2), ..Pack::default() }.particle(jet("jetpack_te1")), Particle::Smoke);
}

#[test]
fn carry_declares_wearables() {
    let dir = std::path::Path::new(CARRY).join("data/simplyjetpacks/horizon/item");
    if !dir.exists() {
        return;
    }
    for k in KINDS {
        let decl = std::fs::read_to_string(dir.join(format!("{}.json", k.item))).unwrap();
        assert!(decl.contains("minecraft:iron_chestplate"), "{}", k.item);
        assert!(decl.contains(&format!("\"rarity\": \"{}\"", k.rarity())), "{}", k.item);
    }
    for (g, t) in [("pilot_goggles_gold", "minecraft:golden_helmet"), ("pilot_goggles_iron", "minecraft:iron_helmet")] {
        let decl = std::fs::read_to_string(dir.join(format!("{g}.json"))).unwrap();
        assert!(decl.contains(t), "{g}");
    }
}
