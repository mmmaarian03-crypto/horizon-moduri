// Smooth Swapping inside Horizon's game (nightly 142388e1c136). The rules are the engine below
// (src/lib.rs); this part reads the open menu once a frame, hears the clicks, and draws the items
// that are on their way.
//
// Where the original replaces `DrawContext.drawItem` and `drawItemInSlot` for a slot, this does the
// same through `draw_item_model` (the hook draws what the plan says and keeps the game from drawing
// the slot's item). The count and bar the game draws after an item cannot be hooked (the function
// has a macro), so that overlay is taken out of the batch afterwards, when it can be shown to be
// exactly what the game's own call produced, and the one the original draws is put in its place.
// A slot is recognised by where its item is drawn: the screen lists its slots and their places at
// the start of the frame (`draw_inventory`).
use super::{Game, Screen, stations};
use horizon_client::containers::BlockEntity;
use horizon_client::hud::{HudBatch, HudVertex};
use horizon_client::items::ItemStack;
use horizon_client::menu::Input;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ops::{ControlFlow, Range};

#[allow(dead_code)]
mod engine {
    include!("smoothswapping_engine.rs");
}

/// A stack as the engine reads it: the game's stack, and whether its bar shows.
#[derive(Clone)]
struct Item {
    stack: Option<ItemStack>,
    bar: bool,
}
impl engine::Stack for Item {
    fn is_empty(&self) -> bool {
        self.stack.is_none()
    }
    fn count(&self) -> i32 {
        self.stack.as_ref().map_or(0, |s| s.count as i32)
    }
    fn same_item(&self, other: &Self) -> bool {
        match (&self.stack, &other.stack) {
            (None, None) => true,
            (Some(a), Some(b)) => a.item == b.item,
            _ => false,
        }
    }
    fn same(&self, other: &Self) -> bool {
        self.stack == other.stack
    }
    fn bar_visible(&self) -> bool {
        self.bar
    }
}

/// One slot of the screen being drawn: its index in the menu's order, its id in the game, and where
/// its item is drawn.
struct Placed {
    index: usize,
    id: usize,
    x: f32,
    y: f32,
}
/// Something to draw over the screen once its slots are done.
enum Later {
    Model(ItemStack, f32, f32),
    Overlay(ItemStack, f32, f32, Option<String>),
}
/// A slot's own count and bar, to put in place of what the game draws after its item.
struct Cleanup {
    frame: u64,
    /// Where the game's overlay begins in the batch.
    start: usize,
    stack: ItemStack,
    x: f32,
    y: f32,
    /// The count to show in its place, if any is shown.
    count: Option<i32>,
}
struct State {
    smooth: engine::Smooth<Item>,
    token: u64,
    last_frame: u64,
    ids: Vec<usize>,
    frame: u64,
    slots: Vec<Placed>,
    /// Where the carried stack is drawn this frame.
    carried_at: Option<(f32, f32)>,
    later: Vec<Later>,
    cleanup: Option<Cleanup>,
}
thread_local! {
    static SS: RefCell<State> = RefCell::new(State {
        smooth: engine::Smooth::new(engine::Config::default()),
        token: 0,
        last_frame: 0,
        ids: Vec::new(),
        frame: 0,
        slots: Vec::new(),
        carried_at: None,
        later: Vec::new(),
        cleanup: None,
    });
    /// Set while the hook draws an item itself, so that the nested call is left alone.
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

/// Replace `range` of the batch's vertices by `with`, keeping the blend runs that come after in
/// place.
fn splice(batch: &mut HudBatch, range: Range<usize>, with: Vec<HudVertex>) {
    let delta = with.len() as i64 - (range.end - range.start) as i64;
    drop(batch.vertices.splice(range.clone(), with));
    let shift = |v: u32| (i64::from(v) + delta) as u32;
    for (run, _) in &mut batch.blends {
        if run.start as usize >= range.end {
            run.start = shift(run.start);
            run.end = shift(run.end);
        } else if run.end as usize > range.start {
            run.end = shift(run.end);
        }
    }
    if batch.blur_at as usize >= range.end {
        batch.blur_at = shift(batch.blur_at);
    }
}
fn same_vertex(a: &HudVertex, b: &HudVertex) -> bool {
    a.pos == b.pos
        && a.uv == b.uv
        && a.color == b.color
        && a.depth == b.depth
        && a.glint_uv == b.glint_uv
}

impl Game<'_> {
    /// A stack as the engine reads it.
    fn smoothswapping_item(&self, stack: Option<ItemStack>) -> Item {
        let bar = stack.as_ref().is_some_and(|s| {
            let items = self.res.items();
            let max_damage = items.get(&s.item).map_or(0, |info| info.max_damage as i32);
            horizon_client::bundle::Contents::of(items, s)
                .map(|c| c.bar())
                .unwrap_or_else(|| {
                    horizon_client::hud_overlays::damage_bar(s.damage as i32, max_damage)
                })
                .is_some()
        });
        Item { stack, bar }
    }

    /// The slots of the open screen in the order of the vanilla menu (`AbstractContainerMenu.slots`),
    /// which the original numbers its slots by.
    fn smoothswapping_menu_order(&self) -> Vec<usize> {
        let stationed = self.station_slots();
        let valid: HashSet<usize> = stationed
            .clone()
            .unwrap_or_else(|| self.slots())
            .into_iter()
            .collect();
        let mut order: Vec<usize> = Vec::new();
        if stationed.is_none() && self.container.is_none() {
            // InventoryMenu / CraftingMenu: result, grid, armour, inventory, hotbar, off hand.
            let n = self.craft_size * self.craft_size;
            order.push(200);
            order.extend(100..100 + n);
            if self.craft_size == 2 {
                order.extend(500..504);
            }
            order.extend(9..36);
            order.extend(0..9);
            if self.craft_size == 2 {
                order.push(504);
            }
        } else {
            // The block's or station's own slots first (inputs, then the result), then the player's.
            let mut own: Vec<usize> = valid
                .iter()
                .copied()
                .filter(|&i| i >= 36 && i != stations::RESULT)
                .collect();
            own.sort_unstable();
            order.extend(own);
            if valid.contains(&stations::RESULT) {
                order.push(stations::RESULT);
            }
            order.extend(9..36);
            order.extend(0..9);
        }
        order.retain(|i| valid.contains(i));
        order
    }
    /// Slots nothing can be put into (`!canTakePartial`): results.
    fn smoothswapping_output(&self, id: usize) -> bool {
        id == 200
            || id == stations::RESULT
            || (id == 302 && matches!(self.container_kind(), Some(BlockEntity::Furnace(_))))
            || (id == 309 && matches!(self.container_kind(), Some(BlockEntity::Crafter(_))))
    }

    /// `ClickSlotC2SPacketMixin`: the clicks of the menu engine, before they are run.
    pub(super) fn smoothswapping_clicks(&mut self, ops: &[(Option<usize>, i32, Input)]) {
        SS.with(|s| {
            let mut s = s.borrow_mut();
            let State { smooth, slots, .. } = &mut *s;
            for &(slot, _, input) in ops {
                let index = slot.and_then(|g| slots.iter().find(|p| p.id == g).map(|p| p.index));
                let kind = match input {
                    Input::QuickMove => engine::ClickKind::QuickMove,
                    Input::Swap => engine::ClickKind::Swap,
                    _ => engine::ClickKind::Other,
                };
                smooth.clicked(index, kind);
            }
        });
    }

    /// Every tick: a closed screen leaves no frame behind.
    pub(super) fn smoothswapping_tick(&mut self) {
        if self.screen != Screen::Inventory {
            SS.with(|s| {
                let mut s = s.borrow_mut();
                s.frame = 0;
                s.later.clear();
                s.cleanup = None;
            });
        }
    }

    /// `HandledScreenMixin.render` at its head: the inventory screen begins a frame.
    pub(super) fn smoothswapping_frame(&mut self, batch: &HudBatch) {
        let (ox, oy) = self.inventory_origin(batch);
        let ids = self.smoothswapping_menu_order();
        let mut views = Vec::with_capacity(ids.len());
        let mut placed = Vec::with_capacity(ids.len());
        for (index, &id) in ids.iter().enumerate() {
            let (sx, sy) = self.slot_position(id);
            let stack = self.slot_view(id);
            views.push(engine::SlotView {
                x: f64::from(sx),
                y: f64::from(sy),
                stack: self.smoothswapping_item(stack),
                output: self.smoothswapping_output(id),
            });
            placed.push(Placed {
                index,
                id,
                x: ox + sx,
                y: oy + sy,
            });
        }
        let cursor = self.smoothswapping_item(self.carried.clone());
        let mouse = (
            f64::from(self.cursor.0 - ox),
            f64::from(self.cursor.1 - oy),
        );
        let frame = self.frames;
        SS.with(|s| {
            let mut s = s.borrow_mut();
            // A screen opened anew, or another menu in it, is another screen.
            if s.last_frame + 1 != frame || s.ids != ids {
                s.token += 1;
            }
            s.last_frame = frame;
            s.ids = ids;
            s.frame = frame;
            s.slots = placed;
            s.carried_at = None;
            s.later.clear();
            s.cleanup = None;
            let token = s.token;
            s.smooth.frame(&views, cursor, mouse, token, false);
        });
    }

    /// The game's overlay of the slot drawn last is replaced by the original's, if it is there.
    fn smoothswapping_cleanup(&self, batch: &mut HudBatch) {
        let Some(c) = SS.with(|s| s.borrow_mut().cleanup.take()) else {
            return;
        };
        if c.frame != self.frames {
            return;
        }
        let size = [
            (batch.width * batch.scale).round() as u32,
            (batch.height * batch.scale).round() as u32,
        ];
        // What the game's call drew, again, to know what to take out and to be sure of it.
        let mut game_overlay = HudBatch::new(size);
        self.draw_item_decorations(&mut game_overlay, &c.stack, c.x, c.y, None);
        let n = game_overlay.vertices.len();
        let end = c.start + n;
        if end > batch.vertices.len()
            || !batch.vertices[c.start..end]
                .iter()
                .zip(&game_overlay.vertices)
                .all(|(a, b)| same_vertex(a, b))
        {
            return;
        }
        let mut ours = HudBatch::new(size);
        if let Some(count) = c.count {
            self.draw_item_decorations(&mut ours, &c.stack, c.x, c.y, Some(count.to_string()));
        }
        batch.patches.append(&mut ours.patches);
        splice(batch, c.start..end, std::mem::take(&mut ours.vertices));
    }

    /// The things to draw over the screen: the items on their way, and their counts.
    fn smoothswapping_draw_later(&self, batch: &mut HudBatch) {
        let later = SS.with(|s| std::mem::take(&mut s.borrow_mut().later));
        for item in later {
            INSIDE.set(true);
            match item {
                Later::Model(stack, x, y) => self.draw_item_model(batch, &stack, x, y),
                Later::Overlay(stack, x, y, text) => {
                    self.draw_item_decorations(batch, &stack, x, y, text)
                }
            }
            INSIDE.set(false);
        }
    }

    /// An item's model is about to be drawn (`DrawContext.drawItem`). A slot with items on their
    /// way is drawn as the plan says: the movers are drawn later, over everything, and the slot's
    /// own item only when it is to be seen.
    pub(super) fn smoothswapping_model(
        &self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        x: f32,
        y: f32,
    ) -> ControlFlow<()> {
        if INSIDE.get() {
            return ControlFlow::Continue(());
        }
        self.smoothswapping_cleanup(batch);
        let found = SS.with(|s| {
            let s = s.borrow();
            if s.frame != self.frames {
                return None;
            }
            if s.carried_at == Some((x, y)) {
                return None;
            }
            s.slots.iter().find(|p| p.x == x && p.y == y).map(|p| p.index)
        });
        let Some(index) = found else {
            return ControlFlow::Continue(());
        };
        let item = self.smoothswapping_item(Some(stack.clone()));
        let ticks = self.frame_ticks;
        let planned = SS.with(|s| {
            let mut s = s.borrow_mut();
            let plan = s.smooth.slot_model(index, ticks)?;
            let overlay = s.smooth.slot_overlay(index, &item);
            for f in &plan.flying {
                // A mover with no distance to go has no place (NaN): nothing to draw.
                if let Some(stack) = f.item.stack.clone()
                    && f.dx.is_finite()
                    && f.dy.is_finite()
                {
                    s.later
                        .push(Later::Model(stack, x + f.dx as f32, y + f.dy as f32));
                }
            }
            if let Some(overlay) = &overlay {
                for f in &overlay.flying {
                    if let Some(stack) = f.stack.stack.clone()
                        && f.dx.is_finite()
                        && f.dy.is_finite()
                    {
                        s.later.push(Later::Overlay(
                            stack,
                            x + f.dx as f32,
                            y + f.dy as f32,
                            f.text.map(|n| n.to_string()),
                        ));
                    }
                }
            }
            Some((plan.draw_destination, overlay.map(|o| o.destination)))
        });
        let Some((draw_destination, overlay)) = planned else {
            return ControlFlow::Continue(());
        };
        if draw_destination {
            INSIDE.set(true);
            self.draw_item_model(batch, stack, x, y);
            INSIDE.set(false);
        }
        if let Some(count) = overlay {
            // The game draws the slot's count and bar next; they are the original's to decide.
            let start = batch.vertices.len();
            let frame = self.frames;
            SS.with(|s| {
                s.borrow_mut().cleanup = Some(Cleanup {
                    frame,
                    start,
                    stack: stack.clone(),
                    x,
                    y,
                    count,
                })
            });
        }
        ControlFlow::Break(())
    }

    /// The carried stack is about to be drawn (`renderFloatingItem`): the items on their way to the
    /// cursor are drawn with it, and everything that was waiting for the slots to be done.
    pub(super) fn smoothswapping_carried(&self, batch: &mut HudBatch) {
        self.smoothswapping_cleanup(batch);
        if self.carried.is_some() {
            let (cx, cy) = (self.cursor.0 - 8.0, self.cursor.1 - 8.0);
            let ticks = self.frame_ticks;
            let flying = SS.with(|s| {
                let mut s = s.borrow_mut();
                s.carried_at = Some((cx, cy));
                s.smooth.cursor_model(ticks)
            });
            for f in flying {
                if let Some(stack) = f.item.stack
                    && f.dx.is_finite()
                    && f.dy.is_finite()
                {
                    INSIDE.set(true);
                    self.draw_item_model(batch, &stack, cx + f.dx as f32, cy + f.dy as f32);
                    INSIDE.set(false);
                }
            }
        }
        self.smoothswapping_draw_later(batch);
    }

    /// The end of the screen's drawing: what is still waiting is drawn (screens that draw no
    /// carried stack, such as the stations when nothing is carried).
    pub(super) fn smoothswapping_end(&self, batch: &mut HudBatch) {
        self.smoothswapping_cleanup(batch);
        self.smoothswapping_draw_later(batch);
    }
}
