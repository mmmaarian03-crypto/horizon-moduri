fn torchmaster_phantom(
    game: &mut Game<'_>,
    kind: &str,
    at: [f64; 3],
    persistent: bool,
    original: impl FnOnce(&mut Game<'_>, &str, [f64; 3], bool) -> Option<u32>,
) -> Option<u32> {
    if game.torchmaster_blocks_natural(kind, at) {
        return None;
    }
    original(game, kind, at, persistent)
}
