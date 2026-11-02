// RETURN on `stack_tooltip`: the grey description line of a pattern item.
fn additionalbanners_tooltip(
    value: Vec<(horizon_client::text::Text, u32)>,
    game: &crate::game::Game<'_>,
    stack: &ItemStack,
) -> Vec<(horizon_client::text::Text, u32)> {
    game.additionalbanners_tooltip(stack, value)
}
