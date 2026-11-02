// The Lost Castle for Horizon: the rules its Java code adds to the castle's
// data, without any game code. Written from the mod's source (Team Remastered,
// The-Lost-Castle-Forge, branch 1.20.X, LGPL-3.0: structures/LostCastle.java,
// mixin/DisableVanillaStrongholdsMixin.java, utils/LCMap.java) and the pack's
// config/tlc.toml.

/// The castle's structure (`data/tlc/worldgen/structure/lost_castle.json`).
pub const STRUCTURE: &str = "tlc:lost_castle";

/// `config/tlc.toml`, `generate_stronghold = false` in the pack:
/// DisableVanillaStrongholdsMixin cancels every vanilla stronghold start
/// (`StructureType.STRONGHOLD`; Better Strongholds' own type is not touched).
pub const GENERATE_STRONGHOLD: bool = false;

/// LostCastle.extraSpawningChecks: the terrain is sampled this far from the
/// start chunk's corner, east, west, south and north.
pub const FLATNESS_REACH: i32 = 78;
/// ...and each of those four heights must differ from the corner's by less than this.
pub const MAX_HEIGHT_DIFFERENCE: i32 = 10;
/// LostCastle.distanceFromSpawn must be over this many blocks.
pub const MIN_DISTANCE_FROM_SPAWN: i32 = 5000;

/// LCMapTrade: `ThreadLocalRandom.nextInt(15, 25 + 1)` emeralds and a compass.
pub const MAP_PRICE_MIN: i32 = 15;
pub const MAP_PRICE_MAX: i32 = 25;
/// `stack.setHoverName(Component.nullToEmpty("Lost Castle Map"))`.
pub const MAP_NAME: &str = "Lost Castle Map";
/// The castle map's search: `LCTags.LOST_CASTLE_MAP` (`#tlc:lost_castle_map`, without `#`).
pub const MAP_DESTINATION: &str = "tlc:lost_castle_map";

/// The terrain the start checks read.
pub trait Terrain {
    /// ChunkGenerator.getFirstOccupiedHeight(x, z, MOTION_BLOCKING_NO_LEAVES).
    fn first_occupied(&mut self, x: i32, z: i32) -> i32;
}

/// LostCastle.distanceFromSpawn: the chunk's corner block from (0, 0), in
/// doubles, truncated to an int as Java's `(int)` cast does (saturating).
pub fn distance_from_spawn(chunk_x: i32, chunk_z: i32) -> i32 {
    let x = f64::from(chunk_x.wrapping_shl(4));
    let z = f64::from(chunk_z.wrapping_shl(4));
    (x.powi(2) + z.powi(2)).sqrt() as i32
}

/// LostCastle.extraSpawningChecks: the land 78 blocks around the start
/// chunk's corner lies within 10 blocks of the corner's height, and the
/// castle is more than 5000 blocks from the world's centre.
pub fn castle_may_start(terrain: &mut impl Terrain, chunk_x: i32, chunk_z: i32) -> bool {
    let (x, z) = (chunk_x.wrapping_shl(4), chunk_z.wrapping_shl(4));
    let start = terrain.first_occupied(x, z);
    // All five heights are read before any is compared, as the mod does.
    let around = [
        terrain.first_occupied(x + FLATNESS_REACH, z),
        terrain.first_occupied(x - FLATNESS_REACH, z),
        terrain.first_occupied(x, z + FLATNESS_REACH),
        terrain.first_occupied(x, z - FLATNESS_REACH),
    ];
    around
        .iter()
        .all(|h| (start - h).abs() < MAX_HEIGHT_DIFFERENCE)
        && distance_from_spawn(chunk_x, chunk_z) > MIN_DISTANCE_FROM_SPAWN
}

/// What the mod's code says about a structure start the game is about to
/// try: `true` refuses it (no start in this chunk).
pub fn refuses_start(
    terrain: &mut impl Terrain,
    structure: &str,
    vanilla_stronghold: bool,
    chunk_x: i32,
    chunk_z: i32,
) -> bool {
    if vanilla_stronghold && !GENERATE_STRONGHOLD {
        return true;
    }
    structure == STRUCTURE && !castle_may_start(terrain, chunk_x, chunk_z)
}

/// The castle map's price in emeralds for a uniform `roll` in `0..u64::MAX`:
/// 15 to 25, each as likely (the mod draws it from ThreadLocalRandom, outside
/// the villager's own random).
pub fn map_price(roll: u64) -> i32 {
    let span = (MAP_PRICE_MAX - MAP_PRICE_MIN + 1) as u64;
    MAP_PRICE_MIN + (roll % span) as i32
}
