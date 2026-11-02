// A mob dies: its hat leaves it, unlocked for the player who killed it.
fn hats_died(game: &mut crate::game::Game<'_>, m: &horizon_client::mobs::Mob) {
    game.hats_mob_died(m);
}
