// Options > Mods lists Neat, with its toggle key.
fn neat_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::neat as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.to_owned(),
        title: m::TITLE.to_owned(),
        description: m::DESCRIPTION.to_owned(),
        keys: vec![[
            "toggle".to_owned(),
            "Toggle health bars".to_owned(),
            "key.keyboard.unknown".to_owned(),
        ]],
    });
}
