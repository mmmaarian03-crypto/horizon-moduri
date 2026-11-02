fn ironchest_crystal(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    game.ironchest_crystal_items(camera, out);
}
