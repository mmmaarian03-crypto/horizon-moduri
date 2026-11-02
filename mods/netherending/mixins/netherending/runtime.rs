// Netherending Ores inside Horizon's game (nightly 142388e1c). The rules are the engine below
// (src/lib.rs): the break event of the original (`NetherendingOres.onBreak`) and the unstable Nether
// ores' delayed explosion (`onServerTick`). The ores in the End are another module of this mod, in
// the generator (native/worldgen.rs). The blocks themselves are content (content/mods/netherending).
//
// Where it runs: the break event and the explosions are server side (the original's events run on the
// logical server). Since Horizon #1514 single player is an integrated server and the window is one of
// its guests, so the break runs inside the guest's turn (`as_guest`: the mode, the held tool and the
// level are the guest's) and the mobs it angers are bound to that guest (`LanHost::mob_players`, the
// player a mob sees, game/lan_mobs.rs). A server's client (`remote`) does nothing.
#[allow(unused_imports)]
use super::{Game, Result, lan::LanHost};
#[allow(dead_code)]
mod engine {
    // Netherending Ores for Horizon: the rules, without any game code.
// Written from the source of the mod (the author's own 1.20.1 rebuild, `Downloads/netherending-1201`,
// `NetherendingOres.java` + the `data/` of the jar, licence MIT) and his `netherending-common.toml`.
// This file is pasted into the game twice: as `crate::game::netherending::engine` (the break event)
// and as `horizon_world::worldgen::generator::netherending::engine` (the End ores in the biomes);
// it is also the `horizon_netherending` crate the tests use: std + serde_json, no game types, no
// inner attributes (`#![...]`).
pub type Pos = [i32; 3];

/// The values of the pack's `config/netherending-common.toml` (all defaults of the mod).
#[allow(dead_code)]
pub mod config {
    /// Chance that nearby endermen get angry when you break an End ore (0..1) Range: 0.0 ~ 1.0
    pub const END_ENDERMAN_ANGER_CHANCE: f64 = 0.35; // netherending-common.toml:5 [end] endermanAngerChance
    /// Radius in which endermen hear the broken ore Range: 0 ~ 64
    pub const END_ENDERMAN_RANGE: i32 = 16; // netherending-common.toml:8 [end] endermanRange
    /// Chance that an endermite comes out of the ore Range: 0.0 ~ 1.0
    pub const END_ENDERMITE_CHANCE: f64 = 0.05; // netherending-common.toml:11 [end] endermiteChance
    /// Radius in which zombified piglins get angry when you break a Nether ore (0 = off) Range: 0 ~ 64
    pub const NETHER_PIGMEN_RANGE: i32 = 24; // netherending-common.toml:16 [nether] pigmenRange
    /// Chance that a Nether ore explodes after it is broken (without Silk Touch) Range: 0.0 ~ 1.0
    pub const NETHER_EXPLODE_CHANCE: f64 = 0.03; // netherending-common.toml:19 [nether] explodeChance
    /// The explosion's power (TNT = 4) Range: 0.0 ~ 6.0
    pub const NETHER_EXPLODE_POWER: f64 = 2.0; // netherending-common.toml:22 [nether] explodePower
}

/// `NetherendingOres.ORES`: the metals, in the order of the original.
pub const ORES: [&str; 16] = [
    "coal", "iron", "gold", "copper", "redstone", "lapis", "diamond", "emerald", "tin", "lead", "silver",
    "nickel", "aluminum", "zinc", "osmium", "uranium",
];

/// The delay of the unstable Nether ore: `level.getGameTime() + 25` (NetherendingOres.onBreak).
pub const EXPLOSION_DELAY_TICKS: i64 = 25;

// ------------------------------------------------------------------------------ the break event

/// `level.random` as the break event draws from it.
pub trait Rng {
    /// `RandomSource.nextDouble()`.
    fn next_double(&mut self) -> f64;
    /// `RandomSource.nextFloat()`.
    fn next_float(&mut self) -> f32;
}

/// The three lines the mod shows over the hotbar (`displayClientMessage(.., true)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    Endermen,
    Pigmen,
    Unstable,
}
impl Message {
    /// The translation key and the English text (`assets/netherending/lang/en_us.json`).
    pub fn text(self) -> (&'static str, &'static str) {
        match self {
            Message::Endermen => ("netherending.chat.endermen", "The Endermen felt that..."),
            Message::Pigmen => ("netherending.chat.pigmen", "The pigmen heard you mining!"),
            Message::Unstable => ("netherending.chat.unstable", "The ore is unstable..."),
        }
    }
}

/// What the world does for the event. The adapter implements it on the game.
pub trait Host {
    /// The endermen whose box meets `AABB(center).inflate(range)` get the player as target
    /// (`EnderMan.setTarget`); how many there were.
    fn anger_endermen(&mut self, center: Pos, range: i32) -> usize;
    /// A fresh endermite at `at` facing `yaw`, aiming at the player (`new Endermite`, `moveTo`,
    /// `setTarget`, `addFreshEntity`).
    fn spawn_endermite(&mut self, at: [f64; 3], yaw: f32);
    /// The zombified piglins whose box meets `AABB(center).inflate(range)` get the player as target
    /// and start their persistent anger; how many there were.
    fn anger_pigmen(&mut self, center: Pos, range: i32) -> usize;
    /// `player.displayClientMessage(Component.translatable(..), true)`.
    fn notice(&mut self, message: Message);
    /// `SoundEvents.CREEPER_PRIMED`, `SoundSource.BLOCKS`, volume 1, pitch 0.8, at the block.
    fn creeper_primed(&mut self, pos: Pos);
    /// `pendingExplosions.add(new Pending(level, pos, gameTime + delay))`.
    fn schedule_explosion(&mut self, pos: Pos, delay_ticks: i64);
}

/// What the broken block is to the mod.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    /// `state.is(netherending:end_ores)`.
    EndOre,
    /// `state.is(netherending:nether_ores)`.
    NetherOre,
    /// Anything else.
    Other,
}

/// Who breaks it, and where.
#[derive(Clone, Copy, Debug)]
pub struct Breaker {
    pub creative: bool,
    pub spectator: bool,
    /// `EnchantmentHelper.getItemEnchantmentLevel(SILK_TOUCH, mainHand) > 0`.
    pub silk_touch: bool,
    /// `level.dimension() == Level.NETHER`.
    pub in_nether: bool,
}

/// `NetherendingOres.onBreak` (`BlockEvent.BreakEvent`), line by line.
pub fn on_break(host: &mut impl Host, rng: &mut impl Rng, who: Breaker, pos: Pos, ground: Ground) {
    if who.creative || who.spectator {
        return;
    }
    let silk = who.silk_touch;
    if ground == Ground::EndOre {
        let r = config::END_ENDERMAN_RANGE;
        if r > 0 && rng.next_double() < config::END_ENDERMAN_ANGER_CHANCE && host.anger_endermen(pos, r) > 0 {
            host.notice(Message::Endermen);
        }
        if !silk && rng.next_double() < config::END_ENDERMITE_CHANCE {
            let yaw = rng.next_float() * 360.0;
            host.spawn_endermite([f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5], yaw);
        }
    } else if ground == Ground::NetherOre && who.in_nether {
        let r = config::NETHER_PIGMEN_RANGE;
        if r > 0 && host.anger_pigmen(pos, r) > 0 {
            host.notice(Message::Pigmen);
        }
        if !silk && rng.next_double() < config::NETHER_EXPLODE_CHANCE {
            host.creeper_primed(pos);
            host.notice(Message::Unstable);
            host.schedule_explosion(pos, EXPLOSION_DELAY_TICKS);
        }
    }
}

/// `Level.getEntitiesOfClass(.., new AABB(pos).inflate(r))`: whether an entity box
/// (`min`, `max`) meets the box of the block `pos` grown by `r` (`AABB.intersects`, strict).
pub fn within(pos: Pos, r: i32, min: [f64; 3], max: [f64; 3]) -> bool {
    let r = f64::from(r);
    (0..3).all(|i| {
        let lo = f64::from(pos[i]) - r;
        let hi = f64::from(pos[i]) + 1.0 + r;
        lo < max[i] && hi > min[i]
    })
}

/// The unstable ores waiting to blow (`NetherendingOres.pendingExplosions`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// The dimension (`minecraft:the_nether`).
    pub dim: String,
    pub pos: Pos,
    /// The game time from which it blows.
    pub at: i64,
}

/// `NetherendingOres.onServerTick` (END phase): in list order, every pending explosion whose time has
/// come is handed to `explode` (the center is the block's, `+ 0.5`) and removed. `explode` answers
/// whether the explosion could happen here (the level is the one the player is in); when it cannot,
/// the entry is dropped all the same (the original's level may be gone, the ore is already broken).
pub fn tick(pending: &mut Vec<Pending>, now: i64, mut explode: impl FnMut(&Pending) -> bool) {
    let mut i = 0;
    while i < pending.len() {
        if now >= pending[i].at {
            let p = pending.remove(i);
            explode(&p);
        } else {
            i += 1;
        }
    }
}

/// How late (ticks) an explosion may still blow when its level comes back into place. A server ticks
/// its levels one after the other (Horizon `game/levels.rs`), so an explosion of the Nether must wait
/// for the Nether's own turn and not be dropped by the Overworld's; a level with nobody in it is
/// closed after 300 idle ticks, and an explosion older than this is dropped (its level is gone).
pub const MAX_LATE: i64 = 1200;

/// `onServerTick` for one level of a server that ticks its levels in turn: the explosions of `dim`
/// whose time has come are taken out of the list (in list order) and returned; those of other levels
/// stay for their level's turn, unless they are more than `MAX_LATE` ticks late (dropped).
pub fn take_due(pending: &mut Vec<Pending>, now: i64, dim: &str) -> Vec<Pending> {
    let mut due = Vec::new();
    let mut i = 0;
    while i < pending.len() {
        let p = &pending[i];
        if now >= p.at && (p.dim == dim || now - p.at > MAX_LATE) {
            let p = pending.remove(i);
            if p.dim == dim {
                due.push(p);
            }
        } else {
            i += 1;
        }
    }
    due
}

/// `ServerStoppedEvent`: nothing is kept.
pub fn stopped(pending: &mut Vec<Pending>) {
    pending.clear();
}

// -------------------------------------------------------------------------- the ores in the End

/// The `features` of `netherending:forge/biome_modifier/end_ores.json`, in its order.
pub fn end_features() -> Vec<String> {
    ORES.iter().map(|o| format!("netherending:end_{o}_ore")).collect()
}

/// The generation step `underground_ores` (`GenerationStep.Decoration.UNDERGROUND_ORES.ordinal()`).
pub const UNDERGROUND_ORES: usize = 6;

/// Whether a biome file (the JSON of `worldgen/biomes.json`) is one of `end_highlands`, `end_midlands`,
/// `end_barrens` and `small_end_islands`, the biomes of the original's biome modifier. The game gives
/// the text without the biome's name, so the biome is told by its content: only endermen spawn
/// (`monster: [enderman]`), no rain, and not the central island (`the_end` is the one with
/// `minecraft:end_spike`). In the 26.3 data these are exactly the four.
pub fn is_outer_end_biome(biome: &serde_json::Value) -> bool {
    use serde_json::Value;
    let spawns = &biome["attributes"]["minecraft:gameplay/natural_mob_spawns"]["argument"]["spawns_by_category"];
    let only_endermen = match spawns {
        Value::Object(map) => {
            map.len() == 1
                && map.get("monster").and_then(Value::as_array).is_some_and(|list| {
                    list.len() == 1 && list[0]["type"] == "minecraft:enderman"
                })
        }
        _ => false,
    };
    let no_rain = biome["has_precipitation"] == Value::Bool(false);
    let center = biome["features"]
        .as_array()
        .is_some_and(|steps| steps.iter().any(|s| s.to_string().contains("minecraft:end_spike")));
    only_endermen && no_rain && !center
}

/// Appends `names` to `step` of the biome's `features` (padding the list of steps with empty ones),
/// skipping names already there. `false` when the step is not a list of names (a tag reference).
pub fn add_features(biome: &mut serde_json::Value, step: usize, names: &[String]) -> bool {
    use serde_json::Value;
    let Some(obj) = biome.as_object_mut() else {
        return false;
    };
    let steps = obj.entry("features").or_insert_with(|| Value::Array(Vec::new()));
    let Some(steps) = steps.as_array_mut() else {
        return false;
    };
    while steps.len() <= step {
        steps.push(Value::Array(Vec::new()));
    }
    let Some(list) = steps[step].as_array_mut() else {
        return false;
    };
    for name in names {
        if !list.iter().any(|v| v.as_str() == Some(name.as_str())) {
            list.push(Value::String(name.clone()));
        }
    }
    true
}

/// The biome file with the End ores in the four outer End biomes; `None` for every other biome (or a
/// file that is not JSON): the biome modifier of the original (`forge:add_features`, step
/// `underground_ores`).
pub fn with_end_ores(biome_json: &str) -> Option<String> {
    let mut biome: serde_json::Value = serde_json::from_str(biome_json).ok()?;
    if !is_outer_end_biome(&biome) || !add_features(&mut biome, UNDERGROUND_ORES, &end_features()) {
        return None;
    }
    serde_json::to_string(&biome).ok()
}

}
use horizon_client::mobs::{Species, Who};

// kit_blocks (tools/kit): block names, properties and tags as the finished mods read them
// (fallingtree, comforts, kleeslabs, respawn_anchor.rs on the pin). Pasted by `tools/hnmod.py bundle`
// in place of `include!("kit_blocks.rs");` inside the mod's runtime module `crate::game::<id>`.
// Use: `kit_blocks::name_at(self, pos)`, `kit_blocks::block_in_tag(self, "minecraft:logs", &name)`.
// Tag names are the 26.3 ones (`minecraft:logs`, `c:...`); 1.20 names (`forge:...`) do not exist.
#[allow(dead_code)]
mod kit_blocks {
    use crate::game::Game;
    use horizon_world::registry::BlockStateId;

    /// Inside the world height and in a loaded chunk.
    pub(super) fn loaded(g: &Game<'_>, p: [i32; 3]) -> bool {
        p[1] >= g.dim.min_y() && p[1] <= g.dim.max_y() && g.world.loaded(p[0], p[2])
    }
    /// The state at `p` (air outside the world, as `world.block` answers).
    pub(super) fn state_at(g: &Game<'_>, p: [i32; 3]) -> BlockStateId {
        g.world.block(p[0], p[1], p[2])
    }
    /// The block name of a state (`"minecraft:oak_log"`).
    pub(super) fn name_of(g: &Game<'_>, state: BlockStateId) -> std::sync::Arc<str> {
        g.res.facts().get(state).name.clone()
    }
    /// The block name at `p`; `None` when not loaded or outside the world.
    pub(super) fn name_at(g: &Game<'_>, p: [i32; 3]) -> Option<std::sync::Arc<str>> {
        loaded(g, p).then(|| name_of(g, state_at(g, p)))
    }
    /// A block state property (`facing`, `half`, `age`...).
    pub(super) fn prop(g: &Game<'_>, state: BlockStateId, key: &str) -> Option<String> {
        g.pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get(key).cloned())
    }
    /// The same block with some properties changed; `None` if a value is not valid.
    pub(super) fn with_props(
        g: &Game<'_>,
        state: BlockStateId,
        props: &[(&str, &str)],
    ) -> Option<BlockStateId> {
        g.with_props(state, props)
    }
    /// `state.is(BlockTags.X)`: block tag membership by block name.
    pub(super) fn block_in_tag(g: &Game<'_>, tag: &str, block: &str) -> bool {
        g.res
            .items()
            .block_tags
            .get(tag)
            .is_some_and(|s| s.contains(block))
    }
    /// `stack.is(ItemTags.X)`: item tag membership by item name.
    pub(super) fn item_in_tag(g: &Game<'_>, tag: &str, item: &str) -> bool {
        g.res
            .items()
            .item_tags
            .get(tag)
            .is_some_and(|s| s.contains(item))
    }
    /// The item in the hand that acts (`getItemInHand(hand)`, `Game::held`).
    pub(super) fn held_name(g: &Game<'_>) -> Option<String> {
        g.held().map(|s| s.item.to_string())
    }
    /// The held item is in an item tag (`minecraft:axes`, `minecraft:pickaxes`...).
    pub(super) fn held_in_tag(g: &Game<'_>, tag: &str) -> bool {
        g.held().is_some_and(|s| item_in_tag(g, tag, &s.item))
    }
    /// The player is sneaking (`isShiftKeyDown`), as fallingtree reads it.
    pub(super) fn sneaking(g: &Game<'_>) -> bool {
        g.player.as_ref().is_some_and(|p| p.shift_key_down)
    }
    /// A block of namespace `namespace` (`is_mod_block(name, "mcwwindows")`).
    pub(super) fn is_mod_block(name: &str, namespace: &str) -> bool {
        name.strip_prefix(namespace)
            .is_some_and(|rest| rest.starts_with(':'))
    }
}
// kit_notice (tools/kit): a line over the hotbar (action bar) for whoever acts, as fallingtree
// does: a LAN guest gets a `SystemChat { overlay: true }` packet, the host the game's own
// `message` line. Pasted by `tools/hnmod.py bundle` in place of `include!("kit_notice.rs");`.
// Use: `kit_notice::action_bar(self, "Tree too big".to_owned())`, `kit_notice::chat(self, ..)`.
#[allow(dead_code)]
mod kit_notice {
    use crate::game::Game;
    use horizon_protocol::packets::play::{Clientbound, SystemChat};

    fn send(g: &mut Game<'_>, text: String, overlay: bool) {
        if g.lan_acting.is_some() {
            g.guest_packets.push(Clientbound::SystemChat(SystemChat {
                content: horizon_protocol::text::literal(&text),
                overlay,
            }));
        } else {
            // The host's window has one line over the hotbar for both (see `chat`).
            g.message = Some((text, std::time::Instant::now()));
        }
    }
    /// `player.displayClientMessage(text, true)`: the action bar.
    pub(super) fn action_bar(g: &mut Game<'_>, text: String) {
        send(g, text, true);
    }
    /// `player.displayClientMessage(text, false)`. For a guest this is a chat line; the host
    /// gets the same action-bar line (this pin has no plain host-side chat append without
    /// going through `chat.rs` internals; see TRADUCATOR.md).
    pub(super) fn chat(g: &mut Game<'_>, text: String) {
        send(g, text, false);
    }
}
// kit_sound (tools/kit): sounds, level events and particles at a block. Pasted by
// `tools/hnmod.py bundle` in place of `include!("kit_sound.rs");`.
// - `audio.play` is captured and sent to LAN guests too (`Audio::play` vs `play_unshared`).
// - `level_event_at` queues `Level.levelEvent` (2001 = block break particles + sound, 1000s =
//   sounds); everyone around sees/hears it.
// - `particles.add` is this window only (guests do not see it).
#[allow(dead_code)]
mod kit_sound {
    use crate::game::Game;

    fn center(p: [i32; 3]) -> [f64; 3] {
        [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]) + 0.5,
            f64::from(p[2]) + 0.5,
        ]
    }
    /// `level.playSound(null, pos, SoundEvents.X, SoundSource.BLOCKS, volume, pitch)`.
    pub(super) fn at_block(g: &mut Game<'_>, sound: &str, pos: [i32; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(center(pos)), volume, pitch);
    }
    /// A sound at an exact point (entities, items).
    pub(super) fn at(g: &mut Game<'_>, sound: &str, at: [f64; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(at), volume, pitch);
    }
    /// `level.levelEvent(id, pos, data)`; `2001` with the block state id = break effect.
    pub(super) fn level_event(g: &mut Game<'_>, id: i32, pos: [i32; 3], data: i32) {
        g.level_event_at(id, pos, data);
    }
    /// `level.addParticle(type, x, y, z, dx, dy, dz)` for this window only.
    pub(super) fn particle(
        g: &mut Game<'_>,
        ty: horizon_client::particles::ParticleType,
        at: [f64; 3],
        velocity: [f64; 3],
    ) {
        let options = horizon_client::particles::ParticleOptions::simple(ty);
        g.particles.add(&options, at, velocity);
    }
}

thread_local! {
    /// `NetherendingOres.pendingExplosions`: in memory only, as in the original.
    static PENDING: std::cell::RefCell<Vec<engine::Pending>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Mobs a guest's break set on him, waiting for the hub (`tick_guests` of their level) when it was
    /// not in `self.lan` during the turn: (dimension, mob id, guest connection).
    static BIND: std::cell::RefCell<Vec<(String, u32, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The game as the break event's host: what the original did with `level` and `player`.
struct Live<'a, 'g> {
    game: &'a mut Game<'g>,
    /// The mobs set on the breaker (`setTarget(player)`), bound to him afterwards.
    targeted: Vec<u32>,
}
impl Live<'_, '_> {
    /// The ids of the live mobs of `species` whose box meets the block's search box.
    fn near(&self, species: Species, center: engine::Pos, range: i32) -> Vec<u32> {
        self.game
            .mobs
            .iter()
            .filter(|m| m.species() == species && m.alive())
            .filter(|m| {
                let half = f64::from(m.width()) / 2.0;
                let (p, h) = (m.position, f64::from(m.height()));
                engine::within(
                    center,
                    range,
                    [p.x - half, p.y, p.z - half],
                    [p.x + half, p.y + h, p.z + half],
                )
            })
            .map(|m| m.id)
            .collect()
    }
    fn target_player(&mut self, ids: &[u32]) {
        for m in self.game.mobs.iter_mut().filter(|m| ids.contains(&m.id)) {
            m.ai.target = Some(Who::Player);
        }
        self.targeted.extend_from_slice(ids);
    }
}
impl engine::Host for Live<'_, '_> {
    fn anger_endermen(&mut self, center: engine::Pos, range: i32) -> usize {
        // EnderMan.setTarget(player).
        let ids = self.near(Species::Enderman, center, range);
        self.target_player(&ids);
        ids.len()
    }
    fn spawn_endermite(&mut self, at: [f64; 3], yaw: f32) {
        // new Endermite(..), moveTo(.., yaw, 0), setTarget(player), addFreshEntity: no finalizeSpawn,
        // not persistent (it despawns by its own 2400-tick life).
        if let Some(id) = self.game.spawn_mob("minecraft:endermite", at, false) {
            if let Some(m) = self.game.mobs.iter_mut().find(|m| m.id == id) {
                m.yaw = yaw;
                m.previous_yaw = yaw;
                m.body_yaw = yaw;
                m.previous_body_yaw = yaw;
                m.head_yaw = yaw;
                m.previous_head_yaw = yaw;
                m.head_pitch = 0.0;
                m.previous_head_pitch = 0.0;
                m.ai.target = Some(Who::Player);
            }
            self.targeted.push(id);
        }
    }
    fn anger_pigmen(&mut self, center: engine::Pos, range: i32) -> usize {
        // setTarget + setPersistentAngerTarget + startPersistentAngerTimer: a target starts the 20 to
        // 39 seconds of anger in the zombified piglin's own step (`m.extra.anger = 400 + nextInt(380)`).
        let ids = self.near(Species::ZombifiedPiglin, center, range);
        self.target_player(&ids);
        ids.len()
    }
    fn notice(&mut self, message: engine::Message) {
        let (key, english) = message.text();
        let text = horizon_client::lang::get_or(key, english);
        kit_notice::action_bar(self.game, text);
    }
    fn creeper_primed(&mut self, pos: engine::Pos) {
        kit_sound::at_block(self.game, "minecraft:entity.creeper.primed", pos, 1.0, 0.8);
    }
    fn schedule_explosion(&mut self, pos: engine::Pos, delay_ticks: i64) {
        let pending = engine::Pending {
            dim: self.game.dim.id().to_owned(),
            pos,
            at: self.game.world.game_time + delay_ticks,
        };
        PENDING.with(|p| p.borrow_mut().push(pending));
    }
}

/// `level.random` of the game.
struct LevelRandom<'a>(&'a mut horizon_core::random::LegacyRandom);
impl engine::Rng for LevelRandom<'_> {
    fn next_double(&mut self) -> f64 {
        use horizon_core::random::RandomSource;
        self.0.next_double()
    }
    fn next_float(&mut self) -> f32 {
        use horizon_core::random::RandomSource;
        self.0.next_float()
    }
}

impl Game<'_> {
    /// HEAD on `break_block_as_player`: `BlockEvent.BreakEvent`, before the block is removed. Only a
    /// survival player who may build. It runs where the world is: the in-frame host's game, or a server
    /// inside the guest's turn (single player's window included), where the mode, the held tool and the
    /// level are the breaker's; `Who::Player` is then whoever the mob sees, so the angered mobs are bound
    /// to the breaker (`netherending_bind`). A server's client only sends the dig.
    pub(super) fn netherending_break(&mut self, pos: [i32; 3]) {
        if self.remote.is_some() || self.creative || self.spectator() || !self.may_build() {
            return;
        }
        let Some(name) = kit_blocks::name_at(self, pos) else {
            return;
        };
        let ground = if kit_blocks::block_in_tag(self, "netherending:end_ores", &name) {
            engine::Ground::EndOre
        } else if kit_blocks::block_in_tag(self, "netherending:nether_ores", &name) {
            engine::Ground::NetherOre
        } else {
            return;
        };
        let silk_touch = self
            .held()
            .is_some_and(|s| s.enchantment_level("minecraft:silk_touch") > 0);
        let who = engine::Breaker {
            creative: self.creative,
            spectator: self.spectator(),
            silk_touch,
            in_nether: self.dim.id() == "minecraft:the_nether",
        };
        // The level random is borrowed apart from the host: take a copy of the generator, play the
        // event on it, put it back (the draws the event makes are the level's).
        let mut random = self.world.random.clone();
        let mut live = Live { game: self, targeted: Vec::new() };
        engine::on_break(&mut live, &mut LevelRandom(&mut random), who, pos, ground);
        let targeted = live.targeted;
        self.world.random = random;
        let breaker = self.lan_acting;
        self.netherending_bind(targeted, breaker);
    }

    /// `setTarget(player)` with several players: the mob sees the breaker (`breaker` = the guest's
    /// connection; `None` = the in-frame host, whom a mob sees when it has no guest binding).
    fn netherending_bind(&mut self, ids: Vec<u32>, breaker: Option<u32>) {
        if ids.is_empty() {
            return;
        }
        match (self.lan.as_mut(), breaker) {
            (Some(host), Some(conn)) => {
                for id in ids {
                    host.mob_players.insert(id, conn);
                }
            }
            (Some(host), None) => {
                for id in ids {
                    host.mob_players.remove(&id);
                }
            }
            // During a guest's turn the hub is apart (`as_guest`): bound in `tick_guests` of this level.
            (None, Some(conn)) => {
                let dim = self.dim.id().to_owned();
                BIND.with(|b| b.borrow_mut().extend(ids.into_iter().map(|id| (dim.clone(), id, conn))));
            }
            // No hub, no guests: the one player is the in-frame host.
            (None, None) => {}
        }
    }

    /// HEAD on `tick_guests` (a server's level in place, single player's included): the mobs a guest's
    /// break angered are bound to him (those of other levels wait for their level's turn).
    pub(super) fn netherending_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() || BIND.with(|b| b.borrow().is_empty()) {
            return;
        }
        let dim = self.dim.id().to_owned();
        let mine: Vec<(u32, u32)> = BIND.with(|b| {
            let mut b = b.borrow_mut();
            let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *b).into_iter().partition(|e| e.0 == dim);
            *b = rest;
            mine.into_iter().map(|(_, id, conn)| (id, conn)).collect()
        });
        for (id, conn) in mine {
            if self.mobs.iter().any(|m| m.id == id) && host.hub.guest(conn).is_some() {
                host.mob_players.insert(id, conn);
            }
        }
    }

    /// HEAD on `tick_blocks` (once per game tick of each level): `NetherendingOres.onServerTick`. A server
    /// ticks its levels one after the other: each level blows its own explosions in its turn
    /// (`engine::take_due`); a server's client has none.
    pub(super) fn netherending_tick(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let now = self.world.game_time;
        let dim = self.dim.id();
        let due = PENDING.with(|p| {
            let mut list = p.borrow_mut();
            if !list.iter().any(|e| now >= e.at) {
                return Vec::new();
            }
            engine::take_due(&mut list, now, dim)
        });
        for e in due {
            // The original explodes in the level it was broken in (`take_due` gives this level's only).
            let center = [
                f64::from(e.pos[0]) + 0.5,
                f64::from(e.pos[1]) + 0.5,
                f64::from(e.pos[2]) + 0.5,
            ];
            // Level.explode(null, .., power, false, ExplosionInteraction.BLOCK): block_explosion_drop_decay.
            let interaction = if self.ui.rules.bool("block_explosion_drop_decay") {
                super::explosion::BlockInteraction::DestroyWithDecay
            } else {
                super::explosion::BlockInteraction::Destroy
            };
            let _ = self.server_explode(
                center,
                engine::config::NETHER_EXPLODE_POWER as f32,
                interaction,
                false,
            );
        }
    }

    /// HEAD on `load_world_state`: a world opens (ServerStoppedEvent of the one before).
    pub(super) fn netherending_world_opened() {
        PENDING.with(|p| engine::stopped(&mut p.borrow_mut()));
        BIND.with(|b| b.borrow_mut().clear());
    }
}
