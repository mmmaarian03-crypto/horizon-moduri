// More Overlays Updated for Horizon: the rules, without any game code.
// Written from More Overlays Updated 1.22.7-mc1.20.2 (feldim2425 / RiDGo8, MIT licence; the `master-1.20.2` branch,
// built on 2024-05-27 like the jar in the pack: `LightScannerBase`, `LightScannerVanilla`, `LightOverlayRenderer`,
// `LightOverlayHandler`, `ChunkBoundsHandler`, `ChunkBoundsRenderer`, `GuiRenderer`, `Config`).
// The game gives the rules a world (`SpawnWorld`) and takes back lists of coloured lines and rectangles.

// ---- the config ----

/// `moreoverlays.toml` (the `Config` class). The defaults are the original's, and also every value of the pack's file.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub light_up_range: i32,
    pub light_down_range: i32,
    pub light_h_range: i32,
    pub light_ignore_layer: bool,
    pub light_ignore_spawn_list: bool,
    pub light_simple_check: bool,
    pub light_save_level: i32,
    pub light_finished_migration: bool,
    pub chunk_edge_radius: i32,
    pub chunk_show_middle: bool,
    pub chunk_edge_color: i32,
    pub chunk_grid_color: i32,
    pub chunk_mid_color: i32,
    pub chunk_line_width: f64,
    pub spawn_always_color: i32,
    pub spawn_night_color: i32,
    /// Read, but the original draws every line with `chunk_line_width` (see `light_lines`).
    pub spawn_line_width: f64,
    pub search_enabled: bool,
    pub search_custom: bool,
    pub search_tooltip: bool,
    pub search_max_results: i32,
    pub search_box_color: i32,
    pub search_slot_color: i32,
    pub search_slot_alpha: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            light_up_range: 4,
            light_down_range: 16,
            light_h_range: 16,
            light_ignore_layer: false,
            light_ignore_spawn_list: false,
            light_simple_check: false,
            light_save_level: 1,
            // The default is false (the 1.18 migration), the pack's file says true.
            light_finished_migration: true,
            chunk_edge_radius: 1,
            chunk_show_middle: true,
            chunk_edge_color: 0xFF0000,
            chunk_grid_color: 0x00FF00,
            chunk_mid_color: 0xFFFF00,
            chunk_line_width: 1.5,
            spawn_always_color: 0xFF0000,
            spawn_night_color: 0xFFFF00,
            spawn_line_width: 2.0,
            search_enabled: true,
            search_custom: true,
            search_tooltip: true,
            search_max_results: 16384,
            search_box_color: 0xFFFF00,
            search_slot_color: 0x000000,
            search_slot_alpha: 0.5,
        }
    }
}

impl Config {
    /// The values stored under the toml's names (`uprange`, `chunk_edge_color`...). A value missing or not a number
    /// is the default, and one outside the range `defineInRange` gives it is corrected to the default too (Forge's
    /// config spec does not clamp).
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let d = Self::default();
        let text = |key: &str| get(key).map(|v| v.trim().trim_matches('"').to_owned());
        let flag = |key: &str, default: bool| match text(key).as_deref() {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        };
        let int = |key: &str, default: i32, lo: i32, hi: i32| match text(key).and_then(|v| v.parse::<i64>().ok()) {
            Some(v) if v >= i64::from(lo) && v <= i64::from(hi) => v as i32,
            _ => default,
        };
        let float = |key: &str, default: f64, lo: f64, hi: f64| match text(key).and_then(|v| v.parse::<f64>().ok()) {
            Some(v) if v >= lo && v <= hi => v,
            _ => default,
        };
        let max = i32::MAX;
        let mut c = Self {
            light_up_range: int("uprange", d.light_up_range, 0, max),
            light_down_range: int("downrange", d.light_down_range, 0, max),
            light_h_range: int("hrange", d.light_h_range, 0, max),
            light_ignore_layer: flag("ignoreLayer", d.light_ignore_layer),
            light_ignore_spawn_list: flag("ignoreSpawnList", d.light_ignore_spawn_list),
            light_simple_check: flag("simpleCheck", d.light_simple_check),
            light_save_level: int("saveLevel", d.light_save_level, 0, max),
            light_finished_migration: flag("finishedMigration", d.light_finished_migration),
            chunk_edge_radius: int("radius", d.chunk_edge_radius, 0, max),
            chunk_show_middle: flag("middle", d.chunk_show_middle),
            chunk_edge_color: int("chunk_edge_color", d.chunk_edge_color, 0, 0xFFFFFF),
            chunk_grid_color: int("chunk_grid_color", d.chunk_grid_color, 0, 0xFFFFFF),
            chunk_mid_color: int("chunk_mid_color", d.chunk_mid_color, 0, 0xFFFFFF),
            chunk_line_width: float("chunk_line_width", d.chunk_line_width, 0.0, f64::MAX),
            spawn_always_color: int("spawn_always_color", d.spawn_always_color, 0, 0xFFFFFF),
            spawn_night_color: int("spawn_night_color", d.spawn_night_color, 0, 0xFFFFFF),
            spawn_line_width: float("spawn_line_width", d.spawn_line_width, 0.0, f64::MAX),
            search_enabled: flag("search_enabled", d.search_enabled),
            search_custom: flag("custom_search", d.search_custom),
            search_tooltip: flag("search_tooltip", d.search_tooltip),
            search_max_results: int("search_max_results", d.search_max_results, 256, max),
            search_box_color: int("search_box_color", d.search_box_color, 0, 0xFFFFFF),
            search_slot_color: int("search_slot_color", d.search_slot_color, 0, 0xFFFFFF),
            search_slot_alpha: float("search_slot_alpha", d.search_slot_alpha, 0.0, 1.0),
        };
        // ClientRegistrationHandler.setupClient: "Quick fix for light level in 1.18".
        if !c.light_finished_migration {
            c.light_save_level = 1;
            c.light_finished_migration = true;
        }
        c
    }
}

/// A coloured line in world coordinates, with its width in pixels. `rgb` is 0xRRGGBB, alpha is always 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub rgb: u32,
    pub width: f32,
}

/// The Java `int` colour of the config as the 0xRRGGBB the lines carry (`(color >> 16) & 0xFF`...).
fn rgb(color: i32) -> u32 {
    (color as u32) & 0xFF_FFFF
}

// ---- the light / mob spawn overlay ----

/// A box, for the collision and liquid questions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// What `LightScannerVanilla` asks of the level. Positions are block positions.
pub trait SpawnWorld {
    /// `getBrightness(LightLayer.BLOCK, pos)`.
    fn block_light(&self, x: i32, y: i32, z: i32) -> i32;
    /// `getBrightness(LightLayer.SKY, pos)`: the stored sky light, not lowered by the time of day.
    fn sky_light(&self, x: i32, y: i32, z: i32) -> i32;
    /// `isEmptyBlock(pos)`: air.
    fn is_air(&self, x: i32, y: i32, z: i32) -> bool;
    /// `getBlockState(pos).isCollisionShapeFullBlock(world, pos)`.
    fn full_collision(&self, x: i32, y: i32, z: i32) -> bool;
    /// The block at `pos` holds a fluid (`containsAnyLiquid` of a box inside that one block).
    fn liquid(&self, x: i32, y: i32, z: i32) -> bool;
    /// `getBlockCollisions(null, bounds)` is not empty.
    fn collides(&self, bounds: Bounds) -> bool;
    /// How many entity types of the monster category can be summoned (`typesToCheck`).
    fn monster_types(&self) -> usize;
    /// `block.isValidSpawn(state, world, pos, ON_GROUND, type)` of the block at `pos`, for monster type `monster`
    /// (an index below `monster_types`), or for a zombie when `None` (`simpleCheck`).
    fn valid_spawn(&self, x: i32, y: i32, z: i32, monster: Option<usize>) -> bool;
}

/// `LightScannerVanilla.TEST_BB`: `0.6 / 2` in from the sides of the block, a whole block high.
fn test_box(x: i32, y: i32, z: i32) -> Bounds {
    let inset = 0.6 / 2.0;
    let far = 1.0 - 0.6 / 2.0;
    let (x, y, z) = (f64::from(x), f64::from(y), f64::from(z));
    Bounds { min: [x + inset, y, z + inset], max: [x + far, y + 1.0, z + far] }
}

/// `LightScannerVanilla.checkCollision`: room for a mob to stand at `pos` (two blocks high unless `ignoreLayer`).
fn check_collision(cfg: &Config, w: &impl SpawnWorld, x: i32, y: i32, z: i32) -> bool {
    // A full block here, or above when the second layer counts: nothing fits.
    if w.full_collision(x, y, z) || (!cfg.light_ignore_layer && w.full_collision(x, y + 1, z)) {
        return false;
    }
    // Air has no collision box.
    if w.is_air(x, y, z) && (cfg.light_ignore_layer || w.is_air(x, y + 1, z)) {
        return true;
    }
    let bb = test_box(x, y, z);
    if !w.collides(bb) && !w.liquid(x, y, z) {
        if cfg.light_ignore_layer {
            return true;
        }
        let up = Bounds { min: [bb.min[0], bb.min[1] + 1.0, bb.min[2]], max: [bb.max[0], bb.max[1] + 1.0, bb.max[2]] };
        return !w.collides(up) && !w.liquid(x, y + 1, z);
    }
    false
}

/// `LightScannerVanilla.getSpawnModeAt`: 0 nothing, 1 spawns at night only (sky light still reaches it), 2 spawns
/// always.
pub fn spawn_mode_at(cfg: &Config, w: &impl SpawnWorld, x: i32, y: i32, z: i32) -> u8 {
    if w.block_light(x, y, z) >= cfg.light_save_level {
        return 0;
    }
    let below = y - 1;
    if w.is_air(x, below, z) {
        return 0;
    }
    if w.liquid(x, below, z) {
        return 0;
    }
    // (The Chisels & Bits check is for a mod that is not in the pack.)
    if !check_collision(cfg, w, x, y, z) {
        return 0;
    }
    if !cfg.light_simple_check {
        let any = (0..w.monster_types()).any(|m| w.valid_spawn(x, below, z, Some(m)));
        if !any {
            return 0;
        }
    } else if !w.valid_spawn(x, below, z, None) {
        return 0;
    }
    if w.sky_light(x, y, z) >= cfg.light_save_level {
        return 1;
    }
    2
}

/// A marked position and its mode (1 night, 2 always).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spawn {
    pub pos: [i32; 3],
    pub mode: u8,
}

/// The player as the scanner reads him: where his feet are and his yaw (`getRotationVector().y`, not wrapped).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScanPlayer {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
}

/// `Direction.fromYRot` of a horizontal direction: 0 south, 1 west, 2 north, 3 east.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    South,
    West,
    North,
    East,
}

pub fn facing(yaw: f32) -> Facing {
    // `Mth.floor(yRot / 90.0 + 0.5) & 3` in doubles.
    let n = (f64::from(yaw) / 90.0 + 0.5).floor() as i64;
    match n & 3 {
        0 => Facing::South,
        1 => Facing::West,
        2 => Facing::North,
        _ => Facing::East,
    }
}

/// `LightScannerBase.update`: every position within the ranges of the player, in the original's order (x, then z,
/// then y). `shouldCheck` is always true on the client (`world.isClientSide`).
pub fn scan(cfg: &Config, w: &impl SpawnWorld, player: ScanPlayer) -> Vec<Spawn> {
    let px = player.x.floor() as i32;
    let py = player.y.floor() as i32;
    let pz = player.z.floor() as i32;
    let y1 = py - cfg.light_down_range;
    let y2 = py + cfg.light_up_range;
    let h = cfg.light_h_range;
    let (mut north, mut east, mut south, mut west) = (h, h, h, h);
    // "Show fewer light overlays behind player". The original tests `getRotationVector().y`, which is the yaw (the
    // vector is (xRot, yRot)), so this only applies while the yaw is within half a degree of 0 (facing south).
    if h > 5 && player.yaw > -0.5 && player.yaw < 0.5 {
        match facing(player.yaw) {
            Facing::North => south = 5,
            Facing::East => west = 5,
            Facing::South => north = 5,
            Facing::West => east = 5,
        }
    }
    let mut out = Vec::new();
    for xo in -west..=east {
        for zo in -north..=south {
            for y in y1..=y2 {
                let mode = spawn_mode_at(cfg, w, px + xo, y, pz + zo);
                if mode != 0 {
                    out.push(Spawn { pos: [px + xo, y, pz + zo], mode });
                }
            }
        }
    }
    out
}

/// `LightOverlayRenderer.renderCross`: the X over one marked block, as the two lines of a `DEBUG_LINES` pass. `snow`
/// is whether the block at the position is in `#minecraft:snow`.
pub fn cross_lines(pos: [i32; 3], player_y: f64, snow: bool, color: i32, width: f64) -> [Line; 2] {
    let py = pos[1];
    // Java's mixed float / double arithmetic: `0.005f + pos.getY()` is a float, adding `0.01f * double` makes a double.
    let height = if snow { py as f32 + 0.125f32 } else { py as f32 };
    let base = 0.005f32 + height;
    let y: f32 = if f64::from(py) > player_y {
        // The block is above the player.
        base
    } else {
        // The block is below the player: lifted a little more the further down it is, against z fighting.
        (f64::from(base) + f64::from(0.01f32) * -(f64::from(py) - player_y - 1.0)) as f32
    };
    let (x0, x1) = (f64::from(pos[0]), f64::from(pos[0] + 1));
    let (z0, z1) = (f64::from(pos[2]), f64::from(pos[2] + 1));
    let y = f64::from(y);
    let (rgb, width) = (rgb(color), width as f32);
    [
        Line { from: [x0, y, z0], to: [x1, y, z1], rgb, width },
        Line { from: [x1, y, z0], to: [x0, y, z1], rgb, width },
    ]
}

/// The lines of `LightOverlayRenderer.renderOverlays`: mode 1 in the night colour, mode 2 in the always colour. The
/// original sets the line width from `chunk_line_width` here (not `spawn_line_width`), and so do these.
pub fn light_lines(cfg: &Config, spawns: &[Spawn], player_y: f64, is_snow: impl Fn(i32, i32, i32) -> bool) -> Vec<Line> {
    let mut out = Vec::with_capacity(spawns.len() * 2);
    for s in spawns {
        let color = match s.mode {
            1 => cfg.spawn_night_color,
            2 => cfg.spawn_always_color,
            _ => continue,
        };
        let snow = is_snow(s.pos[0], s.pos[1], s.pos[2]);
        out.extend(cross_lines(s.pos, player_y, snow, color, cfg.chunk_line_width));
    }
    out
}

/// The light overlay's switch (`LightOverlayHandler`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Light {
    pub enabled: bool,
    pub cache: Vec<Spawn>,
}

pub const LIGHT_ON: &str = "\u{a7}eLight Overlay Enabled";
pub const LIGHT_OFF: &str = "\u{a7}eLight Overlay Disabled";

impl Light {
    /// `setEnabled`: the message for the action bar when it changed. Turning it off empties the scanner's list.
    pub fn set_enabled(&mut self, enabled: bool) -> Option<&'static str> {
        if self.enabled == enabled {
            return None;
        }
        self.enabled = enabled;
        if enabled {
            Some(LIGHT_ON)
        } else {
            self.cache.clear();
            Some(LIGHT_OFF)
        }
    }
    /// The world unloaded: off, with no message (there is no player to show one to).
    pub fn world_gone(&mut self) {
        self.enabled = false;
        self.cache.clear();
    }
}

// ---- the chunk bounds overlay ----

/// `ChunkBoundsHandler.RenderMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkMode {
    None,
    Corners,
    Grid,
    Regions,
}

impl ChunkMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Corners => "CORNERS",
            Self::Grid => "GRID",
            Self::Regions => "REGIONS",
        }
    }
    /// `toggleMode`: the next mode, round.
    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Corners,
            Self::Corners => Self::Grid,
            Self::Grid => Self::Regions,
            Self::Regions => Self::None,
        }
    }
    /// The action bar line `toggleMode` shows (in red).
    pub fn message(self) -> String {
        format!("\u{a7}cChunk Border Overlay: {}", self.name())
    }
}

pub const REGION_SIZE_X: i32 = 32;
pub const REGION_SIZE_Z: i32 = 32;
pub const REGION_SIZE_Y_CUBIC: i32 = 32;

/// The player as the chunk renderer reads him.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkPlayer {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A chunk's region index (`ChunkBoundsRenderer`, `ChunkBoundsHandler`): Java's truncating division.
pub fn region_of(chunk: i32, size: i32) -> i32 {
    if chunk < 0 { (chunk + 1) / size - 1 } else { chunk / size }
}

/// `renderGrid`: lattice lines inside the box, `step` apart, in `float` as the original steps them. The loops run
/// over each axis with the four lines of the original's quad of corners.
#[allow(clippy::too_many_arguments)]
fn grid_lines(out: &mut Vec<Line>, x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32, step: f32, rgb: u32, width: f32) {
    let mut line = |a: [f32; 3], b: [f32; 3]| {
        out.push(Line {
            from: [f64::from(a[0]), f64::from(a[1]), f64::from(a[2])],
            to: [f64::from(b[0]), f64::from(b[1]), f64::from(b[2])],
            rgb,
            width,
        });
    };
    let mut x = x0;
    while x <= x1 {
        line([x, y0, z0], [x, y1, z0]);
        line([x, y0, z1], [x, y1, z1]);
        line([x, y0, z0], [x, y0, z1]);
        line([x, y1, z0], [x, y1, z1]);
        x += step;
    }
    let mut y = y0;
    while y <= y1 {
        line([x0, y, z0], [x1, y, z0]);
        line([x0, y, z1], [x1, y, z1]);
        line([x0, y, z0], [x0, y, z1]);
        line([x1, y, z0], [x1, y, z1]);
        y += step;
    }
    let mut z = z0;
    while z <= z1 {
        line([x0, y0, z], [x1, y0, z]);
        line([x0, y1, z], [x1, y1, z]);
        line([x0, y0, z], [x0, y1, z]);
        line([x1, y0, z], [x1, y1, z]);
        z += step;
    }
}

/// Without the lines that have no length (the GL draws nothing for them, and a line shader divides by the length)
/// and the repeats of a line already there (the original's lattice of a flat plane draws its edges twice).
fn tidy(lines: Vec<Line>) -> Vec<Line> {
    let mut seen = std::collections::HashSet::new();
    let key = |l: &Line| {
        let mut k = [0u64; 6];
        for i in 0..3 {
            k[i] = l.from[i].to_bits();
            k[i + 3] = l.to[i].to_bits();
        }
        (k, l.rgb)
    };
    lines
        .into_iter()
        .filter(|l| l.from != l.to)
        .filter(|l| {
            // A line drawn the other way round is the same line.
            let reversed = Line { from: l.to, to: l.from, ..*l };
            !seen.contains(&key(&reversed)) && seen.insert(key(l))
        })
        .collect()
}

/// `ChunkBoundsRenderer.renderOverlays`: the edge columns of the chunks around the player (red, `radius` chunks),
/// the middle of his chunk (yellow), and a lattice of the chunk (GRID) or of the 32 x 32 region (REGIONS) in green.
/// `level_height` is `level.getHeight()` (384 in the overworld, 256 in the Nether and the End).
pub fn chunk_lines(cfg: &Config, mode: ChunkMode, p: ChunkPlayer, level_height: i32) -> Vec<Line> {
    if mode == ChunkMode::None {
        return Vec::new();
    }
    let width = cfg.chunk_line_width as f32;
    let h = level_height;
    // `(int) player.getY()` truncates.
    let h0 = p.y as i32;
    let h1 = h.min(h0 - 16);
    let h2 = h.min(h0 + 16);
    let h3 = h1.min(0);
    let chunk_x = (p.x.floor() as i32) >> 4;
    let chunk_z = (p.z.floor() as i32) >> 4;
    let x0 = chunk_x * 16;
    let x1 = x0 + 16;
    let x2 = x0 + 8;
    let z0 = chunk_z * 16;
    let z1 = z0 + 16;
    let z2 = z0 + 8;
    // `chunkPosition().getWorldPosition().getY()` is always 0: the region's height index is 0.
    let region_x = region_of(chunk_x, REGION_SIZE_X);
    let region_y = 0;
    let region_z = region_of(chunk_z, REGION_SIZE_Z);
    let border_x0 = region_x * REGION_SIZE_X * 16;
    let border_y0 = (region_y * REGION_SIZE_Y_CUBIC * 16) - 64;
    let border_z0 = region_z * REGION_SIZE_Z * 16;
    let border_x1 = border_x0 + (REGION_SIZE_X * 16);
    let border_y1 = border_y0 + (REGION_SIZE_Y_CUBIC * 16) - 128;
    let border_z1 = border_z0 + (REGION_SIZE_Z * 16);

    let radius = cfg.chunk_edge_radius * 16;
    let mut out = Vec::new();
    let edge = |out: &mut Vec<Line>, x: i32, z: i32, color: i32| {
        out.push(Line {
            from: [f64::from(x), f64::from(h3), f64::from(z)],
            to: [f64::from(x), f64::from(h), f64::from(z)],
            rgb: rgb(color),
            width,
        });
    };
    let mut xo = -16 - radius;
    while xo <= radius {
        let mut yo = -16 - radius;
        while yo <= radius {
            edge(&mut out, x0 - xo, z0 - yo, cfg.chunk_edge_color);
            yo += 16;
        }
        xo += 16;
    }
    if cfg.chunk_show_middle {
        edge(&mut out, x2, z2, cfg.chunk_mid_color);
    }
    let grid = rgb(cfg.chunk_grid_color);
    match mode {
        ChunkMode::Grid => {
            let (fx0, fx1, fh1, fh2) = (x0 as f32, x1 as f32, h1 as f32, h2 as f32);
            let (fz0, fz1) = (z0 as f32, z1 as f32);
            grid_lines(&mut out, fx0, fh1, fz0 - 0.005f32, fx0, fh2, fz1 + 0.005f32, 1.0, grid, width);
            grid_lines(&mut out, fx1, fh1, fz0 - 0.005f32, fx1, fh2, fz1 + 0.005f32, 1.0, grid, width);
            grid_lines(&mut out, fx0 - 0.005f32, fh1, fz0, fx1 + 0.005f32, fh2, fz0, 1.0, grid, width);
            grid_lines(&mut out, fx0 - 0.005f32, fh1, fz1, fx1 + 0.005f32, fh2, fz1, 1.0, grid, width);
        }
        ChunkMode::Regions => {
            grid_lines(
                &mut out,
                border_x0 as f32 - 0.005f32,
                border_y0 as f32 - 0.005f32,
                border_z0 as f32 - 0.005f32,
                border_x1 as f32 + 0.005f32,
                border_y1 as f32 + 0.005f32,
                border_z1 as f32 + 0.005f32,
                16.0,
                grid,
                width,
            );
        }
        _ => {}
    }
    tidy(out)
}

/// The region file text of `ChunkBoundsHandler.regionInfo`, kept as the original keeps it: it changes on a client
/// tick, only in REGIONS mode, and is empty before the first tick in that mode.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionInfo {
    pub lines: Vec<String>,
    prev_x: i32,
    prev_z: i32,
}

impl Default for RegionInfo {
    fn default() -> Self {
        Self { lines: Vec::new(), prev_x: i32::MIN, prev_z: i32::MIN }
    }
}

impl RegionInfo {
    /// `ChunkBoundsHandler.onClientTick`, for a player in the chunk `(chunk_x, chunk_z)`.
    pub fn tick(&mut self, mode: ChunkMode, chunk_x: i32, chunk_z: i32) {
        if mode != ChunkMode::Regions {
            self.lines.clear();
            self.prev_x = 0;
            self.prev_z = 0;
            return;
        }
        let mut update = self.lines.is_empty();
        let new_x = region_of(chunk_x, REGION_SIZE_X);
        if self.prev_x != new_x {
            self.prev_x = new_x;
            update = true;
        }
        let new_z = region_of(chunk_z, REGION_SIZE_Z);
        if self.prev_z != new_z {
            self.prev_z = new_z;
            update = true;
        }
        if update {
            self.lines.clear();
            self.lines.push(format!("region/r.{}.{}.mca", self.prev_x, self.prev_z));
        }
    }
}

// ---- the item search overlay ----

/// `GuiRenderer.FRAME_RADIUS`.
pub const FRAME_RADIUS: f32 = 1.0;

/// What the overlay keeps (`GuiRenderer` and `GuiHandler`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Search {
    pub enabled: bool,
    last_filter: String,
    empty_filter: bool,
    /// `GuiHandler.firstClick`, the time of the first click of a double click in milliseconds.
    first_click: Option<u64>,
}

impl Search {
    pub fn new() -> Self {
        Self { enabled: false, last_filter: String::new(), empty_filter: true, first_click: None }
    }
    pub fn empty_filter(&self) -> bool {
        self.empty_filter
    }
    /// `GuiRenderer.toggleMode`, with the text now in the search box.
    pub fn toggle(&mut self, cfg: &Config, filter_text: &str) {
        if !cfg.search_enabled {
            self.enabled = false;
            return;
        }
        self.enabled = !self.enabled;
        if self.enabled {
            self.last_filter = filter_text.to_owned();
            self.empty_filter = blank(filter_text);
        } else {
            self.last_filter = String::new();
        }
    }
    /// `GuiHandler.onGuiClick` for a left click inside the search box at `now` milliseconds: a second click within a
    /// second of the first toggles the search. True when it toggled.
    pub fn click(&mut self, cfg: &Config, filter_text: &str, now: u64) -> bool {
        if let Some(first) = self.first_click
            && now.saturating_sub(first) < 1000
        {
            self.toggle(cfg, filter_text);
            self.first_click = None;
            return true;
        }
        self.first_click = Some(now);
        false
    }
    /// `GuiRenderer.tick`'s first part: a changed filter text is taken while the search is on.
    pub fn tick_text(&mut self, filter_text: &str) {
        if self.enabled && filter_text != self.last_filter {
            self.last_filter = filter_text.to_owned();
            self.empty_filter = blank(filter_text);
        }
    }
    /// Screens init with the search turned off in the config: `GuiHandler.onGuiInit` calls `toggleMode`, which turns
    /// it off.
    pub fn config_off(&mut self, cfg: &Config) {
        if !cfg.search_enabled {
            self.enabled = false;
        }
    }
}

/// `text.replace(" ", "").isEmpty()`.
fn blank(text: &str) -> bool {
    text.replace(' ', "").is_empty()
}

/// A stack in a slot, as `isSearchedItem` reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackView {
    pub empty: bool,
    /// The item is one of the search's results (`ItemUtils.ingredientMatches` with the filtered ingredients).
    pub ingredient: bool,
    /// `getTooltipLines`, as plain strings.
    pub tooltip: Vec<String>,
    /// `getDisplayName().getString()`: the hover name in square brackets.
    pub display_name: String,
}

/// `GuiRenderer.isSearchedItem`: the slot keeps its colour.
pub fn is_searched(cfg: &Config, search: &Search, filter_text: &str, stack: &StackView) -> bool {
    if search.empty_filter {
        return true;
    }
    if stack.empty {
        return false;
    }
    if stack.ingredient {
        return true;
    }
    let wanted_root = filter_text.to_lowercase();
    if cfg.search_tooltip && stack.tooltip.iter().any(|tip| tip.to_lowercase().contains(&wanted_root)) {
        return true;
    }
    cfg.search_custom && stack.display_name.to_lowercase().contains(&wanted_root)
}

/// `GuiRenderer.checkSlots`: which slots are dimmed (in the order given). `results` is the number of ingredients the
/// search lists; over `search_max_results` the overlay does nothing.
pub fn dimmed_slots(
    cfg: &Config,
    search: &Search,
    filter_text: &str,
    results: usize,
    slots: impl IntoIterator<Item = (usize, StackView)>,
) -> Vec<(usize, bool)> {
    if results > cfg.search_max_results as usize {
        return Vec::new();
    }
    slots
        .into_iter()
        .map(|(id, stack)| (id, !is_searched(cfg, search, filter_text, &stack)))
        .collect()
}

/// `drawSearchFrame`'s four quads around the search box `[x, y, width, height]`, as `[x, y, width, height]`.
pub fn frame_rects(field: [f32; 4]) -> [[f32; 4]; 4] {
    let [x, y, width, height] = field;
    let r = FRAME_RADIUS;
    [
        [x - r, y - r, width + 2.0 * r, r],
        [x - r, y, r, height],
        [x - r, y + height, width + 2.0 * r, r],
        [x + width, y, r, height],
    ]
}

/// The box that `onGuiClick` and `drawSearchFrame` take from a JEI text field `[x, y, width, height]`:
/// `(x - 2, y - 4, width + 8, height - 4)`.
pub fn jei_field(text_field: [f32; 4]) -> [f32; 4] {
    [text_field[0] - 2.0, text_field[1] - 4.0, text_field[2] + 8.0, text_field[3] - 4.0]
}

/// `onGuiClick`'s test: strictly inside the box.
pub fn inside_field(field: [f32; 4], mouse: (f32, f32)) -> bool {
    mouse.0 > field[0] && mouse.0 < field[0] + field[2] && mouse.1 > field[1] && mouse.1 < field[1] + field[3]
}

/// A colour `0xRRGGBB` with an alpha 0..1 as the four bytes the buffer builder makes (`(int) (value * 255)`).
pub fn rgba_bytes(color: i32, alpha: f64) -> [u8; 4] {
    let c = rgb(color);
    let f = |shift: u32| ((((c >> shift) & 0xFF) as f32 / 255.0) * 255.0) as u8;
    [f(16), f(8), f(0), (alpha as f32 * 255.0) as u8]
}
