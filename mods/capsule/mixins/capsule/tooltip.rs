// CapsuleItem.appendHoverText: author, one use, overpowered, size.
fn capsule_tooltip(
    value: Vec<(horizon_client::text::Text, u32)>,
    game: &crate::game::Game<'_>,
    stack: &ItemStack,
) -> Vec<(horizon_client::text::Text, u32)> {
    game.capsule_tooltip(stack, value)
}
