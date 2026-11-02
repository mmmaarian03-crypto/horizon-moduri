// ItemSoulShard.appendHoverText: bound entity, tier, souls (and the owner with advanced tooltips).
fn soulshards_tooltip(
    value: Vec<(horizon_client::text::Text, u32)>,
    game: &crate::game::Game<'_>,
    stack: &ItemStack,
) -> Vec<(horizon_client::text::Text, u32)> {
    game.soulshards_tooltip(stack, value)
}
