// AdditionalBanners inside Horizon's game (nightly 142388e1c): the window's half. The registry, loom and trade
// halves are in the content root builder (native/content.rs) and the menu engine (native/menu.rs).
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("additionalbanners_engine.rs");
}
use horizon_client::{items::ItemStack, text::Text};
use std::cell::RefCell;

thread_local! {
    /// The window's own crafting grid (`--in-frame` host) the last time `craft_recipe` looked at it, when it
    /// matched one of the mod's recipes: the cells and the shears as they were.
    static FRAME_MATCHED: RefCell<Option<(engine::Matched, ItemStack)>> = const { RefCell::new(None) };
}

fn frame_cells(grid: &[Option<ItemStack>]) -> Vec<Option<engine::Cell>> {
    grid.iter()
        .map(|c| {
            c.as_ref().map(|s| engine::Cell {
                item: s.item.to_string(),
                count: s.count,
                damage: s.damage,
            })
        })
        .collect()
}

impl Game<'_> {
    /// The host's own grid (in-frame window, or a LAN host's window): guests craft through the menu engine,
    /// whose `Menu::recompute` HEAD (native/menu.rs) puts their shears back.
    fn additionalbanners_own_grid(&self) -> bool {
        self.remote.is_none() && self.lan_acting.is_none()
    }

    /// HEAD on `Game::craft_recipe` (every frame the result slot is drawn, and once per craft in
    /// `click_inventory`'s ResultSlot loop): remember the grid while it matches a pattern recipe with shears.
    pub(super) fn additionalbanners_frame_look(&self) {
        if !self.additionalbanners_own_grid() {
            return;
        }
        let n = self.craft_size.min(3);
        let grid = &self.craft_grid[..(n * n).min(self.craft_grid.len())];
        // Cheap test first: the result is looked up only when the grid holds shears.
        let snap = if grid.iter().flatten().any(|s| engine::is_shears(&s.item)) {
            let result = self.res.recipes().find(grid, n).map(|(_, stack)| stack.item.to_string());
            let cells = frame_cells(grid);
            engine::matched(&cells, result.as_deref())
                .and_then(|m| grid[m.shears].clone().map(|shears| (m, shears)))
        } else {
            None
        };
        FRAME_MATCHED.with(|s| *s.borrow_mut() = snap);
    }

    /// HEAD on `Game::award_crafted`: with `--in-frame` a plain click on the result slot is
    /// `click_inventory`'s own ResultSlot code (`menu_ops` refuses slot 200 without Shift), which took one of each
    /// ingredient with `Recipes::remainders` (not hookable). `ShapelessDurabilityRecipe.getRemainingItems`: the
    /// shears go back to their cell hurt by one (Shift-click: once per craft, the loop sees them again).
    pub(super) fn additionalbanners_frame_crafted(&mut self) {
        if !self.additionalbanners_own_grid() {
            return;
        }
        let Some((before, shears)) = FRAME_MATCHED.with(|s| s.borrow_mut().take()) else {
            return;
        };
        let n = self.craft_size.min(3);
        let cells = (n * n).min(self.craft_grid.len());
        let now = frame_cells(&self.craft_grid[..cells]);
        let max = self.res.items().get(&shears.item).map_or(0, |i| i.max_damage);
        if let engine::Shears::Back { cell, damage } = engine::after_take(&before, &now, max)
            && cell < cells
        {
            let mut back = shears;
            back.damage = damage;
            self.craft_grid[cell] = Some(back);
        }
    }

    /// RETURN on `stations::Game::stack_tooltip`: `BannerPatternItem.appendHoverText`, the grey
    /// `item.additionalbanners.<name>.desc` line under the name.
    pub(super) fn additionalbanners_tooltip(
        &self,
        stack: &ItemStack,
        mut lines: Vec<(Text, u32)>,
    ) -> Vec<(Text, u32)> {
        let Some(key) = engine::desc_key(&stack.item) else {
            return lines;
        };
        if lines.is_empty() {
            return lines;
        }
        let text = self.pack.names.display_get(&key).unwrap_or(key);
        let gray = horizon_client::hud::rgba(170, 170, 170, 255);
        lines.insert(1, (Text::legacy(&text), gray));
        lines
    }
}
