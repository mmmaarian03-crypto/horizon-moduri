fn storagedrawers_attack(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    // FaceSlotBlock.attack: the first hit of a mining takes an item.
    game.storagedrawers_attack(pos);
}
