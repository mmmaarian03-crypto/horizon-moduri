// InfuserItemRenderer: the item over the open book.
fn enchantinginfuser_floating(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    game.infuser_floating_items(camera, out);
}
