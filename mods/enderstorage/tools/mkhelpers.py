"""Writes the injection helpers of mixins/enderstorage/ (one small function each)."""
from pathlib import Path

out = Path(__file__).resolve().parents[1] / "mixins/enderstorage"
out.mkdir(parents=True, exist_ok=True)

FILES = {
"placed.rs": """fn enderstorage_placed(
    game: &mut crate::game::Game<'_>,
    name: &str,
    pos: [i32; 3],
    stack: &horizon_client::items::ItemStack,
) {
    game.enderstorage_placed(name, pos, stack);
}
""",
"use.rs": """fn enderstorage_use(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    held: Option<&horizon_client::items::ItemStack>,
) -> ::core::ops::ControlFlow<bool> {
    // BlockEnderStorage.use: the buttons, the latch and the tank's bucket, before the block's own use.
    if game.enderstorage_use(pos, held) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
""",
"pouch.rs": """fn enderstorage_pouch(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> ::core::ops::ControlFlow<bool> {
    // ItemEnderPouch.use / onItemUseFirst.
    if game.enderstorage_pouch(stack) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
""",
"blocked.rs": """fn enderstorage_blocked(name: &str) -> ::core::ops::ControlFlow<bool> {
    // No cat or conductor rule on the Ender Chest (BaseEntityBlock); the tank never opens a menu
    // (true = the use counts, no menu).
    match name {
        "enderstorage:ender_chest" => ::core::ops::ControlFlow::Break(false),
        "enderstorage:ender_tank" => ::core::ops::ControlFlow::Break(true),
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
""",
"open.rs": """fn enderstorage_open(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    name: &str,
) -> ::core::ops::ControlFlow<bool> {
    if game.enderstorage_open(pos, name) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
""",
"rows.rs": """fn enderstorage_rows(
    game: &crate::game::Game<'_>,
) -> ::core::ops::ControlFlow<Option<std::sync::Arc<str>>> {
    match game.enderstorage_container_block() {
        Some(name) => ::core::ops::ControlFlow::Break(Some(name)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
""",
"title.rs": """fn enderstorage_title(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<String> {
    match game.enderstorage_title() {
        Some(title) => ::core::ops::ControlFlow::Break(title),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
""",
"craft.rs": """fn enderstorage_craft(
    game: &crate::game::Game<'_>,
) -> ::core::ops::ControlFlow<Option<(String, ItemStack)>> {
    // CreateRecipe and ReColourRecipe first, then the game's own recipes.
    match game.enderstorage_craft() {
        Some(found) => ::core::ops::ControlFlow::Break(Some(found)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
""",
"tooltip.rs": """fn enderstorage_tooltip(
    stack: &crate::items::ItemStack,
    translate: &dyn Fn(&str) -> Option<String>,
) -> ::core::ops::ControlFlow<Option<String>> {
    // ItemEnderStorage / ItemEnderPouch.appendHoverText: the three colours.
    match crate::enderstorage::tooltip(stack, translate) {
        Some(line) => ::core::ops::ControlFlow::Break(Some(line)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
""",
"tick.rs": """fn enderstorage_tick(game: &mut crate::game::Game<'_>) {
    game.enderstorage_tick();
}
""",
"load.rs": """fn enderstorage_load(game: &mut crate::game::Game<'_>) {
    game.enderstorage_load();
}
""",
"save.rs": """fn enderstorage_save(game: &mut crate::game::Game<'_>) {
    game.enderstorage_save();
}
""",
"loot.rs": """fn enderstorage_loot(
    value: Vec<ItemStack>,
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    state: BlockStateId,
) -> Vec<ItemStack> {
    game.enderstorage_loot(pos, state, value)
}
""",
"closing.rs": """fn enderstorage_closing(game: &mut crate::game::Game<'_>, key: (i32, i32, i32)) {
    game.enderstorage_closing(key);
}
""",
"sprites.rs": """fn enderstorage_sprites(game: &mut crate::game::Game<'_>) {
    game.enderstorage_sprites();
}
""",
"pre_remove.rs": """fn enderstorage_pre_remove(name: &str) -> ::core::ops::ControlFlow<PreRemove> {
    // The items live in the frequency's inventory, not in the block: removing a chest spills nothing.
    if name == "enderstorage:ender_chest" || name == "enderstorage:ender_tank" {
        ::core::ops::ControlFlow::Break(PreRemove::Nothing)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
""",
"save_id.rs": """fn enderstorage_save_id(
    e: &crate::containers::BlockEntity,
    block: &str,
) -> ::core::ops::ControlFlow<String> {
    // The block entity type id of the chest is its block's id (enderstorage:ender_chest).
    if matches!(e, crate::containers::BlockEntity::Chest(_)) && block == crate::enderstorage::CHEST {
        ::core::ops::ControlFlow::Break(block.to_owned())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
""",
"save_load.rs": """fn enderstorage_load_entity(
    tag: &Tag,
) -> ::core::ops::ControlFlow<Option<((i32, i32, i32), crate::containers::BlockEntity)>> {
    // The working copy of the chest: the slots of the configured inventory size.
    let Some(id) = text(tag.get("id")) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if id != crate::enderstorage::CHEST {
        return ::core::ops::ControlFlow::Continue(());
    }
    let int = |k: &str| number(tag.get(k)).map(|v| v as i32);
    let (Some(x), Some(y), Some(z)) = (int("x"), int("y"), int("z")) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let items = decode_items(
        tag.get("Items"),
        crate::enderstorage::SIZES[crate::enderstorage::ITEM_STORAGE_SIZE],
    );
    ::core::ops::ControlFlow::Break(Some((
        (x, y, z),
        crate::containers::BlockEntity::Chest(items),
    )))
}
""",
"render_index.rs": """fn enderstorage_render_index(registry: &BlockRegistry) {
    // Which states are Ender Chests, and facing which way, for `kind` below.
    let codes = (0..registry.len() as u32)
        .map(|id| {
            registry.state(BlockStateId(id)).map_or(0, |s| {
                crate::enderstorage::state_code(&s.name, s.properties.get("facing").map(String::as_str))
            })
        })
        .collect();
    crate::enderstorage::set_states(codes);
}
""",
"render_kind.rs": """fn enderstorage_kind(this: &BlockEntityRenderer, state: u32) -> ::core::ops::ControlFlow<&Kind> {
    // RenderTileEnderChest: the chest model with the mod's texture, turned by the facing, never
    // paired, the lid opening as a chest's (the buttons and the latch are not drawn).
    static KINDS: std::sync::OnceLock<Vec<Kind>> = std::sync::OnceLock::new();
    let _ = this;
    let Some(facing) = crate::enderstorage::chest_state(state) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let kinds = KINDS.get_or_init(|| {
        crate::enderstorage::FACINGS
            .iter()
            .map(|f| Kind::Chest {
                block: crate::enderstorage::CHEST.to_owned(),
                texture: crate::enderstorage::CHEST_TEXTURE,
                part: ChestPart::Single,
                y_rot: y_rot(f),
                double_with: None,
            })
            .collect()
    });
    match kinds.get(facing) {
        Some(kind) => ::core::ops::ControlFlow::Break(kind),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
""",
"anim_index.rs": """fn enderstorage_anim_index(registry: &horizon_world::registry::BlockRegistry) {
    let codes = (0..registry.len() as u32)
        .map(|id| {
            registry
                .state(horizon_world::registry::BlockStateId(id))
                .map_or(0, |s| {
                    crate::enderstorage::state_code(&s.name, s.properties.get("facing").map(String::as_str))
                })
        })
        .collect();
    crate::enderstorage::set_states(codes);
}
""",
"anim_ticker.rs": """fn enderstorage_ticker(state: u32) -> ::core::ops::ControlFlow<Option<Ticker>> {
    // The lid of the Ender Chest (TileEnderChest.a_lidAngle) opens and closes as a chest's.
    if crate::enderstorage::chest_state(state).is_some() {
        ::core::ops::ControlFlow::Break(Some(Ticker::Lid))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
""",
}

for name, text in FILES.items():
    (out / name).write_text(text, encoding="utf-8", newline="\n")
print(len(FILES), "helpers")
