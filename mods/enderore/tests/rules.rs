use horizon_enderore::*;
use serde_json::Value;

/// A `level.random` that hands out the given `nextInt` answers in order and remembers the bounds asked.
struct Ints {
    answers: Vec<i32>,
    bounds: Vec<i32>,
}
impl Ints {
    fn new(answers: &[i32]) -> Self {
        Ints { answers: answers.to_vec(), bounds: Vec::new() }
    }
}
impl Rng for Ints {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.bounds.push(bound);
        self.answers.remove(0)
    }
}

#[test]
fn the_ore_gives_two_to_five_experience() {
    // UniformInt.of(2, 5).sample = nextInt(4) + 2
    for (draw, xp) in [(0, 2), (1, 3), (2, 4), (3, 5)] {
        let mut rng = Ints::new(&[draw]);
        assert_eq!(experience("enderore:ender_ore", &mut rng), Some(xp));
        assert_eq!(rng.bounds, [4]);
    }
}

#[test]
fn all_three_blocks_drop_experience_and_nothing_else_does() {
    for b in BLOCKS {
        assert_eq!(experience(b, &mut Ints::new(&[0])), Some(2), "{b}");
    }
    let mut rng = Ints::new(&[]);
    assert_eq!(experience("minecraft:diamond_ore", &mut rng), None);
    assert_eq!(experience("enderore:something_else", &mut rng), None);
    assert!(rng.bounds.is_empty(), "no draw for a block that is not ours");
}

// ---- the ores in the biomes. Biome files as in the 26.3 `worldgen/biomes.json` (pin 142388e1c).

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
fn end_barrens() -> String {
    end_biome("[]")
}
fn small_end_islands() -> String {
    end_biome(r#"[["minecraft:end_island_decorated"]]"#)
}
fn plains() -> String {
    r#"{"attributes":{"minecraft:gameplay/natural_mob_spawns":{"argument":{"spawn_costs":{},"spawns_by_category":{"creature":[{"type":"minecraft:sheep","count":4,"weight":12}],"monster":[{"type":"minecraft:enderman","count":1,"weight":1}]}},"modifier":"overlay"}},"carvers":[],"downfall":0.4,"effects":{},"features":[[],[],[],[],["minecraft:lake_lava_surface"],[],["minecraft:ore_dirt","minecraft:ore_coal_upper","minecraft:ore_iron_small"],[],[],["minecraft:flower_plains"]],"has_precipitation":true,"temperature":0.8}"#.into()
}
fn nether_wastes() -> String {
    r#"{"attributes":{},"carvers":[],"downfall":0.0,"effects":{},"features":[[],[],[],[],[],[],["minecraft:ore_gravel_nether","minecraft:ore_blackstone"]],"has_precipitation":false,"temperature":2.0}"#.into()
}

fn step(json: &str, i: usize) -> Vec<String> {
    let v: Value = serde_json::from_str(json).unwrap();
    v["features"][i].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect()
}

#[test]
fn the_overworld_gets_ender_ore_after_its_other_ores() {
    let out = with_ores(&plains()).expect("an overworld biome");
    assert_eq!(step(&out, UNDERGROUND_ORES), ["minecraft:ore_dirt", "minecraft:ore_coal_upper", "minecraft:ore_iron_small", "enderore:ender_ore"]);
    assert_eq!(step(&out, 9), ["minecraft:flower_plains"]);
    assert_eq!(step(&out, 4), ["minecraft:lake_lava_surface"]);
}

#[test]
fn the_four_outer_end_biomes_get_the_end_stone_ore() {
    for json in [end_highlands(), end_barrens(), small_end_islands()] {
        let out = with_ores(&json).expect("an outer End biome");
        assert_eq!(step(&out, UNDERGROUND_ORES), ["enderore:end_stone_ender_ore"]);
        let v: Value = serde_json::from_str(&out).unwrap();
        let before: Value = serde_json::from_str(&json).unwrap();
        for key in ["attributes", "carvers", "downfall", "effects", "has_precipitation", "temperature"] {
            assert_eq!(v[key], before[key], "{key}");
        }
    }
    let out = with_ores(&end_highlands()).unwrap();
    assert_eq!(step(&out, 4), ["minecraft:end_gateway_return"]);
    assert_eq!(step(&out, 9), ["minecraft:chorus_plant"]);
}

#[test]
fn the_central_island_the_nether_and_junk_are_left_alone() {
    assert_eq!(with_ores(&the_end()), None);
    assert_eq!(with_ores(&nether_wastes()), None);
    assert_eq!(with_ores("not json"), None);
    assert_eq!(with_ores("{}"), None);
}

#[test]
fn adding_twice_does_not_repeat() {
    for json in [plains(), end_highlands()] {
        let once = with_ores(&json).unwrap();
        let twice = with_ores(&once).unwrap();
        assert_eq!(step(&once, UNDERGROUND_ORES), step(&twice, UNDERGROUND_ORES));
    }
}

#[test]
fn a_step_that_is_a_tag_is_not_touched() {
    let mut v: Value = serde_json::from_str(&plains()).unwrap();
    v["features"][6] = Value::String("#minecraft:something".into());
    assert!(!add_feature(&mut v, 6, "enderore:ender_ore"));
    assert_eq!(v["features"][6], "#minecraft:something");
}

// ---- against the real 26.3 data, when the launcher's prepared files are on this machine

fn launcher_file(name: &str) -> Option<serde_json::Map<String, Value>> {
    let versions = std::path::Path::new("../../content/horizon-launcher/versions");
    for entry in std::fs::read_dir(versions).ok()?.flatten() {
        let file = entry.path().join(".local/horizon-client/26.3/worldgen").join(name);
        if let Ok(text) = std::fs::read_to_string(&file) {
            return serde_json::from_str::<Value>(&text).ok()?.as_object().cloned();
        }
    }
    None
}

/// The biomes of a tag (`#minecraft:is_overworld`), tags inside tags followed.
fn tag_biomes(tags: &serde_json::Map<String, Value>, tag: &str, out: &mut std::collections::BTreeSet<String>) {
    let doc: Value = match &tags[tag] {
        Value::String(t) => serde_json::from_str(t).unwrap(),
        other => other.clone(),
    };
    for v in doc["values"].as_array().unwrap() {
        let id = v.as_str().or_else(|| v["id"].as_str()).unwrap();
        match id.strip_prefix('#') {
            Some(inner) => tag_biomes(tags, inner, out),
            None => {
                out.insert(id.to_owned());
            }
        }
    }
}

#[test]
fn in_the_real_data_the_overworld_ore_goes_to_exactly_the_biomes_of_the_tag() {
    let (Some(biomes), Some(tags)) = (launcher_file("biomes.json"), launcher_file("biome_tags.json")) else {
        eprintln!("skipped: no prepared 26.3 resources on this machine");
        return;
    };
    let mut overworld = std::collections::BTreeSet::new();
    tag_biomes(&tags, "minecraft:is_overworld", &mut overworld);
    assert!(overworld.len() > 50);
    let (mut over, mut end) = (std::collections::BTreeSet::new(), Vec::new());
    for (name, text) in &biomes {
        let Some(out) = with_ores(text.as_str().unwrap()) else {
            continue;
        };
        if step(&out, UNDERGROUND_ORES).contains(&OVERWORLD_FEATURE.to_owned()) {
            over.insert(name.clone());
        } else {
            end.push(name.as_str());
        }
    }
    assert_eq!(over, overworld);
    end.sort_unstable();
    assert_eq!(
        end,
        ["minecraft:end_barrens", "minecraft:end_highlands", "minecraft:end_midlands", "minecraft:small_end_islands"]
    );
}
