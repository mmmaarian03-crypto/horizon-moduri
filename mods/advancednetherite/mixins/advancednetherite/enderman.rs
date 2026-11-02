fn advancednetherite_enderman(m: &Mob) -> core::ops::ControlFlow<bool> {
    // AdvancedArmorItem.pacifiesEndermen: the anger of the stare is cancelled.
    if m.extra.hidden_effects.contains(&"advancednetherite:pacified") {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
