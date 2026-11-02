// Tiny Item Animations inside Horizon's game (nightly 142388e1c136). The rules are the engine
// below (src/lib.rs); this part hears the clicks of the menus and scales the item icons that the
// game draws into the HUD batch.
//
// The original scales the pose of the item it is about to draw (a mixin on `renderSlot` and on
// `GuiGraphics.renderItem`). Horizon draws items as vertices in a `HudBatch`, so the same scale is
// applied to the vertices of that one item after the game has drawn it, about the same centre: the
// hook draws the item itself (one nested call of the game's own function) and keeps the game from
// drawing it a second time. Horizon cannot be hooked at `draw_slot_item` (it contains a macro), so
// a slot is recognised by where its item is drawn: the screen lists its slots and their places at
// the start of the frame.
use super::{Game, Screen};
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::menu::Input;
use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;

#[allow(dead_code)]
mod engine {
    // Tiny Item Animations for Horizon: the animation rules, without any game code.
// Written from the behaviour of Tiny Item Animations 1.1 for Forge 1.20 (Trivaxy; the build in
// the pack is `tia-1.20-1.1-forge.jar`), read from the mod's mixin classes and `tia.toml` of the
// pack; no code of the original is used. The source's licence (LPUL) only restricts hosting it on
// public platforms; nothing of it is published here.
use std::collections::BTreeMap;

/// The options of `tia.toml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `animation_speed` (0.01 to 10): how much of an animation one frame plays, per unit of the
    /// frame's partial tick.
    pub animation_speed: f32,
    /// `pickup_scale` (0.01 to 10): the size an item has when it is carried, or as it is put
    /// down.
    pub pickup_scale: f32,
}
impl Default for Settings {
    /// the pack's `config/tia.toml`.
    fn default() -> Self {
        Self {
            animation_speed: 0.5,
            pickup_scale: 1.4,
        }
    }
}

/// `1 + (pickup_scale - 1) * (1 - (1 - progress)^5)`: how large an item is drawn at `progress`
/// (0 to 1). It leaps toward the full size at first and settles slowly: at 0.5 it has gone 97% of
/// the way. Zero is the normal size, one is `pickup_scale`.
pub fn eased_scale(pickup_scale: f32, progress: f32) -> f32 {
    let left = f64::from(1.0 - progress);
    1.0 + (pickup_scale - 1.0) * (1.0 - left.powf(5.0) as f32)
}

/// One slot's animation: what it has left, and the first frame whose step it has not had yet.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Running {
    progress: f32,
    next_step: u64,
}

/// What the mod remembers: the animation of every slot of the open menu, and of the carried item.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tia {
    pub settings: Settings,
    /// `Animated.animationProgress` of the slots that have some left, 1 down to 0.
    slots: BTreeMap<usize, Running>,
    /// `TiaMod.carriedAnimationProgress`, 0 up to 1.
    carried: f32,
    /// The partial tick of the frame before the one that began last.
    last_partial: f32,
}
impl Tia {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    /// `AbstractContainerMenu.doClick` after `tryItemClickBehaviourOverride`: any plain
    /// left or right click (`PICKUP`, button 0 or 1) on a slot (not outside the window) starts the
    /// slot's animation from the top and the carried item's from the start. Quick moves, swaps,
    /// throws, drags and double clicks do not. `frame` is the next frame to be drawn.
    pub fn click(&mut self, slot: Option<usize>, button: i32, pickup: bool, frame: u64) {
        if pickup && (button == 0 || button == 1) && let Some(slot) = slot {
            self.slots.insert(
                slot,
                Running {
                    progress: 1.0,
                    next_step: frame,
                },
            );
            self.carried = 0.0;
        }
    }

    /// A frame of a menu begins. The original steps every slot after drawing it, empty slots
    /// included; a slot nothing was drawn for (it holds nothing) takes the step of the frame before
    /// this one now, with that frame's partial tick. `partial` is this frame's.
    pub fn begin_frame(&mut self, frame: u64, partial: f32) {
        let step = self.last_partial * self.settings.animation_speed;
        self.slots.retain(|_, s| {
            if s.next_step < frame {
                s.progress = (s.progress - step).max(0.0);
                s.next_step = frame;
            }
            s.progress > 0.0
        });
        self.last_partial = partial;
    }

    /// A slot's item is drawn (`renderSlot`): the size of the item now, from the progress the slot
    /// has, and then one step of its animation (`postRenderSlot`), by `partial` (the frame's
    /// partial tick) times the speed. Slots with no animation draw at the normal size.
    pub fn draw_slot(&mut self, slot: usize, partial: f32, frame: u64) -> f32 {
        let Some(running) = self.slots.get_mut(&slot) else {
            return 1.0;
        };
        let scale = eased_scale(self.settings.pickup_scale, running.progress);
        if running.next_step <= frame {
            running.progress = (running.progress - partial * self.settings.animation_speed).max(0.0);
            running.next_step = frame + 1;
        }
        if running.progress <= 0.0 {
            self.slots.remove(&slot);
        }
        scale
    }

    /// The carried item is drawn (`preRenderFloatingItem`): one step of its animation first, up to
    /// one, then the size it is drawn at.
    pub fn draw_carried(&mut self, partial: f32) -> f32 {
        self.carried = (self.carried + partial * self.settings.animation_speed).min(1.0);
        eased_scale(self.settings.pickup_scale, self.carried)
    }

    /// The screen closed: the next menu has new slots.
    pub fn closed(&mut self) {
        self.slots.clear();
        self.last_partial = 0.0;
    }

    /// The progress a slot has left, for tests.
    pub fn slot_progress(&self, slot: usize) -> f32 {
        self.slots.get(&slot).map_or(0.0, |s| s.progress)
    }
    /// The carried item's progress, for tests.
    pub fn carried_progress(&self) -> f32 {
        self.carried
    }
}

}

/// The slots of the screen being drawn and where their items are drawn.
#[derive(Default)]
struct Frame {
    number: u64,
    slots: Vec<(usize, f32, f32)>,
}

thread_local! {
    static TIA: RefCell<engine::Tia> = RefCell::new(engine::Tia::default());
    static FRAME: RefCell<Frame> = RefCell::new(Frame::default());
    /// The carried stack `draw_carried` is about to hand to `draw_item`: the frame, the corner it
    /// is drawn at, the scale.
    static CARRIED: Cell<Option<(u64, f32, f32, f32)>> = const { Cell::new(None) };
    /// Set while the hook draws an item itself, so that the nested call is left alone.
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

impl Game<'_> {
    /// `AbstractContainerMenu.doClick`: the clicks of the menu engine, before they are run.
    pub(super) fn tia_clicks(&mut self, ops: &[(Option<usize>, i32, Input)]) {
        let frame = self.frames;
        TIA.with(|t| {
            let mut t = t.borrow_mut();
            for &(slot, button, input) in ops {
                t.click(slot, button, input == Input::Pickup, frame);
            }
        });
    }

    /// Every tick: a menu that is closed takes its slots' animations with it.
    pub(super) fn tia_tick(&mut self) {
        if self.screen != Screen::Inventory {
            TIA.with(|t| t.borrow_mut().closed());
            FRAME.with(|f| f.borrow_mut().number = 0);
        }
    }

    /// The inventory screen begins a frame: where its slots are, and the animations that are
    /// owed a step for slots with nothing in them.
    pub(super) fn tia_frame(&self, batch: &HudBatch) {
        let (ox, oy) = self.inventory_origin(batch);
        let ids = self.station_slots().unwrap_or_else(|| self.slots());
        let slots = ids
            .into_iter()
            .map(|i| {
                let (sx, sy) = self.slot_position(i);
                (i, ox + sx, oy + sy)
            })
            .collect();
        FRAME.with(|f| {
            *f.borrow_mut() = Frame {
                number: self.frames,
                slots,
            }
        });
        let partial = self.clock.fraction() as f32;
        TIA.with(|t| t.borrow_mut().begin_frame(self.frames, partial));
    }

    /// The model of an item is about to be drawn at `(x, y)`. If it is a slot's and the slot is
    /// animated, the animation steps and the item is drawn scaled.
    pub(super) fn tia_model(
        &self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        x: f32,
        y: f32,
    ) -> ControlFlow<()> {
        if INSIDE.get() {
            return ControlFlow::Continue(());
        }
        // The carried stack is scaled as a whole by `draw_item`, not as a slot's item.
        if let Some((frame, cx, cy, _)) = CARRIED.get()
            && frame == self.frames
            && cx == x
            && cy == y
        {
            return ControlFlow::Continue(());
        }
        let slot = FRAME.with(|f| {
            let f = f.borrow();
            if f.number != self.frames {
                return None;
            }
            f.slots
                .iter()
                .find(|&&(_, sx, sy)| sx == x && sy == y)
                .map(|&(i, _, _)| i)
        });
        let Some(slot) = slot else {
            return ControlFlow::Continue(());
        };
        let partial = self.clock.fraction() as f32;
        let scale = TIA.with(|t| t.borrow_mut().draw_slot(slot, partial, self.frames));
        if scale == 1.0 {
            return ControlFlow::Continue(());
        }
        INSIDE.set(true);
        let start = batch.vertices.len();
        self.draw_item_model(batch, stack, x, y);
        INSIDE.set(false);
        // `pose.translate(x + 8, y + 8)` then `scale`: about the centre of the 16x16 icon.
        batch.scale_since(start, x + 8.0, y + 8.0, scale, scale);
        ControlFlow::Break(())
    }

    /// The carried stack is about to be drawn (`renderFloatingItem`): its animation takes a step.
    pub(super) fn tia_carried(&self) {
        if self.carried.is_none() || self.screen != Screen::Inventory {
            return;
        }
        let partial = self.clock.fraction() as f32;
        let scale = TIA.with(|t| t.borrow_mut().draw_carried(partial));
        CARRIED.set(Some((
            self.frames,
            self.cursor.0 - 8.0,
            self.cursor.1 - 8.0,
            scale,
        )));
    }
    /// `draw_item` is about to draw a stack: if it is the carried one, draw it scaled, the count
    /// and the damage bar with the icon.
    pub(super) fn tia_item(
        &self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        x: f32,
        y: f32,
    ) -> ControlFlow<()> {
        if INSIDE.get() {
            return ControlFlow::Continue(());
        }
        let Some((frame, cx, cy, scale)) = CARRIED.get() else {
            return ControlFlow::Continue(());
        };
        if frame != self.frames || cx != x || cy != y || scale == 1.0 {
            return ControlFlow::Continue(());
        }
        INSIDE.set(true);
        let start = batch.vertices.len();
        self.draw_item(batch, stack, x, y);
        INSIDE.set(false);
        // The pose is moved to the cursor, a little below the centre of the icon (4 pixels),
        // scaled, and moved back: the icon and its count grow about that point.
        batch.scale_since(start, self.cursor.0, self.cursor.1 + 4.0, scale, scale);
        ControlFlow::Break(())
    }
}
