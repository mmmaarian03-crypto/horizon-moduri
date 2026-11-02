fn ae2_generate(
    value: Result<DecorateStats, GeneratorError>,
    region: &mut [ProtoChunk; 9],
) -> Result<DecorateStats, GeneratorError> {
    if value.is_ok() {
        ae2::place_meteorite(region);
    }
    value
}
