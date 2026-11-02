fn torchmaster_join(keep_world: bool) {
    // keep_world is false for a login, true for a respawn.
    if !keep_world {
        crate::game::Game::torchmaster_forget_volumes();
    }
}
