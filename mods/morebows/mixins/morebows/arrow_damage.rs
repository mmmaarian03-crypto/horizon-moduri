fn morebows_arrow_damage(value: f32, arrow: &Projectile) -> f32 {
    crate::game::Game::morebows_blaze_factor(arrow, value)
}
