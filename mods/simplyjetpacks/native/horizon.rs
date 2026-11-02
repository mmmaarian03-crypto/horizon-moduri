// Simply Jetpacks 2 inside Horizon's game (nightly 142388e1c). The rules are the engine (src/lib.rs).
//
// Where the original's code runs here (Horizon #1514: single player is an integrated server, the window its guest):
// - the side that moves the player (the window's client, or a host playing in the frame) runs `flyUser` right before
//   the player's movement tick (`sync_player_body`, as `Player.aiStep` runs `inventory.tick()` before `travel`): the
//   vertical speed, the sideways pushes, the particles. The client's copy of the jetpack is only read.
// - the server runs `flyUser` for every guest in its tick (`tick_guests`) with the guest's own input flags
//   (`ServerboundPlayerInputPacket`): the energy, no fall distance while the jetpack pushes, the emergency hover, the
//   potato's launch and explosion. A host in the frame does both in `sync_player_body`.
// - the keys (`KeybindForgeBusHandler`) are read on the window (`native_key`) and go to the server on foundry's
//   channel `simplyjetpacks:key` (the original's toggle packets); the server changes the worn jetpack and answers on
//   the action bar.
// - the HUD (`HUDHandler`) is drawn on the window from its copy of the jetpack (foundry syncs `custom_data`).
#[allow(unused_imports)]
use super::{Game, Result, Screen, lan::LanHost};
#[allow(dead_code)]
mod engine {
    include!("simplyjetpacks_engine.rs");
}
use horizon_client::items::ItemStack;
use horizon_core::random::RandomSource;
use horizon_world::nbt::{JavaString, Tag};
use std::cell::RefCell;
use std::collections::HashSet;
use winit::keyboard::KeyCode;

include!("kit_notice.rs");

pub(super) const ID: &str = "simplyjetpacks";
pub(super) const TITLE: &str = "Simply Jetpacks 2";
pub(super) const DESCRIPTION: &str = "Jetpacks with energy, hover and emergency hover (port of Simply Jetpacks 2 1.20.1).";

/// `PacketToggleEngine` ... `PacketUpdateThrottle`: the action's code.
const KEY: super::foundry::Channel<u32> = super::foundry::Channel::new("simplyjetpacks:key");

thread_local! {
    /// The guests whose jetpack pushed in this server tick (no fall damage for them).
    static PUSHED: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
}

fn register() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::foundry::register_server_channel(KEY.id, key_from_client);
}

/// A guest's client pressed a jetpack key: the server acts on the guest's worn jetpack, in its turn.
fn key_from_client(game: &mut Game<'_>, host: &mut LanHost, conn: u32, body: &[u8]) {
    let Some(action) = KEY.decode(body).and_then(|c| engine::Action::from_code(c as u8)) else {
        return;
    };
    game.as_guest(host, conn, |g| g.simplyjetpacks_act(action));
}

// ---- the jetpack's NBT in custom_data (energy = foundry:energy, mods/foundry/API.md section 6)

fn custom(s: &ItemStack) -> Option<&Tag> {
    s.data().and_then(|d| d.other("minecraft:custom_data"))
}
fn get_num(s: &ItemStack, key: &str) -> Option<i64> {
    match custom(s)?.get(key)? {
        Tag::Byte(v) => Some(i64::from(*v)),
        Tag::Short(v) => Some(i64::from(*v)),
        Tag::Int(v) => Some(i64::from(*v)),
        Tag::Long(v) => Some(*v),
        _ => None,
    }
}
fn read_pack(s: &ItemStack) -> engine::Pack {
    let flag = |k: &str| get_num(s, k).is_some_and(|v| v != 0);
    engine::Pack {
        engine: flag("Engine"),
        hover: flag("Hover"),
        ehover: flag("EmergencyHover"),
        charger: flag("Charger"),
        energy: get_num(s, "foundry:energy").unwrap_or(0).max(0),
        throttle: get_num(s, "Throttle").map(|v| v as i32),
        particle: get_num(s, "ParticleID").map(|v| v as i32),
        fired: flag("Fired"),
        timer_set: flag("RocketTimerSet"),
        timer: get_num(s, "RocketTimer").unwrap_or(0) as i32,
    }
}
fn write_pack(s: &mut ItemStack, before: &engine::Pack, p: &engine::Pack) {
    if before == p {
        return;
    }
    let mut map = match custom(s) {
        Some(Tag::Compound(m)) => m.clone(),
        _ => Default::default(),
    };
    let mut put = |k: &str, v: Option<Tag>| {
        map.remove(&JavaString::from(k));
        if let Some(v) = v {
            map.insert(JavaString::from(k), v);
        }
    };
    let byte = |b: bool| Some(Tag::Byte(i8::from(b)));
    put("Engine", byte(p.engine));
    put("Hover", byte(p.hover));
    put("EmergencyHover", byte(p.ehover));
    put("Charger", byte(p.charger));
    put("foundry:energy", Some(Tag::Int(p.energy.clamp(0, i64::from(i32::MAX)) as i32)));
    put("Throttle", p.throttle.map(Tag::Int));
    put("ParticleID", p.particle.map(Tag::Int));
    if p.fired || p.timer_set || before.timer_set {
        put("Fired", byte(p.fired));
        put("RocketTimerSet", byte(p.timer_set));
        put("RocketTimer", Some(Tag::Int(p.timer)));
    }
    s.data_mut().set_other("minecraft:custom_data", Some(Tag::Compound(map)));
    s.tidy();
}

fn input_flags(flags: u8, sprinting: bool, sneaking: bool) -> engine::Input {
    engine::Input {
        forwards: flags & 1 != 0,
        backwards: flags & 2 != 0,
        left: flags & 4 != 0,
        right: flags & 8 != 0,
        up: flags & 16 != 0,
        down: flags & 32 != 0,
        inverted: engine::config::INVERT_HOVER_SNEAKING,
        sprinting,
        crouching: sneaking,
        ..engine::Input::default()
    }
}

/// One tick of `inventoryTick` / `onArmorTick` of the worn jetpack: the flight steps and what they did.
struct Step {
    flies: Vec<engine::Fly>,
    launched: bool,
    explodes: bool,
}
fn tick_pack(kind: &engine::Kind, pack: &mut engine::Pack, input: &engine::Input, fuel: u32) -> Step {
    let mut step = Step { flies: Vec::new(), launched: false, explodes: false };
    if kind.potato() {
        // PotatoJetpackItem: inventoryTick calls flyUser(false), onArmorTick flyUser(true), both each tick.
        for force in [false, true] {
            let s = engine::potato(kind, pack, input, force);
            step.launched |= s.launched;
            step.explodes |= s.explodes;
            step.flies.push(s.fly);
            if step.explodes {
                break;
            }
        }
    } else {
        step.flies.push(engine::fly(kind, pack, input, false, fuel));
    }
    step
}

impl Game<'_> {
    fn sj_text(&self, key: &str, args: &[String]) -> String {
        let pattern = self.pack.names.display_get(key).unwrap_or_else(|| key.to_owned());
        engine::fill(&pattern, args)
    }
    /// `SJTextUtil.getStateToggle`: "Jetpack Engine: Enabled" in green / "Disabled" in red.
    fn sj_state_line(&self, key: &str, on: bool) -> String {
        let state = if on {
            format!("§a{}§r", self.sj_text("misc.simplyjetpacks.enabled", &[]))
        } else {
            format!("§c{}§r", self.sj_text("misc.simplyjetpacks.disabled", &[]))
        };
        self.sj_text(key, &[state])
    }

    /// A key packet on the server (or the in-frame host's own key): the worn jetpack of the player in place.
    pub(super) fn simplyjetpacks_act(&mut self, action: engine::Action) {
        let Some(stack) = self.armor[1].clone() else {
            return;
        };
        let Some(kind) = engine::kind(&stack.item) else {
            return;
        };
        let before = read_pack(&stack);
        let mut pack = before.clone();
        let toggled = engine::apply(kind, &mut pack, action);
        if let Some(s) = self.armor[1].as_mut() {
            write_pack(s, &before, &pack);
        }
        if let Some(t) = toggled {
            let line = self.sj_state_line(t.key, t.on);
            kit_notice::action_bar(self, line);
        }
    }

    /// HEAD on `sync_player_body`: the player this game moves (the window's own, or the in-frame host's), right
    /// before its movement tick.
    pub(super) fn simplyjetpacks_before_move(&mut self) {
        if self.lan_acting.is_some() {
            return;
        }
        let Some(stack) = self.armor[1].clone() else {
            return;
        };
        let Some(kind) = engine::kind(&stack.item) else {
            return;
        };
        let spectator = self.spectator();
        let playing = self.screen == Screen::Playing && self.sleeping.is_none();
        let held = |k| playing && self.keys.contains(&k);
        let (up, down) = (held(KeyCode::Space), held(KeyCode::ShiftLeft));
        let (fw, bw, l, r) = (held(KeyCode::KeyW), held(KeyCode::KeyS), held(KeyCode::KeyA), held(KeyCode::KeyD));
        let Some(p) = self.player.as_ref() else {
            return;
        };
        if spectator || p.flying {
            return;
        }
        let input = engine::Input {
            up,
            down,
            forwards: fw,
            backwards: bw,
            left: l,
            right: r,
            inverted: engine::config::INVERT_HOVER_SNEAKING,
            sprinting: p.sprinting,
            crouching: down,
            in_water: p.fluids.was_touching_water,
            on_ground: p.ground.on_ground,
            vy: p.velocity.y,
        };
        let authority = self.remote.is_none();
        let before = read_pack(&stack);
        let mut pack = before.clone();
        let fuel = stack.enchantment_level("simplyjetpacks:fuel_efficiency");
        let step = tick_pack(kind, &mut pack, &input, fuel);
        let yaw = self.yaw;
        let mut pushed = false;
        if let Some(p) = self.player.as_mut() {
            for f in &step.flies {
                if let Some(vy) = f.vy {
                    p.velocity.y = vy;
                }
                for push in &f.pushes {
                    let w = engine::move_relative(*push, yaw);
                    p.velocity.x += w[0];
                    p.velocity.z += w[1];
                }
                pushed |= f.pushed;
            }
            if pushed && authority {
                p.fall_distance = 0.0;
            }
        }
        if self.native_window() && engine::config::ENABLE_PARTICLES {
            let flying = engine::is_flying(kind, &pack, up, input.on_ground);
            if flying {
                self.sj_particles(kind, &pack, down, input.in_water);
            }
        }
        if !authority {
            return;
        }
        // The server's half for the in-frame host.
        if let Some(p) = self.player.as_ref() {
            let danger = engine::Danger {
                y: p.position.y,
                fall_distance: p.fall_distance,
                health: f64::from(p.health.health()),
                creative_mode: self.creative,
                on_ground: p.ground.on_ground,
                in_water: p.fluids.was_touching_water,
            };
            if engine::emergency(kind, &pack, &danger) {
                engine::do_emergency(&mut pack);
                let line = format!("§c{}§r", self.sj_text("chat.simplyjetpacks.itemJetpack.emergencyHoverModeActivated", &[]));
                kit_notice::action_bar(self, line);
            }
        }
        if let Some(s) = self.armor[1].as_mut() {
            write_pack(s, &before, &pack);
        }
        if step.explodes {
            self.sj_potato_explodes();
        }
    }

    /// `PotatoJetpackItem.flyUser` out of energy: the jetpack is gone, an explosion of 4 that breaks no block, a baked
    /// potato and 100 damage (`DEATH_BY_POTATO_JETPACK` / `DEATH_BY_JETPACK_EXPLOSION`).
    fn sj_potato_explodes(&mut self) {
        self.armor[1] = None;
        let Some(at) = self.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]) else {
            return;
        };
        let _ = self.server_explode(at, 4.0, super::explosion::BlockInteraction::Keep, false);
        self.player_drop(ItemStack::new("minecraft:baked_potato", 1));
        let ty = if self.world.random.next_float() < 0.5 {
            "simplyjetpacks:death_by_potato_jetpack"
        } else {
            "simplyjetpacks:death_by_jetpack_explosion"
        };
        self.hurt_player_by(ty, 100.0);
    }

    /// `ClientJetpackHandler.showJetpackParticles` (this window only).
    fn sj_particles(&mut self, kind: &engine::Kind, pack: &engine::Pack, sneaking: bool, in_water: bool) {
        use horizon_client::particles::{ParticleOptions, ParticleType};
        let ty = if in_water {
            Some(ParticleType::Bubble)
        } else {
            match pack.particle(kind) {
                engine::Particle::None | engine::Particle::Hearts => None,
                engine::Particle::Flame => Some(ParticleType::Flame),
                engine::Particle::Smoke => Some(ParticleType::Smoke),
                engine::Particle::Bubbles => Some(ParticleType::Bubble),
                engine::Particle::Soul => Some(ParticleType::SoulFireFlame),
                engine::Particle::Snow => Some(ParticleType::Snowflake),
                // The mod's own `rainbow` particle type: the vanilla flame stands in for it.
                engine::Particle::Rainbow => Some(ParticleType::Flame),
            }
        };
        let Some(ty) = ty else {
            return;
        };
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let base = [p.position.x, p.position.y + 1.5, p.position.z];
        let v = [p.velocity.x, p.velocity.y, p.velocity.z];
        let yaw = self.yaw;
        let rnd = &mut self.world.random;
        let spread = f64::from((rnd.next_float() - 0.5) * 0.1);
        let centre = f64::from((rnd.next_float() - 0.5) * 0.25);
        let options = ParticleOptions::simple(ty);
        for n in engine::nozzles(sneaking, centre, false) {
            let r = engine::rotate_yaw(n, yaw);
            let at = [base[0] + r[0] + v[0], base[1] + r[1] + v[1], base[2] + r[2] + v[2]];
            self.particles.add(&options, at, [spread, -0.2, spread]);
        }
    }

    /// HEAD on `tick_guests` (the server's tick of its guests; single player's window is one): `flyUser` for each
    /// guest wearing a jetpack, with its input flags.
    pub(super) fn simplyjetpacks_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        register();
        PUSHED.with(|s| s.borrow_mut().clear());
        let conns: Vec<u32> = host.hub.guests.iter().map(|g| g.conn).collect();
        for conn in conns {
            let mut message = None;
            let mut explodes = false;
            {
                let Some(g) = host.hub.guest_mut(conn) else {
                    continue;
                };
                let Some(stack) = g.armor[1].clone() else {
                    continue;
                };
                let Some(kind) = engine::kind(&stack.item) else {
                    continue;
                };
                if g.game_mode == horizon_client::lan::guest::SPECTATOR || g.flying || g.health <= 0.0 {
                    continue;
                }
                let mut input = input_flags(g.input, g.sprinting, g.sneaking);
                input.on_ground = g.on_ground;
                let mut fall = 0.0;
                if let Some(m) = g.motion.as_ref() {
                    input.in_water = m.fluids.was_touching_water;
                    input.vy = m.velocity.y;
                    fall = m.fall_distance;
                }
                let before = read_pack(&stack);
                let mut pack = before.clone();
                let fuel = stack.enchantment_level("simplyjetpacks:fuel_efficiency");
                let step = tick_pack(kind, &mut pack, &input, fuel);
                if step.flies.iter().any(|f| f.pushed) {
                    if let Some(m) = g.motion.as_mut() {
                        m.fall_distance = 0.0;
                    }
                    fall = 0.0;
                    PUSHED.with(|s| s.borrow_mut().insert(conn));
                }
                let danger = engine::Danger {
                    y: g.position[1],
                    fall_distance: fall,
                    health: f64::from(g.health),
                    creative_mode: g.creative(),
                    on_ground: g.on_ground,
                    in_water: input.in_water,
                };
                if engine::emergency(kind, &pack, &danger) {
                    engine::do_emergency(&mut pack);
                    message = Some("chat.simplyjetpacks.itemJetpack.emergencyHoverModeActivated");
                }
                if let Some(s) = g.armor[1].as_mut() {
                    write_pack(s, &before, &pack);
                }
                explodes = step.explodes;
            }
            if message.is_some() || explodes {
                self.as_guest(host, conn, |game| {
                    if let Some(key) = message {
                        let line = format!("§c{}§r", game.sj_text(key, &[]));
                        kit_notice::action_bar(game, line);
                    }
                    if explodes {
                        game.sj_potato_explodes();
                    }
                });
            }
        }
    }

    /// HEAD on `guest_glide_tick` (a guest's move, before its fall damage): a jetpack that pushed this tick keeps
    /// the fall distance at 0 (`player.fallDistance = 0` in `flyUser`).
    pub(super) fn simplyjetpacks_guest_fall(&mut self, host: &mut LanHost, conn: u32) {
        if !PUSHED.with(|s| s.borrow().contains(&conn)) {
            return;
        }
        if let Some(m) = host.hub.guest_mut(conn).and_then(|g| g.motion.as_mut()) {
            m.fall_distance = 0.0;
        }
    }

    /// HEAD on `native_key`: the jetpack keys of the window (`KeybindForgeBusHandler.onKeyInput`: pressed, no
    /// screen open, a jetpack worn).
    pub(super) fn simplyjetpacks_key(&mut self, key: &str, pressed: bool) {
        if !pressed || !self.native_window() || self.screen != Screen::Playing || !self.native_on(ID) {
            return;
        }
        if !self.armor[1].as_ref().is_some_and(|s| engine::kind(&s.item).is_some()) {
            return;
        }
        for action in engine::ACTIONS {
            let (name, _, default) = action.key();
            let bound = self.native_mod_key(ID, name, default);
            if bound == "key.keyboard.unknown" || bound != key {
                continue;
            }
            if self.remote.is_some() {
                let _ = super::foundry::send_to_server(self, KEY.id, &KEY.encode(&u32::from(action.code())));
            } else {
                self.simplyjetpacks_act(action);
            }
        }
    }

    /// HEAD on `draw_boss_bars` (hidden with F1): `HUDHandler`'s lines at the top left.
    pub(super) fn simplyjetpacks_hud(&self, batch: &mut horizon_client::hud::HudBatch) {
        if !engine::config::ENABLE_HUD || !self.native_on(ID) || self.player.is_none() {
            return;
        }
        let Some(stack) = self.armor[1].as_ref() else {
            return;
        };
        let Some(kind) = engine::kind(&stack.item) else {
            return;
        };
        let pack = read_pack(stack);
        let tr = |key: &str, args: &[String]| self.sj_text(key, args);
        let lines = engine::hud_lines(kind, &pack, &tr);
        let c = engine::config::HUD_TEXT_COLOR;
        let color = horizon_client::hud::rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, 255);
        let scale = engine::config::HUD_SCALE.max(1) as f32;
        for (i, line) in lines.iter().enumerate() {
            let x = (2 + engine::config::HUD_X_OFFSET) as f32 * scale;
            let y = (2 + engine::config::HUD_Y_OFFSET + 9 * i as i32) as f32 * scale;
            batch.text(&self.gui.atlas, line, x, y, color, engine::config::HUD_TEXT_SHADOW);
        }
    }
}

/// Options > Mods: the mod and its keys (`KeybindModBusHandler`; the GUI key K has no screen here).
pub(super) fn simplyjetpacks_keys() -> Vec<[String; 3]> {
    let titles = [
        "Toggle Jetpack Engine",
        "Toggle Jetpack Hover",
        "Toggle Jetpack Emergency Hover",
        "Toggle Jetpack Charger",
        "Increase Jetpack Throttle",
        "Decrease Jetpack Throttle",
    ];
    engine::ACTIONS
        .iter()
        .zip(titles)
        .map(|(a, t)| {
            let (name, _, default) = a.key();
            [name.to_owned(), t.to_owned(), default.to_owned()]
        })
        .collect()
}
