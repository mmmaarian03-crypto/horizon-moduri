// Overflowing Bars, the part that lives in `horizon_client::hud_layout` (library crate): it can see the HUD's draw
// calls but not the game. The game's side (`native/horizon.rs`, module `crate::game::overflowingbars`) hands it what
// only the game knows (the player's armor toughness, the sheets in the atlas, the settings) through `set_facts` and
// `configure`. Written to `mixins/overflowingbars/hud_runtime.rs` by `tools/bundle_hud.py`, with the engine pasted
// in place of the `include!` below.
//
// Hooks (mixins/overflowingbars/horizon-mixin.toml):
// - HEAD on `HealthHud::layout`: the original (the game's whole layout) runs inside, then the hearts and the armor
//   are taken out of its draws and the mod's bars are put in. `layout` has a closure, so RETURN is refused there.
// - RETURN on `experience`: the level number goes up 3 pixels (`GuiMixin.renderExperienceBar`).
// - HEAD and RETURN on `Chat::draw`, HEAD on `Chat::style_at` and `Chat::indicator_at`: the chat moves up for the
//   extra rows (`CustomizeChatPanelCallback`, `ChatComponentMixin.screenToChatY`).
use super::{Blend, GuiDraw, HealthHud, HealthInputs, HeartKind, WHITE, heart};
use super::{LegacyRandom, RandomSource};
use std::borrow::Cow;
use std::cell::Cell;
use std::ops::ControlFlow;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

#[allow(dead_code)]
mod engine {
    include!("overflowingbars_engine.rs");
}

/// The mod is on in Options > Mods and its sheets are in the GUI atlas (else the game's own bars stay).
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// `floor(getAttributeValue(ARMOR_TOUGHNESS))`, read each tick.
static TOUGHNESS: AtomicI32 = AtomicI32::new(0);
/// `ChatOffsetHelper.getChatOffsetY()`, read each tick.
static CHAT_OFFSET: AtomicI32 = AtomicI32::new(0);
static CONFIG: Mutex<Option<engine::Config>> = Mutex::new(None);

thread_local! {
    /// The original `layout` is running inside the hook: the hook lets it through.
    static INSIDE: Cell<bool> = const { Cell::new(false) };
    /// `batch.vertices.len()` when `Chat::draw` started.
    static CHAT_START: Cell<Option<usize>> = const { Cell::new(None) };
    /// The hook of `style_at` / `indicator_at` calls the original from inside.
    static CHAT_INSIDE: Cell<bool> = const { Cell::new(false) };
}

/// Sets a thread-local flag while it lives.
struct Inside(&'static std::thread::LocalKey<Cell<bool>>);
impl Inside {
    fn enter(key: &'static std::thread::LocalKey<Cell<bool>>) -> Self {
        key.with(|c| c.set(true));
        Self(key)
    }
}
impl Drop for Inside {
    fn drop(&mut self) {
        self.0.with(|c| c.set(false));
    }
}

fn config() -> engine::Config {
    CONFIG
        .lock()
        .ok()
        .and_then(|c| *c)
        .unwrap_or_else(engine::Config::tuned)
}

/// The game's side calls it once: the pack's values, then the files of the mod's settings folder.
pub fn configure(get: &dyn Fn(&str) -> Option<String>) {
    if let Ok(mut c) = CONFIG.lock() {
        *c = Some(engine::Config::read(get));
    }
}

/// The game's side calls it each client tick.
pub fn set_facts(on: bool, sheets: bool, toughness: i32, chat_offset: i32) {
    ACTIVE.store(on && sheets, Ordering::Relaxed);
    TOUGHNESS.store(toughness, Ordering::Relaxed);
    CHAT_OFFSET.store(chat_offset, Ordering::Relaxed);
}

/// The sheets the mod needs in the GUI atlas.
pub const SHEETS: [&str; 2] = [engine::ICONS, engine::NUMBERS];

/// `ChatOffsetHelper.getChatOffsetY()` as the game's side computes it from the player (`engine::chat_offset`).
pub fn chat_offset_for(max_health: f32, absorption: f32, armor: i32, toughness: i32) -> i32 {
    engine::chat_offset(&config(), max_health, absorption, armor, toughness)
}

fn kind(k: HeartKind) -> engine::Kind {
    match k {
        HeartKind::Normal => engine::Kind::Normal,
        HeartKind::Poisoned => engine::Kind::Poisoned,
        HeartKind::Withered => engine::Kind::Withered,
        HeartKind::Frozen => engine::Kind::Frozen,
    }
}

fn blit(texture: &'static str, x: i32, y: i32, size: [i32; 2], uv: [i32; 2], color: u32) -> GuiDraw {
    GuiDraw::Blit {
        texture: Cow::Borrowed(texture),
        blend: Blend::Textured,
        x,
        y,
        w: size[0],
        h: size[1],
        uv: [uv[0] as f32, uv[1] as f32],
        src: size,
        tex: [engine::SHEET, engine::SHEET],
        color,
    }
}

/// The engine's draw as the HUD's own draw call.
fn to_draw(d: engine::Draw) -> GuiDraw {
    match d {
        engine::Draw::Heart { ty, half, blink, hardcore, x, y } => {
            use engine::HeartType as T;
            let (kind, absorbing) = match ty {
                T::Container => (None, false),
                T::Normal | T::Orange => (Some(HeartKind::Normal), false),
                T::Poisoned => (Some(HeartKind::Poisoned), false),
                T::Withered => (Some(HeartKind::Withered), false),
                T::Frozen => (Some(HeartKind::Frozen), false),
                T::Absorbing => (Some(HeartKind::Normal), true),
            };
            GuiDraw::Sprite {
                sprite: Cow::Borrowed(heart(kind, absorbing, hardcore, half, blink)),
                x,
                y,
                w: 9,
                h: 9,
                region: None,
                color: WHITE,
                blend: Blend::Textured,
            }
        }
        engine::Draw::Icon { u, v, x, y } => blit(engine::ICONS, x, y, [9, 9], [u, v], WHITE),
        engine::Draw::Digit { u, v, x, y, color } => blit(engine::NUMBERS, x, y, [3, 5], [u, v], color),
        engine::Draw::Text { text, x, y, color } => GuiDraw::Text {
            text: crate::text::Text::literal(text),
            x,
            y,
            color,
            shadow: false,
        },
    }
}

/// HEAD on `HealthHud::layout`: the game's layout with the hearts and the armor swapped for the mod's bars.
pub fn layout(this: &mut HealthHud, inputs: &HealthInputs, w: i32, h: i32) -> ControlFlow<Vec<GuiDraw>> {
    if !ACTIVE.load(Ordering::Relaxed) || INSIDE.with(Cell::get) {
        return ControlFlow::Continue(());
    }
    let cfg = config();
    if !(cfg.health.allow_layers || cfg.armor.allow_layers || cfg.toughness_bar) {
        return ControlFlow::Continue(());
    }
    // `Hud.healthBlinkTime` before the layout updates it: the layout's own `blink`.
    let tick = i64::from(inputs.tick);
    let blink = this.health_blink_time > tick && (this.health_blink_time - tick) / 3 % 2 == 1;
    let mut draws = {
        let _inside = Inside::enter(&INSIDE);
        this.layout(inputs, w, h)
    };
    // The game's hearts and armor go when the mod draws them.
    draws.retain(|d| match d {
        GuiDraw::Sprite { sprite, .. } => {
            !((cfg.health.allow_layers && sprite.starts_with("hud/heart/"))
                || (cfg.armor.allow_layers && sprite.starts_with("hud/armor_")))
        }
        _ => true,
    });
    let player = engine::Player {
        health: inputs.health,
        max_health: inputs.max_health,
        absorption: inputs.absorption,
        display_health: this.display_health,
        blink,
        kind: kind(inputs.kind),
        regeneration: inputs.regeneration,
        hardcore: inputs.hardcore,
        tick: inputs.tick,
        armor: inputs.armor,
        toughness: TOUGHNESS.load(Ordering::Relaxed),
    };
    // The game's left height ends with its armor row: without it, what its health left.
    let after_health = this.left_height - if inputs.armor > 0 { 10 } else { 0 };
    // `RandomSource.create()` seeded with `tickCount * 312871` before the hearts.
    let mut random = LegacyRandom::new(i64::from(inputs.tick.wrapping_mul(312_871)));
    let mut next = |bound: i32| random.next_int_bound(bound).unwrap_or(0);
    // `Font.width` of digits and of `x` in the default font: 6 each.
    let text_width = |s: &str| s.chars().count() as i32 * 6;
    let bars = engine::bars(&cfg, &player, (w, h), this.right_height, after_health, &text_width, &mut next);
    let mut out: Vec<GuiDraw> = bars.draws.into_iter().map(to_draw).collect();
    out.append(&mut draws);
    this.left_height = bars.left_height;
    this.right_height = bars.right_height;
    ControlFlow::Break(out)
}

/// RETURN on `experience`: `moveExperienceAboveBar`, the level number 3 pixels up.
pub fn experience(mut value: Vec<GuiDraw>) -> Vec<GuiDraw> {
    if !ACTIVE.load(Ordering::Relaxed) || !config().move_experience {
        return value;
    }
    for d in &mut value {
        if let GuiDraw::Text { y, .. } = d {
            *y -= engine::EXPERIENCE_LEVEL_SHIFT;
        }
    }
    value
}

fn chat_shift() -> i32 {
    if ACTIVE.load(Ordering::Relaxed) && config().move_chat {
        CHAT_OFFSET.load(Ordering::Relaxed)
    } else {
        0
    }
}

/// HEAD on `Chat::draw`: where the chat's vertices start.
pub fn chat_begin(batch: &crate::hud::HudBatch) {
    CHAT_START.with(|c| c.set(Some(batch.vertices.len())));
}

/// RETURN on `Chat::draw`: the chat moves up by the offset (`posY - offset`).
pub fn chat_end(batch: &mut crate::hud::HudBatch) {
    let Some(start) = CHAT_START.with(Cell::take) else {
        return;
    };
    let shift = chat_shift() as f32;
    if shift == 0.0 || start > batch.vertices.len() {
        return;
    }
    for v in &mut batch.vertices[start..] {
        v.pos[1] -= shift;
    }
}

/// `screenToChatY(mouseY) + offset`: the mouse as the shifted chat sees it.
pub fn chat_mouse_y(y: f64) -> f64 {
    y + f64::from(chat_shift())
}

/// Whether the hook is the outermost call (the original's own call runs through it again).
pub fn chat_enter() -> Option<impl Drop> {
    if CHAT_INSIDE.with(Cell::get) || chat_shift() == 0 {
        None
    } else {
        Some(Inside::enter(&CHAT_INSIDE))
    }
}
