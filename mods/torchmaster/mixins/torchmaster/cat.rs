fn torchmaster_cat(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> ::core::ops::ControlFlow<()> {
    // CatSpawner.spawnCat: moveTo(pos) (the block's centre), then finalizeSpawn.
    let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5];
    if game.torchmaster_blocks_natural("minecraft:cat", at) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
