use horizon_moreoverlays::*;

/// A flat world: full blocks up to y = 63, air above; `lit` is a block light source position.
struct Flat {
    lit: Option<[i32; 3]>,
    cave: bool,
}
impl SpawnWorld for Flat {
    fn block_light(&self, x: i32, y: i32, z: i32) -> i32 {
        match self.lit {
            Some(p) if p == [x, y, z] => 14,
            _ => 0,
        }
    }
    fn sky_light(&self, _x: i32, y: i32, _z: i32) -> i32 {
        if self.cave { 0 } else if y >= 64 { 15 } else { 0 }
    }
    fn is_air(&self, _x: i32, y: i32, _z: i32) -> bool {
        y >= 64
    }
    fn full_collision(&self, _x: i32, y: i32, _z: i32) -> bool {
        y <= 63
    }
    fn liquid(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }
    fn collides(&self, b: Bounds) -> bool {
        b.min[1] < 64.0
    }
    fn monster_types(&self) -> usize {
        2
    }
    fn valid_spawn(&self, _x: i32, y: i32, _z: i32, _m: Option<usize>) -> bool {
        y == 63
    }
}

#[test]
fn tuned_config_is_the_default_values() {
    let c = Config::read(|_| None);
    assert_eq!((c.light_up_range, c.light_down_range, c.light_h_range), (4, 16, 16));
    assert_eq!(c.light_save_level, 1);
    assert_eq!(c.chunk_edge_color, 0xFF0000);
    assert_eq!(c.spawn_night_color, 0xFFFF00);
    assert!(c.search_enabled);
    // the pack's file (config/moreoverlays.toml) says the same numbers.
    let c = Config::read(|k| match k {
        "uprange" => Some("4".into()),
        "chunk_line_width" => Some("1.5".into()),
        "radius" => Some("1".into()),
        "search_max_results" => Some("16384".into()),
        "hrange" => Some("99999999999".into()),
        _ => None,
    });
    assert_eq!(c.chunk_line_width, 1.5);
    assert_eq!(c.search_max_results, 16384);
    // out of range values fall back to the default
    assert_eq!(c.light_h_range, 16);
}

#[test]
fn spawn_modes_on_flat_ground() {
    let cfg = Config::default();
    let day = Flat { lit: None, cave: false };
    // Open sky: only at night (mode 1).
    assert_eq!(spawn_mode_at(&cfg, &day, 0, 64, 0), 1);
    // In a cave: always (mode 2).
    let cave = Flat { lit: None, cave: true };
    assert_eq!(spawn_mode_at(&cfg, &cave, 0, 64, 0), 2);
    // A torch's light level at or above the save level (1) hides the cross.
    let torch = Flat { lit: Some([0, 64, 0]), cave: true };
    assert_eq!(spawn_mode_at(&cfg, &torch, 0, 64, 0), 0);
    assert_eq!(spawn_mode_at(&cfg, &torch, 1, 64, 0), 2);
    // Inside the ground and in the air nothing marks.
    assert_eq!(spawn_mode_at(&cfg, &cave, 0, 60, 0), 0);
    assert_eq!(spawn_mode_at(&cfg, &cave, 0, 70, 0), 0);
}

#[test]
fn scan_covers_the_ranges() {
    let cfg = Config::default();
    let world = Flat { lit: None, cave: true };
    let player = ScanPlayer { x: 0.5, y: 64.0, z: 0.5, yaw: 90.0 };
    let spawns = scan(&cfg, &world, player);
    // Only y = 64 is a standing place: (2*16+1)^2 columns.
    assert_eq!(spawns.len(), 33 * 33);
    assert!(spawns.iter().all(|s| s.pos[1] == 64 && s.mode == 2));
    assert_eq!(spawns[0].pos, [-16, 64, -16]);
}

#[test]
fn behind_the_player_is_shortened_only_at_yaw_zero() {
    let cfg = Config::default();
    let world = Flat { lit: None, cave: true };
    let south = scan(&cfg, &world, ScanPlayer { x: 0.5, y: 64.0, z: 0.5, yaw: 0.0 });
    // yaw 0 faces south, so the north side is cut to 5 blocks.
    assert_eq!(south.len(), 33 * (16 + 5 + 1));
}

#[test]
fn cross_lines_are_lifted_and_coloured() {
    let cfg = Config::default();
    let spawns = [Spawn { pos: [3, 64, 4], mode: 2 }, Spawn { pos: [5, 64, 4], mode: 1 }];
    let lines = light_lines(&cfg, &spawns, 70.0, |_, _, _| false);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].rgb, 0xFF0000);
    assert_eq!(lines[2].rgb, 0xFFFF00);
    assert_eq!(lines[0].width, 1.5);
    // Below the player the cross is lifted above the block's top by 0.005 + 0.01 * (6 + 1)... (the original's formula).
    assert!(lines[0].from[1] > 64.005);
    // Above the player: just 0.005.
    let above = light_lines(&cfg, &spawns, 10.0, |_, _, _| false);
    assert!((above[0].from[1] - 64.005).abs() < 1e-4);
    // On snow the cross sits an eighth higher.
    let snow = light_lines(&cfg, &spawns, 10.0, |_, _, _| true);
    assert!((snow[0].from[1] - 64.13).abs() < 1e-3);
}

#[test]
fn light_switch_messages() {
    let mut l = Light::default();
    assert_eq!(l.set_enabled(true), Some(LIGHT_ON));
    assert_eq!(l.set_enabled(true), None);
    l.cache.push(Spawn { pos: [0, 0, 0], mode: 1 });
    assert_eq!(l.set_enabled(false), Some(LIGHT_OFF));
    assert!(l.cache.is_empty());
    l.set_enabled(true);
    l.world_gone();
    assert!(!l.enabled);
}

#[test]
fn chunk_mode_cycles_and_draws() {
    let cfg = Config::default();
    let mut m = ChunkMode::None;
    let names: Vec<_> = (0..4).map(|_| { m = m.next(); m.name() }).collect();
    assert_eq!(names, ["CORNERS", "GRID", "REGIONS", "NONE"]);
    assert!(chunk_lines(&cfg, ChunkMode::None, ChunkPlayer { x: 8.0, y: 70.0, z: 8.0 }, 384).is_empty());
    let corners = chunk_lines(&cfg, ChunkMode::Corners, ChunkPlayer { x: 8.0, y: 70.0, z: 8.0 }, 384);
    let grid = chunk_lines(&cfg, ChunkMode::Grid, ChunkPlayer { x: 8.0, y: 70.0, z: 8.0 }, 384);
    assert!(!corners.is_empty());
    assert!(grid.len() > corners.len());
    assert!(corners.iter().all(|l| l.width == 1.5));
}

#[test]
fn region_text_follows_the_region() {
    let mut r = RegionInfo::default();
    r.tick(ChunkMode::Regions, 0, 0);
    assert_eq!(r.lines, vec!["region/r.0.0.mca".to_string()]);
    r.tick(ChunkMode::Regions, -1, 33);
    assert_eq!(r.lines, vec!["region/r.-1.1.mca".to_string()]);
    r.tick(ChunkMode::Grid, 0, 0);
    assert!(r.lines.is_empty());
}

#[test]
fn search_dims_what_does_not_match() {
    let cfg = Config::default();
    let mut s = Search::new();
    s.toggle(&cfg, "iron");
    let stack = |name: &str| StackView { empty: false, ingredient: false, tooltip: vec![], display_name: name.into() };
    let dim = dimmed_slots(&cfg, &s, "iron", 3, vec![(0, stack("Iron Ingot")), (1, stack("Dirt"))]);
    assert_eq!(dim, vec![(0, false), (1, true)]);
    // Too many results: the overlay does nothing.
    assert!(dimmed_slots(&cfg, &s, "iron", 20000, vec![(0, stack("Dirt"))]).is_empty());
    assert_eq!(frame_rects([10.0, 10.0, 50.0, 12.0])[0], [9.0, 9.0, 52.0, 1.0]);
}
