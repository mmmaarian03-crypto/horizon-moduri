// Options > Mods lists Overflowing Bars.
fn overflowingbars_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::overflowingbars as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.to_owned(),
        title: m::TITLE.to_owned(),
        description: m::DESCRIPTION.to_owned(),
        keys: Vec::new(),
    });
}
