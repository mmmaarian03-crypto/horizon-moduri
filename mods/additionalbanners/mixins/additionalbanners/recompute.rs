// HEAD on `Menu::recompute`: the shears of a pattern recipe come back hurt by one.
fn additionalbanners_recompute(menu: &mut crate::menu::Menu, env: &crate::menu::Env<'_>) {
    crate::menu::additionalbanners::recompute(menu, env);
}
