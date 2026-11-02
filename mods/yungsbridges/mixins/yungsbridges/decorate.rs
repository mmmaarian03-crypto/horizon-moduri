fn yungsbridges_decorate(
    this: &Decoration,
    data: &Arc<GeneratorData>,
    seed: i64,
    generation: WorldGenerationContext,
    worker: &mut WorkerState,
    region: &mut [ProtoChunk; 9],
    level_random: &mut dyn super::structure::legacy::PieceRandom,
    structures: Option<&super::generator_structures::StructureState>,
) -> core::ops::ControlFlow<Result<DecorateStats, GeneratorError>> {
    core::ops::ControlFlow::Break(yungsbridges::decorate(
        this,
        data,
        seed,
        generation,
        worker,
        region,
        level_random,
        structures,
    ))
}
