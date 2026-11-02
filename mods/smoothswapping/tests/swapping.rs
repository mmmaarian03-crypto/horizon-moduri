use horizon_smoothswapping::{
    CURSOR, ClickKind, Config, Curve, Smooth, SlotView, Stack, java_hash_order,
};

#[derive(Clone, Debug, PartialEq)]
struct T {
    item: &'static str,
    count: i32,
    damaged: bool,
}
const AIR: T = T {
    item: "",
    count: 0,
    damaged: false,
};
fn t(item: &'static str, count: i32) -> T {
    T {
        item,
        count,
        damaged: false,
    }
}
impl Stack for T {
    fn is_empty(&self) -> bool {
        self.item.is_empty() || self.count == 0
    }
    fn count(&self) -> i32 {
        if self.is_empty() { 0 } else { self.count }
    }
    fn same_item(&self, o: &Self) -> bool {
        (self.is_empty() && o.is_empty()) || self.item == o.item
    }
    fn same(&self, o: &Self) -> bool {
        (self.is_empty() && o.is_empty()) || self == o
    }
    fn bar_visible(&self) -> bool {
        self.damaged
    }
}

/// A row of slots 18 pixels apart; `output` marks result slots.
fn row(stacks: &[T], outputs: &[usize]) -> Vec<SlotView<T>> {
    stacks
        .iter()
        .enumerate()
        .map(|(i, s)| SlotView {
            x: i as f64 * 18.0,
            y: 0.0,
            stack: s.clone(),
            output: outputs.contains(&i),
        })
        .collect()
}
fn smooth() -> Smooth<T> {
    Smooth::new(Config::default())
}
/// Show a screen once, so that it has a first state.
fn open(s: &mut Smooth<T>, stacks: &[T]) {
    s.frame(&row(stacks, &[]), AIR, (0.0, 0.0), 1, false);
}
/// Rows with one more slot at the end, which stays full: the first change of a session sets what the
/// original keeps of the cursor, so the cursor tests make one before the change they are about.
fn rowd(stacks: &[T]) -> Vec<SlotView<T>> {
    let mut v = stacks.to_vec();
    v.push(t("dummy", 1));
    row(&v, &[])
}
fn prime(s: &mut Smooth<T>, stacks: &[T], cursor: T, mouse: (f64, f64)) {
    let mut v = stacks.to_vec();
    v.push(AIR);
    s.frame(&row(&v, &[]), cursor.clone(), mouse, 1, false);
    s.frame(&rowd(stacks), cursor, mouse, 1, false);
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// ---- the curve ------------------------------------------------------------------------------------

#[test]
fn the_default_curve_is_a_gentle_s() {
    let c = Curve::from_points(&[]);
    // -t^3 + 1.5 t^2 + 0.5 t, worked out from the original's Catmull-Rom with its two end points
    // doubled.
    for i in 0..=100 {
        let t = f64::from(i) / 100.0;
        let want = -t * t * t + 1.5 * t * t + 0.5 * t;
        assert!(close(c.progress(t), want), "{t}: {} vs {want}", c.progress(t));
    }
    assert!(close(c.progress(0.0), 0.0));
    assert!(close(c.progress(0.5), 0.5));
    assert!(close(c.progress(1.0), 1.0), "t = 1 falls outside [0,1) and uses the first piece");
    // Slow at the start (below the diagonal), quick in the second half.
    assert!(c.progress(0.2) < 0.2);
    assert!(c.progress(0.8) > 0.8);
}

#[test]
fn a_curve_passes_through_its_points() {
    let c = Curve::from_points(&[[0.5, 0.2]]);
    // The points are floats in the options, so 0.2 is 0.2f32.
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    assert!(near(c.progress(0.5), 0.2));
    assert!(near(c.progress(0.0), 0.0));
    // Continuous across the join.
    assert!((c.progress(0.4999) - c.progress(0.5001)).abs() < 0.01);
    // A point can make it overshoot or lag: this one lags in the first half.
    assert!(c.progress(0.25) < 0.25);
    let two = Curve::from_points(&[[0.3, 0.6], [0.7, 0.4]]);
    assert!(near(two.progress(0.3), 0.6));
    assert!(near(two.progress(0.7), 0.4));
}

// ---- Java's HashMap order -------------------------------------------------------------------------

#[test]
fn small_keys_come_out_in_order_and_big_ones_wrap_round_the_table() {
    assert_eq!(java_hash_order(&[1, 2, 3]), vec![1, 2, 3]);
    assert_eq!(java_hash_order(&[]), Vec::<usize>::new());
    // 20 lands in bucket 4 of 16, before 5.
    assert_eq!(java_hash_order(&[5, 20]), vec![20, 5]);
    // 36 is bucket 4, 9 is bucket 9, 40 is bucket 8.
    assert_eq!(java_hash_order(&[9, 36, 40]), vec![36, 40, 9]);
    // Same bucket: insertion (ascending) order.
    assert_eq!(java_hash_order(&[4, 20, 36]), vec![4, 20, 36]);
    // Thirteen keys outgrow the table (12 entries at most in 16): it doubles to 32.
    let keys: Vec<usize> = (0..13).map(|i| i * 3).collect();
    let order = java_hash_order(&keys);
    let mut sorted = order.clone();
    sorted.sort_by_key(|&k| (k & 31, k));
    assert_eq!(order, sorted);
}

// ---- items on their way ---------------------------------------------------------------------------

#[test]
fn a_shift_click_into_an_empty_slot_sends_the_item_across() {
    let mut s = smooth();
    open(&mut s, &[t("cobble", 32), AIR, AIR]);
    // The item moves from slot 0 to slot 2.
    s.frame(&row(&[AIR, AIR, t("cobble", 32)], &[]), AIR, (0.0, 0.0), 1, false);
    let swaps = s.swaps_to(2);
    assert_eq!(swaps.len(), 1);
    assert_eq!(swaps[0].amount(), 32);
    assert!(!swaps[0].is_checked(), "the slot had a different (empty) stack");
    assert!(close(swaps[0].distance(), 36.0));
    assert_eq!(swaps[0].item(), &t("cobble", 32));
    assert!(s.swaps_to(0).is_empty());
}

#[test]
fn the_mover_starts_at_its_source_and_ends_in_its_slot() {
    let mut s = smooth();
    open(&mut s, &[t("cobble", 32), AIR, AIR]);
    s.frame(&row(&[AIR, AIR, t("cobble", 32)], &[]), AIR, (0.0, 0.0), 1, false);
    // First frame (nothing elapsed): drawn at the source, 36 pixels to the left of the slot.
    let plan = s.slot_model(2, 0.0).expect("a plan");
    assert_eq!(plan.flying.len(), 1);
    assert!(close(plan.flying[0].dx, -36.0) && close(plan.flying[0].dy, 0.0), "{:?}", plan.flying[0]);
    assert!(!plan.draw_destination, "an unchecked swap hides the slot's own item until it arrives");
    // Then it slides right until it has arrived.
    let mut last = plan.flying[0].dx;
    let mut frames = 0;
    while let Some(plan) = s.slot_model(2, 0.5) {
        frames += 1;
        assert!(frames < 100);
        let dx = plan.flying.first().map_or(0.0, |f| f.dx);
        assert!(dx >= last - 1e-9, "never backwards: {dx} after {last}");
        assert!(dx <= 1.0, "stays inside its slot at the end");
        last = dx;
    }
    assert!(s.swaps_to(2).is_empty());
    assert!(!s.busy());
    // Ten ticks at half a tick a frame, a frame or two more for the crossing.
    assert!((19..=24).contains(&frames), "{frames}");
}

#[test]
fn the_speed_option_scales_the_flight() {
    let frames_at = |speed: i32| {
        let mut s = Smooth::new(Config {
            animation_speed: speed,
            ..Config::default()
        });
        open(&mut s, &[t("cobble", 32), AIR, AIR]);
        s.frame(&row(&[AIR, AIR, t("cobble", 32)], &[]), AIR, (0.0, 0.0), 1, false);
        let mut n = 0;
        while s.slot_model(2, 0.5).is_some() {
            n += 1;
            assert!(n < 500);
        }
        n
    };
    let normal = frames_at(100);
    let double = frames_at(200);
    assert!(double * 2 <= normal + 2 && double * 2 + 2 >= normal, "{normal} {double}");
    assert!(frames_at(50) > normal);
}

#[test]
fn a_flight_ten_ticks_long_at_speed_100() {
    // Whole ticks: distance 36 covered at 3.6 per tick; the item passes the target during the
    // eleventh tick.
    let mut s = smooth();
    open(&mut s, &[t("cobble", 32), AIR, AIR]);
    s.frame(&row(&[AIR, AIR, t("cobble", 32)], &[]), AIR, (0.0, 0.0), 1, false);
    let mut ticks = 0;
    while s.slot_model(2, 1.0).is_some() {
        ticks += 1;
    }
    // It reaches the slot at the tenth step; whether the rounding of that step puts it just
    // past the target or just short decides if the eleventh draw is needed.
    assert!((10..=11).contains(&ticks), "{ticks}");
}

#[test]
fn merging_into_the_same_item_keeps_the_slots_own_item_drawn() {
    let mut s = smooth();
    open(&mut s, &[t("cobble", 32), t("cobble", 16)]);
    // 10 of slot 0's items go to slot 1.
    s.frame(
        &row(&[t("cobble", 22), t("cobble", 26)], &[]),
        AIR,
        (0.0, 0.0),
        1,
        false,
    );
    let swaps = s.swaps_to(1);
    assert_eq!(swaps.len(), 1);
    assert!(swaps[0].is_checked());
    assert_eq!(swaps[0].amount(), 10);
    let plan = s.slot_model(1, 0.0).unwrap();
    assert!(plan.draw_destination);
    // The slot's count shows what it had, the mover its own amount.
    let overlay = s.slot_overlay(1, &t("cobble", 26)).unwrap();
    assert_eq!(overlay.destination, Some(16));
    assert_eq!(overlay.flying.len(), 1);
    assert_eq!(overlay.flying[0].text, Some(10));
}

#[test]
fn an_unchecked_swap_hides_the_slots_count_too() {
    let mut s = smooth();
    open(&mut s, &[t("cobble", 32), AIR]);
    s.frame(&row(&[AIR, t("cobble", 32)], &[]), AIR, (0.0, 0.0), 1, false);
    s.slot_model(1, 0.0).unwrap();
    let overlay = s.slot_overlay(1, &t("cobble", 32)).unwrap();
    assert_eq!(overlay.destination, None);
    assert_eq!(overlay.flying.len(), 1);
    assert_eq!(overlay.flying[0].text, Some(32));
}

#[test]
fn a_mover_of_one_item_has_no_count_but_a_damaged_one_has_its_bar() {
    let mut s = smooth();
    open(&mut s, &[t("cobble", 1), AIR]);
    s.frame(&row(&[AIR, t("cobble", 1)], &[]), AIR, (0.0, 0.0), 1, false);
    s.slot_model(1, 0.0).unwrap();
    let overlay = s.slot_overlay(1, &t("cobble", 1)).unwrap();
    assert!(overlay.flying.is_empty());
    assert_eq!(overlay.destination, None);

    let mut s = smooth();
    let pick = T {
        item: "pickaxe",
        count: 1,
        damaged: true,
    };
    open(&mut s, &[pick.clone(), AIR]);
    s.frame(&row(&[AIR, pick.clone()], &[]), AIR, (0.0, 0.0), 1, false);
    s.slot_model(1, 0.0).unwrap();
    let overlay = s.slot_overlay(1, &pick).unwrap();
    assert_eq!(overlay.flying.len(), 1);
    assert_eq!(overlay.flying[0].text, None, "the stack's own bar, no amount");
}

#[test]
fn the_overlay_of_a_changed_stack_forgets_the_movers() {
    let mut s = smooth();
    open(&mut s, &[t("cobble", 32), AIR]);
    s.frame(&row(&[AIR, t("cobble", 32)], &[]), AIR, (0.0, 0.0), 1, false);
    assert!(s.slot_overlay(1, &t("dirt", 5)).is_none());
    assert!(s.swaps_to(1).is_empty());
}

#[test]
fn items_are_paired_slot_by_slot_in_the_order_of_the_lists() {
    let mut s = smooth();
    open(&mut s, &[t("a", 10), t("b", 5), AIR, AIR]);
    s.frame(&row(&[AIR, AIR, t("a", 10), t("b", 5)], &[]), AIR, (0.0, 0.0), 1, false);
    let to2 = s.swaps_to(2);
    let to3 = s.swaps_to(3);
    assert_eq!((to2.len(), to3.len()), (1, 1));
    // Slot 2 gained 10 and is paired with slot 0 (lost 10), slot 3 with slot 1: the distances show
    // where each came from (2 slots away both times).
    assert_eq!(to2[0].amount(), 10);
    assert_eq!(to3[0].amount(), 5);
    assert!(close(to2[0].distance(), 36.0) && close(to3[0].distance(), 36.0));
}

#[test]
fn one_source_can_fill_two_slots() {
    let mut s = smooth();
    open(&mut s, &[t("a", 12), AIR, AIR]);
    s.frame(&row(&[AIR, t("a", 4), t("a", 8)], &[]), AIR, (0.0, 0.0), 1, false);
    let amounts: i32 = [1, 2]
        .iter()
        .flat_map(|&i| s.swaps_to(i).iter().map(|w| w.amount()))
        .sum();
    assert_eq!(amounts, 12);
    assert_eq!(s.swaps_to(1)[0].amount(), 4);
    assert_eq!(s.swaps_to(2)[0].amount(), 8);
}

// ---- items on their way to the cursor -------------------------------------------------------------

/// A cursor with `n` of an item.
fn holding(item: &'static str, n: i32) -> T {
    t(item, n)
}

#[test]
fn collecting_with_a_double_click_sends_the_items_to_the_cursor() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), t("a", 7), AIR], holding("a", 3), (30.0, 40.0));
    // Both slots are emptied into the cursor stack.
    s.frame(&rowd(&[AIR, AIR, AIR]), holding("a", 15), (30.0, 40.0), 1, false);
    let swaps = s.swaps_to(CURSOR);
    assert_eq!(swaps.len(), 2);
    assert_eq!(swaps[0].amount(), 5);
    assert_eq!(swaps[1].amount(), 7);
    // From the slot centres (8, 8) and (26, 8) to the mouse at (30, 40).
    assert!(close(swaps[0].distance(), (22.0f64).hypot(32.0)));
    assert!(close(swaps[1].distance(), (4.0f64).hypot(32.0)));
}

#[test]
fn the_very_first_change_after_start_is_not_animated_to_the_cursor() {
    // The original keeps the cursor of the last change; before any change it has none, so a first
    // change that is a collection into the cursor does not fly.
    let mut s = smooth();
    s.frame(&rowd(&[t("a", 5), AIR]), holding("a", 3), (0.0, 0.0), 1, false);
    // One slot emptied into the cursor: the cursor it is compared with is not known yet.
    s.frame(&rowd(&[AIR, AIR]), holding("a", 8), (0.0, 0.0), 1, false);
    assert!(!s.busy());
    // The next one is.
    s.frame(&rowd(&[AIR, t("a", 4)]), holding("a", 8), (0.0, 0.0), 1, false);
    s.frame(&rowd(&[AIR, AIR]), holding("a", 12), (0.0, 0.0), 1, false);
    assert_eq!(s.swaps_to(CURSOR).len(), 1);
}

#[test]
fn nothing_flies_to_an_empty_cursor() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), t("a", 7)], AIR, (0.0, 0.0));
    s.frame(&rowd(&[AIR, AIR]), AIR, (0.0, 0.0), 1, false);
    assert!(s.swaps_to(CURSOR).is_empty());
}

#[test]
fn taking_into_a_cursor_that_already_holds_the_item_flies_too() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), AIR], holding("a", 10), (0.0, 0.0));
    // One slot emptied, the cursor grew by exactly that.
    s.frame(&rowd(&[AIR, AIR]), holding("a", 15), (0.0, 0.0), 1, false);
    assert_eq!(s.swaps_to(CURSOR).len(), 1);
    assert_eq!(s.swaps_to(CURSOR)[0].amount(), 5);
}

#[test]
fn picking_up_into_an_empty_cursor_does_not_fly() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), AIR], AIR, (0.0, 0.0));
    s.frame(&rowd(&[AIR, AIR]), holding("a", 5), (0.0, 0.0), 1, false);
    assert!(!s.busy());
}

#[test]
fn a_slot_that_lost_something_else_than_the_cursor_gained_does_not_fly() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), AIR], holding("a", 10), (0.0, 0.0));
    // The slot lost 1 but the cursor grew by 2: not a transfer ...
    s.frame(&rowd(&[t("a", 4), AIR]), holding("a", 12), (0.0, 0.0), 1, false);
    assert!(s.swaps_to(CURSOR).is_empty());
    // ... unless the slot was emptied, which the original accepts whatever the cursor did.
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), AIR], holding("a", 10), (0.0, 0.0));
    s.frame(&rowd(&[AIR, AIR]), holding("a", 12), (0.0, 0.0), 1, false);
    assert_eq!(s.swaps_to(CURSOR).len(), 1);
}

#[test]
fn the_mover_to_the_cursor_follows_the_cursor_and_ends_on_it() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), AIR], holding("a", 10), (8.0, 40.0));
    s.frame(&rowd(&[AIR, AIR]), holding("a", 15), (8.0, 40.0), 1, false);
    // The first item drawn only lets it start; the cursor item draws it from then on.
    assert!(s.cursor_model(0.5).is_empty(), "the swap has not started yet");
    let first = s.cursor_model(0.0);
    assert_eq!(first.len(), 1);
    // At the start it is drawn at the source: the slot's centre (8, 8) against the mouse (8, 40),
    // 32 above the cursor item.
    assert!(close(first[0].dx, 0.0) && close(first[0].dy, -32.0), "{:?}", first[0]);
    let mut n = 0;
    loop {
        let out = s.cursor_model(0.5);
        n += 1;
        assert!(n < 100);
        if out.is_empty() {
            break;
        }
        assert!(out[0].dy >= -32.0 && out[0].dy <= 1.0);
    }
    assert!(!s.busy());
}

#[test]
fn drawing_a_slot_item_first_lets_the_swap_start() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), t("a", 2)], holding("a", 10), (8.0, 40.0));
    s.frame(&rowd(&[AIR, t("a", 2)]), holding("a", 15), (8.0, 40.0), 1, false);
    // A slot item is drawn before the cursor's: that is what flips `started`.
    assert!(s.slot_model(1, 0.5).is_none());
    assert_eq!(s.cursor_model(0.1).len(), 1, "the swap draws in the very frame");
}

#[test]
fn a_cursor_that_changes_ends_the_flight() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), AIR], holding("a", 10), (8.0, 40.0));
    s.frame(&rowd(&[AIR, AIR]), holding("a", 15), (8.0, 40.0), 1, false);
    assert!(s.cursor_model(0.0).is_empty());
    assert_eq!(s.cursor_model(0.0).len(), 1);
    // The player puts the stack down: the cursor is empty at the next frame.
    s.frame(&rowd(&[AIR, t("a", 15)]), AIR, (8.0, 40.0), 1, false);
    // A slot is drawn: nothing flies any more and the swap is dropped.
    s.slot_model(1, 0.1);
    assert!(s.swaps_to(CURSOR).is_empty());
}

#[test]
fn a_cursor_that_only_grows_keeps_the_flight() {
    let mut s = smooth();
    prime(&mut s, &[t("a", 5), t("a", 5)], holding("a", 10), (8.0, 40.0));
    s.frame(&rowd(&[AIR, t("a", 5)]), holding("a", 15), (8.0, 40.0), 1, false);
    assert!(s.cursor_model(0.0).is_empty());
    assert_eq!(s.cursor_model(0.1).len(), 1);
    // More arrives in the cursor stack (same item, higher count): same identity, still flying.
    s.frame(&rowd(&[AIR, AIR]), holding("a", 20), (8.0, 40.0), 1, false);
    assert!(!s.cursor_model(0.1).is_empty());
}

// ---- clicks ---------------------------------------------------------------------------------------

#[test]
fn a_click_on_the_slot_an_item_is_going_to_stops_it() {
    let mut s = smooth();
    open(&mut s, &[t("cobble", 32), AIR]);
    s.frame(&row(&[AIR, t("cobble", 32)], &[]), AIR, (0.0, 0.0), 1, false);
    assert!(!s.swaps_to(1).is_empty());
    s.clicked(Some(0), ClickKind::Other);
    assert!(!s.swaps_to(1).is_empty(), "another slot: it goes on");
    s.clicked(Some(1), ClickKind::Other);
    assert!(s.swaps_to(1).is_empty());
}

#[test]
fn a_hotbar_swap_sends_both_items() {
    let mut s = smooth();
    open(&mut s, &[t("a", 3), t("b", 4)]);
    s.clicked(Some(0), ClickKind::Swap);
    s.frame(&row(&[t("b", 4), t("a", 3)], &[]), AIR, (0.0, 0.0), 1, false);
    // Each item goes to the other's slot, with its amount.
    assert_eq!(s.swaps_to(0).len(), 1);
    assert_eq!(s.swaps_to(1).len(), 1);
    assert_eq!(s.swaps_to(0)[0].amount(), 4);
    assert_eq!(s.swaps_to(1)[0].amount(), 3);
    assert_eq!(s.swaps_to(0)[0].item(), &t("b", 4));
}

#[test]
fn a_swap_into_an_empty_slot_sends_one_item() {
    let mut s = smooth();
    open(&mut s, &[t("a", 3), AIR]);
    s.clicked(Some(0), ClickKind::Swap);
    // The 1.20.1 rule needs more than one slot changed: the source and the empty target.
    s.frame(&row(&[AIR, t("a", 3)], &[]), AIR, (0.0, 0.0), 1, false);
    assert_eq!(s.swaps_to(1).len(), 1);
    assert_eq!(s.swaps_to(0).len(), 0, "an emptied slot has nothing to draw");
}

#[test]
fn the_frame_after_a_swap_does_not_look_for_changes() {
    let mut s = smooth();
    open(&mut s, &[t("a", 3), t("b", 4), AIR]);
    s.clicked(Some(0), ClickKind::Swap);
    s.frame(&row(&[t("b", 4), t("a", 3), AIR], &[]), AIR, (0.0, 0.0), 1, false);
    let n = s.swaps_to(0).len() + s.swaps_to(1).len();
    // Same stacks next frame: no new swaps from a diff.
    s.frame(&row(&[t("b", 4), t("a", 3), AIR], &[]), AIR, (0.0, 0.0), 1, false);
    assert_eq!(s.swaps_to(0).len() + s.swaps_to(1).len(), n);
}

#[test]
fn a_shift_click_on_a_result_slot_sends_the_items_from_the_result() {
    let mut s = smooth();
    // Slot 0 is a result; the product goes to slots 1 and 2 (an empty one and a partial stack).
    let before = [t("plank", 4), AIR, t("plank", 10)];
    s.frame(&row(&before, &[0]), AIR, (0.0, 0.0), 1, false);
    s.clicked(Some(0), ClickKind::QuickMove);
    // The result is spent (a different product shows, here nothing), 4 planks arrive in slot 1.
    s.frame(
        &row(&[AIR, t("plank", 4), t("plank", 10)], &[0]),
        AIR,
        (0.0, 0.0),
        1,
        false,
    );
    // The result slot is the source; slot 1 got it.
    assert_eq!(s.swaps_to(1).len(), 1);
    assert_eq!(s.swaps_to(1)[0].amount(), 4);
    // Result 4 planks: the distance from slot 0 to slot 1 is 18.
    assert!(close(s.swaps_to(1)[0].distance(), 18.0));
}

#[test]
fn a_result_slot_never_receives_a_swap() {
    let mut s = smooth();
    s.frame(&row(&[AIR, t("a", 5)], &[0]), AIR, (0.0, 0.0), 1, false);
    s.frame(&row(&[t("a", 5), AIR], &[0]), AIR, (0.0, 0.0), 1, false);
    assert!(s.swaps_to(0).is_empty(), "an output slot is not a destination");
}

// ---- screens and settings -------------------------------------------------------------------------

#[test]
fn a_new_screen_starts_clean() {
    let mut s = smooth();
    open(&mut s, &[t("a", 5), AIR]);
    s.frame(&row(&[AIR, t("a", 5)], &[]), AIR, (0.0, 0.0), 1, false);
    assert!(s.busy());
    // Another screen (a different token): the swaps go and the new first frame is only a copy.
    s.frame(&row(&[t("b", 1), AIR], &[]), AIR, (0.0, 0.0), 2, false);
    assert!(!s.busy());
    s.frame(&row(&[t("b", 1), AIR], &[]), AIR, (0.0, 0.0), 2, false);
    assert!(!s.busy());
}

#[test]
fn the_creative_screen_is_left_alone() {
    let mut s = smooth();
    s.frame(&row(&[t("a", 5), AIR], &[]), AIR, (0.0, 0.0), 1, true);
    s.frame(&row(&[AIR, t("a", 5)], &[]), AIR, (0.0, 0.0), 1, true);
    assert!(!s.busy());
}

#[test]
fn switched_off_it_does_nothing() {
    let mut s = Smooth::new(Config {
        enabled: false,
        ..Config::default()
    });
    open(&mut s, &[t("a", 5), AIR]);
    s.frame(&row(&[AIR, t("a", 5)], &[]), AIR, (0.0, 0.0), 1, false);
    assert!(!s.busy());
    assert!(s.slot_model(1, 0.5).is_none());
    assert!(s.slot_overlay(1, &t("a", 5)).is_none());
    assert!(s.cursor_model(0.5).is_empty());
}

#[test]
fn a_slot_without_swaps_is_drawn_as_the_game_does() {
    let mut s = smooth();
    open(&mut s, &[t("a", 5), AIR]);
    assert!(s.slot_model(0, 0.5).is_none());
    assert!(s.slot_overlay(0, &t("a", 5)).is_none());
}

#[test]
fn a_swap_that_does_not_move_is_never_finished_by_itself() {
    // Two slots on the same spot: distance 0, the original's progress is NaN and it never arrives;
    // a click or a new screen is what ends it.
    let mut s = smooth();
    let at = |stacks: &[T]| -> Vec<SlotView<T>> {
        stacks
            .iter()
            .map(|st| SlotView {
                x: 0.0,
                y: 0.0,
                stack: st.clone(),
                output: false,
            })
            .collect()
    };
    s.frame(&at(&[t("a", 5), AIR]), AIR, (0.0, 0.0), 1, false);
    s.frame(&at(&[AIR, t("a", 5)]), AIR, (0.0, 0.0), 1, false);
    for _ in 0..50 {
        assert!(s.slot_model(1, 1.0).is_some());
    }
    s.clicked(Some(1), ClickKind::Other);
    assert!(!s.busy());
}

#[test]
fn the_speed_formatted_is_a_float_of_percent() {
    assert_eq!(Config::default().speed(), 1.0);
    let c = Config {
        animation_speed: 250,
        ..Config::default()
    };
    assert_eq!(c.speed(), 2.5);
}

#[test]
fn the_defaults_are_tuned_config() {
    let c = Config::default();
    assert!(c.enabled);
    assert_eq!(c.animation_speed, 100);
    assert!(c.curve_points.is_empty());
}

#[test]
fn removing_a_spent_source_while_walking_skips_the_next_one() {
    // The original takes a spent source out of the list it is walking by index, so the source after
    // it is passed over in that round. Slot 4 gains 5; sources 0 (3), 1 (2), 2 (4):
    // 0 gives 3 and is removed, 1 is skipped, 2 gives the last 2.
    let mut s = smooth();
    open(&mut s, &[t("a", 3), t("a", 2), t("a", 4), AIR, AIR]);
    s.frame(
        &row(&[AIR, AIR, t("a", 2), AIR, t("a", 5)], &[]),
        AIR,
        (0.0, 0.0),
        1,
        false,
    );
    let to4 = s.swaps_to(4);
    assert_eq!(to4.len(), 2, "{to4:?}");
    assert_eq!(to4[0].amount(), 3);
    assert_eq!(to4[1].amount(), 2);
    // The first came from slot 0 (4 slots away), the second from slot 2 (2 slots away).
    assert!(close(to4[0].distance(), 72.0));
    assert!(close(to4[1].distance(), 36.0));
}

#[test]
fn slots_gaining_are_served_in_the_order_the_map_walks_them() {
    // Changed slots 20 and 5: bucket 4 comes before bucket 5, so 20 is served first. Source 3
    // gives to slot 20 first (all 4 of its items), then source 4 to slot 5.
    let mut s = smooth();
    let mut before = vec![AIR; 24];
    before[3] = t("a", 4);
    before[4] = t("a", 6);
    let mut after = before.clone();
    after[3] = AIR;
    after[4] = AIR;
    after[20] = t("a", 4);
    after[5] = t("a", 6);
    s.frame(&row(&before, &[]), AIR, (0.0, 0.0), 1, false);
    s.frame(&row(&after, &[]), AIR, (0.0, 0.0), 1, false);
    assert_eq!(s.swaps_to(20).len(), 1);
    assert_eq!(s.swaps_to(5).len(), 1);
    // 20 got 4 from slot 3 ...
    assert_eq!(s.swaps_to(20)[0].amount(), 4);
    assert!(close(s.swaps_to(20)[0].distance(), 17.0 * 18.0));
    // ... and 5 got 6 from slot 4.
    assert_eq!(s.swaps_to(5)[0].amount(), 6);
    assert!(close(s.swaps_to(5)[0].distance(), 18.0));
}
