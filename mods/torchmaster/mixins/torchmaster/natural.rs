fn torchmaster_natural(
    game: &crate::game::Game<'_>,
    r: &horizon_client::mobs::spawn_selection::SpawnRequest<'_>,
) -> ::core::ops::ControlFlow<Option<Spawned>> {
    match game.torchmaster_natural(r) {
        Some(blocked) => ::core::ops::ControlFlow::Break(Some(blocked)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
