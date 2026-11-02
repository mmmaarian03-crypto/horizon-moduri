// LivingExperienceDropEvent cancelled for cage-born mobs (dropExperience = false): the mob whose death the
// game is handling now was marked by the death hook.
fn soulshards_no_xp(m: &Mob) -> core::ops::ControlFlow<bool> {
    if crate::soulshards::NO_XP.load(core::sync::atomic::Ordering::Relaxed) == m.id {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
