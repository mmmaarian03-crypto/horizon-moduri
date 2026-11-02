fn easyanvils_dispense(
    game: &mut Game<'_>,
    pos: Pos,
    facing: Direction,
    stack: &ItemStack,
) -> ::core::ops::ControlFlow<Result<Option<ItemStack>>> {
    let s = step(facing);
    match game.easyanvils_dispense([pos.0, pos.1, pos.2], s, stack) {
        Some((left, event)) => {
            game.dispenser_event(event, pos, 0);
            game.dispenser_event(2000, pos, data_value(facing));
            ::core::ops::ControlFlow::Break(Ok(left))
        }
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
