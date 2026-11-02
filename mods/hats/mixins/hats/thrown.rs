// The thrown hats in the world.
fn hats_thrown(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    game.hats_draw_thrown(camera, alpha, out);
}
