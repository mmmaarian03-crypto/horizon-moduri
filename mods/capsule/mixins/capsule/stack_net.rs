// A capsule goes on the wire with its tag (`minecraft:custom_data`) and its model (`minecraft:custom_model_data`:
// the state's model and the two layer colours): the single-player window is a client of the integrated server.
fn capsule_stack_net(value: Option<Stack>, s: &Option<ItemStack>) -> Option<Stack> {
    use horizon_protocol::item::{Component, component};
    use horizon_world::nbt::Tag;
    let mut value = value;
    let (Some(stack), Some(item)) = (value.as_mut(), s.as_ref()) else {
        return value;
    };
    if !item.item.starts_with("capsule:") {
        return value;
    }
    let Some(d) = item.data() else {
        return value;
    };
    let missing = |stack: &Stack, k: i32| !stack.added.iter().any(|(kind, _)| *kind == k);
    if let Some(tag) = d.other("minecraft:custom_data")
        && missing(stack, component::CUSTOM_DATA)
    {
        stack.added.push((component::CUSTOM_DATA, Component::Nbt(tag.clone())));
    }
    if let Some(t) = d.other("minecraft:custom_model_data")
        && missing(stack, component::CUSTOM_MODEL_DATA)
    {
        let list = |k: &str| match t.get(k) {
            Some(Tag::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        let floats = list("floats").iter().filter_map(|v| match v { Tag::Float(b) => Some(f32::from_bits(*b)), _ => None }).collect();
        let flags = list("flags").iter().filter_map(|v| match v { Tag::Byte(b) => Some(*b != 0), _ => None }).collect();
        let strings = list("strings").iter().filter_map(|v| match v { Tag::String(s) => Some(s.to_string_lossy()), _ => None }).collect();
        let colors = list("colors").iter().filter_map(|v| match v { Tag::Int(c) => Some(*c), _ => None }).collect();
        stack.added.push((component::CUSTOM_MODEL_DATA, Component::CustomModelData { floats, flags, strings, colors }));
    }
    value
}
