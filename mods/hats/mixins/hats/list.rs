// Options > Mods lists Hats Renewed, with its key.
fn hats_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::hats as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.to_owned(),
        title: m::TITLE.to_owned(),
        description: m::DESCRIPTION.to_owned(),
        keys: vec![["open_menu".to_owned(), "Open Hats Menu".to_owned(), "key.keyboard.h".to_owned()]],
    });
}
