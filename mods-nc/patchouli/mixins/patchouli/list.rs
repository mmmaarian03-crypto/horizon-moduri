// Options > Mods lists Patchouli (and turns it off).
fn patchouli_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::Game::patchouli_mods(mods);
}
