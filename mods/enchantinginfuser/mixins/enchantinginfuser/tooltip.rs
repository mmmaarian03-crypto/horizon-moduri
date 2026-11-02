// InfuserBlock.appendHoverText: one gray line, "Choose (, modify and remove) enchantments for your gear."
// and, with repairing, " Repair your gear with levels." (the container lines' place; the item holds none).
fn enchantinginfuser_tooltip(
    stack: &crate::items::ItemStack,
    translate: &dyn Fn(&str) -> Option<String>,
) -> ::core::ops::ControlFlow<Vec<String>> {
    let (key, repair) = match &*stack.item {
        "enchantinginfuser:enchanting_infuser" => ("block.enchantinginfuser.description.choose", false),
        "enchantinginfuser:advanced_enchanting_infuser" => ("block.enchantinginfuser.description.chooseAndModify", true),
        _ => return ::core::ops::ControlFlow::Continue(()),
    };
    let text = |k: &str| translate(k).unwrap_or_else(|| k.to_owned());
    let mut line = format!("\u{a7}7{}", text(key));
    if repair {
        line.push(' ');
        line.push_str(&text("block.enchantinginfuser.description.repair"));
    }
    ::core::ops::ControlFlow::Break(vec![line])
}
