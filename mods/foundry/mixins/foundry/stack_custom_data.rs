// A stack's `minecraft:custom_data` goes on the wire with it (vanilla's stream codec sends every synchronised component).
fn foundry_stack_custom_data(
    value: Option<Stack>,
    s: &Option<ItemStack>,
) -> Option<Stack> {
    let mut value = value;
    if let (Some(stack), Some(item)) = (value.as_mut(), s.as_ref())
        && let Some(tag) = item.data().and_then(|d| d.other("minecraft:custom_data"))
        && !stack.added.iter().any(|(kind, _)| *kind == horizon_protocol::item::component::CUSTOM_DATA)
    {
        stack.added.push((horizon_protocol::item::component::CUSTOM_DATA, horizon_protocol::item::Component::Nbt(tag.clone())));
    }
    value
}
