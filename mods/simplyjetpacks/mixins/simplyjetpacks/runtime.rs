// Simply Jetpacks 2 inside Horizon's game (nightly 142388e1c). The rules are the engine (src/lib.rs).
//
// Where the original's code runs here (Horizon #1514: single player is an integrated server, the window its guest):
// - the side that moves the player (the window's client, or a host playing in the frame) runs `flyUser` right before
//   the player's movement tick (`sync_player_body`, as `Player.aiStep` runs `inventory.tick()` before `travel`): the
//   vertical speed, the sideways pushes, the particles. The client's copy of the jetpack is only read.
// - the server runs `flyUser` for every guest in its tick (`tick_guests`) with the guest's own input flags
//   (`ServerboundPlayerInputPacket`): the energy, no fall distance while the jetpack pushes, the emergency hover, the
//   potato's launch and explosion. A host in the frame does both in `sync_player_body`.
// - the keys (`KeybindForgeBusHandler`) are read on the window (`native_key`) and go to the server on forja's
//   channel `simplyjetpacks:key` (the original's toggle packets); the server changes the worn jetpack and answers on
//   the action bar.
// - the HUD (`HUDHandler`) is drawn on the window from its copy of the jetpack (forja syncs `custom_data`).
#[allow(unused_imports)]
use super::{Game, Result, Screen, lan::LanHost};
#[allow(dead_code)]
mod engine {
    // Simply Jetpacks 2 for Horizon: the rules, without any game code.
// Source: the pack's 1.20.1 jar SimplyJetpacks2-1.20.1-2.11.0.1 (Fernflower of JetpackItem, PotatoJetpackItem,
// JetpackType, JetpackArmorMaterial, ClientJetpackHandler, KeybindForgeBusHandler, SJTextUtil, HUDHandler,
// HUDRenderHelper) and his config (simplyjetpacks-common.toml / -client.toml). Pasted into the game as
// `crate::game::simplyjetpacks::engine` (tools/hnmod.py bundle) and into mod_content.rs (tools/bundle_extra.py); it is
// also the `horizon_simplyjetpacks` crate the tests use.

/// the pack's client config (simplyjetpacks-client.toml), the values the port uses.
pub mod config {
    pub const INVERT_HOVER_SNEAKING: bool = false; // [controls] invertHoverSneakingBehavior
    pub const ENABLE_PARTICLES: bool = true; // [visual] enableJetpackParticles
    pub const SHOW_THROTTLE: bool = true; // [gui] showThrottle
    pub const SHOW_EXACT_ENERGY: bool = false; // [gui] showExactEnergy
    pub const ENABLE_STATE_MESSAGES: bool = true; // [gui] enableStateMessages
    pub const ENABLE_HUD: bool = true; // [gui] enableJetpackHud
    pub const SHOW_HOVER_STATE: bool = true; // [gui] showHoverState
    pub const SHOW_EHOVER_STATE: bool = true; // [gui] showEHoverState
    pub const SHOW_CHARGER_STATE: bool = true; // [gui] showChargerState
    pub const HUD_TEXT_COLOR: u32 = 16777215; // [gui] hudTextColor
    pub const HUD_X_OFFSET: i32 = 0; // [gui] hudXOffset (hudTextPosition = "TOP_LEFT")
    pub const HUD_Y_OFFSET: i32 = 0; // [gui] hudYOffset
    pub const HUD_SCALE: i32 = 1; // [gui] hudScale
    pub const HUD_TEXT_SHADOW: bool = true; // [gui] hudTextShadow
}

/// One `[tuning.<mod>.<key>]` table of simplyjetpacks-common.toml (the pack's values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub key: &'static str,
    pub energy_capacity: i64,
    pub energy_usage: i64,
    pub energy_in: i64,
    pub energy_out: i64,
    pub enchantability: u32,
    pub speed_vertical: f64,
    pub accel_vertical: f64,
    pub speed_hover: f64,
    pub speed_hover_slow: f64,
    pub speed_sideways: f64,
    pub sprint_speed: f64,
    pub sprint_energy: f64,
    pub hover_mode: bool,
    pub emergency_hover: bool,
    pub charger: bool,
}

#[rustfmt::skip]
pub const TUNINGS: [Tuning; 19] = [
    Tuning { key: "vanilla1", energy_capacity: 80000, energy_usage: 32, energy_in: 400, energy_out: 0, enchantability: 4, speed_vertical: 0.22, accel_vertical: 0.1, speed_hover: 0.18, speed_hover_slow: 0.14, speed_sideways: 0.0, sprint_speed: 1.0, sprint_energy: 1.0, hover_mode: true, emergency_hover: false, charger: false },
    Tuning { key: "vanilla2", energy_capacity: 400000, energy_usage: 50, energy_in: 2000, energy_out: 0, enchantability: 8, speed_vertical: 0.3, accel_vertical: 0.12, speed_hover: 0.18, speed_hover_slow: 0.1, speed_sideways: 0.08, sprint_speed: 1.0, sprint_energy: 1.0, hover_mode: true, emergency_hover: false, charger: false },
    Tuning { key: "creative", energy_capacity: 0, energy_usage: 1000, energy_in: 0, energy_out: 1000, enchantability: 20, speed_vertical: 0.9, accel_vertical: 0.15, speed_hover: 0.45, speed_hover_slow: 0.0, speed_sideways: 0.21, sprint_speed: 2.5, sprint_energy: 0.0, hover_mode: true, emergency_hover: true, charger: true },
    Tuning { key: "vanilla3", energy_capacity: 4000000, energy_usage: 200, energy_in: 20000, energy_out: 0, enchantability: 13, speed_vertical: 0.48, accel_vertical: 0.13, speed_hover: 0.34, speed_hover_slow: 0.03, speed_sideways: 0.12, sprint_speed: 1.3, sprint_energy: 2.5, hover_mode: true, emergency_hover: true, charger: false },
    Tuning { key: "vanilla4", energy_capacity: 20000000, energy_usage: 450, energy_in: 50000, energy_out: 0, enchantability: 17, speed_vertical: 0.8, accel_vertical: 0.14, speed_hover: 0.4, speed_hover_slow: 0.005, speed_sideways: 0.16, sprint_speed: 1.9, sprint_energy: 4.5, hover_mode: true, emergency_hover: true, charger: true },
    Tuning { key: "potato", energy_capacity: 1200, energy_usage: 45, energy_in: 45, energy_out: 45, enchantability: 0, speed_vertical: 0.9, accel_vertical: 0.5, speed_hover: 0.18, speed_hover_slow: 0.14, speed_sideways: 0.0, sprint_speed: 1.0, sprint_energy: 1.0, hover_mode: false, emergency_hover: false, charger: false },
    Tuning { key: "te2", energy_capacity: 3000000, energy_usage: 50, energy_in: 8000, energy_out: 0, enchantability: 8, speed_vertical: 0.3, accel_vertical: 0.12, speed_hover: 0.18, speed_hover_slow: 0.1, speed_sideways: 0.08, sprint_speed: 1.1, sprint_energy: 1.5, hover_mode: true, emergency_hover: false, charger: false },
    Tuning { key: "te1", energy_capacity: 800000, energy_usage: 32, energy_in: 1500, energy_out: 0, enchantability: 4, speed_vertical: 0.22, accel_vertical: 0.1, speed_hover: 0.18, speed_hover_slow: 0.14, speed_sideways: 0.0, sprint_speed: 1.0, sprint_energy: 1.0, hover_mode: true, emergency_hover: false, charger: false },
    Tuning { key: "te4", energy_capacity: 25000000, energy_usage: 450, energy_in: 20000, energy_out: 0, enchantability: 17, speed_vertical: 0.8, accel_vertical: 0.14, speed_hover: 0.4, speed_hover_slow: 0.005, speed_sideways: 0.17, sprint_speed: 1.8, sprint_energy: 4.0, hover_mode: true, emergency_hover: true, charger: false },
    Tuning { key: "te3", energy_capacity: 6000000, energy_usage: 200, energy_in: 15000, energy_out: 0, enchantability: 13, speed_vertical: 0.48, accel_vertical: 0.13, speed_hover: 0.34, speed_hover_slow: 0.03, speed_sideways: 0.12, sprint_speed: 1.3, sprint_energy: 2.3, hover_mode: true, emergency_hover: true, charger: false },
    Tuning { key: "te6", energy_capacity: 50000000, energy_usage: 850, energy_in: 30000, energy_out: 30000, enchantability: 20, speed_vertical: 0.9, accel_vertical: 0.15, speed_hover: 0.45, speed_hover_slow: 0.0, speed_sideways: 0.2, sprint_speed: 2.4, sprint_energy: 6.0, hover_mode: true, emergency_hover: true, charger: true },
    Tuning { key: "te5", energy_capacity: 50000000, energy_usage: 850, energy_in: 30000, energy_out: 30000, enchantability: 20, speed_vertical: 0.9, accel_vertical: 0.15, speed_hover: 0.45, speed_hover_slow: 0.0, speed_sideways: 0.2, sprint_speed: 2.2, sprint_energy: 5.5, hover_mode: true, emergency_hover: true, charger: true },
    Tuning { key: "mek2", energy_capacity: 400000, energy_usage: 50, energy_in: 2000, energy_out: 0, enchantability: 8, speed_vertical: 0.3, accel_vertical: 0.12, speed_hover: 0.18, speed_hover_slow: 0.1, speed_sideways: 0.05, sprint_speed: 1.1, sprint_energy: 1.8, hover_mode: true, emergency_hover: false, charger: false },
    Tuning { key: "mek1", energy_capacity: 80000, energy_usage: 32, energy_in: 400, energy_out: 0, enchantability: 4, speed_vertical: 0.22, accel_vertical: 0.1, speed_hover: 0.18, speed_hover_slow: 0.14, speed_sideways: 0.0, sprint_speed: 1.0, sprint_energy: 1.0, hover_mode: true, emergency_hover: false, charger: false },
    Tuning { key: "mek4", energy_capacity: 20000000, energy_usage: 450, energy_in: 50000, energy_out: 0, enchantability: 17, speed_vertical: 0.8, accel_vertical: 0.14, speed_hover: 0.4, speed_hover_slow: 0.005, speed_sideways: 0.15, sprint_speed: 1.9, sprint_energy: 4.5, hover_mode: true, emergency_hover: true, charger: false },
    Tuning { key: "mek3", energy_capacity: 4000000, energy_usage: 200, energy_in: 20000, energy_out: 0, enchantability: 13, speed_vertical: 0.48, accel_vertical: 0.13, speed_hover: 0.34, speed_hover_slow: 0.03, speed_sideways: 0.1, sprint_speed: 1.4, sprint_energy: 2.8, hover_mode: true, emergency_hover: true, charger: false },
    Tuning { key: "ie1", energy_capacity: 80000, energy_usage: 32, energy_in: 400, energy_out: 0, enchantability: 4, speed_vertical: 0.22, accel_vertical: 0.1, speed_hover: 0.18, speed_hover_slow: 0.14, speed_sideways: 0.0, sprint_speed: 1.0, sprint_energy: 1.0, hover_mode: true, emergency_hover: false, charger: false },
    Tuning { key: "ie3", energy_capacity: 4000000, energy_usage: 200, energy_in: 20000, energy_out: 0, enchantability: 13, speed_vertical: 0.48, accel_vertical: 0.13, speed_hover: 0.34, speed_hover_slow: 0.03, speed_sideways: 0.13, sprint_speed: 1.4, sprint_energy: 3.5, hover_mode: true, emergency_hover: true, charger: false },
    Tuning { key: "ie2", energy_capacity: 400000, energy_usage: 50, energy_in: 2000, energy_out: 0, enchantability: 8, speed_vertical: 0.3, accel_vertical: 0.12, speed_hover: 0.18, speed_hover_slow: 0.1, speed_sideways: 0.08, sprint_speed: 1.2, sprint_energy: 2.0, hover_mode: true, emergency_hover: false, charger: false },
];

/// `JetpackArmorMaterial` (defense array indexed by `ArmorItem.Type.ordinal()`, CHESTPLATE = 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Jetpack,
    Armored,
    Potato,
    Jetplate,
}
impl Material {
    /// (defense of the chest piece, toughness, knockback resistance, material enchantability).
    pub fn numbers(self) -> (f64, f64, f64, u32) {
        match self {
            Material::Jetpack => (2.0, 0.0, 0.0, 10),
            Material::Armored => (4.0, 0.0, 0.0, 10),
            Material::Potato => (0.0, 0.0, 0.0, 0),
            Material::Jetplate => (12.0, 3.0, 3.0, 10),
        }
    }
}

/// `JetpackType` with the item it is registered as (`RegistryHandler`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kind {
    pub item: &'static str,
    pub name: &'static str,
    pub tier: u32,
    pub config: &'static str,
    pub material: Material,
}
const fn k(item: &'static str, name: &'static str, tier: u32, config: &'static str, material: Material) -> Kind {
    Kind { item, name, tier, config, material }
}
use Material::{Armored, Jetpack, Jetplate, Potato};
#[rustfmt::skip]
pub const KINDS: [Kind; 35] = [
    k("jetpack_potato", "potato", 1, "potato", Potato),
    k("jetpack_creative", "creative", 5, "creative", Jetpack),
    k("jetpack_creative_armored", "creative_armored", 5, "creative", Armored),
    k("jetpack_vanilla1", "vanilla1", 1, "vanilla1", Jetpack), k("jetpack_vanilla1_armored", "vanilla1_armored", 1, "vanilla1", Armored),
    k("jetpack_vanilla2", "vanilla2", 2, "vanilla2", Jetpack), k("jetpack_vanilla2_armored", "vanilla2_armored", 2, "vanilla2", Armored),
    k("jetpack_vanilla3", "vanilla3", 3, "vanilla3", Jetpack), k("jetpack_vanilla3_armored", "vanilla3_armored", 3, "vanilla3", Armored),
    k("jetpack_vanilla4", "vanilla4", 4, "vanilla4", Jetpack), k("jetpack_vanilla4_armored", "vanilla4_armored", 4, "vanilla4", Armored),
    k("jetpack_ie1", "ie1", 1, "ie1", Jetpack), k("jetpack_ie1_armored", "ie1_armored", 1, "ie1", Armored),
    k("jetpack_ie2", "ie2", 2, "ie2", Jetpack), k("jetpack_ie2_armored", "ie2_armored", 2, "ie2", Armored),
    k("jetpack_ie3", "ie3", 3, "ie3", Jetpack), k("jetpack_ie3_armored", "ie3_armored", 3, "ie3", Armored),
    k("jetpack_mek1", "mek1", 1, "mek1", Jetpack), k("jetpack_mek1_armored", "mek1_armored", 1, "mek1", Armored),
    k("jetpack_mek2", "mek2", 2, "mek2", Jetpack), k("jetpack_mek2_armored", "mek2_armored", 2, "mek2", Armored),
    k("jetpack_mek3", "mek3", 3, "mek3", Jetpack), k("jetpack_mek3_armored", "mek3_armored", 3, "mek3", Armored),
    k("jetpack_mek4", "mek4", 4, "mek4", Jetpack), k("jetpack_mek4_armored", "mek4_armored", 4, "mek4", Armored),
    k("jetpack_te1", "te1", 1, "te1", Jetpack), k("jetpack_te1_armored", "te1_armored", 1, "te1", Armored),
    k("jetpack_te2", "te2", 2, "te2", Jetpack), k("jetpack_te2_armored", "te2_armored", 2, "te2", Armored),
    k("jetpack_te3", "te3", 3, "te3", Jetpack), k("jetpack_te3_armored", "te3_armored", 3, "te3", Armored),
    k("jetpack_te4", "te4", 4, "te4", Jetpack), k("jetpack_te4_armored", "te4_armored", 4, "te4", Armored),
    // `TE5("te5", 5, "te5", true)` and `TE5_ARMORED("te5_enderium", 5, "te5", true)`, both with JETPLATE.
    k("jetpack_te5", "te5", 5, "te5", Jetplate), k("jetpack_te5_enderium", "te5_enderium", 5, "te5", Jetplate),
];

pub const NS: &str = "simplyjetpacks";

pub fn kind(item: &str) -> Option<&'static Kind> {
    let path = item.strip_prefix("simplyjetpacks:")?;
    KINDS.iter().find(|k| k.item == path)
}
pub fn tuning(kind: &Kind) -> &'static Tuning {
    TUNINGS.iter().find(|t| t.key == kind.config).unwrap_or(&TUNINGS[0])
}
impl Kind {
    /// `JetpackItem.isCreative`: the name contains "creative".
    pub fn creative(&self) -> bool {
        self.name.contains("creative")
    }
    pub fn potato(&self) -> bool {
        self.name == "potato"
    }
    /// `JetpackType.getRarity`: tier 3 uncommon, 4 rare, 5 epic, else common.
    pub fn rarity(&self) -> &'static str {
        match self.tier {
            3 => "uncommon",
            4 => "rare",
            5 => "epic",
            _ => "common",
        }
    }
    /// `ArmorItem.getEnchantmentValue` (the material's) + `JetpackType.getEnchantability`.
    pub fn enchantability(&self) -> u32 {
        self.material.numbers().3 + tuning(self).enchantability
    }
    /// `JetpackType.getDefaultParticles`: flame, the rainbow for the creative and the potato jetpacks.
    pub fn default_particle(&self) -> Particle {
        if !self.creative() && !self.potato() {
            Particle::Flame
        } else {
            Particle::Rainbow
        }
    }
}

/// `JetpackParticleType` in ordinal order (the stack's `ParticleID`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Particle {
    None,
    Flame,
    Smoke,
    Rainbow,
    Bubbles,
    Hearts,
    Soul,
    Snow,
}
pub const PARTICLES: [Particle; 8] = [
    Particle::None,
    Particle::Flame,
    Particle::Smoke,
    Particle::Rainbow,
    Particle::Bubbles,
    Particle::Hearts,
    Particle::Soul,
    Particle::Snow,
];

/// The jetpack's NBT (`Engine`, `Hover`, `EmergencyHover`, `Charger`, `Energy`, `Throttle`, `ParticleID`, the potato's
/// `Fired`, `RocketTimerSet`, `RocketTimer`). Here they live in `minecraft:custom_data`, energy as `forja:energy`
/// (FUNDATIE-FORJA section 6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pack {
    pub engine: bool,
    pub hover: bool,
    pub ehover: bool,
    pub charger: bool,
    pub energy: i64,
    /// None = never set: `getThrottle` gives 100.
    pub throttle: Option<i32>,
    pub particle: Option<i32>,
    pub fired: bool,
    pub timer_set: bool,
    pub timer: i32,
}
impl Pack {
    pub fn throttle(&self) -> i32 {
        self.throttle.unwrap_or(100)
    }
    pub fn particle(&self, kind: &Kind) -> Particle {
        self.particle
            .and_then(|i| PARTICLES.get(usize::try_from(i).ok()?).copied())
            .unwrap_or_else(|| kind.default_particle())
    }
}

/// The player's input and body, as `flyUser` reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub up: bool,
    pub down: bool,
    pub forwards: bool,
    pub backwards: bool,
    pub left: bool,
    pub right: bool,
    /// `CommonJetpackHandler.isInverted` (the client's invertHoverSneakingBehavior).
    pub inverted: bool,
    pub sprinting: bool,
    pub crouching: bool,
    pub in_water: bool,
    pub on_ground: bool,
    pub vy: f64,
}

/// What one `flyUser` call does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fly {
    /// The new vertical speed (`fly(player, y)`), when the jetpack pushes.
    pub vy: Option<f64>,
    /// `moveRelative(1.0, (x, 0, z))` calls, in order.
    pub pushes: Vec<[f64; 2]>,
    /// Energy taken (`useEnergy`).
    pub used: i64,
    /// The jetpack pushed: fallDistance = 0 and no flight kick on the server.
    pub pushed: bool,
}

/// `JetpackItem.getEnergyUsage`: Fuel Efficiency level L uses `round(base * (5 - L) / 5)`.
pub fn energy_usage(base: i64, fuel_efficiency: u32) -> i64 {
    if fuel_efficiency == 0 {
        base
    } else {
        ((base * (5 - i64::from(fuel_efficiency.min(5)))) as f64 / 5.0).round() as i64
    }
}

/// `JetpackItem.flyUser` (the engine part; `force` is the potato's launch).
pub fn fly(kind: &Kind, pack: &mut Pack, input: &Input, force: bool, fuel_efficiency: u32) -> Fly {
    let t = tuning(kind);
    let mut out = Fly::default();
    if !pack.engine {
        return out;
    }
    let hover = pack.hover;
    let hover_speed = if input.inverted == input.down {
        t.speed_hover_slow
    } else {
        t.speed_hover
    };
    let fly_key = force || input.up;
    let descend = input.down;
    let accel = t.accel_vertical * if input.vy < 0.3 { 2.5 } else { 1.0 };
    let speed_vertical = t.speed_vertical * if input.in_water { 0.4 } else { 1.0 };
    if !(fly_key || hover && !input.on_ground) {
        return out;
    }
    if !kind.creative() {
        let usage = energy_usage(t.energy_usage, fuel_efficiency);
        let amount = if input.sprinting {
            (usage as f64 * t.sprint_energy).round() as i64
        } else {
            usage
        };
        // useEnergy: min(stored, capacity) - amount, not below 0.
        let stored = pack.energy.min(t.energy_capacity);
        let left = (stored - amount).max(0);
        out.used = pack.energy - left;
        pack.energy = left;
    }
    if pack.energy <= 0 && !kind.creative() {
        return out;
    }
    let target = if fly_key {
        if !hover {
            speed_vertical
        } else if descend {
            -t.speed_hover_slow
        } else {
            t.speed_hover
        }
    } else {
        -hover_speed
    };
    out.vy = Some((input.vy + accel).min(target));
    let throttle = pack.throttle() as f32 / 100.0;
    let sideways = (if input.crouching { t.speed_sideways * 0.5 } else { t.speed_sideways }) as f32 * throttle;
    let forward = (if input.sprinting { f64::from(sideways) * t.sprint_speed } else { f64::from(sideways) }) as f32 * throttle;
    if input.forwards {
        out.pushes.push([0.0, f64::from(forward)]);
    }
    if input.backwards {
        out.pushes.push([0.0, f64::from(-sideways * 0.8)]);
    }
    if input.left {
        out.pushes.push([f64::from(sideways), 0.0]);
    }
    if input.right {
        out.pushes.push([f64::from(-sideways), 0.0]);
    }
    out.pushed = true;
    out
}

/// `Entity.moveRelative(1.0, input)` -> `getInputVector`: the push in world axes for body yaw `yaw` (degrees).
pub fn move_relative(input: [f64; 2], yaw: f32) -> [f64; 2] {
    let (x, z) = (input[0], input[1]);
    let len = x * x + z * z;
    if len < 1.0e-7 {
        return [0.0, 0.0];
    }
    let (x, z) = if len > 1.0 {
        let l = len.sqrt();
        (x / l, z / l)
    } else {
        (x, z)
    };
    let r = yaw * (std::f32::consts::PI / 180.0);
    let (sin, cos) = (f64::from(r.sin()), f64::from(r.cos()));
    [x * cos - z * sin, z * cos + x * sin]
}

/// The emergency hover check of `flyUser` (server only): engine and hover go on when the player falls into the
/// void or would take a deadly fall.
pub struct Danger {
    pub y: f64,
    pub fall_distance: f64,
    pub health: f64,
    pub creative_mode: bool,
    pub on_ground: bool,
    pub in_water: bool,
}
pub fn emergency(kind: &Kind, pack: &Pack, d: &Danger) -> bool {
    let t = tuning(kind);
    if !(pack.ehover && (pack.energy > 0 || kind.creative()) && (!pack.hover || !pack.engine)) {
        return false;
    }
    let danger = d.y < -69.0
        || (!d.creative_mode && d.fall_distance as f32 - 1.2 >= d.health as f32 && !d.on_ground && !d.in_water);
    // doEHover only acts when the type has hover mode.
    danger && t.hover_mode
}
pub fn do_emergency(pack: &mut Pack) {
    pack.engine = true;
    pack.hover = true;
}

/// The keys of `KeybindForgeBusHandler` (the packets they send).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Engine,
    Hover,
    EHover,
    Charger,
    ThrottleUp,
    ThrottleDown,
}
impl Action {
    pub fn code(self) -> u8 {
        self as u8
    }
    pub fn from_code(c: u8) -> Option<Self> {
        [Action::Engine, Action::Hover, Action::EHover, Action::Charger, Action::ThrottleUp, Action::ThrottleDown]
            .get(usize::from(c))
            .copied()
    }
    /// (setting key, title, default GLFW key name) for Options > Mods (`KeybindModBusHandler`).
    pub fn key(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Action::Engine => ("engine", "keybind.simplyjetpacks.jetpack_engine", "key.keyboard.j"),
            Action::Hover => ("hover", "keybind.simplyjetpacks.jetpack_hover", "key.keyboard.h"),
            Action::EHover => ("ehover", "keybind.simplyjetpacks.jetpack_ehover", "key.keyboard.unknown"),
            Action::Charger => ("charger", "keybind.simplyjetpacks.jetpack_charger", "key.keyboard.unknown"),
            Action::ThrottleUp => ("throttle_up", "keybind.simplyjetpacks.jetpack_throttle_increase", "key.keyboard.period"),
            Action::ThrottleDown => ("throttle_down", "keybind.simplyjetpacks.jetpack_throttle_decrease", "key.keyboard.comma"),
        }
    }
}
pub const ACTIONS: [Action; 6] =
    [Action::Engine, Action::Hover, Action::EHover, Action::Charger, Action::ThrottleUp, Action::ThrottleDown];

/// The action bar line of a toggle (`SJTextUtil.getStateToggle`): the chat key and the new state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toggled {
    pub key: &'static str,
    pub on: bool,
}

/// The server's handling of a key packet (`PacketToggleEngine` ... `PacketUpdateThrottle`) on the worn jetpack.
pub fn apply(kind: &Kind, pack: &mut Pack, action: Action) -> Option<Toggled> {
    let t = tuning(kind);
    let flip = |v: &mut bool, key| {
        *v = !*v;
        Some(Toggled { key, on: *v })
    };
    match action {
        Action::Engine => flip(&mut pack.engine, "chat.simplyjetpacks.itemJetpack.engineMode"),
        Action::Hover if t.hover_mode => flip(&mut pack.hover, "chat.simplyjetpacks.itemJetpack.hoverMode"),
        Action::EHover if t.emergency_hover => {
            flip(&mut pack.ehover, "chat.simplyjetpacks.itemJetpack.emergencyHoverMode")
        }
        Action::Charger if t.charger => flip(&mut pack.charger, "chat.simplyjetpacks.itemJetpack.chargerMode"),
        Action::ThrottleUp => {
            pack.throttle = Some((pack.throttle() + 10).clamp(0, 100));
            None
        }
        Action::ThrottleDown => {
            pack.throttle = Some((pack.throttle() - 10).clamp(0, 100));
            None
        }
        _ => None,
    }
}

/// `ClientJetpackHandler.isFlying`: the particles and the sound.
pub fn is_flying(kind: &Kind, pack: &Pack, up: bool, on_ground: bool) -> bool {
    if kind.potato() {
        return pack.fired;
    }
    if pack.engine && (pack.energy > 0 || kind.creative()) {
        if pack.hover { !on_ground } else { up }
    } else {
        false
    }
}

/// What the potato jetpack does in one call of its `flyUser`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PotatoStep {
    pub fly: Fly,
    /// The launch sound (`ROCKET_SOUND`) plays.
    pub launched: bool,
    /// Out of energy while fired: the jetpack is removed, an explosion (4, no block damage), a baked potato, 100 damage.
    pub explodes: bool,
}
/// `PotatoJetpackItem.flyUser`.
pub fn potato(kind: &Kind, pack: &mut Pack, input: &Input, force: bool) -> PotatoStep {
    let mut out = PotatoStep::default();
    if !pack.engine {
        return out;
    }
    if pack.fired {
        out.fly = fly(kind, pack, input, true, 0);
        if pack.energy <= 0 {
            out.explodes = true;
        }
    } else if force || input.up {
        if pack.timer_set {
            pack.timer = (pack.timer - 1).max(0);
            if pack.timer == 0 {
                pack.fired = true;
                out.launched = true;
            }
        } else {
            pack.timer = 50;
            pack.timer_set = true;
        }
    }
    out
}

// ---------------------------------------------------------------- HUD (HUDHandler + SJTextUtil.addHUDInfoText)

/// `SJTextUtil.getColoredPercent` colour code and the percent (`ceil(energy / capacity * 100)`).
pub fn energy_percent(energy: i64, capacity: i64) -> i64 {
    if capacity <= 0 {
        return 0;
    }
    (energy as f64 / capacity as f64 * 100.0).ceil() as i64
}
pub fn percent_color(percent: i64) -> char {
    match percent {
        p if p > 70 => 'a',
        p if p > 40 => 'e',
        p if p > 10 => '6',
        _ => 'c',
    }
}

/// The HUD lines with `§` colour codes; `tr` is `I18n.get` with `%s` arguments.
pub fn hud_lines(kind: &Kind, pack: &Pack, tr: &dyn Fn(&str, &[String]) -> String) -> Vec<String> {
    let t = tuning(kind);
    let mut lines = Vec::new();
    if config::SHOW_THROTTLE {
        lines.push(tr("hud.simplyjetpacks.throttle", &[pack.throttle().to_string()]));
    }
    let energy = if kind.creative() {
        format!("§d{}§r", tr("hud.simplyjetpacks.infiniteEnergy", &[]))
    } else {
        let p = energy_percent(pack.energy, t.energy_capacity);
        if p > 0 {
            format!("§{}{p}%§r", percent_color(p))
        } else {
            format!("§c{}§r", tr("hud.simplyjetpacks.energyDepleted", &[]))
        }
    };
    lines.push(tr("hud.simplyjetpacks.energyDisplay", &[energy]));
    let state = |key: &str, on: bool| format!("§{}{}§r", if on { 'a' } else { 'c' }, tr(key, &[]));
    let mut states = vec![state("hud.simplyjetpacks.engine", pack.engine)];
    if config::SHOW_HOVER_STATE && t.hover_mode {
        states.push(state("hud.simplyjetpacks.hover", pack.hover));
    }
    if config::SHOW_EHOVER_STATE && t.emergency_hover {
        states.push(state("hud.simplyjetpacks.eHover", pack.ehover));
    }
    if config::SHOW_CHARGER_STATE && t.charger {
        states.push(state("hud.simplyjetpacks.charger", pack.charger));
    }
    lines.push(tr(&format!("hud.simplyjetpacks.jetpackStates.{}", states.len()), &states));
    lines
}

/// `%s` / `%n$s` filled in order (`TranslatableContents`).
pub fn fill(pattern: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut rest = pattern;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        if let Some(t) = tail.strip_prefix('%') {
            out.push('%');
            rest = t;
        } else if let Some(t) = tail.strip_prefix('s') {
            out.push_str(args.get(next).map_or("", |s| s.as_str()));
            next += 1;
            rest = t;
        } else if let Some((n, t)) = tail.split_once("$s").filter(|(n, _)| n.parse::<usize>().is_ok()) {
            let n: usize = n.parse().unwrap_or(1);
            out.push_str(args.get(n.saturating_sub(1)).map_or("", |s| s.as_str()));
            rest = t;
        } else {
            out.push('%');
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

/// `ClientJetpackHandler.showJetpackParticles`: the nozzle offsets (left, right, centre) before the body
/// rotation, from the eye-ish point 1.5 above the feet; `centre_x` is `(rand - 0.5) * 0.25`.
pub fn nozzles(sneaking: bool, centre_x: f64, hearts: bool) -> Vec<[f64; 3]> {
    let (dz, dy) = if sneaking { (-0.3, -0.1) } else { (0.0, 0.0) };
    let mut v = vec![[-0.18, -0.9 + dy, -0.3 + dz], [0.18, -0.9 + dy, -0.3 + dz]];
    if !hearts {
        v.push([centre_x, -0.9 + dy, -0.3 + dz]);
    }
    v
}
/// `Pos3D.rotate(yaw, 0, 0)`: x' = x cos - z sin, z' = x sin + z cos (yaw in degrees).
pub fn rotate_yaw(v: [f64; 3], yaw: f32) -> [f64; 3] {
    let r = f64::from(yaw).to_radians();
    let (s, c) = (r.sin(), r.cos());
    [v[0] * c - v[2] * s, v[1], v[0] * s + v[2] * c]
}

}
use horizon_client::items::ItemStack;
use horizon_core::random::RandomSource;
use horizon_world::nbt::{JavaString, Tag};
use std::cell::RefCell;
use std::collections::HashSet;
use winit::keyboard::KeyCode;

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

pub(super) const ID: &str = "simplyjetpacks";
pub(super) const TITLE: &str = "Simply Jetpacks 2";
pub(super) const DESCRIPTION: &str = "Jetpacks with energy, hover and emergency hover (port of Simply Jetpacks 2 1.20.1).";

/// `PacketToggleEngine` ... `PacketUpdateThrottle`: the action's code.
const KEY: super::forja::Channel<u32> = super::forja::Channel::new("simplyjetpacks:key");

thread_local! {
    /// The guests whose jetpack pushed in this server tick (no fall damage for them).
    static PUSHED: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
}

fn register() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::forja::register_server_channel(KEY.id, key_from_client);
}

/// A guest's client pressed a jetpack key: the server acts on the guest's worn jetpack, in its turn.
fn key_from_client(game: &mut Game<'_>, host: &mut LanHost, conn: u32, body: &[u8]) {
    let Some(action) = KEY.decode(body).and_then(|c| engine::Action::from_code(c as u8)) else {
        return;
    };
    game.as_guest(host, conn, |g| g.simplyjetpacks_act(action));
}

// ---- the jetpack's NBT in custom_data (energy = forja:energy, FUNDATIE-FORJA section 6)

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
        energy: get_num(s, "forja:energy").unwrap_or(0).max(0),
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
    put("forja:energy", Some(Tag::Int(p.energy.clamp(0, i64::from(i32::MAX)) as i32)));
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
                let _ = super::forja::send_to_server(self, KEY.id, &KEY.encode(&u32::from(action.code())));
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
