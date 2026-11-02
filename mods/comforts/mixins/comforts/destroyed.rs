fn destroyed(game: &mut crate::game::Game<'_>, pos: [i32; 3], state: BlockStateId) {
    game.comforts_destroyed(pos, state);
}
