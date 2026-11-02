fn ironchest_kind(this: &BlockEntityRenderer, state: u32) -> ::core::ops::ControlFlow<&Kind> {
    // IronChestRenderer: the vanilla single chest model (its iron_bottom, iron_lid and
    // iron_lock are ChestModel's bottom, lid and lock) with the chest's own texture,
    // turned by the facing, never paired, the lid opening as a chest's.
    static KINDS: std::sync::OnceLock<Vec<Kind>> = std::sync::OnceLock::new();
    let _ = this;
    let Some((block, facing)) = crate::ironchest::chest_state(state) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let kinds = KINDS.get_or_init(|| {
        let mut kinds = Vec::new();
        for (i, (name, _, _)) in crate::ironchest::BLOCKS.iter().enumerate() {
            for f in crate::ironchest::FACINGS {
                kinds.push(Kind::Chest {
                    block: (*name).to_owned(),
                    texture: crate::ironchest::texture(i),
                    part: ChestPart::Single,
                    y_rot: y_rot(f),
                    double_with: None,
                });
            }
        }
        kinds
    });
    match kinds.get(block * 4 + facing) {
        Some(kind) => ::core::ops::ControlFlow::Break(kind),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
