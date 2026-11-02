fn tlc_stronghold(
    registry: &StructureRegistry,
    set: &StructureSet,
) -> core::ops::ControlFlow<bool> {
    if super::start_checks::tlc::strongholds_refused(registry, set) {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
