// ItemPhysic inside Horizon's game (nightly 142388e1c). The item entity's physics run in the game library
// (`horizon_client::entities::itemphysic`, mixins/itemphysic/physics.rs); this part plays what the library cannot
// (sounds for every player) and shields the undestroyable items from explosions.
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("itemphysic_engine.rs");
}

#[allow(dead_code)]
const MOD_ID: &str = "itemphysic";

include!("kit_sound.rs");

use horizon_client::items::ItemStack;

thread_local! {
    /// The undestroyable items an explosion is going through: (item entity id, its real stack).
    static SHIELDED: std::cell::RefCell<Vec<(u32, ItemStack)>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Game<'_> {
    /// HEAD `tick_block_entities`: the landing sounds of the items' tick (`WOOL_FALL`, volume
    /// `min(1, fallDistance / 10)`, pitch 1..2), heard by every player; old fall distances are forgotten.
    pub(super) fn itemphysic_after_items(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let sounds = std::mem::take(&mut *horizon_client::entities::itemphysic::SOUNDS.lock().unwrap_or_else(|e| e.into_inner()));
        for (at, volume, pitch) in sounds {
            kit_sound::at(self, engine::FALL_SOUND, at, volume, pitch);
        }
        if self.ticks.is_multiple_of(100) {
            let alive: Vec<u32> = self.dropped.iter().map(|i| i.id).collect();
            horizon_client::entities::itemphysic::forget_except(&alive);
        }
    }

    /// HEAD `explosion_hurt_entities`: undestroyable items (the pack's list: nether star, bedrock, obsidian,
    /// barrier) take no explosion damage (`ItemPhysicServer.hurt`); they still get the push.
    pub(super) fn itemphysic_explosion_begin(&mut self) {
        let mut shielded = Vec::new();
        for item in &mut self.dropped {
            if &*item.stack.item != "minecraft:nether_star"
                && horizon_client::entities::itemphysic::undestroyable(&item.stack.item)
            {
                let star = ItemStack::new("minecraft:nether_star", item.stack.count);
                shielded.push((item.id, std::mem::replace(&mut item.stack, star)));
            }
        }
        SHIELDED.with(|s| s.borrow_mut().extend(shielded));
    }
    /// RETURN `explosion_hurt_entities`: the shielded items get their stacks back.
    pub(super) fn itemphysic_explosion_end(&mut self) {
        let shielded = SHIELDED.with(|s| std::mem::take(&mut *s.borrow_mut()));
        for (id, stack) in shielded {
            if let Some(item) = self.dropped.iter_mut().find(|i| i.id == id) {
                item.stack = stack;
            }
        }
    }
}
