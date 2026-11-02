// The plates of the mobs near the player, before the frame's name tags.
fn neat_plates(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    game.neat_plates(camera, alpha, out);
}
