// AdditionalBanners in the menu engine (`crates/horizon-client/src/menu.rs`, library crate): the shears of the
// pattern recipes come back hurt by one (`bookshelf:shapeless_durability`), like the 1.20.1 recipe type.
// tools/bundle_extra.py pastes src/lib.rs in place of the include! and writes mixins/additionalbanners/menu_runtime.rs.
#[allow(dead_code)]
mod engine {
    // AdditionalBanners for Horizon: the rules, without any game code.
// Source: the pack's 1.20.1 jar AdditionalBanners-Forge-1.20.1-14.0.5 (javap of Content, TradeConfig), Bookshelf
// 20.2.15 (VillagerSells, MerchantTier) and his config (config/additionalbanners/trades/*.json, written into the
// carry by tools/prepare_carry.py). This file is pasted into the game three times: as
// `crate::game::additionalbanners::engine` (tools/hnmod.py bundle), into `mod_content.rs` and `menu.rs`
// (tools/bundle_extra.py); it is also the `horizon_additionalbanners` crate the tests use.

/// The 29 items in registration order (`Content.<init>`: `createPattern(name, Rarity)`), each a
/// `BannerPatternItem` with `stacksTo(1)` and that rarity.
pub const ITEMS: [(&str, &str); 29] = [
    ("anchor", "common"),
    ("balance", "common"),
    ("grass", "common"),
    ("kelp", "common"),
    ("mushroom", "common"),
    ("pumpkin", "common"),
    ("snowflake", "common"),
    ("steps", "common"),
    ("squid", "uncommon"),
    ("shield", "uncommon"),
    ("sword", "uncommon"),
    ("frame", "uncommon"),
    ("hex", "uncommon"),
    ("waterfall", "uncommon"),
    ("starburst", "uncommon"),
    ("prismarine", "uncommon"),
    ("scales", "uncommon"),
    ("chain", "uncommon"),
    ("dragon", "rare"),
    ("phantom", "rare"),
    ("trident", "rare"),
    ("cobweb", "rare"),
    ("spider", "rare"),
    ("clubs", "epic"),
    ("diamond", "epic"),
    ("heart", "epic"),
    ("spades", "epic"),
    ("moon", "epic"),
    ("squares", "epic"),
];

/// `Content.createPattern("scales", UNCOMMON, "up", "down", "left", "right")`: the only item with several
/// patterns (its tag `additionalbanners:pattern_item/scales`, in the 1.20.1 tag's order).
pub const SCALES: [&str; 4] = ["scales_down", "scales_up", "scales_left", "scales_right"];

pub const NS: &str = "additionalbanners";

/// The item's name without the namespace, when it is one of ours.
pub fn item_name(id: &str) -> Option<&'static str> {
    let path = id.strip_prefix("additionalbanners:")?;
    ITEMS.iter().map(|(n, _)| *n).find(|n| *n == path)
}

/// The original's banner pattern ids (`additionalbanners:<p>`) an item gives the loom.
pub fn item_patterns(name: &str) -> Vec<String> {
    if name == "scales" {
        SCALES.iter().map(|p| format!("{NS}:{p}")).collect()
    } else {
        vec![format!("{NS}:{name}")]
    }
}

/// Every pattern of the mod (32: 28 items with one, scales with four).
pub fn patterns() -> Vec<String> {
    ITEMS.iter().flat_map(|(n, _)| item_patterns(n)).collect()
}

/// The pattern's id in Horizon's registry. `Banners::load` and the registry sync read only
/// `data/minecraft/banner_pattern/`, so the mod's patterns live there as `minecraft:additionalbanners_<p>`.
pub fn horizon_pattern(original: &str) -> Option<String> {
    let p = original.strip_prefix("additionalbanners:")?;
    Some(format!("minecraft:{NS}_{p}"))
}

/// `minecraft:provides_banner_patterns` of an item: its own pattern tag, which the carry writes under
/// `data/minecraft/tags/banner_pattern/` (the only folder `Banners::load` reads tags from).
pub fn pattern_tag(id: &str) -> Option<String> {
    item_name(id).map(|n| format!("#minecraft:pattern_item/{NS}/{n}"))
}

/// `BannerPatternItem.appendHoverText`: the grey `<descriptionId>.desc` line.
pub fn desc_key(id: &str) -> Option<String> {
    item_name(id).map(|n| format!("item.{NS}.{n}.desc"))
}

/// Where a carried file is copied in the content root, so the minecraft-only registries read it:
/// pattern definitions, their banner and shield textures, and the villager trades.
pub fn mirror(rel: &str) -> Option<String> {
    let rest = rel
        .strip_prefix("data/additionalbanners/")
        .or_else(|| rel.strip_prefix("assets/additionalbanners/"))?;
    let data = rel.starts_with("data/");
    let file = |dir: &str, ext: &str| {
        rest.strip_prefix(dir)
            .and_then(|f| f.strip_suffix(ext))
            .filter(|s| !s.is_empty() && !s.contains('/'))
    };
    if data {
        if let Some(p) = file("banner_pattern/", ".json") {
            return Some(format!("data/minecraft/banner_pattern/{NS}_{p}.json"));
        }
        if let Some(t) = file("villager_trade/", ".json") {
            return Some(format!("data/minecraft/villager_trade/{NS}/{t}.json"));
        }
    } else {
        for kind in ["banner", "shield"] {
            if let Some(p) = file(&format!("textures/entity/{kind}/"), ".png") {
                return Some(format!("assets/minecraft/textures/entity/{kind}/{NS}_{p}.png"));
            }
        }
    }
    None
}

// ---------------------------------------------------------------- shears in the recipes

/// A crafting grid cell, as the rules see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub item: String,
    pub count: u32,
    pub damage: u32,
}

/// The grid while it matches one of the mod's recipes, and where the shears are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Matched {
    pub grid: Vec<Option<Cell>>,
    pub shears: usize,
}

/// `bookshelf:shears` of the 1.20.1 recipes: the vanilla shears (the tag's only member in the pack).
pub fn is_shears(item: &str) -> bool {
    item == "minecraft:shears"
}

/// The grid matches a recipe whose result is one of the mod's items and holds one pair of shears.
pub fn matched(grid: &[Option<Cell>], result: Option<&str>) -> Option<Matched> {
    item_name(result?)?;
    let mut shears = None;
    for (i, c) in grid.iter().enumerate() {
        if let Some(c) = c
            && is_shears(&c.item)
        {
            if shears.is_some() || c.count != 1 {
                return None;
            }
            shears = Some(i);
        }
    }
    Some(Matched {
        grid: grid.to_vec(),
        shears: shears?,
    })
}

/// What happens to the shears after the grid changed from `before` to `now`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shears {
    /// Not a craft of the matched recipe (the player moved items): nothing to do.
    Untouched,
    /// Crafted: the shears go back to their cell with this damage
    /// (`ShapelessDurabilityRecipe.getRemainingItems`: the shears, hurt by one).
    Back { cell: usize, damage: u32 },
    /// Crafted with the last point of durability: the shears break (no remainder).
    Broke,
}

/// `ResultSlot.onTake` took one of every ingredient (a remainder such as the dragon breath's bottle may
/// take an emptied cell): the shears' cell is empty now and every other cell lost exactly one item.
pub fn after_take(before: &Matched, now: &[Option<Cell>], max_damage: u32) -> Shears {
    if now.len() != before.grid.len() || now[before.shears].is_some() {
        return Shears::Untouched;
    }
    for (i, (b, n)) in before.grid.iter().zip(now).enumerate() {
        if i == before.shears {
            continue;
        }
        let ok = match (b, n) {
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(b), None) => b.count == 1,
            (Some(b), Some(n)) if n.item == b.item => n.count + 1 == b.count,
            // The ingredient's crafting remainder in its emptied cell.
            (Some(b), Some(n)) => b.count == 1 && n.count == 1,
        };
        if !ok {
            return Shears::Untouched;
        }
    }
    let Some(shears) = &before.grid[before.shears] else {
        return Shears::Untouched;
    };
    let damage = shears.damage + 1;
    if max_damage > 0 && damage >= max_damage {
        Shears::Broke
    } else {
        Shears::Back {
            cell: before.shears,
            damage,
        }
    }
}

// ---------------------------------------------------------------- trades (data, checked by the tests)

/// One trade of the pack's config (`TradeConfig.Offer`), as tools/prepare_carry.py writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Trade {
    pub item: String,
    pub emeralds: u32,
    pub max_uses: u32,
    pub xp: u32,
    pub price_multiplier: f64,
}

/// `VillagerSells.create(item, emeraldCost, maxUses, villagerExp, priceMultiplier)`:
/// `new MerchantOffer(new ItemStack(EMERALD, emeraldCost), new ItemStack(item), maxUses, xp, mult)`,
/// in the 26.3 `villager_trade` form.
pub fn trade_json(t: &Trade) -> String {
    format!(
        "{{\"gives\":{{\"id\":\"{}\"}},\"max_uses\":{},\"reputation_discount\":{},\"wants\":{{\"count\":{},\"id\":\"minecraft:emerald\"}},\"xp\":{}}}",
        t.item, t.max_uses, t.price_multiplier, t.emeralds, t.xp
    )
}

/// `MerchantTier` (Bookshelf): NOVICE..MASTER, ordinal + 1 = the villager level of the trade.
pub fn tier_level(tier: &str) -> Option<u32> {
    ["NOVICE", "APPRENTICE", "JOURNEYMAN", "EXPERT", "MASTER"]
        .iter()
        .position(|t| *t == tier)
        .map(|i| i as u32 + 1)
}

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
