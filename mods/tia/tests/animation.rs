use horizon_tia::{Settings, Tia, eased_scale};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn the_defaults_are_tuned_config() {
    let s = Settings::default();
    assert_eq!(s.animation_speed, 0.5);
    assert_eq!(s.pickup_scale, 1.4);
}

#[test]
fn the_scale_runs_from_normal_to_the_pickup_scale() {
    assert_eq!(eased_scale(1.4, 0.0), 1.0);
    assert!(close(eased_scale(1.4, 1.0), 1.4));
    // 1 + 0.4 * (1 - 0.5^5)
    assert!(close(eased_scale(1.4, 0.5), 1.3875));
    // 1 + 0.4 * (1 - 0.9^5)
    assert!(close(eased_scale(1.4, 0.1), 1.0 + 0.4 * (1.0 - 0.59049)));
    // A scale of 1 does nothing at any progress.
    for p in [0.0, 0.3, 1.0] {
        assert_eq!(eased_scale(1.0, p), 1.0);
    }
    // Smaller than 1 shrinks instead.
    assert!(close(eased_scale(0.5, 1.0), 0.5));
}

#[test]
fn the_scale_grows_with_the_progress_and_most_of_it_comes_early() {
    let mut last = 0.0;
    for i in 0..=100 {
        let s = eased_scale(1.4, i as f32 / 100.0);
        assert!(s >= last);
        last = s;
    }
    // Half the progress already gives 97% of the growth.
    let share = (eased_scale(1.4, 0.5) - 1.0) / 0.4;
    assert!((0.96..0.98).contains(&share), "{share}");
}

#[test]
fn a_click_on_a_slot_starts_its_animation_and_restarts_the_carried_one() {
    let mut t = Tia::default();
    t.click(Some(7), 0, true, 1);
    for _ in 0..4 {
        t.draw_carried(1.0);
    }
    assert!(t.carried_progress() > 0.9);
    t.click(Some(12), 1, true, 2);
    assert_eq!(t.slot_progress(12), 1.0);
    assert_eq!(t.carried_progress(), 0.0);
    // The first slot kept the progress it had.
    assert_eq!(t.slot_progress(7), 1.0);
}

#[test]
fn only_plain_clicks_on_a_slot_count() {
    let mut t = Tia::default();
    t.click(Some(3), 0, false, 1); // quick move, swap, throw ...: not PICKUP
    t.click(Some(3), 2, true, 1); // middle button
    t.click(Some(3), 40, true, 1); // a hotbar swap's button
    t.click(None, 0, true, 1); // outside the window
    assert_eq!(t.slot_progress(3), 0.0);
    assert_eq!(t.carried_progress(), 0.0);
    t.draw_carried(1.0);
    t.click(Some(3), 0, false, 1);
    assert!(t.carried_progress() > 0.0, "a quick move leaves the carried item alone");
}

#[test]
fn a_slot_is_drawn_at_the_old_progress_and_then_steps() {
    let mut t = Tia::default();
    t.click(Some(5), 0, true, 10);
    // The first frame draws the full size, then takes partial * speed off.
    assert!(close(t.draw_slot(5, 0.5, 10), 1.4));
    assert!(close(t.slot_progress(5), 0.75));
    assert!(close(t.draw_slot(5, 0.5, 11), eased_scale(1.4, 0.75)));
    assert!(close(t.slot_progress(5), 0.5));
    assert!(close(t.draw_slot(5, 0.5, 12), eased_scale(1.4, 0.5)));
    assert!(close(t.draw_slot(5, 0.5, 13), eased_scale(1.4, 0.25)));
    // Spent: normal size from here on, and nothing is kept.
    assert_eq!(t.slot_progress(5), 0.0);
    assert_eq!(t.draw_slot(5, 0.5, 14), 1.0);
}

#[test]
fn the_step_is_the_partial_tick_not_the_time() {
    // A frame that falls exactly on a tick (partial 0) plays nothing; one late in the tick plays
    // most. This is the original's own behaviour: it uses `getPartialTick`, not the frame time.
    let mut t = Tia::default();
    t.click(Some(1), 0, true, 1);
    t.draw_slot(1, 0.0, 1);
    assert_eq!(t.slot_progress(1), 1.0);
    t.draw_slot(1, 0.9, 2);
    assert!(close(t.slot_progress(1), 1.0 - 0.45));
    t.draw_slot(1, 1.0, 3);
    assert!(close(t.slot_progress(1), 1.0 - 0.45 - 0.5));
}

#[test]
fn a_faster_speed_finishes_in_fewer_frames() {
    let frames = |speed: f32| {
        let mut t = Tia::new(Settings {
            animation_speed: speed,
            ..Settings::default()
        });
        t.click(Some(0), 0, true, 1);
        let mut n = 0;
        while t.slot_progress(0) > 0.0 {
            t.draw_slot(0, 0.5, 1 + n);
            n += 1;
        }
        n
    };
    assert_eq!(frames(0.5), 4);
    assert_eq!(frames(1.0), 2);
    assert_eq!(frames(0.25), 8);
    assert_eq!(frames(10.0), 1);
}

#[test]
fn drawing_other_slots_does_not_touch_an_animated_one() {
    let mut t = Tia::default();
    t.click(Some(9), 0, true, 1);
    for other in 0..9 {
        assert_eq!(t.draw_slot(other, 0.7, 1), 1.0);
    }
    assert_eq!(t.slot_progress(9), 1.0);
}

#[test]
fn a_slot_that_draws_nothing_still_runs_down_a_frame_late() {
    // The slot was emptied by the click, so no item is drawn there; the original steps it after
    // drawing it all the same. Here the next frame's start does it, with the earlier frame's
    // partial tick.
    let mut t = Tia::default();
    t.begin_frame(9, 0.3);
    t.click(Some(4), 0, true, 10);
    t.begin_frame(10, 0.4); // the click is for this frame: no step yet
    assert_eq!(t.slot_progress(4), 1.0);
    t.begin_frame(11, 0.6); // frame 10 drew nothing there: its step, with its 0.4
    assert!(close(t.slot_progress(4), 1.0 - 0.4 * 0.5));
    t.begin_frame(12, 0.2);
    assert!(close(t.slot_progress(4), 1.0 - 0.4 * 0.5 - 0.6 * 0.5));
    // And it goes away at zero.
    for f in 13..20 {
        t.begin_frame(f, 1.0);
    }
    assert_eq!(t.slot_progress(4), 0.0);
}

#[test]
fn a_slot_that_drew_is_not_stepped_twice() {
    let mut t = Tia::default();
    t.begin_frame(10, 0.4);
    t.click(Some(4), 0, true, 11);
    t.begin_frame(11, 0.5);
    t.draw_slot(4, 0.5, 11);
    assert!(close(t.slot_progress(4), 0.75));
    t.begin_frame(12, 0.5); // drawn in 11: nothing more for 11
    assert!(close(t.slot_progress(4), 0.75));
    t.draw_slot(4, 0.5, 12);
    assert!(close(t.slot_progress(4), 0.5));
}

#[test]
fn the_carried_item_grows_when_drawn_and_stops_at_the_full_size() {
    let mut t = Tia::default();
    t.click(Some(2), 0, true, 1);
    // Step first, then the size: the first frame is already a bit bigger than normal.
    let first = t.draw_carried(0.5);
    assert!(close(t.carried_progress(), 0.25));
    assert!(close(first, eased_scale(1.4, 0.25)));
    assert!(first > 1.0 && first < 1.4);
    let mut last = first;
    for _ in 0..10 {
        let s = t.draw_carried(0.5);
        assert!(s >= last);
        last = s;
    }
    assert_eq!(t.carried_progress(), 1.0);
    assert!(close(last, 1.4));
}

#[test]
fn the_carried_item_waits_while_nothing_is_carried() {
    // Nothing draws the floating item when nothing is carried, so its progress stays where the
    // last click left it.
    let mut t = Tia::default();
    t.click(Some(2), 0, true, 1);
    assert_eq!(t.carried_progress(), 0.0);
    t.draw_slot(2, 0.5, 1);
    t.draw_slot(2, 0.5, 2);
    assert_eq!(t.carried_progress(), 0.0);
}

#[test]
fn closing_the_screen_forgets_the_slots() {
    let mut t = Tia::default();
    t.click(Some(2), 0, true, 1);
    t.closed();
    assert_eq!(t.slot_progress(2), 0.0);
    assert_eq!(t.draw_slot(2, 0.5, 2), 1.0);
}

#[test]
fn the_pickup_scale_option_sets_the_peak() {
    let mut t = Tia::new(Settings {
        pickup_scale: 2.0,
        ..Settings::default()
    });
    t.click(Some(0), 0, true, 1);
    assert!(close(t.draw_slot(0, 0.0, 1), 2.0));
    for _ in 0..30 {
        t.draw_carried(1.0);
    }
    assert!(close(t.draw_carried(1.0), 2.0));
}
