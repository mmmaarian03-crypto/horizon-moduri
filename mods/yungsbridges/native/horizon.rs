// YUNG's Bridges inside Horizon's decoration (nightly 142388e1c). The rules
// are the engine below; this part reads and writes the decoration region, and
// places the bridge templates with the game's template placement.
//
// Horizon reads no Forge biome modifiers and has no feature types of its own
// for mods, so `decorate` is the game's Decoration::decorate (the same body,
// see generator_decoration.rs) with one addition: once the features of step
// surface_structures are done (where Forge's `forge:add_features` puts
// `yungsbridges:bridge_list` for `#yungsbridges:has_structure/bridge`), the
// bridge list runs for the chunk, on the decoration random seeded as feature
// BRIDGE_INDEX of that step.
use super::*;
use crate::worldgen::structure::{
    Mirror, Rotation, StructureResources, StructureTemplate,
    processor::{PlaceSettings, ProcessorRandom, place_in_world},
};
use horizon_core::random::{LegacyRandom, RandomSource, position_seed};
use std::sync::{Mutex, Weak};

// The engine's test helpers (State::with) are not called by the game.
#[allow(dead_code)]
mod engine {
    include!("yungsbridges_engine.rs");
}

/// `forge/biome_modifier/bridge_addition.json`: the feature, the biomes, the step.
const FEATURE: &str = "yungsbridges:bridge_list";
const BIOMES: &str = "yungsbridges:has_structure/bridge";
const STEP: usize = 4;
/// The feature index the bridge list's random is seeded with in its step
/// (Forge appends it to the river biomes' lists; this one is past any step's
/// own features, so it shares no seed with them).
const BRIDGE_INDEX: i32 = 4096;
/// ITemplateFeatureProcessor's biome collections, in the order they are asked.
const WOODS: [&str; 4] = [
    "yungsbridges:collections/badlands",
    "yungsbridges:collections/taiga",
    "yungsbridges:collections/jungle",
    "yungsbridges:collections/savanna",
];

/// The mod's data as the game loaded it.
struct Config {
    entries: Vec<engine::Entry>,
    biomes: BTreeSet<BiomeId>,
    woods: [BTreeSet<BiomeId>; 4],
}

fn int(v: &Value, key: &str) -> Option<i32> {
    v.get(key)?.as_i64().map(|n| n as i32)
}

/// One `bridge_list` entry: an inline placed feature naming a bridge feature.
fn entry(data: &GeneratorData, placed: &Value) -> Option<engine::Entry> {
    let feature: Value = match &placed["feature"] {
        Value::String(name) => serde_json::from_str(data.configured_feature_json(name)?).ok()?,
        inline => inline.clone(),
    };
    if feature["type"].as_str()? != "yungsbridges:bridge" {
        return None;
    }
    let config = &feature["config"];
    let bridge = engine::Bridge {
        location: config["location"].as_str()?.to_owned(),
        is_z_axis: config["is_z_axis"].as_bool().unwrap_or(true),
    };
    let mut placement = Vec::new();
    for m in placed["placement"].as_array()? {
        placement.push(match m["type"].as_str()? {
            "yungsbridges:bridge" => engine::Modifier::Bridge(engine::BridgePlacement {
                length: int(m, "length")?,
                width: int(m, "width")?,
                min_water_z: int(m, "min_water_z")?,
                max_water_z: int(m, "max_water_z")?,
                width_offset: int(m, "width_offset")?,
                num_solid_blocks_needed: int(m, "num_solid_blocks_needed")?,
                is_z_axis: m["is_z_axis"].as_bool().unwrap_or(true),
            }),
            "minecraft:rarity_filter" => engine::Modifier::Rarity(int(m, "chance")?),
            "yungsbridges:rng_initializer" => engine::Modifier::RngInitializer,
            _ => return None,
        });
    }
    Some(engine::Entry { bridge, placement })
}

fn biome_tag(data: &GeneratorData, tag: &str) -> BTreeSet<BiomeId> {
    data.biome_tags
        .get(tag)
        .into_iter()
        .flatten()
        .filter_map(|name| data.biome_id(name))
        .collect()
}

fn read_config(data: &GeneratorData) -> Option<Config> {
    // The placed feature the modifier adds: the list behind a biome check.
    let placed: Value = serde_json::from_str(data.placed_feature_json(FEATURE)?).ok()?;
    let checks = placed["placement"].as_array()?;
    if checks.len() != 1 || checks[0]["type"].as_str() != Some("minecraft:biome") {
        return None;
    }
    let list: Value = match &placed["feature"] {
        Value::String(name) => serde_json::from_str(data.configured_feature_json(name)?).ok()?,
        inline => inline.clone(),
    };
    if list["type"].as_str()? != "yungsbridges:multiple_attempt_single_random" {
        return None;
    }
    let entries = list["config"]["features"]
        .as_array()?
        .iter()
        .map(|e| entry(data, e))
        .collect::<Option<Vec<_>>>()?;
    Some(Config {
        entries,
        biomes: biome_tag(data, BIOMES),
        woods: WOODS.map(|t| biome_tag(data, t)),
    })
}

/// The config of a generator's data, read once.
fn config(data: &Arc<GeneratorData>) -> Option<Arc<Config>> {
    static CACHE: Mutex<Vec<(Weak<GeneratorData>, Option<Arc<Config>>)>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(w, _)| w.strong_count() > 0);
    if let Some((_, c)) = cache
        .iter()
        .find(|(w, _)| w.upgrade().is_some_and(|d| Arc::ptr_eq(&d, data)))
    {
        return c.clone();
    }
    let c = read_config(data).map(Arc::new);
    cache.push((Arc::downgrade(data), c.clone()));
    c
}

/// The decoration region as the engine sees it.
struct Live<'w, 'a> {
    world: &'w mut World<'a>,
    sea_level: i32,
}
impl Live<'_, '_> {
    fn id(&self, state: &engine::State) -> BlockStateId {
        let registry = self.world.decoration.bindings.registry();
        let air = registry.default_state("minecraft:air").unwrap();
        let Some(default) = registry.default_state(&state.name) else {
            return air;
        };
        let mut s = registry.state(default).unwrap().clone();
        for (k, v) in &state.props {
            if let Some(slot) = s.properties.get_mut(k) {
                let previous = std::mem::replace(slot, v.clone());
                if registry.id(&s).is_none() {
                    *s.properties.get_mut(k).unwrap() = previous;
                }
            }
        }
        registry.id(&s).unwrap_or(default)
    }
    fn state_of(&self, id: BlockStateId) -> engine::State {
        let registry = self.world.decoration.bindings.registry();
        match registry.state(id) {
            Some(s) => engine::State {
                name: s.name.clone(),
                props: s.properties.clone(),
            },
            None => engine::State::of("air"),
        }
    }
    fn read(&mut self, p: engine::Pos) -> Option<BlockStateId> {
        PlacementContext::block_state(self.world, p).ok()
    }
}
impl engine::World for Live<'_, '_> {
    fn sea_level(&self) -> i32 {
        self.sea_level
    }
    fn can_occlude(&mut self, p: engine::Pos) -> Option<bool> {
        let state = self.read(p)?;
        PlacementContext::can_occlude(self.world, state).ok()
    }
    fn air_or_liquid(&mut self, p: engine::Pos) -> Option<(bool, bool)> {
        let state = self.read(p)?;
        let name = self.world.decoration.bindings.registry().state(state)?.name.as_str();
        Some((
            matches!(
                name,
                "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
            ),
            // BlockState.liquid: the water and lava blocks (LiquidBlock).
            matches!(name, "minecraft:water" | "minecraft:lava"),
        ))
    }
    fn surface(&mut self, x: i32, z: i32) -> Option<i32> {
        PlacementContext::height(self.world, HeightmapType::WorldSurface, x, z).ok()
    }
    fn state(&mut self, p: engine::Pos) -> Option<engine::State> {
        let id = self.read(p)?;
        Some(self.state_of(id))
    }
    fn set(&mut self, p: engine::Pos, state: &engine::State) {
        let id = self.id(state);
        let _ = FeatureWorld::set_block(self.world, p, id, 2);
    }
}

/// The decoration's random as the engine draws it.
struct Draws<'r>(&'r mut WorldgenRandom<XoroshiroRandom>);
impl engine::Random for Draws<'_> {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
    }
    fn next_float(&mut self) -> f32 {
        self.0.next_float()
    }
    fn next_long(&mut self) -> i64 {
        self.0.next_long()
    }
    fn set_seed(&mut self, seed: i64) {
        self.0.set_seed(seed)
    }
}

/// BridgeFeature.place: the template at the level's sea level (turned
/// counterclockwise for bridges along X) with the game's placeInWorld, then
/// the mod's processors on the template's blocks. False when the template is
/// missing.
fn place_bridge(
    live: &mut Live<'_, '_>,
    random: &mut Draws<'_>,
    resources: &StructureResources,
    config: &Config,
    bridge: &engine::Bridge,
    pos: engine::Pos,
) -> bool {
    let Some(template) = resources.template(&bridge.location) else {
        return false;
    };
    if template.palettes().is_empty() {
        return false;
    }
    let corner = engine::bridge_corner(live, pos);
    let rotation = if bridge.is_z_axis {
        Rotation::None
    } else {
        Rotation::Counterclockwise90
    };
    let center = [
        corner[0] + template.size[0] / 2,
        corner[1],
        corner[2] + template.size[2] / 2,
    ];
    let mut settings = PlaceSettings::new(rotation);
    let loot: &mut dyn ProcessorRandom = &mut *random.0;
    let _ = place_in_world(
        &mut *live.world,
        resources,
        &template,
        corner,
        center,
        &mut settings,
        Some(loot),
        2,
    );
    // StructureTemplate.filterBlocks: the palette placeInWorld chose (the
    // corner's seed), each block moved and turned with the template.
    let palette = match template.palettes().len() {
        1 => 0,
        n => LegacyRandom::new(position_seed(BlockPos::new(corner[0], corner[1], corner[2])))
            .next_int_bound(n as i32)
            .unwrap_or(0) as usize,
    };
    let blocks: Vec<(engine::Pos, engine::State)> = template.palettes()[palette]
        .blocks()
        .iter()
        .map(|info| {
            let at = StructureTemplate::transform(info.pos, Mirror::None, rotation, [0; 3]);
            let state = resources.transforms.rotate(info.state, rotation);
            (
                [at[0] + corner[0], at[1] + corner[1], at[2] + corner[2]],
                live.state_of(state),
            )
        })
        .collect();
    // level.getBiome(cornerPos).
    let biome = PlacementContext::biome_identity(&mut *live.world, corner).ok();
    let is = |set: &BTreeSet<BiomeId>| biome.is_some_and(|b| set.contains(&b));
    let wood = engine::Wood::of_biome(
        is(&config.woods[0]),
        is(&config.woods[1]),
        is(&config.woods[2]),
        is(&config.woods[3]),
    );
    engine::process(live, random, &blocks, wood);
    true
}

/// The bridge list for the chunk being decorated, after its surface_structures
/// features: PlacedFeature.placeWithBiomeCheck of `yungsbridges:bridge_list`
/// (the biome at the decoration origin must have it) on the decoration random.
fn bridges(
    this: &Decoration,
    data: &Arc<GeneratorData>,
    world: &mut World<'_>,
    random: &mut WorldgenRandom<XoroshiroRandom>,
    decoration_seed: i64,
    origin: [i32; 3],
) {
    let Some(config) = config(data) else {
        return;
    };
    let Some(resources) = this.bindings.structures.clone() else {
        return;
    };
    random.set_feature_seed(decoration_seed, BRIDGE_INDEX, STEP as i32);
    // BiomeFilter: the biome at the origin carries the feature.
    let here = PlacementContext::biome_identity(&mut *world, origin);
    if !here.is_ok_and(|b| config.biomes.contains(&b)) {
        return;
    }
    let sea_level = PlacementContext::sea_level(&mut *world).unwrap_or(63);
    let mut live = Live { world, sea_level };
    let mut draws = Draws(random);
    engine::place_list(
        &mut live,
        &mut draws,
        &config.entries,
        origin,
        |live, draws, bridge, pos| place_bridge(live, draws, &resources, &config, bridge, pos),
    );
}

/// Decoration::decorate with the bridge list after step surface_structures.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decorate(
    this: &Decoration,
    data: &Arc<GeneratorData>,
    seed: i64,
    generation: WorldGenerationContext,
    worker: &mut WorkerState,
    region: &mut [ProtoChunk; 9],
    level_random: &mut dyn crate::worldgen::structure::legacy::PieceRandom,
    structures: Option<&crate::worldgen::generator_structures::StructureState>,
) -> Result<DecorateStats, GeneratorError> {
    let center = region[4].position();
    for (i, chunk) in region.iter().enumerate() {
        let expected = ChunkPos::new(
            center.x.wrapping_add(i as i32 % 3 - 1),
            center.z.wrapping_add(i as i32 / 3 - 1),
        );
        if chunk.position() != expected {
            return Err(resource("decoration region is not a 3×3 neighborhood"));
        }
        if (chunk.terrain.generation_stage() as u8) < PreLightChunkStage::Terrain as u8 {
            return Err(resource("decoration requires terrain-stage chunks"));
        }
    }
    if region[4].is_decorated() {
        return Ok(DecorateStats::default());
    }
    let mut chunks = Vec::with_capacity(9);
    let mut biomes = Vec::with_capacity(9);
    let (min_y, height) = (region[4].terrain.min_y(), region[4].terrain.height());
    for chunk in region.iter_mut() {
        let empty = crate::worldgen::terrain::TerrainChunk::new(
            chunk.position(),
            min_y,
            height,
            data.registry(),
            data.block_states().clone(),
        )
        .map_err(|e| resource(e.to_string()))?;
        chunks.push(std::mem::replace(&mut chunk.terrain, empty));
        biomes.push(chunk.biomes().clone());
    }
    let terrain = TerrainRegion::new(chunks, generation)
        .and_then(|region| region.with_block_bindings(&this.bindings))
        .map(|region| region.with_world_seed(seed));
    let terrain = match terrain {
        Ok(terrain) => terrain,
        Err(e) => return Err(resource(e.to_string())),
    };
    let structures_only = worker.structures_only;
    let references = region[4].structure_starts.clone();
    let mut block_entities = Vec::new();
    for chunk in region.iter_mut() {
        block_entities.append(&mut chunk.block_entities);
    }
    let mut world = World {
        block_entities,
        entities: Vec::new(),
        level_random,
        structure_mobs: worker.structure_mobs.clone(),
        starts: references.clone(),
        biome_names: data.biome_names(),
        blocks: data
            .feature_resources
            .as_ref()
            .and_then(|r| r.blocks.as_ref()),
        region: terrain,
        biomes: BiomeRegion::new(biomes, obfuscate_seed(seed))
            .map_err(|e| resource(e.to_string()))?,
        zone: GenerationWriteZone::new(center, 1),
        decoration: this,
        temperatures: &mut worker.temperatures,
        behaviours: data.behaviours.as_deref(),
        // WorldGenRegion.isOutsideBuildHeight: the level's range, not the noise's.
        build: (min_y, min_y + height),
    };
    let mut stats = DecorateStats::default();
    let blocks = this.blocks;
    // ChunkGenerator.getWritableArea: the chunk's minimum Y + 1 to its maximum Y.
    let chunk_box = BoundingBox::new(
        [center.x * 16, min_y + 1, center.z * 16],
        [center.x * 16 + 15, min_y + height - 1, center.z * 16 + 15],
    );
    // The decoration origin: the chunk's corner at the lowest section.
    let origin = [center.x.wrapping_shl(4), min_y.wrapping_shr(4).wrapping_shl(4), center.z.wrapping_shl(4)];
    let mut structure_errors = 0;
    let result = this.plan.decorate(
        &mut world,
        center,
        seed,
        STEPS.len(),
        |world, random, decoration_seed, step| {
            // The step after surface_structures starts: its features are done.
            if step == STEP + 1 && !structures_only {
                bridges(this, data, world, random, decoration_seed, origin);
            }
            let Some(structures) = structures else {
                return Ok(());
            };
            let Some(in_step) = structures.by_step.get(step) else {
                return Ok(());
            };
            for (index, &structure) in in_step.iter().enumerate() {
                random.set_feature_seed(decoration_seed, index as i32, step as i32);
                for start in references.iter().filter(|s| s.structure == structure) {
                    let mut place_feature = |world: &mut World<'_>,
                                             random: &mut WorldgenRandom<XoroshiroRandom>,
                                             name: &str,
                                             pos: [i32; 3]| {
                        match this.named_feature(name) {
                            Some(feature) => feature
                                .place(world, random, pos, blocks)
                                .map_err(|e| PlacementQueryError::Failed(e.to_string())),
                            None => Ok(false),
                        }
                    };
                    if place_in_chunk(
                        start,
                        world,
                        &structures.resources,
                        random,
                        chunk_box,
                        &mut place_feature,
                    )
                    .is_err()
                    {
                        structure_errors += 1;
                    }
                }
            }
            Ok(())
        },
        |world, placer, random, id, origin| {
            if structures_only {
                return Ok(false);
            }
            let Ok(feature) = &this.features[id.0 as usize] else {
                stats.unsupported += 1;
                stats.skipped.insert(this.names[id.0 as usize].clone());
                return Ok(false);
            };
            stats.placed += 1;
            let placed = placer.place_with_biome_check(
                &feature.placement,
                world,
                random,
                origin,
                id,
                |world, random, p| feature.feature.place(world, random, p, blocks),
            );
            if placed.is_err() {
                stats.failed += 1;
                stats.skipped.insert(this.names[id.0 as usize].clone());
            }
            Ok(placed.unwrap_or(false))
        },
    );
    for (pos, nbt) in std::mem::take(&mut world.block_entities) {
        let index = ((pos[2] >> 4).wrapping_sub(center.z) + 1) * 3
            + (pos[0] >> 4).wrapping_sub(center.x)
            + 1;
        if let Some(chunk) = usize::try_from(index).ok().and_then(|i| region.get_mut(i)) {
            chunk.block_entities.push((pos, nbt));
        }
    }
    // WorldGenRegion.addFreshEntity: into the chunk holding the position.
    for nbt in std::mem::take(&mut world.entities) {
        let Some(at) = block_of(&nbt) else {
            continue;
        };
        let index = ((at[2] >> 4).wrapping_sub(center.z) + 1) * 3
            + (at[0] >> 4).wrapping_sub(center.x)
            + 1;
        if let Some(chunk) = usize::try_from(index).ok().and_then(|i| region.get_mut(i)) {
            chunk.entities.push(nbt);
        }
    }
    for terrain in world.region.into_chunks() {
        let p = terrain.position();
        let index = (p.z.wrapping_sub(center.z) + 1) * 3 + p.x.wrapping_sub(center.x) + 1;
        region[index as usize].terrain = terrain;
    }
    result.map_err(|e| GeneratorError::Stage("features", e.to_string()))?;
    stats.failed += structure_errors;
    region[4]
        .terrain
        .set_generation_stage(PreLightChunkStage::Features);
    region[4].set_decorated();
    Ok(stats)
}
