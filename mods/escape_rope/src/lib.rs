// Escape Rope for Horizon: the rules, without any game code.
// Written from the bytecode of `escape_rope-1.0.0.jar` (an MCreator mod, Academic Free License v3.0):
// `EscapeRopeUseProcedure`, `LastSkyCoordProcedure`, `EscapeRopeModVariables`, `EscapeRopeItem`. The mod
// has no config file. This file is pasted into the game as `crate::game::escape_rope::engine` and is also
// the `horizon_escape_rope` crate the tests use: plain Rust, no game types, no inner attributes.

pub type Pos = [i32; 3];

/// Niciun fișier de config pentru acest mod în profilul pachetului.
pub mod config {}

pub const ITEM: &str = "escape_rope:escape_rope";
/// `EscapeRopeUseProcedure`: the lines over the hotbar.
pub const USED: &str = "You used the escape rope!";
pub const BLOCKED: &str = "You cannot use that here!";
/// `EscapeRopeItem.appendHoverText`.
pub const TOOLTIP: &str = "This handy rope will take you to the last place you could see the sky.";
/// `LevelReader.getSeaLevel()` (the default method; no dimension of the pack overrides it).
pub const SEA_LEVEL: i32 = 63;

/// The world as the sky tests read it.
pub trait Sky {
    /// `getHeight(Heightmap.Types.WORLD_SURFACE, x, z)`: one above the highest block that is not air.
    fn surface(&self, x: i32, z: i32) -> i32;
    /// `state.getLightBlock(level, pos) > 0 && !state.liquid()` for the block at `p`.
    fn blocks_light_not_liquid(&self, p: Pos) -> bool;
}

/// `LevelReader.canSeeSky(pos)`.
pub fn can_see_sky(world: &impl Sky, p: Pos) -> bool {
    p[1] >= world.surface(p[0], p[2])
}

/// `LevelReader.canSeeSkyFromBelowWater(pos)` (the `m_46861_` both procedures call): at or above the sea
/// level, `canSeeSky`; below it, the sea level's block of the column must see the sky and nothing that
/// stops light (water and lava excepted) may stand between it and `pos`.
pub fn can_see_sky_from_below_water(world: &impl Sky, p: Pos) -> bool {
    if p[1] >= SEA_LEVEL {
        return can_see_sky(world, p);
    }
    if !can_see_sky(world, [p[0], SEA_LEVEL, p[2]]) {
        return false;
    }
    let mut y = SEA_LEVEL - 1;
    while y > p[1] {
        if world.blocks_light_not_liquid([p[0], y, p[2]]) {
            return false;
        }
        y -= 1;
    }
    true
}

/// `BlockPos.containing(x, y, z)`.
pub fn containing(at: [f64; 3]) -> Pos {
    [at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32]
}

/// `PlayerVariables.LastSkyX / LastSkyY / LastSkyZ`: the last place the player could see the sky. The
/// variables start at 0 and are kept through death (`clonePlayer` copies them whatever `isWasDeath`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Last {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Whether the player has been seen under the sky at all (not in the original: it tells a stored
    /// place from the 0, 0, 0 the variables start with).
    pub set: bool,
}
impl Last {
    /// One line per number, as `{:?}` writes a `f64` (exact round trip), then `set`.
    pub fn save(&self) -> String {
        format!("{:?}\n{:?}\n{:?}\n{}\n", self.x, self.y, self.z, self.set)
    }
    /// The inverse; anything unreadable is a player who has not seen the sky yet.
    pub fn load(text: Option<String>) -> Self {
        let text = text.unwrap_or_default();
        let mut lines = text.lines();
        let mut num = || lines.next().and_then(|l| l.trim().parse::<f64>().ok());
        match (num(), num(), num()) {
            (Some(x), Some(y), Some(z)) if x.is_finite() && y.is_finite() && z.is_finite() => {
                Last { x, y, z, set: true }
            }
            _ => Last::default(),
        }
    }
}

/// `LastSkyCoordProcedure` (`PlayerTickEvent`, END): when the player's block sees the sky, remember where
/// he stands (the exact position). `true` when the stored place changed.
pub fn track(world: &impl Sky, feet: [f64; 3], last: &mut Last) -> bool {
    if !can_see_sky_from_below_water(world, containing(feet)) {
        return false;
    }
    let now = Last { x: feet[0], y: feet[1], z: feet[2], set: true };
    let changed = *last != now;
    *last = now;
    changed
}

/// What using the rope does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rope {
    /// "You cannot use that here!": the player sees the sky. Nothing is used up.
    Blocked,
    /// The player has never been seen under the sky (the original would take him to 0, 0, 0, inside the
    /// rock): nothing happens and nothing is used up.
    NowhereToGo,
    /// Taken to the last place; the rope is used up (unless creative); "You used the escape rope!".
    Teleport { to: [f64; 3] },
}

/// `EscapeRopeUseProcedure.execute`.
pub fn use_rope(world: &impl Sky, feet: [f64; 3], last: &Last) -> Rope {
    if can_see_sky_from_below_water(world, containing(feet)) {
        Rope::Blocked
    } else if !last.set {
        Rope::NowhereToGo
    } else {
        Rope::Teleport { to: [last.x, last.y, last.z] }
    }
}
