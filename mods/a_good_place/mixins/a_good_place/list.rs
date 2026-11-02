// Options > Mods lists A Good Place.
fn a_good_place_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::a_good_place as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.to_owned(),
        title: m::TITLE.to_owned(),
        description: m::DESCRIPTION.to_owned(),
        keys: Vec::new(),
    });
}
