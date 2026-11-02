fn torchmaster_patrol(
    game: &mut crate::game::Game<'_>,
    kind: &str,
    at: [f64; 3],
    reason: horizon_client::mobs::raider::Reason,
) -> ::core::ops::ControlFlow<Option<u32>> {
    if reason == horizon_client::mobs::raider::Reason::Patrol && game.torchmaster_blocks_patrol(kind, at) {
        ::core::ops::ControlFlow::Break(None)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
