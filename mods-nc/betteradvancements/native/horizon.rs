// Better Advancements inside Horizon's game (nightly 142388e1c136). The rules (the window, the tabs and their paging,
// the scroll, the lines, the hover box, the criteria grid, the config) are the engine below; this part reads the
// advancement tree and the progress, draws the screen with the game's GUI sprites and answers the mouse. The
// screen replaces the game's own (draw_advancements and its mouse hooks are cancelled while the mod is on).
use super::{Game, KeyCode};
use horizon_client::{
    advancements::{Advancement, Frame, Tree},
    hud::{HudBatch, rgba},
    items::ItemStack,
    text::{Style, Text},
    ui,
};
use std::{cell::RefCell, sync::Arc};

#[allow(dead_code)]
mod engine {
    include!("betteradvancements_engine.rs");
}

pub const ID: &str = "betteradvancements";
pub const TITLE: &str = "Better Advancements 0.6.0.73";
pub const DESCRIPTION: &str = "A larger advancements window that follows the screen size, tabs that page, hover boxes with the list of criteria, and the original's colours and options.";

/// The layout of the loaded tree: widget pixel positions (`floor(x * 32)`, `floor(y * 27)`), and who is whose child.
struct Cache {
    tree: Arc<Tree>,
    server: bool,
    kids: Vec<Vec<usize>>,
    pos: Vec<Option<(i32, i32)>>,
}
struct State {
    config: Option<engine::Config>,
    cache: Option<Cache>,
    paging: engine::Paging,
    tabs: engine::Tabs,
    history: engine::History,
    drag: Option<(f32, f32)>,
    fade: f32,
    last_frame: Option<std::time::Instant>,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        config: None,
        cache: None,
        paging: engine::Paging::default(),
        tabs: engine::Tabs::default(),
        history: engine::History::default(),
        drag: None,
        fade: 0.0,
        last_frame: None,
    });
}

/// What one frame shows.
struct View {
    /// The tabs' roots (indexes in the tree), in the order they are listed.
    tabs: Vec<usize>,
    /// The selected root, if any.
    selected: Option<usize>,
    /// Its visible widgets in tree order, and each one's nearest displayed ancestor.
    widgets: Vec<usize>,
    parent: Vec<Option<usize>>,
    bounds: engine::Bounds,
}

fn rgba_of(argb: u32) -> u32 {
    rgba((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8)
}
fn rgb_of(rgb: u32) -> u32 {
    rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
}
fn frame_sprite(frame: Frame, obtained: bool) -> &'static str {
    match (frame, obtained) {
        (Frame::Task, true) => "gui/sprites/advancements/task_frame_obtained",
        (Frame::Task, false) => "gui/sprites/advancements/task_frame_unobtained",
        (Frame::Goal, true) => "gui/sprites/advancements/goal_frame_obtained",
        (Frame::Goal, false) => "gui/sprites/advancements/goal_frame_unobtained",
        (Frame::Challenge, true) => "gui/sprites/advancements/challenge_frame_obtained",
        (Frame::Challenge, false) => "gui/sprites/advancements/challenge_frame_unobtained",
    }
}
fn box_sprite(obtained: bool) -> &'static str {
    if obtained {
        "gui/sprites/advancements/box_obtained"
    } else {
        "gui/sprites/advancements/box_unobtained"
    }
}
fn tab_sprite(side: engine::Side, column: i32, selected: bool) -> String {
    let (first, middle, last) = match side {
        engine::Side::Above => ("above_left", "above_middle", "above_right"),
        engine::Side::Left => ("left_top", "left_middle", "left_bottom"),
        engine::Side::Right => ("right_top", "right_middle", "right_bottom"),
    };
    let part = match column {
        0 => first,
        1 => middle,
        _ => last,
    };
    format!("gui/sprites/advancements/tab_{part}{}", if selected { "_selected" } else { "" })
}

impl Game<'_> {
    fn betteradvancements_config(&self) -> engine::Config {
        STATE.with(|s| {
            s.borrow_mut()
                .config
                .get_or_insert_with(|| engine::Config::read(|key| self.native_setting(ID, key)))
                .clone()
        })
    }

    /// The tree's layout, cached per loaded tree.
    fn betteradvancements_cache(&self) {
        let tree = self.adv.tree.clone();
        let server = self.adv.positions.is_some();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.cache.as_ref().is_some_and(|c| Arc::ptr_eq(&c.tree, &tree) && c.server == server) {
                return;
            }
            let kids = tree.children();
            let pos = match &self.adv.positions {
                Some(placed) => placed.iter().map(|&(x, y)| Some(engine::widget_pos(x, y))).collect(),
                None => tree
                    .layout()
                    .into_iter()
                    .map(|p| p.map(|(x, y)| engine::widget_pos(x as f32, y)))
                    .collect(),
            };
            s.cache = Some(Cache { tree, server, kids, pos });
        });
    }

    fn betteradvancements_view(&self, cfg: &engine::Config) -> View {
        self.betteradvancements_cache();
        STATE.with(|s| {
            let s = s.borrow();
            let c = s.cache.as_ref().expect("cache");
            let tree = &c.tree;
            let visible = if self.adv.positions.is_some() {
                vec![true; tree.list.len()]
            } else {
                tree.visible(&c.kids, |i| self.adv.progress.done(&tree.list[i]))
            };
            // `onAddAdvancementRoot`: a displayed root with a background.
            let mut tabs: Vec<usize> = (0..tree.list.len())
                .filter(|&i| {
                    let a = &tree.list[i];
                    a.parent.is_none() && visible[i] && a.display.as_ref().is_some_and(|d| d.background.is_some())
                })
                .collect();
            const ORDER: [&str; 5] = [
                "minecraft:story/root",
                "minecraft:nether/root",
                "minecraft:end/root",
                "minecraft:adventure/root",
                "minecraft:husbandry/root",
            ];
            tabs.sort_by_key(|&i| {
                let id = &tree.list[i].id;
                (ORDER.iter().position(|r| r == id).unwrap_or(ORDER.len()), id.clone())
            });
            if cfg.order_tabs_alphabetically {
                let title = |i: usize| {
                    tree.list[i]
                        .display
                        .as_ref()
                        .map(|d| super::advancements::text_of(&d.title).plain())
                        .unwrap_or_default()
                };
                tabs.sort_by(|&a, &b| engine::compare_titles(&title(a), &title(b)));
            }
            let selected = s
                .tabs
                .selected
                .as_ref()
                .and_then(|id| tabs.iter().copied().find(|&i| tree.list[i].id == *id))
                .or_else(|| tabs.first().copied());
            let mut parent = vec![None; tree.list.len()];
            let mut widgets = Vec::new();
            let mut positions = Vec::new();
            if let Some(t) = selected {
                let mut stack = vec![(t, None::<usize>)];
                while let Some((i, up)) = stack.pop() {
                    let shown = visible[i] && tree.list[i].display.is_some() && c.pos[i].is_some();
                    let next = if shown {
                        parent[i] = up;
                        widgets.push(i);
                        positions.push(c.pos[i].unwrap_or_default());
                        Some(i)
                    } else {
                        up
                    };
                    for &k in c.kids[i].iter().rev() {
                        stack.push((k, next));
                    }
                }
            }
            View { tabs, selected, widgets, parent, bounds: engine::Bounds::of(&positions) }
        })
    }

    /// `AdvancementProgress` of the player, per criterion of `a` (names in the client's map order).
    fn betteradvancements_criteria(&self, a: &Advancement) -> Vec<(String, bool)> {
        let names: Vec<String> = a.criteria.iter().map(|c| c.name.clone()).collect();
        engine::java_map_order(&names)
            .into_iter()
            .map(|i| (names[i].clone(), self.adv.progress.has(&a.id, &names[i])))
            .collect()
    }

    /// `CriterionTranslator`: the first key the language has, else the plain name.
    fn betteradvancements_translate(&self, advancement: &str, criterion: &str) -> String {
        let (keys, plain) = engine::translation_keys(advancement, criterion);
        keys.iter()
            .find(|k| horizon_client::lang::has(k))
            .map(|k| horizon_client::lang::get(k))
            .unwrap_or(plain)
    }

    fn betteradvancements_cell_text(&self, adv: &str, cell: &engine::Cell) -> (Text, Text) {
        let body = match &cell.body {
            engine::Body::Criterion(name) => Text::literal(self.betteradvancements_translate(adv, name)),
            engine::Body::Remaining(n) => {
                Text::translate("betteradvancements.remaining", &[Text::literal(n.to_string())])
            }
        };
        let body = if cell.italic { body.italic() } else { body };
        (Text::literal(cell.marker), body)
    }

    pub(super) fn betteradvancements_open(&mut self) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.tabs.scrolling = false;
            s.drag = None;
            s.fade = 0.0;
        });
    }

    /// `BetterAdvancementsScreen.render`.
    pub(super) fn betteradvancements_draw(&mut self, batch: &mut HudBatch) {
        let cfg = self.betteradvancements_config();
        let look = engine::Look::from_config(&cfg);
        let view = self.betteradvancements_view(&cfg);
        let (sw, sh) = (batch.width as i32, batch.height as i32);
        let w = engine::Window::new(sw, sh, cfg.ui_scaling);
        let (width, height) = w.render_size();
        let per_page = engine::max_tabs(width, height, cfg.only_use_above_tabs);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.paging.init(view.tabs.len() as i32, per_page);
        });
        // The selected tab, and its scroll.
        let tree = self.adv.tree.clone();
        let root_id = view.selected.map(|i| tree.list[i].id.clone());
        if let Some(root) = &root_id {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                if s.tabs.selected.as_ref() != Some(root) {
                    let State { tabs, history, .. } = &mut *s;
                    tabs.select(root, &view.bounds, history, &w);
                }
            });
        }
        let scroll = match &root_id {
            Some(root) => STATE.with(|s| s.borrow_mut().tabs.frame_scroll(root, &view.bounds, &w)),
            None => (0, 0),
        };
        let (bl, bt, br, bb) = w.box_rect();
        let (bw, bh) = (br - bl, bb - bt);
        let f = |v: i32| v as f32;
        let (paging, tab_page) = STATE.with(|s| {
            let s = s.borrow();
            (s.paging, s.paging.range(per_page, view.tabs.len()))
        });

        {
            let atlas = &self.gui.atlas;
            ui::menu_background(batch, atlas, 0.0, 0.0, f(sw), f(sh), true);
            if paging.max_pages != 0 {
                let label = Text::literal(format!("{} / {}", paging.page + 1, paging.max_pages + 1));
                let tw = atlas.font.width_text(&label) as i32;
                let (_, x, y) = paging.label(tw, &w);
                batch.text_styled(atlas, &label, f(x), f(y), u32::MAX, true);
                for (n, (x, y, bw, bh)) in engine::page_buttons(&w).into_iter().enumerate() {
                    let hover = ui::hovered(self.cursor, f(x), f(y), f(bw), f(bh));
                    let label = Text::literal(if n == 0 { "<" } else { ">" });
                    let active = if n == 0 { paging.page > 0 } else { paging.page < paging.max_pages };
                    ui::button(batch, atlas, f(x), f(y), f(bw), f(bh), &label, active, hover);
                }
            }
        }

        // The inside (`renderInside` / `drawContents`).
        match view.selected {
            None => {
                let atlas = &self.gui.atlas;
                batch.fill(atlas, f(bl), f(bt), f(bw), f(bh), 0xFF00_0000);
                for (key, y) in [
                    ("advancements.empty", bt + bh / 2 - 9),
                    ("advancements.sad_label", bt + bh / 2 + 9),
                ] {
                    let t = Text::translate(key, &[]);
                    let tw = atlas.font.width_text(&t) as i32;
                    batch.text_styled(atlas, &t, f(bl + (bw - tw) / 2), f(y), u32::MAX, true);
                }
            }
            Some(root) => {
                let start = batch.vertices.len();
                let (origin_x, origin_y) = (bl, bt);
                let bg = tree.list[root].display.as_ref().and_then(|d| d.background.clone()).unwrap_or_default();
                let bg = bg.strip_prefix("minecraft:").unwrap_or(&bg).to_owned();
                let (i0, j0, ks, ls, rest) = engine::background_tiles(scroll, bw, bh);
                for k in ks {
                    let mut last_l = -1;
                    for l in ls.clone() {
                        batch.sprite_scaled(
                            &self.gui.atlas,
                            &bg,
                            f(origin_x + i0 + 16 * k),
                            f(origin_y + j0 + 16 * l),
                            16.0,
                            16.0,
                            u32::MAX,
                        );
                        last_l = l;
                    }
                    // The partial row of `height % 16`: its texels are the top ones.
                    if rest > 0 {
                        let y = origin_y + j0 + 16 * (last_l + 1);
                        batch.sprite_region(
                            &self.gui.atlas,
                            &bg,
                            [f(origin_x + i0 + 16 * k), f(y), 16.0, f(rest)],
                            16.0,
                            16.0,
                        );
                    }
                }
                let pos_of = |i: usize| {
                    STATE.with(|s| s.borrow().cache.as_ref().and_then(|c| c.pos[i]).unwrap_or_default())
                };
                let scale = batch.scale as i32;
                if !look.hide_lines {
                    for inside in [true, false] {
                        for &i in &view.widgets {
                            let Some(p) = view.parent[i] else { continue };
                            let done = self.adv.progress.done(&tree.list[i]);
                            let draws = engine::connection(pos_of(i), pos_of(p), scroll, inside, &look, done, scale);
                            self.betteradvancements_draws(batch, &draws, (origin_x, origin_y));
                        }
                    }
                }
                for &i in &view.widgets {
                    let a = &tree.list[i];
                    let Some(d) = &a.display else { continue };
                    let done = self.adv.progress.done(a);
                    if d.hidden && !done {
                        continue;
                    }
                    let (x, y) = pos_of(i);
                    let (x, y) = (f(origin_x + scroll.0 + x), f(origin_y + scroll.1 + y));
                    let tint = if look.has_custom_icon_color() {
                        rgba_of(look.icon_color(done))
                    } else {
                        u32::MAX
                    };
                    batch.sprite_scaled(&self.gui.atlas, frame_sprite(d.frame, done), x + 3.0, y, 26.0, 26.0, tint);
                    self.draw_item(batch, &ItemStack::new(&d.icon, 1), x + 8.0, y + 5.0);
                }
                batch.clip_since(start, f(bl), f(bt), f(br), f(bb));
            }
        }

        // `renderWindow`: the frame (`window.png`: corners of 30, the sides repeat), the tabs and the title.
        self.betteradvancements_window(batch, &w);
        let atlas = &self.gui.atlas;
        if view.tabs.len() > 1 {
            for (n, ti) in tab_page.clone().enumerate() {
                let global = ti as i32;
                let _ = n;
                let Some(side) = engine::tab_side(width, height, global, cfg.only_use_above_tabs) else { continue };
                let x = w.left + engine::tab_x(side, global, width, height);
                let y = w.top + engine::tab_y(side, global, width, height);
                let (tw, th) = side.size();
                let column = engine::tab_column(side, w.left, width, height, global);
                let selected = Some(view.tabs[ti]) == view.selected;
                batch.sprite_scaled(atlas, &tab_sprite(side, column, selected), f(x), f(y), f(tw), f(th), u32::MAX);
            }
            for ti in tab_page.clone() {
                let global = ti as i32;
                let Some(side) = engine::tab_side(width, height, global, cfg.only_use_above_tabs) else { continue };
                let x = w.left + engine::tab_x(side, global, width, height);
                let y = w.top + engine::tab_y(side, global, width, height);
                let (dx, dy) = engine::tab_icon_offset(side);
                if let Some(d) = &tree.list[view.tabs[ti]].display {
                    self.draw_item(batch, &ItemStack::new(&d.icon, 1), f(x + dx), f(y + dy));
                }
            }
        }
        let atlas = &self.gui.atlas;
        let title = Text::translate("gui.advancements", &[]);
        let title = match view.selected.and_then(|t| tree.list[t].display.as_ref()) {
            Some(d) => {
                let mut full = title;
                full = Text::literal(format!("{} - {}", full.plain(), super::advancements::text_of(&d.title).plain()));
                full
            }
            None => title,
        };
        batch.text_styled(atlas, &title, f(w.left + 8), f(w.top + 6), 0xFF40_4040, false);

        // `renderToolTips`: the veil and the hover box of the widget under the cursor.
        if let Some(root) = view.selected {
            let (ox, oy) = (w.left + engine::PADDING, w.top + 2 * engine::PADDING);
            let (tw, th) = w.tooltip_size();
            let (mx, my) = (self.cursor.0.floor() as i32 - ox, self.cursor.1.floor() as i32 - oy);
            let mut hovered = None;
            if mx > 0 && mx < tw && my > 0 && my < th {
                for &i in &view.widgets {
                    let a = &tree.list[i];
                    if a.display.as_ref().is_some_and(|d| d.hidden) && !self.adv.progress.done(a) {
                        continue;
                    }
                    let pos = STATE.with(|s| s.borrow().cache.as_ref().and_then(|c| c.pos[i]).unwrap_or_default());
                    if engine::widget_over(pos, scroll, mx, my) {
                        hovered = Some((i, pos));
                        break;
                    }
                }
            }
            let _ = root;
            let fade = STATE.with(|s| {
                let mut s = s.borrow_mut();
                let (fade, now) = (s.fade, std::time::Instant::now());
                // The original steps once per drawn frame at 20 ticks a second's pace of `render`; frames here
                // are scaled to the tick so the fade lasts the same.
                let ticks = s.last_frame.map_or(1.0, |t| (now - t).as_secs_f32() * 20.0).min(5.0);
                s.last_frame = Some(now);
                let mut f = fade;
                let steps = ticks.max(0.0);
                let whole = steps.floor() as i32;
                for _ in 0..whole.max(1) {
                    f = engine::fade_step(f, hovered.is_some(), cfg.do_fade);
                }
                s.fade = f;
                f
            });
            let atlas = &self.gui.atlas;
            batch.fill(atlas, f(ox), f(oy), f(tw), f(th), engine::fade_alpha(fade) << 24);
            if let Some((i, pos)) = hovered {
                self.betteradvancements_hover(batch, &cfg, &look, &w, i, pos, scroll, (ox, oy));
            }
            if cfg.show_debug_coordinates {
                if let Some((text, x, y)) =
                    engine::debug_coordinates(&w, (self.cursor.0 as i32, self.cursor.1 as i32), scroll)
                {
                    batch.text_styled(&self.gui.atlas, &Text::literal(text), f(x), f(y), 0xFF00_0000, false);
                }
            }
        }
        // The tab titles.
        if view.tabs.len() > 1 {
            for ti in tab_page {
                let global = ti as i32;
                let Some(side) = engine::tab_side(width, height, global, cfg.only_use_above_tabs) else { continue };
                if engine::tab_over(side, w.left, w.top, width, height, global, f64::from(self.cursor.0), f64::from(self.cursor.1)) {
                    if let Some(d) = &tree.list[view.tabs[ti]].display {
                        ui::tooltip(batch, &self.gui.atlas, &[super::advancements::text_of(&d.title)], self.cursor);
                    }
                }
            }
        }
    }

    /// The window frame of `renderWindow`: `window.png` is 252 x 140; the corners are 30 and the sides repeat.
    fn betteradvancements_window(&self, batch: &mut HudBatch, w: &engine::Window) {
        let atlas = &self.gui.atlas;
        let name = "gui/advancements/window";
        let Some(r) = atlas.sprites.get(name).copied() else { return };
        // The sheet is 256 x 256 texels in the atlas with the window in its top left 252 x 140.
        let (tw, th) = (256.0f32, 256.0f32);
        let piece = |batch: &mut HudBatch, x: i32, y: i32, dw: i32, dh: i32, u: i32, v: i32, sw: i32, sh: i32| {
            if dw <= 0 || dh <= 0 || sw <= 0 || sh <= 0 {
                return;
            }
            let rect = horizon_client::hud::Rect {
                u0: r.u0 + (r.u1 - r.u0) * u as f32 / tw,
                v0: r.v0 + (r.v1 - r.v0) * v as f32 / th,
                u1: r.u0 + (r.u1 - r.u0) * (u + sw) as f32 / tw,
                v1: r.v0 + (r.v1 - r.v0) * (v + sh) as f32 / th,
                w: sw as u32,
                h: sh as u32,
            };
            batch.rect_uv(x as f32, y as f32, dw as f32, dh as f32, rect, u32::MAX);
        };
        // `RenderUtil.renderRepeating`: the source piece repeats to fill the destination, the last tile cut.
        let repeat = |batch: &mut HudBatch, x: i32, y: i32, dw: i32, dh: i32, u: i32, v: i32, sw: i32, sh: i32| {
            if sw <= 0 || sh <= 0 {
                return;
            }
            let mut oy = 0;
            while oy < dh {
                let h = sh.min(dh - oy);
                let mut ox = 0;
                while ox < dw {
                    let wd = sw.min(dw - ox);
                    piece(batch, x + ox, y + oy, wd, h, u, v, wd, h);
                    ox += sw;
                }
                oy += sh;
            }
        };
        let (cs, ww, wh) = (engine::CORNER, engine::WINDOW_TEXTURE_W, engine::WINDOW_TEXTURE_H);
        let (left, top, right, bottom) = (w.left, w.top, w.right, w.bottom);
        let across = w.internal_width - cs - 2 * engine::SIDE - cs;
        piece(batch, left, top, cs, cs, 0, 0, cs, cs);
        repeat(batch, left + cs, top, across, cs, cs, 0, ww - 2 * cs, cs);
        piece(batch, right - cs, top, cs, cs, ww - cs, 0, cs, cs);
        repeat(batch, left, top + cs, cs, bottom - top - 2 * cs, 0, cs, cs, wh - 2 * cs);
        repeat(batch, right - cs, top + cs, cs, bottom - top - 2 * cs, ww - cs, cs, cs, wh - 2 * cs);
        piece(batch, left, bottom - cs, cs, cs, 0, wh - cs, cs, cs);
        repeat(batch, left + cs, bottom - cs, across, cs, cs, wh - cs, ww - 2 * cs, cs);
        piece(batch, right - cs, bottom - cs, cs, cs, ww - cs, wh - cs, cs, cs);
    }

    fn betteradvancements_draws(&self, batch: &mut HudBatch, draws: &[engine::Draw], origin: (i32, i32)) {
        let atlas = &self.gui.atlas;
        let (ox, oy) = (origin.0 as f32, origin.1 as f32);
        for d in draws {
            match d {
                engine::Draw::Fill { x0, y0, x1, y1, color } => {
                    let (ax, bx) = ((*x0).min(*x1), (*x0).max(*x1));
                    let (ay, by) = ((*y0).min(*y1), (*y0).max(*y1));
                    batch.fill(atlas, ox + ax as f32, oy + ay as f32, (bx - ax) as f32, (by - ay) as f32, rgba_of(*color));
                }
                engine::Draw::Quad { corners, color } => {
                    let c = corners.map(|p| [ox + p[0] as f32, oy + p[1] as f32]);
                    batch.fill_quad(atlas, c, rgba_of(*color));
                }
            }
        }
    }

    /// `BetterAdvancementWidget.drawHover`.
    #[allow(clippy::too_many_arguments)]
    fn betteradvancements_hover(
        &self,
        batch: &mut HudBatch,
        cfg: &engine::Config,
        look: &engine::Look,
        w: &engine::Window,
        i: usize,
        pos: (i32, i32),
        scroll: (i32, i32),
        origin: (i32, i32),
    ) {
        let tree = self.adv.tree.clone();
        let a = &tree.list[i];
        let Some(d) = &a.display else { return };
        let atlas = &self.gui.atlas;
        let font = &atlas.font;
        let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
        let show = engine::show_grid(cfg.requires_shift, shift);
        // `refreshHover`.
        let title: String = super::advancements::text_of(&d.title).plain().chars().take(163).collect();
        let k = engine::counter_room(
            a.criteria.len(),
            font.width("  ") as i32,
            font.width("0") as i32,
            font.width("/") as i32,
        );
        let title_width = 29 + font.width(&title) as i32 + k;
        let done = self.adv.progress.done(a);
        let criteria = self.betteradvancements_criteria(a);
        let cells = engine::build_cells(cfg.detail, cfg.color_whole_criteria_text, done, &criteria);
        let cell_texts: Vec<(Text, Text)> = cells
            .as_ref()
            .map(|c| c.iter().map(|cell| self.betteradvancements_cell_text(&a.id, cell)).collect())
            .unwrap_or_default();
        let widths: Vec<i32> = cell_texts.iter().map(|(m, b)| (font.width_text(m) + font.width_text(b)) as i32).collect();
        let grid = if cells.is_some() {
            engine::Grid::find(&widths, engine::LINE_HEIGHT, w.width / 2, w.height / 2)
        } else {
            engine::Grid::empty()
        };
        let max_width = if show { title_width.max(grid.width) } else { title_width };
        let color = match d.frame {
            Frame::Challenge => 0xAA00AA,
            _ => 0x55FF55,
        };
        let desc_text = super::advancements::text_of(&d.description).with_style(&Style::color(color));
        let empty = desc_text.plain().is_empty();
        let mut desc = engine::find_optimal_lines(
            empty,
            |width| font.split(&desc_text, width as f32),
            max_width,
            w.internal_width,
        );
        let mut width = max_width;
        for l in &desc {
            width = width.max(font.width_text(l) as i32);
        }
        let width = width + 8;
        let (got, total) = self.adv.progress.counts(a);
        let percent = if a.criteria.is_empty() { 0.0 } else { engine::percent(got, total) };
        let progress = if a.criteria.is_empty() { None } else { engine::progress_text(got, total) };
        let progress_width = progress.as_ref().map_or(0, |p| font.width(p) as i32);
        let plan = engine::plan_hover(&engine::HoverIn {
            pos,
            scroll,
            width,
            desc_lines: desc.len() as i32,
            grid_height: grid.height,
            show_grid: show,
            percent,
            window_left: w.left,
            window_top: w.top,
            internal_width: w.internal_width,
            screen_height: w.height,
            progress_width,
        });
        let f = |v: i32| v as f32;
        let (ox, oy) = origin;
        let tint_title = |obtained: bool| {
            if look.has_custom_title_color() { rgba_of(look.title_color(obtained)) } else { u32::MAX }
        };
        if !desc.is_empty() {
            ui::nine_slice(
                batch,
                atlas,
                "gui/sprites/advancements/title_box",
                f(ox + plan.draw_x),
                f(oy + plan.box_y),
                f(width),
                f(plan.box_height),
                u32::MAX,
            );
        }
        let (first_w, second_w) = (plan.j, width - plan.j);
        if plan.title_left_obtained != plan.title_right_obtained {
            for (obtained, src_x, at, wd) in [
                (plan.title_left_obtained, 0, plan.draw_x, first_w),
                (plan.title_right_obtained, 200 - second_w, plan.draw_x + first_w, second_w),
            ] {
                let start = batch.vertices.len();
                ui::nine_slice(
                    batch,
                    atlas,
                    box_sprite(obtained),
                    f(ox + at - src_x),
                    f(oy + plan.draw_y),
                    200.0,
                    26.0,
                    tint_title(obtained),
                );
                batch.clip_since(start, f(ox + at), f(oy + plan.draw_y), f(ox + at + wd), f(oy + plan.draw_y + 26));
            }
        } else {
            ui::nine_slice(
                batch,
                atlas,
                box_sprite(plan.title_left_obtained),
                f(ox + plan.draw_x),
                f(oy + plan.draw_y),
                f(width),
                26.0,
                tint_title(plan.title_left_obtained),
            );
        }
        let icon_tint = if look.has_custom_icon_color() { rgba_of(look.icon_color(plan.icon_obtained)) } else { u32::MAX };
        batch.sprite_scaled(
            atlas,
            frame_sprite(d.frame, plan.icon_obtained),
            f(ox + scroll.0 + pos.0 + 3),
            f(oy + scroll.1 + pos.1),
            26.0,
            26.0,
            icon_tint,
        );
        batch.text_styled(atlas, &Text::literal(title), f(ox + plan.title_x), f(oy + plan.title_y), u32::MAX, true);
        if let Some(p) = &progress {
            batch.text_styled(atlas, &Text::literal(p.clone()), f(ox + plan.progress_x), f(oy + plan.title_y), u32::MAX, true);
        }
        for (n, line) in desc.drain(..).enumerate() {
            batch.text_styled(
                atlas,
                &line,
                f(ox + plan.desc_x),
                f(oy + plan.desc_y + n as i32 * engine::LINE_HEIGHT),
                0xFFAA_AAAA,
                false,
            );
        }
        if show && grid.has_pages() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64);
            let page = grid.current_page(now, cfg.criteria_rotation_speed);
            let mut x = plan.grid_x;
            for column in &grid.paged[page.min(grid.paged.len() - 1)] {
                for (row, &cell) in column.cells.iter().enumerate() {
                    let (marker, body) = &cell_texts[cell];
                    let c = cells.as_ref().map(|c| c[cell].clone()).expect("cells");
                    let y = f(oy + plan.grid_y + row as i32 * engine::LINE_HEIGHT);
                    batch.text_styled(atlas, marker, f(ox + x), y, rgb_of(c.marker_color), false);
                    let mw = font.width_text(marker);
                    batch.text_styled(atlas, body, f(ox + x) + mw, y, rgb_of(c.body_color), false);
                }
                x += column.width;
            }
        }
        self.draw_item(
            batch,
            &ItemStack::new(&d.icon, 1),
            f(ox + scroll.0 + pos.0 + 8),
            f(oy + scroll.1 + pos.1 + 5),
        );
    }

    /// A left button press or release (the hook of `advancements_mouse`). True when the mod handled it.
    pub(super) fn betteradvancements_mouse(&mut self, pressed: bool, size: [u32; 2]) {
        if !pressed {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.drag = None;
                s.tabs.scrolling = false;
            });
            return;
        }
        let cfg = self.betteradvancements_config();
        let view = self.betteradvancements_view(&cfg);
        let b = HudBatch::new(size);
        let w = engine::Window::new(b.width as i32, b.height as i32, cfg.ui_scaling);
        let (width, height) = w.render_size();
        let per_page = engine::max_tabs(width, height, cfg.only_use_above_tabs);
        let (cx, cy) = self.cursor;
        // The page buttons.
        let paging = STATE.with(|s| s.borrow().paging);
        if paging.max_pages != 0 {
            for (n, (x, y, bw, bh)) in engine::page_buttons(&w).into_iter().enumerate() {
                if ui::hovered(self.cursor, x as f32, y as f32, bw as f32, bh as f32) {
                    self.audio.play("minecraft:ui.button.click", None, 0.25, 1.0);
                    STATE.with(|s| {
                        let mut s = s.borrow_mut();
                        if n == 0 { s.paging.previous() } else { s.paging.next() }
                    });
                    return;
                }
            }
        }
        // The tabs (`mouseClicked`: the hit size is the click one).
        let (cw, ch) = w.click_size();
        for ti in paging.range(per_page, view.tabs.len()) {
            let global = ti as i32;
            let Some(side) = engine::tab_side(width, height, global, cfg.only_use_above_tabs) else { continue };
            if engine::tab_over(side, w.left, w.top, cw, ch, global, f64::from(cx), f64::from(cy)) {
                let root = self.adv.tree.list[view.tabs[ti]].id.clone();
                let bounds = self.betteradvancements_bounds(view.tabs[ti]);
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    if s.tabs.selected.as_ref() != Some(&root) {
                        let State { tabs, history, .. } = &mut *s;
                        tabs.select(&root, &bounds, history, &w);
                    }
                });
                return;
            }
        }
        STATE.with(|s| s.borrow_mut().drag = Some((cx, cy)));
    }

    /// The bounds of a tab's widgets, for a tab that is not the drawn one.
    fn betteradvancements_bounds(&self, root: usize) -> engine::Bounds {
        STATE.with(|s| {
            let s = s.borrow();
            let Some(c) = s.cache.as_ref() else { return engine::Bounds::of(&[]) };
            let mut positions = Vec::new();
            let mut stack = vec![root];
            while let Some(i) = stack.pop() {
                if c.tree.list[i].display.is_some()
                    && let Some(p) = c.pos[i]
                {
                    positions.push(p);
                }
                stack.extend(c.kids[i].iter().copied());
            }
            engine::Bounds::of(&positions)
        })
    }

    /// `mouseDragged` (left button).
    pub(super) fn betteradvancements_drag(&mut self) {
        let Some(last) = STATE.with(|s| s.borrow().drag) else { return };
        let (dx, dy) = (self.cursor.0 - last.0, self.cursor.1 - last.1);
        STATE.with(|s| s.borrow_mut().drag = Some(self.cursor));
        let moving = STATE.with(|s| s.borrow_mut().tabs.drag_step());
        if moving {
            self.betteradvancements_scroll_by(f64::from(dx), f64::from(dy));
        }
    }

    pub(super) fn betteradvancements_scroll_by(&mut self, dx: f64, dy: f64) {
        let size = self.window.as_ref().map_or([1280, 720], |w| w.inner_size().into());
        let cfg = self.betteradvancements_config();
        let view = self.betteradvancements_view(&cfg);
        let b = HudBatch::new(size);
        let w = engine::Window::new(b.width as i32, b.height as i32, cfg.ui_scaling);
        STATE.with(|s| s.borrow_mut().tabs.scroll_by(&view.bounds, dx, dy, &w));
    }
}
