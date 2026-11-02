// InfuserBlock.use: the infuser's menu (open_menu reaches open_station for a block it does not know).
fn enchantinginfuser_open(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    name: &str,
) -> ::core::ops::ControlFlow<bool> {
    if game.infuser_open(pos, name) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
