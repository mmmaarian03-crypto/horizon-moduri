fn tlc_price(items: &Items, v: &Value, additional: i32) -> core::ops::ControlFlow<ItemCost> {
    let Some(count) = horizon_world::worldgen::structure::start_checks::tlc::map_price(v) else {
        return core::ops::ControlFlow::Continue(());
    };
    // The rest as `cost` does: the count plus any additional cost, within a stack.
    let id = v["id"].as_str().unwrap_or("minecraft:air");
    let max = items.get(id).map_or(64, |i| i.max_stack) as i32;
    let count = (count + additional).clamp(0, max);
    core::ops::ControlFlow::Break(ItemCost {
        stack: ItemStack::new(id, count as u32),
    })
}
