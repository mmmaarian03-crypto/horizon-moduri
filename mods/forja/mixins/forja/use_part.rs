// A part item used on a block: the part is placed (PartPlacement); a wrench takes a part out.
fn forja_use_part(game: &mut crate::game::Game<'_>, stack: &ItemStack) -> ::core::ops::ControlFlow<Result<bool>> {
    match game.forja_use_part(stack) {
        Some(used) => ::core::ops::ControlFlow::Break(used),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
