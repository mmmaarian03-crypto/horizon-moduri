fn torchmaster_tooltip(
    stack: &crate::items::ItemStack,
    translate: &dyn Fn(&str) -> Option<String>,
) -> ::core::ops::ControlFlow<Option<String>> {
    // TMItemBlock and FrozenPearlItem.appendHoverText with beginnerTooltips
    // (true in the pack's torchmaster.toml): the description id plus ".tooltip".
    let key = match &*stack.item {
        "torchmaster:megatorch" => "block.torchmaster.megatorch.tooltip",
        "torchmaster:dreadlamp" => "block.torchmaster.dreadlamp.tooltip",
        "torchmaster:feral_flare_lantern" => "block.torchmaster.feral_flare_lantern.tooltip",
        "torchmaster:frozen_pearl" => "item.torchmaster.frozen_pearl.tooltip",
        _ => return ::core::ops::ControlFlow::Continue(()),
    };
    ::core::ops::ControlFlow::Break(Some(translate(key).unwrap_or_else(|| key.to_owned())))
}
