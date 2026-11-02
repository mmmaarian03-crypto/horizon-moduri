// Iron Chests (native mod), the part in the game library (`horizon_client::ironchest`):
// what the library's hooks need (save ids, block-entity renderer, lid ticker). Plain Rust
// without game types, so the mod's own tests compile this very file (tests/chests.rs).
// The sizes are IronChestsTypes.size of Iron Chests 1.20.1-14.4.4; the tests check them
// against the rules crate.
use std::sync::RwLock;

/// The chest blocks: id (also the block entity id), slots, block-entity texture.
pub const BLOCKS: [(&str, usize, &str); 14] = [
    ("ironchest:iron_chest", 54, "ironchest:block/model/iron_chest"),
    ("ironchest:gold_chest", 81, "ironchest:block/model/gold_chest"),
    ("ironchest:diamond_chest", 108, "ironchest:block/model/diamond_chest"),
    ("ironchest:copper_chest", 45, "ironchest:block/model/copper_chest"),
    ("ironchest:crystal_chest", 108, "ironchest:block/model/crystal_chest"),
    ("ironchest:obsidian_chest", 108, "ironchest:block/model/obsidian_chest"),
    ("ironchest:dirt_chest", 1, "ironchest:block/model/dirt_chest"),
    ("ironchest:trapped_iron_chest", 54, "ironchest:block/model/trapped_iron_chest"),
    ("ironchest:trapped_gold_chest", 81, "ironchest:block/model/trapped_gold_chest"),
    ("ironchest:trapped_diamond_chest", 108, "ironchest:block/model/trapped_diamond_chest"),
    ("ironchest:trapped_copper_chest", 45, "ironchest:block/model/trapped_copper_chest"),
    ("ironchest:trapped_crystal_chest", 108, "ironchest:block/model/trapped_crystal_chest"),
    ("ironchest:trapped_obsidian_chest", 108, "ironchest:block/model/trapped_obsidian_chest"),
    ("ironchest:trapped_dirt_chest", 1, "ironchest:block/model/trapped_dirt_chest"),
];

/// The block's index in [`BLOCKS`].
pub fn index(name: &str) -> Option<usize> {
    if !name.starts_with("ironchest:") {
        return None;
    }
    BLOCKS.iter().position(|b| b.0 == name)
}
/// The chest's slots, for a block (or block entity) id.
pub fn size(name: &str) -> Option<usize> {
    index(name).map(|i| BLOCKS[i].1)
}
pub fn texture(index: usize) -> &'static str {
    BLOCKS[index].2
}
/// The facings in the order of the renderer's y rotation (south 0, west 90, north 180, east 270).
pub const FACINGS: [&str; 4] = ["south", "west", "north", "east"];

/// Each block state's chest: 0 none, else 1 + block index * 4 + facing.
static STATES: RwLock<Vec<u8>> = RwLock::new(Vec::new());

/// A state's code for [`set_states`].
pub fn code(name: &str, facing: Option<&str>) -> u8 {
    let Some(i) = index(name) else {
        return 0;
    };
    let f = FACINGS
        .iter()
        .position(|x| Some(*x) == facing)
        .unwrap_or(2);
    (1 + i * 4 + f) as u8
}
/// The codes of every state of the block registry in use.
pub fn set_states(codes: Vec<u8>) {
    if let Ok(mut s) = STATES.write() {
        *s = codes;
    }
}
/// The chest a state is, as (block index, facing index).
pub fn chest_state(state: u32) -> Option<(usize, usize)> {
    let s = STATES.read().ok()?;
    match s.get(state as usize).copied() {
        None | Some(0) => None,
        Some(c) => Some(((c as usize - 1) / 4, (c as usize - 1) % 4)),
    }
}
