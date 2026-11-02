fn vw_geometry(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    game.visualworkbench_geometry(camera, out);
}
