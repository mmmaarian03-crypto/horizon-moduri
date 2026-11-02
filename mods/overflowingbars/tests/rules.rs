use horizon_overflowingbars::*;

fn player() -> Player {
    Player {
        health: 20.0,
        max_health: 20.0,
        absorption: 0.0,
        display_health: 20,
        blink: false,
        kind: Kind::Normal,
        regeneration: false,
        hardcore: false,
        tick: 0,
        armor: 0,
        toughness: 0,
    }
}
fn no_random(_: i32) -> i32 {
    0
}
fn bar(p: &Player) -> Vec<Draw> {
    health_bar(&Config::tuned(), p, 100, 200, &mut no_random)
}
fn hearts(d: &[Draw], ty: HeartType) -> Vec<(i32, i32, bool)> {
    d.iter()
        .filter_map(|x| match x {
            Draw::Heart { ty: t, half, x, y, .. } if *t == ty => Some((*x, *y, *half)),
            _ => None,
        })
        .collect()
}
fn icons(d: &[Draw]) -> Vec<(i32, i32, i32, i32)> {
    d.iter()
        .filter_map(|x| match x {
            Draw::Icon { u, v, x, y } => Some((*u, *v, *x, *y)),
            _ => None,
        })
        .collect()
}
fn width(s: &str) -> i32 {
    s.len() as i32 * 6
}

#[test]
fn tuned_values_are_the_file() {
    let c = Config::tuned();
    assert!(c.health.allow_layers && c.armor.allow_layers && c.toughness.allow_layers);
    assert!(c.health.allow_count && c.armor.allow_count && c.toughness.allow_count);
    assert!(!c.health.colorize_first_row && !c.armor.colorize_first_row && !c.toughness.colorize_first_row);
    assert!(!c.health.inverse_coloring && !c.armor.inverse_coloring && !c.toughness.inverse_coloring);
    assert!(c.armor.skip_empty && c.toughness.skip_empty);
    assert!(c.toughness_bar && !c.toughness_left);
    assert_eq!(c.row_count, RowCount { color: 0xFF_FFFF, full_rows_only: false, force_font: false, always: false, x: true });
    assert!(c.move_chat && c.move_experience);
}

#[test]
fn the_settings_folder_overrides_tuned_values() {
    let get = |k: &str| match k {
        "toughness.left_side" => Some("true".to_string()),
        "armor.inverse_coloring" => Some("true".to_string()),
        "row_count.row_count_color" => Some("gold".to_string()),
        "general.move_chat_above_armor" => Some("false".to_string()),
        "health.allow_layers" => Some("nope".to_string()),
        _ => None,
    };
    let c = Config::read(get);
    assert!(c.toughness_left && c.armor.inverse_coloring && !c.move_chat);
    assert_eq!(c.row_count.color, 0xFF_AA00);
    assert!(c.health.allow_layers);
    assert_eq!(Config::read(|_| None), Config::tuned());
    assert_eq!(chat_color("dark_red"), Some(0xAA_0000));
    assert_eq!(chat_color("pink"), None);
}

#[test]
fn heart_types_follow_for_player() {
    use HeartType::*;
    assert_eq!(heart_type(Kind::Normal, false, false, false), Normal);
    assert_eq!(heart_type(Kind::Normal, true, false, false), Absorbing);
    assert_eq!(heart_type(Kind::Normal, false, true, false), Orange);
    assert_eq!(heart_type(Kind::Normal, true, true, false), Orange);
    // Inverse: the layers below are the game's, the first row coloured.
    assert_eq!(heart_type(Kind::Normal, false, false, true), Orange);
    assert_eq!(heart_type(Kind::Normal, false, true, true), Normal);
    assert_eq!(heart_type(Kind::Normal, true, true, true), Orange);
    // An effect wins over everything.
    assert_eq!(heart_type(Kind::Withered, true, true, false), Withered);
    assert_eq!(heart_type(Kind::Poisoned, false, true, false), Poisoned);
    assert_eq!(heart_type(Kind::Frozen, false, false, false), Frozen);
    assert!(Normal.can_blink() && Orange.can_blink() && !Absorbing.can_blink() && !Frozen.can_blink());
}

#[test]
fn orange_hearts_are_in_the_sheet_rows_3_and_4() {
    assert_eq!(orange_uv(false, false, false), (0, 27));
    assert_eq!(orange_uv(true, false, false), (9, 27));
    assert_eq!(orange_uv(false, true, false), (18, 27));
    assert_eq!(orange_uv(true, true, false), (27, 27));
    assert_eq!(orange_uv(false, false, true), (0, 36));
}

#[test]
fn twenty_health_is_ten_full_hearts() {
    let d = bar(&player());
    assert_eq!(hearts(&d, HeartType::Container).len(), 10);
    let full = hearts(&d, HeartType::Normal);
    assert_eq!(full.len(), 10);
    assert_eq!(full[0], (100, 200, false));
    assert_eq!(full[9], (100 + 9 * 8, 200, false));
    assert!(icons(&d).is_empty());
}

#[test]
fn an_odd_health_ends_with_a_half_heart() {
    let mut p = player();
    p.health = 7.0;
    p.display_health = 7;
    let d = bar(&p);
    let full = hearts(&d, HeartType::Normal);
    assert_eq!(full.len(), 4);
    assert_eq!(full[3], (100 + 3 * 8, 200, true));
    // Every container is drawn (10), the health is under them.
    assert_eq!(hearts(&d, HeartType::Container).len(), 10);
}

#[test]
fn health_above_20_turns_the_front_hearts_orange_on_the_same_row() {
    let mut p = player();
    p.health = 30.0;
    p.max_health = 30.0;
    p.display_health = 30;
    let d = bar(&p);
    // One row of 10: the first 5 hearts orange (the second layer, 21..30), the rest the game's red.
    assert_eq!(hearts(&d, HeartType::Container).len(), 10);
    let orange = icons(&d);
    assert_eq!(orange.len(), 5);
    assert_eq!(orange[0], (0, 27, 100, 200));
    assert_eq!(orange[4], (0, 27, 100 + 4 * 8, 200));
    let red = hearts(&d, HeartType::Normal);
    assert_eq!(red.len(), 5);
    assert_eq!(red[0].0, 100 + 5 * 8);
}

#[test]
fn an_odd_second_layer_keeps_the_red_heart_under_the_orange_half() {
    let mut p = player();
    p.health = 25.0;
    p.max_health = 25.0;
    p.display_health = 25;
    let d = bar(&p);
    // Hearts 0 and 1 full orange, heart 2 half orange over a full red heart, 3..9 red.
    let orange: Vec<_> = icons(&d);
    assert_eq!(orange.len(), 3);
    assert_eq!(orange[2], (9, 27, 100 + 2 * 8, 200));
    let red = hearts(&d, HeartType::Normal);
    assert_eq!(red.len(), 8);
    assert!(red.iter().any(|h| h.0 == 100 + 2 * 8 && !h.2));
}

#[test]
fn absorption_fills_the_second_row() {
    let mut p = player();
    p.absorption = 6.0;
    let d = bar(&p);
    let abs = hearts(&d, HeartType::Absorbing);
    assert_eq!(abs.len(), 3);
    // The row above, from the left.
    assert_eq!(abs[0], (100, 190, false));
    assert_eq!(abs[2], (100 + 2 * 8, 190, false));
    // Containers: 10 + 3.
    assert_eq!(hearts(&d, HeartType::Container).len(), 13);
}

#[test]
fn absorption_beyond_the_second_row_turns_orange() {
    let mut p = player();
    p.absorption = 24.0;
    let d = bar(&p);
    // 10 hearts of absorption: 20 points; the 4 over make 2 orange hearts at the start of the second row.
    let orange = icons(&d);
    assert_eq!(orange.len(), 2);
    assert_eq!(orange[0], (0, 27, 100, 190));
    assert_eq!(hearts(&d, HeartType::Absorbing).len(), 8);
}

#[test]
fn few_hearts_shake_with_the_random() {
    let mut p = player();
    p.health = 4.0;
    p.display_health = 4;
    let mut calls = 0;
    let d = health_bar(&Config::tuned(), &p, 100, 200, &mut |b| {
        calls += 1;
        assert_eq!(b, 2);
        1
    });
    assert_eq!(calls, 10);
    assert_eq!(hearts(&d, HeartType::Container)[0], (100, 201, false));
}

#[test]
fn regeneration_lifts_one_heart() {
    let mut p = player();
    p.regeneration = true;
    p.tick = 3;
    let d = bar(&p);
    // `tick % ceil(min(20, max) + 5)`: heart 3 is up 2.
    assert_eq!(hearts(&d, HeartType::Container)[3].1, 198);
    assert_eq!(hearts(&d, HeartType::Container)[2].1, 200);
    // The cycle is 25 long: tick 28 lifts heart 3 again, tick 24 lifts none of the hearts.
    p.tick = 28;
    assert_eq!(hearts(&bar(&p), HeartType::Container)[3].1, 198);
    p.tick = 24;
    assert!(hearts(&bar(&p), HeartType::Container).iter().all(|h| h.1 == 200));
}

#[test]
fn the_blink_layer_is_drawn_under_the_current_one() {
    let mut p = player();
    p.blink = true;
    p.display_health = 20;
    p.health = 10.0;
    let d = bar(&p);
    let blinking: Vec<_> = d.iter().filter(|x| matches!(x, Draw::Heart { blink: true, ty: HeartType::Normal, .. })).collect();
    // 10 hearts of the old health, blinking, and the 5 of the current one over them.
    assert_eq!(blinking.len(), 10);
    // The containers blink too.
    assert!(d.iter().any(|x| matches!(x, Draw::Heart { ty: HeartType::Container, blink: true, .. })));
}

#[test]
fn hardcore_and_effects_reach_the_sprites() {
    let mut p = player();
    p.hardcore = true;
    p.kind = Kind::Withered;
    let d = bar(&p);
    assert!(d.iter().all(|x| matches!(x, Draw::Heart { hardcore: true, .. })));
    assert_eq!(hearts(&d, HeartType::Withered).len(), 10);
    // A frozen heart never blinks.
    p.kind = Kind::Frozen;
    p.blink = true;
    p.display_health = 20;
    assert!(bar(&p)
        .iter()
        .all(|x| !matches!(x, Draw::Heart { ty: HeartType::Frozen, blink: true, .. })));
}

#[test]
fn colorize_first_row_and_inverse_change_the_front_hearts() {
    let mut c = Config::tuned();
    c.health.colorize_first_row = true;
    let mut p = player();
    p.health = 10.0;
    p.display_health = 10;
    let d = health_bar(&c, &p, 100, 200, &mut no_random);
    // The first row coloured: 5 orange hearts instead of red.
    assert_eq!(icons(&d).len(), 5);
    assert_eq!(hearts(&d, HeartType::Normal).len(), 0);
    let mut c = Config::tuned();
    c.health.inverse_coloring = true;
    let d = health_bar(&c, &player(), 100, 200, &mut no_random);
    assert_eq!(icons(&d).len(), 10);
}

// ---- armour and toughness

#[test]
fn an_armor_bar_of_15_points() {
    let c = Config::tuned();
    let d = armor_bar(&c.armor, 100, 160, 18, 15, true, false);
    let i = icons(&d);
    // 7 full points, the half, no empty ones (skip).
    assert_eq!(i.len(), 8);
    assert!(i[..7].iter().all(|t| t.0 == 18 && t.1 == 18));
    assert_eq!(i[7], (9, 18, 100 + 7 * 8, 160));
}

#[test]
fn empty_armor_points_are_drawn_when_not_skipped() {
    let mut c = Config::tuned();
    c.armor.skip_empty = false;
    let d = armor_bar(&c.armor, 100, 160, 18, 15, true, false);
    let i = icons(&d);
    assert_eq!(i.len(), 10);
    assert_eq!(i[8].0, 0);
    assert_eq!(i[9].0, 0);
}

#[test]
fn armor_above_20_colours_the_front_row() {
    let c = Config::tuned();
    let d = armor_bar(&c.armor, 100, 160, 18, 25, true, false);
    let u: Vec<i32> = icons(&d).iter().map(|t| t.0).collect();
    // lastRow = (25 - 1) % 20 + 1 = 5: two coloured points, the coloured 'full over the layer', then the plain rest.
    assert_eq!(u, [36, 36, 27, 18, 18, 18, 18, 18, 18, 18]);
}

#[test]
fn colorize_first_row_colours_a_single_layer_too() {
    let mut c = Config::tuned();
    c.armor.colorize_first_row = true;
    let d = armor_bar(&c.armor, 100, 160, 18, 7, true, false);
    let u: Vec<i32> = icons(&d).iter().map(|t| t.0).collect();
    assert_eq!(u, [36, 36, 36, 45]);
}

#[test]
fn inverse_swaps_the_two_sets_of_icons() {
    let mut c = Config::tuned();
    c.armor.inverse_coloring = true;
    let d = armor_bar(&c.armor, 100, 160, 18, 15, true, false);
    let u: Vec<i32> = icons(&d).iter().map(|t| t.0).collect();
    assert_eq!(u, [36, 36, 36, 36, 36, 36, 36, 45]);
}

#[test]
fn an_unmodified_bar_is_the_games_own() {
    let mut c = Config::tuned();
    c.toughness.inverse_coloring = true;
    c.toughness.skip_empty = true;
    // `unmodified`: no inverse, no skipping, no layers.
    let d = armor_bar(&c.toughness, 300, 160, 0, 25, false, true);
    let u: Vec<i32> = icons(&d).iter().map(|t| t.0).collect();
    assert_eq!(u, [18, 18, 18, 18, 18, 18, 18, 18, 18, 18]);
    // 3 points: one full, one half... and the empty ones.
    let d = armor_bar(&c.toughness, 300, 160, 0, 3, false, true);
    let u: Vec<i32> = icons(&d).iter().map(|t| t.0).collect();
    assert_eq!(u, [18, 9, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn the_toughness_bar_grows_to_the_left_from_the_right_edge() {
    let c = Config::tuned();
    let d = armor_bar(&c.toughness, 300, 160, 0, 6, false, false);
    let i = icons(&d);
    assert_eq!(i.len(), 3);
    assert_eq!(i[0], (18, 0, 300 - 9, 160));
    assert_eq!(i[1], (18, 0, 300 - 8 - 9, 160));
    assert_eq!(i[2], (18, 0, 300 - 16 - 9, 160));
}

// ---- the row counts

fn digits(d: &[Draw]) -> Vec<(i32, i32, i32, i32, u32)> {
    d.iter()
        .filter_map(|x| match x {
            Draw::Digit { u, v, x, y, color } => Some((*u, *v, *x, *y, *color)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_row_count_shows_only_over_one_row() {
    let c = RowCount { color: 0xFF_FFFF, full_rows_only: false, force_font: false, always: false, x: true };
    assert!(row_count(&c, 98, 160, 20, true, 20, &width).is_empty());
    assert!(row_count(&c, 98, 160, 0, true, 20, &width).is_empty());
    assert!(!row_count(&c, 98, 160, 21, true, 20, &width).is_empty());
}

#[test]
fn the_count_is_the_digit_and_an_x_with_a_black_border() {
    let c = RowCount { color: 0xFF_FFFF, full_rows_only: false, force_font: false, always: false, x: true };
    let d = row_count(&c, 98, 160, 25, true, 20, &width);
    // 25 / 20 = 1.25 rows, 2 shown: the digit 2 and the x, 5 draws each (4 black, 1 coloured).
    let g = digits(&d);
    assert_eq!(g.len(), 10);
    // Left: x - 7 = 91, y + 2 = 162. The digit 2 is at texel 10, the x at (0, 7).
    assert_eq!(g[4], (10, 0, 91, 162, 0xFFFF_FFFF));
    assert_eq!(g[0], (10, 0, 90, 162, 0xFF00_0000));
    assert_eq!(g[9], (0, 7, 95, 162, 0xFFFF_FFFF));
}

#[test]
fn full_rows_only_floors_and_two_digits_go_left() {
    let c = RowCount { color: 0x55_FF55, full_rows_only: true, force_font: false, always: false, x: false };
    let d = row_count(&c, 98, 160, 39, true, 20, &width);
    // 1.95 rows: 1 (floor).
    assert_eq!(digits(&d)[4].0, 5);
    // 220 points over 20 = 11 rows: digits 1 and 1, each 4 apart, the last digit at the anchor.
    let d = row_count(&c, 98, 160, 220, true, 20, &width);
    let g = digits(&d);
    assert_eq!(g.len(), 10);
    // x: left without 'x' = 98 - 3 = 95; digits 4 apart going left.
    assert_eq!(g[4].2, 95);
    assert_eq!(g[9].2, 91);
    assert_eq!(g[4].4, 0xFF55_FF55);
}

#[test]
fn on_the_right_the_count_starts_at_the_edge() {
    let c = RowCount { color: 0xFF_FFFF, full_rows_only: false, force_font: false, always: false, x: true };
    let d = row_count(&c, 302, 160, 30, false, 20, &width);
    let g = digits(&d);
    // One digit: x + 4 * 1.
    assert_eq!(g[4].2, 306);
    assert_eq!(g[9].2, 310);
}

#[test]
fn always_shows_even_one_row_and_force_font_uses_the_text() {
    let c = RowCount { color: 0xFF_FFFF, full_rows_only: false, force_font: false, always: true, x: true };
    assert!(!row_count(&c, 98, 160, 10, true, 20, &width).is_empty());
    let f = RowCount { force_font: true, ..c };
    let d = row_count(&f, 98, 160, 45, true, 20, &width);
    // "3x" 12 wide: left of 98, one below the row.
    assert_eq!(d.len(), 5);
    assert_eq!(d[4], Draw::Text { text: "3x".into(), x: 98 - 12, y: 161, color: 0xFFFF_FFFF });
    assert_eq!(d[0], Draw::Text { text: "3x".into(), x: 98 - 12 - 1, y: 161, color: 0xFF00_0000 });
}

// ---- the whole bars

#[test]
fn the_bars_stack_health_armor_and_toughness() {
    let c = Config::tuned();
    let mut p = player();
    p.armor = 10;
    p.toughness = 4;
    let b = bars(&c, &p, (480, 270), 59, 49, &width, &mut no_random);
    // Health from the bottom (270 - 39), armor a row above, the toughness bar at the right side's height.
    assert_eq!(hearts(&b.draws, HeartType::Normal)[0].1, 270 - 39);
    let i = icons(&b.draws);
    assert_eq!(i[0].3, 270 - 49);
    assert_eq!(i[0].2, 480 / 2 - 91);
    let tough: Vec<_> = i.iter().filter(|t| t.1 == 0).collect();
    assert_eq!(tough.len(), 2);
    assert_eq!(tough[0].3, 270 - 59);
    assert_eq!(tough[0].2, 480 / 2 + 91 - 9);
    assert_eq!((b.left_height, b.right_height), (39 + 10 + 10, 59 + 10));
}

#[test]
fn two_health_rows_raise_the_armor() {
    let c = Config::tuned();
    let mut p = player();
    p.absorption = 4.0;
    p.armor = 10;
    let b = bars(&c, &p, (480, 270), 49, 49, &width, &mut no_random);
    assert_eq!(icons(&b.draws)[0].3, 270 - 59);
    assert_eq!(b.left_height, 39 + 20 + 10);
    assert_eq!(b.right_height, 49);
}

#[test]
fn counts_are_drawn_next_to_the_bars() {
    let c = Config::tuned();
    let mut p = player();
    p.health = 40.0;
    p.max_health = 40.0;
    p.display_health = 40;
    p.armor = 30;
    let b = bars(&c, &p, (480, 270), 49, 49, &width, &mut no_random);
    let g = digits(&b.draws);
    // Health 40 = 2 rows and armor 30 = 2 rows: two counts of (digit + x) with 5 draws each.
    assert_eq!(g.len(), 20);
    // The health's at the health row, the armor's one row up.
    assert_eq!(g[0].3, 270 - 39 + 2);
    assert_eq!(g[10].3, 270 - 49 + 2);
}

#[test]
fn layers_off_leave_the_games_bars() {
    let mut c = Config::tuned();
    c.health.allow_layers = false;
    let mut p = player();
    p.armor = 4;
    let b = bars(&c, &p, (480, 270), 49, 49, &width, &mut no_random);
    assert!(hearts(&b.draws, HeartType::Normal).is_empty());
    // The game draws the health (it leaves 49): the armor goes above it.
    assert_eq!(icons(&b.draws)[0].3, 270 - 49);
    assert_eq!(b.left_height, 49 + 10);
    // With the armor's layers off too, the game's armor row is only counted.
    let mut c2 = c;
    c2.armor.allow_layers = false;
    let b = bars(&c2, &p, (480, 270), 49, 49, &width, &mut no_random);
    assert!(icons(&b.draws).is_empty());
    assert_eq!(b.left_height, 49 + 10);
    let mut c = Config::tuned();
    c.toughness_bar = false;
    let mut p = player();
    p.toughness = 8;
    let b = bars(&c, &p, (480, 270), 49, 49, &width, &mut no_random);
    assert!(icons(&b.draws).is_empty());
    assert_eq!(b.right_height, 49);
}

#[test]
fn the_toughness_bar_on_the_left_stacks_above_the_armor() {
    let mut c = Config::tuned();
    c.toughness_left = true;
    let mut p = player();
    p.armor = 10;
    p.toughness = 4;
    let b = bars(&c, &p, (480, 270), 49, 49, &width, &mut no_random);
    let tough: Vec<_> = icons(&b.draws).into_iter().filter(|t| t.1 == 9).collect();
    assert_eq!(tough[0].3, 270 - 59);
    assert_eq!(tough[0].2, 480 / 2 - 91);
    assert_eq!(b.left_height, 39 + 10 + 10 + 10);
    assert_eq!(b.right_height, 49);
}

#[test]
fn no_bars_when_the_player_has_nothing_extra() {
    let c = Config::tuned();
    let b = bars(&c, &player(), (480, 270), 49, 49, &width, &mut no_random);
    assert!(icons(&b.draws).is_empty());
    assert_eq!((b.left_height, b.right_height), (49, 49));
}

// ---- the chat and the experience level

#[test]
fn the_chat_moves_up_for_each_extra_row() {
    let c = Config::tuned();
    assert_eq!(chat_offset(&c, 20.0, 0.0, 0, 0), 0);
    assert_eq!(chat_offset(&c, 20.0, 0.0, 5, 0), 10);
    assert_eq!(chat_offset(&c, 20.0, 4.0, 5, 0), 20);
    // Absorption that fits the first row is no row.
    assert_eq!(chat_offset(&c, 14.0, 6.0, 0, 0), 0);
    assert_eq!(chat_offset(&c, 14.0, 8.0, 0, 0), 10);
    // The toughness on the right counts only when nothing else raises the chat.
    assert_eq!(chat_offset(&c, 20.0, 0.0, 0, 4), 10);
    assert_eq!(chat_offset(&c, 20.0, 0.0, 5, 4), 10);
    let mut left = c;
    left.toughness_left = true;
    assert_eq!(chat_offset(&left, 20.0, 0.0, 5, 4), 20);
    let mut off = c;
    off.toughness_bar = false;
    assert_eq!(chat_offset(&off, 20.0, 0.0, 0, 4), 0);
}

#[test]
fn the_experience_level_moves_up_3_pixels() {
    assert_eq!(EXPERIENCE_LEVEL_SHIFT, 3);
}
