fn torchmaster_siege_spot(
    game: &mut Game<'_>,
    centers: &std::collections::HashSet<(i32, i32, i32)>,
    spot: [i32; 3],
    original: impl FnOnce(&mut Game<'_>, &std::collections::HashSet<(i32, i32, i32)>, [i32; 3]) -> Option<[f64; 3]>,
) -> Option<[f64; 3]> {
    if Game::torchmaster_siege_blocked() {
        return None;
    }
    let found = original(game, centers, spot);
    game.torchmaster_siege_spot(found)
}
