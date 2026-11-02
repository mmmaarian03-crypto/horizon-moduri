// InfuserBlock extends EnchantmentTableBlock: super.animateTick, the enchanting glyphs flying from the
// bookshelves around (the game knows the enchanting table only by name).
fn enchantinginfuser_animate(name: &str) -> ::core::ops::ControlFlow<Animate> {
    match name {
        "enchantinginfuser:enchanting_infuser" | "enchantinginfuser:advanced_enchanting_infuser" => {
            ::core::ops::ControlFlow::Break(Animate::EnchantingTable)
        }
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
