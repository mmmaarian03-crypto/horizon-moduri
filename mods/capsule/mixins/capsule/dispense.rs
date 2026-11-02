// DispenseCapsuleBehavior: a dispenser (the capture base is one) deploys or undeploys a capsule.
fn capsule_dispense(
    game: &mut Game<'_>,
    pos: Pos,
    facing: Direction,
    stack: &ItemStack,
) -> ::core::ops::ControlFlow<Result<Option<ItemStack>>> {
    match game.capsule_dispense([pos.0, pos.1, pos.2], facing.normal(), stack) {
        Some(left) => ::core::ops::ControlFlow::Break(Ok(left)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
