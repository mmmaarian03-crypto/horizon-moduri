// AdditionalBanners in the menu engine (`crates/horizon-client/src/menu.rs`, library crate): the shears of the
// pattern recipes come back hurt by one (`bookshelf:shapeless_durability`), like the 1.20.1 recipe type.
// tools/bundle_extra.py pastes src/lib.rs in place of the include! and writes mixins/additionalbanners/menu_runtime.rs.
#[allow(dead_code)]
mod engine {
    include!("additionalbanners_engine.rs");
}
use super::{Env, GRID, Menu};
use crate::items::ItemStack;
use std::cell::RefCell;

thread_local! {
    /// The last grid that matched one of the mod's recipes, with the shears as they were.
    static MATCHED: RefCell<Option<(engine::Matched, ItemStack)>> = const { RefCell::new(None) };
}

fn cells(menu: &Menu) -> Vec<Option<engine::Cell>> {
    menu.containers[GRID]
        .iter()
        .map(|c| {
            c.as_ref().map(|s| engine::Cell {
                item: s.item.to_string(),
                count: s.count,
                damage: s.damage,
            })
        })
        .collect()
}

/// HEAD on `Menu::recompute` (every change of the crafting grid, and the end of `ResultSlot.onTake` after
/// one of each ingredient was taken): a craft of the matched recipe puts the shears back with one more
/// point of damage; then the grid is remembered when it matches again.
pub(super) fn recompute(menu: &mut Menu, env: &Env<'_>) {
    if menu.grid_size == 0 {
        return;
    }
    let now = cells(menu);
    if let Some((before, shears)) = MATCHED.with(|m| m.borrow_mut().take()) {
        let max = env.items.get(&shears.item).map_or(0, |i| i.max_damage);
        if let engine::Shears::Back { cell, damage } = engine::after_take(&before, &now, max)
            && cell < menu.containers[GRID].len()
        {
            let mut back = shears.clone();
            back.damage = damage;
            menu.containers[GRID][cell] = Some(back);
        }
    }
    let now = cells(menu);
    let result = env
        .recipes
        .find(&menu.containers[GRID], menu.grid_size)
        .map(|(_, stack)| stack.item.to_string());
    if let Some(m) = engine::matched(&now, result.as_deref())
        && let Some(shears) = menu.containers[GRID][m.shears].clone()
    {
        MATCHED.with(|s| *s.borrow_mut() = Some((m, shears)));
    }
}
