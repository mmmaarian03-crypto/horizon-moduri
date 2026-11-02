use horizon_fallingtree::{loose_leaves, plan, Kind, Plan, Pos, Settings, World};
use std::collections::BTreeMap;

#[derive(Default)]
struct Map(BTreeMap<Pos, Kind>);
impl World for Map {
    fn kind(&self, p: Pos) -> Option<Kind> {
        if p[1] < -64 || p[1] > 319 {
            return None;
        }
        Some(self.0.get(&p).cloned().unwrap_or(Kind::Other))
    }
}
impl Map {
    fn log(&mut self, p: Pos, name: &str) {
        self.0.insert(p, Kind::Log(name.into()));
    }
    fn leaves(&mut self, p: Pos, persistent: bool) {
        self.0.entry(p).or_insert(Kind::Leaves { persistent });
    }
    fn remove(&mut self, ps: &[Pos]) {
        for p in ps {
            self.0.remove(p);
        }
    }
    /// An oak: a trunk of `height` logs at (x, 0, z) and a 5x5 crown of leaves
    /// over its top two logs plus a 3x3 cap.
    fn oak(&mut self, x: i32, z: i32, height: i32) {
        for y in 0..height {
            self.log([x, y, z], "minecraft:oak_log");
        }
        for y in height - 2..height {
            for dx in -2..=2 {
                for dz in -2..=2 {
                    self.leaves([x + dx, y, z + dz], false);
                }
            }
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                self.leaves([x + dx, height, z + dz], false);
            }
        }
    }
}

fn fell(m: &Map, origin: Pos) -> Vec<Pos> {
    match plan(m, &Settings::default(), origin) {
        Plan::Fell { logs } => logs,
        other => panic!("expected a fall, got {other:?}"),
    }
}

#[test]
fn whole_oak_falls_from_its_lowest_log() {
    let mut m = Map::default();
    m.oak(0, 0, 5);
    let logs = fell(&m, [0, 0, 0]);
    assert_eq!(logs.len(), 5);
    assert_eq!(logs.first(), Some(&[0, 4, 0]), "furthest first");
    assert_eq!(logs.last(), Some(&[0, 0, 0]), "the cut last");
}

#[test]
fn cutting_the_middle_takes_the_whole_tree() {
    let mut m = Map::default();
    m.oak(0, 0, 6);
    assert_eq!(fell(&m, [0, 2, 0]).len(), 6);
}

#[test]
fn diagonal_branches_belong_to_the_tree() {
    let mut m = Map::default();
    m.oak(0, 0, 5);
    m.log([1, 3, 1], "minecraft:oak_log");
    m.log([2, 4, 2], "minecraft:oak_log");
    assert_eq!(fell(&m, [0, 0, 0]).len(), 7);
}

#[test]
fn a_log_house_without_leaves_is_not_a_tree() {
    let mut m = Map::default();
    for x in 0..5 {
        for y in 0..3 {
            m.log([x, y, 0], "minecraft:spruce_log");
        }
    }
    assert_eq!(plan(&m, &Settings::default(), [0, 0, 0]), Plan::NotATree);
}

#[test]
fn other_wood_stays() {
    let mut m = Map::default();
    m.oak(0, 0, 5);
    m.log([1, 1, 0], "minecraft:birch_log");
    let logs = fell(&m, [0, 0, 0]);
    assert!(!logs.contains(&[1, 1, 0]));
    assert_eq!(logs.len(), 5);
}

#[test]
fn mixed_logs_when_allowed() {
    let mut m = Map::default();
    m.oak(0, 0, 5);
    m.log([1, 1, 0], "minecraft:birch_log");
    let s = Settings {
        allow_mixed_logs: true,
        ..Settings::default()
    };
    let Plan::Fell { logs } = plan(&m, &s, [0, 0, 0]) else {
        panic!()
    };
    assert_eq!(logs.len(), 6);
}

#[test]
fn mangrove_roots_go_with_the_mangrove() {
    let mut m = Map::default();
    for y in 0..4 {
        m.log([0, y, 0], "minecraft:mangrove_log");
    }
    m.log([1, -1, 0], "minecraft:mangrove_roots");
    m.leaves([0, 4, 0], false);
    assert_eq!(fell(&m, [0, 0, 0]).len(), 5);
}

#[test]
fn more_than_max_size_logs_aborts() {
    let mut m = Map::default();
    for y in 0..101 {
        m.log([0, y, 0], "minecraft:jungle_log");
    }
    m.leaves([1, 100, 0], false);
    assert_eq!(
        plan(&m, &Settings::default(), [0, 0, 0]),
        Plan::TooBig { found: 101 }
    );
    let s = Settings {
        max_size: 200,
        ..Settings::default()
    };
    assert!(matches!(plan(&m, &s, [0, 0, 0]), Plan::Fell { .. }));
}

#[test]
fn the_scan_stops_past_max_scan_size() {
    let mut m = Map::default();
    for x in 0..30 {
        for y in 0..30 {
            m.log([x, y, 0], "minecraft:oak_log");
        }
    }
    let s = Settings {
        max_size: 10_000,
        ..Settings::default()
    };
    assert_eq!(plan(&m, &s, [0, 0, 0]), Plan::TooBig { found: 501 });
}

#[test]
fn not_a_log() {
    let mut m = Map::default();
    m.leaves([0, 0, 0], false);
    assert_eq!(plan(&m, &Settings::default(), [0, 0, 0]), Plan::NotALog);
}

#[test]
fn the_crown_falls_with_the_tree() {
    let mut m = Map::default();
    m.oak(0, 0, 5);
    let logs = fell(&m, [0, 0, 0]);
    m.remove(&logs);
    let loose = loose_leaves(&m, &Settings::default(), &logs);
    assert_eq!(loose.len(), 25 * 2 - 2 + 9, "every leaf of the crown");
}

#[test]
fn leaves_held_by_another_tree_stay() {
    let mut m = Map::default();
    m.oak(0, 0, 5);
    m.oak(4, 0, 5); // crowns touch at x = 2
    let logs = fell(&m, [0, 0, 0]);
    m.remove(&logs);
    let loose = loose_leaves(&m, &Settings::default(), &logs);
    // Leaves within 6 steps (through leaves) of the second trunk (x = 4) stay,
    // the far side of the first crown goes, as vanilla decay would take it.
    assert!(!loose.contains(&[2, 4, 0]), "2 steps from the other trunk");
    assert!(!loose.contains(&[1, 4, 1]), "4 steps from the other trunk");
    assert!(!loose.contains(&[-1, 3, -1]), "6 steps from the other trunk");
    assert!(loose.contains(&[-1, 3, 0]), "7 steps: the cut trunk was the way");
    assert!(loose.contains(&[-2, 4, 0]), "8 steps");
    assert_eq!(loose.len(), 18);
}

#[test]
fn persistent_leaves_stay() {
    let mut m = Map::default();
    m.oak(0, 0, 5);
    m.0.insert([2, 4, 2], Kind::Leaves { persistent: true });
    let logs = fell(&m, [0, 0, 0]);
    m.remove(&logs);
    let loose = loose_leaves(&m, &Settings::default(), &logs);
    assert!(!loose.contains(&[2, 4, 2]));
    assert_eq!(loose.len(), 25 * 2 - 2 + 9 - 1);
}

#[test]
fn no_leaf_breaking_when_off() {
    let mut m = Map::default();
    m.oak(0, 0, 5);
    let logs = fell(&m, [0, 0, 0]);
    m.remove(&logs);
    let s = Settings {
        leaves_breaking: false,
        ..Settings::default()
    };
    assert!(loose_leaves(&m, &s, &logs).is_empty());
}

#[test]
fn huge_fungus_warts_fall() {
    let mut m = Map::default();
    for y in 0..5 {
        m.log([0, y, 0], "minecraft:crimson_stem");
    }
    for dx in -1..=1 {
        for dz in -1..=1 {
            m.0.insert([dx, 5, dz], Kind::Wart);
        }
    }
    let logs = fell(&m, [0, 0, 0]);
    m.remove(&logs);
    assert_eq!(loose_leaves(&m, &Settings::default(), &logs).len(), 9);
}

#[test]
fn far_leaves_are_left_to_vanilla_decay() {
    let mut m = Map::default();
    m.log([0, 0, 0], "minecraft:oak_log");
    for x in 1..=30 {
        m.leaves([x, 0, 0], false);
    }
    let logs = fell(&m, [0, 0, 0]);
    m.remove(&logs);
    let loose = loose_leaves(&m, &Settings::default(), &logs);
    assert_eq!(loose.len(), 15);
    assert_eq!(loose.last(), Some(&[15, 0, 0]));
}
