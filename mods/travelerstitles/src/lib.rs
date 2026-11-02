// Traveler's Titles (YUNG, 1.20 build 4.0.2 for 1.20.1) for Horizon: the rules, without any game code.
// Source of every rule: surse/travelerstitles (branch 1.20): `TitleRenderManager`, `TitleRenderer`, `ConfigModule`,
// `SoundModule`, `TagModule`; the values are the pack's `config/travelerstitles-forge-1_20.toml`.
// This file is pasted into the game as `crate::game::travelerstitles::engine` (tools/hnmod.py bundle) and is also the
// `horizon_travelerstitles` crate the tests use: plain Rust, no game types, no inner attributes (`#![...]`).
use std::collections::VecDeque;

/// the pack's file `travelerstitles-forge-1_20.toml`, as Rust constants (every value is the original's default).
pub mod config {
    // ---- ["Traveler's Titles"."Biome Titles"]
    pub const BIOMES_ENABLED: bool = true; // "Enable Biome Titles"
    pub const BIOMES_FADE_IN: i64 = 10; // "Text Fade-In Time"
    pub const BIOMES_DISPLAY: i64 = 50; // "Text Display Time"
    pub const BIOMES_FADE_OUT: i64 = 10; // "Text Fade-Out Time"
    pub const BIOMES_COOLDOWN: i64 = 80; // "Text Cooldown Time"
    pub const BIOMES_COLOR: &str = "ffffff"; // "Default Text Color"
    pub const BIOMES_SIZE: f64 = 2.1; // "Text Size"
    pub const BIOMES_SHADOW: bool = true; // "Display Text Shadow"
    pub const BIOMES_Y_OFFSET: i64 = -33; // "Text Y Offset"
    pub const BIOMES_X_OFFSET: i64 = 0; // "Text X Offset"
    pub const BIOMES_BLACKLIST: &str = "[minecraft:the_end, minecraft:river, minecraft:frozen_river]"; // "Blacklisted Biomes"
    pub const BIOMES_RECENT: i64 = 5; // "Number of Most Recent Biomes Saved"
    pub const BIOMES_CENTER: bool = true; // "Center Title"
    pub const BIOMES_RESET_ON_DIMENSION: bool = true; // "Reset Biome Cache When Changing Dimensions"
    pub const BIOMES_ONLY_SURFACE: bool = true; // "Only Show Biome Titles When Exposed To Skylight"
    // ---- ["Traveler's Titles"."Dimension Titles"]
    pub const DIMENSIONS_ENABLED: bool = true; // "Enable Dimension Titles"
    pub const DIMENSIONS_FADE_IN: i64 = 10;
    pub const DIMENSIONS_DISPLAY: i64 = 70;
    pub const DIMENSIONS_FADE_OUT: i64 = 20;
    pub const DIMENSIONS_COLOR: &str = "ffffff";
    pub const DIMENSIONS_SIZE: f64 = 3.0;
    pub const DIMENSIONS_SHADOW: bool = true;
    pub const DIMENSIONS_Y_OFFSET: i64 = -32;
    pub const DIMENSIONS_X_OFFSET: i64 = 0;
    pub const DIMENSIONS_BLACKLIST: &str = "[]";
    pub const DIMENSIONS_CENTER: bool = true;
    pub const DIMENSIONS_ONLY_SURFACE: bool = false;
    // ---- ["Traveler's Titles"."Custom Sound Settings"]
    pub const SOUND_BIOME_VOLUME: f64 = 1.0;
    pub const SOUND_BIOME_PITCH: f64 = 1.0;
    pub const SOUND_DIMENSION_VOLUME: f64 = 1.0;
    pub const SOUND_DIMENSION_PITCH: f64 = 1.0;
}

/// The sound events of the mod (`SoundModule`: `travelerstitles:biome` / `:dimension`; `:waystone` goes with Waystones).
pub const SOUND_BIOME: &str = "travelerstitles:biome";
pub const SOUND_DIMENSION: &str = "travelerstitles:dimension";

/// `data/travelerstitles/tags/worldgen/biome/is_underground.json` with `#forge:is_underground` expanded
/// (Forge 1.20.1-47.4.28 `forge:is_cave`: lush_caves, dripstone_caves, deep_dark) and Terralith's caves.
pub const UNDERGROUND_BIOMES: [&str; 17] = [
    "minecraft:lush_caves",
    "minecraft:dripstone_caves",
    "minecraft:deep_dark",
    "terralith:cave/andesite_caves",
    "terralith:cave/crystal_caves",
    "terralith:cave/deep_caves",
    "terralith:cave/desert_caves",
    "terralith:cave/diorite_caves",
    "terralith:cave/frostfire_caves",
    "terralith:cave/fungal_caves",
    "terralith:cave/granite_caves",
    "terralith:cave/ice_caves",
    "terralith:cave/infested_caves",
    "terralith:cave/mantle_caves",
    "terralith:cave/thermal_caves",
    "terralith:cave/tuff_caves",
    "terralith:cave/underground_jungle",
];

/// `ConfigBiomesForge.parseList`: `[a, b, c]` (or `a, b, c`) into the entries.
pub fn parse_list(text: &str) -> Vec<String> {
    text.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The settings of one title (`TitleRenderer`'s constructor arguments).
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub fade_in: i32,
    pub display: i32,
    pub fade_out: i32,
    /// Only the biome titles have one.
    pub cooldown: i32,
    /// RGB as hex text, parsed by `set_color`.
    pub color: String,
    pub size: f32,
    pub shadow: bool,
    pub y_offset: i32,
    pub x_offset: i32,
    pub centered: bool,
    /// The size of the list of recent entries.
    pub recent: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub biomes: Settings,
    pub biome_blacklist: Vec<String>,
    pub reset_biomes_on_dimension: bool,
    pub biomes_only_surface: bool,
    pub dimensions: Settings,
    pub dimension_blacklist: Vec<String>,
    pub dimensions_only_surface: bool,
    pub biome_volume: f32,
    pub biome_pitch: f32,
    pub dimension_volume: f32,
    pub dimension_pitch: f32,
}

impl Config {
    /// the pack's values.
    pub fn tuned() -> Self {
        use config::*;
        Self {
            biomes: Settings {
                enabled: BIOMES_ENABLED,
                fade_in: BIOMES_FADE_IN as i32,
                display: BIOMES_DISPLAY as i32,
                fade_out: BIOMES_FADE_OUT as i32,
                cooldown: BIOMES_COOLDOWN as i32,
                color: BIOMES_COLOR.to_owned(),
                size: BIOMES_SIZE as f32,
                shadow: BIOMES_SHADOW,
                y_offset: BIOMES_Y_OFFSET as i32,
                x_offset: BIOMES_X_OFFSET as i32,
                centered: BIOMES_CENTER,
                recent: BIOMES_RECENT.max(0) as usize,
            },
            biome_blacklist: parse_list(BIOMES_BLACKLIST),
            reset_biomes_on_dimension: BIOMES_RESET_ON_DIMENSION,
            biomes_only_surface: BIOMES_ONLY_SURFACE,
            dimensions: Settings {
                enabled: DIMENSIONS_ENABLED,
                fade_in: DIMENSIONS_FADE_IN as i32,
                display: DIMENSIONS_DISPLAY as i32,
                fade_out: DIMENSIONS_FADE_OUT as i32,
                cooldown: 0,
                color: DIMENSIONS_COLOR.to_owned(),
                size: DIMENSIONS_SIZE as f32,
                shadow: DIMENSIONS_SHADOW,
                y_offset: DIMENSIONS_Y_OFFSET as i32,
                x_offset: DIMENSIONS_X_OFFSET as i32,
                centered: DIMENSIONS_CENTER,
                // The dimension renderer is built with a list of 1.
                recent: 1,
            },
            dimension_blacklist: parse_list(DIMENSIONS_BLACKLIST),
            dimensions_only_surface: DIMENSIONS_ONLY_SURFACE,
            biome_volume: SOUND_BIOME_VOLUME as f32,
            biome_pitch: SOUND_BIOME_PITCH as f32,
            dimension_volume: SOUND_DIMENSION_VOLUME as f32,
            dimension_pitch: SOUND_DIMENSION_PITCH as f32,
        }
    }

    /// the pack's values, then what the settings folder of the mod overrides: one file per key, named like the
    /// original's config fields: `biomes.enabled`, `biomes.textFadeInTime`, `biomes.textDisplayTime`,
    /// `biomes.textFadeOutTime`, `biomes.textCooldownTime`, `biomes.textColor`, `biomes.textSize`,
    /// `biomes.renderShadow`, `biomes.textYOffset`, `biomes.textXOffset`, `biomes.biomeBlacklist` (`[a, b]`),
    /// `biomes.recentBiomeCacheSize`, `biomes.centerText`, `biomes.resetBiomeCacheOnDimensionChange`,
    /// `biomes.onlyUpdateAtSurface`; `dimensions.` the same without cooldown and cache (`dimensionBlacklist`,
    /// `onlyUpdateAtSurface`); `sound.biomeVolume`, `sound.biomePitch`, `sound.dimensionVolume`,
    /// `sound.dimensionPitch`. A value that does not parse keeps the pack's.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut c = Self::tuned();
        let flag = |key: &str, into: &mut bool| {
            if let Some(v) = get(key) {
                match v.trim().to_ascii_lowercase().as_str() {
                    "true" => *into = true,
                    "false" => *into = false,
                    _ => {}
                }
            }
        };
        let int = |key: &str, into: &mut i32| {
            if let Some(n) = get(key).and_then(|v| v.trim().parse::<i32>().ok()) {
                *into = n;
            }
        };
        let float = |key: &str, into: &mut f32| {
            if let Some(n) = get(key).and_then(|v| v.trim().parse::<f32>().ok()) {
                *into = n;
            }
        };
        let hex = |key: &str, into: &mut String| {
            if let Some(v) = get(key) {
                let v = v.trim().trim_start_matches('#').to_owned();
                if i64::from_str_radix(&v, 16).is_ok() {
                    *into = v;
                }
            }
        };
        let list = |key: &str, into: &mut Vec<String>| {
            if let Some(v) = get(key) {
                *into = parse_list(&v);
            }
        };
        flag("biomes.enabled", &mut c.biomes.enabled);
        int("biomes.textFadeInTime", &mut c.biomes.fade_in);
        int("biomes.textDisplayTime", &mut c.biomes.display);
        int("biomes.textFadeOutTime", &mut c.biomes.fade_out);
        int("biomes.textCooldownTime", &mut c.biomes.cooldown);
        hex("biomes.textColor", &mut c.biomes.color);
        float("biomes.textSize", &mut c.biomes.size);
        flag("biomes.renderShadow", &mut c.biomes.shadow);
        int("biomes.textYOffset", &mut c.biomes.y_offset);
        int("biomes.textXOffset", &mut c.biomes.x_offset);
        list("biomes.biomeBlacklist", &mut c.biome_blacklist);
        let mut recent = c.biomes.recent as i32;
        int("biomes.recentBiomeCacheSize", &mut recent);
        c.biomes.recent = recent.max(0) as usize;
        flag("biomes.centerText", &mut c.biomes.centered);
        flag("biomes.resetBiomeCacheOnDimensionChange", &mut c.reset_biomes_on_dimension);
        flag("biomes.onlyUpdateAtSurface", &mut c.biomes_only_surface);
        flag("dimensions.enabled", &mut c.dimensions.enabled);
        int("dimensions.textFadeInTime", &mut c.dimensions.fade_in);
        int("dimensions.textDisplayTime", &mut c.dimensions.display);
        int("dimensions.textFadeOutTime", &mut c.dimensions.fade_out);
        hex("dimensions.textColor", &mut c.dimensions.color);
        float("dimensions.textSize", &mut c.dimensions.size);
        flag("dimensions.renderShadow", &mut c.dimensions.shadow);
        int("dimensions.textYOffset", &mut c.dimensions.y_offset);
        int("dimensions.textXOffset", &mut c.dimensions.x_offset);
        list("dimensions.dimensionBlacklist", &mut c.dimension_blacklist);
        flag("dimensions.centerText", &mut c.dimensions.centered);
        flag("dimensions.onlyUpdateAtSurface", &mut c.dimensions_only_surface);
        float("sound.biomeVolume", &mut c.biome_volume);
        float("sound.biomePitch", &mut c.biome_pitch);
        float("sound.dimensionVolume", &mut c.dimension_volume);
        float("sound.dimensionPitch", &mut c.dimension_pitch);
        c
    }
}

/// `Util.makeDescriptionId(type, id)`: `type.namespace.path` with `/` as `.`.
pub fn description_id(kind: &str, id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    format!("{kind}.{ns}.{}", path.replace('/', "."))
}

/// `Language.has` / `Language.getOrDefault`.
pub trait Lang {
    fn has(&self, key: &str) -> bool;
    fn get(&self, key: &str) -> Option<String>;
}

/// What a title is drawn with (`TitleRenderer.renderText`'s decisions).
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub text: String,
    /// The text is centred: positions are from the middle of the screen, else from the top left.
    pub centered: bool,
    /// The scale of the pose (`titleTextSize`).
    pub size: f32,
    /// Where the text starts in the scaled space: the offset (and `-width / 2` when centred).
    pub x: i32,
    pub y: i32,
    pub rgb: u32,
    pub alpha: u8,
    pub shadow: bool,
}

/// `TitleRenderer<T>`; `T` is the biome or the dimension's id.
#[derive(Clone, Debug)]
pub struct Renderer {
    pub recent: VecDeque<String>,
    pub displayed: Option<String>,
    pub timer: i32,
    pub cooldown: i32,
    pub settings: Settings,
    color: u32,
}

impl Renderer {
    pub fn new(settings: Settings) -> Self {
        let mut r = Self {
            recent: VecDeque::new(),
            displayed: None,
            timer: 0,
            cooldown: 0,
            color: 0xFF_FFFF,
            settings,
        };
        let c = r.settings.color.clone();
        r.set_color(&c);
        r
    }

    /// `setColor`: an RGB hex; anything else is white.
    pub fn set_color(&mut self, text: &str) {
        self.color = i64::from_str_radix(text, 16).map_or(0xFF_FFFF, |v| (v as u32) & 0xFF_FFFF);
    }

    /// `tick`.
    pub fn tick(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
            if self.timer <= 0 {
                self.clear_timer();
            }
        }
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }
    }

    /// `displayTitle`.
    pub fn display_title(&mut self, text: String) {
        self.displayed = Some(text);
        self.timer = self.settings.fade_in + self.settings.display + self.settings.fade_out;
    }

    pub fn clear_timer(&mut self) {
        self.timer = 0;
    }

    pub fn add_recent(&mut self, entry: &str) {
        if self.recent.len() >= self.settings.recent && !self.recent.is_empty() {
            self.recent.pop_front();
        }
        if self.settings.recent > 0 {
            self.recent.push_back(entry.to_owned());
        }
    }

    pub fn recent_has(&self, entry: &str) -> bool {
        self.recent.iter().any(|e| e == entry)
    }

    /// The opacity of `renderText` (0-255) at `partial` ticks into the tick.
    pub fn opacity(&self, partial: f32) -> i32 {
        let s = &self.settings;
        let age = self.timer as f32 - partial;
        let mut opacity = 255;
        if self.timer > s.fade_out + s.display {
            let r = (s.fade_in + s.display + s.fade_out) as f32 - age;
            opacity = (r * 255.0 / s.fade_in as f32) as i32;
        }
        if self.timer <= s.fade_out {
            opacity = (age * 255.0 / s.fade_out as f32) as i32;
        }
        opacity.clamp(0, 255)
    }

    /// `renderText`: what to draw now, if anything. `width` = the title's width in GUI pixels.
    pub fn frame(&self, partial: f32, width: impl Fn(&str) -> i32) -> Option<Frame> {
        let title = self.displayed.as_ref()?;
        if self.timer <= 0 {
            return None;
        }
        let opacity = self.opacity(partial);
        // `opacity > 8`.
        if opacity <= 8 {
            return None;
        }
        let s = &self.settings;
        let x = if s.centered { s.x_offset + (-width(title) / 2) } else { s.x_offset };
        Some(Frame {
            text: title.clone(),
            centered: s.centered,
            size: s.size,
            x,
            y: s.y_offset,
            rgb: self.color,
            alpha: opacity as u8,
            shadow: s.shadow,
        })
    }
}

/// A sound the tick asks for (`player.playSound(event, volume, pitch)`).
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub event: &'static str,
    pub volume: f32,
    pub pitch: f32,
}

/// What `playerTick` reads from the world.
#[derive(Clone, Debug)]
pub struct View<'a> {
    /// `level.isLoaded(player.blockPosition())`.
    pub loaded: bool,
    /// `dimensionType().hasSkyLight()`.
    pub has_skylight: bool,
    /// `level.canSeeSky(pos)`.
    pub can_see_sky: bool,
    /// `level.dimension().location()`.
    pub dimension: &'a str,
    /// The biome at the player's feet.
    pub biome: &'a str,
}

/// `TitleRenderManager` (without the Waystones part: Waystones is not in Horizon yet).
#[derive(Clone, Debug)]
pub struct Manager {
    pub cfg: Config,
    pub biome: Renderer,
    pub dimension: Renderer,
}

impl Manager {
    pub fn new(cfg: Config) -> Self {
        Self {
            biome: Renderer::new(cfg.biomes.clone()),
            dimension: Renderer::new(cfg.dimensions.clone()),
            cfg,
        }
    }

    /// `ConfigModule.updateRenderersFromConfig`: the settings are baked into the renderers.
    pub fn rebake(&mut self, cfg: Config) {
        self.biome.settings = cfg.biomes.clone();
        self.dimension.settings = cfg.dimensions.clone();
        self.cfg = cfg;
    }

    /// `clientTick`: ticks the renderers unless the game is paused.
    pub fn client_tick(&mut self, paused: bool) {
        if !paused {
            self.dimension.tick();
            self.biome.tick();
        }
    }

    /// `playerChangedDimension` (for the player): resets the biome cache when the config says so.
    pub fn dimension_changed(&mut self) {
        if self.cfg.biomes.enabled && self.cfg.reset_biomes_on_dimension {
            self.biome.clear_timer();
            self.biome.recent.clear();
            self.biome.displayed = None;
        }
    }

    /// `playerTick`: marks the titles to show, and answers the sounds to play.
    pub fn player_tick(&mut self, v: &View, lang: &impl Lang) -> Vec<Sound> {
        let mut sounds = Vec::new();
        if !v.loaded {
            return sounds;
        }
        let underground = v.has_skylight && !v.can_see_sky;
        self.update_dimension(v, underground, lang, &mut sounds);
        // No Waystones: nothing overrides the biome title.
        self.update_biome(v, underground, lang, &mut sounds);
        sounds
    }

    fn update_dimension(&mut self, v: &View, underground: bool, lang: &impl Lang, sounds: &mut Vec<Sound>) {
        if underground && self.cfg.dimensions_only_surface {
            return;
        }
        if !self.dimension.settings.enabled || self.dimension.recent_has(v.dimension) {
            return;
        }
        // Blacklisted dimensions are ignored (not even remembered).
        if self.cfg.dimension_blacklist.iter().any(|d| d == v.dimension) {
            return;
        }
        let key = description_id("travelerstitles", v.dimension);
        // `???` for a dimension with no name.
        let title = if lang.has(&key) { lang.get(&key).unwrap_or_else(|| key.clone()) } else { "???".to_owned() };
        let colour_key = format!("{key}.color");
        let colour = if lang.has(&colour_key) {
            lang.get(&colour_key).unwrap_or_default()
        } else {
            self.dimension.settings.color.clone()
        };
        self.dimension.set_color(&colour);
        self.dimension.display_title(title);
        self.dimension.add_recent(v.dimension);
        sounds.push(Sound {
            event: SOUND_DIMENSION,
            volume: self.cfg.dimension_volume,
            pitch: self.cfg.dimension_pitch,
        });
    }

    fn update_biome(&mut self, v: &View, underground: bool, lang: &impl Lang, sounds: &mut Vec<Sound>) {
        let underground_biome = UNDERGROUND_BIOMES.contains(&v.biome);
        if underground && self.cfg.biomes_only_surface && !underground_biome {
            return;
        }
        if !(self.biome.settings.enabled && self.biome.cooldown <= 0 && !self.biome.recent_has(v.biome)) {
            return;
        }
        if self.cfg.biome_blacklist.iter().any(|b| b == v.biome) {
            return;
        }
        let override_key = description_id("travelerstitles.biome", v.biome);
        let normal_key = description_id("biome", v.biome);
        // A name is shown only for a biome with an entry: the mod's own, then the game's.
        let name_key = if lang.has(&override_key) {
            &override_key
        } else if lang.has(&normal_key) {
            &normal_key
        } else {
            return;
        };
        let title = lang.get(name_key).unwrap_or_else(|| name_key.clone());
        let override_colour = format!("{override_key}.color");
        let normal_colour = format!("{normal_key}.color");
        let colour = if lang.has(&override_colour) {
            lang.get(&override_colour).unwrap_or_default()
        } else if lang.has(&normal_colour) {
            lang.get(&normal_colour).unwrap_or_default()
        } else {
            self.biome.settings.color.clone()
        };
        // No need to display a title that has not changed.
        if self.biome.displayed.as_deref() == Some(title.as_str()) {
            return;
        }
        self.biome.set_color(&colour);
        self.biome.display_title(title);
        self.biome.cooldown = self.cfg.biomes.cooldown;
        self.biome.add_recent(v.biome);
        // The sound does not overlap the dimension title's.
        if self.dimension.timer <= 0 {
            sounds.push(Sound {
                event: SOUND_BIOME,
                volume: self.cfg.biome_volume,
                pitch: self.cfg.biome_pitch,
            });
        }
    }

    /// `/tt_reload` (`ReloadConfigCommand`): the config is read again and the titles forgotten.
    pub fn reload(&mut self, cfg: Config) {
        self.rebake(cfg);
        self.biome.clear_timer();
        self.biome.recent.clear();
        self.dimension.clear_timer();
        self.dimension.recent.clear();
    }

    /// `/biometitle <biome>` (`BiomeTitleCommand.displayTitle`): shows the biome's title at once, with no sound and
    /// without remembering it. `Ok(true)` when the biome is blacklisted (the command says so).
    pub fn show_biome(&mut self, arg: &str, lang: &impl Lang) -> Result<bool, CommandError> {
        let id = normalize_id(arg).ok_or_else(|| CommandError::Invalid(arg.to_owned()))?;
        let blacklisted = self.cfg.biome_blacklist.iter().any(|b| *b == id);
        let override_key = description_id("travelerstitles.biome", &id);
        let normal_key = description_id("biome", &id);
        let name_key = if lang.has(&override_key) {
            &override_key
        } else if lang.has(&normal_key) {
            &normal_key
        } else {
            return Err(CommandError::NotFound(arg.to_owned()));
        };
        let title = lang.get(name_key).unwrap_or_else(|| name_key.clone());
        let colour = [format!("{override_key}.color"), format!("{normal_key}.color")]
            .iter()
            .find(|k| lang.has(k))
            .and_then(|k| lang.get(k))
            .unwrap_or_else(|| self.biome.settings.color.clone());
        self.biome.set_color(&colour);
        self.biome.display_title(title);
        self.biome.cooldown = self.cfg.biomes.cooldown;
        Ok(blacklisted)
    }

    /// `/dimensiontitle <dimension>` (`DimensionTitleCommand.displayTitle`): shows the dimension's title at once.
    /// `true` when the dimension is blacklisted (the command says so).
    pub fn show_dimension(&mut self, id: &str, lang: &impl Lang) -> bool {
        let blacklisted = self.cfg.dimension_blacklist.iter().any(|d| d == id);
        let key = description_id("travelerstitles", id);
        let title = if lang.has(&key) { lang.get(&key).unwrap_or_else(|| key.clone()) } else { "???".to_owned() };
        let colour_key = format!("{key}.color");
        let colour = if lang.has(&colour_key) {
            lang.get(&colour_key).unwrap_or_default()
        } else {
            self.dimension.settings.color.clone()
        };
        self.dimension.set_color(&colour);
        self.dimension.display_title(title);
        blacklisted
    }

    /// `renderTitles`: the dimension title, then the biome title (not while F3 shows).
    pub fn frames(&self, debug_screen: bool, partial: f32, width: impl Fn(&str) -> i32 + Copy) -> Vec<Frame> {
        if debug_screen {
            return Vec::new();
        }
        [self.dimension.frame(partial, width), self.biome.frame(partial, width)]
            .into_iter()
            .flatten()
            .collect()
    }
}

/// `TitleRenderer.drawBackdrop(guiGraphics, -10, width, ..)`: the rectangle (left, top, right, bottom) of the
/// backdrop in the scaled space; drawn only when `Options.getBackgroundColor(0.0F)` is not 0 (the option
/// "show background for chat only" is off).
pub fn backdrop(width: i32) -> [i32; 4] {
    let x = -width / 2;
    [x - 2, -10 - 2, x + width + 2, -10 + 9 + 2]
}

/// `FastColor.ARGB32.multiply`: the channels of two ARGB ints multiplied (each `/ 255`).
pub fn multiply(a: u32, b: u32) -> u32 {
    let ch = |shift: u32| (((a >> shift) & 0xFF) * ((b >> shift) & 0xFF) / 255) << shift;
    ch(24) | ch(16) | ch(8) | ch(0)
}

/// What a command argument can get wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    /// `travelerstitles.commands.biometitle.invalid`: not a resource location.
    Invalid(String),
    /// `travelerstitles.commands.biometitle.notfound`: a biome with no name.
    NotFound(String),
}

/// `new ResourceLocation(text)`: `namespace:path` (the namespace `minecraft` when there is none), or `None` for
/// characters a resource location may not have.
pub fn normalize_id(text: &str) -> Option<String> {
    let (ns, path) = text.split_once(':').unwrap_or(("minecraft", text));
    let ns_ok = !ns.is_empty() && ns.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-.".contains(c));
    let path_ok =
        !path.is_empty() && path.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-./".contains(c));
    (ns_ok && path_ok).then(|| format!("{ns}:{path}"))
}

/// The dimensions `DimensionArgument` finds in a single-player world: the three of the game.
pub fn known_dimension(text: &str) -> Option<&'static str> {
    match normalize_id(text)?.as_str() {
        "minecraft:overworld" => Some("minecraft:overworld"),
        "minecraft:the_nether" => Some("minecraft:the_nether"),
        "minecraft:the_end" => Some("minecraft:the_end"),
        _ => None,
    }
}
