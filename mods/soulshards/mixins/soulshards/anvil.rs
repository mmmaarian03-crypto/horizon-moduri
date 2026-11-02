// AnvilUpdateEvent (EventHandler.onAnvil): two shards of one entity combine; anything else goes on to the anvil.
fn soulshards_anvil(
    input: Option<&ItemStack>,
    addition: Option<&ItemStack>,
) -> ::core::ops::ControlFlow<AnvilResult> {
    match crate::soulshards::anvil(input, addition) {
        Some((result, cost)) => ::core::ops::ControlFlow::Break(AnvilResult {
            result: Some(result),
            cost,
            repair_item_count_cost: 1,
            only_renaming: false,
        }),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
