// Right after the particle tick of the window (`render_tick`): the landed drips' sounds, then new marks.
fn waterdripsound_after(game: &mut crate::game::Game<'_>) {
    game.waterdripsound_after_particles();
}
