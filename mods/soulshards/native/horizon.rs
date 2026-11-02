// Soul Shards inside Horizon's game (nightly 142388e1c). The rules are the engine (src/lib.rs); this part reads
// and changes the world. World logic runs where the world is (the integrated server in single player: the window
// is its guest), never on a remote client; the tooltip runs on the window that draws it.
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("soulshards_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "soulshards";

include!("kit_blocks.rs");
include!("kit_state.rs");

use horizon_client::items::ItemStack;
use horizon_client::mobs::{Mob, Who};
use horizon_core::geometry::{Aabb, Vec3};
use horizon_core::random::RandomSource;
use horizon_world::nbt::{JavaString, Tag};

const SHARD: &str = "soulshards:soul_shard";
const VILE_SWORD: &str = "soulshards:vile_sword";
const SOUL_STEALER: &str = "soulshards:soul_stealer";
/// `#soulshards:no_capture` of the jar (used when the tag is not loaded).
const NO_CAPTURE: [&str; 5] = [
    "minecraft:armor_stand",
    "minecraft:elder_guardian",
    "minecraft:ender_dragon",
    "minecraft:wither",
    "minecraft:player",
];
/// Forge 1.20.1 `#forge:bosses` (26.3 `#c:bosses`, used when the tag is not loaded).
const BOSSES: [&str; 2] = ["minecraft:ender_dragon", "minecraft:wither"];

thread_local! {
    static STATE: std::cell::RefCell<kit_state::PerWorld<engine::State>> =
        const { std::cell::RefCell::new(kit_state::PerWorld::new()) };
}

// ---- the shard's data (`binding` in minecraft:custom_data, like the 1.20.1 stack tag) ----

fn custom_data(stack: &ItemStack) -> Option<&Tag> {
    stack.data().and_then(|d| d.other("minecraft:custom_data"))
}

/// `Binding.fromNBT`: `custom_data.binding` = {bound: "ns:id", owner: [I; 4 ints], kills: int}.
fn binding_of(stack: &ItemStack) -> Option<engine::Binding> {
    let b = custom_data(stack)?.get("binding")?;
    let bound = match b.get("bound") {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    let owner = match b.get("owner") {
        Some(Tag::IntArray(v)) if v.len() == 4 => Some([v[0], v[1], v[2], v[3]]),
        _ => None,
    };
    let kills = match b.get("kills") {
        Some(Tag::Int(n)) => *n,
        Some(Tag::Short(n)) => i32::from(*n),
        Some(Tag::Byte(n)) => i32::from(*n),
        _ => 0,
    };
    Some(engine::Binding { bound, owner, kills })
}

/// `ItemSoulShard.updateBinding`, plus what the 26.3 item model reads: `custom_model_data.strings[0]` =
/// `tier_N` (the 1.20.1 `soulshards:bound` / `soulshards:tier` item properties) and the glint of a full shard
/// (`isFoil`).
fn set_binding(stack: &mut ItemStack, b: &engine::Binding) {
    let mut binding = Tag::compound([("kills", Tag::Int(b.kills))]);
    if let Tag::Compound(c) = &mut binding {
        if let Some(e) = &b.bound {
            c.insert(JavaString::from("bound"), Tag::String(e.as_str().into()));
        }
        if let Some(o) = b.owner {
            c.insert(JavaString::from("owner"), Tag::IntArray(o.to_vec()));
        }
    }
    let mut data = custom_data(stack).cloned().unwrap_or_else(|| Tag::compound(Vec::<(&str, Tag)>::new()));
    if let Tag::Compound(c) = &mut data {
        c.insert(JavaString::from("binding"), binding);
    }
    let d = stack.data_mut();
    d.set_other("minecraft:custom_data", Some(data));
    let model = engine::model_name(&Some(b.clone())).unwrap_or_default();
    d.set_other(
        "minecraft:custom_model_data",
        Some(Tag::compound([("strings", Tag::List(vec![Tag::String(model.as_str().into())]))])),
    );
    d.set_other(
        "minecraft:enchantment_glint_override",
        b.is_full().then_some(Tag::Byte(1)),
    );
}

fn view(stack: &Option<ItemStack>) -> Option<engine::ShardView> {
    let s = stack.as_ref()?;
    Some(engine::ShardView { is_shard: &*s.item == SHARD, binding: if &*s.item == SHARD { binding_of(s) } else { None } })
}

/// The cage's stack as the game's item NBT, in hex (forja's codec): every component kept.
fn encode_stack(s: &ItemStack) -> String {
    use horizon_world::nbt::{Limits, NamedTag, write_named};
    let root = NamedTag { name: JavaString::from(""), tag: horizon_client::save::item_to_nbt(s) };
    let bytes = write_named(&root, Limits::default()).unwrap_or_default();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
    }
    out
}
fn decode_stack(text: &str) -> Option<ItemStack> {
    use horizon_world::nbt::{Limits, read_named};
    let t = text.as_bytes();
    if t.len() % 2 != 0 {
        return None;
    }
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let bytes: Vec<u8> = t.chunks(2).map(|p| Some(hex(p[0])? << 4 | hex(p[1])?)).collect::<Option<_>>()?;
    let (named, _) = read_named(&bytes, Limits::default()).ok()?;
    horizon_client::save::item_from_nbt(&named.tag)
}

struct Live<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::World for Live<'_, '_> {
    fn block(&self, p: engine::Pos) -> Option<String> {
        kit_blocks::name_at(self.game, p).map(|n| n.to_string())
    }
}

impl Game<'_> {
    /// `SoulShardsAPI.isAllowed`: a known entity type outside `#soulshards:no_capture`.
    fn soulshards_allowed(&self, entity: &str) -> bool {
        let known = self.res.mob_types().contains_key(entity) || self.entity_type_exists(entity);
        let denied = match self.res.stations().enchantments.entity_tags.get("soulshards:no_capture") {
            Some(t) => t.contains(entity),
            None => NO_CAPTURE.contains(&entity),
        };
        known && !denied
    }
    fn soulshards_boss(&self, entity: &str) -> bool {
        match self.res.stations().enchantments.entity_tags.get("c:bosses") {
            Some(t) => t.contains(entity),
            None => BOSSES.contains(&entity),
        }
    }
    /// `getPersistentData().getBoolean("cageBorn")`: Forge keeps it in the entity's `ForgeData`; here it lives in
    /// the mob's saved NBT (`persist.raw`), so it survives a save.
    fn soulshards_cage_born(&self, id: u32) -> bool {
        matches!(
            self.persist.raw.get(&id).and_then(|t| t.get("ForgeData")).and_then(|f| f.get("cageBorn")),
            Some(Tag::Byte(b)) if *b != 0
        )
    }
    fn soulshards_mark_cage_born(&mut self, id: u32) {
        let raw = self.persist.raw.entry(id).or_insert_with(|| Tag::compound(Vec::<(&str, Tag)>::new()));
        if let Tag::Compound(c) = raw {
            let forge = c.entry(JavaString::from("ForgeData")).or_insert_with(|| Tag::compound(Vec::<(&str, Tag)>::new()));
            if let Tag::Compound(f) = forge {
                f.insert(JavaString::from("cageBorn"), Tag::Byte(1));
            }
        }
    }

    /// HEAD `creatures::Game::mob_died` (`LivingDeathEvent` + `LivingExperienceDropEvent`).
    pub(super) fn soulshards_mob_died(&mut self, m: &Mob) {
        if self.remote.is_some() {
            return;
        }
        let cage_born = self.soulshards_cage_born(m.id);
        // dropExperience = false: a cage-born mob drops no experience (`dropExperience` cancels the event).
        let no_xp = cage_born && !engine::config::BALANCE_DROP_EXPERIENCE;
        horizon_client::soulshards::NO_XP.store(if no_xp { m.id } else { u32::MAX }, std::sync::atomic::Ordering::Relaxed);
        let entity = m.kind.id.clone();
        let killer = self.last_hurt_by_player.get(&m.id).copied();
        let kill = engine::Kill {
            entity: entity.clone(),
            // `event.getSource().getEntity() instanceof Player`: the killing blow came from a player.
            by_player: killer.is_some() && matches!(m.last_damage, Some((d, _)) if d.attacker == Some(Who::Player)),
            allowed: self.soulshards_allowed(&entity),
            boss: self.soulshards_boss(&entity),
            cage_born,
        };
        if !engine::kill_counts(&kill) {
            return;
        }
        // The killer's inventory: a guest's own when the death comes outside their turn (as `credit_kill`).
        let guest = killer.flatten().filter(|_| self.lan_acting.is_none());
        match guest {
            Some(conn) => {
                let Some(mut host) = self.lan.take() else { return };
                let _ = self.as_guest(&mut host, conn, |g| g.soulshards_take_soul(&entity));
                self.lan = Some(host);
            }
            None => self.soulshards_take_soul(&entity),
        }
    }

    /// The shard part of `onEntityKill`, for the player whose inventory is current.
    fn soulshards_take_soul(&mut self, entity: &str) {
        let hotbar: [Option<engine::ShardView>; 9] = std::array::from_fn(|i| view(&self.inventory[i]));
        let Some(slot) = engine::first_shard(&view(&self.offhand), &hotbar, entity) else {
            return;
        };
        let main = self.inventory[self.selected].as_ref();
        let souls = engine::souls_for_kill(
            main.map_or(0, |s| s.enchantment_level(SOUL_STEALER)),
            main.is_some_and(|s| &*s.item == VILE_SWORD),
        );
        let owner = self.acting_player_uuid();
        let cell = match slot {
            engine::ShardSlot::OffHand => &mut self.offhand,
            engine::ShardSlot::Hotbar(i) => &mut self.inventory[i],
        };
        let Some(stack) = cell.as_mut() else { return };
        let old = binding_of(stack);
        // A stack of blank shards: one is peeled off, bound and given back (`shardItem.split(1)`).
        let mut peeled = None;
        if old.is_none() && stack.count > 1 {
            stack.count -= 1;
            let mut one = stack.clone();
            one.count = 1;
            peeled = Some(one);
        }
        let b = engine::bind_kill(old, entity, owner, souls);
        match peeled {
            Some(mut one) => {
                set_binding(&mut one, &b);
                self.give_or_drop(one);
            }
            None => set_binding(stack, &b),
        }
    }

    /// HEAD `copper_golems::Game::use_golem_statue`: the multiblock (`onBlockInteract`), the shard on a spawner or
    /// a cage (`ItemSoulShard.onItemUseFirst`) and the shard taken out of a cage (`BlockSoulCage.use`).
    pub(super) fn soulshards_use(&mut self, pos: [i32; 3]) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let Some(block) = kit_blocks::name_at(self, pos).map(|n| n.to_string()) else {
            return false;
        };
        let held = self.held().cloned();
        // The Soul Shard altar: a diamond on the glowstone of the 3x3 pattern.
        if held.as_ref().is_some_and(|h| &*h.item == engine::CATALYST) && engine::is_origin(&block) {
            let Some(blocks) = engine::multiblock_match(&Live { game: self }, pos) else {
                return false;
            };
            for p in blocks {
                let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
                self.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, false));
            }
            let _ = self.process_level_events();
            // `held.shrink(1)` (also in creative, as the event does).
            let cell = self.held_mut();
            if let Some(s) = cell.as_mut() {
                s.count -= 1;
                if s.count == 0 {
                    *cell = None;
                }
            }
            self.give_or_drop(ItemStack::new(SHARD, 1));
            return true;
        }
        if let Some(h) = held.as_ref().filter(|h| &*h.item == SHARD) {
            let Some(binding) = binding_of(h) else {
                return false;
            };
            if block == "minecraft:spawner" {
                return self.soulshards_absorb(pos, binding);
            }
            if engine::is_cage(&block) {
                if binding.bound.is_none() {
                    return true; // InteractionResult.FAIL: nothing else happens
                }
                let empty = STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.level(self, engine::State::load).cages.get(&pos).is_none_or(|c| c.shard.is_none())
                });
                if !empty {
                    return false;
                }
                let hex = encode_stack(h);
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    let cage = s.level(self, engine::State::load).cages.entry(pos).or_default();
                    cage.shard = Some(hex);
                    s.dirty(self);
                });
                // `context.getPlayer().setItemInHand(hand, EMPTY)` and `cage.setState(true)`.
                *self.held_mut() = None;
                self.soulshards_set_active(pos, true);
                return true;
            }
            return false;
        }
        // Sneaking with empty hands on a cage: the shard comes out (`extractItem(0, 1)`).
        if engine::is_cage(&block) && held.is_none() && self.offhand.is_none() && kit_blocks::sneaking(self) {
            let taken = STATE.with(|s| {
                let mut s = s.borrow_mut();
                let cages = &mut s.level(self, engine::State::load).cages;
                let hex = cages.get_mut(&pos).and_then(|c| c.shard.take());
                if hex.is_some() {
                    s.dirty(self);
                }
                hex
            });
            let Some(hex) = taken else {
                return false;
            };
            let Some(mut stack) = decode_stack(&hex) else {
                return false;
            };
            if stack.count > 1 {
                // A stack would stay in the slot minus one (bound shards stack to 1, so this is a safety net).
                let mut rest = stack.clone();
                rest.count -= 1;
                stack.count = 1;
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.level(self, engine::State::load).cages.entry(pos).or_default().shard = Some(encode_stack(&rest));
                    s.dirty(self);
                });
            }
            self.give_or_drop(stack);
            return true;
        }
        false
    }

    /// The shard on a mob spawner of its entity: +200 souls (the pack's `absorptionBonus`), the spawner goes.
    fn soulshards_absorb(&mut self, pos: [i32; 3], mut binding: engine::Binding) -> bool {
        if !engine::config::BALANCE_ALLOW_SPAWNER_ABSORPTION || binding.is_full() {
            return false;
        }
        let shown = match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(horizon_client::containers::BlockEntity::Spawner(sp)) => sp.next_data().and_then(|d| d.id()),
            _ => return true, // InteractionResult.FAIL
        };
        let Some(bound) = binding.bound.clone() else {
            return true;
        };
        if !self.soulshards_allowed(&bound) {
            return false;
        }
        if shown.as_deref() != Some(bound.as_str()) {
            return true; // FAIL
        }
        binding.add_kills(engine::config::BALANCE_ABSORPTION_BONUS);
        if let Some(stack) = self.held_mut().as_mut() {
            set_binding(stack, &binding);
        }
        let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, false));
        let _ = self.process_level_events();
        true
    }

    /// `TileEntitySoulCage.setState`: the block becomes the active or idle cage (`setBlock(.., 3)`).
    fn soulshards_set_active(&mut self, pos: [i32; 3], active: bool) {
        let name = if active { engine::SOUL_CAGE_ACTIVE } else { engine::SOUL_CAGE };
        if kit_blocks::name_at(self, pos).as_deref() == Some(name) {
            return;
        }
        let Some(state) = self.pack.assets.registry.default_state(name) else {
            return;
        };
        let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, state, 3));
        let _ = self.process_level_events();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(c) = s.level(self, engine::State::load).cages.get_mut(&pos) {
                c.active = active;
            }
            s.dirty(self);
        });
    }

    /// HEAD `block_entity_ticks::Game::tick_block_entities`: every cage of this dimension, like its block entity
    /// ticker. A cage whose block is gone drops its shard (`BlockSoulCage.onRemove`), on any removal.
    pub(super) fn soulshards_tick(&mut self) {
        if self.remote.is_some() || kit_state::dir(self).is_none() {
            return;
        }
        let cages: Vec<(engine::Pos, engine::Cage)> = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.level(self, engine::State::load).cages.iter().map(|(p, c)| (*p, c.clone())).collect()
        });
        for (p, mut cage) in cages {
            let Some(name) = kit_blocks::name_at(self, p).map(|n| n.to_string()) else {
                continue; // not loaded: nothing changes there
            };
            if !engine::is_cage(&name) {
                if let Some(stack) = cage.shard.as_deref().and_then(decode_stack) {
                    let at = [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5];
                    self.spawn_item(stack, at, super::dispenser::new_item_motion(), 10);
                }
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.level(self, engine::State::load).cages.remove(&p);
                    s.dirty(self);
                });
                continue;
            }
            if !self.tickets.levels().block_ticking_at(p[0], p[2]) {
                continue;
            }
            let binding = cage
                .shard
                .as_deref()
                .and_then(decode_stack)
                .filter(|s| &*s.item == SHARD)
                .and_then(|s| binding_of(&s))
                .filter(|b| b.bound.is_some());
            let check = engine::CageCheck {
                allowed: binding.as_ref().and_then(|b| b.bound.as_deref()).is_some_and(|e| self.soulshards_allowed(e)),
                powered: {
                    let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
                    self.with_level(|b, level, ticks| b.has_neighbor_signal(level, ticks, at))
                },
                player_near: {
                    let (x, y, z) = (f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
                    self.level_players().iter().any(|pl| {
                        !pl.spectator
                            && (pl.position.x - x).powi(2) + (pl.position.y - y).powi(2) + (pl.position.z - z).powi(2)
                                <= 16.0 * 16.0
                    })
                },
                owner_online: true,
                binding: binding.clone(),
            };
            let was_active = name == engine::SOUL_CAGE_ACTIVE;
            cage.active = was_active || cage.active;
            let t = engine::cage_tick(&mut cage, &check);
            let keep = cage.shard.is_some() || cage.active;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                let cages = &mut s.level(self, engine::State::load).cages;
                if keep {
                    cages.insert(p, cage.clone());
                } else {
                    cages.remove(&p);
                }
                s.dirty(self);
            });
            if let Some(active) = t.set_active {
                self.soulshards_set_active(p, active);
            }
            if t.spawn
                && let Some(b) = binding
            {
                self.soulshards_spawn(p, &b);
            }
        }
    }

    /// `TileEntitySoulCage.spawnEntities` (1.3.6): `spawnAmount` mobs, up to 5 tries each, around the cage.
    fn soulshards_spawn(&mut self, cage: engine::Pos, b: &engine::Binding) {
        let Some(kind) = b.bound.clone() else { return };
        let Some(t) = self.res.mob_types().get(&kind).cloned() else {
            return; // ForgeRegistries.ENTITY_TYPES.getValue == null
        };
        let tier = b.tier();
        let block_light = self.world.light(cage[0], cage[1], cage[2]) & 15;
        for _ in 0..tier.spawn_amount {
            for _attempt in 0..5 {
                let r = &mut self.world.random;
                let (d1, d2) = (r.next_double(), r.next_double());
                let n = r.next_int_bound(3).unwrap_or(0);
                let (d3, d4) = (r.next_double(), r.next_double());
                let p = engine::spawn_pos(cage, d1, d2, n, d3, d4);
                if tier.check_light && !engine::light_ok(block_light) {
                    continue;
                }
                let at = [f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 0.5];
                let half = t.width / 2.0;
                let bounds = Aabb::new(
                    Vec3::new(at[0] - half, at[1], at[2] - half),
                    Vec3::new(at[0] + half, at[1] + t.height, at[2] + half),
                );
                if self.soulshards_cap_reached(cage, &kind) {
                    continue;
                }
                // `level.isUnobstructed(entity)`: no living entity (mob or player) in its box.
                let blocked = self.mobs.iter().any(|m| !m.removed && m.bounds().intersects(bounds))
                    || self.level_players().iter().any(|pl| pl.alive && !pl.spectator && pl.bounds.intersects(bounds));
                if blocked {
                    continue;
                }
                if !engine::config::BALANCE_ALLOW_BOSS_SPAWNS && BOSSES.contains(&kind.as_str()) {
                    continue; // !canChangeDimensions
                }
                if let Some(id) = self.spawn_mob(&kind, at, false) {
                    self.soulshards_mark_cage_born(id);
                }
                break;
            }
        }
    }

    /// `hasReachedSpawnCap`: cage-born mobs of the same type within 16 blocks of the cage, at the pack's cap (32).
    fn soulshards_cap_reached(&self, cage: engine::Pos, kind: &str) -> bool {
        let lo = Vec3::new(f64::from(cage[0] - 16), f64::from(cage[1] - 16), f64::from(cage[2] - 16));
        let hi = Vec3::new(f64::from(cage[0] + 16), f64::from(cage[1] + 16), f64::from(cage[2] + 16));
        let area = Aabb::new(lo, hi);
        let n = self
            .mobs
            .iter()
            .filter(|m| !m.removed && m.kind.id == kind && m.bounds().intersects(area) && self.soulshards_cage_born(m.id))
            .count();
        n >= engine::config::BALANCE_SPAWN_CAP
    }

    /// RETURN `stations::Game::stack_tooltip` (`ItemSoulShard.appendHoverText`): the lines after the name.
    pub(super) fn soulshards_tooltip(
        &self,
        stack: &ItemStack,
        mut lines: Vec<(horizon_client::text::Text, u32)>,
    ) -> Vec<(horizon_client::text::Text, u32)> {
        if &*stack.item != SHARD {
            return lines;
        }
        let Some(b) = binding_of(stack) else {
            return lines;
        };
        let exists = b.bound.as_deref().is_some_and(|e| self.res.mob_types().contains_key(e) || self.entity_type_exists(e));
        let advanced = self.settings.file.bool("advancedItemTooltips");
        let at = lines.len().min(1);
        for (i, (key, arg)) in engine::tooltip(&b, exists, advanced, engine::uuid_text).into_iter().enumerate() {
            let pattern = self.pack.names.display_get(key).unwrap_or_else(|| key.to_owned());
            let text = engine::fill(&pattern, &[arg]);
            lines.insert(at + i, (horizon_client::text::Text::legacy(&text), u32::MAX));
        }
        lines
    }

    /// HEAD `save_game_metadata`: the cages are saved with the world.
    pub(super) fn soulshards_save(&mut self) {
        let game: &Game<'_> = self;
        STATE.with(|s| s.borrow_mut().save(game, engine::State::save));
    }
    /// HEAD `load_world_state` (no bindings): a world opens; its cages are read again on first use.
    pub(super) fn soulshards_world_opened() {
        STATE.with(|s| s.borrow_mut().clear());
    }
}
