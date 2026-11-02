fn enderstorage_blocked(name: &str) -> ::core::ops::ControlFlow<bool> {
    // No cat or conductor rule on the Ender Chest (BaseEntityBlock); the tank never opens a menu
    // (true = the use counts, no menu).
    match name {
        "enderstorage:ender_chest" => ::core::ops::ControlFlow::Break(false),
        "enderstorage:ender_tank" => ::core::ops::ControlFlow::Break(true),
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
