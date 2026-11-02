// A mob dies (LivingDeathEvent): the killer's shard takes its soul; a cage-born mob drops no experience.
fn soulshards_died(game: &mut crate::game::Game<'_>, m: &horizon_client::mobs::Mob) {
    game.soulshards_mob_died(m);
}
