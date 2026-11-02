fn advancednetherite_phantom(m: &mut Mob) -> core::ops::ControlFlow<bool> {
    // onLivingChangeTargetEvent: no new target, unless the player hurt this phantom.
    if m.extra.hidden_effects.contains(&"advancednetherite:pacified")
        && m.ai.last_hurt_by != Some(Who::Player)
    {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
