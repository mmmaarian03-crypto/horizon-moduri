// ComputeFovModifierCallback (FovModifierHandler): a gliding player's view widens.
fn hangglider_fov(value: f32, game: &crate::game::Game<'_>) -> f32 {
    game.hangglider_fov(value)
}
