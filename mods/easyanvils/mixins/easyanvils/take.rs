fn easyanvils_take(game: &Game<'_>) -> ::core::ops::ControlFlow<bool> {
    let Some(s) = game.station.as_ref() else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if s.kind != Kind::Anvil {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(
        (game.creative || game.experience.level >= s.anvil.cost) && s.anvil.cost >= 0,
    )
}
