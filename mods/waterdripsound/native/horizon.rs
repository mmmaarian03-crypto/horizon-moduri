// Drip Sounds (waterdripsound) inside Horizon's game (nightly 142388e1c). The rules are the engine below
// (src/lib.rs); the particle engine's side is the module `horizon_client::particles::waterdripsound_particles`
// (mixins/waterdripsound/particles.rs), which gives the falling water and lava drips a landing sound marker.
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("waterdripsound_engine.rs");
}

pub const ID: &str = "waterdripsound";
pub const TITLE: &str = "Drip Sounds";
pub const DESCRIPTION: &str = "Water and lava drips falling from blocks make a sound when they land (the dripstone \
drip sounds). the pack's file: on, volume 30%, Ambient; drip chance and particle range stay vanilla.";

impl Game<'_> {
    /// The player's settings (Options > Mods), the pack's file where nothing was changed.
    fn waterdripsound_settings(&self) -> engine::Settings {
        let mut s = engine::Settings::tuned();
        if let Some(v) = self.native_setting(ID, "enabled").and_then(|v| v.parse::<bool>().ok()) {
            s.enabled = v;
        }
        if let Some(v) = self.native_setting(ID, "volume").and_then(|v| v.parse::<i32>().ok()) {
            s.volume = f64::from(v.clamp(0, 100)) / 100.0;
        }
        if let Some(v) = self.native_setting(ID, "category")
            && engine::SOURCES.contains(&v.as_str())
        {
            s.category = v;
        }
        s
    }

    /// The `FluidState.getType()` of a block of the window's world.
    fn waterdripsound_fluid(&self, p: engine::Pos) -> engine::Fluid {
        use horizon_client::blocks::Fluid;
        let f = self.res.facts().get(self.world.block(p[0], p[1], p[2]));
        engine::Fluid::of(f.fluid == Fluid::Water, f.fluid == Fluid::Lava, f.fluid_level, f.fluid_falling)
    }

    /// HEAD `pickups::Game::tick_pickups`: runs on the window right after `ParticleEngine.tick`, before the
    /// particles' sounds are played. The drips marked by the particle module that landed this tick are in
    /// `particles.sounds` as markers: each becomes the mod's sound (`MixinFallAndLandParticle.onLand`), played
    /// for this client only (`level.playLocalSound`) on the chosen source. Then the new falling drips are marked.
    pub(super) fn waterdripsound_after_particles(&mut self) {
        use horizon_client::particles::waterdripsound_particles as marks;
        let sounds = std::mem::take(&mut self.particles.sounds);
        let mut settings = None;
        let mut keep = Vec::with_capacity(sounds.len());
        for (name, at, volume, pitch) in sounds {
            let drip = match name {
                marks::WATER => engine::Drip::Water,
                marks::LAVA => engine::Drip::Lava,
                _ => {
                    keep.push((name, at, volume, pitch));
                    continue;
                }
            };
            let s = settings.get_or_insert_with(|| self.waterdripsound_settings()).clone();
            // The engine drew `random * 0.7 + 0.3` from the drip's own random; the original's `Math.random()`.
            let random = ((f64::from(volume) - 0.3) / 0.7).clamp(0.0, 1.0);
            let below = self.waterdripsound_fluid(engine::below(at));
            if let Some(sound) = engine::land_sound(&s, drip, below, random) {
                let capture = self.audio.capture.take();
                self.audio.play_on(s.category_index(), sound.event, Some(at), sound.volume, sound.pitch);
                self.audio.capture = capture;
            }
        }
        // Sounds pushed meanwhile (none: nothing runs between) go after the kept ones.
        keep.append(&mut self.particles.sounds);
        self.particles.sounds = keep;
        marks::mark(&mut self.particles);
    }

    /// Options > Mods: the original's Cloth screen (enable, volume, category).
    pub(super) fn waterdripsound_page(&self, mod_id: &str, buttons: &mut Vec<String>) {
        if mod_id != ID {
            return;
        }
        let s = self.waterdripsound_settings();
        let on = if s.enabled { "ON" } else { "OFF" };
        let cat = s.category.clone();
        let mut name = cat.chars();
        let cat = name.next().map(|c| c.to_uppercase().collect::<String>() + name.as_str()).unwrap_or_default();
        buttons.push(format!("Enable Drip Sound: {on}"));
        buttons.push(format!("Drip Volume: {}%", s.volume_percent()));
        buttons.push(format!("Drip Sound Category: {cat}"));
    }

    pub(super) fn waterdripsound_click(&self, mod_id: &str, button: usize) {
        if mod_id != ID {
            return;
        }
        let s = self.waterdripsound_settings();
        match button {
            0 => self.native_set_setting(ID, "enabled", &(!s.enabled).to_string()),
            1 => self.native_set_setting(
                ID,
                "volume",
                &engine::next_volume_percent(s.volume_percent()).to_string(),
            ),
            2 => self.native_set_setting(ID, "category", engine::next_category(&s.category)),
            _ => {}
        }
    }
}
