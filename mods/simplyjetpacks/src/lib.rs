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
/// (mods/forja/API.md section 6).
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
