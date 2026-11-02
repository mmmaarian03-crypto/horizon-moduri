// Options > Mods lists Carry On, with its key.
fn carryon_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::carryon as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.to_owned(),
        title: m::TITLE.to_owned(),
        description: m::DESCRIPTION.to_owned(),
        keys: vec![["carry".to_owned(), "Carry On Key".to_owned(), "key.keyboard.left.shift".to_owned()]],
    });
}
