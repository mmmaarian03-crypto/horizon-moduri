// The tooltip of a stack with the descriptions of its enchantments in (ItemTooltipEvent).
fn enchdesc_tooltip(
    value: Vec<(horizon_client::text::Text, u32)>,
    game: &crate::game::Game<'_>,
    stack: &ItemStack,
) -> Vec<(horizon_client::text::Text, u32)> {
    game.enchdesc_tooltip(stack, value)
}
