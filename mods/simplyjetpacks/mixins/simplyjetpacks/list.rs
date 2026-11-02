// Options > Mods lists Simply Jetpacks 2 with its keys.
fn simplyjetpacks_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::simplyjetpacks as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.to_owned(),
        title: m::TITLE.to_owned(),
        description: m::DESCRIPTION.to_owned(),
        keys: m::simplyjetpacks_keys(),
    });
}
