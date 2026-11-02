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
    include!("tia_engine.rs");
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
