// Options > Mods lists More Overlays, with its two keys.
fn moreoverlays_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::moreoverlays as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.to_owned(),
        title: m::TITLE.to_owned(),
        description: m::DESCRIPTION.to_owned(),
        keys: vec![
            [
                "light".to_owned(),
                "Toggle Light Overlay".to_owned(),
                "key.keyboard.f7".to_owned(),
            ],
            [
                "chunks".to_owned(),
                "Toggle Chunk Bounds Overlay".to_owned(),
                "key.keyboard.f9".to_owned(),
            ],
        ],
    });
}
