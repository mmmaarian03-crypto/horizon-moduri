fn easyanvils_session(
    game: &mut Game<'_>,
    menu: &horizon_client::menu::Menu,
    predicting: bool,
) -> ::core::ops::ControlFlow<()> {
    // The anvil take of the host: as the game does it, with Easy Anvils' wear.
    let Some(s) = game.station.as_mut() else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if s.kind != Kind::Anvil || !menu.combined || predicting {
        return ::core::ops::ControlFlow::Continue(());
    }
    let pos = s.pos;
    s.anvil.cost = menu.data[0];
    s.anvil.repair_item_count_cost = menu.anvil.repair_count;
    s.anvil.only_renaming = menu.anvil.only_renaming;
    s.anvil.result = s.result.clone();
    game.station_after_input();
    game.give_levels(menu.level_change);
    Game::easyanvils_took(menu.level_change);
    let creative = game.creative;
    if game.easyanvils_used(pos, creative, |g, p, c| g.anvil_used(p, c)) {
        game.close_station();
        game.screen = Screen::Playing;
        game.capture(true);
    }
    ::core::ops::ControlFlow::Break(())
}
