use horizon_mousetweaks::{Button, Menu, ScrollDirection, Scaling, SearchOrder, Settings, Slot, Tweaks};

/// A stack: the item, how many, and the item's own stack size.
#[derive(Clone, Debug, PartialEq)]
struct Stack {
    item: &'static str,
    count: u32,
    max: u32,
}
fn cobble(count: u32) -> Option<Stack> {
    Some(Stack { item: "cobble", count, max: 64 })
}
fn torch(count: u32) -> Option<Stack> {
    Some(Stack { item: "torch", count, max: 64 })
}
fn pearl(count: u32) -> Option<Stack> {
    Some(Stack { item: "pearl", count, max: 16 })
}
fn bundle() -> Option<Stack> {
    Some(Stack { item: "bundle", count: 1, max: 64 })
}

struct FSlot {
    stack: Option<Stack>,
    /// `slot.container == player.getInventory()`.
    player: bool,
    /// A crafting output: whatever is taken is made again.
    output: Option<Stack>,
    may_place: bool,
    ignored: bool,
    limit: u32,
    y: i32,
}

/// A menu with vanilla's `PICKUP` click (`AbstractContainerMenu.doClick`), a shift click that is only
/// written down, and a log of every click the rules made.
struct Fake {
    slots: Vec<FSlot>,
    carried: Option<Stack>,
    shift: bool,
    clicks: Vec<(Slot, Button, bool)>,
    rmb_disabled: u32,
    /// A game whose click on an output gives nothing.
    grab_fails: bool,
}

impl Fake {
    /// `container` slots (not the player's) then `player` slots, from the stacks given.
    fn new(container: Vec<Option<Stack>>, player: Vec<Option<Stack>>) -> Self {
        let mut slots = Vec::new();
        for (i, stack) in container.into_iter().enumerate() {
            slots.push(FSlot { stack, player: false, output: None, may_place: true, ignored: false, limit: 99, y: i as i32 * 18 });
        }
        let base = slots.len() as i32;
        for (i, stack) in player.into_iter().enumerate() {
            slots.push(FSlot { stack, player: true, output: None, may_place: true, ignored: false, limit: 99, y: (base + i as i32) * 18 });
        }
        Fake { slots, carried: None, shift: false, clicks: Vec::new(), rmb_disabled: 0, grab_fails: false }
    }
    fn take_clicks(&mut self) -> Vec<(Slot, Button, bool)> {
        std::mem::take(&mut self.clicks)
    }
    fn slot_max_of(&self, i: usize, s: &Stack) -> u32 {
        self.slots[i].limit.min(s.max)
    }
    /// `Slot.safeInsert(stack, amount)`.
    fn safe_insert(&mut self, i: usize, mut c: Stack, amount: u32) -> Option<Stack> {
        if !self.slots[i].may_place {
            return Some(c);
        }
        let cur = self.slots[i].stack.clone();
        let have = cur.as_ref().map_or(0, |s| s.count);
        let to_insert = amount.min(c.count).min(self.slot_max_of(i, &c).saturating_sub(have));
        match cur {
            None => {
                self.slots[i].stack = Some(Stack { count: to_insert, ..c.clone() });
                c.count -= to_insert;
            }
            Some(s) if s.item == c.item => {
                self.slots[i].stack.as_mut().unwrap().count += to_insert;
                c.count -= to_insert;
            }
            Some(_) => {}
        }
        (c.count > 0).then_some(c)
    }
    fn refill(&mut self, i: usize) {
        if let Some(t) = self.slots[i].output.clone() {
            self.slots[i].stack = Some(t);
        }
    }
    fn pickup(&mut self, i: usize, button: Button) {
        if self.grab_fails && self.slots[i].output.is_some() {
            return;
        }
        let left = button == Button::Left;
        let in_slot = self.slots[i].stack.clone();
        match (in_slot, self.carried.take()) {
            (None, None) => {}
            (None, Some(c)) => {
                let amount = if left { c.count } else { 1 };
                self.carried = self.safe_insert(i, c, amount);
            }
            (Some(s), None) => {
                let amount = if left { s.count } else { s.count.div_ceil(2) };
                let rest = s.count - amount;
                self.slots[i].stack = (rest > 0).then(|| Stack { count: rest, ..s.clone() });
                self.carried = Some(Stack { count: amount, ..s });
                self.refill(i);
            }
            // A bundle on the mouse takes the slot's stack in (not modelled): it stays on the mouse.
            (Some(_), Some(c)) if c.item == "bundle" => self.carried = Some(c),
            (Some(s), Some(mut c)) => {
                if self.slots[i].may_place {
                    if s.item == c.item {
                        let amount = if left { c.count } else { 1 };
                        self.carried = self.safe_insert(i, c, amount);
                    } else if c.count <= self.slot_max_of(i, &c) {
                        self.slots[i].stack = Some(c);
                        self.carried = Some(s);
                    } else {
                        self.carried = Some(c);
                    }
                } else if s.item == c.item {
                    // tryRemove(slotStack.count, carried max - carried count); an output does not allow
                    // modification, so it gives nothing when the whole stack does not fit.
                    let room = c.max - c.count;
                    let output = self.slots[i].output.is_some();
                    if !output || room >= s.count {
                        let n = s.count.min(room);
                        c.count += n;
                        let rest = s.count - n;
                        self.slots[i].stack = (rest > 0).then(|| Stack { count: rest, ..s });
                        self.refill(i);
                    }
                    self.carried = Some(c);
                } else {
                    self.carried = Some(c);
                }
            }
        }
    }
}

impl Menu for Fake {
    type Stack = Stack;
    fn slots(&self) -> Vec<Slot> {
        (0..self.slots.len()).collect()
    }
    fn stack(&mut self, slot: Slot) -> Option<Stack> {
        self.slots[slot].stack.clone()
    }
    fn carried(&mut self) -> Option<Stack> {
        self.carried.clone()
    }
    fn count(&self, stack: &Stack) -> u32 {
        stack.count
    }
    fn same(&self, a: &Stack, b: &Stack) -> bool {
        a.item == b.item
    }
    fn max_stack(&self, stack: &Stack) -> u32 {
        stack.max
    }
    fn slot_max(&self, slot: Slot, stack: &Stack) -> u32 {
        self.slot_max_of(slot, stack)
    }
    fn slot_limit(&self, slot: Slot) -> u32 {
        self.slots[slot].limit
    }
    fn may_place(&self, slot: Slot, _stack: &Stack) -> bool {
        self.slots[slot].may_place
    }
    fn in_player_inventory(&self, slot: Slot) -> bool {
        self.slots[slot].player
    }
    fn is_crafting_output(&self, slot: Slot) -> bool {
        self.slots[slot].output.is_some()
    }
    fn is_ignored(&self, slot: Slot) -> bool {
        self.slots[slot].ignored
    }
    fn slot_y(&self, slot: Slot) -> i32 {
        self.slots[slot].y
    }
    fn shift_down(&self) -> bool {
        self.shift
    }
    fn click(&mut self, slot: Slot, button: Button, shift: bool) {
        self.clicks.push((slot, button, shift));
        if !shift {
            self.pickup(slot, button);
        }
    }
    fn disable_rmb_dragging(&mut self) {
        self.rmb_disabled += 1;
    }
    fn is_bundle(&self, stack: &Stack) -> bool {
        stack.item == "bundle"
    }
}

use Button::{Left, Right};

fn tweaks() -> Tweaks {
    let mut t = Tweaks::new();
    t.update_screen(1, Settings::default);
    t
}
fn count_of(m: &Fake, slot: usize) -> u32 {
    m.slots[slot].stack.as_ref().map_or(0, |s| s.count)
}

// ---- the config file ----

#[test]
fn tuned_config_file_reads_as_the_defaults() {
    // config\MouseTweaks.cfg
    let file = "RMBTweak=1\nLMBTweakWithItem=1\nLMBTweakWithoutItem=1\nWheelTweak=1\nWheelSearchOrder=1\nWheelScrollDirection=0\nScrollItemScaling=0\nDebug=0\n";
    let get = |key: &str| {
        file.lines()
            .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
            .map(str::to_owned)
    };
    let s = Settings::read(get);
    assert_eq!(s, Settings::default());
    assert!(s.rmb_tweak && s.lmb_tweak_with_item && s.lmb_tweak_without_item && s.wheel_tweak);
    assert_eq!(s.wheel_search_order, SearchOrder::LastToFirst);
    assert_eq!(s.wheel_scroll_direction, ScrollDirection::Normal);
    assert_eq!(s.scroll_item_scaling, Scaling::Proportional);
}

#[test]
fn other_values_and_bad_text_follow_the_originals_rules() {
    let file = "RMBTweak=0\nLMBTweakWithItem=abc\nWheelSearchOrder=0\nWheelScrollDirection=7\nScrollItemScaling=1\n";
    let get = |key: &str| {
        file.lines()
            .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
            .map(str::to_owned)
    };
    let s = Settings::read(get);
    assert!(!s.rmb_tweak);
    // A value that is not a number is the default.
    assert!(s.lmb_tweak_with_item);
    assert_eq!(s.wheel_search_order, SearchOrder::FirstToLast);
    // fromId: anything past 2 is INVENTORY_POSITION_AWARE_INVERTED.
    assert_eq!(s.wheel_scroll_direction, ScrollDirection::PositionAwareInverted);
    assert_eq!(s.scroll_item_scaling, Scaling::AlwaysOne);
}

// ---- RMB drag ----

#[test]
fn rmb_drag_puts_one_item_in_each_slot_it_enters_and_the_one_it_left() {
    let mut m = Fake::new(vec![None, None, None], vec![]);
    m.carried = cobble(4);
    let mut t = tweaks();
    t.press(&mut m, Some(0), Right);
    // Nothing is clicked while the mouse stays in the first slot (vanilla's own release places one).
    t.drag(&mut m, Some(0), Right);
    assert!(m.clicks.is_empty());
    // Entering the second slot: the first gets its item, then the second.
    t.drag(&mut m, Some(1), Right);
    assert_eq!(m.take_clicks(), vec![(0, Right, false), (1, Right, false)]);
    assert_eq!(m.rmb_disabled, 1);
    t.drag(&mut m, Some(2), Right);
    assert_eq!(m.take_clicks(), vec![(2, Right, false)]);
    // The vanilla drag is disabled only once.
    assert_eq!(m.rmb_disabled, 1);
    assert_eq!((count_of(&m, 0), count_of(&m, 1), count_of(&m, 2)), (1, 1, 1));
    assert_eq!(m.carried.as_ref().map(|s| s.count), Some(1));
}

#[test]
fn rmb_drag_adds_to_a_matching_slot_and_skips_full_other_and_ignored_ones() {
    let mut m = Fake::new(vec![None, cobble(10), cobble(64), torch(3), None, None], vec![]);
    m.slots[4].ignored = true;
    m.slots[5].output = cobble(2);
    m.carried = cobble(20);
    let mut t = tweaks();
    t.press(&mut m, Some(0), Right);
    for slot in 1..=5 {
        t.drag(&mut m, Some(slot), Right);
    }
    // 0 (left), 1 (matching), not 2 (full), not 3 (another item), not 4 (ignored), not 5 (an output).
    assert_eq!(m.take_clicks(), vec![(0, Right, false), (1, Right, false)]);
    assert_eq!(count_of(&m, 1), 11);
    assert_eq!(count_of(&m, 3), 3);
}

#[test]
fn rmb_drag_needs_a_stack_on_the_mouse_and_the_tweak_on() {
    let mut m = Fake::new(vec![None, None], vec![]);
    let mut t = tweaks();
    // Empty hand at the press.
    t.press(&mut m, Some(0), Right);
    m.carried = cobble(4);
    t.drag(&mut m, Some(1), Right);
    assert!(m.clicks.is_empty());
    t.release(Right);
    // The tweak off.
    t.settings.rmb_tweak = false;
    t.press(&mut m, Some(0), Right);
    t.drag(&mut m, Some(1), Right);
    assert!(m.clicks.is_empty());
}

#[test]
fn rmb_drag_with_the_last_item_stops_when_the_hand_is_empty() {
    let mut m = Fake::new(vec![None, cobble(5)], vec![]);
    m.carried = cobble(1);
    let mut t = tweaks();
    t.press(&mut m, Some(0), Right);
    t.drag(&mut m, Some(1), Right);
    // The item went into the first slot; with an empty hand a click on the next would take half of it.
    assert_eq!(m.take_clicks(), vec![(0, Right, false)]);
    assert_eq!(count_of(&m, 1), 5);
}

#[test]
fn rmb_drag_fills_a_bundle_whatever_the_slot_holds() {
    let mut m = Fake::new(vec![cobble(64), torch(2)], vec![]);
    m.carried = bundle();
    let mut t = tweaks();
    t.press(&mut m, Some(0), Right);
    t.drag(&mut m, Some(1), Right);
    // Both clicks are made: with a bundle the compatibility and fullness checks are skipped.
    assert_eq!(m.take_clicks(), vec![(0, Right, false), (1, Right, false)]);
}

#[test]
fn a_release_ends_the_rmb_drag() {
    let mut m = Fake::new(vec![None, None, None], vec![]);
    m.carried = cobble(4);
    let mut t = tweaks();
    t.press(&mut m, Some(0), Right);
    t.release(Right);
    t.drag(&mut m, Some(1), Right);
    assert!(m.clicks.is_empty());
}

// ---- LMB drag ----

#[test]
fn shift_lmb_drag_without_item_quick_moves_every_slot_with_items() {
    let mut m = Fake::new(vec![cobble(3), None, torch(5)], vec![]);
    m.shift = true;
    let mut t = tweaks();
    t.press(&mut m, Some(0), Left);
    t.drag(&mut m, Some(1), Left);
    t.drag(&mut m, Some(2), Left);
    // The empty slot is skipped.
    assert_eq!(m.take_clicks(), vec![(2, Left, true)]);
}

#[test]
fn lmb_drag_without_item_does_nothing_without_shift_or_with_the_tweak_off() {
    let mut m = Fake::new(vec![cobble(3), cobble(4)], vec![]);
    let mut t = tweaks();
    t.press(&mut m, Some(0), Left);
    t.drag(&mut m, Some(1), Left);
    assert!(m.clicks.is_empty());
    m.shift = true;
    t.settings.lmb_tweak_without_item = false;
    t.drag(&mut m, Some(0), Left);
    assert!(m.clicks.is_empty());
}

#[test]
fn lmb_drag_with_item_gathers_matching_stacks_onto_the_mouse() {
    let mut m = Fake::new(vec![cobble(10), cobble(20), torch(4), cobble(40)], vec![]);
    let mut t = tweaks();
    // The press takes the first stack (empty hand at the press: vanilla picks it up).
    t.press(&mut m, Some(0), Left);
    m.click(0, Left, false);
    m.take_clicks();
    assert_eq!(m.carried, cobble(10));
    t.drag(&mut m, Some(1), Left);
    assert_eq!(m.take_clicks(), vec![(1, Left, false), (1, Left, false)]);
    assert_eq!(m.carried, cobble(30));
    assert_eq!(m.slots[1].stack, None);
    // Another item: left alone.
    t.drag(&mut m, Some(2), Left);
    assert!(m.clicks.is_empty());
    // 30 + 40 would not fit on the mouse (64): left alone.
    t.drag(&mut m, Some(3), Left);
    assert!(m.clicks.is_empty());
    assert_eq!(count_of(&m, 3), 40);
}

#[test]
fn lmb_drag_with_item_and_shift_quick_moves_the_slots_with_the_same_item() {
    let mut m = Fake::new(vec![cobble(10), cobble(20), torch(4)], vec![]);
    let mut t = tweaks();
    t.press(&mut m, Some(0), Left);
    m.click(0, Left, false);
    m.take_clicks();
    m.shift = true;
    t.drag(&mut m, Some(1), Left);
    t.drag(&mut m, Some(2), Left);
    assert_eq!(m.take_clicks(), vec![(1, Left, true)]);
}

#[test]
fn lmb_drag_over_a_crafting_output_takes_it_with_one_click() {
    let mut m = Fake::new(vec![cobble(10), None], vec![]);
    m.slots[1].output = cobble(2);
    m.slots[1].stack = cobble(2);
    m.slots[1].may_place = false;
    let mut t = tweaks();
    t.press(&mut m, Some(0), Left);
    m.click(0, Left, false);
    m.take_clicks();
    t.drag(&mut m, Some(1), Left);
    assert_eq!(m.take_clicks(), vec![(1, Left, false)]);
    assert_eq!(m.carried, cobble(12));
}

#[test]
fn lmb_drag_with_a_stack_held_at_the_press_is_vanillas_own_drag() {
    let mut m = Fake::new(vec![cobble(10), cobble(20)], vec![]);
    m.carried = cobble(5);
    let mut t = tweaks();
    t.press(&mut m, Some(0), Left);
    t.drag(&mut m, Some(1), Left);
    assert!(m.clicks.is_empty());
}

#[test]
fn lmb_drag_with_item_off() {
    let mut m = Fake::new(vec![cobble(10), cobble(20)], vec![]);
    let mut t = tweaks();
    t.settings.lmb_tweak_with_item = false;
    t.press(&mut m, Some(0), Left);
    m.carried = cobble(10);
    t.drag(&mut m, Some(1), Left);
    assert!(m.clicks.is_empty());
}

#[test]
fn a_drag_over_nothing_or_back_to_the_same_slot_does_nothing() {
    let mut m = Fake::new(vec![cobble(10), cobble(20)], vec![]);
    m.shift = true;
    let mut t = tweaks();
    t.press(&mut m, Some(0), Left);
    t.drag(&mut m, None, Left);
    assert!(m.clicks.is_empty());
    // The slot left is now "none", so entering slot 0 again is entering a new slot.
    t.drag(&mut m, Some(0), Left);
    assert_eq!(m.take_clicks(), vec![(0, Left, true)]);
    t.drag(&mut m, Some(0), Left);
    assert!(m.clicks.is_empty());
}

#[test]
fn a_new_screen_forgets_the_drag() {
    let mut m = Fake::new(vec![cobble(10), cobble(20)], vec![]);
    m.shift = true;
    let mut t = tweaks();
    t.press(&mut m, Some(0), Left);
    t.update_screen(2, || Settings { lmb_tweak_without_item: false, ..Settings::default() });
    t.drag(&mut m, Some(1), Left);
    assert!(m.clicks.is_empty());
    // The same screen id again keeps the settings the new screen read.
    t.update_screen(2, Settings::default);
    assert!(!t.settings.lmb_tweak_without_item);
}

// ---- the wheel ----

/// Three container slots then three of the player's.
fn chest(container: [Option<Stack>; 3], player: [Option<Stack>; 3]) -> Fake {
    Fake::new(container.to_vec(), player.to_vec())
}

#[test]
fn wheel_down_pushes_one_item_to_the_other_inventory() {
    let mut m = chest([cobble(10), None, None], [cobble(5), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert_eq!(m.take_clicks(), vec![(3, Left, false), (0, Right, false), (3, Left, false)]);
    assert_eq!(count_of(&m, 3), 4);
    assert_eq!(count_of(&m, 0), 11);
    assert_eq!(m.carried, None);
}

#[test]
fn wheel_down_fills_partial_stacks_before_empty_slots_and_in_menu_order() {
    // Two partial stacks (room 2 and 40) and an empty slot between them.
    let mut m = Fake::new(vec![cobble(62), None, cobble(24)], vec![cobble(30)]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -5.0));
    // The first partial slot takes 2, the second 3.
    assert_eq!(count_of(&m, 0), 64);
    assert_eq!(count_of(&m, 2), 27);
    assert_eq!(count_of(&m, 1), 0);
    assert_eq!(count_of(&m, 3), 25);
}

#[test]
fn wheel_down_uses_an_empty_slot_when_the_stacks_are_full() {
    let mut m = chest([cobble(64), None, torch(3)], [cobble(5), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -2.0));
    assert_eq!(count_of(&m, 1), 2);
    assert_eq!(count_of(&m, 3), 3);
    assert_eq!(m.carried, None);
}

#[test]
fn wheel_down_with_nowhere_to_go_moves_nothing_but_keeps_the_wheel() {
    let mut m = chest([cobble(64), torch(1), torch(1)], [cobble(5), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert!(m.clicks.is_empty());
}

#[test]
fn wheel_does_not_move_more_items_than_the_slot_has() {
    let mut m = chest([None, None, None], [cobble(2), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -5.0));
    assert_eq!(count_of(&m, 0), 2);
    assert_eq!(count_of(&m, 3), 0);
}

#[test]
fn wheel_up_pulls_from_the_last_slot_first_by_default() {
    let mut m = chest([cobble(10), None, cobble(3)], [cobble(5), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), 1.0));
    assert_eq!(m.take_clicks(), vec![(2, Left, false), (3, Right, false), (2, Left, false)]);
    assert_eq!(count_of(&m, 3), 6);
    assert_eq!(count_of(&m, 2), 2);
    assert_eq!(count_of(&m, 0), 10);
    assert_eq!(m.carried, None);
}

#[test]
fn wheel_up_pulls_from_the_first_slot_first_when_the_search_order_says_so() {
    let mut m = chest([cobble(10), None, cobble(3)], [cobble(5), None, None]);
    let mut t = tweaks();
    t.settings.wheel_search_order = SearchOrder::FirstToLast;
    assert!(t.scroll(&mut m, Some(3), 1.0));
    assert_eq!(count_of(&m, 0), 9);
    assert_eq!(count_of(&m, 2), 3);
}

#[test]
fn wheel_up_takes_a_whole_small_stack_with_one_left_click() {
    let mut m = chest([None, None, cobble(1)], [cobble(5), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), 1.0));
    assert_eq!(m.take_clicks(), vec![(2, Left, false), (3, Left, false), (2, Left, false)]);
    assert_eq!(count_of(&m, 3), 6);
    assert_eq!(count_of(&m, 2), 0);
}

#[test]
fn wheel_up_runs_on_through_several_slots_and_stops_when_the_slot_is_full() {
    let mut m = chest([cobble(1), cobble(1), cobble(10)], [cobble(62), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), 5.0));
    // Room for 2: the last slot gives 2 (a right click each), nothing more.
    assert_eq!(count_of(&m, 3), 64);
    assert_eq!(count_of(&m, 2), 8);
    assert_eq!(count_of(&m, 0) + count_of(&m, 1), 2);
}

#[test]
fn wheel_up_only_pulls_compatible_stacks() {
    let mut m = chest([torch(9), None, None], [cobble(5), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), 1.0));
    assert!(m.clicks.is_empty());
    // Pulling into an item that is not in the other inventory at all.
    assert_eq!(count_of(&m, 0), 9);
}

#[test]
fn the_wheel_adds_up_small_deltas_and_forgets_them_when_the_direction_changes() {
    let mut m = chest([cobble(10), None, None], [cobble(5), None, None]);
    let mut t = tweaks();
    // 0.4 + 0.4 stays under a whole step, the wheel is still taken.
    assert!(t.scroll(&mut m, Some(3), -0.4));
    assert!(t.scroll(&mut m, Some(3), -0.4));
    assert!(m.clicks.is_empty());
    // 1.2 in all: one item, 0.2 kept.
    assert!(t.scroll(&mut m, Some(3), -0.4));
    assert_eq!(count_of(&m, 3), 4);
    // Up: the 0.2 left over is dropped, 0.3 of a step moves nothing.
    assert!(t.scroll(&mut m, Some(3), 0.3));
    assert_eq!(count_of(&m, 3), 4);
    assert!(t.scroll(&mut m, Some(3), 0.8));
    assert_eq!(count_of(&m, 3), 5);
}

#[test]
fn always_one_scaling_makes_every_tick_one_item() {
    let mut m = chest([cobble(10), None, None], [cobble(9), None, None]);
    let mut t = tweaks();
    t.settings.scroll_item_scaling = Scaling::AlwaysOne;
    assert!(t.scroll(&mut m, Some(3), -0.1));
    assert_eq!(count_of(&m, 3), 8);
    assert!(t.scroll(&mut m, Some(3), -7.0));
    assert_eq!(count_of(&m, 3), 7);
}

#[test]
fn the_inverted_directions_swap_pushing_and_pulling() {
    let mut m = chest([cobble(10), None, None], [cobble(5), None, None]);
    let mut t = tweaks();
    t.settings.wheel_scroll_direction = ScrollDirection::Inverted;
    // Down now pulls: the chest's 10 gives one.
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert_eq!((count_of(&m, 0), count_of(&m, 3)), (9, 6));
    assert!(t.scroll(&mut m, Some(3), 1.0));
    assert_eq!((count_of(&m, 0), count_of(&m, 3)), (10, 5));
}

#[test]
fn the_position_aware_direction_depends_on_where_the_other_inventory_is() {
    // Container above the player's slots (y grows downwards): wheel down is toward the player? No:
    // down pushes away from the selected slot, so with the other inventory above it pulls instead.
    let mut m = chest([cobble(10), None, None], [cobble(5), None, None]);
    let mut t = tweaks();
    t.settings.wheel_scroll_direction = ScrollDirection::PositionAware;
    assert!(t.scroll(&mut m, Some(3), -1.0));
    // Other inventory above the selected (player's) slot: the direction flips, down pulls.
    assert_eq!((count_of(&m, 0), count_of(&m, 3)), (9, 6));
    // From the container, the player's slots are below: down pushes as usual.
    assert!(t.scroll(&mut m, Some(0), -1.0));
    assert_eq!((count_of(&m, 0), count_of(&m, 3)), (8, 7));
    // The inverted variant flips it back.
    t.settings.wheel_scroll_direction = ScrollDirection::PositionAwareInverted;
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert_eq!((count_of(&m, 0), count_of(&m, 3)), (9, 6));
}

#[test]
fn the_wheel_over_nothing_an_ignored_slot_or_with_the_tweak_off_is_not_taken() {
    let mut m = chest([cobble(10), None, None], [cobble(5), None, None]);
    m.slots[3].ignored = true;
    let mut t = tweaks();
    assert!(!t.scroll(&mut m, None, -1.0));
    assert!(!t.scroll(&mut m, Some(3), -1.0));
    t.settings.wheel_tweak = false;
    assert!(!t.scroll(&mut m, Some(0), -1.0));
    assert!(m.clicks.is_empty());
}

#[test]
fn the_wheel_over_an_empty_slot_is_taken_and_does_nothing() {
    let mut m = chest([cobble(10), None, None], [None, None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert!(m.clicks.is_empty());
}

#[test]
fn the_wheel_does_nothing_while_the_mouse_holds_the_same_item() {
    let mut m = chest([cobble(10), None, None], [cobble(5), None, None]);
    m.carried = cobble(3);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert!(m.clicks.is_empty());
}

#[test]
fn the_wheel_swaps_through_another_item_held_on_the_mouse() {
    let mut m = chest([cobble(10), None, None], [cobble(5), None, None]);
    m.carried = torch(7);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert_eq!(m.take_clicks(), vec![(3, Left, false), (0, Right, false), (3, Left, false)]);
    // One cobblestone went over; the torches are back on the mouse and the rest in the slot.
    assert_eq!(count_of(&m, 0), 11);
    assert_eq!(count_of(&m, 3), 4);
    assert_eq!(m.carried, torch(7));
}

#[test]
fn the_wheel_does_not_push_a_held_item_into_a_slot_that_refuses_it() {
    let mut m = chest([cobble(10), None, None], [cobble(5), None, None]);
    m.slots[3].may_place = false;
    m.carried = torch(7);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert!(m.clicks.is_empty());
}

#[test]
fn an_empty_slot_that_refuses_the_item_is_not_a_target() {
    let mut m = chest([None, None, None], [cobble(5), None, None]);
    m.slots[0].may_place = false;
    m.slots[1].may_place = false;
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), -1.0));
    assert_eq!(count_of(&m, 2), 1);
    assert_eq!(count_of(&m, 0) + count_of(&m, 1), 0);
}

#[test]
fn an_item_with_a_small_stack_size_limits_the_pull() {
    let mut m = chest([pearl(16), None, None], [pearl(15), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(3), 3.0));
    assert_eq!(count_of(&m, 3), 16);
    assert_eq!(count_of(&m, 0), 15);
}

// ---- crafting output ----

/// Slot 0 is a crafting output of 3 cobblestone; 1 to 3 are the player's slots.
fn bench(player: [Option<Stack>; 3]) -> Fake {
    let mut m = Fake::new(vec![cobble(3)], player.to_vec());
    m.slots[0].output = cobble(3);
    m.slots[0].may_place = false;
    m
}

#[test]
fn the_wheel_down_on_an_output_distributes_a_whole_batch() {
    let mut m = bench([cobble(62), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(0), -1.0));
    // The partial stack gets 2 by right clicks, the empty slot the last item by a left click.
    assert_eq!(
        m.take_clicks(),
        vec![(0, Left, false), (1, Right, false), (1, Right, false), (2, Left, false)]
    );
    assert_eq!((count_of(&m, 1), count_of(&m, 2)), (64, 1));
    assert_eq!(m.carried, None);
    // The output made its batch again.
    assert_eq!(count_of(&m, 0), 3);
}

#[test]
fn the_wheel_down_on_an_output_does_nothing_when_the_batch_does_not_fit() {
    let mut m = bench([cobble(63), torch(1), torch(1)]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(0), -1.0));
    assert!(m.clicks.is_empty());
    assert_eq!(count_of(&m, 0), 3);
}

#[test]
fn the_wheel_up_on_an_output_with_an_empty_hand_does_nothing() {
    let mut m = bench([None, None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(0), 1.0));
    assert!(m.clicks.is_empty());
}

#[test]
fn the_wheel_on_an_output_with_the_same_item_on_the_mouse_crafts_once_per_tick() {
    let mut m = bench([None, None, None]);
    m.carried = cobble(10);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(0), -2.0));
    assert_eq!(m.take_clicks(), vec![(0, Left, false), (0, Left, false)]);
    assert_eq!(m.carried, cobble(16));
}

#[test]
fn the_wheel_on_an_output_with_another_item_on_the_mouse_does_nothing() {
    let mut m = bench([None, None, None]);
    m.carried = torch(10);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(0), -1.0));
    assert!(m.clicks.is_empty());
}

#[test]
fn the_wheel_up_over_a_slot_pulls_one_crafting_batch_from_an_output() {
    let mut m = bench([cobble(10), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(1), 1.0));
    assert_eq!(m.take_clicks(), vec![(1, Left, false), (0, Left, false), (1, Left, false)]);
    // The batch of 3 was added: 13 in the slot.
    assert_eq!(count_of(&m, 1), 13);
    assert_eq!(m.carried, None);
}

#[test]
fn the_wheel_does_not_pull_a_batch_that_does_not_fit_the_slot() {
    let mut m = bench([cobble(62), None, None]);
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(1), 1.0));
    assert!(m.clicks.is_empty());
}

#[test]
fn an_output_is_never_a_target_of_pushing() {
    let mut m = Fake::new(vec![cobble(3), None], vec![cobble(5)]);
    m.slots[0].output = cobble(3);
    m.slots[0].may_place = false;
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(2), -1.0));
    // The only other-side slot that takes it is the empty one, not the output.
    assert_eq!(count_of(&m, 1), 1);
    assert_eq!(count_of(&m, 0), 3);
}

#[test]
fn a_batch_the_game_does_not_give_is_not_followed_by_clicks_on_other_slots() {
    let mut m = bench([cobble(10), None, None]);
    m.grab_fails = true;
    let mut t = tweaks();
    assert!(t.scroll(&mut m, Some(0), -1.0));
    // Only the grab was tried: nothing was taken from the player's slots.
    assert_eq!(m.take_clicks(), vec![(0, Left, false)]);
    assert_eq!(count_of(&m, 1), 10);
    assert_eq!(m.carried, None);
}