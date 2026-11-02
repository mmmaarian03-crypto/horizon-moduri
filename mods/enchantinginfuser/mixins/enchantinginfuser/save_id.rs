fn enchantinginfuser_save_id(
    e: &crate::containers::BlockEntity,
    block: &str,
) -> ::core::ops::ControlFlow<String> {
    // The infuser's block entity: its one slot, under the block's own name.
    if matches!(e, crate::containers::BlockEntity::Chest(_)) && crate::enchantinginfuser::is_block(block) {
        ::core::ops::ControlFlow::Break(block.to_owned())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
