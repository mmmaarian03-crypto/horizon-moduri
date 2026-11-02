// Mouse Tweaks inside Horizon's game (nightly 142388e1c136). The rules are the engine below; this
// part gives them the open container screen and does the clicks the game's own way
// (`Game::tweak_click`: the menu engine, or a server's predicted clicks).
use super::{Game, ItemStack, Screen};
use horizon_client::{containers::BlockEntity, hud::HudBatch, menu::Input};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    // Mouse Tweaks for Horizon: the rules, without any game code.
// Written from Mouse Tweaks 2.25.1 for Minecraft 1.20.1 (YaLTeR, BSD-3-Clause; `Main.java`, `Config.java`,
// `GuiContainerHandler.java` and the enums of the 1.20.1 branch, read for this port; no code of it is used).
// The original listens to Forge's `ScreenEvent` MouseButtonPressed.Pre, MouseDragged.Pre,
// MouseButtonReleased.Pre and MouseScrolled.Post; here the game calls `Tweaks::press`, `drag`,
// `release` and `scroll` at the same moments and `Menu` is the open container menu.

/// The slot handle the game gives (its own slot id).
pub type Slot = usize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Left,
    Right,
}

/// `WheelSearchOrder`: the order `findPullSlot` goes through the menu's slots.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SearchOrder {
    FirstToLast,
    LastToFirst,
}
/// `WheelScrollDirection`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollDirection {
    Normal,
    Inverted,
    PositionAware,
    PositionAwareInverted,
}
impl ScrollDirection {
    fn inverted(self) -> bool {
        matches!(self, Self::Inverted | Self::PositionAwareInverted)
    }
    fn position_aware(self) -> bool {
        matches!(self, Self::PositionAware | Self::PositionAwareInverted)
    }
}
/// `ScrollItemScaling`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scaling {
    Proportional,
    AlwaysOne,
}

/// `MouseTweaks.cfg`. The defaults are the original's and also the values in the pack's file
/// (`RMBTweak=1 LMBTweakWithItem=1 LMBTweakWithoutItem=1 WheelTweak=1 WheelSearchOrder=1
/// WheelScrollDirection=0 ScrollItemScaling=0 Debug=0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub rmb_tweak: bool,
    pub lmb_tweak_with_item: bool,
    pub lmb_tweak_without_item: bool,
    pub wheel_tweak: bool,
    pub wheel_search_order: SearchOrder,
    pub wheel_scroll_direction: ScrollDirection,
    pub scroll_item_scaling: Scaling,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rmb_tweak: true,
            lmb_tweak_with_item: true,
            lmb_tweak_without_item: true,
            wheel_tweak: true,
            wheel_search_order: SearchOrder::LastToFirst,
            wheel_scroll_direction: ScrollDirection::Normal,
            scroll_item_scaling: Scaling::Proportional,
        }
    }
}

impl Settings {
    /// `Config.read`: every value is an integer under the file's key, a bad or missing one is the default.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let int = |key: &str, default: i32| {
            get(key)
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(default)
        };
        Self {
            rmb_tweak: int("RMBTweak", 1) != 0,
            lmb_tweak_with_item: int("LMBTweakWithItem", 1) != 0,
            lmb_tweak_without_item: int("LMBTweakWithoutItem", 1) != 0,
            wheel_tweak: int("WheelTweak", 1) != 0,
            // fromId: 0 is FIRST_TO_LAST, anything else LAST_TO_FIRST.
            wheel_search_order: if int("WheelSearchOrder", 1) == 0 {
                SearchOrder::FirstToLast
            } else {
                SearchOrder::LastToFirst
            },
            // fromId: 0, 1, 2, else INVENTORY_POSITION_AWARE_INVERTED.
            wheel_scroll_direction: match int("WheelScrollDirection", 0) {
                0 => ScrollDirection::Normal,
                1 => ScrollDirection::Inverted,
                2 => ScrollDirection::PositionAware,
                _ => ScrollDirection::PositionAwareInverted,
            },
            // fromId: 0 is PROPORTIONAL, anything else ALWAYS_ONE.
            scroll_item_scaling: if int("ScrollItemScaling", 0) == 0 {
                Scaling::Proportional
            } else {
                Scaling::AlwaysOne
            },
        }
    }
    /// `ScrollItemScaling.scale`.
    fn scale(&self, delta: f64) -> f64 {
        match self.scroll_item_scaling {
            Scaling::Proportional => delta,
            Scaling::AlwaysOne => signum(delta),
        }
    }
}

/// `Math.signum`.
fn signum(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// The open container menu, as Mouse Tweaks' `IGuiScreenHandler` and `Slot` show it. Slots are
/// handles; `slots` lists them in `AbstractContainerMenu.slots` order, which decides where the wheel
/// looks first.
pub trait Menu {
    type Stack: Clone;
    /// `menu.slots`.
    fn slots(&self) -> Vec<Slot>;
    /// `slot.getItem()`, None when empty.
    fn stack(&mut self, slot: Slot) -> Option<Self::Stack>;
    /// `menu.getCarried()`, None when empty.
    fn carried(&mut self) -> Option<Self::Stack>;
    fn count(&self, stack: &Self::Stack) -> u32;
    /// `isSameItem && isSameItemSameTags`.
    fn same(&self, a: &Self::Stack, b: &Self::Stack) -> bool;
    /// `ItemStack.getMaxStackSize`.
    fn max_stack(&self, stack: &Self::Stack) -> u32;
    /// `Slot.getMaxStackSize(stack)`.
    fn slot_max(&self, slot: Slot, stack: &Self::Stack) -> u32;
    /// `Slot.getMaxStackSize()`.
    fn slot_limit(&self, slot: Slot) -> u32;
    /// `Slot.mayPlace`.
    fn may_place(&self, slot: Slot, stack: &Self::Stack) -> bool;
    /// `slot.container == player.getInventory()`.
    fn in_player_inventory(&self, slot: Slot) -> bool;
    /// `isCraftingOutput`: a `ResultSlot`, `FurnaceResultSlot` or `MerchantResultSlot`.
    fn is_crafting_output(&self, slot: Slot) -> bool;
    /// `isIgnored`.
    fn is_ignored(&self, slot: Slot) -> bool;
    /// `slot.y`.
    fn slot_y(&self, slot: Slot) -> i32;
    /// `Screen.hasShiftDown` (the physical Shift keys).
    fn shift_down(&self) -> bool;
    /// `slotClicked(slot, slot.index, button, shift ? QUICK_MOVE : PICKUP)`.
    fn click(&mut self, slot: Slot, button: Button, shift: bool);
    /// `disableRMBDraggingFunctionality`: the vanilla drag does not run on, and its release clicks nothing.
    fn disable_rmb_dragging(&mut self);
    /// `stack.getItem() instanceof BundleItem`.
    fn is_bundle(&self, stack: &Self::Stack) -> bool;
}

/// `slot.getMaxStackSize(slot.getItem())` of an empty slot: `ItemStack.EMPTY` is air, whose stack size is 64.
fn empty_max<M: Menu>(menu: &M, slot: Slot) -> i32 {
    menu.slot_limit(slot).min(64) as i32
}

/// `Main`'s static state, kept between events of one open screen.
#[derive(Clone, Debug)]
pub struct Tweaks {
    pub settings: Settings,
    /// `openScreen`: an id the game gives each screen it opens.
    screen: Option<u64>,
    old_selected: Option<Slot>,
    accumulated: f64,
    can_lmb_drag: bool,
    can_rmb_drag: bool,
    rmb_left_original: bool,
}

impl Default for Tweaks {
    fn default() -> Self {
        Self::new()
    }
}

impl Tweaks {
    pub fn new() -> Self {
        Self {
            settings: Settings::default(),
            screen: None,
            old_selected: None,
            accumulated: 0.0,
            can_lmb_drag: false,
            can_rmb_drag: false,
            rmb_left_original: false,
        }
    }

    /// `updateScreen`: a screen that is not the open one resets the state and re-reads the config.
    pub fn update_screen(&mut self, screen: u64, read: impl FnOnce() -> Settings) {
        if self.screen == Some(screen) {
            return;
        }
        self.screen = Some(screen);
        self.old_selected = None;
        self.accumulated = 0.0;
        self.can_lmb_drag = false;
        self.can_rmb_drag = false;
        self.rmb_left_original = false;
        self.settings = read();
    }

    /// Whether screen is the one the state belongs to.
    pub fn is_screen(&self, screen: u64) -> bool {
        self.screen == Some(screen)
    }

    /// `onMouseClicked` (it never cancels the click). `under` is the slot under the mouse.
    pub fn press<M: Menu>(&mut self, menu: &mut M, under: Option<Slot>, button: Button) {
        // Store the currently selected slot.
        self.old_selected = under;
        let on_mouse = menu.carried();
        match button {
            Button::Left => {
                // With a stack on the mouse vanilla's own LMB drag starts: left alone.
                if on_mouse.is_none() {
                    self.can_lmb_drag = true;
                }
            }
            Button::Right => {
                // Only with something on the mouse, and with the tweak on.
                if on_mouse.is_none() || !self.settings.rmb_tweak {
                    return;
                }
                self.can_rmb_drag = true;
                self.rmb_left_original = false;
            }
        }
    }

    /// `onMouseReleased`.
    pub fn release(&mut self, button: Button) {
        match button {
            Button::Left => self.can_lmb_drag = false,
            Button::Right => self.can_rmb_drag = false,
        }
    }

    /// `rmbTweakMaybeClickSlot`; `stack_on_mouse` is read when it is needed (the original holds the
    /// carried stack object, whose count its own clicks change).
    fn rmb_maybe_click<M: Menu>(&mut self, menu: &mut M, slot: Option<Slot>) {
        let Some(slot) = slot else {
            return;
        };
        let Some(on_mouse) = menu.carried() else {
            return;
        };
        if menu.is_ignored(slot) || menu.is_crafting_output(slot) {
            return;
        }
        // A bundle on the mouse skips the two checks below (RMB-dragging items into the bundle).
        if !menu.is_bundle(&on_mouse) {
            if let Some(in_slot) = menu.stack(slot) {
                if !menu.same(&in_slot, &on_mouse) {
                    return;
                }
                if menu.count(&in_slot) == menu.slot_max(slot, &in_slot) {
                    return;
                }
            }
        }
        menu.click(slot, Button::Right, false);
    }

    /// `onMouseDrag`: the mouse moved with `button` down. `under` is the slot under the mouse now.
    pub fn drag<M: Menu>(&mut self, menu: &mut M, under: Option<Slot>, button: Button) {
        let selected = under;
        // If the mouse hasn't moved to a new slot, we don't need to do anything.
        if selected == self.old_selected {
            return;
        }
        let on_mouse = menu.carried();
        // When leaving the original slot for the first time, click it to put an item there.
        if self.can_rmb_drag && button == Button::Right && !self.rmb_left_original {
            self.rmb_left_original = true;
            menu.disable_rmb_dragging();
            let old = self.old_selected;
            self.rmb_maybe_click(menu, old);
        }
        self.old_selected = selected;
        let Some(selected) = selected else {
            return;
        };
        if menu.is_ignored(selected) {
            return;
        }
        // The mouse has just entered a new, non-ignored slot.
        match button {
            Button::Left => {
                if !self.can_lmb_drag {
                    return;
                }
                // LMB tweaks don't do anything for empty slots.
                let Some(in_slot) = menu.stack(selected) else {
                    return;
                };
                let shift = menu.shift_down();
                match on_mouse {
                    None => {
                        // Shift-LMB drag without item.
                        if !self.settings.lmb_tweak_without_item || !shift {
                            return;
                        }
                        menu.click(selected, Button::Left, true);
                    }
                    Some(on_mouse) => {
                        // (Shift-)LMB drag with item.
                        if !self.settings.lmb_tweak_with_item {
                            return;
                        }
                        // Only slots with the same item.
                        if !menu.same(&in_slot, &on_mouse) {
                            return;
                        }
                        if shift {
                            // Shift-click: the items go to the other inventory.
                            menu.click(selected, Button::Left, true);
                        } else {
                            // Not more than still fits on the mouse.
                            if menu.count(&on_mouse) + menu.count(&in_slot) > menu.max_stack(&on_mouse) {
                                return;
                            }
                            // Click so that the stack merges with the slot's, then click again to take it all
                            // back; a crafting output adds to the mouse stack on the first click already.
                            menu.click(selected, Button::Left, false);
                            if !menu.is_crafting_output(selected) {
                                menu.click(selected, Button::Left, false);
                            }
                        }
                    }
                }
            }
            Button::Right => {
                if !self.can_rmb_drag {
                    return;
                }
                self.rmb_maybe_click(menu, Some(selected));
            }
        }
    }

    /// `onMouseScrolled`: true when Mouse Tweaks took the wheel (above a usable slot, always).
    pub fn scroll<M: Menu>(&mut self, menu: &mut M, under: Option<Slot>, scroll_delta: f64) -> bool {
        if !self.settings.wheel_tweak {
            return false;
        }
        // Check if the scroll is above a non-ignored slot.
        let Some(selected) = under else {
            return false;
        };
        if menu.is_ignored(selected) {
            return false;
        }
        // Above a valid slot the scroll is always handled, even if no items move, so that the wheel does
        // not suddenly do something else once the items run out.

        // Scale and accumulate the delta the way `MouseHandler.onScroll` does.
        let scaled = self.settings.scale(scroll_delta);
        if self.accumulated != 0.0 && signum(scaled) != signum(self.accumulated) {
            self.accumulated = 0.0;
        }
        self.accumulated += scaled;
        // (int): the number of items to move, the sign is the direction.
        let delta = self.accumulated as i32;
        self.accumulated -= f64::from(delta);
        if delta == 0 {
            return true;
        }

        let slots = menu.slots();
        let mut to_move = delta.unsigned_abs() as i32;
        let mut push = delta < 0;
        if self.settings.wheel_scroll_direction.position_aware() && other_inventory_is_above(menu, selected, &slots) {
            push = !push;
        }
        if self.settings.wheel_scroll_direction.inverted() {
            push = !push;
        }

        // Nothing to do with an empty slot.
        let Some(selected_stack) = menu.stack(selected) else {
            return true;
        };
        let on_mouse = menu.carried();

        // A crafting output behaves differently.
        if menu.is_crafting_output(selected) {
            return self.scroll_crafting_output(menu, selected, &selected_stack, on_mouse, push, to_move, &slots);
        }

        // It's not possible to interact with the slot cleanly when holding a compatible stack.
        if let Some(held) = &on_mouse {
            if menu.same(&selected_stack, held) {
                return true;
            }
        }

        if push {
            // A stack on the mouse has to be placeable in the selected slot.
            if let Some(held) = &on_mouse {
                if !menu.may_place(selected, held) {
                    return true;
                }
            }
            // Clip the number of items to move by what the slot has.
            to_move = to_move.min(menu.count(&selected_stack) as i32);
            let targets = find_push_slots(menu, &slots, selected, &selected_stack, to_move, false).unwrap_or_default();
            // No target: don't even pick the items up.
            if targets.is_empty() {
                return true;
            }
            // Pick the items up.
            menu.click(selected, Button::Left, false);
            // Right click the needed number of times on each target (no last-slot left click: usually less
            // than the whole stack moves).
            for slot in targets {
                let in_target = menu.stack(slot);
                let have = in_target.as_ref().map_or(0, |s| menu.count(s)) as i32;
                let max = match &in_target {
                    Some(s) => menu.slot_max(slot, s) as i32,
                    None => empty_max(menu, slot),
                };
                let mut times = (max - have).min(to_move);
                to_move -= times;
                while times > 0 {
                    menu.click(slot, Button::Right, false);
                    times -= 1;
                }
            }
            // Put the left-over items down and pick up what was on the mouse.
            menu.click(selected, Button::Left, false);
            return true;
        }

        // Pull items: no more than fit in the slot.
        let mut max_to_move = menu.slot_max(selected, &selected_stack) as i32 - menu.count(&selected_stack) as i32;
        to_move = to_move.min(max_to_move);

        while to_move > 0 {
            // Find a slot to pull from.
            let Some(target) = find_pull_slot(menu, &self.settings, &slots, selected) else {
                break;
            };
            let target_stack = match menu.stack(target) {
                Some(s) => s,
                None => break,
            };
            let in_target = menu.count(&target_stack) as i32;

            if menu.is_crafting_output(target) {
                // One wheel tick is one batch of crafted items, pulled one batch at a time.
                if max_to_move < in_target {
                    break;
                }
                max_to_move -= in_target;
                to_move = (to_move - 1).min(max_to_move);
                if let Some(held) = &on_mouse {
                    if !menu.may_place(selected, held) {
                        break;
                    }
                }
                // Put down the stack on the mouse and pick up the items, take the batch, put the items back.
                menu.click(selected, Button::Left, false);
                menu.click(target, Button::Left, false);
                menu.click(selected, Button::Left, false);
                continue;
            }

            let from_target = to_move.min(in_target);
            max_to_move -= from_target;
            to_move -= from_target;
            if let Some(held) = &on_mouse {
                if !menu.may_place(target, held) {
                    break;
                }
            }
            // Pick up the target's items, putting the mouse stack there.
            menu.click(target, Button::Left, false);
            if from_target == in_target {
                // All of them: one left click on the selected slot.
                menu.click(selected, Button::Left, false);
            } else {
                for _ in 0..from_target {
                    menu.click(selected, Button::Right, false);
                }
            }
            // Put the left-overs back and pick up the stack that was on the mouse.
            menu.click(target, Button::Left, false);
        }
        true
    }

    /// The part of `onMouseScrolled` for the wheel over a crafting output slot.
    #[allow(clippy::too_many_arguments)]
    fn scroll_crafting_output<M: Menu>(
        &mut self,
        menu: &mut M,
        selected: Slot,
        selected_stack: &M::Stack,
        on_mouse: Option<M::Stack>,
        push: bool,
        mut to_move: i32,
        slots: &[Slot],
    ) -> bool {
        // Pulling from the output only works with a compatible stack on the mouse (an empty one is).
        if let Some(held) = &on_mouse {
            if !menu.same(selected_stack, held) {
                return true;
            }
        }
        if on_mouse.is_none() {
            // Can't pull into the crafting output slot.
            if !push {
                return true;
            }
            while to_move > 0 {
                to_move -= 1;
                // The output comes in batches: all of one must be placeable.
                let Some(current) = menu.stack(selected) else {
                    break;
                };
                let batch = menu.count(&current) as i32;
                let Some(targets) = find_push_slots(menu, slots, selected, &current, batch, true) else {
                    break;
                };
                // Grab the batch. (A game that does not give it, the later clicks would take other stacks.)
                menu.click(selected, Button::Left, false);
                if menu.carried().is_none() {
                    break;
                }
                // Distribute it.
                let last = targets.len().saturating_sub(1);
                for (i, slot) in targets.into_iter().enumerate() {
                    if i == last {
                        // The last slot takes what is left with one left click.
                        menu.click(slot, Button::Left, false);
                    } else {
                        let in_target = menu.stack(slot);
                        let have = in_target.as_ref().map_or(0, |s| menu.count(s)) as i32;
                        let max = match &in_target {
                            Some(s) => menu.slot_max(slot, s) as i32,
                            None => empty_max(menu, slot),
                        };
                        let mut times = max - have;
                        while times > 0 {
                            menu.click(slot, Button::Right, false);
                            times -= 1;
                        }
                    }
                }
            }
        } else {
            // Retrieve items from the slot by left clicking it.
            while to_move > 0 {
                to_move -= 1;
                menu.click(selected, Button::Left, false);
            }
        }
        true
    }
}

/// `otherInventoryIsAbove`: more slots of the other inventory above the selected one than below it.
fn other_inventory_is_above<M: Menu>(menu: &M, selected: Slot, slots: &[Slot]) -> bool {
    let selected_in_player = menu.in_player_inventory(selected);
    let (mut below, mut above) = (0, 0);
    let y = menu.slot_y(selected);
    for &slot in slots {
        if menu.in_player_inventory(slot) != selected_in_player {
            if menu.slot_y(slot) < y {
                above += 1;
            } else {
                below += 1;
            }
        }
    }
    above > below
}

/// `findPullSlot`: a slot of the other inventory with a compatible, non-empty stack.
fn find_pull_slot<M: Menu>(menu: &mut M, settings: &Settings, slots: &[Slot], selected: Slot) -> Option<Slot> {
    let selected_stack = menu.stack(selected);
    let find_in_player_inventory = !menu.in_player_inventory(selected);
    let order: Vec<Slot> = match settings.wheel_search_order {
        SearchOrder::FirstToLast => slots.to_vec(),
        SearchOrder::LastToFirst => slots.iter().rev().copied().collect(),
    };
    for slot in order {
        if menu.is_ignored(slot) {
            continue;
        }
        if find_in_player_inventory != menu.in_player_inventory(slot) {
            continue;
        }
        // Can't pull from an empty stack, or an incompatible one.
        let Some(stack) = menu.stack(slot) else {
            continue;
        };
        if let Some(selected_stack) = &selected_stack {
            if !menu.same(selected_stack, &stack) {
                continue;
            }
        }
        return Some(slot);
    }
    None
}

/// `findPushSlots`: the slots of the other inventory `count` items go to, in order. With
/// `must_distribute_all` None when they do not all fit.
fn find_push_slots<M: Menu>(
    menu: &mut M,
    slots: &[Slot],
    selected: Slot,
    selected_stack: &M::Stack,
    count: i32,
    must_distribute_all: bool,
) -> Option<Vec<Slot>> {
    let find_in_player_inventory = !menu.in_player_inventory(selected);
    let mut found = Vec::new();
    // Empty slots can be used once the compatible, non-empty ones run out.
    let mut empty = Vec::new();
    let mut remaining = count;
    for &slot in slots {
        if remaining <= 0 {
            break;
        }
        if menu.is_ignored(slot) {
            continue;
        }
        if find_in_player_inventory != menu.in_player_inventory(slot) {
            continue;
        }
        if menu.is_crafting_output(slot) {
            continue;
        }
        match menu.stack(slot) {
            None => {
                // An empty slot has to take the item.
                if menu.may_place(slot, selected_stack) {
                    empty.push(slot);
                }
            }
            Some(stack) => {
                // A compatible stack that is not full.
                let max = menu.slot_max(slot, &stack) as i32;
                let have = menu.count(&stack) as i32;
                if menu.same(selected_stack, &stack) && have < max {
                    found.push(slot);
                    remaining -= remaining.min(max - have);
                }
            }
        }
    }
    // The empty slots take what is left.
    for slot in empty {
        if remaining <= 0 {
            break;
        }
        found.push(slot);
        remaining -= remaining.min(menu.slot_limit(slot) as i32);
    }
    if must_distribute_all && remaining > 0 {
        return None;
    }
    Some(found)
}

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
