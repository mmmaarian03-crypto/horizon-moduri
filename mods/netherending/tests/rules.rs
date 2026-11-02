use horizon_netherending::*;
use serde_json::Value;

/// Scripted `level.random`: the doubles and floats it hands out, in order, and how many were drawn.
struct Draws {
    doubles: Vec<f64>,
    floats: Vec<f32>,
    drawn: usize,
}
impl Draws {
    fn new(doubles: &[f64], floats: &[f32]) -> Self {
        Draws { doubles: doubles.to_vec(), floats: floats.to_vec(), drawn: 0 }
    }
}
impl Rng for Draws {
    fn next_double(&mut self) -> f64 {
        self.drawn += 1;
        self.doubles.remove(0)
    }
    fn next_float(&mut self) -> f32 {
        self.drawn += 1;
        self.floats.remove(0)
    }
}

#[derive(Default)]
struct Log {
    endermen: usize,
    pigmen: usize,
    calls: Vec<String>,
}
impl Host for Log {
    fn anger_endermen(&mut self, center: Pos, range: i32) -> usize {
        self.calls.push(format!("endermen {center:?} r{range}"));
        self.endermen
    }
    fn spawn_endermite(&mut self, at: [f64; 3], yaw: f32) {
        self.calls.push(format!("endermite {at:?} yaw{yaw}"));
    }
    fn anger_pigmen(&mut self, center: Pos, range: i32) -> usize {
        self.calls.push(format!("pigmen {center:?} r{range}"));
        self.pigmen
    }
    fn notice(&mut self, message: Message) {
        self.calls.push(format!("notice {message:?}"));
    }
    fn creeper_primed(&mut self, pos: Pos) {
        self.calls.push(format!("primed {pos:?}"));
    }
    fn schedule_explosion(&mut self, pos: Pos, delay_ticks: i64) {
        self.calls.push(format!("schedule {pos:?} +{delay_ticks}"));
    }
}

const SURVIVAL: Breaker = Breaker { creative: false, spectator: false, silk_touch: false, in_nether: false };
const POS: Pos = [10, 40, -3];

#[test]
fn the_config_is_tuned_file() {
    // netherending-common.toml, lines 5..22
    assert_eq!(config::END_ENDERMAN_ANGER_CHANCE, 0.35);
    assert_eq!(config::END_ENDERMAN_RANGE, 16);
    assert_eq!(config::END_ENDERMITE_CHANCE, 0.05);
    assert_eq!(config::NETHER_PIGMEN_RANGE, 24);
    assert_eq!(config::NETHER_EXPLODE_CHANCE, 0.03);
    assert_eq!(config::NETHER_EXPLODE_POWER, 2.0);
    assert_eq!(EXPLOSION_DELAY_TICKS, 25);
}

#[test]
fn creative_and_spectator_never_trigger_anything() {
    for who in [Breaker { creative: true, ..SURVIVAL }, Breaker { spectator: true, ..SURVIVAL }] {
        let (mut host, mut rng) = (Log::default(), Draws::new(&[], &[]));
        on_break(&mut host, &mut rng, who, POS, Ground::EndOre);
        on_break(&mut host, &mut rng, Breaker { in_nether: true, ..who }, POS, Ground::NetherOre);
        assert!(host.calls.is_empty());
        assert_eq!(rng.drawn, 0);
    }
}

#[test]
fn other_blocks_draw_nothing() {
    let (mut host, mut rng) = (Log::default(), Draws::new(&[], &[]));
    on_break(&mut host, &mut rng, SURVIVAL, POS, Ground::Other);
    on_break(&mut host, &mut rng, Breaker { in_nether: true, ..SURVIVAL }, POS, Ground::Other);
    assert!(host.calls.is_empty());
    assert_eq!(rng.drawn, 0);
}

#[test]
fn end_ore_angers_the_endermen_and_tells_the_player() {
    let mut host = Log { endermen: 3, ..Log::default() };
    // anger draw 0.34 < 0.35, endermite draw 0.9 >= 0.05
    let mut rng = Draws::new(&[0.34, 0.9], &[]);
    on_break(&mut host, &mut rng, SURVIVAL, POS, Ground::EndOre);
    assert_eq!(host.calls, ["endermen [10, 40, -3] r16", "notice Endermen"]);
    assert_eq!(rng.drawn, 2);
}

#[test]
fn end_ore_without_endermen_nearby_says_nothing() {
    let mut host = Log::default();
    let mut rng = Draws::new(&[0.0, 0.9], &[]);
    on_break(&mut host, &mut rng, SURVIVAL, POS, Ground::EndOre);
    assert_eq!(host.calls, ["endermen [10, 40, -3] r16"]);
}

#[test]
fn end_ore_anger_roll_is_strict() {
    let mut host = Log { endermen: 1, ..Log::default() };
    let mut rng = Draws::new(&[0.35, 0.9], &[]);
    on_break(&mut host, &mut rng, SURVIVAL, POS, Ground::EndOre);
    assert!(host.calls.is_empty(), "0.35 is not below 0.35");
}

#[test]
fn end_ore_can_release_an_endermite_at_the_block() {
    let mut host = Log::default();
    // no anger (0.99), endermite 0.049 < 0.05, yaw = 0.5 * 360
    let mut rng = Draws::new(&[0.99, 0.049], &[0.5]);
    on_break(&mut host, &mut rng, SURVIVAL, POS, Ground::EndOre);
    assert_eq!(host.calls, ["endermite [10.5, 40.0, -2.5] yaw180"]);
    assert_eq!(rng.drawn, 3);
}

#[test]
fn silk_touch_keeps_the_endermite_in_but_not_the_anger() {
    let mut host = Log { endermen: 2, ..Log::default() };
    // the endermite roll is skipped entirely with Silk Touch (`!silk && ...`): no draw
    let mut rng = Draws::new(&[0.0], &[]);
    on_break(&mut host, &mut rng, Breaker { silk_touch: true, ..SURVIVAL }, POS, Ground::EndOre);
    assert_eq!(host.calls, ["endermen [10, 40, -3] r16", "notice Endermen"]);
    assert_eq!(rng.drawn, 1);
}

#[test]
fn nether_ore_in_the_nether_angers_the_pigmen_and_may_explode() {
    let mut host = Log { pigmen: 2, ..Log::default() };
    let mut rng = Draws::new(&[0.029], &[]);
    let who = Breaker { in_nether: true, ..SURVIVAL };
    on_break(&mut host, &mut rng, who, POS, Ground::NetherOre);
    assert_eq!(
        host.calls,
        [
            "pigmen [10, 40, -3] r24",
            "notice Pigmen",
            "primed [10, 40, -3]",
            "notice Unstable",
            "schedule [10, 40, -3] +25"
        ]
    );
}

#[test]
fn nether_ore_explosion_roll_and_silk_touch() {
    let who = Breaker { in_nether: true, ..SURVIVAL };
    let mut host = Log::default();
    let mut rng = Draws::new(&[0.03], &[]);
    on_break(&mut host, &mut rng, who, POS, Ground::NetherOre);
    assert_eq!(host.calls, ["pigmen [10, 40, -3] r24"], "0.03 is not below 0.03");
    // Silk Touch: the pigmen still hear, and no roll is made
    let mut host = Log::default();
    let mut rng = Draws::new(&[], &[]);
    on_break(&mut host, &mut rng, Breaker { silk_touch: true, ..who }, POS, Ground::NetherOre);
    assert_eq!(host.calls, ["pigmen [10, 40, -3] r24"]);
    assert_eq!(rng.drawn, 0);
}

#[test]
fn nether_ore_outside_the_nether_does_nothing() {
    let (mut host, mut rng) = (Log { pigmen: 5, ..Log::default() }, Draws::new(&[], &[]));
    on_break(&mut host, &mut rng, SURVIVAL, POS, Ground::NetherOre);
    assert!(host.calls.is_empty());
}

#[test]
fn the_search_box_is_the_block_grown_by_the_range() {
    // AABB(pos).inflate(16) around the block (10, 40, -3) spans x -6..27, y 24..57, z -19..14.
    let enderman = |x0: f64| ([x0, 40.0, -3.3], [x0 + 0.6, 42.9, -2.7]);
    let (lo, hi) = enderman(26.9);
    assert!(within(POS, 16, lo, hi));
    let (lo, hi) = enderman(27.0);
    assert!(!within(POS, 16, lo, hi), "its box starts exactly at the edge: AABB.intersects is strict");
    // below: y - 16 = 24, a top at 24.0 does not meet, 24.1 does
    assert!(!within(POS, 16, [9.7, 21.1, -3.3], [10.3, 24.0, -2.7]));
    assert!(within(POS, 16, [9.7, 21.2, -3.3], [10.3, 24.1, -2.7]));
    // the other side: x - 16 = -6
    assert!(within(POS, 16, [-6.5, 40.0, -3.3], [-5.9, 42.9, -2.7]));
    assert!(!within(POS, 16, [-6.6, 40.0, -3.3], [-6.0, 42.9, -2.7]));
}

fn pending(at: i64, x: i32) -> Pending {
    Pending { dim: "minecraft:the_nether".into(), pos: [x, 30, 0], at }
}

#[test]
fn pending_explosions_blow_in_order_once_due() {
    let mut list = vec![pending(125, 1), pending(100, 2), pending(130, 3), pending(120, 4)];
    let mut blown = Vec::new();
    tick(&mut list, 120, |p| {
        blown.push(p.pos[0]);
        true
    });
    assert_eq!(blown, [2, 4], "list order, only those due");
    assert_eq!(list.iter().map(|p| p.pos[0]).collect::<Vec<_>>(), [1, 3]);
    tick(&mut list, 130, |p| {
        blown.push(p.pos[0]);
        true
    });
    assert_eq!(blown, [2, 4, 1, 3]);
    assert!(list.is_empty());
}

#[test]
fn a_pending_explosion_that_cannot_happen_is_dropped() {
    let mut list = vec![pending(10, 1)];
    tick(&mut list, 10, |_| false);
    assert!(list.is_empty());
}

#[test]
fn each_level_blows_only_its_own_explosions_in_its_turn() {
    // A server ticks the Overworld, then the Nether: the Overworld's turn must not eat the Nether's.
    let over = |at: i64, x: i32| Pending { dim: "minecraft:overworld".into(), pos: [x, 64, 0], at };
    let mut list = vec![pending(100, 1), over(100, 2), pending(150, 3)];
    let blown: Vec<i32> = take_due(&mut list, 120, "minecraft:overworld").iter().map(|p| p.pos[0]).collect();
    assert_eq!(blown, [2]);
    assert_eq!(list.iter().map(|p| p.pos[0]).collect::<Vec<_>>(), [1, 3], "the Nether's wait for its turn");
    let blown: Vec<i32> = take_due(&mut list, 120, "minecraft:the_nether").iter().map(|p| p.pos[0]).collect();
    assert_eq!(blown, [1]);
    assert_eq!(list.iter().map(|p| p.pos[0]).collect::<Vec<_>>(), [3]);
}

#[test]
fn an_explosion_whose_level_never_comes_back_is_dropped() {
    let mut list = vec![pending(100, 1)];
    assert!(take_due(&mut list, 100 + MAX_LATE, "minecraft:overworld").is_empty());
    assert_eq!(list.len(), 1, "still waiting");
    assert!(take_due(&mut list, 101 + MAX_LATE, "minecraft:overworld").is_empty());
    assert!(list.is_empty(), "too late: dropped");
}

#[test]
fn stopping_the_server_forgets_them() {
    let mut list = vec![pending(10, 1), pending(99, 2)];
    stopped(&mut list);
    assert!(list.is_empty());
}

// ---- the End ores in the biomes. The five End files of the 26.3 `worldgen/biomes.json` (pin 142388e1c).

const SPAWNS: &str = r##""attributes":{"minecraft:gameplay/natural_mob_spawns":{"argument":{"spawn_costs":{},"spawns_by_category":{"monster":[{"type":"minecraft:enderman","count":4,"weight":10}]}},"modifier":"overlay"}},"carvers":[],"downfall":0.5,"effects":{"water_color":"#3f76e4"},"has_precipitation":false,"temperature":0.5"##;
fn end_biome(features: &str) -> String {
    format!("{{{SPAWNS},\"features\":{features}}}")
}
fn the_end() -> String {
    end_biome(r#"[[],[],[],[],["minecraft:end_spike"],[],[],[],[],[],["minecraft:end_platform"]]"#)
}
fn end_highlands() -> String {
    end_biome(r#"[[],[],[],[],["minecraft:end_gateway_return"],[],[],[],[],["minecraft:chorus_plant"]]"#)
}
fn end_midlands() -> String {
    end_biome("[]")
}
fn small_end_islands() -> String {
    end_biome(r#"[["minecraft:end_island_decorated"]]"#)
}
fn plains_like() -> String {
    r#"{"attributes":{"minecraft:gameplay/natural_mob_spawns":{"argument":{"spawn_costs":{},"spawns_by_category":{"creature":[{"type":"minecraft:sheep","count":4,"weight":12}],"monster":[{"type":"minecraft:enderman","count":1,"weight":1}]}},"modifier":"overlay"}},"carvers":[],"downfall":0.4,"effects":{},"features":[[],[],[],[],[],[],["minecraft:ore_coal_upper"]],"has_precipitation":true,"temperature":0.8}"#.into()
}

fn step(json: &str, i: usize) -> Vec<String> {
    let v: Value = serde_json::from_str(json).unwrap();
    v["features"][i].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect()
}

#[test]
fn the_sixteen_features_are_in_the_original_order() {
    let f = end_features();
    assert_eq!(f.len(), 16);
    assert_eq!(f[0], "netherending:end_coal_ore");
    assert_eq!(f[8], "netherending:end_tin_ore");
    assert_eq!(f[15], "netherending:end_uranium_ore");
}

#[test]
fn the_four_outer_end_biomes_get_the_ores_in_underground_ores() {
    for json in [end_highlands(), end_midlands(), small_end_islands()] {
        let out = with_end_ores(&json).expect("an outer End biome");
        assert_eq!(step(&out, UNDERGROUND_ORES), end_features());
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["features"].as_array().unwrap().len() >= UNDERGROUND_ORES + 1, "padded up to the step 6: {}", v["features"]);
        // the rest of the file is untouched
        let before: Value = serde_json::from_str(&json).unwrap();
        for key in ["attributes", "carvers", "downfall", "effects", "has_precipitation", "temperature"] {
            assert_eq!(v[key], before[key], "{key}");
        }
    }
}

#[test]
fn existing_steps_keep_their_features() {
    let out = with_end_ores(&end_highlands()).unwrap();
    assert_eq!(step(&out, 4), ["minecraft:end_gateway_return"]);
    assert_eq!(step(&out, 9), ["minecraft:chorus_plant"]);
    assert_eq!(step(&out, 6).len(), 16);
    let out = with_end_ores(&small_end_islands()).unwrap();
    assert_eq!(step(&out, 0), ["minecraft:end_island_decorated"]);
}

#[test]
fn the_central_island_and_other_biomes_are_left_alone() {
    assert_eq!(with_end_ores(&the_end()), None);
    assert_eq!(with_end_ores(&plains_like()), None);
    assert_eq!(with_end_ores("not json"), None);
    assert_eq!(with_end_ores("[]"), None);
}

#[test]
fn adding_twice_does_not_repeat() {
    let once = with_end_ores(&end_highlands()).unwrap();
    let twice = with_end_ores(&once).unwrap();
    assert_eq!(step(&twice, 6), end_features());
}

#[test]
fn a_step_that_is_a_tag_is_not_touched() {
    let mut v: Value = serde_json::from_str(&end_midlands()).unwrap();
    v["features"] = serde_json::json!([[], [], [], [], [], [], "#minecraft:something"]);
    assert!(!add_features(&mut v, 6, &end_features()));
    assert_eq!(v["features"][6], "#minecraft:something");
}

// ---- against the real 26.3 data, when the launcher's prepared files are on this machine

/// `worldgen/biomes.json` of the launcher's prepared resources (name -> the biome file's text).
fn launcher_biomes() -> Option<serde_json::Map<String, Value>> {
    let versions = std::path::Path::new("../../content/horizon-launcher/versions");
    for entry in std::fs::read_dir(versions).ok()?.flatten() {
        let file = entry.path().join(".local/horizon-client/26.3/worldgen/biomes.json");
        if let Ok(text) = std::fs::read_to_string(&file) {
            return serde_json::from_str::<Value>(&text).ok()?.as_object().cloned();
        }
    }
    None
}

#[test]
fn in_the_real_data_exactly_the_four_outer_end_biomes_get_the_ores() {
    let Some(biomes) = launcher_biomes() else {
        eprintln!("skipped: no prepared 26.3 resources on this machine");
        return;
    };
    assert!(biomes.len() > 60);
    let mut hit: Vec<&str> = biomes
        .iter()
        .filter(|(_, text)| text.as_str().is_some_and(|t| with_end_ores(t).is_some()))
        .map(|(name, _)| name.as_str())
        .collect();
    hit.sort_unstable();
    assert_eq!(
        hit,
        ["minecraft:end_barrens", "minecraft:end_highlands", "minecraft:end_midlands", "minecraft:small_end_islands"]
    );
}
