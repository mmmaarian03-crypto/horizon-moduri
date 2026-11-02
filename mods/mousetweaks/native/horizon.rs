// Mouse Tweaks inside Horizon's game (nightly 142388e1c136). The rules are the engine below; this
// part gives them the open container screen and does the clicks the game's own way
// (`Game::tweak_click`: the menu engine, or a server's predicted clicks).
use super::{Game, ItemStack, Screen};
use horizon_client::{containers::BlockEntity, hud::HudBatch, menu::Input};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    include!("mousetweaks_engine.rs");
}
use engine::{Button, Slot};

pub const ID: &str = "mousetweaks";
pub const TITLE: &str = "Mouse Tweaks 2.25.1";
pub const DESCRIPTION: &str = "Drag with the right button to place one item in each slot, drag with the left to gather a stack or (with Shift) quick-move, and roll the wheel over a slot to push or pull items. The original's defaults and the pack's file.";

/// What the rules keep between events, per thread of the window.
struct State {
    tweaks: engine::Tweaks,
    /// `MouseHandler.activeButton`: the button of the drag, none after any release.
    active: Option<Button>,
    /// Bumped each time a container closes, so the next screen starts with its own state.
    screen: u64,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        tweaks: engine::Tweaks::new(),
        active: None,
        screen: 0,
    });
}

/// The open screen as the rules' menu: Horizon's slot ids, listed in the order vanilla's menu has them.
struct Live<'a, 'g> {
    game: &'a mut Game<'g>,
    order: Vec<usize>,
    furnace: bool,
}

/// A player's slot: the inventory, and the armour and off hand.
fn in_player(id: usize) -> bool {
    id < 36 || (500..=504).contains(&id)
}

impl engine::Menu for Live<'_, '_> {
    type Stack = ItemStack;
    fn slots(&self) -> Vec<Slot> {
        self.order.clone()
    }
    fn stack(&mut self, slot: Slot) -> Option<ItemStack> {
        self.game.slot_view(slot)
    }
    fn carried(&mut self) -> Option<ItemStack> {
        self.game.carried.clone()
    }
    fn count(&self, stack: &ItemStack) -> u32 {
        stack.count
    }
    fn same(&self, a: &ItemStack, b: &ItemStack) -> bool {
        a.same_item_same_components(b)
    }
    fn max_stack(&self, stack: &ItemStack) -> u32 {
        self.game.max_stack(stack)
    }
    fn slot_max(&self, slot: Slot, stack: &ItemStack) -> u32 {
        // FurnaceFuelSlot.getMaxStackSize(stack): a bucket stacks to one there.
        if self.furnace && slot == 301 && &*stack.item == "minecraft:bucket" {
            return 1;
        }
        engine::Menu::slot_limit(self, slot).min(self.game.max_stack(stack))
    }
    fn slot_limit(&self, slot: Slot) -> u32 {
        // Container.getMaxStackSize() is 99; the armour slots hold one.
        if (500..=503).contains(&slot) { 1 } else { 99 }
    }
    fn may_place(&self, slot: Slot, stack: &ItemStack) -> bool {
        let g = &*self.game;
        match slot {
            // ResultSlot.mayPlace and FurnaceResultSlot.mayPlace.
            200 => false,
            100..109 | 0..36 | 504 => true,
            // ArmorSlot.mayPlace: the item is equippable in that slot and a player may wear it.
            500..=503 => g.res.items().get(&stack.item).is_some_and(|i| {
                let wanted = ["head", "chest", "legs", "feet"][slot - 500];
                i.equip_slot.as_deref() == Some(wanted)
                    && i.equip_allowed
                        .as_ref()
                        .is_none_or(|a| a.contains("minecraft:player"))
            }),
            300..400 if self.furnace => match slot {
                300 => true,
                // FurnaceFuelSlot.mayPlace: fuel, or a bucket.
                301 => {
                    &*stack.item == "minecraft:bucket"
                        || g.res.items().get(&stack.item).is_some_and(|i| i.fuel.is_some())
                }
                _ => false,
            },
            // ShulkerBoxSlot.mayPlace.
            300..400 => !g.refuses(slot, stack),
            _ => false,
        }
    }
    fn in_player_inventory(&self, slot: Slot) -> bool {
        in_player(slot)
    }
    fn is_crafting_output(&self, slot: Slot) -> bool {
        slot == 200 || (self.furnace && slot == 302)
    }
    fn is_ignored(&self, _slot: Slot) -> bool {
        false
    }
    fn slot_y(&self, slot: Slot) -> i32 {
        self.game.slot_position(slot).1 as i32
    }
    fn shift_down(&self) -> bool {
        self.game.has_shift_down()
    }
    fn click(&mut self, slot: Slot, button: Button, shift: bool) {
        // The crafting result takes its plain click through `click_inventory`, which reads the cursor
        // (the menu engine only does the quick move there): so the click is made as if the cursor
        // were over the slot.
        if slot == 200 && !shift {
            self.game.mousetweaks_click_at(slot, button == Button::Right);
            return;
        }
        let button = i32::from(button == Button::Right);
        let input = if shift { Input::QuickMove } else { Input::Pickup };
        self.game.tweak_click(slot, button, input);
    }
    fn disable_rmb_dragging(&mut self) {
        // `skipNextRelease = true` and the quick craft drag of the right button off.
        self.game.inv_drag = None;
        self.game.inv_clicks.skip_release();
    }
    fn is_bundle(&self, stack: &ItemStack) -> bool {
        self.game
            .res
            .items()
            .get(&stack.item)
            .is_some_and(|i| i.bundle)
    }
}

impl Game<'_> {
    /// The open screen in vanilla's slot order, when Mouse Tweaks works in it: the player's
    /// inventory, a crafting table, a chest-like container (chest, barrel, ender chest, shulker box,
    /// hopper, dispenser, dropper) or a furnace. Not creative, not a station (anvil, loom...), not a
    /// mount's inventory, not a brewing stand or crafter.
    fn mousetweaks_order(&self) -> Option<(Vec<usize>, bool)> {
        if self.screen != Screen::Inventory || self.station.is_some() || self.mount_active() {
            return None;
        }
        if self.remote.is_some() && !self.remote_storage_open() {
            return None;
        }
        let furnace = match self.container_kind() {
            None | Some(BlockEntity::Chest(_)) => false,
            Some(BlockEntity::Furnace(_)) => true,
            Some(_) => return None,
        };
        // The menu's order: result, crafting grid, armour, the container, the inventory, hotbar, off hand.
        let rank = |id: usize| match id {
            200 => Some((0, id)),
            100..109 => Some((1, id)),
            500..=503 => Some((2, id)),
            300..500 => Some((3, id)),
            9..36 => Some((4, id)),
            0..9 => Some((5, id)),
            504 => Some((6, id)),
            _ => None,
        };
        let mut slots: Vec<(usize, (usize, usize))> = Vec::new();
        for id in self.slots() {
            slots.push((id, rank(id)?));
        }
        slots.sort_by_key(|s| s.1);
        Some((slots.into_iter().map(|s| s.0).collect(), furnace))
    }

    /// Runs `f` on the rules and the open screen; `default` when it is not one Mouse Tweaks works in.
    fn mousetweaks_with<R>(
        &mut self,
        default: R,
        f: impl FnOnce(&mut engine::Tweaks, &mut Live<'_, '_>, &mut Option<Button>) -> R,
    ) -> R {
        let Some((order, furnace)) = self.mousetweaks_order() else {
            return default;
        };
        // updateScreen: a screen that is new starts over and reads the config again.
        let screen = STATE.with(|s| s.borrow().screen);
        let fresh = STATE.with(|s| s.borrow().tweaks.is_screen(screen));
        if !fresh {
            let settings = engine::Settings::read(|key| self.native_setting(ID, key));
            STATE.with(|s| s.borrow_mut().tweaks.update_screen(screen, || settings));
        }
        let mut live = Live {
            game: self,
            order,
            furnace,
        };
        STATE.with(|cell| {
            let mut guard = cell.borrow_mut();
            let state = &mut *guard;
            f(&mut state.tweaks, &mut live, &mut state.active)
        })
    }

    /// A plain click on `slot` made the way the press handler's fallback makes it: `click_inventory` acts on
    /// the slot under the cursor, so the cursor is put over the slot for the click and back.
    fn mousetweaks_click_at(&mut self, slot: usize, right: bool) {
        let Some(size) = self
            .renderer
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
        else {
            return;
        };
        let batch = HudBatch::new(size);
        let (ox, oy) = self.inventory_origin(&batch);
        let (sx, sy) = self.slot_position(slot);
        let saved = self.cursor;
        self.cursor = (ox + sx + 8.0, oy + sy + 8.0);
        self.click_inventory(right);
        self.cursor = saved;
        if let Some(key) = self.container {
            self.container_changed(key);
        }
    }

    /// A mouse button went down: over a container screen the rules note the slot and what is on the mouse.
    pub(super) fn mousetweaks_pressed(&mut self, button: winit::event::MouseButton) {
        if !self.native_on(ID) || self.screen != Screen::Inventory {
            return;
        }
        let right = match button {
            winit::event::MouseButton::Left => false,
            winit::event::MouseButton::Right => true,
            _ => return,
        };
        let slot = self.inventory_target().0;
        self.mousetweaks_press(slot, right);
    }

    /// A mouse press on the slot under the cursor (`onMouseClicked`; it never takes the click).
    fn mousetweaks_press(&mut self, slot: Option<usize>, right: bool) {
        let button = if right { Button::Right } else { Button::Left };
        self.mousetweaks_with((), |tweaks, live, active| {
            *active = Some(button);
            tweaks.press(live, slot, button);
        });
    }

    /// The cursor moved over an inventory screen: while a button is down, `onMouseDrag`.
    pub(super) fn mousetweaks_moved(&mut self) {
        if !self.native_on(ID) || self.screen != Screen::Inventory {
            return;
        }
        if STATE.with(|s| s.borrow().active.is_none()) {
            return;
        }
        let slot = self.inventory_target().0;
        self.mousetweaks_with((), |tweaks, live, active| {
            if let Some(button) = *active {
                tweaks.drag(live, slot, button);
            }
        });
    }

    /// A button went up (`onMouseReleased`).
    pub(super) fn mousetweaks_release(&mut self, right: bool) {
        if !self.native_on(ID) {
            return;
        }
        let button = if right { Button::Right } else { Button::Left };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.tweaks.release(button);
            // MouseHandler: any release ends the drag.
            s.active = None;
        });
    }

    /// The wheel over an inventory screen (`onMouseScrolled`): true when Mouse Tweaks took it.
    pub(super) fn mousetweaks_scroll(&mut self, delta: f32, slot: Option<usize>) -> bool {
        self.mousetweaks_with(false, |tweaks, live, _| {
            tweaks.scroll(live, slot, f64::from(delta))
        })
    }

    /// The container closed: the next screen is a new one.
    pub(super) fn mousetweaks_closed(&mut self) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.screen = s.screen.wrapping_add(1);
            s.active = None;
        });
    }
}
