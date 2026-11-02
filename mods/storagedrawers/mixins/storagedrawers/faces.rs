fn storagedrawers_faces(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    game.storagedrawers_faces(camera, out);
}
