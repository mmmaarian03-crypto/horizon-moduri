fn ae2_save_id(e: &crate::containers::BlockEntity, block: &str) -> ::core::ops::ControlFlow<String> {
    // An AE2 window's temporary block entity is saved under its block's id...
    if block.starts_with("ae2:") && matches!(e, crate::containers::BlockEntity::Chest(_)) {
        ::core::ops::ControlFlow::Break(block.to_owned())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
