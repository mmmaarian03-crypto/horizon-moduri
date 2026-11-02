// Oreberries Replanted's bushes in the generation (horizon_world::worldgen::generator): the
// twelve placed features of the original, which add themselves to every overworld biome in the
// `underground_ores` step, run after the chunk's own decoration, on the 3x3 chunks it works on.
use super::{BlockStateId, ProtoChunk};

#[allow(dead_code)]
mod engine {
    include!("oreberriesreplanted_engine.rs");
}

struct Region<'a> {
    chunks: &'a mut [ProtoChunk; 9],
    center: (i32, i32),
    /// The bush of each ore at its last age.
    states: [BlockStateId; 12],
}
impl Region<'_> {
    fn index(&self, p: engine::Pos) -> Option<usize> {
        let (dx, dz) = ((p[0] >> 4) - self.center.0, (p[2] >> 4) - self.center.1);
        if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dz) {
            return None;
        }
        let i = ((dz + 1) * 3 + (dx + 1)) as usize;
        let terrain = &self.chunks[i].terrain;
        (p[1] >= terrain.min_y() && p[1] < terrain.min_y() + terrain.height()).then_some(i)
    }
}
impl engine::GenWorld for Region<'_> {
    fn cell(&self, p: engine::Pos) -> engine::Cell {
        let Some(i) = self.index(p) else {
            return engine::Cell::Air;
        };
        match self.chunks[i].block_name(p[0], p[1], p[2]) {
            "minecraft:air" | "minecraft:void_air" => engine::Cell::Air,
            "minecraft:cave_air" => engine::Cell::CaveAir,
            n if engine::is_base_stone(n) => engine::Cell::BaseStone,
            _ => engine::Cell::Other,
        }
    }
    fn set_bush(&mut self, p: engine::Pos, ore: engine::Ore) {
        if let Some(i) = self.index(p) {
            let _ = self.chunks[i].terrain.set_block_raw(p[0], p[1], p[2], self.states[ore.index()]);
        }
    }
}

/// The bushes of the chunk `region[4]`; nothing outside the overworld (its build range
/// starts at -64) or when the bushes are not in the game.
pub(super) fn decorate_bushes(region: &mut [ProtoChunk; 9]) {
    if region[4].terrain.min_y() != -64 {
        return;
    }
    let registry = region[4].data().registry();
    let mut states = [BlockStateId(0); 12];
    for ore in engine::ORES {
        let Some(id) = registry.default_state(&ore.bush()) else {
            return;
        };
        let Some(mut grown) = registry.state(id).cloned() else {
            return;
        };
        grown.properties.insert("age".to_owned(), "3".to_owned());
        let Ok(id) = registry.resolve(&grown) else {
            return;
        };
        states[ore.index()] = id;
    }
    let at = region[4].position();
    let mut world = Region {
        chunks: region,
        center: (at.x, at.z),
        states,
    };
    engine::decorate_chunk(&mut world, &engine::Settings::default(), at.x, at.z);
}
