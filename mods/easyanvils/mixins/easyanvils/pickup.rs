fn easyanvils_pickup(menu: &Menu, s: usize) -> ::core::ops::ControlFlow<bool> {
    // AnvilMenu.mayPickup of Easy Anvils: free renames (cost 0) may be taken.
    if menu.kind != Kind::Anvil {
        return ::core::ops::ControlFlow::Continue(());
    }
    match menu.slots[s].role {
        Role::CombineResult => ::core::ops::ControlFlow::Break(
            (menu.creative || menu.levels >= menu.data[0]) && menu.data[0] >= 0,
        ),
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
