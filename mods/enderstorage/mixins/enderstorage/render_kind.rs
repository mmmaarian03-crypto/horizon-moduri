fn enderstorage_kind(this: &BlockEntityRenderer, state: u32) -> ::core::ops::ControlFlow<&Kind> {
    // RenderTileEnderChest: the chest model with the mod's texture, turned by the facing, never
    // paired, the lid opening as a chest's (the buttons and the latch are not drawn).
    static KINDS: std::sync::OnceLock<Vec<Kind>> = std::sync::OnceLock::new();
    let _ = this;
    let Some(facing) = crate::enderstorage::chest_state(state) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let kinds = KINDS.get_or_init(|| {
        crate::enderstorage::FACINGS
            .iter()
            .map(|f| Kind::Chest {
                block: crate::enderstorage::CHEST.to_owned(),
                texture: crate::enderstorage::CHEST_TEXTURE,
                part: ChestPart::Single,
                y_rot: y_rot(f),
                double_with: None,
            })
            .collect()
    });
    match kinds.get(facing) {
        Some(kind) => ::core::ops::ControlFlow::Break(kind),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
