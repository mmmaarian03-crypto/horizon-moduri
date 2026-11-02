// The Lost Castle inside Horizon's structure starts (nightly 142388e1c). The
// rules are the engine below; this part reads the generator's terrain. It is
// public, so the stronghold generator and the game's trading code reach it.
//
// The castle is carried as vanilla's jigsaw whose `"horizon:start_checks"` hold
// only `"max_y": 1734426469` (TLC's structure set salt). Horizon reads `max_y`
// only together with `terrain_radius`, so for the game this is no rule at all;
// `castle_checks` knows it as the mark of LostCastle.extraSpawningChecks.
use super::{CheckQueries, StartChecks};
use crate::heightmap::HeightmapType;
use horizon_core::position::ChunkPos;
use serde_json::Value;

pub mod engine {
    include!("tlc_engine.rs");
}

/// The mark: TLC's structure set salt as `max_y`, which Horizon's rules read
/// only together with `terrain_radius`.
pub const MARK: i32 = 1_734_426_469;

/// The generator's noise columns, as Structure.GenerationContext reads them.
struct Noise<'a>(&'a mut dyn CheckQueries);
impl engine::Terrain for Noise<'_> {
    fn first_occupied(&mut self, x: i32, z: i32) -> i32 {
        // ChunkGenerator.getFirstOccupiedHeight: getBaseHeight - 1.
        self.0
            .first_free_height(x, z, HeightmapType::MotionBlockingNoLeaves)
            - 1
    }
}

/// The castle's start rule for checks decoded with the mark, else none.
pub fn castle_checks(
    checks: &StartChecks,
    queries: &mut dyn CheckQueries,
    chunk: ChunkPos,
) -> Option<bool> {
    (checks.max_y == Some(MARK) && checks.terrain_radius.is_none())
        .then(|| engine::castle_may_start(&mut Noise(queries), chunk.x, chunk.z))
}

/// DisableVanillaStrongholdsMixin with the pack's `generate_stronghold = false`:
/// whether `set` holds vanilla strongholds only (minecraft:strongholds), so
/// that none of its chunks starts one. Sets of other types (Better
/// Strongholds) are not touched.
pub fn strongholds_refused(
    registry: &super::super::start::StructureRegistry,
    set: &super::super::start::StructureSet,
) -> bool {
    use super::super::start::StructureKind;
    !engine::GENERATE_STRONGHOLD
        && !set.structures.is_empty()
        && set.structures.iter().all(|(s, _)| {
            registry
                .structures
                .get(*s)
                .is_some_and(|d| matches!(d.kind, StructureKind::Stronghold))
        })
}

/// A trade cost marked `"tlc:map_price": true`: the castle map's price, drawn
/// as LCMapTrade draws it (ThreadLocalRandom, not the villager's random).
pub fn map_price(wants: &Value) -> Option<i32> {
    use std::hash::{BuildHasher, Hasher};
    static DRAWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if wants.get("tlc:map_price").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(DRAWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    Some(engine::map_price(hasher.finish()))
}

/// The mod's villager trades (`data/tlc/villager_trade/*.json` under the
/// resource root) by id, for the game's trade registry: LCMap's
/// VillagerTradesEvent hook as data. The game reads `data/minecraft/` only.
pub fn villager_trades(root: &std::path::Path) -> Vec<(String, Value)> {
    let dir = root.join("data/tlc/villager_trade");
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    let mut files: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    files.sort();
    for path in files {
        let (Some(stem), true) = (
            path.file_stem().and_then(|s| s.to_str()),
            path.extension().is_some_and(|x| x == "json"),
        ) else {
            continue;
        };
        if let Ok(value) = std::fs::read(&path)
            .map_err(|_| ())
            .and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|_| ()))
        {
            out.push((format!("tlc:{stem}"), value));
        }
    }
    out
}
