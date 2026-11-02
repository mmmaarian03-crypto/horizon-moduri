#!/usr/bin/env python3
"""Writes mixins/ae2/horizon-mixin.toml and its helper files (one small function per hook).
  py -I -X utf8 mods/ae2/tools/gen_mixins.py"""
from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / "mixins" / "ae2"
CF = "::core::ops::ControlFlow"

HELPERS = {
    "use.rs": f"""fn ae2_use(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> core::ops::ControlFlow<bool> {{
    // AE2's onActivated for its blocks (open a screen, turn a crank, charge an item).
    if game.ae2_use(pos) {{
        core::ops::ControlFlow::Break(true)
    }} else {{
        core::ops::ControlFlow::Continue(())
    }}
}}
""",
    "placed.rs": """fn ae2_placed(game: &mut crate::game::Game<'_>, name: &str, pos: [i32; 3]) {
    game.ae2_placed(name, pos);
}
""",
    "tick.rs": """fn ae2_tick(game: &mut crate::game::Game<'_>) {
    game.ae2_tick();
}
""",
    "random.rs": """fn ae2_random(game: &mut crate::game::Game<'_>) {
    game.ae2_random_ticks();
}
""",
    "save.rs": """fn ae2_save(game: &mut crate::game::Game<'_>) {
    game.ae2_save();
}
""",
    "open.rs": """fn ae2_open() {
    crate::game::Game::ae2_world_opened();
}
""",
    "menu_ops.rs": f"""fn ae2_menu_ops(
    game: &mut crate::game::Game<'_>,
    ops: &[(Option<usize>, i32, horizon_client::menu::Input)],
) -> {CF}<bool> {{
    match game.ae2_menu_ops(ops) {{
        Some(handled) => {CF}::Break(handled),
        None => {CF}::Continue(()),
    }}
}}
""",
    "mouse.rs": f"""fn ae2_mouse(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
) -> {CF}<bool> {{
    if game.ae2_mouse(button) {{
        {CF}::Break(true)
    }} else {{
        {CF}::Continue(())
    }}
}}
""",
    "scroll.rs": """fn ae2_scroll(
    game: &mut crate::game::Game<'_>,
    delta: f32,
    slot: Option<usize>,
) -> core::ops::ControlFlow<bool> {
    match game.ae2_scroll(delta, slot) {
        Some(v) => core::ops::ControlFlow::Break(v),
        None => core::ops::ControlFlow::Continue(()),
    }
}
""",
    "drag.rs": """fn ae2_drag(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    // No mouse-tweak drags over an AE2 window (its slots are not real slots).
    if game.ae2_screen_height().is_some() {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
""",
    "typing.rs": f"""fn ae2_typing(game: &crate::game::Game<'_>) -> {CF}<bool> {{
    // The terminal's search box takes the keys the way the anvil's rename box does.
    if game.ae2_typing() {{
        {CF}::Break(true)
    }} else {{
        {CF}::Continue(())
    }}
}}
""",
    "key.rs": f"""fn ae2_key(game: &mut crate::game::Game<'_>, key: winit::keyboard::KeyCode) -> {CF}<bool> {{
    match game.ae2_key(key) {{
        Some(v) => {CF}::Break(v),
        None => {CF}::Continue(()),
    }}
}}
""",
    "text.rs": f"""fn ae2_text(game: &mut crate::game::Game<'_>, text: Option<&str>, backspace: bool) -> {CF}<()> {{
    if game.ae2_text(text, backspace) {{
        {CF}::Break(())
    }} else {{
        {CF}::Continue(())
    }}
}}
""",
    "height.rs": f"""fn ae2_height(game: &crate::game::Game<'_>) -> {CF}<f32> {{
    match game.ae2_screen_height() {{
        Some(h) => {CF}::Break(h),
        None => {CF}::Continue(()),
    }}
}}
""",
    "slot_position.rs": f"""fn ae2_slot_position(game: &crate::game::Game<'_>, i: usize) -> {CF}<Option<(f32, f32)>> {{
    match game.ae2_slot_position(i) {{
        Some(p) => {CF}::Break(p),
        None => {CF}::Continue(()),
    }}
}}
""",
    "draw.rs": f"""fn ae2_draw(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    ox: f32,
    oy: f32,
) -> {CF}<bool> {{
    if game.ae2_draw(batch, ox, oy) {{
        {CF}::Break(true)
    }} else {{
        {CF}::Continue(())
    }}
}}
""",
    "origin.rs": f"""fn ae2_origin(
    game: &crate::game::Game<'_>,
    batch: &horizon_client::hud::HudBatch,
) -> {CF}<(f32, f32)> {{
    match game.ae2_origin(batch) {{
        Some(o) => {CF}::Break(o),
        None => {CF}::Continue(()),
    }}
}}
""",
    "image.rs": f"""fn ae2_image(
    game: &crate::game::Game<'_>,
    batch: &horizon_client::hud::HudBatch,
) -> {CF}<horizon_client::container_screen::Image> {{
    match game.ae2_image(batch) {{
        Some(i) => {CF}::Break(i),
        None => {CF}::Continue(()),
    }}
}}
""",
    "overlay.rs": """fn ae2_overlay(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    game.ae2_overlay(batch);
}
""",
    "close.rs": """fn ae2_close(game: &mut crate::game::Game<'_>) {
    game.ae2_close();
}
""",
    "move_in.rs": f"""fn ae2_move_in(
    game: &mut crate::game::Game<'_>,
    view: &View,
    stack: &ItemStack,
    slot: usize,
) -> {CF}<Option<ItemStack>> {{
    // A hopper never fills an AE2 window's temporary slots.
    match view.locate(slot) {{
        Some((pos, _)) if game.ae2_window_at(pos) => {CF}::Break(Some(stack.clone())),
        _ => {CF}::Continue(()),
    }}
}}
""",
    "take_one.rs": f"""fn ae2_take_one(
    game: &crate::game::Game<'_>,
    view: &View,
    slot: usize,
) -> {CF}<Option<ItemStack>> {{
    // ...nor takes from them.
    match view.locate(slot) {{
        Some((pos, _)) if game.ae2_window_at(pos) => {CF}::Break(None),
        _ => {CF}::Continue(()),
    }}
}}
""",
    "save_id.rs": f"""fn ae2_save_id(e: &crate::containers::BlockEntity, block: &str) -> {CF}<String> {{
    // An AE2 window's temporary block entity is saved under its block's id...
    if block.starts_with("ae2:") && matches!(e, crate::containers::BlockEntity::Chest(_)) {{
        {CF}::Break(block.to_owned())
    }} else {{
        {CF}::Continue(())
    }}
}}
""",
    "save_load.rs": f"""fn ae2_save_load(tag: &Tag) -> {CF}<Option<((i32, i32, i32), crate::containers::BlockEntity)>> {{
    // ...and never loaded back (its slots only mirrored the network).
    match text(tag.get("id")) {{
        Some(id) if id.starts_with("ae2:") => {CF}::Break(None),
        _ => {CF}::Continue(()),
    }}
}}
""",
    "generate.rs": """fn ae2_generate(
    value: Result<DecorateStats, GeneratorError>,
    region: &mut [ProtoChunk; 9],
) -> Result<DecorateStats, GeneratorError> {
    if value.is_ok() {
        ae2::place_meteorite(region);
    }
    value
}
""",
}

G = "{ name = \"game\", ty = \"&mut crate::game::Game<'_>\", value = \"&mut *self\" }"
GR = "{ name = \"game\", ty = \"&crate::game::Game<'_>\", value = \"self\" }"
GR2 = "{ name = \"game\", ty = \"&crate::game::Game<'_>\", value = \"&*self\" }"


def b(name, ty, value):
    return f'{{ name = "{name}", ty = "{ty}", value = "{value}" }}'


def inj(id, target, helper, bindings, cancellable=False, kind="HEAD", comment=""):
    s = (f"# {comment}\n" if comment else "") + f'[[injection]]\nid = "{id}"\nkind = "{kind}"\n'
    if cancellable:
        s += "cancellable = true\n"
    s += f'target = "{target}"\nhelper = "{helper}"\n'
    if bindings:
        s += "bindings = [\n" + "".join(f"  {x},\n" for x in bindings) + "]\n"
    else:
        s += "bindings = []\n"
    return s + "\n"


HEADER = """# Applied Energistics 2 (forge 15.4.11, stage 1: the item storage network) for Horizon: native mod,
# private use. Needs the native mod `foundry` in the same build (energy in through foundry, items moved
# through foundry's item handlers). Generated by tools/gen_mixins.py.
[mod]
id = "ae2"
version = "0.1.0"
priority = 100

[compatibility]
horizon_revision = "142388e1c136e622652ec7505af15d8ad195e041"

# The rules and the game side (native/horizon.rs + src/lib.rs).
[[module]]
id = "runtime"
file = "crates/horizon-client/src/game.rs"
name = "ae2"
source = "runtime.rs"

# Meteorites (native/worldgen.rs + src/lib.rs, tools/bundle_extra.py).
[[module]]
id = "worldgen"
file = "crates/horizon-world/src/worldgen/generator.rs"
name = "ae2"
source = "worldgen_runtime.rs"

"""

INJ = [
    inj("use", "horizon_client::game::copper_golems::Game<'_>::use_golem_statue", "use.rs",
        [G, b("pos", "[i32;3]", "pos")], True, comment="Right click on an AE2 block (before the block's own use)."),
    inj("placed", "horizon_client::game::advancements::Game<'_>::trigger_block", "placed.rs",
        [G, b("name", "&str", "name"), b("pos", "[i32; 3]", "pos")], comment="BlockItem.place's last step: a placed AE2 block joins its network."),
    inj("tick", "horizon_client::game::block_entity_ticks::Game<'_>::tick_block_entities", "tick.rs", [G],
        comment="Every tick: networks, power, machines, buses, interfaces."),
    inj("random", "horizon_client::game::Game<'_>::tick_blocks", "random.rs", [G], comment="Random ticks of the budding certus quartz."),
    inj("save", "horizon_client::game::Game<'_>::save_game_metadata", "save.rs", [G], comment="The world's AE2 records are written with the world..."),
    inj("open", "horizon_client::game::chat::Game<'_>::load_world_state", "open.rs", [], comment="...and forgotten when another world opens."),
    inj("menu-ops", "horizon_client::game::Game<'_>::menu_ops", "menu_ops.rs",
        [G, b("ops", "&[(Option<usize>, i32, horizon_client::menu::Input)]", "ops")], True,
        comment="Menu clicks that would touch an AE2 window's slots."),
    inj("mouse", "horizon_client::game::tweaks::Game<'_>::sort_mouse_press", "mouse.rs",
        [G, b("button", "winit::event::MouseButton", "button")], True,
        comment="Every mouse press on a screen: the AE2 window's slots, search box, buttons, scrollbar."),
    inj("scroll", "horizon_client::game::tweaks::Game<'_>::tweak_scroll", "scroll.rs",
        [G, b("delta", "f32", "delta"), b("slot", "Option<usize>", "slot")], True, comment="The wheel over a terminal scrolls its grid."),
    inj("drag", "horizon_client::game::tweaks::Game<'_>::tweak_drag_move", "drag.rs", [GR2], True, comment="No mouse-tweak drag over an AE2 window."),
    inj("typing", "horizon_client::game::stations::Game<'_>::station_typing", "typing.rs", [GR], True,
        comment="The terminal's search box: typing goes the anvil rename box's way."),
    inj("key", "horizon_client::game::stations::Game<'_>::station_key", "key.rs", [G, b("key", "winit::keyboard::KeyCode", "key")], True),
    inj("text", "horizon_client::game::stations::Game<'_>::station_text", "text.rs",
        [G, b("text", "Option<&str>", "text"), b("backspace", "bool", "backspace")], True),
    inj("height", "horizon_client::game::machines::Game<'_>::screen_height", "height.rs", [GR], True,
        comment="The window's geometry and drawing (ironchest's and foundry's screen hooks)."),
    inj("slot-position", "horizon_client::game::machines::Game<'_>::machine_slot_position", "slot_position.rs", [GR, b("i", "usize", "i")], True),
    inj("draw", "horizon_client::game::machines::Game<'_>::draw_machine_screen", "draw.rs",
        [GR, b("batch", "&mut horizon_client::hud::HudBatch", "&mut *batch"), b("ox", "f32", "ox"), b("oy", "f32", "oy")], True),
    inj("origin", "horizon_client::game::Game<'_>::inventory_origin", "origin.rs", [GR, b("batch", "&horizon_client::hud::HudBatch", "batch")], True),
    inj("image", "horizon_client::game::Game<'_>::container_image", "image.rs", [GR, b("batch", "&horizon_client::hud::HudBatch", "batch")], True),
    inj("overlay", "horizon_client::game::Game<'_>::draw_carried", "overlay.rs",
        [GR2, b("batch", "&mut horizon_client::hud::HudBatch", "&mut *batch")], comment="Over the slots: the amounts, the stock sizes, the status line."),
    inj("close", "horizon_client::game::Game<'_>::close_container", "close.rs", [G], comment="The window's temporary block entity goes away with the screen."),
    inj("move-in", "horizon_client::game::machines::Game<'_>::try_move_in_item", "move_in.rs",
        [G, b("view", "&View", "view"), b("stack", "&ItemStack", "&stack"), b("slot", "usize", "slot")], True,
        comment="Hoppers never touch a window's temporary slots."),
    inj("take-one", "horizon_client::game::machines::Game<'_>::take_one", "take_one.rs",
        [GR2, b("view", "&View", "view"), b("slot", "usize", "slot")], True),
    inj("save-id", "horizon_client::save::block_entity_id", "save_id.rs",
        [b("e", "&crate::containers::BlockEntity", "e"), b("block", "&str", "block")], True,
        comment="AE2 block entities (window mirrors) are saved under the block id and never loaded back."),
    inj("save-load", "horizon_client::save::decode_block_entity", "save_load.rs", [b("tag", "&Tag", "tag")], True),
    inj("generate", "horizon_world::worldgen::generator::GeneratorWorker::decorate", "generate.rs",
        [b("value", "Result<DecorateStats, GeneratorError>", "$value"), b("region", "&mut [ProtoChunk; 9]", "&mut *region")],
        kind="RETURN", comment="Meteorites with the presses (native worldgen step)."),
]

for name, text in HELPERS.items():
    (OUT / name).write_text(text, encoding="utf-8", newline="\n")
(OUT / "horizon-mixin.toml").write_text(HEADER + "".join(INJ), encoding="utf-8", newline="\n")
print("ok", len(HELPERS), "helpers")
