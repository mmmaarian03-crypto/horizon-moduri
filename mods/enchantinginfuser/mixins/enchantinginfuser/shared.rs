// Enchanting Infuser (native mod), the part in the game library (`horizon_client::enchantinginfuser`): which
// block states are infusers, for the library's hooks (block-entity renderer, book ticker, save ids). Plain
// Rust without game types.
use std::sync::RwLock;

/// The infuser blocks (also their block entity ids, `enchantinginfuser:enchanting_infuser` in the
/// original for both; each one's own name here, so a load knows the block).
pub const BLOCKS: [&str; 2] = ["enchantinginfuser:enchanting_infuser", "enchantinginfuser:advanced_enchanting_infuser"];

static STATES: RwLock<Vec<bool>> = RwLock::new(Vec::new());

/// Whether each state of the block registry in use is an infuser.
pub fn set_states(states: Vec<bool>) {
    if let Ok(mut s) = STATES.write() {
        *s = states;
    }
}
pub fn is_infuser(state: u32) -> bool {
    STATES.read().ok().and_then(|s| s.get(state as usize).copied()).unwrap_or(false)
}
pub fn is_block(name: &str) -> bool {
    BLOCKS.contains(&name)
}
