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
    include!("netherending_engine.rs");
}
use horizon_client::mobs::{Species, Who};

include!("kit_blocks.rs");
include!("kit_notice.rs");
include!("kit_sound.rs");

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
