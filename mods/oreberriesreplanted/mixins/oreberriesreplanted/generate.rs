fn oreberries_generate(
    value: Result<DecorateStats, GeneratorError>,
    region: &mut [ProtoChunk; 9],
) -> Result<DecorateStats, GeneratorError> {
    if value.is_ok() {
        oreberriesreplanted::decorate_bushes(region);
    }
    value
}
