fn ae2_save_load(tag: &Tag) -> ::core::ops::ControlFlow<Option<((i32, i32, i32), crate::containers::BlockEntity)>> {
    // ...and never loaded back (its slots only mirrored the network).
    match text(tag.get("id")) {
        Some(id) if id.starts_with("ae2:") => ::core::ops::ControlFlow::Break(None),
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
