// Hang Glider inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs).
//
// The original keeps a `GlidingCapability` (deployed, gliding) per player, synced from the server; both
// sides run `PlayerGlidingHandler.onPlayerTick$End`. Here:
// - the side that moves the player (the window's client, or a host playing in the frame) keeps its own
//   copy: the use toggles it (`remote_use_item` on a client, `throw_item` in the frame), the end of the
//   player's tick (`fov::tick_avatar`) applies the gliding movement, the wind and the fall distance;
// - the server keeps a copy per guest: the guest's use (`throw_item` in the guest's turn) toggles it, its
//   body tick (`guest_glide_tick`, before the moves are checked for fall damage) keeps the fall distance
//   at 1 and wears the glider. Both copies follow the same rules from the same clicks.
// - the SPIN_ATTACK pose while gliding (`PlayerMixin.updatePlayerPose`) reads a per-thread flag
//   (`horizon_client::motion::hangglider_glide`) in `MotionWorld::update_player_pose`.
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("hangglider_engine.rs");
}

use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_core::random::RandomSource;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Default)]
struct Players {
    /// The player this game moves.
    local: engine::State,
    /// `ElytraEquippedHandler.tickTime`: the elytra-and-cross icon's ticks left (40 on a use).
    elytra_icon: u32,
    /// The server's copies, by the guest's UUID.
    guests: HashMap<u128, engine::State>,
}

thread_local! {
    static PLAYERS: RefCell<Players> = RefCell::new(Players::default());
}

fn stack_of(s: Option<&ItemStack>) -> Option<engine::Stack> {
    s.map(|s| engine::Stack { item: s.item.to_string(), damage: s.damage })
}

fn is_elytra(s: &Option<ItemStack>) -> bool {
    // `instanceof ElytraItem`: the vanilla elytra only.
    s.as_ref().is_some_and(|s| &*s.item == "minecraft:elytra")
}

impl Game<'_> {
    fn hangglider_hands(&self) -> (Option<engine::Stack>, Option<engine::Stack>) {
        let main = self.inventory.get(self.selected).and_then(|s| s.as_ref());
        (stack_of(main), stack_of(self.offhand.as_ref()))
    }

    /// One point of wear on the glider in `hand` (`ItemStack.hurtAndBreak(1, player, ..)`, not in
    /// creative; Unbreaking may save it). A valid glider has at least two points left, so it never breaks.
    fn hangglider_wear(&mut self, hand: engine::Hand, creative: bool) {
        if creative {
            return;
        }
        let i = self.selected;
        let stack = match hand {
            engine::Hand::Main => self.inventory.get(i).and_then(|s| s.clone()),
            engine::Hand::Off => self.offhand.clone(),
        };
        let Some(stack) = stack else {
            return;
        };
        let loss = self.durability_loss(&stack, 1);
        let place = match hand {
            engine::Hand::Main => self.inventory.get_mut(i).and_then(|s| s.as_mut()),
            engine::Hand::Off => self.offhand.as_mut(),
        };
        if let Some(s) = place {
            s.damage += loss;
        }
    }

    /// `GliderItem.use` for the side that owns `state`: what the click does.
    fn hangglider_use_on(state: &mut engine::State, stack: &ItemStack, chest_elytra: bool) -> engine::Use {
        let s = stack_of(Some(stack));
        engine::on_use(state, s.as_ref(), chest_elytra)
    }

    /// HEAD on `creatures::throw_item` (Item.use, after a block's own use): a host in the frame, or a
    /// guest's use on the server. `true` = the glider opened / closed (the hand swings).
    pub(super) fn hangglider_use(&mut self, stack: &ItemStack) -> bool {
        if engine::glider(&stack.item).is_none() || self.remote.is_some() {
            return false;
        }
        let chest_elytra = is_elytra(&self.armor[1]);
        let guest = self.lan_acting_uuid;
        let window = self.native_window();
        let used = PLAYERS.with(|c| {
            let mut c = c.borrow_mut();
            let state = match (self.lan_acting.is_some(), guest) {
                (true, Some(uuid)) => c.guests.entry(uuid).or_default(),
                (true, None) => return engine::Use::Pass,
                (false, _) => &mut c.local,
            };
            let used = Self::hangglider_use_on(state, stack, chest_elytra);
            if used == engine::Use::Elytra && self.lan_acting.is_none() && window {
                c.elytra_icon = 40;
            }
            used
        });
        used == engine::Use::Toggled
    }

    /// HEAD on `remote::remote_use_item` (the window's client, `MultiPlayerGameMode.useItem`, before the
    /// packet goes): the client's copy of the click (the server does the same with the packet).
    pub(super) fn hangglider_remote_use(&mut self, hand: horizon_client::first_person::Hand) {
        let stack = match hand {
            horizon_client::first_person::Hand::Main => self.inventory.get(self.selected).and_then(|s| s.clone()),
            horizon_client::first_person::Hand::Off => self.offhand.clone(),
        };
        let Some(stack) = stack.filter(|s| engine::glider(&s.item).is_some()) else {
            return;
        };
        let chest_elytra = is_elytra(&self.armor[1]);
        PLAYERS.with(|c| {
            let mut c = c.borrow_mut();
            if Self::hangglider_use_on(&mut c.local, &stack, chest_elytra) == engine::Use::Elytra {
                c.elytra_icon = 40;
            }
        });
    }

    /// Whether the main hand's `Item.use` is a glider that will open / close (`sidedSuccess`): then
    /// `Minecraft.startUseItem` ends with the main hand, the off hand gets no turn.
    pub(super) fn hangglider_main_toggles(&self) -> bool {
        let main = self.inventory.get(self.selected).and_then(|s| s.as_ref());
        engine::valid(stack_of(main).as_ref()) && !is_elytra(&self.armor[1])
    }

    /// HEAD on `sync_player_body`: before the moved player's tick, the pose code reads whether he glides.
    pub(super) fn hangglider_before_move(&mut self) {
        if self.lan_acting.is_some() || self.player.is_none() {
            return;
        }
        let gliding = PLAYERS.with(|c| c.borrow().local.gliding);
        horizon_client::motion::hangglider_glide::set(gliding);
    }

    /// HEAD on `fov::tick_avatar`: the end of the moved player's tick (`PlayerTickEvents.END`).
    pub(super) fn hangglider_end_tick(&mut self) {
        if self.lan_acting.is_some() {
            return;
        }
        PLAYERS.with(|c| {
            let mut c = c.borrow_mut();
            c.elytra_icon = c.elytra_icon.saturating_sub(1);
        });
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let player = engine::Player {
            position: [p.position.x, p.position.y, p.position.z],
            velocity: [p.velocity.x, p.velocity.y, p.velocity.z],
            yaw: self.yaw,
            on_ground: p.ground.on_ground,
            passenger: self.riding.is_some(),
            levitation: self.potions.effects.get("levitation").is_some(),
            flying: p.flying,
            in_water: p.fluids.was_touching_water,
            descending: p.shift_key_down,
            raining: self.world.weather.levels(1.0).0 > 0.2,
            chest_elytra: is_elytra(&self.armor[1]),
        };
        let (main, off) = self.hangglider_hands();
        let server = self.remote.is_none();
        let mut state = PLAYERS.with(|c| c.borrow().local);
        let random = &mut self.world.random;
        let out = engine::end_tick(&mut state, main.as_ref(), off.as_ref(), &player, server, |n| {
            random.next_int_bound(n).unwrap_or(1)
        });
        PLAYERS.with(|c| c.borrow_mut().local = state);
        horizon_client::motion::hangglider_glide::set(state.gliding);
        if let Some(yaw) = out.yaw {
            self.yaw = yaw;
        }
        if let Some(p) = self.player.as_mut() {
            p.yaw = self.yaw;
            if let Some(v) = out.velocity {
                p.velocity = horizon_core::geometry::Vec3::new(v[0], v[1], v[2]);
            }
            if out.fall_distance_one {
                p.fall_distance = 1.0;
            }
        }
        if let Some(hand) = out.wear {
            let creative = self.creative;
            self.hangglider_wear(hand, creative);
        }
    }

    /// HEAD on `lan_actions::guest_glide_tick`: the server's tick of a guest's body, before its moves are
    /// checked for fall damage (`fell` = the last move's height change, the server's delta movement).
    pub(super) fn hangglider_guest_tick(&mut self, host: &mut super::lan::LanHost, conn: u32, fell: f64) {
        let Some(g) = host.hub.guest_mut(conn) else {
            return;
        };
        let uuid = g.uuid;
        let known = PLAYERS.with(|c| c.borrow().guests.get(&uuid).copied());
        let Some(mut state) = known.filter(|s| s.deployed || s.gliding) else {
            // Nothing deployed: nothing to do (and the pose code must not see another player's glide).
            horizon_client::motion::hangglider_glide::set(false);
            return;
        };
        let creative = g.creative();
        let base = engine::Player {
            position: g.position,
            velocity: [0.0, fell, 0.0],
            yaw: g.yaw,
            on_ground: g.on_ground,
            passenger: g.vehicle.is_some(),
            flying: g.flying,
            descending: g.sneaking,
            chest_elytra: is_elytra(&g.armor[1]),
            ..Default::default()
        };
        self.as_guest(host, conn, |game| {
            let mut player = base;
            player.levitation = game.potions.effects.get("levitation").is_some();
            player.in_water = game.player.as_ref().is_some_and(|p| p.fluids.was_touching_water);
            player.raining = game.world.weather.levels(1.0).0 > 0.2;
            let (main, off) = game.hangglider_hands();
            let random = &mut game.world.random;
            let out = engine::end_tick(&mut state, main.as_ref(), off.as_ref(), &player, true, |n| {
                random.next_int_bound(n).unwrap_or(1)
            });
            if out.fall_distance_one && let Some(p) = game.player.as_mut() {
                p.fall_distance = 1.0;
            }
            if let Some(hand) = out.wear {
                game.hangglider_wear(hand, creative);
            }
        });
        PLAYERS.with(|c| {
            c.borrow_mut().guests.insert(uuid, state);
        });
        horizon_client::motion::hangglider_glide::set(state.gliding);
    }

    /// RETURN on `extras::zoomed_fov_target` (`ComputeFovModifierCallback`).
    pub(super) fn hangglider_fov(&self, value: f32) -> f32 {
        let state = PLAYERS.with(|c| c.borrow().local);
        let descending = self.player.as_ref().is_some_and(|p| p.shift_key_down);
        engine::fov(&state, descending, value)
    }

    /// HEAD on `titles::draw_selected_name` (the HUD over the hotbar): `ElytraEquippedHandler.onRenderGui`,
    /// an elytra with a blinking red cross under the crosshair after a use with an elytra on.
    pub(super) fn hangglider_hud(&self, batch: &mut HudBatch) {
        let ticks = PLAYERS.with(|c| c.borrow().elytra_icon);
        if ticks == 0 {
            return;
        }
        let left = ((batch.width - 16.0) / 2.0).floor();
        let top = (batch.height / 2.0 + 16.0).floor();
        self.draw_item(batch, &ItemStack::new("minecraft:elytra", 1), left, top);
        let alpha = ((f64::from(ticks) * 0.5).sin() * 0.5 + 0.5) as f32;
        let a = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        let atlas = &self.gui.atlas;
        for &(x, y, w, red) in CROSS {
            let color = horizon_client::hud::rgba(red, 0, 0, a);
            batch.fill(atlas, left + f32::from(x), top + f32::from(y), f32::from(w), 1.0, color);
        }
    }

    /// HEAD `load_world_state`: another world, other players.
    pub(super) fn hangglider_world_opened() {
        PLAYERS.with(|c| *c.borrow_mut() = Players::default());
    }
}

/// `hangglider:item/cross` (16 x 16) as pixel runs (x, y, width, red), from the carried texture: a native
/// mod cannot put a texture into the GUI atlas.
const CROSS: &[(u8, u8, u8, u8)] = &[
    (0, 0, 3, 108), (13, 0, 2, 108), (15, 0, 1, 57), (0, 1, 1, 108), (1, 1, 2, 188), (3, 1, 1, 57),
    (12, 1, 1, 108), (13, 1, 2, 188), (15, 1, 1, 57), (0, 2, 1, 108), (1, 2, 3, 188), (4, 2, 1, 57),
    (11, 2, 1, 108), (12, 2, 3, 188), (15, 2, 1, 57), (1, 3, 1, 57), (2, 3, 3, 188), (5, 3, 1, 57),
    (10, 3, 1, 108), (11, 3, 3, 188), (14, 3, 1, 57), (2, 4, 1, 57), (3, 4, 3, 188), (6, 4, 1, 57),
    (9, 4, 1, 108), (10, 4, 3, 188), (13, 4, 1, 57), (3, 5, 1, 57), (4, 5, 3, 188), (7, 5, 1, 57),
    (8, 5, 1, 108), (9, 5, 3, 188), (12, 5, 1, 57), (4, 6, 1, 57), (5, 6, 6, 188), (11, 6, 1, 57),
    (5, 7, 1, 57), (6, 7, 4, 188), (10, 7, 1, 57), (5, 8, 1, 108), (6, 8, 4, 188), (10, 8, 1, 57),
    (4, 9, 1, 108), (5, 9, 6, 188), (11, 9, 1, 57), (3, 10, 1, 108), (4, 10, 3, 188), (7, 10, 2, 57),
    (9, 10, 3, 188), (12, 10, 1, 57), (2, 11, 1, 108), (3, 11, 3, 188), (6, 11, 1, 57), (9, 11, 1, 57),
    (10, 11, 3, 188), (13, 11, 1, 57), (1, 12, 1, 108), (2, 12, 3, 188), (5, 12, 1, 57), (10, 12, 1, 57),
    (11, 12, 3, 188), (14, 12, 1, 57), (0, 13, 1, 108), (1, 13, 3, 188), (4, 13, 1, 57), (11, 13, 1, 57),
    (12, 13, 3, 188), (15, 13, 1, 57), (0, 14, 1, 108), (1, 14, 2, 188), (3, 14, 1, 57), (12, 14, 1, 57),
    (13, 14, 2, 188), (15, 14, 1, 57), (0, 15, 1, 108), (1, 15, 2, 57), (13, 15, 3, 57),
];
