// Creative tabs of content mods: the screen's side (game.rs). The "<" and ">" buttons, the
// "page / pages" label and the clicks on the buttons; the rules and the page are in
// creative.rs (module taburi_creative), reached through the getter the mod adds to `Creative`.
use super::{Game, Screen};
use horizon_client::hud::HudBatch;

fn inside(p: (f32, f32), [x, y, w, h]: [f32; 4]) -> bool {
    p.0 >= x && p.1 >= y && p.0 < x + w && p.1 < y + h
}

impl Game<'_> {
    /// The buttons and the label over the creative screen, once there is more than one page
    /// (Forge 1.20.1's "<" and ">" 50 above the screen's edge, the label 44 above it).
    pub(super) fn taburi_creative_draw(&mut self, batch: &mut HudBatch) {
        if self.screen != Screen::Creative {
            return;
        }
        let paging = self.creative_screen.__horizon_mixins_taburi_creative_paging();
        if !paging.paged() {
            return;
        }
        let (ox, oy) = Self::creative_origin(&*batch);
        let atlas = &self.gui.atlas;
        for (label, rect) in ["<", ">"].into_iter().zip(paging.arrows()) {
            let rect = [ox + rect[0], oy + rect[1], rect[2], rect[3]];
            let sprite = if inside(self.cursor, rect) {
                "gui/sprites/widget/button_highlighted"
            } else {
                "gui/sprites/widget/button"
            };
            horizon_client::ui::nine_slice(
                batch, atlas, sprite, rect[0], rect[1], rect[2], rect[3], u32::MAX,
            );
            // Button.renderString: centred, 6 below the top for a 20 high button.
            let width = HudBatch::text_width(atlas, label);
            batch.text(
                atlas,
                label,
                rect[0] + (rect[2] - width) / 2.0,
                rect[1] + 6.0,
                u32::MAX,
                true,
            );
            horizon_client::widgets::record(label, rect, true);
        }
        let text = paging.label();
        let (x, y) = paging.label_at(HudBatch::text_width(atlas, &text));
        batch.text(atlas, &text, ox + x, oy + y, u32::MAX, true);
    }

    /// A left click on one of the buttons turns the page: true when it was one's.
    pub(super) fn taburi_creative_click(&mut self, right: bool) -> bool {
        if right || self.screen != Screen::Creative {
            return false;
        }
        if !self.creative_screen.__horizon_mixins_taburi_creative_paging().paged() {
            return false;
        }
        let Some(batch) = self.gui_scale() else {
            return false;
        };
        let (ox, oy) = Self::creative_origin(&batch);
        let (cx, cy) = self.cursor;
        let taken = self
            .creative_screen
            .__horizon_mixins_taburi_creative_paging()
            .press(cx - ox, cy - oy);
        if taken {
            // Like a tab (select_creative_tab): outside the panel, so the release must not
            // drop a carried stack.
            self.inv_clicks = Default::default();
            self.inv_clicks.skip_release();
            self.inv_drag = None;
            self.last_inv_click = None;
        }
        taken
    }

    /// The pad's shoulder buttons click the next or previous tab where it is drawn, so its
    /// page has to be up before they do.
    pub(super) fn taburi_creative_pad(&mut self, next: bool) {
        let count = self.creative_screen.tabs.len();
        if count == 0 {
            return;
        }
        let selected = self.creative_screen.selected;
        let tab = if next {
            (selected + 1) % count
        } else {
            (selected + count - 1) % count
        };
        let id = self.creative_screen.tabs[tab].id.clone();
        self.creative_screen
            .__horizon_mixins_taburi_creative_paging()
            .show(&id);
    }
}
