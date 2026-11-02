// Every drawn mob and player: its hat on its head.
fn hats_heads(
    game: &crate::game::Game<'_>,
    mob: &horizon_client::mobs::Mob,
    camera: glam::DVec3,
    alpha: f64,
    light: u32,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    game.hats_draw_mob(mob, camera, alpha, light, out);
}
