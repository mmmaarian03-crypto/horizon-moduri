use horizon_betteradvancements::*;

fn tuned() -> Config {
    Config::read(|key| match key {
        "defaultUncompletedIconColor" => Some("\"#FFFFFF\"".into()),
        "defaultUncompletedTitleColor" => Some("\"#0489C1\"".into()),
        "defaultCompletedIconColor" => Some("\"#DBA213\"".into()),
        "defaultCompletedTitleColor" => Some("\"#DBA213\"".into()),
        "uiScaling" => Some("100".into()),
        "criteriaDetail" => Some("\"Default\"".into()),
        "criteriaRotationSpeed" => Some("3".into()),
        _ => None,
    })
}

#[test]
fn tuned_file_is_the_default_and_keeps_vanilla_looks() {
    let c = tuned();
    assert_eq!(c, Config::default());
    let look = Look::from_config(&c);
    assert!(!look.has_custom_icon_color());
    assert!(!look.has_custom_title_color());
    assert_eq!(look.icon_row(true), 0);
    assert_eq!(look.icon_row(false), 1);
    assert_eq!(look.icon_color(true), WHITE);
}

#[test]
fn bad_config_values_fall_back() {
    let c = Config::read(|k| match k {
        "uiScaling" => Some("500".into()),
        "criteriaRotationSpeed" => Some("0".into()),
        "criteriaDetail" => Some("Nonsense".into()),
        "defaultCompletedLineColor" => Some("zzz".into()),
        _ => None,
    });
    assert_eq!(c.ui_scaling, 100);
    assert_eq!(c.criteria_rotation_speed, 3);
    assert_eq!(c.completed_line_color, Config::default().completed_line_color);
    assert_eq!(c.detail, Config::default().detail);
}

#[test]
fn window_follows_the_screen() {
    let w = Window::new(427, 240, 100);
    assert_eq!((w.left, w.top), (SIDE, TOP));
    assert_eq!((w.right, w.bottom), (427 - SIDE, 240 - SIDE));
    assert_eq!(w.box_rect(), (SIDE + PADDING, TOP + 2 * PADDING, 427 - SIDE - PADDING, 240 - SIDE - PADDING));
    let small = Window::new(427, 240, 50);
    assert_eq!(small.internal_width, 213);
    assert!(small.left > w.left);
}

#[test]
fn tabs_page_when_they_do_not_fit() {
    let w = Window::new(427, 240, 100);
    let (width, height) = w.render_size();
    let per_page = max_tabs(width, height, false);
    assert_eq!(per_page, width / 32 + 2 * (height / 32));
    assert_eq!(max_tabs(width, height, true), width / 32);
    let mut p = Paging::default();
    assert!(!p.init(per_page, per_page));
    assert!(p.init(per_page + 1, per_page));
    assert_eq!(p.max_pages, 1);
    p.next();
    p.next();
    assert_eq!(p.page, 1);
    assert_eq!(p.range(per_page, (per_page + 1) as usize), per_page as usize..per_page as usize + 1);
    p.previous();
    p.previous();
    assert_eq!(p.page, 0);
    assert_eq!(tab_side(width, height, 0, false), Some(Side::Above));
    assert_eq!(tab_side(width, height, width / 32, false), Some(Side::Right));
    assert_eq!(tab_side(width, height, 5, true), Some(Side::Above));
    assert_eq!(p.label(10, &w).0, "1 / 2");
}

#[test]
fn tab_positions_and_hits() {
    let (width, height) = (300, 150);
    assert_eq!(tab_x(Side::Above, 2, width, height), (28 + 4) * 2);
    assert_eq!(tab_y(Side::Above, 0, width, height), -28);
    assert_eq!(tab_x(Side::Left, 0, width, height), -28);
    assert_eq!(tab_x(Side::Right, 0, width, height), width - 4);
    assert!(tab_over(Side::Above, 30, 40, width, height, 0, 40.0, 20.0));
    assert!(!tab_over(Side::Above, 30, 40, width, height, 0, 20.0, 20.0));
    assert_eq!(tab_column(Side::Above, 30, width, height, 0), 0);
    assert_eq!(tab_column(Side::Above, 30, width, height, 1), 1);
}

#[test]
fn scroll_centres_clamps_and_remembers() {
    let b = Bounds::of(&[(0, 0), (300, 100)]);
    assert_eq!((b.min_x, b.max_x, b.min_y, b.max_y), (0, 328, 0, 127));
    let mut s = Scroll::default();
    s.center(&b, 200, 200);
    assert_eq!((s.x, s.y), ((200 - 328) / 2, (200 - 127) / 2));
    // Only the axis longer than the view moves: x is longer, y is not.
    let y = s.y;
    s.by(&b, 1000.0, 1000.0, 200, 200);
    assert_eq!(s.x, 0);
    assert_eq!(s.y, y + 0);
    s.by(&b, -1000.0, 0.0, 200, 200);
    assert_eq!(s.x, -(328 - 200));
    let mut h = History::default();
    let w = Window::new(427, 240, 100);
    let mut tabs = Tabs::default();
    tabs.select("a", &b, &mut h, &w);
    tabs.select("b", &b, &mut h, &w);
    assert!(h.0.contains_key("a"));
    assert!(!tabs.drag_step());
    assert!(tabs.drag_step());
}

#[test]
fn widget_places_use_32_and_27() {
    assert_eq!(widget_pos(1.0, 1.0), (32, 27));
    assert_eq!(widget_pos(0.5, 2.5), (16, 67));
    assert!(widget_over((10, 10), (5, 5), 15, 15));
    assert!(widget_over((10, 10), (5, 5), 15 + 26, 15 + 26));
    assert!(!widget_over((10, 10), (5, 5), 15 + 27, 15));
}

#[test]
fn connections_draw_a_border_pass_and_a_colour_pass() {
    let look = Look::from_config(&Config::default());
    let inside = connection((64, 27), (32, 0), (0, 0), true, &look, false, 2);
    let colour = connection((64, 27), (32, 0), (0, 0), false, &look, false, 2);
    assert_eq!(inside.len(), 8);
    assert_eq!(colour.len(), 3);
    let mut direct = look;
    direct.direct_lines = true;
    assert_eq!(connection((64, 27), (32, 0), (0, 0), true, &direct, true, 2).len(), 1);
}

#[test]
fn hover_box_goes_left_when_it_would_leave_the_screen() {
    let base = HoverIn {
        pos: (300, 20),
        scroll: (0, 0),
        width: 150,
        desc_lines: 2,
        grid_height: 0,
        show_grid: false,
        percent: 0.5,
        window_left: 30,
        window_top: 40,
        internal_width: 427,
        screen_height: 240,
        progress_width: 12,
    };
    let right = plan_hover(&HoverIn { pos: (10, 20), ..base });
    assert!(!right.draw_left);
    assert_eq!(right.draw_x, 10);
    assert_eq!(right.title_x, 10 + 32);
    let left = plan_hover(&base);
    assert!(left.draw_left);
    assert_eq!(left.draw_x, 300 - 150 + ADVANCEMENT_SIZE + 6);
    // Half way: the left part of the title bar is obtained, the right one not.
    assert!(left.title_left_obtained && !left.title_right_obtained && !left.icon_obtained);
    assert_eq!(left.j, 75);
    let done = plan_hover(&HoverIn { percent: 1.0, ..base });
    assert!(done.icon_obtained && done.title_right_obtained);
    assert_eq!(done.j, 75);
    // Near the bottom of the screen the box opens upwards.
    let low = plan_hover(&HoverIn { pos: (10, 150), ..base });
    assert!(low.draw_top);
    assert_eq!(low.box_height, TITLE_SIZE + 2 * LINE_HEIGHT);
}

#[test]
fn percent_and_progress_text() {
    assert_eq!(percent(1, 4), 0.25);
    assert_eq!(percent(0, 0), 0.0);
    assert_eq!(progress_text(2, 5).as_deref(), Some("2/5"));
    assert_eq!(progress_text(1, 1), None);
}

#[test]
fn fade_steps() {
    assert!((fade_step(0.0, true, true) - 0.02).abs() < 1e-6);
    assert!((fade_step(0.29, true, true) - 0.3).abs() < 1e-6);
    assert_eq!(fade_step(0.02, false, true), 0.0);
    assert!((fade_step(0.5, true, false) - 0.46).abs() < 1e-6);
    assert_eq!(fade_alpha(0.3), 76);
}

#[test]
fn criteria_cells_follow_the_detail() {
    let list = vec![("a".to_string(), true), ("b".to_string(), false), ("c".to_string(), false)];
    let cells = build_cells(Detail::Default, false, false, &list).expect("cells");
    assert!(!cells.is_empty());
    assert!(build_cells(Detail::Default, false, true, &list).is_none());
    assert!(build_cells(Detail::Default, false, false, &list[..1]).is_none());
    let all = build_cells(Detail::All, true, false, &list).expect("cells");
    assert!(all.iter().any(|c| c.marker_color == DARK_RED && c.body_color == DARK_RED));
    assert!(all.iter().any(|c| c.marker_color == GREEN));
}

#[test]
fn the_criterion_grid_fits_the_box() {
    let widths = vec![40; 12];
    let g = Grid::find(&widths, 9, 200, 100);
    assert!(g.width <= 200);
    assert!(g.height <= 100);
    let placed: usize = g.paged.iter().flatten().map(|c| c.cells.len()).sum();
    assert_eq!(placed, 12);
    assert_eq!(Grid::empty().width, 0);
    let one = Grid::find(&[10], 9, 200, 100);
    assert_eq!(one.columns, 1);
}

#[test]
fn criterion_names_are_looked_up_in_order() {
    let (keys, plain) = translation_keys("minecraft:story/mine_stone", "minecraft:stone");
    assert_eq!(plain, "minecraft.stone");
    assert_eq!(keys[0], "minecraft.stone");
    assert_eq!(keys[1], "betteradvancements.criterion.minecraft:story/mine_stone.minecraft:stone");
    assert!(keys.contains(&"block.minecraft.stone".to_string()));
}

#[test]
fn java_hash_order_and_title_order() {
    let keys: Vec<String> = ["b", "a", "c"].iter().map(|s| s.to_string()).collect();
    assert_eq!(java_map_order(&keys).len(), 3);
    assert_eq!(java_hash("a"), 97);
    assert_eq!(compare_titles("apple", "Banana"), std::cmp::Ordering::Less);
    assert_eq!(rgb_from_str("#0489C1"), Some(0xFF0489C1));
}

#[test]
fn debug_coordinates_only_inside_the_box() {
    let w = Window::new(427, 240, 100);
    assert!(debug_coordinates(&w, (100, 100), (0, 0)).is_some());
    assert!(debug_coordinates(&w, (1, 1), (0, 0)).is_none());
}
