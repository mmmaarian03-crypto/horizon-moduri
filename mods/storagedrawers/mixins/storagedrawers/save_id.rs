fn storagedrawers_save_id(
    e: &crate::containers::BlockEntity,
    block: &str,
) -> ::core::ops::ControlFlow<String> {
    // The block entity id of a drawer is its block's id (as the original's types are).
    if matches!(e, crate::containers::BlockEntity::Chest(_))
        && crate::storagedrawers::chest_size(block, &crate::storagedrawers::Config::tuned())
            .is_some()
    {
        ::core::ops::ControlFlow::Break(block.to_owned())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
