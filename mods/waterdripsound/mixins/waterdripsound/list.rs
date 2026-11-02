// Options > Mods lists Drip Sounds.
fn waterdripsound_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::waterdripsound as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.to_owned(),
        title: m::TITLE.to_owned(),
        description: m::DESCRIPTION.to_owned(),
        keys: Vec::new(),
    });
}
