// AE2 meteorites as a native worldgen step (the carried structure `ae2:meteorite` is a Java
// structure type Horizon cannot run). Module `ae2` in horizon-world's generator.rs; the rules
// (where a meteorite is, what each block becomes) are the engine's (src/lib.rs, pasted below by
// tools/bundle_extra.py). Each chunk places the meteorite whose centre it holds, into its 3x3
// region (MeteoritePlacer clips to the structure piece's box the same way).
use super::{BlockStateId, ProtoChunk};

#[allow(dead_code)]
mod engine {
    include!("ae2_engine.rs");
}

fn state_of(region: &[ProtoChunk; 9], name: &str, props: &[(&str, &str)]) -> Option<BlockStateId> {
    let registry = region[4].data().registry();
    let id = registry.default_state(name)?;
    if props.is_empty() {
        return Some(id);
    }
    let mut s = registry.state(id).cloned()?;
    for (k, v) in props {
        s.properties.insert((*k).to_owned(), (*v).to_owned());
    }
    registry.resolve(&s).ok()
}

struct Area<'a> {
    chunks: &'a mut [ProtoChunk; 9],
    center: (i32, i32),
}
impl Area<'_> {
    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let (dx, dz) = ((x >> 4) - self.center.0, (z >> 4) - self.center.1);
        if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dz) {
            return None;
        }
        let i = ((dz + 1) * 3 + (dx + 1)) as usize;
        let t = &self.chunks[i].terrain;
        (y >= t.min_y() && y < t.min_y() + t.height()).then_some(i)
    }
    fn name(&self, x: i32, y: i32, z: i32) -> Option<&str> {
        let i = self.index(x, y, z)?;
        Some(self.chunks[i].block_name(x, y, z))
    }
    fn set(&mut self, x: i32, y: i32, z: i32, s: BlockStateId) {
        if let Some(i) = self.index(x, y, z) {
            let _ = self.chunks[i].terrain.set_block_raw(x, y, z, s);
        }
    }
    /// WORLD_SURFACE_WG: the first block from the top that is not air.
    fn surface(&self, x: i32, z: i32) -> Option<(i32, bool)> {
        let i = self.index(x, 0, z).or_else(|| self.index(x, 64, z))?;
        let t = &self.chunks[i].terrain;
        let (lo, hi) = (t.min_y(), t.min_y() + t.height() - 1);
        let mut y = hi;
        while y >= lo {
            let n = self.chunks[i].block_name(x, y, z);
            if !matches!(n, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") {
                return Some((y, n == "minecraft:water"));
            }
            y -= 1;
        }
        None
    }
}

/// The meteorite of the chunk `region[4]`, if its centre is there (overworld only).
pub(super) fn place_meteorite(region: &mut [ProtoChunk; 9]) {
    if region[4].terrain.min_y() != -64 {
        return;
    }
    let at = region[4].position();
    let Some(m) = engine::meteor_in_chunk(at.x, at.z) else { return };
    let (Some(sky), Some(air), Some(cube)) = (
        state_of(region, "ae2:sky_stone_block", &[]),
        state_of(region, "minecraft:air", &[]),
        state_of(region, "ae2:mysterious_cube", &[]),
    ) else {
        return;
    };
    let quartz: Vec<BlockStateId> = engine::METEOR_QUARTZ
        .iter()
        .filter_map(|n| state_of(region, n, &[]))
        .collect();
    let buds: Vec<BlockStateId> = engine::BUDS[..3]
        .iter()
        .filter_map(|n| state_of(region, n, &[("facing", "up"), ("waterlogged", "false")]))
        .collect();
    let mut area = Area { chunks: region, center: (at.x, at.z) };
    // MeteoriteStructure.generatePieces: the mean surface height around the centre, lowered on
    // rough ground, minus the radius.
    let scan = ((m.radius * 2.0) as i32).max(1).min(16);
    let mut hs = Vec::new();
    for dx in -scan..=scan {
        for dz in -scan..=scan {
            if let Some((h, _)) = area.surface(m.x + dx, m.z + dz) {
                hs.push(f64::from(h));
            }
        }
    }
    let Some((_, wet)) = area.surface(m.x, m.z) else { return };
    if hs.is_empty() || wet {
        // Ocean meteorites (OCEAN_FLOOR_WG) are left out: a raw crater would leave a hole in the sea.
        return;
    }
    let mean = hs.iter().sum::<f64>() / hs.len() as f64;
    let var = hs.iter().map(|h| (h - mean) * (h - mean)).sum::<f64>() / hs.len() as f64;
    let min = hs.iter().copied().fold(f64::MAX, f64::min);
    let mut cy = mean as i32;
    if var > 5.0 {
        cy -= ((mean - min) * 0.75) as i32;
    }
    let y_off = m.radius.ceil() as i32 + 1;
    cy = (cy - y_off).max(-64 + y_off);
    let mut rng = engine::Rng::new((m.x as i64 as u64) << 32 ^ (m.z as i64 as u64) ^ 0xAE2);
    let reach = ((m.radius as f64 * 2.0 + 5.0) as i32).min(31);
    for dx in -reach..=reach {
        for dz in -reach..=reach {
            for dy in -8..=(319 - cy) {
                let (x, y, z) = (m.x + dx, cy + dy, m.z + dz);
                let b = engine::meteor_block(m.radius, dx, dy, dz);
                match b {
                    engine::MeteorBlock::Keep => {
                        if dy > 8 && dx * dx + dz * dz > reach * reach {
                            break;
                        }
                    }
                    engine::MeteorBlock::SkyStone => area.set(x, y, z, sky),
                    engine::MeteorBlock::Cube => area.set(x, y, z, cube),
                    engine::MeteorBlock::Air => {
                        if area.name(x, y, z).is_some_and(|n| n != "minecraft:air") {
                            area.set(x, y, z, air);
                        }
                    }
                    engine::MeteorBlock::Quartz => {
                        if quartz.is_empty() {
                            continue;
                        }
                        let i = if engine::config::WORLD_GEN_SPAWN_FLAWLESS_ONLY { quartz.len() - 1 } else { rng.next_int(quartz.len() as i32) as usize };
                        area.set(x, y, z, quartz[i]);
                        // A bud on top of a budding block that is not in the middle (70%).
                        if i != 0 && (dx != 0 || dz != 0) && rng.next_float() <= 0.7 && !buds.is_empty() {
                            let b = buds[rng.next_int(buds.len() as i32) as usize];
                            area.set(x, y + 1, z, b);
                        }
                    }
                }
            }
        }
    }
}
