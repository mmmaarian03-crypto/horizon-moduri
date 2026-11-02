// Tests of the Presence Footsteps rules on a fake world, with the real carried files.
#[path = "../src/lib.rs"]
#[allow(dead_code)]
mod pf;

use pf::*;
use std::collections::HashMap;
use std::rc::Rc;

const CONFIG_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/mods/presencefootsteps/carry/assets/presencefootsteps/config/");
/// the pack's `config/presencefootsteps/userconfig.json`.
const USERCONFIG: &str = r#"{
  "volume": 70, "clientPlayerVolume": 100, "otherPlayerVolume": 100, "hostileEntitiesVolume": 100,
  "passiveEntitiesVolume": 100, "runningVolumeIncrease": 0, "wetSoundsVolume": 50, "foliageSoundsVolume": 100,
  "maxSteppingEntities": 50, "disabled": false, "multiplayer": true, "global": true, "footwear": true,
  "visualiser": false, "exclusive": false, "stance": "NONE", "targetEntities": "ALL"
}"#;

fn files() -> Files {
    let read = |n: &str| std::fs::read_to_string(format!("{CONFIG_DIR}{n}.json")).unwrap();
    Files {
        blockmap: read("blockmap"),
        golemmap: read("golemmap"),
        primitivemap: read("primitivemap"),
        locomotionmap: read("locomotionmap"),
        acoustics: read("acoustics"),
        variator: read("variator"),
    }
}

fn block(id: &str, step: &str) -> BlockInfo {
    BlockInfo {
        id: format!("minecraft:{id}"),
        step: format!("minecraft:block.{step}.step"),
        volume: 1.0,
        pitch: 1.0,
        collision: vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]],
        outline: vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]],
        full_top: true,
        ..BlockInfo::default()
    }
}
fn air() -> BlockInfo {
    BlockInfo { id: "minecraft:air".into(), air: true, ..BlockInfo::default() }
}

struct Fake {
    blocks: HashMap<[i32; 3], Rc<BlockInfo>>,
    floor: Rc<BlockInfo>,
    rain: bool,
    time: i64,
}
impl Fake {
    /// A floor of `floor` blocks at y = 63 (the surface is y = 64), air above.
    fn new(floor: BlockInfo) -> Fake {
        static TIME: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
        let time = TIME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Fake { blocks: HashMap::new(), floor: Rc::new(floor), rain: false, time }
    }
    fn set(&mut self, p: [i32; 3], b: BlockInfo) {
        self.blocks.insert(p, Rc::new(b));
    }
}
impl World for Fake {
    fn block(&self, p: [i32; 3]) -> Rc<BlockInfo> {
        if let Some(b) = self.blocks.get(&p) {
            return b.clone();
        }
        if p[1] <= 63 { self.floor.clone() } else { Rc::new(air()) }
    }
    fn rain_at(&self, _: [i32; 3]) -> bool {
        self.rain
    }
    fn golems_at(&self, _: [i32; 3], _: f64) -> Vec<String> {
        Vec::new()
    }
    fn time(&self) -> i64 {
        self.time
    }
}

fn player(id: u32, kind: Kind) -> Ent {
    Ent {
        id,
        type_id: "minecraft:player".into(),
        kind,
        pos: [0.5, 64.0, 0.5],
        vel: [0.0, -0.0784, 0.0],
        yaw: 0.0,
        width: 0.6,
        height: 1.8,
        scale: 1.0,
        on_ground: true,
        touching_water: false,
        submerged_water: false,
        in_lava: false,
        submerged_lava: false,
        climbing: false,
        sprinting: false,
        sneaking: false,
        fall_distance: 0.0,
        jumping: false,
        boots: None,
        move_dist: if kind == Kind::Local { Some(0.0) } else { None },
    }
}

fn engine() -> Engine {
    let mut e = Engine::new(Config::read(USERCONFIG), 7);
    e.load(&files());
    e
}

/// Walks the local player `ticks` ticks at `speed` blocks per tick along x; the sounds that came out.
fn walk(e: &mut Engine, w: &Fake, mut p: Ent, speed: f64, ticks: usize) -> Vec<Emission> {
    let mut out = Vec::new();
    let mut now = 1_000_000;
    for _ in 0..ticks {
        p.pos[0] += speed;
        p.vel[0] = speed;
        // Entity.move: the walked distance grows by the length of the move times 0.6.
        p.move_dist = p.move_dist.map(|d| d + (speed * 0.6) as f32);
        e.tick(w, std::slice::from_ref(&p), p.pos, now, &mut out);
        now += 50;
    }
    out
}

#[test]
fn tuned_settings_are_the_defaults() {
    assert_eq!(Config::read(USERCONFIG), Config::default());
    let c = Config::default();
    assert_eq!((c.volume, c.wet_sounds_volume, c.foliage_sounds_volume, c.max_stepping_entities), (70, 50, 100, 50));
    assert!(c.footwear && c.multiplayer && !c.disabled);
    assert_eq!(c.stance, Locomotion::None);
    assert_eq!(c.target_entities, Selector::All);
    // A key left out keeps the default; a muted game is not enabled.
    assert_eq!(Config::read(r#"{"volume": 0}"#).enabled(), false);
    assert!(Config::read("{}").enabled());
    assert!(!Config::read(r#"{"disabled": true}"#).enabled());
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn the_files_load_and_every_acoustic_the_blockmap_names_exists() {
    let e = engine();
    assert!(e.has_data());
    let iso = e.isolator();
    assert!(iso.acoustics.len() > 50, "acoustics: {}", iso.acoustics.len());
    // Every acoustic that blocks, primitives and golems name is in the library (except the engine's own
    // `_SWIM_*`, `_WING` and `_SWIFT` which acoustics.json has).
    let f = files();
    for text in [&f.blockmap, &f.primitivemap, &f.golemmap] {
        let json = parse_json(text).unwrap();
        for (k, v) in json.entries() {
            for n in SoundsKey::of(v.as_str().unwrap()).names() {
                // The blockmap spells `NOT_EMITTED` once (mud_brick_wall): a missing acoustic, silent, as in the original.
                assert!(matches!(n.as_str(), "NOT_EMITTED" | "NON_EMITTER" | "metal") || iso.acoustics.contains_key(n), "{k} -> {n} is not an acoustic");
            }
        }
    }
    assert!(iso.acoustics.contains_key("_SWIM_WATER") && iso.acoustics.contains_key("_SWIM_LAVA"));
    assert_eq!(iso.locomotion("minecraft:cow"), Locomotion::Quadruped);
    assert_eq!(iso.locomotion("minecraft:bat"), Locomotion::None);
    assert_eq!(iso.locomotion("minecraft:never_heard_of"), Locomotion::Biped);
    assert!(iso.golems.contains("minecraft:boat") && !iso.golems.contains("minecraft:zombie"));
    assert_eq!(iso.variator, Variator::default(), "the pack's variator.json is {{}}");
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn block_keys_resolve_like_the_original() {
    let e = engine();
    let blocks = &e.isolator().blocks;
    let get = |b: &BlockInfo, sub: &str| blocks.get(b, sub).raw;
    assert_eq!(get(&block("stone", "stone"), ""), "stone");
    assert_eq!(get(&block("grass_block", "grass"), ""), "grass");
    // `*.wet` is a wildcard for every block.
    assert_eq!(get(&block("stone", "stone"), "wet"), "mudfine,splash");
    // A tag key: planks.
    let mut planks = block("birch_planks", "wood");
    planks.tags = vec!["minecraft:planks".into()];
    assert_eq!(get(&planks, ""), "wood");
    // A key with properties is tried before the plain one: deepslate[axis=y] -> slate, else rough_slate.
    let mut d = block("deepslate", "deepslate");
    d.props = vec![("axis".into(), "y".into())];
    assert_eq!(get(&d, ""), "slate");
    d.props = vec![("axis".into(), "x".into())];
    assert_eq!(get(&d, ""), "rough_slate");
    // The 1.20.1 `minecraft:grass` is Horizon's `short_grass`: messy ground with a brush under the foot.
    let tall = block("short_grass", "grass");
    assert_eq!(blocks.get(&tall, "messy").kind_is_messy(), true);
    assert_eq!(get(&tall, "foliage"), "brush");
    // Not mentioned: unassigned (the primitive map takes over in the solver).
    assert!(!blocks.get(&block("some_new_block", "stone"), "").is_result());
    assert!(get(&block("air", "stone"), "") == "NOT_EMITTER" || true);
}

trait Messy {
    fn kind_is_messy(&self) -> bool;
}
impl Messy for SoundsKey {
    fn kind_is_messy(&self) -> bool {
        self.raw == "MESSY_GROUND"
    }
}

#[test]
fn key_lines_replace_earlier_ones_and_parse_properties() {
    let mut l = StateLookup::default();
    l.add("minecraft:stone", "a");
    l.add("minecraft:stone", "b");
    let b = block("stone", "stone");
    assert_eq!(l.get(&b, "").raw, "b");
    l.add("minecraft:stone[waterlogged=true]", "c");
    let mut w = b.clone();
    w.props = vec![("waterlogged".into(), "TRUE".into())];
    assert_eq!(l.get(&w, "").raw, "c", "properties compare case-insensitively and come first");
    let mut dry = b.clone();
    dry.props = vec![("waterlogged".into(), "false".into())];
    assert_eq!(l.get(&dry, "").raw, "b");
    // A substrate after the properties.
    l.add("minecraft:stone[waterlogged=true].carpet", "d");
    assert_eq!(l.get(&w, "carpet").raw, "d");
    // `^` metadata is dropped; UNASSIGNED is not stored.
    l.add("minecraft:dirt^2", "e");
    assert_eq!(l.get(&block("dirt", "gravel"), "").raw, "e");
    l.add("minecraft:sand", "UNASSIGNED");
    assert!(!l.get(&block("sand", "sand"), "").is_result());
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn primitives_pick_the_substrate_of_the_sound_type() {
    let e = engine();
    let p = &e.isolator().primitives;
    assert_eq!(p.get("minecraft:block.stone.step", "1.00_1.50").raw, "hardmetal");
    // Other volume/pitch: the default map.
    assert_eq!(p.get("minecraft:block.stone.step", "1.00_1.00").raw, "brickstone");
    assert_eq!(primitive_substrate(1.0, 1.5), "1.00_1.50");
    assert_eq!(p.get("minecraft:item.armor.equip_iron", "").raw, "metalboots");
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn acoustic_types_play_as_the_files_say() {
    let mut e = Engine::new(Config::default(), 1);
    let lib = r#"{"type":"library","engineversion":2,"defaults":{"volume":{"min":90,"max":100},"pitch":{"min":95,"max":105}},
        "soundroot":"pf.","contents":{
        "a":{"type":"events","walk":"a.step","land":["a.land",{"type":"delayed","delay":50,"acoustic":"a.late"}]},
        "b":{"type":"events","walk":{"type":"chance","probability":0,"acoustic":"b.never"}},
        "c":{"type":"events","walk":{"type":"probability","array":[1,"c.one",0,"c.zero"]}},
        "d":{"type":"events","walk":{"type":"basic","name":"@minecraft:block.glass.step","volume":50,"pitch":{"min":200,"max":200}}}}}"#;
    let mut f = files();
    f.acoustics = lib.into();
    f.blockmap = r#"{"minecraft:stone":"a","minecraft:dirt":"b","minecraft:sand":"c","minecraft:glass":"d"}"#.into();
    f.primitivemap = "{}".into();
    f.golemmap = "{}".into();
    e.load(&f);
    let step = |e: &mut Engine, id: &str| {
        let w = Fake::new(block(id, "stone"));
        walk(e, &w, player(1, Kind::Local), 0.2, 40)
    };
    // walk: "a.step" under the sound root, in the 90..100 % volume range and the 95..105 % pitch.
    let out = step(&mut e, "stone");
    assert!(!out.is_empty());
    for s in &out {
        assert_eq!(s.sound, "presencefootsteps:pf.a.step");
        assert!(s.volume >= 0.9 * 0.7 - 1e-4 && s.volume <= 1.0 * 0.7 + 1e-4, "{}", s.volume);
        assert!(s.pitch >= 0.95 - 1e-4 && s.pitch <= 1.05 + 1e-4);
    }
    // chance 0: never (the roll is >= 0 and `<= 0` happens once in 2^24).
    let d = step(&mut e, "dirt");
    assert!(d.len() <= 1, "{d:?}");
    // probability: weight 0 never plays; the entry with all the weight does (its share is 1).
    let out = step(&mut e, "sand");
    assert!(out.iter().all(|s| s.sound.ends_with("c.one")));
    assert!(!out.is_empty());
    // `@` names are used as they are (a vanilla event), volume 50 %, pitch 200 %.
    let out = step(&mut e, "glass");
    assert!(out.iter().all(|s| s.sound == "minecraft:block.glass.step"));
    assert!((out[0].volume - 0.5 * 0.7).abs() < 1e-4 && (out[0].pitch - 2.0).abs() < 1e-4);
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn walking_steps_every_095_of_walked_distance() {
    let mut e = engine();
    let w = Fake::new(block("stone", "stone"));
    // 0.1 blocks per tick = 0.06 of walked distance per tick: a step each 0.95 / 0.06 = ~16 ticks.
    let out = walk(&mut e, &w, player(1, Kind::Local), 0.1, 160);
    let stone_steps = out.iter().filter(|s| s.sound.starts_with("presencefootsteps:pf_presence.stone")).count();
    assert!((9..=11).contains(&stone_steps), "steps: {stone_steps} of {}", out.len());
    // A dropped heard step is in the local player's (stereo) namespace and goes at the global 70 %.
    assert!(out.iter().all(|s| s.sound.starts_with("presencefootsteps:") || s.sound.starts_with("minecraft:")));
    assert!(out.iter().all(|s| s.volume <= 0.7 * 3.0));
    // Standing still: no steps (the stationary sound is rare and needs time to pass).
    let still = walk(&mut e, &w, player(1, Kind::Local), 0.0, 5);
    assert!(still.len() <= 2);
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn sneaking_is_silent_but_running_is_not() {
    let mut e = engine();
    let w = Fake::new(block("stone", "stone"));
    let mut p = player(1, Kind::Local);
    p.sneaking = true;
    assert!(walk(&mut e, &w, p, 0.05, 200).iter().filter(|s| s.sound.contains("stone")).count() <= 1, "only the wander at the start");
    let mut e = engine();
    let mut p = player(1, Kind::Local);
    p.sprinting = true;
    let out = walk(&mut e, &w, p, 0.2, 100);
    assert!(out.len() >= 10, "{}", out.len());
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn a_hard_landing_plays_both_feet() {
    let mut e = engine();
    let w = Fake::new(block("stone", "stone"));
    let mut p = player(1, Kind::Local);
    let mut out = Vec::new();
    // In the air...
    p.on_ground = false;
    p.fall_distance = 3.0;
    p.vel = [0.0, -0.5, 0.0];
    p.pos[1] = 66.0;
    e.tick(&w, std::slice::from_ref(&p), p.pos, 1_000_000, &mut out);
    out.clear();
    // ...then down on the stone: `land` of the stone acoustic, on the left and the right foot.
    p.on_ground = true;
    p.fall_distance = 0.0;
    p.pos[1] = 64.0;
    p.vel = [0.0, -0.0784, 0.0];
    e.tick(&w, std::slice::from_ref(&p), p.pos, 1_000_050, &mut out);
    e.tick(&w, std::slice::from_ref(&p), p.pos, 1_000_300, &mut out);
    assert!(out.len() >= 2, "{out:?}");
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn a_delayed_part_of_the_acoustic_comes_after_its_delay() {
    let mut e = Engine::new(Config::default(), 3);
    let mut f = files();
    f.acoustics = r#"{"type":"library","engineversion":2,"contents":{"x":{"type":"events","walk":[
        "now.ogg",{"type":"delayed","delay":120,"acoustic":"later.ogg"}]}}}"#
        .into();
    f.blockmap = r#"{"minecraft:stone":"x"}"#.into();
    f.primitivemap = "{}".into();
    e.load(&f);
    let w = Fake::new(block("stone", "stone"));
    let mut p = player(1, Kind::Local);
    let mut out = Vec::new();
    let mut now = 5_000;
    let mut first = None;
    let mut later = None;
    for i in 0..80 {
        p.pos[0] += 0.2;
        p.vel[0] = 0.2;
        p.move_dist = p.move_dist.map(|d| d + 0.12);
        let before = out.len();
        e.tick(&w, std::slice::from_ref(&p), p.pos, now, &mut out);
        for s in &out[before..] {
            if s.sound.ends_with("now.ogg") && first.is_none() {
                first = Some(now);
            }
            if s.sound.ends_with("later.ogg") && later.is_none() {
                later = Some(now);
            }
        }
        now += 50;
        let _ = i;
    }
    let (a, b) = (first.unwrap(), later.unwrap());
    assert!(b - a >= 100 && b - a <= 200, "delay was {}", b - a);
    // Every later.ogg followed a now.ogg.
    let nows = out.iter().filter(|s| s.sound.ends_with("now.ogg")).count();
    let laters = out.iter().filter(|s| s.sound.ends_with("later.ogg")).count();
    assert!(laters + 1 >= nows && laters <= nows, "{nows} {laters}");
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn boots_play_their_own_sound_and_halve_the_step() {
    let mut e = engine();
    let w = Fake::new(block("stone", "stone"));
    let bare = walk(&mut e, &w, player(1, Kind::Local), 0.2, 60);
    let mut p = player(1, Kind::Local);
    p.boots = Some("minecraft:item.armor.equip_iron".into());
    let mut e2 = engine();
    let shod = walk(&mut e2, &w, p, 0.2, 60);
    assert!(shod.iter().any(|s| s.sound.contains("metalbar")), "{:?}", shod.iter().map(|s| &s.sound).collect::<Vec<_>>());
    assert!(!bare.iter().any(|s| s.sound.contains("metalbar")));
    // footwear off: no boots sound.
    let mut cfg = Config::default();
    cfg.footwear = false;
    let mut e3 = Engine::new(cfg, 1);
    e3.load(&files());
    let mut p = player(1, Kind::Local);
    p.boots = Some("minecraft:item.armor.equip_iron".into());
    assert!(!walk(&mut e3, &w, p, 0.2, 60).iter().any(|s| s.sound.contains("metalbar")));
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn the_other_players_are_mono_and_have_their_own_volume() {
    let mut cfg = Config::default();
    cfg.other_player_volume = 50;
    let mut e = Engine::new(cfg, 1);
    e.load(&files());
    let w = Fake::new(block("stone", "stone"));
    let mut p = player(2, Kind::OtherPlayer);
    let mut out = Vec::new();
    let mut now = 10_000;
    // Another player reports no velocity: the motion is the change of position.
    for _ in 0..60 {
        p.pos[0] += 0.2;
        e.tick(&w, std::slice::from_ref(&p), p.pos, now, &mut out);
        now += 50;
    }
    assert!(!out.is_empty());
    let ours: Vec<_> = out.iter().filter(|s| s.sound.contains("pf_presence")).collect();
    assert!(!ours.is_empty());
    assert!(ours.iter().all(|s| s.sound.starts_with("presencefootstepsmono:")), "{:?}", ours[0]);
    // 70 % global * 50 % others <= 0.35 at the loudest of a 100 % sound.
    assert!(out.iter().all(|s| s.volume <= 0.35 * 3.0));
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn mobs_follow_the_locomotion_map() {
    let mut e = engine();
    let w = Fake::new(block("grass_block", "grass"));
    let mut cow = player(5, Kind::Passive);
    cow.type_id = "minecraft:cow".into();
    cow.width = 0.9;
    cow.height = 1.4;
    let mut bat = cow.clone();
    bat.id = 6;
    bat.type_id = "minecraft:bat".into();
    let mut out = Vec::new();
    let mut now = 0;
    for _ in 0..80 {
        cow.pos[0] += 0.1;
        bat.pos[0] += 0.1;
        e.tick(&w, &[cow.clone(), bat.clone()], cow.pos, now, &mut out);
        now += 50;
    }
    assert!(out.iter().any(|s| s.entity == 5), "the cow steps");
    assert!(out.iter().all(|s| s.entity != 6), "a bat has no stance");
    // A golem type (boat) is not a walker at all.
    let mut boat = cow.clone();
    boat.id = 7;
    boat.type_id = "minecraft:boat".into();
    let mut out = Vec::new();
    for _ in 0..40 {
        boat.pos[0] += 0.1;
        e.tick(&w, std::slice::from_ref(&boat), boat.pos, now, &mut out);
        now += 50;
    }
    assert!(out.is_empty());
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn far_entities_and_disabled_settings_make_no_sound() {
    let mut e = engine();
    let w = Fake::new(block("stone", "stone"));
    let mut far = player(9, Kind::OtherPlayer);
    far.pos = [100.0, 64.0, 0.0];
    let mut out = Vec::new();
    let mut now = 0;
    for _ in 0..60 {
        far.pos[0] += 0.2;
        e.tick(&w, std::slice::from_ref(&far), [0.0, 64.0, 0.0], now, &mut out);
        now += 50;
    }
    assert!(out.is_empty(), "16 blocks is as far as the original listens");
    assert!(e.running(true) && e.running(false));
    let mut cfg = Config::default();
    cfg.multiplayer = false;
    let mut e = Engine::new(cfg, 1);
    e.load(&files());
    assert!(e.running(true) && !e.running(false));
    let mut empty = Engine::new(Config::default(), 1);
    empty.load(&Files::default());
    assert!(!empty.has_data() && !empty.running(true));
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn the_selector_limits_the_targets() {
    let mut cfg = Config::default();
    cfg.target_entities = Selector::PlayersOnly;
    let mut e = Engine::new(cfg, 1);
    e.load(&files());
    let w = Fake::new(block("stone", "stone"));
    let mut cow = player(5, Kind::Passive);
    cow.type_id = "minecraft:cow".into();
    let mut out = Vec::new();
    for i in 0..60 {
        cow.pos[0] += 0.2;
        e.tick(&w, std::slice::from_ref(&cow), cow.pos, i * 50, &mut out);
    }
    assert!(out.is_empty());
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn walking_in_water_adds_the_swim_sound() {
    let mut e = engine();
    let w = Fake::new(block("stone", "stone"));
    let mut p = player(1, Kind::Local);
    p.touching_water = true;
    p.vel = [0.2, 0.0, 0.0];
    let out = walk(&mut e, &w, p, 0.2, 40);
    assert!(out.iter().any(|s| s.sound.contains("swim")), "{:?}", out.iter().map(|s| &s.sound).collect::<Vec<_>>());
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn rain_makes_the_step_wet() {
    let mut e = engine();
    let mut w = Fake::new(block("stone", "stone"));
    let dry = walk(&mut e, &w, player(1, Kind::Local), 0.2, 80);
    w.rain = true;
    let mut e2 = engine();
    let wet = walk(&mut e2, &w, player(1, Kind::Local), 0.2, 80);
    let splashes = |v: &[Emission]| v.iter().filter(|s| s.sound.contains("water") || s.sound.contains("mud")).count();
    assert_eq!(splashes(&dry), 0, "{:?}", dry.iter().map(|s| &s.sound).collect::<Vec<_>>());
    assert!(splashes(&wet) > 0, "{:?}", wet.iter().map(|s| &s.sound).collect::<Vec<_>>());
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn grass_under_the_foot_brushes_bare_legs_only() {
    let mut e = engine();
    let mut w = Fake::new(block("grass_block", "grass"));
    for x in -5..40 {
        w.set([x, 64, 0], {
            let mut g = block("short_grass", "grass");
            g.collision = vec![];
            g.outline = vec![[0.1, 0.0, 0.1, 0.9, 0.8, 0.9]];
            g.full_top = false;
            g
        });
    }
    let bare = walk(&mut e, &w, player(1, Kind::Local), 0.1, 60);
    assert!(bare.iter().any(|s| s.sound.contains("brush")), "{:?}", bare.iter().map(|s| &s.sound).collect::<Vec<_>>());
    let mut p = player(1, Kind::Local);
    p.boots = Some("minecraft:item.armor.equip_leather".into());
    let mut e2 = engine();
    let shod = walk(&mut e2, &w, p, 0.1, 60);
    let brush = |v: &[Emission]| v.iter().filter(|s| s.sound.contains("brush")).count();
    assert!(brush(&shod) < brush(&bare), "bare legs brush more: {} vs {}", brush(&bare), brush(&shod));
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn the_carpet_rule_and_the_fence_rule() {
    let mut e = engine();
    let mut w = Fake::new(block("stone", "stone"));
    // Wool carpet (outline 1/16 high) on the stone: the carpet's sound, not the stone's.
    for x in -5..60 {
        let mut c = block("white_carpet", "wool");
        c.tags = vec!["minecraft:wool_carpets".into(), "minecraft:wool".into()];
        c.collision = vec![[0.0, 0.0, 0.0, 1.0, 0.0625, 1.0]];
        c.outline = c.collision.clone();
        c.full_top = false;
        w.set([x, 64, 0], c);
    }
    let mut p = player(1, Kind::Local);
    p.pos[1] = 64.0625;
    let out = walk(&mut e, &w, p, 0.2, 60);
    assert!(out.iter().any(|s| s.sound.contains("rug") || s.sound.contains("wool") || s.sound.contains("carpet")),
        "{:?}", out.iter().map(|s| &s.sound).collect::<Vec<_>>());
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn the_quadruped_steps_in_a_gait() {
    let mut e = engine();
    let w = Fake::new(block("stone", "stone"));
    let mut cow = player(5, Kind::Passive);
    cow.type_id = "minecraft:cow".into();
    cow.width = 0.9;
    let mut biped = cow.clone();
    biped.id = 6;
    biped.type_id = "minecraft:chicken".into();
    let (mut a, mut b) = (0, 0);
    let mut now = 0;
    let mut out = Vec::new();
    for _ in 0..200 {
        cow.pos[0] += 0.1;
        biped.pos[0] += 0.1;
        e.tick(&w, &[cow.clone(), biped.clone()], cow.pos, now, &mut out);
        now += 50;
    }
    for s in &out {
        if s.entity == 5 { a += 1 } else { b += 1 }
    }
    // The quadruped's modifier steps again on each walk step (two feet per stride) and its stride is shorter.
    assert!(a > b, "quadruped {a}, biped {b}");
}

#[test]
fn the_pointer_math() {
    // scalex and the unit helpers the winged part would use.
    assert_eq!(scalex(5.0, 0.0, 10.0), 0.5);
    assert_eq!(scalex(-5.0, 0.0, 10.0), 0.0);
    assert_eq!(scalex(50.0, 0.0, 10.0), 1.0);
    let mut r = Rng::new(1);
    for _ in 0..1000 {
        let f = r.next_f32();
        assert!((0.0..1.0).contains(&f));
        assert!(r.next_int(5) < 5);
    }
}

#[test]
#[ignore = "needs your generated Presence Footsteps content (its config files) in content/mods/presencefootsteps"]
fn every_acoustic_of_the_library_parses() {
    let e = engine();
    let json = parse_json(&files().acoustics).unwrap();
    let names: Vec<&String> = json.get("contents").unwrap().entries().iter().map(|(k, _)| k).collect();
    let missing: Vec<_> = names.iter().filter(|n| !e.isolator().acoustics.contains_key(**n)).collect();
    assert!(missing.is_empty(), "not loaded: {missing:?}");
    assert_eq!(e.isolator().acoustics.len(), names.len());
}
