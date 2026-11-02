fn easyanvils_guest(
    game: &mut Game<'_>,
    host: &mut LanHost,
    conn: u32,
    gs: &mut GuestStation,
) -> ::core::ops::ControlFlow<()> {
    // A guest's anvil take: as the game does it, with Easy Anvils' wear.
    if gs.kind == StationKind::Smithing {
        return ::core::ops::ControlFlow::Continue(());
    }
    let change = std::mem::take(&mut gs.menu.level_change);
    if change != 0 {
        game.as_guest(host, conn, |g| g.give_levels(change));
    }
    if !std::mem::take(&mut gs.menu.combined) {
        return ::core::ops::ControlFlow::Break(());
    }
    Game::easyanvils_took(change);
    let creative = gs.menu.creative;
    game.easyanvils_used(gs.pos, creative, |g, p, c| g.anvil_used(p, c));
    ::core::ops::ControlFlow::Break(())
}
