// Advanced Netherite inside Horizon's game (nightly 142388e1c136). The rules are the
// engine below; this part reads the player's armor and held tool, the block that was
// mined and the mob that died, and adds what the original's event handler and its
// three global loot modifiers do. The items' numbers (durability, defence, speed...)
// are set when the content root is built: see items.rs of this bundle.
use super::Game;
#[allow(dead_code)]
mod engine {
    include!("advancednetherite_engine.rs");
}
use engine::Settings;
use horizon_client::items::ItemStack;
use horizon_client::mobs::{Mob, Species, Who};
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::registry::BlockStateId;

/// A creature that must leave the player alone is marked with this in the list
/// of its effects whose particles are hidden: a list nothing else reads names
/// from. The hooks of mobs/enderman.rs and mobs/phantom.rs read it.
pub(crate) const PACIFIED: &str = "advancednetherite:pacified";

/// The level's random as the drops draw from it.
struct Draws<'a>(&'a mut LegacyRandom);
impl engine::Rng for Draws<'_> {
    fn next_float(&mut self) -> f32 {
        self.0.next_float()
    }
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
    }
}

impl Game<'_> {
    /// What the armor the player wears pacifies (`getArmorSlots`, every piece).
    fn advancednetherite_worn(&self) -> engine::Worn {
        engine::worn_perks(
            &Settings::default(),
            self.armor.iter().map(|s| s.as_ref().map(|s| &*s.item)),
        )
    }

    /// Each tick before the mobs think: phantoms and endermen are marked or
    /// cleared (`EventHandler.onLivingChangeTargetEvent` for phantoms,
    /// `onEnderManAngerEvent`; a creative player is not spared the stare).
    pub(super) fn advancednetherite_mark(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let worn = self.advancednetherite_worn();
        let creative = self.creative;
        for m in &mut self.mobs {
            let on = match m.species() {
                Species::Phantom => worn.phantoms,
                Species::Enderman => worn.endermen && !creative,
                _ => continue,
            };
            let marked = m.extra.hidden_effects.contains(&PACIFIED);
            if on && !marked {
                m.extra.hidden_effects.push(PACIFIED);
            } else if !on && marked {
                m.extra.hidden_effects.retain(|h| *h != PACIFIED);
            }
        }
    }

    /// `PiglinAi.isWearingSafeArmor` also true for armor whose piglin perk is on
    /// (`onLivingChangeTargetEvent` for piglins).
    pub(super) fn advancednetherite_gold(&self) -> core::ops::ControlFlow<bool> {
        if self.advancednetherite_worn().piglins {
            core::ops::ControlFlow::Break(true)
        } else {
            core::ops::ControlFlow::Continue(())
        }
    }

    /// `CropDropsLootModifier` and `OreDropsLootModifier`: the drops of a block
    /// the player mined with a tool, plus the extra ones.
    pub(super) fn advancednetherite_block_loot(
        &mut self,
        state: BlockStateId,
        held: Option<&ItemStack>,
        mut drops: Vec<ItemStack>,
    ) -> Vec<ItemStack> {
        let Some(held) = held else {
            return drops;
        };
        let name = self.res.facts().get(state).name.clone();
        let age = self
            .pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get("age"))
            .and_then(|a| a.parse::<i32>().ok());
        let mature = engine::crop_is_mature(&name, age);
        let silk = held.enchantment_level("minecraft:silk_touch") > 0;
        let settings = Settings::default();
        let tool: &str = &held.item;
        let mut draws = Draws(&mut self.world.random);
        let mut extra = engine::crop_drops(&settings, tool, &name, mature, &mut draws);
        extra.extend(engine::ore_drops(&settings, tool, &name, silk, &mut draws));
        for e in extra {
            drops.push(ItemStack::new(e.item, e.count as u32));
        }
        drops
    }

    /// `MobDropsLootModifier`: a player kills a creature with one of the swords.
    /// Called as the mob's death is handled (`mob_died`).
    pub(super) fn advancednetherite_mob_drops(&mut self, m: &Mob) {
        // The loot condition killed_by_player (what mob_died calls by_player) and
        // `killer instanceof Player`: the last blow came from the player.
        let by_player = self.last_hurt_by_player.contains_key(&m.id) || m.ai.last_hurt_by_player > 0;
        let by_blow = m
            .last_damage
            .is_some_and(|(d, _)| d.attacker == Some(Who::Player));
        if !by_player || !by_blow || self.remote.is_some() {
            return;
        }
        let Some(weapon) = self.held().map(|s| s.item.clone()) else {
            return;
        };
        let settings = Settings::default();
        let mut draws = Draws(&mut self.world.random);
        let extra = engine::mob_drops(&settings, &weapon, &m.kind.id, &mut draws);
        let pos = [m.position.x, m.position.y + 0.5, m.position.z];
        for e in extra {
            // Mob.dropFromLootTable spawns its stacks like the rest of the loot.
            self.spawn_item(ItemStack::new(e.item, e.count as u32), pos, [0.0, 0.2, 0.0], 10);
        }
    }
}
