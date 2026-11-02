// Traveler's Titles inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs);
// this part reads the player, the world and the language, plays the sounds and paints the titles.
//
// The original ticks in `Minecraft.tick` (HEAD) and `LocalPlayer.tick` (TAIL), draws in `ForgeGui.render` (HEAD) and
// listens to `Entity.changeDimension`. Here: one HEAD on `native_tick` (the window's client tick; a paused
// single-player game ticks nothing, as there), one HEAD on `draw_boss_bars` (the first thing the HUD draws after
// the hotbar, under the bars) and the dimension change is seen as the dimension id changing between two ticks.
use super::Game;
use horizon_client::{
    hud::{HudBatch, rgba},
    text::Text,
};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    include!("travelerstitles_engine.rs");
}

pub const ID: &str = "travelerstitles";
pub const TITLE: &str = "Traveler's Titles 4.0.2";
pub const DESCRIPTION: &str = "A title with the biome's name when you enter it, and the dimension's name when you change dimension, with a sound. the pack's travelerstitles-forge-1_20.toml.";

thread_local! {
    /// `TravelersTitlesCommon.titleManager`: static in the original, so it outlives a world, as here.
    static MANAGER: RefCell<Option<engine::Manager>> = const { RefCell::new(None) };
    /// The dimension of the last tick (`Entity.changeDimension` is seen as a change of it).
    static LAST_DIMENSION: RefCell<Option<&'static str>> = const { RefCell::new(None) };
}

/// `Language.getInstance()`: the language the game shows, with the pinned English map behind it.
struct GameLang<'a, 'g>(&'a Game<'g>);
impl engine::Lang for GameLang<'_, '_> {
    fn has(&self, key: &str) -> bool {
        self.0.pack.names.display_get(key).is_some()
    }
    fn get(&self, key: &str) -> Option<String> {
        self.0.pack.names.display_get(key)
    }
}

impl Game<'_> {
    /// the pack's values, then the settings folder of the mod.
    fn travelerstitles_with<R>(&self, f: impl FnOnce(&mut engine::Manager) -> R) -> R {
        MANAGER.with(|m| {
            let mut m = m.borrow_mut();
            let manager = m.get_or_insert_with(|| {
                engine::Manager::new(engine::Config::read(|key| self.native_setting(ID, key)))
            });
            f(manager)
        })
    }

    /// HEAD on `native_tick`: `Minecraft.tick` (the renderers tick unless paused), then `LocalPlayer.tick`'s end
    /// (the titles to show).
    pub(super) fn travelerstitles_tick(&mut self) {
        if !self.native_window() || !self.native_on(ID) {
            return;
        }
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let position = [player.position.x, player.position.y, player.position.z];
        // `Minecraft.isPaused()`: a single-player game with a pausing screen open. Nothing ticks then,
        // neither the renderers nor the player.
        let paused = self.pauses_game();
        self.travelerstitles_with(|m| m.client_tick(paused));
        if paused {
            return;
        }
        let dimension = self.dim.id();
        let changed = LAST_DIMENSION.with(|d| d.borrow_mut().replace(dimension).is_some_and(|old| old != dimension));
        if changed {
            self.travelerstitles_with(engine::Manager::dimension_changed);
        }
        let (bx, by, bz) = (
            position[0].floor() as i32,
            position[1].floor() as i32,
            position[2].floor() as i32,
        );
        let loaded = self.world.loaded(bx, bz);
        let biome = self
            .world
            .chunks
            .get(&(bx >> 4, bz >> 4))
            .map_or_else(String::new, |c| self.res.biomes().get(c.biome(bx, by, bz)).name.to_string());
        let view = engine::View {
            loaded,
            has_skylight: self
                .remote
                .as_ref()
                .map_or(dimension == "minecraft:overworld", |r| r.dimension.has_skylight),
            // `canSeeSky`: the sky light at the player's feet is the full 15.
            can_see_sky: (self.world.light(bx, by, bz) >> 4) >= 15,
            dimension,
            biome: &biome,
        };
        let sounds = {
            let lang = GameLang(self);
            self.travelerstitles_with(|m| m.player_tick(&view, &lang))
        };
        if sounds.is_empty() {
            return;
        }
        // `player.playSound`: the player's own sound source, heard by this client only.
        let players = horizon_client::settings::SOUND_SOURCES
            .iter()
            .position(|s| *s == "player")
            .unwrap_or(0);
        for sound in sounds {
            let capture = self.audio.capture.take();
            self.audio
                .play_on(players, sound.event, Some(position), sound.volume, sound.pitch);
            self.audio.capture = capture;
        }
    }

    /// HEAD on `draw_boss_bars`: `renderTitles` (not under F3).
    pub(super) fn travelerstitles_draw(&self, batch: &mut HudBatch) {
        if !self.native_on(ID) || !self.native_window() || self.player.is_none() {
            return;
        }
        let atlas = &self.gui.atlas;
        let partial = self.clock.fraction() as f32;
        let frames = MANAGER.with(|m| {
            m.borrow()
                .as_ref()
                .map(|m| m.frames(self.debug, partial, |t: &str| atlas.font.width(t) as i32))
                .unwrap_or_default()
        });
        if frames.is_empty() {
            return;
        }
        // `Options.getBackgroundColor(0.0F)`: 0 unless the text background is not for the chat only.
        let backdrop = self.settings.text_background(0.0);
        let (sw, sh) = (batch.width, batch.height);
        for f in frames {
            // `pose().translate(width / 2D, height / 2D)` then `scale(size)`: the text is placed in the
            // translated space and scaled about its origin.
            let (cx, cy) = if f.centered { (sw / 2.0, sh / 2.0) } else { (0.0, 0.0) };
            let color = rgba((f.rgb >> 16) as u8, (f.rgb >> 8) as u8, f.rgb as u8, f.alpha);
            let start = batch.vertices.len();
            if backdrop != 0 {
                let width = atlas.font.width(&f.text) as i32;
                let [x0, y0, x1, y1] = engine::backdrop(width);
                let argb = engine::multiply(backdrop, u32::from(f.alpha) << 24 | (f.rgb & 0xFF_FFFF));
                batch.fill(
                    atlas,
                    cx + x0 as f32,
                    cy + y0 as f32,
                    (x1 - x0) as f32,
                    (y1 - y0) as f32,
                    rgba((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8),
                );
            }
            batch.text_styled(
                atlas,
                &Text::literal(f.text.as_str()),
                cx + f.x as f32,
                cy + f.y as f32,
                color,
                f.shadow,
            );
            batch.scale_since(start, cx, cy, f.size, f.size);
        }
    }

    /// A command typed in chat, without its slash (`native_command`): `/biometitle <biome>`, `/dimensiontitle <dimension>`
    /// and `/tt_reload`, all of permission level 2 (the cheats). True when the command was ours.
    pub(super) fn travelerstitles_command(&mut self, command: &str) -> bool {
        // Permission level 2: the window's own level (set by the server), or the world's Allow Commands.
        if !self.native_on(ID) || !self.native_window() || !(self.gamemaster() || self.allow_commands()) {
            return false;
        }
        let words: Vec<&str> = command.split_whitespace().collect();
        let translate = |game: &Self, key: &str, fallback: &str, arg: &str| {
            game.pack
                .names
                .display_get(key)
                .unwrap_or_else(|| fallback.to_owned())
                .replace("%s", arg)
        };
        match words.as_slice() {
            ["biometitle", arg] => {
                let result = {
                    let lang = GameLang(self);
                    self.travelerstitles_with(|m| m.show_biome(arg, &lang))
                };
                match result {
                    Ok(true) => self.native_chat("That biome is blacklisted, so its title won't normally show!"),
                    Ok(false) => {}
                    Err(engine::CommandError::Invalid(a)) => {
                        let text = translate(
                            self,
                            "travelerstitles.commands.biometitle.invalid",
                            "\"%s\" does not seem to be a valid biome",
                            &a,
                        );
                        self.native_chat(&text);
                    }
                    Err(engine::CommandError::NotFound(a)) => {
                        let text = translate(
                            self,
                            "travelerstitles.commands.biometitle.notfound",
                            "No title found for biome \"%s\"",
                            &a,
                        );
                        self.native_chat(&text);
                    }
                }
                true
            }
            ["dimensiontitle", arg] => {
                match engine::known_dimension(arg) {
                    Some(id) => {
                        let blacklisted = {
                            let lang = GameLang(self);
                            self.travelerstitles_with(|m| m.show_dimension(id, &lang))
                        };
                        if blacklisted {
                            self.native_chat("That dimension is blacklisted, so its title won't normally show!");
                        }
                    }
                    None => {
                        let text = format!("Unknown dimension '{arg}'");
                        self.native_chat(&text);
                    }
                }
                true
            }
            ["tt_reload"] => {
                let cfg = engine::Config::read(|key| self.native_setting(ID, key));
                self.travelerstitles_with(|m| m.reload(cfg));
                self.native_chat("Loading changes from Traveler's Titles config...");
                true
            }
            _ => false,
        }
    }
}
