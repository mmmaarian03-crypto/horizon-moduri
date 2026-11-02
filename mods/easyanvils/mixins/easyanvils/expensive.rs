fn easyanvils_expensive(
    game: &Game<'_>,
    key: &str,
    fallback: &str,
) -> ::core::ops::ControlFlow<String> {
    // The screen of Easy Anvils shows the cost, not "Too Expensive!", when the limit is off.
    let _ = fallback;
    if key != "container.repair.expensive" {
        return ::core::ops::ControlFlow::Continue(());
    }
    let Some(s) = game.station.as_ref() else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if s.kind != Kind::Anvil || s.anvil.cost < 40 {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(
        horizon_client::lang::get_or("container.repair.cost", "Enchantment Cost: %1$s")
            .replace("%1$s", &s.anvil.cost.to_string()),
    )
}
