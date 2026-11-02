// Presence Footsteps inside Horizon's game (nightly 142388e1c136). The rules are the engine below
// (src/lib.rs); this part reads the world and the entities once a tick, as the original's
// `ClientTickEvents.END_CLIENT_TICK` does (`SoundEngine.onFrame`), and plays the sounds it makes.
//
// It runs on the game that has the window (`native_window`), not while paused, and not when the mod is
// switched off in Options > Mods. The vanilla step sounds of the players are kept from playing by the gate in
// `horizon_client::audio` (`IEntity.playStepSounds` cancelled by `isStepBlocked`): see `gate.rs`.
use super::Game;
use horizon_client::mobs::Mob;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[allow(dead_code)]
mod engine {
    include!("presencefootsteps_engine.rs");
}

pub const ID: &str = "presencefootsteps";
pub const TITLE: &str = "Presence Footsteps 1.0.0";
pub const DESCRIPTION: &str = "Footsteps that follow the block under the foot, the boots, the speed and the weather, for you and the other players and mobs near you. Your file's values: volume 70, wet 50, foliage 100, footwear on. The settings below; the rest are the keys of config/presencefootsteps/userconfig.json kept in this mod's folder.";

#[derive(Default)]
struct State {
    engine: Option<engine::Engine>,
    /// The file settings the engine was built with, to see a change.
    built_with: Option<engine::Config>,
    blocks: HashMap<u32, Rc<engine::BlockInfo>>,
    loaded_from: Option<std::path::PathBuf>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The settings page: its switches' keys and labels.
const TOGGLES: [(&str, &str); 4] = [
    ("disabled", "Footsteps"),
    ("multiplayer", "On servers"),
    ("footwear", "Boots sounds"),
    ("volume", "Volume"),
];

/// The world as the rules read it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
}

impl engine::World for Live<'_, '_> {
    fn block(&self, p: [i32; 3]) -> Rc<engine::BlockInfo> {
        self.game.pf_block(p)
    }
    fn rain_at(&self, p: [i32; 3]) -> bool {
        self.game
            .raining_at(horizon_core::position::BlockPos { x: p[0], y: p[1], z: p[2] })
    }
    /// An armor stand (not collidable) or anything lower than the feet + 0.2 within the block's box.
    fn golems_at(&self, p: [i32; 3], feet_y: f64) -> Vec<String> {
        let mut out = Vec::new();
        for m in self.game.mobs.iter().filter(|m| m.alive() && !m.removed) {
            let w = m.kind.width / 2.0;
            let (x, y, z) = (m.position.x, m.position.y, m.position.z);
            let inside = x + w > f64::from(p[0]) - 0.5
                && x - w < f64::from(p[0]) + 1.5
                && z + w > f64::from(p[2]) - 0.5
                && z - w < f64::from(p[2]) + 1.5
                && y < f64::from(p[1]) + 1.0
                && y + m.kind.height > f64::from(p[1]);
            let collidable = &*m.kind.id != "minecraft:armor_stand";
            if inside && (!collidable || y + m.kind.height < feet_y + 0.2) {
                out.push(m.kind.id.to_string());
            }
        }
        out
    }
    fn time(&self) -> i64 {
        self.game.world.game_time
    }
}

/// `ArmorItem.getEquipSound` of boots: the vanilla material's sound (the armor sound event ids of 1.20.1).
fn boots_sound(item: &str) -> Option<String> {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    if !name.ends_with("_boots") {
        return None;
    }
    let sound = match name.trim_end_matches("_boots") {
        "leather" => "item.armor.equip_leather",
        "chainmail" => "item.armor.equip_chain",
        "iron" => "item.armor.equip_iron",
        "golden" => "item.armor.equip_gold",
        "diamond" => "item.armor.equip_diamond",
        "netherite" => "item.armor.equip_netherite",
        _ => "item.armor.equip_generic",
    };
    Some(format!("minecraft:{sound}"))
}

impl Game<'_> {
    /// The settings: the pack's values, then the keys the settings page keeps.
    fn pf_config(&self) -> engine::Config {
        let mut c = engine::Config::default();
        let flag = |k: &str| self.native_setting(ID, k).and_then(|v| v.parse::<bool>().ok());
        if let Some(v) = flag("disabled") {
            c.disabled = v;
        }
        if let Some(v) = flag("multiplayer") {
            c.multiplayer = v;
        }
        if let Some(v) = flag("footwear") {
            c.footwear = v;
        }
        if let Some(v) = self.native_setting(ID, "volume").and_then(|v| v.parse::<i32>().ok()) {
            c.volume = v.clamp(0, 100);
        }
        // Any other key of userconfig.json, as a whole file kept in the mod's folder.
        if let Some(text) = self.native_setting(ID, "userconfig.json") {
            let mut from_file = engine::Config::read(&text);
            for (k, set) in [("disabled", 0), ("multiplayer", 1), ("footwear", 2), ("volume", 3)] {
                if self.native_setting(ID, k).is_some() {
                    match set {
                        0 => from_file.disabled = c.disabled,
                        1 => from_file.multiplayer = c.multiplayer,
                        2 => from_file.footwear = c.footwear,
                        _ => from_file.volume = c.volume,
                    }
                }
            }
            return from_file;
        }
        c
    }

    /// The carried files, read from the resource root (`assets/presencefootsteps/config/*.json`).
    fn pf_files(&self) -> engine::Files {
        let dir = self.options.resources.join("assets/presencefootsteps/config");
        let read = |n: &str| std::fs::read_to_string(dir.join(format!("{n}.json"))).unwrap_or_default();
        engine::Files {
            blockmap: read("blockmap"),
            golemmap: read("golemmap"),
            primitivemap: read("primitivemap"),
            locomotionmap: read("locomotionmap"),
            acoustics: read("acoustics"),
            variator: read("variator"),
        }
    }

    /// A block state as the rules read it (kept by state id).
    fn pf_block(&self, p: [i32; 3]) -> Rc<engine::BlockInfo> {
        let state = self.world.block(p[0], p[1], p[2]);
        if let Some(hit) = STATE.with(|s| s.borrow().blocks.get(&state.0).cloned()) {
            return hit;
        }
        let facts = self.res.facts();
        let f = facts.get(state);
        let id = f.name.to_string();
        let props = self
            .pack
            .assets
            .registry
            .state(state)
            .map(|s| s.properties.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
            .unwrap_or_default();
        let step: &str = &f.sounds.step;
        let step = if step.is_empty() || step.contains(':') { step.to_owned() } else { format!("minecraft:{step}") };
        let tags = self
            .res
            .items()
            .block_tags
            .iter()
            .filter(|(_, set)| set.contains(&id))
            .map(|(k, _)| k.trim_start_matches('#').to_owned())
            .collect();
        let outline: Vec<[f64; 6]> = f.outline.to_vec();
        let collision = match self.res.core.collision.cached_shapes().cached_shape(state) {
            Ok(Some(shape)) => {
                use horizon_core::geometry::Axis::{X, Y, Z};
                shape
                    .boxes()
                    .iter()
                    .map(|b| [b.min(X), b.min(Y), b.min(Z), b.max(X), b.max(Y), b.max(Z)])
                    .collect()
            }
            // A state with a dynamic shape: its outline is the best there is.
            _ => outline.clone(),
        };
        let info = Rc::new(engine::BlockInfo {
            air: facts.is_air(state),
            liquid: matches!(id.as_str(), "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column"),
            lava_fluid: matches!(f.fluid, horizon_items::blocks::Fluid::Lava),
            water_fluid: matches!(f.fluid, horizon_items::blocks::Fluid::Water),
            // Direction.UP is ordinal 1.
            full_top: (f.sturdy_full >> 1) & 1 == 1,
            id,
            props,
            tags,
            step,
            volume: f.sounds.volume,
            pitch: f.sounds.pitch,
            collision,
            outline,
        });
        STATE.with(|s| s.borrow_mut().blocks.insert(state.0, info.clone()));
        info
    }

    /// The player you play, as an entity of the rules.
    fn pf_local(&self) -> Option<engine::Ent> {
        let p = self.player.as_ref()?;
        let model = self.player_model.as_ref()?;
        if p.health.dead_or_dying() || p.spectator || p.sleeping {
            return None;
        }
        // The fluids at the feet, the middle of the body and the eyes (the block's own fluid).
        let fluid_at = |y: f64| {
            let state = self.world.block(p.position.x.floor() as i32, y.floor() as i32, p.position.z.floor() as i32);
            self.res.facts().get(state).fluid
        };
        let eye = fluid_at(p.position.y + f64::from(p.dimensions.eye_height));
        let water = [fluid_at(p.position.y), fluid_at(p.position.y + 0.5)]
            .iter()
            .any(|f| matches!(f, horizon_items::blocks::Fluid::Water));
        let lava = [fluid_at(p.position.y), fluid_at(p.position.y + 0.5)]
            .iter()
            .any(|f| matches!(f, horizon_items::blocks::Fluid::Lava));
        let eye_water = matches!(eye, horizon_items::blocks::Fluid::Water);
        let eye_lava = matches!(eye, horizon_items::blocks::Fluid::Lava);
        Some(engine::Ent {
            id: model.id,
            type_id: "minecraft:player".into(),
            kind: engine::Kind::Local,
            pos: [p.position.x, p.position.y, p.position.z],
            vel: [p.velocity.x, p.velocity.y, p.velocity.z],
            yaw: p.yaw,
            width: p.dimensions.width,
            height: p.dimensions.height,
            scale: 1.0,
            on_ground: p.ground.on_ground,
            touching_water: water || p.swimming,
            submerged_water: eye_water,
            in_lava: lava,
            submerged_lava: lava && eye_lava,
            climbing: p.last_climbable.is_some() && !p.ground.on_ground,
            sprinting: p.sprinting,
            sneaking: matches!(p.pose, horizon_client::pose::Pose::Crouching),
            fall_distance: p.fall_distance as f32,
            jumping: p.jumping,
            boots: self.armor[3].as_ref().and_then(|s| boots_sound(&s.item)),
            move_dist: Some(p.steps.movement),
        })
    }

    /// A mob (an other player included) as an entity of the rules.
    fn pf_mob(&self, m: &Mob) -> Option<engine::Ent> {
        let id = &*m.kind.id;
        // getTargets: no water creatures, shulkers, armor stands, boats or minecarts; nothing riding or asleep.
        if !m.alive()
            || m.removed
            || m.extra.passenger
            || matches!(m.extra.avatar_pose, horizon_client::pose::Pose::Sleeping)
            || m.kind.category.contains("water")
            || m.kind.category == "axolotls"
            || matches!(id, "minecraft:shulker" | "minecraft:armor_stand" | "minecraft:ghast" | "minecraft:phantom")
        {
            return None;
        }
        let player = id == "minecraft:player";
        Some(engine::Ent {
            id: m.id,
            type_id: id.to_owned(),
            kind: if player {
                engine::Kind::OtherPlayer
            } else if m.kind.category == "monster" {
                engine::Kind::Hostile
            } else {
                engine::Kind::Passive
            },
            pos: [m.position.x, m.position.y, m.position.z],
            vel: [m.velocity.x, m.velocity.y, m.velocity.z],
            yaw: m.yaw,
            width: m.kind.width as f32,
            height: m.kind.height as f32,
            scale: 1.0,
            on_ground: m.on_ground,
            touching_water: m.fluids.in_water,
            submerged_water: m.fluids.in_water && m.was_eye_in_water,
            in_lava: m.fluids.in_lava,
            submerged_lava: false,
            climbing: m.climbing,
            sprinting: m.extra.sprinting,
            sneaking: m.extra.crouching,
            fall_distance: m.fall_distance as f32,
            jumping: false,
            boots: None,
            move_dist: None,
        })
    }

    /// Once a tick: the steps of everything near the camera (`SoundEngine.onFrame`).
    pub(super) fn pf_tick(&mut self) {
        use horizon_client::audio::presencefootsteps_gate as gate;
        if !self.native_on(ID) || !self.native_window() {
            gate::set_active(false);
            return;
        }
        let config = self.pf_config();
        let files_root = self.options.resources.clone();
        let reload = STATE.with(|s| {
            let s = s.borrow();
            s.engine.is_none() || s.loaded_from.as_deref() != Some(files_root.as_path())
        });
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.engine.is_none() {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(7, |d| d.as_nanos() as u64);
                s.engine = Some(engine::Engine::new(config.clone(), seed));
            }
            s.built_with = Some(config.clone());
        });
        if reload {
            let files = self.pf_files();
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.blocks.clear();
                s.loaded_from = Some(files_root);
                if let Some(e) = s.engine.as_mut() {
                    e.load(&files);
                }
            });
        }
        let single = !self.external_server();
        let running = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let e = s.engine.as_mut().expect("built above");
            e.config = config;
            e.running(single)
        });
        // The vanilla step sounds of the players give way only while the engine runs (`isStepBlocked`).
        gate::set_active(running);
        if !running || self.pause {
            return;
        }
        let mut ents: Vec<engine::Ent> = Vec::new();
        let local = self.pf_local();
        let camera = match &local {
            Some(l) => l.pos,
            None => return,
        };
        let local_id = local.as_ref().map(|l| l.id);
        ents.extend(local);
        for m in &self.mobs {
            if Some(m.id) == local_id && &*m.kind.id == "minecraft:player" {
                continue;
            }
            let near = (m.position.x - camera[0]).abs() < 16.0 && (m.position.z - camera[2]).abs() < 16.0;
            if near && let Some(e) = self.pf_mob(m) {
                ents.push(e);
            }
        }
        if let Some(host) = &self.lan {
            for m in host.models.values() {
                if Some(m.id) != local_id && let Some(e) = self.pf_mob(m) {
                    ents.push(e);
                }
            }
        }
        // The players the vanilla step sounds give way to.
        gate::set_players(
            ents.iter()
                .filter(|e| matches!(e.kind, engine::Kind::Local | engine::Kind::OtherPlayer))
                .map(|e| e.pos)
                .collect(),
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        let mut out = Vec::new();
        let live = Live { game: self };
        STATE.with(|s| {
            if let Some(e) = s.borrow_mut().engine.as_mut() {
                e.tick(&live, &ents, camera, now, &mut out);
            }
        });
        gate::pass(true);
        for s in out {
            self.audio.play(&s.sound, Some(s.pos), s.volume, s.pitch);
        }
        gate::pass(false);
    }

    /// The settings page: the four switches.
    pub(super) fn pf_settings(&self, mod_id: &str, buttons: &mut Vec<String>) {
        if mod_id != ID {
            return;
        }
        let c = self.pf_config();
        let on = |b: bool| if b { "ON" } else { "OFF" };
        buttons.push(format!("{}: {}", TOGGLES[0].1, on(!c.disabled)));
        buttons.push(format!("{}: {}", TOGGLES[1].1, on(c.multiplayer)));
        buttons.push(format!("{}: {}", TOGGLES[2].1, on(c.footwear)));
        buttons.push(format!("{}: {}%", TOGGLES[3].1, c.volume));
    }

    pub(super) fn pf_click(&self, mod_id: &str, button: usize) {
        if mod_id != ID {
            return;
        }
        let c = self.pf_config();
        match button {
            0 => self.native_set_setting(ID, "disabled", &(!c.disabled).to_string()),
            1 => self.native_set_setting(ID, "multiplayer", &(!c.multiplayer).to_string()),
            2 => self.native_set_setting(ID, "footwear", &(!c.footwear).to_string()),
            // PFOptionsScreen's slider, in steps of 10; past 100 it comes round to 0 (muted).
            3 => {
                let next = if c.volume >= 100 { 0 } else { (c.volume / 10 + 1) * 10 };
                self.native_set_setting(ID, "volume", &next.min(100).to_string());
            }
            _ => {}
        }
    }
}
