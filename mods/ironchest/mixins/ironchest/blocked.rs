fn ironchest_blocked(
    game: &crate::game::Game<'_>,
    key: (i32, i32, i32),
    name: &str,
) -> ::core::ops::ControlFlow<bool> {
    // AbstractIronChestBlock.isChestBlockedAt: a redstone conductor or a sitting cat
    // above keeps the menu shut (iron chests never pair, so only their own block).
    if crate::game::ironchest::ironchest_is_chest(name) {
        ::core::ops::ControlFlow::Break(game.chest_blocked_at(key))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
