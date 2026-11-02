use horizon_additionalbanners::*;
use std::path::{Path, PathBuf};

const CARRY: &str = r"../../content/mods/additionalbanners/carry";
const CONFIG: &str =
    r"../../content/config/additionalbanners/trades";

fn carry(rel: &str) -> Option<PathBuf> {
    let p = Path::new(CARRY).join(rel);
    Path::new(CARRY).exists().then_some(p)
}
fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}
fn cell(item: &str, count: u32, damage: u32) -> Option<Cell> {
    Some(Cell {
        item: item.into(),
        count,
        damage,
    })
}

#[test]
fn items_and_patterns_are_the_1201_ones() {
    assert_eq!(ITEMS.len(), 29);
    assert_eq!(patterns().len(), 32);
    assert_eq!(item_name("additionalbanners:scales"), Some("scales"));
    assert_eq!(item_name("additionalbanners:anchor_banner_pattern"), None);
    assert_eq!(item_name("minecraft:anchor"), None);
    assert_eq!(
        item_patterns("scales"),
        vec![
            "additionalbanners:scales_down",
            "additionalbanners:scales_up",
            "additionalbanners:scales_left",
            "additionalbanners:scales_right"
        ]
    );
    let rare: Vec<_> = ITEMS.iter().filter(|i| i.1 == "epic").map(|i| i.0).collect();
    assert_eq!(rare, ["clubs", "diamond", "heart", "spades", "moon", "squares"]);
    assert_eq!(
        pattern_tag("additionalbanners:moon").as_deref(),
        Some("#minecraft:pattern_item/additionalbanners/moon")
    );
    assert_eq!(desc_key("additionalbanners:moon").as_deref(), Some("item.additionalbanners.moon.desc"));
    assert_eq!(pattern_tag("minecraft:creeper_banner_pattern"), None);
}

#[test]
fn mirrored_paths() {
    assert_eq!(
        mirror("data/additionalbanners/banner_pattern/anchor.json").as_deref(),
        Some("data/minecraft/banner_pattern/additionalbanners_anchor.json")
    );
    assert_eq!(
        mirror("assets/additionalbanners/textures/entity/banner/scales_up.png").as_deref(),
        Some("assets/minecraft/textures/entity/banner/additionalbanners_scales_up.png")
    );
    assert_eq!(
        mirror("assets/additionalbanners/textures/entity/shield/moon.png").as_deref(),
        Some("assets/minecraft/textures/entity/shield/additionalbanners_moon.png")
    );
    assert_eq!(
        mirror("data/additionalbanners/villager_trade/moon_shepherd.json").as_deref(),
        Some("data/minecraft/villager_trade/additionalbanners/moon_shepherd.json")
    );
    // Everything else stays where it is.
    for rel in [
        "data/minecraft/banner_pattern/creeper.json",
        "data/additionalbanners/recipe/pattern_anchor.json",
        "data/additionalbanners/banner_pattern/x/y.json",
        "assets/additionalbanners/textures/item/a.png",
        "assets/additionalbanners/lang/en_us.json",
        "data/minecraft/tags/villager_trade/shepherd/level_4.json",
        "data/additionalbanners/banner_pattern/.json",
    ] {
        assert_eq!(mirror(rel), None, "{rel}");
    }
    assert_eq!(
        horizon_pattern("additionalbanners:clubs").as_deref(),
        Some("minecraft:additionalbanners_clubs")
    );
}

#[test]
fn carry_matches_the_rules() {
    let Some(dir) = carry("data/additionalbanners/horizon/item") else {
        return;
    };
    let mut declared: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path().file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    declared.sort();
    let mut names: Vec<String> = ITEMS.iter().map(|i| i.0.to_string()).collect();
    names.sort();
    assert_eq!(declared, names);
    for (name, rarity) in ITEMS {
        let decl = read(&dir.join(format!("{name}.json")));
        assert!(decl.contains("\"minecraft:creeper_banner_pattern\""), "{name}");
        assert!(decl.contains(&format!("\"rarity\": \"{rarity}\"")), "{name}");
        assert!(decl.contains("\"max_stack_size\": 1"), "{name}");
        // The loom's tag of the item names Horizon's ids of the original patterns.
        let tag = read(&carry(&format!(
            "data/minecraft/tags/banner_pattern/pattern_item/additionalbanners/{name}.json"
        ))
        .unwrap());
        for p in item_patterns(name) {
            assert!(tag.contains(&horizon_pattern(&p).unwrap()), "{name}: {p}");
        }
    }
    for p in patterns() {
        let short = p.split_once(':').unwrap().1;
        let def = read(&carry(&format!("data/additionalbanners/banner_pattern/{short}.json")).unwrap());
        assert!(def.contains(&format!("\"minecraft:additionalbanners_{short}\"")), "{p}");
        assert!(def.contains(&format!("block.minecraft.banner.additionalbanners.{short}")), "{p}");
        for kind in ["banner", "shield"] {
            let rel = format!("assets/additionalbanners/textures/entity/{kind}/{short}.png");
            assert!(carry(&rel).unwrap().exists(), "{rel}");
            assert!(mirror(&rel).is_some(), "{rel}");
        }
    }
    // The 1.20.1 lang has every colour of every pattern under the key the registry entry names.
    let lang = read(&carry("assets/additionalbanners/lang/en_us.json").unwrap());
    for p in patterns() {
        let short = p.split_once(':').unwrap().1;
        for colour in ["white", "black", "light_blue"] {
            let key = format!("\"block.minecraft.banner.additionalbanners.{short}.{colour}\"");
            assert!(lang.contains(&key), "{key}");
        }
    }
    for (name, _) in ITEMS {
        let key = format!("\"item.additionalbanners.{name}.desc\"");
        assert!(lang.contains(&key), "{key}");
    }
}

#[test]
fn trades_follow_tuned_config() {
    let (Some(trades), true) = (carry("data/additionalbanners/villager_trade"), Path::new(CONFIG).exists())
    else {
        return;
    };
    let common = read(&carry("data/minecraft/tags/villager_trade/wandering_trader/common.json").unwrap());
    let rare = read(&carry("data/minecraft/tags/villager_trade/wandering_trader/uncommon.json").unwrap());
    let shepherd = read(&carry("data/minecraft/tags/villager_trade/shepherd/level_4.json").unwrap());
    let num = |j: &str, section: &str, key: &str| -> String {
        let s = &j[j.find(&format!("\"{section}\"")).unwrap()..];
        let s = &s[s.find(&format!("\"{key}\"")).unwrap() + key.len() + 3..];
        s.trim_start()
            .split([',', '\n', '}'])
            .next()
            .unwrap()
            .trim()
            .to_string()
    };
    let mut wandering = 0;
    let mut sheep = 0;
    for (name, _) in ITEMS {
        let cfg = read(&Path::new(CONFIG).join(format!("{name}.json")));
        for (section, suffix, pool) in [
            ("wanderingVillager", "wandering", ""),
            ("shepherdOffer", "shepherd", "shepherd"),
        ] {
            let file = trades.join(format!("{name}_{suffix}.json"));
            let id = format!("\"minecraft:additionalbanners/{name}_{suffix}\"");
            if num(&cfg, section, "enabled") != "true" {
                assert!(!file.exists(), "{name} {suffix}");
                continue;
            }
            let t = Trade {
                item: format!("additionalbanners:{name}"),
                emeralds: num(&cfg, section, "emeraldCost").parse().unwrap(),
                max_uses: num(&cfg, section, "maxUses").parse().unwrap(),
                xp: num(&cfg, section, "villagerExp").parse().unwrap(),
                price_multiplier: num(&cfg, section, "priceMultiplier").parse().unwrap(),
            };
            let written: String = read(&file).split_whitespace().collect();
            assert_eq!(written, trade_json(&t), "{name} {suffix}");
            if pool == "shepherd" {
                assert_eq!(tier_level(num(&cfg, section, "tier").trim_matches('"')), Some(4));
                assert!(shepherd.contains(&id), "{id}");
                sheep += 1;
            } else {
                let is_rare = num(&cfg, section, "isRareTrade") == "true";
                assert_eq!(rare.contains(&id), is_rare, "{id}");
                assert_eq!(common.contains(&id), !is_rare, "{id}");
                wandering += 1;
            }
        }
    }
    // the pack's config: every pattern at the wandering trader, the six epic ones at the shepherd.
    assert_eq!((wandering, sheep), (29, 6));
}

#[test]
fn shears_come_back_hurt_after_a_craft() {
    let before = vec![
        cell("minecraft:shears", 1, 10),
        cell("minecraft:paper", 5, 0),
        None,
        cell("minecraft:dragon_breath", 1, 0),
    ];
    let m = matched(&before, Some("additionalbanners:dragon")).expect("our recipe");
    assert_eq!(m.shears, 0);
    // ResultSlot.onTake: one of each taken, the breath's bottle left in its cell.
    let now = vec![None, cell("minecraft:paper", 4, 0), None, cell("minecraft:glass_bottle", 1, 0)];
    assert_eq!(after_take(&m, &now, 238), Shears::Back { cell: 0, damage: 11 });
    // Same with the cell emptied.
    let now = vec![None, cell("minecraft:paper", 4, 0), None, None];
    assert_eq!(after_take(&m, &now, 238), Shears::Back { cell: 0, damage: 11 });
    // The last point of durability: the shears break.
    let worn = vec![cell("minecraft:shears", 1, 237), cell("minecraft:paper", 1, 0), cell("minecraft:kelp", 1, 0)];
    let m = matched(&worn, Some("additionalbanners:kelp")).unwrap();
    assert_eq!(after_take(&m, &[None, None, None], 238), Shears::Broke);
}

#[test]
fn moving_items_is_not_a_craft() {
    let before = vec![cell("minecraft:shears", 1, 0), cell("minecraft:paper", 5, 0), cell("minecraft:kelp", 3, 0)];
    let m = matched(&before, Some("additionalbanners:kelp")).unwrap();
    // The player takes the shears out.
    let now = vec![None, cell("minecraft:paper", 5, 0), cell("minecraft:kelp", 3, 0)];
    assert_eq!(after_take(&m, &now, 238), Shears::Untouched);
    // ...or half the paper and the shears in two clicks (only the last grid is seen).
    let now = vec![None, cell("minecraft:paper", 2, 0), cell("minecraft:kelp", 3, 0)];
    assert_eq!(after_take(&m, &now, 238), Shears::Untouched);
    // An item dropped into an empty cell.
    let before = vec![cell("minecraft:shears", 1, 0), cell("minecraft:paper", 1, 0), None, cell("minecraft:kelp", 1, 0)];
    let m = matched(&before, Some("additionalbanners:kelp")).unwrap();
    let now = vec![None, None, cell("minecraft:dirt", 1, 0), None];
    assert_eq!(after_take(&m, &now, 238), Shears::Untouched);
    // Other recipes and grids with two shears are not ours.
    assert_eq!(matched(&before, Some("minecraft:paper")), None);
    let two = vec![cell("minecraft:shears", 1, 0), cell("minecraft:shears", 1, 0)];
    assert_eq!(matched(&two, Some("additionalbanners:kelp")), None);
    assert_eq!(matched(&before, None), None);
}

#[test]
fn shift_click_crafts_one_after_another() {
    // Quick-move crafting repeats onTake; each time the shears come back one point more worn.
    let mut grid = vec![cell("minecraft:shears", 1, 0), cell("minecraft:paper", 3, 0), cell("minecraft:ink_sac", 3, 0)];
    for round in 1..=3u32 {
        let m = matched(&grid, Some("additionalbanners:squid")).unwrap();
        let mut now: Vec<Option<Cell>> = grid
            .iter()
            .map(|c| {
                c.as_ref()
                    .and_then(|c| (c.count > 1).then(|| Cell { count: c.count - 1, ..c.clone() }))
            })
            .collect();
        match after_take(&m, &now, 238) {
            Shears::Back { cell: i, damage } => {
                assert_eq!(damage, round);
                now[i] = cell("minecraft:shears", 1, damage);
            }
            other => panic!("{other:?}"),
        }
        grid = now;
    }
    assert_eq!(grid, vec![cell("minecraft:shears", 1, 3), None, None]);
}
