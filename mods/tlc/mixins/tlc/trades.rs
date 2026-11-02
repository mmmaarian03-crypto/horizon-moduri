fn tlc_trades(trades: &mut crate::merchant::Trades, root: &Path) {
    for (id, trade) in horizon_world::worldgen::structure::start_checks::tlc::villager_trades(root) {
        trades.trades.insert(id, trade);
    }
}
